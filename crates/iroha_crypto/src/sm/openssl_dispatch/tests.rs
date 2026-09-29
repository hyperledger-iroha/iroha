//! Failure injection at the public SM4 dispatch boundary, independent of host capabilities.

use super::*;
use crate::sm::Sm4Key;
use std::cell::RefCell;

#[derive(Clone, Copy)]
enum Fault {
    None,
    Operational,
    Unavailable,
    Disabled,
    InvalidLength,
}

struct Control {
    state: openssl_provider::RuntimeState,
    fault: Fault,
    attempts: [usize; 4],
    canonical: [usize; 4],
}

thread_local! {
    // The same runtime-state implementation is isolated per test so injected
    // faults cannot quarantine the real provider used by concurrent tests.
    static CONTROL: RefCell<Option<Control>> = const { RefCell::new(None) };
}

pub(super) fn active() -> bool {
    CONTROL.with_borrow(Option::is_some)
}

pub(super) fn with_state<T>(action: impl FnOnce(&openssl_provider::RuntimeState) -> T) -> T {
    CONTROL.with_borrow(|control| action(&control.as_ref().expect("active control").state))
}

pub(super) fn intercept(operation: Operation) -> Option<OpenSslSmError> {
    CONTROL.with_borrow_mut(|control| {
        let control = control.as_mut()?;
        control.attempts[operation as usize] += 1;
        match control.fault {
            Fault::None => None,
            Fault::Operational => Some(OpenSslSmError::OpenSsl(openssl::error::ErrorStack::get())),
            Fault::Unavailable => Some(match operation {
                Operation::GcmEncrypt | Operation::GcmDecrypt => {
                    OpenSslSmError::Sm4GcmNotImplemented
                }
                #[cfg(feature = "sm-ccm")]
                Operation::CcmEncrypt | Operation::CcmDecrypt => {
                    OpenSslSmError::Sm4CcmNotImplemented
                }
            }),
            Fault::Disabled => Some(OpenSslSmError::PreviewDisabled),
            Fault::InvalidLength => Some(OpenSslSmError::InvalidKeyLength(15)),
        }
    })
}

pub(super) fn record_canonical(operation: Operation) {
    CONTROL.with_borrow_mut(|control| {
        if let Some(control) = control {
            control.canonical[operation as usize] += 1;
        }
    });
}

struct Guard;

impl Guard {
    fn new() -> Self {
        CONTROL.with_borrow_mut(|control| {
            assert!(control.is_none(), "dispatch controls cannot nest");
            *control = Some(Control {
                state: openssl_provider::RuntimeState::default(),
                fault: Fault::None,
                attempts: [0; 4],
                canonical: [0; 4],
            });
        });
        Self
    }

    fn set(enabled: bool, fault: Fault) {
        CONTROL.with_borrow_mut(|control| {
            let control = control.as_mut().expect("active control");
            control.state.set_enabled(enabled);
            control.fault = fault;
            control.attempts.fill(0);
            control.canonical.fill(0);
        });
    }

    fn counts(operation: Operation) -> (usize, usize) {
        CONTROL.with_borrow(|control| {
            let control = control.as_ref().expect("active control");
            (
                control.attempts[operation as usize],
                control.canonical[operation as usize],
            )
        })
    }

    fn healthy(operation: Operation) -> bool {
        with_state(|state| state.can_attempt(operation))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        CONTROL.with_borrow_mut(|control| *control = None);
    }
}

#[test]
fn gcm_encrypt_failure_returns_canonical_bytes_and_quarantines_only_encrypt() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x41; 16]);
    let nonce = [0x13; 12];
    let aad = b"authenticated context";
    let plaintext = b"original borrowed input survives the failed attempt";
    let expected = key.encrypt_gcm(&nonce, aad, plaintext);
    assert!(expected.is_ok());
    Guard::set(true, Fault::Operational);
    assert_eq!(key.encrypt_gcm(&nonce, aad, plaintext), expected);
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (1, 1));
    assert!(!Guard::healthy(Operation::GcmEncrypt));
    assert!(Guard::healthy(Operation::GcmDecrypt));
    assert_eq!(key.encrypt_gcm(&nonce, aad, plaintext), expected);
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (1, 2));
    // Reloading opt-out/opt-in must not clear a process-lifetime quarantine.
    Guard::set(false, Fault::Operational);
    Guard::set(true, Fault::Operational);
    assert_eq!(key.encrypt_gcm(&nonce, aad, plaintext), expected);
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (0, 1));
}

