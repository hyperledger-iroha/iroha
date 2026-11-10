//! The folded Receive chain (KAGEMUSHA-Fold, `fold_lineage`) and its genuine
//! fixture, shared by `fold_receive.rs` and the compression wrap tests.
//!
//! - [`receive_keys`]: the keys of one genuine Receive: a real k12 `σ_send`
//!   prover and the incoming-only Q0 plan over its verifier, a k16 Omega
//!   program under a descriptor key (a proof of the Omega profile's public
//!   columns), and today's Receive plan pinned for folding.
//! - [`full_receive`]: the Receive itself: the payer's Omega proof over `D_A`
//!   of its lineage and the transported claims, today's transport bytes, the
//!   Send receipt, Payment, Request and credentials over those tapes, the own
//!   receipt, Q0's prepared slot and Q1's three signatures, and the receiver's
//!   own last Omega, on which the segment rebases.
//! - [`receive_relations`]: the eight step relations (A1, A3, A4, A5, Rebase;
//!   Frame, Sigma, Signature) with monomial commitment keys, as the wraps
//!   decide them with decider v2.
//! - [`run_chain`]: the segment's ten half-steps from the segment base (a
//!   Frame and the Rebase, then the Receive's eight), each checked strictly,
//!   committed and folded; [`decide_chain`] decides what the chain leaves
//!   (both sides' accumulators through decider v2, the last instance started
//!   into its kind's, both claim accumulators).
//!
//! Include it with `#[path = "support/fold_receive_chain.rs"] mod
//! fold_receive_chain;`. It brings the fixture modules it needs as nested
//! modules. Heavy: k16 circuits; run optimized.

#![allow(dead_code)]

#[path = "../common/bootstrap.rs"]
pub mod bootstrap;
#[path = "../common/bootstrap_objects.rs"]
pub mod bootstrap_objects;
#[path = "../common/mod.rs"]
pub mod common;
#[path = "../common/load_objects.rs"]
pub mod load_objects;
#[path = "../common/receive_objects.rs"]
pub mod receive_objects;
#[path = "../common/send_objects.rs"]
pub mod send_objects;

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Witness,
    claims::{Claim, ClaimAccumulator, decide_claims},
    decider_v2::{self, DeciderV2Plan},
    fold::fresh_instance,
    ivc::{Fresh, Kept, fold_or_start, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    SigmaProver, SigmaRelation,
    a_relation::{
        AProofPlan, LINEAGE_DOMAIN, QProofPlan,
        context::ContextObjectSpec,
        fold::{
            rebase::{RebaseFoldStep, RebaseSource, RebaseStepWitness},
            receive::{ReceiveFoldPlan, ReceiveFoldSource, ReceiveFoldStep, ReceiveStepWitness},
        },
        native::receive::{IncomingWitness, Transition},
        own::OwnPolicy,
        receive::{MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES, ReceiveStagePlan},
        schedule::{compiled::OperationSchedule, sigma_selector},
    },
    admin_sigma::StateWitness,
    fold_lineage::{
        A_CLAIM_SLOT, A_DIGESTS, A_STEP_RUNS, AKind, BASE, CLAIM_A, CLAIM_Q, FOLD_A, FOLD_Q,
        Q_CLAIM_SLOT, Q_DIGESTS, QFrameStep, QFrameStepWitness, QKind, claim_of, fold_claims,
        segment_base, segment_start,
        sigma::{QSigmaStep, QSigmaStepWitness, fold_selected},
        signature::QSignatureStep,
        trivial_export, trivial_vesta,
    },
    operation_relation::objects::{ObjectKind, signed_message_native},
    q_sigma::{
        QSigmaPlan, SigmaClass, SigmaSlotWitness,
        native::{FoldedQSigma, IncomingMode, IncomingSigma},
    },
    q_signature::{QSignatureCircuit, QSignaturePlan, SignatureWitness},
};
use iroha_pasta::{
    Ep, Eq, Fp, Fq, PastaCurve, PastaField, msm::MemoryBudget, params::ParamsIpa,
    poseidon::hash_with_domain,
};
use iroha_plonk::{
    DescriptorBinding, ProvingKey, VerifyingKey,
    check::{CheckMode, check_circuit},
    cs::{
        CircuitDescriptorV1, CircuitDescriptorV2, Column, ConstraintSystem, CurveV1,
        DescriptorConfig, Instance, InstanceModeV1, InstanceType, ProofSuffixV1, TranscriptV1,
        TranscriptV2,
    },
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2, keygen_vk_with_binding_v2},
    pcs::ipa::PinnedParams,
    transcript::{decode_point, generator_coordinates},
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig,
    bytes::p_bytes_native,
    p256::native::{self as p256, Affine, HALF_N, ORDER, verify_prehashed},
    sha256::native::sha256_of_digest,
    statement::{STATEMENT_DOMAIN, foreign_limbs},
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR,
    obligation::ledger::Variant, verifier::VerifierPlan,
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

/// Every step circuit's size.
pub const K: u32 = 16;

/// A circuit whose only content is public columns of the given lengths (with
/// values when proving): its key is a descriptor of a Q leaf or Omega program
/// for the plan's schema.
#[derive(Clone)]
pub struct Export(pub Vec<usize>, pub Option<Vec<Fq>>);

