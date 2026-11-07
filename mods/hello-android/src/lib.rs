//! Hello Android: shows where the player stands in the top left corner,
//! answers `/hello` in chat and counts presses of the H key.
use horizon_native_abi::{Host, NativeMod, State, export_mod};

struct Hello {
    presses: u32,
}

impl NativeMod for Hello {
    fn new(host: &mut Host) -> Self {
        host.log("hello from a prebuilt mod");
        let presses = std::fs::read_to_string(host.data_dir().join("presses"))
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0);
        Hello { presses }
    }
    fn hud(&mut self, host: &mut Host, game: &State) {
        if game.in_world {
            host.hud_line("Hello Android", 0xFFFF55);
            host.hud_line(
                &format!("{:.1} {:.1} {:.1}", game.x, game.y, game.z),
                0xFFFFFF,
            );
        }
    }
    fn key(&mut self, host: &mut Host, key: &str, pressed: bool) {
        if pressed && key == "key.keyboard.h" {
            self.presses += 1;
            let _ = std::fs::write(host.data_dir().join("presses"), self.presses.to_string());
        }
    }
    fn command(&mut self, host: &mut Host, command: &str) -> bool {
        if command != "hello" {
            return false;
        }
        host.chat(&format!(
            "Hello from Android! H pressed {} times.",
            self.presses
        ));
        true
    }
}

export_mod!(Hello);
