// Effectual for Horizon: breath steam and bubbles, dripping, wither smoke and cave dust.
use super::Game;
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_core::position::BlockPos;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static SEED: AtomicU64 = AtomicU64::new(0x2545_F491_4F6C_DD1D);

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

/// Ticks to the next breath, and the ticks left dripping.
struct State {
    breath: u32,
    drip: u32,
    was_wet: bool,
}

static STATE: Mutex<State> = Mutex::new(State { breath: 40, drip: 0, was_wet: false });

pub(super) fn tick(game: &mut Game<'_>) {
    if game.title.is_some() {
        return;
    }
    let Some((at, velocity)) = game
        .player
        .as_ref()
        .map(|p| ([p.position.x, p.position.y, p.position.z], [p.velocity.x, p.velocity.y, p.velocity.z]))
    else {
        return;
    };
    let eye = at[1] + 1.62;
    let yaw = f64::from(game.yaw).to_radians();
    let (fx, fz) = (-yaw.sin(), yaw.cos());
    // In first person the mouth is just under the camera: the breath starts a little ahead and
    // lower, so it never fills the view.
    let first = game.camera_type.first_person();
    let (ahead, below) = if first { (0.8, 0.4) } else { (0.35, 0.12) };
    let mouth = [at[0] + fx * ahead, eye - below, at[2] + fz * ahead];
    let underwater = game.eye_in_water(at[0], eye, at[2]);
    let in_water = underwater || game.eye_in_water(at[0], at[1] + 0.3, at[2]);
    let (bx, by, bz) = (at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
    let loaded = game.world.loaded(bx, bz);
    let cold = loaded
        && game.world.chunks.get(&(bx >> 4, bz >> 4)).is_some_and(|c| {
            world_biomes!(game).get(c.biome(bx, by, bz)).temperature_at(BlockPos::new(bx, by, bz), 63) < 0.15
        });
    let sky = if loaded { game.world.light(bx, by + 1, bz) >> 4 } else { 15 };
    let wither = game.potions.effects.get("wither").is_some();
    let running = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt() > 0.2;

    let mut planned: Vec<(ParticleOptions, [f64; 3], [f64; 3])> = Vec::new();
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    // Breath: steam in the cold, bubbles under water.
    if state.breath == 0 {
        if underwater {
            for _ in 0..(2 + (rand() * 3.0) as usize) {
                let p = [mouth[0] + between(-0.05, 0.05), mouth[1], mouth[2] + between(-0.05, 0.05)];
                planned.push((ParticleOptions::simple(ParticleType::Bubble), p, [0.0, 0.1, 0.0]));
            }
            state.breath = 50 + (rand() * 40.0) as u32;
        } else if cold && !in_water {
            for _ in 0..5 {
                let v = [fx * 0.03 + velocity[0] + between(-0.005, 0.005), 0.01, fz * 0.03 + velocity[2] + between(-0.005, 0.005)];
                let p = [mouth[0] + between(-0.04, 0.04), mouth[1] + between(-0.04, 0.04), mouth[2] + between(-0.04, 0.04)];
                planned.push((ParticleOptions::dust([0.92, 0.94, 0.97], between(0.5, 0.9) as f32), p, v));
            }
            state.breath = if running { 25 } else { 50 } + (rand() * 20.0) as u32;
        } else {
            state.breath = 20;
        }
    } else {
        state.breath -= 1;
    }
    // Dripping for three seconds after the water.
    if state.was_wet && !in_water {
        state.drip = 60;
    }
    state.was_wet = in_water;
    if state.drip > 0 && !in_water {
        state.drip -= 1;
        if rand() < f64::from(state.drip) / 60.0 * 0.6 {
            let p = [at[0] + between(-0.3, 0.3), at[1] + between(0.2, 1.6), at[2] + between(-0.3, 0.3)];
            planned.push((ParticleOptions::simple(ParticleType::FallingWater), p, [0.0; 3]));
        }
    }
    drop(state);
    // Wither smoke.
    if wither && rand() < 0.5 {
        // Round the body, and below the eyes in first person.
        let a = rand() * std::f64::consts::TAU;
        let r = if first { between(0.45, 0.6) } else { between(0.0, 0.35) };
        let top = if first { 1.1 } else { 1.8 };
        let p = [at[0] + a.cos() * r, at[1] + between(0.1, top), at[2] + a.sin() * r];
        planned.push((ParticleOptions::simple(ParticleType::Smoke), p, [0.0, 0.02, 0.0]));
    }
    // Cave dust: dark, below the surface, out of the sky.
    if loaded && sky == 0 && at[1] < 56.0 && !in_water {
        for _ in 0..2 {
            let p = [at[0] + between(-8.0, 8.0), at[1] + between(-1.0, 4.0), at[2] + between(-8.0, 8.0)];
            if game.world.loaded(p[0].floor() as i32, p[2].floor() as i32)
                && game.world.light(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32) >> 4 == 0
            {
                planned.push((ParticleOptions::simple(ParticleType::WhiteAsh), p, [0.0; 3]));
            }
        }
    }
    for (options, p, v) in planned {
        game.native_add_particle(&options, p, v);
    }
}
