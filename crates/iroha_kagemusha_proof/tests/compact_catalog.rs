//! Actual compact catalog closure. Every signed source chain is rebuilt under
//! the common outer key, with exact descriptor/key equality across the roundtrip.
//! Two/three terminal component catalogs are not the complete release catalog.

/// Shared real controls-off Send and its authenticated predecessor builders.
#[path = "a_send_recursive.rs"]
pub mod send_chain;

use bootstrap_outer::{RootedBootstrapOmega, bootstrap_chain};
use ff::{Field, PrimeField};
use iroha_kagemusha_proof::omega::{OmegaCircuit, OmegaPlan, OmegaWitness};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, msm::MemoryBudget};
use iroha_plonk::{
    DescriptorBinding, Protocol, ProverConfig, ProverRandomness, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    frontend::{Circuit, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::range::secondary::SecondaryPlan;
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, create_fold, verifier::CompactSpans,
};
use send_chain::load_outer::{
    DiagnosticLoadOmega,
    load_chain::{self, bootstrap_outer},
};

const PROFILE: bootstrap_chain::SourceProfile = bootstrap_chain::SourceProfile::Tagged { buses: 3 };

#[derive(Clone, Copy)]
struct Terminal<'a> {
    key: &'a VerifyingKey<Eq>,
    binding: &'a DescriptorBinding,
    proof: &'a [u8],
    instances: &'a [Fp],
    part: &'a AccumulatorT<Eq>,
    opening: &'a FoldInput<Eq>,
    predecessor: &'a AccumulatorT<Eq>,
}

