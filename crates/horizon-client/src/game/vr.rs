//! Meta Quest (the `vr` build): OpenXR through the Khronos loader on the
//! renderer's own Vulkan device. Each eye is drawn as a frame of its own into
//! the renderer's offscreen target and copied into the runtime's swapchain;
//! the head pose moves the camera and the Touch controllers drive the game as
//! its virtual pad.
//!
//! Controls: left stick walks where the head looks, right stick turns in 45°
//! steps (a right stick click switches to smooth turning), A jumps, B sneaks,
//! the right trigger breaks and attacks, the left trigger or the right grip
//! places and uses, X opens the inventory, right stick up and down change the
//! hotbar slot, Y or the menu button pauses. Menus float in front of the
//! player as a panel; the right controller points at it and its trigger clicks.
use super::{Game, Screen};
use glam::{Mat4, Quat, Vec3};
use horizon_client::{
    gamepad::{Button, PadState},
    gfx::{ExternalDisplay, Frame, Renderer, View, set_external_display},
};
use openxr as xr;
use std::ffi::{CStr, CString};
use std::sync::{Mutex, OnceLock, PoisonError};
use wgpu::hal::api::Vulkan;
use winit::{
    application::ApplicationHandler, dpi::PhysicalPosition, event::*, event_loop::ActiveEventLoop,
};

/// The share of the runtime's recommended eye size drawn.
const RESOLUTION_SCALE: f32 = 0.8;
/// How far the world is drawn on the headset, in blocks.
const VIEW_DISTANCE: f32 = 8.0 * 16.0;
/// Degrees a flick of the right stick turns.
const SNAP_DEGREES: f32 = 45.0;
/// Degrees a second the right stick turns when smooth turning.
const SMOOTH_DEGREES: f32 = 120.0;
/// How far the camera may follow the head away from the body, in blocks.
const HEAD_REACH: f32 = 0.35;
/// The menu panel: its width and distance, in metres.
const PANEL_WIDTH: f32 = 1.6;
const PANEL_DISTANCE: f32 = 1.4;
const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

struct Base {
    instance: xr::Instance,
    system: xr::SystemId,
    size: (u32, u32),
}
static BASE: OnceLock<Base> = OnceLock::new();
/// The swapchain format: the runtime's value and wgpu's.
static FORMAT: OnceLock<(u32, wgpu::TextureFormat)> = OnceLock::new();
static STATE: Mutex<Option<State>> = Mutex::new(None);

struct Actions {
    set: xr::ActionSet,
    aim: xr::Action<xr::Posef>,
    aim_space: Option<xr::Space>,
    walk: xr::Action<xr::Vector2f>,
    turn: xr::Action<xr::Vector2f>,
    a: xr::Action<bool>,
    b: xr::Action<bool>,
    x: xr::Action<bool>,
    y: xr::Action<bool>,
    menu: xr::Action<bool>,
    turn_click: xr::Action<bool>,
    walk_click: xr::Action<bool>,
    left_trigger: xr::Action<f32>,
    right_trigger: xr::Action<f32>,
    left_grip: xr::Action<f32>,
    right_grip: xr::Action<f32>,
}

struct Chain {
    handle: xr::Swapchain<xr::Vulkan>,
    images: Vec<wgpu::Texture>,
}

struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
}

struct State {
    session: xr::Session<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<xr::Vulkan>,
    local: xr::Space,
    head: xr::Space,
    actions: Actions,
    running: bool,
    focused: bool,
    /// The last frame's predicted display time, for locating input.
    time: Option<xr::Time>,
    chains: Option<[Chain; 3]>,
    blit: Option<Blit>,
    /// The body's turn from snap or smooth turning, in degrees (vanilla yaw).
    turn: f32,
    smooth_turn: bool,
    stick_centred: bool,
    hotbar_centred: bool,
    turn_click: bool,
    /// Where the head started, the camera's origin in the tracking space.
    origin: Option<Vec3>,
    /// The menu panel's centre and yaw (radians) in the tracking space.
    panel: Option<(Vec3, f32)>,
    trigger_down: bool,
    /// The comfort vignette's strength now (0 to 1).
    vignette: f32,
    moving: f32,
}

/// Installs the headset as the renderer's display; before the renderer starts.
pub(crate) fn install() {
    set_external_display(ExternalDisplay {
        instance: create_instance,
        device: open_device,
        size: || BASE.get().map(|b| b.size),
    });
}

