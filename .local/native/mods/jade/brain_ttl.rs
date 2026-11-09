// A sniffer's dig cooldown, for Jade's NextEntityDropProvider (`Brain.getTimeUntilExpiry`): `Slot` keeps its time to
// live to this file.
impl super::Slot<()> {
    /// The ticks left of a memory with no value but its time.
    pub fn hz_jade_ttl(&self) -> i64 {
        self.ttl
    }
}
