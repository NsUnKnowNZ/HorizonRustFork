// Camerapture (chrrs, MIT) for Horizon: picture frames, albums and the lectern.
//  - `PictureFrameEntity`: Horizon's mod content has no hanging entity to copy, so a frame is a
//    record in a synced chunk attachment (`camerapture:frames`, saved with the chunk, sent to
//    the clients that see it), at the block in front of the wall it faces from, as
//    `ResizableDecorationEntity` keeps its attachment position. Sneak + use a picture on a
//    wall hangs one; use on it turns its picture, sneak + use opens its editor (size from 1x1
//    to 16x16, glowing, fixed); attacking it breaks it and drops the picture.
//  - `AlbumScreenHandler`: an album's 36 pictures in a chest menu of four rows (Horizon's
//    server menus), each click moving a picture between the album and the inventory.
//  - `LecternBlockEntityMixin`: an album lies on a lectern (`#minecraft:lectern_books`); using
//    the lectern shows its pictures with a Take Book button (`AlbumLecternScreen`).
use super::camerapture::{self as cam, PictureData};
use super::camerapture_client as client;
use super::Game;
use super::native_world_draw::NativeWorldDraw;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, Tag};
use horizon_client::text::Text;
use std::sync::Mutex;

// Client to server.
pub(crate) const FRAME_BUTTON: &str = "camerapture:frame_button";
pub(crate) const TAKE_BOOK: &str = "camerapture:take_book";
// Server to client.
pub(crate) const FRAME_SCREEN: &str = "camerapture:frame_screen";

/// `ResizableDecorationEntity` and `PictureFrameEntity`'s tracked data.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Frame {
    pub id: u64,
    /// `attachmentPos`: the bottom block of the side the frame grows from.
    pub at: [i32; 3],
    /// `Direction.getId` of the way it faces: 2 north, 3 south, 4 west, 5 east.
    pub facing: u8,
    pub width: u8,
    pub height: u8,
    pub picture: PictureData,
    /// The picture stack's custom name.
    pub name: Option<String>,
    pub glowing: bool,
    pub fixed: bool,
    pub rotation: u8,
}

/// The frames of one chunk.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Frames(pub Vec<Frame>);

fn byte(b: bool) -> Tag {
    Tag::Byte(i8::from(b))
}
fn encode(f: &Frames) -> Result<Compound, AttachmentError> {
    let list = f
        .0
        .iter()
        .map(|f| {
            let mut c = vec![
                ("id", Tag::Long(f.id as i64)),
                ("at", Tag::IntArray(f.at.to_vec())),
                ("facing", Tag::Byte(f.facing as i8)),
                ("width", Tag::Byte(f.width as i8)),
                ("height", Tag::Byte(f.height as i8)),
                ("picture", f.picture.to_tag()),
                ("glowing", byte(f.glowing)),
                ("fixed", byte(f.fixed)),
                ("rotation", Tag::Byte(f.rotation as i8)),
            ];
            if let Some(name) = &f.name {
                c.push(("name", Tag::String(name.as_str().into())));
            }
            Tag::compound(c)
        })
        .collect();
    Ok([("frames".into(), Tag::List(list))].into())
}
fn decode(data: &Compound) -> Result<Frames, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"frames".into()) else {
        return Err(AttachmentError::Decode);
    };
    let mut out = Vec::new();
    for t in list {
        let int = |k: &str| match t.get(k) {
            Some(Tag::Byte(b)) => i64::from(*b),
            Some(Tag::Long(l)) => *l,
            Some(Tag::Int(i)) => i64::from(*i),
            _ => 0,
        };
        let Some(Tag::IntArray(at)) = t.get("at") else {
            continue;
        };
        let Ok(at) = <[i32; 3]>::try_from(at.as_slice()) else {
            continue;
        };
        let Some(picture) = t.get("picture").and_then(PictureData::from_tag) else {
            continue;
        };
        let name = match t.get("name") {
            Some(Tag::String(s)) => Some(s.to_string_lossy()),
            _ => None,
        };
        out.push(Frame {
            id: int("id") as u64,
            at,
            facing: (int("facing") as u8).clamp(2, 5),
            width: (int("width") as u8).clamp(1, 16),
            height: (int("height") as u8).clamp(1, 16),
            picture,
            name,
            glowing: int("glowing") != 0,
            fixed: int("fixed") != 0,
            rotation: (int("rotation") as u8) % 4,
        });
    }
    Ok(Frames(out))
}

