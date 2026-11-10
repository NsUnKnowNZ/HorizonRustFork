// A right click on a pot or a board opens its scene instead of reaching the world.
fn handson_delight_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    if crate::game::handson_delight::mouse(game, button, pressed) {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
