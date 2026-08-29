//! Derive macros for `iroha_config_base`.
//!
//! `ReadConfig` reads named struct fields from environment variables, defaults,
//! or nested namespaces. Supported field attributes are:
//!
//! - `env = "VAR"` to read from an environment variable.
//! - `default` or `default = "expr"` to provide a fallback.
//! - `nested` to read a nested struct under the field-name namespace.

use iroha_derive_primitives::Emitter;
use proc_macro2::{Span, TokenStream};

/// Derive `iroha_config_base::read::ReadConfig`.
///
/// ```text
/// use iroha_derive::ReadConfig;
///
/// #[derive(ReadConfig)]
/// struct Config {
///     #[config(default, env = "FOO")]
///     foo: bool,
///     #[config(nested)]
///     nested: Nested,
/// }
///
/// #[derive(ReadConfig)]
/// struct Nested {
///     #[config(default = "42")]
///     foo: u64,
/// }
/// ```
///
/// Supported field shapes for parameters are `T`, `WithOrigin<T>`,
/// `Option<T>`, and `Option<WithOrigin<T>>`. The macro recognizes bare wrapper
/// names as well as `std::option::Option`, `core::option::Option`, and
/// `iroha_config_base::WithOrigin`.
pub fn derive_read_config_impl(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(input): Option<syn::DeriveInput> = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let Some(ir) = ast::lower(input, &mut emitter) else {
        return emitter.finish_token_stream();
    };
    emitter.finish_token_stream_with(ir.generate())
}

mod ast {
    use super::{Span, codegen};
    use iroha_derive_primitives::Emitter;
    use manyhow::emit;
    use syn::{Ident, Token, parse::ParseStream, spanned::Spanned as _};

    pub fn lower(input: syn::DeriveInput, emitter: &mut Emitter) -> Option<codegen::Ir> {
        let syn::DeriveInput {
            attrs,
            ident,
            generics,
            data,
            ..
        } = input;
        let mut valid = true;

        for attr in attrs {
            if attr.path().is_ident("config") {
                emit!(
                    emitter,
                    attr,
                    "struct-level `#[config]` attributes are not supported"
                );
                valid = false;
            }
        }
        if let Some(parameter) = generics.params.first() {
            emit!(
                emitter,
                parameter,
                "[derive(ReadConfig)]: generics are not supported"
            );
            valid = false;
        } else if let Some(where_clause) = generics.where_clause {
            emit!(
                emitter,
                where_clause,
                "[derive(ReadConfig)]: generics are not supported"
            );
            valid = false;
        }

        let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) = data
        else {
            emit!(
                emitter,
                ident,
                "[derive(ReadConfig)]: expected a struct with named fields"
            );
            return None;
        };
        let fields = fields.named;

        let mut entries = Vec::with_capacity(fields.len());
        for field in fields {
            let field_span = field.span();
            let Some(field_ident) = field.ident else {
                emit!(
                    emitter,
                    field_span,
                    "[derive(ReadConfig)]: expected a named field"
                );
                valid = false;
                continue;
            };
            let attrs = match parse_config_attrs(&field.attrs) {
                Ok(attrs) => attrs,
                Err(error) => {
                    emitter.emit(error);
                    valid = false;
                    continue;
                }
            };
            match lower_field(field_ident, &field.ty, attrs, emitter) {
                Some(entry) => entries.push(entry),
                None => valid = false,
            }
        }

