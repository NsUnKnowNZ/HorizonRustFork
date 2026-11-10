fn handson_delight_screen_press(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    crate::game::handson_delight::press(game, screen, button, canceled);
}