pub(crate) static FRAMES: AttachmentType<Frames> = AttachmentType {
    namespace: "camerapture",
    name: "frames",
    default: Frames::default,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

// --- Geometry ------------------------------------------------------------------------------

/// `Direction.getVector` of a horizontal direction id.
fn dir(facing: u8) -> [i32; 3] {
    match facing {
        2 => [0, 0, -1],
        3 => [0, 0, 1],
        4 => [-1, 0, 0],
        _ => [1, 0, 0],
    }
}
fn facing_of(face: [i32; 3]) -> Option<u8> {
    match face {
        [0, 0, -1] => Some(2),
        [0, 0, 1] => Some(3),
        [-1, 0, 0] => Some(4),
        [1, 0, 0] => Some(5),
        _ => None,
    }
}
/// `Direction.rotateYCounterclockwise`: the way a frame grows its width.
fn side(facing: u8) -> [i32; 3] {
    match facing {
        2 => [-1, 0, 0], // north: west
        3 => [1, 0, 0],  // south: east
        4 => [0, 0, 1],  // west: south
        _ => [0, 0, -1], // east: north
    }
}
fn add(a: [i32; 3], b: [i32; 3], n: i32) -> [i32; 3] {
    [a[0] + b[0] * n, a[1] + b[1] * n, a[2] + b[2] * n]
}
impl Frame {
    /// Whether the block `cell` is one the frame covers.
    fn covers(&self, cell: [i32; 3]) -> bool {
        let s = side(self.facing);
        let d = [cell[0] - self.at[0], cell[1] - self.at[1], cell[2] - self.at[2]];
        let along = d[0] * s[0] + d[2] * s[2];
        let f = dir(self.facing);
        let depth = d[0] * f[0] + d[2] * f[2];
        depth == 0 && (0..i32::from(self.width)).contains(&along) && (0..i32::from(self.height)).contains(&d[1])
    }
    /// The wall's plane, the frame's middle on it and its axes: right (as seen from the
    /// front) and up, in world coordinates.
    fn plane(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = dir(self.facing).map(f64::from);
        let s = side(self.facing).map(f64::from);
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        let middle = [
            f64::from(self.at[0]) + 0.5 - f[0] * 0.5 + s[0] * (w - 1.0) / 2.0,
            f64::from(self.at[1]) + h / 2.0,
            f64::from(self.at[2]) + 0.5 - f[2] * 0.5 + s[2] * (w - 1.0) / 2.0,
        ];
        (middle, f, s)
    }
    /// The frame's box: from the wall out 1/16 (`THICKNESS`).
    fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let (m, f, s) = self.plane();
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        let a = [m[0] - s[0] * w / 2.0, m[1] - h / 2.0, m[2] - s[2] * w / 2.0];
        let b = [m[0] + s[0] * w / 2.0 + f[0] * 0.0625, m[1] + h / 2.0, m[2] + s[2] * w / 2.0 + f[2] * 0.0625];
        ([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])])
    }
    fn chunk(&self) -> [i32; 2] {
        [self.at[0] >> 4, self.at[2] >> 4]
    }
}

// --- Server ----------------------------------------------------------------------------------

fn frames_in(game: &mut Game<'_>, chunk: [i32; 2]) -> Vec<Frame> {
    let host: &mut dyn Host = game;
    FRAMES.get_existing(host, Holder::Chunk(chunk)).ok().flatten().map(|f| f.0).unwrap_or_default()
}
fn store_frames(game: &mut Game<'_>, chunk: [i32; 2], frames: Vec<Frame>) {
    let host: &mut dyn Host = game;
    let _ = if frames.is_empty() {
        FRAMES.remove(host, Holder::Chunk(chunk)).map(|_| ())
    } else {
        FRAMES.set(host, Holder::Chunk(chunk), Frames(frames))
    };
}
/// The frame facing `face` that covers the block in front of `pos` (a click on its wall).
fn frame_at(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3]) -> Option<Frame> {
    let facing = facing_of(face)?;
    let cell = add(pos, face, 1);
    let (cx, cz) = (cell[0] >> 4, cell[2] >> 4);
    for x in cx - 1..=cx + 1 {
        for z in cz - 1..=cz + 1 {
            if !game.world.chunks.contains_key(&(x, z)) {
                continue;
            }
            if let Some(f) = frames_in(game, [x, z]).into_iter().find(|f| f.facing == facing && f.covers(cell)) {
                return Some(f);
            }
        }
    }
    None
}
fn find_frame(game: &mut Game<'_>, id: u64, near: [i32; 3]) -> Option<Frame> {
    let (cx, cz) = (near[0] >> 4, near[2] >> 4);
    for x in cx - 2..=cx + 2 {
        for z in cz - 2..=cz + 2 {
            if !game.world.chunks.contains_key(&(x, z)) {
                continue;
            }
            if let Some(f) = frames_in(game, [x, z]).into_iter().find(|f| f.id == id) {
                return Some(f);
            }
        }
    }
    None
}
/// Writes `frame` (a new one, or a changed one, which may have moved chunk).
fn put_frame(game: &mut Game<'_>, old_chunk: Option<[i32; 2]>, frame: Frame) {
    if let Some(old) = old_chunk {
        let mut frames = frames_in(game, old);
        frames.retain(|f| f.id != frame.id);
        store_frames(game, old, frames);
    }
    let chunk = frame.chunk();
    let mut frames = frames_in(game, chunk);
    frames.retain(|f| f.id != frame.id);
    frames.push(frame);
    store_frames(game, chunk, frames);
}
fn remove_frame(game: &mut Game<'_>, frame: &Frame) {
    let chunk = frame.chunk();
    let mut frames = frames_in(game, chunk);
    frames.retain(|f| f.id != frame.id);
    store_frames(game, chunk, frames);
}
fn sound(game: &mut Game<'_>, frame: &Frame, sound: &str) {
    let (m, f, _) = frame.plane();
    // `SoundCategory.NEUTRAL`, as an entity's own sounds.
    game.native_play_sound_in_place(sound, [m[0] + f[0] * 0.03, m[1], m[2] + f[2] * 0.03], 6, 1.0, 1.0);
}

