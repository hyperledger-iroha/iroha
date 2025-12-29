//! Error types for the `iroha_data_model` crate.

/// Error which occurs when parsing string into a data model entity
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ParseError {
    pub(crate) reason: &'static str,
}

impl ParseError {
    /// Construct a new parse error with a static reason message.
    #[must_use]
    pub const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Access the parse error message.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.reason)
    }
}

impl std::error::Error for ParseError {}

/// Error which occurs when converting an enum reference to a variant reference
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct EnumTryAsError<EXPECTED, GOT> {
    expected: core::marker::PhantomData<EXPECTED>,
    /// Actual enum variant which was being converted
    pub got: GOT,
}

// Manual implementation because this allow annotation does not affect `Display` derive
impl<EXPECTED, GOT: core::fmt::Debug> core::fmt::Display for EnumTryAsError<EXPECTED, GOT> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Expected: {}\nGot: {:?}",
            std::any::type_name::<EXPECTED>(),
            self.got,
        )
    }
}

impl<EXPECTED, GOT> EnumTryAsError<EXPECTED, GOT> {
    /// Construct an error from the actual enum variant encountered.
    pub const fn got(got: GOT) -> Self {
        Self {
            expected: core::marker::PhantomData,
            got,
        }
    }
}

impl<EXPECTED: core::fmt::Debug, GOT: core::fmt::Debug> std::error::Error
    for EnumTryAsError<EXPECTED, GOT>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_displays_reason() {
        let err = ParseError { reason: "test" };
        assert_eq!(err.to_string(), "test");
    }

    #[test]
    fn enum_try_as_error_displays_expected() {
        #[derive(Debug, PartialEq)]
        enum Example {
            One,
            Two,
        }

        let _ = Example::One;
        let err = EnumTryAsError::<Example, _>::got(Example::Two);
        assert_eq!(err.got, Example::Two);
        assert!(err.to_string().contains("Example"));
        assert!(err.to_string().contains("Two"));
    }
}
