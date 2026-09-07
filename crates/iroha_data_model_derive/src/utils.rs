use manyhow::{Error as ManyhowError, ToTokensError};
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

/// Parse the one explicit nominal identity owned by a generated child type.
///
/// The child identity is independent of its parent's identity. Missing, duplicate,
/// unknown, nonliteral, and malformed declarations fail before code generation.
pub fn required_child_schema_name(
    input: &syn::DeriveInput,
    attribute_name: &str,
    derive_name: &str,
    child_description: &str,
) -> syn::Result<syn::LitStr> {
    let mut name = None;
    let mut declaration_seen = false;
    for attribute in &input.attrs {
        if !attribute.path().is_ident(attribute_name) {
            continue;
        }
        if declaration_seen {
            return Err(syn::Error::new_spanned(
                attribute,
                format!("duplicate {child_description} identity declaration"),
            ));
        }
        declaration_seen = true;
        attribute.parse_nested_meta(|nested| {
            if !nested.path.is_ident("schema_name") {
                return Err(nested.error(format!("unsupported {child_description} identity option")));
            }
            if name.is_some() {
                return Err(nested.error(format!("duplicate {child_description} schema_name")));
            }
            let literal: syn::LitStr = nested.value()?.parse()?;
            let value = literal.value();
            if value.is_empty()
                || value.trim() != value
                || value.chars().any(char::is_control)
            {
                return Err(syn::Error::new_spanned(
                    literal,
                    format!("{child_description} schema_name must be nonempty without surrounding whitespace or control characters"),
                ));
            }
            name = Some(literal);
            Ok(())
        })?;
    }
    name.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            format!("{derive_name} requires #[{attribute_name}(schema_name = \"captured child identity\")]"),
        )
    })
}
/// Extension trait for [`darling::Error`] adding a `with_spans` helper.
#[allow(dead_code)]
pub trait DarlingErrorExt: Sized {
    /// Attaches multiple spans to the error.
    fn with_spans(self, spans: impl IntoIterator<Item = impl Into<Span>>) -> Self;
}
impl DarlingErrorExt for darling::Error {
    fn with_spans(self, spans: impl IntoIterator<Item = impl Into<Span>>) -> Self {
        let mut iter = spans.into_iter();
        let Some(first) = iter.next() else {
            return self;
        };
        let first: Span = first.into();
        let r = iter
            .try_fold(first, |a, b| a.join(b.into()))
            .unwrap_or(first);
        self.with_span(&r)
    }
}
/// Finds an optional single attribute with specified name.
///
/// Returns `None` if no attributes with specified name are found.
/// Emits an error into accumulator if multiple attributes with specified name are found.
pub fn find_single_attr_opt<'a>(
    accumulator: &mut darling::error::Accumulator,
    attr_name: &str,
    attrs: &'a [syn::Attribute],
) -> Option<&'a syn::Attribute> {
    let mut iter = attrs.iter().filter(|a| a.path().is_ident(attr_name));
    let attr = iter.next()?;
    if let Some(duplicate) = iter.next() {
        accumulator.push(
            darling::Error::custom(format!("Only one #[{attr_name}] attribute is allowed!"))
                .with_span(duplicate),
        );
    }
    Some(attr)
}
/// Parses a single attribute of the form `#[attr_name(...)]` for darling using a `syn::parse::Parse` implementation.
///
/// If no attribute with specified name is found, returns `Ok(None)`.
///
/// # Errors
/// - If multiple attributes with specified name are found
/// - If attribute is not a list
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
/// Parses a single attribute of the form `#[attr_name(...)]`.
/// Returns an error if attribute is missing.
pub fn parse_single_list_attr<Body: syn::parse::Parse>(
    attr_name: &str,
    attrs: &[syn::Attribute],
) -> darling::Result<Body> {
    parse_single_list_attr_opt(attr_name, attrs)?
        .ok_or_else(|| darling::Error::custom(format!("Missing `#[{attr_name}(...)]` attribute")))
}
#[derive(Debug)]
pub struct DarlingErrorWrapper(pub darling::Error);
impl ToTokensError for DarlingErrorWrapper {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.clone().write_errors().to_tokens(tokens);
    }
}
pub fn darling_error(err: darling::Error) -> ManyhowError {
    ManyhowError::from(DarlingErrorWrapper(err))
}
pub fn darling_result<T>(result: darling::Result<T>) -> manyhow::Result<T, DarlingErrorWrapper> {
    result.map_err(DarlingErrorWrapper)
}
// Macro for automatic `syn::parse::Parse` implementation for keyword attribute structs.
#[allow(unused_macros)]
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
                $field_vis $field_name : $field_ty,
            )*
        }
        impl syn::parse::Parse for $name {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                Ok(Self {
                    $( $field_name: input.parse()?, )*
                })
            }
        }
    };
}
#[allow(unused_imports)]
pub(crate) use attr_struct;
#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;
    #[test]
    fn generated_child_identity_is_explicit_and_distinct_from_parent() {
        let input = parse_quote! {
            #[norito_schema(name = "fixture::Parent")]
            #[event_set(schema_name = "fixture::Child")]
            enum Parent { A }
        };
        let identity = required_child_schema_name(&input, "event_set", "EventSet", "event set")
            .expect("explicit child identity");
        assert_eq!(identity.value(), "fixture::Child");
    }
    #[test]
    fn generated_child_identity_rejects_ambiguous_or_invalid_metadata() {
        for (attributes, expected) in [
            ("", "requires #[event_set"),
            (
                "#[norito_schema(name = \"fixture::Parent\")]",
                "requires #[event_set",
            ),
            ("#[event_set()]", "requires #[event_set"),
            ("#[event_set]", "expected attribute arguments"),
            (
                "#[event_set(schema_name = \"First\")] #[event_set(schema_name = \"Second\")]",
                "duplicate event set identity declaration",
            ),
            (
                "#[event_set(schema_name = \"First\", schema_name = \"Second\")]",
                "duplicate event set schema_name",
            ),
            (
                "#[event_set(other = \"Child\")]",
                "unsupported event set identity option",
            ),
            ("#[event_set(schema_name = 4)]", "expected string literal"),
            (
                "#[event_set(schema_name = concat!(\"fixture\", \"::Child\"))]",
                "expected string literal",
            ),
            ("#[event_set(schema_name = \"\")]", "must be nonempty"),
            ("#[event_set(schema_name = \" Child\")]", "must be nonempty"),
            (
                "#[event_set(schema_name = \"Child \" )]",
                "must be nonempty",
            ),
            (
                r#"#[event_set(schema_name = "Child\u{0}Identity")]"#,
                "must be nonempty",
            ),
        ] {
            let input = syn::parse_str(&format!("{attributes} enum Parent {{ A }}"))
                .expect("syntactically valid fixture");
            let error = required_child_schema_name(&input, "event_set", "EventSet", "event set")
                .expect_err(attributes);
            assert!(
                error.to_string().contains(expected),
                "{attributes}: {error}"
            );
        }
    }
    #[test]
    fn find_single_attr_opt_works() {
        let attrs: Vec<syn::Attribute> = vec![parse_quote!(#[test_attr(foo)])];
        let mut acc = darling::error::Accumulator::default();
        let attr = find_single_attr_opt(&mut acc, "test_attr", &attrs).unwrap();
        acc.finish().unwrap();
        assert!(attr.path().is_ident("test_attr"));
    }
    #[test]
    fn find_single_attr_opt_reports_duplicate_declarations() {
        let attrs = vec![
            parse_quote!(#[test_attr(first)]),
            parse_quote!(#[unrelated(value)]),
            parse_quote!(#[test_attr(second)]),
        ];
        let mut accumulator = darling::error::Accumulator::default();
        assert!(find_single_attr_opt(&mut accumulator, "test_attr", &attrs).is_some());
        let error = accumulator
            .finish()
            .expect_err("duplicates must be reported");
        assert!(error.to_string().contains("Only one #[test_attr]"));
    }
    #[test]
    fn parse_single_list_attr_opt_rejects_duplicate_declarations() {
        for duplicate in [
            parse_quote!(#[test_attr(first)]),
            parse_quote!(#[test_attr(second)]),
        ] {
            let attrs = vec![parse_quote!(#[test_attr(first)]), duplicate];
            let error = parse_single_list_attr_opt::<syn::Ident>("test_attr", &attrs)
                .expect_err("even identical declarations are ambiguous");
            assert!(error.to_string().contains("Only one #[test_attr]"));
        }
    }
    #[test]
    fn parse_single_list_attr_opt_parses() {
        let attrs: Vec<syn::Attribute> = vec![parse_quote!(#[test_attr(foo)])];
        let parsed: Option<syn::Ident> = parse_single_list_attr_opt("test_attr", &attrs).unwrap();
        assert_eq!(parsed.unwrap().to_string(), "foo");
    }
    #[test]
    fn parse_single_list_attr_requires_attr() {
        let attrs: Vec<syn::Attribute> = vec![parse_quote!(#[test_attr(foo)])];
        let parsed: syn::Ident = parse_single_list_attr("test_attr", &attrs).unwrap();
        assert_eq!(parsed.to_string(), "foo");
    }
    #[test]
    fn darling_error_ext_with_spans() {
        let err = darling::Error::custom("err").with_spans([Span::call_site()]);
        let _ = err;
    }
    #[test]
    fn attr_struct_macro_parses() {
        attr_struct! {
            pub struct TestAttr { ident: syn::Ident }
        }
        let attr: TestAttr = syn::parse_str("my_ident").unwrap();
        assert_eq!(attr.ident.to_string(), "my_ident");
    }
}
