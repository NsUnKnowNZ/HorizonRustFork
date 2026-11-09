// The player's own body in view: hidden while the freecam holds the camera
// and the setting asks.
fn freecam_body(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    use crate::game::freecam as freecam;
    if game.native_on(freecam::ID) && freecam::hides_player() {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
