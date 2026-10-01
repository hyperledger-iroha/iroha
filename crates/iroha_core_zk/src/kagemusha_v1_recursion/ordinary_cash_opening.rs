//! Complete shared-model ordinary terminal intent/body/record SHA openings.
//!
//! The caller supplies only the actual assigned State/candidate/Guard/output and the original
//! Native-selected clock/prefix/reservation cells. These mathematical cells are data; shipping
//! proving requires the closed Native cash selection loan, genuine candidate/dual Guards and
//! complete typed SHA claim with all histories and reciprocal audits.

use super::{
    composite::assigned_uint_bytes_v1,
    guard_bundle::{constant_bytes, hash},
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1, KAGEMUSHA_ORDINARY_CASH_TERMINAL_BODY_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_CASH_TERMINAL_INTENT_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_CASH_TERMINAL_RECORD_DOMAIN_V1, KAGEMUSHA_ORDINARY_PAYMENT_BODY_DOMAIN_V1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryCashTerminalIntentV1,
    KagemushaOrdinaryCashTerminalRecordV1, KagemushaOrdinaryCashTranscriptV1,
};
type Bytes<F> = [PastaSha256ByteV1<F>; 32];

/// Selected conservative interval cells, separately lent from the authentic Native clock owner.
/// They are not decoded from a caller record and do not authenticate BLS signatures themselves.
pub(super) struct OrdinaryCashClockCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) nonce: Bytes<F>,
    pub(super) signed_observations_original_digest: Bytes<F>,
    pub(super) lower_at_ms: AssignedValue<F>,
    pub(super) upper_at_ms: AssignedValue<F>,
}
/// Actual sources, never a fresh set of values decoded from a carried intent/body hash.
pub(super) struct OrdinaryCashTerminalSourcesV1<F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) state_statement_digest: Bytes<F>,
    pub(super) candidate_digest: Bytes<F>,
    pub(super) preparation_id: Bytes<F>,
    pub(super) prepared_projection_semantic_digest: Bytes<F>,
    pub(super) lifecycle_digest: Bytes<F>,
    pub(super) request_digest: Bytes<F>,
    /// Complete original receiver C digest from the independently admitted receiver request.
    pub(super) recipient_credential_digest: Bytes<F>,
    pub(super) send_output_digest: Bytes<F>,
    pub(super) encrypted_credit_digest: Bytes<F>,
    pub(super) artifact_manifest_digest: Bytes<F>,
    pub(super) reservation_digest: Bytes<F>,
    pub(super) native_operation_id: Bytes<F>,
    pub(super) native_nonce: Bytes<F>,
    pub(super) predecessor_descriptor_prefix_digest: Bytes<F>,
    pub(super) stream_lengths: [AssignedValue<F>; 2],
    pub(super) stream_digests: [Bytes<F>; 2],
    pub(super) secure_index_before: AssignedValue<F>,
    pub(super) secure_index_after: AssignedValue<F>,
    pub(super) logical_journal_sequence_before: AssignedValue<F>,
    pub(super) logical_journal_sequence_after: AssignedValue<F>,
    pub(super) body_clock: OrdinaryCashClockCellsV1<F>,
    pub(super) admission_clock: OrdinaryCashClockCellsV1<F>,
}
/// Complete preimage outputs. The actual SHA claim must authenticate every queued digest.
pub(super) struct OrdinaryCashTerminalOpeningV1<F: KagemushaPoseidonFieldV1> {
    pub(super) intent_digest: Bytes<F>,
    pub(super) body_digest: Bytes<F>,
    pub(super) record_digest: Bytes<F>,
}
fn nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: Bytes<F>,
) -> Result<(), String> {
    let mut sum = ctx.load_zero();
    for byte in value {
        let byte = byte
            .assigned()
            .ok_or("ordinary terminal assigned byte absent")?;
        range.range_check(ctx, byte, 8);
        sum = range.gate().add(ctx, sum, byte);
    }
    let zero = range.gate().is_zero(ctx, sum);
    range.gate().assert_is_const(ctx, &zero, &F::ZERO);
    Ok(())
}
fn selected_slot<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: Bytes<F>,
    present: AssignedValue<F>,
) -> Result<(), String> {
    let mut sum = ctx.load_zero();
    for byte in value {
        let byte = byte
            .assigned()
            .ok_or("ordinary selected slot byte absent")?;
        range.range_check(ctx, byte, 8);
        sum = range.gate().add(ctx, sum, byte);
    }
    let zero = range.gate().is_zero(ctx, sum);
    let actual = range.gate().not(ctx, zero);
    ctx.constrain_equal(&actual, &present);
    Ok(())
}
/// Fill the maintained model transcript's semantic ranges with actual assigned sources.
/// All model bytes outside those exact ranges, including its domain/version framing, are fixed.
/// No raw mathematical message or offset supplied by a caller is accepted.
fn fill_transcript<F: KagemushaPoseidonFieldV1>(
    template: KagemushaOrdinaryCashTranscriptV1,
    domain: &[u8],
    parts: Vec<(&'static str, Vec<PastaSha256ByteV1<F>>)>,
) -> Result<(Vec<PastaSha256ByteV1<F>>, Vec<PastaSha256ByteV1<F>>), String> {
    if !template.bytes.starts_with(domain) || template.fields.len() != parts.len() {
        return Err("ordinary model transcript/domain/field count differs".into());
    }
    let mut full = constant_bytes(&template.bytes);
    let mut covered = vec![false; full.len()];
    for ((name, value), field) in parts.into_iter().zip(&template.fields) {
        if name != field.name
            || field.range.len() != value.len()
            || field.range.start < domain.len()
            || field.range.end > full.len()
        {
            return Err("ordinary model transcript field identity/width differs".into());
        }
        for (index, byte) in field.range.clone().zip(value) {
            if covered[index] {
                return Err("ordinary model transcript semantic fields overlap".into());
            }
            covered[index] = true;
            full[index] = byte;
        }
    }
    if covered[domain.len()..].iter().any(|covered| !covered) {
        return Err("ordinary model transcript semantic payload has a gap".into());
    }
    let payload = full[domain.len()..].to_vec();
    Ok((full, payload))
}
fn clock_payload<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    clock: &OrdinaryCashClockCellsV1<F>,
    specimen: &KagemushaOrdinaryCashClockContextV1,
) -> Result<Vec<PastaSha256ByteV1<F>>, String> {
    nonzero(ctx, range, clock.nonce)?;
    nonzero(ctx, range, clock.signed_observations_original_digest)?;
    range.range_check(ctx, clock.lower_at_ms, 64);
    range.range_check(ctx, clock.upper_at_ms, 64);
    let lower_zero = range.gate().is_zero(ctx, clock.lower_at_ms);
    range.gate().assert_is_const(ctx, &lower_zero, &F::ZERO);
    let reversed = range.is_less_than(ctx, clock.upper_at_ms, clock.lower_at_ms, 64);
    range.gate().assert_is_const(ctx, &reversed, &F::ZERO);
    let lower = assigned_uint_bytes_v1(ctx, range.gate(), clock.lower_at_ms, 64);
    let upper = assigned_uint_bytes_v1(ctx, range.gate(), clock.upper_at_ms, 64);
    let (_, payload) = fill_transcript(
        specimen.binding_transcript(),
        KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("request_nonce", clock.nonce.to_vec()),
            (
                "signed_observations_original_digest",
                clock.signed_observations_original_digest.to_vec(),
            ),
            ("lower_at_ms", lower),
            ("upper_at_ms", upper),
        ],
    )?;
    Ok(payload)
}
fn constrain_clock_window<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    clock: &OrdinaryCashClockCellsV1<F>,
    issued: AssignedValue<F>,
    expires: AssignedValue<F>,
) {
    let before = range.is_less_than(ctx, clock.lower_at_ms, issued, 64);
    range.gate().assert_is_const(ctx, &before, &F::ZERO);
    range.check_less_than(ctx, clock.upper_at_ms, expires, 64);
}

