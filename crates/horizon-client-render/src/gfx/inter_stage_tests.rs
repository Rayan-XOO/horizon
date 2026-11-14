//! Every render pipeline's stage interface against the weakest GPUs' limit (#8195).
//!
//! OpenGL 3.3 drivers (a GeForce 9100M G) allow `max_inter_stage_shader_variables` = 15, and wgpu
//! counts the fragment stage's built-in inputs (`front_facing`, `sample_index`, ...) against it.
//! A pipeline over that limit panics at start-up on such a GPU, with no way to run the game, so
//! this checks the shaders with naga, without a GPU.

use std::path::{Path, PathBuf};

/// `Limits::downlevel_defaults().max_inter_stage_shader_variables`, the least a GPU offers.
const LIMIT: u32 = 15;

/// One entry point's variables: the user-defined `@location`s and the built-ins wgpu counts.
#[derive(Debug, PartialEq, Eq)]
struct Interface {
    entry: String,
    /// Vertex outputs or fragment inputs, user-defined.
    user: u32,
    /// The fragment stage's `front_facing`, `sample_index`, `sample_mask` and `primitive_index`
    /// inputs, which wgpu counts as one variable each; 0 for a vertex stage.
    built_ins: u32,
}

impl Interface {
    fn total(&self) -> u32 {
        self.user + self.built_ins
    }
}

/// The bindings an argument or a result carries, a struct's members flattened.
fn bindings(
    module: &naga::Module,
    binding: &Option<naga::Binding>,
    ty: naga::Handle<naga::Type>,
) -> Vec<naga::Binding> {
    if let Some(binding) = binding {
        return vec![binding.clone()];
    }
    match &module.types[ty].inner {
        naga::TypeInner::Struct { members, .. } => {
            members.iter().filter_map(|m| m.binding.clone()).collect()
        }
        _ => Vec::new(),
    }
}

fn is_location(binding: &naga::Binding) -> bool {
    matches!(binding, naga::Binding::Location { .. })
}

/// The stage interface of every vertex and fragment entry point of `source`.
fn interfaces(source: &str) -> Vec<Interface> {
    let module = naga::front::wgsl::parse_str(source).unwrap();
    let mut out = Vec::new();
    for entry in &module.entry_points {
        let function = &entry.function;
        let (user, built_ins) = match entry.stage {
            naga::ShaderStage::Vertex => {
                let outputs: Vec<_> = function
                    .result
                    .iter()
                    .flat_map(|r| bindings(&module, &r.binding, r.ty))
                    .collect();
                (outputs.iter().filter(|b| is_location(b)).count(), 0)
            }
            naga::ShaderStage::Fragment => {
                let inputs: Vec<_> = function
                    .arguments
                    .iter()
                    .flat_map(|a| bindings(&module, &a.binding, a.ty))
                    .collect();
                let built_ins = inputs
                    .iter()
                    .filter(|b| {
                        matches!(
                            b,
                            naga::Binding::BuiltIn(
                                naga::BuiltIn::FrontFacing
                                    | naga::BuiltIn::SampleIndex
                                    | naga::BuiltIn::SampleMask
                                    | naga::BuiltIn::PrimitiveIndex
                            )
                        )
                    })
                    .count();
                (inputs.iter().filter(|b| is_location(b)).count(), built_ins)
            }
            _ => continue,
        };
        out.push(Interface {
            entry: entry.name.clone(),
            user: user as u32,
            built_ins: built_ins as u32,
        });
    }
    out
}

