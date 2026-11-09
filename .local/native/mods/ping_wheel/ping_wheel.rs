// Ping Wheel for Horizon: pings on the block looked at, drawn through the world primitives.
use super::Game;
use horizon_client::world_primitives::WorldPrimitives;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const ID: &str = "ping_wheel";
const PING_KEY: &str = "ping";
const DEFAULT_KEY: &str = "key.mouse.middle";
const RANGE: f64 = 256.0;
const LIFE: Duration = Duration::from_secs(7);
const MAX: usize = 8;
/// Ping Wheel's default ping colour (the player's own: a light blue).
const COLOR: u32 = 0xFF4E_C3FF;

struct Ping {
    at: [f64; 3],
    block: [i32; 3],
    since: Instant,
}

static PINGS: Mutex<Vec<Ping>> = Mutex::new(Vec::new());

pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_playing() {
        return;
    }
    if key != game.native_mod_key(ID, PING_KEY, DEFAULT_KEY) {
        return;
    }
    let Some((eye, dir)) = game.aim() else { return };
    let Some(hit) = game.clip_ray(eye, dir, RANGE) else { return };
    let mut pings = PINGS.lock().unwrap_or_else(|e| e.into_inner());
    pings.retain(|p| p.since.elapsed() < LIFE && p.block != hit.pos);
    if pings.len() >= MAX {
        pings.remove(0);
    }
    pings.push(Ping { at: hit.point, block: hit.pos, since: Instant::now() });
    drop(pings);
    game.native_play_local_sound("minecraft:block.note_block.bell", hit.point, 1.0, 2.0, 0);
}

pub(super) fn frame(_game: &Game<'_>, camera: [f64; 3], primitives: &mut WorldPrimitives) {
    let mut pings = PINGS.lock().unwrap_or_else(|e| e.into_inner());
    pings.retain(|p| p.since.elapsed() < LIFE);
    for p in pings.iter() {
        let age = p.since.elapsed().as_secs_f32();
        // Fades out over the last second.
        let alpha = ((LIFE.as_secs_f32() - age).clamp(0.0, 1.0) * 255.0) as u32;
        let color = (COLOR & 0x00FF_FFFF) | alpha << 24;
        let [bx, by, bz] = p.block;
        primitives
            .beam([f64::from(bx), f64::from(by) + 1.0, f64::from(bz)], 3, color)
            .through_walls();
        primitives
            .box_outline(
                [f64::from(bx), f64::from(by), f64::from(bz)],
                [f64::from(bx) + 1.0, f64::from(by) + 1.0, f64::from(bz) + 1.0],
                color,
                3.0,
            )
            .through_walls();
        let d = ((p.at[0] - camera[0]).powi(2)
            + (p.at[1] - camera[1]).powi(2)
            + (p.at[2] - camera[2]).powi(2))
        .sqrt();
        primitives
            .text([p.at[0], p.at[1] + 1.6, p.at[2]], format!("\u{25bc} {d:.1}m"), color, 1.0)
            .scale_with_distance()
            .background()
            .through_walls();
    }
}
