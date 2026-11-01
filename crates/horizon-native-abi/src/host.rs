//! The game's side: opening a mod's library and calling it.
use super::{ABI, ENTRY, Host, ModTable, State, Str};
use std::{ffi::c_void, path::Path};

/// The target name this game's mods are built for, as in `binaries`.
pub fn target() -> Option<&'static str> {
    if cfg!(all(target_os = "android", target_arch = "aarch64")) {
        Some(super::ANDROID_ARM64)
    } else if cfg!(all(target_os = "android", target_arch = "x86_64")) {
        Some("x86_64-linux-android")
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu"
    )) {
        Some("x86_64-unknown-linux-gnu")
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "aarch64",
        target_env = "gnu"
    )) {
        Some("aarch64-unknown-linux-gnu")
    } else {
        None
    }
}

/// What a mod asked for in one call.
#[derive(Debug, Default)]
pub struct Outbox {
    pub log: Vec<String>,
    pub chat: Vec<String>,
    pub hud: Vec<(String, u32)>,
}

extern "C" fn put_log(ctx: *mut c_void, text: Str) {
    // SAFETY: `ctx` is the call's Outbox, `text` the mod's live string.
    unsafe { (*ctx.cast::<Outbox>()).log.push(text.as_str().to_owned()) };
}
extern "C" fn put_chat(ctx: *mut c_void, text: Str) {
    // SAFETY: as `put_log`.
    unsafe { (*ctx.cast::<Outbox>()).chat.push(text.as_str().to_owned()) };
}
extern "C" fn put_hud(ctx: *mut c_void, text: Str, rgb: u32) {
    // SAFETY: as `put_log`.
    let out = unsafe { &mut *ctx.cast::<Outbox>() };
    if out.hud.len() < 32 {
        // SAFETY: as `put_log`.
        out.hud.push((unsafe { text.as_str() }.to_owned(), rgb));
    }
}

/// An open mod. Its library stays loaded until the game quits.
pub struct Loaded {
    table: &'static ModTable,
    data_dir: String,
}

impl Loaded {
    /// Opens the library at `path` (on Android, in the app's own storage:
    /// external storage can't run code) and calls its entry.
    ///
    /// # Safety
    /// Runs the library's code with the game's rights.
    pub unsafe fn open(path: &Path, data_dir: &Path) -> Result<(Loaded, Outbox), String> {
        // SAFETY: the caller's promise.
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
        type Entry = extern "C" fn(&mut Host) -> *const ModTable;
        // SAFETY: the symbol has this type by the ABI.
        let entry: Entry = unsafe { lib.get::<Entry>(ENTRY.as_bytes()) }
            .map(|s| *s)
            .map_err(|_| format!("no {ENTRY} in the library: not a Horizon mod"))?;
        // The library outlives every call into it.
        std::mem::forget(lib);
        let _ = std::fs::create_dir_all(data_dir);
        let data_dir = data_dir.to_string_lossy().into_owned();
        let mut out = Outbox::default();
        let mut host = Self::host(&data_dir, &mut out);
        let table = entry(&mut host);
        if table.is_null() {
            return Err("the mod failed to start".into());
        }
        // SAFETY: the mod's table lives as long as its library, which stays.
        let table: &'static ModTable = unsafe { &*table };
        if table.abi == 0 || table.abi > ABI {
            return Err(format!(
                "made for mod ABI {}, this game has {ABI}: update the game",
                table.abi
            ));
        }
        Ok((Loaded { table, data_dir }, out))
    }
    fn host(data_dir: &str, out: &mut Outbox) -> Host {
        Host {
            abi: ABI,
            ctx: (out as *mut Outbox).cast(),
            log: put_log,
            chat: put_chat,
            hud_line: put_hud,
            data_dir: Str::new(data_dir),
        }
    }
    fn call(&self, f: impl FnOnce(&mut Host)) -> Outbox {
        let mut out = Outbox::default();
        let mut host = Self::host(&self.data_dir, &mut out);
        f(&mut host);
        out
    }
    pub fn tick(&self, game: &State) -> Outbox {
        let t = self.table;
        self.call(|h| {
            if let Some(f) = t.tick {
                f(t.state, h, game);
            }
        })
    }
    pub fn hud(&self, game: &State) -> Outbox {
        let t = self.table;
        self.call(|h| {
            if let Some(f) = t.hud {
                f(t.state, h, game);
            }
        })
    }
    pub fn key(&self, key: &str, pressed: bool) -> Outbox {
        let t = self.table;
        self.call(|h| {
            if let Some(f) = t.key {
                f(t.state, h, Str::new(key), pressed);
            }
        })
    }
    pub fn command(&self, command: &str) -> (bool, Outbox) {
        let t = self.table;
        let mut ran = false;
        let out = self.call(|h| {
            if let Some(f) = t.command {
                ran = f(t.state, h, Str::new(command));
            }
        });
        (ran, out)
    }
}
