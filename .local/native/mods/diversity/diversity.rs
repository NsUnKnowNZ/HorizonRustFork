// Diversity for Horizon: the bundle enchantments Vacuum, Selective Vacuum and Refill.
use super::Game;
use horizon_client::bundle::Contents;
use horizon_client::items::ItemStack;
use std::collections::HashMap;
use std::sync::Mutex;

/// Per player: items picked up since their last tick, and a hand slot to refill.
#[derive(Default)]
struct Pending {
    picked: Vec<(String, u32)>,
    refill: Option<(usize, String)>,
}

static PENDING: Mutex<Option<HashMap<u128, Pending>>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut HashMap<u128, Pending>) -> R) -> R {
    let mut held = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    f(held.get_or_insert_with(HashMap::new))
}

fn level(stack: &ItemStack, id: &str) -> u32 {
    stack.enchantment_level(id)
}

pub(super) fn picked_up(player: u128, stack: &ItemStack) {
    if stack.count > 0 {
        with(|p| p.entry(player).or_default().picked.push((stack.item.to_string(), stack.count)));
    }
}

pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack) {
    if stack.count == 1 && !game.creative {
        let player = game.native_acting_id();
        let slot = game.selected;
        with(|p| p.entry(player).or_default().refill = Some((slot, stack.item.to_string())));
    }
}

/// The inventory slots holding enchanted bundles, Vacuum ones first.
fn bundles(game: &Game<'_>, vacuum: bool) -> Vec<usize> {
    (0..game.inventory.len())
        .filter(|&i| {
            game.inventory[i].as_ref().is_some_and(|s| {
                Contents::of(game.items(), s).is_some()
                    && if vacuum {
                        level(s, "diversity:vacuum") > 0 || level(s, "diversity:selective_vacuum") > 0
                    } else {
                        level(s, "diversity:refill") > 0
                    }
            })
        })
        .collect()
}

pub(super) fn player_tick(game: &mut Game<'_>) {
    let player = game.native_acting_id();
    let pending = with(|p| std::mem::take(p.entry(player).or_default()));
    // Vacuum: what was just picked up moves from the inventory into the bundles.
    if !pending.picked.is_empty() {
        for b in bundles(game, true) {
            for (item, mut count) in pending.picked.iter().cloned() {
                for i in 0..game.inventory.len() {
                    if count == 0 || i == b {
                        continue;
                    }
                    let Some(stack) = game.inventory[i].clone().filter(|s| *s.item == *item && s.extra.is_none()) else {
                        continue;
                    };
                    let Some(bundle) = game.inventory[b].clone() else { break };
                    let items = game.items();
                    let Some(mut contents) = Contents::of(items, &bundle) else { break };
                    let selective = level(&bundle, "diversity:vacuum") == 0;
                    if selective && !contents.items().iter().any(|s| *s.item == *item) {
                        break;
                    }
                    let mut moving = ItemStack::new(item.as_str(), stack.count.min(count));
                    let took = contents.try_insert(items, &mut moving);
                    if took == 0 {
                        break;
                    }
                    let mut bundle = bundle;
                    contents.store(&mut bundle);
                    game.inventory[b] = Some(bundle);
                    let left = stack.count - took;
                    game.inventory[i] = (left > 0).then(|| ItemStack { count: left, ..stack });
                    count -= took;
                }
            }
        }
    }
    // Refill: the emptied hand slot takes more of the block from a Refill bundle.
    if let Some((slot, item)) = pending.refill
        && game.inventory.get(slot).is_some_and(|s| s.is_none())
    {
        for b in bundles(game, false) {
            let Some(bundle) = game.inventory[b].clone() else { continue };
            let items = game.items();
            let Some(contents) = Contents::of(items, &bundle) else { continue };
            let inside: Vec<ItemStack> = contents.items().to_vec();
            let Some(pos) = inside.iter().position(|s| *s.item == *item) else { continue };
            let mut rest = inside;
            let taken = rest.remove(pos);
            let mut bundle = bundle;
            Contents::from_items(items, rest).store(&mut bundle);
            game.inventory[b] = Some(bundle);
            game.inventory[slot] = Some(taken);
            break;
        }
    }
}
