fn better_days_no_skip(game: &Game<'_>) -> core::ops::ControlFlow<bool> {
    if super::better_days::night(game.world.day_time) {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
