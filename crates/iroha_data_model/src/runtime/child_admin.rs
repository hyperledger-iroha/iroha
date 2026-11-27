//! Explicit threshold administration of an independently founded child execution root.
//!
//! These records grant no authority by themselves. Core must authenticate the child from
//! signed genesis, require the selected child-admin service, and read its committed policy.
//! Validator release support remains separate from administrative approval.

use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_model_base::topology::DataSpaceId;
use norito::codec::{Decode, Encode};

use super::lifecycle::{ChangeSetEndorsementV1, ChangeSetId, ChangeSetV1, RootScopeV1};
use crate::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    parameter::{
        runtime_provenance_policy::RuntimeProvenancePolicyV1,
        runtime_upgrade_policy::RuntimeUpgradePolicyV1,
    },
};

/// Original signed-genesis recipe; ordinary parameter writes never initialize authority.
pub mod foundation;

/// Dormant selected service; neither an account role nor validator membership selects it.
pub const CHILD_ADMIN_SERVICE_V1: &str = "child_runtime_admin_v1";
/// Exact semantics of the new service, independent of every installed Universal definition.
pub const CHILD_ADMIN_SERVICE_CLOSURE_V1: &[u8] =
    b"iroha:runtime:child-admin:v1:genesis-root-explicit-threshold-policy-nonce-sole-native-signed-source-schedule-cancel-rotate";
/// Maximum distinct administrative or provenance signers in one policy.
pub const MAX_CHILD_ADMIN_SIGNERS_V1: usize = 64;
/// Maximum complete canonical policy frame.
pub const MAX_CHILD_ADMIN_POLICY_BYTES_V1: usize = 64 * 1024;
/// Maximum complete canonical plan or certificate frame.
pub const MAX_CHILD_ADMIN_CERTIFICATE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Policy identity domain.
pub const CHILD_ADMIN_POLICY_DOMAIN_V1: &[u8] = b"iroha:runtime:child-admin:policy:v1\0";
/// Exact action-plan identity domain.
pub const CHILD_ADMIN_PLAN_DOMAIN_V1: &[u8] = b"iroha:runtime:child-admin:plan:v1\0";
/// Administrative signature domain, distinct from validator and release-signing domains.
pub const CHILD_ADMIN_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:runtime:child-admin:signature:v1\0";

/// Complete immutable identity authenticated by the child's own signed genesis.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRootIdentityV1")]
#[norito(deny_unknown_fields)]
pub struct ChildRootIdentityV1 {
    /// The child's genesis hash; never the parent's network or a display chain label.
    pub network: NetworkId,
    /// Parent network authenticated in the child's genesis scope.
    pub parent_network: NetworkId,
    /// Full-width, non-universal data space identity.
    pub dataspace: DataSpaceId,
    /// Native instance derived from child network, chain label and signed root scope.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub instance: [u8; 32],
}

impl ChildRootIdentityV1 {
    /// Validate shape only; authentication belongs to the original genesis owner in Core.
    pub fn validate(&self) -> Result<(), ChildAdminErrorV1> {
        if self.dataspace == DataSpaceId::UNIVERSAL || self.instance == [0; 32] {
            return Err(ChildAdminErrorV1::Shape("invalid child root identity"));
        }
        Ok(())
    }

    /// The immutable signed consensus scope corresponding to these identity fields.
    #[must_use]
    pub const fn scope(&self) -> SumeragiRootScope {
        SumeragiRootScope::Dataspace {
            parent_network_id: self.parent_network,
            dataspace_id: self.dataspace,
        }
    }
}

