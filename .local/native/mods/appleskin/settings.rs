fn official_appleskin_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::official_appleskin as m;
    if mod_id != m::ID {
        return;
    }
    buttons.extend(game.appleskin_settings());
}
