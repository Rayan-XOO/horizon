//! Prebuilt SDK mods (mod ABI 4, docs/android-mods.md): a mod written against `horizon-mod-sdk` and built as a library
//! for this game's target, opened from its `.hnmod` where the game can't build mods from source (Android). The game
//! hands it a snapshot and its input each call and does what it asks: its post pass (any WGSL the mod ships, drawn by
//! `horizon_client_render::sdk_post`), its sounds (from its content in the same `.hnmod`), its screen of quads,
//! sprites, items and text, its HUD panel, its camera shake, its keys, the items it claims, the player's push, carry
//! and flying speed, and its world work: commands, direct block edits, surface heights and column reads, done by the
//! server half in the level being ticked. Nothing here is particular to one mod: a new mod needs only its `.hnmod`.
//!
//! Ported from Stink-O/orbital-laser v0.5.10 (MIT, Copyright (c) 2026 Stink-O), native/mixins/orbital/glue.rs: the
//! Stink-O/horizon fork's game/mods.rs host, made generic over any number of mods behind the C boundary.
use super::Game;
use super::native_claimed_items::{NativeHeldItem, NativeHit, NativeModEvent};
use super::native_hooks::NativeShake;
use super::native_world_columns::NativeColumnBlocks;
use super::native_world_edit::{NativeBlockEdit, NativeBlockRun, NativeHeightmap};
use crate::mod_api::NativeScreenView;
use horizon_client::block_sprites::{BlockSprites, FaceSprite, Lookup, SpriteRef};
use horizon_client::mesher::TintSource;
use horizon_client::text::{self, Text};
use horizon_mod_sdk::host::SdkLib;
use horizon_mod_sdk::wire::{Call, Input, Reply, Request};
use horizon_mod_sdk::{__host as host, Registration, atlas, game as g, hud, input, ui, world};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// The operator level of the console, which the server half runs a client's world work at.
const CONSOLE_LEVEL: u8 = 4;
const WORK: &str = "sdk_work";
const ANSWER: &str = "sdk_answer";
/// A message's bytes at most (the channel's 30 000, less room for its header).
const MESSAGE: usize = 28_000;
/// Columns a read or a height query asks in one message (8 bytes each).
const READ_COLUMNS: usize = 3000;

/// The SDK mods' commands (`Registration::commands`), by mod: for the server's command tree.
static COMMANDS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

thread_local! {
    static MODS: RefCell<Vec<SdkMod>> = const { RefCell::new(Vec::new()) };
    static SERVER: RefCell<Server> = RefCell::new(Server::default());
    static SHAKE: RefCell<Shake> = RefCell::new(Shake::default());
}

/// One SDK mod on the window.
pub(super) struct SdkMod {
    id: String,
    name: String,
    title: String,
    lib: Rc<SdkLib>,
    started: Option<bool>,
    ticks: u64,
    registration: Registration,
    client: Client,
    // what the mod keeps, as its last answer gave it
    panel: Option<(hud::Corner, Vec<hud::Line>)>,
    screen: Option<Vec<ui::Draw>>,
    claims: Vec<String>,
    keys: Vec<host::ModKey>,
    flying_speed: Option<f32>,
    post: Option<(Arc<str>, u64)>,
    params: Option<[f32; host::POST_PARAMS]>,
    data: Option<(Arc<[u32]>, u64)>,
    // input waiting for its next call
    pending: Input,
}

#[derive(Default)]
struct Client {
    out: VecDeque<Vec<u8>>,
    screen_open: bool,
    surface: BTreeMap<u32, (Vec<Option<i32>>, Vec<bool>)>,
    columns: BTreeMap<u32, Columns>,
    fly_sent: Option<(Option<f32>, u64)>,
    level: Option<String>,
    leaving: bool,
}

#[derive(Default)]
struct Columns {
    total: usize,
    got: usize,
    span: usize,
    palette: Vec<String>,
    index: BTreeMap<String, u16>,
    bases: Vec<Option<i32>>,
    blocks: Vec<u16>,
    tints: Vec<u32>,
}

/// Opens `lib` as an SDK mod: `Ok(false)` when it isn't one (a plain ABI mod).
pub(super) fn open(id: &str, name: &str, title: &str, lib: &std::path::Path) -> Result<bool, String> {
    if MODS.with(|m| m.borrow().iter().any(|m| m.id == id)) {
        return Ok(true);
    }
    // SAFETY: the player installed the mod and left it on; it runs with the game's rights.
    let Some(sdk) = (unsafe { SdkLib::open(lib) })? else {
        return Ok(false);
    };
    MODS.with(|m| {
        m.borrow_mut().push(SdkMod {
            id: id.to_owned(),
            name: name.to_owned(),
            title: title.to_owned(),
            lib: Rc::new(sdk),
            started: None,
            ticks: 0,
            registration: Registration::default(),
            client: Client::default(),
            panel: None,
            screen: None,
            claims: Vec::new(),
            keys: Vec::new(),
            flying_speed: None,
            post: None,
            params: None,
            data: None,
            pending: Input::default(),
        })
    });
    Ok(true)
}

fn ids() -> Vec<String> {
    MODS.with(|m| m.borrow().iter().map(|m| m.id.clone()).collect())
}
fn with_mod<T>(id: &str, f: impl FnOnce(&mut SdkMod) -> T) -> Option<T> {
    MODS.with(|m| m.borrow_mut().iter_mut().find(|m| m.id == id).map(f))
}

// ---- block textures (`atlas`) -----------------------------------------------------------------------------------

