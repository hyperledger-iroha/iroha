//! Inner-Product Argument (IPA) for polynomial commitments.
//!
//! This module implements a standard IPA proof for inner products over a prime
//! field with multiplicative group commitments. It follows the Bootle et al.
//! style L/R reduction with transcript-derived challenges.

use core::marker::PhantomData;

use crate::{
    IpaGroup, IpaScalar, backend::IpaBackend, constants::DST, errors::Error, hash::sha3_256,
    params::Params, transcript::Transcript,
};

/// Computes the inner product <a, b> in the prime field.
fn inner_product<B: IpaBackend>(a: &[B::Scalar], b: &[B::Scalar]) -> B::Scalar {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = B::Scalar::default();
    for (ai, bi) in a.iter().zip(b.iter()) {
        acc = acc.add(ai.mul(*bi));
    }
    acc
}

/// Commits to a vector `v` using the provided generator vector `g`.
pub(crate) fn commit_vec<B: IpaBackend>(
    g: &[B::Group],
    v: &[B::Scalar],
) -> Result<B::Group, Error> {
    B::msm(g, v)
}

/// An IPA proof for <a, b> binding into L/R rounds and final scalars.
#[derive(Clone, Debug)]
pub struct IpaProof<B: IpaBackend> {
    /// L commitments, one per round.
    pub l_vec: Vec<B::Group>,
    /// R commitments, one per round.
    pub r_vec: Vec<B::Group>,
    /// Final scalar after reductions for `a`.
    pub a_final: B::Scalar,
    /// Final scalar after reductions for `b`.
    pub b_final: B::Scalar,
}

impl<B: IpaBackend> IpaProof<B> {
    /// Number of rounds in the proof.
    pub fn rounds(&self) -> usize {
        self.l_vec.len()
    }
}

/// Transcript projection for one verifier-side IPA reduction round.
///
/// These values are useful as deterministic recursive-verifier witnesses: the
/// native verifier and any future in-circuit verifier must agree on the
/// transcript state before/after absorbing `L || R`, the derived challenge, and
/// its inverse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaRoundChallenge<S: IpaScalar> {
    /// Zero-based IPA round index.
    pub round_index: usize,
    /// Canonical compressed bytes of this round's `L` commitment.
    pub l_bytes: [u8; 32],
    /// Canonical compressed bytes of this round's `R` commitment.
    pub r_bytes: [u8; 32],
    /// Domain-separated digest of `round_index || L || R`.
    pub round_bytes_digest: [u8; 32],
    /// Transcript state before absorbing the round's `L || R` bytes.
    pub state_before_round: [u8; 32],
    /// Transcript state after absorbing `L || R` and before deriving `ipa.x`.
    pub state_after_round_absorb: [u8; 32],
    /// Transcript state after deriving `ipa.x`.
    pub state_after_challenge: [u8; 32],
    /// Fiat-Shamir challenge scalar for this round.
    pub challenge: S,
    /// Multiplicative inverse of `challenge`.
    pub challenge_inverse: S,
}

/// Public transcript projection for one verifier-side IPA proof.
///
/// The projection records the exact transcript state boundary around
/// `ipa.n`, every `L || R` round byte pair, every derived challenge, and the
/// final transcript state. Recursive verifier public inputs can commit to this
/// projection while the native verifier recomputes it from the proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierTranscriptProjection<S: IpaScalar> {
    /// Public vector length absorbed by the IPA verifier.
    pub n: usize,
    /// Transcript state after the polynomial-opening statement and before
    /// absorbing `ipa.n`.
    pub state_before_ipa_n: [u8; 32],
    /// Transcript state after absorbing `ipa.n` and before the first round.
    pub state_after_ipa_n: [u8; 32],
    /// Per-round transcript and `L/R` byte projections.
    pub rounds: Vec<IpaRoundChallenge<S>>,
    /// Transcript state after the last `ipa.x` challenge derivation.
    pub final_state: [u8; 32],
}

/// Field-friendly binding of one verifier transcript projection.
///
/// The native verifier still validates the exact SHA3 transcript projection.
/// This structure additionally projects that byte transcript into scalar field
/// elements and folds them with a transparent Pow5 accumulator so recursive
/// verifier circuits can bind their public challenge inputs to the host-checked
/// transcript without implementing SHA3 inside the circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierTranscriptBinding<S: IpaScalar> {
    /// Public vector length absorbed by the IPA verifier.
    pub n: usize,
    /// Scalar projection of the transcript header and `ipa.n` state boundary.
    pub header_projection: S,
    /// Scalar projections of each round's bytes, states, challenge, and inverse.
    pub round_projections: Vec<S>,
    /// Per-round Fiat-Shamir challenges copied from the verified projection.
    pub challenges: Vec<S>,
    /// Per-round challenge inverses copied from the verified projection.
    pub challenge_inverses: Vec<S>,
    /// Scalar projection of the final transcript state.
    pub final_projection: S,
    /// Pow5 accumulator binding all scalar projections and challenges.
    pub binding_digest: S,
}

