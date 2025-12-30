//! Shared helper utilities for procedural macro crates.

pub mod emitter;
pub mod repr;

pub use emitter::Emitter;
pub use repr::{Repr, ReprAlignment, ReprKind, ReprPrimitive};

/// Extension trait for [`darling::Error`] that attaches multiple spans.
pub trait DarlingErrorExt: Sized {
    /// Attaches a combination of multiple spans to the error.
    ///
    /// Note: on stable rustc only the first span is attached because
    /// [`proc_macro2::Span::join`] is not fully stabilized.
    #[must_use]
    fn with_spans(self, spans: impl IntoIterator<Item = impl Into<proc_macro2::Span>>) -> Self;
}

impl DarlingErrorExt for darling::Error {
    fn with_spans(self, spans: impl IntoIterator<Item = impl Into<proc_macro2::Span>>) -> Self {
        let mut iter = spans.into_iter();
        let Some(first) = iter.next() else {
            return self;
        };
        let first: proc_macro2::Span = first.into();
        let joined = iter
            .try_fold(first, |acc, span| acc.join(span.into()))
            .unwrap_or(first);
        self.with_span(&joined)
    }
}

/// Finds an optional single attribute with specified name.
///
/// Returns `None` if no attributes with specified name are found.
///
/// Emits an error into accumulator if multiple attributes with specified name are found.
#[must_use]
pub fn find_single_attr_opt<'a>(
    accumulator: &mut darling::error::Accumulator,
    attr_name: &str,
    attrs: &'a [syn::Attribute],
) -> Option<&'a syn::Attribute> {
    let matching_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(attr_name))
        .collect::<Vec<_>>();
    let attr = match *matching_attrs.as_slice() {
        [] => {
            return None;
        }
        [attr] => attr,
        [attr, ref tail @ ..] => {
            accumulator.push(
                darling::Error::custom(format!("Only one #[{attr_name}] attribute is allowed!"))
                    .with_spans(tail.iter().map(syn::spanned::Spanned::span)),
            );
            attr
        }
    };
    Some(attr)
}

/// Parses a single list attribute `#[attr_name(...)]` returning `Ok(None)` when absent.
///
/// # Errors
///
/// - Multiple attributes with the specified name.
/// - Attribute is not a list.
pub fn parse_single_list_attr_opt<Body: syn::parse::Parse>(
    attr_name: &str,
    attrs: &[syn::Attribute],
) -> darling::Result<Option<Body>> {
    let mut accumulator = darling::error::Accumulator::default();
    let Some(attr) = find_single_attr_opt(&mut accumulator, attr_name, attrs) else {
        return accumulator.finish_with(None);
    };

    let mut kind = None;
    match &attr.meta {
        syn::Meta::Path(_) | syn::Meta::NameValue(_) => accumulator.push(darling::Error::custom(
            format!("Expected #[{attr_name}(...)] attribute to be a list"),
        )),
        syn::Meta::List(list) => {
            kind = accumulator.handle(syn::parse2(list.tokens.clone()).map_err(Into::into));
        }
    }

    accumulator.finish_with(kind)
}

/// Parses a single list attribute `#[attr_name(...)]`.
///
/// # Errors
///
/// - Multiple attributes with the specified name.
/// - Attribute is not a list.
/// - Attribute is missing.
pub fn parse_single_list_attr<Body: syn::parse::Parse>(
    attr_name: &str,
    attrs: &[syn::Attribute],
) -> darling::Result<Body> {
    parse_single_list_attr_opt(attr_name, attrs)?
        .ok_or_else(|| darling::Error::custom(format!("Missing `#[{attr_name}(...)]` attribute")))
}

/// Macro for generating simple attribute structs that implement [`syn::parse::Parse`].
#[macro_export]
macro_rules! attr_struct {
    (
        $( #[$meta:meta] )*
        $vis:vis struct $name:ident {
            $(
                $( #[$field_meta:meta] )*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $( #[$meta] )*
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field_name : $field_ty
            ),*
        }

        impl syn::parse::Parse for $name {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                Ok(Self {
                    $(
                        $field_name: input.parse()?,
                    )*
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use manyhow::{Error as ManyhowError, error_message};

    use super::*;

    #[test]
    fn emitter_finish_ok_when_no_errors() {
        let emitter = Emitter::new();
        assert!(emitter.finish().is_ok());
    }

    #[test]
    fn emitter_emit_and_finish_err() {
        let mut emitter = Emitter::new();
        emitter.emit(ManyhowError::from(error_message!("err")));
        assert!(emitter.finish().is_err());
    }

    #[test]
    fn emitter_handle_variants() {
        let mut emitter = Emitter::new();
        let val = emitter.handle::<manyhow::Error, _>(Ok(10));
        assert_eq!(val, Some(10));

        let val2: Option<i32> =
            emitter.handle::<ManyhowError, _>(Err(error_message!("oops").into()));
        assert!(val2.is_none());
        assert!(emitter.finish().is_err());
    }

    #[test]
    fn emitter_handle_or_default() {
        let mut emitter = Emitter::new();
        let val: i32 =
            emitter.handle_or_default::<ManyhowError, _>(Err(error_message!("oops").into()));
        assert_eq!(val, 0);
        assert!(emitter.finish().is_err());
    }

    #[test]
    fn emitter_finish_with_and_finish_and() {
        let emitter = Emitter::new();
        assert_eq!(emitter.finish_with(5).unwrap(), 5);

        let res: manyhow::Result<i32> = Emitter::new().finish_and::<manyhow::Error, _>(Ok(3));
        assert_eq!(res.unwrap(), 3);

        let res_err: manyhow::Result<()> =
            Emitter::new().finish_and::<ManyhowError, _>(Err(error_message!("err").into()));
        assert!(res_err.is_err());
    }

    #[test]
    fn emitter_finish_token_stream() {
        let mut emitter = Emitter::new();
        emitter.emit(ManyhowError::from(error_message!("err")));
        let mut tokens = proc_macro2::TokenStream::new();
        emitter.finish_to_token_stream(&mut tokens);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn attr_struct_macro_generates_parse_impl() {
        attr_struct! {
            pub struct ExampleAttr {
                pub name: syn::Ident,
            }
        }

        let attr: ExampleAttr = syn::parse2(quote::quote!(value)).expect("parse attr");
        assert_eq!(attr.name.to_string(), "value");
    }
}
