//! Content-addressed persistence primitives for KAGEMUSHA authenticated history.
//!
//! This module deliberately separates the small, byte-bounded live prepare/WAL overlay from
//! immutable committed sparse-tree nodes. The overlay limits concurrent uncommitted work only:
//! neither committed replay history nor committed terminal-decision history has a count, age, or
//! byte admission limit here. A storage outage reports the exact committed root as unavailable;
//! it never substitutes an empty root or discards already committed value.

#[cfg(unix)]
#[path = "disk_history_store.rs"]
pub(crate) mod disk_history_store;

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaPastaStateCommitmentV1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::super::DigestV1;

const HISTORY_STORE_VERSION_V1: u16 = 1;
const REPLAY_EMPTY_ROOT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:history-store:replay:empty-root\0";
const DECISION_EMPTY_ROOT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:history-store:terminal-decision:empty-root\0";
const REPLAY_LEAF_ADDRESS_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:history-store:replay:leaf\0";
const DECISION_LEAF_ADDRESS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:history-store:terminal-decision:leaf\0";
const REPLAY_BRANCH_ADDRESS_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:history-store:replay:branch\0";
const DECISION_BRANCH_ADDRESS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:history-store:terminal-decision:branch\0";
const PREPARED_CAS_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:history-store:prepared-cas\0";
const ROOT_SELECTION_CERTIFICATE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:history-store:root-selection-certificate\0";
const PROOF_ROOT_BRIDGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:history-store:proof-root-bridge\0";

/// Independent authenticated sparse-tree namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub(crate) enum KagemushaHistoryTreeV1 {
    /// Consumed-credit replay nonmembership tree.
    Replay,
    /// Durable transition-outcome decision tree used for exact crash recovery.
    TerminalDecision,
}

impl KagemushaHistoryTreeV1 {
    const fn empty_root_domain(self) -> &'static [u8] {
        match self {
            Self::Replay => REPLAY_EMPTY_ROOT_DOMAIN_V1,
            Self::TerminalDecision => DECISION_EMPTY_ROOT_DOMAIN_V1,
        }
    }

    const fn leaf_address_domain(self) -> &'static [u8] {
        match self {
            Self::Replay => REPLAY_LEAF_ADDRESS_DOMAIN_V1,
            Self::TerminalDecision => DECISION_LEAF_ADDRESS_DOMAIN_V1,
        }
    }

    const fn branch_address_domain(self) -> &'static [u8] {
        match self {
            Self::Replay => REPLAY_BRANCH_ADDRESS_DOMAIN_V1,
            Self::TerminalDecision => DECISION_BRANCH_ADDRESS_DOMAIN_V1,
        }
    }
}

/// One immutable sparse-tree node body.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) enum KagemushaHistoryNodeBodyV1 {
    /// One exact key-to-value-digest binding.
    Leaf {
        /// Full 256-bit sparse-tree key.
        key: DigestV1,
        /// Digest of the replay or terminal-decision value.
        value_digest: DigestV1,
    },
    /// One sparse-tree branch addressed through immutable child records.
    Branch {
        /// Root-relative branch depth in `0..256`.
        depth: u16,
        /// Canonical key prefix; bits after `depth` are zero.
        prefix: DigestV1,
        /// Content address of the left child or the tree-specific empty child.
        left: DigestV1,
        /// Content address of the right child or the tree-specific empty child.
        right: DigestV1,
    },
}

/// Domain-separated immutable sparse-tree node record.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryNodeRecordV1 {
    version: u16,
    tree: KagemushaHistoryTreeV1,
    body: KagemushaHistoryNodeBodyV1,
}

impl KagemushaHistoryNodeRecordV1 {
    /// Construct one validated leaf in the selected tree namespace.
    pub(crate) fn leaf(
        tree: KagemushaHistoryTreeV1,
        key: DigestV1,
        value_digest: DigestV1,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        let record = Self {
            version: HISTORY_STORE_VERSION_V1,
            tree,
            body: KagemushaHistoryNodeBodyV1::Leaf { key, value_digest },
        };
        record.validate()?;
        Ok(record)
    }

    /// Construct one validated branch in the selected tree namespace.
    pub(crate) fn branch(
        tree: KagemushaHistoryTreeV1,
        depth: u16,
        prefix: DigestV1,
        left: DigestV1,
        right: DigestV1,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        let record = Self {
            version: HISTORY_STORE_VERSION_V1,
            tree,
            body: KagemushaHistoryNodeBodyV1::Branch {
                depth,
                prefix,
                left,
                right,
            },
        };
        record.validate()?;
        Ok(record)
    }

    /// Return this record's non-interchangeable tree namespace.
    pub(crate) const fn tree(&self) -> KagemushaHistoryTreeV1 {
        self.tree
    }

    /// Borrow the canonical node body for authenticated traversal.
    pub(crate) const fn body(&self) -> &KagemushaHistoryNodeBodyV1 {
        &self.body
    }

    /// Derive the SHA-256 content address of the exact canonical Norito record.
    pub(crate) fn content_address(&self) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
        self.validate()?;
        let domain = match &self.body {
            KagemushaHistoryNodeBodyV1::Leaf { .. } => self.tree.leaf_address_domain(),
            KagemushaHistoryNodeBodyV1::Branch { .. } => self.tree.branch_address_domain(),
        };
        digest_canonical(domain, self)
    }

    fn validate(&self) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if self.version != HISTORY_STORE_VERSION_V1 {
            return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
        }
        match &self.body {
            KagemushaHistoryNodeBodyV1::Leaf { key, value_digest } => {
                if digest_is_zero(*key) || digest_is_zero(*value_digest) {
                    return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
                }
            }
            KagemushaHistoryNodeBodyV1::Branch {
                depth,
                prefix,
                left,
                right,
            } => {
                let depth = usize::from(*depth);
                if depth >= 256
                    || canonical_prefix(*prefix, depth) != *prefix
                    || digest_is_zero(*left)
                    || digest_is_zero(*right)
                {
                    return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
                }
            }
        }
        Ok(())
    }
}

/// The two independently selected committed content-address roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryRootsV1 {
    replay: DigestV1,
    terminal_decision: DigestV1,
}

impl KagemushaHistoryRootsV1 {
    /// Return the deterministic empty roots for both independent tree domains.
    pub(crate) fn empty() -> Self {
        Self {
            replay: empty_root(KagemushaHistoryTreeV1::Replay),
            terminal_decision: empty_root(KagemushaHistoryTreeV1::TerminalDecision),
        }
    }

    /// Return the currently selected replay root.
    pub(crate) const fn replay(self) -> DigestV1 {
        self.replay
    }

    /// Return the currently selected terminal-decision root.
    pub(crate) const fn terminal_decision(self) -> DigestV1 {
        self.terminal_decision
    }

    /// Return the root selected for one tree.
    pub(crate) const fn for_tree(self, tree: KagemushaHistoryTreeV1) -> DigestV1 {
        match tree {
            KagemushaHistoryTreeV1::Replay => self.replay,
            KagemushaHistoryTreeV1::TerminalDecision => self.terminal_decision,
        }
    }

    fn select(&mut self, tree: KagemushaHistoryTreeV1, root: DigestV1) {
        match tree {
            KagemushaHistoryTreeV1::Replay => self.replay = root,
            KagemushaHistoryTreeV1::TerminalDecision => self.terminal_decision = root,
        }
    }
}

impl Default for KagemushaHistoryRootsV1 {
    fn default() -> Self {
        Self::empty()
    }
}

/// Compare-and-swap transition for one independently selected tree root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryRootCasV1 {
    expected: DigestV1,
    selected: DigestV1,
}

impl KagemushaHistoryRootCasV1 {
    /// Construct one exact expected-to-selected root transition.
    pub(crate) const fn new(expected: DigestV1, selected: DigestV1) -> Self {
        Self { expected, selected }
    }

    /// Return the root that must still be selected at commit.
    pub(crate) const fn expected(self) -> DigestV1 {
        self.expected
    }

    /// Return the successor root authenticated by hardware.
    pub(crate) const fn selected(self) -> DigestV1 {
        self.selected
    }

    fn is_valid(self) -> bool {
        !digest_is_zero(self.expected)
            && !digest_is_zero(self.selected)
            && self.expected != self.selected
    }
}

/// Independent optional root transitions covered by one prepared CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryRootSelectionV1 {
    replay: Option<KagemushaHistoryRootCasV1>,
    terminal_decision: Option<KagemushaHistoryRootCasV1>,
}

impl KagemushaHistoryRootSelectionV1 {
    /// Select only a replay-root transition.
    pub(crate) const fn replay(expected: DigestV1, selected: DigestV1) -> Self {
        Self {
            replay: Some(KagemushaHistoryRootCasV1::new(expected, selected)),
            terminal_decision: None,
        }
    }

    /// Select only a terminal-decision-root transition.
    pub(crate) const fn terminal_decision(expected: DigestV1, selected: DigestV1) -> Self {
        Self {
            replay: None,
            terminal_decision: Some(KagemushaHistoryRootCasV1::new(expected, selected)),
        }
    }

    /// Select both roots atomically, retaining a separate CAS precondition for each tree.
    pub(crate) const fn both(
        replay: KagemushaHistoryRootCasV1,
        terminal_decision: KagemushaHistoryRootCasV1,
    ) -> Self {
        Self {
            replay: Some(replay),
            terminal_decision: Some(terminal_decision),
        }
    }

    /// Return the optional transition for one independent tree.
    pub(crate) const fn for_tree(
        self,
        tree: KagemushaHistoryTreeV1,
    ) -> Option<KagemushaHistoryRootCasV1> {
        match tree {
            KagemushaHistoryTreeV1::Replay => self.replay,
            KagemushaHistoryTreeV1::TerminalDecision => self.terminal_decision,
        }
    }

    fn validate(self) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if (self.replay.is_none() && self.terminal_decision.is_none())
            || self.replay.is_some_and(|cas| !cas.is_valid())
            || self.terminal_decision.is_some_and(|cas| !cas.is_valid())
            || self
                .replay
                .is_some_and(|cas| cas.selected() == empty_root(KagemushaHistoryTreeV1::Replay))
            || self.terminal_decision.is_some_and(|cas| {
                cas.selected() == empty_root(KagemushaHistoryTreeV1::TerminalDecision)
            })
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidRootSelection);
        }
        Ok(())
    }

    fn apply_to(
        self,
        current: KagemushaHistoryRootsV1,
    ) -> Result<KagemushaHistoryRootsV1, KagemushaHistoryStoreErrorV1> {
        let mut successor = current;
        for tree in [
            KagemushaHistoryTreeV1::Replay,
            KagemushaHistoryTreeV1::TerminalDecision,
        ] {
            if let Some(cas) = self.for_tree(tree) {
                let actual = current.for_tree(tree);
                if actual != cas.expected() {
                    return Err(KagemushaHistoryStoreErrorV1::CasConflict {
                        tree,
                        expected: cas.expected(),
                        actual,
                    });
                }
                successor.select(tree, cas.selected());
            }
        }
        Ok(successor)
    }
}

/// Canonical content-addressed write retained by a prepared transaction.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaHistoryNodeWriteV1 {
    address: DigestV1,
    node: KagemushaHistoryNodeRecordV1,
}

/// Immutable prepared CAS material stored in the live WAL overlay.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaPreparedHistoryCasV1 {
    transaction_id: DigestV1,
    attempt_binding_digest: DigestV1,
    root_selection: KagemushaHistoryRootSelectionV1,
    node_writes: Vec<KagemushaHistoryNodeWriteV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct KagemushaPreparedHistoryCasSubjectV1 {
    version: u16,
    attempt_binding_digest: DigestV1,
    root_selection: KagemushaHistoryRootSelectionV1,
    node_writes: Vec<KagemushaHistoryNodeWriteV1>,
}

impl KagemushaPreparedHistoryCasV1 {
    /// Canonicalize and content-address one prepared root transition and its immutable nodes.
    pub(crate) fn new(
        root_selection: KagemushaHistoryRootSelectionV1,
        nodes: Vec<KagemushaHistoryNodeRecordV1>,
        attempt_binding_digest: DigestV1,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        if digest_is_zero(attempt_binding_digest) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        root_selection.validate()?;
        let mut node_writes = Vec::with_capacity(nodes.len());
        for node in nodes {
            node_writes.push(KagemushaHistoryNodeWriteV1 {
                address: node.content_address()?,
                node,
            });
        }
        node_writes.sort_by_key(|write| write.address);
        if node_writes
            .windows(2)
            .any(|pair| pair[0].address == pair[1].address)
        {
            return Err(KagemushaHistoryStoreErrorV1::DuplicateNodeAddress);
        }
        let subject = KagemushaPreparedHistoryCasSubjectV1 {
            version: HISTORY_STORE_VERSION_V1,
            attempt_binding_digest,
            root_selection,
            node_writes: node_writes.clone(),
        };
        let transaction_id = digest_canonical(PREPARED_CAS_DOMAIN_V1, &subject)?;
        if digest_is_zero(transaction_id) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        Ok(Self {
            transaction_id,
            attempt_binding_digest,
            root_selection,
            node_writes,
        })
    }

    /// Return this prepared transaction's deterministic content address.
    pub(crate) const fn transaction_id(&self) -> DigestV1 {
        self.transaction_id
    }

    /// Return the Core transition attempt bound into the transaction identity.
    pub(crate) const fn attempt_binding_digest(&self) -> DigestV1 {
        self.attempt_binding_digest
    }

    /// Return the exact independently scoped root selection.
    pub(crate) const fn root_selection(&self) -> KagemushaHistoryRootSelectionV1 {
        self.root_selection
    }

    /// Apply this transaction's independent CAS preconditions to exact predecessor roots.
    pub(crate) fn successor_roots_from(
        &self,
        predecessor: KagemushaHistoryRootsV1,
    ) -> Result<KagemushaHistoryRootsV1, KagemushaHistoryStoreErrorV1> {
        self.validate()?;
        self.root_selection.apply_to(predecessor)
    }

