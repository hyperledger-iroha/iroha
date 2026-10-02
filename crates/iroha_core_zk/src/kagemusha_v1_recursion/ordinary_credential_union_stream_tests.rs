//! Native fixed-K16 selected-original SHA and adversarial option/length/CRC controls.
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
const K: u32 = 16;
#[derive(Clone, Debug)]
struct Config<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
}
#[derive(Clone)]
struct SelectedCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for SelectedCircuit<F> {
    type Config = Config<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;
    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }
    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
        }
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1usize << K) - 9);
        Config {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
        }
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("fixed native selected-original parameters")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "selected original Base"),
        )?;
        self.jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1usize << K) - 9,
        )
    }
}
fn raw(apple: bool, integrity: bool) -> KagemushaOrdinaryAppCredentialV1 {
    let f = if integrity {
        assert!(!apple);
        Fixture::android_with_integrity()
    } else {
        Fixture::new(apple)
    };
    let token = f.verify(300).unwrap();
    KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(token.app_credential().original())
        .unwrap()
}
fn both_originals<F: KagemushaPoseidonFieldV1>(apple: bool, integrity: bool) {
    use super::super::ordinary_app_guard_binding::OrdinaryCredentialIssuerCellsV1;
    let raw = raw(apple, integrity);
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(8)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let mut union = assign_ordinary_credential_union_v1(&mut builder, &raw).unwrap();
    let ed = reconstruct_ordinary_credential_union_v1(&mut builder, &mut jobs, &raw, &union, false)
        .unwrap();
    let range = builder.range_chip();
    let signature = assign_bytes(
        builder.main(0),
        &range,
        raw.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .unwrap();
    union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed,
        signature,
    });
    let full =
        reconstruct_ordinary_credential_union_v1(&mut builder, &mut jobs, &raw, &union, true)
            .unwrap();
    builder.assigned_instances = vec![
        ed.into_iter()
            .chain(full)
            .map(|b| b.assigned().unwrap())
            .collect(),
    ];
    let mut expected = Sha256::digest(raw.ed_only_canonical_bytes().unwrap()).to_vec();
    expected.extend(raw.canonical_digest().unwrap());
    let expected = expected
        .into_iter()
        .map(|b| F::from(u64::from(b)))
        .collect::<Vec<_>>();
    builder.calculate_params(Some(9));
    let profile = jobs.capacity_profile().unwrap();
    assert_eq!(profile.0, 2);
    jobs.validate_capacity((1usize << K) - 9).unwrap();
    println!(
        "credential selected originals apple={apple} integrity={integrity} jobs/blocks/rows={profile:?}"
    );
    let circuit = SelectedCircuit { builder, jobs };
    assert!(
        MockProver::run(K, &circuit, vec![expected.clone()])
            .unwrap()
            .verify()
            .is_ok()
    );
    for byte in [0, 31, 32, 63] {
        let mut changed = expected.clone();
        changed[byte] += F::ONE;
        assert!(
            MockProver::run(K, &circuit, vec![changed])
                .unwrap()
                .verify()
                .is_err()
        );
    }
}
#[test]
fn exact_native_ed_and_full_selected_digests_match_all_options_and_both_fields() {
    for (apple, integrity) in [(false, false), (false, true), (true, false)] {
        both_originals::<Fp>(apple, integrity);
        both_originals::<Fq>(apple, integrity);
    }
}

