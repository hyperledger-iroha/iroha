//! The wraps' native producer: key generation (S, then R, which pins S, then
//! P, which pins R, then Omega, which pins P), then for one finished chain
//! every decider, fold and restructuring natively and the four proofs in
//! order.
//!
//! Both deciders, the last A fold and both restructurings run natively
//! first, so S's witness is complete. S's proof gives R's claim for S, which
//! R forwards, then R's witness is complete. R's proof gives the claim
//! `acc_P` folds, then P's witness is complete; P's proof gives the last
//! claim `acc_V` folds, then Omega's.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    Accumulator,
    claims::{self, ClaimAccumulator},
    commit::Witness as FoldWitnessColumns,
    decider_v2::{self, DeciderV2Plan, Proved},
    ivc::{self, Fresh, Kept},
    transcript::s6_limbs,
};
use iroha_pasta::{
    Ep, Eq, Fp, Fq, PastaCurve, PastaField,
    poseidon::{PoseidonField, hash_with_domain},
};
use iroha_plonk::{
    ProverConfig, ProverRandomness, ProvingKey, Witness, create_proof_owned_with_claim,
    cs::InstanceType,
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::{
        ipa::{GeneratorClaim, PinnedParams},
        mle::{self, BatchOpening},
    },
    transcript::generator_coordinates,
};
use iroha_plonk_gadgets::statement::foreign_limbs;
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, FoldPurpose, FoldTape, FoldWitness, RestructureStatement,
    create_fold, create_restructure,
};

use super::{
    WrapError, WrapSpec,
    crossing::{
        AccumulatorTerms, ClaimRows, ForwardedClaim, PPublic, Pair, PallasClaimCrossing,
        Q_CROSSING_DOMAIN, QCrossing, QMember, RMember, RPublic, SPublic, VestaClaimCrossing,
    },
    omega::{
        INSTANCE_TYPES, OmegaCircuit, OmegaPlan, OmegaWitness, PallasFieldWitness, QFieldWitness,
    },
    p::{AFieldWitness, PCircuit, PPlan, PWitness, PallasClaimWitness, VestaClaimWitness},
    r::{RCircuit, RPlan, RWitness},
    s::{QGroupWitness, SCircuit, SPlan},
};
use crate::a_relation::LINEAGE_DOMAIN;

/// The four wraps' programs and proving keys.
pub struct WrapKeys {
    /// S's program.
    pub s_plan: Arc<SPlan>,
    /// S's proving key (Vesta).
    pub s_key: ProvingKey<Eq>,
    /// R's program.
    pub r_plan: Arc<RPlan>,
    /// R's proving key (Pallas).
    pub r_key: ProvingKey<Ep>,
    /// P's program.
    pub p_plan: Arc<PPlan>,
    /// P's proving key (Vesta).
    pub p_key: ProvingKey<Eq>,
    /// Omega's program.
    pub omega_plan: Arc<OmegaPlan>,
    /// Omega's proving key (Pallas).
    pub omega_key: ProvingKey<Ep>,
}

impl core::fmt::Debug for WrapKeys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WrapKeys").finish_non_exhaustive()
    }
}

/// Generates S's key, then R's (which pins S's), then P's (which pins R's),
/// then Omega's (which pins P's).
///
/// # Errors
/// [`WrapError::Input`] for relations the wraps do not admit,
/// [`WrapError::Proof`] when key generation fails.
pub fn keygen(spec: Arc<WrapSpec>) -> Result<WrapKeys, WrapError> {
    let s_plan = Arc::new(SPlan::new(Arc::clone(&spec))?);
    let s_key = keygen_pk_v2(
        &spec.params_a,
        &SCircuit {
            plan: Arc::clone(&s_plan),
            witness: None,
        },
        &KeygenConfigV2::pipa_r(vec![InstanceType::Field]),
    )
    .map_err(|_| WrapError::Proof)?;
    let r_plan = Arc::new(RPlan::new(
        Arc::clone(&s_plan),
        s_key.binding().clone(),
        s_key.vk().clone(),
    )?);
    let r_key = keygen_pk_v2(
        &spec.params_q,
        &RCircuit {
            plan: Arc::clone(&r_plan),
            witness: None,
        },
        &KeygenConfigV2::pipa_r(vec![InstanceType::Field]),
    )
    .map_err(|_| WrapError::Proof)?;
    let p_plan = Arc::new(PPlan::new(
        Arc::clone(&r_plan),
        r_key.binding().clone(),
        r_key.vk().clone(),
    )?);
    let p_key = keygen_pk_v2(
        &spec.params_a,
        &PCircuit {
            plan: Arc::clone(&p_plan),
            witness: None,
        },
        &KeygenConfigV2::pipa_r(vec![InstanceType::Field]),
    )
    .map_err(|_| WrapError::Proof)?;
    let omega_plan = Arc::new(OmegaPlan::new(
        Arc::clone(&p_plan),
        p_key.binding().clone(),
        p_key.vk().clone(),
    )?);
    let omega_key = keygen_pk_v2(
        &spec.params_q,
        &OmegaCircuit {
            plan: Arc::clone(&omega_plan),
            witness: None,
        },
        &KeygenConfigV2::pipa_r(INSTANCE_TYPES.to_vec()),
    )
    .map_err(|_| WrapError::Proof)?;
    Ok(WrapKeys {
        s_plan,
        s_key,
        r_plan,
        r_key,
        p_plan,
        p_key,
        omega_plan,
        omega_key,
    })
}

