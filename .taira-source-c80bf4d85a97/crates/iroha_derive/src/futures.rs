#![allow(unused)]

use iroha_derive_primitives::Emitter;
use manyhow::emit;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, ItemFn, ReturnType, Signature};

fn impl_telemetry_future(
    emitter: &mut Emitter,
    ItemFn {
        attrs,
        vis,
        sig,
        block,
    }: ItemFn,
) -> TokenStream {
    let Signature {
        asyncness,
        ident,
        generics: Generics {
            params,
            where_clause,
            ..
        },
        inputs,
        output,
        ..
    } = sig;

    if asyncness.is_none() {
        emit!(
            emitter,
            ident,
            "Only async functions can be instrumented for `telemetry_future`",
        );
    }

    let output = match &output {
        ReturnType::Type(_, tp) => quote! { #tp },
        ReturnType::Default => quote! { () },
    };

    quote! {
        #(#attrs)*
        #vis async fn #ident < #params > ( #inputs ) -> #output
        #where_clause
        {
            let __future_name = concat!(module_path!(), "::", stringify!(#ident));
            iroha_futures::TelemetryFuture::new(async #block, __future_name).await
        }
    }
}

/// Attribute macro implementation that wraps an async block with telemetry.
pub fn telemetry_future_impl(args: &TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    if !args.is_empty() {
        emit!(emitter, args, "Unexpected arguments");
    }

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };
    let result = if cfg!(feature = "telemetry") {
        impl_telemetry_future(&mut emitter, input)
    } else {
        quote! { #input }
    };

    emitter.finish_token_stream_with(result)
}