/// `native_use_item_on` (`PictureItem.useOnBlock`): sneak + use a picture on a wall's side
/// hangs it in a new frame.
pub(crate) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    if game.remote.is_some() || &*stack.item != cam::PICTURE || !game.native_sneaking() {
        return;
    }
    let Some(facing) = facing_of(face) else {
        return;
    };
    let Some(picture) = cam::picture_data(game, stack) else {
        return;
    };
    let at = add(pos, face, 1);
    let mut id = 0;
    while id == 0 {
        id = cam::random_uuid() as u64;
    }
    let frame = Frame {
        id,
        at,
        facing,
        width: 1,
        height: 1,
        picture,
        name: cam::custom_name(stack),
        glowing: false,
        fixed: false,
        rotation: 0,
    };
    sound(game, &frame, "minecraft:entity.item_frame.place");
    fresh_click(game.native_acting_id(), frame.id, 1000);
    put_frame(game, None, frame);
    *claimed = true;
    if !game.creative {
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
}

/// `native_right_click_block` (`PictureFrameEntity.interact`): on a frame, sneaking opens its
/// editor, else its picture turns a quarter (`can_rotate_pictures`, not when fixed). A lectern
/// with an album shows its pictures (`AlbumLecternScreen`).
pub(crate) fn right_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    if game.remote.is_some() {
        return;
    }
    if let Some(mut frame) = frame_at(game, pos, face) {
        *canceled = true;
        // `Entity.interact` takes the main hand's turn; the off hand's (which a client that
        // didn't see the main hand act still sends) does nothing more.
        if game.hand != horizon_client::first_person::Hand::Main {
            return;
        }
        let player = game.native_acting_id();
        // One click is one interaction: the click that hung a frame, or the same click
        // heard again within the use key's 4-tick repeat delay, does nothing more.
        if !fresh_click(player, frame.id, 250) {
            return;
        }
        if game.native_sneaking() {
            send_frame_screen(game, player, &frame);
        } else if cam::config().can_rotate_pictures && !frame.fixed {
            frame.rotation = (frame.rotation + 1) % 4;
            sound(game, &frame, "minecraft:entity.item_frame.rotate_item");
            put_frame(game, None, frame);
        }
        return;
    }
    lectern_use(game, pos, canceled);
}

/// `native_left_click_block` (`ResizableDecorationEntity.handleAttack`): the frame breaks and
/// drops its picture; a fixed one only breaks for a creative player.
pub(crate) fn left_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    if game.remote.is_some() {
        return;
    }
    let Some(frame) = frame_at(game, pos, face) else {
        return;
    };
    *canceled = true;
    if frame.fixed && !game.creative {
        return;
    }
    remove_frame(game, &frame);
    sound(game, &frame, "minecraft:entity.item_frame.break");
    // `PictureFrameEntity.onBreak`: the picture drops, its name kept.
    let mut stack = cam::picture_stack(game, &frame.picture);
    if let Some(name) = &frame.name {
        stack.data_mut().custom_name = Some(Tag::String(name.as_str().into()));
    }
    let (m, f, _) = frame.plane();
    game.spawn_item(stack, [m[0] + f[0] * 0.15, m[1], m[2] + f[2] * 0.15], [0.0, 0.1, 0.0], 10);
}

/// Each player's last click on a frame, and until when another click on it is the same one.
static CLICKS: Mutex<Vec<(u128, u64, std::time::Instant)>> = Mutex::new(Vec::new());
/// Whether this click on `frame` by `player` is a new one, noting it for `hold_ms`: the use
/// key's repeat (`MinecraftClient.itemUseCooldown`, 4 ticks) after a click, a whole second
/// after the click that hung the frame (the client, which didn't see it hang one, uses again).
fn fresh_click(player: u128, frame: u64, hold_ms: u64) -> bool {
    let mut clicks = CLICKS.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    clicks.retain(|c| c.2 > now);
    if clicks.iter().any(|c| c.0 == player && c.1 == frame) {
        return false;
    }
    clicks.push((player, frame, now + std::time::Duration::from_millis(hold_ms)));
    true
}

/// Whether `player`'s click went to a frame just now: its use of the item in hand, which a
/// client that doesn't know the frame is there sends after it, is the same click.
pub(crate) fn clicked_frame(player: u128) -> bool {
    let now = std::time::Instant::now();
    CLICKS.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|c| c.0 == player && c.2 > now)
}

fn send_frame_screen(game: &mut Game<'_>, player: u128, frame: &Frame) {
    let mut data = frame.id.to_be_bytes().to_vec();
    for v in frame.at {
        data.extend_from_slice(&v.to_be_bytes());
    }
    data.extend_from_slice(&[frame.width, frame.height, u8::from(frame.glowing), u8::from(frame.fixed)]);
    let _ = game.native_net_send_to_player(player, FRAME_SCREEN, &data);
}

/// What players asked for, done in their own tick (their level and state in place).
enum Request {
    Button { frame: u64, near: [i32; 3], button: u8 },
    TakeBook([i32; 3]),
}
static REQUESTS: Mutex<Vec<(u128, Request)>> = Mutex::new(Vec::new());

/// `native_net_from_client` for frames and the lectern.
pub(crate) fn from_client(_game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    let read3 = |d: &[u8]| -> Option<[i32; 3]> {
        let mut pos = [0; 3];
        for (i, p) in pos.iter_mut().enumerate() {
            *p = i32::from_be_bytes(d.get(i * 4..i * 4 + 4)?.try_into().ok()?);
        }
        Some(pos)
    };
    let request = match channel {
        FRAME_BUTTON if data.len() == 21 => {
            let frame = u64::from_be_bytes(data[..8].try_into().unwrap_or_default());
            let Some(near) = read3(&data[8..20]) else {
                return;
            };
            Request::Button { frame, near, button: data[20] }
        }
        TAKE_BOOK => {
            let Some(pos) = read3(data) else {
                return;
            };
            Request::TakeBook(pos)
        }
        _ => return,
    };
    let mut r = REQUESTS.lock().unwrap_or_else(|e| e.into_inner());
    if r.len() < 256 {
        r.push((player, request));
    }
}

