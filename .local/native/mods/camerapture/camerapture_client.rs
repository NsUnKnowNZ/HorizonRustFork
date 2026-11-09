// Camerapture (chrrs, MIT) for Horizon: the client's half.
//  - `ClientPictureStore`: pictures fetched from the server (or taken here), decoded off the
//    game thread and registered as mod textures (`native_mod_texture_set`), which items, frames
//    and screens draw.
//  - `PictureTaker`: the camera's zoom, the grab of the world (`native_grab_world`), the WebP
//    and its upload in parts once the server reserved an id.
//  - `CameraViewFinder` over a HUD hidden while the camera is up, the hand hidden too.
//  - `PictureItemRenderer` as a special item model (`camerapture:picture`), and
//    `PictureScreen` (full screen, an album's pages, a lectern's album) as the mod's own screen.
use super::camerapture::{self as cam, PictureData, ServerConfig};
use super::Game;
use horizon_client::{
    hud::{HudBatch, rgba},
    items::ItemStack,
    text::Text,
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

/// `CameraptureClient.MIN_ZOOM` and `MAX_ZOOM`.
const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 6.0;
/// The side of the copy kept for the full-screen view (the HUD atlas's mod image slot).
const VIEW_SIZE: u32 = 1024;
/// Messages sent a tick, under the 32 a mod message allows.
const SENDS_PER_TICK: usize = 24;

/// `Config.Client` (`camerapture.client.json`, version 3).
#[derive(Clone, Copy, Debug)]
struct ClientConfig {
    save_screenshot: bool,
    simple_camera_hud: bool,
    zoom_mouse_sensitivity: f32,
}
impl ClientConfig {
    fn load() -> Self {
        let mut c = Self { save_screenshot: false, simple_camera_hud: false, zoom_mouse_sensitivity: 0.5 };
        let Ok(dir) = Game::native_config_dir() else {
            return c;
        };
        let path = dir.join("camerapture.client.json");
        let mut cache = false;
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        {
            let b = |k: &str| v.get(k).and_then(|x| x.as_bool());
            c.save_screenshot = b("save_screenshot").unwrap_or(false);
            c.simple_camera_hud = b("simple_camera_hud").unwrap_or(false);
            cache = b("cache_pictures").unwrap_or(false);
            c.zoom_mouse_sensitivity = v
                .get("zoom_mouse_sensitivity")
                .and_then(|x| x.as_f64())
                .map_or(0.5, |x| (x as f32).clamp(0.0, 1.0));
        }
        let json = serde_json::json!({
            "version": 3,
            "cache_pictures": cache,
            "save_screenshot": c.save_screenshot,
            "simple_camera_hud": c.simple_camera_hud,
            "zoom_mouse_sensitivity": c.zoom_mouse_sensitivity,
        });
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap_or_default());
        c
    }
}

/// `RemotePicture.Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Fetching,
    Success,
    Error,
}

/// RGBA pixels.
pub(crate) struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// `RemotePicture`, with what the client keeps of it.
struct Remote {
    status: Status,
    size: [u32; 2],
    /// Up to [`VIEW_SIZE`] on a side, for the full-screen view.
    view: Option<Arc<Image>>,
    /// The picture's own file, for Ctrl-S.
    bytes: Option<Arc<[u8]>>,
}

/// A picture the screen shows: its id and the name its stack carries.
#[derive(Clone, Debug)]
pub(crate) struct Shown {
    pub id: u128,
    pub name: Option<String>,
}

/// `PictureScreen` and `AlbumLecternScreen`.
pub(crate) struct PictureScreen {
    pub pictures: Vec<Shown>,
    pub index: usize,
    /// The lectern whose album this is (its Take Book button).
    pub lectern: Option<[i32; 3]>,
    /// The picture uploaded into the mod image slot, and its size.
    shown_full: Option<(u128, [u32; 2])>,
}

enum Action {
    Click,
    TakeBook([i32; 3]),
    Save(u128),
}

struct Store {
    config: ClientConfig,
    synced: ServerConfig,
    pictures: HashMap<u128, Remote>,
    /// Ids to ask the server for.
    requests: Vec<u128>,
    collectors: HashMap<u128, cam::Collector>,
    /// Pictures decoded by the workers: the texture's copy, the view's, the file.
    decoded: Vec<(u128, Option<(Image, Image)>, Option<Arc<[u8]>>)>,
    outgoing: VecDeque<(&'static str, Vec<u8>)>,
    zoom: f32,
    /// The grab of the world waiting for its frame, and when it was asked for.
    capture: Option<(PathBuf, Instant)>,
    encoding: bool,
    /// The picture taken, encoded: its file and its two copies.
    encoded: Option<Result<(Vec<u8>, Image, Image), String>>,
    upload_id: Option<u128>,
    screen: Option<PictureScreen>,
    actions: Vec<Action>,
}
static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn store<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(|| Store {
        config: ClientConfig::load(),
        synced: ServerConfig::default(),
        pictures: HashMap::new(),
        requests: Vec::new(),
        collectors: HashMap::new(),
        decoded: Vec::new(),
        outgoing: VecDeque::new(),
        zoom: MIN_ZOOM,
        capture: None,
        encoding: false,
        encoded: None,
        upload_id: None,
        screen: None,
        actions: Vec::new(),
    }))
}

