// native_hooks::Game::native_command (a hook point): run `/carryblock`, leave
// everything else to the game (and to other mods).
fn carryblock_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.carryblock_command(command) {
        *ran = true;
    }
}
