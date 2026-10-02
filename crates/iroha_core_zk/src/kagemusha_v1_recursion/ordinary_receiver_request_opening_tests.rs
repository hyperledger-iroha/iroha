//! Genuine full receiver issuer/platform/SHA equations on public fixture keys, without Native authority.
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::{
    kagemusha::{
        KagemushaAppOperationApprovalEvidenceV1, KagemushaOrdinaryCashClockContextV1,
        KagemushaOrdinaryPaymentRequestBodyV1,
    },
    testing::ordinary_app_enrollment::{
        KagemushaOrdinaryRetailEnrollmentFixtureV1, ordinary_test_issuer_public_key_v1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};
const K: u32 = 17;
#[derive(Clone)]
struct ReceiverCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for ReceiverCircuit<F> {
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
        unreachable!("receiver fixture requires Base params")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1 << K) - 9);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder.synthesize(
            config.0,
            layouter.namespace(|| "full ordinary receiver Base"),
        )?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1 << K) - 9,
        )
    }
}
#[derive(Clone, Copy)]
enum Mutation {
    None,
    Amount,
    Original,
    Operation,
    Expired,
}
fn circuit<F: KagemushaPoseidonFieldV1>(apple: bool, mutation: Mutation) -> ReceiverCircuit<F> {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(apple);
    let token = fixture.verify(300).unwrap();
    let credential =
        KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(token.app_credential().original())
            .unwrap();
    let receiver_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [31; 32],
        signed_observations_original_digest: [32; 32],
        lower_at_ms: 300,
        upper_at_ms: 301,
    };
    let body = KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: credential.subject.release_id,
        network_id: credential.subject.network_id,
        normalized_asset_id: [41; 32],
        asset_incarnation: [42; 32],
        scale: 4,
        reserve_pool_id: [43; 32],
        recipient_account_binding: credential.subject.account_binding,
        amount: (1_u128 << 100) + 17,
        recipient_encryption_key: [9; 32],
        recipient_credential_digest: token.app_credential().digest(),
        recipient_lane_id: credential.subject.lane_id,
        request_id: [44; 32],
        clock_context: receiver_clock,
        issued_at_ms: 300,
        expires_at_ms: 350,
    };
    let signing = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let message = body.canonical_signing_bytes().unwrap();
    let mut authenticator = credential.subject.app_signing_identity_digest.to_vec();
    authenticator.push(0x40);
    authenticator.extend_from_slice(&12_u32.to_be_bytes());
    let signing_message = if apple {
        let mut b = authenticator.clone();
        b.extend(Sha256::digest(&message));
        Sha256::digest(b).to_vec()
    } else {
        message
    };
    let signature: Signature = signing.sign(&signing_message);
    let der = signature.to_der();
    let evidence = if apple {
        let mut raw = vec![0xa2, 0x69];
        raw.extend(b"signature");
        raw.extend([0x58, der.len() as u8]);
        raw.extend(der.as_bytes());
        raw.push(0x71);
        raw.extend(b"authenticatorData");
        raw.extend([0x58, 37]);
        raw.extend(authenticator);
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
    } else {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: der.as_bytes().to_vec(),
        }
    };
    let request = KagemushaOrdinaryPaymentRequestV1 { body, evidence };
    let mut table = OrdinaryIssuerTableV1::default();
    table.slots[0] = super::super::ordinary_issuer_config::OrdinaryIssuerProfileV1 {
        profile_id: credential.subject.hardware_profile_id,
        issuer_sec1: *ordinary_test_issuer_public_key_v1().as_sec1_bytes(),
    };
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(16)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut bytes =
        |raw: &[u8; 32]| -> Bytes<F> { assign_bytes(ctx, &range, raw).try_into().unwrap() };
    let release = bytes(&body.release_id);
    let network = bytes(&body.network_id);
    let asset = bytes(&body.normalized_asset_id);
    let incarnation = bytes(&body.asset_incarnation);
    let pool = bytes(&body.reserve_pool_id);
    let mut expected = request.canonical_original_digest().unwrap();
    if matches!(mutation, Mutation::Original) {
        expected[31] ^= 1;
    }
    let expected = bytes(&expected);
    let expected_c = bytes(&body.recipient_credential_digest);
    let clock_nonce = bytes(&[51; 32]);
    let clock_original = bytes(&[52; 32]);
    let clock = OrdinaryCashClockCellsV1 {
        nonce: clock_nonce,
        signed_observations_original_digest: clock_original,
        lower_at_ms: ctx.load_witness(F::from(302)),
        upper_at_ms: ctx.load_witness(F::from(if matches!(mutation, Mutation::Expired) {
            350
        } else {
            303
        })),
    };
    let operation = ctx.load_witness(F::from(if matches!(mutation, Mutation::Operation) {
        4
    } else {
        2
    }));
    let amount = ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(
        body.amount + u128::from(matches!(mutation, Mutation::Amount)),
    ));
    let scale = ctx.load_witness(F::from(body.scale as u64));
    let mut jobs = PastaSha256JobsV1::default();
    constrain_ordinary_receiver_request_opening_v1(
        &mut builder,
        &mut jobs,
        &table,
        OrdinaryReceiverRequestSourcesV1 {
            operation,
            release,
            network,
            normalized_asset: asset,
            incarnation,
            scale,
            reserve_pool: pool,
            amount,
            preparation_clock: &clock,
            expected_request_digest: expected,
            expected_recipient_credential_digest: expected_c,
        },
        OrdinaryReceiverRequestWitnessV1 {
            request: &request,
            credential: &credential,
            integrity_lease: None,
            previous_app_attest_counter: apple.then_some(11),
            enabled: true,
        },
    )
    .unwrap();
    super::super::base_packing::finalize_base_params_v1(&mut builder, 9).unwrap();
    jobs.validate_capacity((1 << K) - 9).unwrap();
    ReceiverCircuit { builder, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>() {
    for apple in [false, true] {
        for mutation in [
            Mutation::None,
            Mutation::Amount,
            Mutation::Original,
            Mutation::Operation,
            Mutation::Expired,
        ] {
            let circuit = circuit::<F>(apple, mutation);
            let prover = MockProver::run(K, &circuit, vec![vec![]]).unwrap();
            assert_eq!(prover.verify().is_ok(), matches!(mutation, Mutation::None));
        }
    }
}
#[test]
fn full_ordinary_receiver_original_and_signature_bind_amount_scope_operation_and_actual_clock_in_both_fields()
 {
    std::thread::Builder::new()
        .name("ordinary-receiver-full-equations".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            check::<Fp>();
            check::<Fq>();
        })
        .unwrap()
        .join()
        .unwrap();
}
