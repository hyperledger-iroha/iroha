//! Canonical schema declaration parsing and code generation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, GenericParam, LitStr, Result, parse_quote};

fn reject_nested(attributes: &[Attribute]) -> Result<()> {
    if let Some(attribute) = attributes
        .iter()
        .find(|a| a.path().is_ident("norito_schema"))
    {
        return Err(syn::Error::new_spanned(
            attribute,
            "norito_schema is only valid on the type declaration",
        ));
    }
    Ok(())
}

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    let mut name = None;
    let mut frame = None;
    for attribute in &input.attrs {
        if attribute.path().is_ident("norito_schema") {
            attribute.parse_nested_meta(|meta| {
                let slot = if meta.path.is_ident("name") {
                    &mut name
                } else if meta.path.is_ident("frame") {
                    &mut frame
                } else {
                    return Err(meta.error("expected name or frame"));
                };
                if slot.is_some() {
                    return Err(meta.error("duplicate schema identity declaration"));
                }
                let value: LitStr = meta.value()?.parse()?;
                if value.value().is_empty() || value.value().trim() != value.value() {
                    return Err(syn::Error::new_spanned(
                        value,
                        "schema identity must be nonempty without surrounding whitespace",
                    ));
                }
                *slot = Some(value);
                Ok(())
            })?;
        }
    }
    let name = name.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            "NoritoSchema requires #[norito_schema(name = \"canonical identity\")]",
        )
    })?;
    match &input.data {
        Data::Struct(data) => {
            for field in &data.fields {
                reject_nested(&field.attrs)?;
            }
        }
        Data::Enum(data) => {
            for variant in &data.variants {
                reject_nested(&variant.attrs)?;
                for field in &variant.fields {
                    reject_nested(&field.attrs)?;
                }
            }
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "NoritoSchema does not support unions",
            ));
        }
    }
    let mut generics = input.generics.clone();
    let mut arguments = Vec::new();
    for parameter in &input.generics.params {
        match parameter {
            GenericParam::Type(parameter) => {
                reject_nested(&parameter.attrs)?;
                let ident = &parameter.ident;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ident: ::norito::NoritoSchema));
                arguments.push(quote!(<#ident as ::norito::NoritoSchema>::nominal_name()));
            }
            GenericParam::Const(parameter) => {
                reject_nested(&parameter.attrs)?;
                let ident = &parameter.ident;
                arguments.push(quote!(::std::format!("{:?}", #ident)));
            }
            GenericParam::Lifetime(parameter) => {
                reject_nested(&parameter.attrs)?;
                arguments.push(quote!(::std::string::String::from("'_")));
            }
        }
    }
    if input
        .generics
        .params
        .iter()
        .any(|parameter| !matches!(parameter, GenericParam::Lifetime(_)))
        && frame.is_some()
    {
        return Err(syn::Error::new_spanned(
            frame,
            "generic schema identities cannot use a fixed frame projection",
        ));
    }
    let projection = frame.map(|frame| {
        quote! {
            fn frame_name() -> ::std::string::String { ::std::string::String::from(#frame) }
        }
    });
    let ident = input.ident;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::norito::NoritoSchema for #ident #type_generics #where_clause {
            fn nominal_name() -> ::std::string::String {
                ::norito::schema::identity::generic_name(#name, &[#(#arguments),*])
            }
            #projection
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(input: TokenStream) -> String {
        expand(syn::parse2(input).expect("valid Rust declaration"))
            .expect_err("invalid identity declaration")
            .to_string()
    }

    #[test]
    fn requires_explicit_identity() {
        assert!(
            error(quote!(
                struct Missing;
            ))
            .contains("NoritoSchema requires")
        );
    }

    #[test]
    fn rejects_empty_duplicate_and_unknown_keys() {
        for input in [
            quote!(
                #[norito_schema(name = "")]
                struct Empty;
            ),
            quote!(
                #[norito_schema(name = " example ")]
                struct Whitespace;
            ),
            quote!(
                #[norito_schema(name = "a", name = "b")]
                struct Duplicate;
            ),
            quote!(
                #[norito_schema(name = "a", frame = "b", frame = "c")]
                struct DuplicateFrame;
            ),
            quote!(
                #[norito_schema(name = "a", alias = "b")]
                struct Unknown;
            ),
        ] {
            assert!(expand(syn::parse2(input).unwrap()).is_err());
        }
    }

    #[test]
    fn rejects_nested_declarations_and_unions() {
        for input in [
            quote!(
                #[norito_schema(name = "a")]
                struct Field {
                    #[norito_schema(name = "b")]
                    value: u8,
                }
            ),
            quote!(
                #[norito_schema(name = "a")]
                enum Variant {
                    #[norito_schema(name = "b")]
                    Value,
                }
            ),
            quote!(#[norito_schema(name = "a")] union Union { value: u8 }),
        ] {
            assert!(expand(syn::parse2(input).unwrap()).is_err());
        }
    }

    #[test]
    fn rejects_fixed_generic_frame_projection() {
        let result = error(quote!(
            #[norito_schema(name = "a", frame = "b")]
            struct Generic<T>(T);
        ));
        assert_eq!(
            result,
            "generic schema identities cannot use a fixed frame projection"
        );
    }

    #[test]
    fn identity_bounds_do_not_require_payload_serialization() {
        let generated = expand(parse_quote!(
            #[norito_schema(name = "example::Marker")]
            struct Marker<'a, T: ?Sized, const N: usize>(&'a T);
        ))
        .unwrap()
        .to_string();
        assert!(generated.contains("NoritoSchema"));
        assert!(generated.contains("automatically_derived"));
        assert!(!generated.contains("NoritoSerialize"));
        assert!(!generated.contains("NoritoDeserialize"));
        assert!(!generated.contains("type_name"));
    }
}
