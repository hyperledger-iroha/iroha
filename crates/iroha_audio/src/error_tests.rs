//! Error text and source contracts for all deterministic audio backends.

use std::{error::Error as _, fmt, fmt::Write as _};

use super::{ChannelLayout, CodecError};

fn cases() -> Vec<(CodecError, String)> {
    use CodecError::*;

    let mut cases = vec![
        (
            InvalidPcmLength {
                expected: usize::MAX,
                found: 0,
            },
            format!("expected {} PCM samples, got 0", usize::MAX),
        ),
        (
            UnsupportedVersion(u8::MAX),
            "unsupported audio payload version 255".into(),
        ),
        (PacketTooShort, "audio packet too short".into()),
        (
            InvalidChannelCount {
                expected: None,
                found: u8::MAX,
            },
            "invalid channel count: expected None, found 255".into(),
        ),
        (
            InvalidChannelCount {
                expected: Some(u8::MAX),
                found: 0,
            },
            "invalid channel count: expected Some(255), found 0".into(),
        ),
        (
            InvalidSampleCount {
                expected: u16::MAX,
                found: 0,
            },
            "invalid frame sample count: expected 65535, found 0".into(),
        ),
    ];
    for (layout, name) in [
        (ChannelLayout::Mono, "Mono"),
        (ChannelLayout::Stereo, "Stereo"),
        (ChannelLayout::FirstOrderAmbisonics, "FirstOrderAmbisonics"),
    ] {
        cases.push((
            BackendUnavailable { layout },
            format!("audio backend unavailable for layout {name}"),
        ));
        cases.push((
            UnsupportedLayout { layout },
            format!("audio backend does not support layout {name}"),
        ));
    }
    for message in ["", "backend μ\n🦀\0\"", "invalid argument"] {
        cases.push((
            LibopusError {
                message: message.into(),
            },
            format!("libopus backend error: {message}"),
        ));
    }
    cases
}

#[test]
fn codec_errors_preserve_text_flags_and_sources() {
    fn assert_error_traits<T: std::error::Error + Send + Sync + 'static>() {}
    assert_error_traits::<CodecError>();

    for (error, expected) in cases() {
        assert_eq!(error.to_string(), expected);
        assert_eq!(format!("{error:>120.3}"), expected);
        assert_eq!(format!("{error:🦀^120.3}"), expected);
        assert_eq!(format!("{error:+#0120.3}"), expected);
        assert!(error.source().is_none());
        assert_eq!(error.clone(), error);
    }
}

#[test]
fn codec_errors_stop_on_the_first_writer_failure() {
    struct LimitedWriter {
        remaining: usize,
        failed: bool,
    }

    impl fmt::Write for LimitedWriter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            assert!(!self.failed, "formatter continued after writer failure");
            if value.len() > self.remaining {
                self.failed = true;
                return Err(fmt::Error);
            }
            self.remaining -= value.len();
            Ok(())
        }
    }

    for (error, expected) in cases() {
        for remaining in 0..expected.len() {
            let mut writer = LimitedWriter {
                remaining,
                failed: false,
            };
            assert!(write!(writer, "{error}").is_err());
            assert!(writer.failed);
        }
        let mut writer = LimitedWriter {
            remaining: expected.len(),
            failed: false,
        };
        assert!(write!(writer, "{error}").is_ok());
        assert_eq!(writer.remaining, 0);
        assert!(!writer.failed);
    }
}
