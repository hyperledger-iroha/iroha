//! Exact original three-provider topology, public selection and private cross-scope custody.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

fn fixture() -> (tempfile::TempDir, PreparedLocalnet) {
    let temp = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "three-provider-profile",
        &temp.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    (temp, prepared)
}

#[test]
fn three_providers_share_only_network_policy_and_require_explicit_original_selection() {
    let _resources = crate::managed::native_test_guard();
    let (_temp, prepared) = fixture();
    assert_eq!(prepared.peers.len(), 4);
    let manifest = prepared.stream_token_authorities().unwrap().unwrap();
    let plans = prepared.provider_service_plans().unwrap().unwrap();
    let owned = ServiceProfileCommitmentV1 {
        manager: manifest.manager.clone(),
        network: manifest.network.clone(),
        providers: manifest.providers.clone(),
    };
    let owned_frame = norito::encode_canonical(&owned).unwrap();
    let borrowed = ServiceProfileCommitmentRef {
        manager: ProfileValue(&manifest.manager),
        network: ProfileValue(&manifest.network),
        providers: ProfileValue(&manifest.providers),
    };
    assert_eq!(norito::encode_canonical(&borrowed).unwrap(), owned_frame);
    let decoded: ServiceProfileCommitmentV1 = norito::decode_canonical_with_limits(
        &owned_frame,
        norito::DecodeLimits::new(
            MAX_PROFILE_BYTES,
            MAX_PROFILE_BYTES,
            MAX_PROFILE_BYTES * 2,
            MAX_PROFILE_BYTES * 8,
            32,
        ),
    )
    .unwrap();
    assert_eq!(decoded.manager, manifest.manager);
    assert_eq!(decoded.network, manifest.network);
    assert_eq!(decoded.providers, manifest.providers);
    assert_eq!(
        Hash::new(owned_frame),
        profile_commitment(&manifest.manager, &manifest.network, &manifest.providers).unwrap()
    );
    let mut accounts = BTreeSet::from([&manifest.manager]);
    for authority in &manifest.network.authorities {
        assert!(accounts.insert(&authority.account));
    }
    let mut provider_ids = BTreeSet::new();
    let mut origins = BTreeSet::new();
    let mut gateways = BTreeSet::new();
    let mut compliance_keys = BTreeSet::new();
    let original = plans[0].original_profile_commitment();
    for (slot, (inventory, plan)) in manifest.providers.iter().zip(&plans).enumerate() {
        assert_eq!(plan.slot(), u8::try_from(slot).unwrap());
        assert_eq!(plan.peer_index(), slot);
        assert_eq!(plan.provider_id(), inventory.provider_id);
        assert_eq!(manifest.provider(inventory.provider_id).unwrap(), inventory);
        assert!(provider_ids.insert(plan.provider_id()));
        assert!(origins.insert(plan.https_origin()));
        assert_eq!(plan.original_profile_commitment(), original);
        assert_eq!(plan.pricing(), plans[0].pricing());
        assert_eq!(
            plan.admission_material().issued_at,
            plans[0].admission_material().issued_at
        );
        assert_eq!(
            plan.admission_material().retention_epoch,
            plans[0].admission_material().retention_epoch
        );
        for role in &inventory.authorities {
            assert!(accounts.insert(&role.account));
        }
        let selected = prepared
            .provider_service_plan(plan.provider_id())
            .unwrap()
            .unwrap();
        assert_eq!(selected.admission_material(), plan.admission_material());
        assert_eq!(selected.declaration(), plan.declaration());
        let compliance = prepared
            .gateway_compliance_plan(plan.provider_id())
            .unwrap()
            .unwrap();
        assert_eq!(
            compliance.gateway_label(),
            format!("managed-provider-gateway-{slot}")
        );
        assert!(gateways.insert(compliance.gateway_id()));
        for signer in compliance
            .trust_policy()
            .catalog_signers
            .iter()
            .chain(&compliance.trust_policy().gateway_signers)
        {
            assert!(compliance_keys.insert(signer.public_key));
        }
        let advert = prepared
            .provider_advert(plan.provider_id(), plan.admission_material().issued_at)
            .unwrap();
        advert.verify_signature().unwrap();
        assert_eq!(&advert.body, &plan.admission_material().advert_body);
    }
    assert_eq!(accounts.len(), 33); // manager + two network + three times ten provider roles
    assert_eq!(compliance_keys.len(), 12);
    let absent = ProviderId::new([0xFD; 32]);
    assert!(!provider_ids.contains(&absent));
    assert!(manifest.provider(absent).is_err());
    assert!(prepared.provider_service_plan(absent).is_err());
    assert!(prepared.gateway_compliance_plan(absent).is_err());
    assert!(
        prepared
            .provider_advert(absent, plans[0].admission_material().issued_at)
            .is_err()
    );
}