fn sprite(s: SpriteRef) -> atlas::Sprite {
    atlas::Sprite { array: s.array, layer: s.layer, animated: s.animated, cutout: s.cutout }
}
fn face(f: &FaceSprite) -> atlas::Face {
    let [a0, b0, a1, b1] = f.rect;
    let [u0, v0, du_u, du_v, dv_u, dv_v] = f.uv;
    atlas::Face {
        sprite: sprite(f.sprite),
        rect: (a0, b0, a1, b1),
        inset: f.inset,
        uv: (u0, v0, du_u, du_v, dv_u, dv_v),
        tint: match f.tint {
            None => atlas::Tint::None,
            Some(TintSource::Grass) => atlas::Tint::Grass,
            Some(TintSource::Foliage) => atlas::Tint::Foliage,
            Some(TintSource::DryFoliage) => atlas::Tint::DryFoliage,
            Some(TintSource::Water) => atlas::Tint::Water,
            Some(TintSource::Fixed(c)) => atlas::Tint::Fixed(c),
        },
        translucent: f.translucent,
    }
}
fn block_faces(b: BlockSprites) -> atlas::BlockFaces {
    atlas::BlockFaces {
        sides: b.sides.iter().map(|side| side.iter().map(face).collect()).collect(),
        particle: b.particle.map(sprite),
        cube: b.cube,
    }
}

// ---- calling a mod ----------------------------------------------------------------------------------------------

fn held(h: NativeHeldItem) -> input::HeldItem {
    input::HeldItem { id: h.id, count: h.count, slot: h.slot, custom_name: h.custom_name, custom_data: h.custom_data }
}
fn hit(h: NativeHit) -> world::Hit {
    world::Hit { block_x: h.block_x, block_y: h.block_y, block_z: h.block_z, x: h.x, y: h.y, z: h.z, block: h.block }
}

/// The middle of the GUI, in GUI pixels: the origin of `ui.show` and `ui.cursor`.
fn gui_centre(game: &Game<'_>) -> [f32; 2] {
    let size = game.renderer.as_ref().map_or([1, 1], |r| [r.config.width, r.config.height]);
    let b = horizon_client::hud::HudBatch::new(size);
    [b.width / 2.0, b.height / 2.0]
}

fn snapshot(game: &mut Game<'_>) -> host::Snapshot {
    host::Snapshot {
        player: game.native_player().map(|p| g::Player {
            x: p.x,
            y: p.y,
            z: p.z,
            yaw: p.yaw,
            pitch: p.pitch,
            health: 20.0,
            food: 20,
            dimension: p.dimension.to_owned(),
        }),
        clock: game
            .native_clock()
            .map(|c| g::Clock { game_time: c.game_time.max(0) as u64, day_time: c.day_time.max(0) as u64 }),
        held: game.native_held().map(|(id, count, _)| input::HeldItem {
            id,
            count,
            slot: game.selected as u32,
            custom_name: None,
            custom_data: None,
        }),
        view_hit: game.native_view_hit().map(|(d, h)| (d, hit(h))),
        carried: horizon_client::sdk_carry::carried(),
        flying: game.player.as_ref().is_some_and(|p| p.flying || p.spectator),
    }
}

/// Calls mod `id` with `call`, then does what it asked. None when it isn't running.
fn call(game: &mut Game<'_>, id: &str, call: Call) -> Option<Reply> {
    // (a frame reads the tick's snapshot and input: the view's raycast and the item events stay the tick's)
    let frame = matches!(call, Call::Frame(_));
    let snap = (!frame).then(|| snapshot(game));
    let events: Vec<input::Event> = if frame { Vec::new() } else { game
        .native_poll(id)
        .into_iter()
        .map(|e| match e {
            NativeModEvent::UseStart(item, ticks) => input::Event::UseStart(input::ItemUse { item: held(item), ticks }),
            NativeModEvent::UseStop(item, ticks) => input::Event::UseStop(input::ItemUse { item: held(item), ticks }),
        })
        .collect() };
    let shown = game.native_mod_screen() == Some(id);
    let cursor = shown.then(|| {
        let [cx, cy] = gui_centre(game);
        let [x, y] = game.native_cursor();
        ui::Point { x: x - cx, y: y - cy }
    });
    let (lib, req) = with_mod(id, |m| {
        let mut input = if frame { Input::default() } else { std::mem::take(&mut m.pending) };
        // the mod's screen closed by the game (Escape, another screen)
        if m.client.screen_open && !shown {
            m.client.screen_open = false;
            input.events.push(input::Event::ScreenClosed);
            input.screen_closed = true;
        }
        let mut all = events;
        all.append(&mut input.events);
        input.events = all;
        input.snapshot = snap;
        input.cursor = Some(cursor);
        input.post_have = (m.post.as_ref().map_or(0, |(_, h)| *h), m.data.as_ref().map_or(0, |(_, v)| *v));
        (m.lib.clone(), Request { call, input })
    })?;
    // the game's resources, for the mod's atlas lookups in this call
    let split = game.renderer.as_ref().map_or(u32::MAX, |r| r.sprite_split());
    let version = game.native_block_sprites_version();
    let context = game.pack.context.clone();
    let lookup = move |kind: u32, names: &[u8]| -> Vec<u8> {
        let names: Vec<String> = postcard::from_bytes(names).unwrap_or_default();
        let names = &names[..names.len().min(4096)];
        let l = Lookup::new(&context, split);
        if kind == 0 {
            let out: Vec<Option<atlas::BlockFaces>> = names.iter().map(|n| l.block(n).map(block_faces)).collect();
            postcard::to_allocvec(&out).unwrap_or_default()
        } else {
            let out: Vec<Option<atlas::Sprite>> = names.iter().map(|n| l.texture(n).map(sprite)).collect();
            postcard::to_allocvec(&out).unwrap_or_default()
        }
    };
    let Some(reply) = lib.call(&req, version, &lookup) else {
        log::warn!("[mod {id}] its answer can't be read");
        return None;
    };
    for (level, line) in &reply.logs {
        match level {
            1 => log::error!("[mod {id}] {line}"),
            2 => log::warn!("[mod {id}] {line}"),
            _ => log::info!("[mod {id}] {line}"),
        }
    }
    if reply.panicked {
        log::warn!("[mod {id}] panicked: stopped");
        with_mod(id, |m| m.started = Some(false));
        horizon_client_render::sdk_post::set(id, None);
        return None;
    }
    with_mod(id, |m| {
        m.panel = reply.panel.clone();
        m.screen = reply.screen.clone();
        m.claims = reply.claims.clone();
        m.keys = reply.keys.clone();
        m.flying_speed = reply.flying_speed;
        match &reply.post {
            None => {
                m.post = None;
                m.params = None;
                m.data = None;
            }
            Some(p) => {
                if let Some(src) = &p.source {
                    m.post = Some((Arc::from(src.as_str()), p.hash));
                }
                m.params = p.params;
                if let Some(d) = &p.data {
                    m.data = Some((Arc::from(&d[..]), p.data_version));
                } else if p.no_data {
                    m.data = None;
                }
            }
        }
    });
    show_output(game, id, &reply);
    Some(reply)
}