fn wrap(
    source: Terminal<'_>,
    catalog: &[VerifyingKey<Eq>],
) -> (OmegaCircuit, Vec<Vec<Fq>>, AccumulatorT<Eq>) {
    let params = PinnedParams::<Eq>::derive(16).unwrap();
    let trivial = AccumulatorT::trivial(&params, MemoryBudget::DEFAULT).unwrap();
    let (fold, vesta) = create_fold(
        &params,
        &[
            source.part.as_input(),
            source.opening.clone(),
            source.predecessor.as_input(),
            trivial.as_input(),
        ],
        Fq::from(181).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    vesta.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = vesta.g().coordinates().unwrap();
    let public = vec![
        vec![Fq::from_repr(source.instances[0].to_repr()).unwrap()],
        vec![x, y],
        vesta
            .challenges()
            .iter()
            .map(|value| Fq::from_repr(value.to_repr()).unwrap())
            .collect(),
    ];
    let digests = catalog
        .iter()
        .map(|key| key.kagemusha_digest(source.binding).unwrap())
        .collect();
    let plan = OmegaPlan::new(source.binding.clone(), params, digests).unwrap();
    // Complete witnessed-key hashing with fixed membership; no host key-index
    // shortcut and no per-terminal verification profile are accepted.
    let circuit = OmegaCircuit::new(
        plan,
        OmegaWitness {
            key: source.key.clone(),
            instances: source.instances.to_vec(),
            proof: source.proof.to_vec(),
            length: source.proof.len().try_into().unwrap(),
            fold: fold.to_bytes(),
        },
    )
    .unwrap();
    (circuit, public, vesta)
}

struct Program {
    catalog: Vec<VerifyingKey<Eq>>,
    spans: CompactSpans,
    schedule: SecondaryPlan,
    binding: DescriptorBinding,
    key: VerifyingKey<Ep>,
}
struct Outer {
    proof: Vec<u8>,
    public: Vec<Vec<Fq>>,
    opening: FoldInput<Ep>,
    vesta: AccumulatorT<Eq>,
}
fn key_config() -> KeygenConfigV2 {
    let mut config = KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec());
    config.compress_selectors = false;
    config
}
impl Program {
    fn new(
        source: Terminal<'_>,
        catalog: Vec<VerifyingKey<Eq>>,
        predecessor_binding: &DescriptorBinding,
    ) -> Self {
        let (circuit, public, _) = wrap(source, &catalog);
        let (spans, schedule) = bootstrap_outer::compact_layout(&circuit, &public, false)
            .expect("common catalog must fit unchanged k16 range/shared capacity");
        let circuit = circuit.with_secondary_layout(spans, schedule.clone());
        let params = PinnedParams::<Ep>::derive(16).unwrap();
        let key = keygen_pk_v2(&params, &circuit, &key_config()).unwrap();
        let protocol = Protocol::new(key.binding().descriptor()).unwrap();
        assert_eq!(protocol.shape().degree, 9);
        assert_eq!(protocol.shape().lookups, 1);
        assert_eq!(protocol.proof_length(), 3712);
        assert_eq!(
            key.binding(),
            predecessor_binding,
            "catalog growth must preserve the exact predecessor descriptor"
        );
        eprintln!(
            "COMPACT_CATALOG_KEY terminals={} descriptor_unchanged=true shape={:?} expected_transport=4800 actual_proof=false",
            catalog.len(),
            protocol.shape()
        );
        Self {
            catalog,
            spans,
            schedule,
            binding: key.binding().clone(),
            key: key.vk().clone(),
        }
    }
    fn digest(&self) -> Fp {
        self.key.kagemusha_digest(&self.binding).unwrap()
    }
    fn prove(&self, source: Terminal<'_>, label: &str) -> Outer {
        let (circuit, public, vesta) = wrap(source, &self.catalog);
        let circuit = circuit.with_secondary_layout(self.spans, self.schedule.clone());
        let assigned = synthesize(&circuit, 16, Some(&public)).unwrap();
        let report = iroha_plonk::check::check(
            &assigned.cs,
            &assigned.tables,
            iroha_plonk::check::CheckMode::Strict,
        )
        .unwrap();
        assert!(
            report.is_satisfied(),
            "{label}: {:?}",
            report.failures().first()
        );
        let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
        assert_eq!(assigned.tables.fixed(), unknown.tables.fixed());
        assert_eq!(assigned.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            assigned.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
        drop(assigned);
        drop(unknown);
        let params = PinnedParams::<Ep>::derive(16).unwrap();
        let key = keygen_pk_v2(&params, &circuit, &key_config()).unwrap();
        assert_eq!(key.binding(), &self.binding);
        assert_eq!(
            key.vk().to_bytes(),
            self.key.to_bytes(),
            "{label}: immutable common outer key"
        );
        let output = create_proof_owned_with_claim(
            &params,
            &key,
            Witness::from_circuit(&key, &circuit, &public).unwrap(),
            ProverRandomness::os(),
            ProverConfig::default(),
        )
        .unwrap();
        assert_eq!(output.proof.len(), 3712);
        let opened = accumulate_generator(
            &params,
            &self.binding,
            &self.key,
            &public,
            &output.proof,
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        opened.decide(&params, MemoryBudget::DEFAULT).unwrap();
        assert_eq!(opened.g(), output.opening.g());
        assert_eq!(opened.challenges(), output.opening.challenges());
        for (column, row) in [(0, 0), (1, 0), (2, 0)] {
            let mut changed = public.clone();
            changed[column][row] += Fq::ONE;
            assert!(
                iroha_plonk::verify_full(
                    &params,
                    &self.binding,
                    &self.key,
                    &changed,
                    &output.proof,
                    MemoryBudget::DEFAULT
                )
                .is_err()
            );
        }
        eprintln!(
            "COMPACT_CATALOG_OUTER terminal={label} catalog_size={} actual_proof=3712 transport=4800 known_unknown_equal=true immutable_key=true decides=2 full_catalog=false release_qualified=false",
            self.catalog.len()
        );
        Outer {
            proof: output.proof,
            public,
            opening: FoldInput::from_opening(*opened.g(), opened.challenges()).unwrap(),
            vesta,
        }
    }
}

fn bootstrap_terminal<'a>(
    source: &'a bootstrap_chain::AuthenticatedBootstrap,
    trivial: &'a AccumulatorT<Eq>,
) -> Terminal<'a> {
    Terminal {
        key: &source.key,
        binding: &source.binding,
        proof: &source.proof,
        instances: &source.instances,
        part: &source.vesta_part,
        opening: &source.opening,
        predecessor: trivial,
    }
}
fn load_terminal(source: &load_chain::AuthenticatedLoad) -> Terminal<'_> {
    Terminal {
        key: &source.key,
        binding: &source.binding,
        proof: &source.proof,
        instances: &source.instances,
        part: &source.vesta_part,
        opening: &source.opening,
        predecessor: &source.predecessor_vesta,
    }
}
fn send_terminal(source: &send_chain::AuthenticatedSend) -> Terminal<'_> {
    Terminal {
        key: &source.key,
        binding: &source.binding,
        proof: &source.proof,
        instances: &source.instances,
        part: &source.vesta_part,
        opening: &source.opening,
        predecessor: &source.predecessor_vesta,
    }
}

