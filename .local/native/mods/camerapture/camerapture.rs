// Camerapture (chrrs, MIT) for Horizon: the shared half and the server's half.
//  - A camera (`camerapture:camera`) is turned on with a right click; aimed through its
//    viewfinder (scroll zooms), a left click takes a picture for a sheet of paper.
//  - The client grabs the world, encodes it (WebP) and uploads it in parts once the server
//    reserved an id for it; the server keeps it in `<world>/camerapture/<uuid>.webp` and hands
//    the player a `camerapture:picture` naming it (`camerapture:picture_data`).
//  - Clients download the pictures they meet in parts and draw them in hand, in slots, on
//    picture frames, in albums and full screen (`camerapture_client`, `camerapture_frames`).
//  - The server's options are `config/camerapture.server.json` (Camerapture's own file and
//    keys), synced to clients as they join.
// Mod messages stand in for Camerapture's packets, one channel each, bytes as below.
use super::Game;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::Tag;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(crate) const ID: &str = "camerapture";
pub(crate) const CAMERA: &str = "camerapture:camera";
pub(crate) const PICTURE: &str = "camerapture:picture";
pub(crate) const ALBUM: &str = "camerapture:album";
/// `Camerapture.PICTURE_DATA`: `{id: [I; 4 ints], creator: "name", timestamp: millis}`.
pub(crate) const PICTURE_DATA: &str = "camerapture:picture_data";
/// `Camerapture.CAMERA_ACTIVE`: the camera is up to the eye.
pub(crate) const CAMERA_ACTIVE: &str = "camerapture:camera_active";
pub(crate) const SHUTTER: &str = "camerapture:camera_shutter";
/// The most bytes of a picture one message carries: a mod message holds 30 000 bytes, the
/// id and the count of bytes left take 20 of them (`Camerapture.CLIENT_SECTION_SIZE` is
/// 30 000; its server sends a megabyte at a time, which a mod message can't).
pub(crate) const SECTION: usize = 29_900;

// Client to server.
pub(crate) const NEW_PICTURE: &str = "camerapture:new_picture";
pub(crate) const UPLOAD_PARTIAL: &str = "camerapture:upload_partial";
pub(crate) const REQUEST_DOWNLOAD: &str = "camerapture:request_download";
// Server to client.
pub(crate) const REQUEST_UPLOAD: &str = "camerapture:request_upload";
pub(crate) const DOWNLOAD_PARTIAL: &str = "camerapture:download_partial";
pub(crate) const PICTURE_ERROR: &str = "camerapture:picture_error";
pub(crate) const SYNC_CONFIG: &str = "camerapture:sync_config";
/// Opens the picture screen on pictures (the use of a picture or an album, which
/// Camerapture's client opens on its own `UseItemCallback`): one picture each.
pub(crate) const VIEW: &str = "camerapture:view";

/// A picture's data component (`PictureItem.PictureData`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PictureData {
    pub id: u128,
    pub creator: String,
    pub timestamp: i64,
}

