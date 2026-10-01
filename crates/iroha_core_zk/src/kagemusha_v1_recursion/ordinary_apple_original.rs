//! Exact original Apple CBOR and signed release-extension relation for ordinary app approvals.
//!
//! Every active original byte is reconstructed from the same authData/DER cells used by SHA
//! and P-256. Fixed capacities cover the complete admitted two-key profile, both map orders,
//! and up to 128 UTF-8 version bytes. Limited authData37 has no release measurement. Neither
//! parsing nor this relation supplies a native operation owner or monetary state protection.

use super::super::canonical_preimage::stream::KagemushaBoundedByteStreamV1;
use crate::{
    kagemusha_p256_curve_gadget::app_attest_der_gadget::P256CanonicalDerV1,
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context,
    QuantumCell::Constant,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1, kagemusha_ordinary_apple_original_parts_v1,
};

const AUTH_CAP: usize = 206;
const SUFFIX_CAP: usize = 169;
const VERSION_CAP: usize = 128;
const RELEASE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-attest-release\0";

pub(super) struct OrdinaryAppleOriginalStreamsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) original: KagemushaBoundedByteStreamV1<F>,
    pub(super) authenticator: KagemushaBoundedByteStreamV1<F>,
}

fn fixed<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let len = ctx.load_constant(F::from(bytes.len() as u64));
    KagemushaBoundedByteStreamV1::constrain(
        ctx,
        range,
        bytes
            .iter()
            .copied()
            .map(PastaSha256ByteV1::constant)
            .collect(),
        len,
    )
}
fn equal_streams<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    a: &KagemushaBoundedByteStreamV1<F>,
    b: &KagemushaBoundedByteStreamV1<F>,
) -> Result<(), String> {
    if a.bytes().len() != b.bytes().len() {
        return Err("ordinary Apple original capacity differs".into());
    }
    ctx.constrain_equal(&a.actual_len(), &b.actual_len());
    for (a, b) in a.bytes().iter().zip(b.bytes()) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
    Ok(())
}
fn select_stream<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    a: &KagemushaBoundedByteStreamV1<F>,
    b: &KagemushaBoundedByteStreamV1<F>,
    choice: AssignedValue<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if a.bytes().len() != b.bytes().len() {
        return Err("ordinary Apple selection capacity differs".into());
    }
    let gate = range.gate();
    gate.assert_bit(ctx, choice);
    let len = gate.select(ctx, a.actual_len(), b.actual_len(), choice);
    let bytes = a
        .bytes()
        .iter()
        .zip(b.bytes())
        .map(|(a, b)| {
            let value = gate.select(ctx, a.quantum_cell(), b.quantum_cell(), choice);
            PastaSha256ByteV1::range_checked(ctx, range, value)
        })
        .collect();
    KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, len)
}

/// Canonical definite text/byte-string header for a proven payload no larger than 255 bytes.
fn small_header<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    len: AssignedValue<F>,
    major: u8,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let gate = range.gate();
    range.range_check(ctx, len, 8);
    let short = range.is_less_than(ctx, len, Constant(F::from(24_u64)), 8);
    let long = gate.not(ctx, short);
    let inline = gate.add(ctx, len, Constant(F::from(u64::from(major << 5))));
    let first = gate.select(
        ctx,
        Constant(F::from(u64::from((major << 5) | 24))),
        inline,
        long,
    );
    let second = gate.mul(ctx, long, len);
    let width = gate.add(ctx, Constant(F::ONE), long);
    let bytes = vec![
        PastaSha256ByteV1::range_checked(ctx, range, first),
        PastaSha256ByteV1::range_checked(ctx, range, second),
    ];
    KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, width)
}

fn between<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    byte: AssignedValue<F>,
    lo: u64,
    hi: u64,
) -> AssignedValue<F> {
    let below = range.is_less_than(ctx, byte, Constant(F::from(lo)), 9);
    let above = range.is_less_than(ctx, byte, Constant(F::from(hi + 1)), 9);
    let at_least = range.gate().not(ctx, below);
    range.gate().mul(ctx, at_least, above)
}

