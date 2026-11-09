// A mapped key went down or up: the mod takes the fly keys while the camera
// is detached and the hotbar digits while a combo key is held — presses
// only, since a release must reach the game's own bookkeeping.
fn freecam_mapped(
    game: &crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
    pressed: bool,
    repeat: bool,
) -> core::ops::ControlFlow<Result<()>> {
    use crate::game::freecam as freecam;
    if !pressed
        || repeat
        || !game.native_on(freecam::ID)
        || !game.native_playing()
    {
        return core::ops::ControlFlow::Continue(());
    }
    let Some(name) = horizon_client::keybinds::key_name(key) else {
        return core::ops::ControlFlow::Continue(());
    };
    if freecam::swallows(name) {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
