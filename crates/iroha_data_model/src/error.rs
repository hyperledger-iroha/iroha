//! Error types for the `iroha_data_model` crate.
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