fn rebuild_bootstrap_load(
    program: &Program,
    initial_bootstrap: &bootstrap_chain::AuthenticatedBootstrap,
    initial_load: &load_chain::AuthenticatedLoad,
) -> DiagnosticLoadOmega {
    let source = bootstrap_chain::authenticated_bootstrap_with_profile(
        false,
        program.digest(),
        PROFILE,
        Some(2),
    );
    assert_eq!(source.binding, initial_bootstrap.binding);
    assert_eq!(source.key.to_bytes(), initial_bootstrap.key.to_bytes());
    assert_eq!(source.state.lineage[17], program.digest());
    let trivial = AccumulatorT::trivial(
        &PinnedParams::<Eq>::derive(16).unwrap(),
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let outer = program.prove(bootstrap_terminal(&source, &trivial), "Bootstrap");
    let rooted = RootedBootstrapOmega {
        source,
        key: program.key.clone(),
        binding: program.binding.clone(),
        proof: outer.proof,
        instances: outer.public,
        opening: outer.opening,
        vesta: outer.vesta,
    };
    let source = load_chain::authenticated_load_with_profile(&rooted, PROFILE, Some(2), false);
    assert_eq!(source.binding, initial_load.binding);
    assert_eq!(source.key.to_bytes(), initial_load.key.to_bytes());
    assert_eq!(source.state.lineage[17], program.digest());
    let outer = program.prove(load_terminal(&source), "Load");
    source
        .pallas
        .decide(
            &PinnedParams::<Ep>::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    DiagnosticLoadOmega {
        source,
        key: program.key.clone(),
        binding: program.binding.clone(),
        proof: outer.proof,
        instances: outer.public,
        opening: outer.opening,
        vesta: outer.vesta,
    }
}

/// Independently enrolled wallets under one genuine two-terminal compact key.
/// The payer has completed Load; the receiver is at Bootstrap. The catalog admits
/// only those two source relations and does not qualify a Receive terminal.
#[allow(dead_code)] // Consumed by the accepted Receive integration fixture.
pub(crate) struct SharedKeyWallets {
    /// Payer predecessor with spendable loaded value and a verified outer proof.
    pub(crate) payer: DiagnosticLoadOmega,
    /// Distinct receiver with its own signed enrollment and verified outer proof.
    pub(crate) receiver: RootedBootstrapOmega,
}

/// Rebuilds both wallets' exact signed objects and all recursive source proofs
/// under the same immutable compact Bootstrap/Load catalog key. No statement or
/// state word is changed after proving; both retained accumulators are decided.
#[allow(dead_code)] // Consumed by the accepted Receive integration fixture.
pub(crate) fn compact_payer_load_and_receiver() -> SharedKeyWallets {
    let initial_root = bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap();
    let initial_load =
        load_chain::authenticated_load_with_profile(&initial_root, PROFILE, Some(2), false);
    let initial_bootstrap = &initial_root.source;
    assert_eq!(initial_bootstrap.binding, initial_load.binding);
    let trivial = AccumulatorT::trivial(
        &PinnedParams::<Eq>::derive(16).unwrap(),
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let program = Program::new(
        bootstrap_terminal(initial_bootstrap, &trivial),
        vec![initial_bootstrap.key.clone(), initial_load.key.clone()],
        &initial_root.binding,
    );
    let payer = rebuild_bootstrap_load(&program, initial_bootstrap, &initial_load);
    let source = bootstrap_chain::authenticated_bootstrap_with_identity(
        false,
        program.digest(),
        PROFILE,
        Some(2),
        bootstrap_chain::BootstrapIdentity::Receiver,
    );
    assert_eq!(source.binding, initial_bootstrap.binding);
    assert_eq!(source.key.to_bytes(), initial_bootstrap.key.to_bytes());
    assert_eq!(source.state.lineage[17], program.digest());
    assert_ne!(source.state.core[5..7], payer.source.state.core[5..7]);
    let outer = program.prove(bootstrap_terminal(&source, &trivial), "Receiver-Bootstrap");
    source
        .pallas
        .decide(
            &PinnedParams::<Ep>::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    let receiver = RootedBootstrapOmega {
        source,
        key: program.key.clone(),
        binding: program.binding.clone(),
        proof: outer.proof,
        instances: outer.public,
        opening: outer.opening,
        vesta: outer.vesta,
    };
    assert_eq!(payer.binding, receiver.binding);
    assert_eq!(payer.key.to_bytes(), receiver.key.to_bytes());
    assert_eq!(
        payer.source.state.lineage[17],
        receiver.source.state.lineage[17]
    );
    eprintln!(
        "COMPACT_SHARED_WALLETS payer=Load receiver=Bootstrap distinct_wallets=true immutable_key=true signed_objects_rebuilt=true all_native_proofs=true transport=4800 catalog_size=2 receive_terminal_admitted=false full_catalog=false"
    );
    SharedKeyWallets { payer, receiver }
}

fn catalog_roundtrip(include_send: bool) {
    let initial_root = bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap();
    let initial_load =
        load_chain::authenticated_load_with_profile(&initial_root, PROFILE, Some(2), false);
    let bootstrap = &initial_root.source;
    assert_eq!(
        bootstrap.binding, initial_load.binding,
        "uniform actual terminal A descriptor"
    );
    let trivial = AccumulatorT::trivial(
        &PinnedParams::<Eq>::derive(16).unwrap(),
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let catalog = vec![bootstrap.key.clone(), initial_load.key.clone()];
    let program = Program::new(
        bootstrap_terminal(bootstrap, &trivial),
        catalog,
        &initial_root.binding,
    );
    let load = rebuild_bootstrap_load(&program, bootstrap, &initial_load);
    eprintln!(
        "COMPACT_TWO_TERMINAL_CLOSURE source_keys_equal=true common_key_bound_before_signing=true all_native_proofs=true transport=4800 full_catalog=false"
    );
    if !include_send {
        return;
    }

    let initial_send = send_chain::run_send_from_load(&load, PROFILE, Some(2));
    assert_eq!(
        initial_send.binding, bootstrap.binding,
        "Send must share the exact admitted A descriptor"
    );
    let mut catalog = program.catalog.clone();
    catalog.push(initial_send.key.clone());
    let program = Program::new(
        bootstrap_terminal(bootstrap, &trivial),
        catalog,
        &initial_root.binding,
    );
    let load = rebuild_bootstrap_load(&program, bootstrap, &initial_load);
    let send = send_chain::run_send_from_load(&load, PROFILE, Some(2));
    assert_eq!(send.binding, initial_send.binding);
    assert_eq!(send.key.to_bytes(), initial_send.key.to_bytes());
    assert_eq!(send.state.lineage[17], program.digest());
    let _outer = program.prove(send_terminal(&send), "Send-mask0");
    send.pallas
        .decide(
            &PinnedParams::<Ep>::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    eprintln!(
        "COMPACT_THREE_TERMINAL_CLOSURE source_keys_equal=true common_key_bound_before_signing=true all_native_proofs=true transport=4800 mask0_only=true full_catalog=false"
    );
}

#[test]
#[ignore = "actual compact Bootstrap/Load chains rebuilt under one immutable two-terminal key"]
fn compact_bootstrap_load_catalog_rebinds_every_proof_and_key() {
    catalog_roundtrip(false);
}

#[test]
#[ignore = "actual compact Bootstrap/Load/Send-mask0 rebuilt under one immutable three-terminal key"]
fn compact_bootstrap_load_send_catalog_rebinds_every_proof_and_key() {
    catalog_roundtrip(true);
}

#[test]
#[ignore = "genuine distinct payer Load and receiver Bootstrap rebuilt under one compact key"]
fn compact_distinct_wallets_share_the_exact_predecessor_catalog() {
    let wallets = compact_payer_load_and_receiver();
    assert_eq!(wallets.payer.binding, wallets.receiver.binding);
    assert_eq!(
        wallets.payer.key.to_bytes(),
        wallets.receiver.key.to_bytes()
    );
    assert_eq!(wallets.payer.proof.len(), 3712);
    assert_eq!(wallets.receiver.proof.len(), 3712);
    assert_ne!(
        wallets.payer.source.state.core[5..7],
        wallets.receiver.source.state.core[5..7]
    );
}
