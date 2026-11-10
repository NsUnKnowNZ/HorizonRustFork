use super::Game;
use horizon_client::mobs::render::umbx_skins3d_cache as voxels;
use horizon_client::mesher::Vertex;
use horizon_client::mobs::{Mob, MobType, Part, render_model};
use std::collections::HashSet;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

pub(super) const ID: &str = "skins_3d";
const VERSION: &str = "1.1.0";
const DESCRIPTION: &str = "The outer layer of every skin as real 3D pixels: hat, jacket, sleeves and pants.";
const REACH: &str = "reach";
const REACHES: [u32; 4] = [8, 16, 32, 64];
const DEFAULT_REACH: u32 = 32;
const SKIN_EDGE: usize = 64;
const MAX_MISSING: usize = 512;

const OFFLINE_SKIN: &str = "horizon/offline_skin";
const ELYTRA: &str = "minecraft:elytra";

static ACTIVE: AtomicBool = AtomicBool::new(false);
static MISSING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static LOGGED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn reach(game: &Game<'_>) -> u32 {
    game.native_setting(ID, REACH).and_then(|v| v.parse().ok()).filter(|r| REACHES.contains(r)).unwrap_or(DEFAULT_REACH)
}

fn size(bytes: usize) -> Option<(usize, usize)> {
    let pixels = bytes / 4;
    let side = (pixels as f64).sqrt() as usize;
    if bytes % 4 != 0 || pixels == 0 {
        None
    } else if side * side == pixels && side % SKIN_EDGE == 0 {
        Some((side, side))
    } else if pixels == SKIN_EDGE * SKIN_EDGE / 2 {
        Some((SKIN_EDGE, SKIN_EDGE / 2))
    } else {
        None
    }
}

fn texture_file(resources: &Path, location: &str) -> Option<PathBuf> {
    let safe = !location.is_empty()
        && !location.contains("..")
        && !location.starts_with('/')
        && location.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_/.-".contains(&b));
    safe.then(|| resources.join("assets").join("minecraft").join("textures").join(format!("{location}.png")))
}

fn bundled(game: &Game<'_>, location: &str) -> Option<(usize, usize, Vec<u8>)> {
    let file = texture_file(&game.options.resources, location)?;
    let (width, height, rgba) = horizon_client::hud::decode(&file).ok()?;
    Some((width as usize, height as usize, rgba))
}

fn shown(head: bool, chest: Option<&str>, legs: bool) -> u8 {
    let mut shown = voxels::ALL;
    if head {
        shown &= !voxels::HAT;
    }
    if chest.is_some_and(|item| item != ELYTRA) {
        shown &= !voxels::JACKET_AND_SLEEVES;
    }
    if legs {
        shown &= !voxels::PANTS;
    }
    shown
}

fn swap(m: &mut Mob, near: bool, shown: u8) {
    let Some(location) = m.skin.as_deref() else { return };
    let model = voxels::model(&m.kind, location, m.slim, if near { shown } else { 0 });
    let same = match (&model, &m.skin_3d) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        m.skin_3d = model;
    }
}

fn restore(game: &mut Game<'_>) {
    for m in game.mobs.iter_mut().chain(game.player_model.iter_mut()) {
        if &*m.kind.id == voxels::PLAYER {
            m.skin_3d = None;
        }
    }
    game.update_remote_skins();
}

pub(super) fn model(game: &Game<'_>, kind: &Arc<MobType>, skin: &str, slim: bool) -> ControlFlow<Option<Arc<Part>>> {
    if !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    ControlFlow::Break(voxels::model(kind, skin, slim, voxels::ALL))
}

