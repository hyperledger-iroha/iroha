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
    PrivacyActivationValidationError, PrivacyCommitmentV1, PrivacyConsensusLimitsV1,
    PrivacyNamespaceV1, PrivacyNullifierV1, PrivacyP256CiphertextV1, PrivacyP256PointV1,
    PrivacyPgcAccountBootstrapDigestV1, PrivacyPgcAccountV1, PrivacyProtocolActivationRecordV1,
    PrivacyProtocolIdV1, PrivacyRootPublicationDigestV1, PrivacyRootRoleV1, PrivacyRootV1,
    PrivacyStatementDigestV1,
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use thiserror::Error;

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
    /// A persisted activation carries non-canonical chain-wide limits.
    #[error(transparent)]
    ConsensusLimitsMismatch(Box<PrivacyActivationConsensusLimitsMismatchV1>),
    /// A structurally valid record does not match executable consensus code.
    #[error(transparent)]
    CompiledProfile(Box<PrivacyActivationCompiledProfileMismatchV1>),
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
#[error("persisted privacy activation {protocol_id:?} has non-canonical consensus limits")]
pub(crate) struct PrivacyActivationConsensusLimitsMismatchV1 {
    protocol_id: PrivacyProtocolIdV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("persisted privacy activation {protocol_id:?} is not compiled: {source}")]
pub(crate) struct PrivacyActivationCompiledProfileMismatchV1 {
    protocol_id: PrivacyProtocolIdV1,
    source: crate::privacy_profiles::CompiledPrivacyProfileValidationErrorV1,
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
    current_height: u64,
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
        if record.limits != PrivacyConsensusLimitsV1::taira_default() {
            return Err(PrivacyActivationPromotionErrorV1::ConsensusLimitsMismatch(
                Box::new(PrivacyActivationConsensusLimitsMismatchV1 {
                    protocol_id: record.protocol_id,
                }),
            ));
        }
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

        let lifecycle =
            crate::privacy::effective_privacy_lifecycle_v1(record.lifecycle, current_height);
        if lifecycle != record.lifecycle {
            let mut promoted = *record;
            promoted.lifecycle = lifecycle;
            promoted.validate().map_err(|source| {
                PrivacyActivationPromotionErrorV1::InvalidActivation(Box::new(
                    PrivacyInvalidActivationV1 {
                        protocol_id: promoted.protocol_id,
                        source,
                    },
                ))
            })?;
            promotions.push((*key, promoted));
        }
    }
    Ok(promotions)
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

