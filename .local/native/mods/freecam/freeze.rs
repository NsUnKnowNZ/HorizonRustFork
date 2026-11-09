// The player's simulation tick: skipped while the freecam freezes the
// player — no gravity, no momentum, no input drift (the gate is set by
// the game's tick, a moment before this runs). Movement input this tick —
// a bot's held keys, a stick's push — runs the tick anyway: whoever
// drives the player drives it, and the freeze only holds an idle player.
fn freecam_freeze(keys: crate::sprint::Keys) -> core::ops::ControlFlow<Result<()>> {
    let movement = keys.forward || keys.backward || keys.left || keys.right || keys.jump;
    if crate::motion::freecam_gate::skip(movement) {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
