// A click on the settings page: "Click gives items", the language, the list's
// order, the tooltip hints and the bar.
fn jei_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::jei as m;
    if mod_id != m::ID {
        return;
    }
    let lang = m::lang(game.native_setting(m::ID, "language").as_deref());
    match button {
        0 => {
            let on = m::give(game.native_setting(m::ID, "give").as_deref());
            game.native_set_setting(m::ID, "give", if on { "false" } else { "true" });
        }
        1 => {
            let lang = m::lang(game.native_setting(m::ID, "language").as_deref());
            game.native_set_setting(m::ID, "language", lang.next().code());
        }
        2 => crate::game::jei_glue::cycle_sort(game),
        3 => {
            let config = crate::game::jei_glue::toggle_hints();
            game.native_chat(&lang.hints_button(config.hints));
        }
        4 => {
            let config = crate::game::jei_glue::toggle_bar();
            game.native_chat(&lang.bookmarks_button(config.bookmarks));
        }
        _ => {}
    }
}
