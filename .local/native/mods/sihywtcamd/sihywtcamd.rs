// So I Heard You Want To Customize Mob Difficulty for Horizon: its mob changes, written from the
// list on its page (none of its code; where the page gives no number, one is chosen here).
use super::Game;
use super::native_explosions::NativeBlast;
use super::native_player_state::NativeEffect;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

const ARTHROPODS: [&str; 5] = ["spider", "cave_spider", "silverfish", "endermite", "bee"];
const UNDEAD: [&str; 12] = [
    "zombie", "husk", "drowned", "zombie_villager", "zombified_piglin", "skeleton", "stray",
    "wither_skeleton", "bogged", "phantom", "zoglin", "wither",
];
const ZOMBIES: [&str; 5] = ["zombie", "husk", "drowned", "zombie_villager", "zombified_piglin"];
const FIRE: [&str; 4] = ["in_fire", "on_fire", "campfire", "hot_floor"];
const BLASTS: [&str; 2] = ["explosion", "player_explosion"];

/// A hit, kept for the level's tick (the hit itself has no game to act on).
struct Hit {
    victim: u32,
    player: bool,
    attacker: u32,
    damage: String,
    amount: f32,
    /// The level ticks it waited for its attacker's level.
    age: u8,
}

static HITS: Mutex<Vec<Hit>> = Mutex::new(Vec::new());
/// Creepers a blast reached: they light up instead.
static LIT: Mutex<Vec<(u32, u8)>> = Mutex::new(Vec::new());
/// Where each creeper was at the last tick of its level, by level and id: a creeper that blew up
/// is gone when its hits are read.
static CREEPERS: Mutex<Option<HashMap<String, HashMap<u32, [f64; 3]>>>> = Mutex::new(None);

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

