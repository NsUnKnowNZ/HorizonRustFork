// Fancy Health Bar, the vanilla style with Better OneBlock's settings:
// VanillaHealthBarRendering.onDamage spawns a PixelEntity for every pixel of
// each lost half heart; HudEntity.tick moves them (velocity x -40..40,
// y -20..20, x slowed by 0.99, y falling by 1 a tick) for 100 ticks.
use super::*;
use horizon_client::hud_layout::{Blend, GuiDraw, HealthInputs, HeartKind};
use std::borrow::Cow;
use std::sync::Mutex;

struct Pixel {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    ax: f64,
    ay: f64,
    u: i32,
    v: i32,
    sprite: &'static str,
    life: u32,
}

struct State {
    last_health: Option<i32>,
    last_tick: Option<i32>,
    pixels: Vec<Pixel>,
    seed: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    last_health: None,
    last_tick: None,
    pixels: Vec::new(),
    seed: 0x9E37_79B9_7F4A_7C15,
});

const MAX_LIFETIME: u32 = 100;

impl State {
    /// Math.random.
    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }
    /// IntRange.random: floor(lerp(min, max, random)).
    fn int(&mut self, min: i32, max: i32) -> f64 {
        (f64::from(min) + self.random() * f64::from(max - min)).floor()
    }
    /// VanillaHealthBarRendering.spawnHeart: columns `us..ul` of the heart at `at`.
    fn spawn(&mut self, us: i32, ul: i32, at: [i32; 2], sprite: &'static str) {
        for u in us..ul {
            for v in 0..9 {
                let vx = self.int(-40, 40);
                let vy = self.int(-20, 20);
                self.pixels.push(Pixel {
                    x: f64::from(at[0] + u) - 0.5,
                    y: f64::from(at[1] + v) - 0.5,
                    vx,
                    vy,
                    ax: 0.99,
                    ay: -1.0,
                    u,
                    v,
                    sprite,
                    life: 0,
                });
            }
        }
    }
    /// HudEntityManager.tick.
    fn tick(&mut self) {
        self.pixels.retain(|p| p.life < MAX_LIFETIME);
        for p in &mut self.pixels {
            p.life += 1;
            p.x += p.vx;
            p.y -= p.vy;
            p.vx *= p.ax;
            p.vy += p.ay;
            for value in [&mut p.ax, &mut p.ay, &mut p.vx, &mut p.vy] {
                if value.abs() <= 0.1 {
                    *value = 0.0;
                }
            }
        }
    }
}

/// HeartTextureSet.getTexture(hardcore, half = false, blinking = false).
fn texture(kind: HeartKind, hardcore: bool) -> &'static str {
    match (kind, hardcore) {
        (HeartKind::Normal, false) => "hud/heart/full",
        (HeartKind::Normal, true) => "hud/heart/hardcore_full",
        (HeartKind::Poisoned, false) => "hud/heart/poisoned_full",
        (HeartKind::Poisoned, true) => "hud/heart/poisoned_hardcore_full",
        (HeartKind::Withered, false) => "hud/heart/withered_full",
        (HeartKind::Withered, true) => "hud/heart/withered_hardcore_full",
        (HeartKind::Frozen, false) => "hud/heart/frozen_full",
        (HeartKind::Frozen, true) => "hud/heart/frozen_hardcore_full",
    }
}

fn ceil(v: f32) -> i32 {
    f64::from(v).ceil() as i32
}

/// The frame's health row: the pixels move with the HUD's ticks, and a drop
/// in health splashes the lost halves (GuiMixin.render, onDamage).
pub(super) fn observe(i: &HealthInputs, w: i32, h: i32) {
    let Ok(mut s) = STATE.lock() else {
        return;
    };
    let steps = s.last_tick.map_or(0, |t| (i.tick - t).clamp(0, 20));
    s.last_tick = Some(i.tick);
    for _ in 0..steps {
        s.tick();
    }
    let health = ceil(i.health);
    let Some(last) = s.last_health.replace(health) else {
        return;
    };
    if health >= last {
        return;
    }
    // Hud.extractPlayerHealth's row layout.
    let absorption = ceil(i.absorption);
    let max_health = i.max_health.max(last.max(health) as f32);
    let rows = ceil((max_health + absorption as f32) / 2.0 / 10.0);
    let lines = (10 - (rows - 2)).max(3);
    let (x, y) = (w / 2 - 91, h - 39);
    let at = |index: i32| [x + index % 10 * 8, y - index / 10 * lines];
    let sprite = texture(i.kind, i.hardcore);
    let hearts = (f64::from(health) / 2.0).ceil() as i32;
    let last_hearts = (f64::from(last) / 2.0).ceil() as i32;
    let heart_dif = if last - health == 1 { 1 } else { last_hearts - hearts };
    let health_dif = last - health;
    let both = last % 2 != 0 && health % 2 != 0;
    let end_half = last % 2 == 0 && health % 2 != 0;
    let start_half = last % 2 != 0 && health % 2 == 0;
    if both {
        s.spawn(4, 9, at(hearts - 1), sprite);
        s.spawn(0, 5, at(hearts + heart_dif - 1), sprite);
    }
    if end_half {
        s.spawn(4, 9, at(hearts - 1), sprite);
    }
    if start_half {
        s.spawn(0, 5, at(hearts + heart_dif - 1), sprite);
    }
    if health_dif > 1 {
        for k in 0..heart_dif {
            if (start_half || both) && k + 1 == heart_dif {
                continue;
            }
            s.spawn(0, 9, at(hearts + k), sprite);
        }
    }
}

/// HudEntityManager.render: each pixel as a 1x1 piece of its heart sprite.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let Ok(s) = STATE.lock() else {
        return;
    };
    if s.pixels.is_empty() {
        return;
    }
    let draws: Vec<GuiDraw> = s
        .pixels
        .iter()
        .map(|p| GuiDraw::Sprite {
            sprite: Cow::Borrowed(p.sprite),
            x: p.x as i32,
            y: p.y as i32,
            w: 1,
            h: 1,
            region: Some([9, 9, p.u, p.v]),
            color: horizon_client::hud_layout::WHITE,
            blend: Blend::Textured,
        })
        .collect();
    drop(s);
    hud_layout::paint(batch, &game.gui.atlas, &draws);
}
