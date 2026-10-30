/*
    written by: @corescripts
*/

//! Maps keyboard, mouse and gamepads onto the console button set the shell expects.

// >> imports
use crate::config::IconSet;
use gilrs::{Axis, Button, EventType, Gilrs};
use std::sync::atomic::{AtomicU8, Ordering};
use glam::Vec2;
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

/// Chosen glyph set and the device the user last pressed something on (0 Xbox pad, 1 keyboard, 2 PlayStation pad).
static ICON_SET: AtomicU8 = AtomicU8::new(0);
static LAST_DEVICE: AtomicU8 = AtomicU8::new(0);

pub fn set_icon_set(set: IconSet) {
    ICON_SET.store(set as u8, Ordering::Relaxed);
}

/// Texture for a face button ('A', 'B', 'X', 'Y') in the active glyph set.
pub fn glyph(b: char) -> String {
    let device = match ICON_SET.load(Ordering::Relaxed) {
        1 => 0,
        2 => 1,
        3 => 2,
        _ => LAST_DEVICE.load(Ordering::Relaxed),
    };
    match device {
        1 => format!("rbxasset://glyphs/Keyboard/{b}Button.png"),
        2 => format!("rbxasset://glyphs/PlayStation/{b}Button.png"),
        _ => format!("rbxasset://textures/ui/Shell/ButtonIcons/{b}Button.png"),
    }
}

/// Spoken name of a face button in the active glyph set (keyboard keys share the letter).
pub fn button_name(b: char) -> String {
    let device = match ICON_SET.load(Ordering::Relaxed) {
        1 => 0,
        2 => 1,
        3 => 2,
        _ => LAST_DEVICE.load(Ordering::Relaxed),
    };
    match (device, b) {
        (1, _) => format!("the {b} key"),
        (2, 'A') => "Cross".into(),
        (2, 'B') => "Circle".into(),
        (2, 'X') => "Square".into(),
        (2, 'Y') => "Triangle".into(),
        _ => format!("{b}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Btn {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    LB,
    RB,
    Start,
    /// View / Back button (keyboard V)
    View,
    /// right stick click (keyboard R)
    RS,
}

#[derive(Default)]
pub struct Input {
    pub pressed: Vec<Btn>,
    pub released: Vec<Btn>,
    pub typed: String,
    pub backspace: bool,
    /// physical Enter in a text field
    pub enter: bool,
    pub paste: bool,
    pub mouse: Vec2,
    pub mouse_moved: bool,
    pub clicked: bool,
    /// left button held (dragging)
    pub mouse_down: bool,
    pub wheel: f32,
    /// Right stick / mouse parallax for the background camera, -1..1
    pub look: Vec2,
    /// right stick alone (overscan adjustment), -1..1 with dead zone
    pub right_stick: Vec2,
    pub any_activity: bool,
    pub toggle_filter: bool,
    /// F2: hide every UI layer to look at the background
    pub toggle_hide_ui: bool,
    pub toggle_fullscreen: bool,
    gilrs: Option<Gilrs>,
    held_dir: Option<(Btn, f32)>,
    stick_dir: Option<Btn>,
    modifiers: ModifiersState,
    stick: Vec2,
    pad_look: Vec2,
    /// set while feeding gamepad events so press() keeps the pad as the last device
    pad_event: bool,
}

impl Input {
    pub fn new() -> Self {
        Self { gilrs: Gilrs::new().ok(), ..Default::default() }
    }

    pub fn begin_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.typed.clear();
        self.backspace = false;
        self.enter = false;
        self.paste = false;
        self.clicked = false;
        self.mouse_moved = false;
        self.wheel = 0.0;
        self.any_activity = false;
        self.toggle_filter = false;
        self.toggle_hide_ui = false;
        self.toggle_fullscreen = false;
    }

    pub fn has(&self, b: Btn) -> bool {
        self.pressed.contains(&b)
    }

    fn press(&mut self, b: Btn) {
        if !self.pad_event {
            LAST_DEVICE.store(1, Ordering::Relaxed);
        }
        self.pressed.push(b);
        self.any_activity = true;
    }

    pub fn modifiers(&mut self, m: ModifiersState) {
        self.modifiers = m;
    }

    pub fn key(&mut self, key: PhysicalKey, state: ElementState, repeat: bool, text: Option<&str>, text_mode: bool) {
        let PhysicalKey::Code(code) = key else { return };
        if state == ElementState::Released {
            if let Some(b) = map_key(code, text_mode) {
                self.released.push(b);
            }
            return;
        }
        if self.modifiers.control_key() && code == KeyCode::KeyV {
            self.paste = true;
            self.any_activity = true;
            return;
        }
        if code == KeyCode::F1 && !repeat {
            self.toggle_filter = true;
            return;
        }
        if code == KeyCode::F2 && !repeat {
            self.toggle_hide_ui = true;
            return;
        }
        if code == KeyCode::F11 && !repeat {
            self.toggle_fullscreen = true;
            return;
        }
        if text_mode {
            if code == KeyCode::Backspace {
                self.backspace = true;
                return;
            }
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                self.enter = true;
                self.any_activity = true;
                return;
            }
            if let Some(t) = text {
                if !self.modifiers.control_key() && t.chars().all(|c| !c.is_control()) {
                    self.typed.push_str(t);
                    return;
                }
            }
        }
        if let Some(b) = map_key(code, text_mode) {
            if repeat && !matches!(b, Btn::Up | Btn::Down | Btn::Left | Btn::Right) {
                return;
            }
            self.press(b);
        }
    }

    pub fn mouse_move(&mut self, p: Vec2, window: Vec2) {
        self.mouse = p;
        self.mouse_moved = true;
        let n = (p / window) * 2.0 - 1.0;
        self.look = Vec2::new(n.x, -n.y) * 0.35 + self.pad_look;
    }

    pub fn mouse_button(&mut self, b: MouseButton, s: ElementState) {
        if b == MouseButton::Left {
            self.mouse_down = s == ElementState::Pressed;
        }
        if s != ElementState::Pressed {
            return;
        }
        match b {
            MouseButton::Left => {
                self.clicked = true;
                self.any_activity = true;
            }
            MouseButton::Right | MouseButton::Back => self.press(Btn::B),
            _ => {}
        }
    }

    pub fn poll_gamepad(&mut self, dt: f32) {
        let mut events = Vec::new();
        if let Some(g) = self.gilrs.as_mut() {
            while let Some(e) = g.next_event() {
                let ps = {
                    let n = g.gamepad(e.id).name().to_ascii_lowercase();
                    n.contains("playstation") || n.contains("dualshock") || n.contains("dualsense") || n.contains("sony") || n.contains("ps4") || n.contains("ps5")
                };
                if matches!(e.event, EventType::ButtonPressed(..)) {
                    LAST_DEVICE.store(if ps { 2 } else { 0 }, Ordering::Relaxed);
                }
                events.push(e.event);
            }
        }
        self.pad_event = true;
        for ev in events {
            match ev {
                EventType::ButtonPressed(b, _) => {
                    if let Some(m) = map_pad(b) {
                        self.press(m);
                        if matches!(m, Btn::Up | Btn::Down | Btn::Left | Btn::Right) {
                            self.held_dir = Some((m, 0.45));
                        }
                    }
                }
                EventType::ButtonReleased(b, _) => {
                    if let Some(m) = map_pad(b) {
                        self.released.push(m);
                        if self.held_dir.map(|h| h.0) == Some(m) {
                            self.held_dir = None;
                        }
                    }
                }
                EventType::AxisChanged(Axis::LeftStickX, v, _) => self.stick.x = v,
                EventType::AxisChanged(Axis::LeftStickY, v, _) => self.stick.y = v,
                EventType::AxisChanged(Axis::RightStickX, v, _) => self.pad_look.x = dz(v),
                EventType::AxisChanged(Axis::RightStickY, v, _) => self.pad_look.y = dz(v),
                _ => {}
            }
        }
        self.right_stick = self.pad_look;
        if self.pad_look != Vec2::ZERO {
            self.look = self.pad_look;
        }
        // left stick as d-pad with repeat
        let dir = if self.stick.length() < 0.5 {
            None
        } else if self.stick.x.abs() > self.stick.y.abs() {
            Some(if self.stick.x > 0.0 { Btn::Right } else { Btn::Left })
        } else {
            Some(if self.stick.y > 0.0 { Btn::Up } else { Btn::Down })
        };
        if dir != self.stick_dir {
            self.stick_dir = dir;
            if let Some(d) = dir {
                self.press(d);
                self.held_dir = Some((d, 0.45));
            } else {
                self.held_dir = None;
            }
        }
        if let Some((b, t)) = self.held_dir.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                *t = 0.12;
                let b = *b;
                self.press(b);
            }
        }
        self.pad_event = false;
    }
}

