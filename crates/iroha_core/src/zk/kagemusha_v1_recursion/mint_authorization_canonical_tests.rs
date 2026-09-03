//! Assigned canonical-hash regressions for the mint-authorization relation.
//!
//! These tests synthesize the real Base and Table8 SHA paths in the parent circuits. Their dense
//! job lists are deliberately empty: they do not authenticate a recursive credential, generate a
//! mint proof, or qualify a complete mint-authorizing circuit.

use core::ops::Range;

use halo2_proofs::dev::MockProver;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, KagemushaHardwarePlatformClassV1,
        kagemusha_liability_pool_id_v1, kagemusha_mint_credit_opening_commitment_preimage_v1,
        kagemusha_recipient_credential_commitment_preimage_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use p256::ecdsa::SigningKey;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    None,
    ProfileField,
    ProfileId,
    RecipientOpening,
    CreditOpening,
}

struct CommitmentFixture {
    canonical: Vec<u8>,
    layout: Vec<Option<u8>>,
    field_ranges: Vec<Range<usize>>,
    opening_field: usize,
    domain: &'static [u8],
    expected_digest: DigestV1,
}

fn profile_fixture() -> KagemushaHardwareProfileV1 {
    let signing_key = SigningKey::from_bytes((&[0x31; 32]).into()).expect("fixture P-256 key");
    let governance_credential_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .expect("fixture canonical governance key");
    let profile = KagemushaHardwareProfileV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: [1; 32],
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: [2; 32],
        firmware_policy_digest: [3; 32],
        enrollment_attestation_verifier_digest: [4; 32],
        attestation_trust_roots_digest: [5; 32],
        allowed_suite_commitment: [6; 32],
        policy_epoch: 7,
        governance_credential_public_key,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: [8; 32],
        valid_from_ms: 9,
        expires_at_ms: 10_000,
    }
    .seal_hardware_profile_id()
    .expect("fixture canonical profile identity");
    profile.validate().expect("valid canonical model profile");
    profile
}

fn recipient_fixture() -> CommitmentFixture {
    let operation = [0x11; 32];
    let credential = [0x22; 32];
    let opening = [0x33; 32];
    let canonical =
        kagemusha_recipient_credential_commitment_preimage_v1(operation, credential, opening)
            .expect("model recipient commitment preimage");
    assert_eq!(
        canonical.len(),
        KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1
    );
    CommitmentFixture {
        canonical: canonical.to_vec(),
        layout: kagemusha_recipient_credential_commitment_preimage_layout_v1()
            .expect("model recipient framing")
            .to_vec(),
        field_ranges: KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1.to_vec(),
        opening_field: 2,
        domain: RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN_V1,
        expected_digest: kagemusha_recipient_credential_commitment_v1(
            operation, credential, opening,
        )
        .expect("model recipient commitment"),
    }
}

fn credit_fixture() -> CommitmentFixture {
    let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"kagemusha-mint-assigned-canonical-hashes"),
    ));
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("fixture domain"),
        "xor".parse().expect("fixture asset name"),
    );
    let incarnation = AxtAssetIncarnationV1::derive(
        &network,
        &asset,
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"mint-fixture-registration")),
        &Hash::new(b"mint-fixture-registration-execution"),
        1,
    );
    let pool = kagemusha_liability_pool_id_v1(&network, &asset, incarnation)
        .expect("model pooled reserve identity");
    let recipient_key = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    let recipient = AccountId::new(recipient_key.public_key().clone());
    let mut recipient_one_time_key = [0; 32];
    recipient_one_time_key[0] = 9;
    let opening = [0x51; 32];
    let canonical = kagemusha_mint_credit_opening_commitment_preimage_v1(
        &network,
        &asset,
        incarnation,
        2,
        pool,
        123_456,
        &recipient,
        recipient_one_time_key,
        opening,
    )
    .expect("model mint-credit opening preimage");
    assert_eq!(
        canonical.len(),
        KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1
    );
    CommitmentFixture {
        canonical: canonical.to_vec(),
        layout: kagemusha_mint_credit_opening_commitment_preimage_layout_v1()
            .expect("model mint-credit framing")
            .to_vec(),
        field_ranges: KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1.to_vec(),
        opening_field: 9,
        domain: MINT_CREDIT_OPENING_COMMITMENT_DOMAIN_V1,
        expected_digest: kagemusha_mint_credit_opening_commitment_v1(
            &network,
            &asset,
            incarnation,
            2,
            pool,
            123_456,
            &recipient,
            recipient_one_time_key,
            opening,
        )
        .expect("model mint-credit opening commitment"),
    }
}

