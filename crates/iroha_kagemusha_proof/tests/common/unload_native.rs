//! Genuine original-only native Unload fixture, compiled only by the private unit tests.
//! It consumes the existing signed, funded compact Bootstrap/Load chain. Fixture key
//! generation is test-only; no Prepared or accepted claim is fabricated for installation.

#[path = "bootstrap.rs"]
mod bootstrap;
#[path = "bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
#[path = "mod.rs"]
mod common;
#[path = "proof_fixtures/compact_catalog.rs"]
mod compact_catalog;
#[path = "load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

use super::super::*;
use crate::{
    admin_sigma::UnloadCircuit,
    operation_relation::{
        administrative::NULLIFIER_DOMAIN,
        map_effects::{LOAD_DOMAIN, REDEEM_DOMAIN},
    },
    q_sigma::{QSigmaPlan, SigmaClass, SigmaSlotWitness, native::QSigmaProver},
    q_signature::QSignatureCircuit,
    tree::IndexedTree,
    witness::{CORE_DOMAIN, REST_DOMAIN, core_index as core},
};
use iroha_pasta::PastaCurve;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
    keys::{KeygenConfigV2, keygen_pk_v2},
};
use iroha_plonk_gadgets::statement::STATEMENT_DOMAIN;

fn rebind(w: &mut ConsumingWitness) {
    for state in [&mut w.predecessor, &mut w.successor] {
        let mut preimage = state.core.to_vec();
        preimage.push(hash_with_domain(REST_DOMAIN, &state.rest));
        state.lineage[5] = hash_with_domain(CORE_DOMAIN, &preimage);
        state.lineage[1..3].copy_from_slice(&state.core[core::SCHEME..core::SCHEME + 2]);
        state.lineage[6..8].copy_from_slice(&state.core[core::WALLET..core::WALLET + 2]);
        state.lineage[8] = state.core[core::CREDENTIAL];
        state.lineage[13] = state.core[core::LIFECYCLE]
            + Fp::from(256) * state.core[core::POLICY_EPOCH]
            + Fp::from_u128(1 << 72) * state.core[core::ENABLED_CONTROLS];
    }
    w.statement[1..3].copy_from_slice(&w.successor.lineage[3..5]);
    w.statement[3..7].copy_from_slice(&w.successor.core[core::SCHEME..core::ASSET + 2]);
    for (field, index) in [
        (7, core::CREDENTIAL),
        (8, core::LIFECYCLE),
        (9, core::SEQUENCE),
        (10, core::NEXT_LOAD),
    ] {
        w.statement[field] = w.successor.core[index];
    }
    w.statement[12] = w.predecessor.lineage[14];
    w.statement[13] = w.predecessor.lineage[15];
    w.statement[14] = w.predecessor.lineage[5];
    w.statement[15] = w.successor.lineage[5];
}

