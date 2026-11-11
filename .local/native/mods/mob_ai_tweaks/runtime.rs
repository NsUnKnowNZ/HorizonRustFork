// Mob AI Tweaks, three of the pack's settings. Each looks at the damage a
// mob took since the last tick (`last_damage`, stamped with its age).
use std::collections::HashSet;
use std::sync::Mutex;

const PANIC_RADIUS: f64 = 16.0;
const INK_RADIUS: i32 = 8;
const INK_SECONDS: i32 = 5;

/// The hurts already handled: (mob, age stamp).
static SEEN: Mutex<Option<HashSet<(u32, i64)>>> = Mutex::new(None);

pub(super) fn tick(game: &mut super::Game<'_>) {
    if game.remote.is_some() || game.mobs.is_empty() {
        return;
    }
    let Ok(mut seen) = SEEN.lock() else {
        return;
    };
    let seen = seen.get_or_insert_with(HashSet::new);
    // Fresh hurts: this tick's or the last one's.
    let fresh: Vec<(usize, horizon_client::mobs::Damage, i64)> = game
        .mobs
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let (d, t) = m.last_damage?;
            (i64::from(m.age) - t <= 1 && !seen.contains(&(m.id, t))).then_some((i, d, t))
        })
        .collect();
    if fresh.is_empty() {
        if seen.len() > 4096 {
            seen.clear();
        }
        return;
    }
    let mut commands = Vec::new();
    for (i, damage, t) in fresh {
        let (id, kind, pos, creature) = {
            let m = &game.mobs[i];
            (m.id, m.kind.id.clone(), m.position, m.category() == "creature")
        };
        seen.insert((id, t));
        // animals_panic_in_groups: the same kind of animal nearby panics too
        // (a panic with no one to blame, so nobody gets angry).
        if creature && damage.causes_panic() {
            let calm = horizon_client::mobs::Damage { attacker: None, ..damage };
            for o in game.mobs.iter_mut() {
                if o.id == id || o.kind.id != kind || !o.alive() {
                    continue;
                }
                let (dx, dy, dz) = (o.position.x - pos.x, o.position.y - pos.y, o.position.z - pos.z);
                if dx * dx + dy * dy + dz * dz > PANIC_RADIUS * PANIC_RADIUS {
                    continue;
                }
                let age = i64::from(o.age);
                if o.last_damage.is_some_and(|(_, t)| age - t <= 40) {
                    continue;
                }
                o.last_damage = Some((calm, age));
                seen.insert((o.id, age));
            }
        }
        // creepers_delay_explosion_when_hit: a hit puts a lit fuse back.
        if &*kind == "minecraft:creeper" && damage.attacker.is_some() {
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.swell = (m.swell - 10).max(0);
                m.previous_swell = m.swell;
            }
        }
        // squid_ink_inflicts_debuffs: a hurt squid's ink blinds what is near.
        if matches!(&*kind, "minecraft:squid" | "minecraft:glow_squid") {
            let near = |p: [f64; 3]| {
                let (dx, dy, dz) = (p[0] - pos.x, p[1] - pos.y, p[2] - pos.z);
                dx * dx + dy * dy + dz * dz <= f64::from(INK_RADIUS * INK_RADIUS)
            };
            // Players by name (the console's selectors don't reach a guest).
            for p in game.native_server_players() {
                if near(p.position) && p.health > 0.0 {
                    commands.push(format!(
                        "effect give {} minecraft:blindness {INK_SECONDS} 0",
                        p.name
                    ));
                }
            }
            for o in game.mobs.iter_mut() {
                if o.id != id
                    && !matches!(&*o.kind.id, "minecraft:squid" | "minecraft:glow_squid")
                    && near([o.position.x, o.position.y, o.position.z])
                {
                    // A glow squid's ink makes them glow instead.
                    let effect = if &*kind == "minecraft:glow_squid" { "minecraft:glowing" } else { "minecraft:blindness" };
                    o.add_effect_with(effect, INK_SECONDS * 20, 0, false, true);
                }
            }
        }
    }
    for c in commands {
        let r = game.native_run_command_captured(&c);
        if !r.ran {
            log::warn!("[mob_ai_tweaks] `{c}`: {:?}", r.output);
        }
    }
}

