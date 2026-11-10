// Hands-On Delight for Horizon, written from its documented and shown behaviour (its page and
// showcase pictures; none of its code): Farmer's Delight's cooking pot and cutting board work in
// place, without a menu.
//  - A right click flies the camera to the station; the hotbar lies along its front, a recipe book
//    beside it shows what the station can make. Escape, E, a movement key or jump leaves.
//  - Cooking pot: click a hotbar item, then the pot, to drop it in (a right click drops one).
//    Stir by holding the button and drawing wide, steady rounds in the pot: each round hurries the
//    cooking, and steady rounds fill the stars over the dish, each adding servings. The meal card
//    beside the pot serves one into its container from the inventory.
//  - Cutting board: put an item on the board from the hotbar, pick up its tool and drag it across
//    the board to cut; the pieces go to the inventory. A click with nothing in hand takes it back.
use super::farmersdelight as fd;
use super::{Game, Mapping};
use horizon_assets::item_model::DisplayContext;
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{HudMode, NativeScreenView};
use horizon_world::nbt::Tag;
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use winit::event::MouseButton;

pub const ID: &str = "handson_delight";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str =
    "Farmer's Delight's cooking pot and cutting board worked by hand in the world: stir the soup, cut on the board.";
const CHANNEL: &str = "handson_delight:act";
const ENTER: f32 = 0.45;
const LEAVE: f32 = 0.35;
const SLOT: f32 = 20.0;
const CARD: [f32; 2] = [34.0, 46.0];
const BOOK: [f32; 2] = [128.0, 76.0];

const PUT: u8 = 1;
const TAKE: u8 = 2;
const STIR: u8 = 3;
const SERVE: u8 = 4;
const CUT: u8 = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pot,
    Board,
}

type Pose = ([f64; 3], f32, f32);

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
    hud: HudMode,
    logged: bool,
}

struct Client {
    scene: Option<Scene>,
    out: Vec<Vec<u8>>,
    close: bool,
    /// The swirl of the pot's contents (radians) and how fast it turns.
    swirl: f32,
    spin: f32,
    last: Option<Instant>,
    /// The spoon's place on the pot's surface, while the pointer is over it.
    spoon: Option<[f64; 3]>,
    /// The screen's size in GUI pixels and the vertical field of view, from the last frame drawn.
    size: [f32; 2],
    fov: f32,
}

static CLIENT: Mutex<Client> = Mutex::new(Client {
    scene: None,
    out: Vec::new(),
    close: false,
    swirl: 0.0,
    spin: 0.0,
    last: None,
    spoon: None,
    size: [0.0, 0.0],
    fov: 70.0,
});

fn client<T>(f: impl FnOnce(&mut Client) -> T) -> T {
    f(&mut CLIENT.lock().unwrap_or_else(|e| e.into_inner()))
}

// ---- the camera ----

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far the camera is from the player's eyes to the station, 0 to 1.
fn progress(s: &Scene) -> f32 {
    let enter = smooth(s.started.elapsed().as_secs_f32() / ENTER);
    match s.leaving {
        Some(at) => enter.min(1.0 - smooth(at.elapsed().as_secs_f32() / LEAVE)),
        None => enter,
    }
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
fn framing(kind: Kind, pos: [i32; 3], player_yaw: f32) -> Pose {
    let side = (player_yaw / 90.0).round() * 90.0;
    let (pitch, distance, height) = match kind {
        Kind::Pot => (55.0f32, 1.25, 0.62),
        Kind::Board => (66.0f32, 1.4, 0.06),
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

/// The world point under the pointer on the level `y`.
fn pointer_on(pose: Pose, fov: f32, size: [f32; 2], mouse: [f32; 2], y: f64) -> Option<[f64; 3]> {
    let (f, r, u) = basis(pose.1, pose.2);
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0] / size[1].max(1.0));
    let nx = f64::from(mouse[0] / size[0].max(1.0)) * 2.0 - 1.0;
    let ny = 1.0 - f64::from(mouse[1] / size[1].max(1.0)) * 2.0;
    let dir = [0, 1, 2].map(|i| f[i] + r[i] * nx * t * aspect + u[i] * ny * t);
    if dir[1].abs() < 1e-6 {
        return None;
    }
    let k = (y - pose.0[1]) / dir[1];
    (k > 0.0).then(|| [pose.0[0] + dir[0] * k, y, pose.0[2] + dir[2] * k])
}

/// The level of a station's working surface, and its half width.
fn surface(kind: Kind, pos: [i32; 3]) -> (f64, f64) {
    match kind {
        Kind::Pot => (f64::from(pos[1]) + 0.63, 0.31),
        Kind::Board => (f64::from(pos[1]) + 0.07, 0.45),
    }
}

fn station_center(pos: [i32; 3], y: f64) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, y, f64::from(pos[2]) + 0.5]
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
    let state = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
    let kind = match game.native_state_block(state) {
        Some(fd::POT) => Kind::Pot,
        Some(fd::BOARD) => Kind::Board,
        _ => return false,
    };
    if !game.native_mod_screen_open(ID) {
        return false;
    }
    let hud = game.native_hud_mode();
    game.native_set_hud_mode(HudMode::AllHidden);
    let from = (eye, game.yaw, game.pitch);
    let to = framing(kind, hit.pos, game.yaw);
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
            hud,
            logged: false,
        });
        c.close = false;
    });
    true
}

