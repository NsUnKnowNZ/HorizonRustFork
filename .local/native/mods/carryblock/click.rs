// native_hooks::Game::native_settings_click (a hook point): a click on Carry
// Along's settings page changes the setting, keeps it and applies it.
fn carryblock_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::carryblock::ID {
        game.carryblock_settings_click(button);
    }
}
