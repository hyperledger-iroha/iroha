//! Strict child-identity metadata and registration-builder expansion tests.

use manyhow::Emitter;
use quote::{ToTokens, quote};
use syn::{DeriveInput, Item, Meta, Path, Token, parse_quote, punctuated::Punctuated};

use crate::EmitterExt;

use super::{child_schema_name, impl_registrable_builder};

fn parent(attributes: proc_macro2::TokenStream) -> DeriveInput {
    syn::parse2(quote! {
        #attributes
        struct Asset {
            id: u64,
        }
    })
    .expect("test parent")
}

#[test]
fn child_identity_is_an_explicit_literal() {
    let input = parent(quote! {
        #[registrable_builder(schema_name = "captured::private_model::NewAsset")]
    });
    assert_eq!(
        child_schema_name(&input)
            .expect("explicit identity")
            .value(),
        "captured::private_model::NewAsset"
    );
}

#[test]
fn missing_child_identity_has_no_inference_fallback() {
    for input in [
        parent(quote! {}),
        parent(quote! { #[registrable_builder()] }),
        parent(quote! { #[norito_schema(name = "parent::Asset")] }),
    ] {
        assert!(
            child_schema_name(&input)
                .expect_err("child identity required")
                .to_string()
                .contains("RegistrableBuilder requires")
        );
    }
}

#[test]
fn duplicate_identity_keys_are_rejected() {
    let input = parent(quote! {
        #[registrable_builder(schema_name = "same", schema_name = "same")]
    });
    assert_eq!(
        child_schema_name(&input)
            .expect_err("duplicate key")
            .to_string(),
        "duplicate registration builder schema_name"
    );
}

#[test]
fn duplicate_identity_declarations_are_rejected_even_when_empty() {
    for attributes in [
        quote! {
            #[registrable_builder(schema_name = "first")]
            #[registrable_builder(schema_name = "second")]
        },
        quote! {
            #[registrable_builder(schema_name = "first")]
            #[registrable_builder()]
        },
    ] {
        assert_eq!(
            child_schema_name(&parent(attributes))
                .expect_err("one declaration")
                .to_string(),
            "duplicate registration builder identity declaration"
        );
    }
}

#[test]
fn invalid_literal_contents_are_rejected() {
    for value in [
        "",
        " leading",
        "trailing\t",
        "embedded\nnewline",
        "nul\0byte",
        "del\u{7f}",
    ] {
        let input = parent(quote! { #[registrable_builder(schema_name = #value)] });
        assert!(
            child_schema_name(&input)
                .expect_err("invalid identity literal")
                .to_string()
                .contains("without surrounding whitespace or control characters")
        );
    }
}

#[test]
fn nonliteral_or_unknown_metadata_is_rejected() {
    for attributes in [
        quote! { #[registrable_builder(schema_name = 42)] },
        quote! { #[registrable_builder(schema_name = concat!("a", "b"))] },
        quote! { #[registrable_builder(schema_name)] },
        quote! { #[registrable_builder(name = "wrong key")] },
        quote! { #[registrable_builder(default = None)] },
        quote! { #[registrable_builder(schema_name = "valid", unexpected = true)] },
    ] {
        assert!(child_schema_name(&parent(attributes)).is_err());
    }
}

#[test]
fn expansion_declares_only_the_child_identity_and_preserves_builder_fields() {
    let input: DeriveInput = parse_quote! {
        #[norito_schema(name = "captured::Parent")]
        #[registrable_builder(schema_name = "captured::NewAsset")]
        struct Asset {
            id: u64,
            #[registrable_builder(default = Vec::new())]
            tags: Vec<String>,
            #[registrable_builder(skip, init = authority.clone())]
            owned_by: AccountId,
        }
    };
    let mut emitter = Emitter::new();
    let generated = impl_registrable_builder(&mut emitter, &input);
    let expanded: syn::File = syn::parse2(emitter.finish_token_stream_with(generated))
        .expect("generated builder is Rust syntax");
    let builders: Vec<_> = expanded
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(builders.len(), 1);
    let builder = builders[0];
    assert_eq!(builder.ident, "NewAsset");
    let fields: Vec<_> = builder
        .fields
        .iter()
        .map(|field| field.ident.as_ref().expect("named field").to_string())
        .collect();
    assert_eq!(fields, ["id", "tags"]);
    let mut derives = Vec::new();
    let mut identities = Vec::new();
    for attribute in &builder.attrs {
        if attribute.path().is_ident("derive") {
            let paths = attribute
                .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                .expect("derive paths");
            derives.extend(paths.iter().map(ToTokens::to_token_stream));
        }
        if attribute.path().is_ident("norito_schema") {
            let identity = attribute
                .parse_args::<Meta>()
                .expect("explicit generated identity");
            identities.push(identity.to_token_stream().to_string());
        }
    }
    assert_eq!(
        identities,
        [quote!(name = "captured::NewAsset").to_string()]
    );
    assert_eq!(
        derives.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "Debug",
            "Clone",
            "IdEqOrdHash",
            "Decode",
            "Encode",
            "IntoSchema",
            "norito :: NoritoSchema"
        ]
    );
    let tokens = expanded.to_token_stream().to_string();
    assert!(!tokens.contains("captured::Parent"));
    assert!(tokens.contains("tags : Vec :: new ()"));
    assert!(tokens.contains("owned_by : authority . clone ()"));
    assert!(tokens.contains("norito :: json :: JsonDeserialize for NewAsset"));
}

#[test]
fn invalid_identity_emits_only_a_diagnostic() {
    let input = parent(quote! {});
    let mut emitter = Emitter::new();
    let generated = impl_registrable_builder(&mut emitter, &input);
    assert!(generated.is_empty());
    let diagnostic = emitter.finish_token_stream_with(generated).to_string();
    assert!(diagnostic.contains("compile_error"));
    assert!(diagnostic.contains("RegistrableBuilder requires"));
    assert!(!diagnostic.contains("struct NewAsset"));
}