/// A finished chain, ending with the A instance `A_n`: everything the wraps
/// decide and bind.
pub struct FinalChain {
    /// `A_n`'s step index `n` (`Q_{n-1}` is step `n - 1`).
    pub step: u64,
    /// `z_A`, the A state after `A_n`.
    pub a_state: Vec<Fp>,
    /// `z_Q`, the Q state after `Q_{n-1}`.
    pub q_state: Vec<Fq>,
    /// The Q accumulators `A_n` keeps after folding `Q_{n-1}`, one per Q
    /// kind in kind order, with their witnesses.
    pub u_q: Vec<Kept<Ep>>,
    /// The A accumulators `Q_{n-1}` keeps, one per A kind in kind order; only
    /// the last kind's may be absent (`A_n` then starts it).
    pub u_a: Vec<Option<Kept<Eq>>>,
    /// `A_n`'s instance and witness.
    pub a_n: Fresh<Eq>,
    /// The Pallas claim accumulator A keeps and its witness `s`.
    pub pallas_claims: (ClaimAccumulator<Ep>, Vec<Fq>),
    /// The Vesta claim accumulator Q keeps and its witness `s`.
    pub vesta_claims: (ClaimAccumulator<Eq>, Vec<Fp>),
}

impl core::fmt::Debug for FinalChain {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FinalChain { private: [REDACTED] }")
    }
}

/// The private coins of the four committed-mask openings, one tape each.
pub struct WrapTapes {
    /// The Pallas claim accumulator's opening (P).
    pub pallas_restructure: FoldTape<Ep>,
    /// `acc_P`'s fold (P).
    pub pallas_fold: FoldTape<Ep>,
    /// The Vesta claim accumulator's opening (Omega).
    pub vesta_restructure: FoldTape<Eq>,
    /// `acc_V`'s fold (Omega).
    pub vesta_fold: FoldTape<Eq>,
}

/// Omega and what the consumer decides: `acc_P` (inside `D_A`) and `acc_V`
/// (Omega's public columns), in today's transport.
#[derive(Debug)]
pub struct WrapOutput {
    /// Omega's proof (Pallas).
    pub proof: Vec<u8>,
    /// Omega's public columns `[D_A]`, `acc_V.G`, `acc_V.u`.
    pub instances: Vec<Vec<Fq>>,
    /// `D_A`.
    pub d_a: Fp,
    /// The transported Pallas accumulator, bound in `D_A`.
    pub acc_p: AccumulatorT<Ep>,
    /// The transported Vesta accumulator.
    pub acc_v: AccumulatorT<Eq>,
}

/// One decider run natively: its proof, opening and the split values.
struct Decided<C: PastaCurve> {
    proved: Proved<C>,
    opening: BatchOpening<C>,
    claim: GeneratorClaim<C>,
    alpha: C::ScalarExt,
    value: C::ScalarExt,
    folded: C::ScalarExt,
}