pub(crate) fn texture_key(id: u128) -> String {
    format!("camerapture:pictures/{}", cam::uuid_string(id))
}

/// `ClientPictureStore.getServerPicture`: its status and size, asking the server for it the
/// first time (and again after an error, `ensureRemotePicture`).
pub(crate) fn picture(id: u128) -> (Status, [u32; 2]) {
    store(|s| {
        let p = s.pictures.entry(id).or_insert_with(|| {
            s.requests.push(id);
            Remote { status: Status::Fetching, size: [0, 0], view: None, bytes: None }
        });
        (p.status, p.size)
    })
}
/// `ensureRemotePicture` for the screens: an errored picture is fetched again.
fn ensure_picture(id: u128) {
    store(|s| {
        if s.pictures.get(&id).is_none_or(|p| p.status == Status::Error) {
            s.pictures.insert(id, Remote { status: Status::Fetching, size: [0, 0], view: None, bytes: None });
            s.requests.push(id);
        }
    });
}

/// An area average of `image` scaled to fit `side` (never up).
fn fit(image: &Image, side: u32) -> Image {
    let scale = (f64::from(side) / f64::from(image.width.max(image.height))).min(1.0);
    let w = ((f64::from(image.width) * scale).round() as u32).max(1);
    let h = ((f64::from(image.height) * scale).round() as u32).max(1);
    scaled(image, w, h)
}
fn scaled(image: &Image, w: u32, h: u32) -> Image {
    if w == image.width && h == image.height {
        return Image { width: w, height: h, rgba: image.rgba.clone() };
    }
    let (sw, sh) = (image.width as usize, image.height as usize);
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for ty in 0..h as usize {
        let y0 = ty * sh / h as usize;
        let y1 = ((ty + 1) * sh / h as usize).max(y0 + 1).min(sh);
        for tx in 0..w as usize {
            let x0 = tx * sw / w as usize;
            let x1 = ((tx + 1) * sw / w as usize).max(x0 + 1).min(sw);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = (y * sw + x) * 4;
                    for c in 0..4 {
                        sum[c] += u32::from(image.rgba[p + c]);
                    }
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as u32;
            out.extend(sum.map(|v| (v / n) as u8));
        }
    }
    Image { width: w, height: h, rgba: out }
}

/// `ImageUtil.decodeImageFromWebP` off the game thread, then the copies the client keeps.
fn decode_in_background(id: u128, bytes: Vec<u8>) {
    std::thread::spawn(move || {
        let result = horizon_client::hud::decode_image_bytes(&bytes).ok().map(|(width, height, rgba)| {
            let full = Image { width, height, rgba };
            (fit(&full, horizon_client::hud::MOD_TEXTURE_SIZE), fit(&full, VIEW_SIZE))
        });
        if result.is_none() {
            log::error!("Camerapture: failed to decode received image bytes for image {}", cam::uuid_string(id));
        }
        let bytes: Arc<[u8]> = bytes.into();
        store(|s| s.decoded.push((id, result, Some(bytes))));
    });
}

/// `PictureTaker.uploadStoredPicture`'s compression: a lossless WebP, its colours and then
/// its size lowered until it fits `max_bytes` (Camerapture lowers a lossy WebP's quality).
fn encode(image: &Image, max_bytes: usize) -> Result<Vec<u8>, String> {
    let quantized = |img: &Image, bits: u32| -> Vec<u8> {
        if bits >= 8 {
            return img.rgba.clone();
        }
        let shift = 8 - bits;
        img.rgba
            .iter()
            .enumerate()
            .map(|(i, &v)| if i % 4 == 3 { v } else { (v >> shift << shift) | (1 << (shift - 1)) })
            .collect()
    };
    let mut current = Image { width: image.width, height: image.height, rgba: image.rgba.clone() };
    let mut bits = 8;
    loop {
        let bytes = horizon_client::hud::encode_webp(current.width, current.height, &quantized(&current, bits))
            .map_err(|e| e.to_string())?;
        if bytes.len() <= max_bytes {
            log::debug!("Camerapture: sending picture ({} bytes, {}x{}, {bits} bits)", bytes.len(), current.width, current.height);
            return Ok(bytes);
        }
        if bits == 8 {
            bits = 6;
            continue;
        }
        if current.width.min(current.height) <= 32 {
            return Err(format!("image too big, even at 32 pixels ({} bytes)", bytes.len()));
        }
        let ratio = ((max_bytes as f64 / bytes.len() as f64).sqrt() * 0.95).clamp(0.5, 0.95);
        let w = ((f64::from(current.width) * ratio) as u32).max(16);
        let h = ((f64::from(current.height) * ratio) as u32).max(16);
        current = scaled(image, w, h);
    }
}

// --- Hooks -------------------------------------------------------------------------------

