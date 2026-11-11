// Cinematic Interactions for Horizon, written from its documented behaviour (its CurseForge page;
// none of its code): workstations become scenes in the world. Stage 1: the crafting table.
//  - A right click on a crafting table (not sneaking) flies the camera over it and hides the HUD;
//    letterbox bars slide in, the inventory lies in a panel below and a recipe guide on the left.
//  - The table's top is a 3x3 grid. Pick a stack in the panel and click a cell to lay it there
//    (left: the stack, right: one; drag with the right button to lay one in each cell passed).
//    A click on a cell with nothing picked takes its items back (right: one). The result floats
//    beside the grid: click it to craft (shift: as many as the grid makes).
//  - The guide lists what can be crafted, what the inventory makes first; type in its box to
//    search by name. A recipe picked shows its ingredients on the table, the missing ones marked;
//    Lay out puts them on the grid from the inventory (shift: as many sets as there are).
//  - What lies on the table stays when the player leaves and every player sees it; it drops when
//    the table goes. Its rows face the side it was laid out from.
//  - Furnaces, smokers and blast furnaces (stage 2): framed from the front; pick an item and click
//    the top to smelt it, the fire to fuel it; click a part to take what is there (the output pays
//    its experience). Brewing stands: bottles on the arms, the ingredient on top (it dissolves as
//    the brew goes), blaze powder at the foot. Campfires: food onto the fire; H by a campfire sits
//    the player down beside it.
//  - Anvils, grindstones, smithing tables and stonecutters (stage 3) work from the inventory: a
//    stack picked and clicked onto a part of the station lies there (still in the inventory), the
//    result floats over it, and a click on the result makes it, paying what the menu would (the
//    anvil's levels and wear, the grindstone's experience back). The anvil names items in a box,
//    the stonecutter lists what the stone cuts into.
//  - Stage 4: the enchanting table shows the three offers for the item and lapis laid on it (the
//    item rises to the book as it is enchanted); the lectern's book opens over it and its pages
//    turn; discs go into the jukebox and books into a chiseled bookshelf's slots; the loom weaves
//    the pattern picked; the cartography table copies, zooms out and locks maps; the crafter's grid
//    lies on its top (a click on an empty cell with nothing picked turns it off or on); the beacon
//    takes its payment and the powers picked.
//  - The player's own arm (their skin) reaches into every scene but the lectern's: the hand follows
//    the pointer over the station, holds the stack picked up, presses down on a click and takes
//    the hammer's handle at the anvil, whose blow shakes the view. What it lets go of drops the
//    last of the way onto the table.
//  - Hold Left Alt to see yourself at the table. A click on another crafting table nearby glides
//    the camera over to it. Escape, E, a movement key or jump flies back; coming back soon after
//    is quicker. Sneak + right click opens the usual crafting menu, and so does a table with no
//    room over it.
use super::{Game, Mapping};
use horizon_assets::item_model::DisplayContext;
use horizon_client::crafting::{Ingredient, Recipe};
use horizon_client::furnace::FurnaceCooking;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, HudMode, NativeScreenView, Tag};
use horizon_client::text::Text;
use horizon_core::random::RandomSource;
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

pub const ID: &str = "cinematic_interactions";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "Workstations worked in the world: the camera flies to the station and you craft, smelt, brew, forge, \
     enchant, weave, read and play records at it, the inventory at hand.";
const CHANNEL: &str = "cinematic_interactions:act";
const TABLE: &str = "minecraft:crafting_table";

const ENTER: f32 = 0.6;
const ENTER_QUICK: f32 = 0.25;
const LEAVE: f32 = 0.4;
const FLIGHT: f32 = 0.42;
/// Coming back to the same table within this many seconds is quicker.
const QUICK: f32 = 4.0;
/// The panel's slots, and the guide's size and recipe cells.
const SLOT: f32 = 18.0;
const BOOK: [f32; 2] = [204.0, 128.0];
const ICON: f32 = 20.0;
const COLS: usize = 4;
const ROWS: usize = 4;
const PER_PAGE: usize = COLS * ROWS;
/// The grid on the table's top: cells this far apart (the squares of its texture), each this wide.
const CELL: f64 = 0.25;
const HALF: f64 = 0.115;
/// The most a cell holds.
const CELL_MAX: u32 = 64;

const PLACE: u8 = 1;
const TAKE: u8 = 2;
const CRAFT: u8 = 3;
const LAYOUT: u8 = 4;
const CLEAR: u8 = 5;
const OPEN: u8 = 10;
const CLOSE: u8 = 11;
const PUT: u8 = 12;
const PULL: u8 = 13;
const SIT: u8 = 14;
const RESULT: u8 = 15;
/// What lies on a station whose result the server works out; a crafter's cell turned off or on;
/// the lectern's page; its book taken.
const LAY: u8 = 16;
const TOGGLE: u8 = 17;
const PAGE: u8 = 18;
const TAKE_BOOK: u8 = 19;
/// What a station holds, sent to the player working it.
const VIEW: &str = "cinematic_interactions:view";
/// The seats by campfires: invisible, silent mobs that never move.
const SEAT_TAG: &str = "cinematic_seat";
pub const SIT_KEY: &str = "sit";
pub const SIT_DEFAULT: &str = "key.keyboard.h";

/// The stations with a scene.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Table,
    Furnace,
    Brewing,
    Campfire,
    Anvil,
    Grindstone,
    Smithing,
    Stonecutter,
    Enchanting,
    Lectern,
    Jukebox,
    Bookshelf,
    Loom,
    Cartography,
    Crafter,
    Beacon,
}

/// The stations whose stacks stay in the inventory while they lie on them.
fn from_inventory(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Anvil | Kind::Grindstone | Kind::Smithing | Kind::Stonecutter | Kind::Enchanting | Kind::Loom | Kind::Cartography | Kind::Beacon
    )
}

/// The stations whose state the server sends the player working them.
fn viewed(kind: Kind) -> bool {
    !matches!(kind, Kind::Table | Kind::Anvil | Kind::Grindstone | Kind::Smithing | Kind::Stonecutter | Kind::Loom)
}

/// Stations that work from the inventory whose result the server works out (it has the player's
/// enchantment seed and the level's maps): they tell it what lies on them.
fn told(kind: Kind) -> bool {
    matches!(kind, Kind::Enchanting | Kind::Cartography)
}

fn kind_of(name: &str) -> Option<Kind> {
    Some(match name {
        TABLE => Kind::Table,
        "minecraft:furnace" | "minecraft:smoker" | "minecraft:blast_furnace" => Kind::Furnace,
        "minecraft:brewing_stand" => Kind::Brewing,
        "minecraft:campfire" | "minecraft:soul_campfire" => Kind::Campfire,
        "minecraft:anvil" | "minecraft:chipped_anvil" | "minecraft:damaged_anvil" => Kind::Anvil,
        "minecraft:grindstone" => Kind::Grindstone,
        "minecraft:smithing_table" => Kind::Smithing,
        "minecraft:stonecutter" => Kind::Stonecutter,
        "minecraft:enchanting_table" => Kind::Enchanting,
        "minecraft:lectern" => Kind::Lectern,
        "minecraft:jukebox" => Kind::Jukebox,
        "minecraft:chiseled_bookshelf" => Kind::Bookshelf,
        "minecraft:loom" => Kind::Loom,
        "minecraft:cartography_table" => Kind::Cartography,
        "minecraft:crafter" => Kind::Crafter,
        "minecraft:beacon" => Kind::Beacon,
        _ => return None,
    })
}

fn station_at(game: &Game<'_>, pos: [i32; 3]) -> Option<Kind> {
    if !game.world.loaded(pos[0], pos[2]) {
        return None;
    }
    kind_of(&game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).name)
}

type Pose = ([f64; 3], f32, f32);

// ---- what lies on the tables: a synced chunk attachment ----

#[derive(Clone, Debug)]
pub(super) struct Table {
    pos: [i32; 3],
    /// The side the grid is read from: its rows run away from there (0 south, 1 west, 2 north,
    /// 3 east, as a camera's yaw over 90).
    facing: u8,
    items: Vec<Option<ItemStack>>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Tables(Vec<Table>);

fn encode(t: &Tables) -> Result<Compound, AttachmentError> {
    let mut at = Vec::with_capacity(t.0.len() * 3);
    let mut facing = Vec::with_capacity(t.0.len());
    let mut items = Vec::with_capacity(t.0.len());
    for table in &t.0 {
        at.extend_from_slice(&table.pos);
        facing.push(i32::from(table.facing));
        items.push(horizon_items::item_nbt::encode_items(&table.items));
    }
    Ok([("at".into(), Tag::IntArray(at)), ("facing".into(), Tag::IntArray(facing)), ("items".into(), Tag::List(items))].into())
}

fn decode(data: &Compound) -> Result<Tables, AttachmentError> {
    let (Some(Tag::IntArray(at)), Some(Tag::IntArray(facing)), Some(Tag::List(items))) =
        (data.get(&"at".into()), data.get(&"facing".into()), data.get(&"items".into()))
    else {
        return Err(AttachmentError::Decode);
    };
    if at.len() != items.len() * 3 || facing.len() != items.len() {
        return Err(AttachmentError::Decode);
    }
    let out = items
        .iter()
        .enumerate()
        .map(|(i, slots)| Table {
            pos: [at[i * 3], at[i * 3 + 1], at[i * 3 + 2]],
            facing: (facing[i] & 3) as u8,
            items: horizon_items::item_nbt::decode_items(Some(slots), 9),
        })
        .collect();
    Ok(Tables(out))
}

pub(super) static TABLES: AttachmentType<Tables> = AttachmentType {
    namespace: "cinematic_interactions",
    name: "tables",
    default: Tables::default,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

fn chunk(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

fn is_table(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.world.loaded(pos[0], pos[2]) && &*game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).name == TABLE
}

/// The table at `pos` as stored (an empty grid when nothing lies on it).
fn table_at(game: &mut Game<'_>, pos: [i32; 3]) -> Table {
    let host: &mut dyn Host = game;
    TABLES
        .get_existing(host, chunk(pos))
        .ok()
        .flatten()
        .and_then(|t| t.0.into_iter().find(|t| t.pos == pos))
        .unwrap_or(Table { pos, facing: 0, items: vec![None; 9] })
}

fn store(game: &mut Game<'_>, table: Table) {
    let pos = table.pos;
    let host: &mut dyn Host = game;
    let mut all = TABLES.get_existing(host, chunk(pos)).ok().flatten().unwrap_or_default();
    all.0.retain(|t| t.pos != pos);
    if table.items.iter().any(Option::is_some) {
        all.0.push(table);
    }
    let _ = if all.0.is_empty() { TABLES.remove(host, chunk(pos)).map(|_| ()) } else { TABLES.set(host, chunk(pos), all) };
}

// ---- the client ----

/// An item icon flying across the screen (the panel to the table and back).
struct Flight {
    item: String,
    from: [f32; 2],
    to: [f32; 2],
    start: Instant,
}

struct Scene {
    pos: [i32; 3],
    kind: Kind,
    facing: u8,
    from: Pose,
    to: Pose,
    started: Instant,
    enter: f32,
    leaving: Option<Instant>,
    /// The inventory slot picked up in the panel.
    carried: Option<usize>,
    /// Laying one in each cell passed (the right button held): the cells done.
    painting: Option<Vec<usize>>,
    hud: HudMode,
    logged: bool,
    /// The guide: its page, the result picked, the search and whether it is being typed.
    page: usize,
    chosen: Option<String>,
    search: String,
    typing: bool,
    flights: Vec<Flight>,
    /// The inventory slots laid on a station that works from the inventory, and the stonecutter's
    /// choice.
    refs: [Option<usize>; 3],
    choice: Option<usize>,
    /// When the hammer last came down on the anvil (an item was enchanted, a disc went in).
    strike: Option<Instant>,
    /// The lectern's page shown, and a page turning: when, and whether forward.
    leaf: Option<usize>,
    turn: Option<(Instant, bool)>,
    /// The beacon's powers picked: indexes into its list of effects.
    powers: [Option<usize>; 2],
}

struct Client {
    scene: Option<Scene>,
    out: Vec<Vec<u8>>,
    close: bool,
    size: [f32; 2],
    fov: f32,
    /// The camera leaning towards the pointer (-1 to 1 each way), and its last update.
    lean: [f32; 2],
    lean_at: Option<Instant>,
    /// Looking at oneself (Left Alt), 0 to 1, the player's position, and the camera type to
    /// give back.
    peek: f32,
    player: [f64; 3],
    restore: Option<super::perspective::CameraType>,
    /// The table left last, and when (a quicker way back).
    left: Option<([i32; 3], Instant)>,
    /// What lies on the tables near the player, as the server sent it.
    tables: Vec<Table>,
    /// What the station worked holds, as the server sent it.
    view: Option<View>,
    /// The pointer, in GUI pixels.
    mouse: [f32; 2],
    /// The hand in the scene: where it is, when it last moved, when it last pressed down, and
    /// whether it is out over the station (else it rests out of sight).
    hand: Option<[f64; 3]>,
    hand_at: Option<Instant>,
    tap: Option<Instant>,
    hand_out: bool,
    /// The table cells an item just landed on, and when.
    landed: Vec<([i32; 3], usize, Instant)>,
}

/// A station's slots and numbers (a furnace's fire and cooking, a brewing stand's brew and fuel),
/// and its words (the enchanting table's clues).
#[derive(Clone)]
struct View {
    pos: [i32; 3],
    items: Vec<Option<ItemStack>>,
    values: Vec<i32>,
    texts: Vec<String>,
}

static CLIENT: Mutex<Client> = Mutex::new(Client {
    scene: None,
    out: Vec::new(),
    close: false,
    size: [0.0, 0.0],
    fov: 70.0,
    lean: [0.0, 0.0],
    lean_at: None,
    peek: 0.0,
    player: [0.0; 3],
    restore: None,
    left: None,
    tables: Vec::new(),
    view: None,
    mouse: [0.0, 0.0],
    hand: None,
    hand_at: None,
    tap: None,
    hand_out: false,
    landed: Vec::new(),
});

fn client<T>(f: impl FnOnce(&mut Client) -> T) -> T {
    f(&mut CLIENT.lock().unwrap_or_else(|e| e.into_inner()))
}

// ---- the camera ----

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn progress(s: &Scene) -> f32 {
    let enter = smooth(s.started.elapsed().as_secs_f32() / s.enter);
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

fn facing_of(yaw: f32) -> u8 {
    ((yaw / 90.0).round() as i32).rem_euclid(4) as u8
}

fn facing_yaw(facing: u8) -> f32 {
    f32::from(facing) * 90.0
}

/// Over the table from the side its grid faces, looking down on it.
fn framing(pos: [i32; 3], facing: u8) -> Pose {
    let yaw = facing_yaw(facing);
    let (pitch, distance) = (60.0f32, 1.55);
    let f = super::view_vector(yaw, pitch);
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5];
    ([at[0] - f[0] * distance, at[1] - f[1] * distance, at[2] - f[2] * distance], yaw, pitch)
}

/// How a station is framed: the table from above, a furnace from its front, the rest from the
/// side the player came from.
fn framing_of(kind: Kind, pos: [i32; 3], facing: u8) -> Pose {
    let (pitch, distance, focus) = match kind {
        Kind::Table => return framing(pos, facing),
        Kind::Furnace => (24.0f32, 1.75, 0.55),
        Kind::Brewing => (40.0, 1.35, 0.45),
        Kind::Campfire => (50.0, 1.45, 0.3),
        Kind::Anvil => (52.0, 1.5, 1.0),
        Kind::Grindstone => (36.0, 1.55, 0.6),
        Kind::Smithing => (60.0, 1.55, 1.0),
        Kind::Stonecutter => (48.0, 1.4, 0.55),
        Kind::Enchanting => (42.0, 1.6, 0.85),
        Kind::Lectern => (36.0, 1.3, 0.95),
        Kind::Jukebox => (48.0, 1.45, 0.8),
        Kind::Bookshelf => (8.0, 1.55, 0.5),
        Kind::Loom => (50.0, 1.45, 0.9),
        Kind::Cartography => (60.0, 1.5, 1.0),
        Kind::Crafter => return framing(pos, facing),
        Kind::Beacon => (36.0, 1.9, 0.6),
    };
    let yaw = facing_yaw(facing);
    let f = super::view_vector(yaw, pitch);
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + focus, f64::from(pos[2]) + 0.5];
    ([at[0] - f[0] * distance, at[1] - f[1] * distance, at[2] - f[2] * distance], yaw, pitch)
}

/// The side a station is framed from: a laid table's own, a furnace's front, else the player's.
fn open_facing(game: &Game<'_>, kind: Kind, pos: [i32; 3], yaw: f32) -> u8 {
    match kind {
        Kind::Table => {
            let (stored, items) = grid_of(pos);
            if items.iter().any(Option::is_some) { stored } else { facing_of(yaw) }
        }
        // A furnace's, a lectern's and a bookshelf's front faces the player who placed it.
        Kind::Furnace | Kind::Lectern | Kind::Bookshelf => {
            let state = game.world.block(pos[0], pos[1], pos[2]);
            match game.native_block_state_property(state, "facing") {
                Some("north") => 0,
                Some("east") => 1,
                Some("south") => 2,
                Some("west") => 3,
                _ => facing_of(yaw),
            }
        }
        // Along an anvil's or a grindstone's facing, so its two sides are left and right: from
        // the end nearest the player.
        Kind::Anvil | Kind::Grindstone => {
            let state = game.world.block(pos[0], pos[1], pos[2]);
            let along = match game.native_block_state_property(state, "facing") {
                Some("south") => 0u8,
                Some("west") => 1,
                Some("north") => 2,
                Some("east") => 3,
                _ => return facing_of(yaw),
            };
            if facing_of(yaw) == (along + 2) % 4 { (along + 2) % 4 } else { along }
        }
        _ => facing_of(yaw),
    }
}

/// Behind the player, looking at them and the table (Left Alt).
fn peek_pose(player: [f64; 3], pos: [i32; 3]) -> Pose {
    let t = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.9, f64::from(pos[2]) + 0.5];
    let mut d = [player[0] - t[0], player[2] - t[2]];
    let len = d[0].hypot(d[1]);
    if len < 0.3 {
        d = [0.0, 1.0];
    } else {
        d = [d[0] / len, d[1] / len];
    }
    // Back past the player and off to one side, up a little.
    let side = [-d[1], d[0]];
    let cam = [player[0] + d[0] * 2.4 + side[0] * 1.4, player[1] + 2.1, player[2] + d[1] * 2.4 + side[1] * 1.4];
    let mid = [(player[0] + t[0]) / 2.0, player[1] + 0.9, (player[2] + t[2]) / 2.0];
    let v = [mid[0] - cam[0], mid[1] - cam[1], mid[2] - cam[2]];
    let h = v[0].hypot(v[2]);
    let yaw = (-v[0]).atan2(v[2]).to_degrees() as f32;
    let pitch = (-v[1]).atan2(h).to_degrees() as f32;
    (cam, yaw, pitch)
}

/// The pose the frame is drawn from: the glide, the lean towards the pointer and the peek.
fn scene_pose(c: &Client, s: &Scene) -> Pose {
    let mut pose = lerp_pose(s.from, s.to, progress(s));
    let k = progress(s);
    pose.1 += c.lean[0] * 1.2 * k;
    pose.2 += c.lean[1] * 0.8 * k;
    // The hammer landing on the anvil shakes the view.
    if s.kind == Kind::Anvil
        && let Some(t) = s.strike.map(|at| at.elapsed().as_secs_f32() - 0.42).filter(|t| (0.0..0.25).contains(t))
    {
        let fade = 1.0 - t / 0.25;
        pose.1 += (t * 65.0).cos() * 0.3 * fade;
        pose.2 += (t * 80.0).sin() * 0.5 * fade;
    }
    if c.peek > 0.0 {
        pose = lerp_pose(pose, peek_pose(c.player, s.pos), smooth(c.peek));
    }
    pose
}

/// `camera_view`: the table's camera while a scene is on.
pub(crate) fn camera() -> Option<([f64; 3], f32, f32)> {
    client(|c| c.scene.as_ref().map(|s| scene_pose(c, s)))
}

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

fn pointer_ray(pose: Pose, fov: f32, size: [f32; 2], mouse: [f32; 2]) -> [f64; 3] {
    let (f, r, u) = basis(pose.1, pose.2);
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0] / size[1].max(1.0));
    let nx = f64::from(mouse[0] / size[0].max(1.0)) * 2.0 - 1.0;
    let ny = 1.0 - f64::from(mouse[1] / size[1].max(1.0)) * 2.0;
    [0, 1, 2].map(|i| f[i] + r[i] * nx * t * aspect + u[i] * ny * t)
}

fn pointer_on(pose: Pose, fov: f32, size: [f32; 2], mouse: [f32; 2], y: f64) -> Option<[f64; 3]> {
    let dir = pointer_ray(pose, fov, size, mouse);
    if dir[1].abs() < 1e-6 {
        return None;
    }
    let k = (y - pose.0[1]) / dir[1];
    (k > 0.0).then(|| [pose.0[0] + dir[0] * k, y, pose.0[2] + dir[2] * k])
}

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

// ---- the grid ----

/// The grid's directions on a table facing `facing`: across (left to right) and away (near to far).
fn grid_axes(facing: u8) -> ([f64; 2], [f64; 2]) {
    let y = f64::from(facing_yaw(facing)).to_radians();
    let right = [-y.cos(), -y.sin()];
    let away = [-y.sin(), y.cos()];
    (right, away)
}

/// A cell's middle on the table's top (row 0 the far one, column 0 on the left).
fn cell_center(pos: [i32; 3], facing: u8, cell: usize) -> [f64; 3] {
    let (right, away) = grid_axes(facing);
    let (row, col) = ((cell / 3) as f64, (cell % 3) as f64);
    let (a, b) = ((col - 1.0) * CELL, (1.0 - row) * CELL);
    [
        f64::from(pos[0]) + 0.5 + right[0] * a + away[0] * b,
        f64::from(pos[1]) + 1.0,
        f64::from(pos[2]) + 0.5 + right[1] * a + away[1] * b,
    ]
}

/// The cell under a point of the table's top, if any.
fn cell_under(pos: [i32; 3], facing: u8, p: [f64; 3]) -> Option<usize> {
    let (right, away) = grid_axes(facing);
    let d = [p[0] - f64::from(pos[0]) - 0.5, p[2] - f64::from(pos[2]) - 0.5];
    let a = d[0] * right[0] + d[1] * right[1];
    let b = d[0] * away[0] + d[1] * away[1];
    let col = ((a / CELL) + 1.0).round();
    let row = (1.0 - b / CELL).round();
    if !(0.0..=2.0).contains(&col) || !(0.0..=2.0).contains(&row) {
        return None;
    }
    let (ca, cb) = ((col - 1.0) * CELL, (1.0 - row) * CELL);
    ((a - ca).abs() <= HALF + 0.01 && (b - cb).abs() <= HALF + 0.01).then_some(row as usize * 3 + col as usize)
}

/// Where the result floats: beside the grid on the camera's right.
fn result_point(pos: [i32; 3], facing: u8, time: f32) -> [f64; 3] {
    let (right, _) = grid_axes(facing);
    [
        f64::from(pos[0]) + 0.5 + right[0] * 0.7,
        f64::from(pos[1]) + 1.2 + f64::from((time * 0.1).sin()) * 0.03,
        f64::from(pos[2]) + 0.5 + right[1] * 0.7,
    ]
}

