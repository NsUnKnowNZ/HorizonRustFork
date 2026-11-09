// So I Heard You Want To Customize Mob Difficulty for Horizon: a few of its mob tweaks.
use super::Game;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

const ARTHROPODS: [&str; 5] = ["spider", "cave_spider", "silverfish", "endermite", "bee"];
const UNDEAD: [&str; 12] = [
    "zombie", "husk", "drowned", "zombie_villager", "zombified_piglin", "skeleton", "stray",
    "wither_skeleton", "bogged", "phantom", "zoglin", "wither",
];
const FIRE: [&str; 4] = ["in_fire", "on_fire", "campfire", "hot_floor"];

/// What a hit leaves for the level's tick (the hit itself has no game to act on): the attacker,
/// what it dealt, and whether a player was hit.
static HITS: Mutex<Vec<(u32, f32, bool)>> = Mutex::new(Vec::new());

static SEED: AtomicU64 = AtomicU64::new(0x853C_49E6_748F_EA9B);

fn rand() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn path(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

pub(super) fn damage(kind: &str, damage_type: &str, causing: i32, amount: &mut f32, canceled: &mut bool) {
    if *canceled {
        return;
    }
    let (kind, damage) = (path(kind), path(damage_type));
    if (kind == "husk" && FIRE.contains(&damage))
        || (ARTHROPODS.contains(&kind) && damage == "fall")
        || (kind == "evoker" && damage == "arrow")
    {
        *canceled = true;
        return;
    }
    if causing > 0 && *amount > 0.0 && matches!(damage, "mob_attack" | "mob_attack_no_aggro") {
        HITS.lock().unwrap_or_else(|e| e.into_inner()).push((causing as u32, *amount, kind == "player"));
    }
}

pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let hits = std::mem::take(&mut *HITS.lock().unwrap_or_else(|e| e.into_inner()));
    for (attacker, amount, player) in hits {
        let Some(mob) = game.native_mob(dimension, attacker) else { continue };
        let kind = path(&mob.kind);
        if UNDEAD.contains(&kind) && mob.health > 0.0 {
            game.native_set_mob_health(dimension, attacker, mob.health + amount);
        }
        // The player it hit is the one nearest to it.
        if kind == "enderman" && player && rand() < 1.0 / 3.0 {
            let [x, y, z] = mob.pos;
            game.native_run_command(&format!(
                "execute in {dimension} positioned {x} {y} {z} run effect give @p[distance=..6] minecraft:blindness 5 0"
            ));
        }
    }
}