        valid.then_some(codegen::Ir { ident, entries })
    }

    fn parse_config_attrs(attrs: &[syn::Attribute]) -> syn::Result<ConfigAttrs> {
        let mut selected = None;
        let mut errors: Option<syn::Error> = None;
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("config")) {
            if selected.is_some() {
                let error = syn::Error::new(attr.span(), "only one #[config(...)] is allowed");
                if let Some(errors) = &mut errors {
                    errors.combine(error);
                } else {
                    errors = Some(error);
                }
            } else {
                selected = Some(attr);
            }
        }
        if let Some(errors) = errors {
            return Err(errors);
        }
        let Some(attr) = selected else {
            return Ok(ConfigAttrs::default());
        };
        if !matches!(&attr.meta, syn::Meta::List(_)) {
            return Err(syn::Error::new(attr.span(), "expected #[config(...)]"));
        }
        attr.parse_args()
    }

    fn lower_field(
        ident: Ident,
        ty: &syn::Type,
        attrs: ConfigAttrs,
        emitter: &mut Emitter,
    ) -> Option<codegen::Entry> {
        let kind = if attrs.nested.is_some() {
            codegen::EntryKind::Nested
        } else {
            let shape = ParameterTypeShape::analyze(ty);
            let evaluation = match (shape.option, attrs.default) {
                (false, None) => codegen::Evaluation::Required,
                (false, Some(AttrDefault::Value(expression))) => {
                    codegen::Evaluation::OrElse(expression)
                }
                (false, Some(AttrDefault::Flag)) => codegen::Evaluation::OrDefault,
                (true, None) => codegen::Evaluation::Optional,
                (true, Some(_)) => {
                    emit!(
                        emitter,
                        ident,
                        "parameter of type `Option<..>` conflicts with `config(default)` attribute"
                    );
                    return None;
                }
            };
            codegen::EntryKind::Parameter {
                env: attrs.env,
                evaluation,
                with_origin: shape.with_origin,
            }
        };
        Some(codegen::Entry { ident, kind })
    }

    #[derive(Default)]
    struct ConfigAttrs {
        default: Option<AttrDefault>,
        env: Option<syn::LitStr>,
        nested: Option<Span>,
    }

    enum AttrDefault {
        Flag,
        Value(Box<syn::Expr>),
    }

    impl syn::parse::Parse for ConfigAttrs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            fn reject_duplicate<T>(
                target: &mut Option<T>,
                span: Span,
                value: T,
            ) -> syn::Result<()> {
                if target.is_some() {
                    Err(syn::Error::new(span, "duplicate attribute"))
                } else {
                    *target = Some(value);
                    Ok(())
                }
            }

            fn parse_lit_str(input: ParseStream) -> syn::Result<syn::LitStr> {
                input.parse().map_err(|_| {
                    syn::Error::new(input.span(), r#"expected a string literal, e.g. "...""#)
                })
            }

            let mut attrs = Self::default();
            while !input.is_empty() {
                const EXPECTED: &str = "unexpected token; expected `default`, `env`, or `nested`";
                let ident: Ident = input
                    .parse()
                    .map_err(|_| syn::Error::new(input.span(), EXPECTED))?;
                let span = ident.span();

                if ident == "default" {
                    let value = if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let literal = parse_lit_str(input)?;
                        let expression = literal.parse().map_err(|error| {
                            syn::Error::new(
                                literal.span(),
                                format!(r#"expected a valid expression within `default = "<expr>"`, but couldn't parse it: {error}"#),
                            )
                        })?;
                        AttrDefault::Value(Box::new(expression))
                    } else {
                        AttrDefault::Flag
                    };
                    reject_duplicate(&mut attrs.default, span, value)?;
                } else if ident == "env" {
                    if !input.peek(Token![=]) {
                        return Err(syn::Error::new(
                            span,
                            r#"expected `env` to be set as `env = "VARIABLE_NAME"`"#,
                        ));
                    }
                    input.parse::<Token![=]>()?;
                    let value = parse_lit_str(input)?;
                    reject_duplicate(&mut attrs.env, span, value)?;
                } else if ident == "nested" {
                    reject_duplicate(&mut attrs.nested, span, span)?;
                } else {
                    return Err(syn::Error::new(span, EXPECTED));
                }

                if input.is_empty() {
                    break;
                }
                input.parse::<Token![,]>()?;
            }

            if let Some(span) = attrs.nested
                && (attrs.default.is_some() || attrs.env.is_some())
            {
                return Err(syn::Error::new(
                    span,
                    "attributes conflict: `nested` cannot be set with `default` or `env`",
                ));
            }
            Ok(attrs)
        }
    }

    #[derive(Debug, PartialEq)]
    struct ParameterTypeShape {
        option: bool,
        with_origin: bool,
    }

    impl ParameterTypeShape {
        fn analyze(ty: &syn::Type) -> Self {
            #[derive(Clone, Copy)]
            enum Wrapper {
                Option,
                WithOrigin,
            }

            fn path_matches(path: &syn::Path, expected: &[&str]) -> bool {
                path.segments.len() == expected.len()
                    && path
                        .segments
                        .iter()
                        .zip(expected)
                        .all(|(segment, expected)| segment.ident == *expected)
            }

            fn is_wrapper(path: &syn::Path, wrapper: Wrapper) -> bool {
                match wrapper {
                    Wrapper::Option => {
                        path_matches(path, &["Option"])
                            || path_matches(path, &["std", "option", "Option"])
                            || path_matches(path, &["core", "option", "Option"])
                    }
                    Wrapper::WithOrigin => {
                        path_matches(path, &["WithOrigin"])
                            || path_matches(path, &["iroha_config_base", "WithOrigin"])
                    }
                }
            }

            fn single_type_argument(ty: &syn::Type, wrapper: Wrapper) -> Option<&syn::Type> {
                let syn::Type::Path(type_path) = ty else {
                    return None;
                };
                if type_path.qself.is_some() || !is_wrapper(&type_path.path, wrapper) {
                    return None;
                }
                let segment = type_path.path.segments.last()?;
                let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                    return None;
                };
                let mut arguments = arguments.args.iter();
                let syn::GenericArgument::Type(inner) = arguments.next()? else {
                    return None;
                };
                arguments.next().is_none().then_some(inner)
            }

            let optional_inner = single_type_argument(ty, Wrapper::Option);
            let option = optional_inner.is_some();
            let with_origin =
                single_type_argument(optional_inner.unwrap_or(ty), Wrapper::WithOrigin).is_some();
            Self {
                option,
                with_origin,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_defaults_and_environment() {
            let flag: ConfigAttrs = syn::parse_quote!(default);
            assert!(matches!(flag.default, Some(AttrDefault::Flag)));

            let expression: ConfigAttrs = syn::parse_quote!(default = "42 + 411");
            assert!(matches!(expression.default, Some(AttrDefault::Value(_))));

            let environment: ConfigAttrs = syn::parse_quote!(default, env = "$!@#");
            assert!(matches!(environment.default, Some(AttrDefault::Flag)));
            assert_eq!(
                environment.env.as_ref().map(syn::LitStr::value),
                Some("$!@#".to_owned())
            );
        }

        #[test]
        fn parses_nested() {
            let attrs: ConfigAttrs = syn::parse_quote!(nested);
            assert!(attrs.nested.is_some());
            assert!(attrs.default.is_none());
            assert!(attrs.env.is_none());
        }

        #[test]
        fn rejects_conflicts_duplicates_and_unknown_attributes() {
            for tokens in [
                quote::quote!(nested, default),
                quote::quote!(default, default),
                quote::quote!(env),
                quote::quote!(key = "legacy"),
            ] {
                assert!(syn::parse2::<ConfigAttrs>(tokens).is_err());
            }
        }

        #[test]
        fn determines_only_supported_shapes() {
            macro_rules! case {
                ($input:ty, $option:literal, $with_origin:literal) => {
                    let ty: syn::Type = syn::parse_quote!($input);
                    assert_eq!(
                        ParameterTypeShape::analyze(&ty),
                        ParameterTypeShape {
                            option: $option,
                            with_origin: $with_origin,
                        }
                    );
                };
            }

            case!(Something, false, false);
            case!(Option<Something>, true, false);
            case!(Option<WithOrigin<Something>>, true, true);
            case!(WithOrigin<Something>, false, true);
            case!(WithOrigin<Option<Something>>, false, true);
            case!(Option<Option<WithOrigin<Something>>>, true, false);
            case!(std::option::Option<WithOrigin<Something>>, true, true);
            case!(
                core::option::Option<iroha_config_base::WithOrigin<Something>>,
                true,
                true
            );
            case!(
                whatever::Option<whatever::WithOrigin<Something>>,
                false,
                false
            );
        }

        #[test]
        fn rejects_duplicate_config_attributes() {
            let field: syn::Field = syn::parse_quote! {
                #[config(default)]
                #[config(env = "VALUE")]
                value: u64
            };
            assert!(parse_config_attrs(&field.attrs).is_err());
        }
    }
}

