// Hands-On Delight for Horizon, written from its documented and shown behaviour (its page and
// showcase pictures; none of its code): Farmer's Delight's cooking pot and cutting board work in
// place, without a menu.
//  - A right click flies the camera to the station; the hotbar lies along its front and a recipe
//    book beside it lists what can be made from what the player carries (the ones they can make
//    first), page by page. A click on a neighbouring station glides the camera over to it. Escape,
//    E, a movement key or jump flies back.
//  - Cooking pot: the pot is open, its soup taking the meal's colour as it cooks, bubbling and
//    steaming over heat. Click a hotbar item, then the pot, to drop it in (a right click drops
//    one). Stir with the spoon by holding the button and drawing wide, steady rounds in the pot:
//    each round hurries the cooking, and steady rounds fill the stars over the dish, each adding
//    servings. Players nearby see the spoon go round. The meal card beside the pot serves one
//    into its container from the inventory.
//  - Cutting board: put an item on the board from the hotbar; pick up its tool and the cut is
//    marked across it. Drag the tool along the mark to cut; the pieces fly to the hotbar. A click
//    with nothing in hand takes the item back.
use super::farmersdelight as fd;
use super::{Game, Mapping};
use horizon_assets::item_model::DisplayContext;
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::crafting::Ingredient;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{HudMode, NativeScreenView};
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_world::nbt::{Compound, Tag};
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use winit::event::MouseButton;

pub const ID: &str = "handson_delight";
pub const VERSION: &str = "1.1.0";
pub const DESCRIPTION: &str =
    "Farmer's Delight's cooking pot and cutting board worked by hand in the world: stir the soup, cut on the board.";
const CHANNEL: &str = "handson_delight:act";
const ENTER: f32 = 0.45;
const LEAVE: f32 = 0.35;
const FLIGHT: f32 = 0.55;
const SLOT: f32 = 20.0;
const CARD: [f32; 2] = [34.0, 46.0];
const BOOK: [f32; 2] = [180.0, 100.0];
/// The book's recipe grid: columns, rows, cell size and where it starts.
const GRID: (usize, usize, f32, [f32; 2]) = (4, 3, 20.0, [6.0, 8.0]);
/// The soup's level in the open pot, and the inside's half width.
const SOUP: f32 = 0.5;
const INSIDE: f32 = 5.0 / 16.0;

const PUT: u8 = 1;
const TAKE: u8 = 2;
const STIR: u8 = 3;
const SERVE: u8 = 4;
const CUT: u8 = 5;
const TOSS: u8 = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pot,
    Board,
    Skillet,
}

type Pose = ([f64; 3], f32, f32);

/// An item icon flying between the station and the hotbar.
struct Flight {
    item: String,
    from: [f32; 2],
    to: [f32; 2],
    start: Instant,
}

struct Scene {
    pos: [i32; 3],
    kind: Kind,
    from: Pose,
    to: Pose,
    started: Instant,
    leaving: Option<Instant>,
    /// The hotbar slot carried on the pointer.
    carried: Option<usize>,
    /// Stirring: the last angle round the pot, the turn so far and how far out it went.
    stir: Option<f32>,
    turn: f32,
    reach: Vec<f32>,
    /// Cutting: where the drag started on the board.
    cut_from: Option<[f64; 3]>,
    /// Tossing the skillet: where the drag on its handle started.
    toss_from: Option<[f32; 2]>,
    hud: HudMode,
    logged: bool,
    /// The book's page, and the recipe picked on it (by its place in the station's list).
    page: usize,
    chosen: Option<usize>,
    flights: Vec<Flight>,
}

struct Client {
    scene: Option<Scene>,
    out: Vec<Vec<u8>>,
    close: bool,
    /// The swirl of the pot's contents (radians) and how fast it turns.
    swirl: f32,
    spin: f32,
    last: Option<Instant>,
    /// The pointer on the station's working surface (the soup, the board), in the world.
    pointer: Option<[f64; 3]>,
    /// The screen's size in GUI pixels and the vertical field of view, from the last frame drawn.
    size: [f32; 2],
    fov: f32,
    seed: u64,
}

static CLIENT: Mutex<Client> = Mutex::new(Client {
    scene: None,
    out: Vec::new(),
    close: false,
    swirl: 0.0,
    spin: 0.0,
    last: None,
    pointer: None,
    size: [0.0, 0.0],
    fov: 70.0,
    seed: 0x9E37_79B9_7F4A_7C15,
});

fn client<T>(f: impl FnOnce(&mut Client) -> T) -> T {
    f(&mut CLIENT.lock().unwrap_or_else(|e| e.into_inner()))
}

fn rand(c: &mut Client) -> f32 {
    c.seed ^= c.seed << 13;
    c.seed ^= c.seed >> 7;
    c.seed ^= c.seed << 17;
    (c.seed >> 40) as f32 / (1u64 << 24) as f32
}

// ---- the camera ----

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far the camera is from where it started to the station, 0 to 1 (and back when leaving).
fn progress(s: &Scene) -> f32 {
    let enter = smooth(s.started.elapsed().as_secs_f32() / ENTER);
    match s.leaving {
        Some(at) => enter.min(1.0 - smooth(at.elapsed().as_secs_f32() / LEAVE)),
        None => enter,
    }
}

fn settled(s: &Scene) -> bool {
    s.leaving.is_none() && progress(s) >= 0.999
}

fn lerp_pose(a: Pose, b: Pose, t: f32) -> Pose {
    let td = f64::from(t);
    let p = [0, 1, 2].map(|i| a.0[i] + (b.0[i] - a.0[i]) * td);
    let dyaw = ((b.1 - a.1) % 360.0 + 540.0) % 360.0 - 180.0;
    (p, a.1 + dyaw * t, a.2 + (b.2 - a.2) * t)
}

fn current(s: &Scene) -> Pose {
    lerp_pose(s.from, s.to, progress(s))
}

/// Where the camera frames a station from: the side the player came from, looking down on it.
fn framing(kind: Kind, pos: [i32; 3], yaw: f32) -> Pose {
    let side = (yaw / 90.0).round() * 90.0;
    let (pitch, distance, height) = match kind {
        Kind::Pot => (58.0f32, 1.3, 0.45),
        Kind::Board => (66.0f32, 1.4, 0.06),
        Kind::Skillet => (52.0f32, 1.25, 0.1),
    };
    let f = super::view_vector(side, pitch);
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + height, f64::from(pos[2]) + 0.5];
    ([at[0] - f[0] * distance, at[1] - f[1] * distance, at[2] - f[2] * distance], side, pitch)
}

/// `camera_view`: the station's camera while a scene is on.
pub(crate) fn camera() -> Option<([f64; 3], f32, f32)> {
    client(|c| c.scene.as_ref().map(current))
}

/// The camera's forward, right and up.
fn basis(yaw: f32, pitch: f32) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let f = super::view_vector(yaw, pitch);
    let y = f64::from(yaw).to_radians();
    let r = [-y.cos(), 0.0, -y.sin()];
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    (f, r, u)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A point of the world on the screen, in GUI pixels.
fn project(pose: Pose, fov: f32, size: [f32; 2], p: [f64; 3]) -> Option<[f32; 2]> {
    let (f, r, u) = basis(pose.1, pose.2);
    let d = [p[0] - pose.0[0], p[1] - pose.0[1], p[2] - pose.0[2]];
    let z = dot(d, f);
    if z <= 0.01 {
        return None;
    }
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0] / size[1].max(1.0));
    let x = dot(d, r) / (z * t * aspect);
    let y = dot(d, u) / (z * t);
    Some([((x + 1.0) / 2.0) as f32 * size[0], ((1.0 - y) / 2.0) as f32 * size[1]])
}

/// The ray from the camera through the pointer.
fn pointer_ray(pose: Pose, fov: f32, size: [f32; 2], mouse: [f32; 2]) -> [f64; 3] {
    let (f, r, u) = basis(pose.1, pose.2);
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0] / size[1].max(1.0));
    let nx = f64::from(mouse[0] / size[0].max(1.0)) * 2.0 - 1.0;
    let ny = 1.0 - f64::from(mouse[1] / size[1].max(1.0)) * 2.0;
    [0, 1, 2].map(|i| f[i] + r[i] * nx * t * aspect + u[i] * ny * t)
}

/// The world point under the pointer on the level `y`.
fn pointer_on(pose: Pose, fov: f32, size: [f32; 2], mouse: [f32; 2], y: f64) -> Option<[f64; 3]> {
    let dir = pointer_ray(pose, fov, size, mouse);
    if dir[1].abs() < 1e-6 {
        return None;
    }
    let k = (y - pose.0[1]) / dir[1];
    (k > 0.0).then(|| [pose.0[0] + dir[0] * k, y, pose.0[2] + dir[2] * k])
}

/// Where a ray enters a box, if it does.
fn ray_box(o: [f64; 3], d: [f64; 3], min: [f64; 3], max: [f64; 3]) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, f64::INFINITY);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < min[i] || o[i] > max[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[i] - o[i]) / d[i], (max[i] - o[i]) / d[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far).then_some(near)
}

/// The level of a station's working surface, and its half width.
fn surface(kind: Kind, pos: [i32; 3]) -> (f64, f64) {
    match kind {
        Kind::Pot => (f64::from(pos[1]) + f64::from(SOUP), f64::from(INSIDE)),
        Kind::Board => (f64::from(pos[1]) + 0.0625, 0.45),
        Kind::Skillet => (f64::from(pos[1]) + 0.075, 0.4),
    }
}

fn station_center(pos: [i32; 3], y: f64) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, y, f64::from(pos[2]) + 0.5]
}

fn station_kind(game: &Game<'_>, pos: [i32; 3]) -> Option<Kind> {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    match game.native_state_block(state) {
        Some(fd::POT) => Some(Kind::Pot),
        Some(fd::BOARD) => Some(Kind::Board),
        Some(fd::SKILLET) => Some(Kind::Skillet),
        _ => None,
    }
}

