//! Deterministic time and identifier minting.
//!
//! Model Protocol §10: "The `ModelProvider` trait takes no ambient state. All
//! variability — temperature, seed, the model roster, the clock, the provider's
//! responses — is injected." Everything here is an injection point.

use std::time::Duration;

use serea_protocol::ids::{IdMinter, TimestampMs, UlidSource, UlidValue};
use serea_protocol::{Clock, EpochMillis, ProtocolError, Timestamp};

/// The frozen epoch every deterministic scenario starts from
/// (GoalLatch Adapter §6.1).
pub const FROZEN_EPOCH: &str = "2026-10-01T00:00:00.000Z";

/// The independent legacy seed for deterministic identifier minting, denoting
/// 2026-09-20 rather than the clock's [`FROZEN_EPOCH`]. Preserved so P2B does not
/// change identifier sequences. Well inside the frozen ULID 48-bit range.
const BASE_TIMESTAMP_MS: u64 = 1_789_862_400_000;

/// A clock that only moves when a test moves it.
///
/// It never reads a wall clock: `.clippy.toml` bans `SystemTime::now` and
/// `Instant::now` workspace-wide so this cannot regress into one.
#[derive(Debug, Clone)]
pub struct TestClock {
    start_ms: EpochMillis,
    now_ms: EpochMillis,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::at_epoch()
    }
}

impl TestClock {
    /// A clock parked at the frozen epoch.
    pub fn at_epoch() -> Self {
        Self::at(FROZEN_EPOCH).unwrap_or_else(|error| {
            unreachable!("the frozen epoch is a valid timestamp by construction: {error:?}")
        })
    }

    /// A clock parked at `start`.
    ///
    /// Returns a typed error rather than panicking: a scripted epoch is caller
    /// input, and this is the only time source in the workspace.
    pub fn at(start: &str) -> Result<Self, ProtocolError> {
        let start_ms = Timestamp::new(start)?.to_epoch_millis();
        Ok(Self {
            start_ms,
            now_ms: start_ms,
        })
    }

    /// The current fake time. Never changes unless a test changes it.
    ///
    /// Total by construction: the only mutator is [`TestClock::advance`], which
    /// validates the resulting instant before committing it.
    pub fn now(&self) -> Timestamp {
        Timestamp::from_epoch_millis(self.now_ms)
    }

    /// Advances fake time by `delta` and returns the new time.
    ///
    /// Returns a typed error rather than panicking when the result would leave
    /// the frozen wire form — advancing past year 9999, for instance. The clock
    /// is left untouched in that case, so a test that overruns does not corrupt
    /// the state it was building.
    pub fn advance(&mut self, delta: Duration) -> Result<Timestamp, ProtocolError> {
        let overflow = || ProtocolError::MalformedValue {
            field: serea_protocol::ValueField::Timestamp,
            reason: serea_protocol::ValueRejection::OutOfRange,
        };
        // as_millis returns u128 and truncates sub-millisecond fractions, as in
        // P1. Refuse enormous deltas rather than narrowing or saturating them.
        let delta_ms = i64::try_from(delta.as_millis()).map_err(|_| overflow())?;
        let candidate = self
            .now_ms
            .get()
            .checked_add(delta_ms)
            .ok_or_else(overflow)?;
        let candidate = EpochMillis::new(candidate)?;
        let timestamp = Timestamp::from_epoch_millis(candidate);
        self.now_ms = candidate;
        Ok(timestamp)
    }

    /// The current signed instant. Always succeeds for this validated fake;
    /// the Result shape matches the injected Clock port.
    pub fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Ok(self.now_ms)
    }

    /// The total whole milliseconds advanced since creation, derived rather
    /// than stored. The bounded wire-domain difference fits i64 and u64.
    pub fn elapsed_ms(&self) -> u64 {
        let elapsed = self
            .now_ms
            .get()
            .checked_sub(self.start_ms.get())
            .unwrap_or_else(|| unreachable!("the full wire-domain span fits i64"));
        u64::try_from(elapsed).unwrap_or_else(|error| {
            unreachable!("advance only commits nonnegative deltas: {error:?}")
        })
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        Self::now_ms(self)
    }
}

