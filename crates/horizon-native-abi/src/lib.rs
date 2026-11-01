//! Prebuilt native mods: a shared library inside a `.hnmod` (`bin/<target>/`,
//! named by `binaries` in its `horizon-native.json`) that the game opens at
//! start, where it can't build the mod from source (Android). The library
//! exports [`ENTRY`]; everything crossing the boundary is `#[repr(C)]`, so
//! a mod builds with any Rust (or C) toolchain for the target.
//!
//! A mod in Rust implements [`NativeMod`] and calls [`export_mod!`]:
//!
//! ```ignore
//! struct Hello { ticks: u64 }
//! impl horizon_native_abi::NativeMod for Hello {
//!     fn new(_host: &mut horizon_native_abi::Host) -> Self { Hello { ticks: 0 } }
//!     fn tick(&mut self, _host: &mut horizon_native_abi::Host, _state: &horizon_native_abi::State) {
//!         self.ticks += 1;
//!     }
//! }
//! horizon_native_abi::export_mod!(Hello);
//! ```
use std::ffi::c_void;

/// The ABI version. Raised only when a field is added at the end of a
/// struct below; the game opens a mod whose `abi` is this or lower.
pub const ABI: u32 = 1;
/// The symbol a mod exports: `extern "C" fn(&mut Host) -> *const ModTable`.
pub const ENTRY: &str = "horizon_mod_entry";
/// The target name of an Android (arm64) build in `binaries`.
pub const ANDROID_ARM64: &str = "aarch64-linux-android";

/// Borrowed UTF-8 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}
impl Str {
    pub fn new(s: &str) -> Self {
        Str {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }
    /// # Safety
    /// `ptr` and `len` name live bytes for the borrow.
    pub unsafe fn as_str<'a>(self) -> &'a str {
        if self.ptr.is_null() || self.len == 0 {
            return "";
        }
        // SAFETY: the caller's promise; invalid UTF-8 reads as empty.
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(bytes).unwrap_or("")
    }
}

/// What the game hands a mod in each call. Its functions queue what the
/// mod asks for; the game does it after the call returns.
#[repr(C)]
pub struct Host {
    pub abi: u32,
    pub ctx: *mut c_void,
    /// A line in the game's log, under the mod's id.
    pub log: extern "C" fn(ctx: *mut c_void, text: Str),
    /// A line in chat from the client.
    pub chat: extern "C" fn(ctx: *mut c_void, text: Str),
    /// A line of the mod's HUD box (only inside `hud`), `rgb` 0xRRGGBB.
    pub hud_line: extern "C" fn(ctx: *mut c_void, text: Str, rgb: u32),
    /// The mod's own folder for its files (made for it).
    pub data_dir: Str,
}
impl Host {
    pub fn log(&mut self, text: &str) {
        (self.log)(self.ctx, Str::new(text));
    }
    pub fn chat(&mut self, text: &str) {
        (self.chat)(self.ctx, Str::new(text));
    }
    pub fn hud_line(&mut self, text: &str, rgb: u32) {
        (self.hud_line)(self.ctx, Str::new(text), rgb);
    }
    pub fn data_dir(&self) -> std::path::PathBuf {
        // SAFETY: the game keeps the folder's name alive while the mod is open.
        unsafe { self.data_dir.as_str() }.into()
    }
}

/// The game as a mod sees it in `tick` and `hud`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    /// In a world, with a player.
    pub in_world: bool,
    /// Playing: no screen open.
    pub playing: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    /// Game ticks since start.
    pub ticks: u64,
}

/// What a mod's [`ENTRY`] returns: its calls, each optional, and the state
/// they get back. The table lives as long as the library.
#[repr(C)]
pub struct ModTable {
    pub abi: u32,
    pub state: *mut c_void,
    /// Once a client tick.
    pub tick: Option<extern "C" fn(state: *mut c_void, host: &mut Host, game: &State)>,
    /// Once a frame the HUD draws (not under F1): lines through `hud_line`.
    pub hud: Option<extern "C" fn(state: *mut c_void, host: &mut Host, game: &State)>,
    /// A key or mouse button by vanilla name (`key.keyboard.z`), down or up.
    pub key: Option<extern "C" fn(state: *mut c_void, host: &mut Host, key: Str, pressed: bool)>,
    /// A chat command without its slash; true when the mod ran it.
    pub command: Option<extern "C" fn(state: *mut c_void, host: &mut Host, command: Str) -> bool>,
}

/// A mod written in Rust: [`export_mod!`] makes its [`ModTable`].
pub trait NativeMod: Sized + 'static {
    fn new(host: &mut Host) -> Self;
    fn tick(&mut self, host: &mut Host, game: &State) {
        let _ = (host, game);
    }
    fn hud(&mut self, host: &mut Host, game: &State) {
        let _ = (host, game);
    }
    fn key(&mut self, host: &mut Host, key: &str, pressed: bool) {
        let _ = (host, key, pressed);
    }
    fn command(&mut self, host: &mut Host, command: &str) -> bool {
        let _ = (host, command);
        false
    }
}

#[doc(hidden)]
pub mod glue {
    use super::*;
    /// Calls `f` on the mod; a panic stays in the mod's library.
    fn with<M: NativeMod, R: Default>(state: *mut c_void, f: impl FnOnce(&mut M) -> R) -> R {
        // SAFETY: `state` is the Box made in `entry`, used from the game's one thread.
        let m = unsafe { &mut *state.cast::<M>() };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(m))).unwrap_or_default()
    }
    pub extern "C" fn tick<M: NativeMod>(s: *mut c_void, host: &mut Host, game: &State) {
        with::<M, ()>(s, |m| m.tick(host, game));
    }
    pub extern "C" fn hud<M: NativeMod>(s: *mut c_void, host: &mut Host, game: &State) {
        with::<M, ()>(s, |m| m.hud(host, game));
    }
    pub extern "C" fn key<M: NativeMod>(s: *mut c_void, host: &mut Host, key: Str, pressed: bool) {
        // SAFETY: the game's string, alive for the call.
        let key = unsafe { key.as_str() };
        with::<M, ()>(s, |m| m.key(host, key, pressed));
    }
    pub extern "C" fn command<M: NativeMod>(s: *mut c_void, host: &mut Host, c: Str) -> bool {
        // SAFETY: the game's string, alive for the call.
        let c = unsafe { c.as_str() };
        with::<M, bool>(s, |m| m.command(host, c))
    }
    pub fn entry<M: NativeMod>(host: &mut Host) -> *const ModTable {
        let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| M::new(host)));
        let Ok(m) = made else {
            return std::ptr::null();
        };
        let table = ModTable {
            abi: ABI,
            state: Box::into_raw(Box::new(m)).cast(),
            tick: Some(tick::<M>),
            hud: Some(hud::<M>),
            key: Some(key::<M>),
            command: Some(command::<M>),
        };
        Box::into_raw(Box::new(table))
    }
}

/// Exports [`ENTRY`] for a [`NativeMod`] type.
#[macro_export]
macro_rules! export_mod {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_mod_entry(host: &mut $crate::Host) -> *const $crate::ModTable {
            $crate::glue::entry::<$ty>(host)
        }
    };
}

#[cfg(feature = "host")]
pub mod host;
