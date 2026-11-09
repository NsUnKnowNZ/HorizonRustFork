// Reacharound (Spanser, CC BY-NC-SA 3.0), PlacementFeature and Hud: where
// the crosshair meets nothing, a held block item may still go against the
// block the player stands on (ahead of it, or below / above when looking
// down / up). Better OneBlock: both axes, vertical first, off hand allowed.
use super::*;

const RANGE_ADJUSTMENT: f64 = 0.5;
const MIN_VERTICAL_DISTANCE: f64 = 1.0;
const LENIENCY_HORIZONTAL: f64 = 0.5;
const LENIENCY_VERTICAL: f64 = 0.5;

#[derive(Clone, Copy)]
pub(super) struct Target {
    pub(super) pos: [i32; 3],
    pub(super) face: [i32; 3],
    pub(super) vertical: bool,
    pub(super) hand: Hand,
}

/// validateReacharoundStack: a BlockItem.
fn holds_block(game: &Game<'_>, main: bool) -> bool {
    let stack = if main { game.held() } else { game.offhand.as_ref() };
    stack
        .and_then(|s| game.items().get(&s.item))
        .is_some_and(|i| i.block.is_some())
}

/// Inside the world's height and air or replaceable.
fn open(game: &Game<'_>, pos: [i32; 3]) -> bool {
    if pos[1] < game.world.shape.min_y || pos[1] >= game.world.shape.max_y() {
        return false;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts();
    facts.is_air(state) || facts.get(state).replaceable
}

/// Direction.fromYRot as (stepX, stepZ).
fn facing(yaw: f32) -> (i32, i32) {
    match ((f64::from(yaw) / 90.0 + 0.5).floor() as i32) & 3 {
        0 => (0, 1),
        1 => (-1, 0),
        2 => (0, -1),
        _ => (1, 0),
    }
}

/// checkPlayerReacharoundTarget.
pub(super) fn target(game: &Game<'_>) -> Option<Target> {
    if game.spectator() || game.entity_targeted() {
        return None;
    }
    let hand = if holds_block(game, true) {
        Hand::Main
    } else if holds_block(game, false) {
        Hand::Off
    } else {
        return None;
    };
    if game.pick().is_some() {
        return None;
    }
    let (eye, dir) = game.aim()?;
    let range = game.block_reach - RANGE_ADJUSTMENT;
    let ray = [eye[0] + dir[0] * 0.5, eye[1] + dir[1] * 0.5, eye[2] + dir[2] * 0.5];
    if game.clip_ray(ray, dir, range).is_some() {
        return None;
    }
    // Vertical first: the block the ray meets a little higher (looking down)
    // or lower (up), and the cell past it.
    let down = game.pitch > 0.0;
    let dy = if down { LENIENCY_VERTICAL } else { -LENIENCY_VERTICAL };
    if let Some(hit) = game.clip_ray([ray[0], ray[1] + dy, ray[2]], dir, range) {
        let pos = [hit.pos[0], hit.pos[1] + if down { -1 } else { 1 }, hit.pos[2]];
        let feet = game.player.as_ref()?.position.y;
        let mut distance = f64::from(pos[1]) - feet;
        if down {
            distance = -distance;
        }
        if distance > MIN_VERTICAL_DISTANCE && open(game, pos) {
            let face = if down { [0, -1, 0] } else { [0, 1, 0] };
            return Some(Target { pos, face, vertical: true, hand });
        }
    }
    // Horizontal: the ray from a little behind, and the cell ahead of the block it meets.
    let (sx, sz) = facing(game.yaw);
    let origin = [
        ray[0] - LENIENCY_HORIZONTAL * f64::from(sx),
        ray[1],
        ray[2] - LENIENCY_HORIZONTAL * f64::from(sz),
    ];
    let hit = game.clip_ray(origin, dir, range)?;
    let pos = [hit.pos[0] + sx, hit.pos[1], hit.pos[2] + sz];
    open(game, pos).then_some(Target { pos, face: [-sx, 0, -sz], vertical: false, hand })
}

/// Hud.renderStyleDefault, white, after the crosshair layer.
pub(super) fn hud(game: &mut Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if !game.camera_type.first_person() {
        return;
    }
    let Some(t) = target(game) else {
        return;
    };
    let text = if t.vertical { "- -" } else { "-   -" };
    let mut y = batch.height / 2.0 - 4.0;
    if t.vertical {
        y += if game.pitch < 0.0 { -4.0 } else { 4.0 };
    }
    let x = batch.width / 2.0 - game.native_text_width(text) / 2.0;
    game.native_text(batch, text, [x, y], 0xFF_FF_FF, false);
}
