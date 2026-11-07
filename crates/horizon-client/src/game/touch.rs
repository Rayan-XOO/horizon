//! Touch controls (Android; ANDROID.md), laid out like Bedrock Edition's.
//!
//! The default layout (`Preset::Dpad`) is Bedrock's classic one: a D-pad of
//! square arrow buttons bottom left (the forward diagonals show while forward
//! is held, sneak in its centre, a double tap on forward sprints), jump bottom
//! right, pause and chat at the top, and the hotbar's "..." button.
//! Aiming follows `Aim`: `Touch` (Bedrock's Touch mode) breaks and uses at the
//! touched point with a hold-progress ring around the finger; `Crosshair`
//! (Bedrock's split controls) acts at the centre crosshair. The joystick
//! layouts stay as presets. Every icon is drawn here from small pixel grids.
//!
//! In menus a finger is the mouse (down = left click, moving drags). In the
//! world the controls are drawn over the HUD (`draw_touch_controls`): a
//! joystick, jump, sneak, sprint (a toggle), attack and
//! use (held; the same finger turns the view), pause, chat, keyboard, inventory,
//! drop, F1/F2/F3/F5 and the layout editor (the gear). Hotbar slots select
//! on tap. Anywhere else turns the view; a short tap there attacks the
//! entity under the crosshair or else uses (right click), and a finger held
//! still breaks (left click), as on Bedrock. Buttons go through the
//! controller bindings (the virtual pad `horizon-ctl pad` uses).
//!
//! The editor moves (drag), resizes and fades each control, hides or shows
//! it, and saves to `touch_layout.json` in the game directory.
//!
//! A focused text field brings up the soft keyboard (`android_ime`).
use super::event_loop::EventLoopCtl;
use super::Game;
use horizon_client::{
    gamepad::Button,
    hud::{HudAtlas, HudBatch, rgba},
    osk,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase, WindowEvent},
    event_loop::ActiveEventLoop,
};

/// Held this long without moving, a look finger breaks blocks.
const HOLD_MS: u128 = 280;
/// Moved further than this (GUI pixels), a look finger is no tap or hold.
const SLOP: f64 = 4.0;
/// 0.4.3 starts on the D-pad layout: a new file, the old one stays untouched.
const LAYOUT_FILE: &str = "touch_layout_v2.json";

/// What a finger is doing, from where it went down.
#[derive(Clone, Copy, Debug)]
enum Finger {
    /// Acting as the mouse in a menu.
    Pointer,
    /// The joystick; the knob's position (GUI pixels).
    Stick,
    /// On the D-pad (`Id::Dpad`).
    Dpad,
    /// Turning the view; tap = attack an entity or use, hold = attack.
    Look {
        last: [f64; 2],
        start: [f64; 2],
        since: Instant,
        moved: bool,
        attacking: bool,
    },
    /// Holding a controller button; with pass-through on (the bool), moving it turns the view.
    Press(Button, [f64; 2], bool),
    /// Holding a trigger (0 = use, 1 = attack) from its button; moving the
    /// same finger turns the view like `Look`, and the trigger stays held
    /// until it lifts, wherever it slides.
    Trigger { i: usize, last: [f64; 2], since: Instant, look: bool },
    /// Dragging a control in the editor, from this offset to its centre.
    Drag(Id, [f32; 2]),
    /// Took over a hotbar slot or the keyboard; nothing until it lifts.
    Done,
}

/// Every control there is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Id {
    Stick,
    Dpad,
    AimToggle,
    Mount,
    F8,
    Jump,
    Sneak,
    Sprint,
    Attack,
    Use,
    Pause,
    Chat,
    Keyboard,
    Inventory,
    Drop,
    F1,
    F2,
    F3,
    F5,
    Edit,
}

const IDS: [Id; 20] = [
    Id::Stick,
    Id::Dpad,
    Id::AimToggle,
    Id::Mount,
    Id::F8,
    Id::Jump,
    Id::Sneak,
    Id::Sprint,
    Id::Attack,
    Id::Use,
    Id::Pause,
    Id::Chat,
    Id::Keyboard,
    Id::Inventory,
    Id::Drop,
    Id::F1,
    Id::F2,
    Id::F3,
    Id::F5,
    Id::Edit,
];

/// One control's place: centre as a fraction of the GUI, radius in GUI px.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Spec {
    x: f32,
    y: f32,
    size: f32,
    opacity: f32,
    visible: bool,
    /// This button's own choice of whether its finger turns the view (`None`: the
    /// layout's `pass_through`).
    #[serde(default)]
    look: Option<bool>,
}

/// Which set of controls: the classic one with attack and use buttons, Bedrock
/// Edition's (tap and hold on the screen, auto-sprint on the stick), or the
/// player's own from the editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Preset {
    #[default]
    Standard,
    Bedrock,
    Custom,
    /// Bedrock's D-pad (the default).
    Dpad,
}

/// Where taps and holds act (Bedrock's Touch and split controls).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Aim {
    /// At the touched point; no crosshair.
    #[default]
    Touch,
    /// At the centre crosshair.
    Crosshair,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TouchLayout {
    /// Multiplies every control's opacity.
    opacity: f32,
    controls: HashMap<Id, Spec>,
    #[serde(default)]
    preset: Preset,
    /// A finger that went down on a button also turns the view when it moves.
    /// Off: buttons keep their finger to themselves.
    #[serde(default = "yes")]
    pass_through: bool,
    #[serde(default)]
    aim: Aim,
}

impl Default for TouchLayout {
    fn default() -> Self {
        Self::dpad()
    }
}

impl TouchLayout {
    /// Bedrock's D-pad layout on a 560 x 270 GUI: the pad bottom left (`size` is half a
    /// cell), jump bottom right, pause and chat top centre, "..." by the hotbar.
    fn dpad() -> Self {
        let (w, h) = (560.0f32, 270.0f32);
        let at = |x: f32, y: f32, size: f32| Spec { x: x / w, y: y / h, size, opacity: 1.0, visible: true, look: None };
        let off = |x: f32, y: f32, size: f32| Spec { visible: false, ..at(x, y, size) };
        let bar = (w / 2.0 - 91.0).floor() + 182.0;
        let controls = HashMap::from([
            (Id::Dpad, at(64.0, h - 64.0, 13.0)),
            (Id::Stick, off(56.0, h - 60.0, 34.0)),
            (Id::Jump, at(w - 44.0, h - 52.0, 16.0)),
            (Id::Sneak, off(w - 86.0, h - 24.0, 12.0)),
            (Id::Sprint, at(w - 44.0, h - 92.0, 11.0)),
            (Id::Attack, at(w - 82.0, h - 70.0, 13.0)),
            (Id::Use, off(w - 80.0, h - 80.0, 14.0)),
            (Id::Mount, at(w - 78.0, h - 104.0, 10.0)),
            (Id::AimToggle, at(w - 14.0, 40.0, 9.0)),
            (Id::Chat, at(w / 2.0 - 48.0, 12.0, 9.0)),
            (Id::Drop, at(w / 2.0 - 24.0, 12.0, 9.0)),
            (Id::Pause, at(w / 2.0, 12.0, 9.0)),
            (Id::F1, at(w / 2.0 + 24.0, 12.0, 9.0)),
            (Id::F8, at(w / 2.0 + 48.0, 12.0, 9.0)),
            (Id::Keyboard, off(w / 2.0 + 72.0, 12.0, 9.0)),
            (Id::Inventory, at(bar + 12.0, h - 11.0, 10.0)),
            (Id::F2, off(31.0, 12.0, 8.0)),
            (Id::F3, off(50.0, 12.0, 8.0)),
            (Id::F5, at(w - 14.0, 12.0, 9.0)),
            (Id::Edit, at(w - 36.0, 12.0, 8.0)),
        ]);
        Self { opacity: 0.85, controls, preset: Preset::Dpad, pass_through: true, aim: Aim::Touch }
    }

