// A click in the world: eaten while the freecam holds the camera and the
// interaction is Off, so the player neither digs nor places from a distance.
// Press or release, the helper says whose hand is on the mouse first: the
// camera's aim follows only the player's own clicks, and a bot's clicks
// (which never come through `play_mouse`) keep their targeting.
fn freecam_mouse(
    game: &crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    use crate::game::freecam as freecam;
    let _ = button;
    freecam::mouse_click(pressed);
    if game.native_on(freecam::ID)
        && game.native_playing()
        && freecam::blocks_interaction()
    {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