/// Starts mod `id` once: whether it runs.
fn running(game: &mut Game<'_>, id: &str) -> bool {
    if !game.native_window() || !game.native_on(id) {
        return false;
    }
    let Some((started, name)) = with_mod(id, |m| (m.started, m.name.clone())) else {
        return false;
    };
    if let Some(on) = started {
        return on;
    }
    with_mod(id, |m| m.started = Some(false));
    let data_dir = game.native_data_dir(id).to_string_lossy().into_owned();
    let reply = call(game, id, Call::Init { id: id.to_owned(), name, data_dir });
    let on = match reply.and_then(|r| r.init) {
        Some(Ok(registration)) => {
            if let Ok(mut c) = COMMANDS.lock() {
                c.retain(|(m, _)| m != id);
                c.extend(registration.commands.iter().map(|n| (id.to_owned(), n.clone())));
            }
            with_mod(id, |m| m.registration = registration);
            true
        }
        Some(Err(e)) => {
            log::warn!("mod {id} did not start: {e}");
            false
        }
        None => false,
    };
    with_mod(id, |m| m.started = Some(on));
    on
}

/// What a call asked for.
fn show_output(game: &mut Game<'_>, id: &str, reply: &Reply) {
    let out_opened = reply.out.iter().any(|o| *o == host::Output::Screen { open: true });
    let leaving = with_mod(id, |m| m.client.leaving).unwrap_or(false);
    for output in reply.out.iter().cloned() {
        match output {
            host::Output::Sound { event, at, volume, pitch } => {
                if game.native_window() {
                    game.audio.play(&event, at, volume, pitch);
                }
            }
            host::Output::Commands { commands } if leaving => {
                for event in commands.iter().filter_map(|c| c.strip_prefix("stopsound @s * ")) {
                    game.audio.stop_sounds(Some(event.trim()), None);
                }
            }
            host::Output::Edit { .. } | host::Output::Surface { .. } | host::Output::Columns { .. } if leaving => {}
            host::Output::Commands { commands } => send_commands(id, &commands),
            host::Output::Chat { name, text: t } => {
                let mut line = Text::literal(format!("[{name}] ")).with_color(text::GRAY);
                line.push(Text::literal(t));
                chat(game, line);
            }
            host::Output::Keys => {}
            host::Output::Edit { edit } => send_edit(id, &edit),
            host::Output::Surface { tag, columns, heightmap: h } => send_surface(id, tag, &columns, heightmap(h)),
            host::Output::Columns { tag, columns, heightmap: h, below, above } => {
                send_columns(id, tag, &columns, heightmap(h), below, above)
            }
            host::Output::Shake { strength, seconds } => {
                let now = (game.ticks as f64) / 20.0;
                SHAKE.with(|s| s.borrow_mut().add(now, strength, seconds));
            }
            host::Output::Push { velocity } => {
                if !horizon_client::sdk_carry::push(velocity) {
                    game.native_push_player(velocity);
                }
            }
            host::Output::Carry { delta, turn } => horizon_client::sdk_carry::ask(delta, turn),
            host::Output::Screen { open: false } => {
                game.native_mod_screen_close(id);
                with_mod(id, |m| m.client.screen_open = false);
            }
            host::Output::Screen { open: true } => {}
        }
    }
    if reply.screen.is_some() {
        let open = if out_opened { game.native_mod_screen_open(id) } else { game.native_mod_screen() == Some(id) };
        if open {
            with_mod(id, |m| m.client.screen_open = true);
        } else {
            // another screen is up, or the game closed it: the mod hears it closed on its next call
            with_mod(id, |m| {
                if out_opened {
                    m.pending.events.push(input::Event::ScreenClosed);
                }
                m.pending.screen_closed = true;
                m.screen = None;
            });
        }
    }
    let (fly, claims) = with_mod(id, |m| (m.flying_speed, m.claims.clone())).unwrap_or_default();
    sync_flying_speed(game, id, fly);
    game.native_claim_items(id, &claims);
    let day_time = game.native_clock().map_or(0, |c| c.day_time.max(0) as u64);
    let pass = with_mod(id, |m| match (&m.post, m.params) {
        (Some((source, version)), Some(params)) => Some(horizon_client_render::sdk_post::Pass {
            source: source.clone(),
            version: *version,
            params,
            data: m.data.clone(),
            day_time,
        }),
        _ => None,
    })
    .flatten();
    horizon_client_render::sdk_post::set(id, pass);
}

fn sync_flying_speed(game: &mut Game<'_>, id: &str, want: Option<f32>) {
    let ticks = u64::from(game.ticks);
    let have = game.player.as_ref().map(|p| p.flying_speed);
    let spectator = game.player.as_ref().is_some_and(|p| p.spectator);
    with_mod(id, |m| {
        let c = &mut m.client;
        let differs = match want {
            Some(s) => !spectator && have.is_some_and(|h| h != s),
            None => c.fly_sent.is_some_and(|(w, _)| w.is_some()) && have.is_some_and(|h| h != 0.05),
        };
        let changed = c.fly_sent.is_none_or(|(w, _)| w != want);
        let waiting = c.fly_sent.is_some_and(|(_, t)| ticks < t + 20);
        if (changed && (want.is_some() || c.fly_sent.is_some())) || (differs && !waiting) {
            let mut w = Writer::new(4);
            w.f32(want.unwrap_or(f32::NAN));
            c.out.push_back(w.0);
            c.fly_sent = Some((want, ticks));
        }
    });
}

