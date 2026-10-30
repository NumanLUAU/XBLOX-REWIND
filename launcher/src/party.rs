/*
    written by: @corescripts
*/

//! Parties over plain TCP: one launcher hosts on a port, any number of others connect to the
//! host's IP and that port. Messages are one JSON object per line. The host keeps the member
//! list (Roblox user, and the place / server each member is in) and sends it to everyone;
//! when the host's own server changes it tells everyone to follow.

// >> imports
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// First port tried when hosting; the next ones are used if it is taken.
pub const DEFAULT_PORT: u16 = 27500;
/// A line longer than this ends the connection (nothing legitimate comes close).
const MAX_LINE: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Member {
    pub uid: u64,
    pub name: String,
    pub display: String,
    /// place and server (job id) the member is playing in, if any
    pub place_id: Option<u64>,
    pub job_id: Option<String>,
    /// what the member is doing: a game's name, "Studio", "Roblox"...
    pub game: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t")]
enum Msg {
    /// client -> host when it connects
    Hello { me: Member },
    /// anyone -> host: where I am now
    Presence { place_id: Option<u64>, job_id: Option<String>, game: String },
    /// host -> all: the party
    State { host: u64, members: Vec<Member> },
    /// host -> all: the host is in this server
    Follow { place_id: u64, job_id: String, game: String },
    /// ask a member for their current server (relayed by the host)
    WhereAreYou { from: u64, to: u64 },
    /// the answer (relayed by the host)
    Location { from: u64, to: u64, place_id: Option<u64>, job_id: Option<String>, game: String },
    /// host -> all: the party is over
    Closed,
    /// host -> a client it turned away (why)
    Rejected { reason: String },
}

#[derive(Clone, Debug)]
pub enum Event {
    /// the member list changed
    Members,
    /// the host joined a server: follow (auto / prompt per settings)
    Follow { place_id: u64, job_id: String, game: String },
    /// a member answered where they are
    Location { uid: u64, place_id: Option<u64>, job_id: Option<String>, game: String },
    /// the host closed the party or the connection dropped
    Ended(String),
}

pub enum Role {
    Host { port: u16 },
    Client { addr: String },
}

struct Shared {
    host: u64,
    members: Vec<Member>,
    /// host only: a write half per connected client
    clients: HashMap<u64, TcpStream>,
}

pub struct Party {
    pub role: Role,
    me: u64,
    shared: Arc<Mutex<Shared>>,
    events: Receiver<Event>,
    /// client only: the connection to the host
    upstream: Option<TcpStream>,
}

fn send(stream: &mut TcpStream, msg: &Msg) -> bool {
    let Ok(mut line) = serde_json::to_vec(msg) else { return false };
    line.push(b'\n');
    stream.write_all(&line).is_ok()
}

/// Reads one JSON message per line; None when the connection ends or misbehaves.
fn lines(stream: TcpStream) -> impl Iterator<Item = Msg> {
    let mut reader = BufReader::new(stream);
    std::iter::from_fn(move || {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = (&mut reader).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf).ok()?;
            if n == 0 || buf.len() > MAX_LINE {
                return None;
            }
            if let Ok(m) = serde_json::from_slice::<Msg>(&buf) {
                return Some(m);
            }
        }
    })
}

use std::io::Read;

