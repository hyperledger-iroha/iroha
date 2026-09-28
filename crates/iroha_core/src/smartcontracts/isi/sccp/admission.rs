//! Admission of fee-exempt SCCP transactions (`specs/sccp.md` §4.2.3, §4.8, §4.11, §4.12.4,
//! §4.13.4, §4.19). Owner: ws31; ws20 implemented the queue-side pending-key index.
//!
//! A transaction may be SCCP-exempt only when it has one of the [`SccpExemptClassV1`] shapes
//! ([`exempt_shape`]), a pure function of its signed payload. [`classify`] pre-verifies such a
//! transaction against committed state and returns its [`SccpAdmissionKeysV1`]: the class, the
//! *exclusive* keys that encode the per-kind pending limits (at most one pending transaction may
//! hold each), and the *content* keys used for deduplication (a transaction is rejected when
//! every content key is already queued). The queue keeps these keys in a
//! [`SccpPendingIndexV1`] and releases them on commit, expiry and eviction.
//!
//! **Per-block cap.** Proposers include at most `max_exempt_transactions_per_block` exempt-shaped
//! transactions per block, and at most one exempt-shaped keeper advance per network, and block
//! validation enforces the same rule with [`block_exempt_cap_ok`]. Both sides count
//! [`exempt_shape_of_entrypoint`] over the external and sealed-reveal entry points against the
//! parameters of the committed parent state, so the count never depends on admission-time
//! pre-verification, on queue history (restarts), or on state that changes between admission and
//! proposal: a block a proposer builds is never rejected by the cap. Fee exemption at execution
//! ([`super::fees::exempt_on_success`]) implies the shape, so every exempt transaction is counted.

use super::params;
use crate::state::WorldReadOnly;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    bridge::SccpNetworkV1,
    isi::{
        InstructionBox,
        sccp::{
            AdvanceSccpLightClientV1, SccpSettleTargetV1, SetSccpBridgeKeyV1, SettleSccpV1,
            SubmitSccpAttestationFaultV1, SubmitSccpAttestationsV1, SubmitSccpInboundMessageV1,
        },
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint, TransactionPayload},
};
use norito::codec::Encode;
use std::collections::{BTreeMap, BTreeSet};

/// Kinds of fee-exempt SCCP transactions (§4.19). Pending limits are per kind, never across
/// kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode)]
pub enum SccpExemptClassV1 {
    /// One `SubmitSccpAttestationsV1` from a bridge key's account (§4.8).
    Attestation,
    /// One `SetSccpBridgeKeyV1` registration from the new key's account (§4.2.3).
    KeyBinding,
    /// One `SubmitSccpAttestationFaultV1` that records a new fault (§4.11).
    Fault,
    /// A keeper advance of one light client from a bridge key's account (§4.13.4).
    KeeperAdvance {
        /// Advanced source network; at most one exempt advance per network per block.
        network: SccpNetworkV1,
    },
    /// A recipient self-claim of an inbound message (§4.12.4).
    SelfClaim,
}

/// Admission keys of one fee-exempt SCCP transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpAdmissionKeysV1 {
    /// Exempt kind of the transaction.
    pub class: SccpExemptClassV1,
    /// Keys at most one queued transaction may hold at a time (the per-kind pending limits:
    /// one attestation batch per authority, one keeper advance per authority and network, one
    /// key binding per peer, one self-claim per authority and per message).
    pub exclusive: BTreeSet<[u8; 32]>,
    /// Deduplication keys; a transaction must add at least one key that is not already queued
    /// (for example the `(height, signer_index)` entries of an attestation batch). Empty means
    /// the transaction is deduplicated by its exclusive keys alone.
    pub content: BTreeSet<[u8; 32]>,
}

impl SccpAdmissionKeysV1 {
    /// Build keys of `class` with no keys yet.
    #[must_use]
    pub fn new(class: SccpExemptClassV1) -> Self {
        Self {
            class,
            exclusive: BTreeSet::new(),
            content: BTreeSet::new(),
        }
    }