/// The active camera in the local player's hands (`CameraItem.find(player, true)`).
pub(crate) fn active_camera(game: &Game<'_>) -> bool {
    if !game.native_window() || game.player.is_none() {
        return false;
    }
    let main = game.inventory.get(game.selected).and_then(Option::as_ref);
    main.is_some_and(|s| cam::camera_active(game, s)) || game.offhand.as_ref().is_some_and(|s| cam::camera_active(game, s))
}
/// `CameraItem.getPaperInInventory`.
fn paper(game: &Game<'_>) -> u32 {
    game.inventory.iter().chain(std::iter::once(&game.offhand)).flatten().filter(|s| &*s.item == "minecraft:paper").map(|s| s.count).sum()
}
/// `CameraItem.canTakePicture`.
fn can_take(game: &Game<'_>) -> bool {
    game.creative || paper(game) > 0
}

/// `native_interaction_key` (`ClientPreAttackCallback`): the attack key with the camera up
/// takes a picture instead of attacking.
pub(crate) fn interaction_key(game: &mut Game<'_>, button: i32, swing_hand: &mut bool, canceled: &mut bool) {
    if button != 0 || !active_camera(game) {
        return;
    }
    *canceled = true;
    *swing_hand = false;
    if can_take(game) {
        take_picture(game);
    }
}

/// `PictureTaker.takePicture` and `renderTickEnd`: the next frame's world, without the HUD.
fn take_picture(game: &mut Game<'_>) {
    if store(|s| s.capture.is_some() || s.encoding) {
        return;
    }
    let max = store(|s| s.synced.max_image_resolution).max(16);
    let path = game.native_instance_dir().join("camerapture").join("capture.png");
    let _ = std::fs::remove_file(&path);
    if game.native_grab_world(&path, max).is_ok() {
        store(|s| s.capture = Some((path, Instant::now())));
    }
}

/// `native_client_tick_post`: messages out, pictures in, the grab and the upload.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    if game.player.is_none() {
        return;
    }
    // The grab is drawn: encode it off the thread and tell the server (`NewPicturePacket`).
    let grabbed = store(|s| {
        let done = s.capture.as_ref().is_some_and(|(p, _)| p.is_file());
        let stale = s.capture.as_ref().is_some_and(|(_, at)| at.elapsed().as_secs() > 10);
        if stale {
            s.capture = None;
        }
        if done {
            s.encoding = true;
            s.capture.take().map(|(p, _)| (p, s.synced, s.config))
        } else {
            None
        }
    });
    if let Some((path, synced, config)) = grabbed {
        if config.save_screenshot {
            let name = chrono::Local::now().format("%Y-%m-%d_%H.%M.%S.png").to_string();
            let dir = game.native_instance_dir().join("screenshots");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(&path, dir.join(name));
        }
        std::thread::spawn(move || {
            let result = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| horizon_client::hud::decode_bytes(&b).map_err(|e| e.to_string()))
                .and_then(|(width, height, rgba)| {
                    let full = fit(&Image { width, height, rgba }, synced.max_image_resolution.max(16));
                    let bytes = encode(&full, synced.max_image_bytes.max(1024) as usize)?;
                    // What the server will keep, as clients will decode it.
                    let kept = horizon_client::hud::decode_image_bytes(&bytes)
                        .map(|(width, height, rgba)| Image { width, height, rgba })
                        .unwrap_or(full);
                    Ok((bytes, fit(&kept, horizon_client::hud::MOD_TEXTURE_SIZE), fit(&kept, VIEW_SIZE)))
                });
            store(|s| s.encoded = Some(result));
        });
        let _ = game.native_net_send_to_server(cam::NEW_PICTURE, &[]);
    }
    // The server reserved an id and the picture is ready: up it goes, in sections.
    let upload = store(|s| {
        let id = s.upload_id?;
        let encoded = s.encoded.take()?;
        s.upload_id = None;
        s.encoding = false;
        Some((id, encoded))
    });
    if let Some((id, encoded)) = upload {
        match encoded {
            Ok((bytes, texture, view)) => {
                let mut sent = 0;
                store(|s| {
                    while sent < bytes.len() {
                        let end = (sent + cam::SECTION).min(bytes.len());
                        s.outgoing.push_back((cam::UPLOAD_PARTIAL, cam::section_message(id, bytes.len() - end, &bytes[sent..end])));
                        sent = end;
                    }
                    s.decoded.push((id, Some((texture, view)), Some(bytes.into())));
                });
            }
            Err(e) => {
                log::error!("Camerapture: failed to send picture to server: {e}");
                game.native_chat(&Text::tr("text.camerapture.upload_failed", &[]).plain());
            }
        }
    }
    // Requests, then sections, a few a tick.
    let out: Vec<(&'static str, Vec<u8>)> = store(|s| {
        for id in std::mem::take(&mut s.requests) {
            s.outgoing.push_back((cam::REQUEST_DOWNLOAD, id.to_be_bytes().to_vec()));
        }
        let n = s.outgoing.len().min(SENDS_PER_TICK);
        s.outgoing.drain(..n).collect()
    });
    for (channel, data) in out {
        if game.native_net_send_to_server(channel, &data).is_err() {
            break;
        }
    }
    // Decoded pictures become textures (`processReceivedImage`).
    let decoded = store(|s| std::mem::take(&mut s.decoded));
    for (id, result, bytes) in decoded {
        let status = match result {
            Some((texture, view)) => {
                game.native_mod_texture_set(&texture_key(id), texture.width, texture.height, &texture.rgba);
                store(|s| {
                    s.pictures.insert(
                        id,
                        Remote { status: Status::Success, size: [view.width, view.height], view: Some(Arc::new(view)), bytes },
                    );
                });
                continue;
            }
            None => Status::Error,
        };
        store(|s| {
            if let Some(p) = s.pictures.get_mut(&id) {
                p.status = status;
            }
        });
    }
    // `InGameHudMixin`: the zoom goes back to 1 when the camera is down.
    if !active_camera(game) {
        store(|s| s.zoom = MIN_ZOOM);
    }
    super::camerapture_frames::client_tick(game);
}

