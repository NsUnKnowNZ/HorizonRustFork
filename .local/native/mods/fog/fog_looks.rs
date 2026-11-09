// Fog for Horizon: the overworld's air fog nearer, thicker in caves, closer in rain, brighter at
// dawn and dusk, moonlit at night and gone above the clouds.
use super::{Dim, Game};
use horizon_client::fog::{FogData, FogType};
use std::sync::Mutex;
use std::time::Instant;

/// Where the render-distance fog starts, as a part of where it ends (vanilla's is about 0.9).
const START: f32 = 0.2;
/// The heights where the cave fog begins and is at its thickest.
const CAVE_TOP: f64 = 56.0;
const CAVE_DEEP: f64 = -32.0;
/// The nearest the cave fog comes, in blocks.
const CAVE_END: f32 = 48.0;
/// The cloud layer, and how far above it the fog has gone.
const CLOUDS: f64 = 192.0;
const ABOVE: f64 = 48.0;

/// The cave amount eased over time (0 open sky, 1 deep underground), so it never jumps.
static CAVE: Mutex<Option<(f32, Instant)>> = Mutex::new(None);

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// 1 at `centre` ticks of the day, fading to 0 `width` ticks away (wrapping round the day).
fn bump(time: f32, centre: f32, width: f32) -> f32 {
    let d = (time - centre).rem_euclid(24000.0);
    let d = d.min(24000.0 - d);
    smooth(1.0 - d / width)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub(super) fn render(game: &Game<'_>, camera: [f64; 3], fog_type: &FogType, fog: &mut FogData) {
    if !matches!(fog_type, FogType::Atmospheric) || game.dim != Dim::Overworld || game.title.is_some() {
        return;
    }
    let [x, y, z] = camera;
    let (bx, by, bz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let sky = if game.world.loaded(bx, bz) { f32::from(game.world.light(bx, by, bz) >> 4) } else { 15.0 };
    // Underground: below the cave line and out of the sky's light.
    let depth = ((CAVE_TOP - y) / (CAVE_TOP - CAVE_DEEP)).clamp(0.0, 1.0) as f32;
    let target = smooth(depth) * (1.0 - sky / 15.0);
    let cave = {
        let now = Instant::now();
        let mut held = CAVE.lock().unwrap_or_else(|e| e.into_inner());
        let eased = match *held {
            Some((v, at)) => {
                let k = 1.0 - (-now.duration_since(at).as_secs_f32() / 1.5).exp();
                v + (target - v) * k
            }
            None => target,
        };
        *held = Some((eased, now));
        eased
    };
    let (rain, thunder) = game.view_weather(game.clock.fraction() as f32);
    let time = game.world.day_time.rem_euclid(24000) as f32;
    let moon = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75][(game.world.day_time.div_euclid(24000)).rem_euclid(8) as usize];
    let night = smooth((time - 12800.0) / 1200.0) * smooth((23200.0 - time) / 1200.0);
    let golden = bump(time, 12600.0, 1600.0).max(bump(time, 23400.0, 1600.0)) * (1.0 - rain);

    // Distances: a nearer start, rain closer, caves closer still, none above the clouds. Left
    // alone when Horizon's Fog option is off (the distances are pushed out of sight then).
    let end = fog.render_distance_end;
    let high = (((y - CLOUDS) / ABOVE).clamp(0.0, 1.0)) as f32 * (1.0 - cave);
    if end > 1.0e8 {
        return paint(game, fog, golden, night, moon, rain, cave);
    }
    // The dimension's own fog (26.3's environment fog) thins out above the clouds too.
    let env = smooth(high) * 0.95;
    let env_start = fog.environmental_start + (fog.environmental_end - fog.environmental_start) * env;
    if env_start.is_finite() && fog.environmental_end > env_start + 1.0 {
        fog.environmental_start = env_start;
    }
    let mut start = end * START;
    start *= 1.0 - 0.6 * rain;
    let mut end_now = end * (1.0 - 0.3 * rain - 0.15 * thunder);
    let cave_end = CAVE_END.min(end_now);
    end_now += (cave_end - end_now) * cave;
    start = start.min(end_now * 0.5) * (1.0 - 0.8 * cave);
    start += (end_now - start) * smooth(high) * 0.95;
    if start.is_finite() && end_now.is_finite() && end_now > start + 1.0 {
        fog.render_distance_start = start;
        fog.render_distance_end = end_now;
    }

    paint(game, fog, golden, night, moon, rain, cave);
}

/// Colour: brighter at dawn and dusk, the moon's light at night, greyer in rain, dark in caves.
fn paint(_game: &Game<'_>, fog: &mut FogData, golden: f32, night: f32, moon: f32, rain: f32, cave: f32) {
    let mut rgb = [fog.color[0], fog.color[1], fog.color[2]];
    let grey = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
    let vivid = 1.0 + 0.45 * golden;
    rgb = rgb.map(|c| grey + (c - grey) * vivid);
    rgb = rgb.map(|c| c * (1.0 + 0.12 * golden));
    let moonlit = 0.55 + 0.45 * moon;
    rgb = mix(rgb, rgb.map(|c| c * moonlit), night);
    rgb = mix(rgb, [grey * 0.9; 3], 0.35 * rain);
    rgb = mix(rgb, [0.035, 0.038, 0.05], cave * 0.85);
    let rgb = rgb.map(|c| c.clamp(0.0, 1.0));
    if rgb.iter().all(|c| c.is_finite()) {
        fog.color = [rgb[0], rgb[1], rgb[2], fog.color[3]];
    }
}
