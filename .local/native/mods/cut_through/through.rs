fn cut_through(game: &Game<'_>) -> core::ops::ControlFlow<bool> {
    match game.pick_unfiltered() {
        Some(h) if game.block_collision_shape(h.pos).is_empty() => core::ops::ControlFlow::Break(true),
        _ => core::ops::ControlFlow::Continue(()),
    }
}
