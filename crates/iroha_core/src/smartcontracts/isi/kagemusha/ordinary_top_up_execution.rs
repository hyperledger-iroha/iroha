// Actual block-executor ordinary Mint consumer; included in the sole KAGEMUSHA ISI owner.

/// Consume actual ordinary proof and current World purpose before a protected reserve debit.
///
/// An existing exact World record returns its historical data without another debit. A new
/// effect requires independently verified ordinary proof, the original signed decision still
/// valid at actual block time, current release/incarnation and exact reserve indexes. The
/// returned record is applied data; it is not consensus finality or an incoming State grant.
/// # Errors
/// Refuses changed original, payer, purpose, release, incarnation, pool head or reverse index.
pub fn settle_ordinary_kagemusha_top_up_v1(
    proof: iroha_core_zk::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1,
    decision: super::ordinary_mint_debit_admission::KagemushaWorldOrdinaryMintDebitDecisionV1,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<super::kagemusha_v1_reserve::KagemushaOrdinaryTopUpRecordV1, Error> {
    use super::kagemusha_v1_reserve::ordinary_top_up::{
        AdmittedOrdinaryTopUp, OrdinaryTopUpOutcome, plan_ordinary_top_up,
        validate_ordinary_top_up_commit,
    };
    let request =
        iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
            proof.request_original(),
        )
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_invalid", e))?;
    let context = &request.authorization.statement.context;
    let runtime = &context.lineage.owner.runtime;
    if authority != &context.lineage.owner.account_id
        || proof.authorization() != &request.authorization
    {
        return Err(kagemusha_v1_error(
            "ordinary_top_up_payer_or_proof_mismatch",
            "signed payer or closed original differs",
        ));
    }
    let operation_id = context.operation_id;
    // Only actual retained immutable World entries may supply historical recovery. A new Core
    // decision cannot replace the original, renew the receipt or repeat the protected debit.
    if let Some(existing) = state_transaction
        .world
        .kagemusha_reserve_operations
        .get(&operation_id)
    {
        let KagemushaReserveOperationRecordV1::OrdinaryTopUp(record) = existing else {
            return Err(kagemusha_v1_error(
                "ordinary_top_up_operation_conflict",
                "operation belongs to another family",
            ));
        };
        record
            .validate_basic()
            .map_err(|e| kagemusha_v1_error("ordinary_top_up_invalid", e))?;
        if record.request_original != proof.request_original()
            || record.issuer_decision_original != decision.decision_original()
            || &record.payer != authority
            || state_transaction
                .world
                .kagemusha_mint_credit_operations
                .get(&record.credit_id)
                .copied()
                != Some(operation_id)
            || state_transaction
                .world
                .kagemusha_issuance_operations
                .get(&record.issuance_commitment)
                .copied()
                != Some(operation_id)
        {
            return Err(kagemusha_v1_error(
                "ordinary_top_up_operation_conflict",
                "original or immutable reverse index differs",
            ));
        }
        let recovered = record.as_ref().clone();
        record_kagemusha_v1_receipt_read(operation_id, Some(existing))?;
        return Ok(recovered);
    }
    if &runtime.network_id != state_transaction.network_id()
        || state_transaction
            .world
            .axt_asset_incarnations
            .get(&runtime.asset)
            .copied()
            != Some(runtime.asset_incarnation)
    {
        return Err(kagemusha_v1_error(
            "ordinary_top_up_live_scope_mismatch",
            "network or exact asset incarnation differs",
        ));
    }
    require_execution_runtime(
        state_transaction,
        context.release_id,
        execution_availability::Operation::TopUp,
    )?;
    let amount = kagemusha_v1_amount(
        context.amount,
        runtime.scale,
        &runtime.asset,
        state_transaction,
    )?;
    let source_id = canonical_kagemusha_asset_id(
        state_transaction,
        &AssetId::new(runtime.asset.clone(), authority.clone()),
    )?;
    let pool = KagemushaReservePoolKeyV1::new(
        runtime.network_id,
        runtime.asset.clone(),
        runtime.asset_incarnation,
    )
    .map_err(|e| kagemusha_v1_error("ordinary_top_up_pool_invalid", e))?;
    let commit_context = kagemusha_v1_commit_context(state_transaction)?;
    let admitted = AdmittedOrdinaryTopUp::new(proof, decision, state_transaction, authority)
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_admission_invalid", e))?;
    let request = admitted.request();
    let credit_id = request.authorization.statement.credit_id;
    let issuance_commitment = request
        .authorization
        .statement
        .context
        .issuance_commitment()
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_issuance_invalid", e))?;
    let read = KagemushaTopUpReadSetV1 {
        current_pool: state_transaction
            .world
            .kagemusha_reserve_pools
            .get(&pool.liability_pool_id),
        existing_operation: state_transaction
            .world
            .kagemusha_reserve_operations
            .get(&operation_id),
        credit_operation: state_transaction
            .world
            .kagemusha_mint_credit_operations
            .get(&credit_id)
            .copied(),
        issuance_operation: state_transaction
            .world
            .kagemusha_issuance_operations
            .get(&issuance_commitment)
            .copied(),
    };
    record_kagemusha_v1_receipt_read(operation_id, read.existing_operation)?;
    let plan = match plan_ordinary_top_up(&admitted, commit_context, read)
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_reserve_invalid", e))?
    {
        OrdinaryTopUpOutcome::AlreadyCommitted(record) => return Ok(record),
        OrdinaryTopUpOutcome::Commit(plan) => plan,
    };
    if let Some(record) = validate_ordinary_top_up_commit(
        &plan,
        KagemushaTopUpReadSetV1 {
            current_pool: state_transaction
                .world
                .kagemusha_reserve_pools
                .get(&pool.liability_pool_id),
            existing_operation: state_transaction
                .world
                .kagemusha_reserve_operations
                .get(&operation_id),
            credit_operation: state_transaction
                .world
                .kagemusha_mint_credit_operations
                .get(&credit_id)
                .copied(),
            issuance_operation: state_transaction
                .world
                .kagemusha_issuance_operations
                .get(&issuance_commitment)
                .copied(),
        },
    )
    .map_err(|e| kagemusha_v1_error("ordinary_top_up_stale_plan", e))?
    {
        return Ok(record);
    }
    admitted
        .recheck(state_transaction, authority)
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_current_decision_invalid", e))?;
    let reserve_account =
        resolve_kagemusha_reserve_account(state_transaction, &plan.record().pool.asset)?;
    ensure_distinct_kagemusha_reserve_account(
        &reserve_account,
        authority,
        "ordinary payer",
        &plan.record().pool.asset,
    )?;
    let destination_id = kagemusha_reserve_asset_id(&source_id, reserve_account);
    crate::smartcontracts::isi::asset::isi::execute_verified_kagemusha_top_up_transfer_v1(
        state_transaction,
        VerifiedKagemushaTopUpDebitV1::new(
            authority.clone(),
            operation_id,
            source_id,
            destination_id,
            amount,
        ),
    )?;
    let record = plan.record().clone();
    state_transaction
        .world
        .kagemusha_reserve_pools
        .insert(record.pool.liability_pool_id, plan.next_pool().clone());
    state_transaction.world.kagemusha_reserve_operations.insert(
        operation_id,
        KagemushaReserveOperationRecordV1::OrdinaryTopUp(Box::new(record.clone())),
    );
    let credit_result = state_transaction
        .world
        .kagemusha_mint_credit_operations
        .try_insert_admitted(credit_id, operation_id);
    retain_operation_index_refusal(state_transaction, credit_result)?;
    let issuance_result = state_transaction
        .world
        .kagemusha_issuance_operations
        .try_insert_admitted(issuance_commitment, operation_id);
    retain_operation_index_refusal(state_transaction, issuance_result)?;
    crate::exec_witness::record_write_kagemusha_reserve_receipt_v1(&record.reserve_receipt)
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_receipt_encoding_failed", e))?;
    Ok(record)
}

