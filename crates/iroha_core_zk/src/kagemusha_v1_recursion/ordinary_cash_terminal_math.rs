//! Complete ordinary Send/Redeem Terminal scalar relation and ordered recursive consumers.
//!
//! This child of `composite` shares the genuine State/Guard financial relation. Both outgoing
//! operations use one fixed topology, including the complete beneficiary/manifest openings and
//! empty inactive Send originals. No parsed original or padding lends a financial capability.
use super::super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaGuardBundleRelationWitnessV1,
    KagemushaOperationV1, KagemushaPastaParityV1,
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    deferred_parent::{
        KagemushaDeferredParentOutputV1, bind_accumulator_limbs, deferred_field_chips_v1,
        deferred_loader_v1, finalize_deferred_audit_plan_with_u128_binding_v1,
        load_native_accumulator, native_parent_protocol_digest_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    generation::KagemushaOrdinaryRecursivePreparedOpeningV1,
    guard_bundle::{
        assign_bytes, constant_bytes, constrain_guard_bundle_semantics_v1, digest_limbs_assigned,
        hash,
    },
    mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1,
    ordinary_cash_opening::{
        OrdinaryCashClockCellsV1, OrdinaryCashTerminalSourcesV1,
        constrain_ordinary_cash_terminal_opening_v1,
    },
    ordinary_cash_terminal_verifier::{
        ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1, OrdinaryCashTerminalPublicV1,
    },
    ordinary_guard_data_binding::{
        KagemushaOrdinaryGuardDataBindingV1, constrain_ordinary_guard_data_binding_v1,
    },
    ordinary_guard_recursive_consumer::{
        KagemushaOrdinaryGuardCommitmentCellsV1, KagemushaOrdinaryGuardCompleteProofV1,
        constrain_ordinary_guard_complete_v1,
    },
    ordinary_issuer_config::OrdinaryIssuerTableV1,
    ordinary_receiver_request_opening::{
        OrdinaryReceiverRequestSourcesV1, OrdinaryReceiverRequestWitnessV1,
        constrain_ordinary_receiver_request_opening_v1,
    },
    ordinary_redeem_output_opening::{
        OrdinaryRedeemOutputSourcesV1, OrdinaryRedeemOutputWitnessV1,
        constrain_ordinary_redeem_output_opening_v1,
    },
    ordinary_send_output_opening::{
        OrdinarySendOutputSourcesV1, constrain_ordinary_send_output_opening_v1,
    },
    ordinary_state_reserved::constrain_ordinary_state_outer_protocol_positions_v1,
    state_relation::{self, KagemushaStateRelationWitnessV1, public_instance as state_slot},
    terminal_authorization::constrain_candidate_envelope_digest_v1,
    typed_sha_consumer::{
        KagemushaRecursiveHashClaimParityWitnessV1, constrain_recursive_hash_claim_v1,
    },
};
use super::{
    assigned_digest_bytes_v1, assigned_uint_bytes_v1, constrain_outer_state_head_v1,
    constrain_state_guard_binding_v1, constrain_unqualified_hardware_selection_v1,
    ordinary_state_prepared_binding::constrain_ordinary_state_prepared_opening_v1,
    ordinary_state_subject_binding::constrain_ordinary_state_subject_v1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use ff::Field as _;
use halo2_base::{
    AssignedValue, Context,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1, KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_OUTPUT_BINDING_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1, KagemushaAppOperationApprovalV1,
    KagemushaCreditOpeningV1, KagemushaHardwarePlatformClassV1,
    KagemushaHardwareSelectionSigningLayoutV1 as S, KagemushaOrdinaryAppCredentialV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryCashTerminalIntentV1,
    KagemushaOrdinaryCashTerminalRecordV1, KagemushaOrdinaryPaymentOutputV1,
    KagemushaOrdinaryPaymentRequestBodyV1, KagemushaOrdinaryPaymentRequestV1,
    KagemushaPlayIntegrityRefreshLeaseV1,
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
const AUDIT_EQ: usize = 41;
const AUDIT_EP: usize = 43;
const HISTORY: usize = 49;
const CANDIDATE_TAG: u32 = 0x4f43_5401;
const PREPARATION_GUARD_TAG: u32 = 0x4f43_5402;
const TERMINAL_GUARD_TAG: u32 = 0x4f43_5403;
const PREPARATION_MERGE_TAG: u32 = 0x4f43_5404;
const TERMINAL_MERGE_TAG: u32 = 0x4f43_5408;
const SHA_CURRENT_TAG: u32 = 0x4f43_5405;
const SHA_HISTORY_TAG: u32 = 0x4f43_5406;

/// Original mathematical sources lent only by the closed ordinary Native producer.
/// The neutral types themselves are not financial capabilities.
pub(in super::super) struct OrdinaryCashTerminalSemanticWitnessV1<'a> {
    pub(in super::super) state: &'a KagemushaStateRelationWitnessV1,
    pub(in super::super) preparation_relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(in super::super) terminal_relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(in super::super) sender_credential: &'a KagemushaOrdinaryAppCredentialV1,
    pub(in super::super) preparation_approval: &'a KagemushaAppOperationApprovalV1,
    pub(in super::super) preparation_integrity_lease:
        Option<&'a KagemushaPlayIntegrityRefreshLeaseV1>,
    pub(in super::super) terminal_approval: &'a KagemushaAppOperationApprovalV1,
    pub(in super::super) terminal_integrity_lease: Option<&'a KagemushaPlayIntegrityRefreshLeaseV1>,
    pub(in super::super) prepared: KagemushaOrdinaryRecursivePreparedOpeningV1<'a>,
    pub(in super::super) intent: &'a KagemushaOrdinaryCashTerminalIntentV1,
    pub(in super::super) record: &'a KagemushaOrdinaryCashTerminalRecordV1,
    pub(in super::super) preparation_clock: &'a KagemushaOrdinaryCashClockContextV1,
    pub(in super::super) issuer_table: &'a OrdinaryIssuerTableV1,
    pub(in super::super) outgoing: OrdinaryCashTerminalOutgoingWitnessV1<'a>,
}
/// Ordinary mathematical outgoing source selected by the actual Native closed producer.
/// Redeem requires no fictitious receiver owner, original request or platform approval.
pub(in super::super) enum OrdinaryCashTerminalOutgoingWitnessV1<'a> {
    Send {
        receiver: OrdinaryReceiverRequestWitnessV1<'a>,
        output: &'a KagemushaOrdinaryPaymentOutputV1,
        encrypted_credit: &'a [u8],
        credit_opening: &'a KagemushaCreditOpeningV1,
    },
    Redeem(OrdinaryRedeemOutputWitnessV1<'a>),
}
/// Deterministic data-only inactive codec specimen. No signature or receiver owner is admitted.
/// Only the disabled selector can consume this operand, and every receiver public digest is zero.
fn inactive_receiver_request_specimen(
    source: &OrdinaryCashTerminalSemanticWitnessV1<'_>,
) -> Result<KagemushaOrdinaryPaymentRequestV1, String> {
    let c = &source.sender_credential.subject;
    let clock = source.preparation_clock;
    let body = KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: c.release_id,
        network_id: c.network_id,
        normalized_asset_id: iroha_data_model::kagemusha::kagemusha_asset_identity_digest_v1(
            &source.state.successor.lane.asset,
        )
        .map_err(|e| e.to_string())?,
        asset_incarnation: *source.state.successor.asset_incarnation.as_bytes(),
        scale: source.state.successor.lane.scale,
        reserve_pool_id: source.state.successor.liability_pool_id,
        recipient_account_binding: c.account_binding,
        amount: source.state.amount,
        recipient_encryption_key: [9; 32],
        recipient_credential_digest: source.sender_credential.canonical_digest()?,
        recipient_lane_id: c.lane_id,
        request_id: [1; 32],
        clock_context: *clock,
        issued_at_ms: source.preparation_approval.challenge.issued_at_ms,
        expires_at_ms: source.preparation_approval.challenge.expires_at_ms,
    };
    let apple = c.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest;
    let evidence =
        super::super::ordinary_platform_union::inactive_receiver_codec_evidence_v1(apple)?;
    let value = KagemushaOrdinaryPaymentRequestV1 { body, evidence };
    value.validate_shape()?;
    Ok(value)
}
/// Complete current candidate and its exact original history.
pub(in super::super) struct OrdinaryCashCandidateCompleteProofV1<'a, C: CurveAffineExt> {
    pub(in super::super) protocol: &'a PlonkProtocol<C>,
    pub(in super::super) instances: &'a [Vec<C::ScalarExt>],
    pub(in super::super) proof: &'a [u8],
    pub(in super::super) history: &'a IpaAccumulator<C, NativeLoader>,
    pub(in super::super) history_fold_proof: &'a [u8],
}
/// All independently verified recursive inputs, including both purpose-scoped ordinary Guards.
pub(in super::super) struct OrdinaryCashTerminalHalfWitnessV1<'a, C: CurveAffineExt> {
    pub(in super::super) candidate: OrdinaryCashCandidateCompleteProofV1<'a, C>,
    pub(in super::super) preparation_guard: KagemushaOrdinaryGuardCompleteProofV1<'a, C>,
    pub(in super::super) terminal_guard: KagemushaOrdinaryGuardCompleteProofV1<'a, C>,
    /// First actual binary fold combines candidate and complete W2 Guard.
    pub(in super::super) preparation_merge_fold_proof: &'a [u8],
    /// Second actual binary fold combines that result and complete W1 Guard.
    pub(in super::super) terminal_merge_fold_proof: &'a [u8],
    pub(in super::super) hash_claim: KagemushaRecursiveHashClaimParityWitnessV1<'a, C>,
    pub(in super::super) successor_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}
