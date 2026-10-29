//! Horizon on Android: the crate root of `horizon-android` (a NativeActivity
//! cdylib, ANDROID.md). It sits beside `main.rs` so `mod game;` compiles the
//! same game; this file only replaces `main` with `android_main`.
#![cfg(target_os = "android")]
#![allow(dead_code)]
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
    let finish_app = app.clone();
    {
        let mut slot = game::ANDROID_APP.lock().expect("the Android app lock");
        *slot = Some(app);
    }
    if let Err(error) = game::run() {
        log::error!("Horizon: {error}");
        crash::report_error(&home, "client", &*error);
    }
    logging::flush();
    finish_activity(&finish_app);
}

/// Finish the Android activity once the game has stopped: without it the
/// NativeActivity survives `android_main`'s return and the next launch
/// reuses a dead activity (black screen).
fn finish_activity(app: &AndroidApp) {
    let result = (|| -> jni::errors::Result<()> {
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast())? };
        let mut env = vm.attach_current_thread_permanently()?;
        let activity = unsafe { jni::objects::JObject::from_raw(app.activity_as_ptr().cast()) };
        env.call_method(&activity, "finishGame", "()V", &[])?;
        Ok(())
    })();
    if let Err(error) = result {
        log::warn!("Horizon: could not finish the Android activity: {error}");
    }
}
