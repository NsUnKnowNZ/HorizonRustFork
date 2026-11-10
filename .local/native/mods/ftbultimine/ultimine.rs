// FTB Ultimine for Horizon: hold a key, and breaking a block breaks the matching blocks joined
// to it too, in the chosen shape, at the cost of hunger and the tool's durability for each.
// Written from FTB Ultimine's documented behaviour (FTB's mod is not copied):
//  - The key (` by default) is held while mining; the blocks that will go are outlined, and the
//    shape and count are shown at the top left. Scrolling while it is held changes the shape.
//  - Shapes: Shapeless (the matching blocks joined to it, corners included), Small Tunnel (1x1
//    into the face), Small Square (3x3 on the face), Large Tunnel (3x3 into the face), Mining
//    Tunnel (stairs down, two high) and Escape Tunnel (stairs up). Only blocks of the same kind
//    as the first one go, at most 64 together.
//  - Right-clicking a grown crop with the key held harvests the grown crops of its kind joined
//    to it and plants each again (one seed of its drops goes back in the ground).
//  - Each extra block is broken as the player breaks one (drops, experience, statistics, the
//    tool's wear) and costs 0.1 exhaustion (FTB's 20 x vanilla's 0.005). It stops when the tool
//    breaks or, out of creative, when the food bar is empty.
// The server half runs on Horizon servers (single player, LAN, dedicated): the client tells it
// whether the key is held and which shape, on the channel `ftbultimine:state`.
use super::Game;
use horizon_client::{hud::HudBatch, text::Text};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet, VecDeque},
    sync::Mutex,
};

pub const ID: &str = "ftbultimine";
pub const VERSION: &str = "2101.1.0";
pub const DESCRIPTION: &str =
    "Hold the Ultimine key while mining to break the matching blocks joined to it. Scroll while holding it to change the shape.";
pub const KEY: &str = "use";
pub const DEFAULT_KEY: &str = "key.keyboard.grave.accent";
const CHANNEL: &str = "ftbultimine:state";
/// `max_blocks`: the blocks one break may take, the first included.
pub const MAX_BLOCKS: usize = 64;
/// `exhaustion_per_block` (20) times the exhaustion of breaking a block (0.005).
const EXHAUSTION: f32 = 20.0 * 0.005;

pub const SHAPES: [&str; 6] = ["Shapeless", "Small Tunnel", "Small Square", "Large Tunnel", "Mining Tunnel", "Escape Tunnel"];

// --- Selection (both halves) -----------------------------------------------------------------