/// Native verifier-side IPA accumulation state for one reduction round.
///
/// The round records the scalar squares used in `Q' = L^{x^2} * Q *
/// R^{x^{-2}}`, the accumulator before and after that update, and the folded
/// generator vectors after applying the same `x/x^{-1}` powers as the verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierAccumulationRound<G: IpaGroup> {
    /// Zero-based IPA round index.
    pub round_index: usize,
    /// `x^2` for the round challenge.
    pub challenge_square: G::Scalar,
    /// `x^{-2}` for the round challenge.
    pub challenge_inverse_square: G::Scalar,
    /// Verifier accumulator before absorbing this round's `L/R` contribution.
    pub q_before: G,
    /// Verifier accumulator after absorbing this round's `L/R` contribution.
    pub q_after: G,
    /// Folded `g` generator vector after this round.
    pub g_after: Vec<G>,
    /// Folded `h` generator vector after this round.
    pub h_after: Vec<G>,
}

/// Native verifier-side `b`-vector reduction state for one IPA round.
///
/// The round records the public-vector layer before and after applying the
/// transcript challenge relation `b'_i = b_i*x^{-1} + b_{i+half}*x`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierBVectorReductionRound<S: IpaScalar> {
    /// Zero-based IPA round index.
    pub round_index: usize,
    /// Fiat-Shamir challenge scalar for this round.
    pub challenge: S,
    /// Multiplicative inverse of `challenge`.
    pub challenge_inverse: S,
    /// Vector layer before this reduction round.
    pub b_before: Vec<S>,
    /// Vector layer after this reduction round.
    pub b_after: Vec<S>,
}

/// Native verifier-side public `b`-vector reduction projection.
///
/// This is the scalar-side companion to [`IpaVerifierAccumulation`]. It gives a
/// recursive verifier witness a deterministic path from the public opening
/// vector `b` to the proof's final scalar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierBVectorReduction<S: IpaScalar> {
    /// Initial public `b` vector from the polynomial-opening statement.
    pub initial_b: Vec<S>,
    /// Per-round folded `b` vector states.
    pub rounds: Vec<IpaVerifierBVectorReductionRound<S>>,
    /// Final folded scalar expected to match `proof.b_final`.
    pub final_b: S,
}

/// Native verifier-side IPA accumulation projection.
///
/// This is a deterministic witness layout for future recursive verification:
/// it exposes the initial `Q`, every round's folded group state, the final
/// folded generators, and the final expected term compared by the ordinary IPA
/// verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierAccumulation<G: IpaGroup> {
    /// Initial verifier accumulator `P * H(b) * U^t`.
    pub initial_q: G,
    /// Per-round accumulation and generator-folding states.
    pub rounds: Vec<IpaVerifierAccumulationRound<G>>,
    /// Final verifier accumulator after all `L/R` rounds.
    pub final_q: G,
    /// Final folded `g` generator.
    pub final_g: G,
    /// Final folded `h` generator.
    pub final_h: G,
    /// Final expected term `g^a * h^b * u^{a*b}`.
    pub expected_term: G,
}

/// Combined native verifier-side IPA witness for recursive verification.
///
/// This bundles the transcript challenge projection, public `b`-vector
/// reduction, and group-accumulation projection for one proof. It is the
/// canonical host-side witness shape intended for future in-circuit verifier
/// assembly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpaVerifierWitness<B: IpaBackend> {
    /// Public transcript projection for this proof.
    pub transcript_projection: IpaVerifierTranscriptProjection<B::Scalar>,
    /// Field-friendly binding of the verified transcript projection.
    pub transcript_binding: IpaVerifierTranscriptBinding<B::Scalar>,
    /// Per-round transcript states and Fiat-Shamir challenges.
    pub round_challenges: Vec<IpaRoundChallenge<B::Scalar>>,
    /// Public `b`-vector reduction down to `proof.b_final`.
    pub b_reduction: IpaVerifierBVectorReduction<B::Scalar>,
    /// Group accumulator and generator-fold projection.
    pub accumulation: IpaVerifierAccumulation<B::Group>,
    /// Final scalar `a` from the IPA proof.
    pub proof_a_final: B::Scalar,
    /// Final scalar `b` from the IPA proof.
    pub proof_b_final: B::Scalar,
}