/// The shaders as the renderer assembles them, by the `.wgsl` files each is made of.
fn composed() -> Vec<(&'static str, Vec<&'static str>, String)> {
    let entity = super::entity_shader();
    vec![
        (
            "HDR",
            vec!["hdr.wgsl", "hdr_light.wgsl"],
            super::hdr::shader(),
        ),
        ("terrain", vec!["terrain.wgsl"], super::terrain_shader()),
        (
            "stars",
            vec!["stars.wgsl"],
            include_str!("../hdr_light.wgsl").to_owned() + include_str!("stars.wgsl"),
        ),
        (
            "custom sky",
            vec!["custom_sky.wgsl"],
            include_str!("../hdr_light.wgsl").to_owned() + include_str!("custom_sky.wgsl"),
        ),
        ("entity", vec!["entity.wgsl"], entity.clone()),
        (
            "GPU culling occlusion test",
            vec!["gpu_culling.wgsl", "gpu_occlusion.wgsl"],
            include_str!("gpu_culling.wgsl").to_owned() + include_str!("gpu_occlusion.wgsl"),
        ),
        (
            "static entity",
            vec!["entity.wgsl", "static_entity.wgsl"],
            entity.clone() + include_str!("../static_entity.wgsl"),
        ),
        (
            "improved transparency clouds",
            vec!["entity.wgsl", "oit.wgsl", "oit_clouds.wgsl"],
            entity.clone() + include_str!("../oit.wgsl") + include_str!("../oit_clouds.wgsl"),
        ),
        (
            "improved transparency features",
            vec!["entity.wgsl", "oit.wgsl", "oit_features.wgsl"],
            entity + include_str!("../oit.wgsl") + include_str!("../oit_features.wgsl"),
        ),
        (
            "improved transparency world",
            vec!["terrain.wgsl", "oit.wgsl", "oit_world.wgsl"],
            super::terrain_shader()
                + include_str!("../oit.wgsl")
                + include_str!("../oit_world.wgsl"),
        ),
        (
            "visibility buffer",
            vec!["terrain.wgsl", "visbuffer.wgsl"],
            super::terrain_shader() + include_str!("../visbuffer.wgsl"),
        ),
        // Ray tracing's unpacking of compact sections (gfx/ray_scene.rs's `Unpacker`).
        (
            "ray tracing unpack",
            vec!["terrain.wgsl", "rt_unpack.wgsl"],
            super::surface_setup::ray_query::ray_scene::unpack_shader(),
        ),
        // A native mod's world warp (gfx/world_warp.rs): its copies with the plainest warp.
        (
            "world warp terrain",
            vec![
                "terrain.wgsl",
                "world_warp.wgsl",
                "world_warp_fragment.wgsl",
            ],
            super::world_warp::terrain_shader(super::world_warp::IDENTITY).unwrap(),
        ),
        (
            "world warp entities",
            vec!["entity.wgsl", "static_entity.wgsl", "world_warp.wgsl"],
            super::world_warp::entity_shader(super::world_warp::IDENTITY).unwrap(),
        ),
        (
            "world primitives",
            vec!["world_primitives.wgsl"],
            super::world_primitives::shader(),
        ),
        (
            "improved transparency world border",
            vec!["border.wgsl", "oit.wgsl", "oit_border.wgsl"],
            super::border_shader()
                + include_str!("../oit.wgsl")
                + include_str!("../oit_border.wgsl"),
        ),
    ]
}

fn wgsl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            wgsl_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "wgsl") {
            out.push(path);
        }
    }
}

/// Every shader: the assembled ones, and each `.wgsl` file that stands alone. A file that does
/// not (it needs another's declarations) must be part of an assembled one, so that a new shader
/// cannot slip past the audit.
fn audited() -> Vec<(String, Vec<Interface>)> {
    let composed = composed();
    let mut files = Vec::new();
    wgsl_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    files.sort();
    let mut out: Vec<(String, Vec<Interface>)> = composed
        .iter()
        .map(|(name, _, source)| ((*name).to_owned(), interfaces(source)))
        .collect();
    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(&file).unwrap();
        let alone = naga::front::wgsl::parse_str(&source).is_ok();
        let part = composed.iter().any(|(_, parts, _)| parts.contains(&&*name));
        assert!(
            alone || part,
            "{name} needs other files' declarations: add its assembly to `composed`"
        );
        if alone {
            out.push((name, interfaces(&source)));
        }
    }
    out
}

#[test]
fn no_pipeline_exceeds_the_weakest_gpus_inter_stage_limit() {
    let mut over = Vec::new();
    for (shader, entries) in audited() {
        for e in entries {
            if e.total() > LIMIT {
                over.push(format!(
                    "{shader}::{}: {} user-defined + {} built-in = {} > {LIMIT}",
                    e.entry,
                    e.user,
                    e.built_ins,
                    e.total()
                ));
            }
        }
    }
    assert!(over.is_empty(), "over the limit:\n{}", over.join("\n"));
}

