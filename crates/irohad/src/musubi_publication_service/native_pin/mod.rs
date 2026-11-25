//! Portable original pin custody, fresh native Check rounds and ordinary Queue dispatch.
//!
//! One bounded blocking caller owns this coordinator. No Tokio task may poll its synchronous
//! custody/native work directly. Check consumption and Queue insertion are separate effects;
//! native Advance CAS and the ordinary admission/execution path retain their normal authority.
//! Local files never mint currentness, finality, or an externally sealed rollback floor.

#[cfg(test)]
mod current_check_tests;
#[cfg(test)]
mod native_tests;

mod authorization;
mod inventory;
pub(super) mod slot;

pub use authorization::NativePinAuthorizationV1;
pub(super) use authorization::ReservedFeesV1;

use super::{
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationFinalizedPinRegistrationQueryV1,
    MusubiPublicationFinalizedPinRegistrationReaderV1, MusubiPublicationPinTransactionSignerV1,
    MusubiPublicationPrivateServiceContextV1,
};
use eyre::{Result, ensure};
use inventory::{Operation, Store};
use iroha_config::parameters::actual::MusubiPublicationPaidPinPolicy;
use iroha_core::{
    query::musubi_pin_outbox::{
        MusubiPinOutboxCheckAttemptFailureV1, MusubiPinOutboxCheckBindingFailureV1,
        MusubiPinOutboxCheckErrorV1, MusubiPinOutboxCheckExpectedV1,
        MusubiPinOutboxCurrentReadbackV1, PendingMusubiPinOutboxCheckV1,
        PreparedMusubiPinOutboxCheckV1, begin_musubi_pin_outbox_check_v1,
    },
    queue::Queue,
    state::{
        State, StateReadOnly as _, StateReadOnlyWithTransactions as _, TransactionsReadOnly as _,
        WorldReadOnly as _, WorldStateSnapshot as _,
    },
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    isi::{InstructionBox, musubi::AdvanceMusubiPinOutboxV1, sorafs::RegisterPinManifest},
    musubi::{MusubiPinOutboxCheckExpectationV1, MusubiPinOutboxCheckFloorV1},
    sorafs::pin_registry::{ManifestDigest, PinManifestRecord, StorageClass},
    transaction::{Executable, signed::pin_allocation::AllocatedPinTransactionV1},
};
use iroha_musubi_service::{MusubiPublicationServiceClockV1, NativeMusubiPinSessionV1};
use mv::storage::StorageReadOnly as _;
use slot::{Slot, SlotKind, SlotRequest};
use std::{path::Path, sync::Arc, time::Instant};

/// Stage awaiting original native effects or another caller-bounded observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMusubiPinPhaseV1 {
    /// One original fresh Check is being signed, admitted or independently reconciled.
    Check,
    /// Original initialization Advance awaits native execution; it is never resent.
    Initialize,
    /// Original full-inventory Advance awaits native execution; it is never resent.
    Advance,
    /// Exact pin signature is retained; its inventory must still be anchored before exposure.
    RetainedPin,
    /// Original exposed pin awaits exact successful native finality/current record.
    Pin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveOrderProgress {
    NeedsSubmission,
    Submitted,
    Finalized,
}

/// Local progress or a genuine exact successful pin readback. Queue acceptance is not finality.
#[expect(
    variant_size_differences,
    reason = "finality carries one boxed receipt pointer while pending polls need only their phase"
)]
pub enum NativeMusubiPinProgressV1 {
    /// No successful pin current-readback is available yet.
    Pending(NativeMusubiPinPhaseV1),
    /// The sole native source reader authenticated the original input and current pin record.
    Finalized(Box<NativeMusubiFinalizedPinV1>),
}

/// Exact original envelope and native pin record read at one current local cut.
/// This private-constructor result proves neither replication nor publication readiness.
pub struct NativeMusubiFinalizedPinV1 {
    operation: [u8; 32],
    query: MusubiPublicationFinalizedPinRegistrationQueryV1,
    record: PinManifestRecord,
}
impl NativeMusubiFinalizedPinV1 {
    /// Original immutable local operation identity.
    pub const fn operation_id(&self) -> [u8; 32] {
        self.operation
    }
    /// Original source, signed wire, digest and authenticated successful height.
    pub fn query(&self) -> &MusubiPublicationFinalizedPinRegistrationQueryV1 {
        &self.query
    }
    /// Exact current nonretired native pin record at the read's cut.
    pub fn record(&self) -> &PinManifestRecord {
        &self.record
    }
}

/// Complete decoded local selection; construction and decoding confer no native evidence.
pub struct NativeMusubiPinOriginalV1 {
    /// Digest retained before any quote or signature.
    pub context_digest: [u8; 32],
    /// Original independently selected archive registration query.
    pub source: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    /// Original immutable spending and exclusive UTC ceiling.
    pub authorization: NativePinAuthorizationV1,
}

