//! Crate with executor-related derive macros.
mod parameter;
mod permission;
use manyhow::{Result, manyhow};
use proc_macro2::TokenStream;
/// Derive macro for `Parameter` trait.
#[manyhow]
#[proc_macro_derive(Parameter)]
pub fn derive_parameter(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    Ok(parameter::impl_derive_parameter(&input))
}
/// Derive macro for `Permission` trait.
///
/// Implements `iroha_executor_data_model::permission::Permission` and the
/// conversions between the typed token and the data-model permission object.
/// Core validates grants, revocations and use of every built-in token natively.
///
/// # Example
///
/// ```ignore
/// use iroha_data_model::{asset::AssetId, permission::Permission as PermissionObject};
/// use iroha_executor_data_model::permission::Permission;
///
/// #[derive(
///     Debug,
///     Clone,
///     PartialEq,
///     Eq,
///     Permission,
///     iroha_schema::IntoSchema,
///     norito::derive::JsonSerialize,
///     norito::derive::JsonDeserialize,
/// )]
/// struct CanDoSomethingWithAsset {
///     asset: AssetId,
/// }
///
/// let token = CanDoSomethingWithAsset { asset };
/// let object = PermissionObject::from(token.clone());
/// assert_eq!(object.name(), &CanDoSomethingWithAsset::name());
/// assert_eq!(CanDoSomethingWithAsset::try_from(&object)?, token);
/// ```
#[manyhow]
#[proc_macro_derive(Permission)]
pub fn derive_permission(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    Ok(permission::impl_derive_permission(&input))
}
