/*
    written by: @corescripts
*/

//! Persistent settings. The .ROBLOSECURITY cookie is stored encrypted with Windows
//! DPAPI (bound to the current Windows user), never in plain text.

// >> imports
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// config.ini: the site the launcher talks to. It sits next to XbloxRewind.exe (or in a folder
/// above it) and is written with the default the first time the launcher runs.
const CONFIG_INI: &str = "; XBLOX: REWIND
;
; domain: the site every web request goes to. Leave it as roblox.com, or set it to a
; Roblox revival's domain (for example: domain = example.com).

[server]
domain = roblox.com
";

/// The configured site domain ("roblox.com" unless config.ini says otherwise).
pub fn domain() -> &'static str {
    static DOMAIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DOMAIN.get_or_init(|| {
        let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("."));
        let found = exe_dir.ancestors().map(|d| d.join("config.ini")).find(|p| p.is_file());
        let text = match &found {
            Some(p) => std::fs::read_to_string(p).unwrap_or_default(),
            None => {
                let _ = std::fs::write(exe_dir.join("config.ini"), CONFIG_INI);
                CONFIG_INI.to_string()
            }
        };
        text.lines()
            .map(|l| l.split(';').next().unwrap_or("").trim())
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| k.trim().eq_ignore_ascii_case("domain"))
            .map(|(_, v)| v.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_ascii_lowercase())
            .filter(|d| !d.is_empty() && d.contains('.') && !d.contains('/'))
            .unwrap_or_else(|| "roblox.com".into())
    })
}

/// Point a roblox.com URL at the configured domain (unchanged on roblox.com).
pub fn web(url: &str) -> String {
    let d = domain();
    if d == "roblox.com" {
        return url.to_string();
    }
    let Some(rest) = url.strip_prefix("https://") else { return url.to_string() };
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, String::new()));
    if host == "roblox.com" {
        format!("https://{d}{path}")
    } else if let Some(sub) = host.strip_suffix(".roblox.com") {
        format!("https://{sub}.{d}{path}")
    } else {
        url.to_string()
    }
}

pub fn data_dir() -> PathBuf {
    let d = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("XboxShell");
    let _ = std::fs::create_dir_all(&d);
    d
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconSet {
    #[default]
    Auto,
    Xbox,
    Keyboard,
    PlayStation,
}

impl IconSet {
    pub fn label(self) -> &'static str {
        match self {
            IconSet::Auto => "AUTO",
            IconSet::Xbox => "XBOX",
            IconSet::Keyboard => "KEYBOARD",
            IconSet::PlayStation => "PLAYSTATION",
        }
    }
    pub fn next(self) -> Self {
        match self {
            IconSet::Auto => IconSet::Xbox,
            IconSet::Xbox => IconSet::Keyboard,
            IconSet::Keyboard => IconSet::PlayStation,
            IconSet::PlayStation => IconSet::Auto,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub enum LaunchMode {
    /// Open the roblox-player: URI with whatever app is registered (Roblox or a bootstrapper).
    #[default]
    ProtocolHandler,
    /// Pass the URI straight to a chosen bootstrapper executable.
    CustomExe,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub remember_cookie: bool,
    pub launch_mode: LaunchMode,
    pub bootstrapper_path: String,
    pub music_volume: f32,
    pub sfx_volume: f32,
    /// Apply the original menu's desaturate/tint/blur filter over the 3D background.
    pub background_filter: bool,
    pub shadows: bool,
    pub fullscreen: bool,
    /// OverscanScreen edge fraction (GameOptionsSettings.OverscanPX/PY)
    pub overscan: [f32; 2],
    /// user id signed in automatically from the engagement screen (X on Switch Profile)
    pub default_account: u64,
    /// Button glyph set (Configure > Icons)
    pub icons: IconSet,
    /// Configuration hub choices (Menu/Settings.lua names -> option)
    pub ui: std::collections::BTreeMap<String, String>,
}

impl Config {
    /// Current option for a configuration-hub setting, falling back to its default.
    pub fn ui(&self, name: &str) -> &str {
        if let Some(v) = self.ui.get(name) {
            return v;
        }
        crate::hub::SETTINGS.iter().find(|s| s.name == name).map(|s| s.default).unwrap_or("")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            remember_cookie: true,
            launch_mode: LaunchMode::ProtocolHandler,
            bootstrapper_path: String::new(),
            music_volume: 1.0,
            sfx_volume: 1.0,
            background_filter: true,
            shadows: true,
            fullscreen: false,
            overscan: [1.0, 1.0],
            default_account: 0,
            icons: IconSet::Auto,
            ui: Default::default(),
        }
    }
}

impl Config {
    fn path() -> PathBuf {
        data_dir().join("settings.json")
    }
    /// Whether settings were ever saved (false on a fresh install).
    pub fn saved() -> bool {
        Self::path().exists()
    }
    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        if let Ok(s) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::path(), s);
        }
    }
}

/// A saved sign-in. Stored as JSON encrypted with DPAPI for the current Windows user.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Account {
    pub id: u64,
    pub name: String,
    pub display_name: String,
    pub cookie: String,
}

fn accounts_path() -> PathBuf {
    data_dir().join("accounts.bin")
}

pub fn load_accounts() -> Vec<Account> {
    if let Some(dec) = std::fs::read(accounts_path()).ok().and_then(|e| dpapi::unprotect(&e)) {
        if let Ok(v) = serde_json::from_slice::<Vec<Account>>(&dec) {
            return v;
        }
    }
    // single-cookie file from earlier versions
    load_cookie().map(|cookie| vec![Account { cookie, ..Default::default() }]).unwrap_or_default()
}

pub fn save_accounts(accounts: &[Account]) {
    if let Ok(json) = serde_json::to_vec(accounts) {
        if let Some(enc) = dpapi::protect(&json) {
            let _ = std::fs::write(accounts_path(), enc);
            forget_cookie();
        }
    }
}

fn cookie_path() -> PathBuf {
    data_dir().join("session.bin")
}

pub fn save_cookie(cookie: &str) {
    if let Some(enc) = dpapi::protect(cookie.as_bytes()) {
        let _ = std::fs::write(cookie_path(), enc);
    }
}

pub fn load_cookie() -> Option<String> {
    let enc = std::fs::read(cookie_path()).ok()?;
    let dec = dpapi::unprotect(&enc)?;
    String::from_utf8(dec).ok().filter(|s| !s.is_empty())
}

pub fn forget_cookie() {
    let _ = std::fs::remove_file(cookie_path());
}

#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn run(input: &[u8], protect: bool) -> Option<Vec<u8>> {
        let inp = CRYPT_INTEGER_BLOB { cbData: input.len() as u32, pbData: input.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = unsafe {
            if protect {
                CryptProtectData(&inp, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&inp, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            }
        };
        if ok == 0 || out.pbData.is_null() {
            return None;
        }
        let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
        unsafe { LocalFree(out.pbData as _) };
        Some(v)
    }

    pub fn protect(d: &[u8]) -> Option<Vec<u8>> {
        run(d, true)
    }
    pub fn unprotect(d: &[u8]) -> Option<Vec<u8>> {
        run(d, false)
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
    pub fn unprotect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

// >> end << //
