// Enhanced Attack Indicator: this frame's progress (0..1, 2 = full), or none
// (the vanilla attack cooldown shows).
use std::sync::atomic::{AtomicU32, Ordering};

const NONE: u32 = u32::MAX;
static PROGRESS: AtomicU32 = AtomicU32::new(NONE);

pub fn set(progress: Option<f32>) {
    PROGRESS.store(progress.map_or(NONE, f32::to_bits), Ordering::Relaxed);
}

pub fn progress() -> Option<f32> {
    match PROGRESS.load(Ordering::Relaxed) {
        NONE => None,
        bits => Some(f32::from_bits(bits)),
    }
}