impl Circuit<Fq> for Export {
    type Config = (GlueConfig, Vec<Column<Instance>>);
    type Params = Vec<usize>;
    type FloorPlanner = SimpleFloorPlanner;
    fn params(&self) -> Vec<usize> {
        self.0.clone()
    }
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
        Self::configure_with_params(meta, vec![1])
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, lengths: Vec<usize>) -> Self::Config {
        let columns = core::array::from_fn(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let glue = GlueConfig::configure(meta, columns, fixed);
        let public = lengths
            .into_iter()
            .map(|n| {
                let column = meta.instance_column(n);
                meta.enable_equality(column);
                column
            })
            .collect();
        (glue, public)
    }
    fn synthesize(
        &self,
        (config, public): Self::Config,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(config);
        let words = layouter.assign_region(
            || "descriptor export",
            |mut region| {
                (0..self.0.iter().sum::<usize>())
                    .map(|i| {
                        glue.witness(
                            &mut region,
                            self.1
                                .as_ref()
                                .map_or_else(Value::unknown, |v| Value::known(v[i])),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            },
        )?;
        let mut offset = 0;
        for (column, len) in public.into_iter().zip(&self.0) {
            for row in 0..*len {
                layouter.constrain_instance(words[offset + row].cell(), column, row)?;
            }
            offset += len;
        }
        Ok(())
    }
}

/// A descriptor verifier plan and key for public columns of `lengths`.
pub fn export_plan(
    params: &PinnedParams<Ep>,
    lengths: Vec<usize>,
    types: Vec<InstanceType>,
) -> (VerifierPlan<Ep>, VerifyingKey<Ep>) {
    let (binding, key) = keygen_vk_with_binding_v2(
        params,
        &Export(lengths, None),
        &KeygenConfigV2::pipa_r(types),
    )
    .expect("descriptor key");
    (
        VerifierPlan::new(binding, params.clone()).expect("verifier plan"),
        key,
    )
}

/// The incoming-only sigma plan over a k12 Vesta descriptor (only its schema is
/// used by steps that neither verify sigma nor read Q0).
pub fn sigma_plan() -> QSigmaPlan {
    let mut cs = ConstraintSystem::<Fp>::default();
    cs.instance_column(1);
    let layout = CircuitDescriptorV1::from_constraint_system(
        &cs.finalize(&[], false).expect("finalize"),
        DescriptorConfig {
            curve: CurveV1::Vesta,
            k: 12,
            transcript: TranscriptV1::KagemushaPoseidonRp57,
            instance_mode: InstanceModeV1::Direct,
            proof_suffix: ProofSuffixV1::FoldedGenerator,
        },
    )
    .expect("layout");
    let descriptor = CircuitDescriptorV2::from_layout(
        layout,
        TranscriptV2::KagemushaPoseidonRp57Base,
        vec![InstanceType::Bounded],
    )
    .expect("descriptor");
    let verifier = VerifierPlan::new(
        DescriptorBinding::new_v2(descriptor).expect("binding"),
        PinnedParams::<Eq>::derive(12).expect("params"),
    )
    .expect("sigma verifier");
    let incoming =
        SigmaClass::new(verifier, vec![(2, Fq::from(12)), (3, Fq::from(13))]).expect("class");
    QSigmaPlan::incoming_only(incoming, &PinnedParams::<Eq>::derive(16).expect("params"))
        .expect("sigma plan")
}

/// Today's Receive schedule over descriptor keys only, pinned for folding.
pub fn plan() -> Arc<ReceiveFoldPlan> {
    let params = PinnedParams::<Ep>::derive(16).expect("params");
    let (omega, omega_key) = export_plan(
        &params,
        vec![1, 2, 16],
        vec![
            InstanceType::Bounded,
            InstanceType::Field,
            InstanceType::Bounded,
        ],
    );
    plan_with(sigma_plan(), omega, omega_key)
}

/// The Receive schedule over `sigma` and the Omega program `omega`.
pub fn plan_with(
    sigma: QSigmaPlan,
    omega: VerifierPlan<Ep>,
    omega_key: VerifyingKey<Ep>,
) -> Arc<ReceiveFoldPlan> {
    let params = PinnedParams::<Ep>::derive(16).expect("params");
    let policy = OwnPolicy::new([31, 32], bootstrap_objects::key(23)).expect("policy");
    let (verifier, key) = export_plan(
        &params,
        sigma.instance_lengths().to_vec(),
        QSigmaPlan::instance_types().to_vec(),
    );
    let q0 = QProofPlan::new(verifier, key).expect("Q0 plan");
    let schema = ReceiveStagePlan::signature_schema_for(Variant::Receive, policy).expect("schema");
    let (verifier, key) = export_plan(
        &params,
        vec![schema.instance_length()],
        QSignaturePlan::instance_types().to_vec(),
    );
    let q1 = QProofPlan::new(verifier, key).expect("Q1 plan");
    let operation = AProofPlan::inline_with_incoming(
        Variant::Receive,
        sigma,
        vec![q0, q1],
        Some(omega),
        &params,
    )
    .expect("operation");
    let specs =
        ReceiveStagePlan::context_specs(Variant::Receive, MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES)
            .expect("specs");
    let context = OperationSchedule::for_variant(Variant::Receive)
        .bind(operation, specs)
        .expect("schedule");
    let stage = ReceiveStagePlan::new(context, policy).expect("stage plan");
    Arc::new(ReceiveFoldPlan::new(stage, policy, omega_key).expect("fold plan"))
}

/// The pinned trivial transported claims (every challenge one).
pub fn trivial_claims() -> (FoldInput<Ep>, AccumulatorT<Eq>) {
    let p = decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).expect("point");
    let v = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).expect("point");
    (
        FoldInput::from_normalized(p, 16, [Fq::ONE; 16]).expect("claim"),
        AccumulatorT::new(v, [Fp::ONE; 16]).expect("claim"),
    )
}

/// Transported claims that do not decide: the first k16 generator with every
/// challenge one (the true claim of those challenges is the sum of all
/// generators).
pub fn undecidable_claims() -> (AccumulatorT<Ep>, AccumulatorT<Eq>) {
    let p = PinnedParams::<Ep>::derive(16).expect("params").params().g()[0];
    let v = PinnedParams::<Eq>::derive(16).expect("params").params().g()[0];
    (
        AccumulatorT::new(p, [Fq::ONE; 16]).expect("claim"),
        AccumulatorT::new(v, [Fp::ONE; 16]).expect("claim"),
    )
}

/// `LE32(length) || raw`.
pub fn framed(raw: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(raw.len())
        .expect("length")
        .to_le_bytes()
        .to_vec();
    out.extend(raw);
    out
}

/// The commitment of an active original tape (`kgwcact1`), as the context
/// assigns it.
pub fn active_commitment(spec: ContextObjectSpec, digest: Fp, raw: &[u8]) -> [Fp; 3] {
    let domain = u64::from_le_bytes(*b"kgwcact1");
    let length = Fp::from(u64::try_from(raw.len()).expect("length"));
    let words = [
        Fp::from(u64::from(spec.tag)),
        Fp::from(u64::from(spec.capacity)),
        length,
        p_bytes_native(domain, &framed(raw)),
    ];
    [digest, length, hash_with_domain(domain, &words)]
}

/// The commitment of an exact (fixed-capacity) original tape (`kgwctap1`).
pub fn exact_commitment(spec: ContextObjectSpec, digest: Fp, raw: &[u8]) -> [Fp; 3] {
    let mut words = vec![
        Fp::from(u64::from(spec.tag)),
        Fp::from(u64::from(spec.capacity)),
    ];
    for chunk in framed(raw).chunks(31) {
        let mut repr = [0; 32];
        repr[..chunk.len()].copy_from_slice(chunk);
        words.push(Fp::from_repr(repr).expect("31-byte chunk"));
    }
    [
        digest,
        Fp::from(u64::try_from(raw.len()).expect("length")),
        hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &words),
    ]
}

