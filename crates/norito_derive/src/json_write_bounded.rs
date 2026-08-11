//! Checked JSON writer code-generation helpers.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Generics};

use super::{ContainerAttr, FieldAttr, add_bound};

#[derive(Default)]
pub(super) struct EnumAttr {
    pub(super) tag: Option<String>,
    pub(super) content: Option<String>,
}

impl EnumAttr {
    pub(super) fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let container = ContainerAttr::parse(attrs)?;
        Ok(Self {
            tag: container.tag,
            content: container.content,
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct VariantAttr {
    pub(super) rename: Option<String>,
}

impl VariantAttr {
    pub(super) fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = Self::default();
        for attr in attrs {
            if !attr.path().is_ident("norito") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    if out.rename.replace(lit.value()).is_some() {
                        return Err(meta.error("duplicate `rename` attribute"));
                    }
                } else {
                    return Err(meta.error("unknown `norito` variant attribute"));
                }
                Ok(())
            })?;
        }
        Ok(out)
    }
}

impl FieldAttr {
    pub(super) fn require_json_serialize_bound(&self, generics: &mut Generics, ty: &syn::Type) {
        if self.with.is_none() {
            add_bound(generics, ty, quote!(norito::json::JsonSerialize));
        }
    }

    pub(super) fn require_json_deserialize_bound(&self, generics: &mut Generics, ty: &syn::Type) {
        if self.with.is_none() {
            add_bound(generics, ty, quote!(norito::json::JsonDeserialize));
        }
    }

    pub(super) fn deserializer_call(&self, ty: &syn::Type, parser: TokenStream2) -> TokenStream2 {
        if let Some(path) = &self.with {
            quote! { #path::deserialize(#parser)? }
        } else {
            quote! { <#ty as norito::json::JsonDeserialize>::json_deserialize(#parser)? }
        }
    }

    pub(super) fn deserialize_from_value(
        &self,
        ty: &syn::Type,
        value: TokenStream2,
    ) -> TokenStream2 {
        let call = self.deserializer_call(ty, quote!(&mut __parser));
        quote! {{
            let __json = norito::json::to_json(&#value)?;
            let mut __parser = norito::json::Parser::new(&__json);
            #call
        }}
    }

    pub(super) fn bounded_serializer_call(
        &self,
        value: TokenStream2,
        out: TokenStream2,
    ) -> TokenStream2 {
        let ordinary = if let Some(path) = &self.with {
            quote! { #path::serialize(#value, __norito_unbounded_output); }
        } else {
            quote! {
                norito::json::JsonSerialize::json_serialize(
                    #value,
                    __norito_unbounded_output,
                );
            }
        };
        let bounded = if let Some(path) = &self.bounded_with {
            quote! { #path(#value, #out) }
        } else if self.with.is_some() {
            quote! {
                ::core::result::Result::Err(norito::json::BoundedJsonError::Unsupported)
            }
        } else {
            quote! { norito::json::JsonSerialize::json_serialize_to(#value, #out) }
        };
        quote! {{
            if let ::core::option::Option::Some(__norito_unbounded_output) =
                norito::json::JsonWriteSink::unbounded_output(#out)
            {
                #ordinary
                ::core::result::Result::Ok(())
            } else {
                #bounded
            }
        }}
    }
}