    /// Derive the domain-separated key of `parts` for `class` and `role`.
    ///
    /// Keys of different classes, roles or parts never collide, so one index serves every
    /// kind.
    #[must_use]
    pub fn derive_key(class: SccpExemptClassV1, role: &str, parts: &impl Encode) -> [u8; 32] {
        let mut preimage = b"iroha:sccp:admission-key:v1".to_vec();
        preimage.extend(class.encode());
        preimage.extend(u32::try_from(role.len()).unwrap_or(u32::MAX).to_be_bytes());
        preimage.extend(role.as_bytes());
        preimage.extend(parts.encode());
        let hash = Hash::new(preimage);
        let mut key = [0_u8; 32];
        key.copy_from_slice(hash.as_ref());
        key
    }

    /// Add an exclusive key over `parts`.
    #[must_use]
    pub fn with_exclusive(mut self, parts: &impl Encode) -> Self {
        let key = Self::derive_key(self.class, "exclusive", parts);
        self.exclusive.insert(key);
        self
    }

    /// Add a content key over `parts`.
    #[must_use]
    pub fn with_content(mut self, parts: &impl Encode) -> Self {
        let key = Self::derive_key(self.class, "content", parts);
        self.content.insert(key);
        self
    }
}

/// Admission rejection of a transaction that claims an SCCP exemption but fails
/// pre-verification against committed state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("SCCP exempt admission rejected: {reason}")]
pub struct SccpAdmissionRejectV1 {
    /// Human-readable reason.
    pub reason: String,
}

impl SccpAdmissionRejectV1 {
    /// Build a rejection with `reason`.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// Build the rejection of a pre-verification that is not implemented yet.
    #[must_use]
    pub fn not_wired(what: &str, owner: &str) -> Self {
        Self::new(format!("SCCP: {what} not implemented yet (TODO({owner}))"))
    }
}

fn downcast<T: 'static>(instruction: &InstructionBox) -> Option<&T> {
    instruction.as_any().downcast_ref::<T>()
}

/// Return the exempt class `payload` has the shape of, or `None` when no SCCP exemption can
/// apply to it (§4.19). The rule is a pure function of the signed payload:
///
/// * [`SccpExemptClassV1::Attestation`]: exactly one `SubmitSccpAttestationsV1` (§4.8);
/// * [`SccpExemptClassV1::KeyBinding`]: exactly one `SetSccpBridgeKeyV1` that registers a key
///   (§4.2.3); a revocation pays ordinary fees;
/// * [`SccpExemptClassV1::Fault`]: exactly one `SubmitSccpAttestationFaultV1` (§4.11);
/// * [`SccpExemptClassV1::KeeperAdvance`]: exactly one `AdvanceSccpLightClientV1` (§4.13.4);
/// * [`SccpExemptClassV1::SelfClaim`]: `[Register<Account>(authority)?,
///   AdvanceSccpLightClientV1*, SubmitSccpInboundMessageV1]` or `[SettleSccpV1::Inbound]`
///   (§4.12.4); whether the authority is the payload's recipient is decided by
///   pre-verification and execution.
///
/// A shaped transaction counts toward the per-block cap whether or not it turns out exempt.
#[must_use]
pub fn exempt_shape(payload: &TransactionPayload) -> Option<SccpExemptClassV1> {
    let Executable::Instructions(instructions) = &payload.instructions else {
        return None;
    };
    let instructions: &[InstructionBox] = instructions;
    if let [only] = instructions {
        if downcast::<SubmitSccpAttestationsV1>(only).is_some() {
            return Some(SccpExemptClassV1::Attestation);
        }
        if downcast::<SetSccpBridgeKeyV1>(only).is_some_and(|set| set.public_key.is_some()) {
            return Some(SccpExemptClassV1::KeyBinding);
        }
        if downcast::<SubmitSccpAttestationFaultV1>(only).is_some() {
            return Some(SccpExemptClassV1::Fault);
        }
        if let Some(advance) = downcast::<AdvanceSccpLightClientV1>(only) {
            return Some(SccpExemptClassV1::KeeperAdvance {
                network: advance.network,
            });
        }
        if downcast::<SettleSccpV1>(only)
            .is_some_and(|settle| matches!(settle.target, SccpSettleTargetV1::Inbound(_)))
        {
            return Some(SccpExemptClassV1::SelfClaim);
        }
    }
    let rest = if crate::tx::executable_self_registers_authority(
        &payload.instructions,
        &payload.authority,
    ) {
        &instructions[1..]
    } else {
        instructions
    };
    let (last, advances) = rest.split_last()?;
    (downcast::<SubmitSccpInboundMessageV1>(last).is_some()
        && advances
            .iter()
            .all(|instruction| downcast::<AdvanceSccpLightClientV1>(instruction).is_some()))
    .then_some(SccpExemptClassV1::SelfClaim)
}

