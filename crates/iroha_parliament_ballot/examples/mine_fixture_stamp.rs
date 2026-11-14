//! Mine the pinned admission-stamp nonce of a reproducible test ballot.
//!
//! A test of a dependent crate proves its ballots with fixed public tapes
//! (`iroha_parliament_ballot::fixtures`, feature `test-fixtures`) and pins each stamp nonce,
//! because a 28-bit stamp is far too slow to mine in a debug test. When a fixture input
//! changes, the test prints the new stamp targets; this tool finds a nonce for one of them
//! with the crate's own search, so no second implementation of the work frame exists:
//!
//! ```text
//! scripts/cargo_fast.sh -- run --release -p iroha_parliament_ballot \
//!     --example mine_fixture_stamp -- <payload_digest_hex> <difficulty_bits>
//! ```
//!
//! It prints the offset of the first nonce found from the zero nonce (the nonce is that
//! offset, little-endian, in its first eight bytes) and its work bits. Any nonce that
//! reaches the difficulty is valid, so different runs may print different offsets. It reads
//! no credential, proves nothing and never enables a network.

use std::{num::NonZeroUsize, process::ExitCode, sync::atomic::AtomicBool};

use iroha_data_model::governance::anonymous_ballot::{AdmissionNonceV1, Digest512};
use iroha_parliament_ballot::stamp::StampTargetV1;

/// Decode exactly 128 hexadecimal digits.
fn payload_digest(hex: &str) -> Option<Digest512> {
    let digits = hex.as_bytes();
    if digits.len() != 128 {
        return None;
    }
    let mut bytes = [0_u8; 64];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        let high = char::from(pair[0]).to_digit(16)?;
        let low = char::from(pair[1]).to_digit(16)?;
        *byte = u8::try_from(high * 16 + low).ok()?;
    }
    Some(Digest512::new(bytes))
}

/// The offset of `nonce` from the zero nonce, when it fits the first eight bytes.
fn offset_of(nonce: &AdmissionNonceV1) -> Option<u64> {
    let bytes = nonce.as_bytes();
    let (low, high) = bytes.split_at(8);
    high.iter()
        .all(|byte| *byte == 0)
        .then(|| u64::from_le_bytes(low.try_into().expect("eight bytes")))
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [digest, bits] = arguments.as_slice() else {
        eprintln!("usage: mine_fixture_stamp <payload_digest_hex> <difficulty_bits>");
        return ExitCode::from(2);
    };
    let (Some(digest), Ok(bits)) = (payload_digest(digest), bits.parse::<u8>()) else {
        eprintln!("expected 128 hexadecimal digits and a difficulty in bits");
        return ExitCode::from(2);
    };
    let target = match StampTargetV1::new(digest, bits) {
        Ok(target) => target,
        Err(error) => {
            eprintln!("stamp target: {error}");
            return ExitCode::FAILURE;
        }
    };
    let threads = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let zero = AdmissionNonceV1::new([0; AdmissionNonceV1::LENGTH]);
    match target.mine_parallel(zero, threads, &AtomicBool::new(false)) {
        Ok(mined) => {
            let Some(offset) = offset_of(&mined.stamp.nonce) else {
                eprintln!("the search left the first eight nonce bytes");
                return ExitCode::FAILURE;
            };
            println!(
                "offset={offset} work_bits={} attempts={}",
                mined.work_bits, mined.attempts
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("stamp search: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_decode_exactly_one_digest_and_small_offsets() {
        let digest = payload_digest(&"0f".repeat(64)).expect("128 digits");
        assert_eq!(digest.as_bytes(), &[0x0f; 64]);
        assert!(payload_digest(&"0f".repeat(63)).is_none());
        assert!(payload_digest(&"zz".repeat(64)).is_none());

        let mut nonce = [0_u8; AdmissionNonceV1::LENGTH];
        nonce[..8].copy_from_slice(&0x0102_0304_u64.to_le_bytes());
        assert_eq!(offset_of(&AdmissionNonceV1::new(nonce)), Some(0x0102_0304));
        nonce[8] = 1;
        assert_eq!(offset_of(&AdmissionNonceV1::new(nonce)), None);
    }

    #[test]
    fn a_low_difficulty_target_is_mined_and_accepted() {
        let target = StampTargetV1::new(Digest512::new([0x5a; 64]), 8).expect("target");
        let zero = AdmissionNonceV1::new([0; AdmissionNonceV1::LENGTH]);
        let mined = target
            .mine_parallel(zero, NonZeroUsize::MIN, &AtomicBool::new(false))
            .expect("an eight-bit stamp");
        assert!(target.accepts(&mined.stamp.nonce));
        assert!(offset_of(&mined.stamp.nonce).is_some());
    }
}