/// Whether the headset draws the game (the touch controls stay hidden).
pub(crate) fn active() -> bool {
    STATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
}

fn leak(names: &str) -> Vec<&'static CStr> {
    names
        .split_whitespace()
        .filter_map(|n| CString::new(n).ok())
        .map(|n| &*Box::leak(n.into_boxed_c_str()))
        .collect()
}

fn base() -> Result<&'static Base, String> {
    if let Some(base) = BASE.get() {
        return Ok(base);
    }
    let context = ndk_context::android_context();
    // Safety: the process's VM and activity, alive while the app runs.
    let platform = unsafe { xr::AndroidPlatformInfo::new(context.vm(), context.context()) };
    // Safety: the Khronos loader bundled in the APK.
    let entry = unsafe { xr::Entry::load(&platform) }.map_err(|e| format!("OpenXR loader: {e}"))?;
    let offered = entry
        .enumerate_extensions()
        .map_err(|e| format!("OpenXR extensions: {e}"))?;
    if !offered.khr_vulkan_enable {
        return Err("the OpenXR runtime has no XR_KHR_vulkan_enable".into());
    }
    let mut wanted = xr::ExtensionSet::default();
    wanted.khr_vulkan_enable = true;
    wanted.khr_android_create_instance = true;
    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                application_name: "Horizon",
                application_version: 1,
                engine_name: "Horizon",
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 0),
            },
            &wanted,
            &[],
            &platform,
        )
        .map_err(|e| format!("OpenXR instance: {e}"))?;
    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .map_err(|e| format!("no headset: {e}"))?;
    // The runtime asks for this before a session.
    let _ = instance.graphics_requirements::<xr::Vulkan>(system);
    let views = instance
        .enumerate_view_configuration_views(system, VIEW_TYPE)
        .map_err(|e| format!("OpenXR views: {e}"))?;
    let view = views.first().ok_or("OpenXR has no views")?;
    let scale = |v: u32, max: u32| ((v as f32 * RESOLUTION_SCALE) as u32).clamp(64, max.max(64));
    let size = (
        scale(view.recommended_image_rect_width, view.max_image_rect_width),
        scale(
            view.recommended_image_rect_height,
            view.max_image_rect_height,
        ),
    );
    log::info!("Horizon VR: {size:?} per eye");
    let _ = BASE.set(Base {
        instance,
        system,
        size,
    });
    Ok(BASE.get().expect("set"))
}

fn create_instance(descriptor: wgpu::InstanceDescriptor) -> wgpu::Instance {
    match xr_instance(&descriptor) {
        Ok(instance) => instance,
        Err(e) => {
            log::error!("Horizon VR: {e}; drawing without the headset");
            wgpu::Instance::new(descriptor)
        }
    }
}

fn xr_instance(descriptor: &wgpu::InstanceDescriptor) -> Result<wgpu::Instance, String> {
    let base = base()?;
    let extensions = leak(
        &base
            .instance
            .vulkan_legacy_instance_extensions(base.system)
            .map_err(|e| format!("OpenXR Vulkan instance extensions: {e}"))?,
    );
    let hal = wgpu::hal::InstanceDescriptor {
        name: "Horizon",
        flags: descriptor.flags,
        memory_budget_thresholds: descriptor.memory_budget_thresholds,
        backend_options: descriptor.backend_options.clone(),
        telemetry: None,
        display: None,
    };
    // Safety: wgpu's own instance, with the extensions the runtime needs added.
    let instance = unsafe {
        wgpu::hal::vulkan::Instance::init_with_callback(
            &hal,
            Some(Box::new(
                move |args: wgpu::hal::vulkan::CreateInstanceCallbackArgs| {
                    for e in &extensions {
                        if !args.extensions.contains(e) {
                            args.extensions.push(e);
                        }
                    }
                },
            )),
        )
    }
    .map_err(|e| format!("Vulkan instance: {e}"))?;
    // Safety: a Vulkan instance wgpu-hal made.
    Ok(unsafe { wgpu::Instance::from_hal::<Vulkan>(instance) })
}

