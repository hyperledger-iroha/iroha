//! Real generated credentials and immutable original profile controls; no service activation.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

fn prepared(root: &Path) -> PreparedLocalnet {
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    prepare_localnet_at(
        "provider-profile",
        root,
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap()
}
fn directory(prepared: &PreparedLocalnet) -> PrivateDirectory {
    PrivateDirectory::open_exact(
        prepared
            .context
            .client_config
            .parent()
            .unwrap()
            .join(LOCALNET_RUNTIME_DIRECTORY)
            .join(DIRECTORY),
    )
    .unwrap()
}
fn publish(directory: &PrivateDirectory, manifest: &StreamTokenAuthorityManifest) {
    let bytes = norito::json::to_vec(manifest).unwrap();
    assert!(bytes.len() <= MAX_MANIFEST);
    directory
        .write_atomic(MANIFEST, &bytes, PublishMode::Replace)
        .unwrap();
}

#[test]
fn generated_provider_plan_recovers_exact_original_economics_and_interval() {
    let _resources = crate::managed::native_test_guard();
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let prepared = prepared(&temporary.path().join("generation"));
    let manifest = prepared.stream_token_authorities().unwrap().unwrap();
    let original = prepared
        .provider_service_plans()
        .map(|plans| plans.map(|[first, _, _]| first))
        .unwrap()
        .unwrap();
    assert_eq!(original.network_id(), manifest.network_id);
    assert_eq!(original.provider_id(), manifest.providers[0].provider_id);
    assert_eq!(
        original.reserve_terms().provider_account,
        manifest.providers[0].authorities[0].account
    );
    assert_eq!(original.reserve_terms().capacity_gib, 1);
    assert_eq!(original.pricing(), &PricingScheduleRecord::launch_default());
    assert_eq!(
        original.declaration().stake,
        original.admission_material().proposal.stake
    );
    assert_eq!(
        original.declaration().stake.stake_amount,
        "0.75".parse().unwrap()
    );
    let material = original.admission_material();
    assert_eq!(
        material.retention_epoch - material.issued_at,
        VALIDITY_SECONDS
    );
    let cap = policy(&material.proposal.capabilities).unwrap();
    assert!(cap.https_host.split('.').all(|label| label.len() <= 63));
    assert_ne!(cap.https_port, 0);
    assert_eq!(original.https_origin(), cap.https_origin().unwrap());
    assert!(cap.https_host.ends_with(".localhost"));
    let recovered = prepared
        .provider_service_plans()
        .map(|plans| plans.map(|[first, _, _]| first))
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.admission_material(),
        original.admission_material()
    );
    assert_eq!(recovered.declaration(), original.declaration());
    assert_eq!(recovered.pricing(), original.pricing());
    assert_eq!(recovered.https_origin(), original.https_origin());
    // The immutable original interval is intentionally not reissued on reopen.
    tls_identity::verify(
        &material.proposal.endpoints[0]
            .attestation
            .intermediate_certificates[0],
        &material.proposal.endpoints[0].attestation.leaf_certificate,
        &cap.https_host,
        material.issued_at,
    )
    .unwrap();
    assert!(
        tls_identity::verify(
            &material.proposal.endpoints[0]
                .attestation
                .intermediate_certificates[0],
            &material.proposal.endpoints[0].attestation.leaf_certificate,
            &cap.https_host,
            material.retention_epoch + 1
        )
        .is_err()
    );
}

#[test]
fn original_genesis_refuses_rewritten_plan_even_when_structurally_valid() {
    let _resources = crate::managed::native_test_guard();
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let prepared = prepared(&temporary.path().join("generation"));
    let original = prepared.stream_token_authorities().unwrap().unwrap();
    let directory = directory(&prepared);
    let plan = decode(&original.providers[0].provider_plan).unwrap();
    for which in 0..6 {
        let mut changed = original.clone();
        let mut selection = plan.clone();
        match which {
            0 => {
                let mut network =
                    network_material::NetworkServicePlanV1::decode(&changed.network.network_plan)
                        .unwrap();
                network.creation_time_ms += 1;
                changed.network.network_plan = network.bytes().unwrap();
            }
            1 => selection.material.retention_epoch += 1,
            2 => selection.declaration.stake.pool_id[0] ^= 1,
            3 => {
                let mut network =
                    network_material::NetworkServicePlanV1::decode(&changed.network.network_plan)
                        .unwrap();
                network.pricing.notes = Some("changed original pricing".into());
                changed.network.network_plan = network.bytes().unwrap();
            }
            4 => selection.attestation_journal_policy_digest[0] ^= 1,
            _ => {
                selection.material.proposal.endpoints[0]
                    .attestation
                    .leaf_certificate[0] ^= 1
            }
        }
        changed.providers[0].provider_plan = encode(&selection).unwrap();
        publish(&directory, &changed);
        assert!(
            prepared
                .provider_service_plans()
                .map(|plans| plans.map(|[first, _, _]| first))
                .is_err()
        );
    }
    publish(&directory, &original);
    assert!(
        prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .unwrap()
            .is_some()
    );
    let mut json = norito::json::to_value(&original).unwrap();
    assert!(
        json.as_object_mut()
            .unwrap()
            .get_mut("providers")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_object_mut()
            .unwrap()
            .remove("provider_plan")
            .is_some()
    );
    assert!(norito::json::from_value::<StreamTokenAuthorityManifest>(json).is_err());
    let mut malformed = original.clone();
    for bytes in [
        vec![],
        vec![0],
        vec![0; PLAN_MAX_BYTES + 1],
        [original.providers[0].provider_plan.as_slice(), &[0]].concat(),
    ] {
        malformed.providers[0].provider_plan = bytes;
        let bytes = norito::json::to_vec(&malformed).unwrap();
        directory
            .write_atomic(MANIFEST, &bytes, PublishMode::Replace)
            .unwrap();
        assert!(
            prepared
                .provider_service_plans()
                .map(|plans| plans.map(|[first, _, _]| first))
                .is_err()
        );
    }
    publish(&directory, &original);
    assert!(
        prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .unwrap()
            .is_some()
    );
}

