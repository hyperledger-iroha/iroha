//! Fresh native Musubi inventory readback over one original State and signed Check.
//!
//! These move-only stages authenticate authority-wide absence or one entire high-water row.
//! Their result is a read-only observation, never permission to initialize a replacement
//! outbox, sign a pin, enter Queue, or publish. Those effects require their own closed owner.

use std::{sync::Arc, time::Instant};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::consensus::SumeragiRootScope,
    isi::musubi::{CheckMusubiPinOutboxV1, MUSUBI_PIN_OUTBOX_CHECK_MAX_BYTES_V1},
    musubi::{
        MusubiPinOutboxCheckExpectationV1, MusubiPinOutboxCheckFloorV1, MusubiPinOutboxHighWaterV1,
    },
    transaction::SignedTransaction,
};
use iroha_model_base::chain::ChainId;
use mv::storage::StorageReadOnly as _;

use crate::{
    query::signer_check::{
        BorrowedCheckExecutionCutV1, BoundNativeCheckV1, NativeCheckErrorV1, NativeCheckFloorV1,
        NativeCheckRoundV1, NativeCustodyCheckPurposeV1, NativeCustodyCheckRefV1,
        PreparedCheckExecutionV1, SignerCertifiedWalkV1, bind_signed_check_v1,
        validate_native_signatory_v1, with_native_check_read_limits,
    },
    state::{
        State, StateReadOnly as _, StateView, WorldReadOnly as _, is_stable_state_view_generation,
    },
    sumeragi::crypto::BlsCrypto,
};

/// Independently selected readback coordinates. Decoding or constructing these grants no proof.
pub struct MusubiPinOutboxCheckExpectedV1 {
    /// Exact configured chain label, independently selected before any history response.
    pub chain_id: ChainId,
    /// Genesis-derived network selected by the deployment owner.
    pub network_id: NetworkId,
    /// Sole signed authority whose entire current row is queried.
    pub pin_authority: AccountId,
    /// Original locally retained signing session, including for absence.
    pub session_id: [u8; 32],
    /// Complete locally retained inventory, including for absence.
    pub inventory_digest: [u8; 32],
    /// Independently retained native floor; the supplied State must authenticate it.
    pub floor: MusubiPinOutboxCheckFloorV1,
    /// Authority-wide absence or exact equality of every high-water field.
    pub expected: MusubiPinOutboxCheckExpectationV1,
}

/// Closed readback failure; none represents a submitted transaction or pin completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MusubiPinOutboxCheckErrorV1 {
    /// Invalid independently selected scope, deadline, field or capacity.
    #[error("invalid Musubi pin-outbox Check request")]
    Invalid,
    /// The original monotonic lifetime expired without renewal.
    #[error("Musubi pin-outbox Check deadline expired")]
    Expired,
    /// A fresh nonzero challenge could not be obtained.
    #[error("Musubi pin-outbox Check challenge unavailable")]
    Entropy,
    /// The supplied signed transaction differs from the exact prepared native Check.
    #[error("Musubi pin-outbox signed Check differs")]
    Transaction,
    /// The original signed Check has no successful local application proof after its floor.
    #[error("Musubi pin-outbox Check has no successful local application proof")]
    NotApplied,
    /// Original bounded native history or its certified execution relation is unavailable.
    #[error("Musubi pin-outbox Check finality unavailable")]
    Finality,
    /// Original State identity, publication generation or complete current row differs.
    #[error("Musubi pin-outbox current State differs")]
    CurrentState,
}
use MusubiPinOutboxCheckErrorV1 as Error;

impl From<NativeCheckErrorV1> for Error {
    fn from(error: NativeCheckErrorV1) -> Self {
        match error {
            NativeCheckErrorV1::Invalid => Self::Invalid,
            NativeCheckErrorV1::Expired => Self::Expired,
            NativeCheckErrorV1::Entropy => Self::Entropy,
            NativeCheckErrorV1::Transaction => Self::Transaction,
            NativeCheckErrorV1::NotApplied => Self::NotApplied,
            NativeCheckErrorV1::Finality | NativeCheckErrorV1::Execution => Self::Finality,
        }
    }
}

