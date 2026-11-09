// A press on a container screen: over a bookmark it is the mod's and goes no
// further (it would take a slot's stack or the world's use).
fn jei_mouse_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::jei_glue::screen_mouse_pressed(game, screen, button, modifiers, canceled);
}
