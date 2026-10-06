//! Genuine authenticated Load terminal proof wrapped by the complete Omega
//! predicate. The inherited Bootstrap-only catalog is explicitly diagnostic.

/// Shared genuine Bootstrap predecessor and Load continuation fixtures.
#[path = "a_load_recursive.rs"]
pub mod load_chain;

use ff::PrimeField;
use iroha_kagemusha_proof::omega::{OmegaCircuit, OmegaPlan, OmegaWitness};
use iroha_pasta::{Ep, Eq, Fq, PastaAffine, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness, create_proof_owned_with_claim,
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, create_fold};

/// Complete outer artifacts, carrying the construction's explicit provenance.
#[allow(dead_code)]
pub(crate) struct DiagnosticLoadOmega {
    pub(crate) source: load_chain::AuthenticatedLoad,
    pub(crate) key: iroha_plonk::VerifyingKey<Ep>,
    pub(crate) binding: iroha_plonk::DescriptorBinding,
    pub(crate) proof: Vec<u8>,
    pub(crate) instances: Vec<Vec<Fq>>,
    pub(crate) opening: iroha_plonk_recursion::FoldInput<Ep>,
    pub(crate) vesta: AccumulatorT<Eq>,
}

/// Produces a real native outer proof while retaining the Bootstrap-only
/// predecessor catalog; this is not an admitted final-catalog Load lineage.
pub(crate) fn diagnostic_load_omega(diagnostics: bool) -> DiagnosticLoadOmega {
    let predecessor = load_chain::bootstrap_outer::rooted_bootstrap_omega(false);
    let load = load_chain::authenticated_load(&predecessor, 5, false);
    let params = PinnedParams::<Eq>::derive(16).unwrap();
    let trivial = AccumulatorT::trivial(&params, MemoryBudget::DEFAULT).unwrap();
    let (fold, vesta) = create_fold(
        &params,
        &[
            load.vesta_part.as_input(),
            load.opening.clone(),
            load.predecessor_vesta.as_input(),
            trivial.as_input(),
        ],
        Fq::from(141).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    vesta.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = vesta.g().coordinates().unwrap();
    let public = vec![
        vec![Fq::from_repr(load.instances[0].to_repr()).unwrap()],
        vec![x, y],
        vesta
            .challenges()
            .iter()
            .map(|v| Fq::from_repr(v.to_repr()).unwrap())
            .collect(),
    ];
    let digest = load.key.kagemusha_digest(&load.binding).unwrap();
    let plan = OmegaPlan::new(load.binding.clone(), params, vec![digest]).unwrap();
    let circuit = OmegaCircuit::new(
        plan,
        OmegaWitness {
            key: load.key.clone(),
            instances: load.instances.clone(),
            length: u32::try_from(load.proof.len()).unwrap(),
            proof: load.proof.clone(),
            fold: fold.to_bytes(),
        },
    )
    .unwrap();
    if diagnostics {
        eprintln!(
            "AUTHENTICATED_LOAD_COMPACT_CANDIDATE source_buses=5 full_catalog=false carried_outer_key_rebound=false"
        );
        load_chain::bootstrap_outer::compact_diagnostic(&circuit, &public);
        let pinned = circuit
            .clone()
            .with_key_catalog(vec![load.key.clone()])
            .unwrap();
        load_chain::bootstrap_outer::compact_diagnostic(&pinned, &public);
    }
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let key = keygen_pk_v2(
        &params,
        &circuit,
        &KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec()),
    )
    .unwrap();
    let witness = Witness::from_circuit(&key, &circuit, &public).unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        witness,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .unwrap();
    let opened = accumulate_generator(
        &params,
        key.binding(),
        key.vk(),
        &public,
        &proof.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    opened.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let opening =
        iroha_plonk_recursion::FoldInput::from_opening(*opened.g(), opened.challenges()).unwrap();
    eprintln!(
        "AUTHENTICATED_LOAD_OMEGA source_A_bytes={} actual_proof_bytes={} transport_bytes={} source_operation_authenticated=true full_catalog=false carried_outer_key_rebound=false production_size_pass={} release_qualified=false",
        load.proof.len(),
        proof.proof.len(),
        proof.proof.len() + 1088,
        proof.proof.len() + 1088 <= 4821,
    );
    DiagnosticLoadOmega {
        source: load,
        key: key.vk().clone(),
        binding: key.binding().clone(),
        proof: proof.proof,
        instances: public,
        opening,
        vesta,
    }
}

#[test]
#[ignore = "real authenticated Bootstrap and three-stage Load with complete outer proof; run optimized"]
fn five_bus_authenticated_load_reaches_complete_outer_predicate() {
    let _ = diagnostic_load_omega(true);
}