/// The skillet's handle sticks out on the side away from its `facing`; the camera looks from
/// behind it.
fn skillet_side(game: &Game<'_>, pos: [i32; 3]) -> ([f64; 2], f32) {
    // Framed from the side, the handle reaching out to the right of the screen, where the
    // pointer can take it (from the handle's end it pointed at the camera, off the screen).
    let state = game.world.block(pos[0], pos[1], pos[2]);
    match game.native_block_state_property(state, "facing") {
        Some("south") => ([0.0, -1.0], 90.0),
        Some("east") => ([-1.0, 0.0], 0.0),
        Some("west") => ([1.0, 0.0], 180.0),
        _ => ([0.0, 1.0], -90.0),
    }
}

/// The yaw the camera frames a station from.
fn framing_yaw(game: &Game<'_>, kind: Kind, pos: [i32; 3], yaw: f32) -> f32 {
    if kind == Kind::Skillet { skillet_side(game, pos).1 } else { yaw }
}

/// What a client knows of a skillet: its food, how far each side has fried (in ticks), the
/// time a side takes, which side is down, when it was last tossed, and whether it is over heat.
struct SkilletView {
    food: Option<ItemStack>,
    sides: [i32; 2],
    half: i32,
    flipped: bool,
    hot: bool,
}

fn skillet_view(game: &Game<'_>, pos: [i32; 3]) -> SkilletView {
    let Some(nbt) = station_nbt(game, pos) else {
        return SkilletView { food: None, sides: [0, 0], half: 1, flipped: false, hot: false };
    };
    SkilletView {
        food: horizon_items::item_nbt::decode_items(nbt.get(&fd::key("Items")), 1).into_iter().next().flatten(),
        sides: [int(nbt, "SideA"), int(nbt, "SideB")],
        half: (int(nbt, "Total") / 2).max(1),
        flipped: int(nbt, "Flipped") != 0,
        hot: int(nbt, "Hot") != 0,
    }
}

/// The campfire's recipes, which the skillet fries.
fn fries<'a>(game: &'a Game<'_>) -> Vec<&'a horizon_client::crafting::Cooking> {
    game.recipes().cooking.iter().filter(|c| c.kind == "minecraft:campfire_cooking").collect()
}

// ---- what the stations show ----

/// What a client knows of a pot: its items (the ingredients, then the meal), its stars, how far
/// the meal has come (-1 when nothing cooks), whether it is over heat, and when it was last stirred.
struct PotView {
    items: Vec<Option<ItemStack>>,
    stars: i32,
    stage: i32,
    hot: bool,
    stir_at: i32,
}

fn int(nbt: &Compound, k: &str) -> i32 {
    match nbt.get(&fd::key(k)) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn pot_view_of(nbt: &Compound) -> PotView {
    PotView {
        items: horizon_items::item_nbt::decode_items(nbt.get(&fd::key("Items")), fd::MEAL + 1),
        stars: int(nbt, "Stars"),
        stage: if nbt.get(&fd::key("Stage")).is_some() { int(nbt, "Stage") } else { -1 },
        hot: int(nbt, "Hot") != 0,
        stir_at: int(nbt, "StirAt"),
    }
}

fn station_nbt<'a>(game: &'a Game<'_>, pos: [i32; 3]) -> Option<&'a Compound> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) => Some(&m.nbt),
        _ => None,
    }
}

fn pot_view(game: &Game<'_>, pos: [i32; 3]) -> PotView {
    station_nbt(game, pos).map_or_else(
        || PotView { items: vec![None; fd::MEAL + 1], stars: 0, stage: -1, hot: false, stir_at: 0 },
        pot_view_of,
    )
}

/// The board's item and the layers stacked on it, as the server sent them.
fn board_layers(game: &Game<'_>, pos: [i32; 3]) -> Vec<ItemStack> {
    let Some(nbt) = station_nbt(game, pos) else { return Vec::new() };
    horizon_items::item_nbt::decode_items(nbt.get(&fd::key("Items")), fd::BOARD_LAYERS).into_iter().flatten().collect()
}

/// Whether a board holding `layers` takes one of `stack` (as `board_put` decides): an empty one
/// takes anything, bread and the layers on it the next filling of a sandwich.
fn board_takes(game: &Game<'_>, layers: &[ItemStack], stack: &ItemStack) -> bool {
    let Some((base, on)) = layers.split_first() else { return true };
    let one = fd::one(stack);
    let mut fill: Vec<&ItemStack> = on.iter().collect();
    fill.push(&one);
    fd::sandwiches(game).iter().any(|w| w.base.test(base) && fd::partial(&w.fillings, &fill))
}

/// The sandwich the board's stack is on its way to, by its place in the list.
fn sandwich_of(game: &Game<'_>, layers: &[ItemStack]) -> Option<usize> {
    let (base, rest) = layers.split_first()?;
    let rest: Vec<&ItemStack> = rest.iter().collect();
    fd::sandwiches(game).iter().position(|s| s.base.test(base) && fd::partial(&s.fillings, &rest))
}

fn board_item(game: &Game<'_>, pos: [i32; 3]) -> Option<ItemStack> {
    let nbt = station_nbt(game, pos)?;
    horizon_items::item_nbt::decode_items(nbt.get(&fd::key("Items")), 1).into_iter().next().flatten()
}

/// The meal a pot's ingredients make, if they make one.
fn meal_of(game: &Game<'_>, inputs: &[&ItemStack]) -> Option<ItemStack> {
    if inputs.is_empty() {
        return None;
    }
    fd::meals(game).iter().find(|m| fd::matches(&m.inputs, inputs)).map(|m| m.result.clone())
}

/// A meal's soup colour, 0xRRGGBB.
fn soup_colour(item: &str) -> u32 {
    let path = item.rsplit(':').next().unwrap_or(item);
    match path {
        "beef_stew" | "rabbit_stew" | "dog_food" => 0x7A4A2A,
        "chicken_soup" | "fried_rice" => 0xD8B060,
        "vegetable_soup" | "vegetable_noodles" => 0x8A8A3A,
        "fish_stew" | "baked_cod_stew" => 0xC8703A,
        "pumpkin_soup" | "stuffed_pumpkin_block" => 0xE08A2A,
        "tomato_sauce" | "pasta_with_meatballs" | "ratatouille" => 0xC0341E,
        "noodle_soup" | "pasta_with_mutton_chop" => 0xD8B878,
        "bone_broth" | "dumplings" => 0xE8D8B0,
        "mushroom_stew" | "mushroom_rice" => 0xB08A60,
        "beetroot_soup" => 0x9A1A2A,
        "hot_cocoa" => 0x5A3020,
        "apple_cider" => 0xC88A2A,
        "glow_berry_custard" => 0xF0D070,
        "squid_ink_pasta" => 0x202028,
        "cooked_rice" => 0xF0EEE0,
        "cabbage_rolls" => 0x8AB060,
        _ => 0xA07040,
    }
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = ((a >> s) & 0xFF, (b >> s) & 0xFF);
        ((x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// Whether the carried units make the recipe: each ingredient takes one unit, none twice.
fn can_make(ingredients: &[Ingredient], have: &mut [(ItemStack, u32)]) -> bool {
    fn go(i: usize, ingredients: &[Ingredient], have: &mut [(ItemStack, u32)]) -> bool {
        if i == ingredients.len() {
            return true;
        }
        for j in 0..have.len() {
            if have[j].1 > 0 && ingredients[i].test(&have[j].0) {
                have[j].1 -= 1;
                let ok = go(i + 1, ingredients, have);
                have[j].1 += 1;
                if ok {
                    return true;
                }
            }
        }
        false
    }
    go(0, ingredients, have)
}

/// A recipe of the station's book: its place in the station's list, the icon it shows, and
/// whether the player can make it now.
#[derive(Clone)]
struct Entry {
    recipe: usize,
    icon: String,
    ready: bool,
}

/// The book's list, kept a quarter of a second: working it out tests every recipe against the
/// inventory, too much for every frame.
struct BookCache {
    at: Instant,
    kind: Kind,
    pos: [i32; 3],
    entries: Vec<Entry>,
}

static BOOK_CACHE: Mutex<Option<BookCache>> = Mutex::new(None);

fn book_entries_cached(game: &Game<'_>, kind: Kind, pos: [i32; 3]) -> Vec<Entry> {
    let mut cache = BOOK_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = cache.as_ref()
        && c.kind == kind
        && c.pos == pos
        && c.at.elapsed().as_secs_f32() < 0.25
    {
        return c.entries.clone();
    }
    let entries = book_entries(game, kind, pos);
    *cache = Some(BookCache { at: Instant::now(), kind, pos, entries: entries.clone() });
    entries
}

/// After an action the book is worked out again on the next frame.
fn book_changed() {
    *BOOK_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn carried_units(game: &Game<'_>, extra: &[&ItemStack]) -> Vec<(ItemStack, u32)> {
    let mut have: Vec<(ItemStack, u32)> = Vec::new();
    for s in game.inventory.iter().flatten().chain(extra.iter().copied()) {
        match have.iter_mut().find(|(h, _)| h.same_item_same_components(s)) {
            Some(h) => h.1 = (h.1 + s.count).min(6),
            None => have.push((s.clone(), s.count.min(6))),
        }
    }
    have
}

fn book_entries(game: &Game<'_>, kind: Kind, pos: [i32; 3]) -> Vec<Entry> {
    let mut list: Vec<Entry> = match kind {
        Kind::Skillet => Vec::new(),
        Kind::Pot => {
            let view = pot_view(game, pos);
            let inputs: Vec<&ItemStack> = view.items.iter().take(fd::INPUTS).flatten().collect();
            let mut have = carried_units(game, &inputs);
            fd::meals(game)
                .iter()
                .enumerate()
                .map(|(i, m)| Entry { recipe: i, icon: m.result.item.to_string(), ready: can_make(&m.inputs, &mut have) })
                .collect()
        }
        Kind::Board => {
            let on_board = board_item(game, pos);
            let units = carried_units(game, &on_board.iter().collect::<Vec<_>>());
            let has = |i: &Ingredient| units.iter().any(|(s, _)| i.test(s));
            let cuts = fd::cuts(game);
            let mut list: Vec<Entry> = cuts
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let icon = c.input.order.first()?.clone();
                    Some(Entry { recipe: i, icon, ready: has(&c.input) && has(&c.tool) })
                })
                .collect();
            let layers = board_layers(game, pos);
            let mut have = carried_units(game, &layers.iter().collect::<Vec<_>>());
            for (j, w) in fd::sandwiches(game).iter().enumerate() {
                let mut all = vec![w.base.clone()];
                all.extend(w.fillings.iter().cloned());
                list.push(Entry { recipe: cuts.len() + j, icon: w.result.item.to_string(), ready: can_make(&all, &mut have) });
            }
            list
        }
    };
    if kind == Kind::Skillet {
        let units = carried_units(game, &[]);
        list = fries(game)
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let icon = c.ingredient.order.first()?.clone();
                Some(Entry { recipe: i, icon, ready: units.iter().any(|(s, _)| c.ingredient.test(s)) })
            })
            .collect();
    }
    // What can be made comes first.
    list.sort_by_key(|e| !e.ready);
    list
}

