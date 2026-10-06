//! Shared tag7 leaf: one fixed key, exact branch ownership and genuine proofs.

#[path = "common/bootstrap.rs"]
mod bootstrap;
mod common;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    admin_sigma::{RefreshCircuit, RefreshWitness, StateWitness},
    operation_relation::state::rest_index as rest,
    witness::{CORE_DOMAIN, REST_DOMAIN, core_index as core},
};
use iroha_pasta::{Fp, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    frontend::{Circuit, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    verifier::accumulate_generator,
};

const K: u32 = 12;

fn rebind(w: &mut RefreshWitness) {
    for state in [&mut w.predecessor, &mut w.successor] {
        let mut preimage = state.core.to_vec();
        preimage.push(hash_with_domain(REST_DOMAIN, &state.rest));
        state.lineage[5] = hash_with_domain(CORE_DOMAIN, &preimage);
        state.lineage[1..3].copy_from_slice(&state.core[core::SCHEME..=core::SCHEME + 1]);
        state.lineage[6..8].copy_from_slice(&state.core[core::WALLET..=core::WALLET + 1]);
        state.lineage[8] = state.core[core::CREDENTIAL];
        state.lineage[13] = state.core[core::LIFECYCLE]
            + Fp::from(256) * state.core[core::POLICY_EPOCH]
            + Fp::from_u128(1 << 72) * state.core[core::ENABLED_CONTROLS];
    }
    w.statement[3..7].copy_from_slice(&w.successor.core[core::SCHEME..core::ASSET + 2]);
    for (field, index) in [
        (7, core::CREDENTIAL),
        (8, core::LIFECYCLE),
        (9, core::SEQUENCE),
        (10, core::NEXT_LOAD),
    ] {
        w.statement[field] = w.successor.core[index];
    }
    w.statement[11] = w.predecessor.core[core::ENABLED_CONTROLS];
    w.statement[14] = w.predecessor.lineage[5];
    w.statement[15] = w.successor.lineage[5];
}
fn witness(kind: u64) -> RefreshWitness {
    let initial = bootstrap::witness();
    let mut before = StateWitness::from(&initial);
    before.core[core::BALANCE] = Fp::from(100);
    before.core[core::TIME_FLOOR] = Fp::from(30);
    before.core[core::TIME_ANCHOR_MAX_RESPONSE] = Fp::from(5);
    before.core[core::LEASE_EXPIRY] = Fp::from(1000);
    before.rest[rest::PERMITTED] = Fp::from(5);
    let mut after = before;
    after.core[core::SEQUENCE] += Fp::ONE;
    after.core[core::STATE_NONCE] += Fp::ONE;
    after.core[core::TIME_FLOOR] = Fp::from(if kind == 2 { 30 } else { 40 });
    let mut statement = initial.statement;
    statement[12..14].fill(Fp::ZERO);
    statement[16..].fill(Fp::ZERO);
    statement[16] = Fp::from(7);
    statement[17] = Fp::from(kind);
    statement[18] = Fp::from(80);
    statement[19] = after.core[core::TIME_FLOOR];
    match kind {
        1 => {
            after.core[core::CREDENTIAL] = Fp::from(80);
            after.core[core::LEASE_EXPIRY] = Fp::from(2000);
        }
        2 => {
            after.core[core::POLICY_EPOCH] = Fp::from(2);
            after.core[core::ENABLED_CONTROLS] = Fp::from(5);
            after.rest[rest::SCHEME_POLICY] = Fp::from(80);
            after.rest[rest::FEE_SCHEDULE] = Fp::from(90);
        }
        3 => {
            after.core[core::BLACKLIST_VERSION] = Fp::from(2);
            after.core[core::BLACKLIST_ROOT] = Fp::from(89);
            after.core[core::BLACKLIST_ISSUED_AT] = Fp::from(40);
            after.rest[rest::BLACKLIST] = Fp::from(80);
            after.rest[rest::BLACKLIST_HISTORY] = Fp::from(91);
        }
        4 => {
            after.rest[rest::QUOTA_SHARE] = Fp::from(80);
            after.rest[rest::QUOTA_SHARE_ID] = Fp::from(2);
            after.core[core::QUOTA_WINDOWS_ROOT] = Fp::from(89);
            after.core[core::QUOTA_SHARE_EXPIRY] = Fp::from(100);
            after.core[core::QUOTA_USAGE_ROOT] = Fp::from(93);
        }
        5 => after.rest[rest::TIME_ANCHOR] = Fp::from(80),
        _ => unreachable!(),
    }
    let mut w = RefreshWitness {
        predecessor: before,
        successor: after,
        statement,
        issued: after.core[core::TIME_FLOOR],
        policy_controls: Fp::from(if kind == 2 { 7 } else { 0 }),
    };
    rebind(&mut w);
    w
}
fn accepts(w: &RefreshWitness) -> bool {
    let circuit = RefreshCircuit::new(w);
    check_circuit(&circuit, K, &circuit.instances(), CheckMode::Strict)
        .is_ok_and(|r| r.is_satisfied())
}
fn core_changed(kind: u64, index: usize) -> bool {
    matches!(index, core::SEQUENCE | core::STATE_NONCE | core::TIME_FLOOR)
        || match kind {
            1 => matches!(index, core::CREDENTIAL | core::LEASE_EXPIRY),
            2 => matches!(index, core::POLICY_EPOCH | core::ENABLED_CONTROLS),
            3 => matches!(
                index,
                core::BLACKLIST_VERSION | core::BLACKLIST_ROOT | core::BLACKLIST_ISSUED_AT
            ),
            4 => matches!(
                index,
                core::QUOTA_WINDOWS_ROOT | core::QUOTA_SHARE_EXPIRY | core::QUOTA_USAGE_ROOT
            ),
            _ => false,
        }
}
fn rest_changed(kind: u64, index: usize) -> bool {
    match kind {
        2 => matches!(index, rest::SCHEME_POLICY | rest::FEE_SCHEDULE),
        3 => matches!(index, rest::BLACKLIST | rest::BLACKLIST_HISTORY),
        4 => matches!(index, rest::QUOTA_SHARE | rest::QUOTA_SHARE_ID),
        5 => index == rest::TIME_ANCHOR,
        _ => false,
    }
}
#[test]
fn every_kind_has_one_shape_and_preserves_unrelated_state() {
    let first = RefreshCircuit::new(&witness(1));
    let baseline = synthesize(&first, K, Some(&first.instances())).unwrap();
    for kind in 1..=5 {
        let w = witness(kind);
        assert!(accepts(&w), "honest kind{kind}");
        let circuit = RefreshCircuit::new(&w);
        let known = synthesize(&circuit, K, Some(&circuit.instances())).unwrap();
        let unknown = synthesize(&circuit.without_witnesses(), K, None).unwrap();
        for assembly in [&known, &unknown] {
            assert_eq!(assembly.tables.fixed(), baseline.tables.fixed());
            assert_eq!(assembly.tables.permutation(), baseline.tables.permutation());
            assert_eq!(
                assembly.tables.advice_assigned(),
                baseline.tables.advice_assigned()
            );
        }
        let rows = known
            .tables
            .advice_assigned()
            .iter()
            .map(|c| c.iter().rposition(|v| *v).map_or(0, |i| i + 1))
            .collect::<Vec<_>>();
        eprintln!("shared Refresh kind{kind} k{K} rows={rows:?}");
        for i in 0..33 {
            if core_changed(kind, i) {
                continue;
            }
            let mut wrong = w;
            wrong.successor.core[i] += Fp::ONE;
            rebind(&mut wrong);
            assert!(!accepts(&wrong), "kind{kind} rehashed core{i}");
        }
        for i in 0..8 {
            if rest_changed(kind, i) {
                continue;
            }
            let mut wrong = w;
            wrong.successor.rest[i] += Fp::ONE;
            rebind(&mut wrong);
            assert!(!accepts(&wrong), "kind{kind} rehashed rest{i}");
        }
        for i in [14, 15, 16, 17] {
            let mut wrong = w;
            wrong.successor.lineage[i] += Fp::ONE;
            assert!(!accepts(&wrong), "kind{kind} lineage{i}");
        }
        for invalid in [0, 6, 7, u64::MAX] {
            let mut wrong = w;
            wrong.statement[17] = Fp::from(invalid);
            assert!(!accepts(&wrong), "kind{kind} invalid kind{invalid}");
        }
        let mut wrong = w;
        wrong.statement[18] += Fp::ONE;
        assert!(!accepts(&wrong), "kind{kind} digest");
        let mut wrong = w;
        wrong.successor.core[core::TIME_FLOOR] += Fp::ONE;
        wrong.statement[19] += Fp::ONE;
        rebind(&mut wrong);
        assert!(!accepts(&wrong), "kind{kind} floor");
    }
}
#[test]
fn refresh_rejects_stale_counters_mask_escalation_and_expired_share() {
    for kind in [2, 3, 4] {
        let mut w = witness(kind);
        // Both state openings remain internally consistent: retain the full
        // installed object/root/counter tuple, then attempt its second install.
        w.predecessor = w.successor;
        w.successor.core[core::SEQUENCE] += Fp::ONE;
        w.successor.core[core::STATE_NONCE] += Fp::ONE;
        rebind(&mut w);
        assert!(!accepts(&w), "stale counter{kind}");
    }
    let mut w = witness(2);
    w.successor.core[core::ENABLED_CONTROLS] = Fp::from(1);
    rebind(&mut w);
    assert!(!accepts(&w));
    let mut w = witness(4);
    w.successor.core[core::QUOTA_SHARE_EXPIRY] = w.issued;
    rebind(&mut w);
    assert!(!accepts(&w));
    let mut w = witness(5);
    w.predecessor.rest[rest::TIME_ANCHOR] = w.successor.rest[rest::TIME_ANCHOR];
    rebind(&mut w);
    assert!(!accepts(&w));
    let mut w = witness(3);
    w.policy_controls = Fp::ONE;
    assert!(!accepts(&w));
}
#[test]
fn signed_time_below_floor_keeps_floor_and_noncanonical_kinds_reject() {
    for kind in 1..=5 {
        let mut w = witness(kind);
        w.issued = Fp::from(20);
        w.successor.core[core::TIME_FLOOR] = w.predecessor.core[core::TIME_FLOOR];
        w.statement[19] = w.successor.core[core::TIME_FLOOR];
        if kind == 3 {
            w.successor.core[core::BLACKLIST_ISSUED_AT] = w.issued;
        }
        rebind(&mut w);
        assert_eq!(accepts(&w), kind != 2, "floor kind{kind}");
        let mut wrong = witness(kind);
        wrong.statement[17] = -Fp::ONE;
        assert!(!accepts(&wrong), "negative kind{kind}");
        let mut wrong = witness(kind);
        wrong.issued = Fp::from_u128(1 << 64);
        assert!(!accepts(&wrong), "oversized issued kind{kind}");
    }
}

#[test]
fn genuine_five_kind_refresh_proofs_verify_under_one_key() {
    let params = common::vesta_params(K);
    let first = RefreshCircuit::new(&witness(1));
    let key = keygen_pk_v2(
        &params,
        &first,
        &KeygenConfigV2::pipa_r(RefreshCircuit::instance_types().to_vec()),
    )
    .unwrap();
    for kind in 1..=5 {
        let circuit = RefreshCircuit::new(&witness(kind));
        let public = circuit.instances();
        let proof = create_proof_owned_with_claim(
            &params,
            &key,
            Witness::from_circuit(&key, &circuit, &public).unwrap(),
            common::recovery(u8::try_from(kind).unwrap()),
            ProverConfig::default(),
        )
        .unwrap();
        let opening = accumulate_generator(
            &params,
            key.binding(),
            key.vk(),
            &public,
            &proof.proof,
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        opening.decide(&params, MemoryBudget::DEFAULT).unwrap();
        assert_eq!(opening.g(), proof.opening.g());
        assert!(proof.proof.len() <= 3456);
        eprintln!(
            "shared Refresh kind{kind}: {} proof bytes",
            proof.proof.len()
        );
        let mut wrong = public;
        wrong[0][0] += Fp::ONE;
        assert!(
            accumulate_generator(
                &params,
                key.binding(),
                key.vk(),
                &wrong,
                &proof.proof,
                MemoryBudget::DEFAULT
            )
            .is_err()
        );
    }
}

#[test]
#[ignore = "five genuine k16 Q-sigma proofs over the shared tag7 leaf; run optimized"]
fn genuine_refresh_q_sigma_keeps_one_class_key_and_every_opening() {
    use iroha_kagemusha_proof::{
        a_relation::schedule::sigma_selector,
        q_sigma::{QSigmaPlan, SigmaClass, SigmaSlotWitness, native::QSigmaProver},
    };
    use iroha_pasta::{Ep, Fq};
    use iroha_plonk::pcs::ipa::PinnedParams;
    use iroha_plonk_recursion::{FoldConfig, verifier::VerifierPlan};

    let leaf_params = common::vesta_params(K);
    let outer_vesta = common::vesta_params(16);
    let outer_pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let first = RefreshCircuit::new(&witness(1));
    let leaf_key = keygen_pk_v2(
        &leaf_params,
        &first,
        &KeygenConfigV2::pipa_r(RefreshCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let plan = QSigmaPlan::new(
        SigmaClass::new(
            VerifierPlan::new(leaf_key.binding().clone(), leaf_params.clone()).unwrap(),
            vec![(
                sigma_selector(7, 0).unwrap(),
                leaf_key.vk().kagemusha_digest(leaf_key.binding()).unwrap(),
            )],
        )
        .unwrap(),
        None,
        &outer_vesta,
    )
    .unwrap();
    let mut prover: Option<QSigmaProver> = None;
    for kind in 1..=5 {
        let circuit = RefreshCircuit::new(&witness(kind));
        let public = circuit.instances();
        let sigma = create_proof_owned_with_claim(
            &leaf_params,
            &leaf_key,
            Witness::from_circuit(&leaf_key, &circuit, &public).unwrap(),
            common::recovery(u8::try_from(kind + 50).unwrap()),
            ProverConfig::default(),
        )
        .unwrap();
        let prepared = plan
            .prepare(
                SigmaSlotWitness {
                    key: leaf_key.vk().clone(),
                    statement: public[0][0],
                    length: u32::try_from(sigma.proof.len()).unwrap(),
                    proof: sigma.proof.clone(),
                },
                None,
                &outer_vesta,
                Fq::from(71 + kind),
                &FoldConfig::default(),
            )
            .unwrap();
        prepared
            .part()
            .decide(&outer_vesta, MemoryBudget::DEFAULT)
            .unwrap();
        if prover.is_none() {
            prover = Some(
                QSigmaProver::keygen_serialized_foreign(&prepared, outer_pallas.clone(), 2)
                    .unwrap(),
            );
        }
        let q = prover.as_ref().unwrap();
        let proof = q
            .prove(
                &prepared,
                common::recovery(u8::try_from(kind + 60).unwrap()),
                ProverConfig::default(),
            )
            .unwrap();
        let opening = accumulate_generator(
            &outer_pallas,
            q.binding(),
            q.verifying_key(),
            &proof.instances,
            &proof.bytes,
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        opening
            .decide(&outer_pallas, MemoryBudget::DEFAULT)
            .unwrap();
        eprintln!(
            "REFRESH_SHARED_Q kind={kind} sigma={} Q={} full_opening=true shared_keys=true",
            sigma.proof.len(),
            proof.bytes.len()
        );
        let mut wrong = proof.instances;
        wrong[0][0] += Fq::ONE;
        assert!(
            accumulate_generator(
                &outer_pallas,
                q.binding(),
                q.verifying_key(),
                &wrong,
                &proof.bytes,
                MemoryBudget::DEFAULT
            )
            .is_err()
        );
    }
}
