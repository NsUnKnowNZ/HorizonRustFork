fn camerapture_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::camerapture::use_item(game, stack, claimed);
}
