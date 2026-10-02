//! Exact PUBLIC outer State original from the actual outer verifier's assigned cells.
//! The caller authenticates its outer protocol, folds all current/history openings and enforces
//! every reciprocal equation. An inner State reader cannot provide these outer operands.
use super::{
    KagemushaOrdinaryLineageStateOriginalV1, KagemushaPastaParityV1,
    canonical_preimage::{
        assemble_bounded_canonical_frame_v1,
        field_stream::{byte_vector_v1, struct_payload_v1},
        stream::KagemushaBoundedByteStreamV1,
    },
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned},
    state_relation::{RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT as CELLS, public_instance as s},
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1 as Grammar;
type Stream<F> = KagemushaBoundedByteStreamV1<F>;
type Bytes<F> = [PastaSha256ByteV1<F>; 32];

/// Operands are reconstructed from the exact recursively verified OUTER proof. The other
/// parity's bytes are bound by the common full original SHA exposed by both successor proofs.
pub(super) struct OrdinaryOuterParentCanonicalSourcesV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) column: &'a [AssignedValue<F>],
    /// Canonical native scalar bytes of the same93 actual outer semantic cells.
    pub(super) canonical_column: &'a [Bytes<F>],
    pub(super) canonical_current_original: &'a [PastaSha256ByteV1<F>],
    pub(super) counterpart_current_original: &'a [u8],
    pub(super) counterpart_history: &'a [u8],
    /// Actual installed outer Eq/Ep ordinary transcript lengths, never offered lengths.
    pub(super) proof_widths: [usize; 2],
    pub(super) parity: KagemushaPastaParityV1,
}
fn fixed<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
) -> Result<Stream<F>, String> {
    let len = ctx.load_constant(F::from(bytes.len() as u64));
    Stream::constrain(ctx, range, bytes, len)
}
fn raw<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
    width: usize,
) -> Result<Stream<F>, String> {
    if bytes.len() != width {
        return Err("outer parent original differs from installed width".into());
    }
    let assigned = assign_bytes(ctx, range, bytes);
    fixed(ctx, range, assigned)
}
fn literal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
) -> Result<Stream<F>, String> {
    fixed(ctx, range, constant_bytes(bytes))
}
fn digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    at: usize,
) -> Result<Stream<F>, String> {
    let limbs = column
        .get(at..at + 2)
        .ok_or("outer parent digest column absent")?;
    let bytes = assigned_digest_bytes_v1(ctx, range.gate(), [limbs[0], limbs[1]]);
    fixed(ctx, range, bytes)
}
fn current_history<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
) -> Result<Stream<F>, String> {
    let limbs = column
        .get(CELLS..)
        .ok_or("outer parent full history absent")?;
    if limbs.len() != 34 {
        return Err("outer parent entire history has wrong width".into());
    }
    let mut bytes = Vec::new();
    for value in limbs {
        range.range_check(ctx, *value, 128);
        bytes.extend(assigned_uint_bytes_v1(ctx, range.gate(), *value, 128));
    }
    fixed(ctx, range, bytes)
}

/// Sole fixed-array inventory: each scalar32 is an array of32 length-prefixed u8 elements;
/// its64-byte bare payload is itself length-prefixed once in the93-element column array.
/// The exact pattern is checked against the actual maintained Core encoder before construction.
fn column_payload<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    template: &[u8],
    column: &[AssignedValue<F>],
    canonical: &[Bytes<F>],
    parity: KagemushaPastaParityV1,
) -> Result<Stream<F>, String> {
    if template.len() != CELLS * 65 || canonical.len() != CELLS || column.len() != CELLS + 34 {
        return Err("outer public State fixed array inventory differs".into());
    }
    let mut payload = Vec::with_capacity(template.len());
    for (i, encoded) in template.chunks_exact(65).enumerate() {
        if encoded[0] != 64 || encoded[1..].chunks_exact(2).any(|b| b[0] != 1) {
            return Err("outer public State fixed array codec changed".into());
        }
        let bytes = match (i, parity) {
            (s::PREDECESSOR_STATE, KagemushaPastaParityV1::Eq) => assigned_digest_bytes_v1(
                ctx,
                range.gate(),
                [
                    column[s::PREDECESSOR_EQ_COMPONENT_LO],
                    column[s::PREDECESSOR_EQ_COMPONENT_HI],
                ],
            ),
            (s::PREDECESSOR_STATE, KagemushaPastaParityV1::Ep) => assigned_digest_bytes_v1(
                ctx,
                range.gate(),
                [
                    column[s::PREDECESSOR_EP_COMPONENT_LO],
                    column[s::PREDECESSOR_EP_COMPONENT_HI],
                ],
            ),
            (s::SUCCESSOR_STATE, KagemushaPastaParityV1::Eq) => assigned_digest_bytes_v1(
                ctx,
                range.gate(),
                [
                    column[s::SUCCESSOR_EQ_COMPONENT_LO],
                    column[s::SUCCESSOR_EQ_COMPONENT_HI],
                ],
            ),
            (s::SUCCESSOR_STATE, KagemushaPastaParityV1::Ep) => assigned_digest_bytes_v1(
                ctx,
                range.gate(),
                [
                    column[s::SUCCESSOR_EP_COMPONENT_LO],
                    column[s::SUCCESSOR_EP_COMPONENT_HI],
                ],
            ),
            _ => canonical[i].to_vec(),
        };
        payload.extend(constant_bytes(&[64]));
        for byte in bytes {
            payload.extend(constant_bytes(&[1]));
            payload.push(byte);
        }
    }
    fixed(ctx, range, payload)
}

