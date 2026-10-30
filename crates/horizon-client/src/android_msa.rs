//! Hands the Microsoft account signed in on the profile screen to the game.
//! The profile activity writes `msa-account.json` in internal storage. This
//! writes a fresh session file the game reads once (`HORIZON_SESSION_FILE`).
//! Tokens are not logged.
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn prepare(internal: &Path, home: &Path) {
    let path = internal.join("msa-account.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let name = v.get("name").and_then(|s| s.as_str()).unwrap_or("");
    let uuid = v.get("uuid").and_then(|s| s.as_str()).unwrap_or("");
    let token = v.get("minecraft_token").and_then(|s| s.as_str()).unwrap_or("");
    if name.is_empty() || uuid.len() != 32 || token.is_empty() {
        return;
    }
    let expires = v.get("minecraft_expires_at").and_then(|n| n.as_u64()).unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if expires != 0 && expires < now + 60 {
        log::warn!("Horizon: Microsoft sign-in expired. Sign in again in Profile.");
        return;
    }
    let dir = home.join("sessions");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("session-android.json");
    let body = serde_json::json!({
        "name": name,
        "uuid": uuid,
        "access_token": token,
        "user_type": "msa",
        "xuid": v.get("xuid").and_then(|s| s.as_str()).unwrap_or(""),
    });
    if write_private(&file, body.to_string().as_bytes()).is_err() {
        log::warn!("Horizon: could not write the Microsoft session");
        return;
    }
    log::info!("Horizon: signed in as {name} (online-mode servers)");
    unsafe { std::env::set_var("HORIZON_SESSION_FILE", &file) };
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
