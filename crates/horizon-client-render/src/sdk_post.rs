//! Mods' post-processing passes (mod ABI 4, `render` in horizon-mod-sdk): each mod's WGSL fragment shader, after
//! the prelude (`sdk_post/prelude.wgsl`), drawn over the world once it is drawn, before the hand and the HUD, reading
//! a copy of the world's picture and its depth, the frame's camera, the mod's 16 parameters and its data words, and
//! optionally the block sprite arrays. Any mod ships its own shaders this way; the game needs no change for a new one.
//!
//! Ported from Stink-O/orbital-laser v0.5.10 (MIT, Copyright (c) 2026 Stink-O): native/engine-shim/post_hooks.rs and
//! mod_post.rs (the Stink-O/horizon fork's gfx/mod_post.rs), made to hold a pass for each mod.

pub mod pass;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use pass::{FrameUniform, ModPost};

/// What the game thread hands the renderer: the pass's source and its version, its parameters, its data and the
/// data's version, and the time of day (0 sunrise, 0.25 noon) at the last tick.
#[derive(Clone)]
pub struct Pass {
    pub source: Arc<str>,
    pub version: u64,
    pub params: [f32; 16],
    pub data: Option<(Arc<[u32]>, u64)>,
    pub day_time: u64,
}

/// What the pass reads of the frame being drawn, as the fork's `draw_frame` read it.
#[derive(Clone, Copy)]
struct Captured {
    yaw: f32,
    pitch: f32,
    fov_degrees: f32,
    bob: glam::Mat4,
    spin: glam::Mat4,
    depth_far: f32,
    distance: f32,
    sun_angle: f32,
    sky_color: [f32; 3],
    fog_color: [f32; 4],
    partial_tick: f32,
    title: bool,
}

/// Each mod's pass, by mod id.
static PASSES: Mutex<BTreeMap<String, Pass>> = Mutex::new(BTreeMap::new());
static CAPTURED: Mutex<Option<Captured>> = Mutex::new(None);
/// Each mod's drawer, by mod id.
static DRAWERS: Mutex<BTreeMap<String, Drawer>> = Mutex::new(BTreeMap::new());

/// The renderer's side: the fork's `ModPost` for the target's format, and the copy of the world's picture it reads.
struct Drawer {
    format: wgpu::TextureFormat,
    post: ModPost,
    input: Option<([u32; 2], wgpu::Texture, wgpu::TextureView)>,
}

/// The game thread: mod `id`'s pass to draw from the next frame on, or none.
pub fn set(id: &str, pass: Option<Pass>) {
    if let Ok(mut p) = PASSES.lock() {
        match pass {
            Some(pass) => {
                p.insert(id.to_owned(), pass);
            }
            None => {
                p.remove(id);
            }
        }
    }
}

/// Whether any mod has a pass.
pub fn active() -> bool {
    PASSES.lock().is_ok_and(|p| !p.is_empty())
}

/// At the start of `Renderer::render`: what the passes read of this frame.
pub fn capture(frame: &crate::gfx::Frame<'_>) {
    // the fog colour as `draw_frame` works it out for the world's globals
    let fog_color = if let Some(f) = &frame.fog {
        f.color
    } else if frame.underwater {
        [frame.water_fog[0], frame.water_fog[1], frame.water_fog[2], 1.0]
    } else {
        [frame.sky.fog_color[0], frame.sky.fog_color[1], frame.sky.fog_color[2], 1.0]
    };
    let captured = Captured {
        yaw: frame.yaw,
        pitch: frame.pitch,
        fov_degrees: frame.fov_degrees,
        bob: frame.bob,
        spin: frame.spin,
        depth_far: frame.depth_far,
        distance: frame.distance,
        sun_angle: frame.sky.sun_angle,
        sky_color: frame.sky.sky_color,
        fog_color,
        partial_tick: frame.weather_time.values[2],
        title: frame.title,
    };
    if let Ok(mut c) = CAPTURED.lock() {
        *c = Some(captured);
    }
}

/// `native_render_needs`: the world's depth, while a pass is set.
pub fn needs(needs: &mut crate::gfx::native_render::RenderNeeds) {
    if active() {
        needs.world_depth = true;
    }
}

/// `native_world_drawn`: the pass over the world.
pub fn draw(world: &mut crate::gfx::native_render::WorldDrawn<'_>) {
    let passes: Vec<(String, Pass)> = match PASSES.lock() {
        Ok(p) => p.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Err(_) => return,
    };
    if passes.is_empty() {
        return;
    }
    let Some(frame) = CAPTURED.lock().ok().and_then(|c| *c) else {
        return;
    };
    if frame.title {
        return;
    }
    let Ok(mut drawers) = DRAWERS.lock() else {
        return;
    };
    drawers.retain(|id, _| passes.iter().any(|(p, _)| p == id));
    for (id, pass) in passes {
        let drawer = drawers.entry(id.clone()).or_insert_with(|| Drawer {
            format: world.color_format,
            post: ModPost::new(world.device, world.color_format),
            input: None,
        });
        if drawer.format != world.color_format {
            *drawer = Drawer { format: world.color_format, post: ModPost::new(world.device, world.color_format), input: None };
        }
        draw_one(world, &id, &pass, &frame, drawer);
    }
}