/// A Roblox server id is a GUID; anything else is not passed to the launcher.
pub fn valid_job_id(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

impl Party {
    /// Host a party on the first free port from DEFAULT_PORT.
    pub fn host(me: Member) -> std::io::Result<Party> {
        let mut listener = None;
        let mut port = DEFAULT_PORT;
        for p in DEFAULT_PORT..DEFAULT_PORT + 50 {
            if let Ok(l) = TcpListener::bind(("0.0.0.0", p)) {
                listener = Some(l);
                port = p;
                break;
            }
        }
        let listener = listener.ok_or_else(|| std::io::Error::other("no free port"))?;
        let (tx, events) = channel();
        let uid = me.uid;
        let shared = Arc::new(Mutex::new(Shared { host: uid, members: vec![me], clients: HashMap::new() }));
        let sh = shared.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(stream) = conn else { continue };
                let sh = sh.clone();
                let tx = tx.clone();
                std::thread::spawn(move || host_client(stream, sh, tx));
            }
        });
        Ok(Party { role: Role::Host { port }, me: uid, shared, events, upstream: None })
    }

    /// Join the party hosted at `addr`: an IP or host name, with or without a port, as typed or
    /// pasted (playit.gg / tunnel links included, see `candidates`).
    pub fn join(addr: &str, me: Member) -> std::io::Result<Party> {
        let targets = candidates(addr);
        if targets.is_empty() {
            return Err(std::io::Error::other(format!("could not find \"{}\"", addr.trim())));
        }
        let mut last = std::io::Error::other("no address to connect to");
        let mut stream = None;
        for t in &targets {
            match TcpStream::connect_timeout(t, Duration::from_secs(4)) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(e) => last = e,
            }
        }
        let Some(mut stream) = stream else { return Err(last) };
        stream.set_nodelay(true).ok();
        let uid = me.uid;
        if !send(&mut stream, &Msg::Hello { me: me.clone() }) {
            return Err(std::io::Error::other("could not reach the host"));
        }
        let (tx, events) = channel();
        let shared = Arc::new(Mutex::new(Shared { host: 0, members: vec![me], clients: HashMap::new() }));
        let sh = shared.clone();
        let reader = stream.try_clone()?;
        let mut writer = stream.try_clone()?;
        std::thread::spawn(move || {
            for m in lines(reader) {
                match m {
                    Msg::State { host, members } => {
                        let mut s = sh.lock().unwrap();
                        s.host = host;
                        s.members = members;
                        let _ = tx.send(Event::Members);
                    }
                    Msg::Follow { place_id, job_id, game } => {
                        let _ = tx.send(Event::Follow { place_id, job_id, game });
                    }
                    Msg::WhereAreYou { from, to } if to == uid => {
                        // answer with what the host last heard from us
                        let me = sh.lock().unwrap().members.iter().find(|m| m.uid == uid).cloned().unwrap_or_default();
                        send(&mut writer, &Msg::Location { from: uid, to: from, place_id: me.place_id, job_id: me.job_id, game: me.game });
                    }
                    Msg::Location { from, to, place_id, job_id, game } if to == uid => {
                        let _ = tx.send(Event::Location { uid: from, place_id, job_id, game });
                    }
                    Msg::Closed => {
                        let _ = tx.send(Event::Ended("The host closed the party.".into()));
                        return;
                    }
                    Msg::Rejected { reason } => {
                        let _ = tx.send(Event::Ended(reason));
                        return;
                    }
                    _ => {}
                }
            }
            let _ = tx.send(Event::Ended("Lost the connection to the party host.".into()));
        });
        Ok(Party { role: Role::Client { addr: addr.to_string() }, me: uid, shared, events, upstream: Some(stream) })
    }

    pub fn is_host(&self) -> bool {
        matches!(self.role, Role::Host { .. })
    }

    pub fn host_uid(&self) -> u64 {
        self.shared.lock().unwrap().host
    }

    pub fn members(&self) -> Vec<Member> {
        self.shared.lock().unwrap().members.clone()
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Tell the party where we are. On the host a new server also makes everyone follow.
    pub fn set_presence(&mut self, place_id: Option<u64>, job_id: Option<String>, game: String) {
        if let Some(up) = self.upstream.as_mut() {
            send(up, &Msg::Presence { place_id, job_id: job_id.clone(), game: game.clone() });
            let mut s = self.shared.lock().unwrap();
            if let Some(m) = s.members.iter_mut().find(|m| m.uid == self.me) {
                m.place_id = place_id;
                m.job_id = job_id;
                m.game = game;
            }
            return;
        }
        let mut s = self.shared.lock().unwrap();
        let me = self.me;
        let Some(m) = s.members.iter_mut().find(|m| m.uid == me) else { return };
        let moved = m.job_id != job_id && job_id.is_some();
        m.place_id = place_id;
        m.job_id = job_id.clone();
        m.game = game.clone();
        broadcast_state(&mut s);
        if let (true, Some(p), Some(j)) = (moved, place_id, job_id) {
            let msg = Msg::Follow { place_id: p, job_id: j, game };
            for c in s.clients.values_mut() {
                send(c, &msg);
            }
        }
    }

    /// Ask a member for the server they are in; the answer arrives as Event::Location.
    pub fn request_location(&mut self, uid: u64) {
        if let Some(up) = self.upstream.as_mut() {
            send(up, &Msg::WhereAreYou { from: self.me, to: uid });
            return;
        }
        let mut s = self.shared.lock().unwrap();
        if let Some(c) = s.clients.get_mut(&uid) {
            send(c, &Msg::WhereAreYou { from: self.me, to: uid });
        }
    }

    /// Leave; the host closes the party for everyone.
    pub fn leave(self) {
        if let Some(up) = self.upstream {
            let _ = up.shutdown(Shutdown::Both);
            return;
        }
        let mut s = self.shared.lock().unwrap();
        for c in s.clients.values_mut() {
            send(c, &Msg::Closed);
            let _ = c.shutdown(Shutdown::Both);
        }
        s.clients.clear();
    }
}