/// Original fresh challenge and bounded exact instruction; no Clone or decoded constructor.
#[must_use = "bind only the exact signed Check within its original deadline"]
pub struct PreparedMusubiPinOutboxCheckV1 {
    state: Arc<State>,
    chain_id: ChainId,
    instruction: CheckMusubiPinOutboxV1,
    round: NativeCheckRoundV1,
}

/// One original signed Check awaiting native execution authentication.
#[must_use = "reconcile and verify the original signed Check without re-signing"]
pub struct PendingMusubiPinOutboxCheckV1 {
    prepared: PreparedMusubiPinOutboxCheckV1,
    bound: BoundNativeCheckV1,
}

/// One verified native execution, still requiring the final same-State current-row fence.
#[must_use = "consume against the exact original State before using this readback"]
pub struct VerifiedMusubiPinOutboxCheckV1 {
    prepared: PreparedMusubiPinOutboxCheckV1,
    bound: BoundNativeCheckV1,
    generation: u64,
    applied_floor: MusubiPinOutboxCheckFloorV1,
}

/// A failed read attempt retaining the unchanged original paid Check for reconciliation.
/// No failure renews the challenge, signed bytes, floor, expected row or absolute deadline.
pub struct MusubiPinOutboxCheckAttemptFailureV1 {
    error: Error,
    pending: PendingMusubiPinOutboxCheckV1,
}
impl MusubiPinOutboxCheckAttemptFailureV1 {
    /// Payload-free reason for this read refusal.
    #[must_use]
    pub const fn error(&self) -> Error {
        self.error
    }
    /// Recover original custody and require a fresh native verification before consumption.
    #[must_use]
    pub fn into_pending(self) -> PendingMusubiPinOutboxCheckV1 {
        self.pending
    }
}
impl core::fmt::Debug for MusubiPinOutboxCheckAttemptFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPinOutboxCheckAttemptFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl core::fmt::Display for MusubiPinOutboxCheckAttemptFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.error, formatter)
    }
}
impl std::error::Error for MusubiPinOutboxCheckAttemptFailureV1 {}

/// One consumed, read-only native observation at an exact publication cut.
///
/// This has no Clone, codec or public constructor. Its historical facts do not authorize
/// later effects, rollback recovery, replacement custody, signing or Queue submission.
pub struct MusubiPinOutboxCurrentReadbackV1 {
    instruction: CheckMusubiPinOutboxV1,
    applied_floor: MusubiPinOutboxCheckFloorV1,
    canonical_external: Vec<u8>,
    check_block_hash: [u8; 32],
}

fn native_floor(floor: MusubiPinOutboxCheckFloorV1) -> NativeCheckFloorV1 {
    NativeCheckFloorV1 {
        height: floor.height,
        block_hash: floor.block_hash,
        context_id: floor.context_id,
    }
}

fn public_floor(floor: NativeCheckFloorV1) -> MusubiPinOutboxCheckFloorV1 {
    MusubiPinOutboxCheckFloorV1 {
        height: floor.height,
        block_hash: floor.block_hash,
        context_id: floor.context_id,
    }
}

fn current_row(view: &StateView<'_>, instruction: &CheckMusubiPinOutboxV1) -> Result<(), Error> {
    let actual = view
        .world
        .musubi_pin_outbox_high_waters()
        .get(&instruction.pin_authority);
    match (&instruction.expected, actual) {
        (MusubiPinOutboxCheckExpectationV1::Absent, None) => Ok(()),
        (MusubiPinOutboxCheckExpectationV1::Present(expected), Some(actual))
            if expected == actual =>
        {
            Ok(())
        }
        _ => Err(Error::CurrentState),
    }
}

fn global_instance(
    prepared: &PreparedMusubiPinOutboxCheckV1,
) -> Result<iroha_sumeragi::types::Hash32, Error> {
    // This uses only the existing native instance hash derivation, not a verifier or key set.
    SumeragiRootScope::Global
        .instance_id(
            &BlsCrypto::new(),
            prepared.instruction.network_id,
            prepared.chain_id.as_str(),
        )
        .map_err(|_| Error::Invalid)
}