/// UTF-8 validity is constrained, including overlong, surrogate and >U+10FFFF exclusions.
/// The bounded stream already proves a zero tail; active NUL bytes are forbidden by policy.
fn constrain_utf8_version<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    version: &KagemushaBoundedByteStreamV1<F>,
    present: AssignedValue<F>,
) {
    let gate = range.gate();
    let nonempty = gate.is_zero(ctx, version.actual_len());
    let bad_empty = gate.mul(ctx, present, nonempty);
    gate.assert_is_const(ctx, &bad_empty, &F::ZERO);
    let absent = gate.not(ctx, present);
    let absent_len = gate.mul(ctx, absent, version.actual_len());
    gate.assert_is_const(ctx, &absent_len, &F::ZERO);
    let mut remaining = ctx.load_constant(F::ZERO);
    let mut previous = ctx.load_constant(F::ZERO);
    for (index, byte) in version.bytes().iter().enumerate() {
        let b = byte.assigned().expect("version byte assigned");
        let active = range.is_less_than(
            ctx,
            Constant(F::from(index as u64)),
            version.actual_len(),
            8,
        );
        let stopped = gate.mul_not(ctx, active, remaining);
        gate.assert_is_const(ctx, &stopped, &F::ZERO);
        let boundary = gate.is_zero(ctx, remaining);
        let continuing = gate.not(ctx, boundary);
        let ascii = between(ctx, range, b, 1, 0x7f);
        let lead2 = between(ctx, range, b, 0xc2, 0xdf);
        let lead3 = between(ctx, range, b, 0xe0, 0xef);
        let lead4 = between(ctx, range, b, 0xf0, 0xf4);
        let valid_lead = gate.sum(ctx, [ascii, lead2, lead3, lead4]);
        let continuation = between(ctx, range, b, 0x80, 0xbf);
        let valid = gate.select(ctx, valid_lead, continuation, boundary);
        let invalid = gate.sub(ctx, Constant(F::ONE), valid);
        let bad = gate.mul(ctx, active, invalid);
        gate.assert_is_const(ctx, &bad, &F::ZERO);
        for (lead, lo, hi) in [
            (0xe0, 0xa0, 0xbf),
            (0xed, 0x80, 0x9f),
            (0xf0, 0x90, 0xbf),
            (0xf4, 0x80, 0x8f),
        ] {
            let special = gate.is_equal(ctx, previous, Constant(F::from(lead)));
            let limited = between(ctx, range, b, lo, hi);
            let invalid = gate.not(ctx, limited);
            let selected = gate.mul(ctx, special, continuing);
            let bad = gate.mul(ctx, selected, invalid);
            gate.assert_is_const(ctx, &bad, &F::ZERO);
        }
        let count = gate.inner_product(
            ctx,
            [lead2, lead3, lead4],
            [
                Constant(F::ONE),
                Constant(F::from(2_u64)),
                Constant(F::from(3_u64)),
            ],
        );
        let decremented = gate.sub(ctx, remaining, Constant(F::ONE));
        let next = gate.select(ctx, count, decremented, boundary);
        remaining = gate.mul(ctx, active, next);
        previous = gate.mul(ctx, active, b);
    }
    gate.assert_is_const(ctx, &remaining, &F::ZERO);
}

pub(super) fn bounded_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    stream: &KagemushaBoundedByteStreamV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let words = jobs.digest_bounded_constrained(ctx, range, stream.bytes(), stream.actual_len())?;
    let mut bytes = Vec::with_capacity(32);
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
        .map_err(|_| "ordinary Apple bounded SHA width differs".into())
}

