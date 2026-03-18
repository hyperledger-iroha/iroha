use super::*;

/// Register a domain endorsement committee.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RegisterDomainCommittee {
    /// Committee configuration to register.
    pub committee: crate::nexus::DomainCommittee,
}

/// Set or replace the endorsement policy for a domain.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct SetDomainEndorsementPolicy {
    /// Domain requiring endorsements.
    pub domain: crate::domain::DomainId,
    /// Policy to apply.
    pub policy: crate::nexus::DomainEndorsementPolicy,
}

/// Submit an endorsement for a protected domain.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct SubmitDomainEndorsement {
    /// Endorsement to validate and record.
    pub endorsement: crate::nexus::DomainEndorsement,
}

impl crate::seal::Instruction for RegisterDomainCommittee {}
impl crate::seal::Instruction for SetDomainEndorsementPolicy {}
impl crate::seal::Instruction for SubmitDomainEndorsement {}