mod codegen {
    use proc_macro2::TokenStream;
    use quote::{format_ident, quote};

    pub struct Ir {
        pub ident: syn::Ident,
        pub entries: Vec<Entry>,
    }

    impl Ir {
        pub fn generate(self) -> TokenStream {
            let mut reads = TokenStream::new();
            let mut unwraps = TokenStream::new();
            for (index, entry) in self.entries.into_iter().enumerate() {
                let EntryParts { read, unwrap } = entry.generate(index);
                reads.extend(read);
                unwraps.extend(quote! { #unwrap, });
            }
            let ident = self.ident;
            quote! {
                impl ::iroha_config_base::read::ReadConfig for #ident {
                    fn read(
                        __reader: &mut ::iroha_config_base::read::ConfigReader
                    ) -> ::iroha_config_base::read::FinalWrap<Self> {
                        #reads
                        ::iroha_config_base::read::FinalWrap::value_fn(|| Self {
                            #unwraps
                        })
                    }
                }
            }
        }
    }

    pub struct Entry {
        pub ident: syn::Ident,
        pub kind: EntryKind,
    }

    impl Entry {
        fn generate(self, index: usize) -> EntryParts {
            let Self { kind, ident } = self;
            let key = quote! { stringify!(#ident) };
            let binding = format_ident!("__iroha_config_field_{index}", span = ident.span());
            let read = match kind {
                EntryKind::Nested => quote! {
                    let #binding = __reader.read_nested(#key);
                },
                EntryKind::Parameter {
                    env,
                    evaluation,
                    with_origin,
                } => {
                    let mut read = quote! {
                        let #binding = __reader.read_parameter([#key])
                    };
                    if let Some(variable) = env {
                        read.extend(quote! { .env(#variable) });
                    }
                    read.extend(match evaluation {
                        Evaluation::Required => quote! { .value_required() },
                        Evaluation::OrElse(expression) => {
                            quote! { .value_or_else(|| #expression) }
                        }
                        Evaluation::OrDefault => quote! { .value_or_default() },
                        Evaluation::Optional => quote! { .value_optional() },
                    });
                    if with_origin {
                        read.extend(quote! { .finish_with_origin(); });
                    } else {
                        read.extend(quote! { .finish(); });
                    }
                    read
                }
            };
            EntryParts {
                read,
                unwrap: quote! { #ident: #binding.unwrap() },
            }
        }
    }

    struct EntryParts {
        read: TokenStream,
        unwrap: TokenStream,
    }

    pub enum EntryKind {
        Parameter {
            env: Option<syn::LitStr>,
            evaluation: Evaluation,
            with_origin: bool,
        },
        Nested,
    }

    pub enum Evaluation {
        Required,
        OrElse(Box<syn::Expr>),
        OrDefault,
        Optional,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use expect_test::expect;
        use syn::parse_quote;

        #[test]
        fn entry_with_environment_reading() {
            let entry = Entry {
                ident: parse_quote!(test),
                kind: EntryKind::Parameter {
                    env: Some(parse_quote!("TEST_ENV")),
                    evaluation: Evaluation::Required,
                    with_origin: false,
                },
            };
            let actual = entry.generate(2).read.to_string();
            expect![[r#"let __iroha_config_field_2 = __reader . read_parameter ([stringify ! (test)]) . env ("TEST_ENV") . value_required () . finish () ;"#]].assert_eq(&actual);
        }

        #[test]
        fn nested_entry_uses_field_name() {
            let entry = Entry {
                ident: parse_quote!(service),
                kind: EntryKind::Nested,
            };
            let actual = entry.generate(3).read.to_string();
            expect![[
                r"let __iroha_config_field_3 = __reader . read_nested (stringify ! (service)) ;"
            ]]
            .assert_eq(&actual);
        }

        #[test]
        fn optional_entry_uses_field_name() {
            let entry = Entry {
                ident: parse_quote!(test),
                kind: EntryKind::Parameter {
                    env: None,
                    evaluation: Evaluation::Optional,
                    with_origin: false,
                },
            };
            let actual = entry.generate(4).read.to_string();
            expect![[r"let __iroha_config_field_4 = __reader . read_parameter ([stringify ! (test)]) . value_optional () . finish () ;"]].assert_eq(&actual);
        }
    }
}