/// Once a client tick: leaving when the screen closed, the station's messages sent, the swirl.
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
            c.spoon = None;
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

/// The station's block entity's items, as the server sent them.
fn shown_items(game: &Game<'_>, pos: [i32; 3], count: usize) -> (Vec<Option<ItemStack>>, i32) {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) => {
            let items = horizon_items::item_nbt::decode_items(m.nbt.get(&fd::key("Items")), count);
            let stars = match m.nbt.get(&fd::key("Stars")) {
                Some(Tag::Int(s)) => *s,
                _ => 0,
            };
            (items, stars)
        }
        _ => (vec![None; count], 0),
    }
}

/// Where the hotbar strip's slots are: along the station's front, below it.
fn strip(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (f, _, _) = basis(pose.1, pose.2);
    let h = (f[0] * f[0] + f[2] * f[2]).sqrt().max(1e-6);
    let front = [f64::from(pos[0]) + 0.5 - f[0] / h * 0.5, f64::from(pos[1]) + 0.12, f64::from(pos[2]) + 0.5 - f[2] / h * 0.5];
    let at = project(pose, fov, size, front)?;
    Some([at[0] - SLOT * 4.5, at[1] - SLOT / 2.0])
}

fn strip_slot(origin: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    let x = mouse[0] - origin[0];
    let y = mouse[1] - origin[1];
    (x >= 0.0 && y >= 0.0 && x < SLOT * 9.0 && y < SLOT).then(|| (x / SLOT) as usize)
}

/// The meal card beside the pot (its top left), on the camera's right.
fn card(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (_, r, _) = basis(pose.1, pose.2);
    let p = [f64::from(pos[0]) + 0.5 + r[0] * 0.78, f64::from(pos[1]) + 0.75, f64::from(pos[2]) + 0.5 + r[2] * 0.78];
    project(pose, fov, size, p).map(|a| [a[0] - CARD[0] / 2.0, a[1] - CARD[1] / 2.0])
}

/// The recipe book's top left, on the camera's left.
fn book(pose: Pose, fov: f32, size: [f32; 2], pos: [i32; 3]) -> Option<[f32; 2]> {
    let (_, r, _) = basis(pose.1, pose.2);
    let p = [f64::from(pos[0]) + 0.5 - r[0] * 1.05, f64::from(pos[1]) + 0.55, f64::from(pos[2]) + 0.5 - r[2] * 1.05];
    project(pose, fov, size, p).map(|a| [(a[0] - BOOK[0] / 2.0).clamp(8.0, size[0] - BOOK[0] - 8.0), (a[1] - BOOK[1] / 2.0).clamp(8.0, size[1] - BOOK[1] - 30.0)])
}

