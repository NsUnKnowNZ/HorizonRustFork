fn diversity_pickup(player: u128, stack: &horizon_client::items::ItemStack, canceled: &mut bool) {
    if !*canceled {
        crate::game::diversity::picked_up(player, stack);
    }
}