/// The recipe the book shows: the one picked, else the one the station points to.
fn shown_recipe(game: &Game<'_>, s: &Scene) -> Option<usize> {
    if s.chosen.is_some() {
        return s.chosen;
    }
    match s.kind {
        Kind::Skillet => {
            let food = skillet_view(game, s.pos).food?;
            fries(game).iter().position(|c| c.ingredient.test(&food))
        }
        Kind::Pot => {
            let view = pot_view(game, s.pos);
            let inputs: Vec<&ItemStack> = view.items.iter().take(fd::INPUTS).flatten().collect();
            if inputs.is_empty() {
                return None;
            }
            let meals = fd::meals(game);
            meals
                .iter()
                .enumerate()
                .filter(|(_, m)| inputs.iter().all(|i| m.inputs.iter().any(|g| g.test(i))))
                .max_by_key(|(_, m)| m.inputs.iter().filter(|g| inputs.iter().any(|i| g.test(i))).count())
                .map(|(i, _)| i)
        }
        Kind::Board => {
            let layers = board_layers(game, s.pos);
            let cuts = fd::cuts(game);
            if layers.len() > 1 || layers.first().is_some_and(|b| !cuts.iter().any(|c| c.input.test(b))) {
                return sandwich_of(game, &layers).map(|j| cuts.len() + j);
            }
            let item = layers.first()?;
            cuts.iter().position(|c| c.input.test(item))
        }
    }
}

// ---- entering and leaving ----

/// `play_mouse`: a right click on a pot or a board (not sneaking) opens its scene.
pub(crate) fn mouse(game: &mut Game<'_>, button: MouseButton, pressed: bool) -> bool {
    if !pressed || button != MouseButton::Right || !game.native_on(ID) || !game.native_playing() {
        return false;
    }
    if game.keys.held(Mapping::Sneak) || client(|c| c.scene.is_some()) {
        return false;
    }
    let Some(eye) = game.eye_position() else { return false };
    let dir = super::view_vector(game.yaw, game.pitch);
    let reach = if game.creative { 5.0 } else { 4.5 };
    let Some(hit) = game.clip_ray(eye, dir, reach) else { return false };
    let Some(kind) = station_kind(game, hit.pos) else { return false };
    if !game.native_mod_screen_open(ID) {
        return false;
    }
    let hud = game.native_hud_mode();
    game.native_set_hud_mode(HudMode::AllHidden);
    let from = (eye, game.yaw, game.pitch);
    let to = framing(kind, hit.pos, framing_yaw(game, kind, hit.pos, game.yaw));
    client(|c| {
        c.scene = Some(Scene {
            pos: hit.pos,
            kind,
            from,
            to,
            started: Instant::now(),
            leaving: None,
            carried: None,
            stir: None,
            turn: 0.0,
            reach: Vec::new(),
            cut_from: None,
            toss_from: None,
            hud,
            logged: false,
            page: 0,
            chosen: None,
            flights: Vec::new(),
        });
        c.close = false;
    });
    true
}

/// Once a client tick: leaving when the screen closed, the station's messages sent, and the
/// bubbles and steam over the hot pots nearby.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    let (close, out, done) = client(|c| {
        let mut done = None;
        if let Some(s) = &mut c.scene {
            if s.leaving.is_none() && (c.close || game.native_mod_screen() != Some(ID)) {
                s.leaving = Some(Instant::now());
                s.carried = None;
            }
            if s.leaving.is_some_and(|at| at.elapsed().as_secs_f32() >= LEAVE) {
                done = Some(s.hud);
            }
        }
        if done.is_some() {
            c.scene = None;
            c.pointer = None;
        }
        (std::mem::take(&mut c.close), std::mem::take(&mut c.out), done)
    });
    if close && game.native_mod_screen() == Some(ID) {
        game.native_mod_screen_close(ID);
    }
    if let Some(hud) = done {
        game.native_set_hud_mode(hud);
    }
    for message in out {
        if let Err(e) = game.native_net_send_to_server(CHANNEL, &message) {
            log::debug!("[handson_delight] not sent: {e}");
        }
    }
    if !game.native_on(ID) {
        return;
    }
    // The recipes are worked out once a world is in, not when a station first opens.
    static WARM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARM.load(std::sync::atomic::Ordering::Relaxed) && game.player.is_some() {
        let ready = !fd::meals(game).is_empty() && !fd::cuts(game).is_empty();
        WARM.store(ready, std::sync::atomic::Ordering::Relaxed);
    }
    let Some(eye) = game.eye_position() else { return };
    let near: Vec<[i32; 3]> = game.world.block_entities.mods_near(eye[0], eye[2], 16.0).map(|p| [p.0, p.1, p.2]).collect();
    let scene_pos = client(|c| c.scene.as_ref().map(|s| s.pos));
    for pos in near {
        let d2 = (0..3).map(|i| (eye[i] - f64::from(pos[i])).powi(2)).sum::<f64>();
        if d2 > 256.0 {
            continue;
        }
        match station_kind(game, pos) {
            Some(Kind::Pot) => {}
            Some(Kind::Skillet) => {
                let view = skillet_view(game, pos);
                if view.hot && view.food.is_some() {
                    // Close up (its scene), a wisp now and then from over the rim, rising out of
                    // sight: smoke a metre from the camera covers the food.
                    let close = scene_pos == Some(pos);
                    let (rate, y, rise) = if close { (0.05, 0.32, 0.06) } else { (0.25, 0.12, 0.02) };
                    let (r, x, z) = client(|c| (rand(c), rand(c), rand(c)));
                    if r < rate {
                        let at = [f64::from(pos[0]) + 0.3 + f64::from(x) * 0.4, f64::from(pos[1]) + y, f64::from(pos[2]) + 0.3 + f64::from(z) * 0.4];
                        game.native_add_particle(&ParticleOptions::simple(ParticleType::Smoke), at, [0.0, rise, 0.0]);
                    }
                }
                continue;
            }
            _ => continue,
        }
        let view = pot_view(game, pos);
        let full = view.items.iter().any(Option::is_some) || scene_pos == Some(pos);
        if !view.hot || !full {
            continue;
        }
        let (r1, r2, x, z) = client(|c| (rand(c), rand(c), rand(c), rand(c)));
        let at = |y: f64| {
            [
                f64::from(pos[0]) + 0.5 + (f64::from(x) - 0.5) * 0.5,
                f64::from(pos[1]) + y,
                f64::from(pos[2]) + 0.5 + (f64::from(z) - 0.5) * 0.5,
            ]
        };
        if r1 < 0.45 {
            game.native_add_particle(&ParticleOptions::simple(ParticleType::BubblePop), at(f64::from(SOUP) + 0.02), [0.0, 0.01, 0.0]);
        }
        if r2 < 0.07 {
            game.native_add_particle(&ParticleOptions::simple(ParticleType::WhiteSmoke), at(0.7), [0.0, 0.02, 0.0]);
        }
    }
}

fn ours(game: &Game<'_>, view: &NativeScreenView<'_>) -> bool {
    view.id == "mod" && game.native_mod_screen() == Some(ID)
}

fn send(c: &mut Client, op: u8, pos: [i32; 3], arg: u8) {
    let mut m = vec![op];
    for v in pos {
        m.extend_from_slice(&v.to_le_bytes());
    }
    m.push(arg);
    c.out.push(m);
}

// ---- the scene's parts on the screen ----

/// Where the hotbar strip's slots start: along the station's front, below it.
fn strip(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (f, _, _) = basis(pose.1, pose.2);
    let h = (f[0] * f[0] + f[2] * f[2]).sqrt().max(1e-6);
    let front = [f64::from(pos[0]) + 0.5 - f[0] / h * 0.5, f64::from(pos[1]) + 0.12, f64::from(pos[2]) + 0.5 - f[2] / h * 0.5];
    let at = project(pose, fov, size, front)?;
    Some([keep(at[0] - SLOT * 4.5, 4.0, size[0] - SLOT * 9.0 - 4.0), keep(at[1], 4.0, size[1] - SLOT - 20.0) - SLOT / 2.0])
}

fn strip_slot(origin: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    let x = mouse[0] - origin[0];
    let y = mouse[1] - origin[1];
    (x >= 0.0 && y >= 0.0 && x < SLOT * 9.0 && y < SLOT).then(|| (x / SLOT) as usize)
}

fn slot_center(origin: [f32; 2], slot: usize) -> [f32; 2] {
    [origin[0] + slot as f32 * SLOT + SLOT / 2.0, origin[1] + SLOT / 2.0]
}

/// The meal card beside the pot (its top left), on the camera's right.
fn card(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (_, r, _) = basis(pose.1, pose.2);
    let p = [f64::from(pos[0]) + 0.5 + r[0] * 0.78, f64::from(pos[1]) + 0.75, f64::from(pos[2]) + 0.5 + r[2] * 0.78];
    project(pose, fov, size, p).map(|a| [a[0] - CARD[0] / 2.0, a[1] - CARD[1] / 2.0])
}

