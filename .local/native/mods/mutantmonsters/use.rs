fn mutantmonsters_use(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::mutantmonsters::use_item(game, stack, claimed);
}