pub(crate) fn uuid_ints(id: u128) -> [i32; 4] {
    [(id >> 96) as i32, (id >> 64) as i32, (id >> 32) as i32, id as i32]
}
pub(crate) fn uuid_from_ints(i: [i32; 4]) -> u128 {
    (i[0] as u32 as u128) << 96 | (i[1] as u32 as u128) << 64 | (i[2] as u32 as u128) << 32 | i[3] as u32 as u128
}
/// `UUID.toString`.
pub(crate) fn uuid_string(id: u128) -> String {
    let h = format!("{id:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}
/// `UUID.randomUUID`: version 4, from the process's random hasher keys and the clock.
pub(crate) fn random_uuid() -> u128 {
    use std::hash::{BuildHasher, Hasher};
    let state = std::collections::hash_map::RandomState::new();
    let mut a = state.build_hasher();
    a.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let mut b = state.build_hasher();
    b.write_u64(a.finish() ^ 0x9e37_79b9_7f4a_7c15);
    let id = (a.finish() as u128) << 64 | b.finish() as u128;
    (id & !(0xF000u128 << 64) | 0x4000u128 << 64) & !(0xC000u128 << 48) | 0x8000u128 << 48
}

impl PictureData {
    pub(crate) fn to_tag(&self) -> Tag {
        Tag::compound([
            ("id", Tag::IntArray(uuid_ints(self.id).to_vec())),
            ("creator", Tag::String(self.creator.as_str().into())),
            ("timestamp", Tag::Long(self.timestamp)),
        ])
    }
    pub(crate) fn from_tag(tag: &Tag) -> Option<Self> {
        let Some(Tag::IntArray(id)) = tag.get("id") else {
            return None;
        };
        let id: [i32; 4] = id.as_slice().try_into().ok()?;
        let creator = match tag.get("creator") {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => String::new(),
        };
        let timestamp = match tag.get("timestamp") {
            Some(Tag::Long(t)) => *t,
            Some(Tag::Int(t)) => i64::from(*t),
            _ => 0,
        };
        Some(Self { id: uuid_from_ints(id), creator, timestamp })
    }
    /// The component as an item model reads it (`tag_json`).
    pub(crate) fn from_json(v: &serde_json::Value) -> Option<Self> {
        let id = v.get("id")?.as_array()?;
        let ints: Vec<i32> = id.iter().filter_map(|i| i.as_i64()).map(|i| i as i32).collect();
        let id: [i32; 4] = ints.as_slice().try_into().ok()?;
        Some(Self {
            id: uuid_from_ints(id),
            creator: v.get("creator").and_then(|c| c.as_str()).unwrap_or_default().to_owned(),
            timestamp: v.get("timestamp").and_then(|t| t.as_i64()).unwrap_or(0),
        })
    }
}

/// A translation whose `%d` placeholders take `args` in order (Camerapture's lang files use
/// `%d`, which a translatable component only formats as `%s`).
pub(crate) fn tr_d(key: &str, args: &[&dyn std::fmt::Display]) -> Text {
    let mut s = Text::tr(key, &[]).plain();
    for a in args {
        s = s.replacen("%d", &a.to_string(), 1);
    }
    Text::literal(s)
}

/// `PictureItem.getPictureData`.
pub(crate) fn picture_data(game: &Game<'_>, stack: &ItemStack) -> Option<PictureData> {
    if &*stack.item != PICTURE {
        return None;
    }
    game.native_component(stack, PICTURE_DATA).ok().flatten().and_then(PictureData::from_tag)
}
/// `PictureItem.create`.
pub(crate) fn picture_stack(game: &Game<'_>, data: &PictureData) -> ItemStack {
    let mut stack = ItemStack { item: PICTURE.into(), count: 1, damage: 0, extra: None };
    let _ = game.native_set_component(&mut stack, PICTURE_DATA, data.to_tag());
    stack
}
/// `CameraItem.isActive`.
pub(crate) fn camera_active(game: &Game<'_>, stack: &ItemStack) -> bool {
    &*stack.item == CAMERA
        && matches!(game.native_component(stack, CAMERA_ACTIVE), Ok(Some(Tag::Byte(b))) if *b != 0)
}
/// `CameraItem.setActive`.
pub(crate) fn set_camera_active(game: &Game<'_>, stack: &mut ItemStack, active: bool) {
    let _ = game.native_set_component(stack, CAMERA_ACTIVE, Tag::Byte(i8::from(active)));
}
/// The custom name a stack carries (`DataComponentTypes.CUSTOM_NAME`), plain.
pub(crate) fn custom_name(stack: &ItemStack) -> Option<String> {
    stack.custom_name()
}
/// `AlbumItem.getPictures`: the pictures in an album's container, in slot order.
pub(crate) fn album_pictures(stack: &ItemStack) -> Vec<ItemStack> {
    let mut slots: Vec<(u8, ItemStack)> =
        stack.container().iter().filter(|(_, s)| &*s.item == PICTURE).cloned().collect();
    slots.sort_by_key(|(slot, _)| *slot);
    slots.into_iter().map(|(_, s)| s).collect()
}

// --- Wire format -------------------------------------------------------------------------

pub(crate) fn read_uuid(data: &[u8]) -> Option<(u128, &[u8])> {
    let (id, rest) = data.split_first_chunk::<16>()?;
    Some((u128::from_be_bytes(*id), rest))
}
pub(crate) fn read_u32(data: &[u8]) -> Option<(u32, &[u8])> {
    let (n, rest) = data.split_first_chunk::<4>()?;
    Some((u32::from_be_bytes(*n), rest))
}
/// One section of a picture: its id, the bytes after this section, the section.
pub(crate) fn section_message(id: u128, left: usize, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(20 + bytes.len());
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&(left as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}
pub(crate) fn write_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    let n = b.len().min(u16::MAX as usize);
    out.extend_from_slice(&(n as u16).to_be_bytes());
    out.extend_from_slice(&b[..n]);
}
pub(crate) fn read_str(data: &[u8]) -> Option<(String, &[u8])> {
    let (n, rest) = data.split_first_chunk::<2>()?;
    let n = u16::from_be_bytes(*n) as usize;
    (rest.len() >= n).then(|| (String::from_utf8_lossy(&rest[..n]).into_owned(), &rest[n..]))
}

/// `ByteCollector`: the sections of one picture, in order.
pub(crate) struct Collector {
    bytes: Vec<u8>,
    total: usize,
}
impl Collector {
    /// `ByteCollector.push`: false for a section that doesn't follow the last.
    pub(crate) fn push(&mut self, bytes: &[u8], left: usize) -> bool {
        if self.total == 0 && self.bytes.is_empty() {
            self.total = bytes.len() + left;
        }
        if self.bytes.len() + bytes.len() + left != self.total {
            return false;
        }
        self.bytes.extend_from_slice(bytes);
        true
    }
    pub(crate) fn new() -> Self {
        Self { bytes: Vec::new(), total: 0 }
    }
    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }
    pub(crate) fn done(&self) -> bool {
        self.bytes.len() == self.total
    }
    pub(crate) fn take(self) -> Vec<u8> {
        self.bytes
    }
}

// --- Options -------------------------------------------------------------------------------

/// `Config.Server` (`camerapture.server.json`, version 5).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ServerConfig {
    pub max_image_bytes: u32,
    pub max_image_resolution: u32,
    pub ms_per_picture: u32,
    pub can_rotate_pictures: bool,
    pub check_frame_position: bool,
    pub take_picture: u8,
    pub upload: u8,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_image_bytes: 500_000,
            max_image_resolution: 1920,
            ms_per_picture: 20,
            can_rotate_pictures: true,
            check_frame_position: false,
            take_picture: 0,
            upload: 0,
        }
    }
}
impl ServerConfig {
    fn path() -> Option<PathBuf> {
        Game::native_config_dir().ok().map(|d| d.join("camerapture.server.json"))
    }
    /// `ConfigManager.load`: the file's values over the defaults, written back (with any key
    /// it lacked) as Camerapture saves it.
    fn load() -> Self {
        let mut c = Self::default();
        let Some(path) = Self::path() else {
            return c;
        };
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        {
            let int = |k: &str, d: u32| v.get(k).and_then(|x| x.as_u64()).map_or(d, |x| x as u32);
            let bool_ = |k: &str, d: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(d);
            c.max_image_bytes = int("max_image_bytes", c.max_image_bytes);
            c.max_image_resolution = int("max_image_resolution", c.max_image_resolution).max(16);
            c.ms_per_picture = int("ms_per_picture", c.ms_per_picture);
            c.can_rotate_pictures = bool_("can_rotate_pictures", c.can_rotate_pictures);
            c.check_frame_position = bool_("check_frame_position", c.check_frame_position);
            let levels = v.get("permission_levels");
            let level = |k: &str| levels.and_then(|l| l.get(k)).and_then(|x| x.as_u64()).map(|x| x.min(4) as u8);
            c.take_picture = level("take_picture").unwrap_or(0);
            // Version 4's `allow_uploading` (`Config.Server.upgrade`).
            c.upload = level("upload").unwrap_or(if bool_("allow_uploading", true) { 0 } else { 4 });
        }
        let json = serde_json::json!({
            "version": 5,
            "max_image_bytes": c.max_image_bytes,
            "max_image_resolution": c.max_image_resolution,
            "ms_per_picture": c.ms_per_picture,
            "can_rotate_pictures": c.can_rotate_pictures,
            "check_frame_position": c.check_frame_position,
            "permission_levels": { "take_picture": c.take_picture, "upload": c.upload },
        });
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap_or_default());
        c
    }
    /// `SyncConfigPacket`: the permission levels, the most bytes and the largest side.
    pub(crate) fn synced(&self) -> Vec<u8> {
        let mut out = vec![self.take_picture, self.upload];
        out.extend_from_slice(&self.max_image_bytes.to_be_bytes());
        out.extend_from_slice(&self.max_image_resolution.to_be_bytes());
        out
    }
    pub(crate) fn from_synced(data: &[u8]) -> Option<Self> {
        let [take_picture, upload, rest @ ..] = data else {
            return None;
        };
        let (max_image_bytes, rest) = read_u32(rest)?;
        let (max_image_resolution, _) = read_u32(rest)?;
        Some(Self { take_picture: *take_picture, upload: *upload, max_image_bytes, max_image_resolution, ..Self::default() })
    }
}