/// `native_player_tick`: the player's requests.
pub(crate) fn player_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let me = game.native_acting_id();
    let mine: Vec<Request> = {
        let mut r = REQUESTS.lock().unwrap_or_else(|e| e.into_inner());
        if r.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *r).into_iter().partition(|(p, _)| *p == me);
        *r = rest;
        mine.into_iter().map(|(_, q)| q).collect()
    };
    let Some(eye) = game.eye_position() else {
        return;
    };
    for request in mine {
        match request {
            Request::Button { frame, near, button } => {
                let far = |p: [i32; 3]| {
                    let d = [f64::from(p[0]) + 0.5 - eye[0], f64::from(p[1]) + 0.5 - eye[1], f64::from(p[2]) + 0.5 - eye[2]];
                    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 24.0 * 24.0
                };
                if far(near) {
                    continue;
                }
                let Some(f) = find_frame(game, frame, near) else {
                    continue;
                };
                if far(f.at) {
                    continue;
                }
                let old = f.chunk();
                let Some(changed) = press(f, button) else {
                    continue;
                };
                send_frame_screen(game, me, &changed);
                put_frame(game, Some(old), changed);
            }
            Request::TakeBook(pos) => take_book(game, pos),
        }
    }
}

/// `PictureFrameScreenHandler.onButtonClick` and `PictureFrameEntity.resize`.
fn press(mut f: Frame, button: u8) -> Option<Frame> {
    let shrink = button % 2 == 0;
    let n: i32 = if shrink { -1 } else { 1 };
    let grow = |v: u8| -> Option<u8> {
        let v = i32::from(v) + n;
        (1..=16).contains(&v).then_some(v as u8)
    };
    match button {
        0 | 1 => f.height = grow(f.height)?,
        2 | 3 => f.width = grow(f.width)?,
        4 | 5 => {
            f.height = grow(f.height)?;
            // Growing down moves the bottom.
            f.at = add(f.at, [0, 1, 0], if shrink { 1 } else { -1 });
        }
        6 | 7 => {
            f.width = grow(f.width)?;
            f.at = add(f.at, side(f.facing), if shrink { 1 } else { -1 });
        }
        8 => f.glowing = !f.glowing,
        9 => f.fixed = !f.fixed,
        _ => return None,
    }
    Some(f)
}

// --- Albums and the lectern ------------------------------------------------------------------

/// `AlbumItem.use` when sneaking or empty: the album's pages to edit. Horizon's server menus
/// are chests: four rows hold its 36 pictures, a click moves one in or out.
pub(crate) fn open_album(game: &mut Game<'_>) {
    let player = game.native_acting_id();
    let slot = if game.inventory.get(game.selected).and_then(Option::as_ref).is_some_and(|s| &*s.item == cam::ALBUM) {
        game.selected
    } else {
        40
    };
    ALBUMS.lock().unwrap_or_else(|e| e.into_inner()).retain(|a| a.player != player);
    ALBUMS.lock().unwrap_or_else(|e| e.into_inner()).push(OpenAlbum { player, slot, window: None, opened: false });
}

/// An album being edited: its player, its inventory slot and its menu's window.
struct OpenAlbum {
    player: u128,
    slot: usize,
    window: Option<i32>,
    opened: bool,
}
static ALBUMS: Mutex<Vec<OpenAlbum>> = Mutex::new(Vec::new());
const ALBUM_SLOTS: usize = 36;

fn album_items(stack: &ItemStack) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; ALBUM_SLOTS];
    for (slot, s) in stack.container() {
        if let Some(cell) = items.get_mut(usize::from(*slot)) {
            *cell = Some(s.clone());
        }
    }
    items
}
fn set_album_items(stack: &mut ItemStack, items: &[Option<ItemStack>]) {
    let data = stack.data_mut();
    data.container = items.iter().enumerate().filter_map(|(i, s)| Some((i as u8, s.clone()?))).collect();
    stack.tidy();
}
fn album_stack(game: &Game<'_>, player: u128, slot: usize) -> Option<ItemStack> {
    let inv = game.native_inventory(player, super::native_inventories::NativeInventoryKind::Player).ok()?;
    inv.slots.get(slot)?.clone().filter(|s| &*s.item == cam::ALBUM)
}