    /// Bedrock Edition's touch layout: no attack or use buttons (tap to use or hit,
    /// hold to break), jump and sneak bottom right, pause and chat top centre,
    /// the camera switch top right; pushing the stick all the way forward sprints.
    fn bedrock() -> Self {
        let (w, h) = (560.0f32, 270.0f32);
        let at = |x: f32, y: f32, size: f32| Spec { x: x / w, y: y / h, size, opacity: 1.0, visible: true, look: None };
        let off = |x: f32, y: f32, size: f32| Spec { visible: false, ..at(x, y, size) };
        let bar = (w / 2.0 - 91.0).floor() + 182.0;
        let mut layout = Self::dpad();
        let controls = &mut layout.controls;
        controls.insert(Id::Dpad, off(64.0, h - 64.0, 13.0));
        controls.insert(Id::Stick, at(60.0, h - 62.0, 34.0));
        controls.insert(Id::Sneak, at(w - 86.0, h - 24.0, 12.0));
        let _ = bar;
        let controls = layout.controls;
        Self { opacity: 0.85, controls, preset: Preset::Bedrock, pass_through: true, aim: Aim::Touch }
    }

    /// Laid out on a 560 x 270 GUI (a 20:9 phone at GUI scale 4).
    fn standard() -> Self {
        let (w, h) = (560.0f32, 270.0f32);
        let at = |x: f32, y: f32, size: f32| Spec { x: x / w, y: y / h, size, opacity: 1.0, visible: true, look: None };
        let bar = (w / 2.0 - 91.0).floor() + 182.0;
        let controls = HashMap::from([
            (Id::Stick, at(44.0, h - 48.0, 32.0)),
            (Id::Dpad, Spec { visible: false, ..at(62.0, h - 62.0, 13.0) }),
            (Id::AimToggle, Spec { visible: false, ..at(w - 14.0, 40.0, 9.0) }),
            (Id::Mount, Spec { visible: false, ..at(w - 78.0, h - 104.0, 10.0) }),
            (Id::F8, Spec { visible: false, ..at(109.0, 12.0, 8.0) }),
            (Id::Jump, at(w - 30.0, h - 42.0, 18.0)),
            (Id::Sneak, at(w - 72.0, h - 22.0, 13.0)),
            // Up and right of the joystick, clear of it and the hotbar.
            (Id::Sprint, at(96.0, h - 86.0, 11.0)),
            (Id::Attack, at(w - 34.0, h - 96.0, 15.0)),
            (Id::Use, at(w - 72.0, h - 66.0, 14.0)),
            (Id::Pause, at(w - 12.0, 12.0, 9.0)),
            (Id::Chat, at(w - 34.0, 12.0, 9.0)),
            (Id::Keyboard, at(w - 56.0, 12.0, 9.0)),
            (Id::Inventory, at(bar + 13.0, h - 11.0, 10.0)),
            (Id::Drop, at(bar + 13.0, h - 34.0, 10.0)),
            (Id::F1, at(12.0, 12.0, 8.0)),
            (Id::F2, at(31.0, 12.0, 8.0)),
            (Id::F3, at(50.0, 12.0, 8.0)),
            (Id::F5, at(69.0, 12.0, 8.0)),
            (Id::Edit, at(90.0, 12.0, 8.0)),
        ]);
        Self { opacity: 0.85, controls, preset: Preset::Standard, pass_through: true, aim: Aim::Crosshair }
    }

    fn of(preset: Preset) -> Self {
        match preset {
            Preset::Bedrock => Self::bedrock(),
            Preset::Standard => Self::standard(),
            _ => Self::dpad(),
        }
    }

    fn load() -> Self {
        let mut layout = Self::default();
        if let Ok(text) = std::fs::read_to_string(LAYOUT_FILE)
            && let Ok(saved) = serde_json::from_str::<TouchLayout>(&text)
        {
            // A file from before presets has only controls: it is the player's own.
            let preset = if text.contains("\"preset\"") { saved.preset } else { Preset::Custom };
            if preset != Preset::Custom {
                layout = Self::of(preset);
            } else {
                layout.controls.extend(saved.controls);
            }
            layout.preset = preset;
            layout.opacity = saved.opacity.clamp(0.1, 1.0);
            layout.pass_through = saved.pass_through;
            if text.contains("\"aim\"") {
                layout.aim = saved.aim;
            }
        }
        layout
    }
    fn save(&self) {
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(LAYOUT_FILE, text) {
                    log::warn!("Horizon: saving {LAYOUT_FILE}: {e}");
                }
            }
            Err(e) => log::warn!("Horizon: {LAYOUT_FILE}: {e}"),
        }
    }
    /// Whether a finger on `id` also turns the view.
    fn looks(&self, id: Option<Id>) -> bool {
        id.and_then(|id| self.controls.get(&id)?.look).unwrap_or(self.pass_through)
    }
    fn spec(&self, id: Id) -> Spec {
        self.controls.get(&id).copied().unwrap_or(Spec { x: 0.5, y: 0.5, size: 10.0, opacity: 1.0, visible: true, look: None })
    }
}

/// The editor's panel buttons.
#[derive(Clone, Copy, PartialEq, Debug)]
enum EditAction {
    SizeDown,
    SizeUp,
    FadeDown,
    FadeUp,
    AllFadeDown,
    AllFadeUp,
    Toggle,
    Reset,
    Save,
    Standard,
    Bedrock,
    Custom,
    PassThrough,
    Dpad,
    Aim,
}

/// The second row: the layout and how buttons share fingers with the camera.
const PRESET_ACTIONS: [(EditAction, &str); 6] = [
    (EditAction::Dpad, "D-pad"),
    (EditAction::Bedrock, "Joystick"),
    (EditAction::Standard, "Buttons"),
    (EditAction::Custom, "Custom"),
    (EditAction::Aim, "Aim: touch"),
    (EditAction::PassThrough, "Look: on"),
];

const EDIT_ACTIONS: [(EditAction, &str); 9] = [
    (EditAction::SizeDown, "Size-"),
    (EditAction::SizeUp, "Size+"),
    (EditAction::FadeDown, "Alpha-"),
    (EditAction::FadeUp, "Alpha+"),
    (EditAction::AllFadeDown, "All-"),
    (EditAction::AllFadeUp, "All+"),
    (EditAction::Toggle, "Hide"),
    (EditAction::Reset, "Reset"),
    (EditAction::Save, "Save"),
];

pub(super) struct Touches {
    fingers: HashMap<u64, Finger>,
    /// The joystick knob's offset from the centre, -1..1.
    knob: [f32; 2],
    /// The D-pad's held direction (x right, y forward), if any.
    dpad: Option<[i8; 2]>,
    /// When forward was last let go on the D-pad (a second tap soon after sprints).
    forward_up: Option<Instant>,
    /// Touch aiming: the point acted at, -1..1 across the screen (y up).
    aim_at: Option<[f32; 2]>,
    /// Frames the aim point stays after a tap (so the tick acts there).
    aim_frames: u8,
    /// Frames until the Mount button's dismount sneak lets go.
    unsneak_frames: u8,
    /// Frames until tapped triggers are released (use, attack).
    trigger_release: [u8; 2],
    /// Sneak toggled on (Bedrock's sneak button toggles).
    sneaking: bool,
    /// Sprint toggled on; off again when the joystick is let go.
    sprinting: bool,
    layout: TouchLayout,
    /// The control the last finger went down on (for its own pass-through choice).
    pressing: Option<Id>,
    /// The layout editor is open; the selected control.
    editing: bool,
    selected: Option<Id>,
    /// The editor changed a control since it opened (saving makes it Custom).
    dirty: bool,
    /// Where the finger now going down on a button is (for pass-through).
    press_at: [f64; 2],
    /// The soft keyboard was asked for.
    ime_shown: bool,
    /// A menu tap since the last frame: ask for the keyboard again if a field wants it.
    ime_retap: bool,
    /// A controller, keyboard or mouse plays: the controls are hidden until a touch.
    pub(super) hardware: bool,
    /// A mouse has been used: the pointer is captured while playing.
    pub(super) mouse: bool,
    /// The pointer capture last asked for.
    pub(super) pointer_captured: Option<bool>,
}

impl Default for Touches {
    fn default() -> Self {
        Self {
            fingers: HashMap::new(),
            knob: [0.0; 2],
            dpad: None,
            forward_up: None,
            aim_at: None,
            aim_frames: 0,
            unsneak_frames: 0,
            trigger_release: [0; 2],
            sneaking: false,
            sprinting: false,
            layout: if cfg!(target_os = "android") { TouchLayout::load() } else { TouchLayout::default() },
            pressing: None,
            editing: false,
            selected: None,
            dirty: false,
            press_at: [0.0; 2],
            ime_shown: false,
            ime_retap: false,
            hardware: false,
            mouse: false,
            pointer_captured: None,
        }
    }
}

