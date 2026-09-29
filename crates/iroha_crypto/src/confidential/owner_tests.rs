//! Live-cell clearing controls for confidential wallet keys and partial RNG output.

use super::*;
use rand_core::TryRngCore;

thread_local! {
    static CLEARED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn observe_erasure(keyset: &ConfidentialKeyset) {
    for key in [
        keyset.spend_key(),
        keyset.nullifier_key(),
        keyset.incoming_view_key(),
        keyset.outgoing_view_key(),
        keyset.full_view_key(),
    ] {
        assert!(key.iter().all(|&byte| byte == 0));
    }
    CLEARED_BYTES.with(|count| count.set(count.get() + 5 * 32));
}

fn cleared_bytes() -> usize {
    CLEARED_BYTES.with(std::cell::Cell::get)
}

fn populated_keyset() -> ConfidentialKeyset {
    ConfidentialKeyset {
        spend: [0x11; 32],
        nullifier: [0x22; 32],
        incoming_view: [0x33; 32],
        outgoing_view: [0x44; 32],
        full_view: [0x55; 32],
    }
}

#[test]
fn clones_are_independent_redacted_clearing_owners() {
    fn clearing_owner<T: Zeroize + ZeroizeOnDrop>() {}
    clearing_owner::<ConfidentialKeyset>();
    let original = populated_keyset();
    let mut copy = original.clone();
    assert_eq!(format!("{original:?}"), "ConfidentialKeyset { .. }");
    assert_eq!(copy.spend_key(), original.spend_key());
    assert_eq!(copy.full_view_key(), original.full_view_key());
    let before = cleared_bytes();
    copy.zeroize();
    assert_eq!(cleared_bytes() - before, 160);
    assert_eq!(copy.spend_key(), &[0; 32]);
    assert_eq!(copy.nullifier_key(), &[0; 32]);
    assert_eq!(copy.incoming_view_key(), &[0; 32]);
    assert_eq!(copy.outgoing_view_key(), &[0; 32]);
    assert_eq!(copy.full_view_key(), &[0; 32]);
    assert_eq!(original.spend_key(), &[0x11; 32]);
    assert_eq!(original.full_view_key(), &[0x55; 32]);
    drop(original);
    assert_eq!(cleared_bytes() - before, 320);
}

#[test]
fn normal_drop_error_and_unwind_clear_all_five_keys() {
    fn fail() -> Result<(), ()> {
        let _keyset = populated_keyset();
        Err(())?;
        Ok(())
    }
    let before = cleared_bytes();
    drop(populated_keyset());
    assert_eq!(cleared_bytes() - before, 160);
    assert!(fail().is_err());
    assert_eq!(cleared_bytes() - before, 320);
    assert!(
        std::panic::catch_unwind(|| {
            let _keyset = populated_keyset();
            panic!("test-only wallet key owner unwind");
        })
        .is_err()
    );
    assert_eq!(cleared_bytes() - before, 480);
}

#[derive(Debug)]
struct PartialRngError;

impl core::fmt::Display for PartialRngError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("test-only partial RNG failure")
    }
}

impl std::error::Error for PartialRngError {}

struct PartialRng {
    unwind: bool,
}

impl TryRngCore for PartialRng {
    type Error = PartialRngError;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Err(PartialRngError)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Err(PartialRngError)
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        destination.fill(0xa5);
        assert!(
            !self.unwind,
            "test-only RNG failure after writing private seed bytes"
        );
        Err(PartialRngError)
    }
}

impl TryCryptoRng for PartialRng {}

#[test]
fn partial_random_seed_is_owned_before_error_or_unwind() {
    let before = cleared_bytes();
    assert!(matches!(
        generate_keyset(&mut PartialRng { unwind: false }),
        Err(ConfidentialKeyError::RandomBytes)
    ));
    assert_eq!(cleared_bytes() - before, 160);
    assert!(
        std::panic::catch_unwind(|| {
            let _ = generate_keyset(&mut PartialRng { unwind: true });
        })
        .is_err()
    );
    assert_eq!(cleared_bytes() - before, 320);
}

#[test]
fn rejected_and_derived_keysets_keep_the_owner_lifetime() {
    let before = cleared_bytes();
    assert!(matches!(
        derive_keyset([0; 32]),
        Err(ConfidentialKeyError::InertSpendKey)
    ));
    assert_eq!(cleared_bytes() - before, 160);
    let keyset = derive_keyset([0x42; 32]).expect("valid spend key");
    assert_eq!(cleared_bytes() - before, 160);
    assert_eq!(keyset.spend_key(), &[0x42; 32]);
    drop(keyset);
    assert_eq!(cleared_bytes() - before, 320);
}