/// `native_server_tick`: albums asked to open (outside the player's turn, as menus need).
/// `HangingEntity.tick` and `canSurvive`: every 100 ticks a frame whose wall is gone (a block
/// behind one of its cells isn't solid any more) breaks and drops its picture, as an item frame
/// does. A fixed frame stays. A wall in a chunk that isn't loaded counts as there.
fn drop_unsupported(game: &mut Game<'_>) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static TICKS: AtomicU32 = AtomicU32::new(0);
    if TICKS.fetch_add(1, Ordering::Relaxed) % 100 != 0 {
        return;
    }
    let chunks: Vec<(i32, i32)> = game.world.chunks.keys().copied().collect();
    for (cx, cz) in chunks {
        let frames = frames_in(game, [cx, cz]);
        for frame in frames {
            if frame.fixed || supported(game, &frame) {
                continue;
            }
            remove_frame(game, &frame);
            sound(game, &frame, "minecraft:entity.item_frame.break");
            let mut stack = cam::picture_stack(game, &frame.picture);
            if let Some(name) = &frame.name {
                stack.data_mut().custom_name = Some(Tag::String(name.as_str().into()));
            }
            let (m, f, _) = frame.plane();
            game.spawn_item(stack, [m[0] + f[0] * 0.15, m[1], m[2] + f[2] * 0.15], [0.0, 0.1, 0.0], 10);
        }
    }
}
/// Whether every block behind the frame is solid (`HangingEntity.canSurvive`).
fn supported(game: &Game<'_>, frame: &Frame) -> bool {
    let (f, s) = (dir(frame.facing), side(frame.facing));
    for i in 0..i32::from(frame.width) {
        for j in 0..i32::from(frame.height) {
            let wall = add(add(add(frame.at, s, i), [0, 1, 0], j), f, -1);
            if !game.world.chunks.contains_key(&(wall[0] >> 4, wall[2] >> 4)) {
                continue;
            }
            let state = game.world.block(wall[0], wall[1], wall[2]);
            if !game.res.facts().get(state).legacy_solid {
                return false;
            }
        }
    }
    true
}

pub(crate) fn server_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    drop_unsupported(game);
    let pending: Vec<(u128, usize)> = ALBUMS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|a| !a.opened)
        .map(|a| (a.player, a.slot))
        .collect();
    for (player, slot) in pending {
        let window = album_stack(game, player, slot).and_then(|album| {
            let title = album.custom_name().map(|n| Text::literal(n)).unwrap_or_else(|| Text::tr("item.camerapture.album", &[]));
            game.native_open_menu(player, 4, title, album_items(&album))
        });
        let mut albums = ALBUMS.lock().unwrap_or_else(|e| e.into_inner());
        if window.is_none() {
            albums.retain(|a| a.player != player);
        } else if let Some(a) = albums.iter_mut().find(|a| a.player == player) {
            a.window = window;
            a.opened = true;
        }
    }
}

/// `native_menu_clicked` (`AlbumScreenHandler.quickMove`): a picture in the album goes to the
/// inventory, a picture in the inventory into the album's first free slot.
pub(crate) fn menu_clicked(game: &mut Game<'_>, player: u128, window: i32, slot: i32) {
    let Some(album_slot) = ALBUMS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|a| a.player == player && a.window == Some(window))
        .map(|a| a.slot)
    else {
        return;
    };
    let Some(mut album) = album_stack(game, player, album_slot) else {
        game.native_close_menu(player);
        return;
    };
    let before = album.clone();
    let mut items = album_items(&album);
    let Ok(slot) = usize::try_from(slot) else {
        return;
    };
    use super::native_inventories::NativeInventoryKind::Player;
    if slot < ALBUM_SLOTS {
        let Some(picture) = items[slot].take() else {
            return;
        };
        set_album_items(&mut album, &items);
        if game.native_set_inventory_slot(player, Player, album_slot, Some(&before), Some(album)).is_err() {
            return;
        }
        if let Some(rest) = game.native_give_stack(player, picture) {
            // No room: it stays in the album.
            let mut back = album_stack(game, player, album_slot).unwrap_or(before.clone());
            let mut items = album_items(&back);
            items[slot] = Some(rest);
            let now = back.clone();
            set_album_items(&mut back, &items);
            let _ = game.native_set_inventory_slot(player, Player, album_slot, Some(&now), Some(back));
        }
    } else {
        let Some(inv_slot) = super::native_menus::native_menu_inventory_slot(ALBUM_SLOTS, slot as i32) else {
            return;
        };
        let Some(free) = items.iter().position(Option::is_none) else {
            return;
        };
        let Ok(inv) = game.native_inventory(player, Player) else {
            return;
        };
        let Some(stack) = inv.slots.get(inv_slot).cloned().flatten() else {
            return;
        };
        if &*stack.item != cam::PICTURE || inv_slot == album_slot {
            return;
        }
        let mut rest = stack.clone();
        rest.count -= 1;
        let mut one = stack.clone();
        one.count = 1;
        if game
            .native_set_inventory_slot(player, Player, inv_slot, Some(&stack), (rest.count > 0).then_some(rest))
            .is_err()
        {
            return;
        }
        items[free] = Some(one);
        set_album_items(&mut album, &items);
        let _ = game.native_set_inventory_slot(player, Player, album_slot, Some(&before), Some(album));
    }
    if let Some(album) = album_stack(game, player, album_slot) {
        game.native_set_menu_items(player, album_items(&album));
    }
}

/// `native_menu_closed`.
pub(crate) fn menu_closed(player: u128, window: i32) {
    ALBUMS.lock().unwrap_or_else(|e| e.into_inner()).retain(|a| !(a.player == player && a.window == Some(window)));
}

/// The album a lectern holds, read from its block entity (`LecternBlockEntity.book`).
fn lectern_album(game: &mut Game<'_>, pos: [i32; 3]) -> Option<ItemStack> {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    if &*game.res.facts().get(state).name != "minecraft:lectern" {
        return None;
    }
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(horizon_client::containers::BlockEntity::Lectern(l)) => l.book.clone().filter(|b| &*b.item == cam::ALBUM),
        _ => None,
    }
}