/// The GUI's size, for placing controls.
#[derive(Clone, Copy)]
struct Gui {
    w: f32,
    h: f32,
}

impl Gui {
    fn place(&self, layout: &TouchLayout, id: Id) -> ([f32; 2], f32) {
        let s = layout.spec(id);
        ([s.x * self.w, s.y * self.h], s.size)
    }
    fn hotbar_x(&self) -> f32 {
        (self.w / 2.0 - 91.0).floor()
    }
    fn hotbar_slot(&self, p: [f32; 2]) -> Option<usize> {
        let x = p[0] - self.hotbar_x();
        (p[1] >= self.h - 23.0 && (0.0..182.0).contains(&x)).then(|| ((x / 20.0) as usize).min(8))
    }
    /// The close (back) button over menus and containers, top right.
    fn close(&self) -> ([f32; 2], f32) {
        ([self.w - 12.0, 12.0], 9.0)
    }
    /// The profile button (name, skin) on the title and pause screens, top left.
    fn profile(&self) -> ([f32; 2], f32) {
        ([12.0, 12.0], 9.0)
    }
    fn profile_hit(&self, p: [f32; 2]) -> bool {
        let (c, r) = self.profile();
        (p[0] - c[0]).hypot(p[1] - c[1]) <= r + 5.0
    }
    fn close_hit(&self, p: [f32; 2]) -> bool {
        let (c, r) = self.close();
        (p[0] - c[0]).hypot(p[1] - c[1]) <= r + 5.0
    }
    /// The control under `p`; hidden ones only in the editor.
    fn hit(&self, layout: &TouchLayout, p: [f32; 2], editing: bool) -> Option<Id> {
        IDS.into_iter()
            .filter(|&id| editing || layout.spec(id).visible)
            .filter_map(|id| {
                let (c, r) = self.place(layout, id);
                if id == Id::Dpad {
                    // Three cells across, each two half-cells and a gap.
                    let reach = dpad_cell(r) * 1.5;
                    let d = (p[0] - c[0]).abs().max((p[1] - c[1]).abs());
                    return (d <= reach).then_some((id, d - reach));
                }
                let d = (p[0] - c[0]).hypot(p[1] - c[1]);
                let reach = if id == Id::Stick && !editing { r * 1.6 } else { r + 4.0 };
                (d <= reach).then_some((id, d - r))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
    /// The editor's panel: a row of buttons a third of the way down.
    fn panel(&self) -> impl Iterator<Item = (EditAction, &'static str, [f32; 4])> {
        let bw = 44.0f32;
        let gap = 3.0;
        let total = EDIT_ACTIONS.len() as f32 * (bw + gap) - gap;
        let x0 = ((self.w - total) / 2.0).floor();
        let y = (self.h * 0.32).floor();
        let pw = 56.0f32;
        let ptotal = PRESET_ACTIONS.len() as f32 * (pw + gap) - gap;
        let px0 = ((self.w - ptotal) / 2.0).floor();
        EDIT_ACTIONS
            .into_iter()
            .enumerate()
            .map(move |(i, (a, l))| (a, l, [x0 + i as f32 * (bw + gap), y, bw, 16.0]))
            .chain(
                PRESET_ACTIONS
                    .into_iter()
                    .enumerate()
                    .map(move |(i, (a, l))| (a, l, [px0 + i as f32 * (pw + gap), y + 19.0, pw, 16.0])),
            )
    }
    /// The D-pad cell under `p` (x right, y forward), `None` off the pad.
    fn dpad_at(&self, layout: &TouchLayout, p: [f32; 2]) -> Option<[i8; 2]> {
        let (c, r) = self.place(layout, Id::Dpad);
        let cell = dpad_cell(r);
        let (fx, fy) = ((p[0] - c[0]) / cell, (c[1] - p[1]) / cell);
        if fx.abs() > 1.6 || fy.abs() > 1.6 {
            return None;
        }
        Some([fx.round().clamp(-1.0, 1.0) as i8, fy.round().clamp(-1.0, 1.0) as i8])
    }
    fn panel_hit(&self, p: [f32; 2]) -> Option<EditAction> {
        self.panel()
            .find(|(_, _, r)| p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3])
            .map(|(a, _, _)| a)
    }
}

fn disc(batch: &mut HudBatch, atlas: &HudAtlas, c: [f32; 2], r: f32, color: u32) {
    const N: usize = 24;
    let at = |i: usize| {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    };
    for i in 0..N {
        batch.fill_quad(atlas, [c, at(i), at(i + 1), c], color);
    }
}

fn ring(batch: &mut HudBatch, atlas: &HudAtlas, c: [f32; 2], r: f32, t: f32, color: u32) {
    const N: usize = 32;
    let at = |i: usize, r: f32| {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    };
    for i in 0..N {
        batch.fill_quad(atlas, [at(i, r - t), at(i, r), at(i + 1, r), at(i + 1, r - t)], color);
    }
}

/// `color` with its alpha scaled by `a`.
fn fade(color: u32, a: f32) -> u32 {
    let alpha = ((color >> 24) as f32 * a.clamp(0.0, 1.0)) as u32;
    (alpha << 24) | (color & 0x00FF_FFFF)
}

/// A D-pad cell's pitch for a half-cell of `r`: the square and a gap.
fn dpad_cell(r: f32) -> f32 {
    r * 2.0 + 2.0
}

/// The touch buttons' pixel icons, 16 x 16 (`assets/touch/icons.txt`, drawn for
/// Horizon; not taken from any game's textures).
const ICONS_SRC: &str = include_str!("../../assets/touch/icons.txt");
const ICON_N: usize = 16;

fn icon_rows(name: &str) -> Option<&'static [u16; ICON_N]> {
    static ICONS: std::sync::OnceLock<std::collections::HashMap<&'static str, [u16; ICON_N]>> = std::sync::OnceLock::new();
    ICONS
        .get_or_init(|| {
            let mut map = std::collections::HashMap::new();
            let mut lines = ICONS_SRC.lines().filter(|l| !l.starts_with("# "));
            while let Some(head) = lines.next() {
                let Some(name) = head.strip_prefix("= ") else { continue };
                let mut rows = [0u16; ICON_N];
                for row in rows.iter_mut() {
                    let line = lines.next().unwrap_or("");
                    for (x, b) in line.bytes().take(ICON_N).enumerate() {
                        if b == b'#' {
                            *row |= 1 << x;
                        }
                    }
                }
                map.insert(name.trim(), rows);
            }
            map
        })
        .get(name)
}

/// Icon `name` turned a quarter `turns` times clockwise, drawn about `size` wide at
/// `c` on whole screen pixels, so it scales nearest-neighbour and stays sharp.
fn pixels(batch: &mut HudBatch, atlas: &HudAtlas, name: &str, turns: u8, c: [f32; 2], size: f32, color: u32) {
    let Some(rows) = icon_rows(name) else { return };
    // Whole screen pixels per icon pixel.
    let k = batch.scale.max(1.0);
    let px = (size * k / ICON_N as f32).round().max(1.0) / k;
    let x0 = ((c[0] - px * ICON_N as f32 / 2.0) * k).round() / k;
    let y0 = ((c[1] - px * ICON_N as f32 / 2.0) * k).round() / k;
    for (y, row) in rows.iter().enumerate() {
        for x in 0..ICON_N {
            if row & (1 << x) == 0 {
                continue;
            }
            let (mut gx, mut gy) = (x, y);
            for _ in 0..turns % 4 {
                (gx, gy) = (ICON_N - 1 - gy, gx);
            }
            batch.fill(atlas, x0 + gx as f32 * px, y0 + gy as f32 * px, px, px, color);
        }
    }
}

/// A Bedrock-style square tile on whole pixels: a dark outline with cut corners,
/// translucent grey inside, a light top-left bevel and a darker bottom-right one.
#[allow(clippy::too_many_arguments)]
fn tile(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32, w: f32, h: f32, fill: u32, hi: u32, lo: u32, edge: u32) {
    let (x, y, w, h) = (x.round(), y.round(), w.round(), h.round());
    batch.fill(atlas, x + 1.0, y, w - 2.0, 1.0, edge);
    batch.fill(atlas, x + 1.0, y + h - 1.0, w - 2.0, 1.0, edge);
    batch.fill(atlas, x, y + 1.0, 1.0, h - 2.0, edge);
    batch.fill(atlas, x + w - 1.0, y + 1.0, 1.0, h - 2.0, edge);
    batch.fill(atlas, x + 1.0, y + 1.0, w - 2.0, h - 2.0, fill);
    batch.fill(atlas, x + 1.0, y + 1.0, w - 2.0, 1.0, hi);
    batch.fill(atlas, x + 1.0, y + 2.0, 1.0, h - 3.0, hi);
    batch.fill(atlas, x + 2.0, y + h - 2.0, w - 3.0, 1.0, lo);
    batch.fill(atlas, x + w - 2.0, y + 2.0, 1.0, h - 4.0, lo);
}

/// A Bedrock-style square button of half-size `r` at `c`.
fn square(batch: &mut HudBatch, atlas: &HudAtlas, c: [f32; 2], r: f32, down: bool, a: f32) {
    let r = r.round();
    let fill = if down { rgba(214, 214, 214, 200) } else { rgba(160, 160, 160, 130) };
    let hi = rgba(255, 255, 255, if down { 120 } else { 95 });
    let lo = rgba(40, 40, 40, if down { 60 } else { 90 });
    // Pressed: the bevel flips, as a key pushed in.
    let (hi, lo) = if down { (lo, hi) } else { (hi, lo) };
    let edge = fade(rgba(20, 20, 20, 215), a);
    tile(batch, atlas, c[0] - r, c[1] - r, r * 2.0, r * 2.0, fade(fill, a), fade(hi, a), fade(lo, a), edge);
}

/// The dark pixel colour of icons on the grey tiles.
const INK: u32 = 0xF0_1E_1E_1E;

/// The pixel icon of a control on a square button, if it has one.
fn square_icon(id: Id) -> Option<&'static str> {
    Some(match id {
        Id::Jump => "jump",
        Id::Sneak => "sneak",
        Id::Pause => "pause",
        Id::Chat | Id::Keyboard => "chat",
        Id::Inventory => "dots",
        Id::Attack => "sword",
        Id::Sprint => "sprint",
        Id::AimToggle => "crosshair",
        Id::Mount => "mount",
        Id::Drop => "drop",
        _ => return None,
    })
}