fn decide<C: PastaCurve>(
    plans: &[Arc<DeciderV2Plan<C>>],
    params: &PinnedParams<C>,
    domain: &[u8; 32],
    members: &[(&Accumulator<C>, &FoldWitnessColumns<C::ScalarExt>)],
) -> Result<Decided<C>, WrapError>
where
    C::Base: PoseidonField,
    C::ScalarExt: PoseidonField,
{
    if plans.len() != members.len() {
        return Err(WrapError::Input);
    }
    let inputs: Vec<_> = plans
        .iter()
        .zip(members)
        .map(|(plan, (acc, witness))| (&**plan, *acc, *witness))
        .collect();
    let mut proved = decider_v2::prove_many(domain, &inputs).map_err(|_| WrapError::Fold)?;
    // alpha is the opening transcript's one 128-bit squeeze after its tag, k
    // and the claim count, as an endoscalar.
    let mut replay = proved.transcript.clone();
    let (opening, claim) =
        decider_v2::open(params.params(), &mut proved).map_err(|_| WrapError::Fold)?;
    replay.absorb_u64(mle::TAG_BATCH_OPENING);
    replay.absorb_u64(opening.rounds.len() as u64);
    replay.absorb_u64(proved.claims.len() as u64);
    let squeezed = replay.challenge_128().to_canonical_limbs();
    let alpha =
        mle::endoscalar::<C::ScalarExt>(u128::from(squeezed[0]) | (u128::from(squeezed[1]) << 64));
    let value = proved
        .claims
        .iter()
        .rev()
        .fold(C::ScalarExt::ZERO, |acc, c| acc * alpha + c.value);
    let folded = mle::folded_eq(&proved.point, claim.challenges()).ok_or(WrapError::Fold)?;
    Ok(Decided {
        proved,
        opening,
        claim,
        alpha,
        value,
        folded,
    })
}

/// Canonical limbs of a value of the other Pasta field, as two `Fp` rows.
fn pair<F: PastaField>(value: &F) -> Pair<Fp> {
    s6_limbs(value).map(Fp::from_u128)
}

fn terms_pairs(acc: &Accumulator<Ep>, copy_terms: Fq) -> AccumulatorTerms<Pair<Fp>> {
    AccumulatorTerms {
        theta: acc.instance.theta.iter().map(pair).collect(),
        r: acc.instance.r.iter().map(pair).collect(),
        beta: acc.beta.iter().map(pair).collect(),
        e: pair(&acc.e),
        copy_terms: pair(&copy_terms),
    }
}

/// Each member's instance copy term under its batch's copy challenges.
fn copy_terms<C: PastaCurve>(
    plans: &[Arc<DeciderV2Plan<C>>],
    accs: &[&Accumulator<C>],
    [gamma, eta]: [C::ScalarExt; 2],
) -> Result<Vec<C::ScalarExt>, WrapError> {
    plans
        .iter()
        .zip(accs)
        .map(|(plan, acc)| {
            decider_v2::instance_copy_terms(plan, acc, gamma, eta).map_err(|_| WrapError::Fold)
        })
        .collect()
}

/// `D_A` natively: the lineage fields of `z_A`, then `acc_P`'s generator and
/// challenge limbs (the preimage `a_relation::binding::lineage_digest` hashes).
fn lineage_digest(
    spec: &WrapSpec,
    state: &[Fp],
    acc_p: &AccumulatorT<Ep>,
) -> Result<Fp, WrapError> {
    let mut words = spec
        .lineage
        .iter()
        .map(|&i| state.get(i).copied().ok_or(WrapError::Input))
        .collect::<Result<Vec<_>, _>>()?;
    let (x, y) = generator_coordinates::<Ep>(acc_p.g());
    words.extend([x, y]);
    for challenge in acc_p.challenges() {
        words.extend(foreign_limbs(challenge).map(Fp::from_u128));
    }
    Ok(hash_with_domain(LINEAGE_DOMAIN, &words))
}

fn restructure<C: PastaCurve>(
    params: &PinnedParams<C>,
    domain: &[u8; 32],
    (acc, s): (&ClaimAccumulator<C>, &[C::ScalarExt]),
    tape: FoldTape<C>,
    config: &FoldConfig,
) -> Result<
    (
        claims::RestructureProof<C::ScalarExt>,
        claims::RestructureStatement<C>,
        iroha_plonk_recursion::FoldWitness<C>,
        AccumulatorT<C>,
    ),
    WrapError,
>
where
    C::Base: PoseidonField,
{
    let (proof, statement) =
        claims::restructure_prove(domain, acc, s).map_err(|_| WrapError::Fold)?;
    let opened = RestructureStatement {
        commitment: statement.commitment,
        point: statement.point,
        value: statement.value,
    };
    let (witness, claim) =
        create_restructure(params, &opened, s, tape, config).map_err(|_| WrapError::Fold)?;
    Ok((proof, statement, witness, claim))
}

fn input<C: PastaCurve>(claim: &GeneratorClaim<C>) -> Result<FoldInput<C>, WrapError> {
    FoldInput::from_opening(*claim.g(), claim.challenges()).map_err(|_| WrapError::Fold)
}

/// An `Fp` value's integer in `Fq` (exact: `p < q`).
fn embed_fp(value: &Fp) -> Fq {
    Fq::from_raw_reduced(value.to_canonical_limbs())
}

/// Everything computed natively before S's proof: S's witness and public
/// column, and R's, P's and Omega's inputs besides the earlier proofs.
pub struct Prepared {
    /// S's public column.
    pub s_public: Vec<Fp>,
    /// S's witness.
    pub s_witness: Arc<QGroupWitness>,
    r: RInputs,
    p: PInputs,
    omega: OmegaInputs,
}