// Read-only exact historical acknowledgement from the authentic persisted World record.
fn recover_applied_ordinary_kagemusha_top_up_v1(
    submission: &iroha_data_model::kagemusha::KagemushaOrdinaryNodeMintSubmissionV1,
    authority: &AccountId,
    transaction: &mut StateTransaction<'_, '_>,
) -> Result<bool, Error> {
    submission
        .validate_shape()
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_invalid", e))?;
    let request =
        iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
            &submission.topup_request_original,
        )
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_invalid", e))?;
    request
        .verify_account_signature(&submission.account_consent)
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_consent_invalid", e))?;
    let context = &request.authorization.statement.context;
    if authority != &context.lineage.owner.account_id {
        return Err(kagemusha_v1_error(
            "ordinary_top_up_payer_invalid",
            "signed transaction owner differs",
        ));
    }
    let Some(existing) = transaction
        .world
        .kagemusha_reserve_operations
        .get(&context.operation_id)
    else {
        return Ok(false);
    };
    let KagemushaReserveOperationRecordV1::OrdinaryTopUp(record) = existing else {
        return Err(kagemusha_v1_error(
            "ordinary_top_up_operation_conflict",
            "actual record has another operation family",
        ));
    };
    record
        .validate_basic()
        .map_err(|e| kagemusha_v1_error("ordinary_top_up_history_invalid", e))?;
    if record.request_original != submission.topup_request_original
        || record.issuer_decision_original != submission.debit_decision_original
        || &record.payer != authority
        || transaction
            .world
            .kagemusha_mint_credit_operations
            .get(&record.credit_id)
            .copied()
            != Some(context.operation_id)
        || transaction
            .world
            .kagemusha_issuance_operations
            .get(&record.issuance_commitment)
            .copied()
            != Some(context.operation_id)
    {
        return Err(kagemusha_v1_error(
            "ordinary_top_up_operation_conflict",
            "full retained historical originals or reverse indexes differ",
        ));
    }
    record_kagemusha_v1_receipt_read(context.operation_id, Some(existing))?;
    Ok(true)
}