/// A slime's or a magma cube's `Size` (0, 1 or 3 as the world spawns them).
fn size_of(game: &mut Game<'_>, id: u32) -> Option<i32> {
    let entity = game.native_entity(id)?;
    match horizon_world::nbt::Tag::Compound(game.native_entity_nbt(entity)?).get("Size") {
        Some(horizon_world::nbt::Tag::Int(size)) => Some(*size),
        _ => None,
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn damage(entity: i32, kind: &str, damage_type: &str, causing: i32, amount: &mut f32, canceled: &mut bool) {
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
    // A creeper takes no blast: it lights up instead.
    if kind == "creeper" && BLASTS.contains(&damage) {
        *canceled = true;
        if entity > 0 {
            lock(&LIT).push((entity as u32, 0));
        }
        return;
    }
    if causing > 0 && entity > 0 && *amount > 0.0 {
        lock(&HITS).push(Hit {
            victim: entity as u32,
            player: kind == "player",
            attacker: causing as u32,
            damage: damage.to_owned(),
            amount: *amount,
            age: 0,
        });
    }
}

/// Mobs just spawned, for the level's tick: dimension, kind, where and whether the world's own
/// spawning made them.
static SPAWNS: Mutex<Vec<(String, String, [f64; 3], bool)>> = Mutex::new(Vec::new());

pub(super) fn spawned(dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    lock(&SPAWNS).push((dimension.to_owned(), kind.to_owned(), pos, natural));
}

/// The spawn-time changes.
fn on_spawn(game: &mut Game<'_>, dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    let k = path(kind);
    let monster = game.res.mob_types().get(kind).is_some_and(|t| t.category == "monster");
    if !monster && !matches!(k, "ghast" | "slime" | "magma_cube" | "hoglin" | "zombified_piglin" | "piglin") {
        return;
    }
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
    let run = |game: &mut Game<'_>, what: &str| game.native_run_command(&format!("{at} {what}"));
    let attribute = |name: &str, id: &str, by: f64, how: &str| format!("attribute @s minecraft:{name} modifier add sihywtcamd:{id} {by:.3} {how}");
    // A blaze follows from half as far. (The mod's doubled follow range for the others leaves
    // the range they notice a player from alone, which the attribute here would double too.)
    if k == "blaze" {
        run(game, &attribute("follow_range", "follow", -0.5, "add_multiplied_base"));
    }
    let health = match k {
        "ghast" => Some(20.0),
        "evoker" => Some(30.0),
        "wither" => Some(360.0),
        _ => None,
    };
    if let Some(h) = health {
        run(game, &format!("attribute @s minecraft:max_health base set {h}"));
        game.native_set_mob_health(dimension, mob.id, h as f32);
    }
    let baby = game.mobs.iter().find(|m| m.id == mob.id).is_some_and(|m| m.is_baby());
    if ZOMBIES.contains(&k) {
        run(game, &attribute("knockback_resistance", "sturdy", 0.2, "add_value"));
        run(game, &attribute("spawn_reinforcements", "calls", 0.1, "add_value"));
        let speed = (rand() - 0.5) * 0.2;
        run(game, &attribute("movement_speed", "variation", speed, "add_multiplied_base"));
        // A grown one may be a caller, a runner, a tank or an unstoppable one.
        if !baby {
            match (rand() * 10.0) as u32 {
                0 => {
                    run(game, &attribute("spawn_reinforcements", "caller", 0.4, "add_value"));
                }
                1 => {
                    run(game, &attribute("movement_speed", "runner", 0.25, "add_multiplied_base"));
                }
                2 => {
                    run(game, &attribute("max_health", "tank", 10.0, "add_value"));
                    game.native_set_mob_health(dimension, mob.id, mob.max_health + 10.0);
                }
                3 => {
                    run(game, &attribute("knockback_resistance", "unstoppable", 0.6, "add_value"));
                }
                _ => {}
            }
        }
    }
    match k {
        "vindicator" if rand() < 0.5 => {
            run(game, &attribute("movement_speed", "speed_bonus", 0.15, "add_multiplied_base"));
        }
        // The larvae are quicker.
        "silverfish" | "endermite" => {
            run(game, &attribute("movement_speed", "larva", 0.2, "add_multiplied_base"));
        }
        // Some skeletons carry spectral arrows.
        "skeleton" if rand() < 0.15 => {
            run(game, "item replace entity @s weapon.offhand with minecraft:spectral_arrow 16");
        }
        // A wither skeleton's sword is iron; a grown one may carry a bow of flame instead.
        "wither_skeleton" => {
            if !baby && rand() < 0.2 {
                run(game, "item replace entity @s weapon.mainhand with minecraft:bow[minecraft:enchantments={\"minecraft:flame\":1}]");
            } else {
                run(game, "item replace entity @s weapon.mainhand with minecraft:iron_sword");
            }
        }
        // Drowned carry tridents more often, as in Bedrock.
        "drowned" if rand() < 0.15 => {
            run(game, "item replace entity @s weapon.mainhand with minecraft:trident");
        }
        // Some zombified piglins are brutes, with an axe.
        "zombified_piglin" if !baby && rand() < 0.1 => {
            run(game, "item replace entity @s weapon.mainhand with minecraft:golden_axe");
            run(game, "tag @s add sihywtcamd.brute");
        }
        // A pillager's crossbow pierces more often.
        "pillager" if rand() < 0.3 => {
            run(game, "item replace entity @s weapon.mainhand with minecraft:crossbow[minecraft:enchantments={\"minecraft:piercing\":1}]");
        }
        // A slime the world spawned may be a size bigger.
        "slime" if natural && rand() < 0.2 => {
            if let Some(size) = size_of(game, mob.id).filter(|size| (0..3).contains(size)) {
                let bigger = if size == 0 { 1 } else { 3 };
                run(game, &format!("data merge entity @s {{Size:{bigger}}}"));
            }
        }
        _ => {}
    }
}

/// Sets the entity `id` on fire for `ticks`: a player or a mob.
fn ignite(game: &mut Game<'_>, id: u32, ticks: i32) {
    let Some(entity) = game.native_entity(id) else { return };
    match game.native_entity_target(entity) {
        Some(t @ (super::commands::Target::Player | super::commands::Target::Guest(_))) => {
            game.for_player(t, |g| {
                if let Some(p) = g.player.as_mut() {
                    let _ = p.ignite_for_ticks(ticks);
                }
            });
        }
        Some(super::commands::Target::Mob(mob)) => {
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob) {
                m.ignite_for_ticks(ticks);
            }
        }
        _ => {}
    }
}

/// Gives the player with entity id `id` an effect.
fn effect(game: &mut Game<'_>, id: u32, name: &str, ticks: i32) {
    let Some(entity) = game.native_entity(id) else { return };
    if let Some(uuid) = game.native_entity_uuid(entity) {
        game.native_add_effect(uuid, &NativeEffect { id: format!("minecraft:{name}"), amplifier: 0, duration: ticks, ambient: false, particles: true });
    }
}

/// An endermite's bite moves the player as a chorus fruit does: somewhere within 8 blocks with
/// ground under it and room to stand, tried 16 times.
fn displace(game: &mut Game<'_>, dimension: &str, id: u32) {
    let Some(entity) = game.native_entity(id) else { return };
    let (Some(uuid), Some(from), Some(rotation)) =
        (game.native_entity_uuid(entity), game.native_entity_position(entity), game.native_entity_rotation(entity))
    else {
        return;
    };
    let (low, high) = game.native_world_height();
    for _ in 0..16 {
        let x = (from[0] + (rand() - 0.5) * 16.0).floor() + 0.5;
        let z = (from[2] + (rand() - 0.5) * 16.0).floor() + 0.5;
        let top = (from[1] + (rand() * 16.0 - 8.0)).floor().clamp(f64::from(low) + 1.0, f64::from(high) - 2.0);
        let mut y = top;
        // Down to the ground, as far as the fruit's reach.
        let ground = |game: &Game<'_>, y: f64| !game.native_collision_boxes(dimension, [x - 0.3, y - 1.0, z - 0.3, x + 0.3, y - 0.01, z + 0.3]).is_empty();
        while y > f64::from(low) + 1.0 && y > top - 16.0 && !ground(game, y) {
            y -= 1.0;
        }
        if !ground(game, y) || !game.native_collision_boxes(dimension, [x - 0.3, y + 0.01, z - 0.3, x + 0.3, y + 1.8, z + 0.3]).is_empty() {
            continue;
        }
        if game.native_teleport_player(uuid, dimension, [x, y, z], rotation[0], rotation[1]) {
            log::info!("[sihywtcamd] an endermite's bite moved a player to {x} {y} {z}");
            let [fx, fy, fz] = from;
            game.native_run_command(&format!(
                "execute in {dimension} positioned {fx} {fy} {fz} run playsound minecraft:item.chorus_fruit.teleport hostile @a ~ ~ ~ 1 1"
            ));
            return;
        }
    }
    log::info!("[sihywtcamd] an endermite's bite found nowhere to move its player");
}

/// What the hits of this level's mobs do.
fn hits(game: &mut Game<'_>, dimension: &str, creepers: &HashMap<u32, [f64; 3]>) {
    let hits = std::mem::take(&mut *lock(&HITS));
    let mut blown: Vec<u32> = Vec::new();
    for mut hit in hits {
        // A creeper's blast leaves those it reached slow to dig and weak, longer the nearer.
        if BLASTS.contains(&hit.damage.as_str()) {
            if creepers.contains_key(&hit.attacker) && game.native_mob(dimension, hit.attacker).is_none() {
                if !blown.contains(&hit.attacker) {
                    blown.push(hit.attacker);
                }
            } else if hit.age < 3 {
                hit.age += 1;
                lock(&HITS).push(hit);
            }
            continue;
        }
        let Some(mob) = game.native_mob(dimension, hit.attacker) else {
            // Its attacker is in another level, or gone.
            if hit.age < 3 {
                hit.age += 1;
                lock(&HITS).push(hit);
            }
            continue;
        };
        let melee = matches!(hit.damage.as_str(), "mob_attack" | "mob_attack_no_aggro");
        let kind = path(&mob.kind);
        if !melee {
            continue;
        }
        if UNDEAD.contains(&kind) && mob.health > 0.0 {
            game.native_set_mob_health(dimension, hit.attacker, (mob.health + hit.amount).min(mob.max_health));
        }
        match kind {
            "enderman" if hit.player && rand() < 1.0 / 3.0 => effect(game, hit.victim, "blindness", 100),
            "blaze" => ignite(game, hit.victim, 100),
            "magma_cube" => {
                // Longer the bigger it is.
                let size = size_of(game, hit.attacker).unwrap_or(0);
                ignite(game, hit.victim, 40 * (size.clamp(0, 3) + 1));
            }
            "endermite" if hit.player => displace(game, dimension, hit.victim),
            _ => {}
        }
    }
    for id in blown {
        let [x, y, z] = creepers[&id];
        for (ring, seconds) in [("..3", 20), ("3..6", 12), ("6..10", 6)] {
            for name in ["mining_fatigue", "weakness"] {
                game.native_run_command(&format!(
                    "execute in {dimension} positioned {x} {y} {z} run effect give @e[distance={ring},type=!minecraft:creeper] minecraft:{name} {seconds} 0"
                ));
            }
        }
    }
}

/// A wither at half its health, once: three or four wither skeletons, a blast like the one it
/// is born with, and a storm for as long as it lives.
fn withers(game: &mut Game<'_>, dimension: &str, mobs: &[super::native_mobs::NativeMob]) {
    let mut any = false;
    for mob in mobs.iter().filter(|m| path(&m.kind) == "wither" && m.health > 0.0) {
        any = true;
        if mob.health > mob.max_health / 2.0 {
            continue;
        }
        let Some(entity) = game.native_entity(mob.id) else { continue };
        if game.native_entity_tags(entity).is_some_and(|tags| tags.iter().any(|t| t == "sihywtcamd.half")) {
            continue;
        }
        if !game.native_entity_add_tag(entity, "sihywtcamd.half") {
            continue;
        }
        let [x, y, z] = mob.pos;
        game.native_explode(dimension, [x, y + 1.5, z], 7.0, false, NativeBlast::Block);
        // Its own blast leaves it as it was.
        game.native_set_mob_health(dimension, mob.id, mob.health);
        let count = 3 + (rand() * 2.0) as u32;
        for i in 0..count {
            let a = f64::from(i) / f64::from(count) * std::f64::consts::TAU;
            game.native_spawn_mob(dimension, "minecraft:wither_skeleton", [x + a.cos() * 2.0, y, z + a.sin() * 2.0], &Default::default());
        }
    }
    if any && dimension == "minecraft:overworld" && game.world.game_time % 200 == 0 {
        game.native_run_command("weather thunder 30");
    }
}

/// A slime in the Nether's lava becomes a magma cube; a magma cube in the Overworld's water, a
/// slime.
fn convert(game: &mut Game<'_>, dimension: &str, mobs: &[super::native_mobs::NativeMob]) {
    let (from, to, lava) = match dimension {
        "minecraft:the_nether" => ("slime", "magma_cube", true),
        "minecraft:overworld" => ("magma_cube", "slime", false),
        _ => return,
    };
    for mob in mobs.iter().filter(|m| path(&m.kind) == from && m.health > 0.0) {
        let Some(entity) = game.native_entity(mob.id) else { continue };
        let inside = if lava { game.native_entity_is_in_lava(entity) } else { game.native_entity_is_in_water(entity) };
        if inside != Some(true) {
            continue;
        }
        let size = size_of(game, mob.id).unwrap_or(0);
        let [x, y, z] = mob.pos;
        if game.native_remove_mob(dimension, mob.id) {
            game.native_run_command(&format!("execute in {dimension} run summon minecraft:{to} {x} {y} {z} {{Size:{size}}}"));
        }
    }
}

pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let spawns = std::mem::take(&mut *lock(&SPAWNS));
    for (dim, kind, pos, natural) in spawns {
        if dim == dimension {
            on_spawn(game, &dim, &kind, pos, natural);
        } else {
            lock(&SPAWNS).push((dim, kind, pos, natural));
        }
    }
    let mobs = game.native_mobs(dimension);
    // Creepers a blast reached light up.
    let lit = std::mem::take(&mut *lock(&LIT));
    for (id, age) in lit {
        match game.native_entity(id).filter(|_| mobs.iter().any(|m| m.id == id)) {
            Some(entity) => {
                let mut nbt = horizon_world::nbt::Compound::default();
                nbt.insert("ignited".into(), horizon_world::nbt::Tag::Byte(1));
                game.native_entity_merge_nbt(entity, nbt);
            }
            None if age < 3 => lock(&LIT).push((id, age + 1)),
            None => {}
        }
    }
    let before = lock(&CREEPERS).get_or_insert_with(HashMap::new).remove(dimension).unwrap_or_default();
    hits(game, dimension, &before);
    let now: HashMap<u32, [f64; 3]> = mobs.iter().filter(|m| path(&m.kind) == "creeper").map(|m| (m.id, m.pos)).collect();
    lock(&CREEPERS).get_or_insert_with(HashMap::new).insert(dimension.to_owned(), now);
    withers(game, dimension, &mobs);
    if game.world.game_time % 20 == 0 {
        convert(game, dimension, &mobs);
    }
}
