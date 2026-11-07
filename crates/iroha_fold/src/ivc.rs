//! The native side of the two-curve IVC (output `IVC.md` sections 4 and 5).
//!
//! Instances alternate sides, `Q_1, A_1, Q_2, A_2, ...`; each one folds the
//! previous instance of the other side and carries the public inputs
//! `[passthrough, h_out, bridge...]`. `h_out` is [`ivc_hash`] over the circuit's
//! own field, truncated to [`HASH_BITS`] bits so that the same integer is a
//! canonical element of both Pasta fields ([`embed`]); `passthrough` copies the
//! previous instance's `h_out`, and the next instance of the same side checks it.
//!
//! A side keeps a list of digests, hashed in a fixed order: one accumulator per
//! kind of the other side's relations (non-uniform IVC, `IVC.md` section 4),
//! then the side's own claim accumulator (section 6), each zero while absent.
//! A step circuit folds the other side's previous instance into the one slot
//! its schedule fixes ([`ChainSide::slot`]), or into one of a fixed set of
//! slots its witness chooses ([`ChainSide::choices`], for a step that follows
//! several kinds), and passes the others through; its business may replace
//! the claim digest. A wrong choice is never accepted in the end: each slot's
//! accumulator is decided against its own relation.

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

/// The IVC hash of one side: `H(kgwfivc1, domain, step, base, state, digests)`
/// truncated to [`HASH_BITS`] bits. `digests` is the side's digest list: the
/// digests of the accumulators it keeps, in kind order, then its claim
/// accumulator's digest, each zero while absent.
#[must_use]
pub fn ivc_hash<F: PastaField + iroha_pasta::poseidon::PoseidonField>(
    domain: &[u8; 32],
    step: u64,
    base: F,
    state: &[F],
    digests: &[F],
) -> F {
    let mut sponge = Sponge::<F>::new();
    sponge.update(&[F::from(TAG_IVC)]);
    for chunk in domain.as_chunks::<16>().0 {
        sponge.update(&[F::from_u128(u128::from_le_bytes(*chunk))]);
    }
    sponge.update(&[F::from(step), base]);
    sponge.update(state);
    sponge.update(digests);
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

/// The frame of one step circuit (`IVC.md` section 4): its side's hash
/// domain, chain start and digest list, and the kinds of the other side's
/// relations it may fold, which the schedule fixes per circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainSide {
    /// Domain of the IVC hash this side outputs.
    pub ivc_domain: [u8; 32],
    /// Fold domain of the other side's relations this circuit folds (every
    /// kind in [`Self::choices`] folds under it).
    pub fold_domain: [u8; 32],
    /// Step of this side's first instance; the side with `1` starts the chain and
    /// its first instance has no predecessor.
    pub first_step: u64,
    /// Position, in the side's digest list, of the accumulator this fold uses:
    /// the circuit's fixed slot, or the slot a witness chooses among
    /// [`Self::choices`].
    pub slot: usize,
    /// Length of the side's digest list (kept accumulators, then the claim
    /// accumulator).
    pub digests: usize,
    /// The slots this circuit may fold into, as a bit set (bit `i` for slot `i`).
    /// One bit fixes the slot; several make the witness choose one.
    pub choices: u64,
}

impl ChainSide {
    /// Whether the slot lies in the digest list and among the choices, every
    /// choice lies in the list, and the list fits the bit set.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.slot < self.digests
            && self.digests < 64
            && self.choices >> self.digests == 0
            && (self.choices >> self.slot) & 1 == 1
    }

    /// Whether the witness chooses the slot among several.
    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.choices.count_ones() > 1
    }

    /// The same frame folding into `slot` (one of the choices) instead.
    #[must_use]
    pub const fn folding_into(self, slot: usize) -> Self {
        Self { slot, ..self }
    }
}

/// The witness of a step's fold frame
/// ([`crate::circuit::ivc::fold_frame`]).
#[derive(Clone, Debug)]
pub struct FrameWitness<C: PastaCurve> {
    /// The step index.
    pub step: u64,
    /// This side's previous state.
    pub state: Vec<C::Base>,
    /// This side's previous digest list ([`ChainSide::digests`] entries).
    pub digests: Vec<C::Base>,
    /// The slot this step folds into ([`ChainSide::slot`] when prepared).
    pub slot: usize,
    /// The accumulator in that slot (a placeholder while absent).
    pub acc: Accumulator<C>,
    /// The other side's previous instance (a placeholder without predecessor).
    pub fresh: Instance<C>,
    /// The fold messages (zero when starting or without predecessor).
    pub proof: FoldProof<C::ScalarExt>,
}

/// A step's fold done natively: the frame witness, the accumulator in the
/// folded slot after the step, and the frame's outputs.
#[derive(Clone, Debug)]
pub struct PreparedStep<C: PastaCurve> {
    /// The frame witness of the step circuit.
    pub witness: FrameWitness<C>,
    /// The accumulator in the folded slot after the step.
    pub kept: Option<Kept<C>>,
    /// The other side's bridge values.
    pub bridge: Vec<C::Base>,
    /// The passthrough the step outputs.
    pub passthrough: C::Base,
    /// The digest list after the fold: the folded slot replaced, every other
    /// digest passed through; all zero without predecessor. The step's business
    /// may still replace entries (its claim digest) before hashing them.
    pub digests: Vec<C::Base>,
}

