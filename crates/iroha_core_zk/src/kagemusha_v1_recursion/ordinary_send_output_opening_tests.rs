//! Exact output/encrypted-byte/credit/clock mathematics only; no Native envelope owner is fabricated.
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::kagemusha::{
    kagemusha_ciphertext_digest_v1, kagemusha_ordinary_credit_id_v1,
    kagemusha_ordinary_transition_nullifier_v1, kagemusha_peer_credit_opening_commitment_v1,
};
const K: u32 = 16;
#[derive(Clone)]
struct OutputCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for OutputCircuit<F> {
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
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("explicit output Base params")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, p: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, p);
        base.set_usable_rows((1 << K) - 9);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn synthesize(&self, c: Self::Config, mut l: impl Layouter<F>) -> Result<(), Error> {
        self.builder
            .synthesize(c.0, l.namespace(|| "ordinary output Base"))?;
        self.jobs.synthesize(
            &c.1,
            &mut l,
            &self.builder.core().copy_manager,
            (1 << K) - 9,
        )
    }
}
#[derive(Clone, Copy)]
enum Mutation {
    None,
    Amount,
    CipherByte,
    Clock,
    SecureIndex,
}
fn circuit<F: KagemushaPoseidonFieldV1>(mutation: Mutation) -> OutputCircuit<F> {
    let before = [1; 32];
    let after = [2; 32];
    let epoch = [3; 32];
    let network = [4; 32];
    let lane = [5; 32];
    let pool = [6; 32];
    let request = [7; 32];
    let recipient_key = [9; 32];
    let secure = (1_u128 << 100) + 41;
    let amount = (1_u128 << 101) + 17;
    let nullifier =
        kagemusha_ordinary_transition_nullifier_v1(before, secure, epoch, network, lane, pool)
            .unwrap();
    let credit_id = kagemusha_ordinary_credit_id_v1(nullifier, request);
    let opening = KagemushaCreditOpeningV1 {
        version: 1,
        credit_id,
        amount,
        credit_commitment_opening: [11; 32],
        recipient_binding_opening: [12; 32],
        recovery_nonce: [13; 32],
    };
    let cipher = kagemusha_peer_credit_opening_commitment_v1(
        request,
        recipient_key,
        amount,
        opening.credit_commitment_opening,
        opening.recipient_binding_opening,
        opening.recovery_nonce,
    )
    .unwrap();
    // Known-public data exercises the exact full-stream hash only, not AEAD syntax/decryptability.
    let encrypted = vec![0x5a; 77];
    let encrypted_digest = kagemusha_ciphertext_digest_v1(&encrypted);
    let clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [14; 32],
        signed_observations_original_digest: [15; 32],
        lower_at_ms: 1000,
        upper_at_ms: 1001,
    };
    let output = KagemushaOrdinaryPaymentOutputV1 {
        version: 1,
        request_digest: request,
        amount,
        sender_before_commitment: before,
        sender_after_commitment: after,
        transition_nullifier: nullifier,
        credit_id,
        ciphertext_commitment: cipher,
        encrypted_credit_digest: encrypted_digest,
        clock_context_digest: clock.binding_digest().unwrap(),
        prepared_at_ms: 1001,
    };
    let output_digest = output.binding_digest().unwrap();
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut bytes = |d: [u8; 32]| -> Bytes<F> { assign_bytes(ctx, &range, &d).try_into().unwrap() };
    let before = bytes(before);
    let after = bytes(after);
    let epoch = bytes(epoch);
    let network = bytes(network);
    let lane = bytes(lane);
    let pool = bytes(pool);
    let receiver = OrdinaryReceiverRequestOpeningV1 {
        request_digest: bytes(request),
        credential_digest: bytes([8; 32]),
        encryption_key: bytes(recipient_key),
        recipient_lane: bytes([16; 32]),
    };
    // Preserve the original witness assignment order after dropping its unused projection.
    let _ = bytes([17; 32]);
    let credit = bytes(credit_id);
    let expected_output = bytes(output_digest);
    let expected_encrypted = bytes(encrypted_digest);
    let expected_nullifier = bytes(nullifier);
    let expected_cipher = bytes(cipher);
    let clock_cells = OrdinaryCashClockCellsV1 {
        nonce: bytes(clock.request_nonce),
        signed_observations_original_digest: bytes(clock.signed_observations_original_digest),
        lower_at_ms: ctx.load_witness(F::from(clock.lower_at_ms)),
        upper_at_ms: ctx.load_witness(F::from(
            clock.upper_at_ms + u64::from(matches!(mutation, Mutation::Clock)),
        )),
    };
    let operation = ctx.load_witness(F::from(2));
    let amount = ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(
        amount + u128::from(matches!(mutation, Mutation::Amount)),
    ));
    let secure = ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(
        secure + u128::from(matches!(mutation, Mutation::SecureIndex)),
    ));
    let mut original = encrypted;
    if matches!(mutation, Mutation::CipherByte) {
        original[76] ^= 1;
    }
    let mut buffer = vec![0; KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1];
    buffer[..original.len()].copy_from_slice(&original);
    let buffer = assign_bytes(ctx, &range, &buffer);
    let length = ctx.load_witness(F::from(original.len() as u64));
    let stream = KagemushaBoundedByteStreamV1::constrain(ctx, &range, buffer, length).unwrap();
    let mut jobs = PastaSha256JobsV1::default();
    constrain_ordinary_send_output_opening_v1(
        ctx,
        &range,
        &mut jobs,
        OrdinarySendOutputSourcesV1 {
            operation,
            amount,
            before,
            after,
            predecessor_secure_index: secure,
            predecessor_epoch: epoch,
            network,
            sender_lane: lane,
            reserve_pool: pool,
            receiver: &receiver,
            selected_credit_id: credit,
            encrypted_credit: &stream,
            preparation_clock: &clock_cells,
            preparation_clock_specimen: &clock,
            expected_output_digest: expected_output,
            expected_encrypted_digest: expected_encrypted,
            expected_nullifier,
            expected_ciphertext_commitment: expected_cipher,
        },
        &output,
        Some(&opening),
    )
    .unwrap();
    super::super::base_packing::finalize_base_params_v1(&mut builder, 9).unwrap();
    jobs.validate_capacity((1 << K) - 9).unwrap();
    OutputCircuit { builder, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>() {
    for mutation in [
        Mutation::None,
        Mutation::Amount,
        Mutation::CipherByte,
        Mutation::Clock,
        Mutation::SecureIndex,
    ] {
        let c = circuit::<F>(mutation);
        assert_eq!(
            MockProver::run(K, &c, vec![vec![]])
                .unwrap()
                .verify()
                .is_ok(),
            matches!(mutation, Mutation::None)
        );
    }
}
#[test]
fn ordinary_output_opens_exact_amount_ciphertext_clock_and_full_secure_index_in_both_fields() {
    std::thread::Builder::new()
        .name("ordinary-output-opening".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            check::<Fp>();
            check::<Fq>();
        })
        .unwrap()
        .join()
        .unwrap();
}
