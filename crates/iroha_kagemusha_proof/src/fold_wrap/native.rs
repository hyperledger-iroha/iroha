//! The wraps' native producer: key generation, then for one finished chain
//! every decider, fold and restructuring natively, P's proof and Omega's.
//!
//! The order is fixed by the crossing: Omega's commitment-side values (the
//! last A fold and its decider) and P's (the Q decider and the claims) are all
//! native first, so both witnesses are complete before either proof starts.

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
    AccumulatorT, FoldConfig, FoldInput, FoldPurpose, FoldTape, RestructureStatement, create_fold,
    create_restructure,
};

use super::{
    WrapError, WrapSpec,
    crossing::{
        ACrossing, AMember, AccumulatorData, PPublic, Pair, PallasClaimCrossing, QCrossing,
        QMember, VestaClaimCrossing,
    },
    omega::{
        AGroupWitness, INSTANCE_TYPES, OmegaCircuit, OmegaPlan, OmegaWitness, PallasFieldWitness,
        QFieldWitness,
    },
    p::{
        AFieldWitness, PCircuit, PPlan, PWitness, PallasClaimWitness, QGroupWitness,
        VestaClaimWitness,
    },
};
use crate::a_relation::LINEAGE_DOMAIN;

/// Both wraps' programs and proving keys.
pub struct WrapKeys {
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

/// Generates P's key, then Omega's (which pins P's).
///
/// # Errors
/// [`WrapError::Input`] for relations the wraps do not admit,
/// [`WrapError::Proof`] when key generation fails.
pub fn keygen(spec: Arc<WrapSpec>) -> Result<WrapKeys, WrapError> {
    let p_plan = Arc::new(PPlan::new(Arc::clone(&spec))?);
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
    /// P's public column (diagnostics and tests; private to the wallet).
    pub p_public: Vec<Fp>,
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

fn data_pairs(acc: &Accumulator<Ep>) -> AccumulatorData<Pair<Fp>> {
    AccumulatorData {
        public: acc.instance.public.iter().flatten().map(pair).collect(),
        theta: acc.instance.theta.iter().map(pair).collect(),
        r: acc.instance.r.iter().map(pair).collect(),
        beta: acc.beta.iter().map(pair).collect(),
        e: pair(&acc.e),
    }
}

fn data(acc: &Accumulator<Eq>) -> AccumulatorData<Fp> {
    AccumulatorData {
        public: acc.instance.public.iter().flatten().copied().collect(),
        theta: acc.instance.theta.clone(),
        r: acc.instance.r.clone(),
        beta: acc.beta.clone(),
        e: acc.e,
    }
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

/// Everything both witnesses need, computed natively before any proof: P's
/// complete witness and public column, and Omega's inputs besides P's proof.
pub struct Prepared {
    /// P's public column.
    pub p_public: Vec<Fp>,
    /// P's witness.
    pub p_witness: Arc<PWitness>,
    /// `D_A`.
    pub d_a: Fp,
    /// `acc_P`.
    pub acc_p: AccumulatorT<Ep>,
    omega: OmegaInputs,
}

impl core::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Prepared { private: [REDACTED] }")
    }
}

/// Omega's inputs besides P's proof, and what `acc_V`'s fold takes.
struct OmegaInputs {
    step: u64,
    state: Vec<Fq>,
    q: QFieldWitness,
    pallas: PallasFieldWitness,
    a: AGroupWitness,
    vesta: ClaimAccumulator<Eq>,
    restructure: iroha_plonk_recursion::FoldWitness<Eq>,
    a_claim: GeneratorClaim<Eq>,
    vesta_claim: AccumulatorT<Eq>,
    fold_tape: FoldTape<Eq>,
}

/// Runs both deciders, the last A fold, both restructurings and `acc_P`'s
/// fold natively, and assembles P's witness and public column.
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
    let p_plan = &*keys.p_plan;
    let spec = &*p_plan.spec;
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
    // The Q-side decider over every Q kind (P's commitment side, Omega's
    // field side).
    let q_members: Vec<_> = chain.u_q.iter().map(|(acc, w)| (acc, w)).collect();
    let q = decide(&p_plan.q_plans, &spec.params_q, &domains.q_fold, &q_members)?;
    // The last A fold (or start), then the A-side decider over every A kind.
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
    let a = decide(&p_plan.a_plans, &spec.params_a, &domains.a_fold, &a_members)?;
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
    // acc_P = fold(Q decider claim, restructured Pallas claim), and D_A.
    let (pallas_fold, acc_p) = create_fold(
        &spec.params_q,
        &[input(&q.claim)?, pallas_claim.as_input()],
        tapes.pallas_fold,
        FoldPurpose::FoldWrapPallas,
        config,
    )
    .map_err(|_| WrapError::Fold)?;
    let d_a = lineage_digest(spec, &chain.a_state, &acc_p)?;
    // P's public column.
    let a_public: Vec<Fp> = a_n.public.iter().flatten().copied().collect();
    let q_helper = decider_v2::helper_digest::<Ep>(&domains.q_fold, &q.proved.proof.helpers);
    let a_helper = decider_v2::helper_digest::<Eq>(&domains.a_fold, &a.proved.proof.helpers);
    let public = PPublic {
        a_public: a_public.clone(),
        d_a,
        q: QCrossing {
            members: chain
                .u_q
                .iter()
                .map(|(acc, _)| QMember {
                    acc_digest: acc.digest(&domains.q_fold),
                    data: data_pairs(acc),
                })
                .collect(),
            helper_digest: q_helper,
            field_digest: pair(&q.proved.field_digest),
            value: pair(&q.value),
            folded: pair(&q.folded),
            alpha: pair(&q.alpha),
            rounds: q.claim.challenges().iter().map(pair).collect(),
        },
        a: ACrossing {
            members: decided
                .iter()
                .map(|(acc, _)| AMember {
                    acc_digest: pair(&acc.digest(&domains.a_fold)),
                    data: data(acc),
                })
                .collect(),
            helper_digest: pair(&a_helper),
            field_digest: a.proved.field_digest,
            value: a.value,
            folded: a.folded,
            alpha: a.alpha,
            rounds: a.claim.challenges().to_vec(),
        },
        pallas: PallasClaimCrossing {
            digest: pallas_acc.digest(&domains.pallas_claims),
            u: pallas_acc.u.iter().map(pair).collect(),
            beta: pallas_acc.beta.iter().map(pair).collect(),
            e: pair(&pallas_acc.e),
            rho: pallas_statement.point.iter().map(pair).collect(),
            value: pair(&pallas_statement.value),
        },
        vesta: VestaClaimCrossing {
            digest: pair(&vesta_acc.digest(&domains.vesta_claims)),
            u: vesta_acc.u.to_vec(),
            beta: vesta_acc.beta.to_vec(),
            e: vesta_acc.e,
            rho: vesta_statement.point.to_vec(),
            value: vesta_statement.value,
        },
    };
    let p_public = public.rows();
    if p_public.len() != p_plan.layout.rows() {
        return Err(WrapError::Input);
    }
    let p_witness = PWitness {
        step: chain.step,
        state: chain.a_state.clone(),
        a_public,
        q: QGroupWitness {
            accs: chain.u_q.iter().map(|(acc, _)| acc.clone()).collect(),
            helpers: q.proved.proof.helpers.clone(),
            opening: q.opening.clone(),
            imports: [q.proved.field_digest, q.value, q.folded],
        },
        a: AFieldWitness {
            accs: decided.iter().map(|(acc, _)| acc.clone()).collect(),
            proof: a.proved.proof.clone(),
            helper_digest: a_helper,
            alpha: a.alpha,
            rounds: a.claim.challenges().to_vec(),
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
        fold: pallas_fold,
    };
    let omega = OmegaInputs {
        step: chain.step - 1,
        state: chain.q_state.clone(),
        q: QFieldWitness {
            accs: chain.u_q.iter().map(|(acc, _)| acc.clone()).collect(),
            proof: q.proved.proof,
        },
        pallas: PallasFieldWitness {
            acc: *pallas_acc,
            proof: pallas_proof,
        },
        a: AGroupWitness {
            accs: chain
                .u_a
                .iter()
                .map(|kept| kept.as_ref().map(|(acc, _)| acc.clone()))
                .collect(),
            fresh: a_n.clone(),
            fold: a_fold,
            helpers: a.proved.proof.helpers,
            opening: a.opening,
        },
        vesta: *vesta_acc,
        restructure: vesta_opening,
        a_claim: a.claim,
        vesta_claim,
        fold_tape: tapes.vesta_fold,
    };
    Ok(Prepared {
        p_public,
        p_witness: Arc::new(p_witness),
        d_a,
        acc_p,
        omega,
    })
}

/// P with `prepared`'s witness.
#[must_use]
pub fn p_circuit(keys: &WrapKeys, prepared: &Prepared) -> PCircuit {
    PCircuit {
        plan: Arc::clone(&keys.p_plan),
        witness: Some(Arc::clone(&prepared.p_witness)),
    }
}

/// Proves P (Vesta), returning its proof and opening claim.
///
/// # Errors
/// [`WrapError::Proof`] when proving fails.
pub fn prove_p(
    keys: &WrapKeys,
    prepared: &Prepared,
) -> Result<(Vec<u8>, GeneratorClaim<Eq>), WrapError> {
    let circuit = p_circuit(keys, prepared);
    let columns = Witness::from_circuit(&keys.p_key, &circuit, &[prepared.p_public.clone()])
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
    prepared: Prepared,
    p_proof: Vec<u8>,
    p_opening: &GeneratorClaim<Eq>,
    config: &FoldConfig,
) -> Result<(OmegaCircuit, Vec<Vec<Fq>>, AccumulatorT<Eq>), WrapError> {
    let spec = &*keys.p_plan.spec;
    let inputs = prepared.omega;
    // acc_V = fold(P's claim, the A decider's claim, restructured Vesta claim).
    let (fold, acc_v) = create_fold(
        &spec.params_a,
        &[
            input(p_opening)?,
            input(&inputs.a_claim)?,
            inputs.vesta_claim.as_input(),
        ],
        inputs.fold_tape,
        FoldPurpose::FoldWrapVesta,
        config,
    )
    .map_err(|_| WrapError::Fold)?;
    let witness = OmegaWitness {
        step: inputs.step,
        state: inputs.state,
        p_public: prepared.p_public,
        p_proof,
        q: inputs.q,
        pallas: inputs.pallas,
        a: inputs.a,
        vesta: inputs.vesta,
        restructure: inputs.restructure,
        fold,
    };
    let instances = omega_instances(prepared.d_a, &acc_v);
    let circuit = OmegaCircuit {
        plan: Arc::clone(&keys.omega_plan),
        witness: Some(Arc::new(witness)),
    };
    Ok((circuit, instances, acc_v))
}

/// Proves P, then Omega, for `chain`.
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
    let (d_a, acc_p, p_public) = (
        prepared.d_a,
        prepared.acc_p.clone(),
        prepared.p_public.clone(),
    );
    let (p_proof, p_opening) = prove_p(keys, &prepared)?;
    let (circuit, instances, acc_v) = omega_circuit(keys, prepared, p_proof, &p_opening, config)?;
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
        p_public,
    })
}

/// Omega's public columns: `[D_A]`, `acc_V.G`, `acc_V.u` (the integers of the
/// `Fp` values, embedded in `Fq`).
#[must_use]
pub fn omega_instances(d_a: Fp, acc_v: &AccumulatorT<Eq>) -> Vec<Vec<Fq>> {
    let embed = |value: &Fp| Fq::from_raw_reduced(value.to_canonical_limbs());
    let (x, y) = generator_coordinates::<Eq>(acc_v.g());
    vec![
        vec![embed(&d_a)],
        vec![x, y],
        acc_v.challenges().iter().map(embed).collect(),
    ]
}
