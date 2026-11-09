fn mutantmonsters_interact(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    stack: Option<&horizon_client::items::ItemStack>,
    claimed: &mut bool,
) {
    crate::game::mutantmonsters::interact(game, entity, kind, stack, claimed);
}