/// Domain-separated origin of one encrypted PGC account state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(tag = "origin", content = "record")]
pub(crate) enum PrivacyPgcAccountProvenanceV1 {
    /// Initial state admitted by the complete governed pool bootstrap.
    Bootstrap {
        /// Digest of the exact canonical bootstrap payload.
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
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
        admitted_at_height: u64,
    ) -> Result<Self, &'static str> {
        if bootstrap_digest.is_zero() {
            return Err("privacy PGC bootstrap digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy PGC account admission height must be non-zero");
        }
        Ok(Self::Bootstrap {
            bootstrap_digest,
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
                admitted_at_height,
            } => Self::bootstrap(bootstrap_digest, admitted_at_height).map(|_| ()),
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

/// Validate every cross-map invariant in restored first-release privacy state.
///
/// Snapshot decoding invokes this before constructing `World`. Consequently a
/// malformed key, unavailable activation, orphan state item, over-cap history,
/// duplicate root epoch, missing/inconsistent head, or PGC account/root
/// mismatch cannot enter consensus state.
pub(crate) fn validate_privacy_persisted_state_v1(
    activations: &impl StorageReadOnly<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>,
    pgc_accounts: &impl StorageReadOnly<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>,
    nullifiers: &impl StorageReadOnly<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>,
    commitments: &impl StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: &impl StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) -> Result<(), String> {
    plan_due_privacy_activation_promotions_v1(activations, 0)
        .map_err(|error| format!("invalid privacy activation registry: {error}"))?;

    let ensure_activation = |namespace: PrivacyNamespaceV1| -> Result<(), String> {
        let key = PrivacyActivationKeyV1::new(namespace.protocol_id());
        if activations.get(&key).is_none() {
            return Err(format!(
                "privacy state references unregistered protocol {:?}",
                namespace.protocol_id()
            ));
        }
        Ok(())
    };

    for (key, record) in nullifiers.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy nullifier key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid privacy nullifier provenance: {error}"))?;
        ensure_activation(key.namespace())?;
    }
    for (key, record) in commitments.iter() {
        key.validate()
            .map_err(|error| format!("invalid privacy commitment key: {error}"))?;
        record
            .validate()
            .map_err(|error| format!("invalid privacy commitment provenance: {error}"))?;
        ensure_activation(key.namespace())?;
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

    let retained = usize::try_from(PrivacyConsensusLimitsV1::taira_default().retained_root_count)
        .map_err(|_| "privacy retained-root count cannot be represented".to_owned())?;
    for ((namespace, role), history) in &history_by_scope {
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

    let mut pgc_by_namespace = BTreeMap::<PrivacyNamespaceV1, Vec<PrivacyPgcAccountV1>>::new();
    let mut pgc_epoch_by_namespace = BTreeMap::<PrivacyNamespaceV1, u64>::new();
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
        let computed = compute_privacy_pgc_account_state_root_v1(*namespace, epoch, accounts)
            .map_err(|error| format!("invalid privacy PGC account table: {error}"))?;
        if computed != head.root() {
            return Err(format!(
                "privacy PGC account table {namespace:?} does not match its root head"
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
    }
    Ok(())
}

/// Key for one consumed replay-prevention value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct PrivacyNullifierKeyV1 {
    namespace: PrivacyNamespaceV1,
    nullifier: PrivacyNullifierV1,
}

impl PrivacyNullifierKeyV1 {
    /// Construct and validate a scoped nullifier key.
    ///
    /// # Errors
    ///
    /// Rejects an invalid namespace or an all-zero nullifier.
    pub fn new(
        namespace: PrivacyNamespaceV1,
        nullifier: PrivacyNullifierV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy nullifier namespace is invalid")?;
        if nullifier.is_zero() {
            return Err("privacy nullifier must be non-zero");
        }
        Ok(Self {
            namespace,
            nullifier,
        })
    }

    /// Return the exact namespace.
    #[must_use]
    pub const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Return the exact nullifier.
    #[must_use]
    pub const fn nullifier(self) -> PrivacyNullifierV1 {
        self.nullifier
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace, self.nullifier).map(|_| ())
    }
}

/// Key for one admitted output or account commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct PrivacyCommitmentKeyV1 {
    namespace: PrivacyNamespaceV1,
    commitment: PrivacyCommitmentV1,
}

impl PrivacyCommitmentKeyV1 {
    /// Construct and validate a scoped commitment key.
    ///
    /// # Errors
    ///
    /// Rejects an invalid namespace or an all-zero commitment.
    pub fn new(
        namespace: PrivacyNamespaceV1,
        commitment: PrivacyCommitmentV1,
    ) -> Result<Self, &'static str> {
        namespace
            .validate()
            .map_err(|_| "privacy commitment namespace is invalid")?;
        if commitment.is_zero() {
            return Err("privacy commitment must be non-zero");
        }
        Ok(Self {
            namespace,
            commitment,
        })
    }

    /// Return the exact namespace.
    #[must_use]
    pub const fn namespace(self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Return the exact commitment.
    #[must_use]
    pub const fn commitment(self) -> PrivacyCommitmentV1 {
        self.commitment
    }

    fn validate(self) -> Result<(), &'static str> {
        Self::new(self.namespace, self.commitment).map(|_| ())
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
    /// Root certified as the successor state by an admitted native proof.
    VerifiedProof {
        /// Digest of the exact verified public statement.
        statement_digest: PrivacyStatementDigestV1,
        /// Block height at which the proof effect became durable.
        admitted_at_height: u64,
        /// Zero-based privacy-action index within the transaction.
        action_index: u32,
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

    /// Construct verified-proof provenance.
    ///
    /// # Errors
    ///
    /// Rejects a zero statement digest or zero block height.
    pub(crate) fn verified_proof(
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if statement_digest.is_zero() {
            return Err("privacy root statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy root admission height must be non-zero");
        }
        Ok(Self::VerifiedProof {
            statement_digest,
            admitted_at_height,
            action_index,
        })
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

/// Current canonical root and its exact typed provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub(crate) struct PrivacyRootHeadRecordV1 {
    epoch: u64,
    root: PrivacyRootV1,
    provenance: PrivacyRootProvenanceV1,
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
    ) -> Result<Self, &'static str> {
        if epoch == 0 {
            return Err("privacy root-head epoch must be non-zero");
        }
        if root.is_zero() {
            return Err("privacy root-head root must be non-zero");
        }
        provenance.validate()?;
        Ok(Self {
            epoch,
            root,
            provenance,
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

    /// Validate a restored head record.
    ///
    /// # Errors
    ///
    /// Rejects a zero epoch/root or malformed provenance.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        Self::new(self.epoch, self.root, self.provenance).map(|_| ())
    }
}

/// Durable provenance for one verified replay marker or output commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
pub struct PrivacyStateItemRecordV1 {
    /// Digest of the exact verified public statement.
    pub statement_digest: PrivacyStatementDigestV1,
    /// Block height at which the item became durable.
    pub admitted_at_height: u64,
    /// Zero-based privacy-action index within the transaction.
    pub action_index: u32,
}

impl PrivacyStateItemRecordV1 {
    /// Construct validated durable provenance.
    ///
    /// # Errors
    ///
    /// Rejects a zero statement digest or zero block height.
    pub fn new(
        statement_digest: PrivacyStatementDigestV1,
        admitted_at_height: u64,
        action_index: u32,
    ) -> Result<Self, &'static str> {
        if statement_digest.is_zero() {
            return Err("privacy state statement digest must be non-zero");
        }
        if admitted_at_height == 0 {
            return Err("privacy state admission height must be non-zero");
        }
        Ok(Self {
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
    pub fn validate(self) -> Result<(), &'static str> {
        Self::new(
            self.statement_digest,
            self.admitted_at_height,
            self.action_index,
        )
        .map(|_| ())
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

        let existing = roots
            .range(PrivacyRootKeyV1::history_range(namespace, role))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
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

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        PrivacyConsensusLimitsV1, PrivacyCredentialSchemaIdV1, PrivacyIssuerIdV1,
        PrivacyIssuerSchemaPredicateNamespaceV1, PrivacyNamespaceScopeV1, PrivacyNullifierV1,
        PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyPredicateIdV1, PrivacyProposedLifecycleV1,
        PrivacyProtocolLifecycleV1, PrivacyRootV1, PrivacyVerifierDigestV1,
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
            PrivacyNamespaceScopeV1::IssuerSchemaPredicate(
                PrivacyIssuerSchemaPredicateNamespaceV1 {
                    issuer_id: PrivacyIssuerIdV1::new(nonzero(40)),
                    schema_id: PrivacyCredentialSchemaIdV1::new(nonzero(41)),
                    predicate_id: PrivacyPredicateIdV1::new(nonzero(42)),
                },
            ),
        )
    }

    fn root_key(role: PrivacyRootRoleV1, epoch: u64, root_byte: u8) -> PrivacyRootKeyV1 {
        PrivacyRootKeyV1::new(
            vega_namespace(),
            role,
            epoch,
            PrivacyRootV1::new(nonzero(root_byte)),
        )
        .expect("valid root key")
    }

    fn root_provenance() -> PrivacyRootProvenanceV1 {
        PrivacyRootProvenanceV1::verified_proof(PrivacyStatementDigestV1::new(nonzero(50)), 1, 0)
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
    fn promotion_rejects_noncanonical_chain_limits() {
        let mut proposal = activation_proposal();
        proposal.limits.max_proof_bytes_per_action -= 1;
        let key = PrivacyActivationKeyV1::new(proposal.protocol_id);
        let mut activations = Storage::new();
        activations.insert(key, proposal);

        assert!(matches!(
            plan_due_privacy_activation_promotions_v1(&activations.view(), 1_300),
            Err(PrivacyActivationPromotionErrorV1::ConsensusLimitsMismatch(error))
                if error.protocol_id == proposal.protocol_id
        ));
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
        let root = compute_privacy_pgc_account_state_root_v1(namespace, 7, &accounts)
            .expect("canonical PGC account table");
        assert_eq!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, &accounts).expect("same input"),
            root,
            "root derivation must be deterministic"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(pgc_namespace(21), 7, &accounts)
                .expect("different pool"),
            root,
            "pool namespace must be domain bound"
        );
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 8, &accounts)
                .expect("different epoch"),
            root,
            "epoch must be committed"
        );

        let mut changed = accounts.clone();
        changed[5].encrypted_balance.left = pgc_accounts(16)[7].encrypted_balance.right;
        assert_ne!(
            compute_privacy_pgc_account_state_root_v1(namespace, 7, &changed)
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
            compute_privacy_pgc_account_state_root_v1(namespace, 0, &accounts).is_err(),
            "epoch zero is not a persistable root"
        );
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, &accounts[..15]).is_err(),
            "cardinality outside the closed 16/32/64 set must reject"
        );

