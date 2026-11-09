// Handcrafted for Horizon: sheets and cushions colour the furniture, the hammer turns its looks,
// and the seats can be sat on.
use super::{Game, Mapping};
use horizon_client::items::ItemStack;

const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

/// The furniture kind of a Handcrafted block: the last word(s) of its name.
fn kind(block: &str) -> Option<&'static str> {
    let path = block.strip_prefix("handcrafted:")?;
    [
        "fancy_bed", "side_table", "dining_bench", "nightstand", "table", "desk", "chair", "couch",
        "bench", "shelf", "counter", "cupboard", "drawer", "corner_trim", "pillar_trim",
    ]
    .into_iter()
    .find(|k| path == *k || path.ends_with(&format!("_{k}")))
}

fn takes_sheet(kind: &str) -> bool {
    matches!(kind, "table" | "side_table" | "nightstand" | "desk" | "fancy_bed")
}

fn takes_cushion(kind: &str) -> bool {
    matches!(kind, "chair" | "couch" | "bench" | "dining_bench")
}

/// The colour a sheet (`handcrafted:red_sheet`) or cushion (`handcrafted:red_cushion`) gives.
fn cloth(item: &str, suffix: &str) -> Option<&'static str> {
    let path = item.strip_prefix("handcrafted:")?.strip_suffix(suffix)?;
    COLORS.into_iter().find(|c| *c == path)
}

fn block_at(game: &Game<'_>, pos: [i32; 3]) -> Option<(String, horizon_world::registry::BlockStateId)> {
    if !game.world.loaded(pos[0], pos[2]) {
        return None;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    Some((game.native_state_block(state)?.to_owned(), state))
}

/// A fancy bed's other half: the head is one block toward its facing from the foot.
fn other_half(game: &Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId) -> Option<[i32; 3]> {
    let facing = game.native_block_state_property(state, "facing")?;
    let part = game.native_block_state_property(state, "part")?;
    let d = match facing {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    };
    let s = if part == "foot" { 1 } else { -1 };
    Some([pos[0] + d[0] * s, pos[1], pos[2] + d[2] * s])
}

fn set_color(game: &mut Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId, color: &str) -> bool {
    let changed = game.native_set_block_property(pos, "color", color);
    if changed && let Some(other) = other_half(game, pos, state) {
        game.native_set_block_property(other, "color", color);
    }
    changed
}

fn use_up_held(game: &mut Game<'_>) {
    if game.creative {
        return;
    }
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

/// A sheet, cushion or hammer used on Handcrafted furniture.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], claimed: &mut bool) {
    let Some((block, state)) = block_at(game, pos) else {
        return;
    };
    let Some(kind) = kind(&block) else {
        return;
    };
    let item = &*stack.item;
    let centre = pos.map(|v| f64::from(v) + 0.5);
    if item == "handcrafted:hammer" {
        let key = ["type", "trim", "counter"]
            .into_iter()
            .find(|k| game.native_block_state_property(state, k).is_some());
        let Some(key) = key else { return };
        let Some(current) = game.native_block_state_property(state, key).map(str::to_owned) else {
            return;
        };
        let values: Vec<String> = game
            .pack
            .assets
            .registry
            .property_values(&block, key)
            .into_iter()
            .map(str::to_owned)
            .collect();
        if values.is_empty() {
            return;
        }
        let next = values
            .iter()
            .position(|v| *v == current)
            .map_or(0, |i| (i + 1) % values.len());
        *claimed = true;
        if game.native_set_block_property(pos, key, &values[next]) {
            game.audio.play("minecraft:block.wood.hit", Some(centre), 1.0, 1.0);
        }
        return;
    }
    let color = if takes_sheet(kind) {
        cloth(item, "_sheet")
    } else if takes_cushion(kind) {
        cloth(item, "_cushion")
    } else {
        None
    };
    let Some(color) = color else { return };
    if game.native_block_state_property(state, "color") == Some(color) {
        return;
    }
    *claimed = true;
    let old = game.native_block_state_property(state, "color").map(str::to_owned);
    if set_color(game, pos, state, color) {
        use_up_held(game);
        // What was on it comes back.
        if let Some(old) = old.filter(|o| o != "none") {
            let suffix = if takes_sheet(kind) { "sheet" } else { "cushion" };
            game.give_or_drop(ItemStack::new(&format!("handcrafted:{old}_{suffix}"), 1));
        }
        game.audio.play("minecraft:block.wool.place", Some(centre), 1.0, 1.0);
    }
}

/// The tag of the seats: invisible, silent mobs that never move, put in a seat to ride.
const SEAT_TAG: &str = "handcrafted_seat";

fn seat(kind: &str) -> bool {
    matches!(kind, "chair" | "couch" | "bench" | "dining_bench")
}

/// Right click with an empty hand on a seat: sit down on it (an invisible silverfish in the seat,
/// which the rider sits on; it goes when they get up).
fn sit(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let player = game.native_acting_entity();
    if player == 0 && game.lan_acting.is_some() {
        return;
    }
    let dim = game.dim.id();
    // A silverfish's passengers sit 0.3 above it; a rider's own seat is 0.6 under its feet.
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.05, f64::from(pos[2]) + 0.5];
    let snbt = format!(
        "{{NoAI:1b,Silent:1b,Invulnerable:1b,NoGravity:1b,PersistenceRequired:1b,Tags:[\"{SEAT_TAG}\"],active_effects:[{{id:\"minecraft:invisibility\",duration:-1,amplifier:0b,show_particles:0b,ambient:1b}}]}}"
    );
    let Ok((horizon_world::nbt::Tag::Compound(nbt), _)) = horizon_world::nbt::parse_snbt(&snbt) else {
        return;
    };
    let Some(id) = game.native_summon(dim, "minecraft:silverfish", at, nbt) else {
        return;
    };
    *canceled = true;
    if !game.native_mount(player, id) {
        game.native_remove_mob(dim, id);
    }
}

