// native_hooks::Game::native_settings_click (a hook point): a click on
// SmartF3's settings page changes the setting, keeps it and applies it.
fn smartf3_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::smartf3::ID {
        game.smartf3_settings_click(button);
    }
}
