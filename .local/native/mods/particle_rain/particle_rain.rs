// Particle Rain for Horizon: rain drops, snowflakes and blowing sand as particles around the player.
use super::{Dim, Game};
use horizon_client::particles::{ParticleOptions, ParticleType};
use std::sync::atomic::{AtomicU64, Ordering};

/// How far round the player the weather is drawn, and the most particles a tick at full weather.
const RADIUS: f64 = 14.0;
const RAIN_DROPS: f32 = 10.0;
const SNOWFLAKES: f32 = 8.0;
const SAND: f32 = 4.0;

static SEED: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

/// A number in 0..1 (xorshift: looks only, no game randomness used).
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

/// How many of `per_tick` (a fraction carries over by chance).
fn count(per_tick: f32) -> usize {
    let whole = per_tick.floor();
    whole as usize + usize::from(rand() < f64::from(per_tick - whole))
}

enum Weather {
    Rain,
    Snow,
    Sand,
}

pub(super) fn tick(game: &mut Game<'_>) {
    if game.dim != Dim::Overworld || game.title.is_some() {
        return;
    }
    let Some(at) = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]) else {
        return;
    };
    let (rain, thunder) = game.view_weather(0.0);
    if rain < 0.05 {
        return;
    }
    let wind = 0.04 + 0.08 * f64::from(thunder);
    let total = rain.max(thunder);
    // Plan the particles first (the world and biomes are borrowed), then add them.
    let mut planned: Vec<(ParticleOptions, [f64; 3], [f64; 3])> = Vec::new();
    {
        let world = &game.world;
        let biomes = world_biomes!(game);
        let properties = &game.pack.assets.properties;
        // Sand blows only while the player stands in a desert or badlands.
        let sandy_here = world.chunks.get(&((at[0].floor() as i32) >> 4, (at[2].floor() as i32) >> 4)).is_some_and(|c| {
            let name = &biomes.get(c.biome(at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32)).name;
            name.contains("desert") || name.contains("badlands")
        });
        let tries = count(RAIN_DROPS * total).max(count(SNOWFLAKES * total)).max(count(SAND * total));
        for _ in 0..tries {
            let x = at[0] + between(-RADIUS, RADIUS);
            let z = at[2] + between(-RADIUS, RADIUS);
            let (bx, bz) = (x.floor() as i32, z.floor() as i32);
            let Some(chunk) = world.chunks.get(&(bx >> 4, bz >> 4)) else { continue };
            let ground = super::render_tick::motion_blocking_height(world, properties, bx, bz);
            let y = at[1] + between(2.0, 12.0);
            let biome = biomes.get(chunk.biome(bx, at[1].floor() as i32, bz));
            let weather = match biome.snows_at(bx, y.floor() as i32, bz) {
                Some(false) => Weather::Rain,
                Some(true) => Weather::Snow,
                None if sandy_here && (biome.name.contains("desert") || biome.name.contains("badlands")) => Weather::Sand,
                None => continue,
            };
            match weather {
                Weather::Rain if f64::from(ground) <= y && rand() < f64::from(RAIN_DROPS * total) / tries as f64 => {
                    planned.push((ParticleOptions::simple(ParticleType::FallingWater), [x, y, z], [0.0; 3]));
                }
                Weather::Snow if f64::from(ground) <= y && rand() < f64::from(SNOWFLAKES * total) / tries as f64 => {
                    let v = [wind + between(-0.02, 0.02), between(-0.12, -0.06), wind * 0.5 + between(-0.02, 0.02)];
                    planned.push((ParticleOptions::simple(ParticleType::Snowflake), [x, y, z], v));
                }
                Weather::Sand if rand() < f64::from(SAND * total) / tries as f64 => {
                    // Low over the ground, blown along.
                    let y = f64::from(ground) + between(0.1, 2.5);
                    if (y - at[1]).abs() > 10.0 {
                        continue;
                    }
                    let tone = if biome.name.contains("badlands") { [0.74, 0.42, 0.22] } else { [0.86, 0.78, 0.56] };
                    let v = [wind * 4.0, between(-0.01, 0.02), wind * 2.0];
                    planned.push((ParticleOptions::dust(tone, between(0.4, 0.9) as f32), [x, y, z], v));
                }
                _ => {}
            }
        }
    }
    for (options, at, v) in planned {
        game.native_add_particle(&options, at, v);
    }
}
