//! Entity Culling for Horizon (Android): turns on the game's occlusion culling of
//! entities and block entities hidden behind blocks, and hides their name tags.
//!
//! Settings, in `config.txt` in the mod's folder (made on first start):
//!
//! ```text
//! mode=async        # async (default), sync or off
//! name_tags=true    # hide name tags of entities behind blocks
//! ```
//!
//! In game: `/entityculling` shows them, `/entityculling async|sync|off` and
//! `/entityculling tags on|off` change and save them. The player's own
//! Entity Culling video setting, when not off, wins over the mod.
use horizon_native_abi::{Host, NativeMod, State, export_mod, options};
use std::path::PathBuf;

struct Config {
    mode: &'static str,
    name_tags: bool,
}

impl Config {
    fn read(path: &PathBuf) -> Config {
        let mut c = Config { mode: "async", name_tags: true };
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            match line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("mode", v)) => c.mode = mode(v).unwrap_or(c.mode),
                Some(("name_tags", v)) => c.name_tags = v == "true",
                _ => {}
            }
        }
        c
    }
    fn write(&self, path: &PathBuf) {
        let text = format!(
            "# Entity Culling for Horizon\n# mode: async, sync or off\nmode={}\n# hide name tags of entities behind blocks: true or false\nname_tags={}\n",
            self.mode, self.name_tags
        );
        let _ = std::fs::write(path, text);
    }
    fn apply(&self, host: &mut Host) {
        host.set_option(options::ENTITY_CULLING, self.mode);
        host.set_option(options::HIDE_OCCLUDED_NAME_TAGS, if self.name_tags { "true" } else { "false" });
    }
}

fn mode(v: &str) -> Option<&'static str> {
    match v {
        "async" | "asynchronous" => Some("async"),
        "sync" | "synchronous" => Some("sync"),
        "off" => Some("off"),
        _ => None,
    }
}

struct EntityCulling {
    path: PathBuf,
    config: Config,
}

impl NativeMod for EntityCulling {
    fn new(host: &mut Host) -> Self {
        let path = host.data_dir().join("config.txt");
        let config = Config::read(&path);
        if !path.is_file() {
            config.write(&path);
        }
        config.apply(host);
        host.log(&format!("entity culling {}, name tags hidden: {}", config.mode, config.name_tags));
        EntityCulling { path, config }
    }

    fn tick(&mut self, host: &mut Host, game: &State) {
        // Again now and then, in case the game reset its options.
        if game.ticks % 200 == 0 {
            self.config.apply(host);
        }
    }

    fn command(&mut self, host: &mut Host, command: &str) -> bool {
        let mut words = command.split_whitespace();
        if words.next() != Some("entityculling") {
            return false;
        }
        match (words.next(), words.next()) {
            (None, _) => {}
            (Some("tags"), Some(v @ ("on" | "off"))) => self.config.name_tags = v == "on",
            (Some(v), None) if mode(v).is_some() => self.config.mode = mode(v).unwrap_or("async"),
            _ => {
                host.chat("Usage: /entityculling [async|sync|off] | /entityculling tags on|off");
                return true;
            }
        }
        self.config.write(&self.path);
        self.config.apply(host);
        host.chat(&format!(
            "Entity Culling: {}, name tags behind blocks {}",
            self.config.mode,
            if self.config.name_tags { "hidden" } else { "shown" }
        ));
        true
    }
}

export_mod!(EntityCulling);
