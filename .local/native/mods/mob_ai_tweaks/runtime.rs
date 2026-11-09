// Mob AI Tweaks, three of the pack's settings. Each looks at the damage a
// mob took since the last tick (`last_damage`, stamped with its age).
use std::collections::HashSet;
use std::sync::Mutex;

const PANIC_RADIUS: f64 = 16.0;
const INK_RADIUS: i32 = 8;
const INK_SECONDS: i32 = 5;

/// The hurts already handled: (mob, age stamp).
static SEEN: Mutex<Option<HashSet<(u32, i64)>>> = Mutex::new(None);

pub(super) fn tick(game: &mut super::Game<'_>) {
    if game.remote.is_some() || game.mobs.is_empty() {
        return;
    }
    let Ok(mut seen) = SEEN.lock() else {
        return;
    };
    let seen = seen.get_or_insert_with(HashSet::new);
    // Fresh hurts: this tick's or the last one's.
    let fresh: Vec<(usize, horizon_client::mobs::Damage, i64)> = game
        .mobs
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let (d, t) = m.last_damage?;
            (i64::from(m.age) - t <= 1 && !seen.contains(&(m.id, t))).then_some((i, d, t))
        })
        .collect();
    if fresh.is_empty() {
        if seen.len() > 4096 {
            seen.clear();
        }
        return;
    }
    let mut commands = Vec::new();
    for (i, damage, t) in fresh {
        let (id, kind, pos, creature) = {
            let m = &game.mobs[i];
            (m.id, m.kind.id.clone(), m.position, m.category() == "creature")
        };
        seen.insert((id, t));
        // animals_panic_in_groups: the same kind of animal nearby panics too
        // (a panic with no one to blame, so nobody gets angry).
        if creature && damage.causes_panic() {
            let calm = horizon_client::mobs::Damage { attacker: None, ..damage };
            for o in game.mobs.iter_mut() {
                if o.id == id || o.kind.id != kind || !o.alive() {
                    continue;
                }
                let (dx, dy, dz) = (o.position.x - pos.x, o.position.y - pos.y, o.position.z - pos.z);
                if dx * dx + dy * dy + dz * dz > PANIC_RADIUS * PANIC_RADIUS {
                    continue;
                }
                let age = i64::from(o.age);
                if o.last_damage.is_some_and(|(_, t)| age - t <= 40) {
                    continue;
                }
                o.last_damage = Some((calm, age));
                seen.insert((o.id, age));
            }
        }
        // creepers_delay_explosion_when_hit: a hit puts a lit fuse back.
        if &*kind == "minecraft:creeper" && damage.attacker.is_some() {
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.swell = (m.swell - 10).max(0);
                m.previous_swell = m.swell;
            }
        }
        // squid_ink_inflicts_debuffs: a hurt squid's ink blinds what is near.
        if matches!(&*kind, "minecraft:squid" | "minecraft:glow_squid") {
            let near = |p: [f64; 3]| {
                let (dx, dy, dz) = (p[0] - pos.x, p[1] - pos.y, p[2] - pos.z);
                dx * dx + dy * dy + dz * dz <= f64::from(INK_RADIUS * INK_RADIUS)
            };
            // Players by name (the console's selectors don't reach a guest).
            for p in game.native_server_players() {
                if near(p.position) && p.health > 0.0 {
                    commands.push(format!(
                        "effect give {} minecraft:blindness {INK_SECONDS} 0",
                        p.name
                    ));
                }
            }
            for o in game.mobs.iter_mut() {
                if o.id != id
                    && !matches!(&*o.kind.id, "minecraft:squid" | "minecraft:glow_squid")
                    && near([o.position.x, o.position.y, o.position.z])
                {
                    o.add_effect_with("minecraft:blindness", INK_SECONDS * 20, 0, false, true);
                }
            }
        }
    }
    for c in commands {
        let r = game.native_run_command_captured(&c);
        if !r.ran {
            log::warn!("[mob_ai_tweaks] `{c}`: {:?}", r.output);
        }
    }
}
