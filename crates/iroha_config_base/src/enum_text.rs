//! Exact text representations for configuration enums.

/// A configuration value did not match any declared enum spelling.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ParseError {
    /// No variant uses the supplied case-sensitive spelling.
    VariantNotFound,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Matching variant not found")
    }
}

impl std::error::Error for ParseError {}

/// Define a configuration enum with one exact text spelling per unit variant.
///
/// The same declaration supplies allocation-free `as_str`, [`core::fmt::Display`],
/// [`core::str::FromStr`] and `TryFrom<&str>` implementations. Parsing is
/// case-sensitive and neither trims whitespace nor accepts undeclared aliases.
/// Enum derives, variant documentation and `#[default]` retain their ordinary
/// Rust meaning. The declaration must list the same variants in every build.
///
/// ```
/// iroha_config_base::config_enum! {
///     /// Storage mode.
///     #[derive(Debug, Default, PartialEq, Eq)]
///     enum StorageMode {
///         /// Audit storage before opening it.
///         #[default]
///         Strict => "strict",
///         /// Open the bounded recovery view.
///         Fast => "fast",
///     }
/// }
/// assert_eq!(StorageMode::Strict.as_str(), "strict");
/// assert_eq!("fast".parse::<StorageMode>().unwrap(), StorageMode::Fast);
/// assert!(StorageMode::try_from("FAST").is_err());
/// ```
///
/// Duplicate spellings are rejected at compilation:
///
/// ```compile_fail
/// iroha_config_base::config_enum! {
///     enum Ambiguous {
///         First => "same",
///         Second => "same",
///     }
/// }
/// ```
#[macro_export]
macro_rules! config_enum {
    (
        $(#[$enum_attribute:meta])*
        $visibility:vis enum $name:ident {
            $(
                $(#[$variant_attribute:meta])*
                $variant:ident => $spelling:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_attribute])*
        $visibility enum $name {
            $($(#[$variant_attribute])* $variant),+
        }

        impl $name {
            /// Borrow this value's canonical configuration spelling.
            #[must_use]
            pub const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $spelling),+
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(self.as_str(), formatter)
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::enum_text::ParseError;

            #[deny(unreachable_patterns)]
            fn from_str(
                value: &str,
            ) -> ::core::result::Result<Self, <Self as ::core::str::FromStr>::Err> {
                match value {
                    $($spelling => ::core::result::Result::Ok(Self::$variant)),+,
                    _ => ::core::result::Result::Err(
                        $crate::enum_text::ParseError::VariantNotFound,
                    ),
                }
            }
        }

        impl ::core::convert::TryFrom<&str> for $name {
            type Error = $crate::enum_text::ParseError;

            fn try_from(
                value: &str,
            ) -> ::core::result::Result<Self, <Self as ::core::convert::TryFrom<&str>>::Error> {
                <Self as ::core::str::FromStr>::from_str(value)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::ParseError;

    crate::config_enum! {
        /// Small fixture whose spelling is independent of Rust variant names.
        #[derive(Debug, Default, PartialEq, Eq)]
        enum Mode {
            /// Default mode.
            #[default]
            First => "one",
            /// Preserve an explicitly uppercase spelling.
            Second => "TWO",
        }
    }

    #[test]
    fn exact_spelling_drives_parsing_display_and_default() {
        assert_eq!(Mode::default(), Mode::First);
        for (mode, spelling) in [(Mode::First, "one"), (Mode::Second, "TWO")] {
            assert_eq!(mode.as_str(), spelling);
            assert_eq!(spelling.parse::<Mode>().unwrap(), mode);
            assert_eq!(Mode::try_from(spelling).unwrap(), mode);
            assert_eq!(mode.to_string(), spelling);
            assert_eq!(format!("{mode:*>8.2}"), format!("{spelling:*>8.2}"));
        }
        for invalid in ["", "One", "ONE", "two", "First", "one ", " one", "one\0"] {
            assert_eq!(invalid.parse::<Mode>(), Err(ParseError::VariantNotFound));
            assert_eq!(Mode::try_from(invalid), Err(ParseError::VariantNotFound));
        }
    }

    #[test]
    fn parse_error_preserves_diagnostics_without_retaining_input() {
        use std::error::Error as _;

        let error = ParseError::VariantNotFound;
        assert_eq!(core::mem::size_of::<ParseError>(), 0);
        assert_eq!(error.to_string(), "Matching variant not found");
        assert_eq!(format!("{error:?}"), "VariantNotFound");
        assert_eq!(format!("{error:>40}"), "Matching variant not found");
        assert!(error.source().is_none());
    }

    #[test]
    fn error_named_variants_do_not_shadow_conversion_associated_types() {
        crate::config_enum! {
            #[derive(Debug, PartialEq, Eq)]
            enum Outcome {
                Error => "error",
                Err => "err",
            }
        }

        for (variant, spelling) in [(Outcome::Error, "error"), (Outcome::Err, "err")] {
            assert_eq!(spelling.parse::<Outcome>(), Ok(variant));
            assert_eq!(Outcome::try_from(spelling).unwrap().as_str(), spelling);
        }
        assert_eq!(
            Outcome::try_from("unknown"),
            Err(ParseError::VariantNotFound)
        );
    }
}
