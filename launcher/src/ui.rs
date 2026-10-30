/*
    written by: @corescripts
*/

//! Immediate-mode UI over a 1920x1080 virtual canvas (the Shell mockup size), letterboxed
//! to 16:9 like AppContainer.AspectRatioProtector. Handles text, images, 9-slice frames,
//! clipping and GuiService-style spatial selection.

// >> imports
use crate::input::{Btn, Input};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::collections::HashMap;

pub const W: f32 = 1920.0;
pub const H: f32 = 1080.0;

pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * (2.0 - t)
}

/// Utility.SCurveUDim2 style smooth step.
pub fn s_curve(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular = 0,
    Light = 1,
    Bold = 2,
    Italic = 3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub struct Batch {
    pub texture: usize,
    pub start: u32,
    pub count: u32,
    pub clip: Option<[f32; 4]>,
}

#[derive(Default)]
pub struct UiFrame {
    pub vertices: Vec<UiVertex>,
    pub batches: Vec<Batch>,
}

#[derive(Clone, Copy)]
struct Glyph {
    uv: [f32; 4],
    w: f32,
    h: f32,
    xmin: f32,
    ymin: f32,
    advance: f32,
}

pub struct Fonts {
    fonts: Vec<fontdue::Font>,
    glyphs: HashMap<(u8, u16, char), Glyph>,
    pub atlas: Vec<u8>,
    pub size: u32,
    cursor: (u32, u32, u32),
    pub dirty: Option<(u32, u32)>,
    pub texture: usize,
}

impl Fonts {
    pub fn new(content: &crate::assets::Content) -> Self {
        let files = ["SourceSansPro-Regular.ttf", "SourceSansPro-Light.ttf", "SourceSansPro-Bold.ttf", "SourceSansPro-It.ttf"];
        let fonts = files
            .iter()
            .map(|f| {
                let bytes = content.read_local(&format!("rbxasset://fonts/{f}")).expect("missing SourceSans fonts in genericcontent/fonts");
                fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("bad font")
            })
            .collect();
        let size = 2048;
        Self { fonts, glyphs: HashMap::new(), atlas: vec![0; (size * size * 4) as usize], size, cursor: (1, 1, 0), dirty: None, texture: 1 }
    }

    fn glyph(&mut self, font: Font, px: u16, ch: char) -> Glyph {
        let key = (font as u8, px, ch);
        if let Some(g) = self.glyphs.get(&key) {
            return *g;
        }
        let (m, bitmap) = self.fonts[font as usize].rasterize(ch, px as f32);
        let (w, h) = (m.width as u32, m.height as u32);
        if self.cursor.0 + w + 1 >= self.size {
            self.cursor = (1, self.cursor.1 + self.cursor.2 + 1, 0);
        }
        if self.cursor.1 + h + 1 >= self.size {
            // atlas full: start over (glyphs will re-rasterize as needed)
            self.glyphs.clear();
            self.atlas.iter_mut().for_each(|b| *b = 0);
            self.cursor = (1, 1, 0);
            self.dirty = Some((0, self.size));
        }
        let (x0, y0) = (self.cursor.0, self.cursor.1);
        for y in 0..h {
            for x in 0..w {
                let a = bitmap[(y * w + x) as usize];
                let i = (((y0 + y) * self.size + x0 + x) * 4) as usize;
                self.atlas[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
            }
        }
        self.cursor.0 += w + 1;
        self.cursor.2 = self.cursor.2.max(h);
        self.dirty = Some(match self.dirty {
            Some((a, b)) => (a.min(y0), b.max(y0 + h)),
            None => (y0, y0 + h),
        });
        let s = self.size as f32;
        let g = Glyph {
            uv: [x0 as f32 / s, y0 as f32 / s, (x0 + w) as f32 / s, (y0 + h) as f32 / s],
            w: w as f32,
            h: h as f32,
            xmin: m.xmin as f32,
            ymin: m.ymin as f32,
            advance: m.advance_width,
        };
        self.glyphs.insert(key, g);
        g
    }

    fn ascent(&self, font: Font, px: f32) -> f32 {
        self.fonts[font as usize].horizontal_line_metrics(px).map(|m| m.ascent).unwrap_or(px * 0.8)
    }
    fn descent(&self, font: Font, px: f32) -> f32 {
        self.fonts[font as usize].horizontal_line_metrics(px).map(|m| m.descent).unwrap_or(-px * 0.2)
    }

    /// Width in virtual units of a single line.
    pub fn measure(&self, s: &str, font: Font, size: f32) -> f32 {
        let f = &self.fonts[font as usize];
        let mut w = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                w += f.horizontal_kern(p, ch, size).unwrap_or(0.0);
            }
            w += f.metrics(ch, size).advance_width;
            prev = Some(ch);
        }
        w
    }
}

