use iroha_derive_primitives::Emitter;
use manyhow::emit;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

#[cfg(feature = "telemetry")]
fn instrument_telemetry_future(emitter: &mut Emitter, input: ItemFn) -> TokenStream {
    if input.sig.asyncness.is_none() {
        emit!(
            emitter,
            input.sig.ident,
            "only async functions can be instrumented with `telemetry_future`",
        );
        return quote! { #input };
    }

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = input;
    let ident = &sig.ident;
    quote! {
        #(#attrs)*
        #vis #sig {
            ::iroha_futures::TelemetryFuture::new(
                async #block,
                concat!(module_path!(), "::", stringify!(#ident)),
            )
            .await
        }
    }
}

/// Wrap an async function body with telemetry when the `telemetry` feature is enabled.
pub fn telemetry_future_impl(args: &TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();
    if !args.is_empty() {
        emit!(emitter, args, "telemetry_future does not accept arguments");
    }
    let Some(input): Option<ItemFn> = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    #[cfg(feature = "telemetry")]
    let result = instrument_telemetry_future(&mut emitter, input);
    #[cfg(not(feature = "telemetry"))]
    let result = quote! { #input };

    emitter.finish_token_stream_with(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_arguments() {
        let output =
            telemetry_future_impl(&quote! { unexpected }, quote! { async fn example() {} })
                .to_string();
        assert!(output.contains("telemetry_future does not accept arguments"));
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn preserves_the_complete_function_signature() {
        let mut emitter = Emitter::new();
        let input: ItemFn = syn::parse_quote! {
            #[allow(improper_ctypes_definitions)]
            pub async unsafe extern "C" fn example<T>(value: T) -> T
            where
                T: Send,
            {
                value
            }
        };
        let output = instrument_telemetry_future(&mut emitter, input);
        assert!(emitter.finish_token_stream().is_empty());
        let output: ItemFn = syn::parse2(output).expect("instrumented function should parse");
        assert!(output.sig.asyncness.is_some());
        assert!(output.sig.unsafety.is_some());
        assert!(output.sig.abi.is_some());
        assert_eq!(output.sig.generics.params.len(), 1);
        assert!(output.sig.generics.where_clause.is_some());
        assert_eq!(output.attrs.len(), 1);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn non_async_function_reports_one_diagnostic_without_rewriting_signature() {
        let mut emitter = Emitter::new();
        let input: ItemFn = syn::parse_quote! { fn example() {} };
        let output = instrument_telemetry_future(&mut emitter, input);
        let output = emitter.finish_token_stream_with(output).to_string();
        assert!(output.contains("only async functions"));
        assert!(output.contains("fn example"));
        assert!(!output.contains("async fn example"));
    }
}
