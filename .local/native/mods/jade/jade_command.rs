fn jade_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::jade::client_command(game, command, ran);
}
