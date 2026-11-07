# Horizon on Android (arm64, experimental)

The game runs as a `NativeActivity` (no Java/Kotlin code): `crates/horizon-android` builds
horizon-client's game (`src/game.rs` + `src/game/`) as `libhorizon.so`, with the entry point
`crates/horizon-client/src/android_main.rs` instead of `main.rs`. Rendering is wgpu (Vulkan, GLES 3 fallback);
sound is cpal over AAudio. Status: compiles, links and packages; **not yet run on a device**.

## Build the APK (Linux host)

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
# Android SDK: cmdline-tools, then
sdkmanager "platforms;android-34" "build-tools;34.0.0" "ndk;27.2.12479018" "platform-tools"
export ANDROID_HOME=~/android/sdk            # NDK: the newest under $ANDROID_HOME/ndk, or ANDROID_NDK_HOME
./packaging/android/build_apk.sh             # -> target/android/horizon.apk (debug-signed)
PROFILE=release ./packaging/android/build_apk.sh   # slower build, faster game
CARGO_FLAGS="--features mods" ./packaging/android/build_apk.sh   # wasm code mods (untested on Android)
```

The script runs `cargo ndk` (API 26, arm64-v8a), strips the library, links the manifest
(`packaging/android/AndroidManifest.xml`) with `aapt2`, zipaligns and signs with
`~/.android/debug.keystore` (created if missing; password `android`). No Gradle.

## Install

```sh
adb install -r target/android/horizon.apk
```

Or copy the APK to the phone and open it (allow "install unknown apps"). Needs Android 8.0+ (API 26), arm64.

## Game files

Horizon never ships Minecraft's files. Prepare them **on a PC** (needs Java, see README.md):

```sh
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-prepare -- sounds     # optional
```

Then copy the *contents* of `.local/horizon-client/26.3/` (the folder holding `horizon-resources.json`) to the phone:

```
/sdcard/Android/data/dev.horizonlab.horizon/files/resources/
```

Start the app once first so Android creates `/sdcard/Android/data/dev.horizonlab.horizon/files/`.

```sh
adb shell mkdir -p /sdcard/Android/data/dev.horizonlab.horizon/files/resources
adb push .local/horizon-client/26.3/. /sdcard/Android/data/dev.horizonlab.horizon/files/resources/
```

That `files/` folder is the game home (`HORIZON_HOME` and the working directory): `options.txt`, `saves/`,
`logs/latest.log`, `crash-reports/`, `resourcepacks/` and so on live there, as in `~/.local/share/horizon` on
Linux. Existing worlds can be copied into `files/saves/`. If there is no external storage, the internal app
folder is used instead (reachable only with `adb shell run-as` on a debug build).

## Controls

- **Touch, in menus:** a finger is the mouse (tap = left click, drag = drag).
- **Touch, in the world:** left half = floating move stick (where you put your finger is its centre); right half =
  drag to look. Invisible button zones on the right edge (fractions of the screen, `src/game/touch.rs`):
  top-right corner = pause (Start), just left of it = inventory; right edge from top to bottom =
  next hotbar slot, use/place, attack/break, then at the bottom jump (outermost) and sneak.
  They go through the virtual controller, so they follow the controller bindings (`horizon-controller.txt`).
- **Keyboard** (USB/Bluetooth) arrives through winit like on desktop (WASD, Esc, F-keys, typing into
  text fields). The system Back key closes a screen or pauses; it never leaves the game.
- **Mouse:** clicks, wheel and the pointer go to the game as on desktop. While playing, the pointer is
  captured (`View.requestPointerCapture`) and turns the view; menus release it.
- **Gamepads:** gilrs has no Android backend, so `src/game/android_input.rs` reads the activity's own
  events (KEYCODE_BUTTON_*, AXIS_X/Y, Z/RZ or RX/RY, LTRIGGER/RTRIGGER, GAS/BRAKE, HAT_X/HAT_Y) through an
  input filter added to winit (`third_party/winit`) and hands them to the same pad code as the desktop:
  bindings from `horizon-controller.txt`, deadzones, the menu cursor. DirectInput pads without a key
  layout (BUTTON_1..12) are read in the usual order: 1 X, 2 A, 3 B, 4 Y, 5 LB, 6 RB, 7 LT, 8 RT,
  9 Select, 10 Start, 11 LS, 12 RS.
- The touch controls hide while a controller, keyboard or mouse plays and come back on the next touch.

## Mods

`.hnmod` files that carry an Android build (`binaries` → `aarch64-linux-android`) run on the phone: share or
open one with Horizon, use Profile → Install a mod, or copy it into `files/mods/`. Source-only `.hnmod` files
need the PC launcher's compiler; Options → Mods says so. `.hmod` content mods go into `files/horizon-mods/`.
How to build one: [docs/android-mods.md](docs/android-mods.md).

## What differs from desktop

| Area | Android |
|---|---|
| Controllers (`gilrs`, DualSense output) | off (`controllers` feature not enabled) |
| System clipboard (`arboard`) | stubbed: in-game clipboard only |
| Text input | no on-screen keyboard (chat, sign editing, world names need a physical keyboard) |
| Surface | dropped on `suspended`, recreated on `resumed` (`Renderer::lose_surface/regain_surface`) |
| Resources | `HORIZON_RESOURCES` (set by `android_main`) |
| Logs | `/storage/emulated/0/Android/data/dev.horizonlab.horizon/files/logs/latest.log`, crash reports in `crash-reports/` beside it (nothing in logcat). Any file manager with access to Android/data, or `adb pull` |
| Launcher, horizon-prepare, server binary, telemetry server | not built |

## Known risks

- Never run on hardware: the first start may fail on a GPU limit (wgpu downlevel limits, texture sizes, storage
  buffers on Mali/Adreno GLES) or on memory (`largeHeap` is set; the world uses a lot of RAM: try render distance 4–6).
- HDR (`horizon-hdr`) and Discord presence compile but do nothing useful on Android.
- Touch zones are not drawn on screen.

## Notes from the first device run (Galaxy S25, Adreno 830, Vulkan)

- Reaches the title screen, the menus and a single-player world; touch drives the menus. Uses about 2.2 GB RAM.
- Without a launcher install, a `~/.minecraft` built from Mojang's piston-meta works for horizon-prepare
  (versions/26.3/26.3.json + jar, libraries, assets). If prepare rejects `26.3.json` as unknown, save the URL it
  prints as `.local/minecraft/26.3/manifest.json` and run it again. Use a JDK 25 (`JAVA_HOME`).

## Soft keyboard, F-keys, layout editor (ce10266+)

- `HorizonActivity` (packaging/android/java) extends NativeActivity. Its hidden view
  takes IME text and passes it to Rust through JNI (`game/android_ime.rs`). The keyboard
  opens by itself whenever a text field has focus: chat, signs, books, searches, server
  IP, world name. Building it needs JDK 21 `javac` on PATH (the JDK 25 output breaks d8 34).
- Top-left row: F1 (hide HUD), F2 (screenshot), F3 (debug), F5 (camera), then the gear
  for the layout editor.
- Layout editor: drag a control to move it; tap it to select it; then use Size±, Alpha±,
  All± (global opacity), Hide/Show, Reset, Save. Save writes `touch_layout.json` in the
  game directory. Back leaves the editor without saving.
- Attack and use have their own buttons. A quick tap in the world attacks the entity
  under the crosshair, or uses if there isn't one. Holding still breaks blocks.
- The window now draws under the display cutout (shortEdges).

## First-run download (no PC)

When `files/resources` has no finished preparation, the app opens a setup screen
(`SetupActivity`, `crates/horizon-client/src/android_setup.rs`) instead of the game. One button
downloads the official game files, checking every SHA-1: the version JSON, the client JAR (must match
`research-lock.json`'s `client_sha1`), the asset index and its sounds, fonts, languages, panorama
and built-in packs (about 525 MB in total, 8 parallel connections). The JAR's assets, data, tag
and worldgen bundles are then unpacked in Rust (`horizon_prepare::device`), the same bytes
`horizon-prepare` writes. A foreground service keeps it running in the background, with a progress
notification. An interrupted run resumes: verified files are skipped, partial files continue.

What only the Java exporters can make (facts, collision, gameplay facts, entity models, the data
generator's reports and so on, about 126 MB, 4 MB zipped) is the "derived pack":
`packaging/android/make_derived_pack.sh` zips it from a PC-prepared folder. The phone takes it from
the URL on the setup screen (or the build's `HORIZON_DERIVED_URL`), or from
`files/horizon-derived.zip` if that file exists. It is derived from Mojang's game, so do not
publish it.

A folder already prepared on a PC and pushed to `files/resources` is used as before.

## Profile: name and skin

The person icon at the top left of the title and pause screens (and the
"Profile" button on the first-run screen) opens the profile screen.

- **Name**: 3–16 characters, `A-Z a-z 0-9 _`, saved as `name:<NAME>` in
  `files/horizon-player.txt`. It is the offline name used on servers and LAN
  and applies after a restart. Single-player worlds still call the local
  player `Player`.
- **Skin**: a 64x64 or 64x32 PNG picked with the system file picker, copied
  to `files/horizon-skin.png`. Arms: classic or slim, in
  `files/horizon-skin.txt`. The game checks these files every second, so a
  new skin shows on your own player (third person, tab list) right away.
  Offline servers have no texture service, so other players still see the
  default skin.

The first-run screen follows the system language (Russian or English). The
derived pack is taken from `HORIZON_DERIVED_URL` set at build time or from
`files/horizon-derived.zip`. If neither exists, the screen says so.
