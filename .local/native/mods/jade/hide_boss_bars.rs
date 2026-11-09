fn jade_hide_boss_bars(game: &Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::jade::hide_boss_bars(game) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
