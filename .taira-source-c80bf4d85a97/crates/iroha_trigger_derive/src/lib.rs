//! Crate with trigger procedural macros.

use manyhow::{Emitter, emit, manyhow};
use proc_macro2::TokenStream;

mod emitter_ext;
use crate::emitter_ext::EmitterExt;
mod entrypoint;

/// Annotate the user-defined function that starts the execution of the trigger.
///
/// Requires function to accept two arguments of types:
/// 1. `host: Iroha` - handle to the host system (use it to execute instructions and queries)
/// 2. `context: Context` - context of the execution (authority, triggering event, etc)
///
/// # Panics
///
/// - If function has a return type
///
/// # Examples
///
/// ```ignore
/// use iroha_trigger::prelude::*;
///
/// #[main]
/// fn main(host: Iroha, context: Context) {
///     // trigger execution logic goes here
///     let _ = (host, context);
/// }
/// ```
#[manyhow]
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    if !attr.is_empty() {
        emit!(emitter, "#[main] attribute does not accept arguments");
    }

    let Some(item) = emitter.handle(syn::parse2(item)) else {
        return emitter.finish_token_stream();
    };

    let result = entrypoint::impl_entrypoint(&mut emitter, item);

    emitter.finish_token_stream_with(result)
}
