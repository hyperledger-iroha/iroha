//! Durable first-release privacy admission state.
//!
//! Privacy state is split across independent typed world-state maps. This keeps
//! one admission proportional to its actual effects instead of cloning or
//! conflicting with the complete privacy ledger. Every map still participates
//! in the same [`crate::state::StateTransaction`], so a rejected transaction
//! cannot leave a partial replay marker, commitment, or root behind.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::privacy::{
    ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1, PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
    PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1, PRIVACY_ZK_ACE_MAX_POLICIES_V1,
    PrivacyActivationValidationError,
    PrivacyConsensusPolicyV1, PrivacyNamespaceV1, PrivacyP256CiphertextV1, PrivacyP256PointV1,
    PrivacyPgcAccountBootstrapDigestV1, PrivacyPgcAccountV1, PrivacyPgcBootstrapProofDigestV1,
    PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyRootManagementV1,
    PrivacyRootPublicationDigestV1, PrivacyRootRoleV1, PrivacyRootV1, PrivacyStatementDigestV1,
    PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordV1,
    PrivacyZkAmsIssuerPolicyRecordDigestV1, PrivacyZkAmsKeyImageV1, PrivacyZkAmsPhcHashV1,
    PrivacyZkAmsRegistryBootstrapDigestV1, PrivacyZkAmsSeedPublicKeyV1, PrivacyNullifierV1,
    PrivacyPolicyIdV1,
    ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1,
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use thiserror::Error;

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