fn open_device(
    adapter: &wgpu::Adapter,
    desc: &wgpu::DeviceDescriptor<'_>,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    use ash::vk::Handle;
    let base = BASE.get()?;
    let extensions = leak(
        &base
            .instance
            .vulkan_legacy_device_extensions(base.system)
            .ok()?,
    );
    let open = {
        // Safety: read and opened from here; dropped before wgpu takes it back.
        let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
        let raw = hal.shared_instance().raw_instance().handle().as_raw();
        // Safety: the instance the runtime's extensions were enabled on.
        if let Ok(wanted) = unsafe { base.instance.vulkan_graphics_device(base.system, raw as _) }
            && wanted as u64 != hal.raw_physical_device().as_raw()
        {
            log::warn!("Horizon VR: the runtime wants another GPU than wgpu chose");
        }
        let extensions = extensions.clone();
        // Safety: what wgpu's own request_device does, with the runtime's extensions.
        unsafe {
            hal.open_with_callback(
                desc.required_features,
                &desc.required_limits,
                &desc.memory_hints,
                Some(Box::new(
                    move |args: wgpu::hal::vulkan::CreateDeviceCallbackArgs| {
                        for e in &extensions {
                            if !args.extensions.contains(e) {
                                args.extensions.push(e);
                            }
                        }
                    },
                )),
            )
        }
        .ok()?
    };
    // Safety: opened from this adapter with exactly `desc`'s features.
    let (device, queue) = match unsafe { adapter.create_device_from_hal(open, desc) } {
        Ok(opened) => opened,
        Err(e) => return Some(Err(e)),
    };
    match start_session(base, &device) {
        Ok(state) => *STATE.lock().unwrap_or_else(PoisonError::into_inner) = Some(state),
        Err(e) => log::error!("Horizon VR: {e}"),
    }
    Some(Ok((device, queue)))
}

fn start_session(base: &Base, device: &wgpu::Device) -> Result<State, String> {
    use ash::vk::Handle;
    let info = {
        // Safety: handles read while the guard lives; the device outlives the session.
        let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("not a Vulkan device")?;
        xr::vulkan::SessionCreateInfo {
            instance: hal.shared_instance().raw_instance().handle().as_raw() as _,
            physical_device: hal.raw_physical_device().as_raw() as _,
            device: hal.raw_device().handle().as_raw() as _,
            queue_family_index: hal.queue_family_index(),
            queue_index: 0,
        }
    };
    let instance = &base.instance;
    // Safety: the device and queue the renderer uses, on the runtime's instance extensions.
    let (session, waiter, stream) =
        unsafe { instance.create_session::<xr::Vulkan>(base.system, &info) }
            .map_err(|e| format!("OpenXR session: {e}"))?;
    let local = session
        .create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)
        .map_err(|e| format!("OpenXR space: {e}"))?;
    let head = session
        .create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)
        .map_err(|e| format!("OpenXR space: {e}"))?;
    let actions = actions(instance, &session).map_err(|e| format!("OpenXR actions: {e}"))?;
    Ok(State {
        session,
        waiter,
        stream,
        local,
        head,
        actions,
        running: false,
        focused: false,
        time: None,
        chains: None,
        blit: None,
        turn: 0.0,
        smooth_turn: false,
        stick_centred: true,
        hotbar_centred: true,
        turn_click: false,
        origin: None,
        panel: None,
        trigger_down: false,
        vignette: 0.0,
        moving: 0.0,
    })
}