/// The recipe book's top left, on the camera's left, kept on the screen.
fn book(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3], kind: Kind) -> Option<[f32; 2]> {
    let (_, r, _) = basis(pose.1, pose.2);
    let height = if kind == Kind::Pot { 0.55 } else { 0.3 };
    let p = [f64::from(pos[0]) + 0.5 - r[0] * 1.1, f64::from(pos[1]) + height, f64::from(pos[2]) + 0.5 - r[2] * 1.1];
    project(pose, fov, size, p).map(|a| {
        [keep(a[0] - BOOK[0] / 2.0, 8.0, size[0] - BOOK[0] - 8.0), keep(a[1] - BOOK[1] / 2.0, 8.0, size[1] - BOOK[1] - 40.0)]
    })
}

/// `v` kept between `lo` and `hi` (`lo` wins on a screen too small for both).
fn keep(v: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi).max(lo)
}

fn inside(at: [f32; 2], wh: [f32; 2], m: [f32; 2]) -> bool {
    m[0] >= at[0] && m[1] >= at[1] && m[0] < at[0] + wh[0] && m[1] < at[1] + wh[1]
}

/// The book's parts: a grid cell, the page arrows.
fn grid_cell(at: [f32; 2], i: usize) -> [f32; 2] {
    let (cols, _, cell, start) = GRID;
    [at[0] + start[0] + (i % cols) as f32 * cell, at[1] + start[1] + (i / cols) as f32 * cell]
}
const PER_PAGE: usize = GRID.0 * GRID.1;
fn arrow_back(at: [f32; 2]) -> [f32; 2] {
    [at[0] + 6.0, at[1] + BOOK[1] - 16.0]
}
fn arrow_next(at: [f32; 2]) -> [f32; 2] {
    [at[0] + BOOK[0] / 2.0 - 20.0, at[1] + BOOK[1] - 16.0]
}

// ---- drawing the scene ----

pub(crate) fn screen_draw(game: &Game<'_>, batch: &mut HudBatch, view: &NativeScreenView<'_>) {
    if !ours(game, view) {
        return;
    }
    let fov = game.native_camera().map_or(70.0, |c| c.fov_degrees);
    let size = [batch.width, batch.height];
    let mouse = view.mouse;
    let Some((pose, pos, kind, carried, entered)) = client(|c| {
        c.size = size;
        c.fov = fov;
        let s = c.scene.as_ref()?;
        Some((current(s), s.pos, s.kind, s.carried, settled(s)))
    }) else {
        return;
    };
    if !entered {
        return;
    }
    // The pointer on the working surface: the spoon and the tool follow it.
    let (y, half) = surface(kind, pos);
    let c0 = station_center(pos, y);
    let pointer = pointer_on(pose, fov, size, mouse, y)
        .filter(|p| (p[0] - c0[0]).abs() < half + 0.2 && (p[2] - c0[2]).abs() < half + 0.2);
    client(|c| c.pointer = pointer);
    let strip_at = strip(pose, fov, size, pos);
    // Once a scene, where its parts landed on the screen (for tests that click them).
    if client(|c| c.scene.as_mut().is_some_and(|s| !std::mem::replace(&mut s.logged, true))) {
        let s = strip_at.unwrap_or_default();
        let k = card(pose, fov, size, pos).unwrap_or_default();
        let b = book(pose, fov, size, pos, kind).unwrap_or_default();
        let m = project(pose, fov, size, c0).unwrap_or_default();
        let h = if kind == Kind::Skillet { handle_point(game, pose, fov, size, pos).unwrap_or_default() } else { [0.0, 0.0] };
        log::info!(
            "[handson_delight] scene at strip {:.0} {:.0} card {:.0} {:.0} middle {:.0} {:.0} book {:.0} {:.0} handle {:.0} {:.0}",
            s[0] + SLOT / 2.0, s[1] + SLOT / 2.0, k[0] + CARD[0] / 2.0, k[1] + CARD[1] / 2.0, m[0], m[1], b[0], b[1], h[0], h[1]
        );
    }
    // The hotbar along the front.
    if let Some(o) = strip_at {
        game.native_fill(batch, [o[0] - 3.0, o[1] - 3.0, SLOT * 9.0 + 6.0, SLOT + 6.0], 0xD0281C10);
        for i in 0..9 {
            let at = [o[0] + i as f32 * SLOT, o[1]];
            let hover = strip_slot(o, mouse) == Some(i);
            let color = if carried == Some(i) {
                0xF0D8A848
            } else if hover {
                0xE0806450
            } else {
                0xE0503C2C
            };
            game.native_fill(batch, [at[0] + 1.0, at[1] + 1.0, SLOT - 2.0, SLOT - 2.0], color);
            if carried != Some(i)
                && let Some(Some(stack)) = game.inventory.get(i)
            {
                draw_stack(game, batch, stack, [at[0] + 2.0, at[1] + 2.0]);
            }
        }
    }
    match kind {
        Kind::Pot => draw_pot_parts(game, batch, pose, fov, size, pos, mouse),
        Kind::Board => {}
        Kind::Skillet => draw_skillet_parts(game, batch, pose, fov, size, pos, mouse),
    }
    draw_book_parts(game, batch, pose, fov, size, pos, kind, mouse);
    // Items flying between the station and the hotbar.
    client(|c| {
        if let Some(s) = c.scene.as_mut() {
            s.flights.retain(|f| f.start.elapsed().as_secs_f32() < FLIGHT);
            for f in &s.flights {
                let t = smooth(f.start.elapsed().as_secs_f32() / FLIGHT);
                let arc = (t * std::f32::consts::PI).sin() * 36.0;
                let at = [f.from[0] + (f.to[0] - f.from[0]) * t - 8.0, f.from[1] + (f.to[1] - f.from[1]) * t - 8.0 - arc];
                game.native_item(batch, &f.item, at, 16.0);
            }
        }
    });
    // The carried stack on the pointer, unless it is in the world under it (a tool on the board).
    let held_in_world = kind == Kind::Board && pointer.is_some();
    if let Some(slot) = carried
        && !held_in_world
        && let Some(Some(stack)) = game.inventory.get(slot)
    {
        draw_stack(game, batch, stack, [mouse[0] - 8.0, mouse[1] - 8.0]);
    }
    let hint = match kind {
        Kind::Pot => "Pick an item and click the pot to add it  -  hold and circle in the pot to stir  -  Esc leaves",
        Kind::Board => "Put food on the board, stack fillings on bread  -  drag a knife along the mark to cut  -  Esc leaves",
        Kind::Skillet => "Put food in the pan  -  flick the handle up to toss it over  -  Esc leaves",
    };
    let w = hint.len() as f32 * 5.0;
    game.native_fill(batch, [(size[0] - w) / 2.0 - 6.0, size[1] - 17.0, w + 12.0, 13.0], 0x80000000);
    game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 15.0], 0xE8E8E8, true);
}

/// The skillet's handle end, where a drag tosses it.
fn handle_point(game: &Game<'_>, pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (h, _) = skillet_side(game, pos);
    let p = [f64::from(pos[0]) + 0.5 + h[0] * 0.85, f64::from(pos[1]) + 0.12, f64::from(pos[2]) + 0.5 + h[1] * 0.85];
    project(pose, fov, size, p)
}

#[allow(clippy::too_many_arguments)]
fn draw_skillet_parts(game: &Game<'_>, batch: &mut HudBatch, pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3], mouse: [f32; 2]) {
    let view = skillet_view(game, pos);
    // The handle: flick it up to toss.
    if let Some(h) = handle_point(game, pose, fov, size, pos) {
        let near = (mouse[0] - h[0]).hypot(mouse[1] - h[1]) < 28.0;
        let colour = if near { 0xE0FFD040 } else { 0x80FFFFFF };
        game.native_fill(batch, [h[0] - 1.0, h[1] - 26.0, 2.0, 18.0], colour);
        game.native_fill(batch, [h[0] - 4.0, h[1] - 26.0, 8.0, 3.0], colour);
    }
    let Some(food) = &view.food else { return };
    // The two sides: how far each has fried, the one down marked.
    let Some(a) = project(pose, fov, size, station_center(pos, f64::from(pos[1]) + 0.55)) else { return };
    for (i, side) in view.sides.iter().enumerate() {
        let down = (i == 1) == view.flipped;
        let x = a[0] - 42.0 + i as f32 * 46.0;
        let y = a[1];
        let done = (*side as f32 / view.half as f32).clamp(0.0, 1.0);
        game.native_fill(batch, [x, y, 38.0, 7.0], 0xC0201810);
        let fill = if done >= 1.0 { 0xFF60C040 } else if down && view.hot { 0xFFE09030 } else { 0xFFB08050 };
        game.native_fill(batch, [x + 1.0, y + 1.0, 36.0 * done, 5.0], fill);
        if down {
            game.native_text(batch, "v", [x + 16.0, y - 9.0], 0xFFFFFF, true);
        }
    }
    let _ = food;
}

fn draw_stack(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, at: [f32; 2]) {
    game.native_item(batch, &stack.item, at, 16.0);
    if stack.count > 1 {
        let n = stack.count.to_string();
        game.native_text(batch, &n, [at[0] + 17.0 - n.len() as f32 * 6.0, at[1] + 9.0], 0xFFFFFF, true);
    }
}

fn first_item(ingredient: &Ingredient) -> Option<&str> {
    ingredient.order.first().map(String::as_str)
}

fn name_of(game: &Game<'_>, item: &str) -> String {
    horizon_client::stations::hover_name(&game.pack.names, &ItemStack::new(item, 1))
}

