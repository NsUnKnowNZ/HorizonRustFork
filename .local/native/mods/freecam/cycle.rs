// The perspective key: taken while the freecam holds the camera, so F5
// cannot disturb the detached view.
fn freecam_cycle(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    use crate::game::freecam as freecam;
    if game.native_on(freecam::ID) && freecam::is_active() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
