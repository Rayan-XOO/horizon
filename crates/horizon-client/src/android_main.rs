//! Horizon on Android: the crate root of `horizon-android` (a NativeActivity
//! cdylib, ANDROID.md). It sits beside `main.rs` so `mod game;` compiles the
//! same game; this file only replaces `main` with `android_main`.
#![cfg(target_os = "android")]
#![allow(dead_code)]
mod android_msa;
mod android_setup;
mod game;

use horizon_client::{crash, logging};
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
    }
    let _ = std::fs::create_dir_all(home.join("..").join("cache"));
    let _ = std::env::set_current_dir(&home);
    logging::init(&home, "Render thread");
    log::info!("Horizon {} (Android) home {}", crash::version(), home.display());
    crash::install(home.clone(), "client");
    if let Some(internal) = app.internal_data_path() {
        apply_vulkan_driver(&internal);
        android_msa::prepare(&internal, &home);
    }
    let _ = game::ANDROID_APP.set(app.clone());
    // A panic here is already in the crash report (crash::install). Catch it so the activity is
    // still finished below: a NativeActivity whose loop died shows a dead window otherwise.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(game::run)) {
        Ok(Err(error)) => {
            log::error!("Horizon: {error}");
            crash::report_error(&home, "client", &*error);
        }
        Ok(Ok(())) => {}
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
