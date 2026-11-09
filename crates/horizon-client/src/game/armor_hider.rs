//! Zannagh's Armor Hider (MIT), from its documented behaviour: each armour piece the
//! player wears can be hidden from their own model while still worn and protecting.
//! In the inventory, H over an armour slot shows or hides that piece. Kept in
//! `horizon-mods/.horizon-mod-data/armor_hider/config.json`.
use super::*;
use std::sync::Mutex;

const CONFIG: &str = "horizon-mods/.horizon-mod-data/armor_hider/config.json";
const NAMES: [&str; 4] = ["head", "chest", "legs", "feet"];

static HIDDEN: Mutex<Option<[bool; 4]>> = Mutex::new(None);

/// Which pieces (head to feet) are hidden.
pub(super) fn hidden() -> [bool; 4] {
    let mut guard = HIDDEN.lock().unwrap_or_else(|e| e.into_inner());
    *guard.get_or_insert_with(|| {
        let v: serde_json::Value = std::fs::read_to_string(CONFIG)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(serde_json::Value::Null);
        NAMES.map(|n| v[n].as_bool().unwrap_or(false))
    })
}

fn set(hidden: [bool; 4]) {
    *HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(hidden);
    let mut map = serde_json::Map::new();
    for (n, h) in NAMES.iter().zip(hidden) {
        map.insert((*n).to_owned(), serde_json::Value::Bool(h));
    }
    let path = std::path::Path::new(CONFIG);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, serde_json::Value::Object(map).to_string());
}

impl Game<'_> {
    /// H over an armour slot of the inventory: shows or hides that piece.
    pub(super) fn armor_hider_key(&mut self) -> bool {
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let batch = HudBatch::new(size);
        let slot = self.slot_at(&batch, self.cursor.0, self.cursor.1);
        let Some(slot) = slot else {
            return false;
        };
        if !(500..504).contains(&slot) {
            return false;
        }
        let i = slot - 500;
        let mut h = hidden();
        h[i] = !h[i];
        set(h);
        let piece = ["Helmet", "Chestplate", "Leggings", "Boots"][i];
        let state = if h[i] { "hidden" } else { "shown" };
        log::info!("[armor_hider] {piece} {state}");
        self.send_overlay_message(horizon_client::text::Text::literal(format!("{piece}: {state}")));
        true
    }
}