fn dz(v: f32) -> f32 {
    if v.abs() < 0.15 {
        0.0
    } else {
        v
    }
}

fn map_key(k: KeyCode, text_mode: bool) -> Option<Btn> {
    Some(match k {
        KeyCode::ArrowUp => Btn::Up,
        KeyCode::ArrowDown => Btn::Down,
        KeyCode::ArrowLeft => Btn::Left,
        KeyCode::ArrowRight => Btn::Right,
        KeyCode::Enter | KeyCode::NumpadEnter => Btn::A,
        KeyCode::Escape => Btn::B,
        _ if text_mode => return None,
        KeyCode::KeyW => Btn::Up,
        KeyCode::KeyS => Btn::Down,
        KeyCode::KeyA => Btn::Left,
        KeyCode::KeyD => Btn::Right,
        KeyCode::Space => Btn::A,
        KeyCode::Backspace => Btn::B,
        KeyCode::KeyX => Btn::X,
        KeyCode::KeyY => Btn::Y,
        KeyCode::KeyQ | KeyCode::PageUp => Btn::LB,
        KeyCode::KeyE | KeyCode::PageDown | KeyCode::Tab => Btn::RB,
        KeyCode::KeyM => Btn::Start,
        KeyCode::KeyV => Btn::View,
        KeyCode::KeyR => Btn::RS,
        _ => return None,
    })
}

fn map_pad(b: Button) -> Option<Btn> {
    Some(match b {
        Button::DPadUp => Btn::Up,
        Button::DPadDown => Btn::Down,
        Button::DPadLeft => Btn::Left,
        Button::DPadRight => Btn::Right,
        Button::South => Btn::A,
        Button::East => Btn::B,
        Button::West => Btn::X,
        Button::North => Btn::Y,
        Button::LeftTrigger => Btn::LB,
        Button::RightTrigger => Btn::RB,
        Button::Start => Btn::Start,
        Button::Select => Btn::View,
        Button::RightThumb => Btn::RS,
        _ => return None,
    })
}

// >> end << //
