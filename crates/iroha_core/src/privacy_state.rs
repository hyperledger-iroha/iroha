//! Durable first-release privacy admission state.
//!
//! Privacy state is split across independent typed world-state maps. This keeps
//! one admission proportional to its actual effects instead of cloning or
//! conflicting with the complete privacy ledger. Every map still participates
//! in the same [`crate::state::StateTransaction`], so a rejected transaction
//! cannot leave a partial replay marker, commitment, or root behind.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    AssetDefinitionId,
    account::AccountId,
    asset::AssetBalanceScope,
    privacy::{
        ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1, BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1,
        BootleLanternIssuerPolicyV1, FCMP_MAX_INPUTS_V1, FCMP_MAX_OUTPUTS_V1,
        IVM_PRIVATE_NOTE_MAX_INPUTS_V1, IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
        IrohaZkX509StarkP256StatementV1, ORCHARD_MAX_ACTIONS_V1, PQ_MASP_MAX_INPUTS_V1,
        PQ_MASP_MAX_OUTPUTS_V1, PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1,
        PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1, PRIVACY_ZK_ACE_MAX_POLICIES_V1,
        PrivacyActivationValidationError, PrivacyCommitmentV1, PrivacyConsensusLimitsV1,
        PrivacyConsensusPolicyV1, PrivacyFcmpKeyImageV1, PrivacyFcmpOutputIdV1,
        PrivacyFcmpOutputTupleV1, PrivacyFcmpTreeRootV1, PrivacyIssuerIdV1,
        PrivacyNamespaceScopeV1, PrivacyNamespaceV1, PrivacyNullifierV1,
        PrivacyOrchardPoolBootstrapDigestV1, PrivacyOrchardPoolBootstrapV1,
        PrivacyP256CiphertextV1, PrivacyP256PointV1,
        PrivacyPgcAccountBootstrapDigestV1, PrivacyPgcAccountV1, PrivacyPgcBootstrapProofDigestV1,
        PrivacyPolicyIdV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1,
        PrivacyProofManagedPoolBootstrapDigestV1, PrivacyProofManagedPoolBootstrapV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
        PrivacyRootManagementV1, PrivacyRootPublicationDigestV1, PrivacyRootPublicationV1,
        PrivacyRootRoleV1, PrivacyRootV1, PrivacyStatementDigestV1, PrivacyStatementV1,
        PrivacyTrustAnchorNamespaceV1, PrivacyTrustAnchorPolicyNamespaceV1,
        PrivacyVegaIssuerRecordLifecycleV1, PrivacyVegaIssuerRecordV1,
        PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordV1,
        PrivacyZkAmsIssuerPolicyRecordDigestV1, PrivacyZkAmsKeyImageV1, PrivacyZkAmsPhcHashV1,
        PrivacyZkAmsRegistryBootstrapDigestV1, PrivacyZkAmsSeedPublicKeyV1,
        PrivacyZkX509CertificatePolicyRecordDigestV1, PrivacyZkX509CertificatePolicyRecordV1,
        PrivacyZkX509CrlRecordDigestV1, PrivacyZkX509CrlRecordV1, PrivacyZkX509RecordLifecycleV1,
        PrivacyZkX509TrustAnchorRecordDigestV1, PrivacyZkX509TrustAnchorRecordV1,
        VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1, VEGA_MAX_ISSUER_RECORDS_V1,
        ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1, ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1,
        ZK_X509_MAX_CRL_AGE_SECONDS_V1, ZK_X509_MAX_CRL_LINEAGES_V1,
        ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1, ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1,
        validate_vega_issuer_revocation_v1, validate_vega_issuer_rotation_v1,
        validate_zk_x509_certificate_policy_revocation_v1,
        validate_zk_x509_certificate_policy_rotation_v1,
        validate_zk_x509_trust_anchor_revocation_v1, validate_zk_x509_trust_anchor_rotation_v1,
    },
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use thiserror::Error;

mod pgc_account_root;

pub(crate) use pgc_account_root::compute_privacy_pgc_account_state_root_v1;
pub use pgc_account_root::{
    PrivacyPgcAccountStateRootErrorV1, derive_privacy_pgc_account_state_root_v1,
};

const PRIVACY_PGC_POOL_INVARIANT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:pgc-pool-invariant:v1";

/// Typed key for the immutable activation registered for one protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct PrivacyActivationKeyV1 {
    protocol_id: PrivacyProtocolIdV1,
}

impl PrivacyActivationKeyV1 {
    /// Construct the only canonical key for `protocol_id`.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1) -> Self {
        Self { protocol_id }
    }

    /// Return the protocol selected by this key.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }
}

/// Deterministic failure while planning scheduled activation promotion.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum PrivacyActivationPromotionErrorV1 {
    /// The typed storage key and activation payload identify different protocols.
    #[error(transparent)]
    KeyProtocolMismatch(Box<PrivacyActivationKeyProtocolMismatchV1>),
    /// A persisted activation record is structurally invalid.
    #[error(transparent)]
    InvalidActivation(Box<PrivacyInvalidActivationV1>),
    /// A structurally valid record does not match executable consensus code.
    #[error(transparent)]
    CompiledProfile(Box<PrivacyActivationCompiledProfileMismatchV1>),
    /// A scheduled protocol-limit transition was not applied at its exact height.
    #[error(transparent)]
    MissedProtocolLimits(Box<PrivacyActivationMissedProtocolLimitsV1>),
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error(
    "privacy activation key protocol {key_protocol:?} differs from record protocol {record_protocol:?}"
)]
pub(crate) struct PrivacyActivationKeyProtocolMismatchV1 {
    key_protocol: PrivacyProtocolIdV1,
    record_protocol: PrivacyProtocolIdV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("persisted privacy activation {protocol_id:?} is invalid: {source}")]
pub(crate) struct PrivacyInvalidActivationV1 {
    protocol_id: PrivacyProtocolIdV1,
    source: PrivacyActivationValidationError,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("persisted privacy activation {protocol_id:?} is not compiled: {source}")]
pub(crate) struct PrivacyActivationCompiledProfileMismatchV1 {
    protocol_id: PrivacyProtocolIdV1,
    source: crate::privacy_profiles::CompiledPrivacyProfileValidationErrorV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error(
    "privacy activation {protocol_id:?} missed protocol-limit effective height {effective_at_height} before incoming height {incoming_height}"
)]
pub(crate) struct PrivacyActivationMissedProtocolLimitsV1 {
    protocol_id: PrivacyProtocolIdV1,
    effective_at_height: u64,
    incoming_height: u64,
}

/// Prevalidate and plan every scheduled activation promotion due at `current_height`.
///
/// This function is deliberately read-only. The block-start hook applies the
/// returned ordered update set only after every persisted activation has been
/// validated, so one malformed record cannot cause a partially promoted
/// registry. A height jump promotes a due proposal with its original scheduled
/// height, and a subsequent restart produces an empty plan.
///
/// # Errors
///
/// Rejects any key/record mismatch, malformed activation, or non-canonical
/// chain-wide limits before returning an update.
pub(crate) fn plan_due_privacy_activation_promotions_v1(
    activations: &impl StorageReadOnly<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
    incoming_height: u64,
) -> Result<
    Vec<(PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1)>,
    PrivacyActivationPromotionErrorV1,
> {
    let mut promotions = Vec::new();
    for (key, record) in activations.iter() {
        if key.protocol_id() != record.protocol_id {
            return Err(PrivacyActivationPromotionErrorV1::KeyProtocolMismatch(
                Box::new(PrivacyActivationKeyProtocolMismatchV1 {
                    key_protocol: key.protocol_id(),
                    record_protocol: record.protocol_id,
                }),
            ));
        }
        record.validate().map_err(|source| {
            PrivacyActivationPromotionErrorV1::InvalidActivation(Box::new(
                PrivacyInvalidActivationV1 {
                    protocol_id: record.protocol_id,
                    source,
                },
            ))
        })?;
        crate::privacy_profiles::validate_compiled_privacy_activation_v1(record).map_err(
            |source| {
                PrivacyActivationPromotionErrorV1::CompiledProfile(Box::new(
                    PrivacyActivationCompiledProfileMismatchV1 {
                        protocol_id: record.protocol_id,
                        source,
                    },
                ))
            },
        )?;

        let mut promoted = *record;
        let lifecycle =
            crate::privacy::effective_privacy_lifecycle_v1(record.lifecycle, incoming_height);
        promoted.lifecycle = lifecycle;
        if let Some(pending) = record.pending_protocol_limits_tightening {
            if pending.effective_at_height < incoming_height {
                return Err(PrivacyActivationPromotionErrorV1::MissedProtocolLimits(
                    Box::new(PrivacyActivationMissedProtocolLimitsV1 {
                        protocol_id: record.protocol_id,
                        effective_at_height: pending.effective_at_height,
                        incoming_height,
                    }),
                ));
            }
            if pending.effective_at_height == incoming_height {
                promoted.protocol_limits = pending.next_limits;
                promoted.pending_protocol_limits_tightening = None;
            }
        }
        if promoted != *record {
            promoted.validate().map_err(|source| {
                PrivacyActivationPromotionErrorV1::InvalidActivation(Box::new(
                    PrivacyInvalidActivationV1 {
                        protocol_id: promoted.protocol_id,
                        source,
                    },
                ))
            })?;
            crate::privacy_profiles::validate_compiled_privacy_activation_v1(&promoted).map_err(
                |source| {
                    PrivacyActivationPromotionErrorV1::CompiledProfile(Box::new(
                        PrivacyActivationCompiledProfileMismatchV1 {
                            protocol_id: promoted.protocol_id,
                            source,
                        },
                    ))
                },
            )?;
            promotions.push((*key, promoted));
        }
    }
    Ok(promotions)
}

/// Validate every restored activation against its exact committed height.
///
/// A proposed activation or protocol-limit transition effective at `E` is
/// valid in a snapshot committed at `E - 1` and invalid once committed height
/// `E` has already been reached. No lifecycle may claim a transition height
/// after the snapshot's committed height.
pub(crate) fn validate_privacy_activations_at_committed_height_v1(
    activations: &impl StorageReadOnly<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
    committed_height: u64,
) -> Result<(), String> {
    for (key, record) in activations.iter() {
        if key.protocol_id() != record.protocol_id {
            return Err(format!(
                "privacy activation key protocol {:?} differs from record protocol {:?}",
                key.protocol_id(),
                record.protocol_id
            ));
        }
        record.validate().map_err(|error| {
            format!(
                "persisted privacy activation {:?} is invalid: {error}",
                record.protocol_id
            )
        })?;
        crate::privacy_profiles::validate_compiled_privacy_activation_v1(record).map_err(
            |error| {
                format!(
                    "persisted privacy activation {:?} is not compiled: {error}",
                    record.protocol_id
                )
            },
        )?;
        if let Some(pending) = record.pending_protocol_limits_tightening {
            if pending.scheduled_at_height > committed_height {
                return Err(format!(
                    "privacy activation {:?} protocol-limit scheduled-at height {} is after committed height {committed_height}",
                    record.protocol_id, pending.scheduled_at_height
                ));
            }
            if pending.effective_at_height <= committed_height {
                return Err(format!(
                    "privacy activation {:?} protocol-limit effective height {} is not after committed height {committed_height}",
                    record.protocol_id, pending.effective_at_height
                ));
            }
        }
        let (proposed_at_height, activated_at_height, state_since_height) = match record.lifecycle {
            PrivacyProtocolLifecycleV1::Proposed(state) => {
                if state.activate_at_height <= committed_height {
                    return Err(format!(
                        "privacy activation {:?} remains proposed at due height {} in snapshot committed at height {committed_height}",
                        record.protocol_id, state.activate_at_height
                    ));
                }
                (state.proposed_at_height, None, None)
            }
            PrivacyProtocolLifecycleV1::Active(state) => (
                state.proposed_at_height,
                Some(state.activated_at_height),
                Some(state.state_since_height),
            ),
            PrivacyProtocolLifecycleV1::Suspended(state) => (
                state.proposed_at_height,
                Some(state.activated_at_height),
                Some(state.state_since_height),
            ),
            PrivacyProtocolLifecycleV1::Retired(state) => (
                state.proposed_at_height,
                state.activated_at_height,
                Some(state.state_since_height),
            ),
        };
        if proposed_at_height > committed_height {
            return Err(format!(
                "privacy activation {:?} proposal height {proposed_at_height} is after committed height {committed_height}",
                record.protocol_id
            ));
        }
        if let Some(activated_at_height) = activated_at_height
            && activated_at_height > committed_height
        {
            return Err(format!(
                "privacy activation {:?} activation height {activated_at_height} is after committed height {committed_height}",
                record.protocol_id
            ));
        }
        if let Some(state_since_height) = state_since_height
            && state_since_height > committed_height
        {
            return Err(format!(
                "privacy activation {:?} lifecycle state height {state_since_height} is after committed height {committed_height}",
                record.protocol_id
            ));
        }
    }
    Ok(())
}

/// Exact encrypted-account key in one Anonymous PGC pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) struct PrivacyPgcAccountKeyV1 {
    namespace: PrivacyNamespaceV1,
    public_key: PrivacyP256PointV1,
}

impl PrivacyPgcAccountKeyV1 {
    /// Construct a canonical PGC account-state key.
    ///
    /// # Errors
    ///
    /// Rejects a malformed/non-PGC namespace or an all-zero public key.
    pub(crate) fn new(
        namespace: PrivacyNamespaceV1,
        public_key: PrivacyP256PointV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy PGC account namespace is invalid")?;
        if namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
            return Err("privacy PGC account namespace has the wrong protocol");
        }
        if public_key.is_zero() {
            return Err("privacy PGC account public key must be non-zero");
        }
        Ok(Self {
            namespace,
            public_key,
        })
    }

    /// Return the exact PGC pool namespace.
    #[must_use]
    pub(crate) const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Return the exact compressed account public key.
    #[must_use]
    pub(crate) const fn public_key(self) -> PrivacyP256PointV1 {
        self.public_key
    }

    /// Ordered bounds covering one complete pool account table.
    #[must_use]
    pub(crate) fn pool_range(namespace: PrivacyNamespaceV1) -> core::ops::RangeInclusive<Self> {
        Self {
            namespace,
            public_key: PrivacyP256PointV1::new([0; 33]),
        }..=Self {
            namespace,
            public_key: PrivacyP256PointV1::new([u8::MAX; 33]),
        }
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace, self.public_key).map(|_| ())
    }
}

/// Typed key for the immutable audited invariant of one Anonymous PGC pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) struct PrivacyPgcPoolInvariantKeyV1 {
    namespace: PrivacyNamespaceV1,
}

impl PrivacyPgcPoolInvariantKeyV1 {
    /// Construct the only invariant key for a canonical PGC pool namespace.
    ///
    /// # Errors
    ///
    /// Rejects a malformed namespace or one owned by another protocol.
    pub(crate) fn new(namespace: PrivacyNamespaceV1) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy PGC invariant namespace is invalid")?;
        if namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
            return Err("privacy PGC invariant namespace has the wrong protocol");
        }
        Ok(Self { namespace })
    }

    /// Return the exact PGC pool namespace.
    #[must_use]
    pub(crate) const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace).map(|_| ())
    }
}

/// Immutable supply and audit binding established by one verified PGC bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyPgcPoolInvariantV1 {
    total_supply: u32,
    bootstrap_root: PrivacyRootV1,
    bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
    bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
}

/// Domain-separated commitment copied into every retained PGC successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyPgcPoolInvariantDigestV1([u8; 32]);

impl PrivacyPgcPoolInvariantDigestV1 {
    fn new(bytes: [u8; 32]) -> Result<Self, &'static str> {
        if bytes == [0; 32] {
            return Err("privacy PGC pool invariant digest must be non-zero");
        }
        Ok(Self(bytes))
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.0).map(|_| ())
    }
}

impl PrivacyPgcPoolInvariantV1 {
    /// Construct an exact pool invariant after native bootstrap verification.
    ///
    /// # Errors
    ///
    /// Rejects zero supply or either zero audit digest.
    pub(crate) fn new(
        total_supply: u32,
        bootstrap_root: PrivacyRootV1,
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
    ) -> Result<Self, &'static str> {
        if total_supply == 0 {
            return Err("privacy PGC pool total supply must be non-zero");
        }
        if bootstrap_root.is_zero() {
            return Err("privacy PGC bootstrap root must be non-zero");
        }
        if bootstrap_digest.is_zero() {
            return Err("privacy PGC bootstrap digest must be non-zero");
        }
        if bootstrap_proof_digest.is_zero() {
            return Err("privacy PGC bootstrap proof digest must be non-zero");
        }
        Ok(Self {
            total_supply,
            bootstrap_root,
            bootstrap_digest,
            bootstrap_proof_digest,
        })
    }

    /// Return the exact public aggregate supply.
    #[must_use]
    pub(crate) const fn total_supply(self) -> u32 {
        self.total_supply
    }

    /// Return the exact account-state root admitted at canonical epoch one.
    #[must_use]
    pub(crate) const fn bootstrap_root(self) -> PrivacyRootV1 {
        self.bootstrap_root
    }

    /// Return the exact canonical public bootstrap digest.
    #[must_use]
    pub(crate) const fn bootstrap_digest(self) -> PrivacyPgcAccountBootstrapDigestV1 {
        self.bootstrap_digest
    }

    /// Return the digest of the exact canonical admitted proof bytes.
    #[must_use]
    pub(crate) const fn bootstrap_proof_digest(self) -> PrivacyPgcBootstrapProofDigestV1 {
        self.bootstrap_proof_digest
    }

    /// Commit this immutable invariant to its exact pool namespace.
    pub(crate) fn digest(
        self,
        namespace: PrivacyNamespaceV1,
    ) -> Result<PrivacyPgcPoolInvariantDigestV1, &'static str> {
        let key = PrivacyPgcPoolInvariantKeyV1::new(namespace)?;
        self.validate()?;
        let namespace_bytes = norito::to_bytes(&key.namespace())
            .map_err(|_| "privacy PGC invariant namespace encoding failed")?;
        let namespace_len = u64::try_from(namespace_bytes.len())
            .map_err(|_| "privacy PGC invariant namespace length overflow")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_POOL_INVARIANT_DIGEST_DOMAIN_V1);
        hasher.update(&namespace_len.to_le_bytes());
        hasher.update(&namespace_bytes);
        hasher.update(&self.total_supply.to_le_bytes());
        hasher.update(self.bootstrap_root.as_bytes());
        hasher.update(self.bootstrap_digest.as_bytes());
        hasher.update(self.bootstrap_proof_digest.as_bytes());
        PrivacyPgcPoolInvariantDigestV1::new(*hasher.finalize().as_bytes())
    }

    pub(crate) fn validate(self) -> Result<(), &'static str> {
        Self::new(
            self.total_supply,
            self.bootstrap_root,
            self.bootstrap_digest,
            self.bootstrap_proof_digest,
        )
        .map(|_| ())
    }
}

/// Domain-separated origin of one encrypted PGC account state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(tag = "origin", content = "record")]
pub(crate) enum PrivacyPgcAccountProvenanceV1 {
    /// Initial state admitted by the complete governed pool bootstrap.
    Bootstrap {
        /// Digest of the exact canonical bootstrap payload.
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        /// Digest of the exact canonical native bootstrap proof.
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        /// Block height at which the bootstrap became durable.
        admitted_at_height: u64,
    },
    /// State produced by one exhaustively verified PGC proof.
    VerifiedProof {
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the transition became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
}

impl PrivacyPgcAccountProvenanceV1 {
    pub(crate) fn bootstrap(
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("privacy PGC bootstrap digest must be non-zero");
        }
        if bootstrap_proof_digest.is_zero() {
            return Err("privacy PGC bootstrap proof digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy PGC account admission height must be non-zero");
        }
        Ok(Self::Bootstrap {
            bootstrap_digest,
            bootstrap_proof_digest,
            admitted_at_height,
        })
    }

    pub(crate) fn verified_proof(
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if statement_digest.is_zero() {
            return Err("privacy PGC statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy PGC account admission height must be non-zero");
        }
        Ok(Self::VerifiedProof {
            statement_digest,
            admitted_at_height,
            action_index,
        })
    }

    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Bootstrap {
                bootstrap_digest,
                bootstrap_proof_digest,
                admitted_at_height,
            } => Self::bootstrap(bootstrap_digest, bootstrap_proof_digest, admitted_at_height)
                .map(|_| ()),
            Self::VerifiedProof {
                statement_digest,
                admitted_at_height,
                action_index,
            } => {
                Self::verified_proof(statement_digest, admitted_at_height, action_index).map(|_| ())
            }
        }
    }
}

/// Canonical encrypted balance and epoch for one PGC account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyPgcAccountStateV1 {
    encrypted_balance: PrivacyP256CiphertextV1,
    epoch: u64,
    provenance: PrivacyPgcAccountProvenanceV1,
}

impl PrivacyPgcAccountStateV1 {
    pub(crate) fn new(
        encrypted_balance: PrivacyP256CiphertextV1,
        epoch: u64,
        provenance: PrivacyPgcAccountProvenanceV1,
    ) -> Result<Self, &'static str> {
        if encrypted_balance.left.is_zero() || encrypted_balance.right.is_zero() {
            return Err("privacy PGC encrypted balance points must be non-zero");
        }
        if epoch == 0 {
            return Err("privacy PGC account epoch must be non-zero");
        }
        provenance.validate()?;
        Ok(Self {
            encrypted_balance,
            epoch,
            provenance,
        })
    }

    #[must_use]
    pub(crate) const fn encrypted_balance(self) -> PrivacyP256CiphertextV1 {
        self.encrypted_balance
    }

    #[must_use]
    pub(crate) const fn epoch(self) -> u64 {
        self.epoch
    }

    pub(crate) fn validate(self) -> Result<(), &'static str> {
        Self::new(self.encrypted_balance, self.epoch, self.provenance).map(|_| ())
    }
}

/// Fully validated, transaction-local view of one existing Anonymous PGC pool.
///
/// The owned account table and retained history freeze every trusted input
/// across native verification. Runtime admission never asks the verifier to
/// interpret partially related world maps or caller-provided current state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyPgcPoolSnapshotV1 {
    namespace: PrivacyNamespaceV1,
    invariant: PrivacyPgcPoolInvariantV1,
    accounts: Vec<PrivacyPgcAccountV1>,
    current_epoch: u64,
    current_root: PrivacyRootV1,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    retained_roots: Vec<(PrivacyRootKeyV1, PrivacyRootProvenanceV1)>,
}

impl PrivacyPgcPoolSnapshotV1 {
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn invariant(&self) -> PrivacyPgcPoolInvariantV1 {
        self.invariant
    }

    #[must_use]
    pub(crate) fn accounts(&self) -> &[PrivacyPgcAccountV1] {
        &self.accounts
    }

    #[must_use]
    pub(crate) const fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    #[must_use]
    pub(crate) const fn current_root(&self) -> PrivacyRootV1 {
        self.current_root
    }

    #[must_use]
    pub(crate) const fn retention_anchor(&self) -> Option<PrivacyRootRetentionAnchorV1> {
        self.retention_anchor
    }

    /// Return trusted retained membership for the exact current head.
    #[must_use]
    pub(crate) fn retained_current_root(&self) -> Option<(u64, PrivacyRootV1)> {
        self.retained_roots
            .iter()
            .any(|(key, _)| key.epoch() == self.current_epoch && key.root() == self.current_root)
            .then_some((self.current_epoch, self.current_root))
    }
}

fn validate_pgc_successor_link_v1(
    namespace: PrivacyNamespaceV1,
    key: PrivacyRootKeyV1,
    provenance: PrivacyRootProvenanceV1,
    invariant_digest: PrivacyPgcPoolInvariantDigestV1,
) -> Result<(u64, PrivacyRootV1), String> {
    let PrivacyRootProvenanceV1::VerifiedPgcSuccessor {
        parent_epoch,
        parent_root,
        pool_invariant_digest,
        ..
    } = provenance
    else {
        return Err(format!(
            "privacy PGC root history {namespace:?} contains a non-PGC successor"
        ));
    };
    if pool_invariant_digest != invariant_digest {
        return Err(format!(
            "privacy PGC successor {} is bound to a different immutable pool invariant",
            key.epoch()
        ));
    }
    if parent_epoch.checked_add(1) != Some(key.epoch()) {
        return Err(format!(
            "privacy PGC successor {} does not advance parent epoch {parent_epoch} by exactly one",
            key.epoch()
        ));
    }
    Ok((parent_epoch, parent_root))
}

/// Validate an independently retained, newest-first-pruned PGC root window.
///
/// The first retained item is either the canonical epoch-one bootstrap or a
/// successor whose missing parent is the immediately preceding pruned prefix.
/// Once the prefix boundary is crossed, every retained item must link exactly
/// to its preceding `(epoch, root)` with no gap.
fn validate_pgc_retained_root_chain_v1(
    namespace: PrivacyNamespaceV1,
    invariant: PrivacyPgcPoolInvariantV1,
    retained_root_count: usize,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    history: &[(PrivacyRootKeyV1, PrivacyRootProvenanceV1)],
) -> Result<(), String> {
    if retained_root_count == 0 {
        return Err("privacy PGC retained-root count must be non-zero".to_owned());
    }
    if history.is_empty() {
        return Err("privacy PGC pool has no retained root history".to_owned());
    }
    if history.len() > retained_root_count {
        return Err(format!(
            "privacy PGC root history exceeds retention {retained_root_count}"
        ));
    }
    let invariant_digest = invariant
        .digest(namespace)
        .map_err(|error| format!("invalid privacy PGC pool invariant digest: {error}"))?;
    let (first_key, first_provenance) = history[0];
    match first_provenance {
        PrivacyRootProvenanceV1::VerifiedBootstrap {
            bootstrap_digest,
            bootstrap_proof_digest,
            ..
        } => {
            if retention_anchor.is_some() {
                return Err(
                    "privacy PGC retained bootstrap history has an unexpected pruned-prefix anchor"
                        .to_owned(),
                );
            }
            if first_key.epoch() != PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 {
                return Err(format!(
                    "privacy PGC bootstrap root epoch {} is not canonical epoch {}",
                    first_key.epoch(),
                    PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
                ));
            }
            if first_key.root() != invariant.bootstrap_root()
                || bootstrap_digest != invariant.bootstrap_digest()
                || bootstrap_proof_digest != invariant.bootstrap_proof_digest()
            {
                return Err(
                    "privacy PGC bootstrap root provenance differs from its immutable invariant"
                        .to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::VerifiedPgcSuccessor { .. } => {
            let (parent_epoch, parent_root) = validate_pgc_successor_link_v1(
                namespace,
                first_key,
                first_provenance,
                invariant_digest,
            )?;
            if first_key.epoch() <= PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 {
                return Err(
                    "privacy PGC pruned prefix begins at or before the canonical bootstrap epoch"
                        .to_owned(),
                );
            }
            let anchor = retention_anchor.ok_or_else(|| {
                "privacy PGC pruned-prefix history has no exact retention anchor".to_owned()
            })?;
            if anchor.epoch().checked_add(1) != Some(first_key.epoch())
                || parent_epoch != anchor.epoch()
                || parent_root != anchor.root()
            {
                return Err(
                    "privacy PGC first retained successor does not consume its exact pruned-prefix anchor"
                        .to_owned(),
                );
            }
            if anchor.epoch() == PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
                && anchor.root() != invariant.bootstrap_root()
            {
                return Err(
                    "privacy PGC pruned bootstrap anchor differs from its immutable invariant"
                        .to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::Governance { .. }
        | PrivacyRootProvenanceV1::ZkX509CaGovernance { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. }
        | PrivacyRootProvenanceV1::OrchardPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::OrchardPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::VerifiedProof { .. } => {
            return Err("privacy PGC retained history begins with invalid provenance".to_owned());
        }
    }

    for adjacent in history.windows(2) {
        let (parent_key, _) = adjacent[0];
        let (child_key, child_provenance) = adjacent[1];
        let (declared_parent_epoch, declared_parent_root) = validate_pgc_successor_link_v1(
            namespace,
            child_key,
            child_provenance,
            invariant_digest,
        )?;
        if parent_key.epoch().checked_add(1) != Some(child_key.epoch())
            || declared_parent_epoch != parent_key.epoch()
            || declared_parent_root != parent_key.root()
        {
            return Err(format!(
                "privacy PGC retained history has a gap or forged parent between epochs {} and {}",
                parent_key.epoch(),
                child_key.epoch()
            ));
        }
    }
    if retention_anchor.is_some() && history.len() != retained_root_count {
        return Err(format!(
            "privacy PGC anchored history has {} roots but must fill retention {retained_root_count}",
            history.len()
        ));
    }
    Ok(())
}

/// Load and validate every persisted component of one Anonymous PGC pool.
///
/// Iteration is bounded by the closed 64-account profile and governed retained
/// root limit. The returned snapshot owns the exact table and history so a
/// native verifier receives one coherent current state.
///
/// # Errors
///
/// Rejects missing/orphaned components, over-cap or malformed history,
/// incorrect provenance, a mixed account table, or any root mismatch.
pub(crate) fn load_privacy_pgc_pool_snapshot_v1(
    namespace: PrivacyNamespaceV1,
    retained_root_count: u32,
    pgc_accounts: &impl StorageReadOnly<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>,
    pgc_pool_invariants: &impl StorageReadOnly<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyPgcPoolSnapshotV1, String> {
    namespace
        .validate()
        .map_err(|error| format!("invalid privacy PGC pool namespace: {error}"))?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
        return Err("privacy PGC pool snapshot has the wrong protocol".to_owned());
    }
    if retained_root_count == 0 {
        return Err("privacy PGC retained-root count must be non-zero".to_owned());
    }
    let retained_root_count = usize::try_from(retained_root_count)
        .map_err(|_| "privacy PGC retained-root count cannot be represented".to_owned())?;
    let max_accounts = ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1
        .iter()
        .copied()
        .max()
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| "privacy PGC account-count profile is invalid".to_owned())?;

    let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(namespace)
        .map_err(|error| format!("invalid privacy PGC invariant key: {error}"))?;
    let invariant = pgc_pool_invariants
        .get(&invariant_key)
        .copied()
        .ok_or_else(|| "privacy PGC pool has no immutable invariant".to_owned())?;
    invariant
        .validate()
        .map_err(|error| format!("invalid privacy PGC invariant: {error}"))?;

    let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::PgcAccountState)
        .map_err(|error| format!("invalid privacy PGC root-head key: {error}"))?;
    let head = root_heads
        .get(&head_key)
        .copied()
        .ok_or_else(|| "privacy PGC pool has no current root head".to_owned())?;
    head.validate()
        .map_err(|error| format!("invalid privacy PGC root head: {error}"))?;

    let mut retained_roots = Vec::new();
    for (key, provenance) in roots.range(PrivacyRootKeyV1::history_range(
        namespace,
        PrivacyRootRoleV1::PgcAccountState,
    )) {
        if retained_roots.len() == retained_root_count {
            return Err(format!(
                "privacy PGC root history exceeds retention {retained_root_count}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid privacy PGC root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid privacy PGC root provenance: {error}"))?;
        if retained_roots.last().is_some_and(
            |(previous, _): &(PrivacyRootKeyV1, PrivacyRootProvenanceV1)| {
                previous.epoch() == key.epoch()
            },
        ) {
            return Err(format!(
                "privacy PGC root history contains duplicate epoch {}",
                key.epoch()
            ));
        }
        retained_roots.push((*key, *provenance));
    }
    validate_pgc_retained_root_chain_v1(
        namespace,
        invariant,
        retained_root_count,
        head.retention_anchor(),
        &retained_roots,
    )?;
    let latest = retained_roots
        .last()
        .expect("non-empty history checked above");
    if head.epoch() != latest.0.epoch()
        || head.root() != latest.0.root()
        || head.provenance() != latest.1
    {
        return Err("privacy PGC root head does not equal latest retained history".to_owned());
    }

    let mut accounts = Vec::new();
    let mut account_epoch = None;
    let mut account_provenance = None;
    for (key, state) in pgc_accounts.range(PrivacyPgcAccountKeyV1::pool_range(namespace)) {
        if accounts.len() == max_accounts {
            return Err(format!(
                "privacy PGC account table exceeds closed maximum {max_accounts}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid privacy PGC account key: {error}"))?;
        state
            .validate()
            .map_err(|error| format!("invalid privacy PGC account state: {error}"))?;
        if account_epoch
            .replace(state.epoch())
            .is_some_and(|epoch| epoch != state.epoch())
        {
            return Err("privacy PGC account table contains mixed epochs".to_owned());
        }
        if account_provenance
            .replace(state.provenance)
            .is_some_and(|provenance| provenance != state.provenance)
        {
            return Err("privacy PGC account table contains mixed provenance".to_owned());
        }
        accounts.push(PrivacyPgcAccountV1 {
            public_key: key.public_key(),
            encrypted_balance: state.encrypted_balance(),
        });
    }
    let epoch = account_epoch
        .ok_or_else(|| "privacy PGC pool has no encrypted account table".to_owned())?;
    let account_provenance = account_provenance
        .ok_or_else(|| "privacy PGC account table has no provenance".to_owned())?;
    if epoch != head.epoch() {
        return Err("privacy PGC account epoch differs from its root head".to_owned());
    }
    let provenance_matches = match (account_provenance, head.provenance()) {
        (
            PrivacyPgcAccountProvenanceV1::Bootstrap {
                bootstrap_digest: account_bootstrap_digest,
                bootstrap_proof_digest: account_proof_digest,
                admitted_at_height: account_height,
            },
            PrivacyRootProvenanceV1::VerifiedBootstrap {
                bootstrap_digest: root_bootstrap_digest,
                bootstrap_proof_digest: root_proof_digest,
                admitted_at_height: root_height,
            },
        ) => {
            account_bootstrap_digest == root_bootstrap_digest
                && account_proof_digest == root_proof_digest
                && account_height == root_height
                && account_bootstrap_digest == invariant.bootstrap_digest()
                && account_proof_digest == invariant.bootstrap_proof_digest()
        }
        (
            PrivacyPgcAccountProvenanceV1::VerifiedProof {
                statement_digest: account_statement_digest,
                admitted_at_height: account_height,
                action_index: account_action_index,
            },
            PrivacyRootProvenanceV1::VerifiedPgcSuccessor {
                statement_digest: root_statement_digest,
                admitted_at_height: root_height,
                action_index: root_action_index,
                ..
            },
        ) => {
            account_statement_digest == root_statement_digest
                && account_height == root_height
                && account_action_index == root_action_index
        }
        _ => false,
    };
    if !provenance_matches {
        return Err("privacy PGC account provenance differs from its root head".to_owned());
    }
    let computed = compute_privacy_pgc_account_state_root_v1(
        namespace,
        epoch,
        invariant.total_supply(),
        &accounts,
    )
    .map_err(|error| format!("invalid privacy PGC account table: {error}"))?;
    if computed != head.root() {
        return Err("privacy PGC account table does not match its root head".to_owned());
    }

    Ok(PrivacyPgcPoolSnapshotV1 {
        namespace,
        invariant,
        accounts,
        current_epoch: epoch,
        current_root: computed,
        retention_anchor: head.retention_anchor(),
        retained_roots,
    })
}

/// Validate and count all authoritative ZK-ACE policy revisions.
///
/// The global policy count is checked before lookup so adversarial restored
/// state cannot turn proof preflight into an unbounded scan.
pub(crate) fn privacy_zk_ace_policy_count_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<usize, String> {
    let mut policy_count = 0usize;
    for (candidate, record) in commitments.range(PrivacyCommitmentKeyV1::zk_ace_policy_range()) {
        let PrivacyCommitmentKeyV1::ZkAcePolicy {
            policy_id: candidate_policy_id,
        } = candidate
        else {
            return Err("ZK-ACE policy range crossed a typed key boundary".to_owned());
        };
        policy_count = policy_count
            .checked_add(1)
            .ok_or_else(|| "ZK-ACE policy count overflow".to_owned())?;
        if policy_count > PRIVACY_ZK_ACE_MAX_POLICIES_V1 {
            return Err(format!(
                "ZK-ACE policy count exceeds {}",
                PRIVACY_ZK_ACE_MAX_POLICIES_V1
            ));
        }
        let PrivacyStateItemRecordV1::ZkAcePolicyGovernance {
            policy,
            admitted_at_height,
        } = record
        else {
            return Err(format!(
                "ZK-ACE policy {candidate_policy_id:?} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "ZK-ACE policy {candidate_policy_id:?} has zero admission height"
            ));
        }
        policy.validate().map_err(|error| {
            format!("ZK-ACE policy {candidate_policy_id:?} is invalid: {error}")
        })?;
        if policy.policy_id != *candidate_policy_id {
            return Err(format!(
                "ZK-ACE policy key {candidate_policy_id:?} does not match its record"
            ));
        }
    }
    Ok(policy_count)
}

/// Load and validate one authoritative ZK-ACE policy revision.
pub(crate) fn load_privacy_zk_ace_policy_v1(
    policy_id: PrivacyPolicyIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyZkAcePolicyRecordV1, String> {
    let key = PrivacyCommitmentKeyV1::zk_ace_policy(policy_id)
        .map_err(|error| format!("invalid ZK-ACE policy lookup key: {error}"))?;
    privacy_zk_ace_policy_count_v1(commitments)?;
    let record = commitments
        .get(&key)
        .ok_or_else(|| format!("ZK-ACE policy {policy_id:?} is not registered"))?;
    let PrivacyStateItemRecordV1::ZkAcePolicyGovernance { policy, .. } = record else {
        return Err(format!(
            "ZK-ACE policy {policy_id:?} has wrong-role provenance"
        ));
    };
    Ok(policy.clone())
}

/// Validate and count all current authoritative Bootle/Lantern issuer policies.
///
/// The global bound is enforced before lookup so proof preflight cannot be
/// forced to accept adversarially oversized restored governance state.
pub(crate) fn privacy_bootle_lantern_issuer_policy_count_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<usize, String> {
    let mut policy_count = 0usize;
    for (candidate, state_record) in
        commitments.range(PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy_range())
    {
        candidate
            .validate()
            .map_err(|error| format!("invalid Bootle/Lantern issuer-policy key: {error}"))?;
        let PrivacyCommitmentKeyV1::BootleLanternIssuerPolicy {
            issuer_id,
            policy_id,
        } = *candidate
        else {
            return Err(
                "Bootle/Lantern issuer-policy range crossed a typed key boundary".to_owned(),
            );
        };
        policy_count = policy_count
            .checked_add(1)
            .ok_or_else(|| "Bootle/Lantern issuer-policy count overflow".to_owned())?;
        if policy_count > BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1 {
            return Err(format!(
                "Bootle/Lantern issuer-policy count exceeds {}",
                BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1
            ));
        }
        let PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance {
            policy,
            admitted_at_height,
        } = state_record
        else {
            return Err(format!(
                "Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} has zero admission height"
            ));
        }
        policy.validate().map_err(|error| {
            format!("Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} is invalid: {error}")
        })?;
        if policy.issuer_id != issuer_id || policy.policy_id != policy_id {
            return Err(format!(
                "Bootle/Lantern issuer-policy key {issuer_id:?}/{policy_id:?} does not match its record"
            ));
        }
    }
    Ok(policy_count)
}

/// Load and validate one current authoritative Bootle/Lantern issuer policy.
pub(crate) fn load_privacy_bootle_lantern_issuer_policy_v1(
    issuer_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<BootleLanternIssuerPolicyV1, String> {
    let key = PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(issuer_id, policy_id)
        .map_err(|error| format!("invalid Bootle/Lantern issuer-policy lookup key: {error}"))?;
    let state_record = commitments.get(&key).ok_or_else(|| {
        format!("Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} is not registered")
    })?;
    state_record.validate().map_err(|error| {
        format!("Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} is invalid: {error}")
    })?;
    let PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance { policy, .. } = state_record
    else {
        return Err(format!(
            "Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} has wrong-role provenance"
        ));
    };
    if policy.issuer_id != issuer_id || policy.policy_id != policy_id {
        return Err(format!(
            "Bootle/Lantern issuer-policy key {issuer_id:?}/{policy_id:?} does not match its record"
        ));
    }
    Ok(policy.clone())
}

#[derive(Default)]
struct PrivacyVegaIssuerGovernanceIndexV1 {
    lineages: BTreeMap<PrivacyIssuerIdV1, Vec<PrivacyVegaIssuerRecordV1>>,
    key_owners: BTreeMap<PrivacyP256PointV1, PrivacyIssuerIdV1>,
    record_count: usize,
}

/// Bounded facts derived while validating the complete Vega issuer registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyVegaIssuerRegistryFactsV1 {
    record_count: usize,
    key_owner: Option<PrivacyIssuerIdV1>,
}

impl PrivacyVegaIssuerRegistryFactsV1 {
    /// Exact number of persisted Vega issuer revisions.
    pub(crate) const fn record_count(self) -> usize {
        self.record_count
    }

    /// Permanent lineage owner of the candidate issuer key, if registered.
    pub(crate) const fn key_owner(self) -> Option<PrivacyIssuerIdV1> {
        self.key_owner
    }
}

fn validate_privacy_vega_issuer_lineage_v1(
    issuer_id: PrivacyIssuerIdV1,
    records: &[PrivacyVegaIssuerRecordV1],
) -> Result<(), String> {
    if records.is_empty() {
        return Err(format!("Vega issuer lineage {issuer_id:?} is empty"));
    }
    if records.len() > VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1 {
        return Err(format!(
            "Vega issuer lineage {issuer_id:?} exceeds {} revisions",
            VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1
        ));
    }
    records[0].validate_initial().map_err(|error| {
        format!("Vega issuer lineage {issuer_id:?} has invalid origin: {error}")
    })?;
    let mut seen_keys = BTreeSet::from([records[0].issuer_public_key]);
    for pair in records.windows(2) {
        let result = match pair[1].lifecycle {
            PrivacyVegaIssuerRecordLifecycleV1::Active => {
                validate_vega_issuer_rotation_v1(&pair[0], &pair[1])
            }
            PrivacyVegaIssuerRecordLifecycleV1::Revoked => {
                validate_vega_issuer_revocation_v1(&pair[0], &pair[1])
            }
        };
        result.map_err(|error| format!("Vega issuer lineage {issuer_id:?} is invalid: {error}"))?;
        if pair[1].issuer_public_key != pair[0].issuer_public_key
            && !seen_keys.insert(pair[1].issuer_public_key)
        {
            return Err(format!(
                "Vega issuer lineage {issuer_id:?} reactivates a retired P-256 key"
            ));
        }
    }
    Ok(())
}

fn load_privacy_vega_issuer_governance_index_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyVegaIssuerGovernanceIndexV1, String> {
    let mut index = PrivacyVegaIssuerGovernanceIndexV1::default();
    for (key, state_record) in
        commitments.range(PrivacyCommitmentKeyV1::vega_issuer_revision_range())
    {
        key.validate()
            .map_err(|error| format!("invalid Vega issuer revision key: {error}"))?;
        let PrivacyCommitmentKeyV1::VegaIssuerRevision {
            issuer_id,
            record_epoch,
        } = *key
        else {
            return Err("Vega issuer range crossed a typed key boundary".to_owned());
        };
        index.record_count = index
            .record_count
            .checked_add(1)
            .ok_or_else(|| "Vega issuer revision count overflow".to_owned())?;
        if index.record_count > VEGA_MAX_ISSUER_RECORDS_V1 {
            return Err(format!(
                "Vega issuer revision count exceeds {}",
                VEGA_MAX_ISSUER_RECORDS_V1
            ));
        }
        let PrivacyStateItemRecordV1::VegaIssuerGovernance {
            record,
            admitted_at_height,
        } = state_record
        else {
            return Err(format!(
                "Vega issuer revision {issuer_id:?}/{record_epoch} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "Vega issuer revision {issuer_id:?}/{record_epoch} has zero admission height"
            ));
        }
        record.validate().map_err(|error| {
            format!("Vega issuer revision {issuer_id:?}/{record_epoch} is invalid: {error}")
        })?;
        crate::privacy_engines::p256::CompressedPointV1::from_slice(
            record.issuer_public_key.as_bytes(),
        )
        .map_err(|error| {
            format!(
                "Vega issuer revision {issuer_id:?}/{record_epoch} has an invalid P-256 key: {error}"
            )
        })?;
        if record.issuer_id != issuer_id || record.record_epoch != record_epoch {
            return Err(format!(
                "Vega issuer revision key {issuer_id:?}/{record_epoch} differs from its record"
            ));
        }
        if let Some(owner) = index.key_owners.insert(record.issuer_public_key, issuer_id)
            && owner != issuer_id
        {
            return Err(format!(
                "Vega issuer public key is assigned to multiple lineages: {owner:?} and {issuer_id:?}"
            ));
        }
        index.lineages.entry(issuer_id).or_default().push(*record);
    }
    for (issuer_id, records) in &index.lineages {
        validate_privacy_vega_issuer_lineage_v1(*issuer_id, records)?;
    }
    Ok(index)
}

/// Validate all Vega issuer lineages and return their exact global revision count.
pub(crate) fn privacy_vega_issuer_record_count_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<usize, String> {
    Ok(load_privacy_vega_issuer_governance_index_v1(commitments)?.record_count)
}

/// Validate the complete registry and return its size plus the permanent owner
/// of `issuer_public_key`, if that key has appeared in any lineage revision.
pub(crate) fn privacy_vega_issuer_registry_facts_v1(
    issuer_public_key: PrivacyP256PointV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyVegaIssuerRegistryFactsV1, String> {
    let index = load_privacy_vega_issuer_governance_index_v1(commitments)?;
    Ok(PrivacyVegaIssuerRegistryFactsV1 {
        record_count: index.record_count,
        key_owner: index.key_owners.get(&issuer_public_key).copied(),
    })
}

/// Load the current revision of one validated Vega issuer lineage.
pub(crate) fn load_privacy_vega_issuer_v1(
    issuer_id: PrivacyIssuerIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyVegaIssuerRecordV1, String> {
    if issuer_id.is_zero() {
        return Err("Vega issuer lookup id must be non-zero".to_owned());
    }
    let index = load_privacy_vega_issuer_governance_index_v1(commitments)?;
    index
        .lineages
        .get(&issuer_id)
        .and_then(|records| records.last())
        .copied()
        .ok_or_else(|| format!("Vega issuer {issuer_id:?} is not registered"))
}

#[derive(Default)]
struct PrivacyZkX509GovernanceIndexV1 {
    trust_anchors: BTreeMap<PrivacyIssuerIdV1, Vec<PrivacyZkX509TrustAnchorRecordV1>>,
    certificate_policies: BTreeMap<
        (PrivacyIssuerIdV1, PrivacyPolicyIdV1),
        Vec<PrivacyZkX509CertificatePolicyRecordV1>,
    >,
    current_crls: BTreeMap<(PrivacyIssuerIdV1, PrivacyPolicyIdV1), PrivacyZkX509CrlRecordV1>,
    trust_anchor_record_count: usize,
    certificate_policy_record_count: usize,
    crl_lineage_count: usize,
}

fn validate_zk_x509_trust_anchor_lineage_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    records: &[PrivacyZkX509TrustAnchorRecordV1],
) -> Result<(), String> {
    if records.is_empty() {
        return Err(format!(
            "X.509 trust-anchor lineage {trust_anchor_id:?} is empty"
        ));
    }
    if records.len() > ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1 {
        return Err(format!(
            "X.509 trust-anchor lineage {trust_anchor_id:?} exceeds {} revisions",
            ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1
        ));
    }
    records[0].validate_initial().map_err(|error| {
        format!("X.509 trust-anchor lineage {trust_anchor_id:?} has invalid origin: {error}")
    })?;
    for pair in records.windows(2) {
        let result = match pair[1].lifecycle {
            PrivacyZkX509RecordLifecycleV1::Active => {
                validate_zk_x509_trust_anchor_rotation_v1(&pair[0], &pair[1])
            }
            PrivacyZkX509RecordLifecycleV1::Revoked => {
                validate_zk_x509_trust_anchor_revocation_v1(&pair[0], &pair[1])
            }
        };
        result.map_err(|error| {
            format!("X.509 trust-anchor lineage {trust_anchor_id:?} is invalid: {error}")
        })?;
    }
    Ok(())
}

fn validate_zk_x509_certificate_policy_lineage_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    records: &[PrivacyZkX509CertificatePolicyRecordV1],
) -> Result<(), String> {
    if records.is_empty() {
        return Err(format!(
            "X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} is empty"
        ));
    }
    if records.len() > ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1 {
        return Err(format!(
            "X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} exceeds {} revisions",
            ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1
        ));
    }
    records[0].validate_initial().map_err(|error| {
        format!(
            "X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} has invalid origin: {error}"
        )
    })?;
    for pair in records.windows(2) {
        let result = match pair[1].lifecycle {
            PrivacyZkX509RecordLifecycleV1::Active => {
                validate_zk_x509_certificate_policy_rotation_v1(&pair[0], &pair[1])
            }
            PrivacyZkX509RecordLifecycleV1::Revoked => {
                validate_zk_x509_certificate_policy_revocation_v1(&pair[0], &pair[1])
            }
        };
        result.map_err(|error| {
            format!(
                "X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} is invalid: {error}"
            )
        })?;
    }
    Ok(())
}

fn load_privacy_zk_x509_governance_index_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyZkX509GovernanceIndexV1, String> {
    let mut index = PrivacyZkX509GovernanceIndexV1::default();
    for (key, state_record) in
        commitments.range(PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision_range())
    {
        key.validate()
            .map_err(|error| format!("invalid X.509 trust-anchor revision key: {error}"))?;
        let PrivacyCommitmentKeyV1::ZkX509TrustAnchorRevision {
            trust_anchor_id,
            record_epoch,
        } = *key
        else {
            return Err("X.509 trust-anchor range crossed a typed key boundary".to_owned());
        };
        index.trust_anchor_record_count = index
            .trust_anchor_record_count
            .checked_add(1)
            .ok_or_else(|| "X.509 trust-anchor revision count overflow".to_owned())?;
        if index.trust_anchor_record_count > ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1 {
            return Err(format!(
                "X.509 trust-anchor revision count exceeds {}",
                ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1
            ));
        }
        let PrivacyStateItemRecordV1::ZkX509TrustAnchorGovernance {
            record,
            admitted_at_height,
        } = state_record
        else {
            return Err(format!(
                "X.509 trust-anchor revision {trust_anchor_id:?}/{record_epoch} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "X.509 trust-anchor revision {trust_anchor_id:?}/{record_epoch} has zero admission height"
            ));
        }
        record.validate().map_err(|error| {
            format!(
                "X.509 trust-anchor revision {trust_anchor_id:?}/{record_epoch} is invalid: {error}"
            )
        })?;
        if record.trust_anchor_id != trust_anchor_id || record.record_epoch != record_epoch {
            return Err(format!(
                "X.509 trust-anchor revision key {trust_anchor_id:?}/{record_epoch} differs from its record"
            ));
        }
        index
            .trust_anchors
            .entry(trust_anchor_id)
            .or_default()
            .push(*record);
    }

    for (key, state_record) in
        commitments.range(PrivacyCommitmentKeyV1::zk_x509_certificate_policy_revision_range())
    {
        key.validate()
            .map_err(|error| format!("invalid X.509 certificate-policy revision key: {error}"))?;
        let PrivacyCommitmentKeyV1::ZkX509CertificatePolicyRevision {
            trust_anchor_id,
            policy_id,
            record_epoch,
        } = *key
        else {
            return Err("X.509 certificate-policy range crossed a typed key boundary".to_owned());
        };
        index.certificate_policy_record_count = index
            .certificate_policy_record_count
            .checked_add(1)
            .ok_or_else(|| "X.509 certificate-policy revision count overflow".to_owned())?;
        if index.certificate_policy_record_count > ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1 {
            return Err(format!(
                "X.509 certificate-policy revision count exceeds {}",
                ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1
            ));
        }
        let PrivacyStateItemRecordV1::ZkX509CertificatePolicyGovernance {
            record,
            admitted_at_height,
        } = state_record
        else {
            return Err(format!(
                "X.509 certificate-policy revision {trust_anchor_id:?}/{policy_id:?}/{record_epoch} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "X.509 certificate-policy revision {trust_anchor_id:?}/{policy_id:?}/{record_epoch} has zero admission height"
            ));
        }
        record.validate().map_err(|error| {
            format!(
                "X.509 certificate-policy revision {trust_anchor_id:?}/{policy_id:?}/{record_epoch} is invalid: {error}"
            )
        })?;
        if record.trust_anchor_id != trust_anchor_id
            || record.policy_id != policy_id
            || record.record_epoch != record_epoch
        {
            return Err(format!(
                "X.509 certificate-policy revision key {trust_anchor_id:?}/{policy_id:?}/{record_epoch} differs from its record"
            ));
        }
        index
            .certificate_policies
            .entry((trust_anchor_id, policy_id))
            .or_default()
            .push(record.clone());
    }

    for (key, state_record) in
        commitments.range(PrivacyCommitmentKeyV1::zk_x509_crl_current_range())
    {
        key.validate()
            .map_err(|error| format!("invalid X.509 current signed-CRL key: {error}"))?;
        let PrivacyCommitmentKeyV1::ZkX509CrlCurrent {
            trust_anchor_id,
            policy_id,
        } = *key
        else {
            return Err("X.509 signed-CRL range crossed a typed key boundary".to_owned());
        };
        index.crl_lineage_count = index
            .crl_lineage_count
            .checked_add(1)
            .ok_or_else(|| "X.509 signed-CRL lineage count overflow".to_owned())?;
        if index.crl_lineage_count > ZK_X509_MAX_CRL_LINEAGES_V1 {
            return Err(format!(
                "X.509 signed-CRL lineage count exceeds {}",
                ZK_X509_MAX_CRL_LINEAGES_V1
            ));
        }
        let PrivacyStateItemRecordV1::ZkX509CrlGovernance {
            record,
            admitted_at_height,
        } = state_record
        else {
            return Err(format!(
                "X.509 current signed-CRL {trust_anchor_id:?}/{policy_id:?} has wrong-role provenance"
            ));
        };
        if *admitted_at_height == 0 {
            return Err(format!(
                "X.509 current signed-CRL {trust_anchor_id:?}/{policy_id:?} has zero admission height"
            ));
        }
        record.validate().map_err(|error| {
            format!(
                "X.509 current signed-CRL {trust_anchor_id:?}/{policy_id:?} is invalid: {error}"
            )
        })?;
        if record.trust_anchor_id != trust_anchor_id || record.certificate_policy_id != policy_id {
            return Err(format!(
                "X.509 current signed-CRL key {trust_anchor_id:?}/{policy_id:?} differs from its record"
            ));
        }
        if index
            .current_crls
            .insert((trust_anchor_id, policy_id), *record)
            .is_some()
        {
            return Err(format!(
                "X.509 current signed-CRL {trust_anchor_id:?}/{policy_id:?} is duplicated"
            ));
        }
    }

    for (trust_anchor_id, records) in &index.trust_anchors {
        validate_zk_x509_trust_anchor_lineage_v1(*trust_anchor_id, records)?;
    }
    for ((trust_anchor_id, policy_id), records) in &index.certificate_policies {
        if !index.trust_anchors.contains_key(trust_anchor_id) {
            return Err(format!(
                "X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} references a missing trust-anchor lineage"
            ));
        }
        validate_zk_x509_certificate_policy_lineage_v1(*trust_anchor_id, *policy_id, records)?;
        let current_policy = records
            .last()
            .expect("validated X.509 certificate-policy lineage is non-empty");
        let current_trust_anchor = index
            .trust_anchors
            .get(trust_anchor_id)
            .and_then(|lineage| lineage.last())
            .expect("referenced validated X.509 trust-anchor lineage is non-empty");
        if current_policy.lifecycle == PrivacyZkX509RecordLifecycleV1::Active
            && current_trust_anchor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
        {
            return Err(format!(
                "active X.509 certificate-policy lineage {trust_anchor_id:?}/{policy_id:?} has a revoked trust anchor"
            ));
        }
    }
    for ((trust_anchor_id, policy_id), crl_record) in &index.current_crls {
        let Some(policy_lineage) = index
            .certificate_policies
            .get(&(*trust_anchor_id, *policy_id))
        else {
            return Err(format!(
                "X.509 current signed-CRL {trust_anchor_id:?}/{policy_id:?} references a missing certificate-policy lineage"
            ));
        };
        if crl_record.lifecycle == PrivacyZkX509RecordLifecycleV1::Active {
            let current_policy = policy_lineage
                .last()
                .expect("validated X.509 certificate-policy lineage is non-empty");
            let current_trust_anchor = index
                .trust_anchors
                .get(trust_anchor_id)
                .and_then(|lineage| lineage.last())
                .expect("referenced validated X.509 trust-anchor lineage is non-empty");
            if current_policy.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
                || current_trust_anchor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
            {
                return Err(format!(
                    "active X.509 signed-CRL {trust_anchor_id:?}/{policy_id:?} has a revoked parent record"
                ));
            }
        }
    }
    Ok(index)
}

/// Validate all X.509 governance lineages and return their exact global counts.
pub(crate) fn privacy_zk_x509_governance_record_counts_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<(usize, usize), String> {
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    Ok((
        index.trust_anchor_record_count,
        index.certificate_policy_record_count,
    ))
}

/// Validate all X.509 governance and return the current signed-CRL count.
pub(crate) fn privacy_zk_x509_crl_lineage_count_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<usize, String> {
    Ok(load_privacy_zk_x509_governance_index_v1(commitments)?.crl_lineage_count)
}

/// Ensure a trust anchor has no active policy or signed-CRL children.
pub(crate) fn validate_privacy_zk_x509_trust_anchor_revocation_dependencies_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<(), String> {
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    for ((candidate_anchor_id, policy_id), lineage) in &index.certificate_policies {
        if *candidate_anchor_id == trust_anchor_id
            && lineage
                .last()
                .is_some_and(|record| record.lifecycle == PrivacyZkX509RecordLifecycleV1::Active)
        {
            return Err(format!(
                "X.509 trust anchor {trust_anchor_id:?} still has active certificate policy {policy_id:?}"
            ));
        }
    }
    for ((candidate_anchor_id, policy_id), record) in &index.current_crls {
        if *candidate_anchor_id == trust_anchor_id
            && record.lifecycle == PrivacyZkX509RecordLifecycleV1::Active
        {
            return Err(format!(
                "X.509 trust anchor {trust_anchor_id:?} still has active signed CRL {policy_id:?}"
            ));
        }
    }
    Ok(())
}

/// Ensure a certificate policy has no active signed-CRL child.
pub(crate) fn validate_privacy_zk_x509_policy_revocation_dependencies_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<(), String> {
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    if index
        .current_crls
        .get(&(trust_anchor_id, policy_id))
        .is_some_and(|record| record.lifecycle == PrivacyZkX509RecordLifecycleV1::Active)
    {
        return Err(format!(
            "X.509 certificate policy {trust_anchor_id:?}/{policy_id:?} still has an active signed CRL"
        ));
    }
    Ok(())
}

/// Load the current revision of one validated X.509 trust-anchor lineage.
pub(crate) fn load_privacy_zk_x509_trust_anchor_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyZkX509TrustAnchorRecordV1, String> {
    if trust_anchor_id.is_zero() {
        return Err("X.509 trust-anchor lookup id must be non-zero".to_owned());
    }
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    index
        .trust_anchors
        .get(&trust_anchor_id)
        .and_then(|records| records.last())
        .copied()
        .ok_or_else(|| format!("X.509 trust-anchor {trust_anchor_id:?} is not registered"))
}

/// Load the current revision of one validated X.509 certificate-policy lineage.
pub(crate) fn load_privacy_zk_x509_certificate_policy_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyZkX509CertificatePolicyRecordV1, String> {
    if trust_anchor_id.is_zero() || policy_id.is_zero() {
        return Err("X.509 certificate-policy lookup ids must be non-zero".to_owned());
    }
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    index
        .certificate_policies
        .get(&(trust_anchor_id, policy_id))
        .and_then(|records| records.last())
        .cloned()
        .ok_or_else(|| {
            format!("X.509 certificate policy {trust_anchor_id:?}/{policy_id:?} is not registered")
        })
}

/// Load the current self-chained signed-CRL record for one policy lineage.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn load_privacy_zk_x509_crl_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<PrivacyZkX509CrlRecordV1, String> {
    if trust_anchor_id.is_zero() || policy_id.is_zero() {
        return Err("X.509 signed-CRL lookup ids must be non-zero".to_owned());
    }
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    index
        .current_crls
        .get(&(trust_anchor_id, policy_id))
        .copied()
        .ok_or_else(|| {
            format!("X.509 signed CRL {trust_anchor_id:?}/{policy_id:?} is not registered")
        })
}

fn require_active_zk_x509_trust_anchor_v1(
    record: PrivacyZkX509TrustAnchorRecordV1,
) -> Result<PrivacyZkX509TrustAnchorRecordV1, String> {
    if record.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(format!(
            "X.509 trust-anchor {:?} is revoked",
            record.trust_anchor_id
        ));
    }
    Ok(record)
}

fn require_active_zk_x509_certificate_policy_v1(
    record: PrivacyZkX509CertificatePolicyRecordV1,
) -> Result<PrivacyZkX509CertificatePolicyRecordV1, String> {
    if record.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(format!(
            "X.509 certificate policy {:?}/{:?} is revoked",
            record.trust_anchor_id, record.policy_id
        ));
    }
    Ok(record)
}

fn require_active_zk_x509_crl_v1(
    record: PrivacyZkX509CrlRecordV1,
) -> Result<PrivacyZkX509CrlRecordV1, String> {
    if record.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(format!(
            "X.509 signed CRL {:?}/{:?} is revoked",
            record.trust_anchor_id, record.certificate_policy_id
        ));
    }
    Ok(record)
}

/// Derive the sole trust-anchor-wide namespace for one X.509 CA root.
pub(crate) fn privacy_zk_x509_ca_namespace_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
) -> Result<PrivacyNamespaceV1, String> {
    let namespace = PrivacyNamespaceV1::new(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyNamespaceScopeV1::TrustAnchor(PrivacyTrustAnchorNamespaceV1 { trust_anchor_id }),
    );
    namespace
        .validate()
        .map_err(|error| format!("invalid X.509 CA namespace: {error}"))?;
    Ok(namespace)
}

/// Derive the sole policy-scoped namespace for one X.509 statement and CRL root.
pub(crate) fn privacy_zk_x509_policy_namespace_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
) -> Result<PrivacyNamespaceV1, String> {
    let namespace = PrivacyNamespaceV1::new(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
        PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
            trust_anchor_id,
            policy_id,
        }),
    );
    namespace
        .validate()
        .map_err(|error| format!("invalid X.509 policy namespace: {error}"))?;
    Ok(namespace)
}

fn zk_x509_ca_namespace_component_v1(
    namespace: PrivacyNamespaceV1,
) -> Result<PrivacyIssuerIdV1, String> {
    namespace
        .validate()
        .map_err(|error| format!("invalid X.509 namespace: {error}"))?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
        return Err("X.509 CA state requires the X.509 protocol namespace".to_owned());
    }
    let PrivacyNamespaceScopeV1::TrustAnchor(scope) = namespace.scope() else {
        return Err("X.509 CA state requires a trust-anchor-wide scope".to_owned());
    };
    Ok(scope.trust_anchor_id)
}

fn zk_x509_policy_namespace_components_v1(
    namespace: PrivacyNamespaceV1,
) -> Result<(PrivacyIssuerIdV1, PrivacyPolicyIdV1), String> {
    namespace
        .validate()
        .map_err(|error| format!("invalid X.509 namespace: {error}"))?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
        return Err("X.509 policy state requires the X.509 protocol namespace".to_owned());
    }
    let PrivacyNamespaceScopeV1::TrustAnchorPolicy(scope) = namespace.scope() else {
        return Err("X.509 policy state requires a trust-anchor/policy scope".to_owned());
    };
    Ok((scope.trust_anchor_id, scope.policy_id))
}

fn validate_zk_x509_root_provenance_v1(
    key: PrivacyRootKeyV1,
    provenance: PrivacyRootProvenanceV1,
) -> Result<(), String> {
    let (publication_digest, namespace, epoch, root, role) = match provenance {
        PrivacyRootProvenanceV1::ZkX509CaGovernance {
            publication_digest,
            namespace,
            epoch,
            root,
            trust_anchor_record,
            ..
        } => {
            if key.role() != PrivacyRootRoleV1::CertificateAuthorityMembership {
                return Err("X.509 CA provenance was stored under a non-CA role".to_owned());
            }
            let trust_anchor_id = zk_x509_ca_namespace_component_v1(namespace)?;
            trust_anchor_record
                .validate()
                .map_err(|error| format!("invalid embedded X.509 trust-anchor record: {error}"))?;
            if trust_anchor_record.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
                || trust_anchor_record.trust_anchor_id != trust_anchor_id
                || trust_anchor_record.ca_membership_root != root
                || trust_anchor_record.ca_membership_root_epoch != epoch
            {
                return Err(
                    "X.509 CA provenance does not reproduce its active trust-anchor record"
                        .to_owned(),
                );
            }
            (
                publication_digest,
                namespace,
                epoch,
                root,
                PrivacyRootRoleV1::CertificateAuthorityMembership,
            )
        }
        _ => {
            return Err(format!(
                "X.509 root {:?}/{:?}/{} has non-X.509 provenance",
                key.namespace(),
                key.role(),
                key.epoch()
            ));
        }
    };
    if namespace != key.namespace()
        || role != key.role()
        || epoch != key.epoch()
        || root != key.root()
    {
        return Err(format!(
            "X.509 root provenance does not reproduce its exact key {:?}/{:?}/{}",
            key.namespace(),
            key.role(),
            key.epoch()
        ));
    }
    let expected_publication_digest = PrivacyRootPublicationV1 {
        namespace,
        role,
        epoch,
        root,
    }
    .digest()
    .map_err(|error| format!("X.509 root publication digest encoding failed: {error}"))?;
    if publication_digest != expected_publication_digest {
        return Err("X.509 root provenance carries a substituted publication digest".to_owned());
    }
    Ok(())
}

fn validate_zk_x509_root_history_v1(
    namespace: PrivacyNamespaceV1,
    role: PrivacyRootRoleV1,
    retained_root_count: usize,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyRootHeadRecordV1, String> {
    if retained_root_count == 0 {
        return Err("X.509 retained-root count must be non-zero".to_owned());
    }
    let head_key = PrivacyRootHeadKeyV1::new(namespace, role)
        .map_err(|error| format!("invalid X.509 root-head key: {error}"))?;
    let head = root_heads
        .get(&head_key)
        .copied()
        .ok_or_else(|| format!("X.509 {role:?} history has no current head"))?;
    head.validate()
        .map_err(|error| format!("invalid X.509 {role:?} head: {error}"))?;
    let retention_anchor = head.retention_anchor();

    let mut history = Vec::new();
    for (key, provenance) in roots.range(PrivacyRootKeyV1::history_range(namespace, role)) {
        if history.len() == retained_root_count {
            return Err(format!(
                "X.509 {role:?} history exceeds retention {retained_root_count}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid X.509 root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid X.509 root provenance: {error}"))?;
        validate_zk_x509_root_provenance_v1(*key, *provenance)?;
        history.push((*key, *provenance));
    }
    let first = history
        .first()
        .ok_or_else(|| format!("X.509 {role:?} history is empty"))?;
    if let Some(anchor) = retention_anchor {
        if anchor.epoch().checked_add(1) != Some(first.0.epoch()) {
            return Err(format!(
                "X.509 {role:?} first retained epoch does not immediately follow its retention anchor"
            ));
        }
    } else if first.0.epoch() != 1 {
        return Err(format!(
            "X.509 {role:?} unpruned history must begin at epoch one"
        ));
    }
    for pair in history.windows(2) {
        if pair[0].0.epoch().checked_add(1) != Some(pair[1].0.epoch()) {
            return Err(format!(
                "X.509 {role:?} history has a gap or duplicate epoch"
            ));
        }
    }
    let latest = history
        .last()
        .expect("non-empty X.509 history checked above");
    if head.epoch() != latest.0.epoch()
        || head.root() != latest.0.root()
        || head.provenance() != latest.1
    {
        return Err(format!(
            "X.509 {role:?} head does not equal its latest retained history entry"
        ));
    }
    Ok(head)
}

/// Fully joined authoritative X.509 governance and root state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyZkX509AuthoritativeStateV1 {
    namespace: PrivacyNamespaceV1,
    trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
    certificate_policy: PrivacyZkX509CertificatePolicyRecordV1,
    crl_record: PrivacyZkX509CrlRecordV1,
    ca_membership_root_epoch: u64,
    ca_membership_root: PrivacyRootV1,
}

impl PrivacyZkX509AuthoritativeStateV1 {
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn trust_anchor(&self) -> PrivacyZkX509TrustAnchorRecordV1 {
        self.trust_anchor
    }

    #[must_use]
    pub(crate) const fn certificate_policy(&self) -> &PrivacyZkX509CertificatePolicyRecordV1 {
        &self.certificate_policy
    }

    #[must_use]
    pub(crate) const fn crl_record(&self) -> PrivacyZkX509CrlRecordV1 {
        self.crl_record
    }

    #[must_use]
    pub(crate) const fn ca_membership_root_epoch(&self) -> u64 {
        self.ca_membership_root_epoch
    }

    #[must_use]
    pub(crate) const fn ca_membership_root(&self) -> PrivacyRootV1 {
        self.ca_membership_root
    }
}

fn validate_current_zk_x509_ca_root_binding_v1(
    head: PrivacyRootHeadRecordV1,
    trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
) -> Result<(), String> {
    let PrivacyRootProvenanceV1::ZkX509CaGovernance {
        trust_anchor_record,
        ..
    } = head.provenance()
    else {
        return Err("current X.509 CA-root head has non-CA provenance".to_owned());
    };
    if trust_anchor_record != trust_anchor
        || head.root() != trust_anchor.ca_membership_root
        || head.epoch() != trust_anchor.ca_membership_root_epoch
    {
        return Err(
            "current X.509 CA-root head is stale against the authoritative trust-anchor record"
                .to_owned(),
        );
    }
    Ok(())
}

/// Validate the single trust-anchor-wide CA history against its active record.
pub(crate) fn validate_privacy_zk_x509_trust_anchor_root_state_v1(
    trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
    retained_root_count: u32,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<(), String> {
    let retained_root_count = usize::try_from(retained_root_count)
        .map_err(|_| "X.509 retained-root count cannot be represented".to_owned())?;
    let namespace = privacy_zk_x509_ca_namespace_v1(trust_anchor.trust_anchor_id)?;
    let head = validate_zk_x509_root_history_v1(
        namespace,
        PrivacyRootRoleV1::CertificateAuthorityMembership,
        retained_root_count,
        roots,
        root_heads,
    )?;
    validate_current_zk_x509_ca_root_binding_v1(head, trust_anchor)
}

/// Load current active X.509 records and the exact authoritative CA root head.
pub(crate) fn load_privacy_zk_x509_authoritative_state_v1(
    trust_anchor_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    retained_root_count: u32,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyZkX509AuthoritativeStateV1, String> {
    let retained_root_count_u32 = retained_root_count;
    let index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    let trust_anchor = index
        .trust_anchors
        .get(&trust_anchor_id)
        .and_then(|records| records.last())
        .copied()
        .ok_or_else(|| format!("X.509 trust-anchor {trust_anchor_id:?} is not registered"))
        .and_then(require_active_zk_x509_trust_anchor_v1)?;
    let certificate_policy = index
        .certificate_policies
        .get(&(trust_anchor_id, policy_id))
        .and_then(|records| records.last())
        .cloned()
        .ok_or_else(|| {
            format!("X.509 certificate policy {trust_anchor_id:?}/{policy_id:?} is not registered")
        })
        .and_then(require_active_zk_x509_certificate_policy_v1)?;
    let crl_record = index
        .current_crls
        .get(&(trust_anchor_id, policy_id))
        .copied()
        .ok_or_else(|| {
            format!("X.509 signed CRL {trust_anchor_id:?}/{policy_id:?} is not registered")
        })
        .and_then(require_active_zk_x509_crl_v1)?;
    let namespace = privacy_zk_x509_policy_namespace_v1(trust_anchor_id, policy_id)?;
    validate_privacy_zk_x509_trust_anchor_root_state_v1(
        trust_anchor,
        retained_root_count_u32,
        roots,
        root_heads,
    )?;
    let ca_head_key = PrivacyRootHeadKeyV1::new(
        privacy_zk_x509_ca_namespace_v1(trust_anchor_id)?,
        PrivacyRootRoleV1::CertificateAuthorityMembership,
    )
    .map_err(|error| format!("invalid X.509 CA root-head key: {error}"))?;
    let ca_head = root_heads
        .get(&ca_head_key)
        .copied()
        .ok_or_else(|| "X.509 CA root history has no current head".to_owned())?;
    Ok(PrivacyZkX509AuthoritativeStateV1 {
        namespace,
        trust_anchor,
        certificate_policy,
        crl_record,
        ca_membership_root_epoch: ca_head.epoch(),
        ca_membership_root: ca_head.root(),
    })
}

/// Compare one public X.509 statement to a fully joined authoritative snapshot.
pub(crate) fn validate_privacy_zk_x509_statement_state_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
) -> Result<(), String> {
    PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone())
        .validate(consensus_limits)
        .map_err(|error| format!("invalid X.509 public statement: {error}"))?;
    if PrivacyNamespaceV1::from_statement(&PrivacyStatementV1::IrohaZkX509StarkP256V0(
        statement.clone(),
    )) != state.namespace()
    {
        return Err("X.509 statement namespace differs from authoritative state".to_owned());
    }
    let trust_anchor = state.trust_anchor();
    if statement.trust_anchor_record_digest != trust_anchor.record_digest
        || statement.trust_anchor_record_epoch != trust_anchor.record_epoch
    {
        return Err(
            "X.509 statement selects a stale or substituted trust-anchor revision".to_owned(),
        );
    }
    let certificate_policy = state.certificate_policy();
    if statement.certificate_policy_record_digest != certificate_policy.record_digest
        || statement.certificate_policy_record_epoch != certificate_policy.record_epoch
    {
        return Err(
            "X.509 statement selects a stale or substituted certificate-policy revision".to_owned(),
        );
    }
    let crl_record = state.crl_record();
    if statement.crl_record_digest != crl_record.record_digest
        || statement.crl_record_epoch != crl_record.record_epoch
    {
        return Err(
            "X.509 statement selects a stale or substituted signed-CRL revision".to_owned(),
        );
    }
    let trusted_block_unix_seconds = trusted_block_timestamp_ms / 1_000;
    if trusted_block_unix_seconds < statement.presentation_not_before_unix_seconds
        || trusted_block_unix_seconds > statement.presentation_not_after_unix_seconds
    {
        return Err(
            "X.509 executing block timestamp is outside the presentation window".to_owned(),
        );
    }
    if statement.presentation_not_before_unix_seconds < crl_record.this_update_unix_seconds
        || statement.presentation_not_after_unix_seconds >= crl_record.next_update_unix_seconds
        || statement
            .presentation_not_after_unix_seconds
            .checked_sub(crl_record.this_update_unix_seconds)
            .is_none_or(|age| age > ZK_X509_MAX_CRL_AGE_SECONDS_V1)
    {
        return Err(
            "X.509 presentation window is not fully covered by the current signed-CRL freshness window"
                .to_owned(),
        );
    }
    if statement.ca_membership_root != state.ca_membership_root()
        || statement.ca_membership_root_epoch != state.ca_membership_root_epoch()
    {
        return Err("X.509 statement selects a stale or substituted CA root".to_owned());
    }
    if statement.key_usage != certificate_policy.required_key_usage
        || statement.extended_key_usages != certificate_policy.required_extended_key_usages
    {
        return Err("X.509 statement predicates differ from the authoritative policy".to_owned());
    }
    if !statement
        .disclosed_attributes
        .iter()
        .map(|attribute| attribute.index)
        .eq(certificate_policy
            .required_disclosed_attribute_indices
            .iter()
            .copied())
    {
        return Err(
            "X.509 disclosed attributes do not exactly equal the authoritative required set"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_privacy_zk_ace_replay_binding_v1(
    registered_policy_ids: &BTreeSet<PrivacyPolicyIdV1>,
    policy_id: PrivacyPolicyIdV1,
    record: &PrivacyStateItemRecordV1,
) -> Result<(), String> {
    if !registered_policy_ids.contains(&policy_id) {
        return Err(format!(
            "ZK-ACE replay marker references missing policy {policy_id:?}"
        ));
    }
    if !matches!(
        record,
        PrivacyStateItemRecordV1::ZkAceVerifiedAuthorization {
            policy_id: observed,
            ..
        } if *observed == policy_id
    ) {
        return Err(format!(
            "ZK-ACE replay marker for {policy_id:?} has wrong-role provenance"
        ));
    }
    Ok(())
}

/// Fully validated, transaction-local view of one ZK-AMS AccountRegistry.
///
/// The snapshot joins the immutable governed issuer record to the exact
/// retained root chain.  Callers can therefore compare proof input against one
/// coherent authoritative state without trusting duplicated statement fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyZkAmsRegistrySnapshotV1 {
    namespace: PrivacyNamespaceV1,
    issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
    current_epoch: u64,
    current_root: PrivacyRootV1,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    retained_roots: Vec<(PrivacyRootKeyV1, PrivacyRootProvenanceV1)>,
}

impl PrivacyZkAmsRegistrySnapshotV1 {
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn issuer_policy_record_digest(
        &self,
    ) -> PrivacyZkAmsIssuerPolicyRecordDigestV1 {
        self.issuer_policy_record_digest
    }

    #[must_use]
    pub(crate) const fn bootstrap_digest(&self) -> PrivacyZkAmsRegistryBootstrapDigestV1 {
        self.bootstrap_digest
    }

    #[must_use]
    pub(crate) const fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    #[must_use]
    pub(crate) const fn current_root(&self) -> PrivacyRootV1 {
        self.current_root
    }

    #[must_use]
    pub(crate) const fn retention_anchor(&self) -> Option<PrivacyRootRetentionAnchorV1> {
        self.retention_anchor
    }

    /// Return trusted retained membership for the exact current head.
    #[must_use]
    pub(crate) fn retained_current_root(&self) -> Option<(u64, PrivacyRootV1)> {
        self.retained_roots
            .iter()
            .any(|(key, _)| key.epoch() == self.current_epoch && key.root() == self.current_root)
            .then_some((self.current_epoch, self.current_root))
    }
}

fn validate_zk_ams_successor_link_v1(
    namespace: PrivacyNamespaceV1,
    key: PrivacyRootKeyV1,
    provenance: PrivacyRootProvenanceV1,
    bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
) -> Result<(u64, PrivacyRootV1), String> {
    let PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor {
        bootstrap_digest: observed_bootstrap_digest,
        parent_epoch,
        parent_root,
        ..
    } = provenance
    else {
        return Err(format!(
            "ZK-AMS AccountRegistry history {namespace:?} contains a non-successor advancement"
        ));
    };
    if observed_bootstrap_digest != bootstrap_digest {
        return Err(format!(
            "ZK-AMS AccountRegistry successor {} is bound to a different bootstrap",
            key.epoch()
        ));
    }
    if parent_epoch.checked_add(1) != Some(key.epoch()) {
        return Err(format!(
            "ZK-AMS AccountRegistry successor {} does not advance parent epoch {parent_epoch} by exactly one",
            key.epoch()
        ));
    }
    Ok((parent_epoch, parent_root))
}

fn validate_zk_ams_retained_root_chain_v1(
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
    retained_root_count: usize,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    history: &[(PrivacyRootKeyV1, PrivacyRootProvenanceV1)],
) -> Result<(), String> {
    if retained_root_count == 0 {
        return Err("ZK-AMS retained-root count must be non-zero".to_owned());
    }
    if history.is_empty() {
        return Err("ZK-AMS AccountRegistry has no retained root history".to_owned());
    }
    if history.len() > retained_root_count {
        return Err(format!(
            "ZK-AMS AccountRegistry history exceeds retention {retained_root_count}"
        ));
    }

    let (first_key, first_provenance) = history[0];
    match first_provenance {
        PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap {
            bootstrap_digest: observed_bootstrap_digest,
            ..
        } => {
            if observed_bootstrap_digest != bootstrap_digest {
                return Err(
                    "ZK-AMS AccountRegistry root origin differs from its issuer record".to_owned(),
                );
            }
            if retention_anchor.is_some() {
                return Err(
                    "ZK-AMS retained bootstrap history has an unexpected pruned-prefix anchor"
                        .to_owned(),
                );
            }
            if first_key.epoch() != ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1 {
                return Err(format!(
                    "ZK-AMS AccountRegistry bootstrap epoch {} is not canonical epoch {}",
                    first_key.epoch(),
                    ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1
                ));
            }
        }
        PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. } => {
            let (parent_epoch, parent_root) = validate_zk_ams_successor_link_v1(
                namespace,
                first_key,
                first_provenance,
                bootstrap_digest,
            )?;
            if first_key.epoch() <= ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1 {
                return Err(
                    "ZK-AMS pruned history begins at or before the canonical bootstrap epoch"
                        .to_owned(),
                );
            }
            let anchor = retention_anchor.ok_or_else(|| {
                "ZK-AMS pruned-prefix history has no exact retention anchor".to_owned()
            })?;
            if anchor.epoch().checked_add(1) != Some(first_key.epoch())
                || parent_epoch != anchor.epoch()
                || parent_root != anchor.root()
            {
                return Err(
                    "ZK-AMS first retained successor does not consume its exact pruned-prefix anchor"
                        .to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::Governance { .. }
        | PrivacyRootProvenanceV1::ZkX509CaGovernance { .. }
        | PrivacyRootProvenanceV1::OrchardPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::OrchardPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::VerifiedBootstrap { .. }
        | PrivacyRootProvenanceV1::VerifiedProof { .. }
        | PrivacyRootProvenanceV1::VerifiedPgcSuccessor { .. } => {
            return Err(
                "ZK-AMS AccountRegistry retained history begins with invalid provenance".to_owned(),
            );
        }
    }

    for adjacent in history.windows(2) {
        let (parent_key, _) = adjacent[0];
        let (child_key, child_provenance) = adjacent[1];
        let (declared_parent_epoch, declared_parent_root) = validate_zk_ams_successor_link_v1(
            namespace,
            child_key,
            child_provenance,
            bootstrap_digest,
        )?;
        if parent_key.epoch().checked_add(1) != Some(child_key.epoch())
            || declared_parent_epoch != parent_key.epoch()
            || declared_parent_root != parent_key.root()
        {
            return Err(format!(
                "ZK-AMS AccountRegistry history has a gap or forged parent between epochs {} and {}",
                parent_key.epoch(),
                child_key.epoch()
            ));
        }
    }
    if retention_anchor.is_some() && history.len() != retained_root_count {
        return Err(format!(
            "ZK-AMS anchored history has {} roots but must fill retention {retained_root_count}",
            history.len()
        ));
    }
    Ok(())
}

/// Load and validate every bounded authoritative component of one ZK-AMS
/// AccountRegistry.
///
/// # Errors
///
/// Rejects a missing or duplicate issuer record, cross-bootstrap provenance,
/// malformed/pruned root chains, over-retention history, and a head that is
/// not the exact newest retained root.
pub(crate) fn load_privacy_zk_ams_registry_snapshot_v1(
    namespace: PrivacyNamespaceV1,
    retained_root_count: u32,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyZkAmsRegistrySnapshotV1, String> {
    validate_zk_ams_namespace(namespace)
        .map_err(|error| format!("invalid ZK-AMS registry namespace: {error}"))?;
    if retained_root_count == 0 {
        return Err("ZK-AMS retained-root count must be non-zero".to_owned());
    }
    let retained_root_count = usize::try_from(retained_root_count)
        .map_err(|_| "ZK-AMS retained-root count cannot be represented".to_owned())?;

    let mut issuer_record = None;
    for (key, record) in commitments.range(
        PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record_range(namespace),
    ) {
        if issuer_record.is_some() {
            return Err("ZK-AMS registry has multiple governed issuer-policy records".to_owned());
        }
        key.validate()
            .map_err(|error| format!("invalid ZK-AMS issuer-policy key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid ZK-AMS issuer-policy provenance: {error}"))?;
        let record_digest = key.zk_ams_issuer_policy_digest().ok_or_else(|| {
            "ZK-AMS issuer-policy range returned a differently typed key".to_owned()
        })?;
        let PrivacyStateItemRecordV1::ZkAmsGovernance {
            bootstrap_digest, ..
        } = *record
        else {
            return Err(
                "ZK-AMS issuer-policy record was not installed by typed governance".to_owned(),
            );
        };
        issuer_record = Some((record_digest, bootstrap_digest));
    }
    let (issuer_policy_record_digest, bootstrap_digest) = issuer_record
        .ok_or_else(|| "ZK-AMS registry has no governed issuer-policy record".to_owned())?;

    let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::AccountRegistry)
        .map_err(|error| format!("invalid ZK-AMS registry-head key: {error}"))?;
    let head = root_heads
        .get(&head_key)
        .copied()
        .ok_or_else(|| "ZK-AMS registry has no current AccountRegistry head".to_owned())?;
    head.validate()
        .map_err(|error| format!("invalid ZK-AMS AccountRegistry head: {error}"))?;

    let mut retained_roots = Vec::new();
    for (key, provenance) in roots.range(PrivacyRootKeyV1::history_range(
        namespace,
        PrivacyRootRoleV1::AccountRegistry,
    )) {
        if retained_roots.len() == retained_root_count {
            return Err(format!(
                "ZK-AMS AccountRegistry history exceeds retention {retained_root_count}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid ZK-AMS AccountRegistry root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid ZK-AMS AccountRegistry provenance: {error}"))?;
        if retained_roots.last().is_some_and(
            |(previous, _): &(PrivacyRootKeyV1, PrivacyRootProvenanceV1)| {
                previous.epoch() == key.epoch()
            },
        ) {
            return Err(format!(
                "ZK-AMS AccountRegistry history contains duplicate epoch {}",
                key.epoch()
            ));
        }
        retained_roots.push((*key, *provenance));
    }
    validate_zk_ams_retained_root_chain_v1(
        namespace,
        bootstrap_digest,
        retained_root_count,
        head.retention_anchor(),
        &retained_roots,
    )?;
    let latest = retained_roots
        .last()
        .expect("non-empty ZK-AMS history checked above");
    if head.epoch() != latest.0.epoch()
        || head.root() != latest.0.root()
        || head.provenance() != latest.1
    {
        return Err(
            "ZK-AMS AccountRegistry head does not equal latest retained history".to_owned(),
        );
    }
    if head.provenance().zk_ams_bootstrap_digest() != Some(bootstrap_digest) {
        return Err("ZK-AMS AccountRegistry head differs from its governed bootstrap".to_owned());
    }

    Ok(PrivacyZkAmsRegistrySnapshotV1 {
        namespace,
        issuer_policy_record_digest,
        bootstrap_digest,
        current_epoch: head.epoch(),
        current_root: head.root(),
        retention_anchor: head.retention_anchor(),
        retained_roots,
    })
}

fn fcmp_output_to_native_v1(
    output: PrivacyFcmpOutputTupleV1,
) -> Result<crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1, &'static str> {
    crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1::new(
        output.output_key,
        output.linking_tag_generator,
        output.amount_commitment,
    )
    .map_err(|_| "FCMP++ output tuple is not a canonical prime-order Edwards tuple")
}

fn fcmp_output_from_native_v1(
    output: crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1,
) -> PrivacyFcmpOutputTupleV1 {
    let (output_key, linking_tag_generator, amount_commitment) = output.components();
    PrivacyFcmpOutputTupleV1 {
        output_key,
        linking_tag_generator,
        amount_commitment,
    }
}

fn fcmp_root_to_native_v1(
    root: PrivacyFcmpTreeRootV1,
) -> Result<crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1, &'static str> {
    crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1::new(root.layers, root.point)
        .map_err(|_| "FCMP++ root is not canonical for its layer-selected curve")
}

fn fcmp_root_from_native_v1(
    root: crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1,
) -> PrivacyFcmpTreeRootV1 {
    PrivacyFcmpTreeRootV1 {
        layers: root.layers(),
        point: root.point(),
    }
}

/// Validator-owned alternating Selene/Helios frontier for one FCMP++ pool.
///
/// The complete typed root, active `(O, I, C)` branch, and every mixed-radix
/// level are durable. Restore validates the native frontier and independently
/// rebuilds it from the complete position-bound output registry.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct PrivacyFcmpAccumulatorStateV1 {
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    epoch: u64,
    root: PrivacyFcmpTreeRootV1,
    tree_size: u64,
    active_outputs: Vec<PrivacyFcmpOutputTupleV1>,
    levels: Vec<Vec<[u8; 32]>>,
}

impl PrivacyFcmpAccumulatorStateV1 {
    fn bootstrap(
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    ) -> Result<Self, &'static str> {
        let PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(fcmp) = bootstrap else {
            return Err("non-FCMP++ pool cannot construct an FCMP++ frontier");
        };
        let outputs = fcmp
            .initial_outputs
            .iter()
            .copied()
            .map(fcmp_output_to_native_v1)
            .collect::<Result<Vec<_>, _>>()?;
        let frontier = crate::privacy_engines::fcmp_plus_plus::build_fcmp_frontier_v1(&outputs)
            .map_err(|_| "FCMP++ bootstrap frontier is invalid")?;
        let state = Self::from_native_parts(bootstrap.namespace(), bootstrap_digest, 1, frontier);
        state.validate_against_bootstrap(bootstrap, bootstrap_digest)?;
        Ok(state)
    }

    fn from_native_parts(
        namespace: PrivacyNamespaceV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        epoch: u64,
        frontier: crate::privacy_engines::fcmp_plus_plus::FcmpFrontierPartsV1,
    ) -> Self {
        Self {
            namespace,
            bootstrap_digest,
            epoch,
            root: fcmp_root_from_native_v1(frontier.root),
            tree_size: frontier.tree_size,
            active_outputs: frontier
                .active_outputs
                .into_iter()
                .map(fcmp_output_from_native_v1)
                .collect(),
            levels: frontier.levels,
        }
    }

    fn to_native_parts(
        &self,
    ) -> Result<crate::privacy_engines::fcmp_plus_plus::FcmpFrontierPartsV1, &'static str> {
        Ok(
            crate::privacy_engines::fcmp_plus_plus::FcmpFrontierPartsV1 {
                tree_size: self.tree_size,
                active_outputs: self
                    .active_outputs
                    .iter()
                    .copied()
                    .map(fcmp_output_to_native_v1)
                    .collect::<Result<Vec<_>, _>>()?,
                levels: self.levels.clone(),
                root: fcmp_root_to_native_v1(self.root)?,
            },
        )
    }

    fn validate_against_bootstrap(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    ) -> Result<(), &'static str> {
        bootstrap
            .validate()
            .map_err(|_| "FCMP++ accumulator bootstrap is invalid")?;
        let PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(fcmp) = bootstrap else {
            return Err("non-FCMP++ pool cannot carry an FCMP++ frontier");
        };
        if self.namespace != bootstrap.namespace()
            || self.bootstrap_digest.is_zero()
            || self.bootstrap_digest != bootstrap_digest
        {
            return Err("FCMP++ accumulator differs from its immutable bootstrap");
        }
        if self.epoch == 0 || self.tree_size == 0 {
            return Err("FCMP++ accumulator epoch and tree size must be non-zero");
        }
        self.root
            .validate()
            .map_err(|_| "FCMP++ accumulator typed root is malformed")?;
        let native = self.to_native_parts()?;
        crate::privacy_engines::fcmp_plus_plus::validate_fcmp_frontier_v1(&native)
            .map_err(|_| "FCMP++ compact frontier is invalid")?;

        let origin_outputs = fcmp
            .initial_outputs
            .iter()
            .copied()
            .map(fcmp_output_to_native_v1)
            .collect::<Result<Vec<_>, _>>()?;
        let origin =
            crate::privacy_engines::fcmp_plus_plus::build_fcmp_frontier_v1(&origin_outputs)
                .map_err(|_| "FCMP++ origin frontier is invalid")?;
        let transitions = self
            .epoch
            .checked_sub(1)
            .ok_or("FCMP++ accumulator epoch precedes its origin")?;
        let minimum_size = origin
            .tree_size
            .checked_add(transitions)
            .ok_or("FCMP++ accumulator minimum size overflow")?;
        let maximum_size = origin
            .tree_size
            .checked_add(
                transitions
                    .checked_mul(u64::from(FCMP_MAX_OUTPUTS_V1))
                    .ok_or("FCMP++ accumulator maximum size overflow")?,
            )
            .ok_or("FCMP++ accumulator maximum size overflow")?;
        if self.tree_size < minimum_size || self.tree_size > maximum_size {
            return Err("FCMP++ tree size is inconsistent with its transition epoch");
        }
        if self.epoch == 1
            && (self.tree_size != origin.tree_size
                || self.root != fcmp_root_from_native_v1(origin.root)
                || self.active_outputs
                    != origin
                        .active_outputs
                        .into_iter()
                        .map(fcmp_output_from_native_v1)
                        .collect::<Vec<_>>()
                || self.levels != origin.levels)
        {
            return Err("FCMP++ epoch-one frontier differs from its canonical origin");
        }
        Ok(())
    }

    /// Advance the authoritative curve tree by one verified output batch.
    pub(crate) fn advance(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        outputs: &[PrivacyFcmpOutputTupleV1],
    ) -> Result<Self, &'static str> {
        self.validate_against_bootstrap(bootstrap, self.bootstrap_digest)?;
        let output_count = u32::try_from(outputs.len())
            .map_err(|_| "FCMP++ output count cannot be represented")?;
        if output_count == 0 || output_count > FCMP_MAX_OUTPUTS_V1 {
            return Err("FCMP++ successor output count is outside its native bound");
        }
        let native_outputs = outputs
            .iter()
            .copied()
            .map(fcmp_output_to_native_v1)
            .collect::<Result<Vec<_>, _>>()?;
        let successor = crate::privacy_engines::fcmp_plus_plus::append_fcmp_outputs_v1(
            &self.to_native_parts()?,
            &native_outputs,
        )
        .map_err(|_| "FCMP++ successor frontier is invalid")?;
        let state = Self::from_native_parts(
            self.namespace,
            self.bootstrap_digest,
            self.epoch
                .checked_add(1)
                .ok_or("FCMP++ accumulator epoch overflow")?,
            successor,
        );
        if state.root == self.root {
            return Err("FCMP++ successor root must differ from its parent");
        }
        state.validate_against_bootstrap(bootstrap, self.bootstrap_digest)?;
        Ok(state)
    }

    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub(crate) const fn root(&self) -> PrivacyFcmpTreeRootV1 {
        self.root
    }

    #[must_use]
    pub(crate) const fn tree_size(&self) -> u64 {
        self.tree_size
    }
}

/// Validator-owned compact frontier for one IVM or PQ proof-managed pool.
///
/// The fields remain private so only the validating constructors in this
/// module can create or advance a frontier.  The type itself is public because
/// it is part of the durable [`PrivacyStateItemRecordV1`] representation.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct PrivacyProofManagedAccumulatorStateV1 {
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    epoch: u64,
    root: PrivacyRootV1,
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: Vec<[u8; 32]>,
}

impl PrivacyProofManagedAccumulatorStateV1 {
    fn bootstrap(
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    ) -> Result<Self, &'static str> {
        let namespace = bootstrap.namespace();
        let max_outputs = match bootstrap.protocol_id() {
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
            PrivacyProtocolIdV1::PqMaspStarkV0 => PQ_MASP_MAX_OUTPUTS_V1,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                return Err("FCMP++ cannot construct a SHA-256 private-note frontier");
            }
            _ => return Err("proof-managed accumulator protocol is invalid"),
        };
        if max_outputs == 0 {
            return Err("proof-managed accumulator output ceiling must be non-zero");
        }
        let frontier =
            crate::privacy_engines::proof_managed_accumulator::build_proof_managed_frontier_v1(
                namespace,
                bootstrap
                    .initial_note_commitments()
                    .ok_or("private-note bootstrap omits note commitments")?,
            )
            .map_err(|_| "proof-managed bootstrap frontier is invalid")?;
        let state = Self {
            namespace,
            bootstrap_digest,
            epoch: 1,
            root: frontier.root,
            tree_size: frontier.tree_size,
            leaf: frontier.leaf,
            ommers: frontier.ommers,
        };
        state.validate_against_bootstrap(bootstrap, bootstrap_digest, frontier.root)?;
        Ok(state)
    }

    /// Advance the authoritative frontier by one verified statement.
    pub(crate) fn advance(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        output_commitments: &[PrivacyCommitmentV1],
    ) -> Result<Self, &'static str> {
        self.validate_against_bootstrap(
            bootstrap,
            self.bootstrap_digest,
            self.initial_root(bootstrap)?,
        )?;
        let max_outputs = match self.namespace.protocol_id() {
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
            PrivacyProtocolIdV1::PqMaspStarkV0 => PQ_MASP_MAX_OUTPUTS_V1,
            _ => return Err("proof-managed accumulator protocol is invalid"),
        };
        let output_count = u32::try_from(output_commitments.len())
            .map_err(|_| "proof-managed output count cannot be represented")?;
        if output_count == 0 || output_count > max_outputs {
            return Err("proof-managed successor output count is outside its native bound");
        }
        let successor =
            crate::privacy_engines::proof_managed_accumulator::append_proof_managed_commitments_v1(
                self.namespace,
                self.tree_size,
                self.leaf,
                &self.ommers,
                self.root,
                output_commitments,
            )
            .map_err(|_| "proof-managed successor frontier is invalid")?;
        if successor.root == self.root {
            return Err("proof-managed successor root must differ from its parent");
        }
        let state = Self {
            namespace: self.namespace,
            bootstrap_digest: self.bootstrap_digest,
            epoch: self
                .epoch
                .checked_add(1)
                .ok_or("proof-managed accumulator epoch overflow")?,
            root: successor.root,
            tree_size: successor.tree_size,
            leaf: successor.leaf,
            ommers: successor.ommers,
        };
        state.validate_against_bootstrap(
            bootstrap,
            self.bootstrap_digest,
            self.initial_root(bootstrap)?,
        )?;
        Ok(state)
    }

    fn initial_root(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
    ) -> Result<PrivacyRootV1, &'static str> {
        crate::privacy_engines::proof_managed_pool_initial_root_v1(bootstrap)
            .map_err(|_| "proof-managed initial root derivation failed")
    }

    fn validate_against_bootstrap(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        initial_root: PrivacyRootV1,
    ) -> Result<(), &'static str> {
        bootstrap
            .validate()
            .map_err(|_| "proof-managed accumulator bootstrap is invalid")?;
        if bootstrap.protocol_id() == PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 {
            return Err("FCMP++ cannot use the SHA-256 private-note accumulator");
        }
        if self.namespace != bootstrap.namespace()
            || self.bootstrap_digest.is_zero()
            || self.bootstrap_digest != bootstrap_digest
        {
            return Err("proof-managed accumulator differs from its immutable bootstrap");
        }
        if self.epoch == 0 || self.root.is_zero() || self.tree_size == 0 {
            return Err("proof-managed accumulator epoch, root, and tree size must be non-zero");
        }
        crate::privacy_engines::proof_managed_accumulator::validate_proof_managed_frontier_v1(
            self.namespace,
            self.tree_size,
            self.leaf,
            &self.ommers,
            self.root,
        )
        .map_err(|_| "proof-managed compact frontier is invalid")?;

        let origin =
            crate::privacy_engines::proof_managed_accumulator::build_proof_managed_frontier_v1(
                self.namespace,
                bootstrap
                    .initial_note_commitments()
                    .ok_or("private-note bootstrap omits note commitments")?,
            )
            .map_err(|_| "proof-managed origin frontier is invalid")?;
        if origin.root != initial_root {
            return Err("proof-managed origin frontier differs from the stored initial root");
        }
        let transitions = self
            .epoch
            .checked_sub(1)
            .ok_or("proof-managed accumulator epoch precedes its origin")?;
        let max_outputs = match self.namespace.protocol_id() {
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                u64::from(IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1)
            }
            PrivacyProtocolIdV1::PqMaspStarkV0 => u64::from(PQ_MASP_MAX_OUTPUTS_V1),
            _ => return Err("proof-managed accumulator protocol is invalid"),
        };
        let minimum_size = origin
            .tree_size
            .checked_add(transitions)
            .ok_or("proof-managed accumulator minimum size overflow")?;
        let maximum_size = origin
            .tree_size
            .checked_add(
                transitions
                    .checked_mul(max_outputs)
                    .ok_or("proof-managed accumulator maximum size overflow")?,
            )
            .ok_or("proof-managed accumulator maximum size overflow")?;
        if self.tree_size < minimum_size || self.tree_size > maximum_size {
            return Err("proof-managed tree size is inconsistent with its transition epoch");
        }
        if self.epoch == 1
            && (self.root != origin.root
                || self.tree_size != origin.tree_size
                || self.leaf != origin.leaf
                || self.ommers != origin.ommers)
        {
            return Err("proof-managed epoch-one frontier differs from its canonical origin");
        }
        Ok(())
    }

    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub(crate) const fn root(&self) -> PrivacyRootV1 {
        self.root
    }

    #[must_use]
    pub(crate) const fn tree_size(&self) -> u64 {
        self.tree_size
    }
}

/// Closed protocol-specific accumulator state for one proof-managed pool.
///
/// A pool always carries exactly one native frontier, and the enum
/// discriminant prevents an FCMP++ curve frontier from being decoded or
/// persisted as an IVM/PQ SHA-256 note frontier.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(tag = "kind", content = "state", deny_unknown_fields)]
pub enum PrivacyProofManagedPoolAccumulatorStateV1 {
    /// Alternating Selene/Helios FCMP++ curve-tree frontier.
    Fcmp(PrivacyFcmpAccumulatorStateV1),
    /// Domain-separated SHA-256 note frontier used by private-IVM and PQ-MASP.
    PrivateNote(PrivacyProofManagedAccumulatorStateV1),
}

impl PrivacyProofManagedPoolAccumulatorStateV1 {
    fn bootstrap(
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    ) -> Result<Self, &'static str> {
        match bootstrap.protocol_id() {
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyFcmpAccumulatorStateV1::bootstrap(bootstrap, bootstrap_digest)
                    .map(Self::Fcmp)
            }
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
            | PrivacyProtocolIdV1::PqMaspStarkV0 => {
                PrivacyProofManagedAccumulatorStateV1::bootstrap(bootstrap, bootstrap_digest)
                    .map(Self::PrivateNote)
            }
            _ => Err("proof-managed pool protocol is invalid"),
        }
    }

    fn validate_against_bootstrap(
        &self,
        bootstrap: &PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        initial_root: PrivacyRootV1,
    ) -> Result<(), &'static str> {
        match (self, bootstrap.protocol_id()) {
            (Self::Fcmp(state), PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1) => {
                state.validate_against_bootstrap(bootstrap, bootstrap_digest)?;
                if state.root().history_commitment() != initial_root && state.epoch() == 1 {
                    return Err("FCMP++ origin root differs from the shared history commitment");
                }
                Ok(())
            }
            (
                Self::PrivateNote(state),
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
                | PrivacyProtocolIdV1::PqMaspStarkV0,
            ) => state.validate_against_bootstrap(bootstrap, bootstrap_digest, initial_root),
            (Self::Fcmp(_), _) => Err("non-FCMP++ pool carries an FCMP++ curve frontier"),
            (Self::PrivateNote(_), _) => Err("FCMP++ pool carries a foreign SHA-256 note frontier"),
        }
    }

    #[must_use]
    pub(crate) const fn fcmp(&self) -> Option<&PrivacyFcmpAccumulatorStateV1> {
        match self {
            Self::Fcmp(state) => Some(state),
            Self::PrivateNote(_) => None,
        }
    }

    #[must_use]
    pub(crate) const fn private_note(&self) -> Option<&PrivacyProofManagedAccumulatorStateV1> {
        match self {
            Self::PrivateNote(state) => Some(state),
            Self::Fcmp(_) => None,
        }
    }
}

/// Fully validated view of one FCMP++, private-IVM, or PQ-MASP pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyProofManagedPoolSnapshotV1 {
    namespace: PrivacyNamespaceV1,
    root_role: PrivacyRootRoleV1,
    bootstrap: PrivacyProofManagedPoolBootstrapV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    initial_root: PrivacyRootV1,
    accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1,
    output_count: u64,
    bootstrap_admitted_at_height: u64,
    current_epoch: u64,
    current_root: PrivacyRootV1,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    retained_roots: Vec<(PrivacyRootKeyV1, PrivacyRootProvenanceV1)>,
    verified_batches: BTreeMap<u64, ProofManagedVerifiedBatchOriginV1>,
}

impl PrivacyProofManagedPoolSnapshotV1 {
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn root_role(&self) -> PrivacyRootRoleV1 {
        self.root_role
    }

    #[must_use]
    pub(crate) const fn bootstrap(&self) -> &PrivacyProofManagedPoolBootstrapV1 {
        &self.bootstrap
    }

    #[must_use]
    pub(crate) const fn bootstrap_digest(&self) -> PrivacyProofManagedPoolBootstrapDigestV1 {
        self.bootstrap_digest
    }

    #[must_use]
    pub(crate) const fn initial_root(&self) -> PrivacyRootV1 {
        self.initial_root
    }

    /// Borrow the validator-owned compact note frontier.
    #[must_use]
    pub(crate) const fn accumulator_state(&self) -> Option<&PrivacyProofManagedAccumulatorStateV1> {
        self.accumulator_state.private_note()
    }

    /// Borrow the validator-owned FCMP++ mixed-radix curve frontier.
    #[must_use]
    pub(crate) const fn fcmp_accumulator_state(&self) -> Option<&PrivacyFcmpAccumulatorStateV1> {
        self.accumulator_state.fcmp()
    }

    /// Number of genesis and proof-produced outputs in exact append order.
    #[must_use]
    pub(crate) const fn output_count(&self) -> u64 {
        self.output_count
    }

    /// Original governance height of the immutable pool bootstrap.
    #[must_use]
    pub(crate) const fn bootstrap_admitted_at_height(&self) -> u64 {
        self.bootstrap_admitted_at_height
    }

    #[must_use]
    pub(crate) const fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    #[must_use]
    pub(crate) const fn current_root(&self) -> PrivacyRootV1 {
        self.current_root
    }

    #[must_use]
    pub(crate) const fn retention_anchor(&self) -> Option<PrivacyRootRetentionAnchorV1> {
        self.retention_anchor
    }

    /// Return retained membership for the exact current head.
    #[must_use]
    pub(crate) fn retained_current_root(&self) -> Option<(u64, PrivacyRootV1)> {
        self.retained_roots
            .iter()
            .any(|(key, _)| key.epoch() == self.current_epoch && key.root() == self.current_root)
            .then_some((self.current_epoch, self.current_root))
    }

    /// Return whether the authoritative retained window contains this exact
    /// epoch/root pair.
    ///
    /// FCMP++ membership may anchor to any exactly retained append-only
    /// output-set root, while every successful transition still mutates the
    /// current frontier.
    #[must_use]
    pub(crate) fn contains_retained_root(&self, epoch: u64, root: PrivacyRootV1) -> bool {
        self.retained_roots
            .iter()
            .any(|(key, _)| key.epoch() == epoch && key.root() == root)
    }

    fn contains_verified_batch(&self, origin: ProofManagedVerifiedBatchOriginV1) -> bool {
        self.verified_batches
            .values()
            .any(|candidate| *candidate == origin)
    }

    /// Append verified IVM/PQ outputs to the authoritative compact frontier.
    pub(crate) fn derive_note_successor(
        &self,
        output_commitments: &[PrivacyCommitmentV1],
    ) -> Result<PrivacyProofManagedAccumulatorStateV1, String> {
        let state = self
            .accumulator_state
            .private_note()
            .ok_or_else(|| "proof-managed pool has no private-note frontier".to_owned())?;
        state
            .advance(&self.bootstrap, output_commitments)
            .map_err(str::to_owned)
    }

    /// Append verified FCMP++ outputs to the authoritative curve frontier.
    pub(crate) fn derive_fcmp_successor(
        &self,
        outputs: &[PrivacyFcmpOutputTupleV1],
    ) -> Result<PrivacyFcmpAccumulatorStateV1, String> {
        let state = self
            .accumulator_state
            .fcmp()
            .ok_or_else(|| "proof-managed pool has no FCMP++ curve frontier".to_owned())?;
        state
            .advance(&self.bootstrap, outputs)
            .map_err(str::to_owned)
    }

    #[cfg(test)]
    pub(crate) fn canonical_fcmp_bootstrap_for_test(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
    ) -> Self {
        assert_eq!(
            bootstrap.protocol_id(),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            "test helper accepts only FCMP++ bootstraps"
        );
        bootstrap.validate().expect("canonical FCMP++ bootstrap");
        let namespace = bootstrap.namespace();
        let bootstrap_digest = bootstrap.digest().expect("FCMP++ bootstrap digest");
        let initial_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .expect("FCMP++ origin root");
        let accumulator_state =
            PrivacyProofManagedPoolAccumulatorStateV1::bootstrap(&bootstrap, bootstrap_digest)
                .expect("FCMP++ origin frontier");
        let output_count = u64::try_from(
            bootstrap
                .initial_fcmp_outputs()
                .expect("FCMP++ genesis outputs")
                .len(),
        )
        .expect("test output count");
        let provenance = PrivacyRootProvenanceV1::proof_managed_pool_bootstrap(
            bootstrap_digest,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            1,
        )
        .expect("FCMP++ origin provenance");
        let root_key =
            PrivacyRootKeyV1::new(namespace, PrivacyRootRoleV1::OutputSet, 1, initial_root)
                .expect("FCMP++ origin root key");
        Self {
            namespace,
            root_role: PrivacyRootRoleV1::OutputSet,
            bootstrap,
            bootstrap_digest,
            initial_root,
            accumulator_state,
            output_count,
            bootstrap_admitted_at_height: 1,
            current_epoch: 1,
            current_root: initial_root,
            retention_anchor: None,
            retained_roots: vec![(root_key, provenance)],
            verified_batches: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_fcmp_successor_for_test(
        &self,
        outputs: &[PrivacyFcmpOutputTupleV1],
    ) -> Self {
        let successor = self
            .derive_fcmp_successor(outputs)
            .expect("canonical FCMP++ test successor");
        let next_root = successor.root().history_commitment();
        let provenance = PrivacyRootProvenanceV1::proof_managed_pool_successor(
            self.bootstrap_digest,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyStatementDigestV1::new([0xA5; 32]),
            1,
            u32::try_from(outputs.len()).expect("test output count"),
            2,
            0,
            self.current_epoch,
            self.current_root,
        )
        .expect("FCMP++ successor provenance");
        let root_key = PrivacyRootKeyV1::new(
            self.namespace,
            PrivacyRootRoleV1::OutputSet,
            successor.epoch(),
            next_root,
        )
        .expect("FCMP++ successor root key");
        let mut snapshot = self.clone();
        snapshot.accumulator_state = PrivacyProofManagedPoolAccumulatorStateV1::Fcmp(successor);
        snapshot.output_count = snapshot
            .output_count
            .checked_add(u64::try_from(outputs.len()).expect("test output count"))
            .expect("test output count does not overflow");
        snapshot.current_epoch = root_key.epoch();
        snapshot.current_root = root_key.root();
        snapshot.retained_roots.push((root_key, provenance));
        snapshot.verified_batches.insert(
            root_key.epoch(),
            ProofManagedVerifiedBatchOriginV1 {
                statement_digest: PrivacyStatementDigestV1::new([0xA5; 32]),
                admitted_at_height: 2,
                action_index: 0,
                nullifier_count: 1,
                output_count: u32::try_from(outputs.len()).expect("test output count"),
            },
        );
        snapshot
    }

    #[cfg(test)]
    fn canonical_note_bootstrap_for_test(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        protocol_id: PrivacyProtocolIdV1,
    ) -> Self {
        assert_eq!(
            bootstrap.protocol_id(),
            protocol_id,
            "test helper received a different private-note protocol"
        );
        bootstrap
            .validate()
            .expect("canonical private-note bootstrap");
        let namespace = bootstrap.namespace();
        let root_role = bootstrap.root_role();
        let bootstrap_digest = bootstrap.digest().expect("private-note bootstrap digest");
        let initial_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .expect("private-note origin root");
        let accumulator_state =
            PrivacyProofManagedPoolAccumulatorStateV1::bootstrap(&bootstrap, bootstrap_digest)
                .expect("private-note origin frontier");
        let output_count = u64::try_from(
            bootstrap
                .initial_note_commitments()
                .expect("private-note genesis commitments")
                .len(),
        )
        .expect("test output count");
        let provenance =
            PrivacyRootProvenanceV1::proof_managed_pool_bootstrap(bootstrap_digest, protocol_id, 1)
                .expect("private-note origin provenance");
        let root_key = PrivacyRootKeyV1::new(namespace, root_role, 1, initial_root)
            .expect("private-note origin root key");
        Self {
            namespace,
            root_role,
            bootstrap,
            bootstrap_digest,
            initial_root,
            accumulator_state,
            output_count,
            bootstrap_admitted_at_height: 1,
            current_epoch: 1,
            current_root: initial_root,
            retention_anchor: None,
            retained_roots: vec![(root_key, provenance)],
            verified_batches: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn canonical_private_note_bootstrap_for_test(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
    ) -> Self {
        Self::canonical_note_bootstrap_for_test(
            bootstrap,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        )
    }

    #[cfg(test)]
    pub(crate) fn canonical_pq_masp_bootstrap_for_test(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
    ) -> Self {
        Self::canonical_note_bootstrap_for_test(bootstrap, PrivacyProtocolIdV1::PqMaspStarkV0)
    }

    #[cfg(test)]
    fn with_note_successor_for_test(
        &self,
        outputs: &[PrivacyCommitmentV1],
        protocol_id: PrivacyProtocolIdV1,
    ) -> Self {
        assert_eq!(
            self.bootstrap.protocol_id(),
            protocol_id,
            "test helper received a different private-note protocol"
        );
        let successor = self
            .derive_note_successor(outputs)
            .expect("canonical private-note test successor");
        let next_root = successor.root();
        let provenance = PrivacyRootProvenanceV1::proof_managed_pool_successor(
            self.bootstrap_digest,
            protocol_id,
            PrivacyStatementDigestV1::new([0xA6; 32]),
            1,
            u32::try_from(outputs.len()).expect("test output count"),
            2,
            0,
            self.current_epoch,
            self.current_root,
        )
        .expect("private-note successor provenance");
        let root_key =
            PrivacyRootKeyV1::new(self.namespace, self.root_role, successor.epoch(), next_root)
                .expect("private-note successor root key");
        let mut snapshot = self.clone();
        snapshot.accumulator_state =
            PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(successor);
        snapshot.output_count = snapshot
            .output_count
            .checked_add(u64::try_from(outputs.len()).expect("test output count"))
            .expect("test output count does not overflow");
        snapshot.current_epoch = root_key.epoch();
        snapshot.current_root = root_key.root();
        snapshot.retained_roots.push((root_key, provenance));
        snapshot.verified_batches.insert(
            root_key.epoch(),
            ProofManagedVerifiedBatchOriginV1 {
                statement_digest: PrivacyStatementDigestV1::new([0xA6; 32]),
                admitted_at_height: 2,
                action_index: 0,
                nullifier_count: 1,
                output_count: u32::try_from(outputs.len()).expect("test output count"),
            },
        );
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn with_private_note_successor_for_test(
        &self,
        outputs: &[PrivacyCommitmentV1],
    ) -> Self {
        self.with_note_successor_for_test(outputs, PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
    }

    #[cfg(test)]
    pub(crate) fn with_pq_masp_successor_for_test(&self, outputs: &[PrivacyCommitmentV1]) -> Self {
        self.with_note_successor_for_test(outputs, PrivacyProtocolIdV1::PqMaspStarkV0)
    }

    #[cfg(test)]
    pub(crate) fn without_retained_current_root_for_test(&self) -> Self {
        self.without_retained_root_for_test(self.current_epoch, self.current_root)
    }

    #[cfg(test)]
    pub(crate) fn without_retained_root_for_test(&self, epoch: u64, root: PrivacyRootV1) -> Self {
        let mut snapshot = self.clone();
        snapshot
            .retained_roots
            .retain(|(key, _)| key.epoch() != epoch || key.root() != root);
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn with_namespace_for_test(&self, namespace: PrivacyNamespaceV1) -> Self {
        let mut snapshot = self.clone();
        snapshot.namespace = namespace;
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn with_root_role_for_test(&self, root_role: PrivacyRootRoleV1) -> Self {
        let mut snapshot = self.clone();
        snapshot.root_role = root_role;
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn with_inconsistent_fcmp_output_count_for_test(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.output_count = snapshot
            .output_count
            .checked_add(1)
            .expect("test output count does not overflow");
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn with_inconsistent_note_output_count_for_test(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.output_count = snapshot
            .output_count
            .checked_add(1)
            .expect("test output count does not overflow");
        snapshot
    }
}

fn validate_proof_managed_pool_successor_link_v1(
    namespace: PrivacyNamespaceV1,
    key: PrivacyRootKeyV1,
    provenance: PrivacyRootProvenanceV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
) -> Result<(u64, PrivacyRootV1), String> {
    let protocol_id = namespace.protocol_id();
    let PrivacyRootProvenanceV1::ProofManagedPoolSuccessor {
        bootstrap_digest: observed_bootstrap_digest,
        protocol_id: observed_protocol_id,
        parent_epoch,
        parent_root,
        ..
    } = provenance
    else {
        return Err(format!(
            "proof-managed pool history {namespace:?} contains a non-successor advancement"
        ));
    };
    if observed_bootstrap_digest != bootstrap_digest || observed_protocol_id != protocol_id {
        return Err(format!(
            "proof-managed pool successor {} is bound to a different bootstrap or protocol",
            key.epoch()
        ));
    }
    if parent_epoch.checked_add(1) != Some(key.epoch()) {
        return Err(format!(
            "proof-managed pool successor {} does not advance parent epoch {parent_epoch} by exactly one",
            key.epoch()
        ));
    }
    Ok((parent_epoch, parent_root))
}

fn validate_proof_managed_pool_retained_root_chain_v1(
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    initial_root: PrivacyRootV1,
    retained_root_count: usize,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    history: &[(PrivacyRootKeyV1, PrivacyRootProvenanceV1)],
) -> Result<(), String> {
    const INITIAL_EPOCH: u64 = 1;
    if retained_root_count == 0 {
        return Err("proof-managed pool retained-root count must be non-zero".to_owned());
    }
    if history.is_empty() {
        return Err("proof-managed pool has no retained root history".to_owned());
    }
    if history.len() > retained_root_count {
        return Err(format!(
            "proof-managed pool root history exceeds retention {retained_root_count}"
        ));
    }
    let protocol_id = namespace.protocol_id();
    let (first_key, first_provenance) = history[0];
    match first_provenance {
        PrivacyRootProvenanceV1::ProofManagedPoolBootstrap {
            bootstrap_digest: observed_bootstrap_digest,
            protocol_id: observed_protocol_id,
            ..
        } => {
            if observed_bootstrap_digest != bootstrap_digest || observed_protocol_id != protocol_id
            {
                return Err(
                    "proof-managed pool root origin differs from its typed bootstrap".to_owned(),
                );
            }
            if retention_anchor.is_some() {
                return Err(
                    "retained proof-managed bootstrap history has an unexpected prefix anchor"
                        .to_owned(),
                );
            }
            if first_key.epoch() != INITIAL_EPOCH || first_key.root() != initial_root {
                return Err(
                    "proof-managed pool origin is not its canonical epoch-one native root"
                        .to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::ProofManagedPoolSuccessor { .. } => {
            let (parent_epoch, parent_root) = validate_proof_managed_pool_successor_link_v1(
                namespace,
                first_key,
                first_provenance,
                bootstrap_digest,
            )?;
            if first_key.epoch() <= INITIAL_EPOCH {
                return Err(
                    "pruned proof-managed history begins at or before its canonical origin"
                        .to_owned(),
                );
            }
            let anchor = retention_anchor.ok_or_else(|| {
                "pruned proof-managed history has no exact retention anchor".to_owned()
            })?;
            if anchor.epoch().checked_add(1) != Some(first_key.epoch())
                || parent_epoch != anchor.epoch()
                || parent_root != anchor.root()
            {
                return Err(
                    "first retained proof-managed successor does not consume its exact prefix anchor"
                        .to_owned(),
                );
            }
            if anchor.epoch() == INITIAL_EPOCH && anchor.root() != initial_root {
                return Err(
                    "proof-managed prefix anchor substitutes the canonical initial root".to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::Governance { .. }
        | PrivacyRootProvenanceV1::ZkX509CaGovernance { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. }
        | PrivacyRootProvenanceV1::OrchardPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::OrchardPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::VerifiedBootstrap { .. }
        | PrivacyRootProvenanceV1::VerifiedProof { .. }
        | PrivacyRootProvenanceV1::VerifiedPgcSuccessor { .. } => {
            return Err(
                "proof-managed pool retained history begins with invalid provenance".to_owned(),
            );
        }
    }

    for adjacent in history.windows(2) {
        let (parent_key, _) = adjacent[0];
        let (child_key, child_provenance) = adjacent[1];
        let (declared_parent_epoch, declared_parent_root) =
            validate_proof_managed_pool_successor_link_v1(
                namespace,
                child_key,
                child_provenance,
                bootstrap_digest,
            )?;
        if parent_key.epoch().checked_add(1) != Some(child_key.epoch())
            || declared_parent_epoch != parent_key.epoch()
            || declared_parent_root != parent_key.root()
        {
            return Err(format!(
                "proof-managed pool history has a gap or forged parent between epochs {} and {}",
                parent_key.epoch(),
                child_key.epoch()
            ));
        }
    }
    if retention_anchor.is_some() && history.len() != retained_root_count {
        return Err(format!(
            "anchored proof-managed history has {} roots but must fill retention {retained_root_count}",
            history.len()
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofManagedCommitmentOriginKindV1 {
    Bootstrap,
    Verified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProofManagedCommitmentOriginV1 {
    kind: ProofManagedCommitmentOriginKindV1,
    position: u64,
    statement_digest: Option<PrivacyStatementDigestV1>,
    successor_epoch: Option<u64>,
    output_index: Option<u32>,
    nullifier_count: Option<u32>,
    output_count: Option<u32>,
    admitted_at_height: u64,
    action_index: Option<u32>,
}

impl ProofManagedCommitmentOriginV1 {
    const fn bootstrap(position: u64, admitted_at_height: u64) -> Self {
        Self {
            kind: ProofManagedCommitmentOriginKindV1::Bootstrap,
            position,
            statement_digest: None,
            successor_epoch: None,
            output_index: None,
            nullifier_count: None,
            output_count: None,
            admitted_at_height,
            action_index: None,
        }
    }

    const fn verified(
        statement_digest: PrivacyStatementDigestV1,
        successor_epoch: u64,
        output_index: u32,
        append_position: u64,
        nullifier_count: u32,
        output_count: u32,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Self {
        Self {
            kind: ProofManagedCommitmentOriginKindV1::Verified,
            position: append_position,
            statement_digest: Some(statement_digest),
            successor_epoch: Some(successor_epoch),
            output_index: Some(output_index),
            nullifier_count: Some(nullifier_count),
            output_count: Some(output_count),
            admitted_at_height,
            action_index: Some(action_index),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OrderedProofManagedCommitmentV1 {
    position: u64,
    commitment: PrivacyCommitmentV1,
    origin: ProofManagedCommitmentOriginV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OrderedFcmpOutputV1 {
    position: u64,
    output: PrivacyFcmpOutputTupleV1,
    origin: ProofManagedCommitmentOriginV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProofManagedVerifiedBatchOriginV1 {
    statement_digest: PrivacyStatementDigestV1,
    admitted_at_height: u64,
    action_index: u32,
    nullifier_count: u32,
    output_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProofManagedOriginSequenceV1 {
    last_epoch: u64,
    verified_batches: BTreeMap<u64, ProofManagedVerifiedBatchOriginV1>,
}

fn validate_proof_managed_origin_sequence_v1(
    origins: &[ProofManagedCommitmentOriginV1],
    genesis_count: usize,
    bootstrap_admitted_at_height: u64,
    max_outputs: u32,
    max_nullifiers: u32,
) -> Result<ProofManagedOriginSequenceV1, String> {
    if origins.len() < genesis_count {
        return Err("proof-managed pool omits a canonical genesis output".to_owned());
    }
    for (index, origin) in origins.iter().copied().enumerate() {
        let expected_position = u64::try_from(index)
            .map_err(|_| "proof-managed output position cannot be represented".to_owned())?;
        if origin.position != expected_position {
            return Err("proof-managed output order has a duplicate position or a gap".to_owned());
        }
        if index < genesis_count
            && origin
                != ProofManagedCommitmentOriginV1::bootstrap(
                    expected_position,
                    bootstrap_admitted_at_height,
                )
        {
            return Err("proof-managed genesis output provenance is not canonical".to_owned());
        }
        if index >= genesis_count && origin.kind == ProofManagedCommitmentOriginKindV1::Bootstrap {
            return Err(
                "proof-managed bootstrap output appears after the canonical genesis prefix"
                    .to_owned(),
            );
        }
    }

    let mut observed_last_epoch = 1_u64;
    let mut expected_output_index = 0_u32;
    let mut current_origin = None;
    let mut verified_batches = BTreeMap::new();
    for origin in &origins[genesis_count..] {
        if origin.kind != ProofManagedCommitmentOriginKindV1::Verified {
            return Err("proof-managed post-genesis output has governance provenance".to_owned());
        }
        let (
            Some(statement_digest),
            Some(successor_epoch),
            Some(output_index),
            Some(nullifier_count),
            Some(output_count),
            Some(action_index),
        ) = (
            origin.statement_digest,
            origin.successor_epoch,
            origin.output_index,
            origin.nullifier_count,
            origin.output_count,
            origin.action_index,
        )
        else {
            return Err("proof-managed verified output has incomplete typed provenance".to_owned());
        };
        if nullifier_count == 0 || nullifier_count > max_nullifiers {
            return Err(
                "proof-managed verified output nullifier count is outside its native bound"
                    .to_owned(),
            );
        }
        if output_count == 0 || output_count > max_outputs {
            return Err(
                "proof-managed verified output count is outside its native bound".to_owned(),
            );
        }
        let batch_origin = ProofManagedVerifiedBatchOriginV1 {
            statement_digest,
            admitted_at_height: origin.admitted_at_height,
            action_index,
            nullifier_count,
            output_count,
        };
        if successor_epoch
            == observed_last_epoch
                .checked_add(1)
                .ok_or_else(|| "proof-managed output epoch overflow".to_owned())?
        {
            observed_last_epoch = successor_epoch;
            expected_output_index = 0;
            current_origin = Some(batch_origin);
            if verified_batches
                .insert(successor_epoch, batch_origin)
                .is_some()
            {
                return Err("proof-managed output epochs contain a duplicate batch".to_owned());
            }
        } else if successor_epoch != observed_last_epoch {
            return Err("proof-managed output epochs contain a gap or reordering".to_owned());
        }
        if current_origin != Some(batch_origin)
            || output_index != expected_output_index
            || output_index >= max_outputs
        {
            return Err(
                "proof-managed output statement order or provenance is inconsistent".to_owned(),
            );
        }
        expected_output_index = expected_output_index
            .checked_add(1)
            .ok_or_else(|| "proof-managed output index overflow".to_owned())?;
    }
    let unique_origins = verified_batches
        .values()
        .map(|origin| {
            (
                origin.statement_digest,
                origin.admitted_at_height,
                origin.action_index,
            )
        })
        .collect::<BTreeSet<_>>();
    if unique_origins.len() != verified_batches.len() {
        return Err("proof-managed output batches replay verified provenance".to_owned());
    }
    let mut observed_output_counts = BTreeMap::<u64, u32>::new();
    for origin in &origins[genesis_count..] {
        let successor_epoch = origin
            .successor_epoch
            .expect("verified output completeness checked above");
        let count = observed_output_counts.entry(successor_epoch).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| "proof-managed output count overflow".to_owned())?;
    }
    for (epoch, batch) in &verified_batches {
        if observed_output_counts.get(epoch).copied() != Some(batch.output_count) {
            return Err(format!(
                "proof-managed output batch at epoch {epoch} declares {} outputs but restored {}",
                batch.output_count,
                observed_output_counts.get(epoch).copied().unwrap_or(0)
            ));
        }
    }
    Ok(ProofManagedOriginSequenceV1 {
        last_epoch: observed_last_epoch,
        verified_batches,
    })
}

/// Load and cross-check every authoritative component of one proof-managed pool.
pub(crate) fn load_privacy_proof_managed_pool_snapshot_v1(
    namespace: PrivacyNamespaceV1,
    retained_root_count: u32,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyProofManagedPoolSnapshotV1, String> {
    let root_role = proof_managed_pool_root_role_v1(namespace)
        .map_err(|error| format!("invalid proof-managed pool namespace: {error}"))?;
    if retained_root_count == 0 {
        return Err("proof-managed pool retained-root count must be non-zero".to_owned());
    }
    let retained_root_count = usize::try_from(retained_root_count)
        .map_err(|_| "proof-managed retained-root count cannot be represented".to_owned())?;

    let config_key = PrivacyCommitmentKeyV1::proof_managed_pool_config(namespace)
        .map_err(|error| format!("invalid proof-managed pool config key: {error}"))?;
    let config_record = commitments
        .get(&config_key)
        .ok_or_else(|| "proof-managed pool has no typed bootstrap configuration".to_owned())?;
    config_record
        .validate()
        .map_err(|error| format!("invalid proof-managed pool configuration: {error}"))?;
    let (
        bootstrap,
        bootstrap_digest,
        initial_root,
        accumulator_state,
        bootstrap_admitted_at_height,
    ) = config_record
        .proof_managed_pool_bootstrap_ref()
        .ok_or_else(|| "proof-managed pool config key has wrong-role provenance".to_owned())?;
    if bootstrap.namespace() != namespace {
        return Err("proof-managed pool config namespace differs from its bootstrap".to_owned());
    }
    let expected_digest = bootstrap.digest().map_err(|error| {
        format!("proof-managed pool bootstrap canonical encoding failed: {error}")
    })?;
    if bootstrap_digest != expected_digest {
        return Err("proof-managed pool config carries a substituted bootstrap digest".to_owned());
    }
    let expected_initial_root =
        crate::privacy_engines::proof_managed_pool_initial_root_v1(bootstrap).map_err(|error| {
            format!("proof-managed pool native root derivation failed: {error}")
        })?;
    if initial_root != expected_initial_root {
        return Err("proof-managed pool config carries a substituted initial root".to_owned());
    }

    let mut ordered_commitments = Vec::new();
    let mut ordered_fcmp_outputs = Vec::new();
    let (output_count, origin_sequence) = match namespace.protocol_id() {
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
            if commitments
                .range(PrivacyCommitmentKeyV1::proof_managed_pool_commitment_range(
                    namespace,
                ))
                .next()
                .is_some()
            {
                return Err("FCMP++ pool contains a foreign note-commitment key".to_owned());
            }
            let genesis_outputs = bootstrap
                .initial_fcmp_outputs()
                .ok_or_else(|| "FCMP++ bootstrap omits its complete genesis outputs".to_owned())?;
            let entry_count = commitments
                .range(PrivacyCommitmentKeyV1::fcmp_output_range(namespace))
                .count();
            ordered_fcmp_outputs
                .try_reserve_exact(entry_count)
                .map_err(|_| "FCMP++ output-order allocation failed".to_owned())?;
            for (key, record) in
                commitments.range(PrivacyCommitmentKeyV1::fcmp_output_range(namespace))
            {
                key.validate()
                    .map_err(|error| format!("invalid FCMP++ output key: {error}"))?;
                record
                    .validate()
                    .map_err(|error| format!("invalid FCMP++ output provenance: {error}"))?;
                let PrivacyCommitmentKeyV1::FcmpOutput { output_id, .. } = *key else {
                    return Err("FCMP++ output range returned a wrong-role key".to_owned());
                };
                let (output, position, origin, observed) = match record {
                    PrivacyStateItemRecordV1::FcmpBootstrapOutput {
                        bootstrap_digest: observed,
                        output,
                        position,
                        admitted_at_height,
                    } => (
                        *output,
                        *position,
                        ProofManagedCommitmentOriginV1::bootstrap(*position, *admitted_at_height),
                        *observed,
                    ),
                    PrivacyStateItemRecordV1::FcmpVerifiedOutput {
                        bootstrap_digest: observed,
                        output,
                        statement_digest,
                        successor_epoch,
                        output_index,
                        append_position,
                        nullifier_count,
                        output_count,
                        admitted_at_height,
                        action_index,
                    } => (
                        *output,
                        *append_position,
                        ProofManagedCommitmentOriginV1::verified(
                            *statement_digest,
                            *successor_epoch,
                            *output_index,
                            *append_position,
                            *nullifier_count,
                            *output_count,
                            *admitted_at_height,
                            *action_index,
                        ),
                        *observed,
                    ),
                    _ => {
                        return Err(
                            "FCMP++ output has wrong-role or cross-protocol provenance".to_owned()
                        );
                    }
                };
                if observed != bootstrap_digest || output.output_id() != output_id {
                    return Err(
                        "FCMP++ output key or provenance differs from its complete tuple"
                            .to_owned(),
                    );
                }
                ordered_fcmp_outputs.push(OrderedFcmpOutputV1 {
                    position,
                    output,
                    origin,
                });
            }
            ordered_fcmp_outputs.sort_unstable_by_key(|entry| entry.position);
            if ordered_fcmp_outputs.len() < genesis_outputs.len() {
                return Err("FCMP++ pool omits a canonical genesis output".to_owned());
            }
            for (entry, expected) in ordered_fcmp_outputs.iter().zip(genesis_outputs) {
                if entry.output != *expected {
                    return Err("FCMP++ genesis output order is not canonical".to_owned());
                }
            }
            let origins = ordered_fcmp_outputs
                .iter()
                .map(|entry| entry.origin)
                .collect::<Vec<_>>();
            let sequence = validate_proof_managed_origin_sequence_v1(
                &origins,
                genesis_outputs.len(),
                bootstrap_admitted_at_height,
                FCMP_MAX_OUTPUTS_V1,
                FCMP_MAX_INPUTS_V1,
            )?;
            (
                u64::try_from(entry_count)
                    .map_err(|_| "FCMP++ output count cannot be represented".to_owned())?,
                sequence,
            )
        }
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 | PrivacyProtocolIdV1::PqMaspStarkV0 => {
            if commitments
                .range(PrivacyCommitmentKeyV1::fcmp_output_range(namespace))
                .next()
                .is_some()
            {
                return Err("private-note pool contains a foreign FCMP++ output key".to_owned());
            }
            let genesis_commitments = bootstrap
                .initial_note_commitments()
                .ok_or_else(|| "private-note bootstrap omits its genesis commitments".to_owned())?;
            let entry_count = commitments
                .range(PrivacyCommitmentKeyV1::proof_managed_pool_commitment_range(
                    namespace,
                ))
                .count();
            ordered_commitments
                .try_reserve_exact(entry_count)
                .map_err(|_| "proof-managed commitment-order allocation failed".to_owned())?;
            for (key, record) in commitments.range(
                PrivacyCommitmentKeyV1::proof_managed_pool_commitment_range(namespace),
            ) {
                key.validate()
                    .map_err(|error| format!("invalid proof-managed commitment key: {error}"))?;
                record.validate().map_err(|error| {
                    format!("invalid proof-managed commitment provenance: {error}")
                })?;
                let PrivacyCommitmentKeyV1::ProofManagedPoolCommitment { commitment, .. } = *key
                else {
                    return Err(
                        "proof-managed commitment range returned a wrong-role key".to_owned()
                    );
                };
                let (position, origin, observed) = match record {
                    PrivacyStateItemRecordV1::ProofManagedPoolBootstrapCommitment {
                        bootstrap_digest,
                        position,
                        admitted_at_height,
                    } => (
                        *position,
                        ProofManagedCommitmentOriginV1::bootstrap(*position, *admitted_at_height),
                        *bootstrap_digest,
                    ),
                    PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment {
                        bootstrap_digest,
                        statement_digest,
                        successor_epoch,
                        output_index,
                        append_position,
                        nullifier_count,
                        output_count,
                        admitted_at_height,
                        action_index,
                    } => (
                        *append_position,
                        ProofManagedCommitmentOriginV1::verified(
                            *statement_digest,
                            *successor_epoch,
                            *output_index,
                            *append_position,
                            *nullifier_count,
                            *output_count,
                            *admitted_at_height,
                            *action_index,
                        ),
                        *bootstrap_digest,
                    ),
                    _ => {
                        return Err("private-note commitment has wrong-role provenance".to_owned());
                    }
                };
                if observed != bootstrap_digest {
                    return Err("private-note commitment has cross-bootstrap provenance".to_owned());
                }
                ordered_commitments.push(OrderedProofManagedCommitmentV1 {
                    position,
                    commitment,
                    origin,
                });
            }
            ordered_commitments.sort_unstable_by_key(|entry| entry.position);
            if ordered_commitments.len() < genesis_commitments.len() {
                return Err("proof-managed pool omits a canonical genesis commitment".to_owned());
            }
            for (entry, expected) in ordered_commitments.iter().zip(genesis_commitments) {
                if entry.commitment != *expected {
                    return Err(
                        "proof-managed genesis commitment order is not canonical".to_owned()
                    );
                }
            }
            let origins = ordered_commitments
                .iter()
                .map(|entry| entry.origin)
                .collect::<Vec<_>>();
            let (max_outputs, max_nullifiers) = match namespace.protocol_id() {
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => (
                    IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                    IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                ),
                PrivacyProtocolIdV1::PqMaspStarkV0 => {
                    (PQ_MASP_MAX_OUTPUTS_V1, PQ_MASP_MAX_INPUTS_V1)
                }
                _ => unreachable!("private-note branch checked above"),
            };
            let sequence = validate_proof_managed_origin_sequence_v1(
                &origins,
                genesis_commitments.len(),
                bootstrap_admitted_at_height,
                max_outputs,
                max_nullifiers,
            )?;
            (
                u64::try_from(entry_count).map_err(|_| {
                    "proof-managed commitment count cannot be represented".to_owned()
                })?,
                sequence,
            )
        }
        _ => return Err("proof-managed pool uses an unsupported protocol".to_owned()),
    };

    let head_key = PrivacyRootHeadKeyV1::new(namespace, root_role)
        .map_err(|error| format!("invalid proof-managed pool head key: {error}"))?;
    let head = root_heads
        .get(&head_key)
        .copied()
        .ok_or_else(|| "proof-managed pool has no current root head".to_owned())?;
    head.validate()
        .map_err(|error| format!("invalid proof-managed pool head: {error}"))?;

    let mut retained_roots = Vec::new();
    for (key, provenance) in roots.range(PrivacyRootKeyV1::history_range(namespace, root_role)) {
        if retained_roots.len() == retained_root_count {
            return Err(format!(
                "proof-managed pool root history exceeds retention {retained_root_count}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid proof-managed pool root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid proof-managed pool root provenance: {error}"))?;
        if retained_roots.last().is_some_and(
            |(previous, _): &(PrivacyRootKeyV1, PrivacyRootProvenanceV1)| {
                previous.epoch() == key.epoch()
            },
        ) {
            return Err(format!(
                "proof-managed pool root history contains duplicate epoch {}",
                key.epoch()
            ));
        }
        retained_roots.push((*key, *provenance));
    }
    validate_proof_managed_pool_retained_root_chain_v1(
        namespace,
        bootstrap_digest,
        initial_root,
        retained_root_count,
        head.retention_anchor(),
        &retained_roots,
    )?;
    for (key, provenance) in &retained_roots {
        match *provenance {
            PrivacyRootProvenanceV1::ProofManagedPoolBootstrap {
                admitted_at_height, ..
            } => {
                if admitted_at_height != bootstrap_admitted_at_height {
                    return Err(
                        "proof-managed bootstrap root admission height differs from its config"
                            .to_owned(),
                    );
                }
            }
            PrivacyRootProvenanceV1::ProofManagedPoolSuccessor {
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                ..
            } => {
                let Some(batch) = origin_sequence.verified_batches.get(&key.epoch()) else {
                    return Err(format!(
                        "proof-managed successor root epoch {} has no canonical output batch",
                        key.epoch()
                    ));
                };
                if batch.statement_digest != statement_digest
                    || batch.nullifier_count != nullifier_count
                    || batch.output_count != output_count
                    || batch.admitted_at_height != admitted_at_height
                    || batch.action_index != action_index
                {
                    return Err(format!(
                        "proof-managed successor root epoch {} differs from its canonical output-batch provenance",
                        key.epoch()
                    ));
                }
            }
            _ => {
                return Err(
                    "proof-managed pool retained history contains invalid provenance".to_owned(),
                );
            }
        }
    }
    let latest = retained_roots
        .last()
        .expect("non-empty proof-managed history checked above");
    if head.epoch() != latest.0.epoch()
        || head.root() != latest.0.root()
        || head.provenance() != latest.1
    {
        return Err("proof-managed pool head does not equal latest retained history".to_owned());
    }
    if head.provenance().proof_managed_pool_origin()
        != Some((bootstrap_digest, namespace.protocol_id()))
    {
        return Err("proof-managed pool head differs from its typed bootstrap".to_owned());
    }
    if origin_sequence.last_epoch != head.epoch() {
        return Err("proof-managed output epochs do not terminate at the current head".to_owned());
    }
    accumulator_state
        .validate_against_bootstrap(bootstrap, bootstrap_digest, initial_root)
        .map_err(|error| format!("proof-managed native frontier is invalid: {error}"))?;
    match namespace.protocol_id() {
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 | PrivacyProtocolIdV1::PqMaspStarkV0 => {
            let state = accumulator_state.private_note().ok_or_else(|| {
                "proof-managed private-note pool has no compact frontier".to_owned()
            })?;
            if state.namespace() != namespace
                || state.epoch() != head.epoch()
                || state.root() != head.root()
                || state.tree_size() != output_count
            {
                return Err(
                    "proof-managed private-note compact frontier differs from its current head or commitment set"
                        .to_owned(),
                );
            }
            let mut ordered_values = Vec::new();
            ordered_values
                .try_reserve_exact(ordered_commitments.len())
                .map_err(|_| "proof-managed private-note rebuild allocation failed".to_owned())?;
            ordered_values.extend(ordered_commitments.iter().map(|entry| entry.commitment));
            let rebuilt =
                crate::privacy_engines::proof_managed_accumulator::build_proof_managed_frontier_v1(
                    namespace,
                    &ordered_values,
                )
                .map_err(|_| {
                    "proof-managed private-note commitment order cannot rebuild its frontier"
                        .to_owned()
                })?;
            if rebuilt.tree_size != state.tree_size
                || rebuilt.root != state.root
                || rebuilt.leaf != state.leaf
                || rebuilt.ommers != state.ommers
            {
                return Err(
                    "proof-managed private-note commitment order differs from its compact frontier"
                        .to_owned(),
                );
            }
        }
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
            let state = accumulator_state
                .fcmp()
                .ok_or_else(|| "FCMP++ pool has no curve-tree frontier".to_owned())?;
            if state.namespace() != namespace
                || state.epoch() != head.epoch()
                || state.root().history_commitment() != head.root()
                || state.tree_size() != output_count
            {
                return Err(
                    "FCMP++ curve frontier differs from its current head or output set".to_owned(),
                );
            }
            let native_outputs = ordered_fcmp_outputs
                .iter()
                .map(|entry| fcmp_output_to_native_v1(entry.output))
                .collect::<Result<Vec<_>, _>>()
                .map_err(str::to_owned)?;
            let rebuilt =
                crate::privacy_engines::fcmp_plus_plus::build_fcmp_frontier_v1(&native_outputs)
                    .map_err(|_| {
                        "FCMP++ complete output order cannot rebuild its curve frontier".to_owned()
                    })?;
            let rebuilt = PrivacyFcmpAccumulatorStateV1::from_native_parts(
                namespace,
                bootstrap_digest,
                state.epoch(),
                rebuilt,
            );
            if &rebuilt != state {
                return Err(
                    "FCMP++ complete output order differs from its compact curve frontier"
                        .to_owned(),
                );
            }
        }
        _ => {
            return Err("proof-managed pool uses an unsupported protocol".to_owned());
        }
    }

    Ok(PrivacyProofManagedPoolSnapshotV1 {
        namespace,
        root_role,
        bootstrap: bootstrap.clone(),
        bootstrap_digest,
        initial_root,
        accumulator_state: accumulator_state.clone(),
        output_count,
        bootstrap_admitted_at_height,
        current_epoch: head.epoch(),
        current_root: head.root(),
        retention_anchor: head.retention_anchor(),
        retained_roots,
        verified_batches: origin_sequence.verified_batches,
    })
}

/// Validate every cross-map invariant in restored first-release privacy state.
///
/// Snapshot decoding invokes this before constructing `World`. Consequently a
/// malformed key, unavailable activation, orphan state item, over-cap history,
/// duplicate root epoch, missing/inconsistent head, or PGC account/root
/// mismatch cannot enter consensus state.
pub(crate) fn validate_privacy_persisted_state_v1(
    policy: &PrivacyConsensusPolicyV1,
    activations: &impl StorageReadOnly<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
    pgc_accounts: &impl StorageReadOnly<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>,
    pgc_pool_invariants: &impl StorageReadOnly<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>,
    nullifiers: &impl StorageReadOnly<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<(), String> {
    policy
        .validate()
        .map_err(|error| format!("invalid privacy consensus policy: {error}"))?;
    plan_due_privacy_activation_promotions_v1(activations, 0)
        .map_err(|error| format!("invalid privacy activation registry: {error}"))?;

    let ensure_protocol_activation = |protocol_id: PrivacyProtocolIdV1| -> Result<(), String> {
        let key = PrivacyActivationKeyV1::new(protocol_id);
        if activations.get(&key).is_none() {
            return Err(format!(
                "privacy state references unregistered protocol {protocol_id:?}",
            ));
        }
        Ok(())
    };
    let ensure_activation = |namespace: PrivacyNamespaceV1| -> Result<(), String> {
        ensure_protocol_activation(namespace.protocol_id())
    };

    for (key, invariant) in pgc_pool_invariants.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy PGC invariant key: {error}"))?;
        invariant
            .validate()
            .map_err(|error| format!("invalid privacy PGC invariant: {error}"))?;
        ensure_activation(key.namespace())?;
    }

    let mut proof_managed_nullifier_counts =
        BTreeMap::<(PrivacyNamespaceV1, ProofManagedVerifiedBatchOriginV1), u32>::new();
    for (key, record) in nullifiers.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy nullifier key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid privacy nullifier provenance: {error}"))?;
        ensure_protocol_activation(key.protocol_id())?;
    }
    for (key, record) in commitments.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy commitment key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid privacy commitment provenance: {error}"))?;
        if key.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
            ensure_protocol_activation(key.protocol_id())?;
        }
    }
    let zk_x509_index = load_privacy_zk_x509_governance_index_v1(commitments)?;
    privacy_bootle_lantern_issuer_policy_count_v1(commitments)?;
    privacy_vega_issuer_record_count_v1(commitments)?;

    let mut history_by_scope = BTreeMap::<
        (PrivacyNamespaceV1, PrivacyRootRoleV1),
        Vec<(PrivacyRootKeyV1, PrivacyRootProvenanceV1)>,
    >::new();
    for (key, provenance) in roots.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid privacy root provenance: {error}"))?;
        if key.namespace().protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
            ensure_activation(key.namespace())?;
        }
        history_by_scope
            .entry((key.namespace(), key.role()))
            .or_default()
            .push((*key, *provenance));
    }

    let mut zk_ams_bootstraps =
        BTreeMap::<PrivacyNamespaceV1, PrivacyZkAmsRegistryBootstrapDigestV1>::new();
    let mut orchard_bootstraps =
        BTreeMap::<PrivacyNamespaceV1, PrivacyOrchardPoolBootstrapDigestV1>::new();
    let mut proof_managed_pools =
        BTreeMap::<PrivacyNamespaceV1, PrivacyProofManagedPoolSnapshotV1>::new();
    for ((namespace, role), history) in &history_by_scope {
        let retained_root_count =
            if namespace.protocol_id() == PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
                policy.current_limits.retained_root_count
            } else {
                policy.admission_retained_root_count()
            };
        let retained = usize::try_from(retained_root_count)
            .map_err(|_| "privacy retained-root count cannot be represented".to_owned())?;
        if history.len() > retained {
            return Err(format!(
                "privacy root history for {namespace:?}/{role:?} exceeds retention {retained}"
            ));
        }
        for pair in history.windows(2) {
            if pair[0].0.epoch() == pair[1].0.epoch() {
                return Err(format!(
                    "privacy root history for {namespace:?}/{role:?} contains duplicate epoch {}",
                    pair[0].0.epoch()
                ));
            }
        }
        let head_key = PrivacyRootHeadKeyV1::new(*namespace, *role)
            .map_err(|error| format!("invalid privacy root-head key: {error}"))?;
        let head = root_heads.get(&head_key).ok_or_else(|| {
            format!("privacy root history for {namespace:?}/{role:?} has no current head")
        })?;
        if *role == PrivacyRootRoleV1::PgcAccountState {
            let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(*namespace).map_err(|error| {
                format!("invalid privacy PGC invariant key for root history: {error}")
            })?;
            let invariant = pgc_pool_invariants
                .get(&invariant_key)
                .copied()
                .ok_or_else(|| {
                    format!("PGC privacy root history for {namespace:?} has no pool invariant")
                })?;
            validate_pgc_retained_root_chain_v1(
                *namespace,
                invariant,
                retained,
                head.retention_anchor(),
                history,
            )?;
        } else if namespace.protocol_id() == PrivacyProtocolIdV1::IrohaZkAmsV1
            && *role == PrivacyRootRoleV1::AccountRegistry
        {
            let snapshot = load_privacy_zk_ams_registry_snapshot_v1(
                *namespace,
                retained_root_count,
                commitments,
                roots,
                root_heads,
            )?;
            if zk_ams_bootstraps
                .insert(*namespace, snapshot.bootstrap_digest())
                .is_some()
            {
                return Err(format!(
                    "duplicate ZK-AMS AccountRegistry scope for {namespace:?}"
                ));
            }
        } else if namespace.protocol_id() == PrivacyProtocolIdV1::OrchardHalo2ActionsV1
            && *role == PrivacyRootRoleV1::NoteCommitmentAnchor
        {
            let snapshot = load_privacy_orchard_pool_snapshot_v1(
                *namespace,
                retained_root_count,
                commitments,
                roots,
                root_heads,
            )?;
            if orchard_bootstraps
                .insert(*namespace, snapshot.bootstrap_digest())
                .is_some()
            {
                return Err(format!(
                    "duplicate Orchard note-commitment scope for {namespace:?}"
                ));
            }
        } else if proof_managed_pool_root_role_v1(*namespace)
            .is_ok_and(|expected| expected == *role)
        {
            let snapshot = load_privacy_proof_managed_pool_snapshot_v1(
                *namespace,
                retained_root_count,
                commitments,
                roots,
                root_heads,
            )?;
            if proof_managed_pools.insert(*namespace, snapshot).is_some() {
                return Err(format!(
                    "duplicate proof-managed pool root scope for {namespace:?}"
                ));
            }
        } else if namespace.protocol_id() == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
            let Some((first_key, _)) = history.first() else {
                return Err("grouped X.509 root history is unexpectedly empty".to_owned());
            };
            if let Some(anchor) = head.retention_anchor() {
                if anchor.epoch().checked_add(1) != Some(first_key.epoch()) {
                    return Err(format!(
                        "X.509 first retained root for {namespace:?}/{role:?} does not immediately follow its retention anchor"
                    ));
                }
            } else if first_key.epoch() != 1 {
                return Err(format!(
                    "unpruned X.509 root history for {namespace:?}/{role:?} must begin at epoch one"
                ));
            }
            for (key, provenance) in history {
                validate_zk_x509_root_provenance_v1(*key, *provenance)?;
            }
            for adjacent in history.windows(2) {
                if adjacent[0].0.epoch().checked_add(1) != Some(adjacent[1].0.epoch()) {
                    return Err(format!(
                        "X.509 root history for {namespace:?}/{role:?} has a gap or duplicate epoch"
                    ));
                }
            }
            match role {
                PrivacyRootRoleV1::CertificateAuthorityMembership => {
                    let trust_anchor_id = zk_x509_ca_namespace_component_v1(*namespace)?;
                    let lineage = zk_x509_index
                        .trust_anchors
                        .get(&trust_anchor_id)
                        .ok_or_else(|| {
                            format!(
                                "X.509 CA-root history {namespace:?} has no trust-anchor lineage"
                            )
                        })?;
                    let records = history
                        .iter()
                        .map(|(_, provenance)| {
                            let PrivacyRootProvenanceV1::ZkX509CaGovernance {
                                trust_anchor_record,
                                ..
                            } = provenance
                            else {
                                return Err(
                                    "X.509 CA-root history has wrong-role provenance".to_owned()
                                );
                            };
                            if !lineage.contains(trust_anchor_record) {
                                return Err(
                                    "X.509 CA-root provenance embeds an unregistered trust-anchor revision"
                                        .to_owned(),
                                );
                            }
                            Ok(*trust_anchor_record)
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    if head.retention_anchor().is_none() {
                        records[0].validate_initial().map_err(|error| {
                            format!("invalid X.509 CA-root history origin: {error}")
                        })?;
                    }
                    for adjacent in records.windows(2) {
                        validate_zk_x509_trust_anchor_rotation_v1(&adjacent[0], &adjacent[1])
                            .map_err(|error| {
                                format!("invalid X.509 CA-root history transition: {error}")
                            })?;
                    }
                }
                PrivacyRootRoleV1::PgcAccountState
                | PrivacyRootRoleV1::AccountRegistry
                | PrivacyRootRoleV1::Revocation
                | PrivacyRootRoleV1::NoteCommitmentAnchor
                | PrivacyRootRoleV1::OutputSet
                | PrivacyRootRoleV1::ProgramState => {
                    return Err(format!(
                        "X.509 root history {namespace:?} uses incompatible role {role:?}"
                    ));
                }
            }
        } else {
            if head.retention_anchor().is_some() {
                return Err(format!(
                    "non-PGC privacy root head for {namespace:?}/{role:?} carries an unsupported retention anchor"
                ));
            }
            match role.management() {
                PrivacyRootManagementV1::GovernanceManaged => {
                    if history.iter().any(|(_, provenance)| {
                        !matches!(provenance, PrivacyRootProvenanceV1::Governance { .. })
                    }) {
                        return Err(format!(
                            "governance-managed privacy root history for {namespace:?}/{role:?} contains non-governance provenance"
                        ));
                    }
                }
                PrivacyRootManagementV1::ProofManaged => {
                    let Some((_, first_provenance)) = history.first() else {
                        return Err("grouped privacy root history is unexpectedly empty".to_owned());
                    };
                    let valid_origin = if namespace.protocol_id()
                        == PrivacyProtocolIdV1::IrohaZkAmsV1
                        && *role == PrivacyRootRoleV1::AccountRegistry
                    {
                        matches!(
                            first_provenance,
                            PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
                        )
                    } else {
                        matches!(first_provenance, PrivacyRootProvenanceV1::Governance { .. })
                    };
                    if !valid_origin {
                        return Err(format!(
                            "proof-managed privacy root history for {namespace:?}/{role:?} does not begin with governance initialization"
                        ));
                    }
                    if history.iter().skip(1).any(|(_, provenance)| {
                        !matches!(provenance, PrivacyRootProvenanceV1::VerifiedProof { .. })
                    }) {
                        return Err(format!(
                            "proof-managed privacy root history for {namespace:?}/{role:?} contains a non-proof advancement"
                        ));
                    }
                    for adjacent in history.windows(2) {
                        if adjacent[1].1.proof_parent()
                            != Some((adjacent[0].0.epoch(), adjacent[0].0.root()))
                            || adjacent[0].0.epoch().checked_add(1) != Some(adjacent[1].0.epoch())
                        {
                            return Err(format!(
                                "proof-managed privacy root history for {namespace:?}/{role:?} has a gap or forged parent"
                            ));
                        }
                    }
                }
            }
        }
        let latest = history.last().expect("grouped history is non-empty");
        if head.epoch() != latest.0.epoch()
            || head.root() != latest.0.root()
            || head.provenance() != latest.1
        {
            return Err(format!(
                "privacy root head for {namespace:?}/{role:?} does not equal latest history entry"
            ));
        }
    }
    for (key, head) in root_heads.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy root-head key: {error}"))?;
        head.validate()
            .map_err(|error| format!("invalid privacy root-head record: {error}"))?;
        if key.namespace().protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
            ensure_activation(key.namespace())?;
        }
        if !history_by_scope.contains_key(&(key.namespace(), key.role())) {
            return Err(format!(
                "privacy root head for {:?}/{:?} has no retained history",
                key.namespace(),
                key.role()
            ));
        }
    }
    let x509_retained_root_count = policy.admission_retained_root_count();
    for lineage in zk_x509_index.trust_anchors.values() {
        let last_active = lineage
            .iter()
            .rev()
            .find(|record| record.lifecycle == PrivacyZkX509RecordLifecycleV1::Active)
            .copied()
            .expect("validated X.509 trust-anchor lineage begins active");
        validate_privacy_zk_x509_trust_anchor_root_state_v1(
            last_active,
            x509_retained_root_count,
            roots,
            root_heads,
        )?;
    }
    let mut zk_ace_policy_ids = BTreeSet::new();
    for (key, record) in commitments.iter() {
        match key {
            PrivacyCommitmentKeyV1::ZkAcePolicy { policy_id } => {
                let PrivacyStateItemRecordV1::ZkAcePolicyGovernance { policy, .. } = record else {
                    return Err(format!(
                        "ZK-ACE policy {policy_id:?} has wrong-role provenance"
                    ));
                };
                if policy.policy_id != *policy_id {
                    return Err(format!(
                        "ZK-ACE policy key {policy_id:?} does not match its record"
                    ));
                }
                zk_ace_policy_ids.insert(*policy_id);
                if zk_ace_policy_ids.len() > PRIVACY_ZK_ACE_MAX_POLICIES_V1 {
                    return Err(format!(
                        "ZK-ACE policy count exceeds {}",
                        PRIVACY_ZK_ACE_MAX_POLICIES_V1
                    ));
                }
            }
            PrivacyCommitmentKeyV1::BootleLanternIssuerPolicy {
                issuer_id,
                policy_id,
            } => {
                let PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance {
                    policy, ..
                } = record
                else {
                    return Err(format!(
                        "Bootle/Lantern issuer policy {issuer_id:?}/{policy_id:?} has wrong-role provenance"
                    ));
                };
                if policy.issuer_id != *issuer_id || policy.policy_id != *policy_id {
                    return Err(format!(
                        "Bootle/Lantern issuer-policy key {issuer_id:?}/{policy_id:?} does not match its record"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::VegaIssuerRevision {
                issuer_id,
                record_epoch,
            } => {
                let PrivacyStateItemRecordV1::VegaIssuerGovernance { record, .. } = record else {
                    return Err(format!(
                        "Vega issuer revision {issuer_id:?}/{record_epoch} has wrong-role provenance"
                    ));
                };
                if record.issuer_id != *issuer_id || record.record_epoch != *record_epoch {
                    return Err(format!(
                        "Vega issuer revision key {issuer_id:?}/{record_epoch} does not match its record"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::OrchardPoolState { namespace } => {
                let bootstrap_digest = orchard_bootstraps.get(namespace).ok_or_else(|| {
                    format!(
                        "Orchard pool state {namespace:?} has no authoritative note-commitment history"
                    )
                })?;
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::OrchardPoolState { state }
                        if state.bootstrap_digest() == *bootstrap_digest
                ) {
                    return Err(format!(
                        "Orchard pool state {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::ProofManagedPoolConfig { namespace } => {
                let pool = proof_managed_pools.get(namespace).ok_or_else(|| {
                    format!(
                        "proof-managed pool config {namespace:?} has no authoritative root history"
                    )
                })?;
                let bootstrap_digest = pool.bootstrap_digest();
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::ProofManagedPoolBootstrap {
                        bootstrap,
                        bootstrap_digest: observed,
                        ..
                    } if *observed == bootstrap_digest && bootstrap.namespace() == *namespace
                ) {
                    return Err(format!(
                        "proof-managed pool config {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::ProofManagedPoolCommitment { namespace, .. } => {
                let pool = proof_managed_pools.get(namespace).ok_or_else(|| {
                    format!(
                        "proof-managed commitment {namespace:?} has no authoritative root history"
                    )
                })?;
                if record.proof_managed_pool_bootstrap_digest() != Some(pool.bootstrap_digest())
                    || matches!(
                        record,
                        PrivacyStateItemRecordV1::ProofManagedPoolBootstrap { .. }
                    )
                {
                    return Err(format!(
                        "proof-managed commitment {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::FcmpOutput {
                namespace,
                output_id,
            } => {
                let pool = proof_managed_pools.get(namespace).ok_or_else(|| {
                    format!("FCMP++ output {namespace:?} has no authoritative output-set history")
                })?;
                let bootstrap_digest = pool.bootstrap_digest();
                let role_matches = match record {
                    PrivacyStateItemRecordV1::FcmpBootstrapOutput {
                        bootstrap_digest: observed,
                        output,
                        ..
                    }
                    | PrivacyStateItemRecordV1::FcmpVerifiedOutput {
                        bootstrap_digest: observed,
                        output,
                        ..
                    } => *observed == bootstrap_digest && output.output_id() == *output_id,
                    _ => false,
                };
                if !role_matches {
                    return Err(format!(
                        "FCMP++ output {namespace:?} has wrong-role, tuple, or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyCommitmentKeyV1::ZkX509TrustAnchorRevision { .. }
            | PrivacyCommitmentKeyV1::ZkX509CertificatePolicyRevision { .. }
            | PrivacyCommitmentKeyV1::ZkX509CrlCurrent { .. } => {}
            PrivacyCommitmentKeyV1::ZkAmsIssuerPolicyRecord { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsPhc { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsSeedKey { namespace, .. } => {
                let bootstrap_digest = zk_ams_bootstraps.get(namespace).ok_or_else(|| {
                    format!("ZK-AMS commitment {namespace:?} has no authoritative AccountRegistry")
                })?;
                let role_matches = match key {
                    PrivacyCommitmentKeyV1::ZkAmsIssuerPolicyRecord { .. } => matches!(
                        record,
                        PrivacyStateItemRecordV1::ZkAmsGovernance {
                            bootstrap_digest: observed,
                            ..
                        } if observed == bootstrap_digest
                    ),
                    PrivacyCommitmentKeyV1::ZkAmsPhc { .. }
                    | PrivacyCommitmentKeyV1::ZkAmsSeedKey { .. } => matches!(
                        record,
                        PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                            bootstrap_digest: observed,
                            ..
                        } if observed == bootstrap_digest
                    ),
                    PrivacyCommitmentKeyV1::ZkAcePolicy { .. }
                    | PrivacyCommitmentKeyV1::BootleLanternIssuerPolicy { .. }
                    | PrivacyCommitmentKeyV1::VegaIssuerRevision { .. }
                    | PrivacyCommitmentKeyV1::OrchardPoolState { .. }
                    | PrivacyCommitmentKeyV1::ProofManagedPoolConfig { .. }
                    | PrivacyCommitmentKeyV1::ProofManagedPoolCommitment { .. }
                    | PrivacyCommitmentKeyV1::FcmpOutput { .. }
                    | PrivacyCommitmentKeyV1::ZkX509TrustAnchorRevision { .. }
                    | PrivacyCommitmentKeyV1::ZkX509CertificatePolicyRevision { .. }
                    | PrivacyCommitmentKeyV1::ZkX509CrlCurrent { .. } => false,
                };
                if !role_matches {
                    return Err(format!(
                        "ZK-AMS commitment {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
        }
    }
    for (key, record) in nullifiers.iter() {
        match key {
            PrivacyNullifierKeyV1::ZkAceReplay { policy_id, .. } => {
                validate_privacy_zk_ace_replay_binding_v1(&zk_ace_policy_ids, *policy_id, record)?;
            }
            PrivacyNullifierKeyV1::ZkAmsKeyImage { namespace, .. } => {
                let bootstrap_digest = zk_ams_bootstraps.get(namespace).ok_or_else(|| {
                    format!("ZK-AMS nullifier {namespace:?} has no authoritative AccountRegistry")
                })?;
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                        bootstrap_digest: observed,
                        ..
                    } if observed == bootstrap_digest
                ) {
                    return Err(format!(
                        "ZK-AMS key image {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyNullifierKeyV1::ZkX509CertificateNullifier { namespace, .. } => {
                let (trust_anchor_id, policy_id) =
                    zk_x509_policy_namespace_components_v1(*namespace)?;
                if !zk_x509_index.trust_anchors.contains_key(&trust_anchor_id)
                    || !zk_x509_index
                        .certificate_policies
                        .contains_key(&(trust_anchor_id, policy_id))
                {
                    return Err(format!(
                        "X.509 certificate nullifier {namespace:?} has no governed policy lineage"
                    ));
                }
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier { .. }
                ) {
                    return Err(format!(
                        "X.509 certificate nullifier {namespace:?} has wrong-role provenance"
                    ));
                }
            }
            PrivacyNullifierKeyV1::OrchardNullifier { namespace, .. } => {
                let bootstrap_digest = orchard_bootstraps.get(namespace).ok_or_else(|| {
                    format!(
                        "Orchard nullifier {namespace:?} has no authoritative note-commitment pool"
                    )
                })?;
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::OrchardVerifiedNullifier {
                        bootstrap_digest: observed,
                        ..
                    } if observed == bootstrap_digest
                ) {
                    return Err(format!(
                        "Orchard nullifier {namespace:?} has wrong-role or cross-bootstrap provenance"
                    ));
                }
            }
            PrivacyNullifierKeyV1::ProofManagedNullifier { namespace, .. }
            | PrivacyNullifierKeyV1::FcmpKeyImage { namespace, .. } => {
                let pool = proof_managed_pools.get(namespace).ok_or_else(|| {
                    format!(
                        "proof-managed replay marker {namespace:?} has no authoritative pool history"
                    )
                })?;
                let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedNullifier {
                    bootstrap_digest,
                    statement_digest,
                    nullifier_count,
                    output_count,
                    admitted_at_height,
                    action_index,
                } = record
                else {
                    return Err(format!(
                        "proof-managed replay marker {namespace:?} has wrong-role provenance"
                    ));
                };
                let origin = ProofManagedVerifiedBatchOriginV1 {
                    statement_digest: *statement_digest,
                    admitted_at_height: *admitted_at_height,
                    action_index: *action_index,
                    nullifier_count: *nullifier_count,
                    output_count: *output_count,
                };
                if *bootstrap_digest != pool.bootstrap_digest()
                    || !pool.contains_verified_batch(origin)
                {
                    return Err(format!(
                        "proof-managed replay marker {namespace:?} has orphaned or mixed batch provenance"
                    ));
                }
                let count = proof_managed_nullifier_counts
                    .entry((*namespace, origin))
                    .or_default();
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| "proof-managed replay-marker count overflow".to_owned())?;
            }
        }
    }
    for (namespace, pool) in &proof_managed_pools {
        for origin in pool.verified_batches.values().copied() {
            let observed = proof_managed_nullifier_counts
                .get(&(*namespace, origin))
                .copied()
                .unwrap_or(0);
            if observed != origin.nullifier_count {
                return Err(format!(
                    "proof-managed batch in {namespace:?} declares {} replay markers but restored {observed}",
                    origin.nullifier_count
                ));
            }
        }
    }

    let mut pgc_by_namespace = BTreeMap::<PrivacyNamespaceV1, Vec<PrivacyPgcAccountV1>>::new();
    let mut pgc_epoch_by_namespace = BTreeMap::<PrivacyNamespaceV1, u64>::new();
    let mut pgc_provenance_by_namespace =
        BTreeMap::<PrivacyNamespaceV1, PrivacyPgcAccountProvenanceV1>::new();
    for (key, state) in pgc_accounts.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy PGC account key: {error}"))?;
        state
            .validate()
            .map_err(|error| format!("invalid privacy PGC account state: {error}"))?;
        ensure_activation(key.namespace())?;
        if let Some(epoch) = pgc_epoch_by_namespace.insert(key.namespace(), state.epoch())
            && epoch != state.epoch()
        {
            return Err(format!(
                "privacy PGC account table {:?} contains mixed epochs",
                key.namespace()
            ));
        }
        if let Some(provenance) =
            pgc_provenance_by_namespace.insert(key.namespace(), state.provenance)
            && provenance != state.provenance
        {
            return Err(format!(
                "privacy PGC account table {:?} contains mixed provenance",
                key.namespace()
            ));
        }
        pgc_by_namespace
            .entry(key.namespace())
            .or_default()
            .push(PrivacyPgcAccountV1 {
                public_key: key.public_key(),
                encrypted_balance: state.encrypted_balance(),
            });
    }
    for (namespace, accounts) in &pgc_by_namespace {
        let epoch = pgc_epoch_by_namespace[namespace];
        let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(*namespace)
            .map_err(|error| format!("invalid privacy PGC invariant key: {error}"))?;
        let invariant = pgc_pool_invariants.get(&invariant_key).ok_or_else(|| {
            format!("privacy PGC account table {namespace:?} has no pool invariant")
        })?;
        if let PrivacyPgcAccountProvenanceV1::Bootstrap {
            bootstrap_digest,
            bootstrap_proof_digest,
            ..
        } = pgc_provenance_by_namespace[namespace]
            && (bootstrap_digest != invariant.bootstrap_digest()
                || bootstrap_proof_digest != invariant.bootstrap_proof_digest())
        {
            return Err(format!(
                "privacy PGC account table {namespace:?} bootstrap provenance differs from its pool invariant"
            ));
        }
        let head_key = PrivacyRootHeadKeyV1::new(*namespace, PrivacyRootRoleV1::PgcAccountState)
            .map_err(|error| format!("invalid privacy PGC root-head key: {error}"))?;
        let head = root_heads
            .get(&head_key)
            .ok_or_else(|| format!("privacy PGC account table {namespace:?} has no root head"))?;
        if head.epoch() != epoch {
            return Err(format!(
                "privacy PGC account table {namespace:?} epoch differs from its root head"
            ));
        }
        let account_provenance = pgc_provenance_by_namespace[namespace];
        let provenance_matches = match (account_provenance, head.provenance()) {
            (
                PrivacyPgcAccountProvenanceV1::Bootstrap {
                    bootstrap_digest: account_bootstrap_digest,
                    bootstrap_proof_digest: account_proof_digest,
                    admitted_at_height: account_height,
                },
                PrivacyRootProvenanceV1::VerifiedBootstrap {
                    bootstrap_digest: root_bootstrap_digest,
                    bootstrap_proof_digest: root_proof_digest,
                    admitted_at_height: root_height,
                },
            ) => {
                account_bootstrap_digest == root_bootstrap_digest
                    && account_proof_digest == root_proof_digest
                    && account_height == root_height
            }
            (
                PrivacyPgcAccountProvenanceV1::VerifiedProof {
                    statement_digest: account_statement_digest,
                    admitted_at_height: account_height,
                    action_index: account_action_index,
                },
                PrivacyRootProvenanceV1::VerifiedPgcSuccessor {
                    statement_digest: root_statement_digest,
                    admitted_at_height: root_height,
                    action_index: root_action_index,
                    ..
                },
            ) => {
                account_statement_digest == root_statement_digest
                    && account_height == root_height
                    && account_action_index == root_action_index
            }
            _ => false,
        };
        if !provenance_matches {
            return Err(format!(
                "privacy PGC account table {namespace:?} provenance differs from its root head"
            ));
        }
        let computed = compute_privacy_pgc_account_state_root_v1(
            *namespace,
            epoch,
            invariant.total_supply(),
            accounts,
        )
        .map_err(|error| format!("invalid privacy PGC account table: {error}"))?;
        if computed != head.root() {
            return Err(format!(
                "privacy PGC account table {namespace:?} does not match its root head"
            ));
        }
    }
    for (key, _) in pgc_pool_invariants.iter() {
        if !pgc_by_namespace.contains_key(&key.namespace()) {
            return Err(format!(
                "privacy PGC pool invariant {:?} has no encrypted account table",
                key.namespace()
            ));
        }
    }
    for (key, _) in root_heads.iter() {
        if key.role() == PrivacyRootRoleV1::PgcAccountState
            && !pgc_by_namespace.contains_key(&key.namespace())
        {
            return Err(format!(
                "privacy PGC root head {:?} has no encrypted account table",
                key.namespace()
            ));
        }
        if key.role() == PrivacyRootRoleV1::PgcAccountState {
            let invariant_key =
                PrivacyPgcPoolInvariantKeyV1::new(key.namespace()).map_err(|error| {
                    format!("invalid privacy PGC invariant key for root head: {error}")
                })?;
            if pgc_pool_invariants.get(&invariant_key).is_none() {
                return Err(format!(
                    "privacy PGC root head {:?} has no pool invariant",
                    key.namespace()
                ));
            }
        }
    }
    Ok(())
}

/// Complete authoritative compact state for one governed Orchard V3 pool.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub struct PrivacyOrchardPoolStateV1 {
    bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
    asset_definition_id: AssetDefinitionId,
    public_balance_scope: AssetBalanceScope,
    reserve_account: AccountId,
    epoch: u64,
    root: PrivacyRootV1,
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: Vec<[u8; 32]>,
}

impl PrivacyOrchardPoolStateV1 {
    /// Construct the sole empty-frontier origin for a governed Orchard pool.
    pub(crate) fn bootstrap(
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        asset_definition_id: AssetDefinitionId,
        public_balance_scope: AssetBalanceScope,
        reserve_account: AccountId,
    ) -> Result<Self, &'static str> {
        Self::new(
            bootstrap_digest,
            asset_definition_id,
            public_balance_scope,
            reserve_account,
            PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1,
            PrivacyRootV1::new(crate::privacy_engines::orchard::orchard_empty_root_v1()),
            0,
            None,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        asset_definition_id: AssetDefinitionId,
        public_balance_scope: AssetBalanceScope,
        reserve_account: AccountId,
        epoch: u64,
        root: PrivacyRootV1,
        tree_size: u64,
        leaf: Option<[u8; 32]>,
        ommers: Vec<[u8; 32]>,
    ) -> Result<Self, &'static str> {
        let state = Self {
            bootstrap_digest,
            asset_definition_id,
            public_balance_scope,
            reserve_account,
            epoch,
            root,
            tree_size,
            leaf,
            ommers,
        };
        state.validate()?;
        Ok(state)
    }

    /// Derive the next durable state from native frontier output.
    pub(crate) fn advance(
        &self,
        successor: crate::privacy_engines::orchard::OrchardFrontierPartsV1,
    ) -> Result<Self, &'static str> {
        self.validate()?;
        let appended = successor
            .tree_size
            .checked_sub(self.tree_size)
            .ok_or("Orchard successor tree size regressed")?;
        if appended == 0 || appended > u64::from(ORCHARD_MAX_ACTIONS_V1) {
            return Err("Orchard successor must append one or two actions");
        }
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Orchard root epoch overflow")?;
        if successor.root == self.root.into_bytes() {
            return Err("Orchard successor root must differ from its parent");
        }
        Self::new(
            self.bootstrap_digest,
            self.asset_definition_id.clone(),
            self.public_balance_scope,
            self.reserve_account.clone(),
            epoch,
            PrivacyRootV1::new(successor.root),
            successor.tree_size,
            successor.leaf,
            successor.ommers,
        )
    }

    /// Validate complete restored state by reconstructing and rehashing it.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.bootstrap_digest.is_zero() {
            return Err("Orchard pool bootstrap digest must be non-zero");
        }
        if matches!(
            self.public_balance_scope,
            AssetBalanceScope::Dataspace(iroha_data_model::nexus::DataSpaceId::UNIVERSAL)
        ) {
            return Err("Orchard public balance scope cannot be the universal dataspace");
        }
        if self.epoch < PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1 {
            return Err("Orchard pool epoch must be non-zero");
        }
        if self.root.is_zero() {
            return Err("Orchard pool root must be non-zero");
        }
        if (self.tree_size == 0) != (self.epoch == PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1) {
            return Err("Orchard empty frontier and origin epoch disagree");
        }
        let transitions = self
            .epoch
            .checked_sub(PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1)
            .ok_or("Orchard pool epoch precedes its canonical origin")?;
        let maximum_tree_size = transitions
            .checked_mul(u64::from(ORCHARD_MAX_ACTIONS_V1))
            .ok_or("Orchard pool transition count overflow")?;
        if self.tree_size < transitions || self.tree_size > maximum_tree_size {
            return Err("Orchard tree size is inconsistent with its transition epoch");
        }
        crate::privacy_engines::orchard::validate_orchard_frontier_v1(
            self.tree_size,
            self.leaf,
            &self.ommers,
            self.root.into_bytes(),
        )
        .map_err(|_| "Orchard compact frontier is invalid")
    }

    fn validate_bootstrap_binding(&self, namespace: PrivacyNamespaceV1) -> Result<(), String> {
        let PrivacyNamespaceScopeV1::Pool(pool) = namespace.scope() else {
            return Err("Orchard pool state has a non-pool namespace".to_owned());
        };
        let bootstrap = PrivacyOrchardPoolBootstrapV1::new(
            pool.pool_id,
            self.asset_definition_id.clone(),
            self.public_balance_scope,
            self.reserve_account.clone(),
        )
        .map_err(|error| format!("invalid reconstructed Orchard bootstrap: {error}"))?;
        let digest = bootstrap
            .digest()
            .map_err(|error| format!("failed to digest reconstructed Orchard bootstrap: {error}"))?;
        if digest != self.bootstrap_digest {
            return Err(
                "Orchard pool state fields do not match the governed bootstrap digest".to_owned(),
            );
        }
        Ok(())
    }

    #[must_use]
    pub(crate) const fn bootstrap_digest(&self) -> PrivacyOrchardPoolBootstrapDigestV1 {
        self.bootstrap_digest
    }

    #[must_use]
    pub(crate) const fn asset_definition_id(&self) -> &AssetDefinitionId {
        &self.asset_definition_id
    }

    #[must_use]
    pub(crate) const fn public_balance_scope(&self) -> AssetBalanceScope {
        self.public_balance_scope
    }

    #[must_use]
    pub(crate) const fn reserve_account(&self) -> &AccountId {
        &self.reserve_account
    }

    #[must_use]
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub(crate) const fn root(&self) -> PrivacyRootV1 {
        self.root
    }

    #[must_use]
    pub(crate) const fn tree_size(&self) -> u64 {
        self.tree_size
    }

    #[must_use]
    pub(crate) const fn leaf(&self) -> Option<[u8; 32]> {
        self.leaf
    }

    #[must_use]
    pub(crate) fn ommers(&self) -> &[[u8; 32]] {
        &self.ommers
    }
}

/// Public ledger objects that one governed Orchard pool must retain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyOrchardPoolReferenceV1 {
    namespace: PrivacyNamespaceV1,
    asset_definition_id: AssetDefinitionId,
    reserve_account: AccountId,
}

impl PrivacyOrchardPoolReferenceV1 {
    /// Return the exact governed Orchard pool namespace.
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Borrow the backing public asset definition.
    #[must_use]
    pub(crate) const fn asset_definition_id(&self) -> &AssetDefinitionId {
        &self.asset_definition_id
    }

    /// Borrow the public account that custodies pool reserves.
    #[must_use]
    pub(crate) const fn reserve_account(&self) -> &AccountId {
        &self.reserve_account
    }
}

/// Load every governed Orchard pool's exact public ledger dependencies.
///
/// The key range covers only singleton Orchard pool-state rows, so destructive
/// ledger operations remain proportional to the number of governed pools
/// rather than the potentially much larger privacy commitment table.
///
/// # Errors
///
/// Rejects malformed keys, invalid records, and wrong-role provenance.
pub(crate) fn load_privacy_orchard_pool_references_v1(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
) -> Result<Vec<PrivacyOrchardPoolReferenceV1>, String> {
    let mut references = Vec::new();
    for (key, record) in commitments.range(PrivacyCommitmentKeyV1::orchard_pool_state_range()) {
        key.validate()
            .map_err(|error| format!("invalid Orchard pool-state key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid Orchard pool-state record: {error}"))?;
        let namespace = key.orchard_namespace().ok_or_else(|| {
            "Orchard pool-state key range returned a differently typed key".to_owned()
        })?;
        let state = record
            .orchard_pool_state_ref()
            .ok_or_else(|| format!("Orchard pool state {namespace:?} has wrong-role provenance"))?;
        state.validate_bootstrap_binding(namespace)?;
        references.push(PrivacyOrchardPoolReferenceV1 {
            namespace,
            asset_definition_id: state.asset_definition_id().clone(),
            reserve_account: state.reserve_account().clone(),
        });
    }
    Ok(references)
}

/// Reject a restored world with dangling Orchard public-ledger dependencies.
///
/// # Errors
///
/// Rejects malformed Orchard state or a missing reserve account or asset
/// definition.
pub(crate) fn validate_privacy_orchard_public_dependencies_v1<
    AccountValue: mv::Value,
    AssetDefinitionValue: mv::Value,
>(
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    accounts: &impl StorageReadOnly<AccountId, AccountValue>,
    asset_definitions: &impl StorageReadOnly<AssetDefinitionId, AssetDefinitionValue>,
) -> Result<(), String> {
    for reference in load_privacy_orchard_pool_references_v1(commitments)? {
        if accounts.get(reference.reserve_account()).is_none() {
            return Err(format!(
                "Orchard pool {:?} references missing reserve account {}",
                reference.namespace(),
                reference.reserve_account()
            ));
        }
        if asset_definitions
            .get(reference.asset_definition_id())
            .is_none()
        {
            return Err(format!(
                "Orchard pool {:?} references missing asset definition {}",
                reference.namespace(),
                reference.asset_definition_id()
            ));
        }
    }
    Ok(())
}

/// Fully validated, transaction-local view of one governed Orchard pool.
///
/// The snapshot joins the singleton compact frontier to the exact retained
/// root chain. Native verification and successor derivation therefore consume
/// one coherent authoritative state instead of caller-duplicated roots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyOrchardPoolSnapshotV1 {
    namespace: PrivacyNamespaceV1,
    state: PrivacyOrchardPoolStateV1,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    retained_roots: Vec<(PrivacyRootKeyV1, PrivacyRootProvenanceV1)>,
}

impl PrivacyOrchardPoolSnapshotV1 {
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    #[must_use]
    pub(crate) const fn state(&self) -> &PrivacyOrchardPoolStateV1 {
        &self.state
    }

    #[must_use]
    pub(crate) const fn current_epoch(&self) -> u64 {
        self.state.epoch()
    }

    #[must_use]
    pub(crate) const fn current_root(&self) -> PrivacyRootV1 {
        self.state.root()
    }

    #[must_use]
    pub(crate) const fn bootstrap_digest(&self) -> PrivacyOrchardPoolBootstrapDigestV1 {
        self.state.bootstrap_digest()
    }

    #[must_use]
    pub(crate) const fn retention_anchor(&self) -> Option<PrivacyRootRetentionAnchorV1> {
        self.retention_anchor
    }

    /// Return whether the exact statement anchor is in the retained root window.
    #[must_use]
    pub(crate) fn contains_retained_anchor(&self, epoch: u64, root: PrivacyRootV1) -> bool {
        self.retained_roots
            .iter()
            .any(|(key, _)| key.epoch() == epoch && key.root() == root)
    }

    /// Append canonical note commitments to the authoritative current frontier.
    pub(crate) fn derive_successor(
        &self,
        note_commitments: &[[u8; 32]],
    ) -> Result<PrivacyOrchardPoolStateV1, String> {
        let successor = crate::privacy_engines::orchard::append_orchard_commitments_v1(
            self.state.tree_size(),
            self.state.leaf(),
            self.state.ommers(),
            self.state.root().into_bytes(),
            note_commitments,
        )
        .map_err(|error| format!("failed to append Orchard note commitments: {error}"))?;
        self.state
            .advance(successor)
            .map_err(|error| format!("invalid Orchard successor state: {error}"))
    }

    #[cfg(test)]
    pub(crate) fn canonical_bootstrap_for_test(
        namespace: PrivacyNamespaceV1,
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        asset_definition_id: AssetDefinitionId,
        public_balance_scope: AssetBalanceScope,
        reserve_account: AccountId,
    ) -> Self {
        validate_orchard_namespace(namespace).expect("test Orchard namespace is canonical");
        let state = PrivacyOrchardPoolStateV1::bootstrap(
            bootstrap_digest,
            asset_definition_id,
            public_balance_scope,
            reserve_account,
        )
        .expect("test Orchard state is canonical");
        let provenance = PrivacyRootProvenanceV1::orchard_pool_bootstrap(bootstrap_digest, 1)
            .expect("test Orchard provenance is canonical");
        let root_key = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            state.epoch(),
            state.root(),
        )
        .expect("test Orchard root key is canonical");
        Self {
            namespace,
            state,
            retention_anchor: None,
            retained_roots: vec![(root_key, provenance)],
        }
    }
}

fn validate_orchard_successor_link_v1(
    namespace: PrivacyNamespaceV1,
    key: PrivacyRootKeyV1,
    provenance: PrivacyRootProvenanceV1,
    bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
) -> Result<(u64, PrivacyRootV1), String> {
    let PrivacyRootProvenanceV1::OrchardPoolSuccessor {
        bootstrap_digest: observed_bootstrap_digest,
        parent_epoch,
        parent_root,
        ..
    } = provenance
    else {
        return Err(format!(
            "Orchard note-commitment history {namespace:?} contains a non-successor advancement"
        ));
    };
    if observed_bootstrap_digest != bootstrap_digest {
        return Err(format!(
            "Orchard successor {} is bound to a different pool bootstrap",
            key.epoch()
        ));
    }
    if parent_epoch.checked_add(1) != Some(key.epoch()) {
        return Err(format!(
            "Orchard successor {} does not advance parent epoch {parent_epoch} by exactly one",
            key.epoch()
        ));
    }
    Ok((parent_epoch, parent_root))
}

fn validate_orchard_retained_root_chain_v1(
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
    retained_root_count: usize,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    history: &[(PrivacyRootKeyV1, PrivacyRootProvenanceV1)],
) -> Result<(), String> {
    if retained_root_count == 0 {
        return Err("Orchard retained-root count must be non-zero".to_owned());
    }
    if history.is_empty() {
        return Err("Orchard pool has no retained note-commitment roots".to_owned());
    }
    if history.len() > retained_root_count {
        return Err(format!(
            "Orchard note-commitment history exceeds retention {retained_root_count}"
        ));
    }

    let canonical_empty_root =
        PrivacyRootV1::new(crate::privacy_engines::orchard::orchard_empty_root_v1());
    let (first_key, first_provenance) = history[0];
    match first_provenance {
        PrivacyRootProvenanceV1::OrchardPoolBootstrap {
            bootstrap_digest: observed_bootstrap_digest,
            ..
        } => {
            if observed_bootstrap_digest != bootstrap_digest {
                return Err("Orchard note-commitment origin differs from its pool state".to_owned());
            }
            if retention_anchor.is_some() {
                return Err(
                    "Orchard retained bootstrap history has an unexpected pruned-prefix anchor"
                        .to_owned(),
                );
            }
            if first_key.epoch() != PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1
                || first_key.root() != canonical_empty_root
            {
                return Err(
                    "Orchard pool bootstrap is not the canonical epoch-one empty root".to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::OrchardPoolSuccessor { .. } => {
            let (parent_epoch, parent_root) = validate_orchard_successor_link_v1(
                namespace,
                first_key,
                first_provenance,
                bootstrap_digest,
            )?;
            if first_key.epoch() <= PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1 {
                return Err(
                    "Orchard pruned history begins at or before the canonical bootstrap epoch"
                        .to_owned(),
                );
            }
            let anchor = retention_anchor.ok_or_else(|| {
                "Orchard pruned-prefix history has no exact retention anchor".to_owned()
            })?;
            if anchor.epoch().checked_add(1) != Some(first_key.epoch())
                || parent_epoch != anchor.epoch()
                || parent_root != anchor.root()
            {
                return Err(
                    "Orchard first retained successor does not consume its exact pruned-prefix anchor"
                        .to_owned(),
                );
            }
            if anchor.epoch() == PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1
                && anchor.root() != canonical_empty_root
            {
                return Err(
                    "Orchard pruned bootstrap anchor differs from the canonical empty root"
                        .to_owned(),
                );
            }
        }
        PrivacyRootProvenanceV1::Governance { .. }
        | PrivacyRootProvenanceV1::ZkX509CaGovernance { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolBootstrap { .. }
        | PrivacyRootProvenanceV1::ProofManagedPoolSuccessor { .. }
        | PrivacyRootProvenanceV1::VerifiedBootstrap { .. }
        | PrivacyRootProvenanceV1::VerifiedProof { .. }
        | PrivacyRootProvenanceV1::VerifiedPgcSuccessor { .. } => {
            return Err(
                "Orchard retained note-commitment history begins with invalid provenance"
                    .to_owned(),
            );
        }
    }

    for adjacent in history.windows(2) {
        let (parent_key, _) = adjacent[0];
        let (child_key, child_provenance) = adjacent[1];
        let (declared_parent_epoch, declared_parent_root) = validate_orchard_successor_link_v1(
            namespace,
            child_key,
            child_provenance,
            bootstrap_digest,
        )?;
        if parent_key.epoch().checked_add(1) != Some(child_key.epoch())
            || declared_parent_epoch != parent_key.epoch()
            || declared_parent_root != parent_key.root()
        {
            return Err(format!(
                "Orchard retained history has a gap or forged parent between epochs {} and {}",
                parent_key.epoch(),
                child_key.epoch()
            ));
        }
    }
    if retention_anchor.is_some() && history.len() != retained_root_count {
        return Err(format!(
            "Orchard anchored history has {} roots but must fill retention {retained_root_count}",
            history.len()
        ));
    }
    Ok(())
}

/// Load and cross-validate every authoritative component of one Orchard pool.
pub(crate) fn load_privacy_orchard_pool_snapshot_v1(
    namespace: PrivacyNamespaceV1,
    retained_root_count: u32,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<PrivacyOrchardPoolSnapshotV1, String> {
    validate_orchard_namespace(namespace)
        .map_err(|error| format!("invalid Orchard pool namespace: {error}"))?;
    if retained_root_count == 0 {
        return Err("Orchard retained-root count must be non-zero".to_owned());
    }
    let retained_root_count = usize::try_from(retained_root_count)
        .map_err(|_| "Orchard retained-root count cannot be represented".to_owned())?;

    let state_key = PrivacyCommitmentKeyV1::orchard_pool_state(namespace)
        .map_err(|error| format!("invalid Orchard pool-state key: {error}"))?;
    let state_record = commitments
        .get(&state_key)
        .ok_or_else(|| "Orchard pool has no authoritative compact frontier".to_owned())?;
    state_record
        .validate()
        .map_err(|error| format!("invalid Orchard pool-state record: {error}"))?;
    let state = state_record
        .orchard_pool_state_ref()
        .ok_or_else(|| "Orchard pool-state key has wrong-role provenance".to_owned())?
        .clone();
    state.validate_bootstrap_binding(namespace)?;

    let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
        .map_err(|error| format!("invalid Orchard root-head key: {error}"))?;
    let head = root_heads
        .get(&head_key)
        .copied()
        .ok_or_else(|| "Orchard pool has no current note-commitment head".to_owned())?;
    head.validate()
        .map_err(|error| format!("invalid Orchard note-commitment head: {error}"))?;

    let mut retained_roots = Vec::new();
    for (key, provenance) in roots.range(PrivacyRootKeyV1::history_range(
        namespace,
        PrivacyRootRoleV1::NoteCommitmentAnchor,
    )) {
        if retained_roots.len() == retained_root_count {
            return Err(format!(
                "Orchard note-commitment history exceeds retention {retained_root_count}"
            ));
        }
        key.validate()
            .map_err(|error| format!("invalid Orchard root key: {error}"))?;
        provenance
            .validate()
            .map_err(|error| format!("invalid Orchard root provenance: {error}"))?;
        if retained_roots.last().is_some_and(
            |(previous, _): &(PrivacyRootKeyV1, PrivacyRootProvenanceV1)| {
                previous.epoch() == key.epoch()
            },
        ) {
            return Err(format!(
                "Orchard note-commitment history contains duplicate epoch {}",
                key.epoch()
            ));
        }
        retained_roots.push((*key, *provenance));
    }
    validate_orchard_retained_root_chain_v1(
        namespace,
        state.bootstrap_digest(),
        retained_root_count,
        head.retention_anchor(),
        &retained_roots,
    )?;
    let latest = retained_roots
        .last()
        .expect("non-empty Orchard history checked above");
    if head.epoch() != latest.0.epoch()
        || head.root() != latest.0.root()
        || head.provenance() != latest.1
    {
        return Err(
            "Orchard note-commitment head does not equal latest retained history".to_owned(),
        );
    }
    if state.epoch() != head.epoch() || state.root() != head.root() {
        return Err("Orchard compact frontier does not equal its current root head".to_owned());
    }
    if head.provenance().orchard_bootstrap_digest() != Some(state.bootstrap_digest()) {
        return Err("Orchard root head differs from its governed pool bootstrap".to_owned());
    }

    Ok(PrivacyOrchardPoolSnapshotV1 {
        namespace,
        state,
        retention_anchor: head.retention_anchor(),
        retained_roots,
    })
}

/// Closed role-separated key for one consumed privacy replay marker.
///
/// The enum discriminant is part of canonical Norito key bytes. A ZK-AMS key
/// image therefore cannot alias a future protocol nullifier carrying the same
/// 32 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum PrivacyNullifierKeyV1 {
    /// One consumed ZK-ACE authorization nullifier in its exact policy lineage.
    ZkAceReplay {
        /// Stable authoritative policy identifier.
        policy_id: PrivacyPolicyIdV1,
        /// Canonical nonzero per-action replay nullifier.
        replay_nullifier: PrivacyNullifierV1,
    },
    /// One consumed ZK-AMS LSAG key image in its exact registry namespace.
    ZkAmsKeyImage {
        /// Issuer/registry/policy namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical nonzero compressed Ristretto key image.
        key_image: PrivacyZkAmsKeyImageV1,
    },
    /// One consumed certificate nullifier in its exact X.509 policy lineage.
    ZkX509CertificateNullifier {
        /// Exact trust-anchor/policy namespace selected by the certificate proof.
        namespace: PrivacyNamespaceV1,
        /// Canonical nonzero certificate-and-policy-derived nullifier.
        nullifier: PrivacyNullifierV1,
    },
    /// One consumed Orchard nullifier in its exact pool namespace.
    OrchardNullifier {
        /// Governed Orchard pool namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical Pallas-base nullifier encoding.
        nullifier: [u8; 32],
    },
    /// One consumed FCMP++ key image in its exact output-set namespace.
    FcmpKeyImage {
        /// Exact governed FCMP++ pool namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical nonzero Edwards key image `L`.
        key_image: PrivacyFcmpKeyImageV1,
    },
    /// One consumed nullifier in a private-IVM or PQ-MASP pool.
    ProofManagedNullifier {
        /// Exact governed private-note pool namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical nonzero protocol statement nullifier.
        nullifier: PrivacyNullifierV1,
    },
}

impl PrivacyNullifierKeyV1 {
    /// Construct a policy-scoped ZK-ACE replay key.
    ///
    /// # Errors
    ///
    /// Rejects a zero policy id or replay nullifier.
    pub fn zk_ace_replay(
        policy_id: PrivacyPolicyIdV1,
        replay_nullifier: PrivacyNullifierV1,
    ) -> Result<Self, &'static str> {
        if policy_id.is_zero() {
            return Err("ZK-ACE policy id must be non-zero");
        }
        if replay_nullifier.is_zero() {
            return Err("ZK-ACE replay nullifier must be non-zero");
        }
        Ok(Self::ZkAceReplay {
            policy_id,
            replay_nullifier,
        })
    }

    /// Construct a scoped ZK-AMS provisioning replay key.
    ///
    /// # Errors
    ///
    /// Rejects a non-ZK-AMS namespace or an all-zero key image.
    pub fn zk_ams_key_image(
        namespace: PrivacyNamespaceV1,
        key_image: PrivacyZkAmsKeyImageV1,
    ) -> Result<Self, &'static str> {
        validate_zk_ams_namespace(namespace)?;
        if key_image.is_zero() {
            return Err("ZK-AMS key image must be non-zero");
        }
        Ok(Self::ZkAmsKeyImage {
            namespace,
            key_image,
        })
    }

    /// Construct a policy-scoped X.509 certificate-nullifier key.
    ///
    /// # Errors
    ///
    /// Rejects a namespace outside the exact X.509 trust-anchor/policy role
    /// or an all-zero certificate nullifier.
    pub(crate) fn zk_x509_certificate_nullifier(
        namespace: PrivacyNamespaceV1,
        nullifier: PrivacyNullifierV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "X.509 certificate nullifier namespace is invalid")?;
        if namespace.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
            || !matches!(
                namespace.scope(),
                PrivacyNamespaceScopeV1::TrustAnchorPolicy(_)
            )
        {
            return Err(
                "X.509 certificate nullifier requires an X.509 trust-anchor/policy namespace",
            );
        }
        if nullifier.is_zero() {
            return Err("X.509 certificate nullifier must be non-zero");
        }
        Ok(Self::ZkX509CertificateNullifier {
            namespace,
            nullifier,
        })
    }

    /// Construct a pool-scoped canonical Orchard nullifier key.
    pub(crate) fn orchard_nullifier(
        namespace: PrivacyNamespaceV1,
        nullifier: [u8; 32],
    ) -> Result<Self, &'static str> {
        validate_orchard_namespace(namespace)?;
        if !crate::privacy_engines::orchard::is_canonical_orchard_nullifier_v1(&nullifier) {
            return Err("Orchard nullifier encoding is not canonical");
        }
        Ok(Self::OrchardNullifier {
            namespace,
            nullifier,
        })
    }

    /// Construct a nullifier key for one typed proof-managed pool.
    pub(crate) fn proof_managed_nullifier(
        namespace: PrivacyNamespaceV1,
        nullifier: PrivacyNullifierV1,
    ) -> Result<Self, &'static str> {
        validate_proof_managed_pool_namespace_v1(namespace)?;
        if namespace.protocol_id() == PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 {
            return Err("FCMP++ key images require the typed FCMP++ replay key");
        }
        if nullifier.is_zero() {
            return Err("proof-managed pool nullifier must be non-zero");
        }
        Ok(Self::ProofManagedNullifier {
            namespace,
            nullifier,
        })
    }

    /// Construct a typed FCMP++ key-image replay key.
    pub(crate) fn fcmp_key_image(
        namespace: PrivacyNamespaceV1,
        key_image: PrivacyFcmpKeyImageV1,
    ) -> Result<Self, &'static str> {
        validate_fcmp_namespace_v1(namespace)?;
        if key_image.is_zero() {
            return Err("FCMP++ key image must be non-zero");
        }
        Ok(Self::FcmpKeyImage {
            namespace,
            key_image,
        })
    }

    /// Return the exact ZK-AMS namespace, if this is a key-image marker.
    #[must_use]
    pub const fn zk_ams_namespace(self) -> Option<PrivacyNamespaceV1> {
        match self {
            Self::ZkAceReplay { .. }
            | Self::ZkX509CertificateNullifier { .. }
            | Self::OrchardNullifier { .. }
            | Self::FcmpKeyImage { .. }
            | Self::ProofManagedNullifier { .. } => None,
            Self::ZkAmsKeyImage { namespace, .. } => Some(namespace),
        }
    }

    /// Return the typed ZK-AMS key image, if present.
    #[must_use]
    pub const fn zk_ams_image(self) -> Option<PrivacyZkAmsKeyImageV1> {
        match self {
            Self::ZkAceReplay { .. }
            | Self::ZkX509CertificateNullifier { .. }
            | Self::OrchardNullifier { .. }
            | Self::FcmpKeyImage { .. }
            | Self::ProofManagedNullifier { .. } => None,
            Self::ZkAmsKeyImage { key_image, .. } => Some(key_image),
        }
    }

    /// Return the exact X.509 policy namespace and certificate nullifier.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) const fn zk_x509_certificate_identity(
        self,
    ) -> Option<(PrivacyNamespaceV1, PrivacyNullifierV1)> {
        match self {
            Self::ZkX509CertificateNullifier {
                namespace,
                nullifier,
            } => Some((namespace, nullifier)),
            Self::ZkAceReplay { .. }
            | Self::ZkAmsKeyImage { .. }
            | Self::OrchardNullifier { .. }
            | Self::FcmpKeyImage { .. }
            | Self::ProofManagedNullifier { .. } => None,
        }
    }

    /// Return the proof-managed pool namespace and nullifier, if present.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) const fn proof_managed_identity(
        self,
    ) -> Option<(PrivacyNamespaceV1, PrivacyNullifierV1)> {
        match self {
            Self::ProofManagedNullifier {
                namespace,
                nullifier,
            } => Some((namespace, nullifier)),
            Self::ZkAceReplay { .. }
            | Self::ZkAmsKeyImage { .. }
            | Self::ZkX509CertificateNullifier { .. }
            | Self::OrchardNullifier { .. }
            | Self::FcmpKeyImage { .. } => None,
        }
    }

    /// Return the exact FCMP++ namespace and typed key image, if present.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) const fn fcmp_identity(self) -> Option<(PrivacyNamespaceV1, PrivacyFcmpKeyImageV1)> {
        match self {
            Self::FcmpKeyImage {
                namespace,
                key_image,
            } => Some((namespace, key_image)),
            Self::ZkAceReplay { .. }
            | Self::ZkAmsKeyImage { .. }
            | Self::ZkX509CertificateNullifier { .. }
            | Self::OrchardNullifier { .. }
            | Self::ProofManagedNullifier { .. } => None,
        }
    }

    /// Return the protocol whose closed replay-key role is encoded.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAceReplay { .. } => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::ZkAmsKeyImage { .. } => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::ZkX509CertificateNullifier { .. } => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::OrchardNullifier { .. } => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::FcmpKeyImage { .. } => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::ProofManagedNullifier { namespace, .. } => namespace.protocol_id(),
        }
    }

    /// Ordered bounds covering consumed key images in exactly one namespace.
    #[must_use]
    pub fn zk_ams_key_image_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ZkAmsKeyImage {
            namespace,
            key_image: PrivacyZkAmsKeyImageV1::new([0; 32]),
        }..=Self::ZkAmsKeyImage {
            namespace,
            key_image: PrivacyZkAmsKeyImageV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering consumed certificate nullifiers in one X.509 policy.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) fn zk_x509_certificate_nullifier_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ZkX509CertificateNullifier {
            namespace,
            nullifier: PrivacyNullifierV1::new([0; 32]),
        }..=Self::ZkX509CertificateNullifier {
            namespace,
            nullifier: PrivacyNullifierV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering all consumed nullifiers in one Orchard pool.
    #[must_use]
    pub(crate) fn orchard_nullifier_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::OrchardNullifier {
            namespace,
            nullifier: [0; 32],
        }..=Self::OrchardNullifier {
            namespace,
            nullifier: [u8::MAX; 32],
        }
    }

    /// Ordered bounds covering every consumed nullifier in one proof-managed pool.
    #[must_use]
    pub(crate) fn proof_managed_nullifier_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ProofManagedNullifier {
            namespace,
            nullifier: PrivacyNullifierV1::new([0; 32]),
        }..=Self::ProofManagedNullifier {
            namespace,
            nullifier: PrivacyNullifierV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering every consumed FCMP++ key image in one pool.
    #[must_use]
    pub(crate) fn fcmp_key_image_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::FcmpKeyImage {
            namespace,
            key_image: PrivacyFcmpKeyImageV1::new([0; 32]),
        }..=Self::FcmpKeyImage {
            namespace,
            key_image: PrivacyFcmpKeyImageV1::new([u8::MAX; 32]),
        }
    }

    fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::ZkAceReplay {
                policy_id,
                replay_nullifier,
            } => Self::zk_ace_replay(policy_id, replay_nullifier).map(|_| ()),
            Self::ZkAmsKeyImage {
                namespace,
                key_image,
            } => Self::zk_ams_key_image(namespace, key_image).map(|_| ()),
            Self::ZkX509CertificateNullifier {
                namespace,
                nullifier,
            } => Self::zk_x509_certificate_nullifier(namespace, nullifier).map(|_| ()),
            Self::OrchardNullifier {
                namespace,
                nullifier,
            } => Self::orchard_nullifier(namespace, nullifier).map(|_| ()),
            Self::FcmpKeyImage {
                namespace,
                key_image,
            } => Self::fcmp_key_image(namespace, key_image).map(|_| ()),
            Self::ProofManagedNullifier {
                namespace,
                nullifier,
            } => Self::proof_managed_nullifier(namespace, nullifier).map(|_| ()),
        }
    }
}

/// Closed role-separated key for one admitted privacy state item.
///
/// Distinct canonical enum variants provide protocol-level domain separation:
/// issuer records, PHC hashes, and seed keys cannot collide even when their
/// inner 32-byte values happen to be equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum PrivacyCommitmentKeyV1 {
    /// Authoritative ZK-ACE policy selected by its stable identifier.
    ZkAcePolicy {
        /// Stable policy lookup key.
        policy_id: PrivacyPolicyIdV1,
    },
    /// Current authoritative Bootle/Lantern issuer policy.
    BootleLanternIssuerPolicy {
        /// Stable credential issuer identity.
        issuer_id: PrivacyIssuerIdV1,
        /// Stable policy identity within the issuer namespace.
        policy_id: PrivacyPolicyIdV1,
    },
    /// One immutable revision in a Vega issuer-key/policy lineage.
    VegaIssuerRevision {
        /// Stable credential-issuer lineage identifier.
        issuer_id: PrivacyIssuerIdV1,
        /// Exact immutable revision epoch.
        record_epoch: u64,
    },
    /// Complete authoritative compact state of one Orchard pool.
    OrchardPoolState {
        /// Governed Orchard pool namespace.
        namespace: PrivacyNamespaceV1,
    },
    /// Immutable typed bootstrap/configuration for one proof-managed pool.
    ProofManagedPoolConfig {
        /// Exact FCMP++, private-IVM, or PQ-MASP namespace.
        namespace: PrivacyNamespaceV1,
    },
    /// One genesis or proof-produced note commitment in an IVM/PQ pool.
    ProofManagedPoolCommitment {
        /// Exact governed private-note pool namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical nonzero commitment.
        commitment: PrivacyCommitmentV1,
    },
    /// One complete FCMP++ output tuple indexed by its derived output id.
    FcmpOutput {
        /// Exact governed FCMP++ pool namespace.
        namespace: PrivacyNamespaceV1,
        /// Ledger-only id derived from the complete persisted tuple.
        output_id: PrivacyFcmpOutputIdV1,
    },
    /// One immutable revision in an X.509 trust-anchor lineage.
    ZkX509TrustAnchorRevision {
        /// Stable trust-anchor lineage identifier.
        trust_anchor_id: PrivacyIssuerIdV1,
        /// Exact immutable revision epoch.
        record_epoch: u64,
    },
    /// One immutable revision in an X.509 certificate-policy lineage.
    ZkX509CertificatePolicyRevision {
        /// Trust-anchor namespace containing this policy.
        trust_anchor_id: PrivacyIssuerIdV1,
        /// Stable certificate-policy lineage identifier.
        policy_id: PrivacyPolicyIdV1,
        /// Exact immutable revision epoch.
        record_epoch: u64,
    },
    /// Current self-chained signed-CRL record for one certificate policy.
    ZkX509CrlCurrent {
        /// Trust-anchor namespace containing this policy.
        trust_anchor_id: PrivacyIssuerIdV1,
        /// Certificate-policy lineage selecting one direct leaf issuer.
        policy_id: PrivacyPolicyIdV1,
    },
    /// Governed issuer-key/policy record.
    ZkAmsIssuerPolicyRecord {
        /// Issuer/registry/policy namespace.
        namespace: PrivacyNamespaceV1,
        /// Digest of the exact fixed issuer key and policy record.
        record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    },
    /// Admitted canonical Personhood Credential hash.
    ZkAmsPhc {
        /// Issuer/registry/policy namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical PHC hash.
        phc_hash: PrivacyZkAmsPhcHashV1,
    },
    /// Admitted Ristretto seed public key.
    ZkAmsSeedKey {
        /// Issuer/registry/policy namespace.
        namespace: PrivacyNamespaceV1,
        /// Canonical seed public key.
        seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
    },
}

impl PrivacyCommitmentKeyV1 {
    /// Construct the authoritative key for one ZK-ACE policy lineage.
    pub fn zk_ace_policy(policy_id: PrivacyPolicyIdV1) -> Result<Self, &'static str> {
        if policy_id.is_zero() {
            return Err("ZK-ACE policy id must be non-zero");
        }
        Ok(Self::ZkAcePolicy { policy_id })
    }

    /// Construct the singleton current-policy key for one Bootle/Lantern lineage.
    pub fn bootle_lantern_issuer_policy(
        issuer_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
    ) -> Result<Self, &'static str> {
        if issuer_id.is_zero() {
            return Err("Bootle/Lantern issuer id must be non-zero");
        }
        if policy_id.is_zero() {
            return Err("Bootle/Lantern policy id must be non-zero");
        }
        Ok(Self::BootleLanternIssuerPolicy {
            issuer_id,
            policy_id,
        })
    }

    /// Return the Bootle/Lantern issuer and policy identity, if present.
    #[must_use]
    pub const fn bootle_lantern_issuer_policy_identity(
        self,
    ) -> Option<(PrivacyIssuerIdV1, PrivacyPolicyIdV1)> {
        match self {
            Self::BootleLanternIssuerPolicy {
                issuer_id,
                policy_id,
            } => Some((issuer_id, policy_id)),
            Self::ZkAcePolicy { .. }
            | Self::VegaIssuerRevision { .. }
            | Self::OrchardPoolState { .. }
            | Self::ProofManagedPoolConfig { .. }
            | Self::ProofManagedPoolCommitment { .. }
            | Self::FcmpOutput { .. }
            | Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. }
            | Self::ZkAmsIssuerPolicyRecord { .. }
            | Self::ZkAmsPhc { .. }
            | Self::ZkAmsSeedKey { .. } => None,
        }
    }

    /// Construct the singleton compact-state key for one Orchard pool.
    pub(crate) fn orchard_pool_state(namespace: PrivacyNamespaceV1) -> Result<Self, &'static str> {
        validate_orchard_namespace(namespace)?;
        Ok(Self::OrchardPoolState { namespace })
    }

    /// Ordered bounds covering exactly the complete Orchard pool-state table.
    #[must_use]
    pub(crate) fn orchard_pool_state_range() -> core::ops::RangeInclusive<Self> {
        let namespace = |pool_id| {
            PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: PrivacyPoolIdV1::new(pool_id),
                }),
            )
        };
        Self::OrchardPoolState {
            namespace: namespace([0; 32]),
        }..=Self::OrchardPoolState {
            namespace: namespace([u8::MAX; 32]),
        }
    }

    /// Construct the singleton typed configuration key for one proof-managed pool.
    pub(crate) fn proof_managed_pool_config(
        namespace: PrivacyNamespaceV1,
    ) -> Result<Self, &'static str> {
        validate_proof_managed_pool_namespace_v1(namespace)?;
        Ok(Self::ProofManagedPoolConfig { namespace })
    }

    /// Construct one exact proof-managed pool commitment key.
    pub(crate) fn proof_managed_pool_commitment(
        namespace: PrivacyNamespaceV1,
        commitment: PrivacyCommitmentV1,
    ) -> Result<Self, &'static str> {
        validate_proof_managed_pool_namespace_v1(namespace)?;
        if namespace.protocol_id() == PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 {
            return Err("FCMP++ outputs require the typed FCMP++ output key");
        }
        if commitment.is_zero() {
            return Err("proof-managed pool commitment must be non-zero");
        }
        Ok(Self::ProofManagedPoolCommitment {
            namespace,
            commitment,
        })
    }

    /// Construct one exact typed FCMP++ output key.
    pub(crate) fn fcmp_output(
        namespace: PrivacyNamespaceV1,
        output_id: PrivacyFcmpOutputIdV1,
    ) -> Result<Self, &'static str> {
        validate_fcmp_namespace_v1(namespace)?;
        if output_id.is_zero() {
            return Err("FCMP++ output id must be non-zero");
        }
        Ok(Self::FcmpOutput {
            namespace,
            output_id,
        })
    }

    /// Ordered bounds covering all commitments in exactly one proof-managed pool.
    #[must_use]
    pub(crate) fn proof_managed_pool_commitment_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ProofManagedPoolCommitment {
            namespace,
            commitment: PrivacyCommitmentV1::new([0; 32]),
        }..=Self::ProofManagedPoolCommitment {
            namespace,
            commitment: PrivacyCommitmentV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering all FCMP++ outputs in exactly one pool.
    #[must_use]
    pub(crate) fn fcmp_output_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::FcmpOutput {
            namespace,
            output_id: PrivacyFcmpOutputIdV1::new([0; 32]),
        }..=Self::FcmpOutput {
            namespace,
            output_id: PrivacyFcmpOutputIdV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering exactly the complete ZK-ACE policy table.
    #[must_use]
    pub fn zk_ace_policy_range() -> core::ops::RangeInclusive<Self> {
        Self::ZkAcePolicy {
            policy_id: PrivacyPolicyIdV1::new([0; 32]),
        }..=Self::ZkAcePolicy {
            policy_id: PrivacyPolicyIdV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering exactly the current Bootle/Lantern policy table.
    #[must_use]
    pub fn bootle_lantern_issuer_policy_range() -> core::ops::RangeInclusive<Self> {
        Self::BootleLanternIssuerPolicy {
            issuer_id: PrivacyIssuerIdV1::new([0; 32]),
            policy_id: PrivacyPolicyIdV1::new([0; 32]),
        }..=Self::BootleLanternIssuerPolicy {
            issuer_id: PrivacyIssuerIdV1::new([u8::MAX; 32]),
            policy_id: PrivacyPolicyIdV1::new([u8::MAX; 32]),
        }
    }

    /// Construct the exact key for one immutable Vega issuer revision.
    pub fn vega_issuer_revision(
        issuer_id: PrivacyIssuerIdV1,
        record_epoch: u64,
    ) -> Result<Self, &'static str> {
        if issuer_id.is_zero() {
            return Err("Vega issuer id must be non-zero");
        }
        if record_epoch == 0 {
            return Err("Vega issuer revision epoch must be non-zero");
        }
        Ok(Self::VegaIssuerRevision {
            issuer_id,
            record_epoch,
        })
    }

    /// Ordered bounds covering the complete Vega issuer revision table.
    #[must_use]
    pub fn vega_issuer_revision_range() -> core::ops::RangeInclusive<Self> {
        Self::VegaIssuerRevision {
            issuer_id: PrivacyIssuerIdV1::new([0; 32]),
            record_epoch: 0,
        }..=Self::VegaIssuerRevision {
            issuer_id: PrivacyIssuerIdV1::new([u8::MAX; 32]),
            record_epoch: u64::MAX,
        }
    }

    /// Ordered bounds covering exactly one Vega issuer lineage.
    #[must_use]
    pub fn vega_issuer_lineage_range(
        issuer_id: PrivacyIssuerIdV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::VegaIssuerRevision {
            issuer_id,
            record_epoch: 0,
        }..=Self::VegaIssuerRevision {
            issuer_id,
            record_epoch: u64::MAX,
        }
    }

    /// Return the Orchard pool namespace, if this is a compact-state key.
    #[must_use]
    pub(crate) const fn orchard_namespace(self) -> Option<PrivacyNamespaceV1> {
        match self {
            Self::OrchardPoolState { namespace } => Some(namespace),
            Self::ZkAcePolicy { .. }
            | Self::BootleLanternIssuerPolicy { .. }
            | Self::VegaIssuerRevision { .. }
            | Self::ProofManagedPoolConfig { .. }
            | Self::ProofManagedPoolCommitment { .. }
            | Self::FcmpOutput { .. }
            | Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. }
            | Self::ZkAmsIssuerPolicyRecord { .. }
            | Self::ZkAmsPhc { .. }
            | Self::ZkAmsSeedKey { .. } => None,
        }
    }

    /// Return the proof-managed pool namespace, if this key belongs to one.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) const fn proof_managed_namespace(self) -> Option<PrivacyNamespaceV1> {
        match self {
            Self::ProofManagedPoolConfig { namespace }
            | Self::ProofManagedPoolCommitment { namespace, .. }
            | Self::FcmpOutput { namespace, .. } => Some(namespace),
            Self::ZkAcePolicy { .. }
            | Self::BootleLanternIssuerPolicy { .. }
            | Self::VegaIssuerRevision { .. }
            | Self::OrchardPoolState { .. }
            | Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. }
            | Self::ZkAmsIssuerPolicyRecord { .. }
            | Self::ZkAmsPhc { .. }
            | Self::ZkAmsSeedKey { .. } => None,
        }
    }

    /// Construct the exact key for one immutable X.509 trust-anchor revision.
    pub fn zk_x509_trust_anchor_revision(
        trust_anchor_id: PrivacyIssuerIdV1,
        record_epoch: u64,
    ) -> Result<Self, &'static str> {
        if trust_anchor_id.is_zero() {
            return Err("X.509 trust-anchor id must be non-zero");
        }
        if record_epoch == 0 {
            return Err("X.509 trust-anchor revision epoch must be non-zero");
        }
        Ok(Self::ZkX509TrustAnchorRevision {
            trust_anchor_id,
            record_epoch,
        })
    }

    /// Ordered bounds covering the complete X.509 trust-anchor revision table.
    #[must_use]
    pub fn zk_x509_trust_anchor_revision_range() -> core::ops::RangeInclusive<Self> {
        Self::ZkX509TrustAnchorRevision {
            trust_anchor_id: PrivacyIssuerIdV1::new([0; 32]),
            record_epoch: 0,
        }..=Self::ZkX509TrustAnchorRevision {
            trust_anchor_id: PrivacyIssuerIdV1::new([u8::MAX; 32]),
            record_epoch: u64::MAX,
        }
    }

    /// Ordered bounds covering one X.509 trust-anchor lineage.
    #[must_use]
    pub fn zk_x509_trust_anchor_lineage_range(
        trust_anchor_id: PrivacyIssuerIdV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ZkX509TrustAnchorRevision {
            trust_anchor_id,
            record_epoch: 0,
        }..=Self::ZkX509TrustAnchorRevision {
            trust_anchor_id,
            record_epoch: u64::MAX,
        }
    }

    /// Construct the exact key for one immutable X.509 certificate-policy revision.
    pub fn zk_x509_certificate_policy_revision(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        record_epoch: u64,
    ) -> Result<Self, &'static str> {
        if trust_anchor_id.is_zero() {
            return Err("X.509 trust-anchor id must be non-zero");
        }
        if policy_id.is_zero() {
            return Err("X.509 certificate-policy id must be non-zero");
        }
        if record_epoch == 0 {
            return Err("X.509 certificate-policy revision epoch must be non-zero");
        }
        Ok(Self::ZkX509CertificatePolicyRevision {
            trust_anchor_id,
            policy_id,
            record_epoch,
        })
    }

    /// Ordered bounds covering the complete X.509 certificate-policy revision table.
    #[must_use]
    pub fn zk_x509_certificate_policy_revision_range() -> core::ops::RangeInclusive<Self> {
        Self::ZkX509CertificatePolicyRevision {
            trust_anchor_id: PrivacyIssuerIdV1::new([0; 32]),
            policy_id: PrivacyPolicyIdV1::new([0; 32]),
            record_epoch: 0,
        }..=Self::ZkX509CertificatePolicyRevision {
            trust_anchor_id: PrivacyIssuerIdV1::new([u8::MAX; 32]),
            policy_id: PrivacyPolicyIdV1::new([u8::MAX; 32]),
            record_epoch: u64::MAX,
        }
    }

    /// Ordered bounds covering one X.509 certificate-policy lineage.
    #[must_use]
    pub fn zk_x509_certificate_policy_lineage_range(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ZkX509CertificatePolicyRevision {
            trust_anchor_id,
            policy_id,
            record_epoch: 0,
        }..=Self::ZkX509CertificatePolicyRevision {
            trust_anchor_id,
            policy_id,
            record_epoch: u64::MAX,
        }
    }

    /// Construct the singleton current signed-CRL key for one policy lineage.
    pub fn zk_x509_crl_current(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
    ) -> Result<Self, &'static str> {
        if trust_anchor_id.is_zero() {
            return Err("X.509 trust-anchor id must be non-zero");
        }
        if policy_id.is_zero() {
            return Err("X.509 certificate-policy id must be non-zero");
        }
        Ok(Self::ZkX509CrlCurrent {
            trust_anchor_id,
            policy_id,
        })
    }

    /// Ordered bounds covering every current X.509 signed-CRL lineage.
    #[must_use]
    pub fn zk_x509_crl_current_range() -> core::ops::RangeInclusive<Self> {
        Self::ZkX509CrlCurrent {
            trust_anchor_id: PrivacyIssuerIdV1::new([0; 32]),
            policy_id: PrivacyPolicyIdV1::new([0; 32]),
        }..=Self::ZkX509CrlCurrent {
            trust_anchor_id: PrivacyIssuerIdV1::new([u8::MAX; 32]),
            policy_id: PrivacyPolicyIdV1::new([u8::MAX; 32]),
        }
    }

    /// Construct the exact governed ZK-AMS issuer-policy record key.
    pub fn zk_ams_issuer_policy_record(
        namespace: PrivacyNamespaceV1,
        record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    ) -> Result<Self, &'static str> {
        validate_zk_ams_namespace(namespace)?;
        if record_digest.is_zero() {
            return Err("ZK-AMS issuer-policy record digest must be non-zero");
        }
        Ok(Self::ZkAmsIssuerPolicyRecord {
            namespace,
            record_digest,
        })
    }

    /// Construct one exact admitted ZK-AMS PHC key.
    pub fn zk_ams_phc(
        namespace: PrivacyNamespaceV1,
        phc_hash: PrivacyZkAmsPhcHashV1,
    ) -> Result<Self, &'static str> {
        validate_zk_ams_namespace(namespace)?;
        if phc_hash.is_zero() {
            return Err("ZK-AMS PHC hash must be non-zero");
        }
        Ok(Self::ZkAmsPhc {
            namespace,
            phc_hash,
        })
    }

    /// Construct one exact admitted ZK-AMS seed-key membership key.
    pub fn zk_ams_seed_key(
        namespace: PrivacyNamespaceV1,
        seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
    ) -> Result<Self, &'static str> {
        validate_zk_ams_namespace(namespace)?;
        if seed_public_key.is_zero() {
            return Err("ZK-AMS seed public key must be non-zero");
        }
        Ok(Self::ZkAmsSeedKey {
            namespace,
            seed_public_key,
        })
    }

    /// Return the exact ZK-AMS namespace, if this is a ZK-AMS record.
    #[must_use]
    pub const fn zk_ams_namespace(self) -> Option<PrivacyNamespaceV1> {
        match self {
            Self::ZkAcePolicy { .. }
            | Self::BootleLanternIssuerPolicy { .. }
            | Self::VegaIssuerRevision { .. }
            | Self::OrchardPoolState { .. }
            | Self::ProofManagedPoolConfig { .. }
            | Self::ProofManagedPoolCommitment { .. }
            | Self::FcmpOutput { .. }
            | Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. } => None,
            Self::ZkAmsIssuerPolicyRecord { namespace, .. }
            | Self::ZkAmsPhc { namespace, .. }
            | Self::ZkAmsSeedKey { namespace, .. } => Some(namespace),
        }
    }

    /// Return the protocol whose closed state-key role is encoded.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePolicy { .. } => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::BootleLanternIssuerPolicy { .. } => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            Self::VegaIssuerRevision { .. } => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::OrchardPoolState { .. } => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::ProofManagedPoolConfig { namespace }
            | Self::ProofManagedPoolCommitment { namespace, .. }
            | Self::FcmpOutput { namespace, .. } => namespace.protocol_id(),
            Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. } => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::ZkAmsIssuerPolicyRecord { .. }
            | Self::ZkAmsPhc { .. }
            | Self::ZkAmsSeedKey { .. } => PrivacyProtocolIdV1::IrohaZkAmsV1,
        }
    }

    /// Return the governed issuer-policy digest for that exact key role.
    #[must_use]
    pub const fn zk_ams_issuer_policy_digest(
        self,
    ) -> Option<PrivacyZkAmsIssuerPolicyRecordDigestV1> {
        match self {
            Self::ZkAcePolicy { .. }
            | Self::BootleLanternIssuerPolicy { .. }
            | Self::VegaIssuerRevision { .. }
            | Self::OrchardPoolState { .. }
            | Self::ProofManagedPoolConfig { .. }
            | Self::ProofManagedPoolCommitment { .. }
            | Self::FcmpOutput { .. }
            | Self::ZkX509TrustAnchorRevision { .. }
            | Self::ZkX509CertificatePolicyRevision { .. }
            | Self::ZkX509CrlCurrent { .. } => None,
            Self::ZkAmsIssuerPolicyRecord { record_digest, .. } => Some(record_digest),
            Self::ZkAmsPhc { .. } | Self::ZkAmsSeedKey { .. } => None,
        }
    }

    /// Ordered bounds covering issuer-policy records in exactly one namespace.
    #[must_use]
    pub fn zk_ams_issuer_policy_record_range(
        namespace: PrivacyNamespaceV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self::ZkAmsIssuerPolicyRecord {
            namespace,
            record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1::new([0; 32]),
        }..=Self::ZkAmsIssuerPolicyRecord {
            namespace,
            record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering PHC records in exactly one namespace.
    #[must_use]
    pub fn zk_ams_phc_range(namespace: PrivacyNamespaceV1) -> core::ops::RangeInclusive<Self> {
        Self::ZkAmsPhc {
            namespace,
            phc_hash: PrivacyZkAmsPhcHashV1::new([0; 32]),
        }..=Self::ZkAmsPhc {
            namespace,
            phc_hash: PrivacyZkAmsPhcHashV1::new([u8::MAX; 32]),
        }
    }

    /// Ordered bounds covering admitted seed keys in exactly one namespace.
    #[must_use]
    pub fn zk_ams_seed_key_range(namespace: PrivacyNamespaceV1) -> core::ops::RangeInclusive<Self> {
        Self::ZkAmsSeedKey {
            namespace,
            seed_public_key: PrivacyZkAmsSeedPublicKeyV1::new([0; 32]),
        }..=Self::ZkAmsSeedKey {
            namespace,
            seed_public_key: PrivacyZkAmsSeedPublicKeyV1::new([u8::MAX; 32]),
        }
    }

    fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::ZkAcePolicy { policy_id } => Self::zk_ace_policy(policy_id).map(|_| ()),
            Self::BootleLanternIssuerPolicy {
                issuer_id,
                policy_id,
            } => Self::bootle_lantern_issuer_policy(issuer_id, policy_id).map(|_| ()),
            Self::VegaIssuerRevision {
                issuer_id,
                record_epoch,
            } => Self::vega_issuer_revision(issuer_id, record_epoch).map(|_| ()),
            Self::OrchardPoolState { namespace } => Self::orchard_pool_state(namespace).map(|_| ()),
            Self::ProofManagedPoolConfig { namespace } => {
                Self::proof_managed_pool_config(namespace).map(|_| ())
            }
            Self::ProofManagedPoolCommitment {
                namespace,
                commitment,
            } => Self::proof_managed_pool_commitment(namespace, commitment).map(|_| ()),
            Self::FcmpOutput {
                namespace,
                output_id,
            } => Self::fcmp_output(namespace, output_id).map(|_| ()),
            Self::ZkX509TrustAnchorRevision {
                trust_anchor_id,
                record_epoch,
            } => Self::zk_x509_trust_anchor_revision(trust_anchor_id, record_epoch).map(|_| ()),
            Self::ZkX509CertificatePolicyRevision {
                trust_anchor_id,
                policy_id,
                record_epoch,
            } => {
                Self::zk_x509_certificate_policy_revision(trust_anchor_id, policy_id, record_epoch)
                    .map(|_| ())
            }
            Self::ZkX509CrlCurrent {
                trust_anchor_id,
                policy_id,
            } => Self::zk_x509_crl_current(trust_anchor_id, policy_id).map(|_| ()),
            Self::ZkAmsIssuerPolicyRecord {
                namespace,
                record_digest,
            } => Self::zk_ams_issuer_policy_record(namespace, record_digest).map(|_| ()),
            Self::ZkAmsPhc {
                namespace,
                phc_hash,
            } => Self::zk_ams_phc(namespace, phc_hash).map(|_| ()),
            Self::ZkAmsSeedKey {
                namespace,
                seed_public_key,
            } => Self::zk_ams_seed_key(namespace, seed_public_key).map(|_| ()),
        }
    }
}

fn validate_zk_ams_namespace(namespace: PrivacyNamespaceV1) -> Result<(), &'static str> {
    namespace
        .validate()
        .map_err(|_| "ZK-AMS state namespace is invalid")?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::IrohaZkAmsV1 {
        return Err("ZK-AMS state key requires the ZK-AMS protocol namespace");
    }
    Ok(())
}

fn validate_orchard_namespace(namespace: PrivacyNamespaceV1) -> Result<(), &'static str> {
    namespace
        .validate()
        .map_err(|_| "Orchard state namespace is invalid")?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::OrchardHalo2ActionsV1 {
        return Err("Orchard state key requires the Orchard protocol namespace");
    }
    Ok(())
}

/// Return the sole root role for a first-release proof-managed pool namespace.
pub(crate) fn proof_managed_pool_root_role_v1(
    namespace: PrivacyNamespaceV1,
) -> Result<PrivacyRootRoleV1, &'static str> {
    namespace
        .validate()
        .map_err(|_| "proof-managed pool namespace is invalid")?;
    let role = match namespace.protocol_id() {
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => PrivacyRootRoleV1::OutputSet,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => PrivacyRootRoleV1::ProgramState,
        PrivacyProtocolIdV1::PqMaspStarkV0 => PrivacyRootRoleV1::NoteCommitmentAnchor,
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        | PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        | PrivacyProtocolIdV1::VeRangeTransparentRangeV1
        | PrivacyProtocolIdV1::IrohaZkAmsV1
        | PrivacyProtocolIdV1::VegaExistingCredentialZkV0
        | PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
        | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
        | PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
        | PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
            return Err("namespace is not a proof-managed FCMP++, private-IVM, or PQ-MASP pool");
        }
    };
    if !role.is_compatible_with_namespace(namespace) {
        return Err("proof-managed pool root role is incompatible with its namespace");
    }
    Ok(role)
}

fn validate_proof_managed_pool_namespace_v1(
    namespace: PrivacyNamespaceV1,
) -> Result<(), &'static str> {
    proof_managed_pool_root_role_v1(namespace).map(|_| ())
}

fn validate_fcmp_namespace_v1(namespace: PrivacyNamespaceV1) -> Result<(), &'static str> {
    validate_proof_managed_pool_namespace_v1(namespace)?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 {
        return Err("FCMP++ state key requires the FCMP++ protocol namespace");
    }
    Ok(())
}

fn validate_proof_managed_pool_protocol_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> Result<(), &'static str> {
    if matches!(
        protocol_id,
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
            | PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
            | PrivacyProtocolIdV1::PqMaspStarkV0
    ) {
        Ok(())
    } else {
        Err("protocol is not a proof-managed FCMP++, private-IVM, or PQ-MASP pool")
    }
}

/// Exact ordered root-membership key.
///
/// Lexicographic order is namespace, semantic role, epoch, then root bytes.
/// This gives deterministic oldest-first pruning while preserving exact
/// `(namespace, role, epoch, root)` membership checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct PrivacyRootKeyV1 {
    namespace: PrivacyNamespaceV1,
    role: PrivacyRootRoleV1,
    epoch: u64,
    root: PrivacyRootV1,
}

impl PrivacyRootKeyV1 {
    /// Construct and validate an exact root-membership key.
    ///
    /// # Errors
    ///
    /// Rejects an invalid namespace, incompatible root role, zero epoch, or
    /// all-zero root.
    pub fn new(
        namespace: PrivacyNamespaceV1,
        role: PrivacyRootRoleV1,
        epoch: u64,
        root: PrivacyRootV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy root namespace is invalid")?;
        if !role.is_compatible_with_namespace(namespace) {
            return Err("privacy root role is incompatible with its exact namespace");
        }
        if epoch == 0 {
            return Err("privacy root epoch must be non-zero");
        }
        if root.is_zero() {
            return Err("privacy root must be non-zero");
        }
        Ok(Self {
            namespace,
            role,
            epoch,
            root,
        })
    }

    /// Return the exact namespace.
    #[must_use]
    pub const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Return the semantic role.
    #[must_use]
    pub const fn role(self) -> PrivacyRootRoleV1 {
        self.role
    }

    /// Return the exact root epoch.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Return the exact root.
    #[must_use]
    pub const fn root(self) -> PrivacyRootV1 {
        self.root
    }

    /// Return ordered bounds covering exactly one independent root history.
    ///
    /// The sentinel endpoints are used only as B-tree range bounds; they are
    /// not valid persistable root keys.
    #[must_use]
    pub fn history_range(
        namespace: PrivacyNamespaceV1,
        role: PrivacyRootRoleV1,
    ) -> core::ops::RangeInclusive<Self> {
        Self {
            namespace,
            role,
            epoch: 0,
            root: PrivacyRootV1::new([0; 32]),
        }..=Self {
            namespace,
            role,
            epoch: u64::MAX,
            root: PrivacyRootV1::new([u8::MAX; 32]),
        }
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace, self.role, self.epoch, self.root).map(|_| ())
    }
}

/// Exact key for the single current root of one independent history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) struct PrivacyRootHeadKeyV1 {
    namespace: PrivacyNamespaceV1,
    role: PrivacyRootRoleV1,
}

impl PrivacyRootHeadKeyV1 {
    /// Construct and validate one root-head key.
    ///
    /// # Errors
    ///
    /// Rejects an invalid namespace or incompatible root role.
    pub(crate) fn new(
        namespace: PrivacyNamespaceV1,
        role: PrivacyRootRoleV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy root-head namespace is invalid")?;
        if !role.is_compatible_with_namespace(namespace) {
            return Err("privacy root-head role is incompatible with its exact namespace");
        }
        Ok(Self { namespace, role })
    }

    /// Return the exact namespace.
    #[must_use]
    pub(crate) const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Return the root role.
    #[must_use]
    pub(crate) const fn role(self) -> PrivacyRootRoleV1 {
        self.role
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace, self.role).map(|_| ())
    }
}

/// Domain-separated provenance shared by root history and the current head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(tag = "origin", content = "record", deny_unknown_fields)]
pub(crate) enum PrivacyRootProvenanceV1 {
    /// Root published by an authorized governance instruction.
    Governance {
        /// Digest of the exact canonical root-publication payload.
        publication_digest: PrivacyRootPublicationDigestV1,
        /// Block height at which the publication became durable.
        admitted_at_height: u64,
    },
    /// X.509 CA root derived from one complete trust-anchor revision.
    ZkX509CaGovernance {
        /// Digest of the exact canonical root-publication payload.
        publication_digest: PrivacyRootPublicationDigestV1,
        /// Exact protocol and trust-anchor/policy namespace.
        namespace: PrivacyNamespaceV1,
        /// Exact published root epoch.
        epoch: u64,
        /// Exact published root.
        root: PrivacyRootV1,
        /// Complete self-digested trust-anchor revision deriving this CA root.
        trust_anchor_record: PrivacyZkX509TrustAnchorRecordV1,
        /// Block height at which the publication became durable.
        admitted_at_height: u64,
    },
    /// Initial ZK-AMS AccountRegistry root installed by its typed bootstrap.
    ZkAmsRegistryBootstrap {
        /// Digest of the exact canonical registry-bootstrap payload.
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        /// Block height at which the bootstrap became durable.
        admitted_at_height: u64,
    },
    /// ZK-AMS AccountRegistry successor certified by a complete admission proof.
    ZkAmsRegistrySuccessor {
        /// Immutable typed registry-bootstrap provenance.
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the successor became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
        /// Exact epoch consumed by the verified transition.
        parent_epoch: u64,
        /// Exact root consumed by the verified transition.
        parent_root: PrivacyRootV1,
    },
    /// Initial Orchard note-commitment root installed by its typed pool bootstrap.
    OrchardPoolBootstrap {
        /// Digest of the exact canonical pool-bootstrap payload.
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        /// Block height at which the bootstrap became durable.
        admitted_at_height: u64,
    },
    /// Orchard note-commitment successor derived from a verified action bundle.
    OrchardPoolSuccessor {
        /// Immutable typed pool-bootstrap provenance.
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the successor became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
        /// Exact epoch consumed by the verified transition.
        parent_epoch: u64,
        /// Exact root consumed by the verified transition.
        parent_root: PrivacyRootV1,
    },
    /// Initial FCMP++, private-IVM, or PQ-MASP root derived from typed governance.
    ProofManagedPoolBootstrap {
        /// Digest of the complete canonical bootstrap payload.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Exact protocol whose native accumulator derived the root.
        protocol_id: PrivacyProtocolIdV1,
        /// Block height at which the bootstrap became durable.
        admitted_at_height: u64,
    },
    /// Proof-derived successor in an FCMP++, private-IVM, or PQ-MASP pool.
    ProofManagedPoolSuccessor {
        /// Immutable typed pool-bootstrap provenance.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Exact protocol whose native verifier certified the successor.
        protocol_id: PrivacyProtocolIdV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Exact number of nullifiers/key images emitted by the statement.
        nullifier_count: u32,
        /// Exact number of outputs emitted by the statement.
        output_count: u32,
        /// Block height at which the successor became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
        /// Exact epoch consumed by the verified transition.
        parent_epoch: u64,
        /// Exact root consumed by the verified transition.
        parent_root: PrivacyRootV1,
    },
    /// Initial PGC account-state root certified by a native bootstrap proof.
    VerifiedBootstrap {
        /// Digest of the exact canonical public bootstrap payload.
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        /// Digest of the exact canonical native bootstrap proof.
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        /// Block height at which the bootstrap became durable.
        admitted_at_height: u64,
    },
    /// Root certified as the successor state by an admitted native proof.
    VerifiedProof {
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the proof effect became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
        /// Exact epoch consumed by the verified state transition.
        parent_epoch: u64,
        /// Exact root consumed by the verified state transition.
        parent_root: PrivacyRootV1,
    },
    /// Anonymous PGC successor certified by an admitted native payment proof.
    VerifiedPgcSuccessor {
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the proof effect became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
        /// Exact epoch consumed by the verified state transition.
        parent_epoch: u64,
        /// Exact root consumed by the verified state transition.
        parent_root: PrivacyRootV1,
        /// Immutable pool invariant bound by the native proof.
        pool_invariant_digest: PrivacyPgcPoolInvariantDigestV1,
    },
}

impl PrivacyRootProvenanceV1 {
    /// Construct governance provenance.
    ///
    /// # Errors
    ///
    /// Rejects a zero publication digest or zero block height.
    pub(crate) fn governance(
        publication_digest: PrivacyRootPublicationDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if publication_digest.is_zero() {
            return Err("privacy root publication digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::Governance {
            publication_digest,
            admitted_at_height,
        })
    }

    /// Construct trust-anchor-bound X.509 CA-root provenance.
    ///
    /// # Errors
    ///
    /// Rejects malformed publication fields, a non-X.509 namespace, a root
    /// not exactly carried by the trust-anchor record, or zero admission
    /// height.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn zk_x509_ca_governance(
        publication_digest: PrivacyRootPublicationDigestV1,
        namespace: PrivacyNamespaceV1,
        epoch: u64,
        root: PrivacyRootV1,
        trust_anchor_record: PrivacyZkX509TrustAnchorRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if publication_digest.is_zero() {
            return Err("X.509 CA-root publication digest must be non-zero");
        }
        namespace
            .validate()
            .map_err(|_| "X.509 CA-root namespace is invalid")?;
        if namespace.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
            return Err("X.509 CA-root provenance requires the X.509 protocol namespace");
        }
        let PrivacyNamespaceScopeV1::TrustAnchor(scope) = namespace.scope() else {
            return Err("X.509 CA-root namespace requires a trust-anchor-wide scope");
        };
        PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::CertificateAuthorityMembership,
            epoch,
            root,
        )
        .map_err(|_| "X.509 CA-root publication fields are invalid")?;
        trust_anchor_record
            .validate()
            .map_err(|_| "X.509 CA-root trust-anchor record is invalid")?;
        if trust_anchor_record.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
            return Err("X.509 CA-root trust-anchor record must be active");
        }
        if trust_anchor_record.trust_anchor_id != scope.trust_anchor_id
            || trust_anchor_record.ca_membership_root != root
            || trust_anchor_record.ca_membership_root_epoch != epoch
        {
            return Err("X.509 CA root differs from its complete trust-anchor record");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::ZkX509CaGovernance {
            publication_digest,
            namespace,
            epoch,
            root,
            trust_anchor_record,
            admitted_at_height,
        })
    }

    /// Construct typed ZK-AMS registry-bootstrap provenance.
    pub(crate) fn zk_ams_registry_bootstrap(
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("ZK-AMS registry bootstrap digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::ZkAmsRegistryBootstrap {
            bootstrap_digest,
            admitted_at_height,
        })
    }

    /// Construct a ZK-AMS registry successor with immutable origin binding.
    pub(crate) fn zk_ams_registry_successor(
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
        parent_epoch: u64,
        parent_root: PrivacyRootV1,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("ZK-AMS registry bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("ZK-AMS registry statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        if parent_epoch == 0 {
            return Err("ZK-AMS registry parent epoch must be non-zero");
        }
        if parent_root.is_zero() {
            return Err("ZK-AMS registry parent root must be non-zero");
        }
        Ok(Self::ZkAmsRegistrySuccessor {
            bootstrap_digest,
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        })
    }

    /// Construct typed Orchard pool-bootstrap provenance.
    pub(crate) fn orchard_pool_bootstrap(
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("Orchard pool bootstrap digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::OrchardPoolBootstrap {
            bootstrap_digest,
            admitted_at_height,
        })
    }

    /// Construct an Orchard successor with immutable pool-origin binding.
    pub(crate) fn orchard_pool_successor(
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
        parent_epoch: u64,
        parent_root: PrivacyRootV1,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("Orchard pool bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("Orchard statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        if parent_epoch == 0 {
            return Err("Orchard parent epoch must be non-zero");
        }
        if parent_root.is_zero() {
            return Err("Orchard parent root must be non-zero");
        }
        Ok(Self::OrchardPoolSuccessor {
            bootstrap_digest,
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        })
    }

    /// Construct typed provenance for a proof-managed pool origin.
    pub(crate) fn proof_managed_pool_bootstrap(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        protocol_id: PrivacyProtocolIdV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("proof-managed pool bootstrap digest must be non-zero");
        }
        validate_proof_managed_pool_protocol_v1(protocol_id)?;
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::ProofManagedPoolBootstrap {
            bootstrap_digest,
            protocol_id,
            admitted_at_height,
        })
    }

    /// Construct a proof-managed successor with immutable pool-origin binding.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn proof_managed_pool_successor(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        protocol_id: PrivacyProtocolIdV1,
        statement_digest: PrivacyStatementDigestV1,
        nullifier_count: u32,
        output_count: u32,
        admitted_at_height: u64,
        action_index: u32,
        parent_epoch: u64,
        parent_root: PrivacyRootV1,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("proof-managed pool bootstrap digest must be non-zero");
        }
        validate_proof_managed_pool_protocol_v1(protocol_id)?;
        if statement_digest.is_zero() {
            return Err("proof-managed pool statement digest must be non-zero");
        }
        let (max_nullifiers, max_outputs) = match protocol_id {
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => (FCMP_MAX_INPUTS_V1, FCMP_MAX_OUTPUTS_V1),
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => (
                IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
            ),
            PrivacyProtocolIdV1::PqMaspStarkV0 => (PQ_MASP_MAX_INPUTS_V1, PQ_MASP_MAX_OUTPUTS_V1),
            _ => return Err("unsupported proof-managed pool protocol"),
        };
        if nullifier_count == 0 || nullifier_count > max_nullifiers {
            return Err("proof-managed pool successor nullifier count is outside its native bound");
        }
        if output_count == 0 || output_count > max_outputs {
            return Err("proof-managed pool successor output count is outside its native bound");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        if parent_epoch == 0 {
            return Err("proof-managed pool parent epoch must be non-zero");
        }
        if parent_root.is_zero() {
            return Err("proof-managed pool parent root must be non-zero");
        }
        Ok(Self::ProofManagedPoolSuccessor {
            bootstrap_digest,
            protocol_id,
            statement_digest,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        })
    }

    /// Construct native PGC bootstrap provenance.
    ///
    /// # Errors
    ///
    /// Rejects either zero digest or a zero block height.
    pub(crate) fn verified_bootstrap(
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("privacy root bootstrap digest must be non-zero");
        }
        if bootstrap_proof_digest.is_zero() {
            return Err("privacy root bootstrap proof digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::VerifiedBootstrap {
            bootstrap_digest,
            bootstrap_proof_digest,
            admitted_at_height,
        })
    }

    /// Construct verified-proof provenance.
    ///
    /// # Errors
    ///
    /// Rejects a zero statement digest or zero block height.
    pub(crate) fn verified_proof(
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
        parent_epoch: u64,
        parent_root: PrivacyRootV1,
    ) -> Result<Self, &'static str> {
        if statement_digest.is_zero() {
            return Err("privacy root statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        if parent_epoch == 0 {
            return Err("privacy root proof parent epoch must be non-zero");
        }
        if parent_root.is_zero() {
            return Err("privacy root proof parent root must be non-zero");
        }
        Ok(Self::VerifiedProof {
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        })
    }

    /// Construct a PGC successor with its complete retained-chain binding.
    ///
    /// # Errors
    ///
    /// Rejects malformed statement, admission, parent, or pool-invariant
    /// provenance.
    pub(crate) fn verified_pgc_successor(
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
        parent_epoch: u64,
        parent_root: PrivacyRootV1,
        pool_invariant_digest: PrivacyPgcPoolInvariantDigestV1,
    ) -> Result<Self, &'static str> {
        if statement_digest.is_zero() {
            return Err("privacy PGC successor statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy PGC successor admission height must be non-zero");
        }
        if parent_epoch == 0 {
            return Err("privacy PGC successor parent epoch must be non-zero");
        }
        if parent_root.is_zero() {
            return Err("privacy PGC successor parent root must be non-zero");
        }
        pool_invariant_digest.validate()?;
        Ok(Self::VerifiedPgcSuccessor {
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
            pool_invariant_digest,
        })
    }

    /// Return the exact consumed root for proof-produced successors.
    #[must_use]
    pub(crate) const fn proof_parent(self) -> Option<(u64, PrivacyRootV1)> {
        match self {
            Self::VerifiedProof {
                parent_epoch,
                parent_root,
                ..
            }
            | Self::VerifiedPgcSuccessor {
                parent_epoch,
                parent_root,
                ..
            }
            | Self::ZkAmsRegistrySuccessor {
                parent_epoch,
                parent_root,
                ..
            }
            | Self::OrchardPoolSuccessor {
                parent_epoch,
                parent_root,
                ..
            }
            | Self::ProofManagedPoolSuccessor {
                parent_epoch,
                parent_root,
                ..
            } => Some((parent_epoch, parent_root)),
            Self::Governance { .. }
            | Self::ZkX509CaGovernance { .. }
            | Self::ZkAmsRegistryBootstrap { .. }
            | Self::OrchardPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::VerifiedBootstrap { .. } => None,
        }
    }

    /// Return the immutable ZK-AMS registry origin carried by typed root
    /// provenance.
    #[must_use]
    pub(crate) const fn zk_ams_bootstrap_digest(
        self,
    ) -> Option<PrivacyZkAmsRegistryBootstrapDigestV1> {
        match self {
            Self::ZkAmsRegistryBootstrap {
                bootstrap_digest, ..
            }
            | Self::ZkAmsRegistrySuccessor {
                bootstrap_digest, ..
            } => Some(bootstrap_digest),
            Self::Governance { .. }
            | Self::ZkX509CaGovernance { .. }
            | Self::OrchardPoolBootstrap { .. }
            | Self::OrchardPoolSuccessor { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolSuccessor { .. }
            | Self::VerifiedBootstrap { .. }
            | Self::VerifiedProof { .. }
            | Self::VerifiedPgcSuccessor { .. } => None,
        }
    }

    /// Return the immutable Orchard pool origin carried by typed root provenance.
    #[must_use]
    pub(crate) const fn orchard_bootstrap_digest(
        self,
    ) -> Option<PrivacyOrchardPoolBootstrapDigestV1> {
        match self {
            Self::OrchardPoolBootstrap {
                bootstrap_digest, ..
            }
            | Self::OrchardPoolSuccessor {
                bootstrap_digest, ..
            } => Some(bootstrap_digest),
            Self::Governance { .. }
            | Self::ZkX509CaGovernance { .. }
            | Self::ZkAmsRegistryBootstrap { .. }
            | Self::ZkAmsRegistrySuccessor { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolSuccessor { .. }
            | Self::VerifiedBootstrap { .. }
            | Self::VerifiedProof { .. }
            | Self::VerifiedPgcSuccessor { .. } => None,
        }
    }

    /// Return the immutable typed origin for a proof-managed pool root.
    #[must_use]
    pub(crate) const fn proof_managed_pool_origin(
        self,
    ) -> Option<(
        PrivacyProofManagedPoolBootstrapDigestV1,
        PrivacyProtocolIdV1,
    )> {
        match self {
            Self::ProofManagedPoolBootstrap {
                bootstrap_digest,
                protocol_id,
                ..
            }
            | Self::ProofManagedPoolSuccessor {
                bootstrap_digest,
                protocol_id,
                ..
            } => Some((bootstrap_digest, protocol_id)),
            Self::Governance { .. }
            | Self::ZkX509CaGovernance { .. }
            | Self::ZkAmsRegistryBootstrap { .. }
            | Self::ZkAmsRegistrySuccessor { .. }
            | Self::OrchardPoolBootstrap { .. }
            | Self::OrchardPoolSuccessor { .. }
            | Self::VerifiedBootstrap { .. }
            | Self::VerifiedProof { .. }
            | Self::VerifiedPgcSuccessor { .. } => None,
        }
    }

    /// Validate restored provenance.
    ///
    /// # Errors
    ///
    /// Rejects zero domain-separated digests or zero admission heights.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Governance {
                publication_digest,
                admitted_at_height,
            } => Self::governance(publication_digest, admitted_at_height).map(|_| ()),
            Self::ZkX509CaGovernance {
                publication_digest,
                namespace,
                epoch,
                root,
                trust_anchor_record,
                admitted_at_height,
            } => Self::zk_x509_ca_governance(
                publication_digest,
                namespace,
                epoch,
                root,
                trust_anchor_record,
                admitted_at_height,
            )
            .map(|_| ()),
            Self::ZkAmsRegistryBootstrap {
                bootstrap_digest,
                admitted_at_height,
            } => Self::zk_ams_registry_bootstrap(bootstrap_digest, admitted_at_height).map(|_| ()),
            Self::ZkAmsRegistrySuccessor {
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            } => Self::zk_ams_registry_successor(
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            )
            .map(|_| ()),
            Self::OrchardPoolBootstrap {
                bootstrap_digest,
                admitted_at_height,
            } => Self::orchard_pool_bootstrap(bootstrap_digest, admitted_at_height).map(|_| ()),
            Self::OrchardPoolSuccessor {
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            } => Self::orchard_pool_successor(
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            )
            .map(|_| ()),
            Self::ProofManagedPoolBootstrap {
                bootstrap_digest,
                protocol_id,
                admitted_at_height,
            } => Self::proof_managed_pool_bootstrap(
                bootstrap_digest,
                protocol_id,
                admitted_at_height,
            )
            .map(|_| ()),
            Self::ProofManagedPoolSuccessor {
                bootstrap_digest,
                protocol_id,
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            } => Self::proof_managed_pool_successor(
                bootstrap_digest,
                protocol_id,
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            )
            .map(|_| ()),
            Self::VerifiedBootstrap {
                bootstrap_digest,
                bootstrap_proof_digest,
                admitted_at_height,
            } => Self::verified_bootstrap(
                bootstrap_digest,
                bootstrap_proof_digest,
                admitted_at_height,
            )
            .map(|_| ()),
            Self::VerifiedProof {
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            } => Self::verified_proof(
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            )
            .map(|_| ()),
            Self::VerifiedPgcSuccessor {
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
                pool_invariant_digest,
            } => Self::verified_pgc_successor(
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
                pool_invariant_digest,
            )
            .map(|_| ()),
        }
    }
}

/// Exact last-pruned root immediately preceding the retained history window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyRootRetentionAnchorV1 {
    epoch: u64,
    root: PrivacyRootV1,
}

impl PrivacyRootRetentionAnchorV1 {
    pub(crate) fn new(epoch: u64, root: PrivacyRootV1) -> Result<Self, &'static str> {
        if epoch == 0 {
            return Err("privacy root retention-anchor epoch must be non-zero");
        }
        if root.is_zero() {
            return Err("privacy root retention-anchor root must be non-zero");
        }
        Ok(Self { epoch, root })
    }

    #[must_use]
    pub(crate) const fn epoch(self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub(crate) const fn root(self) -> PrivacyRootV1 {
        self.root
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.epoch, self.root).map(|_| ())
    }
}

/// Current canonical root, its provenance, and exact retained-prefix anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyRootHeadRecordV1 {
    epoch: u64,
    root: PrivacyRootV1,
    provenance: PrivacyRootProvenanceV1,
    retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
}

impl PrivacyRootHeadRecordV1 {
    /// Construct a validated current-head record.
    ///
    /// # Errors
    ///
    /// Rejects a zero epoch/root or malformed provenance.
    pub(crate) fn new(
        epoch: u64,
        root: PrivacyRootV1,
        provenance: PrivacyRootProvenanceV1,
        retention_anchor: Option<PrivacyRootRetentionAnchorV1>,
    ) -> Result<Self, &'static str> {
        if epoch == 0 {
            return Err("privacy root-head epoch must be non-zero");
        }
        if root.is_zero() {
            return Err("privacy root-head root must be non-zero");
        }
        provenance.validate()?;
        if let Some(anchor) = retention_anchor {
            anchor.validate()?;
            if anchor.epoch() >= epoch {
                return Err("privacy root retention anchor must precede the current head");
            }
        }
        Ok(Self {
            epoch,
            root,
            provenance,
            retention_anchor,
        })
    }

    /// Return the exact current epoch.
    #[must_use]
    pub(crate) const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Return the exact current root.
    #[must_use]
    pub(crate) const fn root(self) -> PrivacyRootV1 {
        self.root
    }

    /// Return typed publication/proof provenance.
    #[must_use]
    pub(crate) const fn provenance(self) -> PrivacyRootProvenanceV1 {
        self.provenance
    }

    /// Return the exact root immediately before the retained window, if any.
    #[must_use]
    pub(crate) const fn retention_anchor(self) -> Option<PrivacyRootRetentionAnchorV1> {
        self.retention_anchor
    }

    /// Validate a restored head record.
    ///
    /// # Errors
    ///
    /// Rejects a zero epoch/root or malformed provenance.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        Self::new(
            self.epoch,
            self.root,
            self.provenance,
            self.retention_anchor,
        )
        .map(|_| ())
    }
}

/// Durable, origin-typed provenance for one privacy state item.
///
/// Governance records and proof-produced items are distinct closed variants;
/// governance can never manufacture a synthetic statement digest, and a proof
/// path cannot impersonate the registry bootstrap.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(tag = "origin", content = "record", deny_unknown_fields)]
pub enum PrivacyStateItemRecordV1 {
    /// Complete authoritative ZK-ACE policy installed or replaced by governance.
    ZkAcePolicyGovernance {
        /// Canonical self-digested policy state.
        policy: PrivacyZkAcePolicyRecordV1,
        /// Block height at which governance installed this revision.
        admitted_at_height: u64,
    },
    /// Current authoritative Bootle/Lantern issuer policy installed by governance.
    BootleLanternIssuerPolicyGovernance {
        /// Canonical self-authenticating issuer policy.
        policy: BootleLanternIssuerPolicyV1,
        /// Block height at which governance installed this current revision.
        admitted_at_height: u64,
    },
    /// Immutable Vega issuer-key/policy revision installed by typed governance.
    VegaIssuerGovernance {
        /// Complete canonical self-digested Vega issuer revision.
        record: PrivacyVegaIssuerRecordV1,
        /// Block height at which governance admitted this revision.
        admitted_at_height: u64,
    },
    /// Immutable X.509 trust-anchor revision installed by typed governance.
    ZkX509TrustAnchorGovernance {
        /// Complete canonical self-digested trust-anchor revision.
        record: PrivacyZkX509TrustAnchorRecordV1,
        /// Block height at which governance admitted this revision.
        admitted_at_height: u64,
    },
    /// Immutable X.509 certificate-policy revision installed by typed governance.
    ZkX509CertificatePolicyGovernance {
        /// Complete canonical self-digested certificate-policy revision.
        record: PrivacyZkX509CertificatePolicyRecordV1,
        /// Block height at which governance admitted this revision.
        admitted_at_height: u64,
    },
    /// Current self-chained signed-CRL record installed by typed governance.
    ZkX509CrlGovernance {
        /// Complete canonical current signed-CRL record.
        record: PrivacyZkX509CrlRecordV1,
        /// Block height at which governance installed this current revision.
        admitted_at_height: u64,
    },
    /// Replay marker emitted by one directly verified X.509 certificate proof.
    ZkX509VerifiedCertificateNullifier {
        /// Exact trust-anchor revision selected by the verified statement.
        trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1,
        /// Exact trust-anchor revision epoch selected by the verified statement.
        trust_anchor_record_epoch: u64,
        /// Exact certificate-policy revision selected by the verified statement.
        certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1,
        /// Exact certificate-policy revision epoch selected by the verified statement.
        certificate_policy_record_epoch: u64,
        /// Exact signed-CRL revision selected by the verified statement.
        crl_record_digest: PrivacyZkX509CrlRecordDigestV1,
        /// Exact signed-CRL revision epoch selected by the verified statement.
        crl_record_epoch: u64,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the certificate nullifier became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// Complete authoritative compact frontier and invariant for one Orchard pool.
    OrchardPoolState {
        /// Canonical pool state reconstructed and rehashed on restore.
        state: PrivacyOrchardPoolStateV1,
    },
    /// Replay marker emitted by one directly verified Orchard action.
    OrchardVerifiedNullifier {
        /// Immutable pool bootstrap selected for verification.
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the nullifier was consumed.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// Complete immutable bootstrap installed for a proof-managed pool.
    ProofManagedPoolBootstrap {
        /// Canonical typed bootstrap, including asset/program bindings.
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        /// Digest of the exact canonical bootstrap.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Native accumulator root derived from the genesis commitment set.
        initial_root: PrivacyRootV1,
        /// Current validator-owned protocol-specific native frontier.
        accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1,
        /// Block height at which governance initialized the pool.
        admitted_at_height: u64,
    },
    /// One position-bound genesis commitment installed by typed governance.
    ProofManagedPoolBootstrapCommitment {
        /// Digest of the complete canonical bootstrap.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Zero-based position in the canonical genesis commitment order.
        position: u64,
        /// Block height at which governance initialized the pool.
        admitted_at_height: u64,
    },
    /// One position-bound complete FCMP++ genesis output installed by governance.
    FcmpBootstrapOutput {
        /// Digest of the complete canonical FCMP++ bootstrap.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Complete `(O, I, C)` tuple consumed by the curve tree.
        output: PrivacyFcmpOutputTupleV1,
        /// Zero-based position in canonical genesis order.
        position: u64,
        /// Block height at which governance initialized the pool.
        admitted_at_height: u64,
    },
    /// One nullifier emitted by a directly verified pool proof.
    ProofManagedPoolVerifiedNullifier {
        /// Immutable typed pool-bootstrap provenance.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Exact number of nullifiers/key images emitted by this statement.
        nullifier_count: u32,
        /// Exact number of outputs emitted by this statement.
        output_count: u32,
        /// Block height at which the item became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// One position-bound output commitment emitted by a verified pool proof.
    ProofManagedPoolVerifiedCommitment {
        /// Immutable typed pool-bootstrap provenance.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Root epoch produced by the statement that appended this output.
        successor_epoch: u64,
        /// Zero-based output index inside the exact verified statement order.
        output_index: u32,
        /// Zero-based position in the complete append-only commitment order.
        append_position: u64,
        /// Exact number of nullifiers emitted by this statement.
        nullifier_count: u32,
        /// Exact number of outputs emitted by this statement.
        output_count: u32,
        /// Block height at which the commitment became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// One position-bound complete FCMP++ output emitted by a verified proof.
    FcmpVerifiedOutput {
        /// Immutable typed FCMP++ pool-bootstrap provenance.
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        /// Complete `(O, I, C)` tuple appended to the curve tree.
        output: PrivacyFcmpOutputTupleV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Root epoch produced by the statement that appended this output.
        successor_epoch: u64,
        /// Zero-based output index inside the exact verified statement order.
        output_index: u32,
        /// Zero-based position in the complete append-only output order.
        append_position: u64,
        /// Exact number of key images emitted by this statement.
        nullifier_count: u32,
        /// Exact number of outputs emitted by this statement.
        output_count: u32,
        /// Block height at which the output became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// Replay marker emitted by one directly verified ZK-ACE authorization.
    ZkAceVerifiedAuthorization {
        /// Stable policy lineage used for verification.
        policy_id: PrivacyPolicyIdV1,
        /// Exact authoritative policy revision used for verification.
        policy_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the authorization was applied.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
    /// Issuer-policy record installed by the typed ZK-AMS bootstrap.
    ZkAmsGovernance {
        /// Digest of the complete canonical bootstrap.
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        /// Block height at which governance initialized the registry.
        admitted_at_height: u64,
    },
    /// PHC, seed key, or key image emitted by a verified ZK-AMS proof.
    ZkAmsVerifiedProof {
        /// Registry bootstrap whose append-only history admitted this item.
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the item became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
    },
}

impl PrivacyStateItemRecordV1 {
    /// Construct the authoritative value for one governed ZK-ACE policy.
    pub fn zk_ace_policy_governance(
        policy: PrivacyZkAcePolicyRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        policy
            .validate()
            .map_err(|_| "ZK-ACE policy record is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkAcePolicyGovernance {
            policy,
            admitted_at_height,
        })
    }

    /// Construct the authoritative value for one governed Bootle/Lantern policy.
    pub fn bootle_lantern_issuer_policy_governance(
        policy: BootleLanternIssuerPolicyV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        policy
            .validate()
            .map_err(|_| "Bootle/Lantern issuer-policy record is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::BootleLanternIssuerPolicyGovernance {
            policy,
            admitted_at_height,
        })
    }

    /// Construct provenance for one immutable governed Vega issuer revision.
    pub fn vega_issuer_governance(
        record: PrivacyVegaIssuerRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        record
            .validate()
            .map_err(|_| "Vega issuer record is invalid")?;
        crate::privacy_engines::p256::CompressedPointV1::from_slice(
            record.issuer_public_key.as_bytes(),
        )
        .map_err(|_| "Vega issuer public key is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::VegaIssuerGovernance {
            record,
            admitted_at_height,
        })
    }

    /// Construct provenance for one immutable governed X.509 trust-anchor revision.
    pub fn zk_x509_trust_anchor_governance(
        record: PrivacyZkX509TrustAnchorRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        record
            .validate()
            .map_err(|_| "X.509 trust-anchor record is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkX509TrustAnchorGovernance {
            record,
            admitted_at_height,
        })
    }

    /// Construct provenance for one immutable governed X.509 certificate-policy revision.
    pub fn zk_x509_certificate_policy_governance(
        record: PrivacyZkX509CertificatePolicyRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        record
            .validate()
            .map_err(|_| "X.509 certificate-policy record is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkX509CertificatePolicyGovernance {
            record,
            admitted_at_height,
        })
    }

    /// Construct the current governed X.509 signed-CRL state.
    pub fn zk_x509_crl_governance(
        record: PrivacyZkX509CrlRecordV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        record
            .validate()
            .map_err(|_| "X.509 signed-CRL record is invalid")?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkX509CrlGovernance {
            record,
            admitted_at_height,
        })
    }

    /// Construct provenance for one consumed X.509 certificate nullifier.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn zk_x509_verified_certificate_nullifier(
        trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1,
        trust_anchor_record_epoch: u64,
        certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1,
        certificate_policy_record_epoch: u64,
        crl_record_digest: PrivacyZkX509CrlRecordDigestV1,
        crl_record_epoch: u64,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if trust_anchor_record_digest.is_zero() {
            return Err("X.509 trust-anchor record digest must be non-zero");
        }
        if trust_anchor_record_epoch == 0 {
            return Err("X.509 trust-anchor record epoch must be non-zero");
        }
        if certificate_policy_record_digest.is_zero() {
            return Err("X.509 certificate-policy record digest must be non-zero");
        }
        if certificate_policy_record_epoch == 0 {
            return Err("X.509 certificate-policy record epoch must be non-zero");
        }
        if crl_record_digest.is_zero() {
            return Err("X.509 signed-CRL record digest must be non-zero");
        }
        if crl_record_epoch == 0 {
            return Err("X.509 signed-CRL record epoch must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("privacy state statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkX509VerifiedCertificateNullifier {
            trust_anchor_record_digest,
            trust_anchor_record_epoch,
            certificate_policy_record_digest,
            certificate_policy_record_epoch,
            crl_record_digest,
            crl_record_epoch,
            statement_digest,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct the singleton authoritative state for one governed Orchard pool.
    pub(crate) fn orchard_pool_state(
        state: PrivacyOrchardPoolStateV1,
    ) -> Result<Self, &'static str> {
        state.validate()?;
        Ok(Self::OrchardPoolState { state })
    }

    /// Construct provenance for one consumed Orchard nullifier.
    pub(crate) fn orchard_verified_nullifier(
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("Orchard pool bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("privacy state statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::OrchardVerifiedNullifier {
            bootstrap_digest,
            statement_digest,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct the authoritative configuration for one proof-managed pool.
    pub(crate) fn proof_managed_pool_bootstrap(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        initial_root: PrivacyRootV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        let accumulator_state =
            PrivacyProofManagedPoolAccumulatorStateV1::bootstrap(&bootstrap, bootstrap_digest)?;
        Self::proof_managed_pool_state(
            bootstrap,
            bootstrap_digest,
            initial_root,
            accumulator_state,
            admitted_at_height,
        )
    }

    /// Construct a proof-managed pool record with its current native frontier.
    pub(crate) fn proof_managed_pool_state(
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        initial_root: PrivacyRootV1,
        accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        bootstrap
            .validate()
            .map_err(|_| "proof-managed pool bootstrap is invalid")?;
        let expected_digest = bootstrap
            .digest()
            .map_err(|_| "proof-managed pool bootstrap encoding failed")?;
        if bootstrap_digest.is_zero() || bootstrap_digest != expected_digest {
            return Err("proof-managed pool bootstrap digest is invalid");
        }
        let expected_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .map_err(|_| "proof-managed pool native accumulator is unavailable")?;
        if initial_root.is_zero() || initial_root != expected_root {
            return Err("proof-managed pool initial root is invalid");
        }
        accumulator_state.validate_against_bootstrap(&bootstrap, bootstrap_digest, initial_root)?;
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ProofManagedPoolBootstrap {
            bootstrap,
            bootstrap_digest,
            initial_root,
            accumulator_state,
            admitted_at_height,
        })
    }

    /// Construct provenance for one genesis commitment in a proof-managed pool.
    pub(crate) fn proof_managed_pool_bootstrap_commitment(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        position: u64,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("proof-managed pool bootstrap digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ProofManagedPoolBootstrapCommitment {
            bootstrap_digest,
            position,
            admitted_at_height,
        })
    }

    /// Construct provenance and the complete tuple for one FCMP++ genesis output.
    pub(crate) fn fcmp_bootstrap_output(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        output: PrivacyFcmpOutputTupleV1,
        position: u64,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("FCMP++ pool bootstrap digest must be non-zero");
        }
        fcmp_output_to_native_v1(output)?;
        if output.output_id().is_zero() {
            return Err("FCMP++ output id must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::FcmpBootstrapOutput {
            bootstrap_digest,
            output,
            position,
            admitted_at_height,
        })
    }

    /// Construct typed provenance for one proof-consumed nullifier.
    pub(crate) fn proof_managed_pool_verified_nullifier(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        nullifier_count: u32,
        output_count: u32,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("proof-managed pool bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("proof-managed pool statement digest must be non-zero");
        }
        if nullifier_count == 0 {
            return Err("proof-managed pool nullifier count must be non-zero");
        }
        if output_count == 0 {
            return Err("proof-managed pool output count must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ProofManagedPoolVerifiedNullifier {
            bootstrap_digest,
            statement_digest,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct typed provenance for one proof-produced output commitment.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn proof_managed_pool_verified_commitment(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        successor_epoch: u64,
        output_index: u32,
        append_position: u64,
        nullifier_count: u32,
        output_count: u32,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("proof-managed pool bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("proof-managed pool statement digest must be non-zero");
        }
        if successor_epoch < 2 {
            return Err("proof-managed output commitment epoch must follow its bootstrap");
        }
        if append_position == 0 {
            return Err("proof-managed output commitment append position must be non-zero");
        }
        if nullifier_count == 0 {
            return Err("proof-managed output commitment nullifier count must be non-zero");
        }
        if output_count == 0 {
            return Err("proof-managed output commitment output count must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ProofManagedPoolVerifiedCommitment {
            bootstrap_digest,
            statement_digest,
            successor_epoch,
            output_index,
            append_position,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct typed provenance and the complete tuple for one verified FCMP++ output.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fcmp_verified_output(
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        output: PrivacyFcmpOutputTupleV1,
        statement_digest: PrivacyStatementDigestV1,
        successor_epoch: u64,
        output_index: u32,
        append_position: u64,
        nullifier_count: u32,
        output_count: u32,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("FCMP++ pool bootstrap digest must be non-zero");
        }
        fcmp_output_to_native_v1(output)?;
        if output.output_id().is_zero() {
            return Err("FCMP++ output id must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("FCMP++ statement digest must be non-zero");
        }
        if successor_epoch < 2 {
            return Err("FCMP++ output epoch must follow its bootstrap");
        }
        if append_position == 0 {
            return Err("FCMP++ append position must be non-zero");
        }
        if nullifier_count == 0 {
            return Err("FCMP++ verified output key-image count must be non-zero");
        }
        if output_count == 0 {
            return Err("FCMP++ verified output count must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::FcmpVerifiedOutput {
            bootstrap_digest,
            output,
            statement_digest,
            successor_epoch,
            output_index,
            append_position,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct provenance for one consumed ZK-ACE replay nullifier.
    pub fn zk_ace_verified_authorization(
        policy_id: PrivacyPolicyIdV1,
        policy_record_digest: PrivacyZkAcePolicyRecordDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if policy_id.is_zero() {
            return Err("ZK-ACE policy id must be non-zero");
        }
        if policy_record_digest.is_zero() {
            return Err("ZK-ACE policy record digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("privacy state statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkAceVerifiedAuthorization {
            policy_id,
            policy_record_digest,
            statement_digest,
            admitted_at_height,
            action_index,
        })
    }

    /// Construct typed provenance for a governed ZK-AMS issuer record.
    pub fn zk_ams_governance(
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("ZK-AMS bootstrap digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkAmsGovernance {
            bootstrap_digest,
            admitted_at_height,
        })
    }

    /// Construct typed provenance for a proof-produced state item.
    pub fn zk_ams_verified_proof(
        bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("ZK-AMS bootstrap digest must be non-zero");
        }
        if statement_digest.is_zero() {
            return Err("privacy state statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self::ZkAmsVerifiedProof {
            bootstrap_digest,
            statement_digest,
            admitted_at_height,
            action_index,
        })
    }

    /// Validate persisted provenance restored from a snapshot.
    ///
    /// # Errors
    ///
    /// Rejects a zero statement digest or zero block height.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::ZkAcePolicyGovernance {
                policy,
                admitted_at_height,
            } => Self::zk_ace_policy_governance(policy.clone(), *admitted_at_height).map(|_| ()),
            Self::BootleLanternIssuerPolicyGovernance {
                policy,
                admitted_at_height,
            } => Self::bootle_lantern_issuer_policy_governance(policy.clone(), *admitted_at_height)
                .map(|_| ()),
            Self::VegaIssuerGovernance {
                record,
                admitted_at_height,
            } => Self::vega_issuer_governance(*record, *admitted_at_height).map(|_| ()),
            Self::ZkX509TrustAnchorGovernance {
                record,
                admitted_at_height,
            } => Self::zk_x509_trust_anchor_governance(*record, *admitted_at_height).map(|_| ()),
            Self::ZkX509CertificatePolicyGovernance {
                record,
                admitted_at_height,
            } => Self::zk_x509_certificate_policy_governance(record.clone(), *admitted_at_height)
                .map(|_| ()),
            Self::ZkX509CrlGovernance {
                record,
                admitted_at_height,
            } => Self::zk_x509_crl_governance(*record, *admitted_at_height).map(|_| ()),
            Self::ZkX509VerifiedCertificateNullifier {
                trust_anchor_record_digest,
                trust_anchor_record_epoch,
                certificate_policy_record_digest,
                certificate_policy_record_epoch,
                crl_record_digest,
                crl_record_epoch,
                statement_digest,
                admitted_at_height,
                action_index,
            } => Self::zk_x509_verified_certificate_nullifier(
                *trust_anchor_record_digest,
                *trust_anchor_record_epoch,
                *certificate_policy_record_digest,
                *certificate_policy_record_epoch,
                *crl_record_digest,
                *crl_record_epoch,
                *statement_digest,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::OrchardPoolState { state } => Self::orchard_pool_state(state.clone()).map(|_| ()),
            Self::OrchardVerifiedNullifier {
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
            } => Self::orchard_verified_nullifier(
                *bootstrap_digest,
                *statement_digest,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::ProofManagedPoolBootstrap {
                bootstrap,
                bootstrap_digest,
                initial_root,
                accumulator_state,
                admitted_at_height,
            } => Self::proof_managed_pool_state(
                bootstrap.clone(),
                *bootstrap_digest,
                *initial_root,
                accumulator_state.clone(),
                *admitted_at_height,
            )
            .map(|_| ()),
            Self::ProofManagedPoolBootstrapCommitment {
                bootstrap_digest,
                position,
                admitted_at_height,
            } => Self::proof_managed_pool_bootstrap_commitment(
                *bootstrap_digest,
                *position,
                *admitted_at_height,
            )
            .map(|_| ()),
            Self::FcmpBootstrapOutput {
                bootstrap_digest,
                output,
                position,
                admitted_at_height,
            } => Self::fcmp_bootstrap_output(
                *bootstrap_digest,
                *output,
                *position,
                *admitted_at_height,
            )
            .map(|_| ()),
            Self::ProofManagedPoolVerifiedNullifier {
                bootstrap_digest,
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
            } => Self::proof_managed_pool_verified_nullifier(
                *bootstrap_digest,
                *statement_digest,
                *nullifier_count,
                *output_count,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::ProofManagedPoolVerifiedCommitment {
                bootstrap_digest,
                statement_digest,
                successor_epoch,
                output_index,
                append_position,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
            } => Self::proof_managed_pool_verified_commitment(
                *bootstrap_digest,
                *statement_digest,
                *successor_epoch,
                *output_index,
                *append_position,
                *nullifier_count,
                *output_count,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::FcmpVerifiedOutput {
                bootstrap_digest,
                output,
                statement_digest,
                successor_epoch,
                output_index,
                append_position,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
            } => Self::fcmp_verified_output(
                *bootstrap_digest,
                *output,
                *statement_digest,
                *successor_epoch,
                *output_index,
                *append_position,
                *nullifier_count,
                *output_count,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::ZkAceVerifiedAuthorization {
                policy_id,
                policy_record_digest,
                statement_digest,
                admitted_at_height,
                action_index,
            } => Self::zk_ace_verified_authorization(
                *policy_id,
                *policy_record_digest,
                *statement_digest,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
            Self::ZkAmsGovernance {
                bootstrap_digest,
                admitted_at_height,
            } => Self::zk_ams_governance(*bootstrap_digest, *admitted_at_height).map(|_| ()),
            Self::ZkAmsVerifiedProof {
                bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
            } => Self::zk_ams_verified_proof(
                *bootstrap_digest,
                *statement_digest,
                *admitted_at_height,
                *action_index,
            )
            .map(|_| ()),
        }
    }

    /// Return the immutable ZK-AMS registry origin bound to this item, if any.
    #[must_use]
    pub const fn zk_ams_bootstrap_digest(&self) -> Option<PrivacyZkAmsRegistryBootstrapDigestV1> {
        match self {
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. } => None,
            Self::ZkAmsGovernance {
                bootstrap_digest, ..
            }
            | Self::ZkAmsVerifiedProof {
                bootstrap_digest, ..
            } => Some(*bootstrap_digest),
        }
    }

    /// Borrow the complete proof-managed pool bootstrap carried by this record.
    #[must_use]
    pub(crate) const fn proof_managed_pool_bootstrap_ref(
        &self,
    ) -> Option<(
        &PrivacyProofManagedPoolBootstrapV1,
        PrivacyProofManagedPoolBootstrapDigestV1,
        PrivacyRootV1,
        &PrivacyProofManagedPoolAccumulatorStateV1,
        u64,
    )> {
        match self {
            Self::ProofManagedPoolBootstrap {
                bootstrap,
                bootstrap_digest,
                initial_root,
                accumulator_state,
                admitted_at_height,
            } => Some((
                bootstrap,
                *bootstrap_digest,
                *initial_root,
                accumulator_state,
                *admitted_at_height,
            )),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Return the immutable proof-managed pool origin bound to this item.
    #[must_use]
    pub(crate) const fn proof_managed_pool_bootstrap_digest(
        &self,
    ) -> Option<PrivacyProofManagedPoolBootstrapDigestV1> {
        match self {
            Self::ProofManagedPoolBootstrap {
                bootstrap_digest, ..
            }
            | Self::ProofManagedPoolBootstrapCommitment {
                bootstrap_digest, ..
            }
            | Self::FcmpBootstrapOutput {
                bootstrap_digest, ..
            }
            | Self::ProofManagedPoolVerifiedNullifier {
                bootstrap_digest, ..
            }
            | Self::ProofManagedPoolVerifiedCommitment {
                bootstrap_digest, ..
            }
            | Self::FcmpVerifiedOutput {
                bootstrap_digest, ..
            } => Some(*bootstrap_digest),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the authoritative ZK-ACE policy carried by this record.
    #[must_use]
    pub const fn zk_ace_policy(&self) -> Option<&PrivacyZkAcePolicyRecordV1> {
        match self {
            Self::ZkAcePolicyGovernance { policy, .. } => Some(policy),
            Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the authoritative Bootle/Lantern issuer policy carried by this record.
    #[must_use]
    pub const fn bootle_lantern_issuer_policy(&self) -> Option<&BootleLanternIssuerPolicyV1> {
        match self {
            Self::BootleLanternIssuerPolicyGovernance { policy, .. } => Some(policy),
            Self::ZkAcePolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the immutable Vega issuer revision carried by this record.
    #[must_use]
    pub const fn vega_issuer(&self) -> Option<&PrivacyVegaIssuerRecordV1> {
        match self {
            Self::VegaIssuerGovernance { record, .. } => Some(record),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the complete authoritative Orchard pool state carried by this record.
    #[must_use]
    pub(crate) const fn orchard_pool_state_ref(&self) -> Option<&PrivacyOrchardPoolStateV1> {
        match self {
            Self::OrchardPoolState { state } => Some(state),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the immutable X.509 trust-anchor revision carried by this record.
    #[must_use]
    pub const fn zk_x509_trust_anchor(&self) -> Option<&PrivacyZkX509TrustAnchorRecordV1> {
        match self {
            Self::ZkX509TrustAnchorGovernance { record, .. } => Some(record),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the immutable X.509 certificate-policy revision carried by this record.
    #[must_use]
    pub const fn zk_x509_certificate_policy(
        &self,
    ) -> Option<&PrivacyZkX509CertificatePolicyRecordV1> {
        match self {
            Self::ZkX509CertificatePolicyGovernance { record, .. } => Some(record),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CrlGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }

    /// Borrow the current X.509 signed-CRL record carried by this state item.
    #[must_use]
    pub const fn zk_x509_crl(&self) -> Option<&PrivacyZkX509CrlRecordV1> {
        match self {
            Self::ZkX509CrlGovernance { record, .. } => Some(record),
            Self::ZkAcePolicyGovernance { .. }
            | Self::BootleLanternIssuerPolicyGovernance { .. }
            | Self::VegaIssuerGovernance { .. }
            | Self::ZkX509TrustAnchorGovernance { .. }
            | Self::ZkX509CertificatePolicyGovernance { .. }
            | Self::ZkX509VerifiedCertificateNullifier { .. }
            | Self::OrchardPoolState { .. }
            | Self::OrchardVerifiedNullifier { .. }
            | Self::ProofManagedPoolBootstrap { .. }
            | Self::ProofManagedPoolBootstrapCommitment { .. }
            | Self::FcmpBootstrapOutput { .. }
            | Self::ProofManagedPoolVerifiedNullifier { .. }
            | Self::ProofManagedPoolVerifiedCommitment { .. }
            | Self::FcmpVerifiedOutput { .. }
            | Self::ZkAceVerifiedAuthorization { .. }
            | Self::ZkAmsGovernance { .. }
            | Self::ZkAmsVerifiedProof { .. } => None,
        }
    }
}

/// Deterministic root-history admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyRootHistoryErrorV1 {
    /// A configured retained-root count was zero.
    #[error("privacy retained-root count must be non-zero")]
    ZeroRetention,
    /// An effect carried a malformed root key.
    #[error("invalid privacy root key: {0}")]
    InvalidKey(&'static str),
    /// The verifier emitted the same exact root more than once.
    #[error("privacy verifier emitted duplicate root {key:?}")]
    DuplicateAddedRoot {
        /// Duplicated exact root key.
        key: PrivacyRootKeyV1,
    },
    /// The exact root is already retained.
    #[error("privacy root is already retained: {key:?}")]
    ExistingRoot {
        /// Existing exact root key.
        key: PrivacyRootKeyV1,
    },
    /// One history has two different roots for the same epoch.
    #[error("privacy root epoch {epoch} already has a different root in {namespace:?}/{role:?}")]
    EpochConflict {
        /// Exact independent root namespace.
        namespace: PrivacyNamespaceV1,
        /// Exact accumulator role.
        role: PrivacyRootRoleV1,
        /// Conflicting epoch.
        epoch: u64,
    },
    /// A newly produced root does not advance its independent history.
    #[error(
        "privacy root epoch {added_epoch} does not advance latest retained epoch {latest_epoch} in {namespace:?}/{role:?}"
    )]
    NonMonotonicEpoch {
        /// Exact independent root namespace.
        namespace: PrivacyNamespaceV1,
        /// Exact accumulator role.
        role: PrivacyRootRoleV1,
        /// Latest retained epoch.
        latest_epoch: u64,
        /// Candidate new epoch.
        added_epoch: u64,
    },
    /// One action emitted more roots for a history than can be retained.
    #[error(
        "privacy action adds {count} roots to one history, exceeding retained-root count {max}"
    )]
    AddedRootsExceedRetention {
        /// Candidate addition count.
        count: u32,
        /// Configured retention count.
        max: u32,
    },
    /// A collection length or count could not be represented.
    #[error("privacy root-history count overflow")]
    CountOverflow,
}

/// Validate root additions and return the exact oldest retained keys to prune.
///
/// The returned plan is read-only and deterministic. Callers must complete all
/// other admission checks before applying the removals and additions in the
/// same state transaction.
///
/// # Errors
///
/// Rejects malformed, duplicate, existing, same-epoch-conflicting, stale, or
/// over-cap additions without mutating `roots`.
pub(crate) fn plan_privacy_root_history_update_v1(
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    additions: &[PrivacyRootKeyV1],
    retained_root_count: u32,
) -> Result<Vec<PrivacyRootKeyV1>, PrivacyRootHistoryErrorV1> {
    if retained_root_count == 0 {
        return Err(PrivacyRootHistoryErrorV1::ZeroRetention);
    }

    let mut seen = BTreeSet::new();
    let mut by_history =
        BTreeMap::<(PrivacyNamespaceV1, PrivacyRootRoleV1), Vec<PrivacyRootKeyV1>>::new();
    for key in additions {
        key.validate()
            .map_err(PrivacyRootHistoryErrorV1::InvalidKey)?;
        if !seen.insert(*key) {
            return Err(PrivacyRootHistoryErrorV1::DuplicateAddedRoot { key: *key });
        }
        by_history
            .entry((key.namespace(), key.role()))
            .or_default()
            .push(*key);
    }

    let retained = usize::try_from(retained_root_count)
        .map_err(|_| PrivacyRootHistoryErrorV1::CountOverflow)?;
    let mut removals = Vec::new();
    for ((namespace, role), mut added) in by_history {
        added.sort_unstable();
        let added_count =
            u32::try_from(added.len()).map_err(|_| PrivacyRootHistoryErrorV1::CountOverflow)?;
        if added_count > retained_root_count {
            return Err(PrivacyRootHistoryErrorV1::AddedRootsExceedRetention {
                count: added_count,
                max: retained_root_count,
            });
        }

        for adjacent in added.windows(2) {
            if adjacent[0].epoch() == adjacent[1].epoch() {
                return Err(PrivacyRootHistoryErrorV1::EpochConflict {
                    namespace,
                    role,
                    epoch: adjacent[0].epoch(),
                });
            }
        }

        let mut existing = roots
            .range(PrivacyRootKeyV1::history_range(namespace, role))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        existing.sort_unstable();
        for key in &added {
            if roots.get(key).is_some() {
                return Err(PrivacyRootHistoryErrorV1::ExistingRoot { key: *key });
            }
            if existing
                .iter()
                .any(|retained| retained.epoch() == key.epoch())
            {
                return Err(PrivacyRootHistoryErrorV1::EpochConflict {
                    namespace,
                    role,
                    epoch: key.epoch(),
                });
            }
        }
        if let (Some(latest), Some(first_added)) = (existing.last(), added.first())
            && first_added.epoch() <= latest.epoch()
        {
            return Err(PrivacyRootHistoryErrorV1::NonMonotonicEpoch {
                namespace,
                role,
                latest_epoch: latest.epoch(),
                added_epoch: first_added.epoch(),
            });
        }

        let total = existing
            .len()
            .checked_add(added.len())
            .ok_or(PrivacyRootHistoryErrorV1::CountOverflow)?;
        let prune_count = total.saturating_sub(retained);
        removals.extend(existing.into_iter().take(prune_count));
    }
    Ok(removals)
}

/// One atomic per-history retention-reduction plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrivacyRootRetentionReductionPlanV1 {
    pub(crate) head_key: PrivacyRootHeadKeyV1,
    pub(crate) new_anchor: PrivacyRootRetentionAnchorV1,
    pub(crate) removal_keys: Vec<PrivacyRootKeyV1>,
}

/// Protocols whose complete typed root histories support anchored prefix pruning.
///
/// Keep this closed list shared by policy prevalidation and block-start
/// application so a newly prunable history cannot be admitted without also
/// being reduced at the exact effective height.
pub(crate) const PRIVACY_ROOT_RETENTION_ANCHORED_PROTOCOLS_V1: [PrivacyProtocolIdV1; 7] = [
    PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
    PrivacyProtocolIdV1::IrohaZkAmsV1,
    PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
    PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
    PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
    PrivacyProtocolIdV1::PqMaspStarkV0,
];

const fn privacy_root_history_supports_retention_anchor_v1(
    protocol_id: PrivacyProtocolIdV1,
    role: PrivacyRootRoleV1,
) -> bool {
    matches!(
        (protocol_id, role),
        (
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyRootRoleV1::PgcAccountState
        ) | (
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyRootRoleV1::AccountRegistry
        ) | (
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyRootRoleV1::NoteCommitmentAnchor
        ) | (
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyRootRoleV1::CertificateAuthorityMembership
        ) | (
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyRootRoleV1::OutputSet
        ) | (
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyRootRoleV1::ProgramState
        ) | (
            PrivacyProtocolIdV1::PqMaspStarkV0,
            PrivacyRootRoleV1::NoteCommitmentAnchor
        )
    )
}

/// Plan exact per-history anchors and oldest roots for a retention decrease.
///
/// Histories remain independent by `(namespace, role)`. The function is
/// read-only and returns keys in deterministic scope/epoch order so governance
/// can apply the activation replacement and every prune in one transaction.
///
/// # Errors
///
/// Rejects a zero retention policy, malformed existing key, or an
/// unrepresentable count.
pub(crate) fn plan_privacy_root_retention_reduction_v1(
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    protocol_id: PrivacyProtocolIdV1,
    retained_root_count: u32,
) -> Result<Vec<PrivacyRootRetentionReductionPlanV1>, PrivacyRootHistoryErrorV1> {
    if retained_root_count == 0 {
        return Err(PrivacyRootHistoryErrorV1::ZeroRetention);
    }
    let retained = usize::try_from(retained_root_count)
        .map_err(|_| PrivacyRootHistoryErrorV1::CountOverflow)?;
    let mut by_history =
        BTreeMap::<(PrivacyNamespaceV1, PrivacyRootRoleV1), Vec<PrivacyRootKeyV1>>::new();
    for (key, _) in roots.iter() {
        key.validate()
            .map_err(PrivacyRootHistoryErrorV1::InvalidKey)?;
        if key.namespace().protocol_id() == protocol_id {
            by_history
                .entry((key.namespace(), key.role()))
                .or_default()
                .push(*key);
        }
    }

    let mut plans = Vec::new();
    for ((namespace, role), mut history) in by_history {
        history.sort_unstable();
        let prune_count = history.len().saturating_sub(retained);
        if prune_count == 0 {
            continue;
        }
        let removal_keys = history.into_iter().take(prune_count).collect::<Vec<_>>();
        let last_removed = *removal_keys
            .last()
            .expect("positive prune count produces a last removed root");
        let head_key = PrivacyRootHeadKeyV1::new(namespace, role)
            .map_err(PrivacyRootHistoryErrorV1::InvalidKey)?;
        let new_anchor =
            PrivacyRootRetentionAnchorV1::new(last_removed.epoch(), last_removed.root())
                .map_err(PrivacyRootHistoryErrorV1::InvalidKey)?;
        plans.push(PrivacyRootRetentionReductionPlanV1 {
            head_key,
            new_anchor,
            removal_keys,
        });
    }
    Ok(plans)
}

/// Validate that every unanchored history already satisfies a future retention cap.
///
/// PGC account-state, ZK-AMS registry, Orchard note-commitment, proof-managed
/// FCMP++/private-IVM/PQ-MASP, and typed X.509 CA/CRL histories carry exact
/// provenance plus a pruned-prefix anchor and can therefore be reduced
/// atomically at the scheduled height. Every other history must already fit the
/// future cap; governance cannot silently orphan it.
pub(crate) fn validate_unanchored_privacy_root_retention_v1(
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    retained_root_count: u32,
) -> Result<(), String> {
    if retained_root_count == 0 {
        return Err("privacy retained-root count must be non-zero".to_owned());
    }
    let retained = usize::try_from(retained_root_count)
        .map_err(|_| "privacy retained-root count cannot be represented".to_owned())?;
    let mut counts = BTreeMap::<(PrivacyNamespaceV1, PrivacyRootRoleV1), usize>::new();
    for (key, _) in roots.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy root key: {error}"))?;
        if privacy_root_history_supports_retention_anchor_v1(
            key.namespace().protocol_id(),
            key.role(),
        ) {
            continue;
        }
        let count = counts.entry((key.namespace(), key.role())).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| "privacy root-history count overflow".to_owned())?;
    }
    for ((namespace, role), count) in counts {
        if count > retained {
            return Err(format!(
                "unanchored privacy root history for {namespace:?}/{role:?} has {count} roots, exceeding scheduled retention {retained}"
            ));
        }
    }
    Ok(())
}

fn encode_storage_key<T: Encode>(value: &T, out: &mut String) {
    let encoded = norito::to_bytes(value).expect("fixed privacy storage keys always encode");
    json::write_json_string(&hex::encode_upper(encoded), out);
}

fn decode_storage_key<T: Decode + Encode>(encoded: &str) -> Result<T, json::Error> {
    let bytes = hex::decode(encoded)
        .map_err(|error| json::Error::Message(format!("invalid privacy key hex: {error}")))?;
    let key: T = norito::decode_from_bytes(&bytes)
        .map_err(|error| json::Error::Message(format!("invalid privacy key encoding: {error}")))?;
    let canonical_bytes = norito::to_bytes(&key).map_err(|error| {
        json::Error::Message(format!("failed to re-encode decoded privacy key: {error}"))
    })?;
    let canonical = hex::encode_upper(canonical_bytes);
    if encoded != canonical {
        return Err(json::Error::Message(
            "privacy storage key is not canonical uppercase exact Norito hex".into(),
        ));
    }
    Ok(key)
}

impl mv::json::JsonKeyCodec for PrivacyActivationKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        encode_storage_key(self, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        decode_storage_key(encoded)
    }
}

macro_rules! impl_validated_json_key {
    ($key:ty) => {
        impl mv::json::JsonKeyCodec for $key {
            fn encode_json_key(&self, out: &mut String) {
                encode_storage_key(self, out);
            }

            fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                let key: Self = decode_storage_key(encoded)?;
                key.validate().map_err(|message| {
                    json::Error::Message(format!("invalid privacy storage key: {message}"))
                })?;
                Ok(key)
            }
        }
    };
}

impl_validated_json_key!(PrivacyNullifierKeyV1);
impl_validated_json_key!(PrivacyCommitmentKeyV1);
impl_validated_json_key!(PrivacyRootKeyV1);
impl_validated_json_key!(PrivacyRootHeadKeyV1);
impl_validated_json_key!(PrivacyPgcAccountKeyV1);
impl_validated_json_key!(PrivacyPgcPoolInvariantKeyV1);

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BOOTLE_LANTERN_RING_DEGREE_V1,
        BootleLanternAllowedAttributeValuesV1, BootleLanternIssuerPolicyLifecycleV1,
        BootleLanternIssuerPublicMatrixV1, BootleLanternPolynomialV1, PrivacyActiveLifecycleV1,
        PrivacyAttributeDigestV1, PrivacyBootleLanternIssuerPolicyDigestV1,
        PrivacyCertificateKeyDigestV1, PrivacyChallengeV1, PrivacyCommitmentV1,
        PrivacyConsensusLimitsV1, PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1,
        PrivacyIssuerRegistryPolicyNamespaceV1, PrivacyNamespaceScopeV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyParameterNamespaceV1, PrivacyPolicyDigestV1,
        PrivacyPolicyIdV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyProposedLifecycleV1,
        PrivacyProtocolLifecycleV1, PrivacyRetiredLifecycleV1, PrivacyRootV1,
        PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1, PrivacySuspendedLifecycleV1,
        PrivacyTransactionIntentDigestV1, PrivacyTrustAnchorNamespaceV1,
        PrivacyTrustAnchorPolicyNamespaceV1, PrivacyVegaIssuerRecordDigestV1,
        PrivacyVegaMdlDigestAlgorithmV1, PrivacyVegaMdlNamespaceV1,
        PrivacyVegaMdlSignatureAlgorithmV1, PrivacyVerifierDigestV1, PrivacyX509CrlDerDigestV1,
        PrivacyX509CrlIssuerSpkiDigestV1, PrivacyX509ExtendedKeyUsageV1, PrivacyX509KeyUsageV1,
        PrivacyX509TrustStoreDigestV1, PrivacyZkAcePolicyLifecycleV1,
        PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordV1, PrivacyZkAmsKeyImageV1,
        PrivacyZkAmsRegistryIdV1, PrivacyZkX509CertificatePolicyRecordDigestV1,
        PrivacyZkX509CrlRecordDigestV1, PrivacyZkX509CrlRecordV1,
        PrivacyZkX509DisclosedAttributeV1, PrivacyZkX509TrustAnchorRecordDigestV1,
    };
    use iroha_data_model::{
        ChainId, account::AccountId, asset::AssetDefinitionId, domain::DomainId, name::Name,
    };
    use mv::{json::JsonKeyCodec, storage::Storage};
    use p256::{ProjectivePoint, Scalar, elliptic_curve::Group};

    use super::*;

    fn nonzero(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn p256_point(multiple: u64) -> PrivacyP256PointV1 {
        let compressed = crate::privacy_engines::p256::CompressedPointV1::from_projective(
            ProjectivePoint::generator() * Scalar::from(multiple),
        )
        .expect("non-zero generator multiple");
        PrivacyP256PointV1::new(*compressed.as_bytes())
    }

    fn vega_issuer_record(
        issuer_id: PrivacyIssuerIdV1,
        epoch: u64,
        key_multiple: u64,
        previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
        lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    ) -> PrivacyVegaIssuerRecordV1 {
        PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            epoch,
            p256_point(key_multiple),
            iroha_data_model::privacy::PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
            PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
            PrivacyVegaMdlDigestAlgorithmV1::Sha256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical Vega issuer record")
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn zk_ace_policy_id(index: u64) -> PrivacyPolicyIdV1 {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&index.to_le_bytes());
        bytes[8] = 1;
        PrivacyPolicyIdV1::new(bytes)
    }

    fn zk_ace_policy_record(policy_id: PrivacyPolicyIdV1) -> PrivacyZkAcePolicyRecordV1 {
        let mut allowlist = vec![account(11), account(12)];
        allowlist.sort_unstable();
        PrivacyZkAcePolicyRecordV1::new(
            policy_id,
            PrivacyCommitmentV1::new(nonzero(13)),
            PrivacyPolicyDigestV1::new(nonzero(14)),
            1,
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("privacy", "universal").expect("domain"),
                Name::from_str("asset").expect("asset name"),
            ),
            allowlist,
            PrivacyZkAcePolicyLifecycleV1::Active,
        )
        .expect("canonical ZK-ACE policy")
    }

    fn bootle_lantern_issuer_policy(
        issuer_byte: u8,
        policy_byte: u8,
        epoch: u64,
        lifecycle: BootleLanternIssuerPolicyLifecycleV1,
    ) -> BootleLanternIssuerPolicyV1 {
        let fixture_seed = usize::from(issuer_byte) + usize::from(policy_byte);
        let first_column = core::array::from_fn(|block| BootleLanternPolynomialV1 {
            coefficients: (0..BOOTLE_LANTERN_RING_DEGREE_V1)
                .map(|coefficient| {
                    u16::try_from(
                        (fixture_seed + block * BOOTLE_LANTERN_RING_DEGREE_V1 + coefficient)
                            % 12_288
                            + 1,
                    )
                    .expect("fixture residue fits u16")
                })
                .collect(),
        });
        let issuer_public_matrix =
            BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(first_column)
                .expect("canonical degree-512 multiplication matrix");
        let mut policy = BootleLanternIssuerPolicyV1 {
            issuer_id: PrivacyIssuerIdV1::new(nonzero(issuer_byte)),
            policy_id: PrivacyPolicyIdV1::new(nonzero(policy_byte)),
            epoch,
            lifecycle,
            issuer_parameter_id: PrivacyParameterIdV1::new(nonzero(0xB3)),
            issuer_parameter_digest: PrivacyParameterDigestV1::new([0; 32]),
            issuer_public_matrix,
            required_disclosure_bitmap: 0,
            allowed_values: vec![
                BootleLanternAllowedAttributeValuesV1 { values: Vec::new() };
                BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1
            ],
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        policy.issuer_parameter_digest = policy
            .computed_issuer_parameter_digest()
            .expect("canonical Bootle/Lantern issuer matrix encoding");
        policy.record_digest = policy
            .computed_record_digest()
            .expect("canonical Bootle/Lantern policy encoding");
        policy
            .validate()
            .expect("canonical Bootle/Lantern issuer policy");
        policy
    }

    fn validate_persisted_commitments(
        commitments: &Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    ) -> Result<(), String> {
        let activations =
            Storage::<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>::new();
        let pgc_accounts = Storage::<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>::new();
        let pgc_pool_invariants =
            Storage::<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>::new();
        let nullifiers = Storage::<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>::new();
        let roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
        let root_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
        validate_privacy_persisted_state_v1(
            &PrivacyConsensusPolicyV1::taira_default(),
            &activations.view(),
            &pgc_accounts.view(),
            &pgc_pool_invariants.view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
    }

    fn pgc_namespace(pool_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(pool_byte)),
            }),
        )
    }

    fn vega_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                parameter_id: PrivacyParameterIdV1::new(nonzero(40)),
            }),
        )
    }

    fn zk_ams_namespace(registry_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyNamespaceScopeV1::IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1 {
                issuer_id: PrivacyIssuerIdV1::new(nonzero(0x91)),
                registry_id: PrivacyZkAmsRegistryIdV1::new(nonzero(registry_byte)),
                policy_id: PrivacyPolicyIdV1::new(nonzero(0x92)),
            }),
        )
    }

    fn orchard_namespace(pool_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(pool_byte)),
            }),
        )
    }

    fn fcmp_namespace(pool_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(pool_byte)),
            }),
        )
    }

    fn fcmp_output_tuple(seed: u64) -> PrivacyFcmpOutputTupleV1 {
        use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};

        let point = |multiple| {
            (ED25519_BASEPOINT_POINT * Scalar::from(multiple))
                .compress()
                .to_bytes()
        };
        PrivacyFcmpOutputTupleV1 {
            output_key: point(seed),
            linking_tag_generator: point(seed.checked_add(1).expect("test scalar")),
            amount_commitment: point(seed.checked_add(2).expect("test scalar")),
        }
    }

    fn sorted_fcmp_output_tuples(seeds: &[u64]) -> Vec<PrivacyFcmpOutputTupleV1> {
        let mut outputs = seeds
            .iter()
            .copied()
            .map(fcmp_output_tuple)
            .collect::<Vec<_>>();
        outputs.sort_unstable_by_key(|output| output.output_id());
        outputs
    }

    fn ivm_private_note_namespace(pool_byte: u8, program_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyNamespaceScopeV1::PoolProgram(
                iroha_data_model::privacy::PrivacyPoolProgramNamespaceV1 {
                    pool_id: PrivacyPoolIdV1::new(nonzero(pool_byte)),
                    program_id: iroha_data_model::privacy::PrivacyProgramIdV1::new(nonzero(
                        program_byte,
                    )),
                },
            ),
        )
    }

    fn pq_masp_namespace(pool_byte: u8) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::PqMaspStarkV0,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(pool_byte)),
            }),
        )
    }

    fn x509_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
                trust_anchor_id: PrivacyIssuerIdV1::new(nonzero(41)),
                policy_id: PrivacyPolicyIdV1::new(nonzero(42)),
            }),
        )
    }

    fn x509_ca_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyNamespaceScopeV1::TrustAnchor(PrivacyTrustAnchorNamespaceV1 {
                trust_anchor_id: PrivacyIssuerIdV1::new(nonzero(41)),
            }),
        )
    }

    fn x509_root_key(role: PrivacyRootRoleV1, epoch: u64, root_byte: u8) -> PrivacyRootKeyV1 {
        assert_eq!(role, PrivacyRootRoleV1::CertificateAuthorityMembership);
        let namespace = x509_ca_namespace();
        PrivacyRootKeyV1::new(
            namespace,
            role,
            epoch,
            PrivacyRootV1::new(nonzero(root_byte)),
        )
        .expect("valid root key")
    }

    fn indexed_nonzero(domain: u8, index: u64) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[0] = domain;
        bytes[1..9].copy_from_slice(&index.to_le_bytes());
        bytes
    }

    fn x509_trust_anchor_record(
        trust_anchor_id: PrivacyIssuerIdV1,
        epoch: u64,
        trust_store_byte: u8,
        previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509TrustAnchorRecordV1 {
        let ca_membership_root_epoch = match lifecycle {
            PrivacyZkX509RecordLifecycleV1::Active => epoch,
            PrivacyZkX509RecordLifecycleV1::Revoked => epoch.saturating_sub(1),
        };
        PrivacyZkX509TrustAnchorRecordV1::new(
            trust_anchor_id,
            epoch,
            PrivacyX509TrustStoreDigestV1::new(nonzero(trust_store_byte)),
            PrivacyRootV1::new(nonzero(trust_store_byte.wrapping_add(1))),
            ca_membership_root_epoch,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 trust-anchor record")
    }

    fn x509_certificate_policy_record(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        epoch: u64,
        policy_byte: u8,
        disclosures: Vec<u8>,
        previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509CertificatePolicyRecordV1 {
        PrivacyZkX509CertificatePolicyRecordV1::new(
            trust_anchor_id,
            policy_id,
            epoch,
            PrivacyPolicyDigestV1::new(nonzero(policy_byte)),
            PrivacyX509KeyUsageV1 {
                digital_signature: true.into(),
                content_commitment: false.into(),
                key_encipherment: false.into(),
                key_agreement: false.into(),
            },
            vec![
                PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
            ],
            disclosures,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 certificate-policy record")
    }

    fn x509_crl_record(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        epoch: u64,
        crl_number: u64,
        _root_byte: u8,
        previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509CrlRecordV1 {
        PrivacyZkX509CrlRecordV1::new(
            trust_anchor_id,
            policy_id,
            epoch,
            crl_number,
            PrivacyX509CrlDerDigestV1::new(indexed_nonzero(0xC1, epoch)),
            PrivacyX509CrlIssuerSpkiDigestV1::new(nonzero(0xC2)),
            1_749_999_900 + epoch,
            1_750_000_600 + epoch,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 signed-CRL record")
    }

    fn insert_x509_trust_anchor(
        commitments: &mut Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        record: PrivacyZkX509TrustAnchorRecordV1,
        admitted_at_height: u64,
    ) {
        let key = PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision(
            record.trust_anchor_id,
            record.record_epoch,
        )
        .expect("trust-anchor revision key");
        let value =
            PrivacyStateItemRecordV1::zk_x509_trust_anchor_governance(record, admitted_at_height)
                .expect("trust-anchor state record");
        commitments.insert(key, value);
    }

    fn insert_x509_certificate_policy(
        commitments: &mut Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        record: PrivacyZkX509CertificatePolicyRecordV1,
        admitted_at_height: u64,
    ) {
        let key = PrivacyCommitmentKeyV1::zk_x509_certificate_policy_revision(
            record.trust_anchor_id,
            record.policy_id,
            record.record_epoch,
        )
        .expect("certificate-policy revision key");
        let value = PrivacyStateItemRecordV1::zk_x509_certificate_policy_governance(
            record,
            admitted_at_height,
        )
        .expect("certificate-policy state record");
        commitments.insert(key, value);
    }

    fn insert_x509_crl(
        commitments: &mut Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        record: PrivacyZkX509CrlRecordV1,
        admitted_at_height: u64,
    ) {
        let key = PrivacyCommitmentKeyV1::zk_x509_crl_current(
            record.trust_anchor_id,
            record.certificate_policy_id,
        )
        .expect("current signed-CRL key");
        let value = PrivacyStateItemRecordV1::zk_x509_crl_governance(record, admitted_at_height)
            .expect("signed-CRL state record");
        commitments.insert(key, value);
    }

    fn x509_root_provenance(
        key: PrivacyRootKeyV1,
        trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
        admitted_at_height: u64,
    ) -> PrivacyRootProvenanceV1 {
        let publication = PrivacyRootPublicationV1 {
            namespace: key.namespace(),
            role: key.role(),
            epoch: key.epoch(),
            root: key.root(),
        };
        match publication.role {
            PrivacyRootRoleV1::CertificateAuthorityMembership => {
                PrivacyRootProvenanceV1::zk_x509_ca_governance(
                    publication.digest().expect("root publication digest"),
                    publication.namespace,
                    publication.epoch,
                    publication.root,
                    trust_anchor,
                    admitted_at_height,
                )
                .expect("X.509 CA-root provenance")
            }
            _ => panic!("X.509 root fixture requires a closed X.509 role"),
        }
    }

    fn root_provenance() -> PrivacyRootProvenanceV1 {
        PrivacyRootProvenanceV1::verified_proof(
            PrivacyStatementDigestV1::new(nonzero(50)),
            1,
            0,
            1,
            PrivacyRootV1::new(nonzero(49)),
        )
        .expect("valid root provenance")
    }

    fn pgc_accounts(count: u8) -> Vec<PrivacyPgcAccountV1> {
        let point = |multiple: u64| {
            let compressed = crate::privacy_engines::p256::CompressedPointV1::from_projective(
                ProjectivePoint::generator() * Scalar::from(multiple),
            )
            .expect("non-zero generator multiple");
            PrivacyP256PointV1::new(*compressed.as_bytes())
        };
        let mut public_keys = (1..=u64::from(count)).map(point).collect::<Vec<_>>();
        public_keys.sort_unstable();
        public_keys
            .into_iter()
            .enumerate()
            .map(|(index, public_key)| {
                let multiple = u64::try_from(index).expect("small fixture index") + 100;
                PrivacyPgcAccountV1 {
                    public_key,
                    encrypted_balance: PrivacyP256CiphertextV1 {
                        left: point(multiple),
                        right: point(multiple + 100),
                    },
                }
            })
            .collect()
    }

    fn activation_proposal() -> PrivacyProtocolActivationRecordV1 {
        crate::privacy_profiles::compiled_privacy_profile_v1(
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        )
        .expect("compiled VeRange profile")
        .activation_record(PrivacyProtocolLifecycleV1::Proposed(
            PrivacyProposedLifecycleV1 {
                proposed_at_height: 1_000,
                activate_at_height: 1_300,
            },
        ))
    }

    struct PgcPersistedFixture {
        activations: Storage<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
        pgc_accounts: Storage<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>,
        pgc_pool_invariants: Storage<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>,
        nullifiers: Storage<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>,
        commitments: Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        roots: Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
        root_heads: Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
        namespace: PrivacyNamespaceV1,
        invariant_key: PrivacyPgcPoolInvariantKeyV1,
        account_keys: Vec<PrivacyPgcAccountKeyV1>,
        root_key: PrivacyRootKeyV1,
        head_key: PrivacyRootHeadKeyV1,
        root: PrivacyRootV1,
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        provenance: PrivacyRootProvenanceV1,
    }

    impl PgcPersistedFixture {
        fn validate(&self) -> Result<(), String> {
            validate_privacy_persisted_state_v1(
                &PrivacyConsensusPolicyV1::taira_default(),
                &self.activations.view(),
                &self.pgc_accounts.view(),
                &self.pgc_pool_invariants.view(),
                &self.nullifiers.view(),
                &self.commitments.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }

        fn replace_root_and_head(
            &mut self,
            epoch: u64,
            root: PrivacyRootV1,
            provenance: PrivacyRootProvenanceV1,
        ) {
            self.root_key = PrivacyRootKeyV1::new(
                self.namespace,
                PrivacyRootRoleV1::PgcAccountState,
                epoch,
                root,
            )
            .expect("replacement root key");
            self.roots = Storage::new();
            self.roots.insert(self.root_key, provenance);
            self.root_heads = Storage::new();
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(epoch, root, provenance, None)
                    .expect("replacement root head"),
            );
        }

        fn invariant(&self) -> PrivacyPgcPoolInvariantV1 {
            *self
                .pgc_pool_invariants
                .view()
                .get(&self.invariant_key)
                .expect("fixture pool invariant")
        }

        fn account_table(&self) -> Vec<PrivacyPgcAccountV1> {
            self.pgc_accounts
                .view()
                .iter()
                .map(|(key, state)| PrivacyPgcAccountV1 {
                    public_key: key.public_key(),
                    encrypted_balance: state.encrypted_balance(),
                })
                .collect()
        }

        fn advance_with_retention(&mut self, retained_root_count: u32) {
            let head = *self
                .root_heads
                .view()
                .get(&self.head_key)
                .expect("fixture current head");
            let next_epoch = head.epoch().checked_add(1).expect("fixture epoch advance");
            let mut statement_bytes = [0xD0; 32];
            statement_bytes[..8].copy_from_slice(&next_epoch.to_be_bytes());
            let statement_digest = PrivacyStatementDigestV1::new(statement_bytes);
            let account_provenance =
                PrivacyPgcAccountProvenanceV1::verified_proof(statement_digest, next_epoch + 10, 0)
                    .expect("successor account provenance");
            let invariant = self.invariant();
            let account_table = self.account_table();
            let next_root = compute_privacy_pgc_account_state_root_v1(
                self.namespace,
                next_epoch,
                invariant.total_supply(),
                &account_table,
            )
            .expect("successor account root");
            let root_provenance = PrivacyRootProvenanceV1::verified_pgc_successor(
                statement_digest,
                next_epoch + 10,
                0,
                head.epoch(),
                head.root(),
                invariant
                    .digest(self.namespace)
                    .expect("pool invariant digest"),
            )
            .expect("successor root provenance");
            let next_key = PrivacyRootKeyV1::new(
                self.namespace,
                PrivacyRootRoleV1::PgcAccountState,
                next_epoch,
                next_root,
            )
            .expect("successor root key");
            let removals = plan_privacy_root_history_update_v1(
                &self.roots.view(),
                &[next_key],
                retained_root_count,
            )
            .expect("successor history plan");
            let retention_anchor = removals
                .last()
                .map(|key| {
                    PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root())
                        .expect("pruned root anchor")
                })
                .or(head.retention_anchor());

            let retained_roots = self
                .roots
                .view()
                .iter()
                .filter(|(key, _)| !removals.contains(key))
                .map(|(key, provenance)| (*key, *provenance))
                .collect::<Vec<_>>();
            self.roots = retained_roots.into_iter().collect();
            for key in &self.account_keys {
                let encrypted_balance = self
                    .pgc_accounts
                    .view()
                    .get(key)
                    .expect("fixture account")
                    .encrypted_balance();
                self.pgc_accounts.insert(
                    *key,
                    PrivacyPgcAccountStateV1::new(
                        encrypted_balance,
                        next_epoch,
                        account_provenance,
                    )
                    .expect("successor account state"),
                );
            }
            self.roots.insert(next_key, root_provenance);
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(
                    next_epoch,
                    next_root,
                    root_provenance,
                    retention_anchor,
                )
                .expect("successor head"),
            );
            self.root_key = next_key;
            self.root = next_root;
            self.provenance = root_provenance;
        }

        fn tighten_retention(&mut self, retained_root_count: u32) {
            let head = *self
                .root_heads
                .view()
                .get(&self.head_key)
                .expect("fixture current head");
            let plans = plan_privacy_root_retention_reduction_v1(
                &self.roots.view(),
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                retained_root_count,
            )
            .expect("retention reduction plan");
            if plans.is_empty() {
                return;
            }
            assert_eq!(plans.len(), 1);
            let plan = &plans[0];
            assert_eq!(plan.head_key, self.head_key);
            let retained_roots = self
                .roots
                .view()
                .iter()
                .filter(|(key, _)| !plan.removal_keys.contains(key))
                .map(|(key, provenance)| (*key, *provenance))
                .collect::<Vec<_>>();
            self.roots = retained_roots.into_iter().collect();
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(
                    head.epoch(),
                    head.root(),
                    head.provenance(),
                    Some(plan.new_anchor),
                )
                .expect("retention-tightened head"),
            );
        }

        fn load_with_retention(
            &self,
            retained_root_count: u32,
        ) -> Result<PrivacyPgcPoolSnapshotV1, String> {
            load_privacy_pgc_pool_snapshot_v1(
                self.namespace,
                retained_root_count,
                &self.pgc_accounts.view(),
                &self.pgc_pool_invariants.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }
    }

    fn pgc_persisted_fixture() -> PgcPersistedFixture {
        let namespace = pgc_namespace(20);
        let bootstrap_digest = PrivacyPgcAccountBootstrapDigestV1::new(nonzero(0xB1));
        let bootstrap_proof_digest = PrivacyPgcBootstrapProofDigestV1::new(nonzero(0xB2));
        let total_supply = 160;
        let epoch = PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1;
        let account_table = pgc_accounts(16);
        let root = compute_privacy_pgc_account_state_root_v1(
            namespace,
            epoch,
            total_supply,
            &account_table,
        )
        .expect("canonical fixture root");
        let account_provenance =
            PrivacyPgcAccountProvenanceV1::bootstrap(bootstrap_digest, bootstrap_proof_digest, 9)
                .expect("bootstrap account provenance");
        let provenance = PrivacyRootProvenanceV1::verified_bootstrap(
            bootstrap_digest,
            bootstrap_proof_digest,
            9,
        )
        .expect("bootstrap root provenance");

        let profile = crate::privacy_profiles::compiled_privacy_profile_v1(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        )
        .expect("compiled Anonymous PGC profile");
        let activation = profile.activation_record(PrivacyProtocolLifecycleV1::Active(
            PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            },
        ));
        let mut activations = Storage::new();
        activations.insert(
            PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1),
            activation,
        );

        let mut pgc_accounts = Storage::new();
        let mut account_keys = Vec::with_capacity(account_table.len());
        for account in account_table {
            let key =
                PrivacyPgcAccountKeyV1::new(namespace, account.public_key).expect("account key");
            pgc_accounts.insert(
                key,
                PrivacyPgcAccountStateV1::new(account.encrypted_balance, epoch, account_provenance)
                    .expect("account state"),
            );
            account_keys.push(key);
        }
        let invariant_key =
            PrivacyPgcPoolInvariantKeyV1::new(namespace).expect("pool invariant key");
        let mut pgc_pool_invariants = Storage::new();
        pgc_pool_invariants.insert(
            invariant_key,
            PrivacyPgcPoolInvariantV1::new(
                total_supply,
                root,
                bootstrap_digest,
                bootstrap_proof_digest,
            )
            .expect("pool invariant"),
        );
        let root_key =
            PrivacyRootKeyV1::new(namespace, PrivacyRootRoleV1::PgcAccountState, epoch, root)
                .expect("root key");
        let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::PgcAccountState)
            .expect("head key");
        let mut roots = Storage::new();
        roots.insert(root_key, provenance);
        let mut root_heads = Storage::new();
        root_heads.insert(
            head_key,
            PrivacyRootHeadRecordV1::new(epoch, root, provenance, None).expect("root head"),
        );

        PgcPersistedFixture {
            activations,
            pgc_accounts,
            pgc_pool_invariants,
            nullifiers: Storage::new(),
            commitments: Storage::new(),
            roots,
            root_heads,
            namespace,
            invariant_key,
            account_keys,
            root_key,
            head_key,
            root,
            bootstrap_digest,
            bootstrap_proof_digest,
            provenance,
        }
    }

    fn expect_pgc_persisted_error(mutate: impl FnOnce(&mut PgcPersistedFixture), expected: &str) {
        let mut fixture = pgc_persisted_fixture();
        mutate(&mut fixture);
        let error = fixture
            .validate()
            .expect_err("adversarial persisted state must reject");
        assert!(
            error.contains(expected),
            "expected `{expected}` in persisted-state rejection, got `{error}`"
        );
    }

    struct OrchardPersistedFixture {
        activations: Storage<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
        pgc_accounts: Storage<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>,
        pgc_pool_invariants: Storage<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>,
        nullifiers: Storage<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>,
        commitments: Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        roots: Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
        root_heads: Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
        namespace: PrivacyNamespaceV1,
        bootstrap_digest: PrivacyOrchardPoolBootstrapDigestV1,
        state_key: PrivacyCommitmentKeyV1,
        head_key: PrivacyRootHeadKeyV1,
    }

    impl OrchardPersistedFixture {
        fn validate(&self) -> Result<(), String> {
            validate_privacy_persisted_state_v1(
                &PrivacyConsensusPolicyV1::taira_default(),
                &self.activations.view(),
                &self.pgc_accounts.view(),
                &self.pgc_pool_invariants.view(),
                &self.nullifiers.view(),
                &self.commitments.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }

        fn load_with_retention(
            &self,
            retained_root_count: u32,
        ) -> Result<PrivacyOrchardPoolSnapshotV1, String> {
            load_privacy_orchard_pool_snapshot_v1(
                self.namespace,
                retained_root_count,
                &self.commitments.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }

        fn state(&self) -> PrivacyOrchardPoolStateV1 {
            self.commitments
                .view()
                .get(&self.state_key)
                .and_then(PrivacyStateItemRecordV1::orchard_pool_state_ref)
                .expect("fixture Orchard pool state")
                .clone()
        }

        fn set_state(&mut self, state: PrivacyOrchardPoolStateV1) {
            self.commitments.insert(
                self.state_key,
                PrivacyStateItemRecordV1::orchard_pool_state(state)
                    .expect("canonical Orchard pool state record"),
            );
        }

        fn advance_with_retention(
            &mut self,
            retained_root_count: u32,
            note_commitments: &[[u8; 32]],
        ) {
            let snapshot = self
                .load_with_retention(retained_root_count)
                .expect("coherent Orchard predecessor");
            let successor = snapshot
                .derive_successor(note_commitments)
                .expect("canonical Orchard commitments");
            let next_epoch = successor.epoch();
            let mut statement_bytes = [0xC0; 32];
            statement_bytes[..8].copy_from_slice(&next_epoch.to_be_bytes());
            let statement_digest = PrivacyStatementDigestV1::new(statement_bytes);
            let root_provenance = PrivacyRootProvenanceV1::orchard_pool_successor(
                self.bootstrap_digest,
                statement_digest,
                next_epoch + 10,
                0,
                snapshot.current_epoch(),
                snapshot.current_root(),
            )
            .expect("successor root provenance");
            let next_key = PrivacyRootKeyV1::new(
                self.namespace,
                PrivacyRootRoleV1::NoteCommitmentAnchor,
                next_epoch,
                successor.root(),
            )
            .expect("successor root key");
            let removals = plan_privacy_root_history_update_v1(
                &self.roots.view(),
                &[next_key],
                retained_root_count,
            )
            .expect("successor history plan");
            let predecessor_head = *self
                .root_heads
                .view()
                .get(&self.head_key)
                .expect("fixture predecessor head");
            let retention_anchor = removals
                .last()
                .map(|key| {
                    PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root())
                        .expect("pruned Orchard root anchor")
                })
                .or(predecessor_head.retention_anchor());
            let retained = self
                .roots
                .view()
                .iter()
                .filter(|(key, _)| !removals.contains(key))
                .map(|(key, provenance)| (*key, *provenance))
                .collect::<Vec<_>>();
            self.roots = retained.into_iter().collect();
            self.roots.insert(next_key, root_provenance);
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(
                    next_epoch,
                    successor.root(),
                    root_provenance,
                    retention_anchor,
                )
                .expect("successor root head"),
            );
            self.set_state(successor);
        }
    }

    fn orchard_persisted_fixture() -> OrchardPersistedFixture {
        let namespace = orchard_namespace(0xA7);
        let bootstrap_digest = PrivacyOrchardPoolBootstrapDigestV1::new(nonzero(0xA8));
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("orchard_asset").expect("asset name"),
        );
        let state = PrivacyOrchardPoolStateV1::bootstrap(
            bootstrap_digest,
            asset_definition_id,
            AssetBalanceScope::Global,
            account(0xA9),
        )
        .expect("canonical Orchard empty state");
        let root = state.root();
        let provenance = PrivacyRootProvenanceV1::orchard_pool_bootstrap(bootstrap_digest, 9)
            .expect("Orchard bootstrap provenance");
        let root_key = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1,
            root,
        )
        .expect("Orchard bootstrap root key");
        let head_key =
            PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
                .expect("Orchard head key");
        let state_key =
            PrivacyCommitmentKeyV1::orchard_pool_state(namespace).expect("Orchard state key");

        let profile = crate::privacy_profiles::compiled_privacy_profile_v1(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        )
        .expect("compiled Orchard profile");
        let activation = profile.activation_record(PrivacyProtocolLifecycleV1::Active(
            PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            },
        ));
        let mut activations = Storage::new();
        activations.insert(
            PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::OrchardHalo2ActionsV1),
            activation,
        );
        let mut commitments = Storage::new();
        commitments.insert(
            state_key,
            PrivacyStateItemRecordV1::orchard_pool_state(state).expect("Orchard state record"),
        );
        let mut roots = Storage::new();
        roots.insert(root_key, provenance);
        let mut root_heads = Storage::new();
        root_heads.insert(
            head_key,
            PrivacyRootHeadRecordV1::new(
                PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1,
                root,
                provenance,
                None,
            )
            .expect("Orchard bootstrap head"),
        );

        OrchardPersistedFixture {
            activations,
            pgc_accounts: Storage::new(),
            pgc_pool_invariants: Storage::new(),
            nullifiers: Storage::new(),
            commitments,
            roots,
            root_heads,
            namespace,
            bootstrap_digest,
            state_key,
            head_key,
        }
    }

    fn expect_orchard_persisted_error(
        mutate: impl FnOnce(&mut OrchardPersistedFixture),
        expected: &str,
    ) {
        let mut fixture = orchard_persisted_fixture();
        mutate(&mut fixture);
        let error = fixture
            .validate()
            .expect_err("adversarial Orchard state must reject");
        assert!(
            error.contains(expected),
            "expected `{expected}` in Orchard persisted-state rejection, got `{error}`"
        );
    }

    struct ProofManagedPersistedFixture {
        commitments: Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        roots: Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
        root_heads: Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        namespace: PrivacyNamespaceV1,
        config_key: PrivacyCommitmentKeyV1,
        head_key: PrivacyRootHeadKeyV1,
        initial_root: PrivacyRootV1,
    }

    impl ProofManagedPersistedFixture {
        fn load(&self) -> Result<PrivacyProofManagedPoolSnapshotV1, String> {
            load_privacy_proof_managed_pool_snapshot_v1(
                self.namespace,
                PrivacyConsensusPolicyV1::taira_default().admission_retained_root_count(),
                &self.commitments.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }

        fn remove_commitment(&mut self, key: PrivacyCommitmentKeyV1) {
            self.commitments = self
                .commitments
                .view()
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .map(|(candidate, record)| (*candidate, record.clone()))
                .collect();
        }

        fn advance(&mut self, outputs: &[PrivacyCommitmentV1]) {
            let snapshot = self.load().expect("coherent proof-managed predecessor");
            let successor = snapshot
                .derive_note_successor(outputs)
                .expect("canonical proof-managed append");
            let mut statement_bytes = [0xD0; 32];
            statement_bytes[..8].copy_from_slice(&successor.epoch().to_be_bytes());
            let statement_digest = PrivacyStatementDigestV1::new(statement_bytes);
            let output_count = u32::try_from(outputs.len()).expect("output count");
            for (output_index, commitment) in outputs.iter().enumerate() {
                let key = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
                    self.namespace,
                    *commitment,
                )
                .expect("output key");
                let output_index = u32::try_from(output_index).expect("output index");
                let append_position = snapshot
                    .output_count()
                    .checked_add(u64::from(output_index))
                    .expect("append position");
                let item = PrivacyStateItemRecordV1::proof_managed_pool_verified_commitment(
                    self.bootstrap_digest,
                    statement_digest,
                    successor.epoch(),
                    output_index,
                    append_position,
                    1,
                    output_count,
                    10 + successor.epoch(),
                    0,
                )
                .expect("verified commitment");
                self.commitments.insert(key, item);
            }
            self.commitments.insert(
                self.config_key,
                PrivacyStateItemRecordV1::proof_managed_pool_state(
                    self.bootstrap.clone(),
                    self.bootstrap_digest,
                    self.initial_root,
                    PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(successor.clone()),
                    7,
                )
                .expect("successor config"),
            );
            let provenance = PrivacyRootProvenanceV1::proof_managed_pool_successor(
                self.bootstrap_digest,
                self.namespace.protocol_id(),
                statement_digest,
                1,
                output_count,
                10 + successor.epoch(),
                0,
                snapshot.current_epoch(),
                snapshot.current_root(),
            )
            .expect("successor provenance");
            let root_key = PrivacyRootKeyV1::new(
                self.namespace,
                self.bootstrap.root_role(),
                successor.epoch(),
                successor.root(),
            )
            .expect("successor root key");
            self.roots.insert(root_key, provenance);
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(successor.epoch(), successor.root(), provenance, None)
                    .expect("successor head"),
            );
        }
    }

    fn proof_managed_note_persisted_fixture(
        protocol_id: PrivacyProtocolIdV1,
    ) -> ProofManagedPersistedFixture {
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("private_note_state").expect("asset name"),
        );
        let initial_note_commitments = vec![
            PrivacyCommitmentV1::new(nonzero(0xB4)),
            PrivacyCommitmentV1::new(nonzero(0xB5)),
        ];
        let bootstrap = match protocol_id {
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProofManagedPoolBootstrapV1::IrohaIvmPrivateNoteStarkV1(
                    iroha_data_model::privacy::PrivacyIvmPrivateNotePoolBootstrapV1 {
                        pool_id: PrivacyPoolIdV1::new(nonzero(0xB1)),
                        asset_definition_id,
                        public_balance_scope:
                            iroha_data_model::asset::AssetBalanceScope::Global,
                        reserve_account: account(0xB2),
                        program_id: iroha_data_model::privacy::PrivacyProgramIdV1::new(nonzero(
                            0xB3,
                        )),
                        initial_note_commitments,
                    },
                )
            }
            PrivacyProtocolIdV1::PqMaspStarkV0 => {
                PrivacyProofManagedPoolBootstrapV1::PqMaspStarkV0(
                    iroha_data_model::privacy::PrivacyPqMaspPoolBootstrapV1 {
                        pool_id: PrivacyPoolIdV1::new(nonzero(0xB1)),
                        asset_definition_id,
                        initial_note_commitments,
                    },
                )
            }
            _ => panic!("note fixture accepts only private-IVM or PQ-MASP"),
        };
        let namespace = bootstrap.namespace();
        let bootstrap_digest = bootstrap.digest().expect("bootstrap digest");
        let initial_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .expect("native initial root");
        let config_key =
            PrivacyCommitmentKeyV1::proof_managed_pool_config(namespace).expect("config key");
        let mut commitments = Storage::new();
        commitments.insert(
            config_key,
            PrivacyStateItemRecordV1::proof_managed_pool_bootstrap(
                bootstrap.clone(),
                bootstrap_digest,
                initial_root,
                7,
            )
            .expect("bootstrap config"),
        );
        for (position, commitment) in bootstrap
            .initial_note_commitments()
            .expect("private-note bootstrap commitments")
            .iter()
            .enumerate()
        {
            let genesis_item = PrivacyStateItemRecordV1::proof_managed_pool_bootstrap_commitment(
                bootstrap_digest,
                u64::try_from(position).expect("genesis position"),
                7,
            )
            .expect("genesis item");
            commitments.insert(
                PrivacyCommitmentKeyV1::proof_managed_pool_commitment(namespace, *commitment)
                    .expect("genesis key"),
                genesis_item,
            );
        }
        let provenance = PrivacyRootProvenanceV1::proof_managed_pool_bootstrap(
            bootstrap_digest,
            namespace.protocol_id(),
            7,
        )
        .expect("bootstrap provenance");
        let root_key = PrivacyRootKeyV1::new(namespace, bootstrap.root_role(), 1, initial_root)
            .expect("bootstrap root key");
        let head_key =
            PrivacyRootHeadKeyV1::new(namespace, bootstrap.root_role()).expect("head key");
        let mut roots = Storage::new();
        roots.insert(root_key, provenance);
        let mut root_heads = Storage::new();
        root_heads.insert(
            head_key,
            PrivacyRootHeadRecordV1::new(1, initial_root, provenance, None)
                .expect("bootstrap head"),
        );
        ProofManagedPersistedFixture {
            commitments,
            roots,
            root_heads,
            bootstrap,
            bootstrap_digest,
            namespace,
            config_key,
            head_key,
            initial_root,
        }
    }

    fn proof_managed_persisted_fixture() -> ProofManagedPersistedFixture {
        proof_managed_note_persisted_fixture(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
    }

    fn pq_masp_persisted_fixture() -> ProofManagedPersistedFixture {
        proof_managed_note_persisted_fixture(PrivacyProtocolIdV1::PqMaspStarkV0)
    }

    struct FcmpPersistedFixture {
        commitments: Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        roots: Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
        root_heads: Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
        bootstrap: PrivacyProofManagedPoolBootstrapV1,
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
        namespace: PrivacyNamespaceV1,
        config_key: PrivacyCommitmentKeyV1,
        head_key: PrivacyRootHeadKeyV1,
        initial_root: PrivacyRootV1,
    }

    impl FcmpPersistedFixture {
        fn load(&self) -> Result<PrivacyProofManagedPoolSnapshotV1, String> {
            load_privacy_proof_managed_pool_snapshot_v1(
                self.namespace,
                PrivacyConsensusPolicyV1::taira_default().admission_retained_root_count(),
                &self.commitments.view(),
                &self.roots.view(),
                &self.root_heads.view(),
            )
        }

        fn remove_output(&mut self, output: PrivacyFcmpOutputTupleV1) {
            let key = PrivacyCommitmentKeyV1::fcmp_output(self.namespace, output.output_id())
                .expect("typed FCMP++ output key");
            self.commitments = self
                .commitments
                .view()
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .map(|(candidate, record)| (*candidate, record.clone()))
                .collect();
        }

        fn advance(&mut self, outputs: &[PrivacyFcmpOutputTupleV1]) {
            let snapshot = self.load().expect("coherent FCMP++ predecessor");
            let successor = snapshot
                .derive_fcmp_successor(outputs)
                .expect("canonical FCMP++ append");
            let mut statement_bytes = [0xE0; 32];
            statement_bytes[..8].copy_from_slice(&successor.epoch().to_be_bytes());
            let statement_digest = PrivacyStatementDigestV1::new(statement_bytes);
            let output_count = u32::try_from(outputs.len()).expect("output count");
            for (output_index, output) in outputs.iter().copied().enumerate() {
                let key = PrivacyCommitmentKeyV1::fcmp_output(self.namespace, output.output_id())
                    .expect("typed output key");
                let output_index = u32::try_from(output_index).expect("output index");
                let append_position = snapshot
                    .output_count()
                    .checked_add(u64::from(output_index))
                    .expect("append position");
                let record = PrivacyStateItemRecordV1::fcmp_verified_output(
                    self.bootstrap_digest,
                    output,
                    statement_digest,
                    successor.epoch(),
                    output_index,
                    append_position,
                    1,
                    output_count,
                    20 + successor.epoch(),
                    0,
                )
                .expect("verified FCMP++ output");
                self.commitments.insert(key, record);
            }
            self.commitments.insert(
                self.config_key,
                PrivacyStateItemRecordV1::proof_managed_pool_state(
                    self.bootstrap.clone(),
                    self.bootstrap_digest,
                    self.initial_root,
                    PrivacyProofManagedPoolAccumulatorStateV1::Fcmp(successor.clone()),
                    7,
                )
                .expect("FCMP++ successor config"),
            );
            let provenance = PrivacyRootProvenanceV1::proof_managed_pool_successor(
                self.bootstrap_digest,
                self.namespace.protocol_id(),
                statement_digest,
                1,
                output_count,
                20 + successor.epoch(),
                0,
                snapshot.current_epoch(),
                snapshot.current_root(),
            )
            .expect("FCMP++ successor provenance");
            let root = successor.root().history_commitment();
            let root_key = PrivacyRootKeyV1::new(
                self.namespace,
                PrivacyRootRoleV1::OutputSet,
                successor.epoch(),
                root,
            )
            .expect("FCMP++ successor root key");
            self.roots.insert(root_key, provenance);
            self.root_heads.insert(
                self.head_key,
                PrivacyRootHeadRecordV1::new(successor.epoch(), root, provenance, None)
                    .expect("FCMP++ successor head"),
            );
        }
    }

    fn fcmp_persisted_fixture() -> FcmpPersistedFixture {
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("fcmp_state").expect("asset name"),
        );
        let bootstrap = PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(
            iroha_data_model::privacy::PrivacyFcmpPoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(0xD1)),
                asset_definition_id,
                initial_outputs: sorted_fcmp_output_tuples(&[11, 21]),
            },
        );
        bootstrap.validate().expect("canonical FCMP++ bootstrap");
        let namespace = bootstrap.namespace();
        let bootstrap_digest = bootstrap.digest().expect("FCMP++ bootstrap digest");
        let initial_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .expect("native FCMP++ initial root");
        let config_key =
            PrivacyCommitmentKeyV1::proof_managed_pool_config(namespace).expect("config key");
        let mut commitments = Storage::new();
        commitments.insert(
            config_key,
            PrivacyStateItemRecordV1::proof_managed_pool_bootstrap(
                bootstrap.clone(),
                bootstrap_digest,
                initial_root,
                7,
            )
            .expect("FCMP++ bootstrap config"),
        );
        for (position, output) in bootstrap
            .initial_fcmp_outputs()
            .expect("FCMP++ genesis outputs")
            .iter()
            .copied()
            .enumerate()
        {
            let record = PrivacyStateItemRecordV1::fcmp_bootstrap_output(
                bootstrap_digest,
                output,
                u64::try_from(position).expect("genesis position"),
                7,
            )
            .expect("FCMP++ genesis provenance");
            commitments.insert(
                PrivacyCommitmentKeyV1::fcmp_output(namespace, output.output_id())
                    .expect("FCMP++ genesis key"),
                record,
            );
        }
        let provenance = PrivacyRootProvenanceV1::proof_managed_pool_bootstrap(
            bootstrap_digest,
            namespace.protocol_id(),
            7,
        )
        .expect("FCMP++ bootstrap provenance");
        let root_key =
            PrivacyRootKeyV1::new(namespace, PrivacyRootRoleV1::OutputSet, 1, initial_root)
                .expect("FCMP++ bootstrap root key");
        let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::OutputSet)
            .expect("FCMP++ head key");
        let mut roots = Storage::new();
        roots.insert(root_key, provenance);
        let mut root_heads = Storage::new();
        root_heads.insert(
            head_key,
            PrivacyRootHeadRecordV1::new(1, initial_root, provenance, None)
                .expect("FCMP++ bootstrap head"),
        );
        FcmpPersistedFixture {
            commitments,
            roots,
            root_heads,
            bootstrap,
            bootstrap_digest,
            namespace,
            config_key,
            head_key,
            initial_root,
        }
    }

    fn validate_proof_managed_fixture_maps(
        protocol_id: PrivacyProtocolIdV1,
        nullifiers: &Storage<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>,
        commitments: &Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
        roots: &Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
        root_heads: &Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
    ) -> Result<(), String> {
        let activation = crate::privacy_profiles::compiled_privacy_profile_v1(protocol_id)
            .expect("compiled proof-managed profile")
            .activation_record(PrivacyProtocolLifecycleV1::Active(
                PrivacyActiveLifecycleV1 {
                    proposed_at_height: 1,
                    activated_at_height: 2,
                    state_since_height: 2,
                },
            ));
        let mut activations = Storage::new();
        activations.insert(PrivacyActivationKeyV1::new(protocol_id), activation);
        validate_privacy_persisted_state_v1(
            &PrivacyConsensusPolicyV1::taira_default(),
            &activations.view(),
            &Storage::<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>::new().view(),
            &Storage::<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>::new().view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
    }

    #[test]
    fn orchard_bootstrap_is_canonical_authoritative_and_restart_safe() {
        let mut fixture = orchard_persisted_fixture();
        fixture.validate().expect("coherent Orchard bootstrap");
        let snapshot = fixture
            .load_with_retention(
                PrivacyConsensusPolicyV1::taira_default().admission_retained_root_count(),
            )
            .expect("bounded Orchard snapshot");
        assert_eq!(snapshot.namespace(), fixture.namespace);
        assert_eq!(
            snapshot.current_epoch(),
            PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1
        );
        assert_eq!(
            snapshot.current_root().into_bytes(),
            crate::privacy_engines::orchard::orchard_empty_root_v1()
        );
        assert_eq!(snapshot.state().tree_size(), 0);
        assert_eq!(snapshot.state().leaf(), None);
        assert!(snapshot.state().ommers().is_empty());
        assert_eq!(snapshot.bootstrap_digest(), fixture.bootstrap_digest);
        assert!(snapshot.contains_retained_anchor(
            PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1,
            snapshot.current_root()
        ));
        assert_eq!(snapshot.retention_anchor(), None);

        let activations = norito::json::to_json(&fixture.activations).expect("encode activations");
        let nullifiers = norito::json::to_json(&fixture.nullifiers).expect("encode nullifiers");
        let commitments = norito::json::to_json(&fixture.commitments).expect("encode commitments");
        let roots = norito::json::to_json(&fixture.roots).expect("encode roots");
        let root_heads = norito::json::to_json(&fixture.root_heads).expect("encode root heads");
        fixture.activations = norito::json::from_json(&activations).expect("restore activations");
        fixture.nullifiers = norito::json::from_json(&nullifiers).expect("restore nullifiers");
        fixture.commitments = norito::json::from_json(&commitments).expect("restore commitments");
        fixture.roots = norito::json::from_json(&roots).expect("restore roots");
        fixture.root_heads = norito::json::from_json(&root_heads).expect("restore root heads");
        fixture
            .validate()
            .expect("restored Orchard state preserves every invariant");
    }

    #[test]
    fn orchard_public_dependencies_are_typed_bounded_and_fail_closed() {
        let fixture = orchard_persisted_fixture();
        let state = fixture.state();
        let references = load_privacy_orchard_pool_references_v1(&fixture.commitments.view())
            .expect("canonical Orchard public dependencies");
        assert_eq!(
            references,
            vec![PrivacyOrchardPoolReferenceV1 {
                namespace: fixture.namespace,
                asset_definition_id: state.asset_definition_id().clone(),
                reserve_account: state.reserve_account().clone(),
            }]
        );

        let mut accounts = Storage::<AccountId, ()>::new();
        let mut asset_definitions = Storage::<AssetDefinitionId, ()>::new();
        accounts.insert(state.reserve_account().clone(), ());
        asset_definitions.insert(state.asset_definition_id().clone(), ());
        validate_privacy_orchard_public_dependencies_v1(
            &fixture.commitments.view(),
            &accounts.view(),
            &asset_definitions.view(),
        )
        .expect("both exact public dependencies exist");

        let error = validate_privacy_orchard_public_dependencies_v1(
            &fixture.commitments.view(),
            &Storage::<AccountId, ()>::new().view(),
            &asset_definitions.view(),
        )
        .expect_err("missing reserve account must reject restored state");
        assert!(error.contains("references missing reserve account"));
        assert!(error.contains(&state.reserve_account().to_string()));

        let error = validate_privacy_orchard_public_dependencies_v1(
            &fixture.commitments.view(),
            &accounts.view(),
            &Storage::<AssetDefinitionId, ()>::new().view(),
        )
        .expect_err("missing asset definition must reject restored state");
        assert!(error.contains("references missing asset definition"));
        assert!(error.contains(&state.asset_definition_id().to_string()));
    }

    #[test]
    fn orchard_compact_state_rehashes_and_rejects_impossible_transition_shapes() {
        let fixture = orchard_persisted_fixture();
        let state = fixture.state();
        state.validate().expect("canonical empty state");

        let successor = crate::privacy_engines::orchard::append_orchard_commitments_v1(
            state.tree_size(),
            state.leaf(),
            state.ommers(),
            state.root().into_bytes(),
            &[[0; 32], [1; 32]],
        )
        .expect("two canonical Orchard leaves");
        let advanced = state
            .advance(successor.clone())
            .expect("one two-action transition");
        assert_eq!(advanced.epoch(), state.epoch() + 1);
        assert_eq!(advanced.tree_size(), 2);
        assert_eq!(advanced.asset_definition_id(), state.asset_definition_id());
        assert_eq!(advanced.reserve_account(), state.reserve_account());
        advanced.validate().expect("successor rehashes exactly");

        let no_op = crate::privacy_engines::orchard::append_orchard_commitments_v1(
            state.tree_size(),
            state.leaf(),
            state.ommers(),
            state.root().into_bytes(),
            &[],
        )
        .expect("empty native append is representable but not a ledger transition");
        assert_eq!(
            state.advance(no_op),
            Err("Orchard successor must append one or two actions")
        );
        let three = crate::privacy_engines::orchard::append_orchard_commitments_v1(
            state.tree_size(),
            state.leaf(),
            state.ommers(),
            state.root().into_bytes(),
            &[[0; 32], [1; 32], [2; 32]],
        )
        .expect("three leaves fit the tree but exceed the compiled action bound");
        assert_eq!(
            state.advance(three),
            Err("Orchard successor must append one or two actions")
        );

        let mut corruptions = Vec::new();
        let mut changed = state.clone();
        changed.bootstrap_digest = PrivacyOrchardPoolBootstrapDigestV1::new([0; 32]);
        corruptions.push(changed);
        let mut changed = state.clone();
        changed.epoch = 0;
        corruptions.push(changed);
        let mut changed = state.clone();
        changed.root = PrivacyRootV1::new([0; 32]);
        corruptions.push(changed);
        let mut changed = state.clone();
        changed.tree_size = 1;
        corruptions.push(changed);
        let mut changed = advanced.clone();
        changed.epoch = PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1;
        corruptions.push(changed);
        let mut changed = advanced.clone();
        changed.tree_size = 3;
        corruptions.push(changed);
        let mut changed = advanced.clone();
        let mut changed_root = changed.root().into_bytes();
        changed_root[0] ^= 1;
        changed.root = PrivacyRootV1::new(changed_root);
        corruptions.push(changed);
        let mut changed = advanced.clone();
        changed.leaf = Some([u8::MAX; 32]);
        corruptions.push(changed);
        let mut changed = advanced;
        changed.ommers.clear();
        corruptions.push(changed);
        for corrupted in corruptions {
            assert!(
                corrupted.validate().is_err(),
                "every malformed or impossible compact state must fail closed"
            );
        }
    }

    #[test]
    fn orchard_persisted_state_rejects_orphans_wrong_roles_and_cross_origin_state() {
        expect_orchard_persisted_error(
            |fixture| fixture.activations = Storage::new(),
            "unregistered protocol",
        );
        expect_orchard_persisted_error(
            |fixture| fixture.commitments = Storage::new(),
            "no authoritative compact frontier",
        );
        expect_orchard_persisted_error(
            |fixture| fixture.roots = Storage::new(),
            "has no retained history",
        );
        expect_orchard_persisted_error(
            |fixture| fixture.root_heads = Storage::new(),
            "has no current head",
        );
        expect_orchard_persisted_error(
            |fixture| {
                fixture.commitments.insert(
                    fixture.state_key,
                    PrivacyStateItemRecordV1::zk_ams_verified_proof(
                        PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(0x31)),
                        PrivacyStatementDigestV1::new(nonzero(0x32)),
                        3,
                        0,
                    )
                    .expect("locally valid wrong-role record"),
                );
            },
            "wrong-role provenance",
        );
        expect_orchard_persisted_error(
            |fixture| {
                let mut state = fixture.state();
                state.bootstrap_digest = PrivacyOrchardPoolBootstrapDigestV1::new(nonzero(0x33));
                fixture.set_state(state);
            },
            "origin differs from its pool state",
        );
        expect_orchard_persisted_error(
            |fixture| {
                let snapshot = fixture
                    .load_with_retention(
                        PrivacyConsensusPolicyV1::taira_default().admission_retained_root_count(),
                    )
                    .expect("bootstrap snapshot");
                let successor = snapshot
                    .derive_successor(&[[0; 32]])
                    .expect("valid but uncommitted successor");
                fixture.set_state(successor);
            },
            "compact frontier does not equal its current root head",
        );
        expect_orchard_persisted_error(
            |fixture| {
                let head = *fixture
                    .root_heads
                    .view()
                    .get(&fixture.head_key)
                    .expect("bootstrap head");
                fixture.root_heads.insert(
                    fixture.head_key,
                    PrivacyRootHeadRecordV1::new(
                        head.epoch(),
                        PrivacyRootV1::new(nonzero(0x34)),
                        head.provenance(),
                        None,
                    )
                    .expect("locally valid mismatched head"),
                );
            },
            "does not equal latest",
        );
        expect_orchard_persisted_error(
            |fixture| {
                let key = *fixture
                    .roots
                    .view()
                    .iter()
                    .next()
                    .map(|(key, _)| key)
                    .expect("bootstrap root");
                fixture.roots.insert(
                    key,
                    PrivacyRootProvenanceV1::governance(
                        PrivacyRootPublicationDigestV1::new(nonzero(0x35)),
                        9,
                    )
                    .expect("locally valid wrong origin"),
                );
            },
            "invalid provenance",
        );
    }

    #[test]
    fn orchard_nullifiers_are_canonical_pool_scoped_origin_bound_and_restart_safe() {
        let mut fixture = orchard_persisted_fixture();
        let nullifier = [0; 32];
        let key = PrivacyNullifierKeyV1::orchard_nullifier(fixture.namespace, nullifier)
            .expect("canonical Orchard nullifier");
        let record = PrivacyStateItemRecordV1::orchard_verified_nullifier(
            fixture.bootstrap_digest,
            PrivacyStatementDigestV1::new(nonzero(0x41)),
            10,
            0,
        )
        .expect("verified nullifier record");
        fixture.nullifiers.insert(key, record.clone());
        fixture.validate().expect("origin-bound nullifier");
        let encoded = norito::json::to_json(&fixture.nullifiers).expect("encode nullifiers");
        fixture.nullifiers = norito::json::from_json(&encoded).expect("restore nullifiers");
        fixture
            .validate()
            .expect("canonical nullifier survives restart");

        assert!(
            PrivacyNullifierKeyV1::orchard_nullifier(fixture.namespace, [u8::MAX; 32]).is_err(),
            "non-canonical Pallas-base encodings must reject"
        );
        assert!(
            PrivacyNullifierKeyV1::orchard_nullifier(vega_namespace(), nullifier).is_err(),
            "a nullifier cannot be relabeled into another protocol namespace"
        );

        let mut cross_origin = orchard_persisted_fixture();
        cross_origin.nullifiers.insert(
            key,
            PrivacyStateItemRecordV1::orchard_verified_nullifier(
                PrivacyOrchardPoolBootstrapDigestV1::new(nonzero(0x42)),
                PrivacyStatementDigestV1::new(nonzero(0x43)),
                10,
                0,
            )
            .expect("locally valid cross-origin record"),
        );
        assert!(
            cross_origin
                .validate()
                .expect_err("cross-origin nullifier")
                .contains("wrong-role or cross-bootstrap")
        );

        let mut wrong_role = orchard_persisted_fixture();
        wrong_role.nullifiers.insert(
            key,
            PrivacyStateItemRecordV1::zk_ams_verified_proof(
                PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(0x44)),
                PrivacyStatementDigestV1::new(nonzero(0x45)),
                10,
                0,
            )
            .expect("locally valid wrong-role record"),
        );
        assert!(
            wrong_role
                .validate()
                .expect_err("wrong-role nullifier")
                .contains("wrong-role or cross-bootstrap")
        );

        let mut orphan = orchard_persisted_fixture();
        let orphan_namespace = orchard_namespace(0x46);
        let orphan_key = PrivacyNullifierKeyV1::orchard_nullifier(orphan_namespace, nullifier)
            .expect("canonical orphan key");
        orphan.nullifiers.insert(orphan_key, record);
        assert!(
            orphan
                .validate()
                .expect_err("orphan nullifier")
                .contains("no authoritative note-commitment pool")
        );
    }

    #[test]
    fn orchard_retained_window_rejects_gaps_duplicates_forgery_and_bad_anchors() {
        let rolled = || {
            let mut fixture = orchard_persisted_fixture();
            for index in 0..5 {
                fixture
                    .advance_with_retention(3, &[if index % 2 == 0 { [0; 32] } else { [1; 32] }]);
            }
            let snapshot = fixture
                .load_with_retention(3)
                .expect("valid retained Orchard window");
            assert_eq!(snapshot.current_epoch(), 6);
            assert_eq!(
                snapshot
                    .retention_anchor()
                    .expect("pruned prefix anchor")
                    .epoch(),
                3
            );
            assert_eq!(fixture.roots.view().iter().count(), 3);
            fixture
        };

        let mut restart = rolled();
        let commitments =
            norito::json::to_json(&restart.commitments).expect("encode compact state");
        let roots = norito::json::to_json(&restart.roots).expect("encode retained roots");
        let heads = norito::json::to_json(&restart.root_heads).expect("encode root heads");
        restart.commitments = norito::json::from_json(&commitments).expect("restore compact state");
        restart.roots = norito::json::from_json(&roots).expect("restore retained roots");
        restart.root_heads = norito::json::from_json(&heads).expect("restore root heads");
        restart
            .load_with_retention(3)
            .expect("retained window survives exact restart");

        let mut fixture = rolled();
        let keys = fixture
            .roots
            .view()
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        fixture.roots = fixture
            .roots
            .view()
            .iter()
            .filter(|(key, _)| **key != keys[1])
            .map(|(key, provenance)| (*key, *provenance))
            .collect();
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("middle gap")
                .contains("gap or forged parent")
        );

        let mut fixture = rolled();
        let (first_key, first_provenance) = fixture
            .roots
            .view()
            .iter()
            .next()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("first retained root");
        let duplicate_key = PrivacyRootKeyV1::new(
            fixture.namespace,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            first_key.epoch(),
            PrivacyRootV1::new(nonzero(0x51)),
        )
        .expect("same-epoch alternate root");
        fixture.roots.insert(duplicate_key, first_provenance);
        assert!(
            fixture
                .load_with_retention(4)
                .expect_err("duplicate epoch")
                .contains("duplicate epoch")
        );

        let mut fixture = rolled();
        let retained = fixture
            .roots
            .view()
            .iter()
            .map(|(key, provenance)| (*key, *provenance))
            .collect::<Vec<_>>();
        fixture.roots.insert(retained[0].0, retained[1].1);
        fixture.roots.insert(retained[1].0, retained[0].1);
        assert!(
            fixture.load_with_retention(3).is_err(),
            "reordered successor provenance must reject"
        );

        let mut fixture = rolled();
        let (first_key, first_provenance) = fixture
            .roots
            .view()
            .iter()
            .next()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("first retained root");
        let PrivacyRootProvenanceV1::OrchardPoolSuccessor {
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            ..
        } = first_provenance
        else {
            panic!("rolled Orchard prefix starts with a successor");
        };
        fixture.roots.insert(
            first_key,
            PrivacyRootProvenanceV1::orchard_pool_successor(
                fixture.bootstrap_digest,
                statement_digest,
                admitted_at_height,
                action_index,
                parent_epoch,
                PrivacyRootV1::new(nonzero(0x52)),
            )
            .expect("locally valid forged parent"),
        );
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("forged pruned-prefix parent")
                .contains("exact pruned-prefix anchor")
        );

        let mut fixture = rolled();
        let (last_key, last_provenance) = fixture
            .roots
            .view()
            .iter()
            .last()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("latest retained root");
        let PrivacyRootProvenanceV1::OrchardPoolSuccessor {
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
            ..
        } = last_provenance
        else {
            panic!("latest Orchard root is a successor");
        };
        let forged = PrivacyRootProvenanceV1::orchard_pool_successor(
            PrivacyOrchardPoolBootstrapDigestV1::new(nonzero(0x53)),
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        )
        .expect("locally valid cross-origin successor");
        fixture.roots.insert(last_key, forged);
        fixture.root_heads.insert(
            fixture.head_key,
            PrivacyRootHeadRecordV1::new(
                last_key.epoch(),
                last_key.root(),
                forged,
                fixture
                    .root_heads
                    .view()
                    .get(&fixture.head_key)
                    .expect("rolled head")
                    .retention_anchor(),
            )
            .expect("cross-origin head"),
        );
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("cross-origin successor")
                .contains("different pool bootstrap")
        );

        for anchor in [
            None,
            Some(
                PrivacyRootRetentionAnchorV1::new(3, PrivacyRootV1::new(nonzero(0x54)))
                    .expect("wrong-root anchor"),
            ),
            Some(
                PrivacyRootRetentionAnchorV1::new(2, PrivacyRootV1::new(nonzero(0x55)))
                    .expect("stale anchor"),
            ),
            Some(
                PrivacyRootRetentionAnchorV1::new(4, PrivacyRootV1::new(nonzero(0x56)))
                    .expect("advanced anchor"),
            ),
        ] {
            let mut fixture = rolled();
            let head = *fixture
                .root_heads
                .view()
                .get(&fixture.head_key)
                .expect("rolled head");
            fixture.root_heads.insert(
                fixture.head_key,
                PrivacyRootHeadRecordV1::new(head.epoch(), head.root(), head.provenance(), anchor)
                    .expect("locally valid forged anchor"),
            );
            assert!(
                fixture.load_with_retention(3).is_err(),
                "missing, wrong-root, stale, and advanced anchors must reject"
            );
        }

        let mut fixture = orchard_persisted_fixture();
        let bootstrap_key = *fixture
            .roots
            .view()
            .iter()
            .next()
            .map(|(key, _)| key)
            .expect("bootstrap key");
        fixture.roots.insert(
            bootstrap_key,
            PrivacyRootProvenanceV1::governance(
                PrivacyRootPublicationDigestV1::new(nonzero(0x57)),
                9,
            )
            .expect("wrong-role origin"),
        );
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("governance-forged Orchard origin")
                .contains("invalid provenance")
        );
    }

    #[test]
    fn due_activation_plan_preserves_schedule_across_height_jump_and_restart() {
        let proposal = activation_proposal();
        let key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mut activations = Storage::new();
        activations.insert(key, proposal);

        assert!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_299)
                .expect("valid registry")
                .is_empty()
        );

        let promotions = plan_due_privacy_activation_promotions_v1(&activations.view(), 1_337)
            .expect("height jump promotes due proposal");
        assert_eq!(promotions.len(), 1);
        assert_eq!(
            promotions[0].1.lifecycle,
            PrivacyProtocolLifecycleV1::Active(
                iroha_data_model::privacy::PrivacyActiveLifecycleV1 {
                    proposed_at_height: 1_000,
                    activated_at_height: 1_300,
                    state_since_height: 1_300,
                }
            )
        );

        for (key, record) in promotions {
            activations.insert(key, record);
        }
        assert!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_338)
                .expect("restored active registry")
                .is_empty(),
            "promotion must happen exactly once"
        );
    }

    #[test]
    fn malformed_activation_aborts_entire_promotion_plan_without_mutation() {
        let proposal = activation_proposal();
        let valid_key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mismatched_key = PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::PqMaspStarkV0);
        let mut activations = Storage::new();
        activations.insert(valid_key, proposal);
        activations.insert(mismatched_key, proposal);

        assert!(matches!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_300),
            Err(PrivacyActivationPromotionErrorV1::KeyProtocolMismatch(_))
        ));
        assert_eq!(
            activations
                .view()
                .get(&valid_key)
                .expect("valid record remains")
                .lifecycle,
            proposal.lifecycle,
            "read-only planning cannot partially promote an earlier record"
        );
    }

    #[test]
    fn promotion_rejects_a_missed_protocol_limit_schedule() {
        let mut proposal = activation_proposal();
        let mut next_limits = proposal.protocol_limits;
        let iroha_data_model::privacy::PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            ref mut limits,
        ) = next_limits
        else {
            unreachable!("VeRange fixture")
        };
        limits.max_aggregation_count -= 1;
        proposal.pending_protocol_limits_tightening = Some(
            iroha_data_model::privacy::PrivacyProtocolLimitsTighteningV1 {
                scheduled_at_height: 1_000,
                effective_at_height: 1_300,
                next_limits,
            },
        );
        let key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mut activations = Storage::new();
        activations.insert(key, proposal);

        assert!(matches!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_301),
            Err(PrivacyActivationPromotionErrorV1::MissedProtocolLimits(error))
                if error.protocol_id == proposal.protocol_id
                    && error.effective_at_height == 1_300
                    && error.incoming_height == 1_301
        ));
    }

    #[test]
    fn protocol_limit_schedule_applies_with_lifecycle_only_at_exact_height() {
        let mut proposal = activation_proposal();
        let mut next_limits = proposal.protocol_limits;
        let iroha_data_model::privacy::PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            ref mut limits,
        ) = next_limits
        else {
            unreachable!("VeRange fixture")
        };
        limits.max_aggregation_count -= 1;
        proposal.pending_protocol_limits_tightening = Some(
            iroha_data_model::privacy::PrivacyProtocolLimitsTighteningV1 {
                scheduled_at_height: 1_000,
                effective_at_height: 1_300,
                next_limits,
            },
        );
        let key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mut activations = Storage::new();
        activations.insert(key, proposal);

        assert!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_299)
                .expect("valid pre-effective registry")
                .is_empty()
        );
        let promotions = plan_due_privacy_activation_promotions_v1(&activations.view(), 1_300)
            .expect("lifecycle and protocol limits apply atomically");
        assert_eq!(promotions.len(), 1);
        let promoted = promotions[0].1;
        assert_eq!(promoted.protocol_limits, next_limits);
        assert_eq!(promoted.pending_protocol_limits_tightening, None);
        assert_eq!(
            promoted.lifecycle,
            PrivacyProtocolLifecycleV1::Active(
                iroha_data_model::privacy::PrivacyActiveLifecycleV1 {
                    proposed_at_height: 1_000,
                    activated_at_height: 1_300,
                    state_since_height: 1_300,
                }
            )
        );

        assert!(
            validate_privacy_activations_at_committed_height_v1(&activations.view(), 999)
                .expect_err("a snapshot cannot contain a future-admitted schedule")
                .contains("scheduled-at")
        );
        validate_privacy_activations_at_committed_height_v1(&activations.view(), 1_299)
            .expect("effective E is valid in committed E-1");
        assert!(
            validate_privacy_activations_at_committed_height_v1(&activations.view(), 1_300)
                .expect_err("effective E cannot remain pending in committed E")
                .contains("not after committed height")
        );
    }

    #[test]
    fn restored_activation_lifecycle_is_exact_at_committed_height() {
        let validate = |record: PrivacyProtocolActivationRecordV1, committed_height| {
            let key = PrivacyActivationKeyV1::new(record.protocol_id);
            let mut activations = Storage::new();
            activations.insert(key, record);
            validate_privacy_activations_at_committed_height_v1(
                &activations.view(),
                committed_height,
            )
        };

        let proposal = activation_proposal();
        assert!(
            validate(proposal, 999)
                .expect_err("future proposal admission must reject")
                .contains("proposal height")
        );
        validate(proposal, 1_000).expect("proposal is durable at its admission height");
        assert!(
            validate(proposal, 1_300)
                .expect_err("due proposal must already be promoted")
                .contains("remains proposed")
        );

        let mut active = proposal;
        active.lifecycle = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1_000,
            activated_at_height: 1_300,
            state_since_height: 1_300,
        });
        assert!(
            validate(active, 1_299)
                .expect_err("future active interval must reject")
                .contains("activation height")
        );
        validate(active, 1_300).expect("active E is durable in committed E");

        let mut suspended = active;
        suspended.lifecycle = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
            proposed_at_height: 1_000,
            activated_at_height: 1_300,
            state_since_height: 1_400,
        });
        assert!(
            validate(suspended, 1_399)
                .expect_err("future suspension must reject")
                .contains("lifecycle state height")
        );
        validate(suspended, 1_400).expect("suspension is durable at its exact transition height");

        let mut retired = proposal;
        retired.lifecycle = PrivacyProtocolLifecycleV1::Retired(PrivacyRetiredLifecycleV1 {
            proposed_at_height: 1_000,
            activated_at_height: None,
            state_since_height: 1_200,
        });
        assert!(
            validate(retired, 1_199)
                .expect_err("future retirement must reject")
                .contains("lifecycle state height")
        );
        validate(retired, 1_200).expect("retirement is durable at its exact transition height");

        let mut uncompiled = active;
        uncompiled.verifier_digest = PrivacyVerifierDigestV1::new(nonzero(0xD7));
        assert!(
            validate(uncompiled, 1_300)
                .expect_err("uncompiled restored activation must reject")
                .contains("not compiled")
        );
    }

    #[test]
    fn promotion_rejects_structurally_valid_but_uncompiled_binding() {
        let mut proposal = activation_proposal();
        proposal.verifier_digest = PrivacyVerifierDigestV1::new(nonzero(0xD7));
        proposal
            .validate()
            .expect("non-zero alternate digest remains structurally valid");
        let key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mut activations = Storage::new();
        activations.insert(key, proposal);

        assert!(matches!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_300),
            Err(PrivacyActivationPromotionErrorV1::CompiledProfile(error))
                if error.protocol_id == PrivacyProtocolIdV1::VeRangeTransparentRangeV1
                && matches!(
                    &error.source,
                    crate::privacy_profiles::CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch
                )
        ));
        assert_eq!(
            activations
                .view()
                .get(&key)
                .expect("record remains")
                .lifecycle,
            proposal.lifecycle,
            "failed compiled-profile validation cannot partially promote"
        );
    }

    #[test]
    fn pgc_account_root_commits_namespace_epoch_order_and_ciphertexts() {
        let namespace = pgc_namespace(20);
        let accounts = pgc_accounts(16);
        let root = compute_privacy_pgc_account_state_root_v1(namespace, 7, 160, &accounts)
            .expect("canonical PGC account table");
        assert_eq!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, 160, &accounts)
                .expect("same input"),
            root,
            "root derivation must be deterministic"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(pgc_namespace(21), 7, 160, &accounts)
                .expect("different pool"),
            root,
            "pool namespace must be domain bound"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 8, 160, &accounts)
                .expect("different epoch"),
            root,
            "epoch must be committed"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, 161, &accounts)
                .expect("different supply"),
            root,
            "aggregate supply must be committed"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, u32::MAX, &accounts)
                .expect("inclusive u32 boundary"),
            root,
            "maximum aggregate supply must be hashed without wrapping"
        );

        let mut changed = accounts.clone();
        changed[5].encrypted_balance.left = pgc_accounts(16)[7].encrypted_balance.right;
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, 160, &changed)
                .expect("changed ciphertext"),
            root,
            "every encrypted-balance component must be committed"
        );
    }

    #[test]
    fn pgc_account_root_rejects_noncanonical_tables() {
        let namespace = pgc_namespace(20);
        let accounts = pgc_accounts(16);
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 0, 160, &accounts),
            Err(PrivacyPgcAccountStateRootErrorV1::ZeroEpoch),
            "epoch zero must fail at the typed public boundary"
        );
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 0, &accounts),
            Err(PrivacyPgcAccountStateRootErrorV1::ZeroTotalSupply),
            "zero supply must fail at the typed public boundary"
        );
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 160, &accounts[..15]),
            Err(PrivacyPgcAccountStateRootErrorV1::InvalidAccountCount),
            "cardinality outside the closed 16/32/64 set must fail explicitly"
        );

        let mut unordered = accounts.clone();
        unordered.swap(4, 5);
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 160, &unordered),
            Err(PrivacyPgcAccountStateRootErrorV1::KeysNotStrictlyIncreasing),
            "account keys must have one canonical strict order"
        );

        let mut duplicate = accounts.clone();
        duplicate[5].public_key = duplicate[4].public_key;
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 160, &duplicate),
            Err(PrivacyPgcAccountStateRootErrorV1::KeysNotStrictlyIncreasing),
            "duplicate accounts must fail at the ordering gate"
        );

        let mut zero_component = accounts;
        zero_component[3].encrypted_balance.right = PrivacyP256PointV1::new([0; 33]);
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 160, &zero_component),
            Err(PrivacyPgcAccountStateRootErrorV1::ZeroPoint),
            "zero encoded points must fail before hashing"
        );

        let mut off_curve = pgc_accounts(16);
        let mut invalid = [u8::MAX; 33];
        invalid[0] = 2;
        off_curve[3].encrypted_balance.right = PrivacyP256PointV1::new(invalid);
        assert_eq!(
            derive_privacy_pgc_account_state_root_v1(namespace, 1, 160, &off_curve),
            Err(PrivacyPgcAccountStateRootErrorV1::InvalidPoint),
            "non-zero off-curve encodings must not enter durable account state"
        );
    }

    #[test]
    fn persisted_pgc_state_survives_exact_json_restart_validation() {
        let mut fixture = pgc_persisted_fixture();
        fixture.validate().expect("coherent bootstrap state");
        let runtime_snapshot = load_privacy_pgc_pool_snapshot_v1(
            fixture.namespace,
            PrivacyConsensusLimitsV1::taira_default().retained_root_count,
            &fixture.pgc_accounts.view(),
            &fixture.pgc_pool_invariants.view(),
            &fixture.roots.view(),
            &fixture.root_heads.view(),
        )
        .expect("bounded runtime snapshot");
        assert_eq!(runtime_snapshot.namespace(), fixture.namespace);
        assert_eq!(runtime_snapshot.accounts().len(), 16);
        assert_eq!(
            runtime_snapshot.current_epoch(),
            PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
        );
        assert_eq!(runtime_snapshot.current_root(), fixture.root);
        assert_eq!(
            runtime_snapshot.retained_current_root(),
            Some((PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1, fixture.root))
        );

        let activations =
            norito::json::to_json(&fixture.activations).expect("encode activation storage");
        let accounts =
            norito::json::to_json(&fixture.pgc_accounts).expect("encode account storage");
        let invariants =
            norito::json::to_json(&fixture.pgc_pool_invariants).expect("encode invariant storage");
        let nullifiers =
            norito::json::to_json(&fixture.nullifiers).expect("encode nullifier storage");
        let commitments =
            norito::json::to_json(&fixture.commitments).expect("encode commitment storage");
        let roots = norito::json::to_json(&fixture.roots).expect("encode root storage");
        let root_heads =
            norito::json::to_json(&fixture.root_heads).expect("encode root-head storage");

        fixture.activations =
            norito::json::from_json(&activations).expect("restore activation storage");
        fixture.pgc_accounts = norito::json::from_json(&accounts).expect("restore account storage");
        fixture.pgc_pool_invariants =
            norito::json::from_json(&invariants).expect("restore invariant storage");
        fixture.nullifiers =
            norito::json::from_json(&nullifiers).expect("restore nullifier storage");
        fixture.commitments =
            norito::json::from_json(&commitments).expect("restore commitment storage");
        fixture.roots = norito::json::from_json(&roots).expect("restore root storage");
        fixture.root_heads =
            norito::json::from_json(&root_heads).expect("restore root-head storage");

        fixture
            .validate()
            .expect("restored state must preserve every cross-map invariant");
    }

    #[test]
    fn persisted_pgc_state_rejects_every_orphan_class() {
        expect_pgc_persisted_error(
            |fixture| fixture.activations = Storage::new(),
            "unregistered protocol",
        );
        expect_pgc_persisted_error(
            |fixture| fixture.pgc_pool_invariants = Storage::new(),
            "no pool invariant",
        );
        expect_pgc_persisted_error(
            |fixture| fixture.pgc_accounts = Storage::new(),
            "has no encrypted account table",
        );
        expect_pgc_persisted_error(
            |fixture| fixture.roots = Storage::new(),
            "has no retained history",
        );
        expect_pgc_persisted_error(
            |fixture| fixture.root_heads = Storage::new(),
            "has no current head",
        );
    }

    #[test]
    fn persisted_pgc_state_rejects_supply_digest_root_epoch_and_provenance_corruption() {
        expect_pgc_persisted_error(
            |fixture| {
                fixture.pgc_pool_invariants.insert(
                    fixture.invariant_key,
                    PrivacyPgcPoolInvariantV1::new(
                        161,
                        fixture.root,
                        fixture.bootstrap_digest,
                        fixture.bootstrap_proof_digest,
                    )
                    .expect("altered supply remains locally valid"),
                );
            },
            "does not match its root head",
        );
        expect_pgc_persisted_error(
            |fixture| {
                fixture.pgc_pool_invariants.insert(
                    fixture.invariant_key,
                    PrivacyPgcPoolInvariantV1::new(
                        160,
                        PrivacyRootV1::new(nonzero(0xC0)),
                        fixture.bootstrap_digest,
                        fixture.bootstrap_proof_digest,
                    )
                    .expect("altered bootstrap root remains locally valid"),
                );
            },
            "bootstrap root provenance differs from its immutable invariant",
        );
        expect_pgc_persisted_error(
            |fixture| {
                fixture.pgc_pool_invariants.insert(
                    fixture.invariant_key,
                    PrivacyPgcPoolInvariantV1::new(
                        160,
                        fixture.root,
                        PrivacyPgcAccountBootstrapDigestV1::new(nonzero(0xC1)),
                        fixture.bootstrap_proof_digest,
                    )
                    .expect("altered public digest remains locally valid"),
                );
            },
            "bootstrap root provenance differs from its immutable invariant",
        );
        expect_pgc_persisted_error(
            |fixture| {
                fixture.pgc_pool_invariants.insert(
                    fixture.invariant_key,
                    PrivacyPgcPoolInvariantV1::new(
                        160,
                        fixture.root,
                        fixture.bootstrap_digest,
                        PrivacyPgcBootstrapProofDigestV1::new(nonzero(0xC2)),
                    )
                    .expect("altered proof digest remains locally valid"),
                );
            },
            "bootstrap root provenance differs from its immutable invariant",
        );
        expect_pgc_persisted_error(
            |fixture| {
                let state = *fixture
                    .pgc_accounts
                    .view()
                    .get(&fixture.account_keys[0])
                    .expect("first account");
                fixture.pgc_accounts.insert(
                    fixture.account_keys[0],
                    PrivacyPgcAccountStateV1::new(
                        state.encrypted_balance,
                        state.epoch + 1,
                        state.provenance,
                    )
                    .expect("altered epoch remains locally valid"),
                );
            },
            "contains mixed epochs",
        );
        expect_pgc_persisted_error(
            |fixture| {
                let state = *fixture
                    .pgc_accounts
                    .view()
                    .get(&fixture.account_keys[0])
                    .expect("first account");
                fixture.pgc_accounts.insert(
                    fixture.account_keys[0],
                    PrivacyPgcAccountStateV1::new(
                        state.encrypted_balance,
                        state.epoch,
                        PrivacyPgcAccountProvenanceV1::verified_proof(
                            PrivacyStatementDigestV1::new(nonzero(0xC3)),
                            9,
                            0,
                        )
                        .expect("alternate provenance"),
                    )
                    .expect("altered provenance remains locally valid"),
                );
            },
            "contains mixed provenance",
        );
        expect_pgc_persisted_error(
            |fixture| {
                fixture.replace_root_and_head(
                    PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
                    PrivacyRootV1::new(nonzero(0xC4)),
                    fixture.provenance,
                );
            },
            "bootstrap root provenance differs from its immutable invariant",
        );
        expect_pgc_persisted_error(
            |fixture| {
                let wrong = PrivacyRootProvenanceV1::verified_bootstrap(
                    fixture.bootstrap_digest,
                    fixture.bootstrap_proof_digest,
                    10,
                )
                .expect("different admitted height");
                fixture.root_heads.insert(
                    fixture.head_key,
                    PrivacyRootHeadRecordV1::new(
                        PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
                        fixture.root,
                        wrong,
                        None,
                    )
                    .expect("locally valid mismatched head"),
                );
            },
            "does not equal latest history entry",
        );
        expect_pgc_persisted_error(
            |fixture| {
                let governance = PrivacyRootProvenanceV1::governance(
                    PrivacyRootPublicationDigestV1::new(nonzero(0xC5)),
                    9,
                )
                .expect("governance provenance");
                fixture.replace_root_and_head(
                    PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
                    fixture.root,
                    governance,
                );
            },
            "begins with invalid provenance",
        );
    }

    #[test]
    fn persisted_pgc_state_allows_only_proof_successors_after_verified_bootstrap() {
        let mut fixture = pgc_persisted_fixture();
        let statement_digest = PrivacyStatementDigestV1::new(nonzero(0xD1));
        let account_provenance =
            PrivacyPgcAccountProvenanceV1::verified_proof(statement_digest, 10, 0)
                .expect("proof account provenance");
        let invariant = *fixture
            .pgc_pool_invariants
            .view()
            .get(&fixture.invariant_key)
            .expect("pool invariant");
        let root_provenance = PrivacyRootProvenanceV1::verified_pgc_successor(
            statement_digest,
            10,
            0,
            PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            fixture.root,
            invariant
                .digest(fixture.namespace)
                .expect("pool invariant digest"),
        )
        .expect("proof root provenance");
        let account_table = fixture
            .pgc_accounts
            .view()
            .iter()
            .map(|(key, state)| PrivacyPgcAccountV1 {
                public_key: key.public_key(),
                encrypted_balance: state.encrypted_balance(),
            })
            .collect::<Vec<_>>();
        for key in &fixture.account_keys {
            let encrypted_balance = fixture
                .pgc_accounts
                .view()
                .get(key)
                .expect("account")
                .encrypted_balance();
            fixture.pgc_accounts.insert(
                *key,
                PrivacyPgcAccountStateV1::new(
                    encrypted_balance,
                    PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 + 1,
                    account_provenance,
                )
                .expect("proof-updated account"),
            );
        }
        let successor_epoch = PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 + 1;
        let successor_root = compute_privacy_pgc_account_state_root_v1(
            fixture.namespace,
            successor_epoch,
            160,
            &account_table,
        )
        .expect("successor root");
        let successor_key = PrivacyRootKeyV1::new(
            fixture.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            successor_epoch,
            successor_root,
        )
        .expect("successor root key");
        fixture.roots.insert(successor_key, root_provenance);
        fixture.root_heads.insert(
            fixture.head_key,
            PrivacyRootHeadRecordV1::new(successor_epoch, successor_root, root_provenance, None)
                .expect("successor head"),
        );
        fixture
            .validate()
            .expect("one verified-proof successor is coherent");

        expect_pgc_persisted_error(
            |fixture| {
                let second_bootstrap = PrivacyRootProvenanceV1::verified_bootstrap(
                    fixture.bootstrap_digest,
                    fixture.bootstrap_proof_digest,
                    10,
                )
                .expect("second bootstrap provenance");
                let second_root = PrivacyRootV1::new(nonzero(0xD2));
                fixture.roots.insert(
                    PrivacyRootKeyV1::new(
                        fixture.namespace,
                        PrivacyRootRoleV1::PgcAccountState,
                        successor_epoch,
                        second_root,
                    )
                    .expect("second root"),
                    second_bootstrap,
                );
                fixture.root_heads.insert(
                    fixture.head_key,
                    PrivacyRootHeadRecordV1::new(
                        successor_epoch,
                        second_root,
                        second_bootstrap,
                        None,
                    )
                    .expect("second head"),
                );
            },
            "contains a non-PGC successor",
        );
    }

    #[test]
    fn pgc_retention_rollover_keeps_exact_parent_linked_windows() {
        for retained in [1_u32, 2, 3] {
            let mut fixture = pgc_persisted_fixture();
            for _ in 0..retained + 2 {
                fixture.advance_with_retention(retained);
            }
            let snapshot = fixture
                .load_with_retention(retained)
                .expect("small retained window");
            assert_eq!(
                fixture.roots.view().iter().count(),
                usize::try_from(retained).expect("small retention")
            );
            assert_eq!(snapshot.current_epoch(), u64::from(retained) + 3);
            assert!(snapshot.retention_anchor().is_some());
            assert_eq!(
                snapshot.retained_current_root(),
                Some((snapshot.current_epoch(), snapshot.current_root()))
            );
        }

        let retained = PrivacyConsensusLimitsV1::taira_default().retained_root_count;
        let mut fixture = pgc_persisted_fixture();
        for _ in 1..retained {
            fixture.advance_with_retention(retained);
        }
        assert_eq!(
            fixture.roots.view().iter().count(),
            usize::try_from(retained).expect("Taira retention")
        );
        assert!(
            fixture
                .root_heads
                .view()
                .get(&fixture.head_key)
                .expect("pre-rollover head")
                .retention_anchor()
                .is_none(),
            "epoch 2048 still retains the canonical bootstrap"
        );

        fixture.advance_with_retention(retained);
        let snapshot = fixture
            .load_with_retention(retained)
            .expect("2048-to-2049 rollover");
        assert_eq!(snapshot.current_epoch(), u64::from(retained) + 1);
        let anchor = snapshot
            .retention_anchor()
            .expect("rollover anchor replaces pruned bootstrap");
        assert_eq!(anchor.epoch(), PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1);
        assert_eq!(anchor.root(), fixture.invariant().bootstrap_root());
        assert_eq!(
            fixture.roots.view().iter().count(),
            usize::try_from(retained).expect("Taira retention")
        );
        fixture
            .validate()
            .expect("default-policy persisted state survives exact rollover");
    }

    #[test]
    fn pgc_retention_lowering_is_immediate_atomic_and_restart_safe() {
        let mut fixture = pgc_persisted_fixture();
        for _ in 0..5 {
            fixture.advance_with_retention(6);
        }
        assert_eq!(fixture.roots.view().iter().count(), 6);

        let roots_before = fixture
            .roots
            .view()
            .iter()
            .map(|(key, provenance)| (*key, *provenance))
            .collect::<Vec<_>>();
        assert!(matches!(
            plan_privacy_root_retention_reduction_v1(
                &fixture.roots.view(),
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                0,
            ),
            Err(PrivacyRootHistoryErrorV1::ZeroRetention)
        ));
        assert_eq!(
            fixture
                .roots
                .view()
                .iter()
                .map(|(key, provenance)| (*key, *provenance))
                .collect::<Vec<_>>(),
            roots_before,
            "invalid lowering plan must be read-only"
        );

        fixture.tighten_retention(3);
        let snapshot = fixture
            .load_with_retention(3)
            .expect("lowered retained window");
        assert_eq!(fixture.roots.view().iter().count(), 3);
        assert_eq!(snapshot.current_epoch(), 6);
        assert_eq!(
            snapshot
                .retention_anchor()
                .expect("lowering anchor")
                .epoch(),
            3
        );

        let accounts = norito::json::to_json(&fixture.pgc_accounts).expect("encode accounts");
        let invariants =
            norito::json::to_json(&fixture.pgc_pool_invariants).expect("encode invariants");
        let roots = norito::json::to_json(&fixture.roots).expect("encode roots");
        let heads = norito::json::to_json(&fixture.root_heads).expect("encode heads");
        fixture.pgc_accounts = norito::json::from_json(&accounts).expect("restore accounts");
        fixture.pgc_pool_invariants =
            norito::json::from_json(&invariants).expect("restore invariants");
        fixture.roots = norito::json::from_json(&roots).expect("restore roots");
        fixture.root_heads = norito::json::from_json(&heads).expect("restore heads");
        fixture
            .load_with_retention(3)
            .expect("restart preserves lowered parent-linked window");
    }

    #[test]
    fn pgc_retained_window_rejects_orphans_gaps_duplicates_reordering_and_forgery() {
        let rolled = || {
            let mut fixture = pgc_persisted_fixture();
            for _ in 0..4 {
                fixture.advance_with_retention(3);
            }
            fixture
                .load_with_retention(3)
                .expect("valid adversarial fixture");
            fixture
        };

        let mut fixture = rolled();
        let keys = fixture
            .roots
            .view()
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        fixture.roots = fixture
            .roots
            .view()
            .iter()
            .filter(|(key, _)| **key != keys[1])
            .map(|(key, provenance)| (*key, *provenance))
            .collect();
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("orphan in retained middle")
                .contains("gap or forged parent")
        );

        let mut fixture = rolled();
        let (first_key, first_provenance) = fixture
            .roots
            .view()
            .iter()
            .next()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("first retained root");
        let duplicate_key = PrivacyRootKeyV1::new(
            fixture.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            first_key.epoch(),
            PrivacyRootV1::new(nonzero(0xE1)),
        )
        .expect("same-epoch alternate root");
        fixture.roots.insert(duplicate_key, first_provenance);
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("duplicate retained epoch")
                .contains("duplicate epoch")
        );

        let mut fixture = rolled();
        let retained = fixture
            .roots
            .view()
            .iter()
            .map(|(key, provenance)| (*key, *provenance))
            .collect::<Vec<_>>();
        fixture.roots.insert(retained[0].0, retained[1].1);
        fixture.roots.insert(retained[1].0, retained[0].1);
        assert!(
            fixture.load_with_retention(3).is_err(),
            "reordered provenance"
        );

        let mut fixture = rolled();
        let (first_key, first_provenance) = fixture
            .roots
            .view()
            .iter()
            .next()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("first retained root");
        let PrivacyRootProvenanceV1::VerifiedPgcSuccessor {
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            pool_invariant_digest,
            ..
        } = first_provenance
        else {
            panic!("rolled prefix starts with a PGC successor");
        };
        let forged_parent = PrivacyRootProvenanceV1::verified_pgc_successor(
            statement_digest,
            admitted_at_height,
            action_index,
            parent_epoch,
            PrivacyRootV1::new(nonzero(0xE2)),
            pool_invariant_digest,
        )
        .expect("locally valid forged parent");
        fixture.roots.insert(first_key, forged_parent);
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("forged pruned-prefix parent")
                .contains("exact pruned-prefix anchor")
        );

        for (anchor_epoch, anchor_root) in [
            (1, PrivacyRootV1::new(nonzero(0xE3))),
            (3, PrivacyRootV1::new(nonzero(0xE4))),
            (2, PrivacyRootV1::new(nonzero(0xE5))),
        ] {
            let mut fixture = rolled();
            let head = *fixture
                .root_heads
                .view()
                .get(&fixture.head_key)
                .expect("rolled head");
            fixture.root_heads.insert(
                fixture.head_key,
                PrivacyRootHeadRecordV1::new(
                    head.epoch(),
                    head.root(),
                    head.provenance(),
                    Some(
                        PrivacyRootRetentionAnchorV1::new(anchor_epoch, anchor_root)
                            .expect("locally valid forged anchor"),
                    ),
                )
                .expect("locally valid forged head"),
            );
            assert!(
                fixture.load_with_retention(3).is_err(),
                "stale, advanced, and wrong-root anchors must reject"
            );
        }

        let mut fixture = pgc_persisted_fixture();
        let head = *fixture
            .root_heads
            .view()
            .get(&fixture.head_key)
            .expect("bootstrap head");
        fixture.root_heads.insert(
            fixture.head_key,
            PrivacyRootHeadRecordV1 {
                epoch: head.epoch(),
                root: head.root(),
                provenance: head.provenance(),
                retention_anchor: Some(
                    PrivacyRootRetentionAnchorV1::new(1, fixture.root)
                        .expect("nonzero corrupt anchor"),
                ),
            },
        );
        assert!(
            fixture.load_with_retention(3).is_err(),
            "anchor with bootstrap"
        );

        let mut fixture = rolled();
        let altered = PrivacyPgcPoolInvariantV1::new(
            fixture.invariant().total_supply(),
            PrivacyRootV1::new(nonzero(0xE6)),
            fixture.bootstrap_digest,
            fixture.bootstrap_proof_digest,
        )
        .expect("altered immutable bootstrap root");
        fixture
            .pgc_pool_invariants
            .insert(fixture.invariant_key, altered);
        assert!(
            fixture
                .load_with_retention(3)
                .expect_err("mutated immutable metadata after rollover")
                .contains("different immutable pool invariant")
        );
    }

    #[test]
    fn future_unanchored_retention_is_prevalidated_while_typed_histories_are_prunable() {
        let unanchored_namespace = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            PrivacyNamespaceScopeV1::IssuerPolicy(
                iroha_data_model::privacy::PrivacyIssuerPolicyNamespaceV1 {
                    issuer_id: PrivacyIssuerIdV1::new(nonzero(0xA6)),
                    policy_id: PrivacyPolicyIdV1::new(nonzero(0xA7)),
                },
            ),
        );
        let provenance = PrivacyRootProvenanceV1::governance(
            PrivacyRootPublicationDigestV1::new(nonzero(0xA8)),
            1,
        )
        .expect("governance provenance");
        let mut non_pgc_roots = Storage::new();
        for epoch in 1..=2 {
            non_pgc_roots.insert(
                PrivacyRootKeyV1::new(
                    unanchored_namespace,
                    PrivacyRootRoleV1::Revocation,
                    epoch,
                    PrivacyRootV1::new([u8::try_from(epoch).expect("small epoch"); 32]),
                )
                .expect("FCMP++ root key"),
                provenance,
            );
        }
        validate_unanchored_privacy_root_retention_v1(&non_pgc_roots.view(), 2)
            .expect("inclusive future cap");
        assert!(
            validate_unanchored_privacy_root_retention_v1(&non_pgc_roots.view(), 1)
                .expect_err("unanchored histories cannot be implicitly pruned")
                .contains("exceeding scheduled retention 1")
        );
        assert!(validate_unanchored_privacy_root_retention_v1(&non_pgc_roots.view(), 0).is_err());

        for (namespace, role, label) in [
            (
                pgc_namespace(0xB7),
                PrivacyRootRoleV1::PgcAccountState,
                "PGC",
            ),
            (
                zk_ams_namespace(0xB8),
                PrivacyRootRoleV1::AccountRegistry,
                "ZK-AMS",
            ),
            (
                orchard_namespace(0xB9),
                PrivacyRootRoleV1::NoteCommitmentAnchor,
                "Orchard",
            ),
            (
                x509_ca_namespace(),
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                "X.509 CA",
            ),
            (
                PrivacyNamespaceV1::new(
                    PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                    PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                        pool_id: PrivacyPoolIdV1::new(nonzero(0xBA)),
                    }),
                ),
                PrivacyRootRoleV1::OutputSet,
                "FCMP++",
            ),
            (
                PrivacyNamespaceV1::new(
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                    PrivacyNamespaceScopeV1::PoolProgram(
                        iroha_data_model::privacy::PrivacyPoolProgramNamespaceV1 {
                            pool_id: PrivacyPoolIdV1::new(nonzero(0xBB)),
                            program_id: iroha_data_model::privacy::PrivacyProgramIdV1::new(
                                nonzero(0xBC),
                            ),
                        },
                    ),
                ),
                PrivacyRootRoleV1::ProgramState,
                "private-IVM",
            ),
            (
                PrivacyNamespaceV1::new(
                    PrivacyProtocolIdV1::PqMaspStarkV0,
                    PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                        pool_id: PrivacyPoolIdV1::new(nonzero(0xBD)),
                    }),
                ),
                PrivacyRootRoleV1::NoteCommitmentAnchor,
                "PQ-MASP",
            ),
        ] {
            let mut roots = Storage::new();
            for epoch in 1..=3 {
                roots.insert(
                    PrivacyRootKeyV1::new(
                        namespace,
                        role,
                        epoch,
                        PrivacyRootV1::new([u8::try_from(epoch).expect("small epoch"); 32]),
                    )
                    .expect("typed root key"),
                    provenance,
                );
            }
            validate_unanchored_privacy_root_retention_v1(&roots.view(), 1).unwrap_or_else(
                |error| panic!("{label} must use its typed pruning planner: {error}"),
            );
        }
    }

    #[test]
    fn persisted_state_rejects_orphan_privacy_items() {
        let activations =
            Storage::<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>::new();
        let pgc_accounts = Storage::<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>::new();
        let pgc_pool_invariants =
            Storage::<PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1>::new();
        let mut nullifiers = Storage::<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>::new();
        let commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        let roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
        let root_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
        nullifiers.insert(
            PrivacyNullifierKeyV1::zk_ams_key_image(
                zk_ams_namespace(20),
                PrivacyZkAmsKeyImageV1::new(nonzero(22)),
            )
            .expect("valid scoped key"),
            PrivacyStateItemRecordV1::zk_ams_verified_proof(
                PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(24)),
                PrivacyStatementDigestV1::new(nonzero(23)),
                1,
                0,
            )
            .expect("valid provenance"),
        );

        let error = validate_privacy_persisted_state_v1(
            &PrivacyConsensusPolicyV1::taira_default(),
            &activations.view(),
            &pgc_accounts.view(),
            &pgc_pool_invariants.view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect_err("an unregistered protocol cannot own persisted state");
        assert!(error.contains("unregistered protocol"), "{error}");
    }

    #[test]
    fn bootle_lantern_policy_key_is_exact_role_separated_and_canonical() {
        let issuer_id = PrivacyIssuerIdV1::new(nonzero(0xB1));
        let policy_id = PrivacyPolicyIdV1::new(nonzero(0xB2));
        let key = PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(issuer_id, policy_id)
            .expect("nonzero Bootle/Lantern key");

        assert_eq!(
            key.protocol_id(),
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
        );
        assert_eq!(
            key.bootle_lantern_issuer_policy_identity(),
            Some((issuer_id, policy_id))
        );
        assert_eq!(key.zk_ams_namespace(), None);
        assert!(
            PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(
                PrivacyIssuerIdV1::new([0; 32]),
                policy_id,
            )
            .is_err()
        );
        assert!(
            PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(
                issuer_id,
                PrivacyPolicyIdV1::new([0; 32]),
            )
            .is_err()
        );

        let mut encoded_json = String::new();
        key.encode_json_key(&mut encoded_json);
        let encoded =
            norito::json::from_json::<String>(&encoded_json).expect("canonical key string");
        assert_eq!(
            PrivacyCommitmentKeyV1::decode_json_key(&encoded).expect("canonical key roundtrip"),
            key
        );
        assert!(
            PrivacyCommitmentKeyV1::decode_json_key(&encoded.to_ascii_lowercase()).is_err(),
            "lowercase storage-key hex must reject"
        );
        assert!(PrivacyCommitmentKeyV1::decode_json_key(&format!(" {encoded}")).is_err());
        assert!(PrivacyCommitmentKeyV1::decode_json_key(&format!("{encoded} ")).is_err());
        assert!(PrivacyCommitmentKeyV1::decode_json_key(&encoded[..encoded.len() - 1]).is_err());
        let mut trailing = hex::decode(&encoded).expect("canonical key hex");
        trailing.push(0);
        assert!(
            PrivacyCommitmentKeyV1::decode_json_key(&hex::encode_upper(trailing)).is_err(),
            "trailing Norito bytes must reject"
        );
    }

    #[test]
    fn bootle_lantern_policy_loader_and_restore_reject_cross_role_and_corruption() {
        let policy = bootle_lantern_issuer_policy(
            0xB1,
            0xB2,
            1,
            BootleLanternIssuerPolicyLifecycleV1::Active,
        );
        let issuer_id = policy.issuer_id;
        let policy_id = policy.policy_id;
        let key = PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(issuer_id, policy_id)
            .expect("Bootle/Lantern key");
        let record =
            PrivacyStateItemRecordV1::bootle_lantern_issuer_policy_governance(policy.clone(), 7)
                .expect("Bootle/Lantern governance record");

        let mut commitments = Storage::new();
        assert!(
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &commitments.view())
                .expect_err("missing policy must reject")
                .contains("not registered")
        );
        commitments.insert(key, record.clone());
        assert_eq!(
            privacy_bootle_lantern_issuer_policy_count_v1(&commitments.view())
                .expect("bounded policy count"),
            1
        );
        assert_eq!(
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &commitments.view())
                .expect("load canonical policy"),
            policy
        );
        assert_eq!(record.bootle_lantern_issuer_policy(), Some(&policy));
        assert!(
            validate_persisted_commitments(&commitments)
                .expect_err("Bootle/Lantern state requires executable protocol activation")
                .contains("unregistered protocol")
        );

        let mut wrong_role = Storage::new();
        wrong_role.insert(
            key,
            PrivacyStateItemRecordV1::zk_ace_policy_governance(
                zk_ace_policy_record(zk_ace_policy_id(1)),
                7,
            )
            .expect("valid cross-role record"),
        );
        assert!(
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &wrong_role.view())
                .expect_err("cross-role record must reject")
                .contains("wrong-role")
        );
        let mismatched_policy = bootle_lantern_issuer_policy(
            0xB4,
            0xB2,
            1,
            BootleLanternIssuerPolicyLifecycleV1::Active,
        );
        let mut mismatched = Storage::new();
        mismatched.insert(
            key,
            PrivacyStateItemRecordV1::bootle_lantern_issuer_policy_governance(mismatched_policy, 7)
                .expect("intrinsically valid mismatched policy"),
        );
        assert!(
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &mismatched.view())
                .expect_err("key/record identity mismatch must reject")
                .contains("does not match")
        );

        let mut corrupted_policy = policy.clone();
        corrupted_policy.record_digest =
            PrivacyBootleLanternIssuerPolicyDigestV1::new(nonzero(0xB5));
        let mut corrupted = Storage::new();
        corrupted.insert(
            key,
            PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance {
                policy: corrupted_policy,
                admitted_at_height: 7,
            },
        );
        let error =
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &corrupted.view())
                .expect_err("record digest corruption must reject");
        assert!(error.contains("is invalid"), "unexpected error: {error}");

        let mut wrong_parameter_digest = policy.clone();
        wrong_parameter_digest.issuer_parameter_digest =
            PrivacyParameterDigestV1::new(nonzero(0xB6));
        wrong_parameter_digest.record_digest =
            PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        wrong_parameter_digest.record_digest = wrong_parameter_digest
            .computed_record_digest()
            .expect("recompute outer record digest");
        let mut corrupted_parameter = Storage::new();
        corrupted_parameter.insert(
            key,
            PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance {
                policy: wrong_parameter_digest,
                admitted_at_height: 7,
            },
        );
        let error = load_privacy_bootle_lantern_issuer_policy_v1(
            issuer_id,
            policy_id,
            &corrupted_parameter.view(),
        )
        .expect_err("issuer-parameter digest substitution must reject");
        assert!(error.contains("is invalid"), "unexpected error: {error}");

        let mut zero_height = Storage::new();
        zero_height.insert(
            key,
            PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance {
                policy,
                admitted_at_height: 0,
            },
        );
        assert!(
            load_privacy_bootle_lantern_issuer_policy_v1(issuer_id, policy_id, &zero_height.view())
                .expect_err("zero admission height must reject")
                .contains("admission height must be non-zero")
        );
    }

    #[test]
    fn bootle_lantern_terminal_lifecycle_is_durable_but_unknown_json_state_rejects() {
        let revoked = bootle_lantern_issuer_policy(
            0xB1,
            0xB2,
            2,
            BootleLanternIssuerPolicyLifecycleV1::Revoked,
        );
        let record = PrivacyStateItemRecordV1::bootle_lantern_issuer_policy_governance(revoked, 8)
            .expect("terminal policy is valid durable state");
        record
            .validate()
            .expect("terminal policy record remains structurally valid");

        let encoded = norito::json::to_json(&record).expect("encode policy state record");
        let invalid_lifecycle = encoded.replacen("\"revoked\"", "\"reactivated\"", 1);
        assert_ne!(invalid_lifecycle, encoded, "fixture contains lifecycle tag");
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&invalid_lifecycle).is_err(),
            "unknown lifecycle encodings must reject without aliases"
        );

        let unknown_record_field = encoded.replacen(
            "\"admitted_at_height\":8",
            "\"admitted_at_height\":8,\"legacy\":true",
            1,
        );
        assert_ne!(
            unknown_record_field, encoded,
            "fixture contains the record content"
        );
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&unknown_record_field).is_err(),
            "unknown durable record fields must reject in the first release"
        );
    }

    #[test]
    fn bootle_lantern_policy_count_accepts_cap_and_rejects_cap_plus_one() {
        let issuer_id = PrivacyIssuerIdV1::new(nonzero(0xB1));
        let template = bootle_lantern_issuer_policy(
            0xB1,
            0xB2,
            1,
            BootleLanternIssuerPolicyLifecycleV1::Active,
        );
        let mut commitments = Storage::new();
        for index in 1..=BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1 {
            let index = u64::try_from(index).expect("policy cap fits u64");
            let policy_id = PrivacyPolicyIdV1::new(indexed_nonzero(0xB7, index));
            let mut policy = template.clone();
            policy.policy_id = policy_id;
            policy.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
            policy.record_digest = policy
                .computed_record_digest()
                .expect("canonical policy digest");
            commitments.insert(
                PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(issuer_id, policy_id)
                    .expect("bounded policy key"),
                PrivacyStateItemRecordV1::bootle_lantern_issuer_policy_governance(policy, 7)
                    .expect("bounded policy record"),
            );
        }
        assert_eq!(
            privacy_bootle_lantern_issuer_policy_count_v1(&commitments.view())
                .expect("exact global policy cap"),
            BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1
        );

        let over_index =
            u64::try_from(BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1).expect("policy cap fits u64") + 1;
        let over_policy_id = PrivacyPolicyIdV1::new(indexed_nonzero(0xB7, over_index));
        let mut over_policy = template;
        over_policy.policy_id = over_policy_id;
        over_policy.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        over_policy.record_digest = over_policy
            .computed_record_digest()
            .expect("canonical over-cap policy digest");
        commitments.insert(
            PrivacyCommitmentKeyV1::bootle_lantern_issuer_policy(issuer_id, over_policy_id)
                .expect("over-cap policy key"),
            PrivacyStateItemRecordV1::bootle_lantern_issuer_policy_governance(over_policy, 7)
                .expect("over-cap policy record"),
        );
        let error = privacy_bootle_lantern_issuer_policy_count_v1(&commitments.view())
            .expect_err("global policy cap plus one must reject");
        assert!(
            error.contains("issuer-policy count exceeds 4096"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn vega_issuer_loader_rejects_missing_cross_role_key_mismatch_and_corrupt_snapshot() {
        let issuer_id = PrivacyIssuerIdV1::new(nonzero(0xC1));
        let record = vega_issuer_record(
            issuer_id,
            1,
            1,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let key = PrivacyCommitmentKeyV1::vega_issuer_revision(issuer_id, 1)
            .expect("canonical Vega issuer key");
        assert_eq!(
            key.protocol_id(),
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0
        );
        let state_record = PrivacyStateItemRecordV1::vega_issuer_governance(record, 7)
            .expect("canonical Vega governance provenance");
        assert_eq!(state_record.vega_issuer(), Some(&record));

        let mut commitments = Storage::new();
        assert!(
            load_privacy_vega_issuer_v1(issuer_id, &commitments.view())
                .expect_err("missing issuer must reject")
                .contains("not registered")
        );
        commitments.insert(key, state_record);
        assert_eq!(
            privacy_vega_issuer_record_count_v1(&commitments.view())
                .expect("bounded Vega issuer count"),
            1
        );
        assert_eq!(
            load_privacy_vega_issuer_v1(issuer_id, &commitments.view())
                .expect("canonical Vega issuer"),
            record
        );
        let registry_facts =
            privacy_vega_issuer_registry_facts_v1(record.issuer_public_key, &commitments.view())
                .expect("canonical Vega issuer registry facts");
        assert_eq!(registry_facts.record_count(), 1);
        assert_eq!(registry_facts.key_owner(), Some(issuer_id));

        let alias_issuer_id = PrivacyIssuerIdV1::new(nonzero(0xCF));
        let alias = PrivacyVegaIssuerRecordV1::new(
            alias_issuer_id,
            1,
            record.issuer_public_key,
            record.document_type,
            record.namespace,
            record.digest_algorithm,
            record.issuer_authentication_algorithm,
            record.device_authentication_algorithm,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("self-consistent cross-lineage key alias");
        let alias_key = PrivacyCommitmentKeyV1::vega_issuer_revision(alias_issuer_id, 1)
            .expect("canonical alias revision key");
        let mut aliased = Storage::new();
        aliased.insert(
            key,
            PrivacyStateItemRecordV1::vega_issuer_governance(record, 7)
                .expect("canonical Vega governance provenance"),
        );
        aliased.insert(
            alias_key,
            PrivacyStateItemRecordV1::vega_issuer_governance(alias, 8)
                .expect("intrinsically valid alias provenance"),
        );
        let alias_error = privacy_vega_issuer_record_count_v1(&aliased.view())
            .expect_err("one P-256 key cannot own multiple issuer identities");
        assert!(
            alias_error.contains("assigned to multiple lineages"),
            "unexpected alias rejection: {alias_error}"
        );

        let rotated = vega_issuer_record(
            issuer_id,
            2,
            2,
            Some(record.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let reactivated = vega_issuer_record(
            issuer_id,
            3,
            1,
            Some(rotated.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let mut reused = Storage::new();
        for (revision, height) in [(record, 7), (rotated, 8), (reactivated, 9)] {
            reused.insert(
                PrivacyCommitmentKeyV1::vega_issuer_revision(
                    revision.issuer_id,
                    revision.record_epoch,
                )
                .expect("canonical reused-key revision key"),
                PrivacyStateItemRecordV1::vega_issuer_governance(revision, height)
                    .expect("intrinsically valid reused-key provenance"),
            );
        }
        let reuse_error = privacy_vega_issuer_record_count_v1(&reused.view())
            .expect_err("a Vega lineage cannot reactivate a retired issuer key");
        assert!(
            reuse_error.contains("reactivates a retired P-256 key"),
            "unexpected key-reactivation rejection: {reuse_error}"
        );

        let mut wrong_role = Storage::new();
        wrong_role.insert(
            key,
            PrivacyStateItemRecordV1::zk_ace_policy_governance(
                zk_ace_policy_record(zk_ace_policy_id(1)),
                7,
            )
            .expect("valid wrong-role provenance"),
        );
        assert!(
            load_privacy_vega_issuer_v1(issuer_id, &wrong_role.view())
                .expect_err("cross-role record must reject")
                .contains("wrong-role")
        );

        let mismatched = vega_issuer_record(
            PrivacyIssuerIdV1::new(nonzero(0xC2)),
            1,
            1,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let mut mismatched_state = Storage::new();
        mismatched_state.insert(
            key,
            PrivacyStateItemRecordV1::vega_issuer_governance(mismatched, 7)
                .expect("intrinsically valid mismatched issuer"),
        );
        assert!(
            load_privacy_vega_issuer_v1(issuer_id, &mismatched_state.view())
                .expect_err("key/record mismatch must reject")
                .contains("differs from its record")
        );

        let mut corrupt_digest = record;
        corrupt_digest.record_digest.0[0] ^= 1;
        let mut corrupted = Storage::new();
        corrupted.insert(
            key,
            PrivacyStateItemRecordV1::VegaIssuerGovernance {
                record: corrupt_digest,
                admitted_at_height: 7,
            },
        );
        assert!(
            load_privacy_vega_issuer_v1(issuer_id, &corrupted.view())
                .expect_err("self-digest corruption must reject")
                .contains("is invalid")
        );

        let mut invalid_key_bytes = [u8::MAX; 33];
        invalid_key_bytes[0] = 0x02;
        let off_curve = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            1,
            PrivacyP256PointV1::new(invalid_key_bytes),
            record.document_type,
            record.namespace,
            record.digest_algorithm,
            record.issuer_authentication_algorithm,
            record.device_authentication_algorithm,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("wire-level compressed shape is valid");
        let mut corrupted_key = Storage::new();
        corrupted_key.insert(
            key,
            PrivacyStateItemRecordV1::VegaIssuerGovernance {
                record: off_curve,
                admitted_at_height: 7,
            },
        );
        assert!(
            load_privacy_vega_issuer_v1(issuer_id, &corrupted_key.view())
                .expect_err("off-curve snapshot key must reject")
                .contains("invalid P-256 key")
        );
    }

    #[test]
    fn vega_issuer_lineage_rejects_gaps_terminal_advancement_and_wrong_predecessor() {
        let issuer_id = PrivacyIssuerIdV1::new(nonzero(0xC3));
        let origin = vega_issuer_record(
            issuer_id,
            1,
            1,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let insert = |storage: &mut Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
                      record: PrivacyVegaIssuerRecordV1| {
            storage.insert(
                PrivacyCommitmentKeyV1::vega_issuer_revision(record.issuer_id, record.record_epoch)
                    .expect("canonical Vega revision key"),
                PrivacyStateItemRecordV1::vega_issuer_governance(record, record.record_epoch)
                    .expect("canonical Vega revision provenance"),
            );
        };

        let mut gap = Storage::new();
        insert(&mut gap, origin);
        insert(
            &mut gap,
            vega_issuer_record(
                issuer_id,
                3,
                2,
                Some(origin.record_digest),
                PrivacyVegaIssuerRecordLifecycleV1::Active,
            ),
        );
        assert!(
            privacy_vega_issuer_record_count_v1(&gap.view())
                .expect_err("skipped epoch must reject")
                .contains("successor epoch")
        );

        let revoked = vega_issuer_record(
            issuer_id,
            2,
            1,
            Some(origin.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Revoked,
        );
        let mut after_terminal = Storage::new();
        insert(&mut after_terminal, origin);
        insert(&mut after_terminal, revoked);
        insert(
            &mut after_terminal,
            vega_issuer_record(
                issuer_id,
                3,
                2,
                Some(revoked.record_digest),
                PrivacyVegaIssuerRecordLifecycleV1::Active,
            ),
        );
        assert!(
            privacy_vega_issuer_record_count_v1(&after_terminal.view())
                .expect_err("terminal lineage cannot advance")
                .contains("not active")
        );

        let mut wrong_predecessor = Storage::new();
        insert(&mut wrong_predecessor, origin);
        insert(
            &mut wrong_predecessor,
            vega_issuer_record(
                issuer_id,
                2,
                2,
                Some(PrivacyVegaIssuerRecordDigestV1::new(nonzero(0xC4))),
                PrivacyVegaIssuerRecordLifecycleV1::Active,
            ),
        );
        assert!(
            privacy_vega_issuer_record_count_v1(&wrong_predecessor.view())
                .expect_err("substituted predecessor must reject")
                .contains("predecessor digest")
        );
    }

    #[test]
    fn vega_issuer_registry_accepts_exact_caps_and_rejects_cap_plus_one() {
        let mut lineage = Storage::new();
        let issuer_id = PrivacyIssuerIdV1::new(nonzero(0xC5));
        let mut current = vega_issuer_record(
            issuer_id,
            1,
            1,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        for epoch in 1..=VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1 {
            let epoch = u64::try_from(epoch).expect("lineage cap fits u64");
            if epoch > 1 {
                current = vega_issuer_record(
                    issuer_id,
                    epoch,
                    epoch,
                    Some(current.record_digest),
                    PrivacyVegaIssuerRecordLifecycleV1::Active,
                );
            }
            lineage.insert(
                PrivacyCommitmentKeyV1::vega_issuer_revision(issuer_id, epoch)
                    .expect("bounded Vega lineage key"),
                PrivacyStateItemRecordV1::vega_issuer_governance(current, 7)
                    .expect("bounded Vega lineage record"),
            );
        }
        assert_eq!(
            privacy_vega_issuer_record_count_v1(&lineage.view()).expect("exact per-lineage cap"),
            VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1
        );
        let over_epoch = u64::try_from(VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1)
            .expect("cap fits u64")
            + 1;
        let over = vega_issuer_record(
            issuer_id,
            over_epoch,
            over_epoch,
            Some(current.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        lineage.insert(
            PrivacyCommitmentKeyV1::vega_issuer_revision(issuer_id, over_epoch)
                .expect("over-cap Vega lineage key"),
            PrivacyStateItemRecordV1::vega_issuer_governance(over, 7)
                .expect("over-cap Vega lineage record"),
        );
        assert!(
            privacy_vega_issuer_record_count_v1(&lineage.view())
                .expect_err("per-lineage cap plus one must reject")
                .contains("exceeds 64 revisions")
        );

        let mut global = Storage::new();
        for index in 1..=VEGA_MAX_ISSUER_RECORDS_V1 {
            let index = u64::try_from(index).expect("global cap fits u64");
            let issuer_id = PrivacyIssuerIdV1::new(indexed_nonzero(0xC6, index));
            let record = vega_issuer_record(
                issuer_id,
                1,
                index,
                None,
                PrivacyVegaIssuerRecordLifecycleV1::Active,
            );
            global.insert(
                PrivacyCommitmentKeyV1::vega_issuer_revision(issuer_id, 1)
                    .expect("bounded global Vega key"),
                PrivacyStateItemRecordV1::vega_issuer_governance(record, 7)
                    .expect("bounded global Vega record"),
            );
        }
        assert_eq!(
            privacy_vega_issuer_record_count_v1(&global.view()).expect("exact global Vega cap"),
            VEGA_MAX_ISSUER_RECORDS_V1
        );
        let over_index =
            u64::try_from(VEGA_MAX_ISSUER_RECORDS_V1).expect("global cap fits u64") + 1;
        let over_issuer = PrivacyIssuerIdV1::new(indexed_nonzero(0xC6, over_index));
        let over_record = vega_issuer_record(
            over_issuer,
            1,
            over_index,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        global.insert(
            PrivacyCommitmentKeyV1::vega_issuer_revision(over_issuer, 1)
                .expect("over-cap global Vega key"),
            PrivacyStateItemRecordV1::vega_issuer_governance(over_record, 7)
                .expect("over-cap global Vega record"),
        );
        assert!(
            privacy_vega_issuer_record_count_v1(&global.view())
                .expect_err("global Vega cap plus one must reject")
                .contains("revision count exceeds 4096")
        );
    }

    #[test]
    fn zk_ace_storage_keys_are_scoped_role_separated_and_nonzero() {
        let policy_a = zk_ace_policy_id(1);
        let policy_b = zk_ace_policy_id(2);
        let replay = PrivacyNullifierV1::new(nonzero(21));

        let policy_key =
            PrivacyCommitmentKeyV1::zk_ace_policy(policy_a).expect("nonzero policy key");
        assert_eq!(
            policy_key.protocol_id(),
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        );
        assert_eq!(policy_key.zk_ams_namespace(), None);
        assert!(PrivacyCommitmentKeyV1::zk_ace_policy(PrivacyPolicyIdV1::new([0; 32])).is_err());

        let replay_a =
            PrivacyNullifierKeyV1::zk_ace_replay(policy_a, replay).expect("scoped replay key");
        let replay_b =
            PrivacyNullifierKeyV1::zk_ace_replay(policy_b, replay).expect("scoped replay key");
        assert_ne!(
            replay_a, replay_b,
            "the same nullifier in distinct policy lineages must not alias"
        );
        assert_eq!(
            replay_a.protocol_id(),
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        );
        assert_eq!(replay_a.zk_ams_namespace(), None);
        assert!(
            PrivacyNullifierKeyV1::zk_ace_replay(PrivacyPolicyIdV1::new([0; 32]), replay,).is_err()
        );
        assert!(
            PrivacyNullifierKeyV1::zk_ace_replay(policy_a, PrivacyNullifierV1::new([0; 32]))
                .is_err()
        );

        let mut encoded = String::new();
        replay_a.encode_json_key(&mut encoded);
        let canonical = norito::json::from_json::<String>(&encoded).expect("JSON key string");
        assert_eq!(
            PrivacyNullifierKeyV1::decode_json_key(&canonical).expect("decode canonical key"),
            replay_a
        );
    }

    #[test]
    fn zk_x509_certificate_nullifier_keys_are_policy_scoped_role_closed_and_canonical() {
        let namespace_a = x509_namespace();
        let namespace_b = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
                trust_anchor_id: PrivacyIssuerIdV1::new(nonzero(41)),
                policy_id: PrivacyPolicyIdV1::new(nonzero(43)),
            }),
        );
        let nullifier = PrivacyNullifierV1::new(nonzero(44));
        let key_a = PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(namespace_a, nullifier)
            .expect("canonical X.509 replay key A");
        let key_b = PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(namespace_b, nullifier)
            .expect("canonical X.509 replay key B");

        assert_ne!(
            key_a, key_b,
            "the same certificate nullifier in distinct policy lineages must not alias"
        );
        assert_eq!(
            key_a.protocol_id(),
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
        );
        assert_eq!(
            key_a.zk_x509_certificate_identity(),
            Some((namespace_a, nullifier))
        );
        assert_eq!(key_a.zk_ams_namespace(), None);
        assert_eq!(key_a.proof_managed_identity(), None);
        assert!(
            PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(x509_ca_namespace(), nullifier,)
                .is_err(),
            "trust-anchor-only namespaces cannot consume certificate nullifiers"
        );
        assert!(
            PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(
                namespace_a,
                PrivacyNullifierV1::new([0; 32]),
            )
            .is_err()
        );
        assert!(
            PrivacyNullifierKeyV1::proof_managed_nullifier(namespace_a, nullifier).is_err(),
            "X.509 replay state must not alias the proof-managed pool role"
        );
        assert!(
            PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(
                PrivacyNamespaceV1::new(
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                    namespace_a.scope(),
                ),
                nullifier,
            )
            .is_err(),
            "a TrustAnchorPolicy scope cannot be relabelled under another protocol"
        );
        assert!(
            PrivacyNullifierKeyV1::zk_x509_certificate_nullifier_range(namespace_a)
                .contains(&key_a)
        );
        assert!(
            !PrivacyNullifierKeyV1::zk_x509_certificate_nullifier_range(namespace_b)
                .contains(&key_a)
        );

        let mut encoded_json = String::new();
        key_a.encode_json_key(&mut encoded_json);
        let encoded =
            norito::json::from_json::<String>(&encoded_json).expect("canonical key string");
        assert_eq!(
            PrivacyNullifierKeyV1::decode_json_key(&encoded)
                .expect("canonical X.509 replay-key roundtrip"),
            key_a
        );
        assert!(PrivacyNullifierKeyV1::decode_json_key(&encoded.to_ascii_lowercase()).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&format!(" {encoded}")).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&format!("{encoded} ")).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&encoded[..encoded.len() - 1]).is_err());
        let mut trailing = hex::decode(&encoded).expect("canonical key hex");
        trailing.push(0);
        assert!(
            PrivacyNullifierKeyV1::decode_json_key(&hex::encode_upper(trailing)).is_err(),
            "trailing Norito bytes must reject"
        );
    }

    #[test]
    fn zk_x509_certificate_nullifier_provenance_is_complete_and_strict() {
        let record = PrivacyStateItemRecordV1::zk_x509_verified_certificate_nullifier(
            PrivacyZkX509TrustAnchorRecordDigestV1::new(nonzero(51)),
            2,
            PrivacyZkX509CertificatePolicyRecordDigestV1::new(nonzero(52)),
            3,
            PrivacyZkX509CrlRecordDigestV1::new(nonzero(53)),
            4,
            PrivacyStatementDigestV1::new(nonzero(54)),
            5,
            6,
        )
        .expect("complete X.509 replay provenance");
        record.validate().expect("canonical provenance validates");

        let encoded = norito::json::to_json(&record).expect("encode X.509 replay provenance");
        assert_eq!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&encoded)
                .expect("decode exact X.509 replay provenance"),
            record
        );
        let unknown_origin = encoded.replacen(
            "\"zk_x509_verified_certificate_nullifier\"",
            "\"zk_x509_verified_certificate_nullifier_legacy\"",
            1,
        );
        assert_ne!(
            unknown_origin, encoded,
            "fixture must contain the closed provenance origin"
        );
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&unknown_origin).is_err(),
            "unknown provenance origins must reject"
        );
        let unknown_field = encoded.replacen(
            "\"action_index\":6",
            "\"action_index\":6,\"legacy\":true",
            1,
        );
        assert_ne!(
            unknown_field, encoded,
            "fixture must contain the action index"
        );
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&unknown_field).is_err(),
            "first-release durable provenance rejects unknown fields"
        );

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            trust_anchor_record_digest,
            ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *trust_anchor_record_digest = PrivacyZkX509TrustAnchorRecordDigestV1::new([0; 32]);
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            trust_anchor_record_epoch,
            ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *trust_anchor_record_epoch = 0;
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            certificate_policy_record_digest,
            ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *certificate_policy_record_digest =
            PrivacyZkX509CertificatePolicyRecordDigestV1::new([0; 32]);
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            certificate_policy_record_epoch,
            ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *certificate_policy_record_epoch = 0;
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            crl_record_digest, ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *crl_record_digest = PrivacyZkX509CrlRecordDigestV1::new([0; 32]);
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            crl_record_epoch, ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *crl_record_epoch = 0;
        assert!(malformed.validate().is_err());

        let mut malformed = record.clone();
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            statement_digest, ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *statement_digest = PrivacyStatementDigestV1::new([0; 32]);
        assert!(malformed.validate().is_err());

        let mut malformed = record;
        let PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier {
            admitted_at_height, ..
        } = &mut malformed
        else {
            unreachable!("fixture has the X.509 replay role")
        };
        *admitted_at_height = 0;
        assert!(malformed.validate().is_err());
    }

    #[test]
    fn proof_managed_pool_keys_are_closed_scoped_and_nonzero() {
        let fcmp_a = fcmp_namespace(0xC1);
        let fcmp_b = fcmp_namespace(0xC2);
        let ivm = ivm_private_note_namespace(0xC1, 0xC3);
        let pq = pq_masp_namespace(0xC1);
        let commitment = PrivacyCommitmentV1::new(nonzero(0xC4));
        let nullifier = PrivacyNullifierV1::new(nonzero(0xC5));

        for (namespace, protocol_id, role) in [
            (
                ivm,
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyRootRoleV1::ProgramState,
            ),
            (
                pq,
                PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyRootRoleV1::NoteCommitmentAnchor,
            ),
        ] {
            assert_eq!(
                proof_managed_pool_root_role_v1(namespace).expect("closed pool namespace"),
                role
            );
            let config = PrivacyCommitmentKeyV1::proof_managed_pool_config(namespace)
                .expect("typed config key");
            let output =
                PrivacyCommitmentKeyV1::proof_managed_pool_commitment(namespace, commitment)
                    .expect("typed commitment key");
            let replay = PrivacyNullifierKeyV1::proof_managed_nullifier(namespace, nullifier)
                .expect("typed nullifier key");
            assert_eq!(config.protocol_id(), protocol_id);
            assert_eq!(output.protocol_id(), protocol_id);
            assert_eq!(replay.protocol_id(), protocol_id);
            assert_eq!(config.proof_managed_namespace(), Some(namespace));
            assert_eq!(output.proof_managed_namespace(), Some(namespace));
            assert_eq!(
                replay.proof_managed_identity(),
                Some((namespace, nullifier))
            );
            assert!(
                PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
                    namespace,
                    PrivacyCommitmentV1::new([0; 32]),
                )
                .is_err()
            );
            assert!(
                PrivacyNullifierKeyV1::proof_managed_nullifier(
                    namespace,
                    PrivacyNullifierV1::new([0; 32]),
                )
                .is_err()
            );
        }

        assert_eq!(
            proof_managed_pool_root_role_v1(fcmp_a).expect("closed FCMP++ namespace"),
            PrivacyRootRoleV1::OutputSet
        );
        let fcmp_output_id = PrivacyFcmpOutputIdV1::new(nonzero(0xC4));
        let fcmp_key_image = PrivacyFcmpKeyImageV1::new(nonzero(0xC5));
        let fcmp_config =
            PrivacyCommitmentKeyV1::proof_managed_pool_config(fcmp_a).expect("FCMP++ config key");
        let same_fcmp_a =
            PrivacyCommitmentKeyV1::fcmp_output(fcmp_a, fcmp_output_id).expect("FCMP++ key A");
        let same_fcmp_b =
            PrivacyCommitmentKeyV1::fcmp_output(fcmp_b, fcmp_output_id).expect("FCMP++ key B");
        let fcmp_replay = PrivacyNullifierKeyV1::fcmp_key_image(fcmp_a, fcmp_key_image)
            .expect("FCMP++ replay key");
        assert_eq!(
            fcmp_config.protocol_id(),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        );
        assert_eq!(
            same_fcmp_a.protocol_id(),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        );
        assert_eq!(
            fcmp_replay.protocol_id(),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        );
        assert_eq!(fcmp_config.proof_managed_namespace(), Some(fcmp_a));
        assert_eq!(same_fcmp_a.proof_managed_namespace(), Some(fcmp_a));
        assert_eq!(fcmp_replay.fcmp_identity(), Some((fcmp_a, fcmp_key_image)));
        assert!(
            PrivacyCommitmentKeyV1::fcmp_output(fcmp_a, PrivacyFcmpOutputIdV1::new([0; 32]),)
                .is_err()
        );
        assert!(
            PrivacyNullifierKeyV1::fcmp_key_image(fcmp_a, PrivacyFcmpKeyImageV1::new([0; 32]),)
                .is_err()
        );
        assert!(PrivacyCommitmentKeyV1::proof_managed_pool_commitment(fcmp_a, commitment).is_err());
        assert!(PrivacyNullifierKeyV1::proof_managed_nullifier(fcmp_a, nullifier).is_err());

        let same_ivm = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(ivm, commitment)
            .expect("private-IVM key");
        let same_pq = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(pq, commitment)
            .expect("PQ-MASP key");
        assert_ne!(same_fcmp_a, same_fcmp_b);
        assert_ne!(same_fcmp_a, same_ivm);
        assert_ne!(same_fcmp_a, same_pq);

        assert!(
            PrivacyCommitmentKeyV1::proof_managed_pool_config(orchard_namespace(0xC1)).is_err()
        );
        assert!(
            PrivacyNullifierKeyV1::proof_managed_nullifier(zk_ams_namespace(0xC1), nullifier,)
                .is_err()
        );

        let item = PrivacyStateItemRecordV1::proof_managed_pool_bootstrap_commitment(
            PrivacyProofManagedPoolBootstrapDigestV1::new(nonzero(0xC6)),
            0,
            7,
        )
        .expect("canonical bootstrap item");
        let encoded = norito::json::to_json(&item).expect("encode bootstrap item");
        let unknown = encoded.replacen(
            "\"admitted_at_height\":7",
            "\"admitted_at_height\":7,\"legacy\":true",
            1,
        );
        assert_ne!(unknown, encoded);
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&unknown).is_err(),
            "first-release durable records reject unknown legacy fields"
        );
    }

    #[test]
    fn proof_managed_note_frontier_is_durable_bounded_and_self_authenticating() {
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("private_note").expect("asset name"),
        );
        let bootstrap = PrivacyProofManagedPoolBootstrapV1::IrohaIvmPrivateNoteStarkV1(
            iroha_data_model::privacy::PrivacyIvmPrivateNotePoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(0xC7)),
                asset_definition_id,
                public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
                reserve_account: account(0xC8),
                program_id: iroha_data_model::privacy::PrivacyProgramIdV1::new(nonzero(0xC9)),
                initial_note_commitments: vec![
                    PrivacyCommitmentV1::new(nonzero(0xCA)),
                    PrivacyCommitmentV1::new(nonzero(0xCB)),
                ],
            },
        );
        let bootstrap_digest = bootstrap.digest().expect("canonical bootstrap digest");
        let initial_root = crate::privacy_engines::proof_managed_pool_initial_root_v1(&bootstrap)
            .expect("native IVM frontier");
        let origin_record = PrivacyStateItemRecordV1::proof_managed_pool_bootstrap(
            bootstrap.clone(),
            bootstrap_digest,
            initial_root,
            7,
        )
        .expect("canonical origin record");
        origin_record.validate().expect("origin record validates");
        let (_, observed_digest, observed_root, origin_state, admitted_at_height) = origin_record
            .proof_managed_pool_bootstrap_ref()
            .expect("typed config record");
        assert_eq!(observed_digest, bootstrap_digest);
        assert_eq!(observed_root, initial_root);
        assert_eq!(admitted_at_height, 7);
        let origin_state = origin_state
            .private_note()
            .expect("IVM pool carries compact frontier");
        assert_eq!(origin_state.namespace(), bootstrap.namespace());
        assert_eq!(origin_state.epoch(), 1);
        assert_eq!(origin_state.root(), initial_root);

        let successor = origin_state
            .advance(
                &bootstrap,
                &[
                    PrivacyCommitmentV1::new(nonzero(0xCC)),
                    PrivacyCommitmentV1::new(nonzero(0xCD)),
                ],
            )
            .expect("bounded successor");
        assert_eq!(successor.epoch(), 2);
        assert_ne!(successor.root(), initial_root);
        let successor_record = PrivacyStateItemRecordV1::proof_managed_pool_state(
            bootstrap.clone(),
            bootstrap_digest,
            initial_root,
            PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(successor.clone()),
            7,
        )
        .expect("durable successor record");
        successor_record
            .validate()
            .expect("successor compact frontier reconstructs");

        let mut substituted_root = successor.clone();
        substituted_root.root.0[0] ^= 1;
        assert!(
            PrivacyStateItemRecordV1::proof_managed_pool_state(
                bootstrap.clone(),
                bootstrap_digest,
                initial_root,
                PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(substituted_root),
                7,
            )
            .is_err(),
            "substituted compact-frontier root must reject"
        );

        let mut impossible_epoch = successor.clone();
        impossible_epoch.epoch = 4;
        assert!(
            PrivacyStateItemRecordV1::proof_managed_pool_state(
                bootstrap.clone(),
                bootstrap_digest,
                initial_root,
                PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(impossible_epoch),
                7,
            )
            .is_err(),
            "tree size inconsistent with transition epoch must reject"
        );

        let mut cross_namespace = successor;
        cross_namespace.namespace = pq_masp_namespace(0xC7);
        assert!(
            PrivacyStateItemRecordV1::proof_managed_pool_state(
                bootstrap.clone(),
                bootstrap_digest,
                initial_root,
                PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(cross_namespace),
                7,
            )
            .is_err(),
            "cross-protocol frontier substitution must reject"
        );

        let encoded = norito::json::to_json(&successor_record).expect("encode successor record");
        let unknown_nested = encoded.replacen(
            "\"tree_size\":4",
            "\"tree_size\":4,\"legacy_frontier\":[]",
            1,
        );
        assert_ne!(unknown_nested, encoded, "fixture contains frontier size");
        assert!(
            norito::json::from_json::<PrivacyStateItemRecordV1>(&unknown_nested).is_err(),
            "unknown nested durable frontier fields must reject"
        );
    }

    #[test]
    fn proof_managed_snapshot_rejects_frontier_commitment_divergence_after_restart() {
        let mut fixture = proof_managed_persisted_fixture();
        let origin = fixture.load().expect("coherent proof-managed origin");
        assert_eq!(origin.current_epoch(), 1);
        assert_eq!(origin.current_root(), fixture.initial_root);
        assert_eq!(
            origin
                .accumulator_state()
                .expect("IVM accumulator")
                .tree_size(),
            2
        );

        let commitments_json =
            norito::json::to_json(&fixture.commitments).expect("encode commitments");
        let roots_json = norito::json::to_json(&fixture.roots).expect("encode roots");
        let heads_json = norito::json::to_json(&fixture.root_heads).expect("encode heads");
        fixture.commitments =
            norito::json::from_json(&commitments_json).expect("restore commitments");
        fixture.roots = norito::json::from_json(&roots_json).expect("restore roots");
        fixture.root_heads = norito::json::from_json(&heads_json).expect("restore heads");
        fixture
            .load()
            .expect("snapshot round-trip preserves the authenticated frontier");

        let mut extra = proof_managed_persisted_fixture();
        let extra_commitment = PrivacyCommitmentV1::new(nonzero(0xB6));
        extra.commitments.insert(
            PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
                extra.namespace,
                extra_commitment,
            )
            .expect("extra commitment key"),
            PrivacyStateItemRecordV1::proof_managed_pool_verified_commitment(
                extra.bootstrap_digest,
                PrivacyStatementDigestV1::new(nonzero(0xB7)),
                2,
                0,
                2,
                1,
                1,
                8,
                0,
            )
            .expect("extra item"),
        );
        let error = extra.load().expect_err("uncommitted output must reject");
        assert!(
            error.contains("output epochs"),
            "uncommitted output must fail the persisted epoch/head binding, got `{error}`"
        );

        let outputs = [
            PrivacyCommitmentV1::new(nonzero(0xB8)),
            PrivacyCommitmentV1::new(nonzero(0xB9)),
        ];
        fixture.advance(&outputs);
        let advanced = fixture.load().expect("coherent proof-managed successor");
        assert_eq!(advanced.current_epoch(), 2);
        assert_eq!(
            advanced
                .accumulator_state()
                .expect("IVM successor accumulator")
                .tree_size(),
            4
        );
        assert!(
            advanced.contains_retained_root(1, fixture.initial_root),
            "restart snapshot retains the historical proof anchor"
        );
        assert_eq!(
            advanced.retained_current_root(),
            Some((advanced.current_epoch(), advanced.current_root()))
        );

        let commitments_json =
            norito::json::to_json(&fixture.commitments).expect("encode advanced commitments");
        let roots_json = norito::json::to_json(&fixture.roots).expect("encode advanced roots");
        let heads_json =
            norito::json::to_json(&fixture.root_heads).expect("encode advanced root heads");
        fixture.commitments =
            norito::json::from_json(&commitments_json).expect("restart advanced commitments");
        fixture.roots = norito::json::from_json(&roots_json).expect("restart advanced roots");
        fixture.root_heads =
            norito::json::from_json(&heads_json).expect("restart advanced root heads");
        let restarted = fixture
            .load()
            .expect("restart preserves retained anchor and current frontier");
        assert!(restarted.contains_retained_root(1, fixture.initial_root));
        assert_eq!(
            restarted.retained_current_root(),
            Some((restarted.current_epoch(), restarted.current_root()))
        );
        let post_restart_output = PrivacyCommitmentV1::new(nonzero(0xBA));
        let post_restart_successor = restarted
            .derive_note_successor(&[post_restart_output])
            .expect("restart mutation uses the current compact frontier");
        assert_eq!(
            post_restart_successor.epoch(),
            restarted.current_epoch() + 1
        );
        assert_ne!(post_restart_successor.root(), restarted.current_root());

        let missing_key =
            PrivacyCommitmentKeyV1::proof_managed_pool_commitment(fixture.namespace, outputs[1])
                .expect("output key");
        fixture.remove_commitment(missing_key);
        let error = fixture.load().expect_err("omitted output must reject");
        assert!(
            error.contains("declares 2 outputs but restored 1"),
            "omitted output must fail at the exact declared batch arity: {error}"
        );

        let mut corrupted = proof_managed_persisted_fixture();
        let mut config = corrupted
            .commitments
            .view()
            .get(&corrupted.config_key)
            .expect("config")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolBootstrap {
            accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(state),
            ..
        } = &mut config
        else {
            panic!("IVM config must carry a compact frontier");
        };
        state.leaf.as_mut().expect("non-empty frontier")[0] ^= 1;
        corrupted.commitments.insert(corrupted.config_key, config);
        assert!(
            corrupted
                .load()
                .expect_err("mutated compact frontier must reject")
                .contains("compact frontier")
        );
    }

    #[test]
    fn proof_managed_batch_identity_cannot_replay_with_different_arities() {
        let statement_digest = PrivacyStatementDigestV1::new(nonzero(0xBB));
        let origins = [
            ProofManagedCommitmentOriginV1::bootstrap(0, 7),
            ProofManagedCommitmentOriginV1::verified(statement_digest, 2, 0, 1, 1, 1, 10, 0),
            ProofManagedCommitmentOriginV1::verified(statement_digest, 3, 0, 2, 2, 1, 10, 0),
        ];
        assert!(
            validate_proof_managed_origin_sequence_v1(&origins, 1, 7, 2, 2)
                .expect_err("same proof/action identity cannot replay with different arities")
                .contains("replay verified provenance")
        );
    }

    #[test]
    fn proof_managed_snapshot_rejects_append_order_epoch_and_role_corruption() {
        let outputs = [
            PrivacyCommitmentV1::new(nonzero(0xB8)),
            PrivacyCommitmentV1::new(nonzero(0xB9)),
        ];
        let output_key = |fixture: &ProofManagedPersistedFixture, index: usize| {
            PrivacyCommitmentKeyV1::proof_managed_pool_commitment(fixture.namespace, outputs[index])
                .expect("output key")
        };

        let mut duplicate_position = proof_managed_persisted_fixture();
        duplicate_position.advance(&outputs);
        let key = output_key(&duplicate_position, 1);
        let mut record = duplicate_position
            .commitments
            .view()
            .get(&key)
            .expect("second output")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment {
            append_position, ..
        } = &mut record
        else {
            panic!("output must carry commitment provenance");
        };
        *append_position = 2;
        duplicate_position.commitments.insert(key, record);
        assert!(
            duplicate_position
                .load()
                .expect_err("duplicate append position must reject")
                .contains("duplicate position")
        );

        let mut duplicate_output_index = proof_managed_persisted_fixture();
        duplicate_output_index.advance(&outputs);
        let key = output_key(&duplicate_output_index, 1);
        let mut record = duplicate_output_index
            .commitments
            .view()
            .get(&key)
            .expect("second output")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment { output_index, .. } =
            &mut record
        else {
            panic!("output must carry commitment provenance");
        };
        *output_index = 0;
        duplicate_output_index.commitments.insert(key, record);
        assert!(
            duplicate_output_index
                .load()
                .expect_err("duplicate output index must reject")
                .contains("statement order")
        );

        let mut future_epoch = proof_managed_persisted_fixture();
        future_epoch.advance(&outputs);
        let key = output_key(&future_epoch, 0);
        let mut record = future_epoch
            .commitments
            .view()
            .get(&key)
            .expect("first output")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment {
            successor_epoch, ..
        } = &mut record
        else {
            panic!("output must carry commitment provenance");
        };
        *successor_epoch = 3;
        future_epoch.commitments.insert(key, record);
        assert!(
            future_epoch
                .load()
                .expect_err("future output epoch must reject")
                .contains("epoch")
        );

        let mut wrong_role = proof_managed_persisted_fixture();
        wrong_role.advance(&outputs);
        let key = output_key(&wrong_role, 0);
        wrong_role.commitments.insert(
            key,
            PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
                wrong_role.bootstrap_digest,
                PrivacyStatementDigestV1::new(nonzero(0xBA)),
                1,
                1,
                12,
                0,
            )
            .expect("nullifier provenance"),
        );
        assert!(
            wrong_role
                .load()
                .expect_err("nullifier provenance under commitment key must reject")
                .contains("wrong-role")
        );

        let mut substituted_commitment = proof_managed_persisted_fixture();
        substituted_commitment.advance(&outputs);
        let original_key = output_key(&substituted_commitment, 0);
        let record = substituted_commitment
            .commitments
            .view()
            .get(&original_key)
            .expect("first output")
            .clone();
        substituted_commitment.remove_commitment(original_key);
        let replacement_key = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
            substituted_commitment.namespace,
            PrivacyCommitmentV1::new(nonzero(0xBA)),
        )
        .expect("replacement key");
        substituted_commitment
            .commitments
            .insert(replacement_key, record);
        assert!(
            substituted_commitment
                .load()
                .expect_err("position-preserving commitment substitution must reject")
                .contains("differs from its compact frontier")
        );

        let mut reordered_genesis = proof_managed_persisted_fixture();
        let genesis_key = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
            reordered_genesis.namespace,
            reordered_genesis
                .bootstrap
                .initial_note_commitments()
                .expect("private-note bootstrap commitments")[0],
        )
        .expect("genesis key");
        let mut record = reordered_genesis
            .commitments
            .view()
            .get(&genesis_key)
            .expect("first genesis commitment")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolBootstrapCommitment { position, .. } =
            &mut record
        else {
            panic!("genesis commitment must carry bootstrap position");
        };
        *position = 1;
        reordered_genesis.commitments.insert(genesis_key, record);
        assert!(
            reordered_genesis
                .load()
                .expect_err("reordered genesis prefix must reject")
                .contains("duplicate position")
        );
    }

    #[test]
    fn proof_managed_note_snapshot_rejects_mixed_root_and_declared_batch_arities() {
        let outputs = [
            PrivacyCommitmentV1::new(nonzero(0xBC)),
            PrivacyCommitmentV1::new(nonzero(0xBD)),
        ];
        for mut fixture in [
            proof_managed_persisted_fixture(),
            pq_masp_persisted_fixture(),
        ] {
            fixture.advance(&outputs);
            for output in outputs {
                let key = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
                    fixture.namespace,
                    output,
                )
                .expect("verified output key");
                let mut record = fixture
                    .commitments
                    .view()
                    .get(&key)
                    .expect("verified output")
                    .clone();
                let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment {
                    output_count,
                    ..
                } = &mut record
                else {
                    panic!("verified output provenance");
                };
                *output_count = 1;
                fixture.commitments.insert(key, record);
            }
            let error = fixture
                .load()
                .expect_err("coherently truncated declared output arity must reject");
            assert!(
                error.contains("declares 1 outputs but restored 2"),
                "unexpected {:?} output-arity rejection: {error}",
                fixture.namespace.protocol_id()
            );

            let mut fixture = proof_managed_note_persisted_fixture(fixture.namespace.protocol_id());
            fixture.advance(&outputs);
            let (root_key, provenance) = fixture
                .roots
                .view()
                .iter()
                .last()
                .map(|(key, provenance)| (*key, *provenance))
                .expect("successor root");
            let PrivacyRootProvenanceV1::ProofManagedPoolSuccessor {
                bootstrap_digest,
                protocol_id,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
                ..
            } = provenance
            else {
                panic!("successor provenance");
            };
            let forged = PrivacyRootProvenanceV1::proof_managed_pool_successor(
                bootstrap_digest,
                protocol_id,
                PrivacyStatementDigestV1::new(nonzero(0xBE)),
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                parent_epoch,
                parent_root,
            )
            .expect("locally valid mixed successor provenance");
            fixture.roots.insert(root_key, forged);
            fixture.root_heads.insert(
                fixture.head_key,
                PrivacyRootHeadRecordV1::new(root_key.epoch(), root_key.root(), forged, None)
                    .expect("forged but internally consistent head"),
            );
            let error = fixture
                .load()
                .expect_err("root/output batch provenance substitution must reject");
            assert!(
                error.contains("canonical output-batch provenance"),
                "unexpected {:?} root-provenance rejection: {error}",
                fixture.namespace.protocol_id()
            );
        }
    }

    #[test]
    fn proof_managed_restore_requires_exact_replay_marker_batches() {
        let note_outputs = [
            PrivacyCommitmentV1::new(nonzero(0xC1)),
            PrivacyCommitmentV1::new(nonzero(0xC2)),
        ];
        for mut fixture in [
            proof_managed_persisted_fixture(),
            pq_masp_persisted_fixture(),
        ] {
            fixture.advance(&note_outputs);
            let output_key = PrivacyCommitmentKeyV1::proof_managed_pool_commitment(
                fixture.namespace,
                note_outputs[0],
            )
            .expect("verified note output key");
            let output_record = fixture
                .commitments
                .view()
                .get(&output_key)
                .expect("verified note output")
                .clone();
            let PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment {
                bootstrap_digest,
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
                ..
            } = output_record
            else {
                panic!("verified note output provenance");
            };
            let mut nullifiers = Storage::new();
            let error = validate_proof_managed_fixture_maps(
                fixture.namespace.protocol_id(),
                &nullifiers,
                &fixture.commitments,
                &fixture.roots,
                &fixture.root_heads,
            )
            .expect_err("missing replay marker must reject");
            assert!(error.contains("declares 1 replay markers but restored 0"));

            let key = PrivacyNullifierKeyV1::proof_managed_nullifier(
                fixture.namespace,
                PrivacyNullifierV1::new(nonzero(0xC3)),
            )
            .expect("typed note nullifier");
            let exact = PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
                bootstrap_digest,
                statement_digest,
                nullifier_count,
                output_count,
                admitted_at_height,
                action_index,
            )
            .expect("exact replay provenance");
            nullifiers.insert(key, exact.clone());
            validate_proof_managed_fixture_maps(
                fixture.namespace.protocol_id(),
                &nullifiers,
                &fixture.commitments,
                &fixture.roots,
                &fixture.root_heads,
            )
            .expect("exact replay-marker batch restores");

            nullifiers.insert(
                key,
                PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
                    bootstrap_digest,
                    PrivacyStatementDigestV1::new(nonzero(0xC4)),
                    nullifier_count,
                    output_count,
                    admitted_at_height,
                    action_index,
                )
                .expect("locally valid mixed replay provenance"),
            );
            assert!(
                validate_proof_managed_fixture_maps(
                    fixture.namespace.protocol_id(),
                    &nullifiers,
                    &fixture.commitments,
                    &fixture.roots,
                    &fixture.root_heads,
                )
                .expect_err("mixed replay-marker provenance must reject")
                .contains("orphaned or mixed")
            );

            nullifiers.insert(key, exact.clone());
            nullifiers.insert(
                PrivacyNullifierKeyV1::proof_managed_nullifier(
                    fixture.namespace,
                    PrivacyNullifierV1::new(nonzero(0xC5)),
                )
                .expect("second typed note nullifier"),
                exact,
            );
            assert!(
                validate_proof_managed_fixture_maps(
                    fixture.namespace.protocol_id(),
                    &nullifiers,
                    &fixture.commitments,
                    &fixture.roots,
                    &fixture.root_heads,
                )
                .expect_err("surplus replay marker must reject")
                .contains("declares 1 replay markers but restored 2")
            );
        }

        let outputs = sorted_fcmp_output_tuples(&[51, 61]);
        let mut fixture = fcmp_persisted_fixture();
        fixture.advance(&outputs);
        let output_key =
            PrivacyCommitmentKeyV1::fcmp_output(fixture.namespace, outputs[0].output_id())
                .expect("verified FCMP++ output key");
        let output_record = fixture
            .commitments
            .view()
            .get(&output_key)
            .expect("verified FCMP++ output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpVerifiedOutput {
            bootstrap_digest,
            statement_digest,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
            ..
        } = output_record
        else {
            panic!("verified FCMP++ output provenance");
        };
        let mut nullifiers = Storage::new();
        assert!(
            validate_proof_managed_fixture_maps(
                fixture.namespace.protocol_id(),
                &nullifiers,
                &fixture.commitments,
                &fixture.roots,
                &fixture.root_heads,
            )
            .expect_err("missing FCMP++ key image must reject")
            .contains("declares 1 replay markers but restored 0")
        );
        let exact = PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
            bootstrap_digest,
            statement_digest,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
        )
        .expect("exact FCMP++ replay provenance");
        nullifiers.insert(
            PrivacyNullifierKeyV1::fcmp_key_image(
                fixture.namespace,
                PrivacyFcmpKeyImageV1::new(nonzero(0xC6)),
            )
            .expect("typed FCMP++ key image"),
            exact,
        );
        validate_proof_managed_fixture_maps(
            fixture.namespace.protocol_id(),
            &nullifiers,
            &fixture.commitments,
            &fixture.roots,
            &fixture.root_heads,
        )
        .expect("exact FCMP++ replay-marker batch restores");
    }

    #[test]
    fn fcmp_snapshot_round_trip_rebuilds_complete_curve_frontier() {
        let mut fixture = fcmp_persisted_fixture();
        let origin = fixture.load().expect("coherent FCMP++ origin");
        assert_eq!(origin.namespace(), fixture.namespace);
        assert_eq!(origin.root_role(), PrivacyRootRoleV1::OutputSet);
        assert_eq!(origin.current_epoch(), 1);
        assert_eq!(origin.current_root(), fixture.initial_root);
        assert_eq!(origin.output_count(), 2);
        assert!(origin.accumulator_state().is_none());
        let origin_state = origin
            .fcmp_accumulator_state()
            .expect("FCMP++ curve frontier");
        assert_eq!(origin_state.epoch(), 1);
        assert_eq!(origin_state.tree_size(), 2);
        assert_eq!(
            origin_state.root().history_commitment(),
            fixture.initial_root
        );

        let commitments_json =
            norito::json::to_json(&fixture.commitments).expect("encode FCMP++ outputs");
        let roots_json = norito::json::to_json(&fixture.roots).expect("encode FCMP++ roots");
        let heads_json = norito::json::to_json(&fixture.root_heads).expect("encode FCMP++ head");
        fixture.commitments =
            norito::json::from_json(&commitments_json).expect("restore FCMP++ outputs");
        fixture.roots = norito::json::from_json(&roots_json).expect("restore FCMP++ roots");
        fixture.root_heads =
            norito::json::from_json(&heads_json).expect("restore FCMP++ root head");
        fixture
            .load()
            .expect("restored complete tuples rebuild the exact curve frontier");

        let outputs = sorted_fcmp_output_tuples(&[31, 41]);
        fixture.advance(&outputs);
        let advanced = fixture.load().expect("coherent FCMP++ successor");
        assert_eq!(advanced.current_epoch(), 2);
        assert_eq!(advanced.output_count(), 4);
        assert!(advanced.accumulator_state().is_none());
        let state = advanced
            .fcmp_accumulator_state()
            .expect("FCMP++ successor frontier");
        assert_eq!(state.epoch(), 2);
        assert_eq!(state.tree_size(), 4);
        assert_eq!(state.root().history_commitment(), advanced.current_root());
    }

    #[test]
    fn fcmp_snapshot_rejects_tuple_key_order_role_and_frontier_substitution() {
        let mut substituted_tuple = fcmp_persisted_fixture();
        let original = substituted_tuple
            .bootstrap
            .initial_fcmp_outputs()
            .expect("genesis outputs")[0];
        let key =
            PrivacyCommitmentKeyV1::fcmp_output(substituted_tuple.namespace, original.output_id())
                .expect("genesis key");
        let mut record = substituted_tuple
            .commitments
            .view()
            .get(&key)
            .expect("genesis output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpBootstrapOutput { output, .. } = &mut record else {
            panic!("typed FCMP++ genesis provenance");
        };
        output.amount_commitment = fcmp_output_tuple(91).amount_commitment;
        assert_ne!(output.output_id(), original.output_id());
        substituted_tuple.commitments.insert(key, record);
        assert!(
            substituted_tuple
                .load()
                .expect_err("tuple substitution under an old id must reject")
                .contains("complete tuple")
        );

        let mut reordered = fcmp_persisted_fixture();
        let second = reordered
            .bootstrap
            .initial_fcmp_outputs()
            .expect("genesis outputs")[1];
        let key = PrivacyCommitmentKeyV1::fcmp_output(reordered.namespace, second.output_id())
            .expect("second genesis key");
        let mut record = reordered
            .commitments
            .view()
            .get(&key)
            .expect("second genesis output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpBootstrapOutput { position, .. } = &mut record else {
            panic!("typed FCMP++ genesis provenance");
        };
        *position = 2;
        reordered.commitments.insert(key, record);
        assert!(
            reordered
                .load()
                .expect_err("duplicate FCMP++ append position must reject")
                .contains("duplicate position")
        );

        let mut foreign_note = fcmp_persisted_fixture();
        foreign_note.commitments.insert(
            PrivacyCommitmentKeyV1::ProofManagedPoolCommitment {
                namespace: foreign_note.namespace,
                commitment: PrivacyCommitmentV1::new(nonzero(0xD2)),
            },
            PrivacyStateItemRecordV1::proof_managed_pool_bootstrap_commitment(
                foreign_note.bootstrap_digest,
                2,
                7,
            )
            .expect("syntactically valid foreign note provenance"),
        );
        assert!(
            foreign_note
                .load()
                .expect_err("FCMP++ must reject a generic note key")
                .contains("foreign note-commitment")
        );

        let mut wrong_role = fcmp_persisted_fixture();
        let output = wrong_role
            .bootstrap
            .initial_fcmp_outputs()
            .expect("genesis outputs")[0];
        wrong_role.commitments.insert(
            PrivacyCommitmentKeyV1::fcmp_output(wrong_role.namespace, output.output_id())
                .expect("genesis key"),
            PrivacyStateItemRecordV1::proof_managed_pool_bootstrap_commitment(
                wrong_role.bootstrap_digest,
                0,
                7,
            )
            .expect("generic provenance"),
        );
        assert!(
            wrong_role
                .load()
                .expect_err("FCMP++ output key cannot carry note provenance")
                .contains("wrong-role")
        );

        let mut missing = fcmp_persisted_fixture();
        let output = missing
            .bootstrap
            .initial_fcmp_outputs()
            .expect("genesis outputs")[1];
        missing.remove_output(output);
        assert!(
            missing
                .load()
                .expect_err("omitted FCMP++ genesis tuple must reject")
                .contains("omits a canonical genesis output")
        );

        let mut corrupted_frontier = fcmp_persisted_fixture();
        let mut config = corrupted_frontier
            .commitments
            .view()
            .get(&corrupted_frontier.config_key)
            .expect("FCMP++ config")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolBootstrap {
            accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1::Fcmp(state),
            ..
        } = &mut config
        else {
            panic!("FCMP++ config carries its curve frontier");
        };
        state.active_outputs[0] = fcmp_output_tuple(101);
        corrupted_frontier
            .commitments
            .insert(corrupted_frontier.config_key, config);
        assert!(
            corrupted_frontier
                .load()
                .expect_err("valid-point frontier substitution must reject")
                .contains("frontier")
        );

        let note_fixture = proof_managed_persisted_fixture();
        let note_commitments = note_fixture.commitments.view();
        let note_config = note_commitments
            .get(&note_fixture.config_key)
            .expect("private-note config");
        let PrivacyStateItemRecordV1::ProofManagedPoolBootstrap {
            accumulator_state: PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(note_state),
            ..
        } = note_config
        else {
            panic!("private-note fixture carries its SHA-256 frontier");
        };
        let mut cross_protocol = fcmp_persisted_fixture();
        let mut config = cross_protocol
            .commitments
            .view()
            .get(&cross_protocol.config_key)
            .expect("FCMP++ config")
            .clone();
        let PrivacyStateItemRecordV1::ProofManagedPoolBootstrap {
            accumulator_state, ..
        } = &mut config
        else {
            panic!("typed FCMP++ config");
        };
        *accumulator_state =
            PrivacyProofManagedPoolAccumulatorStateV1::PrivateNote(note_state.clone());
        cross_protocol
            .commitments
            .insert(cross_protocol.config_key, config);
        assert!(
            cross_protocol
                .load()
                .expect_err("FCMP++ cannot decode as a private-note frontier")
                .contains("foreign SHA-256")
        );
    }

    #[test]
    fn fcmp_snapshot_rejects_verified_output_provenance_corruption() {
        let outputs = sorted_fcmp_output_tuples(&[31, 41]);

        let mut duplicate_position = fcmp_persisted_fixture();
        duplicate_position.advance(&outputs);
        let second_key = PrivacyCommitmentKeyV1::fcmp_output(
            duplicate_position.namespace,
            outputs[1].output_id(),
        )
        .expect("second output key");
        let mut record = duplicate_position
            .commitments
            .view()
            .get(&second_key)
            .expect("second verified output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpVerifiedOutput {
            append_position, ..
        } = &mut record
        else {
            panic!("verified FCMP++ provenance");
        };
        *append_position = 2;
        duplicate_position.commitments.insert(second_key, record);
        assert!(
            duplicate_position
                .load()
                .expect_err("duplicate verified append position must reject")
                .contains("duplicate position")
        );

        let mut duplicate_output_index = fcmp_persisted_fixture();
        duplicate_output_index.advance(&outputs);
        let second_key = PrivacyCommitmentKeyV1::fcmp_output(
            duplicate_output_index.namespace,
            outputs[1].output_id(),
        )
        .expect("second output key");
        let mut record = duplicate_output_index
            .commitments
            .view()
            .get(&second_key)
            .expect("second verified output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpVerifiedOutput { output_index, .. } = &mut record else {
            panic!("verified FCMP++ provenance");
        };
        *output_index = 0;
        duplicate_output_index
            .commitments
            .insert(second_key, record);
        assert!(
            duplicate_output_index
                .load()
                .expect_err("duplicate statement output index must reject")
                .contains("statement order")
        );

        let mut substituted_output = fcmp_persisted_fixture();
        substituted_output.advance(&outputs);
        let first_key = PrivacyCommitmentKeyV1::fcmp_output(
            substituted_output.namespace,
            outputs[0].output_id(),
        )
        .expect("first output key");
        let mut record = substituted_output
            .commitments
            .view()
            .get(&first_key)
            .expect("first verified output")
            .clone();
        let PrivacyStateItemRecordV1::FcmpVerifiedOutput { output, .. } = &mut record else {
            panic!("verified FCMP++ provenance");
        };
        *output = fcmp_output_tuple(111);
        substituted_output.commitments.insert(first_key, record);
        assert!(
            substituted_output
                .load()
                .expect_err("position-preserving complete tuple substitution must reject")
                .contains("complete tuple")
        );

        let mut missing_output = fcmp_persisted_fixture();
        missing_output.advance(&outputs);
        missing_output.remove_output(outputs[1]);
        let error = missing_output
            .load()
            .expect_err("omitted verified FCMP++ output must reject");
        assert!(
            error.contains("declares 2 outputs but restored 1"),
            "omitted FCMP++ output must fail at the exact declared batch arity: {error}"
        );

        let mut wrong_arity = fcmp_persisted_fixture();
        wrong_arity.advance(&outputs);
        for output in &outputs {
            let key =
                PrivacyCommitmentKeyV1::fcmp_output(wrong_arity.namespace, output.output_id())
                    .expect("verified FCMP++ output key");
            let mut record = wrong_arity
                .commitments
                .view()
                .get(&key)
                .expect("verified FCMP++ output")
                .clone();
            let PrivacyStateItemRecordV1::FcmpVerifiedOutput { output_count, .. } = &mut record
            else {
                panic!("verified FCMP++ output provenance");
            };
            *output_count = 1;
            wrong_arity.commitments.insert(key, record);
        }
        assert!(
            wrong_arity
                .load()
                .expect_err("coherently truncated FCMP++ output arity must reject")
                .contains("declares 1 outputs but restored 2")
        );

        let mut mixed_root = fcmp_persisted_fixture();
        mixed_root.advance(&outputs);
        let (root_key, provenance) = mixed_root
            .roots
            .view()
            .iter()
            .last()
            .map(|(key, provenance)| (*key, *provenance))
            .expect("FCMP++ successor root");
        let PrivacyRootProvenanceV1::ProofManagedPoolSuccessor {
            bootstrap_digest,
            protocol_id,
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
            ..
        } = provenance
        else {
            panic!("FCMP++ successor provenance");
        };
        let forged = PrivacyRootProvenanceV1::proof_managed_pool_successor(
            bootstrap_digest,
            protocol_id,
            PrivacyStatementDigestV1::new(nonzero(0xBF)),
            nullifier_count,
            output_count,
            admitted_at_height,
            action_index,
            parent_epoch,
            parent_root,
        )
        .expect("locally valid mixed FCMP++ root provenance");
        mixed_root.roots.insert(root_key, forged);
        mixed_root.root_heads.insert(
            mixed_root.head_key,
            PrivacyRootHeadRecordV1::new(root_key.epoch(), root_key.root(), forged, None)
                .expect("forged but internally consistent FCMP++ head"),
        );
        assert!(
            mixed_root
                .load()
                .expect_err("FCMP++ root/output batch substitution must reject")
                .contains("canonical output-batch provenance")
        );
    }

    #[test]
    fn proof_managed_root_chain_rejects_cross_origin_gaps_and_forged_anchors() {
        let namespace = fcmp_namespace(0xD1);
        let protocol_id = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        let role = PrivacyRootRoleV1::OutputSet;
        let bootstrap_digest = PrivacyProofManagedPoolBootstrapDigestV1::new(nonzero(0xD2));
        let roots = [
            PrivacyRootV1::new(nonzero(0xD3)),
            PrivacyRootV1::new(nonzero(0xD4)),
            PrivacyRootV1::new(nonzero(0xD5)),
            PrivacyRootV1::new(nonzero(0xD6)),
        ];
        let statement = |byte| PrivacyStatementDigestV1::new(nonzero(byte));
        let key = |epoch: u64, root: PrivacyRootV1| {
            PrivacyRootKeyV1::new(namespace, role, epoch, root).expect("root key")
        };
        let bootstrap =
            PrivacyRootProvenanceV1::proof_managed_pool_bootstrap(bootstrap_digest, protocol_id, 7)
                .expect("bootstrap provenance");
        let successor = |epoch: u64,
                         parent_root: PrivacyRootV1,
                         digest: PrivacyProofManagedPoolBootstrapDigestV1,
                         protocol: PrivacyProtocolIdV1| {
            PrivacyRootProvenanceV1::proof_managed_pool_successor(
                digest,
                protocol,
                statement(u8::try_from(0xD6_u64 + epoch).expect("small epoch")),
                1,
                1,
                7 + epoch,
                0,
                epoch - 1,
                parent_root,
            )
            .expect("successor provenance")
        };
        let complete = vec![
            (key(1, roots[0]), bootstrap),
            (
                key(2, roots[1]),
                successor(2, roots[0], bootstrap_digest, protocol_id),
            ),
            (
                key(3, roots[2]),
                successor(3, roots[1], bootstrap_digest, protocol_id),
            ),
            (
                key(4, roots[3]),
                successor(4, roots[2], bootstrap_digest, protocol_id),
            ),
        ];
        validate_proof_managed_pool_retained_root_chain_v1(
            namespace,
            bootstrap_digest,
            roots[0],
            4,
            None,
            &complete,
        )
        .expect("complete canonical root chain");

        let anchored = complete[2..].to_vec();
        let anchor = PrivacyRootRetentionAnchorV1::new(2, roots[1]).expect("prefix anchor");
        validate_proof_managed_pool_retained_root_chain_v1(
            namespace,
            bootstrap_digest,
            roots[0],
            2,
            Some(anchor),
            &anchored,
        )
        .expect("exact pruned-prefix chain");

        for (label, initial_root, anchor, history) in [
            (
                "substituted origin root",
                PrivacyRootV1::new(nonzero(0xE1)),
                None,
                complete.clone(),
            ),
            (
                "anchor alongside bootstrap",
                roots[0],
                Some(
                    PrivacyRootRetentionAnchorV1::new(1, roots[0])
                        .expect("unexpected bootstrap anchor"),
                ),
                complete.clone(),
            ),
            ("missing pruned anchor", roots[0], None, anchored.clone()),
            (
                "forged pruned anchor",
                roots[0],
                Some(
                    PrivacyRootRetentionAnchorV1::new(2, PrivacyRootV1::new(nonzero(0xE2)))
                        .expect("forged anchor"),
                ),
                anchored.clone(),
            ),
        ] {
            assert!(
                validate_proof_managed_pool_retained_root_chain_v1(
                    namespace,
                    bootstrap_digest,
                    initial_root,
                    if history.len() == complete.len() {
                        4
                    } else {
                        2
                    },
                    anchor,
                    &history,
                )
                .is_err(),
                "{label} must reject"
            );
        }

        let mut forged_parent = complete.clone();
        forged_parent[2].1 = successor(
            3,
            PrivacyRootV1::new(nonzero(0xE3)),
            bootstrap_digest,
            protocol_id,
        );
        assert!(
            validate_proof_managed_pool_retained_root_chain_v1(
                namespace,
                bootstrap_digest,
                roots[0],
                4,
                None,
                &forged_parent,
            )
            .expect_err("forged parent")
            .contains("gap or forged parent")
        );

        let mut cross_origin = complete.clone();
        cross_origin[1].1 = successor(
            2,
            roots[0],
            PrivacyProofManagedPoolBootstrapDigestV1::new(nonzero(0xE4)),
            protocol_id,
        );
        assert!(
            validate_proof_managed_pool_retained_root_chain_v1(
                namespace,
                bootstrap_digest,
                roots[0],
                4,
                None,
                &cross_origin,
            )
            .is_err()
        );

        let mut cross_protocol = complete;
        cross_protocol[1].1 = successor(
            2,
            roots[0],
            bootstrap_digest,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        );
        assert!(
            validate_proof_managed_pool_retained_root_chain_v1(
                namespace,
                bootstrap_digest,
                roots[0],
                4,
                None,
                &cross_protocol,
            )
            .is_err()
        );
    }

    #[test]
    fn zk_ace_policy_loader_rejects_missing_wrong_role_mismatch_and_corruption() {
        let policy_id = zk_ace_policy_id(1);
        let key = PrivacyCommitmentKeyV1::zk_ace_policy(policy_id).expect("policy key");
        let policy = zk_ace_policy_record(policy_id);
        let record = PrivacyStateItemRecordV1::zk_ace_policy_governance(policy.clone(), 7)
            .expect("governance provenance");

        let mut commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        assert!(
            load_privacy_zk_ace_policy_v1(policy_id, &commitments.view())
                .expect_err("missing policy must reject")
                .contains("not registered")
        );
        commitments.insert(key, record);
        assert_eq!(
            privacy_zk_ace_policy_count_v1(&commitments.view()).expect("bounded policy count"),
            1
        );
        assert_eq!(
            load_privacy_zk_ace_policy_v1(policy_id, &commitments.view())
                .expect("load canonical policy"),
            policy
        );

        let namespace = zk_ams_namespace(20);
        commitments.insert(
            PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record(
                namespace,
                PrivacyZkAmsIssuerPolicyRecordDigestV1::new(nonzero(22)),
            )
            .expect("ZK-AMS issuer-policy key"),
            PrivacyStateItemRecordV1::zk_ams_governance(
                PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(23)),
                7,
            )
            .expect("ZK-AMS provenance"),
        );
        assert_eq!(
            privacy_zk_ace_policy_count_v1(&commitments.view())
                .expect("unrelated protocols are not counted"),
            1
        );

        let mut wrong_role = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        wrong_role.insert(
            key,
            PrivacyStateItemRecordV1::zk_ams_governance(
                PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(24)),
                7,
            )
            .expect("valid but wrong-role provenance"),
        );
        assert!(
            load_privacy_zk_ace_policy_v1(policy_id, &wrong_role.view())
                .expect_err("wrong-role value must reject")
                .contains("wrong-role")
        );

        let mismatched_policy_id = zk_ace_policy_id(2);
        let mut mismatched = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        mismatched.insert(
            key,
            PrivacyStateItemRecordV1::zk_ace_policy_governance(
                zk_ace_policy_record(mismatched_policy_id),
                7,
            )
            .expect("valid mismatched policy record"),
        );
        assert!(
            load_privacy_zk_ace_policy_v1(policy_id, &mismatched.view())
                .expect_err("key/payload mismatch must reject")
                .contains("does not match")
        );

        let mut corrupted_policy = zk_ace_policy_record(policy_id);
        corrupted_policy.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(nonzero(25));
        let mut corrupted = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        corrupted.insert(
            key,
            PrivacyStateItemRecordV1::ZkAcePolicyGovernance {
                policy: corrupted_policy,
                admitted_at_height: 7,
            },
        );
        assert!(
            load_privacy_zk_ace_policy_v1(policy_id, &corrupted.view())
                .expect_err("self-digest corruption must reject")
                .contains("self-digest mismatch")
        );

        let mut zero_height = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        zero_height.insert(
            key,
            PrivacyStateItemRecordV1::ZkAcePolicyGovernance {
                policy,
                admitted_at_height: 0,
            },
        );
        assert!(
            load_privacy_zk_ace_policy_v1(policy_id, &zero_height.view())
                .expect_err("zero policy admission height must reject")
                .contains("zero admission height")
        );
    }

    #[test]
    fn zk_ace_policy_count_rejects_state_above_the_closed_global_bound() {
        let template = zk_ace_policy_record(zk_ace_policy_id(1));
        let mut commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        for index in
            1..=u64::try_from(PRIVACY_ZK_ACE_MAX_POLICIES_V1 + 1).expect("policy bound fits u64")
        {
            let policy_id = zk_ace_policy_id(index);
            let mut policy = template.clone();
            policy.policy_id = policy_id;
            policy.record_digest = PrivacyZkAcePolicyRecordDigestV1::new([0; 32]);
            policy.record_digest = policy
                .compute_record_digest()
                .expect("canonical policy digest material");
            commitments.insert(
                PrivacyCommitmentKeyV1::zk_ace_policy(policy_id).expect("policy key"),
                PrivacyStateItemRecordV1::zk_ace_policy_governance(policy, 7)
                    .expect("policy provenance"),
            );
        }

        let error = privacy_zk_ace_policy_count_v1(&commitments.view())
            .expect_err("over-cap restored policy state must reject");
        assert!(
            error.contains("policy count exceeds"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn zk_ace_provenance_rejects_zero_fields_and_cross_role_reuse() {
        let policy_id = zk_ace_policy_id(1);
        let policy = zk_ace_policy_record(policy_id);
        assert!(PrivacyStateItemRecordV1::zk_ace_policy_governance(policy.clone(), 0).is_err());
        let mut corrupted_policy = policy.clone();
        corrupted_policy.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(nonzero(31));
        assert!(PrivacyStateItemRecordV1::zk_ace_policy_governance(corrupted_policy, 7).is_err());

        let record_digest = policy.record_digest;
        let statement_digest = PrivacyStatementDigestV1::new(nonzero(32));
        let valid = PrivacyStateItemRecordV1::zk_ace_verified_authorization(
            policy_id,
            record_digest,
            statement_digest,
            7,
            0,
        )
        .expect("canonical replay provenance");
        valid
            .validate()
            .expect("canonical replay provenance validates");

        assert!(
            PrivacyStateItemRecordV1::zk_ace_verified_authorization(
                PrivacyPolicyIdV1::new([0; 32]),
                record_digest,
                statement_digest,
                7,
                0,
            )
            .is_err()
        );
        assert!(
            PrivacyStateItemRecordV1::zk_ace_verified_authorization(
                policy_id,
                PrivacyZkAcePolicyRecordDigestV1::new([0; 32]),
                statement_digest,
                7,
                0,
            )
            .is_err()
        );
        assert!(
            PrivacyStateItemRecordV1::zk_ace_verified_authorization(
                policy_id,
                record_digest,
                PrivacyStatementDigestV1::new([0; 32]),
                7,
                0,
            )
            .is_err()
        );
        assert!(
            PrivacyStateItemRecordV1::zk_ace_verified_authorization(
                policy_id,
                record_digest,
                statement_digest,
                0,
                0,
            )
            .is_err()
        );

        let registered = BTreeSet::from([policy_id]);
        validate_privacy_zk_ace_replay_binding_v1(&registered, policy_id, &valid)
            .expect("matching replay provenance");
        let wrong_policy = PrivacyStateItemRecordV1::ZkAceVerifiedAuthorization {
            policy_id: zk_ace_policy_id(2),
            policy_record_digest: record_digest,
            statement_digest,
            admitted_at_height: 7,
            action_index: 0,
        };
        assert!(
            validate_privacy_zk_ace_replay_binding_v1(&registered, policy_id, &wrong_policy)
                .expect_err("cross-policy provenance must reject")
                .contains("wrong-role")
        );
        let wrong_role = PrivacyStateItemRecordV1::zk_ace_policy_governance(policy, 7)
            .expect("valid governance record");
        assert!(
            validate_privacy_zk_ace_replay_binding_v1(&registered, policy_id, &wrong_role)
                .expect_err("governance record cannot serve as replay provenance")
                .contains("wrong-role")
        );
        assert!(
            validate_privacy_zk_ace_replay_binding_v1(&BTreeSet::new(), policy_id, &valid,)
                .expect_err("orphan replay marker must reject")
                .contains("missing policy")
        );
        assert_eq!(valid.zk_ace_policy(), None);
    }

    #[test]
    fn identical_key_images_in_distinct_registries_have_distinct_keys() {
        let namespace_a = zk_ams_namespace(20);
        let namespace_b = zk_ams_namespace(21);
        let key_image = PrivacyZkAmsKeyImageV1::new(nonzero(22));
        let key_a =
            PrivacyNullifierKeyV1::zk_ams_key_image(namespace_a, key_image).expect("valid key");
        let key_b =
            PrivacyNullifierKeyV1::zk_ams_key_image(namespace_b, key_image).expect("valid key");

        assert_ne!(key_a, key_b);
        assert!(key_a < key_b);
    }

    #[test]
    fn root_order_is_epoch_then_root_within_one_namespace() {
        let namespace = pgc_namespace(20);
        let earlier = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::PgcAccountState,
            7,
            PrivacyRootV1::new(nonzero(30)),
        )
        .expect("valid root");
        let later = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::PgcAccountState,
            8,
            PrivacyRootV1::new(nonzero(1)),
        )
        .expect("valid root");
        assert!(earlier < later);
    }

    #[test]
    fn mismatched_protocol_scope_and_root_role_fail_closed() {
        let pgc_namespace = pgc_namespace(20);
        let invalid_namespace = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            pgc_namespace.scope(),
        );
        assert!(invalid_namespace.validate().is_err());
        assert!(
            PrivacyRootKeyV1::new(
                pgc_namespace,
                PrivacyRootRoleV1::ProgramState,
                1,
                PrivacyRootV1::new(nonzero(1)),
            )
            .is_err()
        );

        let vega_namespace = vega_namespace();
        assert!(vega_namespace.validate().is_ok());
        for role in [
            PrivacyRootRoleV1::PgcAccountState,
            PrivacyRootRoleV1::AccountRegistry,
            PrivacyRootRoleV1::Revocation,
            PrivacyRootRoleV1::CertificateAuthorityMembership,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            PrivacyRootRoleV1::OutputSet,
            PrivacyRootRoleV1::ProgramState,
        ] {
            let role = match role {
                PrivacyRootRoleV1::PgcAccountState
                | PrivacyRootRoleV1::AccountRegistry
                | PrivacyRootRoleV1::Revocation
                | PrivacyRootRoleV1::CertificateAuthorityMembership
                | PrivacyRootRoleV1::NoteCommitmentAnchor
                | PrivacyRootRoleV1::OutputSet
                | PrivacyRootRoleV1::ProgramState => role,
            };
            assert!(
                PrivacyRootKeyV1::new(vega_namespace, role, 1, PrivacyRootV1::new(nonzero(2)),)
                    .is_err(),
                "Vega Figure 9 has no canonical root role, but {role:?} was accepted"
            );
            assert!(
                PrivacyRootHeadKeyV1::new(vega_namespace, role).is_err(),
                "Vega Figure 9 has no canonical root-head role, but {role:?} was accepted"
            );
        }
    }

    #[test]
    fn storage_key_json_roundtrip_and_malformed_inputs_reject() {
        let namespace = zk_ams_namespace(20);
        let key = PrivacyNullifierKeyV1::zk_ams_key_image(
            namespace,
            PrivacyZkAmsKeyImageV1::new(nonzero(0xAB)),
        )
        .expect("valid key");
        let mut json_string = String::new();
        key.encode_json_key(&mut json_string);
        let encoded = norito::json::from_json::<String>(&json_string).expect("JSON string");
        assert_eq!(
            PrivacyNullifierKeyV1::decode_json_key(&encoded).expect("decode key"),
            key
        );
        assert!(PrivacyNullifierKeyV1::decode_json_key("not-hex").is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key("").is_err());
        assert!(
            PrivacyNullifierKeyV1::decode_json_key(&encoded.to_ascii_lowercase()).is_err(),
            "lowercase aliases must not decode to the same consensus key"
        );
        let mut mixed_case = encoded.clone();
        let letter = mixed_case
            .find(|character: char| {
                character.is_ascii_hexdigit() && character.is_ascii_alphabetic()
            })
            .expect("fixture encoding contains hexadecimal letters");
        let lowercase_letter = mixed_case[letter..=letter].to_ascii_lowercase();
        mixed_case.replace_range(letter..=letter, &lowercase_letter);
        assert!(PrivacyNullifierKeyV1::decode_json_key(&mixed_case).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&format!(" {encoded}")).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&format!("{encoded} ")).is_err());
        assert!(PrivacyNullifierKeyV1::decode_json_key(&encoded[..encoded.len() - 1]).is_err());

        let mut trailing_bytes = hex::decode(&encoded).expect("canonical hex");
        trailing_bytes.push(0);
        let trailing = hex::encode_upper(trailing_bytes);
        assert!(
            PrivacyNullifierKeyV1::decode_json_key(&trailing).is_err(),
            "trailing Norito bytes must not alias a canonical storage key"
        );
    }

    #[test]
    fn zk_x509_governance_lineages_are_append_only_terminal_and_role_typed() {
        let trust_anchor_id = PrivacyIssuerIdV1::new(nonzero(41));
        let policy_id = PrivacyPolicyIdV1::new(nonzero(42));
        let mut commitments = Storage::new();
        let anchor_origin = x509_trust_anchor_record(
            trust_anchor_id,
            1,
            51,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let policy_origin = x509_certificate_policy_record(
            trust_anchor_id,
            policy_id,
            1,
            52,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut commitments, anchor_origin, 10);
        insert_x509_certificate_policy(&mut commitments, policy_origin.clone(), 11);
        assert_eq!(
            privacy_zk_x509_governance_record_counts_v1(&commitments.view())
                .expect("valid origins"),
            (1, 1)
        );

        let anchor_rotation = x509_trust_anchor_record(
            trust_anchor_id,
            2,
            53,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let policy_rotation = x509_certificate_policy_record(
            trust_anchor_id,
            policy_id,
            2,
            54,
            vec![0, 2, 3],
            Some(policy_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut commitments, anchor_rotation, 12);
        insert_x509_certificate_policy(&mut commitments, policy_rotation.clone(), 13);
        assert_eq!(
            load_privacy_zk_x509_trust_anchor_v1(trust_anchor_id, &commitments.view())
                .expect("current trust anchor"),
            anchor_rotation
        );
        assert_eq!(
            load_privacy_zk_x509_certificate_policy_v1(
                trust_anchor_id,
                policy_id,
                &commitments.view()
            )
            .expect("current certificate policy"),
            policy_rotation
        );

        let anchor_revoked = x509_trust_anchor_record(
            trust_anchor_id,
            3,
            53,
            Some(anchor_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        let policy_revoked = x509_certificate_policy_record(
            trust_anchor_id,
            policy_id,
            3,
            54,
            vec![0, 2, 3],
            Some(policy_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        insert_x509_trust_anchor(&mut commitments, anchor_revoked, 14);
        insert_x509_certificate_policy(&mut commitments, policy_revoked.clone(), 15);
        privacy_zk_x509_governance_record_counts_v1(&commitments.view())
            .expect("canonical terminal revisions");

        let after_terminal = x509_trust_anchor_record(
            trust_anchor_id,
            4,
            55,
            Some(anchor_revoked.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut commitments, after_terminal, 16);
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&commitments.view())
                .expect_err("terminal lineage cannot advance")
                .contains("not active")
        );

        let mut gap = Storage::new();
        insert_x509_trust_anchor(&mut gap, anchor_origin, 10);
        let skipped = x509_trust_anchor_record(
            trust_anchor_id,
            3,
            56,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut gap, skipped, 11);
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&gap.view())
                .expect_err("skipped epoch must reject")
                .contains("must be 2")
        );

        let mut wrong_role = Storage::new();
        let anchor_key = PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision(trust_anchor_id, 1)
            .expect("anchor key");
        wrong_role.insert(
            anchor_key,
            PrivacyStateItemRecordV1::zk_x509_certificate_policy_governance(policy_origin, 10)
                .expect("policy state record"),
        );
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&wrong_role.view())
                .expect_err("cross-role record must reject")
                .contains("wrong-role")
        );
    }

    #[test]
    fn zk_x509_revision_caps_accept_cap_and_reject_cap_plus_one() {
        let trust_anchor_id = PrivacyIssuerIdV1::new(nonzero(61));
        let mut lineage = Storage::new();
        let mut current = x509_trust_anchor_record(
            trust_anchor_id,
            1,
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut lineage, current, 1);
        for epoch in 2..=u64::try_from(ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1)
            .expect("lineage cap fits u64")
        {
            let next = x509_trust_anchor_record(
                trust_anchor_id,
                epoch,
                u8::try_from(epoch).expect("lineage test epoch fits u8"),
                Some(current.record_digest),
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            insert_x509_trust_anchor(&mut lineage, next, epoch);
            current = next;
        }
        assert_eq!(
            privacy_zk_x509_governance_record_counts_v1(&lineage.view())
                .expect("exact lineage cap"),
            (ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1, 0)
        );
        let over_cap_epoch =
            u64::try_from(ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1).expect("cap fits u64") + 1;
        let over_cap = x509_trust_anchor_record(
            trust_anchor_id,
            over_cap_epoch,
            u8::try_from(over_cap_epoch).expect("cap+1 fits u8"),
            Some(current.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut lineage, over_cap, over_cap_epoch);
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&lineage.view())
                .expect_err("lineage cap+1 must reject")
                .contains("exceeds 64 revisions")
        );

        let mut anchors = Storage::new();
        for index in 1..=ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1 {
            let index = u64::try_from(index).expect("global anchor index fits u64");
            let id = PrivacyIssuerIdV1::new(indexed_nonzero(0xA1, index));
            let record = PrivacyZkX509TrustAnchorRecordV1::new(
                id,
                1,
                PrivacyX509TrustStoreDigestV1::new(nonzero(71)),
                PrivacyRootV1::new(nonzero(72)),
                1,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            )
            .expect("global-cap anchor");
            insert_x509_trust_anchor(&mut anchors, record, 1);
        }
        assert_eq!(
            privacy_zk_x509_governance_record_counts_v1(&anchors.view())
                .expect("exact global anchor cap"),
            (ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1, 0)
        );
        let over_index =
            u64::try_from(ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1).expect("cap fits u64") + 1;
        let over_id = PrivacyIssuerIdV1::new(indexed_nonzero(0xA1, over_index));
        let over_record = PrivacyZkX509TrustAnchorRecordV1::new(
            over_id,
            1,
            PrivacyX509TrustStoreDigestV1::new(nonzero(71)),
            PrivacyRootV1::new(nonzero(72)),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("cap+1 anchor");
        insert_x509_trust_anchor(&mut anchors, over_record, 1);
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&anchors.view())
                .expect_err("global anchor cap+1 must reject")
                .contains("exceeds 4096")
        );

        let policy_anchor_id = PrivacyIssuerIdV1::new(nonzero(72));
        let mut policies = Storage::new();
        insert_x509_trust_anchor(
            &mut policies,
            x509_trust_anchor_record(
                policy_anchor_id,
                1,
                73,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            1,
        );
        for index in 1..=ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1 {
            let index = u64::try_from(index).expect("global policy index fits u64");
            let policy_id = PrivacyPolicyIdV1::new(indexed_nonzero(0xB1, index));
            let record = x509_certificate_policy_record(
                policy_anchor_id,
                policy_id,
                1,
                74,
                vec![0, 3],
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            insert_x509_certificate_policy(&mut policies, record, 1);
        }
        assert_eq!(
            privacy_zk_x509_governance_record_counts_v1(&policies.view())
                .expect("exact global policy cap"),
            (1, ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1)
        );
        let over_index =
            u64::try_from(ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1).expect("cap fits u64") + 1;
        let over_policy = x509_certificate_policy_record(
            policy_anchor_id,
            PrivacyPolicyIdV1::new(indexed_nonzero(0xB1, over_index)),
            1,
            74,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_certificate_policy(&mut policies, over_policy, 1);
        assert!(
            privacy_zk_x509_governance_record_counts_v1(&policies.view())
                .expect_err("global policy cap+1 must reject")
                .contains("exceeds 4096")
        );
    }

    #[test]
    fn zk_x509_authoritative_roots_bind_exact_revisions_without_activation() {
        let trust_anchor_id = PrivacyIssuerIdV1::new(nonzero(41));
        let policy_id = PrivacyPolicyIdV1::new(nonzero(42));
        let namespace = x509_namespace();
        let anchor_origin = x509_trust_anchor_record(
            trust_anchor_id,
            1,
            81,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let policy_origin = x509_certificate_policy_record(
            trust_anchor_id,
            policy_id,
            1,
            82,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let crl_origin = x509_crl_record(
            trust_anchor_id,
            policy_id,
            1,
            1,
            84,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let mut commitments = Storage::new();
        insert_x509_trust_anchor(&mut commitments, anchor_origin, 10);
        insert_x509_certificate_policy(&mut commitments, policy_origin.clone(), 11);
        insert_x509_crl(&mut commitments, crl_origin, 12);
        let mut roots = Storage::new();
        let mut root_heads = Storage::new();
        let ca_key = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 1, 82);
        let ca_provenance = x509_root_provenance(ca_key, anchor_origin, 12);
        roots.insert(ca_key, ca_provenance);
        root_heads.insert(
            PrivacyRootHeadKeyV1::new(
                ca_key.namespace(),
                PrivacyRootRoleV1::CertificateAuthorityMembership,
            )
            .expect("root-head key"),
            PrivacyRootHeadRecordV1::new(1, ca_key.root(), ca_provenance, None).expect("root head"),
        );
        let snapshot = load_privacy_zk_x509_authoritative_state_v1(
            trust_anchor_id,
            policy_id,
            8,
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect("complete authoritative snapshot");
        assert_eq!(snapshot.namespace(), namespace);
        assert_eq!(snapshot.trust_anchor(), anchor_origin);
        assert_eq!(snapshot.certificate_policy(), &policy_origin);

        let statement = IrohaZkX509StarkP256StatementV1 {
            context: PrivacyStatementContextV1 {
                chain_id: ChainId::from("x509-authoritative-state-test"),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(nonzero(91)),
                parameter_id: PrivacyParameterIdV1::new(nonzero(92)),
                parameter_digest: PrivacyParameterDigestV1::new(nonzero(93)),
                verifier_digest: PrivacyVerifierDigestV1::new(nonzero(94)),
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new(nonzero(95)),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new(nonzero(96)),
            },
            trust_anchor_id,
            certificate_policy_id: policy_id,
            trust_anchor_record_digest: anchor_origin.record_digest,
            trust_anchor_record_epoch: anchor_origin.record_epoch,
            certificate_policy_record_digest: policy_origin.record_digest,
            certificate_policy_record_epoch: policy_origin.record_epoch,
            crl_record_digest: crl_origin.record_digest,
            crl_record_epoch: crl_origin.record_epoch,
            subject_public_key_digest: PrivacyCertificateKeyDigestV1::new(nonzero(97)),
            ca_membership_root: snapshot.ca_membership_root(),
            ca_membership_root_epoch: snapshot.ca_membership_root_epoch(),
            key_usage: policy_origin.required_key_usage,
            extended_key_usages: policy_origin.required_extended_key_usages.clone(),
            disclosed_attributes: policy_origin
                .required_disclosed_attribute_indices
                .iter()
                .map(|index| PrivacyZkX509DisclosedAttributeV1 {
                    index: *index,
                    attribute_digest: PrivacyAttributeDigestV1::new(indexed_nonzero(
                        0xD1,
                        u64::from(*index) + 1,
                    )),
                })
                .collect(),
            presentation_not_before_unix_seconds: 1_750_000_000,
            presentation_not_after_unix_seconds: 1_750_000_200,
            wallet_account: account(33),
            wallet_challenge: PrivacyChallengeV1::new(nonzero(98)),
            certificate_nullifier: PrivacyNullifierV1::new(nonzero(99)),
        };
        let limits = PrivacyConsensusLimitsV1::taira_default();
        validate_privacy_zk_x509_statement_state_v1(
            &statement,
            &snapshot,
            1_750_000_100_000,
            &limits,
        )
        .expect("exact authoritative statement");
        for trusted_block_timestamp_ms in [1_750_000_000_000, 1_750_000_200_999] {
            validate_privacy_zk_x509_statement_state_v1(
                &statement,
                &snapshot,
                trusted_block_timestamp_ms,
                &limits,
            )
            .expect("both inclusive presentation-window boundaries are admitted");
        }
        for trusted_block_timestamp_ms in [1_749_999_999_999, 1_750_000_201_000] {
            assert!(
                validate_privacy_zk_x509_statement_state_v1(
                    &statement,
                    &snapshot,
                    trusted_block_timestamp_ms,
                    &limits,
                )
                .expect_err("a block outside the presentation window must reject")
                .contains("block timestamp")
            );
        }

        let assert_statement_rejected =
            |label: &str, mutate: fn(&mut IrohaZkX509StarkP256StatementV1)| {
                let mut candidate = statement.clone();
                mutate(&mut candidate);
                assert!(
                    validate_privacy_zk_x509_statement_state_v1(
                        &candidate,
                        &snapshot,
                        1_750_000_100_000,
                        &limits,
                    )
                    .is_err(),
                    "{label} must fail closed"
                );
            };
        assert_statement_rejected("substituted trust-anchor id", |candidate| {
            candidate.trust_anchor_id = PrivacyIssuerIdV1::new(nonzero(100));
        });
        assert_statement_rejected("stale trust-anchor digest", |candidate| {
            candidate.trust_anchor_record_digest =
                PrivacyZkX509TrustAnchorRecordDigestV1::new(nonzero(101));
        });
        assert_statement_rejected("stale certificate-policy epoch", |candidate| {
            candidate.certificate_policy_record_epoch += 1;
        });
        assert_statement_rejected("substituted CA root", |candidate| {
            candidate.ca_membership_root = PrivacyRootV1::new(nonzero(102));
        });
        assert_statement_rejected("weakened key usage", |candidate| {
            candidate.key_usage.digital_signature = false.into();
        });
        assert_statement_rejected("substituted EKU policy", |candidate| {
            candidate.extended_key_usages = vec![PrivacyX509ExtendedKeyUsageV1::DocumentSigning];
        });
        assert_statement_rejected("omitted required disclosure", |candidate| {
            candidate.disclosed_attributes.pop();
        });
        assert_statement_rejected("extra disclosure", |candidate| {
            candidate.disclosed_attributes.insert(
                1,
                PrivacyZkX509DisclosedAttributeV1 {
                    index: 1,
                    attribute_digest: PrivacyAttributeDigestV1::new(nonzero(103)),
                },
            );
        });
        assert_statement_rejected("presentation starts before CRL thisUpdate", |candidate| {
            candidate.presentation_not_before_unix_seconds = 1_749_999_900;
            candidate.presentation_not_after_unix_seconds = 1_750_000_200;
        });
        assert_statement_rejected("presentation exceeds maximum CRL age", |candidate| {
            candidate.presentation_not_before_unix_seconds = 1_749_999_902;
            candidate.presentation_not_after_unix_seconds = 1_750_000_202;
        });
        let mut exact_crl_age_boundary = statement.clone();
        exact_crl_age_boundary.presentation_not_before_unix_seconds = 1_749_999_902;
        exact_crl_age_boundary.presentation_not_after_unix_seconds = 1_750_000_201;
        validate_privacy_zk_x509_statement_state_v1(
            &exact_crl_age_boundary,
            &snapshot,
            1_750_000_100_000,
            &limits,
        )
        .expect("the exact 300-second CRL freshness boundary is admitted");

        let activations = Storage::new();
        let pgc_accounts = Storage::new();
        let pgc_pool_invariants = Storage::new();
        let nullifiers = Storage::new();
        validate_privacy_persisted_state_v1(
            &PrivacyConsensusPolicyV1::taira_default(),
            &activations.view(),
            &pgc_accounts.view(),
            &pgc_pool_invariants.view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect("pre-activation X.509 governance state is durable");

        let anchor_rotation = x509_trust_anchor_record(
            trust_anchor_id,
            2,
            85,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_trust_anchor(&mut commitments, anchor_rotation, 13);
        assert!(
            validate_privacy_persisted_state_v1(
                &PrivacyConsensusPolicyV1::taira_default(),
                &activations.view(),
                &pgc_accounts.view(),
                &pgc_pool_invariants.view(),
                &nullifiers.view(),
                &commitments.view(),
                &roots.view(),
                &root_heads.view(),
            )
            .expect_err("record/root updates must be atomic")
            .contains("stale")
        );
        let ca_key = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 2, 86);
        let ca_provenance = x509_root_provenance(ca_key, anchor_rotation, 14);
        roots.insert(ca_key, ca_provenance);
        root_heads.insert(
            PrivacyRootHeadKeyV1::new(
                ca_key.namespace(),
                PrivacyRootRoleV1::CertificateAuthorityMembership,
            )
            .expect("CA root-head key"),
            PrivacyRootHeadRecordV1::new(2, ca_key.root(), ca_provenance, None)
                .expect("CA successor head"),
        );
        let crl_rotation = x509_crl_record(
            trust_anchor_id,
            policy_id,
            2,
            2,
            87,
            Some(crl_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        insert_x509_crl(&mut commitments, crl_rotation, 14);
        validate_privacy_persisted_state_v1(
            &PrivacyConsensusPolicyV1::taira_default(),
            &activations.view(),
            &pgc_accounts.view(),
            &pgc_pool_invariants.view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect("complete signed-CRL rotation restores valid state without a secondary root");
        load_privacy_zk_x509_authoritative_state_v1(
            trust_anchor_id,
            policy_id,
            8,
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect("refreshed CA root and signed CRL restore authoritative state");

        let ca_head_key = PrivacyRootHeadKeyV1::new(
            ca_key.namespace(),
            PrivacyRootRoleV1::CertificateAuthorityMembership,
        )
        .expect("CA head key");
        let generic = PrivacyRootProvenanceV1::governance(
            PrivacyRootPublicationV1 {
                namespace: ca_key.namespace(),
                role: ca_key.role(),
                epoch: ca_key.epoch(),
                root: ca_key.root(),
            }
            .digest()
            .expect("publication digest"),
            14,
        )
        .expect("generic governance provenance");
        roots.insert(ca_key, generic);
        root_heads.insert(
            ca_head_key,
            PrivacyRootHeadRecordV1::new(2, ca_key.root(), generic, None).expect("generic head"),
        );
        assert!(
            load_privacy_zk_x509_authoritative_state_v1(
                trust_anchor_id,
                policy_id,
                8,
                &commitments.view(),
                &roots.view(),
                &root_heads.view(),
            )
            .expect_err("generic provenance cannot impersonate X.509 governance")
            .contains("non-X.509 provenance")
        );
    }

    #[test]
    fn compiled_limits_keep_bounded_root_history() {
        assert_eq!(
            PrivacyConsensusLimitsV1::taira_default().retained_root_count,
            2_048
        );
    }

    #[test]
    fn root_history_prunes_independently_per_namespace() {
        let mut roots = Storage::new();
        let record = root_provenance();
        let independent_namespace = pgc_namespace(0xE1);
        for epoch in 1..=2_048 {
            let root_byte = (epoch % 251 + 1) as u8;
            roots.insert(
                x509_root_key(
                    PrivacyRootRoleV1::CertificateAuthorityMembership,
                    epoch,
                    root_byte,
                ),
                record,
            );
            roots.insert(
                PrivacyRootKeyV1::new(
                    independent_namespace,
                    PrivacyRootRoleV1::PgcAccountState,
                    epoch,
                    PrivacyRootV1::new(nonzero(root_byte)),
                )
                .expect("independent root key"),
                record,
            );
        }

        let added = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 2_049, 43);
        let removals = plan_privacy_root_history_update_v1(&roots.view(), &[added], 2_048)
            .expect("valid plan");

        assert_eq!(
            removals,
            vec![x509_root_key(
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                1,
                2
            )]
        );
        assert!(
            roots
                .view()
                .get(
                    &PrivacyRootKeyV1::new(
                        independent_namespace,
                        PrivacyRootRoleV1::PgcAccountState,
                        1,
                        PrivacyRootV1::new(nonzero(2)),
                    )
                    .expect("independent root key")
                )
                .is_some(),
            "planning one namespace must not prune another namespace"
        );
    }

    #[test]
    fn root_history_rejects_replays_epoch_conflicts_and_stale_epochs() {
        let mut roots = Storage::new();
        roots.insert(
            x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 7, 70),
            root_provenance(),
        );

        let exact_replay = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 7, 70);
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &[exact_replay], 8),
            Err(PrivacyRootHistoryErrorV1::ExistingRoot { key }) if key == exact_replay
        ));

        assert!(matches!(
            plan_privacy_root_history_update_v1(
                &roots.view(),
                &[x509_root_key(
                    PrivacyRootRoleV1::CertificateAuthorityMembership,
                    7,
                    71,
                )],
                8,
            ),
            Err(PrivacyRootHistoryErrorV1::EpochConflict { epoch: 7, .. })
        ));

        assert!(matches!(
            plan_privacy_root_history_update_v1(
                &roots.view(),
                &[x509_root_key(
                    PrivacyRootRoleV1::CertificateAuthorityMembership,
                    6,
                    60,
                )],
                8,
            ),
            Err(PrivacyRootHistoryErrorV1::NonMonotonicEpoch {
                latest_epoch: 7,
                added_epoch: 6,
                ..
            })
        ));
    }

    #[test]
    fn root_history_rejects_duplicate_and_over_capacity_effects() {
        let roots = Storage::new();
        let duplicate = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 1, 10);
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &[duplicate, duplicate], 8),
            Err(PrivacyRootHistoryErrorV1::DuplicateAddedRoot { key }) if key == duplicate
        ));

        let additions = [
            x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 1, 10),
            x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 2, 20),
            x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 3, 30),
        ];
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &additions, 2),
            Err(PrivacyRootHistoryErrorV1::AddedRootsExceedRetention { count: 3, max: 2 })
        ));
    }
}
