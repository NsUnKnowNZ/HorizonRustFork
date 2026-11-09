// Enhanced Attack Indicator, the game side: EnhancedAttackIndicator.getProgress
// with Better OneBlock's settings (weapon cooldown MIDDLE, everything shown).
use super::hud_layout::enhanced_attack_indicator as state;

/// BowItem.getPowerForTime.
fn bow_power(ticks: u32) -> f32 {
    let f = ticks as f32 / 20.0;
    ((f * f + f * 2.0) / 3.0).min(1.0)
}

fn progress(game: &super::Game<'_>) -> Option<f32> {
    // Block breaking: a guest's (and the integrated world's) dig is `mining`,
    // a host's own is the hand's `destroying`.
    if let Some(m) = game.mining.as_ref()
        && m.progress > 0.0
    {
        return Some(m.progress.min(1.0));
    }
    let d = &game.clicks.destroying;
    if d.active && d.progress > 0.0 {
        return Some(d.progress.min(1.0));
    }
    // The item being used: bow, crossbow and trident draws, food and potions.
    if let Some((slot, ticks)) = game.using {
        let stack = game.inventory.get(slot).and_then(Option::as_ref).or(game.offhand.as_ref());
        if let Some(stack) = stack {
            let full = |p: f32| if p >= 1.0 { 2.0 } else { p };
            match &*stack.item {
                "minecraft:bow" => return Some(full(bow_power(ticks))),
                "minecraft:crossbow" => return Some(full(ticks as f32 / 25.0)),
                "minecraft:trident" => return Some(full(ticks as f32 / 10.0)),
                _ => {
                    if game.use_duration > 0 {
                        let p = ticks as f32 / game.use_duration as f32;
                        return Some(if p == 0.0 { 1.0 } else { p.min(1.0) });
                    }
                }
            }
        }
    }
    // MIDDLE: the weapon cooldown, when vanilla would show it.
    let speed = game.main_hand_attack_speed();
    let weapon = game.hand_strength.scale(0.0, speed);
    if weapon < 1.0 {
        return None;
    }
    // Item cooldowns: off hand, then main hand.
    for stack in [game.offhand.as_ref(), game.held()].into_iter().flatten() {
        let c = game.cooldowns.percent(&stack.item, 0.0);
        if c > 0.0 {
            return Some(c);
        }
    }
    None
}

pub(super) fn frame(game: &mut super::Game<'_>) {
    state::set(progress(game));
}
