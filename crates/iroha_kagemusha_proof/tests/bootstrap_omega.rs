//! Actual authenticated Bootstrap sigma/Q/A1/W/A2 wrapped by the complete Omega
//! predicate. Layout diagnostics do not establish the production transport cap.

/// Shared authenticated Bootstrap proof-chain fixture.
#[path = "a_recursive.rs"]
pub mod bootstrap_chain;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::omega::{OmegaCircuit, OmegaPlan, OmegaWitness};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, msm::MemoryBudget};
use iroha_plonk::{
    Protocol, ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check},
    create_proof_owned_with_claim,
    cs::{
        CircuitDescriptorV1, CircuitDescriptorV2, CurveV1, DescriptorConfig, InstanceModeV1,
        ProofSuffixV1, TranscriptV1, TranscriptV2,
    },
    frontend::synthesize,
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, create_fold, verifier::CompactSpans};

/// Checks the complete compact predicate and reports its exact row/byte gates.
pub(crate) fn compact_diagnostic(circuit: &OmegaCircuit, public: &[Vec<Fq>]) {
    // Explicitly diagnostic only. All output descriptors below still use k16.
    let spans = CompactSpans::new(32_768, 131_072, 262_138).unwrap();
    let diagnostic = circuit.clone().with_compact_layout(spans);
    let assigned = synthesize(&diagnostic, 18, Some(public)).unwrap();
    let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    let assigned_rows = assigned.tables.advice_assigned();
    let used = |start: usize, end: usize| {
        assigned_rows[..10]
            .iter()
            .filter_map(|column| column[start..end].iter().rposition(|v| *v).map(|r| r + 1))
            .max()
            .unwrap_or(0)
    };
    let sponge = used(0, 32_768).div_ceil(37) * 37;
    let arithmetic = used(32_768, 131_072);
    let curve = used(131_072, 262_138);
    let range = assigned_rows[10]
        .iter()
        .rposition(|v| *v)
        .map_or(0, |r| r + 1);
    // The compact profile allocates eight phase columns, then
    // two tuple-table columns, top width and range pattern. Audit the actual fixed tape,
    // not a count extrapolated from the host interpreter's operations.
    let fixed = assigned.tables.fixed();
    assert_eq!(fixed.len(), 12);
    let pattern = &fixed[11];
    let widths_column = &fixed[10];
    let mut widths = std::collections::BTreeMap::<usize, (usize, usize)>::new();
    let mut row = 16;
    while row < range {
        let start = row;
        while pattern[row] == Fq::ONE {
            row += 1;
        }
        let steps = row - start;
        let top = if pattern[row] == Fq::from(3) {
            1
        } else if pattern[row] == Fq::from(4) {
            2
        } else {
            assert_eq!(pattern[row], Fq::from(2));
            (3..=15)
                .find(|bits| widths_column[row] == Fq::from(*bits as u64))
                .unwrap()
        };
        row += 1;
        let entry = widths.entry(15 * steps + top).or_default();
        entry.0 += 1;
        entry.1 += row - start;
    }
    assert_eq!(widths.values().map(|v| v.1).sum::<usize>() + 16, range);
    eprintln!("AUTHENTICATED_SOURCE_COMPACT_RANGE widths_count_rows={widths:?}");
    let pure_glue = (32_768..32_768 + arithmetic)
        .filter(|row| fixed[0][*row] == Fq::ZERO && fixed[1][*row] == Fq::ZERO)
        .count();
    let mut kernels = std::collections::BTreeMap::<&str, usize>::new();
    for (row, phase) in fixed[0].iter().enumerate().skip(32_768).take(arithmetic) {
        if *phase != Fq::ONE || fixed[1][row] != Fq::ZERO {
            continue;
        }
        for (name, active) in [
            ("mul", fixed[7][row] == Fq::from(3)),
            ("div", fixed[7][row] == Fq::from(4)),
            ("proper_finish", fixed[7][row] == Fq::from(5)),
            (
                "wide_finish",
                fixed[4][row] == Fq::from(4) && fixed[5][row] == Fq::ZERO,
            ),
            (
                "narrow_finish",
                fixed[4][row] == Fq::from(4) && fixed[5][row] == Fq::from(3),
            ),
        ] {
            if active {
                *kernels.entry(name).or_default() += 1;
            }
        }
    }
    eprintln!("AUTHENTICATED_SOURCE_COMPACT_KERNELS counts={kernels:?}");
    let sponge_spare = (16..sponge)
        .filter(|row| {
            fixed[1][*row] == Fq::ONE
                && [0, 1, 2, 4, 5, 9]
                    .iter()
                    .all(|col| !assigned_rows[*col][*row])
        })
        .count();
    eprintln!(
        "AUTHENTICATED_SOURCE_COMPACT_SPARE pure_glue_rows={pure_glue} staged_ff_rows={} sponge_six_spare_rows={sponge_spare} optimistic_87bit_secondary_checks={} secondary_not_implemented=true",
        arithmetic - pure_glue,
        pure_glue / 8 + sponge_spare / 6
    );

    let total = sponge + arithmetic + curve;
    let finalized = assigned
        .cs
        .finalize(assigned.tables.selectors(), true)
        .unwrap();
    let layout = CircuitDescriptorV1::from_constraint_system(
        &finalized,
        DescriptorConfig {
            curve: CurveV1::Pallas,
            k: 16,
            transcript: TranscriptV1::Blake2bChallenge255,
            instance_mode: InstanceModeV1::Direct,
            proof_suffix: ProofSuffixV1::FoldedGenerator,
        },
    )
    .unwrap();
    let descriptor = CircuitDescriptorV2::from_layout(
        layout,
        TranscriptV2::KagemushaPoseidonRp57Base,
        OmegaPlan::instance_types().to_vec(),
    )
    .unwrap();
    let protocol = Protocol::new(&descriptor).unwrap();
    eprintln!(
        "AUTHENTICATED_SOURCE_COMPACT_OMEGA sponge={sponge} arithmetic={arithmetic} curve={curve} shared_total={total} range={range} shape={:?} descriptor_transport={} actual_outer_proof=false actual_authenticated_terminal_A=true production_k16_fit={}",
        protocol.shape(),
        protocol.proof_length() + 1088,
        total <= protocol.shape().usable_rows && range <= protocol.shape().usable_rows
    );
    drop(assigned.tables);
    let packed = circuit
        .clone()
        .with_compact_layout(CompactSpans::new(sponge, sponge + arithmetic, total).unwrap());
    if total > protocol.shape().usable_rows || range > protocol.shape().usable_rows {
        assert!(synthesize(&packed, 16, Some(public)).is_err());
    }
}