impl core::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Prepared { private: [REDACTED] }")
    }
}

/// R's witness and public column, except S's column and proof and R's claim
/// for S, which [`r_stage`] adds after S's proof.
struct RInputs {
    witness: RWitness,
    public: RPublic<Fq>,
}

/// P's inputs besides R's proof and `acc_P`'s fold.
struct PInputs {
    step: u64,
    state: Vec<Fp>,
    a: AFieldWitness,
    pallas: PallasClaimWitness,
    vesta: VestaClaimWitness,
    restructure: FoldWitness<Ep>,
    q_claim: GeneratorClaim<Ep>,
    pallas_claim: AccumulatorT<Ep>,
    fold_tape: FoldTape<Ep>,
    q_crossing_digest: Fq,
    a_claim: ForwardedClaim<Fp>,
    pallas_crossing: PallasClaimCrossing<Fp>,
    vesta_crossing: VestaClaimCrossing<Fp>,
}

/// Omega's inputs besides P's proof and `acc_V`'s fold.
struct OmegaInputs {
    q: QFieldWitness,
    pallas: PallasFieldWitness,
    vesta: ClaimAccumulator<Eq>,
    restructure: FoldWitness<Eq>,
    a_claim: GeneratorClaim<Eq>,
    vesta_claim: AccumulatorT<Eq>,
    fold_tape: FoldTape<Eq>,
}