/// `text` cut to `max` characters, with an ellipsis when it was longer.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_pot_parts(game: &Game<'_>, batch: &mut HudBatch, pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3], mouse: [f32; 2]) {
    let view = pot_view(game, pos);
    // The stars over the dish, and what they add.
    if let Some(a) = project(pose, fov, size, station_center(pos, f64::from(pos[1]) + 0.95)) {
        for i in 0..3 {
            let x = a[0] - 33.0 + i as f32 * 22.0;
            let color = if i < view.stars { 0xFFFFD040 } else { 0xA0303030 };
            game.native_fill(batch, [x + 6.0, a[1], 6.0, 18.0], color);
            game.native_fill(batch, [x, a[1] + 6.0, 18.0, 6.0], color);
            game.native_fill(batch, [x + 3.0, a[1] + 3.0, 12.0, 12.0], color);
        }
        let bonus = format!("+{}%", view.stars * 25);
        game.native_text(batch, &bonus, [a[0] + 36.0, a[1] + 5.0], if view.stars > 0 { 0xFFD040 } else { 0xA0A0A0 }, true);
    }
    // The meal card: the meal waiting in the pot, served by a click.
    let meal = view.items.get(fd::MEAL).cloned().flatten();
    if let Some(at) = card(pose, fov, size, pos) {
        let hover = inside(at, CARD, mouse);
        game.native_fill(batch, [at[0], at[1], CARD[0], CARD[1]], if hover && meal.is_some() { 0xF0806040 } else { 0xE0302820 });
        game.native_fill(batch, [at[0] + 2.0, at[1] + 2.0, CARD[0] - 4.0, CARD[1] - 4.0], 0xE0403428);
        if let Some(meal) = &meal {
            draw_stack(game, batch, meal, [at[0] + 9.0, at[1] + 6.0]);
            if let Some(container) = fd::meal_container(game, meal) {
                game.native_item(batch, &container, [at[0] + 11.0, at[1] + 28.0], 12.0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_book_parts(game: &Game<'_>, batch: &mut HudBatch, pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3], kind: Kind, mouse: [f32; 2]) {
    let Some(at) = book(pose, fov, size, pos, kind) else { return };
    // The open book: covers, two pages and the spine.
    game.native_fill(batch, [at[0] - 4.0, at[1] - 4.0, BOOK[0] + 8.0, BOOK[1] + 8.0], 0xFF8A5A18);
    game.native_fill(batch, [at[0] - 2.0, at[1] - 2.0, BOOK[0] + 4.0, BOOK[1] + 4.0], 0xFFC8901E);
    game.native_fill(batch, [at[0], at[1], BOOK[0], BOOK[1]], 0xFFF6ECD4);
    game.native_fill(batch, [at[0] + BOOK[0] / 2.0 - 1.0, at[1], 2.0, BOOK[1]], 0xFFD6C4A0);
    game.native_fill(batch, [at[0] + BOOK[0] - 10.0, at[1] - 4.0, 5.0, 14.0], 0xFFB03020);
    let entries = book_entries_cached(game, kind, pos);
    let (page, chosen) = client(|c| c.scene.as_ref().map_or((0, None), |s| (s.page, shown_recipe(game, s))));
    let pages = entries.len().div_ceil(PER_PAGE).max(1);
    let page = page.min(pages - 1);
    // The left page: what the station can make, the ones the player can make first.
    for (i, e) in entries.iter().skip(page * PER_PAGE).take(PER_PAGE).enumerate() {
        let cell = grid_cell(at, i);
        let picked = chosen == Some(e.recipe);
        let hover = inside(cell, [GRID.2, GRID.2], mouse);
        let bg = match (picked, e.ready) {
            (true, _) => 0xFFE8C060,
            (false, true) => if hover { 0xFFDCE8B8 } else { 0xFFE6EEC8 },
            (false, false) => if hover { 0xFFE0D4BC } else { 0xFFEADCC4 },
        };
        game.native_fill(batch, [cell[0], cell[1], GRID.2 - 2.0, GRID.2 - 2.0], bg);
        game.native_item(batch, &e.icon, [cell[0] + 1.0, cell[1] + 1.0], 16.0);
        if !e.ready {
            game.native_fill(batch, [cell[0], cell[1], GRID.2 - 2.0, GRID.2 - 2.0], 0x70F6ECD4);
        }
    }
    let (back, next) = (arrow_back(at), arrow_next(at));
    if page > 0 {
        game.native_text(batch, "<", [back[0] + 3.0, back[1] + 2.0], 0x6A4A20, false);
    }
    if page + 1 < pages {
        game.native_text(batch, ">", [next[0] + 5.0, next[1] + 2.0], 0x6A4A20, false);
    }
    let label = format!("{}/{}", page + 1, pages);
    game.native_text(batch, &label, [at[0] + BOOK[0] / 4.0 - label.len() as f32 * 3.0, back[1] + 2.0], 0x8A6A40, false);
    // The right page: the recipe shown.
    let x = at[0] + BOOK[0] / 2.0 + 6.0;
    let Some(recipe) = chosen else {
        let hint = match kind {
            Kind::Pot => "Add ingredients",
            Kind::Board => "Put food on the board",
            Kind::Skillet => "Put food in the pan",
        };
        game.native_text(batch, hint, [x, at[1] + 44.0], 0x8A6A40, false);
        return;
    };
    match kind {
        Kind::Skillet => {
            let fries = fries(game);
            let Some(c) = fries.get(recipe) else { return };
            if let Some(item) = first_item(&c.ingredient) {
                game.native_item(batch, item, [x, at[1] + 8.0], 20.0);
            }
            game.native_text(batch, ">", [x + 26.0, at[1] + 14.0], 0x8A6A40, false);
            game.native_item(batch, &c.result.item, [x + 38.0, at[1] + 6.0], 24.0);
            game.native_text(batch, &fit(&name_of(game, &c.result.item), 13), [x, at[1] + 36.0], 0x40301A, false);
            let side = format!("{}s a side", (c.time / 2).max(1) / 20);
            game.native_text(batch, &side, [x, at[1] + 52.0], 0x8A6A40, false);
            game.native_text(batch, "Toss to turn it", [x, at[1] + 66.0], 0x8A6A40, false);
        }
        Kind::Pot => {
            let meals = fd::meals(game);
            let Some(m) = meals.get(recipe) else { return };
            let view = pot_view(game, pos);
            let inputs: Vec<&ItemStack> = view.items.iter().take(fd::INPUTS).flatten().collect();
            game.native_item(batch, &m.result.item, [x, at[1] + 6.0], 24.0);
            game.native_text(batch, &fit(&name_of(game, &m.result.item), 13), [x, at[1] + 34.0], 0x40301A, false);
            for (i, g) in m.inputs.iter().enumerate() {
                let cell = [x + (i % 4) as f32 * 19.0, at[1] + 46.0 + (i / 4) as f32 * 19.0];
                let have = inputs.iter().any(|s| g.test(s));
                game.native_fill(batch, [cell[0], cell[1], 18.0, 18.0], if have { 0x8060B040 } else { 0x40806040 });
                if let Some(item) = first_item(g) {
                    game.native_item(batch, item, [cell[0] + 1.0, cell[1] + 1.0], 16.0);
                }
            }
            let mut foot = format!("{}s", m.time / 20);
            if let Some(container) = &m.container {
                game.native_item(batch, container, [x + 60.0, at[1] + BOOK[1] - 18.0], 14.0);
                foot = format!("{foot}  in");
            }
            game.native_text(batch, &foot, [x, at[1] + BOOK[1] - 14.0], 0x8A6A40, false);
        }
        Kind::Board if recipe >= fd::cuts(game).len() => {
            let sandwiches = fd::sandwiches(game);
            let Some(w) = sandwiches.get(recipe - fd::cuts(game).len()) else { return };
            let layers = board_layers(game, pos);
            game.native_item(batch, &w.result.item, [x, at[1] + 6.0], 24.0);
            game.native_text(batch, &fit(&name_of(game, &w.result.item), 13), [x, at[1] + 34.0], 0x40301A, false);
            let parts: Vec<&Ingredient> = std::iter::once(&w.base).chain(w.fillings.iter()).collect();
            for (i, g) in parts.iter().enumerate() {
                let cell = [x + (i % 4) as f32 * 19.0, at[1] + 46.0 + (i / 4) as f32 * 19.0];
                let have = layers.iter().any(|s| g.test(s));
                game.native_fill(batch, [cell[0], cell[1], 18.0, 18.0], if have { 0x8060B040 } else { 0x40806040 });
                if let Some(item) = first_item(g) {
                    game.native_item(batch, item, [cell[0] + 1.0, cell[1] + 1.0], 16.0);
                }
            }
            game.native_text(batch, "Bread first", [x, at[1] + BOOK[1] - 14.0], 0x8A6A40, false);
        }
        Kind::Board => {
            let cuts = fd::cuts(game);
            let Some(c) = cuts.get(recipe) else { return };
            if let Some(item) = first_item(&c.input) {
                game.native_item(batch, item, [x, at[1] + 6.0], 24.0);
                game.native_text(batch, &fit(&name_of(game, item), 13), [x, at[1] + 34.0], 0x40301A, false);
            }
            if let Some(tool) = first_item(&c.tool) {
                game.native_text(batch, "with", [x, at[1] + 50.0], 0x8A6A40, false);
                game.native_item(batch, tool, [x + 26.0, at[1] + 45.0], 16.0);
            }
            for (i, (result, chance)) in c.results.iter().enumerate().take(4) {
                let cell = [x + i as f32 * 20.0, at[1] + 70.0];
                draw_stack(game, batch, result, cell);
                if *chance < 1.0 {
                    let pct = format!("{}%", (chance * 100.0).round());
                    game.native_text(batch, &pct, [cell[0], cell[1] + 17.0], 0x8A6A40, false);
                }
            }
        }
    }
}

// ---- the pointer ----

pub(crate) fn press(game: &Game<'_>, view: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    let mouse = view.mouse;
    let Some((pos, kind, ready)) = client(|c| c.scene.as_ref().map(|s| (s.pos, s.kind, settled(s)))) else { return };
    if !ready {
        return;
    }
    let entries = book_entries_cached(game, kind, pos);
    book_changed();
    let board_empty = board_item(game, pos).is_none();
    let pot_meal = pot_view(game, pos).items.get(fd::MEAL).cloned().flatten();
    // A neighbouring station under the pointer (beside or below this one), and the yaw it is
    // framed from.
    let neighbour = neighbour_under(game, mouse).map(|(p, k)| (p, k, framing_yaw(game, k, p, client(|c| c.scene.as_ref().map_or(0.0, |s| s.to.1)))));
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(s) = c.scene.as_mut() else { return };
        let pose = current(s);
        // The strip: pick a stack up, or put it back.
        let strip_at = strip(pose, fov, size, pos);
        if let Some(o) = strip_at
            && let Some(slot) = strip_slot(o, mouse)
        {
            s.carried = if s.carried == Some(slot) || game.inventory.get(slot).is_none_or(Option::is_none) {
                None
            } else {
                Some(slot)
            };
            return;
        }
        // The book: turn a page, pick a recipe.
        if let Some(at) = book(pose, fov, size, pos, kind)
            && inside(at, BOOK, mouse)
        {
            let pages = entries.len().div_ceil(PER_PAGE).max(1);
            if inside(arrow_back(at), [16.0, 14.0], mouse) {
                s.page = s.page.saturating_sub(1);
            } else if inside(arrow_next(at), [16.0, 14.0], mouse) {
                s.page = (s.page + 1).min(pages - 1);
            } else if let Some(i) = (0..PER_PAGE).find(|&i| inside(grid_cell(at, i), [GRID.2, GRID.2], mouse)) {
                let picked = entries.get(s.page * PER_PAGE + i).map(|e| e.recipe);
                s.chosen = if s.chosen == picked { None } else { picked };
            }
            return;
        }
        if kind == Kind::Pot
            && let Some(at) = card(pose, fov, size, pos)
            && inside(at, CARD, mouse)
        {
            if let (Some(meal), Some(o)) = (&pot_meal, strip_at) {
                let to = slot_center(o, landing(game, &meal.item));
                s.flights.push(Flight { item: meal.item.to_string(), from: [at[0] + CARD[0] / 2.0, at[1] + 14.0], to, start: Instant::now() });
            }
            send(c, SERVE, pos, 0);
            return;
        }
        if kind == Kind::Skillet {
            // The handle first: a drag from it tosses the pan.
            if handle_point(game, pose, fov, size, pos).is_some_and(|h| (mouse[0] - h[0]).hypot(mouse[1] - h[1]) < 28.0) {
                s.toss_from = Some(mouse);
                return;
            }
        }
        let (y, half) = surface(kind, pos);
        let c0 = station_center(pos, y);
        let on = pointer_on(pose, fov, size, mouse, y)
            .filter(|p| (p[0] - c0[0]).abs() < half + 0.04 && (p[2] - c0[2]).abs() < half + 0.04);
        let Some(point) = on else {
            // Off the station: a click on its neighbour glides the camera there.
            if let Some((npos, nkind, nyaw)) = neighbour.filter(|(p, _, _)| *p != pos) {
                s.from = pose;
                s.to = framing(nkind, npos, nyaw);
                s.started = Instant::now();
                s.pos = npos;
                s.kind = nkind;
                s.carried = None;
                s.stir = None;
                s.cut_from = None;
                s.toss_from = None;
                s.logged = false;
                s.page = 0;
                s.chosen = None;
            }
            return;
        };
        let middle = project(pose, fov, size, c0).unwrap_or(mouse);
        match kind {
            Kind::Skillet => {
                let _ = point;
                if let Some(slot) = s.carried {
                    s.carried = None;
                    if let (Some(o), Some(Some(stack))) = (strip_at, game.inventory.get(slot)) {
                        s.flights.push(Flight { item: stack.item.to_string(), from: slot_center(o, slot), to: middle, start: Instant::now() });
                    }
                    send(c, PUT, pos, slot as u8);
                } else if let (Some(o), Some(food)) = (strip_at, skillet_view(game, pos).food) {
                    let to = slot_center(o, landing(game, &food.item));
                    s.flights.push(Flight { item: food.item.to_string(), from: middle, to, start: Instant::now() });
                    send(c, TAKE, pos, 0);
                }
            }
            Kind::Pot => {
                if let Some(slot) = s.carried {
                    let all = button == 0;
                    if all {
                        s.carried = None;
                    }
                    if let (Some(o), Some(Some(stack))) = (strip_at, game.inventory.get(slot)) {
                        s.flights.push(Flight { item: stack.item.to_string(), from: slot_center(o, slot), to: middle, start: Instant::now() });
                    }
                    send(c, PUT, pos, slot as u8 | if all { 0x80 } else { 0 });
                } else if button == 1 {
                    send(c, TAKE, pos, 0);
                } else {
                    s.stir = Some(angle(point, c0));
                    s.turn = 0.0;
                    s.reach.clear();
                }
            }
            Kind::Board => {
                let carried = s.carried;
                let stack = carried.and_then(|slot| game.inventory.get(slot).cloned().flatten());
                let layers = board_layers(game, pos);
                let cuts_it = match (&stack, layers.as_slice()) {
                    (Some(tool), [item]) => fd::cuts(game).iter().any(|c| c.input.test(item) && c.tool.test(tool)),
                    _ => false,
                };
                match (carried, stack) {
                    (Some(_), Some(_)) if cuts_it => s.cut_from = Some(point),
                    (Some(slot), Some(stack)) if board_takes(game, &layers, &stack) => {
                        // Put down, or stacked on the bread when it fits a sandwich (the server
                        // decides).
                        if stack.count <= 1 {
                            s.carried = None;
                        }
                        if let Some(o) = strip_at {
                            s.flights.push(Flight { item: stack.item.to_string(), from: slot_center(o, slot), to: middle, start: Instant::now() });
                        }
                        send(c, PUT, pos, slot as u8);
                    }
                    // Nothing carried, or what is carried can't go on what the board holds (a
                    // finished sandwich): the click picks the top of it up.
                    _ if !board_empty => {
                        if let (Some(o), Some(top)) = (strip_at, layers.last()) {
                            let to = slot_center(o, landing(game, &top.item));
                            s.flights.push(Flight { item: top.item.to_string(), from: middle, to, start: Instant::now() });
                        }
                        send(c, TAKE, pos, 0);
                    }
                    _ => {}
                }
            }
        }
    });
}

/// The hotbar slot an item would land in: one holding it, else the first empty one.
fn landing(game: &Game<'_>, item: &str) -> usize {
    (0..9)
        .find(|&i| matches!(game.inventory.get(i), Some(Some(s)) if *s.item == *item))
        .or_else(|| (0..9).find(|&i| matches!(game.inventory.get(i), Some(None) | None)))
        .unwrap_or(4)
}

/// The station block under the pointer near the one open (two blocks round it), if any.
fn neighbour_under(game: &Game<'_>, mouse: [f32; 2]) -> Option<([i32; 3], Kind)> {
    let (pose, pos, size, fov) = client(|c| c.scene.as_ref().map(|s| (current(s), s.pos, c.size, c.fov)))?;
    let dir = pointer_ray(pose, fov, size, mouse);
    let mut best: Option<(f64, [i32; 3], Kind)> = None;
    for dx in -2..=2 {
        for dy in -1..=1 {
            for dz in -2..=2 {
                let p = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                let Some(kind) = station_kind(game, p) else { continue };
                let top = match kind {
                    Kind::Pot => 0.65,
                    Kind::Board => 0.15,
                    Kind::Skillet => 0.3,
                };
                let min = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
                let max = [min[0] + 1.0, min[1] + top, min[2] + 1.0];
                if let Some(t) = ray_box(pose.0, dir, min, max)
                    && best.as_ref().is_none_or(|b| t < b.0)
                {
                    best = Some((t, p, kind));
                }
            }
        }
    }
    best.map(|(_, p, k)| (p, k))
}

fn angle(p: [f64; 3], c: [f64; 3]) -> f32 {
    ((p[2] - c[2]).atan2(p[0] - c[0])) as f32
}

pub(crate) fn release(game: &Game<'_>, view: &NativeScreenView<'_>, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    let mouse = view.mouse;
    let Some(pos) = client(|c| c.scene.as_ref().map(|s| s.pos)) else { return };
    let item = board_item(game, pos);
    let cuts = fd::cuts(game);
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(s) = c.scene.as_mut() else { return };
        s.stir = None;
        s.toss_from = None;
        let Some(from) = s.cut_from.take() else { return };
        let pose = current(s);
        let (y, _) = surface(s.kind, s.pos);
        let Some(slot) = s.carried else { return };
        let Some(to) = pointer_on(pose, fov, size, mouse, y) else { return };
        let d = ((to[0] - from[0]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
        if d < 0.35 {
            return;
        }
        // The pieces fly to the hotbar as the cut goes to the server.
        if let (Some(item), Some(Some(tool)), Some(o)) = (&item, game.inventory.get(slot), strip(pose, fov, size, pos))
            && let Some(cut) = cuts.iter().find(|c| c.input.test(item) && c.tool.test(tool))
        {
            let middle = project(pose, fov, size, station_center(pos, y)).unwrap_or(mouse);
            for (i, (result, _)) in cut.results.iter().enumerate() {
                let to = slot_center(o, landing(game, &result.item));
                let from = [middle[0] + i as f32 * 10.0 - 5.0, middle[1]];
                s.flights.push(Flight { item: result.item.to_string(), from, to, start: Instant::now() });
            }
        }
        send(c, CUT, pos, slot as u8);
    });
}

pub(crate) fn drag(game: &Game<'_>, view: &NativeScreenView<'_>, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    let mouse = view.mouse;
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(s) = c.scene.as_mut() else { return };
        if let Some(from) = s.toss_from
            && from[1] - mouse[1] >= 28.0
        {
            s.toss_from = None;
            let pos = s.pos;
            send(c, TOSS, pos, 0);
            return;
        }
        let Some(last) = s.stir else { return };
        let pose = current(s);
        let (y, half) = surface(s.kind, s.pos);
        let c0 = station_center(s.pos, y);
        let Some(p) = pointer_on(pose, fov, size, mouse, y) else { return };
        let now = angle(p, c0);
        let mut d = now - last;
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        } else if d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        s.stir = Some(now);
        s.turn += d;
        let r = (((p[0] - c0[0]).powi(2) + (p[2] - c0[2]).powi(2)).sqrt() / half) as f32;
        s.reach.push(r);
        let mut round = None;
        if s.turn.abs() >= std::f32::consts::TAU {
            // A wide round (well out from the middle) drawn evenly counts as steady.
            let n = s.reach.len().max(1) as f32;
            let mean = s.reach.iter().sum::<f32>() / n;
            let var = s.reach.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
            let quality = ((mean / 0.6).min(1.0) * 100.0 - var.sqrt() * 150.0).clamp(0.0, 100.0);
            round = Some(quality as u8);
            s.turn -= std::f32::consts::TAU * s.turn.signum();
            s.reach.clear();
        }
        let pos = s.pos;
        c.spin = (c.spin + d.abs() * 0.6).min(6.0);
        if let Some(q) = round {
            send(c, STIR, pos, q);
        }
    });
}

