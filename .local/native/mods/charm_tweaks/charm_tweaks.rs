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