/// Reconstruct both canonical outer/extension orders without a witness-dependent circuit shape.
/// The supplied host digest is used solely to reject malformed witnesses; the assigned digest
/// must be copy-bound to the actual complete credential original by the enclosing relation.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn constrain_original_apple_assertion_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw: &[u8],
    header: &[AssignedValue<F>; 37],
    expected_release: [u8; 32],
    expected_release_cells: &[AssignedValue<F>; 32],
    der: &P256CanonicalDerV1<F>,
) -> Result<OrdinaryAppleOriginalStreamsV1<F>, String> {
    let parts = kagemusha_ordinary_apple_original_parts_v1(raw, expected_release)?;
    let version_witness = parts.bundle_version.unwrap_or("").as_bytes();
    let category_witness = parts.validation_category.unwrap_or(0);
    let present_witness = parts.bundle_version.is_some();
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let present = ctx.load_witness(F::from(u64::from(present_witness)));
    gate.assert_bit(ctx, present);
    let extension_flag = ctx.load_witness(F::from(u64::from(parts.authenticator_data[32] == 0xc0)));
    gate.assert_bit(ctx, extension_flag);
    let without_suffix = gate.mul_not(ctx, present, extension_flag);
    gate.assert_is_const(ctx, &without_suffix, &F::ZERO);
    let flags = gate.mul_add(
        ctx,
        extension_flag,
        Constant(F::from(0x80_u64)),
        Constant(F::from(0x40_u64)),
    );
    ctx.constrain_equal(&header[32], &flags);
    let version_len = ctx.load_witness(F::from(version_witness.len() as u64));
    let version = (0..VERSION_CAP)
        .map(|i| {
            let b = ctx.load_witness(F::from(u64::from(
                version_witness.get(i).copied().unwrap_or(0),
            )));
            PastaSha256ByteV1::range_checked(ctx, &range, b)
        })
        .collect();
    let version = KagemushaBoundedByteStreamV1::constrain(ctx, &range, version, version_len)?;
    constrain_utf8_version(ctx, &range, &version, present);
    let category_bytes: [PastaSha256ByteV1<F>; 4] = std::array::from_fn(|i| {
        let b = ctx.load_witness(F::from(u64::from(category_witness.to_le_bytes()[i])));
        PastaSha256ByteV1::range_checked(ctx, &range, b)
    });
    let category = gate.inner_product(
        ctx,
        category_bytes
            .iter()
            .copied()
            .map(PastaSha256ByteV1::quantum_cell),
        (0..4).map(|i| Constant(F::from(1_u64 << (8 * i)))),
    );
    let mut polynomial = ctx.load_constant(F::ONE);
    for allowed in [1_u64, 2, 3, 4, 5, 6, 10] {
        let difference = gate.sub(ctx, category, Constant(F::from(allowed)));
        polynomial = gate.mul(ctx, polynomial, difference);
    }
    let bad = gate.mul(ctx, present, polynomial);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let absent = gate.not(ctx, present);
    let bad = gate.mul(ctx, absent, category);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let mut category_entry = fixed(ctx, &range, b"\x72validationCategory\x44")?;
    let category_len = ctx.load_constant(F::from(4_u64));
    let category_stream = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        category_bytes.to_vec(),
        category_len,
    )?;
    category_entry = category_entry.concat(ctx, &range, &category_stream, 24)?;
    let version_key = fixed(ctx, &range, b"\x6dbundleVersion")?;
    let version_header = small_header(ctx, &range, version.actual_len(), 3)?;
    let version_entry = version_key
        .concat(ctx, &range, &version_header, 16)?
        .concat(ctx, &range, &version, 144)?;
    let cv = category_entry.concat(ctx, &range, &version_entry, 168)?;
    let vc = version_entry.concat(ctx, &range, &category_entry, 168)?;
    let cat_first = ctx.load_witness(F::from(u64::from(
        parts.authenticator_data.get(38) == Some(&0x72),
    )));
    let contents = select_stream(ctx, &range, &cv, &vc, cat_first)?;
    let map = fixed(ctx, &range, b"\xa2")?;
    let full_suffix = map.concat(ctx, &range, &contents, SUFFIX_CAP)?;
    let suffix_len = gate.mul(ctx, present, full_suffix.actual_len());
    let suffix_bytes = full_suffix
        .bytes()
        .iter()
        .map(|b| {
            let value = gate.mul(ctx, present, b.quantum_cell());
            PastaSha256ByteV1::range_checked(ctx, &range, value)
        })
        .collect();
    let suffix = KagemushaBoundedByteStreamV1::constrain(ctx, &range, suffix_bytes, suffix_len)?;
    let header_bytes = header
        .iter()
        .copied()
        .map(|b| PastaSha256ByteV1::range_checked(ctx, &range, b))
        .collect();
    let header_len = ctx.load_constant(F::from(37_u64));
    let header_stream =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, header_bytes, header_len)?;
    let authenticator = header_stream.concat(ctx, &range, &suffix, AUTH_CAP)?;

    // The governed release digest commits the exact category/version that occurs in authData.
    let release_domain = fixed(ctx, &range, RELEASE_DOMAIN)?;
    let version_length_bits = PastaSha256BitV1::decompose(ctx, gate, version.actual_len(), 16);
    let length_bytes = version_length_bits
        .chunks_exact(8)
        .map(|b| PastaSha256ByteV1::from_bits_le(ctx, gate, b))
        .collect();
    let length_len = ctx.load_constant(F::from(2_u64));
    let length_stream =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, length_bytes, length_len)?;
    let release_prefix = release_domain
        .concat(ctx, &range, &category_stream, RELEASE_DOMAIN.len() + 4)?
        .concat(ctx, &range, &length_stream, RELEASE_DOMAIN.len() + 6)?;
    let release = release_prefix.concat(
        ctx,
        &range,
        &version,
        RELEASE_DOMAIN.len() + 6 + VERSION_CAP,
    )?;
    let digest = bounded_hash(ctx, &range, jobs, &release)?;
    for (actual, expected) in digest.iter().zip(expected_release_cells) {
        range.range_check(ctx, *expected, 8);
        let difference = gate.sub(ctx, actual.quantum_cell(), *expected);
        let bad = gate.mul(ctx, present, difference);
        gate.assert_is_const(ctx, &bad, &F::ZERO);
    }

    // Authenticator lengths37..206 always use the one-byte additional canonical bstr length.
    let auth_key = fixed(ctx, &range, b"\x71authenticatorData\x58")?;
    let auth_len_byte = PastaSha256ByteV1::range_checked(ctx, &range, authenticator.actual_len());
    let one = ctx.load_constant(F::ONE);
    let auth_length =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, vec![auth_len_byte], one)?;
    let auth_entry =
        auth_key
            .concat(ctx, &range, &auth_length, 20)?
            .concat(ctx, &range, &authenticator, 226)?;
    let der = KagemushaBoundedByteStreamV1::constrain(ctx, &range, der.bytes.to_vec(), der.len)?;
    let too_short = range.is_less_than(ctx, der.actual_len(), Constant(F::from(8_u64)), 7);
    gate.assert_is_const(ctx, &too_short, &F::ZERO);
    let sig_key = fixed(ctx, &range, b"\x69signature")?;
    let sig_header = small_header(ctx, &range, der.actual_len(), 2)?;
    let sig = sig_key
        .concat(ctx, &range, &sig_header, 12)?
        .concat(ctx, &range, &der, 84)?;
    let as_order = auth_entry.concat(ctx, &range, &sig, 310)?;
    let sa_order = sig.concat(ctx, &range, &auth_entry, 310)?;
    let auth_first = map.concat(
        ctx,
        &range,
        &as_order,
        KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1,
    )?;
    let sig_first = map.concat(
        ctx,
        &range,
        &sa_order,
        KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1,
    )?;
    let order = ctx.load_witness(F::from(u64::from(raw.get(1) == Some(&0x71))));
    let reconstructed = select_stream(ctx, &range, &auth_first, &sig_first, order)?;
    let raw_len = ctx.load_witness(F::from(raw.len() as u64));
    let raw_bytes = (0..KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1)
        .map(|i| {
            let value = ctx.load_witness(F::from(u64::from(raw.get(i).copied().unwrap_or(0))));
            PastaSha256ByteV1::range_checked(ctx, &range, value)
        })
        .collect();
    let original = KagemushaBoundedByteStreamV1::constrain(ctx, &range, raw_bytes, raw_len)?;
    equal_streams(ctx, &range, &original, &reconstructed)?;
    Ok(OrdinaryAppleOriginalStreamsV1 {
        original,
        authenticator,
    })
}

#[cfg(test)]
#[path = "ordinary_apple_original_tests.rs"]
mod tests;