// --- The server ----------------------------------------------------------------------------

/// A picture on its way to a player (`DownloadQueue.QueuedPicture`).
struct Download {
    player: u128,
    id: u128,
    bytes: Arc<[u8]>,
    sent: usize,
}

#[derive(Default)]
struct Server {
    config: Option<ServerConfig>,
    /// `ServerPictureStore.reservedIds`, with the player each was reserved for.
    reserved: HashMap<u128, u128>,
    collectors: HashMap<u128, Collector>,
    /// `ServerPictureStore.pictureCache`: the last 250 pictures read or written.
    cache: VecDeque<(u128, Arc<[u8]>)>,
    downloads: VecDeque<Download>,
    /// Players whose `NewPicturePacket` waits for their own tick (their state in place).
    takes: Vec<u128>,
    /// Pictures saved, waiting to be handed to their players in their tick.
    gives: Vec<(u128, PictureData)>,
    /// Players who asked for each picture, so one asked twice goes once.
    asked: HashSet<(u128, u128)>,
}
static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn server<R>(f: impl FnOnce(&mut Server) -> R) -> R {
    let mut s = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(Server::default))
}
pub(crate) fn config() -> ServerConfig {
    server(|s| *s.config.get_or_insert_with(ServerConfig::load))
}

fn picture_path(game: &Game<'_>, id: u128) -> Option<PathBuf> {
    Some(game.native_world_dir()?.join("camerapture").join(format!("{}.webp", uuid_string(id))))
}
/// `ServerPictureStore.get`.
fn stored_picture(game: &Game<'_>, id: u128) -> Option<Arc<[u8]>> {
    if let Some(hit) = server(|s| s.cache.iter().find(|(i, _)| *i == id).map(|(_, b)| b.clone())) {
        return Some(hit);
    }
    let bytes: Arc<[u8]> = std::fs::read(picture_path(game, id)?).ok()?.into();
    remember(id, bytes.clone());
    Some(bytes)
}
fn remember(id: u128, bytes: Arc<[u8]>) {
    server(|s| {
        s.cache.retain(|(i, _)| *i != id);
        s.cache.push_back((id, bytes));
        while s.cache.len() > 250 {
            s.cache.pop_front();
        }
    });
}