fn chat(game: &mut Game<'_>, line: Text) {
    let focused = game.screen == super::Screen::Chat;
    game.ui.chat.add(&game.gui.atlas, line, None, horizon_client::chat::ChatSource::Client, game.ticks, focused);
}

fn heightmap(h: world::Heightmap) -> u8 {
    match h {
        world::Heightmap::WorldSurface => 1,
        world::Heightmap::MotionBlocking => 2,
        world::Heightmap::MotionBlockingNoLeaves => 3,
        world::Heightmap::OceanFloor => 4,
    }
}
fn heightmap_of(h: u8) -> Option<NativeHeightmap> {
    Some(match h {
        1 => NativeHeightmap::WorldSurface,
        2 => NativeHeightmap::MotionBlocking,
        3 => NativeHeightmap::MotionBlockingNoLeaves,
        4 => NativeHeightmap::OceanFloor,
        _ => return None,
    })
}

// ---- the wire between the halves ----------------------------------------------------------------------------------

struct Writer(Vec<u8>);
impl Writer {
    fn new(kind: u8) -> Self {
        Self(vec![kind])
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn strs(&mut self, v: &[String]) {
        self.u32(v.len() as u32);
        for s in v {
            self.str(s);
        }
    }
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes(b.try_into().unwrap()))
    }
    fn i32(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_le_bytes(b.try_into().unwrap()))
    }
    fn f32(&mut self) -> Option<f32> {
        self.take(4).map(|b| f32::from_le_bytes(b.try_into().unwrap()))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        self.take(n).and_then(|b| String::from_utf8(b.to_vec()).ok())
    }
    fn strs(&mut self) -> Option<Vec<String>> {
        let n = self.u32()? as usize;
        (0..n).map(|_| self.str()).collect()
    }
    fn columns(&mut self) -> Option<Vec<(i32, i32)>> {
        let n = self.u32()? as usize;
        (0..n).map(|_| Some((self.i32()?, self.i32()?))).collect()
    }
}

fn send(id: &str, message: Vec<u8>) {
    with_mod(id, |m| m.client.out.push_back(message));
}
fn send_commands(id: &str, commands: &[String]) {
    let mut lines: Vec<&String> = Vec::new();
    let flush = |lines: &mut Vec<&String>| {
        let mut w = Writer::new(0);
        w.u32(lines.len() as u32);
        for l in lines.drain(..) {
            w.str(l);
        }
        send(id, w.0);
    };
    let mut size = 0;
    for line in commands {
        if !lines.is_empty() && size + line.len() + 4 > MESSAGE {
            flush(&mut lines);
            size = 0;
        }
        size += line.len() + 4;
        lines.push(line);
    }
    if !lines.is_empty() {
        flush(&mut lines);
    }
}
fn send_edit(id: &str, edit: &world::BlockEdit) {
    let head = 9 + edit.palette.iter().chain(&edit.filters).map(|s| s.len() + 4).sum::<usize>();
    let per = ((MESSAGE.saturating_sub(head)) / 25).max(1);
    for runs in edit.runs.chunks(per) {
        let mut w = Writer::new(1);
        w.strs(&edit.palette);
        w.strs(&edit.filters);
        w.u32(runs.len() as u32);
        for r in runs {
            w.i32(r.x);
            w.i32(r.z);
            w.i32(r.y);
            w.u32(r.height);
            w.0.push(r.surface.map_or(0, heightmap));
            w.u32(r.block);
            w.i32(r.only.map_or(-1, |o| o as i32));
        }
        send(id, w.0);
    }
}
fn send_surface(id: &str, tag: u32, columns: &[(i32, i32)], h: u8) {
    let parts: Vec<&[(i32, i32)]> = if columns.is_empty() { vec![&[]] } else { columns.chunks(READ_COLUMNS).collect() };
    for (i, part) in parts.into_iter().enumerate() {
        let mut w = Writer::new(2);
        w.u32(tag);
        w.0.push(h);
        w.u32((i * READ_COLUMNS) as u32);
        w.u32(columns.len() as u32);
        w.u32(part.len() as u32);
        for &(x, z) in part {
            w.i32(x);
            w.i32(z);
        }
        send(id, w.0);
    }
}
fn send_columns(id: &str, tag: u32, columns: &[(i32, i32)], h: u8, below: u32, above: u32) {
    let parts: Vec<&[(i32, i32)]> = if columns.is_empty() { vec![&[]] } else { columns.chunks(READ_COLUMNS).collect() };
    for (i, part) in parts.into_iter().enumerate() {
        let mut w = Writer::new(3);
        w.u32(tag);
        w.0.push(h);
        w.u32(below);
        w.u32(above);
        w.u32((i * READ_COLUMNS) as u32);
        w.u32(columns.len() as u32);
        w.u32(part.len() as u32);
        for &(x, z) in part {
            w.i32(x);
            w.i32(z);
        }
        send(id, w.0);
    }
}

fn flush_work(game: &mut Game<'_>, id: &str) {
    let channel = format!("{id}:{WORK}");
    loop {
        let Some(batch) = with_mod(id, |m| {
            let c = &m.client;
            if c.out.is_empty() {
                return None;
            }
            let mut batch = vec![9u8];
            let mut taken = 0;
            for msg in c.out.iter() {
                if taken > 0 && batch.len() + 4 + msg.len() > MESSAGE {
                    break;
                }
                batch.extend_from_slice(&(msg.len() as u32).to_le_bytes());
                batch.extend_from_slice(msg);
                taken += 1;
            }
            Some((batch, taken, c.out.len()))
        })
        .flatten() else {
            return;
        };
        let (bytes, taken, waiting) = batch;
        if let Err(e) = game.native_net_send_to_server(&channel, &bytes) {
            if waiting > 512 {
                log::debug!("[mod {id}] world work waiting: {e}");
            }
            return;
        }
        with_mod(id, |m| {
            m.client.out.drain(..taken);
        });
    }
}

