fn camerapture_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::camerapture::player_tick(game);
    crate::game::camerapture_frames::player_tick(game);
}