        let mut unordered = accounts.clone();
        unordered.swap(4, 5);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, &unordered).is_err(),
            "account keys must be in one canonical strict order"
        );

        let mut duplicate = accounts.clone();
        duplicate[5].public_key = duplicate[4].public_key;
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, &duplicate).is_err(),
            "duplicate accounts must reject"
        );

        let mut zero_component = accounts;
        zero_component[3].encrypted_balance.right = PrivacyP256PointV1::new([0; 33]);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, &zero_component).is_err(),
            "zero encoded points must reject before hashing"
        );

        let mut off_curve = pgc_accounts(16);
        let mut invalid = [u8::MAX; 33];
        invalid[0] = 2;
        off_curve[3].encrypted_balance.right = PrivacyP256PointV1::new(invalid);
        assert!(
            compute_privacy_pgc_account_state_root_v1(namespace, 1, &off_curve).is_err(),
            "non-zero off-curve encodings must not enter durable account state"
        );
    }

    #[test]
    fn persisted_state_rejects_orphan_privacy_items() {
        let activations =
            Storage::<PrivacyActivationKeyV1, PrivacyProtocolActivationRecordV1>::new();
        let pgc_accounts = Storage::<PrivacyPgcAccountKeyV1, PrivacyPgcAccountStateV1>::new();
        let mut nullifiers = Storage::<PrivacyNullifierKeyV1, PrivacyStateItemRecordV1>::new();
        let commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        let roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
        let root_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
        nullifiers.insert(
            PrivacyNullifierKeyV1::new(pgc_namespace(20), PrivacyNullifierV1::new(nonzero(22)))
                .expect("valid scoped key"),
            PrivacyStateItemRecordV1::new(PrivacyStatementDigestV1::new(nonzero(23)), 1, 0)
                .expect("valid provenance"),
        );

        let error = validate_privacy_persisted_state_v1(
            &activations.view(),
            &pgc_accounts.view(),
            &nullifiers.view(),
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
        .expect_err("an unregistered protocol cannot own persisted state");
        assert!(error.contains("unregistered protocol"), "{error}");
    }

    #[test]
    fn identical_nullifiers_in_distinct_pools_have_distinct_keys() {
        let namespace_a = pgc_namespace(20);
        let namespace_b = pgc_namespace(21);
        let nullifier = PrivacyNullifierV1::new(nonzero(22));
        let key_a = PrivacyNullifierKeyV1::new(namespace_a, nullifier).expect("valid key");
        let key_b = PrivacyNullifierKeyV1::new(namespace_b, nullifier).expect("valid key");

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
    }

    #[test]
    fn storage_key_json_roundtrip_and_malformed_inputs_reject() {
        let namespace = pgc_namespace(20);
        let key = PrivacyNullifierKeyV1::new(namespace, PrivacyNullifierV1::new(nonzero(0xAB)))
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
                root_key(PrivacyRootRoleV1::Issuer, epoch, root_byte),
                record,
            );
            roots.insert(
                root_key(PrivacyRootRoleV1::Revocation, epoch, root_byte),
                record,
            );
        }

        let added = root_key(PrivacyRootRoleV1::Issuer, 2_049, 43);
        let removals = plan_privacy_root_history_update_v1(&roots.view(), &[added], 2_048)
            .expect("valid plan");

        assert_eq!(removals, vec![root_key(PrivacyRootRoleV1::Issuer, 1, 2)]);
        assert!(
            roots
                .view()
                .get(&root_key(PrivacyRootRoleV1::Revocation, 1, 2))
                .is_some(),
            "planning one role must not prune another role"
        );
    }

    #[test]
    fn root_history_rejects_replays_epoch_conflicts_and_stale_epochs() {
        let mut roots = Storage::new();
        roots.insert(
            root_key(PrivacyRootRoleV1::Issuer, 7, 70),
            root_provenance(),
        );

        let exact_replay = root_key(PrivacyRootRoleV1::Issuer, 7, 70);
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &[exact_replay], 8),
            Err(PrivacyRootHistoryErrorV1::ExistingRoot { key }) if key == exact_replay
        ));

        assert!(matches!(
            plan_privacy_root_history_update_v1(
                &roots.view(),
                &[root_key(PrivacyRootRoleV1::Issuer, 7, 71)],
                8,
            ),
            Err(PrivacyRootHistoryErrorV1::EpochConflict { epoch: 7, .. })
        ));

        assert!(matches!(
            plan_privacy_root_history_update_v1(
                &roots.view(),
                &[root_key(PrivacyRootRoleV1::Issuer, 6, 60)],
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
        let duplicate = root_key(PrivacyRootRoleV1::Issuer, 1, 10);
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &[duplicate, duplicate], 8),
            Err(PrivacyRootHistoryErrorV1::DuplicateAddedRoot { key }) if key == duplicate
        ));

        let additions = [
            root_key(PrivacyRootRoleV1::Issuer, 1, 10),
            root_key(PrivacyRootRoleV1::Issuer, 2, 20),
            root_key(PrivacyRootRoleV1::Issuer, 3, 30),
        ];
        assert!(matches!(
            plan_privacy_root_history_update_v1(&roots.view(), &additions, 2),
            Err(PrivacyRootHistoryErrorV1::AddedRootsExceedRetention { count: 3, max: 2 })
        ));
    }

    #[test]
    fn identical_epoch_and_root_are_allowed_in_distinct_roles() {
        let roots = Storage::new();
        let issuer = root_key(PrivacyRootRoleV1::Issuer, 9, 90);
        let revocation = root_key(PrivacyRootRoleV1::Revocation, 9, 90);

        assert_eq!(
            plan_privacy_root_history_update_v1(&roots.view(), &[issuer, revocation], 8)
                .expect("roles are independent"),
            Vec::<PrivacyRootKeyV1>::new()
        );
    }
}
