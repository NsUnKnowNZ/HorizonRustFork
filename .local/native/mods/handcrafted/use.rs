fn handcrafted_use(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::handcrafted::use_on(game, stack, pos, claimed);
}