/// Update every copy/lookup alias so the relation must reject the arithmetic substitution.
fn replace<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    target: AssignedValue<F>,
    value: F,
) {
    let target = target.cell.unwrap();
    let equalities = builder
        .core()
        .copy_manager
        .lock()
        .unwrap()
        .advice_equalities
        .clone();
    let mut cells = BTreeSet::from([target]);
    loop {
        let old = cells.len();
        for (a, b) in &equalities {
            if cells.contains(a) || cells.contains(b) {
                cells.insert(*a);
                cells.insert(*b);
            }
        }
        if old == cells.len() {
            break;
        }
    }
    for cell in &cells {
        assert_eq!(cell.context_id(), 0);
        assert_eq!(cell.type_id(), target.type_id());
        builder
            .main(0)
            .replace_advice_with_trivial(cell.offset(), value);
    }
    let replacement = builder
        .main(0)
        .get(isize::try_from(target.offset()).unwrap())
        .value;
    for manager in builder.lookup_manager() {
        for row in manager
            .cells_to_lookup
            .lock()
            .unwrap()
            .values_mut()
            .flatten()
        {
            for lookup in row {
                if lookup.cell.is_some_and(|c| cells.contains(&c)) {
                    lookup.value = replacement;
                }
            }
        }
    }
}
fn selected_outputs<F: KagemushaPoseidonFieldV1>() {
    for selected in [false, true] {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(10)
            .use_lookup_bits(8)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let no = assign_bytes(ctx, &range, &[3, 4]);
        let yes = assign_bytes(ctx, &range, &[7, 8, 9, 10]);
        let selector = ctx.load_witness(F::from(u64::from(selected)));
        let stream = select_credential_original_v1(&mut builder, selector, &yes, &no).unwrap();
        let outputs = stream
            .bytes()
            .iter()
            .map(|b| b.assigned().unwrap())
            .collect::<Vec<_>>();
        let len = stream.actual_len();
        builder.assigned_instances = vec![outputs.clone()];
        builder.calculate_params(Some(9));
        let expected = if selected {
            vec![7, 8, 9, 10]
        } else {
            vec![3, 4, 0, 0]
        }
        .into_iter()
        .map(F::from)
        .collect::<Vec<_>>();
        assert!(
            MockProver::run(10, &builder, vec![expected.clone()])
                .unwrap()
                .verify()
                .is_ok()
        );
        for (index, cell) in outputs.iter().copied().enumerate() {
            let mut bad = builder.deep_clone();
            let replacement = expected[index] + F::ONE;
            replace(&mut bad, cell, replacement);
            let mut claimed = expected.clone();
            claimed[index] = replacement;
            assert!(
                MockProver::run(10, &bad, vec![claimed])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
        let mut bad = builder.deep_clone();
        replace(&mut bad, len, F::from(if selected { 3 } else { 1 }));
        assert!(
            MockProver::run(10, &bad, vec![expected.clone()])
                .unwrap()
                .verify()
                .is_err()
        );
        let mut bad = builder.deep_clone();
        replace(&mut bad, selector, F::from(2));
        assert!(
            MockProver::run(10, &bad, vec![expected])
                .unwrap()
                .verify()
                .is_err()
        );
    }
}
#[test]
fn selected_stream_rejects_forged_bit_length_selected_bytes_and_unselected_tail_both_fields() {
    selected_outputs::<Fp>();
    selected_outputs::<Fq>();
}

fn original_crc<F: KagemushaPoseidonFieldV1>(integrity: bool) {
    let raw = raw(false, integrity);
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(8)
        .use_instance_columns(1);
    let union = assign_ordinary_credential_union_v1(&mut builder, &raw).unwrap();
    let mut none = raw.clone();
    none.subject.play_integrity = None;
    let mut some = raw.clone();
    some.subject.play_integrity = Some(KagemushaPlayIntegrityBindingV1 {
        request_hash: [1; 32],
        evidence_digest: [1; 32],
        policy_digest: [1; 32],
        verified_at_ms: 1,
        refresh_before_ms: 2,
    });
    let no_layout = none.ed_only_preimage_layout().unwrap();
    let yes_layout = some.ed_only_preimage_layout().unwrap();
    let mut no_cells = union.cells.clone();
    no_cells.play_integrity_fields = None;
    let no = assemble_ordinary_credential_original_v1(&mut builder, &no_layout, &no_cells).unwrap();
    let yes =
        assemble_ordinary_credential_original_v1(&mut builder, &yes_layout, &union.cells).unwrap();
    let stream = select_credential_original_v1(&mut builder, union.integrity, &yes, &no).unwrap();
    builder.assigned_instances = vec![vec![stream.actual_len()]];
    builder.calculate_params(Some(9));
    let expected = vec![F::from(if integrity { yes.len() } else { no.len() } as u64)];
    assert!(
        MockProver::run(K, &builder, vec![expected.clone()])
            .unwrap()
            .verify()
            .is_ok()
    );
    // These are the model frame's computed checksum slots, not caller-supplied expected bytes.
    for (original, layout) in [(&no, &no_layout), (&yes, &yes_layout)] {
        let index = layout.original.start + 31;
        let target = original[index].assigned().unwrap();
        let value = F::from(u64::from(original[index].test_value() ^ 0x40));
        let mut bad = builder.deep_clone();
        replace(&mut bad, target, value);
        assert!(
            MockProver::run(K, &bad, vec![expected.clone()])
                .unwrap()
                .verify()
                .is_err(),
            "even the unselected original keeps its actual CRC equation"
        );
    }
}
#[test]
fn selected_and_unselected_original_crc_equations_reject_mutation_both_fields() {
    for integrity in [false, true] {
        original_crc::<Fp>(integrity);
        original_crc::<Fq>(integrity);
    }
}
