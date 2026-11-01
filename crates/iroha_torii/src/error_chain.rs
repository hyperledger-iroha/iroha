//! `Debug` rendering of an error together with its chain of causes.
//!
//! [`crate::Error`] prints its message, then every
//! [`source`](std::error::Error::source) under a `Caused by:` heading. Causes are
//! numbered when there are two or more, and continuation lines of a multi-line
//! cause stay indented under its text.

use std::fmt::{self, Write as _};

/// Write `error`'s message followed by the chain of its sources.
///
/// A single cause is indented by four spaces. Two or more are numbered from 1,
/// right-aligned in a five-column field.
///
/// # Errors
/// Fails only when writing to `f` fails.
pub(crate) fn write_error_chain(
    error: &dyn std::error::Error,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    fmt::Display::fmt(error, f)?;
    let Some(cause) = error.source() else {
        return Ok(());
    };
    f.write_str("\n\nCaused by:")?;
    let numbered = cause.source().is_some();
    let mut next = Some(cause);
    let mut number = 0_usize;
    while let Some(current) = next {
        number += 1;
        f.write_char('\n')?;
        let mut indented = IndentedCause {
            inner: f,
            number: numbered.then_some(number),
            started: false,
        };
        write!(indented, "{current}")?;
        next = current.source();
    }
    Ok(())
}

/// Writer that prefixes one cause and indents its continuation lines.
struct IndentedCause<'a, 'b> {
    inner: &'a mut fmt::Formatter<'b>,
    number: Option<usize>,
    started: bool,
}

impl fmt::Write for IndentedCause<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for (index, line) in text.split('\n').enumerate() {
            if !self.started {
                self.started = true;
                match self.number {
                    Some(number) => write!(self.inner, "{number: >5}: ")?,
                    None => self.inner.write_str("    ")?,
                }
            } else if index > 0 {
                let indent = if self.number.is_some() {
                    "       "
                } else {
                    "    "
                };
                self.inner.write_char('\n')?;
                self.inner.write_str(indent)?;
            }
            self.inner.write_str(line)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fmt};

    use super::write_error_chain;

    /// One error of a test chain, with an optional next cause.
    #[derive(Debug)]
    struct Layer {
        message: &'static str,
        cause: Option<Box<Layer>>,
    }

    impl Layer {
        fn chain(messages: &[&'static str]) -> Self {
            let (last, rest) = messages.split_last().expect("at least one message");
            rest.iter().rev().fold(
                Self {
                    message: last,
                    cause: None,
                },
                |cause, message| Self {
                    message,
                    cause: Some(Box::new(cause)),
                },
            )
        }
    }

    impl fmt::Display for Layer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.message)
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            let cause: &(dyn Error + 'static) = self.cause.as_deref()?;
            Some(cause)
        }
    }

    /// Debug-formats an error through [`write_error_chain`].
    struct Rendered<'a>(&'a dyn Error);

    impl fmt::Debug for Rendered<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write_error_chain(self.0, f)
        }
    }

    fn render(messages: &[&'static str]) -> String {
        format!("{:?}", Rendered(&Layer::chain(messages)))
    }

    #[test]
    fn error_without_source_prints_only_its_message() {
        assert_eq!(render(&["top"]), "top");
    }

    #[test]
    fn single_cause_is_indented_without_a_number() {
        assert_eq!(render(&["top", "middle"]), "top\n\nCaused by:\n    middle");
    }

    #[test]
    fn several_causes_are_numbered_in_a_right_aligned_column() {
        assert_eq!(
            render(&["top", "middle", "bottom"]),
            "top\n\nCaused by:\n    1: middle\n    2: bottom"
        );
    }

    #[test]
    fn multi_line_causes_stay_indented_under_their_text() {
        assert_eq!(
            render(&["top\nsecond", "one\ntwo"]),
            "top\nsecond\n\nCaused by:\n    one\n    two"
        );
        assert_eq!(
            render(&["top", "one\ntwo", "three"]),
            "top\n\nCaused by:\n    1: one\n       two\n    2: three"
        );
    }

    #[test]
    fn numbers_are_right_aligned_in_five_columns() {
        let messages: Vec<&'static str> = std::iter::once("top")
            .chain(std::iter::repeat_n("cause", 10))
            .collect();
        let rendered = render(&messages);
        assert!(
            rendered.ends_with("\n    9: cause\n   10: cause"),
            "{rendered}"
        );
    }

    #[test]
    fn torii_error_debug_prints_the_cause_chain() {
        let error = crate::Error::Config(eyre::eyre!("missing field").wrap_err("loading config"));
        assert_eq!(
            format!("{error:?}"),
            "Failed to get or set configuration\n\nCaused by:\n    1: loading config\n    2: missing field"
        );
    }
}