fn player_name(game: &Game<'_>, player: u128) -> String {
    game.native_server_players().into_iter().find(|p| p.uuid == player).map(|p| p.name).unwrap_or_default()
}
fn player_level(game: &Game<'_>, player: u128) -> u8 {
    game.native_player_permission(player).unwrap_or(0)
}

/// `native_component_types`.
pub(crate) fn component_types(types: &mut Vec<String>) {
    for id in [PICTURE_DATA, CAMERA_ACTIVE] {
        if !types.iter().any(|t| t == id) {
            types.push(id.to_owned());
        }
    }
}

/// `native_net_from_client`: Camerapture's serverbound packets (`registerPacketHandlers`).
pub(crate) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if !channel.starts_with("camerapture:") || game.remote.is_some() {
        return;
    }
    match channel {
        NEW_PICTURE => server(|s| {
            if !s.takes.contains(&player) {
                s.takes.push(player);
            }
        }),
        UPLOAD_PARTIAL => upload_partial(game, player, data),
        REQUEST_DOWNLOAD => {
            let Some((id, _)) = read_uuid(data) else {
                return;
            };
            if !server(|s| s.asked.insert((player, id))) {
                return;
            }
            match stored_picture(game, id) {
                Some(bytes) => server(|s| s.downloads.push_back(Download { player, id, bytes, sent: 0 })),
                None => {
                    log::warn!("Camerapture: {} asked for an unknown picture {}", player_name(game, player), uuid_string(id));
                    server(|s| s.asked.remove(&(player, id)));
                    let _ = game.native_net_send_to_player(player, PICTURE_ERROR, &id.to_be_bytes());
                }
            }
        }
        _ => super::camerapture_frames::from_client(game, player, channel, data),
    }
}