/// The signed digest of a fixed signed object (body message, then the four
/// signature limbs).
pub fn signed_digest(kind: ObjectKind, raw: &[u8]) -> Fp {
    let end = kind.body_len();
    let mut words = vec![signed_message_native(kind, &raw[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            raw[end + offset..end + offset + 16]
                .try_into()
                .expect("limb"),
        )));
    }
    hash_with_domain(kind.object_domain(), &words)
}

/// The Payment transcript from the retained Request and payer credential.
pub fn payment_transcript(
    request: &[u8],
    payer: &[u8],
    statement: &[Fp; 26],
    proof_digest: Fp,
    receipt: Fp,
) -> Vec<u8> {
    let package = hash_with_domain(
        u64::from_le_bytes(*b"kgwpkg_1"),
        &[
            hash_with_domain(STATEMENT_DOMAIN, statement),
            proof_digest,
            receipt,
        ],
    );
    let mut payment = 1u16.to_le_bytes().to_vec();
    payment.extend(signed_digest(ObjectKind::Request, request).to_repr());
    payment.extend(&payer[130..195]);
    payment.extend(signed_digest(ObjectKind::Credential, payer).to_repr());
    payment.extend(package.to_repr());
    assert_eq!(payment.len(), 163);
    payment
}

/// A receipt over `statement` under the wallet `wallet`, signed by `secret`.
pub fn receipt(
    statement: &[Fp; 26],
    wallet: &[Fp],
    proof: Fp,
    payment: Fp,
    secret: u64,
    nonce: u64,
) -> bootstrap_objects::Signed {
    let operation = hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[wallet[0], wallet[1], statement[16], statement[17]],
    );
    let mut body = 1u16.to_le_bytes().to_vec();
    body.extend(bootstrap_objects::id(statement[3], statement[4]));
    body.extend(bootstrap_objects::id(wallet[0], wallet[1]));
    body.extend(bootstrap_objects::small_id(31, 32));
    body.extend(&statement[9].to_repr()[..16]);
    for word in [
        operation,
        statement[14],
        statement[15],
        hash_with_domain(STATEMENT_DOMAIN, statement),
        proof,
    ] {
        body.extend(word.to_repr());
    }
    body.extend(bootstrap_objects::small_id(501, 502));
    body.extend(payment.to_repr());
    bootstrap_objects::sign(ObjectKind::Receipt, body, secret, nonce)
}

/// A genuine `SHA256withECDSA` signature over the 32-byte encoding of `digest`.
pub fn p256_signature(digest: u64, secret: u64, nonce: u64) -> SignatureWitness {
    let digest = Fp::from(digest);
    let hash = sha256_of_digest(&digest);
    let message = core::array::from_fn(|i| {
        u64::from_be_bytes(
            hash[(3 - i) * 8..(4 - i) * 8]
                .try_into()
                .expect("eight bytes"),
        )
    });
    let secret = [
        secret,
        0x1234_5678_9abc_def0,
        0x0fed_cba9_8765_4321,
        0x0123_4567,
    ];
    let nonce = [
        nonce,
        0x0bad_cafe_dead_beef,
        0x1357_9bdf_2468_ace0,
        0x0765_4321,
    ];
    let key = p256::mul(&Affine::GENERATOR, &secret).expect("key");
    let r = ORDER.reduce_once(
        &p256::mul(&Affine::GENERATOR, &nonce)
            .expect("nonce point")
            .x,
    );
    let mut s = ORDER.mul(
        &ORDER.inverse(&nonce),
        &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)),
    );
    if p256::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(verify_prehashed(&message, &r, &s, &key));
    SignatureWitness {
        digest,
        key: [key.x, key.y],
        signature: [r, s],
    }
}

/// The 320 public bytes of a lineage's Omega transport.
pub fn public_bytes(fields: &[Fp; 18]) -> Vec<u8> {
    let mut out = 1u16.to_le_bytes().to_vec();
    for i in [1, 2, 3, 4] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[5].to_repr());
    for i in [6, 7] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[8].to_repr());
    out.push(4);
    for i in [10, 9, 12, 11] {
        out.extend(fields[i].to_repr()[..16].iter().rev());
    }
    out.extend(&fields[13].to_repr()[..13]);
    out.extend(&fields[14].to_repr()[..16]);
    out.extend(fields[15].to_repr());
    out.extend(fields[16].to_repr());
    assert_eq!(out.len(), 320);
    out
}

/// The keys of the genuine Receive.
pub struct ReceiveKeys {
    /// Today's Receive plan over the descriptor keys and the sigma plan below.
    pub plan: Arc<ReceiveFoldPlan>,
    /// The payer's k12 `σ_send` prover.
    pub sigma: SigmaProver<Eq>,
    /// The incoming-only Q0 plan over its verifier.
    pub sigma_plan: QSigmaPlan,
    /// The Omega program's key (public columns `D_A`, `acc_V`'s coordinates
    /// and challenges).
    pub omega: ProvingKey<Ep>,
    /// Its digest, the lineage's Omega-key field.
    pub omega_digest: Fp,
}

/// Generates the [`ReceiveKeys`].
#[must_use]
pub fn receive_keys() -> ReceiveKeys {
    let params = PinnedParams::<Ep>::derive(16).expect("params");
    let sigma = SigmaProver::<Eq>::keygen_with_params(
        common::pinned_shape(common::folded(SigmaRelation::SEND), (12, 1)),
        common::vesta_params(12),
    )
    .expect("sigma key");
    let sigma_plan = QSigmaPlan::incoming_only(
        SigmaClass::from_verifiers(&[(sigma_selector(3, 0).expect("selector"), &sigma.verifier())])
            .expect("class"),
        &common::vesta_params(16),
    )
    .expect("sigma plan");
    let config = KeygenConfigV2::pipa_r(vec![
        InstanceType::Bounded,
        InstanceType::Field,
        InstanceType::Bounded,
    ]);
    let omega = keygen_pk_v2(&params, &Export(vec![1, 2, 16], None), &config).expect("Omega key");
    let omega_plan = VerifierPlan::new(omega.binding().clone(), params).expect("Omega plan");
    let omega_digest = omega
        .vk()
        .kagemusha_digest(omega.binding())
        .expect("key digest");
    let plan = plan_with(sigma_plan.clone(), omega_plan, omega.vk().clone());
    ReceiveKeys {
        plan,
        sigma,
        sigma_plan,
        omega,
        omega_digest,
    }
}

