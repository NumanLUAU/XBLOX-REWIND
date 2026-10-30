/*
    written by: @corescripts
*/

//! Content resolution and loading.
//!
//! `rbxasset://` paths resolve against the local content folders (xboxassets first,
//! then genericcontent). Remote asset ids are fetched from Roblox asset delivery with
//! the signed-in user's cookie and cached on disk, so later launches work offline.
//! `rbxclient://` paths resolve against the newest installed Roblox client's content folder
//! (Roblox or a bootstrapper), for modern UI art such as the current Robux icon.

// >> imports
use crate::api::Session;
use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Content {
    pub root: PathBuf,
    pub cache: PathBuf,
    pub client: Option<PathBuf>,
}

/// content/ folder of the most recently installed Roblox player.
fn installed_client_content() -> Option<PathBuf> {
    let local = dirs::data_local_dir()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for app in ["Roblox", "Bloxstrap", "Fishstrap", "Froststrap", "Voidstrap"] {
        let Ok(rd) = std::fs::read_dir(local.join(app).join("Versions")) else { continue };
        for e in rd.flatten() {
            let c = e.path().join("content");
            if !c.join("textures").is_dir() {
                continue;
            }
            let t = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().map(|b| t > b.0).unwrap_or(true) {
                best = Some((t, c));
            }
        }
    }
    best.map(|b| b.1)
}

impl Content {
    pub fn new(root: PathBuf) -> Self {
        let cache = crate::config::data_dir().join("cache");
        let _ = std::fs::create_dir_all(cache.join("assets"));
        let _ = std::fs::create_dir_all(cache.join("web"));
        Self { root, cache, client: installed_client_content() }
    }

    /// Resolve an `rbxasset://` path to a file inside the content folders.
    pub fn local(&self, uri: &str) -> Option<PathBuf> {
        let rel = uri.strip_prefix("rbxasset://").unwrap_or(uri).trim_start_matches('/');
        for base in ["xboxassets", "genericcontent", "launcher/assets"] {
            let p = self.root.join(base).join(rel);
            if p.exists() {
                if std::env::var("XBOX_ASSET_LOG").is_ok() {
                    eprintln!("asset-used {base}/{rel}");
                }
                return Some(p);
            }
        }
        // debug: XBOX_ASSET_LOG lists every content file used or missing (checks a clean checkout)
        if std::env::var("XBOX_ASSET_LOG").is_ok() {
            eprintln!("asset-missing {rel}");
        }
        None
    }

    pub fn read_local(&self, uri: &str) -> Result<Vec<u8>> {
        let p = self.local(uri).ok_or_else(|| anyhow!("missing content {uri}"))?;
        Ok(std::fs::read(p)?)
    }

    fn asset_cache_path(&self, id: u64) -> PathBuf {
        self.cache.join("assets").join(format!("{id}.bin"))
    }

    pub fn cached_asset(&self, id: u64) -> Option<Vec<u8>> {
        std::fs::read(self.asset_cache_path(id)).ok()
    }

    /// Fetch a Roblox asset by id (disk cache first, then asset delivery).
    pub fn fetch_asset(&self, id: u64, session: Option<&Session>) -> Result<Vec<u8>> {
        // images shipped with the launcher (the VR lobby's decal) work before sign-in
        // (predownload_textures.py saves each place texture here under its real format)
        let shipped = self.root.join("launcher").join("assets").join("assetids");
        for ext in ["png", "jpg", "ktx", "dds"] {
            if let Ok(d) = std::fs::read(shipped.join(format!("{id}.{ext}"))) {
                return Ok(d);
            }
        }
        if let Some(d) = self.cached_asset(id) {
            return Ok(d);
        }
        let Some(session) = session else { bail!("asset {id} needs sign-in to download") };
        let mut data = session.asset_delivery(id)?;
        // Decal/face assets come back as a tiny model pointing at the real image (only decals:
        // hats and clothing are XML models too and must stay models).
        if data.starts_with(b"<roblox") && String::from_utf8_lossy(&data[..data.len().min(600)]).contains("class=\"Decal\"") {
            let text = String::from_utf8_lossy(&data);
            if let Some(inner) = find_inner_asset_id(&text) {
                if inner != id {
                    data = self.fetch_asset(inner, Some(session))?;
                }
            }
        }
        let _ = std::fs::write(self.asset_cache_path(id), &data);
        Ok(data)
    }

