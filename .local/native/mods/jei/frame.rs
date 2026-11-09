// Once a frame: build the world's tables once, show the item list, and run
// what a click on a bookmark left for it.
fn jei_frame(game: &mut crate::game::Game<'_>) {
    crate::game::jei_glue::sync(game);
    crate::game::jei_glue::flush(game);
}