fn ipa_round_bytes_digest(round_index: usize, l_bytes: [u8; 32], r_bytes: [u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(DST.len() + 8 + 32 + 32 + 16);
    buf.extend_from_slice(DST.as_bytes());
    buf.push(4u8);
    buf.extend_from_slice(b"ipa.round.bytes");
    buf.extend_from_slice(&(round_index as u64).to_le_bytes());
    buf.extend_from_slice(&l_bytes);
    buf.extend_from_slice(&r_bytes);
    sha3_256(&buf)
}

fn scalar_from_u64<S: IpaScalar>(value: u64) -> S {
    let mut out = S::zero();
    for _ in 0..value {
        out = out.add(S::one());
    }
    out
}

fn scalar_pow5<S: IpaScalar>(value: S) -> S {
    let square = value.mul(value);
    square.mul(square).mul(value)
}

/// Transparent field compressor used by the transcript-binding accumulator.
///
/// This is deliberately small and circuit-friendly: recursive verifier circuits
/// can enforce it with quadratic constraints by witnessing the intermediate
/// squares. It is not a replacement for the native SHA3 transcript; it binds
/// scalar projections of a SHA3-validated transcript into the recursive public
/// input surface.
pub fn ipa_transcript_binding_compress<S: IpaScalar>(left: S, right: S) -> S {
    let left_shifted = left.add(scalar_from_u64::<S>(7));
    let right_shifted = right.add(scalar_from_u64::<S>(13));
    scalar_from_u64::<S>(2)
        .mul(scalar_pow5(left_shifted))
        .add(scalar_from_u64::<S>(3).mul(scalar_pow5(right_shifted)))
}

/// Fold one round projection and challenge pair into a transcript-binding state.
pub fn ipa_transcript_binding_round<S: IpaScalar>(
    state: S,
    round_projection: S,
    challenge: S,
    challenge_inverse: S,
) -> S {
    let after_round = ipa_transcript_binding_compress(state, round_projection);
    let after_challenge = ipa_transcript_binding_compress(after_round, challenge);
    ipa_transcript_binding_compress(after_challenge, challenge_inverse)
}

fn projection_scalar<S: IpaScalar>(label: &[u8], body: &[u8]) -> S {
    let mut buf = Vec::with_capacity(DST.len() + label.len() + body.len() + 24);
    buf.extend_from_slice(DST.as_bytes());
    buf.push(5u8);
    buf.extend_from_slice(&(label.len() as u64).to_le_bytes());
    buf.extend_from_slice(label);
    buf.extend_from_slice(&(body.len() as u64).to_le_bytes());
    buf.extend_from_slice(body);
    let wide = crate::hash::sha3_512(&buf);
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(&wide);
    S::from_uniform(&bytes)
}

fn transcript_header_projection_scalar<S: IpaScalar>(
    projection: &IpaVerifierTranscriptProjection<S>,
) -> S {
    let mut body = Vec::with_capacity(8 + 32 + 32);
    body.extend_from_slice(&(projection.n as u64).to_le_bytes());
    body.extend_from_slice(&projection.state_before_ipa_n);
    body.extend_from_slice(&projection.state_after_ipa_n);
    projection_scalar(b"ipa.transcript.binding.header", &body)
}

fn transcript_round_projection_scalar<S: IpaScalar>(round: &IpaRoundChallenge<S>) -> S {
    let mut body = Vec::with_capacity(8 + 32 * 8);
    body.extend_from_slice(&(round.round_index as u64).to_le_bytes());
    body.extend_from_slice(&round.l_bytes);
    body.extend_from_slice(&round.r_bytes);
    body.extend_from_slice(&round.round_bytes_digest);
    body.extend_from_slice(&round.state_before_round);
    body.extend_from_slice(&round.state_after_round_absorb);
    body.extend_from_slice(&round.state_after_challenge);
    body.extend_from_slice(&round.challenge.to_bytes());
    body.extend_from_slice(&round.challenge_inverse.to_bytes());
    projection_scalar(b"ipa.transcript.binding.round", &body)
}

fn transcript_final_projection_scalar<S: IpaScalar>(
    projection: &IpaVerifierTranscriptProjection<S>,
) -> S {
    projection_scalar(b"ipa.transcript.binding.final", &projection.final_state)
}

/// Derive a field-friendly binding from a verified IPA transcript projection.
///
/// # Errors
///
/// Returns an error if the projection shape is inconsistent or if any
/// challenge/inverse pair does not multiply to one.
pub fn derive_ipa_verifier_transcript_binding<S: IpaScalar>(
    projection: &IpaVerifierTranscriptProjection<S>,
) -> Result<IpaVerifierTranscriptBinding<S>, Error> {
    if projection.n == 0 || (projection.n & (projection.n - 1)) != 0 {
        return Err(Error::InvalidN(projection.n));
    }
    let expected_rounds = projection.n.trailing_zeros() as usize;
    if projection.rounds.len() != expected_rounds {
        return Err(Error::InvalidProofShape {
            reason: "transcript binding round count",
            expected: expected_rounds,
            actual: projection.rounds.len(),
        });
    }

    let header_projection = transcript_header_projection_scalar(projection);
    let final_projection = transcript_final_projection_scalar(projection);
    let mut round_projections = Vec::with_capacity(expected_rounds);
    let mut challenges = Vec::with_capacity(expected_rounds);
    let mut challenge_inverses = Vec::with_capacity(expected_rounds);
    let mut state = header_projection;
    for (round_index, round) in projection.rounds.iter().enumerate() {
        if round.round_index != round_index {
            return Err(Error::InvalidProofShape {
                reason: "transcript binding round index",
                expected: round_index,
                actual: round.round_index,
            });
        }
        if round.challenge.mul(round.challenge_inverse) != S::one() {
            return Err(Error::VerificationFailed);
        }
        let round_projection = transcript_round_projection_scalar(round);
        state = ipa_transcript_binding_round(
            state,
            round_projection,
            round.challenge,
            round.challenge_inverse,
        );
        round_projections.push(round_projection);
        challenges.push(round.challenge);
        challenge_inverses.push(round.challenge_inverse);
    }
    let binding_digest = ipa_transcript_binding_compress(state, final_projection);

    Ok(IpaVerifierTranscriptBinding {
        n: projection.n,
        header_projection,
        round_projections,
        challenges,
        challenge_inverses,
        final_projection,
        binding_digest,
    })
}

/// Validate a supplied field-friendly binding against a transcript projection.
///
/// # Errors
///
/// Returns an error if the deterministic binding derived from `projection`
/// differs from `binding`.
pub fn validate_ipa_verifier_transcript_binding<S: IpaScalar>(
    projection: &IpaVerifierTranscriptProjection<S>,
    binding: &IpaVerifierTranscriptBinding<S>,
) -> Result<(), Error> {
    let expected = derive_ipa_verifier_transcript_binding(projection)?;
    if &expected == binding {
        Ok(())
    } else {
        Err(Error::VerificationFailed)
    }
}

/// Derive the verifier-side IPA transcript projection from the current transcript.
///
/// The caller must have already absorbed the polynomial-opening statement into
/// `transcript`. This helper applies the IPA-layer `ipa.n` absorb, records the
/// `ipa.n` boundary, then records each round's `L || R` bytes, `ipa.round`
/// absorb state, and `ipa.x` challenge derivation exactly as the verifier does.
/// It mutates `transcript` to the post-round final state.
///
/// # Errors
///
/// Returns an error if `n` is not compatible with the proof shape or if any
/// challenge is zero and cannot be inverted.
pub fn derive_ipa_verifier_transcript_projection<B: IpaBackend>(
    n: usize,
    transcript: &mut Transcript,
    proof: &IpaProof<B>,
) -> Result<IpaVerifierTranscriptProjection<B::Scalar>, Error> {
    if n == 0 || (n & (n - 1)) != 0 {
        return Err(Error::InvalidN(n));
    }
    if proof.l_vec.len() != proof.r_vec.len() {
        return Err(Error::InvalidProofShape {
            reason: "L/R round count",
            expected: proof.l_vec.len(),
            actual: proof.r_vec.len(),
        });
    }
    let expected_rounds = n.trailing_zeros() as usize;
    if proof.rounds() != expected_rounds {
        return Err(Error::InvalidProofShape {
            reason: "round count",
            expected: expected_rounds,
            actual: proof.rounds(),
        });
    }

    let state_before_ipa_n = transcript.cur_digest();
    transcript.absorb("ipa.n", &(n as u64).to_le_bytes());
    let state_after_ipa_n = transcript.cur_digest();

    let mut rounds = Vec::with_capacity(proof.rounds());
    for (round_index, (&l, &r)) in proof.l_vec.iter().zip(proof.r_vec.iter()).enumerate() {
        let state_before_round = transcript.cur_digest();
        let l_bytes = l.to_bytes();
        let r_bytes = r.to_bytes();
        let round_bytes_digest = ipa_round_bytes_digest(round_index, l_bytes, r_bytes);
        let mut lr_bytes = Vec::with_capacity(64);
        lr_bytes.extend_from_slice(&l_bytes);
        lr_bytes.extend_from_slice(&r_bytes);
        transcript.absorb("ipa.round", &lr_bytes);
        let state_after_round_absorb = transcript.cur_digest();
        let challenge = transcript.challenge_scalar::<B::Scalar>("ipa.x");
        let challenge_inverse = challenge.inv()?;
        let state_after_challenge = transcript.cur_digest();
        rounds.push(IpaRoundChallenge {
            round_index,
            l_bytes,
            r_bytes,
            round_bytes_digest,
            state_before_round,
            state_after_round_absorb,
            state_after_challenge,
            challenge,
            challenge_inverse,
        });
    }
    let final_state = transcript.cur_digest();
    Ok(IpaVerifierTranscriptProjection {
        n,
        state_before_ipa_n,
        state_after_ipa_n,
        rounds,
        final_state,
    })
}

/// Derive the verifier-side IPA round challenges from the current transcript.
///
/// The caller must have already absorbed the polynomial-opening statement into
/// `transcript`. This helper returns the per-round portion of
/// [`derive_ipa_verifier_transcript_projection`].
///
/// # Errors
///
/// Returns an error if `n` is not compatible with the proof shape or if any
/// challenge is zero and cannot be inverted.
pub fn derive_ipa_verifier_round_challenges<B: IpaBackend>(
    n: usize,
    transcript: &mut Transcript,
    proof: &IpaProof<B>,
) -> Result<Vec<IpaRoundChallenge<B::Scalar>>, Error> {
    Ok(derive_ipa_verifier_transcript_projection::<B>(n, transcript, proof)?.rounds)
}

/// Validate a supplied transcript projection against the native IPA verifier.
///
/// The caller must have already absorbed the polynomial-opening statement into
/// `transcript`; this function mutates it exactly as the verifier would.
///
/// # Errors
///
/// Returns an error when the proof shape is invalid or when any transcript
/// state, round byte digest, challenge, inverse, or final state differs from
/// the deterministic native verifier projection.
pub fn validate_ipa_verifier_transcript_projection<B: IpaBackend>(
    n: usize,
    transcript: &mut Transcript,
    proof: &IpaProof<B>,
    projection: &IpaVerifierTranscriptProjection<B::Scalar>,
) -> Result<(), Error> {
    let expected = derive_ipa_verifier_transcript_projection::<B>(n, transcript, proof)?;
    if &expected == projection {
        Ok(())
    } else {
        Err(Error::VerificationFailed)
    }
}

/// Project the verifier-side IPA public `b`-vector reduction.
///
/// `round_challenges` must come from
/// [`derive_ipa_verifier_round_challenges`] for the same statement and proof.
/// The returned `final_b` is the value that must match `proof.b_final` in the
/// final IPA comparison.
///
/// # Errors
///
/// Returns an error if `b` is not a non-zero power-of-two vector, if the
/// challenge sequence has the wrong shape, if any round index is inconsistent,
/// or if a supplied challenge/inverse pair is not multiplicative inverse.
pub fn derive_ipa_verifier_b_vector_reduction<S: IpaScalar>(
    b: &[S],
    round_challenges: &[IpaRoundChallenge<S>],
) -> Result<IpaVerifierBVectorReduction<S>, Error> {
    let n = b.len();
    if n == 0 || (n & (n - 1)) != 0 {
        return Err(Error::InvalidN(n));
    }
    let expected_rounds = n.trailing_zeros() as usize;
    if round_challenges.len() != expected_rounds {
        return Err(Error::InvalidProofShape {
            reason: "round challenge count",
            expected: expected_rounds,
            actual: round_challenges.len(),
        });
    }

    let initial_b = b.to_vec();
    let mut current = initial_b.clone();
    let mut rounds = Vec::with_capacity(expected_rounds);
    for (round_index, round) in round_challenges.iter().enumerate() {
        if round.round_index != round_index {
            return Err(Error::InvalidProofShape {
                reason: "b-vector round challenge index",
                expected: round_index,
                actual: round.round_index,
            });
        }
        if round.challenge.mul(round.challenge_inverse) != S::one() {
            return Err(Error::VerificationFailed);
        }
        let half = current.len() / 2;
        let b_before = current.clone();
        let mut b_after = Vec::with_capacity(half);
        for index in 0..half {
            b_after.push(
                current[index]
                    .mul(round.challenge_inverse)
                    .add(current[half + index].mul(round.challenge)),
            );
        }
        rounds.push(IpaVerifierBVectorReductionRound {
            round_index,
            challenge: round.challenge,
            challenge_inverse: round.challenge_inverse,
            b_before,
            b_after: b_after.clone(),
        });
        current = b_after;
    }

    debug_assert_eq!(current.len(), 1);
    Ok(IpaVerifierBVectorReduction {
        initial_b,
        rounds,
        final_b: current[0],
    })
}

/// Project the verifier-side IPA scalar-multiplication accumulation.
///
/// `round_challenges` must come from
/// [`derive_ipa_verifier_round_challenges`] for the same statement and proof.
/// The function does not mutate a transcript; it deterministically folds `Q`,
/// `g`, and `h` exactly as the verifier does and returns the final comparison
/// term.
///
/// # Errors
///
/// Returns an error if the public vector, proof, or supplied challenge sequence
/// has the wrong shape.
pub fn derive_ipa_verifier_accumulation<B: IpaBackend>(
    params: &Params<B>,
    b: &[B::Scalar],
    p_g: B::Group,
    t: B::Scalar,
    proof: &IpaProof<B>,
    round_challenges: &[IpaRoundChallenge<B::Scalar>],
) -> Result<IpaVerifierAccumulation<B::Group>, Error> {
    let n = params.n();
    if b.len() != n {
        return Err(Error::DimensionMismatch {
            expected: n,
            actual: b.len(),
        });
    }
    if proof.l_vec.len() != proof.r_vec.len() {
        return Err(Error::InvalidProofShape {
            reason: "L/R round count",
            expected: proof.l_vec.len(),
            actual: proof.r_vec.len(),
        });
    }
    let expected_rounds = n.trailing_zeros() as usize;
    if proof.rounds() != expected_rounds {
        return Err(Error::InvalidProofShape {
            reason: "round count",
            expected: expected_rounds,
            actual: proof.rounds(),
        });
    }
    if round_challenges.len() != expected_rounds {
        return Err(Error::InvalidProofShape {
            reason: "round challenge count",
            expected: expected_rounds,
            actual: round_challenges.len(),
        });
    }

    let hb = commit_vec::<B>(params.h(), b)?;
    let ut = params.u().pow(t);
    let mut q = p_g.mul(hb).mul(ut);
    let initial_q = q;

    let mut g_vec = params.g().to_vec();
    let mut h_vec = params.h().to_vec();
    let mut rounds = Vec::with_capacity(expected_rounds);

    for (round_index, ((&l, &r), round)) in proof
        .l_vec
        .iter()
        .zip(proof.r_vec.iter())
        .zip(round_challenges.iter())
        .enumerate()
    {
        if round.round_index != round_index {
            return Err(Error::InvalidProofShape {
                reason: "round challenge index",
                expected: round_index,
                actual: round.round_index,
            });
        }
        let x = round.challenge;
        let x_inv = round.challenge_inverse;
        if x.mul(x_inv) != B::Scalar::one() {
            return Err(Error::VerificationFailed);
        }
        let x2 = x.mul(x);
        let x2_inv = x_inv.mul(x_inv);
        let q_before = q;
        q = l.pow(x2).mul(q).mul(r.pow(x2_inv));

        let m = g_vec.len();
        let half = m / 2;
        let (g_l, g_r) = g_vec.split_at(half);
        let (h_l, h_r) = h_vec.split_at(half);

        let mut g_new = Vec::with_capacity(half);
        for i in 0..half {
            g_new.push(g_l[i].pow(x_inv).mul(g_r[i].pow(x)));
        }
        let mut h_new = Vec::with_capacity(half);
        for i in 0..half {
            h_new.push(h_l[i].pow(x).mul(h_r[i].pow(x_inv)));
        }

        rounds.push(IpaVerifierAccumulationRound {
            round_index,
            challenge_square: x2,
            challenge_inverse_square: x2_inv,
            q_before,
            q_after: q,
            g_after: g_new.clone(),
            h_after: h_new.clone(),
        });
        g_vec = g_new;
        h_vec = h_new;
    }

    debug_assert_eq!(g_vec.len(), 1);
    debug_assert_eq!(h_vec.len(), 1);
    let final_g = g_vec[0];
    let final_h = h_vec[0];
    let a = proof.a_final;
    let b_fin = proof.b_final;
    let expected_term = final_g
        .pow(a)
        .mul(final_h.pow(b_fin))
        .mul(params.u().pow(a.mul(b_fin)));

    Ok(IpaVerifierAccumulation {
        initial_q,
        rounds,
        final_q: q,
        final_g,
        final_h,
        expected_term,
    })
}

/// Derive the complete native verifier-side witness for one IPA proof.
///
/// The caller must have already absorbed the polynomial-opening statement into
/// `transcript`. The function mutates `transcript` exactly as the verifier does
/// and returns the transcript, scalar-reduction, and group-accumulation witness
/// projections used by recursive verification.
///
/// # Errors
///
/// Returns an error if the proof shape is invalid, if `proof.b_final` is not the
/// transcript-derived fold of the public `b` vector, or if any projected
/// sub-witness cannot be derived.
pub fn derive_ipa_verifier_witness<B: IpaBackend>(
    params: &Params<B>,
    transcript: &mut Transcript,
    b: &[B::Scalar],
    p_g: B::Group,
    t: B::Scalar,
    proof: &IpaProof<B>,
) -> Result<IpaVerifierWitness<B>, Error> {
    let transcript_projection =
        derive_ipa_verifier_transcript_projection::<B>(params.n(), transcript, proof)?;
    let transcript_binding = derive_ipa_verifier_transcript_binding(&transcript_projection)?;
    let round_challenges = transcript_projection.rounds.clone();
    let b_reduction = derive_ipa_verifier_b_vector_reduction(b, &round_challenges)?;
    if b_reduction.final_b != proof.b_final {
        return Err(Error::VerificationFailed);
    }
    let accumulation =
        derive_ipa_verifier_accumulation::<B>(params, b, p_g, t, proof, &round_challenges)?;
    Ok(IpaVerifierWitness {
        transcript_projection,
        transcript_binding,
        round_challenges,
        b_reduction,
        accumulation,
        proof_a_final: proof.a_final,
        proof_b_final: proof.b_final,
    })
}

/// Validate a supplied recursive-verifier IPA witness against the native verifier.
///
/// The caller must have already absorbed the polynomial-opening statement into
/// `transcript`. This recomputes the expected witness from the proof and public
/// statement, compares it with `witness`, and enforces the final verifier group
/// equality.
///
/// # Errors
///
/// Returns an error if the supplied witness differs from the native verifier's
/// deterministic projection or if the projected final group comparison fails.
pub fn validate_ipa_verifier_witness<B: IpaBackend>(
    params: &Params<B>,
    transcript: &mut Transcript,
    b: &[B::Scalar],
    p_g: B::Group,
    t: B::Scalar,
    proof: &IpaProof<B>,
    witness: &IpaVerifierWitness<B>,
) -> Result<(), Error> {
    let expected = derive_ipa_verifier_witness::<B>(params, transcript, b, p_g, t, proof)?;
    if witness.transcript_projection != expected.transcript_projection
        || witness.transcript_binding != expected.transcript_binding
        || witness.round_challenges != expected.round_challenges
        || witness.b_reduction != expected.b_reduction
        || witness.accumulation != expected.accumulation
        || witness.proof_a_final != expected.proof_a_final
        || witness.proof_b_final != expected.proof_b_final
    {
        return Err(Error::VerificationFailed);
    }
    if witness.accumulation.final_q == witness.accumulation.expected_term {
        Ok(())
    } else {
        Err(Error::VerificationFailed)
    }
}

/// Prover for the IPA opening.
pub struct IpaProver<B: IpaBackend>(PhantomData<B>);

impl<B: IpaBackend> IpaProver<B> {
    /// Creates an IPA proof that a committed vector `a` has inner product `t`
    /// with the public vector `b` under parameters `params` and transcript.
    pub fn prove(
        params: &Params<B>,
        transcript: &mut Transcript,
        a: &[B::Scalar],
        b: &[B::Scalar],
        p_g: B::Group,
        t: B::Scalar,
    ) -> Result<IpaProof<B>, Error> {
        let n = params.n();
        if a.len() != n || b.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: a.len().max(b.len()),
            });
        }
        // Bind public inputs
        transcript.absorb("ipa.n", &(n as u64).to_le_bytes());
        // Construct Q = g^a · h^b · u^{<a,b>} where g^a is provided as `p_g`.
        let hb = commit_vec::<B>(params.h(), b)?;
        let ut = params.u().pow(t);
        let mut q = p_g.mul(hb).mul(ut);

        let mut a_vec = a.to_vec();
        let mut b_vec = b.to_vec();
        let mut g_vec = params.g().to_vec();
        let mut h_vec = params.h().to_vec();

        let mut l_vec = Vec::new();
        let mut r_vec = Vec::new();

        while a_vec.len() > 1 {
            let m = a_vec.len();
            debug_assert_eq!(m & (m - 1), 0, "vector length must stay power-of-two");
            let half = m / 2;

            let (a_l, a_r) = a_vec.split_at(half);
            let (b_l, b_r) = b_vec.split_at(half);
            let (g_l, g_r) = g_vec.split_at(half);
            let (h_l, h_r) = h_vec.split_at(half);

            let c_l = inner_product::<B>(a_l, b_r);
            let c_r = inner_product::<B>(a_r, b_l);

            // L = g_R^{a_L} · h_L^{b_R} · u^{c_l}
            let l = commit_vec::<B>(g_r, a_l)?
                .mul(commit_vec::<B>(h_l, b_r)?)
                .mul(params.u().pow(c_l));
            // R = g_L^{a_R} · h_R^{b_L} · u^{c_r}
            let r = commit_vec::<B>(g_l, a_r)?
                .mul(commit_vec::<B>(h_r, b_l)?)
                .mul(params.u().pow(c_r));

            // Absorb and derive challenge
            let mut lr_bytes = Vec::with_capacity(64);
            lr_bytes.extend_from_slice(&l.to_bytes());
            lr_bytes.extend_from_slice(&r.to_bytes());
            transcript.absorb("ipa.round", &lr_bytes);
            let x = transcript.challenge_scalar::<B::Scalar>("ipa.x");
            let x_inv = x.inv()?;

            // Fold vectors: a' = a_L*x + a_R*x^{-1}
            //               b' = b_L*x^{-1} + b_R*x
            let mut a_new = Vec::with_capacity(half);
            let mut b_new = Vec::with_capacity(half);
            for i in 0..half {
                a_new.push(a_l[i].mul(x).add(a_r[i].mul(x_inv)));
                b_new.push(b_l[i].mul(x_inv).add(b_r[i].mul(x)));
            }

            // Fold generators: g' = g_L^{x^{-1}} || g_R^{x}
            //                  h' = h_L^{x}     || h_R^{x^{-1}}
            let mut g_tmp = Vec::with_capacity(half * 2);
            let mut h_tmp = Vec::with_capacity(half * 2);
            for i in 0..half {
                g_tmp.push(g_l[i].pow(x_inv));
                g_tmp.push(g_r[i].pow(x));
            }
            let g_new = g_tmp
                .chunks_exact(2)
                .map(|pair| pair[0].mul(pair[1]))
                .collect::<Vec<_>>();

            for i in 0..half {
                h_tmp.push(h_l[i].pow(x));
                h_tmp.push(h_r[i].pow(x_inv));
            }
            let h_new = h_tmp
                .chunks_exact(2)
                .map(|pair| pair[0].mul(pair[1]))
                .collect::<Vec<_>>();

            // Update Q: Q' = L^{x^2} · Q · R^{x^{-2}}
            let x2 = x.mul(x);
            let x2_inv = x_inv.mul(x_inv);
            q = l.pow(x2).mul(q).mul(r.pow(x2_inv));

            l_vec.push(l);
            r_vec.push(r);

            a_vec = a_new;
            b_vec = b_new;
            g_vec = g_new;
            h_vec = h_new;
        }

        debug_assert_eq!(a_vec.len(), 1);
        debug_assert_eq!(b_vec.len(), 1);
        let a_final = a_vec[0];
        let b_final = b_vec[0];

        Ok(IpaProof {
            l_vec,
            r_vec,
            a_final,
            b_final,
        })
    }
}