fn broadcast_state(s: &mut Shared) {
    let msg = Msg::State { host: s.host, members: s.members.clone() };
    let mut dead = Vec::new();
    for (uid, c) in s.clients.iter_mut() {
        if !send(c, &msg) {
            dead.push(*uid);
        }
    }
    for d in dead {
        s.clients.remove(&d);
        s.members.retain(|m| m.uid != d);
    }
}

/// Host side of one client connection.
fn host_client(stream: TcpStream, sh: Arc<Mutex<Shared>>, tx: Sender<Event>) {
    stream.set_nodelay(true).ok();
    let Ok(writer) = stream.try_clone() else { return };
    let mut it = lines(stream);
    // the first message must introduce the member
    let Some(Msg::Hello { me }) = it.next() else { return };
    let uid = me.uid;
    {
        let mut s = sh.lock().unwrap();
        let why = if uid == 0 {
            Some("You need to be signed in to join a party.")
        } else if uid == s.host {
            Some("You are signed in with the same Roblox account as the party host. Sign in with another account to join.")
        } else if s.clients.contains_key(&uid) {
            Some("That Roblox account is already in this party.")
        } else {
            None
        };
        if let Some(reason) = why {
            if let Ok(mut w) = writer.try_clone() {
                send(&mut w, &Msg::Rejected { reason: reason.into() });
            }
            return;
        }
        s.members.retain(|m| m.uid != uid);
        s.members.push(Member { place_id: None, job_id: None, ..me });
        s.clients.insert(uid, writer);
        broadcast_state(&mut s);
        // a newcomer follows the host straight away if the host is in a game
        if let Some(h) = s.members.iter().find(|m| m.uid == s.host).cloned() {
            if let (Some(p), Some(j)) = (h.place_id, h.job_id) {
                if let Some(c) = s.clients.get_mut(&uid) {
                    send(c, &Msg::Follow { place_id: p, job_id: j, game: h.game });
                }
            }
        }
    }
    let _ = tx.send(Event::Members);
    for m in it {
        let mut s = sh.lock().unwrap();
        match m {
            Msg::Presence { place_id, job_id, game } => {
                if let Some(mm) = s.members.iter_mut().find(|m| m.uid == uid) {
                    mm.place_id = place_id;
                    mm.job_id = job_id.filter(|j| valid_job_id(j));
                    mm.game = game.chars().take(120).collect();
                }
                broadcast_state(&mut s);
                let _ = tx.send(Event::Members);
            }
            Msg::WhereAreYou { to, .. } => {
                if to == s.host {
                    // the host answers for itself
                    let h = s.members.iter().find(|m| m.uid == s.host).cloned().unwrap_or_default();
                    let host = s.host;
                    if let Some(c) = s.clients.get_mut(&uid) {
                        send(c, &Msg::Location { from: host, to: uid, place_id: h.place_id, job_id: h.job_id, game: h.game });
                    }
                } else if let Some(c) = s.clients.get_mut(&to) {
                    send(c, &Msg::WhereAreYou { from: uid, to });
                }
            }
            Msg::Location { to, place_id, job_id, game, .. } => {
                if to == s.host {
                    let _ = tx.send(Event::Location { uid, place_id, job_id, game });
                } else if let Some(c) = s.clients.get_mut(&to) {
                    send(c, &Msg::Location { from: uid, to, place_id, job_id, game });
                }
            }
            _ => {}
        }
    }
    // disconnected
    let mut s = sh.lock().unwrap();
    s.clients.remove(&uid);
    s.members.retain(|m| m.uid != uid);
    broadcast_state(&mut s);
    let _ = tx.send(Event::Members);
}

