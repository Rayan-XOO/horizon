//! Hardware input on Android: game controllers, a mouse and a keyboard.
//!
//! winit has no gamepad events and reads every motion event as a touch, so the
//! patched winit (third_party/winit) hands each input event to [`filter`] first.
//! Controller buttons and axes become a [`PadState`] that `pad.rs` reads as its
//! hardware pad, so a controller plays like one on the desktop (bindings, menu
//! cursor, deadzones). Mouse events are queued for the game thread
//! (`Game::android_input_frame`). Keyboard keys stay with winit, which already
//! maps them to the physical keys the game handles; they are only noted so the
//! touch controls can hide.
use horizon_client::gamepad::{Button, PadState};
use std::sync::Mutex;
use winit::platform::android::activity::input::{Axis, InputEvent, KeyAction, Keycode, MotionAction};

/// `InputDevice.SOURCE_*` bits.
const SOURCE_KEYBOARD: u32 = 0x0000_0101;
const SOURCE_DPAD: u32 = 0x0000_0201;
const SOURCE_GAMEPAD: u32 = 0x0000_0401;
const SOURCE_JOYSTICK: u32 = 0x0100_0010;
const SOURCE_MOUSE: u32 = 0x0000_2002;
const SOURCE_MOUSE_RELATIVE: u32 = 0x0002_0004;

/// What the game thread gets from a mouse.
#[derive(Clone, Copy, Debug)]
pub(super) enum MouseInput {
    /// The pointer, in window pixels.
    Move([f64; 2]),
    /// Movement while the pointer is captured, in pixels.
    Delta([f64; 2]),
    /// 0 left, 1 right, 2 middle, 3 back, 4 forward.
    Button(u8, bool),
    /// Wheel clicks, up and right positive.
    Wheel([f32; 2]),
}

#[derive(Default)]
struct Shared {
    /// Buttons held on the controller's keys.
    keys: u32,
    /// Analog trigger buttons held as keys (L2, R2).
    key_triggers: [bool; 2],
    left: [f32; 2],
    z_rz: [f32; 2],
    rx_ry: [f32; 2],
    /// Analog triggers: LTRIGGER/RTRIGGER and BRAKE/GAS.
    triggers: [f32; 2],
    /// The hat (the D-pad on most controllers) as buttons.
    hat: u32,
    /// The right stick shows up on RX/RY (some DirectInput controllers); Z/RZ
    /// then carry the triggers.
    rx_ry_stick: bool,
    /// Trigger axes have moved (LTRIGGER, RTRIGGER, GAS, BRAKE).
    trigger_axes: bool,
    mouse: Vec<MouseInput>,
    mouse_buttons: u32,
    /// A primary click without a button state is held.
    tap: bool,
    keyboard_used: bool,
    mouse_used: bool,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    keys: 0,
    key_triggers: [false; 2],
    left: [0.0; 2],
    z_rz: [0.0; 2],
    rx_ry: [0.0; 2],
    triggers: [0.0; 2],
    hat: 0,
    rx_ry_stick: false,
    trigger_axes: false,
    mouse: Vec::new(),
    mouse_buttons: 0,
    tap: false,
    keyboard_used: false,
    mouse_used: false,
});

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Route input events through [`filter`]; called once before the event loop starts.
pub(super) fn install() {
    winit::platform::android::set_input_filter(filter);
}

/// The controller as the game reads it.
pub(super) fn pad_state() -> PadState {
    let s = shared();
    let right = if s.rx_ry_stick { s.rx_ry } else { s.z_rz };
    let mut triggers = s.triggers;
    if s.rx_ry_stick && !s.trigger_axes {
        triggers = [s.z_rz[0].max(0.0), s.z_rz[1].max(0.0)];
    }
    for i in 0..2 {
        if s.key_triggers[i] {
            triggers[i] = 1.0;
        }
    }
    PadState { buttons: s.keys | s.hat, left: s.left, right: [right[0], -right[1]], triggers }
}

/// Everything let go (the app went to the background, a controller went away).
pub(super) fn reset() {
    let mut s = shared();
    s.keys = 0;
    s.key_triggers = [false; 2];
    s.left = [0.0; 2];
    s.z_rz = [0.0; 2];
    s.rx_ry = [0.0; 2];
    s.triggers = [0.0; 2];
    s.hat = 0;
    s.mouse_buttons = 0;
    s.tap = false;
    s.mouse.clear();
}

/// Mouse events since the last call.
pub(super) fn take_mouse() -> Vec<MouseInput> {
    std::mem::take(&mut shared().mouse)
}

/// A hardware keyboard or a mouse was used since the last call.
pub(super) fn take_used() -> (bool, bool) {
    let mut s = shared();
    let used = (s.keyboard_used, s.mouse_used);
    s.keyboard_used = false;
    s.mouse_used = false;
    used
}

fn has(source: u32, bits: u32) -> bool {
    source & bits == bits
}

