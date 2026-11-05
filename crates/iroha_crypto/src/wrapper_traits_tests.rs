//! Formatting and borrowed-wrapper contracts independent of macro generation.

use std::{
    error::Error as _,
    fmt,
    fmt::Write as _,
    ops::{Deref, DerefMut},
};

use crate::{
    Algorithm, CryptoOperation, Error, Hash, HashOf, Signature, SignatureOf,
    UnsupportedImplementation,
    error::{NoSuchAlgorithm, ParseError},
};

struct NoTraits;

struct FailingWriter;
impl fmt::Write for FailingWriter {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Err(fmt::Error)
    }
}

fn assert_composite_display(value: &impl fmt::Display, expected: &str) {
    assert_eq!(format!("{value}"), expected);
    assert_eq!(format!("{value:>120.3}"), expected);
    assert_eq!(format!("{value:🦀^120.3}"), expected);
    assert_eq!(format!("{value:+#0120.3}"), expected);
    assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
}

fn assert_string_display(value: &impl fmt::Display, expected: &str) {
    assert_eq!(format!("{value}"), expected);
    assert_eq!(format!("{value:>120.3}"), format!("{expected:>120.3}"));
    assert_eq!(format!("{value:🦀^120.3}"), format!("{expected:🦀^120.3}"));
    assert_eq!(format!("{value:+#0120.3}"), format!("{expected:+#0120.3}"));
    if !expected.is_empty() {
        assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
    }
}

#[test]
fn hash_wrapper_formatting_preserves_hex_flags_and_unbounded_marker() {
    let bytes = std::array::from_fn(|index| u8::try_from(index).unwrap());
    let hash = Hash::from_marked_bytes(bytes).expect("odd final byte");
    let expected = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    assert_string_display(&hash, expected);
    assert_eq!(format!("{hash:?}"), expected);
    assert_eq!(format!("{hash:>120.3?}"), format!("{expected:>120.3}"));
    assert_eq!(format!("{hash:🦀^120.3?}"), format!("{expected:🦀^120.3}"));
    assert_eq!(format!("{hash:+#0120.3?}"), format!("{expected:+#0120.3}"));
    assert_eq!(write!(FailingWriter, "{hash:?}"), Err(fmt::Error));

    // The marker carries no Display/Debug bound.
    let typed = HashOf::<NoTraits>::from_untyped_unchecked(hash);
    assert_string_display(&typed, expected);
    let debug = format!(
        "{{ {} {expected} }}",
        core::any::type_name::<HashOf<NoTraits>>()
    );
    assert_eq!(format!("{typed:?}"), debug);
    assert_eq!(format!("{typed:>120.3?}"), debug);
    assert_eq!(format!("{typed:🦀^120.3?}"), debug);
    assert_eq!(format!("{typed:+#0120.3?}"), debug);
    assert_eq!(write!(FailingWriter, "{typed:?}"), Err(fmt::Error));
}

#[test]
fn hash_formatting_preserves_partial_writer_calls_and_failures() {
    #[derive(Debug, PartialEq, Eq)]
    struct TraceWriter {
        remaining: usize,
        calls: Vec<String>,
    }
    impl fmt::Write for TraceWriter {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            self.calls.push(text.into());
            self.remaining = self.remaining.checked_sub(text.len()).ok_or(fmt::Error)?;
            Ok(())
        }
    }

    fn check(
        actual: impl Fn(&mut TraceWriter) -> fmt::Result,
        expected: impl Fn(&mut TraceWriter) -> fmt::Result,
    ) {
        for remaining in [0, 1, 2, 3, 5, 20, 63, 64, 70, 120, 512] {
            let mut actual_writer = TraceWriter {
                remaining,
                calls: Vec::new(),
            };
            let mut expected_writer = TraceWriter {
                remaining,
                calls: Vec::new(),
            };
            assert_eq!(actual(&mut actual_writer), expected(&mut expected_writer));
            assert_eq!(actual_writer, expected_writer);
        }
    }

    let hash = Hash::prehashed([0xa5; Hash::LENGTH]);
    let expected = "a5".repeat(Hash::LENGTH);
    check(|w| write!(w, "{hash}"), |w| write!(w, "{expected}"));
    check(|w| write!(w, "{hash:?}"), |w| write!(w, "{expected}"));
    check(
        |w| write!(w, "{hash:>120.3}"),
        |w| write!(w, "{expected:>120.3}"),
    );
    check(
        |w| write!(w, "{hash:>120.3?}"),
        |w| write!(w, "{expected:>120.3}"),
    );
    check(
        |w| write!(w, "{hash:🦀^120.3}"),
        |w| write!(w, "{expected:🦀^120.3}"),
    );
    check(
        |w| write!(w, "{hash:🦀^120.3?}"),
        |w| write!(w, "{expected:🦀^120.3}"),
    );
    check(
        |w| write!(w, "{hash:+#0120.3}"),
        |w| write!(w, "{expected:+#0120.3}"),
    );
    check(
        |w| write!(w, "{hash:+#0120.3?}"),
        |w| write!(w, "{expected:+#0120.3}"),
    );
}

