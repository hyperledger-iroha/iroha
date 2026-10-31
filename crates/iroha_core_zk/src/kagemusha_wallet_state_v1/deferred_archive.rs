//! Source-selected receipt custody and scheduler-admitted deferred ArchiveSent dispatch.
//!
//! Intake publishes exact bytes before verification. Only the first verified original for
//! an actual Send becomes its permanent Archive anchor. All indexes share the existing
//! provider archive and selected manifest; filesystem enumeration never discovers work.

use super::*;

// Native Request setup rejects the zero identity. The current authenticated sessions
// map selects this bounded metadata leaf without changing the fixed Manifest layout.
const DEFERRED_INDEX_KEY: [u8; 32] = [0; 32];

mod clock;
mod drain;
pub use drain::{DeferredArchiveDrainV1, DeferredArchiveWaitV1};

#[derive(Debug, Default, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::DeferredArchiveIndex")]
pub(super) struct DeferredIndex {
    receipts: IndexRoot,
    sends: IndexRoot,
    unverified: IndexRoot,
    ready: IndexRoot,
    next_ordinal: u128,
    count: u64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::DeferredReceiptState")]
enum ReceiptState {
    Unverified,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::DeferredReceipt")]
struct Receipt {
    send_request: [u8; 32],
    capsule: [u8; 32],
    commitment: [u8; 32],
    operation: [u8; 32],
    original: [u8; 32],
    ordinal: u128,
    first_unix_ms: Option<u64>,
    state: ReceiptState,
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::DeferredSend")]
struct Send {
    anchor: [u8; 32],
    ordinal: u128,
    complete: bool,
}

/// A receipt locator issued by Native. Every use reopens its source-selected record.
/// Possession grants no delivery verdict, Archive admission or monetary permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredArchiveReceiptV1 {
    scheme: [u8; 32],
    wallet: [u8; 32],
    identity: [u8; 32],
}

/// Exact immutable inputs for one queued Archive, selected by Native from durable custody.
/// This is DATA, not an idle-dispatch reservation; callers must recheck shared admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredArchiveEntryV1 {
    receipt: DeferredArchiveReceiptV1,
    send_request: [u8; 32],
    original: Vec<u8>,
    ordinal: u128,
}
impl DeferredArchiveEntryV1 {
    /// The exact local request mapping of the committed Send.
    #[must_use]
    pub const fn send_request(&self) -> &[u8; 32] {
        &self.send_request
    }
    /// The immutable first verified Credited original, never a newer replacement.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// The arrival ordinal selected when these exact bytes first became durable.
    #[must_use]
    pub const fn ordinal(&self) -> u128 {
        self.ordinal
    }
}

/// Fresh local wall-clock age observation, separate from authenticated monetary time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredArchiveAgeV1 {
    /// At least 24 hours by the current local wall clock.
    Due,
    /// Less than 24 hours by the current local wall clock.
    NotDue,
    /// No first/current observation, or no queued item.
    Unavailable,
    /// The current clock precedes the retained first observation.
    Regression,
}

/// Rust-only housekeeping hint. A wake must re-read source and clock; it grants no admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredArchiveWakeHintV1 {
    /// Empty queue or an immutable first anchor with no recorded wall observation.
    NoAgeWake,
    /// Fresh local age reached the threshold; do not repeatedly arm a zero-delay timer.
    Due,
    /// Recheck after a bounded relative monotonic wait, never accumulated age authority.
    RecheckAfter(std::time::Duration),
}

// Current source-selected queue data, including the private immutable age origin.
struct SelectedDeferredArchive {
    count: u64,
    oldest: Option<DeferredArchiveEntryV1>,
    first_unix_ms: Option<u64>,
}

/// A current queue view. Neither its count nor age grants permission across another call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredArchiveSummaryV1 {
    /// Distinct actual Sends with verified anchors not yet reconciled as exact Complete.
    pub count: u64,
    /// Oldest immutable receipt arrival among the currently queued Sends.
    pub oldest: Option<DeferredArchiveEntryV1>,
    /// Fresh age of that oldest receipt. Charging and count are independent alternatives.
    pub oldest_age: DeferredArchiveAgeV1,
}

