//! Crate with various derive macros.
//!
//! Shared helper utilities (e.g., `Emitter`, attribute parsers) live in the
//! companion `iroha_derive_primitives` crate so other proc-macro crates can use
//! them without depending on `iroha_derive` directly.

use darling::{Error as DarlingError, FromDeriveInput as _};

#[cfg(feature = "config_base")]
mod config_base;
mod from_variant;
#[cfg(feature = "futures")]
mod futures;

use manyhow::{Result, ToTokensError, manyhow};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Helper macro to expand FFI functions
#[manyhow]
#[proc_macro_attribute]
pub fn ffi_impl_opaque(_: TokenStream, item: TokenStream) -> Result<TokenStream> {
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
/// use iroha_derive::FromVariant;
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
///     // You can also skip implementing `From` for item inside containers such as `Box`
///     Box(#[skip_container] Box<dyn MyTrait>)
/// }
///
/// // For example, to avoid:
/// impl<T: Into<Obj>> From<Vec<T>> for Obj {
///     fn from(vec: Vec<T>) -> Self {
///         # stringify!(
///         ...
///         # );
///         # unimplemented!()
///     }
/// }
/// ```
#[manyhow]
#[proc_macro_derive(FromVariant, attributes(skip_from, skip_try_from, skip_container))]
pub fn from_variant_derive(input: TokenStream) -> Result<TokenStream> {
    let ast = syn::parse2(input)?;
    let ast = from_variant::FromVariantInput::from_derive_input(&ast).map_err(darling_error)?;
    Ok(from_variant::impl_from_variant(&ast))
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

#[derive(Debug)]
struct DarlingErrorWrapper(DarlingError);

impl ToTokensError for DarlingErrorWrapper {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.clone().write_errors().to_tokens(tokens);
    }
}

fn darling_error(err: DarlingError) -> manyhow::Error {
    manyhow::Error::from(DarlingErrorWrapper(err))
}