/// The blocks a break at `origin` takes in `shape`: `face` the clicked face (out of the block),
/// `look` the horizontal way the player faces (for the stairs), `same` whether a block is one
/// of the kind of the first. The first is always in, then nearest first, `MAX_BLOCKS` at most.
pub fn select(origin: [i32; 3], face: [i32; 3], look: [i32; 3], shape: usize, same: &dyn Fn([i32; 3]) -> bool) -> Vec<[i32; 3]> {
    let add = |a: [i32; 3], b: [i32; 3], n: i32| [a[0] + b[0] * n, a[1] + b[1] * n, a[2] + b[2] * n];
    let mut out = vec![origin];
    let mut seen: HashSet<[i32; 3]> = HashSet::from([origin]);
    let mut take = |p: [i32; 3], out: &mut Vec<[i32; 3]>| -> bool {
        if out.len() >= MAX_BLOCKS {
            return false;
        }
        if seen.insert(p) && same(p) {
            out.push(p);
        }
        true
    };
    // Into the wall, and the two axes across it.
    let into = face.map(|v| -v);
    let (a, b) = if face[1] != 0 { ([1, 0, 0], [0, 0, 1]) } else if face[0] != 0 { ([0, 1, 0], [0, 0, 1]) } else { ([1, 0, 0], [0, 1, 0]) };
    match shape {
        // Shapeless: a walk over the joined matching blocks (all 26 neighbours).
        0 => {
            let mut queue = VecDeque::from([origin]);
            while let Some(p) = queue.pop_front() {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if (dx, dy, dz) == (0, 0, 0) {
                                continue;
                            }
                            let q = [p[0] + dx, p[1] + dy, p[2] + dz];
                            let before = out.len();
                            if !take(q, &mut out) {
                                return out;
                            }
                            if out.len() > before {
                                queue.push_back(q);
                            }
                        }
                    }
                }
            }
        }
        // Small Tunnel: a line into the face, while it matches.
        1 => {
            for k in 1..MAX_BLOCKS as i32 {
                let p = add(origin, into, k);
                let before = out.len();
                if !take(p, &mut out) || out.len() == before {
                    break;
                }
            }
        }
        // Small Square: the 3x3 on the face.
        2 => {
            for i in -1..=1 {
                for j in -1..=1 {
                    take(add(add(origin, a, i), b, j), &mut out);
                }
            }
        }
        // Large Tunnel: 3x3 slices into the face, while a slice has one.
        3 => {
            for k in 0..MAX_BLOCKS as i32 {
                let before = out.len();
                for i in -1..=1 {
                    for j in -1..=1 {
                        if !take(add(add(add(origin, into, k), a, i), b, j), &mut out) {
                            return out;
                        }
                    }
                }
                if k > 0 && out.len() == before {
                    break;
                }
            }
        }
        // Mining and Escape Tunnels: stairs two high, down or up the way the player faces.
        _ => {
            let step = if shape == 4 { -1 } else { 1 };
            for k in 0..MAX_BLOCKS as i32 {
                let base = add(add(origin, look, k), [0, step, 0], k);
                let before = out.len();
                for h in 0..2 {
                    if !take(add(base, [0, 1, 0], h - if step < 0 { 1 } else { 0 }), &mut out) {
                        return out;
                    }
                }
                if k > 0 && out.len() == before {
                    break;
                }
            }
        }
    }
    out
}

/// The horizontal way a yaw faces (`Direction.fromYRot`).
fn look_of(yaw: f32) -> [i32; 3] {
    let r = yaw.to_radians();
    let (x, z) = (-(r.sin()), r.cos());
    if x.abs() > z.abs() { [x.signum() as i32, 0, 0] } else { [0, 0, z.signum() as i32] }
}

/// The face a ray along `dir` meets first on a block: the axis it moves most along, against it.
fn face_of(dir: [f64; 3]) -> [i32; 3] {
    let m = (0..3).max_by(|&i, &j| dir[i].abs().total_cmp(&dir[j].abs())).unwrap_or(1);
    let mut f = [0; 3];
    f[m] = if dir[m] > 0.0 { -1 } else { 1 };
    f
}

fn block_name(game: &Game<'_>, p: [i32; 3]) -> std::sync::Arc<str> {
    game.res.facts().get(game.world.block(p[0], p[1], p[2])).name.clone()
}

/// Whether a block can go with the others: the same kind, and not one that can't be broken.
fn matcher<'a>(game: &'a Game<'_>, origin: [i32; 3]) -> impl Fn([i32; 3]) -> bool + 'a {
    let first = block_name(game, origin);
    move |p: [i32; 3]| {
        if !game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) {
            return false;
        }
        let facts = game.res.facts().get(game.world.block(p[0], p[1], p[2]));
        facts.name == first && facts.hardness >= 0.0
    }
}

// --- Client ------------------------------------------------------------------------------------

struct Client {
    held: bool,
    shape: usize,
    sent: Option<(bool, usize)>,
    resend: u32,
    /// The blocks the outline shows, and the block they were found from.
    selection: Vec<[i32; 3]>,
}
static CLIENT: Mutex<Client> = Mutex::new(Client { held: false, shape: 0, sent: None, resend: 0, selection: Vec::new() });
fn client<T>(f: impl FnOnce(&mut Client) -> T) -> T {
    f(&mut CLIENT.lock().unwrap_or_else(|e| e.into_inner()))
}

/// `native_key`: the Ultimine key went down or up.
pub(crate) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) {
        return;
    }
    if key == game.native_mod_key(ID, KEY, DEFAULT_KEY) {
        client(|c| c.held = pressed && game.native_playing());
    }
}

