//! The player's own skin on Android: `horizon-skin.png` (64x64 or 64x32) and
//! `horizon-skin.txt` (`model:slim` or `model:classic`) in the game directory,
//! written by the profile screen (ProfileActivity). Checked once a second; a
//! changed file is put in an atlas skin slot (`store_skin`) and on the
//! player's model, slim arms through `player_slim#main`.
use super::Game;
use horizon_client::mobs::{Mob, MobType};
use std::{sync::Arc, time::SystemTime};

#[derive(Default)]
pub(super) struct LocalSkin {
    checked: Option<std::time::Instant>,
    stamp: Option<(SystemTime, SystemTime)>,
    location: Option<String>,
    rgba: Vec<u8>,
    slim: bool,
    slim_type: Option<Option<Arc<MobType>>>,
}

const PNG: &str = "horizon-skin.png";
const MODEL: &str = "horizon-skin.txt";

fn mtime(name: &str) -> SystemTime {
    std::fs::metadata(name).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}

impl LocalSkin {
    /// The atlas location of the player's own skin, if one is set.
    pub(super) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
}

impl Game<'_> {
    pub(super) fn tick_local_skin(&mut self) {
        let now = std::time::Instant::now();
        let s = &mut self.local_skin;
        if s.checked.is_none_or(|t| now.duration_since(t).as_secs() >= 1) {
            s.checked = Some(now);
            let stamp = (mtime(PNG), mtime(MODEL));
            if s.stamp != Some(stamp) {
                s.stamp = Some(stamp);
                s.slim = std::fs::read_to_string(MODEL).is_ok_and(|t| t.contains("model:slim"));
                let skin = horizon_client::hud::decode(std::path::Path::new(PNG))
                    .ok()
                    .and_then(|(w, h, rgba)| horizon_client::skins::process_legacy_skin(w, h, &rgba));
                match skin {
                    Some(rgba) => {
                        let secs = stamp.0.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
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
        if !(self.skin_slots.contains(&location) && self.gui.atlas.sprites.contains_key(&location)) {
            let rgba = self.local_skin.rgba.clone();
            self.store_skin(location.clone(), rgba);
        }
        if self.local_skin.slim && self.player.is_some() {
            if self.local_skin.slim_type.is_none() {
                let slim = self.res.player_type().as_ref().and_then(|kind| {
                    horizon_client::mobs::load_model(&self.options.resources, "minecraft:player_slim#main")
                        .map(|model| Arc::new(MobType { model: Arc::new(model), ..(**kind).clone() }))
                });
                if slim.is_none() {
                    log::warn!("Horizon: no player_slim model; classic arms");
                }
                self.local_skin.slim_type = Some(slim);
            }
            let is_slim = |m: &Mob| self.local_skin.slim_type.as_ref().and_then(|t| t.as_ref()).is_some_and(|t| Arc::ptr_eq(&m.kind, t));
            if !self.player_model.as_ref().is_some_and(is_slim) {
                if let (Some(kind), Some(p)) = (self.local_skin.slim_type.clone().flatten(), self.player.as_ref()) {
                    self.player_model = Some(Mob::new(0, kind, p.position, self.yaw));
                }
            }
        }
        if let Some(m) = self.player_model.as_mut() {
            m.skin = Some(location);
        }
    }
}
