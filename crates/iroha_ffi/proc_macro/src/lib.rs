//! Crate containing FFI related macro functionality
use crate::{
    attr_parse::derive::Derive,
    convert::{FfiTypeData, FfiTypeInput, derive_ffi_type},
    emitter_ext::EmitterExt,
    utils::darling_result,
};
use darling::FromDeriveInput;
use impl_visitor::{FnDescriptor, ImplDescriptor};
use manyhow::{Emitter, emit, manyhow};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;
use wrapper::wrap_method;
mod attr_parse;
mod convert;
mod emitter_ext;
mod ffi_fn;
mod getset_gen;
mod impl_visitor;
mod utils;
mod wrapper;
struct FfiItems(Vec<FfiTypeInput>);
impl syn::parse::Parse for FfiItems {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            let input = input.parse::<syn::DeriveInput>()?;
            let input = FfiTypeInput::from_derive_input(&input)?;
            items.push(input);
        }
        Ok(Self(items))
    }
}
/// A test utility function that parses multiple attributes
#[cfg(test)]
fn parse_attributes(ts: TokenStream) -> Vec<syn::Attribute> {
    struct Attributes(Vec<syn::Attribute>);
    impl syn::parse::Parse for Attributes {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            syn::Attribute::parse_outer(input).map(Attributes)
        }
    }
    syn::parse2::<Attributes>(ts)
        .expect("Failed to parse attributes")
        .0
}
#[doc = include_str!("lib_docs/ffi.md")]
#[manyhow]
#[proc_macro]
pub fn ffi(input: TokenStream) -> TokenStream {
    let items = match syn::parse2::<FfiItems>(input) {
        Ok(items) => items.0,
        Err(err) => return err.to_compile_error(),
    };
    let mut emitter = Emitter::new();
    let items = items
        .into_iter()
        .map(|item| {
            if !matches!(item.vis, syn::Visibility::Public(_)) {
                emit!(emitter, item.span, "Only public types are allowed in FFI");
            }
            if !item.is_opaque() {
                let item = item.ast;
                return quote! {
                    #[derive(iroha_ffi::FfiType)]
                    #item
                };
            }
            if let FfiTypeData::Struct(fields) = &item.data
                && item
                    .derive_attr
                    .derives
                    .iter()
                    .any(|d| matches!(d, Derive::GetSet(_)))
            {
                let derived_methods: Vec<_> = getset_gen::gen_derived_methods(
                    &mut emitter,
                    &item.ident,
                    &item.derive_attr,
                    &item.getset_attr,
                    fields,
                )
                .collect();
                let ffi_fns: Vec<_> = derived_methods
                    .iter()
                    .map(|fn_| ffi_fn::gen_declaration(fn_, None))
                    .collect();
                let impl_block = wrapper::wrap_impl_items(&ImplDescriptor {
                    attrs: Vec::new(),
                    trait_name: None,
                    associated_types: Vec::new(),
                    fns: derived_methods,
                });
                let opaque = wrapper::wrap_as_opaque(&mut emitter, item);
                return quote! {
                    #opaque
                    #impl_block
                    #(#ffi_fns)*
                };
            }
            wrapper::wrap_as_opaque(&mut emitter, item)
        })
        .collect::<Vec<_>>();
    emitter.finish_token_stream_with(quote! { #(#items)* })
}
// NOTE: `ffi_type(local)` should be reserved for enums that truly borrow stack-bound
// data and therefore cannot implement `NonLocal`. Most data-carrying enums no longer
// require this escape hatch.
#[doc = include_str!("lib_docs/ffi_type.md")]
#[manyhow]
#[proc_macro_derive(FfiType, attributes(ffi_type))]
pub fn ffi_type_derive(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    let Some(item) = emitter.handle(syn::parse2::<syn::DeriveInput>(input)) else {
        return emitter.finish_token_stream();
    };
    if !matches!(item.vis, syn::Visibility::Public(_)) {
        emit!(emitter, item, "Only public types are allowed in FFI");
    }
    let result = derive_ffi_type(&mut emitter, &item);
    emitter.finish_token_stream_with(result)
}
#[doc = include_str!("lib_docs/ffi_export.md")]
#[manyhow]
#[proc_macro_attribute]
pub fn ffi_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = match syn::parse2::<Item>(item) {
        Ok(item) => item,
        Err(err) => return err.to_compile_error(),
    };
    let mut emitter = Emitter::new();
    if !attr.is_empty() {
        emit!(emitter, item, "Unknown tokens in the attribute");
    }
    let result = match item {
        Item::Impl(item) => {
            let Some(impl_descriptor) = ImplDescriptor::from_impl(&mut emitter, &item) else {
                // continuing here creates a lot of dubious errors
                return emitter.finish_token_stream();
            };
            let ffi_fns = impl_descriptor
                .fns
                .iter()
                .map(|fn_| ffi_fn::gen_definition(fn_, impl_descriptor.trait_name()));
            quote! {
                #item
                #(#ffi_fns)*
            }
        }
        Item::Fn(item) => {
            let Some(fn_descriptor) = FnDescriptor::from_fn(&mut emitter, &item) else {
                // continuing here creates a lot of dubious errors
                return emitter.finish_token_stream();
            };
            let ffi_fn = ffi_fn::gen_definition(&fn_descriptor, None);
            quote! {
                #item
                #ffi_fn
            }
        }
        Item::Struct(item) => {
            // re-parse as a DeriveInput to utilize darling
            let input = syn::parse2(quote!(#item)).unwrap();
            let Some(input) =
                emitter.handle(darling_result(FfiTypeInput::from_derive_input(&input)))
            else {
                return emitter.finish_token_stream();
            };
            // we don't need ffi fns for getset accessors if the type is not opaque or there are no accessors
            if !input.is_opaque()
                || !input
                    .derive_attr
                    .derives
                    .iter()
                    .any(|d| matches!(d, Derive::GetSet(_)))
            {
                let input = input.ast;
                return emitter.finish_token_stream_with(quote! { #input });
            }
            let darling::ast::Data::Struct(fields) = &input.data else {
                unreachable!("We parsed struct above");
            };
            if !input.generics.params.is_empty() {
                emit!(
                    emitter,
                    input.generics,
                    "Generics on derived methods not supported"
                );
                // continuing codegen results in a lot of spurious errors
                return emitter.finish_token_stream();
            }
            let derived_ffi_fns = getset_gen::gen_derived_methods(
                &mut emitter,
                &input.ident,
                &input.derive_attr,
                &input.getset_attr,
                fields,
            )
            .map(|fn_| ffi_fn::gen_definition(&fn_, None));
            quote! {
                #item
                #(#derived_ffi_fns)*
            }
        }
        Item::Enum(item) => quote! { #item },
        Item::Union(item) => quote! { #item },
        item => {
            emit!(emitter, item, "Item not supported");
            quote!()
        }
    };
    emitter.finish_token_stream_with(result)
}
#[doc = include_str!("lib_docs/ffi_import.md")]
#[manyhow]
#[proc_macro_attribute]
pub fn ffi_import(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = match syn::parse2::<Item>(item) {
        Ok(item) => item,
        Err(err) => return err.to_compile_error(),
    };
    let mut emitter = Emitter::new();
    if !attr.is_empty() {
        emit!(emitter, item, "Unknown tokens in the attribute");
    }
    let result = match item {
        Item::Impl(item) => {
            let attrs = &item.attrs;
            let Some(impl_desc) = ImplDescriptor::from_impl(&mut emitter, &item) else {
                // continuing codegen results in a lot of spurious errors
                return emitter.finish_token_stream();
            };
            let wrapped_items = wrapper::wrap_impl_items(&impl_desc);
            let is_shared_fn = impl_desc
                .trait_name
                .filter(|name| {
                    name.is_ident("Clone")
                        || name.is_ident("PartialEq")
                        || name.is_ident("PartialOrd")
                        || name.is_ident("Eq")
                        || name.is_ident("Ord")
                })
                .is_some();
            let ffi_fns = if is_shared_fn {
                Vec::new()
            } else {
                impl_desc
                    .fns
                    .iter()
                    .map(|fn_| ffi_fn::gen_declaration(fn_, impl_desc.trait_name()))
                    .collect()
            };
            quote! {
                #(#attrs)*
                #wrapped_items
                #(#ffi_fns)*
            }
        }
        Item::Fn(item) => {
            let Some(fn_descriptor) = FnDescriptor::from_fn(&mut emitter, &item) else {
                // continuing here creates a lot of dubious errors
                return emitter.finish_token_stream();
            };
            let ffi_fn = ffi_fn::gen_declaration(&fn_descriptor, None);
            let wrapped_item = wrap_method(&fn_descriptor, None);
            quote! {
                #wrapped_item
                #ffi_fn
            }
        }
        Item::Struct(item) => quote! { #item },
        Item::Enum(item) => quote! { #item },
        Item::Union(item) => quote! { #item },
        item => {
            emit!(emitter, item, "Item not supported");
            quote!()
        }
    };
    emitter.finish_token_stream_with(result)
}
