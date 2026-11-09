// Yori3o's Grappling Hooks for Horizon: cast, hang, swing, climb and jump off.
use super::Game;
use horizon_client::actions::Action;
use std::sync::Mutex;

/// The hook out: where it holds, the chain's length, the hook's item and the ticks since cast.
#[derive(Clone)]
struct Hook {
    anchor: [f64; 3],
    block: [i32; 3],
    length: f64,
    range: f64,
    climb: f64,
    item: String,
    age: u32,
}

struct State {
    hook: Option<Hook>,
    /// Ticks to wait before the use key acts again (a held key repeats).
    cooldown: u32,
    /// The player's position last tick, for the chain's start between ticks.
    last: Option<[f64; 3]>,
}

static STATE: Mutex<State> = Mutex::new(State { hook: None, cooldown: 0, last: None });

fn range(item: &str) -> Option<f64> {
    let path = item.strip_prefix("yo_hooks:")?.strip_suffix("_grappling_hook")?;
    Some(match path {
        "copper" => 16.0,
        "iron" => 24.0,
        "gold" => 28.0,
        "diamond" => 32.0,
        "netherite" => 40.0,
        _ => return None,
    })
}

fn held_hook(game: &Game<'_>) -> Option<(String, f64, f64)> {
    let stack = game.held()?;
    let base = range(&stack.item)?;
    let reach = f64::from(stack.enchantment_level("yo_hooks:long_reach")) * 8.0;
    let climb = 0.18 + 0.06 * f64::from(stack.enchantment_level("yo_hooks:agility"));
    Some((stack.item.to_string(), base + reach, climb))
}

fn centre(game: &Game<'_>) -> Option<[f64; 3]> {
    let p = game.player.as_ref()?;
    Some([p.position.x, p.position.y + 0.9, p.position.z])
}

fn sound(game: &mut Game<'_>, name: &str) {
    if let Some(at) = centre(game) {
        game.native_play_local_sound(name, at, 0.8, 1.0, 0);
    }
}

/// The use key with a grappling hook: cast it, or let go.
pub(super) fn use_key(game: &mut Game<'_>, button: i32, swing_hand: &mut bool, canceled: &mut bool) {
    if button != 1 || *canceled {
        return;
    }
    let Some((item, range, climb)) = held_hook(game) else { return };
    *canceled = true;
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.cooldown > 0 {
        *swing_hand = false;
        return;
    }
    state.cooldown = 8;
    if state.hook.take().is_some() {
        // Let go with a jump off the chain.
        drop(state);
        if let Some(p) = game.player.as_mut() {
            p.velocity.y = p.velocity.y.max(0.0) + 0.55;
        }
        sound(game, "yo_hooks:back");
        return;
    }
    drop(state);
    let Some(hit) = game.native_raycast(range as f32) else {
        sound(game, "minecraft:entity.fishing_bobber.throw");
        return;
    };
    let Some(from) = centre(game) else { return };
    let anchor = [hit.x, hit.y, hit.z];
    let length = (0..3).map(|i| (anchor[i] - from[i]).powi(2)).sum::<f64>().sqrt();
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.hook = Some(Hook {
        anchor,
        block: [hit.block_x, hit.block_y, hit.block_z],
        length: length.max(1.5),
        range,
        climb,
        item,
        age: 0,
    });
    drop(state);
    sound(game, "yo_hooks:cast");
}

/// Before the player moves: hold them on the chain, climb, and let go when the hook can't hold.
pub(super) fn tick(game: &mut Game<'_>) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.cooldown = state.cooldown.saturating_sub(1);
    state.last = centre(game);
    let Some(mut hook) = state.hook.clone() else { return };
    let Some(at) = centre(game) else {
        state.hook = None;
        return;
    };
    let held = held_hook(game).is_some_and(|h| h.0 == hook.item);
    let [bx, by, bz] = hook.block;
    let block_gone = game.world.loaded(bx, bz) && game.res.facts().is_air(game.world.block(bx, by, bz));
    let d = [hook.anchor[0] - at[0], hook.anchor[1] - at[1], hook.anchor[2] - at[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !held || block_gone || dist > hook.range * 1.5 || game.player.is_none() {
        state.hook = None;
        drop(state);
        sound(game, "yo_hooks:back");
        return;
    }
    hook.age += 1;
    // Climbing: jump shortens the chain, sneak lets it out.
    if game.keys.held(Action::Jump) {
        hook.length = (hook.length - hook.climb).max(1.2);
    } else if game.keys.held(Action::Sneak) {
        hook.length = (hook.length + hook.climb).min(hook.range);
    }
    let length = hook.length;
    state.hook = Some(hook);
    drop(state);
    let Some(p) = game.player.as_mut() else { return };
    p.fall_distance = 0.0;
    if dist <= length || dist < 1e-6 {
        return;
    }
    // Past the chain's length: the part of the velocity pulling away goes, and the player is
    // drawn back onto the chain's sphere (a swing under the hook).
    let n = [d[0] / dist, d[1] / dist, d[2] / dist];
    let v = [p.velocity.x, p.velocity.y, p.velocity.z];
    let away = -(v[0] * n[0] + v[1] * n[1] + v[2] * n[2]);
    let pull = ((dist - length) * 0.35).min(0.6);
    let k = away.max(0.0) + pull;
    p.velocity.x += n[0] * k;
    p.velocity.y += n[1] * k;
    p.velocity.z += n[2] * k;
    // A little drag, so a swing settles.
    p.velocity.x *= 0.995;
    p.velocity.z *= 0.995;
}

/// The chain, from the player's hand to the hook.
pub(super) fn draw(game: &Game<'_>, partial: f32, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(hook) = &state.hook else { return };
    let Some(now) = centre(game) else { return };
    let last = state.last.unwrap_or(now);
    let t = f64::from(partial.clamp(0.0, 1.0));
    let at = [0, 1, 2].map(|i| last[i] + (now[i] - last[i]) * t);
    let yaw = f64::from(game.yaw).to_radians();
    // The right hand: a little to the right of and in front of the body.
    let hand = [at[0] - yaw.cos() * 0.35 - yaw.sin() * 0.3, at[1] + 0.35, at[2] - yaw.sin() * 0.35 + yaw.cos() * 0.3];
    primitives.line(hand, hook.anchor, 0xFF5A5A5A, 2.5);
    let a = hook.anchor;
    primitives.box_outline([a[0] - 0.12, a[1] - 0.12, a[2] - 0.12], [a[0] + 0.12, a[1] + 0.12, a[2] + 0.12], 0xFF8A8A8A, 2.0);
}
