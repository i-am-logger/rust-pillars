//! A thermostat's vocabulary: what the sensor reports, the temperature we
//! want to hold, and whether to heat.
//!
//! The sensor sends tenths of a degree Celsius as a signed 16-bit integer,
//! little-endian, two bytes per reading: `[0xD7, 0x00]` is 21.5 °C and
//! `[0xD8, 0xFF]` is -4.0 °C. Those bytes become a [`Reading`] through the
//! derived `zerocopy::FromBytes` -- `Reading::read_from_bytes` for one, or
//! `<[Reading]>::ref_from_bytes` to borrow a whole buffer of them -- so no
//! integer crosses the API and no constructor is a new seam. A [`Setpoint`]
//! has the same two-byte layout, so a host stores and loads it as bytes too.
//!
//! The value is held as zerocopy's little-endian `I16`, so the layout is the
//! sensor's on every target, and it is compared by value, never by bytes.

use zerocopy::little_endian::I16;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

/// One temperature from the sensor, in tenths of a degree Celsius.
///
/// Plain old data with the sensor's own layout: two bytes, little-endian,
/// valid for every bit pattern. Ordered by temperature.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    FromBytes,
    IntoBytes,
    Immutable,
    KnownLayout,
    Unaligned,
)]
#[repr(transparent)]
pub struct Reading(pub(crate) I16);

/// The temperature to hold, in the sensor's units: tenths of a degree
/// Celsius, two bytes, little-endian.
///
/// Its own type, not a [`Reading`]: a setpoint is what we want, a reading is
/// what we measured, and passing one for the other does not compile.
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned,
)]
#[repr(transparent)]
pub struct Setpoint(pub(crate) I16);

/// Should we heat right now? Two states, named, never a `bool`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub enum Heat {
    /// Below the setpoint: heat.
    On,
    /// At or above the setpoint: don't.
    Off,
}

/// The decision: heat when the reading is below the setpoint, otherwise
/// don't. At the setpoint exactly, the heater stays off.
pub const fn heat(now: Reading, hold: Setpoint) -> Heat {
    if now.0.get() < hold.0.get() {
        Heat::On
    } else {
        Heat::Off
    }
}

// The layouts are the sensor's: two bytes, no more.
const _: () = assert!(core::mem::size_of::<Reading>() == 2);
const _: () = assert!(core::mem::size_of::<Setpoint>() == 2);

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A reading of `tenths`, as the sensor would send it.
    fn reading(tenths: i16) -> Reading {
        Reading::read_from_bytes(&tenths.to_le_bytes()).expect("two bytes are a reading")
    }

    /// A setpoint of `tenths`, as a host would store it.
    fn setpoint(tenths: i16) -> Setpoint {
        Setpoint::read_from_bytes(&tenths.to_le_bytes()).expect("two bytes are a setpoint")
    }

    #[test]
    fn sensor_bytes_are_little_endian_tenths() {
        let warm = Reading::read_from_bytes(&[0xD7, 0x00]).expect("two bytes");
        let cold = Reading::read_from_bytes(&[0xD8, 0xFF]).expect("two bytes");
        assert_eq!(warm.0.get(), 215);
        assert_eq!(cold.0.get(), -40);
        assert!(cold < warm);
    }

    #[test]
    fn a_buffer_of_readings_is_borrowed_whole_and_an_odd_byte_is_refused() {
        let stream = [0xD7, 0x00, 0xD8, 0xFF];
        let readings = <[Reading]>::ref_from_bytes(&stream).expect("whole readings");
        assert_eq!(readings, [reading(215), reading(-40)]);
        assert!(<[Reading]>::ref_from_bytes(&stream[..3]).is_err());
        assert!(Reading::read_from_bytes(&[0xD7]).is_err());
    }

    #[test]
    fn below_the_setpoint_heats() {
        assert_eq!(heat(reading(209), setpoint(210)), Heat::On);
    }

    #[test]
    fn at_the_setpoint_does_not_heat() {
        assert_eq!(heat(reading(210), setpoint(210)), Heat::Off);
    }

    #[test]
    fn above_the_setpoint_does_not_heat() {
        assert_eq!(heat(reading(211), setpoint(210)), Heat::Off);
    }

    #[test]
    fn negative_temperatures_compare_by_value_not_by_bytes() {
        // -0.1 °C is `[0xFF, 0xFF]`: larger than 0.0 °C as bytes, smaller as
        // a temperature.
        assert_eq!(heat(reading(-1), setpoint(0)), Heat::On);
        assert_eq!(heat(reading(-40), setpoint(-35)), Heat::On);
        assert_eq!(heat(reading(-40), setpoint(-40)), Heat::Off);
        assert_eq!(heat(reading(-40), setpoint(-45)), Heat::Off);
        assert_eq!(heat(reading(0), setpoint(-1)), Heat::Off);
    }

    proptest! {
        #[test]
        fn heats_exactly_when_below_the_setpoint(now in any::<i16>(), hold in any::<i16>()) {
            prop_assert_eq!(heat(reading(now), setpoint(hold)) == Heat::On, now < hold);
        }
    }
}