fn actions(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> xr::Result<Actions> {
    let set = instance.create_action_set("gameplay", "Gameplay", 0)?;
    let mut actions = Actions {
        aim: set.create_action("aim", "Aim", &[])?,
        aim_space: None,
        walk: set.create_action("walk", "Walk", &[])?,
        turn: set.create_action("turn", "Turn", &[])?,
        a: set.create_action("jump", "Jump", &[])?,
        b: set.create_action("sneak", "Sneak", &[])?,
        x: set.create_action("inventory", "Inventory", &[])?,
        y: set.create_action("pause", "Pause", &[])?,
        menu: set.create_action("menu", "Menu", &[])?,
        turn_click: set.create_action("turn_mode", "Turn mode", &[])?,
        walk_click: set.create_action("sprint", "Sprint", &[])?,
        left_trigger: set.create_action("use", "Use", &[])?,
        right_trigger: set.create_action("attack", "Attack", &[])?,
        left_grip: set.create_action("left_grip", "Left grip", &[])?,
        right_grip: set.create_action("right_grip", "Right grip", &[])?,
        set,
    };
    let path = |p: &str| instance.string_to_path(p);
    let a = &actions;
    instance.suggest_interaction_profile_bindings(
        path("/interaction_profiles/oculus/touch_controller")?,
        &[
            xr::Binding::new(&a.aim, path("/user/hand/right/input/aim/pose")?),
            xr::Binding::new(&a.walk, path("/user/hand/left/input/thumbstick")?),
            xr::Binding::new(&a.turn, path("/user/hand/right/input/thumbstick")?),
            xr::Binding::new(&a.a, path("/user/hand/right/input/a/click")?),
            xr::Binding::new(&a.b, path("/user/hand/right/input/b/click")?),
            xr::Binding::new(&a.x, path("/user/hand/left/input/x/click")?),
            xr::Binding::new(&a.y, path("/user/hand/left/input/y/click")?),
            xr::Binding::new(&a.menu, path("/user/hand/left/input/menu/click")?),
            xr::Binding::new(
                &a.turn_click,
                path("/user/hand/right/input/thumbstick/click")?,
            ),
            xr::Binding::new(
                &a.walk_click,
                path("/user/hand/left/input/thumbstick/click")?,
            ),
            xr::Binding::new(
                &a.left_trigger,
                path("/user/hand/left/input/trigger/value")?,
            ),
            xr::Binding::new(
                &a.right_trigger,
                path("/user/hand/right/input/trigger/value")?,
            ),
            xr::Binding::new(&a.left_grip, path("/user/hand/left/input/squeeze/value")?),
            xr::Binding::new(&a.right_grip, path("/user/hand/right/input/squeeze/value")?),
        ],
    )?;
    session.attach_action_sets(&[&actions.set])?;
    actions.aim_space = Some(actions.aim.create_space(
        session,
        xr::Path::NULL,
        xr::Posef::IDENTITY,
    )?);
    Ok(actions)
}

fn vec3(v: xr::Vector3f) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}
fn quat(q: xr::Quaternionf) -> Quat {
    Quat::from_xyzw(q.x, q.y, q.z, q.w).normalize()
}
/// The tracking space's rotation into the camera-relative world, turned by `turn` degrees.
fn world_from_local(turn: f32) -> Mat4 {
    horizon_client::camera::view_matrix(turn, 0.0).inverse()
}
/// Vanilla yaw and pitch (degrees) looking along `d`, a world direction.
fn yaw_pitch(d: Vec3) -> (f32, f32) {
    let d = d.normalize_or(Vec3::NEG_Z);
    (
        (-d.x).atan2(d.z).to_degrees(),
        (-d.y).clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

impl Game<'_> {
    /// Each loop iteration, before the pads: the runtime's events and the
    /// controllers, as the virtual pad, the view's direction and menu clicks.
    pub(super) fn vr_input(&mut self, event_loop: &ActiveEventLoop) {
        let Some(base) = BASE.get() else { return };
        let mut guard = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(vr) = guard.as_mut() else { return };
        let mut events = xr::EventDataBuffer::new();
        while let Ok(Some(event)) = base.instance.poll_event(&mut events) {
            match event {
                xr::Event::SessionStateChanged(e) => match e.state() {
                    xr::SessionState::READY => {
                        vr.running = vr.session.begin(VIEW_TYPE).is_ok();
                    }
                    xr::SessionState::STOPPING => {
                        let _ = vr.session.end();
                        vr.running = false;
                    }
                    xr::SessionState::FOCUSED => vr.focused = true,
                    xr::SessionState::VISIBLE => vr.focused = false,
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        drop(guard);
                        event_loop.exit();
                        return;
                    }
                    _ => {}
                },
                xr::Event::InstanceLossPending(_) => {
                    drop(guard);
                    event_loop.exit();
                    return;
                }
                _ => {}
            }
        }
        self.settings.controller.enabled = true;
        self.focus_lost = None;
        // A short view on the headset: it draws the world twice a frame.
        if self.player.is_none() {
            self.settings.render_distance = self.settings.render_distance.min(8);
        }
        let (Some(time), true, true) = (vr.time, vr.running, vr.focused) else {
            self.pad.injected = PadState::default();
            return;
        };
        let a = &vr.actions;
        if vr.session.sync_actions(&[(&a.set).into()]).is_err() {
            return;
        }
        let s = &vr.session;
        let on = |action: &xr::Action<bool>| {
            action
                .state(s, xr::Path::NULL)
                .is_ok_and(|v| v.current_state)
        };
        let value = |action: &xr::Action<f32>| {
            action
                .state(s, xr::Path::NULL)
                .map_or(0.0, |v| v.current_state)
        };
        let stick = |action: &xr::Action<xr::Vector2f>| {
            action
                .state(s, xr::Path::NULL)
                .map_or([0.0; 2], |v| [v.current_state.x, v.current_state.y])
        };
        let walk = stick(&a.walk);
        let turn = stick(&a.turn);
        let (jump, sneak, inventory) = (on(&a.a), on(&a.b), on(&a.x));
        let pause = on(&a.y) || on(&a.menu);
        let sprint = on(&a.walk_click);
        let turn_click = on(&a.turn_click);
        let use_item = value(&a.left_trigger).max(value(&a.right_grip));
        let attack = value(&a.right_trigger);
        let hotbar_grip = value(&a.left_grip) > 0.7;
        let Ok(head) = vr.head.locate(&vr.local, time) else {
            return;
        };
        let head_ok = head
            .location_flags
            .contains(xr::SpaceLocationFlags::ORIENTATION_VALID);
        let head_rot = quat(head.pose.orientation);
        let head_pos = vec3(head.pose.position);
        let aim = a
            .aim_space
            .as_ref()
            .and_then(|space| space.locate(&vr.local, time).ok())
            .filter(|l| {
                l.location_flags
                    .contains(xr::SpaceLocationFlags::ORIENTATION_VALID)
            });
        let in_menu = self.pad_in_menu();
        let mut pad = PadState::default();
        pad.set(Button::Start, pause);
        if in_menu {
            pad.set(Button::East, sneak);
            vr.moving = 0.0;
            // The panel appears where the head looks as the menu opens.
            if vr.panel.is_none() && head_ok {
                let forward = head_rot * Vec3::NEG_Z;
                let yaw = (-forward.x).atan2(-forward.z);
                let centre = head_pos + Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * PANEL_DISTANCE;
                vr.panel = Some((centre, yaw));
            }
            let trigger = attack > 0.6;
            if let (Some((centre, yaw)), Some(aim), Some(renderer)) =
                (vr.panel, aim, self.renderer.as_ref())
            {
                let (w, h) = (renderer.config.width as f32, renderer.config.height as f32);
                let normal = Quat::from_rotation_y(yaw) * Vec3::Z;
                let right = Quat::from_rotation_y(yaw) * Vec3::X;
                let origin = vec3(aim.pose.position);
                let dir = quat(aim.pose.orientation) * Vec3::NEG_Z;
                let facing = dir.dot(normal);
                if facing < -1e-3 {
                    let t = (centre - origin).dot(normal) / facing;
                    let hit = origin + dir * t - centre;
                    let height = PANEL_WIDTH * h / w;
                    let u = hit.dot(right) / PANEL_WIDTH + 0.5;
                    let v = 0.5 - hit.y / height;
                    let (pressed, released) =
                        (trigger && !vr.trigger_down, !trigger && vr.trigger_down);
                    vr.trigger_down = trigger;
                    drop(guard);
                    if t > 0.0 && (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                        if let Some(id) = self.window.as_ref().map(|w| w.id()) {
                            let position =
                                PhysicalPosition::new(f64::from(u * w), f64::from(v * h));
                            self.window_event(
                                event_loop,
                                id,
                                WindowEvent::CursorMoved {
                                    device_id: DeviceId::dummy(),
                                    position,
                                },
                            );
                            for (go, state) in [
                                (pressed, ElementState::Pressed),
                                (released, ElementState::Released),
                            ] {
                                if go {
                                    self.window_event(
                                        event_loop,
                                        id,
                                        WindowEvent::MouseInput {
                                            device_id: DeviceId::dummy(),
                                            state,
                                            button: MouseButton::Left,
                                        },
                                    );
                                }
                            }
                        }
                    }
                    self.pad.injected = pad;
                    return;
                }
            }
            vr.trigger_down = trigger;
            self.pad.injected = pad;
            return;
        }
        vr.panel = None;
        vr.trigger_down = false;
        // Turning: a flick per step, or smoothly.
        if turn_click && !vr.turn_click {
            vr.smooth_turn = !vr.smooth_turn;
        }
        vr.turn_click = turn_click;
        let mut turning = 0.0f32;
        if vr.smooth_turn {
            if turn[0].abs() > 0.2 {
                turning = turn[0].signum() * (turn[0].abs() - 0.2) / 0.8;
                vr.turn += turning * SMOOTH_DEGREES / 72.0;
            }
        } else if vr.stick_centred && turn[0].abs() > 0.7 {
            vr.turn += SNAP_DEGREES * turn[0].signum();
            vr.stick_centred = false;
        } else if turn[0].abs() < 0.3 {
            vr.stick_centred = true;
        }
        vr.turn = vr.turn.rem_euclid(360.0);
        // The hotbar: right stick up and down, or the left grip.
        if vr.hotbar_centred && turn[1].abs() > 0.7 {
            let button = if turn[1] > 0.0 {
                Button::LeftBumper
            } else {
                Button::RightBumper
            };
            pad.set(button, true);
            self.pad.taps.push(button);
            vr.hotbar_centred = false;
        } else if turn[1].abs() < 0.3 {
            vr.hotbar_centred = true;
        }
        pad.set(
            Button::RightBumper,
            pad.held(Button::RightBumper) || hotbar_grip,
        );
        pad.set(Button::South, jump);
        pad.set(Button::RightStick, sneak);
        pad.set(Button::North, inventory);
        pad.set(Button::LeftStick, sprint);
        pad.triggers = [use_item, attack];
        // Where the player looks: along the right controller (the block it points
        // at is the one broken or used), else along the head.
        let world = world_from_local(vr.turn);
        let head_dir = world.transform_vector3(head_rot * Vec3::NEG_Z);
        let look = aim.map_or(head_dir, |aim| {
            world.transform_vector3(quat(aim.pose.orientation) * Vec3::NEG_Z)
        });
        let (head_yaw, _) = yaw_pitch(head_dir);
        let (yaw, pitch) = yaw_pitch(look);
        // The left stick walks where the head looks; the game moves along `yaw`.
        let d = (head_yaw - yaw).to_radians();
        let (sin, cos) = d.sin_cos();
        pad.left = [walk[0] * cos + walk[1] * sin, walk[1] * cos - walk[0] * sin];
        let speed = (walk[0] * walk[0] + walk[1] * walk[1]).sqrt().min(1.0);
        vr.moving = speed.max(turning.abs());
        if self.screen == Screen::Playing && self.player.is_some() {
            self.yaw = yaw;
            self.pitch = pitch.clamp(-90.0, 90.0);
        }
        self.pad.injected = pad;
    }
}

