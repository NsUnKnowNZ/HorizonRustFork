fn xaeroworldmap_client_tick(game: &mut crate::game::Game<'_>) {
    crate::game::xaeroworldmap::close_pending(game);
    crate::game::xaeroworldmap::client_tick(game);
}
