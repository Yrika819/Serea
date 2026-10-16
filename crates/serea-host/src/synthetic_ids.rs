//! Host-owned ULID material for synthetic runtime processes.
//!
//! Unlike testkit's replayable source, this source starts with OS-provided
//! entropy so fast relaunches, clock rollback, and PID reuse do not replay the
//! same deterministic sequence. EventBus shares one source for EventIds and
//! Router RequestIds during the lifetime protected by the Host lock.

use serea_protocol::{TimestampMs, UlidSource, UlidValue};

pub(crate) struct SyntheticHostUlidSource {
    timestamp: TimestampMs,
    entropy: [u8; 10],
}

impl SyntheticHostUlidSource {
    pub(crate) fn new(timestamp: TimestampMs) -> Result<Self, getrandom::Error> {
        let mut entropy = [0; 10];
        getrandom::fill(&mut entropy)?;
        Ok(Self { timestamp, entropy })
    }
}

impl UlidSource for SyntheticHostUlidSource {
    fn next_ulid(&mut self) -> UlidValue {
        let value = UlidValue::new(self.timestamp, self.entropy);
        // The 80-bit counter cannot exhaust in a practical Host lifetime. If
        // it ever does, fail loudly instead of reusing an identifier.
        let mut exhausted = true;
        for byte in self.entropy.iter_mut().rev() {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                exhausted = false;
                break;
            }
        }
        assert!(!exhausted, "synthetic Host ULID entropy exhausted");
        self.timestamp = self.timestamp.checked_next().unwrap_or(self.timestamp);
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_source_advances_and_relaunch_seed_is_not_replayed() {
        let timestamp = TimestampMs::new(1_800_000_000_000).unwrap();
        let mut first = SyntheticHostUlidSource::new(timestamp).unwrap();
        let a = first.next_ulid();
        let b = first.next_ulid();
        assert_ne!(a, b);

        // Independent process lifetimes get independent OS-entropy seeds.
        // A collision has probability 2^-80 for the same timestamp.
        let mut relaunched = SyntheticHostUlidSource::new(timestamp).unwrap();
        assert_ne!(a, relaunched.next_ulid());
    }
}
