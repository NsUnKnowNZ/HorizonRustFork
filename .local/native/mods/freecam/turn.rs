// The player's view turned: the freecam's turn while it holds the camera,
// the player's own otherwise.
fn freecam_turn(
    game: &crate::game::Game<'_>,
    xo: f64,
    yo: f64,
) -> core::ops::ControlFlow<()> {
    use crate::game::freecam as freecam;
    if game.native_on(freecam::ID) && freecam::turn(xo, yo) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
