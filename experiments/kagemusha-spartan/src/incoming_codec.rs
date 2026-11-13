//! Total parsing of fixed-profile proof words, preserving the original digest.
use super::*;
use crate::proof_bytes::{self, SharedBytes};
use group::GroupEncoding;

pub(super) const PARSE_BITS: usize = 2;
pub(super) const VERDICT_BITS: usize = 8 + PARSE_BITS;

fn canonical_scalars<S: Scalar>(raw: &[u8], offsets: &[usize]) -> bool {
    offsets
        .iter()
        .all(|at| Option::<S>::from(S::from_repr(raw[*at..*at + 32].try_into().unwrap())).is_some())
}
fn canonical_points<C: iroha_pasta::PastaCurve>(raw: &[u8], offsets: &[usize]) -> bool {
    offsets.iter().all(|at| {
        bool::from(
            crate::ipa::Aff::<C>::from_bytes(&raw[*at..*at + 32].try_into().unwrap()).is_some(),
        )
    })
}
pub(super) fn native_bits(pair: &TracedPair, wire: &IncomingEncoding) -> [u128; 2] {
    let (pp, ps) = word_offsets(&pair.proof_p, wire.primary);
    let (sp, ss) = word_offsets(&pair.proof_s, wire.secondary);
    [
        u128::from(
            canonical_scalars::<Fp>(&wire.bytes, &ps)
                && canonical_points::<iroha_pasta::Ep>(&wire.bytes, &sp),
        ),
        u128::from(
            canonical_scalars::<crate::verifier::Fq>(&wire.bytes, &ss)
                && canonical_scalars::<crate::verifier::Fq>(&wire.bytes, &[42])
                && canonical_points::<iroha_pasta::Eq>(&wire.bytes, &pp),
        ),
    ]
}
fn conjunction<S: Scalar>(w: &mut Wiring<S>, family: usize, bits: &[VerifierCell]) -> VerifierCell {
    let mut values = vec![vec![S::ONE]];
    let mut rows = Vec::new();
    for (i, bit) in bits.iter().enumerate() {
        let v = values[i][0] * w.arrays[bit.array].values[bit.cell][bit.instance];
        values.push(vec![v]);
        rows.push([own(i, 1), bit.linear(), own(i + 1, 1)]);
    }
    rows.push([
        plus(&own(0, 1), &constant(vec![-S::ONE])),
        constant(vec![S::ONE]),
        vec![],
    ]);
    let array = w.push(Array {
        name: "derived complete proof parse conjunction",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![1; values.len()],
        values,
        rows,
    });
    VerifierCell {
        array,
        cell: bits.len(),
        instance: 0,
    }
}

/// Derive this field's half of the parser and join the paired result. The
/// selected proof is either the original or a unique all-zero canonical
/// encoding; the enclosing circuit still hashes `original` for Payment.
pub(super) fn select<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    original: &SharedBytes,
    wire: &IncomingEncoding,
    pair: &TracedPair,
    shared: usize,
    verdict_at: usize,
    primary: bool,
) -> SharedBytes {
    let (pp, ps) = word_offsets(&pair.proof_p, wire.primary);
    let (sp, ss) = word_offsets(&pair.proof_s, wire.secondary);
    let (scalars, points) = if primary { (ps, sp) } else { (ss, pp) };
    let mut bits = Vec::with_capacity(scalars.len() + points.len());
    if !primary { bits.push(proof_bytes::total_scalar(w,family,original,42).canonical); }
    for at in scalars {
        bits.push(proof_bytes::total_scalar(w, family, original, at).canonical);
    }
    for at in points {
        bits.push(proof_bytes::total_point(w, family, original, at).canonical);
    }
    let local = conjunction(w, family, &bits);
    bind_verdict(
        w,
        family,
        local,
        shared,
        verdict_at + 8 + usize::from(!primary),
    );
    let full = conjunction(
        w,
        family,
        &[
            VerifierCell {
                array: shared,
                cell: 0,
                instance: verdict_at + 8,
            },
            VerifierCell {
                array: shared,
                cell: 0,
                instance: verdict_at + 9,
            },
        ],
    );
    let mut dummy = vec![0; wire.bytes.len()];
    // Framing is checked before monetary admission and remains exactly bound.
    dummy[..42].copy_from_slice(&wire.bytes[..42]);
    proof_bytes::select_canonical_bytes(w, family, original, full, &dummy)
}
