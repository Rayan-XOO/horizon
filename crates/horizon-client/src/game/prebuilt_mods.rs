//! Prebuilt native mods (docs/android-mods.md): where the game can't build a
//! `.hnmod` from its sources (Android), it opens the library the bundle
//! carries for this target (`binaries` in `horizon-native.json`) and calls it
//! through `horizon-native-abi` at its tick, HUD, keys and commands. A
//! bundle with no library for this target gets a row saying so.
//!
//! Bundles are read from `mods/` and `horizon-native-mods/` beside
//! `options.txt`. External storage can't hold running code, so each library
//! is copied into the app's own storage (`HORIZON_CACHE`'s folder) first.
use super::Game;
use horizon_client::{
    extras::Corner,
    extras_hud::Corners,
    hud::HudBatch,
    mod_list::{self, Installed, NativePage},
};
use horizon_mods::index::native::Bundle;
use horizon_native_abi::{
    State,
    host::{Loaded, Outbox},
};
use std::path::{Path, PathBuf};

/// The folder players drop `.hnmod` files into, beside `options.txt`.
pub(super) const DIR: &str = "mods";

/// Whether this game opens prebuilt mods: only where it can't build them.
pub(super) const ON: bool = cfg!(target_os = "android");

/// One prebuilt mod found at start.
pub(super) struct Prebuilt {
    id: String,
    /// The library in the app's own storage, opened when the mod is first on.
    lib: PathBuf,
    loaded: Option<Loaded>,
    /// Opening it failed: not tried again.
    failed: bool,
}

#[derive(Default)]
pub(super) struct PrebuiltMods {
    mods: Vec<Prebuilt>,
}

/// The message on a bundle's row when it carries no library for this game.
pub(super) fn no_build(target: &str) -> String {
    format!(
        "No {target} build in this mod: it carries only sources, which the PC launcher builds. Ask its author for an Android build (cargo horizon-mod android)."
    )
}

/// The `.hnmod` files in `dirs`, first folder first, by name; a later file
/// of the same name is left out.
fn bundles(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        found.retain(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("hnmod"))
        });
        found.sort();
        for p in found {
            if !out.iter().any(|o| o.file_name() == p.file_name()) {
                out.push(p);
            }
        }
    }
    out
}

