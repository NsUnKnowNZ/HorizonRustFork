fn camerapture_render_hand(game: &crate::game::Game<'_>, canceled: &mut bool) {
    crate::game::camerapture_client::render_hand(game, canceled);
}
