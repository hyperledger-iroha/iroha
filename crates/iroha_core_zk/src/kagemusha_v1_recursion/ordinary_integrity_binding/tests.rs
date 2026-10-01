//! Numeric reconstruction tests for exact canonical Integrity lease originals.
//! These test data identity and rejection; they grant no authenticated lease authority.

use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
#[test]
fn private_current_lease_uses_one_union_entry_point_in_both_fields() {
    let _ = super::super::ordinary_integrity_union::constrain_ordinary_integrity_union_v1::<Fp>;
    let _ = super::super::ordinary_integrity_union::constrain_ordinary_integrity_union_v1::<Fq>;
}

const TEST_K: u32 = 17;
const UNUSABLE: usize = 9;

#[derive(Clone)]
struct ReconstructionCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for ReconstructionCircuit<F> {
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
        }
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.k) - UNUSABLE;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("lease reconstruction uses fixed circuit parameters")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.0,
            layouter.namespace(|| "lease original reconstruction Base"),
        )?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1_usize << TEST_K) - UNUSABLE,
        )
    }
}

struct ReconstructionFixture<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    layout: KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1,
    cells: OrdinaryIntegrityLeaseCellsV1<F>,
    model_digest: DigestV1,
}

fn reconstruction_fixture<F: KagemushaPoseidonFieldV1>(
    complete: bool,
    substituted_field: bool,
) -> ReconstructionFixture<F> {
    // Known-public signed model originals only. This test grants no current lease authority.
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
    let (_, lease) = fixture.integrity_refresh_originals();
    let (layout, original, model_digest) = if complete {
        (
            lease.original_preimage_layout().unwrap(),
            lease.canonical_bytes().unwrap(),
            lease.canonical_digest().unwrap(),
        )
    } else {
        let original = lease.ed_only_canonical_bytes().unwrap();
        let digest = <[u8; 32]>::from(Sha256::digest(&original));
        (lease.ed_only_preimage_layout().unwrap(), original, digest)
    };
    let mut bytes = layout.bytes[..layout.original.start]
        .iter()
        .map(|byte| byte.expect("model-owned fixed original digest prefix"))
        .collect::<Vec<_>>();
    bytes.extend(original);
    assert_eq!(bytes.len(), layout.bytes.len());
    if substituted_field {
        bytes[layout.fixed_digest_bytes[0][0]] ^= 1;
    }
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(TEST_K as usize)
        .use_lookup_bits(16);
    let cells = {
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let mut positions = |indices: &[usize]| {
            assign_bytes(
                ctx,
                &range,
                &indices
                    .iter()
                    .map(|index| bytes[*index])
                    .collect::<Vec<_>>(),
            )
        };
        OrdinaryIntegrityLeaseCellsV1 {
            version: positions(&layout.version_bytes).try_into().unwrap(),
            fields: core::array::from_fn(|i| {
                positions(&layout.fixed_digest_bytes[i]).try_into().unwrap()
            }),
            scalars: core::array::from_fn(|i| {
                positions(&layout.scalar_bytes[i]).try_into().unwrap()
            }),
            signature: positions(&layout.signature_bytes).try_into().unwrap(),
            possession: positions(&layout.possession_signature_bytes),
            issuer_admission: layout.issuer_admission_layout.as_ref().map(|issuer| {
                OrdinaryCredentialIssuerCellsV1 {
                    ed_original_sha256: positions(&issuer.fixed_digest_bytes[2])
                        .try_into()
                        .unwrap(),
                    signature: positions(&issuer.signature_bytes).try_into().unwrap(),
                }
            }),
        }
    };
    ReconstructionFixture {
        builder,
        layout,
        cells,
        model_digest,
    }
}

fn matches_model_digest<F: KagemushaPoseidonFieldV1>(
    complete: bool,
    substituted_field: bool,
) -> bool {
    let mut fixture = reconstruction_fixture::<F>(complete, substituted_field);
    let mut jobs = PastaSha256JobsV1::default();
    let digest = reconstruct(
        &mut fixture.builder,
        &mut jobs,
        &fixture.layout,
        &fixture.cells,
    )
    .expect("maintained model layout reconstructs");
    let range = fixture.builder.range_chip();
    equal(
        fixture.builder.main(0),
        &range,
        &digest,
        &constant_bytes(&fixture.model_digest),
    )
    .expect("fixed digest width");
    fixture.builder.calculate_params(Some(UNUSABLE));
    let circuit = ReconstructionCircuit {
        builder: fixture.builder,
        jobs,
    };
    MockProver::run(TEST_K, &circuit, vec![])
        .expect("lease original SHA/CRC relation synthesizes")
        .verify()
        .is_ok()
}

#[derive(Clone, Copy)]
enum MalformedInput {
    TrailingFrame,
    FieldWidth,
    FieldOutsideFrame,
    MissingIssuer,
}

fn malformed_inputs_are_rejected<F: KagemushaPoseidonFieldV1>() {
    for mutation in [
        MalformedInput::TrailingFrame,
        MalformedInput::FieldWidth,
        MalformedInput::FieldOutsideFrame,
        MalformedInput::MissingIssuer,
    ] {
        let mut fixture = reconstruction_fixture::<F>(true, false);
        let expected = match mutation {
            MalformedInput::TrailingFrame => {
                fixture.layout.original.end += 1;
                "ordinary lease original layout has trailing bytes"
            }
            MalformedInput::FieldWidth => {
                assert!(fixture.cells.possession.pop().is_some());
                "ordinary lease original field width differs"
            }
            MalformedInput::FieldOutsideFrame => {
                fixture.layout.fixed_digest_bytes[0][0] = fixture.layout.original.end;
                "ordinary lease original position is outside the frame"
            }
            MalformedInput::MissingIssuer => {
                fixture.cells.issuer_admission = None;
                "ordinary full lease original lacks issuer cells"
            }
        };
        let actual = reconstruct(
            &mut fixture.builder,
            &mut PastaSha256JobsV1::default(),
            &fixture.layout,
            &fixture.cells,
        );
        assert_eq!(actual.err().as_deref(), Some(expected));
    }
}

#[test]
fn reconstruction_matches_model_ed_and_complete_original_digests_in_both_pasta_fields() {
    for complete in [false, true] {
        assert!(matches_model_digest::<Fp>(complete, false));
        assert!(matches_model_digest::<Fq>(complete, false));
    }
}

#[test]
fn reconstruction_rejects_substituted_fields_and_malformed_inputs_in_both_pasta_fields() {
    assert!(!matches_model_digest::<Fp>(true, true));
    assert!(!matches_model_digest::<Fq>(true, true));
    malformed_inputs_are_rejected::<Fp>();
    malformed_inputs_are_rejected::<Fq>();
}
