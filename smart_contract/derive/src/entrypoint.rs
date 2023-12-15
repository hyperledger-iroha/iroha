//! Macro for writing smart contract entrypoint

#![allow(clippy::str_to_string)]

use iroha_macro_utils::Emitter;
use manyhow::emit;
use proc_macro2::TokenStream;
use quote::quote;
use syn2::parse_quote;

mod export {
    pub const SMART_CONTRACT_MAIN: &str = "_iroha_smart_contract_main";
}

#[allow(clippy::needless_pass_by_value)]
pub fn impl_entrypoint(emitter: &mut Emitter, item: syn2::ItemFn) -> TokenStream {
    let syn2::ItemFn {
        attrs,
        vis,
        sig,
        mut block,
    } = item;

    if sig.output != syn2::ReturnType::Default {
        emit!(
            emitter,
            "Smart contract entrypoint must not have a return type"
        );
    }

    let fn_name = &sig.ident;

    block.stmts.insert(
        0,
        parse_quote!(
            use ::iroha_smart_contract::{
                debug::DebugExpectExt as _, ExecuteOnHost as _, ExecuteQueryOnHost as _,
            };
        ),
    );

    let main_fn_name =
        syn2::Ident::new(export::SMART_CONTRACT_MAIN, proc_macro2::Span::call_site());

    quote! {
        /// Smart contract entrypoint
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn #main_fn_name() {
            let payload = ::iroha_smart_contract::get_smart_contract_payload();
            #fn_name(payload.owner)
        }

        // NOTE: Host objects are always passed by value to wasm
        #[allow(clippy::needless_pass_by_value)]
        #(#attrs)*
        #[inline]
        #vis #sig
        #block
    }
}
