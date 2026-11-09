fn official_appleskin_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::official_appleskin as m;
    if mod_id != m::ID {
        return;
    }
    game.appleskin_settings_click(button);
}
