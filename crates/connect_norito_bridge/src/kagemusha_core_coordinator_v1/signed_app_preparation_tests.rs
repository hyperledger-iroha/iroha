//! Exact issuer signatures test preparation bindings; they never qualify physical hardware.
use super::*;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId, asset::AssetDefinitionId, block::BlockHeader,
    kagemusha::KagemushaRetailEnrollmentRuntimeV1, nexus::AxtAssetIncarnationV1,
};
use iroha_model_base::topology::DataSpaceId;

fn fixture() -> (AccountId, KagemushaRetailEnrollmentIssuerPolicyV1, KeyPair) {
    let issuer = KeyPair::from_seed(vec![81; 32], Algorithm::Ed25519);
    let account = AccountId::new(
        KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let policy = KagemushaRetailEnrollmentIssuerPolicyV1 {
        version: 1,
        issuer_policy_id: [71; 32],
        issuer_public_key: issuer.public_key().clone(),
        issuer_audience: "test-issuer".parse().unwrap(),
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "test-bank".parse().unwrap(),
            ledger_dataspace_id: DataSpaceId::new(7),
            authentication_namespace: "test.bank".parse().unwrap(),
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"custom-preparation-test-network",
                )),
            ),
            asset: AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"custom-preparation-test-incarnation").as_ref(),
            )
            .unwrap(),
            scale: 2,
        },
        valid_from_ms: 100,
        expires_at_ms: 200_000,
        maximum_certificate_lifetime_ms: 4_000,
    };
    (account, policy, issuer)
}
fn token(
    account: &AccountId,
    policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
    issuer: &KeyPair,
    key: [u8; 32],
) -> Vec<u8> {
    let mut value = vec![VERSION];
    value.extend_from_slice(&1_000_u64.to_le_bytes());
    value.extend_from_slice(&121_000_u64.to_le_bytes());
    for field in [[1; 32], [7; 32], [2; 32], [3; 32], key, [5; 32]] {
        value.extend_from_slice(&field);
    }
    let mut message = DOMAIN.to_vec();
    message.extend_from_slice(&value[1..209]);
    message.extend_from_slice(&policy.issuer_policy_id);
    message.extend_from_slice(&Sha256::digest(
        account.canonical_i105().unwrap().as_bytes(),
    ));
    value.extend_from_slice(
        Signature::try_new(issuer.private_key(), &message)
            .unwrap()
            .payload(),
    );
    value
}
fn pins<'a>(
    account: &'a AccountId,
    policy: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
    class: KagemushaHardwarePlatformClassV1,
    key: [u8; 32],
) -> SignedAppPreparationPinsV1<'a> {
    SignedAppPreparationPinsV1 {
        policy,
        account_id: account,
        platform_class: class,
        selected_attested_key_id: key,
        client_nonce: [1; 32],
        release_id: [2; 32],
        profile_id: [3; 32],
        lane_id: [5; 32],
        trusted_now_ms: 1_001,
    }
}
#[test]
fn custom_preparation_requires_exact_selected_nonzero_hardware_point_identity() {
    let (account, policy, issuer) = fixture();
    let original = token(&account, &policy, &issuer, [4; 32]);
    for class in [
        KagemushaHardwarePlatformClassV1::AndroidOemService,
        KagemushaHardwarePlatformClassV1::AppleOemService,
        KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        KagemushaHardwarePlatformClassV1::OtherQualified,
    ] {
        assert_eq!(
            verify_signed_app_preparation_v1(&original, pins(&account, &policy, class, [4; 32]))
                .unwrap()
                .attested_key_id,
            [4; 32]
        );
        assert!(
            verify_signed_app_preparation_v1(&original, pins(&account, &policy, class, [9; 32]))
                .is_err()
        );
        assert!(
            verify_signed_app_preparation_v1(
                &token(&account, &policy, &issuer, [0; 32]),
                pins(&account, &policy, class, [0; 32])
            )
            .is_err()
        );
    }
}
#[test]
fn ordinary_keymint_zero_sentinel_cannot_authorize_a_custom_hardware_identity() {
    let (account, policy, issuer) = fixture();
    let original = token(&account, &policy, &issuer, [0; 32]);
    assert!(
        verify_signed_app_preparation_v1(
            &original,
            pins(
                &account,
                &policy,
                KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                [0; 32]
            )
        )
        .is_ok()
    );
    assert!(
        verify_signed_app_preparation_v1(
            &original,
            pins(
                &account,
                &policy,
                KagemushaHardwarePlatformClassV1::OtherQualified,
                [0; 32]
            )
        )
        .is_err()
    );
    assert!(
        verify_signed_app_preparation_v1(
            &token(&account, &policy, &issuer, [4; 32]),
            pins(
                &account,
                &policy,
                KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                [4; 32]
            )
        )
        .is_err()
    );
}