fn assign_commitment<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    fixture: &CommitmentFixture,
    substitute_opening: bool,
) -> [PastaSha256ByteV1<F>; 32] {
    let fields = fixture
        .field_ranges
        .iter()
        .enumerate()
        .map(|(index, field_range)| {
            let mut bytes = fixture.canonical[field_range.clone()].to_vec();
            if substitute_opening && index == fixture.opening_field {
                // Keep the original expected model digest. No host validation examines this
                // substituted semantic witness before the actual CRC and SHA constraints run.
                bytes[0] ^= 1;
            }
            assign_bytes(ctx, range, &bytes)
        })
        .collect::<Vec<_>>();
    let fields = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let preimage =
        assemble_canonical_preimage_v1(ctx, range, &fixture.layout, &fixture.field_ranges, &fields)
            .expect("assigned complete canonical commitment frame");
    let actual = hash_framed(ctx, jobs, fixture.domain, fixture.canonical.len(), preimage)
        .expect("queued canonical commitment SHA");
    let expected = assign_digest(ctx, range, fixture.expected_digest);
    bind_equal_digest(ctx, range, &actual, &expected);
    expected
}

struct HashOnlyCase<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    sha_jobs: PastaSha256JobsV1<F>,
    public_values: Vec<F>,
}

fn hash_only_case<F: KagemushaPoseidonFieldV1>(mutation: Mutation) -> HashOnlyCase<F> {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(KAGEMUSHA_HALO2_K_V1 as usize)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut sha_jobs = PastaSha256JobsV1::default();
    let mut profile = profile_fixture();
    let mut expected_profile_id = profile.hardware_profile_id;
    if mutation == Mutation::ProfileField {
        profile.firmware_policy_digest[0] ^= 1;
    }
    if mutation == Mutation::ProfileId {
        expected_profile_id[0] ^= 1;
    }
    let assigned_profile_id = assign_digest(ctx, &range, expected_profile_id);
    // This invokes the actual production binder. In the mutation cases no profile.validate()
    // call or host-side digest comparison is allowed to stand in for circuit rejection.
    bind_hardware_profile(ctx, &range, &mut sha_jobs, &profile, &assigned_profile_id)
        .expect("assign actual hardware-profile relation");
    let recipient = recipient_fixture();
    let assigned_recipient = assign_commitment(
        ctx,
        &range,
        &mut sha_jobs,
        &recipient,
        mutation == Mutation::RecipientOpening,
    );
    let credit = credit_fixture();
    let assigned_credit = assign_commitment(
        ctx,
        &range,
        &mut sha_jobs,
        &credit,
        mutation == Mutation::CreditOpening,
    );
    let public_cells = [assigned_profile_id, assigned_recipient, assigned_credit]
        .into_iter()
        .flatten()
        .map(|byte| byte.assigned().expect("assigned public digest byte"))
        .collect();
    let public_values = [
        expected_profile_id,
        recipient.expected_digest,
        credit.expected_digest,
    ]
    .into_iter()
    .flatten()
    .map(|byte| F::from(u64::from(byte)))
    .collect();
    builder.assigned_instances = vec![public_cells];
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let (job_count, block_count, _) = sha_jobs.capacity_profile().expect("actual SHA jobs");
    assert_eq!((job_count, block_count), (3, 17));
    HashOnlyCase {
        builder,
        sha_jobs,
        public_values,
    }
}

macro_rules! assert_hash_only_case {
    ($field:ty, $circuit:ident, $mutation:expr, $expected:expr) => {{
        let mutation = $mutation;
        let case = hash_only_case::<$field>(mutation);
        let circuit = $circuit {
            builder: case.builder,
            sha_jobs: case.sha_jobs,
            dense_jobs: PastaDenseMsmJobsV1::default(),
        };
        let verified = MockProver::run(KAGEMUSHA_HALO2_K_V1, &circuit, vec![case.public_values])
            .expect("mint canonical Base and actual SHA synthesis")
            .verify();
        assert_eq!(
            verified.is_ok(),
            $expected,
            "{} {mutation:?}: {verified:?}",
            stringify!($field),
        );
    }};
}

#[test]
fn assigned_canonical_mint_hashes_match_model_in_both_parities() {
    assert_hash_only_case!(
        Fp,
        KagemushaMintAuthorizationEqCircuitV1,
        Mutation::None,
        true
    );
    assert_hash_only_case!(
        Fq,
        KagemushaMintAuthorizationEpCircuitV1,
        Mutation::None,
        true
    );
}

#[test]
fn assigned_canonical_mint_hashes_reject_substituted_fields_and_ids() {
    for mutation in [
        Mutation::ProfileField,
        Mutation::ProfileId,
        Mutation::RecipientOpening,
        Mutation::CreditOpening,
    ] {
        assert_hash_only_case!(Fp, KagemushaMintAuthorizationEqCircuitV1, mutation, false);
        assert_hash_only_case!(Fq, KagemushaMintAuthorizationEpCircuitV1, mutation, false);
    }
}