fn upload_partial(game: &mut Game<'_>, player: u128, data: &[u8]) {
    let Some((id, rest)) = read_uuid(data) else {
        return;
    };
    let Some((left, bytes)) = read_u32(rest) else {
        return;
    };
    let max = config().max_image_bytes as usize;
    let name = player_name(game, player);
    let done = server(|s| {
        if s.reserved.get(&id) != Some(&player) {
            log::error!("Camerapture: {name} tried to send a byte section for an unreserved UUID");
            return None;
        }
        let collector = s.collectors.entry(id).or_insert_with(Collector::new);
        let ok = collector.push(bytes, left as usize);
        if !ok || collector.len() + left as usize > max {
            log::error!("Camerapture: {name} sent a malformed or oversized picture");
            s.collectors.remove(&id);
            s.reserved.remove(&id);
            return None;
        }
        if !s.collectors.get(&id).is_some_and(Collector::done) {
            return None;
        }
        s.reserved.remove(&id);
        s.collectors.remove(&id).map(Collector::take)
    });
    let Some(bytes) = done else {
        return;
    };
    // `ServerPictureStore.put`, then the picture to its taker.
    let saved = picture_path(game, id).ok_or_else(|| "no world".to_owned()).and_then(|path| {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())
    });
    match saved {
        Ok(()) => {
            remember(id, bytes.into());
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64);
            server(|s| s.gives.push((player, PictureData { id, creator: name, timestamp })));
        }
        Err(e) => {
            log::error!("Camerapture: failed to save picture from {name}: {e}");
            game.native_tell(player, Text::tr("text.camerapture.picture_failed", &[]).with_color(0xFF5555));
        }
    }
}

/// `native_player_tick`: what waits for this player's own state (`NewPicturePacket`'s
/// handler, and the picture they took).
pub(crate) fn player_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let (take, gives) = {
        let me = game.native_acting_id();
        server(|s| {
            let take = s.takes.iter().position(|p| *p == me).map(|i| s.takes.remove(i)).is_some();
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.gives).into_iter().partition(|g| g.0 == me);
            s.gives = rest;
            (take.then_some(me), mine)
        })
    };
    if let Some(player) = take {
        new_picture(game, player);
    }
    for (_, data) in gives {
        let stack = picture_stack(game, &data);
        game.give_or_drop(stack);
        game.guest_inventory_resend = true;
    }
}