/// A line `t` wide from `a` to `b`.
fn line(batch: &mut HudBatch, atlas: &HudAtlas, a: [f32; 2], b: [f32; 2], t: f32, color: u32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy).max(0.001);
    let (nx, ny) = (-dy / len * t / 2.0, dx / len * t / 2.0);
    batch.fill_quad(
        atlas,
        [[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]],
        color,
    );
}

/// The icon of a control, drawn in a circle of radius `r` at `c`.
fn icon(batch: &mut HudBatch, atlas: &HudAtlas, id: Id, c: [f32; 2], r: f32, color: u32) {
    let s = r * 0.5;
    let (x, y) = (c[0], c[1]);
    let text = |batch: &mut HudBatch, label: &str| {
        let tw = HudBatch::text_width(atlas, label);
        batch.text(atlas, label, (x - tw / 2.0).round(), (y - 3.5).round(), color, true);
    };
    match id {
        Id::Stick | Id::Dpad => {}
        Id::AimToggle => {
            line(batch, atlas, [x, y - s], [x, y + s], 1.0, color);
            line(batch, atlas, [x - s, y], [x + s, y], 1.0, color);
        }
        Id::Mount => batch.fill(atlas, x - s * 0.8, y - s * 0.2, s * 1.6, s * 0.6, color),
        Id::F8 => text(batch, "F8"),
        Id::Jump => batch.fill_quad(atlas, [[x, y - s], [x + s, y + s * 0.6], [x - s, y + s * 0.6], [x, y - s]], color),
        Id::Sneak => {
            batch.fill_quad(atlas, [[x - s, y - s * 0.7], [x + s, y - s * 0.7], [x, y + s * 0.5], [x, y + s * 0.5]], color);
            batch.fill(atlas, x - s, y + s * 0.65, s * 2.0, (s * 0.25).max(1.0), color);
        }
        Id::Sprint => {
            // Two chevrons pointing forward (up): speed.
            let t = (s * 0.3).max(1.2);
            for dy in [-0.45, 0.35] {
                let yy = y + s * dy;
                line(batch, atlas, [x - s * 0.7, yy + s * 0.45], [x, yy - s * 0.25], t, color);
                line(batch, atlas, [x, yy - s * 0.25], [x + s * 0.7, yy + s * 0.45], t, color);
            }
        }
        Id::Attack => {
            // A sword, point up and to the right.
            line(batch, atlas, [x - s * 0.6, y + s * 0.6], [x + s, y - s], (s * 0.35).max(1.5), color);
            line(batch, atlas, [x - s * 0.9, y + s * 0.1], [x - s * 0.1, y + s * 0.9], (s * 0.3).max(1.2), color);
            line(batch, atlas, [x - s * 0.6, y + s * 0.6], [x - s, y + s], (s * 0.3).max(1.2), color);
        }
        Id::Use => {
            // A hand: palm and fingers.
            batch.fill(atlas, x - s * 0.6, y - s * 0.1, s * 1.2, s * 0.9, color);
            for i in 0..4 {
                let fx = x - s * 0.6 + i as f32 * s * 0.32;
                batch.fill(atlas, fx, y - s * 0.85, s * 0.24, s * 0.8, color);
            }
        }
        Id::Pause => {
            batch.fill(atlas, x - s * 0.55, y - s * 0.6, s * 0.4, s * 1.2, color);
            batch.fill(atlas, x + s * 0.15, y - s * 0.6, s * 0.4, s * 1.2, color);
        }
        Id::Chat => {
            batch.fill(atlas, x - s * 0.8, y - s * 0.6, s * 1.6, s * 1.0, color);
            batch.fill_quad(atlas, [[x - s * 0.5, y + s * 0.4], [x, y + s * 0.4], [x - s * 0.6, y + s * 0.9], [x - s * 0.6, y + s * 0.9]], color);
        }
        Id::Keyboard => {
            for row in 0..3 {
                for col in 0..(if row == 2 { 1 } else { 4 }) {
                    let (kw, kx) = if row == 2 { (s * 1.4, x - s * 0.7) } else { (s * 0.3, x - s * 0.85 + col as f32 * s * 0.45) };
                    batch.fill(atlas, kx, y - s * 0.7 + row as f32 * s * 0.5, kw, s * 0.3, color);
                }
            }
        }
        Id::Inventory => {
            for i in -1..=1 {
                disc(batch, atlas, [x + i as f32 * s * 0.6, y], (s * 0.2).max(0.8), color);
            }
        }
        Id::Drop => {
            batch.fill(atlas, x - s * 0.15, y - s * 0.8, s * 0.3, s * 0.8, color);
            batch.fill_quad(atlas, [[x - s * 0.55, y], [x + s * 0.55, y], [x, y + s * 0.7], [x, y + s * 0.7]], color);
        }
        Id::F1 => text(batch, "F1"),
        Id::F2 => text(batch, "F2"),
        Id::F3 => text(batch, "F3"),
        Id::F5 => text(batch, "F5"),
        Id::Edit => {
            ring(batch, atlas, c, s * 0.75, (s * 0.35).max(1.0), color);
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU;
                let (co, si) = (a.cos(), a.sin());
                line(batch, atlas, [x + co * s * 0.6, y + si * s * 0.6], [x + co * s * 1.05, y + si * s * 1.05], (s * 0.35).max(1.0), color);
            }
        }
    }
}

