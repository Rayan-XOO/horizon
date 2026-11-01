# Mods on Android

A `.hnmod` is a **source** bundle: `horizon-native.json` plus a horizon-mixins bundle
(`mixins/<id>/horizon-mixin.toml` and `.rs` files) that patches the game's own Rust source. The PC launcher
applies it and compiles a game of its own with it ([native-mods.md](native-mods.md)). A phone has no Rust
toolchain and the game's internals have no stable binary interface, so a source-only `.hnmod` can't run on
Android. Instead a `.hnmod` may also carry **prebuilt libraries**, one per target, that speak a small C ABI
(`crates/horizon-native-abi`). Android opens those.

## The format

`horizon-native.json` gains `binaries`, target → path in the bundle:

```json
{
  "schema": 1, "id": "hello_android", "version": "0.1.0", "game": "26.3",
  "horizon": "<40-hex commit>", "mixins": "<40-hex commit>",
  "binaries": { "aarch64-linux-android": "bin/aarch64-linux-android/libhello_android.so" }
}
```

A path is always `bin/<target>/<file>`. A bundle may hold both its mixins (for the PC launcher) and its
libraries (for Android); a bundle with libraries only needs no `horizon-mixin.toml`. The PC game ignores
`binaries`.

## The ABI (`horizon-native-abi`, version 1)

The library exports `horizon_mod_entry(&mut Host) -> *const ModTable`. Every type is `#[repr(C)]`.

- `ModTable`: `tick` (each client tick), `hud` (each frame the HUD draws, not under F1), `key` (a key or mouse
  button by vanilla name, `key.keyboard.h`), `command` (a chat command without its slash; true when the mod ran
  it). Each is optional.
- `Host`: `log`, `chat` (a line in chat), `hud_line` (a line of the mod's box in the top left corner, inside
  `hud` only), `data_dir` (the mod's own folder, `horizon-mods/.horizon-mod-data/<id>/`).
- `State`: in a world, playing, the player's position, yaw and pitch, the tick count.

In Rust, implement `horizon_native_abi::NativeMod` and call `export_mod!(MyMod)`; panics stay in the mod. The
ABI only grows at the end of its structs; the game opens a mod made for its ABI version or an older one.

This is far less than the hook points of [native-mods.md](native-mods.md): no world, item, block or server
hooks. Mods that need them stay PC-only until the ABI grows.

## Build a mod for Android

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk                       # and an NDK: ANDROID_NDK_HOME
cargo install --path crates/cargo-horizon-mod
cd mods/hello-android                         # a cdylib on horizon-native-abi
cargo horizon-mod android                     # -> target/horizon-mod/hello_android.hnmod
cargo horizon-mod android --bundle my-mod.hnmod   # add the library to an existing source bundle
cargo horizon-mod android --lib libmine.so    # pack a library built elsewhere
```

`android` runs `cargo ndk -t arm64-v8a -P 26 build --release`, adds the library as
`bin/aarch64-linux-android/lib<id>.so`, sets `binaries` and checks the result.

## Install on the phone

- Open or share a `.hnmod` (or `.hmod`) with Horizon ("Install mod"), or tap **Profile → Install a mod**; or
- copy it into `Android/data/dev.horizonlab.horizon/files/mods/` (`horizon-native-mods/` works too).

Restart the game. **Options → Mods** lists every bundle: one with an Android library is on until turned
off there (turning it off stops it at once; turning it back on opens it without a restart, once it was
found at start). A bundle with no Android library, or one that fails to open, says why. The library is
copied into the app's own storage before it is opened, since external storage can't run code.

Prebuilt mods run native code with the game's rights, unsandboxed: install only mods you trust.
