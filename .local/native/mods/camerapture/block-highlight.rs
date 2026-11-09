fn camerapture_block_highlight(game: &crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::camerapture_client::block_highlight(game, pos, canceled);
}