/// The controller button an Android key code stands for. BUTTON_1..12 are
/// DirectInput controllers without a key layout, in the usual order
/// (1 X, 2 A, 3 B, 4 Y, 5 L1, 6 R1, 7 L2, 8 R2, 9 select, 10 start, 11 L3, 12 R3).
fn pad_button(code: Keycode) -> Option<Button> {
    Some(match code {
        Keycode::ButtonA | Keycode::Button2 => Button::South,
        Keycode::ButtonB | Keycode::Button3 => Button::East,
        Keycode::ButtonX | Keycode::Button1 => Button::West,
        Keycode::ButtonY | Keycode::Button4 => Button::North,
        Keycode::ButtonL1 | Keycode::Button5 => Button::LeftBumper,
        Keycode::ButtonR1 | Keycode::Button6 => Button::RightBumper,
        Keycode::ButtonL2 | Keycode::Button7 => Button::LeftTrigger,
        Keycode::ButtonR2 | Keycode::Button8 => Button::RightTrigger,
        Keycode::ButtonSelect | Keycode::Button9 => Button::Select,
        Keycode::ButtonStart | Keycode::Button10 => Button::Start,
        Keycode::ButtonThumbl | Keycode::Button11 => Button::LeftStick,
        Keycode::ButtonThumbr | Keycode::Button12 => Button::RightStick,
        Keycode::DpadUp => Button::DPadUp,
        Keycode::DpadDown => Button::DPadDown,
        Keycode::DpadLeft => Button::DPadLeft,
        Keycode::DpadRight => Button::DPadRight,
        // A controller's back key (the View/Select button on many).
        Keycode::Back => Button::Select,
        _ => return None,
    })
}

fn is_gamepad_key(code: Keycode) -> bool {
    matches!(
        code,
        Keycode::ButtonA
            | Keycode::ButtonB
            | Keycode::ButtonX
            | Keycode::ButtonY
            | Keycode::ButtonL1
            | Keycode::ButtonR1
            | Keycode::ButtonL2
            | Keycode::ButtonR2
            | Keycode::ButtonSelect
            | Keycode::ButtonStart
            | Keycode::ButtonThumbl
            | Keycode::ButtonThumbr
            | Keycode::Button1
            | Keycode::Button2
            | Keycode::Button3
            | Keycode::Button4
            | Keycode::Button5
            | Keycode::Button6
            | Keycode::Button7
            | Keycode::Button8
            | Keycode::Button9
            | Keycode::Button10
            | Keycode::Button11
            | Keycode::Button12
    )
}

