//! Actual inventory proofs and adversarial transcript/semantic bindings in both Pasta fields.
//!
//! These use the complete inventory Circuit around the existing two-source true-equation
//! fixture. They qualify this proof-reader boundary, not the missing complete Claim graph.
//! Native equation evaluation below is an independent test oracle, never an admission path.

use super::super::proof::{
    KagemushaClaimInventoryVerifierPinsV1, verify_kagemusha_claim_inventory_proof_v1,
};
use super::*;
use crate::zk::{
    kagemusha_v1_recursion::{
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        deferred_parent::native_parent_protocol_digest_v1,
        generation::{KagemushaRawHalo2IpaProofV1, augment_halo2_ipa_proof_columns_v1},
    },
    pasta_cycle_loader::DeferredEquationWitness,
};
use halo2_proofs::{
    halo2curves::CurveExt as _,
    plonk::{create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver as _,
        ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPAHybrid},
    },
};
use snark_verifier::{
    pcs::{
        AccumulationDecider as _,
        ipa::{Bgh19, IpaAs, IpaDecidingKey},
    },
    system::halo2::{
        Config as ProtocolConfig, compile,
        transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    },
    util::arithmetic::{Domain, root_of_unity},
};

const VERIFIER_K: usize = KAGEMUSHA_RECURSION_IPA_K_V1 as usize;
const VERIFIER_ROWS: usize = (1 << VERIFIER_K) - MINIMUM_UNUSABLE_ROWS;

#[derive(Clone, Debug)]
struct InventoryVerifierConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    native: PastaNativePoseidonConfigV1,
}

#[derive(Clone)]
struct InventoryVerifierCircuit<F: KagemushaPoseidonFieldV1> {
    base: BaseCircuitBuilder<F>,
    jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for InventoryVerifierCircuit<F> {
    type Config = InventoryVerifierConfig<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.base.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            base: self.base.deep_clone().unknown(true),
            jobs: self.jobs.clone().unknown(),
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("inventory verifier uses parameterized Base configuration")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(VERIFIER_ROWS);
        InventoryVerifierConfig {
            base,
            native: PastaNativePoseidonConfigV1::configure::<F>(meta, 2),
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        self.base.reset_synthesis_state();
        self.base.synthesize(
            config.base,
            layouter.namespace(|| "inventory verifier Base"),
        )?;
        self.jobs.synthesize(
            &config.native,
            &mut layouter,
            &self.base.core().copy_manager,
            self.base.witness_gen_only(),
            VERIFIER_ROWS,
        )
    }
}

struct CapturedInventory<C: CurveAffineExt>
where
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    circuit: InventoryVerifierCircuit<C::ScalarExt>,
    public: Vec<C::ScalarExt>,
    audit: DeferredEquationWitness<C>,
    accumulator: IpaAccumulator<C, NativeLoader>,
    proof_read_carriers: [C; 2],
}

fn capture_inventory<C>(
    pins: &KagemushaClaimInventoryVerifierPinsV1<'_, C>,
    protocol: &PlonkProtocol<C>,
    semantic: &[C::ScalarExt],
    bytes: &[u8],
) -> Result<CapturedInventory<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut base = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(VERIFIER_K)
        .use_lookup_bits(CLAIM_RLC_RADIX_BITS)
        .use_instance_columns(1);
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let assigned = semantic
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    let mut jobs = PastaNativePoseidonJobsV1::new(2, VERIFIER_ROWS)?;
    let graph = verify_kagemusha_claim_inventory_proof_v1(
        &loader, pins, protocol, &assigned, bytes, &mut jobs,
    )?;
    assert_eq!(graph.semantic.len(), INVENTORY_SEMANTIC_COUNT);
    for (original, retained) in assigned.iter().zip(&graph.semantic) {
        assert_eq!(original.assigned().cell, retained.assigned().cell);
    }
    let chip = loader.ecc_chip();
    let audit = chip.witness();
    assert_eq!(graph.equations, 0..audit.equations.len());
    assert!(!graph.equations.is_empty());
    let point =
        |point: &crate::zk::kagemusha_v1_recursion::deferred_parent::DeferredEcPoint<'_, C>| {
            let index = chip.assigned_point_source_index(&point.assigned()).unwrap();
            audit.sources[index]
        };
    let proof_read_carriers = std::array::from_fn(|index| point(&graph.carrier_commitments[index]));
    let accumulator = IpaAccumulator {
        u: point(&graph.accumulator.u),
        xi: graph
            .accumulator
            .xi
            .iter()
            .map(|value| *value.assigned().value())
            .collect(),
    };
    drop(chip);
    let mut output = graph
        .semantic
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    output.push(graph.transcript_binding);
    let public = output.iter().map(|value| *value.value()).collect();
    *base.pool(0) = loader.take_ctx();
    base.assigned_instances = vec![output];
    crate::zk::kagemusha_v1_recursion::base_packing::finalize_base_params_v1(
        &mut base,
        MINIMUM_UNUSABLE_ROWS,
    )?;
    Ok(CapturedInventory {
        circuit: InventoryVerifierCircuit { base, jobs },
        public,
        audit,
        accumulator,
        proof_read_carriers,
    })
}

