//! Independently governed ordinary issuer keys fixed in the reconstructed constraint system.
//!
//! A profile and its issuer point are selected together. Empty padding cannot be selected. The
//! app's own approval point is unrelated to this issuer table. Native key readers derive it only
//! from the threshold-admitted release; a witness cannot replace a governed issuer public key.

use super::DigestV1;
use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs};
use halo2_base::AssignedValue;
use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};
use iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1;

pub(super) const ORDINARY_ISSUER_SLOTS: usize = 64;
#[derive(Clone, Copy, Debug)]
pub(super) struct OrdinaryIssuerProfileV1 {
    pub(super) profile_id: DigestV1,
    pub(super) issuer_sec1: [u8; 65],
}
#[derive(Clone, Debug)]
pub(super) struct OrdinaryIssuerTableV1 {
    pub(super) slots: [OrdinaryIssuerProfileV1; ORDINARY_ISSUER_SLOTS],
}
impl Default for OrdinaryIssuerTableV1 {
    fn default() -> Self {
        Self {
            slots: [OrdinaryIssuerProfileV1 {
                profile_id: [0; 32],
                issuer_sec1: [0; 65],
            }; ORDINARY_ISSUER_SLOTS],
        }
    }
}
impl OrdinaryIssuerTableV1 {
    pub(super) fn from_release(release: &KagemushaAuthenticatedReleaseV1) -> Result<Self, String> {
        let mut slots = [OrdinaryIssuerProfileV1 {
            profile_id: [0; 32],
            issuer_sec1: [0; 65],
        }; ORDINARY_ISSUER_SLOTS];
        let mut next = 0;
        for entry in release.enabled_profiles() {
            let profile = &entry.hardware_profile;
            if !profile.platform_class.is_ordinary_app() {
                continue;
            }
            if next == ORDINARY_ISSUER_SLOTS || entry.hardware_profile_id == [0; 32] {
                return Err("ordinary governed issuer table bound differs".into());
            }
            profile
                .governance_credential_public_key
                .validate()
                .map_err(|e| e.to_string())?;
            slots[next] = OrdinaryIssuerProfileV1 {
                profile_id: entry.hardware_profile_id,
                issuer_sec1: *profile.governance_credential_public_key.as_sec1_bytes(),
            };
            next += 1;
        }
        // An OEM-only release has no selectable ordinary row. It cannot admit ordinary money.
        Ok(Self { slots })
    }
    #[cfg(any(
        test,
        feature = "kagemusha-real-proof-harness",
        feature = "kagemusha-production-prover"
    ))]
    pub(super) fn selected(&self, profile: DigestV1) -> Result<usize, String> {
        if profile == [0; 32] {
            return Err("ordinary issuer profile is absent".into());
        }
        self.slots
            .iter()
            .position(|row| row.profile_id == profile)
            .ok_or_else(|| "ordinary issuer profile is not independently released".into())
    }
}

