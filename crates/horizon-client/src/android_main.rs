//! Horizon on Android: the crate root of `horizon-android` (a NativeActivity
//! cdylib, ANDROID.md). It sits beside `main.rs` so `mod game;` compiles the
//! same game; this file only replaces `main` with `android_main`.
#![cfg(target_os = "android")]
#![allow(dead_code)]
mod android_msa;
mod android_setup;
use horizon_client::game;

use horizon_client::{crash, logging};
use horizon_client_render::graphics::GraphicsApi;
use winit::platform::android::activity::AndroidApp;

static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

/// Called by android-activity on its own thread once the activity starts.
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    std::hint::black_box(&MOD_API_LEVEL);
    // The game's home: the app's external files folder
    // (/sdcard/Android/data/dev.horizonlab.horizon/files), which a file manager or
    // `adb push` can reach; internal storage when there is none.
    let home = app
        .external_data_path()
        .or_else(|| app.internal_data_path())
        .unwrap_or_else(|| "/data/local/tmp/horizon".into());
    let _ = std::fs::create_dir_all(&home);
    // SAFETY: nothing else runs yet; the game reads these on this thread.
    unsafe {
        std::env::set_var("HORIZON_HOME", &home);
        if std::env::var_os("HORIZON_RESOURCES").is_none() {
            std::env::set_var("HORIZON_RESOURCES", home.join("resources"));
        }
        std::env::set_var("HOME", &home);
        if let Some(internal) = app.internal_data_path() {
            std::env::set_var("HORIZON_CACHE", internal.join("horizon-cache"));
        }
        std::env::set_var("TMPDIR", app.internal_data_path().unwrap_or(home.clone()).join("tmp"));
        // `spikes.txt` in the game folder (a number of milliseconds) turns on HORIZON_SPIKES:
        // frames slower than that log their timings, without a debug build.
        if let Ok(text) = std::fs::read_to_string(home.join("spikes.txt"))
            && text.trim().parse::<f32>().is_ok()
        {
            std::env::set_var("HORIZON_SPIKES", text.trim());
        }
    }
    let _ = std::fs::create_dir_all(home.join("..").join("cache"));
    let _ = std::env::set_current_dir(&home);
    // The folders players drop things into, ready in the Files app's Horizon root.
    for dir in ["saves", "resourcepacks", "mods", "shaderpacks"] {
        let _ = std::fs::create_dir_all(home.join(dir));
    }
    logging::init(&home, "Render thread");
    log::info!("Horizon {} (Android) home {}", crash::version(), home.display());
    crash::install(home.clone(), "client");
    if let Some(internal) = app.internal_data_path() {
        apply_vulkan_driver(&internal);
        apply_safe_mode(&internal, &home);
        android_msa::prepare(&internal, &home);
    }
    let _ = game::ANDROID_APP.set(app.clone());
    // A panic here is already in the crash report (crash::install). Catch it so the activity is
    // still finished below: a NativeActivity whose loop died shows a dead window otherwise.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(game::run_with)) {
        Ok(Err(error)) => {
            log::error!("Horizon: {error}");
            crash::report_error(&home, "client", &*error);
        }
        Ok(Ok(())) => clear_launch_marks(&app),
        Err(_) => log::error!("Horizon: the game thread panicked"),
    }
    logging::flush();
    // Quit Game only leaves the native loop. Finish the activity or the next
    // launch resumes a dead window (black, then it minimizes).
    game::finish_activity();
}