pub(crate) fn key(game: &Game<'_>, view: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    let leave = matches!(
        key,
        "key.keyboard.e" | "key.keyboard.w" | "key.keyboard.a" | "key.keyboard.s" | "key.keyboard.d" | "key.keyboard.space"
    );
    if leave {
        *canceled = true;
        client(|c| c.close = true);
    }
}

// ---- drawing in the world ----

/// What the stations show in the world: the pot's soup, its contents and its spoon; the tool over
/// the board and the cut marked across its item.
pub(crate) fn submit(
    game: &Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    if !game.native_on(ID) {
        return;
    }
    match block {
        fd::POT => submit_pot(game, pos, state, entity, draw),
        fd::BOARD => submit_board(game, pos, entity, draw),
        _ => {}
    }
}

fn submit_pot(
    game: &Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    let view = pot_view_of(&entity.nbt);
    let inputs: Vec<&ItemStack> = view.items.iter().take(fd::INPUTS).flatten().collect();
    let meal = view.items.get(fd::MEAL).cloned().flatten();
    let (here, pointer, swirl) = client(|c| {
        let now = Instant::now();
        let dt = c.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        c.last = Some(now);
        c.swirl += (0.3 + c.spin) * dt;
        c.spin *= (1.0 - dt * 1.5).max(0.0);
        let here = c.scene.as_ref().is_some_and(|s| s.pos == pos && s.leaving.is_none());
        (here, if here { c.pointer } else { None }, c.swirl)
    });
    let time = draw.game_time as f32 + draw.partial;
    // Someone stirred it in the last two seconds: the spoon goes round for everyone.
    let stirred = (draw.game_time / 20) as i32 - view.stir_at <= 2 && view.stir_at > 0;
    if here || !inputs.is_empty() || meal.is_some() {
        // The soup: water, taking the meal's colour as it cooks.
        let target = meal_of(game, &inputs).or_else(|| meal.clone());
        let colour = match (&target, view.stage) {
            (Some(t), stage) if stage >= 0 => mix(0x3F76E4, soup_colour(&t.item), 0.35 + 0.65 * stage as f32 / 4.0),
            (Some(t), _) if meal.is_some() => soup_colour(&t.item),
            (Some(t), _) => mix(0x3F76E4, soup_colour(&t.item), 0.3),
            (None, _) if inputs.is_empty() => 0x3F76E4,
            (None, _) => mix(0x3F76E4, 0x8A8070, 0.4),
        };
        let (a, b) = (3.0 / 16.0, 13.0 / 16.0);
        let quad = [[a, SOUP, a], [a, SOUP, b], [b, SOUP, b], [b, SOUP, a]];
        if !game.native_draw_quad(draw, "block/water_still", quad, [0.0, 0.0, 16.0, 16.0], 0xFF00_0000 | colour) {
            game.native_draw_quad(draw, "block/white_concrete", quad, [0.0, 0.0, 16.0, 16.0], 0xFF00_0000 | colour);
        }
    }
    // The ingredients float round the middle, the faster the harder it is stirred.
    let n = inputs.len().max(1) as f32;
    let turn = if here { swirl } else { time * 0.04 + if stirred { time * 0.15 } else { 0.0 } };
    for (i, stack) in inputs.iter().enumerate() {
        let a = turn + i as f32 * std::f32::consts::TAU / n;
        let r = 0.17;
        let bob = (time * 0.15 + i as f32 * 1.7).sin() * 0.008;
        draw.push();
        draw.translate([0.5 + a.cos() * r, SOUP + 0.015 + bob, 0.5 + a.sin() * r]);
        draw.rotate_about([0.0, 1.0, 0.0], -a.to_degrees(), [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], 72.0, [0.0, 0.0, 0.0]);
        draw.scale([0.2, 0.2, 0.2]);
        game.native_draw_item(draw, stack, DisplayContext::Fixed, None, i as i32);
        draw.pop();
    }
    // A meal ready to serve shows in the middle.
    if let Some(meal) = &meal {
        draw.push();
        draw.translate([0.5, SOUP + 0.06, 0.5]);
        draw.rotate_about([0.0, 1.0, 0.0], time * 2.0, [0.0, 0.0, 0.0]);
        draw.scale([0.3, 0.3, 0.3]);
        game.native_draw_item(draw, meal, DisplayContext::Fixed, None, 9);
        draw.pop();
    }
    // The spoon: at the pointer while stirring here, going round while someone stirs, else at
    // rest (Farmer's Delight draws it there).
    let foot = if let Some(p) = pointer {
        let mut l = [(p[0] - f64::from(pos[0])) as f32 - 0.5, (p[2] - f64::from(pos[2])) as f32 - 0.5];
        let r = (l[0] * l[0] + l[1] * l[1]).sqrt();
        let max = INSIDE - 0.06;
        if r > max {
            l = [l[0] / r * max, l[1] / r * max];
        }
        Some(l)
    } else if stirred {
        let a = time * 0.35;
        Some([a.cos() * 0.17, a.sin() * 0.17])
    } else {
        None
    };
    if let Some([lx, lz]) = foot {
        if let Ok(mut away) = fd::SPOON_AWAY.lock() {
            *away = Some(pos);
        }
        let r = (lx * lx + lz * lz).sqrt();
        // It leans out towards the rim, the handle over the side it stirs.
        let (dx, dz) = if r > 0.01 { (lx / r, lz / r) } else { (0.0, 1.0) };
        let _ = state;
        draw.push();
        draw.translate([lx, 0.08, lz]);
        draw.rotate_about([dz, 0.0, -dx], 14.0 + r * 40.0, [0.5, 0.1875, 0.5]);
        game.native_draw_model(draw, fd::SPOON_MODEL, 0xFF_FFFF);
        draw.pop();
    }
}