/// Return the exempt shape of a block or queue entry point: the [`exempt_shape`] of the signed
/// transaction of an external or sealed-reveal entry point, and `None` for a sealed
/// commitment, which executes nothing.
#[must_use]
pub fn exempt_shape_of_entrypoint(entrypoint: &TransactionEntrypoint) -> Option<SccpExemptClassV1> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => exempt_shape(transaction.payload()),
        TransactionEntrypoint::SealedReveal(reveal) => {
            exempt_shape(reveal.signed_transaction().payload())
        }
        TransactionEntrypoint::SealedCommitment(_) => None,
    }
}

/// Classify `transaction` against committed state (`world` at the next block height
/// `next_block_height`).
///
/// Returns `Ok(None)` for a transaction that claims no SCCP exemption (it pays ordinary fees
/// and needs no SCCP admission step), `Ok(Some(keys))` for an admissible exempt transaction,
/// and `Err` for an exempt-shaped transaction that fails pre-verification. Only a transaction
/// with an [`exempt_shape`] is ever pre-verified, and its keys always carry that class.
///
/// # Errors
///
/// Returns [`SccpAdmissionRejectV1`] when an exempt-shaped transaction fails pre-verification.
pub fn classify(
    world: &(impl WorldReadOnly + ?Sized),
    next_block_height: u64,
    transaction: &SignedTransaction,
) -> Result<Option<SccpAdmissionKeysV1>, SccpAdmissionRejectV1> {
    if !params::exists(world) {
        return Ok(None);
    }
    let Some(class) = exempt_shape(transaction.payload()) else {
        return Ok(None);
    };
    #[cfg(test)]
    if let Some(keys) = test_override::classify(transaction) {
        return keys.map(|keys| keys.filter(|keys| keys.class == class));
    }
    let _ = (next_block_height, class);
    // TODO(ws31): pre-verify the shape `class` (§4.2.3, §4.8, §4.11, §4.12.4, §4.13.4) through
    // `attestations::preverify`, `faults::preverify`, `light_clients::preverify_keeper_advance`
    // and `self_claim::preverify`, returning keys whose class is `class`.
    Ok(None)
}

/// Test-only replacement of the pre-verification step of [`classify`], per thread, standing in
/// for ws31's pre-verifiers in queue tests.
#[cfg(test)]
pub(crate) mod test_override {
    use super::{SccpAdmissionKeysV1, SccpAdmissionRejectV1, SignedTransaction};
    use std::cell::RefCell;

    type Classifier = Box<
        dyn Fn(&SignedTransaction) -> Result<Option<SccpAdmissionKeysV1>, SccpAdmissionRejectV1>,
    >;

    std::thread_local! {
        static CLASSIFIER: RefCell<Option<Classifier>> = const { RefCell::new(None) };
    }

