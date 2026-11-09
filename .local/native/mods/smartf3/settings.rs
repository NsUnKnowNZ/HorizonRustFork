// native_hooks::Game::native_settings (a hook point): SmartF3's settings page.
fn smartf3_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::smartf3::ID {
        buttons.extend(game.smartf3_settings());
    }
}
