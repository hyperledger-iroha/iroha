//! Compact ticket signature errors preserve their owned causes and diagnostics.

use iroha_data_model::soranet::ticket::{TicketCommitmentError, TicketSignatureError};

#[test]
fn ticket_signature_errors_preserve_owned_details_and_source() {
    let mismatch = TicketCommitmentError::Mismatch {
        expected: [0x11; 32],
        actual: [0x22; 32],
    };
    let commitment = TicketSignatureError::from(mismatch);
    assert_eq!(commitment.to_string(), mismatch.to_string());
    assert!(matches!(
        &commitment,
        TicketSignatureError::Commitment(error) if **error == mismatch
    ));
    // Transparent commitment errors preserve the wrapped error's source.
    assert!(std::error::Error::source(&commitment).is_none());

    let crypto = iroha_crypto::Error::Other("owned ticket verification detail".to_owned());
    let crypto_display = crypto.to_string();
    let signature = TicketSignatureError::from(crypto);
    assert_eq!(
        signature.to_string(),
        format!("ticket signature verification failed: {crypto_display}")
    );
    let source = std::error::Error::source(&signature)
        .expect("signature error retains its crypto source")
        .downcast_ref::<iroha_crypto::Error>()
        .expect("source remains the original typed crypto error");
    assert_eq!(
        source,
        &iroha_crypto::Error::Other("owned ticket verification detail".to_owned())
    );
}
#[test]
fn ticket_signature_error_fits_two_pointer_words() {
    // Both error payloads remain out of line on 32-bit Wasm and native
    // targets, so either failure carries only a tag and an owned pointer.
    assert!(core::mem::size_of::<TicketSignatureError>() <= 2 * core::mem::size_of::<usize>());
}
