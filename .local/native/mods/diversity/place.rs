fn diversity_place(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    crate::game::diversity::placed(game, stack);
}
