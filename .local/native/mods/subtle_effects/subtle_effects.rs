// Subtle Effects for Horizon: sparks, block dust, running and landing dust, heartbeat and stomach.
use super::Game;
use horizon_client::particles::ParticleOptions;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Blocks sampled round the player each tick, and how far.
const SAMPLES: usize = 48;
const REACH: i32 = 10;

static SEED: AtomicU64 = AtomicU64::new(0xD1B5_4A32_D192_ED03);

fn rand() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn between(lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * rand()
}

/// What the player was doing last tick: on the ground, the highest point of the fall, and the
/// ticks to the next heartbeat and growl.
struct Last {
    on_ground: bool,
    fall_top: f64,
    heart: u32,
    growl: u32,
}

static LAST: Mutex<Last> = Mutex::new(Last { on_ground: true, fall_top: f64::MIN, heart: 0, growl: 400 });

enum Glow {
    Spark,
    Dust([f32; 3]),
}

fn glow(game: &Game<'_>, state: horizon_world::registry::BlockStateId) -> Option<Glow> {
    let name = game.native_state_block(state)?;
    let lit = || game.native_block_state_property(state, "lit").is_none_or(|v| v == "true");
    match name {
        "minecraft:fire" | "minecraft:torch" | "minecraft:wall_torch" => Some(Glow::Spark),
        "minecraft:campfire" if lit() => Some(Glow::Spark),
        n if (n.ends_with("candle") || n == "minecraft:candle") && n.starts_with("minecraft:") && lit() => {
            (rand() < 0.3).then_some(Glow::Spark)
        }
        "minecraft:glowstone" => Some(Glow::Dust([1.0, 0.86, 0.45])),
        "minecraft:redstone_block" => Some(Glow::Dust([0.9, 0.1, 0.05])),
        _ => None,
    }
}

pub(super) fn tick(game: &mut Game<'_>) {
    if game.title.is_some() {
        return;
    }
    let Some((at, velocity, on_ground, health, food)) = game.player.as_ref().map(|p| {
        (
            [p.position.x, p.position.y, p.position.z],
            [p.velocity.x, p.velocity.y, p.velocity.z],
            p.ground.on_ground,
            p.health.health(),
            p.health.food.food,
        )
    }) else {
        return;
    };
    let mut planned: Vec<(ParticleOptions, [f64; 3], [f64; 3])> = Vec::new();
    // Sparks and dust from the blocks round the player.
    let (px, py, pz) = (at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
    for _ in 0..SAMPLES {
        let x = px + (between(-f64::from(REACH), f64::from(REACH) + 1.0)).floor() as i32;
        let y = py + (between(-4.0, 6.0)).floor() as i32;
        let z = pz + (between(-f64::from(REACH), f64::from(REACH) + 1.0)).floor() as i32;
        if !game.world.loaded(x, z) {
            continue;
        }
        let state = game.world.block(x, y, z);
        match glow(game, state) {
            Some(Glow::Spark) => {
                let top = if game.native_state_block(state).is_some_and(|n| n.contains("torch")) { 0.7 } else { 0.5 };
                let p = [f64::from(x) + between(0.3, 0.7), f64::from(y) + top, f64::from(z) + between(0.3, 0.7)];
                let v = [between(-0.03, 0.03), between(0.04, 0.1), between(-0.03, 0.03)];
                planned.push((ParticleOptions::dust([1.0, between(0.45, 0.7) as f32, 0.15], 0.35), p, v));
            }
            Some(Glow::Dust(rgb)) => {
                // Just outside one of its faces.
                let face = (rand() * 6.0) as usize;
                let mut p = [f64::from(x) + rand(), f64::from(y) + rand(), f64::from(z) + rand()];
                let axis = face / 2;
                p[axis] = [f64::from(x), f64::from(y), f64::from(z)][axis] + if face % 2 == 0 { -0.05 } else { 1.05 };
                planned.push((ParticleOptions::dust(rgb, 0.5), p, [0.0, 0.01, 0.0]));
            }
            None => {}
        }
    }
    // Dust at the feet: running, and landing from a fall.
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let feet_tone = [0.55, 0.5, 0.45];
    let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
    if on_ground && speed > 0.22 && rand() < 0.5 {
        let p = [at[0] - velocity[0] * 2.0 + between(-0.2, 0.2), at[1] + 0.1, at[2] - velocity[2] * 2.0 + between(-0.2, 0.2)];
        planned.push((ParticleOptions::dust(feet_tone, 1.2), p, [0.0, 0.02, 0.0]));
    }
    if on_ground {
        if !last.on_ground && last.fall_top - at[1] > 2.5 {
            let n = ((last.fall_top - at[1]) * 2.0).min(24.0) as usize;
            for i in 0..n {
                let a = i as f64 / n as f64 * std::f64::consts::TAU;
                let p = [at[0] + a.cos() * 0.4, at[1] + 0.1, at[2] + a.sin() * 0.4];
                planned.push((ParticleOptions::dust(feet_tone, 1.4), p, [a.cos() * 0.1, 0.02, a.sin() * 0.1]));
            }
        }
        last.fall_top = at[1];
    } else {
        last.fall_top = last.fall_top.max(at[1]);
    }
    last.on_ground = on_ground;
    // Heartbeat and stomach.
    let mut sounds: Vec<(&str, f32, f32, i32)> = Vec::new();
    if health > 0.0 && health <= 6.0 {
        if last.heart == 0 {
            sounds.push(("minecraft:block.note_block.basedrum", 0.7, 0.5, 0));
            sounds.push(("minecraft:block.note_block.basedrum", 0.5, 0.45, 5));
            // From every 30 ticks at three hearts to every 15 at half a heart.
            last.heart = 15 + (health * 2.5) as u32;
        } else {
            last.heart -= 1;
        }
    } else {
        last.heart = 0;
    }
    if health > 0.0 && food <= 6 {
        if last.growl == 0 {
            sounds.push(("minecraft:entity.ravager.ambient", 0.18, 1.9, 0));
            last.growl = 600 + (rand() * 900.0) as u32;
        } else {
            last.growl -= 1;
        }
    }
    drop(last);
    for (options, p, v) in planned {
        game.native_add_particle(&options, p, v);
    }
    for (sound, volume, pitch, delay) in sounds {
        game.native_play_local_sound(sound, at, volume, pitch, delay);
    }
}