/// The grid the client was sent for a table.
fn grid_of(pos: [i32; 3]) -> (u8, Vec<Option<ItemStack>>) {
    client(|c| c.tables.iter().find(|t| t.pos == pos).map(|t| (t.facing, t.items.clone())))
        .unwrap_or((0, vec![None; 9]))
}

fn crafted(game: &Game<'_>, grid: &[Option<ItemStack>]) -> Option<(String, ItemStack)> {
    if grid.iter().all(Option::is_none) {
        return None;
    }
    game.recipes().find(grid, 3).map(|(name, result)| (name.to_owned(), result))
}

// ---- the guide ----

/// A result of the guide: its item, its name (and in lower case, for the search), and the
/// crafting recipes making it, by their place in the recipe list.
struct Entry {
    item: String,
    name: String,
    lower: String,
    recipes: Vec<usize>,
}

struct Guide {
    stamp: (usize, usize),
    entries: Arc<Vec<Entry>>,
    /// Which entries the inventory and the grid make now, and for what they were worked out.
    ready: Arc<Vec<bool>>,
    have_sig: u64,
}

static GUIDE: Mutex<Option<Guide>> = Mutex::new(None);

fn ingredients_of(recipe: &Recipe) -> Option<Vec<&Ingredient>> {
    match recipe {
        Recipe::Shaped { cells, .. } => Some(cells.iter().flatten().collect()),
        Recipe::Shapeless { ingredients, .. } => Some(ingredients.iter().collect()),
        Recipe::Special(_) => None,
    }
}

fn result_of(recipe: &Recipe) -> Option<ItemStack> {
    match recipe {
        Recipe::Shaped { result, .. } | Recipe::Shapeless { result, .. } => Some(result.create()).filter(|s| &*s.item != "minecraft:air"),
        Recipe::Special(_) => None,
    }
}

fn name_of(game: &Game<'_>, item: &str) -> String {
    horizon_client::stations::hover_name(&game.pack.names, &ItemStack::new(item, 1))
}

/// Each kind of item with a count (at most 9 of each count: a recipe takes no more).
fn units(game: &Game<'_>, grid: &[Option<ItemStack>]) -> Vec<(ItemStack, u32)> {
    let mut have: Vec<(ItemStack, u32)> = Vec::new();
    for s in game.inventory.iter().take(36).flatten().chain(grid.iter().flatten()) {
        match have.iter_mut().find(|(h, _)| h.same_item_same_components(s)) {
            Some(h) => h.1 = (h.1 + s.count).min(9),
            None => have.push((s.clone(), s.count.min(9))),
        }
    }
    have
}

fn can_make(ingredients: &[&Ingredient], have: &mut [(ItemStack, u32)]) -> bool {
    fn go(i: usize, ingredients: &[&Ingredient], have: &mut [(ItemStack, u32)]) -> bool {
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
    !ingredients.is_empty() && go(0, ingredients, have)
}

fn signature(have: &[(ItemStack, u32)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (s, n) in have {
        for b in s.item.bytes().chain(n.to_le_bytes()) {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn makes(game: &Game<'_>, recipe: usize, have: &mut [(ItemStack, u32)]) -> bool {
    game.recipes().crafting.get(recipe).and_then(|(_, r)| ingredients_of(r)).is_some_and(|i| can_make(&i, have))
}

/// The guide's entries (by name) and which of them the player can make, worked out again only
/// when the recipes or what the player has change.
fn guide(game: &Game<'_>, grid: &[Option<ItemStack>]) -> (Arc<Vec<Entry>>, Arc<Vec<bool>>) {
    let mut cache = GUIDE.lock().unwrap_or_else(|e| e.into_inner());
    let recipes = &game.recipes().crafting;
    let stamp = (recipes.as_ptr() as usize, recipes.len());
    if cache.as_ref().is_none_or(|g| g.stamp != stamp) {
        let mut by_item: HashMap<String, Vec<usize>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for (i, (_, recipe)) in recipes.iter().enumerate() {
            let Some(result) = result_of(recipe) else { continue };
            if ingredients_of(recipe).is_none_or(|g| g.is_empty()) {
                continue;
            }
            let item = result.item.to_string();
            let list = by_item.entry(item.clone()).or_default();
            if list.is_empty() {
                order.push(item);
            }
            list.push(i);
        }
        let mut entries: Vec<Entry> = order
            .into_iter()
            .map(|item| {
                let name = name_of(game, &item);
                Entry { lower: name.to_lowercase(), name, recipes: by_item.remove(&item).unwrap_or_default(), item }
            })
            .collect();
        entries.sort_by(|a, b| a.lower.cmp(&b.lower));
        *cache = Some(Guide { stamp, ready: Arc::new(vec![false; entries.len()]), entries: Arc::new(entries), have_sig: 0 });
    }
    let Some(g) = cache.as_mut() else { return (Arc::default(), Arc::default()) };
    let mut have = units(game, grid);
    let sig = signature(&have);
    if sig != g.have_sig {
        g.have_sig = sig;
        g.ready = Arc::new(g.entries.iter().map(|e| e.recipes.iter().any(|&r| makes(game, r, &mut have))).collect());
    }
    (Arc::clone(&g.entries), Arc::clone(&g.ready))
}

/// The guide's list as shown (entries by their place): the search applied, what can be made first.
fn shown_entries(game: &Game<'_>, grid: &[Option<ItemStack>], search: &str) -> (Arc<Vec<Entry>>, Vec<(usize, bool)>) {
    let (entries, ready) = guide(game, grid);
    let q = search.trim().to_lowercase();
    let mut list: Vec<(usize, bool)> = (0..entries.len())
        .filter(|&i| q.is_empty() || entries[i].lower.contains(&q))
        .map(|i| (i, ready.get(i).copied().unwrap_or(false)))
        .collect();
    list.sort_by_key(|(_, r)| !*r);
    (entries, list)
}

/// The recipe (its name) the guide works with for an item: one the player can make, else the first.
fn recipe_for(game: &Game<'_>, item: &str, grid: &[Option<ItemStack>]) -> Option<String> {
    let (entries, _) = guide(game, grid);
    let e = entries.iter().find(|e| e.item == item)?;
    let mut have = units(game, grid);
    let r = e.recipes.iter().copied().find(|&r| makes(game, r, &mut have)).or(e.recipes.first().copied())?;
    game.recipes().crafting.get(r).map(|(name, _)| name.clone())
}

/// A recipe's ingredient for each cell of the grid (shaped ones from the top left).
fn pattern(game: &Game<'_>, recipe: &str) -> Vec<Option<Ingredient>> {
    let mut cells = vec![None; 9];
    let Some(p) = game.recipes().placement(recipe) else { return cells };
    match p.shaped {
        Some((w, _)) => {
            for (i, slot) in p.slots.iter().enumerate() {
                let (row, col) = (i / w.max(1), i % w.max(1));
                if row < 3 && col < 3 && *slot >= 0 {
                    cells[row * 3 + col] = p.ingredients.get(*slot as usize).cloned();
                }
            }
        }
        None => {
            for (i, slot) in p.slots.iter().enumerate().take(9) {
                if *slot >= 0 {
                    cells[i] = p.ingredients.get(*slot as usize).cloned();
                }
            }
        }
    }
    cells
}

/// An item showing an ingredient, turning through its choices once a second.
fn ingredient_icon(i: &Ingredient, time: f32) -> Option<&str> {
    if i.order.is_empty() {
        return None;
    }
    Some(i.order[(time as usize) % i.order.len()].as_str())
}

// ---- entering and leaving ----

/// Whether a station has room for the scene: the camera's place clear, and nothing solid on a table.
fn room(game: &Game<'_>, kind: Kind, pos: [i32; 3], pose: Pose) -> bool {
    let solid = |x: i32, y: i32, z: i32| game.res.facts().get(game.world.block(x, y, z)).full_collision;
    let cam = pose.0;
    (kind != Kind::Table || !solid(pos[0], pos[1] + 1, pos[2]))
        && !solid(cam[0].floor() as i32, cam[1].floor() as i32, cam[2].floor() as i32)
}

/// `play_mouse`: a right click on a crafting table (not sneaking) opens its scene.
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
    let Some(kind) = station_at(game, hit.pos) else { return false };
    let pos = hit.pos;
    // A lectern with no book takes one as it always does.
    if kind == Kind::Lectern && game.native_block_state_property(game.world.block(pos[0], pos[1], pos[2]), "has_book") != Some("true") {
        return false;
    }
    let facing = open_facing(game, kind, pos, game.yaw);
    let to = framing_of(kind, pos, facing);
    if !room(game, kind, pos, to) {
        return false;
    }
    if !game.native_mod_screen_open(ID) {
        return false;
    }
    let hud = game.native_hud_mode();
    game.native_set_hud_mode(HudMode::AllHidden);
    let from = (eye, game.yaw, game.pitch);
    client(|c| {
        let quick = c.left.is_some_and(|(p, at)| p == pos && at.elapsed().as_secs_f32() < QUICK);
        c.scene = Some(Scene {
            pos,
            kind,
            facing,
            from,
            to,
            started: Instant::now(),
            enter: if quick { ENTER_QUICK } else { ENTER },
            leaving: None,
            carried: None,
            painting: None,
            hud,
            logged: false,
            page: 0,
            chosen: None,
            search: String::new(),
            typing: false,
            flights: Vec::new(),
            refs: [None; 3],
            choice: None,
            strike: None,
            leaf: None,
            turn: None,
            powers: [None; 2],
        });
        c.close = false;
        c.lean = [0.0, 0.0];
        c.view = None;
        if viewed(kind) {
            send(c, OPEN, pos, facing, 0, 0, &[]);
        }
    });
    true
}

/// Once a client tick: what lies on the tables near the player, leaving, looking at oneself, the
/// messages for the server.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    let (close, out, done) = client(|c| {
        let mut done = None;
        if let Some(s) = &mut c.scene {
            if s.leaving.is_none() && (c.close || game.native_mod_screen() != Some(ID)) {
                s.leaving = Some(Instant::now());
                s.carried = None;
                s.typing = false;
            }
            if s.leaving.is_some_and(|at| at.elapsed().as_secs_f32() >= LEAVE) {
                done = Some((s.hud, s.pos, s.kind));
            }
        }
        if let Some((_, pos, kind)) = done {
            c.scene = None;
            c.left = Some((pos, Instant::now()));
            c.view = None;
            if viewed(kind) {
                send(c, CLOSE, pos, 0, 0, 0, &[]);
            }
        }
        (std::mem::take(&mut c.close), std::mem::take(&mut c.out), done)
    });
    if close && game.native_mod_screen() == Some(ID) {
        game.native_mod_screen_close(ID);
    }
    if let Some((hud, _, _)) = done {
        game.native_set_hud_mode(hud);
    }
    for message in out {
        if let Err(e) = game.native_net_send_to_server(CHANNEL, &message) {
            log::debug!("[cinematic_interactions] not sent: {e}");
        }
    }
    // Looking at oneself while Left Alt is held: the camera pulls back and the player is drawn.
    let alt = game.physical_keys.contains(&KeyCode::AltLeft);
    let player = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
    let (peek, restore) = client(|c| {
        let on = alt && c.scene.as_ref().is_some_and(settled);
        c.peek = if on { (c.peek + 0.12).min(1.0) } else { (c.peek - 0.15).max(0.0) };
        if let Some(p) = player {
            c.player = p;
        }
        (c.peek, c.restore)
    });
    if peek > 0.0 && restore.is_none() {
        client(|c| c.restore = Some(game.camera_type));
        game.camera_type = super::perspective::CameraType::ThirdPersonBack;
    } else if peek <= 0.0
        && let Some(back) = restore
    {
        game.camera_type = back;
        client(|c| c.restore = None);
    }
    if !game.native_on(ID) {
        return;
    }
    // What lies on the tables: the scene's every tick, the rest twice a second.
    let scene = client(|c| c.scene.as_ref().map(|s| s.pos));
    let every = game.world.game_time % 10 == 0;
    if !every && scene.is_none() {
        return;
    }
    let Some(p) = player else {
        client(|c| c.tables.clear());
        return;
    };
    let (cx, cz) = ((p[0].floor() as i32) >> 4, (p[2].floor() as i32) >> 4);
    let chunks: Vec<(i32, i32)> = if every {
        let r = 4;
        (cx - r..=cx + r).flat_map(|x| (cz - r..=cz + r).map(move |z| (x, z))).collect()
    } else {
        scene.map(|s| vec![(s[0] >> 4, s[2] >> 4)]).unwrap_or_default()
    };
    let mut seen: Vec<Table> = Vec::new();
    for (x, z) in &chunks {
        if !game.world.chunks.contains_key(&(*x, *z)) {
            continue;
        }
        let host: &mut dyn Host = game;
        if let Ok(Some(t)) = TABLES.get_existing(host, Holder::Chunk([*x, *z])) {
            seen.extend(t.0);
        }
    }
    client(|c| {
        // What was just laid on the scene's table drops onto it.
        let now = Instant::now();
        c.landed.retain(|l| now.duration_since(l.2).as_secs_f32() < 1.0);
        if let Some(at) = c.scene.as_ref().map(|s| s.pos)
            && let Some(new) = seen.iter().find(|t| t.pos == at)
        {
            let old = c.tables.iter().find(|t| t.pos == at);
            for (cell, stack) in new.items.iter().enumerate() {
                let Some(stack) = stack else { continue };
                let was = old.and_then(|t| t.items.get(cell)).and_then(Option::as_ref);
                if was.is_none_or(|w| w.item != stack.item || w.count < stack.count) {
                    c.landed.push((at, cell, now));
                }
            }
        }
        if every {
            c.tables = seen;
        } else {
            c.tables.retain(|t| !chunks.contains(&(t.pos[0] >> 4, t.pos[2] >> 4)));
            c.tables.extend(seen);
        }
    });
}

fn ours(game: &Game<'_>, view: &NativeScreenView<'_>) -> bool {
    view.id == "mod" && game.native_mod_screen() == Some(ID)
}

fn send(c: &mut Client, op: u8, pos: [i32; 3], facing: u8, a: u8, b: u8, extra: &[u8]) {
    let mut m = vec![op];
    for v in pos {
        m.extend_from_slice(&v.to_le_bytes());
    }
    m.extend_from_slice(&[facing, a, b]);
    m.extend_from_slice(extra);
    c.out.push(m);
}

// ---- the scene's parts on the screen ----

/// The letterbox bars' height at full entry.
fn bar(size: [f32; 2]) -> f32 {
    (size[1] * 0.06).round()
}

/// The inventory panel's top left: below the table, above the bottom bar, the hotbar along its foot.
fn panel(size: [f32; 2]) -> [f32; 2] {
    let (w, h) = (SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0);
    [(size[0] - w) / 2.0, size[1] - bar(size) - h - 4.0]
}

fn panel_slot_at(o: [f32; 2], slot: usize) -> [f32; 2] {
    let (row, col) = if slot < 9 { (3usize, slot) } else { ((slot - 9) / 9, (slot - 9) % 9) };
    let gap = if row == 3 { 4.0 } else { 0.0 };
    [o[0] + 4.0 + col as f32 * SLOT, o[1] + 4.0 + row as f32 * SLOT + gap]
}

fn panel_slot(o: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    (0..36).find(|&i| inside(panel_slot_at(o, i), [SLOT, SLOT], mouse))
}

fn slot_middle(o: [f32; 2], slot: usize) -> [f32; 2] {
    let a = panel_slot_at(o, slot);
    [a[0] + SLOT / 2.0, a[1] + SLOT / 2.0]
}

/// The guide's top left, on the left of the screen.
fn book(size: [f32; 2]) -> [f32; 2] {
    [8.0, keep((size[1] - BOOK[1]) / 2.0 - 30.0, 8.0, size[1] - BOOK[1] - 110.0)]
}

fn keep(v: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi).max(lo)
}

fn inside(at: [f32; 2], wh: [f32; 2], m: [f32; 2]) -> bool {
    m[0] >= at[0] && m[1] >= at[1] && m[0] < at[0] + wh[0] && m[1] < at[1] + wh[1]
}

fn search_box(at: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    ([at[0] + 5.0, at[1] + 5.0], [BOOK[0] / 2.0 - 10.0, 12.0])
}

fn recipe_cell(at: [f32; 2], i: usize) -> [f32; 2] {
    [at[0] + 6.0 + (i % COLS) as f32 * ICON, at[1] + 22.0 + (i / COLS) as f32 * ICON]
}

fn arrow_back(at: [f32; 2]) -> [f32; 2] {
    [at[0] + 6.0, at[1] + BOOK[1] - 16.0]
}

fn arrow_next(at: [f32; 2]) -> [f32; 2] {
    [at[0] + BOOK[0] / 2.0 - 22.0, at[1] + BOOK[1] - 16.0]
}

fn layout_button(at: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    ([at[0] + BOOK[0] / 2.0 + 6.0, at[1] + BOOK[1] - 18.0], [44.0, 13.0])
}

fn clear_button(at: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    ([at[0] + BOOK[0] / 2.0 + 54.0, at[1] + BOOK[1] - 18.0], [40.0, 13.0])
}

/// The slot an item would land in: a stack of it with room, else the first empty slot.
fn landing(game: &Game<'_>, item: &str) -> usize {
    (0..36)
        .find(|&i| matches!(game.inventory.get(i), Some(Some(s)) if *s.item == *item && s.count < 64))
        .or_else(|| (0..36).find(|&i| matches!(game.inventory.get(i), Some(None) | None)))
        .unwrap_or(0)
}