pub(super) fn arm(game: &Game<'_>, left: bool, matrix: glam::Mat4, light: u32, out: &mut Vec<Vertex>) -> ControlFlow<()> {
    if !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    let player = game.player_model.as_ref();
    let name = if left { "left_arm" } else { "right_arm" };
    let part = player.and_then(|m| m.skin_3d.as_ref()).and_then(|model| model.children.iter().find(|c| c.name == name));
    let sprite = player.and_then(|m| m.skin.as_deref()).and_then(|skin| game.gui.atlas.sprites.get(skin));
    let (Some(part), Some(sprite)) = (part, sprite) else {
        return ControlFlow::Continue(());
    };
    let z_rot = if left { -0.1 } else { 0.1 };
    let pose = |p: &Part| {
        let mut pose = p.pose;
        pose[3] = 0.0;
        pose[4] = 0.0;
        pose[5] = if p.name == name { z_rot } else { 0.0 };
        pose
    };
    render_model(part, matrix, &pose, [sprite.u0, sprite.v0, sprite.u1, sprite.v1], light, 0, out);
    ControlFlow::Break(())
}

pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(ID) {
        if ACTIVE.swap(false, Ordering::Relaxed) {
            restore(game);
        }
        return;
    }
    ACTIVE.store(true, Ordering::Relaxed);
    let reach = f64::from(reach(game));
    let camera = game.native_camera().map(|c| c.position);
    let near = |x: f64, y: f64, z: f64| camera.is_none_or(|c| (x - c[0]).powi(2) + (y - c[1]).powi(2) + (z - c[2]).powi(2) <= reach * reach);
    let [head, chest, legs, _] = game.armor.each_ref().map(|s| s.as_ref().map(|s| &*s.item));
    let own = shown(head.is_some(), chest, legs.is_some());
    if let Some(m) = game.player_model.as_mut() {
        let close = near(m.position.x, m.position.y, m.position.z);
        swap(m, close, own);
    }
    for m in game.mobs.iter_mut().filter(|m| &*m.kind.id == voxels::PLAYER) {
        let worn = shown(m.extra.head.is_some(), m.extra.chest.as_deref(), m.extra.legs.is_some());
        let close = near(m.position.x, m.position.y, m.position.z);
        swap(m, close, worn);
    }
    let wanted = voxels::wanted();
    if wanted.is_empty() {
        return;
    }
    let mut missing = MISSING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut logged = LOGGED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let downloads = game.skin_downloads.borrow();
    for location in wanted {
        if let Some(rgba) = downloads.image(&location)
            && let Some((width, height)) = size(rgba.len())
        {
            voxels::offer(&location, width, height, rgba);
            continue;
        }
        if missing.contains(&location) {
            continue;
        }
        // The offline profile's skin, named by its reading (`horizon/offline_skin_<n>`).
        if location.starts_with(OFFLINE_SKIN)
            && let Some(skin) = horizon_client::skins::offline_skin()
            && let Some((width, height)) = size(skin.0.len())
        {
            voxels::offer(&location, width, height, &skin.0);
            continue;
        }
        if location.starts_with("skins/") {
            if logged.len() < MAX_MISSING && logged.insert(location.clone()) {
                log::info!("3D Skins: waiting for the pixels of downloaded skin {location}");
            }
            continue;
        }
        match bundled(game, &location) {
            Some((width, height, rgba)) if size(rgba.len()) == Some((width, height)) => voxels::offer(&location, width, height, &rgba),
            _ => {
                log::info!("3D Skins: no pixels for skin {location}; it keeps its flat layer");
                if missing.len() < MAX_MISSING {
                    missing.insert(location);
                }
            }
        }
    }
}

pub(super) fn listed(mods: &mut Vec<horizon_client::mod_api::NativeMod>) {
    mods.push(horizon_client::mod_api::NativeMod {
        id: ID.into(),
        title: format!("3D Skins {VERSION}"),
        description: DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

pub(super) fn page(game: &Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == ID {
        buttons.push(format!("3D layers within: {} blocks", reach(game)));
    }
}

pub(super) fn click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID || button != 0 {
        return;
    }
    let now = reach(game);
    let next = REACHES[(REACHES.iter().position(|r| *r == now).unwrap_or(0) + 1) % REACHES.len()];
    game.native_set_setting(ID, REACH, &next.to_string());
}
