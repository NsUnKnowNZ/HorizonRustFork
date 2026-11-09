// Underlay (Dooji, GPL-3.0, local use) for Horizon: a carpet, a pressure plate or a rail in
// the space of a block that doesn't fill it (a chest, a slab, a fence, a bed, a flower pot...).
// The overlays are a synced chunk attachment (`underlay:overlays`), so they save with the world
// and every Horizon client in sight is sent them; the client draws them in the world canvas.
use super::Game;
use super::native_world_draw::NativeWorldDraw;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, Tag};
use std::sync::Mutex;

/// The overlays of one chunk: each one's position and block.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Overlays(Vec<([i32; 3], String)>);

fn encode(o: &Overlays) -> Result<Compound, AttachmentError> {
    let mut at = Vec::with_capacity(o.0.len() * 3);
    let mut blocks = Vec::with_capacity(o.0.len());
    for (pos, block) in &o.0 {
        at.extend_from_slice(pos);
        blocks.push(Tag::String(block.as_str().into()));
    }
    Ok([("at".into(), Tag::IntArray(at)), ("blocks".into(), Tag::List(blocks))].into())
}

fn decode(data: &Compound) -> Result<Overlays, AttachmentError> {
    let (Some(Tag::IntArray(at)), Some(Tag::List(blocks))) = (data.get(&"at".into()), data.get(&"blocks".into())) else {
        return Err(AttachmentError::Decode);
    };
    if at.len() != blocks.len() * 3 {
        return Err(AttachmentError::Decode);
    }
    let mut out = Vec::with_capacity(blocks.len());
    for (pos, block) in at.chunks_exact(3).zip(blocks) {
        let Tag::String(block) = block else {
            return Err(AttachmentError::Decode);
        };
        out.push(([pos[0], pos[1], pos[2]], block.to_string_lossy()));
    }
    Ok(Overlays(out))
}