/// Begin one fresh challenged readback within the caller's original absolute deadline.
///
/// The independent floor is verified against original native execution before any signed
/// Check is requested. Genesis alone cannot authenticate its execution result. A successful
/// preparation grants no signing credential, permission, custody or Queue capability.
///
/// # Errors
/// Refuses invalid fields, an expired/overlong deadline, changed current rows, a different
/// network/chain/private root, missing native history or the inherited resource allowance.
pub fn begin_musubi_pin_outbox_check_v1(
    state: Arc<State>,
    expected: MusubiPinOutboxCheckExpectedV1,
    deadline: Instant,
) -> Result<PreparedMusubiPinOutboxCheckV1, Error> {
    let mut round = NativeCheckRoundV1::start_until(deadline)?;
    with_native_check_read_limits(|| {
        iroha_primitives::chain_id::validate_chain_id(expected.chain_id.as_str())
            .map_err(|_| Error::Invalid)?;
        validate_native_signatory_v1(&expected.pin_authority)?;
        let mut instruction = CheckMusubiPinOutboxV1 {
            network_id: expected.network_id,
            pin_authority: expected.pin_authority,
            session_id: expected.session_id,
            inventory_digest: expected.inventory_digest,
            challenge: [1; 32],
            floor: expected.floor,
            expected: expected.expected,
        };
        if norito::canonical_frame_len(&instruction).map_err(|_| Error::Invalid)?
            > MUSUBI_PIN_OUTBOX_CHECK_MAX_BYTES_V1
        {
            return Err(Error::Invalid);
        }
        instruction.validate_fields().map_err(|_| Error::Invalid)?;
        instruction.challenge = round.issue_challenge()?;
        let prepared = PreparedMusubiPinOutboxCheckV1 {
            state,
            chain_id: expected.chain_id,
            instruction,
            round,
        };
        let generation = prepared.state.state_view_generation();
        let view = prepared.state.view();
        if !is_stable_state_view_generation(generation, prepared.state.state_view_generation())
            || view.network_id() != &prepared.instruction.network_id
            || view.chain_id() != &prepared.chain_id
        {
            return Err(Error::CurrentState);
        }
        current_row(&view, &prepared.instruction)?;
        let chain = SignerCertifiedWalkV1::new(&view)?;
        let expected_instance = global_instance(&prepared)?;
        let floor = prepared.instruction.floor;
        for receipt in chain.walk(floor.height, floor.height.max(2)) {
            prepared.round.ensure_live()?;
            let receipt = receipt?;
            let block = receipt.in_view(&view)?;
            if block.height() == floor.height
                && (*block.block_hash().as_ref() != floor.block_hash
                    || block.id() != floor.context_id)
            {
                return Err(Error::Finality);
            }
            if block.height() >= 2
                && block
                    .header()
                    .is_none_or(|header| header.instance != expected_instance)
            {
                return Err(Error::Finality);
            }
        }
        prepared.round.ensure_live()?;
        if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
            return Err(Error::CurrentState);
        }
        drop(chain);
        drop(view);
        Ok(prepared)
    })
}

impl PreparedMusubiPinOutboxCheckV1 {
    /// Exact bounded instruction which the configured pin authority must sign alone.
    #[must_use]
    pub const fn instruction(&self) -> &CheckMusubiPinOutboxV1 {
        &self.instruction
    }
    /// Unchanged absolute deadline established before preparation.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.round.deadline()
    }
    /// Bind the exact sole signed native Check without renewing the original round.
    ///
    /// # Errors
    /// Consumes the preparation on expiry, signature/profile failure, field substitution or capacity refusal.
    pub fn bind_signed_transaction(
        mut self,
        signed: SignedTransaction,
    ) -> Result<PendingMusubiPinOutboxCheckV1, Error> {
        let bound = bind_signed_check_v1(
            &mut self.round,
            NativeCustodyCheckRefV1::MusubiPinOutbox(&self.instruction),
            self.chain_id.as_str(),
            *self.instruction.network_id.as_bytes(),
            &self.instruction.pin_authority,
            native_floor(self.instruction.floor),
            signed,
        )?;
        Ok(PendingMusubiPinOutboxCheckV1 {
            prepared: self,
            bound,
        })
    }
}

