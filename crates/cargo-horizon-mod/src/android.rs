//! `cargo horizon-mod android`: a prebuilt native mod for Android
//! (docs/android-mods.md). Builds the crate in the current folder (a
//! `cdylib` on `horizon-native-abi`) for `aarch64-linux-android` with
//! `cargo ndk`, then packs its library into a `.hnmod`.
//!
//! - `android`: the bundle's manifest is `horizon-native.json` in the folder;
//!   writes `target/horizon-mod/<id>.hnmod` holding the library.
//! - `android --bundle <mod.hnmod>`: adds the library to that source bundle
//!   instead (its mixins stay for the PC launcher), written to the same place.
//! - `--lib <file.so>`: pack a library built elsewhere; no build.
use horizon_mods::index::native::{Bundle, MANIFEST, Manifest};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

const TARGET: &str = "aarch64-linux-android";

pub fn main(args: &[String]) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let lib = match super::flag(args, "--lib") {
        Some(lib) => PathBuf::from(lib),
        None => build(&root)?,
    };
    let bytes = std::fs::read(&lib).map_err(|e| format!("{}: {e}", lib.display()))?;
    if !bytes.starts_with(b"\x7fELF") {
        return Err(format!("{}: not an ELF library", lib.display()));
    }
    let (mut manifest, mut files) = match super::flag(args, "--bundle") {
        Some(path) => {
            let b = Bundle::read(Path::new(path))?;
            (b.manifest, b.files)
        }
        None => {
            let path = root.join(MANIFEST);
            let text = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let m: Manifest =
                serde_json::from_slice(&text).map_err(|e| format!("{MANIFEST}: {e}"))?;
            (m, Default::default())
        }
    };
    let inner = format!("bin/{TARGET}/lib{}.so", manifest.id);
    if let Some(old) = manifest.binaries.insert(TARGET.into(), inner.clone()) {
        files.remove(&old);
    }
    manifest.check()?;
    files.insert(inner, bytes);
    files.insert(
        MANIFEST.into(),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    );
    let dir = root.join("target/horizon-mod");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{}.hnmod", manifest.id));
    write(&out, &files)?;
    Bundle::read(&out)?;
    println!("Built {}", out.display());
    Ok(())
}

fn write(out: &Path, files: &std::collections::BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in files {
        zip.start_file(name.as_str(), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// `cargo ndk -t arm64-v8a -P 26 build --release`, and the library it made.
fn build(root: &Path) -> Result<PathBuf, String> {
    let status = Command::new("cargo")
        .args(["ndk", "-t", "arm64-v8a", "-P", "26", "build", "--release"])
        .current_dir(root)
        .status()
        .map_err(|e| {
            format!("cargo ndk: {e} (cargo install cargo-ndk, and set ANDROID_NDK_HOME)")
        })?;
    if !status.success() {
        return Err("the Android build failed".into());
    }
    let meta = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let meta: serde_json::Value =
        serde_json::from_slice(&meta.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let target_dir = meta["target_directory"]
        .as_str()
        .ok_or("cargo metadata: no target directory")?;
    let lib = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| p["targets"].as_array().into_iter().flatten())
        .find(|t| {
            t["crate_types"]
                .as_array()
                .is_some_and(|c| c.iter().any(|c| c == "cdylib"))
        })
        .and_then(|t| t["name"].as_str())
        .ok_or("no cdylib in this crate: set [lib] crate-type = [\"cdylib\"]")?
        .replace('-', "_");
    let path = Path::new(target_dir).join(format!("{TARGET}/release/lib{lib}.so"));
    path.is_file()
        .then_some(path.clone())
        .ok_or_else(|| format!("{} was not built", path.display()))
}
