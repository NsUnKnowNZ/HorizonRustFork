// The freecam's freeze gate: the game's tick writes it, the motion tick
// reads it. Pure standard library, so it compiles wherever motion does —
// the game's own state is a world away from this crate.
pub static FREEZE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The player stands still while the freecam asks (the game's tick writes
/// this every tick, so a lost exit clears itself within a tick).
pub fn set_freeze(on: bool) {
    FREEZE.store(on, core::sync::atomic::Ordering::Relaxed);
}

/// Whether the motion tick should skip the player's simulation entirely:
/// no gravity, no momentum, no input drift.
pub fn frozen() -> bool {
    FREEZE.load(core::sync::atomic::Ordering::Relaxed)
}

/// `frozen`, yielding to movement input: a tick whose keys say the player
/// is being driven — a bot's held keys, a stick's push — runs anyway, so
/// whoever drives the player drives it, and the freeze only holds an idle
/// player still.
pub fn skip(movement: bool) -> bool {
    frozen() && !movement
}
