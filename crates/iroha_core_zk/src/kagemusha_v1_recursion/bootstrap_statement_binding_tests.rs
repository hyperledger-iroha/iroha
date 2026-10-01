//! Model SHA/CRC equality and adversarial assigned-state binding in both Pasta fields.
//!
//! The fixture is a projection only. These tests exercise the real Base/Table8 SHA relation,
//! independently of monetary State admission, recursive proof production or device qualification.

use halo2_base::{
    AssignedValue, Context,
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};

use super::*;
use crate::kagemusha_v1_state::BootstrapStatementV1;
use crate::pasta_sha256::{PastaSha256ConfigV1, PastaSha256JobsV1, PastaSha256PlanMessageV1};

const TEST_K: u32 = 17;
const UNUSABLE_ROWS: usize = 9;
const DOMAIN: &[u8] = b"iroha:kagemusha:v1:bootstrap-statement\0";

#[derive(Clone, Debug)]
struct BindingConfig<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
}

#[derive(Clone)]
struct BindingCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for BindingCircuit<F> {
    type Config = BindingConfig<F>;
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
        base.set_usable_rows((1_usize << TEST_K) - UNUSABLE_ROWS);
        BindingConfig {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("bootstrap SHA tests require explicit Base parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "bootstrap binding Base"),
        )?;
        self.jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1_usize << TEST_K) - UNUSABLE_ROWS,
        )
    }
}

fn fixture() -> KagemushaStateRelationWitnessV1 {
    let p = super::tests::public_projection_fixture();
    KagemushaStateRelationWitnessV1 {
        operation: KagemushaOperationV1::Bootstrap,
        predecessor: None,
        successor: p.successor,
        amount: 0,
        journal_revision_before: 0,
        journal_revision_after: 0,
        transition_effect_digest: p.transition_effect_digest,
        mint_finality_semantic_digest: p.mint_finality_semantic_digest,
        mint_finality_proof_binding_digest: p.mint_finality_proof_binding_digest,
        peer_credit_id: p.peer_credit_id,
        recipient_encryption_key_binding: p.recipient_encryption_key_binding,
        receive_credit: None,
        receive_credit_binding_digest: p.receive_credit_binding_digest,
        lifecycle_binding_digest: p.lifecycle_binding_digest,
        prepared_transition_binding_digest: p.prepared_transition_binding_digest,
        prepared_intent: None,
        transport_semantic_digest: p.transport_semantic_digest,
        guard_statement_digest: p.guard_statement_digest,
        eq_protocol_digest: p.eq_protocol_digest,
        ep_protocol_digest: p.ep_protocol_digest,
        guard_eq_protocol_digest: p.guard_eq_protocol_digest,
        guard_ep_protocol_digest: p.guard_ep_protocol_digest,
        mint_eq_protocol_digest: p.mint_eq_protocol_digest,
        mint_ep_protocol_digest: p.mint_ep_protocol_digest,
        mint_authorization_eq_protocol_digest: p.mint_authorization_eq_protocol_digest,
        mint_authorization_ep_protocol_digest: p.mint_authorization_ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: p.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: p.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit: p.guard_eq_credential_audit,
        guard_ep_credential_audit: p.guard_ep_credential_audit,
        eq_deferred_audit: p.eq_deferred_audit,
        ep_deferred_audit: p.ep_deferred_audit,
        replay_insert: None,
    }
}

fn statement(s: &KagemushaStateV1) -> BootstrapStatementV1 {
    BootstrapStatementV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        protocol_version: s.protocol_version,
        suite_id: s.suite_id,
        vk_digest: s.vk_digest,
        release_id: s.release_id,
        asset_incarnation: s.asset_incarnation,
        liability_pool_id: s.liability_pool_id,
        hardware_profile_id: s.hardware_profile_id,
        policy_epoch: s.policy_epoch,
        lane: s.lane.clone(),
        hardware_epoch: s.hardware_epoch,
        device_policy_binding: s.device_policy_binding,
        next_one_use_key_reference: s.next_one_use_key_reference,
        state_nonce_commitment: s.state_nonce_commitment,
        state_commitment: s.state_commitment,
    }
}

fn digest_cells<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    digest_limbs::<F>(digest).map(|limb| ctx.load_witness(limb))
}

