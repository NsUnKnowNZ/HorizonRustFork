// The toast on screen: the last message, centered above the hotbar in a
// box, as the original's notification overlay did.
fn freecam_hud(game: &crate::game::Game<'_>, batch: &mut HudBatch) {
    use crate::game::freecam as freecam;
    if !game.native_on(freecam::ID) {
        return;
    }
    let Some(text) = freecam::toast() else {
        return;
    };
    let width = game.native_text_width(&text);
    let x = (batch.width - width) / 2.0;
    let y = batch.height - 59.0;
    game.native_fill(batch, [x - 4.0, y - 2.0, width + 8.0, 12.0], 0x78000000);
    game.native_text(batch, &text, [x, y], 0xFFFFFF, true);
}
