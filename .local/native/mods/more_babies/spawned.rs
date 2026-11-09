fn more_babies_spawned(game: &mut crate::game::Game<'_>, dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    crate::game::more_babies::spawned(game, dimension, kind, pos, natural);
}