/// Where libraries are copied to run: the app's own storage.
fn lib_dir() -> PathBuf {
    let base = std::env::var_os("HORIZON_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.with_file_name("horizon-mod-libs")
}

/// Reads the bundles in `dirs` for `target`: a row for each, and the mods
/// with a library, copied into `libs`.
pub(super) fn scan(
    dirs: &[PathBuf],
    target: Option<&str>,
    libs: &Path,
) -> (Vec<Installed>, Vec<Prebuilt>) {
    let mut rows: Vec<Installed> = Vec::new();
    let mut mods = Vec::new();
    for path in bundles(dirs) {
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let bundle = match Bundle::read(&path) {
            Ok(b) => b,
            Err(e) => {
                rows.push(row(
                    String::new(),
                    file,
                    Some(format!("Can't be read: {e}")),
                ));
                continue;
            }
        };
        let m = &bundle.manifest;
        if rows.iter().any(|r| r.id == m.id) {
            continue;
        }
        let title = format!("{} {}", m.id, m.version);
        let Some((target, inner)) = target.and_then(|t| m.binaries.get(t).map(|p| (t, p))) else {
            let problem = no_build(target.unwrap_or("Android"));
            rows.push(row(m.id.clone(), title, Some(problem)));
            continue;
        };
        let bytes = &bundle.files[inner];
        let name = format!("{}-{}-{:016x}.so", m.id, target, fnv(bytes));
        let lib = libs.join(name);
        let copied = lib.is_file()
            || std::fs::create_dir_all(libs)
                .and_then(|()| std::fs::write(&lib, bytes))
                .is_ok();
        if !copied {
            let problem = format!("Can't copy its library to {}", libs.display());
            rows.push(row(m.id.clone(), title, Some(problem)));
            continue;
        }
        let mut r = row(m.id.clone(), title, None);
        r.native = Some(NativePage::default());
        rows.push(r);
        mods.push(Prebuilt {
            id: m.id.clone(),
            lib,
            loaded: None,
            failed: false,
        });
    }
    (rows, mods)
}

fn row(id: String, title: String, problem: Option<String>) -> Installed {
    Installed {
        id,
        title,
        description: String::new(),
        problem,
        dev: false,
        native: None,
    }
}

/// FNV-1a: tells one build of a library from another in its copy's name.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Game<'_> {
    /// Options > Mods' rows for the prebuilt mods, and the mods to open.
    pub(super) fn start_prebuilt_mods(&mut self) {
        if !self.native_window() {
            return;
        }
        let options = Self::options_path();
        let dirs = [
            options.with_file_name(DIR),
            options.with_file_name(mod_list::NATIVE_DIR),
        ];
        let _ = std::fs::create_dir_all(&dirs[0]);
        let (rows, mods) = scan(&dirs, horizon_native_abi::host::target(), &lib_dir());
        let installed = &mut self.settings.mods.installed;
        installed.retain(|r| !rows.iter().any(|n| n.id == r.id && !n.id.is_empty()));
        installed.extend(rows);
        // A library already open stays open: it can't be opened twice.
        let mut old = std::mem::take(&mut self.prebuilt.mods);
        self.prebuilt.mods = mods
            .into_iter()
            .map(|m| match old.iter().position(|o| o.lib == m.lib) {
                Some(i) => old.swap_remove(i),
                None => m,
            })
            .collect();
        self.open_prebuilt_mods();
    }
    /// Opens the mods that are on and not open yet.
    fn open_prebuilt_mods(&mut self) {
        let home = Self::options_path().with_file_name(mod_list::DIR);
        for i in 0..self.prebuilt.mods.len() {
            let m = &self.prebuilt.mods[i];
            if m.loaded.is_some() || m.failed || !self.native_on(&m.id) {
                continue;
            }
            let data = home.join(".horizon-mod-data").join(&m.id);
            // SAFETY: the player installed the mod and left it on; it runs with the game's rights.
            let opened = unsafe { Loaded::open(&m.lib, &data) };
            let id = m.id.clone();
            match opened {
                Ok((loaded, out)) => {
                    log::info!("Horizon: mod {id} opened ({})", m.lib.display());
                    self.prebuilt.mods[i].loaded = Some(loaded);
                    self.prebuilt_outbox(&id, out);
                }
                Err(e) => {
                    log::warn!("Horizon: mod {id} can't open: {e}");
                    self.prebuilt.mods[i].failed = true;
                    let row = self.settings.mods.installed.iter_mut().find(|r| r.id == id);
                    if let Some(row) = row {
                        row.problem = Some(format!("Can't open: {e}"));
                        row.native = None;
                    }
                }
            }
        }
    }
    /// What a mod asked for in a call.
    fn prebuilt_outbox(&mut self, id: &str, out: Outbox) {
        for line in out.log {
            log::info!("[{id}] {line}");
        }
        for line in out.chat {
            self.native_chat(&line);
        }
    }
    fn prebuilt_state(&self) -> State {
        let player = self.native_player();
        State {
            in_world: player.is_some(),
            playing: self.native_playing(),
            x: player.as_ref().map_or(0.0, |p| p.x),
            y: player.as_ref().map_or(0.0, |p| p.y),
            z: player.as_ref().map_or(0.0, |p| p.z),
            yaw: self.yaw,
            pitch: self.pitch,
            ticks: u64::from(self.ticks),
        }
    }
    /// Calls `f` on each mod that is open and on, then does what it asked.
    fn each_prebuilt(&mut self, mut f: impl FnMut(&Loaded) -> Outbox) {
        if self.prebuilt.mods.is_empty() || !self.native_window() {
            return;
        }
        self.open_prebuilt_mods();
        let mut asked = Vec::new();
        for m in &self.prebuilt.mods {
            if let Some(l) = &m.loaded
                && !self.settings.mods.disabled.contains(&m.id)
            {
                asked.push((m.id.clone(), f(l)));
            }
        }
        for (id, out) in asked {
            self.prebuilt_outbox(&id, out);
        }
    }
    pub(super) fn prebuilt_tick(&mut self) {
        let state = self.prebuilt_state();
        self.each_prebuilt(|m| m.tick(&state));
    }
    pub(super) fn prebuilt_key(&mut self, key: &str, pressed: bool) {
        self.each_prebuilt(|m| m.key(key, pressed));
    }
    /// A chat command: whether a mod ran it.
    pub(super) fn prebuilt_command(&mut self, command: &str) -> bool {
        let mut ran = false;
        self.each_prebuilt(|m| {
            if ran {
                return Outbox::default();
            }
            let (r, out) = m.command(command);
            ran = r;
            out
        });
        ran
    }
    /// The mods' HUD lines, a box per mod in the top left corner.
    pub(super) fn prebuilt_hud(&mut self, batch: &mut HudBatch, corners: &mut Corners) {
        if self.prebuilt.mods.is_empty() {
            return;
        }
        let state = self.prebuilt_state();
        let mut boxes = Vec::new();
        self.each_prebuilt(|m| {
            let mut out = m.hud(&state);
            boxes.push(std::mem::take(&mut out.hud));
            out
        });
        if !self.native_corner_shown(Corner::TopLeft) {
            return;
        }
        for lines in boxes.into_iter().filter(|l| !l.is_empty()) {
            let w = lines
                .iter()
                .map(|(t, _)| self.native_text_width(t))
                .fold(0.0, f32::max)
                + 4.0;
            let h = lines.len() as f32 * 10.0 + 2.0;
            let (x, y) = corners.place(Corner::TopLeft, w, h);
            self.native_fill(batch, [x, y, w, h], 0x7800_0000);
            for (i, (text, rgb)) in lines.iter().enumerate() {
                let at = [x + 2.0, y + 2.0 + i as f32 * 10.0];
                self.native_text(batch, text, at, *rgb, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn bundle(dir: &Path, id: &str, lib: Option<&[u8]>) {
        let mut manifest = serde_json::json!({
            "schema": 1, "id": id, "version": "0.1.0", "game": "26.3",
            "horizon": "a".repeat(40), "mixins": "b".repeat(40),
        });
        let inner = format!("bin/aarch64-linux-android/lib{id}.so");
        if lib.is_some() {
            manifest["binaries"] = serde_json::json!({ "aarch64-linux-android": inner });
        }
        let mixin = format!(
            "[mod]\nid = \"{id}\"\nversion = \"0.1.0\"\n[compatibility]\nhorizon_revision = \"{}\"\n",
            "a".repeat(40)
        );
        let file = std::fs::File::create(dir.join(format!("{id}.hnmod"))).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("horizon-native.json", options).unwrap();
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file(format!("mixins/{id}/horizon-mixin.toml"), options)
            .unwrap();
        zip.write_all(mixin.as_bytes()).unwrap();
        if let Some(lib) = lib {
            zip.start_file(inner, options).unwrap();
            zip.write_all(lib).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn bundles_with_a_library_for_the_target_are_copied_others_say_why() {
        let root = std::env::temp_dir().join(format!("horizon-prebuilt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (mods, native, libs) = (root.join("mods"), root.join("native"), root.join("libs"));
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::create_dir_all(&native).unwrap();
        bundle(&mods, "hello", Some(b"\x7fELF library"));
        bundle(&mods, "zoom", None);
        bundle(&native, "hello", None);
        std::fs::write(mods.join("broken.hnmod"), "not a zip").unwrap();
        let (rows, found) = scan(&[mods, native], Some("aarch64-linux-android"), &libs);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["", "hello", "zoom"]);
        assert!(
            rows[0]
                .problem
                .as_deref()
                .unwrap()
                .starts_with("Can't be read")
        );
        assert!(rows[1].problem.is_none() && rows[1].native.is_some());
        assert_eq!(
            rows[2].problem.as_deref(),
            Some(no_build("aarch64-linux-android").as_str())
        );
        assert_eq!(found.len(), 1);
        assert_eq!(std::fs::read(&found[0].lib).unwrap(), b"\x7fELF library");
        // Another game: nothing to open.
        let (_, found) = scan(
            &[root.join("mods")],
            Some("x86_64-unknown-linux-gnu"),
            &libs,
        );
        assert!(found.is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
