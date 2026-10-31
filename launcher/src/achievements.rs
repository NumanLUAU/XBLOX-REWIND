/*
    written by: @corescripts
*/

//! Port of Shell/AchievementManager: the eleven ROBLOX (Xbox One) achievements, tracked per Roblox
//! user id and saved locally (there is no Xbox Live here), plus the unlock toast (Shell-style card).
//! Triggers follow AchievementManager.lua: games played, ratings, play time, avatar changes,
//! consecutive days, parties and playing with friends.
//! On top of those: the account's Roblox badges. Every badge is worth gamerscore by how rare it
//! is, badge milestones are achievements of their own, and badges earned in a game pop up when
//! the player comes back to the launcher.

// >> imports
use crate::audio::Audio;
use crate::ui::{ease_in_out_quad, Align, Font, Rect, Ui};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Def {
    pub id: u8,
    pub name: &'static str,
    pub description: &'static str,
    pub gamerscore: u32,
}

/// AchievementManager.AchivementId order, names/descriptions/gamerscore as listed on Xbox Live,
/// then the badge milestones (ids 30+).
pub const DEFS: [Def; 21] = [
    Def { id: 2, name: "Scout", description: "You played a Roblox game!", gamerscore: 10 },
    Def { id: 3, name: "Explorer", description: "You played 5 different Roblox games.", gamerscore: 50 },
    Def { id: 4, name: "Trailblazer", description: "Played 15 different Roblox games.", gamerscore: 200 },
    Def { id: 5, name: "Pollster", description: "You rated 5 Roblox games.", gamerscore: 50 },
    Def { id: 6, name: "Marathon", description: "You played a Roblox game for one hour!", gamerscore: 50 },
    Def { id: 7, name: "One Name, Many Faces", description: "You changed your avatar's look!", gamerscore: 50 },
    Def { id: 8, name: "3 Day Roll", description: "You played Roblox 3 days in a row.", gamerscore: 100 },
    Def { id: 9, name: "10 Day Roll", description: "You played Roblox 10 days in a row.", gamerscore: 200 },
    Def { id: 10, name: "20 Day Roll", description: "You played Roblox 20 days in a row.", gamerscore: 200 },
    Def { id: 11, name: "Strength in Numbers", description: "You joined an Xbox party and played Roblox.", gamerscore: 45 },
    Def { id: 12, name: "Four's Company", description: "You played a Roblox game with 3 of your friends!", gamerscore: 45 },
    Def { id: 30, name: "First Steps", description: "You earned your first badge.", gamerscore: 10 },
    Def { id: 31, name: "Collector", description: "You earned 10 badges.", gamerscore: 25 },
    Def { id: 32, name: "Curator", description: "You earned 100 badges.", gamerscore: 100 },
    Def { id: 33, name: "Archivist", description: "You earned 500 badges.", gamerscore: 200 },
    Def { id: 34, name: "Hall of Fame", description: "You earned 1,000 badges.", gamerscore: 300 },
    Def { id: 35, name: "Rare Find", description: "You earned a badge fewer than 1% of players have.", gamerscore: 50 },
    Def { id: 36, name: "One in a Thousand", description: "You earned a badge fewer than 0.1% of players have.", gamerscore: 1000 },
    Def { id: 37, name: "Busy Day", description: "You earned 5 badges in one day.", gamerscore: 30 },
    Def { id: 38, name: "Back to Back", description: "You earned badges two days in a row.", gamerscore: 20 },
    Def { id: 39, name: "Daily Grind", description: "You earned badges 7 days in a row.", gamerscore: 75 },
];

/// Badge milestones: (badges needed, achievement).
const BADGE_COUNTS: [(usize, u8); 5] = [(1, 30), (10, 31), (100, 32), (500, 33), (1000, 34)];
/// a badge counts as rare under this win rate (Roblox's "rarity", 0..1)
const RARE_RATE: f32 = 0.01;
const ULTRA_RARE_RATE: f32 = 0.001;
/// rarity is only trusted once this many players have the badge
const TRUSTED_WINS: u64 = 1000;

/// A badge the account owns, as the badges API reports it.
pub struct BadgeInfo {
    pub id: u64,
    pub name: String,
    /// rarity: share of players who earned it (0..1; 0 = not rated yet)
    pub rate: f32,
    pub won_yesterday: u64,
    pub won_ever: u64,
    /// unix seconds it was awarded
    pub awarded: u64,
    pub icon: String,
}

