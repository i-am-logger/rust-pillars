//! Known Adler-32 values, and the API around them.

use rust_code_checksum::{Adler32, Bytes, Checksum, ParseChecksumError};

#[test]
fn known_values() {
    assert_eq!(Checksum::of(b"").get(), 0x0000_0001);
    assert_eq!(Checksum::of(b"a").get(), 0x0062_0062);
    assert_eq!(Checksum::of(b"abc").get(), 0x024d_0127);
    assert_eq!(Checksum::of(b"Wikipedia").get(), 0x11e6_0398);
}

#[test]
fn feeding_in_pieces_matches_feeding_whole() {
    let mut sum = Adler32::new();
    sum.update(Bytes::of(b"Wiki"));
    sum.update(Bytes::of(b""));
    sum.update(Bytes::of(b"pedia"));
    assert_eq!(sum.finish(), Checksum::of(b"Wikipedia"));
    assert_eq!(Adler32::default(), Adler32::new());
}

#[test]
fn a_long_run_of_high_bytes_is_reduced_in_time() {
    let ones = vec![0xff_u8; 100_000];
    assert_eq!(Checksum::of(&ones).get(), 0x149a_302c);
}

#[test]
fn it_prints_and_parses_eight_hex_digits() {
    let sum = Checksum::of(b"Wikipedia");
    assert_eq!(sum.to_string(), "11e60398");
    assert_eq!(Checksum::parse("11e60398"), Ok(sum));
    assert_eq!(Checksum::parse("00000001"), Ok(Checksum::of(b"")));
}

#[test]
fn it_refuses_what_is_not_a_checksum() {
    assert_eq!(Checksum::parse("11e6039"), Err(ParseChecksumError::Length));
    assert_eq!(
        Checksum::parse("11e603980"),
        Err(ParseChecksumError::Length)
    );
    assert_eq!(Checksum::parse("+1e60398"), Err(ParseChecksumError::NotHex));
    assert_eq!(Checksum::parse("11e6039g"), Err(ParseChecksumError::NotHex));
}
