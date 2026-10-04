//! Canonical completion authorities retain the independently selected signer.

use super::*;

fn authority() -> ProviderIngestCompletionAuthorityV1 {
    let owner = AccountId::new(
        iroha_crypto::KeyPair::try_from_seed(vec![81; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let signer = AccountId::new(
        iroha_crypto::KeyPair::try_from_seed(vec![82; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    ProviderIngestCompletionAuthorityV1::new(
        owner,
        signer,
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [1; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [2; 32],
        },
    )
}

#[test]
fn dedicated_completion_signer_roundtrips_binary_and_json() {
    let value = authority();
    assert_ne!(value.provider_owner, value.completion_signer);
    assert!(value.is_valid());
    let wire = norito::to_bytes(&value).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<ProviderIngestCompletionAuthorityV1>(&wire).unwrap(),
        value
    );
    let json = norito::json::to_json(&value).unwrap();
    assert_eq!(
        norito::json::from_json::<ProviderIngestCompletionAuthorityV1>(&json).unwrap(),
        value
    );
    let mut changed = value.clone();
    changed.completion_signer = changed.provider_owner.clone();
    assert_ne!(norito::to_bytes(&changed).unwrap(), wire);
}

#[test]
fn completion_signer_json_is_required_and_cannot_be_null() {
    let value = authority();
    let json = norito::json::to_value(&value).unwrap();
    let mut omitted = json.clone();
    omitted.as_object_mut().unwrap().remove("completion_signer");
    assert!(norito::json::from_value::<ProviderIngestCompletionAuthorityV1>(omitted).is_err());
    let mut null = json;
    null.as_object_mut()
        .unwrap()
        .insert("completion_signer".into(), norito::json::Value::Null);
    assert!(norito::json::from_value::<ProviderIngestCompletionAuthorityV1>(null).is_err());
}

#[test]
fn retired_owner_only_authority_frame_has_a_different_schema() {
    #[derive(Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1"
    )]
    struct RetiredOwnerOnly {
        provider_owner: AccountId,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
    }
    let value = authority();
    let retired = RetiredOwnerOnly {
        provider_owner: value.provider_owner,
        signer_policy: value.signer_policy,
    };
    let wire = norito::to_bytes(&retired).unwrap();
    assert!(matches!(
        norito::decode_from_bytes::<ProviderIngestCompletionAuthorityV1>(&wire),
        Err(norito::Error::SchemaMismatch)
    ));
}
