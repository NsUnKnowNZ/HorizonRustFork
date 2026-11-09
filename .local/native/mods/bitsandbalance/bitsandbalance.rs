// Bits and Balance for Horizon: restocking, biome titles, Cascading and Aerodynamic.
use super::{Game, Mapping};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

// ---- Automatic restock (server side, the acting player's state in place) ----

/// Per player: a slot that will empty (the last block placed), and what the hand held last tick
/// (slot, item, wear left) for a tool that breaks.
#[derive(Default)]
struct Hands {
    pending: Option<(usize, String)>,
    last: Option<(usize, String, u32)>,
}

static HANDS: Mutex<Option<HashMap<u128, Hands>>> = Mutex::new(None);

fn with_hands<R>(f: impl FnOnce(&mut HashMap<u128, Hands>) -> R) -> R {
    let mut held = HANDS.lock().unwrap_or_else(|e| e.into_inner());
    f(held.get_or_insert_with(HashMap::new))
}

pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack) {
    if stack.count != 1 || game.creative {
        return;
    }
    let player = game.native_acting_id();
    let slot = game.selected;
    with_hands(|h| h.entry(player).or_default().pending = Some((slot, stack.item.to_string())));
}

/// Moves the first stack of `item` outside the hotbar's `slot` into it (the main inventory
/// first, then the rest of the hotbar).
fn restock(game: &mut Game<'_>, slot: usize, item: &str) -> bool {
    if game.inventory.get(slot).is_none_or(|s| s.is_some()) {
        return false;
    }
    let order = (9..game.inventory.len()).chain((0..9).filter(|&i| i != slot));
    for i in order {
        if game.inventory.get(i).and_then(Option::as_ref).is_some_and(|s| &*s.item == item) {
            let stack = game.inventory[i].take();
            game.inventory[slot] = stack;
            return true;
        }
    }
    false
}

pub(super) fn player_tick(game: &mut Game<'_>) {
    cascade_tick(game);
    let player = game.native_acting_id();
    let slot = game.selected;
    let (pending, last) = with_hands(|h| {
        let e = h.entry(player).or_default();
        (e.pending.take(), e.last.take())
    });
    if let Some((s, item)) = pending {
        restock(game, s, &item);
    }
    // A tool with one point of wear left that is gone from its slot broke.
    if let Some((s, item, left)) = last
        && s == slot
        && left <= 1
        && game.inventory.get(s).is_some_and(|x| x.is_none())
    {
        restock(game, s, &item);
    }
    let now = game.inventory.get(slot).and_then(Option::as_ref).and_then(|st| {
        let max = game.res.items().get(&st.item)?.max_damage;
        (max > 0).then(|| (slot, st.item.to_string(), max.saturating_sub(st.damage)))
    });
    with_hands(|h| h.entry(player).or_default().last = now);
}

// ---- Cascading (server side) ----

/// Ores waiting to be mined, one a tick, per player (mined in their own tick, as them).
static CASCADE: Mutex<Option<HashMap<u128, VecDeque<[i32; 3]>>>> = Mutex::new(None);
static CASCADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_ore(name: &str) -> bool {
    name.ends_with("_ore") || name == "minecraft:ancient_debris"
}

pub(super) fn destroyed(game: &mut Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId, canceled: &mut bool) {
    if *canceled || CASCADING.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let level = game.held().map_or(0, |s| s.enchantment_level("bitsandbalance:cascading"));
    if level == 0 || game.keys.held(Mapping::Sneak) {
        return;
    }
    let Some(name) = game.native_state_block(state).map(str::to_owned) else { return };
    if !is_ore(&name) {
        return;
    }
    // The ores joined to this one (faces and edges), nearest first, up to 8 a level.
    let limit = 8 * level as usize;
    let mut seen = HashSet::from([pos]);
    let mut queue = VecDeque::from([pos]);
    let mut found = Vec::new();
    while let Some(p) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let q = [p[0] + dx, p[1] + dy, p[2] + dz];
                    if found.len() >= limit || !seen.insert(q) {
                        continue;
                    }
                    let s = game.world.block(q[0], q[1], q[2]);
                    if game.native_state_block(s) == Some(name.as_str()) {
                        found.push(q);
                        queue.push_back(q);
                    }
                }
            }
        }
    }
    let player = game.native_acting_id();
    CASCADE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).entry(player).or_default().extend(found);
}