    /// Return exact canonical bytes charged to the uncommitted overlay/WAL.
    pub(crate) fn wal_bytes(&self) -> Result<u64, KagemushaHistoryStoreErrorV1> {
        let encoded = norito::encode_canonical(self)
            .map_err(|_| KagemushaHistoryStoreErrorV1::CanonicalEncoding)?;
        u64::try_from(encoded.len()).map_err(|_| KagemushaHistoryStoreErrorV1::CanonicalEncoding)
    }

    fn validate(&self) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if digest_is_zero(self.attempt_binding_digest) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        self.root_selection.validate()?;
        if self.node_writes.windows(2).any(|pair| {
            pair[0].address >= pair[1].address
                || pair[0].node.content_address().ok() != Some(pair[0].address)
        }) || self
            .node_writes
            .last()
            .is_some_and(|write| write.node.content_address().ok() != Some(write.address))
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        let subject = KagemushaPreparedHistoryCasSubjectV1 {
            version: HISTORY_STORE_VERSION_V1,
            attempt_binding_digest: self.attempt_binding_digest,
            root_selection: self.root_selection,
            node_writes: self.node_writes.clone(),
        };
        if digest_canonical(PREPARED_CAS_DOMAIN_V1, &subject)? != self.transaction_id {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        Ok(())
    }
}

/// Exact hardware-signed subject selecting the roots of one prepared CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryRootSelectionSubjectV1 {
    version: u16,
    transaction_id: DigestV1,
    root_selection: KagemushaHistoryRootSelectionV1,
    hardware_profile_id: DigestV1,
    hardware_epoch: u128,
    monotonic_counter: u128,
}

impl KagemushaHistoryRootSelectionSubjectV1 {
    /// Bind one prepared transaction to a hardware profile, epoch, and rollback-resistant counter.
    pub(crate) const fn new(
        transaction: &KagemushaPreparedHistoryCasV1,
        hardware_profile_id: DigestV1,
        hardware_epoch: u128,
        monotonic_counter: u128,
    ) -> Self {
        Self {
            version: HISTORY_STORE_VERSION_V1,
            transaction_id: transaction.transaction_id(),
            root_selection: transaction.root_selection(),
            hardware_profile_id,
            hardware_epoch,
            monotonic_counter,
        }
    }

    /// Return domain-separated canonical bytes for the hardware signature operation.
    pub(crate) fn signing_bytes(&self) -> Result<Vec<u8>, KagemushaHistoryStoreErrorV1> {
        self.validate()?;
        let encoded = norito::encode_canonical(self)
            .map_err(|_| KagemushaHistoryStoreErrorV1::CanonicalEncoding)?;
        let mut bytes =
            Vec::with_capacity(ROOT_SELECTION_CERTIFICATE_DOMAIN_V1.len() + encoded.len());
        bytes.extend_from_slice(ROOT_SELECTION_CERTIFICATE_DOMAIN_V1);
        bytes.extend_from_slice(&encoded);
        Ok(bytes)
    }

    fn validate(self) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if self.version != HISTORY_STORE_VERSION_V1
            || digest_is_zero(self.transaction_id)
            || digest_is_zero(self.hardware_profile_id)
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        self.root_selection
            .validate()
            .map_err(|_| KagemushaHistoryStoreErrorV1::InvalidCertificate)
    }
}

/// Hardware-authenticated selection of the exact roots in one prepared CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryRootSelectionCertificateV1 {
    subject: KagemushaHistoryRootSelectionSubjectV1,
    signature: KagemushaDeviceSignatureV1,
}

impl KagemushaHistoryRootSelectionCertificateV1 {
    /// Attach the hardware signature to its exact root-selection subject.
    pub(crate) const fn new(
        subject: KagemushaHistoryRootSelectionSubjectV1,
        signature: KagemushaDeviceSignatureV1,
    ) -> Self {
        Self { subject, signature }
    }

    /// Authenticate this certificate against the release-approved profile and device key.
    pub(crate) fn verify(
        self,
        expected_hardware_profile_id: DigestV1,
        device_public_key: &KagemushaDevicePublicKeyV1,
    ) -> Result<VerifiedKagemushaHistoryRootSelectionV1, KagemushaHistoryStoreErrorV1> {
        self.subject.validate()?;
        if self.subject.hardware_profile_id != expected_hardware_profile_id {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        let signing_bytes = self.subject.signing_bytes()?;
        self.signature
            .verify(device_public_key, &signing_bytes)
            .map_err(|_| KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
        Ok(VerifiedKagemushaHistoryRootSelectionV1 { certificate: self })
    }
}

/// Typestate proving that a release-approved device authenticated one exact root selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedKagemushaHistoryRootSelectionV1 {
    // Keep signed evidence for durable replay, but never serialize the verified typestate.
    certificate: KagemushaHistoryRootSelectionCertificateV1,
}

impl VerifiedKagemushaHistoryRootSelectionV1 {
    /// Return the authenticated prepared-transaction identity.
    pub(crate) const fn transaction_id(self) -> DigestV1 {
        self.certificate.subject.transaction_id
    }

    /// Return the authenticated independent root selection.
    pub(crate) const fn root_selection(self) -> KagemushaHistoryRootSelectionV1 {
        self.certificate.subject.root_selection
    }

    /// Return the authenticated hardware profile identity.
    pub(crate) const fn hardware_profile_id(self) -> DigestV1 {
        self.certificate.subject.hardware_profile_id
    }

    /// Return the authenticated hardware epoch.
    pub(crate) const fn hardware_epoch(self) -> u128 {
        self.certificate.subject.hardware_epoch
    }

    /// Return the authenticated rollback-resistant device counter.
    pub(crate) const fn monotonic_counter(self) -> u128 {
        self.certificate.subject.monotonic_counter
    }
}

/// Exact byte accounting for the live, uncommitted prepare/WAL overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaHistoryOverlayUsageV1 {
    live_bytes: u64,
    capacity_bytes: u64,
}

impl KagemushaHistoryOverlayUsageV1 {
    /// Return canonical bytes retained for transactions that have not reached a terminal state.
    pub(crate) const fn live_bytes(self) -> u64 {
        self.live_bytes
    }

    /// Return the local live-overlay capacity; this is not a committed-history limit.
    pub(crate) const fn capacity_bytes(self) -> u64 {
        self.capacity_bytes
    }
}

/// Availability of the exact node selected by a committed tree root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaCommittedRootReadV1 {
    /// The backing store answered for the exact root.
    Available {
        /// Exact committed root that was requested.
        root: DigestV1,
        /// Root node, absent only for the deterministic empty-tree root.
        node: Option<KagemushaHistoryNodeRecordV1>,
    },
    /// The backing store is unavailable; the committed root remains authoritative.
    Unavailable {
        /// Exact committed root whose node could not currently be retrieved.
        root: DigestV1,
    },
}

/// Result of durably preparing one root CAS in the live overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryPrepareOutcomeV1 {
    /// The transaction was added to the uncommitted overlay.
    Prepared,
    /// The exact transaction was already prepared.
    AlreadyPrepared,
    /// The exact transaction already committed to these roots.
    AlreadyCommitted {
        /// Roots selected immediately after the transaction committed.
        committed_roots: KagemushaHistoryRootsV1,
    },
    /// The exact transaction already reached an aborted terminal state.
    AlreadyAborted,
}

/// Result of committing one hardware-authenticated prepared root CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryCommitOutcomeV1 {
    /// The authenticated root selection committed atomically.
    Committed {
        /// Roots selected immediately after this commit.
        committed_roots: KagemushaHistoryRootsV1,
    },
    /// The same transaction had already committed.
    AlreadyCommitted {
        /// Roots selected immediately after the original commit.
        committed_roots: KagemushaHistoryRootsV1,
    },
    /// The transaction had already reached an aborted terminal state.
    Aborted,
}

/// Result of aborting one prepared root CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryAbortOutcomeV1 {
    /// The uncommitted transaction was removed and recorded as aborted.
    Aborted,
    /// The same transaction had already been aborted.
    AlreadyAborted,
    /// The transaction had already committed and cannot be invalidated by abort.
    AlreadyCommitted {
        /// Roots selected immediately after the original commit.
        committed_roots: KagemushaHistoryRootsV1,
    },
}

/// Result of recovering one hardware-authenticated prepared root CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryRecoveryOutcomeV1 {
    /// Recovery completed the pending atomic commit.
    Committed {
        /// Roots selected immediately after the recovered commit.
        committed_roots: KagemushaHistoryRootsV1,
    },
    /// The transaction had already committed before recovery was retried.
    AlreadyCommitted {
        /// Roots selected immediately after the original commit.
        committed_roots: KagemushaHistoryRootsV1,
    },
    /// The transaction had already reached an aborted terminal state.
    Aborted,
}

/// Deterministic failures exposed by authenticated-history storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum KagemushaHistoryStoreErrorV1 {
    /// Canonical Norito encoding failed.
    #[error("canonical KAGEMUSHA history encoding failed")]
    CanonicalEncoding,
    /// A node record violates the V1 content-addressing rules.
    #[error("invalid KAGEMUSHA history node")]
    InvalidNode,
    /// A prepared transaction repeats one content address.
    #[error("duplicate KAGEMUSHA history node address")]
    DuplicateNodeAddress,
    /// A root transition is empty, zero, unchanged, or selects an empty root.
    #[error("invalid KAGEMUSHA history root selection")]
    InvalidRootSelection,
    /// Prepared material does not match its canonical transaction identity.
    #[error("invalid KAGEMUSHA prepared history transaction")]
    InvalidTransaction,
    /// The hardware root-selection certificate is malformed or unauthenticated.
    #[error("invalid KAGEMUSHA history root-selection certificate")]
    InvalidCertificate,
    /// The external SHA-256 and recursive Pasta roots do not describe one exact replay CAS.
    #[error("invalid KAGEMUSHA authenticated-history proof-root bridge request")]
    InvalidProofRootBridge,
    /// A new hardware authorization requires this exact attempt to remain prepared.
    #[error("KAGEMUSHA history attempt is no longer prepared: {0:?}")]
    AttemptNotPrepared(DigestV1),
    /// The certificate selects different roots or a different transaction.
    #[error("KAGEMUSHA history certificate does not match the prepared transaction")]
    CertificateMismatch,
    /// The uncommitted overlay cannot retain this prepared transaction.
    #[error(
        "KAGEMUSHA live history overlay needs {required_bytes} bytes but has {available_bytes} bytes available"
    )]
    OverlayCapacityExceeded {
        /// Exact canonical transaction bytes that would be added.
        required_bytes: u64,
        /// Bytes currently available to uncommitted work.
        available_bytes: u64,
    },
    /// Another commit changed the independently selected tree root.
    #[error("KAGEMUSHA {tree:?} root CAS conflict: expected {expected:?}, found {actual:?}")]
    CasConflict {
        /// Independent tree whose root no longer matches.
        tree: KagemushaHistoryTreeV1,
        /// Root against which the transaction was prepared.
        expected: DigestV1,
        /// Root selected at the time commit was attempted.
        actual: DigestV1,
    },
    /// Recovery opened an external store whose authoritative roots differ from the checkpoint.
    #[error("KAGEMUSHA authenticated-history recovery roots do not match the checkpoint")]
    CommittedRootsMismatch {
        /// Roots authenticated by the recovery checkpoint.
        expected: KagemushaHistoryRootsV1,
        /// Roots currently selected by the external store.
        actual: KagemushaHistoryRootsV1,
    },
    /// The same content address resolved to non-identical immutable bytes.
    #[error("KAGEMUSHA history content-address collision at {0:?}")]
    ContentAddressCollision(DigestV1),
    /// A selected root is not backed by a node in its exact tree namespace.
    #[error("missing KAGEMUSHA {tree:?} selected root node {root:?}")]
    MissingSelectedRoot {
        /// Independent tree whose selected root is missing.
        tree: KagemushaHistoryTreeV1,
        /// Missing selected content address.
        root: DigestV1,
    },
    /// A committed nonempty root is missing even though storage answered.
    #[error("missing committed KAGEMUSHA {tree:?} root node {root:?}")]
    MissingCommittedRoot {
        /// Independent tree whose committed node is missing.
        tree: KagemushaHistoryTreeV1,
        /// Authoritative committed root that must not be replaced.
        root: DigestV1,
    },
    /// A non-root child selected by authenticated history is missing.
    #[error("missing KAGEMUSHA {tree:?} history node {address:?}")]
    MissingHistoryNode {
        /// Independent tree whose authenticated path selected the missing node.
        tree: KagemushaHistoryTreeV1,
        /// Missing content address.
        address: DigestV1,
    },
    /// Stored bytes do not match the content address or tree namespace used to reach them.
    #[error("corrupt KAGEMUSHA {tree:?} history node {address:?}")]
    CorruptHistoryNode {
        /// Independent tree whose authenticated path selected the corrupt node.
        tree: KagemushaHistoryTreeV1,
        /// Content address whose stored record failed validation.
        address: DigestV1,
    },
    /// An authenticated sparse-tree branch graph is cyclic or structurally inconsistent.
    #[error("invalid KAGEMUSHA {tree:?} history tree rooted at {root:?}")]
    InvalidHistoryTree {
        /// Independent tree whose structure failed validation.
        tree: KagemushaHistoryTreeV1,
        /// Authoritative root that must remain selected despite the read failure.
        root: DigestV1,
    },
    /// The transaction is neither prepared nor terminal.
    #[error("unknown KAGEMUSHA prepared history transaction {0:?}")]
    UnknownTransaction(DigestV1),
    /// The external immutable-node store is currently unavailable.
    #[error("KAGEMUSHA authenticated-history storage is unavailable")]
    StorageUnavailable,
    /// The persisted journal failed framing, identity, or deterministic replay validation.
    #[error("KAGEMUSHA authenticated-history journal is corrupt")]
    JournalCorrupt,
    /// Another owner already holds the journal's descriptor-based writer lock.
    #[error("KAGEMUSHA authenticated-history journal already has a writer")]
    StoreAlreadyOpen,
    /// The exact retained prepare/terminal history differs from the hardware-sealed snapshot.
    #[error("KAGEMUSHA authenticated-history recovery commitment mismatch")]
    RecoveryCommitmentMismatch,
    /// A write or sync had an uncertain outcome; this handle can no longer authorize work.
    #[error("KAGEMUSHA authenticated-history durability is uncertain")]
    DurabilityUncertain,
}