fn wrapper(
    bootstrap: &bootstrap_chain::AuthenticatedBootstrap,
) -> (OmegaCircuit, Vec<Vec<Fq>>, AccumulatorT<Eq>) {
    let params = PinnedParams::<Eq>::derive(16).unwrap();
    let trivial = AccumulatorT::trivial(&params, MemoryBudget::DEFAULT).unwrap();
    let (fold, output) = create_fold(
        &params,
        &[
            bootstrap.vesta_part.as_input(),
            bootstrap.opening.clone(),
            trivial.as_input(),
            trivial.as_input(),
        ],
        Fq::from(101).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    output.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = output.g().coordinates().unwrap();
    let public = vec![
        vec![Fq::from_repr(bootstrap.instances[0].to_repr()).unwrap()],
        vec![x, y],
        output
            .challenges()
            .iter()
            .map(|v| Fq::from_repr(v.to_repr()).unwrap())
            .collect(),
    ];
    let digest = bootstrap.key.kagemusha_digest(&bootstrap.binding).unwrap();
    let plan = OmegaPlan::new(bootstrap.binding.clone(), params, vec![digest]).unwrap();
    let source_bytes = bootstrap.proof.len();
    let circuit = OmegaCircuit::new(
        plan,
        OmegaWitness {
            key: bootstrap.key.clone(),
            instances: bootstrap.instances.clone(),
            length: u32::try_from(source_bytes).unwrap(),
            proof: bootstrap.proof.clone(),
            fold: fold.to_bytes(),
        },
    )
    .unwrap();
    (circuit, public, output)
}

/// Genuine rooted Bootstrap outer artifact for following-operation tests.
/// The fixed catalog currently contains Bootstrap only; full catalog and byte
/// qualification remain open and changing that catalog requires rebinding.
#[allow(dead_code)]
pub(crate) struct RootedBootstrapOmega {
    pub(crate) source: bootstrap_chain::AuthenticatedBootstrap,
    pub(crate) key: iroha_plonk::VerifyingKey<Ep>,
    pub(crate) binding: iroha_plonk::DescriptorBinding,
    pub(crate) proof: Vec<u8>,
    pub(crate) instances: Vec<Vec<Fq>>,
    pub(crate) opening: iroha_plonk_recursion::FoldInput<Ep>,
    pub(crate) vesta: AccumulatorT<Eq>,
}

/// Rebuilds all authenticated witnesses against the actual immutable outer key.
pub(crate) fn rooted_bootstrap_omega(diagnostics: bool) -> RootedBootstrapOmega {
    rooted_bootstrap_omega_with_layout(diagnostics, 0)
}

/// Rooted component with an explicit common source profile. Shared-range
/// profiles disable Omega selector compression so catalog size cannot alter
/// the predecessor descriptor before the terminal keys are constructed.
pub(crate) fn rooted_bootstrap_omega_with_layout(
    diagnostics: bool,
    source_buses: usize,
) -> RootedBootstrapOmega {
    rooted_bootstrap_omega_with_q_layout(diagnostics, source_buses, None)
}

/// Explicit candidate Q/source profile, identically used before and after rebinding.
pub(crate) fn rooted_bootstrap_omega_with_q_layout(
    diagnostics: bool,
    source_buses: usize,
    q_buses: Option<usize>,
) -> RootedBootstrapOmega {
    let first = bootstrap_chain::authenticated_bootstrap_with_q_layout(
        false,
        Fp::from(91),
        source_buses,
        q_buses,
    );
    let (first_circuit, _, _) = wrapper(&first);
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let mut config = KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec());
    config.compress_selectors = source_buses == 0;
    let first_key = keygen_pk_v2(&params, &first_circuit, &config).unwrap();
    let omega_digest = first_key
        .vk()
        .kagemusha_digest(first_key.binding())
        .unwrap();
    let first_binding = first_key.binding().clone();
    let first_vk = first_key.vk().clone();
    drop(first_key);
    // Rebuild every signed object and proof with the actual admitted Omega key
    // digest, then prove that neither A nor Omega key depends on that witness.
    let bootstrap = bootstrap_chain::authenticated_bootstrap_with_q_layout(
        false,
        omega_digest,
        source_buses,
        q_buses,
    );
    assert_eq!(first.binding, bootstrap.binding);
    assert_eq!(
        first.key.kagemusha_digest(&first.binding).unwrap(),
        bootstrap.key.kagemusha_digest(&bootstrap.binding).unwrap()
    );
    eprintln!(
        "AUTHENTICATED_BOOTSTRAP_A_SOURCE shape={:?}",
        Protocol::new(bootstrap.binding.descriptor())
            .unwrap()
            .shape()
    );
    let source_bytes = bootstrap.proof.len();
    let (circuit, public, vesta) = wrapper(&bootstrap);
    if diagnostics {
        compact_diagnostic(&circuit, &public);
    }
    let assigned = synthesize(&circuit, 16, Some(&public)).unwrap();
    let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    let lanes: Vec<_> = assigned
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .collect();
    drop(assigned);
    let key = keygen_pk_v2(&params, &circuit, &config).unwrap();
    assert_eq!(key.binding(), &first_binding);
    assert_eq!(key.vk().to_bytes(), first_vk.to_bytes());
    let witness = Witness::from_circuit(&key, &circuit, &public).unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        witness,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .unwrap();
    let verified = accumulate_generator(
        &params,
        key.binding(),
        key.vk(),
        &public,
        &proof.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    verified.decide(&params, MemoryBudget::DEFAULT).unwrap();
    eprintln!(
        "AUTHENTICATED_BOOTSTRAP_OMEGA source_A2_bytes={source_bytes} lanes={lanes:?} actual_proof_bytes={} transport_bytes={} source_operation_authenticated=true actual_omega_key_bound=true production_size_pass={} release_qualified=false",
        proof.proof.len(),
        proof.proof.len() + 1088,
        proof.proof.len() + 1088 <= 4821
    );
    RootedBootstrapOmega {
        source: bootstrap,
        key: key.vk().clone(),
        binding: key.binding().clone(),
        proof: proof.proof,
        instances: public,
        opening: iroha_plonk_recursion::FoldInput::from_opening(
            *verified.g(),
            verified.challenges(),
        )
        .unwrap(),
        vesta,
    }
}

#[test]
#[ignore = "full authenticated Bootstrap chain and outer proof; run optimized"]
fn authenticated_bootstrap_reaches_complete_outer_predicate() {
    let _ = rooted_bootstrap_omega(true);
}

#[test]
#[ignore = "actual authenticated source A using parallel serialized FF; run optimized"]
fn serialized_foreign_bootstrap_source_and_outer_inventory() {
    serialized_foreign_inventory(4);
}

#[test]
#[ignore = "actual authenticated five-bus source candidate and outer inventory; run optimized"]
fn five_bus_bootstrap_source_and_outer_inventory() {
    serialized_foreign_inventory(5);
}

#[test]
#[ignore = "actual explicit two-bus Q and three-bus A profiles, then full compact Omega; run optimized"]
fn reduced_q_three_bus_bootstrap_source_and_outer_inventory() {
    let bootstrap = bootstrap_chain::authenticated_bootstrap_with_q_layout(
        false,
        iroha_pasta::Fp::from(91),
        3,
        Some(2),
    );
    source_outer_inventory(&bootstrap, 3);
}

#[test]
#[ignore = "actual explicit two-bus Q and four-bus A profiles, then full compact Omega; run optimized"]
fn reduced_q_four_bus_bootstrap_source_and_outer_inventory() {
    let bootstrap = bootstrap_chain::authenticated_bootstrap_with_q_layout(
        false,
        iroha_pasta::Fp::from(91),
        4,
        Some(2),
    );
    source_outer_inventory(&bootstrap, 4);
}

fn serialized_foreign_inventory(range_buses: usize) {
    let bootstrap = bootstrap_chain::authenticated_bootstrap_with_layout(
        false,
        iroha_pasta::Fp::from(91),
        range_buses,
    );
    source_outer_inventory(&bootstrap, range_buses);
}

fn source_outer_inventory(bootstrap: &bootstrap_chain::AuthenticatedBootstrap, range_buses: usize) {
    let source = Protocol::new(bootstrap.binding.descriptor()).unwrap();
    eprintln!(
        "SERIALIZED_FF_BOOTSTRAP_A_SOURCE total_buses={range_buses} shape={:?} actual_proof_bytes={}",
        source.shape(),
        bootstrap.proof.len()
    );
    let (circuit, public, _) = wrapper(bootstrap);
    compact_diagnostic(&circuit, &public);
    let pinned = circuit
        .clone()
        .with_key_catalog(vec![bootstrap.key.clone()])
        .unwrap();
    eprintln!("AUTHENTICATED_BOOTSTRAP_PINNED_KEY_CATALOG entries=1 full_catalog=false");
    compact_diagnostic(&pinned, &public);
}
