// Simple Death Improvements, the game side.
//
// `Player.dropEquipment` hands the native mods the stacks it is about to throw
// (`loot_drops`), in `death_drops`' order: inventory slots 0..36, the off hand,
// armour head to feet, without Curse of Vanishing items. A snapshot of each
// player's slots, taken every server tick, tells which stack came from which
// slot; the kept ones go back where they were.
use horizon_client::entities::simple_death::config;
use horizon_client::items::ItemStack;
use std::collections::HashMap;
use std::sync::Mutex;

/// Where a stack sat: an inventory slot, the off hand, or an armour slot.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Slot {
    Inventory(usize),
    Offhand,
    Armor(usize),
}

type Layout = Vec<(Slot, String)>;

static LAYOUTS: Mutex<Option<HashMap<u128, Layout>>> = Mutex::new(None);
/// The death drops' entity ids, to calm on the next tick: (first id, count).
static CALM: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

fn cursed(s: &ItemStack) -> bool {
    s.enchantment_level("minecraft:vanishing_curse") != 0
}

/// `death_drops`' order over the slots that hold an uncursed stack.
fn layout(
    inventory: &[Option<ItemStack>],
    offhand: &Option<ItemStack>,
    armor: &[Option<ItemStack>],
) -> Layout {
    let mut out = Vec::new();
    for (i, s) in inventory.iter().enumerate() {
        if let Some(s) = s.as_ref().filter(|s| !cursed(s)) {
            out.push((Slot::Inventory(i), s.item.to_string()));
        }
    }
    if let Some(s) = offhand.as_ref().filter(|s| !cursed(s)) {
        out.push((Slot::Offhand, s.item.to_string()));
    }
    for (i, s) in armor.iter().enumerate().rev() {
        if let Some(s) = s.as_ref().filter(|s| !cursed(s)) {
            out.push((Slot::Armor(i), s.item.to_string()));
        }
    }
    out
}

fn kept(slot: Slot) -> bool {
    let c = config();
    match slot {
        Slot::Inventory(i) => c.hotbar && i < 9,
        Slot::Offhand => c.offhand,
        Slot::Armor(_) => c.armor,
    }
}

pub(super) fn tick(game: &mut super::Game<'_>) {
    // Calm the last deaths' items: they stay where they fell, and stay.
    let pending: Vec<(u32, u32)> = std::mem::take(&mut *CALM.lock().unwrap_or_else(|e| e.into_inner()));
    if !pending.is_empty() {
        let c = config();
        for e in game.dropped.iter_mut() {
            if pending.iter().any(|&(first, n)| e.id >= first && e.id < first + n) {
                if c.no_splatter {
                    e.velocity.x = 0.0;
                    e.velocity.z = 0.0;
                    e.velocity.y = e.velocity.y.min(0.0);
                }
                if c.no_despawn {
                    e.age = i16::MIN as i32;
                }
            }
        }
    }
    let Some(host) = game.lan.as_ref() else { return };
    let mut layouts = LAYOUTS.lock().unwrap_or_else(|e| e.into_inner());
    let layouts = layouts.get_or_insert_with(HashMap::new);
    layouts.clear();
    for g in &host.hub.guests {
        layouts.insert(g.uuid, layout(&g.inventory, &g.offhand, &g.armor));
    }
}

pub(super) fn player_drops(game: &mut super::Game<'_>, uuid: u128, drops: &mut Vec<ItemStack>) {
    let layout = LAYOUTS
        .lock()
        .ok()
        .and_then(|l| l.as_ref().and_then(|l| l.get(&uuid).cloned()));
    let Some(layout) = layout else {
        log::warn!("[simple_death] no slot snapshot for the dying player: vanilla drops");
        return;
    };
    // The drops must be exactly the snapshot's stacks, in order; anything else
    // (the inventory changed this tick) leaves vanilla's drops alone.
    if layout.len() != drops.len()
        || layout.iter().zip(drops.iter()).any(|((_, id), s)| *id != *s.item)
    {
        log::warn!("[simple_death] inventory changed since the snapshot: vanilla drops");
        return;
    }
    let mut thrown = Vec::new();
    for ((slot, _), stack) in layout.into_iter().zip(std::mem::take(drops)) {
        if !kept(slot) {
            thrown.push(stack);
            continue;
        }
        match slot {
            Slot::Inventory(i) => {
                if let Some(s) = game.inventory.get_mut(i) {
                    *s = Some(stack);
                }
            }
            Slot::Offhand => game.offhand = Some(stack),
            Slot::Armor(i) => game.armor[i] = Some(stack),
        }
    }
    *drops = thrown;
    game.apply_equipment();
    // drop_inventory spawns them next, one entity id each, after next_entity.
    CALM.lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((game.next_entity + 1, drops.len() as u32));
}