/// The crafting table under the pointer near the one open, if any.
fn neighbour_under(game: &Game<'_>, pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3], mouse: [f32; 2]) -> Option<[i32; 3]> {
    let dir = pointer_ray(pose, fov, size, mouse);
    let mut best: Option<(f64, [i32; 3])> = None;
    for dx in -3..=3 {
        for dy in -1..=1 {
            for dz in -3..=3 {
                let p = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                if p == pos || station_at(game, p).is_none() {
                    continue;
                }
                let min = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
                let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
                if let Some(t) = ray_box(pose.0, dir, min, max)
                    && best.as_ref().is_none_or(|b| t < b.0)
                {
                    best = Some((t, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

// ---- drawing the scene ----

fn draw_stack(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, at: [f32; 2]) {
    // The whole stack: its glint, its wear, a banner's patterns.
    game.native_item_stack(batch, stack, at, 16.0);
    if stack.count > 1 {
        let n = stack.count.to_string();
        game.native_text(batch, &n, [at[0] + 17.0 - n.len() as f32 * 6.0, at[1] + 9.0], 0xFFFFFF, true);
    }
}

fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

pub(crate) fn screen_draw(game: &Game<'_>, batch: &mut HudBatch, view: &NativeScreenView<'_>) {
    if !ours(game, view) {
        return;
    }
    let fov = game.native_camera().map_or(70.0, |c| c.fov_degrees);
    let size = [batch.width, batch.height];
    let mouse = view.mouse;
    let Some((pose, pos, kind, facing, carried, entered, k, typing, search, page, chosen)) = client(|c| {
        c.size = size;
        c.fov = fov;
        c.mouse = mouse;
        // The camera leans a little towards the pointer.
        let now = Instant::now();
        let dt = c.lean_at.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        c.lean_at = Some(now);
        let target = [((mouse[0] / size[0].max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0), ((mouse[1] / size[1].max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0)];
        for i in 0..2 {
            c.lean[i] += (target[i] - c.lean[i]) * (dt * 3.0).min(1.0);
        }
        let s = c.scene.as_ref()?;
        Some((scene_pose(c, s), s.pos, s.kind, s.facing, s.carried, settled(s) && c.peek <= 0.0, progress(s), s.typing, s.search.clone(), s.page, s.chosen.clone()))
    }) else {
        return;
    };
    // The letterbox bars.
    let bar = (bar(size) * k).round();
    if bar > 0.0 {
        game.native_fill(batch, [0.0, 0.0, size[0], bar], 0xFF00_0000);
        game.native_fill(batch, [0.0, size[1] - bar, size[0], bar], 0xFF00_0000);
    }
    if !entered {
        if client(|c| c.peek > 0.0) {
            let hint = "Release Left Alt to go back to work";
            let w = hint.len() as f32 * 5.0;
            game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 15.0], 0xE8E8E8, true);
        }
        return;
    }
    let time = game.world.game_time as f32 / 20.0;
    if kind != Kind::Table {
        station_draw(game, batch, pose, fov, size, mouse, (pos, kind, facing, carried), time);
        return;
    }
    let (_, grid) = grid_of(pos);
    let result = crafted(game, &grid);
    let o = panel(size);
    let at = book(size);
    // Once a scene, where its parts landed on the screen (for tests that click them).
    if client(|c| c.scene.as_mut().is_some_and(|s| !std::mem::replace(&mut s.logged, true))) {
        let cells: Vec<String> = (0..9)
            .map(|i| project(pose, fov, size, cell_center(pos, facing, i)).map_or("0 0".into(), |p| format!("{:.0} {:.0}", p[0], p[1])))
            .collect();
        let r = project(pose, fov, size, result_point(pos, facing, time)).unwrap_or_default();
        let l = layout_button(at).0;
        log::info!(
            "[cinematic_interactions] scene at panel {:.0} {:.0} book {:.0} {:.0} result {:.0} {:.0} layout {:.0} {:.0} cells {}",
            o[0], o[1], at[0], at[1], r[0], r[1], l[0] + 20.0, l[1] + 6.0, cells.join(" ")
        );
    }
    // The cell under the pointer.
    let hover_cell = pointer_on(pose, fov, size, mouse, f64::from(pos[1]) + 1.0).and_then(|p| cell_under(pos, facing, p));
    if let Some(cell) = hover_cell
        && let Some(m) = project(pose, fov, size, cell_center(pos, facing, cell))
    {
        let c = if carried.is_some() { 0x50FFE080 } else { 0x40FFFFFF };
        game.native_fill(batch, [m[0] - 12.0, m[1] - 9.0, 24.0, 18.0], c);
    }
    // The recipe picked: its ingredients over the cells still to fill, the missing ones in red.
    let recipe = chosen.as_deref().and_then(|item| recipe_for(game, item, &grid));
    if let Some(recipe) = &recipe {
        let have = units(game, &[]);
        for (cell, ingredient) in pattern(game, recipe).iter().enumerate() {
            let Some(ingredient) = ingredient else { continue };
            if grid[cell].as_ref().is_some_and(|s| ingredient.test(s)) {
                continue;
            }
            let Some(m) = project(pose, fov, size, cell_center(pos, facing, cell)) else { continue };
            let held = have.iter().any(|(s, _)| ingredient.test(s));
            game.native_fill(batch, [m[0] - 9.0, m[1] - 9.0, 18.0, 18.0], if held { 0x6040A040 } else { 0x80C03030 });
            if let Some(item) = ingredient_icon(ingredient, time) {
                game.native_item(batch, item, [m[0] - 6.0, m[1] - 6.0], 12.0);
            }
            game.native_fill(batch, [m[0] - 9.0, m[1] - 9.0, 18.0, 18.0], 0x50000000);
        }
    }
    // The result beside the grid.
    if let Some((_, stack)) = &result
        && let Some(r) = project(pose, fov, size, result_point(pos, facing, time))
    {
        let hover = (mouse[0] - r[0]).hypot(mouse[1] - r[1]) < 20.0;
        let label = if stack.count > 1 { format!("{} x{}", fit(&name_of(game, &stack.item), 18), stack.count) } else { fit(&name_of(game, &stack.item), 22) };
        let w = label.chars().count() as f32 * 5.0 + 10.0;
        game.native_fill(batch, [r[0] - w / 2.0, r[1] + 22.0, w, 12.0], if hover { 0xE0806020 } else { 0xC0201810 });
        game.native_text(batch, &label, [r[0] - w / 2.0 + 5.0, r[1] + 24.0], if hover { 0xFFF0A0 } else { 0xF0F0F0 }, true);
    }
    // The inventory panel.
    game.native_fill(batch, [o[0], o[1], SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0], 0xC8181410);
    let hover_slot = panel_slot(o, mouse);
    for i in 0..36 {
        let a = panel_slot_at(o, i);
        let color = if carried == Some(i) {
            0xF0D8A848
        } else if hover_slot == Some(i) {
            0xE0705A44
        } else {
            0xE0403428
        };
        game.native_fill(batch, [a[0] + 1.0, a[1] + 1.0, SLOT - 2.0, SLOT - 2.0], color);
        if carried != Some(i)
            && let Some(Some(stack)) = game.inventory.get(i)
        {
            draw_stack(game, batch, stack, [a[0] + 1.0, a[1] + 1.0]);
        }
    }
    draw_guide(game, batch, at, mouse, &grid, &search, typing, page, chosen.as_deref(), recipe.as_deref(), time);
    // Items flying between the panel and the table.
    client(|c| {
        // The hand carries what goes to the station; only what comes back flies.
        let carries = c.hand_out;
        if let Some(s) = c.scene.as_mut() {
            s.flights.retain(|f| f.start.elapsed().as_secs_f32() < FLIGHT);
            for f in &s.flights {
                if carries && inside(panel(size), [SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0], f.from) {
                    continue;
                }
                let t = smooth(f.start.elapsed().as_secs_f32() / FLIGHT);
                let arc = (t * std::f32::consts::PI).sin() * 30.0;
                let p = [f.from[0] + (f.to[0] - f.from[0]) * t - 8.0, f.from[1] + (f.to[1] - f.from[1]) * t - 8.0 - arc];
                game.native_item(batch, &f.item, p, 16.0);
            }
        }
    });
    // The stack picked up, on the pointer.
    if let Some(slot) = carried
        && !client(|c| c.hand_out)
        && let Some(Some(stack)) = game.inventory.get(slot)
    {
        draw_stack(game, batch, stack, [mouse[0] - 8.0, mouse[1] - 8.0]);
    }
    let hint = if typing {
        "Type to search  -  Enter or Esc to stop"
    } else {
        "Pick an item, click a cell to lay it  -  click the result to craft  -  hold Left Alt to look at yourself  -  Esc leaves"
    };
    let w = hint.len() as f32 * 5.0;
    game.native_fill(batch, [(size[0] - w) / 2.0 - 6.0, size[1] - 17.0, w + 12.0, 13.0], 0x80000000);
    game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 15.0], 0xE8E8E8, true);
}

#[allow(clippy::too_many_arguments)]
fn draw_guide(
    game: &Game<'_>,
    batch: &mut HudBatch,
    at: [f32; 2],
    mouse: [f32; 2],
    grid: &[Option<ItemStack>],
    search: &str,
    typing: bool,
    page: usize,
    chosen: Option<&str>,
    recipe: Option<&str>,
    time: f32,
) {
    // The open book: covers, two pages and the spine.
    game.native_fill(batch, [at[0] - 4.0, at[1] - 4.0, BOOK[0] + 8.0, BOOK[1] + 8.0], 0xFF5A3A18);
    game.native_fill(batch, [at[0] - 2.0, at[1] - 2.0, BOOK[0] + 4.0, BOOK[1] + 4.0], 0xFF8A5E2A);
    game.native_fill(batch, [at[0], at[1], BOOK[0], BOOK[1]], 0xFFF6ECD4);
    game.native_fill(batch, [at[0] + BOOK[0] / 2.0 - 1.0, at[1], 2.0, BOOK[1]], 0xFFD6C4A0);
    // The search box.
    let (sb, sw) = search_box(at);
    game.native_fill(batch, [sb[0], sb[1], sw[0], sw[1]], if typing { 0xFF302418 } else { 0xFF5A4A34 });
    let shown = if search.is_empty() && !typing { "Search...".to_owned() } else { fit(search, 15) };
    let caret = if typing && (time * 2.0) as i32 % 2 == 0 { "_" } else { "" };
    game.native_text(batch, &format!("{shown}{caret}"), [sb[0] + 3.0, sb[1] + 2.0], if search.is_empty() && !typing { 0xA09080 } else { 0xFFFFFF }, false);
    let (entries, list) = shown_entries(game, grid, search);
    let pages = list.len().div_ceil(PER_PAGE).max(1);
    let page = page.min(pages - 1);
    for (i, (index, ready)) in list.iter().skip(page * PER_PAGE).take(PER_PAGE).enumerate() {
        let e = &entries[*index];
        let cell = recipe_cell(at, i);
        let picked = chosen == Some(e.item.as_str());
        let hover = inside(cell, [ICON, ICON], mouse);
        let bg = match (picked, *ready) {
            (true, _) => 0xFFE8C060,
            (false, true) => if hover { 0xFFDCE8B8 } else { 0xFFE6EEC8 },
            (false, false) => if hover { 0xFFE0D4BC } else { 0xFFEADCC4 },
        };
        game.native_fill(batch, [cell[0], cell[1], ICON - 2.0, ICON - 2.0], bg);
        game.native_item(batch, &e.item, [cell[0] + 1.0, cell[1] + 1.0], 16.0);
        if !ready {
            game.native_fill(batch, [cell[0], cell[1], ICON - 2.0, ICON - 2.0], 0x70F6ECD4);
        }
        if hover {
            let name = fit(&e.name, 24);
            let w = name.chars().count() as f32 * 5.0 + 8.0;
            game.native_fill(batch, [mouse[0] + 8.0, mouse[1] - 14.0, w, 12.0], 0xF0201810);
            game.native_text(batch, &name, [mouse[0] + 12.0, mouse[1] - 12.0], 0xFFFFFF, true);
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
    // The right page: the recipe picked, its pattern, what is missing.
    let x = at[0] + BOOK[0] / 2.0 + 6.0;
    let (Some(item), Some(recipe)) = (chosen, recipe) else {
        game.native_text(batch, "Pick a recipe", [x, at[1] + 40.0], 0x8A6A40, false);
        game.native_text(batch, "to lay it out", [x, at[1] + 52.0], 0x8A6A40, false);
        return;
    };
    game.native_item(batch, item, [x, at[1] + 5.0], 20.0);
    game.native_text(batch, &fit(&name_of(game, item), 14), [x + 24.0, at[1] + 11.0], 0x40301A, false);
    let have = units(game, grid);
    let mut missing = 0;
    for (cell, ingredient) in pattern(game, recipe).iter().enumerate() {
        let p = [x + (cell % 3) as f32 * 18.0, at[1] + 32.0 + (cell / 3) as f32 * 18.0];
        let Some(ingredient) = ingredient else {
            game.native_fill(batch, [p[0], p[1], 17.0, 17.0], 0x30806040);
            continue;
        };
        let held = have.iter().any(|(s, _)| ingredient.test(s));
        if !held {
            missing += 1;
        }
        game.native_fill(batch, [p[0], p[1], 17.0, 17.0], if held { 0x8060B040 } else { 0x90D04030 });
        if let Some(icon) = ingredient_icon(ingredient, time) {
            game.native_item(batch, icon, [p[0] + 0.5, p[1] + 0.5], 16.0);
        }
    }
    let status = if missing == 0 { "All at hand".to_owned() } else { format!("{missing} missing") };
    game.native_text(batch, &status, [x, at[1] + 88.0], if missing == 0 { 0x307020 } else { 0xA03020 }, false);
    for (button, label) in [(layout_button(at), "Lay out"), (clear_button(at), "Clear")] {
        let (b, wh) = button;
        let hover = inside(b, wh, mouse);
        game.native_fill(batch, [b[0], b[1], wh[0], wh[1]], if hover { 0xFF8A5E2A } else { 0xFF6A4A20 });
        game.native_text(batch, label, [b[0] + 4.0, b[1] + 3.0], 0xFFF0D8, false);
    }
}

// ---- input ----

pub(crate) fn press(game: &Game<'_>, view: &NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    let mouse = view.mouse;
    let shift = modifiers & 1 != 0;
    let time = game.world.game_time as f32 / 20.0;
    let Some((pos, kind, ready)) = client(|c| c.scene.as_ref().map(|s| (s.pos, s.kind, settled(s) && c.peek <= 0.0))) else { return };
    if !ready {
        return;
    }
    // The hand presses down where the pointer is.
    client(|c| {
        c.mouse = mouse;
        c.tap = Some(Instant::now());
    });
    if kind != Kind::Table {
        station_press(game, mouse, button, pos);
        return;
    }
    let (_, grid) = grid_of(pos);
    let result = crafted(game, &grid);
    let (search, chosen) = client(|c| c.scene.as_ref().map(|s| (s.search.clone(), s.chosen.clone()))).unwrap_or_default();
    let (entries, list) = shown_entries(game, &grid, &search);
    let recipe = chosen.as_deref().and_then(|item| recipe_for(game, item, &grid));
    let (pose, size, fov) = client(|c| (c.scene.as_ref().map(|s| scene_pose(c, s)), c.size, c.fov));
    let Some(pose) = pose else { return };
    let neighbour = neighbour_under(game, pose, fov, size, pos, mouse);
    let on_top = pointer_on(pose, fov, size, mouse, f64::from(pos[1]) + 1.0);
    client(|c| {
        let Some(s) = c.scene.as_mut() else { return };
        let facing = s.facing;
        let o = panel(size);
        let at = book(size);
        let (sb, sw) = search_box(at);
        s.typing = inside(sb, sw, mouse);
        if s.typing {
            return;
        }
        // The panel: pick a stack up, or put it back.
        if let Some(slot) = panel_slot(o, mouse) {
            s.carried = if s.carried == Some(slot) || game.inventory.get(slot).is_none_or(Option::is_none) { None } else { Some(slot) };
            return;
        }
        // The guide: pages, recipes, laying out, clearing.
        if inside([at[0] - 4.0, at[1] - 4.0], [BOOK[0] + 8.0, BOOK[1] + 8.0], mouse) {
            let pages = list.len().div_ceil(PER_PAGE).max(1);
            if inside(arrow_back(at), [16.0, 14.0], mouse) {
                s.page = s.page.saturating_sub(1);
            } else if inside(arrow_next(at), [16.0, 14.0], mouse) {
                s.page = (s.page + 1).min(pages - 1);
            } else if let Some(i) = (0..PER_PAGE).find(|&i| inside(recipe_cell(at, i), [ICON, ICON], mouse)) {
                if let Some((index, _)) = list.get(s.page.min(pages - 1) * PER_PAGE + i) {
                    let item = &entries[*index].item;
                    s.chosen = if s.chosen.as_deref() == Some(item.as_str()) { None } else { Some(item.clone()) };
                }
            } else if inside(layout_button(at).0, layout_button(at).1, mouse) {
                if let Some(recipe) = &recipe {
                    // The ingredients fly from the inventory to their cells.
                    for (cell, ingredient) in pattern(game, recipe).iter().enumerate() {
                        let Some(ingredient) = ingredient else { continue };
                        let Some(slot) = (0..36).find(|&i| matches!(game.inventory.get(i), Some(Some(st)) if ingredient.test(st))) else {
                            continue;
                        };
                        if let (Some(Some(st)), Some(to)) = (game.inventory.get(slot), project(pose, fov, size, cell_center(pos, facing, cell))) {
                            s.flights.push(Flight { item: st.item.to_string(), from: slot_middle(o, slot), to, start: Instant::now() });
                        }
                    }
                    let sets = if shift { 64 } else { 1 };
                    send_layout(c, pos, facing, sets, recipe);
                }
            } else if inside(clear_button(at).0, clear_button(at).1, mouse) {
                for (cell, stack) in grid.iter().enumerate() {
                    if let (Some(st), Some(from)) = (stack, project(pose, fov, size, cell_center(pos, facing, cell))) {
                        s.flights.push(Flight { item: st.item.to_string(), from, to: slot_middle(o, landing(game, &st.item)), start: Instant::now() });
                    }
                }
                send(c, CLEAR, pos, facing, 0, 0, &[]);
            }
            return;
        }
        // The result: craft it.
        if let Some((_, stack)) = &result
            && let Some(r) = project(pose, fov, size, result_point(pos, facing, time))
            && (mouse[0] - r[0]).hypot(mouse[1] - r[1]) < 20.0
        {
            s.flights.push(Flight { item: stack.item.to_string(), from: r, to: slot_middle(o, landing(game, &stack.item)), start: Instant::now() });
            let times = if shift || button == 1 { 64 } else { 1 };
            send(c, CRAFT, pos, facing, times, 0, &[]);
            return;
        }
        // A cell of the grid.
        if let Some(cell) = on_top.and_then(|p| cell_under(pos, facing, p)) {
            let middle = project(pose, fov, size, cell_center(pos, facing, cell)).unwrap_or(mouse);
            match s.carried {
                Some(slot) => {
                    let one = button == 1;
                    let Some(Some(stack)) = game.inventory.get(slot) else {
                        s.carried = None;
                        return;
                    };
                    s.flights.push(Flight { item: stack.item.to_string(), from: slot_middle(o, slot), to: middle, start: Instant::now() });
                    if !one || stack.count <= 1 {
                        s.carried = None;
                    }
                    if one {
                        s.painting = Some(vec![cell]);
                    }
                    send(c, PLACE, pos, facing, cell as u8, slot as u8, &[if one { 1 } else { 255 }]);
                }
                None => {
                    if let Some(stack) = &grid[cell] {
                        s.flights.push(Flight { item: stack.item.to_string(), from: middle, to: slot_middle(o, landing(game, &stack.item)), start: Instant::now() });
                        send(c, TAKE, pos, facing, cell as u8, if button == 1 { 1 } else { 255 }, &[]);
                    }
                }
            }
            return;
        }
        // Another station nearby: glide over to it.
        if let Some(npos) = neighbour {
            if station_at(game, npos) != Some(Kind::Table) {
                glide(c, game, npos, pose);
                return;
            }
            let (stored, items) = client_grid(&c.tables, npos);
            let nfacing = if items.iter().any(Option::is_some) { stored } else { s.facing };
            s.from = pose;
            s.to = framing(npos, nfacing);
            s.started = Instant::now();
            s.enter = ENTER;
            s.pos = npos;
            s.facing = nfacing;
            s.carried = None;
            s.painting = None;
            s.logged = false;
        }
    });
}

/// Glides the camera from the station in view to another one nearby, and tells the server.
fn glide(c: &mut Client, game: &Game<'_>, npos: [i32; 3], pose: Pose) {
    let Some(kind) = station_at(game, npos) else { return };
    let Some(s) = c.scene.as_ref() else { return };
    let (old_pos, old_kind) = (s.pos, s.kind);
    // A table's side is read from the client's own copy: `open_facing` would ask for the
    // client again, which this caller already holds.
    let facing = if kind == Kind::Table {
        let (stored, items) = client_grid(&c.tables, npos);
        if items.iter().any(Option::is_some) { stored } else { facing_of(s.to.1) }
    } else {
        open_facing(game, kind, npos, s.to.1)
    };
    // A lectern with no book is not worked here.
    if kind == Kind::Lectern && game.native_block_state_property(game.world.block(npos[0], npos[1], npos[2]), "has_book") != Some("true") {
        return;
    }
    if viewed(old_kind) {
        send(c, CLOSE, old_pos, 0, 0, 0, &[]);
    }
    if let Some(s) = c.scene.as_mut() {
        s.from = pose;
        s.to = framing_of(kind, npos, facing);
        s.started = Instant::now();
        s.enter = ENTER;
        s.pos = npos;
        s.kind = kind;
        s.facing = facing;
        s.carried = None;
        s.painting = None;
        s.logged = false;
        s.chosen = None;
        s.refs = [None; 3];
        s.choice = None;
        s.search.clear();
        s.typing = false;
        s.strike = None;
        s.leaf = None;
        s.turn = None;
        s.powers = [None; 2];
    }
    c.view = None;
    if viewed(kind) {
        send(c, OPEN, npos, facing, 0, 0, &[]);
    }
}

fn client_grid(tables: &[Table], pos: [i32; 3]) -> (u8, Vec<Option<ItemStack>>) {
    tables.iter().find(|t| t.pos == pos).map_or((0, vec![None; 9]), |t| (t.facing, t.items.clone()))
}

fn send_layout(c: &mut Client, pos: [i32; 3], facing: u8, sets: u8, recipe: &str) {
    let bytes = recipe.as_bytes();
    if bytes.len() <= 200 {
        send(c, LAYOUT, pos, facing, sets, 0, bytes);
    }
}

pub(crate) fn release(game: &Game<'_>, view: &NativeScreenView<'_>, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    client(|c| {
        if let Some(s) = c.scene.as_mut() {
            s.painting = None;
        }
    });
}

/// Dragging with the right button down and a stack picked: one in each cell passed.
pub(crate) fn drag(game: &Game<'_>, view: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    if button != 1 {
        return;
    }
    let mouse = view.mouse;
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(pose) = c.scene.as_ref().map(|s| scene_pose(c, s)) else { return };
        let Some(s) = c.scene.as_mut() else { return };
        if s.kind != Kind::Table {
            return;
        }
        let (Some(slot), Some(done)) = (s.carried, s.painting.as_mut()) else { return };
        let Some(cell) = pointer_on(pose, fov, size, mouse, f64::from(s.pos[1]) + 1.0).and_then(|p| cell_under(s.pos, s.facing, p)) else {
            return;
        };
        if done.contains(&cell) {
            return;
        }
        let Some(Some(stack)) = game.inventory.get(slot) else { return };
        // What was laid already has left the stack.
        if stack.count as usize <= done.len() {
            return;
        }
        done.push(cell);
        let (pos, facing) = (s.pos, s.facing);
        if let Some(to) = project(pose, fov, size, cell_center(pos, facing, cell)) {
            let o = panel(size);
            s.flights.push(Flight { item: stack.item.to_string(), from: slot_middle(o, slot), to, start: Instant::now() });
        }
        c.tap = Some(Instant::now());
        send(c, PLACE, pos, facing, cell as u8, slot as u8, &[1]);
    });
}

pub(crate) fn key(game: &Game<'_>, view: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    let typing = client(|c| c.scene.as_ref().is_some_and(|s| s.typing));
    if typing {
        *canceled = true;
        client(|c| {
            if let Some(s) = c.scene.as_mut() {
                match key {
                    "key.keyboard.backspace" => {
                        s.search.pop();
                        s.page = 0;
                    }
                    "key.keyboard.enter" | "key.keyboard.escape" | "key.keyboard.keypad.enter" => s.typing = false,
                    _ => {}
                }
            }
        });
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

pub(crate) fn char_typed(game: &Game<'_>, view: &NativeScreenView<'_>, ch: char, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    client(|c| {
        if let Some(s) = c.scene.as_mut()
            && s.typing
            && !ch.is_control()
            && s.search.chars().count() < 24
        {
            s.search.push(ch);
            s.page = 0;
        }
    });
}

// ---- drawing in the world ----

/// How to draw an item `size` blocks across on a surface: its scale, how far up it sits, and
/// whether it is a block model (stood up) rather than a flat one (laid down). From its model's own
/// bounds, so a cube, a stick and a big modded item come out alike.
fn fitted(game: &Game<'_>, stack: &ItemStack, size: f32, seed: i32) -> (f32, f32, bool) {
    let state = game.resolve_item(stack, DisplayContext::Fixed, None, seed);
    let (lo, hi) = state.bounding_box();
    let d = hi - lo;
    if !d.is_finite() {
        return (0.3, 0.0, false);
    }
    if state.uses_block_light() {
        let s = size * 0.7 / d.x.max(d.y).max(d.z).max(0.01);
        (s, -lo.y * s, true)
    } else {
        let s = size / d.x.max(d.y).max(0.01);
        (s, hi.z.max(0.0) * s, false)
    }
}

/// What lies on the tables near the player, on their tops; over the table in the scene, its result.
pub(crate) fn world_submit(game: &Game<'_>, draw: &mut super::native_world_draw::NativeWorldDraw) {
    if !game.native_on(ID) {
        return;
    }
    draw_hand(game, draw);
    let (tables, scene, landed) = client(|c| {
        (c.tables.clone(), c.scene.as_ref().filter(|s| s.leaving.is_none() && s.kind == Kind::Table).map(|s| (s.pos, s.facing)), c.landed.clone())
    });
    let time = game.world.game_time as f32 + draw.partial;
    for table in &tables {
        if !is_table(game, table.pos) {
            continue;
        }
        let pos = table.pos;
        let light = game.world.light(pos[0], pos[1] + 1, pos[2]);
        for (cell, stack) in table.items.iter().enumerate() {
            let Some(stack) = stack else { continue };
            let c = cell_center(pos, table.facing, cell);
            let seed = (cell * 3) as i32;
            let (scale, lift, block) = fitted(game, stack, 0.19, seed);
            // What the hand just let go of drops the last of the way.
            let fall = landed.iter().rev().find(|l| l.0 == pos && l.1 == cell).map_or(0.0, |l| {
                let t = l.2.elapsed().as_secs_f32() / 0.2;
                if t < 1.0 { f64::from((1.0 - t) * (1.0 - t)) * 0.09 } else { 0.0 }
            });
            // A pile of flat items shows a few lying on each other; a block, one.
            let copies = match (block, stack.count) {
                (true, _) | (false, 0 | 1) => 1,
                (false, 2..=15) => 2,
                (false, _) => 3,
            };
            for n in 0..copies {
                draw.push();
                let fall = if n + 1 == copies { fall } else { 0.0 };
                if draw.translate_world([c[0], c[1] + 0.002 + f64::from(lift) + n as f64 * 0.01 + fall, c[2]]) {
                    draw.light = horizon_client::block_entity_render::vertex_light(light);
                    let jitter = if block { 0.0 } else { ((cell * 37 + n * 23) % 30) as f32 - 15.0 };
                    draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(table.facing) - jitter, [0.0, 0.0, 0.0]);
                    if !block {
                        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
                    }
                    draw.scale([scale, scale, scale]);
                    game.native_draw_item(&mut *draw, stack, DisplayContext::Fixed, None, seed);
                }
                draw.pop();
            }
        }
    }
    // A station worked shows what it holds; a table, its result floating beside the grid.
    if let Some((kind, pos, facing)) =
        client(|c| c.scene.as_ref().filter(|s| s.leaving.is_none() && from_inventory(s.kind)).map(|s| (s.kind, s.pos, s.facing)))
    {
        draw_laid(game, draw, kind, pos, facing);
        return;
    }
    let station = client(|c| {
        let s = c.scene.as_ref().filter(|s| s.leaving.is_none() && s.kind != Kind::Table)?;
        Some((s.kind, s.pos, s.facing, c.view.clone().filter(|v| v.pos == s.pos)?, s.strike))
    });
    if let Some((kind, pos, facing, view, strike)) = station {
        draw_station(game, draw, kind, pos, facing, &view, strike);
        return;
    }
    let Some((pos, facing)) = scene else { return };
    let (_, grid) = client_grid(&tables, pos);
    let Some((_, result)) = crafted(game, &grid) else { return };
    let p = result_point(pos, facing, time / 20.0);
    draw.push();
    if draw.translate_world(p) {
        draw.light = horizon_client::block_entity_render::vertex_light(game.world.light(pos[0], pos[1] + 1, pos[2]));
        draw.rotate_about([0.0, 1.0, 0.0], time * 3.0, [0.0, 0.0, 0.0]);
        let (scale, _, _) = fitted(game, &result, 0.32, 99);
        draw.scale([scale, scale, scale]);
        game.native_draw_item(&mut *draw, &result, DisplayContext::Fixed, None, 99);
    }
    draw.pop();
}

// ---- the hand ----

/// Where the player's hand goes in the scene: over the part of the station under the pointer, or
/// to the hammer's handle while it strikes. None rests it out of sight.
fn hand_goal(game: &Game<'_>, c: &Client, s: &Scene, pose: Pose) -> Option<[f64; 3]> {
    if !settled(s) || c.peek > 0.0 || s.typing || s.kind == Kind::Lectern {
        return None;
    }
    let (size, fov, mouse) = (c.size, c.fov, c.mouse);
    if size[0] < 1.0 {
        return None;
    }
    let time = game.world.game_time as f32 / 20.0;
    // A little short of a point, on the eye's side of it.
    let towards = |p: [f64; 3], by: f64| {
        let d = [pose.0[0] - p[0], pose.0[1] - p[1], pose.0[2] - p[2]];
        let len = dot(d, d).sqrt().max(1e-6);
        [p[0] + d[0] / len * by, p[1] + d[1] / len * by, p[2] + d[2] / len * by]
    };
    if s.kind == Kind::Anvil
        && let Some(at) = s.strike
        && at.elapsed().as_secs_f32() < 0.45
    {
        let item = spots(s.kind, s.pos, s.facing).first()?.0;
        let yaw = f64::from(facing_yaw(s.facing)).to_radians();
        return Some([item[0] - yaw.cos() * 0.2, item[1] + 0.2, item[2] - yaw.sin() * 0.2]);
    }
    if inside(panel(size), [SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0], mouse) {
        return None;
    }
    let near = |p: [f64; 3]| project(pose, fov, size, p).is_some_and(|q| (q[0] - mouse[0]).hypot(q[1] - mouse[1]) < 30.0);
    if matches!(s.kind, Kind::Table | Kind::Crafter) {
        if s.kind == Kind::Table && inside(book(size), BOOK, mouse) {
            return None;
        }
        let result = result_point(s.pos, s.facing, time);
        if near(result) {
            return Some(towards(result, 0.14));
        }
        let top = f64::from(s.pos[1]) + 1.0;
        let p = pointer_on(pose, fov, size, mouse, top)?;
        let (dx, dz) = (p[0] - f64::from(s.pos[0]) - 0.5, p[2] - f64::from(s.pos[2]) - 0.5);
        return (dx.abs() < 0.6 && dz.abs() < 0.6).then_some([p[0], top + 0.11, p[2]]);
    }
    let list = spots(s.kind, s.pos, s.facing);
    if let Some(i) = spot_under(pose, fov, size, &list, mouse) {
        return Some(towards(list[i].0, 0.14));
    }
    let result = result_spot(s.kind, s.pos, s.facing, time);
    if near(result) {
        return Some(towards(result, 0.14));
    }
    // Anywhere else on the station: just off its surface.
    let ray = pointer_ray(pose, fov, size, mouse);
    let min = [f64::from(s.pos[0]), f64::from(s.pos[1]), f64::from(s.pos[2])];
    let k = ray_box(pose.0, ray, min, [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0])?;
    Some(towards([pose.0[0] + ray[0] * k, pose.0[1] + ray[1] * k, pose.0[2] + ray[2] * k], 0.12))
}

/// The player's own arm, from the lower corner of the view to where the hand is: it follows the
/// pointer over the station, holds the stack picked up, and presses down on a click.
fn draw_hand(game: &Game<'_>, draw: &mut super::native_world_draw::NativeWorldDraw) {
    use glam::{DVec3, Mat4, Vec3};
    let left = game.settings.main_hand_left;
    let side = if left { -1.0 } else { 1.0 };
    let Some((pose, hand, carried, flat)) = client(|c| {
        let s = c.scene.as_ref()?;
        if s.leaving.is_some() {
            c.hand = None;
            c.hand_out = false;
            return None;
        }
        let pose = scene_pose(c, s);
        let goal = hand_goal(game, c, s, pose);
        let (carried, flat) = (s.carried, matches!(s.kind, Kind::Table | Kind::Crafter));
        let (f, r, u) = basis(pose.1, pose.2);
        let rest = [0, 1, 2].map(|i| pose.0[i] + f[i] * 0.45 + r[i] * 0.5 * side - u[i] * 1.05);
        let now = Instant::now();
        let dt = c.hand_at.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        c.hand_at = Some(now);
        let (from, to) = (c.hand.unwrap_or(rest), goal.unwrap_or(rest));
        let k = f64::from(1.0 - (-dt * 13.0).exp());
        let mut hand = [0, 1, 2].map(|i| from[i] + (to[i] - from[i]) * k);
        c.hand = Some(hand);
        c.hand_out = goal.is_some();
        // At rest it is out of sight: nothing to draw.
        if goal.is_none() && (0..3).map(|i| (hand[i] - rest[i]).powi(2)).sum::<f64>() < 0.0009 {
            return None;
        }
        // A click presses it down, away from the eye.
        if let Some(t) = c.tap.map(|t| t.elapsed().as_secs_f32() / 0.22).filter(|t| *t < 1.0) {
            let dip = f64::from((t * std::f32::consts::PI).sin()) * 0.075;
            hand = [0, 1, 2].map(|i| hand[i] + f[i] * dip);
        }
        Some((pose, hand, carried, flat))
    }) else {
        return;
    };
    let slim = game.player_model.as_ref().is_some_and(|m| m.slim);
    let Some(arm) = game.res.player_arm(slim, left).as_ref() else { return };
    let (f, r, u) = basis(pose.1, pose.2);
    let v = |a: [f64; 3]| DVec3::from_array(a);
    let (eye, f, r, u, hand) = (v(pose.0), v(f), v(r), v(u), v(hand));
    // The shoulder: below the view, on the hand's side.
    let shoulder = eye + f * 0.5 + r * (0.46 * side) - u * 0.62;
    let reach = hand - shoulder;
    let len = reach.length();
    if len < 0.05 {
        return;
    }
    let d = reach / len;
    // The back of the hand up, as the eye sees it.
    let up = (u - d * u.dot(d)).try_normalize().unwrap_or(f);
    let x = if left { up } else { -up };
    let z = x.cross(d);
    // The palm, in the arm's own frame: 9 of its 12 texels down from the shoulder's pivot.
    let across = if slim { 0.5 } else { 1.0 } * if left { 1.0 } else { -1.0 };
    let palm = Vec3::new((arm.pose[0] + across) / 16.0, (arm.pose[1] + 9.0) / 16.0, arm.pose[2] / 16.0);
    // The arm at the size of a hand on the station, lying straight along its reach.
    let size = 0.62f32;
    let block = [hand.x.floor() as i32, hand.y.floor() as i32, hand.z.floor() as i32];
    let light = horizon_client::block_entity_render::vertex_light(game.world.light(block[0], block[1], block[2]));
    let sprites = &game.gui.atlas.sprites;
    let skin = game
        .player_model
        .as_ref()
        .and_then(|m| m.skin.as_deref())
        .filter(|skin| sprites.contains_key(*skin))
        .unwrap_or("entity/player/wide/steve");
    let Some(rect) = sprites.get(skin).copied() else { return };
    draw.push();
    if draw.translate_world(hand.to_array()) {
        let (xv, dv, zv) = (x.as_vec3(), d.as_vec3(), z.as_vec3());
        let turn = Mat4::from_cols(xv.extend(0.0), dv.extend(0.0), zv.extend(0.0), glam::Vec4::W);
        let m = draw.pose() * turn * Mat4::from_scale(Vec3::splat(size)) * Mat4::from_translation(-palm);
        let straight = |p: &horizon_client::mobs::Part| {
            let mut pose = p.pose;
            pose[3] = 0.0;
            pose[4] = 0.0;
            pose[5] = 0.0;
            pose
        };
        if m.is_finite() {
            horizon_client::mobs::render_model(arm, m, &straight, [rect.u0, rect.v0, rect.u1, rect.v1], light, 0, draw.vertices_mut());
        }
        // Its sleeve goes on behind it as far as out of sight: the skin's rows at the shoulder,
        // drawn out along the reach (the arm itself, then its outer layer).
        draw.light = light;
        let wide = if slim { 3.0f32 } else { 4.0 };
        let far = -(len as f32 + 0.3);
        let layers = [(if left { [32.0f32, 48.0] } else { [40.0, 16.0] }, 0.0f32), (if left { [48.0, 48.0] } else { [40.0, 32.0] }, 0.25)];
        for (origin, grow) in layers {
            let near = -(11.0 + grow) / 16.0 * size;
            if far >= near {
                break;
            }
            let (hx, hz) = ((wide / 2.0 + grow) / 16.0 * size, (2.0 + grow) / 16.0 * size);
            // A box's four sides as a skin lays them out: its two side faces and its front and back.
            let faces = [(xv, hx, hz, 0.0, 4.0), (-zv, hz, hx, 4.0, wide), (-xv, hx, hz, 4.0 + wide, 4.0), (zv, hz, hx, 8.0 + wide, wide)];
            for (n, off, half, u, w) in faces {
                let t = dv.cross(n);
                let corner = |side: f32, y: f32| (n * off + t * (half * side) + dv * y).to_array();
                let v = origin[1] + 5.0;
                let uv = [(origin[0] + u) / 4.0, v / 4.0, (origin[0] + u + w) / 4.0, (v + 1.0) / 4.0];
                game.native_draw_quad(&mut *draw, skin, [corner(-1.0, near), corner(-1.0, far), corner(1.0, far), corner(1.0, near)], uv, 0xFF_FFFF);
            }
        }
    }
    draw.pop();
    // What it holds, at its fingers.
    let Some(stack) = carried.and_then(|slot| game.inventory.get(slot).cloned().flatten()) else { return };
    let (scale, _, block_model) = fitted(game, &stack, 0.15, 41);
    let at = hand + d * (f64::from(size) * 0.25 + 0.04) - up * 0.03;
    draw.push();
    if draw.translate_world(at.to_array()) {
        draw.light = light;
        draw.rotate_about([0.0, 1.0, 0.0], 180.0 - pose.1, [0.0, 0.0, 0.0]);
        if flat && !block_model {
            draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
        }
        draw.scale([scale, scale, scale]);
        game.native_draw_item(&mut *draw, &stack, DisplayContext::Fixed, None, 41);
    }
    draw.pop();
}

// ---- the server ----

static SERVER: Mutex<Option<HashMap<u128, Vec<Vec<u8>>>>> = Mutex::new(None);

/// `net_from_client`: a table action, done on the player's next tick (where their inventory is).
pub(crate) fn net_from_client(player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() < 16 || data.len() > 260 {
        return;
    }
    let mut queue = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let list = queue.get_or_insert_with(HashMap::new).entry(player).or_default();
    if list.len() < 64 {
        list.push(data.to_vec());
    }
}

pub(crate) fn player_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let Some(uuid) = game.native_acting_player().map(|a| a.uuid) else { return };
    let acts = SERVER.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|q| q.remove(&uuid)).unwrap_or_default();
    for a in acts {
        let pos = [0, 1, 2].map(|i| i32::from_le_bytes(a[1 + i * 4..5 + i * 4].try_into().unwrap_or_default()));
        act(game, uuid, a[0], pos, a[13] & 3, a[14], a[15], &a[16..]);
    }
    send_view(game, uuid);
}

fn act(game: &mut Game<'_>, uuid: u128, op: u8, pos: [i32; 3], facing: u8, a: u8, b: u8, extra: &[u8]) {
    if op == CLOSE {
        unwatch(uuid);
        return;
    }
    let Some(kind) = station_at(game, pos) else { return };
    let Some(eye) = game.eye_position() else { return };
    let d2 = (0..3).map(|i| (eye[i] - (f64::from(pos[i]) + 0.5)).powi(2)).sum::<f64>();
    if d2 > 64.0 {
        return;
    }
    if kind != Kind::Table {
        station_act(game, uuid, kind, op, pos, a, b, extra);
        game.guest_inventory_resend = true;
        return;
    }
    let mut table = table_at(game, pos);
    if table.items.iter().all(Option::is_none) {
        table.facing = facing;
    }
    match op {
        PLACE => place(game, &mut table, usize::from(a), usize::from(b), extra.first().copied().unwrap_or(1)),
        TAKE => {
            let cell = usize::from(a);
            if let Some(Some(stack)) = table.items.get_mut(cell) {
                let n = if b == 1 { 1 } else { stack.count };
                let taken = ItemStack { count: n, ..stack.clone() };
                stack.count -= n;
                if stack.count == 0 {
                    table.items[cell] = None;
                }
                game.give_or_drop(taken);
                game.native_play_sound_in_place("minecraft:entity.item.pickup", top(pos), 4, 0.3, 1.4);
            }
        }
        CRAFT => craft(game, &mut table, u32::from(a.max(1))),
        LAYOUT => {
            let Ok(recipe) = std::str::from_utf8(extra) else { return };
            back_to_player(game, &mut table);
            layout(game, &mut table, recipe, u32::from(a.max(1)));
        }
        CLEAR => back_to_player(game, &mut table),
        _ => return,
    }
    store(game, table);
    game.guest_inventory_resend = true;
}

fn top(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.05, f64::from(pos[2]) + 0.5]
}

fn cell_room(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.items().get(&stack.item).map_or(64, |i| i.max_stack).min(CELL_MAX)
}

/// Lays `count` (255: all) of inventory slot `slot` in a cell; what the cell held of something
/// else goes back to the player.
fn place(game: &mut Game<'_>, table: &mut Table, cell: usize, slot: usize, count: u8) {
    if cell >= 9 || slot >= 36 {
        return;
    }
    let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
    if let Some(old) = table.items[cell].clone()
        && !old.same_item_same_components(&stack)
    {
        table.items[cell] = None;
        game.give_or_drop(old);
    }
    let room = cell_room(game, &stack).saturating_sub(table.items[cell].as_ref().map_or(0, |s| s.count));
    let want = if count == 255 { stack.count } else { u32::from(count) };
    let n = want.min(room).min(stack.count);
    if n == 0 {
        return;
    }
    if let Some(cell) = game.inventory.get_mut(slot)
        && let Some(s) = cell.as_mut()
    {
        s.count -= n;
        if s.count == 0 {
            *cell = None;
        }
    }
    match &mut table.items[cell] {
        Some(s) => s.count += n,
        None => table.items[cell] = Some(ItemStack { count: n, ..stack }),
    }
    game.native_play_sound_in_place("minecraft:block.wood.hit", top(table.pos), 4, 0.5, 1.6);
}

/// Everything on the table back to the player.
fn back_to_player(game: &mut Game<'_>, table: &mut Table) {
    let mut any = false;
    for cell in table.items.iter_mut() {
        if let Some(stack) = cell.take() {
            game.give_or_drop(stack);
            any = true;
        }
    }
    if any {
        game.native_play_sound_in_place("minecraft:entity.item.pickup", top(table.pos), 4, 0.3, 1.2);
    }
}

/// Crafts up to `times` from the grid, as the crafting table's result slot does.
fn craft(game: &mut Game<'_>, table: &mut Table, times: u32) {
    let mut made = 0;
    for _ in 0..times {
        let Some((name, result)) = crafted(game, &table.items) else { break };
        let remainders = game.recipes().remainders(&name, &table.items, 3);
        for cell in table.items.iter_mut() {
            if let Some(s) = cell {
                s.count -= 1;
                if s.count == 0 {
                    *cell = None;
                }
            }
        }
        for (i, rest) in remainders.into_iter().enumerate() {
            let Some(rest) = rest else { continue };
            match &mut table.items[i] {
                None => table.items[i] = Some(rest),
                Some(_) => game.give_or_drop(rest),
            }
        }
        game.give_or_drop(result);
        made += 1;
    }
    if made > 0 {
        game.native_play_sound_in_place("minecraft:block.crafter.craft", top(table.pos), 4, 0.8, 1.0);
        game.native_play_sound_in_place("minecraft:entity.item.pickup", top(table.pos), 4, 0.25, 1.0);
    }
}

/// One of an item matching `ingredient` (like the cell's own stack, if it has one) from the inventory.
fn take_matching(game: &mut Game<'_>, ingredient: &Ingredient, like: Option<&ItemStack>) -> Option<ItemStack> {
    let slot = (0..36).find(|&i| {
        matches!(game.inventory.get(i), Some(Some(s)) if ingredient.test(s) && like.is_none_or(|l| l.same_item_same_components(s)))
    })?;
    let cell = game.inventory.get_mut(slot)?;
    let stack = cell.as_mut()?;
    let one = ItemStack { count: 1, ..stack.clone() };
    stack.count -= 1;
    if stack.count == 0 {
        *cell = None;
    }
    Some(one)
}

/// Lays a recipe out from the inventory, `sets` times over: the first set as far as it goes (the
/// rest shows as missing), then only whole sets.
fn layout(game: &mut Game<'_>, table: &mut Table, recipe: &str, sets: u32) {
    let need: Vec<(usize, Ingredient)> = pattern(game, recipe).into_iter().enumerate().filter_map(|(i, g)| Some((i, g?))).collect();
    if need.is_empty() {
        return;
    }
    for set in 0..sets.min(CELL_MAX) {
        if set > 0 {
            // A whole set must be there: try it on a copy of the inventory first.
            let saved = game.inventory.clone();
            let whole = need.iter().all(|(cell, g)| take_matching(game, g, table.items[*cell].as_ref()).is_some());
            game.inventory = saved;
            if !whole {
                break;
            }
        }
        for (cell, g) in &need {
            if table.items[*cell].as_ref().is_some_and(|s| s.count >= cell_room(game, s)) {
                continue;
            }
            if let Some(one) = take_matching(game, g, table.items[*cell].as_ref()) {
                match &mut table.items[*cell] {
                    Some(s) => s.count += 1,
                    None => table.items[*cell] = Some(one),
                }
            }
        }
    }
    game.native_play_sound_in_place("minecraft:block.wood.hit", top(table.pos), 4, 0.6, 1.3);
}

// ---- a table that goes drops what lies on it ----

/// Tables that changed this tick, by dimension, to look at in the level's tick.
static CHANGED: Mutex<Vec<(String, [i32; 3])>> = Mutex::new(Vec::new());

fn dimension_id(d: horizon_world::level::Dimension) -> &'static str {
    match d {
        horizon_world::level::Dimension::Overworld => "minecraft:overworld",
        horizon_world::level::Dimension::Nether => "minecraft:the_nether",
        horizon_world::level::Dimension::End => "minecraft:the_end",
    }
}

/// `native_block_changed`: a crafting table replaced by something else.
pub(crate) fn block_changed(
    level: &super::GameLevel<'_>,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    old: BlockStateId,
    new: BlockStateId,
) {
    let facts = level.facts;
    if &*facts.get(old).name == TABLE && &*facts.get(new).name != TABLE {
        CHANGED.lock().unwrap_or_else(|e| e.into_inner()).push((dimension_id(dimension).to_owned(), [pos.x, pos.y, pos.z]));
    }
}

pub(crate) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    seeds(game);
    if game.world.game_time % 20 == 0 {
        remove_empty_seats(game, dimension);
    }
    let here: Vec<[i32; 3]> = {
        let mut all = CHANGED.lock().unwrap_or_else(|e| e.into_inner());
        if all.is_empty() {
            return;
        }
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *all).into_iter().partition(|(d, _)| d == dimension);
        *all = rest;
        here.into_iter().map(|(_, p)| p).collect()
    };
    for pos in here {
        if !game.world.loaded(pos[0], pos[2]) || is_table(game, pos) {
            continue;
        }
        let table = table_at(game, pos);
        if table.items.iter().all(Option::is_none) {
            continue;
        }
        for stack in table.items.iter().flatten() {
            game.pop_resource(pos, stack.clone());
        }
        store(game, Table { pos, facing: 0, items: vec![None; 9] });
    }
}