/// Explicit committed authority. There is deliberately no default administrator or threshold.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRuntimeAdminPolicyV1")]
#[norito(deny_unknown_fields)]
pub struct ChildRuntimeAdminPolicyV1 {
    /// Starts at one; a rotation advances it by exactly one.
    pub revision: u64,
    /// Explicit administrative signing keys, strictly sorted and unique.
    pub admins: Vec<PublicKey>,
    /// Number of distinct current administrator signatures required.
    pub threshold: u16,
    /// Committed notice, frozen into every accepted scheduled change.
    pub notice: RuntimeUpgradePolicyV1,
    /// Explicit release-provenance requirement; `None` deliberately requires no endorsements.
    pub provenance: Option<RuntimeProvenancePolicyV1>,
}

// The existing policy types need not gain traits or change their frozen definitions.
impl Ord for ChildRuntimeAdminPolicyV1 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (
            self.revision,
            &self.admins,
            self.threshold,
            self.notice.activation,
            self.notice.disable,
            self.provenance.as_ref().map(|p| (&p.signers, p.threshold)),
        )
            .cmp(&(
                other.revision,
                &other.admins,
                other.threshold,
                other.notice.activation,
                other.notice.disable,
                other.provenance.as_ref().map(|p| (&p.signers, p.threshold)),
            ))
    }
}
impl PartialOrd for ChildRuntimeAdminPolicyV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ChildRuntimeAdminPolicyV1 {
    /// Validate finite authority and notice before any signature verification or mutation.
    pub fn validate(&self) -> Result<(), ChildAdminErrorV1> {
        if self.revision == 0
            || self.admins.is_empty()
            || self.admins.len() > MAX_CHILD_ADMIN_SIGNERS_V1
            || !self.admins.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(ChildAdminErrorV1::Shape("invalid admin roster or revision"));
        }
        if self.threshold == 0 || usize::from(self.threshold) > self.admins.len() {
            return Err(ChildAdminErrorV1::Shape("invalid admin threshold"));
        }
        if self.notice.activation.blocks == 0 || self.notice.validate().is_err() {
            return Err(ChildAdminErrorV1::Shape("invalid committed upgrade notice"));
        }
        if let Some(provenance) = &self.provenance
            && (provenance.signers.len() > MAX_CHILD_ADMIN_SIGNERS_V1
                || provenance.validate().is_err())
        {
            return Err(ChildAdminErrorV1::Shape(
                "invalid release provenance policy",
            ));
        }
        check_size(self, MAX_CHILD_ADMIN_POLICY_BYTES_V1)
    }

    /// Identity of every policy field, including its explicit notice and provenance decisions.
    pub fn id(&self) -> Result<[u8; 32], ChildAdminErrorV1> {
        self.validate()?;
        digest(CHILD_ADMIN_POLICY_DOMAIN_V1, self)
    }
}

/// Addition-owned canonical authority and replay cursor of exactly one child root.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRuntimeAdminStateV1")]
#[norito(deny_unknown_fields)]
pub struct ChildRuntimeAdminStateV1 {
    /// Root bound by the original authenticated foundation, never by local configuration.
    pub root: ChildRootIdentityV1,
    /// Currently committed administrative authority.
    pub policy: ChildRuntimeAdminPolicyV1,
    /// Exact next certificate nonce; successful actions increment it without resetting on rotation.
    pub next_nonce: u64,
}

impl ChildRuntimeAdminStateV1 {
    /// Validate a restored state; this does not establish its genesis provenance or selection.
    pub fn validate(&self) -> Result<(), ChildAdminErrorV1> {
        self.root.validate()?;
        self.policy.validate()
    }
}

/// The complete action authorized by the current committed administrator threshold.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRuntimeAdminActionV1")]
#[norito(tag = "kind", content = "content")]
pub enum ChildRuntimeAdminActionV1 {
    /// Schedule an exact child-root change; validators still independently attest before activation.
    Schedule {
        /// Exact definitions, transitions, qualification and boundary authorized by the admins.
        change_set: ChangeSetV1,
        /// Release endorsements judged only by the explicitly committed provenance policy.
        provenance: Vec<ChangeSetEndorsementV1>,
    },
    /// Cancel one still-scheduled change on this child, never a parent's change.
    Cancel(ChangeSetId),
    /// Replace policy under the current policy's signatures, advancing its revision exactly once.
    RotatePolicy(ChildRuntimeAdminPolicyV1),
}

