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
    // A panic on the render thread used to leave the activity up, so the next
    // tap died with "EventLoop can't be recreated".
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        std::process::exit(1);
    }));
    if let Err(error) = game::run() {
        log::error!("Horizon: {error}");
        crash::report_error(&home, "client", &*error);
    }
    logging::flush();
    // Quit Game only leaves the native loop. Finish the activity or the next
    // launch resumes a dead window (black, then it minimizes).
    game::finish_activity();
}

/// `vulkan-driver.txt` in internal storage (Profile → Turnip) is the absolute
/// path of a custom `libvulkan*.so`. Sibling `.so` files are opened first so
/// the linker can resolve them. Nothing is set when the file is missing.
fn apply_vulkan_driver(internal: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(internal.join("vulkan-driver.txt")) else {
        return;
    };
    let path = text.trim();
    if path.is_empty() {
        return;
    }
    let so = std::path::Path::new(path);
    if !so.is_file() {
        log::warn!("Horizon: Vulkan driver missing: {path}");
        return;
    }
    if let Some(dir) = so.parent()
        && let Ok(read) = std::fs::read_dir(dir)
    {
        for entry in read.flatten() {
            let lib = entry.path();
            if lib == so || lib.extension().is_none_or(|e| e != "so") {
                continue;
            }
            preload(&lib);
        }
    }
    preload(so);
    log::info!("Horizon: custom Vulkan driver {path}");
    unsafe { std::env::set_var("HORIZON_VULKAN_LIB", path) };
}

fn preload(path: &std::path::Path) {
    let Ok(c) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) else {
        return;
    };
    // RTLD_NOW | RTLD_GLOBAL. A null handle means the .so did not load; the
    // later Vulkan init logs the real failure.
    let handle = unsafe { dlopen(c.as_ptr(), 2 | 0x00100) };
    if handle.is_null() {
        log::warn!("Horizon: could not preload {}", path.display());
    }
}

unsafe extern "C" {
    fn dlopen(filename: *const u8, flags: i32) -> *mut std::ffi::c_void;
}