#[derive(Clone, Debug)]
pub(super) struct OrdinaryIssuerConfigV1 {
    active: Selector,
    selected: [Column<Advice>; ORDINARY_ISSUER_SLOTS],
    profile: [Column<Advice>; 2],
    issuer: [Column<Advice>; 65],
}
impl OrdinaryIssuerConfigV1 {
    pub(super) fn configure<F: KagemushaPoseidonFieldV1>(
        meta: &mut ConstraintSystem<F>,
        table: &OrdinaryIssuerTableV1,
    ) -> Self {
        let active = meta.selector();
        let selected = core::array::from_fn(|_| meta.advice_column());
        let profile = core::array::from_fn(|_| meta.advice_column());
        let issuer = core::array::from_fn(|_| meta.advice_column());
        for column in profile.into_iter().chain(issuer) {
            meta.enable_equality(column);
        }
        let rows = table.slots;
        meta.create_gate(
            "Kagemusha independently governed ordinary issuer",
            move |meta| {
                let enabled = meta.query_selector(active);
                let bits = selected.map(|c| meta.query_advice(c, Rotation::cur()));
                let mut constraints = Vec::with_capacity(ORDINARY_ISSUER_SLOTS + 68);
                let mut sum = Expression::Constant(F::ZERO);
                for (bit, row) in bits.iter().zip(rows) {
                    constraints.push(
                        enabled.clone()
                            * bit.clone()
                            * (bit.clone() - Expression::Constant(F::ONE)),
                    );
                    if row.profile_id == [0; 32] {
                        constraints.push(enabled.clone() * bit.clone());
                    }
                    sum = sum + bit.clone();
                }
                constraints.push(enabled.clone() * (sum - Expression::Constant(F::ONE)));
                for limb in 0..2 {
                    let expected = bits.iter().zip(rows).fold(
                        Expression::Constant(F::ZERO),
                        |value, (bit, row)| {
                            value
                                + bit.clone()
                                    * Expression::Constant(digest_limbs::<F>(row.profile_id)[limb])
                        },
                    );
                    constraints.push(
                        enabled.clone()
                            * (meta.query_advice(profile[limb], Rotation::cur()) - expected),
                    );
                }
                for byte in 0..65 {
                    let expected = bits.iter().zip(rows).fold(
                        Expression::Constant(F::ZERO),
                        |value, (bit, row)| {
                            value
                                + bit.clone()
                                    * Expression::Constant(F::from(u64::from(
                                        row.issuer_sec1[byte],
                                    )))
                        },
                    );
                    constraints.push(
                        enabled.clone()
                            * (meta.query_advice(issuer[byte], Rotation::cur()) - expected),
                    );
                }
                constraints
            },
        );
        Self {
            active,
            selected,
            profile,
            issuer,
        }
    }
    pub(super) fn synthesize<F: KagemushaPoseidonFieldV1>(
        &self,
        layouter: &mut impl Layouter<F>,
        selected_index: usize,
        profile: [AssignedValue<F>; 2],
        issuer: [AssignedValue<F>; 65],
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
    ) -> Result<(), Error> {
        if selected_index >= ORDINARY_ISSUER_SLOTS {
            return Err(Error::Synthesis);
        }
        let physical = if witness_gen_only {
            None
        } else {
            Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
        };
        layouter.assign_region(
            || "ordinary issuer original key and profile",
            |mut region| {
                self.active.enable(&mut region, 0)?;
                for (index, column) in self.selected.iter().enumerate() {
                    region.assign_advice(
                        *column,
                        0,
                        Value::known(F::from(u64::from(index == selected_index))),
                    );
                }
                for (column, value) in self
                    .profile
                    .into_iter()
                    .zip(profile)
                    .chain(self.issuer.into_iter().zip(issuer))
                {
                    let assigned = region.assign_advice(column, 0, Value::known(*value.value()));
                    if let Some(physical) = &physical {
                        let original = physical
                            .assigned_advices
                            .resolve(&value.cell.ok_or(Error::Synthesis)?)
                            .ok_or(Error::Synthesis)?;
                        region.constrain_equal(assigned.cell(), original);
                    }
                }
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        SerdeFormat,
        circuit::V1,
        dev::MockProver,
        halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
        plonk::{Circuit, VerifyingKey, keygen_vk_custom},
        poly::{
            commitment::{Params as _, ParamsProver as _},
            ipa::commitment::ParamsIPA,
        },
    };
    use p256::ecdsa::SigningKey;

    #[derive(Clone, Debug, Default)]
    struct Params {
        base: BaseCircuitParams,
        table: OrdinaryIssuerTableV1,
    }
    #[derive(Clone)]
    struct Tiny<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        table: OrdinaryIssuerTableV1,
        index: usize,
        profile: [AssignedValue<F>; 2],
        key: [AssignedValue<F>; 65],
    }
    impl<F: KagemushaPoseidonFieldV1> Tiny<F> {
        fn new(index: usize, profile: DigestV1, key: [u8; 65]) -> Self {
            // Deliberately public synthetic table and native curve points; no release admission.
            let mut table = OrdinaryIssuerTableV1::default();
            for (slot, seed) in [7_u8, 8].into_iter().enumerate() {
                let k = SigningKey::from_bytes((&[seed; 32]).into()).unwrap();
                table.slots[slot] = OrdinaryIssuerProfileV1 {
                    profile_id: [slot as u8 + 3; 32],
                    issuer_sec1: k
                        .verifying_key()
                        .to_encoded_point(false)
                        .as_bytes()
                        .try_into()
                        .unwrap(),
                };
            }
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(7)
                .use_instance_columns(0);
            let profile = digest_limbs::<F>(profile).map(|v| builder.main(0).load_witness(v));
            let key = key.map(|v| builder.main(0).load_witness(F::from(u64::from(v))));
            builder.calculate_params(Some(9));
            Self {
                builder,
                table,
                index,
                profile,
                key,
            }
        }
        fn good() -> Self {
            let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
            Self::new(
                0,
                [3; 32],
                key.verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
                    .try_into()
                    .unwrap(),
            )
        }
    }
    #[derive(Clone, Debug)]
    struct Config<F: KagemushaPoseidonFieldV1> {
        base: BaseConfig<F>,
        issuer: OrdinaryIssuerConfigV1,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for Tiny<F> {
        type Config = Config<F>;
        type FloorPlanner = V1;
        type Params = Params;
        fn params(&self) -> Params {
            Params {
                base: self.builder.config_params.clone(),
                table: self.table.clone(),
            }
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                ..self.clone()
            }
        }
        fn configure_with_params(meta: &mut ConstraintSystem<F>, p: Params) -> Config<F> {
            let mut base = BaseConfig::configure(meta, p.base);
            base.set_usable_rows((1 << 7) - 9);
            Config {
                base,
                issuer: OrdinaryIssuerConfigV1::configure(meta, &p.table),
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Config<F> {
            unreachable!("requires explicit synthetic table")
        }
        fn synthesize(
            &self,
            config: Config<F>,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.builder
                .synthesize(config.base, layouter.namespace(|| "original cells"))?;
            config.issuer.synthesize(
                &mut layouter,
                self.index,
                self.profile,
                self.key,
                &self.builder.core().copy_manager,
                self.builder.witness_gen_only(),
            )
        }
    }
    fn check<F: KagemushaPoseidonFieldV1>() {
        assert!(
            MockProver::run(7, &Tiny::<F>::good(), vec![])
                .unwrap()
                .verify()
                .is_ok()
        );
        for mutation in 0..4 {
            let mut c = Tiny::<F>::good();
            match mutation {
                0 => c.index = 1, // another actual approved key cannot authorize the first profile
                1 => c.index = 2, // canonical empty padding must not be selected
                2 => c.profile[0].debug_prank(c.builder.main(0), F::ONE),
                _ => c.key[0].debug_prank(c.builder.main(0), F::ONE),
            }
            assert!(
                MockProver::run(7, &c, vec![]).unwrap().verify().is_err(),
                "mutation {mutation}"
            );
        }
    }
    #[test]
    fn profile_and_whole_issuer_point_are_selected_together_in_both_parities() {
        check::<Fp>();
        check::<Fq>();
    }
    #[test]
    fn native_reconstructed_issuer_table_changes_exact_vk_identity_in_both_parities() {
        macro_rules! check {
            ($curve:ty, $field:ty) => {{
                let p = ParamsIPA::<$curve>::new(7);
                let c = Tiny::<$field>::good();
                let key = keygen_vk_custom(&p, &c, true).unwrap();
                let bytes = key.to_bytes(SerdeFormat::Processed);
                let mut changed = c.params();
                changed.table.slots[0].issuer_sec1 = changed.table.slots[1].issuer_sec1;
                assert!(
                    VerifyingKey::<$curve>::read_checked::<_, Tiny<$field>>(
                        &mut std::io::Cursor::new(bytes),
                        SerdeFormat::Processed,
                        p.k(),
                        changed,
                    )
                    .is_err()
                );
            }};
        }
        check!(EqAffine, Fp);
        check!(EpAffine, Fq);
    }
}
