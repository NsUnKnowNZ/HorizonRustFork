// Mob Plaques: MobPlaquesRenderer with the pack's settings (health RELATIVE,
// armour, scale 0.5 growing with distance, 48 blocks, background, depth
// tested, hidden at full health, armor stands left out).
use super::*;
use horizon_client::world_primitives::{WorldPrimitives, text_unit};

const SCALE: f32 = 0.5;
const MAX_DISTANCE: f64 = 48.0;
const HEART: u32 = 0xFFFF_5555;
const ARMOR: u32 = 0xFFAA_AAAA;
const VALUE: u32 = 0xFFFF_FFFF;

/// One row: (glyph, glyph colour, value) pairs laid out left to right and
/// centred on `at`, each piece a primitive of its own.
fn row(
    game: &Game<'_>,
    primitives: &mut WorldPrimitives,
    at: [f64; 3],
    camera: [f64; 3],
    pieces: &[(&str, u32)],
) {
    let fwd: [f64; 3] = std::array::from_fn(|k| at[k] - camera[k]);
    let distance = fwd.iter().map(|v| v * v).sum::<f64>().sqrt();
    if distance < 1e-6 {
        return;
    }
    // The camera's right: forward x up.
    let mut right = [-fwd[2], 0.0, fwd[0]];
    let len = (right[0] * right[0] + right[2] * right[2]).sqrt();
    if len < 1e-9 {
        right = [1.0, 0.0, 0.0];
    } else {
        right = [right[0] / len, 0.0, right[2] / len];
    }
    let unit = f64::from(text_unit(SCALE, true, distance));
    let widths: Vec<f64> = pieces
        .iter()
        .map(|(t, _)| f64::from(game.native_text_width(t)))
        .collect();
    let total: f64 = widths.iter().sum();
    let mut x = -total / 2.0;
    for ((text, color), w) in pieces.iter().zip(&widths) {
        let centre = (x + w / 2.0) * unit;
        let p = std::array::from_fn(|k| at[k] + right[k] * centre);
        primitives
            .text(p, *text, *color, SCALE)
            .scale_with_distance()
            .background();
        x += w;
    }
}

pub(super) fn frame(game: &Game<'_>, partial: f32, camera: [f64; 3], primitives: &mut WorldPrimitives) {
    let t = f64::from(partial);
    for m in game.mobs.iter() {
        if !m.alive() || m.kind.id == "minecraft:armor_stand" {
            continue;
        }
        let max = m.kind.max_health;
        if max <= 0.0 || m.health >= max {
            continue;
        }
        let pos = [
            m.previous.x + (m.position.x - m.previous.x) * t,
            m.previous.y + (m.position.y - m.previous.y) * t,
            m.previous.z + (m.position.z - m.previous.z) * t,
        ];
        let d2: f64 = (0..3).map(|k| (pos[k] - camera[k]).powi(2)).sum();
        if d2 > MAX_DISTANCE * MAX_DISTANCE {
            continue;
        }
        // Above the name tag when the mob shows one.
        let named = m.custom_name.is_some();
        let y = pos[1] + m.kind.height + 0.5 + if named { 0.3 } else { 0.0 };
        let percent = ((m.health / max * 100.0) as i32).max(1);
        let health = format!("{percent}%");
        let armor = m.kind.armor.floor() as i32;
        let armor_text = armor.to_string();
        let mut pieces: Vec<(&str, u32)> = vec![("\u{2764} ", HEART), (&health, VALUE)];
        if armor > 0 {
            pieces.extend([("  \u{26E8} ", ARMOR), (armor_text.as_str(), VALUE)]);
        }
        row(game, primitives, [pos[0], y, pos[2]], camera, &pieces);
    }
}