/// External persistence boundary for KAGEMUSHA's two authenticated history trees.
///
/// Implementations must make a commit's nodes, selected roots, and terminal transaction result
/// atomic and durable. Repeating prepare, commit, abort, or recovery must return the corresponding
/// `Already*`/terminal outcome. Capacity applies only to the live prepare/WAL overlay; committed
/// nodes and terminal records are not evicted or rejected through that capacity.
pub(crate) trait KagemushaAuthenticatedHistoryStoreV1 {
    /// Return both authoritative committed roots without consulting object availability.
    fn committed_roots(&self) -> KagemushaHistoryRootsV1;

    /// Bind the exact successful prepare/commit/abort history into the hardware-sealed snapshot.
    /// This detects valid host-log rollback even when neither committed tree root changed.
    fn recovery_commitment(&self) -> Result<DigestV1, KagemushaHistoryStoreErrorV1>;

    /// Require a retained hardware-sealed checkpoint with no later hardware commit. Any
    /// prepare/abort suffix remains recorded; validating it never commits or discards work.
    fn validate_recovery_checkpoint(
        &self,
        expected: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1>;

    /// Validate one complete retained tree. Implementations may reuse only process-local
    /// validation of immutable durable subtrees whose availability/integrity is still known.
    /// The default always traverses every node; external data cannot supply cache authority.
    fn validate_tree(
        &self,
        tree: KagemushaHistoryTreeV1,
        root: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        validate_tree_with_lookup(tree, root, |address| self.read_node(address))
    }

    /// Require an exact, currently prepared CAS before requesting new hardware authority.
    /// Storage/integrity and stale-root errors remain errors, never `false` or absence.
    fn require_prepared(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1>;

    /// Return exact byte usage of the uncommitted overlay.
    fn overlay_usage(&self) -> KagemushaHistoryOverlayUsageV1;

    /// Read one immutable node by its content address.
    fn read_node(
        &self,
        address: DigestV1,
    ) -> Result<Option<KagemushaHistoryNodeRecordV1>, KagemushaHistoryStoreErrorV1>;

    /// Read the node at one exact committed root without substituting another root on failure.
    fn read_committed_root(
        &self,
        tree: KagemushaHistoryTreeV1,
    ) -> Result<KagemushaCommittedRootReadV1, KagemushaHistoryStoreErrorV1>;

    /// Durably stage one canonical CAS before requesting hardware root authorization.
    fn prepare_cas(
        &mut self,
        transaction: KagemushaPreparedHistoryCasV1,
    ) -> Result<KagemushaHistoryPrepareOutcomeV1, KagemushaHistoryStoreErrorV1>;

    /// Atomically commit one prepared CAS selected by an authenticated hardware certificate.
    fn commit_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryCommitOutcomeV1, KagemushaHistoryStoreErrorV1>;

    /// Abort one still-uncommitted CAS without changing either committed root.
    fn abort_prepared(
        &mut self,
        transaction_id: DigestV1,
    ) -> Result<KagemushaHistoryAbortOutcomeV1, KagemushaHistoryStoreErrorV1>;

    /// Resolve a prepared transaction after restart using its authenticated terminal selection.
    fn recover_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryRecoveryOutcomeV1, KagemushaHistoryStoreErrorV1>;
}

/// Validate both complete committed authenticated trees without changing either selected root.
///
/// Recovery calls this before trusting external history. Any unavailable, missing, corrupt,
/// cross-namespace, cyclic, or structurally inconsistent node fails closed while the store's
/// authoritative committed roots remain unchanged.
pub(crate) fn validate_committed_history_v1<S>(
    store: &S,
) -> Result<KagemushaHistoryRootsV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    let roots = store.committed_roots();
    for tree in [
        KagemushaHistoryTreeV1::Replay,
        KagemushaHistoryTreeV1::TerminalDecision,
    ] {
        validate_tree_from_store(store, tree, roots.for_tree(tree))?;
    }
    Ok(roots)
}

/// Exact replay/terminal identity classification against an authenticated committed root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryIdentityClassificationV1 {
    /// The identity has no committed leaf.
    Absent,
    /// The identity is already bound to the byte-identical value digest.
    ExactDuplicate,
    /// The identity is committed to another value digest.
    Conflict {
        /// Value digest already authenticated under this identity.
        existing_value_digest: DigestV1,
    },
}

/// Result of preparing one exact authenticated identity insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryInsertPreparationV1 {
    /// A new root CAS was retained in the live WAL overlay.
    Prepared {
        /// Canonical transaction that hardware must authenticate before commit.
        transaction: KagemushaPreparedHistoryCasV1,
        /// Idempotent low-level prepare result for the same transaction.
        outcome: KagemushaHistoryPrepareOutcomeV1,
    },
    /// The committed tree already binds the identity to the same value digest.
    ExactDuplicate,
    /// The committed tree already binds the identity to different bytes.
    Conflict {
        /// Value digest already authenticated under this identity.
        existing_value_digest: DigestV1,
    },
}

/// Result of preparing replay and terminal-decision identity updates as one root selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KagemushaHistoryDualInsertPreparationV1 {
    /// At least one absent identity produced a prepared one- or two-root CAS.
    Prepared {
        /// Canonical transaction that hardware must authenticate before commit.
        transaction: KagemushaPreparedHistoryCasV1,
        /// Idempotent low-level prepare result for the same transaction.
        outcome: KagemushaHistoryPrepareOutcomeV1,
    },
    /// Both committed trees already contain their byte-identical bindings.
    ExactDuplicate,
    /// At least one committed identity is bound to different bytes.
    Conflict {
        /// Independent tree containing the conflict.
        tree: KagemushaHistoryTreeV1,
        /// Identity whose committed value differs.
        key: DigestV1,
        /// Value digest already authenticated under that identity.
        existing_value_digest: DigestV1,
    },
}

/// Exact cross-commitment statement needed before an external replay-root CAS may authorize a
/// recursive state transition.
///
/// SHA-256 content addresses and paired Pasta commitments are intentionally carried as distinct
/// types and fields. Their byte representations are not interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaHistoryProofRootBridgeRequestV1 {
    transaction_id: DigestV1,
    operation_binding_digest: DigestV1,
    external_predecessor_roots: KagemushaHistoryRootsV1,
    external_successor_roots: KagemushaHistoryRootsV1,
    pasta_predecessor_replay_root: KagemushaPastaStateCommitmentV1,
    pasta_successor_replay_root: KagemushaPastaStateCommitmentV1,
}

impl KagemushaHistoryProofRootBridgeRequestV1 {
    /// Construct the exact roots that a future proof- and hardware-authenticated bridge must bind.
    pub(crate) fn new(
        transaction: &KagemushaPreparedHistoryCasV1,
        operation_binding_digest: DigestV1,
        external_predecessor_roots: KagemushaHistoryRootsV1,
        external_successor_roots: KagemushaHistoryRootsV1,
        pasta_predecessor_replay_root: KagemushaPastaStateCommitmentV1,
        pasta_successor_replay_root: KagemushaPastaStateCommitmentV1,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        if transaction
            .root_selection()
            .apply_to(external_predecessor_roots)?
            != external_successor_roots
            || transaction
                .root_selection()
                .for_tree(KagemushaHistoryTreeV1::Replay)
                .is_none()
            || digest_is_zero(operation_binding_digest)
            || pasta_predecessor_replay_root.is_zero()
            || pasta_successor_replay_root.is_zero()
            || pasta_predecessor_replay_root == pasta_successor_replay_root
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidProofRootBridge);
        }
        Ok(Self {
            transaction_id: transaction.transaction_id(),
            operation_binding_digest,
            external_predecessor_roots,
            external_successor_roots,
            pasta_predecessor_replay_root,
            pasta_successor_replay_root,
        })
    }

    /// Return the prepared external CAS identity.
    pub(crate) const fn transaction_id(self) -> DigestV1 {
        self.transaction_id
    }

    /// Return the logical replay/decision operation jointly bound by proof and hardware CAS.
    pub(crate) const fn operation_binding_digest(self) -> DigestV1 {
        self.operation_binding_digest
    }

    /// Return the canonical digest committed by the verified state transition.
    pub(crate) fn canonical_digest(self) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
        digest_canonical(PROOF_ROOT_BRIDGE_DOMAIN_V1, &self)
    }

    /// Return the exact external roots before the prepared CAS.
    pub(crate) const fn external_predecessor_roots(self) -> KagemushaHistoryRootsV1 {
        self.external_predecessor_roots
    }

    /// Return the exact external roots selected by the prepared CAS.
    pub(crate) const fn external_successor_roots(self) -> KagemushaHistoryRootsV1 {
        self.external_successor_roots
    }

    /// Return the paired Pasta replay root consumed by the recursive transition.
    pub(crate) const fn pasta_predecessor_replay_root(self) -> KagemushaPastaStateCommitmentV1 {
        self.pasta_predecessor_replay_root
    }

    /// Return the paired Pasta replay root produced by the recursive transition.
    pub(crate) const fn pasta_successor_replay_root(self) -> KagemushaPastaStateCommitmentV1 {
        self.pasta_successor_replay_root
    }
}

/// Opaque proof/hardware bridge capability minted only after the state proof and hardware root
/// selection authenticate the same logical history operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedKagemushaHistoryProofRootBridgeV1 {
    request: KagemushaHistoryProofRootBridgeRequestV1,
}

impl VerifiedKagemushaHistoryProofRootBridgeV1 {
    /// Return the exact request authenticated by this capability.
    pub(crate) const fn request(self) -> KagemushaHistoryProofRootBridgeRequestV1 {
        self.request
    }
}

/// Typed state-proof integration failure for the cross-commitment bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum KagemushaHistoryProofRootBridgeErrorV1 {
    /// The state proof and hardware-selected transaction do not bind the same logical operation.
    #[error("KAGEMUSHA authenticated external-history proof-root bridge does not match")]
    BindingMismatch {
        /// Exact roots and logical operation which failed authenticated reconciliation.
        request: KagemushaHistoryProofRootBridgeRequestV1,
    },
}

/// Reconcile independently encoded SHA-256 and Pasta histories through one authenticated logical
/// operation.
///
/// The caller must invoke this only after verifying (1) the recursive transition which commits
/// `authenticated_operation_binding_digest` and the Pasta predecessor/successor roots, and (2)
/// the qualified-hardware certificate selecting `request.transaction_id`. This function never
/// casts one root representation into the other.
pub(crate) fn require_history_proof_root_bridge_v1(
    request: KagemushaHistoryProofRootBridgeRequestV1,
    authenticated_operation_binding_digest: DigestV1,
) -> Result<VerifiedKagemushaHistoryProofRootBridgeV1, KagemushaHistoryProofRootBridgeErrorV1> {
    if authenticated_operation_binding_digest == [0; 32]
        || request.operation_binding_digest() != authenticated_operation_binding_digest
        || request.canonical_digest().is_err()
    {
        return Err(KagemushaHistoryProofRootBridgeErrorV1::BindingMismatch { request });
    }
    Ok(VerifiedKagemushaHistoryProofRootBridgeV1 { request })
}

/// Classify one candidate key/value pair against a complete authenticated committed tree.
///
/// This read never treats an unavailable or corrupt path as absence. Recovery and admission fail
/// closed until the exact committed root can be traversed again.
pub(crate) fn classify_history_identity_v1<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    key: DigestV1,
    candidate_value_digest: DigestV1,
) -> Result<KagemushaHistoryIdentityClassificationV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    if digest_is_zero(key) || digest_is_zero(candidate_value_digest) {
        return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
    }
    let root = store.committed_roots().for_tree(tree);
    validate_tree_from_store(store, tree, root)?;
    Ok(match lookup_value_digest(store, tree, root, key)? {
        None => KagemushaHistoryIdentityClassificationV1::Absent,
        Some(existing) if existing == candidate_value_digest => {
            KagemushaHistoryIdentityClassificationV1::ExactDuplicate
        }
        Some(existing_value_digest) => KagemushaHistoryIdentityClassificationV1::Conflict {
            existing_value_digest,
        },
    })
}

/// Build and durably prepare one immutable authenticated-map insertion.
///
/// The committed tree is validated before the live overlay is touched. Repeating an already
/// committed identity returns exact duplicate/conflict classification without consuming WAL
/// capacity. Only a genuinely absent identity can consume the byte-bounded prepared overlay.
pub(crate) fn prepare_history_identity_insert_v1<S>(
    store: &mut S,
    tree: KagemushaHistoryTreeV1,
    key: DigestV1,
    value_digest: DigestV1,
    attempt_binding_digest: DigestV1,
) -> Result<KagemushaHistoryInsertPreparationV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    if digest_is_zero(attempt_binding_digest) {
        return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
    }
    if digest_is_zero(key) || digest_is_zero(value_digest) {
        return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
    }
    let expected_root = store.committed_roots().for_tree(tree);
    validate_tree_from_store(store, tree, expected_root)?;
    let mut nodes = BTreeMap::new();
    let selected_root =
        match build_inserted_root(store, tree, expected_root, key, value_digest, &mut nodes)? {
            KagemushaHistoryInsertBuildV1::Inserted(root) => root,
            KagemushaHistoryInsertBuildV1::ExactDuplicate => {
                return Ok(KagemushaHistoryInsertPreparationV1::ExactDuplicate);
            }
            KagemushaHistoryInsertBuildV1::Conflict {
                existing_value_digest,
            } => {
                return Ok(KagemushaHistoryInsertPreparationV1::Conflict {
                    existing_value_digest,
                });
            }
        };
    let selection = match tree {
        KagemushaHistoryTreeV1::Replay => {
            KagemushaHistoryRootSelectionV1::replay(expected_root, selected_root)
        }
        KagemushaHistoryTreeV1::TerminalDecision => {
            KagemushaHistoryRootSelectionV1::terminal_decision(expected_root, selected_root)
        }
    };
    let transaction = KagemushaPreparedHistoryCasV1::new(
        selection,
        nodes.into_values().collect(),
        attempt_binding_digest,
    )?;
    let outcome = store.prepare_cas(transaction.clone())?;
    Ok(KagemushaHistoryInsertPreparationV1::Prepared {
        transaction,
        outcome,
    })
}