#[test]
fn gcm_decrypt_failure_recomputes_original_inputs_before_returning_plaintext() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x72; 16]);
    let nonce = [0x24; 12];
    let (ciphertext, tag) = key.encrypt_gcm(&nonce, b"aad", b"message").unwrap();
    let expected = key.decrypt_gcm(&nonce, b"aad", &ciphertext, &tag);
    assert_eq!(expected.as_deref(), Ok(b"message".as_slice()));
    Guard::set(true, Fault::Operational);
    assert_eq!(key.decrypt_gcm(&nonce, b"aad", &ciphertext, &tag), expected);
    assert_eq!(Guard::counts(Operation::GcmDecrypt), (1, 1));
    assert!(!Guard::healthy(Operation::GcmDecrypt));
    assert!(Guard::healthy(Operation::GcmEncrypt));
    assert_eq!(key.decrypt_gcm(&nonce, b"aad", &ciphertext, &tag), expected);
    assert_eq!(Guard::counts(Operation::GcmDecrypt), (1, 2));
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_encrypt_failure_returns_canonical_bytes_and_quarantines_only_encrypt() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x53; 16]);
    let nonce = [0x35; 13];
    let expected = key.encrypt_ccm(&nonce, b"aad", b"message", 12);
    assert!(expected.is_ok());
    Guard::set(true, Fault::Operational);
    assert_eq!(key.encrypt_ccm(&nonce, b"aad", b"message", 12), expected);
    assert_eq!(Guard::counts(Operation::CcmEncrypt), (1, 1));
    assert!(!Guard::healthy(Operation::CcmEncrypt));
    assert!(Guard::healthy(Operation::CcmDecrypt));
    assert!(Guard::healthy(Operation::GcmEncrypt));
    assert_eq!(key.encrypt_ccm(&nonce, b"aad", b"message", 12), expected);
    assert_eq!(Guard::counts(Operation::CcmEncrypt), (1, 2));
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_decrypt_failure_recomputes_original_inputs_before_returning_plaintext() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x64; 16]);
    let nonce = [0x46; 13];
    let (ciphertext, tag) = key.encrypt_ccm(&nonce, b"aad", b"message", 10).unwrap();
    let expected = key.decrypt_ccm(&nonce, b"aad", &ciphertext, &tag);
    assert_eq!(expected.as_deref(), Ok(b"message".as_slice()));
    Guard::set(true, Fault::Operational);
    assert_eq!(key.decrypt_ccm(&nonce, b"aad", &ciphertext, &tag), expected);
    assert_eq!(Guard::counts(Operation::CcmDecrypt), (1, 1));
    assert!(!Guard::healthy(Operation::CcmDecrypt));
    assert!(Guard::healthy(Operation::CcmEncrypt));
    assert!(Guard::healthy(Operation::GcmDecrypt));
    assert_eq!(key.decrypt_ccm(&nonce, b"aad", &ciphertext, &tag), expected);
    assert_eq!(Guard::counts(Operation::CcmDecrypt), (1, 2));
}

#[test]
fn gcm_invalid_tag_nonce_aad_and_ciphertext_keep_canonical_errors_without_quarantine() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x75; 16]);
    let nonce = [0x57; 12];
    let (ciphertext, tag) = key.encrypt_gcm(&nonce, b"aad", b"message").unwrap();
    for field in 0..4 {
        let mut nonce = nonce;
        let mut tag = tag;
        let mut ciphertext = ciphertext.clone();
        let mut aad = b"aad".to_vec();
        match field {
            0 => tag[0] ^= 1,
            1 => nonce[0] ^= 1,
            2 => aad[0] ^= 1,
            _ => ciphertext[0] ^= 1,
        }
        Guard::set(false, Fault::None);
        let expected = key.decrypt_gcm(&nonce, &aad, &ciphertext, &tag);
        assert!(expected.is_err());
        Guard::set(true, Fault::Operational);
        assert_eq!(key.decrypt_gcm(&nonce, &aad, &ciphertext, &tag), expected);
        assert_eq!(Guard::counts(Operation::GcmDecrypt), (1, 1));
        assert!(Guard::healthy(Operation::GcmDecrypt));
    }
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_invalid_tag_nonce_aad_and_ciphertext_keep_canonical_errors_without_quarantine() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x86; 16]);
    let nonce = [0x68; 13];
    let (ciphertext, tag) = key.encrypt_ccm(&nonce, b"aad", b"message", 8).unwrap();
    for field in 0..4 {
        let mut nonce = nonce;
        let mut tag = tag.clone();
        let mut ciphertext = ciphertext.clone();
        let mut aad = b"aad".to_vec();
        match field {
            0 => tag[0] ^= 1,
            1 => nonce[0] ^= 1,
            2 => aad[0] ^= 1,
            _ => ciphertext[0] ^= 1,
        }
        Guard::set(false, Fault::None);
        let expected = key.decrypt_ccm(&nonce, &aad, &ciphertext, &tag);
        assert!(expected.is_err());
        Guard::set(true, Fault::Operational);
        assert_eq!(key.decrypt_ccm(&nonce, &aad, &ciphertext, &tag), expected);
        assert_eq!(Guard::counts(Operation::CcmDecrypt), (1, 1));
        assert!(Guard::healthy(Operation::CcmDecrypt));
    }
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_malformed_nonce_and_tag_lengths_reject_before_backend_dispatch() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0x97; 16]);
    for (nonce_len, tag_len) in [(6, 16), (14, 16), (13, 3), (13, 5), (13, 17)] {
        let nonce = vec![0; nonce_len];
        let tag = vec![0; tag_len];
        Guard::set(false, Fault::None);
        let expected_encrypt = key.encrypt_ccm(&nonce, b"aad", b"message", tag_len);
        let expected_decrypt = key.decrypt_ccm(&nonce, b"aad", b"message", &tag);
        assert!(expected_encrypt.is_err());
        assert!(expected_decrypt.is_err());
        Guard::set(true, Fault::Operational);
        assert_eq!(
            key.encrypt_ccm(&nonce, b"aad", b"message", tag_len),
            expected_encrypt
        );
        assert_eq!(
            key.decrypt_ccm(&nonce, b"aad", b"message", &tag),
            expected_decrypt
        );
        assert_eq!(Guard::counts(Operation::CcmEncrypt), (0, 0));
        assert_eq!(Guard::counts(Operation::CcmDecrypt), (0, 0));
        assert!(Guard::healthy(Operation::CcmEncrypt));
        assert!(Guard::healthy(Operation::CcmDecrypt));
    }
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_unrepresentable_payload_is_canonical_input_error_not_backend_quarantine() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0xa8; 16]);
    let nonce = [0x8a; 13];
    let payload = vec![0; 65_536];
    let expected = key.encrypt_ccm(&nonce, b"aad", &payload, 16);
    assert!(expected.is_err());
    Guard::set(true, Fault::Operational);
    assert_eq!(key.encrypt_ccm(&nonce, b"aad", &payload, 16), expected);
    assert_eq!(Guard::counts(Operation::CcmEncrypt), (1, 1));
    assert!(Guard::healthy(Operation::CcmEncrypt));
}

