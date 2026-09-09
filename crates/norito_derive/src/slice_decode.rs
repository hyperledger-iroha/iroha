//! Derived slice decoding and collision-free implementation lifetimes.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::{Attribute, Generics, Ident, Lifetime};

use super::has_decode_from_slice_attr;

pub(super) enum DecodeBody {
    Archived(TokenStream2),
    Prefix,
}

fn collect_lifetimes(tokens: TokenStream2, names: &mut BTreeSet<String>) {
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Group(group) => collect_lifetimes(group.stream(), names),
            TokenTree::Punct(mark) if mark.as_char() == '\'' => {
                if let Some(TokenTree::Ident(name)) = tokens.next() {
                    let name = name.to_string();
                    names.insert(name.strip_prefix("r#").unwrap_or(&name).to_owned());
                }
            }
            _ => {}
        }
    }
}

fn fresh_lifetime(generics: &Generics) -> Lifetime {
    let params = &generics.params;
    let where_clause = &generics.where_clause;
    let mut names = BTreeSet::new();
    // Include nested higher-ranked binders, not only top-level parameters.
    // Keep the original parsed generics for emission; tokens only select a name.
    collect_lifetimes(quote!(#params #where_clause), &mut names);
    let mut name = "__norito_slice".to_owned();
    while names.contains(&name) {
        name.insert(0, '_');
    }
    Lifetime::new(&format!("'{name}"), Span::call_site())
}

pub(super) fn derive(
    ident: &Ident,
    generics: &Generics,
    container_attrs: &[Attribute],
    decode_body: DecodeBody,
) -> TokenStream2 {
    if !has_decode_from_slice_attr(container_attrs) {
        return TokenStream2::new();
    }
    let slice_lifetime = fresh_lifetime(generics);
    let mut implementation = generics.clone();
    implementation.params.insert(
        0,
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(slice_lifetime.clone())),
    );
    let (impl_generics, _, where_clause) = implementation.split_for_impl();
    let (_, ty_generics, _) = generics.split_for_impl();
    let decode_body = match decode_body {
        DecodeBody::Archived(body) => quote! {
            let __logical_len = __prepared.logical_len();
            let __archived_bytes = __prepared.bytes();
            let _pg = norito::core::PayloadCtxGuard::enter_with_len(
                __archived_bytes,
                __logical_len,
            );
            let __archived = __prepared.archived::<Self>();
            #body
        },
        DecodeBody::Prefix => quote! {
            norito::core::decode_prepared_slice_prefix::<Self>(&__prepared)
        },
    };
    quote! {
        impl #impl_generics norito::core::DecodeFromSlice<#slice_lifetime> for #ident #ty_generics #where_clause {
            #[inline]
            fn decode_from_slice(bytes: &#slice_lifetime [u8]) -> ::core::result::Result<(Self, usize), norito::core::Error> {
                let __prepared = norito::core::prepare_decode_from_slice(
                    bytes,
                    norito::core::archived_payload_size::<Self>(),
                    norito::core::archived_payload_align::<Self>(),
                )?;
                #decode_body
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens as _;
    use syn::{DeriveInput, GenericParam, ItemImpl};

    #[test]
    fn fresh_lifetime_avoids_parameters_bounds_where_clauses_and_nested_groups() {
        let cases: [(DeriveInput, &str); 7] = [
            (
                syn::parse_quote! { struct Record<T>(T); },
                "'__norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<'__norito_slice, T>(&'__norito_slice T);
                },
                "'___norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<T: for<'__norito_slice> Scope<'__norito_slice>>(T);
                },
                "'___norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<T> where T: for<'__norito_slice> Scope<'__norito_slice> { value: T }
                },
                "'___norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<T: Fn(for<'__norito_slice> fn(&'__norito_slice ()))>(T);
                },
                "'___norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<'__norito_slice, T>
                    where T: for<'___norito_slice> Scope<'___norito_slice>
                    { value: &'__norito_slice T }
                },
                "'____norito_slice",
            ),
            (
                syn::parse_quote! {
                    struct Record<'r#__norito_slice>(&'r#__norito_slice u8);
                },
                "'___norito_slice",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(fresh_lifetime(&input.generics).to_string(), expected);
        }
    }

    #[test]
    fn slice_impl_retains_type_arguments_defaults_bounds_and_body() {
        let input: DeriveInput = syn::parse_quote! {
            #[norito(decode_from_slice)]
            struct Record<'original, T = u8, const N: usize = 4>
            where T: for<'__norito_slice> Scope<'__norito_slice>
            { value: &'original [T; N] }
        };
        let body = quote! { Ok((Self::default(), __logical_len)) };
        let tokens = derive(
            &input.ident,
            &input.generics,
            &input.attrs,
            DecodeBody::Archived(body.clone()),
        );
        let emitted: ItemImpl = syn::parse2(tokens.clone()).expect("one valid impl parameter list");
        assert_eq!(emitted.generics.params.len(), 4);
        assert_eq!(
            emitted
                .generics
                .lifetimes()
                .map(|param| param.lifetime.to_string())
                .collect::<Vec<_>>(),
            ["'___norito_slice", "'original"],
        );
        assert_eq!(
            emitted.self_ty.to_token_stream().to_string(),
            "Record < 'original , T , N >"
        );
        assert_eq!(
            emitted.generics.where_clause.to_token_stream().to_string(),
            input.generics.where_clause.to_token_stream().to_string(),
        );
        assert!(emitted.generics.params.iter().all(|param| match param {
            GenericParam::Type(param) => param.default.is_none(),
            GenericParam::Const(param) => param.default.is_none(),
            GenericParam::Lifetime(_) => true,
        }));
        assert!(tokens.to_string().contains(&body.to_string()));
        assert!(
            tokens
                .to_string()
                .contains("DecodeFromSlice < '___norito_slice >")
        );
        assert!(
            tokens
                .to_string()
                .contains("bytes : & '___norito_slice [u8]")
        );
    }

    #[test]
    fn slice_generation_remains_opt_in_and_does_not_inject_validation() {
        let input: DeriveInput = syn::parse_quote! { struct Record<T>(T); };
        assert!(
            derive(
                &input.ident,
                &input.generics,
                &input.attrs,
                DecodeBody::Archived(quote!())
            )
            .is_empty()
        );
        let input: DeriveInput = syn::parse_quote! {
            #[norito(decode_from_slice)]
            struct Record<T>(T);
        };
        let tokens = derive(
            &input.ident,
            &input.generics,
            &input.attrs,
            DecodeBody::Archived(quote! {
                Ok((Self::checked(), 7))
            }),
        );
        let source = tokens.to_string();
        assert_eq!(source.matches("Self :: checked ()").count(), 1);
        assert!(!source.contains("validate"));
        syn::parse2::<ItemImpl>(tokens).expect("nonguarded body remains valid");
    }

    #[test]
    fn prefix_slice_delegates_to_one_payload_decoder_without_frame_or_encode_bounds() {
        let input: DeriveInput = syn::parse_quote! {
            #[norito(decode_from_slice)]
            struct Record<T> { value: T }
        };
        let tokens = derive(
            &input.ident,
            &input.generics,
            &input.attrs,
            DecodeBody::Prefix,
        );
        let source = tokens.to_string();
        assert_eq!(source.matches("decode_prepared_slice_prefix").count(), 1);
        assert!(!source.contains("SerializePayload"));
        assert!(!source.contains("NoritoSchema"));
        assert!(!source.contains("PayloadCtxGuard"));
        assert!(!source.contains("__logical_len"));
        syn::parse2::<ItemImpl>(tokens).expect("prefix-only slice implementation");
    }
}
