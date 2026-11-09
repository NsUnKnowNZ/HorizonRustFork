use super::Game;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

const MOD_ID: &str = "smoothie_capes";
const KEY: &str = "animate_elytra";

static ENABLED: OnceLock<AtomicBool> = OnceLock::new();

fn setting(game: &Game<'_>) -> &'static AtomicBool {
    ENABLED.get_or_init(|| {
        AtomicBool::new(game.native_setting(MOD_ID, KEY).as_deref() == Some("true"))
    })
}

pub(super) fn enabled(game: &Game<'_>) -> bool {
    setting(game).load(Ordering::Relaxed)
}

pub(super) fn toggle(game: &Game<'_>) {
    let enabled = !enabled(game);
    game.native_set_setting(MOD_ID, KEY, if enabled { "true" } else { "false" });
    setting(game).store(enabled, Ordering::Relaxed);
}
