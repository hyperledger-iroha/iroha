//! The fresh clock read must belong to the operation whose review is executing.
use iroha_data_model::kagemusha::{KagemushaWalletTimeAnchorBodyV1, KagemushaWalletTimeAnchorV1};
use native::TimeSource as _;

use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletAnchorPolicyV1, test_support::FakePlatformV1,
};

fn fixture() -> (
    NativeTimeSource<FakePlatformV1>,
    KagemushaWalletAnchoredTimeV1,
) {
    let platform = Arc::new(FakePlatformV1::new(
        KagemushaWalletAnchorPolicyV1::NotRequired,
        1,
    ));
    let source = NativeTimeSource::new(platform);
    let now = source.now().unwrap();
    let anchor = KagemushaWalletTimeAnchorV1 {
        body: KagemushaWalletTimeAnchorBodyV1 {
            version: 1,
            scheme_id: [1; 32],
            wallet_id: [2; 32],
            nonce: [3; 32],
            issuer_time_ms: 1_000_000,
            signer_certificate: [4; 32],
        },
        signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
    };
    let anchored = KagemushaWalletAnchoredTimeV1::new(
        anchor,
        now.boot_id,
        now.monotonic_ms - 10,
        now.monotonic_ms,
        10,
    )
    .unwrap();
    (source, anchored)
}

#[test]
fn fresh_observations_follow_their_own_frozen_operation() {
    let (source, first) = fixture();
    let wallet = first.anchor.body.wallet_id;
    let mut second = first;
    second.anchor.body.nonce = [9; 32];
    let a = [5; 32];
    let b = [6; 32];
    let frozen = source.capture(wallet, a, first, 10).unwrap();
    source.capture(wallet, b, second, 10).unwrap();
    assert_eq!(source.capture(wallet, a, first, 10).unwrap(), frozen);
    assert!(source.capture(wallet, a, second, 10).is_err());
    assert_eq!(source.observe(&wallet, &a).unwrap().0, first);
    assert_eq!(source.observe(&wallet, &b).unwrap().0, second);
    source.release(&wallet, &b);
    assert!(source.observe(&wallet, &b).is_err());
    assert_eq!(source.observe(&wallet, &a).unwrap().0, first);

    source
        .choices
        .lock()
        .unwrap()
        .get_mut(&(wallet, a))
        .unwrap()
        .1
        .monotonic_ms += 1;
    assert!(source.observe(&wallet, &a).is_err());
    assert!(source.capture(wallet, a, first, 10).is_err());
}

#[test]
fn retired_operations_free_their_capacity() {
    let (source, anchor) = fixture();
    let wallet = anchor.anchor.body.wallet_id;
    for index in 1..=256u32 {
        let mut operation = [0; 32];
        operation[..4].copy_from_slice(&index.to_le_bytes());
        source.capture(wallet, operation, anchor, 10).unwrap();
    }
    assert!(matches!(
        source.capture(wallet, [7; 32], anchor, 10),
        Err(native::Error::ResourceLimit)
    ));
    let mut first = [0; 32];
    first[0] = 1;
    source.release(&wallet, &first);
    source.capture(wallet, [7; 32], anchor, 10).unwrap();
    assert!(source.prepared(&wallet, &first).is_err());
}
