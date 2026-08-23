// Same-scope validator qualification regression coverage.
const VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID: &str = "fixture.authority.v1";

#[test]
fn catalog_revalidation_signature_payload_matches_python_golden() {
    assert!(valid_catalog_revalidation_key_id_v1("python.authority-v1"));
    assert!(!valid_catalog_revalidation_key_id_v1(".python-authority"));
    assert!(!valid_catalog_revalidation_key_id_v1("python:authority"));
    // Produced with Python 3 json.dumps(sort_keys=True, separators=(",", ":"),
    // ensure_ascii=True, allow_nan=False) and PyNaCl 1.5.0 from seed 00..1f.
    let receipt: norito::json::Value = norito::json::from_str(
        r#"{
            "catalog_sha256":"938883be545353d9d133a37a4c5490ac0c3522173f9fef9488c99006524fdd12",
            "expires_at_unix_ms":1700000300000,
            "issued_at_unix_ms":1700000000000,
            "promotion_id":"4444444444444444444444444444444444444444444444444444444444444444",
            "receipt_id":"5555555555555555555555555555555555555555555555555555555555555555",
            "release_statuses":[{
                "app_attest_key_id":"python-fixture-app-attest-key",
                "apple_status":"good",
                "apple_status_checked_at_unix_ms":1700000000000,
                "apple_status_source":"apple-app-attest-online-status-authority-v1",
                "consumption_receipt_sha256":"3333333333333333333333333333333333333333333333333333333333333333",
                "evidence_sha256":"2222222222222222222222222222222222222222222222222222222222222222",
                "refreshed_apple_receipt_sha256":"6666666666666666666666666666666666666666666666666666666666666666",
                "release_manifest_sha256":"1111111111111111111111111111111111111111111111111111111111111111",
                "risk_metric":0
            }],
            "schema":"iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1",
            "signature":"76cdc21a9eaeb17dd5fbc5f2f23777c4b6caf9c31e2b12508cbb8b6771c1a2631c3ac9bebc7427f726c4a585fa8785d8413842d85133b5930be9186111bd0706",
            "signature_algorithm":"ed25519",
            "signature_payload_sha256":"533318aa2f61f2bc77cc60ba8a3e98b9b6535051147e69f8781c040e6450348f",
            "signer_key_id":"python.authority-v1",
            "signer_public_key_sha256":"a050837d85070582ccf7394b0988847cc312cb88259b894899f6f239cf1791a5",
            "status":"catalog-revalidated-for-one-promotion",
            "version":1
        }"#,
    )
    .expect("decode Python-produced catalog-revalidation receipt");
    let norito::json::Value::Object(object) = receipt else {
        unreachable!("golden catalog-revalidation receipt is an object");
    };
    let payload = catalog_revalidation_signature_payload_v1(&object)
        .expect("encode Python-compatible signature payload");
    assert_eq!(
        hex::encode(Sha256::digest(&payload)),
        "533318aa2f61f2bc77cc60ba8a3e98b9b6535051147e69f8781c040e6450348f"
    );
    let public_key_bytes =
        hex::decode("03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8")
            .expect("decode Python authority public key");
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &public_key_bytes)
        .expect("parse Python authority public key");
    assert_eq!(
        hex::encode(
            catalog_revalidation_authority_spki_sha256_v1(&public_key)
                .expect("hash Python authority SPKI")
        ),
        "a050837d85070582ccf7394b0988847cc312cb88259b894899f6f239cf1791a5"
    );
    validate_catalog_revalidation_authority_v1(
        &object,
        &payload,
        "python.authority-v1",
        &public_key,
    )
    .expect("verify Python-produced Ed25519 catalog-revalidation signature");
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_capture_retains_catalog_after_validation_clone() {
    let policy_path = Path::new("/sealed-fixture/policy.norito");
    let artifact_dir = Path::new("/sealed-fixture/artifacts");
    let capture = KagemushaValidatorQualificationCatalogCaptureV1 {
        catalog: KagemushaReleaseCatalogV4::empty(),
        seal: qualification_seal_fixture(policy_path, artifact_dir),
        policy_path: policy_path.to_owned(),
        artifact_dir: artifact_dir.to_owned(),
    };
    let validation_catalog = capture.catalog_for_validation();
    assert!(validation_catalog.is_empty());
    assert!(capture.catalog.is_empty());
    assert_eq!(
        capture.catalog_qualification_seal().canonical_policy_path,
        canonical_catalog_path_string_v1(policy_path, "fixture policy path")
            .expect("canonical fixture policy path")
    );
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn validator_qualification_fixture() -> (
    iroha_genesis::GenesisBlock,
    Vec<KeyPair>,
    Vec<u8>,
    KagemushaAuthenticatedReleaseV4,
    iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    KagemushaCatalogQualificationSealV1,
    KagemushaV4ValidatorQualificationSubjectV1,
    [u8; 32],
) {
    let validators = (0_u8..4)
        .map(|index| KeyPair::from_seed(vec![0xB0 + index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let topology = validators
        .iter()
        .map(|validator| {
            let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
                .expect("validator qualification PoP");
            iroha_genesis::GenesisTopologyEntry::new(
                iroha_data_model::peer::PeerId::new(validator.public_key().clone()),
                pop,
            )
        })
        .collect::<Vec<_>>();
    let genesis_signer = KeyPair::from_seed(vec![0xA4; 32], Algorithm::Ed25519);
    let genesis = iroha_genesis::GenesisBuilder::new_without_executor(
        iroha_data_model::ChainId::from("validator-qualification-fixture"),
        ".",
    )
    .set_topology(topology)
    .build_raw()
    .with_consensus_meta()
    .build_and_sign(&genesis_signer)
    .expect("signed validator qualification genesis");
    let genesis_bytes = genesis
        .0
        .encode_wire()
        .expect("canonical validator qualification genesis");
    let network_id = NetworkId::from_genesis_hash(genesis.0.hash());
    let (authenticated, promotion, release_record) =
        authenticated_candidate_binding_release_for_network(network_id);
    let seal = qualification_seal_fixture_for_release(
        Path::new("/sealed-fixture/policy.norito"),
        Path::new("/sealed-fixture/artifacts"),
        &authenticated,
        &promotion,
    );
    let policy =
        crate::smartcontracts::isi::offline::isi::production_offline_device_attestation_policy_v1(
            "TEAMID1234".to_owned(),
            "io.soramitsu.pk".to_owned(),
            vec![4, 10],
            vec!["41".to_owned(), "42".to_owned()],
            "com.pk.retailwallet".to_owned(),
            vec![[0x55; 32], [0x66; 32]],
            1_800_000_000_000,
        )
        .expect("production validator qualification policy");
    let controller = KeyPair::from_seed(vec![0xC6; 32], Algorithm::Ed25519);
    let reservation_identity = iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(
        b"fixture controller-signed promotion reservation",
    )
    .expect("fixture reservation identity");
    let subject = KagemushaV4ValidatorQualificationSubjectV1::try_new(
        controller.public_key().clone(),
        reservation_identity,
        [0xD4; 32],
        authenticated.manifest_sha256(),
        policy,
        1_800_000_000_000,
        1_800_000_000_000,
        1_800_000_300_000,
    )
    .expect("trusted validator qualification subject");
    let release_record_bytes =
        norito::encode_canonical(&release_record).expect("canonical release record");
    let catalog_digest = kagemusha_catalog_consensus_policy_digest_from_identities_v1(
        authenticated.release_policy_sha256(),
        vec![KagemushaCatalogReleaseConsensusIdentityV1 {
            manifest_sha256: authenticated.manifest_sha256(),
            release_record_sha256: Sha256::digest(release_record_bytes).into(),
            qualification_receipt_sha256: authenticated.manifest().qualification_receipt_sha256,
            qualified_candidate_sha256: authenticated.manifest().qualified_candidate_sha256,
        }],
    );
    (
        genesis,
        validators,
        genesis_bytes,
        authenticated,
        release_record,
        seal,
        subject,
        catalog_digest,
    )
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn runtime_effective_config_fixture(
    genesis: &iroha_genesis::GenesisBlock,
    validators: &[KeyPair],
) -> KagemushaV4RuntimeEffectiveConfigProjectionV1 {
    let metadata = exact_genesis_consensus_metadata_v1(genesis).expect("signed consensus metadata");
    let projected_validators = crate::sumeragi::signed_genesis_validator_pops(genesis)
        .expect("signed genesis voters and PoPs")
        .into_iter()
        .enumerate()
        .map(|(index, (validator_id, bls_pop))| {
            assert!(
                validators
                    .iter()
                    .any(|key| key.public_key() == validator_id.public_key()),
                "fixture voter key"
            );
            iroha_data_model::offline::KagemushaV4RuntimeValidatorProjectionV1 {
                validator_id,
                public_address: format!("127.0.0.1:{}", 16_000 + index)
                    .parse()
                    .expect("fixture validator address"),
                bls_pop,
            }
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("exactly four validator projections");
    let genesis_public_key = genesis
        .0
        .external_transactions()
        .next()
        .and_then(|transaction| transaction.authority().try_signatory())
        .expect("single-key genesis authority")
        .clone();
    let projection = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
        chain: iroha_data_model::ChainId::from("validator-qualification-fixture"),
        chain_discriminant: 42,
        is_validator: true,
        genesis_public_key,
        genesis_expected_hash: genesis.0.hash(),
        validators: projected_validators,
        sumeragi_config_fingerprint: Hash::new(b"effective Sumeragi V2 fixture"),
        genesis_context: metadata.sumeragi_v2,
        kagemusha_max_decoded_bytes: 64 * 1024 * 1024,
    };
    projection
        .validate()
        .expect("valid runtime-effective config fixture");
    projection
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn validator_catalog_revalidation_receipt_fixture(
    promotion_id: [u8; 32],
    manifest_sha256: [u8; 32],
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    authority: &KeyPair,
) -> (Vec<u8>, [u8; 32]) {
    fn digest_text(digest: [u8; 32]) -> norito::json::Value {
        norito::json::Value::String(hex::encode(digest))
    }

    let evidence_sha256 = [0x81; 32];
    let consumption_receipt_sha256 = [0x82; 32];
    let mut status = norito::json::Map::new();
    status.insert(
        "app_attest_key_id".to_owned(),
        norito::json::Value::String("fixture-app-attest-key".to_owned()),
    );
    status.insert(
        "apple_status".to_owned(),
        norito::json::Value::String("good".to_owned()),
    );
    status.insert(
        "apple_status_checked_at_unix_ms".to_owned(),
        norito::json::Value::from(issued_at_unix_ms),
    );
    status.insert(
        "apple_status_source".to_owned(),
        norito::json::Value::String(
            KAGEMUSHA_V4_CATALOG_REVALIDATION_APPLE_STATUS_SOURCE_V1.to_owned(),
        ),
    );
    status.insert(
        "consumption_receipt_sha256".to_owned(),
        digest_text(consumption_receipt_sha256),
    );
    status.insert("evidence_sha256".to_owned(), digest_text(evidence_sha256));
    status.insert(
        "refreshed_apple_receipt_sha256".to_owned(),
        digest_text([0x83; 32]),
    );
    status.insert(
        "release_manifest_sha256".to_owned(),
        digest_text(manifest_sha256),
    );
    status.insert("risk_metric".to_owned(), norito::json::Value::from(0_u64));

    let mut binding = norito::json::Map::new();
    binding.insert(
        "consumption_receipt_sha256".to_owned(),
        digest_text(consumption_receipt_sha256),
    );
    binding.insert("evidence_sha256".to_owned(), digest_text(evidence_sha256));
    binding.insert(
        "release_manifest_sha256".to_owned(),
        digest_text(manifest_sha256),
    );
    let mut catalog = norito::json::Map::new();
    catalog.insert(
        "releases".to_owned(),
        norito::json::Value::Array(vec![norito::json::Value::Object(binding)]),
    );
    catalog.insert(
        "schema".to_owned(),
        norito::json::Value::String(KAGEMUSHA_V4_CATALOG_REVALIDATION_BINDING_SCHEMA_V1.to_owned()),
    );
    catalog.insert("version".to_owned(), norito::json::Value::from(1_u64));
    let catalog_sha256: [u8; 32] = Sha256::digest(
        canonical_json_bytes_v1(&norito::json::Value::Object(catalog))
            .expect("canonical fixture catalog binding"),
    )
    .into();

    let mut receipt = norito::json::Map::new();
    receipt.insert("catalog_sha256".to_owned(), digest_text(catalog_sha256));
    receipt.insert(
        "expires_at_unix_ms".to_owned(),
        norito::json::Value::from(expires_at_unix_ms),
    );
    receipt.insert(
        "issued_at_unix_ms".to_owned(),
        norito::json::Value::from(issued_at_unix_ms),
    );
    receipt.insert("promotion_id".to_owned(), digest_text(promotion_id));
    receipt.insert("receipt_id".to_owned(), digest_text([0x84; 32]));
    receipt.insert(
        "release_statuses".to_owned(),
        norito::json::Value::Array(vec![norito::json::Value::Object(status)]),
    );
    receipt.insert(
        "schema".to_owned(),
        norito::json::Value::String(KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_SCHEMA_V1.to_owned()),
    );
    receipt.insert(
        "signature_algorithm".to_owned(),
        norito::json::Value::String("ed25519".to_owned()),
    );
    receipt.insert(
        "signer_key_id".to_owned(),
        norito::json::Value::String(VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID.to_owned()),
    );
    receipt.insert(
        "signer_public_key_sha256".to_owned(),
        digest_text(
            catalog_revalidation_authority_spki_sha256_v1(authority.public_key())
                .expect("fixture authority SPKI digest"),
        ),
    );
    receipt.insert(
        "status".to_owned(),
        norito::json::Value::String(KAGEMUSHA_V4_CATALOG_REVALIDATION_STATUS_V1.to_owned()),
    );
    receipt.insert("version".to_owned(), norito::json::Value::from(1_u64));
    let signature_payload = norito::json::to_string(&norito::json::Value::Object(receipt.clone()))
        .expect("canonical fixture signature payload");
    receipt.insert(
        "signature_payload_sha256".to_owned(),
        digest_text(Sha256::digest(signature_payload.as_bytes()).into()),
    );
    receipt.insert(
        "signature".to_owned(),
        norito::json::Value::String(hex::encode(
            iroha_crypto::Signature::try_new(authority.private_key(), signature_payload.as_bytes())
                .expect("fixture authority signature")
                .payload(),
        )),
    );
    let bytes = canonical_json_bytes_v1(&norito::json::Value::Object(receipt))
        .expect("canonical fixture catalog-revalidation receipt");
    (bytes, catalog_sha256)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn refresh_catalog_receipt_fixture_signature_metadata(
    value: &mut norito::json::Value,
    authority: &KeyPair,
) -> Vec<u8> {
    let norito::json::Value::Object(object) = value else {
        panic!("fixture catalog receipt must be an object");
    };
    object.remove("signature");
    object.remove("signature_payload_sha256");
    let signature_payload = norito::json::to_string(&norito::json::Value::Object(object.clone()))
        .expect("canonical mutated fixture signature payload");
    object.insert(
        "signature_payload_sha256".to_owned(),
        norito::json::Value::String(hex::encode(<[u8; 32]>::from(Sha256::digest(
            signature_payload.as_bytes(),
        )))),
    );
    object.insert(
        "signature".to_owned(),
        norito::json::Value::String(hex::encode(
            iroha_crypto::Signature::try_new(authority.private_key(), signature_payload.as_bytes())
                .expect("mutated fixture authority signature")
                .payload(),
        )),
    );
    canonical_json_bytes_v1(value).expect("canonical mutated catalog receipt")
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[allow(clippy::too_many_arguments)]
fn validator_qualification_reservation_fixture(
    genesis: &iroha_genesis::GenesisBlock,
    genesis_bytes: &[u8],
    authenticated: &KagemushaAuthenticatedReleaseV4,
    release_record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    seal: &KagemushaCatalogQualificationSealV1,
    device_policy: &iroha_data_model::offline::OfflineDeviceAttestationPolicy,
    catalog_digest: [u8; 32],
    validator_id: &iroha_data_model::peer::PeerId,
) -> (
    iroha_data_model::offline::KagemushaV4PromotionReservationV1,
    KeyPair,
    Vec<u8>,
    KeyPair,
) {
    use iroha_data_model::offline::{
        KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION, KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA,
        KagemushaExactBytesDigestV1, KagemushaV4GitHubPromotionRunV1,
        KagemushaV4PromotionReservationBodyV1, KagemushaV4PromotionReservationV1,
    };

    let controller = KeyPair::from_seed(vec![0xC7; 32], Algorithm::Ed25519);
    let github_run = KagemushaV4GitHubPromotionRunV1 {
        repository: "hyperledger/iroha".to_owned(),
        workflow_ref: ".github/workflows/kagemusha.yml@refs/heads/main".to_owned(),
        workflow_sha: [0x71; 20],
        run_id: 7_001,
        run_attempt: 1,
    };
    let promotion_record_bytes = norito::encode_canonical(&release_record.promotion_record)
        .expect("canonical reservation promotion record");
    let release_record_bytes =
        norito::encode_canonical(release_record).expect("canonical reservation release record");
    let source_descriptor = authenticated
        .manifest()
        .reviewed_source_closure
        .canonical_descriptor_bytes()
        .expect("canonical reservation source descriptor");
    let release_policy_source = exact_sealed_file_identity_v1(
        seal,
        &seal.canonical_policy_path,
        seal.configured_policy_sha256,
        "fixture release policy",
    )
    .expect("sealed fixture release-policy identity");
    let catalog_revalidation_authority = KeyPair::from_seed(vec![0xC8; 32], Algorithm::Ed25519);
    let (catalog_revalidation_receipt_bytes, catalog_revalidation_catalog_sha256) =
        validator_catalog_revalidation_receipt_fixture(
            github_run.promotion_id(),
            authenticated.manifest_sha256(),
            1_800_000_000_000,
            1_800_000_300_000,
            &catalog_revalidation_authority,
        );
    let catalog_revalidation_receipt_json =
        KagemushaExactBytesDigestV1::from_bytes(&catalog_revalidation_receipt_bytes)
            .expect("promotion-scoped catalog-revalidation receipt identity");
    assert_ne!(
        catalog_revalidation_receipt_json.sha256,
        authenticated.manifest().qualification_receipt_sha256,
        "the promotion-scoped JSON receipt is not the recursive proof receipt",
    );
    let (network_id, execution_policy_hash) =
        exact_genesis_qualification_identity_v1(genesis, genesis_bytes, validator_id)
            .expect("fixture genesis qualification identity");
    let body = KagemushaV4PromotionReservationBodyV1 {
        schema: KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        promotion_controller: controller.public_key().clone(),
        promotion_id: github_run.promotion_id(),
        github_run,
        network_id,
        reviewed_source_closure_descriptor: KagemushaExactBytesDigestV1::from_bytes(
            &source_descriptor,
        )
        .expect("source descriptor identity"),
        manifest_sha256: authenticated.manifest_sha256(),
        release_record_sha256: Sha256::digest(release_record_bytes).into(),
        promotion_record_norito: KagemushaExactBytesDigestV1::from_bytes(&promotion_record_bytes)
            .expect("promotion-record identity"),
        release_policy_source,
        signed_genesis: KagemushaExactBytesDigestV1::from_bytes(genesis_bytes)
            .expect("signed-genesis identity"),
        catalog_revalidation_receipt_json,
        catalog_revalidation_catalog_sha256,
        catalog_consensus_policy_digest: catalog_digest,
        execution_policy_hash,
        device_attestation_policy: device_policy.clone(),
        policy_evaluation_time_ms: 1_800_000_000_000,
        validator_qualification_expires_at_unix_ms: 1_800_000_300_000,
    };
    let reservation = KagemushaV4PromotionReservationV1::try_sign(body, &controller)
        .expect("signed validator qualification reservation");
    (
        reservation,
        controller,
        catalog_revalidation_receipt_bytes,
        catalog_revalidation_authority,
    )
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_reservation_binds_all_same_load_provenance() {
    let (genesis, validators, genesis_bytes, authenticated, release_record, seal, subject, digest) =
        validator_qualification_fixture();
    let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());
    let (reservation, controller, catalog_receipt, catalog_authority) =
        validator_qualification_reservation_fixture(
            &genesis,
            &genesis_bytes,
            &authenticated,
            &release_record,
            &seal,
            subject.device_attestation_policy(),
            digest,
            &validator_id,
        );
    reservation
        .verify(controller.public_key())
        .expect("controller-authenticated reservation");
    let reservation_bytes =
        norito::encode_canonical(&reservation).expect("canonical reservation bytes");
    let (reservation_identity, receipt_facts) = validate_exact_kagemusha_promotion_sources_v1(
        &reservation,
        &reservation_bytes,
        &catalog_receipt,
        VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
        catalog_authority.public_key(),
    )
    .expect("same-read reservation and promotion-scoped receipt");
    assert!(reservation_identity.matches_bytes(&reservation_bytes));
    assert_eq!(
        receipt_facts.release_manifest_sha256,
        vec![authenticated.manifest_sha256()]
    );
    let mut changed_reservation_bytes = reservation_bytes.clone();
    changed_reservation_bytes.push(0);
    assert!(
        validate_exact_kagemusha_promotion_sources_v1(
            &reservation,
            &changed_reservation_bytes,
            &catalog_receipt,
            VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
            catalog_authority.public_key(),
        )
        .is_err()
    );
    assert!(
        validate_exact_kagemusha_promotion_sources_v1(
            &reservation,
            &reservation_bytes,
            b"substituted catalog receipt",
            VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
            catalog_authority.public_key(),
        )
        .is_err()
    );
    let validate = |candidate: &iroha_data_model::offline::KagemushaV4PromotionReservationV1,
                    exact_candidate_bytes: &[u8]| {
        let decoded = iroha_data_model::offline::KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            exact_candidate_bytes,
            controller.public_key(),
        )
        .map_err(|error| format!("invalid controller reservation: {error}"))?;
        if &decoded != candidate {
            return Err("decoded reservation differs from the candidate".to_owned());
        }
        validate_exact_kagemusha_promotion_sources_v1(
            candidate,
            exact_candidate_bytes,
            &catalog_receipt,
            VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
            catalog_authority.public_key(),
        )?;
        validate_kagemusha_promotion_reservation_against_verified_release_v1(
            candidate,
            &seal,
            &authenticated,
            &release_record,
            digest,
            &genesis,
            &genesis_bytes,
            &validator_id,
        )
    };
    validate(&reservation, &reservation_bytes).expect("exact reservation evidence");

    for mutate in 0_u8..9 {
        let mut changed_body = reservation.body.clone();
        match mutate {
            0 => changed_body.reviewed_source_closure_descriptor.byte_len += 1,
            1 => changed_body.release_record_sha256[0] ^= 1,
            2 => changed_body.promotion_record_norito.sha256[0] ^= 1,
            3 => changed_body.release_policy_source.byte_len += 1,
            4 => changed_body.signed_genesis.byte_len += 1,
            5 => changed_body.catalog_revalidation_receipt_json.byte_len += 1,
            6 => changed_body.catalog_revalidation_catalog_sha256[0] ^= 1,
            7 => changed_body.catalog_consensus_policy_digest[0] ^= 1,
            8 => changed_body.execution_policy_hash = Hash::new(b"different execution policy"),
            _ => unreachable!(),
        }
        let changed = iroha_data_model::offline::KagemushaV4PromotionReservationV1::try_sign(
            changed_body,
            &controller,
        )
        .expect("controller can re-sign a structurally valid hostile reservation");
        let changed_bytes =
            norito::encode_canonical(&changed).expect("canonical re-signed hostile reservation");
        assert!(
            validate(&changed, &changed_bytes).is_err(),
            "re-signed reservation provenance mutation {mutate} must fail closed"
        );
    }

    let reservation_subject = KagemushaV4ValidatorQualificationSubjectV1::try_new(
        controller.public_key().clone(),
        iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(
            &norito::encode_canonical(&reservation).expect("canonical reservation"),
        )
        .expect("reservation identity"),
        reservation.body.promotion_id,
        reservation.body.manifest_sha256,
        reservation.body.device_attestation_policy.clone(),
        reservation.body.policy_evaluation_time_ms,
        receipt_facts.issued_at_unix_ms,
        reservation.body.validator_qualification_expires_at_unix_ms,
    )
    .expect("reservation-backed qualification subject");
    let signed = build_and_sign_validator_qualification_from_verified_release_v1(
        &seal,
        &authenticated,
        &release_record,
        authenticated.manifest().qualification_receipt_sha256,
        digest,
        &reservation_subject,
        &genesis,
        &genesis_bytes,
        b"exact config",
        &runtime_effective_config_fixture(&genesis, &validators),
        &validator_id,
        &validators[0],
    )
    .expect("reservation-backed validator qualification");
    validate_validator_qualification_matches_reservation_v1(&signed, &reservation)
        .expect("signed qualification preserves reservation binding");
    let mut spliced = reservation.clone();
    spliced.body.network_id = NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"different network")),
    );
    assert!(
        validate_validator_qualification_matches_reservation_v1(&signed, &spliced).is_err(),
        "a reservation/network splice must fail closed"
    );
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_strictly_parses_revalidation_receipt_semantics() {
    let (genesis, validators, genesis_bytes, authenticated, release_record, seal, subject, digest) =
        validator_qualification_fixture();
    let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());
    let (reservation, controller, catalog_receipt, catalog_authority) =
        validator_qualification_reservation_fixture(
            &genesis,
            &genesis_bytes,
            &authenticated,
            &release_record,
            &seal,
            subject.device_attestation_policy(),
            digest,
            &validator_id,
        );
    let canonical_reservation =
        norito::encode_canonical(&reservation).expect("canonical baseline reservation");
    validate_exact_kagemusha_promotion_sources_v1(
        &reservation,
        &canonical_reservation,
        &catalog_receipt,
        VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
        catalog_authority.public_key(),
    )
    .expect("baseline strict catalog receipt");

    let wrong_authority = KeyPair::from_seed(vec![0xC9; 32], Algorithm::Ed25519);
    assert!(
        validate_exact_kagemusha_promotion_sources_v1(
            &reservation,
            &canonical_reservation,
            &catalog_receipt,
            VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
            wrong_authority.public_key(),
        )
        .is_err(),
        "a different configured authority key must fail closed"
    );

    let mut wrong_key_claim: norito::json::Value =
        norito::json::from_slice(&catalog_receipt).expect("fixture catalog receipt JSON");
    let norito::json::Value::Object(wrong_key_claim_object) = &mut wrong_key_claim else {
        unreachable!("fixture catalog receipt is an object");
    };
    wrong_key_claim_object.insert(
        "signer_public_key_sha256".to_owned(),
        norito::json::Value::String(hex::encode(
            catalog_revalidation_authority_spki_sha256_v1(wrong_authority.public_key())
                .expect("wrong authority SPKI digest"),
        )),
    );
    let wrong_key_claim = refresh_catalog_receipt_fixture_signature_metadata(
        &mut wrong_key_claim,
        &catalog_authority,
    );
    let mut wrong_key_body = reservation.body.clone();
    wrong_key_body.catalog_revalidation_receipt_json =
        iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(&wrong_key_claim)
            .expect("wrong-key receipt identity");
    let wrong_key_reservation =
        iroha_data_model::offline::KagemushaV4PromotionReservationV1::try_sign(
            wrong_key_body,
            &controller,
        )
        .expect("controller can bind the hostile wrong-key receipt");
    let wrong_key_reservation_bytes =
        norito::encode_canonical(&wrong_key_reservation).expect("canonical wrong-key reservation");
    let wrong_key_error = validate_exact_kagemusha_promotion_sources_v1(
        &wrong_key_reservation,
        &wrong_key_reservation_bytes,
        &wrong_key_claim,
        VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
        wrong_authority.public_key(),
    )
    .expect_err("a receipt signed by another key must fail after its claimed digest matches");
    assert!(wrong_key_error.contains("authority signature"));

    for mutation in 0_u8..10 {
        let mut receipt: norito::json::Value =
            norito::json::from_slice(&catalog_receipt).expect("fixture catalog receipt JSON");
        let norito::json::Value::Object(object) = &mut receipt else {
            unreachable!("fixture catalog receipt is an object");
        };
        match mutation {
            0 => {
                object.insert(
                    "unreserved".to_owned(),
                    norito::json::Value::String("field".to_owned()),
                );
            }
            1 => {
                object.insert(
                    "status".to_owned(),
                    norito::json::Value::String("catalog-not-revalidated".to_owned()),
                );
            }
            2 => {
                object.insert(
                    "promotion_id".to_owned(),
                    norito::json::Value::String(hex::encode([0x91; 32])),
                );
            }
            3 => {
                object.insert(
                    "catalog_sha256".to_owned(),
                    norito::json::Value::String(hex::encode([0x92; 32])),
                );
            }
            4 => {
                object.insert(
                    "issued_at_unix_ms".to_owned(),
                    norito::json::Value::from(1_800_000_300_000_u64),
                );
            }
            5 => {
                object.insert(
                    "issued_at_unix_ms".to_owned(),
                    norito::json::Value::from(1_800_000_030_001_u64),
                );
            }
            6 => {
                object.insert(
                    "expires_at_unix_ms".to_owned(),
                    norito::json::Value::from(1_800_000_299_999_u64),
                );
            }
            7 => {
                object.insert(
                    "signer_key_id".to_owned(),
                    norito::json::Value::String("fixture.other-authority.v1".to_owned()),
                );
            }
            8 => {
                object.insert(
                    "signer_public_key_sha256".to_owned(),
                    norito::json::Value::String(hex::encode([0x93; 32])),
                );
            }
            9 => {}
            _ => unreachable!(),
        }
        let mut changed_receipt =
            refresh_catalog_receipt_fixture_signature_metadata(&mut receipt, &catalog_authority);
        if mutation == 9 {
            let mut changed_signature: norito::json::Value =
                norito::json::from_slice(&changed_receipt).expect("signed fixture receipt JSON");
            let norito::json::Value::Object(object) = &mut changed_signature else {
                unreachable!("fixture catalog receipt is an object");
            };
            object.insert(
                "signature".to_owned(),
                norito::json::Value::String("87".repeat(64)),
            );
            changed_receipt = canonical_json_bytes_v1(&changed_signature)
                .expect("canonical bad-signature receipt");
        }
        let mut changed_body = reservation.body.clone();
        changed_body.catalog_revalidation_receipt_json =
            iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(&changed_receipt)
                .expect("mutated receipt identity");
        let changed = iroha_data_model::offline::KagemushaV4PromotionReservationV1::try_sign(
            changed_body,
            &controller,
        )
        .expect("controller can re-sign the hostile exact receipt identity");
        let changed_reservation =
            norito::encode_canonical(&changed).expect("canonical re-signed hostile reservation");
        iroha_data_model::offline::KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            &changed_reservation,
            controller.public_key(),
        )
        .expect("hostile reservation itself is canonical and controller-authenticated");
        assert!(
            validate_exact_kagemusha_promotion_sources_v1(
                &changed,
                &changed_reservation,
                &changed_receipt,
                VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
                catalog_authority.public_key(),
            )
            .is_err(),
            "strict receipt semantic mutation {mutation} must fail closed"
        );
    }

    let mut noncanonical_receipt = vec![b' '];
    noncanonical_receipt.extend_from_slice(&catalog_receipt);
    let mut noncanonical_body = reservation.body.clone();
    noncanonical_body.catalog_revalidation_receipt_json =
        iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(&noncanonical_receipt)
            .expect("noncanonical receipt identity");
    let noncanonical = iroha_data_model::offline::KagemushaV4PromotionReservationV1::try_sign(
        noncanonical_body,
        &controller,
    )
    .expect("controller can sign an exact noncanonical JSON identity");
    let noncanonical_reservation =
        norito::encode_canonical(&noncanonical).expect("canonical enclosing reservation");
    assert!(
        validate_exact_kagemusha_promotion_sources_v1(
            &noncanonical,
            &noncanonical_reservation,
            &noncanonical_receipt,
            VALIDATOR_CATALOG_REVALIDATION_AUTHORITY_KEY_ID,
            catalog_authority.public_key(),
        )
        .is_err(),
        "valid but noncanonical JSON must fail closed"
    );
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_freshness_is_bounded_at_the_signing_clock() {
    let (_, _, _, authenticated, _, _, subject, _) = validator_qualification_fixture();
    let issued = subject.catalog_revalidation_issued_at_unix_ms();
    let expires = subject.validator_qualification_expires_at_unix_ms();
    validate_validator_qualification_freshness_at_v1(
        &subject,
        issued - KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS,
    )
    .expect("the exact future-skew boundary is accepted");
    validate_validator_qualification_freshness_at_v1(&subject, expires)
        .expect("the signed expiry millisecond is inclusive");
    assert!(
        validate_validator_qualification_freshness_at_v1(
            &subject,
            issued - KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS - 1,
        )
        .is_err()
    );
    assert!(validate_validator_qualification_freshness_at_v1(&subject, expires + 1).is_err());
    assert!(validate_validator_qualification_freshness_at_v1(&subject, 0).is_err());

    assert!(
        KagemushaV4ValidatorQualificationSubjectV1::try_new(
            subject.promotion_controller().clone(),
            subject.promotion_reservation(),
            subject.promotion_id(),
            authenticated.manifest_sha256(),
            subject.device_attestation_policy().clone(),
            expires,
            expires - 1,
            expires,
        )
        .is_err(),
        "policy evaluation must strictly precede the signed expiry"
    );
    assert!(
        KagemushaV4ValidatorQualificationSubjectV1::try_new(
            subject.promotion_controller().clone(),
            subject.promotion_reservation(),
            subject.promotion_id(),
            authenticated.manifest_sha256(),
            subject.device_attestation_policy().clone(),
            issued,
            issued,
            issued + KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS + 1,
        )
        .is_err(),
        "the signed qualification lifetime may not exceed five minutes"
    );
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_requires_the_exact_four_voter_genesis_roster() {
    for voter_count in [7_u8, 10] {
        let validators = (0..voter_count)
            .map(|index| KeyPair::from_seed(vec![0xD0 + index; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        let topology = validators
            .iter()
            .map(|validator| {
                let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
                    .expect("validator roster PoP");
                iroha_genesis::GenesisTopologyEntry::new(
                    iroha_data_model::peer::PeerId::new(validator.public_key().clone()),
                    pop,
                )
            })
            .collect::<Vec<_>>();
        let genesis_signer = KeyPair::from_seed(vec![0xDF; 32], Algorithm::Ed25519);
        let genesis = iroha_genesis::GenesisBuilder::new_without_executor(
            format!("validator-qualification-{voter_count}-voter-fixture")
                .parse()
                .expect("canonical inexact-roster chain id"),
            ".",
        )
        .set_topology(topology)
        .build_raw()
        .with_consensus_meta()
        .build_and_sign(&genesis_signer)
        .expect("signed inexact-roster genesis fixture");
        let genesis_bytes = genesis
            .0
            .encode_wire()
            .expect("canonical inexact-roster genesis fixture");
        let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());
        let error =
            exact_genesis_qualification_identity_v1(&genesis, &genesis_bytes, &validator_id)
                .expect_err("an inexact signed-genesis voter roster must fail closed");
        assert!(error.contains("exact 4-validator signed genesis roster"));
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_rejects_exact_four_npos_voter_genesis_roster() {
    let validators = (0_u8..4)
        .map(|index| KeyPair::from_seed(vec![0xE0 + index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let topology = validators
        .iter()
        .map(|validator| {
            let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
                .expect("NPoS validator roster PoP");
            iroha_genesis::GenesisTopologyEntry::new(
                iroha_data_model::peer::PeerId::new(validator.public_key().clone()),
                pop,
            )
        })
        .collect::<Vec<_>>();
    let genesis_signer = KeyPair::from_seed(vec![0xE4; 32], Algorithm::Ed25519);
    let genesis = iroha_genesis::GenesisBuilder::new_without_executor(
        iroha_data_model::ChainId::from("validator-qualification-four-npos-voter-fixture"),
        ".",
    )
    .set_topology(topology)
    .append_parameter(Parameter::Custom(
        iroha_data_model::parameter::system::SumeragiNposParameters::default()
            .into_custom_parameter(),
    ))
    .build_raw()
    .with_consensus_mode(SumeragiConsensusMode::Npos)
    .with_consensus_meta()
    .build_and_sign(&genesis_signer)
    .expect("signed exact-four NPoS validator genesis fixture");
    let genesis_bytes = genesis
        .0
        .encode_wire()
        .expect("canonical exact-four NPoS validator genesis fixture");
    let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());

    let error = exact_genesis_qualification_identity_v1(&genesis, &genesis_bytes, &validator_id)
        .expect_err("an exact-four NPoS-mode roster must not qualify");
    assert!(error.contains("signed permissioned consensus with unit-power validators"));
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_derives_and_signs_exact_captured_identities() {
    let (genesis, validators, genesis_bytes, authenticated, release_record, seal, subject, digest) =
        validator_qualification_fixture();
    let config_bytes = b"[common]\nchain = 'exact'\n";
    let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());
    let runtime_effective_config = runtime_effective_config_fixture(&genesis, &validators);
    let signed = build_and_sign_validator_qualification_from_verified_release_v1(
        &seal,
        &authenticated,
        &release_record,
        authenticated.manifest().qualification_receipt_sha256,
        digest,
        &subject,
        &genesis,
        &genesis_bytes,
        config_bytes,
        &runtime_effective_config,
        &validator_id,
        &validators[0],
    )
    .expect("exact captured validator qualification");
    signed.verify().expect("validator qualification signature");
    assert_eq!(signed.body.binding.promotion_id, subject.promotion_id());
    assert_eq!(
        signed.body.binding.network_id,
        authenticated.manifest().network_id
    );
    assert_eq!(
        signed.body.binding.manifest_sha256,
        authenticated.manifest_sha256()
    );
    assert_eq!(signed.body.binding.catalog_consensus_policy_digest, digest);
    assert!(
        signed
            .body
            .binding
            .signed_genesis
            .matches_bytes(&genesis_bytes)
    );
    assert!(
        signed
            .body
            .flattened_toml_config_source
            .matches_bytes(config_bytes)
    );
    assert!(
        signed
            .body
            .catalog_qualification_seal
            .matches_bytes(&seal.canonical_bytes().expect("canonical catalog seal"))
    );
    assert_eq!(
        signed.body.binding.release_policy_source.sha256,
        seal.configured_policy_sha256
    );
    assert_eq!(
        signed.body.iroha3d_executable.sha256,
        seal.executable_sha256
    );
    let exact_seal = seal.canonical_bytes().expect("canonical catalog seal");
    seal.verify_exact_canonical_bytes(&exact_seal)
        .expect("exact same-load seal bytes");
    let mut changed_seal = exact_seal;
    changed_seal[0] ^= 1;
    assert!(seal.verify_exact_canonical_bytes(&changed_seal).is_err());
    assert!(seal.verify_exact_canonical_bytes(&[]).is_err());

    let rejects_projection = |projection: &KagemushaV4RuntimeEffectiveConfigProjectionV1| {
        build_and_sign_validator_qualification_from_verified_release_v1(
            &seal,
            &authenticated,
            &release_record,
            authenticated.manifest().qualification_receipt_sha256,
            digest,
            &subject,
            &genesis,
            &genesis_bytes,
            config_bytes,
            projection,
            &validator_id,
            &validators[0],
        )
        .is_err()
    };
    let mut observer_projection = runtime_effective_config.clone();
    observer_projection.is_validator = false;
    assert!(rejects_projection(&observer_projection));
    let mut stale_context = runtime_effective_config.clone();
    stale_context.genesis_context.execution_policy_hash[0] ^= 1;
    assert!(rejects_projection(&stale_context));
    let mut stale_pop = runtime_effective_config.clone();
    stale_pop.validators[0].bls_pop[0] ^= 1;
    assert!(rejects_projection(&stale_pop));

    let mut changed_config = config_bytes.to_vec();
    changed_config.extend_from_slice(b"# changed\n");
    let changed = build_and_sign_validator_qualification_from_verified_release_v1(
        &seal,
        &authenticated,
        &release_record,
        authenticated.manifest().qualification_receipt_sha256,
        digest,
        &subject,
        &genesis,
        &genesis_bytes,
        &changed_config,
        &runtime_effective_config,
        &validator_id,
        &validators[0],
    )
    .expect("changed exact config is captured, not caller-digested");
    assert_ne!(
        changed.body.flattened_toml_config_source,
        signed.body.flattened_toml_config_source
    );
    assert_eq!(
        changed.body.binding.manifest_sha256,
        signed.body.binding.manifest_sha256
    );
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn validator_qualification_rejects_stale_release_genesis_policy_and_signer() {
    let (genesis, validators, genesis_bytes, authenticated, release_record, seal, subject, digest) =
        validator_qualification_fixture();
    let validator_id = iroha_data_model::peer::PeerId::new(validators[0].public_key().clone());
    let runtime_effective_config = runtime_effective_config_fixture(&genesis, &validators);
    let attempt = |record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
                   raw_genesis: &[u8],
                   selected: &KagemushaV4ValidatorQualificationSubjectV1,
                   signer: &KeyPair| {
        build_and_sign_validator_qualification_from_verified_release_v1(
            &seal,
            &authenticated,
            record,
            authenticated.manifest().qualification_receipt_sha256,
            digest,
            selected,
            &genesis,
            raw_genesis,
            b"exact config",
            &runtime_effective_config,
            &validator_id,
            signer,
        )
    };
    let mut stale_genesis = genesis_bytes.clone();
    stale_genesis.push(0);
    assert!(attempt(&release_record, &stale_genesis, &subject, &validators[0]).is_err());
    let mut stale_record = release_record.clone();
    stale_record
        .manifest
        .reviewed_source_closure_descriptor_sha256[0] ^= 1;
    assert!(attempt(&stale_record, &genesis_bytes, &subject, &validators[0]).is_err());
    assert!(attempt(&release_record, &genesis_bytes, &subject, &validators[1]).is_err());

    let mut invalid_policy = subject.device_attestation_policy().clone();
    invalid_policy.require_android_app_policy = false;
    assert!(
        KagemushaV4ValidatorQualificationSubjectV1::try_new(
            subject.promotion_controller().clone(),
            subject.promotion_reservation(),
            [0xD5; 32],
            authenticated.manifest_sha256(),
            invalid_policy,
            1_800_000_000_000,
            subject.catalog_revalidation_issued_at_unix_ms(),
            subject.validator_qualification_expires_at_unix_ms(),
        )
        .is_err()
    );
    assert!(
        KagemushaV4ValidatorQualificationSubjectV1::try_new(
            subject.promotion_controller().clone(),
            subject.promotion_reservation(),
            [0; 32],
            authenticated.manifest_sha256(),
            subject.device_attestation_policy().clone(),
            1_800_000_000_000,
            subject.catalog_revalidation_issued_at_unix_ms(),
            subject.validator_qualification_expires_at_unix_ms(),
        )
        .is_err()
    );
    let wrong_release = KagemushaV4ValidatorQualificationSubjectV1::try_new(
        subject.promotion_controller().clone(),
        subject.promotion_reservation(),
        [0xD5; 32],
        [0xE5; 32],
        subject.device_attestation_policy().clone(),
        1_800_000_000_000,
        subject.catalog_revalidation_issued_at_unix_ms(),
        subject.validator_qualification_expires_at_unix_ms(),
    )
    .expect("structurally valid but absent release selector");
    assert!(
        attempt(
            &release_record,
            &genesis_bytes,
            &wrong_release,
            &validators[0]
        )
        .is_err()
    );

    let mut signed = attempt(&release_record, &genesis_bytes, &subject, &validators[0])
        .expect("baseline qualification");
    signed.body.binding.promotion_id[0] ^= 1;
    assert!(
        signed.verify().is_err(),
        "promotion mutation must invalidate the signature"
    );
}