/// Original retained native Check refusal, borrowed without erasing its cause or retry custody.
pub enum NativeMusubiPinCheckRefusalV1<'a> {
    /// Original binding/allocation failure still owns the exact signed graph.
    Binding(&'a MusubiPinOutboxCheckBindingFailureV1),
    /// Original native finality/current-row attempt still owns the paid Check.
    Read(&'a MusubiPinOutboxCheckAttemptFailureV1),
}

#[derive(Debug)]
enum RoundStart {
    Started,
    AwaitingControl(NativeMusubiPinPhaseV1),
}

#[derive(Clone, Copy)]
enum Goal {
    Initialize,
    SignPin,
    Advance,
    ExposePin,
}
struct Round {
    operation: Operation,
    local_digest: [u8; 32],
    goal: Goal,
    deadline: Instant,
    prepared: Option<PreparedMusubiPinOutboxCheckV1>,
    binding_failure: Option<MusubiPinOutboxCheckBindingFailureV1>,
    pending: Option<PendingMusubiPinOutboxCheckV1>,
    read_failure: Option<MusubiPinOutboxCheckAttemptFailureV1>,
    slot: Option<Slot>,
}

/// One retained native network/pin-authority session and its runtime-only original credential.
/// Ordinary open never creates missing history. One instance serializes every operation and
/// retains successful signatures even when subsequent persistence or observation refuses.
pub struct NativeMusubiPinCoordinatorV1 {
    store: Store,
    signer: MusubiPublicationPinTransactionSignerV1,
    reader: MusubiPublicationFinalizedPinRegistrationReaderV1,
    state: Arc<State>,
    queue: Arc<Queue>,
    round: Option<Round>,
    // A signature created by a successful Check but not durably written yet. A later call first
    // reconciles these exact bytes; it still needs a new fresh Check before any new effect.
    retained: Option<Slot>,
}
impl NativeMusubiPinCoordinatorV1 {
    /// Atomically initialize one fresh, explicitly selected nonzero session before any effects.
    /// # Errors
    /// Refuses existing/unsafe custody, foreign network/key or invalid original paid-pin policy.
    pub fn initialize(
        context: &MusubiPublicationPrivateServiceContextV1,
        path: &Path,
        session: [u8; 32],
        policy: MusubiPublicationPaidPinPolicy,
        key: KeyPair,
    ) -> Result<Self> {
        Self::construct(context, path, session, policy, key, true)
    }
    /// Reopen exactly the original initialized session without repairing or replacing history.
    /// # Errors
    /// Refuses missing/changed complete inventory, original policy/identity or runtime key.
    pub fn open(
        context: &MusubiPublicationPrivateServiceContextV1,
        path: &Path,
        session: [u8; 32],
        policy: MusubiPublicationPaidPinPolicy,
        key: KeyPair,
    ) -> Result<Self> {
        Self::construct(context, path, session, policy, key, false)
    }
    fn construct(
        context: &MusubiPublicationPrivateServiceContextV1,
        path: &Path,
        session: [u8; 32],
        policy: MusubiPublicationPaidPinPolicy,
        key: KeyPair,
        fresh: bool,
    ) -> Result<Self> {
        ensure!(
            context.state().network_id_ref() == &context.network_id(),
            "native pin State network differs"
        );
        ensure!(
            key.public_key().try_algorithm()? == iroha_crypto::Algorithm::Ed25519,
            "native pin requires the original direct Ed25519 profile"
        );
        let original = NativeMusubiPinSessionV1 {
            network: context.network_id(),
            authority: policy.transaction_authority.clone(),
            session,
            storage_class: match policy.storage_class {
                StorageClass::Hot => 0,
                StorageClass::Warm => 1,
                StorageClass::Cold => 2,
            },
            retention_horizon_secs: policy.retention_horizon_secs,
        };
        let reader =
            context.finalized_pin_registration_reader(policy.transaction_authority.clone());
        let signer = MusubiPublicationPinTransactionSignerV1::new(context, policy, key)?;
        let store = if fresh {
            Store::initialize(path, original)?
        } else {
            Store::open(path, &original)?
        };
        Ok(Self {
            store,
            signer,
            reader,
            state: context.state(),
            queue: context.queue(),
            round: None,
            retained: None,
        })
    }
    /// Retain a fresh explicit operation and its complete original source/spending selection.
    /// This performs no quote, signature, native Check or Queue dispatch.
    /// # Errors
    /// Refuses any changed existing original, unsafe/full inventory or noncanonical selection.
    pub fn prepare_operation(
        &mut self,
        id: [u8; 32],
        context_digest: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        authorization: NativePinAuthorizationV1,
    ) -> Result<()> {
        self.prepare_selected_operation(id, context_digest, source, authorization, false)
    }
    pub(super) fn prepare_selected_operation(
        &mut self,
        id: [u8; 32],
        context_digest: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        authorization: NativePinAuthorizationV1,
        includes_archive_order: bool,
    ) -> Result<()> {
        self.reconcile_retained()?;
        ensure!(self.round.is_none(), "another native Check round is active");
        let selected_source = slot::encode_frame(source)?;
        if let Some(original) = self.store.find_operation(id)? {
            ensure!(
                original.context_digest == context_digest
                    && original.source == selected_source
                    && original.authorization == authorization
                    && original.includes_archive_order == includes_archive_order,
                "native pin operation changed its original source or authorization"
            );
            return Ok(());
        }
        let inventory = self.store.inventory(None)?;
        let operation = Operation {
            id,
            context_digest,
            ordinal: inventory
                .operations
                .checked_add(1)
                .ok_or_else(|| eyre::eyre!("native operation ordinal overflow"))?,
            source: selected_source,
            authorization,
            includes_archive_order,
        };
        self.store.create_operation(&operation)
    }
    /// Recover the complete local operation selection after auditing the entire session.
    /// Returned claims grant no native finality, current-state or signing authority.
    /// # Errors
    /// Refuses incomplete or substituted custody and original resource/codec failures.
    pub fn original_operation(&self, id: [u8; 32]) -> Result<Option<NativeMusubiPinOriginalV1>> {
        self.inspected_operation(id)?
            .0
            .map(|operation| {
                let source = operation.source()?;
                Ok(NativeMusubiPinOriginalV1 {
                    context_digest: operation.context_digest,
                    source,
                    authorization: operation.authorization,
                })
            })
            .transpose()
    }

    // Audit all retained operations and original slot bytes once while this exact coordinator
    // and any active signing slot are borrowed. No result crosses a later effect or call.
    fn inspected_operation(&self, id: [u8; 32]) -> Result<(Option<Operation>, Option<&Slot>)> {
        ensure!(id != [0; 32], "native pin operation ID is zero");
        if let Some(round) = &self.round {
            ensure!(
                round.operation.id == id,
                "another native operation owns the active round"
            );
        }
        ensure!(
            self.round
                .as_ref()
                .and_then(|round| round.slot.as_ref())
                .is_none()
                || self.retained.is_none(),
            "two native slots unexpectedly hold original custody"
        );
        let held = self
            .round
            .as_ref()
            .and_then(|round| round.slot.as_ref())
            .or(self.retained.as_ref());
        if let Some(slot) = held {
            ensure!(
                slot.request().operation == id,
                "another native operation owns retained custody"
            );
        }
        Ok((self.store.find_operation_with_held(id, held)?, held))
    }

    /// Read existing exact pin finality without signing, dispatching or repairing missing files.
    /// # Errors
    /// Refuses absent/incomplete originals and preserves the native reader's current refusal.
    pub fn observe_operation(&self, id: [u8; 32]) -> Result<NativeMusubiPinProgressV1> {
        let (operation, held) = self.inspected_operation(id)?;
        let operation = operation.ok_or_else(|| eyre::eyre!("native operation is missing"))?;
        if held.is_some_and(|slot| slot.request().kind == SlotKind::Pin) {
            // An owned, unexposed pin is pending custody; inspecting it never releases or repairs it.
            return Ok(NativeMusubiPinProgressV1::Pending(
                NativeMusubiPinPhaseV1::RetainedPin,
            ));
        }
        self.recover_operation(&operation)
    }

    /// Observe whether one complete original operation is retained. This performs no signing,
    /// creation or Queue dispatch and grants no native evidence. A caller may prepare only true
    /// absence; malformed/incomplete inventory is an error, never permission to renew terms.
    /// # Errors
    /// Refuses changed or incomplete custody and original decode/resource failures.
    pub fn contains_operation(&self, id: [u8; 32]) -> Result<bool> {
        ensure!(id != [0; 32], "native pin operation ID is zero");
        // A live owner already retained this operation before locking its selected slot. A
        // positive local observation grants no new effect; do not reopen our own exclusive lock.
        if let Some(round) = &self.round {
            ensure!(
                round.operation.id == id,
                "another native operation owns the active round"
            );
            return Ok(true);
        }
        if let Some(slot) = &self.retained {
            ensure!(
                slot.request().operation == id,
                "another native operation owns retained custody"
            );
            return Ok(true);
        }
        Ok(self.store.find_operation(id)?.is_some())
    }

    /// Retire future effects under the original operation. Already exposed transactions remain
    /// eligible for their normal native execution and are reconciled only with `recover`.
    /// # Errors
    /// Refuses changed custody or a still-unpersisted successful signature; no original is erased.
    pub fn cancel_operation(&mut self, id: [u8; 32]) -> Result<()> {
        self.reconcile_retained()?;
        if let Some(round) = &self.round {
            ensure!(
                round.operation.id == id,
                "another native operation owns the active round"
            );
            if let Some(slot) = &round.slot
                && slot.signed().is_some()
            {
                slot.persist_signed()?;
            }
        }
        drop(self.round.take());
        let operation = self.store.operation(id)?;
        self.store.retire_operation(&operation)
    }

    /// Borrow a pending original native refusal; no diagnostic constructs another signed attempt.
    pub fn check_refusal(&self) -> Option<NativeMusubiPinCheckRefusalV1<'_>> {
        let round = self.round.as_ref()?;
        if let Some(error) = &round.binding_failure {
            return Some(NativeMusubiPinCheckRefusalV1::Binding(error));
        }
        round
            .read_failure
            .as_ref()
            .map(NativeMusubiPinCheckRefusalV1::Read)
    }
    // A missing commit is observable without turning a bound, rejected or expired Check
    // into a pending receipt. The native verifier remains the sole source of success.
    pub(super) fn awaiting_check_receipt(&self) -> Result<bool> {
        let Some(round) = &self.round else {
            return Ok(false);
        };
        if Instant::now() >= round.deadline || round.binding_failure.is_some() {
            return Ok(false);
        }
        if let Some(refusal) = &round.read_failure {
            if refusal.rejection() != Some(MusubiPinOutboxCheckErrorV1::NotApplied) {
                return Ok(false);
            }
            let hash = refusal.signed_transaction().try_hash_as_entrypoint()?;
            return Ok(self.state.committed_entrypoint_height(&hash).is_none());
        }
        Ok(round.slot.is_none() && round.pending.is_some())
    }
    /// Read only the exact original pin's successful native result/current record, including after
    /// original authorization expiry. Missing or unprepared records never trigger creation/I/O to Queue.
    /// # Errors
    /// Preserves native reader refusals; rejects changed original custody or pin shape.
    pub fn recover(&mut self, id: [u8; 32]) -> Result<NativeMusubiPinProgressV1> {
        self.reconcile_retained()?;
        ensure!(
            self.round.is_none(),
            "finish or retire the current Check before detached recovery"
        );
        let operation = self.store.operation(id)?;
        self.recover_operation(&operation)
    }
    /// Perform at most one original Queue exposure, or one signature-retention step, then return.
    /// Every effect follows a newly completed fresh native Check within the same original round.
    /// # Errors
    /// Retains signed custody on refusal; never renews UTC/fees/nonce/TTL or resends exposed bytes.
    pub fn advance(
        &mut self,
        id: [u8; 32],
        clock: &mut dyn MusubiPublicationServiceClockV1,
        deadline: Instant,
    ) -> Result<NativeMusubiPinProgressV1> {
        self.reconcile_retained()?;
        if let Some(round) = &self.round {
            ensure!(
                round.operation.id == id,
                "another native operation owns the active round"
            );
        }
        if self.round.is_none() {
            let operation = self.store.operation(id)?;
            self.store.require_active_operation(&operation)?;
            self.signer
                .recheck_finalized_archive(&operation.source()?)?;
            if let Some(slot) = self.store.open_slot(&operation, SlotKind::Pin)?
                && slot.exposed()?
            {
                drop(slot);
                return self.recover_operation(&operation);
            }
            let now = clock.current_time_ms()?;
            operation.authorization.ensure_live(now)?;
            ensure!(
                deadline > Instant::now(),
                "native pin caller deadline expired"
            );
            if let RoundStart::AwaitingControl(phase) =
                self.begin_round(operation, now, deadline)?
            {
                return Ok(NativeMusubiPinProgressV1::Pending(phase));
            }
        }
        let mut round = self
            .round
            .take()
            .ok_or_else(|| eyre::eyre!("native round is missing"))?;
        let result = self.drive_round(&mut round, clock, deadline);
        match result {
            Err(error) => {
                self.round = Some(round);
                Err(error)
            }
            Ok(None) => {
                if Instant::now() < round.deadline {
                    self.round = Some(round);
                }
                Ok(NativeMusubiPinProgressV1::Pending(
                    NativeMusubiPinPhaseV1::Check,
                ))
            }
            Ok(Some(readback)) => {
                // Release the round slot lock before auditing the complete inventory.
                drop(round.slot.take());
                ensure!(
                    self.store.inventory(None)?.digest == round.local_digest,
                    "native inventory changed during Check"
                );
                self.perform_goal(
                    &round.operation,
                    round.goal,
                    &readback,
                    clock,
                    deadline.min(round.deadline),
                )
            }
        }
    }
    /// Drive one immutable, governed purpose order after the original paid pin finalized.
    /// A recorded submission is reconciled read-only, including an unsuccessful native output.
    pub(super) fn issue_archive_order(
        &mut self,
        pin: &NativeMusubiFinalizedPinV1,
        seed: iroha_data_model::sorafs::capacity::ProviderId,
        clock: &mut dyn MusubiPublicationServiceClockV1,
        deadline: Instant,
    ) -> Result<()> {
        self.reconcile_retained()?;
        ensure!(
            self.round.is_none(),
            "a native Check round still owns the publication"
        );
        let operation = self.store.operation(pin.operation_id())?;
        self.store.require_active_operation(&operation)?;
        let now = operation
            .authorization
            .check_effect_boundary(clock, deadline)?;
        let query = pin.query();
        let plan =
            iroha_core::query::native_musubi_storage::plan_native_musubi_replication_order_v1(
                &self.state.view(),
                &self.store.original().authority,
                (
                    query.source.registration.archive_id,
                    query.manifest_digest,
                    query.finalized_height,
                ),
                seed,
                now / 1_000,
            )?;
        let plan = plan
            .ok_or_else(|| eyre::eyre!("issued archive order has no matching exposed original"))?;
        let request = self.request(&operation, SlotKind::ArchiveOrder, &plan.into())?;
        let slot = if let Some(slot) = self.store.open_slot(&operation, SlotKind::ArchiveOrder)? {
            ensure!(
                slot.request() == &request,
                "native archive order differs from its original plan"
            );
            if slot.exposed()? {
                // Reconciliation can recover a marker whose original write returned an error.
                return Ok(());
            }
            slot
        } else {
            self.store.create_slot(&operation, request)?
        };
        self.retained = Some(slot);
        let slot = self.retained.as_mut().expect("just retained");
        if slot.payload()?.is_none() {
            let payload = self.signer.prepare_control(slot.request(), now)?;
            self.store.admit_payload(&operation, slot, &payload)?;
            slot.retain_payload(&payload)?;
        }
        self.signer
            .sign_archive_order(slot, pin, seed, clock, deadline)?;
        operation
            .authorization
            .check_effect_boundary(clock, deadline)?;
        let dispatch = slot.record_exposure()?;
        let slot = self
            .retained
            .take()
            .expect("original archive order is retained");
        if dispatch {
            self.dispatch(slot, &operation.authorization, clock, deadline)?;
        }
        Ok(())
    }

    /// Require the local original as well as native success before any source staging.
    pub(super) fn archive_order_progress(
        &self,
        pin: &NativeMusubiFinalizedPinV1,
    ) -> Result<ArchiveOrderProgress> {
        if self.retained.as_ref().is_some_and(|slot| {
            slot.request().kind == SlotKind::ArchiveOrder
                && slot.request().operation == pin.operation_id()
        }) {
            return Ok(ArchiveOrderProgress::NeedsSubmission);
        }
        let operation = self.store.operation(pin.operation_id())?;
        let Some(slot) = self.store.open_slot(&operation, SlotKind::ArchiveOrder)? else {
            return Ok(ArchiveOrderProgress::NeedsSubmission);
        };
        if !slot.exposed()? {
            return Ok(ArchiveOrderProgress::NeedsSubmission);
        }
        let signed = slot
            .signed()
            .ok_or_else(|| eyre::eyre!("original archive order signature missing"))?;
        let entry = signed.try_hash_as_entrypoint()?;
        let Some(height) = self.state.committed_entrypoint_height(&entry) else {
            return Ok(ArchiveOrderProgress::Submitted);
        };
        let height = u64::try_from(height.get())?;
        ensure!(
            height >= pin.query().finalized_height,
            "archive order predates the paid pin"
        );
        let query = pin.query();
        let expected =
            iroha_core::query::native_musubi_storage::native_musubi_replication_order_id_v1(
                query.source.registration.archive_id,
                query.manifest_digest,
            );
        let Executable::Instructions(instructions) = signed.instructions() else {
            eyre::bail!("original archive order has no instruction");
        };
        ensure!(
            matches!(instructions.as_ref(), [instruction] if instruction.as_any()
            .downcast_ref::<iroha_data_model::isi::sorafs::IssueReplicationOrder>()
            .is_some_and(|order| order.order_id == expected && order.musubi_archive == Some(query.source.registration.archive_id))),
            "original archive order belongs to another pin"
        );
        super::pin_registration::require_finalized_transaction(
            &self.state.query_view(),
            self.state.network_id_ref(),
            height,
            signed,
        )?;
        Ok(ArchiveOrderProgress::Finalized)
    }

    fn reconcile_retained(&mut self) -> Result<()> {
        if let Some(slot) = &self.retained
            && slot.signed().is_some()
        {
            slot.persist_signed()?;
        }
        drop(self.retained.take());
        Ok(())
    }
    fn begin_round(
        &mut self,
        operation: Operation,
        now: u64,
        deadline: Instant,
    ) -> Result<RoundStart> {
        require_coherent_native_tip(&self.state)?;
        let inventory = self.store.inventory(None)?;
        let pin = self.store.open_slot(&operation, SlotKind::Pin)?;
        let has_signed_pin = pin.as_ref().is_some_and(|slot| slot.signed().is_some());
        drop(pin);
        let view = self.state.view();
        let row = view
            .world
            .musubi_pin_outbox_high_waters()
            .get(&self.store.original().authority);
        let (goal, expected, digest) = match row {
            None => {
                ensure!(
                    inventory.signed_pins == 0,
                    "native absence cannot replace retained signed inventory"
                );
                (
                    Goal::Initialize,
                    MusubiPinOutboxCheckExpectationV1::Absent,
                    inventory.digest,
                )
            }
            Some(row) => {
                ensure!(
                    row.network_id == self.store.original().network
                        && row.session_id == self.store.original().session,
                    "native outbox belongs to another original session"
                );
                let goal = if row.inventory_digest == inventory.digest {
                    if has_signed_pin {
                        Goal::ExposePin
                    } else {
                        Goal::SignPin
                    }
                } else {
                    ensure!(
                        has_signed_pin
                            && self.store.inventory(Some(operation.id))?.digest
                                == row.inventory_digest,
                        "native high-water differs from the complete retained predecessor"
                    );
                    Goal::Advance
                };
                (
                    goal,
                    MusubiPinOutboxCheckExpectationV1::Present(row.clone()),
                    row.inventory_digest,
                )
            }
        };
        let control = match goal {
            Goal::Initialize => Some(SlotKind::Initialize),
            Goal::Advance => Some(SlotKind::Advance),
            _ => None,
        };
        if let Some(kind) = control
            && let Some(slot) = self.store.open_slot(&operation, kind)?
            && slot.exposed()?
        {
            // Never re-expose this transaction. Once indexed, its expected current
            // high-water has to agree with the independently selected goal above.
            let hash = slot.into_exposed_transaction()?.try_hash_as_entrypoint()?;
            ensure!(
                view.transactions().get(&hash).is_none(),
                "original native Advance remains exposed; await its exact native effect"
            );
            return Ok(RoundStart::AwaitingControl(
                if kind == SlotKind::Initialize {
                    NativeMusubiPinPhaseV1::Initialize
                } else {
                    NativeMusubiPinPhaseV1::Advance
                },
            ));
        }
        let height = u64::try_from(view.block_hashes().len())?;
        ensure!(
            height >= 2,
            "native pin Check needs a result-bearing Global floor"
        );
        let block = iroha_core::sumeragi::certified_chain::committed_block(&view, height)?;
        let floor = MusubiPinOutboxCheckFloorV1 {
            height,
            block_hash: *block.block_hash().as_ref(),
            context_id: block.id(),
        };
        let expected = MusubiPinOutboxCheckExpectedV1 {
            chain_id: view.chain_id().clone(),
            network_id: self.store.original().network,
            pin_authority: self.store.original().authority.clone(),
            session_id: self.store.original().session,
            inventory_digest: digest,
            floor,
            expected,
        };
        drop(view);
        let deadline = operation.authorization.round_deadline(now, deadline)?;
        let prepared =
            begin_musubi_pin_outbox_check_v1(Arc::clone(&self.state), expected, deadline)?;
        let round_number = self.store.next_check_round(&operation)?;
        let instruction: InstructionBox = prepared.instruction().clone().into();
        let request = self.request(&operation, SlotKind::Check(round_number), &instruction)?;
        let slot = self.store.create_slot(&operation, request)?;
        self.round = Some(Round {
            operation,
            local_digest: inventory.digest,
            goal,
            deadline,
            prepared: Some(prepared),
            binding_failure: None,
            pending: None,
            read_failure: None,
            slot: Some(slot),
        });
        Ok(RoundStart::Started)
    }
    fn request(
        &self,
        operation: &Operation,
        kind: SlotKind,
        instruction: &InstructionBox,
    ) -> Result<SlotRequest> {
        Ok(SlotRequest {
            network: self.store.original().network,
            authority: self.store.original().authority.clone(),
            session: self.store.original().session,
            operation: operation.id,
            kind,
            pin_selected_at_unix_ms: None,
            instruction: slot::encode_frame(instruction)?,
            authorization: operation.authorization.clone(),
        })
    }
    fn drive_round(
        &mut self,
        round: &mut Round,
        clock: &mut dyn MusubiPublicationServiceClockV1,
        caller: Instant,
    ) -> Result<Option<MusubiPinOutboxCurrentReadbackV1>> {
        let now = clock.current_time_ms()?;
        let deadline = round.deadline.min(caller);
        if Instant::now() >= round.deadline || now >= round.operation.authorization.deadline_unix_ms
        {
            // Persist a successful signature before relinquishing its live owner. No old round,
            // retained bytes or elapsed deadline can be converted into another prepared stage.
            if let Some(slot) = &round.slot
                && slot.signed().is_some()
            {
                slot.persist_signed()?;
            }
            return Ok(None);
        }
        ensure!(caller > Instant::now(), "native caller deadline expired");
        if let Some(slot) = &mut round.slot {
            if slot.payload()?.is_none() {
                let payload = self.signer.prepare_control(slot.request(), now)?;
                self.store.admit_payload(&round.operation, slot, &payload)?;
                slot.retain_payload(&payload)?;
            }
            self.signer.sign_retained(slot, None, clock, deadline)?;
        }
        if let Some(failure) = round.binding_failure.take() {
            match failure.retry() {
                Ok(pending) => round.pending = Some(pending),
                Err(failure) => {
                    round.binding_failure = Some(failure);
                    return Ok(None);
                }
            }
        }
        if round.prepared.is_some() {
            let signed = round
                .slot
                .as_ref()
                .and_then(Slot::signed)
                .ok_or_else(|| eyre::eyre!("native Check signature missing"))?;
            let allocated =
                AllocatedPinTransactionV1::copy_from(signed, &self.state.ivm_execution_budget())?;
            let prepared = round.prepared.take().unwrap();
            match prepared.bind_allocated_transaction(allocated) {
                Ok(pending) => round.pending = Some(pending),
                Err(failure) => {
                    round.binding_failure = Some(failure);
                    return Ok(None);
                }
            }
        }
        if let Some(slot) = &round.slot {
            round
                .operation
                .authorization
                .check_effect_boundary(clock, deadline)?;
            let dispatch = slot.record_exposure()?;
            let slot = round.slot.take().unwrap();
            if dispatch {
                self.dispatch(slot, &round.operation.authorization, clock, deadline)?;
            }
            // An existing/uncertain marker never permits another Queue attempt.
            return Ok(None);
        }
        // Keep the original pending/read refusal in the round if local publication is
        // between State and Kura cuts. This observation itself grants no finality.
        require_coherent_native_tip(&self.state)?;
        if let Some(failure) = round.read_failure.take() {
            round.pending = Some(failure.into_pending());
        }
        let pending = round
            .pending
            .take()
            .ok_or_else(|| eyre::eyre!("native pending Check missing"))?;
        let verified = match pending.verify_finalized() {
            Ok(value) => value,
            Err(failure) => {
                round.read_failure = Some(failure);
                return Ok(None);
            }
        };
        match verified.consume_current(&self.state) {
            Ok(value) => Ok(Some(value)),
            Err(failure) => {
                round.read_failure = Some(failure);
                Ok(None)
            }
        }
    }
    fn dispatch(
        &self,
        slot: Slot,
        authorization: &NativePinAuthorizationV1,
        clock: &mut dyn MusubiPublicationServiceClockV1,
        deadline: Instant,
    ) -> Result<()> {
        ensure!(
            &slot.request().authorization == authorization,
            "native dispatch authorization differs"
        );
        // The durable marker is already permanent. Expiry after fsync/slot validation must
        // retain it and must not expose the envelope to ordinary admission or Queue.
        let signed = slot.into_exposed_transaction()?;
        authorization.check_effect_boundary(clock, deadline)?;
        let parameters = self.state.transaction_admission_parameters();
        let admission_policy = parameters.require_crypto_policy().map_err(|_| {
            iroha_core::execution_attempt::ExecutionDeferred::from(
                ivm::error::ExecutionDeferral::CryptoAdmissionPolicyUnavailable,
            )
        })?;
        let accepted = AcceptedTransaction::accept(
            signed,
            self.state.network_id_ref(),
            parameters.sumeragi().max_clock_drift(),
            parameters.transaction(),
            admission_policy,
        )?;
        drop(parameters);
        // Ordinary admission can perform expensive verification. Recheck the same originals
        // after that work, immediately before the only Queue call; never replace the marker.
        authorization.check_effect_boundary(clock, deadline)?;
        // No Check publication lock is held here. This is exactly ordinary admission and Queue;
        // Queue's existing retained-cost estimate is not relabelled as physical graph custody.
        self.queue
            .push_with_lane_with_state(accepted, &self.state)
            .map_err(|failure| eyre::Report::new(failure.err))?;
        Ok(())
    }
    fn perform_goal(
        &mut self,
        operation: &Operation,
        goal: Goal,
        readback: &MusubiPinOutboxCurrentReadbackV1,
        clock: &mut dyn MusubiPublicationServiceClockV1,
        deadline: Instant,
    ) -> Result<NativeMusubiPinProgressV1> {
        let now = clock.current_time_ms()?;
        operation.authorization.ensure_live(now)?;
        ensure!(
            Instant::now() < deadline,
            "original native Check action deadline expired"
        );
        let full = self.store.inventory(None)?;
        let source = operation.source()?;
        match goal {
            Goal::SignPin => {
                let existing = self.store.open_slot(operation, SlotKind::Pin)?;
                let (slot, payload) = if let Some(slot) = existing {
                    let payload = if slot.payload()?.is_none() {
                        Some(
                            self.signer
                                .prepare_retained_pin(slot.request(), &source, now)?,
                        )
                    } else {
                        None
                    };
                    (slot, payload)
                } else {
                    let (_, payload) = self.signer.prepare_finalized_archive(
                        &source,
                        clock,
                        operation.authorization.deadline_unix_ms,
                    )?;
                    let Executable::Instructions(values) = &payload.instructions else {
                        eyre::bail!("native pin draft differs");
                    };
                    let [instruction] = values.as_ref() else {
                        eyre::bail!("native pin draft differs");
                    };
                    let mut request = self.request(operation, SlotKind::Pin, instruction)?;
                    request.pin_selected_at_unix_ms = Some(payload.creation_time_ms);
                    (self.store.create_slot(operation, request)?, Some(payload))
                };
                self.retained = Some(slot);
                let slot = self.retained.as_mut().unwrap();
                if let Some(payload) = payload {
                    self.store.admit_payload(operation, slot, &payload)?;
                    slot.retain_payload(&payload)?;
                }
                ensure!(
                    Instant::now() < deadline,
                    "native Check expired before pin signature"
                );
                self.signer
                    .sign_retained(slot, Some(&source), clock, deadline)?;
                drop(self.retained.take());
                Ok(NativeMusubiPinProgressV1::Pending(
                    NativeMusubiPinPhaseV1::RetainedPin,
                ))
            }
            Goal::Initialize | Goal::Advance => {
                let (kind, revision, previous) = match readback.high_water() {
                    None => {
                        ensure!(
                            matches!(goal, Goal::Initialize) && full.signed_pins == 0,
                            "native initialization differs"
                        );
                        (SlotKind::Initialize, 0, [0; 32])
                    }
                    Some(row) => {
                        ensure!(matches!(goal, Goal::Advance), "native advance differs");
                        (SlotKind::Advance, row.revision, row.inventory_digest)
                    }
                };
                let instruction: InstructionBox = AdvanceMusubiPinOutboxV1 {
                    network_id: self.store.original().network,
                    pin_authority: self.store.original().authority.clone(),
                    session_id: self.store.original().session,
                    expected_revision: revision,
                    expected_inventory_digest: previous,
                    inventory_digest: full.digest,
                }
                .into();
                let request = self.request(operation, kind, &instruction)?;
                let slot = if let Some(slot) = self.store.open_slot(operation, kind)? {
                    ensure!(
                        slot.request() == &request,
                        "original Advance selection changed"
                    );
                    slot
                } else {
                    self.store.create_slot(operation, request)?
                };
                self.retained = Some(slot);
                let slot = self.retained.as_mut().unwrap();
                if slot.payload()?.is_none() {
                    let payload = self.signer.prepare_control(slot.request(), now)?;
                    self.store.admit_payload(operation, slot, &payload)?;
                    slot.retain_payload(&payload)?;
                }
                self.signer.sign_retained(slot, None, clock, deadline)?;
                operation
                    .authorization
                    .check_effect_boundary(clock, deadline)?;
                let dispatch = slot.record_exposure()?;
                let slot = self.retained.take().unwrap();
                if dispatch {
                    self.dispatch(slot, &operation.authorization, clock, deadline)?;
                }
                Ok(NativeMusubiPinProgressV1::Pending(
                    if kind == SlotKind::Initialize {
                        NativeMusubiPinPhaseV1::Initialize
                    } else {
                        NativeMusubiPinPhaseV1::Advance
                    },
                ))
            }
            Goal::ExposePin => {
                ensure!(
                    readback
                        .high_water()
                        .is_some_and(|row| row.inventory_digest == full.digest),
                    "native pin inventory is not anchored"
                );
                let slot = self
                    .store
                    .open_slot(operation, SlotKind::Pin)?
                    .ok_or_else(|| eyre::eyre!("original pin is missing"))?;
                ensure!(slot.signed().is_some(), "native pin signature missing");
                self.retained = Some(slot);
                self.signer.sign_retained(
                    self.retained.as_mut().unwrap(),
                    Some(&source),
                    clock,
                    deadline,
                )?;
                operation
                    .authorization
                    .check_effect_boundary(clock, deadline)?;
                let dispatch = self.retained.as_ref().unwrap().record_exposure()?;
                let slot = self.retained.take().unwrap();
                if dispatch {
                    self.dispatch(slot, &operation.authorization, clock, deadline)?;
                }
                Ok(NativeMusubiPinProgressV1::Pending(
                    NativeMusubiPinPhaseV1::Pin,
                ))
            }
        }
    }
    fn recover_operation(&self, operation: &Operation) -> Result<NativeMusubiPinProgressV1> {
        let Some(slot) = self.store.open_slot(operation, SlotKind::Pin)? else {
            return Ok(NativeMusubiPinProgressV1::Pending(
                NativeMusubiPinPhaseV1::RetainedPin,
            ));
        };
        if !slot.exposed()? {
            return Ok(NativeMusubiPinProgressV1::Pending(
                NativeMusubiPinPhaseV1::RetainedPin,
            ));
        }
        let signed = slot
            .signed()
            .ok_or_else(|| eyre::eyre!("exposed pin signature missing"))?;
        let hash = signed.try_hash_as_entrypoint()?;
        let Some(height) = self.state.committed_entrypoint_height(&hash) else {
            return Ok(NativeMusubiPinProgressV1::Pending(
                NativeMusubiPinPhaseV1::Pin,
            ));
        };
        let Executable::Instructions(instructions) = signed.instructions() else {
            eyre::bail!("original pin instruction differs");
        };
        let pin = instructions[0]
            .as_any()
            .downcast_ref::<RegisterPinManifest>()
            .ok_or_else(|| eyre::eyre!("original pin instruction differs"))?;
        let manifest = sorafs_manifest::decode_manifest_v1_canonical(&pin.manifest_payload)?;
        let manifest_digest = ManifestDigest::from_manifest(&manifest)?;
        let query = MusubiPublicationFinalizedPinRegistrationQueryV1 {
            version: 1,
            source: operation.source()?,
            transaction: slot.into_exposed_transaction()?,
            manifest_digest,
            finalized_height: u64::try_from(height.get())?,
        };
        let record = self.reader.read_current_pin(&query)?;
        Ok(NativeMusubiPinProgressV1::Finalized(Box::new(
            NativeMusubiFinalizedPinV1 {
                operation: operation.id,
                query,
                record,
            },
        )))
    }
}

/// Refuse an in-progress State/Kura publication before obtaining/consuming fresh native evidence.
/// This is only a local coherence fence; Core still owns exact native certification and currentness.
pub(super) fn require_coherent_native_tip(state: &State) -> Result<()> {
    let view = state.query_view();
    ensure!(
        view.kura().exact_durable_blocks_count()? == view.block_hashes().len(),
        "native pin State/Kura height differs"
    );
    Ok(())
}
