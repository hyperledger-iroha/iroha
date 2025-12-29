//! Validator lifecycle helpers and identifiers.

use norito::codec::{Decode, Encode};

/// Mode for validator admission and activation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "mode", content = "value"))]
pub enum ValidatorMode {
    /// Permissioned networks: admins register peers directly; staking admission is bypassed.
    Permissioned,
    /// Public `NPoS`: validators activate via stake-backed admission and epoch elections.
    PublicNpos,
}
