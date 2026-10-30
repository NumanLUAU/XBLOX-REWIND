/*
    written by: @corescripts
*/

//! The configuration hub shown before the menu: the "Configure Xbox UI Settings?" prompt and the
//! Home / Presets / Configure / Begin panes over the Lobby3D world. Option names, captions
//! and presets follow Menu/Settings.lua and Panes/Presets/Presets.lua; the camera orbit follows
//! Menu/Render.lua (Center * yaw(pane) * Center:toObjectSpace(Slide) * bob).

// >> imports
use crate::audio::Audio;
use crate::config::{Config, IconSet};
use crate::input::{button_name, glyph, Btn, Input};
use crate::rbxl::CFrame;
use crate::shell::Shell;
use crate::ui::{ease_in_out_quad, id, Align, Font, Rect, Ui};
use glam::{Mat3, Vec3};
use std::collections::HashMap;

pub struct Setting {
    pub name: &'static str,
    pub description: &'static str,
    pub default: &'static str,
    pub options: &'static [&'static str],
    pub info: &'static [&'static str],
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        name: "Fast Boot",
        description: "Allows you skip the configuration prompt and run the menu right away.",
        default: "Off",
        options: &["On", "Off"],
        info: &["You will be boot into the menu right away.\nClick this to disable it once enabled.", "You will see the configuration prompt as usual when joining."],
    },
    Setting {
        name: "Theme",
        description: "Changes the logo to match the selected time period.",
        default: "Mid 2016",
        options: &["Late 2015", "Early 2016", "Mid 2016", "Early 2017", "Late 2017", "Mid 2022"],
        info: &[
            "Earliest iteration of the logo used in the menu.",
            "Roblox redid the logo to be more cleaner and slick.",
            "Identical to Early 2016, but with a different slogan.",
            "Roblox made an entirely new block-shaped logo.",
            "Identical to Early 2017, but with a change to player icons.",
            "The current day Roblox logo.",
        ],
    },
    Setting {
        name: "Version",
        description: "Changes the version of the menu.",
        default: "v1.0",
        options: &["v0.5", "v1.0", "v2.0", "v3.0"],
        info: &[
            "Prototype beta version of the menu.\nLate 2015",
            "First public release.\nEarly 2016 - Mid 2017",
            "Revamped second version with new Avatar Editor.\nMid 2017 \u{2014} Late 2022",
            "Xbox Series X/S exclusive version that was scrapped.\nEarly 2023",
        ],
    },
    Setting {
        name: "Search",
        description: "Toggles searching in versions v1.0 and below.",
        default: "On",
        options: &["On", "Off"],
        info: &["Enables searching.\nAdded in Late 2016.", "Disables searching.\nPrior to Late 2016."],
    },
    Setting {
        name: "Avatar Editor",
        description: "Changes the version of the Avatar Editor to match the selected time period.",
        default: "2017",
        options: &["2017", "2018", "2019", "2021", "2022"],
        info: &[
            "First initial version of the Avatar Editor.",
            "Identical to prior version, with some improvements.",
            "Different camera angles, with the addition of emotes.",
            "Complete overhaul, adds catalog and a dark and light theme.",
            "Adds the ability to use emotes for your profile picture.",
        ],
    },
    Setting {
        name: "Avatar Theme",
        description: "Changes the theme of the 2021+ avatar editor.",
        default: "Light",
        options: &["Light", "Dark"],
        info: &["Default Roblox Light theme.", "Default Roblox Dark theme."],
    },
    Setting {
        name: "Renders",
        description: "Changes Roblox default avatar/catalog renders to mimick a time period.",
        default: "Current",
        options: &["2015", "2017", "2019", "2020", "2022", "Current"],
        info: &[
            "Initial rendering utilized in the menu.",
            "R15 was introduced.",
            "Body colors and light direction changed for bundles.",
            "Headshots stared forward at you rather then the side as prior.\nThe Legacy Lighting system for the renders were replaced with Voxel.",
            "The default idle pose changed to the rthro idle pose.",
            "Current renders, which are dynamic compared to the previous varients.",
        ],
    },
    Setting {
        name: "Branding",
        description: "Changes words to fit the branding, which changed in 2021.",
        default: "Classic",
        options: &["Classic", "Modern"],
        info: &["The branding before 2021.", "The branding since 2021."],
    },
    Setting {
        name: "Robux",
        description: "Changes the robux branding, which changed in 2019.",
        default: "Classic",
        options: &["Classic", "Modern"],
        info: &["The old, original robux icon and branding prior to 2019.", "The current robux icon and branding since 2019."],
    },
    Setting {
        name: "Loading Screen",
        description: "Changes the loading screen for joining games.",
        default: "2015",
        options: &["2015", "2017", "2018", "2020", "2022", "Current"],
        info: &[
            "Initial loading screen menu utilized.",
            "A total revamp of the loading screen, now shows a thumbnail of the game joined.",
            "Tiny changes, such as font and trasitions.",
            "Tiny change again, now shows game icon instead of the thumbnail.",
            "A complete revamp of the menu, which now shows both thumbnail and icon.",
            "Current loading screen, identical to prior with minor differences.",
        ],
    },
    Setting {
        name: "Lighting",
        description: "Changes the type of lighting for the menu, which changed in 2020.",
        default: "Before 2020",
        options: &["Before 2020", "After 2020", "After 2022"],
        info: &["Roblox's old lighting.\nSeen before 2020.", "Roblox's current lighting.\nSeen since 2020.", "Identical to after 2020, but unions broke.\nSeen after 2022"],
    },
    Setting {
        name: "Age Ratings",
        description: "Adds Age Ratings to games, which was added in late 2022.",
        default: "Off",
        options: &["On", "Off"],
        info: &["Adds Age Ratings to games.", "Doesn't add Age Ratings."],
    },
    Setting {
        name: "Icons",
        description: "Changes the gamepad icons to match a controller style.",
        default: "Auto",
        options: &["Auto", "Xbox", "Keyboard", "PlayStation"],
        info: &[
            "Automatically changes the icons to fit your input on any device.",
            "The default Xbox icons used in the original menu.",
            "Custom keyboard icons.",
            "Custom PlayStation icons.",
        ],
    },
    Setting {
        name: "Screen",
        description: "Changes how the UI appears in terms of sizing.",
        default: "Borderless",
        options: &["Stretch", "Borders", "Borderless", "Widescreen"],
        info: &[
            "The UI will stretch to fit your screen.",
            "The UI will fit within the original ratio in your screen with borders.",
            "The UI will fit within the original ratio in your screen without borders.",
            "The UI will work at any screen ratio. Unintended looks may occur.",
        ],
    },
    Setting {
        name: "Uppercase Words",
        description: "Changes certain words to be uppercase in the UI, which changed in 2018.",
        default: "On",
        options: &["On", "Off"],
        info: &["Makes the words uppercase \u{2014} Seen before 2018.", "Does not make the words uppercase \u{2014} Seen since 2018."],
    },
    Setting {
        name: "Audio Fading",
        description: "Normally, the theme fades to be quieter after a loop, but you can disable it.",
        default: "On",
        options: &["On", "Off"],
        info: &["Fades the theme like usual after a loop", "Disables fading, note that this is a custom option for preference.\nThis never happened in the original menu and is toggleable for enjoyment."],
    },
];

