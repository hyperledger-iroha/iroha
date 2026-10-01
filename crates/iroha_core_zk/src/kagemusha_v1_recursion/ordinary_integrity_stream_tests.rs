//! One bounded canonical stream, real SHA/CRC, and identical wiring at every DER width.
//!
//! These data-only known-public originals do not admit a refresh lease or authenticate issuance.

use super::super::guard_bundle::assign_bytes;
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::{
    ContextCell, ContextTag,
    gates::circuit::{BaseCircuitParams, BaseConfig},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaPlayIntegrityRefreshLeaseV1,
};
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
use sha2::{Digest as _, Sha256};

const K: u32 = 17;
const UNUSABLE: usize = 9;
#[derive(Clone)]
struct StreamCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
    expected: [u8; 32],
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for StreamCircuit<F> {
    type Config = (BaseConfig<F>, PastaSha256ConfigV1);
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;
    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }
    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
            expected: self.expected,
        }
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1 << K) - UNUSABLE);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("bounded stream has fixed params")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.0, layouter.namespace(|| "bounded lease stream Base"))?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1 << K) - UNUSABLE,
        )
    }
}

fn stream_circuit<F: KagemushaPoseidonFieldV1>(
    complete: bool,
    width: Option<usize>,
    mutate: bool,
) -> StreamCircuit<F> {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
    let (_, mut lease) = fixture.integrity_refresh_originals();
    if let Some(width) = width {
        // Codec-width specimens only. The unchanged public signature is not asserted valid.
        lease.app_possession = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: vec![0x5a; width],
        };
        lease.circuit_admission.subject =
            KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
                &lease.subject,
                &lease.signature,
                &lease.app_possession,
            )
            .unwrap();
    }
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(16)
        .use_instance_columns(1);
    let layout = lease.ed_only_preimage_layout().unwrap();
    let mut raw = lease.ed_only_canonical_bytes().unwrap();
    if mutate {
        raw[layout.fixed_digest_bytes[0][5]] ^= 1;
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut positions = |indices: &[usize]| {
        assign_bytes(
            ctx,
            &range,
            &indices.iter().map(|i| raw[*i]).collect::<Vec<_>>(),
        )
    };
    let mut cells = OrdinaryIntegrityLeaseCellsV1 {
        version: positions(&layout.version_bytes).try_into().unwrap(),
        fields: core::array::from_fn(|i| {
            positions(&layout.fixed_digest_bytes[i]).try_into().unwrap()
        }),
        scalars: core::array::from_fn(|i| positions(&layout.scalar_bytes[i]).try_into().unwrap()),
        signature: positions(&layout.signature_bytes).try_into().unwrap(),
        possession: Vec::new(),
        issuer_admission: None,
    };
    let der = match &lease.app_possession {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => signature_der,
        _ => unreachable!(),
    };
    cells.possession = assign_bytes(
        ctx,
        &range,
        &(0..72)
            .map(|i| der.get(i).copied().unwrap_or(0))
            .collect::<Vec<_>>(),
    );
    if complete {
        cells.issuer_admission = Some(
            super::super::ordinary_app_guard_binding::OrdinaryCredentialIssuerCellsV1 {
                ed_original_sha256: assign_bytes(
                    ctx,
                    &range,
                    &lease.circuit_admission.subject.ed_original_sha256,
                )
                .try_into()
                .unwrap(),
                signature: assign_bytes(
                    ctx,
                    &range,
                    lease.circuit_admission.signature.as_raw_bytes(),
                )
                .try_into()
                .unwrap(),
            },
        );
    }
    let length = ctx.load_witness(F::from(der.len() as u64));
    let grammar = if complete {
        lease.original_canonical_stream_grammar().unwrap()
    } else {
        lease.ed_only_canonical_stream_grammar().unwrap()
    };
    let expected = if complete {
        lease.canonical_digest().unwrap()
    } else {
        Sha256::digest(lease.ed_only_canonical_bytes().unwrap()).into()
    };
    let mut jobs = PastaSha256JobsV1::default();
    let digest =
        reconstruct_ordinary_integrity_stream_v1(&mut builder, &mut jobs, &grammar, &cells, length)
            .unwrap();
    builder.assigned_instances[0].extend(digest.iter().map(|byte| byte.assigned().unwrap()));
    builder.calculate_params(Some(UNUSABLE));
    StreamCircuit {
        builder,
        jobs,
        expected,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Shape<F> {
    columns: Vec<usize>,
    lookups: Vec<usize>,
    fixed: usize,
    advice_rows: Vec<usize>,
    selectors: Vec<Vec<bool>>,
    copies: Vec<(ContextCell, ContextCell)>,
    constants: Vec<(F, ContextCell)>,
    lookup_cells: Vec<(usize, ContextTag, ContextCell)>,
    instances: Vec<Vec<ContextCell>>,
}
fn shape<F: KagemushaPoseidonFieldV1>(c: &StreamCircuit<F>) -> Shape<F> {
    let mut lookup_cells = Vec::new();
    for (phase, manager) in c.builder.lookup_manager().iter().enumerate() {
        let rows = manager.cells_to_lookup.lock().unwrap();
        for (tag, cells) in rows.iter() {
            for row in cells {
                lookup_cells.push((phase, *tag, row[0].cell.unwrap()));
            }
        }
    }
    let copies = c.builder.core().copy_manager.lock().unwrap();
    Shape {
        lookup_cells,
        instances: c
            .builder
            .assigned_instances
            .iter()
            .map(|column| column.iter().map(|value| value.cell.unwrap()).collect())
            .collect(),
        columns: c.builder.config_params.num_advice_per_phase.clone(),
        lookups: c.builder.config_params.num_lookup_advice_per_phase.clone(),
        fixed: c.builder.config_params.num_fixed,
        advice_rows: c
            .builder
            .core()
            .phase_manager
            .iter()
            .flat_map(|p| p.threads.iter().map(|t| t.advice_len()))
            .collect(),
        selectors: c
            .builder
            .core()
            .phase_manager
            .iter()
            .flat_map(|p| {
                p.threads
                    .iter()
                    .map(|t| t.selector.iter().copied().collect())
            })
            .collect(),
        copies: copies.advice_equalities.clone(),
        constants: copies
            .constant_equalities
            .iter()
            .map(|(v, c)| (*v, *c))
            .collect(),
    }
}
fn check<F: KagemushaPoseidonFieldV1>(complete: bool, width: Option<usize>, mutate: bool) -> bool {
    let circuit = stream_circuit::<F>(complete, width, mutate);
    let instance = circuit
        .expected
        .iter()
        .map(|byte| F::from(u64::from(*byte)))
        .collect();
    MockProver::run(K, &circuit, vec![instance])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn actual_model_original_stream_sha_and_crc_match_in_both_fields() {
    for complete in [false, true] {
        assert!(check::<Fp>(complete, None, false));
        assert!(check::<Fq>(complete, None, false));
    }
}
#[test]
fn original_credential_scope_substitution_changes_the_canonical_digest_in_both_fields() {
    assert!(!check::<Fp>(true, None, true));
    assert!(!check::<Fq>(true, None, true));
}
fn check_shape<F: KagemushaPoseidonFieldV1>() {
    for complete in [false, true] {
        let baseline = stream_circuit::<F>(complete, Some(8), false);
        let expected = shape(&baseline);
        for width in [9, 31, 63, 64, 70, 71, 72] {
            let actual = stream_circuit::<F>(complete, Some(width), false);
            assert_eq!(shape(&actual), expected, "complete {complete}, DER {width}");
            assert_eq!(
                shape(&StreamCircuit {
                    builder: actual.builder.deep_clone().unknown(true),
                    jobs: actual.jobs.unknown(),
                    expected: actual.expected,
                }),
                expected
            );
        }
    }
}
#[test]
fn stream_columns_selectors_and_copy_wiring_are_fixed_across_der_lengths() {
    check_shape::<Fp>();
    check_shape::<Fq>();
}
