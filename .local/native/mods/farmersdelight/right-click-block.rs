fn farmersdelight_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    let _ = face;
    crate::game::farmersdelight::click(game, pos, canceled);
}
