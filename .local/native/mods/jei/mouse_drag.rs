// Dragging a bookmark over another slot moves it there, as JEI's bar does.
fn jei_mouse_drag(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    delta: [f32; 2],
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::jei_glue::screen_mouse_dragged(game, screen, button, delta, modifiers, canceled);
}