// The leading zero half reserves [0xff;32] as an exclusive predecessor sentinel, even
// for ordinal zero. New ordinals start at one and never wrap or get reused after removal.
fn order_key(ordinal: u128) -> [u8; 32] {
    manifest::sequence_key(u128::MAX - ordinal)
}

impl DeferredIndex {
    fn next_intake_ordinal(&self) -> Result<u128, Error> {
        self.next_ordinal
            .checked_add(1)
            .ok_or(Error::Invalid("deferred intake ordinal exhausted"))
    }

    fn insert_ready(
        &mut self,
        store: &mut impl ObjectStore,
        capsule: [u8; 32],
        anchor: [u8; 32],
        ordinal: u128,
    ) -> Result<(), Error> {
        if ordinal == 0 || ordinal > self.next_ordinal || self.sends.get(store, &capsule)?.is_some()
        {
            return Err(Error::WitnessLost("deferred new anchor"));
        }
        let count = self
            .count
            .checked_add(1)
            .ok_or(Error::Invalid("deferred count exhausted"))?;
        let send = Send {
            anchor,
            ordinal,
            complete: false,
        };
        self.sends = self.sends.set(store, capsule, &archive::encode(&send)?)?;
        self.ready = self.ready.set(store, order_key(ordinal), &capsule)?;
        self.count = count;
        Ok(())
    }
    fn validate(&self) -> Result<(), Error> {
        if (self.count == 0) != (self.ready == IndexRoot::default())
            || (self.next_ordinal == 0
                && (self.receipts != IndexRoot::default()
                    || self.sends != IndexRoot::default()
                    || self.unverified != IndexRoot::default()))
        {
            return Err(Error::WitnessLost("deferred queue roots/count"));
        }
        Ok(())
    }
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn deferred_archive_index(
        &mut self,
        manifest: &manifest::Manifest,
    ) -> Result<DeferredIndex, Error> {
        let Some(bytes) = manifest
            .sessions
            .get(&mut self.archive, &DEFERRED_INDEX_KEY)?
        else {
            // Only authenticated absence means no deferred intake was ever selected.
            return Ok(DeferredIndex::default());
        };
        let index: DeferredIndex = archive::decode(&bytes)?;
        index.validate()?;
        Ok(index)
    }

    fn select_deferred_archive_index(
        &mut self,
        manifest: &mut manifest::Manifest,
        index: &DeferredIndex,
    ) -> Result<(), Error> {
        index.validate()?;
        manifest.sessions = manifest.sessions.set(
            &mut self.archive,
            DEFERRED_INDEX_KEY,
            &archive::encode(index)?,
        )?;
        // This stages an immutable path only. The caller's existing compare-and-publish
        // barrier must durably select it before acknowledging any queue transition.
        Ok(())
    }

    fn deferred_handle(&self, identity: [u8; 32]) -> DeferredArchiveReceiptV1 {
        DeferredArchiveReceiptV1 {
            scheme: self.scheme_id,
            wallet: self.wallet_id,
            identity,
        }
    }

    fn require_deferred_handle(&self, receipt: &DeferredArchiveReceiptV1) -> Result<(), Error> {
        if receipt.scheme != self.scheme_id
            || receipt.wallet != self.wallet_id
            || receipt.identity == [0; 32]
        {
            return Err(Error::Invalid("foreign deferred receipt"));
        }
        Ok(())
    }

