//! The Horizon mod SDK (Stink-O/horizon `horizon-mod-sdk`, wit/horizon.wit) for a native build: the same modules,
//! types and functions the WebAssembly SDK's bindings give, so the mod's code builds unchanged. Each call does what
//! the fork's runtime (`horizon-mods/src/runtime.rs`) did on the host side of the WebAssembly boundary: it reads the
//! snapshot the game gave before the call, or keeps state, or queues an `Output`. The game's glue drives it through
//! `__host`. The WebAssembly sandbox's per-call caps are left out; value clamps stay.
#![allow(dead_code, clippy::all)]


pub mod log {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Level {
        Debug,
        Info,
        Warn,
        Error,
    }
    pub fn write(level: Level, message: &str) {
        let level = match level {
            Level::Debug => ::log::Level::Debug,
            Level::Info => ::log::Level::Info,
            Level::Warn => ::log::Level::Warn,
            Level::Error => ::log::Level::Error,
        };
        let id = super::__host::with(|s| {
            if s.logs.len() < 256 {
                s.logs.push((level as u8, message.to_owned()));
            }
            s.id.clone()
        });
        ::log::log!(target: "horizon_mods", level, "[mod {}] {}", id, message);
    }
}

pub mod game {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
    pub struct Player {
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: f32,
        pub pitch: f32,
        pub health: f32,
        pub food: u32,
        pub dimension: String,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct Clock {
        pub game_time: u64,
        pub day_time: u64,
    }
    pub fn get_player() -> Option<Player> {
        super::__host::with(|s| s.snapshot.player.clone())
    }
    pub fn get_clock() -> Option<Clock> {
        super::__host::with(|s| s.snapshot.clock)
    }
    /// Whether the local player flies (creative flight, or a spectator).
    pub fn player_flying() -> bool {
        super::__host::with(|s| s.snapshot.flying)
    }
}

pub mod chat {
    pub fn show(text: &str) {
        let text = text.to_owned();
        super::__host::with(|s| {
            let name = s.name.clone();
            s.out.push(super::__host::Output::Chat { name, text })
        });
    }
}

pub mod hud {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Corner {
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct Line {
        pub text: String,
        pub color: u32,
    }
    pub fn set_panel(corner: Corner, lines: &[Line]) {
        let lines = lines
            .iter()
            .take(super::__host::HUD_LINES)
            .map(|l| Line {
                text: l.text.clone(),
                color: l.color & 0xFF_FFFF,
            })
            .collect();
        super::__host::with(|s| s.panel = Some((corner, lines)));
    }
    pub fn clear() {
        super::__host::with(|s| s.panel = None);
    }
}

pub mod storage {
    fn key_ok(key: &str) -> bool {
        let chars_ok = (1..=64).contains(&key.len())
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
        chars_ok && key != "." && key != ".."
    }
    pub fn get(key: &str) -> Option<Vec<u8>> {
        let dir = super::__host::with(|s| s.data_dir.clone());
        key_ok(key).then(|| std::fs::read(dir.join(key)).ok()).flatten()
    }
    pub fn set(key: &str, value: &[u8]) -> Result<(), String> {
        if !key_ok(key) {
            return Err(format!("{key:?} is not [a-z0-9_.-]{{1,64}}"));
        }
        let dir = super::__host::with(|s| s.data_dir.clone());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(key), value).map_err(|e| e.to_string())
    }
    pub fn delete(key: &str) {
        if key_ok(key) {
            let dir = super::__host::with(|s| s.data_dir.clone());
            let _ = std::fs::remove_file(dir.join(key));
        }
    }
}

pub mod input {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct HeldItem {
        pub id: String,
        pub count: u32,
        pub slot: u32,
        pub custom_name: Option<String>,
        pub custom_data: Option<String>,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct ItemUse {
        pub item: HeldItem,
        pub ticks: u32,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
    pub struct Click {
        pub x: f32,
        pub y: f32,
        pub button: u32,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub enum Event {
        UseStart(ItemUse),
        UseStop(ItemUse),
        KeyDown(String),
        KeyUp(String),
        ScreenClick(Click),
        ScreenClosed,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub enum Typed {
        Text(String),
        Key(String),
    }
    pub fn claim_items(ids: &[String]) {
        let ids = ids.iter().take(64).cloned().collect();
        super::__host::with(|s| s.claims = ids);
    }
    pub fn register_key(name: &str, label: &str, key: &str) {
        let k = super::__host::ModKey {
            name: name.to_owned(),
            label: label.to_owned(),
            key: key.to_owned(),
        };
        super::__host::with(|s| {
            if s.keys.contains(&k) {
                return;
            }
            s.keys.retain(|o| o.name != k.name);
            s.keys.push(k);
            s.out.push(super::__host::Output::Keys);
        });
    }
    pub fn held() -> Option<HeldItem> {
        super::__host::with(|s| s.snapshot.held.clone())
    }
    pub fn poll() -> Vec<Event> {
        super::__host::with(|s| std::mem::take(&mut s.events))
    }
    pub fn poll_typed() -> Vec<Typed> {
        super::__host::with(|s| std::mem::take(&mut s.typed))
    }
}

pub mod ui {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Default)]
    pub struct Rect {
        pub x: f32,
        pub y: f32,
        pub w: f32,
        pub h: f32,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Default)]
    pub struct Point {
        pub x: f32,
        pub y: f32,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub enum Draw {
        Fill((Rect, u32)),
        Quad((Vec<Point>, u32)),
        Sprite((String, Rect, u32)),
        Item((String, String, Point, f32)),
        Text((String, Point, u32, bool)),
    }
    pub fn show(draws: &[Draw]) {
        let draws: Vec<Draw> = draws.iter().take(4096).cloned().collect();
        super::__host::with(|s| {
            if s.screen.replace(draws).is_none() {
                s.out.push(super::__host::Output::Screen { open: true });
            }
        });
    }
    pub fn close() {
        super::__host::with(|s| {
            if s.screen.take().is_some() {
                s.typed.clear();
                s.out.push(super::__host::Output::Screen { open: false });
            }
        });
    }
    pub fn cursor() -> Option<Point> {
        super::__host::with(|s| s.cursor)
    }
}

pub mod render {
    use std::sync::Arc;
    pub fn set_post(wgsl: &str) {
        if wgsl.len() > super::__host::POST_SOURCE {
            ::log::warn!("post pass over {} KiB, ignored", super::__host::POST_SOURCE >> 10);
            return;
        }
        super::__host::with(|s| {
            if s.post_source.as_ref().is_some_and(|(src, _)| **src == *wgsl) {
                return;
            }
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            s.id.hash(&mut h);
            wgsl.hash(&mut h);
            s.post_source = Some((Arc::from(wgsl), h.finish()));
        });
    }
    pub fn set_params(params: &[f32]) {
        let mut p = [0.0; super::__host::POST_PARAMS];
        for (to, from) in p.iter_mut().zip(params) {
            *to = if from.is_finite() { *from } else { 0.0 };
        }
        super::__host::with(|s| s.post_params = Some(p));
    }
    pub fn clear_post() {
        super::__host::with(|s| {
            s.post_source = None;
            s.post_params = None;
            s.post_data = None;
        });
    }
    pub fn set_data(words: &[u32]) {
        if words.len() > super::__host::POST_DATA_WORDS {
            ::log::warn!("post pass data over {} KiB, ignored", (super::__host::POST_DATA_WORDS * 4) >> 10);
            return;
        }
        super::__host::with(|s| {
            if s.post_data.as_ref().is_some_and(|(d, _)| **d == *words) {
                return;
            }
            let version = s.post_data.as_ref().map_or(1, |(_, v)| v + 1);
            s.post_data = Some((Arc::from(words), version));
        });
    }
    pub fn shake(strength: f32, seconds: f32) {
        if strength.is_finite() && seconds.is_finite() && strength > 0.0 && seconds > 0.0 {
            super::__host::with(|s| {
                s.out.push(super::__host::Output::Shake {
                    strength: strength.min(1.0),
                    seconds: seconds.min(60.0),
                })
            });
        }
    }
}

pub mod world {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct Hit {
        pub block_x: i32,
        pub block_y: i32,
        pub block_z: i32,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub block: String,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Heightmap {
        WorldSurface,
        MotionBlocking,
        MotionBlockingNoLeaves,
        OceanFloor,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
    pub struct BlockRun {
        pub x: i32,
        pub z: i32,
        pub y: i32,
        pub height: u32,
        pub surface: Option<Heightmap>,
        pub block: u32,
        pub only: Option<u32>,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
    pub struct BlockEdit {
        pub palette: Vec<String>,
        pub filters: Vec<String>,
        pub runs: Vec<BlockRun>,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct SurfaceHeights {
        pub tag: u32,
        pub heights: Vec<Option<i32>>,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct ColumnBlocks {
        pub tag: u32,
        pub palette: Vec<String>,
        pub bases: Vec<Option<i32>>,
        pub blocks: Vec<u16>,
        pub tints: Vec<u32>,
    }
    use super::__host::{Output, with};
    pub fn raycast(distance: f32) -> Option<Hit> {
        let max = distance.clamp(0.0, super::__host::RAYCAST_MAX);
        with(|s| {
            s.snapshot
                .view_hit
                .as_ref()
                .filter(|(d, _)| *d <= max)
                .map(|(_, hit)| hit.clone())
        })
    }
    pub fn run_commands(commands: &[String]) {
        if !commands.is_empty() {
            let commands = commands.to_vec();
            with(|s| s.out.push(Output::Commands { commands }));
        }
    }
    pub fn play_sound(event: &str, at: Option<(f64, f64, f64)>, volume: f32, pitch: f32) {
        let event = event.to_owned();
        with(|s| {
            s.out.push(Output::Sound {
                event,
                at: at.map(|(x, y, z)| [x, y, z]),
                volume: if volume.is_finite() { volume.clamp(0.0, 16.0) } else { 1.0 },
                pitch: if pitch.is_finite() { pitch.clamp(0.0, 4.0) } else { 1.0 },
            })
        });
    }
    pub fn push_player(x: f32, y: f32, z: f32) {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            with(|s| {
                s.out.push(super::__host::Output::Push {
                    velocity: [x.clamp(-4.0, 4.0), y.clamp(-4.0, 4.0), z.clamp(-4.0, 4.0)],
                })
            });
        }
    }
    /// The fork's `world.carry-player`: the local player moved by `x`, `y`, `z` this tick in place of their own
    /// motion, their view turned by `turn` degrees; whether the carry held them (the glue's `carry_step`).
    pub fn carry_player(x: f32, y: f32, z: f32, turn: f32) -> bool {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return false;
        }
        with(|s| {
            s.out.retain(|o| !matches!(o, super::__host::Output::Carry { .. }));
            s.out.push(super::__host::Output::Carry {
                delta: [x.clamp(-4.0, 4.0), y.clamp(-4.0, 4.0), z.clamp(-4.0, 4.0)],
                turn: if turn.is_finite() { turn.clamp(-6.0, 6.0) } else { 0.0 },
            });
            s.snapshot.carried
        })
    }
    pub fn set_flying_speed(speed: Option<f32>) {
        let speed = speed
            .filter(|s| s.is_finite())
            .map(|s| s.clamp(0.0, super::__host::FLYING_SPEED_MAX));
        with(|s| s.flying_speed = speed);
    }
    pub fn edit_blocks(edit: &BlockEdit) {
        let palette = edit.palette.len();
        let filters = edit.filters.len();
        if let Some(bad) = edit
            .runs
            .iter()
            .find(|r| r.block as usize >= palette || r.only.is_some_and(|f| f as usize >= filters))
        {
            ::log::warn!(
                "edit-blocks: a run at {} {} {} names palette entry {} or filter {:?}, past the end; left out",
                bad.x, bad.y, bad.z, bad.block, bad.only
            );
            return;
        }
        if !edit.runs.is_empty() {
            let edit = edit.clone();
            with(|s| s.out.push(Output::Edit { edit }));
        }
    }
    pub fn surface_height(tag: u32, columns: &[(i32, i32)], heightmap: Heightmap) {
        let columns = columns.to_vec();
        with(|s| s.out.push(Output::Surface { tag, columns, heightmap }));
    }
    pub fn poll_surface_heights() -> Vec<SurfaceHeights> {
        with(|s| std::mem::take(&mut s.surface))
    }
    pub fn read_columns(tag: u32, columns: &[(i32, i32)], heightmap: Heightmap, below: u32, above: u32) {
        let columns = columns.to_vec();
        with(|s| s.out.push(Output::Columns { tag, columns, heightmap, below, above }));
    }
    pub fn poll_columns() -> Vec<ColumnBlocks> {
        with(|s| std::mem::take(&mut s.columns))
    }
}

/// Capability `atlas` (the fork's API 10): where blocks' textures are in the block sprite arrays, answered at once by
/// the game (`native_block_sprites`, through `__host::Atlas`); the arrays themselves are the post pass's bindings 8
/// to 16 (`WorldDrawn::block_sprites`).
pub mod atlas {
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
    pub enum Tint {
        None,
        Grass,
        Foliage,
        DryFoliage,
        Water,
        Fixed(u32),
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
    pub struct Sprite {
        pub array: u32,
        pub layer: u32,
        pub animated: bool,
        pub cutout: bool,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
    pub struct Face {
        pub sprite: Sprite,
        pub rect: (f32, f32, f32, f32),
        pub inset: f32,
        pub uv: (f32, f32, f32, f32, f32, f32),
        pub tint: Tint,
        pub translucent: bool,
    }
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct BlockFaces {
        pub sides: Vec<Vec<Face>>,
        pub particle: Option<Sprite>,
        pub cube: bool,
    }
    /// At most this many names a call, as the fork's runtime; the rest are none.
    const MAX_NAMES: usize = 4096;
    pub fn block_sprites(names: &[String]) -> Vec<Option<BlockFaces>> {
        let atlas = super::__host::with(|s| s.atlas.clone());
        let mut out = match atlas {
            Some(a) => (a.blocks)(&names[..names.len().min(MAX_NAMES)]),
            None => Vec::new(),
        };
        out.resize(names.len(), None);
        out
    }
    pub fn texture_sprites(names: &[String]) -> Vec<Option<Sprite>> {
        let atlas = super::__host::with(|s| s.atlas.clone());
        let mut out = match atlas {
            Some(a) => (a.textures)(&names[..names.len().min(MAX_NAMES)]),
            None => Vec::new(),
        };
        out.resize(names.len(), None);
        out
    }
    pub fn resources_version() -> u64 {
        super::__host::with(|s| s.atlas.as_ref().map_or(0, |a| a.version))
    }
}

/// `events.registration`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Registration {
    pub commands: Vec<String>,
    pub tick: bool,
    pub frame: bool,
    pub chat: bool,
}

/// The game's calls into a mod. Only the hooks [`Registration`] asks for
/// are called; the others may stay as their defaults.
pub trait Mod: Sized + 'static {
    fn init() -> Result<(Self, Registration), String>;
    fn tick(&mut self, _tick: u64) {}
    fn frame(&mut self, _seconds: f32) {}
    fn command(&mut self, _name: &str, _args: &str) -> Result<Option<String>, String> {
        Err("This command does nothing".into())
    }
    fn chat(&mut self, _text: &str) {}
}

impl Registration {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn command(mut self, name: &str) -> Self {
        self.commands.push(name.into());
        self
    }
    pub fn on_tick(mut self) -> Self {
        self.tick = true;
        self
    }
    pub fn on_frame(mut self) -> Self {
        self.frame = true;
        self
    }
    pub fn on_chat(mut self) -> Self {
        self.chat = true;
        self
    }
}

pub fn info(message: &str) {
    log::write(log::Level::Info, message);
}
pub fn warn(message: &str) {
    log::write(log::Level::Warn, message);
}

/// Exports a [`Mod`]: its calls, for the game's glue (`__host::Exports`).
#[macro_export]
macro_rules! export_mod {
    ($t:ty) => {
        ::std::thread_local! {
            static __MOD_STATE: ::std::cell::RefCell<Option<$t>> = const { ::std::cell::RefCell::new(None) };
        }
        fn __mod_with(f: impl FnOnce(&mut $t)) {
            __MOD_STATE.with(|s| {
                if let Some(m) = s.borrow_mut().as_mut() {
                    f(m)
                }
            })
        }
        /// The mod's calls, for the game's glue.
        pub static __EXPORTS: $crate::__host::Exports = $crate::__host::Exports {
            init: || {
                let (m, registration) = <$t as $crate::Mod>::init()?;
                __MOD_STATE.with(|s| *s.borrow_mut() = Some(m));
                Ok(registration)
            },
            tick: |tick| __mod_with(|m| $crate::Mod::tick(m, tick)),
            frame: |seconds| __mod_with(|m| $crate::Mod::frame(m, seconds)),
            command: |name, args| {
                let mut out = Err("The mod is not running".to_string());
                __mod_with(|m| out = $crate::Mod::command(m, name, args));
                out
            },
            chat: |text| __mod_with(|m| $crate::Mod::chat(m, text)),
        };
        /// Mod ABI 4: the game's one call into a prebuilt SDK mod.
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_sdk_entry(call: &$crate::wire::SdkCall) {
            $crate::wire::serve(&__EXPORTS, call)
        }
    };
}

/// The host side: what the game's glue sets before a call and takes after it (the fork's runtime `State`).
pub mod __host {
    use super::{game, hud, input, ui, world};
    use std::{cell::RefCell, path::PathBuf, sync::Arc};

    pub const HUD_LINES: usize = 16;
    pub const POST_SOURCE: usize = 256 << 10;
    pub const POST_PARAMS: usize = 16;
    pub const POST_DATA_WORDS: usize = 1 << 18;
    pub const RAYCAST_MAX: f32 = 1024.0;
    pub const FLYING_SPEED_MAX: f32 = 50.0;

    /// The mod's calls.
    pub struct Exports {
        pub init: fn() -> Result<super::Registration, String>,
        pub tick: fn(u64),
        pub frame: fn(f32),
        pub command: fn(&str, &str) -> Result<Option<String>, String>,
        pub chat: fn(&str),
    }

    /// The game's answers to `atlas`, given by the glue (and again when the resources load anew): lookups over the
    /// pack's resources, answered at once, and the resources' version.
    #[derive(Clone)]
    pub struct Atlas {
        pub blocks: Arc<dyn Fn(&[String]) -> Vec<Option<super::atlas::BlockFaces>>>,
        pub textures: Arc<dyn Fn(&[String]) -> Vec<Option<super::atlas::Sprite>>>,
        pub version: u64,
    }

    /// What mods may read of the game, copied once per tick by the game.
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
    pub struct Snapshot {
        pub player: Option<game::Player>,
        pub clock: Option<game::Clock>,
        pub held: Option<input::HeldItem>,
        /// The first block along the view within `RAYCAST_MAX`, and its distance.
        pub view_hit: Option<(f32, world::Hit)>,
        /// Whether the last `carry_player` held the player (true when none was asked: the next holds).
        pub carried: bool,
        /// Whether the local player flies (creative flight, or a spectator).
        pub flying: bool,
    }

    /// A keybind the mod registered.
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub struct ModKey {
        pub name: String,
        pub label: String,
        pub key: String,
    }

    /// What the mod asked the game to do since the last `drain`.
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    pub enum Output {
        Chat { name: String, text: String },
        Screen { open: bool },
        Commands { commands: Vec<String> },
        Edit { edit: world::BlockEdit },
        Surface { tag: u32, columns: Vec<(i32, i32)>, heightmap: world::Heightmap },
        Columns { tag: u32, columns: Vec<(i32, i32)>, heightmap: world::Heightmap, below: u32, above: u32 },
        Keys,
        Sound { event: String, at: Option<[f64; 3]>, volume: f32, pitch: f32 },
        Shake { strength: f32, seconds: f32 },
        /// `world.push-player`: added to the local player's velocity, blocks a tick.
        Push { velocity: [f32; 3] },
        /// `world.carry-player`: the local player moved by `delta` this tick, their view turned by `turn` degrees.
        Carry { delta: [f32; 3], turn: f32 },
    }

    #[derive(Default)]
    pub struct State {
        pub name: String,
        pub id: String,
        pub snapshot: Snapshot,
        pub out: Vec<Output>,
        pub panel: Option<(hud::Corner, Vec<hud::Line>)>,
        pub data_dir: PathBuf,
        pub claims: Vec<String>,
        pub keys: Vec<ModKey>,
        pub events: Vec<input::Event>,
        pub typed: Vec<input::Typed>,
        pub screen: Option<Vec<ui::Draw>>,
        pub cursor: Option<ui::Point>,
        pub post_source: Option<(Arc<str>, u64)>,
        pub post_params: Option<[f32; POST_PARAMS]>,
        pub post_data: Option<(Arc<[u32]>, u64)>,
        pub surface: Vec<world::SurfaceHeights>,
        pub columns: Vec<world::ColumnBlocks>,
        pub flying_speed: Option<f32>,
        pub atlas: Option<Atlas>,
        /// Log lines since the last call's answer (level 1 debug .. 4 error), for the game's log.
        pub logs: Vec<(u8, String)>,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    /// The mod's state, for a call or for the glue.
    pub fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
        STATE.with(|s| f(&mut s.borrow_mut()))
    }
}

/// Prebuilt SDK mods (mod ABI 4, docs/android-mods.md): a mod written against this SDK, built as a library for the
/// game's target, is driven by the game through one C call, `horizon_sdk_entry`, which carries a [`wire::Request`]
/// and answers a [`wire::Reply`], both postcard-encoded. Everything a mod draws, plays or changes (its post pass and
/// its data, its sounds, its screen, its HUD, its camera shake, its block edits) crosses as data, so a new mod needs
/// no change to the game.
pub mod wire {
    use super::{__host, hud, input, ui, world, Registration};
    use serde::{Deserialize, Serialize};
    use std::ffi::c_void;

    /// The SDK's wire version: raised when [`Request`] or [`Reply`] change.
    pub const VERSION: u32 = 1;
    /// The symbol an SDK mod exports: `extern "C" fn(&SdkCall)`.
    pub const ENTRY: &str = "horizon_sdk_entry";

    pub type ReplyFn = extern "C" fn(ctx: *mut c_void, bytes: *const u8, len: usize);
    /// The game's block texture lookups (`atlas`): kind 0 blocks, 1 textures; names in, answers out, postcard.
    pub type AtlasFn =
        extern "C" fn(ctx: *mut c_void, kind: u32, req: *const u8, len: usize, reply: ReplyFn, reply_ctx: *mut c_void);

    #[repr(C)]
    pub struct SdkCall {
        pub version: u32,
        pub req: *const u8,
        pub len: usize,
        pub reply: ReplyFn,
        pub reply_ctx: *mut c_void,
        pub atlas: AtlasFn,
        pub atlas_ctx: *mut c_void,
        /// The resources' version the atlas answers for.
        pub atlas_version: u64,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Call {
        Init { id: String, name: String, data_dir: String },
        Tick(u64),
        Frame(f32),
        Command { name: String, args: String },
        Chat(String),
        /// Only the input and the answer.
        Sync,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Default)]
    pub struct Input {
        pub snapshot: Option<__host::Snapshot>,
        pub events: Vec<input::Event>,
        pub typed: Vec<input::Typed>,
        /// Set: the cursor on the mod's screen (none when it isn't shown).
        pub cursor: Option<Option<ui::Point>>,
        pub surface: Vec<world::SurfaceHeights>,
        pub columns: Vec<world::ColumnBlocks>,
        /// The game closed the mod's screen.
        pub screen_closed: bool,
        /// The post pass the game holds: its source's hash and its data's version.
        pub post_have: (u64, u64),
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Request {
        pub call: Call,
        pub input: Input,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Default)]
    pub struct Post {
        pub hash: u64,
        /// The source, when the game's `post_have` hash isn't this one.
        pub source: Option<String>,
        pub params: Option<[f32; __host::POST_PARAMS]>,
        pub data_version: u64,
        /// The words, when the game's `post_have` version isn't this one.
        pub data: Option<Vec<u32>>,
        /// Data was cleared.
        pub no_data: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Default)]
    pub struct Reply {
        pub init: Option<Result<Registration, String>>,
        pub command: Option<Result<Option<String>, String>>,
        pub out: Vec<__host::Output>,
        pub logs: Vec<(u8, String)>,
        pub panel: Option<(hud::Corner, Vec<hud::Line>)>,
        pub screen: Option<Vec<ui::Draw>>,
        pub claims: Vec<String>,
        pub keys: Vec<__host::ModKey>,
        pub flying_speed: Option<f32>,
        /// None: no post pass.
        pub post: Option<Post>,
        /// The mod panicked in this call.
        pub panicked: bool,
    }

    std::thread_local! {
        static ATLAS: std::cell::Cell<Option<(AtlasFn, usize)>> = const { std::cell::Cell::new(None) };
    }

    extern "C" fn take(ctx: *mut c_void, bytes: *const u8, len: usize) {
        // SAFETY: `ctx` is the caller's Vec, `bytes` live for the call.
        unsafe { (*ctx.cast::<Vec<u8>>()).extend_from_slice(std::slice::from_raw_parts(bytes, len)) };
    }

    fn lookup<T: for<'a> Deserialize<'a>>(kind: u32, names: &[String]) -> Vec<Option<T>> {
        let Some((f, ctx)) = ATLAS.with(std::cell::Cell::get) else { return Vec::new() };
        let Ok(req) = postcard::to_allocvec(names) else { return Vec::new() };
        let mut out: Vec<u8> = Vec::new();
        f(ctx as *mut c_void, kind, req.as_ptr(), req.len(), take, (&raw mut out).cast());
        postcard::from_bytes(&out).unwrap_or_default()
    }

    /// The mod's side of `horizon_sdk_entry` (made by `export_mod!`).
    pub fn serve(exports: &__host::Exports, call: &SdkCall) {
        let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(exports, call))).unwrap_or_else(|_| {
            __host::with(|s| s.out.clear());
            Reply { panicked: true, ..Reply::default() }
        });
        if let Ok(bytes) = postcard::to_allocvec(&reply) {
            (call.reply)(call.reply_ctx, bytes.as_ptr(), bytes.len());
        }
    }