fn submit_board(game: &Game<'_>, pos: [i32; 3], entity: &ModBlockEntity, draw: &mut super::native_block_entity_draw::BlockEntityDraw) {
    // The layers stacked on the board's bread (Farmer's Delight draws the bread itself).
    let layers = horizon_items::item_nbt::decode_items(entity.nbt.get(&fd::key("Items")), fd::BOARD_LAYERS);
    for (i, layer) in layers.iter().enumerate().skip(1) {
        let Some(layer) = layer else { continue };
        let scale = 0.58 - 0.035 * i as f32;
        draw.push();
        draw.translate([0.5, 0.0625 + 0.016 + 0.034 * i as f32, 0.5]);
        draw.rotate_about([0.0, 1.0, 0.0], 13.0 * i as f32 - 20.0, [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
        draw.scale([scale, scale, scale]);
        game.native_draw_item(draw, layer, DisplayContext::Fixed, None, 20 + i as i32);
        draw.pop();
    }
    let Some((pointer, carried, cut_from, yaw)) = client(|c| {
        let s = c.scene.as_ref().filter(|s| s.pos == pos && s.leaving.is_none())?;
        Some((c.pointer, s.carried, s.cut_from, s.to.1))
    }) else {
        return;
    };
    let Some(tool) = carried.and_then(|slot| game.inventory.get(slot).cloned().flatten()) else { return };
    let items: Vec<&ItemStack> = layers.iter().flatten().collect();
    let cuts = fd::cuts(game);
    let cuttable = matches!(items.as_slice(), [i] if cuts.iter().any(|c| c.input.test(i) && c.tool.test(&tool)));
    // The camera's right, along which the cut is drawn, in the block's frame.
    let y = f64::from(yaw).to_radians();
    let right = [-(y.cos() as f32), -(y.sin() as f32)];
    let along = [-right[1], right[0]];
    if cuttable {
        // The mark across the item: dashes along the camera's right.
        let mut t = -0.32f32;
        while t < 0.32 {
            let a = [0.5 + right[0] * t, 0.5 + right[1] * t];
            let b = [0.5 + right[0] * (t + 0.05), 0.5 + right[1] * (t + 0.05)];
            let w = [along[0] * 0.008, along[1] * 0.008];
            let h = 0.0625 + 0.042;
            let quad = [[a[0] - w[0], h, a[1] - w[1]], [a[0] + w[0], h, a[1] + w[1]], [b[0] + w[0], h, b[1] + w[1]], [b[0] - w[0], h, b[1] - w[1]]];
            game.native_draw_quad(draw, "block/black_concrete", quad, [0.0, 0.0, 16.0, 16.0], 0xFF20_1810);
            t += 0.085;
        }
    }
    // The tool (or what is carried) under the pointer, over the board; tipped down mid-cut.
    let Some(p) = pointer else { return };
    let l = [(p[0] - f64::from(pos[0])) as f32, (p[2] - f64::from(pos[2])) as f32];
    let blade = game.items().get(&tool.item).is_some_and(|i| i.max_damage > 0);
    draw.push();
    draw.translate([l[0], if blade { 0.2 } else { 0.12 }, l[1]]);
    draw.rotate_about([0.0, 1.0, 0.0], 180.0 - yaw, [0.0, 0.0, 0.0]);
    if blade {
        // Upright, its edge to the board, leaning into the cut.
        let lean = if cut_from.is_some() { 35.0 } else { 15.0 };
        draw.rotate_about([0.0, 0.0, 1.0], 45.0 + 90.0, [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], lean, [0.0, 0.0, 0.0]);
        draw.scale([0.55, 0.55, 0.55]);
    } else {
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
        draw.scale([0.45, 0.45, 0.45]);
    }
    game.native_draw_item(draw, &tool, DisplayContext::Fixed, None, 3);
    draw.pop();
}

// ---- the server ----

static SERVER: Mutex<Option<HashMap<u128, Vec<Vec<u8>>>>> = Mutex::new(None);

/// `net_from_client`: a station's action, done on the player's next tick (where their inventory is).
pub(crate) fn net_from_client(player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() != 14 {
        return;
    }
    let mut queue = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let list = queue.get_or_insert_with(HashMap::new).entry(player).or_default();
    if list.len() < 32 {
        list.push(data.to_vec());
    }
}

pub(crate) fn player_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let Some(uuid) = game.native_acting_player().map(|a| a.uuid) else { return };
    let acts = SERVER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|q| q.remove(&uuid))
        .unwrap_or_default();
    for a in acts {
        let op = a[0];
        let pos = [0, 1, 2].map(|i| i32::from_le_bytes(a[1 + i * 4..5 + i * 4].try_into().unwrap_or_default()));
        act(game, op, pos, a[13]);
    }
}