    /// A model asset (accessory, clothing, body part, animation) exactly as delivered.
    pub fn fetch_model(&self, id: u64, session: Option<&Session>) -> Result<Vec<u8>> {
        let p = self.cache.join("assets").join(format!("{id}.model"));
        if let Ok(d) = std::fs::read(&p) {
            return Ok(d);
        }
        let Some(session) = session else { bail!("asset {id} needs sign-in to download") };
        let data = session.asset_delivery(id)?;
        let _ = std::fs::write(&p, &data);
        Ok(data)
    }

    /// Cache for arbitrary web images (thumbnails) keyed by URL.
    pub fn fetch_url(&self, url: &str) -> Result<Vec<u8>> {
        let key = format!("{:016x}", fnv(url.as_bytes()));
        let p = self.cache.join("web").join(key);
        if let Ok(d) = std::fs::read(&p) {
            return Ok(d);
        }
        let d = crate::api::get_bytes_anon(url)?;
        let _ = std::fs::write(&p, &d);
        Ok(d)
    }

    /// Load any content reference: rbxasset://, asset ids, or https URLs.
    pub fn load(&self, uri: &str, session: Option<&Session>) -> Result<Vec<u8>> {
        if let Some(rel) = uri.strip_prefix("rbxclient://") {
            let base = self.client.as_ref().ok_or_else(|| anyhow!("no Roblox client installed"))?;
            return Ok(std::fs::read(base.join(rel))?);
        }
        if uri.starts_with("rbxasset://") {
            return self.read_local(uri);
        }
        if uri.starts_with("https://") && !uri.contains("roblox.com/asset") {
            return self.fetch_url(uri);
        }
        let id = parse_asset_id(uri).ok_or_else(|| anyhow!("unrecognised content id {uri}"))?;
        self.fetch_asset(id, session)
    }
}

pub fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn parse_asset_id(uri: &str) -> Option<u64> {
    let u = uri.trim();
    if let Some(r) = u.strip_prefix("rbxassetid://") {
        return r.trim().parse().ok();
    }
    let lower = u.to_ascii_lowercase();
    if let Some(i) = lower.find("id=") {
        let digits: String = u[i + 3..].chars().take_while(|c| c.is_ascii_digit()).collect();
        return digits.parse().ok();
    }
    if u.chars().all(|c| c.is_ascii_digit()) && !u.is_empty() {
        return u.parse().ok();
    }
    None
}

fn find_inner_asset_id(xml: &str) -> Option<u64> {
    for tag in ["<url>", "<Content name=\"Texture\"><url>"] {
        if let Some(i) = xml.find(tag) {
            let rest = &xml[i + tag.len()..];
            let end = rest.find("</url>")?;
            return parse_asset_id(&rest[..end]);
        }
    }
    None
}

pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

pub fn decode_image(bytes: &[u8]) -> Result<Rgba> {
    let img = image::load_from_memory(bytes)?.to_rgba8();
    Ok(Rgba { w: img.width(), h: img.height(), data: img.into_raw() })
}

/// Background loader: decodes images off the render thread.
pub struct ImageLoader {
    tx: Sender<(String, Option<Arc<Session>>)>,
    rx: Receiver<(String, Result<Rgba>)>,
    pending: HashMap<String, ()>,
}

impl ImageLoader {
    pub fn new(content: Content, threads: usize) -> Self {
        let (tx, jobs) = channel::<(String, Option<Arc<Session>>)>();
        let (done_tx, rx) = channel();
        let jobs = Arc::new(Mutex::new(jobs));
        for _ in 0..threads {
            let jobs = jobs.clone();
            let done = done_tx.clone();
            let content = content.clone();
            std::thread::spawn(move || loop {
                let job = { jobs.lock().unwrap().recv() };
                let Ok((key, session)) = job else { break };
                let res = content.load(&key, session.as_deref()).and_then(|b| decode_image(&b));
                if done.send((key, res)).is_err() {
                    break;
                }
            });
        }
        Self { tx, rx, pending: HashMap::new() }
    }

    pub fn request(&mut self, key: &str, session: Option<Arc<Session>>) {
        if self.pending.contains_key(key) {
            return;
        }
        self.pending.insert(key.to_string(), ());
        let _ = self.tx.send((key.to_string(), session));
    }

    pub fn poll(&mut self) -> Vec<(String, Result<Rgba>)> {
        let mut out = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            self.pending.remove(&r.0);
            out.push(r);
        }
        out
    }

    pub fn forget(&mut self, key: &str) {
        self.pending.remove(key);
    }
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}

// >> end << //