/// Prepare replay and terminal-decision insertions under one hardware-selected transaction.
///
/// If one binding already committed byte-identically, the transaction selects only the still
/// absent root. If both already committed, no WAL bytes are consumed. Any conflict aborts before
/// either root or the live overlay changes.
pub(crate) fn prepare_history_identity_pair_v1<S>(
    store: &mut S,
    replay_key: DigestV1,
    replay_value_digest: DigestV1,
    terminal_decision_key: DigestV1,
    terminal_decision_value_digest: DigestV1,
    attempt_binding_digest: DigestV1,
) -> Result<KagemushaHistoryDualInsertPreparationV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    if digest_is_zero(attempt_binding_digest) {
        return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
    }
    if [
        replay_key,
        replay_value_digest,
        terminal_decision_key,
        terminal_decision_value_digest,
    ]
    .into_iter()
    .any(digest_is_zero)
    {
        return Err(KagemushaHistoryStoreErrorV1::InvalidNode);
    }
    let predecessor = validate_committed_history_v1(store)?;
    let mut replay_nodes = BTreeMap::new();
    let replay = build_inserted_root(
        store,
        KagemushaHistoryTreeV1::Replay,
        predecessor.replay(),
        replay_key,
        replay_value_digest,
        &mut replay_nodes,
    )?;
    if let KagemushaHistoryInsertBuildV1::Conflict {
        existing_value_digest,
    } = &replay
    {
        return Ok(KagemushaHistoryDualInsertPreparationV1::Conflict {
            tree: KagemushaHistoryTreeV1::Replay,
            key: replay_key,
            existing_value_digest: *existing_value_digest,
        });
    }

    let mut decision_nodes = BTreeMap::new();
    let decision = build_inserted_root(
        store,
        KagemushaHistoryTreeV1::TerminalDecision,
        predecessor.terminal_decision(),
        terminal_decision_key,
        terminal_decision_value_digest,
        &mut decision_nodes,
    )?;
    if let KagemushaHistoryInsertBuildV1::Conflict {
        existing_value_digest,
    } = &decision
    {
        return Ok(KagemushaHistoryDualInsertPreparationV1::Conflict {
            tree: KagemushaHistoryTreeV1::TerminalDecision,
            key: terminal_decision_key,
            existing_value_digest: *existing_value_digest,
        });
    }

    let root_selection = match (replay, decision) {
        (
            KagemushaHistoryInsertBuildV1::Inserted(replay_selected),
            KagemushaHistoryInsertBuildV1::Inserted(decision_selected),
        ) => KagemushaHistoryRootSelectionV1::both(
            KagemushaHistoryRootCasV1::new(predecessor.replay(), replay_selected),
            KagemushaHistoryRootCasV1::new(predecessor.terminal_decision(), decision_selected),
        ),
        (
            KagemushaHistoryInsertBuildV1::Inserted(replay_selected),
            KagemushaHistoryInsertBuildV1::ExactDuplicate,
        ) => KagemushaHistoryRootSelectionV1::replay(predecessor.replay(), replay_selected),
        (
            KagemushaHistoryInsertBuildV1::ExactDuplicate,
            KagemushaHistoryInsertBuildV1::Inserted(decision_selected),
        ) => KagemushaHistoryRootSelectionV1::terminal_decision(
            predecessor.terminal_decision(),
            decision_selected,
        ),
        (
            KagemushaHistoryInsertBuildV1::ExactDuplicate,
            KagemushaHistoryInsertBuildV1::ExactDuplicate,
        ) => return Ok(KagemushaHistoryDualInsertPreparationV1::ExactDuplicate),
        (KagemushaHistoryInsertBuildV1::Conflict { .. }, _)
        | (_, KagemushaHistoryInsertBuildV1::Conflict { .. }) => {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
    };
    for (address, node) in decision_nodes {
        if replay_nodes
            .insert(address, node.clone())
            .is_some_and(|existing| existing != node)
        {
            return Err(KagemushaHistoryStoreErrorV1::ContentAddressCollision(
                address,
            ));
        }
    }
    let transaction = KagemushaPreparedHistoryCasV1::new(
        root_selection,
        replay_nodes.into_values().collect(),
        attempt_binding_digest,
    )?;
    let outcome = store.prepare_cas(transaction.clone())?;
    Ok(KagemushaHistoryDualInsertPreparationV1::Prepared {
        transaction,
        outcome,
    })
}

enum KagemushaHistoryInsertBuildV1 {
    Inserted(DigestV1),
    ExactDuplicate,
    Conflict { existing_value_digest: DigestV1 },
}

fn build_inserted_root<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    current: DigestV1,
    key: DigestV1,
    value_digest: DigestV1,
    new_nodes: &mut BTreeMap<DigestV1, KagemushaHistoryNodeRecordV1>,
) -> Result<KagemushaHistoryInsertBuildV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    if current == empty_root(tree) {
        let leaf = KagemushaHistoryNodeRecordV1::leaf(tree, key, value_digest)?;
        let root = insert_overlay_node(new_nodes, leaf)?;
        return Ok(KagemushaHistoryInsertBuildV1::Inserted(root));
    }
    let node = load_verified_node_with_overlay(store, tree, current, new_nodes)?;
    match node.body() {
        KagemushaHistoryNodeBodyV1::Leaf {
            key: existing_key,
            value_digest: existing_value_digest,
        } => {
            if *existing_key == key {
                return Ok(if *existing_value_digest == value_digest {
                    KagemushaHistoryInsertBuildV1::ExactDuplicate
                } else {
                    KagemushaHistoryInsertBuildV1::Conflict {
                        existing_value_digest: *existing_value_digest,
                    }
                });
            }
            let leaf = KagemushaHistoryNodeRecordV1::leaf(tree, key, value_digest)?;
            let leaf_address = insert_overlay_node(new_nodes, leaf)?;
            let branch_depth = common_prefix_bits(key, *existing_key);
            let (left, right) = if key_bit(key, branch_depth) {
                (current, leaf_address)
            } else {
                (leaf_address, current)
            };
            let branch = KagemushaHistoryNodeRecordV1::branch(
                tree,
                u16::try_from(branch_depth).map_err(|_| {
                    KagemushaHistoryStoreErrorV1::InvalidHistoryTree {
                        tree,
                        root: current,
                    }
                })?,
                canonical_prefix(key, branch_depth),
                left,
                right,
            )?;
            let root = insert_overlay_node(new_nodes, branch)?;
            Ok(KagemushaHistoryInsertBuildV1::Inserted(root))
        }
        KagemushaHistoryNodeBodyV1::Branch {
            depth,
            prefix,
            left,
            right,
        } => {
            let depth_u16 = *depth;
            let depth = usize::from(depth_u16);
            if canonical_prefix(key, depth) != *prefix {
                let branch_depth = common_prefix_bits(key, *prefix);
                if branch_depth >= depth {
                    return Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree {
                        tree,
                        root: current,
                    });
                }
                let leaf = KagemushaHistoryNodeRecordV1::leaf(tree, key, value_digest)?;
                let leaf_address = insert_overlay_node(new_nodes, leaf)?;
                let (new_left, new_right) = if key_bit(key, branch_depth) {
                    (current, leaf_address)
                } else {
                    (leaf_address, current)
                };
                let branch = KagemushaHistoryNodeRecordV1::branch(
                    tree,
                    u16::try_from(branch_depth).map_err(|_| {
                        KagemushaHistoryStoreErrorV1::InvalidHistoryTree {
                            tree,
                            root: current,
                        }
                    })?,
                    canonical_prefix(key, branch_depth),
                    new_left,
                    new_right,
                )?;
                let root = insert_overlay_node(new_nodes, branch)?;
                return Ok(KagemushaHistoryInsertBuildV1::Inserted(root));
            }

            let go_right = key_bit(key, depth);
            let child = if go_right { *right } else { *left };
            let successor_child =
                match build_inserted_root(store, tree, child, key, value_digest, new_nodes)? {
                    KagemushaHistoryInsertBuildV1::Inserted(successor_child) => successor_child,
                    duplicate_or_conflict => return Ok(duplicate_or_conflict),
                };
            let successor = KagemushaHistoryNodeRecordV1::branch(
                tree,
                depth_u16,
                *prefix,
                if go_right { *left } else { successor_child },
                if go_right { successor_child } else { *right },
            )?;
            let root = insert_overlay_node(new_nodes, successor)?;
            Ok(KagemushaHistoryInsertBuildV1::Inserted(root))
        }
    }
}

fn insert_overlay_node(
    overlay: &mut BTreeMap<DigestV1, KagemushaHistoryNodeRecordV1>,
    node: KagemushaHistoryNodeRecordV1,
) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
    let address = node.content_address()?;
    if let Some(existing) = overlay.get(&address) {
        return if existing == &node {
            Ok(address)
        } else {
            Err(KagemushaHistoryStoreErrorV1::ContentAddressCollision(
                address,
            ))
        };
    }
    overlay.insert(address, node);
    Ok(address)
}

fn load_verified_node_with_overlay<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    address: DigestV1,
    overlay: &BTreeMap<DigestV1, KagemushaHistoryNodeRecordV1>,
) -> Result<KagemushaHistoryNodeRecordV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    if let Some(node) = overlay.get(&address) {
        if node.tree() != tree || node.content_address().ok() != Some(address) {
            return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode { tree, address });
        }
        return Ok(node.clone());
    }
    load_verified_node(store, tree, address)
}

fn lookup_value_digest<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    mut address: DigestV1,
    key: DigestV1,
) -> Result<Option<DigestV1>, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    while address != empty_root(tree) {
        let node = load_verified_node(store, tree, address)?;
        match node.body() {
            KagemushaHistoryNodeBodyV1::Leaf {
                key: existing_key,
                value_digest,
            } => return Ok((*existing_key == key).then_some(*value_digest)),
            KagemushaHistoryNodeBodyV1::Branch {
                depth,
                prefix,
                left,
                right,
            } => {
                let depth = usize::from(*depth);
                if canonical_prefix(key, depth) != *prefix {
                    return Ok(None);
                }
                address = if key_bit(key, depth) { *right } else { *left };
            }
        }
    }
    Ok(None)
}

fn load_verified_node<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    address: DigestV1,
) -> Result<KagemushaHistoryNodeRecordV1, KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    let node = store
        .read_node(address)?
        .ok_or(KagemushaHistoryStoreErrorV1::MissingHistoryNode { tree, address })?;
    if node.tree() != tree || node.content_address().ok() != Some(address) {
        return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode { tree, address });
    }
    Ok(node)
}

fn validate_tree_from_store<S>(
    store: &S,
    tree: KagemushaHistoryTreeV1,
    root: DigestV1,
) -> Result<(), KagemushaHistoryStoreErrorV1>
where
    S: KagemushaAuthenticatedHistoryStoreV1 + ?Sized,
{
    store.validate_tree(tree, root)
}

// A summary certifies the entire immutable subtree, not just its root record. Its root
// shape must still satisfy every new incoming edge; a valid subtree cannot be moved across
// prefixes/namespaces or attached above a non-increasing branch depth.
#[derive(Clone, Copy)]
struct ValidatedHistorySubtree {
    tree: KagemushaHistoryTreeV1,
    representative: DigestV1,
    branch_depth: Option<usize>,
}

#[derive(Clone, Copy)]
struct HistoryParentEdge {
    depth: usize,
    prefix: DigestV1,
    right: bool,
}

fn validate_subtree_edge(
    tree: KagemushaHistoryTreeV1,
    root: DigestV1,
    address: DigestV1,
    summary: ValidatedHistorySubtree,
    parent: Option<HistoryParentEdge>,
) -> Result<(), KagemushaHistoryStoreErrorV1> {
    if summary.tree != tree {
        return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode { tree, address });
    }
    if let Some(parent) = parent {
        if canonical_prefix(summary.representative, parent.depth) != parent.prefix
            || key_bit(summary.representative, parent.depth) != parent.right
            || summary
                .branch_depth
                .is_some_and(|depth| depth <= parent.depth)
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree { tree, root });
        }
    }
    Ok(())
}

fn validate_tree_with_lookup<F>(
    tree: KagemushaHistoryTreeV1,
    root: DigestV1,
    lookup: F,
) -> Result<(), KagemushaHistoryStoreErrorV1>
where
    F: FnMut(
        DigestV1,
    ) -> Result<Option<KagemushaHistoryNodeRecordV1>, KagemushaHistoryStoreErrorV1>,
{
    validate_tree_with_immutable_subtrees(tree, root, lookup, |_| None).map(|_| ())
}

