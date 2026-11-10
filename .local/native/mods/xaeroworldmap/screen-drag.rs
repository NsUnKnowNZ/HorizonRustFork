fn xaeroworldmap_screen_drag(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, delta: [f32; 2], modifiers: u32, canceled: &mut bool) {
    let _ = (button, modifiers);
    crate::game::xaeroworldmap::screen_drag(game, screen, delta, canceled);
}
