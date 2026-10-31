//! Touch controls (Android; ANDROID.md), laid out like Bedrock Edition's.
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
const LAYOUT_FILE: &str = "touch_layout.json";

/// What a finger is doing, from where it went down.
#[derive(Clone, Copy, Debug)]
enum Finger {
    /// Acting as the mouse in a menu.
    Pointer,
    /// The joystick; the knob's position (GUI pixels).
    Stick,
    /// Turning the view; tap = attack an entity or use, hold = attack.
    Look {
        last: [f64; 2],
        start: [f64; 2],
        since: Instant,
        moved: bool,
        attacking: bool,
    },
    /// Holding a controller button.
    Press(Button),
    /// Holding a trigger (0 = use, 1 = attack) from its button; moving the
    /// same finger turns the view like `Look`, and the trigger stays held
    /// until it lifts, wherever it slides.
    Trigger { i: usize, last: [f64; 2], since: Instant },
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

const IDS: [Id; 16] = [
    Id::Stick,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TouchLayout {
    /// Multiplies every control's opacity.
    opacity: f32,
    controls: HashMap<Id, Spec>,
}

impl Default for TouchLayout {
    /// Laid out on a 560 x 270 GUI (a 20:9 phone at GUI scale 4).
    fn default() -> Self {
        let (w, h) = (560.0f32, 270.0f32);
        let at = |x: f32, y: f32, size: f32| Spec { x: x / w, y: y / h, size, opacity: 1.0, visible: true };
        let bar = (w / 2.0 - 91.0).floor() + 182.0;
        let controls = HashMap::from([
            (Id::Stick, at(44.0, h - 48.0, 32.0)),
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
        Self { opacity: 0.85, controls }
    }
}

impl TouchLayout {
    fn load() -> Self {
        let mut layout = Self::default();
        if let Ok(text) = std::fs::read_to_string(LAYOUT_FILE)
            && let Ok(saved) = serde_json::from_str::<TouchLayout>(&text)
        {
            layout.opacity = saved.opacity.clamp(0.1, 1.0);
            layout.controls.extend(saved.controls);
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
    fn spec(&self, id: Id) -> Spec {
        self.controls.get(&id).copied().unwrap_or(Spec { x: 0.5, y: 0.5, size: 10.0, opacity: 1.0, visible: true })
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
}

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
    /// Frames until tapped triggers are released (use, attack).
    trigger_release: [u8; 2],
    /// Sneak toggled on (Bedrock's sneak button toggles).
    sneaking: bool,
    /// Sprint toggled on; off again when the joystick is let go.
    sprinting: bool,
    layout: TouchLayout,
    /// The layout editor is open; the selected control.
    editing: bool,
    selected: Option<Id>,
    /// The soft keyboard was asked for.
    ime_shown: bool,
    /// A menu tap since the last frame: ask for the keyboard again if a field wants it.
    ime_retap: bool,
}

impl Default for Touches {
    fn default() -> Self {
        Self {
            fingers: HashMap::new(),
            knob: [0.0; 2],
            trigger_release: [0; 2],
            sneaking: false,
            sprinting: false,
            layout: if cfg!(target_os = "android") { TouchLayout::load() } else { TouchLayout::default() },
            editing: false,
            selected: None,
            ime_shown: false,
            ime_retap: false,
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
        EDIT_ACTIONS
            .into_iter()
            .enumerate()
            .map(move |(i, (a, l))| (a, l, [x0 + i as f32 * (bw + gap), y, bw, 16.0]))
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
        Id::Stick => {}
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
        cfg!(target_os = "android") && self.player.is_some() && !self.pad_in_menu()
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
            if id == Id::Stick {
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
                None => format!("Drag a control; tap one to pick it | all {:.0}%", layout.opacity * 100.0),
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
                batch.fill(atlas, r[0], r[1], r[2], r[3], rgba(30, 30, 30, 220));
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
        let mut attack = false;
        let mut held = [false; 2];
        for finger in self.touch.fingers.values_mut() {
            if let Finger::Look { since, moved, attacking, .. } = finger
                && !*moved
                && since.elapsed().as_millis() >= HOLD_MS
            {
                *attacking = true;
            }
            match finger {
                Finger::Look { attacking: true, .. } => attack = true,
                Finger::Trigger { i, .. } => held[*i] = true,
                _ => {}
            }
        }
        held[1] |= attack;
        // Like vanilla, sprint ends when the player stops moving: here when
        // the joystick is let go.
        if self.touch.sprinting && !self.touch.fingers.values().any(|f| matches!(f, Finger::Stick)) {
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
                t.layout = TouchLayout::default();
                t.selected = None;
            }
            EditAction::Save => {
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
            Finger::Press(button)
        };
        match id {
            Id::Stick => Finger::Stick,
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
            Id::Chat => pad(self, Button::DPadRight),
            Id::Inventory => pad(self, Button::North),
            Id::Drop => pad(self, Button::East),
            Id::Keyboard => {
                // Opens chat; the keyboard follows. Shown again if dismissed.
                #[cfg(target_os = "android")]
                if self.touch.ime_shown {
                    super::android_ime::show(true);
                }
                pad(self, Button::DPadRight)
            }
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
                self.touch.selected = None;
                Finger::Done
            }
        }
    }

    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, touch: Touch) {
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
            game.window_event(event_loop, window_id, event);
        };
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
                    Finger::Trigger { i, last: pos, since: Instant::now() }
                } else if let Some(control) = gui.hit(&self.touch.layout, p, false) {
                    let finger = self.touch_press(control);
                    if matches!(finger, Finger::Stick) {
                        self.touch_stick(&gui, p);
                    }
                    finger
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
                Some(Finger::Drag(control, off)) => {
                    if let Some(spec) = self.touch.layout.controls.get_mut(&control) {
                        spec.x = ((p[0] + off[0]) / gui.w).clamp(0.0, 1.0);
                        spec.y = ((p[1] + off[1]) / gui.h).clamp(0.0, 1.0);
                    }
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
                Some(Finger::Trigger { i, last, since }) => {
                    // The attack (or use) button's finger turns the view too.
                    let (dx, dy) = (pos[0] - last[0], pos[1] - last[1]);
                    self.touch.fingers.insert(id, Finger::Trigger { i, last: pos, since });
                    self.mouse.on_move(0.0, 0.0, dx, dy, true, true);
                    self.last_input = Instant::now();
                }
                _ => {}
            },
            TouchPhase::Ended | TouchPhase::Cancelled => match self.touch.fingers.remove(&id) {
                Some(Finger::Pointer) => {
                    send(self, cursor(device_id, pos));
                    send(self, click(device_id, ElementState::Released));
                }
                Some(Finger::Stick) => {
                    self.touch.knob = [0.0, 0.0];
                    self.pad.injected.left = [0.0, 0.0];
                }
                Some(Finger::Press(button)) => self.pad.injected.set(button, false),
                Some(Finger::Trigger { i, since, .. }) if since.elapsed().as_millis() < HOLD_MS => {
                    // A short tap: keep it down a few frames so a tick sees
                    // one hit (or one use).
                    self.pad.injected.triggers[i] = 1.0;
                    self.touch.trigger_release[i] = 3;
                }
                Some(Finger::Look { moved: false, attacking: false, .. }) if touch.phase == TouchPhase::Ended => {
                    // A tap: attack the entity under the crosshair (Bedrock),
                    // else use; held for a few frames so a tick sees it.
                    let i = if self.pick_entity_hit().is_some() { 1 } else { 0 };
                    self.pad.injected.triggers[i] = 1.0;
                    self.touch.trigger_release[i] = 3;
                }
                _ => {}
            },
        }
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