pub enum TexSlot {
    Loading,
    Ready { idx: usize, w: u32, h: u32 },
    Failed,
}

pub struct Ui {
    pub frame: UiFrame,
    pub scale: f32,
    /// horizontal stretch on top of `scale` (Configure > Screen: Stretch)
    pub sx: f32,
    /// 0 fit 16:9, 1 stretch to the window, 2 widescreen (scale by height)
    pub mode: u8,
    pub offset: Vec2,
    pub screen: Vec2,
    pub fonts: Fonts,
    pub tex: HashMap<String, TexSlot>,
    pub tex_requests: Vec<String>,
    clip_stack: Vec<[f32; 4]>,
    translate: Vec2,
    pub alpha: f32,
    // selection
    pub focus: Option<u64>,
    /// left / right are taken by the focused control (sliders); navigation ignores them
    pub lock_x: bool,
    items: Vec<(u64, Rect)>,
    prev_items: Vec<(u64, Rect)>,
    pub activated: bool,
    /// X pressed this frame (HintActionView actions)
    pub activated_x: bool,
    pub activated_y: bool,
    pub moved: bool,
    pub mouse: Option<Vec2>,
    mouse_moved: bool,
    /// the mouse was clicked this frame
    pub clicked: bool,
    pub time: f32,
    pub nav_blocked: bool,
    pub overscan: Vec2,
    /// draw-only mode for screens shown underneath another (no selectables registered)
    pub inert: bool,
    /// when the current selection started, and the previous selection + when it ended
    pub focus_since: f32,
    pub prev_focus: Option<u64>,
    pub prev_since: f32,
    seen_focus: Option<u64>,
    last_focus_rect: HashMap<u64, Rect>,
}

impl Ui {
    pub fn new(fonts: Fonts) -> Self {
        Self {
            frame: UiFrame::default(),
            scale: 1.0,
            sx: 1.0,
            mode: 0,
            offset: Vec2::ZERO,
            screen: Vec2::new(W, H),
            fonts,
            tex: HashMap::new(),
            tex_requests: Vec::new(),
            clip_stack: Vec::new(),
            translate: Vec2::ZERO,
            alpha: 1.0,
            focus: None,
            lock_x: false,
            items: Vec::new(),
            prev_items: Vec::new(),
            activated: false,
            activated_x: false,
            activated_y: false,
            moved: false,
            mouse: None,
            mouse_moved: false,
            clicked: false,
            time: 0.0,
            nav_blocked: false,
            overscan: Vec2::ONE,
            inert: false,
            focus_since: 0.0,
            prev_focus: None,
            prev_since: 0.0,
            seen_focus: None,
            last_focus_rect: HashMap::new(),
        }
    }

    pub fn begin(&mut self, screen: Vec2, input: &Input, time: f32) {
        self.screen = screen;
        // AspectRatioProtector: fit 16:9
        let fit = screen * self.overscan;
        let s = match self.mode {
            1 | 2 => fit.y / H,
            _ => (fit.x / W).min(fit.y / H),
        };
        self.scale = s;
        self.sx = if self.mode == 1 { (fit.x / W) / s } else { 1.0 };
        self.offset = (screen - Vec2::new(W * self.sx, H) * s) * 0.5;
        self.frame.vertices.clear();
        self.frame.batches.clear();
        self.clip_stack.clear();
        self.translate = Vec2::ZERO;
        self.alpha = 1.0;
        self.time = time;
        self.prev_items = std::mem::take(&mut self.items);
        if self.focus != self.seen_focus {
            self.prev_focus = self.seen_focus;
            self.prev_since = time;
            self.focus_since = time;
            self.seen_focus = self.focus;
        }
        self.activated = false;
        self.moved = false;
        self.nav_blocked = false;
        self.mouse = Some((input.mouse - self.offset) / Vec2::new(self.scale * self.sx, self.scale));
        self.mouse_moved = input.mouse_moved;
        self.clicked = input.clicked;

        if input.has(Btn::A) {
            self.activated = true;
        }
        self.activated_x = input.has(Btn::X);
        self.activated_y = input.has(Btn::Y);
    }

