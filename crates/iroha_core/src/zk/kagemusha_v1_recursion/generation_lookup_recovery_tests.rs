//! Production Poseidon recovery and compact-key reloads must preserve varied lookup proofs.

use super::*;
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Instance, Selector, TableColumn},
    poly::{Rotation, ipa::strategy::SingleStrategy},
};

#[derive(Clone)]
struct LookupRecoveryCircuit<F: ff::PrimeField, const HYBRID: bool> {
    offset: Value<F>,
}

#[derive(Clone)]
struct LookupRecoveryConfig {
    advice: Column<Advice>,
    copied: Column<Advice>,
    instance: Vec<Column<Instance>>,
    selector: Selector,
    table: TableColumn,
}

impl<F: ff::PrimeField, const HYBRID: bool> halo2_proofs::plonk::Circuit<F>
    for LookupRecoveryCircuit<F, HYBRID>
{
    type Config = LookupRecoveryConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            offset: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let copied = meta.advice_column();
        let instance = (0..if HYBRID { 2 } else { 1 })
            .map(|_| meta.instance_column())
            .collect::<Vec<_>>();
        let selector = meta.complex_selector();
        let table = meta.lookup_table_column();
        meta.enable_equality(advice);
        meta.enable_equality(copied);
        for column in &instance {
            meta.enable_equality(*column);
        }
        meta.set_minimum_degree(7);
        meta.create_gate("recovery lookup copy", |meta| {
            vec![
                meta.query_selector(selector)
                    * (meta.query_advice(advice, Rotation::cur())
                        - meta.query_advice(copied, Rotation::cur())),
            ]
        });
        meta.lookup(
            "recovery lookup includes repeated and distinct values",
            |meta| {
                vec![(
                    meta.query_selector(selector) * meta.query_advice(advice, Rotation::cur()),
                    table,
                )]
            },
        );
        LookupRecoveryConfig {
            advice,
            copied,
            instance,
            selector,
            table,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_table(
            || "recovery range",
            |mut table| {
                for row in 0..16 {
                    table.assign_cell(
                        || "range entry",
                        config.table,
                        row,
                        || Value::known(F::from(row as u64)),
                    )?;
                }
                Ok(())
            },
        )?;
        let cells = layouter.assign_region(
            || "varied recovery witness",
            |mut region| {
                let mut public_cells = Vec::new();
                for (row, addend) in [0, 7, 2, 7, 1, 9, 4, 2, 11, 5, 0, 11, 3, 4, 9, 1]
                    .into_iter()
                    .enumerate()
                {
                    config.selector.enable(&mut region, row)?;
                    let value = self.offset.map(|offset| offset + F::from(addend));
                    let cell = region.assign_advice(config.advice, row, value);
                    if row < config.instance.len() {
                        public_cells.push(cell.cell());
                    }
                    cell.copy_advice(&mut region, config.copied, row);
                }
                Ok(public_cells)
            },
        )?;
        assert_eq!(cells.len(), config.instance.len());
        for (cell, column) in cells.into_iter().zip(config.instance) {
            layouter.constrain_instance(cell, column, 0);
        }
        Ok(())
    }
}

fn recover_compact_lookup_key<C, const HYBRID: bool>(
    key: &ProvingKey<C>,
    parity: KagemushaPastaParityV1,
    role: KagemushaArtifactRoleV1,
) -> ProvingKey<C>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    let mut bytes = compact_proving_key_buffer_v1(
        parity,
        "lookup recovery proving key",
        KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1,
        key,
    )
    .expect("production compact buffer cap");
    key.clone()
        .write_compact_v1_consuming(&mut bytes)
        .expect("consuming compact key writer");
    let binding = binding(role, &bytes);
    let mut cursor = Cursor::new(bytes.as_slice());
    let recovered = read_canonical_proving_key_v1::<C, LookupRecoveryCircuit<C::Scalar, HYBRID>>(
        &mut cursor,
        binding,
        parity,
        6,
        (),
    )
    .expect("shape-checked, digest-checked canonical compact recovery");
    ensure_cursor_consumed(parity, "lookup recovery proving key", &cursor, bytes.len())
        .expect("no trailing key bytes");
    assert_eq!(
        recovered.to_bytes(SerdeFormat::Processed),
        key.to_bytes(SerdeFormat::Processed)
    );
    ensure_embedded_vk(
        parity,
        &recovered,
        &key.get_vk().to_bytes(SerdeFormat::Processed),
    )
    .expect("exact recovered verifier key");
    recovered
}