// ---- stations other than the table ----

/// The parts of a station worked by clicking: where each lies in the world and what it is. Their
/// order is the station's slots' (a furnace's input, fuel and output; a brewing stand's three
/// bottles, ingredient and fuel).
fn spots(kind: Kind, pos: [i32; 3], facing: u8) -> Vec<([f64; 3], &'static str)> {
    let c = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
    let yaw = f64::from(facing_yaw(facing)).to_radians();
    // Towards the camera, and the camera's right.
    let out = [yaw.sin(), -yaw.cos()];
    let right = [-yaw.cos(), -yaw.sin()];
    let at = |o: f64, r: f64, y: f64| [c[0] + out[0] * o + right[0] * r, c[1] + y, c[2] + out[1] * o + right[1] * r];
    match kind {
        Kind::Table => Vec::new(),
        Kind::Furnace => vec![(at(0.0, 0.0, 0.64), "Input"), (at(0.56, -0.08, -0.3), "Fuel"), (at(0.66, 0.34, 0.08), "Output")],
        Kind::Brewing => {
            let mut v: Vec<([f64; 3], &'static str)> = (0..3)
                .map(|i| {
                    let a = f64::from(i) * std::f64::consts::TAU / 3.0;
                    ([c[0] + a.cos() * 0.3, c[1] - 0.3, c[2] + a.sin() * 0.3], "Bottle")
                })
                .collect();
            v.push((at(0.0, 0.0, 0.44), "Ingredient"));
            v.push((at(0.46, -0.3, -0.44), "Blaze powder"));
            v
        }
        Kind::Campfire => vec![(at(0.0, 0.0, -0.05), "Fire")],
        Kind::Anvil => vec![(at(0.0, -0.25, 0.52), "Item"), (at(0.0, 0.25, 0.52), "Material")],
        Kind::Grindstone => vec![(at(0.05, -0.42, 0.25), "Item"), (at(0.05, 0.42, 0.25), "Item")],
        Kind::Smithing => vec![(at(0.0, -0.27, 0.51), "Template"), (at(0.0, 0.0, 0.51), "Base"), (at(0.0, 0.27, 0.51), "Material")],
        Kind::Stonecutter => vec![(at(0.0, 0.0, 0.1), "Stone")],
        // On the table's top, in front of the book.
        Kind::Enchanting => vec![(at(0.24, -0.22, 0.27), "Item"), (at(0.24, 0.24, 0.27), "Lapis Lazuli")],
        Kind::Lectern => Vec::new(),
        Kind::Jukebox => vec![(at(0.0, 0.0, 0.52), "Disc")],
        // ChiseledBookShelfBlock.getHitSlot: three across, the top row first, from the front's left.
        Kind::Bookshelf => (0..6).map(|i| (at(0.52, [-0.31, 0.03, 0.34][i % 3], if i < 3 { 0.25 } else { -0.25 }), "Book")).collect(),
        Kind::Loom => vec![(at(0.05, 0.0, 0.52), "Banner"), (at(0.2, 0.32, 0.52), "Dye"), (at(0.2, -0.32, 0.52), "Pattern")],
        Kind::Cartography => vec![(at(0.0, -0.2, 0.52), "Map"), (at(0.0, 0.22, 0.52), "Material")],
        Kind::Crafter => (0..9).map(|i| (cell_center(pos, facing, i), "Slot")).collect(),
        Kind::Beacon => vec![(at(0.62, 0.0, -0.1), "Payment")],
    }
}

/// Where a station that works from the inventory shows its result: floating over it.
fn result_spot(kind: Kind, pos: [i32; 3], facing: u8, time: f32) -> [f64; 3] {
    let c = [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5];
    let yaw = f64::from(facing_yaw(facing)).to_radians();
    let right = [-yaw.cos(), -yaw.sin()];
    let (r, y) = match kind {
        Kind::Anvil => (0.0, 1.42),
        Kind::Grindstone => (0.0, 1.3),
        Kind::Smithing => (0.7, 1.2),
        Kind::Crafter => return result_point(pos, facing, time),
        Kind::Loom | Kind::Cartography => (0.0, 1.3),
        Kind::Jukebox => (0.0, 1.35),
        _ => (0.0, 1.05),
    };
    [c[0] + right[0] * r, c[1] + y + f64::from((time * 0.1).sin()) * 0.03, c[2] + right[1] * r]
}

/// The stacks laid on a station that works from the inventory.
fn laid(game: &Game<'_>, refs: &[Option<usize>; 3]) -> [Option<ItemStack>; 3] {
    refs.map(|r| r.and_then(|s| game.inventory.get(s).cloned().flatten()))
}

/// What such a station makes of what lies on it: the result, the anvil's cost in levels, and the
/// stonecutter's choices.
struct Made {
    result: Option<ItemStack>,
    cost: i32,
    choices: Vec<ItemStack>,
}

/// `view` is what the server sent of it: the cartography table's result (a client has no maps).
fn made(game: &Game<'_>, kind: Kind, stacks: &[Option<ItemStack>; 3], name: &str, choice: Option<usize>, view: Option<&View>) -> Made {
    use horizon_client::stations;
    let data = game.stations();
    let e = &data.enchantments;
    let none = Made { result: None, cost: 0, choices: Vec::new() };
    match kind {
        // LoomMenu: a pattern picked of those the pattern item (or none) allows, in the dye's colour.
        Kind::Loom => {
            let banners = &data.banners;
            let (Some(banner), Some(dye)) = (&stacks[0], &stacks[1]) else { return none };
            let Some(color) = banners.loom_dye(game.items(), dye) else { return none };
            let item = stacks[2].as_ref().filter(|p| banners.is_pattern_item(game.items(), p));
            let patterns = banners.selectable(item);
            let choices: Vec<ItemStack> = patterns.iter().map(|p| horizon_client::banner::loom_result(banner, &color, p)).collect();
            // A single pattern is picked by itself; a banner with all its layers takes no more.
            let pick = if patterns.len() == 1 { Some(0) } else { choice };
            let result = pick.filter(|_| !horizon_client::banner::has_max_layers(banner)).and_then(|c| choices.get(c).cloned());
            Made { result, cost: 0, choices }
        }
        Kind::Cartography => Made { result: view.and_then(|v| v.items.first().cloned().flatten()).filter(|_| stacks[0].is_some() && stacks[1].is_some()), ..none },
        Kind::Anvil => {
            let hover = stacks[0].as_ref().map(|i| stations::hover_name(&game.pack.names, i)).unwrap_or_default();
            let r = stations::anvil_result(e, game.items(), stacks[0].as_ref(), stacks[1].as_ref(), Some(name), &hover, game.creative);
            Made { result: r.result, cost: r.cost, choices: Vec::new() }
        }
        Kind::Grindstone => Made { result: stations::grindstone_result(e, game.items(), stacks[0].as_ref(), stacks[1].as_ref()), ..none },
        Kind::Smithing => Made { result: data.recipes.smithing_result(e, stacks[0].as_ref(), stacks[1].as_ref(), stacks[2].as_ref()), ..none },
        Kind::Stonecutter => {
            let choices: Vec<ItemStack> = stacks[0]
                .as_ref()
                .map(|input| data.recipes.stonecutting.iter().filter(|r| r.input.test(input)).map(|r| r.result.clone()).collect())
                .unwrap_or_default();
            Made { result: choice.and_then(|c| choices.get(c).cloned()), cost: 0, choices }
        }
        _ => none,
    }
}

/// `CartographyTableMenu.slotsChanged`: what the table makes of a map and paper, glass or an
/// empty map, from the level's maps (the server's), and the maps made on the way (`take`: the
/// result's `onCraftedPostProcess`, a scaled or locked copy under a new id).
fn carto_result(game: &mut Game<'_>, stacks: &[Option<ItemStack>; 3], take: bool) -> Option<ItemStack> {
    use horizon_client::menu::{Env, Kind as MenuKind, Maps, Menu};
    let (Some(map), Some(extra)) = (stacks[0].clone(), stacks[1].clone()) else { return None };
    let maps = game.level_maps([&map]);
    let out = {
        let env = Env { maps: Some(&maps as &dyn Maps), ..game.menu_env() };
        let mut menu = Menu::new(MenuKind::Cartography);
        menu.set_slot(&env, 0, Some(ItemStack { count: 1, ..map }));
        menu.set_slot(&env, 1, Some(ItemStack { count: 1, ..extra }));
        let mut out = menu.slot(2).cloned();
        if take && let Some(out) = out.as_mut() {
            Menu::map_crafted(env.maps, out);
        }
        out
    };
    if take {
        game.register_made_maps(maps);
    }
    out
}

/// Whether a part of such a station takes a stack.
fn part_takes(game: &Game<'_>, kind: Kind, part: usize, stack: &ItemStack) -> bool {
    let data = game.stations();
    match kind {
        Kind::Anvil | Kind::Enchanting if part == 0 => true,
        Kind::Anvil => true,
        Kind::Enchanting => &*stack.item == "minecraft:lapis_lazuli",
        Kind::Grindstone => horizon_client::stations::grindstone_accepts(game.items(), stack),
        Kind::Smithing => data.recipes.smithing_accepts(part, stack),
        Kind::Stonecutter => data.recipes.stonecutting.iter().any(|r| r.input.test(stack)),
        Kind::Loom => match part {
            0 => stack.item.ends_with("_banner"),
            1 => data.banners.loom_dye(game.items(), stack).is_some(),
            _ => data.banners.is_pattern_item(game.items(), stack),
        },
        Kind::Cartography => match part {
            0 => stack.data().is_some_and(|d| d.map_id.is_some()),
            _ => matches!(&*stack.item, "minecraft:paper" | "minecraft:map" | "minecraft:glass_pane"),
        },
        Kind::Beacon => game.items().item_tags.get("minecraft:beacon_payment_items").is_some_and(|t| t.contains(&*stack.item)),
        _ => false,
    }
}

/// The anvil's name box, and the stonecutter's choices (their top left).
fn name_box(size: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let o = panel(size);
    ([size[0] / 2.0 - 70.0, o[1] - 20.0], [140.0, 13.0])
}

/// The stonecutter's and the loom's choices: six across, at most seven rows.
const CHOICES: usize = 42;

fn choice_cell(size: [f32; 2], i: usize) -> [f32; 2] {
    [size[0] - 8.0 - 6.0 * ICON + (i % 6) as f32 * ICON, size[1] / 2.0 - 90.0 + (i / 6) as f32 * ICON]
}

/// A translation, or `fallback` where a resource pack emptied it (some draw a station's labels into
/// their own textures).
fn tr_or(game: &Game<'_>, key: &str, fallback: &str) -> String {
    let t = game.translate(key, fallback);
    if t.trim().is_empty() { fallback.to_owned() } else { t }
}

/// A colour for the HUD batch from 0xAARRGGBB.
fn col(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    horizon_client::hud::rgba(r, g, b, a)
}

/// Words for the offers in the enchanting script: letters from the offer's own numbers, so they
/// stay the same while it is shown.
fn runes(seed: u32) -> String {
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    let words = 3 + next() % 2;
    (0..words)
        .map(|_| {
            let len = 3 + next() % 5;
            (0..len).map(|_| char::from(b'a' + (next() % 26) as u8)).collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text in the Standard Galactic alphabet (the enchanting table's font), cut to `max` pixels.
fn galactic(game: &Game<'_>, batch: &mut HudBatch, text: &str, at: [f32; 2], max: f32, argb: u32) {
    let atlas = &game.gui.atlas;
    let Some(sheet) = atlas.sprites.get("font/ascii_sga").copied() else { return };
    let cell = sheet.w / 16;
    if cell == 0 {
        return;
    }
    let (px, py) = ((sheet.u0 * atlas.width as f32) as u32, (sheet.v0 * atlas.height as f32) as u32);
    let (aw, ah) = (atlas.width as f32, atlas.height as f32);
    let mut x = at[0];
    for ch in text.chars() {
        if ch == ' ' {
            x += 4.0;
            continue;
        }
        let code = ch as u32;
        if !(0x61..=0x7A).contains(&code) {
            continue;
        }
        let used = f32::from(atlas.galactic_widths[code as usize]);
        let advance = (used * 8.0 / cell as f32 + 0.5).floor() + 1.0;
        if x + advance - at[0] > max {
            break;
        }
        let (cx, cy) = (px + (code % 16) * cell, py + (code / 16) * cell);
        let r = horizon_client::hud::Rect {
            u0: cx as f32 / aw,
            v0: cy as f32 / ah,
            u1: (cx + cell) as f32 / aw,
            v1: (cy + cell) as f32 / ah,
            w: cell,
            h: cell,
        };
        batch.rect_uv(x, at[1], 8.0, 8.0, r, col(argb));
        x += advance;
    }
}

/// The enchanting table's offers, down the right of the screen.
fn offer_plate(size: [f32; 2], i: usize) -> [f32; 2] {
    [size[0] - 12.0 - 108.0, size[1] / 2.0 - 50.0 + i as f32 * 21.0]
}

/// The three offers for what lies on the enchanting table, as its screen shows them: the runes, the
/// levels each costs, and its clue when pointed at.
fn draw_offers(game: &Game<'_>, batch: &mut HudBatch, size: [f32; 2], mouse: [f32; 2], server: Option<&View>, stacks: &[Option<ItemStack>; 3]) {
    let Some(v) = server else { return };
    if stacks[0].is_none() {
        return;
    }
    let atlas = &game.gui.atlas;
    let lapis = stacks[1].as_ref().filter(|l| &*l.item == "minecraft:lapis_lazuli").map_or(0, |l| l.count as i32);
    let level = game.experience.level;
    let dir = "gui/sprites/container/enchanting_table";
    let mut tip = None;
    for i in 0..3 {
        let at = offer_plate(size, i);
        let cost = v.values.get(i).copied().unwrap_or(0);
        if cost <= 0 {
            batch.sprite_scaled(atlas, &format!("{dir}/enchantment_slot_disabled"), at[0], at[1], 108.0, 19.0, u32::MAX);
            continue;
        }
        let off = (lapis < i as i32 + 1 || level < cost) && !game.creative;
        let hover = inside(at, [108.0, 19.0], mouse);
        let plate = if off { "enchantment_slot_disabled" } else if hover { "enchantment_slot_highlighted" } else { "enchantment_slot" };
        batch.sprite_scaled(atlas, &format!("{dir}/{plate}"), at[0], at[1], 108.0, 19.0, u32::MAX);
        batch.sprite(atlas, &format!("{dir}/level_{}{}", i + 1, if off { "_disabled" } else { "" }), at[0] + 1.0, at[1] + 1.0);
        let number = cost.to_string();
        let width = game.native_text_width(&number);
        let words = runes((cost as u32) << 4 | i as u32 | (v.values.get(3 + i).copied().unwrap_or(0) as u32) << 12);
        let ink = if off { 0xFF34_2F25 } else if hover { 0xFFFF_FF80 } else { 0xFF68_5E4A };
        galactic(game, batch, &words, [at[0] + 20.0, at[1] + 2.0], 86.0 - width, ink);
        game.native_text(batch, &number, [at[0] + 106.0 - width, at[1] + 9.0], if off { 0x407F10 } else { 0x80FF20 }, true);
        if hover {
            tip = Some(i);
        }
    }
    // EnchantmentScreen's tooltip: the clue, then the lapis and levels it takes.
    let Some(i) = tip else { return };
    let cost = v.values.get(i).copied().unwrap_or(0);
    let mut lines: Vec<(String, u32)> = Vec::new();
    if let Some(id) = v.texts.get(i).filter(|t| !t.is_empty()) {
        let (name, _) = game.enchantments().full_name(id, v.values.get(3 + i).copied().unwrap_or(1).max(1) as u32);
        lines.push((game.translate("container.enchant.clue", "%s . . . ?").replace("%s", &name), 0xFFFFFF));
    }
    if !game.creative {
        if level < cost {
            lines.push((game.translate("container.enchant.level.requirement", "Level Requirement: %s").replace("%s", &cost.to_string()), 0xFF5555));
        } else {
            let n = i + 1;
            let lapis_line = if n == 1 {
                game.translate("container.enchant.lapis.one", "1 Lapis Lazuli")
            } else {
                game.translate("container.enchant.lapis.many", "%s Lapis Lazuli").replace("%s", &n.to_string())
            };
            lines.push((lapis_line, if lapis >= n as i32 { 0xAAAAAA } else { 0xFF5555 }));
            let levels = if n == 1 {
                game.translate("container.enchant.level.one", "1 Enchantment Level")
            } else {
                game.translate("container.enchant.level.many", "%s Enchantment Levels").replace("%s", &n.to_string())
            };
            lines.push((levels, 0xAAAAAA));
        }
    }
    let w = lines.iter().map(|(t, _)| game.native_text_width(t)).fold(0.0f32, f32::max) + 8.0;
    let at = [(mouse[0] - w - 8.0).max(2.0), mouse[1] + 4.0];
    game.native_fill(batch, [at[0], at[1], w, lines.len() as f32 * 10.0 + 6.0], 0xF0100010);
    for (n, (text, rgb)) in lines.iter().enumerate() {
        game.native_text(batch, text, [at[0] + 4.0, at[1] + 4.0 + n as f32 * 10.0], *rgb, true);
    }
}

/// The beacon's buttons: its five primary powers by tier, regeneration and the primary's level
/// II as the secondary ones, and confirm.
fn power_at(size: [f32; 2], k: usize) -> [f32; 2] {
    let (x, y) = (size[0] - 12.0 - 106.0, size[1] / 2.0 - 70.0);
    match k {
        0 => [x, y],
        1 => [x + 24.0, y],
        2 => [x, y + 26.0],
        3 => [x + 24.0, y + 26.0],
        4 => [x + 12.0, y + 52.0],
        5 => [x + 70.0, y + 13.0],
        6 => [x + 70.0, y + 39.0],
        _ => [x + 70.0, y + 76.0],
    }
}

/// The level a power needs (`BeaconBlockEntity.BEACON_EFFECTS`).
fn power_tier(e: usize) -> i32 {
    super::beacon::EFFECTS.get(e).map_or(9, |p| p.1)
}

/// The beacon's powers as its screen shows them, the ones picked lit.
fn draw_powers(game: &Game<'_>, batch: &mut HudBatch, size: [f32; 2], mouse: [f32; 2], server: Option<&View>, paid: bool) {
    let Some(v) = server else { return };
    let levels = v.values.first().copied().unwrap_or(0);
    let picked = client(|c| c.scene.as_ref().map_or([None; 2], |s| s.powers));
    let atlas = &game.gui.atlas;
    let dir = "gui/sprites/container/beacon";
    let (x, y) = (size[0] - 12.0 - 106.0, size[1] / 2.0 - 70.0);
    game.native_fill(batch, [x - 6.0, y - 18.0, 118.0, 124.0], 0xC0101418);
    // The columns' names, their first word (there is room for no more).
    let head = |key: &str, fallback: &str| tr_or(game, key, fallback).split_whitespace().next().map_or_else(String::new, |w| fit(w, 9));
    game.native_text(batch, &head("block.minecraft.beacon.primary", "Primary Power"), [x - 2.0, y - 13.0], 0xE0E0E0, true);
    game.native_text(batch, &head("block.minecraft.beacon.secondary", "Secondary Power"), [x + 58.0, y - 13.0], 0xE0E0E0, true);
    let mut tip = None;
    let effects = super::beacon::EFFECTS;
    for k in 0..8 {
        let at = power_at(size, k);
        let hover = inside(at, [22.0, 22.0], mouse);
        let (active, selected, icon) = match k {
            0..=4 => (power_tier(k) <= levels, picked[0] == Some(k), Some(k)),
            5 => (levels >= 4, picked[1] == Some(5), Some(5)),
            6 => (levels >= 4 && picked[0].is_some(), picked[0].is_some() && picked[1] == picked[0], picked[0]),
            _ => (paid && picked[0].is_some(), false, None),
        };
        if k == 7 {
            let sprite = if !active { "button_disabled" } else if hover { "button_highlighted" } else { "button" };
            batch.sprite(atlas, &format!("{dir}/{sprite}"), at[0], at[1]);
            batch.sprite(atlas, &format!("{dir}/confirm"), at[0] + 2.0, at[1] + 2.0);
            continue;
        }
        let sprite = if !active { "button_disabled" } else if selected { "button_selected" } else if hover { "button_highlighted" } else { "button" };
        batch.sprite(atlas, &format!("{dir}/{sprite}"), at[0], at[1]);
        if let Some(e) = icon.and_then(|e| effects.get(e)) {
            batch.sprite_scaled(atlas, &format!("mob_effect/{}", e.0), at[0] + 2.0, at[1] + 2.0, 18.0, 18.0, u32::MAX);
            if hover {
                let name = game.translate(&format!("effect.minecraft.{}", e.0), e.0);
                tip = Some(if k == 6 { format!("{name} II") } else { name });
            }
        }
    }
    if !paid {
        game.native_text(batch, "Payment?", [x + 66.0, y + 100.0], 0xA0A0A0, true);
    }
    if let Some(text) = tip {
        let w = game.native_text_width(&text) + 8.0;
        game.native_fill(batch, [mouse[0] - w - 6.0, mouse[1] + 4.0, w, 13.0], 0xF0100010);
        game.native_text(batch, &text, [mouse[0] - w - 2.0, mouse[1] + 6.0], 0xFFFFFF, true);
    }
}

/// The open book over the lectern, 192 GUI pixels square as the book screen's.
fn book_origin(size: [f32; 2]) -> [f32; 2] {
    [((size[0] - 192.0) / 2.0).floor(), ((size[1] - 192.0) / 2.0 - 18.0).floor().max(bar(size) + 2.0)]
}

/// The pages a lectern's book reads: a written book's components, else a writable book's text
/// (`BookViewScreen`'s pages).
fn pages_of(stack: &ItemStack) -> Vec<Text> {
    let Some(data) = stack.data() else { return Vec::new() };
    let Some(w) = &data.written else {
        return data.writable_pages.iter().map(Text::literal).collect();
    };
    let tagged = w.page_tags.len() == w.pages.len();
    w.pages
        .iter()
        .enumerate()
        .map(|(i, page)| match tagged.then(|| &w.page_tags[i]) {
            Some(tag @ (horizon_world::nbt::Tag::Compound(_) | horizon_world::nbt::Tag::List(_))) => horizon_client::entity_nbt::name_text(tag),
            _ => Text::literal(page),
        })
        .collect()
}

/// How many pages a book has (`LecternBlockEntity.getPageCount`).
fn page_count(stack: &ItemStack) -> usize {
    stack.data().map_or(0, |d| d.written.as_ref().map_or(d.writable_pages.len(), |w| w.pages.len()))
}

/// The lectern's book, open over it: the page, its number, the arrows, Done and Take Book; a page
/// turning sweeps over the new one.
fn draw_lectern(game: &Game<'_>, batch: &mut HudBatch, size: [f32; 2], mouse: [f32; 2], server: Option<&View>) {
    let Some(book) = server.and_then(|v| v.items.first().cloned().flatten()) else { return };
    let atlas = &game.gui.atlas;
    let pages = pages_of(&book);
    let count = pages.len();
    let (leaf, turn, opened) = client(|c| {
        c.scene.as_ref().map_or((0, None, 1.0), |s| (s.leaf.unwrap_or(0), s.turn, (s.started.elapsed().as_secs_f32() - s.enter) / 0.3))
    });
    let page = leaf.min(count.saturating_sub(1));
    let [left, top] = book_origin(size);
    // Lifted off the lectern: the book grows into place.
    let grow = smooth(opened);
    if grow < 1.0 {
        let s = 0.3 + 0.7 * grow;
        let (w, h) = (192.0 * s, 192.0 * s);
        let (cx, cy) = (left + 96.0, top + 96.0 + (1.0 - grow) * 80.0);
        game.native_sprite(batch, "gui/book", [0.0, 0.0, 192.0, 192.0], [cx - w / 2.0, cy - h / 2.0, w, h]);
        return;
    }
    game.native_sprite(batch, "gui/book", [0.0, 0.0, 192.0, 192.0], [left, top, 192.0, 192.0]);
    let black = col(0xFF00_0000);
    // BookViewScreen: the page number, then up to 14 lines of 114 pixels.
    let number = horizon_protocol::text::format_java(&game.translate("book.pageIndicator", "Page %1$s of %2$s"), &[(page + 1).to_string(), count.max(1).to_string()]);
    let nw = game.native_text_width(&number);
    batch.text(atlas, &number, left + 148.0 - nw, top + 16.0, black, false);
    if let Some(text) = pages.get(page) {
        for (n, line) in atlas.font.split(text, 114.0).iter().take(14).enumerate() {
            batch.text_styled(atlas, line, left + 36.0, top + 30.0 + n as f32 * 9.0, black, false);
        }
    }
    // The page turning: a leaf swinging about the spine over the new page.
    if let Some((at, forward)) = turn {
        let t = at.elapsed().as_secs_f32() / 0.35;
        if t < 1.0 {
            // Forward, the old page folds over towards the spine; back, away from it.
            let k = smooth(t);
            let width = 132.0 * ((1.0 - k) * std::f32::consts::FRAC_PI_2).sin().max(0.02);
            let x = if forward { left + 28.0 } else { left + 160.0 - width };
            let shade = (16.0 + 56.0 * k) as u32;
            game.native_fill(batch, [x, top + 12.0, width, 146.0], 0xFFF2_E6CC);
            game.native_fill(batch, [x, top + 12.0, width, 146.0], shade << 24);
            let edge = if forward { x + width - 2.0 } else { x };
            game.native_fill(batch, [edge, top + 12.0, 2.0, 146.0], 0x60000000);
        }
    }
    // The arrows while there is a page that way.
    let hover = |x: f32, y: f32, w: f32, h: f32| inside([x, y], [w, h], mouse);
    for (show, x, name) in [(page + 1 < count, 116.0, "page_forward"), (page > 0, 43.0, "page_backward")] {
        if show {
            let lit = if hover(left + x, top + 157.0, 23.0, 13.0) { "_highlighted" } else { "" };
            batch.sprite(atlas, &format!("gui/sprites/widget/{name}{lit}"), left + x, top + 157.0);
        }
    }
    // LecternScreen's buttons under the book.
    let y = top + 194.0;
    let done = game.translate("gui.done", "Done");
    let take = game.translate("lectern.take_book", "Take Book");
    horizon_client::ui::button(batch, atlas, size[0] / 2.0 - 100.0, y, 98.0, 20.0, &Text::literal(done), true, hover(size[0] / 2.0 - 100.0, y, 98.0, 20.0));
    horizon_client::ui::button(batch, atlas, size[0] / 2.0 + 2.0, y, 98.0, 20.0, &Text::literal(take), true, hover(size[0] / 2.0 + 2.0, y, 98.0, 20.0));
}

/// The spot under the pointer, if one is near it on the screen.
fn spot_under(pose: Pose, fov: f32, size: [f32; 2], list: &[([f64; 3], &'static str)], mouse: [f32; 2]) -> Option<usize> {
    list.iter()
        .enumerate()
        .filter_map(|(i, (p, _))| project(pose, fov, size, *p).map(|q| (i, (q[0] - mouse[0]).hypot(q[1] - mouse[1]))))
        .filter(|(_, d)| *d < 24.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn hint_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Table => "",
        Kind::Furnace => "Pick an item: click the top to smelt it, the fire to fuel it  -  click a part to take from it  -  Esc leaves",
        Kind::Brewing => "Bottles on the arms, the ingredient on top, blaze powder below  -  click a part to take from it  -  Esc leaves",
        Kind::Campfire => "Pick food and click the fire to cook it  -  H by a campfire sits you down  -  Esc leaves",
        Kind::Anvil => "The item on the left, the material on the right  -  name it in the box  -  click the result to forge it  -  Esc leaves",
        Kind::Grindstone => "Enchanted or worn items on either side of the wheel  -  click the result to grind them  -  Esc leaves",
        Kind::Smithing => "Template, base and material on the table  -  click the result to smith it  -  Esc leaves",
        Kind::Stonecutter => "Stone on the saw, then pick what to cut  -  click the result to cut it (shift: all)  -  Esc leaves",
        Kind::Enchanting => "The item and lapis on the table  -  pick an offer to enchant it  -  Esc leaves",
        Kind::Lectern => "Click the arrows or the page's edges to turn it  -  Esc leaves",
        Kind::Jukebox => "Pick a disc and click the jukebox to play it  -  click it to take the disc out  -  Esc leaves",
        Kind::Bookshelf => "Pick a book and click a slot to shelve it  -  click a book to take it  -  Esc leaves",
        Kind::Loom => "The banner on top, the dye and a pattern beside it  -  pick a pattern  -  click the result to weave it  -  Esc leaves",
        Kind::Cartography => "A map, and paper, glass or an empty map  -  click the result to take it  -  Esc leaves",
        Kind::Crafter => "Pick an item and click a cell  -  click an empty cell to turn it off or on  -  power it to craft  -  Esc leaves",
        Kind::Beacon => "Pay with an ingot, an emerald or a diamond  -  pick the powers and confirm  -  Esc leaves",
    }
}

/// A bar of `done` (0 to 1) at `at`, `w` wide.
fn meter(game: &Game<'_>, batch: &mut HudBatch, at: [f32; 2], w: f32, done: f32, color: u32) {
    game.native_fill(batch, [at[0] - 1.0, at[1] - 1.0, w + 2.0, 6.0], 0xC0201810);
    game.native_fill(batch, [at[0], at[1], w * done.clamp(0.0, 1.0), 4.0], color);
}

#[allow(clippy::too_many_arguments)]
fn station_draw(
    game: &Game<'_>,
    batch: &mut HudBatch,
    pose: Pose,
    fov: f32,
    size: [f32; 2],
    mouse: [f32; 2],
    (pos, kind, facing, carried): ([i32; 3], Kind, u8, Option<usize>),
    time: f32,
) {
    let (refs, name, typing, choice) =
        client(|c| c.scene.as_ref().map_or(([None; 3], String::new(), false, None), |s| (s.refs, s.search.clone(), s.typing, s.choice)));
    let stacks = laid(game, &refs);
    let server = client(|c| c.view.clone().filter(|v| v.pos == pos));
    // What lies at the spots: the stacks laid from the inventory, or the station's own.
    let view = if from_inventory(kind) {
        Some(View { pos, items: stacks.to_vec(), values: Vec::new(), texts: Vec::new() })
    } else {
        server.clone()
    };
    let list = spots(kind, pos, facing);
    let o = panel(size);
    if client(|c| c.scene.as_mut().is_some_and(|s| !std::mem::replace(&mut s.logged, true))) {
        let at: Vec<String> = list
            .iter()
            .map(|(p, _)| project(pose, fov, size, *p).map_or("0 0".into(), |q| format!("{:.0} {:.0}", q[0], q[1])))
            .collect();
        let r = project(pose, fov, size, result_spot(kind, pos, facing, time)).unwrap_or_default();
        log::info!(
            "[cinematic_interactions] station {kind:?} at panel {:.0} {:.0} result {:.0} {:.0} spots {}",
            o[0], o[1], r[0], r[1], at.join(" ")
        );
    }
    if from_inventory(kind) {
        let made = made(game, kind, &stacks, &name, choice, server.as_ref());
        // The result over the station, with what it costs.
        if let Some(result) = &made.result
            && let Some(r) = project(pose, fov, size, result_spot(kind, pos, facing, time))
        {
            let hover = (mouse[0] - r[0]).hypot(mouse[1] - r[1]) < 22.0;
            let mut label = fit(&name_of(game, &result.item), 20);
            if result.count > 1 {
                label = format!("{label} x{}", result.count);
            }
            let w = label.chars().count() as f32 * 5.0 + 10.0;
            game.native_fill(batch, [r[0] - w / 2.0, r[1] + 22.0, w, 12.0], if hover { 0xE0806020 } else { 0xC0201810 });
            game.native_text(batch, &label, [r[0] - w / 2.0 + 5.0, r[1] + 24.0], 0xF0F0F0, true);
            if kind == Kind::Anvil && made.cost > 0 {
                let afford = game.creative || game.experience.level >= made.cost;
                let cost = if made.cost >= 40 && !game.creative { "Too expensive!".to_owned() } else { format!("Cost: {} levels", made.cost) };
                let w = cost.len() as f32 * 5.0 + 10.0;
                game.native_fill(batch, [r[0] - w / 2.0, r[1] + 36.0, w, 12.0], 0xC0201810);
                game.native_text(batch, &cost, [r[0] - w / 2.0 + 5.0, r[1] + 38.0], if afford && made.cost < 40 { 0x80FF20 } else { 0xFF6060 }, true);
            }
        }
        // The anvil's name box.
        if kind == Kind::Anvil && stacks[0].is_some() {
            let (b, wh) = name_box(size);
            game.native_fill(batch, [b[0] - 1.0, b[1] - 1.0, wh[0] + 2.0, wh[1] + 2.0], 0xFFA0A0A0);
            game.native_fill(batch, [b[0], b[1], wh[0], wh[1]], if typing { 0xFF101010 } else { 0xFF303030 });
            let caret = if typing && (time * 2.0) as i32 % 2 == 0 { "_" } else { "" };
            game.native_text(batch, &format!("{}{caret}", fit(&name, 26)), [b[0] + 4.0, b[1] + 2.0], 0xFFFFFF, false);
        }
        // What the stonecutter cuts the stone into; the loom's patterns.
        if matches!(kind, Kind::Stonecutter | Kind::Loom) {
            let picked = if kind == Kind::Loom && made.choices.len() == 1 { Some(0) } else { choice };
            let mut tip = None;
            for (i, st) in made.choices.iter().enumerate().take(CHOICES) {
                let cell = choice_cell(size, i);
                let hover = inside(cell, [ICON, ICON], mouse);
                game.native_fill(batch, [cell[0], cell[1], ICON - 2.0, ICON - 2.0], if picked == Some(i) { 0xF0D8A848 } else if hover { 0xE0705A44 } else { 0xE0403428 });
                draw_stack(game, batch, st, [cell[0] + 1.0, cell[1] + 1.0]);
                if hover {
                    tip = Some((i, cell));
                }
            }
            if let Some((i, cell)) = tip {
                let text = match (kind, horizon_client::banner::layers(&made.choices[i]).last()) {
                    (Kind::Loom, Some(layer)) => {
                        let key = game.stations().banners.layer_key(layer);
                        game.translate(&key, &layer.pattern)
                    }
                    _ => name_of(game, &made.choices[i].item),
                };
                let w = game.native_text_width(&text) + 8.0;
                game.native_fill(batch, [cell[0] - w - 2.0, cell[1] + 3.0, w, 12.0], 0xE0100810);
                game.native_text(batch, &text, [cell[0] - w + 2.0, cell[1] + 5.0], 0xF0F0F0, true);
            }
        }
        if kind == Kind::Enchanting {
            draw_offers(game, batch, size, mouse, server.as_ref(), &stacks);
        }
        if kind == Kind::Beacon {
            draw_powers(game, batch, size, mouse, server.as_ref(), stacks[0].is_some());
        }
    }
    if kind == Kind::Lectern {
        draw_lectern(game, batch, size, mouse, server.as_ref());
        let hint = hint_of(kind);
        let w = hint.len() as f32 * 5.0;
        game.native_fill(batch, [(size[0] - w) / 2.0 - 6.0, size[1] - 17.0, w + 12.0, 13.0], 0x80000000);
        game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 15.0], 0xE8E8E8, true);
        return;
    }
    // The song a jukebox plays.
    if kind == Kind::Jukebox
        && let Some(disc) = server.as_ref().and_then(|v| v.items.first().cloned().flatten())
        && let Some(q) = project(pose, fov, size, result_spot(kind, pos, facing, time))
    {
        let song = game.res.items().get(&disc.item).and_then(|i| i.jukebox_song.clone());
        let title = song.map_or_else(|| name_of(game, &disc.item), |s| {
            let key = format!("jukebox_song.{}", s.replace(':', "."));
            game.translate(&key, &s)
        });
        let text = game.translate("record.nowPlaying", "Now Playing: %s").replace("%s", &title);
        let w = game.native_text_width(&text) + 10.0;
        game.native_fill(batch, [q[0] - w / 2.0, q[1], w, 12.0], 0xC0201810);
        game.native_text(batch, &text, [q[0] - w / 2.0 + 5.0, q[1] + 2.0], 0x55FF55, true);
    }
    // A crafter's cells turned off.
    if kind == Kind::Crafter
        && let Some(v) = &server
    {
        let off = v.values.first().copied().unwrap_or(0);
        for (i, (p, _)) in list.iter().enumerate() {
            if off & (1 << i) != 0
                && let Some(q) = project(pose, fov, size, *p)
            {
                game.native_fill(batch, [q[0] - 13.0, q[1] - 13.0, 26.0, 26.0], 0xB0A01818);
                game.native_sprite(batch, "gui/sprites/container/crafter/disabled_slot", [0.0, 0.0, 18.0, 18.0], [q[0] - 9.0, q[1] - 9.0, 18.0, 18.0]);
            }
        }
        if let Some((_, result)) = crafted(game, &v.items)
            && let Some(r) = project(pose, fov, size, result_spot(kind, pos, facing, time))
        {
            let label = format!("{}  -  {}", fit(&name_of(game, &result.item), 20), if v.values.get(1).copied().unwrap_or(0) != 0 { "crafting" } else { "power it to craft" });
            let w = label.chars().count() as f32 * 5.0 + 10.0;
            game.native_fill(batch, [r[0] - w / 2.0, r[1] + 22.0, w, 12.0], 0xC0201810);
            game.native_text(batch, &label, [r[0] - w / 2.0 + 5.0, r[1] + 24.0], 0xF0F0F0, true);
        }
    }
    let hover = spot_under(pose, fov, size, &list, mouse);
    for (i, (p, label)) in list.iter().enumerate() {
        let Some(q) = project(pose, fov, size, *p) else { continue };
        let held = view.as_ref().and_then(|v| v.items.get(i).cloned().flatten());
        if hover == Some(i) {
            game.native_fill(batch, [q[0] - 13.0, q[1] - 13.0, 26.0, 26.0], if carried.is_some() { 0x50FFE080 } else { 0x40FFFFFF });
            let text = match &held {
                Some(st) => {
                    let name = name_of(game, &st.item);
                    let shown = if name.eq_ignore_ascii_case(label) { fit(&name, 24) } else { format!("{}: {}", label, fit(&name, 20)) };
                    if st.count > 1 { format!("{shown} x{}", st.count) } else { shown }
                }
                None => (*label).to_owned(),
            };
            let w = text.chars().count() as f32 * 5.0 + 10.0;
            game.native_fill(batch, [q[0] - w / 2.0, q[1] + 16.0, w, 12.0], 0xC0201810);
            game.native_text(batch, &text, [q[0] - w / 2.0 + 5.0, q[1] + 18.0], 0xF0F0F0, true);
        } else if let Some(st) = &held
            && st.count > 1
        {
            let n = st.count.to_string();
            game.native_text(batch, &n, [q[0] + 6.0, q[1] + 4.0], 0xFFFFFF, true);
        }
    }
    // The station's progress.
    if let Some(v) = &view {
        let val = |i: usize| v.values.get(i).copied().unwrap_or(0) as f32;
        match kind {
            Kind::Furnace => {
                if let (Some(a), Some(b)) = (project(pose, fov, size, list[0].0), project(pose, fov, size, list[2].0)) {
                    let mid = [(a[0] + b[0]) / 2.0 + 18.0, (a[1] + b[1]) / 2.0];
                    meter(game, batch, [mid[0] - 20.0, mid[1]], 40.0, val(2) / val(3).max(1.0), 0xFFF0F0F0);
                }
                if let Some(f) = project(pose, fov, size, list[1].0) {
                    meter(game, batch, [f[0] - 14.0, f[1] + 16.0], 28.0, val(0) / val(1).max(1.0), 0xFFF08020);
                }
            }
            Kind::Brewing => {
                if let Some(t) = project(pose, fov, size, list[3].0) {
                    let brewing = val(0) > 0.0;
                    let done = if brewing { 1.0 - val(0) / val(1).max(1.0) } else { 0.0 };
                    meter(game, batch, [t[0] - 20.0, t[1] - 22.0], 40.0, done, 0xFFB070E0);
                }
                if let Some(f) = project(pose, fov, size, list[4].0) {
                    meter(game, batch, [f[0] - 14.0, f[1] + 16.0], 28.0, val(2) / 20.0, 0xFFF0A020);
                }
            }
            Kind::Campfire => {
                if let Some(q) = project(pose, fov, size, list[0].0) {
                    let cooking: Vec<f32> = (0..4).filter(|&i| val(i + 4) > 0.0).map(|i| val(i) / val(i + 4)).collect();
                    for (n, done) in cooking.iter().enumerate() {
                        meter(game, batch, [q[0] - 20.0, q[1] - 30.0 - n as f32 * 7.0], 40.0, *done, 0xFFF0A040);
                    }
                }
            }
            _ => {}
        }
    }
    // The inventory panel.
    game.native_fill(batch, [o[0], o[1], SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0], 0xC8181410);
    let hover_slot = panel_slot(o, mouse);
    for i in 0..36 {
        let a = panel_slot_at(o, i);
        let color = if carried == Some(i) {
            0xF0D8A848
        } else if hover_slot == Some(i) {
            0xE0705A44
        } else {
            0xE0403428
        };
        game.native_fill(batch, [a[0] + 1.0, a[1] + 1.0, SLOT - 2.0, SLOT - 2.0], color);
        if carried != Some(i)
            && let Some(Some(stack)) = game.inventory.get(i)
        {
            draw_stack(game, batch, stack, [a[0] + 1.0, a[1] + 1.0]);
        }
    }
    client(|c| {
        // The hand carries what goes to the station; only what comes back flies.
        let carries = c.hand_out;
        if let Some(s) = c.scene.as_mut() {
            s.flights.retain(|f| f.start.elapsed().as_secs_f32() < FLIGHT);
            for f in &s.flights {
                if carries && inside(panel(size), [SLOT * 9.0 + 8.0, SLOT * 4.0 + 12.0], f.from) {
                    continue;
                }
                let t = smooth(f.start.elapsed().as_secs_f32() / FLIGHT);
                let arc = (t * std::f32::consts::PI).sin() * 30.0;
                let p = [f.from[0] + (f.to[0] - f.from[0]) * t - 8.0, f.from[1] + (f.to[1] - f.from[1]) * t - 8.0 - arc];
                game.native_item(batch, &f.item, p, 16.0);
            }
        }
    });
    if let Some(slot) = carried
        && !client(|c| c.hand_out)
        && let Some(Some(stack)) = game.inventory.get(slot)
    {
        draw_stack(game, batch, stack, [mouse[0] - 8.0, mouse[1] - 8.0]);
    }
    let _ = time;
    let hint = hint_of(kind);
    let w = hint.len() as f32 * 5.0;
    game.native_fill(batch, [(size[0] - w) / 2.0 - 6.0, size[1] - 17.0, w + 12.0, 13.0], 0x80000000);
    game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 15.0], 0xE8E8E8, true);
}

fn station_press(game: &Game<'_>, mouse: [f32; 2], button: i32, pos: [i32; 3]) {
    let view = client(|c| c.view.clone().filter(|v| v.pos == pos));
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(pose) = c.scene.as_ref().map(|s| scene_pose(c, s)) else { return };
        let Some(s) = c.scene.as_mut() else { return };
        let (kind, facing) = (s.kind, s.facing);
        if kind == Kind::Lectern {
            lectern_press(c, size, mouse, pos, view.as_ref());
            return;
        }
        let o = panel(size);
        if let Some(slot) = panel_slot(o, mouse) {
            s.carried = if s.carried == Some(slot) || game.inventory.get(slot).is_none_or(Option::is_none) { None } else { Some(slot) };
            return;
        }
        let list = spots(kind, pos, facing);
        if from_inventory(kind) {
            inventory_press(c, game, (pose, fov, size), mouse, button, shift_of(game), (pos, kind, facing), &list);
            return;
        }
        if let Some(i) = spot_under(pose, fov, size, &list, mouse) {
            let at = project(pose, fov, size, list[i].0).unwrap_or(mouse);
            let one = button == 1;
            match s.carried {
                Some(slot) => {
                    let Some(Some(stack)) = game.inventory.get(slot) else {
                        s.carried = None;
                        return;
                    };
                    s.flights.push(Flight { item: stack.item.to_string(), from: slot_middle(o, slot), to: at, start: Instant::now() });
                    if !one || stack.count <= 1 {
                        s.carried = None;
                    }
                    // The disc sinks into the jukebox once it gets there.
                    if kind == Kind::Jukebox {
                        s.strike = Some(Instant::now());
                    }
                    send(c, PUT, pos, facing, i as u8, slot as u8, &[if one { 1 } else { 255 }]);
                }
                None => {
                    if let Some(st) = view.as_ref().and_then(|v| v.items.get(i).cloned().flatten()) {
                        s.flights.push(Flight { item: st.item.to_string(), from: at, to: slot_middle(o, landing(game, &st.item)), start: Instant::now() });
                        send(c, PULL, pos, facing, i as u8, if one { 1 } else { 255 }, &[]);
                    } else if kind == Kind::Crafter {
                        // CrafterScreen: an empty cell clicked with nothing held turns off, or on.
                        send(c, TOGGLE, pos, facing, i as u8, 0, &[]);
                    }
                }
            }
            return;
        }
        if let Some(npos) = neighbour_under(game, pose, fov, size, pos, mouse) {
            glide(c, game, npos, pose);
        }
    });
}

/// `native_net_from_server`: what the station worked holds.
pub(crate) fn net_from_server(channel: &str, data: &[u8]) {
    if channel != VIEW || data.len() < 14 {
        return;
    }
    let pos = [0, 1, 2].map(|i| i32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap_or_default()));
    let n = usize::from(data[12]);
    let mut at = 13 + n * 4;
    if data.len() <= at {
        return;
    }
    let values: Vec<i32> = (0..n).map(|i| i32::from_le_bytes(data[13 + i * 4..17 + i * 4].try_into().unwrap_or_default())).collect();
    // The words: a count, then each with its length.
    let words = usize::from(data[at]);
    at += 1;
    let mut texts = Vec::with_capacity(words);
    for _ in 0..words {
        let Some(&len) = data.get(at) else { return };
        let Some(bytes) = data.get(at + 1..at + 1 + usize::from(len)) else { return };
        texts.push(String::from_utf8_lossy(bytes).into_owned());
        at += 1 + usize::from(len);
    }
    let items = match horizon_world::nbt::read_network(&data[at..], horizon_world::nbt::Limits::tables()) {
        Ok((tag, _)) => horizon_items::item_nbt::decode_items(Some(&tag), 9),
        Err(_) => vec![None; 9],
    };
    client(|c| {
        let first = c.view.as_ref().is_none_or(|v| v.pos != pos);
        let before = c.view.as_ref().filter(|v| v.pos == pos).and_then(|v| v.values.first().copied());
        if let Some(s) = c.scene.as_mut().filter(|s| s.pos == pos) {
            match s.kind {
                // The lectern's page follows the server's when it changes (another reader).
                Kind::Lectern if first || before != values.first().copied() => {
                    let page = values.first().copied().unwrap_or(0).max(0) as usize;
                    if s.leaf != Some(page) {
                        s.leaf = Some(page);
                    }
                }
                // The beacon's powers as they are, until the player picks others.
                Kind::Beacon if first => {
                    let pick = |v: Option<&i32>| v.copied().filter(|&v| v > 0).map(|v| v as usize - 1);
                    s.powers = [pick(values.get(1)), pick(values.get(2))];
                }
                _ => {}
            }
        }
        c.view = Some(View { pos, items, values, texts });
    });
}