/// Identifier minting driven by a counter rather than by randomness.
///
/// Protocol Index §2 rule 1 makes a ULID the only minting scheme. Supplying the
/// 48-bit timestamp from an injected counter is what makes a minting sequence
/// reproducible across runs and across processes, which no test can rely on
/// from a wall clock or an unseeded RNG.
#[derive(Debug, Clone)]
pub struct DeterministicUlidSource {
    /// The validated starting millisecond.
    starting_ms: TimestampMs,
    /// The millisecond the next mint carries. Holds at the frozen range end.
    next_timestamp_ms: TimestampMs,
    /// Mints so far. Drives the entropy so uniqueness never depends on the
    /// timestamp staying strictly increasing.
    counter: u64,
    entropy_seed: u8,
}

impl Default for DeterministicUlidSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicUlidSource {
    /// A source that starts at the frozen base timestamp.
    pub fn new() -> Self {
        // The base is a literal inside the frozen 48-bit range, so the only
        // failure below is unreachable; the alternative would be a `const fn`
        // validator, and this is the single place the assumption lives.
        let base = match TimestampMs::new(BASE_TIMESTAMP_MS) {
            Ok(base) => base,
            Err(error) => unreachable!("the frozen base timestamp is in range: {error:?}"),
        };
        Self {
            starting_ms: base,
            next_timestamp_ms: base,
            counter: 0,
            entropy_seed: 0x5a,
        }
    }

    /// A source whose first identifier carries `timestamp_ms` exactly, for a
    /// scenario that needs its identifiers to sort after another scenario's.
    ///
    /// Returns a typed error for a value outside the frozen 48-bit range rather
    /// than panicking on it.
    pub fn starting_at(timestamp_ms: u64) -> Result<Self, ProtocolError> {
        let base = TimestampMs::new(timestamp_ms)?;
        Ok(Self {
            starting_ms: base,
            next_timestamp_ms: base,
            counter: 0,
            entropy_seed: 0x5a,
        })
    }

    /// Returns the source to its starting state, so two harnesses can replay the
    /// same script.
    pub fn reset(&mut self) {
        self.next_timestamp_ms = self.starting_ms;
        self.counter = 0;
    }

    /// The number of identifiers minted so far.
    pub fn minted(&self) -> u64 {
        self.counter
    }

    /// The millisecond the next mint will carry.
    pub fn next_timestamp_ms(&self) -> u64 {
        self.next_timestamp_ms.get()
    }
}

impl UlidSource for DeterministicUlidSource {
    fn next_ulid(&mut self) -> UlidValue {
        let timestamp_ms = self.next_timestamp_ms;
        // Hold at the range end rather than overflowing. A script that mints
        // more identifiers than the 48-bit millisecond range holds is not a
        // scenario, and holding keeps the timestamps monotonic; the
        // counter-derived entropy below keeps every mint distinct either way.
        self.next_timestamp_ms = self
            .next_timestamp_ms
            .checked_next()
            .unwrap_or(self.next_timestamp_ms);
        self.counter += 1;
        let entropy = [
            self.entropy_seed.wrapping_add(self.counter as u8),
            (self.counter >> 8) as u8,
            (self.counter >> 16) as u8,
            self.entropy_seed,
            self.entropy_seed,
            self.entropy_seed.wrapping_add((self.counter >> 24) as u8),
            (self.counter >> 32) as u8,
            (self.counter >> 40) as u8,
            (self.counter >> 48) as u8,
            (self.counter >> 56) as u8,
        ];
        UlidValue::new(timestamp_ms, entropy)
    }
}

/// A minter wired to a deterministic source, for tests that need stable
/// identifiers across runs.
pub fn deterministic_minter() -> IdMinter<DeterministicUlidSource> {
    IdMinter::new(DeterministicUlidSource::new())
}