/// Runs both deciders, the last A fold and both restructurings natively,
/// and assembles S's witness and public column and R's, P's and Omega's
/// inputs.
///
/// # Errors
/// [`WrapError::Input`] for a chain of another shape and [`WrapError::Fold`]
/// when a native step fails (a chain whose accumulators do not decide).
#[allow(clippy::too_many_lines)]
pub fn prepare(
    keys: &WrapKeys,
    chain: &FinalChain,
    tapes: WrapTapes,
    config: &FoldConfig,
) -> Result<Prepared, WrapError> {
    let s_plan = &*keys.s_plan;
    let r_plan = &*keys.r_plan;
    let spec = &*s_plan.spec;
    let domains = &spec.domains;
    let last = spec.last;
    if chain.a_state.len() != spec.a_state
        || chain.q_state.len() != spec.q_state
        || chain.step < 2
        || chain.u_q.len() != spec.q.len()
        || chain.u_a.len() != spec.a.len()
        || chain
            .u_a
            .iter()
            .enumerate()
            .any(|(k, kept)| kept.is_none() && k != last)
    {
        return Err(WrapError::Input);
    }
    // The Q-side decider over every Q kind (S's commitment side, Omega's
    // field side).
    let q_members: Vec<_> = chain.u_q.iter().map(|(acc, w)| (acc, w)).collect();
    let q = decide(&s_plan.q_plans, &spec.params_q, &domains.q_fold, &q_members)?;
    // The last A fold (or start), then the A-side decider over every A kind
    // (R's commitment side, P's field side).
    let (a_n, a_n_witness) = &chain.a_n;
    let (folded, folded_witness, a_fold) = ivc::fold_or_start(
        &spec.a[last],
        &domains.a_fold,
        chain.u_a[last].as_ref().map(|(acc, w)| (acc, w)),
        a_n,
        a_n_witness,
    )
    .map_err(|_| WrapError::Fold)?;
    let mut decided: Vec<(Accumulator<Eq>, FoldWitnessColumns<Fp>)> =
        Vec::with_capacity(spec.a.len());
    for (kind, kept) in chain.u_a.iter().enumerate() {
        if kind == last {
            decided.push((folded.clone(), folded_witness.clone()));
        } else {
            decided.push(kept.clone().ok_or(WrapError::Input)?);
        }
    }
    let a_members: Vec<_> = decided.iter().map(|(acc, w)| (acc, w)).collect();
    let a = decide(&r_plan.a_plans, &spec.params_a, &domains.a_fold, &a_members)?;
    // The instance copy terms each field side imports.
    let q_accs: Vec<&Accumulator<Ep>> = chain.u_q.iter().map(|(acc, _)| acc).collect();
    let a_accs: Vec<&Accumulator<Eq>> = decided.iter().map(|(acc, _)| acc).collect();
    let q_terms = copy_terms(&s_plan.q_plans, &q_accs, q.proved.copy_challenges)?;
    let a_terms = copy_terms(&r_plan.a_plans, &a_accs, a.proved.copy_challenges)?;
    // The claim accumulators' restructurings.
    let (pallas_acc, pallas_s) = &chain.pallas_claims;
    let (pallas_proof, pallas_statement, pallas_opening, pallas_claim) = restructure(
        &spec.params_q,
        &domains.pallas_claims,
        (pallas_acc, pallas_s),
        tapes.pallas_restructure,
        config,
    )?;
    let (vesta_acc, vesta_s) = &chain.vesta_claims;
    let (vesta_proof, vesta_statement, vesta_opening, vesta_claim) = restructure(
        &spec.params_a,
        &domains.vesta_claims,
        (vesta_acc, vesta_s),
        tapes.vesta_restructure,
        config,
    )?;
    // S's public column: the Q crossing and G'_0.
    let q_helper = decider_v2::helper_digest::<Ep>(&domains.q_fold, &q.proved.proof.helpers);
    let (q_x, q_y) = generator_coordinates::<Ep>(q.claim.g());
    let s_public = SPublic {
        q: QCrossing {
            members: q_accs
                .iter()
                .zip(&q_terms)
                .map(|(acc, copy)| QMember {
                    acc_digest: acc.digest(&domains.q_fold),
                    terms: terms_pairs(acc, *copy),
                })
                .collect(),
            helper_digest: q_helper,
            copy_challenges: q.proved.copy_challenges.map(|c| pair(&c)),
            field_digest: pair(&q.proved.field_digest),
            value: pair(&q.value),
            folded: pair(&q.folded),
            alpha: pair(&q.alpha),
            rounds: q.claim.challenges().iter().map(pair).collect(),
        },
        claim: [q_x, q_y],
    };
    // D_SQ, as R hashes S's rows.
    let two128 = Fq::from_u128(1 << 64).square();
    let q_crossing_digest = hash_with_domain(
        Q_CROSSING_DOMAIN,
        &s_public
            .q
            .to_words(
                |digest: &Fp| s6_limbs(digest).map(Fq::from_u128),
                |[lo, hi]: &Pair<Fp>| Ok::<_, WrapError>(embed_fp(lo) + two128 * embed_fp(hi)),
            )?
            .words(),
    );
    let s_rows = s_public.rows();
    if s_rows.len() != s_plan.layout.rows() {
        return Err(WrapError::Input);
    }
    let s_witness = QGroupWitness {
        accs: q_accs.iter().map(|acc| (*acc).clone()).collect(),
        helpers: q.proved.proof.helpers.clone(),
        opening: q.opening.clone(),
        imports: [q.proved.field_digest, q.value, q.folded],
        copy_challenges: q.proved.copy_challenges,
    };
    // R's public column and witness, but for S's column and proof and R's
    // claim for S.
    let a_helper = decider_v2::helper_digest::<Eq>(&domains.a_fold, &a.proved.proof.helpers);
    let (claim_x, claim_y) = generator_coordinates::<Eq>(a.claim.g());
    let vesta_digest = vesta_acc.digest(&domains.vesta_claims);
    let r_public = RPublic {
        a_public: a_n.public.iter().flatten().map(embed_fp).collect(),
        members: a_accs
            .iter()
            .zip(&a_terms)
            .map(|(acc, copy)| RMember {
                acc_digest: acc.digest(&domains.a_fold),
                terms: AccumulatorTerms {
                    theta: acc.instance.theta.iter().map(embed_fp).collect(),
                    r: acc.instance.r.iter().map(embed_fp).collect(),
                    beta: acc.beta.iter().map(embed_fp).collect(),
                    e: embed_fp(&acc.e),
                    copy_terms: embed_fp(copy),
                },
            })
            .collect(),
        helper_digest: a_helper,
        copy_challenges: a.proved.copy_challenges.map(|c| embed_fp(&c)),
        field_digest: embed_fp(&a.proved.field_digest),
        value: embed_fp(&a.value),
        folded: embed_fp(&a.folded),
        alpha: embed_fp(&a.alpha),
        rounds: a.claim.challenges().iter().map(embed_fp).collect(),
        claim: [claim_x, claim_y],
        claim_digest: vesta_digest,
        q_digests: q_accs
            .iter()
            .map(|acc| embed_fp(&acc.digest(&domains.q_fold)))
            .collect(),
        q_claim: ClaimRows {
            g: [embed_fp(&q_x), embed_fp(&q_y)],
            u: q.claim.challenges().to_vec(),
        },
        q_crossing_digest,
        s_claim: ClaimRows {
            g: [Fq::ZERO; 2],
            u: Vec::new(),
        },
    };
    let r_witness = RWitness {
        s_public: Vec::new(),
        s_proof: Vec::new(),
        step: chain.step - 1,
        state: chain.q_state.clone(),
        accs: chain
            .u_a
            .iter()
            .map(|kept| kept.as_ref().map(|(acc, _)| acc.clone()))
            .collect(),
        fresh: a_n.clone(),
        fold: a_fold,
        helpers: a.proved.proof.helpers.clone(),
        opening: a.opening.clone(),
        copy_challenges: a.proved.copy_challenges,
        imports: [a.proved.field_digest, a.value, a.folded],
        claim_digest: vesta_digest,
    };
    // P's inputs.
    let p = PInputs {
        step: chain.step,
        state: chain.a_state.clone(),
        q_crossing_digest,
        a_claim: ForwardedClaim {
            g: [pair(&claim_x), pair(&claim_y)],
            u: a.claim.challenges().to_vec(),
        },
        pallas_crossing: PallasClaimCrossing {
            digest: pallas_acc.digest(&domains.pallas_claims),
            u: pallas_acc.u.iter().map(pair).collect(),
            beta: pallas_acc.beta.iter().map(pair).collect(),
            e: pair(&pallas_acc.e),
            rho: pallas_statement.point.iter().map(pair).collect(),
            value: pair(&pallas_statement.value),
        },
        vesta_crossing: VestaClaimCrossing {
            digest: pair(&vesta_digest),
            u: vesta_acc.u.to_vec(),
            beta: vesta_acc.beta.to_vec(),
            e: vesta_acc.e,
            rho: vesta_statement.point.to_vec(),
            value: vesta_statement.value,
        },
        a: AFieldWitness {
            accs: a_accs.iter().map(|acc| (*acc).clone()).collect(),
            proof: a.proved.proof.clone(),
            copy_terms: a_terms,
        },
        pallas: PallasClaimWitness {
            acc: *pallas_acc,
            rho: pallas_statement.point,
            value: pallas_statement.value,
        },
        vesta: VestaClaimWitness {
            acc: *vesta_acc,
            proof: vesta_proof,
        },
        restructure: pallas_opening,
        q_claim: q.claim.clone(),
        pallas_claim,
        fold_tape: tapes.pallas_fold,
    };
    let omega = OmegaInputs {
        q: QFieldWitness {
            accs: chain.u_q.iter().map(|(acc, _)| acc.clone()).collect(),
            copy_terms: q_terms,
            rounds: q.claim.challenges().to_vec(),
            proof: q.proved.proof,
            helper_digest: q_helper,
            alpha: q.alpha,
        },
        pallas: PallasFieldWitness {
            acc: *pallas_acc,
            proof: pallas_proof,
        },
        vesta: *vesta_acc,
        restructure: vesta_opening,
        a_claim: a.claim,
        vesta_claim,
        fold_tape: tapes.vesta_fold,
    };
    Ok(Prepared {
        s_public: s_rows,
        s_witness: Arc::new(s_witness),
        r: RInputs {
            witness: r_witness,
            public: r_public,
        },
        p,
        omega,
    })
}