/// Prepares the fold frame of step `step` of `side`: folds the other side's last
/// instance into the accumulator in `side.slot` (or starts it), as the circuit
/// will. `digests` is the side's previous digest list; its folded slot must be
/// `kept`'s digest (zero while absent). `start_passthrough` is the passthrough
/// of an instance without predecessor: the other side's base hash, the same
/// constant the circuit holds.
///
/// # Errors
/// As [`fold_or_start`]; [`Error::Shape`] for an instance of another shape, a
/// digest list of another length or a folded slot that is not `kept`'s digest.
#[allow(clippy::too_many_arguments)]
pub fn prepare_step<C: PastaCurve>(
    side: &ChainSide,
    step: u64,
    other: &FoldedRelation<C::ScalarExt>,
    kept: Option<&Kept<C>>,
    their_last: Option<&Fresh<C>>,
    state: Vec<C::Base>,
    digests: Vec<C::Base>,
    start_passthrough: C::Base,
) -> Result<PreparedStep<C>, Error> {
    let zero_base = <C::Base as Field>::ZERO;
    if !side.is_valid() || digests.len() != side.digests {
        return Err(Error::Shape("digest list"));
    }
    let stored = kept.map_or(zero_base, |(acc, _)| acc.digest(&side.fold_domain));
    if digests[side.slot] != stored {
        return Err(Error::Shape("kept digest"));
    }
    let acc = kept.map_or_else(|| placeholder_accumulator::<C>(other), |(acc, _)| acc.clone());
    let Some((fresh, fresh_witness)) = their_last else {
        let zero = <C::ScalarExt as Field>::ZERO;
        let proof = FoldProof { f: vec![zero; other.index_bits()], k: vec![zero; other.degree() - 1] };
        let witness =
            FrameWitness { step, state, digests, slot: side.slot, acc, fresh: placeholder_instance::<C>(other), proof };
        return Ok(PreparedStep {
            witness,
            kept: kept.cloned(),
            bridge: vec![zero_base; other.instance_lengths()[0].saturating_sub(2)],
            passthrough: start_passthrough,
            digests: vec![zero_base; side.digests],
        });
    };
    let public = fresh.public.first().ok_or(Error::Shape("passthrough column"))?;
    if public.len() < 2 {
        return Err(Error::Shape("passthrough column"));
    }
    let (folded, folded_witness, proof) =
        fold_or_start(other, &side.fold_domain, kept.map(|(a, w)| (a, w)), fresh, fresh_witness)?;
    let mut next = digests.clone();
    next[side.slot] = folded.digest(&side.fold_domain);
    let passthrough = embed(&public[1]);
    let bridge = public[2..].iter().map(embed).collect();
    let witness = FrameWitness { step, state, digests, slot: side.slot, acc, fresh: fresh.clone(), proof };
    Ok(PreparedStep { witness, kept: Some((folded, folded_witness)), bridge, passthrough, digests: next })
}

/// The public inputs a step outputs: `[passthrough, h_out, bridge...]` with
/// `h_out = ivc_hash(step, base, next, digests)`.
#[must_use]
pub fn step_public<F: PastaField + iroha_pasta::poseidon::PoseidonField>(
    side: &ChainSide,
    step: u64,
    base: F,
    passthrough: F,
    next: &[F],
    digests: &[F],
    bridge: &[F],
) -> Vec<F> {
    let mut out = vec![passthrough, ivc_hash(&side.ivc_domain, step, base, next, digests)];
    out.extend_from_slice(bridge);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::{Fp, Fq};

    #[test]
    fn truncated_hashes_embed_exactly() {
        let h = ivc_hash::<Fq>(&[7; 32], 3, Fq::from(5u64), &[Fq::from(9u64)], &[Fq::ZERO]);
        assert_eq!(h.to_canonical_limbs()[3] >> (HASH_BITS - 192), 0);
        let in_fp: Fp = embed(&h);
        assert_eq!(in_fp.to_canonical_limbs(), h.to_canonical_limbs());
        assert_eq!(embed::<Fp, Fq>(&in_fp), h);
        assert_ne!(h, ivc_hash::<Fq>(&[7; 32], 4, Fq::from(5u64), &[Fq::from(9u64)], &[Fq::ZERO]));
    }

    #[test]
    fn every_digest_of_the_list_enters_the_hash_in_order() {
        let hash = |digests: &[Fp]| ivc_hash::<Fp>(&[3; 32], 2, Fp::from(5u64), &[Fp::from(9u64)], digests);
        let (a, b) = (Fp::from(11u64), Fp::from(13u64));
        assert_ne!(hash(&[a, b]), hash(&[b, a]));
        assert_ne!(hash(&[a, b]), hash(&[a, b + Fp::ONE]));
        assert_ne!(hash(&[a]), hash(&[a, Fp::ZERO]));
    }
}
