//! Genuine monetary Load inputs following direct native receipt authentication.
//! The exact receipt is never relabeled; Load binds its terms and signed Advance.

use super::{bootstrap_objects, common, load_chain, load_objects};
use ff::Field;
use iroha_kagemusha_proof::{
    a_relation::{
        AProofPlan, QProofPlan,
        native::load::{Inputs, Plan, PredecessorInput, QInput},
    },
    admin_sigma::LoadCircuit,
    q_signature::QSignaturePlan,
    witness::core_index as core,
};
use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, Witness, create_proof_owned_with_claim,
    cs::InstanceType,
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2, pk::artifact::ReadConfig},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierPlan};
use std::{fs, path::Path};

pub use super::load_objects::from_finalized_receipt as monetary;

/// Build from a real rooted predecessor and a natively authenticated receipt.
/// The monetary relation verifies receipt binding, authorization and the state transition.
pub fn build(
    rooted: &load_chain::bootstrap_outer::RootedBootstrapOmega,
    receipt: &[u8; 282],
    directory: &Path,
) -> load_chain::InstalledLoad {
    build_with_source(
        rooted,
        receipt,
        directory,
        |plan| plan,
        ordinary_sigma,
        |_| {},
    )
}

fn ordinary_sigma(state: &mut iroha_kagemusha_proof::admin_sigma::LoadWitness) -> Vec<u8> {
    let sigma_params = PinnedParams::<Eq>::derive(12).unwrap();
    let sigma = LoadCircuit::new(state);
    let sigma_instances = sigma.instances();
    let sigma_key = keygen_pk_v2(
        &sigma_params,
        &sigma,
        &KeygenConfigV2::pipa_r(LoadCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let sigma_proof = create_proof_owned_with_claim(
        &sigma_params,
        &sigma_key,
        Witness::from_circuit(&sigma_key, &sigma, &sigma_instances).unwrap(),
        common::fresh_randomness(),
        ProverConfig::default(),
    )
    .unwrap();
    sigma_proof.proof
}

/// Exercise the native single-terminal Load with its selected state opening and
/// independently generated receipt-bound administrative proof.
pub fn build_with_source(
    rooted: &load_chain::bootstrap_outer::RootedBootstrapOmega,
    receipt: &[u8; 282],
    directory: &Path,
    select_plan: impl FnOnce(Plan) -> Plan,
    prepare_sigma: impl FnOnce(&mut iroha_kagemusha_proof::admin_sigma::LoadWitness) -> Vec<u8>,
    bind_bootstrap: impl FnOnce(&mut iroha_kagemusha_proof::admin_sigma::BootstrapWitness),
) -> load_chain::InstalledLoad {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => assert!(metadata.file_type().is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(directory).unwrap()
        }
        Err(error) => panic!("unavailable Load original directory: {error}"),
    }
    let (mut state, insertion) = monetary(&rooted.source.state, receipt);
    let pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let vesta = PinnedParams::<Eq>::derive(16).unwrap();
    // The wallet still proves its k12 step relation before Advance, and the own receipt
    // binds that proof's digest. Lean lineage proves the Load transition in its single
    // terminal A directly, so no Q_sigma verifies this proof inside the lineage.
    let sigma_proof = prepare_sigma(&mut state);
    let mut signed_state = bootstrap_objects::enrollment();
    bind_bootstrap(&mut signed_state.0);
    signed_state.0.lineage[17] = rooted.source.state.lineage[17];
    assert_eq!(signed_state.0.core, rooted.source.state.core);
    assert_eq!(signed_state.0.rest, rooted.source.state.rest);
    assert_eq!(signed_state.0.lineage, rooted.source.state.lineage);
    let own = load_objects::receipt(&state, &sigma_proof);
    let (circuit, instances) = load_objects::own_signature(own.signature);
    let signature_plans: [QSignaturePlan; 1] = [circuit.plan().clone()];
    let key = keygen_pk_v2(
        &pallas,
        &circuit,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let proof = create_proof_owned_with_claim(
        &pallas,
        &key,
        Witness::from_circuit(&key, &circuit, &instances).unwrap(),
        common::fresh_randomness(),
        ProverConfig::default(),
    )
    .unwrap();
    let q_plans = vec![
        QProofPlan::new(
            VerifierPlan::new(key.binding().clone(), pallas.clone()).unwrap(),
            key.vk().clone(),
        )
        .unwrap(),
    ];
    let q_inputs = [QInput {
        proof: proof.proof,
        instances: instances.to_vec(),
    }];
    drop(key);
    let operation = AProofPlan::inline(
        Variant::Load,
        q_plans,
        Some(VerifierPlan::new(rooted.binding.clone(), pallas.clone()).unwrap()),
        &pallas,
    )
    .unwrap();
    let plan = Plan::new(
        operation,
        load_objects::policy(),
        signature_plans,
        rooted.key.clone(),
        pallas.clone(),
        vesta.clone(),
    )
    .unwrap();
    let plan = select_plan(plan);
    let inputs = Inputs {
        state,
        receipt: *receipt,
        objects: [own.bytes],
        insertion,
        q: q_inputs,
        predecessor: PredecessorInput {
            proof: rooted.proof.clone(),
            pallas: rooted.source.pallas.to_bytes(),
            vesta: rooted.vesta.to_bytes(),
        },
    };
    let _prepared = plan.prepare(inputs.clone(), MemoryBudget::DEFAULT).unwrap();
    // One terminal A original; no W source follows it.
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    let key = keygen_pk_v2(&vesta, &plan.source_circuit().unwrap(), &config).unwrap();
    let terminal = load_chain::Original::persist(
        &directory.join("a0.pk"),
        key.binding().encoded().to_vec(),
        key.vk().to_bytes().to_vec(),
        key.artifact_bytes_v2().unwrap(),
    );
    drop(key);
    eprintln!("ORDINARY_LOAD_ARTIFACT stage=1 lean1A=true");
    load_chain::InstalledLoad {
        plan,
        terminal,
        inputs,
        read: ReadConfig {
            maximum_bytes: 256 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
    }
}

/// Check exact first-Load identities/terms and the actual local sigma constraints.
pub fn check_first_capture(capture: &str) {
    let json: norito::json::Value = norito::json::from_str(capture).unwrap();
    let text = json
        .get("receipt_transcript_hex")
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(text.len(), 564);
    let receipt: [u8; 282] = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let (before, _, _) = bootstrap_objects::enrollment();
    let (state, insertion) = monetary(&before, &receipt);
    assert_eq!(state.statement[18..21], [Fp::ZERO, Fp::from(100), Fp::ZERO]);
    assert_eq!(state.successor.core[core::BALANCE], Fp::from(100));
    assert!(insertion.slot > 0);
    let circuit = LoadCircuit::new(&state);
    let report = iroha_plonk::check::check_circuit(
        &circuit,
        12,
        &circuit.instances(),
        iroha_plonk::check::CheckMode::Strict,
    )
    .unwrap();
    assert!(report.is_satisfied());
    for offset in [2, 34, 66, 130] {
        let mut changed = receipt;
        changed[offset] ^= 1;
        assert!(std::panic::catch_unwind(|| monetary(&before, &changed)).is_err());
    }
}
