fn no_durability_loss(game: &mut Game<'_>, stack: &ItemStack) -> std::ops::ControlFlow<u32> {
    if super::what_durability::keeps_durability(&stack.item, &game.items().item_tags) {
        std::ops::ControlFlow::Continue(())
    } else {
        std::ops::ControlFlow::Break(0)
    }
}