fn assigned_state<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    s: &KagemushaStateV1,
) -> AssignedState<F> {
    AssignedState {
        protocol_version: ctx.load_witness(F::from(u64::from(s.protocol_version))),
        suite_id: digest_cells(ctx, s.suite_id),
        vk_digest: digest_cells(ctx, s.vk_digest),
        balance: ctx.load_witness(from_u128(s.balance)),
        sequence: ctx.load_witness(from_u128(s.logical_sequence)),
        secure_index: ctx.load_witness(from_u128(s.secure_index)),
        epoch_generation: ctx.load_witness(from_u128(s.hardware_epoch.generation)),
        epoch_id: digest_cells(ctx, s.hardware_epoch.epoch_id),
        key_reference: digest_cells(ctx, s.device_policy_binding.device_key_reference),
        policy_id: digest_cells(ctx, s.device_policy_binding.hardware_policy_id),
        next_one_use_key_reference: digest_cells(ctx, s.next_one_use_key_reference),
        nonce: digest_cells(ctx, s.state_nonce_commitment),
        replay_root: ctx.load_witness(F::ZERO),
        commitment: ctx.load_witness(F::ZERO),
        release_id: digest_cells(ctx, s.release_id),
        asset_incarnation: digest_cells(ctx, *s.asset_incarnation.as_bytes()),
        liability_pool_id: digest_cells(ctx, s.liability_pool_id),
        hardware_profile_id: digest_cells(ctx, s.hardware_profile_id),
        policy_epoch: ctx.load_witness(F::from(s.policy_epoch)),
        network_id: digest_cells(ctx, s.lane.normalized_network_id()),
        asset_id: digest_cells(
            ctx,
            s.lane.normalized_asset_id().expect("model asset identity"),
        ),
        scale: ctx.load_witness(F::from(u64::from(s.lane.scale))),
        lane_id: digest_cells(ctx, s.lane.device_lane_id),
    }
}

fn assigned_relation<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    s: &KagemushaStateV1,
) -> KagemushaAssignedStateRelationV1<F> {
    let zero = ctx.load_witness(F::ZERO);
    let pair = [zero; 2];
    let successor = assigned_state(ctx, s);
    KagemushaAssignedStateRelationV1 {
        operation: zero,
        amount: zero,
        predecessor: successor,
        successor,
        predecessor_outer: pair,
        successor_outer: digest_cells(ctx, s.state_commitment),
        guard_digest: pair,
        journal_revision_before: zero,
        journal_revision_after: zero,
        transition_effect_digest: pair,
        mint_finality_semantic_digest: pair,
        mint_finality_proof_binding_digest: pair,
        peer_credit_id: pair,
        recipient_encryption_key_binding: pair,
        receive_credit: KagemushaAssignedReceiveFoldCreditV1 {
            active: zero,
            amount: zero,
            credit_id: pair,
            recipient_lane_id: pair,
            incoming_proof_binding_digest: pair,
            request_digest: pair,
            prepared_transfer_digest: pair,
            transition_nullifier: pair,
            recipient_encryption_key: pair,
            ciphertext_commitment: pair,
            credit_commitment_opening: pair,
            recipient_binding_opening: pair,
            recovery_nonce: pair,
            receiver_binding_digest: pair,
            payment_output_digest: pair,
            envelope_digest: pair,
        },
        receive_credit_binding_digest: pair,
        lifecycle_binding_digest: pair,
        prepared_transition_binding_digest: pair,
        predecessor_eq_components: pair,
        predecessor_ep_components: pair,
        successor_eq_components: pair,
        successor_ep_components: pair,
        replay_credit_id: pair,
        replay_envelope_digest: pair,
    }
}

fn circuit<F: KagemushaPoseidonFieldV1>(
    witness: &KagemushaStateRelationWitnessV1,
    assigned: &KagemushaStateV1,
    corrupt_sha_output: bool,
) -> BindingCircuit<F> {
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(TEST_K as usize)
        .use_lookup_bits(TEST_K as usize - 1)
        .use_instance_columns(1);
    let relation = assigned_relation(builder.main(0), assigned);
    let mut jobs = PastaSha256JobsV1::default();
    if corrupt_sha_output {
        // Job zero is the typed UUID asset digest; job one is the final bootstrap SHA.
        jobs = jobs.with_output_word_xor(1, 0, 1);
    }
    let digest =
        constrain_bootstrap_statement_digest_v1(&mut builder, &mut jobs, &relation, witness)
            .expect("bootstrap binding relation");
    builder.assigned_instances = vec![
        digest
            .iter()
            .map(|byte| byte.assigned().expect("assigned SHA output byte"))
            .collect(),
    ];
    builder.calculate_params(Some(UNUSABLE_ROWS));
    BindingCircuit { builder, jobs }
}

fn public_digest<F: KagemushaPoseidonFieldV1>(s: &KagemushaStateV1) -> Vec<F> {
    statement(s)
        .proof_statement_digest()
        .expect("model bootstrap digest")
        .into_iter()
        .map(|byte| F::from(u64::from(byte)))
        .collect()
}

fn proves<F: KagemushaPoseidonFieldV1>(c: &BindingCircuit<F>, digest: Vec<F>) -> bool {
    MockProver::run(TEST_K, c, vec![digest])
        .expect("bootstrap SHA mock prover")
        .verify()
        .is_ok()
}