// >> addresses

/// Every address worth trying for what the player typed: "1.2.3.4:27500", "name.ply.gg:12345",
/// "https://name.ply.gg/", "[::1]:27500" or a bare host. Without a port the host's SRV record is
/// asked first (playit.gg's joinmc.link addresses carry their port there), then the party's
/// default port and 80.
pub fn candidates(input: &str) -> Vec<SocketAddr> {
    let mut s = input.trim();
    if let Some(i) = s.find("://") {
        s = &s[i + 3..];
    }
    let s = s.split(['/', '?', '#']).next().unwrap_or("").trim().trim_end_matches('.');
    let s = s.rsplit('@').next().unwrap_or(s);
    if s.is_empty() {
        return Vec::new();
    }
    // host and optional port ("[v6]:port", "v6", "host:port", "host")
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, tail) = rest.split_once(']').unwrap_or((rest, ""));
        (h.to_string(), tail.strip_prefix(':').and_then(|p| p.parse::<u16>().ok()))
    } else if s.matches(':').count() == 1 {
        let (h, p) = s.split_once(':').unwrap();
        (h.to_string(), p.parse::<u16>().ok())
    } else {
        (s.to_string(), None)
    };
    let mut ports: Vec<(String, u16)> = Vec::new();
    match port {
        Some(p) => ports.push((host.clone(), p)),
        None => {
            if host.parse::<std::net::IpAddr>().is_err() {
                ports.extend(srv_lookup(&host));
            }
            ports.push((host.clone(), DEFAULT_PORT));
            ports.push((host.clone(), 80));
        }
    }
    let mut out: Vec<SocketAddr> = Vec::new();
    for (h, p) in ports {
        if let Ok(addrs) = (h.as_str(), p).to_socket_addrs() {
            // IPv4 first: most tunnels and home routers only forward it
            let mut v: Vec<SocketAddr> = addrs.collect();
            v.sort_by_key(|a| a.is_ipv6());
            for a in v {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
    }
    out
}

/// SRV targets for a host (DNS over HTTPS): `_minecraft._tcp` is where playit.gg publishes the
/// real port of its joinmc.link names; `_xblox._tcp` lets anyone point a domain at a party.
fn srv_lookup(host: &str) -> Vec<(String, u16)> {
    let mut out = Vec::new();
    for svc in ["_xblox._tcp", "_minecraft._tcp"] {
        let url = format!("https://cloudflare-dns.com/dns-query?name={svc}.{host}&type=SRV");
        let Ok(r) = ureq::get(&url).set("accept", "application/dns-json").timeout(Duration::from_secs(4)).call() else { continue };
        let Ok(v) = r.into_json::<serde_json::Value>() else { continue };
        for a in v["Answer"].as_array().cloned().unwrap_or_default() {
            // "priority weight port target."
            let data = a["data"].as_str().unwrap_or("");
            let f: Vec<&str> = data.split_whitespace().collect();
            if let (Some(p), Some(t)) = (f.get(2).and_then(|p| p.parse::<u16>().ok()), f.get(3)) {
                out.push((t.trim_end_matches('.').to_string(), p));
            }
        }
    }
    out
}

// >> end << //
