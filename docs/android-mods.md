# Моды для Android / Mods on Android

[Русский](#русский) · [English](#english)

## Русский

### Что можно и что нельзя

Обычный `.hnmod` с ПК — это **исходники**: патчи horizon-mixins, которые лаунчер вшивает в исходный код игры и
компилирует. На телефоне компилятора нет, поэтому **такой мод на Android не запустится**. На Android работают
только моды с **готовой библиотекой** (`.so`) под `aarch64-linux-android`, написанные на небольшом C ABI
`horizon-native-abi`. Один `.hnmod` может содержать и исходники для ПК, и библиотеку для Android.

### Шаг 1. Инструменты (один раз)

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
# Android NDK: через Android Studio (SDK Manager → NDK) или с developer.android.com/ndk
export ANDROID_NDK_HOME=/путь/к/android-ndk
git clone https://github.com/Rayan-XOO/horizon && cd horizon && git checkout android-port
cargo install --path crates/cargo-horizon-mod
```

### Шаг 2. Создайте крейт

```sh
cargo new --lib my-mod && cd my-mod
```

`Cargo.toml`:

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
horizon-native-abi = { git = "https://github.com/Rayan-XOO/horizon", branch = "android-port" }
# или path = "../horizon/crates/horizon-native-abi"

[profile.release]
strip = true
panic = "unwind"
```

### Шаг 3. `horizon-native.json`

Рядом с `Cargo.toml`:

```json
{
  "schema": 1,
  "id": "my_mod",
  "version": "0.1.0",
  "game": "26.3",
  "horizon": "<40-символьный хеш коммита Horizon>",
  "mixins": "0000000000000000000000000000000000000000"
}
```

`id` — латиница, цифры и `_`. Поле `binaries` заполнит сборщик сам.

### Шаг 4. Код

`src/lib.rs` (это `mods/hello-android`):

```rust
use horizon_native_abi::{Host, NativeMod, State, export_mod};

struct Hello { presses: u32 }

impl NativeMod for Hello {
    fn new(host: &mut Host) -> Self {
        host.log("hello from a prebuilt mod");
        Hello { presses: 0 }
    }
    fn hud(&mut self, host: &mut Host, game: &State) {
        if game.in_world {
            host.hud_line("Hello Android", 0xFFFF55);
            host.hud_line(&format!("{:.1} {:.1} {:.1}", game.x, game.y, game.z), 0xFFFFFF);
        }
    }
    fn key(&mut self, _host: &mut Host, key: &str, pressed: bool) {
        if pressed && key == "key.keyboard.h" { self.presses += 1; }
    }
    fn command(&mut self, host: &mut Host, command: &str) -> bool {
        if command != "hello" { return false; }
        host.chat(&format!("Hello from Android! H pressed {} times.", self.presses));
        true
    }
}

export_mod!(Hello);
```

Все методы, кроме `new`, необязательны. Паника внутри мода не роняет игру.

### Шаг 5. Сборка

```sh
cargo horizon-mod android                          # -> target/horizon-mod/my_mod.hnmod
cargo horizon-mod android --bundle my-mod.hnmod    # добавить .so в уже готовый ПК-бандл
cargo horizon-mod android --lib libmine.so         # упаковать .so, собранную иначе
```

Внутри выполняется `cargo ndk -t arm64-v8a -P 26 build --release`, библиотека кладётся в
`bin/aarch64-linux-android/lib<id>.so`, заполняется `binaries`, бандл проверяется.

### Шаг 6. Установка на телефон

- Откройте или «Поделитесь» файлом `.hnmod` с Horizon, или **Профиль → Установить мод**; либо
- скопируйте файл в `Android/data/dev.horizonlab.horizon/files/mods/`.

Перезапустите игру. **Настройки → Моды** — список модов, там же включение/выключение. Если у мода нет
Android-библиотеки, там будет написано почему он не работает. Проверка: зайдите в мир, вверху слева появится
«Hello Android», в чате `/hello`.

### Доступный API (ABI версии 1)

- Колбэки мода: `tick` (каждый тик клиента), `hud` (каждый кадр HUD, не при F1), `key` (клавиша/кнопка мыши по
  ванильному имени, напр. `key.keyboard.h`), `command` (команда чата без `/`; вернуть `true`, если обработана).
- `Host`: `log`, `chat` (строка в чат), `hud_line(text, rgb)` (строка в блоке мода слева сверху, только в
  `hud`), `data_dir()` (своя папка мода для файлов).
- `State`: `in_world`, `playing`, `x`, `y`, `z`, `yaw`, `pitch`, `ticks`.

### Ограничения

- ПК-моды из одних исходников (`.hnmod` с mixins) на Android **не работают** — нужна собранная `.so`.
- Нет хуков мира, блоков, предметов, сервера — только HUD, клавиши, чат и позиция игрока.
- Только arm64 (`aarch64-linux-android`).
- Моды — нативный код без песочницы с правами игры. Ставьте только то, чему доверяете.

## English

### What works and what doesn't

A desktop `.hnmod` is **source**: horizon-mixins patches the launcher compiles into the game. A phone has no
compiler, so **such a mod can't run on Android**. Android runs only mods that carry a **prebuilt library**
(`.so`) for `aarch64-linux-android`, written against the small C ABI `horizon-native-abi`. One `.hnmod` can
hold both the desktop sources and the Android library.

### Step 1. Toolchain (once)

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
# Android NDK: Android Studio (SDK Manager → NDK) or developer.android.com/ndk
export ANDROID_NDK_HOME=/path/to/android-ndk
git clone https://github.com/Rayan-XOO/horizon && cd horizon && git checkout android-port
cargo install --path crates/cargo-horizon-mod
```

### Step 2. Create the crate

`cargo new --lib my-mod`, then use the same `Cargo.toml` as above: `crate-type = ["cdylib"]` and a
dependency on `horizon-native-abi` (git or path).

### Step 3. `horizon-native.json`

Next to `Cargo.toml`, as in the Russian section: `schema`, `id` (letters, digits, `_`), `version`, `game`,
`horizon`, `mixins`. `binaries` is filled in by the build.

### Step 4. Code

Implement `NativeMod` and call `export_mod!` — see the example above (`mods/hello-android`). Every method but
`new` is optional; a panic stays inside the mod.

### Step 5. Build

`cargo horizon-mod android` → `target/horizon-mod/<id>.hnmod`. `--bundle file.hnmod` adds the library to an
existing desktop bundle; `--lib libmine.so` packs a library built elsewhere.

### Step 6. Install on the phone

Open or share the `.hnmod` with Horizon, or **Profile → Install a mod**, or copy it into
`Android/data/dev.horizonlab.horizon/files/mods/`. Restart; **Options → Mods** lists and toggles mods. In a
world you should see "Hello Android" top left; type `/hello` in chat.

### Available API and limitations

Same as listed above: `tick`/`hud`/`key`/`command` callbacks; `log`, `chat`, `hud_line`, `data_dir`;
player state. No world/block/item/server hooks, arm64 only, source-only desktop `.hnmod` files don't run,
and mods are unsandboxed native code — install only mods you trust.

---

# Reference

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