/// `native_wheel`: scrolling while the key is held picks the shape.
pub(crate) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if !game.native_on(ID) || steps == 0.0 || !client(|c| c.held) {
        return;
    }
    *claimed = true;
    client(|c| {
        let n = SHAPES.len() as i32;
        let by = if steps > 0.0 { -1 } else { 1 };
        c.shape = (c.shape as i32 + by).rem_euclid(n) as usize;
    });
}

/// `native_client_tick_post`: the state to the server, and the outline's blocks.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    if !game.native_on(ID) || game.player.is_none() {
        client(|c| {
            c.held = false;
            c.selection.clear();
        });
        return;
    }
    if !game.native_playing() {
        client(|c| c.held = false);
    }
    let (held, shape, send) = client(|c| {
        c.resend += 1;
        let state = (c.held, c.shape);
        // Again now and then while held, in case the server came in late.
        let send = c.sent != Some(state) || (c.held && c.resend % 40 == 0);
        (c.held, c.shape, send)
    });
    if send && game.native_net_send_to_server(CHANNEL, &[u8::from(held), shape as u8]).is_ok() {
        client(|c| c.sent = Some((held, shape)));
    }
    let selection = if held { aimed_selection(game, shape) } else { Vec::new() };
    client(|c| c.selection = selection);
}

/// The blocks a break at the crosshair would take.
fn aimed_selection(game: &Game<'_>, shape: usize) -> Vec<[i32; 3]> {
    let Some(eye) = game.eye_position() else {
        return Vec::new();
    };
    let dir = super::view_vector(game.yaw, game.pitch);
    let reach = if game.creative { 5.0 } else { 4.5 };
    let Some(hit) = game.clip_ray(eye, dir, reach) else {
        return Vec::new();
    };
    let state = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
    if game.res.facts().is_air(state) {
        return Vec::new();
    }
    let same = matcher(game, hit.pos);
    select(hit.pos, hit.face, look_of(game.yaw), shape, &same)
}

/// `native_world_primitives`: an outline round each block that will go.
pub(crate) fn primitives(game: &Game<'_>, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    if game.ui.hide_gui {
        return;
    }
    let selection = client(|c| c.selection.clone());
    for p in selection {
        let lo = [f64::from(p[0]) - 0.002, f64::from(p[1]) - 0.002, f64::from(p[2]) - 0.002];
        let hi = [lo[0] + 1.004, lo[1] + 1.004, lo[2] + 1.004];
        primitives.box_outline(lo, hi, 0xCCFF_FFFF, 2.0);
    }
}

/// `native_hud`: the shape and the count at the top left while the key is held.
pub(crate) fn hud(game: &Game<'_>, batch: &mut HudBatch) {
    let (held, shape, count) = client(|c| (c.held, c.shape, c.selection.len()));
    if !held || game.ui.hide_gui {
        return;
    }
    let lines = [
        (Text::literal(format!("FTB Ultimine: {}", SHAPES[shape])), 0xFFFF_FFFF_u32),
        (Text::literal(format!("{count} block{}", if count == 1 { "" } else { "s" })), 0xFFAA_AAAA),
        (Text::literal("Scroll to change shape"), 0xFF77_7777),
    ];
    for (i, (t, argb)) in lines.iter().enumerate() {
        let [a, r, g, b] = argb.to_be_bytes();
        batch.text_styled(&game.gui.atlas, t, 4.0, 4.0 + i as f32 * 10.0, horizon_client::hud::rgba(r, g, b, a), true);
    }
}

// --- Server ------------------------------------------------------------------------------------

/// Each player's key and shape, as their client last said.
static PLAYERS: Mutex<Option<HashMap<u128, (bool, usize)>>> = Mutex::new(None);

thread_local! {
    /// Inside a break this mod started: the breaks it makes are not taken further.
    static BREAKING: Cell<bool> = const { Cell::new(false) };
}

/// `native_net_from_client`: a client's key and shape.
pub(crate) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || game.remote.is_some() || data.len() < 2 {
        return;
    }
    let state = (data[0] != 0, usize::from(data[1]).min(SHAPES.len() - 1));
    PLAYERS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(player, state);
}

