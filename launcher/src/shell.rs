/*
    written by: @corescripts
*/

//! The AppShell screens ported from the Xbox CoreScripts onto the immediate-mode UI:
//! EngagementScreen, AppHub (+ Home/Avatar/Games/Social/Store panes), GameDetailScreen,
//! SocialScreen, SettingsScreen, AccountScreen, OverscanScreen, and the overlays
//! (ErrorOverlay, ConfirmPrompt, SideBar, RobuxBalanceOverlay).
//!
//! Layout numbers come from the Lua (1920x1080 mockup space, inside TitleSafeContainer).

// >> imports
use crate::api;
use crate::api::{AvatarItem, Friend, GameDetails, GameInfo, Profile, Session, Sort, User};
use crate::party;
use crate::achievements::{self, Achievements};
use crate::audio::Audio;
use crate::config::{self, Account, Config, LaunchMode};
use crate::input::{glyph, Btn, Input};
use crate::launch::{self, JoinTarget};
use crate::strings::{self as s, ErrDef};
use crate::ui::{ease_in_out_quad, ease_out_quad, id, Align, Font, Rect, Ui};
use glam::Vec2;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// GlobalSettings
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const GREY_TEXT: [f32; 3] = [0.5, 0.5, 0.5];
const LIGHT_GREY_TEXT: [f32; 3] = [184.0 / 255.0, 184.0 / 255.0, 184.0 / 255.0];
const BLUE_TEXT: [f32; 3] = [0.0, 116.0 / 255.0, 189.0 / 255.0];
const GREEN_TEXT: [f32; 3] = [2.0 / 255.0, 183.0 / 255.0, 87.0 / 255.0];
const RED_TEXT: [f32; 3] = [216.0 / 255.0, 104.0 / 255.0, 104.0 / 255.0];
const TEXT_SELECTED: [f32; 3] = [19.0 / 255.0, 19.0 / 255.0, 19.0 / 255.0];
const LINE_BREAK: [f32; 3] = [78.0 / 255.0, 78.0 / 255.0, 78.0 / 255.0];
const PAGE_DIVIDE: [f32; 3] = [151.0 / 255.0, 151.0 / 255.0, 151.0 / 255.0];
const OVERLAY: [f32; 3] = [26.0 / 255.0, 57.0 / 255.0, 76.0 / 255.0];
const BLUE_BUTTON: [f32; 3] = [50.0 / 255.0, 181.0 / 255.0, 1.0];
const GREY_SELECTION: [f32; 3] = [84.0 / 255.0, 99.0 / 255.0, 109.0 / 255.0];
const GREEN_BUTTON: [f32; 3] = [2.0 / 255.0, 163.0 / 255.0, 77.0 / 255.0];
const GREEN_SELECTED: [f32; 3] = [63.0 / 255.0, 198.0 / 255.0, 121.0 / 255.0];
const GREY_BUTTON: [f32; 3] = [78.0 / 255.0, 84.0 / 255.0, 96.0 / 255.0];
const GREY_SELECTED: [f32; 3] = [50.0 / 255.0, 181.0 / 255.0, 1.0];
const CHARACTER_BG: [f32; 3] = [39.0 / 255.0, 69.0 / 255.0, 82.0 / 255.0];
const FOREGROUND_GREY: [f32; 3] = [58.0 / 255.0, 60.0 / 255.0, 64.0 / 255.0];
const TAB_UNDERLINE: [f32; 3] = [50.0 / 255.0, 181.0 / 255.0, 1.0];
const MODAL_ALPHA: f32 = 0.7; // ModalBackgroundTransparency 0.3
const TEXTBOX_DEFAULT_ALPHA: f32 = 0.25; // TextBoxDefaultTransparency 0.75
const TEXTBOX_SELECTED_ALPHA: f32 = 0.5;
const AVATAR_BOX_SELECTED_ALPHA: f32 = 0.25;

// FontSize enum values
const LARGE: f32 = 96.0;
const HEADER: f32 = 60.0;
const MEDIUM: f32 = 48.0;
const TITLE: f32 = 42.0;
const BUTTON: f32 = 36.0;
const DESCRIPTION: f32 = 32.0;
const SUBHEADER: f32 = 28.0;
const SMALL_TITLE: f32 = 24.0;

// Avatar editor layout: LuaApp/Legacy/AvatarEditor/LayoutInfoConsole
const ED_X: f32 = 100.0;
const ED_SUB_X: f32 = 220.0;
const ED_Y0: f32 = 270.0;
const ED_ROW_W: f32 = 360.0;
const ED_ROW_H: f32 = 80.0;
const ED_STEP: f32 = 100.0;
const ED_TEXT: f32 = 30.0;
/// the card grid's scrolling frame ends 99px from the right: 3 x 150 + 2 x 20 wide
const ED_GRID_X: f32 = 1920.0 - 99.0 - 490.0;
const ED_TILE: f32 = 150.0;
const ED_TILE_STEP: f32 = 170.0;
const ED_BLUE: [f32; 3] = [13.0 / 255.0, 170.0 / 255.0, 247.0 / 255.0];
const ED_TILE_BG: [f32; 3] = [48.0 / 255.0, 98.0 / 255.0, 125.0 / 255.0];
const ED_GREEN: [f32; 3] = [40.0 / 255.0, 200.0 / 255.0, 90.0 / 255.0];
const CIRCLE_IMG: &str = "rbxasset://glyphs/editor/Circle.png";
const CORNER_IMG: &str = "rbxasset://glyphs/editor/Corner.png";
const VIEW_IMG: &str = "rbxasset://glyphs/editor/View.png";
const RS_IMG: &str = "rbxasset://glyphs/editor/RS.png";

/// What an editor (sub) category shows.
#[derive(Clone, Copy, PartialEq)]
enum EdSub {
    Recent,
    Asset(u32),
    Skin,
    Scale,
    /// emote wheel slot 1-8
    Emote(u8),
    MyOutfits,
    PresetOutfits,
    /// bundles the user owns (UGC ones too)
    Bundles,
}

/// Bundle tiles share the editor grid (and its thumbnail map) with asset ids; this bit keeps
/// their ids apart.
const BUNDLE_TAG: u64 = 1 << 62;

/// Asset types an avatar wears one of at a time (wearing another replaces it).
fn single_slot(ty: u32) -> bool {
    matches!(ty, 2 | 11 | 12 | 17 | 18 | 19 | 27..=31 | 48..=56 | 78 | 79)
}

struct EdCat {
    name: &'static str,
    icon: &'static str,
    /// shown when the category has no sub menu
    own: EdSub,
    subs: &'static [(&'static str, EdSub)],
}

const ED_CATS: [EdCat; 6] = [
    EdCat { name: "Recent", icon: "rbxasset://glyphs/editor/Recent.png", own: EdSub::Recent, subs: &[] },
    EdCat {
        name: "Clothing",
        icon: "rbxasset://glyphs/editor/Clothing.png",
        own: EdSub::Asset(8),
        subs: &[
            ("Hats", EdSub::Asset(8)),
            ("Shirts", EdSub::Asset(11)),
            ("Pants", EdSub::Asset(12)),
            ("Hair", EdSub::Asset(41)),
            ("Face Accessories", EdSub::Asset(42)),
            ("Neck Accessories", EdSub::Asset(43)),
            ("Shoulder Accessories", EdSub::Asset(44)),
            ("Front Accessories", EdSub::Asset(45)),
            ("Back Accessories", EdSub::Asset(46)),
            ("Waist Accessories", EdSub::Asset(47)),
            ("T-Shirts", EdSub::Asset(2)),
            ("Gear", EdSub::Asset(19)),
        ],
    },
    EdCat {
        name: "Body",
        icon: "rbxasset://glyphs/editor/Body.png",
        own: EdSub::Skin,
        subs: &[
            ("Skin Tone", EdSub::Skin),
            ("Scale", EdSub::Scale),
            ("Faces", EdSub::Asset(18)),
            ("Heads", EdSub::Asset(17)),
            ("Dynamic Heads", EdSub::Asset(79)),
            ("Torsos", EdSub::Asset(27)),
            ("Right Arms", EdSub::Asset(28)),
            ("Left Arms", EdSub::Asset(29)),
            ("Right Legs", EdSub::Asset(30)),
            ("Left Legs", EdSub::Asset(31)),
        ],
    },
    EdCat {
        name: "Animations",
        icon: "rbxasset://glyphs/editor/Animations.png",
        own: EdSub::Asset(51),
        subs: &[
            ("Idle Animations", EdSub::Asset(51)),
            ("Walk Animations", EdSub::Asset(55)),
            ("Run Animations", EdSub::Asset(53)),
            ("Jump Animations", EdSub::Asset(52)),
            ("Fall Animations", EdSub::Asset(50)),
            ("Climb Animations", EdSub::Asset(48)),
            ("Swim Animations", EdSub::Asset(54)),
        ],
    },
    EdCat {
        name: "Emotes",
        icon: "rbxasset://glyphs/editor/Emotes.png",
        own: EdSub::Emote(1),
        subs: &[
            ("1", EdSub::Emote(1)),
            ("2", EdSub::Emote(2)),
            ("3", EdSub::Emote(3)),
            ("4", EdSub::Emote(4)),
            ("5", EdSub::Emote(5)),
            ("6", EdSub::Emote(6)),
            ("7", EdSub::Emote(7)),
            ("8", EdSub::Emote(8)),
        ],
    },
    EdCat {
        name: "Outfits",
        icon: "rbxasset://glyphs/editor/Outfits.png",
        own: EdSub::MyOutfits,
        subs: &[("My Costumes", EdSub::MyOutfits), ("My Bundles", EdSub::Bundles), ("Preset Costumes", EdSub::PresetOutfits)],
    },
];

/// Every inventory type the editor lists (prefetched in the background after sign-in).
pub const EDITOR_ASSET_TYPES: [u32; 28] = [8, 11, 12, 41, 42, 43, 44, 45, 46, 47, 2, 19, 18, 17, 79, 27, 28, 29, 30, 31, 51, 55, 53, 52, 50, 48, 54, 61];

/// The console editor's skin tone swatches (5 x 6).
const SKIN_HEX: [&str; 30] = [
    "594A41", "775845", "AC9083", "C6886B", "E4B593", "533F36", "653C28", "B89B5E", "C3AB78", "D4C49A", "917575", "9B404A", "D27D7A", "F9C0C8", "F788DB",
    "74859B", "567BAA", "84BDD8", "B09FFB", "A15198", "0D9D9A", "5FA54E", "7DA16C", "A2C98C", "DA9844", "F0D236", "F3DB70", "635D62", "A1A0A3", "F8F8F8",
];

const XBOX_SPHERE_IMG: &str = "rbxasset://achievements/XboxSphere.png";
const XBOX_GREEN: [f32; 3] = [16.0 / 255.0, 124.0 / 255.0, 16.0 / 255.0];
const ACH_ART_BG: [f32; 3] = [28.0 / 255.0, 88.0 / 255.0, 128.0 / 255.0];

const BUTTON_IMG: &str = "rbxasset://textures/ui/Shell/Buttons/Generic9ScaleButton@720.png";
const SHADOW_IMG: &str = "rbxasset://textures/ui/Shell/Buttons/Generic9ScaleShadow.png";
const SPINNER: &str = "rbxasset://textures/ui/Shell/Icons/LoadingSpinner@1080.png";

// AppContainer: ActionSafe (64,32) then TitleSafe inset
const SAFE: Rect = Rect::new(64.0 + 36.0, 32.0 + 7.5, 1920.0 - 128.0 - 72.0, 1080.0 - 64.0 - 15.0);
const PANE_Y: f32 = 0.214 * 1001.0;
const TAB_Y: f32 = 0.132 * 1001.0;

const TABS: [&str; 5] = ["HOME", "AVATAR", "GAMES", "FRIENDS", "ROBUX"];
const DISPLAY_FRIEND_COUNT: usize = 15;

/// (robux, price) — the Xbox catalog ROBUX packages. Purchases are simulated per session.
const PACKAGES: [(i64, &str); 6] = [(400, "4.99"), (800, "9.99"), (1700, "19.99"), (4500, "49.99"), (10000, "99.99"), (22500, "199.99")];

pub enum Msg {
    Auth(Result<User, String>, Arc<Session>),
    /// Quick Login: a fresh code, a status poll (status, masked name), the redeemed sign-in
    QuickCode(Result<api::QuickCode, String>),
    QuickStatus(String, String, Option<String>),
    QuickDone(Result<(String, User), String>),
    QuickHeadshot(u64, String),
    Robux(i64),
    Avatar(String, String),
    Friends(Vec<Friend>),
    Headshots(Vec<(u64, String)>),
    Sorts(Vec<Sort>),
    Favorites(Vec<GameInfo>),
    Icons(Vec<(u64, String)>),
    Details(u64, Result<GameDetails, String>),
    PlaceUniverse(u64, Option<u64>),
    Search(String, Result<Vec<GameInfo>, String>),
    Wearing(Vec<(u64, String, String)>),
    Launched(Result<String, String>),
    EditorItems(u32, Result<Vec<(u64, String)>, String>),
    EditorThumbs(Vec<(u64, String)>),
    /// new badges for this account (badge gamerscore and popups)
    Badges(u64, Vec<achievements::BadgeInfo>),
    /// the bundles the user owns
    EditorBundles(Result<Vec<(u64, String)>, String>),
    /// a bundle's assets (id, type); wear them once known when the bundle was pressed
    BundleAssets(u64, Vec<(u64, u32)>, bool),
    EditorWearing(Vec<u64>),
    /// a party join attempt finished
    PartyJoined(String, Result<party::Party, String>),
    /// our own presence (for the party): type, place, server, location
    SelfPresence(u8, Option<u64>, Option<String>, String),
    /// a friend's full body render for the Friends tab
    FriendRender(u64, String),
    /// home RECENTLY PLAYED (discovery "Continue" sort)
    Recent(Vec<GameInfo>),
    EditorEmotes(Vec<(u8, u64, String)>),
    /// R15 scales and whether the saved avatar is R15
    EditorScales([f32; 5], bool),
    /// the saved avatar (avatar/v2): what the editor changes locally before saving
    EditorDesc(serde_json::Value),
    /// a background avatar save finished: its number, success, new 2D renders
    EditorDone(u64, bool, Option<(String, String)>),
    Favorited(u64, bool, bool),
    AvatarItems(Result<(Vec<AvatarItem>, Vec<AvatarItem>), String>),
    GamePids(Vec<u32>),
    GameWindow(isize),
    GameClosed,
    LoadingInfo(String, String),
    Profile(u64, Result<Profile, String>),
    Worn(u64, bool, Option<(String, String)>),
    Voted(u64, Option<bool>, Option<bool>, bool),
}

enum Screen {
    Engagement { leaving: Option<f32>, authing: bool },
    SignIn { cookie: String, busy: bool },
    /// Quick Login: a code / QR approved on a device that is already signed in
    QuickLogin { t0: f32 },
    Hub,
    GameDetail { universe: u64, place: u64, t0: f32, scroll: f32, target: f32 },
    Social { t0: f32, scroll: f32 },
    Settings { t0: f32 },
    Account { t0: f32 },
    Overscan { edges: Vec2, shown: Vec2, last: f32 },
    Achievements { t0: f32 },
    /// party: members, settings, leave (or host / join when not in one)
    Party { t0: f32 },
    /// sub: the entered category's sub menu row (None = the main category list)
    AvatarEditor { t0: f32, cat: usize, sub: Option<usize>, full: bool },
    Search { query: String, t0: f32, symbols: bool, shift: bool, closing: Option<f32> },
    /// "View Gamer Card": a friend's profile in the GameDetailScreen style
    Profile { uid: u64, t0: f32, scroll: f32, target: f32 },
    GameGrid { title: String, games: Vec<GameInfo>, t0: f32, scroll: f32 },
    /// carousel of games; `games` None while `query` is being searched; `pos` the scroll position
    Carousel { title: String, back: String, games: Option<Vec<GameInfo>>, query: Option<String>, t0: f32, pos: f32 },
    Profiles { t0: f32, scroll: f32 },
    Help { t0: f32 },
    /// 2016 in-game loading screen while the client starts, then a black "waiting" screen
    /// until the client closes. `window` is when the Roblox window appeared.
    Loading { name: String, creator: String, thumb: Option<String>, icon: Option<String>, t0: f32, launched: bool, error: Option<String>, pids: Vec<u32>, window: Option<f32>, presses: Vec<f32>, stop: Arc<AtomicBool> },
}

#[derive(Clone)]
enum SideAction {
    JoinGame(u64, String),
    ViewDetails(u64),
    ViewGamerCard(u64),
    /// ask a party member for their server and join it
    JoinPartyMember(u64),
}

enum Confirm {
    BuyRobux(i64),
    SignOut,
    Quit,
    /// the party host's server: place, job id, game name
    JoinServer(u64, String, String),
}

enum Overlay {
    Error { def: ErrDef, t0: f32 },
    Confirm { title: String, phrase: String, image: Option<String>, action: Confirm, t0: f32 },
    SideBar { items: Vec<(String, SideAction)>, t0: f32 },
    RobuxBalance { t0: f32 },
    Report { place: u64, t0: f32 },
    /// ImageOverlay: game screenshots; `prev` = (old index, switch time, direction)
    Images { urls: Vec<String>, index: usize, prev: Option<(usize, f32, f32)>, t0: f32 },
    Busy { text: String },
}

#[derive(Default)]
pub struct ShellEvents {
    pub quit: bool,
    /// bring the launcher back to the front
    pub focus_self: bool,
    /// hand the foreground to the Roblox window
    pub focus_game: Option<isize>,
    /// keep the launcher in front while the client is still starting
    pub keep_focus: bool,
    pub signed_in: bool,
    pub toggle_fullscreen: bool,
    pub config_changed: bool,
}

pub struct Shell {
    stack: Vec<Screen>,
    overlay: Option<Overlay>,
    /// BaseOverlay:Hide fades the modal background out after the content is gone.
    modal_out: Option<f32>,
    sidebar_out: Option<(f32, Vec<String>)>,
    pub session: Option<Arc<Session>>,
    user: Option<User>,
    robux: Option<i64>,
    fake_robux: i64,
    avatar: Option<String>,
    avatar_full: Option<String>,
    friends: Option<Vec<Friend>>,
    headshots: HashMap<u64, String>,
    sorts: Option<Vec<Sort>>,
    favorites: Option<Vec<GameInfo>>,
    icons: HashMap<u64, String>,
    icons_requested: HashSet<u64>,
    details: HashMap<u64, GameDetails>,
    details_requested: HashSet<u64>,
    wearing: Option<Vec<(u64, String, String)>>,
    tab: usize,
    tab_t0: f32,
    games_scroll: f32,
    games_row_scroll: HashMap<usize, f32>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub cfg: Config,
    remember: bool,
    last_tab_focus: Option<u64>,
    pane_focus: HashMap<usize, u64>,
    focus_memory: Vec<Option<u64>>,
    pub avatar_items: Option<Result<(Vec<AvatarItem>, Vec<AvatarItem>), String>>,
    wearing_id: Option<u64>,
    equip_busy: bool,
    avatar_fade: (f32, Option<String>),
    outfit_scroll: f32,
    catalog_scroll: f32,
    vote_busy: bool,
    accounts: Vec<Account>,
    adding_account: bool,
    /// Quick Login state: the code on screen, its status, the masked account name, when it was
    /// last polled, whether a request is out, and who is signing in once it is approved
    ql_code: Option<api::QuickCode>,
    ql_status: String,
    ql_name: Option<String>,
    ql_polled: f32,
    ql_busy: bool,
    ql_user: Option<(User, Option<String>)>,
    profiles_data: HashMap<u64, Result<Profile, String>>,
    music_ducked: bool,
    pub ach: Achievements,
    /// main.rs rebuilds the 3D avatar when set; avatar3d once it is drawable
    pub avatar_dirty: bool,
    pub avatar3d: bool,
    /// editor state: rotation, outfit being previewed, inventory per category, what is worn
    pub editor_yaw: f32,
    pub preview_outfit: Option<u64>,
    /// animation pack or emote asset whose animation the editor avatar plays (focused tile)
    pub preview_anim: Option<u64>,
    editor_items: HashMap<u32, Result<Vec<(u64, String)>, String>>,
    editor_thumbs: HashMap<u64, String>,
    editor_bundles: Option<Result<Vec<(u64, String)>, String>>,
    bundle_assets: HashMap<u64, Vec<(u64, u32)>>,
    editor_wearing: Option<Vec<u64>>,
    editor_busy: bool,
    /// the avatar as the editor shows it (changed locally before Roblox confirms), whether main
    /// must rebuild the 3D avatar from it, and the look before each pending save
    pub editor_desc: Option<serde_json::Value>,
    pub editor_desc_dirty: bool,
    editor_seq: u64,
    editor_undo: HashMap<u64, serde_json::Value>,
    /// console editor layout state: recently worn items, R15 scales (+ when a slider last moved),
    /// equipped emotes, the bottom toast, list / grid scroll, tiles drawn this frame and the first
    /// tile of each (category, sub) for jumping into the grid, the chosen skin swatch
    editor_recent: Vec<u64>,
    editor_scales: Option<[f32; 5]>,
    editor_scales_dirty: Option<f32>,
    editor_emotes: Vec<(u8, u64, String)>,
    editor_toast: Option<(String, f32)>,
    editor_list_scroll: f32,
    editor_grid_scroll: f32,
    /// where the grid is scrolling to, the row focused last frame, mouse wheel this frame, and
    /// where a drag across the avatar started
    editor_grid_target: f32,
    editor_grid_row: Option<usize>,
    editor_wheel: f32,
    editor_drag_x: Option<f32>,
    editor_tile_ids: Vec<u64>,
    editor_tile_assets: Vec<u64>,
    editor_first_tile: HashMap<(usize, usize), u64>,
    editor_skin: Option<String>,
    /// achievements screen scroll
    ach_scroll: f32,
    /// home: recently played games
    recent: Option<Vec<GameInfo>>,
    /// carousel: index of the game shown large
    carousel_idx: usize,
    /// the party we host or are in, its screen state, our last presence sent to it, the member
    /// we asked to join (waiting for their server), keyboard used for the host address
    party: Option<party::Party>,
    party_section: usize,
    party_scroll: f32,
    party_headshot_req: HashSet<u64>,
    party_presence_at: f32,
    party_last_presence: Option<(Option<u64>, Option<String>, String)>,
    party_join_wait: Option<(u64, f32)>,
    keyboard_party: bool,
    /// a screen changed a setting that must be saved
    config_dirty: bool,
    /// friends tab: Online / All, list scroll, full body renders (and which were asked for)
    friends_filter: usize,
    friends_scroll: f32,
    friend_renders: HashMap<u64, String>,
    friend_render_req: HashSet<u64>,
    /// debug (--editor N): open the editor previewing outfit N once signed in
    pub debug_editor: Option<usize>,
    /// rig type the avatar API reported (Switch Avatar Type flips it)
    pub avatar_type_r15: bool,
    debug_lock: bool,
    debug_party_done: bool,
    /// place being joined and when its window appeared (Marathon, played games)
    playing: Option<(u64, Option<f32>)>,
}

fn spawn<F: FnOnce() -> Msg + Send + 'static>(tx: &Sender<Msg>, f: F) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
}

fn fmt_num(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn fmt_short(n: i64) -> String {
    match n {
        n if n >= 1_000_000_000 => format!("{:.1}B", n as f64 / 1e9),
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}K", n as f64 / 1e3),
        n => n.to_string(),
    }
}

impl Shell {
    pub fn new(cfg: Config) -> Self {
        let (tx, rx) = channel();
        Shell {
            stack: vec![Screen::Engagement { leaving: None, authing: false }],
            overlay: None,
            modal_out: None,
            sidebar_out: None,
            session: None,
            user: None,
            robux: None,
            fake_robux: 0,
            avatar: None,
            avatar_full: None,
            friends: None,
            headshots: HashMap::new(),
            sorts: None,
            favorites: None,
            icons: HashMap::new(),
            icons_requested: HashSet::new(),
            details: HashMap::new(),
            details_requested: HashSet::new(),
            wearing: None,
            tab: 0,
            tab_t0: 0.0,
            games_scroll: 0.0,
            games_row_scroll: HashMap::new(),
            tx,
            rx,
            remember: cfg.remember_cookie,
            cfg,
            last_tab_focus: None,
            pane_focus: HashMap::new(),
            focus_memory: Vec::new(),
            avatar_items: None,
            wearing_id: None,
            equip_busy: false,
            avatar_fade: (0.0, None),
            outfit_scroll: 0.0,
            catalog_scroll: 0.0,
            vote_busy: false,
            accounts: config::load_accounts(),
            adding_account: false,
            ql_code: None,
            ql_status: String::new(),
            ql_name: None,
            ql_polled: -100.0,
            ql_busy: false,
            ql_user: None,
            profiles_data: HashMap::new(),
            music_ducked: false,
            ach: Achievements::new(),
            avatar_dirty: false,
            avatar3d: false,
            editor_yaw: std::env::var("XBOX_YAW").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            preview_outfit: None,
            preview_anim: None,
            editor_items: HashMap::new(),
            editor_thumbs: HashMap::new(),
            editor_bundles: None,
            bundle_assets: HashMap::new(),
            editor_wearing: None,
            editor_busy: false,
            editor_desc: None,
            editor_desc_dirty: false,
            editor_seq: 0,
            editor_undo: HashMap::new(),
            editor_recent: Vec::new(),
            editor_scales: None,
            editor_scales_dirty: None,
            editor_emotes: Vec::new(),
            editor_toast: None,
            editor_list_scroll: 0.0,
            editor_grid_scroll: 0.0,
            editor_grid_target: 0.0,
            editor_grid_row: None,
            editor_wheel: 0.0,
            editor_drag_x: None,
            editor_tile_ids: Vec::new(),
            editor_tile_assets: Vec::new(),
            editor_first_tile: HashMap::new(),
            editor_skin: None,
            ach_scroll: 0.0,
            recent: None,
            carousel_idx: 0,
            party: None,
            party_section: 0,
            party_scroll: 0.0,
            party_headshot_req: HashSet::new(),
            party_presence_at: -100.0,
            party_last_presence: None,
            party_join_wait: None,
            keyboard_party: false,
            config_dirty: false,
            friends_filter: 0,
            friends_scroll: 0.0,
            friend_renders: HashMap::new(),
            friend_render_req: HashSet::new(),
            debug_editor: None,
            avatar_type_r15: false,
            debug_lock: false,
            debug_party_done: false,
            playing: None,
        }
    }

    /// Debug: fill the hub with placeholder data to check layout without signing in.
    pub fn demo(&mut self, what: &str) {
        self.user = Some(User { id: 1, name: "Builderman".into(), display_name: "Builderman".into() });
        self.robux = Some(1234);
        self.avatar = Some(String::new());
        self.avatar_full = Some(String::new());
        let games: Vec<GameInfo> = (0..24).map(|i| GameInfo { name: format!("Demo Game {}", i + 1), ..Default::default() }).collect();
        self.sorts = Some(vec![
            Sort { id: "top".into(), title: "Top Trending".into(), games: games.clone() },
            Sort { id: "Continue".into(), title: "Continue".into(), games: games[..6].to_vec() },
            Sort { id: "pop".into(), title: "Popular".into(), games: games.clone() },
        ]);
        self.favorites = Some(games[..5].to_vec());
        self.friends = Some(
            (0..20u64)
                .map(|i| Friend {
                    id: i + 1,
                    name: format!("friend{i}"),
                    display_name: format!("Friend {i}"),
                    presence: [2u8, 1, 0][i as usize % 3],
                    location: if i % 3 == 0 { "Natural Disaster Survival".into() } else if i % 3 == 1 { "Website".into() } else { String::new() },
                    place_id: if i % 3 == 0 { Some(189707) } else { None },
                    ..Default::default()
                })
                .collect(),
        );
        self.wearing = Some(vec![]);
        let outfit = |i: u64, owned: bool| AvatarItem { id: i, name: format!("Outfit {i}"), owned, is_bundle: !owned, price: if owned { None } else { Some(i as i64 * 50) }, ..Default::default() };
        self.avatar_items = Some(Ok(((1..5).map(|i| outfit(i, true)).collect(), (10..18).map(|i| outfit(i, false)).collect())));
        self.wearing_id = Some(1);
        self.stack = vec![Screen::Hub];
        let mut parts = what.split(':');
        let kind = parts.next().unwrap_or("");
        let n: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        self.tab_t0 = -10.0;
        match kind {
            "engagement" => self.stack = vec![Screen::Engagement { leaving: None, authing: false }],
            "tab" => self.tab = n.min(TABS.len() - 1),
            "settings" => self.stack.push(Screen::Settings { t0: -10.0 }),
            "profiles" => {
                self.accounts = vec![
                    Account { id: 1, name: "Builderman".into(), display_name: "Builderman".into(), cookie: String::new() },
                    Account { id: 2, name: "AltAccount".into(), display_name: "Alt".into(), cookie: String::new() },
                ];
                self.cfg.default_account = 2;
                self.stack.push(Screen::Profiles { t0: -10.0, scroll: 0.0 })
            }
            "help" => self.stack.push(Screen::Help { t0: -10.0 }),
            "achievements" => {
                self.ach.sign_in(0);
                self.ach.demo();
                self.stack.push(Screen::Achievements { t0: -10.0 })
            }
            "search" => self.stack.push(Screen::Search { query: "natural".into(), t0: -10.0, symbols: false, shift: false, closing: None }),
            "quit" => self.overlay = Some(Overlay::Confirm { title: "Exit".into(), phrase: "Do you want to close the launcher?".into(), image: None, action: Confirm::Quit, t0: -10.0 }),
            "profile" => {
                self.profiles_data.insert(
                    7,
                    Ok(Profile {
                        id: 7,
                        name: "friend0".into(),
                        display_name: "Friend 0".into(),
                        description: "Builder, scripter, and professional obby skipper.".into(),
                        created: "2014-06-02".into(),
                        friends: 187,
                        followers: 1204,
                        following: 33,
                        favorites: (0..6).map(|i| GameInfo { name: format!("Favorite {i}"), ..Default::default() }).collect(),
                        creations: (0..3).map(|i| GameInfo { name: format!("My Game {i}"), ..Default::default() }).collect(),
                        wearing: (0..6).map(|i| (format!("Item {i}"), String::new())).collect(),
                        badges: (0..6).map(|i| (format!("Badge {i}"), String::new())).collect(),
                        ..Default::default()
                    }),
                );
                self.stack.push(Screen::Profile { uid: 7, t0: -10.0, scroll: 0.0, target: 0.0 })
            }
            "loading" => self.stack.push(Screen::Loading { name: "Natural Disaster Survival".into(), creator: "Stickmasterluke".into(), thumb: Some("rbxasset://textures/ui/Shell/Images/Robux/Robux02.png".into()), icon: Some("rbxasset://textures/ui/Shell/Images/Robux/RobuxSquare01.png".into()), t0: -10.0, launched: true, error: None, pids: vec![], window: None, presses: vec![], stop: Arc::new(AtomicBool::new(false)) }),
            "waiting" => self.stack.push(Screen::Loading { name: "Natural Disaster Survival".into(), creator: "Stickmasterluke".into(), thumb: None, icon: None, t0: -10.0, launched: true, error: None, pids: vec![], window: Some(-10.0), presses: vec![], stop: Arc::new(AtomicBool::new(false)) }),
            "account" => self.stack.push(Screen::Account { t0: -10.0 }),
            "overscan" => self.stack.push(Screen::Overscan { edges: Vec2::splat(0.9), shown: Vec2::splat(0.9), last: -1.0 }),
            "social" => self.stack.push(Screen::Social { t0: -10.0, scroll: 0.0 }),
            "signin" => self.stack = vec![Screen::SignIn { cookie: String::new(), busy: false }],
            "quicklogin" => self.stack = vec![Screen::QuickLogin { t0: -10.0 }],
            "error" => self.overlay = Some(Overlay::Error { def: s::feature_not_available(), t0: -10.0 }),
            "balance" => {
                self.tab = 4;
                self.overlay = Some(Overlay::RobuxBalance { t0: -10.0 })
            }
            "sidebar" => {
                self.tab = 3;
                self.overlay = Some(Overlay::SideBar {
                    items: vec![("JOIN GAME".into(), SideAction::ViewGamerCard(1)), ("VIEW GAME DETAILS".into(), SideAction::ViewGamerCard(1)), ("VIEW GAMER CARD".into(), SideAction::ViewGamerCard(1))],
                    t0: -10.0,
                })
            }
            "confirm" => {
                self.tab = 4;
                self.open_buy(400, "4.99", 0, -10.0);
            }
            "detail" => {
                self.details.insert(
                    1,
                    GameDetails {
                        universe_id: 1,
                        name: "Demo Game".into(),
                        description: "A placeholder description used to check the game detail layout.".into(),
                        creator: "Roblox".into(),
                        playing: 1234,
                        visits: 5_600_000,
                        max_players: 12,
                        created: "2016-01-26".into(),
                        updated: "2026-10-28".into(),
                        up_votes: 900,
                        down_votes: 100,
                        is_favorited: true,
                        screenshots: (1..5).map(|i| format!("rbxasset://textures/ui/Shell/Images/Robux/Robux0{i}.png")).collect(),
                        ..Default::default()
                    },
                );
                self.stack.push(Screen::GameDetail { universe: 1, place: 0, t0: -10.0, scroll: 0.0, target: 0.0 });
            }
            _ => {}
        }
    }

    pub fn text_mode(&self) -> bool {
        self.overlay.is_none() && matches!(self.stack.last(), Some(Screen::SignIn { .. }) | Some(Screen::Search { .. }))
    }

    /// Overscan edge fraction applied to the whole UI root (GameOptionsSettings.OverscanPX/PY).
    pub fn overscan(&self) -> Vec2 {
        match self.stack.last() {
            Some(Screen::Overscan { .. }) => Vec2::ONE,
            _ => Vec2::new(self.cfg.overscan[0], self.cfg.overscan[1]),
        }
    }

    fn push(&mut self, ui: &mut Ui, sc: Screen) {
        self.focus_memory.push(ui.focus);
        self.stack.push(sc);
        ui.focus = None;
    }

    fn pop(&mut self, ui: &mut Ui) {
        self.stack.pop();
        ui.focus = self.focus_memory.pop().flatten();
        // the screen we go back to slides and fades in again, as when it was opened
        match self.stack.last_mut() {
            Some(Screen::Hub) => self.tab_t0 = ui.time,
            Some(
                Screen::GameDetail { t0, .. }
                | Screen::Social { t0, .. }
                | Screen::Settings { t0 }
                | Screen::Account { t0 }
                | Screen::Achievements { t0 }
                | Screen::Party { t0 }
                | Screen::AvatarEditor { t0, .. }
                | Screen::Profile { t0, .. }
                | Screen::GameGrid { t0, .. }
                | Screen::Carousel { t0, .. }
                | Screen::Profiles { t0, .. }
                | Screen::Help { t0 },
            ) => *t0 = ui.time,
            _ => {}
        }
    }

    fn error(&mut self, ui: &Ui, audio: &mut Audio, def: ErrDef) {
        // ErrorOverlay:GetOverlaySound
        audio.play("Error", 1.0);
        self.overlay = Some(Overlay::Error { def, t0: ui.time });
    }

    fn close_overlay(&mut self, ui: &Ui) {
        if let Some(Overlay::SideBar { items, .. }) = self.overlay.take() {
            self.sidebar_out = Some((ui.time, items.into_iter().map(|i| i.0).collect()));
        }
        self.modal_out = Some(ui.time);
    }

    fn begin_auth(&mut self, cookie: String) {
        let session = Arc::new(Session::new(&cookie));
        spawn(&self.tx, move || {
            let r = session.authenticated().map_err(|e| e.to_string());
            Msg::Auth(r, session)
        });
    }

    /// The avatar editor is open: main.rs shows its 3D room instead of the menu world.
    pub fn editor_open(&self) -> bool {
        matches!(self.stack.last(), Some(Screen::AvatarEditor { .. }))
    }

    pub fn user_id(&self) -> Option<u64> {
        self.user.as_ref().map(|u| u.id)
    }