#[test]
fn hash_wrapper_deref_borrows_and_updates_its_stored_hash() {
    fn assert_target<T: Deref<Target = Hash> + DerefMut>() {}
    assert_target::<HashOf<NoTraits>>();
    let mut typed = HashOf::<NoTraits>::from_untyped_unchecked(Hash::prehashed([1; Hash::LENGTH]));
    let stored = std::ptr::from_ref(&typed).cast::<Hash>();
    let shared: &Hash = &typed;
    assert!(std::ptr::eq(shared, stored));
    let exclusive: &mut Hash = &mut typed;
    assert!(std::ptr::eq(exclusive, stored));
    let replacement = Hash::prehashed([0xa5; Hash::LENGTH]);
    *exclusive = replacement;
    assert_eq!(Hash::from(typed), replacement);
}

#[test]
fn signature_debug_preserves_uppercase_hex_and_braces() {
    for (payload, expected) in [(&[][..], "{  }"), (&[0, 1, 0xab, 0xff][..], "{ 0001ABFF }")] {
        let signature = Signature::from_bytes(payload);
        assert_eq!(format!("{signature:?}"), expected);
        assert_eq!(format!("{signature:>120.3?}"), expected);
        assert_eq!(format!("{signature:🦀^120.3?}"), expected);
        assert_eq!(format!("{signature:+#0120.3?}"), expected);
        assert_eq!(write!(FailingWriter, "{signature:?}"), Err(fmt::Error));
    }
}

#[test]
fn signature_debug_preserves_every_byte_and_chunk_boundary() {
    for length in [0, 1, 63, 64, 127, 128, 129, 255, 256, 257, 3309, 8193] {
        let payload: Vec<u8> = (0..length).map(|index| (index % 256) as u8).collect();
        let signature = Signature::from_bytes(&payload);
        let expected = format!("{{ {} }}", hex::encode_upper(&payload));
        assert_eq!(format!("{signature:?}"), expected);
        assert_eq!(format!("{signature:#?}"), expected);
        assert_eq!(format!("{signature:🦀^120.3?}"), expected);
        assert_eq!(format!("{signature:+#0120.3?}"), expected);
        assert_eq!(signature.payload(), payload);
    }
}

#[test]
fn signature_debug_stops_at_writer_failure() {
    struct LimitedWriter {
        remaining: usize,
        failed: bool,
    }
    impl fmt::Write for LimitedWriter {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            assert!(!self.failed, "formatting continued after the writer failed");
            if text.len() > self.remaining {
                self.failed = true;
                Err(fmt::Error)
            } else {
                self.remaining -= text.len();
                Ok(())
            }
        }
    }

    for length in [0, 1, 64, 127, 128, 129, 3309] {
        let signature = Signature::from_bytes(&vec![0xab; length]);
        let output_length = 4 + length * 2;
        for limit in [0, 1, 2, output_length / 2, output_length - 1, output_length] {
            let mut writer = LimitedWriter {
                remaining: limit,
                failed: false,
            };
            let result = write!(&mut writer, "{signature:?}");
            assert_eq!(result.is_ok(), limit == output_length);
            assert_eq!(writer.failed, limit < output_length);
        }
    }
}

