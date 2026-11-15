//! Checked STEP/DIR command encoding and fixed-clock timing configuration.
//!
//! The command word is transferred to the PIO as a native `u32`:
//! bit 31 is step/hold, bit 30 is the physical DIR level, and bits 29:0
//! contain a positive duration in 10 microsecond units.

use fixed::types::U24F8;

pub const DELAY_UNIT_NS: u64 = 10_000;
pub const CYCLES_PER_UNIT: u32 = 20;
pub const SM_CLOCK_HZ: u32 = 2_000_000;
pub const MAX_DELAY_UNITS: u32 = 0x3fff_ffff;

const STEP_MASK: u32 = 1 << 31;
const DIRECTION_MASK: u32 = 1 << 30;
const DELAY_MASK: u32 = MAX_DELAY_UNITS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Hold,
    Step,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Low,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayError {
    Zero,
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    DividerOutOfRange,
    NotExactlyRepresentable,
}

/// A validated positive duration in 10 microsecond units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayUnits(u32);

impl DelayUnits {
    pub const fn new(units: u32) -> Result<Self, DelayError> {
        if units == 0 {
            Err(DelayError::Zero)
        } else if units > MAX_DELAY_UNITS {
            Err(DelayError::OutOfRange)
        } else {
            Ok(Self(units))
        }
    }

    /// Convert nanoseconds to units, rounding every positive partial unit up.
    pub const fn from_nanos_ceil(ns: u64) -> Result<Self, DelayError> {
        if ns == 0 {
            return Err(DelayError::Zero);
        }
        // Quotient/remainder avoids overflowing `ns + unit - 1`.
        let units = ns / DELAY_UNIT_NS + if ns % DELAY_UNIT_NS == 0 { 0 } else { 1 };
        if units > MAX_DELAY_UNITS as u64 {
            Err(DelayError::OutOfRange)
        } else {
            Ok(Self(units as u32))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A validated command word. Its private field prevents unchecked words entering the API.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command(u32);

impl Command {
    pub const fn new(action: Action, dir: Direction, delay: DelayUnits) -> Self {
        let step = match action {
            Action::Hold => 0,
            Action::Step => STEP_MASK,
        };
        let direction = match dir {
            Direction::Low => 0,
            Direction::High => DIRECTION_MASK,
        };
        Self(step | direction | delay.0)
    }

    pub const fn word(self) -> u32 {
        self.0
    }

    pub const fn try_from_word(word: u32) -> Result<Self, DelayError> {
        let delay = word & DELAY_MASK;
        if delay == 0 {
            Err(DelayError::Zero)
        } else {
            // All remaining bits are the two defined flags in a u32.
            Ok(Self(word))
        }
    }
}

/// Fixed 2 MHz timing, with the exact hardware 16.8 divider stored as raw bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingConfig {
    divider_raw: u32,
}

impl TimingConfig {
    pub const fn for_sys_clock(sys_hz: u32) -> Result<Self, ClockError> {
        let numerator = (sys_hz as u64) * 256;
        let target = SM_CLOCK_HZ as u64;
        if numerator % target != 0 {
            return Err(ClockError::NotExactlyRepresentable);
        }
        let raw = numerator / target;
        if raw < 256 || raw > 0x00ff_ffff {
            return Err(ClockError::DividerOutOfRange);
        }
        Ok(Self {
            divider_raw: raw as u32,
        })
    }

    pub const fn divider(self) -> U24F8 {
        U24F8::from_bits(self.divider_raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_encoding_and_rejection() {
        let one = DelayUnits::new(1).unwrap();
        assert_eq!(
            Command::new(Action::Step, Direction::Low, one).word(),
            0x8000_0001
        );
        assert_eq!(
            Command::new(Action::Step, Direction::High, one).word(),
            0xc000_0001
        );
        assert_eq!(
            Command::new(Action::Hold, Direction::High, one).word(),
            0x4000_0001
        );
        assert_eq!(DelayUnits::new(0), Err(DelayError::Zero));
        assert_eq!(
            DelayUnits::new(MAX_DELAY_UNITS + 1),
            Err(DelayError::OutOfRange)
        );
        assert_eq!(Command::try_from_word(0x8000_0000), Err(DelayError::Zero));
        let max = Command::new(
            Action::Hold,
            Direction::Low,
            DelayUnits::new(MAX_DELAY_UNITS).unwrap(),
        );
        assert_eq!(max.word(), MAX_DELAY_UNITS);
        assert_eq!(Command::try_from_word(max.word()), Ok(max));
    }

    #[test]
    fn nanoseconds_round_up_without_overflow() {
        assert_eq!(DelayUnits::from_nanos_ceil(1).unwrap().get(), 1);
        assert_eq!(DelayUnits::from_nanos_ceil(10_000).unwrap().get(), 1);
        assert_eq!(DelayUnits::from_nanos_ceil(10_001).unwrap().get(), 2);
        assert_eq!(DelayUnits::from_nanos_ceil(0), Err(DelayError::Zero));
        assert_eq!(
            DelayUnits::from_nanos_ceil(u64::MAX),
            Err(DelayError::OutOfRange)
        );
        assert_eq!(
            DelayUnits::from_nanos_ceil(MAX_DELAY_UNITS as u64 * DELAY_UNIT_NS)
                .unwrap()
                .get(),
            MAX_DELAY_UNITS
        );
        assert_eq!(
            DelayUnits::from_nanos_ceil(MAX_DELAY_UNITS as u64 * DELAY_UNIT_NS + 1),
            Err(DelayError::OutOfRange)
        );
    }

    #[test]
    fn validates_exact_divider_without_clamping() {
        assert_eq!(
            TimingConfig::for_sys_clock(150_000_000)
                .unwrap()
                .divider()
                .to_bits(),
            19_200
        );
        assert_eq!(
            TimingConfig::for_sys_clock(125_000_000)
                .unwrap()
                .divider()
                .to_bits(),
            16_000
        );
        assert_eq!(
            TimingConfig::for_sys_clock(2_000_000)
                .unwrap()
                .divider()
                .to_bits(),
            256
        );
        assert_eq!(
            TimingConfig::for_sys_clock(1_999_999),
            Err(ClockError::NotExactlyRepresentable)
        );
        assert_eq!(
            TimingConfig::for_sys_clock(2_000_001),
            Err(ClockError::NotExactlyRepresentable)
        );
        assert_eq!(
            TimingConfig::for_sys_clock(0),
            Err(ClockError::DividerOutOfRange)
        );
    }
}
