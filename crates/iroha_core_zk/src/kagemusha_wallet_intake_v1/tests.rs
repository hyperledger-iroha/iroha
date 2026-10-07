//! Original-account components, not an installed producer or phone qualification.

use iroha_crypto::KeyPair;

use super::*;

#[test]
fn asset_original_binds_native_review_scale_and_exact_monetary_scope() {
    use iroha_crypto::Hash;
    use iroha_data_model::{asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};
    let scope = KagemushaWalletAssetScopeV1::new(
        AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .unwrap(),
        &AxtAssetIncarnationV1::try_from_bytes(
            *Hash::new(b"intake component asset incarnation").as_ref(),
        )
        .unwrap(),
        2,
    )
    .unwrap();
    let original = norito::encode_canonical(&scope).unwrap();
    assert_eq!(
        asset_scope(&original, &scope.asset_digest()).unwrap(),
        scope
    );
    for changed in [
        KagemushaWalletAssetScopeV1 {
            scale: 3,
            ..scope.clone()
        },
        KagemushaWalletAssetScopeV1 {
            asset_incarnation: *Hash::new(b"other intake component incarnation").as_ref(),
            ..scope.clone()
        },
    ] {
        let changed_original = norito::encode_canonical(&changed).unwrap();
        assert!(asset_scope(&changed_original, &scope.asset_digest()).is_err());
    }
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(asset_scope(&trailing, &scope.asset_digest()).is_err());
    assert!(asset_scope(&[], &scope.asset_digest()).is_err());
    assert!(
        asset_scope(
            &vec![0; ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1 + 1],
            &scope.asset_digest()
        )
        .is_err()
    );
    assert!(asset_scope(&original, &[0; 32]).is_err());
}

fn existing_account(seed: u8, algorithm: Algorithm) -> (KeyPair, AccountId, Vec<u8>) {
    let key = KeyPair::from_seed(vec![seed; 32], algorithm);
    let owner = AccountId::new(key.public_key().clone());
    let original = norito::encode_canonical(&owner).unwrap();
    (key, owner, original)
}

#[test]
fn account_original_must_be_exact_existing_ed25519_owner() {
    let (_, owner, original) = existing_account(7, Algorithm::Ed25519);
    let digest = kagemusha_wallet_account_digest_v1(&owner).unwrap();
    assert_eq!(account(&original, &digest).unwrap(), owner);
    let (_, other, foreign) = existing_account(11, Algorithm::Ed25519);
    assert_ne!(kagemusha_wallet_account_digest_v1(&other).unwrap(), digest);
    assert!(account(&foreign, &digest).is_err());
    assert!(account(&original, &[0; 32]).is_err());
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(account(&trailing, &digest).is_err());
    assert!(account(&[], &digest).is_err());
    assert!(account(&vec![0; ACCOUNT_ORIGINAL_MAX_BYTES_V1 + 1], &digest).is_err());
    let (_, secp, secp_original) = existing_account(13, Algorithm::Secp256k1);
    assert!(
        account(
            &secp_original,
            &kagemusha_wallet_account_digest_v1(&secp).unwrap()
        )
        .is_err()
    );
}

#[test]
fn native_open_authorization_binds_every_exact_original_and_source_role() {
    let (key, owner, account_original) = existing_account(17, Algorithm::Ed25519);
    let nonce = [19; 32];
    let manifest = [23; 32];
    let slot = KagemushaWalletSlotIdV1([29; 32]);
    let marker = [31; 32];
    let credential = [1, 2, 3];
    let certificate = [4, 5];
    let message = open_message(
        &nonce,
        &manifest,
        &slot,
        &marker,
        &credential,
        &certificate,
        &account_original,
        &[7, 8],
    );
    let signature = Signature::try_new(key.private_key(), &message).unwrap();
    authorize_account(&owner, &message, signature.payload()).unwrap();
    for changed in [
        open_message(
            &nonce,
            &manifest,
            &slot,
            &marker,
            &credential,
            &certificate,
            &account_original,
            &[7, 9],
        ),
        open_message(
            &[37; 32],
            &manifest,
            &slot,
            &marker,
            &credential,
            &certificate,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &[41; 32],
            &slot,
            &marker,
            &credential,
            &certificate,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &KagemushaWalletSlotIdV1([43; 32]),
            &marker,
            &credential,
            &certificate,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &slot,
            &[47; 32],
            &credential,
            &certificate,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &slot,
            &marker,
            &[1, 2, 4],
            &certificate,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &slot,
            &marker,
            &credential,
            &[4, 6],
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &slot,
            &marker,
            &certificate,
            &credential,
            &account_original,
            &[7, 8],
        ),
        open_message(
            &nonce,
            &manifest,
            &slot,
            &marker,
            &credential[..2],
            &[3, 4, 5],
            &account_original,
            &[7, 8],
        ),
    ] {
        assert_ne!(changed, message);
        assert!(authorize_account(&owner, &changed, signature.payload()).is_err());
    }
    let (_, other, foreign_original) = existing_account(53, Algorithm::Ed25519);
    let other_message = open_message(
        &nonce,
        &manifest,
        &slot,
        &marker,
        &credential,
        &certificate,
        &foreign_original,
        &[7, 8],
    );
    assert_ne!(other_message, message);
    assert!(authorize_account(&other, &message, signature.payload()).is_err());
    assert!(authorize_account(&owner, &other_message, signature.payload()).is_err());
}

#[test]
fn account_authorization_rejects_wrong_length_mutations_and_non_ed_owner() {
    let (key, owner, _) = existing_account(59, Algorithm::Ed25519);
    let message = [61; 32];
    let signature = Signature::try_new(key.private_key(), &message).unwrap();
    assert_eq!(signature.payload().len(), 64);
    authorize_account(&owner, &message, signature.payload()).unwrap();
    assert!(authorize_account(&owner, &message, &signature.payload()[..63]).is_err());
    let mut trailing = signature.payload().to_vec();
    trailing.push(0);
    assert!(authorize_account(&owner, &message, &trailing).is_err());
    assert!(authorize_account(&owner, &message[..31], signature.payload()).is_err());
    let mut changed = signature.payload().to_vec();
    changed[0] ^= 1;
    assert!(authorize_account(&owner, &message, &changed).is_err());
    let (_, secp, _) = existing_account(67, Algorithm::Secp256k1);
    assert!(authorize_account(&secp, &message, signature.payload()).is_err());
}
