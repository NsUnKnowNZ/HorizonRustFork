// Sounds for Horizon: a sword's swoosh.
use super::Game;
use std::time::Instant;
use std::sync::Mutex;

static LAST: Mutex<Option<Instant>> = Mutex::new(None);

/// The attack key with a sword in hand: a swoosh, at most every quarter second (a held key
/// repeats).
pub(super) fn swing(game: &mut Game<'_>, button: i32) {
    if button != 0 || game.held().is_none_or(|s| !s.item.ends_with("_sword")) {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed().as_millis() < 250) {
        return;
    }
    *last = Some(Instant::now());
    drop(last);
    if let Some(p) = game.player.as_ref() {
        let at = [p.position.x, p.position.y + 1.4, p.position.z];
        game.native_play_local_sound("sounds:item.sword.swoosh", at, 0.6, 1.0, 0);
    }
}