    /// Replace pre-verification of exempt-shaped transactions on this thread until the
    /// returned guard drops.
    pub(crate) fn install(
        classifier: impl Fn(
            &SignedTransaction,
        ) -> Result<Option<SccpAdmissionKeysV1>, SccpAdmissionRejectV1>
        + 'static,
    ) -> impl Drop {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                CLASSIFIER.with(|slot| slot.borrow_mut().take());
            }
        }
        CLASSIFIER.with(|slot| *slot.borrow_mut() = Some(Box::new(classifier)));
        Reset
    }

    pub(super) fn classify(
        transaction: &SignedTransaction,
    ) -> Option<Result<Option<SccpAdmissionKeysV1>, SccpAdmissionRejectV1>> {
        CLASSIFIER.with(|slot| slot.borrow().as_ref().map(|classify| classify(transaction)))
    }
}

/// Return whether `authority` may be absent from state for `executable` because of an SCCP
/// rule: a bridge-key registration from the new key's own account (§4.2.3).
#[must_use]
pub fn allows_unregistered_authority(executable: &Executable, authority: &AccountId) -> bool {
    let _ = (executable, authority);
    // TODO(ws31): admit exactly one `SetSccpBridgeKeyV1` registration whose authority is
    // `account_of(public_key)`.
    false
}

/// Return the per-block cap on fee-exempt SCCP transactions, or `None` when SCCP does not
/// exist.
#[must_use]
pub fn exempt_cap(world: &(impl WorldReadOnly + ?Sized)) -> Option<u32> {
    params::parameters(world).map(|parameters| parameters.max_exempt_transactions_per_block)
}

/// Return whether a block whose exempt-shaped SCCP transactions have `classes`, in block
/// order, respects the per-block caps of the parent state `world` (§4.19): at most
/// `max_exempt_transactions_per_block` of them and at most one keeper advance per network.
///
/// This is the rule [`SccpExemptBlockBudgetV1`] applies during proposal selection, so a block
/// its proposer built always passes.
#[must_use]
pub fn block_exempt_cap_ok(
    world: &(impl WorldReadOnly + ?Sized),
    classes: &[SccpExemptClassV1],
) -> bool {
    let mut budget = SccpExemptBlockBudgetV1::new(world);
    classes.iter().all(|class| budget.admit(Some(*class)))
}

/// Why the pending index refused a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpPendingClaimErrorV1 {
    /// The entry point already holds a claim.
    EntrypointClaimed,
    /// Another queued transaction holds an exclusive key (a pending limit).
    ExclusiveHeld {
        /// Queued transaction holding the key.
        holder: HashOf<TransactionEntrypoint>,
    },
    /// Every content key is already queued, so the transaction adds nothing.
    NothingNew,
}

impl core::fmt::Display for SccpPendingClaimErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EntrypointClaimed => f.write_str("the transaction already holds an SCCP claim"),
            Self::ExclusiveHeld { holder } => write!(
                f,
                "an SCCP pending limit is held by queued transaction {holder}"
            ),
            Self::NothingNew => {
                f.write_str("every SCCP entry of the transaction is already queued")
            }
        }
    }
}

/// Queue index of the admission keys of every queued fee-exempt SCCP transaction.
///
/// The queue mutates it only under its admission/removal lock, together with its transaction
/// map, so keys live exactly as long as their transaction is queued.
#[derive(Debug, Default)]
pub struct SccpPendingIndexV1 {
    by_entrypoint: BTreeMap<HashOf<TransactionEntrypoint>, SccpAdmissionKeysV1>,
    exclusive: BTreeMap<[u8; 32], HashOf<TransactionEntrypoint>>,
    content: BTreeMap<[u8; 32], usize>,
}

impl SccpPendingIndexV1 {
    /// Check whether `hash` may claim `keys` without mutating the index.
    ///
    /// # Errors
    ///
    /// Returns why the claim is refused.
    pub fn validate_claim(
        &self,
        hash: &HashOf<TransactionEntrypoint>,
        keys: &SccpAdmissionKeysV1,
    ) -> Result<(), SccpPendingClaimErrorV1> {
        if self.by_entrypoint.contains_key(hash) {
            return Err(SccpPendingClaimErrorV1::EntrypointClaimed);
        }
        if let Some(holder) = keys
            .exclusive
            .iter()
            .find_map(|key| self.exclusive.get(key))
        {
            return Err(SccpPendingClaimErrorV1::ExclusiveHeld { holder: *holder });
        }
        if !keys.content.is_empty()
            && keys
                .content
                .iter()
                .all(|key| self.content.contains_key(key))
        {
            return Err(SccpPendingClaimErrorV1::NothingNew);
        }
        Ok(())
    }