/// Gamerscore for one badge. Rarity sets the tier (like Xbox's 5 to 100G); a badge few people
/// have ever won can't prove it is hard, and one most of its winners got yesterday is being
/// farmed right now, so both are held back.
pub fn badge_points(rate: f32, won_yesterday: u64, won_ever: u64) -> u32 {
    let tier = match rate {
        // 0.0% is what Roblox shows for old badges in huge games, easy ones included: judge
        // those by whether people still win them every day
        r if r <= 0.0 => {
            if won_ever < TRUSTED_WINS {
                5
            } else if won_yesterday >= 50 {
                15
            } else {
                30
            }
        }
        r if r >= 0.5 => 5,
        r if r >= 0.25 => 10,
        r if r >= 0.10 => 15,
        r if r >= 0.05 => 20,
        r if r >= RARE_RATE => 30,
        r if r >= ULTRA_RARE_RATE => 50,
        _ => 100,
    };
    let mut points = if won_ever < TRUSTED_WINS { tier.min(10) } else { tier };
    if won_ever > 0 && won_yesterday as f32 / won_ever as f32 > 0.05 {
        points = ((points as f32 * 0.8 / 5.0).round() as u32 * 5).max(5);
    }
    points
}

pub const SCOUT: u8 = 2;
pub const EXPLORER: u8 = 3;
pub const TRAILBLAZER: u8 = 4;
pub const POLLSTER: u8 = 5;
pub const MARATHON: u8 = 6;
pub const ONE_NAME_MANY_FACES: u8 = 7;
pub const STRENGTH_IN_NUMBERS: u8 = 11;
pub const FOURS_COMPANY: u8 = 12;

const GAMES_FOR_AWARD_SAMPLER: usize = 5;
const GAMES_FOR_WORLD_TRAVELER: usize = 15;
const GAMES_RATED_FOR_POLLSTER: usize = 5;
const PLAY_SECONDS_FOR_DEEP_DIVER: f32 = 60.0 * 60.0;
pub const FRIENDS_FOR_FOURS_COMPANY: usize = 3;

pub fn def(id: u8) -> &'static Def {
    DEFS.iter().find(|d| d.id == id).unwrap_or(&DEFS[0])
}

/// The achievement art with rounded corners (toast).
pub fn icon_round(id: u8) -> String {
    let n = DEFS.iter().position(|d| d.id == id).unwrap_or(0) + 1;
    format!("rbxasset://achievements/r{n}.png")
}

pub fn icon(id: u8) -> String {
    let n = DEFS.iter().position(|d| d.id == id).unwrap_or(0) + 1;
    format!("rbxasset://achievements/{n}.png")
}

fn today() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86400).unwrap_or(0)
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Progress {
    /// achievement id -> unix seconds it unlocked
    unlocked: BTreeMap<u8, u64>,
    places: Vec<u64>,
    rated: Vec<u64>,
    /// days (since 1970) Roblox was played on
    days: Vec<u64>,
    /// badge id -> what it is worth and when it was earned
    badges: BTreeMap<u64, BadgeRec>,
    /// the first badge sync only records what the account already had (no popups)
    badges_synced: bool,
    /// milestone achievement -> the badge art it unlocked with
    icons: BTreeMap<u8, String>,
    /// popups not shown yet (kept across launches)
    pending: Vec<Pending>,
    /// badge scoring rules the saved badges were scored with
    score_version: u32,
}

/// Bump when badge_points changes: saved badges are then re-read (quietly) and re-scored.
const SCORE_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone)]
struct BadgeRec {
    name: String,
    points: u32,
    rate: f32,
    /// day (since 1970) it was earned
    day: u64,
    icon: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
enum Pending {
    Achievement(u8),
    Badge(u64),
}

/// The popup on screen: when it opened, whether the rare closing sound played.
struct Showing {
    item: Pending,
    rare: bool,
    t0: f32,
    ended: bool,
}

pub struct Achievements {
    user: u64,
    data: Progress,
    showing: Option<Showing>,
    /// debug popups (never saved)
    preview: Vec<(Pending, bool)>,
}

impl Achievements {
    pub fn new() -> Self {
        Achievements { user: 0, data: Progress::default(), showing: None, preview: Vec::new() }
    }