#[test]
fn topology_reordering_duplicates_and_network_rebinding_cannot_replace_original_genesis() {
    let _resources = crate::managed::native_test_guard();
    let (_temp, prepared) = fixture();
    let original = prepared.stream_token_authorities().unwrap().unwrap();
    let directory = PrivateDirectory::open_exact(
        prepared
            .context
            .client_config
            .parent()
            .unwrap()
            .join(LOCALNET_RUNTIME_DIRECTORY)
            .join(DIRECTORY),
    )
    .unwrap();
    let original_bytes = directory.read(MANIFEST, MAX_MANIFEST).unwrap();
    for mutation in 0..7 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.providers.swap(0, 1),
            1 => changed.providers[1].provider_id = changed.providers[0].provider_id,
            2 => changed.providers[1].slot = 0,
            3 => {
                changed.providers[2].authorities[0].account =
                    changed.providers[0].authorities[0].account.clone()
            }
            4 => {
                changed.network.authorities[0].account =
                    changed.providers[0].authorities[0].account.clone()
            }
            5 => changed.network.authorities.swap(0, 1),
            _ => {
                let mut network =
                    network_material::NetworkServicePlanV1::decode(&changed.network.network_plan)
                        .unwrap();
                network.council.trusted_signers.swap(0, 1);
                changed.network.network_plan = network.bytes().unwrap();
            }
        }
        directory
            .write_atomic(
                MANIFEST,
                &norito::json::to_vec(&changed).unwrap(),
                PublishMode::Replace,
            )
            .unwrap();
        assert!(
            prepared.stream_token_authorities().is_err(),
            "mutation {mutation}"
        );
        assert!(
            prepared
                .provider_service_plan(original.providers[0].provider_id)
                .is_err()
        );
        directory
            .write_atomic(MANIFEST, &original_bytes, PublishMode::Replace)
            .unwrap();
    }
    for field in ["network", "providers"] {
        let mut missing = norito::json::to_value(&original).unwrap();
        assert!(missing.as_object_mut().unwrap().remove(field).is_some());
        assert!(norito::json::from_value::<StreamTokenAuthorityManifest>(missing).is_err());
    }
    let mut short = norito::json::to_value(&original).unwrap();
    short
        .as_object_mut()
        .unwrap()
        .get_mut("providers")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(norito::json::from_value::<StreamTokenAuthorityManifest>(short).is_err());
    let mut old = norito::json::to_value(&original).unwrap();
    old.as_object_mut().unwrap().insert(
        "provider_id".into(),
        norito::json::to_value(&original.providers[0].provider_id).unwrap(),
    );
    assert!(norito::json::from_value::<StreamTokenAuthorityManifest>(old).is_err());
    assert_eq!(
        prepared.stream_token_authorities().unwrap().unwrap(),
        original
    );
    assert_eq!(
        directory.read(MANIFEST, MAX_MANIFEST).unwrap(),
        original_bytes
    );
}

#[test]
fn provider_credentials_cannot_cross_another_original_slot_or_network_purpose() {
    let _resources = crate::managed::native_test_guard();
    let (_temp, prepared) = fixture();
    let original = prepared.stream_token_authorities().unwrap().unwrap();
    let root = PrivateDirectory::open_exact(
        prepared
            .context
            .client_config
            .parent()
            .unwrap()
            .join(LOCALNET_RUNTIME_DIRECTORY)
            .join(DIRECTORY),
    )
    .unwrap();
    let first = open_provider_directory(&root, 0).unwrap();
    let second = open_provider_directory(&root, 1).unwrap();
    let network = root.open_child(NETWORK_DIRECTORY).unwrap();
    for (destination, name, source, other) in [
        (
            &first,
            "provider-ingest.key",
            &second,
            "provider-ingest.key",
        ),
        (
            &first,
            "provider-advert.key",
            &second,
            "provider-advert.key",
        ),
        (
            &first,
            "compliance-gateway.key",
            &second,
            "compliance-gateway.key",
        ),
        (
            &first,
            "provider-tls.key.der",
            &second,
            "provider-tls.key.der",
        ),
        (
            &network,
            "reserve-operations.key",
            &first,
            "issuer-operator.key",
        ),
    ] {
        let before = destination.read(name, 16 * 1024).unwrap();
        let substitute = source.read(other, 16 * 1024).unwrap();
        destination
            .write_atomic(name, &substitute, PublishMode::Replace)
            .unwrap();
        assert!(prepared.stream_token_authorities().is_err());
        destination
            .write_atomic(name, &before, PublishMode::Replace)
            .unwrap();
    }
    assert_eq!(
        prepared.stream_token_authorities().unwrap().unwrap(),
        original
    );
}
