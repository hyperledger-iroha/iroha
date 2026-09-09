//! Fallible post-reconstruction validation for binary derives.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

pub(super) fn value(hook: Option<&Path>, value: TokenStream) -> TokenStream {
    hook.map_or_else(|| quote!(Ok(#value)), |path| quote!(#path(#value)))
}

pub(super) fn unit_methods(ident: &Ident, hook: Option<&Path>) -> TokenStream {
    let Some(path) = hook else {
        return quote! {
            fn deserialize(_archived: &'de norito::core::Archived<Self>) -> Self {
                Self
            }
        };
    };
    // A fieldless value can still carry a packed offset table. Its existing
    // canonical boundary checks that metadata; a zero field offset is not a
    // claim that the complete payload is empty.
    quote! {
        fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
            match <Self as norito::core::DeserializePayload<'de>>::try_deserialize(archived) {
                Ok(value) => value,
                Err(err) => panic!(
                    concat!("norito: fallible deserialize failed for ", stringify!(#ident), ": {:?}"),
                    err,
                ),
            }
        }
        fn try_deserialize(
            _archived: &'de norito::core::Archived<Self>,
        ) -> ::core::result::Result<Self, norito::core::Error> {
            #path(Self)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ContainerAttr;
    use quote::ToTokens as _;
    use syn::{Data, DeriveInput};

    #[test]
    fn validate_accepts_a_quoted_function_path_alongside_shared_attributes() {
        let input: DeriveInput = syn::parse_quote! {
            #[norito(validate = "Self::checked", decode_from_slice, deny_unknown_fields)]
            struct Record { value: u8 }
        };
        let attrs = ContainerAttr::parse(&input.attrs).unwrap();
        assert_eq!(
            attrs.validate.unwrap().to_token_stream().to_string(),
            "Self :: checked"
        );
        assert!(attrs.decode_from_slice);
        assert!(attrs.deny_unknown_fields);
    }

    #[test]
    fn validate_rejects_duplicate_missing_and_non_path_values() {
        let inputs: [(DeriveInput, &str); 5] = [
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", validate = "Self::other")]
                    struct Record;
                },
                "duplicate `validate` attribute",
            ),
            (
                syn::parse_quote! { #[norito(validate)] struct Record; },
                "expected `=`",
            ),
            (
                syn::parse_quote! { #[norito(validate = 1)] struct Record; },
                "expected string literal",
            ),
            (
                syn::parse_quote! { #[norito(validate = "")] struct Record; },
                "invalid path ``",
            ),
            (
                syn::parse_quote! { #[norito(validate = "Self::checked()")] struct Record; },
                "invalid path `Self::checked()`",
            ),
        ];
        for (input, diagnostic) in inputs {
            let Err(error) = ContainerAttr::parse(&input.attrs) else {
                panic!("invalid validate attribute must reject");
            };
            let error = error.to_string();
            assert!(error.contains(diagnostic), "{error}");
        }
    }

    #[test]
    fn invalid_hook_paths_emit_diagnostics_from_struct_and_enum_generators() {
        for input in [
            syn::parse_quote! {
                #[norito(validate = "Self::checked()")]
                struct Record;
            },
            syn::parse_quote! {
                #[norito(validate = "Self::checked()")]
                enum Record { Unit }
            },
        ] {
            let input: DeriveInput = input;
            let tokens = match &input.data {
                Data::Struct(data) => crate::derive_struct_deserialize(
                    &input.ident,
                    &input.generics,
                    &data.fields,
                    &input.attrs,
                ),
                Data::Enum(data) => crate::derive_enum_deserialize(
                    &input.ident,
                    &input.generics,
                    data,
                    &input.attrs,
                ),
                Data::Union(_) => unreachable!(),
            };
            assert!(tokens.to_string().contains("compile_error"));
            assert!(
                tokens
                    .to_string()
                    .contains("invalid path `Self::checked()`")
            );
        }
    }

    #[test]
    fn validation_codegen_covers_every_reconstruction_without_slice_double_calls() {
        for (input, expected_calls) in [
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", decode_from_slice)]
                    struct Record<T> { value: T }
                },
                1,
            ),
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", decode_from_slice)]
                    struct Record<T>(T);
                },
                1,
            ),
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", decode_from_slice)]
                    struct Record<'__norito_slice, T> {
                        marker: ::core::marker::PhantomData<&'__norito_slice ()>,
                        value: T,
                    }
                },
                1,
            ),
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", decode_from_slice)]
                    struct Record;
                },
                1,
            ),
            (
                syn::parse_quote! {
                    #[norito(validate = "Self::checked", decode_from_slice)]
                    enum Record<T> { Unit, Tuple(T), Named { value: T } }
                },
                1,
            ),
        ] {
            let input: DeriveInput = input;
            let tokens = match &input.data {
                Data::Struct(data) => crate::derive_struct_deserialize(
                    &input.ident,
                    &input.generics,
                    &data.fields,
                    &input.attrs,
                ),
                Data::Enum(data) => crate::derive_enum_deserialize(
                    &input.ident,
                    &input.generics,
                    data,
                    &input.attrs,
                ),
                Data::Union(_) => unreachable!(),
            };
            syn::parse2::<syn::File>(tokens.clone()).expect("valid generated Rust items");
            let source = tokens.to_string();
            assert_eq!(
                source.matches("Self :: checked (").count(),
                expected_calls,
                "{source}"
            );
            assert!(!source.contains("T : norito :: core :: NoritoSerialize"));
        }
    }
}