/// Every field signed by administrators. Validity uses certified block heights.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRuntimeAdminPlanV1")]
#[norito(deny_unknown_fields)]
pub struct ChildRuntimeAdminPlanV1 {
    /// Exact child instance, preventing replay under another parent or after a genesis reset.
    pub root: ChildRootIdentityV1,
    /// Exact current policy identity, including its administrator roster and notice.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub policy_id: [u8; 32],
    /// Current committed revision, retained explicitly for review and stale-policy refusal.
    pub policy_revision: u64,
    /// Exact current replay cursor.
    pub nonce: u64,
    /// First certified height at which this approval may be consumed, inclusive.
    pub valid_from: u64,
    /// First certified height at which this approval is expired, exclusive.
    pub valid_until: u64,
    /// Complete intended effect.
    pub action: ChildRuntimeAdminActionV1,
}

impl ChildRuntimeAdminPlanV1 {
    /// Validate root, action shape, validity and canonical byte bounds without claiming authority.
    pub fn validate(&self) -> Result<(), ChildAdminErrorV1> {
        self.root.validate()?;
        if self.policy_revision == 0
            || self.policy_id == [0; 32]
            || self.valid_from == 0
            || self.valid_from >= self.valid_until
        {
            return Err(ChildAdminErrorV1::Shape(
                "invalid policy or validity interval",
            ));
        }
        match &self.action {
            ChildRuntimeAdminActionV1::Schedule {
                change_set,
                provenance,
            } => {
                if change_set.network != self.root.network
                    || change_set.root != RootScopeV1::DataSpace(self.root.dataspace)
                    || change_set.effective_height < 2
                    || provenance.len() > MAX_CHILD_ADMIN_SIGNERS_V1
                {
                    return Err(ChildAdminErrorV1::Shape(
                        "foreign or malformed scheduled change",
                    ));
                }
            }
            ChildRuntimeAdminActionV1::RotatePolicy(next) => {
                next.validate()?;
                if self.policy_revision.checked_add(1) != Some(next.revision) {
                    return Err(ChildAdminErrorV1::Shape(
                        "rotation must advance policy revision once",
                    ));
                }
            }
            ChildRuntimeAdminActionV1::Cancel(_) => {}
        }
        check_size(self, MAX_CHILD_ADMIN_CERTIFICATE_BYTES_V1)
    }

    /// Canonical bounded plan bytes; no ambient Norito layout flag selects their encoding.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ChildAdminErrorV1> {
        self.validate()?;
        norito::core::classify_decode_attempt(|| norito::encode_canonical(self))
            .map_err(ChildAdminErrorV1::Resource)
    }

    /// Domain-separated signature preimage covering every plan field.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, ChildAdminErrorV1> {
        let bytes = self.to_bytes()?;
        norito::core::classify_decode_attempt(|| {
            let length = CHILD_ADMIN_SIGNATURE_DOMAIN_V1
                .len()
                .checked_add(bytes.len())
                .ok_or(norito::Error::LengthMismatch)?;
            norito::core::reserve_decode_allocation(length)?;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(length)
                .map_err(|_| norito::Error::AllocationFailed {
                    bytes: u64::try_from(length).unwrap_or(u64::MAX),
                })?;
            payload.extend_from_slice(CHILD_ADMIN_SIGNATURE_DOMAIN_V1);
            payload.extend_from_slice(&bytes);
            Ok(payload)
        })
        .map_err(ChildAdminErrorV1::Resource)
    }

    /// Content identity of the complete intended action and authority context.
    pub fn id(&self) -> Result<[u8; 32], ChildAdminErrorV1> {
        self.validate()?;
        digest(CHILD_ADMIN_PLAN_DOMAIN_V1, self)
    }
}