    /// Art for an achievement: the Xbox art, or for a badge milestone the badge that unlocked it.
    pub fn icon(&self, id: u8) -> String {
        if id >= 30 {
            return self.data.icons.get(&id).cloned().unwrap_or_else(|| "rbxasset://textures/ui/Shell/Icons/RobloxIcon32.png".into());
        }
        icon(id)
    }

    /// Badges owned, earned today, earned yesterday, and the gamerscore they are worth.
    pub fn badge_stats(&self) -> (usize, usize, usize, u32) {
        let d = today();
        let on = |day: u64| self.data.badges.values().filter(|b| b.day == day).count();
        (self.data.badges.len(), on(d), on(d.wrapping_sub(1)), self.data.badges.values().map(|b| b.points).sum())
    }

    pub fn known_badges(&self) -> std::collections::HashSet<u64> {
        self.data.badges.keys().copied().collect()
    }

    /// New badges from the badges API: each is scored, newly earned ones pop up (not on the
    /// account's first sync), then the badge milestones are checked.
    pub fn add_badges(&mut self, list: Vec<BadgeInfo>) {
        if self.user == 0 {
            return;
        }
        let first = !self.data.badges_synced;
        for b in list {
            if self.data.badges.contains_key(&b.id) {
                continue;
            }
            let rec = BadgeRec { name: b.name, points: badge_points(b.rate, b.won_yesterday, b.won_ever), rate: if b.won_ever >= TRUSTED_WINS { b.rate } else { 0.0 }, day: b.awarded / 86400, icon: b.icon };
            self.data.badges.insert(b.id, rec);
            if !first {
                self.queue(Pending::Badge(b.id));
            }
        }
        self.data.badges_synced = true;
        self.check_badge_milestones();
        self.save();
    }

    fn check_badge_milestones(&mut self) {
        let newest = |f: &dyn Fn(&BadgeRec) -> bool| self.data.badges.values().filter(|b| f(b)).max_by_key(|b| b.day).map(|b| b.icon.clone());
        let mut earned: Vec<(u8, Option<String>)> = Vec::new();
        let n = self.data.badges.len();
        for (need, id) in BADGE_COUNTS {
            if n >= need {
                earned.push((id, newest(&|_| true)));
            }
        }
        if let Some(i) = newest(&|b| b.rate > 0.0 && b.rate < RARE_RATE) {
            earned.push((35, Some(i)));
        }
        if let Some(i) = newest(&|b| b.rate > 0.0 && b.rate < ULTRA_RARE_RATE) {
            earned.push((36, Some(i)));
        }
        let days: BTreeSet<u64> = self.data.badges.values().map(|b| b.day).collect();
        let per_day = |d: u64| self.data.badges.values().filter(|b| b.day == d).count();
        if let Some(d) = days.iter().copied().find(|d| per_day(*d) >= 5) {
            earned.push((37, newest(&|b| b.day == d)));
        }
        let mut best = 0;
        let mut run = 0;
        let mut prev = None;
        for &d in &days {
            run = if prev == Some(d.wrapping_sub(1)) { run + 1 } else { 1 };
            best = best.max(run);
            prev = Some(d);
        }
        if best >= 2 {
            earned.push((38, newest(&|_| true)));
        }
        if best >= 7 {
            earned.push((39, newest(&|_| true)));
        }
        for (id, art) in earned {
            if !self.data.unlocked.contains_key(&id) {
                if let Some(a) = art {
                    self.data.icons.insert(id, a);
                }
                self.award(id);
            }
        }
    }

    /// Add a popup once (the same achievement or badge never queues twice).
    fn queue(&mut self, p: Pending) {
        if !self.data.pending.contains(&p) && self.showing.as_ref().map(|s| s.item) != Some(p) {
            self.data.pending.push(p);
        }
    }

    fn path(user: u64) -> std::path::PathBuf {
        let d = crate::config::data_dir().join("achievements");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{user}.json"))
    }

