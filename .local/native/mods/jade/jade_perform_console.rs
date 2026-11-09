fn jade_perform_console(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::jade::perform_console_command(game, command, ran);
}
