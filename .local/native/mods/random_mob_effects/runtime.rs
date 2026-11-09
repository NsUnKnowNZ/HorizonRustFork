// Random Mob Effects for Horizon. Config: horizon-mods/.horizon-mod-data/random_mob_effects/
//   blacklist.txt (Serilum's format: one effect id per line, a "!" in front disables it)
//   config.json   {"potionEffectLevel": 1, "hideEffectParticles": false, "disableCreepers": true}
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const DIR: &str = "horizon-mods/.horizon-mod-data/random_mob_effects";
const DEFAULT_LIST: &str = "minecraft:speed,\nminecraft:slowness,\nminecraft:haste,\nminecraft:mining_fatigue,\nminecraft:strength,\n!minecraft:instant_health,\n!minecraft:instant_damage,\nminecraft:jump_boost,\nminecraft:nausea,\nminecraft:regeneration,\nminecraft:resistance,\nminecraft:fire_resistance,\nminecraft:water_breathing,\n!minecraft:invisibility,\nminecraft:blindness,\nminecraft:night_vision,\nminecraft:hunger,\nminecraft:weakness,\nminecraft:poison,\n!minecraft:wither,\nminecraft:health_boost,\nminecraft:absorption,\nminecraft:saturation,\n!minecraft:glowing,\n!minecraft:levitation,\nminecraft:luck,\nminecraft:unluck,\nminecraft:slow_falling,\nminecraft:conduit_power,\nminecraft:dolphins_grace,\n!minecraft:bad_omen,\n!minecraft:hero_of_the_village,\nminecraft:darkness,\n!minecraft:trial_omen,\n!minecraft:raid_omen,\nminecraft:wind_charged,\nminecraft:weaving,\nminecraft:oozing,\nminecraft:infested,\n";
const DEFAULT_CONFIG: &str = "{\n  \"potionEffectLevel\": 1,\n  \"hideEffectParticles\": false,\n  \"disableCreepers\": true\n}\n";

pub struct Config {
    pub effects: Vec<&'static str>,
    pub amplifier: i32,
    pub hide_particles: bool,
    pub no_creepers: bool,
}

/// The enabled vanilla effects of a blacklist (other mods' are left out).
pub fn parse_list(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_end_matches(',').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('!') && !l.starts_with('#'))
        .filter(|l| l.starts_with("minecraft:"))
        .map(str::to_owned)
        .collect()
}

fn read_or_write(name: &str, default: &str) -> String {
    let path = std::path::Path::new(DIR).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        let _ = std::fs::create_dir_all(DIR);
        let _ = std::fs::write(&path, default);
        default.to_owned()
    })
}

fn config() -> &'static Config {
    static CELL: OnceLock<Config> = OnceLock::new();
    CELL.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(&read_or_write("config.json", DEFAULT_CONFIG)).unwrap_or_default();
        let effects = parse_list(&read_or_write("blacklist.txt", DEFAULT_LIST))
            .into_iter()
            .map(|e| &*Box::leak(e.into_boxed_str()))
            .collect::<Vec<&'static str>>();
        log::info!("[random_mob_effects] {} effects to choose from", effects.len());
        Config {
            effects,
            amplifier: (v["potionEffectLevel"].as_i64().unwrap_or(1).clamp(1, 50) - 1) as i32,
            hide_particles: v["hideEffectParticles"].as_bool().unwrap_or(false),
            no_creepers: v["disableCreepers"].as_bool().unwrap_or(true),
        }
    })
}

/// The tag a mob carries once it has had its effect (saved with it, as `/tag` does).
const TAG: &str = "random_mob_effects";
static SEEN: Mutex<Option<std::collections::HashSet<u32>>> = Mutex::new(None);
static SEED: AtomicU64 = AtomicU64::new(0);

fn next_random(bound: usize) -> usize {
    // SplitMix64 over a counter seeded from the clock.
    let mut s = SEED.load(Ordering::Relaxed);
    if s == 0 {
        s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
    }
    s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    SEED.store(s, Ordering::Relaxed);
    let mut z = s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z % bound.max(1) as u64) as usize
}

/// Every server tick: each hostile mob not seen before gets its effect once.
pub(super) fn tick(game: &mut super::Game<'_>) {
    if game.remote.is_some() || game.mobs.is_empty() {
        return;
    }
    let c = config();
    if c.effects.is_empty() {
        return;
    }
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let seen = seen.get_or_insert_with(Default::default);
    let new: Vec<u32> = game
        .mobs
        .iter()
        .filter(|m| !seen.contains(&m.id))
        .filter(|m| m.category() == "monster")
        .filter(|m| !(c.no_creepers && &*m.kind.id == "minecraft:creeper"))
        .map(|m| m.id)
        .collect();
    for id in new {
        seen.insert(id);
        let target = super::commands::Target::Mob(id);
        let mut tags = game.tags_of(target);
        if tags.iter().any(|t| t == TAG) {
            continue;
        }
        let effect = c.effects[next_random(c.effects.len())];
        if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
            m.add_effect_with(effect, i32::MAX, c.amplifier, false, !c.hide_particles);
        }
        tags.push(TAG.to_owned());
        game.set_entity_tags(target, &tags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blacklist_keeps_enabled_vanilla_effects() {
        let l = parse_list("minecraft:speed,\n!minecraft:wither,\nfriendsandfoes:reach,\n");
        assert_eq!(l, vec!["minecraft:speed".to_owned()]);
    }
}
