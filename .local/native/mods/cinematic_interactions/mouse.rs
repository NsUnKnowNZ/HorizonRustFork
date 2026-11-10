// A right click on a station opens its scene instead of reaching the world.
fn cinematic_interactions_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    if crate::game::cinematic_interactions::mouse(game, button, pressed) {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