/// Verifier for the IPA opening.
pub struct IpaVerifier<B: IpaBackend>(PhantomData<B>);

impl<B: IpaBackend> IpaVerifier<B> {
    /// Verifies an IPA proof that commitment `p_g` to `a` satisfies
    /// <a, b> == `t` for public vector `b`.
    pub fn verify(
        params: &Params<B>,
        transcript: &mut Transcript,
        b: &[B::Scalar],
        p_g: B::Group,
        t: B::Scalar,
        proof: &IpaProof<B>,
    ) -> Result<(), Error> {
        let n = params.n();
        if b.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: b.len(),
            });
        }
        if proof.l_vec.len() != proof.r_vec.len() {
            return Err(Error::InvalidProofShape {
                reason: "L/R round count",
                expected: proof.l_vec.len(),
                actual: proof.r_vec.len(),
            });
        }
        let expected_rounds = n.trailing_zeros() as usize;
        if proof.rounds() != expected_rounds {
            return Err(Error::InvalidProofShape {
                reason: "round count",
                expected: expected_rounds,
                actual: proof.rounds(),
            });
        }

        let witness = derive_ipa_verifier_witness::<B>(params, transcript, b, p_g, t, proof)?;

        if witness.accumulation.final_q == witness.accumulation.expected_term {
            Ok(())
        } else {
            Err(Error::VerificationFailed)
        }
    }
}
