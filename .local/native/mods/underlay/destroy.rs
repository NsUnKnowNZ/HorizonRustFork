fn underlay_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::underlay::destroyed(game, pos);
}
