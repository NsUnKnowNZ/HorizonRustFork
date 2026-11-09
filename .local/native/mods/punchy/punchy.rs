// Punchy! for Horizon: the first-person hands sway, bob, sprint, fall and punch.
use super::Game;
use horizon_client::first_person::Hand;
use std::sync::Mutex;
use std::time::Instant;

/// Eased per frame: how far the hands lag the camera, the walk's phase and strength, and the
/// sprint and fall poses.
struct Motion {
    at: Option<Instant>,
    yaw: f32,
    pitch: f32,
    lag: [f32; 2],
    phase: f32,
    walk: f32,
    sprint: f32,
    fall: f32,
}

static MOTION: Mutex<Motion> = Mutex::new(Motion {
    at: None,
    yaw: 0.0,
    pitch: 0.0,
    lag: [0.0; 2],
    phase: 0.0,
    walk: 0.0,
    sprint: 0.0,
    fall: 0.0,
});

fn ease(value: &mut f32, target: f32, rate: f32, dt: f32) {
    *value += (target - *value) * (1.0 - (-rate * dt).exp());
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pose(
    game: &Game<'_>,
    hand: Hand,
    swing: f32,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    roll: &mut f32,
) {
    let Some(p) = game.player.as_ref() else { return };
    let mut m = MOTION.lock().unwrap_or_else(|e| e.into_inner());
    // The main hand's call moves the clock; the off hand reads the same motion.
    if matches!(hand, Hand::Main) {
        let now = Instant::now();
        let dt = m.at.map_or(0.0, |t| now.duration_since(t).as_secs_f32()).min(0.1);
        m.at = Some(now);
        if dt > 0.0 {
            let turn = ((game.yaw - m.yaw + 540.0).rem_euclid(360.0)) - 180.0;
            let tilt = game.pitch - m.pitch;
            // Degrees a second, as a lag the hands ease into and out of.
            let target = [(turn / dt * 0.004).clamp(-1.0, 1.0), (tilt / dt * 0.004).clamp(-1.0, 1.0)];
            ease(&mut m.lag[0], target[0], 10.0, dt);
            ease(&mut m.lag[1], target[1], 10.0, dt);
            let speed = (p.velocity.x * p.velocity.x + p.velocity.z * p.velocity.z).sqrt() as f32;
            let walking = if p.ground.on_ground { (speed / 0.2).min(1.5) } else { 0.0 };
            ease(&mut m.walk, walking, 6.0, dt);
            m.phase = (m.phase + dt * (6.0 + 6.0 * speed / 0.28)) % std::f32::consts::TAU;
            ease(&mut m.sprint, if speed > 0.25 && p.ground.on_ground { 1.0 } else { 0.0 }, 5.0, dt);
            ease(&mut m.fall, (-(p.velocity.y as f32) - 0.3).clamp(0.0, 1.5), 4.0, dt);
        }
        m.yaw = game.yaw;
        m.pitch = game.pitch;
    }
    let side = if matches!(hand, Hand::Main) { 1.0 } else { -1.0 };
    // Lag: turning right leaves the hands to the left, looking up leaves them low.
    *x -= m.lag[0] * 0.06;
    *y += m.lag[1] * 0.05;
    *roll -= m.lag[0] * 6.0;
    // Walking: a side to side swing, the hands out of step with each other.
    let s = m.phase.sin();
    *x += s * 0.018 * m.walk * side;
    *y += (m.phase * 2.0).cos().abs() * 0.012 * m.walk;
    *roll += s * 2.0 * m.walk;
    // Sprinting: lower and tipped forward.
    *y -= 0.05 * m.sprint;
    *pitch -= 10.0 * m.sprint;
    // Falling: the hands rise.
    *y += 0.06 * m.fall;
    // Punching: a thrust forward with the swing.
    let punch = (swing.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    *z -= punch * 0.06;
    *pitch -= punch * 5.0;
}
