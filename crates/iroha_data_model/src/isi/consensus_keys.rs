//! Consensus key lifecycle instructions.

use super::*;

/// Register a consensus/committee key with lifecycle metadata.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RegisterConsensusKey {
    /// Identifier of the key being registered.
    pub id: crate::consensus::ConsensusKeyId,
    /// Key record to register.
    pub record: crate::consensus::ConsensusKeyRecord,
}

/// Rotate an existing consensus key by registering a successor and marking the old one retiring.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RotateConsensusKey {
    /// Identifier of the key being rotated out.
    pub id: crate::consensus::ConsensusKeyId,
    /// Replacement key record (must target the same role).
    pub record: crate::consensus::ConsensusKeyRecord,
}

/// Disable an existing consensus key.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct DisableConsensusKey {
    /// Identifier of the key being disabled.
    pub id: crate::consensus::ConsensusKeyId,
}

impl crate::seal::Instruction for RegisterConsensusKey {}
impl crate::seal::Instruction for RotateConsensusKey {}
impl crate::seal::Instruction for DisableConsensusKey {}
