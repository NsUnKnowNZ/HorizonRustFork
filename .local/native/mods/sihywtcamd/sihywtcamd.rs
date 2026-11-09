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

/// Mobs just spawned, for the level's tick: dimension, kind and where.
static SPAWNS: Mutex<Vec<(String, String, [f64; 3])>> = Mutex::new(Vec::new());

pub(super) fn spawned(dimension: &str, kind: &str, pos: [f64; 3]) {
    let k = path(kind);
    if matches!(k, "ghast" | "evoker" | "wither" | "vindicator") || k.contains("zombie") || k == "husk" || k == "drowned" {
        SPAWNS.lock().unwrap_or_else(|e| e.into_inner()).push((dimension.to_owned(), kind.to_owned(), pos));
    }
}

/// The spawn-time changes: more health for ghasts, evokers and withers, knockback resistance
/// and a varied speed for zombies, a speed bonus for some vindicators.
fn on_spawn(game: &mut Game<'_>, dimension: &str, kind: &str, pos: [f64; 3]) {
    let Some(mob) = game
        .native_mobs(dimension)
        .into_iter()
        .filter(|m| m.kind == kind)
        .min_by(|a, b| {
            let d = |m: &super::native_mobs::NativeMob| (0..3).map(|i| (m.pos[i] - pos[i]).powi(2)).sum::<f64>();
            d(a).total_cmp(&d(b))
        })
    else {
        return;
    };
    let [x, y, z] = mob.pos;
    let at = format!("execute in {dimension} positioned {x} {y} {z} as @e[type={kind},limit=1,sort=nearest,distance=..0.5] run");
    let health = match path(kind) {
        "ghast" => Some(20.0),
        "evoker" => Some(30.0),
        "wither" => Some(360.0),
        _ => None,
    };
    if let Some(h) = health {
        game.native_run_command(&format!("{at} attribute @s minecraft:max_health base set {h}"));
        game.native_set_mob_health(dimension, mob.id, h as f32);
    }
    let k = path(kind);
    if k.contains("zombie") || k == "husk" || k == "drowned" {
        game.native_run_command(&format!("{at} attribute @s minecraft:knockback_resistance modifier add sihywtcamd:sturdy 0.2 add_value"));
        let speed = (rand() - 0.5) * 0.2;
        game.native_run_command(&format!("{at} attribute @s minecraft:movement_speed modifier add sihywtcamd:variation {speed:.3} add_multiplied_base"));
    }
    if k == "vindicator" && rand() < 0.5 {
        game.native_run_command(&format!("{at} attribute @s minecraft:movement_speed modifier add sihywtcamd:speed_bonus 0.15 add_multiplied_base"));
    }
}

pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let spawns = std::mem::take(&mut *SPAWNS.lock().unwrap_or_else(|e| e.into_inner()));
    for (dim, kind, pos) in spawns {
        if dim == dimension {
            on_spawn(game, &dim, &kind, pos);
        } else {
            SPAWNS.lock().unwrap_or_else(|e| e.into_inner()).push((dim, kind, pos));
        }
    }
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
