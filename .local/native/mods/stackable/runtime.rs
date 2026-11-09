// Stackable for Horizon. Config: horizon-mods/.horizon-mod-data/stackable/config.json
// (Stackable.json's shape: {"maxStack": 100, "StacksizeOverride": [{"namespaceID": "...", "value": 50}]}).

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

const CONFIG: &str = "horizon-mods/.horizon-mod-data/stackable/config.json";
const DEFAULT_CONFIG: &str = r#"{
  "maxStack": 100,
  "StacksizeOverride": [
    { "namespaceID": "minecraft:egg", "value": 50 },
    { "namespaceID": "minecraft:snowball", "value": 50 },
    { "namespaceID": "minecraft:golden_apple", "value": 50 },
    { "namespaceID": "minecraft:enchanted_golden_apple", "value": 10 }
  ]
}
"#;

pub struct Config {
    pub max: i64,
    pub overrides: HashMap<String, i64>,
}

pub fn parse(text: &str) -> Config {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let max = v["maxStack"].as_i64().unwrap_or(64).clamp(1, i32::MAX as i64);
    let overrides = v["StacksizeOverride"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| Some((o["namespaceID"].as_str()?.to_owned(), o["value"].as_i64()?.max(1))))
        .collect();
    Config { max, overrides }
}

fn config() -> &'static Config {
    static CONFIG_CELL: OnceLock<Config> = OnceLock::new();
    CONFIG_CELL.get_or_init(|| {
        let path = std::path::Path::new(CONFIG);
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, DEFAULT_CONFIG);
            DEFAULT_CONFIG.to_owned()
        });
        let c = parse(&text);
        log::info!("[stackable] 64-stacks become {}, {} overrides", c.max, c.overrides.len());
        c
    })
}

pub fn size_for(c: &Config, item: &str, current: Option<i64>) -> Option<i64> {
    if let Some(v) = c.overrides.get(item) {
        return Some(*v);
    }
    // Only the common default (64) changes, as the original's DataComponents
    // patch does: 16-stacks and unstackables keep theirs.
    (current == Some(64)).then_some(c.max)
}

pub fn apply(item: &str, components: &mut Map<String, Value>) {
    let current = components.get("minecraft:max_stack_size").and_then(Value::as_i64);
    if let Some(size) = size_for(config(), item, current) {
        components.insert("minecraft:max_stack_size".into(), size.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_64_stacks_and_overrides_change() {
        let c = parse(DEFAULT_CONFIG);
        assert_eq!(size_for(&c, "minecraft:stone", Some(64)), Some(100));
        assert_eq!(size_for(&c, "minecraft:ender_pearl", Some(16)), None);
        assert_eq!(size_for(&c, "minecraft:diamond_sword", Some(1)), None);
        assert_eq!(size_for(&c, "minecraft:egg", Some(16)), Some(50));
        assert_eq!(size_for(&c, "minecraft:enchanted_golden_apple", Some(64)), Some(10));
    }
}
