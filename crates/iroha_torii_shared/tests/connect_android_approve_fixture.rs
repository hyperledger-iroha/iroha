//! Interop fixture checks for Android-emitted Connect approve frames.

use base64::Engine as _;
use iroha_torii_shared::connect::{ConnectControlV1, FrameKind, decode_connect_frame_bare};

#[test]
fn decodes_android_approve_frame_fixture() {
    let hex = include_str!("fixtures/android_approve_frame.hex").trim();
    let bytes = hex::decode(hex).expect("fixture hex should decode");
    let frame = decode_connect_frame_bare(&bytes).expect("android approve frame should decode");

    assert_eq!(frame.seq, 1);
    assert_eq!(frame.sid, [0xCDu8; 32]);
    assert!(matches!(frame.dir, iroha_torii_shared::connect::Dir::WalletToApp));

    match frame.kind {
        FrameKind::Control(ConnectControlV1::Approve {
            wallet_pk,
            account_id,
            permissions,
            proof,
            sig_wallet,
        }) => {
            assert_eq!(wallet_pk, [0x07u8; 32]);
            assert_eq!(
                account_id,
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            );
            assert_eq!(permissions, None);
            assert_eq!(proof, None);
            assert_eq!(sig_wallet.bytes(), [0x09u8; 64]);
        }
        _ => panic!("expected control approve frame"),
    }
}

#[test]
fn fixture_base64_roundtrip_sanity() {
    let hex = include_str!("fixtures/android_approve_frame.hex").trim();
    let bytes = hex::decode(hex).expect("fixture hex should decode");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 decode");
    assert_eq!(decoded, bytes);
}