/// Every input event, before winit: `true` takes it.
fn filter(event: &InputEvent<'_>) -> bool {
    match event {
        InputEvent::KeyEvent(key) => {
            let source: u32 = key.source().into();
            let code = key.key_code();
            let down = match key.action() {
                KeyAction::Down => true,
                KeyAction::Up => false,
                _ => return false,
            };
            // A mouse's right button turns into Back when nobody takes it: the
            // button already went to the game.
            if code == Keycode::Back && has(source, SOURCE_MOUSE) {
                return true;
            }
            let from_pad = has(source, SOURCE_GAMEPAD)
                || has(source, SOURCE_JOYSTICK)
                || is_gamepad_key(code)
                || (source == SOURCE_DPAD
                    && matches!(code, Keycode::DpadUp | Keycode::DpadDown | Keycode::DpadLeft | Keycode::DpadRight));
            if from_pad {
                let Some(button) = pad_button(code) else {
                    // Home/Mode and the rest stay with the system.
                    return false;
                };
                let mut s = shared();
                match button {
                    Button::LeftTrigger => s.key_triggers[0] = down,
                    Button::RightTrigger => s.key_triggers[1] = down,
                    _ => {
                        if down {
                            s.keys |= button.bit();
                        } else {
                            s.keys &= !button.bit();
                        }
                    }
                }
                return true;
            }
            if has(source, SOURCE_KEYBOARD)
                && !matches!(code, Keycode::Back | Keycode::Home | Keycode::VolumeUp | Keycode::VolumeDown | Keycode::VolumeMute | Keycode::Power)
            {
                shared().keyboard_used = true;
            }
            false
        }
        InputEvent::MotionEvent(motion) => {
            let source: u32 = motion.source().into();
            if has(source, SOURCE_JOYSTICK) {
                if motion.action() != MotionAction::Move {
                    return true;
                }
                let p = motion.pointer_at_index(0);
                let a = |axis| {
                    let v = p.axis_value(axis);
                    if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 }
                };
                let mut s = shared();
                s.left = [a(Axis::X), -a(Axis::Y)];
                s.z_rz = [a(Axis::Z), a(Axis::Rz)];
                s.rx_ry = [a(Axis::Rx), a(Axis::Ry)];
                if s.rx_ry[0].abs() > 0.3 || s.rx_ry[1].abs() > 0.3 {
                    s.rx_ry_stick = true;
                }
                let (lt, rt) = (a(Axis::Ltrigger).max(a(Axis::Brake)), a(Axis::Rtrigger).max(a(Axis::Gas)));
                if lt > 0.05 || rt > 0.05 {
                    s.trigger_axes = true;
                }
                s.triggers = [lt.max(0.0), rt.max(0.0)];
                let (hx, hy) = (a(Axis::HatX), a(Axis::HatY));
                let mut hat = 0;
                if hx < -0.5 {
                    hat |= Button::DPadLeft.bit();
                } else if hx > 0.5 {
                    hat |= Button::DPadRight.bit();
                }
                if hy < -0.5 {
                    hat |= Button::DPadUp.bit();
                } else if hy > 0.5 {
                    hat |= Button::DPadDown.bit();
                }
                s.hat = hat;
                return true;
            }
            let relative = has(source, SOURCE_MOUSE_RELATIVE);
            if !relative && !has(source, SOURCE_MOUSE) {
                return false;
            }
            let p = motion.pointer_at_index(0);
            let mut s = shared();
            s.mouse_used = true;
            match motion.action() {
                MotionAction::Scroll => {
                    let w = [p.axis_value(Axis::Hscroll), p.axis_value(Axis::Vscroll)];
                    if w != [0.0, 0.0] {
                        s.mouse.push(MouseInput::Wheel(w));
                    }
                }
                MotionAction::Move | MotionAction::HoverMove | MotionAction::Down | MotionAction::Up => {
                    if relative {
                        let d = [f64::from(p.x()), f64::from(p.y())];
                        if d != [0.0, 0.0] {
                            s.mouse.push(MouseInput::Delta(d));
                        }
                    } else {
                        s.mouse.push(MouseInput::Move([f64::from(p.x()), f64::from(p.y())]));
                    }
                }
                _ => {}
            }
            // Buttons from the state each event carries: press and release events, and
            // the down/up a primary click also sends, all agree on it.
            let mut buttons = motion.button_state().0 & 0x1f;
            // A touchpad tap may come as down/up without a button state: the primary button.
            let action = motion.action();
            if buttons == 0 && action == MotionAction::Down {
                s.tap = true;
            }
            if s.tap {
                if matches!(action, MotionAction::Up | MotionAction::Cancel) {
                    s.tap = false;
                } else {
                    buttons |= 1;
                }
            }
            let changed = buttons ^ s.mouse_buttons;
            for i in 0..5u8 {
                if changed & (1 << i) != 0 {
                    s.mouse.push(MouseInput::Button(i, buttons & (1 << i) != 0));
                }
            }
            s.mouse_buttons = buttons;
            true
        }
        _ => false,
    }
}

impl super::Game<'_> {
    /// Once per loop iteration, before the pads are read: mouse events go to the
    /// game as the window's own would, the pointer is captured while playing with
    /// a mouse, and the touch controls hide while a controller, keyboard or mouse
    /// plays (a touch brings them back, `touch_event`).
    pub(super) fn android_input_frame(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        use winit::{
            application::ApplicationHandler,
            dpi::PhysicalPosition,
            event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
        };
        let (keyboard, mouse) = take_used();
        if keyboard || mouse {
            self.touch.hardware = true;
            self.pad.in_use = false;
        }
        if mouse {
            self.touch.mouse = true;
        }
        let Some(window_id) = self.window.as_ref().map(|w| w.id()) else {
            take_mouse();
            return;
        };
        let device_id = DeviceId::dummy();
        for input in take_mouse() {
            let in_menu = self.pad_in_menu();
            let event = match input {
                MouseInput::Move(p) if in_menu => WindowEvent::CursorMoved {
                    device_id,
                    position: PhysicalPosition::new(p[0], p[1]),
                },
                MouseInput::Move(_) => continue,
                MouseInput::Delta(d) => {
                    if !in_menu {
                        self.mouse.on_move(0.0, 0.0, d[0], d[1], true, true);
                        self.last_input = std::time::Instant::now();
                    }
                    continue;
                }
                MouseInput::Button(i, down) => WindowEvent::MouseInput {
                    device_id,
                    state: if down { ElementState::Pressed } else { ElementState::Released },
                    button: match i {
                        0 => MouseButton::Left,
                        1 => MouseButton::Right,
                        2 => MouseButton::Middle,
                        3 => MouseButton::Back,
                        _ => MouseButton::Forward,
                    },
                },
                MouseInput::Wheel(w) => WindowEvent::MouseWheel {
                    device_id,
                    delta: MouseScrollDelta::LineDelta(w[0], w[1]),
                    phase: TouchPhase::Moved,
                },
            };
            self.window_event(event_loop, window_id, event);
        }
        // The pointer is captured for turning the view, and let go in menus.
        let capture = self.touch.mouse && self.focused && self.player.is_some() && !self.pad_in_menu();
        if self.touch.pointer_captured != Some(capture) {
            self.touch.pointer_captured = Some(capture);
            super::android_ime::pointer_capture(capture);
        }
    }
}