/// One genuine Receive.
#[derive(Clone)]
pub struct FullReceive {
    /// Its originals.
    pub source: ReceiveFoldSource,
    /// Q0's prepared incoming slot (the Accept mode).
    pub sigma: FoldedQSigma,
    /// Q1's slots: the own receipt, the Send receipt, the Request.
    pub signatures: Vec<SignatureWitness>,
    /// The receiver's last Omega (over the Receive's predecessor lineage), on
    /// which the segment rebases.
    pub rebase: RebaseSource,
}

/// An Omega proof under the fixture's Omega key over `D_A` of `lineage` and
/// `acc_p`, with `acc_v` in its instance: the proof bytes and the opening claim
/// its verification defers.
fn omega_proof(
    params: &PinnedParams<Ep>,
    keys: &ReceiveKeys,
    lineage: &[Fp; 18],
    acc_p: &AccumulatorT<Ep>,
    acc_v: &AccumulatorT<Eq>,
) -> (Vec<u8>, FoldInput<Ep>) {
    let mut words = lineage.to_vec();
    let (x, y) = generator_coordinates::<Ep>(acc_p.g());
    words.extend([x, y]);
    for challenge in acc_p.challenges() {
        words.extend(foreign_limbs(challenge).map(Fp::from_u128));
    }
    let d_a = hash_with_domain(LINEAGE_DOMAIN, &words);
    let (vx, vy) = generator_coordinates::<Eq>(acc_v.g());
    let instances = vec![
        vec![Fq::from_repr(d_a.to_repr()).expect("D_A fits")],
        vec![vx, vy],
        acc_v
            .challenges()
            .iter()
            .map(|c| Fq::from_repr(c.to_repr()).expect("challenge fits"))
            .collect::<Vec<_>>(),
    ];
    let values = instances.iter().flatten().copied().collect::<Vec<_>>();
    let output = iroha_plonk::create_proof_owned_with_claim(
        params,
        &keys.omega,
        iroha_plonk::Witness::from_circuit(
            &keys.omega,
            &Export(vec![1, 2, 16], Some(values)),
            &instances,
        )
        .expect("witness"),
        common::fresh_randomness(),
        iroha_plonk::ProverConfig::default(),
    )
    .expect("Omega proof");
    let claim = iroha_plonk::verifier::accumulate_generator(
        params,
        keys.omega.binding(),
        keys.omega.vk(),
        &instances,
        &output.proof,
        MemoryBudget::DEFAULT,
    )
    .expect("Omega verifies");
    (
        output.proof.to_vec(),
        FoldInput::from_opening(*claim.g(), claim.challenges()).expect("opening"),
    )
}

/// A genuine Receive: a real Send sigma, the payer's Omega proof over `D_A` of
/// its lineage and `transported` (`acc_P`, `acc_V`; the trivial claims when
/// `None`), today's transport bytes, the Send receipt, Payment, Request and
/// credentials over those tapes, the own receipt under the receiver's key, Q0's
/// slot and Q1's signatures. Every incoming result is true and every mode
/// Accept, so `transported` claims that do not decide pass every step and
/// only the claim decider refuses them.
#[must_use]
pub fn full_receive(
    keys: &ReceiveKeys,
    transported: Option<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
) -> FullReceive {
    full_receive_with(keys, transported, None)
}