// ---- more of its changes (0.2): what a hit does, bosses, pets, small habits ----
//
// From the list of changes on the mod's page (none of its code). The reworked attacks of its
// bosses and mobs are not here; these are the changes that are rules, not new behaviour.
use horizon_client::mod_api::EquipmentSlot;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

const BOSSES: [(&str, &str); 4] =
    [("wither", "The Wither"), ("ender_dragon", "The Ender Dragon"), ("elder_guardian", "The Elder Guardian"), ("warden", "The Warden")];
const PROJECTILES: [&str; 4] = ["arrow", "trident", "thrown", "mob_projectile"];

/// What each player holds, by entity id, and the withers below half their health: what the
/// hit's hook reads (it has no game).
static HELD: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
static LOW_WITHERS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// Hits to act on at the level's tick: victim, its kind, attacker, damage type.
static STRUCK: Mutex<Vec<(u32, String, u32, String)>> = Mutex::new(Vec::new());
/// Commands a hook inside a player's turn leaves for the tick.
static LATER: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Mobs just spawned: dimension, kind, where.
static NEW: Mutex<Vec<(String, String, [f64; 3], bool)>> = Mutex::new(Vec::new());
/// The boss bars shown, by mob id: the bar's name.
static BARS: Mutex<Option<HashMap<u32, (String, String)>>> = Mutex::new(None);

static SEED: AtomicU64 = AtomicU64::new(0x2545_F491_4F6C_DD1D);

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

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A hit about to land: vexes and phantoms take three times a projectile's damage, iron golems
/// and shulkers more from a pickaxe, silverfish more from a shovel, a wither below half its
/// health half an arrow's, an elder guardian none of a blast.
pub(super) fn damage(entity: i32, kind: &str, damage_type: &str, causing: i32, amount: &mut f32, canceled: &mut bool) {
    if *canceled || entity <= 0 {
        return;
    }
    let (kind, damage) = (path(kind), path(damage_type));
    if kind == "elder_guardian" && matches!(damage, "explosion" | "player_explosion") {
        *canceled = true;
        return;
    }
    if matches!(kind, "vex" | "phantom") && PROJECTILES.contains(&damage) {
        *amount = (*amount * 3.0).max(3.0);
    }
    if kind == "wither" && damage == "arrow" && lock(&LOW_WITHERS).contains(&(entity as u32)) {
        *amount *= 0.5;
    }
    if causing > 0 && damage == "player_attack" {
        let held = lock(&HELD).as_ref().and_then(|h| h.get(&(causing as u32)).cloned()).unwrap_or_default();
        if (matches!(kind, "iron_golem" | "shulker") && held.ends_with("_pickaxe")) || (kind == "silverfish" && held.ends_with("_shovel")) {
            *amount *= 1.5;
        }
    }
    if causing > 0 && (kind == "wandering_trader" || kind == "player") {
        lock(&STRUCK).push((entity as u32, kind.to_owned(), causing as u32, damage.to_owned()));
    }
}

pub(super) fn spawned(dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    let k = path(kind);
    if BOSSES.iter().any(|b| b.0 == k) || matches!(k, "snow_golem" | "zombie") {
        lock(&NEW).push((dimension.to_owned(), kind.to_owned(), pos, natural));
    }
}

/// A mob about to die: a boss is announced, a wither leaves far more experience, a skeleton the
/// wither effect killed rises as a wither skeleton.
pub(super) fn died(game: &mut super::Game<'_>, entity: u32, kind: &str, damage_type: &str) {
    let k = path(kind);
    let Some(pos) = game.native_entity(entity).and_then(|e| game.native_entity_position(e)) else { return };
    let dimension = game.dim.id();
    let [x, y, z] = pos;
    if let Some((_, name)) = BOSSES.iter().find(|b| b.0 == k) {
        lock(&LATER).push(format!("tellraw @a {{\"text\":\"{name} has been defeated!\",\"color\":\"gold\"}}"));
        if k == "wither" {
            for _ in 0..10 {
                lock(&LATER).push(format!("execute in {dimension} run summon minecraft:experience_orb {x} {y} {z} {{Value:95s}}"));
            }
        }
    }
    if k == "skeleton" && path(damage_type) == "wither" {
        lock(&LATER).push(format!("execute in {dimension} run summon minecraft:wither_skeleton {x} {y} {z}"));
    }
}

