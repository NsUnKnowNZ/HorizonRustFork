// Forgiving Void for Horizon. Config: horizon-mods/.horizon-mod-data/forgiving_void/config.json
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const CONFIG: &str = "horizon-mods/.horizon-mod-data/forgiving_void/config.json";
const DEFAULT_CONFIG: &str = r#"{
  "triggerAtDistanceBelow": 32,
  "fallingHeight": 300,
  "damageOnFall": 19.9,
  "returnToLastGrounded": true,
  "triggerInOverworld": true,
  "triggerInNether": true,
  "triggerInEnd": true,
  "blindnessTicks": 60,
  "blindnessAmplifier": 3,
  "tamedAnimals": true,
  "entityAllowList": [
    "minecraft:wolf", "minecraft:cat", "minecraft:parrot", "minecraft:horse",
    "minecraft:donkey", "minecraft:mule", "minecraft:llama", "minecraft:camel",
    "minecraft:allay", "minecraft:villager", "minecraft:cow", "minecraft:sheep",
    "minecraft:pig", "minecraft:chicken"
  ]
}
"#;

pub struct Config {
    pub trigger_below: f64,
    pub falling_height: f64,
    pub damage: f64,
    pub last_grounded: bool,
    pub overworld: bool,
    pub nether: bool,
    pub end: bool,
    pub blind_ticks: i64,
    pub blind_amp: i64,
    /// Mobs saved too: tamed ones, and these kinds.
    pub tamed: bool,
    pub allow: Vec<String>,
}

pub fn parse(text: &str) -> Config {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    let f = |k: &str, d: f64| v[k].as_f64().unwrap_or(d);
    let b = |k: &str| v[k].as_bool().unwrap_or(true);
    Config {
        trigger_below: f("triggerAtDistanceBelow", 32.0),
        falling_height: f("fallingHeight", 300.0),
        damage: f("damageOnFall", 19.9).max(0.0),
        last_grounded: b("returnToLastGrounded"),
        overworld: b("triggerInOverworld"),
        nether: b("triggerInNether"),
        end: b("triggerInEnd"),
        blind_ticks: v["blindnessTicks"].as_i64().unwrap_or(60),
        blind_amp: v["blindnessAmplifier"].as_i64().unwrap_or(3),
        tamed: v["tamedAnimals"].as_bool().unwrap_or(true),
        allow: v["entityAllowList"]
            .as_array()
            .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    }
}

fn config() -> &'static Config {
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
        parse(&text)
    })
}

/// The lowest y of a vanilla dimension; None where the mod is off.
pub fn bottom(c: &Config, dimension: &str) -> Option<f64> {
    match dimension {
        "minecraft:overworld" if c.overworld => Some(-64.0),
        "minecraft:the_nether" if c.nether => Some(0.0),
        "minecraft:the_end" if c.end => Some(0.0),
        _ => None,
    }
}

#[derive(Default)]
struct Player {
    last_ground: Option<[f64; 2]>,
    falling_ticks: Option<u32>,
}

static PLAYERS: Mutex<Option<HashMap<u128, Player>>> = Mutex::new(None);

/// Allow-listed mobs that fall out of the world drop back in from `fallingHeight`, slow
/// falling so they land unhurt.
fn save_mobs(game: &mut super::Game<'_>, c: &Config) {
    if game.remote.is_some() || (c.allow.is_empty() && !c.tamed) {
        return;
    }
    let Some(bottom) = bottom(c, game.dim.id()) else { return };
    static GROUND: Mutex<Option<HashMap<u32, [f64; 2]>>> = Mutex::new(None);
    let mut ground = GROUND.lock().unwrap_or_else(|e| e.into_inner());
    let ground = ground.get_or_insert_with(HashMap::new);
    if ground.len() > 4096 {
        ground.clear();
    }
    for m in game.mobs.iter_mut() {
        if m.on_ground {
            ground.insert(m.id, [m.position.x, m.position.z]);
        }
        if !m.alive() || m.position.y >= bottom - c.trigger_below {
            continue;
        }
        let allowed = (c.tamed && m.extra.tame) || c.allow.iter().any(|k| **k == *m.kind.id);
        if !allowed {
            continue;
        }
        let [x, z] = match ground.get(&m.id) {
            Some(xz) if c.last_grounded => *xz,
            _ => [m.position.x, m.position.z],
        };
        m.position = horizon_core::geometry::Vec3::new(x, c.falling_height, z);
        m.previous = m.position;
        m.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
        m.fall_distance = 0.0;
        m.add_effect_with("minecraft:slow_falling", 20 * 120, 0, false, true);
        log::info!("[forgiving_void] {} {} falls back from y={}", m.kind.id, m.id, c.falling_height);
    }
}

pub(super) fn tick(game: &mut super::Game<'_>) {
    save_mobs(game, config());
    let players = game.native_server_players();
    if players.is_empty() {
        return;
    }
    let c = config();
    let mut commands = Vec::new();
    {
        let mut all = PLAYERS.lock().unwrap_or_else(|e| e.into_inner());
        let all = all.get_or_insert_with(HashMap::new);
        for p in &players {
            let me = all.entry(p.uuid).or_default();
            if let Some(t) = me.falling_ticks.as_mut() {
                *t += 1;
                // Landed (after the teleport has settled), or died / left.
                if p.on_ground && *t > 5 {
                    me.falling_ticks = None;
                    horizon_client::motion::forgiving_void_state::set_falling(p.entity_id, false);
                    if c.damage > 0.0 && p.game_mode != 1 && p.game_mode != 3 {
                        commands.push(format!("damage {} {} minecraft:fall", p.name, c.damage));
                    }
                } else if p.health <= 0.0 || *t > 20 * 60 {
                    me.falling_ticks = None;
                    horizon_client::motion::forgiving_void_state::set_falling(p.entity_id, false);
                }
                continue;
            }
            if p.on_ground {
                me.last_ground = Some([p.position[0], p.position[2]]);
            }
            let Some(bottom) = bottom(c, &p.dimension) else { continue };
            if p.game_mode == 3 || p.health <= 0.0 || p.position[1] >= bottom - c.trigger_below {
                continue;
            }
            let [x, z] = match (c.last_grounded, me.last_ground) {
                (true, Some(xz)) => xz,
                _ => [p.position[0], p.position[2]],
            };
            me.falling_ticks = Some(0);
            horizon_client::motion::forgiving_void_state::set_falling(p.entity_id, true);
            commands.push(format!(
                "execute in {} run tp {} {} {} {}",
                p.dimension, p.name, x, c.falling_height, z
            ));
            if c.blind_ticks > 0 {
                commands.push(format!(
                    "effect give {} minecraft:blindness {} {} true",
                    p.name,
                    (c.blind_ticks + 19) / 20,
                    c.blind_amp
                ));
            }
        }
    }
    for line in commands {
        let r = game.native_run_command_captured(&line);
        if !r.ran {
            log::warn!("[forgiving_void] `{line}` failed: {:?}", r.output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_bottoms() {
        let c = parse(DEFAULT_CONFIG);
        assert_eq!(c.falling_height, 300.0);
        assert_eq!(bottom(&c, "minecraft:overworld"), Some(-64.0));
        assert_eq!(bottom(&c, "minecraft:the_end"), Some(0.0));
        assert_eq!(bottom(&c, "modded:dim"), None);
    }
}
