/*
    written by: @corescripts
*/

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod achievements;
mod assets;
mod avatar;
mod audio;
mod config;
mod gfx;
mod hub;
mod input;
mod launch;
mod mesh;
mod party;
mod rbxl;
mod scene;
mod shell;
mod strings;
mod terrain;
mod ui;

// >> imports
use assets::{Content, ImageLoader, Rgba};
use glam::Vec2;
use scene::{Flythrough, MeshKey, Scene};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

/// seconds the editor avatar takes to ease between animations, and rests before a preview replays
const POSE_BLEND: f32 = 0.3;
const PREVIEW_REST: f32 = 1.5;

enum Job {
    Mesh(MeshKey, anyhow::Result<mesh::MeshData>, bool),
    SceneTex(String, anyhow::Result<Rgba>, bool),
    Sky(String, Option<Box<[Rgba; 6]>>, bool),
    /// generation of the scene it was built for, chunks
    Terrain(u32, Vec<(glam::Vec3, mesh::MeshData, mesh::MeshData)>),
    Avatar(u64, Result<Arc<avatar::AvatarModel>, String>),
    /// animation pack / emote previewed in the avatar editor
    PreviewAnim(u64, Option<avatar::Anim>),
}

type Task = Box<dyn FnOnce() -> Job + Send>;

struct Pool {
    tx: Sender<Task>,
    rx: Receiver<Job>,
}

impl Pool {
    fn new(n: usize) -> Self {
        let (tx, jobs) = channel::<Task>();
        let (done, rx) = channel();
        let jobs = Arc::new(Mutex::new(jobs));
        for _ in 0..n {
            let jobs = jobs.clone();
            let done = done.clone();
            std::thread::spawn(move || loop {
                let t = { jobs.lock().unwrap().recv() };
                let Ok(t) = t else { break };
                if done.send(t()).is_err() {
                    break;
                }
            });
        }
        Pool { tx, rx }
    }
    fn run(&self, f: impl FnOnce() -> Job + Send + 'static) {
        let _ = self.tx.send(Box::new(f));
    }
}

struct App {
    content: Content,
    window: Option<Arc<Window>>,
    gfx: Option<gfx::Gfx>,
    ui: Option<ui::Ui>,
    shell: shell::Shell,
    input: input::Input,
    audio: audio::Audio,
    scene: Option<Scene>,
    /// the world not on screen: the menu map while the avatar editor is open, and the other way round
    menu_stash: Option<Stashed>,
    editor_stash: Option<Stashed>,
    fly: Flythrough,
    images: ImageLoader,
    pool: Pool,
    start: Instant,
    last: Instant,
    scene_started: Option<f32>,
    deferred_meshes: HashSet<MeshKey>,
    deferred_textures: HashSet<String>,
    skies_requested: HashSet<String>,
    skies_fallback: HashSet<String>,
    terrain_pending: bool,
    /// bumped on every scene swap so late terrain jobs from the old place are dropped
    scene_gen: u32,
    terrain_keys: Vec<MeshKey>,
    hub: hub::Hub,
    /// the loaded scene is Lobby3D (configuration hub) rather than Map.rbxl
    lobby: bool,
    map_path: PathBuf,
    lobby_loaded: PathBuf,
    /// the signed-in user's 3D avatar and its idle animation
    avatar: Option<Arc<avatar::AvatarModel>>,
    /// every outfit of the signed-in user, built in the background after sign-in (outfit id -> model)
    outfit_models: Arc<Mutex<HashMap<u64, Arc<avatar::AvatarModel>>>>,
    /// the user the background avatar prefetch last ran for
    prefetched_uid: Option<u64>,
    avatar_player: avatar::Player,
    /// the editor's focused animation pack or emote: (asset, its animation once loaded, start time)
    preview_anim: Option<(u64, Option<avatar::Anim>, f32)>,
    /// the editor avatar's joint poses last frame, and the pose a change of animation blends
    /// away from (with when the blend started)
    pose_last: HashMap<String, rbxl::CFrame>,
    pose_blend: Option<(HashMap<String, rbxl::CFrame>, f32)>,
    /// whether the preview animation (not the idle) was showing last frame
    preview_showing: bool,
    avatar_loading: bool,
    /// the avatar editor room (ReplicatedStorage.AvatarSceneNew) is the loaded scene
    editor_room: bool,
    /// F2: draw the background only
    hide_ui: bool,
    /// CharacterPlacement in the editor room
    editor_stage: Option<rbxl::CFrame>,
    /// Begin: black screen with the loading spinner until the menu world is ready, then fade in
    intro: bool,
    /// started with Fast Boot (no hub): the menu appears without the loading spinner
    fast_boot: bool,
    /// saved default account, used only to download place assets before anyone signs in (hub sky, unions)
    asset_session: Option<Arc<api::Session>>,
    quit: bool,
    last_refocus: Instant,
    debug: Debug,
}

#[derive(Default)]
struct Debug {
    shot: Option<PathBuf>,
    after: f32,
    zone: Option<String>,
    seek: f32,
    demo: Option<String>,
    /// sign in with the default saved account at startup (testing the real account)
    autosignin: bool,
    /// open the avatar editor after sign-in, previewing outfit N (debug)
    editor: Option<usize>,
    /// summon an achievement toast: "--achievement 3" or "--achievement 9:rare"
    achievement: Option<String>,
    /// scripted presses: one every 0.4s starting at 2s
    keys: Vec<input::Btn>,
    /// seconds before the first --keys press
    key_at: f32,
    key_i: usize,
}

fn own_hwnd(window: &Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

fn window_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../assets/branding/AppIcon256.png")).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn find_root() -> PathBuf {
    let mut starts = vec![std::env::current_dir().unwrap_or_default()];
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe.parent().map(Path::to_path_buf).unwrap_or_default());
    }
    for s in starts {
        let mut d = Some(s.as_path());
        while let Some(dir) = d {
            if dir.join("genericcontent").is_dir() && dir.join("xboxassets").is_dir() {
                return dir.to_path_buf();
            }
            d = dir.parent();
        }
    }
    PathBuf::from(".")
}