/// The items a station shows in the world while worked: what its slots hold, at their spots.
fn draw_station(
    game: &Game<'_>,
    draw: &mut super::native_world_draw::NativeWorldDraw,
    kind: Kind,
    pos: [i32; 3],
    facing: u8,
    view: &View,
    strike: Option<Instant>,
) {
    let time = game.world.game_time as f32 + draw.partial;
    let light = horizon_client::block_entity_render::vertex_light(game.world.light(pos[0], pos[1] + 1, pos[2]));
    // A crafter's stacks lie on its cells as a table's do.
    if kind == Kind::Crafter {
        for (i, stack) in view.items.iter().enumerate().take(9) {
            let Some(stack) = stack else { continue };
            let c = cell_center(pos, facing, i);
            let (scale, lift, block) = fitted(game, stack, 0.19, i as i32 * 3);
            draw.push();
            if draw.translate_world([c[0], c[1] + 0.002 + f64::from(lift), c[2]]) {
                draw.light = light;
                draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(facing), [0.0, 0.0, 0.0]);
                if !block {
                    draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
                }
                draw.scale([scale, scale, scale]);
                game.native_draw_item(&mut *draw, stack, DisplayContext::Fixed, None, i as i32 * 3);
            }
            draw.pop();
        }
        return;
    }
    // A disc put in sinks into the jukebox, turning; then it is inside.
    if kind == Kind::Jukebox {
        let (Some(disc), Some(at)) = (view.items.first().cloned().flatten(), strike) else { return };
        let t = at.elapsed().as_secs_f32() - FLIGHT;
        if !(0.0..0.7).contains(&t) {
            return;
        }
        let k = smooth(t / 0.7);
        let top = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5];
        draw.push();
        if draw.translate_world([top[0], top[1] + 0.3 - f64::from(k) * 0.42, top[2]]) {
            draw.light = light;
            draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(facing), [0.0, 0.0, 0.0]);
            draw.rotate_about([0.0, 0.0, 1.0], time * 12.0, [0.0, 0.0, 0.0]);
            draw.scale([0.36, 0.36, 0.36]);
            game.native_draw_item(&mut *draw, &disc, DisplayContext::Fixed, None, 7);
        }
        draw.pop();
        return;
    }
    if matches!(kind, Kind::Bookshelf | Kind::Lectern) {
        return;
    }
    for (i, (p, _)) in spots(kind, pos, facing).iter().enumerate() {
        let Some(stack) = view.items.get(i).cloned().flatten() else { continue };
        let (mut size, spin) = match (kind, i) {
            (Kind::Campfire, _) => continue,
            (Kind::Furnace, 2) => (0.26, time * 2.0),
            (Kind::Brewing, 0..=2) => (0.24, 0.0),
            _ => (0.22, time * 1.2),
        };
        // The ingredient dissolves into the bottles as the brew goes.
        if kind == Kind::Brewing && i == 3 && view.values.first().is_some_and(|&t| t > 0) {
            let left = view.values[0] as f32 / view.values.get(1).copied().unwrap_or(400).max(1) as f32;
            size *= 0.35 + 0.65 * left.clamp(0.0, 1.0);
        }
        let (scale, lift, _) = fitted(game, &stack, size, i as i32);
        let bob = if kind == Kind::Furnace && i == 0 { (time * 0.12).sin() * 0.02 } else { 0.0 };
        draw.push();
        if draw.translate_world([p[0], p[1] + f64::from(lift * 0.5 + bob), p[2]]) {
            draw.light = light;
            draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(facing) + spin, [0.0, 0.0, 0.0]);
            draw.scale([scale, scale, scale]);
            game.native_draw_item(&mut *draw, &stack, DisplayContext::Fixed, None, i as i32);
        }
        draw.pop();
    }
}