/// `native_frame`: the screen's actions and its full-size picture.
pub(crate) fn frame(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let open = game.native_mod_screen() == Some(cam::ID);
    let (actions, upload) = store(|s| {
        if !open {
            s.screen = None;
        }
        let actions = std::mem::take(&mut s.actions);
        // The shown picture into the mod image slot, once it is decoded.
        let upload = s.screen.as_mut().and_then(|screen| {
            let id = screen.pictures.get(screen.index)?.id;
            if screen.shown_full.is_some_and(|(shown, _)| shown == id) {
                return None;
            }
            let view = s.pictures.get(&id)?.view.clone()?;
            screen.shown_full = Some((id, [view.width, view.height]));
            Some(view)
        });
        (actions, upload)
    });
    if let Some(view) = upload
        && !game.native_mod_image(view.width, view.height, &view.rgba)
    {
        store(|s| {
            if let Some(screen) = s.screen.as_mut() {
                screen.shown_full = None;
            }
        });
    }
    for action in actions {
        match action {
            Action::Click => game.audio.play_button_click(),
            Action::TakeBook(pos) => {
                let mut data = Vec::new();
                for v in pos {
                    data.extend_from_slice(&v.to_be_bytes());
                }
                let _ = game.native_net_send_to_server(super::camerapture_frames::TAKE_BOOK, &data);
                game.native_mod_screen_close(cam::ID);
            }
            Action::Save(id) => save_picture(game, id),
        }
    }
}

/// Ctrl-S on the picture screen (`PictureScreen.saveAs`): Horizon has no file dialog, so the
/// picture goes to `screenshots/camerapture/<uuid>.png`, said in chat.
fn save_picture(game: &mut Game<'_>, id: u128) {
    let Some(bytes) = store(|s| s.pictures.get(&id).and_then(|p| p.bytes.clone())) else {
        return;
    };
    let dir = game.native_instance_dir().join("screenshots").join("camerapture");
    let path = dir.join(format!("{}.png", cam::uuid_string(id)));
    let written = horizon_client::hud::decode_image_bytes(&bytes).map_err(|e| e.to_string()).and_then(|(w, h, rgba)| {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().and_then(|mut wr| wr.write_image_data(&rgba)).map_err(|e| e.to_string())
    });
    match written {
        Ok(()) => game.native_chat(&format!("{} {}", Text::tr("text.camerapture.save_as", &[]).plain(), path.display())),
        Err(e) => log::error!("Camerapture: failed to save picture to disk: {e}"),
    }
}

/// `native_net_from_server`: Camerapture's clientbound packets.
pub(crate) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    match channel {
        cam::REQUEST_UPLOAD => {
            if let Some((id, _)) = cam::read_uuid(data) {
                store(|s| s.upload_id = Some(id));
            }
        }
        cam::DOWNLOAD_PARTIAL => {
            let Some((id, rest)) = cam::read_uuid(data) else {
                return;
            };
            let Some((left, bytes)) = cam::read_u32(rest) else {
                return;
            };
            let done = store(|s| {
                let collector = s.collectors.entry(id).or_insert_with(cam::Collector::new);
                if !collector.push(bytes, left as usize) {
                    log::error!("Camerapture: received malformed byte section from server");
                    s.collectors.remove(&id);
                    if let Some(p) = s.pictures.get_mut(&id) {
                        p.status = Status::Error;
                    }
                    return None;
                }
                if s.collectors.get(&id).is_some_and(cam::Collector::done) {
                    s.collectors.remove(&id).map(cam::Collector::take)
                } else {
                    None
                }
            });
            if let Some(bytes) = done {
                decode_in_background(id, bytes);
            }
        }
        cam::PICTURE_ERROR => {
            if let Some((id, _)) = cam::read_uuid(data) {
                log::error!("Camerapture: remote error for image {}", cam::uuid_string(id));
                store(|s| {
                    s.collectors.remove(&id);
                    if let Some(p) = s.pictures.get_mut(&id) {
                        p.status = Status::Error;
                    }
                });
            }
        }
        cam::SYNC_CONFIG => {
            if let Some(c) = ServerConfig::from_synced(data) {
                log::info!("Camerapture: received synced config: {c:?}");
                store(|s| s.synced = c);
            }
        }
        cam::VIEW => {
            let Some((&kind, mut rest)) = data.split_first() else {
                return;
            };
            let mut lectern = None;
            if kind == 1 {
                let mut pos = [0; 3];
                for p in &mut pos {
                    let Some((v, r)) = rest.split_first_chunk::<4>() else {
                        return;
                    };
                    *p = i32::from_be_bytes(*v);
                    rest = r;
                }
                lectern = Some(pos);
            }
            let mut pictures = Vec::new();
            while let Some((id, r)) = cam::read_uuid(rest) {
                let Some((name, r)) = cam::read_str(r) else {
                    break;
                };
                pictures.push(Shown { id, name: (!name.is_empty()).then_some(name) });
                rest = r;
            }
            open_screen(game, pictures, lectern);
        }
        _ => super::camerapture_frames::from_server(game, channel, data),
    }
}

