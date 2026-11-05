//! Exact diagnostics and error traits for versioned containers.

use std::{error::Error as _, fmt::Write as _};

use super::{RawVersioned, UnsupportedVersion, error::Error};

fn resource_refusal() -> Error {
    #[derive(Debug, norito::Encode, norito::Decode)]
    #[norito(decode_from_slice)]
    struct Payload(String);
    impl super::Version for Payload {
        fn version(&self) -> u8 {
            1
        }
        fn supported_versions() -> core::ops::Range<u8> {
            1..2
        }
    }

    let mut wire = vec![1];
    wire.extend(norito::codec::encode_adaptive(&Payload(
        "owned text".into(),
    )));
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    norito::with_decode_limits_scope(limits, || {
        super::codec::decode_exact_versioned::<Payload>(&wire).expect_err("allocation refusal")
    })
}

fn cases() -> Vec<(Error, String)> {
    let mut cases = vec![
        (Error::NotVersioned, "Not a versioned object".into()),
        (
            Error::NoritoCodec(norito::Error::Message("bad 🦀\n\"field\"\t\0".into())),
            "Norito (de)serialization issue: bad 🦀\n\"field\"\t\0".into(),
        ),
        (
            resource_refusal(),
            "Norito decoding exceeded its resource limit".into(),
        ),
    ];
    for version in [0, 1, u8::MAX] {
        cases.push((
            Error::UnsupportedVersion(Box::new(UnsupportedVersion::new(
                version,
                RawVersioned::NoritoBytes(b"hidden raw payload".to_vec()),
            ))),
            format!("Input version {version} is unsupported"),
        ));
    }
    for bytes in [0, 1, u64::MAX] {
        cases.push((
            Error::ExtraBytesLeft(bytes),
            format!("Buffer contains {bytes} bytes after decoding"),
        ));
    }
    cases
}

#[test]
fn all_version_error_messages_ignore_outer_flags_and_keep_no_source() {
    for (error, expected) in cases() {
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{error:*^100.3}"), expected);
        assert_eq!(format!("{error:+#0100}"), expected);
        assert!(error.source().is_none());
    }
}

#[test]
fn unsupported_version_diagnostic_preserves_numeric_boundaries_and_hides_raw_bytes() {
    for version in [0, 1, u8::MAX] {
        let error = UnsupportedVersion::new(version, RawVersioned::NoritoBytes(vec![0, 255, 17]));
        let expected = format!("Unsupported version. Expected: 1, got: {version}");
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{error:*^100.3}"), expected);
        assert_eq!(format!("{error:+#0100}"), expected);
        assert!(error.source().is_none());
    }
}

#[test]
fn version_error_traits_and_original_conversions_remain_available() {
    fn assert_error_bounds<T: std::error::Error + Send + Sync + 'static>() {}
    assert_error_bounds::<Error>();
    assert_error_bounds::<UnsupportedVersion>();

    assert!(matches!(Error::from(17_u64), Error::ExtraBytesLeft(17)));
    assert!(matches!(
        Error::from(norito::Error::InvalidMagic),
        Error::NoritoCodec(norito::Error::InvalidMagic),
    ));
    let error = Box::new(UnsupportedVersion::new(
        255,
        RawVersioned::NoritoBytes(vec![17]),
    ));
    let pointer = core::ptr::from_ref(&*error);
    let Error::UnsupportedVersion(converted) = Error::from(error) else {
        panic!("unsupported version conversion");
    };
    assert_eq!(core::ptr::from_ref(&*converted), pointer);
    let error: &dyn std::error::Error = &*converted;
    assert!(error.downcast_ref::<UnsupportedVersion>().is_some());

    let refusal = resource_refusal();
    assert!(matches!(refusal, Error::NoritoResourceLimit(_)));
    assert!(refusal.is_decode_resource_limit());
    assert!(refusal.is_local_resource_refusal());
}

#[test]
fn version_error_formatting_stops_at_the_first_writer_failure() {
    struct Sink {
        remaining: usize,
        failed: bool,
        text: String,
    }
    impl core::fmt::Write for Sink {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            assert!(!self.failed, "formatter wrote after an earlier failure");
            if text.len() > self.remaining {
                self.failed = true;
                return Err(core::fmt::Error);
            }
            self.remaining -= text.len();
            self.text.push_str(text);
            Ok(())
        }
    }
    fn assert_failure_boundary(error: &dyn core::fmt::Display, expected: &str) {
        for remaining in 0..=expected.len() {
            let mut sink = Sink {
                remaining,
                failed: false,
                text: String::new(),
            };
            let result = write!(sink, "{error:*^100.3}");
            assert_eq!(result.is_ok(), remaining >= expected.len());
            assert!(expected.starts_with(&sink.text));
            if result.is_ok() {
                assert_eq!(sink.text, expected);
            }
        }
    }
    for (error, expected) in cases() {
        assert_failure_boundary(&error, &expected);
    }
    let error = UnsupportedVersion::new(255, RawVersioned::NoritoBytes(vec![0]));
    assert_failure_boundary(&error, "Unsupported version. Expected: 1, got: 255");
}