    /// Apply directional navigation using last frame's selectable layout.
    pub fn navigate(&mut self, input: &Input) {
        for b in &input.pressed {
            let dir = match b {
                Btn::Up => Vec2::new(0.0, -1.0),
                Btn::Down => Vec2::new(0.0, 1.0),
                Btn::Left | Btn::Right if self.lock_x => continue,
                Btn::Left => Vec2::new(-1.0, 0.0),
                Btn::Right => Vec2::new(1.0, 0.0),
                _ => continue,
            };
            let cur = self.focus.and_then(|f| self.prev_items.iter().find(|i| i.0 == f).map(|i| i.1));
            let Some(cur) = cur else {
                if let Some(first) = self.prev_items.first() {
                    self.focus = Some(first.0);
                    self.moved = true;
                }
                continue;
            };
            let mut best: Option<(f32, u64)> = None;
            for (id, r) in &self.prev_items {
                if Some(*id) == self.focus {
                    continue;
                }
                let (primary, secondary, overlap) = if dir.x != 0.0 {
                    let d = if dir.x > 0.0 { r.x - (cur.x + cur.w) } else { cur.x - (r.x + r.w) };
                    let cd = (r.center().x - cur.center().x) * dir.x;
                    let ov = r.y < cur.y + cur.h && r.y + r.h > cur.y;
                    (if cd > 1.0 { d.max(0.0) + cd * 0.01 } else { -1.0 }, (r.center().y - cur.center().y).abs(), ov)
                } else {
                    let d = if dir.y > 0.0 { r.y - (cur.y + cur.h) } else { cur.y - (r.y + r.h) };
                    let cd = (r.center().y - cur.center().y) * dir.y;
                    let ov = r.x < cur.x + cur.w && r.x + r.w > cur.x;
                    (if cd > 1.0 { d.max(0.0) + cd * 0.01 } else { -1.0 }, (r.center().x - cur.center().x).abs(), ov)
                };
                if primary < 0.0 {
                    continue;
                }
                let score = primary + if overlap { secondary * 0.05 } else { secondary * 2.0 + 200.0 };
                if best.map(|b| score < b.0).unwrap_or(true) {
                    best = Some((score, *id));
                }
            }
            if let Some((_, id)) = best {
                self.focus = Some(id);
                self.moved = true;
            }
        }
    }

    pub fn finish(&mut self) {
        if let Some(f) = self.focus {
            if !self.items.iter().any(|i| i.0 == f) && !self.prev_items.iter().any(|i| i.0 == f) {
                // focused item vanished
            }
        }
    }

    pub fn set_focus(&mut self, id: u64) {
        if self.focus != Some(id) {
            self.focus = Some(id);
        }
    }

    pub fn is_focused(&self, id: u64) -> bool {
        self.focus == Some(id)
    }

    /// Register a selectable element. Returns true while it is selected.
    pub fn selectable(&mut self, id: u64, r: Rect) -> bool {
        if self.inert {
            return false;
        }
        let r = r.offset(self.translate.x, self.translate.y);
        if let Some(c) = self.clip_stack.last() {
            // skip items scrolled fully outside the clip region
            let cr = Rect::new((c[0] - self.offset.x) / (self.scale * self.sx), (c[1] - self.offset.y) / self.scale, c[2] / (self.scale * self.sx), c[3] / self.scale);
            if r.x + r.w < cr.x - 4.0 || r.x > cr.x + cr.w + 4.0 || r.y + r.h < cr.y - 4.0 || r.y > cr.y + cr.h + 4.0 {
                // still navigable so scroll containers can bring it into view
            }
        }
        self.items.push((id, r));
        self.last_focus_rect.insert(id, r);
        if let Some(m) = self.mouse {
            if r.contains(m) {
                if self.mouse_moved && self.focus != Some(id) {
                    self.focus = Some(id);
                    self.moved = true;
                }
                if self.clicked {
                    self.focus = Some(id);
                    self.activated = true;
                }
            }
        }
        self.focus == Some(id)
    }

