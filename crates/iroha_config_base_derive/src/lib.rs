//! TODO

#![allow(unused)]
#![allow(clippy::large_enum_variant)]

use darling::{FromAttributes, FromDeriveInput};
use iroha_macro_utils::Emitter;
use manyhow::{manyhow, Result};
use proc_macro2::TokenStream;

use crate::ast::Input;

/// Derive `iroha_config_base::reader::ReadConfig` trait.
///
/// Example:
///
/// ```
/// use iroha_config_base_derive::ReadConfig;
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
/// Supported field attributes:
///
/// - `env = "<env var name>"` - read parameter from env (bound: `T: FromEnvStr`)
/// - `default` - fallback to default value (bound: `T: Default`)
/// - `default = "<expr>"` - fallback to a default value specified as an expression
/// - `nested` - delegates further reading (bound: `T: ReadConfig`).
///   It uses the field name as a namespace. Conflicts with others.
///
/// Supported field shapes (if `nested` is not specified):
///
/// - `T` - required parameter
/// - `WithOrigin<T>` - required parameter with origin data
/// - `Option<T>` - optional parameter
/// - `Option<WithOrigin<T>>` - optional parameter with origin data
///
/// Note: the macro recognizes the shape **syntactically**. That is, the wrapper types must appear
/// with exactly these names, with optional paths to them
/// (e.g. `Option<T>` is equivalent to `std::option::Option<T>`).
///
/// If `default` attr is specified, it expects a default value for the actual field type, i.e. `T`.
///
/// Note: bounds aren't generated by this macro,
/// but you will get compile errors if you do not satisfy them.
#[manyhow]
#[proc_macro_derive(ReadConfig, attributes(config))]
pub fn derive_read_config(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let Some(parsed) = emitter.handle(Input::from_derive_input(&input)) else {
        return emitter.finish_token_stream();
    };
    let Some(ir) = parsed.lower(&mut emitter) else {
        return emitter.finish_token_stream();
    };

    emitter.finish_token_stream_with(ir.generate())
}

/// Parsing proc-macro input
mod ast {
    use std::collections::HashSet;

    use iroha_macro_utils::Emitter;
    use manyhow::{emit, JoinToTokensError};
    use proc_macro2::{Ident, Span, TokenStream, TokenTree};
    use syn::{parse::ParseStream, punctuated::Punctuated, Token};

    use crate::codegen;

    // TODO: `attributes(config)` rejects all unknown fields
    //       it would be better to emit an error "we don't support struct attrs" instead
    #[derive(darling::FromDeriveInput, Debug)]
    #[darling(supports(struct_named), attributes(config))]
    pub struct Input {
        ident: syn::Ident,
        generics: syn::Generics,
        data: darling::ast::Data<(), Field>,
    }

    impl Input {
        pub fn lower(self, emitter: &mut Emitter) -> Option<codegen::Ir> {
            let mut halt_codegen = false;

            for i in self.generics.params {
                emit!(
                    emitter,
                    i,
                    "[derive(ReadConfig)]: generics are not supported"
                );
                // proceeding to codegen with these errors will produce a mess
                halt_codegen = true;
            }

            let entries = self
                .data
                .take_struct()
                .expect("darling should reject enums")
                .fields
                .into_iter()
                .map(|field| field.into_codegen(emitter))
                .collect();

            if halt_codegen {
                None
            } else {
                Some(codegen::Ir {
                    ident: self.ident,
                    entries,
                })
            }
        }
    }

    #[derive(Debug)]
    struct Field {
        ident: syn::Ident,
        ty: syn::Type,
        attrs: Attrs,
    }

    impl darling::FromField for Field {
        fn from_field(field: &syn::Field) -> darling::Result<Self> {
            let ident = field
                .ident
                .as_ref()
                .expect("darling should only allow named structs")
                .clone();
            let ty = field.ty.clone();

            let attrs: Attrs =
                iroha_macro_utils::parse_single_list_attr_opt("config", &field.attrs)?
                    .unwrap_or_default();

            Ok(Self { ident, ty, attrs })
        }
    }