fn validate_tree_with_immutable_subtrees<F, C>(
    tree: KagemushaHistoryTreeV1,
    root: DigestV1,
    mut lookup: F,
    mut cached: C,
) -> Result<BTreeMap<DigestV1, ValidatedHistorySubtree>, KagemushaHistoryStoreErrorV1>
where
    F: FnMut(
        DigestV1,
    ) -> Result<Option<KagemushaHistoryNodeRecordV1>, KagemushaHistoryStoreErrorV1>,
    C: FnMut(DigestV1) -> Option<ValidatedHistorySubtree>,
{
    let mut newly_validated = BTreeMap::new();
    if root == empty_root(tree) {
        return Ok(newly_validated);
    }
    if digest_is_zero(root) {
        return Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree { tree, root });
    }
    enum Work {
        Enter(DigestV1, Option<HistoryParentEdge>),
        Complete(DigestV1, ValidatedHistorySubtree),
    }
    let mut visited = BTreeSet::new();
    let mut stack = vec![Work::Enter(root, None)];
    while let Some(work) = stack.pop() {
        let (address, parent) = match work {
            Work::Complete(address, summary) => {
                // Published locally only after every child passed. The caller publishes this
                // delta to the retained index only when a Commit becomes durable.
                newly_validated.insert(address, summary);
                continue;
            }
            Work::Enter(address, parent) => (address, parent),
        };
        if address == empty_root(tree) {
            continue;
        }
        if !visited.insert(address) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree { tree, root });
        }
        if let Some(summary) = cached(address) {
            validate_subtree_edge(tree, root, address, summary, parent)?;
            // Validated subtree descendants all obey this root's prefix. Distinct branches
            // impose disjoint key prefixes, so their cached descendants cannot overlap while
            // both incoming-edge checks succeed. New nodes retain the explicit visited guard.
            continue;
        }
        let node = lookup(address)?
            .ok_or(KagemushaHistoryStoreErrorV1::MissingHistoryNode { tree, address })?;
        if node.tree() != tree || node.content_address().ok() != Some(address) {
            return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode { tree, address });
        }
        let summary = match node.body() {
            KagemushaHistoryNodeBodyV1::Leaf { key, .. } => ValidatedHistorySubtree {
                tree,
                representative: *key,
                branch_depth: None,
            },
            KagemushaHistoryNodeBodyV1::Branch { depth, prefix, .. } => ValidatedHistorySubtree {
                tree,
                representative: *prefix,
                branch_depth: Some(usize::from(*depth)),
            },
        };
        validate_subtree_edge(tree, root, address, summary, parent)?;
        stack.push(Work::Complete(address, summary));
        if let KagemushaHistoryNodeBodyV1::Branch {
            depth,
            prefix,
            left,
            right,
        } = node.body()
        {
            if left == right {
                return Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree { tree, root });
            }
            let depth = usize::from(*depth);
            stack.push(Work::Enter(
                *right,
                Some(HistoryParentEdge {
                    depth,
                    prefix: *prefix,
                    right: true,
                }),
            ));
            stack.push(Work::Enter(
                *left,
                Some(HistoryParentEdge {
                    depth,
                    prefix: *prefix,
                    right: false,
                }),
            ));
        }
    }
    Ok(newly_validated)
}

// The sole production mutation adds immutable records and their fully validated subtree delta
// after commit. Fault injection invalidates every cached subtree before replacing/removing data.
// No cache is serialized, loaded from disk, or retained across another process's storage handle.
#[derive(Clone, Default)]
struct ImmutableHistoryNodeIndex {
    nodes: BTreeMap<DigestV1, KagemushaHistoryNodeRecordV1>,
    subtrees: BTreeMap<DigestV1, ValidatedHistorySubtree>,
    #[cfg(test)]
    validation_visits: std::cell::Cell<u64>,
}

impl ImmutableHistoryNodeIndex {
    fn get(&self, address: &DigestV1) -> Option<&KagemushaHistoryNodeRecordV1> {
        self.nodes.get(address)
    }

    fn validated_subtree(&self, address: DigestV1) -> Option<ValidatedHistorySubtree> {
        self.subtrees.get(&address).copied()
    }

    fn record_validation_visit(&self) {
        #[cfg(test)]
        self.validation_visits.set(self.validation_visits.get() + 1);
    }

    fn install_committed(
        &mut self,
        writes: Vec<KagemushaHistoryNodeWriteV1>,
        subtrees: BTreeMap<DigestV1, ValidatedHistorySubtree>,
    ) {
        for write in writes {
            self.nodes.entry(write.address).or_insert(write.node);
        }
        self.subtrees.extend(subtrees);
    }

    #[cfg(test)]
    fn remove(&mut self, address: &DigestV1) -> Option<KagemushaHistoryNodeRecordV1> {
        self.subtrees.clear();
        self.nodes.remove(address)
    }

    #[cfg(test)]
    fn insert(&mut self, address: DigestV1, node: KagemushaHistoryNodeRecordV1) {
        self.subtrees.clear();
        self.nodes.insert(address, node);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaLiveHistoryWalEntryV1 {
    transaction: KagemushaPreparedHistoryCasV1,
    wal_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KagemushaTerminalHistoryCasV1 {
    Committed {
        certificate: Box<KagemushaHistoryRootSelectionCertificateV1>,
        root_selection: KagemushaHistoryRootSelectionV1,
        committed_roots: KagemushaHistoryRootsV1,
    },
    Aborted {
        root_selection: KagemushaHistoryRootSelectionV1,
    },
}

/// Deterministic in-memory reference implementation of the external persistence contract.
///
/// This implementation models the required atomic boundaries and failure semantics. Production
/// integrations can place the same immutable records, prepared entries, terminal records, and root
/// heads in an external transactional store without changing the protocol-facing interface.
#[derive(Clone)]
pub(crate) struct KagemushaMemoryAuthenticatedHistoryStoreV1 {
    roots: KagemushaHistoryRootsV1,
    durable_nodes: ImmutableHistoryNodeIndex,
    prepared: BTreeMap<DigestV1, KagemushaLiveHistoryWalEntryV1>,
    terminal: BTreeMap<DigestV1, KagemushaTerminalHistoryCasV1>,
    live_overlay_bytes: u64,
    overlay_capacity_bytes: u64,
    object_store_available: bool,
    recovery_commitment: DigestV1,
    recoverable_checkpoints: BTreeSet<DigestV1>,
}

impl KagemushaMemoryAuthenticatedHistoryStoreV1 {
    /// Construct an empty reference store with a byte cap for uncommitted WAL entries only.
    pub(crate) fn new(overlay_capacity_bytes: u64) -> Self {
        Self {
            roots: KagemushaHistoryRootsV1::empty(),
            durable_nodes: ImmutableHistoryNodeIndex::default(),
            prepared: BTreeMap::new(),
            terminal: BTreeMap::new(),
            live_overlay_bytes: 0,
            overlay_capacity_bytes,
            object_store_available: true,
            recovery_commitment: Sha256::digest(b"iroha:kagemusha:v1:history-recovery:empty\0")
                .into(),
            recoverable_checkpoints: BTreeSet::from([Sha256::digest(
                b"iroha:kagemusha:v1:history-recovery:empty\0",
            )
            .into()]),
        }
    }

    #[cfg(test)]
    fn set_object_store_available_for_test(&mut self, available: bool) {
        self.object_store_available = available;
    }

    fn terminal_prepare_outcome(
        terminal: KagemushaTerminalHistoryCasV1,
        root_selection: KagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryPrepareOutcomeV1, KagemushaHistoryStoreErrorV1> {
        match terminal {
            KagemushaTerminalHistoryCasV1::Committed {
                root_selection: terminal_selection,
                committed_roots,
                ..
            } => {
                if terminal_selection != root_selection {
                    return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
                }
                Ok(KagemushaHistoryPrepareOutcomeV1::AlreadyCommitted { committed_roots })
            }
            KagemushaTerminalHistoryCasV1::Aborted {
                root_selection: terminal_selection,
            } => {
                if terminal_selection != root_selection {
                    return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
                }
                Ok(KagemushaHistoryPrepareOutcomeV1::AlreadyAborted)
            }
        }
    }

    fn terminal_commit_outcome(
        terminal: KagemushaTerminalHistoryCasV1,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryCommitOutcomeV1, KagemushaHistoryStoreErrorV1> {
        let root_selection = certificate.root_selection();
        match terminal {
            KagemushaTerminalHistoryCasV1::Committed {
                root_selection: terminal_selection,
                committed_roots,
                certificate: original_certificate,
            } => {
                if terminal_selection != root_selection
                    || *original_certificate != certificate.certificate
                {
                    return Err(KagemushaHistoryStoreErrorV1::CertificateMismatch);
                }
                Ok(KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots })
            }
            KagemushaTerminalHistoryCasV1::Aborted {
                root_selection: terminal_selection,
            } => {
                if terminal_selection != root_selection {
                    return Err(KagemushaHistoryStoreErrorV1::CertificateMismatch);
                }
                Ok(KagemushaHistoryCommitOutcomeV1::Aborted)
            }
        }
    }

    fn validate_selected_root_nodes(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<BTreeMap<DigestV1, ValidatedHistorySubtree>, KagemushaHistoryStoreErrorV1> {
        let mut validated = BTreeMap::new();
        for tree in [
            KagemushaHistoryTreeV1::Replay,
            KagemushaHistoryTreeV1::TerminalDecision,
        ] {
            let Some(cas) = transaction.root_selection.for_tree(tree) else {
                continue;
            };
            let overlay = |address| {
                transaction
                    .node_writes
                    .binary_search_by_key(&address, |write| write.address)
                    .ok()
                    .and_then(|index| transaction.node_writes.get(index))
            };
            let delta = validate_tree_with_immutable_subtrees(
                tree,
                cas.selected(),
                |address| {
                    Ok(overlay(address)
                        .map(|write| write.node.clone())
                        .or_else(|| self.durable_nodes.get(&address).cloned()))
                },
                // An overlay record must validate on its own, even if a retained record has
                // the same address. The separate collision check still compares exact bytes.
                |address| {
                    // This callback runs for every nonempty Work::Enter, including overlay
                    // nodes. Count total traversal work rather than only cache lookups.
                    self.durable_nodes.record_validation_visit();
                    if overlay(address).is_some() {
                        None
                    } else {
                        self.durable_nodes.validated_subtree(address)
                    }
                },
            )
            .map_err(|error| match error {
                KagemushaHistoryStoreErrorV1::MissingHistoryNode { address, .. }
                    if address == cas.selected() =>
                {
                    KagemushaHistoryStoreErrorV1::MissingSelectedRoot {
                        tree,
                        root: cas.selected(),
                    }
                }
                other => other,
            })?;
            validated.extend(delta);
        }
        Ok(validated)
    }

    fn validate_node_collisions(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        for write in &transaction.node_writes {
            if self
                .durable_nodes
                .get(&write.address)
                .is_some_and(|existing| existing != &write.node)
            {
                return Err(KagemushaHistoryStoreErrorV1::ContentAddressCollision(
                    write.address,
                ));
            }
        }
        Ok(())
    }

    fn available_overlay_bytes(&self) -> u64 {
        self.overlay_capacity_bytes
            .saturating_sub(self.live_overlay_bytes)
    }
}

// Plans retain only the changing overlay entry, not a clone of committed history. The disk store
// persists a plan's authenticated journal operation before applying its infallible state delta.
struct HistoryMutationPlan<T> {
    outcome: T,
    mutation: Option<HistoryMutation>,
    next_recovery_commitment: Option<DigestV1>,
}

impl<T> HistoryMutationPlan<T> {
    const fn unchanged(outcome: T) -> Self {
        Self {
            outcome,
            mutation: None,
            next_recovery_commitment: None,
        }
    }
}

#[derive(Encode)]
enum HistoryRecoveryOperationV1 {
    Prepared(DigestV1),
    Committed(KagemushaHistoryRootSelectionCertificateV1),
    Aborted(DigestV1),
}

fn next_history_recovery_commitment(
    previous: DigestV1,
    operation: HistoryRecoveryOperationV1,
) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
    digest_canonical(
        b"iroha:kagemusha:v1:history-recovery:operation\0",
        &(previous, operation),
    )
}

enum HistoryMutation {
    Prepare {
        entry: KagemushaLiveHistoryWalEntryV1,
        next_live_bytes: u64,
    },
    Commit {
        certificate: KagemushaHistoryRootSelectionCertificateV1,
        entry: KagemushaLiveHistoryWalEntryV1,
        committed_roots: KagemushaHistoryRootsV1,
        next_live_bytes: u64,
        validated_subtrees: BTreeMap<DigestV1, ValidatedHistorySubtree>,
    },
    Abort {
        transaction_id: DigestV1,
        root_selection: KagemushaHistoryRootSelectionV1,
        next_live_bytes: u64,
    },
}

impl KagemushaMemoryAuthenticatedHistoryStoreV1 {
    fn plan_prepare_cas(
        &self,
        transaction: KagemushaPreparedHistoryCasV1,
    ) -> Result<HistoryMutationPlan<KagemushaHistoryPrepareOutcomeV1>, KagemushaHistoryStoreErrorV1>
    {
        transaction.validate()?;
        let transaction_id = transaction.transaction_id();
        let root_selection = transaction.root_selection();
        if let Some(terminal) = self.terminal.get(&transaction_id).cloned() {
            return Self::terminal_prepare_outcome(terminal, root_selection)
                .map(HistoryMutationPlan::unchanged);
        }
        if let Some(existing) = self.prepared.get(&transaction_id) {
            return if existing.transaction == transaction {
                Ok(HistoryMutationPlan::unchanged(
                    KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared,
                ))
            } else {
                Err(KagemushaHistoryStoreErrorV1::InvalidTransaction)
            };
        }
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        root_selection.apply_to(self.roots)?;
        self.validate_selected_root_nodes(&transaction)?;
        self.validate_node_collisions(&transaction)?;

        let wal_bytes = transaction.wal_bytes()?;
        let available_bytes = self.available_overlay_bytes();
        if wal_bytes > available_bytes {
            return Err(KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded {
                required_bytes: wal_bytes,
                available_bytes,
            });
        }
        let next_live_bytes = self.live_overlay_bytes.checked_add(wal_bytes).ok_or(
            KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded {
                required_bytes: wal_bytes,
                available_bytes,
            },
        )?;
        Ok(HistoryMutationPlan {
            outcome: KagemushaHistoryPrepareOutcomeV1::Prepared,
            next_recovery_commitment: Some(next_history_recovery_commitment(
                self.recovery_commitment,
                HistoryRecoveryOperationV1::Prepared(transaction_id),
            )?),
            mutation: Some(HistoryMutation::Prepare {
                entry: KagemushaLiveHistoryWalEntryV1 {
                    transaction,
                    wal_bytes,
                },
                next_live_bytes,
            }),
        })
    }

    fn plan_commit_prepared(
        &self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<HistoryMutationPlan<KagemushaHistoryCommitOutcomeV1>, KagemushaHistoryStoreErrorV1>
    {
        let transaction_id = certificate.transaction_id();
        let root_selection = certificate.root_selection();
        if let Some(terminal) = self.terminal.get(&transaction_id).cloned() {
            return Self::terminal_commit_outcome(terminal, certificate)
                .map(HistoryMutationPlan::unchanged);
        }
        let entry = self.prepared.get(&transaction_id).cloned().ok_or(
            KagemushaHistoryStoreErrorV1::UnknownTransaction(transaction_id),
        )?;
        if entry.transaction.root_selection() != root_selection {
            return Err(KagemushaHistoryStoreErrorV1::CertificateMismatch);
        }
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }

        let committed_roots = root_selection.apply_to(self.roots)?;
        let validated_subtrees = self.validate_selected_root_nodes(&entry.transaction)?;
        self.validate_node_collisions(&entry.transaction)?;
        let next_live_bytes = self
            .live_overlay_bytes
            .checked_sub(entry.wal_bytes)
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidTransaction)?;

        Ok(HistoryMutationPlan {
            outcome: KagemushaHistoryCommitOutcomeV1::Committed { committed_roots },
            next_recovery_commitment: Some(next_history_recovery_commitment(
                self.recovery_commitment,
                HistoryRecoveryOperationV1::Committed(certificate.certificate),
            )?),
            mutation: Some(HistoryMutation::Commit {
                certificate: certificate.certificate,
                entry,
                committed_roots,
                next_live_bytes,
                validated_subtrees,
            }),
        })
    }

    fn plan_abort_prepared(
        &self,
        transaction_id: DigestV1,
    ) -> Result<HistoryMutationPlan<KagemushaHistoryAbortOutcomeV1>, KagemushaHistoryStoreErrorV1>
    {
        if let Some(terminal) = self.terminal.get(&transaction_id).cloned() {
            return Ok(HistoryMutationPlan::unchanged(match terminal {
                KagemushaTerminalHistoryCasV1::Committed {
                    committed_roots, ..
                } => KagemushaHistoryAbortOutcomeV1::AlreadyCommitted { committed_roots },
                KagemushaTerminalHistoryCasV1::Aborted { .. } => {
                    KagemushaHistoryAbortOutcomeV1::AlreadyAborted
                }
            }));
        }
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        let entry = self.prepared.get(&transaction_id).cloned().ok_or(
            KagemushaHistoryStoreErrorV1::UnknownTransaction(transaction_id),
        )?;
        let next_live_bytes = self
            .live_overlay_bytes
            .checked_sub(entry.wal_bytes)
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidTransaction)?;
        Ok(HistoryMutationPlan {
            outcome: KagemushaHistoryAbortOutcomeV1::Aborted,
            next_recovery_commitment: Some(next_history_recovery_commitment(
                self.recovery_commitment,
                HistoryRecoveryOperationV1::Aborted(transaction_id),
            )?),
            mutation: Some(HistoryMutation::Abort {
                transaction_id,
                root_selection: entry.transaction.root_selection(),
                next_live_bytes,
            }),
        })
    }

    fn apply_plan<T>(&mut self, plan: HistoryMutationPlan<T>) -> T {
        match plan.mutation {
            None => {}
            Some(HistoryMutation::Prepare {
                entry,
                next_live_bytes,
            }) => {
                self.prepared
                    .insert(entry.transaction.transaction_id(), entry);
                self.live_overlay_bytes = next_live_bytes;
            }
            Some(HistoryMutation::Commit {
                certificate,
                entry,
                committed_roots,
                next_live_bytes,
                validated_subtrees,
            }) => {
                let transaction_id = entry.transaction.transaction_id();
                let root_selection = entry.transaction.root_selection();
                self.durable_nodes
                    .install_committed(entry.transaction.node_writes, validated_subtrees);
                self.roots = committed_roots;
                // An older anchor cannot authorize a suffix that selected new money-history
                // roots. Only subsequent non-monetary prepare/abort suffixes are recoverable.
                self.recoverable_checkpoints.clear();
                self.prepared.remove(&transaction_id);
                self.live_overlay_bytes = next_live_bytes;
                self.terminal.insert(
                    transaction_id,
                    KagemushaTerminalHistoryCasV1::Committed {
                        certificate: Box::new(certificate),
                        root_selection,
                        committed_roots,
                    },
                );
            }
            Some(HistoryMutation::Abort {
                transaction_id,
                root_selection,
                next_live_bytes,
            }) => {
                self.prepared.remove(&transaction_id);
                self.live_overlay_bytes = next_live_bytes;
                self.terminal.insert(
                    transaction_id,
                    KagemushaTerminalHistoryCasV1::Aborted { root_selection },
                );
            }
        }
        if let Some(commitment) = plan.next_recovery_commitment {
            self.recovery_commitment = commitment;
            self.recoverable_checkpoints.insert(commitment);
        }
        plan.outcome
    }
}

impl KagemushaAuthenticatedHistoryStoreV1 for KagemushaMemoryAuthenticatedHistoryStoreV1 {
    fn committed_roots(&self) -> KagemushaHistoryRootsV1 {
        self.roots
    }

    fn recovery_commitment(&self) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        Ok(self.recovery_commitment)
    }

    fn validate_recovery_checkpoint(
        &self,
        expected: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        if !self.recoverable_checkpoints.contains(&expected) {
            return Err(KagemushaHistoryStoreErrorV1::RecoveryCommitmentMismatch);
        }
        Ok(())
    }

    fn validate_tree(
        &self,
        tree: KagemushaHistoryTreeV1,
        root: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if root != empty_root(tree) && !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        validate_tree_with_immutable_subtrees(
            tree,
            root,
            |address| self.read_node(address),
            |address| {
                self.durable_nodes.record_validation_visit();
                self.durable_nodes.validated_subtree(address)
            },
        )
        .map(|_| ())
    }

    fn require_prepared(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        transaction.validate()?;
        let Some(entry) = self.prepared.get(&transaction.transaction_id()) else {
            return Err(KagemushaHistoryStoreErrorV1::AttemptNotPrepared(
                transaction.transaction_id(),
            ));
        };
        if &entry.transaction != transaction {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        transaction
            .root_selection()
            .apply_to(validate_committed_history_v1(self)?)?;
        self.validate_selected_root_nodes(transaction)?;
        Ok(())
    }

    fn overlay_usage(&self) -> KagemushaHistoryOverlayUsageV1 {
        KagemushaHistoryOverlayUsageV1 {
            live_bytes: self.live_overlay_bytes,
            capacity_bytes: self.overlay_capacity_bytes,
        }
    }

    fn read_node(
        &self,
        address: DigestV1,
    ) -> Result<Option<KagemushaHistoryNodeRecordV1>, KagemushaHistoryStoreErrorV1> {
        if !self.object_store_available {
            return Err(KagemushaHistoryStoreErrorV1::StorageUnavailable);
        }
        let node = self.durable_nodes.get(&address).cloned();
        if node
            .as_ref()
            .is_some_and(|node| node.content_address().ok() != Some(address))
        {
            let tree = node
                .as_ref()
                .map_or(KagemushaHistoryTreeV1::Replay, |node| node.tree());
            return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode { tree, address });
        }
        Ok(node)
    }

    fn read_committed_root(
        &self,
        tree: KagemushaHistoryTreeV1,
    ) -> Result<KagemushaCommittedRootReadV1, KagemushaHistoryStoreErrorV1> {
        let root = self.roots.for_tree(tree);
        if root == empty_root(tree) {
            return Ok(KagemushaCommittedRootReadV1::Available { root, node: None });
        }
        if !self.object_store_available {
            return Ok(KagemushaCommittedRootReadV1::Unavailable { root });
        }
        let node = self
            .durable_nodes
            .get(&root)
            .cloned()
            .ok_or(KagemushaHistoryStoreErrorV1::MissingCommittedRoot { tree, root })?;
        if node.tree() != tree || node.content_address().ok() != Some(root) {
            return Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode {
                tree,
                address: root,
            });
        }
        Ok(KagemushaCommittedRootReadV1::Available {
            root,
            node: Some(node),
        })
    }