/// `native_player_logged_out`.
pub(crate) fn logged_out(player: u128) {
    if let Some(p) = PLAYERS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        p.remove(&player);
    }
}

/// `native_destroy_block`: a player breaks a block with the key held; the others go first,
/// each as the player breaks a block, then the game breaks the first.
pub(crate) fn destroy_block(game: &mut Game<'_>, pos: [i32; 3]) {
    if game.remote.is_some() || BREAKING.with(Cell::get) {
        return;
    }
    let Some(actor) = game.native_acting_player() else {
        return;
    };
    let Some((true, shape)) = PLAYERS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|p| p.get(&actor.uuid).copied()) else {
        return;
    };
    // The face the player is looking at: the one the ray from their eye meets.
    let eye = [actor.position[0], actor.position[1] + f64::from(actor.eye_height), actor.position[2]];
    let dir = super::view_vector(actor.yaw, actor.pitch);
    let face = match game.clip_ray(eye, dir, 6.0) {
        Some(hit) if hit.pos == pos => hit.face,
        _ => face_of(dir),
    };
    let selection = {
        let same = matcher(game, pos);
        select(pos, face, look_of(actor.yaw), shape, &same)
    };
    let tool = game.held().map(|s| s.item.clone());
    BREAKING.with(|b| b.set(true));
    for p in selection.into_iter().skip(1) {
        let starving = !game.creative && game.player.as_ref().is_some_and(|pl| pl.health.food.food <= 0);
        // The tool broke (or was swapped out): the rest stays.
        if starving || game.held().map(|s| s.item.clone()) != tool {
            break;
        }
        let _ = game.break_block_as_player(p);
        if let Some(pl) = game.player.as_mut() {
            pl.cause_food_exhaustion(EXHAUSTION);
        }
    }
    BREAKING.with(|b| b.set(false));
}

/// The crops a right click harvests, their grown age.
fn grown_age(name: &str) -> Option<&'static str> {
    Some(match name {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => "7",
        "minecraft:beetroots" | "minecraft:nether_wart" => "3",
        "minecraft:cocoa" => "2",
        _ => return None,
    })
}

/// `native_right_click_block`: a grown crop with the key held: the grown crops of its kind
/// joined to it are harvested and planted again.
pub(crate) fn right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if game.remote.is_some() || *canceled || BREAKING.with(Cell::get) {
        return;
    }
    let Some(actor) = game.native_acting_player() else {
        return;
    };
    let Some((true, _)) = PLAYERS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|p| p.get(&actor.uuid).copied()) else {
        return;
    };
    let name = block_name(game, pos);
    let Some(age) = grown_age(&name) else {
        return;
    };
    let grown = |game: &Game<'_>, p: [i32; 3]| block_name(game, p) == name && game.native_block_property(p, "age").as_deref() == Some(age);
    if !grown(game, pos) {
        return;
    }
    *canceled = true;
    let selection = {
        let same = |p: [i32; 3]| game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) && grown(game, p);
        select(pos, [0, 1, 0], [0, 0, 1], 0, &same)
    };
    let held = game.held().cloned();
    BREAKING.with(|b| b.set(true));
    for p in selection {
        let state = game.world.block(p[0], p[1], p[2]);
        let seed = game.res.facts().get(state).item.clone();
        let mut drops = game.block_loot_at(p, state, held.as_ref(), None);
        // One seed goes back in the ground.
        if let Some(d) = drops.iter_mut().find(|d| d.item == seed && d.count > 0) {
            d.count -= 1;
        }
        if !game.native_set_block_property(p, "age", "0") {
            continue;
        }
        let at = [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5];
        for d in drops.into_iter().filter(|d| d.count > 0 && &*d.item != "minecraft:air") {
            game.spawn_item(d, at, [0.0, 0.1, 0.0], 10);
        }
        if let Some(pl) = game.player.as_mut() {
            pl.cause_food_exhaustion(EXHAUSTION);
        }
    }
    BREAKING.with(|b| b.set(false));
}
