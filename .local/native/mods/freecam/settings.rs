// Freecam's settings page: the runtime's buttons.
fn freecam_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::freecam as freecam;
    if mod_id != freecam::ID {
        return;
    }
    let game = &*game;
    let settings = freecam::settings(|key| game.native_setting(freecam::ID, key));
    buttons.extend(settings.labels());
}