fn draw_one(
    world: &mut crate::gfx::native_render::WorldDrawn<'_>,
    id: &str,
    pass: &Pass,
    frame: &Captured,
    drawer: &mut Drawer,
) {
    // The block sprite arrays (the fork's capability `atlas`), bindings 8 to 16, sampled sharp as the fork's terrain
    // sampler did: upstream's `WorldDrawn::block_sprites`.
    let sprites = &world.block_sprites;
    drawer.post.set_atlas(sprites.views, sprites.nearest);
    // (a strike's frame waits for its pass; the frame that builds it ahead, its parameters all nothing, doesn't)
    let strike = pass.params.iter().any(|&p| p != 0.0);
    if !drawer.post.prepare(world.device, pass.version, &pass.source, true, strike) {
        return;
    }
    let size = world.size;
    if world.depth_size != size {
        // (a world drawn at a lower render scale: the prelude samples depth by uv, so the pass still lines up; a pass
        // reading depth texel by texel at the picture's size reads it a little off)
        static SAID: std::sync::Once = std::sync::Once::new();
        SAID.call_once(|| log::info!("[mod {id}] the world draws at {:?} under a {:?} picture", world.depth_size, size));
    }
    if drawer.input.as_ref().is_none_or(|(s, _, _)| *s != size) {
        let texture = world.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mod post pass input"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: world.color.format(),
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[world.color_format],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(world.color_format),
            ..Default::default()
        });
        drawer.input = Some((size, texture, view));
    }
    let (_, input, input_view) = drawer.input.as_ref().expect("made above");
    world.encoder.copy_texture_to_texture(
        world.color.as_image_copy(),
        input.as_image_copy(),
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    // The fork's uniform (gfx.rs, the mod pass): the bob's turn in `view`, its shift in `camera_position`, the
    // portal and Nausea skew in the projection.
    let aspect = size[0] as f32 / size[1].max(1) as f32;
    let turn = glam::Mat4::from_mat3(glam::Mat3::from_mat4(frame.bob));
    let view = turn * crate::camera::view_matrix(frame.yaw, frame.pitch);
    let projection =
        glam::Mat4::perspective_rh(frame.fov_degrees.to_radians(), aspect, frame.depth_far, 0.05)
            * frame.spin;
    let eye = view.inverse().transform_point3(-frame.bob.w_axis.truncate());
    let sun_angle = frame.sun_angle.to_radians();
    let sun = glam::Vec3::new(0.0, sun_angle.cos(), -sun_angle.sin());
    let sky = frame.sky_color;
    let day_fraction =
        ((pass.day_time % 24_000) as f32 + frame.partial_tick) / 24_000.0;
    let mut uniform = FrameUniform {
        view: view.to_cols_array_2d(),
        projection: projection.to_cols_array_2d(),
        inverse_view: view.inverse().to_cols_array_2d(),
        inverse_projection: projection.inverse().to_cols_array_2d(),
        camera_position: [
            (world.camera.x + f64::from(eye.x)) as f32,
            (world.camera.y + f64::from(eye.y)) as f32,
            (world.camera.z + f64::from(eye.z)) as f32,
            0.0,
        ],
        // (what the f32 camera position leaves out: its rounding, far from the world's origin)
        camera_rest: {
            let at = [
                world.camera.x + f64::from(eye.x),
                world.camera.y + f64::from(eye.y),
                world.camera.z + f64::from(eye.z),
            ];
            let rest = |v: f64| (v - f64::from(v as f32)) as f32;
            [rest(at[0]), rest(at[1]), rest(at[2]), 0.0]
        },
        time: [0.0, 0.0, day_fraction, frame.partial_tick],
        sun: [sun.x, sun.y, sun.z, sun_angle],
        sky_color: [sky[0], sky[1], sky[2], 1.0],
        fog_color: frame.fog_color,
        view_params: [
            0.05,
            frame.depth_far,
            frame.distance,
            frame.fov_degrees.to_radians(),
        ],
        ..Default::default()
    };
    for (to, from) in uniform.mod_params.iter_mut().zip(pass.params.chunks(4)) {
        to.copy_from_slice(from);
    }
    let data = pass.data.as_ref().map(|(d, v)| (&d[..], *v));
    drawer.post.set_data(world.device, world.queue, data);
    drawer.post.run(
        world.device,
        world.queue,
        world.encoder,
        input_view,
        world.depth,
        size,
        uniform,
        world.color_view,
    );
}