fn on_new(game: &mut super::Game<'_>, dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    let k = path(kind);
    let [x, y, z] = pos;
    let at = format!("execute in {dimension} positioned {x} {y} {z} as @e[type={kind},limit=1,sort=nearest,distance=..2] run");
    if let Some((_, name)) = BOSSES.iter().find(|b| b.0 == k) {
        // The dragon and the monuments' guardians come with the world: only what is summoned
        // in play is announced.
        if !natural && k != "elder_guardian" {
            game.native_run_command(&format!("tellraw @a {{\"text\":\"{name} has been summoned!\",\"color\":\"dark_purple\"}}"));
        }
    }
    match k {
        // A true boss: 150 health and 8 more armour.
        "elder_guardian" => {
            game.native_run_command(&format!("{at} attribute @s minecraft:max_health base set 150"));
            game.native_run_command(&format!("{at} attribute @s minecraft:armor modifier add mob_ai_tweaks:boss 8 add_value"));
            game.native_run_command(&format!("{at} data merge entity @s {{Health:150f}}"));
        }
        // With its pumpkin on, 10 armour.
        "snow_golem" => {
            game.native_run_command(&format!("{at} attribute @s minecraft:armor modifier add mob_ai_tweaks:pumpkin 10 add_value"));
        }
        // Some zombies come with a baby zombie on their back.
        "zombie" if natural && rand() < 0.03 => {
            let grown = game.mobs.iter().any(|m| {
                &*m.kind.id == "minecraft:zombie" && !m.is_baby() && (m.position.x - x).abs() < 0.5 && (m.position.z - z).abs() < 0.5
            });
            if grown {
                game.native_run_command(&format!(
                    "execute in {dimension} run summon minecraft:zombie {x} {y} {z} {{IsBaby:1b,Tags:[\"mob_ai_tweaks.rider\"]}}"
                ));
                game.native_run_command(&format!(
                    "execute in {dimension} positioned {x} {y} {z} run ride @e[tag=mob_ai_tweaks.rider,limit=1,sort=nearest,distance=..2] mount @e[type=minecraft:zombie,tag=!mob_ai_tweaks.rider,limit=1,sort=nearest,distance=..2]"
                ));
                game.native_run_command("tag @e[tag=mob_ai_tweaks.rider] remove mob_ai_tweaks.rider");
            }
        }
        _ => {}
    }
}

