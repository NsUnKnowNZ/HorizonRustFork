fn cinematic_interactions_screen_key(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    let _ = modifiers;
    crate::game::cinematic_interactions::key(game, screen, key, canceled);
}
