// The points the player holds, worked out from the level and the bar
// (`/xp set … levels` and level costs leave `totalExperience` behind), or the
// running total when that is higher.
fn simple_death_xp(value: u32, xp: &Experience, keep: bool) -> u32 {
    let mut held: i64 = (0..xp.level.max(0)).map(|l| i64::from(Experience::needed(l))).sum();
    held += (f64::from(xp.progress) * f64::from(Experience::needed(xp.level.max(0)))) as i64;
    let total = held.max(i64::from(xp.total)).min(i64::from(i32::MAX)) as i32;
    simple_death::death_reward(value, total, keep)
}