    impl Field {
        fn into_codegen(self, emitter: &mut Emitter) -> codegen::Entry {
            let Field { ident, ty, attrs } = self;

            let kind = match attrs {
                Attrs::Nested => codegen::EntryKind::Nested,
                Attrs::Parameter { default, env } => {
                    let shape = ParameterTypeShape::analyze(&ty);
                    let evaluation = match (shape.option, default) {
                        (false, None) => codegen::Evaluation::Required,
                        (false, Some(AttrDefault::Value(expr))) => {
                            codegen::Evaluation::OrElse(expr)
                        }
                        (false, Some(AttrDefault::Flag)) => codegen::Evaluation::OrDefault,
                        (true, None) => codegen::Evaluation::Optional,
                        (true, _) => {
                            emit!(emitter, ident, "parameter of type `Option<..>` conflicts with `config(default)` attribute");
                            codegen::Evaluation::Optional
                        }
                    };

                    codegen::EntryKind::Parameter {
                        env,
                        evaluation,
                        with_origin: shape.with_origin,
                    }
                }
            };

            codegen::Entry { ident, kind }
        }
    }

    #[derive(Debug)]
    enum Attrs {
        Nested,
        Parameter {
            default: Option<AttrDefault>,
            env: Option<syn::LitStr>,
        },
    }

    impl Default for Attrs {
        fn default() -> Self {
            Self::Parameter {
                default: <_>::default(),
                env: <_>::default(),
            }
        }
    }

    #[derive(Debug)]
    enum AttrDefault {
        /// `config(default)`
        Flag,
        /// `config(default = "<expr>")`
        Value(syn::Expr),
    }

    impl syn::parse::Parse for Attrs {
        #[allow(clippy::too_many_lines)]
        fn parse(input: ParseStream) -> syn::Result<Self> {
            #[derive(Default)]
            struct Accumulator {
                default: Option<AttrDefault>,
                env: Option<(Span, syn::LitStr)>,
                nested: Option<Span>,
            }

            fn reject_duplicate<T>(
                acc: &mut Option<T>,
                span: Span,
                value: T,
            ) -> Result<(), syn::Error> {
                if acc.is_some() {
                    Err(syn::Error::new(span, "duplicate attribute"))
                } else {
                    *acc = Some(value);
                    Ok(())
                }
            }

            let mut acc = Accumulator::default();
            let tokens: Punctuated<AttrItem, Token![,]> = Punctuated::parse_terminated(input)?;
            for token in tokens {
                match token {
                    AttrItem::Default(span, value) => {
                        reject_duplicate(&mut acc.default, span, value)?
                    }
                    AttrItem::Env(span, value) => {
                        reject_duplicate(&mut acc.env, span, (span, value))?
                    }
                    AttrItem::Nested(span) => reject_duplicate(&mut acc.nested, span, span)?,
                }
            }

            let value = match acc {
                Accumulator {
                    nested: Some(_),
                    default: None,
                    env: None,
                } => Self::Nested,
                Accumulator {
                    nested: Some(span), ..
                } => {
                    return Err(syn::Error::new(
                        span,
                        "attributes conflict: `nested` cannot be set with other attributes",
                    ))
                }
                Accumulator { default, env, .. } => Self::Parameter {
                    default,
                    env: env.map(|(_, lit)| lit),
                },
            };

            Ok(value)
        }
    }

    #[derive(Debug)]
    /// A single item in the attribute list. Used for parsing of [`Attrs`].
    enum AttrItem {
        Default(Span, AttrDefault),
        Env(Span, syn::LitStr),
        Nested(Span),
    }

