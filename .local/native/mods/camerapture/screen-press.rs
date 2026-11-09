fn camerapture_screen_press(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    crate::game::camerapture_client::screen_press(game, screen, button, canceled);
}
