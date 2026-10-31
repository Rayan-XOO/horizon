//! The phone's thermal status (`AThermal_getCurrentThermalStatus`, API 30), read on its own
//! thread every two seconds so the game thread never makes the binder call. Uncapped frame
//! rates heat a phone until the system holds its GPU at the lowest clocks; the frame limiter
//! lowers its cap while the status says the phone is hot, which lets the clocks come back.
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Once;

static STATUS: AtomicI32 = AtomicI32::new(0);
static START: Once = Once::new();

type Acquire = unsafe extern "C" fn() -> *mut libc::c_void;
type Status = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;

fn start() {
    START.call_once(|| {
        // SAFETY: dlopen/dlsym on the system's libandroid; the symbols are used with their
        // NDK signatures, and only when present (API 30 and newer).
        let (acquire, status) = unsafe {
            let lib = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW);
            if lib.is_null() {
                return;
            }
            let a = libc::dlsym(lib, c"AThermal_acquireManager".as_ptr());
            let s = libc::dlsym(lib, c"AThermal_getCurrentThermalStatus".as_ptr());
            if a.is_null() || s.is_null() {
                return;
            }
            (std::mem::transmute::<*mut libc::c_void, Acquire>(a), std::mem::transmute::<*mut libc::c_void, Status>(s))
        };
        let _ = std::thread::Builder::new().name("horizon-thermal".into()).spawn(move || {
            // SAFETY: as above; the manager is kept for the process's life.
            let manager = unsafe { acquire() };
            if manager.is_null() {
                return;
            }
            let mut last = 0;
            loop {
                let now = unsafe { status(manager) }.max(0);
                if now != last {
                    log::info!("Horizon: thermal status {now}");
                    last = now;
                }
                STATUS.store(now, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    });
}

/// The frame cap the phone's temperature allows (`i32::MAX` when it is cool).
pub(super) fn frame_cap() -> i32 {
    start();
    match STATUS.load(Ordering::Relaxed) {
        // NONE, LIGHT
        0 | 1 => i32::MAX,
        // MODERATE
        2 => 90,
        // SEVERE
        3 => 60,
        // CRITICAL and above
        _ => 45,
    }
}
