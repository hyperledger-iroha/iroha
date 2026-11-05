// Ironwood-only request/authorization path. Existing Orchard wire and behavior remain fixed.
/// Prepare randomized Ironwood actions, bind the entire ordered transaction intent, then authorize
/// once. `public_debit` must exactly equal the transparent reserve bridge, including all change.
/// # Errors
/// Rejects a wrong profile, scope, anchor, public debit, call plan, key or native authorization.
pub fn build_signed_ironwood_note_action_v1(
    context: PrivacyActionTransactionContextV1,
    request: IronwoodNoteActionRequestV1,
    calls: Option<OrchardOrderedContractCallsV1>,
    canonical_genesis_hash: [u8; 32],
    private_key: &PrivateKey,
) -> Result<SignedPrivacyActionV1, PrivacyNativeActionErrorV1> {
    validate_action_preflight(&context, canonical_genesis_hash, private_key)?;
    if let Some(calls) = &calls {
        validate_orchard_ordered_context(&context, calls)?;
    }
    if request.anchor_epoch == 0 || request.expiry_height == 0 {
        return Err(PrivacyNativeActionErrorV1::at("ironwood-epoch-or-expiry"));
    }
    if request.public_balance_scope != AssetBalanceScope::Global {
        return Err(PrivacyNativeActionErrorV1::at(
            "ironwood-public-balance-scope",
        ));
    }
    let profile = compiled_profile(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)?;
    let prepared = prepare_ironwood_bundle_v1(
        request.anchor.into_bytes(),
        request.spends,
        request.outputs,
        request.minimum_action_count,
    )
    .map_err(|_| PrivacyNativeActionErrorV1::at("ironwood-prepare"))?;
    let output_mapping = IronwoodOutputActionMappingV1 {
        direct_instruction_index: calls.as_ref().map_or(0, |c| c.direct_instruction_index()),
        output_action_indices: prepared.output_action_indices().to_vec(),
    };
    let draft = prepared.public_draft().clone();
    if draft.anchor != request.anchor.into_bytes() {
        return Err(PrivacyNativeActionErrorV1::at("ironwood-anchor-drift"));
    }
    let debit = i64::try_from(request.public_debit)
        .map_err(|_| PrivacyNativeActionErrorV1::at("ironwood-public-debit"))?;
    if draft.value_balance != -debit {
        return Err(PrivacyNativeActionErrorV1::at(
            "ironwood-public-debit-mismatch",
        ));
    }
    let mut statement = OrchardHalo2ActionsStatementV1 {
        context: statement_context(&context, profile),
        asset_definition_id: request.asset_definition_id,
        public_balance_scope: request.public_balance_scope,
        pool_id: request.pool_id,
        anchor: request.anchor,
        anchor_epoch: request.anchor_epoch,
        actions: draft.actions.iter().map(orchard_action).collect(),
        value_balance: orchard_value_balance(draft.value_balance),
        expiry_height: request.expiry_height,
    };
    let draft_envelope = intent_projection_envelope(
        profile,
        PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement.clone()),
        PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(PrivacyProofBytesV1::new(Vec::new())),
    )?;
    let draft_payload = match &calls {
        Some(calls) => orchard_ordered_payload(&context, calls, Some(draft_envelope))?,
        None => transaction_payload(&context, Some(draft_envelope))?,
    };
    let intent = draft_payload
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyNativeActionErrorV1::at("transaction-intent"))?;
    statement.context.transaction_intent_digest = intent;
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let binding = PrivacyNativeConsensusBindingV1::new(
        &statement.context,
        canonical_genesis_hash,
        &consensus_limits,
    )
    .map_err(|_| PrivacyNativeActionErrorV1::at("ironwood-consensus-binding"))?;
    let proved = authorize_ironwood_bundle_v1(prepared, &binding, &consensus_limits)
        .map_err(|_| PrivacyNativeActionErrorV1::at("ironwood-authorization"))?;
    if proved.public.anchor != draft.anchor
        || proved.public.value_balance != draft.value_balance
        || proved.public.actions != draft.actions
    {
        return Err(PrivacyNativeActionErrorV1::at("ironwood-public-drift"));
    }
    let typed_statement = PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement);
    if let Some(calls) = &calls {
        let envelope = bind_envelope(
            profile,
            typed_statement,
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(PrivacyProofBytesV1::new(
                proved.authorization,
            )),
            "proof-envelope-validation",
        )?;
        envelope
            .validate_with_limits(&consensus_limits)
            .map_err(|_| PrivacyNativeActionErrorV1::at("proof-envelope-validation"))?;
        let payload = orchard_ordered_payload(&context, calls, Some(envelope))?;
        if payload
            .validate_privacy_transaction_intent_binding_v1()
            .map_err(|_| PrivacyNativeActionErrorV1::at("final-intent-binding"))?
            != intent
        {
            return Err(PrivacyNativeActionErrorV1::at("signed-intent-drift"));
        }
        let signed = TransactionBuilder::from_payload(payload)
            .map_err(|_| PrivacyNativeActionErrorV1::at("final-payload"))?
            .try_sign(private_key)
            .map_err(|_| PrivacyNativeActionErrorV1::at("transaction-signing"))?;
        inspect_signed_privacy_ironwood_note_action_with_contract_calls_v1(
            &signed, &context, calls,
        )?;
        let mut result = wrap_canonical_signed_transaction_v1(signed, profile.protocol_id)?;
        result.ironwood_output_mapping = Some(output_mapping);
        return Ok(result);
    }
    let mut result = finalize(
        &context,
        profile,
        typed_statement,
        PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(PrivacyProofBytesV1::new(
            proved.authorization,
        )),
        private_key,
    )?;
    result.ironwood_output_mapping = Some(output_mapping);
    Ok(result)
}
/// Authenticate the exact Ironwood ordered plan and compiled profile.
/// # Errors
/// Rejects altered source, signatures, profile, ordinal, context or before/after calls.
pub fn inspect_signed_privacy_ironwood_note_action_with_contract_calls_v1(
    signed: &SignedTransaction,
    expected_context: &PrivacyActionTransactionContextV1,
    expected_calls: &OrchardOrderedContractCallsV1,
) -> Result<InspectedOrchardOrderedActionV1, PrivacyNativeActionErrorV1> {
    validate_orchard_ordered_context(expected_context, expected_calls)?;
    if signed.encode_versioned().len() > PRIVACY_NATIVE_ACTION_MAX_SIGNED_TRANSACTION_BYTES_V1 {
        return Err(PrivacyNativeActionErrorV1::at(
            "signed-transaction-response-cap",
        ));
    }
    let action = inspect_signed(signed, PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)?;
    let (_, submission) = signed
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| PrivacyNativeActionErrorV1::at("inspect-intent"))?
        .ok_or_else(|| PrivacyNativeActionErrorV1::at("inspect-missing-action"))?;
    let submission_envelope = submission.runtime_envelope();
    if !compiled_profile(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)?
        .matches_envelope_v1(&submission_envelope)
    {
        return Err(PrivacyNativeActionErrorV1::at("inspect-ironwood-profile"));
    }
    let expected_payload = orchard_ordered_payload(
        expected_context,
        expected_calls,
        Some(submission_envelope.clone()),
    )?;
    if signed.payload() != &expected_payload || action.statement().context().action_index != 0 {
        return Err(PrivacyNativeActionErrorV1::at(
            "inspect-ironwood-ordered-payload",
        ));
    }
    Ok(InspectedOrchardOrderedActionV1 {
        action,
        direct_instruction_index: expected_calls.direct_instruction_index(),
    })
}
