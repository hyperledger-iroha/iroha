//! Tests covering `SessionKey` zeroization behavior.
use iroha_crypto::SessionKey;

#[test]
fn session_key_zeroizes_on_drop() {
    iroha_crypto::__debug_clear_last_zeroized_session_key();

    let original = vec![0xAA; 32];
    {
        let session_key = SessionKey::new(original.clone());
        assert_eq!(session_key.payload(), original.as_slice());
    }

    let recorded = iroha_crypto::__debug_last_zeroized_session_key();
    assert_eq!(recorded.len(), original.len());
    assert!(recorded.iter().all(|&byte| byte == 0));
}