/// Opens (or refreshes) the picture screen.
pub(crate) fn open_screen(game: &mut Game<'_>, pictures: Vec<Shown>, lectern: Option<[i32; 3]>) {
    let open = game.native_mod_screen() == Some(cam::ID) || game.native_mod_screen_open(cam::ID);
    if !open {
        return;
    }
    for p in &pictures {
        ensure_picture(p.id);
    }
    store(|s| s.screen = Some(PictureScreen { pictures, index: 0, lectern, shown_full: None }));
    super::camerapture_frames::screen_opened();
}

/// The frame editor takes the mod's screen: no pictures under it.
pub(crate) fn open_screen_none() {
    store(|s| s.screen = None);
}

/// `native_zoom`: the camera's zoom (`AbstractClientPlayerEntityMixin.getFovMultiplier`)
/// and the slower mouse it brings (`MouseMixin.updateMouse`).
pub(crate) fn zoom(game: &Game<'_>, zoom: &mut Option<horizon_client::mod_api::NativeZoom>) {
    if zoom.is_some() || !active_camera(game) {
        return;
    }
    let (level, sensitivity) = store(|s| (s.zoom, s.config.zoom_mouse_sensitivity));
    // `PictureTaker.getFovModifier` and `getSensitivityModifier`.
    let progress = (level - MIN_ZOOM) / (MAX_ZOOM - MIN_ZOOM);
    let fov = 0.1 + 0.9 * (1.0 - progress).powi(2);
    let turn = 1.0 - progress * (1.0 - sensitivity);
    *zoom = Some(horizon_client::mod_api::NativeZoom { factor: 1.0 / fov, smooth: true, turn });
}

/// `native_wheel` (`MouseMixin.onScroll`): the wheel zooms the camera, not the hotbar.
pub(crate) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if !active_camera(game) {
        return;
    }
    *claimed = true;
    store(|s| s.zoom = (s.zoom + steps / 4.0).clamp(MIN_ZOOM, MAX_ZOOM));
}

/// `native_gui_layer_pre` (`InGameHudMixin`): no HUD while the camera is up.
pub(crate) fn gui_layer(game: &Game<'_>, canceled: &mut bool) {
    if active_camera(game) {
        *canceled = true;
    }
}

/// `native_render_hand` (`HeldItemRendererMixin`): no hands while the camera is up.
pub(crate) fn render_hand(game: &Game<'_>, canceled: &mut bool) {
    if active_camera(game) {
        *canceled = true;
    }
}

/// `native_block_highlight` (`GameRendererMixin.shouldRenderBlockOutline`).
pub(crate) fn block_highlight(game: &Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if active_camera(game) || super::camerapture_frames::aimed_wall(pos) {
        *canceled = true;
    }
}

fn color(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    rgba(r, g, b, a)
}
fn fill(game: &Game<'_>, batch: &mut HudBatch, x1: i32, y1: i32, x2: i32, y2: i32, argb: u32) {
    game.native_fill(batch, [x1 as f32, y1 as f32, (x2 - x1) as f32, (y2 - y1) as f32], argb);
}
fn text(game: &Game<'_>, batch: &mut HudBatch, t: &Text, x: f32, y: f32, argb: u32, shadow: bool) {
    batch.text_styled(&game.gui.atlas, t, x, y, color(argb), shadow);
}
fn width(game: &Game<'_>, t: &Text) -> f32 {
    game.gui.atlas.font.width_text(t)
}

