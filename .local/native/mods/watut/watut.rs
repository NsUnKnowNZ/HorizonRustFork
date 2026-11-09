// What Are They Up To for Horizon: what the other players are doing, over their heads.
use super::{Game, Screen};
use horizon_client::world_primitives::WorldPrimitives;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const MINE: &str = "watut:state";
const ALL: &str = "watut:states";
/// Seconds without moving or turning before a player counts as away.
const AWAY_AFTER: f32 = 60.0;

const NONE: u8 = 0;
const TYPING: u8 = 1;
const INVENTORY: u8 = 2;
const MENU: u8 = 3;
const PAUSED: u8 = 4;
const AWAY: u8 = 5;

struct Local {
    sent: u8,
    sent_at: Option<Instant>,
    last_pose: [f64; 5],
    still_since: Instant,
}

static LOCAL: Mutex<Option<Local>> = Mutex::new(None);
/// The server: each player's state.
static SERVER: Mutex<Option<HashMap<u128, u8>>> = Mutex::new(None);
/// A client: the others' states.
static OTHERS: Mutex<Option<HashMap<u128, u8>>> = Mutex::new(None);

fn label(state: u8, t: f32) -> Option<String> {
    let dots = ".".repeat(1 + (t * 2.5) as usize % 3);
    Some(match state {
        TYPING => format!("\u{270e} typing{dots}"),
        INVENTORY => "\u{25a3} in inventory".to_owned(),
        MENU => "\u{25a3} in a menu".to_owned(),
        PAUSED => "\u{23f8} paused".to_owned(),
        AWAY => format!("Zz away{dots}"),
        _ => return None,
    })
}

/// A client tells the server what its player is doing when it changes (and every 5 s).
pub(super) fn tick(game: &mut Game<'_>) {
    if game.remote.is_none() {
        return;
    }
    let Some(p) = game.player.as_ref() else { return };
    let pose = [p.position.x, p.position.y, p.position.z, f64::from(game.yaw), f64::from(game.pitch)];
    let now = Instant::now();
    let mut guard = LOCAL.lock().unwrap_or_else(|e| e.into_inner());
    let local = guard.get_or_insert_with(|| Local { sent: 255, sent_at: None, last_pose: pose, still_since: now });
    if pose.iter().zip(local.last_pose).any(|(a, b)| (a - b).abs() > 1e-3) {
        local.last_pose = pose;
        local.still_since = now;
    }
    let state = match game.screen {
        Screen::Chat => TYPING,
        Screen::Inventory | Screen::Creative => INVENTORY,
        Screen::Paused => PAUSED,
        Screen::Playing if now.duration_since(local.still_since).as_secs_f32() > AWAY_AFTER => AWAY,
        Screen::Playing => NONE,
        _ => MENU,
    };
    let due = local.sent_at.is_none_or(|t| now.duration_since(t).as_secs() >= 5);
    if state == local.sent && !due {
        return;
    }
    local.sent = state;
    local.sent_at = Some(now);
    drop(guard);
    let _ = game.native_net_send_to_server(MINE, &[state]);
}

/// The server: a player's state goes to everyone.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != MINE || data.len() != 1 || data[0] > AWAY {
        return;
    }
    let players = game.native_server_players();
    let out = {
        let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(player, data[0]);
        map.retain(|u, _| players.iter().any(|p| p.uuid == *u));
        let mut out = Vec::with_capacity(map.len() * 17);
        for (u, s) in map.iter() {
            out.extend_from_slice(&u.to_be_bytes());
            out.push(*s);
        }
        out
    };
    for p in &players {
        let _ = game.native_net_send_to_player(p.uuid, ALL, &out);
    }
}

pub(super) fn from_server(channel: &str, data: &[u8]) {
    if channel != ALL {
        return;
    }
    let map = data
        .chunks_exact(17)
        .map(|c| (u128::from_be_bytes(c[..16].try_into().unwrap()), c[16]))
        .collect();
    *OTHERS.lock().unwrap_or_else(|e| e.into_inner()) = Some(map);
}

/// Over each other player's head, what they are doing.
pub(super) fn frame(game: &Game<'_>, partial: f32, primitives: &mut WorldPrimitives) {
    let guard = OTHERS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(map) = guard.as_ref() else { return };
    if map.values().all(|&s| s == NONE) {
        return;
    }
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let t = START.get_or_init(Instant::now).elapsed().as_secs_f32();
    for e in game.native_client_entities() {
        let Some(uuid) = e.player_uuid else { continue };
        if !e.alive || e.invisible {
            continue;
        }
        let Some(text) = map.get(&uuid).and_then(|&s| label(s, t)) else { continue };
        let [x, y, z] = e.position_at(f64::from(partial));
        primitives
            .text([x, y + f64::from(e.height) + 0.9, z], text, 0xFFFF_FFFF, 0.9)
            .background();
    }
}
