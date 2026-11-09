// Enhanced Visuals for Horizon: red edges on damage and at low health, with a heartbeat.
use super::Game;
use std::sync::Mutex;
use std::time::Instant;

struct State {
    health: Option<f32>,
    max: f32,
    /// The damage flash, 1 just hit, fading to 0.
    flash: f32,
    last: Option<Instant>,
    /// Seconds into the heartbeat.
    beat: f32,
}

static STATE: Mutex<State> =
    Mutex::new(State { health: None, max: 20.0, flash: 0.0, last: None, beat: 0.0 });

pub(super) fn observe(health: f32, max: f32) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = s.health
        && health < old - 0.01
        && health > 0.0
    {
        let lost = old - health;
        s.flash = (s.flash + 0.45 + lost / max.max(1.0) * 2.0).min(1.0);
    }
    s.health = Some(health);
    s.max = max.max(1.0);
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let now = Instant::now();
    let (intensity, heartbeat) = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let dt = s.last.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.25);
        s.last = Some(now);
        s.flash = (s.flash - dt * 1.1).max(0.0);
        let ratio = s.health.map_or(1.0, |h| h / s.max);
        let mut pulse = 0.0;
        let mut heartbeat = false;
        if ratio < 0.3 && ratio > 0.0 {
            // A beat every 1.2 s at 30%, every 0.55 s near death.
            let period = 0.55 + ratio / 0.3 * 0.65;
            s.beat += dt;
            if s.beat >= period {
                s.beat -= period;
                heartbeat = true;
            }
            let phase = s.beat / period;
            // A quick double thump at the start of the beat.
            let thump = (-(phase * 9.0)).exp() + 0.6 * (-((phase - 0.22).max(0.0) * 9.0)).exp();
            pulse = (0.25 + 0.45 * thump) * (1.0 - ratio / 0.3 * 0.5);
        } else {
            s.beat = 0.0;
        }
        ((s.flash * 0.85 + pulse).min(0.95), heartbeat)
    };
    if heartbeat && let Some(p) = game.player.as_ref() {
        let at = [p.position.x, p.position.y, p.position.z];
        game.native_play_local_sound("minecraft:entity.warden.heartbeat", at, 0.7, 1.0, 0);
    }
    if intensity <= 0.01 {
        return;
    }
    // The vignette blend keeps `1 - colour` of what is under its bright edges: taking away
    // green and blue leaves them red.
    let k = (intensity * 255.0) as u8;
    let color = horizon_client::hud::rgba(0, k, k, 255);
    // `hud_layout::paint` leaves vignette blits to the screen effects pass, so the texture is
    // drawn into the batch here with the vignette blend (as the win screen draws its own).
    let atlas = &game.gui.atlas;
    let Some(r) = atlas.sprites.get("misc/vignette").copied() else {
        return;
    };
    let (w, h) = (batch.width, batch.height);
    let start = batch.vertices.len();
    batch.rect_uv(0.0, 0.0, w, h, r, color);
    batch.blend_since(start, horizon_client::hud::Blend::Vignette);
}
