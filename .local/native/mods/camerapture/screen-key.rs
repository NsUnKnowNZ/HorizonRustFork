fn camerapture_screen_key(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    crate::game::camerapture_client::screen_key(game, screen, key, modifiers, canceled);
}
