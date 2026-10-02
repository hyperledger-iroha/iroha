//! Reusable exact ordinary canonical struct streams from the sole model-owned field grammar.
//! Codec structure, semantic copies, proof verification and Native custody are separate duties.
use super::{assemble_bounded_canonical_frame_v1, stream::KagemushaBoundedByteStreamV1};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamGrammarV1;

/// Minimal unsigned compact length for the existing field codec, with a fixed five-byte capacity.
/// The original protocol selects capacity separately; u32 covers every maintained finite carrier.
/// The proven prefix length is derived from the actual value, without witness-selected topology.
pub(super) fn compact_u32_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    length: AssignedValue<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let gate = range.gate();
    let bits = gate.num_to_bits(ctx, length, 32);
    let groups = (0..5)
        .map(|i| {
            gate.inner_product(
                ctx,
                bits[i * 7..(i * 7 + 7).min(32)].iter().copied(),
                (0..(32 - i * 7).min(7)).map(|b| QuantumCell::Constant(F::from(1_u64 << b))),
            )
        })
        .collect::<Vec<_>>();
    let mut continues = Vec::new();
    for i in 0..4 {
        let remaining = gate.sum(ctx, groups[i + 1..].iter().copied());
        let zero = gate.is_zero(ctx, remaining);
        continues.push(gate.not(ctx, zero));
    }
    let extra = gate.sum(ctx, continues.iter().copied());
    let actual_len = gate.add(ctx, extra, QuantumCell::Constant(F::ONE));
    let bytes = groups
        .into_iter()
        .enumerate()
        .map(|(i, g)| {
            let continuation = if i < 4 {
                gate.mul(ctx, continues[i], QuantumCell::Constant(F::from(128)))
            } else {
                ctx.load_zero()
            };
            let byte = gate.add(ctx, g, continuation);
            PastaSha256ByteV1::range_checked(ctx, range, byte)
        })
        .collect();
    KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, actual_len)
}

/// Prefix one exact field payload with the sole minimal canonical field length.
pub(in crate::kagemusha_v1_recursion) fn field_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    payload: &KagemushaBoundedByteStreamV1<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let capacity = payload
        .bytes()
        .len()
        .checked_add(5)
        .ok_or("ordinary field capacity overflow")?;
    let prefix = compact_u32_v1(ctx, range, payload.actual_len())?;
    prefix.concat(ctx, range, payload, capacity)
}

/// Concatenate an immutable inventory with a balanced fixed-capacity routing tree. This retains
/// every field capacity during key generation/proving and avoids routing each leaf through the
/// entire final carrier. No host index or loop count depends on a witnessed active length.
pub(in crate::kagemusha_v1_recursion) fn concat_fields_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    fields: &[KagemushaBoundedByteStreamV1<F>],
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if fields.is_empty() {
        let zero = ctx.load_zero();
        return KagemushaBoundedByteStreamV1::constrain(ctx, range, Vec::new(), zero);
    }
    if fields.len() == 1 {
        return Ok(fields[0].clone());
    }
    let mid = fields.len() / 2;
    let left = concat_fields_v1(ctx, range, &fields[..mid])?;
    let right = concat_fields_v1(ctx, range, &fields[mid..])?;
    let capacity = left
        .bytes()
        .len()
        .checked_add(right.bytes().len())
        .ok_or("ordinary struct capacity overflow")?;
    left.concat(ctx, range, &right, capacity)
}

/// Bare canonical payload from the exact sealed model struct inventory. A field's semantic
/// source must be already constrained; assigning a DTO digest here does not authenticate it.
pub(in crate::kagemusha_v1_recursion) fn struct_payload_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    grammar: &KagemushaOrdinaryCanonicalFieldStreamGrammarV1,
    fields: &[KagemushaBoundedByteStreamV1<F>],
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if grammar.fields().len() != fields.len() {
        return Err("ordinary canonical field inventory differs".into());
    }
    let fields = fields
        .iter()
        .map(|f| field_v1(ctx, range, f))
        .collect::<Result<Vec<_>, _>>()?;
    concat_fields_v1(ctx, range, &fields)
}

/// Complete schema/header/alignment, actual active payload length and derived CRC64-XZ.
/// The same bounded buffers and field order are used at every amount/account/proof/DER width.
pub(super) fn struct_frame_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    grammar: &KagemushaOrdinaryCanonicalFieldStreamGrammarV1,
    fields: &[KagemushaBoundedByteStreamV1<F>],
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let payload = struct_payload_v1(ctx, range, grammar, fields)?;
    assemble_bounded_canonical_frame_v1(ctx, range, grammar.framing(), &payload)
}

/// Canonical Vec<u8> payload uses its maintained fixed LE64 count followed by the exact raw
/// bytes. This is distinct from the enclosing struct's compact field-length prefix.
pub(in crate::kagemusha_v1_recursion) fn byte_vector_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: &KagemushaBoundedByteStreamV1<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let bytes =
        super::super::composite::assigned_uint_bytes_v1(ctx, range.gate(), raw.actual_len(), 64);
    let length = ctx.load_constant(F::from(8));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, length)?;
    let capacity = raw
        .bytes()
        .len()
        .checked_add(8)
        .ok_or("ordinary byte vector capacity overflow")?;
    prefix.concat(ctx, range, raw, capacity)
}

/// SHA256(domain || LE64(actual original length) || complete original). Callers separately
/// copy-bind the result to the same recursively verified semantic/original public column.
pub(in crate::kagemusha_v1_recursion) fn framed_hash_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    raw: &KagemushaBoundedByteStreamV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let mut prefix = super::super::guard_bundle::constant_bytes(domain);
    prefix.extend(super::super::composite::assigned_uint_bytes_v1(
        ctx,
        range.gate(),
        raw.actual_len(),
        64,
    ));
    let prefix_capacity = prefix.len();
    let prefix_len = ctx.load_constant(F::from(prefix_capacity as u64));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, range, prefix, prefix_len)?;
    let capacity = prefix_capacity
        .checked_add(raw.bytes().len())
        .ok_or("ordinary original hash capacity overflow")?;
    let full = prefix.concat(ctx, range, raw, capacity)?;
    let words = jobs.digest_bounded_constrained(ctx, range, full.bytes(), full.actual_len())?;
    let mut bytes = Vec::new();
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    bytes
        .try_into()
        .map_err(|_| "ordinary original SHA width differs".into())
}

#[cfg(test)]
#[path = "canonical_field_stream_tests.rs"]
mod tests;
