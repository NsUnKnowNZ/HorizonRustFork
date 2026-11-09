// FindMe for Horizon: which storages nearby hold the item under the pointer.
use super::Game;
use horizon_client::world_primitives::WorldPrimitives;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const ID: &str = "findme";
const FIND_KEY: &str = "find";
const DEFAULT_KEY: &str = "key.keyboard.y";
const ASK: &str = "findme:find";
const FOUND: &str = "findme:found";
const RANGE: f64 = 24.0;
const SHOWN: Duration = Duration::from_secs(10);
const COLOR: u32 = 0xFF5C_FF6E;

static FOUND_AT: Mutex<Option<(Vec<[i32; 3]>, Instant)>> = Mutex::new(None);

pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || game.native_playing() || game.player.is_none() {
        return;
    }
    if key != game.native_mod_key(ID, FIND_KEY, DEFAULT_KEY) {
        return;
    }
    let Some(size) = game.renderer.as_ref().map(|r| [r.config.width, r.config.height]) else {
        return;
    };
    let batch = horizon_client::hud::HudBatch::new(size);
    // The stack under the pointer, else the one it carries.
    let slot = game.slot_at(&batch, game.cursor.0, game.cursor.1);
    let stack = slot.and_then(|s| game.slot_view(s)).or_else(|| game.carried.clone());
    let Some(stack) = stack else { return };
    log::info!("[findme] looking for {}", stack.item);
    if let Err(e) = game.native_net_send_to_server(ASK, stack.item.as_bytes()) {
        log::warn!("[findme] {e}");
    }
}

/// The server: the containers near the player holding the item.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != ASK || data.len() > 256 {
        return;
    }
    let Ok(item) = std::str::from_utf8(data) else { return };
    let Some(p) = game.native_server_players().into_iter().find(|p| p.uuid == player) else {
        return;
    };
    let dim = super::Dim::from_id(&p.dimension);
    let world = if dim == game.dim { Some(&game.world) } else { game.levels.parked_world(dim) };
    let Some(world) = world else { return };
    let [px, py, pz] = p.position;
    let mut found = Vec::new();
    for (&(x, y, z), entity) in &world.block_entities {
        let d = (f64::from(x) + 0.5 - px).powi(2)
            + (f64::from(y) + 0.5 - py).powi(2)
            + (f64::from(z) + 0.5 - pz).powi(2);
        if d > RANGE * RANGE {
            continue;
        }
        let holds = match entity {
            horizon_client::containers::BlockEntity::Chest(items) => {
                items.iter().flatten().any(|s| &*s.item == item)
            }
            _ => false,
        };
        if holds && found.len() < 256 {
            found.push([x, y, z]);
        }
    }
    let mut out = Vec::with_capacity(found.len() * 12);
    for [x, y, z] in &found {
        for v in [x, y, z] {
            out.extend_from_slice(&v.to_be_bytes());
        }
    }
    log::info!("[findme] {item}: {} storages for {}", found.len(), p.name);
    let _ = game.native_net_send_to_player(player, FOUND, &out);
}

pub(super) fn from_server(channel: &str, data: &[u8]) {
    if channel != FOUND {
        return;
    }
    let found = data
        .chunks_exact(12)
        .map(|c| {
            let i = |k: usize| i32::from_be_bytes([c[k], c[k + 1], c[k + 2], c[k + 3]]);
            [i(0), i(4), i(8)]
        })
        .collect();
    *FOUND_AT.lock().unwrap_or_else(|e| e.into_inner()) = Some((found, Instant::now()));
}

pub(super) fn frame(primitives: &mut WorldPrimitives) {
    let mut guard = FOUND_AT.lock().unwrap_or_else(|e| e.into_inner());
    let Some((found, since)) = guard.as_ref() else { return };
    if since.elapsed() > SHOWN {
        *guard = None;
        return;
    }
    // Blinks for the first two seconds, as FindMe's highlight does.
    let t = since.elapsed().as_secs_f32();
    if t < 2.0 && (t * 4.0) as u32 % 2 == 1 {
        return;
    }
    for &[x, y, z] in found {
        let (x, y, z) = (f64::from(x), f64::from(y), f64::from(z));
        primitives
            .box_outline([x - 0.01, y - 0.01, z - 0.01], [x + 1.01, y + 1.01, z + 1.01], COLOR, 3.0)
            .through_walls();
    }
}