/// Draws the frame on the headset: both eyes while playing, a floating panel
/// in menus. `None` when the headset isn't running (the frame is drawn as usual).
pub(super) fn render(
    renderer: &mut Renderer,
    frame: &Frame,
    in_menu: bool,
) -> Option<horizon_client::Result<bool>> {
    let base = BASE.get()?;
    let mut guard = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let vr = guard.as_mut()?;
    if !vr.running {
        renderer.set_eye(None);
        return None;
    }
    let state = match vr.waiter.wait() {
        Ok(state) => state,
        Err(e) => return Some(Err(format!("OpenXR wait: {e}").into())),
    };
    if let Err(e) = vr.stream.begin() {
        return Some(Err(format!("OpenXR begin: {e}").into()));
    }
    let time = state.predicted_display_time;
    vr.time = Some(time);
    let end_empty = |vr: &mut State| {
        let _ = vr.stream.end(time, xr::EnvironmentBlendMode::OPAQUE, &[]);
    };
    if !state.should_render {
        end_empty(vr);
        return Some(Ok(true));
    }
    if vr.chains.is_none() {
        match chains(base, vr, &renderer.device) {
            Ok(c) => vr.chains = Some(c),
            Err(e) => {
                log::error!("Horizon VR: {e}");
                end_empty(vr);
                vr.running = false;
                return Some(Err(e.into()));
            }
        }
    }
    let format = FORMAT
        .get()
        .map_or(wgpu::TextureFormat::Rgba8UnormSrgb, |f| f.1);
    if vr.blit.is_none() {
        vr.blit = Some(blit(&renderer.device, format));
    }
    let (w, h) = base.size;
    let rect = xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: w as i32,
            height: h as i32,
        },
    };
    let mut result = Ok(true);
    let target = 0.4 * vr.moving.min(1.0);
    vr.vignette += (target - vr.vignette) * 0.2;
    if in_menu {
        renderer.set_eye(None);
        result = renderer.render(frame);
        copy(renderer, vr, 2, 0.0, format);
        let (centre, yaw) = vr
            .panel
            .unwrap_or((Vec3::new(0.0, 0.0, -PANEL_DISTANCE), 0.0));
        let q = Quat::from_rotation_y(yaw);
        let chains = vr.chains.as_ref().expect("made");
        let layer = xr::CompositionLayerQuad::new()
            .space(&vr.local)
            .eye_visibility(xr::EyeVisibility::BOTH)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&chains[2].handle)
                    .image_rect(rect),
            )
            .pose(xr::Posef {
                orientation: xr::Quaternionf {
                    x: q.x,
                    y: q.y,
                    z: q.z,
                    w: q.w,
                },
                position: xr::Vector3f {
                    x: centre.x,
                    y: centre.y,
                    z: centre.z,
                },
            })
            .size(xr::Extent2Df {
                width: PANEL_WIDTH,
                height: PANEL_WIDTH * h as f32 / w as f32,
            });
        if let Err(e) = vr
            .stream
            .end(time, xr::EnvironmentBlendMode::OPAQUE, &[&layer])
        {
            log::warn!("Horizon VR: end frame: {e}");
        }
        return Some(result);
    }
    let views = match vr.session.locate_views(VIEW_TYPE, time, &vr.local) {
        Ok((_, views)) if views.len() >= 2 => views,
        _ => {
            end_empty(vr);
            return Some(Ok(true));
        }
    };
    let origin = *vr.origin.get_or_insert_with(|| {
        let p = vec3(views[0].pose.position).lerp(vec3(views[1].pose.position), 0.5);
        p
    });
    let world = world_from_local(vr.turn);
    let far = frame.depth_far.min(VIEW_DISTANCE * 1.5);
    let centre = vec3(views[0].pose.position).lerp(vec3(views[1].pose.position), 0.5);
    let mut offset = centre - origin;
    let flat = Vec3::new(offset.x, 0.0, offset.z);
    if flat.length() > HEAD_REACH {
        let keep = flat.normalize() * HEAD_REACH;
        offset.x = keep.x;
        offset.z = keep.z;
    }
    offset.y = offset.y.clamp(-HEAD_REACH * 2.0, HEAD_REACH);
    let eye_view = |i: usize| {
        let p = vec3(views[i].pose.position) - centre + offset;
        let from = world * Mat4::from_rotation_translation(quat(views[i].pose.orientation), p);
        let f = views[i].fov;
        View::eye(
            from.inverse(),
            [
                f.angle_left.tan(),
                f.angle_right.tan(),
                f.angle_down.tan(),
                f.angle_up.tan(),
            ],
            far,
        )
    };
    let eyes = [eye_view(0), eye_view(1)];
    // Both eyes cull with one view: the head's, wide enough for either lens.
    let wide = {
        let f = [views[0].fov, views[1].fov];
        let t = |a: f32| a.tan();
        let from = world * Mat4::from_rotation_translation(quat(views[0].pose.orientation), offset);
        View::eye(
            from.inverse(),
            [
                t(f[0].angle_left.min(f[1].angle_left)) - 0.1,
                t(f[0].angle_right.max(f[1].angle_right)) + 0.1,
                t(f[0].angle_down.min(f[1].angle_down)) - 0.1,
                t(f[0].angle_up.max(f[1].angle_up)) + 0.1,
            ],
            far,
        )
    };
    let vignette = vr.vignette;
    for (i, eye) in eyes.iter().enumerate() {
        renderer.set_eye(Some((*eye, wide)));
        result = renderer.render(frame);
        copy(renderer, vr, i, vignette, format);
    }
    // The eye stays set: the next frame culls its entities with the head's view.
    let chains = vr.chains.as_ref().expect("made");
    let projection = [0, 1].map(|i| {
        xr::CompositionLayerProjectionView::new()
            .pose(views[i].pose)
            .fov(views[i].fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&chains[i].handle)
                    .image_rect(rect),
            )
    });
    // The world is drawn where the eyes were with the head's offset in it; the
    // runtime corrects for the head moving since.
    let layer = xr::CompositionLayerProjection::new()
        .space(&vr.local)
        .views(&projection);
    if let Err(e) = vr
        .stream
        .end(time, xr::EnvironmentBlendMode::OPAQUE, &[&layer])
    {
        log::warn!("Horizon VR: end frame: {e}");
    }
    Some(result)
}