impl Game<'_> {
    fn touch_gui(&self) -> Option<(Gui, f32)> {
        let batch = self.gui_scale()?;
        Some((Gui { w: batch.width, h: batch.height }, batch.scale))
    }

    fn touch_controls_shown(&self) -> bool {
        cfg!(target_os = "android") && self.player.is_some() && !self.pad_in_menu() && !self.touch.hardware
    }

    /// Over a screen in the world (inventory, containers, pause, chat...):
    /// a close button, as Escape.
    fn touch_close_shown(&self) -> bool {
        cfg!(target_os = "android")
            && self.player.is_some()
            && self.title.is_none()
            && self.pad_in_menu()
            && !self.touch_text_focused()
    }

    /// On the title or pause screen: the profile button.
    fn touch_profile_shown(&self) -> bool {
        cfg!(target_os = "android")
            && !self.touch_text_focused()
            && (self.title.as_ref().is_some_and(|t| matches!(t.mode, horizon_client::title::Mode::Main)) || (self.player.is_some() && self.screen == super::Screen::Paused))
    }

    /// A text box has focus (the soft keyboard is wanted).
    fn touch_text_focused(&self) -> bool {
        use super::Screen;
        matches!(self.screen, Screen::Chat | Screen::CommandBlock)
            || self.sign_edit.is_some()
            || self.book.is_some()
            || (self.screen == Screen::Creative && self.creative_screen.tab().search)
            || self.recipe_search_focused()
            || self.station_typing()
            || self.pad_typing()
    }

    /// Escape: closes a screen, or pauses (the controller's Start, tapped).
    pub(super) fn touch_back(&mut self) {
        if self.touch.editing {
            self.touch.editing = false;
            self.touch.selected = None;
            return;
        }
        self.pad.injected.set(Button::Start, true);
        self.pad.taps.push(Button::Start);
    }

    /// Every finger lifted at once (the app went to the background): nothing stays held.
    pub(super) fn touch_release_all(&mut self) {
        self.touch.fingers.clear();
        self.touch.knob = [0.0, 0.0];
        self.touch.dpad = None;
        self.touch.aim_at = None;
        self.touch.trigger_release = [0; 2];
        self.touch.sprinting = false;
        let sneaking = self.touch.sneaking;
        self.pad.injected = Default::default();
        self.pad.injected.set(Button::RightStick, sneaking);
    }

    /// The controls over the HUD, while playing.
    pub(super) fn draw_touch_controls(&self, batch: &mut HudBatch) {
        let gui = Gui { w: batch.width, h: batch.height };
        let atlas = &self.gui.atlas;
        if self.touch_close_shown() {
            let (c, r) = gui.close();
            disc(batch, atlas, c, r, rgba(0, 0, 0, 150));
            ring(batch, atlas, c, r, 1.0, rgba(255, 255, 255, 160));
            line(batch, atlas, [c[0] - 3.5, c[1] - 3.5], [c[0] + 3.5, c[1] + 3.5], 1.5, 0xFFFF_FFFF);
            line(batch, atlas, [c[0] - 3.5, c[1] + 3.5], [c[0] + 3.5, c[1] - 3.5], 1.5, 0xFFFF_FFFF);
        }
        if self.touch_profile_shown() {
            let (c, r) = gui.profile();
            disc(batch, atlas, c, r, rgba(0, 0, 0, 150));
            ring(batch, atlas, c, r, 1.0, rgba(255, 255, 255, 160));
            // A head and shoulders.
            batch.fill(atlas, c[0] - 2.5, c[1] - 5.0, 5.0, 5.0, 0xFFFF_FFFF);
            batch.fill(atlas, c[0] - 4.5, c[1] + 1.0, 9.0, 4.0, 0xFFFF_FFFF);
        }
        if !self.touch_controls_shown() {
            return;
        }
        let t = &self.touch;
        let layout = &t.layout;
        let editing = t.editing;
        if editing {
            batch.fill(atlas, 0.0, 0.0, gui.w, gui.h, rgba(0, 0, 0, 90));
        }
        for id in IDS {
            let spec = layout.spec(id);
            if !spec.visible && !editing {
                continue;
            }
            let mut a = layout.opacity * spec.opacity;
            if !spec.visible {
                a *= 0.35;
            }
            let (c, r) = gui.place(layout, id);
            let squared = layout.preset != Preset::Standard;
            if id == Id::Dpad {
                let cell = dpad_cell(r);
                let held = t.dpad;
                let forward = held.is_some_and(|d| d[1] == 1);
                for (cx, cy) in [(0i8, 1i8), (0, -1), (-1, 0), (1, 0), (0, 0), (-1, 1), (1, 1)] {
                    // The forward diagonals only while forward is held, as on Bedrock.
                    if cy == 1 && cx != 0 && !forward {
                        continue;
                    }
                    let at = [c[0] + f32::from(cx) * cell, c[1] - f32::from(cy) * cell];
                    let down = if (cx, cy) == (0, 0) { t.sneaking } else { held == Some([cx, cy]) };
                    square(batch, atlas, at, r, down, a);
                    let ink = fade(INK, a);
                    match (cx, cy) {
                        (0, 0) => pixels(batch, atlas, "sneak", 0, at, r * 1.2, ink),
                        (0, 1) => pixels(batch, atlas, "arrow_up", 0, at, r * 1.2, ink),
                        (1, 0) => pixels(batch, atlas, "arrow_up", 1, at, r * 1.2, ink),
                        (0, -1) => pixels(batch, atlas, "arrow_up", 2, at, r * 1.2, ink),
                        (-1, 0) => pixels(batch, atlas, "arrow_up", 3, at, r * 1.2, ink),
                        (-1, 1) => pixels(batch, atlas, "arrow_up_left", 0, at, r * 1.2, ink),
                        _ => pixels(batch, atlas, "arrow_up_left", 1, at, r * 1.2, ink),
                    }
                }
            } else if id == Id::Stick && squared {
                // A square base with a square knob.
                square(batch, atlas, c, r, false, a * 0.8);
                // A sunken well the knob travels in.
                let w = (r * 0.78).round();
                let well = [rgba(60, 60, 60, 70), rgba(0, 0, 0, 70), rgba(255, 255, 255, 60), rgba(20, 20, 20, 120)].map(|v| fade(v, a));
                tile(batch, atlas, c[0] - w, c[1] - w, w * 2.0, w * 2.0, well[0], well[1], well[2], well[3]);
                let k = t.knob;
                let knob = [(c[0] + k[0] * r * 0.4).round(), (c[1] - k[1] * r * 0.4).round()];
                let held = t.fingers.values().any(|f| matches!(f, Finger::Stick));
                let kr = (r * 0.38).round();
                square(batch, atlas, knob, kr, held, a);
                // Grip: three short ridges.
                let g = (kr * 0.25).round().max(1.0);
                for i in -1..=1 {
                    let y = (knob[1] + i as f32 * g * 2.0 - g / 2.0).round();
                    batch.fill(atlas, (knob[0] - kr * 0.5).round(), y, kr, g, fade(rgba(30, 30, 30, 120), a));
                }
            } else if squared {
                let down = match id {
                    Id::Sneak => t.sneaking,
                    Id::Sprint => t.sprinting,
                    Id::Jump => self.pad.injected.held(Button::South),
                    Id::Attack => self.pad.injected.triggers[1] > 0.5,
                    Id::AimToggle => layout.aim == Aim::Crosshair,
                    _ => false,
                };
                square(batch, atlas, c, r, down, a);
                let ink = fade(INK, a);
                match square_icon(id) {
                    Some(rows) => pixels(batch, atlas, rows, 0, c, r * 1.2, ink),
                    None => icon(batch, atlas, id, c, r, ink),
                }
            } else if id == Id::Stick {
                disc(batch, atlas, c, r, fade(rgba(0, 0, 0, 80), a));
                ring(batch, atlas, c, r, 1.5, fade(rgba(255, 255, 255, 130), a));
                let k = t.knob;
                let knob = [c[0] + k[0] * r * 0.6, c[1] - k[1] * r * 0.6];
                let held = t.fingers.values().any(|f| matches!(f, Finger::Stick));
                disc(batch, atlas, knob, r * 0.4, fade(rgba(255, 255, 255, if held { 190 } else { 130 }), a));
            } else {
                let down = match id {
                    Id::Sneak => t.sneaking,
                    Id::Sprint => t.sprinting,
                    Id::Jump => self.pad.injected.held(Button::South),
                    Id::Attack => self.pad.injected.triggers[1] > 0.5,
                    Id::Use => self.pad.injected.triggers[0] > 0.5,
                    _ => false,
                };
                disc(batch, atlas, c, r, fade(rgba(0, 0, 0, if down { 160 } else { 95 }), a));
                ring(batch, atlas, c, r, 1.0, fade(rgba(255, 255, 255, 150), a));
                icon(batch, atlas, id, c, r, fade(0xF0FF_FFFF, a));
            }
            if editing && t.selected == Some(id) {
                ring(batch, atlas, c, r + 2.0, 1.5, rgba(255, 220, 60, 255));
            }
        }
        // Touch aiming: Bedrock's ring round a finger held on a block, filling as it breaks.
        if !editing && let Some((_, scale)) = self.touch_gui() {
            for f in t.fingers.values() {
                if let Finger::Look { start, since, moved: false, attacking, .. } = *f {
                    let c = [(start[0] / f64::from(scale)) as f32, (start[1] / f64::from(scale)) as f32];
                    let held = since.elapsed().as_millis() as f32 / HOLD_MS as f32;
                    let frac = if attacking { self.mining.as_ref().map_or(0.0, |m| m.progress) } else { held * 0.15 };
                    if held > 0.4 {
                        // A full white circle round the finger; the break progress
                        // fills it from the centre.
                        ring(batch, atlas, c, 15.0, 3.0, rgba(255, 255, 255, 220));
                        disc(batch, atlas, c, 12.0 * frac.clamp(0.0, 1.0), rgba(255, 255, 255, 70));
                    }
                }
            }
        }
        if editing {
            let title = match t.selected {
                Some(id) => {
                    let s = layout.spec(id);
                    format!(
                        "{id:?}: size {:.0}, alpha {:.0}%{} | all {:.0}%",
                        s.size,
                        s.opacity * 100.0,
                        if s.visible { "" } else { ", hidden" },
                        layout.opacity * 100.0
                    )
                }
                None => format!(
                    "Drag a control; tap one to pick it | all {:.0}% | Look: buttons {} the camera",
                    layout.opacity * 100.0,
                    if layout.pass_through { "also turn" } else { "don't turn" }
                ),
            };
            let y = (gui.h * 0.32).floor() - 12.0;
            let tw = HudBatch::text_width(atlas, &title);
            batch.text(atlas, &title, ((gui.w - tw) / 2.0).floor(), y, 0xFFFF_FFFF, true);
            for (action, label, r) in gui.panel() {
                let mut label = label;
                if action == EditAction::Toggle
                    && t.selected.is_some_and(|id| !layout.spec(id).visible)
                {
                    label = "Show";
                }
                if action == EditAction::PassThrough && !layout.looks(t.selected) {
                    label = "Look: off";
                }
                if action == EditAction::Aim && layout.aim == Aim::Crosshair {
                    label = "Aim: cross";
                }
                let current = matches!(
                    (action, layout.preset),
                    (EditAction::Dpad, Preset::Dpad)
                        | (EditAction::Standard, Preset::Standard)
                        | (EditAction::Bedrock, Preset::Bedrock)
                        | (EditAction::Custom, Preset::Custom)
                );
                let fill = if current { rgba(40, 110, 48, 230) } else { rgba(30, 30, 30, 220) };
                batch.fill(atlas, r[0], r[1], r[2], r[3], fill);
                batch.fill(atlas, r[0], r[1] + r[3] - 1.0, r[2], 1.0, rgba(255, 255, 255, 120));
                let tw = HudBatch::text_width(atlas, label);
                batch.text(atlas, label, (r[0] + (r[2] - tw) / 2.0).floor(), r[1] + 4.0, 0xFFFF_FFFF, true);
            }
        }
    }

    /// Once per loop iteration, before the pads are read: tapped triggers
    /// let go, a finger held still starts breaking, the soft keyboard follows
    /// text focus and its text goes to the focused field.
    pub(super) fn touch_frame(&mut self) {
        for i in 0..2 {
            if self.touch.trigger_release[i] > 0 {
                self.touch.trigger_release[i] -= 1;
                if self.touch.trigger_release[i] == 0 {
                    self.pad.injected.triggers[i] = 0.0;
                }
            }
        }
        if self.touch.unsneak_frames > 0 {
            self.touch.unsneak_frames -= 1;
            if self.touch.unsneak_frames == 0 {
                self.pad.injected.set(Button::RightStick, self.touch.sneaking);
            }
        }
        let touch_aim = self.touch.layout.aim == Aim::Touch;
        let mut attack = false;
        let mut held = [false; 2];
        let mut aim_start = None;
        for finger in self.touch.fingers.values_mut() {
            if let Finger::Look { since, moved, attacking, start, .. } = finger
                && !*moved
                && !*attacking
                && since.elapsed().as_millis() >= HOLD_MS
            {
                *attacking = true;
                aim_start = Some(*start);
            }
            match finger {
                Finger::Look { attacking: true, .. } => attack = true,
                Finger::Trigger { i, .. } => held[*i] = true,
                _ => {}
            }
        }
        held[1] |= attack;
        if touch_aim && let Some(start) = aim_start {
            self.touch.aim_at = self.touch_ndc(start);
        }
        if !attack && self.touch.aim_at.is_some() {
            if self.touch.aim_frames > 0 {
                self.touch.aim_frames -= 1;
            } else {
                self.touch.aim_at = None;
            }
        }
        if !touch_aim {
            self.touch.aim_at = None;
        }
        // Like vanilla, sprint ends when the player stops moving: here when
        // the joystick is let go.
        if self.touch.sprinting && !self.touch.fingers.values().any(|f| matches!(f, Finger::Stick | Finger::Dpad)) {
            self.touch.sprinting = false;
            self.pad.injected.set(Button::LeftStick, false);
        }
        for i in 0..2 {
            if self.touch.trigger_release[i] == 0 {
                self.pad.injected.triggers[i] = if held[i] { 1.0 } else { 0.0 };
            }
        }
        #[cfg(target_os = "android")]
        self.touch_ime();
    }

    #[cfg(target_os = "android")]
    fn touch_ime(&mut self) {
        use super::android_ime::{self, ImeInput};
        let want = self.touch_text_focused()
            || (self.screen == super::Screen::Options && self.ui.options.wants_text())
            || (self.screen == super::Screen::WorldOptions
                && self.world_options.as_ref().and_then(|o| o.rules.as_ref()).is_some_and(|r| r.typing()))
            || (self.screen != super::Screen::Options
                && self.title.as_mut().is_some_and(|t| t.focused_field().is_some()));
        if want != self.touch.ime_shown || (want && self.touch.ime_retap) {
            self.touch.ime_shown = want;
            android_ime::show(want);
        }
        self.touch.ime_retap = false;
        for input in android_ime::take() {
            self.last_input = Instant::now();
            self.pad_osk_output(match input {
                ImeInput::Text(text) => osk::Output::Text(text),
                ImeInput::Backspace => osk::Output::Backspace,
                ImeInput::Enter => osk::Output::Enter,
            });
        }
    }

    fn touch_edit(&mut self, action: EditAction) {
        let t = &mut self.touch;
        if let Some(id) = t.selected
            && let Some(spec) = t.layout.controls.get_mut(&id)
        {
            if matches!(
                action,
                EditAction::SizeDown | EditAction::SizeUp | EditAction::FadeDown | EditAction::FadeUp | EditAction::Toggle
            ) {
                t.dirty = true;
            }
            match action {
                EditAction::SizeDown => spec.size = (spec.size - 1.0).max(5.0),
                EditAction::SizeUp => spec.size = (spec.size + 1.0).min(60.0),
                EditAction::FadeDown => spec.opacity = (spec.opacity - 0.1).max(0.1),
                EditAction::FadeUp => spec.opacity = (spec.opacity + 0.1).min(1.0),
                EditAction::Toggle if id != Id::Edit => spec.visible = !spec.visible,
                _ => {}
            }
        }
        match action {
            EditAction::AllFadeDown => t.layout.opacity = (t.layout.opacity - 0.1).max(0.1),
            EditAction::AllFadeUp => t.layout.opacity = (t.layout.opacity + 0.1).min(1.0),
            EditAction::Reset => {
                let keep = (t.layout.opacity, t.layout.pass_through);
                t.layout = TouchLayout::of(t.layout.preset);
                (t.layout.opacity, t.layout.pass_through) = keep;
                t.selected = None;
                t.dirty = false;
            }
            EditAction::Aim => {
                t.layout.aim = if t.layout.aim == Aim::Touch { Aim::Crosshair } else { Aim::Touch };
                t.layout.save();
            }
            EditAction::Standard | EditAction::Bedrock | EditAction::Dpad => {
                let keep = (t.layout.opacity, t.layout.pass_through, t.layout.aim);
                t.layout = TouchLayout::of(match action {
                    EditAction::Bedrock => Preset::Bedrock,
                    EditAction::Standard => Preset::Standard,
                    _ => Preset::Dpad,
                });
                (t.layout.opacity, t.layout.pass_through, t.layout.aim) = keep;
                t.selected = None;
                t.dirty = false;
            }
            EditAction::Custom => {
                // The player's own: start from the saved one, if there is one.
                let saved = TouchLayout::load();
                if saved.preset == Preset::Custom {
                    t.layout = saved;
                } else {
                    t.layout.preset = Preset::Custom;
                }
                t.selected = None;
            }
            // With a control selected, only that one; else every button.
            EditAction::PassThrough => match t.selected {
                Some(id) => {
                    let now = t.layout.looks(Some(id));
                    if let Some(spec) = t.layout.controls.get_mut(&id) {
                        spec.look = Some(!now);
                    }
                    t.dirty = true;
                }
                None => t.layout.pass_through = !t.layout.pass_through,
            },
            EditAction::Save => {
                if t.dirty {
                    t.layout.preset = Preset::Custom;
                }
                t.dirty = false;
                t.layout.save();
                t.editing = false;
                t.selected = None;
            }
            _ => {}
        }
    }

    /// A control tapped (or held) during play.
    fn touch_press(&mut self, id: Id) -> Finger {
        let pad = |game: &mut Self, button: Button| {
            game.pad.injected.set(button, true);
            Finger::Press(button, game.touch.press_at, game.touch.layout.looks(game.touch.pressing))
        };
        match id {
            Id::Stick => Finger::Stick,
            Id::Dpad => Finger::Dpad,
            Id::AimToggle => {
                let t = &mut self.touch;
                t.layout.aim = if t.layout.aim == Aim::Touch { Aim::Crosshair } else { Aim::Touch };
                t.layout.save();
                Finger::Done
            }
            Id::Mount => {
                // Riding: get off (sneak); else use what is under the aim (a horse, a boat).
                if self.riding.is_some() {
                    self.pad.injected.set(Button::RightStick, true);
                    self.touch.unsneak_frames = 3;
                } else {
                    self.pad.injected.triggers[0] = 1.0;
                    self.touch.trigger_release[0] = 3;
                }
                Finger::Done
            }
            Id::F8 => {
                self.smooth_camera = !self.smooth_camera;
                Finger::Done
            }
            Id::Jump => pad(self, Button::South),
            Id::Sneak => {
                self.touch.sneaking = !self.touch.sneaking;
                self.pad.injected.set(Button::RightStick, self.touch.sneaking);
                Finger::Done
            }
            Id::Sprint => {
                self.touch.sprinting = !self.touch.sprinting;
                self.pad.injected.set(Button::LeftStick, self.touch.sprinting);
                Finger::Done
            }
            Id::Attack | Id::Use => unreachable!("touch_event starts trigger fingers"),
            Id::Pause => pad(self, Button::Start),
            // Straight to the chat screen: the pad's chat binding (DPadUp by
            // default, DPadRight in some layouts) is not a fixed button.
            Id::Chat | Id::Keyboard => {
                self.open_chat_method("");
                #[cfg(target_os = "android")]
                super::android_ime::show(true);
                Finger::Done
            }
            Id::Inventory => pad(self, Button::North),
            Id::Drop => pad(self, Button::East),
            Id::F1 => {
                self.toggle_hide_gui();
                Finger::Done
            }
            Id::F2 => {
                self.take_screenshot();
                Finger::Done
            }
            Id::F3 => {
                use horizon_client::debug_keys::KeyAction;
                use winit::keyboard::KeyCode;
                self.physical_keys.insert(KeyCode::F3);
                self.debug_key(KeyCode::F3, KeyAction::Press);
                self.physical_keys.remove(&KeyCode::F3);
                self.debug_key(KeyCode::F3, KeyAction::Release);
                Finger::Done
            }
            Id::F5 => {
                self.cycle_camera();
                Finger::Done
            }
            Id::Edit => {
                self.touch.editing = true;
                self.touch.dirty = false;
                self.touch.selected = None;
                Finger::Done
            }
        }
    }

    pub(super) fn touch_event(&mut self, event_loop: &dyn EventLoopCtl, touch: Touch) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some((gui, scale)) = self.touch_gui() else {
            return;
        };
        let pos = [touch.location.x, touch.location.y];
        let p = [(pos[0] / f64::from(scale)) as f32, (pos[1] / f64::from(scale)) as f32];
        let id = touch.id;
        let device_id = touch.device_id;
        let window_id = window.id();
        let send = |game: &mut Self, event: WindowEvent| {
            game.handle_window_event(event_loop, window_id, event, super::input_trace::InputOrigin::User);
        };
        // A finger on the screen: the touch controls come back.
        if touch.phase == TouchPhase::Started {
            self.touch.hardware = false;
        }
        if self.touch.editing && !self.touch_controls_shown() {
            self.touch.editing = false;
        }
        match touch.phase {
            TouchPhase::Started => {
                let finger = if self.touch_profile_shown() && gui.profile_hit(p) {
                    #[cfg(target_os = "android")]
                    super::android_ime::open_profile();
                    Finger::Done
                } else if self.touch_close_shown() && gui.close_hit(p) {
                    self.touch_back();
                    Finger::Done
                } else if !self.touch_controls_shown()
                    && self.touch.fingers.values().any(|f| matches!(f, Finger::Pointer))
                {
                    // One finger is the mouse; a second one would click twice.
                    Finger::Done
                } else if !self.touch_controls_shown() {
                    send(self, cursor(device_id, pos));
                    send(self, click(device_id, ElementState::Pressed));
                    // Asked again on the next frame: a tap on a text field brings the
                    // keyboard back after the system's back gesture hid it.
                    self.touch.ime_retap = true;
                    Finger::Pointer
                } else if self.touch.editing {
                    if let Some(action) = gui.panel_hit(p) {
                        self.touch_edit(action);
                        Finger::Done
                    } else if let Some(control) = gui.hit(&self.touch.layout, p, true) {
                        self.touch.selected = Some(control);
                        let (c, _) = gui.place(&self.touch.layout, control);
                        Finger::Drag(control, [c[0] - p[0], c[1] - p[1]])
                    } else {
                        self.touch.selected = None;
                        Finger::Done
                    }
                } else if let Some(control) = gui.hit(&self.touch.layout, p, false)
                    && matches!(control, Id::Attack | Id::Use)
                {
                    let i = if control == Id::Attack { 1 } else { 0 };
                    let look = self.touch.layout.looks(Some(control));
                    Finger::Trigger { i, last: pos, since: Instant::now(), look }
                } else if let Some(control) = gui.hit(&self.touch.layout, p, false) {
                    self.touch.press_at = pos;
                    self.touch.pressing = Some(control);
                    if control == Id::Dpad && gui.dpad_at(&self.touch.layout, p) == Some([0, 0]) {
                        // The centre: sneak, toggled (Bedrock).
                        self.touch_press(Id::Sneak)
                    } else {
                        let finger = self.touch_press(control);
                        match finger {
                            Finger::Stick => self.touch_stick(&gui, p),
                            Finger::Dpad => self.touch_dpad(&gui, p, true),
                            _ => {}
                        }
                        finger
                    }
                } else if let Some(slot) = gui.hotbar_slot(p) {
                    self.selected = slot;
                    Finger::Done
                } else {
                    Finger::Look { last: pos, start: pos, since: Instant::now(), moved: false, attacking: false }
                };
                self.touch.fingers.insert(id, finger);
            }
            TouchPhase::Moved => match self.touch.fingers.get(&id).copied() {
                Some(Finger::Pointer) => send(self, cursor(device_id, pos)),
                Some(Finger::Stick) => self.touch_stick(&gui, p),
                Some(Finger::Dpad) => self.touch_dpad(&gui, p, false),
                Some(Finger::Drag(control, off)) => {
                    self.touch.dirty = true;
                    if let Some(spec) = self.touch.layout.controls.get_mut(&control) {
                        spec.x = ((p[0] + off[0]) / gui.w).clamp(0.0, 1.0);
                        spec.y = ((p[1] + off[1]) / gui.h).clamp(0.0, 1.0);
                    }
                }
                Some(Finger::Look { last, start, since, moved, attacking })
                    if attacking && self.touch.layout.aim == Aim::Touch =>
                {
                    // Breaking: the target follows the finger, the view stays.
                    self.touch.fingers.insert(id, Finger::Look { last: pos, start: pos, since, moved, attacking });
                    self.touch.aim_at = self.touch_ndc(pos);
                    let _ = (last, start);
                }
                Some(Finger::Look { last, start, since, moved, attacking }) => {
                    let (dx, dy) = (pos[0] - last[0], pos[1] - last[1]);
                    let far = ((pos[0] - start[0]).hypot(pos[1] - start[1])) / f64::from(scale) > SLOP;
                    self.touch
                        .fingers
                        .insert(id, Finger::Look { last: pos, start, since, moved: moved || far, attacking });
                    self.mouse.on_move(0.0, 0.0, dx, dy, true, true);
                    self.last_input = Instant::now();
                }
                Some(Finger::Trigger { i, last, since, look }) => {
                    // The attack (or use) button's finger turns the view too, unless
                    // another finger is already turning it: two fingers' moves added
                    // together made the view jump.
                    let (dx, dy) = (pos[0] - last[0], pos[1] - last[1]);
                    self.touch.fingers.insert(id, Finger::Trigger { i, last: pos, since, look });
                    if look && !self.touch_looking(id) {
                        self.mouse.on_move(0.0, 0.0, dx, dy, true, true);
                        self.last_input = Instant::now();
                    }
                }
                Some(Finger::Press(button, last, true)) => {
                    let (dx, dy) = (pos[0] - last[0], pos[1] - last[1]);
                    self.touch.fingers.insert(id, Finger::Press(button, pos, true));
                    if !self.touch_looking(id) {
                        self.mouse.on_move(0.0, 0.0, dx, dy, true, true);
                        self.last_input = Instant::now();
                    }
                }
                _ => {}
            },
            TouchPhase::Ended | TouchPhase::Cancelled => match self.touch.fingers.remove(&id) {
                Some(Finger::Pointer) => {
                    // A cancelled touch (a system gesture took it) lets go off screen: no click.
                    let at = if touch.phase == TouchPhase::Cancelled { [-1.0, -1.0] } else { pos };
                    send(self, cursor(device_id, at));
                    send(self, click(device_id, ElementState::Released));
                }
                Some(Finger::Stick) => {
                    self.touch.knob = [0.0, 0.0];
                    self.pad.injected.left = [0.0, 0.0];
                }
                Some(Finger::Dpad) => {
                    if self.touch.dpad.is_some_and(|d| d == [0, 1]) {
                        self.touch.forward_up = Some(Instant::now());
                    }
                    self.touch.dpad = None;
                    self.pad.injected.left = [0.0, 0.0];
                }
                Some(Finger::Look { attacking: true, .. }) => {
                    // The aim point stays until the attack trigger is let go.
                    self.touch.aim_frames = 2;
                }
                Some(Finger::Press(button, _, _)) => self.pad.injected.set(button, false),
                Some(Finger::Trigger { i, since, .. }) if since.elapsed().as_millis() < HOLD_MS => {
                    // A short tap: keep it down a few frames so a tick sees
                    // one hit (or one use).
                    self.pad.injected.triggers[i] = 1.0;
                    self.touch.trigger_release[i] = 3;
                }
                Some(Finger::Look { moved: false, attacking: false, .. }) if touch.phase == TouchPhase::Ended => {
                    // A tap: attack the entity under the crosshair (Bedrock),
                    // else use; held for a few frames so a tick sees it.
                    if self.touch.layout.aim == Aim::Touch {
                        self.touch.aim_at = self.touch_ndc(pos);
                        self.touch.aim_frames = 4;
                    }
                    let i = if self.pick_entity_hit().is_some() { 1 } else { 0 };
                    self.pad.injected.triggers[i] = 1.0;
                    self.touch.trigger_release[i] = 3;
                }
                _ => {}
            },
        }
    }

    /// Whether a finger other than `id` is turning the view on open screen.
    fn touch_looking(&self, id: u64) -> bool {
        self.touch.fingers.iter().any(|(f, finger)| *f != id && matches!(finger, Finger::Look { .. }))
    }

    /// A window position as -1..1 across the screen, y up.
    fn touch_ndc(&self, pos: [f64; 2]) -> Option<[f32; 2]> {
        let r = self.renderer.as_ref()?;
        let (w, h) = (r.config.width.max(1) as f64, r.config.height.max(1) as f64);
        Some([(pos[0] / w * 2.0 - 1.0) as f32, (1.0 - pos[1] / h * 2.0) as f32])
    }

    /// Touch aiming: the unit ray through the touched point (`Game::aim`).
    pub(super) fn touch_aim_dir(&self) -> Option<[f64; 3]> {
        let [nx, ny] = self.touch.aim_at?;
        let r = self.renderer.as_ref()?;
        let aspect = r.config.width.max(1) as f64 / r.config.height.max(1) as f64;
        let half = (f64::from(self.settings.fov as f32) / 2.0).to_radians().tan();
        let (y, p) = (f64::from(self.yaw).to_radians(), f64::from(self.pitch).to_radians());
        let f = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
        let right = [-y.cos(), 0.0, -y.sin()];
        let up = [-y.sin() * p.sin(), p.cos(), y.cos() * p.sin()];
        let (sx, sy) = (f64::from(nx) * half * aspect, f64::from(ny) * half);
        let d: [f64; 3] = std::array::from_fn(|i| f[i] + right[i] * sx + up[i] * sy);
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
        Some(d.map(|v| v / len))
    }

    /// Touch aiming hides the crosshair, as Bedrock's Touch mode.
    pub(super) fn touch_hides_crosshair(&self) -> bool {
        self.touch_controls_shown() && self.touch.layout.preset != Preset::Standard && self.touch.layout.aim == Aim::Touch
    }

    /// The D-pad under the finger: the direction it moves; the forward diagonals
    /// only from forward, as they only show then; a second forward soon after the
    /// last one sprints.
    fn touch_dpad(&mut self, gui: &Gui, p: [f32; 2], first: bool) {
        let cell = gui.dpad_at(&self.touch.layout, p);
        let now = self.touch.dpad;
        let next = match cell {
            None => None,
            Some([0, 0]) => now,
            Some([x, 1]) if x != 0 => now.filter(|d| d[1] == 1).map(|_| [x, 1]),
            Some([x, -1]) if x != 0 => Some([0, -1]),
            Some(d) => Some(d),
        };
        if first
            && next == Some([0, 1])
            && self.touch.forward_up.is_some_and(|t| t.elapsed().as_millis() < 300)
            && !self.touch.sprinting
        {
            self.touch.sprinting = true;
            self.pad.injected.set(Button::LeftStick, true);
        }
        if now == Some([0, 1]) && next != Some([0, 1]) && next.is_none_or(|d| d[1] != 1) {
            self.touch.forward_up = Some(Instant::now());
        }
        self.touch.dpad = next;
        self.pad.injected.left = match next {
            Some([x, y]) => {
                let v = [f32::from(x), f32::from(y)];
                let len = v[0].hypot(v[1]).max(1.0);
                [v[0] / len, v[1] / len]
            }
            None => [0.0, 0.0],
        };
    }

    fn touch_stick(&mut self, gui: &Gui, p: [f32; 2]) {
        let (c, r) = gui.place(&self.touch.layout, Id::Stick);
        let mut v = [(p[0] - c[0]) / (r * 0.6), (c[1] - p[1]) / (r * 0.6)];
        let len = v[0].hypot(v[1]);
        if len > 1.0 {
            v = [v[0] / len, v[1] / len];
        }
        self.touch.knob = v;
        self.pad.injected.left = v;
        // Bedrock: the stick pushed all the way forward sprints.
        if self.touch.layout.preset == Preset::Bedrock && v[1] > 0.93 && v[0].abs() < 0.4 && !self.touch.sprinting {
            self.touch.sprinting = true;
            self.pad.injected.set(Button::LeftStick, true);
        }
    }
}

fn cursor(device_id: DeviceId, p: [f64; 2]) -> WindowEvent {
    WindowEvent::CursorMoved {
        device_id,
        position: PhysicalPosition::new(p[0], p[1]),
    }
}

fn click(device_id: DeviceId, state: ElementState) -> WindowEvent {
    WindowEvent::MouseInput {
        device_id,
        state,
        button: MouseButton::Left,
    }
}