/// The `net_from_server` hook: answers from the server half.
pub(super) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    let Some((id, ANSWER)) = channel.split_once(':') else { return };
    if !game.native_window() {
        return;
    }
    let id = id.to_owned();
    let mut r = Reader(data);
    let Some(kind) = r.u8() else { return };
    let done = with_mod(&id, |m| -> Option<()> {
        match kind {
            10 => {
                let (tag, offset, total) = (r.u32()?, r.u32()? as usize, r.u32()? as usize);
                let n = r.u32()? as usize;
                let entry = m.client.surface.entry(tag).or_insert_with(|| (vec![None; total], vec![false; total]));
                for i in 0..n {
                    let has = r.u8()?;
                    let h = r.i32()?;
                    if let Some(slot) = entry.0.get_mut(offset + i) {
                        *slot = (has == 1).then_some(h);
                        entry.1[offset + i] = true;
                    }
                }
                if entry.1.iter().all(|&b| b) {
                    let (heights, _) = m.client.surface.remove(&tag)?;
                    m.pending.surface.push(world::SurfaceHeights { tag, heights });
                }
            }
            11 => {
                let (tag, offset, total, span) = (r.u32()?, r.u32()? as usize, r.u32()? as usize, r.u32()? as usize);
                let palette = r.strs()?;
                let n = r.u32()? as usize;
                let entry = m.client.columns.entry(tag).or_insert_with(|| Columns {
                    total,
                    span,
                    bases: vec![None; total],
                    blocks: vec![0; total * span],
                    tints: vec![0; total * 3],
                    ..Columns::default()
                });
                let map: Vec<u16> = palette
                    .into_iter()
                    .map(|name| {
                        if let Some(&i) = entry.index.get(&name) {
                            return i;
                        }
                        let i = entry.palette.len() as u16;
                        entry.palette.push(name.clone());
                        entry.index.insert(name, i);
                        i
                    })
                    .collect();
                for i in 0..n {
                    let k = offset + i;
                    let has = r.u8()?;
                    let base = r.i32()?;
                    if k < entry.total {
                        entry.bases[k] = (has == 1).then_some(base);
                    }
                    for j in 0..entry.span {
                        let b = r.u16()? as usize;
                        if k < entry.total {
                            entry.blocks[k * entry.span + j] = map.get(b).copied().unwrap_or(0);
                        }
                    }
                    for t in 0..3 {
                        let v = r.u32()?;
                        if k < entry.total {
                            entry.tints[k * 3 + t] = v;
                        }
                    }
                }
                entry.got += n;
                if entry.got >= entry.total {
                    let e = m.client.columns.remove(&tag)?;
                    m.pending.columns.push(world::ColumnBlocks {
                        tag,
                        palette: e.palette,
                        bases: e.bases,
                        blocks: e.blocks,
                        tints: e.tints,
                    });
                }
            }
            _ => {}
        }
        Some(())
    });
    if done == Some(None) {
        log::warn!("[mod {id}] a bad answer from the server half");
    }
}

// ---- the server half ----------------------------------------------------------------------------------------------

#[derive(Default)]
struct Server {
    /// Each (mod, player)'s requests, in order.
    work: BTreeMap<(String, u128), VecDeque<Vec<u8>>>,
    /// Answers waiting for the channel.
    answers: BTreeMap<(String, u128), VecDeque<Vec<u8>>>,
}

/// The `net_from_client` hook: a player's request for an SDK mod, kept for the tick of their level.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    let Some((id, WORK)) = channel.split_once(':') else { return };
    if game.native_window() || !game.native_on(id) {
        return;
    }
    SERVER.with(|s| s.borrow_mut().work.entry((id.to_owned(), player)).or_default().push_back(data.to_vec()));
}

fn uuid(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// The `level_tick_post` hook: the requests of the players in this level, and the answers that fit.
pub(super) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if game.native_window() {
        return;
    }
    let has_work = SERVER.with(|s| {
        let s = s.borrow();
        !s.work.is_empty() || !s.answers.is_empty()
    });
    if !has_work {
        return;
    }
    let players = game.native_server_players();
    let mine: Vec<(u128, u8)> =
        players.iter().filter(|p| p.dimension == dimension).map(|p| (p.uuid, p.permission)).collect();
    let keys: Vec<(String, u128)> = SERVER.with(|s| s.borrow().work.keys().cloned().collect());
    for (id, player) in keys {
        let Some(&(_, permission)) = mine.iter().find(|(p, _)| *p == player) else { continue };
        let work = SERVER.with(|s| s.borrow_mut().work.remove(&(id.clone(), player))).unwrap_or_default();
        if !game.native_on(&id) {
            continue;
        }
        // (what a client sends is its own input, run as the console: only for a player at the console's level)
        if permission < CONSOLE_LEVEL {
            static SAID: std::sync::Once = std::sync::Once::new();
            SAID.call_once(|| {
                log::warn!("[mod {id}] world work from {} left out: not a level {CONSOLE_LEVEL} operator (turn cheats on)", uuid(player))
            });
            continue;
        }
        for batch in work {
            let mut r = Reader(&batch);
            if r.u8() != Some(9) {
                continue;
            }
            while let Some(n) = r.u32() {
                let Some(request) = r.take(n as usize) else { break };
                if serve(game, &id, player, dimension, request).is_none() {
                    log::warn!("[mod {id}] a bad request from {}", uuid(player));
                }
            }
        }
    }
    let keys: Vec<(String, u128)> = SERVER.with(|s| s.borrow().answers.keys().cloned().collect());
    for key in keys {
        let channel = format!("{}:{ANSWER}", key.0);
        loop {
            let Some(m) = SERVER.with(|s| s.borrow_mut().answers.get_mut(&key).and_then(|q| q.pop_front())) else {
                break;
            };
            if m.len() > 30_000 {
                continue;
            }
            if game.native_net_send_to_player(key.1, &channel, &m).is_err() {
                SERVER.with(|s| s.borrow_mut().answers.entry(key.clone()).or_default().push_front(m));
                break;
            }
        }
    }
    SERVER.with(|s| s.borrow_mut().answers.retain(|_, q| !q.is_empty()));
}