    fn deferred_receipt(
        &mut self,
        index: &DeferredIndex,
        identity: &[u8; 32],
    ) -> Result<Option<(Receipt, Vec<u8>)>, Error> {
        index.validate()?;
        let Some(bytes) = index.receipts.get(&mut self.archive, identity)? else {
            return Ok(None);
        };
        let record: Receipt = archive::decode(&bytes)?;
        if record.ordinal == 0 || record.ordinal > index.next_ordinal || record.original == [0; 32]
        {
            return Err(Error::WitnessLost("deferred receipt metadata"));
        }
        let original = self
            .archive
            .read_object(&record.original, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        let binding = self.committed_send_binding(&record.send_request)?;
        if original.is_empty()
            || record.capsule != binding.capsule
            || record.commitment != binding.commitment
            || record.operation != binding.operation
            || NativeIntentV1::archive_identity(&binding.capsule, &self.wallet_id, &original)
                != *identity
        {
            return Err(Error::WitnessLost("deferred receipt/Send binding"));
        }
        Ok(Some((record, original)))
    }

    fn require_deferred_receipt(
        &mut self,
        index: &DeferredIndex,
        receipt: &DeferredArchiveReceiptV1,
    ) -> Result<(Receipt, Vec<u8>), Error> {
        self.require_deferred_handle(receipt)?;
        self.deferred_receipt(index, &receipt.identity)?
            .ok_or(Error::WitnessLost("selected deferred receipt"))
    }

    fn deferred_send(
        &mut self,
        index: &DeferredIndex,
        capsule: &[u8; 32],
    ) -> Result<Option<Send>, Error> {
        let Some(bytes) = index.sends.get(&mut self.archive, capsule)? else {
            return Ok(None);
        };
        let send: Send = archive::decode(&bytes)?;
        let (anchor, _) = self
            .deferred_receipt(index, &send.anchor)?
            .ok_or(Error::WitnessLost("deferred anchor receipt"))?;
        if anchor.state != ReceiptState::Verified
            || anchor.capsule != *capsule
            || send.ordinal != anchor.ordinal
        {
            return Err(Error::WitnessLost("deferred Send anchor"));
        }
        let ready = index
            .ready
            .get(&mut self.archive, &order_key(send.ordinal))?;
        if if send.complete {
            ready.is_some()
        } else {
            ready.as_deref() != Some(capsule.as_slice())
        } {
            return Err(Error::WitnessLost("deferred ready membership"));
        }
        Ok(Some(send))
    }

    /// Durably retain exact receipt bytes before any receipt verification or delivery claim.
    /// A duplicate returns its original locator without changing age, FIFO or capacity policy.
    /// # Errors
    /// Pending custody requires selected recovery first. Foreign Send, unavailable storage,
    /// missing selected objects, exhausted ordinal or insufficient fresh storage refuse intake.
    pub fn retain_deferred_credited(
        &mut self,
        send_request: &[u8; 32],
        original: &[u8],
    ) -> Result<DeferredArchiveReceiptV1, Error> {
        self.retain_deferred_credited_with_clock(send_request, original, || {
            clock::observe().ok().map(|v| v.unix_ms)
        })
    }

    pub(super) fn retain_deferred_credited_with_clock(
        &mut self,
        send_request: &[u8; 32],
        original: &[u8],
        observe: impl FnOnce() -> Option<u64>,
    ) -> Result<DeferredArchiveReceiptV1, Error> {
        if original.is_empty() || original.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
            return Err(Error::Invalid("deferred Credited size"));
        }
        let (selected, mut manifest) = self.sync_manifest()?;
        let mut index = self.deferred_archive_index(&manifest)?;
        let binding = self.committed_send_binding(send_request)?;
        let identity =
            NativeIntentV1::archive_identity(&binding.capsule, &self.wallet_id, original);
        if let Some((_, held)) = self.deferred_receipt(&index, &identity)? {
            if held != original {
                return Err(Error::OperationConflict);
            }
            return Ok(self.deferred_handle(identity));
        }
        let ordinal = index.next_intake_ordinal()?;
        self.require_deferred_archive_capacity(original.len(), false)?;
        let first_unix_ms = observe();
        let original = self
            .archive
            .write_object(original, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        let record = Receipt {
            send_request: *send_request,
            capsule: binding.capsule,
            commitment: binding.commitment,
            operation: binding.operation,
            original,
            ordinal,
            first_unix_ms,
            state: ReceiptState::Unverified,
        };
        index.receipts =
            index
                .receipts
                .set(&mut self.archive, identity, &archive::encode(&record)?)?;
        index.unverified =
            index
                .unverified
                .set(&mut self.archive, order_key(ordinal), &identity)?;
        index.next_ordinal = ordinal;
        self.select_deferred_archive_index(&mut manifest, &index)?;
        // This source-selection barrier drains grouped object writes before acknowledging
        // custody. A successful raw put alone would leave an undiscoverable staged receipt.
        self.publish_manifest(selected, &manifest)?;
        Ok(self.deferred_handle(identity))
    }

    /// Recover the oldest not-yet-verified exact intake without scanning historical files.
    /// # Errors
    /// Pending custody, unavailable or missing selected receipt/index objects never mean empty.
    pub fn next_deferred_credited(&mut self) -> Result<Option<DeferredArchiveReceiptV1>, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let index = self.deferred_archive_index(&manifest)?;
        index.validate()?;
        let Some((key, value)) = index
            .unverified
            .predecessor(&mut self.archive, &[255; 32])?
        else {
            return Ok(None);
        };
        let identity: [u8; 32] = value
            .try_into()
            .map_err(|_| Error::WitnessLost("deferred intake locator"))?;
        let receipt = self.deferred_handle(identity);
        let (record, _) = self.require_deferred_receipt(&index, &receipt)?;
        if record.state != ReceiptState::Unverified || order_key(record.ordinal) != key {
            return Err(Error::WitnessLost("deferred intake order"));
        }
        Ok(Some(receipt))
    }

