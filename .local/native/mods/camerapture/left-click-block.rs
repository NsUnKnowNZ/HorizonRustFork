fn camerapture_left_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    crate::game::camerapture_frames::left_click(game, pos, face, canceled);
}
