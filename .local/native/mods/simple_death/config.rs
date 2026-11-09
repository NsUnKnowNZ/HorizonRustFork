// Simple Death Improvements for Horizon. Config: horizon-mods/.horizon-mod-data/simple_death/config.json
use std::sync::OnceLock;

const CONFIG: &str = "horizon-mods/.horizon-mod-data/simple_death/config.json";
const DEFAULT_CONFIG: &str = r#"{
  "keepHotbarOnDeath": true,
  "keepArmorOnDeath": true,
  "keepOffhandOnDeath": true,
  "noItemSplatterOnDeath": true,
  "noDeathItemDespawn": true,
  "playerDropMoreXpOnDeath": true,
  "droppedExperiencePercent": 80
}
"#;

pub struct Config {
    pub hotbar: bool,
    pub armor: bool,
    pub offhand: bool,
    pub no_splatter: bool,
    pub no_despawn: bool,
    pub more_xp: bool,
    pub xp_percent: u32,
}

pub fn parse(text: &str) -> Config {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    let b = |k: &str| v[k].as_bool().unwrap_or(true);
    Config {
        hotbar: b("keepHotbarOnDeath"),
        armor: b("keepArmorOnDeath"),
        offhand: b("keepOffhandOnDeath"),
        no_splatter: b("noItemSplatterOnDeath"),
        no_despawn: b("noDeathItemDespawn"),
        more_xp: b("playerDropMoreXpOnDeath"),
        xp_percent: v["droppedExperiencePercent"].as_u64().unwrap_or(80).min(100) as u32,
    }
}

pub fn config() -> &'static Config {
    static CELL: OnceLock<Config> = OnceLock::new();
    CELL.get_or_init(|| {
        let path = std::path::Path::new(CONFIG);
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, DEFAULT_CONFIG);
            DEFAULT_CONFIG.to_owned()
        });
        parse(&text)
    })
}

/// ServerPlayer's death experience: a share of every point, not level * 7.
pub fn death_reward(vanilla: u32, total: i32, keep: bool) -> u32 {
    let c = config();
    if keep || !c.more_xp {
        return vanilla;
    }
    (i64::from(total.max(0)) * i64::from(c.xp_percent) / 100) as u32
}