/// S with `prepared`'s witness.
#[must_use]
pub fn s_circuit(keys: &WrapKeys, prepared: &Prepared) -> SCircuit {
    SCircuit {
        plan: Arc::clone(&keys.s_plan),
        witness: Some(Arc::clone(&prepared.s_witness)),
    }
}

/// Proves S (Vesta), returning its proof and opening claim.
///
/// # Errors
/// [`WrapError::Proof`] when proving fails.
pub fn prove_s(
    keys: &WrapKeys,
    prepared: &Prepared,
) -> Result<(Vec<u8>, GeneratorClaim<Eq>), WrapError> {
    let circuit = s_circuit(keys, prepared);
    let columns = Witness::from_circuit(&keys.s_key, &circuit, &[prepared.s_public.clone()])
        .map_err(|_| WrapError::Proof)?;
    let output = create_proof_owned_with_claim(
        &keys.s_plan.spec.params_a,
        &keys.s_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .map_err(|_| WrapError::Proof)?;
    Ok((output.proof, output.opening))
}

/// R's witness and public column after S's proof.
pub struct RStage {
    /// R's public column.
    pub r_public: Vec<Fq>,
    /// R's witness.
    pub r_witness: Arc<RWitness>,
    s_claim: GeneratorClaim<Eq>,
    p: PInputs,
    omega: OmegaInputs,
}

impl core::fmt::Debug for RStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("RStage { private: [REDACTED] }")
    }
}