fn authenticate<'view, 'state>(
    view: &'view StateView<'state>,
    prepared: &PreparedMusubiPinOutboxCheckV1,
    bound: BoundNativeCheckV1,
) -> Result<
    (
        BorrowedCheckExecutionCutV1<'view, 'state>,
        BoundNativeCheckV1,
    ),
    (BoundNativeCheckV1, Error),
> {
    with_native_check_read_limits(|| {
        let mut proof = PreparedCheckExecutionV1::new_retaining(
            view,
            NativeCustodyCheckPurposeV1::MusubiPinOutbox,
            bound,
            &prepared.round,
        )
        .map_err(|(bound, error)| (bound, error.into()))?;
        let checked = (|| {
            let chain = SignerCertifiedWalkV1::new(view)?;
            let instance = global_instance(prepared)?;
            for receipt in chain.walk(proof.floor_height(), proof.applied_height()) {
                prepared.round.ensure_live()?;
                let receipt = receipt?;
                let block = receipt.in_view(view)?;
                if block.height() >= 2
                    && block
                        .header()
                        .is_none_or(|header| header.instance != instance)
                {
                    return Err(Error::Finality);
                }
                proof.consume(&receipt)?;
            }
            current_row(view, &prepared.instruction)
        })();
        if let Err(error) = checked {
            return Err((proof.into_bound(), error));
        }
        proof
            .finish_retaining_attempt()
            .map_err(|(bound, error)| (bound, error.into()))
    })
}

fn verify_attempt(
    prepared: &PreparedMusubiPinOutboxCheckV1,
    bound: BoundNativeCheckV1,
) -> Result<(BoundNativeCheckV1, u64, MusubiPinOutboxCheckFloorV1), (BoundNativeCheckV1, Error)> {
    if let Err(error) = prepared.round.ensure_live() {
        return Err((bound, error.into()));
    }
    let generation = prepared.state.state_view_generation();
    let view = prepared.state.view();
    if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
        return Err((bound, Error::CurrentState));
    }
    let (cut, bound) = authenticate(&view, prepared, bound)?;
    let applied_floor = public_floor(cut.applied_floor());
    if let Err(error) = prepared.round.ensure_live() {
        return Err((bound, error.into()));
    }
    if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
        return Err((bound, Error::CurrentState));
    }
    Ok((bound, generation, applied_floor))
}

impl PendingMusubiPinOutboxCheckV1 {
    /// Original exact signed Check; callers must retain its bytes before dispatch.
    #[must_use]
    pub fn signed_transaction(&self) -> &SignedTransaction {
        self.bound.signed_transaction()
    }
    /// Original absolute deadline, never extended by submission or reconciliation.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.prepared.round.deadline()
    }
    /// Authenticate exact successful execution and full current row on the original State.
    ///
    /// # Errors
    /// Returns unchanged original custody on missing/failed execution, changed rows, expiry or unavailable history.
    pub fn verify_finalized(
        self,
    ) -> Result<VerifiedMusubiPinOutboxCheckV1, MusubiPinOutboxCheckAttemptFailureV1> {
        match verify_attempt(&self.prepared, self.bound) {
            Ok((bound, generation, applied_floor)) => Ok(VerifiedMusubiPinOutboxCheckV1 {
                prepared: self.prepared,
                bound,
                generation,
                applied_floor,
            }),
            Err((bound, error)) => Err(MusubiPinOutboxCheckAttemptFailureV1 {
                error,
                pending: PendingMusubiPinOutboxCheckV1 {
                    prepared: self.prepared,
                    bound,
                },
            }),
        }
    }
}

