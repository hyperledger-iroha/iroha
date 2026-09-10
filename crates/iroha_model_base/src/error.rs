//! Errors returned when parsing canonical foundational model values.
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_error_displays_reason() {
        let err = ParseError { reason: "test" };
        assert_eq!(err.to_string(), "test");
    }
}
