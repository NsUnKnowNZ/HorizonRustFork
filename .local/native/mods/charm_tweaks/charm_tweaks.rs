// Charm's Collection and Aerial Affinity enchantments.
use super::Game;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeLootDrops;

const COLLECTION: &str = "charm:collection";
const AERIAL_AFFINITY: &str = "charm:aerial_affinity";

/// A block broken with a Collection tool: its drops go to the breaker, what fits.
pub(super) fn collection(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    if context.block.is_none() || context.used || context.explosion.is_some() {
        return;
    }
    let (Some(player), Some(tool)) = (context.player, context.tool) else { return };
    if tool.enchantment_level(COLLECTION) == 0 {
        return;
    }
    let mut left = Vec::new();
    for stack in drops.drain(..) {
        if stack.count == 0 {
            continue;
        }
        if let Some(rest) = game.native_give_stack(player, stack) {
            left.push(rest);
        }
    }
    *drops = left;
}

/// Digging off the ground with Aerial Affinity boots: as fast as standing.
pub(super) fn aerial_affinity(game: &Game<'_>, original: f32, speed: &mut f32) {
    let Some(player) = game.player.as_ref() else { return };
    if player.ground.on_ground {
        return;
    }
    if game.armor[3].as_ref().is_none_or(|b| b.enchantment_level(AERIAL_AFFINITY) == 0) {
        return;
    }
    // Vanilla divides by five off the ground; a mod before us may have changed the speed, so
    // scale what it left.
    if original > 0.0 {
        *speed *= 5.0;
    }
}

/// A crop at its last age, the property that says it, and its seed.
fn grown(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match name {
        "minecraft:wheat" => ("age", "7", "minecraft:wheat_seeds"),
        "minecraft:carrots" => ("age", "7", "minecraft:carrot"),
        "minecraft:potatoes" => ("age", "7", "minecraft:potato"),
        "minecraft:beetroots" => ("age", "3", "minecraft:beetroot_seeds"),
        "minecraft:nether_wart" => ("age", "3", "minecraft:nether_wart"),
        _ => return None,
    })
}

const TURN: [&str; 4] = ["north", "east", "south", "west"];

fn step(dir: &str, by: usize) -> Option<&'static str> {
    let i = TURN.iter().position(|d| *d == dir)?;
    Some(TURN[(i + by) % 4])
}

fn offset(dir: &str) -> [i32; 3] {
    match dir {
        "north" => [0, 0, -1],
        "east" => [1, 0, 0],
        "south" => [0, 0, 1],
        _ => [-1, 0, 0],
    }
}

/// On the world's own game (a client of a server leaves it to the server): replanting and
/// double doors.
pub(super) fn click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || game.remote.is_some() || !game.world.loaded(pos[0], pos[2]) {
        return;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let Some(name) = game.native_state_block(state).map(str::to_owned) else { return };
    let prop = |game: &Game<'_>, s, k| game.native_block_state_property(s, k).map(str::to_owned);
    if let Some((key, last, seed)) = grown(&name) {
        if prop(game, state, key).as_deref() != Some(last) || game.keys.held(super::Mapping::Sneak) {
            return;
        }
        *canceled = true;
        let [x, y, z] = pos;
        // Harvested into the player's hands (what doesn't fit is lost), one seed kept back for
        // the planting, and it grows again from the start.
        let roll = |n: u32| (WEAR.fetch_add(7919, std::sync::atomic::Ordering::Relaxed).wrapping_mul(2654435761) >> 7) % (n + 1);
        let harvest: Vec<(&str, u32)> = match name.as_str() {
            "minecraft:wheat" => vec![("minecraft:wheat", 1), (seed, roll(3))],
            "minecraft:beetroots" => vec![("minecraft:beetroot", 1), (seed, roll(3))],
            "minecraft:nether_wart" => vec![(seed, 1 + roll(2))],
            _ => vec![(seed, roll(3))],
        };
        let player = game.native_acting_id();
        for (item, count) in harvest {
            if count > 0 {
                let _ = game.native_give_stack(player, ItemStack::new(item, count));
            }
        }
        game.native_set_block_property(pos, key, "0");
        game.native_play_local_sound("minecraft:item.crop.plant", [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5], 1.0, 1.0, 0);
        return;
    }
    // Chairs: a bottom stair is a seat for an empty hand (Handcrafted's seats do the sitting).
    if name.ends_with("_stairs")
        && prop(game, state, "half").as_deref() == Some("bottom")
        && game.stack_in(game.selected).is_none()
        && !game.keys.held(super::Mapping::Sneak)
        && game.riding.is_none()
    {
        super::handcrafted::sit(game, pos, canceled);
        return;
    }
    // A wooden door: its double's other half follows. The door itself turns after this hook.
    if name.ends_with("_door") && name != "minecraft:iron_door" {
        let (Some(facing), Some(hinge), Some(open), Some(half)) =
            (prop(game, state, "facing"), prop(game, state, "hinge"), prop(game, state, "open"), prop(game, state, "half"))
        else {
            return;
        };
        let Some(side) = step(&facing, if hinge == "left" { 1 } else { 3 }) else { return };
        let o = offset(side);
        let other = [pos[0] + o[0], pos[1], pos[2] + o[2]];
        let other_state = game.world.block(other[0], other[1], other[2]);
        let same = game.native_state_block(other_state) == Some(name.as_str())
            && prop(game, other_state, "facing").as_deref() == Some(facing.as_str())
            && prop(game, other_state, "hinge").as_deref() != Some(hinge.as_str())
            && prop(game, other_state, "half").as_deref() == Some(half.as_str())
            && prop(game, other_state, "open").as_deref() == Some(open.as_str());
        if same {
            let to = if open == "true" { "false" } else { "true" };
            game.native_set_block_property(other, "open", to);
        }
    }
}

static WEAR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Anvils last longer: every other bit of wear is undone (the anvil goes back as it was).
pub(super) fn anvil(game: &mut Game<'_>, pos: [i32; 3], from: &str) {
    if WEAR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 2 == 1 {
        return;
    }
    let [x, y, z] = pos;
    let state = game.world.block(x, y, z);
    let facing = game.native_block_state_property(state, "facing").unwrap_or("north").to_owned();
    let registry = game.pack.assets.registry;
    let Some(back) = registry
        .default_state(from)
        .and_then(|s| registry.state(s).map(|d| d.name.clone()))
        .and_then(|name| {
            (0..registry.len() as u32).map(horizon_world::registry::BlockStateId).find(|&id| {
                registry
                    .state(id)
                    .is_some_and(|s| s.name == name && s.properties.get("facing").is_some_and(|f| *f == facing))
            })
        })
    else {
        return;
    };
    let dim = game.dim.id().to_owned();
    game.native_set_block(&dim, pos, back, 3);
}