/// After each level tick: seats nobody sits on go.
pub(super) fn level_tick_post(game: &mut Game<'_>, dimension: &str) {
    let seats: Vec<u32> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.kind.id == "minecraft:silverfish")
        .map(|m| m.id)
        .filter(|&id| game.tags_of(super::commands::Target::Mob(id)).iter().any(|t| t == SEAT_TAG))
        .collect();
    for id in seats {
        let ridden = game.riding == Some(id)
            || game
                .lan
                .as_ref()
                .is_some_and(|h| h.hub.guests.iter().any(|g| g.vehicle == Some(id)));
        if !ridden {
            game.native_remove_mob(dimension, id);
        }
    }
}

/// Sneak + right click with an empty hand: the sheet or cushion comes off. Without sneaking, on a
/// seat: sit down.
pub(super) fn empty_hand(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if game.stack_in(game.selected).is_some() {
        return;
    }
    if !game.keys.held(Mapping::Sneak) {
        if let Some((block, _)) = block_at(game, pos)
            && kind(&block).is_some_and(seat)
            && game.riding.is_none()
        {
            sit(game, pos, canceled);
        }
        return;
    }
    let Some((block, state)) = block_at(game, pos) else {
        return;
    };
    let Some(kind) = kind(&block) else {
        return;
    };
    if !(takes_sheet(kind) || takes_cushion(kind)) {
        return;
    }
    let Some(old) = game.native_block_state_property(state, "color").map(str::to_owned) else {
        return;
    };
    if old == "none" {
        return;
    }
    *canceled = true;
    if set_color(game, pos, state, "none") {
        let suffix = if takes_sheet(kind) { "sheet" } else { "cushion" };
        if !game.creative {
            game.give_or_drop(ItemStack::new(&format!("handcrafted:{old}_{suffix}"), 1));
        }
        game.audio.play("minecraft:block.wool.break", Some(pos.map(|v| f64::from(v) + 0.5)), 1.0, 1.0);
    }
}

/// Handcrafted's storage furniture is a barrel underneath, which would face up or down when
/// placed looking at the floor: it faces the player, level, as the furniture does.
pub(super) fn state_for_placement(
    level: &super::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    yaw: f32,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    if answer.is_some() {
        return;
    }
    let Some(block) = level.native_state_block(state) else {
        return;
    };
    if !block.starts_with("handcrafted:") || level.native_state_with_property(state, "open", "false").is_none() {
        return;
    }
    // The player's horizontal direction (yaw 0 looks south), turned to face them.
    let toward = match (((f64::from(yaw) / 90.0).round() as i64).rem_euclid(4)) as u8 {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    };
    if let Some(s) = level.native_state_with_property(state, "facing", toward) {
        *answer = Some(Some(s));
    }
}