/// The audit counts what wgpu counts (the failure of #8195: 15 user-defined + `FrontFacing`).
#[test]
fn a_front_facing_input_counts_against_the_limit() {
    let source = "struct Io { @builtin(position) p: vec4<f32>, @location(0) a: vec4<f32>, @location(1) b: f32 }
        @vertex fn vs() -> Io { return Io(vec4<f32>(0.0), vec4<f32>(0.0), 0.0); }
        @fragment fn fs(in: Io, @builtin(front_facing) f: bool) -> @location(0) vec4<f32> { return in.a; }";
    assert_eq!(
        interfaces(source),
        vec![
            Interface {
                entry: "vs".into(),
                user: 2,
                built_ins: 0
            },
            Interface {
                entry: "fs".into(),
                user: 2,
                built_ins: 1
            },
        ]
    );
}

/// The uniform and storage buffers an entry point uses whose type is not a multiple of 16
/// bytes: `(compute, "entry: group:binding name (size)")` for each. wgpu refuses such a render
/// pipeline on a device without `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`, which
/// ANGLE is: Samsung's OpenGL ES (Xclipse 550, Galaxy A57) panicked at start-up on `Horizon HDR
/// emission thresholds`. wgpu-core checks render pipelines only (`create_render_pipeline`), so
/// compute entry points are listed apart. A runtime-sized array counts one element, as wgpu
/// counts it.
fn unaligned_buffers(source: &str) -> Vec<(bool, String)> {
    let module = naga::front::wgsl::parse_str(source).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
    let mut out = Vec::new();
    for (i, entry) in module.entry_points.iter().enumerate() {
        let compute = !matches!(
            entry.stage,
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment
        );
        let used = info.get_entry_point(i);
        for (handle, var) in module.global_variables.iter() {
            let (Some(binding), false) = (&var.binding, used[handle].is_empty()) else {
                continue;
            };
            if !matches!(
                var.space,
                naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. }
            ) {
                continue;
            }
            let size = module.types[var.ty].inner.size(module.to_ctx());
            if size % 16 != 0 {
                out.push((
                    compute,
                    format!(
                        "{}: {}:{} {} ({size} bytes)",
                        entry.name,
                        binding.group,
                        binding.binding,
                        var.name.as_deref().unwrap_or("?")
                    ),
                ));
            }
        }
    }
    out
}

#[test]
fn no_render_pipeline_binds_a_buffer_off_16_bytes() {
    let composed = composed();
    let mut sources: Vec<(String, String)> = composed
        .iter()
        .map(|(name, _, source)| ((*name).to_owned(), source.clone()))
        .collect();
    let mut files = Vec::new();
    wgsl_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    files.sort();
    let mut skipped = Vec::new();
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(file).unwrap();
        if naga::front::wgsl::parse_str(&source).is_ok() {
            sources.push((name, source));
        } else if !composed.iter().any(|(_, parts, _)| parts.contains(&&*name)) {
            skipped.push(name);
        }
    }
    // Every file is checked, alone or in the shader it is part of: none slips past.
    assert!(skipped.is_empty(), "not checked: {skipped:?}");
    assert!(
        sources.len() >= files.len().min(30),
        "only {} shaders checked of {} files",
        sources.len(),
        files.len()
    );
    // terrain.wgsl needs hdr_light.wgsl's declarations: checked as the assembled "terrain".
    for must in ["terrain", "hdr_stats.wgsl", "visibility buffer", "GPU culling occlusion test"] {
        assert!(sources.iter().any(|(n, _)| n == must), "{must} is not checked");
    }
    let (mut render, mut compute) = (Vec::new(), Vec::new());
    for (shader, source) in &sources {
        for (is_compute, e) in unaligned_buffers(source) {
            let line = format!("{shader}::{e}");
            if is_compute {
                compute.push(line);
            } else {
                render.push(line);
            }
        }
    }
    render.dedup();
    compute.dedup();
    if !compute.is_empty() {
        println!(
            "compute entry points (wgpu does not check these):\n{}",
            compute.join("\n")
        );
    }
    assert!(
        render.is_empty(),
        "not a multiple of 16 bytes:\n{}",
        render.join("\n")
    );
}

#[test]
fn a_two_float_uniform_is_off_16_bytes() {
    let source = "struct P { a: f32, b: f32 }
        @group(0) @binding(0) var<uniform> p: P;
        @fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(p.a, p.b, 0.0, 1.0); }";
    assert_eq!(
        unaligned_buffers(source),
        vec![(false, "fs: 0:0 p (8 bytes)".to_owned())]
    );
    let padded = "struct W { @align(16) w: array<u32> }
        @group(0) @binding(0) var<storage, read> l: W;
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            return vec4<f32>(f32(l.w[i]));
        }";
    assert!(unaligned_buffers(padded).is_empty());
}

