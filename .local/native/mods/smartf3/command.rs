// native_hooks::Game::native_command (a hook point): run `/smartf3`, leave
// everything else to the game (and to other mods).
fn smartf3_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.smartf3_command(command) {
        *ran = true;
    }
}