// ---- the server's half of the stations ----

/// The players working a station: where, what they were last sent, and what they laid on it
/// from their inventory.
#[derive(Clone, Copy)]
struct Watch {
    pos: [i32; 3],
    sig: u64,
    refs: [Option<usize>; 3],
}

static WATCH: Mutex<Option<HashMap<u128, Watch>>> = Mutex::new(None);

fn watch(uuid: u128, pos: [i32; 3]) {
    WATCH.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(uuid, Watch { pos, sig: 0, refs: [None; 3] });
}

/// The inventory slots a player laid on the station they work (`LAY`).
fn set_refs(uuid: u128, pos: [i32; 3], extra: &[u8]) {
    if extra.len() < 3 {
        return;
    }
    if let Some(w) = WATCH.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|w| w.get_mut(&uuid))
        && w.pos == pos
    {
        w.refs = [extra[0], extra[1], extra[2]].map(|v| (v > 0 && v <= 36).then(|| usize::from(v - 1)));
        w.sig = 0;
    }
}

/// The players' enchantment seeds, read in a level's tick (where the hub is in reach) for the
/// offers made in their own tick; a seed used up is replaced there.
static SEEDS: Mutex<Option<HashMap<u128, i32>>> = Mutex::new(None);
static RESEED: Mutex<Vec<u128>> = Mutex::new(Vec::new());

