// Naturally Trimmed for Horizon. Config: horizon-mods/.horizon-mod-data/naturally_trimmed/config.json
use horizon_client::mobs::{finalize, gear};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const CONFIG: &str = "horizon-mods/.horizon-mod-data/naturally_trimmed/config.json";
const DEFAULT_CONFIG: &str = "{\n  \"trimChance\": 33,\n  \"pieceTrimChance\": 75\n}\n";
const TAG: &str = "naturally_trimmed";

struct Config {
    trim_chance: u64,
    piece_chance: u64,
}

fn config() -> &'static Config {
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
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        Config {
            trim_chance: v["trimChance"].as_u64().unwrap_or(33).min(100),
            piece_chance: v["pieceTrimChance"].as_u64().unwrap_or(75).min(100),
        }
    })
}

/// The registry's trim materials and patterns (the prepared game data's).
fn ids(root: &std::path::Path, kind: &str) -> Vec<Arc<str>> {
    let mut out: Vec<Arc<str>> = std::fs::read_dir(root.join("data/minecraft").join(kind))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".json").map(|n| Arc::from(format!("minecraft:{n}")))
        })
        .collect();
    out.sort();
    out
}

static SEED: AtomicU64 = AtomicU64::new(0);
fn next(bound: u64) -> u64 {
    let mut s = SEED.load(Ordering::Relaxed);
    if s == 0 {
        s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(7);
    }
    s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    SEED.store(s, Ordering::Relaxed);
    let mut z = s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z % bound.max(1)
}

static SEEN: Mutex<Option<std::collections::HashSet<u32>>> = Mutex::new(None);
static TRIMS: OnceLock<(Vec<Arc<str>>, Vec<Arc<str>>)> = OnceLock::new();

const SLOTS: [(usize, usize); 4] = [
    (finalize::HEAD, gear::HEAD),
    (finalize::CHEST, gear::CHEST),
    (finalize::LEGS, gear::LEGS),
    (finalize::FEET, gear::FEET),
];

pub(super) fn tick(game: &mut super::Game<'_>) {
    if game.remote.is_some() || game.mobs.is_empty() {
        return;
    }
    let (materials, patterns) =
        TRIMS.get_or_init(|| (ids(&game.options.resources, "trim_material"), ids(&game.options.resources, "trim_pattern")));
    if materials.is_empty() || patterns.is_empty() {
        return;
    }
    let c = config();
    let trimmable = game.items().item_tags.get("minecraft:trimmable_armor").cloned().unwrap_or_default();
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let seen = seen.get_or_insert_with(Default::default);
    let new: Vec<u32> = game.mobs.iter().filter(|m| !seen.contains(&m.id)).map(|m| m.id).collect();
    for id in new {
        seen.insert(id);
        let Some(m) = game.mobs.iter().find(|m| m.id == id) else { continue };
        let pieces: Vec<usize> = SLOTS
            .iter()
            .filter(|(f, _)| finalize::item(m, *f).is_some_and(|i| trimmable.contains(&*i)))
            .map(|(_, g)| *g)
            .collect();
        if pieces.is_empty() {
            continue;
        }
        let target = super::commands::Target::Mob(id);
        let mut tags = game.tags_of(target);
        if tags.iter().any(|t| t == TAG) {
            continue;
        }
        tags.push(TAG.to_owned());
        game.set_entity_tags(target, &tags);
        // TrimApplier.trimEquipment: trimChance >= nextInt(100), one trim for
        // the mob, each piece pieceTrimChance >= nextInt(100).
        if c.trim_chance < next(100) {
            continue;
        }
        let trim = (
            materials[next(materials.len() as u64) as usize].clone(),
            patterns[next(patterns.len() as u64) as usize].clone(),
        );
        if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
            for slot in pieces {
                if c.piece_chance >= next(100) {
                    gear::set_trim(m, slot, Some(trim.clone()));
                }
            }
        }
    }
}