#[test]
fn unavailable_or_concurrently_disabled_provider_falls_back_without_quarantine() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0xb9; 16]);
    let nonce = [0x9b; 12];
    let expected = key.encrypt_gcm(&nonce, b"aad", b"message");
    for fault in [Fault::Unavailable, Fault::Disabled] {
        Guard::set(true, fault);
        assert_eq!(key.encrypt_gcm(&nonce, b"aad", b"message"), expected);
        assert_eq!(Guard::counts(Operation::GcmEncrypt), (1, 1));
        assert!(Guard::healthy(Operation::GcmEncrypt));
    }
}

#[test]
fn backend_length_rejection_of_canonical_input_is_operational_failure() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0xca; 16]);
    let nonce = [0xac; 12];
    let expected = key.encrypt_gcm(&nonce, b"aad", b"message");
    Guard::set(true, Fault::InvalidLength);
    assert_eq!(key.encrypt_gcm(&nonce, b"aad", b"message"), expected);
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (1, 1));
    assert!(!Guard::healthy(Operation::GcmEncrypt));
}

#[test]
fn provider_success_does_not_recompute_canonical_result() {
    let _guard = Guard::new();
    Guard::set(true, Fault::None);
    assert_eq!(
        execute(
            Operation::GcmEncrypt,
            || Ok(42),
            || panic!("success must not recompute")
        ),
        Ok(42)
    );
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (1, 0));
    assert!(Guard::healthy(Operation::GcmEncrypt));
}

#[test]
fn config_opt_out_avoids_the_backend_entirely() {
    let _guard = Guard::new();
    assert_eq!(
        execute(
            Operation::GcmEncrypt,
            || panic!("disabled backend must not run"),
            || Ok(7)
        ),
        Ok(7)
    );
    assert_eq!(Guard::counts(Operation::GcmEncrypt), (0, 1));
}

#[cfg(feature = "sm-ccm")]
#[test]
fn ccm_unavailable_provider_falls_back_without_quarantine() {
    let _guard = Guard::new();
    let key = Sm4Key::new([0xdb; 16]);
    let nonce = [0xbd; 13];
    let expected = key.encrypt_ccm(&nonce, b"aad", b"message", 16);
    let (ciphertext, tag) = expected.as_ref().unwrap();
    Guard::set(true, Fault::Unavailable);
    assert_eq!(key.encrypt_ccm(&nonce, b"aad", b"message", 16), expected);
    assert_eq!(
        key.decrypt_ccm(&nonce, b"aad", ciphertext, tag).as_deref(),
        Ok(b"message".as_slice())
    );
    assert_eq!(Guard::counts(Operation::CcmEncrypt), (1, 1));
    assert_eq!(Guard::counts(Operation::CcmDecrypt), (1, 1));
    assert!(Guard::healthy(Operation::CcmEncrypt));
    assert!(Guard::healthy(Operation::CcmDecrypt));
}

#[test]
fn concurrent_quarantines_retain_each_operation_across_config_reload() {
    let state = openssl_provider::RuntimeState::default();
    state.set_enabled(true);
    let operations = [
        Operation::GcmEncrypt,
        Operation::GcmDecrypt,
        #[cfg(feature = "sm-ccm")]
        Operation::CcmEncrypt,
        #[cfg(feature = "sm-ccm")]
        Operation::CcmDecrypt,
    ];
    std::thread::scope(|scope| {
        for operation in operations {
            let state = &state;
            scope.spawn(move || state.quarantine(operation));
        }
    });
    state.set_enabled(false);
    state.set_enabled(true);
    for operation in operations {
        assert!(!state.can_attempt(operation));
    }
}