fn verify_recovered_lookup_proof<C, const MASK: u64>(
    parameters: &ParamsIPA<C>,
    key: &VerifyingKey<C>,
    proof: &[u8],
    columns: &[Vec<C::Scalar>],
) -> Result<(), String>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    type Transcript<C, S> = PoseidonTranscript<
        C,
        NativeLoader,
        S,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >;
    let raw_len = proof
        .len()
        .checked_sub(32)
        .ok_or("missing folded generator")?;
    let raw = &proof[..raw_len];
    let columns = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let all_instances = [columns.as_slice()];
    let mut cursor = Cursor::new(raw);
    let mut transcript =
        Transcript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&mut cursor);
    verify_proof::<
        IPACommitmentScheme<C>, VerifierIPA<'_, C, true, MASK>, ChallengeScalar<C>, _, _,
    >(
        parameters, key, SingleStrategy::<C, true, MASK>::new(parameters),
        &all_instances, &mut transcript,
    ).map_err(|error| format!("final IPA equation: {error}"))?;
    drop(transcript);
    if cursor.position() as usize != raw_len {
        return Err("trailing raw proof bytes".to_owned());
    }
    let canonical = augment_halo2_ipa_proof_columns_v1::<C, MASK>(
        parameters,
        key,
        KagemushaRawHalo2IpaProofV1::new(raw.to_vec()),
        &columns,
    )?;
    if canonical != proof {
        return Err("noncanonical folded-generator suffix".to_owned());
    }
    Ok(())
}

fn verify_lookup_cases<C, const MASK: u64>(
    parameters: &ParamsIPA<C>,
    key: &VerifyingKey<C>,
    proofs: &[Vec<u8>],
    columns: &[Vec<C::Scalar>],
) where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    assert!(!proofs.is_empty());
    for proof in proofs {
        verify_recovered_lookup_proof::<C, MASK>(parameters, key, proof, columns)
            .expect("full Poseidon IPA proof and canonical folded generator");
    }
    // Each public column is independently bound, including the proof-supplied carrier.
    for index in 0..columns.len() {
        let mut wrong = columns.to_vec();
        wrong[index][0] += C::Scalar::ONE;
        assert!(
            verify_recovered_lookup_proof::<C, MASK>(parameters, key, &proofs[0], &wrong).is_err()
        );
    }
    assert!(proofs[0].len() > 33);
    for index in [proofs[0].len() - 33, proofs[0].len() - 1] {
        let mut corrupted = proofs[0].clone();
        corrupted[index] ^= 1;
        assert!(
            verify_recovered_lookup_proof::<C, MASK>(parameters, key, &corrupted, columns).is_err()
        );
    }
}

macro_rules! ordinary_recovery_test {
    ($name:ident, $curve:ty, $field:ty, $parity:ident, $role:ident, $create:ident) => {
        #[test]
        fn $name() {
            let parameters = ParamsIPA::<$curve>::new(6);
            let value = <$field>::from(3);
            let circuit = LookupRecoveryCircuit::<$field, false> {
                offset: Value::known(value),
            };
            let vk = keygen_vk(&parameters, &circuit).expect("lookup ordinary VK");
            let key = keygen_pk(&parameters, vk, &circuit).expect("lookup ordinary PK");
            assert_eq!(key.get_vk().cs().lookups().len(), 1);
            assert_eq!(key.get_vk().get_domain().extended_len() / 64, 8);
            let recovered = recover_compact_lookup_key::<$curve, false>(
                &key,
                KagemushaPastaParityV1::$parity,
                KagemushaArtifactRoleV1::$role,
            );
            // Deliberately public fixture entropy; never a production operation seed.
            let seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD1; 32]).expect("public fixture seed");
            let recovered_seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD1; 32]).expect("same fixture seed");
            let other_seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD2; 32]).expect("different fixture seed");
            let make = |key, seed: &KagemushaRecoverySeedV1, phase| {
                $create(&parameters, key, circuit.clone(), &[value], phase, seed)
                    .expect("actual production ordinary recovery helper")
            };
            let original = make(&key, &seed, KagemushaProofRecoveryPhaseV1::CommitWrapper);
            let restored = make(
                &recovered,
                &recovered_seed,
                KagemushaProofRecoveryPhaseV1::CommitWrapper,
            );
            assert_eq!(
                original, restored,
                "compact reload and seed reconstruction preserve exact bytes"
            );
            let different_seed = make(
                &recovered,
                &other_seed,
                KagemushaProofRecoveryPhaseV1::CommitWrapper,
            );
            let different_phase = make(
                &recovered,
                &seed,
                KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
            );
            assert_ne!(original, different_seed);
            assert_ne!(original, different_phase);
            verify_lookup_cases::<$curve, 0>(
                &parameters,
                key.get_vk(),
                &[original, restored, different_seed, different_phase],
                &[vec![value]],
            );
        }
    };
}

