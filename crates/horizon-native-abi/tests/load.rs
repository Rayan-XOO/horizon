//! Opens a built mod library (`HORIZON_ABI_TEST_LIB`, e.g. mods/hello-android built
//! for this machine) and drives it as the game does. Skipped when unset.
#![cfg(feature = "host")]
use horizon_native_abi::{State, host::Loaded};

#[test]
fn opens_a_mod_and_calls_it() {
    let Some(lib) = std::env::var_os("HORIZON_ABI_TEST_LIB") else {
        return;
    };
    let data = std::env::temp_dir().join(format!("horizon-abi-{}", std::process::id()));
    // SAFETY: the test's own example mod.
    let (m, out) = unsafe { Loaded::open(lib.as_ref(), &data) }.unwrap();
    assert_eq!(out.log, ["hello from a prebuilt mod"]);
    let game = State {
        in_world: true,
        x: 1.0,
        y: 64.0,
        z: -2.0,
        ..State::default()
    };
    let hud = m.hud(&game).hud;
    assert_eq!(hud[0].0, "Hello Android");
    assert_eq!(hud[1].0, "1.0 64.0 -2.0");
    m.key("key.keyboard.h", true);
    let (ran, out) = m.command("hello");
    assert!(ran);
    assert_eq!(out.chat, ["Hello from Android! H pressed 1 times."]);
    assert!(!m.command("other").0);
    let _ = std::fs::remove_dir_all(data);
}