/// [`full_receive`] whose segment rebases on a predecessor Omega transporting
/// `predecessor` (`acc_P`, `acc_V`; the trivial claims when `None`).
#[must_use]
pub fn full_receive_with(
    keys: &ReceiveKeys,
    transported: Option<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
    predecessor: Option<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
) -> FullReceive {
    let params = PinnedParams::<Ep>::derive(16).expect("params");
    let vparams = common::vesta_params(16);
    let plan = &keys.plan;
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    let send = send_objects::from_load(&loaded.successor);
    // The incoming Send sigma.
    let proof = keys
        .sigma
        .prove(&send.step, common::fresh_randomness())
        .expect("sigma proof");
    let sigma_tape = proof.bytes.clone();
    let sigma_witness = SigmaSlotWitness {
        key: keys.sigma.proving_key().vk().clone(),
        statement: proof.public.instance()[0],
        length: sigma_tape.len().try_into().expect("length"),
        proof: sigma_tape.clone(),
    };
    // The payer's Omega proof.
    let mut payer_lineage = send.before.lineage;
    payer_lineage[17] = keys.omega_digest;
    let (p, v) = trivial_claims();
    let trivial = || {
        (
            AccumulatorT::new(*p.g(), *p.challenges()).expect("claim"),
            v.clone(),
        )
    };
    let (acc_p, acc_v) = transported.unwrap_or_else(&trivial);
    let (omega_bytes, opening) = omega_proof(&params, keys, &payer_lineage, &acc_p, &acc_v);
    let mut omega_tape = public_bytes(&payer_lineage);
    omega_tape.extend(&omega_bytes);
    omega_tape.extend(acc_p.to_bytes());
    omega_tape.extend(acc_v.to_bytes());
    // The Send package over both tapes, then the own transition and receipts.
    let proof_digest = p_bytes_native(
        u64::from_le_bytes(*b"kgwprf_1"),
        &[framed(&omega_tape), framed(&sigma_tape)].concat(),
    );
    let send_receipt = receipt(
        &send.statement,
        &send.before.lineage[6..8],
        proof_digest,
        Fp::ZERO,
        29,
        71,
    );
    let request = send.objects[1].clone();
    let payer_credential = send.objects[0].clone();
    let payment = payment_transcript(
        &request,
        &payer_credential,
        &send.statement,
        proof_digest,
        send_receipt.digest(),
    );
    let payment_digest = p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &payment);
    let (receiver, certificate, credential) =
        bootstrap_objects::enrollment_for(bootstrap_objects::Identity::Receiver);
    let before = StateWitness::from(&receiver);
    let mut fixture = receive_objects::from_send(&send, &before, payment_digest, true, true);
    fixture.before.lineage[17] = keys.omega_digest;
    fixture.after.lineage[17] = keys.omega_digest;
    let own_proof = p_bytes_native(
        u64::from_le_bytes(*b"kgwstep1"),
        &framed(b"retained own sigma"),
    );
    let own_receipt = receipt(
        &fixture.statement,
        &before.lineage[6..8],
        own_proof,
        payment_digest,
        43,
        73,
    );
    let request_signature = {
        let signed = bootstrap_objects::sign(
            ObjectKind::Request,
            request[..ObjectKind::Request.body_len()].to_vec(),
            43,
            59,
        );
        assert_eq!(signed.bytes, request);
        signed.signature
    };
    let signatures = vec![
        own_receipt.signature,
        send_receipt.signature,
        request_signature,
    ];
    let sigma = keys
        .sigma_plan
        .prepare_folded(
            IncomingSigma {
                sigma: sigma_witness,
                mode: IncomingMode::Accept,
            },
            &vparams,
            &FoldConfig::default(),
        )
        .expect("Q0 slot");
    let objects: [Vec<u8>; 9] = [
        request,
        payer_credential,
        send_receipt.bytes,
        payment,
        omega_tape,
        sigma_tape,
        own_receipt.bytes,
        credential.bytes,
        certificate.bytes,
    ];
    let kinds = [
        Some(ObjectKind::Request),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Receipt),
        None,
        None,
        None,
        Some(ObjectKind::Receipt),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
    ];
    let sigma_digest = p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &framed(&objects[5]));
    let commitments = plan
        .stage()
        .context()
        .object_specs()
        .iter()
        .enumerate()
        .map(|(i, spec)| match (i, kinds[i]) {
            (_, Some(kind)) => {
                exact_commitment(*spec, signed_digest(kind, &objects[i]), &objects[i])
            }
            (3, None) => exact_commitment(*spec, payment_digest, &objects[i]),
            (4, None) => active_commitment(*spec, proof_digest, &objects[i]),
            (5, None) => active_commitment(*spec, sigma_digest, &objects[i]),
            _ => unreachable!("nine fixed slots"),
        })
        .collect();
    let incoming = IncomingWitness {
        public: payer_lineage,
        public_valid: true,
        pallas: acc_p,
        vesta: acc_v.clone(),
        opening,
        results: [true; 4],
        modes: [IncomingMode::Accept; 4],
        pallas_corrections: [*p.g(); 2],
        vesta_correction: *v.g(),
    };
    let source = ReceiveFoldSource::new(
        plan,
        Transition {
            before: fixture.before,
            after: fixture.after,
            statement: fixture.statement,
            consumed: fixture.consumed,
            credit: fixture.credit,
            insert: fixture.insert,
        },
        send.statement,
        fixture.step,
        objects,
        commitments,
        incoming,
    )
    .expect("genuine full source");
    // The receiver's own last Omega, over the Receive's predecessor lineage.
    let (pred_p, pred_v) = predecessor.unwrap_or_else(&trivial);
    let lineage = source.transition.before.lineage;
    let (proof, opening) = omega_proof(&params, keys, &lineage, &pred_p, &pred_v);
    let rebase = RebaseSource {
        lineage,
        pallas: pred_p,
        vesta: pred_v,
        proof,
        opening,
    };
    FullReceive {
        source,
        sigma,
        signatures,
        rebase,
    }
}

/// The eight step relations of the folded segment and their keys.
pub struct ReceiveRelations {
    /// The A relations in Q's digest order (A1, A3, A4, A5, Rebase).
    pub a: Vec<FoldedRelation<Fp>>,
    /// The Q relations in A's digest order (Frame, Sigma, Signature).
    pub q: Vec<FoldedRelation<Fq>>,
    /// The fold shape every A relation shares.
    pub a_shape: FoldShape,
    /// A's commitment key (monomial generators of `params_a`).
    pub key_a: CommitKey<Eq>,
    /// Q's commitment key (monomial generators of `params_q`).
    pub key_q: CommitKey<Ep>,
    /// k16 Vesta parameters: A instances and Q's claims.
    pub params_a: PinnedParams<Eq>,
    /// k16 Pallas parameters: Q instances and A's claims.
    pub params_q: PinnedParams<Ep>,
    /// The segment's base A state ([`segment_base`]).
    pub base: Vec<Fp>,
}

fn relation_of<F: PastaField, C: Circuit<F>>(circuit: &C) -> FoldedRelation<F> {
    FoldedRelation::from_synthesized(&synthesize(circuit, K, None).expect("layout"))
        .expect("relation")
}

/// The eight relations of `full`'s segment (they depend on the plan and the Q
/// leaves' plans only).
#[must_use]
pub fn receive_relations(keys: &ReceiveKeys, full: &FullReceive) -> ReceiveRelations {
    let plan = &keys.plan;
    let a_relation = |kind: AKind, shape: FoldShape| match kind {
        AKind::Rebase => relation_of(&RebaseFoldStep::new(plan.clone(), shape, None)),
        _ => relation_of(&ReceiveFoldStep::new(plan.clone(), kind, shape, None).expect("A step")),
    };
    // An A relation's shape does not depend on the Q shape it folds.
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![2],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_shape = a_relation(AKind::Receive1, placeholder).shape();
    let schema = plan
        .stage()
        .signature_schema(ReceiveStagePlan::SIGNATURE_Q)
        .expect("schema")
        .clone();
    let q = vec![
        relation_of(&QFrameStep {
            shape: a_shape.clone(),
            witness: None,
        }),
        relation_of(&QSigmaStep {
            leaf: full.sigma.circuit.without_witnesses(),
            shape: a_shape.clone(),
            witness: None,
        }),
        relation_of(&QSignatureStep {
            leaf: QSignatureCircuit::new(schema, full.signatures.clone())
                .expect("leaf")
                .without_witnesses(),
            shape: a_shape.clone(),
            frame: None,
        }),
    ];
    let a: Vec<_> = AKind::ALL
        .into_iter()
        .map(|kind| a_relation(kind, q[kind.folds() as usize].shape()))
        .collect();
    assert!(
        a.iter().all(|relation| relation.shape() == a_shape),
        "every A kind shares one fold shape"
    );
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    ReceiveRelations {
        key_a: CommitKey::monomial(params_a.params()),
        key_q: CommitKey::monomial(params_q.params()),
        a,
        q,
        a_shape,
        params_a,
        params_q,
        base: segment_base(),
    }
}

/// One half-step of a Receive: a Q kind with the A kind it folds, or an A kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalfStep {
    /// A Q step folding the preceding A instance of that kind.
    Q(QKind, AKind),
    /// An A step.
    A(AKind),
}