    fn prepare_cas(
        &mut self,
        transaction: KagemushaPreparedHistoryCasV1,
    ) -> Result<KagemushaHistoryPrepareOutcomeV1, KagemushaHistoryStoreErrorV1> {
        let plan = self.plan_prepare_cas(transaction)?;
        Ok(self.apply_plan(plan))
    }

    fn commit_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryCommitOutcomeV1, KagemushaHistoryStoreErrorV1> {
        let plan = self.plan_commit_prepared(certificate)?;
        Ok(self.apply_plan(plan))
    }

    fn abort_prepared(
        &mut self,
        transaction_id: DigestV1,
    ) -> Result<KagemushaHistoryAbortOutcomeV1, KagemushaHistoryStoreErrorV1> {
        let plan = self.plan_abort_prepared(transaction_id)?;
        Ok(self.apply_plan(plan))
    }

    fn recover_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryRecoveryOutcomeV1, KagemushaHistoryStoreErrorV1> {
        match self.commit_prepared(certificate)? {
            KagemushaHistoryCommitOutcomeV1::Committed { committed_roots } => {
                Ok(KagemushaHistoryRecoveryOutcomeV1::Committed { committed_roots })
            }
            KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots } => {
                Ok(KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted { committed_roots })
            }
            KagemushaHistoryCommitOutcomeV1::Aborted => {
                Ok(KagemushaHistoryRecoveryOutcomeV1::Aborted)
            }
        }
    }
}

