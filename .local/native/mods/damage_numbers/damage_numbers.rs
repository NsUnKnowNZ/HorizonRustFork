// Damage Numbers: DamageNumbersImpl.onEntityHealthChange and TextParticle
// (a vanilla Particle: random start velocity, gravity 0.75, friction 0.99,
// 32 ticks), drawn as camera-facing text through the world primitives.
use super::*;
use horizon_client::world_primitives::WorldPrimitives;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const COLOR_SM: u32 = 0xFFFF_AA00;
const COLOR_MD: u32 = 0xFFFF_0000;
const COLOR_LG: u32 = 0xFFAA_0000;
const MAX_AGE: u32 = 32;
const LIMIT: usize = 256;
const TICK: Duration = Duration::from_millis(50);

struct Number {
    pos: [f64; 3],
    old: [f64; 3],
    vel: [f64; 3],
    age: u32,
    text: String,
    color: u32,
}

struct State {
    health: HashMap<u32, f32>,
    numbers: Vec<Number>,
    clock: Option<Instant>,
    seed: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

impl State {
    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Particle.tick.
    fn tick(&mut self) {
        self.numbers.retain_mut(|n| {
            n.old = n.pos;
            n.age += 1;
            if n.age >= MAX_AGE {
                return false;
            }
            n.vel[1] -= 0.04 * 0.75;
            for k in 0..3 {
                n.pos[k] += n.vel[k];
                n.vel[k] *= 0.99;
            }
            true
        });
    }
}

/// Color.lerp, channel by channel.
fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let mut out = 0;
    for shift in [24, 16, 8, 0] {
        let (x, y) = ((a >> shift & 255) as f32, (b >> shift & 255) as f32);
        out |= ((x + (y - x) * t).round().clamp(0.0, 255.0) as u32) << shift;
    }
    out
}

fn color(damage: f32) -> u32 {
    if damage >= 16.0 {
        COLOR_LG
    } else if damage >= 8.0 {
        lerp(COLOR_MD, COLOR_LG, (damage - 8.0) / 8.0)
    } else if damage >= 2.0 {
        lerp(COLOR_SM, COLOR_MD, (damage - 2.0) / 6.0)
    } else {
        COLOR_SM
    }
}

/// String.format("%.1f") without a trailing ".0".
fn label(damage: f32) -> String {
    let text = format!("{damage:.1}");
    text.strip_suffix(".0").map(str::to_owned).unwrap_or(text)
}

pub(super) fn frame(game: &Game<'_>, partial: f32, camera: [f64; 3], primitives: &mut WorldPrimitives) {
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let s = guard.get_or_insert_with(|| State {
        health: HashMap::new(),
        numbers: Vec::new(),
        clock: None,
        seed: 0x2545_F491_4F6C_DD1D,
    });
    // The particles' ticks, at 20 a second.
    let now = Instant::now();
    let mut clock = s.clock.unwrap_or(now);
    let mut steps = 0;
    while now.duration_since(clock) >= TICK && steps < 20 {
        clock += TICK;
        steps += 1;
    }
    if steps == 20 {
        clock = now;
    }
    s.clock = Some(clock);
    for _ in 0..steps {
        s.tick();
    }
    // MixinLivingEntity.tick: each living entity's health against the last seen.
    let mut seen = HashMap::with_capacity(s.health.len());
    for m in game.mobs.iter() {
        let new = m.health;
        seen.insert(m.id, new);
        let Some(&old) = s.health.get(&m.id) else {
            continue;
        };
        let damage = old - new;
        if damage <= 0.0 {
            continue;
        }
        let p = [m.position.x, m.position.y, m.position.z];
        let d2: f64 = (0..3).map(|k| (p[k] - camera[k]).powi(2)).sum();
        if d2 > 2304.0 {
            continue;
        }
        while s.numbers.len() >= LIMIT {
            s.numbers.remove(0);
        }
        // The velocity: the entity's, back toward the camera by ten widths, 20 up;
        // then Particle's random spread and speed.
        let mut forward = [p[0] - camera[0], p[1] - camera[1], p[2] - camera[2]];
        let len = forward.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
        for v in &mut forward {
            *v = *v / len * m.kind.width * 10.0;
        }
        let base = [m.velocity.x - forward[0], m.velocity.y + 20.0, m.velocity.z - forward[2]];
        let mut vel = [0.0; 3];
        for k in 0..3 {
            vel[k] = base[k] + (s.random() * 2.0 - 1.0) * 0.4;
        }
        let speed = (s.random() + s.random() + 1.0) * 0.15;
        let norm = vel.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
        for v in &mut vel {
            *v = *v / norm * speed * 0.4;
        }
        vel[1] += 0.1;
        let pos = [p[0], p[1] + m.kind.height + 0.25, p[2]];
        s.numbers.push(Number {
            pos,
            old: pos,
            vel,
            age: 0,
            text: label(damage),
            color: color(damage),
        });
    }
    s.health = seen;
    let t = f64::from(partial);
    for n in &s.numbers {
        let at = std::array::from_fn(|k| n.old[k] + (n.pos[k] - n.old[k]) * t);
        primitives.text(at, n.text.clone(), n.color, 1.0);
    }
}