/// `native_hud` (`CameraViewFinder.drawCameraViewFinder`), not under F1.
pub(crate) fn hud(game: &Game<'_>, batch: &mut HudBatch) {
    if !active_camera(game) {
        return;
    }
    let (w, h) = (batch.width as i32, batch.height as i32);
    view_finder(game, batch, 10, 10, w - 10, h - 10, 2, 30);
    view_finder(game, batch, w / 2 - 20, h / 2 - 20, w / 2 + 20, h / 2 + 20, 1, 10);
    zoom_bar(game, batch, w - 10, h / 2 - h / 6, h / 3);
    let config = store(|s| s.config);
    let fh = 9.0;
    let (tx, ty) = (25.0, (h - 25) as f32);
    if !config.simple_camera_hud {
        let date = chrono::Local::now().format("%Y/%m/%d").to_string();
        text(game, batch, &Text::tr("text.camerapture.date", &[&date]), tx, ty - fh, 0xFFFF_FFFF, false);
    }
    if !can_take(game) {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        if millis % 1000 < 500 {
            let t = Text::tr("text.camerapture.no_paper", &[]);
            let x = (w / 2) as f32 - width(game, &t) / 2.0;
            text(game, batch, &t, x.floor(), (h / 2 + 32) as f32, 0xFFFF_0000, false);
        }
    } else if !config.simple_camera_hud {
        let t = cam::tr_d("text.camerapture.paper_available", &[&paper(game)]);
        let x = (w - 25) as f32 - width(game, &t);
        text(game, batch, &t, x, (h - 25) as f32 - fh, 0xFFFF_FFFF, false);
    }
}
#[allow(clippy::too_many_arguments)]
fn view_finder(game: &Game<'_>, batch: &mut HudBatch, x1: i32, y1: i32, x2: i32, y2: i32, thickness: i32, length: i32) {
    let white = 0xFFFF_FFFF;
    fill(game, batch, x1, y1, x1 + length, y1 + thickness, white);
    fill(game, batch, x1, y1, x1 + thickness, y1 + length, white);
    fill(game, batch, x2 - length, y1, x2, y1 + thickness, white);
    fill(game, batch, x2 - thickness, y1, x2, y1 + length, white);
    fill(game, batch, x1, y2 - thickness, x1 + length, y2, white);
    fill(game, batch, x1, y2 - length, x1 + thickness, y2, white);
    fill(game, batch, x2 - length, y2 - thickness, x2, y2, white);
    fill(game, batch, x2 - thickness, y2 - length, x2, y2, white);
}
fn zoom_bar(game: &Game<'_>, batch: &mut HudBatch, x: i32, y: i32, height: i32) {
    let ticks = (height / 10).max(2);
    for i in 0..ticks {
        let ty = y + height * i / (ticks - 1);
        fill(game, batch, x - 6, ty, x, ty + 1, 0xB0FF_FFFF);
    }
    let level = store(|s| s.zoom);
    let progress = 1.0 - (level - MIN_ZOOM) / (MAX_ZOOM - MIN_ZOOM);
    let ty = y + (height as f32 * progress) as i32;
    fill(game, batch, x - 10, ty - 1, x, ty + 1, 0xFFFF_FFFF);
    let label = Text::literal(format!("{level:.1}x"));
    let tw = width(game, &label);
    text(game, batch, &label, (x - 12) as f32 - tw, (ty - 4) as f32, 0xFFFF_FFFF, false);
}

/// `native_item_model_property`: `camerapture:should_render_picture`, the picture item's
/// model override (`PictureItemRenderer.canRender`).
pub(crate) fn item_model_property(property: &str, stack: &horizon_assets::item_model::StackView, value: &mut bool) {
    if property != "camerapture:should_render_picture" {
        return;
    }
    let data = stack.patch.get(cam::PICTURE_DATA).and_then(Option::as_ref).and_then(PictureData::from_json);
    *value = data.is_some_and(|d| picture(d.id).0 == Status::Success);
}

/// `native_special_item_model_claimed`.
pub(crate) fn special_claimed(kind: &str, claimed: &mut bool) {
    if kind == "camerapture:picture" {
        *claimed = true;
    }
}

/// `native_special_item_model` (`PictureItemRenderer.render`): the picture on a quad in the
/// item's cube, fitted to 14 of its 16 pixels.
pub(crate) fn special_item_model(
    game: &Game<'_>,
    kind: &str,
    argument: &serde_json::Value,
    pose: glam::Mat4,
    sink: &mut dyn horizon_client::item_render::ItemSink,
) {
    use glam::Vec3;
    if kind != "camerapture:picture" {
        return;
    }
    let Some(data) = argument.get(cam::PICTURE_DATA).and_then(PictureData::from_json) else {
        return;
    };
    let (status, [pw, ph]) = picture(data.id);
    if status != Status::Success || pw == 0 || ph == 0 {
        return;
    }
    let Some(rect) = game.native_mod_texture(&texture_key(data.id)) else {
        return;
    };
    let mut m = pose
        * glam::Mat4::from_translation(Vec3::new(0.0, 0.0, 0.5))
        * glam::Mat4::from_translation(Vec3::new(0.0625, 0.0625, 0.0))
        * glam::Mat4::from_scale(Vec3::splat(0.875));
    if pw > ph {
        let h = ph as f32 / pw as f32;
        m = m * glam::Mat4::from_translation(Vec3::new(0.0, (1.0 - h) / 2.0, 0.0)) * glam::Mat4::from_scale(Vec3::new(1.0, h, 1.0));
    } else {
        let w = pw as f32 / ph as f32;
        m = m * glam::Mat4::from_translation(Vec3::new((1.0 - w) / 2.0, 0.0, 0.0)) * glam::Mat4::from_scale(Vec3::new(w, 1.0, 1.0));
    }
    let pos = [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]].map(|[x, y]| m.transform_point3(Vec3::new(x, y, 0.0)));
    let local = [[1.0f32, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]];
    let uv = local.map(|[u, v]| [rect.u0 + (rect.u1 - rect.u0) * u, rect.v0 + (rect.v1 - rect.v0) * v]);
    let normal = (pos[1] - pos[0]).cross(pos[2] - pos[0]).normalize_or(Vec3::Z);
    let normal = if m.determinant() < 0.0 { -normal } else { normal };
    sink.quad(horizon_client::item_render::ItemQuad {
        pos,
        uv,
        foil_uv: local,
        gui_glint_uv: None,
        normal,
        color: 0xFFFF_FFFF,
        foil: horizon_client::item_render::Foil::None,
        light_emission: 0,
        cull_back: false,
        translucent: false,
        oit: false,
    });
}

