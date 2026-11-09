// Biome Paint for Horizon: the Biome Brush paints the box between two clicked blocks with the
// biome of the paint held in the other hand.
use super::{Game, Hand, Mapping};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::Tag;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const BRUSH: &str = "biome_paint:biome_brush";
const PAINT: &str = "biome_paint:biome_paint";
/// The brush's own colour when clean (`INT_COLOUR` 16765350).
const CLEAN: i32 = 16765350;
const PAINT_MAX_DAMAGE: u32 = 128;
const BRUSH_MAX_DAMAGE: u32 = 512;

/// Each player's first corner and the biome it was set with.
static FIRST: Mutex<Option<HashMap<u128, ([i32; 3], String)>>> = Mutex::new(None);
/// Columns painted inside a player's click, by level: their biomes go to the players at that
/// level's next tick (the hub is out of reach while a player's state is swapped in).
static RESEND: Mutex<Vec<(&'static str, (i32, i32))>> = Mutex::new(Vec::new());

/// After a level's tick: send the columns painted there since.
pub(super) fn level_tick_post(game: &mut Game<'_>, dimension: &str) {
    let columns: Vec<(i32, i32)> = {
        let mut pending = RESEND.lock().unwrap_or_else(|e| e.into_inner());
        if pending.is_empty() || game.lan.is_none() {
            return;
        }
        let (here, rest): (Vec<_>, Vec<_>) = pending.drain(..).partition(|(d, _)| *d == dimension);
        *pending = rest;
        here.into_iter().map(|(_, c)| c).collect()
    };
    if columns.is_empty() {
        return;
    }
    if let Some(host) = game.lan.as_mut() {
        host.hub.resend_biomes(&game.world, &columns);
    }
}

fn paint_biome(stack: &ItemStack) -> Option<String> {
    if &*stack.item != PAINT {
        return None;
    }
    match stack.custom_data()?.get(&horizon_world::nbt::JavaString::from("biome_paint"))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn tell(game: &mut Game<'_>, text: Text) {
    match game.lan_acting_uuid {
        Some(uuid) => {
            game.native_action_bar(uuid, text);
        }
        None => game.system_message(text),
    }
}

fn set_brush_colour(game: &mut Game<'_>, colour: i32) {
    if let Some(brush) = game.held_mut().as_mut() {
        brush.data_mut().dyed_color = (colour != CLEAN).then_some(colour);
        brush.tidy();
    }
    game.guest_inventory_resend = true;
}

pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], claimed: &mut bool) {
    if &*stack.item != BRUSH {
        return;
    }
    let player = game.lan_acting_uuid.unwrap_or(0);
    let other_slot = match game.hand {
        Hand::Main => super::shield::OFFHAND_SLOT,
        Hand::Off => game.selected,
    };
    let mut first = FIRST.lock().unwrap_or_else(|e| e.into_inner());
    let first = first.get_or_insert_with(HashMap::new);
    if game.keys.held(Mapping::Sneak) {
        // Clean the brush: forget the corner.
        first.remove(&player);
        *claimed = true;
        set_brush_colour(game, CLEAN);
        game.audio.play("minecraft:item.bucket.empty", Some(centre(pos)), 0.5, 1.4);
        tell(game, Text::translate("biome_paint.cleared", &[]));
        return;
    }
    let Some(paint) = game.stack_in(other_slot).cloned() else {
        return;
    };
    let Some(biome) = paint_biome(&paint) else {
        return;
    };
    *claimed = true;
    let colour = paint.data().and_then(|d| d.dyed_color).unwrap_or(CLEAN);
    match first.get(&player) {
        Some((corner, b)) if *b == biome => {
            let corner = *corner;
            match fill(game, corner, pos, &biome) {
                Ok(changed) => {
                    first.remove(&player);
                    set_brush_colour(game, CLEAN);
                    if !game.creative {
                        let brush_slot = game.held_slot();
                        for (slot, max, sound) in [
                            (brush_slot, BRUSH_MAX_DAMAGE, "minecraft:entity.item.break"),
                            (other_slot, PAINT_MAX_DAMAGE, "minecraft:block.glass.break"),
                        ] {
                            let cell = game.stack_in_mut(slot);
                            if let Some(s) = cell.as_mut() {
                                s.damage += 1;
                                if s.damage >= max {
                                    *cell = None;
                                    game.audio.play(sound, Some(centre(pos)), 0.8, 1.0);
                                }
                            }
                        }
                    }
                    game.guest_inventory_resend = true;
                    let pitch = 0.4 + rand_unit(pos) * 0.4;
                    game.audio.play("minecraft:item.brush.brushing.generic", Some(centre(pos)), 0.5, pitch);
                    game.audio.play("minecraft:block.amethyst_block.chime", Some(centre(pos)), 0.8, 1.0);
                    tell(game, Text::translate("biome_paint.painted", &[Text::literal(changed.to_string())]));
                }
                Err(message) => tell(game, message),
            }
        }
        _ => {
            first.insert(player, (pos, biome));
            set_brush_colour(game, colour);
            game.audio.play("minecraft:item.brush.brushing.generic", Some(centre(pos)), 0.5, 1.2);
            tell(game, Text::translate("biome_paint.first", &[]));
        }
    }
}

