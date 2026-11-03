//! Parsing utilities for `#[repr(...)]` attributes shared across derive crates.
use darling::{FromAttributes, error::Accumulator};
use proc_macro2::{Delimiter, Span};
use syn::{
    Attribute, Meta, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned as _,
};
// One spelling table defines the enum, formatter and both string conversion traits.
macro_rules! repr_primitives {
    ($($(#[$meta:meta])* $variant:ident => $spelling:literal),* $(,)?) => {
        /// Primitive types accepted by `#[repr(..)]`.
        #[derive(Debug, Copy, Clone, Eq, PartialEq)]
        pub enum ReprPrimitive {
            $($(#[$meta])* $variant),*
        }

        impl std::fmt::Display for ReprPrimitive {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let spelling = match self {
                    $(Self::$variant => $spelling),*
                };
                std::fmt::Display::fmt(spelling, formatter)
            }
        }

        impl std::str::FromStr for ReprPrimitive {
            type Err = ParseReprPrimitiveError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($spelling => Ok(Self::$variant)),*,
                    _ => Err(ParseReprPrimitiveError),
                }
            }
        }

        impl TryFrom<&str> for ReprPrimitive {
            type Error = ParseReprPrimitiveError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                value.parse()
            }
        }
    };
}

/// The input is not an exact lowercase primitive spelling accepted by `#[repr(..)]`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ParseReprPrimitiveError;

impl std::fmt::Display for ParseReprPrimitiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Matching variant not found")
    }
}

impl std::error::Error for ParseReprPrimitiveError {}

repr_primitives! {
    /// `u8`
    U8 => "u8",
    /// `u16`
    U16 => "u16",
    /// `u32`
    U32 => "u32",
    /// `u64`
    U64 => "u64",
    /// `u128`
    U128 => "u128",
    /// `usize`
    Usize => "usize",
    /// `i8`
    I8 => "i8",
    /// `i16`
    I16 => "i16",
    /// `i32`
    I32 => "i32",
    /// `i64`
    I64 => "i64",
    /// `i128`
    I128 => "i128",
    /// `isize`
    Isize => "isize",
}
/// Kinds accepted by `#[repr(..)]`.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReprKind {
    /// `repr(C)`
    C,
    /// `repr(transparent)`
    Transparent,
    /// `repr(<primitive>)`
    Primitive(ReprPrimitive),
}
/// Alignment specifiers accepted by `#[repr(..)]`.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReprAlignment {
    /// `repr(packed)`
    Packed,
    /// `repr(align(N))`
    Aligned(u32),
}
#[derive(Debug)]
enum ReprToken {
    Kind(ReprKind),
    Alignment(ReprAlignment),
}
#[derive(Debug)]
struct SpannedReprToken {
    span: Span,
    token: ReprToken,
}
impl Parse for SpannedReprToken {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (span, token) = input.step(|cursor| {
            let Some((ident, after_token)) = cursor.ident() else {
                return Err(cursor.error("Expected repr kind"));
            };
            let mut span = ident.span();
            let repr_ident = ident.to_string();
            if let Ok(primitive) = repr_ident.parse() {
                return Ok((
                    (span, ReprToken::Kind(ReprKind::Primitive(primitive))),
                    after_token,
                ));
            }
            match repr_ident.as_str() {
                "C" => Ok(((span, ReprToken::Kind(ReprKind::C)), after_token)),
                "transparent" => Ok(((span, ReprToken::Kind(ReprKind::Transparent)), after_token)),
                "packed" => Ok((
                    (span, ReprToken::Alignment(ReprAlignment::Packed)),
                    after_token,
                )),
                "align" => {
                    let Some((inside_group, group_span, after_group)) =
                        after_token.group(Delimiter::Parenthesis)
                    else {
                        return Err(cursor.error(
                            "Expected a number inside `repr(align(<number>))`, found `repr(align)`",
                        ));
                    };
                    span = span.join(group_span.span()).unwrap_or(span);
                    let alignment = syn::parse2::<syn::LitInt>(inside_group.token_stream())?;
                    let alignment = alignment.base10_parse::<u32>()?;
                    Ok((
                        (
                            span,
                            ReprToken::Alignment(ReprAlignment::Aligned(alignment)),
                        ),
                        after_group,
                    ))
                }
                _ => Err(cursor.error("Unrecognized repr kind")),
            }
        })?;
        Ok(SpannedReprToken { span, token })
    }
}
#[derive(Debug)]
struct ReprTokens(Vec<SpannedReprToken>);
impl Parse for ReprTokens {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self(
            Punctuated::<_, Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect(),
        ))
    }
}
/// Parsed representation of a `#[repr(..)]` attribute.
#[derive(Debug, Default, Clone, Copy)]
pub struct Repr {
    /// Repr kind (None means `repr(Rust)`).
    pub kind: Option<darling::util::SpannedValue<ReprKind>>,
    /// Repr alignment.
    pub alignment: Option<darling::util::SpannedValue<ReprAlignment>>,
}
impl FromAttributes for Repr {
    fn from_attributes(attrs: &[Attribute]) -> darling::Result<Self> {
        let mut result = Repr::default();
        let mut accumulator = Accumulator::default();
        for attr in attrs {
            if attr.path().is_ident("repr") {
                match &attr.meta {
                    Meta::Path(_) | Meta::NameValue(_) => accumulator.push(
                        darling::Error::custom(
                            "Unsupported repr shape, expected parenthesized list",
                        )
                        .with_span(attr),
                    ),
                    Meta::List(list) => {
                        let Some(tokens) = accumulator.handle(
                            syn::parse2::<ReprTokens>(list.tokens.clone()).map_err(Into::into),
                        ) else {
                            continue;
                        };
                        for SpannedReprToken { token, span } in tokens.0 {
                            match token {
                                ReprToken::Kind(kind) => {
                                    if result.kind.is_some() {
                                        accumulator.push(
                                            darling::error::Error::custom("Duplicate repr kind")
                                                .with_span(&span),
                                        );
                                    }
                                    result.kind =
                                        Some(darling::util::SpannedValue::new(kind, span));
                                }
                                ReprToken::Alignment(alignment) => {
                                    if result.alignment.is_some() {
                                        accumulator.push(
                                            darling::error::Error::custom(
                                                "Duplicate repr alignment",
                                            )
                                            .with_span(&span),
                                        );
                                    }
                                    result.alignment =
                                        Some(darling::util::SpannedValue::new(alignment, span));
                                }
                            }
                        }
                    }
                }
            }
        }
        accumulator.finish_with(result)
    }
}
#[cfg(test)]
mod tests {
    use super::{ParseReprPrimitiveError, Repr, ReprAlignment, ReprKind, ReprPrimitive};
    use darling::FromAttributes as _;
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse::Parser;