fn inside(at: [f32; 2], wh: [f32; 2], m: [f32; 2]) -> bool {
    m[0] >= at[0] && m[1] >= at[1] && m[0] < at[0] + wh[0] && m[1] < at[1] + wh[1]
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
        Some((current(s), s.pos, s.kind, s.carried, progress(s) >= 0.999 && s.leaving.is_none()))
    }) else {
        return;
    };
    if !entered {
        return;
    }
    // The spoon follows the pointer over the pot.
    if kind == Kind::Pot {
        let (y, half) = surface(kind, pos);
        let c0 = station_center(pos, y);
        let spoon = pointer_on(pose, fov, size, mouse, y).filter(|p| (p[0] - c0[0]).abs() < half && (p[2] - c0[2]).abs() < half);
        client(|c| c.spoon = spoon);
    }
    let count = if kind == Kind::Pot { fd::MEAL + 1 } else { 1 };
    let (items, stars) = shown_items(game, pos, count);
    // Once a scene, where its parts landed on the screen (for tests that click them).
    if client(|c| c.scene.as_mut().is_some_and(|s| !std::mem::replace(&mut s.logged, true))) {
        let strip_at = strip(pose, fov, size, pos).unwrap_or_default();
        let card_at = card(pose, fov, size, pos).unwrap_or_default();
        let (y, _) = surface(kind, pos);
        let middle = project(pose, fov, size, station_center(pos, y)).unwrap_or_default();
        log::info!(
            "[handson_delight] scene at strip {:.0} {:.0} card {:.0} {:.0} middle {:.0} {:.0}",
            strip_at[0] + SLOT / 2.0, strip_at[1] + SLOT / 2.0, card_at[0] + CARD[0] / 2.0, card_at[1] + CARD[1] / 2.0, middle[0], middle[1]
        );
    }
    // The hotbar along the front.
    if let Some(o) = strip(pose, fov, size, pos) {
        game.native_fill(batch, [o[0] - 2.0, o[1] - 2.0, SLOT * 9.0 + 4.0, SLOT + 4.0], 0xC020_1810);
        for i in 0..9 {
            let at = [o[0] + i as f32 * SLOT, o[1]];
            let hover = strip_slot(o, mouse) == Some(i);
            let color = if carried == Some(i) {
                0xE0C8A040
            } else if hover {
                0xC0807060
            } else {
                0xC0504030
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
        Kind::Pot => draw_pot_parts(game, batch, pose, fov, size, pos, &items, stars, mouse),
        Kind::Board => draw_board_parts(game, batch, pose, fov, size, pos, &items),
    }
    // The carried stack on the pointer.
    if let Some(slot) = carried
        && let Some(Some(stack)) = game.inventory.get(slot)
    {
        draw_stack(game, batch, stack, [mouse[0] - 8.0, mouse[1] - 8.0]);
    }
    let hint = match kind {
        Kind::Pot => "Pick an item, click the pot to add it  -  hold and circle in the pot to stir  -  Esc leaves",
        Kind::Board => "Put an item on the board  -  pick up its tool and drag it across to cut  -  Esc leaves",
    };
    let w = hint.len() as f32 * 5.0;
    game.native_text(batch, hint, [(size[0] - w) / 2.0, size[1] - 14.0], 0xE0E0E0, true);
}

fn draw_stack(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, at: [f32; 2]) {
    game.native_item(batch, &stack.item, at, 16.0);
    if stack.count > 1 {
        let n = stack.count.to_string();
        game.native_text(batch, &n, [at[0] + 17.0 - n.len() as f32 * 6.0, at[1] + 9.0], 0xFFFFFF, true);
    }
}

fn first_item(ingredient: &horizon_client::crafting::Ingredient) -> Option<&str> {
    ingredient.order.first().map(String::as_str)
}

#[allow(clippy::too_many_arguments)]
fn draw_pot_parts(
    game: &Game<'_>,
    batch: &mut HudBatch,
    pose: Pose,
    fov: f32,
    size: [f32; 2],
    pos: [i32; 3],
    items: &[Option<ItemStack>],
    stars: i32,
    mouse: [f32; 2],
) {
    // The stars over the dish.
    if let Some(a) = project(pose, fov, size, station_center(pos, f64::from(pos[1]) + 0.95)) {
        for i in 0..3 {
            let x = a[0] - 33.0 + i as f32 * 22.0;
            let color = if i < stars { 0xFFFFD040 } else { 0xA0303030 };
            game.native_fill(batch, [x + 6.0, a[1], 6.0, 18.0], color);
            game.native_fill(batch, [x, a[1] + 6.0, 18.0, 6.0], color);
            game.native_fill(batch, [x + 3.0, a[1] + 3.0, 12.0, 12.0], color);
        }
        let bonus = format!("+{}%", stars * 25);
        game.native_text(batch, &bonus, [a[0] + 36.0, a[1] + 5.0], if stars > 0 { 0xFFD040 } else { 0xA0A0A0 }, true);
    }
    // The meal card: the meal waiting in the pot, served by a click.
    let meal = items.get(fd::MEAL).cloned().flatten();
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
    // The recipe book: the meal the ingredients point to, with what it takes.
    let inputs: Vec<&ItemStack> = items.iter().take(fd::INPUTS).flatten().collect();
    let meals = fd::meals(game);
    let best = meals
        .iter()
        .filter(|m| inputs.iter().all(|s| m.inputs.iter().any(|i| i.test(s))))
        .max_by_key(|m| m.inputs.iter().filter(|i| inputs.iter().any(|s| i.test(s))).count());
    if let Some(at) = book(pose, fov, size, pos) {
        draw_book(game, batch, at);
        match best {
            Some(m) if !inputs.is_empty() => {
                game.native_item(batch, &m.result.item, [at[0] + 84.0, at[1] + 8.0], 28.0);
                for (i, ingredient) in m.inputs.iter().enumerate() {
                    let cell = [at[0] + 5.0 + (i % 3) as f32 * 19.0, at[1] + 8.0 + (i / 3) as f32 * 19.0];
                    let have = inputs.iter().any(|s| ingredient.test(s));
                    game.native_fill(batch, [cell[0], cell[1], 18.0, 18.0], if have { 0x6040A040 } else { 0x40806040 });
                    if let Some(item) = first_item(ingredient) {
                        game.native_item(batch, item, [cell[0] + 1.0, cell[1] + 1.0], 16.0);
                    }
                }
                if let Some(container) = &m.container {
                    game.native_text(batch, "in", [at[0] + 76.0, at[1] + 52.0], 0x604020, false);
                    game.native_item(batch, container, [at[0] + 92.0, at[1] + 48.0], 16.0);
                }
            }
            _ => game.native_text(batch, "Add ingredients", [at[0] + 8.0, at[1] + 26.0], 0x604020, false),
        }
    }
}

fn draw_book(game: &Game<'_>, batch: &mut HudBatch, at: [f32; 2]) {
    game.native_fill(batch, [at[0] - 3.0, at[1] - 3.0, BOOK[0] + 6.0, BOOK[1] + 6.0], 0xFFB07818);
    game.native_fill(batch, [at[0], at[1], BOOK[0], BOOK[1]], 0xFFF4EAD0);
    game.native_fill(batch, [at[0] + BOOK[0] / 2.0 - 1.0, at[1], 2.0, BOOK[1]], 0xFFD8C8A0);
}

fn draw_board_parts(
    game: &Game<'_>,
    batch: &mut HudBatch,
    pose: Pose,
    fov: f32,
    size: [f32; 2],
    pos: [i32; 3],
    items: &[Option<ItemStack>],
) {
    let Some(at) = book(pose, fov, size, pos) else { return };
    draw_book(game, batch, at);
    let Some(Some(item)) = items.first() else {
        game.native_text(batch, "Put food here", [at[0] + 8.0, at[1] + 26.0], 0x604020, false);
        return;
    };
    let cuts = fd::cuts(game);
    let Some(cut) = cuts.iter().find(|c| c.input.test(item)) else {
        game.native_text(batch, "Nothing to cut", [at[0] + 8.0, at[1] + 26.0], 0x604020, false);
        return;
    };
    game.native_item(batch, &item.item, [at[0] + 8.0, at[1] + 8.0], 24.0);
    if let Some(tool) = first_item(&cut.tool) {
        game.native_item(batch, tool, [at[0] + 12.0, at[1] + 42.0], 20.0);
    }
    for (i, (result, _)) in cut.results.iter().enumerate().take(4) {
        draw_stack(game, batch, result, [at[0] + 74.0 + (i % 2) as f32 * 24.0, at[1] + 10.0 + (i / 2) as f32 * 26.0]);
    }
}

// ---- the pointer ----

pub(crate) fn press(game: &Game<'_>, view: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    let mouse = view.mouse;
    let items_board = shown_items(game, current_pos(), 1).0;
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(s) = c.scene.as_mut() else { return };
        if s.leaving.is_some() || progress(s) < 0.999 {
            return;
        }
        let pose = current(s);
        let (pos, kind) = (s.pos, s.kind);
        // The strip: pick a stack up, or put it back.
        if let Some(o) = strip(pose, fov, size, pos)
            && let Some(slot) = strip_slot(o, mouse)
        {
            s.carried = if s.carried == Some(slot) || game.inventory.get(slot).is_none_or(Option::is_none) {
                None
            } else {
                Some(slot)
            };
            return;
        }
        let (y, half) = surface(kind, pos);
        let c0 = station_center(pos, y);
        let on = pointer_on(pose, fov, size, mouse, y)
            .filter(|p| (p[0] - c0[0]).abs() < half + 0.04 && (p[2] - c0[2]).abs() < half + 0.04);
        match kind {
            Kind::Pot => {
                if let Some(at) = card(pose, fov, size, pos)
                    && inside(at, CARD, mouse)
                {
                    send(c, SERVE, pos, 0);
                    return;
                }
                let Some(point) = on else { return };
                if let Some(slot) = s.carried {
                    let all = button == 0;
                    if all {
                        s.carried = None;
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
                let Some(point) = on else { return };
                let empty = items_board.first().is_none_or(Option::is_none);
                let carried = s.carried;
                match carried {
                    Some(slot) if empty => {
                        s.carried = None;
                        send(c, PUT, pos, slot as u8);
                    }
                    Some(_) => s.cut_from = Some(point),
                    None if !empty => send(c, TAKE, pos, 0),
                    None => {}
                }
            }
        }
    });
}

fn current_pos() -> [i32; 3] {
    client(|c| c.scene.as_ref().map_or([0, i32::MIN, 0], |s| s.pos))
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
    client(|c| {
        let (size, fov) = (c.size, c.fov);
        let Some(s) = c.scene.as_mut() else { return };
        s.stir = None;
        let Some(from) = s.cut_from.take() else { return };
        let pose = current(s);
        let (y, _) = surface(s.kind, s.pos);
        let (pos, carried) = (s.pos, s.carried);
        if let (Some(to), Some(slot)) = (pointer_on(pose, fov, size, mouse, y), carried) {
            let d = ((to[0] - from[0]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
            if d >= 0.4 {
                send(c, CUT, pos, slot as u8);
            }
        }
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

/// The pot's contents swirling in its water, and the spoon at the pointer.
pub(crate) fn submit(
    game: &Game<'_>,
    pos: [i32; 3],
    block: &str,
    _state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    if block != fd::POT || !game.native_on(ID) {
        return;
    }
    let items = horizon_items::item_nbt::decode_items(entity.nbt.get(&fd::key("Items")), fd::MEAL + 1);
    let inputs: Vec<&ItemStack> = items.iter().take(fd::INPUTS).flatten().collect();
    let here = client(|c| c.scene.as_ref().is_some_and(|s| s.pos == pos));
    if inputs.is_empty() && !here && items.get(fd::MEAL).is_none_or(Option::is_none) {
        return;
    }
    let (swirl, spoon, yaw) = client(|c| {
        let now = Instant::now();
        let dt = c.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        c.last = Some(now);
        c.swirl += (0.3 + c.spin) * dt;
        c.spin *= (1.0 - dt * 1.5).max(0.0);
        let yaw = c.scene.as_ref().map_or(0.0, |s| s.to.1);
        (c.swirl, if here { c.spoon } else { None }, yaw)
    });
    // The water, browner as more goes in.
    let level: f32 = 0.632;
    let fill = (inputs.len() as f32 / 4.0).min(1.0);
    let mixc = |a: u32, b: u32| (a as f32 + (b as f32 - a as f32) * fill) as u32;
    let tint = if inputs.is_empty() {
        0xFF3F_76E4
    } else {
        0xFF00_0000 | mixc(0xD8, 0x9A) << 16 | mixc(0xB8, 0x60) << 8 | mixc(0x78, 0x30)
    };
    let (a, b): (f32, f32) = (3.0 / 16.0, 13.0 / 16.0);
    let quad = [[a, level, a], [a, level, b], [b, level, b], [b, level, a]];
    if !game.native_draw_quad(draw, "block/water_still", quad, [0.0, 0.0, 16.0, 16.0], tint) {
        game.native_draw_quad(draw, "block/light_blue_concrete", quad, [0.0, 0.0, 16.0, 16.0], tint);
    }
    // The ingredients float round the middle.
    let n = inputs.len().max(1) as f32;
    for (i, stack) in inputs.iter().enumerate() {
        let a = swirl + i as f32 * std::f32::consts::TAU / n;
        let r = 0.17;
        let bob = (swirl * 2.0 + i as f32).sin() * 0.01;
        draw.push();
        draw.translate([0.5 + a.cos() * r, level + 0.02 + bob, 0.5 + a.sin() * r]);
        draw.rotate_about([0.0, 1.0, 0.0], -a.to_degrees(), [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], 70.0, [0.0, 0.0, 0.0]);
        draw.scale([0.2, 0.2, 0.2]);
        game.native_draw_item(draw, stack, DisplayContext::Fixed, None, i as i32);
        draw.pop();
    }
    // The spoon, standing in the pot where the pointer is.
    if let Some(p) = spoon {
        let local = [(p[0] - f64::from(pos[0])) as f32, (p[2] - f64::from(pos[2])) as f32];
        let stick = ItemStack::new("minecraft:stick", 1);
        draw.push();
        // A stick stands upright in the water, leaning a little, its flat side to the camera.
        draw.translate([local[0], level + 0.2, local[1]]);
        draw.rotate_about([0.0, 1.0, 0.0], 180.0 - yaw, [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], -15.0, [0.0, 0.0, 0.0]);
        draw.rotate_about([0.0, 0.0, 1.0], -45.0, [0.0, 0.0, 0.0]);
        draw.scale([0.5, 0.5, 0.5]);
        game.native_draw_item(draw, &stick, DisplayContext::Fixed, None, 7);
        draw.pop();
    }
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
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let kind = match game.native_state_block(state) {
        Some(fd::POT) => Kind::Pot,
        Some(fd::BOARD) => Kind::Board,
        _ => return,
    };
    let key = (pos[0], pos[1], pos[2]);
    if game.world.block_entities.get(&key).is_none() {
        let name = if kind == Kind::Pot { fd::POT } else { fd::BOARD };
        if let Some(entity) = game.new_block_entity(name) {
            game.world.block_entities.insert(key, entity);
        }
    }
    match (kind, op) {
        (Kind::Pot, PUT) => pot_put(game, pos, usize::from(arg & 0x7f), arg & 0x80 != 0),
        (Kind::Pot, TAKE) => pot_take(game, pos),
        (Kind::Pot, STIR) => {
            fd::stirred(game, pos, arg.min(100));
            game.native_play_sound_in_place("farmersdelight:block.cooking_pot.boil", fd::center(pos, 0.6), 4, 0.4, 1.2);
        }
        (Kind::Pot, SERVE) => pot_serve(game, pos),
        (Kind::Board, PUT) => board_put(game, pos, usize::from(arg & 0x7f)),
        (Kind::Board, TAKE) => {
            if let Some(Some(item)) = fd::items_at(game, pos).into_iter().next() {
                fd::set_slot(game, pos, 0, None);
                game.give_or_drop(item);
                game.native_play_sound_in_place("farmersdelight:block.cutting_board.remove_item", fd::center(pos, 0.1), 4, 1.0, 1.0);
            }
        }
        (Kind::Board, CUT) => board_cut(game, pos, usize::from(arg)),
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
    if fd::items_at(game, pos).first().is_some_and(Option::is_some) {
        return;
    }
    let Some(one) = take_from_slot(game, slot, 1) else { return };
    fd::set_slot(game, pos, 0, Some(one));
    game.native_play_sound_in_place("farmersdelight:block.cutting_board.place_item", fd::center(pos, 0.1), 4, 1.0, 0.8);
}

fn board_cut(game: &mut Game<'_>, pos: [i32; 3], slot: usize) {
    let Some(Some(item)) = fd::items_at(game, pos).into_iter().next() else { return };
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