/// Open intent/body/record from actual candidate, dual Guard and Native selection sources.
/// No captured Bootstrap object enters this function. Neither an opening nor native-looking
/// cells supply a proof/custody grant; all enclosing recursive verification is mandatory.
pub(super) fn constrain_ordinary_cash_terminal_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: &OrdinaryCashTerminalSourcesV1<F>,
    preparation: &KagemushaOrdinaryGuardDataBindingV1<F>,
    terminal: &KagemushaOrdinaryGuardDataBindingV1<F>,
    native_intent: &KagemushaOrdinaryCashTerminalIntentV1,
    native_record: &KagemushaOrdinaryCashTerminalRecordV1,
) -> Result<OrdinaryCashTerminalOpeningV1<F>, String> {
    // These host checks select no circuit topology and create no authority. Both operations
    // use the same complete fixed field lists, clock arithmetic and queued SHA construction.
    native_intent.validate_shape()?;
    native_record.validate_shape()?;
    let gate = range.gate();
    gate.assert_is_const(ctx, &preparation.approval_purpose, &F::from(2));
    gate.assert_is_const(ctx, &terminal.approval_purpose, &F::ONE);
    range.range_check(ctx, sources.operation, 8);
    let send = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    let redeem = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(4)));
    let outgoing = gate.or(ctx, send, redeem);
    gate.assert_is_const(ctx, &outgoing, &F::ONE);
    range.range_check(ctx, sources.amount, 128);
    let amount_zero = gate.is_zero(ctx, sources.amount);
    gate.assert_is_const(ctx, &amount_zero, &F::ZERO);
    for value in [
        sources.state_statement_digest,
        sources.candidate_digest,
        sources.preparation_id,
        sources.prepared_projection_semantic_digest,
        sources.lifecycle_digest,
        sources.reservation_digest,
        sources.native_operation_id,
        sources.native_nonce,
        sources.predecessor_descriptor_prefix_digest,
    ] {
        nonzero(ctx, range, value)?;
    }
    for value in [
        sources.request_digest,
        sources.recipient_credential_digest,
        sources.send_output_digest,
        sources.encrypted_credit_digest,
    ] {
        selected_slot(ctx, range, value, send)?;
    }
    selected_slot(ctx, range, sources.artifact_manifest_digest, redeem)?;
    // Both branches queue this same complete job. Send uses its derived actual output/cipher
    // binding; Redeem requires zero Send slots and its separately opened redemption semantic.
    let mut payment_body = constant_bytes(KAGEMUSHA_ORDINARY_PAYMENT_BODY_DOMAIN_V1);
    payment_body.extend(sources.send_output_digest);
    payment_body.extend(sources.encrypted_credit_digest);
    let payment_body = hash(ctx, jobs, payment_body)?;
    for (actual, expected) in payment_body
        .into_iter()
        .zip(sources.prepared_projection_semantic_digest)
    {
        let difference = gate.sub(
            ctx,
            actual
                .assigned()
                .ok_or("derived payment body byte absent")?,
            expected.assigned().ok_or("prepared semantic byte absent")?,
        );
        let selected = gate.mul(ctx, difference, send);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }

    // Same original sender C in both phases; each full Guard separately proves it under its issuer.
    for (left, right) in preparation.digests[1].into_iter().zip(terminal.digests[1]) {
        ctx.constrain_equal(
            &left.assigned().ok_or("purpose2 C byte absent")?,
            &right.assigned().ok_or("purpose1 C byte absent")?,
        );
    }
    for (original, actual) in terminal
        .approval_operation_id
        .into_iter()
        .zip(sources.native_operation_id)
    {
        ctx.constrain_equal(
            &original.assigned().ok_or("W1 opID absent")?,
            &actual.assigned().ok_or("native opID absent")?,
        );
    }
    for (original, actual) in terminal
        .approval_nonce
        .into_iter()
        .zip(sources.native_nonce)
    {
        ctx.constrain_equal(
            &original.assigned().ok_or("W1 nonce absent")?,
            &actual.assigned().ok_or("native nonce absent")?,
        );
    }
    let secure_before = assigned_uint_bytes_v1(ctx, gate, sources.secure_index_before, 128);
    let secure_after = assigned_uint_bytes_v1(ctx, gate, sources.secure_index_after, 128);
    let journal_before =
        assigned_uint_bytes_v1(ctx, gate, sources.logical_journal_sequence_before, 64);
    let journal_after =
        assigned_uint_bytes_v1(ctx, gate, sources.logical_journal_sequence_after, 64);
    let secure_expected = gate.add(
        ctx,
        sources.secure_index_before,
        QuantumCell::Constant(F::ONE),
    );
    ctx.constrain_equal(&secure_expected, &sources.secure_index_after);
    let journal_expected = gate.add(
        ctx,
        sources.logical_journal_sequence_before,
        QuantumCell::Constant(F::ONE),
    );
    ctx.constrain_equal(&journal_expected, &sources.logical_journal_sequence_after);
    let issued = terminal.approval_issued_at_ms;
    let expires = terminal.approval_expires_at_ms;
    range.range_check(ctx, issued, 64);
    range.range_check(ctx, expires, 64);
    range.check_less_than(ctx, issued, expires, 64);
    let lifetime = gate.sub(ctx, expires, issued);
    range.check_less_than(
        ctx,
        lifetime,
        QuantumCell::Constant(F::from(
            iroha_data_model::kagemusha::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1 + 1,
        )),
        64,
    );
    let issued_bytes = assigned_uint_bytes_v1(ctx, gate, issued, 64);
    let expires_bytes = assigned_uint_bytes_v1(ctx, gate, expires, 64);
    let operation = assigned_uint_bytes_v1(ctx, gate, sources.operation, 8);
    let amount = assigned_uint_bytes_v1(ctx, gate, sources.amount, 128);
    let (intent, _) = fill_transcript(
        native_intent.binding_transcript(),
        KAGEMUSHA_ORDINARY_CASH_TERMINAL_INTENT_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("operation", operation.clone()),
            ("native_operation_id", sources.native_operation_id.to_vec()),
            ("native_nonce", sources.native_nonce.to_vec()),
            ("preparation_id", sources.preparation_id.to_vec()),
            ("candidate_digest", sources.candidate_digest.to_vec()),
            (
                "state_statement_digest",
                sources.state_statement_digest.to_vec(),
            ),
            (
                "predecessor_descriptor_prefix_digest",
                sources.predecessor_descriptor_prefix_digest.to_vec(),
            ),
            ("sender_credential_digest", terminal.digests[1].to_vec()),
            ("reservation_digest", sources.reservation_digest.to_vec()),
            ("secure_index_before", secure_before.clone()),
            ("secure_index_after", secure_after.clone()),
            ("logical_journal_sequence_before", journal_before.clone()),
            ("logical_journal_sequence_after", journal_after.clone()),
            ("issued_at_ms", issued_bytes.clone()),
            ("expires_at_ms", expires_bytes.clone()),
        ],
    )?;
    let intent_digest = hash(ctx, jobs, intent)?;
    let body_clock = clock_payload(
        ctx,
        range,
        &sources.body_clock,
        &native_record.body.clock_context,
    )?;
    let admission_clock = clock_payload(
        ctx,
        range,
        &sources.admission_clock,
        &native_record.admission_clock_context,
    )?;
    constrain_clock_window(ctx, range, &sources.body_clock, issued, expires);
    constrain_clock_window(ctx, range, &sources.admission_clock, issued, expires);
    for (before, after) in [
        (
            sources.body_clock.lower_at_ms,
            sources.admission_clock.lower_at_ms,
        ),
        (
            sources.body_clock.upper_at_ms,
            sources.admission_clock.upper_at_ms,
        ),
    ] {
        let reversed = range.is_less_than(ctx, after, before, 64);
        gate.assert_is_const(ctx, &reversed, &F::ZERO);
    }
    let mut streams = Vec::with_capacity(4);
    for index in 0..2 {
        range.range_check(ctx, sources.stream_lengths[index], 64);
        let zero = gate.is_zero(ctx, sources.stream_lengths[index]);
        gate.assert_is_const(ctx, &zero, &F::ZERO);
        let maximum = if index == 0 {
            iroha_data_model::kagemusha::KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1
        } else {
            iroha_data_model::kagemusha::KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1
        };
        range.check_less_than(
            ctx,
            sources.stream_lengths[index],
            QuantumCell::Constant(F::from(maximum as u64 + 1)),
            64,
        );
        nonzero(ctx, range, sources.stream_digests[index])?;
        streams.push(assigned_uint_bytes_v1(
            ctx,
            gate,
            sources.stream_lengths[index],
            64,
        ));
        streams.push(sources.stream_digests[index].to_vec());
    }
    let (body, body_payload) = fill_transcript(
        native_record.body.binding_transcript(),
        KAGEMUSHA_ORDINARY_CASH_TERMINAL_BODY_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("operation", operation),
            ("amount", amount),
            (
                "state_statement_digest",
                sources.state_statement_digest.to_vec(),
            ),
            ("candidate_digest", sources.candidate_digest.to_vec()),
            ("preparation_id", sources.preparation_id.to_vec()),
            (
                "prepared_projection_semantic_digest",
                sources.prepared_projection_semantic_digest.to_vec(),
            ),
            ("lifecycle_digest", sources.lifecycle_digest.to_vec()),
            ("request_digest", sources.request_digest.to_vec()),
            (
                "recipient_credential_digest",
                sources.recipient_credential_digest.to_vec(),
            ),
            ("send_output_digest", sources.send_output_digest.to_vec()),
            (
                "encrypted_credit_digest",
                sources.encrypted_credit_digest.to_vec(),
            ),
            (
                "artifact_manifest_digest",
                sources.artifact_manifest_digest.to_vec(),
            ),
            ("reservation_digest", sources.reservation_digest.to_vec()),
            ("native_operation_id", sources.native_operation_id.to_vec()),
            ("terminal_intent_digest", intent_digest.to_vec()),
            (
                "predecessor_descriptor_prefix_digest",
                sources.predecessor_descriptor_prefix_digest.to_vec(),
            ),
            ("transition_stream_length", streams[0].clone()),
            ("transition_stream_digest", streams[1].clone()),
            ("recovery_stream_length", streams[2].clone()),
            ("recovery_stream_digest", streams[3].clone()),
            ("clock_context", body_clock),
            ("secure_index_before", secure_before),
            ("secure_index_after", secure_after),
            ("logical_journal_sequence_before", journal_before),
            ("logical_journal_sequence_after", journal_after),
        ],
    )?;
    let body_digest = hash(ctx, jobs, body)?;
    let (record, _) = fill_transcript(
        native_record.binding_transcript(),
        KAGEMUSHA_ORDINARY_CASH_TERMINAL_RECORD_DOMAIN_V1,
        vec![
            ("version", constant_bytes(&1_u16.to_le_bytes())),
            ("body", body_payload),
            ("sender_credential_digest", terminal.digests[1].to_vec()),
            (
                "preparation_authorization_digest",
                preparation.digests[2].to_vec(),
            ),
            (
                "terminal_authorization_digest",
                terminal.digests[2].to_vec(),
            ),
            ("terminal_subject_digest", terminal.digests[3].to_vec()),
            ("admission_clock_context", admission_clock),
            ("approval_issued_at_ms", issued_bytes),
            ("approval_expires_at_ms", expires_bytes),
        ],
    )?;
    let record_digest = hash(ctx, jobs, record)?;
    Ok(OrdinaryCashTerminalOpeningV1 {
        intent_digest,
        body_digest,
        record_digest,
    })
}