    fn finish_deferred_verification(
        &mut self,
        receipt: &DeferredArchiveReceiptV1,
        state: ReceiptState,
    ) -> Result<(), Error> {
        let (selected, mut manifest) = self.sync_manifest()?;
        let mut index = self.deferred_archive_index(&manifest)?;
        let (mut record, _) = self.require_deferred_receipt(&index, receipt)?;
        if record.state != ReceiptState::Unverified {
            return Err(Error::WitnessLost("deferred verification state changed"));
        }
        if index
            .unverified
            .get(&mut self.archive, &order_key(record.ordinal))?
            .as_deref()
            != Some(receipt.identity.as_slice())
        {
            return Err(Error::WitnessLost("deferred verification intake"));
        }
        if state == ReceiptState::Verified && self.deferred_send(&index, &record.capsule)?.is_none()
        {
            index.insert_ready(
                &mut self.archive,
                record.capsule,
                receipt.identity,
                record.ordinal,
            )?;
        }
        record.state = state;
        index.receipts = index.receipts.set(
            &mut self.archive,
            receipt.identity,
            &archive::encode(&record)?,
        )?;
        index.unverified = index
            .unverified
            .remove(&mut self.archive, &order_key(record.ordinal))?;
        self.select_deferred_archive_index(&mut manifest, &index)?;
        self.publish_manifest(selected, &manifest)?;
        Ok(())
    }

