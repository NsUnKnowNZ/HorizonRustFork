// Cinematic Zoom for Horizon, written from its documented behaviour (none of its code): hold C
// to zoom with a smooth glide in and out, the cinematic (smoothed) camera while zoomed, black
// letterbox bars that slide in, and the mouse wheel to zoom further or less (2x to 10x).
use super::Game;
use horizon_client::{hud::HudBatch, mod_api::NativeZoom};
use std::{sync::Mutex, time::Instant};

pub const ID: &str = "cinematiczoom";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str = "Hold C to zoom with a smooth glide, a cinematic camera and letterbox bars; the mouse wheel sets how far.";
pub const KEY: &str = "zoom";
pub const DEFAULT_KEY: &str = "key.keyboard.c";
/// The zoom the wheel starts from, and its range.
const START: f32 = 4.0;
const MIN: f32 = 2.0;
const MAX: f32 = 10.0;
/// How fast the zoom and the bars follow (per second, exponential).
const GLIDE: f32 = 8.0;
/// The bars' height at full zoom, a share of the screen's.
const BARS: f32 = 0.11;

struct State {
    held: bool,
    target: f32,
    /// The zoom and the bars (0 to 1) as drawn, and when they were last moved.
    now: f32,
    bars: f32,
    at: Option<Instant>,
}
static STATE: Mutex<State> = Mutex::new(State { held: false, target: START, now: 1.0, bars: 0.0, at: None });
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Moves the drawn zoom and bars toward where they are going, once a frame.
fn glide(s: &mut State) {
    let now = Instant::now();
    let dt = s.at.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
    s.at = Some(now);
    let k = 1.0 - (-GLIDE * dt).exp();
    let (zoom, bars) = if s.held { (s.target, 1.0) } else { (1.0, 0.0) };
    // In the log of the zoom, so the glide looks even at every level.
    s.now = (s.now.ln() + (zoom.ln() - s.now.ln()) * k).exp();
    s.bars += (bars - s.bars) * k;
    if (s.now - zoom).abs() < 0.002 {
        s.now = zoom;
    }
}

/// `native_key`: C held zooms.
pub(crate) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) || key != game.native_mod_key(ID, KEY, DEFAULT_KEY) {
        return;
    }
    let playing = game.native_playing();
    state(|s| s.held = pressed && playing);
}

/// `native_zoom`: the gliding zoom, with the cinematic camera while it is in.
pub(crate) fn zoom(game: &Game<'_>, zoom: &mut Option<NativeZoom>) {
    if zoom.is_some() || !game.native_on(ID) {
        return;
    }
    let (now, held) = state(|s| {
        glide(s);
        (s.now, s.held)
    });
    if now > 1.001 {
        *zoom = Some(NativeZoom { factor: now, smooth: held, turn: (1.0 / now).sqrt().max(0.2) });
    }
}

/// `native_wheel`: while zoomed, the wheel sets how far.
pub(crate) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if !game.native_on(ID) || !state(|s| s.held) || steps == 0.0 {
        return;
    }
    *claimed = true;
    state(|s| s.target = (s.target * 1.25f32.powf(steps)).clamp(MIN, MAX));
}

/// `native_hud`: the letterbox bars.
pub(crate) fn hud(game: &Game<'_>, batch: &mut HudBatch) {
    let bars = state(|s| {
        glide(s);
        if !s.held && s.bars < 0.01 {
            s.bars = 0.0;
        }
        s.bars
    });
    if bars <= 0.0 || !game.native_on(ID) {
        return;
    }
    let h = (batch.height * BARS * bars).round();
    game.native_fill(batch, [0.0, 0.0, batch.width, h], 0xFF00_0000);
    game.native_fill(batch, [0.0, batch.height - h, batch.width, h], 0xFF00_0000);
}