fn answer(id: &str, player: u128, message: Vec<u8>) {
    SERVER.with(|s| s.borrow_mut().answers.entry((id.to_owned(), player)).or_default().push_back(message));
}

fn serve(game: &mut Game<'_>, id: &str, player: u128, dimension: &str, request: &[u8]) -> Option<()> {
    let mut r = Reader(request);
    match r.u8()? {
        0 => {
            let who = uuid(player);
            for line in r.strs()? {
                game.native_run_command(&format!("execute as {who} at @s run {}", line.trim_start_matches('/')));
            }
        }
        1 => {
            let palette = r.strs()?;
            let filters = r.strs()?;
            let n = r.u32()? as usize;
            let mut runs = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                let (x, z, y, height) = (r.i32()?, r.i32()?, r.i32()?, r.u32()?);
                let surface = heightmap_of(r.u8()?);
                let block = r.u32()?;
                let only = r.i32()?;
                runs.push(NativeBlockRun { x, z, y, height, surface, block, only: u32::try_from(only).ok() });
            }
            if let Err(e) = game.native_edit_blocks(dimension, &NativeBlockEdit { palette, filters, runs }) {
                log::warn!("[mod {id}] edit-blocks left out: {e}");
            }
        }
        2 => {
            let (tag, h, offset, total) = (r.u32()?, r.u8()?, r.u32()?, r.u32()?);
            let columns = r.columns()?;
            let heights = game
                .native_surface_heights(dimension, &columns, heightmap_of(h)?)
                .unwrap_or_else(|_| vec![None; columns.len()]);
            if heights.is_empty() {
                let mut w = Writer::new(10);
                w.u32(tag);
                w.u32(offset);
                w.u32(total);
                w.u32(0);
                answer(id, player, w.0);
            }
            for (k, part) in heights.chunks(MESSAGE / 5).enumerate() {
                let mut w = Writer::new(10);
                w.u32(tag);
                w.u32(offset + (k * (MESSAGE / 5)) as u32);
                w.u32(total);
                w.u32(part.len() as u32);
                for h in part {
                    w.0.push(u8::from(h.is_some()));
                    w.i32(h.unwrap_or(0));
                }
                answer(id, player, w.0);
            }
        }
        3 => {
            let (tag, h, below, above, offset, total) = (r.u32()?, r.u8()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
            let columns = r.columns()?;
            let span = (below + above) as usize;
            let read = game
                .native_read_columns(dimension, &columns, heightmap_of(h)?, (below, above))
                .unwrap_or_else(|_| NativeColumnBlocks {
                    bases: vec![None; columns.len()],
                    blocks: vec![0; columns.len() * span],
                    tints: vec![0; columns.len() * 3],
                    ..NativeColumnBlocks::default()
                });
            let palette_bytes: usize = read.palette.iter().map(|p| p.len() + 4).sum();
            let per = ((MESSAGE.saturating_sub(palette_bytes + 32)) / (5 + 2 * span + 12)).max(1);
            let starts: Vec<usize> = if columns.is_empty() { vec![0] } else { (0..columns.len()).step_by(per).collect() };
            for start in starts {
                let end = (start + per).min(columns.len());
                let mut w = Writer::new(11);
                w.u32(tag);
                w.u32(offset + start as u32);
                w.u32(total);
                w.u32(span as u32);
                w.strs(&read.palette);
                w.u32((end - start) as u32);
                for i in start..end {
                    let base = read.bases.get(i).copied().flatten();
                    w.0.push(u8::from(base.is_some()));
                    w.i32(base.unwrap_or(0));
                    for j in 0..span {
                        w.0.extend_from_slice(&read.blocks.get(i * span + j).copied().unwrap_or(0).to_le_bytes());
                    }
                    for t in 0..3 {
                        w.u32(read.tints.get(i * 3 + t).copied().unwrap_or(0));
                    }
                }
                answer(id, player, w.0);
            }
        }
        4 => {
            let speed = r.f32()?;
            game.native_set_flying_speed(player, speed.is_finite().then_some(speed));
        }
        _ => return None,
    }
    Some(())
}

// ---- the camera shake (`render.shake`) ---------------------------------------------------------------------------

#[derive(Default)]
struct Shake {
    jolts: Vec<(f64, f32, f32)>,
}
const SHAKE_DEGREES: [f32; 3] = [3.2, 3.6, 2.4];
impl Shake {
    fn add(&mut self, now: f64, strength: f32, seconds: f32) {
        self.jolts.retain(|&(start, secs, _)| now - start < f64::from(secs));
        if self.jolts.len() < 64 {
            self.jolts.push((now, seconds, strength.clamp(0.0, 1.0)));
        }
    }
    fn trauma(&self, now: f64) -> f32 {
        let sum: f32 = self
            .jolts
            .iter()
            .map(|&(start, secs, strength)| {
                let u = ((now - start) as f32 / secs).clamp(0.0, 1.0);
                let left = strength * (1.0 - u) * (1.0 - u);
                if now >= start { left * left } else { 0.0 }
            })
            .sum();
        sum.sqrt().min(1.0)
    }
    fn turn(&self, now: f64) -> Option<[f32; 3]> {
        let amount = self.trauma(now).powi(2);
        if amount <= 0.0 {
            return None;
        }
        let t = now as f32;
        let slow = 1.0 - 0.35 * amount;
        let axis = |k: f32| {
            (shake_noise(t * 2.1 * slow + k * 17.3) * 0.55
                + shake_noise(t * 6.3 * slow + k * 31.7) * 0.35
                + shake_noise(t * 13.0 + k * 53.1) * 0.1)
                * 1.6
        };
        let [p, y, r] = SHAKE_DEGREES;
        Some([axis(0.0) * p * amount, axis(1.0) * y * amount, axis(2.0) * r * amount])
    }
}
fn shake_noise(x: f32) -> f32 {
    let grad = |i: f32| {
        let h = (i * 127.1).sin() * 43_758.547;
        (h - h.floor()) * 2.0 - 1.0
    };
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = grad(i) * f;
    let b = grad(i + 1.0) * (f - 1.0);
    a + (b - a) * u
}