pub(super) fn level_tick(_game: &mut Game<'_>) {}

/// The acting player's next ore, in their own tick.
fn cascade_tick(game: &mut Game<'_>) {
    let player = game.native_acting_id();
    let next = CASCADE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get_mut(&player)
        .and_then(VecDeque::pop_front);
    let Some(p) = next else { return };
    // Stops when the pickaxe is put away or worn out.
    if game.held().is_none_or(|s| s.enchantment_level("bitsandbalance:cascading") == 0) {
        if let Some(m) = CASCADE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            m.remove(&player);
        }
        return;
    }
    CASCADING.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = game.break_block(p);
    CASCADING.store(false, std::sync::atomic::Ordering::Relaxed);
}

// ---- Aerodynamic (client side: the player's own movement) ----

pub(super) fn client_tick(game: &mut Game<'_>) {
    let level = game.armor[1].as_ref().map_or(0, |s| s.enchantment_level("bitsandbalance:aerodynamic"));
    if level == 0 {
        return;
    }
    let Some(p) = game.player.as_mut() else { return };
    if !p.fall_flying {
        return;
    }
    // Gliding loses about 1% of its speed a tick to drag; this gives back up to two thirds of
    // it, all of it from cloud height (192) up.
    let strength = ((p.position.y - 64.0) / 128.0).clamp(0.25, 1.0);
    let k = 1.0 + 0.0066 * strength;
    p.velocity.x *= k;
    p.velocity.z *= k;
}

// ---- Biome titles (client side) ----

struct Title {
    biome: String,
    since: Instant,
    shown: Option<(String, Instant)>,
}

static TITLE: Mutex<Option<Title>> = Mutex::new(None);

const IN: f32 = 0.6;
const HOLD: f32 = 2.2;
const OUT: f32 = 1.0;

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    let Some(at) = game.player.as_ref().map(|p| [p.position.x, p.position.y + 1.0, p.position.z]) else { return };
    let biome = game.biome_at(at).to_owned();
    let mut held = TITLE.lock().unwrap_or_else(|e| e.into_inner());
    let t = held.get_or_insert_with(|| Title {
        biome: biome.clone(),
        since: Instant::now(),
        // The biome the player starts in isn't announced.
        shown: Some((biome.clone(), Instant::now() - std::time::Duration::from_secs(60))),
    });
    if t.biome != biome {
        t.biome = biome.clone();
        t.since = Instant::now();
    }
    // A biome stood in for a second, and not the one shown last, is announced.
    if t.since.elapsed().as_secs_f32() > 1.0 && t.shown.as_ref().is_none_or(|s| s.0 != t.biome) {
        t.shown = Some((t.biome.clone(), Instant::now()));
    }
    let Some((name, start)) = t.shown.clone() else { return };
    drop(held);
    let e = start.elapsed().as_secs_f32();
    if e > IN + HOLD + OUT {
        return;
    }
    let alpha = if e < IN { e / IN } else if e < IN + HOLD { 1.0 } else { 1.0 - (e - IN - HOLD) / OUT };
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
    if a < 8 {
        return;
    }
    let key = format!("biome.{}", name.replace(':', "."));
    let fallback = name.rsplit(':').next().unwrap_or(&name).replace('_', " ");
    let text = Text::literal(horizon_client::lang::get_or(&key, &fallback));
    let atlas = &game.gui.atlas;
    let w = atlas.font.width_text(&text);
    let scale = 2.0;
    let x = (batch.width / scale - w) / 2.0;
    let y = batch.height / scale * 0.22;
    let start_v = batch.vertices.len();
    batch.text_styled(atlas, &text, x, y, a << 24 | 0x00FF_FFFF, true);
    batch.scale_since(start_v, 0.0, 0.0, scale, scale);
}
