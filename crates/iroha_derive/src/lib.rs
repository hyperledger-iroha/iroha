//! Crate with various derive macros.
//!
//! Shared diagnostic and representation helpers live in the companion
//! `iroha_derive_primitives` crate so other proc-macro crates can use them
//! without depending on `iroha_derive` directly.
#[cfg(feature = "config_base")]
mod config_base;
mod from_variant;
#[cfg(feature = "futures")]
mod futures;
use manyhow::{Result, manyhow};
use proc_macro2::TokenStream;
use quote::quote;
/// Define the private diagnostic-emitter convenience trait used by Iroha's
/// procedural-macro crates.
///
/// The generated trait deliberately stays crate-private. This macro exists so
/// the consumers that already depend on `iroha_derive` share one maintained
/// definition without adding another workspace dependency edge.
#[proc_macro]
pub fn define_emitter_ext(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if !input.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "define_emitter_ext! accepts no input",
        )
        .to_compile_error()
        .into();
    }
    quote! {
        trait EmitterExt {
            fn handle<E: manyhow::ToTokensError + 'static, T>(
                &mut self,
                result: manyhow::Result<T, E>,
            ) -> Option<T>;

            fn finish_token_stream(self) -> proc_macro2::TokenStream
            where
                Self: Sized;

            fn finish_token_stream_with(
                self,
                tokens: proc_macro2::TokenStream,
            ) -> proc_macro2::TokenStream
            where
                Self: Sized;
        }

        impl EmitterExt for manyhow::Emitter {
            fn handle<E: manyhow::ToTokensError + 'static, T>(
                &mut self,
                result: manyhow::Result<T, E>,
            ) -> Option<T> {
                match result {
                    Ok(value) => Some(value),
                    Err(err) => {
                        self.emit(err);
                        None
                    }
                }
            }

            fn finish_token_stream(self) -> proc_macro2::TokenStream
            where
                Self: Sized,
            {
                self.finish_token_stream_with(proc_macro2::TokenStream::new())
            }

            fn finish_token_stream_with(
                mut self,
                mut tokens: proc_macro2::TokenStream,
            ) -> proc_macro2::TokenStream
            where
                Self: Sized,
            {
                if let Err(err) = self.into_result() {
                    manyhow::ToTokensError::to_tokens(&err, &mut tokens);
                }
                tokens
            }
        }
    }
    .into()
}
/// Helper macro to expand FFI functions
#[manyhow]
#[proc_macro_attribute]
pub fn ffi_impl_opaque(args: TokenStream, item: TokenStream) -> Result<TokenStream> {
    ffi_impl_opaque_impl(args, item).map_err(Into::into)
}

fn ffi_impl_opaque_impl(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(
            args,
            "ffi_impl_opaque does not accept arguments",
        ));
    }
    let item: syn::ItemImpl = syn::parse2(item)?;
    Ok(quote! {
        #[cfg_attr(
            all(feature = "ffi_export", not(feature = "ffi_import")),
            iroha_ffi::ffi_export
        )]
        #[cfg_attr(feature = "ffi_import", iroha_ffi::ffi_import)]
        #item
    })
}
/// [`FromVariant`] is used for implementing `From<Variant> for Enum`
/// and `TryFrom<Enum> for Variant`.
///
/// ```rust
/// use iroha_macro::FromVariant;
///
/// trait MyTrait {}
///
/// #[derive(FromVariant)]
/// enum Obj {
///     Uint(u32),
///     Int(i32),
///     String(String),
///     // You can skip implementing `From`
///     Vec(#[skip_from] Vec<Obj>),
///     // Conversions always use the exact field type; no container is allocated implicitly.
///     Box(Box<dyn MyTrait>)
/// }
///
/// // For example, to avoid:
/// impl<T: Into<Obj>> From<Vec<T>> for Obj {
///     fn from(vec: Vec<T>) -> Self {
///         # stringify!(
///         ...
///         # );
///         # Obj::Uint(vec.len() as u32)
///     }
/// }
/// ```
#[manyhow]
#[proc_macro_derive(FromVariant, attributes(skip_from, skip_try_from))]
pub fn from_variant_derive(input: TokenStream) -> Result<TokenStream> {
    from_variant::impl_from_variant(syn::parse2(input)?).map_err(Into::into)
}
/// Macro for wrapping future for getting telemetry info about poll times and numbers
#[cfg(feature = "futures")]
#[manyhow]
#[proc_macro_attribute]
pub fn telemetry_future(args: TokenStream, input: TokenStream) -> TokenStream {
    futures::telemetry_future_impl(&args, input)
}
/// Derive `iroha_config_base::read::ReadConfig` trait.
#[cfg(feature = "config_base")]
#[manyhow]
#[proc_macro_derive(ReadConfig, attributes(config))]
pub fn derive_read_config(input: TokenStream) -> TokenStream {
    config_base::derive_read_config_impl(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_impl_opaque_rejects_arguments() {
        let item = quote! { impl Example {} };
        let error = ffi_impl_opaque_impl(quote! { unexpected }, item)
            .expect_err("arguments must be rejected");
        assert!(error.to_string().contains("does not accept arguments"));
    }
}
