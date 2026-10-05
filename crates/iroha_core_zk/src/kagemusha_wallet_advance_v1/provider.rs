//! Exclusive provider handle over one custody root (spec §4.1; G2 design rev 2 §§1, 4, 5).
//!
//! [`KagemushaWalletProviderV1`] owns the durable store, the platform services and the
//! exclusive custody lock for its lifetime: a second opener of the same root gets
//! `Unavailable(Busy)`, and any lock error is `Unavailable` (the provider never proceeds
//! unlocked). Every mutating method takes `&mut self`, so operations on one root are serialized
//! inside the process; the bridge keeps one handle per process.
//!
//! Each operation starts with reconciliation of its slot (`reconcile.rs`). A slot whose marker
//! was verified by a full reconcile is cached; the next operation only re-lists `markers/` and
//! compares the current marker's exact bytes. This includes a selected head that waits for its
//! receipt (`Pending`): its unacknowledged files were rewritten once by the full reconcile, so
//! later status calls in this process do not rewrite them again, and calls with a transition
//! owner retry only the finishing steps. Any write with an unknown outcome drops the cache
//! entry (poisons the slot), so the next operation reconciles fully from disk.
//!
//! The durable store and the platform services are private to the provider: custody files
//! and the payment key are reached only through its operations.

use std::{collections::BTreeMap, marker::PhantomData};

use iroha_data_model::kagemusha::{
    KagemushaWalletCompletionRecordV1, KagemushaWalletRecoveryCapsuleV1,
};

use super::{
    KagemushaWalletProviderErrorV1,
    advance::{KagemushaWalletAdvanceCapsuleV1, KagemushaWalletTransitionOwnerV1},
    completion::KagemushaWalletCompletionFrameV1,
    layout::{
        KAGEMUSHA_WALLET_BALLAST_BYTES_V1, KAGEMUSHA_WALLET_CANARY_NAME_V1,
        KAGEMUSHA_WALLET_LOCK_NAME_V1, KAGEMUSHA_WALLET_PROBE_DIR_NAME_V1,
        KAGEMUSHA_WALLET_SLOTS_DIR_NAME_V1, KagemushaWalletCustodyDirV1,
        KagemushaWalletRootSentinelV1, KagemushaWalletSlotIdV1,
        kagemusha_wallet_is_staging_name_v1, kagemusha_wallet_list_dir_v1,
        kagemusha_wallet_list_slots_v1, kagemusha_wallet_prepare_root_v1,
        kagemusha_wallet_read_root_sentinel_v1,
    },
    marker::{KagemushaWalletDurableMarkerV1, KagemushaWalletMarkerRecordV1},
    platform::{
        KagemushaWalletEntryKindV1, KagemushaWalletFsV1, KagemushaWalletPlatformV1,
        KagemushaWalletUnavailableV1,
    },
    store::KagemushaWalletDurableStoreV1,
};

/// Node-local provider options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KagemushaWalletProviderOptionsV1 {
    /// Size of the capacity ballast; production uses the worst-case Advance footprint
    /// [`KAGEMUSHA_WALLET_BALLAST_BYTES_V1`].
    pub ballast_bytes: u64,
}

impl Default for KagemushaWalletProviderOptionsV1 {
    fn default() -> Self {
        Self {
            ballast_bytes: KAGEMUSHA_WALLET_BALLAST_BYTES_V1,
        }
    }
}

/// Reconciled state of one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KagemushaWalletSlotStatusV1 {
    /// The slot directory exists without an intent: an interrupted slot creation, never used.
    Empty,
    /// The intent is durable, the payment key is definitively absent and no marker exists:
    /// enrollment may continue on this slot while its challenge is live.
    IntentOnly,
    /// The slot was abandoned before any marker; its key (if any) is never used.
    SlotAbandoned,
    /// The generation-0 enrollment marker is current.
    Enrollment(KagemushaWalletMarkerRecordV1),
    /// A selected head waits for its receipt (signing unavailable, or no transition owner was
    /// given); the operation is performed but not released.
    Pending(KagemushaWalletMarkerRecordV1),
    /// A released head is current.
    Released(KagemushaWalletMarkerRecordV1),
    /// A terminal marker is current; operations are refused.
    Terminal(KagemushaWalletMarkerRecordV1),
}

impl KagemushaWalletSlotStatusV1 {
    /// Current marker, when one exists.
    #[must_use]
    pub fn marker(&self) -> Option<&KagemushaWalletMarkerRecordV1> {
        match self {
            Self::Enrollment(record)
            | Self::Pending(record)
            | Self::Released(record)
            | Self::Terminal(record) => Some(record),
            Self::Empty | Self::IntentOnly | Self::SlotAbandoned => None,
        }
    }
}