fn digest_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
    let encoded = norito::encode_canonical(value)
        .map_err(|_| KagemushaHistoryStoreErrorV1::CanonicalEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

fn empty_root(tree: KagemushaHistoryTreeV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(tree.empty_root_domain());
    hasher.update(HISTORY_STORE_VERSION_V1.to_le_bytes());
    hasher.finalize().into()
}

fn canonical_prefix(mut prefix: DigestV1, depth: usize) -> DigestV1 {
    let whole_bytes = depth / 8;
    let retained_bits = depth % 8;
    if retained_bits == 0 {
        prefix[whole_bytes..].fill(0);
    } else {
        prefix[whole_bytes] &= u8::MAX << (8 - retained_bits);
        prefix[whole_bytes + 1..].fill(0);
    }
    prefix
}

fn key_bit(key: DigestV1, depth: usize) -> bool {
    let byte = key[depth / 8];
    let shift = 7 - (depth % 8);
    ((byte >> shift) & 1) == 1
}

fn common_prefix_bits(left: DigestV1, right: DigestV1) -> usize {
    for (byte_index, (&left_byte, &right_byte)) in left.iter().zip(&right).enumerate() {
        let difference = left_byte ^ right_byte;
        if difference != 0 {
            return byte_index * 8 + difference.leading_zeros() as usize;
        }
    }
    256
}

const fn digest_is_zero(digest: DigestV1) -> bool {
    let mut index = 0;
    while index < digest.len() {
        if digest[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
    use sha2::Digest as _;

    use super::*;

    fn digest(label: &[u8]) -> DigestV1 {
        Sha256::digest(label).into()
    }

    fn leaf(tree: KagemushaHistoryTreeV1, label: &[u8]) -> KagemushaHistoryNodeRecordV1 {
        KagemushaHistoryNodeRecordV1::leaf(
            tree,
            digest(&[label, b":key"].concat()),
            digest(&[label, b":value"].concat()),
        )
        .expect("valid history leaf")
    }

    fn prepared_leaf(
        tree: KagemushaHistoryTreeV1,
        expected: DigestV1,
        label: &[u8],
    ) -> (KagemushaPreparedHistoryCasV1, DigestV1) {
        let node = leaf(tree, label);
        let selected = node.content_address().expect("content address");
        let selection = match tree {
            KagemushaHistoryTreeV1::Replay => {
                KagemushaHistoryRootSelectionV1::replay(expected, selected)
            }
            KagemushaHistoryTreeV1::TerminalDecision => {
                KagemushaHistoryRootSelectionV1::terminal_decision(expected, selected)
            }
        };
        (
            KagemushaPreparedHistoryCasV1::new(
                selection,
                vec![node],
                digest(b"history-test-attempt"),
            )
            .expect("prepared history CAS"),
            selected,
        )
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[0x5A; 32]).into()).expect("P-256 signing key")
    }

    fn verified_selection(
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> VerifiedKagemushaHistoryRootSelectionV1 {
        let key = signing_key();
        let profile_id = digest(b"hardware-profile");
        let subject = KagemushaHistoryRootSelectionSubjectV1::new(transaction, profile_id, 7, 41);
        let bytes = subject.signing_bytes().expect("certificate bytes");
        let signature: Signature = key.sign(&bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature = KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical low-S signature");
        let public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("canonical device public key");
        let verified = KagemushaHistoryRootSelectionCertificateV1::new(subject, signature)
            .verify(profile_id, &public_key)
            .expect("authenticated root selection");
        assert_eq!(verified.hardware_profile_id(), profile_id);
        assert_eq!(verified.hardware_epoch(), 7);
        assert_eq!(verified.monotonic_counter(), 41);
        verified
    }

    #[test]
    fn stale_cas_never_replaces_a_committed_root() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let empty_replay = store.committed_roots().replay();
        let (first, first_root) = prepared_leaf(
            KagemushaHistoryTreeV1::Replay,
            empty_replay,
            b"first-replay",
        );
        let (stale, stale_root) = prepared_leaf(
            KagemushaHistoryTreeV1::Replay,
            empty_replay,
            b"stale-replay",
        );
        let first_certificate = verified_selection(&first);
        let stale_certificate = verified_selection(&stale);
        assert_eq!(
            store.prepare_cas(first),
            Ok(KagemushaHistoryPrepareOutcomeV1::Prepared)
        );
        assert_eq!(
            store.prepare_cas(stale),
            Ok(KagemushaHistoryPrepareOutcomeV1::Prepared)
        );
        assert_eq!(
            store.commit_prepared(first_certificate),
            Ok(KagemushaHistoryCommitOutcomeV1::Committed {
                committed_roots: KagemushaHistoryRootsV1 {
                    replay: first_root,
                    terminal_decision: empty_root(KagemushaHistoryTreeV1::TerminalDecision),
                },
            })
        );
        assert_eq!(
            store.commit_prepared(stale_certificate),
            Err(KagemushaHistoryStoreErrorV1::CasConflict {
                tree: KagemushaHistoryTreeV1::Replay,
                expected: empty_replay,
                actual: first_root,
            })
        );
        assert_eq!(store.committed_roots().replay(), first_root);
        assert_ne!(store.committed_roots().replay(), stale_root);
    }

    #[test]
    fn authenticated_recovery_is_idempotent() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let (transaction, selected) = prepared_leaf(
            KagemushaHistoryTreeV1::Replay,
            store.committed_roots().replay(),
            b"recover-replay",
        );
        let transaction_id = transaction.transaction_id();
        let certificate = verified_selection(&transaction);
        assert_eq!(
            store.prepare_cas(transaction),
            Ok(KagemushaHistoryPrepareOutcomeV1::Prepared)
        );
        let expected_roots = KagemushaHistoryRootsV1 {
            replay: selected,
            terminal_decision: empty_root(KagemushaHistoryTreeV1::TerminalDecision),
        };
        assert_eq!(
            store.recover_prepared(certificate),
            Ok(KagemushaHistoryRecoveryOutcomeV1::Committed {
                committed_roots: expected_roots,
            })
        );
        assert_eq!(
            store.recover_prepared(certificate),
            Ok(KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted {
                committed_roots: expected_roots,
            })
        );
        assert_eq!(
            store.abort_prepared(transaction_id),
            Ok(KagemushaHistoryAbortOutcomeV1::AlreadyCommitted {
                committed_roots: expected_roots,
            })
        );
        assert_eq!(store.overlay_usage().live_bytes(), 0);
    }

    #[test]
    fn replay_and_terminal_decision_roots_are_independent() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let roots = store.committed_roots();
        let replay_node = leaf(KagemushaHistoryTreeV1::Replay, b"same-payload");
        let decision_node = leaf(KagemushaHistoryTreeV1::TerminalDecision, b"same-payload");
        let replay_root = replay_node.content_address().expect("replay address");
        let decision_root = decision_node.content_address().expect("decision address");
        assert_ne!(replay_root, decision_root);

        let replay = KagemushaPreparedHistoryCasV1::new(
            KagemushaHistoryRootSelectionV1::replay(roots.replay(), replay_root),
            vec![replay_node],
            digest(b"history-test-attempt"),
        )
        .expect("replay CAS");
        let decision = KagemushaPreparedHistoryCasV1::new(
            KagemushaHistoryRootSelectionV1::terminal_decision(
                roots.terminal_decision(),
                decision_root,
            ),
            vec![decision_node],
            digest(b"history-test-attempt"),
        )
        .expect("decision CAS");
        let replay_certificate = verified_selection(&replay);
        let decision_certificate = verified_selection(&decision);
        assert_eq!(
            store.prepare_cas(replay),
            Ok(KagemushaHistoryPrepareOutcomeV1::Prepared)
        );
        assert_eq!(
            store.prepare_cas(decision),
            Ok(KagemushaHistoryPrepareOutcomeV1::Prepared)
        );
        store
            .commit_prepared(replay_certificate)
            .expect("commit replay root");
        store
            .commit_prepared(decision_certificate)
            .expect("commit independent decision root");
        assert_eq!(store.committed_roots().replay(), replay_root);
        assert_eq!(store.committed_roots().terminal_decision(), decision_root);
    }

    #[test]
    fn overlay_byte_cap_is_checked_before_prepare_mutates_wal() {
        let empty_replay = KagemushaHistoryRootsV1::empty().replay();
        let (transaction, selected) = prepared_leaf(
            KagemushaHistoryTreeV1::Replay,
            empty_replay,
            b"oversized-overlay",
        );
        let wal_bytes = transaction.wal_bytes().expect("WAL bytes");
        let capacity = wal_bytes.checked_sub(1).expect("nonempty transaction");
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(capacity);
        assert_eq!(
            store.prepare_cas(transaction),
            Err(KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded {
                required_bytes: wal_bytes,
                available_bytes: capacity,
            })
        );
        assert_eq!(store.overlay_usage().live_bytes(), 0);
        assert_eq!(store.overlay_usage().capacity_bytes(), capacity);
        assert_eq!(store.committed_roots(), KagemushaHistoryRootsV1::empty());
        assert_eq!(store.read_node(selected), Ok(None));
    }

    #[test]
    fn committed_root_survives_object_storage_unavailability() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let (transaction, selected) = prepared_leaf(
            KagemushaHistoryTreeV1::Replay,
            store.committed_roots().replay(),
            b"durable-replay",
        );
        let transaction_id = transaction.transaction_id();
        let certificate = verified_selection(&transaction);
        store.prepare_cas(transaction).expect("prepare durable CAS");
        store
            .commit_prepared(certificate)
            .expect("commit durable CAS");
        let committed_roots = store.committed_roots();

        store.set_object_store_available_for_test(false);
        assert_eq!(
            store.read_committed_root(KagemushaHistoryTreeV1::Replay),
            Ok(KagemushaCommittedRootReadV1::Unavailable { root: selected })
        );
        assert_eq!(store.committed_roots(), committed_roots);
        assert_eq!(
            store.abort_prepared(transaction_id),
            Ok(KagemushaHistoryAbortOutcomeV1::AlreadyCommitted { committed_roots })
        );
        assert_eq!(store.committed_roots(), committed_roots);

        store.set_object_store_available_for_test(true);
        assert!(matches!(
            store.read_committed_root(KagemushaHistoryTreeV1::Replay),
            Ok(KagemushaCommittedRootReadV1::Available {
                root,
                node: Some(_),
            }) if root == selected
        ));
        assert_eq!(store.committed_roots(), committed_roots);
    }

    fn prepare_identity(
        store: &mut KagemushaMemoryAuthenticatedHistoryStoreV1,
        tree: KagemushaHistoryTreeV1,
        key: DigestV1,
        value_digest: DigestV1,
    ) -> KagemushaPreparedHistoryCasV1 {
        match prepare_history_identity_insert_v1(
            store,
            tree,
            key,
            value_digest,
            digest(b"history-test-attempt"),
        )
        .expect("prepare authenticated identity")
        {
            KagemushaHistoryInsertPreparationV1::Prepared {
                transaction,
                outcome:
                    KagemushaHistoryPrepareOutcomeV1::Prepared
                    | KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared,
            } => transaction,
            other => panic!("unexpected prepare outcome: {other:?}"),
        }
    }

    fn commit_identity(
        store: &mut KagemushaMemoryAuthenticatedHistoryStoreV1,
        tree: KagemushaHistoryTreeV1,
        key: DigestV1,
        value_digest: DigestV1,
    ) -> KagemushaPreparedHistoryCasV1 {
        let transaction = prepare_identity(store, tree, key, value_digest);
        store
            .commit_prepared(verified_selection(&transaction))
            .expect("commit authenticated identity");
        transaction
    }

    #[test]
    fn authenticated_identity_map_recovers_exact_duplicates_and_conflicts() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let first_key = digest(b"replay-key-one");
        let second_key = digest(b"replay-key-two");
        let first_value = digest(b"replay-envelope-one");
        let second_value = digest(b"replay-envelope-two");
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            first_key,
            first_value,
        );
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            second_key,
            second_value,
        );

        assert_eq!(
            classify_history_identity_v1(
                &store,
                KagemushaHistoryTreeV1::Replay,
                first_key,
                first_value,
            ),
            Ok(KagemushaHistoryIdentityClassificationV1::ExactDuplicate)
        );
        let conflicting = digest(b"different-envelope");
        assert_eq!(
            classify_history_identity_v1(
                &store,
                KagemushaHistoryTreeV1::Replay,
                first_key,
                conflicting,
            ),
            Ok(KagemushaHistoryIdentityClassificationV1::Conflict {
                existing_value_digest: first_value,
            })
        );
        assert_eq!(
            prepare_history_identity_insert_v1(
                &mut store,
                KagemushaHistoryTreeV1::Replay,
                first_key,
                first_value,
                digest(b"history-test-attempt"),
            ),
            Ok(KagemushaHistoryInsertPreparationV1::ExactDuplicate)
        );
        assert_eq!(store.overlay_usage().live_bytes(), 0);
        validate_committed_history_v1(&store).expect("complete replay tree remains recoverable");
    }

    #[test]
    fn replay_and_terminal_identity_maps_advance_independently() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let initial = store.committed_roots();
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            digest(b"shared-id"),
            digest(b"replay-value"),
        );
        let after_replay = store.committed_roots();
        assert_ne!(after_replay.replay(), initial.replay());
        assert_eq!(
            after_replay.terminal_decision(),
            initial.terminal_decision()
        );
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::TerminalDecision,
            digest(b"shared-id"),
            digest(b"terminal-value"),
        );
        let after_both = validate_committed_history_v1(&store).expect("both trees validate");
        assert_eq!(after_both.replay(), after_replay.replay());
        assert_ne!(
            after_both.terminal_decision(),
            after_replay.terminal_decision()
        );
    }

    #[test]
    fn dual_identity_prepare_commits_both_roots_atomically() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let predecessor = store.committed_roots();
        let replay_key = digest(b"atomic-replay-key");
        let replay_value = digest(b"atomic-replay-value");
        let decision_key = digest(b"atomic-decision-key");
        let decision_value = digest(b"atomic-decision-value");
        let transaction = match prepare_history_identity_pair_v1(
            &mut store,
            replay_key,
            replay_value,
            decision_key,
            decision_value,
            digest(b"history-test-attempt"),
        )
        .expect("prepare dual-root CAS")
        {
            KagemushaHistoryDualInsertPreparationV1::Prepared {
                transaction,
                outcome: KagemushaHistoryPrepareOutcomeV1::Prepared,
            } => transaction,
            other => panic!("unexpected dual prepare: {other:?}"),
        };
        let selected = transaction
            .successor_roots_from(predecessor)
            .expect("apply dual CAS");
        assert_ne!(selected.replay(), predecessor.replay());
        assert_ne!(
            selected.terminal_decision(),
            predecessor.terminal_decision()
        );
        assert_eq!(store.committed_roots(), predecessor);

        assert_eq!(
            store.commit_prepared(verified_selection(&transaction)),
            Ok(KagemushaHistoryCommitOutcomeV1::Committed {
                committed_roots: selected,
            })
        );
        assert_eq!(store.committed_roots(), selected);
        assert_eq!(
            prepare_history_identity_pair_v1(
                &mut store,
                replay_key,
                replay_value,
                decision_key,
                decision_value,
                digest(b"history-test-attempt"),
            ),
            Ok(KagemushaHistoryDualInsertPreparationV1::ExactDuplicate)
        );
        let roots_before_conflict = store.committed_roots();
        assert!(matches!(
            prepare_history_identity_pair_v1(
                &mut store,
                replay_key,
                digest(b"conflicting-replay-value"),
                digest(b"never-inserted-decision"),
                digest(b"never-inserted-value"),
             digest(b"history-test-attempt"),),
            Ok(KagemushaHistoryDualInsertPreparationV1::Conflict {
                tree: KagemushaHistoryTreeV1::Replay,
                key,
                existing_value_digest,
            }) if key == replay_key && existing_value_digest == replay_value
        ));
        assert_eq!(store.committed_roots(), roots_before_conflict);
        assert_eq!(store.overlay_usage().live_bytes(), 0);
    }

    #[test]
    fn recovery_fails_closed_for_missing_or_corrupt_external_nodes() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            digest(b"recovery-key-one"),
            digest(b"recovery-value-one"),
        );
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            digest(b"recovery-key-two"),
            digest(b"recovery-value-two"),
        );
        let committed_roots = store.committed_roots();
        let replay_root = committed_roots.replay();
        let root_node = store
            .durable_nodes
            .get(&replay_root)
            .cloned()
            .expect("committed root node");
        let missing_address = match root_node.body() {
            KagemushaHistoryNodeBodyV1::Branch { left, .. } => *left,
            KagemushaHistoryNodeBodyV1::Leaf { .. } => panic!("two identities require a branch"),
        };
        let missing_node = store
            .durable_nodes
            .remove(&missing_address)
            .expect("remove one committed child");
        assert!(matches!(
            validate_committed_history_v1(&store),
            Err(KagemushaHistoryStoreErrorV1::MissingHistoryNode {
                tree: KagemushaHistoryTreeV1::Replay,
                address,
            }) if address == missing_address
        ));
        assert_eq!(store.committed_roots(), committed_roots);
        assert!(
            matches!(classify_history_identity_v1(&store, KagemushaHistoryTreeV1::Replay, digest(b"off-path key"), digest(b"off-path value")), Err(KagemushaHistoryStoreErrorV1::MissingHistoryNode { address, .. }) if address == missing_address)
        );
        assert!(
            matches!(prepare_history_identity_insert_v1(&mut store, KagemushaHistoryTreeV1::Replay, digest(b"off-path key"), digest(b"off-path value"), digest(b"off-path attempt")), Err(KagemushaHistoryStoreErrorV1::MissingHistoryNode { address, .. }) if address == missing_address)
        );
        assert_eq!(store.overlay_usage().live_bytes(), 0);

        store.durable_nodes.insert(missing_address, missing_node);
        let corrupt = leaf(KagemushaHistoryTreeV1::Replay, b"corrupt-substitute");
        assert_ne!(
            corrupt.content_address().expect("corrupt address"),
            missing_address
        );
        store.durable_nodes.insert(missing_address, corrupt);
        assert!(matches!(
            validate_committed_history_v1(&store),
            Err(KagemushaHistoryStoreErrorV1::CorruptHistoryNode {
                tree: KagemushaHistoryTreeV1::Replay,
                address,
            }) if address == missing_address
        ));
        assert_eq!(store.committed_roots(), committed_roots);
    }

    #[test]
    fn overlay_exhaustion_blocks_only_new_prepared_work() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let committed_key = digest(b"already-committed-key");
        let committed_value = digest(b"already-committed-value");
        commit_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            committed_key,
            committed_value,
        );
        let roots_before_exhaustion = store.committed_roots();
        store.overlay_capacity_bytes = 0;

        assert!(matches!(
            prepare_history_identity_insert_v1(
                &mut store,
                KagemushaHistoryTreeV1::Replay,
                digest(b"new-prepared-key"),
                digest(b"new-prepared-value"),
                digest(b"history-test-attempt"),
            ),
            Err(KagemushaHistoryStoreErrorV1::OverlayCapacityExceeded { .. })
        ));
        assert_eq!(store.committed_roots(), roots_before_exhaustion);
        assert_eq!(store.overlay_usage().live_bytes(), 0);
        assert_eq!(
            classify_history_identity_v1(
                &store,
                KagemushaHistoryTreeV1::Replay,
                committed_key,
                committed_value,
            ),
            Ok(KagemushaHistoryIdentityClassificationV1::ExactDuplicate)
        );

        let mut staged_store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let staged = prepare_identity(
            &mut staged_store,
            KagemushaHistoryTreeV1::TerminalDecision,
            digest(b"already-staged-decision"),
            digest(b"already-staged-value"),
        );
        assert!(staged_store.overlay_usage().live_bytes() > 0);
        staged_store.overlay_capacity_bytes = 0;
        assert!(matches!(
            staged_store.commit_prepared(verified_selection(&staged)),
            Ok(KagemushaHistoryCommitOutcomeV1::Committed { .. })
        ));
        assert_eq!(staged_store.overlay_usage().live_bytes(), 0);
        assert_eq!(
            classify_history_identity_v1(
                &staged_store,
                KagemushaHistoryTreeV1::TerminalDecision,
                digest(b"already-staged-decision"),
                digest(b"already-staged-value"),
            ),
            Ok(KagemushaHistoryIdentityClassificationV1::ExactDuplicate)
        );
    }

    #[test]
    fn sha_history_and_pasta_replay_roots_require_an_authenticated_bridge() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let predecessor_roots = store.committed_roots();
        let transaction = prepare_identity(
            &mut store,
            KagemushaHistoryTreeV1::Replay,
            digest(b"bridge-credit"),
            digest(b"bridge-envelope"),
        );
        let successor_roots = transaction
            .root_selection()
            .apply_to(predecessor_roots)
            .expect("exact successor roots");
        let pasta_predecessor = KagemushaPastaStateCommitmentV1 {
            eq: digest(b"pasta-predecessor-eq"),
            ep: digest(b"pasta-predecessor-ep"),
        };
        let pasta_successor = KagemushaPastaStateCommitmentV1 {
            eq: digest(b"pasta-successor-eq"),
            ep: digest(b"pasta-successor-ep"),
        };
        let request = KagemushaHistoryProofRootBridgeRequestV1::new(
            &transaction,
            digest(b"logical-receive-fold"),
            predecessor_roots,
            successor_roots,
            pasta_predecessor,
            pasta_successor,
        )
        .expect("well-shaped bridge request");

        assert_eq!(request.transaction_id(), transaction.transaction_id());
        assert_eq!(request.external_predecessor_roots(), predecessor_roots);
        assert_eq!(request.external_successor_roots(), successor_roots);
        assert_eq!(request.pasta_predecessor_replay_root(), pasta_predecessor);
        assert_eq!(request.pasta_successor_replay_root(), pasta_successor);
        assert_eq!(
            require_history_proof_root_bridge_v1(request, digest(b"wrong-logical-operation")),
            Err(KagemushaHistoryProofRootBridgeErrorV1::BindingMismatch { request })
        );
        assert_eq!(
            require_history_proof_root_bridge_v1(request, digest(b"logical-receive-fold"),)
                .expect("matching authenticated logical operation")
                .request(),
            request
        );
        assert_eq!(store.committed_roots(), predecessor_roots);
    }
    #[test]
    fn aborted_attempt_keeps_its_tombstone_while_a_new_attempt_can_prepare() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let roots = store.committed_roots();
        let (first, _) = prepared_leaf(KagemushaHistoryTreeV1::Replay, roots.replay(), b"attempts");
        store.prepare_cas(first.clone()).unwrap();
        store.abort_prepared(first.transaction_id()).unwrap();
        let next = KagemushaPreparedHistoryCasV1::new(
            first.root_selection(),
            first
                .node_writes
                .iter()
                .map(|write| write.node.clone())
                .collect(),
            digest(b"next authenticated transition attempt"),
        )
        .unwrap();
        assert_ne!(first.transaction_id(), next.transaction_id());
        assert_eq!(
            KagemushaPreparedHistoryCasV1::new(
                first.root_selection(),
                first
                    .node_writes
                    .iter()
                    .map(|write| write.node.clone())
                    .collect(),
                [0; 32],
            ),
            Err(KagemushaHistoryStoreErrorV1::InvalidTransaction)
        );
        assert_eq!(
            store.prepare_cas(next.clone()).unwrap(),
            KagemushaHistoryPrepareOutcomeV1::Prepared
        );
        assert_eq!(
            store.prepare_cas(next.clone()).unwrap(),
            KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
        );
        assert_eq!(
            store.prepare_cas(first).unwrap(),
            KagemushaHistoryPrepareOutcomeV1::AlreadyAborted
        );
        let mut substituted = next;
        substituted.attempt_binding_digest = digest(b"unbound replacement attempt");
        assert_eq!(
            store.prepare_cas(substituted),
            Err(KagemushaHistoryStoreErrorV1::InvalidTransaction)
        );
        assert_eq!(store.committed_roots(), roots);
    }

    #[test]
    fn immutable_subtree_cache_rejects_wrong_edges_depth_and_namespace() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let tree = KagemushaHistoryTreeV1::Replay;
        let key_one = [0x10; 32];
        commit_identity(&mut store, tree, key_one, digest(b"cached one"));
        let leaf_root = store.committed_roots().replay();
        assert!(store.durable_nodes.subtrees.contains_key(&leaf_root));
        let wrong_edge =
            KagemushaHistoryNodeRecordV1::branch(tree, 0, [0; 32], empty_root(tree), leaf_root)
                .unwrap();
        let foreign_tree = KagemushaHistoryTreeV1::TerminalDecision;
        let foreign = KagemushaHistoryNodeRecordV1::branch(
            foreign_tree,
            0,
            [0; 32],
            leaf_root,
            empty_root(foreign_tree),
        )
        .unwrap();
        for (tree, node) in [(tree, wrong_edge), (foreign_tree, foreign)] {
            let selected = node.content_address().unwrap();
            let selection = match tree {
                KagemushaHistoryTreeV1::Replay => KagemushaHistoryRootSelectionV1::replay(
                    store.committed_roots().replay(),
                    selected,
                ),
                KagemushaHistoryTreeV1::TerminalDecision => {
                    KagemushaHistoryRootSelectionV1::terminal_decision(
                        store.committed_roots().terminal_decision(),
                        selected,
                    )
                }
            };
            let full = validate_tree_with_lookup(tree, selected, |address| {
                Ok(if address == selected {
                    Some(node.clone())
                } else {
                    store.durable_nodes.get(&address).cloned()
                })
            });
            let tx = KagemushaPreparedHistoryCasV1::new(
                selection,
                vec![node],
                digest(b"bad cached edge"),
            )
            .unwrap();
            let cached = store.prepare_cas(tx);
            assert_eq!(cached.unwrap_err(), full.unwrap_err());
            assert_eq!(store.overlay_usage().live_bytes(), 0);
        }
        commit_identity(&mut store, tree, [0x20; 32], digest(b"cached two"));
        let root = store.committed_roots().replay();
        let summary = *store.durable_nodes.subtrees.get(&root).unwrap();
        let depth = summary.branch_depth.unwrap();
        let node = KagemushaHistoryNodeRecordV1::branch(
            tree,
            depth as u16,
            summary.representative,
            root,
            empty_root(tree),
        )
        .unwrap();
        let selected = node.content_address().unwrap();
        let tx = KagemushaPreparedHistoryCasV1::new(
            KagemushaHistoryRootSelectionV1::replay(root, selected),
            vec![node],
            digest(b"non-advancing cached depth"),
        )
        .unwrap();
        assert!(matches!(
            store.prepare_cas(tx),
            Err(KagemushaHistoryStoreErrorV1::InvalidHistoryTree { .. })
        ));
    }

    #[test]
    fn immutable_subtree_cache_never_publishes_prepared_or_aborted_nodes() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let tree = KagemushaHistoryTreeV1::Replay;
        let (first, selected) =
            prepared_leaf(tree, store.committed_roots().replay(), b"not committed");
        store.prepare_cas(first.clone()).unwrap();
        assert!(store.durable_nodes.subtrees.is_empty());
        store.abort_prepared(first.transaction_id()).unwrap();
        let key = match first.node_writes[0].node.body() {
            KagemushaHistoryNodeBodyV1::Leaf { key, .. } => *key,
            _ => unreachable!(),
        };
        let (left, right) = if key_bit(key, 0) {
            (empty_root(tree), selected)
        } else {
            (selected, empty_root(tree))
        };
        let node = KagemushaHistoryNodeRecordV1::branch(tree, 0, [0; 32], left, right).unwrap();
        let next = KagemushaPreparedHistoryCasV1::new(
            KagemushaHistoryRootSelectionV1::replay(
                store.committed_roots().replay(),
                node.content_address().unwrap(),
            ),
            vec![node],
            digest(b"missing speculative child"),
        )
        .unwrap();
        assert!(
            matches!(store.prepare_cas(next), Err(KagemushaHistoryStoreErrorV1::MissingHistoryNode {address, ..}) if address == selected)
        );
        assert!(store.durable_nodes.subtrees.is_empty());
    }

    #[test]
    fn immutable_subtree_validation_visits_only_new_paths_as_history_grows() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        for index in 0_u64..512 {
            store.durable_nodes.validation_visits.set(0);
            let transaction = match prepare_history_identity_pair_v1(
                &mut store,
                digest(&[b"replay-key".as_slice(), &index.to_le_bytes()].concat()),
                digest(b"replay-value"),
                digest(&[b"decision-key".as_slice(), &index.to_le_bytes()].concat()),
                digest(b"decision-value"),
                digest(&[b"attempt".as_slice(), &index.to_le_bytes()].concat()),
            )
            .unwrap()
            {
                KagemushaHistoryDualInsertPreparationV1::Prepared { transaction, .. } => {
                    transaction
                }
                _ => panic!("unique identities must prepare"),
            };
            let new_nodes = transaction.node_writes.len() as u64;
            // Two 256-bit paths have at most 256 branches and one leaf each. A
            // whole-history overlay would violate this bound as this fixture grows.
            assert!(new_nodes <= 2 * 257);
            assert!(store.durable_nodes.validation_visits.get() <= 2 * new_nodes + 4);
            store.durable_nodes.validation_visits.set(0);
            store
                .commit_prepared(verified_selection(&transaction))
                .unwrap();
            assert!(store.durable_nodes.validation_visits.get() <= 2 * new_nodes + 2);
            store.durable_nodes.validation_visits.set(0);
            validate_committed_history_v1(&store).unwrap();
            assert_eq!(store.durable_nodes.validation_visits.get(), 2);
        }
        assert!(store.durable_nodes.nodes.len() > 1_000);
        store.set_object_store_available_for_test(false);
        assert_eq!(
            validate_committed_history_v1(&store),
            Err(KagemushaHistoryStoreErrorV1::StorageUnavailable)
        );
    }
    #[test]
    fn hardware_preflight_requires_exact_live_prepared_attempt_and_storage() {
        let mut store = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let root = store.committed_roots().replay();
        let (first, _) = prepared_leaf(KagemushaHistoryTreeV1::Replay, root, b"preflight first");
        let (stale, _) = prepared_leaf(KagemushaHistoryTreeV1::Replay, root, b"preflight stale");
        assert_eq!(
            store.require_prepared(&first),
            Err(KagemushaHistoryStoreErrorV1::AttemptNotPrepared(
                first.transaction_id()
            ))
        );
        store.prepare_cas(first.clone()).unwrap();
        store.prepare_cas(stale.clone()).unwrap();
        store.require_prepared(&first).unwrap();
        store.require_prepared(&stale).unwrap();
        let mut substituted = first.clone();
        substituted.attempt_binding_digest = digest(b"unbound preflight");
        assert_eq!(
            store.require_prepared(&substituted),
            Err(KagemushaHistoryStoreErrorV1::InvalidTransaction)
        );
        let cert = verified_selection(&first);
        store.commit_prepared(cert).unwrap();
        assert_eq!(
            store.require_prepared(&first),
            Err(KagemushaHistoryStoreErrorV1::AttemptNotPrepared(
                first.transaction_id()
            ))
        );
        assert!(matches!(
            store.require_prepared(&stale),
            Err(KagemushaHistoryStoreErrorV1::CasConflict { .. })
        ));
        assert!(matches!(
            store.recover_prepared(cert).unwrap(),
            KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted { .. }
        ));
        store.set_object_store_available_for_test(false);
        assert_eq!(
            store.require_prepared(&first),
            Err(KagemushaHistoryStoreErrorV1::StorageUnavailable)
        );
        store.set_object_store_available_for_test(true);
        store.abort_prepared(stale.transaction_id()).unwrap();
        assert_eq!(
            store.require_prepared(&stale),
            Err(KagemushaHistoryStoreErrorV1::AttemptNotPrepared(
                stale.transaction_id()
            ))
        );
    }
}
