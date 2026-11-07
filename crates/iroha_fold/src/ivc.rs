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

/// A fresh instance and its witness.
pub type Fresh<C> = (Instance<C>, Witness<<C as PastaCurve>::ScalarExt>);
/// An accumulator and its witness.
pub type Kept<C> = (Accumulator<C>, Witness<<C as PastaCurve>::ScalarExt>);

/// One side of the chain as its circuits see it (`IVC.md` section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainSide {
    /// Domain of the IVC hash this side outputs.
    pub ivc_domain: [u8; 32],
    /// Fold domain of the other side's relation, whose instances this side folds.
    pub fold_domain: [u8; 32],
    /// Step of this side's first instance; the side with `1` starts the chain and
    /// its first instance has no predecessor.
    pub first_step: u64,
}

/// The witness of a step's fold frame
/// ([`crate::circuit::ivc::fold_frame`]).
#[derive(Clone, Debug)]
pub struct FrameWitness<C: PastaCurve> {
    /// The step index.
    pub step: u64,
    /// This side's previous state.
    pub state: Vec<C::Base>,
    /// Digest of the accumulator this side keeps, zero while absent.
    pub stored: C::Base,
    /// That accumulator (a placeholder while absent).
    pub acc: Accumulator<C>,
    /// The other side's previous instance (a placeholder without predecessor).
    pub fresh: Instance<C>,
    /// The fold messages (zero when starting or without predecessor).
    pub proof: FoldProof<C::ScalarExt>,
}

/// A step's fold done natively: the frame witness, the accumulator the side
/// keeps after the step, and the frame's outputs.
#[derive(Clone, Debug)]
pub struct PreparedStep<C: PastaCurve> {
    /// The frame witness of the step circuit.
    pub witness: FrameWitness<C>,
    /// The kept accumulator after the step.
    pub kept: Option<Kept<C>>,
    /// The other side's bridge values.
    pub bridge: Vec<C::Base>,
    /// The passthrough the step outputs.
    pub passthrough: C::Base,
    /// The kept accumulator's digest after the step (zero without predecessor).
    pub digest: C::Base,
}

/// Prepares the fold frame of step `step` of `side`: folds the other side's last
/// instance into the kept accumulator (or starts it), as the circuit will.
/// `start_passthrough` is the passthrough of an instance without predecessor:
/// the other side's base hash, the same constant the circuit holds.
///
/// # Errors
/// As [`fold_or_start`]; [`Error::Shape`] for an instance of another shape.
pub fn prepare_step<C: PastaCurve>(
    side: &ChainSide,
    step: u64,
    other: &FoldedRelation<C::ScalarExt>,
    kept: Option<&Kept<C>>,
    their_last: Option<&Fresh<C>>,
    state: Vec<C::Base>,
    start_passthrough: C::Base,
) -> Result<PreparedStep<C>, Error> {
    let zero_base = <C::Base as Field>::ZERO;
    let stored = kept.map_or(zero_base, |(acc, _)| acc.digest(&side.fold_domain));
    let acc = kept.map_or_else(|| placeholder_accumulator::<C>(other), |(acc, _)| acc.clone());
    let Some((fresh, fresh_witness)) = their_last else {
        let zero = <C::ScalarExt as Field>::ZERO;
        let proof = FoldProof { f: vec![zero; other.index_bits()], k: vec![zero; other.degree() - 1] };
        let witness = FrameWitness { step, state, stored, acc, fresh: placeholder_instance::<C>(other), proof };
        return Ok(PreparedStep {
            witness,
            kept: kept.cloned(),
            bridge: vec![zero_base; other.instance_lengths()[0].saturating_sub(2)],
            passthrough: start_passthrough,
            digest: zero_base,
        });
    };
    let public = fresh.public.first().ok_or(Error::Shape("passthrough column"))?;
    if public.len() < 2 {
        return Err(Error::Shape("passthrough column"));
    }
    let (folded, folded_witness, proof) =
        fold_or_start(other, &side.fold_domain, kept.map(|(a, w)| (a, w)), fresh, fresh_witness)?;
    let digest = folded.digest(&side.fold_domain);
    let passthrough = embed(&public[1]);
    let bridge = public[2..].iter().map(embed).collect();
    let witness = FrameWitness { step, state, stored, acc, fresh: fresh.clone(), proof };
    Ok(PreparedStep { witness, kept: Some((folded, folded_witness)), bridge, passthrough, digest })
}

/// The public inputs a step outputs: `[passthrough, h_out, bridge...]` with
/// `h_out = ivc_hash(step, base, next, digest)`.
#[must_use]
pub fn step_public<F: PastaField + iroha_pasta::poseidon::PoseidonField>(
    side: &ChainSide,
    step: u64,
    base: F,
    passthrough: F,
    next: &[F],
    digest: F,
    bridge: &[F],
) -> Vec<F> {
    let mut out = vec![passthrough, ivc_hash(&side.ivc_domain, step, base, next, digest)];
    out.extend_from_slice(bridge);
    out
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