struct Preset {
    year: u32,
    description: &'static str,
    /// (Early/Mid/Late, caption, settings)
    subs: &'static [(&'static str, &'static str, &'static [(&'static str, &'static str)])],
}

const PRESETS: &[Preset] = &[
    Preset {
        year: 2023,
        description: "Menu was revamped a tiny bit. Was exclusive to Xbox Series X/S.",
        subs: &[(
            "Early",
            "Menu was revamped entirely as a secret beta.\nWas exclusive to Xbox Series X/S.",
            &[("Renders", "Current"), ("Avatar Editor", "2022"), ("Branding", "Modern"), ("Lighting", "After 2022"), ("Robux", "Modern"), ("Theme", "Mid 2022"), ("Uppercase Words", "Off"), ("Version", "v3.0"), ("Age Ratings", "On"), ("Loading Screen", "Current")],
        )],
    },
    Preset {
        year: 2022,
        description: "New Roblox logo, among a small change that broke unions.",
        subs: &[
            (
                "Mid",
                "Identical to 2021, but with a new Roblox logo. Also, a new loading screen.",
                &[("Renders", "2022"), ("Avatar Editor", "2021"), ("Branding", "Modern"), ("Lighting", "After 2022"), ("Robux", "Modern"), ("Theme", "Mid 2022"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2022")],
            ),
            (
                "Late",
                "Avatar Editor added support for accessory adjustment and\ncustom profile pictures. Age Ratings were added.",
                &[("Renders", "Current"), ("Avatar Editor", "2022"), ("Branding", "Modern"), ("Lighting", "After 2022"), ("Robux", "Modern"), ("Theme", "Mid 2022"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "On"), ("Loading Screen", "Current")],
            ),
        ],
    },
    Preset {
        year: 2021,
        description: "New Avatar Editor, includes the catalog for the first time.",
        subs: &[(
            "Early",
            "The new Avatar Editor finally released on Xbox.\nNotably you could search and buy things from the catalog now.",
            &[("Renders", "2020"), ("Avatar Editor", "2021"), ("Branding", "Modern"), ("Lighting", "After 2020"), ("Robux", "Modern"), ("Theme", "Late 2017"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2020")],
        )],
    },
    Preset {
        year: 2020,
        description: "Identical to 2019 but with a lighting change.",
        subs: &[(
            "Mid",
            "Identical to 2019, but the lighting had changed from Legacy\nto the new Voxel lighting.",
            &[("Renders", "2020"), ("Avatar Editor", "2019"), ("Branding", "Classic"), ("Lighting", "After 2020"), ("Robux", "Modern"), ("Theme", "Late 2017"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2020")],
        )],
    },
    Preset {
        year: 2019,
        description: "Emotes, among small differences to the Avatar Editor and the new Robux icon.",
        subs: &[
            (
                "Mid",
                "Emotes, among small improvements were added to the Avatar Editor.",
                &[("Renders", "2019"), ("Avatar Editor", "2019"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Late 2017"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2018")],
            ),
            (
                "Late",
                "Robux had its icon and branding changed.",
                &[("Renders", "2019"), ("Avatar Editor", "2019"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Modern"), ("Theme", "Late 2017"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2018")],
            ),
        ],
    },
    Preset {
        year: 2018,
        description: "Avatar Editor's background had changed and everything became lowercase.",
        subs: &[(
            "Mid",
            "Identical to 2017, but with a new Avatar Editor background,\nwords now being lowercase, and loading screen changes.",
            &[("Renders", "2017"), ("Avatar Editor", "2018"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Late 2017"), ("Uppercase Words", "Off"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2018")],
        )],
    },
    Preset {
        year: 2017,
        description: "New Roblox logo, Avatar Editor was added, and mostly everything was redone.",
        subs: &[
            (
                "Early",
                "Identical to 2016, but with a brand new logo.",
                &[("Renders", "2017"), ("Search", "On"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Early 2017"), ("Uppercase Words", "On"), ("Version", "v1.0"), ("Loading Screen", "2015")],
            ),
            (
                "Mid",
                "Menu was overhauled a bit, bringing improvements to many things.\nMost notably, a proper Avatar Editor was added and new loading screen.",
                &[("Renders", "2017"), ("Avatar Editor", "2017"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Early 2017"), ("Uppercase Words", "On"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2017")],
            ),
            (
                "Late",
                "Identical to Mid 2017, but with a slight icon change.",
                &[("Renders", "2017"), ("Avatar Editor", "2017"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Late 2017"), ("Uppercase Words", "On"), ("Version", "v2.0"), ("Age Ratings", "Off"), ("Loading Screen", "2017")],
            ),
        ],
    },
    Preset {
        year: 2016,
        description: "Menu publically released, with small visual differences to 2015.",
        subs: &[
            (
                "Early",
                "First ever version of the menu to release public.\nIdentical to 2015 feature-wise but different visually.",
                &[("Renders", "2015"), ("Search", "Off"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Early 2016"), ("Uppercase Words", "On"), ("Version", "v1.0"), ("Loading Screen", "2015")],
            ),
            (
                "Mid",
                "Identical to Early 2016, but the slogan on the title screen\nchanged to the iconic \"Powering Imagination\".",
                &[("Renders", "2015"), ("Search", "Off"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Mid 2016"), ("Uppercase Words", "On"), ("Version", "v1.0"), ("Loading Screen", "2015")],
            ),
            (
                "Late",
                "Searching was added for games.",
                &[("Renders", "2017"), ("Search", "On"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Mid 2016"), ("Uppercase Words", "On"), ("Version", "v1.0"), ("Loading Screen", "2015")],
            ),
        ],
    },
    Preset {
        year: 2015,
        description: "First ever version of the menu, prior to release.",
        subs: &[(
            "Late",
            "This is a prototype beta version of the menu.\nOnly seen once publically in a Roblox stream.",
            &[("Renders", "2015"), ("Search", "Off"), ("Branding", "Classic"), ("Lighting", "Before 2020"), ("Robux", "Classic"), ("Theme", "Late 2015"), ("Uppercase Words", "On"), ("Version", "v0.5"), ("Loading Screen", "2015")],
        )],
    },
];

const PANES: [&str; 3] = ["Home", "Presets", "Configure"];
const PRESETS_TAB: usize = 1;
const CONFIGURE_TAB: usize = 2;
const BEGIN_TAB: usize = 3;
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const DARK: [f32; 3] = [19.0 / 255.0, 19.0 / 255.0, 19.0 / 255.0];
const OVERLAY: [f32; 3] = [26.0 / 255.0, 57.0 / 255.0, 76.0 / 255.0];
const GREY_BUTTON: [f32; 3] = [78.0 / 255.0, 84.0 / 255.0, 96.0 / 255.0];
const BLUE_TEXT: [f32; 3] = [0.0, 116.0 / 255.0, 189.0 / 255.0];
const TAB_UNDERLINE: [f32; 3] = [50.0 / 255.0, 181.0 / 255.0, 1.0];
const GREY_SELECTED: [f32; 3] = [50.0 / 255.0, 181.0 / 255.0, 1.0];
const SHADOW_IMG: &str = "rbxasset://textures/ui/Shell/Buttons/Generic9ScaleShadow.png";
const BUTTON_IMG: &str = "rbxasset://textures/ui/Shell/Buttons/Generic9ScaleButton@720.png";
const CHECK: &str = "rbxasset://textures/ui/Shell/Icons/Checkmark@1080.png";

// Assets.UI parts in Lobby3D world space
fn part(pos: [f32; 3], r: [f32; 9]) -> CFrame {
    CFrame { pos: Vec3::from(pos), rot: Mat3::from_cols(Vec3::new(r[0], r[3], r[6]), Vec3::new(r[1], r[4], r[7]), Vec3::new(r[2], r[5], r[8])) }
}
fn center() -> CFrame {
    part([900.65, 17.14, 490.2], [-0.786773, -0.006229, 0.617211, 0.0000282, 0.999949, 0.010128, -0.617242, 0.007985, -0.786733])
}
fn slide() -> CFrame {
    part([915.78766, 15.20087, 472.03055], [-0.786807, -0.440816, 0.431991, 0.0000176, 0.699905, 0.714236, -0.617199, 0.561973, -0.550683])
}
fn camera_part() -> CFrame {
    part([912.07178, 17.14, 476.76666], [-0.786773, -0.006229, 0.617211, 0.0000282, 0.999949, 0.010128, -0.617242, 0.007985, -0.786733])
}
fn lerp_cf(a: &CFrame, b: &CFrame, t: f32) -> CFrame {
    let qa = glam::Quat::from_mat3(&a.rot);
    let qb = glam::Quat::from_mat3(&b.rot);
    CFrame { pos: a.pos.lerp(b.pos, t), rot: Mat3::from_quat(qa.slerp(qb, t)) }
}
fn yaw(a: f32) -> CFrame {
    CFrame { pos: Vec3::ZERO, rot: Mat3::from_rotation_y(a) }
}
fn pitch(a: f32) -> CFrame {
    CFrame { pos: Vec3::ZERO, rot: Mat3::from_rotation_x(a) }
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Prompt,
    Menu,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Tabs,
    /// browsing years (Presets) or categories (Configure)
    List,
    /// inside a year's Early/Mid/Late list or a category's options
    Sub,
}

pub struct HubEvents {
    pub done: bool,
    pub changed: bool,
}

pub struct Hub {
    pub active: bool,
    stage: Stage,
    /// pane index; 4 = Begin
    tab: usize,
    mode: Mode,
    pane_x: f32,
    t0: f32,
    year: usize,
    sub: usize,
    cat: usize,
    opt: usize,
    /// option the category had when it was opened (B restores the clicked one)
    committed: String,
    cat_scroll: f32,
    year_scroll: f32,
    /// fade to black when leaving for the menu
    pub leaving: Option<f32>,
    previews: HashMap<&'static str, Shell>,
    caption: (String, f32),
    /// Begin was pressed: asking whether to turn on Fast Boot first
    ask_fast_boot: bool,
}

impl Hub {
    pub fn new(cfg: &Config) -> Self {
        Hub {
            active: cfg.ui("Fast Boot") != "On",
            // a fresh install (nothing saved yet) goes straight into the hub
            stage: if Config::saved() { Stage::Prompt } else { Stage::Menu },
            tab: 0,
            mode: Mode::Tabs,
            pane_x: 0.0,
            t0: 0.0,
            year: 0,
            sub: 0,
            cat: 0,
            opt: 0,
            committed: String::new(),
            cat_scroll: 0.0,
            year_scroll: 0.0,
            leaving: None,
            previews: HashMap::new(),
            caption: (String::new(), 0.0),
            ask_fast_boot: false,
        }
    }

    /// Debug: jump straight to a pane ("presets", "configure", "configure:Icons", "prompt").
    pub fn demo(&mut self, what: &str) {
        self.stage = Stage::Menu;
        let (pane, rest) = what.split_once(':').unwrap_or((what, ""));
        match pane {
            "prompt" => self.stage = Stage::Prompt,
            "presets" => {
                self.tab = PRESETS_TAB;
                self.mode = Mode::List;
            }
            "configure" => {
                self.tab = CONFIGURE_TAB;
                self.mode = Mode::List;
                if let Some(i) = SETTINGS.iter().position(|s| s.name.eq_ignore_ascii_case(rest)) {
                    self.cat = i;
                    self.mode = Mode::Sub;
                    self.committed = SETTINGS[i].default.to_string();
                    self.opt = SETTINGS[i].options.iter().position(|o| *o == SETTINGS[i].default).unwrap_or(0);
                }
            }
            _ => {}
        }
        self.pane_x = self.tab.min(CONFIGURE_TAB) as f32;
    }

    /// FOV camera: orbit around Center as the panes slide, with a slow bob.
    pub fn camera(&self, now: f32) -> CFrame {
        let c = center();
        // Contrast.Brightness tweens -1 -> 0 when the menu opens: Slide:Lerp(Camera, 1 + Brightness)
        let k = if self.stage == Stage::Menu { ease_in_out_quad(((now - self.t0) / 2.0).clamp(0.0, 1.0)) } else { 0.0 };
        let diff = c.inverse().mul(&lerp_cf(&slide(), &camera_part(), k));
        // the place's panes were Home, Donate, Presets, Configure: keep those orbit stops
        let stop = if self.pane_x <= 1.0 { self.pane_x * 2.0 } else { 2.0 + (self.pane_x - 1.0) };
        let a = (3.0 - stop) / 3.0 * 3.752_457_9;
        c.mul(&yaw(a)).mul(&diff).mul(&pitch((now / 2.0).cos() / 50.0))
    }

    fn visible(cfg: &Config, name: &str) -> bool {
        let modern = cfg.ui("Version") >= "v2.0";
        match name {
            "Age Ratings" | "Avatar Editor" => modern,
            "Avatar Theme" => modern && cfg.ui("Avatar Editor").parse::<u32>().unwrap_or(0) > 2020,
            "Search" => !modern,
            _ => true,
        }
    }

    fn set(cfg: &mut Config, name: &str, value: &str) {
        cfg.ui.insert(name.to_string(), value.to_string());
        crate::strings::set_style(cfg.ui("Uppercase Words") != "Off", cfg.ui("Branding") == "Modern");
        if name == "Icons" {
            cfg.icons = match value {
                "Xbox" => IconSet::Xbox,
                "Keyboard" => IconSet::Keyboard,
                "PlayStation" => IconSet::PlayStation,
                _ => IconSet::Auto,
            };
            crate::input::set_icon_set(cfg.icons);
        }
    }

    pub fn frame(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, cfg: &mut Config) -> HubEvents {
        let mut ev = HubEvents { done: false, changed: false };
        let now = ui.time;
        let dt = 1.0 / 60.0;
        self.pane_x += (self.tab.min(CONFIGURE_TAB) as f32 - self.pane_x) * (1.0 - (-10.0 * dt as f32).exp());
        if let Some(t) = self.leaving {
            let k = ((now - t) / 0.6).clamp(0.0, 1.0);
            if self.stage == Stage::Menu {
                self.menu(ui, &Input::default(), audio, cfg, &mut ev, true);
            }
            ui.rect(full(ui), [0.0; 3], ease_in_out_quad(k));
            if k >= 1.0 {
                self.active = false;
                ev.done = true;
            }
            return ev;
        }
        if self.ask_fast_boot {
            self.menu(ui, &Input::default(), audio, cfg, &mut ev, true);
            self.fast_boot_prompt(ui, input, audio, cfg, &mut ev);
            return ev;
        }
        match self.stage {
            Stage::Prompt => self.prompt(ui, input, audio, cfg, &mut ev),
            Stage::Menu => self.menu(ui, input, audio, cfg, &mut ev, false),
        }
        ev
    }

    /// After Begin (Fast Boot off): offer to skip the hub on later launches.
    fn fast_boot_prompt(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, cfg: &mut Config, ev: &mut HubEvents) {
        let before = ui.focus;
        ui.navigate(input);
        if ui.moved && ui.focus != before && before.is_some() {
            audio.play("MoveSelection", 1.0);
        }
        ui.rect(full(ui), [0.0; 3], 0.6);
        ui.rect(Rect::new(full(ui).x, 194.0, full(ui).w, 683.0), OVERLAY, 1.0);
        ui.image("rbxasset://glyphs/Hub/Info.png", Rect::new(228.0, 375.0, 320.0, 320.0), WHITE, 1.0);
        ui.label("Enable Fast Boot?", Font::Regular, 48.0, Rect::new(776.0, 300.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.paragraph(
            "This will boot you directly into the launcher with your own settings, skipping the hub and the initial prompt.",
            Font::Light,
            34.0,
            Rect::new(776.0, 392.0, 1130.0, 90.0),
            WHITE,
            1.0,
            2,
        );
        ui.paragraph(
            &format!("You can disable this by going to Settings and pressing {} to disable fast boot.", button_name('X')),
            Font::Light,
            34.0,
            Rect::new(776.0, 476.0, 1130.0, 90.0),
            WHITE,
            1.0,
            2,
        );
        let yes = Rect::new(774.0, 714.0, 328.0, 66.0);
        let no = Rect::new(1106.0, 714.0, 320.0, 66.0);
        let (y, n) = (id("hubfastboot", 0), id("hubfastboot", 1));
        let yf = ui.selectable(y, yes);
        let nf = ui.selectable(n, no);
        if !ui.focus_valid() {
            ui.set_focus(y);
        }
        for (r, f, t) in [(yes, yf, "Yes"), (no, nf, "No")] {
            popup_button(ui, r, t, f);
        }
        if ui.pressed(y) || ui.pressed(n) {
            audio.play("ButtonPress", 1.0);
            if ui.pressed(y) {
                Self::set(cfg, "Fast Boot", "On");
                ev.changed = true;
            }
            self.ask_fast_boot = false;
            self.leaving = Some(ui.time);
        } else if input.has(Btn::B) {
            // back to the hub, Begin still selected
            audio.play("ButtonPress", 1.0);
            self.ask_fast_boot = false;
            ui.set_focus(id("hubtab", BEGIN_TAB as u64));
        }
    }

    // >> startup prompt

    fn prompt(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, cfg: &mut Config, ev: &mut HubEvents) {
        let before = ui.focus;
        ui.navigate(input);
        if ui.moved && ui.focus != before && before.is_some() {
            audio.play("MoveSelection", 1.0);
        }
        ui.rect(full(ui), [0.0; 3], 1.0);
        ui.rect(Rect::new(full(ui).x, 194.0, full(ui).w, 683.0), OVERLAY, 1.0);
        ui.image("rbxasset://glyphs/Hub/Info.png", Rect::new(228.0, 375.0, 320.0, 320.0), WHITE, 1.0);
        ui.label("Configure Xbox UI Settings?", Font::Regular, 48.0, Rect::new(776.0, 300.0, 1100.0, 60.0), Align::Start, Align::Center, WHITE, 1.0);
        ui.paragraph("Would you like to modify your settings for the Xbox UI, or go to the UI right away?", Font::Light, 34.0, Rect::new(776.0, 392.0, 1130.0, 90.0), WHITE, 1.0, 2);
        ui.paragraph(
            "Consider enabling fast boot if you would like to access the UI immediately without seeing this prompt.",
            Font::Light,
            34.0,
            Rect::new(776.0, 476.0, 1130.0, 90.0),
            WHITE,
            1.0,
            2,
        );
        let proceed = Rect::new(774.0, 714.0, 328.0, 66.0);
        let configure = Rect::new(1106.0, 714.0, 320.0, 66.0);
        let (p, c) = (id("hubprompt", 0), id("hubprompt", 1));
        let pf = ui.selectable(p, proceed);
        let cf = ui.selectable(c, configure);
        if !ui.focus_valid() {
            ui.set_focus(p);
        }
        for (r, f, t) in [(proceed, pf, "Proceed to UI"), (configure, cf, "Configure")] {
            popup_button(ui, r, t, f);
        }
        if ui.pressed(p) {
            audio.play("ButtonPress", 1.0);
            self.active = false;
            ev.done = true;
        } else if ui.pressed(c) {
            audio.play("ButtonPress", 1.0);
            self.stage = Stage::Menu;
            self.t0 = ui.time;
            ui.focus = None;
        }
        let _ = cfg;
    }

    // >> hub panes

    fn menu(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, cfg: &mut Config, ev: &mut HubEvents, inert: bool) {
        let fade = ease_in_out_quad(((ui.time - self.t0) / 0.8).clamp(0.0, 1.0));
        let f = full(ui);
        // Background: white -> dark gradient band under the tabs
        gradient(ui, Rect::new(f.x, 1080.0 - 300.0, f.w, 300.0 + (f.y + f.h - 1080.0)), 0.0, 0.75);
        let mut caption = String::new();

        if !inert {
            self.input(ui, input, audio, cfg, ev);
        }

        // panes slide horizontally; each sits one screen-width apart
        for (i, _) in PANES.iter().enumerate() {
            let dx = (i as f32 - self.pane_x) * 1920.0;
            if dx.abs() >= 1920.0 {
                continue;
            }
            let old = ui.push_translate(glam::Vec2::new(dx, 0.0));
            match i {
                0 => self.home_pane(ui),
                PRESETS_TAB => caption = self.presets_pane(ui, cfg, i == self.tab).unwrap_or(caption),
                _ => caption = self.configure_pane(ui, cfg, i == self.tab).unwrap_or(caption),
            }
            ui.pop_translate(old);
        }

        // caption (Info.Description) above the tabs
        if caption != self.caption.0 {
            self.caption = (caption, ui.time);
        }
        if !self.caption.0.is_empty() {
            let a = ((ui.time - self.caption.1) / 0.25).clamp(0.0, 1.0);
            let lines: Vec<&str> = self.caption.0.lines().collect();
            // Info.Description sits above the tabs; while browsing years it runs under the preview
            let (size, base) = match (self.mode, self.tab) {
                (Mode::Sub, _) => (60.0, 955.0),
                (_, PRESETS_TAB) => (40.0, 850.0),
                _ => (60.0, 720.0),
            };
            let y0 = base - (lines.len() as f32 - 1.0) * size * 1.15;
            for (k, l) in lines.iter().enumerate() {
                ui.label(l, Font::Light, size, Rect::new(0.0, y0 + k as f32 * size * 1.15, 1920.0, size * 1.15), Align::Center, Align::Center, WHITE, a);
            }
        }

        // tab headers (hidden while a submenu has the sidebar open)
        if self.mode != Mode::Sub {
            let names = ["Home", "Presets", "Configure", "Begin"];
            let widths: Vec<f32> = names.iter().map(|n| ui.measure(n, Font::Light, 75.0) + 40.0).collect();
            let total: f32 = widths.iter().sum::<f32>() + 100.0 * (names.len() - 1) as f32;
            let mut x = 960.0 - total / 2.0;
            for (i, n) in names.iter().enumerate() {
                let r = Rect::new(x, 935.0, widths[i], 80.0);
                let sel = id("hubtab", i as u64);
                let focused = !inert && self.mode == Mode::Tabs && ui.selectable(sel, r);
                if focused && i != self.tab {
                    self.tab = i;
                    audio.play("MoveSelection", 1.0);
                }
                // TabDock look: current tab white, the rest blue, blue underline while selected
                let current = i == self.tab;
                ui.label(n, Font::Light, 75.0, r, Align::Center, Align::Center, if current { WHITE } else { BLUE_TEXT }, fade);
                if focused {
                    ui.rect(Rect::new(r.x + 20.0, r.y + r.h + 4.0, r.w - 40.0, 5.0), TAB_UNDERLINE, fade);
                }
                x += widths[i] + 100.0;
            }
            if !inert && self.mode == Mode::Tabs && !ui.focus_valid() {
                ui.set_focus(id("hubtab", self.tab as u64));
            }
        }
        // Begin pressed
        if !inert && self.mode == Mode::Tabs && ui.pressed(id("hubtab", BEGIN_TAB as u64)) {
            audio.play("ButtonPress", 1.0);
            if cfg.ui("Fast Boot") == "On" {
                self.leaving = Some(ui.time);
            } else {
                self.ask_fast_boot = true;
                ui.focus = None;
            }
        }
        ui.rect(full(ui), [0.0; 3], 1.0 - fade);
    }

    fn input(&mut self, ui: &mut Ui, input: &Input, audio: &mut Audio, cfg: &mut Config, ev: &mut HubEvents) {
        let up = input.has(Btn::Up);
        let down = input.has(Btn::Down);
        let left = input.has(Btn::Left);
        let right = input.has(Btn::Right);
        let a = input.has(Btn::A) || ui.activated && ui.mouse.is_some() && false;
        let b = input.has(Btn::B);
        match self.mode {
            Mode::Tabs => {
                let mut t = self.tab as i32;
                if left || input.has(Btn::LB) {
                    t -= 1;
                }
                if right || input.has(Btn::RB) {
                    t += 1;
                }
                let t = t.clamp(0, BEGIN_TAB as i32) as usize;
                if t != self.tab {
                    self.tab = t;
                    ui.set_focus(id("hubtab", t as u64));
                    audio.play("MoveSelection", 1.0);
                }
                if self.tab == 0 && input.has(Btn::X) {
                    audio.play("ButtonPress", 1.0);
                    crate::launch::open_url(crate::launch::GITHUB_URL);
                }
                if self.tab == 0 && input.has(Btn::Y) {
                    audio.play("ButtonPress", 1.0);
                    crate::launch::open_url(crate::launch::DISCORD_URL);
                }
                if (up || a) && (self.tab == PRESETS_TAB || self.tab == CONFIGURE_TAB) {
                    self.mode = Mode::List;
                    audio.play(if a { "ButtonPress" } else { "MoveSelection" }, 1.0);
                }
                // A on Begin is handled by the tab selectable
            }
            Mode::List => {
                if b || (self.tab == PRESETS_TAB && down && self.year + 1 >= PRESETS.len()) || (self.tab == CONFIGURE_TAB && down) {
                    self.mode = Mode::Tabs;
                    ui.set_focus(id("hubtab", self.tab as u64));
                    audio.play("ButtonPress", 1.0);
                    return;
                }
                if self.tab == PRESETS_TAB {
                    if up && self.year > 0 {
                        self.year -= 1;
                        audio.play("MoveSelection", 1.0);
                    }
                    if down && self.year + 1 < PRESETS.len() {
                        self.year += 1;
                        audio.play("MoveSelection", 1.0);
                    }
                    if a || right {
                        self.mode = Mode::Sub;
                        self.sub = PRESETS[self.year].subs.len() - 1;
                        audio.play("ButtonPress", 1.0);
                    }
                } else {
                    let cats: Vec<usize> = (0..SETTINGS.len()).filter(|&i| Self::visible(cfg, SETTINGS[i].name)).collect();
                    let mut k = cats.iter().position(|&c| c == self.cat).unwrap_or(0) as i32;
                    if left {
                        k -= 1;
                    }
                    if right {
                        k += 1;
                    }
                    let k = k.clamp(0, cats.len() as i32 - 1) as usize;
                    if cats[k] != self.cat {
                        self.cat = cats[k];
                        audio.play("MoveSelection", 1.0);
                    }
                    if a {
                        let s = &SETTINGS[self.cat];
                        self.committed = cfg.ui(s.name).to_string();
                        self.opt = s.options.iter().position(|o| *o == self.committed).unwrap_or(0);
                        self.mode = Mode::Sub;
                        audio.play("ButtonPress", 1.0);
                    }
                }
            }
            Mode::Sub => {
                if self.tab == PRESETS_TAB {
                    let p = &PRESETS[self.year];
                    if up && self.sub > 0 {
                        self.sub -= 1;
                        audio.play("MoveSelection", 1.0);
                    }
                    if down && self.sub + 1 < p.subs.len() {
                        self.sub += 1;
                        audio.play("MoveSelection", 1.0);
                    }
                    if a {
                        for (k, v) in p.subs[self.sub].2 {
                            Self::set(cfg, k, v);
                        }
                        cfg.ui.insert("Preset".into(), format!("{} {}", p.subs[self.sub].0, p.year));
                        ev.changed = true;
                        audio.play("ButtonPress", 1.0);
                    }
                    if b || left {
                        self.mode = Mode::List;
                        audio.play("ButtonPress", 1.0);
                    }
                } else {
                    let s = &SETTINGS[self.cat];
                    let mut moved = false;
                    if up && self.opt > 0 {
                        self.opt -= 1;
                        moved = true;
                    }
                    if down && self.opt + 1 < s.options.len() {
                        self.opt += 1;
                        moved = true;
                    }
                    if moved {
                        audio.play("MoveSelection", 1.0);
                        // SelectionGained: ChangeSetting previews the hovered option
                        Self::set(cfg, s.name, s.options[self.opt]);
                    }
                    if a {
                        self.committed = s.options[self.opt].to_string();
                        Self::set(cfg, s.name, &self.committed);
                        ev.changed = true;
                        audio.play("ButtonPress", 1.0);
                    }
                    if b {
                        // Menu.Action: restore the clicked option and close the sidebar
                        let c = self.committed.clone();
                        Self::set(cfg, s.name, &c);
                        ev.changed = true;
                        self.mode = Mode::List;
                        audio.play("ButtonPress", 1.0);
                    }
                }
            }
        }
    }

    fn home_pane(&mut self, ui: &mut Ui) {
        // the XBLOX: REWIND logo (used here and nowhere else in the menus)
        ui.image("rbxasset://branding/FullLogo.png", Rect::new(960.0 - 440.0, 110.0, 880.0, 180.0), WHITE, 1.0);
        let slogan = "Relive the past as if you were in 2016!";
        let w = ui.measure(slogan, Font::Light, 44.0) + 100.0;
        ui.rect(Rect::new(960.0 - w / 2.0, 330.0, w, 80.0), [0.0; 3], 0.5);
        ui.label(slogan, Font::Light, 44.0, Rect::new(960.0 - w / 2.0, 330.0, w, 80.0), Align::Center, Align::Center, WHITE, 1.0);
        ui.label("Controls", Font::Light, 28.0, Rect::new(660.0, 450.0, 600.0, 30.0), Align::Center, Align::Center, WHITE, 1.0);
        ui.image("rbxasset://textures/ui/Shell/ScreenAdjustment/Controller@1080.png", Rect::new(960.0 - 220.0, 490.0, 439.0, 287.0), WHITE, 1.0);
        let left = [("A", "Enter / Space"), ("B", "Esc / Backspace"), ("X", "X"), ("Y", "Y")];
        for (k, (b, keys)) in left.iter().enumerate() {
            let y = 500.0 + k as f32 * 64.0;
            ui.image(&glyph(b.chars().next().unwrap()), Rect::new(1220.0, y, 44.0, 44.0), WHITE, 1.0);
            ui.label(keys, Font::Regular, 28.0, Rect::new(1276.0, y, 400.0, 44.0), Align::Start, Align::Center, WHITE, 0.85);
        }
        let right = [("LB / RB", "Q / E"), ("D-pad", "Arrows / WASD"), ("Fullscreen", "F11"), ("Filter", "F1"), ("Hide UI", "F2")];
        for (k, (b, keys)) in right.iter().enumerate() {
            let y = 500.0 + k as f32 * 64.0;
            ui.label(b, Font::Regular, 28.0, Rect::new(360.0, y, 180.0, 44.0), Align::End, Align::Center, WHITE, 1.0);
            ui.label(keys, Font::Regular, 28.0, Rect::new(560.0, y, 180.0, 44.0), Align::Start, Align::Center, WHITE, 0.85);
        }
        // credits and the project links (X: GitHub, Y: Discord; both clickable)
        ui.label("Created by Numan  ·  Discord @corescripts  ·  GitHub @numanluau", Font::Light, 26.0, Rect::new(0.0, 836.0, 1920.0, 34.0), Align::Center, Align::Center, WHITE, 0.75);
        let links = [('X', "Source on GitHub", crate::launch::GITHUB_URL), ('Y', "Join the Discord", crate::launch::DISCORD_URL)];
        let widths: Vec<f32> = links.iter().map(|l| 52.0 + ui.measure(l.1, Font::Regular, 28.0)).collect();
        let mut x = 960.0 - (widths.iter().sum::<f32>() + 60.0) / 2.0;
        for (k, (b, text, url)) in links.iter().enumerate() {
            let r = Rect::new(x, 880.0, widths[k], 44.0);
            let hover = ui.mouse.is_some_and(|m| r.contains(m));
            ui.image(&glyph(*b), Rect::new(x, 880.0, 44.0, 44.0), WHITE, 1.0);
            ui.label(text, Font::Regular, 28.0, Rect::new(x + 52.0, 880.0, widths[k], 44.0), Align::Start, Align::Center, WHITE, if hover { 1.0 } else { 0.85 });
            if hover && ui.clicked {
                crate::launch::open_url(url);
            }
            x += widths[k] + 60.0;
        }
    }

    fn presets_pane(&mut self, ui: &mut Ui, cfg: &mut Config, active: bool) -> Option<String> {
        ui.label("Presets", Font::Light, 60.0, Rect::new(116.0, 161.0, 600.0, 35.0), Align::Start, Align::Center, WHITE, 1.0);
        let applied = cfg.ui.get("Preset").cloned().unwrap_or_default();
        let opened = active && self.mode == Mode::Sub;
        // years list (scrolls to keep the selection inside six rows)
        let target = (self.year as f32 - 4.0).clamp(0.0, (PRESETS.len() as f32 - 6.0).max(0.0));
        self.year_scroll += (target - self.year_scroll) * 0.2;
        let list_w = if opened { 80.0 } else { 360.0 };
        ui.push_clip(Rect::new(90.0, 262.0, 400.0, 596.0));
        for (i, p) in PRESETS.iter().enumerate() {
            let y = 270.0 + (i as f32 - self.year_scroll) * 100.0;
            let r = Rect::new(100.0, y, list_w, 80.0);
            let focused = active && self.mode == Mode::List && i == self.year;
            let mine = applied.ends_with(&p.year.to_string());
            let year = p.year.to_string();
            list_button_at(ui, r, if opened { "" } else { &year }, 76.0, focused, opened && i == self.year, mine && !opened);
            sheet_icon(ui, PRESETS_SHEET, 9, (p.year - 2015) as usize, focused, Rect::new(r.x + 22.0, y + 20.0, 40.0, 40.0));
        }
        ui.pop_clip();
        if self.year_scroll > 0.05 {
            ui.image_rotated("rbxasset://textures/ui/Shell/Icons/DownIndicatorIcon@1080.png", glam::Vec2::new(100.0 + list_w / 2.0, 243.0), glam::Vec2::new(33.0, 30.0), std::f32::consts::PI, 0.75);
        }
        // Early / Mid / Late
        let p = &PRESETS[self.year];
        let mut caption = Some(p.description.to_string());
        let mut preview_set: &[(&str, &str)] = p.subs.last().map(|s| s.2).unwrap_or(&[]);
        if opened {
            for (k, (name, info, set)) in p.subs.iter().enumerate() {
                let r = Rect::new(220.0, 270.0 + k as f32 * 100.0, 360.0, 80.0);
                let mine = applied == format!("{name} {}", p.year);
                list_button(ui, r, name, k == self.sub, false, mine);
                if k == self.sub {
                    caption = Some(info.to_string());
                    preview_set = set;
                }
            }
        }
        if !active {
            return None;
        }
        // preview with the hovered preset applied
        let mut pc = cfg.clone();
        for (k, v) in preview_set {
            pc.ui.insert(k.to_string(), v.to_string());
        }
        self.preview(ui, Rect::new(780.0, 266.0, 960.0, 540.0), &pc, "Version");
        crate::strings::set_style(cfg.ui("Uppercase Words") != "Off", cfg.ui("Branding") == "Modern");
        caption
    }

    fn configure_pane(&mut self, ui: &mut Ui, cfg: &mut Config, active: bool) -> Option<String> {
        let s = &SETTINGS[self.cat];
        if active && self.mode == Mode::Sub {
            ui.label(s.name, Font::Light, 60.0, Rect::new(116.0, 161.0, 700.0, 35.0), Align::Start, Align::Center, WHITE, 1.0);
            let committed = self.committed.clone();
            for (k, o) in s.options.iter().enumerate() {
                let r = Rect::new(100.0, 270.0 + k as f32 * 100.0, 360.0, 80.0);
                list_button(ui, r, o, k == self.opt, false, *o == committed);
            }
            let r = Rect::new(576.0, 100.0, 1248.0, 702.0);
            self.preview(ui, r, &cfg.clone(), s.name);
            return Some(s.info.get(self.opt).copied().unwrap_or("").to_string());
        }
        // category strip along the bottom (300x80, scaled 1.25 while selected)
        let cats: Vec<usize> = (0..SETTINGS.len()).filter(|&i| Self::visible(cfg, SETTINGS[i].name)).collect();
        let k = cats.iter().position(|&c| c == self.cat).unwrap_or(0);
        let target = (k as f32 * 320.0 - 1650.0 / 2.0 + 150.0).clamp(0.0, (cats.len() as f32 * 320.0 - 20.0 - 1650.0).max(0.0));
        self.cat_scroll += (target - self.cat_scroll) * 0.2;
        let x0 = 135.0 - self.cat_scroll;
        ui.push_clip(Rect::new(135.0 - 40.0, 760.0, 1650.0 + 80.0, 190.0));
        for (j, &c) in cats.iter().enumerate() {
            let focused = active && self.mode == Mode::List && c == self.cat;
            let sc = if focused { 1.25 } else { 1.0 };
            let base = Rect::new(x0 + j as f32 * 320.0, 815.0, 300.0, 80.0);
            let r = Rect::new(base.x + base.w * (1.0 - sc) / 2.0, base.y + base.h * (1.0 - sc) / 2.0, base.w * sc, base.h * sc);
            list_button(ui, r, "", focused, false, false);
            sheet_icon(ui, CONFIGURE_SHEET, 17, c, focused, Rect::new(r.x + 22.0 * sc, r.y + r.h / 2.0 - 20.0 * sc, 40.0 * sc, 40.0 * sc));
            let t = ui.fit(SETTINGS[c].name, Font::Regular, 30.0 * sc, r.w - 90.0 * sc);
            ui.label(&t, Font::Regular, 30.0 * sc, Rect::new(r.x + 76.0 * sc, r.y, r.w, r.h), Align::Start, Align::Center, if focused { DARK } else { WHITE }, 1.0);
        }
        ui.pop_clip();
        ui.image("rbxasset://textures/ui/Settings/Slider/Left.png", Rect::new(135.0 - 25.0 - 26.0, 835.0, 26.0, 40.0), WHITE, 0.25);
        ui.image("rbxasset://textures/ui/Settings/Slider/Right.png", Rect::new(135.0 + 1650.0 + 25.0, 835.0, 26.0, 40.0), WHITE, 0.25);
        if !active {
            return None;
        }
        let r = Rect::new(576.0, 60.0, 1248.0 * 0.9, 702.0 * 0.9);
        let r = Rect::new(960.0 - r.w / 2.0, r.y, r.w, r.h);
        self.preview(ui, r, &cfg.clone(), s.name);
        Some(s.description.to_string())
    }

    /// Preview.Render: the real menu drawn into a window, showing whatever the setting affects.
    fn preview(&mut self, ui: &mut Ui, r: Rect, cfg: &Config, setting: &str) {
        let kind: &'static str = match setting {
            "Fast Boot" | "Theme" | "Version" | "Screen" => "engagement",
            "Avatar Editor" | "Avatar Theme" | "Renders" => "tab:1",
            "Robux" => "tab:4",
            "Loading Screen" => "loading",
            "Search" | "Age Ratings" => "tab:2",
            _ => "tab:0",
        };
        ui.nine_slice("rbxasset://textures/ui/Shell/Buttons/Generic9ScaleShadow.png", r.inset(-25.0), [10.0, 10.0, 28.0, 28.0], WHITE, 1.0);
        ui.rect(r, [0.0784, 0.1686, 0.2353], 1.0);
        let shell = self.previews.entry(kind).or_insert_with(|| {
            let mut s = Shell::new(cfg.clone());
            s.demo(kind);
            s
        });
        shell.cfg = cfg.clone();
        if kind.starts_with("tab:") {
            let start: usize = kind[4..].parse().unwrap_or(0);
            shell.preview_tab((start + (ui.time / 3.0) as usize) % 5, ui.time);
        }
        crate::strings::set_style(cfg.ui("Uppercase Words") != "Off", cfg.ui("Branding") == "Modern");
        let saved = ui.push_canvas(r);
        ui.image("rbxasset://textures/ui/Shell/Background/Home_screen_01.png", Rect::new(0.0, 0.0, 1920.0, 1080.0), WHITE, 1.0);
        ui.rect(Rect::new(0.0, 0.0, 1920.0, 1080.0), [0.0; 3], 0.25);
        let (focus, prev_inert) = (ui.focus, ui.inert);
        ui.inert = true;
        let mut mute = Audio::muted();
        shell.preview_frame(ui, &mut mute);
        ui.inert = prev_inert;
        ui.focus = focus;
        ui.pop_canvas(saved);
        let cur = format!("Currently set to {}", cfg.ui(setting));
        if setting != "Version" || self.tab == CONFIGURE_TAB {
            ui.label(&cur, Font::Light, 28.0, Rect::new(r.x, r.y + 4.0, r.w, 36.0), Align::Center, Align::Center, WHITE, 0.8);
        }
    }
}

fn full(ui: &Ui) -> Rect {
    Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, ui.screen.x / ui.scale, ui.screen.y / ui.scale)
}

fn gradient(ui: &mut Ui, r: Rect, a0: f32, a1: f32) {
    let n = 24;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let (y0, y1) = ((r.y + r.h * t).round(), (r.y + r.h * (i + 1) as f32 / n as f32).round());
        ui.rect(Rect::new(r.x, y0, r.w, y1 - y0), [0.0; 3], a0 + (a1 - a0) * t);
    }
}

const CONFIGURE_SHEET: &str = "rbxasset://glyphs/Hub/ConfigureSheet.png";
const PRESETS_SHEET: &str = "rbxasset://glyphs/Hub/PresetsSheet.png";

/// One cell of the menu's icon sheets: two columns (normal, selected), one row per entry.
fn sheet_icon(ui: &mut Ui, sheet: &str, rows: usize, row: usize, selected: bool, r: Rect) {
    let (v0, v1) = (row as f32 / rows as f32, (row + 1) as f32 / rows as f32);
    let (u0, u1) = if selected { (0.5, 1.0) } else { (0.0, 0.5) };
    ui.image_uv(sheet, r, [u0, v0, u1, v1], 1.0);
}

/// The error overlay's button: flat blue with dark text while selected, grey otherwise.
fn popup_button(ui: &mut Ui, r: Rect, text: &str, focused: bool) {
    ui.rect(r, if focused { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
    ui.label(text, Font::Regular, 34.0, r, Align::Center, Align::Center, if focused { DARK } else { WHITE }, 1.0);
}

/// Buttons:Create - translucent white button, blue with a white edge while selected, checkmark when applied.
fn list_button(ui: &mut Ui, r: Rect, text: &str, focused: bool, open: bool, check: bool) {
    list_button_at(ui, r, text, 18.0, focused, open, check)
}

/// The menu's 9-slice button (Settings/Account style): grey, blue while selected, with its drop shadow.
fn list_button_at(ui: &mut Ui, r: Rect, text: &str, text_x: f32, focused: bool, open: bool, check: bool) {
    ui.rect(r, if focused || open { GREY_SELECTED } else { GREY_BUTTON }, 1.0);
    if !text.is_empty() {
        ui.label(text, Font::Regular, 30.0, Rect::new(r.x + text_x, r.y, r.w - text_x - 62.0, r.h), Align::Start, Align::Center, if focused { DARK } else { WHITE }, 1.0);
    }
    if check {
        ui.image(CHECK, Rect::new(r.x + r.w - 58.0, r.y + r.h / 2.0 - 16.0, 32.0, 32.0), if focused { DARK } else { WHITE }, 1.0);
    }
}

// >> end << //
