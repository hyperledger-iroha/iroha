//! The fixed ANSI styles used by Kagami, formatted directly into the caller's writer.
//!
//! Callers retain control of terminal detection and plain-output policy. The escape ordering
//! preserves the existing nested styles, including their style and foreground resets.

use std::fmt;

/// A style combination used by a Kagami status, diagnostic or prompt.
#[derive(Clone, Copy)]
pub(crate) enum Style {
    /// Bold text.
    Bold,
    /// Italic text inside a cyan foreground.
    CyanItalic,
    /// Blue foreground inside bold text.
    BoldBlue,
}

impl Style {
    /// Wrap a display value without allocating or rendering it in advance.
    pub(crate) const fn apply<T>(self, value: T) -> Styled<T> {
        Styled { style: self, value }
    }
}

/// A display value and its fixed surrounding ANSI escapes.
#[derive(Clone, Copy)]
pub(crate) struct Styled<T> {
    style: Style,
    value: T,
}

impl<T: fmt::Display> fmt::Display for Styled<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (prefix, suffix) = match self.style {
            Style::Bold => ("\x1b[1m", "\x1b[0m"),
            Style::CyanItalic => ("\x1b[36m\x1b[3m", "\x1b[0m\x1b[39m"),
            Style::BoldBlue => ("\x1b[1m\x1b[34m", "\x1b[39m\x1b[0m"),
        };
        formatter.write_str(prefix)?;
        self.value.fmt(formatter)?;
        formatter.write_str(suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn styles_preserve_exact_escape_and_reset_order() {
        for (style, expected) in [
            (Style::Bold, "\x1b[1m日本語 🦀\x1b[0m"),
            (Style::CyanItalic, "\x1b[36m\x1b[3m日本語 🦀\x1b[0m\x1b[39m"),
            (Style::BoldBlue, "\x1b[1m\x1b[34m日本語 🦀\x1b[39m\x1b[0m"),
        ] {
            assert_eq!(style.apply("日本語 🦀").to_string(), expected);
        }
        assert_eq!(Style::Bold.apply("").to_string(), "\x1b[1m\x1b[0m");
        assert_eq!(
            Style::CyanItalic.apply("a\x1b[0mb").to_string(),
            "\x1b[36m\x1b[3ma\x1b[0mb\x1b[0m\x1b[39m"
        );
    }

    struct StackWriter {
        bytes: [u8; 64],
        len: usize,
    }

    impl fmt::Write for StackWriter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
            self.bytes
                .get_mut(self.len..end)
                .ok_or(fmt::Error)?
                .copy_from_slice(value.as_bytes());
            self.len = end;
            Ok(())
        }
    }

    #[test]
    fn every_unicode_scalar_passes_through_unchanged() {
        for character in (0..=0x10_ffff).filter_map(char::from_u32) {
            let mut output = StackWriter {
                bytes: [0; 64],
                len: 0,
            };
            write!(output, "{}", Style::Bold.apply(character)).unwrap();
            let mut utf8 = [0; 4];
            let encoded = character.encode_utf8(&mut utf8).as_bytes();
            assert_eq!(&output.bytes[..4], b"\x1b[1m");
            assert_eq!(&output.bytes[4..4 + encoded.len()], encoded);
            assert_eq!(&output.bytes[4 + encoded.len()..output.len], b"\x1b[0m");
        }
    }

    #[test]
    fn numeric_and_argument_values_format_directly_into_a_stack_writer() {
        let mut output = StackWriter {
            bytes: [0; 64],
            len: 0,
        };
        write!(
            output,
            "{} {}",
            Style::Bold.apply(42),
            Style::BoldBlue.apply(format_args!("file-{}", 7))
        )
        .unwrap();
        assert_eq!(
            &output.bytes[..output.len],
            b"\x1b[1m42\x1b[0m \x1b[1m\x1b[34mfile-7\x1b[39m\x1b[0m"
        );
    }

    #[test]
    fn formatter_errors_propagate() {
        for len in [64, 60, 56] {
            let mut output = StackWriter {
                bytes: [0; 64],
                len,
            };
            assert!(write!(output, "{}", Style::Bold.apply("text")).is_err());
        }
    }
}