#[cfg(test)]
mod tests {
    //! Mathematical upstream fixtures exercise the real opening only. No Guard proof, Native
    //! financial/clock owner, signed release, physical device or money admission is fabricated.
    use super::*;
    use crate::{kagemusha_v1_poseidon::from_u128, pasta_sha256::PastaSha256ConfigV1};
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::kagemusha::{
        KagemushaOrdinaryCashTerminalBodyV1, kagemusha_ordinary_payment_body_digest_v1,
    };
    const K: usize = 16;
    const UNUSABLE: usize = 9;
    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = (BaseConfig<F>, PastaSha256ConfigV1);
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;
        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows((1 << K) - UNUSABLE);
            (base, PastaSha256ConfigV1::configure(meta))
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("fixed ordinary opening parameters")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.builder
                .synthesize(config.0, layouter.namespace(|| "cash opening base"))?;
            self.jobs.synthesize(
                &config.1,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1 << K) - UNUSABLE,
            )
        }
    }
    fn d(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn fixtures(
        operation: u8,
    ) -> (
        KagemushaOrdinaryCashTerminalIntentV1,
        KagemushaOrdinaryCashTerminalRecordV1,
    ) {
        let intent = KagemushaOrdinaryCashTerminalIntentV1 {
            version: 1,
            operation,
            native_operation_id: d(1),
            native_nonce: d(2),
            preparation_id: d(3),
            candidate_digest: d(4),
            state_statement_digest: d(5),
            predecessor_descriptor_prefix_digest: d(6),
            sender_credential_digest: d(7),
            reservation_digest: d(8),
            secure_index_before: (1_u128 << 100) + 41,
            secure_index_after: (1_u128 << 100) + 42,
            logical_journal_sequence_before: 10,
            logical_journal_sequence_after: 11,
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };
        let output = if operation == 2 { d(12) } else { [0; 32] };
        let encrypted = if operation == 2 { d(13) } else { [0; 32] };
        let body = KagemushaOrdinaryCashTerminalBodyV1 {
            version: 1,
            operation,
            amount: 17,
            state_statement_digest: intent.state_statement_digest,
            candidate_digest: intent.candidate_digest,
            preparation_id: intent.preparation_id,
            prepared_projection_semantic_digest: if operation == 2 {
                kagemusha_ordinary_payment_body_digest_v1(output, encrypted).unwrap()
            } else {
                d(14)
            },
            lifecycle_digest: d(9),
            request_digest: if operation == 2 { d(10) } else { [0; 32] },
            recipient_credential_digest: if operation == 2 { d(11) } else { [0; 32] },
            send_output_digest: output,
            encrypted_credit_digest: encrypted,
            artifact_manifest_digest: if operation == 4 { d(15) } else { [0; 32] },
            reservation_digest: intent.reservation_digest,
            native_operation_id: intent.native_operation_id,
            terminal_intent_digest: intent.binding_digest().unwrap(),
            predecessor_descriptor_prefix_digest: intent.predecessor_descriptor_prefix_digest,
            stream_lengths: [17, 19],
            stream_digests: [d(16), d(17)],
            clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: d(18),
                signed_observations_original_digest: d(19),
                lower_at_ms: 1001,
                upper_at_ms: 1002,
            },
            secure_index_before: intent.secure_index_before,
            secure_index_after: intent.secure_index_after,
            logical_journal_sequence_before: intent.logical_journal_sequence_before,
            logical_journal_sequence_after: intent.logical_journal_sequence_after,
        };
        let record = KagemushaOrdinaryCashTerminalRecordV1 {
            version: 1,
            body,
            sender_credential_digest: intent.sender_credential_digest,
            preparation_authorization_digest: d(20),
            terminal_authorization_digest: d(21),
            terminal_subject_digest: d(22),
            admission_clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: d(23),
                signed_observations_original_digest: d(24),
                lower_at_ms: 1003,
                upper_at_ms: 1004,
            },
            approval_issued_at_ms: intent.issued_at_ms,
            approval_expires_at_ms: intent.expires_at_ms,
        };
        record.validate_shape().unwrap();
        record.body.validate_against_intent(&intent).unwrap();
        (intent, record)
    }
    fn assigned<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        raw: [u8; 32],
    ) -> Bytes<F> {
        raw.map(|byte| {
            let value = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, range, value)
        })
    }
    fn clock_cells<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        clock: KagemushaOrdinaryCashClockContextV1,
    ) -> OrdinaryCashClockCellsV1<F> {
        OrdinaryCashClockCellsV1 {
            nonce: assigned(ctx, range, clock.request_nonce),
            signed_observations_original_digest: assigned(
                ctx,
                range,
                clock.signed_observations_original_digest,
            ),
            lower_at_ms: ctx.load_witness(F::from(clock.lower_at_ms)),
            upper_at_ms: ctx.load_witness(F::from(clock.upper_at_ms)),
        }
    }
    fn binding<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        phase: u64,
        intent: &KagemushaOrdinaryCashTerminalIntentV1,
        record: &KagemushaOrdinaryCashTerminalRecordV1,
    ) -> KagemushaOrdinaryGuardDataBindingV1<F> {
        let auth = if phase == 2 {
            record.preparation_authorization_digest
        } else {
            record.terminal_authorization_digest
        };
        KagemushaOrdinaryGuardDataBindingV1 {
            digests: [
                d(30),
                record.sender_credential_digest,
                auth,
                record.terminal_subject_digest,
                d(31),
            ]
            .map(|raw| assigned(ctx, range, raw)),
            canonical_subject: core::array::from_fn(|_| ctx.load_witness(F::ZERO)),
            approval_purpose: ctx.load_witness(F::from(phase)),
            approval_operation_id: assigned(ctx, range, intent.native_operation_id),
            approval_nonce: assigned(ctx, range, intent.native_nonce),
            approval_issued_at_ms: ctx.load_witness(F::from(intent.issued_at_ms)),
            approval_expires_at_ms: ctx.load_witness(F::from(intent.expires_at_ms)),
        }
    }
    fn check<F: KagemushaPoseidonFieldV1>(operation: u8, mutation: Option<usize>) -> bool {
        let (intent, record) = fixtures(operation);
        let b = record.body;
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(0);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let mut prep = binding(ctx, &range, 2, &intent, &record);
        let mut term = binding(ctx, &range, 1, &intent, &record);
        let mut sources = OrdinaryCashTerminalSourcesV1 {
            operation: ctx.load_witness(F::from(u64::from(operation))),
            amount: ctx.load_witness(from_u128::<F>(b.amount)),
            state_statement_digest: assigned(ctx, &range, b.state_statement_digest),
            candidate_digest: assigned(ctx, &range, b.candidate_digest),
            preparation_id: assigned(ctx, &range, b.preparation_id),
            prepared_projection_semantic_digest: assigned(
                ctx,
                &range,
                b.prepared_projection_semantic_digest,
            ),
            lifecycle_digest: assigned(ctx, &range, b.lifecycle_digest),
            request_digest: assigned(ctx, &range, b.request_digest),
            recipient_credential_digest: assigned(ctx, &range, b.recipient_credential_digest),
            send_output_digest: assigned(ctx, &range, b.send_output_digest),
            encrypted_credit_digest: assigned(ctx, &range, b.encrypted_credit_digest),
            artifact_manifest_digest: assigned(ctx, &range, b.artifact_manifest_digest),
            reservation_digest: assigned(ctx, &range, b.reservation_digest),
            native_operation_id: assigned(ctx, &range, intent.native_operation_id),
            native_nonce: assigned(ctx, &range, intent.native_nonce),
            predecessor_descriptor_prefix_digest: assigned(
                ctx,
                &range,
                b.predecessor_descriptor_prefix_digest,
            ),
            stream_lengths: b
                .stream_lengths
                .map(|length| ctx.load_witness(F::from(length))),
            stream_digests: b.stream_digests.map(|raw| assigned(ctx, &range, raw)),
            secure_index_before: ctx.load_witness(from_u128::<F>(b.secure_index_before)),
            secure_index_after: ctx.load_witness(from_u128::<F>(b.secure_index_after)),
            logical_journal_sequence_before: ctx
                .load_witness(F::from(b.logical_journal_sequence_before)),
            logical_journal_sequence_after: ctx
                .load_witness(F::from(b.logical_journal_sequence_after)),
            body_clock: clock_cells(ctx, &range, b.clock_context),
            admission_clock: clock_cells(ctx, &range, record.admission_clock_context),
        };
        match mutation {
            Some(0) => sources.native_nonce = assigned(ctx, &range, d(32)),
            Some(1) => sources.native_operation_id = assigned(ctx, &range, d(33)),
            Some(2) => prep.approval_purpose = ctx.load_witness(F::ONE),
            Some(3) => term.approval_purpose = ctx.load_witness(F::from(2)),
            Some(4) => prep.digests[1] = assigned(ctx, &range, d(34)),
            Some(5) => sources.amount = ctx.load_witness(F::ZERO),
            Some(6) => sources.amount = ctx.load_witness(F::from(18)),
            Some(7) => sources.secure_index_before = ctx.load_witness(F::from(41)),
            Some(8) => sources.logical_journal_sequence_after = ctx.load_witness(F::from(12)),
            Some(9) => sources.admission_clock.upper_at_ms = ctx.load_witness(F::from(2000)),
            Some(10) => sources.body_clock.lower_at_ms = ctx.load_witness(F::from(999)),
            Some(11) => sources.admission_clock.lower_at_ms = ctx.load_witness(F::from(1000)),
            Some(12) => {
                sources.body_clock.signed_observations_original_digest =
                    assigned(ctx, &range, d(35))
            }
            Some(13) => sources.stream_lengths[0] = ctx.load_witness(F::from(2049)),
            Some(14) => sources.stream_digests[1] = assigned(ctx, &range, d(36)),
            Some(15) => sources.reservation_digest = assigned(ctx, &range, d(37)),
            Some(16) => {
                sources.request_digest =
                    assigned(ctx, &range, if operation == 2 { [0; 32] } else { d(38) })
            }
            Some(17) => sources.predecessor_descriptor_prefix_digest = assigned(ctx, &range, d(39)),
            Some(18) => prep.digests[2] = assigned(ctx, &range, d(40)),
            Some(19) => term.digests[2] = assigned(ctx, &range, d(41)),
            Some(20) => term.digests[3] = assigned(ctx, &range, d(42)),
            _ => (),
        }
        let mut jobs = PastaSha256JobsV1::default();
        let opening = constrain_ordinary_cash_terminal_opening_v1(
            ctx, &range, &mut jobs, &sources, &prep, &term, &intent, &record,
        )
        .unwrap();
        for (actual, expected) in [
            (opening.intent_digest, intent.binding_digest().unwrap()),
            (opening.body_digest, record.body.binding_digest().unwrap()),
            (opening.record_digest, record.binding_digest().unwrap()),
        ] {
            for (byte, expected) in actual.into_iter().zip(expected) {
                range.gate().assert_is_const(
                    ctx,
                    &byte.assigned().unwrap(),
                    &F::from(u64::from(expected)),
                );
            }
        }
        assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 4);
        builder.calculate_params(Some(UNUSABLE));
        let circuit = TestCircuit { builder, jobs };
        MockProver::run(K as u32, &circuit, vec![])
            .unwrap()
            .verify()
            .is_ok()
    }
    #[test]
    fn ordinary_cash_model_openings_full_sha_match_actual_sources_both_fields_send_and_redeem() {
        for operation in [2, 4] {
            assert!(check::<Fp>(operation, None));
            assert!(check::<Fq>(operation, None));
        }
    }
    #[test]
    fn ordinary_cash_openings_reject_nonce_phase_state_prefix_interval_output_and_original_substitution()
     {
        for operation in [2, 4] {
            for mutation in 0..=20 {
                assert!(
                    !check::<Fp>(operation, Some(mutation)),
                    "Fp operation {operation} mutation {mutation}"
                );
                assert!(
                    !check::<Fq>(operation, Some(mutation)),
                    "Fq operation {operation} mutation {mutation}"
                );
            }
        }
    }
    #[test]
    fn maintained_cash_fields_cover_complete_model_payloads_without_holes_or_alternate_order() {
        for operation in [2, 4] {
            let (intent, record) = fixtures(operation);
            for (template, domain) in [
                (
                    intent.binding_transcript(),
                    KAGEMUSHA_ORDINARY_CASH_TERMINAL_INTENT_DOMAIN_V1,
                ),
                (
                    record.body.binding_transcript(),
                    KAGEMUSHA_ORDINARY_CASH_TERMINAL_BODY_DOMAIN_V1,
                ),
                (
                    record.binding_transcript(),
                    KAGEMUSHA_ORDINARY_CASH_TERMINAL_RECORD_DOMAIN_V1,
                ),
            ] {
                let parts = template
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.name,
                            constant_bytes::<Fp>(&template.bytes[field.range.clone()]),
                        )
                    })
                    .collect::<Vec<_>>();
                assert!(fill_transcript(template.clone(), domain, parts.clone()).is_ok());
                let mut wrong = parts.clone();
                wrong.swap(0, 1);
                assert!(fill_transcript(template.clone(), domain, wrong).is_err());
                let mut wrong = parts;
                wrong.pop();
                assert!(fill_transcript(template.clone(), domain, wrong).is_err());
            }
        }
    }
}