impl<'a, C: CurveAffineExt> OrdinaryCashTerminalHalfWitnessV1<'a, C> {
    pub(in super::super) fn reborrow(&self) -> OrdinaryCashTerminalHalfWitnessV1<'_, C> {
        let guard = |g: &KagemushaOrdinaryGuardCompleteProofV1<'a, C>| {
            KagemushaOrdinaryGuardCompleteProofV1 {
                protocol: g.protocol,
                proof: g.proof,
                history: g.history,
                history_bytes: g.history_bytes,
                history_fold_proof: g.history_fold_proof,
            }
        };
        OrdinaryCashTerminalHalfWitnessV1 {
            candidate: OrdinaryCashCandidateCompleteProofV1 {
                protocol: self.candidate.protocol,
                instances: self.candidate.instances,
                proof: self.candidate.proof,
                history: self.candidate.history,
                history_fold_proof: self.candidate.history_fold_proof,
            },
            preparation_guard: guard(&self.preparation_guard),
            terminal_guard: guard(&self.terminal_guard),
            preparation_merge_fold_proof: self.preparation_merge_fold_proof,
            terminal_merge_fold_proof: self.terminal_merge_fold_proof,
            hash_claim: KagemushaRecursiveHashClaimParityWitnessV1 {
                protocol_digests: self.hash_claim.protocol_digests,
                protocol: self.hash_claim.protocol,
                instances: self.hash_claim.instances,
                proof: self.hash_claim.proof,
                history: self.hash_claim.history,
                history_fold_proof: self.hash_claim.history_fold_proof,
                merge_fold_proof: self.hash_claim.merge_fold_proof,
            },
            successor_history: self.successor_history,
        }
    }
}
pub(in super::super) struct OrdinaryCashTerminalSemanticAssignmentV1<F: KagemushaPoseidonFieldV1> {
    pub(in super::super) builder: BaseCircuitBuilder<F>,
    pub(in super::super) jobs: PastaSha256JobsV1<F>,
    public: Vec<AssignedValue<F>>,
    history: Vec<AssignedValue<F>>,
    candidate_instances: Vec<Vec<AssignedValue<F>>>,
    preparation: KagemushaOrdinaryGuardDataBindingV1<F>,
    terminal: KagemushaOrdinaryGuardDataBindingV1<F>,
}
fn bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: DigestV1,
) -> Bytes<F> {
    assign_bytes(ctx, range, &raw)
        .try_into()
        .expect("fixed digest32")
}
fn limbs_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: [AssignedValue<F>; 2],
) -> Bytes<F> {
    assigned_digest_bytes_v1(ctx, range.gate(), limbs)
        .try_into()
        .expect("fixed digest32")
}
fn equal_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    left: Bytes<F>,
    right: Bytes<F>,
) {
    for (a, b) in left.into_iter().zip(right) {
        let difference = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &difference, &F::ZERO);
    }
}
fn bind_limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    actual: [AssignedValue<F>; 2],
    expected: [AssignedValue<F>; 2],
) {
    for (a, b) in actual.into_iter().zip(expected) {
        ctx.constrain_equal(&a, &b);
    }
}
fn clock_cells<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    original: &KagemushaOrdinaryCashClockContextV1,
) -> Result<OrdinaryCashClockCellsV1<F>, String> {
    original.validate_shape()?;
    Ok(OrdinaryCashClockCellsV1 {
        nonce: bytes(ctx, range, original.request_nonce),
        signed_observations_original_digest: bytes(
            ctx,
            range,
            original.signed_observations_original_digest,
        ),
        lower_at_ms: ctx.load_witness(F::from(original.lower_at_ms)),
        upper_at_ms: ctx.load_witness(F::from(original.upper_at_ms)),
    })
}
fn assign_terminal_public<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &OrdinaryCashTerminalPublicV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<(Vec<AssignedValue<F>>, Vec<AssignedValue<F>>), String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let cells = public
        .public_prefix::<F>()?
        .into_iter()
        .map(|value| {
            let cell = ctx.load_witness(value);
            range.range_check(ctx, cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    let history = history
        .chunks_exact(16)
        .map(|raw| {
            let cell = ctx.load_witness(from_u128::<F>(u128::from_le_bytes(
                raw.try_into().expect("full history limb"),
            )));
            range.range_check(ctx, cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    if cells.len() != HISTORY || history.len() != 34 {
        return Err("ordinary Terminal public/history shape differs".into());
    }
    let role = ctx.load_witness(F::from(0x4f43_5407_u64));
    range
        .gate()
        .assert_is_const(ctx, &role, &F::from(0x4f43_5407_u64));
    Ok((cells, history))
}

/// Derive the entire ordered SHA queue from the same actual assigned financial transition.
/// No decoded request, body, proof or Native-looking projection selects a financial authority.
pub(in super::super) fn assign_ordinary_cash_terminal_semantics_v1<F: KagemushaPoseidonFieldV1>(
    parity: KagemushaPastaParityV1,
    public: &OrdinaryCashTerminalPublicV1,
    source: &OrdinaryCashTerminalSemanticWitnessV1<'_>,
    candidate_instances: &[Vec<F>],
    successor_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<OrdinaryCashTerminalSemanticAssignmentV1<F>, String> {
    public.validate()?;
    let send = source.state.operation == KagemushaOperationV1::SendSplit;
    let redeem = source.state.operation == KagemushaOperationV1::RedeemSplit;
    if (!send && !redeem) || public.operation != if send { 2 } else { 4 } {
        return Err("ordinary Terminal requires its actual outgoing State operation".into());
    }
    let inactive_request;
    let inactive_output;
    let empty_encrypted: &[u8] = &[];
    let (receiver_witness, output_witness, encrypted_credit, credit_opening, redemption_witness) =
        match &source.outgoing {
            OrdinaryCashTerminalOutgoingWitnessV1::Send {
                receiver,
                output,
                encrypted_credit,
                credit_opening,
            } if send => {
                if !receiver.enabled
                    || encrypted_credit.len() != KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
                {
                    return Err(
                        "ordinary Send requires actual receiver and encrypted originals".into(),
                    );
                }
                (
                    OrdinaryReceiverRequestWitnessV1 {
                        request: receiver.request,
                        credential: receiver.credential,
                        integrity_lease: receiver.integrity_lease,
                        previous_app_attest_counter: receiver.previous_app_attest_counter,
                        enabled: true,
                    },
                    *output,
                    *encrypted_credit,
                    Some(*credit_opening),
                    None,
                )
            }
            OrdinaryCashTerminalOutgoingWitnessV1::Redeem(redemption) if redeem => {
                inactive_request = inactive_receiver_request_specimen(source)?;
                inactive_output = KagemushaOrdinaryPaymentOutputV1 {
                    version: 1,
                    request_digest: [0; 32],
                    amount: 0,
                    sender_before_commitment: [0; 32],
                    sender_after_commitment: [0; 32],
                    transition_nullifier: [0; 32],
                    credit_id: [0; 32],
                    ciphertext_commitment: [0; 32],
                    encrypted_credit_digest: [0; 32],
                    clock_context_digest: [0; 32],
                    prepared_at_ms: 0,
                };
                (
                    OrdinaryReceiverRequestWitnessV1 {
                        request: &inactive_request,
                        credential: source.sender_credential,
                        integrity_lease: source.preparation_integrity_lease,
                        previous_app_attest_counter: None,
                        enabled: false,
                    },
                    &inactive_output,
                    empty_encrypted,
                    None,
                    Some(OrdinaryRedeemOutputWitnessV1 {
                        output: redemption.output,
                        beneficiary: redemption.beneficiary,
                        manifest_original: redemption.manifest_original,
                    }),
                )
            }
            _ => {
                return Err(
                    "ordinary Terminal outgoing witness differs from its State operation".into(),
                );
            }
        };
    if candidate_instances.len() != 1
        || candidate_instances[0].len()
            != state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT + 34
    {
        return Err("ordinary Terminal requires the complete127-cell candidate".into());
    }
    source.intent.validate_shape()?;
    source.record.validate_shape()?;

    let (mut builder, assigned) =
        state_relation::relation_builder_with_bindings::<F>(Some(source.state))?;
    constrain_unqualified_hardware_selection_v1(&mut builder, &assigned);
    let mut jobs = PastaSha256JobsV1::default();
    let prep_guard =
        constrain_guard_bundle_semantics_v1(&mut builder, &mut jobs, source.preparation_relation)?;
    constrain_state_guard_binding_v1(&mut builder, &assigned, &prep_guard)?;
    let preparation = constrain_ordinary_guard_data_binding_v1(
        &mut builder,
        &mut jobs,
        &prep_guard,
        source.sender_credential,
        source.preparation_approval,
        source.preparation_integrity_lease,
    )?;
    let transition = state_relation::constrain_transition_statement_digest_v1(
        &mut builder,
        &mut jobs,
        &assigned,
        source.state,
    )?;
    constrain_ordinary_state_subject_v1(
        &mut builder,
        &mut jobs,
        &assigned,
        source.state,
        &transition,
        &preparation,
    )?;
    let transition_limbs = digest_limbs_assigned(builder.main(0), &transition);
    builder.assigned_instances[0].extend(transition_limbs);
    let prepared = state_relation::assign_prepared_intent_public_v1(
        &mut builder,
        assigned.operation,
        source.state.prepared_intent,
    );
    builder.assigned_instances[0].extend_from_slice(&prepared);
    let lengths = constrain_ordinary_state_prepared_opening_v1(
        &mut builder,
        &mut jobs,
        &assigned,
        source.state,
        &preparation,
        &transition,
        &prepared,
        Some(source.prepared),
    )?;
    let state_semantic = builder.assigned_instances[0].clone();
    if state_semantic.len() != state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT {
        return Err("ordinary Terminal assigned State semantic shape differs".into());
    }
    constrain_ordinary_state_outer_protocol_positions_v1(
        &mut builder,
        [state_semantic[44], state_semantic[45]],
        [state_semantic[46], state_semantic[47]],
    );
    // The State consumed here is the actual OUTER pair. Both purposes use the same exact
    // release-held outer protocol cells, rather than caller-supplied credential-audit padding.
    for (actual, expected) in [
        (44, state_slot::EQ_PROTOCOL_LO),
        (46, state_slot::EP_PROTOCOL_LO),
    ] {
        for limb in 0..2 {
            builder.main(0).constrain_equal(
                &state_semantic[actual + limb],
                &state_semantic[expected + limb],
            );
        }
    }
    let predecessor = source
        .state
        .predecessor
        .as_ref()
        .ok_or("ordinary outgoing predecessor absent")?;
    let enabled = builder.main(0).load_constant(F::ONE);
    constrain_outer_state_head_v1(
        &mut builder,
        &mut jobs,
        predecessor.state_commitment_components,
        assigned.predecessor_eq_components,
        assigned.predecessor_ep_components,
        assigned.predecessor_outer,
        enabled,
    )?;
    constrain_outer_state_head_v1(
        &mut builder,
        &mut jobs,
        source.state.successor.state_commitment_components,
        assigned.successor_eq_components,
        assigned.successor_ep_components,
        assigned.successor_outer,
        enabled,
    )?;
    let candidate =
        constrain_candidate_envelope_digest_v1(&mut builder, &mut jobs, &state_semantic)?;
    let range = builder.range_chip();
    // Candidate public cells are copied to the exact State relation. This includes all4
    // prepared-stream/ID limbs and both transition SHA limbs; only whole history is new.
    let candidate_column = candidate_instances[0]
        .iter()
        .map(|v| builder.main(0).load_witness(*v))
        .collect::<Vec<_>>();
    for (actual, expected) in candidate_column[..state_semantic.len()]
        .iter()
        .zip(&state_semantic)
    {
        builder.main(0).constrain_equal(actual, expected);
    }
    for limb in &candidate_column[state_semantic.len()..] {
        range.range_check(builder.main(0), *limb, 128);
    }
    let (public_cells, history_cells) =
        assign_terminal_public(&mut builder, public, successor_history)?;
    let ctx = builder.main(0);
    let is_send = range.gate().is_equal(
        ctx,
        assigned.operation,
        halo2_base::QuantumCell::Constant(F::from(2)),
    );
    let is_redeem = range.gate().is_equal(
        ctx,
        assigned.operation,
        halo2_base::QuantumCell::Constant(F::from(4)),
    );
    let outgoing = range.gate().or(ctx, is_send, is_redeem);
    range.gate().assert_is_const(ctx, &outgoing, &F::ONE);
    for (actual, expected) in [
        (public_cells[0], assigned.operation),
        (public_cells[1], assigned.successor.protocol_version),
        (public_cells[14], assigned.successor.scale),
        (public_cells[19], assigned.successor.policy_epoch),
        (public_cells[36], assigned.amount),
    ] {
        ctx.constrain_equal(&actual, &expected);
    }
    for (offset, expected) in [
        (2, assigned.successor.suite_id),
        (4, assigned.successor.vk_digest),
        (6, assigned.successor.release_id),
        (8, assigned.successor.network_id),
        (10, assigned.successor.asset_id),
        (12, assigned.successor.asset_incarnation),
        (15, assigned.successor.liability_pool_id),
        (17, assigned.successor.hardware_profile_id),
        (20, assigned.lifecycle_binding_digest),
    ] {
        bind_limbs(
            ctx,
            [public_cells[offset], public_cells[offset + 1]],
            expected,
        );
    }
    let public_digest = |ctx: &mut Context<F>, offset: usize| {
        limbs_bytes(
            ctx,
            &range,
            [public_cells[offset], public_cells[offset + 1]],
        )
    };
    let expected_candidate = public_digest(ctx, 24);
    equal_bytes(ctx, &range, candidate, expected_candidate);
    let prep_clock = clock_cells(ctx, &range, source.preparation_clock)?;
    let before = limbs_bytes(ctx, &range, assigned.predecessor_outer);
    let after = limbs_bytes(ctx, &range, assigned.successor_outer);
    let request_expected = public_digest(ctx, 30);
    let receiver_expected = public_digest(ctx, 32);
    let release = limbs_bytes(ctx, &range, assigned.successor.release_id);
    let network = limbs_bytes(ctx, &range, assigned.successor.network_id);
    let asset = limbs_bytes(ctx, &range, assigned.successor.asset_id);
    let incarnation = limbs_bytes(ctx, &range, assigned.successor.asset_incarnation);
    let pool = limbs_bytes(ctx, &range, assigned.successor.liability_pool_id);
    let receiver = constrain_ordinary_receiver_request_opening_v1(
        &mut builder,
        &mut jobs,
        source.issuer_table,
        OrdinaryReceiverRequestSourcesV1 {
            operation: assigned.operation,
            release,
            network,
            normalized_asset: asset,
            incarnation,
            scale: assigned.successor.scale,
            reserve_pool: pool,
            amount: assigned.amount,
            preparation_clock: &prep_clock,
            expected_request_digest: request_expected,
            expected_recipient_credential_digest: receiver_expected,
        },
        OrdinaryReceiverRequestWitnessV1 {
            request: receiver_witness.request,
            credential: receiver_witness.credential,
            integrity_lease: receiver_witness.integrity_lease,
            previous_app_attest_counter: receiver_witness.previous_app_attest_counter,
            enabled: receiver_witness.enabled,
        },
    )?;
    let ctx = builder.main(0);
    let expected_key = limbs_bytes(ctx, &range, assigned.recipient_encryption_key_binding);
    equal_bytes(ctx, &range, receiver.encryption_key, expected_key);
    let mut raw = vec![0; KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1];
    raw[..encrypted_credit.len()].copy_from_slice(encrypted_credit);
    let encrypted = assign_bytes(ctx, &range, &raw);
    let encrypted_length = ctx.load_witness(F::from(encrypted_credit.len() as u64));
    let encrypted =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, encrypted, encrypted_length)?;
    let sender_lane = limbs_bytes(ctx, &range, assigned.successor.lane_id);
    let epoch = limbs_bytes(ctx, &range, assigned.predecessor.epoch_id);
    let credit = limbs_bytes(ctx, &range, assigned.peer_credit_id);
    let output_expected = bytes(ctx, &range, source.record.body.send_output_digest);
    let encrypted_expected = bytes(ctx, &range, source.record.body.encrypted_credit_digest);
    let nullifier_expected = public_digest(ctx, 28);
    let ciphertext_expected = public_digest(ctx, 34);
    let output = constrain_ordinary_send_output_opening_v1(
        ctx,
        &range,
        &mut jobs,
        OrdinarySendOutputSourcesV1 {
            operation: assigned.operation,
            amount: assigned.amount,
            before,
            after,
            predecessor_secure_index: assigned.predecessor.secure_index,
            predecessor_epoch: epoch,
            network,
            sender_lane,
            reserve_pool: pool,
            receiver: &receiver,
            selected_credit_id: credit,
            encrypted_credit: &encrypted,
            preparation_clock: &prep_clock,
            preparation_clock_specimen: source.preparation_clock,
            expected_output_digest: output_expected,
            expected_encrypted_digest: encrypted_expected,
            expected_nullifier: nullifier_expected,
            expected_ciphertext_commitment: ciphertext_expected,
        },
        output_witness,
        credit_opening,
    )?;
    let ctx = builder.main(0);
    let lifecycle = limbs_bytes(ctx, &range, assigned.lifecycle_binding_digest);
    let manifest_expected = public_digest(ctx, 39);
    let semantic_expected = limbs_bytes(
        ctx,
        &range,
        [
            state_semantic[state_slot::TRANSPORT_LO],
            state_semantic[state_slot::TRANSPORT_HI],
        ],
    );
    let redemption = constrain_ordinary_redeem_output_opening_v1(
        ctx,
        &range,
        &mut jobs,
        OrdinaryRedeemOutputSourcesV1 {
            operation: assigned.operation,
            amount: assigned.amount,
            release,
            network,
            asset,
            incarnation,
            scale: assigned.successor.scale,
            pool,
            beneficiary_account_binding: preparation.account_binding,
            before,
            after,
            nullifier: output.transition_nullifier,
            lifecycle,
            preparation_clock: &prep_clock,
            preparation_clock_specimen: source.preparation_clock,
            expected_manifest_digest: manifest_expected,
            expected_semantic_digest: semantic_expected,
        },
        redemption_witness,
    )?;
    let terminal_guard =
        constrain_guard_bundle_semantics_v1(&mut builder, &mut jobs, source.terminal_relation)?;
    let terminal_digest = digest_limbs_assigned(builder.main(0), &terminal_guard.guard_digest);
    let mut terminal_state = assigned;
    terminal_state.guard_digest = terminal_digest;
    constrain_state_guard_binding_v1(&mut builder, &terminal_state, &terminal_guard)?;
    let terminal = constrain_ordinary_guard_data_binding_v1(
        &mut builder,
        &mut jobs,
        &terminal_guard,
        source.sender_credential,
        source.terminal_approval,
        source.terminal_integrity_lease,
    )?;
    let ctx = builder.main(0);
    range
        .gate()
        .assert_is_const(ctx, &preparation.approval_purpose, &F::from(2));
    range
        .gate()
        .assert_is_const(ctx, &terminal.approval_purpose, &F::ONE);
    // A separate W1 owns a separate nonce and operation; a prepared approval never upgrades.
    let mut same_nonce = ctx.load_constant(F::ONE);
    for (a, b) in preparation
        .approval_nonce
        .into_iter()
        .zip(terminal.approval_nonce)
    {
        let equal = range
            .gate()
            .is_equal(ctx, a.quantum_cell(), b.quantum_cell());
        same_nonce = range.gate().and(ctx, same_nonce, equal);
    }
    range.gate().assert_is_const(ctx, &same_nonce, &F::ZERO);
    let same_op = {
        let mut value = ctx.load_constant(F::ONE);
        for (a, b) in preparation
            .approval_operation_id
            .into_iter()
            .zip(terminal.approval_operation_id)
        {
            let equal = range
                .gate()
                .is_equal(ctx, a.quantum_cell(), b.quantum_cell());
            value = range.gate().and(ctx, value, equal);
        }
        value
    };
    range.gate().assert_is_const(ctx, &same_op, &F::ZERO);
    equal_bytes(ctx, &range, preparation.digests[1], terminal.digests[1]);
    let preparation_id = limbs_bytes(ctx, &range, [prepared[0], prepared[1]]);
    let semantic = limbs_bytes(
        ctx,
        &range,
        [
            state_semantic[state_slot::TRANSPORT_LO],
            state_semantic[state_slot::TRANSPORT_HI],
        ],
    );
    let lifecycle = limbs_bytes(ctx, &range, assigned.lifecycle_binding_digest);
    let reservation = bytes(ctx, &range, source.prepared.record.reservation_digest);
    // Copy the independently opened receiver request into the exact pre-W2 prepared original.
    let request_in_prepared = bytes(ctx, &range, source.prepared.record.request_digest);
    equal_bytes(ctx, &range, receiver.request_digest, request_in_prepared);
    let manifest_in_prepared = bytes(ctx, &range, source.prepared.record.artifact_manifest_digest);
    equal_bytes(
        ctx,
        &range,
        manifest_in_prepared,
        redemption.manifest_digest,
    );
    let stream_digests = [
        limbs_bytes(ctx, &range, [prepared[2], prepared[3]]),
        limbs_bytes(ctx, &range, [prepared[4], prepared[5]]),
    ];
    let body_clock = clock_cells(ctx, &range, &source.record.body.clock_context)?;
    let admission_clock = clock_cells(ctx, &range, &source.record.admission_clock_context)?;
    let native_operation_id = bytes(ctx, &range, source.intent.native_operation_id);
    let native_nonce = bytes(ctx, &range, source.intent.native_nonce);
    let prefix = bytes(
        ctx,
        &range,
        source.intent.predecessor_descriptor_prefix_digest,
    );
    let opening = constrain_ordinary_cash_terminal_opening_v1(
        ctx,
        &range,
        &mut jobs,
        &OrdinaryCashTerminalSourcesV1 {
            operation: assigned.operation,
            amount: assigned.amount,
            state_statement_digest: transition,
            candidate_digest: candidate,
            preparation_id,
            prepared_projection_semantic_digest: semantic,
            lifecycle_digest: lifecycle,
            request_digest: receiver.request_digest,
            recipient_credential_digest: receiver.credential_digest,
            send_output_digest: output.output_digest,
            encrypted_credit_digest: output.encrypted_digest,
            artifact_manifest_digest: redemption.manifest_digest,
            reservation_digest: reservation,
            native_operation_id,
            native_nonce,
            predecessor_descriptor_prefix_digest: prefix,
            stream_lengths: lengths,
            stream_digests,
            secure_index_before: assigned.predecessor.secure_index,
            secure_index_after: assigned.successor.secure_index,
            logical_journal_sequence_before: assigned.journal_revision_before,
            logical_journal_sequence_after: assigned.journal_revision_after,
            body_clock,
            admission_clock,
        },
        &preparation,
        &terminal,
        source.intent,
        source.record,
    )?;
    let expected_body = public_digest(ctx, 22);
    equal_bytes(ctx, &range, opening.body_digest, expected_body);
    let expected_record = public_digest(ctx, 26);
    equal_bytes(ctx, &range, opening.record_digest, expected_record);
    // Open the actual same full S1 and all independent secure indexes. Financial logical
    // sequence is already joined in the shared Guard/State relation and is never narrowed.
    for (slot, expected) in [
        (S::TRANSITION_STATEMENT_DIGEST, transition),
        (S::CANDIDATE_ENVELOPE_DIGEST, candidate),
        (S::TERMINAL_BODY_COMMITMENT, opening.body_digest),
    ] {
        for (actual, expected) in terminal.canonical_subject[slot].iter().zip(expected) {
            ctx.constrain_equal(
                actual,
                &expected.assigned().ok_or("Terminal S1 SHA byte absent")?,
            );
        }
    }
    for (slot, index) in [
        (S::SECURE_INDEX_BEFORE, assigned.predecessor.secure_index),
        (S::SECURE_INDEX_AFTER, assigned.successor.secure_index),
    ] {
        let expected = assigned_uint_bytes_v1(ctx, range.gate(), index, 128);
        for (actual, expected) in terminal.canonical_subject[slot].iter().zip(expected) {
            ctx.constrain_equal(
                actual,
                &expected.assigned().ok_or("Terminal index byte absent")?,
            );
        }
    }
    let mut commit = constant_bytes(KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1);
    for digest in [opening.body_digest, candidate, transition, reservation] {
        commit.extend(digest);
    }
    let commit = hash(ctx, &mut jobs, commit)?;
    let commit_limbs = digest_limbs_assigned(ctx, &commit);
    bind_limbs(
        ctx,
        commit_limbs,
        terminal_guard.terminal_commit_binding_digest,
    );
    let body_limbs = digest_limbs_assigned(ctx, &opening.body_digest);
    bind_limbs(ctx, body_limbs, terminal_guard.transition_intent);
    let intent_limbs = digest_limbs_assigned(ctx, &opening.intent_digest);
    bind_limbs(ctx, intent_limbs, terminal_guard.recovery_record);
    let preparation_authorization = preparation.digests[2].map(|b| {
        let value = range.gate().mul(ctx, b.quantum_cell(), is_send);
        PastaSha256ByteV1::range_checked(ctx, &range, value)
    });
    let preparation_authorization_limbs = digest_limbs_assigned(ctx, &preparation_authorization);
    bind_limbs(
        ctx,
        preparation_authorization_limbs,
        terminal_guard.sender_one_time_authorization_digest,
    );
    let mut final_output = constant_bytes(KAGEMUSHA_ORDINARY_OUTPUT_BINDING_DOMAIN_V1);
    for digest in [semantic, candidate, opening.record_digest] {
        final_output.extend(digest);
    }
    let final_output = hash(ctx, &mut jobs, final_output)?;
    let expected_output = public_digest(ctx, 37);
    equal_bytes(ctx, &range, final_output, expected_output);
    // The candidate's actual parity protocol identity is a mandatory loaded-key constant.
    // The actual recursive verifier below pins this cell to its original protocol as well.
    let parity_offset = match parity {
        KagemushaPastaParityV1::Eq => state_slot::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_slot::EP_PROTOCOL_LO,
    };
    range.range_check(ctx, state_semantic[parity_offset], 128);
    range.range_check(ctx, state_semantic[parity_offset + 1], 128);
    builder.assigned_instances = vec![public_cells.iter().chain(&history_cells).copied().collect()];
    Ok(OrdinaryCashTerminalSemanticAssignmentV1 {
        builder,
        jobs,
        public: public_cells,
        history: history_cells,
        candidate_instances: vec![candidate_column],
        preparation,
        terminal,
    })
}

/// Consume the genuine current candidate, both exact-original Guards and the whole ordered
/// typed SHA claim. Every history participates in the exposed accumulator and opposite audit.
pub(in super::super) fn build_ordinary_cash_terminal_scalar_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    public: &OrdinaryCashTerminalPublicV1,
    source: &OrdinaryCashTerminalSemanticWitnessV1<'_>,
    witness: OrdinaryCashTerminalHalfWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        KagemushaDeferredParentOutputV1<C>,
        Vec<AssignedValue<C::ScalarExt>>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if witness.candidate.protocol.num_instance
        != [state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT + 34]
    {
        return Err("ordinary Terminal candidate protocol shape differs".into());
    }
    let assignment = assign_ordinary_cash_terminal_semantics_v1(
        parity,
        public,
        source,
        witness.candidate.instances,
        witness.successor_history,
    )?;
    let OrdinaryCashTerminalSemanticAssignmentV1 {
        mut builder,
        jobs,
        public: public_cells,
        history,
        candidate_instances,
        preparation,
        terminal,
    } = assignment;
    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let candidate_instances = candidate_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .copied()
                .map(|cell| loader.scalar_from_assigned(cell))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let column = &candidate_instances[0];
    let protocol_offset = match parity {
        KagemushaPastaParityV1::Eq => state_slot::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_slot::EP_PROTOCOL_LO,
    };
    let protocol_digest = native_parent_protocol_digest_v1(witness.candidate.protocol, parity)?;
    for (actual, expected) in column[protocol_offset..protocol_offset + 2]
        .iter()
        .zip(digest_limbs::<C::ScalarExt>(protocol_digest))
    {
        let expected = loader.ctx_mut().main().load_constant(expected);
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&actual.assigned(), &expected);
    }
    let start = loader.ecc_chip().equation_count();
    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &witness.candidate.protocol.loaded(&loader),
        &candidate_instances,
        witness.candidate.proof,
    )
    .map_err(|e| format!("ordinary Terminal genuine candidate proof: {e:?}"))?;
    let prior = load_native_accumulator(&loader, witness.candidate.history)
        .map_err(|e| format!("ordinary Terminal candidate history: {e:?}"))?;
    let prior_limbs = column[state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT..]
        .iter()
        .map(|v| *v.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &prior, &prior_limbs)
        .map_err(|e| format!("ordinary Terminal exact candidate history: {e:?}"))?;
    let complete_candidate = verify_fold(
        &loader,
        succinct_vk,
        &[current, prior],
        witness.candidate.history_fold_proof,
    )
    .map_err(|e| format!("ordinary Terminal complete candidate fold: {e:?}"))?;
    let candidate_end = loader.ecc_chip().equation_count();
    if start != 0 || candidate_end <= start {
        return Err("ordinary Terminal candidate emitted no complete equation interval".into());
    }
    let guard_offset = match parity {
        KagemushaPastaParityV1::Eq => state_slot::GUARD_EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_slot::GUARD_EP_PROTOCOL_LO,
    };
    let expected_guard = [
        *column[guard_offset].assigned(),
        *column[guard_offset + 1].assigned(),
    ];
    let (complete_preparation, preparation_span) = constrain_ordinary_guard_complete_v1(
        &loader,
        &range,
        succinct_vk,
        parity,
        expected_guard,
        KagemushaOrdinaryGuardCommitmentCellsV1::from_binding(&preparation),
        witness.preparation_guard,
    )?;
    let (complete_terminal, terminal_span) = constrain_ordinary_guard_complete_v1(
        &loader,
        &range,
        succinct_vk,
        parity,
        expected_guard,
        KagemushaOrdinaryGuardCommitmentCellsV1::from_binding(&terminal),
        witness.terminal_guard,
    )?;
    if preparation_span.start != candidate_end || terminal_span.start != preparation_span.end {
        return Err("ordinary Terminal dual Guard equation intervals are detached".into());
    }
    let candidate_with_preparation = verify_fold(
        &loader,
        succinct_vk,
        &[complete_candidate, complete_preparation],
        witness.preparation_merge_fold_proof,
    )
    .map_err(|e| format!("ordinary Terminal candidate/W2 history merge: {e:?}"))?;
    let preparation_merge_end = loader.ecc_chip().equation_count();
    if preparation_merge_end <= terminal_span.end {
        return Err("ordinary Terminal candidate/W2 merge emitted no equations".into());
    }
    let complete = verify_fold(
        &loader,
        succinct_vk,
        &[candidate_with_preparation, complete_terminal],
        witness.terminal_merge_fold_proof,
    )
    .map_err(|e| format!("ordinary Terminal complete W1 history merge: {e:?}"))?;
    let merge_end = loader.ecc_chip().equation_count();
    if merge_end <= preparation_merge_end {
        return Err("ordinary Terminal W1 history merge emitted no equations".into());
    }
    let (complete, claim_tail, claim_current_end) = constrain_recursive_hash_claim_v1(
        &loader,
        succinct_vk,
        parity,
        witness.hash_claim,
        &jobs,
        [public_cells[6], public_cells[7]],
        complete,
    )?;
    bind_accumulator_limbs(&loader, &complete, &history)
        .map_err(|e| format!("ordinary Terminal exposed complete history: {e:?}"))?;
    let claim_end = loader.ecc_chip().equation_count();
    if claim_tail.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
        || claim_current_end <= merge_end
        || claim_end <= claim_current_end
    {
        return Err("ordinary Terminal SHA claim current/history/merge is incomplete".into());
    }
    // Only the exact queue consumed above can be released. There is no unconsumed SHA circuit
    // or accepting host verifier standing in for the original bytes/signature equations.
    drop(jobs);
    let spans = [
        (0..candidate_end, CANDIDATE_TAG),
        (preparation_span, PREPARATION_GUARD_TAG),
        (terminal_span.clone(), TERMINAL_GUARD_TAG),
        (
            terminal_span.end..preparation_merge_end,
            PREPARATION_MERGE_TAG,
        ),
        (preparation_merge_end..merge_end, TERMINAL_MERGE_TAG),
        (merge_end..claim_current_end, SHA_CURRENT_TAG),
        (claim_current_end..claim_end, SHA_HISTORY_TAG),
    ];
    let mut tags = Vec::with_capacity(claim_end);
    for (span, tag) in spans {
        tags.extend(std::iter::repeat_n(tag, span.len()));
    }
    if tags.len() != claim_end {
        return Err("ordinary Terminal deferred audit omits an equation".into());
    }
    let enabled = loader.ctx_mut().main().load_constant(C::ScalarExt::ONE);
    let output = finalize_deferred_audit_plan_with_u128_binding_v1(
        &mut builder,
        loader,
        tags,
        vec![enabled; claim_end],
        vec![true; claim_end],
        &claim_tail,
    )
    .map_err(|e| format!("ordinary Terminal full scalar audit: {e:?}"))?;
    let offset = match parity {
        KagemushaPastaParityV1::Eq => AUDIT_EQ,
        KagemushaPastaParityV1::Ep => AUDIT_EP,
    };
    for (actual, expected) in output
        .audit_digest_limbs
        .iter()
        .zip(&public_cells[offset..offset + 2])
    {
        builder.main(0).constrain_equal(actual, expected);
    }
    if builder.assigned_instances[0].len() != ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1 {
        return Err("ordinary Terminal final public shape differs".into());
    }
    Ok((builder, output, claim_tail))
}
