fn camerapture_client_tick(game: &mut crate::game::Game<'_>) {
    crate::game::camerapture_client::client_tick(game);
    crate::game::camerapture_frames::flush(game);
}