    /// Verify a durably selected receipt and retain the first valid Archive anchor for its Send.
    /// Newer receipts update only delivery evidence; they never replace that anchor or its age.
    /// Definitively invalid intake is durably rejected so it cannot block the retry FIFO.
    /// # Errors
    /// Verification, missing custody or publication errors grant no delivery verdict. Local
    /// resource/storage refusals remain retryable and never discard the selected original.
    pub fn verify_deferred_credited(
        &mut self,
        receipt: &DeferredArchiveReceiptV1,
    ) -> Result<CreditProjectionV1, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let index = self.deferred_archive_index(&manifest)?;
        let (record, original) = self.require_deferred_receipt(&index, receipt)?;
        if record.state == ReceiptState::Rejected {
            return Err(Error::ReceiptRejected("deferred Credited rejected"));
        }
        let existing = self.deferred_send(&index, &record.capsule)?;
        if record.state == ReceiptState::Unverified {
            self.require_deferred_archive_capacity(0, existing.is_none())?;
        }
        let result = (|| {
            // These bytes have already passed their object hash and selected-record binding.
            // A completed malformed decode is receipt invalidity, not missing archive custody.
            let evidence = read_model_original(
                || {
                    let evidence: KagemushaWalletCreditedV1 = norito::decode_canonical_with_limits(
                        &original,
                        norito::canonical_decode_limits(original.len()),
                    )?;
                    evidence.validate()?;
                    Ok(evidence)
                },
                Error::ReceiptRejected("deferred Credited encoding or model"),
            )?;
            let credit = match &evidence.evidence {
                KagemushaWalletCreditedEvidenceV1::Receive { package } => {
                    let KagemushaWalletEffectV1::Receive { credit_id, .. } =
                        package.statement.effect
                    else {
                        return Err(Error::ReceiptRejected("deferred Credited Receive kind"));
                    };
                    credit_id
                }
                KagemushaWalletCreditedEvidenceV1::Status { status } => status.opening.credit_id,
            };
            if evidence.scheme_id != self.scheme_id {
                return Err(Error::ReceiptRejected("deferred Credited scheme"));
            }
            let anchor = if let Some(send) = &existing {
                self.require_deferred_receipt(&index, &self.deferred_handle(send.anchor))?
                    .1
            } else {
                original.clone()
            };
            let intent = self.source_bound_archive_intent(&record.send_request, &anchor)?;
            let held = intent
                .archive_request()
                .ok_or(Error::WitnessLost("deferred Archive originals"))?;
            let request: KagemushaWalletRequestV1 = archive::decode(&held.request)?;
            if credit != request.body.credit_id() {
                return Err(Error::ReceiptRejected("deferred Credited Send binding"));
            }
            if let Some(send) = &existing {
                let (_, anchor) =
                    self.require_deferred_receipt(&index, &self.deferred_handle(send.anchor))?;
                self.verified_delivery_with_anchor(&record.send_request, &anchor, &original)
            } else {
                self.verified_delivery_projection(&record.send_request, &original)
            }
        })();
        let projection = match result {
            Ok(projection) => projection,
            Err(error) => {
                if record.state == ReceiptState::Unverified
                    && matches!(&error, Error::ReceiptRejected(_))
                {
                    self.finish_deferred_verification(receipt, ReceiptState::Rejected)?;
                }
                return Err(error);
            }
        };
        if record.state == ReceiptState::Verified {
            return Ok(projection);
        }
        if existing.is_none() {
            self.retain_deferred_archive_intent(&record.send_request, &original)?;
        }
        // Verification and exact intent retention may reconcile/publish metadata. Reload
        // their current manifest before selecting the queue transition, never stale-clone it.
        self.finish_deferred_verification(receipt, ReceiptState::Verified)?;
        Ok(projection)
    }

    /// Read the distinct-Send count, oldest immutable anchor and freshly observed local age.
    /// This is an advisory view; a later dispatch must recheck all shared admission gates.
    /// # Errors
    /// Pending custody, unavailable storage or inconsistent selected queue data.
    pub fn deferred_archive_summary(&mut self) -> Result<DeferredArchiveSummaryV1, Error> {
        let selected = self.selected_deferred_archive()?;
        let oldest_age = match clock::age(selected.first_unix_ms, clock::observe()) {
            clock::Age::Due => DeferredArchiveAgeV1::Due,
            clock::Age::NotDue => DeferredArchiveAgeV1::NotDue,
            clock::Age::Unavailable => DeferredArchiveAgeV1::Unavailable,
            clock::Age::Regression => DeferredArchiveAgeV1::Regression,
        };
        Ok(DeferredArchiveSummaryV1 {
            count: selected.count,
            oldest: selected.oldest,
            oldest_age,
        })
    }

    /// Suggest a bounded timer for the currently selected oldest anchor, without exporting
    /// its timestamp. Requery after every wake or event; a prior Due is never retained.
    /// Relative delays range from one to fifteen minutes. An absent first observation has
    /// no age timer; an unavailable or regressed current clock receives a coarse recheck.
    /// # Errors
    /// Pending, unavailable custody or inconsistent selected queue data remains an error.
    pub fn deferred_archive_wake_hint(&mut self) -> Result<DeferredArchiveWakeHintV1, Error> {
        let selected = self.selected_deferred_archive()?;
        Ok(clock::wake_hint(selected.first_unix_ms, clock::observe()))
    }

    fn selected_deferred_archive(&mut self) -> Result<SelectedDeferredArchive, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let index = self.deferred_archive_index(&manifest)?;
        index.validate()?;
        let first = index.ready.predecessor(&mut self.archive, &[255; 32])?;
        let (oldest, first_time) = if let Some((key, value)) = first {
            let capsule: [u8; 32] = value
                .try_into()
                .map_err(|_| Error::WitnessLost("deferred ready locator"))?;
            let send = self
                .deferred_send(&index, &capsule)?
                .ok_or(Error::WitnessLost("deferred ready Send"))?;
            let receipt = self.deferred_handle(send.anchor);
            let (record, original) = self.require_deferred_receipt(&index, &receipt)?;
            if send.complete || key != order_key(record.ordinal) {
                return Err(Error::WitnessLost("deferred ready order"));
            }
            (
                Some(DeferredArchiveEntryV1 {
                    receipt,
                    send_request: record.send_request,
                    original,
                    ordinal: record.ordinal,
                }),
                record.first_unix_ms,
            )
        } else {
            (None, None)
        };
        if oldest.is_none() != (index.count == 0) {
            return Err(Error::WitnessLost("deferred queue count"));
        }
        Ok(SelectedDeferredArchive {
            count: index.count,
            oldest,
            first_unix_ms: first_time,
        })
    }

    /// Reconcile only this exact first-anchor Archive. Only Native Complete removes ready work.
    /// Generic resume results are never attributed to this entry; callers must requery here.
    /// # Errors
    /// Foreign/stale entry, lost custody or uncertain publication retains the queue. A Pending
    /// head may be queried, but must be recovered before any Released-only dequeue publication.
    pub fn reconcile_deferred_archive(
        &mut self,
        entry: &DeferredArchiveEntryV1,
    ) -> Result<RequestStatusV1, Error> {
        self.status()?;
        let (_, manifest) = self.manifest()?;
        let index = self.deferred_archive_index(&manifest)?;
        let (record, original) = self.require_deferred_receipt(&index, &entry.receipt)?;
        let send = self
            .deferred_send(&index, &record.capsule)?
            .ok_or(Error::WitnessLost("deferred reconciliation Send"))?;
        if send.anchor != entry.receipt.identity
            || record.send_request != entry.send_request
            || original != entry.original
            || record.ordinal != entry.ordinal
        {
            return Err(Error::Invalid("deferred reconciliation anchor"));
        }
        let outcome = self.credited_status_for_send(&record.send_request, &original)?;
        if !matches!(&outcome, RequestStatusV1::Outcome(Completion::Complete(_))) || send.complete {
            return Ok(outcome);
        }
        let (selected, mut manifest) = self.sync_manifest()?;
        let mut index = self.deferred_archive_index(&manifest)?;
        let mut current = self
            .deferred_send(&index, &record.capsule)?
            .ok_or(Error::WitnessLost("deferred reconciliation binding"))?;
        if current.anchor != send.anchor || current.ordinal != send.ordinal || current.complete {
            return Err(Error::WitnessLost("deferred reconciliation changed"));
        }
        index.count = index
            .count
            .checked_sub(1)
            .ok_or(Error::WitnessLost("deferred count underflow"))?;
        index.ready = index
            .ready
            .remove(&mut self.archive, &order_key(record.ordinal))?;
        current.complete = true;
        index.sends = index.sends.set(
            &mut self.archive,
            record.capsule,
            &archive::encode(&current)?,
        )?;
        self.select_deferred_archive_index(&mut manifest, &index)?;
        self.publish_manifest(selected, &manifest)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests;