pub(super) static OVERLAYS: AttachmentType<Overlays> = AttachmentType {
    namespace: "underlay",
    name: "overlays",
    default: Overlays::default,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

/// The blocks that go under others: the mod's defaults (carpets, pressure plates, rails).
fn overlay_block(item: &str) -> bool {
    item.ends_with("_carpet")
        || item.ends_with("_pressure_plate")
        || matches!(item, "minecraft:rail" | "minecraft:powered_rail" | "minecraft:detector_rail" | "minecraft:activator_rail")
}

fn chunk(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

/// A block an overlay may share: there, not filling its space, not replaceable, no fluid.
fn host(game: &Game<'_>, pos: [i32; 3]) -> bool {
    if !game.world.loaded(pos[0], pos[2]) {
        return false;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts().get(state);
    let name = &*facts.name;
    !matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
        && !facts.replaceable
        && !facts.full_collision
        && !facts.waterlogged
        && !overlay_block(name)
}

fn overlays_at(game: &mut Game<'_>, pos: [i32; 3]) -> Option<Overlays> {
    let host: &mut dyn Host = game;
    OVERLAYS.get_existing(host, chunk(pos)).ok().flatten()
}

fn store(game: &mut Game<'_>, pos: [i32; 3], overlays: Overlays) {
    let host: &mut dyn Host = game;
    let _ = if overlays.0.is_empty() {
        OVERLAYS.remove(host, chunk(pos)).map(|_| ())
    } else {
        OVERLAYS.set(host, chunk(pos), overlays)
    };
}

/// Sneak + use a carpet, pressure plate or rail on such a block: it goes under it.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], claimed: &mut bool) {
    if game.remote.is_some() || !game.native_sneaking() || !overlay_block(&stack.item) || !host(game, pos) {
        return;
    }
    let block = stack.item.to_string();
    if game.native_default_state(&block).is_none() {
        return;
    }
    let mut overlays = overlays_at(game, pos).unwrap_or_default();
    if overlays.0.iter().any(|(p, _)| *p == pos) {
        return;
    }
    overlays.0.push((pos, block));
    store(game, pos, overlays);
    *claimed = true;
    if !game.creative {
        let cell = game.held_mut();
        if let Some(s) = cell.as_mut() {
            if s.count > 1 {
                s.count -= 1;
            } else {
                *cell = None;
            }
        }
        game.guest_inventory_resend = true;
    }
}

/// Takes the overlay at `pos` out, dropping its item unless `drop` is false (creative).
fn take(game: &mut Game<'_>, pos: [i32; 3], drop: bool) -> bool {
    let Some(mut overlays) = overlays_at(game, pos) else {
        return false;
    };
    let Some(i) = overlays.0.iter().position(|(p, _)| *p == pos) else {
        return false;
    };
    let (_, block) = overlays.0.remove(i);
    store(game, pos, overlays);
    if drop {
        let item = game
            .native_default_state(&block)
            .map(|s| game.res.facts().get(s).item.to_string())
            .filter(|i| !i.is_empty() && i != "minecraft:air")
            .unwrap_or(block);
        let dimension = game.dim.id().to_owned();
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.25, f64::from(pos[2]) + 0.5];
        DROPS.lock().unwrap_or_else(|e| e.into_inner()).push((dimension, item, at));
    }
    true
}

/// Sneak + attack a block with an overlay breaks the overlay instead of the block (Underlay
/// aims its attacks at the overlay; Carry On keeps sneak + use with an empty hand).
pub(super) fn attack(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if game.remote.is_some() || !game.native_sneaking() {
        return;
    }
    let drop = !game.creative;
    if take(game, pos, drop) {
        *canceled = true;
    }
}

/// Items an overlay leaves when its block goes, for the level's tick.
static DROPS: Mutex<Vec<(String, String, [f64; 3])>> = Mutex::new(Vec::new());

/// A player breaks a block: its overlay drops with it.
pub(super) fn destroyed(game: &mut Game<'_>, pos: [i32; 3]) {
    if game.remote.is_none() {
        let drop = !game.creative;
        take(game, pos, drop);
    }
}

pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    let drops = {
        let mut all = DROPS.lock().unwrap_or_else(|e| e.into_inner());
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *all).into_iter().partition(|d| d.0 == dimension);
        *all = rest;
        here
    };
    for (_, item, [x, y, z]) in drops {
        game.native_run_command(&format!("execute in {dimension} run summon minecraft:item {x} {y} {z} {{Item:{{id:\"{item}\",count:1}}}}"));
    }
}

/// The overlays near the client's player, read from the synced chunks each half second.
static SEEN: Mutex<Vec<([i32; 3], String)>> = Mutex::new(Vec::new());

pub(super) fn client_tick(game: &mut Game<'_>) {
    if game.world.game_time % 10 != 0 {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        SEEN.lock().unwrap_or_else(|e| e.into_inner()).clear();
        return;
    };
    let (cx, cz) = ((player.position.x.floor() as i32) >> 4, (player.position.z.floor() as i32) >> 4);
    let radius = 6;
    let mut seen = Vec::new();
    for x in cx - radius..=cx + radius {
        for z in cz - radius..=cz + radius {
            if !game.world.chunks.contains_key(&(x, z)) {
                continue;
            }
            let host: &mut dyn Host = game;
            if let Ok(Some(o)) = OVERLAYS.get_existing(host, Holder::Chunk([x, z])) {
                seen.extend(o.0);
            }
        }
    }
    seen.retain(|(pos, _)| host(game, *pos));
    *SEEN.lock().unwrap_or_else(|e| e.into_inner()) = seen;
}

/// Draws each overlay in its block's space, lit as that block is.
pub(super) fn submit(game: &Game<'_>, draw: &mut NativeWorldDraw) {
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    for (pos, block) in seen.iter() {
        let at = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
        draw.push();
        if draw.translate_world(at) {
            let light = game.world.light(pos[0], pos[1], pos[2]);
            draw.light = horizon_client::block_entity_render::vertex_light(light);
            game.native_draw_block(&mut *draw, block, &[]);
        }
        draw.pop();
    }
}
