//! Encryption error text, formatter failures, and original source ownership.

use std::{error::Error as _, fmt, fmt::Write as _};

use super::{Error, SysError};

fn error_cases() -> [(Error, &'static str); 6] {
    [
        (
            Error::NonceGeneration(SysError::UNSUPPORTED),
            "Failed to generate nonce for an encryption operation",
        ),
        (
            Error::InertNonce,
            "Generated nonce material must not be an inert all-zero placeholder",
        ),
        (Error::Encryption(aead::Error), "Failed to encrypt data"),
        (Error::Decryption(aead::Error), "Failed to decrypt data"),
        (Error::NotEnoughData, "Not enough data to decrypt message"),
        (
            Error::InvalidKeyLength,
            "Provided key material has an invalid length",
        ),
    ]
}

#[test]
fn fixed_error_messages_preserve_text_and_ignore_outer_formatting_flags() {
    for (error, expected) in error_cases() {
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{error:>100.3}"), expected);
        assert_eq!(format!("{error:🦀^100.3}"), expected);
        assert_eq!(format!("{error:+#0100.3}"), expected);
    }
}

#[test]
fn only_nonce_generation_exposes_its_original_source() {
    for (error, _) in error_cases() {
        if let Error::NonceGeneration(inner) = &error {
            let source = error.source().expect("nonce generation retains its source");
            assert!(std::ptr::eq(
                source
                    .downcast_ref::<SysError>()
                    .expect("original RNG error"),
                inner,
            ));
            assert_eq!(source.to_string(), inner.to_string());
            assert!(source.source().is_none());
        } else {
            assert!(error.source().is_none(), "unexpected source for {error:?}");
        }
    }
}

#[test]
fn fixed_error_messages_propagate_formatter_failures() {
    struct FailingWriter;
    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    for (error, _) in error_cases() {
        assert_eq!(write!(FailingWriter, "{error}"), Err(fmt::Error));
    }
}