/// A segment's ten half-steps: the first Frame opens the segment (its slot is
/// unused there) and the Rebase moves it to the predecessor Omega's lineage;
/// then the Receive's eight, its first Frame folding the Rebase.
pub const SCHEDULE: [HalfStep; 10] = [
    HalfStep::Q(QKind::Frame, AKind::Receive5),
    HalfStep::A(AKind::Rebase),
    HalfStep::Q(QKind::Frame, AKind::Rebase),
    HalfStep::A(AKind::Receive1),
    HalfStep::Q(QKind::Sigma, AKind::Receive1),
    HalfStep::A(AKind::Receive3),
    HalfStep::Q(QKind::Signature, AKind::Receive3),
    HalfStep::A(AKind::Receive4),
    HalfStep::Q(QKind::Frame, AKind::Receive4),
    HalfStep::A(AKind::Receive5),
];

/// A chain after some half-steps of [`SCHEDULE`].
#[derive(Clone)]
pub struct Chain {
    /// A's accumulators of Q instances, in kind order.
    pub a_kept: Vec<Option<Kept<Ep>>>,
    /// Q's accumulators of A instances, in kind order.
    pub q_kept: Vec<Option<Kept<Eq>>>,
    /// A's claim accumulator (the predecessor's and the incoming Omega's
    /// Pallas claims) and its witness.
    pub claims_a: Option<(ClaimAccumulator<Ep>, Vec<Fq>)>,
    /// Q's claim accumulator (the Frame steps' export claims, among them the
    /// predecessor's `acc_V`; the `σ_send` opening and the incoming `acc_V`) and
    /// its witness.
    pub claims_q: Option<(ClaimAccumulator<Eq>, Vec<Fp>)>,
    /// The last A instance.
    pub last_a: Option<Fresh<Eq>>,
    /// The last Q instance.
    pub last_q: Option<Fresh<Ep>>,
    /// The last half-step (its instance is the one no step has folded yet).
    pub last: Option<HalfStep>,
    /// A's digest list after its last step.
    pub a_digests: Vec<Fp>,
    /// Q's digest list after its last step.
    pub q_digests: Vec<Fq>,
    /// The A state after A's last step.
    pub a_state: Vec<Fp>,
    /// Every A step's public columns, in order.
    pub a_public: Vec<Vec<Vec<Fp>>>,
    /// Every Q step's public columns, in order.
    pub q_public: Vec<Vec<Vec<Fq>>>,
}

fn commit_q<C: Circuit<Fq>>(
    rel: &ReceiveRelations,
    kind: QKind,
    step: &C,
    public: &[Vec<Fq>],
    rng: &mut ChaCha20Rng,
) -> Fresh<Ep> {
    let report = check_circuit(step, K, public, CheckMode::Strict).expect("Q synthesis");
    assert!(
        report.is_satisfied(),
        "Q {kind:?}: {:?}",
        &report.failures()[..report.failures().len().min(4)]
    );
    let relation = &rel.q[kind as usize];
    let (advice, staged) = relation
        .stage_witness(&synthesize(step, K, Some(public)).expect("Q witness"))
        .expect("stage");
    fresh_instance(relation, &rel.key_q, &FOLD_Q, advice, staged, rng).expect("Q instance")
}

fn q_half_step(
    keys: &ReceiveKeys,
    rel: &ReceiveRelations,
    full: &FullReceive,
    chain: &mut Chain,
    j: u64,
    kind: QKind,
    folds: AKind,
    rng: &mut ChaCha20Rng,
) {
    let plan = &keys.plan;
    let side = kind.chain(folds);
    let slot = folds as usize;
    let mut prepared = prepare_step(
        &side,
        j,
        &rel.a[slot],
        chain.q_kept[slot].as_ref(),
        chain.last_a.as_ref(),
        vec![],
        chain.q_digests.clone(),
        segment_start(),
    )
    .expect("prepare Q");
    let mut digests = prepared.digests.clone();
    let mut public = Vec::new();
    let fresh = match kind {
        QKind::Frame => {
            // The claim the folded A instance exports: the predecessor's acc_V
            // after the Rebase, the trivial claim otherwise. The segment's first
            // instance folds a placeholder, which carries the trivial export.
            let selected = if chain.last_a.is_some() && folds == AKind::Rebase {
                full.rebase.vesta_claim()
            } else {
                trivial_vesta()
            };
            if chain.last_a.is_none() {
                prepared.witness.fresh.public[1] = trivial_export();
            }
            let kept = chain.claims_q.as_ref().map(|(acc, s)| (acc, s.as_slice()));
            let (acc, s, claims) =
                fold_claims(&CLAIM_Q, kept, &[claim_of(&selected)]).expect("Q claim fold");
            digests[Q_CLAIM_SLOT] = acc.digest(&CLAIM_Q);
            chain.claims_q = Some((acc, s));
            public.push(step_public(
                &side,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[],
                &digests,
                &[],
            ));
            let witness = QFrameStepWitness {
                frame: prepared.witness.clone(),
                selected,
                claims,
            };
            let step = QFrameStep {
                shape: rel.a_shape.clone(),
                witness: Some(witness),
            };
            commit_q(rel, kind, &step, &public, rng)
        }
        QKind::Sigma => {
            let incoming = plan
                .incoming_vesta_claim(&full.source)
                .expect("incoming acc_V");
            let kept = chain.claims_q.as_ref().map(|(acc, s)| (acc, s.as_slice()));
            let (acc, s, claims) =
                fold_selected(kept, &full.sigma.selected, &incoming).expect("Q claim folds");
            digests[Q_CLAIM_SLOT] = acc.digest(&CLAIM_Q);
            public.push(step_public(
                &side,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[],
                &digests,
                &[],
            ));
            public.extend(full.sigma.instances.clone());
            let witness = QSigmaStepWitness {
                frame: prepared.witness.clone(),
                selected: full.sigma.selected.clone(),
                incoming,
                claims,
            };
            let step = QSigmaStep {
                leaf: full.sigma.circuit.clone(),
                shape: rel.a_shape.clone(),
                witness: Some(witness),
            };
            chain.claims_q = Some((acc, s));
            commit_q(rel, kind, &step, &public, rng)
        }
        QKind::Signature => {
            let schema = plan
                .stage()
                .signature_schema(ReceiveStagePlan::SIGNATURE_Q)
                .expect("schema")
                .clone();
            let [column] = schema
                .native_instances(&full.signatures)
                .expect("Q1 column");
            public.push(step_public(
                &side,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[],
                &digests,
                &[],
            ));
            public.push(column);
            let leaf = QSignatureCircuit::new(schema, full.signatures.clone()).expect("leaf");
            let step = QSignatureStep {
                leaf,
                shape: rel.a_shape.clone(),
                frame: Some(prepared.witness.clone()),
            };
            commit_q(rel, kind, &step, &public, rng)
        }
    };
    if chain.last_a.is_some() {
        chain.q_kept[slot] = prepared.kept.take();
    }
    chain.q_digests = digests;
    chain.q_public.push(public);
    chain.last_q = Some(fresh);
}

