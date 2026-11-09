fn mutantmonsters_death(game: &mut crate::game::Game<'_>, entity: u32, kind: &str, cancel: &mut bool) {
    crate::game::mutantmonsters::death(game, entity, kind, cancel);
}