/// Every shader parses, validates and translates for each backend the game ships: SPIR-V
/// (Vulkan) and GLSL ES 3.10 / 3.20 (OpenGL ES, ANGLE). A mistake here is no test failure in the
/// renderer's own tests (they run on desktop Vulkan, or not at all), but a panic at start-up on
/// a phone: 0.4.7's terrain shader read a wrapped storage array without its `.w`
/// ("Invalid sub-access into type"), which only `create_shader_module` reported.
#[test]
fn every_shader_builds_for_vulkan_and_opengl_es() {
    use naga::back::glsl;
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    let composed = composed();
    let mut sources: Vec<(String, String)> = composed
        .iter()
        .map(|(name, _, source)| ((*name).to_owned(), source.clone()))
        .collect();
    let mut files = Vec::new();
    wgsl_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    files.sort();
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(file).unwrap();
        if naga::front::wgsl::parse_str(&source).is_ok()
            || !composed.iter().any(|(_, parts, _)| parts.contains(&&*name))
        {
            sources.push((name, source));
        }
    }
    // The default override specializes written light away: its active form too.
    let written: Vec<(String, String)> = sources
        .iter()
        .filter(|(_, source)| source.contains("override HDR_WRITE_LIGHT: bool = false;"))
        .map(|(name, source)| {
            (
                format!("{name} (HDR_WRITE_LIGHT)"),
                source.replace(
                    "override HDR_WRITE_LIGHT: bool = false;",
                    "override HDR_WRITE_LIGHT: bool = true;",
                ),
            )
        })
        .collect();
    sources.extend(written);
    let mut failed = Vec::new();
    for (name, source) in &sources {
        let module = match naga::front::wgsl::parse_str(source) {
            Ok(m) => m,
            Err(e) => {
                failed.push(format!("{name}: {}", e.emit_to_string(source)));
                continue;
            }
        };
        let info = match Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module) {
            Ok(info) => info,
            Err(e) => {
                failed.push(format!("{name}: {}", e.emit_to_string(source)));
                continue;
            }
        };
        for entry in &module.entry_points {
            // What wgpu does before any backend: the overrides take their defaults here.
            let constants = Default::default();
            let (module, info) = match naga::back::pipeline_constants::process_overrides(
                &module,
                &info,
                Some((entry.stage, &entry.name)),
                &constants,
            ) {
                Ok(done) => done,
                Err(e) => {
                    failed.push(format!("{name}::{} (overrides): {e}", entry.name));
                    continue;
                }
            };
            let spv = naga::back::spv::Options::default();
            let spv_pipeline = naga::back::spv::PipelineOptions {
                shader_stage: entry.stage,
                entry_point: entry.name.clone(),
            };
            if let Err(e) = naga::back::spv::write_vec(&module, &info, &spv, Some(&spv_pipeline)) {
                failed.push(format!("{name}::{} (SPIR-V): {e}", entry.name));
            }
            // Never built on OpenGL: frame generation (Vulkan only) and the custom-item glint
            // (`CitGlintPass::new` keeps the vanilla glint there).
            if name == "interpolation.wgsl" || (name.starts_with("hud.wgsl") && entry.name == "fs_cit_glint") {
                continue;
            }
            // Compute stages need GLSL ES 3.10; both versions the GL backend asks for.
            for version in [310, 320] {
                let options = glsl::Options {
                    version: glsl::Version::Embedded { version, is_webgl: false },
                    ..Default::default()
                };
                let pipeline = glsl::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                    multiview: None,
                };
                let mut out = String::new();
                let written = glsl::Writer::new(
                    &mut out,
                    &module,
                    &info,
                    &options,
                    &pipeline,
                    naga::proc::BoundsCheckPolicies::default(),
                )
                .and_then(|mut w| w.write().map(|_| ()));
                if let Err(e) = written {
                    failed.push(format!("{name}::{} (GLSL ES {version}): {e}", entry.name));
                }
            }
        }
    }
    assert!(sources.len() >= 30, "only {} shaders checked", sources.len());
    assert!(failed.is_empty(), "{} failed:\n{}", failed.len(), failed.join("\n"));
}