/// Completes R's witness with S's column and proof, and R's public column
/// with R's claim for S (S's opening claim, which R's verification of S
/// yields).
///
/// # Errors
/// [`WrapError::Input`] for another layout.
pub fn r_stage(
    keys: &WrapKeys,
    prepared: Prepared,
    s_proof: Vec<u8>,
    s_opening: &GeneratorClaim<Eq>,
) -> Result<RStage, WrapError> {
    let RInputs {
        mut witness,
        mut public,
    } = prepared.r;
    let (x, y) = generator_coordinates::<Eq>(s_opening.g());
    public.s_claim = ClaimRows {
        g: [x, y],
        u: s_opening.challenges().iter().map(embed_fp).collect(),
    };
    let r_public = public.rows();
    if r_public.len() != keys.r_plan.layout.rows() {
        return Err(WrapError::Input);
    }
    witness.s_public = prepared.s_public;
    witness.s_proof = s_proof;
    Ok(RStage {
        r_public,
        r_witness: Arc::new(witness),
        s_claim: s_opening.clone(),
        p: prepared.p,
        omega: prepared.omega,
    })
}

/// R with `stage`'s witness.
#[must_use]
pub fn r_circuit(keys: &WrapKeys, stage: &RStage) -> RCircuit {
    RCircuit {
        plan: Arc::clone(&keys.r_plan),
        witness: Some(Arc::clone(&stage.r_witness)),
    }
}

/// Proves R (Pallas), returning its proof and opening claim.
///
/// # Errors
/// [`WrapError::Proof`] when proving fails.
pub fn prove_r(
    keys: &WrapKeys,
    stage: &RStage,
) -> Result<(Vec<u8>, GeneratorClaim<Ep>), WrapError> {
    let circuit = r_circuit(keys, stage);
    let columns = Witness::from_circuit(&keys.r_key, &circuit, &[stage.r_public.clone()])
        .map_err(|_| WrapError::Proof)?;
    let output = create_proof_owned_with_claim(
        &keys.r_plan.spec.params_q,
        &keys.r_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .map_err(|_| WrapError::Proof)?;
    Ok((output.proof, output.opening))
}

/// P's witness and public column after R's proof: `acc_P` folds natively
/// with R's claim, then `D_A`.
pub struct PStage {
    /// P's public column.
    pub p_public: Vec<Fp>,
    /// P's witness.
    pub p_witness: Arc<PWitness>,
    /// `D_A`.
    pub d_a: Fp,
    /// `acc_P`.
    pub acc_p: AccumulatorT<Ep>,
    s_claim: GeneratorClaim<Eq>,
    omega: OmegaInputs,
}

impl core::fmt::Debug for PStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PStage { private: [REDACTED] }")
    }
}

/// Folds `acc_P` from the Q decider's claim, the restructured Pallas claim
/// and R's claim, computes `D_A`, and completes P's witness.
///
/// # Errors
/// [`WrapError::Fold`] when `acc_P`'s fold fails, [`WrapError::Input`] for
/// another layout.
pub fn p_stage(
    keys: &WrapKeys,
    stage: RStage,
    r_proof: Vec<u8>,
    r_opening: &GeneratorClaim<Ep>,
    config: &FoldConfig,
) -> Result<PStage, WrapError> {
    let spec = &*keys.p_plan.spec;
    let inputs = stage.p;
    let (fold, acc_p) = create_fold(
        &spec.params_q,
        &[
            input(&inputs.q_claim)?,
            inputs.pallas_claim.as_input(),
            input(r_opening)?,
        ],
        inputs.fold_tape,
        FoldPurpose::FoldWrapPallas,
        config,
    )
    .map_err(|_| WrapError::Fold)?;
    let d_a = lineage_digest(spec, &inputs.state, &acc_p)?;
    let (s_x, s_y) = generator_coordinates::<Eq>(stage.s_claim.g());
    let p_public = PPublic {
        d_a,
        q_crossing_digest: pair(&inputs.q_crossing_digest),
        s_claim: ForwardedClaim {
            g: [pair(&s_x), pair(&s_y)],
            u: stage.s_claim.challenges().to_vec(),
        },
        a_claim: inputs.a_claim,
        pallas: inputs.pallas_crossing,
        vesta: inputs.vesta_crossing,
    }
    .rows();
    if p_public.len() != keys.p_plan.layout.rows() {
        return Err(WrapError::Input);
    }
    let p_witness = PWitness {
        step: inputs.step,
        state: inputs.state,
        r_public: stage.r_public,
        r_proof,
        a: inputs.a,
        pallas: inputs.pallas,
        vesta: inputs.vesta,
        restructure: inputs.restructure,
        fold,
    };
    Ok(PStage {
        p_public,
        p_witness: Arc::new(p_witness),
        d_a,
        acc_p,
        s_claim: stage.s_claim,
        omega: stage.omega,
    })
}

