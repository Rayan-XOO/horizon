//! Hands the Microsoft account signed in on the profile screen to the game.
//! The profile activity writes `msa-account.json` in internal storage. This
//! writes a fresh session file the game reads (`HORIZON_SESSION_FILE`), also in
//! internal storage so no other app can read the token. When the Java side
//! renews the sign-in in the background (`Msa.refresh`), it calls
//! `nativeAccountRefreshed` and the new session takes over. Tokens are not logged.
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

static INTERNAL: OnceLock<PathBuf> = OnceLock::new();

/// The session JSON for the saved account, if it is still good for a minute.
fn session(internal: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(internal.join("msa-account.json")).ok()?;
    let v = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let name = v.get("name").and_then(|s| s.as_str()).unwrap_or("");
    let uuid = v.get("uuid").and_then(|s| s.as_str()).unwrap_or("");
    let token = v.get("minecraft_token").and_then(|s| s.as_str()).unwrap_or("");
    if name.is_empty() || uuid.len() != 32 || token.is_empty() {
        return None;
    }
    let expires = v.get("minecraft_expires_at").and_then(|n| n.as_u64()).unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if expires != 0 && expires < now + 60 {
        log::info!("Horizon: the Microsoft sign-in is being renewed");
        return None;
    }
    Some(serde_json::json!({
        "name": name,
        "uuid": uuid,
        "access_token": token,
        "user_type": "msa",
        "xuid": v.get("xuid").and_then(|s| s.as_str()).unwrap_or(""),
    }))
}

fn session_path(internal: &Path) -> PathBuf {
    internal.join("sessions").join("session-android.json")
}

pub fn prepare(internal: &Path, home: &Path) {
    let _ = INTERNAL.set(internal.to_owned());
    // Older builds kept the session on shared storage.
    let _ = std::fs::remove_file(home.join("sessions").join("session-android.json"));
    let Some(body) = session(internal) else {
        return;
    };
    let file = session_path(internal);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if write_private(&file, body.to_string().as_bytes()).is_err() {
        log::warn!("Horizon: could not write the Microsoft session");
        return;
    }
    log::info!("Horizon: signed in with Microsoft (online-mode servers)");
    // SAFETY: called before the game's other threads start.
    unsafe { std::env::set_var("HORIZON_SESSION_FILE", &file) };
}

/// The Java side renewed the account: the game uses the new token from now on.
#[unsafe(no_mangle)]
extern "system" fn Java_dev_horizonlab_horizon_HorizonActivity_nativeAccountRefreshed(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    let Some(internal) = INTERNAL.get() else {
        return;
    };
    let Some(body) = session(internal) else {
        return;
    };
    let file = session_path(internal);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = write_private(&file, body.to_string().as_bytes());
    match horizon_client::auth::AuthSession::parse(&body.to_string()) {
        Ok(session) => {
            horizon_client::auth::set_current(session);
            log::info!("Horizon: Microsoft sign-in renewed");
        }
        Err(e) => log::warn!("Horizon: renewed session not read: {e}"),
    }
}

fn write_private(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    Ok(())
}