// --- The picture screen ------------------------------------------------------------------

/// `LoadingDisplay.get`.
pub(crate) fn loading_dots() -> &'static str {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    ["O o o", "o O o", "o o O", "o O o"][(millis / 300 % 4) as usize]
}

/// `PictureDrawingUtil.drawPicture`: the picture fitted into the box, or what is happening
/// to it. `full` draws the full-size copy from the mod image slot when it is this one.
pub(crate) fn draw_picture(game: &Game<'_>, batch: &mut HudBatch, id: u128, area: [f32; 4], full: Option<[u32; 2]>) {
    let [x, y, w, h] = area;
    let (status, [pw, ph]) = picture(id);
    match status {
        Status::Fetching => {
            let t = Text::tr("text.camerapture.fetching_picture", &[]);
            text(game, batch, &t, x + w / 2.0 - width(game, &t) / 2.0, y + h / 2.0 - 9.0, 0xFFFF_FFFF, true);
            let d = Text::literal(loading_dots());
            text(game, batch, &d, x + w / 2.0 - width(game, &d) / 2.0, y + h / 2.0, 0xFF80_8080, true);
        }
        Status::Error => {
            let t = Text::tr("text.camerapture.fetching_failed", &[]);
            text(game, batch, &t, x + w / 2.0 - width(game, &t) / 2.0, y + h / 2.0 - 4.0, 0xFFFF_0000, true);
        }
        Status::Success => {
            if pw == 0 || ph == 0 {
                return;
            }
            let scale = (w / pw as f32).min(h / ph as f32);
            let (nw, nh) = ((pw as f32 * scale).floor(), (ph as f32 * scale).floor());
            let dst = [(x + w / 2.0 - nw / 2.0).floor(), (y + h / 2.0 - nh / 2.0).floor(), nw, nh];
            let drawn = full.is_some_and(|[fw, fh]| {
                game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, [0.0, 0.0, fw as f32, fh as f32], dst)
            });
            if !drawn {
                game.native_mod_texture_draw(batch, &texture_key(id), dst, 0xFFFF_FFFF);
            }
        }
    }
}

/// A button as vanilla's `ButtonWidget` draws: its sprite in three slices and its label.
pub(crate) fn button(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], label: &Text, mouse: [f32; 2], active: bool) {
    let [x, y, w, h] = rect;
    let hovered = active && hit(rect, mouse);
    let sprite = if !active {
        "gui/sprites/widget/button_disabled"
    } else if hovered {
        "gui/sprites/widget/button_highlighted"
    } else {
        "gui/sprites/widget/button"
    };
    horizon_client::ui::nine_slice(batch, &game.gui.atlas, sprite, x, y, w, h, u32::MAX);
    let c = if active { 0xFFFF_FFFF } else { 0xFFA0_A0A0 };
    text(game, batch, label, x + w / 2.0 - width(game, label) / 2.0, y + (h - 8.0) / 2.0, c, true);
}
pub(crate) fn hit([x, y, w, h]: [f32; 4], [mx, my]: [f32; 2]) -> bool {
    mx >= x && my >= y && mx < x + w && my < y + h
}

struct Layout {
    previous: Option<[f32; 4]>,
    next: Option<[f32; 4]>,
    take: Option<[f32; 4]>,
    picture: [f32; 4],
}
fn layout(screen: &PictureScreen, w: f32, h: f32, can_take: bool) -> Layout {
    let single = screen.pictures.len() == 1;
    let bar_width = 360.0f32.min(w - 48.0);
    let bar_x = (w / 2.0 - bar_width / 2.0).floor();
    let bar_y = h - 24.0 - 20.0;
    let bottom = if single { 0.0 } else { 24.0 };
    Layout {
        previous: (!single).then_some([bar_x, bar_y, 20.0, 20.0]),
        next: (!single).then_some([bar_x + bar_width - 20.0, bar_y, 20.0, 20.0]),
        take: (screen.lectern.is_some() && can_take).then_some([w / 2.0 - 40.0, h - 24.0 + 2.0, 80.0, 16.0]),
        picture: [24.0, 24.0, w - 48.0, h - 48.0 - bottom],
    }
}

fn is_ours(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    screen.id == "mod" && game.native_mod_screen() == Some(cam::ID)
}

