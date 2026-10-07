//! Both-curve verifier metadata rejects foreign stages and outlives proving buffers.

use super::*;
use ff::Field;
use iroha_pasta::{Ep, Eq, PastaField};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{GlueChip, GlueConfig};

#[derive(Clone)]
struct KeyShape<F: PastaField>(F);
impl<F: PastaField> Circuit<F> for KeyShape<F> {
    type Config = (GlueConfig, Column<Instance>);
    type Params = ();
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = core::array::from_fn(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let glue = GlueConfig::configure(meta, advice, fixed);
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        (glue, public)
    }
    fn synthesize(
        &self,
        (config, public): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let word = layouter.assign_region(
            || "distinct fixed source identity",
            |mut region| GlueChip::new(config).constant(&mut region, self.0),
        )?;
        layouter.constrain_instance(word.cell(), public, 0)
    }
}
fn identity<C: PastaCurve>() {
    let params = PinnedParams::<C>::derive(8).unwrap();
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    let original = keygen_pk_v2(&params, &KeyShape(C::ScalarExt::ONE), &config).unwrap();
    let foreign = keygen_pk_v2(&params, &KeyShape(C::ScalarExt::from(2)), &config).unwrap();
    assert_eq!(original.binding(), foreign.binding());
    assert_ne!(original.vk().to_bytes(), foreign.vk().to_bytes());
    let artifact = KeyArtifact::new(original.binding().clone(), original.vk().clone()).unwrap();
    assert_eq!(artifact.require_prover(&original), Ok(()));
    assert_eq!(
        artifact.require_prover(&foreign),
        Err(ArtifactError::Identity)
    );
    let descriptor = artifact.binding().encoded().to_vec();
    let verifier = artifact.key().to_bytes().to_vec();
    drop(original);
    assert_eq!(artifact.binding().encoded(), descriptor);
    assert_eq!(artifact.key().to_bytes(), verifier);
    let larger = keygen_pk_v2(
        &PinnedParams::<C>::derive(9).unwrap(),
        &KeyShape(C::ScalarExt::ONE),
        &config,
    )
    .unwrap();
    assert_eq!(
        artifact.require_prover(&larger),
        Err(ArtifactError::Identity)
    );
    assert!(matches!(
        KeyArtifact::new(larger.binding().clone(), foreign.vk().clone()),
        Err(ArtifactError::Identity)
    ));
}
#[test]
fn pallas_metadata_rejects_foreign_stage_and_releases_proving_buffers() {
    identity::<Ep>();
}
#[test]
fn vesta_metadata_rejects_foreign_stage_and_releases_proving_buffers() {
    identity::<Eq>();
}