macro_rules! hybrid_recovery_test {
    ($name:ident, $curve:ty, $field:ty, $parity:ident, $role:ident, $create:ident, $consume:ident) => {
        #[test]
        fn $name() {
            let parameters = ParamsIPA::<$curve>::new(6);
            let value = <$field>::from(3);
            let circuit = LookupRecoveryCircuit::<$field, true> {
                offset: Value::known(value),
            };
            let vk = keygen_vk(&parameters, &circuit).expect("lookup hybrid VK");
            let key = keygen_pk(&parameters, vk, &circuit).expect("lookup hybrid PK");
            assert_eq!(key.get_vk().cs().lookups().len(), 1);
            assert_eq!(key.get_vk().cs().num_instance_columns(), 2);
            assert_eq!(key.get_vk().get_domain().extended_len() / 64, 8);
            let recovered = recover_compact_lookup_key::<$curve, true>(
                &key,
                KagemushaPastaParityV1::$parity,
                KagemushaArtifactRoleV1::$role,
            );
            let seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD1; 32]).expect("public fixture seed");
            let recovered_seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD1; 32]).expect("same fixture seed");
            let other_seed =
                KagemushaRecoverySeedV1::from_unsealed([0xD2; 32]).expect("different fixture seed");
            let columns = [vec![value], vec![value + <$field>::from(7)]];
            let make = |key, seed: &KagemushaRecoverySeedV1, phase| {
                $create(&parameters, key, circuit.clone(), &columns, phase, seed)
                    .expect("actual production hybrid recovery helper")
            };
            let original = make(&key, &seed, KagemushaProofRecoveryPhaseV1::StateCarrier);
            let restored = make(
                &recovered,
                &recovered_seed,
                KagemushaProofRecoveryPhaseV1::StateCarrier,
            );
            assert_eq!(
                original, restored,
                "hybrid compact reload preserves exact recovery bytes"
            );
            let consuming = $consume::<_, KAGEMUSHA_ONE_CARRIER_INSTANCE_MASK_V1>(
                &parameters,
                recovered,
                circuit.clone(),
                &columns,
                KagemushaProofRecoveryPhaseV1::StateCarrier,
                &recovered_seed,
            )
            .expect("actual consuming hybrid recovery helper");
            assert_eq!(
                original, consuming,
                "consuming compact key preserves seeded hybrid transcript"
            );
            let different_seed = make(
                &key,
                &other_seed,
                KagemushaProofRecoveryPhaseV1::StateCarrier,
            );
            let different_phase = make(&key, &seed, KagemushaProofRecoveryPhaseV1::StateTransport);
            assert_ne!(original, different_seed);
            assert_ne!(original, different_phase);
            verify_lookup_cases::<$curve, KAGEMUSHA_ONE_CARRIER_INSTANCE_MASK_V1>(
                &parameters,
                key.get_vk(),
                &[
                    original,
                    restored,
                    consuming,
                    different_seed,
                    different_phase,
                ],
                &columns,
            );
        }
    };
}

ordinary_recovery_test!(
    real_lookup_eq_ordinary_recovery_survives_compact_reload,
    EqAffine,
    Fp,
    Eq,
    StatePkEq,
    create_eq_proof_with_key_v1
);
ordinary_recovery_test!(
    real_lookup_ep_ordinary_recovery_survives_compact_reload,
    EpAffine,
    Fq,
    Ep,
    StatePkEp,
    create_ep_proof_with_key_v1
);
hybrid_recovery_test!(
    real_lookup_eq_hybrid_recovery_survives_compact_reload,
    EqAffine,
    Fp,
    Eq,
    StatePkEq,
    create_eq_hybrid_proof_with_key_v1,
    create_eq_hybrid_proof_consuming_key_with_mask_v1
);
hybrid_recovery_test!(
    real_lookup_ep_hybrid_recovery_survives_compact_reload,
    EpAffine,
    Fq,
    Ep,
    StatePkEp,
    create_ep_hybrid_proof_with_key_v1,
    create_ep_hybrid_proof_consuming_key_with_mask_v1
);
