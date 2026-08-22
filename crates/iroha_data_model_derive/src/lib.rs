//! A crate containing various derive macros for `iroha_data_model`
// darling-generated code triggers this lint
#![allow(clippy::needless_continue)]
iroha_derive::define_emitter_ext!();
mod enum_ref;
mod event_set;
mod has_origin;
mod id;
mod model;
mod registrable_builder;
mod utils;
use crate::utils::darling_error;
use darling::{FromMeta, ast::NestedMeta};
use manyhow::{Emitter, Result, emit, manyhow};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;
#[doc = include_str!("lib_docs/enum_ref.md")]
#[manyhow]
#[proc_macro_derive(EnumRef, attributes(enum_ref))]
pub fn enum_ref(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    enum_ref::impl_enum_ref(&input)
}
#[doc = include_str!("lib_docs/model.md")]
#[manyhow]
#[proc_macro_attribute]
pub fn model(attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    if !attr.is_empty() {
        emit!(emitter, attr, "This attribute does not take any arguments");
    }
    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let result = model::impl_model(&mut emitter, &input);
    emitter.finish_token_stream_with(result)
}
/// Same as [`model()`] macro, but only processes a single item.
///
/// You should prefer using [`model()`] macro over this one.
#[manyhow]
#[proc_macro]
pub fn model_single(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    emitter.finish_token_stream_with(model::process_item(input))
}
#[doc = include_str!("lib_docs/instruction.md")]
#[manyhow]
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    #[derive(FromMeta)]
    struct Args {
        id: String,
    }
    let metas = NestedMeta::parse_meta_list(attr.clone())?;
    let args = Args::from_list(&metas).map_err(darling_error)?;
    let item: Item = syn::parse2(input.clone())?;
    let (ident, generics) = match &item {
        Item::Struct(s) => (s.ident.clone(), s.generics.clone()),
        Item::Enum(e) => (e.ident.clone(), e.generics.clone()),
        _ => return Ok(quote! { #item }),
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let wire_id_lit = syn::LitStr::new(&args.id, proc_macro2::Span::call_site());
    let expanded = quote! {
        #item
        impl #impl_generics #ident #ty_generics #where_clause {
            /// Stable wire identifier for instruction encoding
            pub const WIRE_ID: &'static str = #wire_id_lit;
        }
    };
    Ok(expanded)
}
#[doc = include_str!("lib_docs/id_eq_ord_hash.md")]
#[manyhow]
#[proc_macro_derive(IdEqOrdHash, attributes(id, opaque))]
pub fn id_eq_ord_hash(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let result = id::impl_id_eq_ord_hash(&mut emitter, &input);
    emitter.finish_token_stream_with(result)
}
#[doc = include_str!("lib_docs/has_origin.md")]
#[manyhow]
#[proc_macro_derive(HasOrigin, attributes(has_origin))]
pub fn has_origin_derive(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let result = has_origin::impl_has_origin(&mut emitter, &input);
    emitter.finish_token_stream_with(result)
}
#[doc = include_str!("lib_docs/event_set.md")]
#[manyhow]
#[proc_macro_derive(EventSet)]
pub fn event_set_derive(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let result = event_set::impl_event_set_derive(&mut emitter, &input);
    emitter.finish_token_stream_with(result)
}
/// Derive macro generating registration builders for data model structs.
#[manyhow]
#[proc_macro_derive(RegistrableBuilder, attributes(registrable_builder))]
pub fn registrable_builder(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    let mut emitter = Emitter::new();
    let result = registrable_builder::impl_registrable_builder(&mut emitter, &input);
    Ok(emitter.finish_token_stream_with(result))
}