fn equations_hold<C: CurveAffineExt>(audit: &DeferredEquationWitness<C>) -> bool {
    !audit.equations.is_empty()
        && audit.equations.iter().all(|equation| {
            let (scalars, points): (Vec<_>, Vec<_>) = equation
                .iter()
                .map(|(source, scalar)| (*scalar, audit.sources[*source]))
                .unzip();
            bool::from(
                best_multiexp::<C>(&scalars, &points)
                    .to_affine()
                    .is_identity(),
            )
        })
}

fn check_actual_inventory_proof<C>(
    parity: KagemushaPastaParityV1,
    bind_commitments: impl Fn([C; 2]) -> ClaimCarrierCommitmentsV1,
) where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
{
    let params = ParamsIPA::<C>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
    let preliminary = full_inventory_fixture::<C>(parity, 17, false, None);
    let commitments = std::array::from_fn(|index| {
        (best_multiexp::<C>(
            &preliminary.instances[index + 1],
            &params.get_g_lagrange()[..KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
        ) + params.get_blind_base().to_curve())
        .to_affine()
    });
    let input = full_inventory_fixture::<C>(parity, 17, false, Some(bind_commitments(commitments)));
    assert_eq!(
        &input.instances[1..],
        &preliminary.instances[1..],
        "source carriers must be independent of their subsequently bound proof commitments"
    );
    assert_eq!(
        input.instances.iter().map(Vec::len).collect::<Vec<_>>(),
        [113, 4090, 4090]
    );
    drop(preliminary);
    MockProver::run(
        KAGEMUSHA_RECURSION_IPA_K_V1,
        &input.circuit,
        input.instances.clone(),
    )
    .expect("actual inventory with exact computed commitments")
    .assert_satisfied();
    input.circuit.builder.reset_synthesis_state();
    let vk = keygen_vk(&params, &input.circuit).expect("actual inventory VK");
    input.circuit.builder.reset_synthesis_state();
    let pk = keygen_pk(&params, vk, &input.circuit).expect("actual inventory PK");
    input.circuit.builder.reset_synthesis_state();
    let mut protocol = compile(
        &params,
        pk.get_vk(),
        ProtocolConfig::ipa().with_num_instance(vec![113, 4090, 4090]),
    );
    protocol
        .instance_committing_key
        .as_mut()
        .unwrap()
        .bases
        .truncate(INVENTORY_SEMANTIC_COUNT);
    let columns = input
        .instances
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let mut transcript = PoseidonTranscript::<
        C,
        NativeLoader,
        _,
        KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
        KAGEMUSHA_IPA_POSEIDON_RATE_V1,
        KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
        KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    >::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(Vec::new());
    create_proof::<
        IPACommitmentScheme<C>,
        ProverIPAHybrid<'_, C, 0b110>,
        ChallengeScalar<C>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[input.circuit],
        &[&columns],
        rand_core_06::OsRng,
        &mut transcript,
    )
    .expect("produce a genuine three-column inventory proof");
    let raw = transcript.finalize();
    let bytes = augment_halo2_ipa_proof_columns_v1::<C, 0b110>(
        &params,
        pk.get_vk(),
        KagemushaRawHalo2IpaProofV1::new(raw.clone()),
        &columns,
    )
    .expect("native verification of real proof before deriving its folded generator");
    for (index, point) in commitments.iter().enumerate() {
        assert_eq!(
            &bytes[index * 32..(index + 1) * 32],
            point.to_bytes().as_ref()
        );
    }
    let hash_to_curve = C::CurveExt::hash_to_curve("Halo2-Parameters");
    let key = IpaSuccinctVerifyingKey::new(
        Domain::new(VERIFIER_K, root_of_unity(VERIFIER_K)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let pins = KagemushaClaimInventoryVerifierPinsV1 {
        parity,
        succinct_vk: &key,
        structure_digest: kagemusha_protocol_structure_digest_v1(&protocol, parity).unwrap(),
        protocol_digest: native_parent_protocol_digest_v1(&protocol, parity).unwrap(),
    };
    let captured = capture_inventory(&pins, &protocol, &input.instances[0], &bytes).unwrap();
    assert_eq!(captured.proof_read_carriers, commitments);
    assert!(
        equations_hold(&captured.audit),
        "every emitted curve equation is true"
    );
    let deciding_key = IpaDecidingKey::new(key.clone(), params.get_g().to_vec());
    IpaAs::<C, Bgh19>::decide(&deciding_key, captured.accumulator.clone())
        .expect("actual inventory IPA opening has the exact SRS expansion");
    MockProver::run(
        VERIFIER_K as u32,
        &captured.circuit,
        vec![captured.public.clone()],
    )
    .expect("inventory scalar verifier and mandatory native identity region")
    .assert_satisfied();
    let offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
    };
    for mutation in 0..4 {
        let mut semantic = input.instances[0].clone();
        let mut changed_bytes = bytes.clone();
        let mut changed_protocol = protocol.clone();
        match mutation {
            0 => semantic[offset] += C::ScalarExt::ONE,
            1 => semantic[offset + 3] += C::ScalarExt::ONE,
            2 => {
                let (first, rest) = changed_bytes.split_at_mut(32);
                first.swap_with_slice(&mut rest[..32]);
            }
            _ => {
                changed_protocol.preprocessed[0] =
                    (C::generator() * C::ScalarExt::from(91)).to_affine()
            }
        }
        let changed = capture_inventory(&pins, &changed_protocol, &semantic, &changed_bytes)
            .expect("canonical mutated proof/statement still reaches constrained checks");
        assert!(
            MockProver::run(VERIFIER_K as u32, &changed.circuit, vec![changed.public])
                .expect("mutated inventory commitment or VK identity")
                .verify()
                .is_err(),
            "mutation {mutation} escaped the proof-read/identity binding"
        );
    }
    for index in [0, INVENTORY_EQ_SOURCE_COUNT, INVENTORY_EP_SOURCE_COUNT] {
        let mut semantic = input.instances[0].clone();
        semantic[index] += C::ScalarExt::ONE;
        let changed_columns = [&semantic[..], columns[1], columns[2]];
        assert!(
            augment_halo2_ipa_proof_columns_v1::<C, 0b110>(
                &params,
                pk.get_vk(),
                KagemushaRawHalo2IpaProofV1::new(raw.clone()),
                &changed_columns,
            )
            .is_err(),
            "native verifier rejects changed inventory semantic cell {index}"
        );
        let changed = capture_inventory(&pins, &protocol, &semantic, &bytes).unwrap();
        assert!(
            !equations_hold(&changed.audit)
                || IpaAs::<C, Bgh19>::decide(&deciding_key, changed.accumulator).is_err(),
            "the explicitly deferred result must preserve the failed proof obligation"
        );
    }
    let mut changed_shape = protocol.clone();
    changed_shape.num_instance[1] -= 1;
    assert!(capture_inventory(&pins, &changed_shape, &input.instances[0], &bytes).is_err());
    let wrong_parity = KagemushaClaimInventoryVerifierPinsV1 {
        parity: match parity {
            KagemushaPastaParityV1::Eq => KagemushaPastaParityV1::Ep,
            KagemushaPastaParityV1::Ep => KagemushaPastaParityV1::Eq,
        },
        succinct_vk: &key,
        structure_digest: pins.structure_digest,
        protocol_digest: pins.protocol_digest,
    };
    assert!(capture_inventory(&wrong_parity, &protocol, &input.instances[0], &bytes).is_err());
    assert!(capture_inventory(&pins, &protocol, &input.instances[0][..112], &bytes).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(capture_inventory(&pins, &protocol, &input.instances[0], &trailing).is_err());
    assert!(
        capture_inventory(
            &pins,
            &protocol,
            &input.instances[0],
            &bytes[..bytes.len() - 1]
        )
        .is_err()
    );
}

#[test]
fn actual_eq_inventory_proof_binds_carriers_semantics_and_independent_vk() {
    check_actual_inventory_proof::<EqAffine>(KagemushaPastaParityV1::Eq, |[eq, ep]| {
        ClaimCarrierCommitmentsV1 {
            eq_proof_eq_carrier: eq,
            eq_proof_ep_carrier: ep,
            ep_proof_eq_carrier: EpAffine::generator(),
            ep_proof_ep_carrier: (EpAffine::generator() * Fq::from(2)).to_affine(),
        }
    });
}

#[test]
fn actual_ep_inventory_proof_binds_carriers_semantics_and_independent_vk() {
    check_actual_inventory_proof::<EpAffine>(KagemushaPastaParityV1::Ep, |[eq, ep]| {
        ClaimCarrierCommitmentsV1 {
            eq_proof_eq_carrier: EqAffine::generator(),
            eq_proof_ep_carrier: (EqAffine::generator() * Fp::from(2)).to_affine(),
            ep_proof_eq_carrier: eq,
            ep_proof_ep_carrier: ep,
        }
    });
}
