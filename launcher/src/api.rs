/*
    written by: @corescripts
*/

//! Roblox web API client built around a .ROBLOSECURITY cookie.
//!
//! The cookie is only ever attached to requests for `*.roblox.com` over HTTPS.
//! Thumbnail/CDN downloads go through `get_bytes_anon` without it.
//!
//! Purchasing is intentionally not implemented: there are no economy write
//! calls anywhere in this client.

// >> imports
use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) XbloxRewind/1.0";

/// "https://www.<domain>/" and its origin, for Referer / Origin headers.
fn site_root() -> String {
    format!("https://www.{}/", crate::config::domain())
}

fn site_origin() -> String {
    format!("https://www.{}", crate::config::domain())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .redirects(0)
        .user_agent(UA)
        .build()
}

pub fn get_bytes_anon(url: &str) -> Result<Vec<u8>> {
    let url = &crate::config::web(url);
    if !url.starts_with("https://") {
        bail!("refusing non-https url");
    }
    let a = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).user_agent(UA).build();
    let resp = a.get(url).call()?;
    let mut v = Vec::new();
    resp.into_reader().take(64 << 20).read_to_end(&mut v)?;
    Ok(v)
}

// >> quick login
// Roblox's Quick Login: the launcher shows a code (and QR), the player approves it on a device
// that is already signed in, and the code is exchanged for a .ROBLOSECURITY cookie.

/// A Quick Login code and the private key that redeems it.
#[derive(Clone, Debug, Default)]
pub struct QuickCode {
    pub code: String,
    pub private_key: String,
    /// Roblox's QR image for the code
    pub qr_url: String,
}

/// POST without a cookie (Quick Login runs before sign-in), negotiating the CSRF token.
/// Returns the status, body and Set-Cookie headers.
fn anon_post(url: &str, body: &Value) -> Result<(u16, String, Vec<String>)> {
    let url = crate::config::web(url);
    let a = agent();
    let mut token = String::new();
    for _ in 0..2 {
        let mut req = a.post(&url).set("Content-Type", "application/json").set("Accept", "application/json").set("Origin", &site_origin()).set("Referer", &site_root());
        if !token.is_empty() {
            req = req.set("x-csrf-token", &token);
        }
        let r = match req.send_string(&body.to_string()) {
            Ok(r) => r,
            Err(ureq::Error::Status(403, r)) if r.header("x-csrf-token").is_some() && token.is_empty() => {
                token = r.header("x-csrf-token").unwrap_or_default().to_string();
                continue;
            }
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(e.into()),
        };
        let status = r.status();
        let cookies = r.all("set-cookie").iter().map(|c| c.to_string()).collect();
        return Ok((status, r.into_string().unwrap_or_default(), cookies));
    }
    bail!("csrf negotiation failed for {url}")
}

/// A new Quick Login code.
pub fn quick_login_create() -> Result<QuickCode> {
    let (st, body, _) = anon_post("https://apis.roblox.com/auth-token-service/v1/login/create", &json!({}))?;
    let v: Value = serde_json::from_str(&body).map_err(|_| anyhow!("Quick Login is unavailable (HTTP {st})"))?;
    let (Some(code), Some(key)) = (v["code"].as_str(), v["privateKey"].as_str()) else { bail!("Quick Login is unavailable (HTTP {st})") };
    let qr = v["imagePath"].as_str().map(|p| crate::config::web(&format!("https://apis.roblox.com/auth-token-service{p}"))).unwrap_or_default();
    Ok(QuickCode { code: code.to_string(), private_key: key.to_string(), qr_url: qr })
}

/// Where the code stands: Created, UserLinked, Validated or Cancelled (an expired or unknown
/// code counts as Cancelled), with the masked account name once a player has entered it.
pub fn quick_login_status(c: &QuickCode) -> Result<(String, Option<String>)> {
    let (st, body, _) = anon_post("https://apis.roblox.com/auth-token-service/v1/login/status", &json!({ "code": c.code, "privateKey": c.private_key }))?;
    if st == 400 || body.contains("CodeInvalid") {
        return Ok(("Cancelled".into(), None));
    }
    let v: Value = serde_json::from_str(&body)?;
    Ok((v["status"].as_str().unwrap_or("Created").to_string(), v["accountName"].as_str().map(|s| s.to_string())))
}

/// Throw a code away (leaving the screen).
pub fn quick_login_cancel(code: &str) {
    let _ = anon_post("https://apis.roblox.com/auth-token-service/v1/login/cancel", &json!({ "code": code }));
}

/// Redeem an approved code: the new .ROBLOSECURITY and who it signs in.
pub fn quick_login_finish(c: &QuickCode) -> Result<(String, User)> {
    let (st, body, cookies) = anon_post("https://auth.roblox.com/v2/login", &json!({ "ctype": "AuthToken", "cvalue": c.code, "password": c.private_key }))?;
    let cookie = cookies
        .iter()
        .find_map(|c| c.strip_prefix(".ROBLOSECURITY=").map(|v| v.split(';').next().unwrap_or("").to_string()))
        .filter(|v| !v.is_empty());
    let Some(cookie) = cookie else {
        let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["errors"][0]["message"].as_str().map(|s| s.to_string())).unwrap_or_else(|| format!("HTTP {st}"));
        bail!("Roblox did not finish the sign-in: {msg}")
    };
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let user = serde_json::from_value(v["user"].clone()).unwrap_or_default();
    Ok((cookie, user))
}