    /// Record the claim of `hash` over `keys`.
    ///
    /// # Errors
    ///
    /// Returns why the claim is refused; the index is unchanged on refusal.
    pub fn claim(
        &mut self,
        hash: HashOf<TransactionEntrypoint>,
        keys: SccpAdmissionKeysV1,
    ) -> Result<(), SccpPendingClaimErrorV1> {
        self.validate_claim(&hash, &keys)?;
        for key in &keys.exclusive {
            self.exclusive.insert(*key, hash);
        }
        for key in &keys.content {
            *self.content.entry(*key).or_insert(0) += 1;
        }
        self.by_entrypoint.insert(hash, keys);
        Ok(())
    }

    /// Release the claim of `hash`, returning its keys. Releasing an unclaimed hash is a no-op.
    pub fn release(&mut self, hash: &HashOf<TransactionEntrypoint>) -> Option<SccpAdmissionKeysV1> {
        let keys = self.by_entrypoint.remove(hash)?;
        for key in &keys.exclusive {
            if self.exclusive.get(key) == Some(hash) {
                self.exclusive.remove(key);
            }
        }
        for key in &keys.content {
            if let Some(count) = self.content.get_mut(key) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.content.remove(key);
                }
            }
        }
        Some(keys)
    }

    /// Return the exempt class of the queued transaction `hash`, if it holds a claim.
    #[must_use]
    pub fn class_of(&self, hash: &HashOf<TransactionEntrypoint>) -> Option<SccpExemptClassV1> {
        self.by_entrypoint.get(hash).map(|keys| keys.class)
    }

    /// Forget every claim.
    pub fn clear(&mut self) {
        self.by_entrypoint.clear();
        self.exclusive.clear();
        self.content.clear();
    }

    /// Return the number of claiming transactions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_entrypoint.len()
    }

    /// Return whether no transaction holds a claim.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_entrypoint.is_empty()
    }
}

/// Selects exempt-shaped SCCP transactions for one block within the per-block caps.
///
/// The proposer feeds candidate transactions in queue order with their
/// [`exempt_shape_of_entrypoint`]; [`Self::admit`] returns whether a candidate of `class` still
/// fits. Unshaped transactions always fit. [`block_exempt_cap_ok`] applies the same budget to
/// a whole block, so validation accepts exactly what selection admits.
#[derive(Debug, Clone)]
pub struct SccpExemptBlockBudgetV1 {
    remaining: Option<u32>,
    advanced_networks: BTreeSet<SccpNetworkV1>,
}

impl SccpExemptBlockBudgetV1 {
    /// Start a block budget from committed parameters (unbounded when SCCP is absent, since no
    /// transaction is then exempt).
    #[must_use]
    pub fn new(world: &(impl WorldReadOnly + ?Sized)) -> Self {
        Self {
            remaining: exempt_cap(world),
            advanced_networks: BTreeSet::new(),
        }
    }