    /// Switch to a Roblox account's achievements (AuthenticationSuccess) and run the day-roll check.
    pub fn sign_in(&mut self, user: u64) {
        self.user = user;
        self.data = std::fs::read(Self::path(user)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if self.data.score_version != SCORE_VERSION {
            self.data.badges.clear();
            self.data.badges_synced = false;
            self.data.pending.retain(|p| matches!(p, Pending::Achievement(_)));
            self.data.score_version = SCORE_VERSION;
        }
        self.played_today();
    }

    pub fn sign_out(&mut self) {
        self.user = 0;
        self.data = Progress::default();
        self.showing = None;
    }

    fn save(&self) {
        if self.user == 0 {
            return;
        }
        if let Ok(b) = serde_json::to_vec_pretty(&self.data) {
            let _ = std::fs::write(Self::path(self.user), b);
        }
    }

    pub fn unlocked(&self, id: u8) -> Option<u64> {
        self.data.unlocked.get(&id).copied()
    }

    /// Achievements unlocked plus every badge's own points.
    pub fn gamerscore(&self) -> u32 {
        DEFS.iter().filter(|d| self.data.unlocked.contains_key(&d.id)).map(|d| d.gamerscore).sum::<u32>() + self.data.badges.values().map(|b| b.points).sum::<u32>()
    }

    /// SendAchievementEventAsync: unlock once per account, then show the toast.
    pub fn award(&mut self, id: u8) {
        if self.user == 0 || self.data.unlocked.contains_key(&id) {
            return;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.data.unlocked.insert(id, now);
        self.queue(Pending::Achievement(id));
        self.save();
    }

    /// Debug (--demo achievements): pretend a few are unlocked, without saving.
    pub fn demo(&mut self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        for id in [SCOUT, EXPLORER, POLLSTER, ONE_NAME_MANY_FACES, 8] {
            self.data.unlocked.insert(id, now);
        }
    }

    /// Debug: show the toast without unlocking anything.
    pub fn simulate(&mut self, id: u8, rare: bool) {
        self.preview.push((Pending::Achievement(id), rare));
    }

    fn played_today(&mut self) {
        let d = today();
        if !self.data.days.contains(&d) {
            self.data.days.push(d);
            self.data.days.sort_unstable();
            self.save();
        }
        // consecutive days ending today
        let mut streak = 0;
        let mut day = d;
        while self.data.days.contains(&day) {
            streak += 1;
            day = day.wrapping_sub(1);
        }
        for (need, id) in [(3, 8), (10, 9), (20, 10)] {
            if streak >= need {
                self.award(id);
            }
        }
    }

    /// OnPlayedGamesChanged: Scout, Explorer, Trailblazer.
    pub fn played_game(&mut self, place: u64) {
        if self.user == 0 {
            return;
        }
        if place != 0 && !self.data.places.contains(&place) {
            self.data.places.push(place);
            self.save();
        }
        self.played_today();
        let n = self.data.places.len();
        self.award(SCOUT);
        if n >= GAMES_FOR_AWARD_SAMPLER {
            self.award(EXPLORER);
        }
        if n >= GAMES_FOR_WORLD_TRAVELER {
            self.award(TRAILBLAZER);
        }
    }

    /// VotedOnPlace: Pollster after five different games.
    pub fn rated_game(&mut self, universe: u64) {
        if self.user == 0 {
            return;
        }
        if !self.data.rated.contains(&universe) {
            self.data.rated.push(universe);
            self.save();
        }
        if self.data.rated.len() >= GAMES_RATED_FOR_POLLSTER {
            self.award(POLLSTER);
        }
    }

    /// PlayerSessionEnd: Marathon after an hour in one game.
    pub fn session_ended(&mut self, seconds: f32) {
        if seconds >= PLAY_SECONDS_FOR_DEEP_DIVER {
            self.award(MARATHON);
        }
    }

    /// Draw the popup (bottom centre) for the next achievement or badge, one at a time, and play
    /// its sound when it appears. A popup leaves the saved queue once it has been shown.
    pub fn draw(&mut self, ui: &mut Ui, audio: &mut Audio) {
        let now = ui.time;
        if self.showing.is_none() {
            let next = if !self.preview.is_empty() {
                Some(self.preview.remove(0))
            } else if !self.data.pending.is_empty() {
                let p = self.data.pending.remove(0);
                self.save();
                let rare = match p {
                    Pending::Badge(b) => self.data.badges.get(&b).is_some_and(|b| b.rate > 0.0 && b.rate < RARE_RATE),
                    Pending::Achievement(id) => matches!(id, 35 | 36),
                };
                Some((p, rare))
            } else {
                None
            };
            let Some((item, rare)) = next else { return };
            audio.play(if rare { "RareAchievement" } else { "AchievementUnlocked" }, 1.0);
            self.showing = Some(Showing { item, rare, t0: now, ended: false });
        }
        let Some(t) = self.showing.as_mut() else { return };
        let (item, rare) = (t.item, t.rare);
        let e = now - t.t0;
        const OPEN: f32 = 0.45;
        const HOLD: f32 = 5.5;
        if rare && !t.ended && e > OPEN + HOLD - 0.2 {
            t.ended = true;
            audio.play("RareAchievementEnd", 1.0);
        }
        if e > OPEN * 2.0 + HOLD {
            self.showing = None;
            return;
        }
        // what the card shows: art, heading, name, gamerscore
        let (art, line1, name, points) = match item {
            Pending::Achievement(id) => {
                let d = def(id);
                let art = if id >= 30 { self.icon(id) } else { icon_round(id) };
                (art, if rare { "Rare Achievement Unlocked" } else { "Achievement Unlocked" }, d.name.to_string(), d.gamerscore)
            }
            Pending::Badge(b) => match self.data.badges.get(&b) {
                Some(r) => (r.icon.clone(), if rare { "Rare Badge Earned" } else { "Badge Earned" }, r.name.clone(), r.points),
                None => {
                    self.showing = None;
                    return;
                }
            },
        };

        let slide = ease_in_out_quad((e / 0.35).min(1.0)) * ease_in_out_quad(((OPEN * 2.0 + HOLD - e) / 0.35).min(1.0));
        let (w, h) = (760.0, 132.0);
        let r = Rect::new(960.0 - w / 2.0, 1080.0 - (60.0 + h) * slide, w, h);
        // slides up into place and back down (no fade)
        ui.nine_slice("rbxasset://textures/ui/Shell/Buttons/Generic9ScaleShadow.png", Rect::new(r.x, r.y, r.w + 3.0, r.h + 3.0), [10.0, 10.0, 28.0, 28.0], [1.0; 3], 1.0);
        ui.nine_slice("rbxasset://textures/ui/Shell/Buttons/Generic9ScaleButton@720.png", r, [8.0, 8.0, 24.0, 24.0], [26.0 / 255.0, 57.0 / 255.0, 76.0 / 255.0], 0.97);
        let accent = if rare { [212.0 / 255.0, 175.0 / 255.0, 55.0 / 255.0] } else { [50.0 / 255.0, 181.0 / 255.0, 1.0] };
        ui.rect(Rect::new(r.x + 8.0, r.y, r.w - 16.0, 4.0), accent, 1.0);
        // art: the rounded achievement icon in a selection outline
        let c = Rect::new(r.x + 26.0, r.y + 28.0, h - 56.0, h - 56.0);
        ui.image(&art, c, [1.0; 3], 1.0);
        // text, with the Roblox "R" before the heading
        let tx = c.x + c.w + 28.0;
        ui.image("rbxasset://textures/ui/Shell/Icons/RobloxIcon32.png", Rect::new(tx, r.y + 24.0, 26.0, 26.0), [1.0; 3], 1.0);
        ui.label(line1, Font::Regular, 26.0, Rect::new(tx + 36.0, r.y + 22.0, 420.0, 30.0), Align::Start, Align::Center, [0.78, 0.84, 0.9], 1.0);
        let name_w = r.x + r.w - 150.0 - tx;
        let name = ui.fit(&name, Font::Light, 40.0, name_w);
        ui.label(&name, Font::Light, 40.0, Rect::new(tx, r.y + 58.0, name_w, 48.0), Align::Start, Align::Center, [1.0; 3], 1.0);
        // gamerscore: G in a disc + the points
        let gs = points.to_string();
        let gw = ui.measure(&gs, Font::Regular, 40.0);
        let gx = r.x + r.w - 28.0 - gw;
        ui.label(&gs, Font::Regular, 40.0, Rect::new(gx, r.y + 42.0, gw, 48.0), Align::Start, Align::Center, [1.0; 3], 1.0);
        let g = Rect::new(gx - 44.0, r.y + 49.0, 34.0, 34.0);
        ui.image("rbxasset://glyphs/Hub/Disc.png", g, [1.0; 3], 1.0);
        ui.label("G", Font::Bold, 24.0, Rect::new(g.x, g.y - 1.0, g.w, g.h), Align::Center, Align::Center, [19.0 / 255.0; 3], 1.0);
        ui.selection_box(r, 6.0);
        ui.alpha = 1.0;
    }
}

// >> end << //