/// The swapchains: one per eye and the menu panel's, wrapped as wgpu textures.
fn chains(base: &Base, vr: &mut State, device: &wgpu::Device) -> Result<[Chain; 3], String> {
    const R8G8B8A8_SRGB: u32 = 43;
    const B8G8R8A8_SRGB: u32 = 50;
    const R8G8B8A8_UNORM: u32 = 37;
    let offered = vr
        .session
        .enumerate_swapchain_formats()
        .map_err(|e| format!("swapchain formats: {e}"))?;
    let (raw, format) = [
        (R8G8B8A8_SRGB, wgpu::TextureFormat::Rgba8UnormSrgb),
        (B8G8R8A8_SRGB, wgpu::TextureFormat::Bgra8UnormSrgb),
        (R8G8B8A8_UNORM, wgpu::TextureFormat::Rgba8Unorm),
    ]
    .into_iter()
    .find(|(raw, _)| offered.contains(raw))
    .ok_or("no 8-bit swapchain format")?;
    let _ = FORMAT.set((raw, format));
    let (w, h) = base.size;
    let make = || -> Result<Chain, String> {
        let handle = vr
            .session
            .create_swapchain(&xr::SwapchainCreateInfo {
                create_flags: xr::SwapchainCreateFlags::EMPTY,
                usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | xr::SwapchainUsageFlags::SAMPLED,
                format: raw as _,
                sample_count: 1,
                width: w,
                height: h,
                face_count: 1,
                array_size: 1,
                mip_count: 1,
            })
            .map_err(|e| format!("swapchain: {e}"))?;
        let raw_images = handle
            .enumerate_images()
            .map_err(|e| format!("swapchain images: {e}"))?;
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("not a Vulkan device")?;
        let images = raw_images
            .into_iter()
            .map(|image| {
                use ash::vk::Handle;
                // Safety: the runtime's image, alive as long as the swapchain, which outlives this.
                unsafe {
                    let texture = hal.texture_from_raw(
                        ash::vk::Image::from_raw(image),
                        &wgpu::hal::TextureDescriptor {
                            label: Some("VR eye"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format,
                            usage: wgpu::TextureUses::COLOR_TARGET,
                            memory_flags: wgpu::hal::MemoryFlags::empty(),
                            view_formats: Vec::new(),
                        },
                        None,
                        wgpu::hal::vulkan::TextureMemory::External,
                    );
                    device.create_texture_from_hal::<Vulkan>(
                        texture,
                        &wgpu::TextureDescriptor {
                            label: Some("VR eye"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        },
                        wgpu::TextureUses::UNINITIALIZED,
                    )
                }
            })
            .collect();
        Ok(Chain { handle, images })
    };
    Ok([make()?, make()?, make()?])
}

const BLIT: &str = r#"
struct Uniforms { vignette: f32, decode: f32, pad0: f32, pad1: f32 }
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var linear: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}
@fragment fn fs(v: Out) -> @location(0) vec4<f32> {
    var c = textureSample(frame, linear, v.uv).rgb;
    if (u.decode > 0.5) {
        c = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    }
    let d = length(v.uv - vec2<f32>(0.5)) * 2.0;
    let shade = 1.0 - u.vignette * smoothstep(0.45, 1.0, d) * 2.0;
    return vec4<f32>(c * clamp(shade, 0.0, 1.0), 1.0);
}
"#;

fn blit(device: &wgpu::Device, format: wgpu::TextureFormat) -> Blit {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("VR copy"),
        source: wgpu::ShaderSource::Wgsl(BLIT.into()),
    });
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("VR copy"),
        entries: &[
            entry(
                0,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            ),
            entry(
                1,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            ),
            entry(
                2,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            ),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("VR copy"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("VR copy"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("VR copy"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("VR copy"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    Blit {
        pipeline,
        layout,
        sampler,
        uniforms,
    }
}

/// Copies the offscreen frame into swapchain `chain`'s next image.
fn copy(
    renderer: &Renderer,
    vr: &mut State,
    chain: usize,
    vignette: f32,
    format: wgpu::TextureFormat,
) {
    let (Some(chains), Some(blit), Some(frame)) = (
        vr.chains.as_mut(),
        vr.blit.as_ref(),
        renderer.offscreen_texture(),
    ) else {
        return;
    };
    let chain = &mut chains[chain];
    let Ok(index) = chain.handle.acquire_image() else {
        return;
    };
    if chain.handle.wait_image(xr::Duration::INFINITE).is_err() {
        return;
    }
    let device = &renderer.device;
    let decode = if format.is_srgb() { 1.0f32 } else { 0.0 };
    renderer.queue.write_buffer(
        &blit.uniforms,
        0,
        bytemuck::cast_slice(&[vignette, decode, 0.0, 0.0]),
    );
    let source = frame.create_view(&wgpu::TextureViewDescriptor::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("VR copy"),
        layout: &blit.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&blit.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: blit.uniforms.as_entire_binding(),
            },
        ],
    });
    let target = chain.images[index as usize].create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("VR copy"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("VR copy"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&blit.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
    renderer.queue.submit([encoder.finish()]);
    let _ = chain.handle.release_image();
}