fn consume_attempt(
    prepared: &PreparedMusubiPinOutboxCheckV1,
    bound: BoundNativeCheckV1,
    generation: u64,
    applied_floor: MusubiPinOutboxCheckFloorV1,
    expected_state: &Arc<State>,
) -> Result<(Vec<u8>, [u8; 32]), (BoundNativeCheckV1, Error)> {
    if !Arc::ptr_eq(expected_state, &prepared.state) {
        return Err((bound, Error::CurrentState));
    }
    if let Err(error) = prepared.round.ensure_live() {
        return Err((bound, error.into()));
    }
    if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
        return Err((bound, Error::CurrentState));
    }
    let view = prepared.state.view();
    if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
        return Err((bound, Error::CurrentState));
    }
    let (cut, bound) = authenticate(&view, prepared, bound)?;
    if public_floor(cut.applied_floor()) != applied_floor {
        return Err((bound, Error::CurrentState));
    }
    let checked = {
        let _publication = prepared.state.musubi_pin_outbox_publication_lease();
        if !is_stable_state_view_generation(generation, prepared.state.state_view_generation()) {
            Err(Error::CurrentState)
        } else {
            current_row(&view, &prepared.instruction)
                .and_then(|()| prepared.round.ensure_live().map_err(Error::from))
        }
    };
    match checked {
        Ok(()) => Ok(cut.into_verified_entry()),
        Err(error) => Err((bound, error)),
    }
}

impl VerifiedMusubiPinOutboxCheckV1 {
    /// Consume under the exact recipient State and a final short current-row publication fence.
    ///
    /// Native history reads precede the lease. Under it this owner checks only the original
    /// generation, deadline and complete authority-keyed row. No caller callback can replace
    /// those predicates or perform effects. Failure retains the original Pending Check and drops
    /// only this verified cut; retry must reauthenticate before another consumption.
    ///
    /// # Errors
    /// Returns original custody on another State (even byte-identical), intervening publication,
    /// changed rows, missing native source, inherited resource refusal or the original deadline.
    pub fn consume_current(
        self,
        expected_state: &Arc<State>,
    ) -> Result<MusubiPinOutboxCurrentReadbackV1, MusubiPinOutboxCheckAttemptFailureV1> {
        match consume_attempt(
            &self.prepared,
            self.bound,
            self.generation,
            self.applied_floor,
            expected_state,
        ) {
            Ok((canonical_external, check_block_hash)) => Ok(MusubiPinOutboxCurrentReadbackV1 {
                instruction: self.prepared.instruction,
                applied_floor: self.applied_floor,
                canonical_external,
                check_block_hash,
            }),
            Err((bound, error)) => Err(MusubiPinOutboxCheckAttemptFailureV1 {
                error,
                pending: PendingMusubiPinOutboxCheckV1 {
                    prepared: self.prepared,
                    bound,
                },
            }),
        }
    }
}

impl MusubiPinOutboxCurrentReadbackV1 {
    /// Exact successfully executed Check, including independent session/inventory and challenge.
    #[must_use]
    pub const fn instruction(&self) -> &CheckMusubiPinOutboxV1 {
        &self.instruction
    }
    /// Authority-wide absence or the entire authenticated expected row at this cut.
    #[must_use]
    pub fn high_water(&self) -> Option<&MusubiPinOutboxHighWaterV1> {
        match &self.instruction.expected {
            MusubiPinOutboxCheckExpectationV1::Absent => None,
            MusubiPinOutboxCheckExpectationV1::Present(row) => Some(row),
        }
    }
    /// Native applied tip authenticated through the original floor and exact signed Check.
    #[must_use]
    pub const fn applied_floor(&self) -> MusubiPinOutboxCheckFloorV1 {
        self.applied_floor
    }
    /// Exact successfully executed canonical External bytes, retained within the shared bound.
    #[must_use]
    pub fn canonical_external(&self) -> &[u8] {
        &self.canonical_external
    }
    /// Canonical hash of the block containing that exact successful Check.
    #[must_use]
    pub const fn check_block_hash(&self) -> [u8; 32] {
        self.check_block_hash
    }
}

#[cfg(test)]
mod tests;
