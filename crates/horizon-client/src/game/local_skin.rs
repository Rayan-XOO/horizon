//! The player's own skin on Android: `horizon-skin.png` (64x64 or 64x32) and
//! `horizon-skin.txt` (`model:slim` or `model:classic`) in the game directory,
//! written by the profile screen (ProfileActivity). Checked once a second; a
//! changed file is put in an atlas skin slot (`store_skin`) and on the
//! player's model, slim arms through the model's `slim` flag (`player_slim#main`).
//!
//! Every world, single player too, is joined through a connection (the
//! integrated server), so this runs from the connected tick and wins over the
//! skin the server's profile names (`update_remote_skins`).
use super::Game;
use std::time::SystemTime;

#[derive(Default)]
pub(super) struct LocalSkin {
    checked: Option<std::time::Instant>,
    stamp: Option<(SystemTime, SystemTime)>,
    location: Option<String>,
    rgba: Vec<u8>,
    slim: bool,
}

const PNG: &str = "horizon-skin.png";
const MODEL: &str = "horizon-skin.txt";

fn mtime(name: &str) -> SystemTime {
    std::fs::metadata(name)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

impl LocalSkin {
    /// The atlas location of the player's own skin, if one is set.
    pub(super) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    /// The skin and arm model to draw the local player with, if one is set.
    pub(super) fn chosen(&self) -> Option<(String, bool)> {
        self.location.clone().map(|l| (l, self.slim))
    }
}

impl Game<'_> {
    pub(super) fn tick_local_skin(&mut self) {
        let now = std::time::Instant::now();
        let s = &mut self.local_skin;
        if s.checked
            .is_none_or(|t| now.duration_since(t).as_secs() >= 1)
        {
            s.checked = Some(now);
            let stamp = (mtime(PNG), mtime(MODEL));
            if s.stamp != Some(stamp) {
                s.stamp = Some(stamp);
                s.slim = std::fs::read_to_string(MODEL).is_ok_and(|t| t.contains("model:slim"));
                let skin = horizon_client::hud::decode(std::path::Path::new(PNG))
                    .ok()
                    .and_then(|(w, h, rgba)| {
                        horizon_client::skins::process_legacy_skin(w, h, &rgba)
                    });
                match skin {
                    Some(rgba) => {
                        let secs = stamp
                            .0
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_or(0, |d| d.as_secs());
                        s.location = Some(format!("horizon/local_skin_{secs}"));
                        s.rgba = rgba;
                    }
                    None => {
                        s.location = None;
                        s.rgba.clear();
                    }
                }
                // A fresh model, so the arm width follows the setting.
                self.player_model = None;
            }
        }
        let Some(location) = self.local_skin.location.clone() else {
            return;
        };
        if !(self.skin_slots.contains(&location) && self.gui.atlas.sprites.contains_key(&location))
        {
            let rgba = self.local_skin.rgba.clone();
            self.store_skin(location.clone(), rgba);
        }
        let slim = self.local_skin.slim;
        let differs = self
            .player_model
            .as_ref()
            .is_some_and(|m| m.skin.as_deref() != Some(location.as_str()) || m.slim != slim);
        if differs {
            let kind = self.player_model.as_ref().map(|m| m.kind.clone());
            let skin_3d = kind.and_then(|kind| self.skin_3d_model(&kind, &location, slim));
            if let Some(m) = self.player_model.as_mut() {
                // AvatarRenderer picks `player_slim#main` from `slim`; the first-person
                // arm reads the same flag.
                m.skin = Some(location);
                m.slim = slim;
                m.skin_3d = skin_3d;
            }
        }
    }
}
