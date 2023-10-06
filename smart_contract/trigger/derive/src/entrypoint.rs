//! Module wht [`main`](super::main) macro implementation

use super::*;

mod export {
    pub const TRIGGER_MAIN: &str = "_iroha_trigger_main";
}

/// [`main`](super::main()) macro implementation
#[allow(clippy::needless_pass_by_value)]
pub fn impl_entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        mut block,
    } = parse_macro_input!(item);

    assert!(
        syn::ReturnType::Default == sig.output,
        "Trigger `main()` function must not have a return type"
    );

    let fn_name = &sig.ident;

    block.stmts.insert(
        0,
        parse_quote!(
            use ::iroha_trigger::smart_contract::{
                debug::DebugExpectExt as _, ExecuteOnHost as _, QueryHost as _,
            };
        ),
    );

    let main_fn_name = syn::Ident::new(export::TRIGGER_MAIN, proc_macro2::Span::call_site());

    quote! {
        /// Smart contract entrypoint
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn #main_fn_name() {
            let payload = ::iroha_trigger::get_trigger_payload();
            #fn_name(payload.owner, payload.event)
        }

        // NOTE: Host objects are always passed by value to wasm
        #[allow(clippy::needless_pass_by_value)]
        #(#attrs)*
        #vis #sig
        #block
    }
    .into()
}