#[test]
fn retained_provider_private_material_rejects_cross_purpose_and_certificate_substitution() {
    let _resources = crate::managed::native_test_guard();
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let prepared = prepared(&temporary.path().join("generation"));
    let root = directory(&prepared);
    let directory = open_provider_directory(&root, 0).unwrap();
    let network = root.open_child(NETWORK_DIRECTORY).unwrap();
    let original = prepared
        .provider_service_plans()
        .unwrap()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    for (target_dir, target, substitute_dir, substitute) in [
        (
            &directory,
            ADVERT_KEY,
            &network,
            network_material::COUNCIL_KEYS[0],
        ),
        (
            &network,
            network_material::COUNCIL_KEYS[0],
            &directory,
            ADVERT_KEY,
        ),
        (&directory, VRF_KEY, &directory, ADVERT_KEY),
        (
            &directory,
            tls_identity::LEAF_KEY,
            &directory,
            tls_identity::CA_KEY,
        ),
        (
            &directory,
            tls_identity::LEAF_CERT,
            &directory,
            tls_identity::CA_CERT,
        ),
    ] {
        let before = target_dir.read(target, 16 * 1024).unwrap();
        let other = substitute_dir.read(substitute, 16 * 1024).unwrap();
        target_dir
            .write_atomic(target, &other, PublishMode::Replace)
            .unwrap();
        assert!(
            prepared
                .provider_service_plan(original.provider_id())
                .is_err()
        );
        target_dir
            .write_atomic(target, &before, PublishMode::Replace)
            .unwrap();
        assert_eq!(
            prepared
                .provider_service_plan(original.provider_id())
                .unwrap()
                .unwrap()
                .admission_material(),
            original.admission_material()
        );
    }
}

#[test]
fn real_generated_tls_accepts_exact_root_name_key_and_refuses_other_identities() {
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let root = PrivateDirectory::open_exact(temporary.path()).unwrap();
    let first = root.ensure_child("first").unwrap();
    let second = root.ensure_child("second").unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut unique_tls = BTreeSet::new();
    let first_public = tls_identity::generate(
        first.path(),
        "provider.localhost",
        now,
        now + 3_600,
        &mut unique_tls,
    )
    .unwrap();
    let second_public = tls_identity::generate(
        second.path(),
        "other.localhost",
        now,
        now + 3_600,
        &mut unique_tls,
    )
    .unwrap();
    assert_eq!(unique_tls.len(), 4);
    tls_identity::verify(
        &first_public.root,
        &first_public.leaf,
        "provider.localhost",
        now,
    )
    .unwrap();
    assert!(
        tls_identity::verify(
            &second_public.root,
            &first_public.leaf,
            "provider.localhost",
            now
        )
        .is_err()
    );
    assert!(
        tls_identity::verify(
            &first_public.root,
            &first_public.leaf,
            "other.localhost",
            now
        )
        .is_err()
    );
    assert!(
        tls_identity::verify(
            &first_public.root,
            &first_public.leaf,
            "provider.localhost",
            now - 1
        )
        .is_err()
    );
    assert!(
        tls_identity::verify(
            &first_public.root,
            &first_public.leaf,
            "provider.localhost",
            now + 3_601
        )
        .is_err()
    );
    let key = first.read(tls_identity::LEAF_KEY, 16 * 1024).unwrap();
    let other_key = second.read(tls_identity::LEAF_KEY, 16 * 1024).unwrap();
    assert!(tls_identity::validate_key(&first_public.leaf, &other_key).is_err());
    tls_identity::validate_key(&first_public.leaf, &key).unwrap();
    // Exercise real Rustls TLS records without a TCP listener or machine trust-store mutation.
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(first_public.root))
        .unwrap();
    let client_config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(first_public.leaf)],
            rustls::pki_types::PrivateKeyDer::try_from(key.to_vec()).unwrap(),
        )
        .unwrap();
    let mut client = rustls::ClientConnection::new(
        std::sync::Arc::new(client_config),
        rustls::pki_types::ServerName::try_from("provider.localhost").unwrap(),
    )
    .unwrap();
    let mut server = rustls::ServerConnection::new(std::sync::Arc::new(server_config)).unwrap();
    for _ in 0..16 {
        let mut client_wire = Vec::new();
        client.write_tls(&mut client_wire).unwrap();
        server
            .read_tls(&mut std::io::Cursor::new(client_wire))
            .unwrap();
        server.process_new_packets().unwrap();
        let mut server_wire = Vec::new();
        server.write_tls(&mut server_wire).unwrap();
        client
            .read_tls(&mut std::io::Cursor::new(server_wire))
            .unwrap();
        client.process_new_packets().unwrap();
        if !client.is_handshaking() && !server.is_handshaking() {
            break;
        }
    }
    assert!(!client.is_handshaking() && !server.is_handshaking());
    assert_eq!(
        client.negotiated_cipher_suite(),
        server.negotiated_cipher_suite()
    );
}