fn seed_of(uuid: u128) -> Option<i32> {
    SEEDS.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(&uuid).copied().filter(|&s| s != 0)
}

/// `Player.enchantmentSeed` of everyone on the server: made when a player working a station has
/// none yet (as the enchanting menu makes it), new after they enchanted.
fn seeds(game: &mut Game<'_>) {
    let reseed: Vec<u128> = std::mem::take(&mut *RESEED.lock().unwrap_or_else(|e| e.into_inner()));
    let working: Vec<u128> = WATCH.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|w| w.keys().copied().collect()).unwrap_or_default();
    let Some(host) = game.lan.as_mut() else { return };
    let mut all = HashMap::new();
    for g in host.hub.guests.iter_mut() {
        if reseed.contains(&g.uuid) || (g.xp_seed == 0 && working.contains(&g.uuid)) {
            g.xp_seed = game.world.random.next_int();
        }
        all.insert(g.uuid, g.xp_seed);
    }
    *SEEDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(all);
}

fn unwatch(uuid: u128) {
    if let Some(w) = WATCH.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        w.remove(&uuid);
    }
}

/// What a player working a station is sent: its slots, numbers and words.
struct State {
    items: Vec<Option<ItemStack>>,
    values: Vec<i32>,
    texts: Vec<String>,
}

/// A station's state: its block entity's slots and numbers, or what the server works out for the
/// stacks a player laid on it (`refs`: the enchanting table's offers for their seed, the
/// cartography table's result).
fn station_state(game: &Game<'_>, pos: [i32; 3], refs: [Option<usize>; 3], uuid: u128) -> Option<State> {
    use horizon_client::containers::BlockEntity;
    let kind = station_at(game, pos)?;
    let plain = |items: Vec<Option<ItemStack>>, values: Vec<i32>| Some(State { items, values, texts: Vec::new() });
    match kind {
        // EnchantmentMenu.slotsChanged: the costs, and the clues' enchantments and levels.
        Kind::Enchanting => {
            let item = refs[0].and_then(|r| game.inventory.get(r).cloned().flatten());
            let Some(seed) = seed_of(uuid) else { return plain(Vec::new(), vec![0; 6]) };
            let shelves = game.count_bookshelves(pos);
            let offers = horizon_client::stations::enchant_offers(&game.stations().enchantments, game.items(), seed, shelves, item.as_ref());
            let mut values = offers.costs.to_vec();
            values.extend(offers.clues.iter().map(|c| c.as_ref().map_or(0, |(_, l)| *l as i32)));
            let texts = offers.clues.iter().map(|c| c.as_ref().map_or(String::new(), |(e, _)| e.to_string())).collect();
            Some(State { items: Vec::new(), values, texts })
        }
        _ => match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Furnace(f)) => plain(f.items.to_vec(), vec![f.lit_remaining, f.lit_total, f.cooking_timer, f.cooking_total]),
            Some(BlockEntity::Brewing(b)) => plain(b.items.to_vec(), vec![b.brew_time, 400, b.fuel, 20]),
            Some(BlockEntity::Campfire(c)) => plain(c.items.to_vec(), c.progress.iter().chain(c.total.iter()).copied().collect()),
            Some(BlockEntity::Lectern(l)) => plain(vec![l.book.clone()], vec![l.page, l.book.as_ref().map_or(0, |b| page_count(b) as i32)]),
            Some(BlockEntity::Jukebox(j)) => plain(vec![j.items[0].clone()], vec![i32::from(j.playing)]),
            Some(BlockEntity::ChiseledBookshelf(b)) => plain(b.items.to_vec(), Vec::new()),
            Some(BlockEntity::Crafter(c)) => {
                let off = c.disabled.iter().enumerate().filter(|(_, d)| **d).fold(0, |m, (i, _)| m | 1 << i);
                plain(c.items.to_vec(), vec![off, i32::from(c.triggered)])
            }
            Some(BlockEntity::Beacon(b)) => {
                let index = |e: &Option<String>| e.as_deref().and_then(|e| super::beacon::EFFECTS.iter().position(|(n, _)| e.ends_with(n))).map_or(0, |i| i as i32 + 1);
                plain(Vec::new(), vec![b.levels, index(&b.primary), index(&b.secondary)])
            }
            // A bookshelf or jukebox never used has no block entity yet: it is empty.
            None if matches!(kind, Kind::Bookshelf | Kind::Jukebox | Kind::Crafter) => plain(vec![None; if kind == Kind::Crafter { 9 } else { 6 }], vec![0, 0]),
            _ => None,
        },
    }
}

/// Sends the acting player what their station holds when it changed (its timers by the second).
fn send_view(game: &mut Game<'_>, uuid: u128) {
    let Some(w) = WATCH.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|w| w.get(&uuid).copied()) else {
        return;
    };
    // The cartography table's result reads the level's maps.
    let state = if station_at(game, w.pos) == Some(Kind::Cartography) {
        let stacks = laid(game, &w.refs);
        State { items: vec![carto_result(game, &stacks, false)], values: Vec::new(), texts: Vec::new() }
    } else {
        let Some(state) = station_state(game, w.pos, w.refs, uuid) else { return };
        state
    };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    for s in state.items.iter() {
        match s {
            Some(s) => s.item.bytes().chain(s.count.to_le_bytes()).for_each(&mut feed),
            None => feed(0xFF),
        }
    }
    for v in &state.values {
        (v / 5).to_le_bytes().into_iter().for_each(&mut feed);
    }
    for t in &state.texts {
        t.bytes().chain([0]).for_each(&mut feed);
    }
    let sig = h | 1;
    if sig == w.sig {
        return;
    }
    let Ok(nbt) = horizon_world::nbt::write_network(&horizon_items::item_nbt::encode_items(&state.items), horizon_world::nbt::Limits::tables()) else {
        return;
    };
    let mut m = Vec::with_capacity(14 + state.values.len() * 4 + nbt.len());
    for v in w.pos {
        m.extend_from_slice(&v.to_le_bytes());
    }
    m.push(state.values.len() as u8);
    for v in &state.values {
        m.extend_from_slice(&v.to_le_bytes());
    }
    let texts: Vec<&str> = state.texts.iter().take(255).map(String::as_str).collect();
    m.push(texts.len() as u8);
    for t in texts {
        let b = &t.as_bytes()[..t.len().min(255)];
        m.push(b.len() as u8);
        m.extend_from_slice(b);
    }
    m.extend_from_slice(&nbt);
    if game.native_net_send_to_player(uuid, VIEW, &m).is_ok()
        && let Some(all) = WATCH.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
        && let Some(e) = all.get_mut(&uuid)
    {
        e.sig = sig;
    }
}

/// Takes up to `n` from inventory slot `slot`.
fn take_slot(game: &mut Game<'_>, slot: usize, n: u32) -> Option<ItemStack> {
    let cell = game.inventory.get_mut(slot)?;
    let stack = cell.as_mut()?;
    let n = n.min(stack.count);
    if n == 0 {
        return None;
    }
    let taken = ItemStack { count: n, ..stack.clone() };
    stack.count -= n;
    if stack.count == 0 {
        *cell = None;
    }
    Some(taken)
}

/// Whether a station's slot takes `stack` (as its menu's slot would), and how many it holds.
fn slot_takes(game: &Game<'_>, kind: Kind, slot: usize, stack: &ItemStack, held: Option<&ItemStack>) -> u32 {
    let max = game.items().get(&stack.item).map_or(64, |i| i.max_stack);
    match (kind, slot) {
        (Kind::Furnace, 0) => max,
        // AbstractFurnaceBlockEntity.canPlaceItem: fuel, or a bucket where no bucket is.
        (Kind::Furnace, 1) => {
            let fuel = game.items().get(&stack.item).is_some_and(|i| i.fuel.is_some());
            let bucket = &*stack.item == "minecraft:bucket" && held.is_none_or(|h| &*h.item != "minecraft:bucket");
            if fuel || bucket { max } else { 0 }
        }
        // BrewingStandBlockEntity.canPlaceItem: one potion or bottle an arm, an ingredient on
        // top, blaze powder below.
        (Kind::Brewing, 0..=2) => u32::from(game.stations().brewing.is_bottle(stack) && held.is_none()),
        (Kind::Brewing, 3) => if game.stations().brewing.is_reagent(stack) { max } else { 0 },
        (Kind::Brewing, 4) => if horizon_client::brewing::brewing_fuel(game.items(), stack).is_some() { max } else { 0 },
        // CrafterMenu's slots: anything, where the cell is on (checked by the caller).
        (Kind::Crafter, 0..=8) => max,
        _ => 0,
    }
}

fn center(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]
}

#[allow(clippy::too_many_arguments)]
fn station_act(game: &mut Game<'_>, uuid: u128, kind: Kind, op: u8, pos: [i32; 3], a: u8, b: u8, extra: &[u8]) {
    use horizon_client::containers::BlockEntity;
    let key = (pos[0], pos[1], pos[2]);
    match op {
        OPEN => {
            watch(uuid, pos);
            // LecternBlock.openScreen
            if kind == Kind::Lectern {
                game.award_custom_stat("interact_with_lectern", 1);
            }
        }
        SIT if kind == Kind::Campfire => sit(game, pos),
        LAY if told(kind) => set_refs(uuid, pos, extra),
        RESULT if from_inventory(kind) => make(game, uuid, kind, pos, u32::from(a.max(1)), extra),
        // CrafterMenu.setSlotState: an empty cell turns off, or back on.
        TOGGLE if kind == Kind::Crafter => {
            let cell = usize::from(a);
            if cell < 9
                && let Some(BlockEntity::Crafter(c)) = game.world.block_entities.get_mut(&key)
                && c.items[cell].is_none()
            {
                c.disabled[cell] = !c.disabled[cell];
                let off = c.disabled[cell];
                game.world.modified.insert((key.0 >> 4, key.2 >> 4));
                game.native_play_sound_in_place("minecraft:ui.button.click", center(pos), 4, 0.4, if off { 0.75 } else { 1.0 });
            }
        }
        // LecternMenu: a page picked becomes the block entity's (its redstone pulse and sound).
        PAGE if kind == Kind::Lectern => {
            let Some(mut menu) = game.lectern_menu(key) else { return };
            let count = match game.world.block_entities.get(&key) {
                Some(BlockEntity::Lectern(l)) => l.book.as_ref().map_or(0, page_count),
                _ => 0,
            };
            menu.data[0] = i32::from(a).min(count as i32 - 1).max(0);
            game.lectern_menu_done(key, &menu);
        }
        TAKE_BOOK if kind == Kind::Lectern => game.take_lectern_book(key),
        // JukeboxPlayable.tryInsertIntoJukebox: one disc, its song starts (a creative player
        // keeps theirs).
        PUT if kind == Kind::Jukebox => {
            let slot = usize::from(b);
            let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
            if game.res.items().get(&stack.item).is_none_or(|i| i.jukebox_song.is_none()) {
                return;
            }
            match game.world.block_entities.get(&key) {
                Some(BlockEntity::Jukebox(j)) if j.record().is_some() => return,
                Some(BlockEntity::Jukebox(_)) => {}
                Some(_) => return,
                None => {
                    game.world.block_entities.insert(key, BlockEntity::Jukebox(Default::default()));
                }
            }
            if !game.creative {
                take_slot(game, slot, 1);
            }
            game.award_custom_stat("play_record", 1);
            game.jukebox_set_item(pos, Some(ItemStack { count: 1, ..stack }));
            game.world.modified.insert((key.0 >> 4, key.2 >> 4));
        }
        // JukeboxBlock.useWithoutItem pops the disc out: here it goes to the player.
        PULL if kind == Kind::Jukebox => {
            let disc = match game.world.block_entities.get(&key) {
                Some(BlockEntity::Jukebox(j)) => j.record().cloned(),
                _ => None,
            };
            let Some(disc) = disc else { return };
            game.jukebox_set_item(pos, None);
            game.world.modified.insert((key.0 >> 4, key.2 >> 4));
            game.give_or_drop(disc);
        }
        // ChiseledBookShelfBlock.addBook: a book into an empty slot (a creative player keeps theirs).
        PUT if kind == Kind::Bookshelf => {
            let (spot, slot) = (usize::from(a), usize::from(b));
            let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
            let book = game.items().item_tags.get("minecraft:bookshelf_books").is_some_and(|m| m.contains(&*stack.item));
            if spot >= 6 || !book {
                return;
            }
            let entity = game.world.block_entities.get_or_insert_with(key, || BlockEntity::ChiseledBookshelf(Box::default()));
            let BlockEntity::ChiseledBookshelf(shelf) = entity else { return };
            if shelf.items[spot].is_some() {
                return;
            }
            shelf.items[spot] = Some(ItemStack { count: 1, ..stack.clone() });
            if !game.creative {
                take_slot(game, slot, 1);
            }
            game.award_stat("used", &stack.item, 1);
            game.container_changed(key);
            let sound = if &*stack.item == "minecraft:enchanted_book" {
                "minecraft:block.chiseled_bookshelf.insert.enchanted"
            } else {
                "minecraft:block.chiseled_bookshelf.insert"
            };
            game.native_play_sound_in_place(sound, center(pos), 4, 1.0, 1.0);
        }
        // removeBook: the book to the inventory.
        PULL if kind == Kind::Bookshelf => {
            let spot = usize::from(a);
            let Some(BlockEntity::ChiseledBookshelf(shelf)) = game.world.block_entities.get_mut(&key) else { return };
            let Some(book) = shelf.items.get_mut(spot).and_then(Option::take) else { return };
            game.container_changed(key);
            let sound = if &*book.item == "minecraft:enchanted_book" {
                "minecraft:block.chiseled_bookshelf.pickup.enchanted"
            } else {
                "minecraft:block.chiseled_bookshelf.pickup"
            };
            game.native_play_sound_in_place(sound, center(pos), 4, 1.0, 1.0);
            game.give_or_drop(book);
        }
        PUT if kind == Kind::Campfire => {
            let slot = usize::from(b);
            let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
            let Some(time) = game.recipes().cook("minecraft:campfire_cooking", &stack).map(|r| r.time as i32) else { return };
            let placed = match game.world.block_entities.get_mut(&key) {
                Some(BlockEntity::Campfire(c)) => {
                    let mut one = ItemStack { count: 1, ..stack.clone() };
                    c.place_food(&mut one, time)
                }
                _ => false,
            };
            if placed {
                take_slot(game, slot, 1);
                game.native_campfire_start(pos, &stack.item);
                game.mark_block_entity_updated(key);
            }
        }
        PUT => {
            let (spot, slot) = (usize::from(a), usize::from(b));
            let count = extra.first().copied().unwrap_or(1);
            let Some(Some(stack)) = game.inventory.get(slot).cloned() else { return };
            if kind == Kind::Crafter && c_off(game, key, spot) {
                return;
            }
            let Some(state) = station_state(game, pos, [None; 3], uuid) else { return };
            let held = state.items.get(spot).cloned().flatten();
            let room = slot_takes(game, kind, spot, &stack, held.as_ref().filter(|h| !h.same_item_same_components(&stack)));
            if room == 0 {
                return;
            }
            // Something else there goes back to the player first.
            let mut have = 0;
            if let Some(h) = &held {
                if h.same_item_same_components(&stack) {
                    have = h.count;
                } else {
                    let back = h.clone();
                    set_station_slot(game, kind, key, spot, None);
                    game.give_or_drop(back);
                }
            }
            let want = if count == 255 { stack.count } else { u32::from(count) };
            let n = want.min(room.saturating_sub(have));
            let Some(taken) = take_slot(game, slot, n) else { return };
            log::debug!("[cinematic_interactions] put {} x{} in {kind:?} {pos:?} slot {spot} (room {room}, had {have})", taken.item, taken.count);
            set_station_slot(game, kind, key, spot, Some(ItemStack { count: have + taken.count, ..taken }));
            let sound = if kind == Kind::Crafter { "minecraft:block.crafter.craft" } else { "minecraft:block.stone.hit" };
            game.native_play_sound_in_place(sound, [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.8, f64::from(pos[2]) + 0.5], 4, 0.3, 1.4);
        }
        PULL => {
            let spot = usize::from(a);
            let Some(state) = station_state(game, pos, [None; 3], uuid) else { return };
            let Some(held) = state.items.get(spot).cloned().flatten() else { return };
            if kind == Kind::Campfire {
                return;
            }
            let n = if b == 1 { 1 } else { held.count };
            let rest = (held.count > n).then(|| ItemStack { count: held.count - n, ..held.clone() });
            set_station_slot(game, kind, key, spot, rest);
            game.give_or_drop(ItemStack { count: n, ..held });
            // FurnaceResultSlot.checkTakeAchievements: the output pays what it made.
            if kind == Kind::Furnace && spot == 2 {
                let at = game.player.as_ref().map_or([f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5], |p| [p.position.x, p.position.y, p.position.z]);
                game.award_furnace_recipes(key, at);
            }
            game.native_play_sound_in_place("minecraft:entity.item.pickup", [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.8, f64::from(pos[2]) + 0.5], 4, 0.3, 1.2);
        }
        _ => {}
    }
}

/// Whether a crafter's cell is turned off.
fn c_off(game: &Game<'_>, key: (i32, i32, i32), cell: usize) -> bool {
    matches!(game.world.block_entities.get(&key), Some(horizon_client::containers::BlockEntity::Crafter(c)) if c.disabled.get(cell).copied().unwrap_or(false))
}

/// Sets a slot of a station's block entity, as its menu would.
fn set_station_slot(game: &mut Game<'_>, kind: Kind, key: (i32, i32, i32), slot: usize, stack: Option<ItemStack>) {
    use horizon_client::containers::BlockEntity;
    match game.world.block_entities.get_mut(&key) {
        // AbstractFurnaceBlockEntity.setItem: only a new input restarts the cooking.
        Some(BlockEntity::Furnace(f)) if slot == 0 => f.set_input(world_recipes!(game), stack),
        Some(BlockEntity::Furnace(f)) if slot < 3 => f.items[slot] = stack,
        Some(BlockEntity::Brewing(b)) if slot < 5 => b.items[slot] = stack,
        Some(BlockEntity::Crafter(c)) if slot < 9 => c.items[slot] = stack,
        other => log::warn!("[cinematic_interactions] no {kind:?} slot {slot} at {key:?}: {}", other.is_some()),
    }
    // BlockEntity.setChanged: saved with the chunk, and the comparators read it again.
    game.container_changed(key);
}

// ---- sitting by a campfire ----

/// `native_key`: the sit key (H) while looking at a campfire.
pub(crate) fn sit_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_on(ID) || !game.native_playing() || key != game.native_mod_key(ID, SIT_KEY, SIT_DEFAULT) {
        return;
    }
    if game.riding.is_some() {
        return;
    }
    let Some(eye) = game.eye_position() else { return };
    let dir = super::view_vector(game.yaw, game.pitch);
    let Some(hit) = game.clip_ray(eye, dir, 5.0) else { return };
    if station_at(game, hit.pos) != Some(Kind::Campfire) {
        return;
    }
    let mut m = vec![SIT];
    for v in hit.pos {
        m.extend_from_slice(&v.to_le_bytes());
    }
    m.extend_from_slice(&[0, 0, 0]);
    if let Err(e) = game.native_net_send_to_server(CHANNEL, &m) {
        log::debug!("[cinematic_interactions] not sent: {e}");
    }
}

