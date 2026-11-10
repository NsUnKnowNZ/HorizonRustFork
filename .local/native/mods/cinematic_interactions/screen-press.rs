fn cinematic_interactions_screen_press(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    crate::game::cinematic_interactions::press(game, screen, button, modifiers, canceled);
}