/// A user's headshot URL (public, no cookie).
pub fn headshot_anon(uid: u64) -> Option<String> {
    let b = get_bytes_anon(&format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={uid}&size=150x150&format=Png&isCircular=false")).ok()?;
    let v: Value = serde_json::from_slice(&b).ok()?;
    v["data"][0]["imageUrl"].as_str().map(|s| s.to_string())
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct GameInfo {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub player_count: i64,
    pub up_votes: i64,
    pub down_votes: i64,
}

#[derive(Clone, Debug, Default)]
pub struct GameDetails {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub description: String,
    pub creator: String,
    pub playing: i64,
    pub visits: i64,
    pub max_players: i64,
    pub created: String,
    pub updated: String,
    pub favorites: i64,
    pub up_votes: i64,
    pub down_votes: i64,
    pub is_favorited: bool,
    pub screenshots: Vec<String>,
    /// Configure > Age Ratings: experience guidelines age recommendation ("Ages 9+")
    pub age_rating: Option<String>,
    pub can_vote: bool,
    pub user_vote: Option<bool>,
}

/// An avatar tile: one of the user's outfits, or a catalog bundle.
#[derive(Clone, Debug, Default)]
pub struct AvatarItem {
    pub id: u64,
    pub name: String,
    pub thumb: String,
    pub owned: bool,
    pub is_bundle: bool,
    pub price: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Sort {
    pub id: String,
    pub title: String,
    pub games: Vec<GameInfo>,
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub id: u64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub created: String,
    pub friends: i64,
    pub followers: i64,
    pub following: i64,
    pub avatar: String,
    pub favorites: Vec<GameInfo>,
    pub creations: Vec<GameInfo>,
    /// (name, thumbnail)
    pub wearing: Vec<(String, String)>,
    pub badges: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct Friend {
    pub id: u64,
    pub name: String,
    pub display_name: String,
    /// 0 offline, 1 online, 2 in game, 3 in studio
    pub presence: u8,
    pub location: String,
    pub place_id: Option<u64>,
    pub game_id: Option<String>,
}

pub struct Session {
    cookie: String,
    csrf: Mutex<String>,
    agent: ureq::Agent,
    pub session_id: String,
}

impl Session {
    pub fn new(cookie: &str) -> Self {
        let cookie = cookie.trim().trim_matches('"').to_string();
        let cookie = cookie.strip_prefix(".ROBLOSECURITY=").unwrap_or(&cookie).to_string();
        Self { cookie, csrf: Mutex::new(String::new()), agent: agent(), session_id: uuid::Uuid::new_v4().to_string() }
    }

    pub fn cookie(&self) -> &str {
        &self.cookie
    }

    /// The cookie only ever goes to the configured site (roblox.com unless config.ini says
    /// otherwise), over HTTPS.
    fn check_host(url: &str) -> Result<()> {
        let d = crate::config::domain();
        let host = url.strip_prefix("https://").and_then(|r| r.split('/').next()).unwrap_or("");
        if host == d || host.ends_with(&format!(".{d}")) {
            Ok(())
        } else {
            bail!("cookie requests are limited to {d} (got {host})")
        }
    }

    fn request(&self, method: &str, url: &str, body: Option<&Value>) -> Result<ureq::Response> {
        let url = &crate::config::web(url);
        Self::check_host(url)?;
        for _ in 0..2 {
            let mut req = self
                .agent
                .request(method, url)
                .set("Cookie", &format!(".ROBLOSECURITY={}", self.cookie))
                .set("Accept", "application/json")
                .set("Referer", &site_root())
                .set("Origin", &site_origin());
            let token = self.csrf.lock().unwrap().clone();
            if !token.is_empty() {
                req = req.set("x-csrf-token", &token);
            }
            let res = match body {
                Some(b) => req.send_json(b.clone()),
                None if method == "POST" => req.set("Content-Type", "application/json").send_string("{}"),
                None => req.call(),
            };
            match res {
                Ok(r) => return Ok(r),
                Err(ureq::Error::Status(403, r)) => {
                    if let Some(t) = r.header("x-csrf-token") {
                        *self.csrf.lock().unwrap() = t.to_string();
                        continue;
                    }
                    bail!("403 from {url}");
                }
                Err(ureq::Error::Status(code, r)) => {
                    let text = r.into_string().unwrap_or_default();
                    bail!("HTTP {code} from {}: {}", url.split('?').next().unwrap_or(url), text.chars().take(200).collect::<String>());
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!("csrf negotiation failed for {url}")
    }

    fn get(&self, url: &str) -> Result<Value> {
        Ok(self.request("GET", url, None)?.into_json()?)
    }

    fn post(&self, url: &str, body: &Value) -> Result<Value> {
        Ok(self.request("POST", url, Some(body))?.into_json()?)
    }

    pub fn authenticated(&self) -> Result<User> {
        let v = self.get("https://users.roblox.com/v1/users/authenticated")?;
        Ok(serde_json::from_value(v)?)
    }

    /// Real Robux balance (read-only).
    pub fn robux(&self, uid: u64) -> Result<i64> {
        let v = self.get(&format!("https://economy.roblox.com/v1/users/{uid}/currency"))?;
        v["robux"].as_i64().ok_or_else(|| anyhow!("no robux field"))
    }

    pub fn asset_delivery(&self, id: u64) -> Result<Vec<u8>> {
        // v2 returns signed CDN locations; the CDN fetch itself is made without the cookie.
        let v = self.get(&format!("https://assetdelivery.roblox.com/v2/assetId/{id}"))?;
        let loc = v["locations"][0]["location"]
            .as_str()
            .ok_or_else(|| anyhow!("asset {id}: {}", v["errors"][0]["message"].as_str().unwrap_or("no location")))?;
        get_bytes_anon(loc)
    }

    /// A mesh asset in an older representation (v4/v5) the parser understands; newer uploads are
    /// otherwise served as compressed v7 COREMESH.
    pub fn asset_delivery_mesh(&self, id: u64, major: &str) -> Result<Vec<u8>> {
        let list = format!(r#"[{{"format":"mesh","majorVersion":"{major}","fidelity":""}}]"#);
        let enc = base64url(list.as_bytes());
        let v = self.get(&format!("https://assetdelivery.roblox.com/v2/assetId/{id}?contentRepresentationPriorityList={enc}"))?;
        let loc = v["locations"][0]["location"].as_str().ok_or_else(|| anyhow!("asset {id}: {}", v["errors"][0]["message"].as_str().unwrap_or("no location")))?;
        get_bytes_anon(loc)
    }

    /// assetdelivery v1 (the uploaded representation). Redirects are not followed with the
    /// cookie: the CDN location is read and fetched anonymously.
    pub fn asset_delivery_v1(&self, id: u64) -> Result<Vec<u8>> {
        let url = crate::config::web(&format!("https://assetdelivery.roblox.com/v1/asset/?id={id}"));
        Self::check_host(&url)?;
        let agent = ureq::AgentBuilder::new().redirects(0).build();
        let res = agent.get(&url).set("Cookie", &format!(".ROBLOSECURITY={}", self.cookie)).set("Referer", &site_root()).call();
        let r = match res {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(e.into()),
        };
        if let Some(loc) = r.header("location").map(|l| l.to_string()) {
            return get_bytes_anon(&loc);
        }
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut r.into_reader(), &mut out)?;
        Ok(out)
    }

    pub fn auth_ticket(&self) -> Result<String> {
        let ticket_url = crate::config::web("https://auth.roblox.com/v1/authentication-ticket");
        Self::check_host(&ticket_url)?;
        for _ in 0..2 {
            let token = self.csrf.lock().unwrap().clone();
            let res = self
                .agent
                .post(&ticket_url)
                .set("Cookie", &format!(".ROBLOSECURITY={}", self.cookie))
                .set("Referer", &site_root())
                .set("Origin", &site_origin())
                .set("RBXAuthenticationNegotiation", "1")
                .set("Content-Type", "application/json")
                .set("x-csrf-token", &token)
                .send_string("{}");
            match res {
                Ok(r) => {
                    return r
                        .header("rbx-authentication-ticket")
                        .map(|s| s.to_string())
                        .ok_or_else(|| anyhow!("no authentication ticket returned"))
                }
                Err(ureq::Error::Status(403, r)) => {
                    if let Some(t) = r.header("x-csrf-token") {
                        *self.csrf.lock().unwrap() = t.to_string();
                        continue;
                    }
                    bail!("ticket request forbidden");
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!("could not get authentication ticket")
    }

    /// One user's presence: (type 0-3, place id, server id, location).
    pub fn presence_of(&self, uid: u64) -> Result<(u8, Option<u64>, Option<String>, String)> {
        let v = self.post("https://presence.roblox.com/v1/presence/users", &json!({ "userIds": [uid] }))?;
        let p = &v["userPresences"][0];
        Ok((
            p["userPresenceType"].as_u64().unwrap_or(0) as u8,
            p["placeId"].as_u64().or(p["rootPlaceId"].as_u64()),
            p["gameId"].as_str().map(|s| s.to_string()),
            p["lastLocation"].as_str().unwrap_or("").to_string(),
        ))
    }

    pub fn avatar_url(&self, uid: u64, kind: &str, size: &str) -> Result<String> {
        let v = self.get(&format!(
            "https://thumbnails.roblox.com/v1/users/{kind}?userIds={uid}&size={size}&format=Png&isCircular=false"
        ))?;
        thumb_url(&v, 0)
    }

    pub fn headshots(&self, ids: &[u64]) -> Result<Vec<(u64, String)>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(100) {
            let list = join(chunk);
            let v = self.get(&format!(
                "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={list}&size=150x150&format=Png&isCircular=false"
            ))?;
            collect_thumbs(&v, &mut out);
        }
        Ok(out)
    }

    pub fn game_icons(&self, universe_ids: &[u64]) -> Result<Vec<(u64, String)>> {
        let mut out = Vec::new();
        for chunk in universe_ids.chunks(100) {
            let v = self.get(&format!(
                "https://thumbnails.roblox.com/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=512x512&format=Png&isCircular=false",
                join(chunk)
            ))?;
            collect_thumbs(&v, &mut out);
        }
        Ok(out)
    }

    pub fn sorts(&self) -> Result<Vec<Sort>> {
        let v = self.get(&format!(
            "https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={}&device=computer&country=all&cpuCores=8&maxResolution=1920x1080&maxMemory=8192&networkType=Wifi",
            self.session_id
        ))?;
        let mut out = Vec::new();
        for s in v["sorts"].as_array().cloned().unwrap_or_default() {
            let Some(games) = s["games"].as_array() else { continue };
            if games.is_empty() {
                continue;
            }
            let title = s["sortDisplayName"].as_str().or(s["topic"].as_str()).unwrap_or("Games").to_string();
            out.push(Sort {
                id: s["sortId"].as_str().unwrap_or("").to_string(),
                title,
                games: games.iter().map(game_from_json).collect(),
            });
        }
        Ok(out)
    }

    /// The home page's "Continue" sort (recently played) from discovery-api omni-recommendation.
    pub fn recently_played(&self) -> Result<Vec<GameInfo>> {
        let v = self.post("https://apis.roblox.com/discovery-api/omni-recommendation", &json!({ "pageType": "Home", "sessionId": self.session_id }))?;
        let meta = &v["contentMetadata"]["Game"];
        for s in v["sorts"].as_array().cloned().unwrap_or_default() {
            let topic = s["topic"].as_str().unwrap_or("").to_lowercase();
            let tid = s["topicId"].as_u64().unwrap_or(0);
            if !(topic.contains("continue") || topic.contains("recent") || tid == 100000003) {
                continue;
            }
            let games = s["recommendationList"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter(|r| r["contentType"].as_str() == Some("Game"))
                .filter_map(|r| r["contentId"].as_u64())
                .map(|u| {
                    let g = &meta[u.to_string()];
                    GameInfo { universe_id: u, ..game_from_json(g) }
                })
                .collect();
            return Ok(games);
        }
        if std::env::var("XBOX_APITEST").is_ok() {
            let topics: Vec<String> = v["sorts"].as_array().cloned().unwrap_or_default().iter().map(|s| format!("{}:{}", s["topicId"], s["topic"])).collect();
            eprintln!("omni topics: {topics:?}");
        }
        Ok(Vec::new())
    }

    pub fn favorites(&self, uid: u64) -> Result<Vec<GameInfo>> {
        let v = self.get(&format!("https://games.roblox.com/v2/users/{uid}/favorite/games?limit=50&sortOrder=Desc"))?;
        Ok(v["data"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|g| GameInfo {
                universe_id: g["id"].as_u64().unwrap_or(0),
                place_id: g["rootPlace"]["id"].as_u64().unwrap_or(0),
                name: g["name"].as_str().unwrap_or("").to_string(),
                ..Default::default()
            })
            .collect())
    }

    pub fn search(&self, query: &str) -> Result<Vec<GameInfo>> {
        let v = self.get(&format!(
            "https://apis.roblox.com/search-api/omni-search?searchQuery={}&sessionId={}&pageType=all",
            urlencoding::encode(query),
            self.session_id
        ))?;
        let mut out = Vec::new();
        for group in v["searchResults"].as_array().cloned().unwrap_or_default() {
            if group["contentGroupType"].as_str() != Some("Game") {
                continue;
            }
            for g in group["contents"].as_array().cloned().unwrap_or_default() {
                out.push(game_from_json(&g));
            }
        }
        Ok(out)
    }

    pub fn place_to_universe(&self, place_id: u64) -> Result<u64> {
        let v = self.get(&format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe"))?;
        v["universeId"].as_u64().ok_or_else(|| anyhow!("unknown place"))
    }

    pub fn game_details(&self, universe_id: u64) -> Result<GameDetails> {
        let v = self.get(&format!("https://games.roblox.com/v1/games?universeIds={universe_id}"))?;
        let g = &v["data"][0];
        let mut d = GameDetails {
            universe_id,
            place_id: g["rootPlaceId"].as_u64().unwrap_or(0),
            name: g["name"].as_str().unwrap_or("").to_string(),
            description: g["description"].as_str().unwrap_or("").to_string(),
            creator: g["creator"]["name"].as_str().unwrap_or("").to_string(),
            playing: g["playing"].as_i64().unwrap_or(0),
            visits: g["visits"].as_i64().unwrap_or(0),
            max_players: g["maxPlayers"].as_i64().unwrap_or(0),
            created: g["created"].as_str().unwrap_or("").chars().take(10).collect(),
            updated: g["updated"].as_str().unwrap_or("").chars().take(10).collect(),
            favorites: g["favoritedCount"].as_i64().unwrap_or(0),
            ..Default::default()
        };
        if let Ok(v) = self.get(&format!("https://games.roblox.com/v1/games/votes?universeIds={universe_id}")) {
            d.up_votes = v["data"][0]["upVotes"].as_i64().unwrap_or(0);
            d.down_votes = v["data"][0]["downVotes"].as_i64().unwrap_or(0);
        }
        if let Ok(v) = self.get(&format!("https://games.roblox.com/v1/games/{universe_id}/votes/user")) {
            d.can_vote = v["canVote"].as_bool().unwrap_or(false);
            d.user_vote = v["userVote"].as_bool();
        }
        if let Ok(v) = self.get(&format!("https://games.roblox.com/v1/games/{universe_id}/favorites")) {
            d.is_favorited = v["isFavorited"].as_bool().unwrap_or(false);
        }
        if let Ok(v) = self.get(&format!(
            "https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={universe_id}&countPerUniverse=10&defaults=true&size=768x432&format=Png&isCircular=false"
        )) {
            for t in v["data"][0]["thumbnails"].as_array().cloned().unwrap_or_default() {
                if let Some(u) = t["imageUrl"].as_str() {
                    d.screenshots.push(u.to_string());
                }
            }
        }
        Ok(d)
    }

    /// Owned assets of one type (newest first): (asset id, name).
    pub fn inventory(&self, uid: u64, asset_type: u32) -> Result<Vec<(u64, String)>> {
        let v = self.get(&format!("https://inventory.roblox.com/v2/users/{uid}/inventory/{asset_type}?limit=100&sortOrder=Desc"))?;
        Ok(v["data"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|a| Some((a["assetId"].as_u64()?, a["assetName"].as_str().unwrap_or("").to_string())))
            .collect())
    }

    /// Replace what the avatar wears (avatar editor).
    /// `metas` keeps each accessory's saved adjustment (position / rotation / scale, layered
    /// clothing order), which Roblox drops when an asset is sent without it.
    pub fn set_wearing(&self, ids: &[u64], metas: &std::collections::HashMap<u64, Value>) -> Result<()> {
        // v1 set-wearing-assets is gone (404); v2 takes asset objects
        let assets: Vec<Value> = ids
            .iter()
            .map(|id| match metas.get(id) {
                Some(m) => json!({ "id": id, "meta": m }),
                None => json!({ "id": id }),
            })
            .collect();
        let r = self.request("POST", "https://avatar.roblox.com/v2/avatar/set-wearing-assets", Some(&json!({ "assets": assets })))?;
        let body: Value = r.into_json().unwrap_or(Value::Null);
        if body["success"].as_bool() == Some(false) {
            bail!("set-wearing-assets refused: {}", body);
        }
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    pub fn set_avatar_type(&self, r15: bool) -> Result<()> {
        self.request("POST", "https://avatar.roblox.com/v1/avatar/set-player-avatar-type", Some(&json!({ "playerAvatarType": if r15 { "R15" } else { "R6" } })))?;
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    /// One BrickColor for every body part (the console editor's skin tone swatches).
    pub fn set_skin(&self, brick: u32) -> Result<()> {
        let b = json!({ "headColorId": brick, "torsoColorId": brick, "rightArmColorId": brick, "leftArmColorId": brick, "rightLegColorId": brick, "leftLegColorId": brick });
        self.request("POST", "https://avatar.roblox.com/v1/avatar/set-body-colors", Some(&b))?;
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    /// One colour (hex "RRGGBB") for every body part, through avatar/v2 (the newer editor's swatches).
    pub fn set_skin_hex(&self, hex: &str) -> Result<()> {
        let b = json!({ "headColor3": hex, "torsoColor3": hex, "rightArmColor3": hex, "leftArmColor3": hex, "rightLegColor3": hex, "leftLegColor3": hex });
        self.request("POST", "https://avatar.roblox.com/v2/avatar/set-body-colors", Some(&b))?;
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    /// R15 body scales (avatar editor Scale sliders).
    pub fn set_scales(&self, height: f32, width: f32, head: f32, body_type: f32, proportion: f32) -> Result<()> {
        let b = json!({ "height": height, "width": width, "depth": width, "head": head, "bodyType": body_type, "proportion": proportion });
        self.request("POST", "https://avatar.roblox.com/v1/avatar/set-scales", Some(&b))?;
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    /// Equipped emotes: (slot 1-8, asset id, name).
    pub fn emotes(&self) -> Result<Vec<(u8, u64, String)>> {
        let v = self.get("https://avatar.roblox.com/v1/emotes")?;
        Ok(v.as_array()
            .map(|a| a.iter().filter_map(|e| Some((e["position"].as_u64()? as u8, e["assetId"].as_u64()?, e["assetName"].as_str().unwrap_or("").to_string()))).collect())
            .unwrap_or_default())
    }

    /// Put an owned emote in wheel slot 1-8.
    pub fn set_emote(&self, asset: u64, slot: u8) -> Result<()> {
        self.request("POST", &format!("https://avatar.roblox.com/v1/emotes/{asset}/{slot}"), None)?;
        Ok(())
    }

    /// v3 outfit details: assets, rig type, scale and body colours.
    pub fn outfit_details(&self, outfit: u64) -> Result<Value> {
        self.get(&format!("https://avatar.roblox.com/v3/outfits/{outfit}/details"))
    }

    /// avatar/v2: rig type, scales, body colours and every worn asset.
    pub fn avatar_info(&self, uid: u64) -> Result<Value> {
        self.get(&format!("https://avatar.roblox.com/v2/avatar/users/{uid}/avatar"))
    }

    /// Experience guidelines age recommendation for a universe, e.g. "Ages 9+" or "Maturity: Minimal".
    pub fn age_rating(&self, universe_id: u64) -> Option<String> {
        let v = self
            .post(
                "https://apis.roblox.com/experience-guidelines-api/experience-guidelines/get-age-recommendation",
                &serde_json::json!({ "universeId": universe_id.to_string() }),
            )
            .ok()?;
        let r = &v["ageRecommendationDetails"]["summary"]["ageRecommendation"];
        r["displayName"].as_str().or_else(|| r["title"].as_str()).map(|s| s.to_string()).filter(|s| !s.is_empty())
    }

    /// Like (true), dislike (false) or clear (None) the user's vote.
    pub fn set_vote(&self, universe_id: u64, vote: Option<bool>) -> Result<()> {
        self.request("PATCH", &format!("https://games.roblox.com/v1/games/{universe_id}/user-votes"), Some(&json!({ "vote": vote })))?;
        Ok(())
    }

    /// The user's saved outfits plus a page of catalog bundles (owned ones flagged).
    pub fn avatar_items(&self, uid: u64) -> Result<(Vec<AvatarItem>, Vec<AvatarItem>)> {
        let mut outfits = Vec::new();
        let v = self
            .get(&format!("https://avatar.roblox.com/v2/avatar/users/{uid}/outfits?page=1&itemsPerPage=50&isEditable=true"))
            .or_else(|_| self.get(&format!("https://avatar.roblox.com/v1/users/{uid}/outfits?page=1&itemsPerPage=50&isEditable=true")))?;
        for o in v["data"].as_array().cloned().unwrap_or_default() {
            if let Some(id) = o["id"].as_u64() {
                outfits.push(AvatarItem { id, name: o["name"].as_str().unwrap_or("").to_string(), owned: true, ..Default::default() });
            }
        }
        let ids: Vec<u64> = outfits.iter().map(|o| o.id).collect();
        for chunk in ids.chunks(100) {
            if let Ok(t) = self.get(&format!("https://thumbnails.roblox.com/v1/users/outfits?userOutfitIds={}&size=420x420&format=Png&isCircular=false", join(chunk))) {
                let mut thumbs = Vec::new();
                collect_thumbs(&t, &mut thumbs);
                for (id, url) in thumbs {
                    if let Some(o) = outfits.iter_mut().find(|o| o.id == id) {
                        o.thumb = url;
                    }
                }
            }
        }
        // catalog: avatar bundles
        let mut catalog = Vec::new();
        if let Ok(v) = self.get("https://catalog.roblox.com/v1/search/items?category=Characters&limit=30&sortType=0") {
            let ids: Vec<u64> = v["data"].as_array().cloned().unwrap_or_default().iter().filter(|i| i["itemType"].as_str() == Some("Bundle")).filter_map(|i| i["id"].as_u64()).collect();
            if !ids.is_empty() {
                let items: Vec<Value> = ids.iter().map(|id| json!({ "itemType": "Bundle", "id": id })).collect();
                if let Ok(d) = self.post("https://catalog.roblox.com/v1/catalog/items/details", &json!({ "items": items })) {
                    for b in d["data"].as_array().cloned().unwrap_or_default() {
                        catalog.push(AvatarItem {
                            id: b["id"].as_u64().unwrap_or(0),
                            name: b["name"].as_str().unwrap_or("").to_string(),
                            is_bundle: true,
                            owned: b["owned"].as_bool().unwrap_or(false),
                            price: b["price"].as_i64().or(b["lowestPrice"].as_i64()),
                            ..Default::default()
                        });
                    }
                }
                let cids: Vec<u64> = catalog.iter().map(|c| c.id).collect();
                if let Ok(t) = self.get(&format!("https://thumbnails.roblox.com/v1/bundles/thumbnails?bundleIds={}&size=420x420&format=Png&isCircular=false", join(&cids))) {
                    let mut thumbs = Vec::new();
                    collect_thumbs(&t, &mut thumbs);
                    for (id, url) in thumbs {
                        if let Some(c) = catalog.iter_mut().find(|c| c.id == id) {
                            c.thumb = url;
                        }
                    }
                }
            }
        }
        // bundles the user already owns move into "my outfits"
        let (owned, rest): (Vec<_>, Vec<_>) = catalog.into_iter().partition(|c| c.owned);
        outfits.extend(owned);
        Ok((outfits, rest))
    }

    /// Equip an outfit (or the outfit inside an owned bundle).
    pub fn wear(&self, item: &AvatarItem) -> Result<()> {
        let outfit = if item.is_bundle {
            let d = self.get(&format!("https://catalog.roblox.com/v1/bundles/{}/details", item.id))?;
            d["items"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .find(|i| i["type"].as_str() == Some("UserOutfit"))
                .and_then(|i| i["id"].as_u64())
                .ok_or_else(|| anyhow!("bundle has no outfit"))?
        } else {
            item.id
        };
        // outfits/{id}/wear was retired; apply the outfit's assets, colors, scales and rig instead
        let d = self
            .get(&format!("https://avatar.roblox.com/v3/outfits/{outfit}/details"))
            .or_else(|_| self.get(&format!("https://avatar.roblox.com/v1/outfits/{outfit}/details")))?;
        let assets: Vec<Value> = d["assets"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|a| {
                let id = a["id"].as_u64()?;
                let mut o = json!({ "id": id });
                if !a["meta"].is_null() {
                    o["meta"] = a["meta"].clone();
                }
                Some(o)
            })
            .collect();
        if let Some(t) = d["playerAvatarType"].as_str() {
            let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/set-player-avatar-type", Some(&json!({ "playerAvatarType": t })));
        }
        if d["scale"].is_object() {
            let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/set-scales", Some(&d["scale"]));
        }
        if d["bodyColor3s"].is_object() {
            let _ = self.request("POST", "https://avatar.roblox.com/v2/avatar/set-body-colors", Some(&d["bodyColor3s"]));
        } else if d["bodyColors"].is_object() {
            let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/set-body-colors", Some(&d["bodyColors"]));
        }
        let r = self.request("POST", "https://avatar.roblox.com/v2/avatar/set-wearing-assets", Some(&json!({ "assets": assets })))?;
        let body: Value = r.into_json().unwrap_or(Value::Null);
        if body["success"].as_bool() == Some(false) {
            bail!("set-wearing-assets refused: {}", body);
        }
        let _ = self.request("POST", "https://avatar.roblox.com/v1/avatar/redraw-thumbnail", None);
        Ok(())
    }

    /// Poll until the avatar render differs from `old` (or give up).
    pub fn wait_new_avatar(&self, uid: u64, old: &str) -> Option<(String, String)> {
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(1200));
            if let Ok(a) = self.avatar_url(uid, "avatar", "352x352") {
                if a != old {
                    let f = self.avatar_url(uid, "avatar", "720x720").unwrap_or_default();
                    return Some((a, f));
                }
            }
        }
        None
    }

    pub fn set_favorite(&self, universe_id: u64, fav: bool) -> Result<()> {
        self.post(&format!("https://games.roblox.com/v1/games/{universe_id}/favorites"), &json!({ "isFavorited": fav }))?;
        Ok(())
    }

    /// id -> (username, display name), retrying through every lookup Roblox offers.
    pub fn user_names(&self, ids: &[u64]) -> std::collections::HashMap<u64, (String, String)> {
        let mut out = std::collections::HashMap::new();
        for chunk in ids.chunks(100) {
            for attempt in 0..3 {
                match self.post("https://users.roblox.com/v1/users", &json!({ "userIds": chunk, "excludeBannedUsers": false })) {
                    Ok(v) => {
                        for u in v["data"].as_array().cloned().unwrap_or_default() {
                            if let Some(id) = u["id"].as_u64() {
                                out.insert(id, (u["name"].as_str().unwrap_or("").to_string(), u["displayName"].as_str().unwrap_or("").to_string()));
                            }
                        }
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(600 * (attempt + 1))),
                }
            }
        }
        let missing: Vec<u64> = ids.iter().copied().filter(|i| out.get(i).map(|n| n.0.is_empty()).unwrap_or(true)).collect();
        for chunk in missing.chunks(100) {
            let body = json!({ "userIds": chunk, "fields": ["names.username", "names.displayName"] });
            if let Ok(v) = self.post("https://apis.roblox.com/user-profile-api/v1/user/profiles/get-profiles", &body) {
                for u in v["profileDetails"].as_array().cloned().unwrap_or_default() {
                    if let Some(id) = u["userId"].as_u64() {
                        let n = u["names"]["username"].as_str().unwrap_or("").to_string();
                        let d = u["names"]["displayName"].as_str().unwrap_or("").to_string();
                        if !n.is_empty() {
                            out.insert(id, (n, d));
                        }
                    }
                }
            }
        }
        for id in ids {
            if out.get(id).map(|n| n.0.is_empty()).unwrap_or(true) {
                if let Ok(u) = self.get(&format!("https://users.roblox.com/v1/users/{id}")) {
                    out.insert(*id, (u["name"].as_str().unwrap_or("").to_string(), u["displayName"].as_str().unwrap_or("").to_string()));
                }
            }
        }
        out
    }

    /// Everything the gamer card shows for one user.
    pub fn profile(&self, uid: u64) -> Result<Profile> {
        let u = self.get(&format!("https://users.roblox.com/v1/users/{uid}"))?;
        let count = |url: String| self.get(&url).ok().and_then(|v| v["count"].as_i64()).unwrap_or(0);
        let mut p = Profile {
            id: uid,
            name: u["name"].as_str().unwrap_or("").to_string(),
            display_name: u["displayName"].as_str().unwrap_or("").to_string(),
            description: u["description"].as_str().unwrap_or("").to_string(),
            created: u["created"].as_str().unwrap_or("").chars().take(10).collect(),
            friends: count(format!("https://friends.roblox.com/v1/users/{uid}/friends/count")),
            followers: count(format!("https://friends.roblox.com/v1/users/{uid}/followers/count")),
            following: count(format!("https://friends.roblox.com/v1/users/{uid}/followings/count")),
            avatar: self.avatar_url(uid, "avatar", "720x720").unwrap_or_default(),
            ..Default::default()
        };
        let to_games = |v: &Value| -> Vec<GameInfo> {
            v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|g| GameInfo {
                    universe_id: g["id"].as_u64().unwrap_or(0),
                    place_id: g["rootPlace"]["id"].as_u64().unwrap_or(0),
                    name: g["name"].as_str().unwrap_or("").to_string(),
                    ..Default::default()
                })
                .collect()
        };
        if let Ok(v) = self.get(&format!("https://games.roblox.com/v2/users/{uid}/favorite/games?limit=50&sortOrder=Desc")) {
            p.favorites = to_games(&v);
        }
        if let Ok(v) = self.get(&format!("https://games.roblox.com/v2/users/{uid}/games?accessFilter=Public&limit=10&sortOrder=Desc")) {
            p.creations = to_games(&v);
        }
        if let Ok(ids) = self.currently_wearing(uid) {
            let names: std::collections::HashMap<u64, String> = self.asset_names(&ids).into_iter().collect();
            let thumbs: std::collections::HashMap<u64, String> = self.asset_thumbs(&ids).unwrap_or_default().into_iter().collect();
            p.wearing = ids.iter().map(|i| (names.get(i).cloned().unwrap_or_default(), thumbs.get(i).cloned().unwrap_or_default())).collect();
        }
        if let Ok(v) = self.get(&format!("https://badges.roblox.com/v1/users/{uid}/badges?limit=10&sortOrder=Desc")) {
            let badges: Vec<(u64, String)> = v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|b| Some((b["id"].as_u64()?, b["displayName"].as_str().or(b["name"].as_str()).unwrap_or("").to_string())))
                .collect();
            let ids: Vec<u64> = badges.iter().map(|b| b.0).collect();
            let mut thumbs = Vec::new();
            if !ids.is_empty() {
                if let Ok(t) = self.get(&format!("https://thumbnails.roblox.com/v1/badges/icons?badgeIds={}&size=150x150&format=Png&isCircular=false", join(&ids))) {
                    collect_thumbs(&t, &mut thumbs);
                }
            }
            p.badges = badges.into_iter().map(|(id, n)| (n, thumbs.iter().find(|t| t.0 == id).map(|t| t.1.clone()).unwrap_or_default())).collect();
        }
        Ok(p)
    }

    pub fn friends(&self, uid: u64) -> Result<Vec<Friend>> {
        let v = self.get(&format!("https://friends.roblox.com/v1/users/{uid}/friends"))?;
        let ids: Vec<u64> = v["data"].as_array().cloned().unwrap_or_default().iter().filter_map(|f| f["id"].as_u64()).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut friends: Vec<Friend> = ids.iter().map(|&id| Friend { id, ..Default::default() }).collect();
        // names (the friends endpoint no longer fills them in)
        let names = self.user_names(&ids);
        for f in friends.iter_mut() {
            if let Some((n, d)) = names.get(&f.id) {
                f.name = n.clone();
                f.display_name = d.clone();
            }
        }
        for chunk in ids.chunks(100) {
            if let Ok(v) = self.post("https://presence.roblox.com/v1/presence/users", &json!({ "userIds": chunk })) {
                for p in v["userPresences"].as_array().cloned().unwrap_or_default() {
                    if let Some(f) = friends.iter_mut().find(|f| Some(f.id) == p["userId"].as_u64()) {
                        f.presence = p["userPresenceType"].as_u64().unwrap_or(0) as u8;
                        f.location = p["lastLocation"].as_str().unwrap_or("").to_string();
                        f.place_id = p["placeId"].as_u64().or(p["rootPlaceId"].as_u64());
                        f.game_id = p["gameId"].as_str().map(|s| s.to_string());
                    }
                }
            }
        }
        friends.sort_by(|a, b| b.presence.cmp(&a.presence).then(a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())));
        Ok(friends)
    }

    /// Asset ids the user is currently wearing (read-only; shown on the Avatar tab).
    pub fn currently_wearing(&self, uid: u64) -> Result<Vec<u64>> {
        let v = self.get(&format!("https://avatar.roblox.com/v1/users/{uid}/currently-wearing"))?;
        Ok(v["assetIds"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_u64()).collect())
    }

    /// The account's badges, newest first, with rarity, award date and icon. `known` badges are
    /// not looked up again: paging stops at the first page made only of them.
    pub fn user_badges(&self, uid: u64, known: &std::collections::HashSet<u64>) -> Result<Vec<crate::achievements::BadgeInfo>> {
        let mut found: Vec<Value> = Vec::new();
        let mut cursor = String::new();
        for _ in 0..50 {
            let v = self.get(&format!("https://badges.roblox.com/v1/users/{uid}/badges?limit=100&sortOrder=Desc&cursor={cursor}"))?;
            let page = v["data"].as_array().cloned().unwrap_or_default();
            let fresh: Vec<Value> = page.iter().filter(|b| b["id"].as_u64().is_some_and(|i| !known.contains(&i))).cloned().collect();
            let done = fresh.is_empty() && !known.is_empty();
            found.extend(fresh);
            match v["nextPageCursor"].as_str() {
                Some(c) if !c.is_empty() && !done => cursor = c.to_string(),
                _ => break,
            }
        }
        let ids: Vec<u64> = found.iter().filter_map(|b| b["id"].as_u64()).collect();
        let mut dates: HashMap<u64, u64> = HashMap::new();
        let mut icons: HashMap<u64, String> = HashMap::new();
        for chunk in ids.chunks(100) {
            if let Ok(v) = self.get(&format!("https://badges.roblox.com/v1/users/{uid}/badges/awarded-dates?badgeIds={}", join(chunk))) {
                for d in v["data"].as_array().cloned().unwrap_or_default() {
                    if let (Some(id), Some(t)) = (d["badgeId"].as_u64(), d["awardedDate"].as_str()) {
                        dates.insert(id, iso_seconds(t));
                    }
                }
            }
            if let Ok(v) = self.get(&format!("https://thumbnails.roblox.com/v1/badges/icons?badgeIds={}&size=150x150&format=Png", join(chunk))) {
                let mut t = Vec::new();
                collect_thumbs(&v, &mut t);
                icons.extend(t);
            }
        }
        Ok(found
            .iter()
            .filter_map(|b| {
                let id = b["id"].as_u64()?;
                let st = &b["statistics"];
                Some(crate::achievements::BadgeInfo {
                    id,
                    name: b["displayName"].as_str().or(b["name"].as_str()).unwrap_or("").to_string(),
                    rate: st["winRatePercentage"].as_f64().unwrap_or(0.0) as f32,
                    won_yesterday: st["pastDayAwardedCount"].as_u64().unwrap_or(0),
                    won_ever: st["awardedCount"].as_u64().unwrap_or(0),
                    awarded: dates.get(&id).copied().unwrap_or(0),
                    icon: icons.get(&id).cloned().unwrap_or_default(),
                })
            })
            .collect())
    }

    /// Bundles the user owns (bodies, dynamic heads, animation packs, shoes; Roblox's and UGC
    /// creators'): id, name.
    pub fn owned_bundles(&self, uid: u64) -> Result<Vec<(u64, String)>> {
        let mut out = Vec::new();
        let mut cursor = String::new();
        loop {
            let v = self.get(&format!("https://catalog.roblox.com/v1/users/{uid}/bundles?limit=100&sortOrder=Desc&cursor={cursor}"))?;
            for b in v["data"].as_array().cloned().unwrap_or_default() {
                if let Some(id) = b["id"].as_u64() {
                    out.push((id, b["name"].as_str().unwrap_or("").trim().to_string()));
                }
            }
            match v["nextPageCursor"].as_str() {
                Some(c) if !c.is_empty() => cursor = c.to_string(),
                _ => break,
            }
        }
        Ok(out)
    }

    /// What a bundle puts on: (asset id, asset type); its costume outfit is left out.
    pub fn bundle_assets(&self, id: u64) -> Result<Vec<(u64, u32)>> {
        let v = self.get(&format!("https://catalog.roblox.com/v1/bundles/{id}/details"))?;
        Ok(v["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|i| i["type"].as_str() == Some("Asset"))
            .filter_map(|i| Some((i["id"].as_u64()?, i["assetType"].as_u64().unwrap_or(0) as u32)))
            .collect())
    }

    pub fn bundle_thumbs(&self, ids: &[u64]) -> Result<Vec<(u64, String)>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(100) {
            let v = self.get(&format!("https://thumbnails.roblox.com/v1/bundles/thumbnails?bundleIds={}&size=150x150&format=Png&isCircular=false", join(chunk)))?;
            collect_thumbs(&v, &mut out);
        }
        Ok(out)
    }

    pub fn asset_thumbs(&self, ids: &[u64]) -> Result<Vec<(u64, String)>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(100) {
            let v = self.get(&format!(
                "https://thumbnails.roblox.com/v1/assets?assetIds={}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false",
                join(chunk)
            ))?;
            collect_thumbs(&v, &mut out);
        }
        Ok(out)
    }

    pub fn asset_names(&self, ids: &[u64]) -> Vec<(u64, String)> {
        let items: Vec<Value> = ids.iter().map(|id| json!({ "itemType": "Asset", "id": id })).collect();
        let mut out = Vec::new();
        if let Ok(v) = self.post("https://catalog.roblox.com/v1/catalog/items/details", &json!({ "items": items })) {
            for d in v["data"].as_array().cloned().unwrap_or_default() {
                if let (Some(id), Some(n)) = (d["id"].as_u64(), d["name"].as_str()) {
                    out.push((id, n.to_string()));
                }
            }
        }
        out
    }
}

/// "2026-10-17T23:05:10.26Z" as unix seconds (UTC; 0 when unreadable).
fn iso_seconds(t: &str) -> u64 {
    let n = |a: usize, b: usize| t.get(a..b).and_then(|x| x.parse::<i64>().ok());
    let (Some(y), Some(m), Some(d)) = (n(0, 4), n(5, 7), n(8, 10)) else { return 0 };
    let (hh, mm, ss) = (n(11, 13).unwrap_or(0), n(14, 16).unwrap_or(0), n(17, 19).unwrap_or(0));
    // days from the civil date (Howard Hinnant's algorithm)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + hh * 3600 + mm * 60 + ss).max(0) as u64
}

fn join(ids: &[u64]) -> String {
    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

fn thumb_url(v: &Value, i: usize) -> Result<String> {
    v["data"][i]["imageUrl"].as_str().map(|s| s.to_string()).ok_or_else(|| anyhow!("thumbnail not ready"))
}

fn collect_thumbs(v: &Value, out: &mut Vec<(u64, String)>) {
    for t in v["data"].as_array().cloned().unwrap_or_default() {
        if let (Some(id), Some(u)) = (t["targetId"].as_u64(), t["imageUrl"].as_str()) {
            if !u.is_empty() {
                out.push((id, u.to_string()));
            }
        }
    }
}

fn game_from_json(g: &Value) -> GameInfo {
    GameInfo {
        universe_id: g["universeId"].as_u64().unwrap_or(0),
        place_id: g["rootPlaceId"].as_u64().or(g["placeId"].as_u64()).unwrap_or(0),
        name: g["name"].as_str().unwrap_or("").to_string(),
        player_count: g["playerCount"].as_i64().unwrap_or(0),
        up_votes: g["totalUpVotes"].as_i64().unwrap_or(0),
        down_votes: g["totalDownVotes"].as_i64().unwrap_or(0),
    }
}

fn base64url(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..c.len() + 1 {
            out.push(T[(n >> (18 - 6 * i)) as usize & 63] as char);
        }
    }
    out
}

// >> end << //