fn genuine_input() -> (Plan, Inputs) {
    let budget = MemoryBudget::DEFAULT;
    let funded = compact_catalog::compact_payer_load();
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let vparams = PinnedParams::<Eq>::derive(16).unwrap();
    verify_full(
        &params,
        &funded.binding,
        &funded.key,
        &funded.instances,
        &funded.proof,
        budget,
    )
    .unwrap();
    funded.source.pallas.decide(&params, budget).unwrap();
    funded.vesta.decide(&vparams, budget).unwrap();
    assert_eq!(funded.proof.len(), 3_712);
    assert_eq!(funded.proof.len() + 1_088, 4_800);
    assert!(funded.proof.len() + 1_088 <= OMEGA_TRANSPORT_CAP);
    let before = funded.source.state;
    assert_eq!(before.core[core::BALANCE], Fp::from(100));
    // Reconstruct the *same signed Load entry*, then assert all predecessor
    // fields against the actual proved head before building its next insertion.
    let (mut initial, certificate, credential) = bootstrap_objects::enrollment();
    initial.lineage[17] = before.lineage[17];
    let (load, _, _, voucher) = load_objects::authorized(&initial);
    assert_eq!(load.successor.core, before.core);
    assert_eq!(load.successor.rest, before.rest);
    assert_eq!(load.successor.lineage, before.lineage);
    let mut tree = IndexedTree::new();
    let two_to_128 = Fp::from(2).pow_vartime([128]);
    tree.insert(
        two_to_128 + initial.core[core::NEXT_LOAD],
        hash_with_domain(
            LOAD_DOMAIN,
            &[
                initial.core[core::NEXT_LOAD],
                voucher.digest(),
                Fp::from(100),
            ],
        ),
    )
    .unwrap();
    assert_eq!(tree.root(), before.core[core::LOAD_REDEEM_ROOT]);
    let ordinal = before.core[core::NEXT_REDEEM];
    let nullifier = hash_with_domain(
        NULLIFIER_DOMAIN,
        &[
            before.core[core::SCHEME],
            before.core[core::SCHEME + 1],
            before.core[core::WALLET],
            before.core[core::WALLET + 1],
            ordinal,
        ],
    );
    let amount = Fp::from(30);
    let charge = Fp::from(3);
    let insertion = tree
        .insert(
            two_to_128.double() + ordinal,
            hash_with_domain(REDEEM_DOMAIN, &[ordinal, nullifier, amount, charge]),
        )
        .unwrap();
    let mut after = before;
    after.core[core::BALANCE] -= amount;
    after.core[core::BURNED_TOTAL] = before.lineage[14];
    after.core[core::PENDING_OUTGOING_ROOT] = before.lineage[15];
    after.core[core::SEQUENCE] += Fp::ONE;
    after.core[core::STATE_NONCE] += Fp::ONE;
    after.core[core::NEXT_REDEEM] += Fp::ONE;
    after.core[core::LOAD_REDEEM_ROOT] = tree.root();
    let mut state = ConsumingWitness {
        predecessor: before,
        successor: after,
        statement: [Fp::ZERO; 26],
    };
    state.statement[0] = Fp::ONE;
    state.statement[16] = Fp::from(6);
    state.statement[17..22].copy_from_slice(&[nullifier, ordinal, amount, charge, Fp::from(43)]);
    rebind(&mut state);
    assert_eq!(state.predecessor.core, before.core);
    assert_eq!(state.predecessor.rest, before.rest);
    assert_eq!(state.predecessor.lineage, before.lineage);
    let leaf = UnloadCircuit::new(&state);
    let public = leaf.instances();
    assert!(
        check_circuit(&leaf, 12, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let leaf_params = PinnedParams::<Eq>::derive(12).unwrap();
    let leaf_key = keygen_pk_v2(
        &leaf_params,
        &leaf,
        &KeygenConfigV2::pipa_r(UnloadCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let leaf_output = create_proof_owned_with_claim(
        &leaf_params,
        &leaf_key,
        Witness::from_circuit(&leaf_key, &leaf, &public).unwrap(),
        common::recovery(221),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &leaf_params,
        leaf_key.binding(),
        leaf_key.vk(),
        &public,
        &leaf_output.proof,
        budget,
    )
    .unwrap();
    leaf_output.opening.decide(&leaf_params, budget).unwrap();
    assert!(leaf_output.proof.len() <= 3_456);
    let sigma_plan = QSigmaPlan::new(
        SigmaClass::new(
            VerifierPlan::new(leaf_key.binding().clone(), leaf_params).unwrap(),
            vec![(
                13,
                leaf_key.vk().kagemusha_digest(leaf_key.binding()).unwrap(),
            )],
        )
        .unwrap(),
        None,
        &vparams,
    )
    .unwrap();
    let prepared = sigma_plan
        .prepare(
            SigmaSlotWitness {
                key: leaf_key.vk().clone(),
                statement: public[0][0],
                proof: leaf_output.proof.clone(),
                length: leaf_output.proof.len().try_into().unwrap(),
            },
            None,
            &vparams,
            Fq::from(131),
            &FoldConfig::default(),
        )
        .unwrap();
    let q = QSigmaProver::keygen_serialized_foreign(&prepared, params.clone(), 4).unwrap();
    let qproof = q
        .prove(&prepared, common::recovery(222), ProverConfig::default())
        .unwrap();
    verify_full(
        &params,
        q.binding(),
        q.verifying_key(),
        &qproof.instances,
        &qproof.bytes,
        budget,
    )
    .unwrap();
    qproof.part.decide(&vparams, budget).unwrap();
    let omega = lineage_bytes(
        &before.lineage,
        &funded.proof,
        &funded.source.pallas,
        &funded.vesta,
    )
    .unwrap();
    let proof_digest = p_bytes_native(
        u64::from_le_bytes(*b"kgwprf_1"),
        &[frame(&omega).unwrap(), frame(&leaf_output.proof).unwrap()].concat(),
    );
    let mut body = 1_u16.to_le_bytes().to_vec();
    body.extend(bootstrap_objects::id(after.core[1], after.core[2]));
    body.extend(bootstrap_objects::id(after.core[5], after.core[6]));
    body.extend(bootstrap_objects::small_id(31, 32));
    body.extend(&state.statement[9].to_repr()[..16]);
    let operation = hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[after.core[5], after.core[6], Fp::from(6), nullifier],
    );
    for value in [
        operation,
        state.statement[14],
        state.statement[15],
        hash_with_domain(STATEMENT_DOMAIN, &state.statement),
        proof_digest,
    ] {
        body.extend(value.to_repr());
    }
    body.extend(bootstrap_objects::small_id(101, 102));
    body.extend(Fp::ZERO.to_repr());
    let receipt = bootstrap_objects::sign(ObjectKind::Receipt, body, 29, 59);
    let policy = load_objects::policy();
    let schema = UnloadStagePlan::signature_schema(policy).unwrap();
    let signatures = QSignatureCircuit::new(
        schema.clone(),
        vec![
            receipt.signature,
            credential.signature,
            certificate.signature,
        ],
    )
    .unwrap();
    let signature_instances = signatures.instances(&[true; 3]).unwrap();
    let signature_key = keygen_pk_v2(
        &params,
        &signatures,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let signature_output = create_proof_owned_with_claim(
        &params,
        &signature_key,
        Witness::from_circuit(&signature_key, &signatures, &signature_instances).unwrap(),
        common::recovery(223),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &params,
        signature_key.binding(),
        signature_key.vk(),
        &signature_instances,
        &signature_output.proof,
        budget,
    )
    .unwrap();
    signature_output.opening.decide(&params, budget).unwrap();
    let operation = AProofPlan::new(
        Variant::Unload,
        sigma_plan,
        vec![
            crate::a_relation::QProofPlan::new(
                VerifierPlan::new(q.binding().clone(), params.clone()).unwrap(),
                q.verifying_key().clone(),
            )
            .unwrap(),
            crate::a_relation::QProofPlan::new(
                VerifierPlan::new(signature_key.binding().clone(), params.clone()).unwrap(),
                signature_key.vk().clone(),
            )
            .unwrap(),
        ],
        Some(VerifierPlan::new(funded.binding.clone(), params.clone()).unwrap()),
        &params,
    )
    .unwrap();
    let plan = Plan::new(operation, policy, schema, funded.key, params, vparams).unwrap();
    let input = Inputs {
        state,
        sigma: leaf_output.proof,
        objects: [credential.bytes, certificate.bytes, receipt.bytes],
        insertion,
        q: [
            QInput {
                proof: qproof.bytes,
                instances: qproof.instances,
            },
            QInput {
                proof: signature_output.proof,
                instances: signature_instances.to_vec(),
            },
        ],
        omega,
        predecessor: PredecessorInput {
            proof: funded.proof,
            pallas: funded.source.pallas.to_bytes(),
            vesta: funded.vesta.to_bytes(),
        },
    };
    (plan, input)
}

#[derive(Clone)]
struct Original {
    descriptor: Vec<u8>,
    vk: Vec<u8>,
    pk: Vec<u8>,
}
impl Original {
    fn from_key<C: PastaCurve>(key: &ProvingKey<C>) -> Self {
        Self {
            descriptor: key.binding().encoded().to_vec(),
            vk: key.vk().to_bytes().to_vec(),
            pk: key.artifact_bytes_v2().unwrap(),
        }
    }
    fn borrow(&self) -> OriginalArtifact<'_> {
        OriginalArtifact {
            descriptor: &self.descriptor,
            verifying_key: &self.vk,
            proving_key: &self.pk,
        }
    }
}
fn fixture_originals(plan: &Plan) -> ([Original; 4], [Original; 3]) {
    let first = FirstCircuit::blank(plan).unwrap();
    assert!(!first.known);
    assert!(first.source.predecessor.pallas.is_none());
    assert!(first.source.predecessor.vesta.is_none());
    let mut a = Vec::new();
    let mut w = Vec::new();
    let mut wrappers: Vec<WKey> = Vec::new();
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    for stage in 0..A_STAGE_COUNT {
        let circuit = if stage == 0 {
            StageCircuit {
                inner: StageData::First(first.clone()),
            }
        } else {
            let split = SplitPlan::new(
                plan.context.clone(),
                stage,
                wrappers[stage - 1].clone(),
                &plan.pallas,
            )
            .unwrap();
            let continuation = ContinuationCircuit::blank(first.clone(), split).unwrap();
            assert_eq!(continuation.history.len(), stage - 1);
            assert!(continuation.vesta.is_none() && continuation.carried.is_none());
            StageCircuit {
                inner: StageData::Continued(continuation),
            }
        };
        let key = keygen_pk_v2(&plan.vesta, &circuit, &config).unwrap();
        a.push(Original::from_key(&key));
        if stage + 1 < A_STAGE_COUNT {
            let source = wrapper_source(plan, stage, key.binding(), key.vk()).unwrap();
            let key = keygen_pk_v2(
                &plan.pallas,
                &source,
                &KeygenConfigV2::pipa_r(crate::omega::OmegaPlan::instance_types().to_vec()),
            )
            .unwrap();
            wrappers.push(
                WKey::from_artifact(
                    &plan.context,
                    stage,
                    key.binding().clone(),
                    plan.pallas.clone(),
                    key.vk().clone(),
                )
                .unwrap(),
            );
            w.push(Original::from_key(&key));
        }
    }
    assert!(a.iter().all(|key| key.descriptor == a[0].descriptor));
    (
        a.try_into().unwrap_or_else(|_| panic!("four A keys")),
        w.try_into().unwrap_or_else(|_| panic!("three W keys")),
    )
}
fn install(
    plan: Plan,
    a: &[Original; 4],
    w: &[Original; 3],
    config: ReadConfig,
) -> Result<Prover, Error> {
    Prover::from_original_artifacts(
        plan,
        a.each_ref().map(Original::borrow),
        w.each_ref().map(Original::borrow),
        config,
    )
}
fn assert_layout(source: &StageCircuit, actual: &StageCircuit, public: &[Fp]) {
    let unknown = synthesize(source, 16, None).unwrap();
    let known = synthesize(actual, 16, Some(&[public.to_vec()])).unwrap();
    assert_eq!(unknown.tables.fixed(), known.tables.fixed());
    assert_eq!(unknown.tables.permutation(), known.tables.permutation());
    assert_eq!(
        unknown.tables.advice_assigned(),
        known.tables.advice_assigned()
    );
}

#[test]
#[ignore = "genuine compact funded predecessor, original-only seven-stage imports/proofs/restoration; run optimized"]
fn installed_unload_originals_prove_and_restore_from_compact_funded_head() {
    let budget = MemoryBudget::DEFAULT;
    let fold = FoldConfig::default();
    let (plan, input) = genuine_input();
    // Only metadata/source shapes produce these test originals; no actual native
    // Prepared or checkpoint exists until after the original importer succeeds.
    let (key_originals_a, key_originals_w) = fixture_originals(&plan);
    let config = ReadConfig {
        maximum_bytes: key_originals_a
            .iter()
            .chain(&key_originals_w)
            .map(|key| key.pk.len())
            .max()
            .unwrap(),
        maximum_rows: 1 << 16,
        coset_cache: iroha_plonk::keys::CosetCachePolicy::OnDemand,
        msm_budget: budget,
    };
    let installed = install(plan.clone(), &key_originals_a, &key_originals_w, config).unwrap();
    for mutation in 0..12 {
        let mut a = key_originals_a.clone();
        let mut w = key_originals_w.clone();
        match mutation {
            0 => a.swap(0, 3),
            1 => a.swap(1, 2),
            2 => w.swap(0, 2),
            3 => a[0].vk = a[3].vk.clone(),
            4 => w[0].vk = w[2].vk.clone(),
            5 => a[0].pk[44 + key_originals_a[0].vk.len()] ^= 1,
            6 => a[0].pk[44 + key_originals_a[0].vk.len() + 32] ^= 1,
            7 => w[0].pk[44 + key_originals_w[0].vk.len()] ^= 1,
            8 => a[3].pk[44 + key_originals_a[3].vk.len()] ^= 1,
            9 => a[0].pk.push(0),
            10 => {
                a[0].pk.pop();
            }
            _ => w[0].descriptor = a[0].descriptor.clone(),
        }
        assert!(
            install(plan.clone(), &a, &w, config).is_err(),
            "original mutation {mutation}"
        );
    }
    for bad in [
        ReadConfig {
            maximum_rows: (1 << 16) - 1,
            ..config
        },
        ReadConfig {
            maximum_bytes: 0,
            ..config
        },
    ] {
        assert!(install(plan.clone(), &key_originals_a, &key_originals_w, bad).is_err());
    }
    let descriptor =
        iroha_plonk::cs::CircuitDescriptorV2::decode(&key_originals_a[0].descriptor).unwrap();
    for mutation in 0..6 {
        let mut wrong = descriptor.clone();
        match mutation {
            0 => wrong.instance_types[0] = InstanceType::Field,
            1 => wrong.transcript = TranscriptV2::KagemushaPoseidonRp57,
            2 => wrong.instance_mode = InstanceModeV1::Committed,
            3 => wrong.proof_suffix = ProofSuffixV1::None,
            4 => wrong.instance_lengths[0] = 68,
            _ => wrong.curve = CurveV1::Pallas,
        }
        let mut a = key_originals_a.clone();
        a[0].descriptor = wrong.encode().unwrap();
        assert!(
            install(plan.clone(), &a, &key_originals_w, config).is_err(),
            "profile mutation {mutation}"
        );
    }
    let changed_policy = OwnPolicy::new([1, 2], [31, 32], bootstrap_objects::key(19)).unwrap();
    let changed = Plan::new(
        plan.context.operation().clone(),
        changed_policy,
        UnloadStagePlan::signature_schema(changed_policy).unwrap(),
        plan.predecessor_key.clone(),
        plan.pallas.clone(),
        plan.vesta.clone(),
    )
    .unwrap();
    assert!(install(changed, &key_originals_a, &key_originals_w, config).is_err());
    for mutation in 0..10 {
        let mut bad = input.clone();
        match mutation {
            0 => bad.predecessor.proof[0] ^= 1,
            1 => bad.predecessor.pallas[0] ^= 1,
            2 => bad.predecessor.vesta[0] ^= 1,
            3 => bad.omega[321] ^= 1,
            4 => bad.sigma[0] ^= 1,
            5 => bad.q[0].proof[0] ^= 1,
            6 => bad.q[1].proof[0] ^= 1,
            7 => bad.q[0].instances[2][0] = Fq::from(15),
            8 => bad.state.predecessor.lineage[17] += Fp::ONE,
            _ => {
                bad.objects[2].pop();
            }
        }
        assert!(
            installed.prepare(bad, budget).is_err(),
            "input mutation {mutation}"
        );
    }
    let session = installed.prepare(input.clone(), budget).unwrap();
    let other_session = installed.prepare(input.clone(), budget).unwrap();
    let first_source = FirstCircuit::blank(&plan).unwrap();
    let (actual, public) = session
        .prepared
        .first_circuit(Fp::from(122), &fold)
        .unwrap();
    assert_layout(
        &StageCircuit {
            inner: StageData::First(first_source.clone()),
        },
        &actual,
        &public,
    );
    for mutation in 0..2 {
        let StageData::First(mut bad) = actual.inner.clone() else {
            panic!("A1 source")
        };
        let source = Arc::make_mut(&mut bad.source);
        if mutation == 0 {
            source.omega[321] ^= 1;
        } else {
            source.maps.witness.successor.core[core::BALANCE] += Fp::ONE;
        }
        assert!(
            !check_circuit(
                &StageCircuit {
                    inner: StageData::First(bad)
                },
                16,
                &[public.clone()],
                CheckMode::Strict
            )
            .unwrap()
            .is_satisfied(),
            "A1 exact carrier/state mutation {mutation}"
        );
    }
    let mut a = session
        .first(
            Fp::from(122),
            &fold,
            common::recovery(225),
            ProverConfig::default(),
        )
        .unwrap();
    assert!(session.terminal(&a, budget).is_err());
    assert!(
        other_session
            .wrapper(
                &a,
                Fq::from(143),
                &fold,
                common::recovery(226),
                ProverConfig::default()
            )
            .is_err()
    );
    let mut proof = a.proof().to_vec();
    proof[0] ^= 1;
    assert!(
        session
            .restore_first(proof, &a.pallas_bytes(), budget)
            .is_err()
    );
    let mut claim = a.pallas_bytes();
    claim[0] ^= 1;
    assert!(
        session
            .restore_first(a.proof().to_vec(), &claim, budget)
            .is_err()
    );
    let restored = session
        .restore_first(a.proof().to_vec(), &a.pallas_bytes(), budget)
        .unwrap();
    assert_eq!(a.instances(), restored.instances());
    a = restored;
    for stage in 0..3 {
        let w = session
            .wrapper(
                &a,
                Fq::from(143 + stage as u64),
                &fold,
                common::recovery(227 + stage as u8),
                ProverConfig::default(),
            )
            .unwrap();
        let mut proof = w.proof().to_vec();
        proof[0] ^= 1;
        assert!(
            session
                .restore_wrapper(&a, proof, &w.vesta_bytes(), budget)
                .is_err()
        );
        let mut claim = w.vesta_bytes();
        claim[0] ^= 1;
        assert!(
            session
                .restore_wrapper(&a, w.proof().to_vec(), &claim, budget)
                .is_err()
        );
        let w = session
            .restore_wrapper(&a, w.proof().to_vec(), &w.vesta_bytes(), budget)
            .unwrap();
        let next = stage + 1;
        let mut claims = vec![w.source.pallas.as_input(), w.opening.clone()];
        claims.extend(
            plan.context
                .q_partition(next)
                .unwrap()
                .iter()
                .map(|index| session.prepared.source.q_openings[*index].clone()),
        );
        let salt = Fp::from(161 + stage as u64);
        let (pfold, pallas) = create_fold(&plan.pallas, &claims, salt.to_repr(), &fold).unwrap();
        let continuation = session
            .continuation(&w, pallas, pfold.to_bytes().to_vec())
            .unwrap();
        let public = continuation_public(&continuation).unwrap();
        let blank =
            ContinuationCircuit::blank(first_source.clone(), continuation.plan.clone()).unwrap();
        assert_layout(
            &StageCircuit {
                inner: StageData::Continued(blank),
            },
            &StageCircuit {
                inner: StageData::Continued(continuation.circuit()),
            },
            &public,
        );
        if next == 1 {
            let mut bad = continuation.circuit();
            Arc::make_mut(&mut bad.first.source)
                .maps
                .insertion
                .slot_siblings[0] += Fp::ONE;
            assert!(
                !check_circuit(
                    &StageCircuit {
                        inner: StageData::Continued(bad)
                    },
                    16,
                    &[public.clone()],
                    CheckMode::Strict
                )
                .unwrap()
                .is_satisfied(),
                "A2 must reject a changed recovery path"
            );
        }
        if next == 3 {
            let mut bad = continuation.circuit();
            Arc::make_mut(&mut bad.first.source).maps.objects[0].bytes[10] ^= 1;
            assert!(
                !check_circuit(
                    &StageCircuit {
                        inner: StageData::Continued(bad)
                    },
                    16,
                    &[public.clone()],
                    CheckMode::Strict
                )
                .unwrap()
                .is_satisfied(),
                "A4 must reject a changed current credential tape"
            );
        }
        let next_a = session
            .advance(
                &w,
                salt,
                &fold,
                common::recovery(231 + stage as u8),
                ProverConfig::default(),
            )
            .unwrap();
        assert_eq!(next_a.instances(), public);
        let mut proof = next_a.proof().to_vec();
        proof[0] ^= 1;
        assert!(
            session
                .restore_a(&w, proof, &next_a.pallas_bytes(), budget)
                .is_err()
        );
        let mut claim = next_a.pallas_bytes();
        claim[0] ^= 1;
        assert!(
            session
                .restore_a(&w, next_a.proof().to_vec(), &claim, budget)
                .is_err()
        );
        a = session
            .restore_a(&w, next_a.proof().to_vec(), &next_a.pallas_bytes(), budget)
            .unwrap();
        assert_eq!(a.instances(), next_a.instances());
        eprintln!(
            "NATIVE_UNLOAD stage=A{} original_import=true proof_bytes={} restored=true",
            next + 1,
            a.proof().len()
        );
    }
    let terminal = session.terminal(&a, budget).unwrap();
    verify_full(
        &plan.vesta,
        installed.a[3].binding(),
        installed.a[3].vk(),
        &[terminal.instances.clone()],
        &terminal.proof,
        budget,
    )
    .unwrap();
    terminal.pallas.decide(&plan.pallas, budget).unwrap();
    terminal.vesta_part.decide(&plan.vesta, budget).unwrap();
    terminal
        .predecessor_vesta
        .decide(&plan.vesta, budget)
        .unwrap();
    terminal.opening.decide(&plan.vesta, budget).unwrap();
    assert_eq!(
        terminal.predecessor_vesta.to_bytes(),
        input.predecessor.vesta
    );
    assert_eq!(
        terminal.instances[0],
        terminal_digest(&input.state.successor.lineage, &terminal.pallas.as_input()).unwrap()
    );
    assert!(
        session
            .wrapper(
                &a,
                Fq::from(200),
                &fold,
                common::recovery(239),
                ProverConfig::default()
            )
            .is_err()
    );
    eprintln!(
        "NATIVE_UNLOAD_COMPLETE funded_compact_predecessor=true original_import=true stages=7 all_restored=true terminal_A4=true final_omega=false full_catalog=false"
    );
}
