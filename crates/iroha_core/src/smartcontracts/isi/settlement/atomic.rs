//! Exact-consent, prefunded N-payment atomic settlement execution.
//!
//! TODO: Complete composed native tests and network qualification before claiming
//! matched-control execution results.

use super::*;
use iroha_data_model::isi::{ResolvedSettlementMovements, SettleAtomic, SettlementDetails};

/// An unforgeable, non-reusable capability issued only after complete intent validation.
pub(in crate::smartcontracts::isi) struct VerifiedSettlementNumericBatch {
    authority: AccountId,
    movements: ResolvedSettlementMovements,
}
impl VerifiedSettlementNumericBatch {
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (AccountId, ResolvedSettlementMovements) {
        (self.authority, self.movements)
    }
}

fn validate_atomic_intent(
    authority: &AccountId,
    stx: &StateTransaction<'_, '_>,
    instruction: &SettleAtomic,
) -> Result<(VerifiedSettlementNumericBatch, Hash), Error> {
    instruction
        .validate()
        .map_err(|message| InstructionExecutionError::InvariantViolation(message.into()))?;
    if instruction.network_id() != &stx.network_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "atomic settlement network does not match the executing genesis".into(),
        ));
    }
    if stx._curr_block.height().get() > instruction.expires_at_height().get() {
        return Err(InstructionExecutionError::InvariantViolation(
            "atomic settlement inclusive expiry height has passed".into(),
        ));
    }
    ensure_settlement_id_unused(stx, instruction.settlement_id())?;
    stx.world.account(authority)?;
    let intent_hash = instruction.intent_hash().map_err(|error| {
        InstructionExecutionError::InvariantViolation(
            format!("atomic settlement canonical intent cannot be encoded: {error}").into(),
        )
    })?;
    let consents =
        settlement_consent_sources(stx, authority, instruction.settlement_id(), intent_hash);
    for movement in instruction.movements().as_slice() {
        // The caller's exact signed instruction authorizes its own balances.
        // Every other source needs owner-issued consent to the complete intent.
        if movement.source.account() != authority && !consents.contains(&movement.source) {
            return Err(InstructionExecutionError::InvariantViolation(
                "atomic settlement requires exact whole-intent consent for every foreign source bucket".into(),
            ));
        }
    }
    let movements = instruction
        .movements()
        .resolve()
        .map_err(|message| InstructionExecutionError::InvariantViolation(message.into()))?;
    Ok((
        VerifiedSettlementNumericBatch {
            authority: authority.clone(),
            movements,
        },
        intent_hash,
    ))
}

/// Validate exact atomic intent, current owner consents and complete movement policies.
///
/// This path prepares the same whole batch as execution and never applies it.
pub(crate) fn admission_validate_atomic(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    instruction: &SettleAtomic,
) -> Result<(), Error> {
    let (authorization, _) = validate_atomic_intent(authority, stx, instruction)?;
    crate::smartcontracts::isi::asset::isi::validate_verified_settlement_numeric_batch(
        stx,
        authorization,
    )
}

impl Execute for SettleAtomic {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let (authorization, intent_hash) = validate_atomic_intent(authority, stx, &self)?;
        // Construct all receipt material before the first balance mutation. The
        // asset owner will reject any prepared bucket different from this exact
        // signed vector, so receipt scopes never depend on post-execution lookup.
        let receipt = settlement_receipt(
            stx,
            authority,
            self.metadata().clone(),
            SettlementDetails::Atomic(iroha_data_model::isi::AtomicSettlementDetails {
                movements: authorization.movements.clone(),
                intent_hash,
            }),
        );
        crate::smartcontracts::isi::asset::isi::execute_verified_settlement_numeric_batch(
            stx,
            authorization,
        )?;
        // The unique ID was checked in this exclusive StateTransaction, and no
        // callback or other instruction can insert a receipt while the private
        // asset plan applies. All remaining operations are infallible staging.
        stx.world
            .settlement_receipts
            .insert(self.settlement_id().clone(), receipt);
        Ok(())
    }
}