    fn run(exports: &__host::Exports, call: &SdkCall) -> Reply {
        if call.version != VERSION || call.req.is_null() {
            return Reply { panicked: true, ..Reply::default() };
        }
        // SAFETY: the game's bytes, alive for the call.
        let bytes = unsafe { std::slice::from_raw_parts(call.req, call.len) };
        let Ok(req) = postcard::from_bytes::<Request>(bytes) else {
            return Reply { panicked: true, ..Reply::default() };
        };
        ATLAS.with(|a| a.set(Some((call.atlas, call.atlas_ctx as usize))));
        let version = call.atlas_version;
        let have = req.input.post_have;
        __host::with(|s| {
            if s.atlas.as_ref().is_none_or(|a| a.version != version) {
                s.atlas = Some(__host::Atlas {
                    blocks: std::sync::Arc::new(|names: &[String]| lookup(0, names)),
                    textures: std::sync::Arc::new(|names: &[String]| lookup(1, names)),
                    version,
                });
            }
            let i = req.input;
            if let Some(snapshot) = i.snapshot {
                s.snapshot = snapshot;
            }
            if i.screen_closed {
                s.screen = None;
                s.typed.clear();
            }
            s.events.extend(i.events);
            s.typed.extend(i.typed);
            if let Some(c) = i.cursor {
                s.cursor = c;
            }
            s.surface.extend(i.surface);
            s.columns.extend(i.columns);
        });
        let mut reply = Reply::default();
        match req.call {
            Call::Init { id, name, data_dir } => {
                __host::with(|s| {
                    s.id = id;
                    s.name = name;
                    s.data_dir = data_dir.into();
                });
                reply.init = Some((exports.init)());
            }
            Call::Tick(t) => (exports.tick)(t),
            Call::Frame(f) => (exports.frame)(f),
            Call::Command { name, args } => reply.command = Some((exports.command)(&name, &args)),
            Call::Chat(t) => (exports.chat)(&t),
            Call::Sync => {}
        }
        ATLAS.with(|a| a.set(None));
        __host::with(|s| {
            reply.out = std::mem::take(&mut s.out);
            reply.logs = std::mem::take(&mut s.logs);
            reply.panel = s.panel.clone();
            reply.screen = s.screen.clone();
            reply.claims = s.claims.clone();
            reply.keys = s.keys.clone();
            reply.flying_speed = s.flying_speed;
            reply.post = s.post_source.as_ref().map(|(src, hash)| Post {
                hash: *hash,
                source: (*hash != have.0).then(|| src.to_string()),
                params: s.post_params,
                data_version: s.post_data.as_ref().map_or(0, |(_, v)| *v),
                data: s.post_data.as_ref().filter(|(_, v)| *v != have.1).map(|(d, _)| d.to_vec()),
                no_data: s.post_data.is_none(),
            });
        });
        reply
    }
}

/// The game's side: opening an SDK mod's library and calling it.
#[cfg(feature = "host")]
pub mod host {
    use super::wire::{self, Reply, Request, SdkCall};
    use std::ffi::c_void;

