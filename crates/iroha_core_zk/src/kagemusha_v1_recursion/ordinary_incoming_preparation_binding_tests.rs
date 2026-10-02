//! Actual both-field SHA equations for every fresh IncomingPreparation operand, no money cap.
use super::*;
use crate::{
    kagemusha_v1_poseidon::{digest_limbs, from_u128},
    pasta_sha256::PastaSha256ConfigV1,
};
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::kagemusha::KagemushaOrdinaryIncomingPreparationV1;

const K: usize = 17;
const UNUSABLE: usize = 9;
#[derive(Clone)]
struct BindingCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for BindingCircuit<F> {
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
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1 << K) - UNUSABLE);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("fixed active Mint binding profile")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.0, layouter.namespace(|| "Mint binding Base"))?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1 << K) - UNUSABLE,
        )
    }
}

#[derive(Clone, Copy)]
enum Mutation {
    Operand(usize),
    FinancialIndex,
    JournalIndex,
    Purpose,
    Effect,
    Recovery,
}
fn specimen() -> KagemushaOrdinaryIncomingPreparationV1 {
    KagemushaOrdinaryIncomingPreparationV1 {
        version: 1,
        reservation_digest: [1; 32],
        operation_id: [2; 32],
        nonce: [3; 32],
        transition_statement_digest: [4; 32],
        predecessor_state_commitment: [5; 32],
        successor_state_commitment: [6; 32],
        financial_control_original_sha256: [7; 32],
        clock_context_digest: [8; 32],
        financial_index_before: (1_u128 << 100) + 17,
        financial_index_after: (1_u128 << 100) + 18,
        logical_journal_sequence_before: 21,
        logical_journal_sequence_after: 22,
    }
}
fn circuit<F: KagemushaPoseidonFieldV1>(
    operation: u64,
    mutation: Option<Mutation>,
) -> BindingCircuit<F> {
    let expected = specimen();
    let mut actual = expected;
    match mutation {
        Some(Mutation::Operand(i)) => {
            let field = match i {
                0 => &mut actual.reservation_digest,
                1 => &mut actual.operation_id,
                2 => &mut actual.nonce,
                3 => &mut actual.transition_statement_digest,
                4 => &mut actual.predecessor_state_commitment,
                5 => &mut actual.successor_state_commitment,
                6 => &mut actual.financial_control_original_sha256,
                _ => &mut actual.clock_context_digest,
            };
            field[0] ^= 1;
        }
        Some(Mutation::FinancialIndex) => {
            actual.financial_index_after = (actual.financial_index_after as u64) as u128
        }
        Some(Mutation::JournalIndex) => actual.logical_journal_sequence_after -= 1,
        _ => {}
    }
    let mut b = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(16)
        .use_instance_columns(0);
    let range = b.range_chip();
    let ctx = b.main(0);
    let digest =
        |ctx: &mut Context<F>, raw: [u8; 32]| digest_limbs::<F>(raw).map(|v| ctx.load_witness(v));
    let bytes = |ctx: &mut Context<F>, raw: [u8; 32]| {
        super::super::guard_bundle::assign_bytes(ctx, &range, &raw)
            .try_into()
            .unwrap()
    };
    let effect = if matches!(mutation, Some(Mutation::Effect)) {
        [11; 32]
    } else {
        expected.reservation_digest
    };
    let recovery = if matches!(mutation, Some(Mutation::Recovery)) {
        [12; 32]
    } else {
        expected.recovery_binding_digest().unwrap()
    };
    let cells = OrdinaryIncomingPreparationCellsV1 {
        operation: ctx.load_witness(F::from(operation)),
        reservation_digest: digest(ctx, actual.reservation_digest),
        operation_id: bytes(ctx, actual.operation_id),
        approval_nonce: bytes(ctx, actual.nonce),
        transition_statement_digest: bytes(ctx, actual.transition_statement_digest),
        predecessor_state: digest(ctx, actual.predecessor_state_commitment),
        successor_state: digest(ctx, actual.successor_state_commitment),
        financial_control_original_sha256: bytes(ctx, actual.financial_control_original_sha256),
        clock_context_digest: bytes(ctx, actual.clock_context_digest),
        financial_index_before: ctx.load_witness(from_u128::<F>(actual.financial_index_before)),
        financial_index_after: ctx.load_witness(from_u128::<F>(actual.financial_index_after)),
        journal_before: ctx.load_witness(F::from(actual.logical_journal_sequence_before)),
        journal_after: ctx.load_witness(F::from(actual.logical_journal_sequence_after)),
        approval_purpose: ctx.load_witness(F::from(
            if matches!(mutation, Some(Mutation::Purpose)) {
                1
            } else {
                2
            },
        )),
        transition_effect: digest(ctx, effect),
        guard_intent: digest(ctx, expected.binding_digest().unwrap()),
        guard_recovery: digest(ctx, recovery),
    };
    let mut jobs = PastaSha256JobsV1::default();
    constrain_ordinary_incoming_preparation_v1(ctx, &range, &mut jobs, cells).unwrap();
    assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 2);
    b.calculate_params(Some(UNUSABLE));
    BindingCircuit { builder: b, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>(operation: u64, mutation: Option<Mutation>) -> bool {
    MockProver::run(K as u32, &circuit::<F>(operation, mutation), vec![])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn ordinary_incoming_fresh_transcript_sha_and_recovery_reject_all_source_and_edge_substitutions() {
    for operation in [1, 3] {
        assert!(check::<Fp>(operation, None));
        assert!(check::<Fq>(operation, None));
    }
    for mutation in (0..8).map(Mutation::Operand).chain([
        Mutation::FinancialIndex,
        Mutation::JournalIndex,
        Mutation::Purpose,
        Mutation::Effect,
        Mutation::Recovery,
    ]) {
        assert!(!check::<Fp>(1, Some(mutation)));
        assert!(!check::<Fq>(3, Some(mutation)));
    }
}
#[test]
fn ordinary_incoming_transcript_sha_graph_is_fixed_for_inactive_operations() {
    for operation in [0, 2, 4, 5] {
        // This test establishes fixed SHA topology/gating only. Inactive fields create no cap.
        assert!(check::<Fp>(operation, Some(Mutation::Operand(6))));
        assert!(check::<Fq>(operation, Some(Mutation::Operand(7))));
    }
}
