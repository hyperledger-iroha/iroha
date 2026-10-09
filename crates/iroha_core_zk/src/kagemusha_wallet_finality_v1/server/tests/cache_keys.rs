//! Genuine small-circuit originals exercise cache eviction and strict reimport.

use super::*;
use iroha_pasta::{CancellationToken, Ep, Eq, PastaCurve, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness, create_proof_owned,
    cs::{Advice, Column, ConstraintSystem, Fixed, Instance, InstanceType, Rotation, Selector},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
    keys::{
        CosetCachePolicy, KeygenConfigV2, ProvingKey, keygen_pk_v2_cancellable,
        pk::artifact::ReadConfig,
    },
    pcs::ipa::PinnedParams,
    verify_full,
};

#[derive(Clone, Copy)]
struct SmallCircuit(u64);

#[derive(Clone, Copy)]
struct Columns {
    advice: Column<Advice>,
    fixed: Column<Fixed>,
    instance: Column<Instance>,
    selector: Selector,
}

impl<F: PastaField> Circuit<F> for SmallCircuit {
    type Config = Columns;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Columns {
        let advice = meta.advice_column();
        let fixed = meta.fixed_column();
        let instance = meta.instance_column(1);
        meta.enable_equality(advice);
        meta.enable_equality(instance);
        let selector = meta.selector();
        meta.create_gate("exact public fixed value", |cells| {
            vec![
                cells.query_selector(selector)
                    * (cells.query_advice(advice, Rotation::cur())
                        - cells.query_fixed(fixed, Rotation::cur())),
            ]
        });
        Columns {
            advice,
            fixed,
            instance,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Columns,
        mut layouter: impl Layouter<F>,
    ) -> std::result::Result<(), Error> {
        let cell = layouter.assign_region(
            || "selected constant",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                region.assign_fixed(config.fixed, 0, F::from(self.0))?;
                Ok(region
                    .assign_advice(config.advice, 0, Value::known(F::from(self.0)))?
                    .cell())
            },
        )?;
        layouter.constrain_instance(cell, config.instance, 0)
    }
}

fn actual_record<C: PastaCurve>(
    key: &ProvingKey<C>,
) -> (
    iroha_kagemusha_proof::finality::catalog::ArtifactRecord,
    Vec<u8>,
) {
    let bytes = key.artifact_bytes_v2().unwrap();
    let descriptor = key.binding().encoded();
    let vk = key.vk().to_bytes();
    (
        iroha_kagemusha_proof::finality::catalog::ArtifactRecord {
            // This test's record is local DATA, never a signed finality graph entry.
            name: vec![1],
            lengths: [descriptor.len() as u64, vk.len() as u64, bytes.len() as u64],
            sha256: [
                Sha256::digest(descriptor).into(),
                Sha256::digest(vk).into(),
                Sha256::digest(&bytes).into(),
            ],
        },
        bytes,
    )
}

fn check<C: PastaCurve>() {
    let (_temp, path) = root();
    let parameters = PinnedParams::<C>::derive(6).unwrap();
    let config = KeygenConfigV2::pipa_r(vec![InstanceType::Field]);
    let first = SmallCircuit(3);
    let second = SmallCircuit(5);
    let original = keygen_pk_v2_cancellable(&parameters, &first, &config, None).unwrap();
    let other = keygen_pk_v2_cancellable(&parameters, &second, &config, None).unwrap();
    let (record, bytes) = actual_record(&original);
    let (other_record, other_bytes) = actual_record(&other);
    assert_ne!(record.sha256[2], other_record.sha256[2]);
    let bound = bytes.len().max(other_bytes.len());
    let records = [record.clone(), other_record.clone()];
    let selection = b"explicit small-circuit engineering selection";
    let mut cache = cache::Cache::acquire(
        &path,
        selection,
        &records,
        bound,
        bound,
        custody::Mode::Initialize,
    )
    .unwrap();
    cache.put(&record, &bytes, None).unwrap();
    cache.put(&other_record, &other_bytes, None).unwrap();
    assert!(cache.get(&record, None).unwrap().is_none());
    drop(cache);

    let mut cache = cache::Cache::acquire(
        &path,
        selection,
        &records,
        bound,
        bound,
        custody::Mode::Open,
    )
    .unwrap();
    assert_eq!(
        cache.get(&other_record, None).unwrap().unwrap(),
        other_bytes
    );
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        keygen_pk_v2_cancellable(&parameters, &first, &config, Some(&cancelled))
            .err()
            .unwrap()
            .is_cancelled()
    );
    assert!(cache.get(&record, None).unwrap().is_none());
    assert_eq!(
        cache.get(&other_record, None).unwrap().unwrap(),
        other_bytes
    );

    let fresh = CancellationToken::new();
    let regenerated = keygen_pk_v2_cancellable(&parameters, &first, &config, Some(&fresh)).unwrap();
    let (regenerated_record, regenerated_bytes) = actual_record(&regenerated);
    assert_eq!(regenerated_record.lengths, record.lengths);
    assert_eq!(regenerated_record.sha256, record.sha256);
    assert_eq!(regenerated_bytes, bytes);
    cache
        .put(&record, &regenerated_bytes, Some(&fresh))
        .unwrap();
    assert!(cache.get(&other_record, None).unwrap().is_none());
    drop(cache);
    let cache = cache::Cache::acquire(
        &path,
        selection,
        &records,
        bound,
        bound,
        custody::Mode::Open,
    )
    .unwrap();
    let retained = cache.get(&record, Some(&fresh)).unwrap().unwrap();
    assert_eq!(retained, bytes);
    let read = ReadConfig {
        maximum_bytes: bound,
        maximum_rows: 1 << 6,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    };
    assert!(
        ProvingKey::<C>::from_artifact_v2_cancellable(
            &retained,
            original.binding(),
            &parameters,
            &second,
            read,
            Some(&fresh)
        )
        .is_err()
    );
    let imported = ProvingKey::<C>::from_artifact_v2_cancellable(
        &retained,
        original.binding(),
        &parameters,
        &first,
        read,
        Some(&fresh),
    )
    .unwrap();
    assert_eq!(imported.vk().to_bytes(), original.vk().to_bytes());
    assert_eq!(imported.artifact_bytes_v2().unwrap(), bytes);
    let instances = vec![vec![C::ScalarExt::from(first.0)]];
    let witness = Witness::from_circuit(&imported, &first, &instances).unwrap();
    let proof = create_proof_owned(
        &parameters,
        &imported,
        witness,
        ProverRandomness::hedged(),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &parameters,
        original.binding(),
        original.vk(),
        &instances,
        &proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let changed = vec![vec![C::ScalarExt::from(second.0)]];
    assert!(
        verify_full(
            &parameters,
            original.binding(),
            original.vk(),
            &changed,
            &proof,
            MemoryBudget::DEFAULT
        )
        .is_err()
    );
}

#[test]
fn actual_both_curve_keys_evict_regenerate_reopen_strictly_import_and_prove() {
    check::<Ep>();
    check::<Eq>();
}
