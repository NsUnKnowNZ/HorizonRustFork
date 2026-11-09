fn handcrafted_click(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::handcrafted::empty_hand(game, pos, canceled);
}
