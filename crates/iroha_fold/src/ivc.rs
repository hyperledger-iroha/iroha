//! The native side of the two-curve IVC (output `IVC.md` sections 4 and 5).
//!
//! Instances alternate sides, `Q_1, A_1, Q_2, A_2, ...`; each one folds the
//! previous instance of the other side and carries the public inputs
//! `[passthrough, h_out, bridge...]`. `h_out` is [`ivc_hash`] over the circuit's
//! own field, truncated to [`HASH_BITS`] bits so that the same integer is a
//! canonical element of both Pasta fields ([`embed`]); `passthrough` copies the
//! previous instance's `h_out`, and the next instance of the same side checks it.

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField, poseidon::Sponge};

use crate::{
    Error,
    commit::Witness,
    fold::{self, Accumulator, FoldProof, Instance},
    relation::FoldedRelation,
};

/// Role tag of an IVC hash.
pub const TAG_IVC: u64 = u64::from_le_bytes(*b"kgwfivc1");
/// Bits kept of an IVC hash: the integer is canonical in both Pasta fields.
pub const HASH_BITS: u32 = 250;

/// The low [`HASH_BITS`] bits of `value`.
#[must_use]
pub fn truncate<F: PastaField>(value: &F) -> F {
    let mut limbs = value.to_canonical_limbs();
    limbs[3] &= (1u64 << (HASH_BITS - 192)) - 1;
    F::from_raw_reduced(limbs)
}

/// The same integer in another Pasta field; exact for every value below `2^254`,
/// in particular for truncated hashes and bounded bridge values.
#[must_use]
pub fn embed<F: PastaField, G: PastaField>(value: &F) -> G {
    G::from_raw_reduced(value.to_canonical_limbs())
}

/// The IVC hash of one side: `H(kgwfivc1, domain, step, base, state, digest)`
/// truncated to [`HASH_BITS`] bits. `digest` is the digest of the accumulator the
/// side keeps, zero while it is absent.
#[must_use]
pub fn ivc_hash<F: PastaField + iroha_pasta::poseidon::PoseidonField>(
    domain: &[u8; 32],
    step: u64,
    base: F,
    state: &[F],
    digest: F,
) -> F {
    let mut sponge = Sponge::<F>::new();
    sponge.update(&[F::from(TAG_IVC)]);
    for chunk in domain.as_chunks::<16>().0 {
        sponge.update(&[F::from_u128(u128::from_le_bytes(*chunk))]);
    }
    sponge.update(&[F::from(step), base]);
    sponge.update(state);
    sponge.update(&[digest]);
    truncate(&sponge.squeeze())
}

/// The accumulator, its witness and the fold messages of [`fold_or_start`].
pub type FoldOrStart<C> = (
    Accumulator<C>,
    Witness<<C as PastaCurve>::ScalarExt>,
    FoldProof<<C as PastaCurve>::ScalarExt>,
);

/// Folds `fresh` into `acc`, or starts an accumulator from it when the side keeps
/// none yet (the first fold of a kind). A start carries zero messages, which the
/// circuit ignores ([`crate::circuit::verify::verify_fold_or_start`]).
///
/// # Errors
/// As [`fold::fold`] and [`Accumulator::from_fresh`].
pub fn fold_or_start<C: PastaCurve>(
    relation: &FoldedRelation<C::ScalarExt>,
    domain: &[u8; 32],
    acc: Option<(&Accumulator<C>, &Witness<C::ScalarExt>)>,
    fresh: &Instance<C>,
    fresh_witness: &Witness<C::ScalarExt>,
) -> Result<FoldOrStart<C>, Error> {
    match acc {
        Some((acc, witness)) => fold::fold(relation, domain, acc, witness, fresh, fresh_witness),
        None => {
            let started = Accumulator::from_fresh(relation, domain, fresh.clone())?;
            let zero = <C::ScalarExt as Field>::ZERO;
            let proof = FoldProof {
                f: vec![zero; relation.index_bits()],
                k: vec![zero; relation.degree() - 1],
            };
            Ok((started, fresh_witness.clone(), proof))
        }
    }
}

/// An instance of `relation`'s shape with identity commitments and zero values:
/// the witness of the fold inputs a circuit ignores (no predecessor, or no
/// accumulator yet), so that every instance has one layout.
#[must_use]
pub fn placeholder_instance<C: PastaCurve>(relation: &FoldedRelation<C::ScalarExt>) -> Instance<C> {
    let zero = <C::ScalarExt as Field>::ZERO;
    Instance {
        commitments: vec![C::AffineExt::default(); relation.columns()],
        public: relation.instance_lengths().iter().map(|len| vec![zero; *len]).collect(),
        theta: vec![zero; relation.theta_len()],
        r: vec![zero; relation.groups()],
    }
}

/// An accumulator of `relation`'s shape with identity commitments and zero values.
#[must_use]
pub fn placeholder_accumulator<C: PastaCurve>(relation: &FoldedRelation<C::ScalarExt>) -> Accumulator<C> {
    let zero = <C::ScalarExt as Field>::ZERO;
    Accumulator {
        instance: placeholder_instance(relation),
        beta: vec![zero; relation.index_bits()],
        e: zero,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::{Fp, Fq};

    #[test]
    fn truncated_hashes_embed_exactly() {
        let h = ivc_hash::<Fq>(&[7; 32], 3, Fq::from(5u64), &[Fq::from(9u64)], Fq::ZERO);
        assert_eq!(h.to_canonical_limbs()[3] >> (HASH_BITS - 192), 0);
        let in_fp: Fp = embed(&h);
        assert_eq!(in_fp.to_canonical_limbs(), h.to_canonical_limbs());
        assert_eq!(embed::<Fp, Fq>(&in_fp), h);
        assert_ne!(h, ivc_hash::<Fq>(&[7; 32], 4, Fq::from(5u64), &[Fq::from(9u64)], Fq::ZERO));
    }
}