// ---- the hooks ----------------------------------------------------------------------------------------------------

impl Game<'_> {
    /// `native_camera_shake`: the SDK mods' shake.
    pub(super) fn sdk_camera_shake(&self, partial: f32, shake: &mut NativeShake) {
        if !self.native_window() {
            return;
        }
        let now = (self.ticks as f64 + f64::from(partial)) / 20.0;
        if let Some([p, y, r]) = SHAKE.with(|s| s.borrow().turn(now)) {
            shake.pitch += p;
            shake.yaw += y;
            shake.roll += r;
        }
    }

    /// `native_mods`: each SDK mod's row on Options > Mods, with its keys.
    pub(super) fn sdk_mods(&mut self, mods: &mut Vec<crate::mod_api::NativeMod>) {
        for id in ids() {
            self.sdk_running(&id);
            let Some((title, keys)) = with_mod(&id, |m| {
                (m.title.clone(), m.keys.iter().map(|k| [k.name.clone(), k.label.clone(), k.key.clone()]).collect())
            }) else {
                continue;
            };
            mods.retain(|m| m.id != id);
            mods.push(crate::mod_api::NativeMod { id, title, description: String::new(), keys });
        }
    }

    fn sdk_running(&mut self, id: &str) -> bool {
        running(self, id)
    }

    /// `native_key`: a key for the SDK mods' keys bound to it, while playing.
    pub(super) fn sdk_key(&mut self, key: &str, pressed: bool) {
        if self.screen != super::Screen::Playing || self.title.is_some() {
            return;
        }
        for id in ids() {
            if !running(self, &id) {
                continue;
            }
            let keys = with_mod(&id, |m| m.keys.clone()).unwrap_or_default();
            for k in keys {
                if self.native_mod_key(&id, &k.name, &k.key) == key {
                    let event = if pressed { input::Event::KeyDown(k.name) } else { input::Event::KeyUp(k.name) };
                    with_mod(&id, |m| m.pending.events.push(event));
                }
            }
        }
    }

    /// `native_tick`: each SDK mod's tick on the window; on a server, nothing (its half runs in `level_tick_post`).
    pub(super) fn sdk_tick(&mut self) {
        if !self.native_window() {
            return;
        }
        for id in ids() {
            if !running(self, &id) {
                continue;
            }
            horizon_client::sdk_carry::untaken();
            let Some(ticks) = with_mod(&id, |m| {
                if !m.registration.tick {
                    return None;
                }
                m.ticks += 1;
                Some(m.ticks)
            })
            .flatten() else {
                call(self, &id, Call::Sync);
                flush_work(self, &id);
                continue;
            };
            call(self, &id, Call::Tick(ticks));
            flush_work(self, &id);
        }
    }

    /// `native_frame`: each SDK mod's frame.
    pub(super) fn sdk_frame(&mut self, elapsed: std::time::Duration) {
        if !self.native_window() {
            return;
        }
        let turn = horizon_client::sdk_carry::turn();
        if turn != 0.0 && self.screen == super::Screen::Playing && self.player.is_some() {
            self.yaw += turn * elapsed.as_secs_f32().min(0.1) * 20.0;
        }
        for id in ids() {
            let on = with_mod(&id, |m| m.started == Some(true) && m.registration.frame).unwrap_or(false);
            if on && self.native_on(&id) {
                call(self, &id, Call::Frame(elapsed.as_secs_f32()));
            }
        }
    }

    /// `native_command`: a command one of the SDK mods registered; true when it ran it.
    pub(super) fn sdk_command(&mut self, command: &str) -> bool {
        let line = command.trim_start_matches('/');
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        if !self.native_window() {
            return false;
        }
        for id in ids() {
            if !running(self, &id) {
                continue;
            }
            let mine = with_mod(&id, |m| m.registration.commands.iter().any(|c| c == name)).unwrap_or(false);
            if !mine {
                continue;
            }
            let reply = call(self, &id, Call::Command { name: name.to_owned(), args: args.trim_start().to_owned() });
            match reply.and_then(|r| r.command) {
                Some(Ok(Some(t))) => chat(self, Text::literal(t)),
                Some(Err(t)) => chat(self, Text::literal(t).with_color(text::RED)),
                _ => {}
            }
            flush_work(self, &id);
            return true;
        }
        false
    }

    /// `native_hud`: each SDK mod's panel.
    pub(super) fn sdk_hud(&mut self, batch: &mut horizon_client::hud::HudBatch, corners: &mut horizon_client::extras_hud::Corners) {
        if !self.native_window() {
            return;
        }
        use horizon_client::extras::Corner;
        for id in ids() {
            let Some(Some((corner, lines))) = with_mod(&id, |m| if m.started == Some(true) { m.panel.clone() } else { None })
            else {
                continue;
            };
            let corner = match corner {
                hud::Corner::TopLeft => Corner::TopLeft,
                hud::Corner::TopRight => Corner::TopRight,
                hud::Corner::BottomLeft => Corner::BottomLeft,
                hud::Corner::BottomRight => Corner::BottomRight,
            };
            if self.debug && corner == Corner::TopLeft {
                continue;
            }
            let lines: Vec<(&str, u32)> = lines.iter().map(|l| (l.text.as_str(), l.color)).collect();
            horizon_client::extras_hud::draw_lines(batch, &self.gui.atlas, corners, corner, &lines);
        }
    }

    fn sdk_screen_of(&self, screen: &NativeScreenView<'_>) -> Option<String> {
        if screen.id != "mod" {
            return None;
        }
        let shown = self.native_mod_screen()?;
        ids().into_iter().find(|id| id == shown)
    }

    /// `native_screen_render_post`: an SDK mod's screen, from its centre.
    pub(super) fn sdk_screen_render_post(&self, batch: &mut horizon_client::hud::HudBatch, screen: &NativeScreenView<'_>) {
        let Some(id) = self.sdk_screen_of(screen) else { return };
        let Some(Some(draws)) = with_mod(&id, |m| m.screen.clone()) else { return };
        let (cx, cy) = (batch.width / 2.0, batch.height / 2.0);
        for d in &draws {
            match d {
                ui::Draw::Fill((r, c)) => self.native_fill(batch, [cx + r.x, cy + r.y, r.w, r.h], *c),
                ui::Draw::Quad((p, c)) if p.len() >= 4 => {
                    self.native_quad(batch, std::array::from_fn(|i| [cx + p[i].x, cy + p[i].y]), *c)
                }
                ui::Draw::Quad(_) => {}
                ui::Draw::Sprite((name, r, _)) => {
                    let [w, h] = self.native_sprite_size(name).unwrap_or([r.w, r.h]);
                    self.native_sprite(batch, name, [0.0, 0.0, w, h], [cx + r.x, cy + r.y, r.w, r.h]);
                }
                ui::Draw::Item((item, model, at, scale)) => {
                    let at = [cx + at.x, cy + at.y];
                    if model.is_empty() {
                        self.native_item(batch, item, at, 16.0 * scale);
                    } else {
                        self.native_item_model(batch, item, model, at, 16.0 * scale);
                    }
                }
                ui::Draw::Text((t, at, c, centred)) => {
                    let x = if *centred { cx + at.x - (self.native_text_width(t) / 2.0).floor() } else { cx + at.x };
                    self.native_text(batch, t, [x, cy + at.y], *c, true);
                }
            }
        }
    }

    /// `native_screen_mouse_button_pressed_pre`: a click on an SDK mod's screen.
    pub(super) fn sdk_screen_click(&self, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
        let Some(id) = self.sdk_screen_of(screen) else { return };
        if !(0..=2).contains(&button) {
            return;
        }
        let [cx, cy] = gui_centre(self);
        let [x, y] = screen.mouse;
        let click = input::Click { x: x - cx, y: y - cy, button: button as u32 };
        with_mod(&id, |m| m.pending.events.push(input::Event::ScreenClick(click)));
        *canceled = true;
    }

    /// `native_screen_key_pressed_pre`: a key on an SDK mod's screen that types no text; Escape stays the game's.
    pub(super) fn sdk_screen_key(&self, screen: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
        let Some(id) = self.sdk_screen_of(screen) else { return };
        if key == "key.keyboard.escape" {
            return;
        }
        let name = key.strip_prefix("key.keyboard.").unwrap_or(key);
        let types_text = name.chars().count() == 1 || name == "space" || name.starts_with("keypad.") && name.len() == 8;
        if !types_text {
            with_mod(&id, |m| m.pending.typed.push(input::Typed::Key(key.to_owned())));
        }
        *canceled = true;
    }

    /// `native_screen_char_typed_pre`: a character typed on an SDK mod's screen.
    pub(super) fn sdk_screen_char(&self, screen: &NativeScreenView<'_>, c: char, canceled: &mut bool) {
        let Some(id) = self.sdk_screen_of(screen) else { return };
        with_mod(&id, |m| m.pending.typed.push(input::Typed::Text(c.to_string())));
        *canceled = true;
    }

    /// `native_register_commands`: the SDK mods' commands in the server's tree (the window's `native_command` runs them).
    pub(super) fn sdk_register_commands(&mut self, commands: &mut Vec<crate::mod_api::NativeCommand>) {
        let names = COMMANDS.lock().map(|c| c.clone()).unwrap_or_default();
        for (id, name) in names {
            if self.native_on(&id) {
                commands.push(crate::mod_api::NativeCommand::op(&format!("{name} [args:text]"), CONSOLE_LEVEL));
            }
        }
    }

    /// `native_client_level_load`.
    pub(super) fn sdk_client_level_load(&mut self, dimension: &str) {
        for id in ids() {
            with_mod(&id, |m| m.client.level = Some(dimension.to_owned()));
        }
    }

    /// `native_client_level_unload`: on leaving the world, each SDK mod hears `left-world` on its first command (as
    /// the fork told it), its waiting world work is dropped, and every sound stops.
    pub(super) fn sdk_client_level_unload(&mut self, dimension: &str) {
        if !self.native_window() {
            return;
        }
        let mut any = false;
        for id in ids() {
            let left = with_mod(&id, |m| {
                let left = m.client.level.as_deref() == Some(dimension);
                if left {
                    m.client.level = None;
                }
                left
            })
            .unwrap_or(false);
            if !left {
                continue;
            }
            any = true;
            let (started, first) = with_mod(&id, |m| (m.started, m.registration.commands.first().cloned())).unwrap_or_default();
            if started == Some(true) {
                with_mod(&id, |m| m.client.leaving = true);
                if let Some(name) = first {
                    call(self, &id, Call::Command { name, args: "left-world".into() });
                }
                with_mod(&id, |m| {
                    m.client.leaving = false;
                    m.client.out.clear();
                    m.client.surface.clear();
                    m.client.columns.clear();
                });
            }
        }
        if any {
            horizon_client::sdk_carry::reset();
            self.audio.stop_all();
        }
    }
}