#[test]
fn signature_wrapper_deref_borrows_and_updates_its_stored_signature() {
    fn assert_target<T: Deref<Target = Signature> + DerefMut>() {}
    assert_target::<SignatureOf<NoTraits>>();
    let mut typed = SignatureOf::<NoTraits>::from_signature(Signature::from_bytes(&[1, 2, 3]));
    let stored = std::ptr::from_ref(&typed).cast::<Signature>();
    let shared: &Signature = &typed;
    assert!(std::ptr::eq(shared, stored));
    let exclusive: &mut Signature = &mut typed;
    assert!(std::ptr::eq(exclusive, stored));
    let replacement = Signature::from_bytes(&[0xab, 0xff]);
    *exclusive = replacement.clone();
    assert_eq!(Signature::from(typed), replacement);
}

#[cfg(feature = "sm")]
#[test]
fn sm3_digest_deref_borrows_and_updates_the_original_byte_array() {
    use crate::sm::Sm3Digest;

    fn assert_target<T: Deref<Target = [u8; 32]> + DerefMut>() {}
    assert_target::<Sm3Digest>();
    let mut digest = Sm3Digest::hash(b"abc");
    let stored = std::ptr::from_ref(digest.as_bytes());
    let shared: &[u8; 32] = &digest;
    assert!(std::ptr::eq(shared, stored));
    let exclusive: &mut [u8; 32] = &mut digest;
    assert!(std::ptr::eq(exclusive, stored));
    *exclusive = [0xa5; 32];
    assert_eq!(digest.as_bytes(), &[0xa5; 32]);
}

#[test]
fn parse_error_display_preserves_transparent_string_flags_and_source_contract() {
    for message in ["", "malformed", "μ🔥\nquote\"\\"] {
        let error = ParseError::Invalid(message.into());
        assert_string_display(&error, message);
        assert!(error.source().is_none());
    }
    let error = ParseError::UnsupportedImplementation(UnsupportedImplementation {
        algorithm: Algorithm::Ed25519,
        operation: CryptoOperation::Sign,
    });
    assert_composite_display(&error, "signing implementation unavailable for ed25519");
    assert!(error.source().is_none());
}

#[test]
fn crypto_error_display_preserves_all_variants_without_sources() {
    assert_composite_display(&NoSuchAlgorithm, "Algorithm not supported");
    assert!(NoSuchAlgorithm.source().is_none());
    for (error, expected) in [
        (
            Error::UnsupportedImplementation(UnsupportedImplementation {
                algorithm: Algorithm::Ed25519,
                operation: CryptoOperation::Sign,
            }),
            "signing implementation unavailable for ed25519",
        ),
        (
            Error::NoSuchAlgorithm("μ\n".into()),
            "Algorithm 'μ\n' is not supported",
        ),
        (
            Error::Parse(ParseError::Invalid("μ\n".into())),
            "Key could not be parsed. μ\n",
        ),
        (Error::Signing("μ\n".into()), "Signing failed. μ\n"),
        (Error::BadSignature, "Signature verification failed"),
        (Error::KeyGen("μ\n".into()), "Key generation failed. μ\n"),
        (Error::Other("μ\n".into()), "General error. μ\n"),
    ] {
        assert_composite_display(&error, expected);
        assert!(error.source().is_none());
    }
}

#[test]
fn crypto_error_conversions_preserve_original_variants_and_payloads() {
    let unavailable = UnsupportedImplementation {
        algorithm: Algorithm::Ed25519,
        operation: CryptoOperation::Sign,
    };
    assert!(matches!(
        Error::from(ParseError::UnsupportedImplementation(unavailable)),
        Error::UnsupportedImplementation(original) if original == unavailable
    ));
    assert!(matches!(
        Error::from(ParseError::Invalid("μ\n".into())),
        Error::Parse(ParseError::Invalid(message)) if message == "μ\n"
    ));
    assert!(matches!(
        Error::from(NoSuchAlgorithm),
        Error::NoSuchAlgorithm(message) if message == "Algorithm not supported"
    ));
}