/// `LecternBlockEntityMixin.createMenu`: an album on a lectern shows its pictures.
fn lectern_use(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if game.native_sneaking() {
        return;
    }
    let Some(album) = lectern_album(game, pos) else {
        return;
    };
    *canceled = true;
    let player = game.native_acting_id();
    // The item in hand's use that follows is the same click (`clicked_frame`).
    fresh_click(player, 0, 250);
    let pictures = cam::album_pictures(&album);
    cam::send_view(game, player, &pictures, Some(pos));
}

/// `AlbumLecternScreenHandler.onButtonClick`: the Take Book button.
fn take_book(game: &mut Game<'_>, pos: [i32; 3]) {
    let Some(eye) = game.eye_position() else {
        return;
    };
    let d = [f64::from(pos[0]) + 0.5 - eye[0], f64::from(pos[1]) + 0.5 - eye[1], f64::from(pos[2]) + 0.5 - eye[2]];
    if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 8.0 * 8.0 || lectern_album(game, pos).is_none() {
        return;
    }
    game.take_lectern_book((pos[0], pos[1], pos[2]));
    game.guest_inventory_resend = true;
}

// --- Client ------------------------------------------------------------------------------------

struct Seen {
    frames: Vec<Frame>,
    /// The frame the crosshair is on, and the block behind it.
    aimed: Option<(u64, [i32; 3])>,
}
static SEEN: Mutex<Seen> = Mutex::new(Seen { frames: Vec::new(), aimed: None });

/// `native_client_tick_post`: the frames near the player, read from the synced chunks, and
/// the one aimed at.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let (cx, cz) = ((player.position.x.floor() as i32) >> 4, (player.position.z.floor() as i32) >> 4);
    if game.world.game_time % 4 == 0 {
        let radius = 5;
        let mut seen = Vec::new();
        for x in cx - radius..=cx + radius {
            for z in cz - radius..=cz + radius {
                if !game.world.chunks.contains_key(&(x, z)) {
                    continue;
                }
                let host: &mut dyn Host = game;
                if let Ok(Some(f)) = FRAMES.get_existing(host, Holder::Chunk([x, z])) {
                    seen.extend(f.0);
                }
            }
        }
        SEEN.lock().unwrap_or_else(|e| e.into_inner()).frames = seen;
    }
    // `PictureFrameEntityRenderer.shouldRenderOutline`: the crosshair on the frame's box.
    let aimed = game.aim().and_then(|(eye, look)| {
        let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let mut best: Option<(f64, &Frame)> = None;
        for f in &seen.frames {
            let (lo, hi) = f.bounds();
            if let Some(t) = ray_box(eye, look, lo, hi)
                && t <= game.reach()
                && best.is_none_or(|(b, _)| t < b)
            {
                best = Some((t, f));
            }
        }
        best.map(|(t, f)| {
            let hit = [eye[0] + look[0] * t, eye[1] + look[1] * t, eye[2] + look[2] * t];
            let d = dir(f.facing).map(f64::from);
            let cell = [
                (hit[0] - d[0] * 0.01).floor() as i32,
                (hit[1]).floor() as i32,
                (hit[2] - d[2] * 0.01).floor() as i32,
            ];
            (f.id, cell)
        })
    });
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).aimed = aimed;
}

