// native_hooks::Game::native_settings (a hook point): Carry Block's settings page.
fn carryblock_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::carryblock::ID {
        buttons.extend(game.carryblock_settings());
    }
}