fn queued_bootstrap<F: KagemushaPoseidonFieldV1>(c: &BindingCircuit<F>) -> Vec<u8> {
    let messages = c.jobs.canonical_plan_messages().expect("actual SHA queue");
    assert_eq!(messages.len(), 2, "typed asset then canonical bootstrap");
    match messages.into_iter().last().unwrap() {
        PastaSha256PlanMessageV1::Ordinary(bytes) => bytes,
        _ => panic!("bootstrap SHA has a fixed complete model message"),
    }
}

fn assert_model_message<F: KagemushaPoseidonFieldV1>(c: &BindingCircuit<F>, s: &KagemushaStateV1) {
    let frame = norito::encode_canonical(&statement(s)).expect("authoritative model frame");
    let crc = u64::from_le_bytes(frame[31..39].try_into().unwrap());
    let payload_len = usize::try_from(u64::from_le_bytes(frame[23..31].try_into().unwrap()))
        .expect("model payload length");
    let payload_start = frame
        .len()
        .checked_sub(payload_len)
        .expect("complete model payload");
    assert!(payload_start >= norito::core::Header::SIZE);
    assert!(
        frame[norito::core::Header::SIZE..payload_start]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(crc, norito::crc64_fallback(&frame[payload_start..]));
    let mut expected = (DOMAIN.len() as u64).to_be_bytes().to_vec();
    expected.extend_from_slice(DOMAIN);
    expected.extend_from_slice(&(frame.len() as u64).to_be_bytes());
    expected.extend_from_slice(&frame);
    assert_eq!(queued_bootstrap(c), expected);
}

fn positive<F: KagemushaPoseidonFieldV1>() {
    let witness = fixture();
    let c = circuit::<F>(&witness, &witness.successor, false);
    assert_model_message(&c, &witness.successor);
    assert!(proves(&c, public_digest(&witness.successor)));
    let mut changed_public = public_digest::<F>(&witness.successor);
    changed_public[0] += F::ONE;
    assert!(!proves(&c, changed_public));
}

#[test]
fn bootstrap_model_sha_and_crc_equal_real_pasta_relation_in_both_parities() {
    positive::<Fp>();
    positive::<Fq>();
}

fn mutations<F: KagemushaPoseidonFieldV1>() {
    let witness = fixture();
    for mutation in 0..5 {
        let mut changed = witness.successor.clone();
        match mutation {
            0 => changed.state_commitment[0] ^= 2,
            1 => changed.device_policy_binding.hardware_policy_id[0] ^= 2,
            2 => changed.policy_epoch += 1,
            3 => changed.state_nonce_commitment[0] ^= 2,
            _ => changed.hardware_epoch.generation += 1,
        }
        let c = circuit::<F>(&witness, &changed, false);
        // Every replaced semantic byte and checksum must come from the assigned State,
        // even though the layout locator still receives the unchanged host witness.
        assert_model_message(&c, &changed);
        assert!(
            !proves(&c, public_digest(&witness.successor)),
            "mutation {mutation}"
        );
    }
}

#[test]
fn bootstrap_assigned_state_policy_nonce_and_epoch_reject_old_digest_in_both_parities() {
    mutations::<Fp>();
    mutations::<Fq>();
}

fn asset_substitution<F: KagemushaPoseidonFieldV1>() {
    let mut witness = fixture();
    let original = witness.successor.clone();
    let mut uuid = witness.successor.lane.asset.aid_bytes();
    uuid[0] ^= 2;
    witness.successor.lane.asset =
        iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes(uuid)
            .expect("distinct valid UUID with unchanged version and variant");
    assert_ne!(witness.successor.lane.asset, original.lane.asset);
    let c = circuit::<F>(&witness, &original, false);
    // A different host UUID cannot replace the assigned model asset digest, even if the
    // caller presents the digest of that different full canonical bootstrap statement.
    assert!(!proves(&c, public_digest(&witness.successor)));
}

#[test]
fn bootstrap_typed_asset_uuid_is_bound_to_assigned_identity_in_both_parities() {
    asset_substitution::<Fp>();
    asset_substitution::<Fq>();
}

fn output_substitution<F: KagemushaPoseidonFieldV1>() {
    let witness = fixture();
    let c = circuit::<F>(&witness, &witness.successor, true);
    assert_model_message(&c, &witness.successor);
    let mut forged = public_digest::<F>(&witness.successor);
    // Match the deliberately corrupted first SHA word. Base constraints and public bytes
    // agree; only the independently synthesized real SHA compression must reject it.
    let honest = statement(&witness.successor)
        .proof_statement_digest()
        .unwrap();
    let word = u32::from_be_bytes(honest[..4].try_into().unwrap()) ^ 1;
    for (slot, byte) in forged[..4].iter_mut().zip(word.to_be_bytes()) {
        *slot = F::from(u64::from(byte));
    }
    assert!(!proves(&c, forged));
}

#[test]
fn bootstrap_host_sha_output_cannot_bypass_real_compression_in_both_parities() {
    output_substitution::<Fp>();
    output_substitution::<Fq>();
}