/// Verified durable marker of one slot and the status it produced.
#[derive(Debug, Clone)]
pub(super) struct CachedSlotV1 {
    pub(super) durable: KagemushaWalletDurableMarkerV1,
    pub(super) status: KagemushaWalletSlotStatusV1,
}

/// Stock-phone Advance provider over one custody root.
///
/// `C` and `R` are the frozen capsule and completion types (the G1 objects by default).
pub struct KagemushaWalletProviderV1<
    F: KagemushaWalletFsV1,
    P,
    C = KagemushaWalletRecoveryCapsuleV1,
    R = KagemushaWalletCompletionRecordV1,
> {
    pub(super) store: KagemushaWalletDurableStoreV1<F>,
    pub(super) platform: P,
    pub(super) scheme_id: [u8; 32],
    pub(super) options: KagemushaWalletProviderOptionsV1,
    pub(super) cache: BTreeMap<KagemushaWalletSlotIdV1, CachedSlotV1>,
    pub(super) pending_reasons: BTreeMap<KagemushaWalletSlotIdV1, KagemushaWalletProviderErrorV1>,
    sentinel: KagemushaWalletRootSentinelV1,
    _lock: F::Lock,
    _frames: PhantomData<fn() -> (C, R)>,
}

impl<F, P, C, R> KagemushaWalletProviderV1<F, P, C, R>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    /// Open the custody root behind `fs` for slots of `scheme_id` (design R0): require
    /// available protected storage, take the exclusive lock, then adopt the root sentinel or
    /// create it on a root that holds nothing else.
    ///
    /// # Errors
    ///
    /// `Unavailable` when storage is locked, the lock is held elsewhere (`Busy`) or any read
    /// fails; `UnavailableCustodyData` for an invalid sentinel, a sentinel without `slots/`, or
    /// a root that holds custody entries without one; `Invalid` for a zero ballast size; the
    /// write errors of the sentinel adoption and directory creation (a sentinel is rewritten to
    /// a fresh inode on every open, two syncs).
    pub fn open(
        fs: F,
        platform: P,
        scheme_id: [u8; 32],
        options: KagemushaWalletProviderOptionsV1,
    ) -> Result<Self, KagemushaWalletProviderErrorV1> {
        if options.ballast_bytes == 0 {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "options.ballast_bytes",
            });
        }
        platform
            .storage_state()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?;
        let store = KagemushaWalletDurableStoreV1::new(fs);
        let lock = store
            .lock_exclusive()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?;
        if kagemusha_wallet_read_root_sentinel_v1(&store)?.is_none() {
            // A fresh root holds at most the lock, the canary, staging files and the skeleton
            // directories of an interrupted first open (their emptiness is checked when they
            // are recreated); anything else without a sentinel is never adopted as an empty
            // wallet.
            let entries =
                kagemusha_wallet_list_dir_v1(&store, &KagemushaWalletCustodyDirV1::root())?
                    .unwrap_or_default();
            if entries.iter().any(|entry| {
                let skeleton = entry.kind == KagemushaWalletEntryKindV1::Directory
                    && (entry.name == KAGEMUSHA_WALLET_PROBE_DIR_NAME_V1
                        || entry.name == KAGEMUSHA_WALLET_SLOTS_DIR_NAME_V1);
                !skeleton
                    && entry.name != KAGEMUSHA_WALLET_LOCK_NAME_V1
                    && entry.name != KAGEMUSHA_WALLET_CANARY_NAME_V1
                    && !kagemusha_wallet_is_staging_name_v1(&entry.name)
            }) {
                return Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                    object: "root sentinel",
                });
            }
        }
        let sentinel = kagemusha_wallet_prepare_root_v1(&store)?;
        Ok(Self {
            store,
            platform,
            scheme_id,
            options,
            cache: BTreeMap::new(),
            pending_reasons: BTreeMap::new(),
            sentinel,
            _lock: lock,
            _frames: PhantomData,
        })
    }

    /// Durable store (provider-internal; tests).
    #[cfg(test)]
    pub(super) fn store(&self) -> &KagemushaWalletDurableStoreV1<F> {
        &self.store
    }

    /// Platform services (provider-internal; tests: the payment key is reached only through
    /// the provider's operations).
    #[cfg(test)]
    pub(super) fn platform(&self) -> &P {
        &self.platform
    }

    /// Why the selected head of `slot` was last left unreleased in this process, when it was:
    /// signing unavailable, an unknown write outcome, a full disk, or a transition-owner
    /// failure. The operation itself stays performed and pending; this is a diagnostic only.
    #[must_use]
    pub fn pending_reason(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Option<KagemushaWalletProviderErrorV1> {
        self.pending_reasons.get(slot).copied()
    }

    /// Scheme of every slot under this root.
    pub fn scheme_id(&self) -> &[u8; 32] {
        &self.scheme_id
    }

    /// Root sentinel.
    pub fn sentinel(&self) -> &KagemushaWalletRootSentinelV1 {
        &self.sentinel
    }

    /// Options.
    pub fn options(&self) -> &KagemushaWalletProviderOptionsV1 {
        &self.options
    }

    /// Every slot under the root, ascending.
    ///
    /// # Errors
    ///
    /// `Unavailable` on a listing error and `UnexpectedEntry` for a foreign entry.
    pub fn slots(&self) -> Result<Vec<KagemushaWalletSlotIdV1>, KagemushaWalletProviderErrorV1> {
        let mut slots = kagemusha_wallet_list_slots_v1(&self.store)?.unwrap_or_default();
        slots.sort_unstable();
        Ok(slots)
    }

    /// Reconcile `slot` without signing: a selected head stays `Pending`.
    ///
    /// # Errors
    ///
    /// As [`Self::reconcile`].
    pub fn status(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<KagemushaWalletSlotStatusV1, KagemushaWalletProviderErrorV1> {
        self.reconcile_slot(slot, None)
    }

    /// Reconcile `slot` (startup, after an uncertain write and before every operation) and
    /// finish a selected head offline with `owner`.
    ///
    /// # Errors
    ///
    /// `Unavailable` (retry) for any read, lock, key-store or storage error; `Uncertain` when a
    /// write's outcome is unknown; `UnavailableCustodyData` for a current marker or bound
    /// object missing in every copy; `LostCustody` and `KeyLost` for custody loss, which is
    /// shown and never acted on destructively; `UnexpectedEntry` for a foreign entry.
    pub fn reconcile(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        owner: &dyn KagemushaWalletTransitionOwnerV1<C, R>,
    ) -> Result<KagemushaWalletSlotStatusV1, KagemushaWalletProviderErrorV1> {
        self.reconcile_slot(slot, Some(owner))
    }

    /// Current boot identity.
    pub(super) fn boot(&self) -> Result<[u8; 32], KagemushaWalletUnavailableV1> {
        self.platform.boot_id()
    }

    /// Require available protected storage now. Called before and after every answer from
    /// which absence or loss would be concluded (design R0, I9): on Android, credential-
    /// encrypted names read as absent while their keys are evicted.
    ///
    /// # Errors
    ///
    /// `Unavailable` with the platform's reason.
    pub(super) fn require_storage(&self) -> Result<(), KagemushaWalletProviderErrorV1> {
        self.platform
            .storage_state()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)
    }

    /// Probe the payment key of `slot` inside storage-state brackets: an `Absent` or
    /// mismatching answer counts only while storage was available before and after it.
    ///
    /// # Errors
    ///
    /// `Unavailable` when storage is locked at either bracket.
    pub(super) fn probe_key(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<
        super::platform::KagemushaWalletProbeV1<
            iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
        >,
        KagemushaWalletProviderErrorV1,
    > {
        self.require_storage()?;
        let answer = self.platform.key_probe(slot);
        self.require_storage()?;
        Ok(answer)
    }

    /// Forget the verified state of `slot`; the next operation reconciles it fully.
    pub(super) fn poison(&mut self, slot: &KagemushaWalletSlotIdV1) {
        self.cache.remove(slot);
    }

    /// Record why the selected head of `slot` stays unreleased.
    pub(super) fn note_pending(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        reason: KagemushaWalletProviderErrorV1,
    ) {
        self.pending_reasons.insert(*slot, reason);
    }

    /// Remember a verified stable state of `slot`.
    pub(super) fn remember(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        durable: KagemushaWalletDurableMarkerV1,
        status: KagemushaWalletSlotStatusV1,
    ) {
        self.cache.insert(*slot, CachedSlotV1 { durable, status });
    }

    /// Poison `slot` when `result` is an error (an unknown write outcome among them), so the
    /// next operation reconciles it fully from disk.
    pub(super) fn guard<T>(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        result: Result<T, KagemushaWalletProviderErrorV1>,
    ) -> Result<T, KagemushaWalletProviderErrorV1> {
        if result.is_err() {
            self.poison(slot);
        }
        result
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