    #[test]
    fn primitive_spellings_round_trip_and_preserve_discriminants() {
        for (index, (primitive, spelling)) in [
            (ReprPrimitive::U8, "u8"),
            (ReprPrimitive::U16, "u16"),
            (ReprPrimitive::U32, "u32"),
            (ReprPrimitive::U64, "u64"),
            (ReprPrimitive::U128, "u128"),
            (ReprPrimitive::Usize, "usize"),
            (ReprPrimitive::I8, "i8"),
            (ReprPrimitive::I16, "i16"),
            (ReprPrimitive::I32, "i32"),
            (ReprPrimitive::I64, "i64"),
            (ReprPrimitive::I128, "i128"),
            (ReprPrimitive::Isize, "isize"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(primitive as usize, index);
            assert_eq!(primitive.to_string(), spelling);
            assert_eq!(spelling.parse::<ReprPrimitive>(), Ok(primitive));
            assert_eq!(ReprPrimitive::try_from(spelling), Ok(primitive));
            assert_eq!(format!("{primitive:>6.2}"), format!("{spelling:>6.2}"));
            assert_eq!(format!("{primitive:*^8}"), format!("{spelling:*^8}"));
        }
    }

    #[test]
    fn primitive_parser_rejects_noncanonical_and_unknown_spellings() {
        for value in [
            "", "U8", "Usize", "u8 ", " u8", "u08", "u256", "i256", "f32", "C", "Ｕ８", "u8\0",
        ] {
            assert_eq!(
                value.parse::<ReprPrimitive>(),
                Err(ParseReprPrimitiveError),
                "{value:?}"
            );
            assert_eq!(
                ReprPrimitive::try_from(value),
                Err(ParseReprPrimitiveError),
                "{value:?}"
            );
        }
        assert_eq!(
            ParseReprPrimitiveError.to_string(),
            "Matching variant not found"
        );
        assert!(std::error::Error::source(&ParseReprPrimitiveError).is_none());
    }
    #[derive(Debug)]
    struct ExpectedRepr {
        kind: Option<ReprKind>,
        alignment: Option<ReprAlignment>,
    }
    fn parse_repr(tokens: TokenStream) -> darling::Result<Repr> {
        let attrs = syn::Attribute::parse_outer.parse2(tokens).unwrap();
        Repr::from_attributes(&attrs)
    }
    macro_rules! assert_repr_ok {
        ($( #[$meta:meta] )* , $repr:expr) => {{
            let repr = parse_repr(quote!( $( #[$meta] )* )).unwrap();
            let expected: ExpectedRepr = $repr;
            assert_eq!(
                repr.kind.map(|v| *v.as_ref()),
                expected.kind,
                "repr kind mismatch"
            );
            assert_eq!(
                repr.alignment.map(|v| *v.as_ref()),
                expected.alignment,
                "repr alignment mismatch"
            );
        }};
    }
    #[test]
    fn parse_empty_repr() {
        assert_repr_ok!(
            #[repr()],
            ExpectedRepr {
                kind: None,
                alignment: None,
            }
        );
    }
    #[test]
    fn parse_repr_c() {
        assert_repr_ok!(
            #[repr(C)],
            ExpectedRepr {
                kind: Some(ReprKind::C),
                alignment: None,
            }
        );
    }
    #[test]
    fn parse_repr_aligned() {
        assert_repr_ok!(
            #[repr(align(4))],
            ExpectedRepr {
                kind: None,
                alignment: Some(ReprAlignment::Aligned(4)),
            }
        );
    }
    #[test]
    fn parse_repr_primitive() {
        assert_repr_ok!(
            #[repr(u8)],
            ExpectedRepr {
                kind: Some(ReprKind::Primitive(ReprPrimitive::U8)),
                alignment: None,
            }
        );
    }
    #[test]
    fn parse_repr_combined() {
        assert_repr_ok!(
            #[repr(C, align(8))],
            ExpectedRepr {
                kind: Some(ReprKind::C),
                alignment: Some(ReprAlignment::Aligned(8)),
            }
        );
    }
}