    /// Return whether a transaction of `class` fits, consuming its share of the budget when it
    /// does.
    pub fn admit(&mut self, class: Option<SccpExemptClassV1>) -> bool {
        let Some(class) = class else {
            return true;
        };
        if self.remaining == Some(0) {
            return false;
        }
        if let SccpExemptClassV1::KeeperAdvance { network } = class
            && !self.advanced_networks.insert(network)
        {
            return false;
        }
        if let Some(remaining) = self.remaining.as_mut() {
            *remaining -= 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::{
        store,
        test_support::{blank_state, header, sample_signed_transaction},
    };
    use iroha_data_model::sccp::params::SccpParametersV1;

    fn entry(seed: u8) -> HashOf<TransactionEntrypoint> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32]))
    }

    fn attestation(authority: u8, entries: &[(u64, u8)]) -> SccpAdmissionKeysV1 {
        entries.iter().fold(
            SccpAdmissionKeysV1::new(SccpExemptClassV1::Attestation).with_exclusive(&authority),
            |keys, entry| keys.with_content(entry),
        )
    }

    #[test]
    fn derived_keys_are_domain_separated_by_class_and_role() {
        let a = SccpAdmissionKeysV1::derive_key(SccpExemptClassV1::Attestation, "content", &7_u64);
        let b = SccpAdmissionKeysV1::derive_key(SccpExemptClassV1::Fault, "content", &7_u64);
        let c =
            SccpAdmissionKeysV1::derive_key(SccpExemptClassV1::Attestation, "exclusive", &7_u64);
        let d = SccpAdmissionKeysV1::derive_key(SccpExemptClassV1::Attestation, "content", &8_u64);
        assert_eq!(
            a,
            SccpAdmissionKeysV1::derive_key(SccpExemptClassV1::Attestation, "content", &7_u64)
        );
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        let advance = |network| SccpExemptClassV1::KeeperAdvance { network };
        assert_ne!(
            SccpAdmissionKeysV1::derive_key(advance(SccpNetworkV1::BscMainnet), "e", &1_u8),
            SccpAdmissionKeysV1::derive_key(advance(SccpNetworkV1::TonMainnet), "e", &1_u8)
        );
    }

    #[test]
    fn pending_index_enforces_exclusive_limits_and_content_dedupe() {
        let mut index = SccpPendingIndexV1::default();
        assert!(index.is_empty());
        index
            .claim(entry(1), attestation(1, &[(10, 0), (11, 0)]))
            .expect("first batch");
        assert_eq!(
            index.class_of(&entry(1)),
            Some(SccpExemptClassV1::Attestation)
        );
        // Same authority: the per-kind pending limit is held.
        assert_eq!(
            index.claim(entry(2), attestation(1, &[(12, 0)])),
            Err(SccpPendingClaimErrorV1::ExclusiveHeld { holder: entry(1) })
        );
        // Another authority whose entries are all queued adds nothing.
        assert_eq!(
            index.claim(entry(3), attestation(2, &[(10, 0), (11, 0)])),
            Err(SccpPendingClaimErrorV1::NothingNew)
        );
        // One new entry is enough.
        index
            .claim(entry(3), attestation(2, &[(10, 0), (12, 1)]))
            .expect("adds a new entry");
        assert_eq!(
            index.claim(entry(3), attestation(3, &[(13, 0)])),
            Err(SccpPendingClaimErrorV1::EntrypointClaimed)
        );
        // A different kind never competes with an attestation batch of the same account.
        let binding = SccpAdmissionKeysV1::new(SccpExemptClassV1::KeyBinding).with_exclusive(&1_u8);
        index.claim(entry(4), binding).expect("per-kind limits");
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn released_claims_free_their_keys() {
        let mut index = SccpPendingIndexV1::default();
        index
            .claim(entry(1), attestation(1, &[(10, 0)]))
            .expect("claim");
        index
            .claim(entry(2), attestation(2, &[(10, 0), (11, 0)]))
            .expect("claim");
        assert!(index.release(&entry(1)).is_some());
        assert!(index.release(&entry(1)).is_none(), "release is idempotent");
        // (10, 0) is still queued by entry 2.
        assert_eq!(
            index.validate_claim(&entry(5), &attestation(5, &[(10, 0)])),
            Err(SccpPendingClaimErrorV1::NothingNew)
        );
        // The authority-1 limit is free again.
        index
            .validate_claim(&entry(6), &attestation(1, &[(20, 0)]))
            .expect("limit released");
        index.release(&entry(2));
        index
            .validate_claim(&entry(5), &attestation(5, &[(10, 0)]))
            .expect("content released");
        index
            .claim(entry(7), attestation(7, &[(1, 0)]))
            .expect("claim");
        index.clear();
        assert!(index.is_empty());
        assert_eq!(index.class_of(&entry(7)), None);
    }

    #[test]
    fn block_budget_caps_exempt_transactions_and_advances_per_network() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut unbounded = SccpExemptBlockBudgetV1::new(&*stx.world);
        assert!(unbounded.admit(Some(SccpExemptClassV1::Fault)));
        assert!(unbounded.admit(None));
        let mut parameters = SccpParametersV1::taira_default();
        parameters.max_exempt_transactions_per_block = 2;
        store::parameters::set(&mut stx, Some(parameters));
        assert_eq!(exempt_cap(&*stx.world), Some(2));
        let mut budget = SccpExemptBlockBudgetV1::new(&*stx.world);
        let advance = SccpExemptClassV1::KeeperAdvance {
            network: SccpNetworkV1::EthereumMainnet,
        };
        assert!(budget.admit(Some(advance)));
        assert!(
            !budget.admit(Some(advance)),
            "one exempt advance per network"
        );
        assert!(budget.admit(None), "ordinary transactions are not capped");
        assert!(budget.admit(Some(SccpExemptClassV1::Attestation)));
        assert!(
            !budget.admit(Some(SccpExemptClassV1::Attestation)),
            "cap reached"
        );
        assert!(budget.admit(None));
    }

    fn signed(seed: u8, instructions: Vec<InstructionBox>) -> SignedTransaction {
        use iroha_data_model::transaction::{FeePaymentIntent, TransactionBuilder};
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("deterministic seed");
        TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new([seed; 32]),
            )),
            AccountId::new(key_pair.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(key_pair.private_key())
    }

    fn shape(instructions: Vec<InstructionBox>) -> Option<SccpExemptClassV1> {
        exempt_shape(signed(0x61, instructions).payload())
    }

    fn self_registration(seed: u8) -> InstructionBox {
        let authority = signed(seed, Vec::new()).authority().clone();
        iroha_data_model::isi::Register::account(iroha_data_model::account::Account::new(authority))
            .into()
    }

    #[test]
    fn exempt_shapes_are_exact_instruction_layouts() {
        use crate::smartcontracts::isi::sccp::test_support::SampleInstructions as I;
        let one = |instruction: InstructionBox| shape(vec![instruction]);
        assert_eq!(
            one(I::attestations().into()),
            Some(SccpExemptClassV1::Attestation)
        );
        assert_eq!(
            one(I::set_bridge_key().into()),
            Some(SccpExemptClassV1::KeyBinding)
        );
        let mut revocation = I::set_bridge_key();
        revocation.public_key = None;
        assert_eq!(
            one(revocation.into()),
            None,
            "a revocation pays ordinary fees"
        );
        assert_eq!(one(I::fault().into()), Some(SccpExemptClassV1::Fault));
        assert_eq!(
            one(I::advance().into()),
            Some(SccpExemptClassV1::KeeperAdvance {
                network: I::advance().network
            })
        );
        assert_eq!(one(I::settle().into()), Some(SccpExemptClassV1::SelfClaim));
        assert_eq!(
            one(SettleSccpV1::refund(SccpNetworkV1::BscMainnet, 1, 0).into()),
            None,
            "only an inbound settle is a self-claim"
        );
        assert_eq!(one(I::inbound().into()), Some(SccpExemptClassV1::SelfClaim));
        assert_eq!(
            shape(vec![
                self_registration(0x61),
                I::advance().into(),
                I::advance().into(),
                I::inbound().into(),
            ]),
            Some(SccpExemptClassV1::SelfClaim)
        );
        for not_exempt in [
            vec![self_registration(0x62), I::inbound().into()],
            vec![I::inbound().into(), I::advance().into()],
            vec![I::attestations().into(), I::attestations().into()],
            vec![I::fault().into(), I::fault().into()],
            vec![I::record().into()],
            vec![I::void().into()],
            vec![I::equivocation().into()],
            vec![self_registration(0x61)],
            Vec::new(),
        ] {
            assert_eq!(shape(not_exempt), None);
        }
    }

    #[test]
    fn entrypoint_shapes_cover_external_and_sealed_reveal_transactions() {
        use crate::smartcontracts::isi::sccp::test_support::SampleInstructions as I;
        use iroha_data_model::transaction::signed::SealedTransactionReveal;
        let attestation = signed(0x63, vec![I::attestations().into()]);
        assert_eq!(
            exempt_shape_of_entrypoint(&TransactionEntrypoint::External(attestation.clone())),
            Some(SccpExemptClassV1::Attestation)
        );
        let reveal = SealedTransactionReveal::new(Hash::new(b"commitment"), attestation, [7; 32]);
        assert_eq!(
            exempt_shape_of_entrypoint(&TransactionEntrypoint::SealedReveal(reveal)),
            Some(SccpExemptClassV1::Attestation),
            "a sealed reveal counts like the transaction it reveals"
        );
        assert_eq!(
            exempt_shape_of_entrypoint(&TransactionEntrypoint::External(
                sample_signed_transaction()
            )),
            None
        );
    }

    #[test]
    fn block_cap_counts_shapes_against_the_parent_parameters() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let advance = |network| SccpExemptClassV1::KeeperAdvance { network };
        let classes = [SccpExemptClassV1::Attestation; 3];
        assert!(
            block_exempt_cap_ok(&*stx.world, &classes),
            "no cap without SCCP"
        );
        let mut parameters = SccpParametersV1::taira_default();
        parameters.max_exempt_transactions_per_block = 2;
        store::parameters::set(&mut stx, Some(parameters));
        assert!(block_exempt_cap_ok(&*stx.world, &classes[..2]));
        assert!(!block_exempt_cap_ok(&*stx.world, &classes));
        assert!(block_exempt_cap_ok(
            &*stx.world,
            &[
                advance(SccpNetworkV1::EthereumMainnet),
                advance(SccpNetworkV1::TonMainnet)
            ]
        ));
        assert!(!block_exempt_cap_ok(
            &*stx.world,
            &[advance(SccpNetworkV1::TonMainnet); 2]
        ));
    }

    #[test]
    fn classification_is_gated_by_the_shape_and_keeps_its_class() {
        use crate::smartcontracts::isi::sccp::test_support::SampleInstructions as I;
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let attestation = signed(0x64, vec![I::attestations().into()]);
        let ordinary = sample_signed_transaction();
        let _override = test_override::install(|transaction| {
            Ok(Some(
                SccpAdmissionKeysV1::new(SccpExemptClassV1::Attestation)
                    .with_exclusive(transaction.authority()),
            ))
        });
        assert_eq!(
            classify(&*stx.world, 3, &attestation),
            Ok(None),
            "nothing is exempt without SCCP"
        );
        store::parameters::set(&mut stx, Some(SccpParametersV1::taira_default()));
        assert_eq!(
            classify(&*stx.world, 3, &attestation).map(|keys| keys.map(|keys| keys.class)),
            Ok(Some(SccpExemptClassV1::Attestation))
        );
        assert_eq!(
            classify(&*stx.world, 3, &ordinary),
            Ok(None),
            "an unshaped transaction is never pre-verified"
        );
        let fault = signed(0x64, vec![I::fault().into()]);
        assert_eq!(
            classify(&*stx.world, 3, &fault),
            Ok(None),
            "keys of another class never apply"
        );
    }

    #[test]
    fn skeleton_classification_is_neutral() {
        let state = blank_state();
        let view = state.world_view();
        let transaction = sample_signed_transaction();
        assert_eq!(classify(&view, 1, &transaction), Ok(None));
        assert!(!allows_unregistered_authority(
            transaction.instructions(),
            transaction.authority()
        ));
        assert!(block_exempt_cap_ok(&view, &[SccpExemptClassV1::Fault]));
        assert_eq!(exempt_cap(&view), None);
        let reject = SccpAdmissionRejectV1::not_wired("attestation pre-verification", "ws31");
        assert!(reject.to_string().contains("TODO(ws31)"));
    }
}