/// P with `stage`'s witness.
#[must_use]
pub fn p_circuit(keys: &WrapKeys, stage: &PStage) -> PCircuit {
    PCircuit {
        plan: Arc::clone(&keys.p_plan),
        witness: Some(Arc::clone(&stage.p_witness)),
    }
}

/// Proves P (Vesta), returning its proof and opening claim.
///
/// # Errors
/// [`WrapError::Proof`] when proving fails.
pub fn prove_p(
    keys: &WrapKeys,
    stage: &PStage,
) -> Result<(Vec<u8>, GeneratorClaim<Eq>), WrapError> {
    let circuit = p_circuit(keys, stage);
    let columns = Witness::from_circuit(&keys.p_key, &circuit, &[stage.p_public.clone()])
        .map_err(|_| WrapError::Proof)?;
    let output = create_proof_owned_with_claim(
        &keys.p_plan.spec.params_a,
        &keys.p_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .map_err(|_| WrapError::Proof)?;
    Ok((output.proof, output.opening))
}

/// Omega with its witness for P's proof: folds `acc_V` natively and returns
/// the circuit, Omega's public columns and `acc_V`.
///
/// # Errors
/// [`WrapError::Fold`] when `acc_V`'s fold fails.
pub fn omega_circuit(
    keys: &WrapKeys,
    stage: PStage,
    p_proof: Vec<u8>,
    p_opening: &GeneratorClaim<Eq>,
    config: &FoldConfig,
) -> Result<(OmegaCircuit, Vec<Vec<Fq>>, AccumulatorT<Eq>), WrapError> {
    let spec = &*keys.p_plan.spec;
    let inputs = stage.omega;
    // acc_V = fold(P's claim, R's claim for S, the A decider's claim, the
    // restructured Vesta claim).
    let (fold, acc_v) = create_fold(
        &spec.params_a,
        &[
            input(p_opening)?,
            input(&stage.s_claim)?,
            input(&inputs.a_claim)?,
            inputs.vesta_claim.as_input(),
        ],
        inputs.fold_tape,
        FoldPurpose::FoldWrapVesta,
        config,
    )
    .map_err(|_| WrapError::Fold)?;
    let witness = OmegaWitness {
        p_public: stage.p_public,
        p_proof,
        q: inputs.q,
        pallas: inputs.pallas,
        vesta: inputs.vesta,
        restructure: inputs.restructure,
        fold,
    };
    let instances = omega_instances(stage.d_a, &acc_v);
    let circuit = OmegaCircuit {
        plan: Arc::clone(&keys.omega_plan),
        witness: Some(Arc::new(witness)),
    };
    Ok((circuit, instances, acc_v))
}

/// Proves S, then R, then P, then Omega, for `chain`.
///
/// # Errors
/// [`WrapError::Input`] for a chain of another shape, [`WrapError::Fold`]
/// when a native step fails (a chain whose accumulators do not decide), and
/// [`WrapError::Proof`] when proving fails.
pub fn prove(
    keys: &WrapKeys,
    chain: &FinalChain,
    tapes: WrapTapes,
    config: &FoldConfig,
) -> Result<WrapOutput, WrapError> {
    let prepared = prepare(keys, chain, tapes, config)?;
    let (s_proof, s_opening) = prove_s(keys, &prepared)?;
    let stage = r_stage(keys, prepared, s_proof, &s_opening)?;
    let (r_proof, r_opening) = prove_r(keys, &stage)?;
    let stage = p_stage(keys, stage, r_proof, &r_opening, config)?;
    let (d_a, acc_p) = (stage.d_a, stage.acc_p.clone());
    let (p_proof, p_opening) = prove_p(keys, &stage)?;
    let (circuit, instances, acc_v) = omega_circuit(keys, stage, p_proof, &p_opening, config)?;
    let columns = Witness::from_circuit(&keys.omega_key, &circuit, &instances)
        .map_err(|_| WrapError::Proof)?;
    let output = create_proof_owned_with_claim(
        &keys.p_plan.spec.params_q,
        &keys.omega_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .map_err(|_| WrapError::Proof)?;
    Ok(WrapOutput {
        proof: output.proof,
        instances,
        d_a,
        acc_p,
        acc_v,
    })
}

/// Omega's public columns: `[D_A]`, `acc_V.G`, `acc_V.u` (the integers of the
/// `Fp` values, embedded in `Fq`).
#[must_use]
pub fn omega_instances(d_a: Fp, acc_v: &AccumulatorT<Eq>) -> Vec<Vec<Fq>> {
    let (x, y) = generator_coordinates::<Eq>(acc_v.g());
    vec![
        vec![embed_fp(&d_a)],
        vec![x, y],
        acc_v.challenges().iter().map(embed_fp).collect(),
    ]
}