impl App {
    fn now(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    fn request_assets(&mut self) {
        let (Some(gfx), Some(scene)) = (self.gfx.as_mut(), self.scene.as_ref()) else { return };
        let session = self.shell.session.clone().or_else(|| self.asset_session.clone());
        for key in gfx.missing_meshes(scene) {
            let id = match key {
                MeshKey::Asset(i) | MeshKey::Csg(i) => i,
                _ => continue,
            };
            let content = self.content.clone();
            let s = session.clone();
            self.pool.run(move || {
                let needs_login = s.is_none() && content.cached_asset(id).is_none();
                let r = content.fetch_asset(id, s.as_deref()).and_then(|b| match key {
                    MeshKey::Csg(_) => mesh::parse_csg(&b),
                    _ => mesh::parse_mesh(&b),
                });
                Job::Mesh(key, r, needs_login)
            });
        }
        for key in gfx.missing_textures(scene) {
            let content = self.content.clone();
            let s = session.clone();
            self.pool.run(move || {
                let needs_login = s.is_none() && !key.starts_with("rbxasset://") && assets::parse_asset_id(&key).map(|i| content.cached_asset(i).is_none()).unwrap_or(false);
                let r = content.load(&key, s.as_deref()).and_then(|b| assets::decode_image(&b));
                Job::SceneTex(key, r, needs_login)
            });
        }
        let zone = scene.zone.clone();
        if !zone.is_empty() && !gfx.has_sky(&zone) && self.skies_requested.insert(zone.clone()) {
            let faces = scene.sky.faces.clone();
            let content = self.content.clone();
            let s = session.clone();
            self.pool.run(move || {
                let load = |uri: &str| content.load(uri, s.as_deref()).and_then(|b| assets::decode_image(&b));
                let got: Vec<_> = faces.iter().map(|f| load(f)).collect();
                if got.iter().all(|r| r.is_ok()) {
                    let v: Vec<Rgba> = got.into_iter().map(|r| r.unwrap()).collect();
                    let arr: [Rgba; 6] = v.try_into().ok().unwrap();
                    return Job::Sky(zone, Some(Box::new(arr)), false);
                }
                // fall back to the stock sky shipped in genericcontent
                let f = |s: &str| load(&format!("rbxasset://sky/null_plainsky512_{s}.jpg"));
                let v: Vec<Rgba> = ["bk", "dn", "ft", "lf", "rt", "up"].iter().filter_map(|n| f(n).ok()).collect();
                let arr: Option<[Rgba; 6]> = v.try_into().ok();
                Job::Sky(zone, arr.map(Box::new), true)
            });
        }
    }

    fn poll_jobs(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        while let Ok(j) = self.pool.rx.try_recv() {
            match j {
                Job::Mesh(k, Ok(m), _) => gfx.upload_mesh(k, &m),
                Job::Mesh(k, Err(e), needs_login) => {
                    if needs_login {
                        // stay hidden until the user signs in, then retry
                        self.deferred_meshes.insert(k);
                    } else {
                        eprintln!("mesh {k:?}: {e}");
                        gfx.mesh_failed(k);
                    }
                }
                Job::SceneTex(k, r, needs_login) => match r {
                    Ok(img) => gfx.set_scene_texture(&k, Some(&img)),
                    Err(_) => {
                        if needs_login {
                            self.deferred_textures.insert(k.clone());
                        }
                        gfx.set_scene_texture(&k, None)
                    }
                },
                Job::Terrain(gen, chunks) => {
                    if gen != self.scene_gen {
                        continue;
                    }
                    self.terrain_pending = false;
                    if let Some(scene) = self.scene.as_mut() {
                        for (i, (center, solid, water)) in chunks.into_iter().enumerate() {
                            if !solid.indices.is_empty() {
                                let k = MeshKey::Terrain(i as u32 * 2);
                                gfx.upload_mesh(k, &solid);
                                self.terrain_keys.push(k);
                                scene.add_terrain_part(k, center, false);
                            }
                            if !water.indices.is_empty() {
                                let k = MeshKey::Terrain(i as u32 * 2 + 1);
                                gfx.upload_mesh(k, &water);
                                self.terrain_keys.push(k);
                                scene.add_terrain_part(k, center, true);
                            }
                        }
                    }
                }
                Job::Avatar(uid, r) => {
                    self.avatar_loading = false;
                    match r {
                        Ok(m) if self.shell.user_id() == Some(uid) => {
                            if let Some(old) = self.avatar.take() {
                                for (k, _) in &old.meshes {
                                    gfx.forget_mesh(*k);
                                }
                            }
                            for (k, m) in &m.meshes {
                                gfx.upload_mesh(*k, m);
                            }
                            for (key, img) in &m.textures {
                                gfx.set_scene_texture(key, Some(img));
                            }
                            if let Some(ui) = self.ui.as_mut() {
                                ui.tex.insert("avatar3d://self".into(), ui::TexSlot::Ready { idx: gfx.avatar_ui, w: gfx::AVATAR_SIZE, h: gfx::AVATAR_SIZE });
                            }
                            self.avatar = Some(m);
                            self.shell.avatar3d = true;
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("avatar {uid}: {e}"),
                    }
                }
                Job::PreviewAnim(asset, anim) => {
                    if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
                        eprintln!("preview anim {asset}: {:?}", anim.as_ref().map(|a| (a.length, a.track_count())));
                    }
                    if let Some(p) = self.preview_anim.as_mut().filter(|p| p.0 == asset) {
                        p.1 = anim;
                        p.2 = self.start.elapsed().as_secs_f32();
                    }
                }
                Job::Sky(zone, faces, fallback) => {
                    if let Some(f) = faces {
                        gfx.set_sky(&zone, &f);
                    }
                    if fallback {
                        self.skies_fallback.insert(zone);
                    }
                }
            }
        }
    }

    /// Replace the background world (Lobby3D -> Map.rbxl when the hub hands over to the menu).
    /// The avatar editor's room, or back to the menu world.
    /// Park the current world (scene, flythrough, terrain) to bring it back later untouched.
    fn stash_scene(&mut self) -> Option<Stashed> {
        let scene = self.scene.take()?;
        Some(Stashed {
            scene,
            started: self.scene_started.take(),
            fly: std::mem::replace(&mut self.fly, Flythrough::new()),
            terrain_keys: std::mem::take(&mut self.terrain_keys),
            terrain_pending: self.terrain_pending,
            gen: self.scene_gen,
            lobby: self.lobby,
        })
    }

    fn restore_scene(&mut self, st: Stashed) {
        self.scene = Some(st.scene);
        self.scene_started = st.started;
        self.fly = st.fly;
        self.terrain_keys = st.terrain_keys;
        self.terrain_pending = st.terrain_pending;
        self.scene_gen = st.gen;
        self.lobby = st.lobby;
    }

    fn set_editor_room(&mut self, on: bool) {
        self.editor_room = on;
        // both worlds stay loaded: switching swaps them instead of rebuilding
        if on {
            self.menu_stash = self.stash_scene();
            if let Some(st) = self.editor_stash.take() {
                self.restore_scene(st);
                return;
            }
            let path = self.content.root.join("xboxassets").join("AvatarEditor.rbxl");
            self.load_scene_from(&path, Some("NewModel"));
            if let Some(s) = self.scene.as_mut() {
                // Scene_PFX bloom sprites and near-invisible neon light shafts only read right with
                // Roblox's own glow; here they wash the room out
                s.emitters.clear();
                // the room is indoors: no sun glare showing through the walls
                s.sky.celestial = false;
                for p in s.parts.iter_mut() {
                    if p.material == 288 && p.alpha < 0.2 {
                        p.visible = false;
                    }
                }
                for l in s.lights.iter_mut() {
                    l.brightness *= 0.5;
                }
            }
            let dom = std::fs::read(&path).ok().and_then(|d| rbxl::parse(&d).ok());
            // the character's HumanoidRootPart goes where Workspace.NewModel.CharacterPlacement is
            self.editor_stage = dom.as_ref().and_then(|dom| dom.instances.iter().find(|i| i.name == "CharacterPlacement").and_then(|i| i.cframe("CFrame")));
            // debug: XBOX_EDITOR_HIDE="a,b" hides room parts whose full name contains a or b
            if let (Some(s), Some(dom), Ok(hide)) = (self.scene.as_mut(), dom.as_ref(), std::env::var("XBOX_EDITOR_HIDE")) {
                for p in s.parts.iter_mut() {
                    let n = dom.full_name(p.inst);
                    if hide.split(',').any(|h| !h.is_empty() && n.contains(h)) {
                        p.visible = false;
                    }
                }
            }
            if let Some(s) = self.scene.as_mut() {
                s.set_zone("AvatarEditor", 0.0);
            }
        } else {
            self.editor_stash = self.stash_scene();
            if let Some(st) = self.menu_stash.take() {
                self.restore_scene(st);
                return;
            }
            let map = self.map_path.clone();
            self.load_scene(&map, false);
        }
    }

    fn load_scene(&mut self, path: &Path, lobby: bool) {
        self.load_scene_from(path, None);
        if lobby {
            if let Some(s) = self.scene.as_mut() {
                s.use_base_zone("Lobby");
            }
        }
        self.lobby = lobby && self.scene.is_some();
    }

    fn load_scene_from(&mut self, path: &Path, model: Option<&str>) {
        self.scene_gen += 1;
        self.lobby_loaded = path.to_path_buf();
        if let Some(g) = self.gfx.as_mut() {
            for k in self.terrain_keys.drain(..) {
                g.forget_mesh(k);
            }
        }
        self.scene = match std::fs::read(path).map_err(anyhow::Error::from).and_then(|d| rbxl::parse(&d)) {
            Ok(dom) => match model {
                Some(path) => {
                    // "A/B/C": first instance named A, then its children by name
                    let mut it = path.split('/');
                    let first = it.next().unwrap_or("");
                    let mut cur = dom.instances.iter().position(|i| i.name == first);
                    for seg in it {
                        cur = cur.and_then(|c| dom.find_child(c, seg));
                    }
                    cur.map(|r| Scene::build_from(&dom, r))
                }
                None => Some(Scene::build(&dom)),
            },
            Err(e) => {
                eprintln!("could not load {}: {e}", path.display());
                None
            }
        };
        self.lobby = false;
        self.scene_started = None;
        self.fly = Flythrough::new();
        self.terrain_pending = false;
        if let Some(src) = self.scene.as_ref().and_then(|s| s.terrain.as_ref()).map(|t| (t.grid.clone(), t.colors.clone())) {
            self.terrain_pending = true;
            let gen = self.scene_gen;
            self.pool.run(move || {
                let chunks = match terrain::decode(&src.0) {
                    Ok(grid) => terrain::build(&grid, &src.1).into_iter().map(|(k, a, b)| (terrain::chunk_center(&grid, k), a, b)).collect(),
                    Err(e) => {
                        eprintln!("terrain: {e}");
                        Vec::new()
                    }
                };
                Job::Terrain(gen, chunks)
            });
        }
    }

    /// After sign-in, load everything the avatar editor shows in the background: each outfit as a
    /// ready 3D model, then the owned items of every editor category (models + thumbnails on disk).
    fn prefetch_avatars(&mut self) {
        let uid = self.shell.user_id();
        if uid == self.prefetched_uid {
            return;
        }
        let (Some(uid), Some(s)) = (uid, self.shell.session.clone()) else {
            self.prefetched_uid = None;
            self.outfit_models.lock().unwrap().clear();
            return;
        };
        let Some(Ok((outfits, _))) = &self.shell.avatar_items else { return };
        let ids: Vec<u64> = outfits.iter().map(|o| o.id).collect();
        self.prefetched_uid = Some(uid);
        self.outfit_models.lock().unwrap().clear();
        self.shell.request_editor_items();
        let cache = self.outfit_models.clone();
        let content = self.content.clone();
        std::thread::spawn(move || {
            avatar::lower_priority();
            for o in ids {
                if cache.lock().unwrap().contains_key(&o) {
                    continue;
                }
                match avatar::load_outfit(&content, &s, uid, o) {
                    Ok(m) => {
                        cache.lock().unwrap().insert(o, Arc::new(m));
                    }
                    Err(e) => eprintln!("prefetch outfit {o}: {e}"),
                }
            }
            for ty in shell::EDITOR_ASSET_TYPES {
                let Ok(items) = s.inventory(uid, ty) else { continue };
                let ids: Vec<u64> = items.iter().map(|i| i.0).collect();
                avatar::prefetch_assets(&content, &s, &ids);
                for (_, url) in s.asset_thumbs(&ids).unwrap_or_default() {
                    let _ = content.fetch_url(&url);
                }
            }
        });
    }

    fn after_sign_in(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        for k in self.deferred_meshes.drain() {
            gfx.forget_mesh(k);
        }
        for k in self.deferred_textures.drain() {
            gfx.forget_texture(&k);
        }
        for z in self.skies_fallback.drain() {
            gfx.forget_sky(&z);
            self.skies_requested.remove(&z);
        }
    }

    fn apply_config(&mut self) {
        let cfg = &self.shell.cfg;
        strings::set_style(cfg.ui("Uppercase Words") != "Off", cfg.ui("Branding") == "Modern");
        self.audio.music_volume = cfg.music_volume;
        self.audio.loop_fading = cfg.ui("Audio Fading") != "Off";
        self.audio.sfx_volume = cfg.sfx_volume;
        if let Some(g) = self.gfx.as_mut() {
            // Configure > Lighting: Voxel (after 2020) always casts shadows
            g.shadows = cfg.shadows || cfg.ui("Lighting") != "Before 2020";
        }
        cfg.save();
    }

    fn frame(&mut self) {
        let now = self.now();
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        // debug: XBOX_FRAMETIME logs slow frames
        if dt > 0.025 && std::env::var("XBOX_FRAMETIME").is_ok() {
            eprintln!("slow frame {:.1} ms at {now:.2}", dt * 1000.0);
        }
        self.input.poll_gamepad(dt);
        if self.debug.key_i < self.debug.keys.len() && now >= self.debug.key_at + self.debug.key_i as f32 * 0.4 {
            let b = self.debug.keys[self.debug.key_i];
            self.input.pressed.push(b);
            self.debug.key_i += 1;
        }
        if self.input.toggle_hide_ui {
            self.hide_ui = !self.hide_ui;
        }
        if self.input.toggle_filter {
            self.shell.cfg.background_filter = !self.shell.cfg.background_filter;
            self.apply_config();
        }

        self.poll_jobs();
        self.request_assets();

        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let screen = Vec2::new(size.width as f32, size.height as f32);
        let mut ui = self.ui.take().unwrap();
        ui.overscan = self.shell.overscan();
        ui.mode = match self.shell.cfg.ui("Screen") {
            "Stretch" => 1,
            "Widescreen" => 2,
            _ => 0,
        };
        ui.begin(screen, &self.input, now);

        // BackgroundSceneManager: show the static background until the 3D scene is ready, then crossfade.
        let pending = self.gfx.as_ref().map(|g| g.pending_meshes()).unwrap_or(0).saturating_sub(self.deferred_meshes.len());
        if self.scene_started.is_none() && self.scene.is_some() && ((pending == 0 && !self.terrain_pending) || now > 15.0) {
            self.scene_started = Some(now);
            if let Some(s) = self.scene.as_mut().filter(|_| !self.lobby) {
                self.fly.start(s, now);
                if let Some(z) = &self.debug.zone {
                    self.fly.force(s, z, self.debug.seek, now);
                }
            }
        }
        let bg_alpha = match self.scene_started {
            Some(t0) => 1.0 - ui::ease_in_out_quad((now - t0) / 1.5),
            None => 1.0,
        };
        if bg_alpha > 0.0 {
            // covers the whole window, outside the 16:9 safe area too
            let full = ui::Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, screen.x / ui.scale, screen.y / ui.scale);
            if self.intro {
                // drawn over everything below, after the shell
            } else if self.lobby || self.hub.active {
                ui.rect(full, [0.0; 3], bg_alpha);
            } else {
                ui.image("rbxasset://textures/ui/Shell/Background/Home_screen_01.png", full, [1.0; 3], bg_alpha);
            }
            // LoadingWidget while the 3D background streams in; fades out with the splash (none on fast boot)
            if self.scene.is_some() && !self.intro && !self.fast_boot {
                let c = glam::Vec2::new(full.x + full.w - 120.0, full.y + full.h - 120.0);
                ui.image_rotated("rbxasset://textures/ui/Shell/Icons/LoadingSpinner@1080.png", c, glam::Vec2::splat(100.0), now * std::f32::consts::TAU, bg_alpha);
            }
        }

        if self.hub.active {
            let hev = self.hub.frame(&mut ui, &self.input, &mut self.audio, &mut self.shell.cfg);
            if hev.changed {
                self.apply_config();
                // "After 2022" is the original Lobby3D with its broken unions
                let want = lobby_file(&self.content.root, &self.shell.cfg);
                if self.lobby && want != self.lobby_loaded {
                    self.load_scene(&want, true);
                }
            }
            if hev.done {
                self.apply_config();
                self.intro = true;
                if self.debug.shot.is_none() {
                    self.audio.start_music();
                }
                if self.lobby {
                    let map = self.map_path.clone();
                    self.load_scene(&map, false);
                }
            }
        }
        let ev = if self.hub.active {
            shell::ShellEvents::default()
        } else {
            self.shell.frame(&mut ui, &self.input, &mut self.audio)
        };
        // after Begin: black with the Roblox loading spinner bottom-right; the spinner goes first,
        // then the black fades to reveal the menu
        if self.intro && !self.hub.active {
            let full = ui::Rect::new(-ui.offset.x / (ui.scale * ui.sx), -ui.offset.y / ui.scale, screen.x / (ui.scale * ui.sx), screen.y / ui.scale);
            let (spin, black) = match self.scene_started {
                Some(t) => ((1.0 - (now - t) / 0.35).clamp(0.0, 1.0), 1.0 - ui::ease_in_out_quad(((now - t - 0.35) / 1.0).clamp(0.0, 1.0))),
                None => (1.0, 1.0),
            };
            ui.rect(full, [0.0; 3], black);
            if spin > 0.0 {
                let c = glam::Vec2::new(full.x + full.w - 120.0, full.y + full.h - 120.0);
                ui.image_rotated("rbxasset://textures/ui/Shell/Icons/LoadingSpinner@1080.png", c, glam::Vec2::splat(100.0), now * std::f32::consts::TAU, spin);
            }
            if black <= 0.0 {
                self.intro = false;
            }
        }
        // Configure > Screen: "Borders" letterboxes the 16:9 UI
        if self.shell.cfg.ui("Screen") == "Borders" {
            let full = ui::Rect::new(-ui.offset.x / ui.scale, -ui.offset.y / ui.scale, screen.x / ui.scale, screen.y / ui.scale);
            ui.rect(ui::Rect::new(full.x, full.y, full.w, -full.y), [0.0; 3], 1.0);
            ui.rect(ui::Rect::new(full.x, 1080.0, full.w, full.y + full.h - 1080.0), [0.0; 3], 1.0);
            ui.rect(ui::Rect::new(full.x, full.y, -full.x, full.h), [0.0; 3], 1.0);
            ui.rect(ui::Rect::new(1920.0, full.y, full.x + full.w - 1920.0, full.h), [0.0; 3], 1.0);
        }
        if ev.quit {
            self.quit = true;
        }
        if let Some(h) = ev.focus_game {
            launch::force_foreground(h);
        }
        if ev.focus_self || (ev.keep_focus && !window.has_focus() && self.last_refocus.elapsed().as_secs_f32() > 1.0) {
            self.last_refocus = Instant::now();
            if let Some(h) = own_hwnd(&window) {
                launch::force_foreground(h);
            }
        }
        if ev.signed_in {
            self.after_sign_in();
        }
        if self.shell.editor_open() != self.editor_room && !self.hub.active {
            let on = self.shell.editor_open();
            self.set_editor_room(on);
        }
        // the editor's local avatar (changed before Roblox confirmed it)
        if self.shell.editor_desc_dirty && !self.avatar_loading && self.shell.preview_outfit.is_none() {
            self.shell.editor_desc_dirty = false;
            self.shell.avatar_dirty = false;
            if let (Some(uid), Some(s), Some(desc)) = (self.shell.user_id(), self.shell.session.clone(), self.shell.editor_desc.clone()) {
                self.avatar_loading = true;
                let content = self.content.clone();
                self.pool.run(move || Job::Avatar(uid, avatar::load_desc(&content, &s, uid, &desc).map(Arc::new).map_err(|e| e.to_string())));
            }
        }
        // (re)build the 3D avatar after sign-in or an outfit change
        if self.shell.avatar_dirty && !self.avatar_loading {
            self.shell.avatar_dirty = false;
            if let (Some(uid), Some(s)) = (self.shell.user_id(), self.shell.session.clone()) {
                self.avatar_loading = true;
                let content = self.content.clone();
                let outfit = self.shell.preview_outfit;
                let cache = self.outfit_models.clone();
                self.pool.run(move || {
                    let r = match outfit {
                        Some(o) => match cache.lock().unwrap().get(&o).cloned() {
                            Some(m) => Ok(m),
                            None => avatar::load_outfit(&content, &s, uid, o).map(Arc::new),
                        },
                        None => avatar::load(&content, &s, uid).map(Arc::new),
                    };
                    if let (Some(o), Ok(m)) = (outfit, &r) {
                        cache.lock().unwrap().insert(o, m.clone());
                    }
                    Job::Avatar(uid, r.map_err(|e| e.to_string()))
                });
            }
        }
        self.prefetch_avatars();
        // editor: play the focused animation pack / emote instead of the idle
        let want = if self.editor_room { self.shell.preview_anim } else { None };
        if want != self.preview_anim.as_ref().map(|p| p.0) {
            self.preview_anim = want.map(|a| (a, None, 0.0));
            if let (Some(a), Some(s)) = (want, self.shell.session.clone()) {
                let content = self.content.clone();
                self.pool.run(move || Job::PreviewAnim(a, avatar::load_preview_anim(&content, &s, a)));
            }
        }
        if self.shell.user_id().is_none() && self.avatar.is_some() {
            self.avatar = None;
            self.shell.avatar3d = false;
        }
        if ev.config_changed {
            self.apply_config();
        }
        if ev.toggle_fullscreen {
            let fs = window.fullscreen().is_none();
            window.set_fullscreen(if fs { Some(Fullscreen::Borderless(None)) } else { None });
            self.shell.cfg.fullscreen = fs;
            self.apply_config();
        }
        self.audio.update();

        // UI textures
        let image_session = self.shell.session.clone().or_else(|| self.asset_session.clone());
        for key in ui.tex_requests.drain(..) {
            self.images.request(&key, image_session.clone());
        }
        let gfx = self.gfx.as_mut().unwrap();
        for (key, res) in self.images.poll() {
            match res {
                Ok(img) => {
                    let idx = gfx.add_ui_texture(img.w, img.h, &img.data);
                    ui.tex.insert(key, ui::TexSlot::Ready { idx, w: img.w, h: img.h });
                }
                Err(e) => {
                    eprintln!("image {key}: {e}");
                    ui.tex.insert(key, ui::TexSlot::Failed);
                }
            }
        }
        if let Some((y0, y1)) = ui.fonts.dirty.take() {
            gfx.update_ui_texture_rows(ui.fonts.texture, y0, y1, &ui.fonts.atlas);
        }

        // scene + camera
        let look = self.input.look;
        let mut cam = None;
        let mut fx = None;
        if let (Some(scene), Some(_), true) = (self.scene.as_mut(), self.scene_started, self.editor_room) {
            scene.update(now, dt);
            fx = Some(if self.shell.cfg.background_filter { gfx::PostFx { clear: 0.62, bloom: 0.2, ..gfx::PostFx::menu() } } else { gfx::PostFx { bloom: 0.2, ..Default::default() } });
            cam = Some(avatar::editor_camera(self.editor_stage));
        } else if let (Some(scene), Some(_), true) = (self.scene.as_mut(), self.scene_started, self.lobby) {
            scene.update(now, dt);
            // the menu's dark blue background filter, like the Shell behind it
            fx = Some(if self.shell.cfg.background_filter {
                gfx::PostFx::menu()
            } else {
                // Lobby3D's own ColorCorrection
                gfx::PostFx { brightness: -0.01, contrast: 0.01, saturation: 0.0, tint: glam::Vec3::ONE, blur: 0.0, ..Default::default() }
            });
            cam = Some(self.hub.camera(now));
        } else if let (Some(scene), Some(_)) = (self.scene.as_mut(), self.scene_started) {
            let info = self.fly.frame(scene, now, dt, look);
            scene.update(now, dt);
            let filter = self.shell.cfg.background_filter;
            let brightness = self.fly.brightness(now);
            fx = Some(if filter {
                gfx::PostFx { brightness, contrast: info.contrast, saturation: info.saturation, tint: info.tint, blur: info.blur, ..Default::default() }
            } else {
                // unfiltered view of the lit scene; keep the zone-transition fades
                let fade = ((info.contrast - 0.5) / 1.5).min(0.0);
                gfx::PostFx { brightness: (brightness - 0.3).min(0.0) + fade, contrast: 0.0, saturation: 0.0, tint: glam::Vec3::ONE, blur: (info.blur - 10.0).max(0.0), ..Default::default() }
            });
            cam = Some(info.cframe);
        }
        let scene_arg = match (&self.scene, &cam, &fx) {
            (Some(s), Some(c), Some(f)) => Some((s, c, f)),
            _ => None,
        };
        if let Some(p) = &self.debug.shot {
            if now >= self.debug.after {
                gfx.capture = Some(p.clone());
            }
        }
        let capturing = gfx.capture.is_some();
        // in the editor the avatar stands on the room's platform; it is not drawn anywhere else
        let room_avatar: Vec<scene::Part> = match (&self.avatar, self.editor_room) {
            (Some(m), true) => {
                // the focused pack / emote plays once, the avatar eases back to its idle, then
                // the preview plays again after a short rest
                if let Some((_, Some(a), t0)) = self.preview_anim.as_mut() {
                    if now - *t0 >= a.length + PREVIEW_REST {
                        *t0 = now;
                    }
                }
                let (anim, t, showing) = match &self.preview_anim {
                    Some((_, Some(a), t0)) if now - t0 < a.length => (Some(a), now - t0, true),
                    _ => {
                        let (a, t) = self.avatar_player.frame(m, now);
                        (a, t, false)
                    }
                };
                // any switch between animations is a short tween from the last pose shown
                if showing != self.preview_showing && !self.pose_last.is_empty() {
                    self.pose_blend = Some((std::mem::take(&mut self.pose_last), now));
                }
                self.preview_showing = showing;
                let w = match &self.pose_blend {
                    Some((_, t0)) if now - t0 < POSE_BLEND => {
                        let x = (now - t0) / POSE_BLEND;
                        x * x * (3.0 - 2.0 * x)
                    }
                    _ => {
                        self.pose_blend = None;
                        1.0
                    }
                };
                let from = self.pose_blend.as_ref().map(|b| &b.0);
                let mut last = HashMap::new();
                for name in m.joint_names() {
                    let target = m.joint(anim, t, name);
                    let cf = match from.and_then(|f| f.get(name)) {
                        Some(a) => a.lerp(&target, w),
                        None => target,
                    };
                    last.insert(name.to_string(), cf);
                }
                let parts = m.pose_with(&|name| last.get(name).copied().unwrap_or_default());
                self.pose_last = last;
                avatar::place_on_stage(self.editor_stage, parts, self.shell.editor_yaw)
            }
            _ => Vec::new(),
        };
        let no_ui = ui::UiFrame { vertices: Vec::new(), batches: Vec::new() };
        gfx.render(scene_arg, None, &room_avatar, if self.hide_ui { &no_ui } else { &ui.frame }, None);
        // the window stays hidden until there is a real frame to show (no white flash)
        if !window.is_visible().unwrap_or(true) {
            window.set_visible(true);
            if self.debug.shot.is_none() {
                window.focus_window();
            }
        }
        if capturing {
            std::process::exit(0);
        }
        self.ui = Some(ui);
        self.input.begin_frame();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // test captures open on the second display (Windows' monitor order) so they stay out of the way
        let primary = el.primary_monitor();
        let second = if self.debug.shot.is_some() { el.available_monitors().find(|m| Some(m) != primary.as_ref()) } else { None };
        let attrs = Window::default_attributes().with_visible(false).with_active(self.debug.shot.is_none()).with_title("XBLOX: REWIND").with_window_icon(window_icon()).with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0)).with_min_inner_size(winit::dpi::LogicalSize::new(640.0, 360.0));
        let attrs = match &second {
            Some(m) => attrs.with_position(winit::dpi::PhysicalPosition::new(m.position().x + 40, m.position().y + 40)),
            None => attrs,
        };
        let window = Arc::new(el.create_window(attrs).expect("window"));
        if self.shell.cfg.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let mut gfx = gfx::Gfx::new(window.clone()).expect("graphics init failed");
        gfx.shadows = self.shell.cfg.shadows;
        let mut fonts = ui::Fonts::new(&self.content);
        fonts.texture = gfx.add_ui_texture(fonts.size, fonts.size, &fonts.atlas);
        let mut ui = ui::Ui::new(fonts);
        // the first frame already has the splash background, logo and spinner
        for key in [
            "rbxasset://textures/ui/Shell/Background/Home_screen_01.png",
            "rbxasset://textures/ui/Shell/Icons/ROBLOXSplashLogo.png",
            "rbxasset://textures/ui/Shell/Icons/LoadingSpinner@1080.png",
        ] {
            if let Ok(img) = self.content.read_local(key).and_then(|b| assets::decode_image(&b)) {
                let idx = gfx.add_ui_texture(img.w, img.h, &img.data);
                ui.tex.insert(key.to_string(), ui::TexSlot::Ready { idx, w: img.w, h: img.h });
            }
        }
        self.ui = Some(ui);
        self.gfx = Some(gfx);
        self.window = Some(window.clone());
        if self.debug.shot.is_none() && !self.hub.active {
            self.audio.start_music();
        }
        if let Some(a) = &self.debug.achievement {
            let rare = a.ends_with("rare");
            let n: usize = a.trim_end_matches("rare").trim_end_matches(':').parse().unwrap_or(1);
            let id = achievements::DEFS[n.clamp(1, achievements::DEFS.len()) - 1].id;
            self.shell.ach.simulate(id, rare);
        }
        if self.debug.autosignin {
            self.shell.auto_sign_in();
        }
        self.shell.debug_editor = self.debug.editor;
        if let Some(d) = &self.debug.demo {
            self.shell.demo(d);
        }
        // hidden windows get no redraw events, so draw the first frame here (it also shows the window)
        self.frame();
        window.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(g) = self.gfx.as_mut() {
                    g.resize(s.width, s.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.input.modifiers(m.state()),
            WindowEvent::KeyboardInput { event, .. } => {
                let text_mode = self.shell.text_mode();
                self.input.key(event.physical_key, event.state, event.repeat, event.text.as_deref(), text_mode);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(w) = &self.window {
                    let s = w.inner_size();
                    self.input.mouse_move(Vec2::new(position.x as f32, position.y as f32), Vec2::new(s.width as f32, s.height as f32));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => self.input.mouse_button(button, state),
            WindowEvent::RedrawRequested => {
                self.frame();
                if self.quit {
                    el.exit();
                    return;
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Lobby3D for the configuration hub: the fixed copy, or the original (broken unions) for "After 2022" lighting.
fn lobby_file(root: &Path, cfg: &config::Config) -> PathBuf {
    let dir = root.join("VRMenu").join("content").join("places");
    let original = dir.join("Lobby3D.rbxl");
    if cfg.ui("Lighting") == "After 2022" && original.exists() {
        original
    } else {
        dir.join("Lobby3DFixed.rbxl")
    }
}

/// A loaded world kept aside (see App::stash_scene).
struct Stashed {
    scene: Scene,
    started: Option<f32>,
    fly: Flythrough,
    terrain_keys: Vec<MeshKey>,
    terrain_pending: bool,
    gen: u32,
    lobby: bool,
}

fn dump(path: &str, filter: Option<&String>) -> anyhow::Result<()> {
    let dom = rbxl::parse(&std::fs::read(path)?)?;
    for (i, inst) in dom.instances.iter().enumerate() {
        if filter.map(|f| &inst.class != f).unwrap_or(false) {
            continue;
        }
        println!("{} [{}]", dom.full_name(i), inst.class);
        let mut keys: Vec<_> = inst.props.iter().collect();
        keys.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in keys {
            let s = match v {
                rbxl::Value::String(b) if b.iter().all(|c| (32..127).contains(c)) => String::from_utf8_lossy(b).into_owned(),
                _ => format!("{v:?}"),
            };
            println!("    {k} = {}", if s.len() > 120 { format!("{}...({} chars)", &s[..120], s.len()) } else { s });
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "--rawprop").unwrap_or(false) {
        let dom = rbxl::parse(&std::fs::read(&args[2])?)?;
        let inst = dom.instances.iter().find(|i| i.class == args[3]).ok_or_else(|| anyhow::anyhow!("no {}", args[3]))?;
        let v = inst.get(&args[4]).ok_or_else(|| anyhow::anyhow!("no prop"))?;
        std::fs::write(&args[5], dom.shared(v).unwrap_or_default())?;
        return Ok(());
    }
    if args.get(1).map(|a| a == "--meshtest").unwrap_or(false) {
        // --meshtest <id>: which mesh versions asset delivery hands out
        let accounts = config::load_accounts();
        let s = api::Session::new(&accounts.first().map(|a| a.cookie.clone()).unwrap_or_default());
        let id: u64 = args[2].parse()?;
        match s.asset_delivery_v1(id) {
            Ok(b) => {
                println!("v1: {} bytes, {:?}", b.len(), String::from_utf8_lossy(&b[..b.len().min(12)]));
                match mesh::parse_mesh(&b) {
                    Ok(m) => {
                        println!("parsed: {} verts {} indices", m.vertices.len(), m.indices.len());
                        let (mut lo, mut hi) = ([f32::MAX; 5], [f32::MIN; 5]);
                        for v in &m.vertices {
                            for (k, x) in v.pos.iter().chain(v.uv.iter()).enumerate() {
                                lo[k] = lo[k].min(*x);
                                hi[k] = hi[k].max(*x);
                            }
                        }
                        println!("bounds xyz {:.3?} .. {:.3?}  uv {:.3?} .. {:.3?}", &lo[..3], &hi[..3], &lo[3..], &hi[3..]);
                    }
                    Err(e) => println!("parse: {e}"),
                }
            }
            Err(e) => println!("v1: {e}"),
        }
        for major in ["4"] {
            match s.asset_delivery_mesh(id, major) {
                Ok(b) => println!("{major}: {} bytes, {:?}", b.len(), String::from_utf8_lossy(&b[..b.len().min(12)])),
                Err(e) => println!("{major}: {e}"),
            }
        }
        return Ok(());
    }
    if args.get(1).map(|a| a == "--audiotest").unwrap_or(false) {
        use rodio::Source;
        let c = Content::new(find_root());
        let b = c.read_local("rbxasset://sounds/ui/Shell/RobloxMusic.ogg")?;
        let d = rodio::Decoder::new(std::io::Cursor::new(b))?;
        println!("ok: {} Hz, {} ch, duration {:?}", d.sample_rate(), d.channels(), d.total_duration());
        return Ok(());
    }
    if args.get(1).map(|a| a == "--dump").unwrap_or(false) {
        return dump(&args[2], args.get(3));
    }
    let root = find_root();
    let content = Content::new(root.clone());
    let mut cfg = config::Config::load();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    match arg("--icons").as_deref() {
        Some("keyboard") => cfg.icons = config::IconSet::Keyboard,
        Some("playstation") => cfg.icons = config::IconSet::PlayStation,
        Some("xbox") => cfg.icons = config::IconSet::Xbox,
        _ => {}
    }
    input::set_icon_set(cfg.icons);
    strings::set_style(cfg.ui("Uppercase Words") != "Off", cfg.ui("Branding") == "Modern");
    let debug = Debug {
        shot: arg("--shot").map(PathBuf::from),
        after: arg("--after").and_then(|v| v.parse().ok()).unwrap_or(12.0),
        zone: arg("--zone"),
        achievement: arg("--achievement"),
        autosignin: args.iter().any(|a| a == "--autosignin"),
        editor: arg("--editor").and_then(|v| v.parse().ok()),
        seek: arg("--seek").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        demo: arg("--demo"),
        keys: arg("--keys")
            .unwrap_or_default()
            .split(',')
            .filter_map(|k| {
                use input::Btn::*;
                Some(match k.trim() {
                    "Up" => Up,
                    "Down" => Down,
                    "Left" => Left,
                    "Right" => Right,
                    "A" => A,
                    "B" => B,
                    "X" => X,
                    "Y" => Y,
                    "LB" => LB,
                    "RB" => RB,
                    "View" => View,
                    "RS" => RS,
                    _ => return None,
                })
            })
            .collect(),
        key_i: 0,
        key_at: arg("--keyat").and_then(|v| v.parse().ok()).unwrap_or(2.0),
    };
    if args.iter().any(|a| a == "--nofilter") {
        cfg.background_filter = false;
    }
    let map = args.iter().skip(1).find(|a| a.ends_with(".rbxl")).map(PathBuf::from).unwrap_or_else(|| root.join("Map.rbxl"));
    let cfg_default = cfg.default_account;
    let mut hub = hub::Hub::new(&cfg);
    if let Some(h) = arg("--hub") {
        hub.active = true;
        hub.demo(&h);
    }
    if debug.demo.is_some() || debug.autosignin {
        hub.active = false;
    }
    let lobby_path = lobby_file(&root, &cfg);
    let mut audio = audio::Audio::new(&content);
    audio.music_volume = cfg.music_volume;
    audio.sfx_volume = cfg.sfx_volume;
    let mut app = App {
        images: ImageLoader::new(content.clone(), 4),
        pool: Pool::new(3),
        content,
        window: None,
        gfx: None,
        ui: None,
        shell: shell::Shell::new(cfg),
        input: input::Input::new(),
        audio,
        scene: None,
        fly: Flythrough::new(),
        menu_stash: None,
        editor_stash: None,
        start: Instant::now(),
        last: Instant::now(),
        scene_started: None,
        deferred_meshes: HashSet::new(),
        deferred_textures: HashSet::new(),
        skies_requested: HashSet::new(),
        skies_fallback: HashSet::new(),
        terrain_pending: false,
        scene_gen: 0,
        terrain_keys: Vec::new(),
        lobby: false,
        map_path: map.clone(),
        lobby_loaded: PathBuf::new(),
        intro: false,
        fast_boot: !hub.active,
        editor_room: false,
        hide_ui: std::env::var("XBOX_HIDE_UI").is_ok(),
        editor_stage: None,
        avatar: None,
        avatar_player: avatar::Player::new(),
        preview_anim: None,
        pose_last: HashMap::new(),
        pose_blend: None,
        preview_showing: false,
        avatar_loading: false,
        outfit_models: Arc::new(Mutex::new(HashMap::new())),
        prefetched_uid: None,
        asset_session: {
            let accounts = config::load_accounts();
            let default = cfg_default;
            accounts.iter().find(|a| a.id == default && default != 0).or(accounts.first()).map(|a| Arc::new(api::Session::new(&a.cookie)))
        },
        hub,
        quit: false,
        last_refocus: Instant::now(),
        debug,
    };
    if app.hub.active && lobby_path.exists() {
        app.load_scene(&lobby_path, true);
    } else {
        app.load_scene(&map, false);
    }
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut app)?;
    Ok(())
}

// >> end << //
