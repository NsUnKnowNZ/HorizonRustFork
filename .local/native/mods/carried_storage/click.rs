fn carried_storage_click(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::carried_storage::click(game, pos, canceled);
}