fn centre(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

fn rand_unit(pos: [i32; 3]) -> f32 {
    let h = (pos[0] as u32).wrapping_mul(73_856_093) ^ (pos[1] as u32).wrapping_mul(19_349_663) ^ (pos[2] as u32).wrapping_mul(83_492_791);
    (h % 1000) as f32 / 1000.0
}

/// `/fillbiome a b biome` (FillBiomeCommand.fill): the box quantized to 4-block biome cells,
/// at most `max_block_modifications` of them, every column loaded.
fn fill(game: &mut Game<'_>, a: [i32; 3], b: [i32; 3], biome: &str) -> Result<usize, Text> {
    let q = |v: i32| (v >> 2) << 2;
    let lo = [0, 1, 2].map(|i| q(a[i]).min(q(b[i])));
    let hi = [0, 1, 2].map(|i| q(a[i]).max(q(b[i])));
    let cells = i64::from((hi[0] - lo[0]) / 4 + 1) * i64::from((hi[1] - lo[1]) / 4 + 1) * i64::from((hi[2] - lo[2]) / 4 + 1);
    let volume = i64::from(hi[0] - lo[0] + 1) * i64::from(hi[1] - lo[1] + 1) * i64::from(hi[2] - lo[2] + 1);
    let limit = game.ui.rules.int("max_block_modifications");
    if volume > i64::from(limit) {
        return Err(Text::translate(
            "biome_paint.too_big",
            &[Text::literal(cells.to_string()), Text::literal((i64::from(limit) / 64).to_string())],
        ));
    }
    let Some(target) = world_biomes!(game).list.iter().position(|b| b.name == biome) else {
        return Err(Text::translate("argument.resource.not_found", &[Text::literal(biome.to_owned()), Text::literal("minecraft:worldgen/biome".to_owned())]));
    };
    for cz in (lo[2] >> 4)..=(hi[2] >> 4) {
        for cx in (lo[0] >> 4)..=(hi[0] >> 4) {
            if !game.world.chunks.contains_key(&(cx, cz)) {
                return Err(Text::translate("biome_paint.unloaded", &[]));
            }
        }
    }
    let mut changed = 0;
    let mut touched = HashSet::new();
    for z in (lo[2]..=hi[2]).step_by(4) {
        for y in (lo[1]..=hi[1]).step_by(4) {
            for x in (lo[0]..=hi[0]).step_by(4) {
                let key = (x >> 4, z >> 4);
                let Some(chunk) = game.world.chunks.get(&key) else {
                    continue;
                };
                if usize::from(chunk.biome(x, y, z)) == target {
                    continue;
                }
                let qy = ((y - game.world.bottom_y()) >> 2).clamp(0, game.world.layout().height() / 4 - 1);
                if let Some(chunk) = game.world.chunks.get_mut(&key) {
                    std::sync::Arc::make_mut(chunk).set_biome_cell((x & 15) >> 2, qy, (z & 15) >> 2, target as u16);
                }
                touched.insert(key);
                changed += 1;
            }
        }
    }
    for key in &touched {
        game.world.modified.insert(*key);
        for sy in 0..game.world.layout().sections() as i32 {
            game.world.dirty_sections.insert((key.0, sy, key.1));
        }
    }
    let columns: Vec<(i32, i32)> = ((lo[2] >> 4)..=(hi[2] >> 4))
        .flat_map(|cz| ((lo[0] >> 4)..=(hi[0] >> 4)).map(move |cx| (cx, cz)))
        .filter(|key| touched.contains(key))
        .collect();
    match game.lan.as_mut() {
        Some(host) => host.hub.resend_biomes(&game.world, &columns),
        None => {
            let level = game.dim.id();
            RESEND.lock().unwrap_or_else(|e| e.into_inner()).extend(columns.into_iter().map(|c| (level, c)));
        }
    }
    Ok(changed)
}