fn act(game: &mut Game<'_>, op: u8, pos: [i32; 3], arg: u8) {
    if !game.world.loaded(pos[0], pos[2]) {
        return;
    }
    let Some(eye) = game.eye_position() else { return };
    let d2 = (0..3).map(|i| (eye[i] - (f64::from(pos[i]) + 0.5)).powi(2)).sum::<f64>();
    if d2 > 64.0 {
        return;
    }
    let Some(kind) = station_kind(game, pos) else { return };
    let key = (pos[0], pos[1], pos[2]);
    if game.world.block_entities.get(&key).is_none() {
        let name = match kind {
            Kind::Pot => fd::POT,
            Kind::Board => fd::BOARD,
            Kind::Skillet => fd::SKILLET,
        };
        if let Some(entity) = game.new_block_entity(name) {
            game.world.block_entities.insert(key, entity);
        }
    }
    match (kind, op) {
        (Kind::Pot, PUT) => pot_put(game, pos, usize::from(arg & 0x7f), arg & 0x80 != 0),
        (Kind::Pot, TAKE) => pot_take(game, pos),
        (Kind::Pot, STIR) => {
            fd::stirred(game, pos, arg.min(100));
            game.native_block_entity_sync(pos);
            game.native_play_sound_in_place("farmersdelight:block.cooking_pot.boil", fd::center(pos, 0.6), 4, 0.4, 1.2);
        }
        (Kind::Pot, SERVE) => pot_serve(game, pos),
        (Kind::Board, PUT) => board_put(game, pos, usize::from(arg & 0x7f)),
        (Kind::Board, TAKE) => {
            let items = fd::items_at(game, pos);
            if let Some(top) = items.iter().rposition(Option::is_some)
                && let Some(Some(item)) = items.get(top).cloned()
            {
                fd::set_slot(game, pos, top, None);
                game.give_or_drop(item);
                game.native_play_sound_in_place("farmersdelight:block.cutting_board.remove_item", fd::center(pos, 0.1), 4, 1.0, 1.0);
            }
        }
        (Kind::Board, CUT) => board_cut(game, pos, usize::from(arg)),
        (Kind::Skillet, PUT) => {
            fd::skillet_put_from(game, pos, Some(usize::from(arg & 0x7f)));
        }
        (Kind::Skillet, TAKE) => {
            if let Some(Some(food)) = fd::items_at(game, pos).into_iter().next() {
                fd::set_slot(game, pos, 0, None);
                fd::skillet_reset(game, pos);
                game.give_or_drop(food);
            }
        }
        (Kind::Skillet, TOSS) => fd::tossed(game, pos),
        _ => {}
    }
    game.guest_inventory_resend = true;
}

fn take_from_slot(game: &mut Game<'_>, slot: usize, count: u32) -> Option<ItemStack> {
    let cell = game.inventory.get_mut(slot)?;
    let stack = cell.as_mut()?;
    let n = count.min(stack.count);
    let taken = ItemStack { count: n, ..stack.clone() };
    stack.count -= n;
    if stack.count == 0 {
        *cell = None;
    }
    Some(taken)
}

fn pot_put(game: &mut Game<'_>, pos: [i32; 3], slot: usize, all: bool) {
    let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
    let items = fd::items_at(game, pos);
    let max = game.items().get(&stack.item).map_or(64, |i| i.max_stack);
    let target = (0..fd::INPUTS)
        .find(|&i| matches!(items.get(i), Some(Some(s)) if s.same_item_same_components(&stack) && s.count < max))
        .or_else(|| (0..fd::INPUTS).find(|&i| items.get(i).is_none_or(Option::is_none)));
    let Some(target) = target else { return };
    let room = match items.get(target) {
        Some(Some(s)) => max - s.count,
        _ => max,
    };
    let n = if all { stack.count.min(room) } else { 1 };
    let Some(taken) = take_from_slot(game, slot, n) else { return };
    let mut cell = items.get(target).cloned().flatten();
    fd::add_to(&mut cell, taken);
    fd::set_slot(game, pos, target, cell);
    game.native_play_sound_in_place("minecraft:entity.generic.splash", fd::center(pos, 0.6), 4, 0.25, 1.4);
}

fn pot_take(game: &mut Game<'_>, pos: [i32; 3]) {
    let items = fd::items_at(game, pos);
    let Some(i) = (0..fd::INPUTS).rev().find(|&i| matches!(items.get(i), Some(Some(_)))) else { return };
    let Some(Some(stack)) = items.get(i).cloned() else { return };
    fd::set_slot(game, pos, i, None);
    game.give_or_drop(stack);
}

fn pot_serve(game: &mut Game<'_>, pos: [i32; 3]) {
    let items = fd::items_at(game, pos);
    let Some(Some(meal)) = items.get(fd::MEAL).cloned() else { return };
    if let Some(container) = fd::meal_container(game, &meal) {
        let Some(slot) = (0..game.inventory.len()).find(|&i| matches!(&game.inventory[i], Some(s) if *s.item == *container)) else {
            return;
        };
        take_from_slot(game, slot, 1);
    }
    let mut rest = meal.clone();
    rest.count -= 1;
    fd::set_slot(game, pos, fd::MEAL, (rest.count > 0).then_some(rest));
    game.give_or_drop(fd::one(&meal));
    game.native_play_sound_in_place("farmersdelight:block.food.take_portion", fd::center(pos, 0.6), 4, 1.0, 1.0);
}

fn board_put(game: &mut Game<'_>, pos: [i32; 3], slot: usize) {
    let items = fd::items_at(game, pos);
    let Some(Some(carried)) = game.inventory.get(slot).cloned() else { return };
    let Some(Some(base)) = items.first().cloned() else {
        let Some(one) = take_from_slot(game, slot, 1) else { return };
        fd::set_slot(game, pos, 0, Some(one));
        game.native_play_sound_in_place("farmersdelight:block.cutting_board.place_item", fd::center(pos, 0.1), 4, 1.0, 0.8);
        return;
    };
    // On bread: a layer of a sandwich, the last one making it.
    let layers: Vec<ItemStack> = items.iter().skip(1).flatten().cloned().collect();
    let one = fd::one(&carried);
    let mut stack: Vec<&ItemStack> = layers.iter().collect();
    stack.push(&one);
    let sandwiches = fd::sandwiches(game);
    let Some(w) = sandwiches.iter().find(|w| w.base.test(&base) && fd::partial(&w.fillings, &stack)) else { return };
    if take_from_slot(game, slot, 1).is_none() {
        return;
    }
    if stack.len() == w.fillings.len() {
        for i in 1..fd::BOARD_LAYERS {
            fd::set_slot(game, pos, i, None);
        }
        fd::set_slot(game, pos, 0, Some(w.result.clone()));
        game.native_play_sound_in_place("farmersdelight:block.cutting_board.place_item", fd::center(pos, 0.1), 4, 1.0, 1.3);
        game.native_play_sound_in_place("minecraft:entity.item.pickup", fd::center(pos, 0.2), 4, 0.4, 1.4);
    } else {
        fd::set_slot(game, pos, 1 + layers.len(), Some(one));
        game.native_play_sound_in_place("farmersdelight:block.cutting_board.place_item", fd::center(pos, 0.1), 4, 1.0, 1.0 + 0.08 * layers.len() as f32);
    }
}

fn board_cut(game: &mut Game<'_>, pos: [i32; 3], slot: usize) {
    let items = fd::items_at(game, pos);
    if items.iter().skip(1).any(Option::is_some) {
        return;
    }
    let Some(Some(item)) = items.into_iter().next() else { return };
    let Some(Some(tool)) = game.inventory.get(slot).cloned() else { return };
    let cuts = fd::cuts(game);
    let Some(cut) = cuts.iter().find(|c| c.input.test(&item) && c.tool.test(&tool)) else { return };
    fd::set_slot(game, pos, 0, None);
    for (i, (result, chance)) in cut.results.iter().enumerate() {
        if *chance < 1.0 && fd::roll(game, pos, i as u64) >= *chance {
            continue;
        }
        game.give_or_drop(result.clone());
    }
    if slot < 9 && game.items().get(&tool.item).is_some_and(|i| i.max_damage > 0) {
        let selected = game.selected;
        game.selected = slot;
        game.damage_held(1);
        game.selected = selected;
    }
    let sound = cut.sound.as_deref().unwrap_or("farmersdelight:block.cutting_board.knife_cut");
    game.native_play_sound_in_place(sound, fd::center(pos, 0.1), 4, 0.8, 1.0);
}