/// Validate that every restored protocol-limit schedule is still future.
///
/// A transition effective at `E` is valid in a snapshot committed at `E - 1`
/// and invalid once committed height `E` has already been reached.
pub(crate) fn validate_privacy_activation_schedules_at_committed_height_v1(
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

/// Deterministically derive one complete PGC encrypted account-state root.
///
/// Accounts must be a complete table in strict public-key order. The closed
/// first-release cardinalities ensure this operation is bounded by 64 entries.
pub(crate) fn compute_privacy_pgc_account_state_root_v1(
    namespace: PrivacyNamespaceV1,
    epoch: u64,
    total_supply: u32,
    accounts: &[PrivacyPgcAccountV1],
) -> Result<PrivacyRootV1, &'static str> {
    namespace
        .validate()
        .map_err(|_| "privacy PGC account-root namespace is invalid")?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
        return Err("privacy PGC account-root namespace has the wrong protocol");
    }
    if epoch == 0 {
        return Err("privacy PGC account-root epoch must be non-zero");
    }
    if total_supply == 0 {
        return Err("privacy PGC account-root total supply must be non-zero");
    }
    let count = u32::try_from(accounts.len())
        .map_err(|_| "privacy PGC account-root count cannot be represented")?;
    if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&count) {
        return Err("privacy PGC account-root count is not a closed first-release size");
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.public_key.is_zero()
            || account.encrypted_balance.left.is_zero()
            || account.encrypted_balance.right.is_zero()
        {
            return Err("privacy PGC account-root contains a zero point");
        }
        if index > 0 && accounts[index - 1].public_key >= account.public_key {
            return Err("privacy PGC account-root keys are not strictly increasing");
        }
        for point in [
            account.public_key,
            account.encrypted_balance.left,
            account.encrypted_balance.right,
        ] {
            crate::privacy_engines::p256::CompressedPointV1::from_slice(point.as_bytes()).map_err(
                |_| "privacy PGC account-root contains an invalid canonical P-256 point",
            )?;
        }
    }

    let namespace_bytes =
        norito::to_bytes(&namespace).map_err(|_| "privacy PGC namespace encoding failed")?;
    let namespace_len = u64::try_from(namespace_bytes.len())
        .map_err(|_| "privacy PGC namespace encoding length overflow")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1);
    hasher.update(&namespace_len.to_le_bytes());
    hasher.update(&namespace_bytes);
    hasher.update(&epoch.to_le_bytes());
    hasher.update(&total_supply.to_le_bytes());
    hasher.update(&count.to_le_bytes());
    for account in accounts {
        hasher.update(account.public_key.as_bytes());
        hasher.update(account.encrypted_balance.left.as_bytes());
        hasher.update(account.encrypted_balance.right.as_bytes());
    }
    let root = PrivacyRootV1::new(*hasher.finalize().as_bytes());
    if root.is_zero() {
        return Err("privacy PGC account-root digest is zero");
    }
    Ok(root)
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
        | PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
        | PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. }
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
    for (candidate, record) in commitments.iter() {
        if let PrivacyCommitmentKeyV1::ZkAcePolicy {
            policy_id: candidate_policy_id,
        } = candidate
        {
            policy_count = policy_count
                .checked_add(1)
                .ok_or_else(|| "ZK-ACE policy count overflow".to_owned())?;
            if policy_count > PRIVACY_ZK_ACE_MAX_POLICIES_V1 {
                return Err(format!(
                    "ZK-ACE policy count exceeds {}",
                    PRIVACY_ZK_ACE_MAX_POLICIES_V1
                ));
            }
            let PrivacyStateItemRecordV1::ZkAcePolicyGovernance { policy, .. } = record else {
                return Err(format!(
                    "ZK-ACE policy {candidate_policy_id:?} has wrong-role provenance"
                ));
            };
            policy.validate().map_err(|error| {
                format!("ZK-ACE policy {candidate_policy_id:?} is invalid: {error}")
            })?;
            if policy.policy_id != *candidate_policy_id {
                return Err(format!(
                    "ZK-ACE policy key {candidate_policy_id:?} does not match its record"
                ));
            }
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
        ensure_protocol_activation(key.protocol_id())?;
    }

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
        ensure_activation(key.namespace())?;
        history_by_scope
            .entry((key.namespace(), key.role()))
            .or_default()
            .push((*key, *provenance));
    }

    let mut zk_ams_bootstraps =
        BTreeMap::<PrivacyNamespaceV1, PrivacyZkAmsRegistryBootstrapDigestV1>::new();
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
        ensure_activation(key.namespace())?;
        if !history_by_scope.contains_key(&(key.namespace(), key.role())) {
            return Err(format!(
                "privacy root head for {:?}/{:?} has no retained history",
                key.namespace(),
                key.role()
            ));
        }
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
            PrivacyCommitmentKeyV1::ZkAmsIssuerPolicyRecord { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsPhc { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsSeedKey { namespace, .. } => {
                let bootstrap_digest = zk_ams_bootstraps.get(namespace).ok_or_else(|| {
                    format!(
                        "ZK-AMS commitment {namespace:?} has no authoritative AccountRegistry"
                    )
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
                    PrivacyCommitmentKeyV1::ZkAcePolicy { .. } => false,
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
                if !zk_ace_policy_ids.contains(policy_id) {
                    return Err(format!(
                        "ZK-ACE replay marker references missing policy {policy_id:?}"
                    ));
                }
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::ZkAceVerifiedAuthorization {
                        policy_id: observed,
                        ..
                    } if observed == policy_id
                ) {
                    return Err(format!(
                        "ZK-ACE replay marker for {policy_id:?} has wrong-role provenance"
                    ));
                }
            }
            PrivacyNullifierKeyV1::ZkAmsKeyImage { namespace, .. } => {
                let bootstrap_digest = zk_ams_bootstraps.get(namespace).ok_or_else(|| {
                    format!(
                        "ZK-AMS nullifier {namespace:?} has no authoritative AccountRegistry"
                    )
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

    /// Return the exact ZK-AMS namespace, if this is a key-image marker.
    #[must_use]
    pub const fn zk_ams_namespace(self) -> Option<PrivacyNamespaceV1> {
        match self {
            Self::ZkAceReplay { .. } => None,
            Self::ZkAmsKeyImage { namespace, .. } => Some(namespace),
        }
    }

    /// Return the typed ZK-AMS key image, if present.
    #[must_use]
    pub const fn zk_ams_image(self) -> Option<PrivacyZkAmsKeyImageV1> {
        match self {
            Self::ZkAceReplay { .. } => None,
            Self::ZkAmsKeyImage { key_image, .. } => Some(key_image),
        }
    }

    /// Return the protocol whose closed replay-key role is encoded.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAceReplay { .. } => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::ZkAmsKeyImage { .. } => PrivacyProtocolIdV1::IrohaZkAmsV1,
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
            Self::ZkAcePolicy { .. } => None,
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
            Self::ZkAcePolicy { .. } => None,
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
        if !role.is_compatible_with(namespace.protocol_id()) {
            return Err("privacy root role is incompatible with its namespace protocol");
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
        if !role.is_compatible_with(namespace.protocol_id()) {
            return Err("privacy root-head role is incompatible with its namespace protocol");
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
#[norito(tag = "origin", content = "record")]
pub(crate) enum PrivacyRootProvenanceV1 {
    /// Root published by an authorized governance instruction.
    Governance {
        /// Digest of the exact canonical root-publication payload.
        publication_digest: PrivacyRootPublicationDigestV1,
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
            } => Some((parent_epoch, parent_root)),
            Self::Governance { .. }
            | Self::ZkAmsRegistryBootstrap { .. }
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
#[norito(tag = "origin", content = "record")]
pub enum PrivacyStateItemRecordV1 {
    /// Complete authoritative ZK-ACE policy installed or replaced by governance.
    ZkAcePolicyGovernance {
        /// Canonical self-digested policy state.
        policy: PrivacyZkAcePolicyRecordV1,
        /// Block height at which governance installed this revision.
        admitted_at_height: u64,
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
    pub const fn zk_ams_bootstrap_digest(
        &self,
    ) -> Option<PrivacyZkAmsRegistryBootstrapDigestV1> {
        match self {
            Self::ZkAcePolicyGovernance { .. }
            | Self::ZkAceVerifiedAuthorization { .. } => None,
            Self::ZkAmsGovernance {
                bootstrap_digest, ..
            }
            | Self::ZkAmsVerifiedProof {
                bootstrap_digest, ..
            } => Some(*bootstrap_digest),
        }
    }

    /// Borrow the authoritative ZK-ACE policy carried by this record.
    #[must_use]
    pub const fn zk_ace_policy(&self) -> Option<&PrivacyZkAcePolicyRecordV1> {
        match self {
            Self::ZkAcePolicyGovernance { policy, .. } => Some(policy),
            Self::ZkAceVerifiedAuthorization { .. }
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

/// Validate that every non-PGC history already satisfies a future retention cap.
///
/// Non-PGC histories have no typed parent anchor in the first release and
/// therefore cannot be pruned implicitly. Governance must not schedule a
/// chain-wide retention tightening which would orphan one of those histories.
pub(crate) fn validate_non_pgc_privacy_root_retention_v1(
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
        if key.namespace().protocol_id() == PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
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
                "non-PGC privacy root history for {namespace:?}/{role:?} has {count} roots, exceeding scheduled retention {retained}"
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
    use iroha_data_model::privacy::{
        PrivacyActiveLifecycleV1, PrivacyConsensusLimitsV1, PrivacyIssuerIdV1,
        PrivacyIssuerRegistryPolicyNamespaceV1, PrivacyNamespaceScopeV1, PrivacyParameterIdV1,
        PrivacyParameterNamespaceV1, PrivacyPolicyIdV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1,
        PrivacyProposedLifecycleV1, PrivacyProtocolLifecycleV1, PrivacyRootV1,
        PrivacyTrustAnchorPolicyNamespaceV1, PrivacyVerifierDigestV1, PrivacyZkAmsKeyImageV1,
        PrivacyZkAmsRegistryIdV1,
    };
    use mv::{json::JsonKeyCodec, storage::Storage};
    use p256::{ProjectivePoint, Scalar, elliptic_curve::Group};

    use super::*;

    fn nonzero(byte: u8) -> [u8; 32] {
        [byte; 32]
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

    fn x509_namespace() -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
                trust_anchor_id: PrivacyIssuerIdV1::new(nonzero(41)),
                policy_id: PrivacyPolicyIdV1::new(nonzero(42)),
            }),
        )
    }

    fn x509_root_key(role: PrivacyRootRoleV1, epoch: u64, root_byte: u8) -> PrivacyRootKeyV1 {
        PrivacyRootKeyV1::new(
            x509_namespace(),
            role,
            epoch,
            PrivacyRootV1::new(nonzero(root_byte)),
        )
        .expect("valid root key")
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
            validate_privacy_activation_schedules_at_committed_height_v1(&activations.view(), 999)
                .expect_err("a snapshot cannot contain a future-admitted schedule")
                .contains("scheduled-at")
        );
        validate_privacy_activation_schedules_at_committed_height_v1(&activations.view(), 1_299)
            .expect("effective E is valid in committed E-1");
        assert!(
            validate_privacy_activation_schedules_at_committed_height_v1(
                &activations.view(),
                1_300
            )
            .expect_err("effective E cannot remain pending in committed E")
            .contains("not after committed height")
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
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 0, 160, &accounts).is_err(),
            "epoch zero is not a persistable root"
        );
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 0, &accounts).is_err(),
            "zero supply is not a persistable root"
        );
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 160, &accounts[..15]).is_err(),
            "cardinality outside the closed 16/32/64 set must reject"
        );

        let mut unordered = accounts.clone();
        unordered.swap(4, 5);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 160, &unordered).is_err(),
            "account keys must be in one canonical strict order"
        );

        let mut duplicate = accounts.clone();
        duplicate[5].public_key = duplicate[4].public_key;
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 160, &duplicate).is_err(),
            "duplicate accounts must reject"
        );

        let mut zero_component = accounts;
        zero_component[3].encrypted_balance.right = PrivacyP256PointV1::new([0; 33]);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 160, &zero_component).is_err(),
            "zero encoded points must reject before hashing"
        );

        let mut off_curve = pgc_accounts(16);
        let mut invalid = [u8::MAX; 33];
        invalid[0] = 2;
        off_curve[3].encrypted_balance.right = PrivacyP256PointV1::new(invalid);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, 160, &off_curve).is_err(),
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
    fn future_non_pgc_retention_is_prevalidated_while_pgc_is_prunable() {
        let orchard_namespace = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(nonzero(0xA7)),
            }),
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
                    orchard_namespace,
                    PrivacyRootRoleV1::NoteCommitmentAnchor,
                    epoch,
                    PrivacyRootV1::new([u8::try_from(epoch).expect("small epoch"); 32]),
                )
                .expect("Orchard root key"),
                provenance,
            );
        }
        validate_non_pgc_privacy_root_retention_v1(&non_pgc_roots.view(), 2)
            .expect("inclusive future cap");
        assert!(
            validate_non_pgc_privacy_root_retention_v1(&non_pgc_roots.view(), 1)
                .expect_err("non-PGC histories cannot be implicitly pruned")
                .contains("exceeding scheduled retention 1")
        );
        assert!(validate_non_pgc_privacy_root_retention_v1(&non_pgc_roots.view(), 0).is_err());

        let pgc_namespace = pgc_namespace(0xB7);
        let mut pgc_roots = Storage::new();
        for epoch in 1..=3 {
            pgc_roots.insert(
                PrivacyRootKeyV1::new(
                    pgc_namespace,
                    PrivacyRootRoleV1::PgcAccountState,
                    epoch,
                    PrivacyRootV1::new([u8::try_from(epoch).expect("small epoch"); 32]),
                )
                .expect("PGC root key"),
                provenance,
            );
        }
        validate_non_pgc_privacy_root_retention_v1(&pgc_roots.view(), 1)
            .expect("PGC histories use the typed due-height pruning planner");
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
            PrivacyRootRoleV1::CertificateRevocationNonmembership,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            PrivacyRootRoleV1::OutputSet,
            PrivacyRootRoleV1::ProgramState,
        ] {
            let role = match role {
                PrivacyRootRoleV1::PgcAccountState
                | PrivacyRootRoleV1::AccountRegistry
                | PrivacyRootRoleV1::Revocation
                | PrivacyRootRoleV1::CertificateAuthorityMembership
                | PrivacyRootRoleV1::CertificateRevocationNonmembership
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
    fn compiled_limits_keep_bounded_root_history() {
        assert_eq!(
            PrivacyConsensusLimitsV1::taira_default().retained_root_count,
            2_048
        );
    }

    #[test]
    fn root_history_prunes_independently_per_namespace_and_role() {
        let mut roots = Storage::new();
        let record = root_provenance();
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
                x509_root_key(
                    PrivacyRootRoleV1::CertificateRevocationNonmembership,
                    epoch,
                    root_byte,
                ),
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
                .get(&x509_root_key(
                    PrivacyRootRoleV1::CertificateRevocationNonmembership,
                    1,
                    2,
                ))
                .is_some(),
            "planning one role must not prune another role"
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

    #[test]
    fn identical_epoch_and_root_are_allowed_in_distinct_roles() {
        let roots = Storage::new();
        let ca_membership = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 9, 90);
        let crl_nonmembership =
            x509_root_key(PrivacyRootRoleV1::CertificateRevocationNonmembership, 9, 90);

        assert_eq!(
            plan_privacy_root_history_update_v1(
                &roots.view(),
                &[ca_membership, crl_nonmembership],
                8,
            )
            .expect("roles are independent"),
            Vec::<PrivacyRootKeyV1>::new()
        );
    }
}