/// `native_screen_render_post`: `PictureScreen.render`.
pub(crate) fn screen_draw(game: &Game<'_>, batch: &mut HudBatch, view: &horizon_client::mod_api::NativeScreenView<'_>) {
    if !is_ours(game, view) {
        return;
    }
    if super::camerapture_frames::screen_draw(game, batch, view) {
        return;
    }
    let Some((pictures, index, lectern, full)) =
        store(|s| s.screen.as_ref().map(|sc| (sc.pictures.clone(), sc.index, sc.lectern, sc.shown_full)))
    else {
        return;
    };
    let screen = PictureScreen { pictures, index, lectern, shown_full: full };
    let (w, h) = (batch.width, batch.height);
    // `renderInGameBackground`.
    game.native_apply_screen_blur();
    game.native_fill(batch, [0.0, 0.0, w, h], 0xC810_1010);
    let lay = layout(&screen, w, h, can_modify(game));
    let mouse = view.mouse;
    if let Some(r) = lay.previous {
        button(game, batch, r, &Text::literal("\u{2190}"), mouse, true);
    }
    if let Some(r) = lay.next {
        button(game, batch, r, &Text::literal("\u{2192}"), mouse, true);
    }
    if let Some(r) = lay.take {
        button(game, batch, r, &Text::tr("lectern.take_book", &[]), mouse, true);
    }
    if screen.pictures.len() != 1 {
        let bar_y = h - 24.0 - 10.0;
        let page = Text::literal(format!("{} / {}", screen.index + 1, screen.pictures.len())).with_color(0xAAAAAA);
        let px = (w / 2.0 - width(game, &page) / 2.0).floor();
        match screen.pictures.get(screen.index).and_then(|p| p.name.clone()) {
            Some(name) => {
                let name = Text::literal(name);
                text(game, batch, &name, (w / 2.0 - width(game, &name) / 2.0).floor(), bar_y - 1.0 - 9.0, 0xFFFF_FFFF, false);
                text(game, batch, &page, px, bar_y + 1.0, 0xFFFF_FFFF, false);
            }
            None => text(game, batch, &page, px, bar_y - 4.0, 0xFFFF_FFFF, false),
        }
    }
    if let Some(shown) = screen.pictures.get(screen.index) {
        if game.has_control_down() {
            let t = Text::tr("text.camerapture.save_as", &[]).with_color(0xAAAAAA);
            text(game, batch, &t, (w / 2.0 - width(game, &t) / 2.0).floor(), 24.0 - 9.0 - 2.0, 0xFFFF_FFFF, false);
        }
        let full = screen.shown_full.filter(|(id, _)| *id == shown.id).map(|(_, size)| size);
        draw_picture(game, batch, shown.id, lay.picture, full);
    }
}

/// `ClientPlayerEntity.canModifyBlocks`: the Take Book button's condition.
fn can_modify(game: &Game<'_>) -> bool {
    game.player.is_some()
}

fn change_index(by: i64) {
    store(|s| {
        if let Some(screen) = s.screen.as_mut()
            && !screen.pictures.is_empty()
        {
            let n = screen.pictures.len() as i64;
            screen.index = (screen.index as i64 + by).rem_euclid(n) as usize;
            if let Some(p) = screen.pictures.get(screen.index) {
                let id = p.id;
                if s.pictures.get(&id).is_none_or(|p| p.status == Status::Error) {
                    s.pictures.insert(id, Remote { status: Status::Fetching, size: [0, 0], view: None, bytes: None });
                    s.requests.push(id);
                }
            }
        }
    });
}

/// `native_screen_mouse_button_pressed_pre`.
pub(crate) fn screen_press(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !is_ours(game, view) {
        return;
    }
    if super::camerapture_frames::screen_press(game, view, button) {
        *canceled = true;
        return;
    }
    let Some(screen) = store(|s| s.screen.as_ref().map(|sc| PictureScreen { pictures: sc.pictures.clone(), index: sc.index, lectern: sc.lectern, shown_full: None })) else {
        return;
    };
    *canceled = true;
    if button != 0 {
        return;
    }
    let (w, h) = game.gui_size();
    let lay = layout(&screen, w, h, can_modify(game));
    if lay.previous.is_some_and(|r| hit(r, view.mouse)) {
        change_index(-1);
        click();
    } else if lay.next.is_some_and(|r| hit(r, view.mouse)) {
        change_index(1);
        click();
    } else if let (Some(r), Some(pos)) = (lay.take, screen.lectern)
        && hit(r, view.mouse)
    {
        click();
        store(|s| s.actions.push(Action::TakeBook(pos)));
    }
}

/// `native_screen_mouse_scrolled_pre` (`PictureScreen.mouseScrolled`).
pub(crate) fn screen_scroll(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !is_ours(game, view) || store(|s| s.screen.is_none()) {
        return;
    }
    *canceled = true;
    if scroll[1] != 0.0 {
        change_index(-(scroll[1].signum() as i64));
    }
}

/// `native_screen_key_pressed_pre` (`PictureScreen.keyPressed`).
pub(crate) fn screen_key(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    if !is_ours(game, view) {
        return;
    }
    if super::camerapture_frames::screen_key(game, key) {
        return;
    }
    let Some(id) = store(|s| s.screen.as_ref().and_then(|sc| sc.pictures.get(sc.index).map(|p| p.id))) else {
        return;
    };
    match key {
        "key.keyboard.left" => {
            change_index(-1);
            *canceled = true;
        }
        "key.keyboard.right" => {
            change_index(1);
            *canceled = true;
        }
        "key.keyboard.s" if modifiers & 2 != 0 => {
            if picture(id).0 == Status::Success {
                store(|s| s.actions.push(Action::Save(id)));
            }
            *canceled = true;
        }
        _ => {}
    }
}

/// The button sound, played from the frame hook (`PressableWidget.playDownSound`).
pub(crate) fn click() {
    store(|s| s.actions.push(Action::Click));
}
