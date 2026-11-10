// Xaero's World Map for Horizon, written from its documented behaviour (none of Xaero's code
// or assets): the world the player has explored, kept per world and dimension and shown full
// screen.
//  - As the player explores, each chunk they load goes on the map (its top blocks' map
//    colours, shaded by height as a map item is; the Nether's floor under its roof). The map
//    is saved under `xaero/world-map/<world>/<dimension>/` in the game folder every ten
//    seconds and on leaving the world.
//  - M opens the map: drag to move it, scroll to zoom (1/8 to 16 blocks a pixel), Space goes
//    back to the player, Escape or M closes it. The player is the white arrow; the block
//    under the pointer and its height are shown at the bottom.
use super::Game;
use horizon_client::{hud::HudBatch, minimap::Layer, text::Text, world_map::WorldMap};
use std::{sync::Mutex, time::Duration};

pub const ID: &str = "xaeroworldmap";
pub const VERSION: &str = "1.39.0";
pub const DESCRIPTION: &str = "A full-screen map of the world you have explored, saved per world and dimension. Press M to open it.";
pub const KEY: &str = "open";
pub const DEFAULT_KEY: &str = "key.keyboard.m";
/// Map tiles a texture holds (the mod texture slots' side).
const TILE: usize = 512;
const ZOOMS: [f64; 8] = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

struct State {
    map: WorldMap,
    ticks: u64,
    /// The screen's view: its centre in blocks, the zoom (an index in ZOOMS) and whether it
    /// follows the player.
    center: [f64; 2],
    zoom: usize,
    follow: bool,
    /// What the textures show: centre, zoom, map revision, size.
    shown: Option<([f64; 2], usize, u64, [usize; 2])>,
    textures: [usize; 2],
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(|| State {
        map: WorldMap::default(),
        ticks: 0,
        center: [0.0; 2],
        zoom: 3,
        follow: true,
        shown: None,
        textures: [0; 2],
    }))
}

/// A world's folder name: letters, digits and a few signs kept, the rest `_`.
fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect()
}

/// The folder of the map the player is on: the world, then the dimension.
fn map_dir(game: &Game<'_>) -> Option<std::path::PathBuf> {
    let client = game.native_client_state()?;
    let world = match client.world? {
        super::native_client_services::NativeWorldId::Singleplayer(folder) => format!("singleplayer/{}", clean(&folder)),
        super::native_client_services::NativeWorldId::Server(address) => format!("multiplayer/{}", clean(&address)),
    };
    let dim = clean(&game.dim.id().replace(':', "_"));
    Some(game.native_instance_dir().join("xaero").join("world-map").join(world).join(dim))
}

/// `native_client_tick_post`: the chunks round the player go on the map; saved every minute.
pub(crate) fn client_tick(game: &mut Game<'_>) {
    if !game.native_on(ID) {
        return;
    }
    let dir = if game.player.is_some() { map_dir(game) } else { None };
    let Some(p) = game.player.as_ref() else {
        state(|s| {
            if s.map.dir().is_some() {
                let _ = s.map.open(None);
            }
        });
        return;
    };
    let (bx, by, bz) = (p.position.x.floor() as i32, (p.position.y + f64::from(game.eye)).floor() as i32, p.position.z.floor() as i32);
    let layer = if game.dim == super::Dim::Nether { Layer::Below(by.min(120)) } else { Layer::Surface };
    let radius = 12;
    state(|s| {
        if s.map.dir() != dir.as_deref() {
            if let Err(e) = s.map.open(dir.clone()) {
                log::warn!("[xaeroworldmap] the map was not saved: {e}");
            }
            s.follow = true;
            s.shown = None;
        }
        s.map.update(&game.world, game.res.facts(), (bx >> 4, bz >> 4), radius, layer, Duration::from_micros(1500));
        s.ticks += 1;
        if s.ticks % 200 == 0 {
            if let Err(e) = s.map.save() {
                log::warn!("[xaeroworldmap] the map was not saved: {e}");
            }
        }
    });
}

/// `native_key`: M opens the map.
pub(crate) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_on(ID) || !game.native_playing() {
        return;
    }
    if key == game.native_mod_key(ID, KEY, DEFAULT_KEY) && game.native_mod_screen().is_none() {
        state(|s| {
            s.follow = true;
            s.shown = None;
        });
        game.native_mod_screen_open(ID);
    }
}

fn ours(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    view.id == "mod" && game.native_mod_screen() == Some(ID)
}

