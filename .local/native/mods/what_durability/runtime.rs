// What Durability? for Horizon. Config: horizon-mods/.horizon-mod-data/what_durability/config.json
// {"itemsWithDurability": "minecraft:elytra,#minecraft:trimmable_armor"}  (empty: nothing keeps durability)

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const CONFIG: &str = "horizon-mods/.horizon-mod-data/what_durability/config.json";

pub struct Keep {
    pub items: HashSet<String>,
    pub tags: Vec<String>,
}

pub fn parse(list: &str) -> Keep {
    let mut keep = Keep { items: HashSet::new(), tags: Vec::new() };
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (is_tag, id) = match entry.strip_prefix('#') {
            Some(rest) => (true, rest),
            None => (false, entry),
        };
        let id = if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") };
        if is_tag { keep.tags.push(id) } else { keep.items.insert(id); }
    }
    keep
}

fn config() -> &'static Keep {
    static KEEP: OnceLock<Keep> = OnceLock::new();
    KEEP.get_or_init(|| {
        let path = std::path::Path::new(CONFIG);
        let list = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["itemsWithDurability"].as_str().map(str::to_owned))
                .unwrap_or_default(),
            Err(_) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(path, "{\n  \"itemsWithDurability\": \"\"\n}\n");
                String::new()
            }
        };
        let keep = parse(&list);
        log::info!(
            "[what_durability] items lose no durability (kept: {} items, {} tags)",
            keep.items.len(),
            keep.tags.len()
        );
        keep
    })
}

pub fn keeps(keep: &Keep, item: &str, tags: &HashMap<String, HashSet<String>>) -> bool {
    keep.items.contains(item)
        || keep.tags.iter().any(|t| tags.get(t).is_some_and(|members| members.contains(item)))
}

pub fn keeps_durability(item: &str, tags: &HashMap<String, HashSet<String>>) -> bool {
    keeps(config(), item, tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_keeps_nothing() {
        assert!(!keeps(&parse(""), "minecraft:diamond_pickaxe", &HashMap::new()));
    }

    #[test]
    fn items_and_tags_keep_durability() {
        let keep = parse("elytra, #minecraft:trimmable_armor");
        let mut tags = HashMap::new();
        tags.insert(
            "minecraft:trimmable_armor".to_owned(),
            HashSet::from(["minecraft:iron_helmet".to_owned()]),
        );
        assert!(keeps(&keep, "minecraft:elytra", &tags));
        assert!(keeps(&keep, "minecraft:iron_helmet", &tags));
        assert!(!keeps(&keep, "minecraft:iron_sword", &tags));
    }
}
