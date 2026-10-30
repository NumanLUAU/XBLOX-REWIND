/*
    written by: @corescripts
*/

//! Builds the same `roblox-player:` launch URI the website hands to the OS, so that
//! whichever handler is registered (RobloxPlayerLauncher, Bloxstrap, Fishstrap, ...)
//! can start the game. Optionally the URI is passed straight to a chosen bootstrapper exe.

// >> imports
use crate::api::Session;
use crate::config::{Config, LaunchMode};
use anyhow::{bail, Result};
use std::time::{SystemTime, UNIX_EPOCH};

pub enum JoinTarget {
    Place(u64),
    /// Follow a friend into their server.
    Follow(u64),
    /// A specific server instance.
    Instance(u64, String),
}

fn browser_tracker_id() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen_range(100_000_000_000u64..999_999_999_999u64)
}

pub fn build_uri(ticket: &str, target: &JoinTarget) -> String {
    let bt = browser_tracker_id();
    let join_id = uuid::Uuid::new_v4();
    let launcher = match target {
        JoinTarget::Place(pid) => crate::config::web(&format!(
            "https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={bt}&placeId={pid}&isPlayTogetherGame=false&joinAttemptId={join_id}&joinAttemptOrigin=PlayButton"
        )),
        JoinTarget::Follow(uid) => crate::config::web(&format!(
            "https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestFollowUser&browserTrackerId={bt}&userId={uid}&joinAttemptId={join_id}&joinAttemptOrigin=followUser"
        )),
        JoinTarget::Instance(pid, game_id) => crate::config::web(&format!(
            "https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={bt}&placeId={pid}&gameId={game_id}&isPlayTogetherGame=false&joinAttemptId={join_id}&joinAttemptOrigin=publicServerListJoin"
        )),
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!(
        "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now}+placelauncherurl:{}+browsertrackerid:{bt}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp",
        urlencoding::encode(&launcher)
    )
}

pub fn launch(session: &Session, cfg: &Config, target: JoinTarget) -> Result<()> {
    let ticket = session.auth_ticket()?;
    let uri = build_uri(&ticket, &target);
    match cfg.launch_mode {
        LaunchMode::ProtocolHandler => {
            if registered_handler().is_none() {
                bail!("No roblox-player handler is installed. Install Roblox or a bootstrapper first.");
            }
            open::that_detached(&uri)?;
        }
        LaunchMode::CustomExe => {
            if cfg.bootstrapper_path.is_empty() || !std::path::Path::new(&cfg.bootstrapper_path).exists() {
                bail!("Bootstrapper path is not set (Settings > Launcher).");
            }
            std::process::Command::new(&cfg.bootstrapper_path).arg(&uri).spawn()?;
        }
    }
    Ok(())
}

/// PIDs of running Roblox player processes.
pub fn player_pids() -> Vec<u32> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            if name.eq_ignore_ascii_case("RobloxPlayerBeta.exe") {
                out.push(e.th32ProcessID);
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn kill(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).creation_flags(0x0800_0000).output();
    }
}

/// Let the client (or a bootstrapper) take the foreground when it opens its window.
pub fn allow_foreground() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY);
    }
}

/// Visible top-level window owned by one of `pids`, if the client has opened one yet.
pub fn game_window(pids: &[u32]) -> Option<isize> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible};
        struct Search<'a> {
            pids: &'a [u32],
            found: Option<isize>,
        }
        unsafe extern "system" fn visit(hwnd: HWND, lp: LPARAM) -> BOOL {
            let s = &mut *(lp as *mut Search);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if s.pids.contains(&pid) && IsWindowVisible(hwnd) != 0 && GetWindowTextLengthW(hwnd) > 0 {
                s.found = Some(hwnd as isize);
                return 0;
            }
            1
        }
        let mut s = Search { pids, found: None };
        unsafe { EnumWindows(Some(visit), &mut s as *mut Search as LPARAM) };
        s.found
    }
    #[cfg(not(windows))]
    {
        let _ = pids;
        None
    }
}

/// Bring a window to the front even when another app currently has focus.
pub fn force_foreground(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::WindowsAndMessaging::{BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(me, fg_thread, 0);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// The project's Discord server and source code.
pub const DISCORD_URL: &str = "https://discord.gg/F7Nzshp9VT";
pub const GITHUB_URL: &str = "https://github.com/NumanLUAU/XBLOX-REWIND";

/// Open a web page in the default browser (only the fixed project links above are passed here).
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing over the menu
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Returns the command registered for `roblox-player:` (to show which launcher will open games).
pub fn registered_handler() -> Option<String> {
    #[cfg(windows)]
    {
        for key in [r"HKCU\Software\Classes\roblox-player\shell\open\command", r"HKCR\roblox-player\shell\open\command"] {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console flashing over the menu
            let out = std::process::Command::new("reg").args(["query", key, "/ve"]).creation_flags(0x0800_0000).output().ok()?;
            if !out.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Some(i) = line.find("REG_SZ") {
                    return Some(line[i + 6..].trim().to_string());
                }
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// handler_name for the screen opened at `key` (its t0): looked up once per visit on a
/// background thread, so the screen never waits on the registry. Empty until it arrives.
pub fn handler_name_cached(key: f32) -> String {
    static CACHE: std::sync::Mutex<Option<(f32, String)>> = std::sync::Mutex::new(None);
    let mut c = CACHE.lock().unwrap();
    match c.as_ref() {
        Some((k, n)) if *k == key => n.clone(),
        _ => {
            let old = c.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
            *c = Some((key, old.clone()));
            std::thread::spawn(move || {
                let n = handler_name();
                if let Some((k, v)) = CACHE.lock().unwrap().as_mut() {
                    if *k == key {
                        *v = n;
                    }
                }
            });
            old
        }
    }
}

/// Friendly name of the handler (e.g. "Bloxstrap") from its command line.
pub fn handler_name() -> String {
    let Some(cmd) = registered_handler() else { return "Not installed".into() };
    let exe = cmd.trim_start_matches('"').split('"').next().unwrap_or(&cmd).to_string();
    let file = std::path::Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(exe);
    if file.eq_ignore_ascii_case("RobloxPlayerLauncher") || file.eq_ignore_ascii_case("RobloxPlayerBeta") {
        "Roblox".into()
    } else {
        file
    }
}

// >> end << //
