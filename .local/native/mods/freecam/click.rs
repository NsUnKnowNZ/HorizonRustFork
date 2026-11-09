// A click on Freecam's settings page: change the setting and keep it.
fn freecam_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::freecam as freecam;
    if mod_id != freecam::ID {
        return;
    }
    let settings = {
        let game = &*game;
        freecam::click(button, |key| game.native_setting(freecam::ID, key))
    };
    for (key, value) in settings.stored() {
        game.native_set_setting(freecam::ID, key, &value);
    }
}