/// The slot of the camera in the acting player's hands (`CameraItem.find`).
fn held_camera(game: &Game<'_>) -> Option<bool> {
    if game.inventory.get(game.selected).and_then(Option::as_ref).is_some_and(|s| &*s.item == CAMERA) {
        return Some(true);
    }
    game.offhand.as_ref().is_some_and(|s| &*s.item == CAMERA).then_some(false)
}

/// `NewPicturePacket`'s handler: a sheet of paper (none in creative), the shutter, the camera
/// down and cooling, and an id the client may upload the picture to.
fn new_picture(game: &mut Game<'_>, player: u128) {
    let Some(main) = held_camera(game) else {
        return;
    };
    if !game.creative && game.native_take_items("minecraft:paper", 1) != 1 {
        return;
    }
    let mut stack = if main { game.inventory[game.selected].take() } else { game.offhand.take() };
    if let Some(s) = stack.as_mut() {
        if camera_active(game, s)
            && let Some(p) = game.native_acting_player()
        {
            let at = [p.position[0], p.position[1] + 1.62, p.position[2]];
            // `SoundCategory.PLAYERS`.
            game.native_play_sound_in_place(SHUTTER, at, 7, 1.0, 1.0);
        }
        set_camera_active(game, s, false);
    }
    if main {
        game.inventory[game.selected] = stack;
    } else {
        game.offhand = stack;
    }
    game.guest_inventory_resend = true;
    game.add_cooldown(CAMERA.into(), 60);
    let id = random_uuid();
    server(|s| {
        s.reserved.insert(id, player);
    });
    let _ = game.native_net_send_to_player(player, REQUEST_UPLOAD, &id.to_be_bytes());
}

/// `native_use_item` (`CameraItem.use`, `PictureItem.use`, `AlbumItem.use`).
pub(crate) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if game.remote.is_some() {
        return;
    }
    if matches!(&*stack.item, CAMERA | PICTURE | ALBUM)
        && super::camerapture_frames::clicked_frame(game.native_acting_id())
    {
        // The click turned or opened a frame (`Entity.interact` took it).
        *claimed = true;
        return;
    }
    let sneaking = game.native_sneaking();
    match &*stack.item {
        CAMERA => {
            let player = game.native_acting_id();
            let level = player_level(game, player);
            let c = config();
            let active = camera_active(game, stack);
            if active || (!sneaking && level >= c.take_picture) {
                if let Some(mut s) = game.held_mut().clone() {
                    set_camera_active(game, &mut s, !active);
                    *game.held_mut() = Some(s);
                    game.guest_inventory_resend = true;
                }
                *claimed = true;
            } else if sneaking && level < c.upload {
                game.native_action_bar(player, Text::tr("text.camerapture.uploading_disabled", &[]).with_color(0xFF5555));
                *claimed = true;
            }
        }
        PICTURE if !sneaking => {
            if picture_data(game, stack).is_some() {
                let player = game.native_acting_id();
                send_view(game, player, std::slice::from_ref(stack), None);
                *claimed = true;
            }
        }
        ALBUM => {
            let pictures = album_pictures(stack);
            if sneaking || pictures.is_empty() {
                super::camerapture_frames::open_album(game);
            } else {
                let player = game.native_acting_id();
                send_view(game, player, &pictures, None);
            }
            *claimed = true;
        }
        _ => {}
    }
}