/// One administrator's signature over the entire domain-separated plan.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildAdminSignatureV1")]
#[norito(deny_unknown_fields)]
pub struct ChildAdminSignatureV1 {
    /// Exact explicit policy member; membership is checked against committed State.
    pub signer: PublicKey,
    /// Signature under the administrative domain, not a release or validator attestation.
    pub signature: Signature,
}

impl ChildAdminSignatureV1 {
    /// Sign a structurally valid plan without inferring policy membership.
    pub fn sign(plan: &ChildRuntimeAdminPlanV1, key: &KeyPair) -> Result<Self, ChildAdminErrorV1> {
        Ok(Self {
            signer: key.public_key().clone(),
            signature: Signature::try_new(key.private_key(), &plan.signing_bytes()?)
                .map_err(|_| ChildAdminErrorV1::Signing)?,
        })
    }
}

/// Bounded, sorted distinct administrator signatures; no quorum aggregation hides duplicates.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::child_admin::ChildRuntimeAdminCertificateV1")]
#[norito(deny_unknown_fields)]
pub struct ChildRuntimeAdminCertificateV1 {
    /// Exact intended effect and its full root, policy, nonce and validity binding.
    pub plan: ChildRuntimeAdminPlanV1,
    /// Strictly sorted by signer; every provided signature must be valid and authorized.
    pub signatures: Vec<ChildAdminSignatureV1>,
}

impl ChildRuntimeAdminCertificateV1 {
    /// Verify structural bounds and each claimed signature; policy authority is not established.
    pub fn verify_signatures(&self) -> Result<(), ChildAdminErrorV1> {
        if self.signatures.is_empty()
            || self.signatures.len() > MAX_CHILD_ADMIN_SIGNERS_V1
            || !self
                .signatures
                .windows(2)
                .all(|pair| pair[0].signer < pair[1].signer)
        {
            return Err(ChildAdminErrorV1::Shape(
                "invalid certificate signer order or count",
            ));
        }
        check_size(self, MAX_CHILD_ADMIN_CERTIFICATE_BYTES_V1)?;
        let bytes = self.plan.signing_bytes()?;
        for (index, signed) in self.signatures.iter().enumerate() {
            signed
                .signature
                .verify(&signed.signer, &bytes)
                .map_err(|error| match error {
                    iroha_crypto::Error::UnsupportedImplementation(original) => {
                        ChildAdminErrorV1::CryptoUnavailable(original)
                    }
                    _ => ChildAdminErrorV1::BadSignature(index),
                })?;
        }
        Ok(())
    }

    /// Verify current policy, threshold, root, validity and nonce against supplied state.
    /// Core must obtain that state from the selected authenticated child's canonical World.
    pub fn verify_against(
        &self,
        state: &ChildRuntimeAdminStateV1,
        height: u64,
    ) -> Result<(), ChildAdminErrorV1> {
        state.validate()?;
        if self.plan.root != state.root {
            return Err(ChildAdminErrorV1::RootMismatch);
        }
        if self.plan.policy_revision != state.policy.revision
            || self.plan.policy_id != state.policy.id()?
        {
            return Err(ChildAdminErrorV1::PolicyMismatch);
        }
        if self.plan.nonce != state.next_nonce || state.next_nonce == u64::MAX {
            return Err(ChildAdminErrorV1::Nonce);
        }
        if height < self.plan.valid_from || height >= self.plan.valid_until {
            return Err(ChildAdminErrorV1::Validity);
        }
        self.verify_signatures()?;
        if self.signatures.len() < usize::from(state.policy.threshold) {
            return Err(ChildAdminErrorV1::Threshold);
        }
        if self
            .signatures
            .iter()
            .any(|signed| state.policy.admins.binary_search(&signed.signer).is_err())
        {
            return Err(ChildAdminErrorV1::UnauthorizedSigner);
        }
        Ok(())
    }