/// `native_screen_render_post`: the map, the player and the pointer's block.
pub(crate) fn screen_draw(game: &Game<'_>, batch: &mut HudBatch, view: &horizon_client::mod_api::NativeScreenView<'_>) {
    if !ours(game, view) {
        return;
    }
    let (w, h) = (batch.width, batch.height);
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let player = [p.position.x, p.position.z];
    let (center, bpp) = state(|s| {
        if s.follow {
            s.center = player;
        }
        let bpp = ZOOMS[s.zoom];
        let size = [w.ceil() as usize, h.ceil() as usize];
        let want = (s.center, s.zoom, s.map.revision, size);
        if s.shown != Some(want) {
            let image = s.map.compose(s.center[0], s.center[1], bpp, size[0], size[1]);
            let (tw, th) = (size[0].div_ceil(TILE), size[1].div_ceil(TILE));
            for ty in 0..th {
                for tx in 0..tw {
                    let (x0, y0) = (tx * TILE, ty * TILE);
                    let (cw, ch) = ((size[0] - x0).min(TILE), (size[1] - y0).min(TILE));
                    let mut part = Vec::with_capacity(cw * ch * 4);
                    for y in y0..y0 + ch {
                        part.extend_from_slice(&image[(y * size[0] + x0) * 4..(y * size[0] + x0 + cw) * 4]);
                    }
                    game.native_mod_texture_set(&format!("{ID}:tile{tx}_{ty}"), cw as u32, ch as u32, &part);
                }
            }
            s.textures = [tw, th];
            s.shown = Some(want);
        }
        (s.center, bpp)
    });
    game.native_fill(batch, [0.0, 0.0, w, h], 0xFF10_1012);
    let [tw, th] = state(|s| s.textures);
    for ty in 0..th {
        for tx in 0..tw {
            let (x0, y0) = ((tx * TILE) as f32, (ty * TILE) as f32);
            let dst = [x0, y0, (TILE as f32).min(w - x0), (TILE as f32).min(h - y0)];
            game.native_mod_texture_draw(batch, &format!("{ID}:tile{tx}_{ty}"), dst, 0xFFFF_FFFF);
        }
    }
    // The player: an arrow the way they face.
    let at = [
        w / 2.0 + ((player[0] - center[0]) / bpp) as f32,
        h / 2.0 + ((player[1] - center[1]) / bpp) as f32,
    ];
    let (s, c) = game.yaw.to_radians().sin_cos();
    // Yaw 0 faces south (+z, down the screen).
    let fwd = [-s, c];
    let side = [c, s];
    let pt = |f: f32, r: f32| [at[0] + fwd[0] * f + side[0] * r, at[1] + fwd[1] * f + side[1] * r];
    batch.fill_quad(&game.gui.atlas, [pt(7.0, 0.0), pt(-5.0, 5.0), pt(-2.0, 0.0), pt(-5.0, -5.0)], horizon_client::hud::rgba(0, 0, 0, 255));
    batch.fill_quad(&game.gui.atlas, [pt(5.5, 0.0), pt(-4.0, 3.8), pt(-1.5, 0.0), pt(-4.0, -3.8)], horizon_client::hud::rgba(255, 255, 255, 255));
    // The pointer's block, and how to use the map.
    let mx = center[0] + f64::from(view.mouse[0] - w / 2.0) * bpp;
    let mz = center[1] + f64::from(view.mouse[1] - h / 2.0) * bpp;
    let (bx, bz) = (mx.floor() as i32, mz.floor() as i32);
    let height = state(|s| s.map.height_at(bx, bz));
    let line = match height {
        Some(y) => format!("X {bx}  Y {y}  Z {bz}"),
        None => format!("X {bx}  Z {bz}"),
    };
    let zoom = if bpp >= 1.0 { format!("1:{}", bpp as u32) } else { format!("{}:1", (1.0 / bpp) as u32) };
    let help = format!("{line}    {zoom}    Drag to move, scroll to zoom, Space: back to you");
    let t = Text::literal(help);
    let tw = game.gui.atlas.font.width_text(&t);
    game.native_fill(batch, [w / 2.0 - tw / 2.0 - 4.0, h - 16.0, tw + 8.0, 13.0], 0xA000_0000);
    batch.text_styled(&game.gui.atlas, &t, w / 2.0 - tw / 2.0, h - 14.0, horizon_client::hud::rgba(255, 255, 255, 255), false);
    let title = Text::literal("World Map");
    let tw = game.gui.atlas.font.width_text(&title);
    batch.text_styled(&game.gui.atlas, &title, w / 2.0 - tw / 2.0, 6.0, horizon_client::hud::rgba(255, 255, 255, 255), true);
}

/// `native_screen_mouse_dragged_pre`: the map follows the pointer.
pub(crate) fn screen_drag(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, delta: [f32; 2], canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    *canceled = true;
    state(|s| {
        let bpp = ZOOMS[s.zoom];
        s.follow = false;
        s.center[0] -= f64::from(delta[0]) * bpp;
        s.center[1] -= f64::from(delta[1]) * bpp;
    });
}

/// `native_screen_mouse_scrolled_pre`: zoom, about the pointer.
pub(crate) fn screen_scroll(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !ours(game, view) || scroll[1] == 0.0 {
        return;
    }
    *canceled = true;
    state(|s| {
        let old = ZOOMS[s.zoom];
        s.zoom = if scroll[1] > 0.0 { s.zoom.saturating_sub(1) } else { (s.zoom + 1).min(ZOOMS.len() - 1) };
        let new = ZOOMS[s.zoom];
        // The block under the pointer stays under it.
        let (w, h) = game.gui_size();
        let (dx, dy) = (f64::from(view.mouse[0] - w / 2.0), f64::from(view.mouse[1] - h / 2.0));
        if !s.follow {
            s.center[0] += dx * (old - new);
            s.center[1] += dy * (old - new);
        }
    });
}

/// `native_screen_key_pressed_pre`: Space back to the player, M closes.
pub(crate) fn screen_key(game: &Game<'_>, view: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !ours(game, view) {
        return;
    }
    if key == "key.keyboard.space" {
        *canceled = true;
        state(|s| s.follow = true);
    } else if key == game.native_mod_key(ID, KEY, DEFAULT_KEY) {
        *canceled = true;
        CLOSE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
/// M pressed on the map: closed on the next tick (the key hooks can't close a screen).
static CLOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) fn close_pending(game: &mut Game<'_>) {
    if CLOSE.swap(false, std::sync::atomic::Ordering::Relaxed) {
        game.native_mod_screen_close(ID);
    }
}


/// `native_client_disconnecting`: what changed is written before the world goes.
pub(crate) fn save() {
    state(|s| {
        if let Err(e) = s.map.save() {
            log::warn!("[xaeroworldmap] the map was not saved: {e}");
        }
    });
}
