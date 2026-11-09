// /jei says what this build is; /jei give turns "Click gives items" on or off;
// /jei sort orders the list; /jei bookmarks clear empties the bar; /jei config
// says where its files are.
fn jei_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::jei as m;
    let Some(args) = command.strip_prefix("jei") else {
        return;
    };
    if !(args.is_empty() || args.starts_with(' ')) {
        return;
    }
    let Some(cmd) = m::command(args) else {
        *ran = true;
        game.native_chat(m::COMMAND_USAGE);
        return;
    };
    *ran = true;
    let lang = m::lang(game.native_setting(m::ID, "language").as_deref());
    match cmd {
        m::Command::About => {
            let text = match crate::game::jei_glue::counts() {
                Some((items, recipes)) => m::about_counts(items, recipes),
                None => m::about(),
            };
            game.native_chat(&text);
        }
        m::Command::ToggleGive => {
            let on = m::give(game.native_setting(m::ID, "give").as_deref());
            game.native_set_setting(m::ID, "give", if on { "false" } else { "true" });
            game.native_chat(&m::give_button(!on));
        }
        m::Command::Sort(sort) => {
            crate::game::jei_glue::set_sort(game, sort);
            game.native_chat(&lang.sort_button(sort));
        }
        m::Command::SortNone => {
            let sort = crate::game::jei_glue::settings().sort;
            game.native_chat(&lang.sort_button(sort));
        }
        m::Command::BookmarksClear => {
            crate::game::jei_glue::clear_bookmarks();
            game.native_chat(lang.bookmarks_cleared());
        }
        m::Command::Config => game.native_chat(&crate::game::jei_glue::files()),
    }
}
