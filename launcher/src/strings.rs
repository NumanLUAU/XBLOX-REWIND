/*
    written by: @corescripts
*/

//! English strings for the Shell. The original en-US.lua was not part of the provided
//! scripts, so these are reconstructed from the keys used in the Lua.

pub struct ErrDef {
    pub title: String,
    pub msg: String,
    /// Errors carry a code, alerts carry an id (ErrorOverlay picks the icon from this).
    pub code: Option<u32>,
    pub alert: bool,
}

fn err(title: &str, msg: &str, code: u32) -> ErrDef {
    ErrDef { title: title.into(), msg: msg.into(), code: Some(code), alert: false }
}

fn alert(title: &str, msg: &str) -> ErrDef {
    ErrDef { title: title.into(), msg: msg.into(), code: None, alert: true }
}

pub const ERROR_OCCURRED: &str = "An Error Occurred";
pub const UNABLE_TO_JOIN: &str = "Unable to Join Game";

/// Errors.Default
pub fn default_error() -> ErrDef {
    err(ERROR_OCCURRED, "An error occurred. Please try again later.", 0)
}

/// Errors.Test.FeatureNotAvailableInStudio — what the Shell shows for console-only features on Windows.
pub fn feature_not_available() -> ErrDef {
    err("An Error Occured", "This feature is not available in Roblox Studio.", 403)
}

/// Errors.GameJoin.Default with the underlying reason appended.
pub fn game_join(reason: &str) -> ErrDef {
    err(UNABLE_TO_JOIN, &format!("Unable to join the game. Please try again later.\n{reason}"), 112)
}

pub fn favorite_failed() -> ErrDef {
    err("Unable to Favorite", "An error occurred. Please try again later.", 301)
}

/// Errors.Authentication[-1]
pub fn auth_error(reason: &str) -> ErrDef {
    err("Authentication Error", &format!("Unable to sign in. {reason}"), 901)
}

pub fn web_connect_failed() -> ErrDef {
    err("Authentication Error", "Unable to connect to the ROBLOX web server. Please check your connection and try again.", 1108)
}

/// Errors.Vote.PlayGame
pub fn vote_play_game() -> ErrDef {
    err("Cannot Vote", "You must play this game before you can vote on it.", 202)
}

/// Errors.PackageEquip.Default
pub fn equip_failed() -> ErrDef {
    err("Unable to Equip", "Unable to equip this item. Please try again later.", 501)
}

/// Purchases are disabled in this launcher.
pub fn purchase_unavailable() -> ErrDef {
    err("An Error Occured", "This feature is not available.", 701)
}

pub const REPORT_PHRASE: &str = "Are you sure you want to report this game? Reports are reviewed by Roblox moderation. Submitting opens the Roblox report form for this game.";

/// Alerts.Default style info message
pub fn info(title: &str, msg: &str) -> ErrDef {
    alert(title, msg)
}

pub fn error_text(d: &ErrDef) -> String {
    match d.code {
        Some(c) => format!("{}\n\nError Code: {}", d.msg, c),
        None => d.msg.clone(),
    }
}

pub const ROBUX_STORE_DESCRIPTION: &str = "Buy ROBUX to purchase upgrades for your avatar, or buy special abilities in games.";
pub const ROBUX_BALANCE_OVERLAY_TITLE: &str = "ROBUX Balance";
pub const ROBUX_BALANCE_OVERLAY_PHRASE: &str =
    "Your ROBUX balance is shared across every platform you play ROBLOX on. Purchases made in this launcher are simulated and only last until you close it.";
pub const NO_FRIENDS_ONLINE: &str = "None of your friends are online.";
pub const NO_FRIENDS: &str = "None of your friends are online right now.";
pub const ENGAGEMENT_HINT: &str = "Press any button to start";
pub const RESIZE_PROMPT: &str = "Adjust the arrows so they fit the edges of your TV screen.";
pub const RESIZE_INPUT_HINT: &str = "Resize";
pub const LINKED_AS: &str = "This launcher is signed in to the ROBLOX account %s with a .ROBLOSECURITY cookie.";

static UPPER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static MODERN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Configure > Uppercase Words / Branding.
pub fn set_style(upper: bool, modern: bool) {
    UPPER.store(upper, std::sync::atomic::Ordering::Relaxed);
    MODERN.store(modern, std::sync::atomic::Ordering::Relaxed);
}

/// A menu word in the selected style: "GAMES" -> "Games" (lowercase era) / "EXPERIENCES" (2021 branding).
pub fn word(s: &str) -> String {
    let mut w = s.to_string();
    if MODERN.load(std::sync::atomic::Ordering::Relaxed) {
        // Hub Change.lua: the Games tab becomes Discover with the 2021 branding
        w = w.replace("SEARCH GAMES", "SEARCH EXPERIENCES").replace("GAMES", "DISCOVER");
    }
    if UPPER.load(std::sync::atomic::Ordering::Relaxed) {
        return w.to_uppercase();
    }
    w.split(' ')
        .map(|t| {
            let lower = t.to_lowercase();
            let mut c = lower.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// >> end << //
