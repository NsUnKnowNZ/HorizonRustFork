fn handson_delight_screen_release(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::handson_delight::release(game, screen, canceled);
}