    /// Exact bounded canonical certificate. Signature consistency alone grants no root authority.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ChildAdminErrorV1> {
        self.verify_signatures()?;
        norito::core::classify_decode_attempt(|| norito::encode_canonical(self))
            .map_err(ChildAdminErrorV1::Resource)
    }

    /// Decode a bounded exact frame, rejecting trailing bytes and inconsistent signatures.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ChildAdminErrorV1> {
        if bytes.len() > MAX_CHILD_ADMIN_CERTIFICATE_BYTES_V1 {
            return Err(ChildAdminErrorV1::TooLarge);
        }
        let certificate: Self =
            norito::core::classify_decode_attempt(|| norito::decode_canonical(bytes))
                .map_err(ChildAdminErrorV1::Resource)?;
        if certificate.to_bytes()? != bytes {
            return Err(ChildAdminErrorV1::Encoding);
        }
        Ok(certificate)
    }
}

/// Stable validation failures of the new child-administration contract.
#[derive(Debug, thiserror::Error)]
pub enum ChildAdminErrorV1 {
    /// Original codec attempt, retaining any local allocator or admission refusal.
    #[error("child-admin canonical encoding did not complete: {0}")]
    Resource(#[source] norito::core::DecodeAttemptError),
    /// The explicit algorithm exists but this node lacks its verification implementation.
    #[error("child-admin verification implementation unavailable: {0}")]
    CryptoUnavailable(#[source] iroha_crypto::UnsupportedImplementation),
    /// Original JSON attempt, retaining any local decoder refusal.
    #[error("child-admin JSON encoding did not complete: {0}")]
    Json(#[source] norito::json::Error),
    /// A required finite, canonical protocol invariant is missing.
    #[error("invalid child-admin shape: {0}")]
    Shape(&'static str),
    /// Complete canonical bytes exceed the explicit bound.
    #[error("child-admin canonical bytes exceed their bound")]
    TooLarge,
    /// The record does not have an exact canonical encoding.
    #[error("invalid child-admin canonical encoding")]
    Encoding,
    /// The configured private key could not sign.
    #[error("child-admin signature creation failed")]
    Signing,
    /// A claimed signature failed, even if other signatures already reach threshold.
    #[error("invalid child-admin signature at index {0}")]
    BadSignature(usize),
    /// The full authenticated child identity differs.
    #[error("child-admin action names another root")]
    RootMismatch,
    /// The signed policy identity or revision is no longer current.
    #[error("child-admin action names another policy")]
    PolicyMismatch,
    /// The nonce was consumed, is out of order, or the monotonic cursor is exhausted.
    #[error("child-admin action nonce is not the next consumable nonce")]
    Nonce,
    /// The certified block height lies outside the signed interval.
    #[error("child-admin approval is outside its height interval")]
    Validity,
    /// Too few distinct current administrators approved the plan.
    #[error("child-admin approval falls short of the committed threshold")]
    Threshold,
    /// A provided signer is absent from the committed admin roster.
    #[error("child-admin certificate contains a non-administrator")]
    UnauthorizedSigner,
}

fn check_size(value: &impl norito::NoritoSerialize, limit: usize) -> Result<(), ChildAdminErrorV1> {
    if norito::core::classify_decode_attempt(|| norito::canonical_frame_len(value))
        .map_err(ChildAdminErrorV1::Resource)?
        > limit
    {
        return Err(ChildAdminErrorV1::TooLarge);
    }
    Ok(())
}

fn digest(
    domain: &[u8],
    value: &impl norito::NoritoSerialize,
) -> Result<[u8; 32], ChildAdminErrorV1> {
    norito::core::classify_decode_attempt(|| {
        let mut encoded = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(domain)?;
            // Retain the original codec error separately from the infallible hash sink.
            // The preimage remains domain || canonical frame, without retaining that frame.
            encoded = norito::core::write_canonical_to_writer(value, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(hash.into())
    })
    .map_err(ChildAdminErrorV1::Resource)
}

#[cfg(test)]
mod tests;