/// The distance along `dir` from `from` to the box, if the ray meets it.
fn ray_box(from: [f64; 3], dir: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> Option<f64> {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if from[i] < lo[i] || from[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - from[i]) / dir[i], (hi[i] - from[i]) / dir[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// Whether the block highlight at `pos` is a frame's wall the crosshair is on.
pub(crate) fn aimed_wall(pos: [i32; 3]) -> bool {
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.aimed.is_some_and(|(id, cell)| {
        seen.frames.iter().any(|f| f.id == id && add(cell, dir(f.facing), -1) == pos)
    })
}

/// `native_world_primitives`: the outline of the frame aimed at (not with the camera up).
pub(crate) fn primitives(game: &Game<'_>, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    if client::active_camera(game) || game.ui.hide_gui {
        return;
    }
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some((id, _)) = seen.aimed else {
        return;
    };
    if let Some(f) = seen.frames.iter().find(|f| f.id == id) {
        let (lo, hi) = f.bounds();
        primitives.box_outline(lo, hi, 0x6600_0000, 2.0);
    }
}

/// `native_world_submit` (`PictureFrameEntityRenderer.render`): each frame's picture fitted
/// into it and turned, glowing ones full bright; otherwise what is happening to it.
pub(crate) fn submit(game: &Game<'_>, draw: &mut NativeWorldDraw) {
    let frames = SEEN.lock().unwrap_or_else(|e| e.into_inner()).frames.clone();
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let eye = [player.position.x, player.position.y, player.position.z];
    for f in &frames {
        let (m, d, s) = f.plane();
        // `shouldRender`: 64 blocks.
        let far = (m[0] - eye[0]).powi(2) + (m[1] - eye[1]).powi(2) + (m[2] - eye[2]).powi(2);
        if far > 64.0 * 64.0 {
            continue;
        }
        let light = if f.glowing {
            240 | 240 << 8
        } else {
            horizon_client::block_entity_render::vertex_light(game.world.light(f.at[0], f.at[1], f.at[2]))
        };
        let (status, [pw, ph]) = client::picture(f.picture.id);
        // The picture a hair off the wall (Camerapture's 0.02625 in front of its box's middle).
        let front = [m[0] + d[0] * 0.005, m[1], m[2] + d[2] * 0.005];
        draw.push();
        if !draw.translate_world(front) {
            draw.pop();
            continue;
        }
        draw.light = light;
        if status == Status::Success && pw > 0 && ph > 0 {
            let (fw, fh) = (f32::from(f.width), f32::from(f.height));
            let (iw, ih) = if f.rotation % 2 == 1 { (ph as f32, pw as f32) } else { (pw as f32, ph as f32) };
            let scale = (fw / iw).min(fh / ih);
            let (hw, hh) = (pw as f32 * scale / 2.0, ph as f32 * scale / 2.0);
            let right = [s[0] as f32, 0.0, s[2] as f32];
            let up = [0.0f32, 1.0, 0.0];
            // Corners of the image (left-top, left-bottom, right-bottom, right-top) as
            // offsets along right and up, turned a quarter clockwise per rotation.
            let mut corners = [[-hw, hh], [-hw, -hh], [hw, -hh], [hw, hh]];
            for _ in 0..f.rotation {
                corners = corners.map(|[x, y]| [y, -x]);
            }
            let points = corners.map(|[x, y]| [right[0] * x + up[0] * y, right[1] * x + up[1] * y, right[2] * x + up[2] * y]);
            game.native_world_mod_texture_quad(draw, &client::texture_key(f.picture.id), points, [0.0, 0.0, 1.0, 1.0], [255; 4]);
        } else {
            // `renderFetching` and `renderErrorText`: text facing out of the wall.
            let yaw = match f.facing {
                2 => 180.0,
                3 => 0.0,
                4 => -90.0,
                _ => 90.0,
            };
            draw.translate([d[0] as f32 * 0.01, 0.0, d[2] as f32 * 0.01]);
            draw.rotate_about([0.0, 1.0, 0.0], yaw, [0.0; 3]);
            draw.scale([0.015625, -0.015625, 0.015625]);
            let lines: Vec<(Text, u32)> = if status == Status::Fetching {
                vec![
                    (Text::tr("text.camerapture.fetching_picture", &[]), 0xFFFF_FFFF),
                    (Text::literal(client::loading_dots()), 0xFF80_8080),
                ]
            } else {
                vec![(Text::tr("text.camerapture.fetching_failed", &[]), 0xFFFF_5555)]
            };
            let top = -(lines.len() as f32) * 9.0 / 2.0;
            for (i, (t, color)) in lines.iter().enumerate() {
                let w = game.gui.atlas.font.width_text(t);
                draw.push();
                draw.translate([-w / 2.0, top + i as f32 * 9.5, 0.0]);
                game.native_world_text(draw, t, *color, None);
                draw.pop();
            }
        }
        draw.pop();
    }
}
use client::Status;

// The frame editor (`PictureFrameScreen`), drawn on the mod's own screen.
struct Editor {
    frame: u64,
    near: [i32; 3],
    width: u8,
    height: u8,
    glowing: bool,
    fixed: bool,
}
static EDITOR: Mutex<Option<Editor>> = Mutex::new(None);
static OUT: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

/// The picture screen took the mod's screen: the editor is gone.
pub(crate) fn screen_opened() {
    *EDITOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// `native_net_from_server` for frames.
pub(crate) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != FRAME_SCREEN || data.len() != 24 {
        return;
    }
    let frame = u64::from_be_bytes(data[..8].try_into().unwrap_or_default());
    let mut near = [0; 3];
    for (i, p) in near.iter_mut().enumerate() {
        *p = i32::from_be_bytes(data[8 + i * 4..12 + i * 4].try_into().unwrap_or_default());
    }
    let editor = Editor { frame, near, width: data[20], height: data[21], glowing: data[22] != 0, fixed: data[23] != 0 };
    let open = game.native_mod_screen() == Some(cam::ID);
    let editing = EDITOR.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|e| e.frame == frame);
    if open && editing {
        *EDITOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(editor);
        return;
    }
    if open || game.native_mod_screen_open(cam::ID) {
        client::open_screen_none();
        *EDITOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(editor);
    }
}

/// `native_client_tick_post`: the editor's button presses go to the server.
pub(crate) fn flush(game: &mut Game<'_>) {
    let out = std::mem::take(&mut *OUT.lock().unwrap_or_else(|e| e.into_inner()));
    for data in out {
        let _ = game.native_net_send_to_server(FRAME_BUTTON, &data);
    }
    if game.native_mod_screen() != Some(cam::ID) {
        *EDITOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

const BG_W: f32 = 158.0;
const BG_H: f32 = 52.0;
struct EditorLayout {
    up: [f32; 4],
    right: [f32; 4],
    down: [f32; 4],
    left: [f32; 4],
    glowing: [f32; 4],
    fixed: [f32; 4],
    bg: [f32; 2],
}
fn editor_layout(w: f32, h: f32) -> EditorLayout {
    let (cx, cy) = ((w / 2.0).floor(), (h / 2.0).floor());
    let (x, y) = (cx - BG_W / 2.0, cy - BG_H / 2.0);
    EditorLayout {
        up: [x, y - 20.0 - 4.0, BG_W, 20.0],
        right: [x + BG_W + 4.0, y, 20.0, BG_H],
        down: [x, y + BG_H + 4.0, BG_W, 20.0],
        left: [x - 20.0 - 4.0, y, 20.0, BG_H],
        glowing: [x + 7.0, y + 34.0, 11.0, 11.0],
        fixed: [x + BG_W - 7.0 - 11.0, y + 34.0, 11.0, 11.0],
        bg: [x, y],
    }
}

/// `PictureFrameScreen.render`. True when the editor is the screen up.
pub(crate) fn screen_draw(game: &Game<'_>, batch: &mut HudBatch, view: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    let editor = EDITOR.lock().unwrap_or_else(|e| e.into_inner());
    let Some(e) = editor.as_ref() else {
        return false;
    };
    let (w, h) = (batch.width, batch.height);
    game.native_apply_screen_blur();
    game.native_fill(batch, [0.0, 0.0, w, h], 0xC810_1010);
    let l = editor_layout(w, h);
    let shrink = game.has_shift_down();
    let (labels, active) = if shrink {
        (["\u{2193}", "\u{2190}", "\u{2191}", "\u{2192}"], [e.height > 1, e.width > 1, e.height > 1, e.width > 1])
    } else {
        (["\u{2191}", "\u{2192}", "\u{2193}", "\u{2190}"], [e.height < 16, e.width < 16, e.height < 16, e.width < 16])
    };
    for (i, r) in [l.up, l.right, l.down, l.left].into_iter().enumerate() {
        client::button(game, batch, r, &Text::literal(labels[i]), view.mouse, active[i]);
    }
    let tex = "camerapture:gui/edit_picture_frame";
    game.native_sprite(batch, tex, [0.0, 0.0, BG_W, BG_H], [l.bg[0], l.bg[1], BG_W, BG_H]);
    let size = cam::tr_d("text.camerapture.edit_picture_frame.size", &[&e.width, &e.height]);
    let sw = game.gui.atlas.font.width_text(&size);
    batch.text_styled(&game.gui.atlas, &size, l.bg[0] + BG_W / 2.0 - sw / 2.0, l.bg[1] + 7.0, u32::MAX, true);
    let hint = Text::tr("text.camerapture.edit_picture_frame.shrink_hint", &[]);
    let hw = game.gui.atlas.font.width_text(&hint);
    let gray = horizon_client::hud::rgba(0xA0, 0xA0, 0xA0, 0xFF);
    batch.text_styled(&game.gui.atlas, &hint, l.bg[0] + BG_W / 2.0 - hw / 2.0, l.bg[1] + 7.0 + 9.0 + 2.0, gray, true);
    // `SmallCheckboxWidget`: the box, its tick and its label (left of the second box).
    let light = horizon_client::hud::rgba(0xE0, 0xE0, 0xE0, 0xFF);
    for (r, checked, key, left) in [
        (l.glowing, e.glowing, "text.camerapture.edit_picture_frame.glowing", false),
        (l.fixed, e.fixed, "text.camerapture.edit_picture_frame.fixed", true),
    ] {
        let hovered = client::hit(r, view.mouse);
        game.native_sprite(batch, tex, [if hovered { 11.0 } else { 0.0 }, 52.0, 11.0, 11.0], r);
        if checked {
            game.native_sprite(batch, tex, [22.0, 52.0, 11.0, 11.0], r);
        }
        let t = Text::tr(key, &[]);
        let tw = game.gui.atlas.font.width_text(&t);
        let x = if left { r[0] - 4.0 - tw } else { r[0] + 15.0 };
        batch.text_styled(&game.gui.atlas, &t, x, r[1] + 2.0, light, true);
    }
    true
}

/// A click on the editor (`PictureFrameScreen`'s buttons). True when the editor is up.
pub(crate) fn screen_press(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, button: i32) -> bool {
    let mut editor = EDITOR.lock().unwrap_or_else(|e| e.into_inner());
    let Some(e) = editor.as_mut() else {
        return false;
    };
    if button != 0 {
        return true;
    }
    let (w, h) = game.gui_size();
    let l = editor_layout(w, h);
    let shrink = game.has_shift_down();
    let s = u8::from(!shrink);
    let pressed = if client::hit(l.up, view.mouse) && (if shrink { e.height > 1 } else { e.height < 16 }) {
        Some(s)
    } else if client::hit(l.right, view.mouse) && (if shrink { e.width > 1 } else { e.width < 16 }) {
        Some(2 + s)
    } else if client::hit(l.down, view.mouse) && (if shrink { e.height > 1 } else { e.height < 16 }) {
        Some(4 + s)
    } else if client::hit(l.left, view.mouse) && (if shrink { e.width > 1 } else { e.width < 16 }) {
        Some(6 + s)
    } else if client::hit(l.glowing, view.mouse) {
        Some(8)
    } else if client::hit(l.fixed, view.mouse) {
        Some(9)
    } else {
        None
    };
    if let Some(b) = pressed {
        match b {
            0..=1 | 4..=5 => e.height = if shrink { e.height - 1 } else { e.height + 1 },
            2..=3 | 6..=7 => e.width = if shrink { e.width - 1 } else { e.width + 1 },
            8 => e.glowing = !e.glowing,
            _ => e.fixed = !e.fixed,
        }
        let mut data = e.frame.to_be_bytes().to_vec();
        for v in e.near {
            data.extend_from_slice(&v.to_be_bytes());
        }
        data.push(b);
        OUT.lock().unwrap_or_else(|e| e.into_inner()).push(data);
        client::click();
    }
    true
}

/// Keys over the editor: Shift only changes what the buttons do.
pub(crate) fn screen_key(_game: &Game<'_>, _key: &str) -> bool {
    EDITOR.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}