    pub struct SdkLib {
        _lib: libloading::Library,
        entry: extern "C" fn(&SdkCall),
    }

    /// The game's atlas lookups for one call: kind (0 blocks, 1 textures), names' bytes in, answer's bytes out.
    pub type Lookup<'a> = &'a dyn Fn(u32, &[u8]) -> Vec<u8>;

    extern "C" fn take(ctx: *mut c_void, bytes: *const u8, len: usize) {
        // SAFETY: `ctx` is the caller's Vec, `bytes` live for the call.
        unsafe { (*ctx.cast::<Vec<u8>>()).extend_from_slice(std::slice::from_raw_parts(bytes, len)) };
    }
    extern "C" fn atlas(
        ctx: *mut c_void,
        kind: u32,
        req: *const u8,
        len: usize,
        reply: wire::ReplyFn,
        reply_ctx: *mut c_void,
    ) {
        // SAFETY: `ctx` is the `Lookup` of the call in progress; `req` the mod's live bytes.
        let (f, bytes) = unsafe { (&*ctx.cast::<Lookup<'_>>(), std::slice::from_raw_parts(req, len)) };
        let out = f(kind, bytes);
        reply(reply_ctx, out.as_ptr(), out.len());
    }

    impl SdkLib {
        /// Opens `path` if it exports [`wire::ENTRY`]; `Ok(None)` when it is a plain ABI mod.
        ///
        /// # Safety
        /// The library runs with the game's rights.
        pub unsafe fn open(path: &std::path::Path) -> Result<Option<Self>, String> {
            // SAFETY: the caller's promise.
            let lib = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
            // SAFETY: the symbol has this type by the ABI.
            let entry = match unsafe { lib.get::<extern "C" fn(&SdkCall)>(wire::ENTRY.as_bytes()) } {
                Ok(f) => *f,
                Err(_) => return Ok(None),
            };
            Ok(Some(Self { _lib: lib, entry }))
        }

        /// One call; none when the mod's answer can't be read.
        pub fn call(&self, req: &Request, atlas_version: u64, lookup: Lookup<'_>) -> Option<Reply> {
            let bytes = postcard::to_allocvec(req).ok()?;
            let mut out: Vec<u8> = Vec::new();
            let mut l: Lookup<'_> = lookup;
            let call = SdkCall {
                version: wire::VERSION,
                req: bytes.as_ptr(),
                len: bytes.len(),
                reply: take,
                reply_ctx: (&raw mut out).cast(),
                atlas,
                atlas_ctx: (&raw mut l).cast(),
                atlas_version,
            };
            (self.entry)(&call);
            postcard::from_bytes(&out).ok()
        }
    }
}