    /// Selected and A pressed (or clicked) this frame.
    pub fn pressed(&self, id: u64) -> bool {
        self.focus == Some(id) && self.activated
    }

    pub fn rect_of(&self, id: u64) -> Option<Rect> {
        self.last_focus_rect.get(&id).copied()
    }

    pub fn selectable_count(&self) -> usize {
        self.prev_items.len()
    }

    pub fn focus_valid(&self) -> bool {
        self.focus.map(|f| self.prev_items.iter().any(|i| i.0 == f)).unwrap_or(false)
    }

    // >> drawing

    pub fn push_translate(&mut self, d: Vec2) -> Vec2 {
        let old = self.translate;
        self.translate += d;
        old
    }
    pub fn pop_translate(&mut self, old: Vec2) {
        self.translate = old;
    }

    /// Draw a whole 1920x1080 canvas into `r` (the configuration preview of the menu).
    pub fn push_canvas(&mut self, r: Rect) -> (f32, Vec2, Vec2) {
        let saved = (self.scale, self.offset, self.translate);
        let r = r.offset(self.translate.x, self.translate.y);
        self.offset += Vec2::new(r.x * self.sx, r.y) * self.scale;
        self.scale *= r.w / W;
        self.translate = Vec2::ZERO;
        self.push_clip(Rect::new(0.0, 0.0, W, H));
        saved
    }
    pub fn pop_canvas(&mut self, saved: (f32, Vec2, Vec2)) {
        self.pop_clip();
        (self.scale, self.offset, self.translate) = saved;
    }