    /// Debug (--autosignin): sign in with the default saved account right away.
    pub fn auto_sign_in(&mut self) {
        let default = self.cfg.default_account;
        // XBOX_ACCOUNT=<name> picks another saved account (two launchers side by side for parties)
        let named = std::env::var("XBOX_ACCOUNT").ok().and_then(|n| self.accounts.iter().find(|a| a.name.eq_ignore_ascii_case(&n)).map(|a| a.cookie.clone()));
        if let Some(c) = named.or_else(|| self.accounts.iter().find(|a| a.id == default && default != 0).or(self.accounts.first()).map(|a| a.cookie.clone())) {
            self.begin_auth(c);
        }
    }

    /// Look for badges the account earned since the last check (sign-in, back from a game).
    fn sync_badges(&mut self) {
        let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) else { return };
        let known = self.ach.known_badges();
        spawn(&self.tx, move || Msg::Badges(u.id, s.user_badges(u.id, &known).unwrap_or_default()));
    }

    fn on_signed_in(&mut self, user: User, session: Arc<Session>) {
        self.ach.sign_in(user.id);
        self.avatar_dirty = true;
        self.user = Some(user.clone());
        self.session = Some(session.clone());
        self.sync_badges();
        let uid = user.id;
        let s = session.clone();
        spawn(&self.tx, move || Msg::Robux(s.robux(uid).unwrap_or(-1)));
        let s = session.clone();
        spawn(&self.tx, move || {
            let a = s.avatar_url(uid, "avatar", "352x352").unwrap_or_default();
            let f = s.avatar_url(uid, "avatar", "720x720").unwrap_or_default();
            Msg::Avatar(a, f)
        });
        self.refresh_friends();
        let s = session.clone();
        spawn(&self.tx, move || Msg::Sorts(s.sorts().unwrap_or_default()));
        let s = session.clone();
        spawn(&self.tx, move || Msg::Favorites(s.favorites(uid).unwrap_or_default()));
        let s = session.clone();
        spawn(&self.tx, move || Msg::Recent(s.recently_played().unwrap_or_default()));
        let s = session.clone();
        spawn(&self.tx, move || Msg::AvatarItems(s.avatar_items(uid).map_err(|e| e.to_string())));
        if std::env::var("XBOX_APITEST").is_ok() {
            let s = session.clone();
            std::thread::spawn(move || {
                let r = s.recently_played();
                eprintln!("apitest: recently played {:?}", r.map(|v| v.iter().map(|g| format!("{} {} {}", g.universe_id, g.place_id, g.name)).collect::<Vec<_>>()));
            });
        }
        // debug: XBOX_WEARTEST re-sends exactly what is worn (no change) and prints the result
        if std::env::var("XBOX_WEARTEST").is_ok() {
            let s = session.clone();
            std::thread::spawn(move || {
                let ids = s.currently_wearing(uid).unwrap_or_default();
                eprintln!("weartest: wearing {ids:?}");
                let worn = s.avatar_info(uid).unwrap_or_default();
                let metas: HashMap<u64, serde_json::Value> = worn["assets"].as_array().map(|a| a.iter().filter(|x| x["meta"].is_object()).filter_map(|x| Some((x["id"].as_u64()?, x["meta"].clone()))).collect()).unwrap_or_default();
                eprintln!("weartest: set_wearing -> {:?}", s.set_wearing(&ids, &metas));
                let info = s.avatar_info(uid).unwrap_or_default();
                let r15 = info["playerAvatarType"].as_str() == Some("R15");
                eprintln!("weartest: set_avatar_type({r15}) -> {:?}", s.set_avatar_type(r15));
                let sc = &info["scales"];
                let f = |k: &str| sc[k].as_f64().unwrap_or(1.0) as f32;
                eprintln!("weartest: scales {sc} -> {:?}", s.set_scales(f("height"), f("width"), f("head"), f("bodyType"), f("proportion")));
                let em = s.emotes();
                eprintln!("weartest: emotes {em:?}");
            });
        }
        let s = session;
        spawn(&self.tx, move || {
            let ids = s.currently_wearing(uid).unwrap_or_default();
            let names: HashMap<u64, String> = s.asset_names(&ids).into_iter().collect();
            let thumbs: HashMap<u64, String> = s.asset_thumbs(&ids).unwrap_or_default().into_iter().collect();
            Msg::Wearing(ids.iter().map(|i| (*i, names.get(i).cloned().unwrap_or_default(), thumbs.get(i).cloned().unwrap_or_default())).collect())
        });
    }

    fn refresh_friends(&mut self) {
        let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) else { return };
        let tx = self.tx.clone();
        spawn(&self.tx, move || {
            let friends = s.friends(u.id).unwrap_or_default();
            let ids: Vec<u64> = friends.iter().map(|f| f.id).collect();
            if !ids.is_empty() {
                if let Ok(h) = s.headshots(&ids) {
                    let _ = tx.send(Msg::Headshots(h));
                }
            }
            Msg::Friends(friends)
        });
    }

    fn want_icons(&mut self, ids: impl IntoIterator<Item = u64>) {
        let need: Vec<u64> = ids.into_iter().filter(|i| *i != 0 && self.icons_requested.insert(*i)).collect();
        if need.is_empty() {
            return;
        }
        if let Some(s) = self.session.clone() {
            spawn(&self.tx, move || Msg::Icons(s.game_icons(&need).unwrap_or_default()));
        }
    }

    fn want_details(&mut self, universe: u64) {
        if universe == 0 || !self.details_requested.insert(universe) {
            return;
        }
        if let Some(s) = self.session.clone() {
            let ratings = self.cfg.ui("Age Ratings") == "On";
            spawn(&self.tx, move || {
                let r = s.game_details(universe).map(|mut d| {
                    if ratings {
                        d.age_rating = s.age_rating(universe);
                    }
                    d
                });
                Msg::Details(universe, r.map_err(|e| e.to_string()))
            });
        }
    }

    fn reset_user_data(&mut self) {
        self.robux = None;
        self.fake_robux = 0;
        self.avatar = None;
        self.avatar_full = None;
        self.friends = None;
        self.sorts = None;
        self.favorites = None;
        self.wearing = None;
        self.avatar_items = None;
        self.wearing_id = None;
        self.details.clear();
        self.details_requested.clear();
        self.pane_focus.clear();
    }

    fn ask_quit(&mut self, ui: &Ui, audio: &mut Audio) {
        audio.play("OverlayOpen", 1.0);
        self.overlay = Some(Overlay::Confirm { title: "Exit".into(), phrase: "Do you want to close the launcher?".into(), image: None, action: Confirm::Quit, t0: ui.time });
    }

    fn open_profile(&mut self, ui: &mut Ui, audio: &mut Audio, uid: u64) {
        audio.play("ScreenChange", 1.0);
        if !self.profiles_data.contains_key(&uid) {
            if let Some(sess) = self.session.clone() {
                spawn(&self.tx, move || Msg::Profile(uid, sess.profile(uid).map_err(|e| e.to_string())));
            }
        }
        let t0 = ui.time;
        self.push(ui, Screen::Profile { uid, t0, scroll: 0.0, target: 0.0 });
    }

    fn sign_out(&mut self, ui: &mut Ui) {
        if let Some(u) = &self.user {
            let id = u.id;
            self.accounts.retain(|a| a.id != id);
            config::save_accounts(&self.accounts);
            if self.cfg.default_account == id {
                self.cfg.default_account = self.accounts.first().map(|a| a.id).unwrap_or(0);
            }
        }
        config::forget_cookie();
        self.ach.sign_out();
        self.session = None;
        self.user = None;
        self.robux = None;
        self.fake_robux = 0;
        self.avatar = None;
        self.avatar_full = None;
        self.friends = None;
        self.sorts = None;
        self.favorites = None;
        self.wearing = None;
        self.details.clear();
        self.details_requested.clear();
        self.focus_memory.clear();
        ui.focus = None;
        self.stack = vec![Screen::Engagement { leaving: None, authing: false }];
    }

    /// EventHub "OpenGameDetail"
    fn open_game(&mut self, ui: &mut Ui, audio: &mut Audio, universe: u64, place: u64) {
        audio.play("ScreenChange", 1.0);
        if universe == 0 && place != 0 {
            if let Some(s) = self.session.clone() {
                spawn(&self.tx, move || Msg::PlaceUniverse(place, s.place_to_universe(place).ok()));
            }
        }
        self.want_details(universe);
        self.want_icons([universe]);
        let t0 = ui.time;
        self.push(ui, Screen::GameDetail { universe, place, t0, scroll: 0.0, target: 0.0 });
    }

    fn launch(&mut self, ui: &mut Ui, target: JoinTarget, name: String, creator: String, place: Option<u64>) {
        let Some(s) = self.session.clone() else { return };
        let cfg = self.cfg.clone();
        let t0 = ui.time;
        let stop = Arc::new(AtomicBool::new(false));
        // place 0 = joined a friend whose place wasn't known yet
        self.playing = Some((place.unwrap_or(0), None));
        if matches!(target, JoinTarget::Follow(_)) {
            // StrengthInNumbers: joining a friend stands in for an Xbox party
            self.ach.award(achievements::STRENGTH_IN_NUMBERS);
        }
        let detail = place.and_then(|pid| self.details.values().find(|d| d.place_id == pid));
        let thumb = detail.and_then(|d| d.screenshots.first().cloned());
        let icon = detail.and_then(|d| self.icons.get(&d.universe_id).cloned());
        self.push(ui, Screen::Loading { name: name.clone(), creator: creator.clone(), thumb, icon, t0, launched: false, error: None, pids: Vec::new(), window: None, presses: Vec::new(), stop: stop.clone() });
        // fill in the game name / creator when we only know the place (joining a friend)
        if let (Some(pid), true) = (place, creator.is_empty()) {
            let s2 = s.clone();
            spawn(&self.tx, move || {
                let info = s2.place_to_universe(pid).and_then(|u| s2.game_details(u));
                match info {
                    Ok(d) => Msg::LoadingInfo(d.name, d.creator),
                    Err(_) => Msg::LoadingInfo(String::new(), String::new()),
                }
            });
        }
        let tx = self.tx.clone();
        spawn(&self.tx, move || {
            let before = launch::player_pids();
            launch::allow_foreground();
            let r = launch::launch(&s, &cfg, target).map(|_| name).map_err(|e| e.to_string());
            if r.is_ok() {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let mut seen: Vec<u32> = Vec::new();
                    let mut window_sent = false;
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if stop.load(Ordering::Relaxed) {
                            // cancelled on the loading screen (no game window yet): close the
                            // Roblox that was starting, including one the bootstrapper starts late
                            if !window_sent {
                                let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
                                while std::time::Instant::now() < until {
                                    for p in launch::player_pids().into_iter().filter(|p| !before.contains(p)) {
                                        launch::kill(p);
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(300));
                                }
                            }
                            return;
                        }
                        let now = launch::player_pids();
                        for p in now.iter().copied().filter(|p| !before.contains(p)) {
                            if !seen.contains(&p) {
                                seen.push(p);
                                let _ = tx.send(Msg::GamePids(seen.clone()));
                            }
                        }
                        if seen.is_empty() {
                            continue;
                        }
                        if !window_sent {
                            if let Some(h) = launch::game_window(&seen) {
                                window_sent = true;
                                let _ = tx.send(Msg::GameWindow(h));
                            }
                        }
                        if !now.iter().any(|p| seen.contains(p)) {
                            let _ = tx.send(Msg::GameClosed);
                            return;
                        }
                    }
                });
            }
            Msg::Launched(r)
        });
    }

    fn open_buy(&mut self, amount: i64, price: &str, idx: usize, t0: f32) {
        self.overlay = Some(Overlay::Confirm {
            title: "Confirm Purchase".into(),
            phrase: format!("Are you sure you want to buy \"{} ROBUX\" for ${}?", fmt_num(amount), price),
            image: Some(format!("rbxasset://textures/ui/Shell/Images/Robux/RobuxSquare0{}.png", idx + 1)),
            action: Confirm::BuyRobux(amount),
            t0,
        });
    }

    fn poll(&mut self, ui: &mut Ui, audio: &mut Audio, ev: &mut ShellEvents) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Auth(r, session) => match r {
                    Ok(user) => {
                        if self.remember || !self.adding_account {
                            self.accounts.retain(|a| a.id != user.id && !(a.id == 0 && a.cookie == session.cookie()));
                            self.accounts.push(Account { id: user.id, name: user.name.clone(), display_name: user.display_name.clone(), cookie: session.cookie().to_string() });
                            if self.remember {
                                config::save_accounts(&self.accounts);
                            }
                        }
                        if self.cfg.default_account == 0 {
                            self.cfg.default_account = user.id;
                            ev.config_changed = true;
                        }
                        self.adding_account = false;
                        self.overlay = None;
                        self.reset_user_data();
                        self.on_signed_in(user, session);
                        self.stack = vec![Screen::Hub];
                        self.focus_memory.clear();
                        self.tab = 0;
                        self.tab_t0 = ui.time;
                        ui.focus = Some(id("tab", 0));
                        ev.signed_in = true;
                        audio.play("ScreenChange", 1.0);
                    }
                    Err(e) => {
                        let def = if e.contains("401") {
                            s::auth_error("That .ROBLOSECURITY cookie was rejected. It may have expired or been signed out.")
                        } else if e.contains("HTTP") {
                            s::auth_error(&e)
                        } else {
                            s::web_connect_failed()
                        };
                        match self.stack.last_mut() {
                            Some(Screen::SignIn { busy, .. }) => *busy = false,
                            // a new code for another try
                            Some(Screen::QuickLogin { .. }) => self.ql_reset(),
                            _ => {
                                config::forget_cookie();
                                self.stack = vec![Screen::Engagement { leaving: None, authing: false }, Screen::QuickLogin { t0: ui.time }];
                                self.focus_memory = vec![None];
                                self.ql_reset();
                            }
                        }
                        self.error(ui, audio, def);
                    }
                },
                Msg::QuickCode(r) => {
                    self.ql_busy = false;
                    match r {
                        Ok(c) => {
                            if std::env::var("XBOX_QL_DEBUG").is_ok() {
                                eprintln!("quicklogin code {}", c.code);
                            }
                            self.ql_code = Some(c);
                            self.ql_status = "Created".into();
                            self.ql_name = None;
                            self.ql_polled = ui.time;
                        }
                        Err(e) => {
                            if matches!(self.stack.last(), Some(Screen::QuickLogin { .. })) {
                                self.ql_status = "Error".into();
                                self.ql_name = Some(e);
                            }
                        }
                    }
                }
                Msg::QuickStatus(code, status, name) => {
                    // (answers for a code that was replaced are ignored)
                    let current = self.ql_code.as_ref().is_some_and(|c| c.code == code);
                    if current {
                        self.ql_busy = false;
                    }
                    if current && matches!(self.stack.last(), Some(Screen::QuickLogin { .. })) {
                        if status != self.ql_status && status == "UserLinked" {
                            audio.play("PopUp", 1.0);
                        }
                        match status.as_str() {
                            // expired or turned down on the other device: show a fresh code
                            "Cancelled" => self.ql_reset(),
                            "Validated" => {
                                self.ql_status = status;
                                self.ql_name = name;
                                if let Some(c) = self.ql_code.clone() {
                                    self.ql_busy = true;
                                    spawn(&self.tx, move || Msg::QuickDone(api::quick_login_finish(&c).map_err(|e| e.to_string())));
                                }
                            }
                            _ => {
                                self.ql_status = status;
                                self.ql_name = name;
                            }
                        }
                    }
                }
                Msg::QuickDone(r) => {
                    self.ql_busy = false;
                    match r {
                        Ok((cookie, user)) => {
                            // the code is spent; show who is signing in while the session starts
                            self.ql_code = None;
                            self.ql_status = "SigningIn".into();
                            let uid = user.id;
                            self.ql_user = Some((user, None));
                            spawn(&self.tx, move || Msg::QuickHeadshot(uid, api::headshot_anon(uid).unwrap_or_default()));
                            self.begin_auth(cookie);
                        }
                        Err(e) => {
                            self.ql_reset();
                            self.error(ui, audio, s::auth_error(&e));
                        }
                    }
                }
                Msg::QuickHeadshot(uid, url) => {
                    if let Some((u, pic)) = self.ql_user.as_mut() {
                        if u.id == uid && !url.is_empty() {
                            *pic = Some(url);
                        }
                    }
                }
                Msg::Robux(r) => self.robux = Some(r),
                Msg::Avatar(a, f) => {
                    self.avatar = Some(a);
                    self.avatar_full = Some(f);
                }
                Msg::Friends(f) => self.friends = Some(f),
                Msg::Headshots(h) => self.headshots.extend(h),
                Msg::Sorts(s) => {
                    let ids: Vec<u64> = s.iter().flat_map(|s| s.games.iter().take(24).map(|g| g.universe_id)).collect();
                    self.sorts = Some(s);
                    self.want_icons(ids);
                }
                Msg::Favorites(f) => {
                    let ids: Vec<u64> = f.iter().map(|g| g.universe_id).collect();
                    self.favorites = Some(f);
                    self.want_icons(ids);
                }
                Msg::Icons(i) => self.icons.extend(i),
                Msg::Details(u, r) => match r {
                    Ok(d) => {
                        self.details.insert(u, d);
                    }
                    Err(_) => {
                        self.details_requested.remove(&u);
                        if matches!(self.stack.last(), Some(Screen::GameDetail { universe, .. }) if *universe == u) {
                            self.pop(ui);
                        }
                        self.error(ui, audio, s::default_error());
                    }
                },
                Msg::PlaceUniverse(place, u) => {
                    let mut found = None;
                    for sc in self.stack.iter_mut() {
                        if let Screen::GameDetail { universe, place: p, .. } = sc {
                            if *p == place && *universe == 0 {
                                *universe = u.unwrap_or(0);
                                found = u;
                            }
                        }
                    }
                    match found {
                        Some(u) => {
                            self.want_details(u);
                            self.want_icons([u]);
                        }
                        None => {
                            if matches!(self.stack.last(), Some(Screen::GameDetail { universe: 0, .. })) {
                                self.pop(ui);
                            }
                            self.error(ui, audio, s::default_error());
                        }
                    }
                }
                Msg::Search(q, r) => match r {
                    Ok(games) => {
                        self.want_icons(games.iter().map(|g| g.universe_id).collect::<Vec<_>>());
                        if let Some(Screen::Carousel { query: Some(cq), games: g, .. }) = self.stack.last_mut() {
                            if *cq == q {
                                *g = Some(games);
                            }
                        }
                    }
                    Err(_) => self.error(ui, audio, s::default_error()),
                },
                Msg::Wearing(w) => self.wearing = Some(w),
                Msg::EditorItems(t, r) => {
                    if let Ok(items) = &r {
                        let ids: Vec<u64> = items.iter().map(|i| i.0).filter(|i| !self.editor_thumbs.contains_key(i)).collect();
                        if let Some(s) = self.session.clone() {
                            spawn(&self.tx, move || Msg::EditorThumbs(s.asset_thumbs(&ids).unwrap_or_default()));
                        }
                    }
                    self.editor_items.insert(t, r);
                }
                Msg::EditorThumbs(t) => self.editor_thumbs.extend(t),
                Msg::Badges(uid, list) => {
                    if self.user.as_ref().is_some_and(|u| u.id == uid) {
                        self.ach.add_badges(list);
                    }
                }
                Msg::EditorBundles(r) => {
                    if let (Ok(b), Some(s)) = (&r, self.session.clone()) {
                        // thumbnails, and every bundle's contents so worn ones show as worn
                        let ids: Vec<u64> = b.iter().map(|b| b.0).collect();
                        spawn(&self.tx, move || Msg::EditorThumbs(s.bundle_thumbs(&ids).unwrap_or_default().into_iter().map(|(i, u)| (i | BUNDLE_TAG, u)).collect()));
                        for (bid, _) in b {
                            let (s, bid) = (self.session.clone().unwrap(), *bid);
                            spawn(&self.tx, move || Msg::BundleAssets(bid, s.bundle_assets(bid).unwrap_or_default(), false));
                        }
                    }
                    self.editor_bundles = Some(r);
                }
                Msg::BundleAssets(bid, assets, wear) => {
                    self.bundle_assets.insert(bid, assets);
                    if wear {
                        self.editor_wear_bundle(bid);
                    }
                }
                Msg::EditorWearing(w) => {
                    // Recent starts as what is worn; newly worn items go to the front
                    for a in &w {
                        if !self.editor_recent.contains(a) {
                            self.editor_recent.push(*a);
                        }
                    }
                    let missing: Vec<u64> = w.iter().copied().filter(|a| !self.editor_thumbs.contains_key(a)).collect();
                    if let (Some(s), false) = (self.session.clone(), missing.is_empty()) {
                        spawn(&self.tx, move || Msg::EditorThumbs(s.asset_thumbs(&missing).unwrap_or_default()));
                    }
                    self.editor_wearing = Some(w);
                }
                Msg::EditorEmotes(e) => self.editor_emotes = e,
                Msg::PartyJoined(addr, r) => {
                    self.overlay = None;
                    match r {
                        Ok(p) => {
                            if let Some(old) = self.party.replace(p) {
                                old.leave();
                            }
                            self.party_section = 0;
                            self.party_presence_at = -100.0;
                        }
                        Err(e) => self.error(ui, audio, s::info("Party", &format!("Could not join the party at {addr}.\n{e}"))),
                    }
                }
                Msg::SelfPresence(kind, place, job, location) => {
                    let game = match kind {
                        2 => if location.is_empty() { "In Game".to_string() } else { location },
                        3 => "In Studio".to_string(),
                        _ => "Roblox".to_string(),
                    };
                    let (place, job) = if kind == 2 { (place, job.filter(|j| party::valid_job_id(j))) } else { (None, None) };
                    let now = (place, job.clone(), game.clone());
                    if self.party_last_presence.as_ref() != Some(&now) {
                        self.party_last_presence = Some(now);
                        if let Some(p) = self.party.as_mut() {
                            p.set_presence(place, job, game);
                        }
                    }
                }
                Msg::FriendRender(u, url) => {
                    self.friend_renders.insert(u, url);
                }
                Msg::Recent(g) => {
                    self.want_icons(g.iter().take(12).map(|g| g.universe_id).collect::<Vec<_>>());
                    self.recent = Some(g);
                }
                Msg::EditorDesc(info) => {
                    let sc = &info["scales"];
                    let f = |k: &str, d: f64| sc[k].as_f64().unwrap_or(d) as f32;
                    self.editor_scales = Some([f("height", 1.0), f("width", 1.0), f("head", 1.0), f("bodyType", 0.0), f("proportion", 0.0)]);
                    self.avatar_type_r15 = info["playerAvatarType"].as_str() == Some("R15");
                    if info["assets"].is_array() {
                        self.editor_desc = Some(info);
                    }
                }
                Msg::EditorScales(v, r15) => {
                    self.editor_scales = Some(v);
                    self.avatar_type_r15 = r15;
                }
                Msg::EditorDone(seq, ok, avatar) => {
                    self.editor_busy = false;
                    let undo = self.editor_undo.remove(&seq);
                    if !ok {
                        // only the newest change is undone; older ones were already replaced
                        if seq == self.editor_seq {
                            if let Some(prev) = undo {
                                self.editor_desc = Some(prev);
                                self.editor_desc_dirty = true;
                            }
                            self.editor_refresh_wearing();
                        }
                    }
                    if ok {
                        self.ach.award(achievements::ONE_NAME_MANY_FACES);
                        // without a local description the 3D avatar comes from Roblox again
                        if self.editor_desc.is_none() {
                            self.avatar_dirty = true;
                        }
                        if let Some((a, f)) = avatar {
                            self.avatar_fade = (ui.time, self.avatar_full.clone());
                            self.avatar = Some(a);
                            self.avatar_full = Some(f);
                        }
                        self.editor_refresh_wearing();
                    } else {
                        self.error(ui, audio, s::equip_failed());
                    }
                }
                Msg::Launched(r) => {
                    if r.is_ok() {
                        if let Some((place, _)) = self.playing {
                            self.ach.played_game(place);
                            // FoursCompany: three friends already in that place
                            let here = self.friends.as_ref().map(|f| f.iter().filter(|f| f.place_id == Some(place)).count()).unwrap_or(0);
                            if here >= achievements::FRIENDS_FOR_FOURS_COMPANY {
                                self.ach.award(achievements::FOURS_COMPANY);
                            }
                        }
                    }
                    if let Some(Screen::Loading { launched, error, .. }) = self.stack.last_mut() {
                        match r {
                            Ok(_) => *launched = true,
                            Err(e) => *error = Some(e),
                        }
                    } else if let Err(e) = r {
                        self.error(ui, audio, s::game_join(&e));
                    }
                }
                Msg::Profile(uid, r) => {
                    self.profiles_data.insert(uid, r);
                    if let Some(Ok(p)) = self.profiles_data.get(&uid) {
                        let ids: Vec<u64> = p.favorites.iter().chain(p.creations.iter()).map(|g| g.universe_id).collect();
                        self.want_icons(ids);
                    }
                }
                Msg::GamePids(p) => {
                    if let Some(Screen::Loading { pids, .. }) = self.stack.last_mut() {
                        *pids = p;
                    }
                }
                Msg::GameWindow(h) => {
                    if let Some((_, started)) = self.playing.as_mut() {
                        started.get_or_insert(ui.time);
                    }
                    if let Some(Screen::Loading { window, .. }) = self.stack.last_mut() {
                        if window.is_none() {
                            *window = Some(ui.time);
                            ev.focus_game = Some(h);
                        }
                    }
                }
                Msg::GameClosed => {
                    if let Some((_, Some(t))) = self.playing.take() {
                        self.ach.session_ended(ui.time - t);
                    }
                    // badges earned in that game pop up now
                    self.sync_badges();
                    if matches!(self.stack.last(), Some(Screen::Loading { .. })) {
                        self.pop(ui);
                        ev.focus_self = true;
                    }
                }
                Msg::LoadingInfo(n, c) => {
                    if let Some(Screen::Loading { name, creator, .. }) = self.stack.last_mut() {
                        if !n.is_empty() {
                            *name = n;
                        }
                        *creator = c;
                    }
                }
                Msg::AvatarItems(r) => {
                    self.avatar_items = Some(r);
                    if let Some(n) = self.debug_editor.take() {
                        self.open_editor(ui);
                        let id = self.avatar_items.as_ref().and_then(|r| r.as_ref().ok()).and_then(|(o, _)| o.get(n).map(|o| o.id));
                        self.preview_outfit = id;
                        eprintln!("debug editor: previewing outfit {id:?}");
                        if let Some(Ok((o, _))) = &self.avatar_items {
                            for (i, it) in o.iter().enumerate() {
                                eprintln!("debug outfit {i}: {} ({})", it.name, it.id);
                            }
                        }
                        self.debug_lock = true;
                        self.avatar_dirty = true;
                    }
                }
                Msg::Worn(item, ok, avatar) => {
                    self.equip_busy = false;
                    if ok {
                        self.ach.award(achievements::ONE_NAME_MANY_FACES);
                        self.avatar_dirty = true;
                        self.wearing_id = Some(item);
                        if let Some((a, f)) = avatar {
                            self.avatar_fade = (ui.time, self.avatar_full.clone());
                            self.avatar = Some(a);
                            self.avatar_full = Some(f);
                        }
                    } else {
                        self.error(ui, audio, s::equip_failed());
                    }
                }
                Msg::Voted(u, vote, prev, ok) => {
                    self.vote_busy = false;
                    if ok && vote.is_some() {
                        self.ach.rated_game(u);
                    }
                    if let Some(d) = self.details.get_mut(&u) {
                        if ok {
                            // VoteView:postVoteAsync count bookkeeping
                            match prev {
                                Some(true) => d.up_votes -= 1,
                                Some(false) => d.down_votes -= 1,
                                None => {}
                            }
                            match vote {
                                Some(true) => d.up_votes += 1,
                                Some(false) => d.down_votes += 1,
                                None => {}
                            }
                            d.user_vote = vote;
                        }
                    }
                    if !ok {
                        self.error(ui, audio, s::default_error());
                    }
                }
                Msg::Favorited(u, v, ok) => {
                    if let Some(d) = self.details.get_mut(&u) {
                        d.is_favorited = if ok { v } else { !v };
                    }
                    if !ok {
                        self.error(ui, audio, s::favorite_failed());
                    }
                }
            }
        }
    }

    // >> frame

    /// Configuration preview: show a given hub tab.
    pub fn preview_tab(&mut self, tab: usize, now: f32) {
        if self.tab != tab {
            self.tab = tab;
            self.tab_t0 = now;
        }
    }

    /// Draw the current screen with no input (configuration hub preview).
    pub fn preview_frame(&mut self, ui: &mut Ui, audio: &mut Audio) {
        let idle = Input::default();
        self.frame(ui, &idle, audio);
    }

    pub fn frame(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio) -> ShellEvents {
        let mut ev = ShellEvents { quit: false, focus_self: false, focus_game: None, keep_focus: false, signed_in: false, toggle_fullscreen: false, config_changed: false };
        self.poll(ui, audio, &mut ev);
        // debug: XBOX_PARTY=host or XBOX_PARTY=join:<address> once signed in
        if self.user.is_some() && self.party.is_none() && !self.debug_party_done {
            if let Ok(cmd) = std::env::var("XBOX_PARTY") {
                self.debug_party_done = true;
                match cmd.strip_prefix("join:") {
                    Some(addr) => self.party_join(ui, audio, addr.to_string()),
                    None => self.party_host(ui, audio),
                }
                let t0 = ui.time;
                self.push(ui, Screen::Party { t0 });
            }
        }
        self.party_tick(ui, audio);
        if std::mem::take(&mut self.config_dirty) {
            ev.config_changed = true;
        }

        let blocked = self.overlay.is_some();
        // remembered before navigating so arrow/stick moves count, not only mouse hover
        let had_focus = ui.focus;
        if !blocked {
            ui.navigate(input);
        }
        // the avatar editor's sliders take left / right again this frame if still focused
        ui.lock_x = false;
        // ConfirmPrompt is opened with hideCurrent, so the screen under it is not drawn
        let hide_current = matches!(self.overlay, Some(Overlay::Confirm { .. }));
        match self.stack.last() {
            _ if hide_current => {}
            Some(Screen::Engagement { .. }) => self.engagement(ui, input, audio, blocked),
            Some(Screen::SignIn { .. }) => self.sign_in(ui, input, audio, blocked, &mut ev),
            Some(Screen::QuickLogin { .. }) => self.quick_login(ui, input, audio, blocked, &mut ev),
            Some(Screen::Hub) => self.hub(ui, input, audio, blocked),
            Some(Screen::GameDetail { .. }) => self.game_detail(ui, input, audio, blocked),
            Some(Screen::Social { .. }) => self.social_screen(ui, input, audio, blocked),
            Some(Screen::Settings { .. }) => self.settings(ui, input, audio, blocked, &mut ev),
            Some(Screen::Account { .. }) => self.account(ui, input, audio, blocked, &mut ev),
            Some(Screen::Overscan { .. }) => self.overscan_screen(ui, input, audio, blocked, &mut ev),
            Some(Screen::Search { .. }) => {
                // the system keyboard slides up over the Games tab
                if matches!(self.stack.iter().rev().nth(1), Some(Screen::Hub)) {
                    ui.inert = true;
                    let idle = Input::default();
                    self.hub(ui, &idle, audio, true);
                    ui.inert = false;
                }
                self.search(ui, input, audio, blocked)
            }
            Some(Screen::GameGrid { .. }) => self.game_grid(ui, input, audio, blocked),
            Some(Screen::Carousel { .. }) => self.carousel(ui, input, audio, blocked),
            Some(Screen::Party { .. }) => self.party_screen(ui, input, audio, blocked),
            Some(Screen::Profiles { .. }) => self.profiles(ui, input, audio, blocked, &mut ev),
            Some(Screen::Help { .. }) => self.help(ui, input, audio, blocked),
            Some(Screen::Achievements { .. }) => self.achievements_screen(ui, input, audio, blocked),
            Some(Screen::AvatarEditor { .. }) => self.avatar_editor(ui, input, audio, blocked),
            Some(Screen::Loading { .. }) => self.loading_screen(ui, input, audio, &mut ev),
            Some(Screen::Profile { .. }) => self.profile_screen(ui, input, audio, blocked),
            None => {}
        }
        let loading = self.stack.iter().any(|sc| matches!(sc, Screen::Loading { .. }));
        if loading != self.music_ducked {
            self.music_ducked = loading;
            audio.duck(loading);
        }
        // modal fade-out after an overlay closed
        if let Some(t0) = self.modal_out {
            let k = ((ui.time - t0) / 0.25).min(1.0);
            if k >= 1.0 {
                self.modal_out = None;
            } else {
                ui.rect(Rect::new(-4000.0, -4000.0, 10000.0, 10000.0), [0.0; 3], MODAL_ALPHA * (1.0 - ease_in_out_quad(k)));
            }
        }
        if let Some((t0, items)) = self.sidebar_out.clone() {
            let k = ((ui.time - t0) / 0.25).min(1.0);
            if k >= 1.0 {
                self.sidebar_out = None;
            } else {
                // Hide: InOut Sine
                let slide = 0.5 - 0.5 * (k * std::f32::consts::PI).cos();
                draw_sidebar_panel(ui, &items, false, slide);
            }
        }
        if self.overlay.is_some() {
            if blocked {
                // overlays own selection (GuiService:AddSelectionParent)
                ui.navigate(input);
                self.draw_overlay(ui, input, audio, &mut ev);
            } else {
                // opened this frame: the press that opened it must not also activate it
                ui.activated = false;
                let idle = Input::default();
                self.draw_overlay(ui, &idle, audio, &mut ev);
            }
        }
        if ui.moved && had_focus != ui.focus {
            let on_keyboard = self.overlay.is_none() && matches!(self.stack.last(), Some(Screen::Search { .. }));
            audio.play(if on_keyboard { "KeyboardMove" } else { "MoveSelection" }, 1.0);
        }
        self.ach.draw(ui, audio);
        if input.toggle_fullscreen {
            ev.toggle_fullscreen = true;
        }
        ev
    }

    fn fade(&self, ui: &mut Ui, t0: f32) -> Vec2 {
        // ScreenManager:DefaultFadeIn — slide in from +15% width and fade in over 0.5s (ease-out)
        let k = ease_out_quad((ui.time - t0) / 0.5);
        ui.alpha = k;
        ui.push_translate(Vec2::new(0.15 * crate::ui::W * (1.0 - k), 0.0))
    }

    fn end_fade(&self, ui: &mut Ui, old: Vec2) {
        ui.pop_translate(old);
        ui.alpha = 1.0;
    }

    fn loading(&self, ui: &mut Ui, at: Vec2) {
        // LoadingWidget: 100x100 spinner, 360 deg/s
        ui.image_rotated(SPINNER, at, Vec2::new(100.0, 100.0), ui.time * std::f32::consts::TAU, 1.0);
    }

    // >> engagement screen

    fn engagement(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let (splash, _, h) = theme_art(self.cfg.ui("Theme"));
        let logo = Rect::new(960.0 - 297.0, 540.0 - h / 2.0, 594.0, h);
        if !ui.image(splash, logo, WHITE, 1.0) && ui.image_failed(splash) {
            ui.image("rbxasset://textures/ui/Shell/Icons/ROBLOXSplashLogo.png", Rect::new(960.0 - 297.0, 540.0 - 99.5, 594.0, 199.0), WHITE, 1.0);
        }
        // v1.0 and older say "Press Any Button To Begin" (Engagement Change.lua)
        let modern = self.cfg.ui("Version") >= "v2.0";
        let Some(Screen::Engagement { leaving, authing }) = self.stack.last() else { return };
        let (leaving, authing) = (*leaving, *authing);
        let hint_y = logo.y + 415.0;
        if authing {
            ui.image_rotated(SPINNER, Vec2::new(960.0, hint_y), Vec2::new(100.0, 100.0), ui.time * std::f32::consts::TAU, 1.0);
            return;
        }
        let alpha = match leaving {
            Some(t0) => 1.0 - ease_out_quad((ui.time - t0) / 0.25),
            None => 1.0,
        };
        if !blocked && leaving.is_none() && input.has(Btn::B) {
            self.ask_quit(ui, audio);
            return;
        }
        if !blocked && leaving.is_none() && input.has(Btn::X) && !self.accounts.is_empty() {
            // "Switch Account": pick which saved account to sign in with
            let t0 = ui.time;
            self.push(ui, Screen::Profiles { t0, scroll: 0.0 });
            audio.play("ScreenChange", 1.0);
            return;
        }
        let any = [Btn::A, Btn::Y, Btn::Start, Btn::LB, Btn::RB];
        let held = !blocked && input.pressed.iter().any(|b| any.contains(b));
        // "Press (A) to Begin"
        let col = if held { GREY_TEXT } else { WHITE };
        if !modern {
            ui.label("Press Any Button To Begin", Font::Regular, BUTTON, Rect::new(0.0, hint_y - 20.0, 1920.0, 40.0), Align::Center, Align::Center, col, alpha);
        }
        let (w1, w2) = (ui.measure("Press ", Font::Regular, BUTTON), ui.measure(" to Begin", Font::Regular, BUTTON));
        let total = w1 + 48.0 + w2;
        let x0 = 960.0 - total * 0.5;
        if modern {
            ui.label("Press ", Font::Regular, BUTTON, Rect::new(x0, hint_y - 20.0, w1, 40.0), Align::Start, Align::Center, col, alpha);
            ui.image(&glyph('A'), Rect::new(x0 + w1 + 2.0, hint_y - 22.0, 44.0, 44.0), WHITE, alpha);
            ui.label(" to Begin", Font::Regular, BUTTON, Rect::new(x0 + w1 + 48.0, hint_y - 20.0, w2, 40.0), Align::Start, Align::Center, col, alpha);
        }
        if !self.accounts.is_empty() {
            hint(ui, &glyph('X'), "SWITCH ACCOUNT");
        }
        if !blocked && leaving.is_none() && (held || input.clicked) {
            audio.play("ButtonPress", 1.0);
            if let Some(Screen::Engagement { leaving, .. }) = self.stack.last_mut() {
                *leaving = Some(ui.time);
            }
        }
        if let Some(t0) = leaving {
            if ui.time - t0 >= 0.25 {
                let default = self.cfg.default_account;
                let pick = self.accounts.iter().find(|a| a.id == default && default != 0).or(self.accounts.first()).map(|a| a.cookie.clone());
                if let Some(Screen::Engagement { leaving, authing }) = self.stack.last_mut() {
                    *leaving = None;
                    *authing = pick.is_some();
                }
                match pick {
                    Some(c) => self.begin_auth(c),
                    None => {
                        let t0 = ui.time;
                        self.push(ui, Screen::QuickLogin { t0 });
                        audio.play("ScreenChange", 1.0);
                    }
                }
            }
        }
    }

    // >> sign in screen
    // (cookie instead of Xbox linking)

    fn sign_in(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        let sa = SAFE;
        let remember = self.remember;
        let Some(Screen::SignIn { cookie, busy }) = self.stack.last_mut() else { return };
        if !blocked && !*busy {
            if input.paste {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if let Ok(t) = cb.get_text() {
                        *cookie = t.trim().to_string();
                    }
                }
            }
            cookie.push_str(input.typed.trim());
            if input.backspace {
                cookie.pop();
            }
        }
        let (cookie_len, busy_now) = (cookie.len(), *busy);

        back_and_title(ui, "Sign In");
        ui.paragraph(
            &format!("Paste your .ROBLOSECURITY cookie to sign in. It is only ever sent to {}. If Remember Me is on it is saved encrypted for your Windows account.", crate::config::domain()),
            Font::Light,
            TITLE,
            Rect::new(sa.x + 16.0, sa.y + 238.0, 1100.0, 200.0),
            WHITE,
            1.0,
            4,
        );
        let field = Rect::new(sa.x + 16.0, sa.y + 460.0, 1100.0, 72.0);
        let fid = id("signin", 0);
        let ffocus = ui.selectable(fid, field);
        if !ui.focus_valid() {
            ui.set_focus(fid);
        }
        ui.rect(field, WHITE, if ffocus { TEXTBOX_SELECTED_ALPHA } else { TEXTBOX_DEFAULT_ALPHA });
        let shown = if cookie_len == 0 { "Press Ctrl+V to paste".to_string() } else { format!("{}  ({} characters)", "\u{2022}".repeat(cookie_len.min(36)), cookie_len) };
        ui.label(&shown, Font::Regular, BUTTON, field.inset(18.0), Align::Start, Align::Center, if cookie_len == 0 { LIGHT_GREY_TEXT } else { WHITE }, 1.0);
        if ffocus {
            ui.selection_box(field, 7.0);
        }
        let y = sa.y + 572.0;
        let sign = button(ui, id("signin", 1), Rect::new(sa.x + 16.0, y, 320.0, 66.0), if busy_now { "SIGNING IN..." } else { "SIGN IN" }, true);
        let paste = button(ui, id("signin", 2), Rect::new(sa.x + 356.0, y, 320.0, 66.0), "PASTE", false);
        let rem = button(ui, id("signin", 3), Rect::new(sa.x + 696.0, y, 420.0, 66.0), if remember { "REMEMBER ME: ON" } else { "REMEMBER ME: OFF" }, false);
        if busy_now {
            self.loading(ui, Vec2::new(sa.x + 1200.0, y + 33.0));
        }
        if blocked || busy_now {
            return;
        }
        let Some(Screen::SignIn { cookie, busy }) = self.stack.last_mut() else { return };
        if paste {
            audio.play("ButtonPress", 1.0);
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(t) = cb.get_text() {
                    *cookie = t.trim().to_string();
                }
            }
        } else if rem {
            audio.play("ButtonPress", 1.0);
            self.remember = !self.remember;
            self.cfg.remember_cookie = self.remember;
            ev.config_changed = true;
        } else if (sign || (ffocus && input.has(Btn::A))) && !cookie.is_empty() {
            audio.play("ButtonPress", 1.0);
            *busy = true;
            let c = cookie.clone();
            self.begin_auth(c);
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    /// Drop the current Quick Login code (cancelled on Roblox's side) so a new one is made.
    fn ql_reset(&mut self) {
        if let Some(c) = self.ql_code.take() {
            spawn(&self.tx, move || {
                api::quick_login_cancel(&c.code);
                Msg::QuickStatus(String::new(), String::new(), None)
            });
        }
        self.ql_status.clear();
        self.ql_name = None;
        self.ql_user = None;
        self.ql_busy = false;
    }

    /// Quick Login, the main way in: enter the code (or scan the QR) on a phone or computer that
    /// is signed in to Roblox and approve it there. Y switches to pasting a .ROBLOSECURITY.
    fn quick_login(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        let Some(Screen::QuickLogin { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let sa = SAFE;
        // keep a live code: make one, then ask how it is doing every 3 seconds
        if !self.ql_busy && self.ql_user.is_none() && self.ql_status != "Error" {
            if self.ql_code.is_none() {
                self.ql_busy = true;
                spawn(&self.tx, move || Msg::QuickCode(api::quick_login_create().map_err(|e| e.to_string())));
            } else if ui.time - self.ql_polled >= 3.0 && self.ql_status != "Validated" {
                self.ql_polled = ui.time;
                self.ql_busy = true;
                let c = self.ql_code.clone().unwrap();
                spawn(&self.tx, move || match api::quick_login_status(&c) {
                    Ok((st, name)) => Msg::QuickStatus(c.code, st, name),
                    Err(_) => Msg::QuickStatus(c.code, "Created".into(), None),
                });
            }
        }
        let old = self.fade(ui, t0);
        back_and_title(ui, "Sign In");
        let x0 = sa.x + 16.0;
        let site = crate::config::domain();
        let status = self.ql_status.clone();
        if let Some((user, pic)) = self.ql_user.clone() {
            // approved: who is signing in
            let c = Vec2::new(960.0, 470.0);
            ui.image(CIRCLE_IMG, Rect::new(c.x - 130.0, c.y - 130.0, 260.0, 260.0), [0.2, 0.3, 0.38], 1.0);
            if let Some(p) = &pic {
                ui.image(p, Rect::new(c.x - 120.0, c.y - 120.0, 240.0, 240.0), WHITE, 1.0);
            }
            let name = if user.display_name.is_empty() { user.name.clone() } else { user.display_name.clone() };
            ui.label(&format!("Signing in as {name}"), Font::Light, HEADER, Rect::new(0.0, c.y + 150.0, 1920.0, 60.0), Align::Center, Align::Center, WHITE, 1.0);
            ui.label(&format!("@{}", user.name), Font::Regular, TITLE, Rect::new(0.0, c.y + 214.0, 1920.0, 40.0), Align::Center, Align::Center, LIGHT_GREY_TEXT, 1.0);
            self.loading(ui, Vec2::new(960.0, c.y + 300.0));
            self.end_fade(ui, old);
            return;
        }
        ui.paragraph(
            &format!("On a phone or computer that is signed in to Roblox, go to {site}/crossdevicelogin and enter this code, or scan the QR code with your phone's camera."),
            Font::Light,
            TITLE,
            Rect::new(x0, sa.y + 200.0, 980.0, 140.0),
            WHITE,
            1.0,
            3,
        );
        // the code, one box per character
        let code = self.ql_code.as_ref().map(|c| c.code.clone()).unwrap_or_default();
        let (bw, gap) = (118.0, 18.0);
        for (i, ch) in code.chars().enumerate() {
            let r = Rect::new(x0 + i as f32 * (bw + gap), sa.y + 370.0, bw, 150.0);
            ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], WHITE, 0.14);
            ui.label(&ch.to_string(), Font::Regular, 96.0, r, Align::Center, Align::Center, WHITE, 1.0);
        }
        let qr = Rect::new(sa.x + sa.w - 420.0, sa.y + 200.0, 400.0, 400.0);
        match status.as_str() {
            "UserLinked" | "Validated" => {
                // a player entered the code: who it is (Roblox only shows the first letters)
                ui.nine_slice(BUTTON_IMG, qr, [8.0, 8.0, 24.0, 24.0], [0.12, 0.2, 0.26], 0.95);
                let c = Vec2::new(qr.center().x, qr.y + 140.0);
                ui.image(CIRCLE_IMG, Rect::new(c.x - 90.0, c.y - 90.0, 180.0, 180.0), [0.3, 0.42, 0.5], 1.0);
                ui.image("rbxasset://glyphs/editor/Body.png", Rect::new(c.x - 48.0, c.y - 48.0, 96.0, 96.0), WHITE, 0.9);
                let name = self.ql_name.clone().unwrap_or_default();
                ui.label(&name, Font::Regular, TITLE, Rect::new(qr.x, c.y + 110.0, qr.w, 44.0), Align::Center, Align::Center, WHITE, 1.0);
                let line = if status == "Validated" { "Approved, signing in..." } else { "Approve the sign-in on your device" };
                ui.label(line, Font::Light, BUTTON, Rect::new(qr.x, c.y + 160.0, qr.w, 40.0), Align::Center, Align::Center, LIGHT_GREY_TEXT, 1.0);
            }
            "Error" => {
                let msg = self.ql_name.clone().unwrap_or_default();
                ui.paragraph(&format!("Quick Login could not start: {msg}\nPress Y to sign in with a .ROBLOSECURITY cookie instead."), Font::Light, BUTTON, qr, LIGHT_GREY_TEXT, 1.0, 6);
            }
            _ => match self.ql_code.as_ref().map(|c| c.qr_url.clone()).filter(|u| !u.is_empty()) {
                Some(url) => {
                    ui.rect(qr, WHITE, 1.0);
                    ui.image(&url, qr.inset(16.0), WHITE, 1.0);
                }
                None => self.loading(ui, qr.center()),
            },
        }
        let remember = self.remember;
        let y = sa.y + 590.0;
        let rem = button(ui, id("quicklogin", 0), Rect::new(x0, y, 420.0, 66.0), if remember { "REMEMBER ME: ON" } else { "REMEMBER ME: OFF" }, false);
        if !ui.focus_valid() {
            ui.set_focus(id("quicklogin", 0));
        }
        hint(ui, &glyph('Y'), "USE COOKIE");
        self.end_fade(ui, old);
        if blocked {
            return;
        }
        if rem {
            audio.play("ButtonPress", 1.0);
            self.remember = !self.remember;
            self.cfg.remember_cookie = self.remember;
            ev.config_changed = true;
        } else if input.has(Btn::Y) {
            // the .ROBLOSECURITY screen, on top of this one
            audio.play("ScreenChange", 1.0);
            self.push(ui, Screen::SignIn { cookie: String::new(), busy: false });
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.ql_reset();
            self.pop(ui);
        }
    }

    // >> app hub

    fn hub(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let sa = SAFE;
        let (_, small, _) = theme_art(self.cfg.ui("Theme"));
        if !ui.image(small, Rect::new(sa.x, sa.y, 232.0, 56.0), WHITE, 1.0) && ui.image_failed(small) {
            ui.image("rbxasset://textures/ui/Shell/Icons/ROBLOXLogoSmall@1080.png", Rect::new(sa.x, sa.y, 232.0, 56.0), WHITE, 1.0);
        }

        // TabDock: items sized to text + 20, 30px apart
        let mut x = sa.x;
        let mut tab_rects = Vec::new();
        let tab_names: Vec<String> = TABS.iter().map(|t| s::word(t)).collect();
        let shown = self.visible_tabs();
        if !shown.contains(&self.tab) {
            self.tab = 0;
        }
        for (i, name) in tab_names.iter().enumerate() {
            if !shown.contains(&i) {
                tab_rects.push(Rect::new(x, sa.y + TAB_Y, 0.0, 36.0));
                continue;
            }
            let w = ui.measure(name, Font::Light, HEADER) + 20.0;
            if i > 0 {
                x += 30.0;
            }
            tab_rects.push(Rect::new(x, sa.y + TAB_Y, w, 36.0));
            x += w;
        }
        let mut tab_focused = None;
        for (i, r) in tab_rects.iter().enumerate() {
            if shown.contains(&i) && ui.selectable(id("tab", i as u64), *r) {
                tab_focused = Some(i);
            }
        }
        if !blocked && !ui.focus_valid() && ui.selectable_count() > 0 {
            ui.set_focus(self.last_tab_focus.unwrap_or(id("tab", self.tab as u64)));
        }
        if let Some(i) = tab_focused {
            self.last_tab_focus = Some(id("tab", i as u64));
            if i != self.tab {
                self.tab = i;
                self.tab_t0 = ui.time;
                audio.play("ScreenChange", 1.0);
            }
        }
        if !blocked {
            let mut new_tab = None;
            if input.has(Btn::LB) || input.has(Btn::RB) {
                if tab_focused.is_none() {
                    // CycleTabDock: first bumper press returns focus to the dock
                    ui.set_focus(id("tab", self.tab as u64));
                } else {
                    let k = shown.iter().position(|t| *t == self.tab).unwrap_or(0);
                    if input.has(Btn::LB) && k > 0 {
                        new_tab = Some(shown[k - 1]);
                    } else if input.has(Btn::RB) && k + 1 < shown.len() {
                        new_tab = Some(shown[k + 1]);
                    }
                }
            }
            if let Some(t) = new_tab {
                self.tab = t;
                self.tab_t0 = ui.time;
                ui.set_focus(id("tab", t as u64));
                audio.play("ScreenChange", 1.0);
            }
            if input.has(Btn::B) && tab_focused.is_none() {
                ui.set_focus(id("tab", self.tab as u64));
            } else if input.has(Btn::B) {
                self.ask_quit(ui, audio);
            }
        }
        let a_down = input.pressed.contains(&Btn::A);
        for (i, (name, r)) in tab_names.iter().zip(tab_rects.iter()).enumerate() {
            if !shown.contains(&i) {
                continue;
            }
            let col = if i == self.tab { WHITE } else { BLUE_TEXT };
            // TabDockItem:OnClick swaps to the smaller text while A is held
            let size = if tab_focused == Some(i) && a_down { MEDIUM } else { HEADER };
            ui.label(name, Font::Light, size, *r, Align::Center, Align::Center, col, 1.0);
            if tab_focused == Some(i) {
                ui.rect(Rect::new(r.x, r.y + r.h + 14.0, r.w, 4.0), TAB_UNDERLINE, 1.0);
            }
        }
        if let Some(i) = tab_focused {
            if !blocked && ui.pressed(id("tab", i as u64)) {
                audio.play("ButtonPress", 1.0);
                if let Some(f) = self.pane_focus.get(&i).copied() {
                    ui.set_focus(f);
                }
            }
        }

        let pane = Rect::new(sa.x, sa.y + PANE_Y, sa.w, 0.786 * 1001.0);
        let old = self.fade(ui, self.tab_t0);
        match self.tab {
            0 => self.home_pane(ui, audio, pane, blocked),
            1 => self.avatar_pane(ui, audio, pane, blocked),
            2 => self.games_pane(ui, audio, pane, blocked),
            3 => self.social_pane(ui, audio, pane, blocked),
            _ => self.store_pane(ui, audio, pane, blocked),
        }
        self.end_fade(ui, old);
        if let Some(f) = ui.focus {
            if tab_focused.is_none() {
                self.pane_focus.insert(self.tab, f);
            }
        }

        // HintActionView (X), bottom right
        let hint_x = match self.tab {
            0 => Some("SETTINGS"),
            2 if self.cfg.ui("Search") != "Off" || self.cfg.ui("Version") >= "v2.0" => Some("SEARCH"),
            3 => Some(if self.party.is_some() { "SHOW PARTY" } else { "START PARTY" }),
            _ => None,
        };
        if let Some(text) = hint_x {
            hint(ui, &glyph('X'), &s::word(text));
            if !blocked && input.has(Btn::X) {
                match self.tab {
                    0 => {
                        let t0 = ui.time;
                        self.push(ui, Screen::Settings { t0 });
                        audio.play("ScreenChange", 1.0);
                    }
                    2 => {
                        let t0 = ui.time;
                        self.keyboard_party = false;
                        self.push(ui, Screen::Search { query: String::new(), t0, symbols: false, shift: false, closing: None });
                        audio.play("OverlayOpen", 1.0);
                    }
                    // onOpenPartyUI: the LAN / IP party
                    _ => {
                        let t0 = ui.time;
                        self.push(ui, Screen::Party { t0 });
                        audio.play("ScreenChange", 1.0);
                    }
                }
            }
        }
    }

    fn game_tile(&mut self, ui: &mut Ui, sel: u64, r: Rect, g: Option<&GameInfo>) -> bool {
        let focused = ui.selectable(sel, r);
        ui.rect(r, [0.0, 0.0, 0.0], 1.0);
        match g {
            Some(g) => {
                let loaded = self.icons.get(&g.universe_id).map(|u| ui.image(&u.clone(), r, WHITE, 1.0)).unwrap_or(false);
                if !loaded {
                    ui.rect(r, GREY_BUTTON, 1.0);
                    let k = (r.w * 0.44).min(102.0);
                    ui.image("rbxasset://textures/ui/Shell/Icons/NoGameImageIcon.png", Rect::new(r.x + (r.w - k) * 0.5, r.y + (r.h - k) * 0.5, k, k), WHITE, 1.0);
                }
                popup_text(ui, sel, r, &g.name);
            }
            None => {
                ui.rect(r, GREY_BUTTON, 1.0);
                ui.image("rbxasset://textures/ui/Shell/Icons/GamePlusIcon.png", Rect::new(r.x + r.w * 0.5 - 51.0, r.y + r.h * 0.5 - 51.0, 102.0, 102.0), WHITE, 1.0);
            }
        }
        if focused {
            ui.selection_box(r, 7.0);
        }
        focused
    }

    // >> home pane

    fn home_pane(&mut self, ui: &mut Ui, audio: &mut Audio, p: Rect, blocked: bool) {
        // ProfileContainer 450x343: name label, then a 450x300 avatar button anchored to the bottom
        let name = self.user.as_ref().map(|u| u.display_name.to_uppercase()).unwrap_or_default();
        ui.label(&name, Font::Regular, SUBHEADER, Rect::new(p.x, p.y, 450.0, 38.0), Align::Start, Align::Start, WHITE, 1.0);
        let pb = Rect::new(p.x, p.y + 343.0 - 6.0 - 300.0, 450.0, 300.0);
        let pid = id("home", 1);
        let pf = ui.selectable(pid, pb);
        ui.nine_slice(SHADOW_IMG, Rect::new(pb.x, pb.y, pb.w + 3.0, pb.h + 3.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
        ui.rect(pb, CHARACTER_BG, 1.0);
        match self.avatar.clone() {
            Some(a) if !a.is_empty() => {
                // ImageRectSize crops the 352 thumbnail to the 450:300 aspect from the top
                ui.image_uv(&a, pb, [0.0, 0.0, 1.0, 300.0 / 450.0], 1.0);
            }
            Some(_) => {}
            None => self.loading(ui, pb.center()),
        }
        if pf {
            ui.selection_box(pb, 7.0);
        }
        if !blocked && ui.pressed(pid) {
            // NavigateToEquippedAvatar
            audio.play("ButtonPress", 1.0);
            self.tab = 1;
            self.tab_t0 = ui.time;
            ui.set_focus(id("tab", 1));
        }

        // FriendActivity: online friends, 446x114 cells with 26 spacing
        let fa = Rect::new(p.x, p.y + 343.0, 450.0, p.h - 343.0);
        ui.label("FRIEND ACTIVITY", Font::Regular, SUBHEADER, Rect::new(fa.x, fa.y + 10.0, 450.0, 50.0), Align::Start, Align::Center, WHITE, 1.0);
        match self.online_friends() {
            None => self.loading(ui, Vec2::new(fa.x + 225.0, fa.y + 200.0)),
            Some(online) => {
                if online.is_empty() {
                    ui.paragraph(s::NO_FRIENDS_ONLINE, Font::Bold, DESCRIPTION, Rect::new(fa.x + fa.w * 0.05, fa.y + 125.0, fa.w * 0.9, 200.0), GREY_TEXT, 0.5, 4);
                }
                // two at a time; the list scrolls through the rest
                let list = Rect::new(fa.x, fa.y + 60.0, 450.0, 2.0 * 140.0 - 26.0);
                let focused = ui.focus.and_then(|fid| online.iter().position(|fr| id("homefriend", fr.id) == fid));
                let scroll = focused.map(|i| ((i as f32 + 1.0) * 140.0 - list.h).max(0.0)).unwrap_or(0.0);
                ui.push_clip(Rect::new(list.x - 20.0, list.y - 10.0, list.w + 40.0, list.h + 16.0));
                for (i, fr) in online.iter().enumerate() {
                    let r = Rect::new(list.x, list.y + i as f32 * 140.0 - scroll, 446.0, 114.0);
                    if r.y > list.y + list.h + 140.0 {
                        break;
                    }
                    self.friend_item(ui, audio, id("homefriend", fr.id), r, fr, blocked);
                }
                ui.pop_clip();
            }
        }

        // Sorts: FAVORITES, RECENTLY PLAYED and FEATURED side by side, each one 378 tile over two
        // 186s with a "..." button underneath that opens it as a carousel. Nothing scrolls.
        if self.sorts.is_none() {
            self.loading(ui, Vec2::new(p.x + 1100.0, p.y + 300.0));
            return;
        }
        let favs = self.favorites.clone().unwrap_or_default();
        let featured: Vec<GameInfo> = self.sorts.as_ref().and_then(|v| v.first()).map(|s| s.games.clone()).unwrap_or_default();
        let recent = self.recent.clone().unwrap_or_default();
        let ty = p.y + 37.0;
        let col_w = 378.0;
        let step = ((p.w - 486.0 - col_w) / 2.0).max(col_w + 20.0);
        let mut open = None;
        let mut more = None;
        for (gi, (title, games)) in [("FAVORITES", &favs), ("RECENTLY PLAYED", &recent), ("FEATURED", &featured)].into_iter().enumerate() {
            let x0 = p.x + 486.0 + gi as f32 * step;
            ui.label(title, Font::Regular, SUBHEADER, Rect::new(x0, p.y, col_w, 38.0), Align::Start, Align::Start, WHITE, 1.0);
            let rects = [Rect::new(x0, ty, col_w, col_w), Rect::new(x0, ty + col_w + 7.0, 186.0, 186.0), Rect::new(x0 + col_w - 186.0, ty + col_w + 7.0, 186.0, 186.0)];
            for (k, r) in rects.iter().enumerate() {
                let sel = id("homegrid", (gi * 100 + k) as u64);
                self.game_tile(ui, sel, *r, games.get(k));
                if !blocked && ui.pressed(sel) {
                    open = games.get(k).map(|g| (g.universe_id, g.place_id));
                }
            }
            let r = Rect::new(x0 + col_w - 100.0, ty + col_w + 7.0 + 186.0 + 14.0, 100.0, 40.0);
            let sel = id("homemore", gi as u64);
            let f = ui.selectable(sel, r);
            ui.rect(r, if f { WHITE } else { GREY_BUTTON }, 1.0);
            for d in 0..3 {
                ui.rect(Rect::new(r.x + 30.0 + d as f32 * 17.0, r.y + 16.0, 8.0, 8.0), if f { TEXT_SELECTED } else { WHITE }, 1.0);
            }
            if f {
                ui.selection_box(r, 4.0);
            }
            if !blocked && ui.pressed(sel) {
                more = Some(gi);
            }
        }
        if let Some((u, pl)) = open {
            audio.play("ButtonPress", 1.0);
            self.open_game(ui, audio, u, pl);
        } else if let Some(k) = more {
            let (title, games) = match k {
                0 => ("Favorites", favs),
                1 => ("Recently Played", recent),
                _ => ("Featured", featured),
            };
            self.open_carousel(ui, audio, title, "HOME", games);
        }
    }

    /// FriendPresenceItem (446x114) — click opens the friends SideBar.
    fn friend_item(&mut self, ui: &mut Ui, audio: &mut Audio, sel: u64, r: Rect, f: &Friend, blocked: bool) {
        let focused = ui.selectable(sel, r);
        if focused {
            ui.rect(r, WHITE, AVATAR_BOX_SELECTED_ALPHA);
        }
        let av = Rect::new(r.x, r.y, 104.0, 104.0);
        ui.nine_slice(SHADOW_IMG, Rect::new(av.x, av.y, av.w + 3.0, av.h + 3.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
        ui.rect(av, CHARACTER_BG, 1.0);
        match self.headshots.get(&f.id).cloned() {
            Some(h) => {
                ui.image(&h, av, WHITE, 1.0);
            }
            None if !self.headshots.is_empty() => {
                ui.image("rbxasset://textures/ui/Shell/Icons/DefaultProfileIcon.png", Rect::new(av.x + 26.0, av.y + 26.0, 52.0, 52.0), WHITE, 1.0);
            }
            None => {}
        }
        let tx = r.x + 104.0 + 12.0;
        let tw = r.w - 104.0 - 12.0;
        ui.push_clip(Rect::new(tx, r.y, tw, 104.0));
        // name row (display name, or the username when there is none), Roblox username row, status row
        let username = if f.name.is_empty() { format!("User {}", f.id) } else { f.name.clone() };
        let top = if f.display_name.is_empty() { username.clone() } else { f.display_name.clone() };
        let rows_h = 24.0 + 12.0 + 24.0 + 7.0 + 20.0;
        let mut y = r.y + (104.0 - rows_h) * 0.5;
        ui.label(&top, Font::Regular, BUTTON, Rect::new(tx, y, tw, 24.0), Align::Start, Align::Center, WHITE, 1.0);
        y += 36.0;
        ui.image(roblox_icon(&self.cfg), Rect::new(tx, y, 24.0, 24.0), WHITE, 1.0);
        ui.label(&username, Font::Regular, BUTTON, Rect::new(tx + 34.0, y + 8.0, tw, 0.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        y += 31.0;
        // Windows path: setPresence(LastLocation, PlaceId ~= nil)
        if f.presence > 0 {
            let (status, col) = presence_status(f);
            ui.image(CIRCLE_IMG, Rect::new(tx + 2.0, y + 1.0, 18.0, 18.0), col, 1.0);
            ui.label(&status, Font::Regular, DESCRIPTION, Rect::new(tx + 33.0, y + 9.0, tw, 0.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
        ui.pop_clip();
        ui.rect(Rect::new(tx, r.y + r.h - 2.0, tw, 2.0), LINE_BREAK, 1.0);
        if focused {
            ui.selection_box(r, 7.0);
        }
        if !blocked && ui.pressed(sel) {
            audio.play("ButtonPress", 1.0);
            let mut items = Vec::new();
            let name = if f.display_name.is_empty() { f.name.clone() } else { f.display_name.clone() };
            if let Some(place) = f.place_id {
                items.push(("JOIN GAME".to_string(), SideAction::JoinGame(f.id, name)));
                items.push(("VIEW GAME DETAILS".to_string(), SideAction::ViewDetails(place)));
            }
            items.push(("VIEW GAMER CARD".to_string(), SideAction::ViewGamerCard(f.id)));
            self.overlay = Some(Overlay::SideBar { items, t0: ui.time });
            ui.set_focus(id("sidebar", 0));
            audio.play("SideMenuSlideIn", 1.0);
        }
    }

    // >> avatar pane

    fn avatar_tile(&mut self, ui: &mut Ui, sel: u64, r: Rect, item: &AvatarItem) -> bool {
        // BaseTile/AvatarTile: 220x220, active (owned) vs inactive background, dimmed image when not owned
        let f = ui.selectable(sel, r);
        let bg = if item.owned { [45.0 / 255.0, 96.0 / 255.0, 128.0 / 255.0] } else { CHARACTER_BG };
        ui.rect(r, bg, 1.0);
        if !item.thumb.is_empty() {
            let tint = if item.owned { WHITE } else { [0.55, 0.55, 0.55] };
            ui.image(&item.thumb, r, tint, 1.0);
        }
        if item.owned && self.wearing_id == Some(item.id) {
            ui.image("rbxasset://textures/ui/Shell/Icons/EquippedOverlay.png", r, WHITE, 1.0);
        }
        if !item.owned {
            // PriceText: orange label in the top-right corner
            let text = match item.price {
                Some(0) | None => "FREE".to_string(),
                Some(p) => format!("R$ {}", fmt_num(p)),
            };
            let w = ui.measure(&text, Font::Regular, DESCRIPTION) + 28.0;
            let pr = Rect::new(r.x + r.w - 6.0 - w, r.y + 6.0, w, 36.0);
            ui.rect(pr, [241.0 / 255.0, 116.0 / 255.0, 10.0 / 255.0], 1.0);
            ui.label(&text, Font::Regular, DESCRIPTION, pr, Align::Center, Align::Center, [0.0; 3], 1.0);
        }
        popup_text(ui, sel, r, &item.name);
        if f {
            ui.selection_box(r, 7.0);
        }
        f
    }

    fn equip(&mut self, ui: &Ui, audio: &mut Audio, item: &AvatarItem) {
        if self.equip_busy || self.wearing_id == Some(item.id) {
            return;
        }
        let (Some(sess), Some(user)) = (self.session.clone(), self.user.clone()) else {
            self.error(ui, audio, s::equip_failed());
            return;
        };
        audio.play("ButtonPress", 1.0);
        self.equip_busy = true;
        let item = item.clone();
        let old = self.avatar.clone().unwrap_or_default();
        spawn(&self.tx, move || match sess.wear(&item) {
            Ok(()) => Msg::Worn(item.id, true, sess.wait_new_avatar(user.id, &old)),
            Err(_) => Msg::Worn(item.id, false, None),
        });
    }

    fn open_editor(&mut self, ui: &mut Ui) {
        let t0 = ui.time;
        self.push(ui, Screen::AvatarEditor { t0, cat: 0, sub: None, full: false });
        self.editor_toast = None;
        if let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) {
            spawn(&self.tx, move || {
                Msg::EditorDesc(s.avatar_info(u.id).unwrap_or_default())
            });
        }
        self.preview_outfit = None;
        if !self.avatar3d {
            self.avatar_dirty = true;
        }
        self.editor_refresh_wearing();
    }

    /// Put on everything a bundle holds (replacing what sits in the same slots), or take it all
    /// off when it is already worn. Shows at once; saved in the background like any equip.
    fn editor_wear_bundle(&mut self, bid: u64) {
        let Some(parts) = self.bundle_assets.get(&bid).cloned().filter(|a| !a.is_empty()) else { return };
        let mut ids = self.editor_wearing.clone().unwrap_or_default();
        let worn_all = parts.iter().all(|p| ids.contains(&p.0));
        let types: HashMap<u64, u32> = self.editor_desc.as_ref().and_then(|d| d["assets"].as_array()).map(|a| a.iter().filter_map(|x| Some((x["id"].as_u64()?, x["assetType"]["id"].as_u64()? as u32))).collect()).unwrap_or_default();
        let mut added = Vec::new();
        if worn_all {
            ids.retain(|i| !parts.iter().any(|p| p.0 == *i));
        } else {
            let mut slots: Vec<u32> = parts.iter().map(|p| p.1).filter(|t| single_slot(*t)).collect();
            if slots.iter().any(|t| matches!(t, 17 | 79)) {
                slots.extend([17, 79]);
            }
            ids.retain(|i| !types.get(i).is_some_and(|t| slots.contains(t)));
            for (aid, ty) in &parts {
                if !ids.contains(aid) {
                    ids.push(*aid);
                    added.push((*aid, *ty));
                }
                self.editor_recent.retain(|i| i != aid);
                self.editor_recent.insert(0, *aid);
            }
        }
        self.editor_wearing = Some(ids.clone());
        let keep = ids.clone();
        let metas = self.editor_metas();
        self.editor_apply(
            move |d| {
                let mut assets: Vec<serde_json::Value> = d["assets"].as_array().cloned().unwrap_or_default().into_iter().filter(|a| a["id"].as_u64().map(|i| keep.contains(&i)).unwrap_or(false)).collect();
                for (aid, ty) in added {
                    assets.push(serde_json::json!({ "id": aid, "assetType": { "id": ty } }));
                }
                d["assets"] = serde_json::json!(assets);
            },
            move |s| s.set_wearing(&ids, &metas),
        );
    }

    /// Each worn asset's saved "meta" (accessory adjustment / layer order), so saving keeps it.
    fn editor_metas(&self) -> HashMap<u64, serde_json::Value> {
        self.editor_desc.as_ref().and_then(|d| d["assets"].as_array()).map(|a| a.iter().filter(|x| x["meta"].is_object()).filter_map(|x| Some((x["id"].as_u64()?, x["meta"].clone()))).collect()).unwrap_or_default()
    }

    fn editor_refresh_wearing(&mut self) {
        if let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) {
            spawn(&self.tx, move || Msg::EditorWearing(s.currently_wearing(u.id).unwrap_or_default()));
        }
    }

    /// Run an avatar change, then refresh the 2D renders like an outfit change does.
    /// Change the avatar: `local` edits the avatar description so the 3D avatar updates right
    /// away, `save` sends it to Roblox in the background. A refusal puts the old look back.
    fn editor_apply(&mut self, local: impl FnOnce(&mut serde_json::Value), save: impl FnOnce(&Session) -> anyhow::Result<()> + Send + 'static) {
        let (Some(sess), Some(user)) = (self.session.clone(), self.user.clone()) else { return };
        self.editor_seq += 1;
        let seq = self.editor_seq;
        if let Some(desc) = self.editor_desc.as_mut() {
            self.editor_undo.insert(seq, desc.clone());
            local(desc);
            self.editor_desc_dirty = true;
            self.preview_outfit = None;
        }
        let old = self.avatar.clone().unwrap_or_default();
        spawn(&self.tx, move || match save(&sess) {
            Ok(()) => Msg::EditorDone(seq, true, sess.wait_new_avatar(user.id, &old)),
            Err(e) => {
                eprintln!("avatar editor change failed: {e}");
                Msg::EditorDone(seq, false, None)
            }
        });
    }

    /// Avatar editor (console editor layout): categories with icons on the left, their sub menus
    /// next to an icon column once entered, the owned items in a three column grid on the right,
    /// and the avatar standing in the 3D room behind it all.
    fn avatar_editor(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::AvatarEditor { t0, cat, sub, full }) = self.stack.last() else { return };
        let (t0, mut cat, mut sub, mut full) = (*t0, *cat, *sub, *full);
        let old = self.fade(ui, t0);
        // rotate with the right stick or the bumpers
        let dt = 1.0 / 60.0;
        self.editor_yaw += input.right_stick.x * 2.5 * dt;
        if input.has(Btn::LB) {
            self.editor_yaw -= 0.6;
        }
        if input.has(Btn::RB) {
            self.editor_yaw += 0.6;
        }
        // mouse: drag across the avatar to turn it, wheel scrolls the item grid
        let stage = Rect::new(560.0, 120.0, 730.0, 800.0);
        match (input.mouse_down, ui.mouse) {
            (true, Some(m)) => {
                if let Some(last) = self.editor_drag_x {
                    self.editor_yaw += (m.x - last) * 0.012;
                    self.editor_drag_x = Some(m.x);
                } else if stage.contains(m) {
                    self.editor_drag_x = Some(m.x);
                }
            }
            _ => self.editor_drag_x = None,
        }
        self.editor_wheel = input.wheel;
        if !self.avatar3d {
            self.loading(ui, Vec2::new(900.0, 560.0));
        }

        let r15 = self.avatar_type_r15;
        let mut chosen: Option<u64> = None;
        let mut skin: Option<&'static str> = None;
        let mut entered: Option<usize> = None;
        let mut back_to_menu = false;
        if !full {
            let cats = &ED_CATS;
            let subs = cats[cat].subs;
            let in_sub = sub.is_some() && !subs.is_empty();
            if in_sub {
                // icon column: where the entered category sits
                for (i, c) in cats.iter().enumerate() {
                    let r = Rect::new(ED_X, ED_Y0 + i as f32 * ED_STEP, 80.0, ED_ROW_H);
                    let hover = ui.mouse.map(|m| r.contains(m)).unwrap_or(false);
                    if hover && input.clicked && !blocked && i != cat {
                        audio.play("ButtonPress", 1.0);
                        cat = i;
                        sub = if cats[i].subs.is_empty() { None } else { Some(0) };
                        self.editor_list_scroll = 0.0;
                        self.editor_grid_scroll = 0.0;
                        self.editor_grid_target = 0.0;
                    }
                    if hover {
                        ui.rect(r, WHITE, 0.12);
                    }
                    let current = i == cat;
                    ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], if current { ED_BLUE } else { WHITE }, if current { 1.0 } else { 0.18 });
                    ui.image(c.icon, Rect::new(r.x + 24.0, r.y + 22.0, 32.0, 32.0), if current { TEXT_SELECTED } else { WHITE }, if current { 1.0 } else { 0.8 });
                }
                // sub menu list, scrolled so the current row stays in view
                let sel = sub.unwrap_or(0).min(subs.len() - 1);
                let list_focused = (0..subs.len()).any(|j| ui.is_focused(id("edsub", j as u64)));
                let top = ED_Y0 + (sel as f32 - self.editor_list_scroll) * ED_STEP;
                let target = if top < ED_Y0 - 1.0 {
                    sel as f32
                } else if 1080.0 - (top + ED_ROW_H) < 190.0 {
                    sel as f32 - 5.0
                } else {
                    self.editor_list_scroll
                };
                self.editor_list_scroll += (target - self.editor_list_scroll) * 0.3;
                ui.push_clip(Rect::new(ED_SUB_X - 10.0, 160.0, ED_ROW_W + 20.0, 920.0));
                for (j, (name, _)) in subs.iter().enumerate() {
                    let r = Rect::new(ED_SUB_X, ED_Y0 + (j as f32 - self.editor_list_scroll) * ED_STEP, ED_ROW_W, ED_ROW_H);
                    let sid = id("edsub", j as u64);
                    let f = ui.selectable(sid, r);
                    if f && sel != j {
                        sub = Some(j);
                        // (--editor N keeps its outfit preview for testing)
                        if !self.debug_lock && self.preview_outfit.take().is_some() {
                            self.avatar_dirty = true;
                        }
                        self.editor_grid_scroll = 0.0;
                        self.editor_grid_target = 0.0;
                    }
                    let current = j == sel;
                    ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], if current { ED_BLUE } else { WHITE }, if current { 1.0 } else { 0.18 });
                    if f {
                        ui.selection_box(r, 3.0);
                    }
                    // inactiveTabButton: half transparent text once the selection moves to the items
                    let fade = if current || list_focused { 1.0 } else { 0.5 };
                    ui.label(name, Font::Regular, ED_TEXT, Rect::new(r.x + 22.0, r.y, r.w - 30.0, r.h), Align::Start, Align::Center, if current { TEXT_SELECTED } else { WHITE }, fade);
                    if !blocked && ui.pressed(sid) {
                        // consumed here: the tile focused below must not take the same press
                        ui.activated = false;
                        sub = Some(j);
                        if let Some(first) = self.editor_first_tile.get(&(cat, j)).copied() {
                            ui.set_focus(first);
                        }
                    }
                }
                ui.pop_clip();
            } else {
                for (i, c) in cats.iter().enumerate() {
                    let r = Rect::new(ED_X, ED_Y0 + i as f32 * ED_STEP, ED_ROW_W, ED_ROW_H);
                    let mid = id("edmain", i as u64);
                    let f = ui.selectable(mid, r);
                    if f && cat != i {
                        cat = i;
                        // (--editor N keeps its outfit preview for testing)
                        if !self.debug_lock && self.preview_outfit.take().is_some() {
                            self.avatar_dirty = true;
                        }
                        self.editor_grid_scroll = 0.0;
                        self.editor_grid_target = 0.0;
                    }
                    let current = cat == i;
                    ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], if current { ED_BLUE } else { WHITE }, if current { 1.0 } else { 0.18 });
                    if f {
                        ui.selection_box(r, 3.0);
                    }
                    let tint = if current { TEXT_SELECTED } else { WHITE };
                    ui.image(c.icon, Rect::new(r.x + 24.0, r.y + 22.0, 32.0, 32.0), tint, 1.0);
                    ui.label(c.name, Font::Regular, ED_TEXT, Rect::new(r.x + 80.0, r.y, r.w - 90.0, r.h), Align::Start, Align::Center, tint, 1.0);
                    if !blocked && ui.pressed(mid) {
                        ui.activated = false;
                        entered = Some(i);
                    }
                }
            }
            if !ui.focus_valid() {
                match sub {
                    Some(j) if in_sub => ui.set_focus(id("edsub", j as u64)),
                    _ => ui.set_focus(id("edmain", cat as u64)),
                }
            }

            // content for the current (sub) category
            let what = if subs.is_empty() { cats[cat].own } else { subs[sub.unwrap_or(0).min(subs.len() - 1)].1 };
            let wearing = self.editor_wearing.clone().unwrap_or_default();
            match what {
                EdSub::Skin => {
                    ui.nine_slice(BUTTON_IMG, Rect::new(ED_GRID_X - 4.0, ED_Y0 - 3.0, 490.0, 6.0 * 74.0 + 7.0 * 20.0 + 8.0), [8.0, 8.0, 24.0, 24.0], [0.55, 0.55, 0.55], 0.85);
                    for (i, hex) in SKIN_HEX.iter().enumerate() {
                        let r = Rect::new(ED_GRID_X + 20.0 + (i % 5) as f32 * 94.0, ED_Y0 + 20.0 + (i / 5) as f32 * 94.0, 74.0, 74.0);
                        let sid = id("edskin", i as u64);
                        let f = ui.selectable(sid, r);
                        ui.image(CIRCLE_IMG, r.inset(-1.0), [0.75, 0.75, 0.75], 0.6);
                        ui.image(CIRCLE_IMG, r, hex_rgb(hex), 1.0);
                        if self.editor_skin.as_deref() == Some(*hex) {
                            ui.image(CIRCLE_IMG, r.inset(-7.0), WHITE, 0.9);
                            ui.image(CIRCLE_IMG, r.inset(-3.0), [0.55, 0.55, 0.55], 1.0);
                            ui.image(CIRCLE_IMG, r, hex_rgb(hex), 1.0);
                            ui.image("rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png", r.inset(21.0), WHITE, 1.0);
                        }
                        if f {
                            ui.image(CIRCLE_IMG, r.inset(-9.0), [0.4, 0.75, 1.0], 0.5);
                            ui.image(CIRCLE_IMG, r, hex_rgb(hex), 1.0);
                        }
                        if !blocked && ui.pressed(sid) {
                            skin = Some(hex);
                        }
                    }
                }
                EdSub::Scale => self.editor_scale_sliders(ui, input, blocked, r15),
                EdSub::Emote(slot) => {
                    self.emote_wheel(ui, slot);
                    let tiles: Vec<(u64, bool)> = self
                        .editor_list(61)
                        .iter()
                        .map(|(aid, _)| (*aid, self.editor_emotes.iter().any(|e| e.0 == slot && e.1 == *aid)))
                        .collect();
                    chosen = self.editor_grid(ui, blocked, (cat, sub.unwrap_or(0)), &tiles);
                }
                EdSub::MyOutfits | EdSub::PresetOutfits => {
                    let mine = what == EdSub::MyOutfits;
                    let tiles: Vec<(u64, bool)> = match &self.avatar_items {
                        Some(Ok((o, c))) => (if mine { o } else { c }).iter().map(|o| (o.id, mine && self.wearing_id == Some(o.id))).collect(),
                        _ => Vec::new(),
                    };
                    for o in self.avatar_items.as_ref().and_then(|r| r.as_ref().ok()).map(|(o, c)| o.iter().chain(c.iter()).cloned().collect::<Vec<_>>()).unwrap_or_default() {
                        if !o.thumb.is_empty() {
                            self.editor_thumbs.entry(o.id).or_insert(o.thumb);
                        }
                    }
                    chosen = self.editor_grid(ui, blocked, (cat, sub.unwrap_or(0)), &tiles);
                    // focusing one of your outfits previews it on the avatar
                    if mine && !self.debug_lock {
                        if let Some((aid, _)) = tiles.iter().find(|t| ui.is_focused(id("editem", t.0))) {
                            if self.preview_outfit != Some(*aid) {
                                self.preview_outfit = Some(*aid);
                                self.avatar_dirty = true;
                            }
                        }
                    }
                }
                EdSub::Bundles => {
                    if self.editor_bundles.is_none() {
                        self.request_editor_items();
                    }
                    // worn: everything the bundle puts on is worn
                    let tiles: Vec<(u64, bool)> = match &self.editor_bundles {
                        Some(Ok(b)) => b.iter().map(|(bid, _)| (bid | BUNDLE_TAG, self.bundle_assets.get(bid).is_some_and(|a| !a.is_empty() && a.iter().all(|x| wearing.contains(&x.0))))).collect(),
                        _ => Vec::new(),
                    };
                    chosen = self.editor_grid(ui, blocked, (cat, sub.unwrap_or(0)), &tiles);
                }
                EdSub::Recent => {
                    let tiles: Vec<(u64, bool)> = self.editor_recent.iter().map(|a| (*a, wearing.contains(a))).collect();
                    chosen = self.editor_grid(ui, blocked, (cat, 0), &tiles);
                }
                EdSub::Asset(ty) => {
                    let tiles: Vec<(u64, bool)> = self.editor_list(ty).iter().map(|(a, _)| (*a, wearing.contains(a))).collect();
                    chosen = self.editor_grid(ui, blocked, (cat, sub.unwrap_or(0)), &tiles);
                }
            }
            self.preview_anim = match what {
                EdSub::Asset(48..=56) | EdSub::Emote(_) => self.editor_tile_ids.iter().position(|t| ui.is_focused(*t)).and_then(|i| self.editor_tile_assets.get(i).copied()),
                _ => None,
            };
            // moving into the content from the main list enters that category
            if sub.is_none() && !subs.is_empty() && !(0..cats.len()).any(|i| ui.is_focused(id("edmain", i as u64))) {
                sub = Some(0);
            }
            // R6 notices, like the console's toast above the hints
            if !r15 && what == EdSub::Scale {
                self.editor_toast = Some(("Scaling only works for R15 avatars".into(), ui.time));
            } else if !r15 && matches!(what, EdSub::Asset(48..=56)) {
                self.editor_toast = Some(("Animations only work for R15 avatars".into(), ui.time));
            }
        }

        // toast + hints along the bottom
        if let Some((text, at)) = self.editor_toast.clone() {
            let a = (1.0 - (ui.time - at - 2.5) / 0.4).clamp(0.0, 1.0);
            if a > 0.0 {
                let tw = (ui.measure(&text, Font::Regular, 27.0) + 48.0).min(580.0);
                let r = Rect::new(960.0 - tw * 0.5, 1080.0 - 60.0 - 83.0 - 90.0, tw, 66.0);
                ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], WHITE, a);
                ui.image(CORNER_IMG, Rect::new(r.x + 40.0, r.y + r.h - 1.0, 22.0, 16.0), WHITE, 0.0);
                ui.label(&text, Font::Regular, 27.0, r, Align::Center, Align::Center, TEXT_SELECTED, a);
            } else {
                self.editor_toast = None;
            }
        }
        const ICON: f32 = 83.0;
        let hy = 1080.0 - 60.0 - ICON;
        let switch_text = if r15 { "Switch to R6" } else { "Switch to R15" };
        let full_text = if full { "Exit Full View" } else { "Full View" };
        let w1 = ICON + 16.0 + ui.measure(switch_text, Font::Regular, ED_TEXT);
        let w2 = ICON + 16.0 + ui.measure(full_text, Font::Regular, ED_TEXT);
        let x1 = 960.0 - (w1 + 60.0 + w2) * 0.5;
        let x2 = x1 + w1 + 60.0;
        ui.image(CIRCLE_IMG, Rect::new(x1 + 3.0, hy + 3.0, ICON - 6.0, ICON - 6.0), [0.08, 0.08, 0.09], 0.85);
        ui.image(VIEW_IMG, Rect::new(x1, hy, ICON, ICON), WHITE, 1.0);
        ui.label(switch_text, Font::Regular, ED_TEXT, Rect::new(x1 + ICON + 16.0, hy, w1, ICON), Align::Start, Align::Center, WHITE, 1.0);
        ui.image(RS_IMG, Rect::new(x2, hy, ICON, ICON), WHITE, 1.0);
        ui.label(full_text, Font::Regular, ED_TEXT, Rect::new(x2 + ICON + 16.0, hy, w2, ICON), Align::Start, Align::Center, WHITE, 1.0);
        let clicked_in = |r: Rect| input.clicked && ui.mouse.map(|m| r.contains(m)).unwrap_or(false);
        let switch_clicked = clicked_in(Rect::new(x1, hy, w1, ICON));
        let full_clicked = clicked_in(Rect::new(x2, hy, w2, ICON));

        self.end_fade(ui, old);
        if let Some(Screen::AvatarEditor { cat: c, sub: s, full: fu, .. }) = self.stack.last_mut() {
            *c = cat;
            *s = sub;
            *fu = full;
        }
        if blocked {
            return;
        }
        let in_grid = ui.focus.map(|f| self.editor_tile_ids.contains(&f)).unwrap_or(false);
        let in_list = sub.is_some() && (0..ED_CATS[cat].subs.len()).any(|j| ui.is_focused(id("edsub", j as u64)));
        if input.has(Btn::RS) || full_clicked || (full && input.has(Btn::B)) {
            audio.play("ButtonPress", 1.0);
            full = !full;
        } else if input.has(Btn::View) || switch_clicked {
            audio.play("ButtonPress", 1.0);
            self.avatar_type_r15 = !r15;
            if r15 {
                self.editor_toast = Some(("Animations only work for R15 avatars".into(), ui.time));
            }
            self.editor_apply(move |d| d["playerAvatarType"] = serde_json::json!(if r15 { "R6" } else { "R15" }), move |s| s.set_avatar_type(!r15));
        } else if let Some(i) = entered {
            audio.play("ButtonPress", 1.0);
            if ED_CATS[i].subs.is_empty() {
                if let Some(first) = self.editor_first_tile.get(&(i, 0)).copied() {
                    ui.set_focus(first);
                }
            } else {
                sub = Some(0);
                self.editor_list_scroll = 0.0;
                ui.set_focus(id("edsub", 0));
            }
        } else if let Some(aid) = chosen {
            audio.play("ButtonPress", 1.0);
            self.editor_choose(ui, audio, cat, sub, aid);
        } else if let Some(hex) = skin {
            audio.play("ButtonPress", 1.0);
            self.editor_skin = Some(hex.to_string());
            self.editor_apply(
                move |d| {
                    for k in ["headColor3", "torsoColor3", "rightArmColor3", "leftArmColor3", "rightLegColor3", "leftLegColor3"] {
                        d["bodyColor3s"][k] = serde_json::json!(hex);
                    }
                },
                move |s| s.set_skin_hex(hex),
            );
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            if in_grid && sub.is_some() && !ED_CATS[cat].subs.is_empty() {
                ui.set_focus(id("edsub", sub.unwrap_or(0) as u64));
            } else if in_grid || in_list {
                back_to_menu = true;
            } else {
                if self.preview_outfit.take().is_some() {
                    self.avatar_dirty = true;
                }
                self.pop(ui);
                return;
            }
        }
        if back_to_menu {
            sub = None;
            ui.set_focus(id("edmain", cat as u64));
        }
        if let Some(Screen::AvatarEditor { sub: s, full: fu, .. }) = self.stack.last_mut() {
            *s = sub;
            *fu = full;
        }
    }

    /// Owned items of one asset type (asks the inventory API the first time).
    fn editor_list(&mut self, ty: u32) -> Vec<(u64, String)> {
        match self.editor_items.get(&ty) {
            None => {
                self.editor_items.insert(ty, Ok(Vec::new()));
                if let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) {
                    spawn(&self.tx, move || Msg::EditorItems(ty, s.inventory(u.id, ty).map_err(|e| e.to_string())));
                }
                Vec::new()
            }
            Some(Ok(items)) => items.clone(),
            Some(Err(_)) => Vec::new(),
        }
    }

    /// Ask for every editor category's inventory now (the background prefetch after sign-in).
    pub fn request_editor_items(&mut self) {
        for ty in EDITOR_ASSET_TYPES {
            let _ = self.editor_list(ty);
        }
        if let (None, Some(s), Some(u)) = (&self.editor_bundles, self.session.clone(), self.user.clone()) {
            self.editor_bundles = Some(Ok(Vec::new()));
            spawn(&self.tx, move || Msg::EditorBundles(s.owned_bundles(u.id).map_err(|e| e.to_string())));
        }
        if let Some(s) = self.session.clone() {
            spawn(&self.tx, move || Msg::EditorEmotes(s.emotes().unwrap_or_default()));
        }
        self.editor_refresh_wearing();
    }

    /// The three column item grid: blue tiles, green border + corner on what is worn, white
    /// border on the focused tile. Returns the tile pressed.
    fn editor_grid(&mut self, ui: &mut Ui, blocked: bool, key: (usize, usize), tiles: &[(u64, bool)]) -> Option<u64> {
        const COLS: usize = 3;
        let focus_row = tiles.iter().position(|t| ui.is_focused(id("editem", t.0))).map(|i| i / COLS);
        let rows = tiles.len().div_ceil(COLS);
        let max_scroll = (rows as f32 * ED_TILE_STEP - (1080.0 - ED_Y0 - 40.0)).max(0.0);
        let over_grid = ui.mouse.map(|m| m.x >= ED_GRID_X - 20.0 && m.y >= ED_Y0 - 20.0).unwrap_or(false);
        if self.editor_wheel != 0.0 && over_grid {
            self.editor_grid_target = (self.editor_grid_target - self.editor_wheel * ED_TILE_STEP).clamp(0.0, max_scroll);
        } else if let Some(row) = focus_row.filter(|r| Some(*r) != self.editor_grid_row) {
            // keep a newly focused row in view (controller / keyboard)
            let top = row as f32 * ED_TILE_STEP;
            let view = 1080.0 - ED_Y0 - ED_TILE_STEP - 40.0;
            if top < self.editor_grid_target {
                self.editor_grid_target = top;
            } else if top > self.editor_grid_target + view {
                self.editor_grid_target = top - view;
            }
        }
        self.editor_grid_row = focus_row;
        self.editor_grid_target = self.editor_grid_target.clamp(0.0, max_scroll);
        self.editor_grid_scroll += (self.editor_grid_target - self.editor_grid_scroll) * 0.3;
        self.editor_tile_ids.clear();
        self.editor_tile_assets.clear();
        if let Some(t) = tiles.first() {
            self.editor_first_tile.insert(key, id("editem", t.0));
        } else {
            self.editor_first_tile.remove(&key);
        }
        let mut pressed = None;
        ui.push_clip(Rect::new(ED_GRID_X - 12.0, ED_Y0 - 12.0, 3.0 * ED_TILE_STEP + 24.0, 1080.0 - ED_Y0 + 12.0));
        for (i, (aid, worn)) in tiles.iter().enumerate() {
            let r = Rect::new(ED_GRID_X + (i % COLS) as f32 * ED_TILE_STEP, ED_Y0 + (i / COLS) as f32 * ED_TILE_STEP - self.editor_grid_scroll, ED_TILE, ED_TILE);
            let sel = id("editem", *aid);
            self.editor_tile_ids.push(sel);
            self.editor_tile_assets.push(*aid);
            let f = ui.selectable(sel, r);
            ui.rect(r, ED_TILE_BG, 1.0);
            if let Some(t) = self.editor_thumbs.get(aid).cloned() {
                ui.image(&t, r.inset(6.0), WHITE, 1.0);
            }
            if *worn {
                outline(ui, r, 4.0, ED_GREEN, 1.0);
                ui.image(CORNER_IMG, Rect::new(r.x + r.w - 28.0, r.y, 28.0, 28.0), ED_GREEN, 1.0);
            }
            if f {
                outline(ui, r, 4.0, WHITE, 1.0);
                ui.selection_box(r, 4.0);
            }
            if !blocked && ui.pressed(sel) {
                pressed = Some(*aid);
            }
        }
        ui.pop_clip();
        pressed
    }

    /// Scale sliders (R15): Height, Width, Head, Body Type, Proportions; left/right moves the
    /// focused one, the change is saved once the stick rests.
    fn editor_scale_sliders(&mut self, ui: &mut Ui, input: &Input, blocked: bool, r15: bool) {
        const NAMES: [&str; 5] = ["Height", "Width", "Head", "Body Type", "Proportions"];
        const RANGE: [(f32, f32); 5] = [(0.9, 1.05), (0.7, 1.0), (0.95, 1.0), (0.0, 1.0), (0.0, 1.0)];
        let mut v = self.editor_scales.unwrap_or([1.0, 1.0, 1.0, 0.0, 0.0]);
        let mut changed = false;
        for (i, name) in NAMES.iter().enumerate() {
            let y = ED_Y0 + 56.0 + 15.0 + i as f32 * 120.0;
            ui.label(name, Font::Regular, 24.0, Rect::new(ED_GRID_X, y - 44.0, 300.0, 26.0), Align::Start, Align::Center, WHITE, 1.0);
            let track = Rect::new(ED_GRID_X + 15.0, y - 3.0, 460.0, 6.0);
            let (lo, hi) = RANGE[i];
            let k = ((v[i] - lo) / (hi - lo)).clamp(0.0, 1.0);
            ui.rect(track, [0.85, 0.85, 0.85], 1.0);
            ui.rect(Rect::new(track.x, track.y, track.w * k, track.h), ED_BLUE, 1.0);
            let knob = Vec2::new(track.x + track.w * k, y);
            let sid = id("edscale", i as u64);
            let f = ui.selectable(sid, Rect::new(track.x - 20.0, y - 30.0, track.w + 40.0, 60.0));
            if f {
                ui.image(CIRCLE_IMG, Rect::new(knob.x - 28.0, knob.y - 28.0, 56.0, 56.0), [0.4, 0.75, 1.0], 0.45);
            }
            ui.image(CIRCLE_IMG, Rect::new(knob.x - 17.0, knob.y - 17.0, 34.0, 34.0), WHITE, 1.0);
            ui.image(CIRCLE_IMG, Rect::new(knob.x - 13.0, knob.y - 13.0, 26.0, 26.0), ED_BLUE, 1.0);
            if f {
                // left / right move the slider instead of the selection
                ui.lock_x = true;
            }
            if f && !blocked && r15 {
                let step = (hi - lo) / 20.0;
                if input.has(Btn::Left) {
                    v[i] = (v[i] - step).max(lo);
                    changed = true;
                }
                if input.has(Btn::Right) {
                    v[i] = (v[i] + step).min(hi);
                    changed = true;
                }
            }
        }
        if changed {
            self.editor_scales = Some(v);
            self.editor_scales_dirty = Some(ui.time);
        }
        // save after the slider rests for a moment
        if let Some(at) = self.editor_scales_dirty {
            if ui.time - at > 0.6 && !self.editor_busy {
                self.editor_scales_dirty = None;
                self.editor_apply(
                    move |d| d["scales"] = serde_json::json!({ "height": v[0], "width": v[1], "depth": v[1], "head": v[2], "bodyType": v[3], "proportion": v[4] }),
                    move |s| s.set_scales(v[0], v[1], v[2], v[3], v[4]),
                );
            }
        }
    }

    /// The emote wheel in the top right: eight numbered slots, the current one highlighted.
    fn emote_wheel(&mut self, ui: &mut Ui, slot: u8) {
        let c = Vec2::new(1545.0, 158.0);
        ui.image(CIRCLE_IMG, Rect::new(c.x - 104.0, c.y - 104.0, 208.0, 208.0), [0.2, 0.2, 0.22], 0.75);
        ui.image(CIRCLE_IMG, Rect::new(c.x - 48.0, c.y - 48.0, 96.0, 96.0), [0.08, 0.08, 0.09], 0.95);
        for n in 1..=8u8 {
            let a = (n as f32 - 1.0) * std::f32::consts::TAU / 8.0;
            let p = c + Vec2::new(a.sin(), -a.cos()) * 76.0;
            if n == slot {
                ui.image(CIRCLE_IMG, Rect::new(p.x - 26.0, p.y - 26.0, 52.0, 52.0), [0.96, 0.45, 0.1], 1.0);
            }
            ui.label(&n.to_string(), Font::Regular, 24.0, Rect::new(p.x - 20.0, p.y - 16.0, 40.0, 32.0), Align::Center, Align::Center, WHITE, 1.0);
        }
    }

    /// A tile was pressed: wear / take off, equip an outfit, or put an emote in its slot.
    fn editor_choose(&mut self, ui: &mut Ui, audio: &mut Audio, cat: usize, sub: Option<usize>, aid: u64) {
        let subs = ED_CATS[cat].subs;
        let what = if subs.is_empty() { ED_CATS[cat].own } else { subs[sub.unwrap_or(0).min(subs.len() - 1)].1 };
        match what {
            EdSub::MyOutfits => {
                let item = self.avatar_items.as_ref().and_then(|r| r.as_ref().ok()).and_then(|(o, _)| o.iter().find(|o| o.id == aid).cloned());
                if let Some(item) = item {
                    self.preview_outfit = None;
                    self.equip(ui, audio, &item);
                }
            }
            // catalog costumes would need buying them
            EdSub::PresetOutfits => self.error(ui, audio, s::purchase_unavailable()),
            EdSub::Bundles => {
                let bid = aid & !BUNDLE_TAG;
                if self.bundle_assets.get(&bid).is_some_and(|a| !a.is_empty()) {
                    self.editor_wear_bundle(bid);
                } else if let Some(s) = self.session.clone() {
                    spawn(&self.tx, move || Msg::BundleAssets(bid, s.bundle_assets(bid).unwrap_or_default(), true));
                }
            }
            EdSub::Emote(slot) => {
                if let Some(s) = self.session.clone() {
                    self.editor_emotes.retain(|e| e.0 != slot);
                    self.editor_emotes.push((slot, aid, String::new()));
                    spawn(&self.tx, move || {
                        let _ = s.set_emote(aid, slot);
                        Msg::EditorEmotes(s.emotes().unwrap_or_default())
                    });
                }
            }
            _ => {
                // wear / take off; one item per slot for clothing, faces, body parts and animations
                let mut ids = self.editor_wearing.clone().unwrap_or_default();
                let mut added_ty = None;
                if let Some(p) = ids.iter().position(|i| *i == aid) {
                    ids.remove(p);
                } else {
                    let ty = match what {
                        EdSub::Asset(t) => Some(t),
                        _ => EDITOR_ASSET_TYPES.iter().copied().find(|t| self.editor_items.get(t).and_then(|r| r.as_ref().ok()).map(|v| v.iter().any(|i| i.0 == aid)).unwrap_or(false)),
                    };
                    if let Some(ty) = ty.filter(|t| single_slot(*t)) {
                        // a classic head and a dynamic head share the one head slot
                        let slot: &[u32] = if matches!(ty, 17 | 79) { &[17, 79] } else { std::slice::from_ref(&ty) };
                        let same: Vec<u64> = slot.iter().filter_map(|t| self.editor_items.get(t).and_then(|r| r.as_ref().ok())).flat_map(|v| v.iter().map(|i| i.0)).collect();
                        ids.retain(|i| !same.contains(i));
                    }
                    ids.push(aid);
                    added_ty = ty;
                    self.editor_recent.retain(|i| *i != aid);
                    self.editor_recent.insert(0, aid);
                }
                // green borders and the 3D avatar change now; Roblox is told in the background
                self.editor_wearing = Some(ids.clone());
                let keep = ids.clone();
                let metas = self.editor_metas();
                self.editor_apply(
                    move |d| {
                        let mut assets: Vec<serde_json::Value> = d["assets"].as_array().cloned().unwrap_or_default().into_iter().filter(|a| a["id"].as_u64().map(|i| keep.contains(&i)).unwrap_or(false)).collect();
                        if let Some(ty) = added_ty {
                            assets.push(serde_json::json!({ "id": aid, "assetType": { "id": ty } }));
                        }
                        d["assets"] = serde_json::json!(assets);
                    },
                    move |s| s.set_wearing(&ids, &metas),
                );
            }
        }
    }

    fn avatar_pane(&mut self, ui: &mut Ui, audio: &mut Audio, p: Rect, blocked: bool) {
        // MyAvatarContainer: 38% width, name label then the render with two rotating glows
        let left = Rect::new(p.x, p.y, p.w * 0.38, p.h);
        let name = self.user.as_ref().map(|u| u.display_name.clone()).unwrap_or_default();
        ui.label(&name, Font::Regular, SUBHEADER, Rect::new(left.x + 12.0, left.y, left.w, 25.0), Align::Start, Align::Center, WHITE, 1.0);
        let pic = Rect::new(left.x, left.y + 30.0, left.w, left.h - 30.0);
        let side = pic.w.min(pic.h) * 1.05;
        let img = Rect::new(pic.center().x - side * 0.5, pic.center().y - side * 0.5, side, side);
        // GLOW_BASE_RPM 2, GLOW_TOP_RPM -0.5, transparency 0.2; sized to fade out inside the pane
        let k = (left.w * 1.15).min(left.h * 1.05) / 1015.0;
        let t = ui.time;
        let fade_k = ((t - self.avatar_fade.0) / 0.75).clamp(0.0, 1.0);
        let glow_a = 0.8 * if self.avatar_fade.1.is_some() { ease_in_out_quad(fade_k) } else { 1.0 };
        ui.image_rotated("rbxasset://textures/ui/Shell/Images/CharacterGlow/CharacterGlowBase.png", img.center(), Vec2::new(1015.0, 1002.0) * k, (t * 2.0 * 6.0).to_radians(), glow_a);
        ui.image_rotated("rbxasset://textures/ui/Shell/Images/CharacterGlow/CharacterGlowTop.png", img.center(), Vec2::new(1026.0, 1009.0) * k, (t * -0.5 * 6.0).to_radians(), glow_a);
        match self.avatar_full.clone() {
            Some(u) if !u.is_empty() => {
                // CrossfadeAvatarImage: old render fades out while the new one fades in (0.75s)
                if let Some(old) = self.avatar_fade.1.clone().filter(|o| !o.is_empty() && fade_k < 1.0) {
                    ui.image(&old, img, WHITE, 1.0 - ease_in_out_quad(fade_k));
                }
                ui.image(&u, img, WHITE, if self.avatar_fade.1.is_some() { ease_in_out_quad(fade_k) } else { 1.0 });
            }
            Some(_) => {}
            None => self.loading(ui, img.center()),
        }
        if self.equip_busy {
            self.loading(ui, img.center());
        }

        // SelectableAvatarsContainer: 60% width at 40%
        let right = Rect::new(p.x + p.w * 0.4, p.y, p.w * 0.6, p.h);
        let (outfits, catalog) = match &self.avatar_items {
            None => {
                self.loading(ui, right.center());
                return;
            }
            Some(Err(_)) => {
                ui.paragraph("An error occurred. Please try again later.", Font::Bold, DESCRIPTION, Rect::new(right.x + right.w * 0.05, right.y, right.w * 0.9, 300.0), GREY_TEXT, 0.5, 4);
                return;
            }
            Some(Ok((o, c))) => (o.clone(), c.clone()),
        };
        let (cell, gap) = (220.0, 25.0);
        let mut picked: Option<(AvatarItem, bool)> = None;
        let mut focused_item: Option<AvatarItem> = None;
        let mut row = |this: &mut Self, ui: &mut Ui, title: &str, items: &[AvatarItem], y: f32, rows: usize, key: &str, scroll_is_outfit: bool| {
            ui.label(title, Font::Regular, SUBHEADER, Rect::new(right.x, y, right.w, 40.0), Align::Start, Align::Center, WHITE, 1.0);
            let per_col = rows.max(1);
            let focus_idx = ui.focus.and_then(|f| (0..items.len()).find(|i| id(key, *i as u64) == f));
            let scroll = if scroll_is_outfit { &mut this.outfit_scroll } else { &mut this.catalog_scroll };
            if let Some(i) = focus_idx {
                let left_x = (i / per_col) as f32 * (cell + gap);
                let visible = right.w - cell;
                let target = if left_x - *scroll > visible { left_x - visible } else if left_x < *scroll { left_x } else { *scroll };
                *scroll += (target - *scroll) * 0.25;
            }
            let sx = *scroll;
            ui.push_clip(Rect::new(right.x - 10.0, y + 30.0, right.w + 200.0, rows as f32 * (cell + gap) + 60.0));
            for (i, it) in items.iter().enumerate() {
                let r = Rect::new(right.x + (i / per_col) as f32 * (cell + gap) - sx, y + 40.0 + (i % per_col) as f32 * (cell + gap), cell, cell);
                if r.x > right.x + right.w + 300.0 || r.x + r.w < right.x - 300.0 {
                    continue;
                }
                let sel = id(key, i as u64);
                if this.avatar_tile(ui, sel, r, it) {
                    focused_item = Some(it.clone());
                }
                if !blocked && ui.pressed(sel) {
                    picked = Some((it.clone(), true));
                }
            }
            ui.pop_clip();
        };
        if outfits.is_empty() {
            row(self, ui, "CATALOG", &catalog, right.y, 2, "avcat", false);
        } else if catalog.is_empty() {
            row(self, ui, "MY OUTFITS", &outfits, right.y, 2, "avown", true);
        } else {
            row(self, ui, "MY OUTFITS", &outfits, right.y, 1, "avown", true);
            row(self, ui, "CATALOG", &catalog, right.y + 320.0, 1, "avcat", false);
        }
        // HintActionView: X = Equip, shown for owned items that are not being worn
        let can_equip = focused_item.as_ref().map(|i| i.owned && self.wearing_id != Some(i.id)).unwrap_or(false);
        if can_equip {
            hint(ui, &glyph('X'), "EQUIP");
        }
        hint_left(ui, &glyph('Y'), "AVATAR EDITOR");
        if blocked {
            return;
        }
        if ui.activated_y {
            audio.play("ScreenChange", 1.0);
            self.open_editor(ui);
            return;
        }
        if let Some((item, _)) = picked {
            if item.owned {
                self.equip(ui, audio, &item);
            } else {
                // AvatarTile:OnClick -> PurchasePackagePrompt; purchasing is disabled
                audio.play("ButtonPress", 1.0);
                self.error(ui, audio, s::purchase_unavailable());
            }
        } else if can_equip && ui.activated_x {
            if let Some(item) = focused_item {
                self.equip(ui, audio, &item);
            }
        }
    }

    // >> games pane

    fn games_pane(&mut self, ui: &mut Ui, audio: &mut Audio, p: Rect, blocked: bool) {
        let Some(sorts) = self.sorts.clone() else {
            self.loading(ui, p.center());
            return;
        };
        let mut rows: Vec<(String, Vec<GameInfo>)> = Vec::new();
        if let Some(f) = &self.favorites {
            if !f.is_empty() {
                rows.push(("FAVORITES".into(), f.clone()));
            }
        }
        for s in &sorts {
            rows.push((s.title.to_uppercase(), s.games.clone()));
        }
        let row_h = 300.0;
        let tile = 232.0;
        let gap = 20.0;
        let focused_row = ui.focus.and_then(|f| (0..rows.len()).find(|r| (0..60u64).any(|k| id("gamesrow", (*r as u64) << 8 | k) == f)));
        if let Some(r) = focused_row {
            let max = (rows.len() as f32 * row_h - p.h + 60.0).max(0.0);
            let target = (r as f32 * row_h).min(max);
            self.games_scroll += (target - self.games_scroll) * 0.2;
        }
        ui.push_clip(Rect::new(p.x - 20.0, p.y - 10.0, p.w + 120.0, p.h + 20.0));
        let mut open = None;
        for (ri, (title, games)) in rows.iter().enumerate() {
            let y = p.y + ri as f32 * row_h - self.games_scroll;
            if y > p.y + p.h + row_h || y < p.y - row_h * 2.0 {
                continue;
            }
            ui.label(title, Font::Regular, SUBHEADER, Rect::new(p.x, y, 1200.0, 36.0), Align::Start, Align::Start, WHITE, 1.0);
            let n = games.len().min(60);
            let focused_col = ui.focus.and_then(|f| (0..n).find(|k| id("gamesrow", (ri as u64) << 8 | *k as u64) == f));
            let scroll = self.games_row_scroll.entry(ri).or_insert(0.0);
            if let Some(c) = focused_col {
                let left = c as f32 * (tile + gap);
                let visible = p.w - tile;
                let target = if left - *scroll > visible { left - visible } else if left < *scroll { left } else { *scroll };
                *scroll += (target - *scroll) * 0.25;
            }
            let sx = *scroll;
            for (k, g) in games.iter().take(n).enumerate() {
                let r = Rect::new(p.x + k as f32 * (tile + gap) - sx, y + 46.0, tile, tile);
                if r.x > p.x + p.w + 200.0 || r.x + r.w < p.x - 400.0 {
                    continue;
                }
                let sel = id("gamesrow", (ri as u64) << 8 | k as u64);
                self.game_tile(ui, sel, r, Some(g));
                if !blocked && ui.pressed(sel) {
                    open = Some((g.universe_id, g.place_id));
                }
            }
        }
        ui.pop_clip();
        let want: Vec<u64> = rows.iter().flat_map(|r| r.1.iter().take(60).map(|g| g.universe_id)).collect();
        self.want_icons(want);
        if let Some((u, pl)) = open {
            audio.play("ButtonPress", 1.0);
            self.open_game(ui, audio, u, pl);
        }
    }

    // >> social pane

    fn online_friends(&self) -> Option<Vec<Friend>> {
        self.friends.as_ref().map(|f| f.iter().filter(|f| f.presence > 0).cloned().collect())
    }

    /// Friends tab (v2.0+ Shell): Online / All filters, the list of friends (headshot, username,
    /// status dot + where they are), and the focused friend's full body render on the right.
    fn friends_pane(&mut self, ui: &mut Ui, audio: &mut Audio, blocked: bool) {
        let Some(all) = self.friends.clone() else {
            self.loading(ui, Vec2::new(960.0, 600.0));
            return;
        };
        // filters
        let mut chosen_filter = None;
        for (i, name) in ["Online", "All"].iter().enumerate() {
            let r = Rect::new(158.0, 283.0 + i as f32 * 118.0, 360.0, 78.0);
            let sel = id("frfilter", i as u64);
            let f = ui.selectable(sel, r);
            if f && self.friends_filter != i {
                self.friends_filter = i;
                self.friends_scroll = 0.0;
            }
            let current = self.friends_filter == i;
            ui.rect(r, if current { ED_BLUE } else { GREY_BUTTON }, if current { 1.0 } else { 0.85 });
            ui.label(name, Font::Regular, 30.0, Rect::new(r.x + 24.0, r.y, r.w - 30.0, r.h), Align::Start, Align::Center, WHITE, 1.0);
            if f {
                ui.selection_box(r, 4.0);
            }
            if current {
                // the little arrow pointing at the list
                ui.label("\u{25B6}", Font::Regular, 26.0, Rect::new(r.x + r.w + 12.0, r.y, 30.0, r.h), Align::Start, Align::Center, WHITE, 1.0);
            }
            if !blocked && ui.pressed(sel) {
                chosen_filter = Some(i);
            }
        }
        // online first (in game, studio, website), then everyone else by name
        let mut list: Vec<Friend> = if self.friends_filter == 0 { all.iter().filter(|f| f.presence > 0).cloned().collect() } else { all.clone() };
        let rank = |f: &Friend| match f.presence {
            2 => 0,
            3 => 1,
            1 => 2,
            _ => 3,
        };
        list.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        if list.is_empty() {
            ui.label(if self.friends_filter == 0 { s::NO_FRIENDS_ONLINE } else { s::NO_FRIENDS }, Font::Regular, 30.0, Rect::new(598.0, 283.0, 900.0, 78.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
        let focused = list.iter().position(|f| ui.is_focused(id("frrow", f.id)));
        if let Some(i) = focused {
            let top = i as f32 * 139.0;
            let target = if top - self.friends_scroll > 139.0 * 4.0 { top - 139.0 * 4.0 } else if top < self.friends_scroll { top } else { self.friends_scroll };
            self.friends_scroll += (target - self.friends_scroll) * 0.25;
        }
        ui.push_clip(Rect::new(588.0, 265.0, 470.0, 1040.0 - 265.0));
        let mut pressed = None;
        for (i, f) in list.iter().enumerate() {
            let r = Rect::new(598.0, 275.0 + i as f32 * 139.0 - self.friends_scroll, 439.0, 118.0);
            if r.y > 1080.0 || r.y + r.h < 200.0 {
                continue;
            }
            let sel = id("frrow", f.id);
            let fo = ui.selectable(sel, r);
            ui.rect(r, if fo { ED_BLUE } else { GREY_BUTTON }, if fo { 1.0 } else { 0.85 });
            let av = Rect::new(r.x + 10.0, r.y + 10.0, 100.0, 98.0);
            ui.rect(av, CHARACTER_BG, 1.0);
            if let Some(h) = self.headshots.get(&f.id).cloned() {
                ui.image(&h, av, WHITE, 1.0);
            }
            ui.image(roblox_icon(&self.cfg), Rect::new(r.x + 126.0, r.y + 30.0, 26.0, 26.0), WHITE, 1.0);
            let name = if f.name.is_empty() { format!("User {}", f.id) } else { f.name.clone() };
            let nm = ui.fit(&name, Font::Regular, 30.0, r.w - 175.0);
            ui.label(&nm, Font::Regular, 30.0, Rect::new(r.x + 164.0, r.y + 22.0, r.w - 175.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
            let (status, col) = presence_status(f);
            ui.image(CIRCLE_IMG, Rect::new(r.x + 131.0, r.y + 70.0, 18.0, 18.0), col, 1.0);
            let st = ui.fit(&status, Font::Regular, 26.0, r.w - 175.0);
            ui.label(&st, Font::Regular, 26.0, Rect::new(r.x + 159.0, r.y + 62.0, r.w - 175.0, 34.0), Align::Start, Align::Center, WHITE, 1.0);
            if fo {
                ui.selection_box(r, 4.0);
            }
            if !blocked && ui.pressed(sel) {
                pressed = Some(f.clone());
            }
        }
        ui.pop_clip();
        if !ui.focus_valid() {
            ui.set_focus(id("frfilter", self.friends_filter as u64));
        }
        // the focused friend's avatar
        let show = focused.and_then(|i| list.get(i)).or(list.first()).map(|f| f.id);
        if let Some(uid) = show {
            let at = Rect::new(1515.0 - 280.0, 610.0 - 280.0, 560.0, 560.0);
            match self.friend_renders.get(&uid).cloned() {
                Some(url) if !url.is_empty() => {
                    ui.image(&url, at, WHITE, 1.0);
                }
                Some(_) => {}
                None => {
                    if self.friend_render_req.insert(uid) {
                        if let Some(s) = self.session.clone() {
                            spawn(&self.tx, move || Msg::FriendRender(uid, s.avatar_url(uid, "avatar", "720x720").unwrap_or_default()));
                        }
                    }
                }
            }
            if !self.friend_renders.contains_key(&uid) || !ui.image_loaded(self.friend_renders.get(&uid).map(|s| s.as_str()).unwrap_or("")) {
                self.loading(ui, at.center());
            }
        }
        if blocked {
            return;
        }
        if let Some(i) = chosen_filter {
            audio.play("ButtonPress", 1.0);
            self.friends_filter = i;
            if let Some(f) = list.first() {
                ui.set_focus(id("frrow", f.id));
            }
        } else if let Some(f) = pressed {
            audio.play("ButtonPress", 1.0);
            let mut items = Vec::new();
            let name = if f.display_name.is_empty() { f.name.clone() } else { f.display_name.clone() };
            if let Some(place) = f.place_id {
                items.push(("JOIN GAME".to_string(), SideAction::JoinGame(f.id, name)));
                items.push(("VIEW GAME DETAILS".to_string(), SideAction::ViewDetails(place)));
            }
            items.push(("VIEW GAMER CARD".to_string(), SideAction::ViewGamerCard(f.id)));
            self.overlay = Some(Overlay::SideBar { items, t0: ui.time });
            ui.set_focus(id("sidebar", 0));
            audio.play("SideMenuSlideIn", 1.0);
        }
    }

    fn social_pane(&mut self, ui: &mut Ui, audio: &mut Audio, p: Rect, blocked: bool) {
        // the v2.0+ Shell has the list + render layout; v1.x keeps the friend grid
        if self.cfg.ui("Version") >= "v2.0" {
            self.friends_pane(ui, audio, blocked);
            return;
        }
        let Some(online) = self.online_friends() else {
            self.loading(ui, p.center());
            return;
        };
        if online.is_empty() {
            ui.image("rbxasset://textures/ui/Shell/Icons/FriendsIcon@1080.png", Rect::new(p.x + p.w * 0.5 - 148.0, p.y + 100.0, 296.0, 259.0), WHITE, 1.0);
            ui.label(s::NO_FRIENDS, Font::Regular, BUTTON, Rect::new(p.x + p.w * 0.5 - 400.0, p.y + 100.0 + 259.0 + 32.0, 800.0, 72.0), Align::Center, Align::Center, WHITE, 1.0);
            return;
        }
        ui.label("ONLINE FRIENDS", Font::Regular, SUBHEADER, Rect::new(p.x, p.y, 1200.0, 33.0), Align::Start, Align::Center, WHITE, 1.0);
        // ScrollingGrid 1438x610: 3 columns of 446x114, spacing (50, 10)
        let grid = Rect::new(p.x, p.y + 33.0, 1438.0, 610.0);
        for (i, f) in online.iter().take(DISPLAY_FRIEND_COUNT).enumerate() {
            let r = Rect::new(grid.x + (i % 3) as f32 * (446.0 + 50.0), grid.y + (i / 3) as f32 * (114.0 + 10.0), 446.0, 114.0);
            self.friend_item(ui, audio, id("friend", f.id), r, f, blocked);
        }
        if online.len() > DISPLAY_FRIEND_COUNT {
            let more = Rect::new(grid.x + grid.w - 100.0, grid.y + grid.h + 12.0, 100.0, 50.0);
            if more_button(ui, id("friendsmore", 0), more) && !blocked {
                audio.play("ButtonPress", 1.0);
                let t0 = ui.time;
                self.push(ui, Screen::Social { t0, scroll: 0.0 });
                audio.play("ScreenChange", 1.0);
            }
        }
    }

    /// SocialScreen: the full online friends list
    fn social_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Social { t0, scroll }) = self.stack.last() else { return };
        let (t0, mut scroll) = (*t0, *scroll);
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Online Friends");
        let online = self.online_friends().unwrap_or_default();
        let grid = Rect::new(sa.x, sa.y + 210.0, 1438.0, 744.0);
        if let Some(fi) = ui.focus.and_then(|f| online.iter().position(|fr| id("sfriend", fr.id) == f)) {
            let top = (fi / 3) as f32 * 124.0;
            let target = if top - scroll > grid.h - 114.0 { top - (grid.h - 114.0) } else if top < scroll { top } else { scroll };
            scroll += (target - scroll) * 0.25;
        }
        ui.push_clip(Rect::new(grid.x - 20.0, grid.y - 10.0, grid.w + 40.0, grid.h + 20.0));
        for (i, f) in online.iter().enumerate() {
            let r = Rect::new(grid.x + (i % 3) as f32 * 496.0, grid.y + (i / 3) as f32 * 124.0 - scroll, 446.0, 114.0);
            if r.y > grid.y + grid.h + 200.0 || r.y + r.h < grid.y - 200.0 {
                continue;
            }
            self.friend_item(ui, audio, id("sfriend", f.id), r, f, blocked);
        }
        ui.pop_clip();
        if !ui.focus_valid() {
            if let Some(f) = online.first() {
                ui.set_focus(id("sfriend", f.id));
            }
        }
        self.end_fade(ui, old);
        if let Some(Screen::Social { scroll: sc, .. }) = self.stack.last_mut() {
            *sc = scroll;
        }
        if !blocked && input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    // >> store pane

    fn balance(&self) -> Option<i64> {
        self.robux.filter(|r| *r >= 0).map(|r| r + self.fake_robux)
    }

    fn store_pane(&mut self, ui: &mut Ui, audio: &mut Audio, p: Rect, blocked: bool) {
        ui.label(s::ROBUX_STORE_DESCRIPTION, Font::Regular, TITLE, Rect::new(p.x, p.y, p.w, 50.0), Align::Start, Align::Center, WHITE, 1.0);
        // percent more is relative to the worst ROBUX-per-dollar package
        let ratio = |a: i64, price: &str| a as f32 / price.parse::<f32>().unwrap_or(1.0);
        let worst = PACKAGES.iter().map(|(a, pr)| ratio(*a, pr)).fold(f32::MAX, f32::min);
        let grid = Rect::new(p.x + 40.0, p.y + 65.0, 1620.0, 610.0);
        for (i, (amount, price)) in PACKAGES.iter().enumerate() {
            let r = Rect::new(grid.x + (i % 3) as f32 * (520.0 + 20.0), grid.y + (i / 3) as f32 * (285.0 + 20.0), 520.0, 285.0);
            let sel = id("robux", i as u64);
            let f = ui.selectable(sel, r);
            ui.nine_slice(SHADOW_IMG, Rect::new(r.x, r.y, r.w + 3.0, r.h + 3.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
            ui.image(&format!("rbxasset://textures/ui/Shell/Images/Robux/Robux0{}.png", i + 1), r, WHITE, 1.0);
            // RobuxIcon 80x80 at (5,5) with the amount to its right
            let icon = Rect::new(r.x + 5.0, r.y + 5.0, 80.0, 80.0);
            robux_icon(ui, &self.cfg, icon, false);
            ui.label(&fmt_num(*amount), Font::Bold, LARGE, Rect::new(icon.x + 90.0, icon.y, 400.0, 80.0), Align::Start, Align::Center, WHITE, 1.0);
            let more = ((ratio(*amount, price) / worst - 1.0) * 100.0).round() as i64;
            if more > 0 {
                ui.label(&format!("{more}% MORE"), Font::Bold, BUTTON, Rect::new(icon.x + 5.0, icon.y + 90.0, 300.0, BUTTON), Align::Start, Align::Start, GREEN_TEXT, 1.0);
            }
            // price: large text in the bottom-right corner, small "$" superscript
            let pw = ui.measure(price, Font::Regular, LARGE);
            let px = r.x + r.w - 15.0 - pw;
            let py = r.y + r.h - 12.0 - LARGE;
            ui.text(price, Font::Regular, LARGE, px, py, WHITE, 1.0);
            let dw = ui.measure("$", Font::Bold, MEDIUM);
            ui.text("$", Font::Bold, MEDIUM, px - dw, py + 15.0, WHITE, 1.0);
            if f {
                ui.selection_box(r, 7.0);
            }
            if !blocked && ui.pressed(sel) {
                audio.play("ButtonPress", 1.0);
                audio.play("OverlayOpen", 1.0);
                self.open_buy(*amount, price, i, ui.time);
            }
        }
        // RobuxBalanceButton with CurrencyWidget
        let text_w = self.currency_width(ui, "My Balance:");
        let bal = Rect::new(p.x, p.y + p.h - 100.0, 56.0 + 42.0 + text_w + 10.0, 75.0);
        let bid = id("robuxbalance", 0);
        let bf = ui.selectable(bid, bal);
        if bf {
            ui.rect(bal, GREY_SELECTION, 1.0);
            ui.image("rbxasset://textures/ui/Shell/Icons/HelpIconSmall.png", Rect::new(bal.x + 10.0, bal.y + 37.5 + 4.0 - 21.0, 42.0, 42.0), WHITE, 1.0);
        }
        self.currency_widget_as(ui, Vec2::new(bal.x + 10.0 + 42.0 + 10.0, bal.y + 37.5 - 30.0), "My Balance:");
        if !blocked && ui.pressed(bid) {
            audio.play("ButtonPress", 1.0);
            audio.play("OverlayOpen", 1.0);
            self.overlay = Some(Overlay::RobuxBalance { t0: ui.time });
        }
    }

    fn currency_width(&self, ui: &Ui, label: &str) -> f32 {
        let v = self.balance().map(fmt_num).unwrap_or_default();
        ui.measure(label, Font::Regular, HEADER) + 17.0 + 46.0 + 5.0 + ui.measure(&v, Font::Regular, HEADER)
    }

    /// CurrencyWidget: "ROBUX:" + outlined icon + green balance
    fn currency_widget(&self, ui: &mut Ui, at: Vec2) {
        self.currency_widget_as(ui, at, "ROBUX:");
    }

    fn currency_widget_as(&self, ui: &mut Ui, at: Vec2, label: &str) {
        let tw = ui.measure(label, Font::Regular, HEADER);
        ui.label(label, Font::Regular, HEADER, Rect::new(at.x, at.y, tw, 60.0), Align::Start, Align::Center, WHITE, 1.0);
        let icon = Rect::new(at.x + tw + 17.0, at.y + 30.0 - 23.0, 46.0, 46.0);
        robux_icon(ui, &self.cfg, icon, true);
        match self.balance() {
            Some(b) => ui.label(&fmt_num(b), Font::Regular, HEADER, Rect::new(icon.x + 51.0, at.y - 2.0, 600.0, 60.0), Align::Start, Align::Center, GREEN_TEXT, 1.0),
            None if self.robux.is_some() => {}
            None => ui.image_rotated(SPINNER, Vec2::new(icon.x + 51.0 + 50.0, at.y + 30.0), Vec2::new(50.0, 50.0), ui.time * std::f32::consts::TAU, 1.0),
        }
    }

    // >> game detail screen

    fn game_detail(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::GameDetail { universe, t0, scroll, target, .. }) = self.stack.last() else { return };
        let (universe, t0, mut scroll, mut target) = (*universe, *t0, *scroll, *target);
        let d = self.details.get(&universe).cloned();
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, d.as_ref().map(|d| d.name.as_str()).unwrap_or(""));
        let Some(d) = d else {
            self.loading(ui, Vec2::new(960.0, 540.0));
            self.end_fade(ui, old);
            if !blocked && input.has(Btn::B) {
                audio.play("ButtonPress", 1.0);
                self.pop(ui);
            }
            return;
        };

        if let (Some(r), "On") = (&d.age_rating, self.cfg.ui("Age Ratings")) {
            let tw = ui.measure(&s::word(&d.name), Font::Light, HEADER).min(sa.w - 400.0);
            ui.label(r, Font::Regular, SUBHEADER, Rect::new(sa.x + 16.0 + tw + 30.0, sa.y + 122.0, 600.0, 35.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }

        // ContentManager: columns laid out left to right at y=210, 622 tall
        let cy = sa.y + 210.0;
        let ch = 622.0;
        ui.push_clip(Rect::new(sa.x - 30.0, cy - 30.0, sa.w + 60.0, ch + 60.0));
        let tr = ui.push_translate(Vec2::new(-scroll, 0.0));
        let mut x = sa.x + 8.0;
        let icon = Rect::new(x, cy + (ch - 566.0) / 2.0, 566.0, 566.0);
        ui.rect(icon, [0.0; 3], 1.0);
        if let Some(u) = self.icons.get(&universe).cloned() {
            ui.image(&u, icon, WHITE, 1.0);
        }
        x += 566.0 + 38.0;
        // RatingDescriptionContainer 394
        ui.label("RATING & DESCRIPTION", Font::Regular, SUBHEADER, Rect::new(x, cy, 394.0, 33.0), Align::Start, Align::Center, WHITE, 1.0);
        // VoteView: thumbs buttons around a 203px VoteFrame bar at (88, 34), rating text + counts below
        let vy = cy + 33.0 + 5.0;
        let bar = Rect::new(x + 88.0, vy + 34.0, 203.0, 16.0);
        let no_vote = d.up_votes == 0 && d.down_votes == 0;
        ui.rect(bar, GREY_TEXT, 1.0);
        if !no_vote {
            let up = d.up_votes as f32 / (d.up_votes + d.down_votes) as f32;
            ui.image_uv("rbxasset://textures/ui/Shell/Icons/RatingBar@1080.png", bar, [0.0, 0.0, 1.0, 1.0], 0.0);
            ui.rect(Rect::new(bar.x, bar.y, bar.w * up, bar.h), GREEN_TEXT, 1.0);
            ui.rect(Rect::new(bar.x + bar.w * up, bar.y, bar.w * (1.0 - up), bar.h), RED_TEXT, 1.0);
        }
        let up_btn = Rect::new(bar.x - 66.0 - 12.0, bar.y + 8.0 - 33.0, 66.0, 66.0);
        let down_btn = Rect::new(bar.x + bar.w + 12.0, up_btn.y, 66.0, 66.0);
        let (up_sel, down_sel) = (id("detail", 5), id("detail", 6));
        let uf = ui.selectable(up_sel, up_btn);
        let df = ui.selectable(down_sel, down_btn);
        let up_img = if d.user_vote == Some(true) { "rbxasset://textures/ui/Shell/Icons/ThumbsUpFilled@1080.png" } else { "rbxasset://textures/ui/Shell/Icons/ThumbsUp@1080.png" };
        let down_img = if d.user_vote == Some(false) { "rbxasset://textures/ui/Shell/Icons/ThumbsDownFilled@1080.png" } else { "rbxasset://textures/ui/Shell/Icons/ThumbsDown@1080.png" };
        ui.image(up_img, Rect::new(up_btn.x + 9.0, up_btn.y + 9.0, 48.0, 48.0), WHITE, 1.0);
        ui.image(down_img, Rect::new(down_btn.x + 9.0, down_btn.y + 9.0, 48.0, 48.0), WHITE, 1.0);
        if uf {
            ui.selection_box(up_btn, 0.0);
        }
        if df {
            ui.selection_box(down_btn, 0.0);
        }
        let text_y = vy + 114.0 - 20.0;
        let (rating, rcol) = if no_vote {
            ("First to rate!", GREY_TEXT)
        } else {
            match d.user_vote {
                Some(true) => ("Liked", GREEN_TEXT),
                Some(false) => ("Disliked", RED_TEXT),
                None => ("", GREY_TEXT),
            }
        };
        ui.label(rating, Font::Bold, SUBHEADER, Rect::new(bar.x + bar.w * 0.5 - 150.0, text_y, 300.0, 0.0), Align::Center, Align::Center, rcol, 1.0);
        if !no_vote {
            ui.label(&fmt_short(d.up_votes), Font::Regular, SUBHEADER, Rect::new(x + 43.0 - 100.0, text_y, 200.0, 0.0), Align::Center, Align::Center, GREEN_TEXT, 1.0);
            ui.label(&fmt_short(d.down_votes), Font::Regular, SUBHEADER, Rect::new(x + 336.0 - 100.0, text_y, 200.0, 0.0), Align::Center, Align::Center, RED_TEXT, 1.0);
        }
        let lb = vy + 114.0;
        ui.rect(Rect::new(x, lb, 362.0, 2.0), LINE_BREAK, 1.0);
        ui.paragraph(&d.description, Font::Light, SMALL_TITLE, Rect::new(x, lb + 22.0, 380.0, 430.0), WHITE, 1.0, 16);
        x += 394.0 + 32.0;
        ui.rect(Rect::new(x, cy + 33.0, 2.0, 566.0), LINE_BREAK, 1.0);
        x += 2.0 + 52.0;
        // ThumbsContainer 630
        let thumbs_x = x;
        ui.label("GAME IMAGES", Font::Regular, SUBHEADER, Rect::new(x, cy, 630.0, 33.0), Align::Start, Align::Center, WHITE, 1.0);
        let main = Rect::new(x, cy + 33.0, 630.0, 374.0);
        let sm1 = Rect::new(x, main.y + 374.0 + 8.0, 310.0, 184.0);
        let sm2 = Rect::new(x + 320.0, main.y + 374.0 + 8.0, 310.0, 184.0);
        for (k, r) in [main, sm1, sm2].iter().enumerate() {
            ui.rect(*r, [0.0; 3], 1.0);
            if let Some(u) = d.screenshots.get(k) {
                ui.image_cover(u, *r, 1.0);
            }
        }
        let shot_sel = id("detail", 3);
        let shot_f = ui.selectable(shot_sel, main);
        if shot_f {
            // edgeSelectionImage: 16px outside
            ui.selection_box(main, 16.0);
        }
        x += 630.0 + 52.0;
        // MoreDetailsContainer 386
        let details_x = x;
        ui.label("MORE DETAILS", Font::Regular, SUBHEADER, Rect::new(x, cy, 386.0, 33.0), Align::Start, Align::Center, WHITE, 1.0);
        let mut y = cy + 33.0 + 48.0;
        for (k, v) in [("Last Updated", d.updated.clone()), ("Creation Date", d.created.clone()), ("Max Players", d.max_players.to_string()), ("Created By", d.creator.clone())] {
            ui.label(k, Font::Regular, SMALL_TITLE, Rect::new(x, y - 12.0, 386.0, 24.0), Align::Start, Align::Center, WHITE, 1.0);
            if k == "Created By" {
                let v = ui.fit(&v, Font::Light, TITLE, 330.0);
                ui.image("rbxasset://textures/ui/Shell/Icons/RobloxIcon32.png", Rect::new(x, y + 17.0, 32.0, 32.0), WHITE, 1.0);
                ui.label(&v, Font::Light, TITLE, Rect::new(x + 40.0, y + 12.0, 346.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
            } else {
                let v = ui.fit(&v, Font::Light, TITLE, 380.0);
                ui.label(&v, Font::Light, TITLE, Rect::new(x, y + 12.0, 386.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
            }
            y += 104.0;
        }
        let rep = Rect::new(x, cy + 33.0 + 566.0 - 2.0 - 95.0, 91.0, 91.0);
        let rep_sel = id("detail", 4);
        let rf = ui.selectable(rep_sel, rep);
        ui.rect(rep, if rf { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
        ui.image("rbxasset://textures/ui/Shell/Icons/ReportIcon@1080.png", Rect::new(rep.x + 19.5, rep.y + 24.0, 52.0, 43.0), WHITE, 1.0);
        ui.label("Report Game", Font::Light, TITLE, Rect::new(rep.x + rep.w + 16.0, rep.y, 300.0, rep.h), Align::Start, Align::Center, WHITE, 1.0);
        ui.rect(Rect::new(x + 107.0, cy + 33.0 + 566.0 - 2.0, 279.0, 2.0), LINE_BREAK, 1.0);
        if rf {
            ui.selection_box(rep, 7.0);
        }
        ui.pop_translate(tr);
        ui.pop_clip();

        // ContentManager:TweenContent — page to the right when selection leaves the first screen
        if shot_f || rf {
            let col_right = if rf { details_x + 386.0 } else { thumbs_x + 630.0 };
            if col_right > sa.x + sa.w {
                target = sa.w - 8.0;
            }
        } else if [1, 2, 5, 6].iter().any(|k| ui.focus == Some(id("detail", *k))) {
            target = 0.0;
        }
        target = target.min((details_x + 386.0 + 40.0 - (sa.x + sa.w)).max(0.0));
        scroll += (target - scroll) * 0.15;

        // Play / Favorite at the bottom
        let play = Rect::new(sa.x, sa.y + 1001.0 - 77.0, 228.0, 72.0);
        // Utility.ResizeButtonWithText: the button grows to fit "FAVORITED" plus the star
        let fav_w = if d.is_favorited { (16.0 + 32.0 + 12.0 + ui.measure("FAVORITED", Font::Regular, BUTTON) + 13.0 * 2.0).max(228.0) } else { 228.0 };
        let fav = Rect::new(sa.x + 238.0, sa.y + 1001.0 - 77.0, fav_w, 72.0);
        let (play_sel, fav_sel) = (id("detail", 1), id("detail", 2));
        if !ui.focus_valid() {
            ui.set_focus(play_sel);
        }
        let pf = ui.selectable(play_sel, play);
        ui.nine_slice(BUTTON_IMG, play, [8.0, 8.0, 24.0, 24.0], if pf { GREEN_SELECTED } else { GREEN_BUTTON }, 1.0);
        ui.label("PLAY", Font::Regular, BUTTON, play, Align::Center, Align::Center, if pf { TEXT_SELECTED } else { WHITE }, 1.0);
        let ff = ui.selectable(fav_sel, fav);
        ui.nine_slice(BUTTON_IMG, fav, [8.0, 8.0, 24.0, 24.0], if ff { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
        let tc = if ff { TEXT_SELECTED } else { WHITE };
        if d.is_favorited {
            ui.image("rbxasset://textures/ui/Shell/Icons/FavoriteStar@1080.png", Rect::new(fav.x + 16.0, fav.y + 20.5, 32.0, 31.0), WHITE, 1.0);
            ui.label("FAVORITED", Font::Regular, BUTTON, Rect::new(fav.x + 60.0, fav.y, fav.w - 60.0 - 13.0, fav.h), Align::Center, Align::Center, tc, 1.0);
        } else {
            ui.label("FAVORITE", Font::Regular, BUTTON, fav, Align::Center, Align::Center, tc, 1.0);
        }
        self.end_fade(ui, old);
        if let Some(Screen::GameDetail { scroll: sc, target: tg, .. }) = self.stack.last_mut() {
            *sc = scroll;
            *tg = target;
        }
        if blocked {
            return;
        }
        if ui.pressed(play_sel) {
            audio.play("ButtonPress", 1.0);
            self.launch(ui, JoinTarget::Place(d.place_id), d.name.clone(), d.creator.clone(), Some(d.place_id));
        } else if ui.pressed(fav_sel) {
            audio.play("ButtonPress", 1.0);
            if let Some(sess) = self.session.clone() {
                let v = !d.is_favorited;
                if let Some(dd) = self.details.get_mut(&universe) {
                    dd.is_favorited = v;
                }
                spawn(&self.tx, move || {
                    let ok = sess.set_favorite(universe, v).is_ok();
                    Msg::Favorited(universe, v, ok)
                });
            }
        } else if ui.pressed(up_sel) || ui.pressed(down_sel) {
            audio.play("ButtonPress", 1.0);
            let want = ui.pressed(up_sel);
            if !d.can_vote {
                self.error(ui, audio, s::vote_play_game());
            } else if !self.vote_busy {
                // pressing the active thumb clears the vote
                let new_vote = if d.user_vote == Some(want) { None } else { Some(want) };
                if let Some(sess) = self.session.clone() {
                    self.vote_busy = true;
                    let prev = d.user_vote;
                    spawn(&self.tx, move || {
                        let ok = sess.set_vote(universe, new_vote).is_ok();
                        Msg::Voted(universe, new_vote, prev, ok)
                    });
                }
            }
        } else if ui.pressed(rep_sel) {
            audio.play("ButtonPress", 1.0);
            audio.play("OverlayOpen", 1.0);
            self.overlay = Some(Overlay::Report { place: d.place_id, t0: ui.time });
        } else if ui.pressed(shot_sel) && !d.screenshots.is_empty() {
            audio.play("ButtonPress", 1.0);
            audio.play("OverlayOpen", 1.0);
            self.overlay = Some(Overlay::Images { urls: d.screenshots.clone(), index: 0, prev: None, t0: ui.time });
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    // >> settings screen

    fn settings(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        let Some(Screen::Settings { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Settings");
        // four 394x612 tiles 40px apart at y=238
        let tiles = [
            ("Account", "rbxasset://textures/ui/Shell/Icons/AccountIcon.png", Vec2::new(256.0, 256.0)),
            ("Switch Profile", "rbxasset://textures/ui/Shell/Icons/ProfileIcon.png", Vec2::new(224.0, 255.0)),
            ("Overscan", "rbxasset://textures/ui/Shell/Icons/TVIcon.png", Vec2::new(256.0, 181.0)),
            ("Help", "rbxasset://textures/ui/Shell/Icons/HelpIcon.png", Vec2::new(256.0, 256.0)),
        ];
        let mut chosen = None;
        for (i, (label, icon, isz)) in tiles.iter().enumerate() {
            let r = Rect::new(sa.x + i as f32 * (394.0 + 40.0), sa.y + 238.0, 394.0, 612.0);
            let sel = id("settings", i as u64);
            let f = ui.selectable(sel, r);
            ui.rect(r, WHITE, if f { TEXTBOX_SELECTED_ALPHA } else { TEXTBOX_DEFAULT_ALPHA });
            ui.image(icon, Rect::new(r.x + (r.w - isz.x) * 0.5, r.y + (r.h - isz.y) * 0.5, isz.x, isz.y), WHITE, 1.0);
            ui.label(label, Font::Regular, TITLE, Rect::new(r.x, r.y + r.h - 96.0 - 21.0, r.w, 42.0), Align::Center, Align::Center, WHITE, 1.0);
            if f {
                ui.selection_box(r, 7.0);
            }
            if !blocked && ui.pressed(sel) {
                chosen = Some(i);
            }
        }
        if !ui.focus_valid() {
            ui.set_focus(id("settings", 0));
        }
        hint(ui, &glyph('Y'), "ACHIEVEMENTS");
        let fast_boot = self.cfg.ui("Fast Boot") == "On";
        if fast_boot {
            // second hint, left of the Y hint
            let yw = 83.0 + 8.0 + ui.measure(&s::word("ACHIEVEMENTS"), Font::Regular, TITLE);
            hint_at(ui, &glyph('X'), "DISABLE FAST BOOT", SAFE.x + SAFE.w - yw - 60.0);
        }
        self.end_fade(ui, old);
        if blocked {
            return;
        }
        if fast_boot && input.has(Btn::X) {
            audio.play("ButtonPress", 1.0);
            self.cfg.ui.insert("Fast Boot".into(), "Off".into());
            ev.config_changed = true;
            self.overlay = Some(Overlay::Error {
                def: s::info("Fast Boot", "Fast boot is now disabled. On the next launch you will get the option to configure settings or boot straight into the hub."),
                t0: ui.time,
            });
            return;
        }
        if input.has(Btn::Y) {
            audio.play("ScreenChange", 1.0);
            let t0 = ui.time;
            self.push(ui, Screen::Achievements { t0 });
            return;
        }
        if let Some(i) = chosen {
            audio.play("ButtonPress", 1.0);
            match i {
                0 => {
                    let t0 = ui.time;
                    self.push(ui, Screen::Account { t0 });
                    audio.play("ScreenChange", 1.0);
                }
                2 => {
                    let edges = Vec2::new(self.cfg.overscan[0], self.cfg.overscan[1]);
                    self.push(ui, Screen::Overscan { edges, shown: edges, last: -1.0 });
                    audio.play("ScreenChange", 1.0);
                }
                1 => {
                    let t0 = ui.time;
                    self.want_account_headshots();
                    self.push(ui, Screen::Profiles { t0, scroll: 0.0 });
                    audio.play("ScreenChange", 1.0);
                }
                _ => {
                    let t0 = ui.time;
                    self.push(ui, Screen::Help { t0 });
                    audio.play("ScreenChange", 1.0);
                }
            }
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    // >> account screen

    fn account(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        let Some(Screen::Account { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Account Settings");
        let user = self.user.clone().unwrap_or_default();
        // AccountLinkingView at (75, 275) 765x630
        let lv = Vec2::new(sa.x + 75.0, sa.y + 275.0);
        ui.label("ACCOUNT LINKING", Font::Regular, SUBHEADER, Rect::new(lv.x + 40.0, lv.y - 14.0, 700.0, 28.0), Align::Start, Align::Center, WHITE, 1.0);
        let pc = Rect::new(lv.x + 40.0, lv.y + 25.0, 300.0, 300.0);
        ui.rect(pc, CHARACTER_BG, 1.0);
        ui.image("rbxasset://textures/ui/Shell/Icons/DefaultProfileIcon.png", pc.inset(60.0), WHITE, 1.0);
        ui.rect(Rect::new(lv.x + 354.0, lv.y + 174.0, 58.0, 4.0), WHITE, 1.0);
        let pr = Rect::new(lv.x + 425.0, lv.y + 25.0, 300.0, 300.0);
        ui.rect(pr, CHARACTER_BG, 1.0);
        if let Some(a) = self.avatar.clone().filter(|a| !a.is_empty()) {
            ui.image(&a, pr, WHITE, 1.0);
        }
        ui.label("Windows", Font::Regular, SUBHEADER, Rect::new(lv.x + 40.0, lv.y + 335.0, 300.0, 28.0), Align::Center, Align::Center, WHITE, 1.0);
        ui.label(&user.name, Font::Regular, SUBHEADER, Rect::new(lv.x + 425.0, lv.y + 335.0, 300.0, 28.0), Align::Center, Align::Center, WHITE, 1.0);
        ui.paragraph(&s::LINKED_AS.replace("%s", &user.name), Font::Regular, SUBHEADER, Rect::new(lv.x + 40.0, lv.y + 395.0, 686.0, 120.0), GREY_TEXT, 1.0, 4);
        let unlink = Rect::new(lv.x + 220.0, lv.y + 520.0, 320.0, 80.0);
        let uid_ = id("account", 0);
        let uf = ui.selectable(uid_, unlink);
        ui.nine_slice(BUTTON_IMG, unlink, [8.0, 8.0, 24.0, 24.0], if uf { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
        ui.label("SIGN OUT", Font::Regular, BUTTON, unlink, Align::Center, Align::Center, if uf { TEXT_SELECTED } else { WHITE }, 1.0);
        // ScreenDivide + settings column (GameplaySettingsView position)
        ui.rect(Rect::new(sa.x + 840.0, sa.y + 275.0, 2.0, 615.0), PAGE_DIVIDE, 1.0);
        let gx = sa.x + 840.0 + 40.0;
        ui.label("LAUNCHER SETTINGS", Font::Regular, SUBHEADER, Rect::new(gx, sa.y + 261.0, 700.0, 28.0), Align::Start, Align::Center, WHITE, 1.0);
        let handler = launch::handler_name_cached(t0);
        let launcher_label = match self.cfg.launch_mode {
            LaunchMode::ProtocolHandler => format!("GAMES OPEN WITH: {}", handler.to_uppercase()),
            LaunchMode::CustomExe => {
                let n = std::path::Path::new(&self.cfg.bootstrapper_path).file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default();
                format!("GAMES OPEN WITH: {n} (DIRECT)")
            }
        };
        let items = [
            launcher_label,
            format!("BACKGROUND FILTER: {}", if self.cfg.background_filter { "ON" } else { "OFF" }),
            format!("SHADOWS: {}", if self.cfg.shadows { "ON" } else { "OFF" }),
            format!("MUSIC VOLUME: {}%", (self.cfg.music_volume * 100.0).round()),
            format!("FULLSCREEN: {}", if self.cfg.fullscreen { "ON" } else { "OFF" }),
            format!("BUTTON ICONS: {}", self.cfg.icons.label()),
        ];
        let mut chosen = None;
        for (i, label) in items.iter().enumerate() {
            let r = Rect::new(gx, sa.y + 310.0 + i as f32 * 90.0, 720.0, 72.0);
            let sel = id("account", 1 + i as u64);
            let f = ui.selectable(sel, r);
            ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], if f { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
            let l = ui.fit(label, Font::Regular, BUTTON * 0.85, r.w - 40.0);
            ui.label(&l, Font::Regular, BUTTON * 0.85, r.inset(20.0), Align::Start, Align::Center, if f { TEXT_SELECTED } else { WHITE }, 1.0);
            if !blocked && ui.pressed(sel) {
                chosen = Some(i);
            }
        }
        if !ui.focus_valid() {
            ui.set_focus(uid_);
        }
        self.end_fade(ui, old);
        if blocked {
            return;
        }
        if ui.pressed(uid_) {
            audio.play("ButtonPress", 1.0);
            audio.play("OverlayOpen", 1.0);
            self.overlay = Some(Overlay::Confirm {
                title: "Sign Out".into(),
                phrase: "Are you sure you want to sign out? The saved cookie will be removed.".into(),
                image: None,
                action: Confirm::SignOut,
                t0: ui.time,
            });
        } else if let Some(i) = chosen {
            audio.play("ButtonPress", 1.0);
            match i {
                0 => {
                    // cycle: system handler -> detected bootstrappers -> back
                    let options = detect_bootstrappers();
                    let cur = if self.cfg.launch_mode == LaunchMode::ProtocolHandler { None } else { options.iter().position(|p| *p == self.cfg.bootstrapper_path) };
                    let next = cur.map(|k| k + 1).unwrap_or(0);
                    if next < options.len() {
                        self.cfg.launch_mode = LaunchMode::CustomExe;
                        self.cfg.bootstrapper_path = options[next].clone();
                    } else {
                        self.cfg.launch_mode = LaunchMode::ProtocolHandler;
                    }
                }
                1 => self.cfg.background_filter = !self.cfg.background_filter,
                2 => self.cfg.shadows = !self.cfg.shadows,
                3 => self.cfg.music_volume = (((self.cfg.music_volume * 4.0).round() as i32 + 1) % 5) as f32 / 4.0,
                5 => {
                    self.cfg.icons = self.cfg.icons.next();
                    crate::input::set_icon_set(self.cfg.icons);
                }
                _ => {
                    self.cfg.fullscreen = !self.cfg.fullscreen;
                    ev.toggle_fullscreen = true;
                }
            }
            ev.config_changed = true;
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    // >> overscan screen

    fn overscan_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        // full GUI-root space, not the safe area
        let full = Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, ui.screen.x / ui.scale, ui.screen.y / ui.scale);
        ui.rect(full, [3.0 / 255.0; 3], 1.0);
        ui.image("rbxasset://textures/ui/Shell/ScreenAdjustment/Background.png", full, WHITE, 1.0);
        ui.label("Screen Size", Font::Light, HEADER, Rect::new(230.0, 175.0, 1000.0, 50.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.label(s::RESIZE_PROMPT, Font::Regular, BUTTON, Rect::new(230.0, 225.0, 1400.0, 40.0), Align::Start, Align::Center, WHITE, 1.0);
        let ctrl = Rect::new(960.0 - 299.5, 540.0 - 202.0, 599.0, 404.0);
        ui.image("rbxasset://textures/ui/Shell/ScreenAdjustment/Controller@1080.png", ctrl, WHITE, 1.0);
        let line = Rect::new(ctrl.x + ctrl.w * 0.73, ctrl.y + ctrl.h * 0.545, ctrl.w * 0.4, 1.0);
        ui.rect(line, WHITE, 1.0);
        ui.label(s::RESIZE_INPUT_HINT, Font::Regular, BUTTON, Rect::new(line.x + line.w + 3.0, line.y - 20.0, 300.0, 40.0), Align::Start, Align::Center, WHITE, 1.0);
        let accept = Rect::new(960.0 + 25.0, 810.0, 65.0, 65.0);
        ui.image(&glyph('A'), accept, WHITE, 1.0);
        ui.label("ACCEPT", Font::Regular, BUTTON, Rect::new(accept.x + 70.0, accept.y - 3.0, 300.0, 65.0), Align::Start, Align::Center, WHITE, 1.0);
        let rw = ui.measure("RESET", Font::Regular, BUTTON);
        let reset = Rect::new(960.0 - rw - 25.0 - 65.0 - 5.0, 810.0, 65.0, 65.0);
        ui.image(&glyph('X'), reset, WHITE, 1.0);
        ui.label("RESET", Font::Regular, BUTTON, Rect::new(reset.x + 70.0, reset.y - 3.0, 300.0, 65.0), Align::Start, Align::Center, WHITE, 1.0);

        let Some(Screen::Overscan { edges, shown, last }) = self.stack.last_mut() else { return };
        let dt = if *last < 0.0 { 0.0 } else { (ui.time - *last).min(0.1) };
        *last = ui.time;
        if !blocked {
            // right stick resizes continuously; the d-pad/arrows nudge 2px per step
            let rs = input.right_stick;
            if rs != Vec2::ZERO {
                *edges = (*edges + Vec2::new(rs.x, rs.y) * 0.12 * dt).clamp(Vec2::splat(0.85), Vec2::ONE);
            }
            let mut push = Vec2::ZERO;
            for b in &input.pressed {
                match b {
                    Btn::Up => push.y -= 2.0,
                    Btn::Down => push.y += 2.0,
                    Btn::Left => push.x -= 2.0,
                    Btn::Right => push.x += 2.0,
                    _ => {}
                }
            }
            *edges = (*edges + push * 2.0 / ui.screen).clamp(Vec2::splat(0.85), Vec2::ONE);
            if input.has(Btn::X) {
                audio.play("ButtonPress", 1.0);
                *edges = Vec2::splat(0.9);
            }
        }
        // the frame glides toward the target instead of jumping
        *shown += (*edges - *shown) * (1.0 - (-14.0 * dt).exp());
        let e = *edges;
        let shown = *shown;
        let size = Vec2::new(full.w, full.h) * shown;
        let er = Rect::new(full.x + (full.w - size.x) * 0.5, full.y + (full.h - size.y) * 0.5, size.x, size.y);
        ui.nine_slice("rbxasset://textures/ui/Shell/ScreenAdjustment/ScreenRangeOverlay.png", er.inset(-1.0), [21.0, 21.0, 41.0, 41.0], WHITE, 1.0);
        let arrow = "rbxasset://textures/ui/Shell/ScreenAdjustment/ScreenAdjustmentArrow.png";
        let corners = [Vec2::new(er.x, er.y), Vec2::new(er.x + er.w - 95.0, er.y), Vec2::new(er.x + er.w - 95.0, er.y + er.h - 95.0), Vec2::new(er.x, er.y + er.h - 95.0)];
        for (i, c) in corners.iter().enumerate() {
            ui.image_rotated(arrow, *c + Vec2::splat(47.5), Vec2::splat(95.0), std::f32::consts::FRAC_PI_2 * i as f32, 1.0);
        }
        if blocked {
            return;
        }
        if input.has(Btn::A) || input.clicked {
            audio.play("ButtonPress", 1.0);
            self.cfg.overscan = [e.x, e.y];
            ev.config_changed = true;
            self.pop(ui);
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    fn want_account_headshots(&mut self) {
        let ids: Vec<u64> = self.accounts.iter().map(|a| a.id).filter(|i| *i != 0 && !self.headshots.contains_key(i)).collect();
        if ids.is_empty() {
            return;
        }
        if let Some(s) = self.session.clone() {
            spawn(&self.tx, move || Msg::Headshots(s.headshots(&ids).unwrap_or_default()));
        }
    }

    /// Switch Profile: saved accounts as SettingsScreen-style tiles, plus Add Account.
    /// A = sign in as that account, X = make it the default, Y = remove it.
    fn profiles(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool, ev: &mut ShellEvents) {
        let Some(Screen::Profiles { t0, scroll }) = self.stack.last() else { return };
        let (t0, mut scroll) = (*t0, *scroll);
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Switch Profile");
        let (tw, gap) = (394.0, 40.0);
        let count = self.accounts.len() + 1;
        if let Some(fi) = ui.focus.and_then(|f| (0..count).find(|i| id("profile", *i as u64) == f)) {
            let left = fi as f32 * (tw + gap);
            let target = if left + tw - scroll > sa.w { left + tw - sa.w } else if left < scroll { left } else { scroll };
            scroll += (target - scroll) * 0.25;
        }
        let current = self.user.as_ref().map(|u| u.id);
        let mut chosen = None;
        let mut focused_account = None;
        ui.push_clip(Rect::new(sa.x - 20.0, sa.y + 200.0, sa.w + 40.0, 700.0));
        for i in 0..count {
            let r = Rect::new(sa.x + i as f32 * (tw + gap) - scroll, sa.y + 238.0, tw, 612.0);
            let sel = id("profile", i as u64);
            let f = ui.selectable(sel, r);
            ui.rect(r, WHITE, if f { TEXTBOX_SELECTED_ALPHA } else { TEXTBOX_DEFAULT_ALPHA });
            if let Some(acc) = self.accounts.get(i) {
                let pic = Rect::new(r.x + (r.w - 256.0) * 0.5, r.y + 120.0, 256.0, 256.0);
                ui.rect(pic, CHARACTER_BG, 1.0);
                match self.headshots.get(&acc.id).cloned() {
                    Some(h) => {
                        ui.image(&h, pic, WHITE, 1.0);
                    }
                    None => {
                        ui.image("rbxasset://textures/ui/Shell/Icons/DefaultProfileIcon.png", pic.inset(64.0), WHITE, 1.0);
                    }
                }
                let name = if acc.display_name.is_empty() { "Saved account".to_string() } else { acc.display_name.clone() };
                let name = ui.fit(&name, Font::Regular, TITLE, r.w - 30.0);
                ui.label(&name, Font::Regular, TITLE, Rect::new(r.x, r.y + r.h - 96.0 - 21.0 - 30.0, r.w, 42.0), Align::Center, Align::Center, WHITE, 1.0);
                if !acc.name.is_empty() {
                    let un = ui.fit(&format!("@{}", acc.name), Font::Regular, SMALL_TITLE, r.w - 30.0);
                    ui.label(&un, Font::Regular, SMALL_TITLE, Rect::new(r.x, r.y + r.h - 96.0 + 14.0, r.w, 24.0), Align::Center, Align::Center, LIGHT_GREY_TEXT, 1.0);
                }
                if self.cfg.default_account == acc.id && acc.id != 0 {
                    ui.image("rbxasset://textures/ui/Shell/Icons/FavoriteStar@1080.png", Rect::new(r.x + 16.0, r.y + 16.0, 32.0, 31.0), WHITE, 1.0);
                    ui.label("DEFAULT", Font::Regular, SMALL_TITLE, Rect::new(r.x + 56.0, r.y + 16.0, 200.0, 31.0), Align::Start, Align::Center, WHITE, 1.0);
                }
                if current == Some(acc.id) {
                    ui.image("rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png", Rect::new(r.x + r.w - 56.0, r.y + 16.0, 40.0, 40.0), WHITE, 1.0);
                }
                if f {
                    focused_account = Some(i);
                }
            } else {
                ui.image("rbxasset://textures/ui/Shell/Icons/GamePlusIcon.png", Rect::new(r.x + (r.w - 160.0) * 0.5, r.y + (r.h - 160.0) * 0.5 - 20.0, 160.0, 160.0), WHITE, 1.0);
                ui.label("Add Account", Font::Regular, TITLE, Rect::new(r.x, r.y + r.h - 96.0 - 21.0, r.w, 42.0), Align::Center, Align::Center, WHITE, 1.0);
            }
            if f {
                ui.selection_box(r, 7.0);
            }
            if !blocked && ui.pressed(sel) {
                chosen = Some(i);
            }
        }
        ui.pop_clip();
        if !ui.focus_valid() {
            ui.set_focus(id("profile", 0));
        }
        if focused_account.is_some() {
            hint(ui, &glyph('X'), "SET AS DEFAULT");
        }
        self.end_fade(ui, old);
        if let Some(Screen::Profiles { scroll: sc, .. }) = self.stack.last_mut() {
            *sc = scroll;
        }
        if blocked {
            return;
        }
        if let Some(i) = chosen {
            audio.play("ButtonPress", 1.0);
            match self.accounts.get(i).cloned() {
                Some(acc) if current == Some(acc.id) => {
                    self.pop(ui);
                }
                Some(acc) => {
                    self.overlay = Some(Overlay::Busy { text: format!("Signing in as {}...", if acc.display_name.is_empty() { "saved account".into() } else { acc.display_name.clone() }) });
                    self.adding_account = false;
                    self.begin_auth(acc.cookie);
                }
                None => {
                    self.adding_account = true;
                    let t0 = ui.time;
                    self.push(ui, Screen::QuickLogin { t0 });
                    audio.play("ScreenChange", 1.0);
                }
            }
        } else if let (Some(i), true) = (focused_account, ui.activated_x) {
            let acc_id = self.accounts[i].id;
            if acc_id != 0 {
                self.cfg.default_account = acc_id;
                ev.config_changed = true;
                audio.play("PopUp", 1.0);
            }
        } else if let (Some(i), true) = (focused_account, input.has(Btn::Y)) {
            if current != Some(self.accounts[i].id) {
                audio.play("ButtonPress", 1.0);
                let removed = self.accounts.remove(i);
                if self.cfg.default_account == removed.id {
                    self.cfg.default_account = current.unwrap_or(0);
                    ev.config_changed = true;
                }
                config::save_accounts(&self.accounts);
            } else {
                self.error(ui, audio, s::info("Remove Account", "Switch to another account before removing the one you are signed in with."));
            }
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    /// Help: controller diagram and the keyboard/mouse equivalents.
    /// Tabs shown for the configured version (Tabs Change.lua: Beta hides Avatar, Enrolled hides the store).
    fn visible_tabs(&self) -> Vec<usize> {
        let v = self.cfg.ui("Version");
        (0..TABS.len()).filter(|&i| !(i == 1 && v < "v1.0") && !(i == 4 && v >= "v3.0")).collect()
    }

    /// The account's ROBLOX achievements, Xbox One achievements style: a summary (gamerscore,
    /// progress), wide art cards with name, description and gamerscore, and the focused
    /// achievement described in full on the right.
    fn achievements_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Achievements { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Achievements");
        let defs = &achievements::DEFS;
        let total: u32 = defs.iter().map(|d| d.gamerscore).sum();
        let got = defs.iter().filter(|d| self.ach.unlocked(d.id).is_some()).count();
        let score = self.ach.gamerscore();

        // summary: Xbox sphere + ROBLOX, gamerscore, progress bar
        let top = sa.y + 180.0;
        let x0 = sa.x + 16.0;
        ui.image(XBOX_SPHERE_IMG, Rect::new(x0, top + 4.0, 44.0, 44.0), WHITE, 0.9);
        ui.label("ROBLOX", Font::Regular, SUBHEADER, Rect::new(x0 + 58.0, top + 4.0, 300.0, 44.0), Align::Start, Align::Center, WHITE, 1.0);
        let gs = score.to_string();
        let gw = ui.measure(&gs, Font::Light, 54.0);
        let gx = x0 + 260.0;
        gamerscore_glyph(ui, Vec2::new(gx + 18.0, top + 26.0), 36.0, false);
        ui.label(&gs, Font::Light, 54.0, Rect::new(gx + 46.0, top - 6.0, gw, 64.0), Align::Start, Align::Center, WHITE, 1.0);
        let pct = (got as f32 / defs.len() as f32 * 100.0).round();
        let line = format!("{got} of {} achievements  ·  {pct}%", defs.len());
        ui.label(&line, Font::Regular, SMALL_TITLE, Rect::new(gx + 300.0, top + 10.0, 500.0, 40.0), Align::Start, Align::Center, WHITE, 1.0);
        // badges: owned, earned today / yesterday, and their share of the gamerscore
        let (badges, today, yesterday, badge_g) = self.ach.badge_stats();
        let bline = format!("{badges} badges ({badge_g} G)  ·  {today} today  ·  {yesterday} yesterday");
        ui.label(&bline, Font::Regular, SMALL_TITLE, Rect::new(gx + 300.0, top - 30.0, 700.0, 40.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        let ach_g = score - badge_g;
        let bar = Rect::new(x0, top + 66.0, 1188.0, 6.0);
        ui.rect(bar, FOREGROUND_GREY, 1.0);
        ui.rect(Rect::new(bar.x, bar.y, bar.w * ach_g as f32 / total.max(1) as f32, bar.h), XBOX_GREEN, 1.0);

        // 3 columns of wide cards: 16:9 art over a name / description / gamerscore strip
        const COLS: usize = 3;
        let (cw, art_h, info_h, gap) = (380.0, 214.0, 118.0, 24.0);
        let ch = art_h + info_h;
        let gy = top + 100.0;
        let view = Rect::new(x0 - 12.0, gy - 12.0, COLS as f32 * (cw + gap) + 12.0, 1080.0 - gy);
        let mut focused = None;
        for k in 0..defs.len() {
            if ui.is_focused(id("achievement", k as u64)) {
                focused = Some(k);
            }
        }
        if let Some(k) = focused {
            let row = (k / COLS) as f32;
            let target = (row - 1.0).max(0.0) * (ch + gap);
            self.ach_scroll += (target - self.ach_scroll) * 0.25;
        }
        ui.push_clip(view);
        for (k, d) in defs.iter().enumerate() {
            let r = Rect::new(x0 + (k % COLS) as f32 * (cw + gap), gy + (k / COLS) as f32 * (ch + gap) - self.ach_scroll, cw, ch);
            let sel = id("achievement", k as u64);
            let f = ui.selectable(sel, r);
            let unlocked = self.ach.unlocked(d.id).is_some();
            ui.nine_slice(SHADOW_IMG, Rect::new(r.x, r.y, r.w + 3.0, r.h + 3.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
            // art
            let art = Rect::new(r.x, r.y, r.w, art_h);
            ui.rect(art, if unlocked { ACH_ART_BG } else { CHARACTER_BG }, 1.0);
            ui.rect(Rect::new(art.x, art.y + art.h * 0.55, art.w, art.h * 0.45), [0.0; 3], 0.18);
            let isz = art_h - 34.0;
            ui.image(&self.ach.icon(d.id), Rect::new(art.x + (art.w - isz) / 2.0, art.y + 17.0, isz, isz), WHITE, if unlocked { 1.0 } else { 0.25 });
            if !unlocked {
                ui.image("rbxasset://glyphs/Hub/Lock.png", Rect::new(art.x + art.w - 52.0, art.y + 12.0, 40.0, 40.0), WHITE, 0.85);
            }
            // info
            let info = Rect::new(r.x, r.y + art_h, r.w, info_h);
            ui.rect(info, if f { WHITE } else { [0.07, 0.09, 0.11] }, if f { 1.0 } else { 0.85 });
            let (tc, sc) = if f { (TEXT_SELECTED, [0.25, 0.25, 0.25]) } else if unlocked { (WHITE, LIGHT_GREY_TEXT) } else { (LIGHT_GREY_TEXT, GREY_TEXT) };
            let name = ui.fit(d.name, Font::Regular, 28.0, info.w - 32.0);
            ui.label(&name, Font::Regular, 28.0, Rect::new(info.x + 16.0, info.y + 10.0, info.w - 32.0, 34.0), Align::Start, Align::Center, tc, 1.0);
            let desc = ui.fit(d.description, Font::Light, 22.0, info.w - 32.0);
            ui.label(&desc, Font::Light, 22.0, Rect::new(info.x + 16.0, info.y + 44.0, info.w - 32.0, 28.0), Align::Start, Align::Center, sc, 1.0);
            gamerscore_glyph(ui, Vec2::new(info.x + 28.0, info.y + info_h - 24.0), 22.0, f);
            ui.label(&d.gamerscore.to_string(), Font::Regular, 24.0, Rect::new(info.x + 46.0, info.y + info_h - 40.0, 100.0, 32.0), Align::Start, Align::Center, tc, 1.0);
            if unlocked {
                ui.image("rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png", Rect::new(info.x + info.w - 44.0, info.y + info_h - 38.0, 28.0, 28.0), if f { [0.0, 0.55, 0.25] } else { XBOX_GREEN }, 1.0);
            }
            if f {
                ui.selection_box(r, 4.0);
            }
        }
        ui.pop_clip();
        if !ui.focus_valid() {
            ui.set_focus(id("achievement", 0));
        }

        // the focused achievement in full
        let k = focused.unwrap_or(0);
        let d = &defs[k];
        let px = x0 + COLS as f32 * (cw + gap) + 30.0;
        let pw = sa.x + sa.w - px;
        ui.rect(Rect::new(px - 28.0, gy, 2.0, 1080.0 - gy - 60.0), PAGE_DIVIDE, 1.0);
        let unlocked = self.ach.unlocked(d.id);
        let art = Rect::new(px, gy, pw, pw * 9.0 / 16.0);
        ui.rect(art, if unlocked.is_some() { ACH_ART_BG } else { CHARACTER_BG }, 1.0);
        ui.rect(Rect::new(art.x, art.y + art.h * 0.55, art.w, art.h * 0.45), [0.0; 3], 0.18);
        let isz = art.h - 40.0;
        ui.image(&self.ach.icon(d.id), Rect::new(art.x + (art.w - isz) / 2.0, art.y + 20.0, isz, isz), WHITE, if unlocked.is_some() { 1.0 } else { 0.3 });
        if unlocked.is_none() {
            ui.image("rbxasset://glyphs/Hub/Lock.png", Rect::new(art.x + art.w - 64.0, art.y + 16.0, 48.0, 48.0), WHITE, 0.9);
        }
        let ty = art.y + art.h + 24.0;
        ui.label(d.name, Font::Light, HEADER, Rect::new(px, ty, pw, 60.0), Align::Start, Align::Center, WHITE, 1.0);
        gamerscore_glyph(ui, Vec2::new(px + 16.0, ty + 92.0), 30.0, false);
        ui.label(&d.gamerscore.to_string(), Font::Regular, TITLE, Rect::new(px + 42.0, ty + 70.0, 200.0, 44.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.paragraph(d.description, Font::Light, DESCRIPTION, Rect::new(px, ty + 130.0, pw, 90.0), LIGHT_GREY_TEXT, 1.0, 3);
        match unlocked {
            Some(secs) => {
                ui.image("rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png", Rect::new(px, ty + 228.0, 30.0, 30.0), XBOX_GREEN, 1.0);
                ui.label(&format!("Unlocked {}", ymd(secs)), Font::Regular, BUTTON, Rect::new(px + 42.0, ty + 222.0, pw, 42.0), Align::Start, Align::Center, XBOX_GREEN, 1.0);
            }
            None => {
                ui.image("rbxasset://glyphs/Hub/Lock.png", Rect::new(px, ty + 228.0, 30.0, 30.0), GREY_TEXT, 1.0);
                ui.label("Locked", Font::Regular, BUTTON, Rect::new(px + 42.0, ty + 222.0, pw, 42.0), Align::Start, Align::Center, GREY_TEXT, 1.0);
            }
        }
        let share = d.gamerscore as f32 / total.max(1) as f32 * 100.0;
        ui.label(&format!("{share:.0}% of this game's gamerscore"), Font::Light, SMALL_TITLE, Rect::new(px, ty + 270.0, pw, 36.0), Align::Start, Align::Center, GREY_TEXT, 1.0);

        self.end_fade(ui, old);
        if !blocked && input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    fn help(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Help { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Help");
        let ctrl = Rect::new(sa.x + 16.0, sa.y + 260.0, 599.0, 404.0);
        ui.image("rbxasset://textures/ui/Shell/ScreenAdjustment/Controller@1080.png", ctrl, WHITE, 1.0);
        let rows: [(&str, &str, &str); 10] = [
            ("A", "Select", "Enter / Space / Left click"),
            ("B", "Back", "Esc / Backspace / Right click"),
            ("X", "Hint action (Settings, Search, Equip, Set Default)", "X"),
            ("Y", "Remove account (Switch Profile)", "Y"),
            ("LB / RB", "Switch tabs", "Q / E (or Page Up / Page Down)"),
            ("D-pad / Left stick", "Move selection", "Arrow keys / WASD / mouse"),
            ("Right stick", "Look around the background", "Mouse position"),
            ("", "Toggle the background filter", "F1"),
            ("", "Hide the interface", "F2"),
            ("Start", "Start from the title screen", "Any key above"),
        ];
        let x = ctrl.x + ctrl.w + 70.0;
        ui.label("CONTROLLER", Font::Regular, SUBHEADER, Rect::new(x, sa.y + 220.0, 300.0, 28.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.label("ACTION", Font::Regular, SUBHEADER, Rect::new(x + 260.0, sa.y + 220.0, 500.0, 28.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.label("KEYBOARD / MOUSE", Font::Regular, SUBHEADER, Rect::new(x + 760.0, sa.y + 220.0, 400.0, 28.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.rect(Rect::new(x, sa.y + 252.0, 1020.0, 2.0), LINE_BREAK, 1.0);
        for (i, (pad, action, keys)) in rows.iter().enumerate() {
            let y = sa.y + 268.0 + i as f32 * 52.0;
            let pad_icon = match *pad {
                "A" | "B" | "X" | "Y" => Some(glyph(pad.chars().next().unwrap_or('A'))),
                _ => None,
            };
            match pad_icon {
                Some(icon) => {
                    ui.image(&icon, Rect::new(x, y + 2.0, 40.0, 40.0), WHITE, 1.0);
                }
                None => ui.label(pad, Font::Regular, SMALL_TITLE, Rect::new(x, y, 250.0, 44.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0),
            }
            let a = ui.fit(action, Font::Light, SMALL_TITLE * 1.15, 480.0);
            ui.label(&a, Font::Light, SMALL_TITLE * 1.15, Rect::new(x + 260.0, y, 480.0, 44.0), Align::Start, Align::Center, WHITE, 1.0);
            ui.label(keys, Font::Regular, SMALL_TITLE, Rect::new(x + 760.0, y, 400.0, 44.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
        ui.paragraph(
            "Games open through the roblox-player handler (Roblox or your bootstrapper). Change it in Settings > Account.",
            Font::Light,
            SMALL_TITLE * 1.15,
            Rect::new(ctrl.x, ctrl.y + ctrl.h + 50.0, 600.0, 120.0),
            LIGHT_GREY_TEXT,
            1.0,
            3,
        );
        self.end_fade(ui, old);
        if !blocked && input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    /// The 2016 loading screen (LoadingScript.lua): tiled dark texture, place name and "By creator"
    /// bottom-left, the blue loading ring with "Loading.." bottom-right, B Cancel top-right.
    /// Once the Roblox window opens it turns black and waits for the client to close.
    fn loading_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, ev: &mut ShellEvents) {
        let Some(Screen::Loading { name, creator, thumb, icon, t0, error, pids, window, stop, .. }) = self.stack.last() else { return };
        let (name, creator, thumb, icon, t0, error, pids, window, stop) = (name.clone(), creator.clone(), thumb.clone(), icon.clone(), *t0, error.clone(), pids.clone(), *window, stop.clone());
        let style = self.cfg.ui("Loading Screen").to_string();
        let now = ui.time;
        let full = Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, ui.screen.x / ui.scale, ui.screen.y / ui.scale);
        let (right, bottom) = (full.x + full.w, full.y + full.h);
        let mut go_home = false;

        if let Some(w) = window {
            // black "waiting" screen; press anything 3 times in a row to leave
            ui.alpha = ((now - w) / 0.4).clamp(0.0, 1.0);
            ui.rect(full, [0.0; 3], 1.0);
            ui.label("Waiting for Roblox to close", Font::Light, TITLE, Rect::new(full.x, 500.0, full.w, 50.0), Align::Center, Align::Center, WHITE, 1.0);
            ui.label("Press any button 3 times in a row to go home", Font::Regular, SMALL_TITLE, Rect::new(full.x, 560.0, full.w, 30.0), Align::Center, Align::Center, GREY_TEXT, 1.0);
            ui.alpha = 1.0;
            if input.any_activity || !input.pressed.is_empty() {
                if let Some(Screen::Loading { presses, .. }) = self.stack.last_mut() {
                    presses.retain(|t| now - *t < 1.5);
                    presses.push(now);
                    go_home = presses.len() >= 3;
                }
            }
        } else {
            ev.keep_focus = true;
            ui.alpha = ((now - t0) / 0.2).clamp(0.0, 1.0);
            if style == "2015" {
                ui.rect(full, [45.0 / 255.0; 3], 1.0);
                let tile = 512.0;
                let mut ty = full.y;
                while ty < bottom {
                    let mut tx = full.x;
                    while tx < right {
                        ui.image("rbxasset://textures/loading/darkLoadingTexture.png", Rect::new(tx, ty, tile, tile), WHITE, 1.0);
                        tx += tile;
                    }
                    ty += tile;
                }
                // place name (with the 1px text stroke) and "By creator"
                let place = if name.is_empty() { "Loading game".to_string() } else { name.clone() };
                let place = ui.fit(&place, Font::Regular, MEDIUM, 1300.0);
                let nx = full.x + 140.0;
                let ny = bottom - 140.0 - MEDIUM * 0.5;
                for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)] {
                    ui.text(&place, Font::Regular, MEDIUM, nx + dx, ny + dy, [0.0; 3], 0.8);
                }
                ui.text(&place, Font::Regular, MEDIUM, nx, ny, WHITE, 1.0);
                if !creator.is_empty() {
                    let by = format!("By {creator}");
                    let cy = bottom - 84.0 - DESCRIPTION * 0.5;
                    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                        ui.text(&by, Font::Regular, DESCRIPTION, nx + dx, cy + dy, [0.0; 3], 0.8);
                    }
                    ui.text(&by, Font::Regular, DESCRIPTION, nx, cy, WHITE, 1.0);
                }
                // loading ring: one turn every 2s, "Loading" dots every 0.2s
                let c = Vec2::new(right - 232.0, bottom - 161.0);
                ui.image_rotated("rbxasset://textures/loading/loadingCircle.png", c, Vec2::splat(175.0), now * std::f32::consts::TAU / 2.0, 1.0);
                let dots = ((now - t0) / 0.2) as usize % 4;
                ui.text(&format!("Loading{}", ".".repeat(dots)), Font::Regular, SMALL_TITLE, c.x - 52.0, c.y - SMALL_TITLE * 0.5, WHITE, 1.0);
            } else {
                modern_loading(ui, full, &style, &name, &creator, thumb.as_deref(), icon.as_deref(), now);
            }
            // B Cancel
            let bsize = 83.0;
            let bx = right - 32.0 - bsize;
            ui.image(&glyph('B'), Rect::new(bx, full.y + 32.0, bsize, bsize), WHITE, 1.0);
            let cw = ui.measure("Cancel", Font::Regular, BUTTON);
            ui.label("Cancel", Font::Regular, BUTTON, Rect::new(bx - 16.0 - cw, full.y + 32.0, cw, bsize), Align::Start, Align::Center, WHITE, 1.0);
            if let Some(e) = &error {
                let ef = Rect::new(full.x + full.w * 0.25, full.y, full.w * 0.5, 80.0);
                ui.rect(ef, [253.0 / 255.0, 68.0 / 255.0, 72.0 / 255.0], 1.0);
                ui.paragraph(&format!("Unable to join game: {e}"), Font::Bold, SMALL_TITLE, ef.inset(12.0), WHITE, 1.0, 3);
            }
            ui.alpha = 1.0;
            if input.has(Btn::B) || (error.is_some() && input.has(Btn::A)) {
                audio.play("ButtonPress", 1.0);
                for p in &pids {
                    launch::kill(*p);
                }
                go_home = true;
            }
        }
        if go_home {
            stop.store(true, Ordering::Relaxed);
            self.pop(ui);
            ev.focus_self = true;
            ev.keep_focus = false;
        }
    }

    // >> search
    // (Xbox One keyboard)

    fn search(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Search { query, t0, symbols, shift, closing }) = self.stack.last_mut() else { return };
        if !blocked && closing.is_none() {
            query.push_str(&input.typed);
            if input.backspace {
                query.pop();
            }
            if input.paste {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if let Ok(t) = cb.get_text() {
                        query.push_str(t.lines().next().unwrap_or(""));
                    }
                }
            }
        }
        let (q, t0, sym, shf, closing_t) = (query.clone(), *t0, *symbols, *shift, *closing);
        // slide up 0.3s on open, down 0.25s on close
        let k = match closing_t {
            Some(c) => 1.0 - ease_out_quad((ui.time - c) / 0.25),
            None => ease_out_quad((ui.time - t0) / 0.3),
        };
        if closing_t.is_some() && k <= 0.0 {
            self.pop(ui);
            return;
        }
        let full = Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, ui.screen.x / ui.scale, ui.screen.y / ui.scale);
        ui.rect(full, [0.0; 3], 0.55 * k);
        let ph = 560.0;
        let panel = Rect::new(full.x, full.y + full.h - ph * k, full.w, ph);
        ui.rect(panel, [31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0], 1.0);
        ui.rect(Rect::new(panel.x, panel.y, panel.w, 4.0), [16.0 / 255.0, 124.0 / 255.0, 16.0 / 255.0], 1.0);
        let left = 960.0 - 640.0;
        let kb_title = if self.keyboard_party { "HOST ADDRESS (IP:PORT)" } else { "SEARCH GAMES" };
        ui.label(&s::word(kb_title), Font::Regular, SMALL_TITLE, Rect::new(left, panel.y + 22.0, 600.0, 28.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        let field = Rect::new(left, panel.y + 56.0, 1280.0, 64.0);
        ui.rect(field, WHITE, 1.0);
        let caret = if (ui.time * 2.0) as i32 % 2 == 0 { "|" } else { "" };
        ui.label(&format!("{q}{caret}"), Font::Regular, BUTTON, field.inset(14.0), Align::Start, Align::Center, [0.0; 3], 1.0);

        let rows: [&[&str]; 4] = if sym {
            [&["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"], &["@", "#", "$", "%", "&", "*", "-", "+", "(", ")"], &["!", "\"", "'", ":", ";", "/", "?", "_", ",", "."], &["~", "=", "[", "]", "{", "}", "<", ">", "^", "|"]]
        } else {
            [&["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"], &["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"], &["a", "s", "d", "f", "g", "h", "j", "k", "l", "'"], &["z", "x", "c", "v", "b", "n", "m", ",", ".", "?"]]
        };
        let (kw, kh, gap) = (116.0, 62.0, 8.0);
        let gy = panel.y + 140.0;
        let green = [16.0 / 255.0, 124.0 / 255.0, 16.0 / 255.0];
        let key_bg = [58.0 / 255.0, 58.0 / 255.0, 58.0 / 255.0];
        let mut pressed: Option<String> = None;
        for (ri, row) in rows.iter().enumerate() {
            for (ci, key) in row.iter().enumerate() {
                let r = Rect::new(left + ci as f32 * (kw + gap), gy + ri as f32 * (kh + gap), kw, kh);
                let sel = id("kb", (ri * 16 + ci) as u64);
                let f = ui.selectable(sel, r);
                ui.rect(r, if f { green } else { key_bg }, 1.0);
                let label = if shf && !sym { key.to_uppercase() } else { key.to_string() };
                ui.label(&label, Font::Regular, BUTTON, r, Align::Center, Align::Center, WHITE, 1.0);
                if !blocked && ui.pressed(sel) {
                    pressed = Some(label);
                }
            }
        }
        // right column: backspace, enter
        let rc = left + 10.0 * (kw + gap);
        let special: [(&str, u64, Rect); 2] = [
            ("DELETE", 100, Rect::new(rc, gy, 144.0, kh * 2.0 + gap)),
            ("ENTER", 101, Rect::new(rc, gy + 2.0 * (kh + gap), 144.0, kh * 2.0 + gap)),
        ];
        // bottom row: &123 / abc, shift, space, left, right
        let by = gy + 4.0 * (kh + gap);
        let bottom: [(&str, u64, Rect); 3] = [
            (if sym { "abc" } else { "&123" }, 102, Rect::new(left, by, kw * 2.0 + gap, kh)),
            ("SHIFT", 103, Rect::new(left + 2.0 * (kw + gap), by, kw * 2.0 + gap, kh)),
            ("SPACE", 104, Rect::new(left + 4.0 * (kw + gap), by, kw * 6.0 + gap * 5.0 + 144.0 + gap, kh)),
        ];
        let mut action: Option<u64> = None;
        for (label, k, r) in special.iter().chain(bottom.iter()) {
            let sel = id("kb", *k);
            let f = ui.selectable(sel, *r);
            let on = *k == 103 && shf;
            ui.rect(*r, if f { green } else if on { [80.0 / 255.0; 3] } else { key_bg }, 1.0);
            ui.label(label, Font::Regular, SMALL_TITLE, *r, Align::Center, Align::Center, WHITE, 1.0);
            if !blocked && ui.pressed(sel) {
                action = Some(*k);
            }
        }
        if !ui.focus_valid() && k > 0.5 {
            ui.set_focus(id("kb", 16));
        }
        // controller hints along the bottom of the panel
        let hy = panel.y + ph - 50.0;
        let mut hx = left;
        for (img, text) in [("XButton", "Backspace"), ("YButton", "Space"), ("BButton", "Close")] {
            ui.image(&glyph(img.chars().next().unwrap_or('A')), Rect::new(hx, hy, 36.0, 36.0), WHITE, 1.0);
            let w = ui.measure(text, Font::Regular, SMALL_TITLE);
            ui.label(text, Font::Regular, SMALL_TITLE, Rect::new(hx + 44.0, hy, w, 36.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
            hx += 44.0 + w + 40.0;
        }
        if blocked || closing_t.is_some() {
            return;
        }
        let Some(Screen::Search { query, symbols, shift, closing, .. }) = self.stack.last_mut() else { return };
        let mut submit = input.enter;
        // debug: XBOX_SEARCH="text" types and submits a search
        if let (true, false, Ok(q)) = (query.is_empty(), self.keyboard_party, std::env::var("XBOX_SEARCH")) {
            *query = q;
            submit = true;
        }
        if pressed.is_some() || action.is_some() {
            audio.play("KeyboardPress", 1.0);
        }
        if let Some(ch) = pressed {
            query.push_str(&ch);
            *shift = false;
        }
        match action {
            Some(100) => {
                query.pop();
            }
            Some(101) => submit = true,
            Some(102) => *symbols = !*symbols,
            Some(103) => *shift = !*shift,
            Some(104) => query.push(' '),
            _ => {}
        }
        if input.has(Btn::X) && !input.typed.contains(['x', 'X']) {
            query.pop();
        }
        if input.has(Btn::Y) && !input.typed.contains(['y', 'Y']) {
            query.push(' ');
        }
        if submit && !query.trim().is_empty() && self.keyboard_party {
            audio.play("ButtonPress", 1.0);
            let addr = query.trim().to_string();
            self.keyboard_party = false;
            self.pop(ui);
            self.party_join(ui, audio, addr);
            return;
        }
        if submit && !query.trim().is_empty() {
            audio.play("ButtonPress", 1.0);
            let qq = query.trim().to_string();
            if let Some(sess) = self.session.clone() {
                let q2 = qq.clone();
                spawn(&self.tx, move || {
                    let r = sess.search(&q2).map_err(|e| e.to_string());
                    Msg::Search(q2, r)
                });
            }
            // the keyboard closes and the results page opens right away with a spinner
            self.pop(ui);
            // searching again from a results page replaces it
            let back = match self.stack.last() {
                Some(Screen::Carousel { query: Some(_), back, .. }) => Some(back.clone()),
                _ => None,
            };
            if back.is_some() {
                self.pop(ui);
            }
            self.carousel_idx = 0;
            let t0 = ui.time;
            self.push(ui, Screen::Carousel { title: format!("Searching for: {qq}"), back: back.unwrap_or_else(|| "GAMES".into()), games: None, query: Some(qq), t0, pos: 0.0 });
            return;
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            *closing = Some(ui.time);
        }
    }

    // >> gamer card

    fn profile_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Profile { uid, t0, scroll, target }) = self.stack.last() else { return };
        let (uid, t0, mut scroll, mut target) = (*uid, *t0, *scroll, *target);
        let sa = SAFE;
        let old = self.fade(ui, t0);
        let p = match self.profiles_data.get(&uid) {
            Some(Ok(p)) => p.clone(),
            Some(Err(_)) => {
                back_and_title(ui, "Gamer Card");
                ui.paragraph("An error occurred. Please try again later.", Font::Bold, DESCRIPTION, Rect::new(sa.x + 16.0, sa.y + 260.0, 900.0, 200.0), GREY_TEXT, 1.0, 3);
                self.end_fade(ui, old);
                if !blocked && input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.pop(ui);
                }
                return;
            }
            None => {
                back_and_title(ui, "Gamer Card");
                self.loading(ui, Vec2::new(960.0, 540.0));
                self.end_fade(ui, old);
                if !blocked && input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.pop(ui);
                }
                return;
            }
        };
        let title = if p.display_name.is_empty() { p.name.clone() } else { p.display_name.clone() };
        back_and_title(ui, &title);
        let cy = sa.y + 210.0;
        let ch = 622.0;
        ui.push_clip(Rect::new(sa.x - 30.0, cy - 30.0, sa.w + 60.0, ch + 90.0));
        let tr = ui.push_translate(Vec2::new(-scroll, 0.0));
        let mut x = sa.x + 8.0;
        // avatar column (566)
        let av = Rect::new(x, cy + (ch - 566.0) / 2.0, 566.0, 566.0);
        ui.nine_slice(SHADOW_IMG, Rect::new(av.x, av.y, av.w + 3.0, av.h + 3.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
        ui.rect(av, CHARACTER_BG, 1.0);
        if !p.avatar.is_empty() {
            ui.image(&p.avatar, av, WHITE, 1.0);
        }
        // the avatar and about columns are selectable so the page can be scrolled back to them
        if ui.selectable(id("profile_tile", 900), av) {
            ui.selection_box(av, 7.0);
        }
        x += 566.0 + 38.0;
        let about = Rect::new(x, cy + 40.0, 394.0, ch - 40.0);
        if ui.selectable(id("profile_tile", 901), about) {
            ui.selection_box(about, 7.0);
        }
        // about column (394)
        ui.label("ABOUT", Font::Regular, SUBHEADER, Rect::new(x, cy, 394.0, 33.0), Align::Start, Align::Center, WHITE, 1.0);
        let mut y = cy + 48.0;
        ui.image(roblox_icon(&self.cfg), Rect::new(x, y + 4.0, 24.0, 24.0), WHITE, 1.0);
        ui.label(&format!("@{}", p.name), Font::Regular, SUBHEADER, Rect::new(x + 32.0, y, 360.0, 32.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        y += 50.0;
        for (label, n) in [("Friends", p.friends), ("Followers", p.followers), ("Following", p.following)] {
            ui.label(label, Font::Regular, SMALL_TITLE, Rect::new(x, y, 200.0, 24.0), Align::Start, Align::Center, WHITE, 1.0);
            ui.label(&fmt_num(n), Font::Light, TITLE, Rect::new(x + 180.0, y - 9.0, 214.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
            y += 52.0;
        }
        if !p.created.is_empty() {
            ui.label("Joined", Font::Regular, SMALL_TITLE, Rect::new(x, y, 200.0, 24.0), Align::Start, Align::Center, WHITE, 1.0);
            ui.label(&p.created, Font::Light, TITLE, Rect::new(x + 180.0, y - 9.0, 214.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
            y += 52.0;
        }
        ui.rect(Rect::new(x, y + 6.0, 362.0, 2.0), LINE_BREAK, 1.0);
        let desc = if p.description.trim().is_empty() { "This user has not written a description." } else { p.description.as_str() };
        ui.paragraph(desc, Font::Light, SMALL_TITLE, Rect::new(x, y + 24.0, 380.0, cy + ch - y - 24.0), WHITE, 1.0, 12);
        x += 394.0 + 32.0;
        ui.rect(Rect::new(x, cy + 33.0, 2.0, 566.0), LINE_BREAK, 1.0);
        x += 2.0 + 52.0;
        // grids: favorites, currently wearing, badges, creations (3x2 of 200)
        let tile = 196.0;
        let gap = 12.0;
        let mut columns: Vec<(f32, f32)> = Vec::new();
        let mut open: Option<GameInfo> = None;
        let sections: [(&str, usize); 4] = [("FAVORITES", 0), ("CURRENTLY WEARING", 1), ("BADGES", 2), ("CREATIONS", 3)];
        for (title, kind) in sections {
            let count = match kind {
                0 => p.favorites.len(),
                1 => p.wearing.len(),
                2 => p.badges.len(),
                _ => p.creations.len(),
            };
            if count == 0 {
                continue;
            }
            let w = 3.0 * tile + 2.0 * gap;
            columns.push((x, w));
            ui.label(title, Font::Regular, SUBHEADER, Rect::new(x, cy, w, 33.0), Align::Start, Align::Center, WHITE, 1.0);
            for i in 0..count.min(6) {
                let r = Rect::new(x + (i % 3) as f32 * (tile + gap), cy + 33.0 + 10.0 + (i / 3) as f32 * (tile + gap), tile, tile);
                let sel = id("profile_tile", (kind * 100 + i) as u64);
                match kind {
                    0 | 3 => {
                        let g = if kind == 0 { &p.favorites[i] } else { &p.creations[i] };
                        let g = g.clone();
                        self.game_tile(ui, sel, r, Some(&g));
                        if !blocked && ui.pressed(sel) {
                            open = Some(g);
                        }
                    }
                    _ => {
                        let (name, thumb) = if kind == 1 { &p.wearing[i] } else { &p.badges[i] };
                        let f = ui.selectable(sel, r);
                        ui.rect(r, CHARACTER_BG, 1.0);
                        if !thumb.is_empty() {
                            ui.image(thumb, r.inset(10.0), WHITE, 1.0);
                        }
                        popup_text(ui, sel, r, name);
                        if f {
                            ui.selection_box(r, 7.0);
                        }
                    }
                }
            }
            x += w + 52.0;
        }
        ui.pop_translate(tr);
        ui.pop_clip();
        // page the content like ContentManager:TweenContent
        if let Some(fr) = ui.focus.and_then(|f| ui.rect_of(f)) {
            let right_edge = fr.x + fr.w + scroll;
            let left_edge = fr.x + scroll;
            if right_edge - target > sa.x + sa.w {
                target = right_edge - (sa.x + sa.w) + 40.0;
            } else if left_edge - target < sa.x {
                target = (left_edge - sa.x - 40.0).max(0.0);
            }
        }
        scroll += (target - scroll) * 0.15;
        if !ui.focus_valid() {
            ui.set_focus(id("profile_tile", if p.favorites.is_empty() { 100 } else { 0 }));
        }
        let _ = columns;
        self.end_fade(ui, old);
        if let Some(Screen::Profile { scroll: sc, target: tg, .. }) = self.stack.last_mut() {
            *sc = scroll;
            *tg = target;
        }
        if let Some(g) = open {
            audio.play("ButtonPress", 1.0);
            self.open_game(ui, audio, g.universe_id, g.place_id);
        } else if !blocked && input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    /// Carousel view (Favorites / Featured / Recently Played "..." and search results): the focused
    /// game large on the left, the rest in a row, its name, rating, creator, description and
    /// Play / Favorite below. `games` is None while a search is still running.
    fn carousel(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Carousel { title, back, games, t0, pos, .. }) = self.stack.last() else { return };
        let (title, back, games, t0, mut pos) = (title.clone(), back.clone(), games.clone(), *t0, *pos);
        let sa = SAFE;
        let old = self.fade(ui, t0);
        // "<- GAMES" and the title
        let bw = ui.measure(&s::word(&back), Font::Regular, BUTTON);
        ui.image("rbxasset://textures/ui/Shell/Icons/BackIcon@1080.png", Rect::new(sa.x, sa.y, 48.0, 48.0), WHITE, 1.0);
        ui.label(&s::word(&back), Font::Regular, BUTTON, Rect::new(sa.x + 56.0, sa.y, bw, 48.0), Align::Start, Align::Center, WHITE, 1.0);
        let t = ui.fit(&title.to_uppercase(), Font::Light, HEADER, sa.w - 40.0);
        ui.label(&t, Font::Light, HEADER, Rect::new(sa.x + 16.0, sa.y + 122.0, sa.w, 35.0), Align::Start, Align::Center, WHITE, 1.0);
        hint(ui, &glyph('X'), "SEARCH");

        let Some(games) = games else {
            self.loading(ui, Vec2::new(960.0, 540.0));
            self.end_fade(ui, old);
            if !blocked && input.has(Btn::B) {
                audio.play("ButtonPress", 1.0);
                self.pop(ui);
            }
            return;
        };
        if games.is_empty() {
            ui.label("No games found.", Font::Light, TITLE, Rect::new(sa.x + 16.0, 480.0, 800.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
        }
        let n = games.len();
        let focus_idx = (0..n).find(|k| ui.is_focused(id("car", *k as u64)));
        if let Some(k) = focus_idx {
            self.carousel_idx = k;
        }
        let cur = self.carousel_idx.min(n.saturating_sub(1));
        pos += (cur as f32 - pos) * 0.3;
        // tiles: the current one 450 at x=100, others 300 in a row, 18 apart, centred on y=501
        const BIG: f32 = 450.0;
        const SMALL: f32 = 300.0;
        const GAP: f32 = 18.0;
        let mut open = None;
        for (k, g) in games.iter().enumerate() {
            let d = k as f32 - pos;
            let x = if d >= 0.0 { 100.0 + d.min(1.0) * (BIG + GAP) + (d - 1.0).max(0.0) * (SMALL + GAP) } else { 100.0 + d * (SMALL + GAP) };
            let size = SMALL + (BIG - SMALL) * (1.0 - d.abs()).max(0.0);
            let r = Rect::new(x, 501.0 - size / 2.0, size, size);
            if r.x > 1920.0 || r.x + r.w < -400.0 {
                continue;
            }
            let sel = id("car", k as u64);
            self.game_tile(ui, sel, r, Some(g));
            if !blocked && ui.pressed(sel) {
                open = Some((g.universe_id, g.place_id));
            }
        }
        if !ui.focus_valid() && n > 0 {
            ui.set_focus(id("car", cur as u64));
        }

        // the focused game's details
        let mut play = false;
        let mut fav = false;
        if let Some(g) = games.get(cur) {
            self.want_details(g.universe_id);
            let d = self.details.get(&g.universe_id).cloned();
            let name = d.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| g.name.clone());
            let nm = ui.fit(&name, Font::Light, 48.0, 1700.0);
            ui.label(&nm, Font::Light, 48.0, Rect::new(119.0, 742.0, 1700.0, 56.0), Align::Start, Align::Center, WHITE, 1.0);
            let ry = 826.0;
            let (up, down) = d.as_ref().map(|d| (d.up_votes, d.down_votes)).unwrap_or((g.up_votes, g.down_votes));
            ui.image("rbxasset://textures/ui/Shell/Icons/ThumbsUpFilled@1080.png", Rect::new(116.0, ry - 16.0, 32.0, 32.0), GREEN_TEXT, 1.0);
            let bar = Rect::new(158.0, ry - 8.0, 203.0, 16.0);
            ui.rect(bar, GREY_TEXT, 1.0);
            if up + down > 0 {
                let k = up as f32 / (up + down) as f32;
                ui.rect(Rect::new(bar.x, bar.y, bar.w * k, bar.h), GREEN_TEXT, 1.0);
                ui.rect(Rect::new(bar.x + bar.w * k, bar.y, bar.w * (1.0 - k), bar.h), RED_TEXT, 1.0);
            }
            // VoteFrame segments
            for i in 1..5 {
                ui.rect(Rect::new(bar.x + bar.w * i as f32 / 5.0 - 1.0, bar.y, 2.0, bar.h), [0.0; 3], 0.6);
            }
            ui.image("rbxasset://textures/ui/Shell/Icons/ThumbsDownFilled@1080.png", Rect::new(371.0, ry - 16.0, 32.0, 32.0), RED_TEXT, 1.0);
            let mut x = 420.0;
            if let Some(d) = &d {
                ui.image("rbxasset://textures/ui/Shell/Icons/SeparatorDot@1080.png", Rect::new(x, ry - 5.0, 10.0, 10.0), WHITE, 1.0);
                x += 30.0;
                ui.image("rbxasset://textures/ui/Shell/Icons/RobloxIcon24.png", Rect::new(x, ry - 13.0, 26.0, 26.0), WHITE, 1.0);
                x += 36.0;
                let cw = ui.measure(&d.creator, Font::Regular, 26.0);
                ui.label(&d.creator, Font::Regular, 26.0, Rect::new(x, ry - 18.0, cw, 36.0), Align::Start, Align::Center, WHITE, 1.0);
                x += cw + 20.0;
                if let (Some(rating), "On") = (&d.age_rating, self.cfg.ui("Age Ratings")) {
                    ui.image("rbxasset://textures/ui/Shell/Icons/SeparatorDot@1080.png", Rect::new(x, ry - 5.0, 10.0, 10.0), WHITE, 1.0);
                    ui.label(rating, Font::Regular, 26.0, Rect::new(x + 30.0, ry - 18.0, 400.0, 36.0), Align::Start, Align::Center, WHITE, 1.0);
                }
                let desc: String = d.description.lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join("\n");
                ui.paragraph(&desc, Font::Light, 26.0, Rect::new(117.0, 852.0, 1300.0, 70.0), LIGHT_GREY_TEXT, 1.0, 2);
            }
            // Play / Favorite straight away; the favourite state fills in with the details
            {
                let favorited = d.as_ref().map(|d| d.is_favorited).unwrap_or(false);
                let play_sel = id("car_play", 0);
                let fav_sel = id("car_fav", 0);
                let pr = Rect::new(100.0, 956.0, 228.0, 70.0);
                let fr = Rect::new(338.0, 956.0, 228.0, 70.0);
                let pf = ui.selectable(play_sel, pr);
                let ff = ui.selectable(fav_sel, fr);
                ui.rect(pr, if pf { [0.2, 0.85, 0.45] } else { GREEN_TEXT }, 1.0);
                ui.label(&s::word("PLAY"), Font::Regular, BUTTON, pr, Align::Center, Align::Center, if pf { TEXT_SELECTED } else { WHITE }, 1.0);
                ui.rect(fr, if ff { ED_BLUE } else { GREY_BUTTON }, 1.0);
                let ft = if favorited { "FAVORITED" } else { "FAVORITE" };
                ui.label(&s::word(ft), Font::Regular, BUTTON, fr, Align::Center, Align::Center, if ff { TEXT_SELECTED } else { WHITE }, 1.0);
                if pf {
                    ui.selection_box(pr, 4.0);
                }
                if ff {
                    ui.selection_box(fr, 4.0);
                }
                play = !blocked && ui.pressed(play_sel);
                fav = !blocked && ui.pressed(fav_sel);
            }
        }
        self.end_fade(ui, old);
        if let Some(Screen::Carousel { pos: p, .. }) = self.stack.last_mut() {
            *p = pos;
        }
        if blocked {
            return;
        }
        let g = games.get(cur).cloned();
        if let (true, Some(g)) = (play, &g) {
            audio.play("ButtonPress", 1.0);
            let d = self.details.get(&g.universe_id).cloned().unwrap_or_default();
            let name = if d.name.is_empty() { g.name.clone() } else { d.name.clone() };
            self.launch(ui, JoinTarget::Place(g.place_id), name, d.creator.clone(), Some(g.place_id));
        } else if let (true, Some(g)) = (fav, &g) {
            audio.play("ButtonPress", 1.0);
            let universe = g.universe_id;
            if let (Some(sess), Some(dd)) = (self.session.clone(), self.details.get_mut(&universe)) {
                let v = !dd.is_favorited;
                dd.is_favorited = v;
                spawn(&self.tx, move || {
                    let ok = sess.set_favorite(universe, v).is_ok();
                    Msg::Favorited(universe, v, ok)
                });
            }
        } else if let Some((u, pl)) = open {
            audio.play("ButtonPress", 1.0);
            self.open_game(ui, audio, u, pl);
        } else if input.has(Btn::X) {
            audio.play("OverlayOpen", 1.0);
            let t0 = ui.time;
            self.push(ui, Screen::Search { query: String::new(), t0, symbols: false, shift: false, closing: None });
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    /// Open a carousel of `games` (a Home "..." button).
    fn open_carousel(&mut self, ui: &mut Ui, audio: &mut Audio, title: &str, back: &str, games: Vec<GameInfo>) {
        audio.play("ScreenChange", 1.0);
        self.want_icons(games.iter().map(|g| g.universe_id).collect::<Vec<_>>());
        self.carousel_idx = 0;
        let t0 = ui.time;
        self.push(ui, Screen::Carousel { title: title.into(), back: back.into(), games: Some(games), query: None, t0, pos: 0.0 });
    }

    /// Party screen. In a party: Players / Settings / Leave Party on the left, the members (or
    /// the settings) in the middle and the selected member's avatar on the right. Not in one:
    /// Host Party / Join Party.
    fn party_screen(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::Party { t0 }) = self.stack.last() else { return };
        let t0 = *t0;
        let old = self.fade(ui, t0);
        back_and_title(ui, "Party");
        let in_party = self.party.is_some();
        let labels: &[&str] = if in_party { &["Players", "Settings", "Leave Party"] } else { &["Host Party", "Join Party"] };
        let mut pressed_left = None;
        for (i, name) in labels.iter().enumerate() {
            let r = Rect::new(158.0, 283.0 + i as f32 * 118.0, 360.0, 78.0);
            let sel = id("partyleft", i as u64);
            let f = ui.selectable(sel, r);
            if f && in_party && i < 2 {
                self.party_section = i;
            }
            let current = in_party && self.party_section == i;
            ui.rect(r, if current { ED_BLUE } else { GREY_BUTTON }, if current { 1.0 } else { 0.85 });
            ui.label(name, Font::Regular, 30.0, Rect::new(r.x + 24.0, r.y, r.w - 30.0, r.h), Align::Start, Align::Center, WHITE, 1.0);
            if f {
                ui.selection_box(r, 4.0);
            }
            if current {
                ui.label("\u{25B6}", Font::Regular, 26.0, Rect::new(r.x + r.w + 12.0, r.y, 30.0, r.h), Align::Start, Align::Center, WHITE, 1.0);
            }
            if !blocked && ui.pressed(sel) {
                pressed_left = Some(i);
            }
        }
        // how others get in
        let info = match self.party.as_ref().map(|p| &p.role) {
            Some(party::Role::Host { port }) => {
                let online = match self.party.as_ref().map(|p| p.online()) {
                    Some(party::Online::Ready(a)) => format!("Share this address so anyone can join:\n{a}"),
                    Some(party::Online::Opening) => "Opening an address anyone can join...".to_string(),
                    Some(party::Online::Failed(e)) => format!("Could not open an internet address ({e})."),
                    None => String::new(),
                };
                format!("{online}\n\nOn the same network: this PC's IP and port {port}. playit.gg also works: a TCP tunnel to 127.0.0.1:{port}.")
            }
            Some(party::Role::Client { addr }) => format!("Connected to the party at {addr}."),
            None => "Host a party so others can connect to you, or join one with the host's address: an IP and port, or a playit.gg address.\n\nWhen the host joins a game, everyone in the party follows them into the same server.".to_string(),
        };
        ui.paragraph(&info, Font::Regular, 26.0, Rect::new(158.0, 283.0 + labels.len() as f32 * 118.0 + 20.0, 400.0, 400.0), WHITE, 1.0, 12);

        let mut member_pressed = None;
        let mut follow_choice = None;
        if let Some(p) = self.party.as_ref() {
            let host = p.host_uid();
            let me = self.user.as_ref().map(|u| u.id).unwrap_or(0);
            let members = p.members();
            let want: Vec<u64> = members.iter().map(|m| m.uid).filter(|u| !self.headshots.contains_key(u) && self.party_headshot_req.insert(*u)).collect();
            if let (false, Some(s)) = (want.is_empty(), self.session.clone()) {
                spawn(&self.tx, move || Msg::Headshots(s.headshots(&want).unwrap_or_default()));
            }
            if self.party_section == 1 {
                // Settings: what happens when the host joins a game
                ui.label("When the host joins a game", Font::Regular, 30.0, Rect::new(598.0, 230.0, 800.0, 40.0), Align::Start, Align::Center, WHITE, 1.0);
                let cur = self.cfg.ui("Party Follow").to_string();
                for (i, (opt, label)) in [("Auto", "Join them automatically"), ("Prompt", "Ask me first"), ("Off", "Do nothing")].iter().enumerate() {
                    let r = Rect::new(598.0, 283.0 + i as f32 * 118.0, 560.0, 78.0);
                    let sel = id("partyset", i as u64);
                    let f = ui.selectable(sel, r);
                    let on = cur == *opt;
                    ui.rect(r, if on { ED_BLUE } else { GREY_BUTTON }, if on { 1.0 } else { 0.85 });
                    ui.label(label, Font::Regular, 30.0, Rect::new(r.x + 24.0, r.y, r.w - 80.0, r.h), Align::Start, Align::Center, WHITE, 1.0);
                    if on {
                        ui.image("rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png", Rect::new(r.x + r.w - 56.0, r.y + 23.0, 32.0, 32.0), WHITE, 1.0);
                    }
                    if f {
                        ui.selection_box(r, 4.0);
                    }
                    if !blocked && ui.pressed(sel) {
                        follow_choice = Some(*opt);
                    }
                }
            } else {
                // Players
                let focused = members.iter().position(|m| ui.is_focused(id("partymember", m.uid)));
                if let Some(i) = focused {
                    let top = i as f32 * 139.0;
                    let target = if top - self.party_scroll > 139.0 * 4.0 { top - 139.0 * 4.0 } else if top < self.party_scroll { top } else { self.party_scroll };
                    self.party_scroll += (target - self.party_scroll) * 0.25;
                }
                ui.push_clip(Rect::new(588.0, 265.0, 470.0, 1040.0 - 265.0));
                for (i, m) in members.iter().enumerate() {
                    let r = Rect::new(598.0, 275.0 + i as f32 * 139.0 - self.party_scroll, 439.0, 118.0);
                    if r.y > 1080.0 || r.y + r.h < 200.0 {
                        continue;
                    }
                    let sel = id("partymember", m.uid);
                    let fo = ui.selectable(sel, r);
                    ui.rect(r, if fo { ED_BLUE } else { GREY_BUTTON }, if fo { 1.0 } else { 0.85 });
                    let av = Rect::new(r.x + 10.0, r.y + 10.0, 100.0, 98.0);
                    ui.rect(av, CHARACTER_BG, 1.0);
                    if let Some(h) = self.headshots.get(&m.uid).cloned() {
                        ui.image(&h, av, WHITE, 1.0);
                    }
                    ui.image(roblox_icon(&self.cfg), Rect::new(r.x + 126.0, r.y + 30.0, 26.0, 26.0), WHITE, 1.0);
                    let mut name = if m.name.is_empty() { format!("User {}", m.uid) } else { m.name.clone() };
                    if m.uid == host {
                        name.push_str("  (Host)");
                    } else if m.uid == me {
                        name.push_str("  (You)");
                    }
                    let nm = ui.fit(&name, Font::Regular, 30.0, r.w - 175.0);
                    ui.label(&nm, Font::Regular, 30.0, Rect::new(r.x + 164.0, r.y + 22.0, r.w - 175.0, 42.0), Align::Start, Align::Center, WHITE, 1.0);
                    let (status, col) = if m.place_id.is_some() {
                        (if m.game.is_empty() { "In Game".to_string() } else { m.game.clone() }, [36.0 / 255.0, 200.0 / 255.0, 90.0 / 255.0])
                    } else if m.game == "In Studio" {
                        ("In Studio".to_string(), [246.0 / 255.0, 136.0 / 255.0, 30.0 / 255.0])
                    } else {
                        ("Roblox".to_string(), [0.0, 162.0 / 255.0, 1.0])
                    };
                    ui.image(CIRCLE_IMG, Rect::new(r.x + 131.0, r.y + 70.0, 18.0, 18.0), col, 1.0);
                    let st = ui.fit(&status, Font::Regular, 26.0, r.w - 175.0);
                    ui.label(&st, Font::Regular, 26.0, Rect::new(r.x + 159.0, r.y + 62.0, r.w - 175.0, 34.0), Align::Start, Align::Center, WHITE, 1.0);
                    if fo {
                        ui.selection_box(r, 4.0);
                    }
                    if !blocked && ui.pressed(sel) {
                        member_pressed = Some(m.clone());
                    }
                }
                ui.pop_clip();
                // the selected member's avatar
                if let Some(uid) = focused.and_then(|i| members.get(i)).map(|m| m.uid) {
                    let at = Rect::new(1515.0 - 280.0, 610.0 - 280.0, 560.0, 560.0);
                    match self.friend_renders.get(&uid).cloned() {
                        Some(url) if !url.is_empty() => {
                            ui.image(&url, at, WHITE, 1.0);
                        }
                        Some(_) => {}
                        None => {
                            if self.friend_render_req.insert(uid) {
                                if let Some(s) = self.session.clone() {
                                    spawn(&self.tx, move || Msg::FriendRender(uid, s.avatar_url(uid, "avatar", "720x720").unwrap_or_default()));
                                }
                            }
                            self.loading(ui, at.center());
                        }
                    }
                }
            }
            hint(ui, &glyph('X'), "SETTINGS");
        }
        if !ui.focus_valid() {
            ui.set_focus(id("partyleft", 0));
        }
        self.end_fade(ui, old);
        if blocked {
            return;
        }
        if in_party && input.has(Btn::X) {
            audio.play("ButtonPress", 1.0);
            self.party_section = 1;
            ui.set_focus(id("partyset", 0));
        } else if let Some(opt) = follow_choice {
            audio.play("ButtonPress", 1.0);
            self.cfg.ui.insert("Party Follow".into(), opt.into());
            self.config_dirty = true;
        } else if let Some(m) = member_pressed {
            audio.play("ButtonPress", 1.0);
            let me = self.user.as_ref().map(|u| u.id).unwrap_or(0);
            let mut items = Vec::new();
            if m.uid != me {
                items.push(("JOIN GAME".to_string(), SideAction::JoinPartyMember(m.uid)));
            }
            if let Some(place) = m.place_id {
                items.push(("VIEW GAME DETAILS".to_string(), SideAction::ViewDetails(place)));
            }
            items.push(("VIEW GAMER CARD".to_string(), SideAction::ViewGamerCard(m.uid)));
            self.overlay = Some(Overlay::SideBar { items, t0: ui.time });
            ui.set_focus(id("sidebar", 0));
            audio.play("SideMenuSlideIn", 1.0);
        } else if let Some(i) = pressed_left {
            audio.play("ButtonPress", 1.0);
            match (in_party, i) {
                (true, 0) => {
                    if let Some(m) = self.party.as_ref().and_then(|p| p.members().first().cloned()) {
                        ui.set_focus(id("partymember", m.uid));
                    }
                }
                (true, 1) => ui.set_focus(id("partyset", 0)),
                (true, _) => {
                    let host = self.party.as_ref().map(|p| p.is_host()).unwrap_or(false);
                    if let Some(p) = self.party.take() {
                        p.leave();
                    }
                    self.party_section = 0;
                    let msg = if host { "You closed the party. Everyone in it has left." } else { "You left the party." };
                    self.overlay = Some(Overlay::Error { def: s::info("Party", msg), t0: ui.time });
                    ui.set_focus(id("partyleft", 0));
                }
                (false, 0) => self.party_host(ui, audio),
                _ => {
                    // the keyboard asks for the host's "ip:port"
                    self.keyboard_party = true;
                    let t0 = ui.time;
                    audio.play("OverlayOpen", 1.0);
                    self.push(ui, Screen::Search { query: String::new(), t0, symbols: false, shift: false, closing: None });
                }
            }
        } else if input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    fn party_me(&self) -> Option<party::Member> {
        let u = self.user.as_ref()?;
        Some(party::Member { uid: u.id, name: u.name.clone(), display: u.display_name.clone(), ..Default::default() })
    }

    fn party_host(&mut self, ui: &mut Ui, audio: &mut Audio) {
        let Some(me) = self.party_me() else { return };
        match party::Party::host(me) {
            Ok(p) => {
                self.party = Some(p);
                self.party_section = 0;
                self.party_presence_at = -100.0;
            }
            Err(e) => self.error(ui, audio, s::info("Party", &format!("Could not start the party: {e}"))),
        }
    }

    fn party_join(&mut self, ui: &mut Ui, audio: &mut Audio, addr: String) {
        let Some(me) = self.party_me() else { return };
        let addr = addr.trim().to_string();
        let (tx, a) = (self.tx.clone(), addr.clone());
        self.overlay = Some(Overlay::Busy { text: format!("Joining the party at {addr}") });
        std::thread::spawn(move || {
            let _ = tx.send(Msg::PartyJoined(a.clone(), party::Party::join(&a, me).map_err(|e| e.to_string())));
        });
        let _ = (ui, audio);
    }

    /// Party upkeep each frame: its events, and telling it where we are every few seconds.
    fn party_tick(&mut self, ui: &mut Ui, audio: &mut Audio) {
        let Some(p) = self.party.as_ref() else { return };
        for e in p.poll() {
            match e {
                party::Event::Members => {}
                party::Event::Follow { place_id, job_id, game } => {
                    if !party::valid_job_id(&job_id) {
                        continue;
                    }
                    match self.cfg.ui("Party Follow") {
                        "Auto" => self.launch(ui, JoinTarget::Instance(place_id, job_id), game, String::new(), Some(place_id)),
                        "Off" => {}
                        _ => {
                            audio.play("OverlayOpen", 1.0);
                            self.overlay = Some(Overlay::Confirm {
                                title: "Join the Party Host".into(),
                                phrase: format!("The party host joined {}. Do you want to join their server?", if game.is_empty() { "a game".to_string() } else { format!("\"{game}\"") }),
                                image: None,
                                action: Confirm::JoinServer(place_id, job_id, game),
                                t0: ui.time,
                            });
                        }
                    }
                }
                party::Event::Location { uid, place_id, job_id, game } => {
                    if self.party_join_wait.map(|w| w.0) == Some(uid) {
                        self.party_join_wait = None;
                        self.overlay = None;
                        match (place_id, job_id.filter(|j| party::valid_job_id(j))) {
                            (Some(p), Some(j)) => self.launch(ui, JoinTarget::Instance(p, j), game, String::new(), Some(p)),
                            (Some(p), None) => self.launch(ui, JoinTarget::Place(p), game, String::new(), Some(p)),
                            _ => self.error(ui, audio, s::info("Party", "That player is not in a game right now.")),
                        }
                    }
                }
                party::Event::Ended(why) => {
                    self.party = None;
                    self.party_section = 0;
                    self.error(ui, audio, s::info("Party", &why));
                    return;
                }
            }
        }
        // no answer: use the server they last told the party about
        if let Some((uid, at)) = self.party_join_wait {
            if ui.time - at > 6.0 {
                self.party_join_wait = None;
                self.overlay = None;
                let m = self.party.as_ref().and_then(|p| p.members().into_iter().find(|m| m.uid == uid));
                match m.and_then(|m| Some((m.place_id?, m.job_id, m.game))) {
                    Some((p, Some(j), game)) if party::valid_job_id(&j) => self.launch(ui, JoinTarget::Instance(p, j), game, String::new(), Some(p)),
                    Some((p, _, game)) => self.launch(ui, JoinTarget::Place(p), game, String::new(), Some(p)),
                    None => self.error(ui, audio, s::info("Party", "That player is not in a game right now.")),
                }
            }
        }
        // our own place / server from the presence API
        if ui.time - self.party_presence_at > 6.0 {
            self.party_presence_at = ui.time;
            if let (Some(s), Some(u)) = (self.session.clone(), self.user.clone()) {
                spawn(&self.tx, move || {
                    let (presence, place, job, location) = s.presence_of(u.id).unwrap_or((0, None, None, String::new()));
                    Msg::SelfPresence(presence, place, job, location)
                });
            }
        }
    }

    fn game_grid(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, blocked: bool) {
        let Some(Screen::GameGrid { title, games, t0, scroll }) = self.stack.last() else { return };
        let (title, games, t0, mut scroll) = (title.clone(), games.clone(), *t0, *scroll);
        let sa = SAFE;
        let old = self.fade(ui, t0);
        back_and_title(ui, &title);
        let area = Rect::new(sa.x + 8.0, sa.y + 210.0, sa.w, 760.0);
        let (tile, gap, cols) = (232.0, 26.0, 6usize);
        if games.is_empty() {
            ui.label("No games found.", Font::Regular, BUTTON, area, Align::Center, Align::Center, WHITE, 1.0);
        }
        if let Some(fi) = ui.focus.and_then(|f| (0..games.len()).find(|k| id("grid", *k as u64) == f)) {
            let top = (fi / cols) as f32 * (tile + gap + 60.0);
            let target = if top - scroll > area.h - tile - 80.0 { top - (area.h - tile - 80.0) } else if top < scroll { top } else { scroll };
            scroll += (target - scroll) * 0.25;
        }
        ui.push_clip(Rect::new(area.x - 20.0, area.y - 20.0, area.w + 40.0, area.h + 40.0));
        let mut open = None;
        for (k, g) in games.iter().enumerate() {
            let r = Rect::new(area.x + (k % cols) as f32 * (tile + gap), area.y + (k / cols) as f32 * (tile + gap + 60.0) - scroll, tile, tile);
            if r.y > area.y + area.h + 300.0 || r.y + r.h < area.y - 300.0 {
                continue;
            }
            let sel = id("grid", k as u64);
            self.game_tile(ui, sel, r, Some(g));
            if !blocked && ui.pressed(sel) {
                open = Some((g.universe_id, g.place_id));
            }
        }
        ui.pop_clip();
        if !ui.focus_valid() && !games.is_empty() {
            ui.set_focus(id("grid", 0));
        }
        self.end_fade(ui, old);
        if let Some(Screen::GameGrid { scroll: sc, .. }) = self.stack.last_mut() {
            *sc = scroll;
        }
        if let Some((u, pl)) = open {
            audio.play("ButtonPress", 1.0);
            self.open_game(ui, audio, u, pl);
        } else if !blocked && input.has(Btn::B) {
            audio.play("ButtonPress", 1.0);
            self.pop(ui);
        }
    }

    // >> overlays

    fn draw_overlay(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, ev: &mut ShellEvents) {
        let full = Rect::new(-4000.0, -4000.0, 10000.0, 10000.0);
        let t0 = match &self.overlay {
            Some(Overlay::Error { t0, .. }) | Some(Overlay::Confirm { t0, .. }) | Some(Overlay::SideBar { t0, .. }) | Some(Overlay::RobuxBalance { t0 }) | Some(Overlay::Report { t0, .. }) | Some(Overlay::Images { t0, .. }) => *t0,
            _ => ui.time - 1.0,
        };
        // BaseOverlay: modal background fades in over 0.25s; the content band appears immediately
        let k = ease_in_out_quad((ui.time - t0) / 0.25);
        ui.rect(full, [0.0; 3], MODAL_ALPHA * k);
        // content band: y=225, 690 tall, full width of the GUI root
        let band = Rect::new(-ui.offset.x / ui.scale, 225.0, ui.screen.x / ui.scale, 690.0);
        let right = 776.0;
        match self.overlay.take() {
            Some(Overlay::Busy { text }) => {
                ui.rect(band, OVERLAY, 1.0);
                self.loading(ui, Vec2::new(100.0 + 288.0, band.y + 345.0));
                ui.label(&text, Font::Regular, HEADER, Rect::new(right, band.y + 136.0 - 30.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
                self.overlay = Some(Overlay::Busy { text });
            }
            Some(Overlay::Error { def, t0 }) => {
                ui.rect(band, OVERLAY, 1.0);
                let c = Vec2::new(100.0 + 288.0, band.y + 345.0);
                if def.alert {
                    ui.image("rbxasset://textures/ui/Shell/Icons/AlertIcon.png", Rect::new(c.x - 208.0, c.y - 208.0, 416.0, 416.0), WHITE, 1.0);
                } else {
                    ui.image("rbxasset://textures/ui/Shell/Icons/ErrorIconLargeCopy@1080.png", Rect::new(c.x - 160.5, c.y - 132.0, 321.0, 264.0), WHITE, 1.0);
                }
                ui.label(&def.title, Font::Regular, HEADER, Rect::new(right, band.y + 136.0 - 30.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
                ui.paragraph(&s::error_text(&def), Font::Light, TITLE, Rect::new(right, band.y + 198.0, 762.0, 304.0), WHITE, 1.0, 7);
                let ok_w = (ui.measure("OK", Font::Regular, BUTTON) + 26.0).max(320.0);
                let ok = Rect::new(right, band.y + 690.0 - 100.0 - 66.0, ok_w, 66.0);
                let sel = id("overlay", 1);
                ui.selectable(sel, ok);
                ui.set_focus(sel);
                ui.rect(ok, BLUE_BUTTON, 1.0);
                ui.label("OK", Font::Regular, BUTTON, ok, Align::Center, Align::Center, TEXT_SELECTED, 1.0);
                if ui.pressed(sel) || input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                } else {
                    self.overlay = Some(Overlay::Error { def, t0 });
                }
            }
            Some(Overlay::RobuxBalance { t0 }) => {
                ui.rect(band, OVERLAY, 1.0);
                let c = Vec2::new(100.0 + 288.0, band.y + 345.0);
                ui.image("rbxasset://textures/ui/Shell/Icons/AlertIcon.png", Rect::new(c.x - 208.0, c.y - 208.0, 416.0, 416.0), WHITE, 1.0);
                ui.label(s::ROBUX_BALANCE_OVERLAY_TITLE, Font::Regular, HEADER, Rect::new(right, band.y + 88.0 - 30.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
                ui.paragraph(s::ROBUX_BALANCE_OVERLAY_PHRASE, Font::Light, TITLE, Rect::new(right, band.y + 150.0, 762.0, 200.0), WHITE, 1.0, 4);
                self.currency_widget(ui, Vec2::new(right, band.y + 370.0));
                let ok = Rect::new(right, band.y + 690.0 - 100.0 - 66.0, 320.0, 66.0);
                let sel = id("overlay", 5);
                ui.selectable(sel, ok);
                ui.set_focus(sel);
                ui.rect(ok, BLUE_BUTTON, 1.0);
                ui.label("OK", Font::Regular, BUTTON, ok, Align::Center, Align::Center, TEXT_SELECTED, 1.0);
                if ui.pressed(sel) || input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                } else {
                    self.overlay = Some(Overlay::RobuxBalance { t0 });
                }
            }
            Some(Overlay::Images { urls, index, prev, t0 }) => {
                // ImageOverlay: 668px band at y=226, 1030x580 image, slider arrows, "n/N" under it
                let band = Rect::new(band.x, 226.0, band.w, 668.0);
                ui.rect(band, OVERLAY, 1.0);
                let img = Rect::new(960.0 - 515.0, band.y + 334.0 - 290.0, 1030.0, 580.0);
                ui.push_clip(Rect::new(band.x, band.y, band.w, band.h));
                // slide: the new image comes in from 1.5 widths away, the old one leaves the other way
                let draw = |ui: &mut Ui, i: usize, dx: f32| {
                    let r = img.offset(dx, 0.0);
                    ui.rect(r, [0.0; 3], 1.0);
                    ui.image_cover(&urls[i], r, 1.0);
                };
                match prev {
                    Some((old, ts, dir)) if ui.time - ts < 0.3 => {
                        let k = ease_in_out_quad((ui.time - ts) / 0.3);
                        let dist = 1920.0 * 0.75;
                        draw(ui, old, -dir * dist * k);
                        draw(ui, index, dir * dist * (1.0 - k));
                    }
                    _ => draw(ui, index, 0.0),
                }
                ui.pop_clip();
                if urls.len() > 1 {
                    ui.image("rbxasset://textures/ui/Settings/Slider/Left.png", Rect::new(img.x - 18.0 - 75.0, img.y + 290.0 - 22.5, 26.0, 45.0), WHITE, 1.0);
                    ui.image("rbxasset://textures/ui/Settings/Slider/Right.png", Rect::new(img.x + img.w + 75.0, img.y + 290.0 - 22.5, 26.0, 45.0), WHITE, 1.0);
                    let t = format!("{}/{}", index + 1, urls.len());
                    ui.label(&t, Font::Regular, BUTTON, Rect::new(0.0, band.y + band.h - 24.0 - 18.0, 1920.0, 36.0), Align::Center, Align::Center, WHITE, 1.0);
                }
                let n = urls.len();
                let step = if input.has(Btn::Right) || input.has(Btn::RB) {
                    1.0
                } else if input.has(Btn::Left) || input.has(Btn::LB) {
                    -1.0
                } else {
                    0.0
                };
                if input.has(Btn::B) || input.has(Btn::A) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                } else if step != 0.0 && n > 1 {
                    audio.play("MoveSelection", 1.0);
                    let next = (index as i64 + step as i64).rem_euclid(n as i64) as usize;
                    self.overlay = Some(Overlay::Images { urls, index: next, prev: Some((index, ui.time, step)), t0 });
                } else {
                    self.overlay = Some(Overlay::Images { urls, index, prev, t0 });
                }
            }
            Some(Overlay::Report { place, t0 }) => {
                // ReportOverlay
                ui.rect(band, OVERLAY, 1.0);
                let c = Vec2::new(100.0 + 288.0, band.y + 345.0);
                ui.image("rbxasset://textures/ui/Shell/Icons/ErrorIconLargeCopy@1080.png", Rect::new(c.x - 160.5, c.y - 132.0, 321.0, 264.0), WHITE, 1.0);
                ui.label("Report Game", Font::Regular, HEADER, Rect::new(right, band.y + 136.0 - 30.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
                ui.paragraph(s::REPORT_PHRASE, Font::Light, TITLE, Rect::new(right, band.y + 198.0, 762.0, 304.0), WHITE, 1.0, 7);
                let submit = Rect::new(right, band.y + 690.0 - 100.0 - 66.0, 320.0, 66.0);
                let cancel = Rect::new(right + 330.0, submit.y, 320.0, 66.0);
                let (ss, cs) = (id("overlay", 7), id("overlay", 8));
                let sf = ui.selectable(ss, submit);
                let cf = ui.selectable(cs, cancel);
                if !sf && !cf {
                    ui.set_focus(ss);
                }
                for (r, label, f) in [(submit, "SUBMIT", sf), (cancel, "CANCEL", cf)] {
                    ui.nine_slice(BUTTON_IMG, r, [4.0, 4.0, 28.0, 28.0], if f { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
                    ui.label(label, Font::Regular, BUTTON, r, Align::Center, Align::Center, if f { TEXT_SELECTED } else { WHITE }, 1.0);
                }
                if ui.pressed(ss) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                    let uid = self.user.as_ref().map(|u| u.id).unwrap_or(0);
                    let _ = open::that_detached(crate::config::web(&format!("https://www.roblox.com/report-abuse/?targetId={place}&submitterId={uid}&abuseVector=place")));
                } else if ui.pressed(cs) || input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                } else {
                    self.overlay = Some(Overlay::Report { place, t0 });
                }
            }
            Some(Overlay::Confirm { title, phrase, image, action, t0 }) => {
                // ConfirmPrompt: package image on the left (580x640 grey backing), details on the right
                ui.rect(band, OVERLAY, 1.0);
                let pkg = Rect::new(100.0, band.y + 25.0, 580.0, 640.0);
                match &image {
                    Some(img) => {
                        ui.rect(pkg, FOREGROUND_GREY, 1.0);
                        // Utility.CalculateFill of a 484x540 image
                        let k = (pkg.w / 484.0).min(pkg.h / 540.0);
                        let (w, h) = (484.0 * k, 540.0 * k);
                        ui.image(img, Rect::new(pkg.center().x - w * 0.5, pkg.center().y - h * 0.5, w, h), WHITE, 1.0);
                    }
                    None => {
                        ui.image("rbxasset://textures/ui/Shell/Icons/AlertIcon.png", Rect::new(pkg.center().x - 208.0, pkg.center().y - 208.0, 416.0, 416.0), WHITE, 1.0);
                    }
                }
                let dx = 780.0;
                ui.label(&title, Font::Regular, HEADER, Rect::new(dx, band.y + 66.0 - 17.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
                ui.paragraph(&phrase, Font::Regular, TITLE, Rect::new(dx, band.y + 197.0, 800.0, 265.0), WHITE, 1.0, 5);
                let is_quit = matches!(action, Confirm::Quit | Confirm::JoinServer(..));
                let yes_label = if is_quit { "YES" } else { "CONFIRM" };
                let cw = (ui.measure(yes_label, Font::Regular, BUTTON) + 26.0).max(320.0);
                let yes = Rect::new(dx, band.y + 690.0 - 50.0 - 64.0, cw, 64.0);
                let sel = id("overlay", 2);
                let no_sel = id("overlay", 9);
                let yf = ui.selectable(sel, yes);
                if is_quit {
                    let no = Rect::new(dx + cw + 20.0, yes.y, 320.0, 64.0);
                    let nf = ui.selectable(no_sel, no);
                    if !yf && !nf {
                        ui.set_focus(no_sel);
                    }
                    ui.rect(yes, if yf { BLUE_BUTTON } else { GREY_BUTTON }, 1.0);
                    ui.label("YES", Font::Regular, BUTTON, yes, Align::Center, Align::Center, if yf { TEXT_SELECTED } else { WHITE }, 1.0);
                    ui.rect(no, if nf { BLUE_BUTTON } else { GREY_BUTTON }, 1.0);
                    ui.label("NO", Font::Regular, BUTTON, no, Align::Center, Align::Center, if nf { TEXT_SELECTED } else { WHITE }, 1.0);
                } else {
                    ui.set_focus(sel);
                    ui.rect(yes, BLUE_BUTTON, 1.0);
                    ui.label(yes_label, Font::Regular, BUTTON, yes, Align::Center, Align::Center, TEXT_SELECTED, 1.0);
                }
                let declined = is_quit && ui.pressed(no_sel);
                if let Confirm::BuyRobux(amount) = action {
                    // the package price in green under the title, "My Balance:" bottom left
                    if let Some((_, price)) = PACKAGES.iter().find(|(a, _)| *a == amount) {
                        ui.label(&format!("${price}"), Font::Regular, TITLE, Rect::new(dx, band.y + 120.0, 600.0, 50.0), Align::Start, Align::Center, GREEN_TEXT, 1.0);
                    }
                    self.currency_widget_as(ui, Vec2::new(0.052 * 1920.0, 0.88 * 1080.0), "My Balance:");
                }
                if ui.pressed(sel) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                    match action {
                        Confirm::BuyRobux(n) => {
                            // simulated: added to this session's balance only, never persisted
                            self.fake_robux += n;
                            audio.play("PurchaseSuccess", 1.0);
                        }
                        Confirm::SignOut => {
                            self.sign_out(ui);
                            ev.config_changed = true;
                        }
                        Confirm::Quit => ev.quit = true,
                        Confirm::JoinServer(place, job, game) => self.launch(ui, JoinTarget::Instance(place, job), game, String::new(), Some(place)),
                    }
                } else if declined || input.has(Btn::B) {
                    audio.play("ButtonPress", 1.0);
                    self.close_overlay(ui);
                } else {
                    self.overlay = Some(Overlay::Confirm { title, phrase, image, action, t0 });
                }
            }
            Some(Overlay::SideBar { items, t0 }) => {
                // slide in from the right over 0.25s (InOut Quad)
                let slide = ease_in_out_quad((ui.time - t0) / 0.25);
                let labels: Vec<String> = items.iter().map(|i| i.0.clone()).collect();
                let picked = draw_sidebar_panel(ui, &labels, true, 1.0 - slide);
                if let Some(i) = picked {
                    audio.play("ButtonPress", 1.0);
                    self.sidebar_out = Some((ui.time, labels));
                    self.modal_out = Some(ui.time);
                    match items[i].1.clone() {
                        SideAction::JoinGame(uid, name) => {
                            let friend = self.friends.as_ref().and_then(|f| f.iter().find(|x| x.id == uid)).cloned();
                            let title = friend.as_ref().map(|f| f.location.clone()).filter(|l| !l.is_empty()).unwrap_or(format!("Joining {name}"));
                            self.launch(ui, JoinTarget::Follow(uid), title, String::new(), friend.and_then(|f| f.place_id))
                        }
                        SideAction::ViewDetails(place) => self.open_game(ui, audio, 0, place),
                        SideAction::ViewGamerCard(uid) => self.open_profile(ui, audio, uid),
                        SideAction::JoinPartyMember(uid) => {
                            if let Some(p) = self.party.as_mut() {
                                p.request_location(uid);
                                self.party_join_wait = Some((uid, ui.time));
                                self.overlay = Some(Overlay::Busy { text: "Asking for their server...".into() });
                            }
                        }
                    }
                } else if input.has(Btn::B) {
                    self.sidebar_out = Some((ui.time, labels));
                    self.modal_out = Some(ui.time);
                    if let Some(f) = self.pane_focus.get(&self.tab).copied() {
                        ui.set_focus(f);
                    }
                } else {
                    self.overlay = Some(Overlay::SideBar { items, t0 });
                }
            }
            None => {}
        }
    }
}

/// PopupText: the item name in a semi-transparent black box at the bottom-left of the tile,
/// sliding up when the tile is selected and back down when it loses selection (0.3s, Quad Out).
fn popup_text(ui: &mut Ui, sel: u64, r: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let showing = ui.is_focused(sel);
    let k = if showing {
        ease_out_quad((ui.time - ui.focus_since) / 0.3)
    } else if ui.prev_focus == Some(sel) {
        1.0 - ease_out_quad((ui.time - ui.prev_since) / 0.3)
    } else {
        0.0
    };
    if k <= 0.0 {
        return;
    }
    const VPAD: f32 = 12.0;
    const HPAD: f32 = 18.0;
    const BORDER: f32 = 7.0;
    let size = (r.h * 0.11).clamp(20.0, 30.0);
    let lines = ui.wrap(text, Font::Light, size, r.w - BORDER - HPAD - 4.0);
    let tw = lines.iter().map(|l| ui.measure(l, Font::Light, size)).fold(0.0, f32::max);
    let th = (lines.len() as f32 * size * 1.1).min(r.h * 0.75 - VPAD);
    let (w, h) = (tw + HPAD + BORDER, th + VPAD + BORDER);
    let y = (r.y + r.h + 5.0) + (-h - 5.0) * k;
    ui.push_clip(r);
    let bg = Rect::new(r.x, y, w.min(r.w), h);
    ui.rect(bg, [0.0; 3], MODAL_ALPHA);
    let mut ty = y + VPAD / 2.0;
    for l in lines {
        if ty + size > bg.y + bg.h + 2.0 {
            break;
        }
        ui.text(&l, Font::Light, size, r.x + HPAD / 2.0 + BORDER, ty, WHITE, 1.0);
        ty += size * 1.1;
    }
    ui.pop_clip();
}

/// SideBar: 30% width panel on the right, items 75px tall starting at y=156, text inset 65.
/// `offset` 0 = fully shown, 1 = fully off screen. Returns the activated item.
fn draw_sidebar_panel(ui: &mut Ui, items: &[String], interactive: bool, offset: f32) -> Option<usize> {
    let full_w = ui.screen.x / ui.scale;
    let left = -ui.offset.x / ui.scale;
    let top = -ui.offset.y / ui.scale;
    let h = ui.screen.y / ui.scale;
    let w = full_w * 0.3;
    let x = left + full_w - w + w * offset;
    ui.rect(Rect::new(x, top, w, h), OVERLAY, 1.0);
    let mut picked = None;
    for (i, label) in items.iter().enumerate() {
        let r = Rect::new(x, top + 156.0 + 75.0 * i as f32, w, 75.0);
        let sel = id("sidebar", i as u64);
        let f = interactive && ui.selectable(sel, r);
        if f {
            ui.rect(r, BLUE_BUTTON, 1.0);
        }
        ui.label(label, Font::Regular, MEDIUM, Rect::new(r.x + 65.0, r.y, r.w - 65.0, r.h), Align::Start, Align::Center, if f { TEXT_SELECTED } else { WHITE }, 1.0);
        if interactive && ui.pressed(sel) {
            picked = Some(i);
        }
    }
    if interactive && !items.is_empty() && !(0..items.len()).any(|i| ui.is_focused(id("sidebar", i as u64))) {
        ui.set_focus(id("sidebar", 0));
    }
    picked
}

/// Templates/BaseScreen: back icon + "BACK", title at y=122
fn back_and_title(ui: &mut Ui, title: &str) {
    let sa = SAFE;
    let back = s::word("BACK");
    let tw = ui.measure(&back, Font::Regular, BUTTON);
    ui.image("rbxasset://textures/ui/Shell/Icons/BackIcon@1080.png", Rect::new(sa.x, sa.y, 48.0, 48.0), WHITE, 1.0);
    ui.label(&back, Font::Regular, BUTTON, Rect::new(sa.x + 56.0, sa.y, tw, 48.0), Align::Start, Align::Center, WHITE, 1.0);
    let t = ui.fit(&s::word(title), Font::Light, HEADER, sa.w - 40.0);
    ui.label(&t, Font::Light, HEADER, Rect::new(sa.x + 16.0, sa.y + 122.0, sa.w, 35.0), Align::Start, Align::Center, WHITE, 1.0);
}

/// Generic9ScaleButton-based button. Returns true when activated.
fn button(ui: &mut Ui, sel: u64, r: Rect, label: &str, primary: bool) -> bool {
    let f = ui.selectable(sel, r);
    let col = match (primary, f) {
        (true, true) => GREEN_SELECTED,
        (true, false) => GREEN_BUTTON,
        (false, true) => GREY_SELECTED,
        (false, false) => GREY_BUTTON,
    };
    ui.nine_slice(BUTTON_IMG, r, [8.0, 8.0, 24.0, 24.0], col, 1.0);
    ui.label(label, Font::Regular, BUTTON, r, Align::Center, Align::Center, if f { TEXT_SELECTED } else { WHITE }, 1.0);
    ui.pressed(sel)
}

/// Widgets/MoreButton
fn more_button(ui: &mut Ui, sel: u64, r: Rect) -> bool {
    let f = ui.selectable(sel, r);
    let img = if f { "rbxasset://textures/ui/Shell/Buttons/MoreButtonSelected@1080.png" } else { "rbxasset://textures/ui/Shell/Buttons/MoreButton@1080.png" };
    ui.image(img, r, WHITE, 1.0);
    if f {
        ui.selection_box(r, 7.0);
    }
    ui.pressed(sel)
}

/// HintActionView: button glyph + label anchored to the bottom-right of the title-safe area.
fn hint(ui: &mut Ui, img: &str, text: &str) {
    let text = &s::word(text);
    let sa = SAFE;
    let tw = ui.measure(text, Font::Regular, TITLE);
    let w = 83.0 + 8.0 + tw;
    let x = sa.x + sa.w - w;
    let y = sa.y + sa.h - 83.0;
    ui.image(img, Rect::new(x, y, 83.0, 83.0), WHITE, 1.0);
    ui.label(text, Font::Regular, TITLE, Rect::new(x + 91.0, y - 4.0, tw, 83.0), Align::Start, Align::Center, WHITE, 1.0);
}

/// Known bootstrapper locations (Bloxstrap, Fishstrap, Voidstrap, plain Roblox).
pub fn detect_bootstrappers() -> Vec<String> {
    let mut out = Vec::new();
    let local = dirs::data_local_dir().unwrap_or_default();
    for (dir, exe) in [("Bloxstrap", "Bloxstrap.exe"), ("Fishstrap", "Fishstrap.exe"), ("Voidstrap", "Voidstrap.exe"), ("Froststrap", "Froststrap.exe")] {
        let p = local.join(dir).join(exe);
        if p.exists() {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    if let Ok(rd) = std::fs::read_dir(local.join("Roblox").join("Versions")) {
        for e in rd.flatten() {
            let p = e.path().join("RobloxPlayerBeta.exe");
            if p.exists() {
                out.push(p.to_string_lossy().into_owned());
                break;
            }
        }
    }
    out
}

/// Theme.lua: the splash logo, the small hub logo and the splash height for each era. The
/// 2016 files ship in xboxassets; the other eras were downloaded from the place's image ids.
fn theme_art(theme: &str) -> (&'static str, &'static str, f32) {
    match theme {
        "Late 2015" => ("rbxasset://glyphs/theme/Splash_Late2015.png", "rbxasset://glyphs/theme/Small_Late2015.png", 245.0),
        "Early 2016" => ("rbxasset://glyphs/theme/Splash_Early2016.png", "rbxasset://glyphs/theme/Small_2016.png", 245.0),
        "Early 2017" | "Late 2017" => ("rbxasset://glyphs/theme/Splash_2017.png", "rbxasset://glyphs/theme/Small_2017.png", 209.0),
        "Mid 2022" => ("rbxasset://glyphs/theme/Splash_2022.png", "rbxasset://glyphs/theme/Small_2022.png", 209.0),
        _ => ("rbxasset://textures/ui/Shell/Icons/ROBLOXSplashLogo.png", "rbxasset://textures/ui/Shell/Icons/ROBLOXLogoSmall@1080.png", 199.0),
    }
}

/// Configure > Robux: the 2016 green R$ icon, or the current hexagon from the installed client.
fn robux_icon(ui: &mut Ui, cfg: &Config, r: Rect, outlined: bool) {
    let classic = if outlined { "rbxasset://textures/ui/Shell/Icons/ROBUXIconOutlined@1080.png" } else { "rbxasset://textures/ui/Shell/Icons/ROBUXIcon@1080.png" };
    if cfg.ui("Robux") == "Modern" {
        let modern = "rbxclient://textures/ui/common/robux@3x.png";
        if ui.image(modern, r.inset(r.w * 0.08), WHITE, 1.0) || !ui.image_failed(modern) {
            return;
        }
    }
    ui.image(classic, r, WHITE, 1.0);
}

/// Loading screens after 2015: game thumbnail (2017/2018), game icon (2020), or both (2022/Current).
#[allow(clippy::too_many_arguments)]
fn modern_loading(ui: &mut Ui, full: Rect, style: &str, name: &str, creator: &str, thumb: Option<&str>, icon: Option<&str>, now: f32) {
    let bg = [20.0 / 255.0, 43.0 / 255.0, 60.0 / 255.0];
    ui.rect(full, if style == "2022" || style == "Current" { [0.0; 3] } else { bg }, 1.0);
    let title_font = if style == "2017" { Font::Light } else { Font::Regular };
    let by = if creator.is_empty() { String::new() } else { format!("By {creator}") };
    let name = if name.is_empty() { "Loading game" } else { name };
    match style {
        "2017" | "2018" => {
            let r = Rect::new(960.0 - 560.0, 150.0, 1120.0, 630.0);
            ui.rect(r, CHARACTER_BG, 1.0);
            if let Some(t) = thumb {
                ui.image_cover(t, r, 1.0);
            }
            let n = ui.fit(name, title_font, HEADER, 1500.0);
            ui.label(&n, title_font, HEADER, Rect::new(0.0, 810.0, 1920.0, 64.0), Align::Center, Align::Center, WHITE, 1.0);
            ui.label(&by, Font::Regular, DESCRIPTION, Rect::new(0.0, 874.0, 1920.0, 40.0), Align::Center, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
        "2020" => {
            let r = Rect::new(960.0 - 210.0, 230.0, 420.0, 420.0);
            ui.rect(r, CHARACTER_BG, 1.0);
            if let Some(i) = icon.or(thumb) {
                ui.image_cover(i, r, 1.0);
            }
            let n = ui.fit(name, title_font, HEADER, 1500.0);
            ui.label(&n, title_font, HEADER, Rect::new(0.0, 690.0, 1920.0, 64.0), Align::Center, Align::Center, WHITE, 1.0);
            ui.label(&by, Font::Regular, DESCRIPTION, Rect::new(0.0, 754.0, 1920.0, 40.0), Align::Center, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
        _ => {
            if let Some(t) = thumb {
                ui.image_cover(t, full, 1.0);
            }
            // darken towards the bottom so the title reads
            for k in 0..24 {
                let f = k as f32 / 24.0;
                let y0 = (full.y + full.h * f).round();
                let y1 = (full.y + full.h * (k + 1) as f32 / 24.0).round();
                ui.rect(Rect::new(full.x, y0, full.w, y1 - y0), [0.0; 3], 0.35 + 0.5 * f);
            }
            let r = Rect::new(150.0, 640.0, 260.0, 260.0);
            ui.rect(r, CHARACTER_BG, 1.0);
            if let Some(i) = icon {
                ui.image_cover(i, r, 1.0);
            }
            let n = ui.fit(name, Font::Bold, HEADER, 1300.0);
            ui.label(&n, Font::Bold, HEADER, Rect::new(450.0, 730.0, 1300.0, 64.0), Align::Start, Align::Center, WHITE, 1.0);
            ui.label(&by, Font::Regular, DESCRIPTION, Rect::new(450.0, 794.0, 1300.0, 40.0), Align::Start, Align::Center, LIGHT_GREY_TEXT, 1.0);
        }
    }
    let c = Vec2::new(full.x + full.w - 120.0, full.y + full.h - 120.0);
    ui.image_rotated(SPINNER, c, Vec2::splat(80.0), now * std::f32::consts::TAU, 1.0);
}

/// Theme.lua RobloxIcon24 for each era (friend rows, gamer card).
fn roblox_icon(cfg: &Config) -> &'static str {
    match cfg.ui("Theme") {
        "Late 2015" => "rbxasset://glyphs/theme/Icon_Late2015.png",
        "Early 2017" => "rbxasset://glyphs/theme/Icon_Early2017.png",
        "Late 2017" => "rbxasset://glyphs/theme/Icon_Late2017.png",
        "Mid 2022" => "rbxasset://glyphs/theme/Icon_2022.png",
        _ => "rbxasset://textures/ui/Shell/Icons/RobloxIcon24.png",
    }
}

/// Unix seconds to "YYYY-MM-DD" (UTC).
fn ymd(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Where a friend is and the colour of their dot: in a game green, Studio orange, on the
/// website blue, offline grey.
fn presence_status(f: &Friend) -> (String, [f32; 3]) {
    match f.presence {
        2 => (if f.location.is_empty() { "In Game".to_string() } else { f.location.clone() }, [36.0 / 255.0, 200.0 / 255.0, 90.0 / 255.0]),
        3 => ("In Studio".to_string(), [246.0 / 255.0, 136.0 / 255.0, 30.0 / 255.0]),
        1 => ("Roblox".to_string(), [0.0, 162.0 / 255.0, 1.0]),
        _ => ("Offline".to_string(), [0.55, 0.55, 0.55]),
    }
}

/// Xbox gamerscore mark: a G in a disc.
fn gamerscore_glyph(ui: &mut Ui, center: Vec2, size: f32, dark: bool) {
    let (disc, letter) = if dark { (TEXT_SELECTED, WHITE) } else { (WHITE, TEXT_SELECTED) };
    ui.image(CIRCLE_IMG, Rect::new(center.x - size / 2.0, center.y - size / 2.0, size, size), disc, 1.0);
    ui.label("G", Font::Bold, size * 0.72, Rect::new(center.x - size / 2.0, center.y - size / 2.0 - size * 0.04, size, size), Align::Center, Align::Center, letter, 1.0);
}

/// A rectangle's border, `w` thick, drawn inside it.
fn outline(ui: &mut Ui, r: Rect, w: f32, color: [f32; 3], alpha: f32) {
    ui.rect(Rect::new(r.x, r.y, r.w, w), color, alpha);
    ui.rect(Rect::new(r.x, r.y + r.h - w, r.w, w), color, alpha);
    ui.rect(Rect::new(r.x, r.y + w, w, r.h - 2.0 * w), color, alpha);
    ui.rect(Rect::new(r.x + r.w - w, r.y + w, w, r.h - 2.0 * w), color, alpha);
}

fn hex_rgb(hex: &str) -> [f32; 3] {
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0) as f32 / 255.0;
    [c(0), c(2), c(4)]
}

/// HintActionView whose right edge sits at `right`.
fn hint_at(ui: &mut Ui, img: &str, text: &str, right: f32) {
    let text = &s::word(text);
    let tw = ui.measure(text, Font::Regular, TITLE);
    let x = right - 83.0 - 8.0 - tw;
    let y = SAFE.y + SAFE.h - 83.0;
    ui.image(img, Rect::new(x, y, 83.0, 83.0), WHITE, 1.0);
    ui.label(text, Font::Regular, TITLE, Rect::new(x + 91.0, y - 4.0, tw, 83.0), Align::Start, Align::Center, WHITE, 1.0);
}

/// HintActionView anchored to the bottom-left of the title-safe area.
fn hint_left(ui: &mut Ui, img: &str, text: &str) {
    let text = &s::word(text);
    let sa = SAFE;
    let tw = ui.measure(text, Font::Regular, TITLE);
    let y = sa.y + sa.h - 83.0;
    let x = sa.x;
    ui.image(img, Rect::new(x, y, 83.0, 83.0), WHITE, 1.0);
    ui.label(text, Font::Regular, TITLE, Rect::new(x + 91.0, y - 4.0, tw, 83.0), Align::Start, Align::Center, WHITE, 1.0);
}

// >> end << //