    impl syn::parse::Parse for AttrItem {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            input.step(|cursor| {
                const EXPECTED_IDENT: &str =
                    "unexpected token; expected `default`, `env`, or `nested`";

                let Some((ident, cursor)) = cursor.ident() else {
                    Err(syn::Error::new(cursor.span(), EXPECTED_IDENT))?
                };

                match ident.to_string().as_str() {
                    "default" => {
                        let (lit_str, cursor) = expect_eq_with_lit_str(cursor)?;
                        let Some(lit_str) = lit_str else {
                            return Ok((Self::Default(ident.span(), AttrDefault::Flag), cursor));
                        };
                        let expr: syn::Expr = lit_str.parse().map_err(|err| {
                            syn::Error::new(err.span(), format!("expected a valid expression within `default = \"<expr>\"`, but couldn't parse it: {err}"))
                        })?;
                        Ok((Self::Default(ident.span(), AttrDefault::Value(expr)), cursor))
                    }
                    "nested" => {
                        Ok((Self::Nested(ident.span()), cursor))
                    }
                    "env" => {
                        let (Some(lit), cursor) = expect_eq_with_lit_str(cursor)? else {
                            return Err(syn::Error::new(
                                ident.span(),
                                "expected `env` to be set as `env = \"VARIABLE_NAME\"",
                            ));
                        };
                        Ok((Self::Env(ident.span(), lit), cursor))
                    }
                    other => Err(syn::Error::new(cursor.span(), EXPECTED_IDENT)),
                }
            })
        }
    }

    fn expect_eq_with_lit_str(
        cursor: syn::buffer::Cursor,
    ) -> syn::Result<(Option<syn::LitStr>, syn::buffer::Cursor)> {
        const EXPECTED_STR_LIT: &str = r#"expected a string literal, e.g. "...""#;

        let next = match cursor.token_tree() {
            Some((TokenTree::Punct(punct), next)) if punct.as_char() == '=' => next,
            _ => return Ok((None, cursor)),
        };

        let (lit, next) = match next.token_tree() {
            Some((TokenTree::Literal(lit), next)) => (lit, next),
            Some((other, _)) => Err(syn::Error::new(other.span(), EXPECTED_STR_LIT))?,
            None => Err(syn::Error::new(next.span(), EXPECTED_STR_LIT))?,
        };

        let string = lit.to_string();
        let trimmed = string.trim_matches('"');
        if string == trimmed {
            // not a string literal
            Err(syn::Error::new(lit.span(), EXPECTED_STR_LIT))?;
        }

        let lit_str = syn::LitStr::new(trimmed, lit.span());
        Ok((Some(lit_str), next))
    }

    #[derive(Debug, PartialEq)]
    struct ParameterTypeShape {
        option: bool,
        with_origin: bool,
    }

    impl ParameterTypeShape {
        fn analyze(ty: &syn::Type) -> Self {
            #[derive(Debug)]
            enum Token {
                Option,
                WithOrigin,
                Unknown,
            }

            fn try_find(ty: &syn::Type) -> Option<(Option<&syn::Type>, &Ident)> {
                if let syn::Type::Path(type_path) = ty {
                    if let Some(last_segment) = type_path.path.segments.last() {
                        match &last_segment.arguments {
                            syn::PathArguments::AngleBracketed(args) if args.args.len() == 1 => {
                                if let syn::GenericArgument::Type(ty) =
                                    args.args.first().expect("should be exactly 1")
                                {
                                    return Some((Some(ty), &last_segment.ident));
                                }
                            }
                            syn::PathArguments::None => return Some((None, &last_segment.ident)),
                            _ => {}
                        }
                    }
                }

                None
            }

            fn parse_tokens(ty: &syn::Type, depth: u8) -> Vec<Token> {
                if depth == 0 {
                    return vec![];
                }

                if let Some((next, ident)) = try_find(ty) {
                    let token = match ident.to_string().as_ref() {
                        "Option" => Token::Option,
                        "WithOrigin" => Token::WithOrigin,
                        _ => Token::Unknown,
                    };

                    let mut chain = vec![token];

                    if let Some(next) = next {
                        chain.extend(parse_tokens(next, depth - 1));
                    }
                    chain
                } else {
                    vec![]
                }
            }

            let chain = parse_tokens(ty, 3);

            let (option, with_origin) = match (chain.first(), chain.get(1)) {
                (Some(Token::Option), Some(Token::WithOrigin)) => (true, true),
                (Some(Token::Option), Some(_)) => (true, false),
                (Some(Token::WithOrigin), _) => (false, true),
                _ => (false, false),
            };

            Self {
                option,
                with_origin,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use syn::parse_quote;

        use super::*;

        #[test]
        fn parse_default() {
            let attrs: Attrs = syn::parse_quote!(default);

            assert!(matches!(
                attrs,
                Attrs::Parameter {
                    default: Some(AttrDefault::Flag),
                    env: None
                }
            ));
        }

        #[test]
        fn parse_default_with_expr() {
            let attrs: Attrs = syn::parse_quote!(default = "42 + 411");

            assert!(matches!(
                attrs,
                Attrs::Parameter {
                    default: Some(AttrDefault::Value(_)),
                    env: None
                }
            ));
        }

        #[test]
        fn parse_default_env() {
            let attrs: Attrs = syn::parse_quote!(default, env = "$!@#");

            let Attrs::Parameter {
                default: Some(AttrDefault::Flag),
                env: Some(var),
            } = attrs
            else {
                panic!("expectation failed")
            };
            assert_eq!(var.value(), "$!@#");
        }

        #[test]
        #[should_panic(
            expected = "attributes conflict: `nested` cannot be set with other attributes"
        )]
        fn conflict() {
            let _: Attrs = syn::parse_quote!(nested, default);
        }

        #[test]
        #[should_panic(expected = "duplicate attribute")]
        fn duplicates() {
            let _: Attrs = syn::parse_quote!(default, default);
        }

        #[test]
        fn determine_shapes() {
            macro_rules! case {
                ($input:ty, $option:literal, $with_origin:literal) => {
                    let ty: syn::Type = syn::parse_quote!($input);
                    let shape = ParameterTypeShape::analyze(&ty);
                    assert_eq!(
                        shape,
                        ParameterTypeShape {
                            option: $option,
                            with_origin: $with_origin
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
            case!(
                std::option::Option<whatever::WithOrigin<Something>>,
                true,
                true
            );
        }
    }
}

/// Generating code based on [`model`]
mod codegen {
    use proc_macro2::TokenStream;
    use quote::quote;

    pub struct Ir {
        /// The type we are implementing `ReadConfig` for
        pub ident: syn::Ident,
        pub entries: Vec<Entry>,
    }

    impl Ir {
        pub fn generate(self) -> TokenStream {
            let (read_fields, unwrap_fields): (Vec<_>, Vec<_>) = self
                .entries
                .into_iter()
                .map(Entry::generate)
                .map(|EntryParts { read, unwrap }| (read, unwrap))
                .unzip();

            let ident = self.ident;

            quote! {
                impl ::iroha_config_base::read::ReadConfig for #ident {
                    fn read(
                        __reader: &mut ::iroha_config_base::read::ConfigReader
                    ) -> ::iroha_config_base::read::FinalWrap<Self> {
                        #(#read_fields)*

                        ::iroha_config_base::read::FinalWrap::value_fn(|| Self {
                            #(#unwrap_fields),*
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
        fn generate(self) -> EntryParts {
            let Self { kind, ident } = self;

            let read = match kind {
                EntryKind::Nested => {
                    quote! { let #ident = __reader.read_nested(stringify!(#ident)); }
                }
                EntryKind::Parameter {
                    env,
                    evaluation,
                    with_origin,
                } => {
                    let mut read = quote! {
                        let #ident = __reader.read_parameter([stringify!(#ident)])
                    };
                    if let Some(var) = env {
                        read.extend(quote! { .env(#var) })
                    }
                    read.extend(match evaluation {
                        Evaluation::Required => quote! { .value_required() },
                        Evaluation::OrElse(expr) => quote! { .value_or_else(|| #expr) },
                        Evaluation::OrDefault => quote! { .value_or_default() },
                        Evaluation::Optional => quote! { .value_optional() },
                    });
                    read.extend(if with_origin {
                        quote! { .finish_with_origin(); }
                    } else {
                        quote! { .finish(); }
                    });
                    read
                }
            };

            EntryParts {
                read,
                unwrap: quote! { #ident: #ident.unwrap() },
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
        OrElse(syn::Expr),
        OrDefault,
        Optional,
    }

    #[cfg(test)]
    mod tests {
        use expect_test::expect;
        use syn::parse_quote;

        use super::*;

        #[test]
        fn entry_with_env_reading() {
            let entry = Entry {
                ident: parse_quote!(test),
                kind: EntryKind::Parameter {
                    env: Some(parse_quote!("TEST_ENV")),
                    evaluation: Evaluation::Required,
                    with_origin: false,
                },
            };

            let actual = entry.generate().read.to_string();

            expect![[r#"let test = __reader . read_parameter ([stringify ! (test)]) . env ("TEST_ENV") . value_required () . finish () ;"#]].assert_eq(&actual);
        }
    }
}