/// The boss bars of elder guardians and ravagers: shown to the players within 48 blocks, gone
/// with the mob.
fn bars(game: &mut super::Game<'_>, dimension: &str) {
    let now: Vec<(u32, &'static str, [f64; 3], f32, f32)> = game
        .mobs
        .iter()
        .filter(|m| m.alive())
        .filter_map(|m| {
            let name = match &*m.kind.id {
                "minecraft:elder_guardian" => "Elder Guardian",
                "minecraft:ravager" => "Ravager",
                _ => return None,
            };
            Some((m.id, name, [m.position.x, m.position.y, m.position.z], m.health as f32, m.attrs.max_health as f32))
        })
        .collect();
    let mut bars = lock(&BARS).take().unwrap_or_default();
    for (id, name, [x, y, z], health, max) in &now {
        let bar = format!("mob_ai_tweaks:b{id}");
        if !bars.contains_key(id) {
            let colour = if *name == "Ravager" { "red" } else { "blue" };
            game.native_run_command(&format!("bossbar add {bar} \"{name}\""));
            game.native_run_command(&format!("bossbar set {bar} color {colour}"));
            bars.insert(*id, (bar.clone(), dimension.to_owned()));
        }
        game.native_run_command(&format!("bossbar set {bar} max {}", max.ceil().max(1.0) as i32));
        game.native_run_command(&format!("bossbar set {bar} value {}", health.ceil().max(0.0) as i32));
        game.native_run_command(&format!("execute in {dimension} positioned {x} {y} {z} run bossbar set {bar} players @a[distance=..48]"));
    }
    bars.retain(|id, (bar, dim)| {
        if dim != dimension || now.iter().any(|n| n.0 == *id) {
            return true;
        }
        game.native_run_command(&format!("bossbar remove {bar}"));
        false
    });
    *lock(&BARS) = Some(bars);
}

/// Once a tick for each level.
pub(super) fn level(game: &mut super::Game<'_>, dimension: &str) {
    for line in std::mem::take(&mut *lock(&LATER)) {
        game.native_run_command(&line);
    }
    let new = std::mem::take(&mut *lock(&NEW));
    for (dim, kind, pos, natural) in new {
        if dim == dimension {
            on_new(game, &dim, &kind, pos, natural);
        } else {
            lock(&NEW).push((dim, kind, pos, natural));
        }
    }
    // What the hit's hook reads.
    let players = game.native_server_players();
    let mut held = HashMap::new();
    for p in &players {
        let item = game
            .native_entity(p.entity_id as u32)
            .and_then(|e| game.native_living(e))
            .and_then(|l| game.native_living_equipment(l, EquipmentSlot::MainHand))
            .map(|s| s.item.to_string())
            .unwrap_or_default();
        held.insert(p.entity_id as u32, item);
    }
    *lock(&HELD) = Some(held);
    let low: Vec<u32> = game
        .mobs
        .iter()
        .filter(|m| &*m.kind.id == "minecraft:wither" && (m.health as f32) < (m.attrs.max_health as f32) / 2.0)
        .map(|m| m.id)
        .collect();
    {
        let mut all = lock(&LOW_WITHERS);
        all.retain(|id| !game.mobs.iter().any(|m| m.id == *id));
        all.extend(low);
        all.truncate(64);
    }
    // A wandering trader blinds who strikes it; the dragon's blow sets on fire.
    for (victim, kind, attacker, damage) in std::mem::take(&mut *lock(&STRUCK)) {
        if kind == "wandering_trader" {
            if let Some(p) = players.iter().find(|p| p.entity_id as u32 == attacker) {
                game.native_run_command(&format!("effect give {} minecraft:blindness 8 0", p.name));
            }
        } else if damage == "mob_attack"
            && game.native_entity(attacker).and_then(|e| game.native_entity_kind(e)).is_some_and(|k| path(&k) == "ender_dragon")
            && let Some(t) = game.native_entity(victim).and_then(|e| game.native_entity_target(e))
        {
            game.for_player(t, |g| {
                if let Some(p) = g.player.as_mut() {
                    let _ = p.ignite_for_ticks(100);
                }
            });
        }
    }
    let time = game.world.game_time;
    if time % 10 == 0 {
        bars(game, dimension);
    }
    // Pets mend slowly, villagers too (they eat); a chicken sheds a feather now and then.
    let mut feathers = Vec::new();
    for m in game.mobs.iter_mut().filter(|m| m.alive()) {
        let id = &*m.kind.id;
        let max = m.attrs.max_health;
        if time % 100 == 0 && m.health < max && (m.extra.owner.is_some() || m.extra.tame) {
            m.health = (m.health + 1.0).min(max);
        }
        if time % 200 == 0 && m.health < max && id == "minecraft:villager" {
            m.health = (m.health + 1.0).min(max);
        }
        if id == "minecraft:chicken" && !m.is_baby() && rand() < 1.0 / 12000.0 {
            feathers.push([m.position.x, m.position.y, m.position.z]);
        }
    }
    for [x, y, z] in feathers {
        game.native_run_command(&format!("execute in {dimension} run summon minecraft:item {x} {y} {z} {{Item:{{id:\"minecraft:feather\",count:1}}}}"));
    }
}