/// Sits the acting player down beside the campfire at `pos`, on their side of it: on an invisible
/// silverfish on the ground (its rider sits 0.3 above it, 0.6 under the rider's feet), which goes
/// when they get up.
fn sit(game: &mut Game<'_>, pos: [i32; 3]) {
    let player = game.native_acting_entity();
    if player == 0 && game.lan_acting.is_some() {
        return;
    }
    let Some(p) = game.player.as_ref().map(|p| [p.position.x, p.position.z]) else { return };
    let c = [f64::from(pos[0]) + 0.5, f64::from(pos[2]) + 0.5];
    let mut d = [p[0] - c[0], p[1] - c[1]];
    let len = d[0].hypot(d[1]);
    d = if len < 0.2 { [0.0, 1.0] } else { [d[0] / len, d[1] / len] };
    // Its side first, then the others round the fire, where there is room.
    let sides = [d, [-d[1], d[0]], [d[1], -d[0]], [-d[0], -d[1]]];
    let free = |x: i32, y: i32, z: i32| !game.res.facts().get(game.world.block(x, y, z)).full_collision;
    let Some(at) = sides.iter().map(|s| [c[0] + s[0] * 1.35, f64::from(pos[1]), c[1] + s[1] * 1.35]).find(|a| {
        let (x, z) = (a[0].floor() as i32, a[2].floor() as i32);
        free(x, pos[1], z) && free(x, pos[1] + 1, z) && !free(x, pos[1] - 1, z)
    }) else {
        return;
    };
    let dim = game.dim.id();
    let snbt = format!(
        "{{NoAI:1b,Silent:1b,Invulnerable:1b,NoGravity:1b,PersistenceRequired:1b,Tags:[\"{SEAT_TAG}\"],active_effects:[{{id:\"minecraft:invisibility\",duration:-1,amplifier:0b,show_particles:0b,ambient:1b}}]}}"
    );
    let Ok((horizon_world::nbt::Tag::Compound(nbt), _)) = horizon_world::nbt::parse_snbt(&snbt) else { return };
    let Some(id) = game.native_summon(dim, "minecraft:silverfish", at, nbt) else { return };
    if !game.native_mount(player, id) {
        game.native_remove_mob(dim, id);
    }
}

/// Seats nobody sits on go.
fn remove_empty_seats(game: &mut Game<'_>, dimension: &str) {
    let seats: Vec<u32> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.kind.id == "minecraft:silverfish")
        .map(|m| m.id)
        .filter(|&id| game.tags_of(super::commands::Target::Mob(id)).iter().any(|t| t == SEAT_TAG))
        .collect();
    for id in seats {
        let ridden = game.riding == Some(id) || game.lan.as_ref().is_some_and(|h| h.hub.guests.iter().any(|g| g.vehicle == Some(id)));
        if !ridden {
            game.native_remove_mob(dimension, id);
        }
    }
}

// ---- stations that work from the inventory ----

fn shift_of(game: &Game<'_>) -> bool {
    game.physical_keys.contains(&KeyCode::ShiftLeft) || game.physical_keys.contains(&KeyCode::ShiftRight)
}

#[allow(clippy::too_many_arguments)]
fn inventory_press(
    c: &mut Client,
    game: &Game<'_>,
    (pose, fov, size): (Pose, f32, [f32; 2]),
    mouse: [f32; 2],
    button: i32,
    shift: bool,
    (pos, kind, facing): ([i32; 3], Kind, u8),
    list: &[([f64; 3], &'static str)],
) {
    let server = c.view.clone().filter(|v| v.pos == pos);
    let Some(s) = c.scene.as_mut() else { return };
    let o = panel(size);
    let time = game.world.game_time as f32 / 20.0;
    // The anvil's name box.
    let (nb, nwh) = name_box(size);
    let stacks = laid(game, &s.refs);
    let refs = |s: &Scene| s.refs.map(|r| r.map_or(0, |v| v as u8 + 1));
    if kind == Kind::Anvil && stacks[0].is_some() && inside(nb, nwh, mouse) {
        s.typing = true;
        return;
    }
    s.typing = false;
    if matches!(kind, Kind::Stonecutter | Kind::Loom)
        && let Some(i) = (0..CHOICES).find(|&i| inside(choice_cell(size, i), [ICON, ICON], mouse))
    {
        let n = made(game, kind, &stacks, "", None, server.as_ref()).choices.len();
        if i < n {
            s.choice = Some(i);
        }
        return;
    }
    // An offer of the enchanting table: enchant with it.
    if kind == Kind::Enchanting
        && let Some(i) = (0..3).find(|&i| inside(offer_plate(size, i), [108.0, 19.0], mouse))
    {
        let cost = server.as_ref().and_then(|v| v.values.get(i).copied()).unwrap_or(0);
        let lapis = stacks[1].as_ref().filter(|l| &*l.item == "minecraft:lapis_lazuli").map_or(0, |l| l.count as i32);
        if stacks[0].is_none() || cost <= 0 || (!game.creative && (lapis < i as i32 + 1 || game.experience.level < cost)) {
            return;
        }
        s.strike = Some(Instant::now());
        let r = refs(s);
        send(c, RESULT, pos, facing, 1, 0, &[r[0], r[1], r[2], i as u8 + 1]);
        return;
    }
    // The beacon's powers, and confirm.
    if kind == Kind::Beacon
        && let Some(k) = (0..8).find(|&k| inside(power_at(size, k), [22.0, 22.0], mouse))
    {
        let levels = server.as_ref().and_then(|v| v.values.first().copied()).unwrap_or(0);
        match k {
            0..=4 if power_tier(k) <= levels => {
                // BeaconScreen: the primary's level II goes with it.
                if s.powers[1].is_some() && s.powers[1] == s.powers[0] {
                    s.powers[1] = None;
                }
                s.powers[0] = Some(k);
            }
            5 if levels >= 4 => s.powers[1] = Some(5),
            6 if levels >= 4 && s.powers[0].is_some() => s.powers[1] = s.powers[0],
            7 if stacks[0].is_some() && s.powers[0].is_some() => {
                let r = refs(s);
                let p = s.powers.map(|e| e.map_or(0, |e| e as u8 + 1));
                if let Some(at) = project(pose, fov, size, list[0].0) {
                    if let Some(st) = &stacks[0] {
                        s.flights.push(Flight { item: st.item.to_string(), from: slot_middle(o, s.refs[0].unwrap_or(0)), to: at, start: Instant::now() });
                    }
                }
                s.refs[0] = None;
                send(c, RESULT, pos, facing, 1, 0, &[r[0], r[1], r[2], 0, p[0], p[1]]);
            }
            _ => {}
        }
        return;
    }
    // The result: make it.
    let m = made(game, kind, &stacks, &s.search, s.choice, server.as_ref());
    if let Some(result) = &m.result
        && let Some(r) = project(pose, fov, size, result_spot(kind, pos, facing, time))
        && (mouse[0] - r[0]).hypot(mouse[1] - r[1]) < 22.0
    {
        if kind == Kind::Anvil && !game.creative && (game.experience.level < m.cost || m.cost >= 40) {
            return;
        }
        s.flights.push(Flight { item: result.item.to_string(), from: r, to: slot_middle(o, landing(game, &result.item)), start: Instant::now() });
        let picked = if kind == Kind::Loom && m.choices.len() == 1 { Some(0) } else { s.choice };
        let mut extra = refs(s).to_vec();
        extra.push(picked.map_or(0, |v| v as u8 + 1));
        if kind == Kind::Anvil {
            // AnvilScreen's box takes 50 characters.
            let name: String = s.search.chars().take(50).collect();
            extra.extend_from_slice(name.as_bytes());
            s.strike = Some(Instant::now());
        }
        // What was used up leaves the station (the stonecutter keeps its stone, the loom its
        // pattern).
        match kind {
            Kind::Stonecutter => {}
            Kind::Loom => {
                s.refs[0] = None;
                s.refs[1] = None;
            }
            _ => {
                s.refs = [None; 3];
                s.search.clear();
            }
        }
        let times = if shift || button == 1 { 64 } else { 1 };
        let told_refs = refs(s);
        send(c, RESULT, pos, facing, times, 0, &extra);
        if told(kind) {
            send(c, LAY, pos, facing, 0, 0, &told_refs);
        }
        return;
    }
    if let Some(slot) = panel_slot(o, mouse) {
        s.carried = if s.carried == Some(slot) || game.inventory.get(slot).is_none_or(Option::is_none) { None } else { Some(slot) };
        return;
    }
    if let Some(i) = spot_under(pose, fov, size, list, mouse) {
        let at = project(pose, fov, size, list[i].0).unwrap_or(mouse);
        match s.carried.take() {
            Some(slot) => {
                let Some(Some(stack)) = game.inventory.get(slot) else { return };
                if !part_takes(game, kind, i, stack) || s.refs.iter().enumerate().any(|(j, r)| j != i && *r == Some(slot)) {
                    return;
                }
                s.flights.push(Flight { item: stack.item.to_string(), from: slot_middle(o, slot), to: at, start: Instant::now() });
                s.refs[i] = Some(slot);
                if kind == Kind::Anvil && i == 0 {
                    s.search = horizon_client::stations::hover_name(&game.pack.names, stack);
                }
                if kind == Kind::Stonecutter || (kind == Kind::Loom && i == 2) {
                    s.choice = None;
                }
            }
            None => {
                if let Some(Some(stack)) = s.refs[i].and_then(|r| game.inventory.get(r)) {
                    s.flights.push(Flight { item: stack.item.to_string(), from: at, to: slot_middle(o, s.refs[i].unwrap_or(0)), start: Instant::now() });
                }
                s.refs[i] = None;
                if kind == Kind::Anvil && i == 0 {
                    s.search.clear();
                }
                if kind == Kind::Loom {
                    s.choice = None;
                }
            }
        }
        // The server works out what this station makes: it hears what lies on it.
        if told(kind) {
            let r = refs(s);
            send(c, LAY, pos, facing, 0, 0, &r);
        }
        return;
    }
    if let Some(npos) = neighbour_under(game, pose, fov, size, pos, mouse) {
        glide(c, game, npos, pose);
    }
}

/// Clicks on the lectern's book: the arrows or the page's edges turn it, Done leaves, Take Book
/// takes it.
fn lectern_press(c: &mut Client, size: [f32; 2], mouse: [f32; 2], pos: [i32; 3], view: Option<&View>) {
    let Some(count) = view.and_then(|v| v.items.first().cloned().flatten()).map(|b| page_count(&b)) else { return };
    let [left, top] = book_origin(size);
    let y = top + 194.0;
    if inside([size[0] / 2.0 - 100.0, y], [98.0, 20.0], mouse) {
        c.close = true;
        return;
    }
    if inside([size[0] / 2.0 + 2.0, y], [98.0, 20.0], mouse) {
        send(c, TAKE_BOOK, pos, 0, 0, 0, &[]);
        c.close = true;
        return;
    }
    let Some(s) = c.scene.as_mut() else { return };
    let page = s.leaf.unwrap_or(0).min(count.saturating_sub(1));
    let forward = inside([left + 116.0, top + 157.0], [23.0, 13.0], mouse) || inside([left + 124.0, top + 10.0], [56.0, 146.0], mouse);
    let back = inside([left + 43.0, top + 157.0], [23.0, 13.0], mouse) || inside([left + 12.0, top + 10.0], [40.0, 146.0], mouse);
    let to = if forward && page + 1 < count {
        page + 1
    } else if back && page > 0 {
        page - 1
    } else {
        return;
    };
    s.leaf = Some(to);
    s.turn = Some((Instant::now(), to > page));
    send(c, PAGE, pos, 0, to.min(255) as u8, 0, &[]);
}

/// The stacks laid on a station that works from the inventory, and its result, in the world; the
/// anvil's hammer coming down.
fn draw_laid(game: &Game<'_>, draw: &mut super::native_world_draw::NativeWorldDraw, kind: Kind, pos: [i32; 3], facing: u8) {
    let Some((refs, name, choice, strike, server)) =
        client(|c| c.scene.as_ref().map(|s| (s.refs, s.search.clone(), s.choice, s.strike, c.view.clone().filter(|v| v.pos == pos))))
    else {
        return;
    };
    let stacks = laid(game, &refs);
    let time = game.world.game_time as f32 + draw.partial;
    let light = horizon_client::block_entity_render::vertex_light(game.world.light(pos[0], pos[1] + 1, pos[2]));
    for (i, (p, _)) in spots(kind, pos, facing).iter().enumerate() {
        let Some(stack) = stacks.get(i).cloned().flatten() else { continue };
        let (scale, lift, block) = fitted(game, &stack, 0.24, i as i32);
        // The item enchanted rises to the book and turns, a second long.
        let (mut up, mut spin) = (0.0f64, 0.0f32);
        if kind == Kind::Enchanting
            && i == 0
            && let Some(at) = strike
            && at.elapsed().as_secs_f32() < 1.2
        {
            let t = at.elapsed().as_secs_f32() / 1.2;
            up = f64::from((t * std::f32::consts::PI).sin()) * 0.4;
            spin = t * 720.0;
        }
        // The beacon's payment hangs in front of it, turning.
        let upright = matches!(kind, Kind::Grindstone | Kind::Beacon) || up > 0.0;
        if kind == Kind::Beacon {
            spin = time * 2.0;
        }
        draw.push();
        if draw.translate_world([p[0], p[1] + 0.002 + f64::from(lift) + up, p[2]]) {
            draw.light = if up > 0.0 { horizon_client::block_entity_render::vertex_light(15 << 4 | 15) } else { light };
            draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(facing) + spin, [0.0, 0.0, 0.0]);
            // Flat items lie on the station; the grindstone holds them up to the wheel.
            if !block && !upright {
                draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
            }
            draw.scale([scale, scale, scale]);
            game.native_draw_item(&mut *draw, &stack, DisplayContext::Fixed, None, i as i32);
        }
        draw.pop();
    }
    let m = made(game, kind, &stacks, &name, choice, server.as_ref());
    if let Some(result) = &m.result {
        let p = result_spot(kind, pos, facing, time / 20.0);
        draw.push();
        if draw.translate_world(p) {
            draw.light = light;
            draw.rotate_about([0.0, 1.0, 0.0], time * 3.0, [0.0, 0.0, 0.0]);
            let (scale, _, _) = fitted(game, result, 0.3, 77);
            draw.scale([scale, scale, scale]);
            game.native_draw_item(&mut *draw, result, DisplayContext::Fixed, None, 77);
        }
        draw.pop();
    }
    // The hammer: up over the anvil, down onto it, a third of a second.
    if kind == Kind::Anvil
        && let Some(at) = strike
        && at.elapsed().as_secs_f32() < 0.45
    {
        // Raised behind, then brought down onto the item, in the plane the camera sees: about
        // the handle's end, the head over the item's spot.
        let t = at.elapsed().as_secs_f32() / 0.45;
        let swing = if t < 0.55 { 60.0 * (t / 0.55) } else { 60.0 - 95.0 * ((t - 0.55) / 0.45) };
        let item = spots(kind, pos, facing).first().map_or([f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5], |s| s.0);
        draw.push();
        if draw.translate_world([item[0], item[1] + 0.32, item[2]]) {
            draw.light = light;
            draw.rotate_about([0.0, 1.0, 0.0], 180.0 - facing_yaw(facing), [0.0, 0.0, 0.0]);
            draw.rotate_about([0.0, 0.0, 1.0], swing, [0.16, -0.16, 0.0]);
            draw.scale([0.38, 0.38, 0.38]);
            game.native_draw_item(&mut *draw, &ItemStack::new("minecraft:mace", 1), DisplayContext::Fixed, None, 5);
        }
        draw.pop();
    }
}

/// Makes what a station that works from the inventory makes, from the slots the player laid on
/// it, paying as its menu's result slot does.
fn make(game: &mut Game<'_>, uuid: u128, kind: Kind, pos: [i32; 3], times: u32, extra: &[u8]) {
    use horizon_client::stations;
    if extra.len() < 4 {
        return;
    }
    let refs: [Option<usize>; 3] = [extra[0], extra[1], extra[2]].map(|v| (v > 0 && v <= 36).then(|| usize::from(v - 1)));
    let choice = (extra[3] > 0).then(|| usize::from(extra[3] - 1));
    match kind {
        Kind::Enchanting => return enchant(game, uuid, pos, refs, choice),
        Kind::Beacon => return power(game, pos, refs, extra),
        Kind::Cartography => return chart(game, pos, refs),
        _ => {}
    }
    let name = std::str::from_utf8(&extra[4..]).unwrap_or("").to_owned();
    let mut made_any = false;
    for _ in 0..times.max(1) {
        let stacks = laid(game, &refs);
        let m = made(game, kind, &stacks, &name, choice, None);
        let Some(result) = m.result else { break };
        match kind {
            // LoomMenu's result: one banner and one dye go, the pattern item stays.
            Kind::Loom => {
                for r in refs.iter().take(2).flatten() {
                    take_slot(game, *r, 1);
                }
                game.give_or_drop(result);
                game.native_play_sound_in_place("minecraft:ui.loom.take_result", center(pos), 4, 1.0, 1.0);
                made_any = true;
                break;
            }

            Kind::Anvil => {
                // AnvilMenu.mayPickup and onTake.
                if !game.creative && (m.cost <= 0 || m.cost >= 40 || game.experience.level < m.cost) {
                    break;
                }
                let hover = stacks[0].as_ref().map(|i| stations::hover_name(&game.pack.names, i)).unwrap_or_default();
                let r = stations::anvil_result(&game.stations().enchantments, game.items(), stacks[0].as_ref(), stacks[1].as_ref(), Some(&name), &hover, game.creative);
                if let Some(slot) = refs[0] {
                    game.inventory[slot] = None;
                }
                if let (Some(slot), Some(_)) = (refs[1], &stacks[1]) {
                    if r.repair_item_count_cost > 0 {
                        take_slot(game, slot, r.repair_item_count_cost);
                    } else {
                        game.inventory[slot] = None;
                    }
                }
                if !game.creative {
                    game.give_levels(-m.cost);
                }
                game.give_or_drop(result);
                let creative = game.creative;
                game.anvil_used(pos, creative);
                made_any = true;
                break;
            }
            Kind::Grindstone => {
                let amount = stations::grindstone_xp_base(&game.stations().enchantments, stacks[0].as_ref(), stacks[1].as_ref());
                for r in refs.iter().take(2).flatten() {
                    take_slot(game, *r, 1);
                }
                game.give_or_drop(result);
                game.grindstone_taken(pos, amount);
                made_any = true;
                break;
            }
            Kind::Smithing => {
                for (i, r) in refs.iter().enumerate() {
                    if let (Some(r), Some(_)) = (r, &stacks[i]) {
                        take_slot(game, *r, 1);
                    }
                }
                game.give_or_drop(result);
                game.level_event_at(1044, pos, 0);
                made_any = true;
            }
            Kind::Stonecutter => {
                let Some(slot) = refs[0] else { break };
                if take_slot(game, slot, 1).is_none() {
                    break;
                }
                game.give_or_drop(result);
                made_any = true;
            }
            _ => break,
        }
    }
    if made_any && kind == Kind::Stonecutter {
        game.native_play_sound_in_place("minecraft:ui.stonecutter.take_result", [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.6, f64::from(pos[2]) + 0.5], 4, 1.0, 1.0);
    }
}

/// `EnchantmentMenu.clickMenuButton`: the item laid on the table gets the offer's enchantments
/// (a book becomes an enchanted book); the player pays its lapis and levels and gets a new seed.
fn enchant(game: &mut Game<'_>, uuid: u128, pos: [i32; 3], refs: [Option<usize>; 3], choice: Option<usize>) {
    use horizon_client::stations;
    let (Some(button), Some(slot)) = (choice.filter(|&c| c < 3), refs[0]) else { return };
    let Some(item) = game.inventory.get(slot).cloned().flatten() else { return };
    let Some(seed) = seed_of(uuid) else { return };
    let shelves = game.count_bookshelves(pos);
    let offers = stations::enchant_offers(&game.stations().enchantments, game.items(), seed, shelves, Some(&item));
    let cost = offers.costs[button];
    let need = button as i32 + 1;
    let lapis = refs[1].and_then(|r| game.inventory.get(r).cloned().flatten()).filter(|l| &*l.item == "minecraft:lapis_lazuli");
    let have = lapis.as_ref().map_or(0, |l| l.count as i32);
    if cost <= 0 || (!game.creative && (have < need || game.experience.level < need || game.experience.level < cost)) {
        return;
    }
    let one = ItemStack { count: 1, ..item.clone() };
    let Some(enchanted) = stations::apply_enchantment(&game.stations().enchantments, game.items(), seed, button, cost, &one) else { return };
    // Player.onEnchantmentPerformed, the stat and the trigger, then the lapis.
    game.give_levels(-need);
    game.award_custom_stat("enchant_item", 1);
    game.trigger_enchanted_item(&enchanted, need);
    if !game.creative
        && let Some(r) = refs[1]
    {
        take_slot(game, r, need as u32);
    }
    // The table holds one item: of a stack, one is enchanted and the rest stay.
    if item.count > 1 {
        take_slot(game, slot, 1);
        game.give_or_drop(enchanted);
    } else {
        game.inventory[slot] = Some(enchanted);
    }
    RESEED.lock().unwrap_or_else(|e| e.into_inner()).push(uuid);
    if let Some(all) = SEEDS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        all.remove(&uuid);
    }
    let pitch = game.world.random.next_float() * 0.1 + 0.9;
    game.native_play_sound_in_place("minecraft:block.enchantment_table.use", center(pos), 4, 1.0, pitch);
}

/// CartographyTableMenu's result taken: one of each input goes, and the map is made anew.
fn chart(game: &mut Game<'_>, pos: [i32; 3], refs: [Option<usize>; 3]) {
    let (Some(map), Some(more)) = (refs[0], refs[1]) else { return };
    let stacks = laid(game, &refs);
    if !stacks.iter().take(2).enumerate().all(|(i, st)| st.as_ref().is_some_and(|st| part_takes(game, Kind::Cartography, i, st))) {
        return;
    }
    let Some(out) = carto_result(game, &stacks, true) else { return };
    take_slot(game, map, 1);
    take_slot(game, more, 1);
    game.give_or_drop(out);
    game.native_play_sound_in_place("minecraft:ui.cartography_table.take_result", center(pos), 4, 1.0, 1.0);
}

/// `BeaconMenu.updateEffects`: with a payment laid, the powers the pyramid allows become the
/// beacon's and the payment is used (the power sound while its beam is up).
fn power(game: &mut Game<'_>, pos: [i32; 3], refs: [Option<usize>; 3], extra: &[u8]) {
    use horizon_client::containers::BlockEntity;
    let effects = super::beacon::EFFECTS;
    let (Some(slot), Some(&p), Some(&q)) = (refs[0], extra.get(4), extra.get(5)) else { return };
    let Some(pay) = game.inventory.get(slot).cloned().flatten() else { return };
    if !part_takes(game, Kind::Beacon, 0, &pay) {
        return;
    }
    let pick = |v: u8| (v > 0 && usize::from(v) <= effects.len()).then(|| format!("minecraft:{}", effects[usize::from(v) - 1].0));
    let (primary, secondary) = (pick(p), pick(q));
    let key = (pos[0], pos[1], pos[2]);
    let Some(BlockEntity::Beacon(b)) = game.world.block_entities.get(&key) else { return };
    let levels = b.levels;
    let need = |e: &Option<String>| horizon_client::menu::beacon_required_levels(e.as_deref());
    let (np, ns) = (need(&primary), need(&secondary));
    let valid = primary.is_some()
        && !(secondary.is_some() && levels < 4)
        && np <= levels
        && ns <= levels
        && np < 4
        && !(ns != 0 && ns < 4 && primary != secondary);
    if !valid {
        return;
    }
    take_slot(game, slot, 1);
    let mut beam = false;
    if let Some(BlockEntity::Beacon(b)) = game.world.block_entities.get_mut(&key) {
        b.primary = primary;
        b.secondary = secondary;
        beam = !b.sections.is_empty();
    }
    game.mark_block_entity_updated(key);
    game.world.modified.insert((key.0 >> 4, key.2 >> 4));
    if beam {
        game.native_play_sound_in_place("minecraft:block.beacon.power_select", center(pos), 4, 1.0, 1.0);
    }
}
