/*
    written by: @corescripts
*/

//! Port of Shell/SoundManager: UI sound effects and the background music loop.

// >> imports
use crate::assets::Content;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SOUNDS: &[(&str, &str)] = &[
    ("BackgroundLoop", "RobloxMusic.ogg"),
    ("Error", "Error.mp3"),
    ("ButtonPress", "ButtonPress.mp3"),
    ("MoveSelection", "MoveSelection.mp3"),
    ("OverlayOpen", "OverlayOpen.mp3"),
    ("PopUp", "PopUp.mp3"),
    ("PurchaseSuccess", "PurchaseSuccess.mp3"),
    ("ScreenChange", "ScreenChange.mp3"),
    ("SideMenuSlideIn", "SideMenuSlideIn.mp3"),
];

/// Xbox dashboard sounds (launcher/assets/sounds): keyboard keys and achievement unlocks.
const XBOX_SOUNDS: &[(&str, &str)] = &[
    ("KeyboardMove", "KeyboardMove.mp3"),
    ("KeyboardPress", "KeyboardPress.mp3"),
    ("AchievementUnlocked", "AchievementUnlocked.mp3"),
    ("RareAchievement", "RareAchievement.mp3"),
    ("RareAchievementEnd", "RareAchievementEnd.mp3"),
];

pub struct Audio {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    data: HashMap<&'static str, Arc<[u8]>>,
    music: Option<Sink>,
    pub music_volume: f32,
    duck: f32,
    duck_target: f32,
    last_update: Instant,
    pub sfx_volume: f32,
    last_move: Instant,
    /// Configure > Audio Fading: the theme gets quieter once it has looped
    pub loop_fading: bool,
    loop_fade: f32,
    loops: u32,
}

impl Audio {
    pub fn new(content: &Content) -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                eprintln!("audio unavailable: {e}");
                (None, None)
            }
        };
        let mut data = HashMap::new();
        for (name, file) in XBOX_SOUNDS {
            if let Ok(b) = content.read_local(&format!("rbxasset://sounds/{file}")) {
                data.insert(*name, Arc::<[u8]>::from(b));
            }
        }
        for (name, file) in SOUNDS {
            if let Ok(b) = content.read_local(&format!("rbxasset://sounds/ui/Shell/{file}")) {
                data.insert(*name, Arc::<[u8]>::from(b));
            }
        }
        Self {
            _stream: stream,
            handle,
            data,
            music: None,
            music_volume: 1.0,
            duck: 1.0,
            duck_target: 1.0,
            last_update: Instant::now(),
            sfx_volume: 1.0,
            last_move: Instant::now(),
            loop_fading: true,
            loop_fade: 1.0,
            loops: 0,
        }
    }

    /// Silent sink for screens drawn as previews.
    pub fn muted() -> Self {
        Self {
            _stream: None,
            handle: None,
            data: HashMap::new(),
            music: None,
            music_volume: 0.0,
            duck: 1.0,
            duck_target: 1.0,
            last_update: Instant::now(),
            sfx_volume: 0.0,
            last_move: Instant::now(),
            loop_fading: false,
            loop_fade: 1.0,
            loops: 0,
        }
    }

    pub fn play(&mut self, name: &str, vol: f32) {
        // selection moves can fire several times a frame when scrolling quickly
        if name == "MoveSelection" {
            if self.last_move.elapsed() < Duration::from_millis(40) {
                return;
            }
            self.last_move = Instant::now();
        }
        let (Some(h), Some(d)) = (self.handle.as_ref(), self.data.get(name)) else { return };
        if let Ok(src) = Decoder::new(Cursor::new(d.clone())) {
            let _ = h.play_raw(src.amplify(vol * self.sfx_volume).convert_samples());
        }
    }

    fn queue_music(&self) {
        let (Some(sink), Some(d)) = (self.music.as_ref(), self.data.get("BackgroundLoop")) else { return };
        match Decoder::new(Cursor::new(d.clone())) {
            Ok(src) => sink.append(src),
            Err(e) => eprintln!("RobloxMusic.ogg: {e}"),
        }
    }

    /// Background music: loops RobloxMusic.ogg forever. The next play-through is queued
    /// while the current one is still playing so there is no gap at the loop point.
    pub fn start_music(&mut self) {
        let Some(h) = self.handle.as_ref() else { return };
        let Ok(sink) = Sink::try_new(h) else { return };
        sink.set_volume(0.33 * self.music_volume);
        self.music = Some(sink);
        self.queue_music();
        self.queue_music();
    }

    /// Fade the music out (true) or back in (false) over about a second.
    pub fn duck(&mut self, on: bool) {
        self.duck_target = if on { 0.0 } else { 1.0 };
    }

    pub fn update(&mut self) {
        let dt = self.last_update.elapsed().as_secs_f32().min(0.1);
        self.last_update = Instant::now();
        let step = dt / 1.0;
        if self.duck < self.duck_target {
            self.duck = (self.duck + step).min(self.duck_target);
        } else if self.duck > self.duck_target {
            self.duck = (self.duck - step).max(self.duck_target);
        }
        // after the first play-through the theme settles to a quieter level over a few seconds
        let fade_target = if self.loop_fading && self.loops > 0 { 0.45 } else { 1.0 };
        self.loop_fade += (fade_target - self.loop_fade) * (dt / 4.0).min(1.0);
        let Some(sink) = &self.music else { return };
        sink.set_volume(0.33 * self.music_volume * self.duck * self.loop_fade);
        if sink.len() < 2 {
            self.loops += 1;
            self.queue_music();
        }
    }
}

// >> end << //
