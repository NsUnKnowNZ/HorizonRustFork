fn underlay_attack(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::underlay::attack(game, pos, canceled);
}
