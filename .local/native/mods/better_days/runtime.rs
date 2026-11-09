// Better Days for Horizon: the sleep feature. Config: horizon-mods/.horizon-mod-data/better_days/config.json
use std::sync::{Mutex, OnceLock};

const CONFIG: &str = "horizon-mods/.horizon-mod-data/better_days/config.json";
const DEFAULT_CONFIG: &str = "{\n  \"enableSleepFeature\": true,\n  \"sleepSpeedMin\": 1.0,\n  \"sleepSpeedMax\": 110.0,\n  \"sleepSpeedCurve\": 0.3,\n  \"sleepSpeedAll\": -1.0,\n  \"ratioPlayersForSleep\": 0.0,\n  \"dayStart\": 23500,\n  \"nightStart\": 12500\n}\n";

pub struct Config {
    pub enabled: bool,
    pub min: f64,
    pub max: f64,
    pub curve: f64,
    pub all: f64,
    pub ratio_needed: f64,
    pub day_start: i64,
    pub night_start: i64,
}

pub fn config() -> &'static Config {
    static CELL: OnceLock<Config> = OnceLock::new();
    CELL.get_or_init(|| {
        let path = std::path::Path::new(CONFIG);
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, DEFAULT_CONFIG);
            DEFAULT_CONFIG.to_owned()
        });
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let f = |k: &str, d: f64| v[k].as_f64().unwrap_or(d);
        Config {
            enabled: v["enableSleepFeature"].as_bool().unwrap_or(true),
            min: f("sleepSpeedMin", 1.0),
            max: f("sleepSpeedMax", 110.0),
            curve: f("sleepSpeedCurve", 0.3),
            all: f("sleepSpeedAll", -1.0),
            ratio_needed: f("ratioPlayersForSleep", 0.0),
            day_start: v["dayStart"].as_i64().unwrap_or(23500),
            night_start: v["nightStart"].as_i64().unwrap_or(12500),
        }
    })
}

/// Between nightStart and dayStart: the time the sleep feature speeds up.
pub fn night(day_time: i64) -> bool {
    let c = config();
    let t = day_time.rem_euclid(24000);
    c.enabled && t >= c.night_start && t < c.day_start
}

/// MathUtils.normalizedTunableSigmoid.
fn sigmoid(x: f64, c: f64) -> f64 {
    c * x / (2.0 * c * x - c - x + 1.0)
}

/// TimeService.getSleepTimeSpeed.
pub fn sleep_speed(sleeping: usize, active: usize) -> f64 {
    let c = config();
    if sleeping == 0 || active == 0 {
        return 1.0;
    }
    let ratio = sleeping as f64 / active as f64;
    if ratio < c.ratio_needed {
        return 1.0;
    }
    if sleeping == active && c.all >= 0.0 {
        return c.all;
    }
    c.min + sigmoid(ratio, c.curve) * (c.max - c.min)
}

static CARRY: Mutex<f64> = Mutex::new(0.0);

pub(super) fn tick(game: &mut super::Game<'_>) {
    if game.remote.is_some() || !night(game.world.day_time) {
        return;
    }
    let Some(host) = game.lan.as_ref() else { return };
    let players = host.hub.guests.iter().filter(|g| g.placed && g.game_mode != 3 && g.dimension == "minecraft:overworld");
    let (mut active, mut sleeping) = (0, 0);
    for g in players {
        active += 1;
        if g.bed.is_some() {
            sleeping += 1;
        }
    }
    let speed = sleep_speed(sleeping, active);
    if speed <= 1.0 {
        return;
    }
    // The vanilla tick adds one; the rest, whole ticks, carried over.
    let mut carry = CARRY.lock().unwrap_or_else(|e| e.into_inner());
    *carry += speed - 1.0;
    let mut add = carry.floor() as i64;
    *carry -= add as f64;
    // Not past the morning: vanilla wakes the sleepers there.
    let t = game.world.day_time.rem_euclid(24000);
    add = add.min(config().day_start - t);
    if add > 0 {
        let _ = game.native_run_command_captured(&format!("time add {add}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sigmoid_ends() {
        assert!((sigmoid(1.0, 0.3) - 1.0).abs() < 1e-9);
        assert!(sigmoid(0.0, 0.3).abs() < 1e-9);
    }
}