/// Recreate the complete public original: exact projection, both actual current outer proofs,
/// both complete histories, protocol/audit metadata, schema/alignment/length and derived CRC.
/// This output must be SHA-copy-bound to the original predecessor selector; it grants nothing.
pub(super) fn constrain_outer_parent_public_original_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    source: OrdinaryOuterParentCanonicalSourcesV1<'_, F>,
    data: &KagemushaOrdinaryLineageStateOriginalV1,
) -> Result<(Stream<F>, Bytes<F>), String> {
    if source.column.len() != CELLS + 34 || source.canonical_column.len() != CELLS {
        return Err("outer parent exact current public column differs".into());
    }
    let current_index = if source.parity == KagemushaPastaParityV1::Eq {
        0
    } else {
        1
    };
    if source.canonical_current_original.len() != source.proof_widths[current_index]
        || source.proof_widths.contains(&0)
    {
        return Err("outer parent real current width differs".into());
    }
    let (projection_grammar, original_grammar) = data.canonical_field_grammars()?;
    let proof_grammar = Grammar::from_original(data.proof())?;
    // Both native State slots are copied to their full component digests. All other cells are
    // shared canonical u128 values, so the counterpart projection has no free scalar witness.
    for (i, offset) in [
        (
            s::PREDECESSOR_STATE,
            if current_index == 0 {
                s::PREDECESSOR_EQ_COMPONENT_LO
            } else {
                s::PREDECESSOR_EP_COMPONENT_LO
            },
        ),
        (
            s::SUCCESSOR_STATE,
            if current_index == 0 {
                s::SUCCESSOR_EQ_COMPONENT_LO
            } else {
                s::SUCCESSOR_EP_COMPONENT_LO
            },
        ),
    ] {
        let expected = assigned_digest_bytes_v1(
            ctx,
            range.gate(),
            [source.column[offset], source.column[offset + 1]],
        );
        for (actual, expected) in source.canonical_column[i].iter().zip(expected) {
            let a = range
                .gate()
                .add(ctx, actual.quantum_cell(), QuantumCell::Constant(F::ZERO));
            let e = range
                .gate()
                .add(ctx, expected.quantum_cell(), QuantumCell::Constant(F::ZERO));
            ctx.constrain_equal(&a, &e);
        }
    }
    for i in 0..CELLS {
        if !matches!(i, s::PREDECESSOR_STATE | s::SUCCESSOR_STATE) {
            range.range_check(ctx, source.column[i], 128);
        }
    }
    let version = literal(ctx, range, &1_u16.to_le_bytes())?;
    let eq_column = column_payload(
        ctx,
        range,
        &projection_grammar.fields()[1],
        source.column,
        source.canonical_column,
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_column = column_payload(
        ctx,
        range,
        &projection_grammar.fields()[2],
        source.column,
        source.canonical_column,
        KagemushaPastaParityV1::Ep,
    )?;
    let projection = struct_payload_v1(
        ctx,
        range,
        &projection_grammar,
        &[version.clone(), eq_column, ep_column],
    )?;
    let current = fixed(ctx, range, source.canonical_current_original.to_vec())?;
    let other = raw(
        ctx,
        range,
        source.counterpart_current_original,
        source.proof_widths[1 - current_index],
    )?;
    let history = current_history(ctx, range, source.column)?;
    let other_history = raw(ctx, range, source.counterpart_history, 544)?;
    let (eq_proof, ep_proof, eq_history, ep_history) = if current_index == 0 {
        (current, other, history, other_history)
    } else {
        (other, current, other_history, history)
    };
    let mut proof_fields = vec![version.clone()];
    for at in [
        s::EQ_PROTOCOL_LO,
        s::EP_PROTOCOL_LO,
        s::TRANSPORT_LO,
        s::GUARD_EQ_CREDENTIAL_AUDIT_LO,
        s::GUARD_EP_CREDENTIAL_AUDIT_LO,
        s::EQ_DEFERRED_AUDIT_LO,
        s::EP_DEFERRED_AUDIT_LO,
    ] {
        proof_fields.push(digest(ctx, range, source.column, at)?);
    }
    for raw in [eq_proof, ep_proof, eq_history, ep_history] {
        proof_fields.push(byte_vector_v1(ctx, range, &raw)?);
    }
    let proof = struct_payload_v1(ctx, range, &proof_grammar, &proof_fields)?;
    let payload = struct_payload_v1(ctx, range, &original_grammar, &[version, projection, proof])?;
    let original =
        assemble_bounded_canonical_frame_v1(ctx, range, original_grammar.framing(), &payload)?;
    let words =
        jobs.digest_bounded_constrained(ctx, range, original.bytes(), original.actual_len())?;
    let mut hash = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            hash.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    Ok((
        original,
        hash.try_into()
            .map_err(|_| "outer parent SHA width differs")?,
    ))
}

/// The exact full public predecessor original identity opened in ordinary Mint113 cell32.
pub(super) fn constrain_mint_parent_original_sha_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    actual: &Bytes<F>,
    mint_column: &[AssignedValue<F>],
    enabled: AssignedValue<F>,
) -> Result<(), String> {
    let expected = mint_column
        .get(64..66)
        .ok_or("ordinary Mint113 full predecessor SHA absent")?;
    for (a, e) in digest_limbs_assigned(ctx, actual).into_iter().zip(expected) {
        let d = range.gate().sub(ctx, a, *e);
        let selected = range.gate().mul(ctx, d, enabled);
        range.gate().assert_is_const(ctx, &selected, &F::ZERO);
    }
    Ok(())
}
