//! Both-field exact incoming W1 body SHA/copy equations. Fixtures are data, never owners or approvals.
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
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryIncomingTerminalBodyV1,
    KagemushaOrdinaryIncomingTerminalIntentV1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
};

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

fn specimen() -> KagemushaOrdinaryIncomingTerminalBodyV1 {
    KagemushaOrdinaryIncomingTerminalBodyV1 {
        intent: KagemushaOrdinaryIncomingTerminalIntentV1 {
            version: 1,
            operation: 1,
            native_operation_id: [1; 32],
            native_nonce: [2; 32],
            preparation_digest: [3; 32],
            reservation_digest: [4; 32],
            finalized_source_original_sha256: [5; 32],
            source_proof_original_sha256: [6; 32],
            state_original_sha256: [7; 32],
            transition_statement_original_sha256: [8; 32],
            preparation_guard_original_sha256: [9; 32],
            candidate_original_sha256: [7; 32],
            purpose2_approval_original_sha256: [11; 32],
            financial_control_original_sha256: [12; 32],
            predecessor_descriptor_prefix_digest: [13; 32],
            reserve_request_original_sha256: [14; 32],
            reserve_receipt_original_sha256: [15; 32],
            clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [16; 32],
                signed_observations_original_digest: [17; 32],
                lower_at_ms: 2000,
                upper_at_ms: 2001,
            },
            financial_index_before: (1_u128 << 110) + 9,
            financial_index_after: (1_u128 << 110) + 10,
            financial_sequence_before: (1_u128 << 100) + 5,
            financial_sequence_after: (1_u128 << 100) + 6,
            logical_journal_sequence_before: 7,
            logical_journal_sequence_after: 8,
            issued_at_ms: 2000,
            expires_at_ms: 3000,
        },
    }
}
fn circuit<F: KagemushaPoseidonFieldV1>(
    active: bool,
    mutation: Option<usize>,
) -> BindingCircuit<F> {
    let body = specimen();
    let i = body.intent;
    let mut raw = if active {
        i.binding_transcript().unwrap()
    } else {
        [0; 661]
    };
    if let Some(n) = mutation {
        raw[n] ^= 1
    }
    let mut b = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(16)
        .use_instance_columns(0);
    let range = b.range_chip();
    let ctx = b.main(0);
    let zero = ctx.load_constant(F::ZERO);
    let empty = constant_bytes(&[0; 32]).try_into().unwrap();
    let mut g = KagemushaAssignedGuardBundleV1 {
        guard_digest: empty,
        credential_digests: [empty; 2],
        credential_issuance_digests: [empty; 2],
        credential_app_policy_binding_digests: [empty; 2],
        credential_device_public_keys: [Vec::new(), Vec::new()],
        credential_financial_authority_commitments: [empty; 2],
        protocol_version: zero,
        predecessor_suite_id: [zero; 2],
        predecessor_vk_digest: [zero; 2],
        successor_suite_id: [zero; 2],
        successor_vk_digest: [zero; 2],
        operation: zero,
        amount: zero,
        peer_credit_id: [zero; 2],
        recipient_encryption_key_binding: [zero; 2],
        mint_finality_proof_binding_digest: [zero; 2],
        predecessor_release_id: [zero; 2],
        release_id: [zero; 2],
        network_id: [zero; 2],
        asset_id: [zero; 2],
        asset_incarnation: [zero; 2],
        asset_scale: zero,
        liability_pool_id: [zero; 2],
        hardware_profile_id: [zero; 2],
        policy_epoch: zero,
        lane_id: [zero; 2],
        predecessor_state: [zero; 2],
        successor_state: [zero; 2],
        predecessor_nonce: [zero; 2],
        successor_nonce: [zero; 2],
        predecessor_sequence: zero,
        successor_sequence: zero,
        predecessor_generation: zero,
        successor_generation: zero,
        predecessor_epoch: [zero; 2],
        successor_epoch: [zero; 2],
        predecessor_key: [zero; 2],
        successor_key: [zero; 2],
        predecessor_policy: [zero; 2],
        successor_policy: [zero; 2],
        journal_before: zero,
        journal_after: zero,
        lifecycle_binding_digest: [zero; 2],
        prepared_transition_binding_digest: [zero; 2],
        terminal_commit_binding_digest: [zero; 2],
        sender_one_time_authorization_digest: [zero; 2],
        receive_credit_binding_digest: [zero; 2],
        transition_intent: [zero; 2],
        transition_effect: [zero; 2],
        recovery_record: [zero; 2],
        durable_inbox_effect: [zero; 2],
        durable_outbox_effect: [zero; 2],
    };
    let digest =
        |ctx: &mut Context<F>, raw: [u8; 32]| digest_limbs::<F>(raw).map(|v| ctx.load_witness(v));
    g.operation = ctx.load_witness(F::from(if active { 1 } else { 0 }));
    if active {
        g.predecessor_sequence = ctx.load_witness(from_u128::<F>(i.financial_sequence_before));
        g.successor_sequence = ctx.load_witness(from_u128::<F>(i.financial_sequence_after));
        g.journal_before = ctx.load_witness(F::from(i.logical_journal_sequence_before));
        g.journal_after = ctx.load_witness(F::from(i.logical_journal_sequence_after));
        g.transition_effect = digest(ctx, i.reservation_digest);
        g.transition_intent = digest(ctx, body.binding_digest().unwrap());
        g.recovery_record = digest(ctx, i.binding_digest().unwrap());
        g.terminal_commit_binding_digest = digest(
            ctx,
            kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
                body.binding_digest().unwrap(),
                i.candidate_original_sha256,
                i.state_original_sha256,
                i.reservation_digest,
            )
            .unwrap(),
        );
    }
    let mut w = [0_u8; A::TOTAL_BYTES];
    let mut s = [0_u8; S::TOTAL_BYTES];
    if active {
        w[A::PURPOSE].copy_from_slice(&[1]);
        w[A::OPERATION_ID].copy_from_slice(&i.native_operation_id);
        w[A::NONCE].copy_from_slice(&i.native_nonce);
        w[A::ISSUED_AT_MS].copy_from_slice(&i.issued_at_ms.to_le_bytes());
        w[A::EXPIRES_AT_MS].copy_from_slice(&i.expires_at_ms.to_le_bytes());
        s[S::CANDIDATE_ENVELOPE_DIGEST].copy_from_slice(&i.candidate_original_sha256);
        s[S::TERMINAL_BODY_COMMITMENT].copy_from_slice(&body.binding_digest().unwrap());
        s[S::SECURE_INDEX_BEFORE].copy_from_slice(&i.financial_index_before.to_le_bytes());
        s[S::SECURE_INDEX_AFTER].copy_from_slice(&i.financial_index_after.to_le_bytes());
    }
    let wrapper = core::array::from_fn(|n| ctx.load_witness(F::from(u64::from(w[n]))));
    let subject = core::array::from_fn(|n| ctx.load_witness(F::from(u64::from(s[n]))));
    let assigned = assign_bytes(ctx, &range, &raw);
    let mut jobs = PastaSha256JobsV1::default();
    constrain_assigned(ctx, &range, &mut jobs, &g, &subject, &wrapper, &assigned).unwrap();
    assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 3);
    b.calculate_params(Some(UNUSABLE));
    BindingCircuit { builder: b, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>(active: bool, mutation: Option<usize>) -> bool {
    MockProver::run(K as u32, &circuit::<F>(active, mutation), vec![])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn incoming_terminal_full_original_and_128bit_edge_mutations_fail_both_fields() {
    assert!(check::<Fp>(true, None));
    assert!(check::<Fq>(true, None));
    // Every digest, both signed clock identities, full index/sequence/journal and window.
    for n in (0..15).map(|i| 3 + i * 32).chain([
        0,
        2,
        483,
        485,
        517,
        549,
        557,
        565 + 13,
        581 + 13,
        597 + 12,
        613 + 12,
        629,
        637,
        645,
        653,
    ]) {
        assert!(
            !check::<Fp>(true, Some(n)),
            "Fp accepted modified original byte {n}"
        );
        assert!(
            !check::<Fq>(true, Some(n)),
            "Fq accepted modified original byte {n}"
        );
    }
}
#[test]
fn incoming_terminal_inactive_padding_is_zero_and_has_same_three_sha_jobs() {
    assert!(check::<Fp>(false, None));
    assert!(check::<Fq>(false, None));
    assert!(!check::<Fp>(false, Some(10)));
    assert!(!check::<Fq>(false, Some(565)));
}
