//! Distinct concrete ordinary pre-debit Mint circuit family.
//!
//! These keys cannot be interchanged with OEM MintAuthorization keys. The fixed mode gate is
//! reconstructed by the structured reader, independently of the public column width.
//! Native admission additionally authenticates the whole issuer original and current journal.

use super::{
    DigestV1,
    ordinary_issuer_config::{OrdinaryIssuerConfigV1, OrdinaryIssuerTableV1},
    provider_policy_root::ProviderPolicyRootConfigV1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ConfigV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue,
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
};
use halo2_proofs::{
    circuit::{Layouter, V1, Value},
    halo2curves::pasta::{Fp, Fq},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

pub(super) const ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1: usize = 113;
const UNUSABLE: usize = 9;
const MODE: u64 = 0x4f_4d_49_01;

#[derive(Clone, Debug)]
pub(crate) struct KagemushaOrdinaryMintCircuitParamsV1 {
    pub(crate) base: BaseCircuitParams,
    pub(crate) provider_policy_root: DigestV1,
    pub(super) issuer_table: OrdinaryIssuerTableV1,
}
impl Default for KagemushaOrdinaryMintCircuitParamsV1 {
    fn default() -> Self {
        Self {
            base: BaseCircuitParams::default(),
            provider_policy_root: [0; 32],
            issuer_table: OrdinaryIssuerTableV1::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OrdinaryMintConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    provider: ProviderPolicyRootConfigV1,
    issuer: OrdinaryIssuerConfigV1,
    mode: Column<Advice>,
    selected: Selector,
}

impl<F: KagemushaPoseidonFieldV1> OrdinaryMintConfig<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        params: KagemushaOrdinaryMintCircuitParamsV1,
    ) -> Self {
        let usable = (1_usize << params.base.k) - UNUSABLE;
        let mut base = BaseConfig::configure(meta, params.base);
        base.set_usable_rows(usable);
        let mode = meta.advice_column();
        let selected = meta.selector();
        meta.create_gate("Kagemusha ordinary pre-debit Mint role", |meta| {
            vec![
                meta.query_selector(selected)
                    * (meta.query_advice(mode, Rotation::cur())
                        - Expression::Constant(F::from(MODE))),
            ]
        });
        Self {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
            provider: ProviderPolicyRootConfigV1::configure(meta, params.provider_policy_root),
            issuer: OrdinaryIssuerConfigV1::configure(meta, &params.issuer_table),
            mode,
            selected,
        }
    }
}

#[derive(Clone)]
pub(crate) struct KagemushaOrdinaryMintEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
    pub(super) jobs: PastaSha256JobsV1<Fp>,
    pub(super) provider_policy_root: DigestV1,
    pub(super) provider_cells: [AssignedValue<Fp>; 2],
    pub(super) issuer_table: OrdinaryIssuerTableV1,
    pub(super) issuer_index: usize,
    pub(super) issuer_cells: [AssignedValue<Fp>; 65],
    pub(super) profile_cells: [AssignedValue<Fp>; 2],
}
#[derive(Clone)]
pub(crate) struct KagemushaOrdinaryMintEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
    pub(super) jobs: PastaSha256JobsV1<Fq>,
    pub(super) provider_policy_root: DigestV1,
    pub(super) provider_cells: [AssignedValue<Fq>; 2],
    pub(super) issuer_table: OrdinaryIssuerTableV1,
    pub(super) issuer_index: usize,
    pub(super) issuer_cells: [AssignedValue<Fq>; 65],
    pub(super) profile_cells: [AssignedValue<Fq>; 2],
}

macro_rules! ordinary_circuit {
    ($name:ty, $field:ty) => {
        impl Circuit<$field> for $name {
            type Config = OrdinaryMintConfig<$field>;
            type FloorPlanner = V1;
            type Params = KagemushaOrdinaryMintCircuitParamsV1;
            fn params(&self) -> Self::Params {
                Self::Params {
                    base: self.builder.config_params.clone(),
                    provider_policy_root: self.provider_policy_root,
                    issuer_table: self.issuer_table.clone(),
                }
            }
            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    jobs: self.jobs.unknown(),
                    provider_policy_root: self.provider_policy_root,
                    provider_cells: self.provider_cells,
                    issuer_table: self.issuer_table.clone(),
                    issuer_index: self.issuer_index,
                    issuer_cells: self.issuer_cells,
                    profile_cells: self.profile_cells,
                }
            }
            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                OrdinaryMintConfig::configure(meta, params)
            }
            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!("ordinary Mint requires release-authenticated parameters")
            }
            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), Error> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
            }
            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), Error> {
                self.builder
                    .synthesize(config.base, layouter.namespace(|| "ordinary Mint Base"))?;
                layouter.assign_region(
                    || "ordinary Mint concrete role",
                    |mut region| {
                        config.selected.enable(&mut region, 0)?;
                        region.assign_advice(config.mode, 0, Value::known(<$field>::from(MODE)));
                        Ok(())
                    },
                )?;
                config.provider.synthesize(
                    &mut layouter,
                    self.provider_cells,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                )?;
                config.issuer.synthesize(
                    &mut layouter,
                    self.issuer_index,
                    self.profile_cells,
                    self.issuer_cells,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                )?;
                self.jobs.synthesize(
                    &config.sha,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    (1_usize << self.builder.config_params.k) - UNUSABLE,
                )
            }
        }
    };
}
ordinary_circuit!(KagemushaOrdinaryMintEqCircuitV1, Fp);
ordinary_circuit!(KagemushaOrdinaryMintEpCircuitV1, Fq);

#[cfg(any(
    test,
    feature = "kagemusha-production-prover",
    feature = "kagemusha-real-proof-harness"
))]
#[path = "ordinary_mint_relation.rs"]
mod relation;
#[cfg(any(
    test,
    feature = "kagemusha-production-prover",
    feature = "kagemusha-real-proof-harness"
))]
pub(crate) use relation::{
    OrdinaryMintWitnessV1, build_ordinary_mint_ep_v1, build_ordinary_mint_eq_v1,
};
