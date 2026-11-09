fn camerapture_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    crate::game::camerapture_frames::right_click(game, pos, face, canceled);
}
