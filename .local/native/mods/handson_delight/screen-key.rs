fn handson_delight_screen_key(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    let _ = modifiers;
    crate::game::handson_delight::key(game, screen, key, canceled);
}