    pub fn push_clip(&mut self, r: Rect) {
        let r = r.offset(self.translate.x, self.translate.y);
        let p = self.offset + Vec2::new(r.x * self.sx, r.y) * self.scale;
        let mut c = [p.x, p.y, r.w * self.scale * self.sx, r.h * self.scale];
        if let Some(o) = self.clip_stack.last() {
            let x0 = c[0].max(o[0]);
            let y0 = c[1].max(o[1]);
            let x1 = (c[0] + c[2]).min(o[0] + o[2]);
            let y1 = (c[1] + c[3]).min(o[1] + o[3]);
            c = [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)];
        }
        self.clip_stack.push(c);
    }
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn batch(&mut self, texture: usize) {
        let clip = self.clip_stack.last().copied();
        let n = self.frame.vertices.len() as u32;
        if let Some(b) = self.frame.batches.last_mut() {
            if b.texture == texture && b.clip == clip {
                return;
            }
        }
        self.frame.batches.push(Batch { texture, start: n, count: 0, clip });
    }

    fn quad_px(&mut self, texture: usize, p: [Vec2; 4], uv: [f32; 4], color: [f32; 4]) {
        self.batch(texture);
        let uvs = [[uv[0], uv[1]], [uv[2], uv[1]], [uv[2], uv[3]], [uv[0], uv[3]]];
        for i in [0usize, 1, 2, 0, 2, 3] {
            self.frame.vertices.push(UiVertex { pos: p[i].into(), uv: uvs[i], color });
        }
        self.frame.batches.last_mut().unwrap().count += 6;
    }

    fn quad(&mut self, texture: usize, r: Rect, uv: [f32; 4], color: [f32; 4]) {
        let r = r.offset(self.translate.x, self.translate.y);
        let o = self.offset + Vec2::new(r.x * self.sx, r.y) * self.scale;
        let s = Vec2::new(r.w * self.sx, r.h) * self.scale;
        self.quad_px(texture, [o, o + Vec2::new(s.x, 0.0), o + s, o + Vec2::new(0.0, s.y)], uv, color);
    }

    pub fn rect(&mut self, r: Rect, c: [f32; 3], alpha: f32) {
        self.quad(0, r, [0.0, 0.0, 1.0, 1.0], [c[0], c[1], c[2], alpha * self.alpha]);
    }

    fn texture(&mut self, key: &str) -> Option<(usize, u32, u32)> {
        match self.tex.get(key) {
            Some(TexSlot::Ready { idx, w, h }) => Some((*idx, *w, *h)),
            Some(_) => None,
            None => {
                self.tex.insert(key.to_string(), TexSlot::Loading);
                self.tex_requests.push(key.to_string());
                None
            }
        }
    }

    pub fn image_loaded(&mut self, key: &str) -> bool {
        self.texture(key).is_some()
    }

    pub fn image_failed(&self, key: &str) -> bool {
        matches!(self.tex.get(key), Some(TexSlot::Failed))
    }

    pub fn image(&mut self, key: &str, r: Rect, tint: [f32; 3], alpha: f32) -> bool {
        let Some((idx, _, _)) = self.texture(key) else { return false };
        self.quad(idx, r, [0.0, 0.0, 1.0, 1.0], [tint[0], tint[1], tint[2], alpha * self.alpha]);
        true
    }

    /// Image with an explicit UV rect (ImageRectOffset/ImageRectSize).
    pub fn image_uv(&mut self, key: &str, r: Rect, uv: [f32; 4], alpha: f32) -> bool {
        let Some((idx, _, _)) = self.texture(key) else { return false };
        self.quad(idx, r, uv, [1.0, 1.0, 1.0, alpha * self.alpha]);
        true
    }

    /// Image cropped to fill `r` (ScaleType.Crop).
    pub fn image_cover(&mut self, key: &str, r: Rect, alpha: f32) -> bool {
        let Some((idx, w, h)) = self.texture(key) else { return false };
        let (ia, ra) = (w as f32 / h as f32, r.w / r.h);
        let uv = if ia > ra {
            let k = ra / ia;
            [(1.0 - k) * 0.5, 0.0, (1.0 + k) * 0.5, 1.0]
        } else {
            let k = ia / ra;
            [0.0, (1.0 - k) * 0.5, 1.0, (1.0 + k) * 0.5]
        };
        self.quad(idx, r, uv, [1.0, 1.0, 1.0, alpha * self.alpha]);
        true
    }

    pub fn image_rotated(&mut self, key: &str, center: Vec2, size: Vec2, angle: f32, alpha: f32) {
        let Some((idx, _, _)) = self.texture(key) else { return };
        let st = Vec2::new(self.sx, 1.0);
        let c = self.offset + (center + self.translate) * st * self.scale;
        let (s, co) = angle.sin_cos();
        let hx = Vec2::new(co, s) * size.x * 0.5 * self.scale * st;
        let hy = Vec2::new(-s, co) * size.y * 0.5 * self.scale * st;
        self.quad_px(idx, [c - hx - hy, c + hx - hy, c + hx + hy, c - hx + hy], [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, alpha * self.alpha]);
    }

    /// ScaleType.Slice with SliceCenter given as (left, top, right, bottom) in source pixels.
    pub fn nine_slice(&mut self, key: &str, r: Rect, slice: [f32; 4], tint: [f32; 3], alpha: f32) -> bool {
        let Some((idx, w, h)) = self.texture(key) else { return false };
        let (tw, th) = (w as f32, h as f32);
        let (l, t, rr, b) = (slice[0], slice[1], tw - slice[2], th - slice[3]);
        let xs = [r.x, r.x + l, r.x + r.w - rr, r.x + r.w];
        let ys = [r.y, r.y + t, r.y + r.h - b, r.y + r.h];
        let us = [0.0, l / tw, slice[2] / tw, 1.0];
        let vs = [0.0, t / th, slice[3] / th, 1.0];
        let color = [tint[0], tint[1], tint[2], alpha * self.alpha];
        for yi in 0..3 {
            for xi in 0..3 {
                let rc = Rect::new(xs[xi], ys[yi], xs[xi + 1] - xs[xi], ys[yi + 1] - ys[yi]);
                if rc.w <= 0.0 || rc.h <= 0.0 {
                    continue;
                }
                self.quad(idx, rc, [us[xi], vs[yi], us[xi + 1], vs[yi + 1]], color);
            }
        }
        true
    }

    /// The default Xbox selection frame (SelectionBox.png, SliceCenter 19,19,43,43), drawn `pad` px outside `r`.
    pub fn selection_box(&mut self, r: Rect, pad: f32) {
        self.nine_slice("rbxasset://textures/ui/SelectionBox.png", r.inset(-pad), [19.0, 19.0, 43.0, 43.0], [1.0, 1.0, 1.0], 1.0);
    }

    pub fn measure(&self, s: &str, font: Font, size: f32) -> f32 {
        self.fonts.measure(s, font, size)
    }

    /// Draw one line with its top-left at (x, y) inside a line box `size` tall.
    pub fn text(&mut self, s: &str, font: Font, size: f32, x: f32, y: f32, color: [f32; 3], alpha: f32) {
        let px = (size * self.scale).round().max(1.0) as u16;
        let k = px as f32 / self.scale; // virtual units per pixel-size
        let asc = self.fonts.ascent(font, px as f32) / self.scale;
        let desc = self.fonts.descent(font, px as f32) / self.scale;
        let line_h = asc - desc;
        let baseline = y + (size - line_h) * 0.5 + asc;
        let base_px = (self.offset.y + (baseline + self.translate.y) * self.scale).round();
        let mut pen = self.offset.x + (x + self.translate.x) * self.scale * self.sx;
        let _ = k;
        let color = [color[0], color[1], color[2], alpha * self.alpha];
        let tex = self.fonts.texture;
        let mut prev: Option<char> = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                pen += self.fonts.fonts[font as usize].horizontal_kern(p, ch, px as f32).unwrap_or(0.0) * self.sx;
            }
            let g = self.fonts.glyph(font, px, ch);
            if g.w > 0.0 {
                let gx = (pen + g.xmin * self.sx).round();
                let gy = base_px - g.ymin - g.h;
                let p0 = Vec2::new(gx, gy);
                let gw = g.w * self.sx;
                self.quad_px(tex, [p0, p0 + Vec2::new(gw, 0.0), p0 + Vec2::new(gw, g.h), p0 + Vec2::new(0.0, g.h)], g.uv, color);
            }
            pen += g.advance * self.sx;
            prev = Some(ch);
        }
    }

    /// TextLabel-style placement of a single line inside `r`.
    pub fn label(&mut self, s: &str, font: Font, size: f32, r: Rect, ha: Align, va: Align, color: [f32; 3], alpha: f32) {
        let w = self.measure(s, font, size);
        let x = match ha {
            Align::Start => r.x,
            Align::Center => r.x + (r.w - w) * 0.5,
            Align::End => r.x + r.w - w,
        };
        let y = match va {
            Align::Start => r.y,
            Align::Center => r.y + (r.h - size) * 0.5,
            Align::End => r.y + r.h - size,
        };
        self.text(s, font, size, x, y, color, alpha);
    }

    /// Wrapped paragraph; returns the height used.
    pub fn paragraph(&mut self, s: &str, font: Font, size: f32, r: Rect, color: [f32; 3], alpha: f32, max_lines: usize) -> f32 {
        let lines = self.wrap(s, font, size, r.w);
        let lh = size * 1.1;
        let mut y = r.y;
        for (i, l) in lines.iter().enumerate() {
            if i >= max_lines || y + lh > r.y + r.h + 0.5 {
                break;
            }
            self.text(l, font, size, r.x, y, color, alpha);
            y += lh;
        }
        y - r.y
    }

    pub fn wrap(&self, s: &str, font: Font, size: f32, width: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.measure(&cand, font, size) > width && !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = cand;
                }
            }
            out.push(line);
        }
        out
    }

    /// Truncate with an ellipsis to fit `width`.
    pub fn fit(&self, s: &str, font: Font, size: f32, width: f32) -> String {
        if self.measure(s, font, size) <= width {
            return s.to_string();
        }
        let mut t: String = s.to_string();
        while !t.is_empty() && self.measure(&(t.clone() + "..."), font, size) > width {
            t.pop();
        }
        t + "..."
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// FNV hash for building stable selection ids.
pub fn id(s: &str, n: u64) -> u64 {
    crate::assets::fnv(s.as_bytes()) ^ n.wrapping_mul(0x9E3779B97F4A7C15)
}

// >> end << //