/// `VIEW`: the pictures' data and names, and the lectern they lie on (its album's view).
pub(crate) fn send_view(game: &mut Game<'_>, player: u128, stacks: &[ItemStack], lectern: Option<[i32; 3]>) {
    let mut out = Vec::new();
    match lectern {
        Some(pos) => {
            out.push(1);
            for v in pos {
                out.extend_from_slice(&v.to_be_bytes());
            }
        }
        None => out.push(0),
    }
    for stack in stacks {
        let Some(data) = picture_data(game, stack) else {
            continue;
        };
        if out.len() > 29_000 {
            break;
        }
        out.extend_from_slice(&data.id.to_be_bytes());
        write_str(&mut out, &custom_name(stack).unwrap_or_default());
    }
    let _ = game.native_net_send_to_player(player, VIEW, &out);
}

/// `CameraItem.inventoryTick`: a camera not in the selected slot goes down.
pub(crate) fn inventory_tick(game: &mut Game<'_>, slot: i32, selected: bool) {
    if selected || game.remote.is_some() {
        return;
    }
    let active = match game.native_carried_stack(slot) {
        Some(s) if &*s.item == CAMERA => s.clone(),
        _ => return,
    };
    if !camera_active(game, &active) {
        return;
    }
    let mut s = active;
    set_camera_active(game, &mut s, false);
    if let Some(cell) = game.native_carried_stack(slot) {
        *cell = s;
    }
}

/// `native_player_logged_in`: `SyncConfigPacket`.
pub(crate) fn logged_in(game: &mut Game<'_>, player: u128) {
    if game.remote.is_some() {
        return;
    }
    let synced = config().synced();
    let _ = game.native_net_send_to_player(player, SYNC_CONFIG, &synced);
}

/// `native_server_tick` (its end): `DownloadQueue`, a few sections a tick to each player.
pub(crate) fn server_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let mut out: Vec<(u128, Vec<u8>)> = Vec::new();
    server(|s| {
        let mut budget: HashMap<u128, usize> = HashMap::new();
        let mut keep = VecDeque::new();
        while let Some(mut d) = s.downloads.pop_front() {
            let left = budget.entry(d.player).or_insert(20);
            while *left > 0 && d.sent < d.bytes.len() {
                let end = (d.sent + SECTION).min(d.bytes.len());
                out.push((d.player, section_message(d.id, d.bytes.len() - end, &d.bytes[d.sent..end])));
                d.sent = end;
                *left -= 1;
            }
            if d.sent < d.bytes.len() {
                keep.push_back(d);
            } else {
                s.asked.remove(&(d.player, d.id));
            }
        }
        s.downloads = keep;
    });
    for (player, message) in out {
        if game.native_net_send_to_player(player, DOWNLOAD_PARTIAL, &message).is_err() {
            server(|s| s.downloads.retain(|d| d.player != player));
        }
    }
}

/// `native_player_logged_out`: what waited for them goes.
pub(crate) fn logged_out(player: u128) {
    server(|s| {
        s.downloads.retain(|d| d.player != player);
        s.takes.retain(|p| *p != player);
        s.asked.retain(|(p, _)| *p != player);
        let ids: Vec<u128> = s.reserved.iter().filter(|(_, p)| **p == player).map(|(i, _)| *i).collect();
        for id in ids {
            s.reserved.remove(&id);
            s.collectors.remove(&id);
        }
    });
}

/// `native_tooltip` (`PictureItem.appendTooltip`): who took it and when.
pub(crate) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let Some(data) = picture_data(game, stack) else {
        return;
    };
    let gray = 0xAAAAAA;
    let white = 0xFFFFFF;
    lines.push((
        Text::translate("item.camerapture.picture.creator_tooltip", &[Text::literal(data.creator.clone()).with_color(white)]).with_color(gray),
        u32::MAX,
    ));
    // `SimpleDateFormat("MMM d, yyyy 'at' HH:mm")`, in local time.
    let when = chrono::DateTime::from_timestamp_millis(data.timestamp)
        .map(|t| t.with_timezone(&chrono::Local).format("%b %-d, %Y at %H:%M").to_string())
        .unwrap_or_default();
    lines.push((
        Text::translate("item.camerapture.picture.timestamp_tooltip", &[Text::literal(when).with_color(white)]).with_color(gray),
        u32::MAX,
    ));
}