/// `vulkan-driver.txt` in internal storage (Profile -> Graphics driver, experimental) names a
/// custom Vulkan driver `.so` (Turnip). The renderer opens it itself and goes back to the system
/// driver when it can't (wgpu-hal `vulkan/instance.rs`).
///
/// A launch that dies before the game has been up for a while leaves `vulkan-driver.pending`
/// behind; the next launch sees it, turns the custom driver off and leaves a note for the
/// profile screen, so a bad driver can never lock the game out.
fn apply_vulkan_driver(internal: &std::path::Path) {
    let choice = internal.join("vulkan-driver.txt");
    let pending = internal.join("vulkan-driver.pending");
    if pending.exists() {
        let _ = std::fs::remove_file(&pending);
        if choice.exists() {
            log::warn!("Horizon: the custom Vulkan driver did not start last time; back to the system driver");
            let _ = std::fs::rename(&choice, internal.join("vulkan-driver.failed"));
            return;
        }
    }
    let Ok(text) = std::fs::read_to_string(&choice) else {
        return;
    };
    let path = text.trim();
    if path.is_empty() {
        return;
    }
    if !std::path::Path::new(path).is_file() {
        log::warn!("Horizon: Vulkan driver missing: {path}");
        return;
    }
    let _ = std::fs::write(&pending, b"1");
    log::info!("Horizon: custom Vulkan driver {path}");
    // SAFETY: still before any other thread starts.
    unsafe { std::env::set_var("HORIZON_VULKAN_LIB", path) };
    let pending = pending.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(25));
        let _ = std::fs::remove_file(pending);
    });
}

/// The player quit on purpose: however short the launch, it did not fail. Without this, opening
/// the game and quitting twice within 25 seconds turned safe start on.
fn clear_launch_marks(app: &AndroidApp) {
    if let Some(internal) = app.internal_data_path() {
        let _ = std::fs::remove_file(internal.join("launch.pending"));
        let _ = std::fs::remove_file(internal.join("vulkan-driver.pending"));
    }
}

/// Safe start: a launch that died within its first 25 seconds (with the system driver)
/// leaves `launch.pending`; after two such launches in a row the next ones start on
/// OpenGL ES with GPU occlusion culling off. Some drivers (Adreno 740 on certain firmware,
/// Mali on old Android) fail inside Vulkan before the first frame. `safe-mode.txt` in
/// internal storage keeps it on until the profile screen's "Try Vulkan again" deletes it, or
/// the player picks another graphics API in the video settings than the one that failed
/// (`safe-mode.txt` names it): that one is tried, and two more failed launches bring safe
/// start back.
fn apply_safe_mode(internal: &std::path::Path, home: &std::path::Path) {
    let pending = internal.join("launch.pending");
    let forced = internal.join("safe-mode.txt");
    let mut failures: u32 = std::fs::read_to_string(&pending)
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    let chosen = chosen_api(home);
    if let Ok(note) = std::fs::read_to_string(&forced) {
        // A note from before 0.4.6 names no API: only Vulkan picked since lifts it.
        let failed = note
            .lines()
            .find_map(|l| l.strip_prefix("api:"))
            .map(|name| GraphicsApi::from_name(name).unwrap_or_default());
        let changed = match failed {
            Some(failed) => failed != chosen,
            None => chosen == GraphicsApi::Vulkan,
        };
        if chosen != GraphicsApi::OpenGl && changed {
            log::info!(
                "Horizon: {} picked in the video settings after safe start; trying it again",
                chosen.label()
            );
            let _ = std::fs::remove_file(&forced);
            failures = 0;
        }
    }
    let safe = forced.exists() || failures >= 2;
    if failures >= 2 && !forced.exists() {
        let _ = std::fs::write(
            &forced,
            format!(
                "Vulkan failed to start twice; delete this file to try it again\napi:{}\n",
                chosen.name()
            ),
        );
    }
    if safe && std::env::var_os("HORIZON_GRAPHICS_API").is_none() {
        log::warn!("Horizon: safe start (OpenGL ES, no GPU culling) after {failures} failed launches");
        // SAFETY: still before any other thread starts.
        unsafe {
            std::env::set_var("HORIZON_GRAPHICS_API", "opengl");
            std::env::set_var("HORIZON_GPU_CULL", "0");
        }
    }
    let _ = std::fs::write(&pending, format!("{}", failures + 1));
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(25));
        let _ = std::fs::remove_file(pending);
    });
}

/// The graphics API the video settings name (horizon-graphics.txt in the game folder, which
/// `Settings::load` reads beside options.txt), read by the game's own parser: Auto when unset.
fn chosen_api(home: &std::path::Path) -> GraphicsApi {
    std::fs::read_to_string(home.join(horizon_client_render::graphics::FILE))
        .map(|text| horizon_client_render::graphics::Graphics::parse(&text).api)
        .unwrap_or_default()
}