fn commit_a<C: Circuit<Fp>>(
    rel: &ReceiveRelations,
    kind: AKind,
    step: &C,
    public: &[Vec<Fp>],
    rng: &mut ChaCha20Rng,
) -> Fresh<Eq> {
    let report = check_circuit(step, K, public, CheckMode::Strict).expect("A synthesis");
    assert!(
        report.is_satisfied(),
        "A {kind:?}: {:?}",
        &report.failures()[..report.failures().len().min(4)]
    );
    let relation = &rel.a[kind as usize];
    let (advice, staged) = relation
        .stage_witness(&synthesize(step, K, Some(public)).expect("A witness"))
        .expect("stage");
    fresh_instance(relation, &rel.key_a, &FOLD_A, advice, staged, rng).expect("A instance")
}

/// The Rebase: hard-verifies the receiver's last Omega, folds its Pallas claims
/// into A's claim accumulator and moves the segment base to its lineage.
fn rebase_half_step(
    keys: &ReceiveKeys,
    rel: &ReceiveRelations,
    full: &FullReceive,
    chain: &mut Chain,
    j: u64,
    rng: &mut ChaCha20Rng,
) {
    let kind = AKind::Rebase;
    let side = kind.chain();
    let slot = kind.folds() as usize;
    let mut prepared = prepare_step(
        &side,
        j,
        &rel.q[slot],
        chain.a_kept[slot].as_ref(),
        chain.last_q.as_ref(),
        chain.a_state.clone(),
        chain.a_digests.clone(),
        Fp::ZERO,
    )
    .expect("prepare Rebase");
    let rebase = &full.rebase;
    let next = rebase.next_state();
    let mut digests = prepared.digests.clone();
    let claims = rebase.pallas_claims().map(|c| claim_of(&c));
    let kept = chain.claims_a.as_ref().map(|(acc, s)| (acc, s.as_slice()));
    let (acc, s, folds) = fold_claims(&CLAIM_A, kept, &claims).expect("A claim folds");
    digests[A_CLAIM_SLOT] = acc.digest(&CLAIM_A);
    chain.claims_a = Some((acc, s));
    let public = vec![
        step_public(
            &side,
            j,
            Fp::from(BASE),
            prepared.passthrough,
            &next,
            &digests,
            &[],
        ),
        rebase.export(),
    ];
    let witness = RebaseStepWitness {
        source: Arc::new(rebase.clone()),
        frame: prepared.witness.clone(),
        claims: folds,
    };
    let step = RebaseFoldStep::new(keys.plan.clone(), rel.q[slot].shape(), Some(witness));
    let fresh = commit_a(rel, kind, &step, &public, rng);
    if chain.last_q.is_some() {
        chain.a_kept[slot] = prepared.kept.take();
    }
    chain.a_digests = digests;
    chain.a_state = next;
    chain.a_public.push(public);
    chain.last_a = Some(fresh);
}

fn a_half_step(
    keys: &ReceiveKeys,
    rel: &ReceiveRelations,
    full: &FullReceive,
    chain: &mut Chain,
    j: u64,
    kind: AKind,
    rng: &mut ChaCha20Rng,
) {
    if kind == AKind::Rebase {
        return rebase_half_step(keys, rel, full, chain, j, rng);
    }
    let plan = &keys.plan;
    let side = kind.chain();
    let slot = kind.folds() as usize;
    let mut prepared = prepare_step(
        &side,
        j,
        &rel.q[slot],
        chain.a_kept[slot].as_ref(),
        chain.last_q.as_ref(),
        chain.a_state.clone(),
        chain.a_digests.clone(),
        Fp::ZERO,
    )
    .expect("prepare A");
    let next = plan
        .next_state(kind, &full.source, &chain.a_state)
        .expect("next A state");
    let mut digests = prepared.digests.clone();
    // A5 folds the incoming Pallas claims into A's claim accumulator.
    let claims = (kind == AKind::Receive5).then(|| {
        let claims = plan
            .incoming_pallas_claims(&full.source)
            .expect("incoming Pallas claims")
            .map(|c| claim_of(&c));
        let kept = chain.claims_a.as_ref().map(|(acc, s)| (acc, s.as_slice()));
        let (acc, s, folds) = fold_claims(&CLAIM_A, kept, &claims).expect("A claim folds");
        digests[A_CLAIM_SLOT] = acc.digest(&CLAIM_A);
        chain.claims_a = Some((acc, s));
        folds
    });
    let public = vec![
        step_public(
            &side,
            j,
            Fp::from(BASE),
            prepared.passthrough,
            &next,
            &digests,
            &[],
        ),
        plan.export(kind, &full.source),
    ];
    let witness = ReceiveStepWitness {
        source: Arc::new(full.source.clone()),
        frame: prepared.witness.clone(),
        claims,
    };
    let step = ReceiveFoldStep::new(plan.clone(), kind, rel.q[slot].shape(), Some(witness))
        .expect("A step");
    let fresh = commit_a(rel, kind, &step, &public, rng);
    if chain.last_q.is_some() {
        chain.a_kept[slot] = prepared.kept.take();
    }
    chain.a_digests = digests;
    chain.a_state = next;
    chain.a_public.push(public);
    chain.last_a = Some(fresh);
}

/// Runs the first `steps` half-steps of [`SCHEDULE`] from the segment base:
/// the Rebase on the receiver's last Omega, then `full`'s Receive (each checked
/// strictly, committed under the monomial keys and folded by the next).
#[must_use]
pub fn run_chain(
    keys: &ReceiveKeys,
    rel: &ReceiveRelations,
    full: &FullReceive,
    steps: usize,
) -> Chain {
    let mut rng = ChaCha20Rng::seed_from_u64(61);
    let mut chain = Chain {
        a_kept: vec![None; QKind::ALL.len()],
        q_kept: vec![None; AKind::ALL.len()],
        claims_a: None,
        claims_q: None,
        last_a: None,
        last_q: None,
        last: None,
        a_digests: vec![Fp::ZERO; A_DIGESTS],
        q_digests: vec![Fq::ZERO; Q_DIGESTS],
        a_state: rel.base.clone(),
        a_public: Vec::new(),
        q_public: Vec::new(),
    };
    for (i, step) in SCHEDULE.iter().take(steps).enumerate() {
        let j = u64::try_from(i + 1).expect("step");
        match *step {
            HalfStep::Q(kind, folds) => {
                q_half_step(keys, rel, full, &mut chain, j, kind, folds, &mut rng)
            }
            HalfStep::A(kind) => a_half_step(keys, rel, full, &mut chain, j, kind, &mut rng),
        }
        chain.last = Some(*step);
    }
    chain
}

/// Decides one field's accumulators through decider v2 with step bound
/// `step_runs`: one batched proof, verified; its batched opening, verified; and
/// the opening's generator claim.
///
/// # Errors
/// The failing stage.
pub fn decide_batch<C: PastaCurve>(
    domain: &[u8; 32],
    relations: &[FoldedRelation<C::ScalarExt>],
    (key, step_runs): (&CommitKey<C>, usize),
    params: &ParamsIpa<C>,
    inputs: &[(usize, &Accumulator<C>, &Witness<C::ScalarExt>)],
) -> Result<(), String> {
    let plans = inputs
        .iter()
        .map(|(kind, _, _)| DeciderV2Plan::with_step_runs(relations[*kind].clone(), key, step_runs))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("decider plan: {e:?}"))?;
    let batch: Vec<_> = plans
        .iter()
        .zip(inputs)
        .map(|(plan, (_, acc, witness))| (plan, *acc, *witness))
        .collect();
    let mut proved =
        decider_v2::prove_many(domain, &batch).map_err(|e| format!("decider proof: {e:?}"))?;
    let (opening, claim) =
        decider_v2::open(params, &mut proved).map_err(|e| format!("opening: {e:?}"))?;
    let accs: Vec<_> = plans
        .iter()
        .zip(inputs)
        .map(|(plan, (_, acc, _))| (plan, *acc))
        .collect();
    let mut verified = decider_v2::verify_many(domain, &accs, &proved.proof)
        .map_err(|e| format!("decider verify: {e:?}"))?;
    let checked = decider_v2::verify_opening(params, &mut verified, &opening)
        .map_err(|e| format!("opening verify: {e:?}"))?;
    if checked != claim {
        return Err("the verified opening claim differs".into());
    }
    let claim =
        Claim::from_generator_claim(&checked).map_err(|e| format!("opening claim: {e:?}"))?;
    let (acc, s) = ClaimAccumulator::from_claim(domain, &claim);
    decide_claims(params.g(), &acc, &s).map_err(|e| format!("opening claim decides: {e:?}"))
}

/// What deciding a chain found.
#[derive(Debug)]
pub struct Decision {
    /// Q's relations (A's accumulators of Q instances, the last Q instance
    /// started or folded into its kind's).
    pub q: Result<(), String>,
    /// A's relations (Q's accumulators of A instances, the last A instance
    /// started or folded into its kind's, as Omega does).
    pub a: Result<(), String>,
    /// A's claim accumulator (the predecessor's and the incoming Pallas claims).
    pub claims_a: Result<(), String>,
    /// Q's claim accumulator (the export claims, `σ_send` and the incoming
    /// `acc_V`).
    pub claims_q: Result<(), String>,
}

impl Decision {
    /// Whether everything decided.
    #[must_use]
    pub fn all(&self) -> bool {
        self.q.is_ok() && self.a.is_ok() && self.claims_a.is_ok() && self.claims_q.is_ok()
    }
}

/// Decides everything `chain` leaves to the compression.
#[must_use]
pub fn decide_chain(rel: &ReceiveRelations, chain: &Chain) -> Decision {
    let mut a_kept = chain.a_kept.clone();
    let mut q_kept = chain.q_kept.clone();
    match chain.last {
        Some(HalfStep::A(kind)) => {
            let (instance, witness) = chain.last_a.as_ref().expect("last A");
            let kept = q_kept[kind as usize].as_ref().map(|(acc, w)| (acc, w));
            let (acc, w, _) =
                fold_or_start(&rel.a[kind as usize], &FOLD_A, kept, instance, witness)
                    .expect("fold the last A");
            q_kept[kind as usize] = Some((acc, w));
        }
        Some(HalfStep::Q(kind, _)) => {
            let (instance, witness) = chain.last_q.as_ref().expect("last Q");
            let kept = a_kept[kind as usize].as_ref().map(|(acc, w)| (acc, w));
            let (acc, w, _) =
                fold_or_start(&rel.q[kind as usize], &FOLD_Q, kept, instance, witness)
                    .expect("fold the last Q");
            a_kept[kind as usize] = Some((acc, w));
        }
        None => {}
    }
    let q_inputs: Vec<_> = a_kept
        .iter()
        .enumerate()
        .filter_map(|(i, k)| k.as_ref().map(|(acc, w)| (i, acc, w)))
        .collect();
    let a_inputs: Vec<_> = q_kept
        .iter()
        .enumerate()
        .filter_map(|(i, k)| k.as_ref().map(|(acc, w)| (i, acc, w)))
        .collect();
    let claims = |name: &str, result: Option<Result<(), iroha_fold::Error>>| match result {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(format!("{name}: {e:?}")),
        None => Err(format!("{name}: absent")),
    };
    Decision {
        q: decide_batch(
            &FOLD_Q,
            &rel.q,
            (&rel.key_q, iroha_fold::decider_v2::STEP_RUNS),
            rel.params_q.params(),
            &q_inputs,
        ),
        a: decide_batch(
            &FOLD_A,
            &rel.a,
            (&rel.key_a, A_STEP_RUNS),
            rel.params_a.params(),
            &a_inputs,
        ),
        claims_a: claims(
            "A claims",
            chain
                .claims_a
                .as_ref()
                .map(|(acc, s)| decide_claims(rel.params_q.params().g(), acc, s)),
        ),
        claims_q: claims(
            "Q claims",
            chain
                .claims_q
                .as_ref()
                .map(|(acc, s)| decide_claims(rel.params_a.params().g(), acc, s)),
        ),
    }
}
