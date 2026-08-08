// Python crypto/admission boundary regressions share the parent test module's fixtures.
#[test]
fn fee_sponsor_program_ids_require_exact_canonical_literals() {
    let sponsor_literal = taira_i105_from_seed(0x74);
    let literal = format!("{sponsor_literal}/retail");
    let parsed = parse_fee_sponsor_program_id(&literal).expect("program id parses");
    assert_eq!(parsed.sponsor, sample_account(0x74));
    assert_eq!(parsed.name.as_ref(), "retail");
    assert!(parse_fee_sponsor_program_id(&format!(" {literal}")).is_err());
    assert!(parse_fee_sponsor_program_id(&sponsor_literal).is_err());
}

#[test]
fn i105_discriminant_hint_decodes_valid_literals_only() {
    let custom_account = custom_i105_from_seed(0x70, 42);
    assert_eq!(
        AccountAddress::i105_discriminant(&custom_account).ok(),
        Some(42)
    );

    let noncanonical = custom_account.replacen("n42", "n00042", 1);
    assert_eq!(AccountAddress::i105_discriminant(&noncanonical).ok(), None);

    let mut chars = custom_account.chars().collect::<Vec<_>>();
    let last = chars.len().saturating_sub(1);
    chars[last] = if chars[last] == '1' { '2' } else { '1' };
    let tampered = chars.into_iter().collect::<String>();
    assert_eq!(AccountAddress::i105_discriminant(&tampered).ok(), None);
    assert_eq!(AccountAddress::i105_discriminant("n").ok(), None);
    assert_eq!(AccountAddress::i105_discriminant("nabc").ok(), None);
    assert_eq!(
        AccountAddress::i105_discriminant("n65536payload").ok(),
        None
    );
}

#[test]
fn parse_account_id_accepts_taira_i105_literals_without_global_discriminant() {
    let taira_account = taira_i105_from_seed(0x71);
    assert!(
        taira_account.starts_with("test"),
        "Taira I105 account must use the public test sentinel"
    );
    assert_eq!(
        parse_account_id(&taira_account).expect("Taira account parses"),
        sample_account(0x71)
    );
}

#[test]
fn parse_account_id_accepts_numeric_custom_i105_literals_without_global_discriminant() {
    let custom_account = custom_i105_from_seed(0x72, 42);
    assert!(
        custom_account.starts_with("n42"),
        "custom I105 account must use the numeric sentinel"
    );
    assert_eq!(
        parse_account_id(&custom_account).expect("custom account parses"),
        sample_account(0x72)
    );
}

#[test]
fn parse_account_id_rejects_noncanonical_and_tampered_numeric_custom_i105_literals() {
    let custom_account = custom_i105_from_seed(0x73, 42);
    let noncanonical = custom_account.replacen("n42", "n00042", 1);
    assert!(
        parse_account_id(&noncanonical).is_err(),
        "noncanonical numeric sentinel must be rejected"
    );

    let mut chars = custom_account.chars().collect::<Vec<_>>();
    let last = chars.len().saturating_sub(1);
    chars[last] = if chars[last] == '1' { '2' } else { '1' };
    let tampered = chars.into_iter().collect::<String>();
    assert!(
        parse_account_id(&tampered).is_err(),
        "payload/checksum tampering must be rejected"
    );
}
#[test]
fn seed_derivation_pyfunctions_use_checked_backend_derivation() {
    ensure_python();
    let seed = b"python checked seed derivation";
    let expected = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .expect("derive expected checked Ed25519 key pair");
    let (_, expected_private) = expected.private_key().to_bytes();
    let (_, expected_public) = public_key_to_bytes(expected.public_key(), "expected public key")
        .expect("expected public key payload is well-formed");

    Python::attach(|py| {
        let (generic_private, generic_public) =
            derive_keypair_from_seed_py(py, seed, Algorithm::Ed25519.as_static_str())
                .expect("generic checked seed derivation succeeds");
        let (ed25519_private, ed25519_public) = derive_ed25519_keypair_from_seed_py(py, seed)
            .expect("Ed25519 checked seed derivation succeeds");

        assert_eq!(
            generic_private.bind(py).as_bytes(),
            expected_private.as_slice()
        );
        assert_eq!(generic_public.bind(py).as_bytes(), expected_public);
        assert_eq!(
            ed25519_private.bind(py).as_bytes(),
            expected_private.as_slice()
        );
        assert_eq!(ed25519_public.bind(py).as_bytes(), expected_public);
    });
}

#[test]
fn parse_algorithm_arg_accepts_exact_supported_aliases() {
    for (label, expected) in [
        ("ed-25519", Algorithm::Ed25519),
        ("ECDSA-SECP256K1-SHA256", Algorithm::Secp256k1),
        ("ml_dsa", Algorithm::MlDsa),
        (
            "gost3410_2012_512_paramset_b",
            Algorithm::Gost3410_2012_512ParamSetB,
        ),
        ("bls-normal", Algorithm::BlsNormal),
        ("SM2", Algorithm::Sm2),
    ] {
        assert_eq!(parse_algorithm_arg(label).unwrap(), expected, "{label}");
    }
}

#[test]
fn parse_algorithm_arg_rejects_empty_padded_control_and_non_ascii_labels() {
    for (label, expected_message) in [
        ("", "algorithm must be a non-empty string"),
        (" ", "algorithm must not contain surrounding whitespace"),
        ("\t", "algorithm must not contain surrounding whitespace"),
        (
            "\u{00A0}",
            "algorithm must not contain surrounding whitespace",
        ),
        (
            " ed25519",
            "algorithm must not contain surrounding whitespace",
        ),
        (
            "ed25519 ",
            "algorithm must not contain surrounding whitespace",
        ),
        (
            "\ted25519",
            "algorithm must not contain surrounding whitespace",
        ),
        (
            "ed25519\n",
            "algorithm must not contain surrounding whitespace",
        ),
        ("ed\u{0000}25519", "unsupported crypto algorithm"),
        ("ed\u{001F}25519", "unsupported crypto algorithm"),
        ("ed\u{007F}25519", "unsupported crypto algorithm"),
        ("ed\u{200B}25519", "unsupported crypto algorithm"),
        ("\u{0435}d25519", "unsupported crypto algorithm"),
        ("ed\u{FF0D}25519", "unsupported crypto algorithm"),
    ] {
        let err = parse_algorithm_arg(label).expect_err(label);
        let message = py_err_message(err);
        assert!(
            message.contains(expected_message),
            "{label:?}: expected {expected_message:?}, got {message:?}"
        );
    }
}

#[test]
fn privacy_compiled_profile_catalog_is_the_exact_closed_registry() {
    let catalog = privacy_compiled_profile_catalog().expect("compiled-profile catalog");
    catalog
        .validate()
        .expect("canonical compiled-profile catalog");
    assert_eq!(catalog.protocols.len(), PrivacyProtocolIdV1::COUNT);
    assert!(
        catalog
            .protocols
            .iter()
            .map(|row| row.protocol_id)
            .eq(PrivacyProtocolIdV1::ALL)
    );
}

fn provider_metadata(provider_id: &str) -> PyProviderMetadata {
    PyProviderMetadata {
        provider_id: Some(provider_id.to_string()),
        profile_id: None,
        profile_aliases: None,
        availability: None,
        stake_amount: None,
        max_streams: Some(2),
        refresh_deadline: None,
        expires_at: None,
        ttl_secs: None,
        allow_unknown_capabilities: Some(true),
        capability_names: None,
        rendezvous_topics: None,
        notes: None,
        range_capability: Some(PyRangeCapability {
            max_chunk_span: u32::MAX,
            min_granularity: 1,
            supports_sparse_offsets: Some(true),
            requires_alignment: Some(false),
            supports_merkle_proof: Some(true),
        }),
        stream_budget: Some(PyStreamBudget {
            max_in_flight: 4,
            max_bytes_per_sec: 8 * 1024 * 1024,
            burst_bytes: Some(8 * 1024 * 1024),
        }),
        transport_hints: None,
    }
}

#[test]
fn generate_sm2_keypair_roundtrip() {
    ensure_python();
    Python::attach(|py| {
        let (private_py, public_py) =
            generate_sm2_keypair_py(py, None).expect("generate SM2 keypair");
        let private_bytes = private_py.bind(py).as_bytes();
        let public_bytes = public_py.bind(py).as_bytes();
        assert_eq!(private_bytes.len(), SM2_PRIVATE_KEY_LENGTH);
        assert_eq!(public_bytes.len(), SM2_PUBLIC_KEY_UNCOMPRESSED_LENGTH);
        let private = parse_sm2_private_key(None, private_bytes).expect("parse SM2 private key");
        let derived_public = private.public_key().to_sec1_bytes(false);
        assert_eq!(derived_public.as_slice(), public_bytes);
    });
}

#[test]
fn parse_public_key_multihash_returns_checked_payload() {
    ensure_python();
    let key_pair =
        KeyPair::try_from_seed(b"python-public-key-multihash".to_vec(), Algorithm::Ed25519)
            .expect("derive Python public-key multihash fixture key");
    let (algorithm, expected_payload) =
        public_key_to_bytes(key_pair.public_key(), "fixture public key")
            .expect("fixture public key is well-formed");
    let encoded = key_pair
        .public_key()
        .try_to_prefixed_string()
        .expect("fixture public key prefixed multihash formats");

    Python::attach(|py| {
        let (parsed_algorithm, parsed_payload) =
            parse_public_key_multihash_py(py, &encoded).expect("public key multihash parses");
        assert_eq!(parsed_algorithm, algorithm.as_static_str());
        assert_eq!(parsed_payload.bind(py).as_bytes(), expected_payload);
    });
}

#[test]
fn public_key_multihash_rejects_malformed_ed25519_public_key_material() {
    for (label, public_key, expected_error) in MALFORMED_ED25519_PUBLIC_KEYS {
        for prefixed in [false, true] {
            let err = py_err_message(
                public_key_multihash_py(Algorithm::Ed25519.as_static_str(), &public_key, prefixed)
                    .expect_err("malformed Ed25519 public key must not format"),
            );
            assert!(
                err.contains("failed to parse public key"),
                "unexpected public-key multihash {label} error: {err}"
            );
            assert!(
                err.contains(expected_error),
                "public-key multihash {label} error lost parser detail: {err}"
            );
        }
    }
}

#[test]
fn multihash_helpers_use_checked_formatters() {
    ensure_python();
    let key_pair = KeyPair::try_from_seed(b"python-multihash-helper".to_vec(), Algorithm::Ed25519)
        .expect("derive Python multihash helper fixture key");
    let (_, public_payload) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
        .expect("fixture public key is well-formed");
    let public_payload = public_payload.to_vec();
    let (private_algorithm, private_payload) = key_pair.private_key().to_bytes();
    assert_eq!(private_algorithm, Algorithm::Ed25519);
    let exposed_private = ExposedPrivateKey(key_pair.private_key().clone());

    assert_eq!(
        public_key_multihash_py(Algorithm::Ed25519.as_static_str(), &public_payload, false)
            .expect("public key multihash formats"),
        public_key_multihash_string(key_pair.public_key(), false, "expected public key")
            .expect("expected public key multihash formats")
    );
    assert_eq!(
        public_key_multihash_py(Algorithm::Ed25519.as_static_str(), &public_payload, true)
            .expect("prefixed public key multihash formats"),
        public_key_multihash_string(key_pair.public_key(), true, "expected public key")
            .expect("expected prefixed public key multihash formats")
    );
    assert_eq!(
        private_key_multihash_py(
            Algorithm::Ed25519.as_static_str(),
            private_payload.as_slice(),
            false,
        )
        .expect("private key multihash formats"),
        private_key_multihash_string(&exposed_private, false, "expected private key")
            .expect("expected private key multihash formats")
    );
    assert_eq!(
        private_key_multihash_py(
            Algorithm::Ed25519.as_static_str(),
            private_payload.as_slice(),
            true,
        )
        .expect("prefixed private key multihash formats"),
        private_key_multihash_string(&exposed_private, true, "expected private key")
            .expect("expected prefixed private key multihash formats")
    );
}

#[test]
fn sm2_fixture_from_seed_uses_checked_public_key_formatters() {
    ensure_python();
    let distid = "1234567812345678";
    let seed = [0x42_u8; SM2_PRIVATE_KEY_LENGTH];
    let message = b"python sm2 fixture checked multihash";

    Python::attach(|py| {
        let fixture =
            sm2_fixture_from_seed_py(py, distid, &seed, message).expect("SM2 fixture generates");
        let fixture = fixture.bind(py);
        let public_key_sec1_hex = fixture
            .get_item("public_key_sec1_hex")
            .expect("SEC1 public key item lookup succeeds")
            .expect("SEC1 public key item exists")
            .extract::<String>()
            .expect("SEC1 public key is string");
        let public_key_multihash = fixture
            .get_item("public_key_multihash")
            .expect("multihash item lookup succeeds")
            .expect("multihash item exists")
            .extract::<String>()
            .expect("multihash is string");
        let public_key_prefixed = fixture
            .get_item("public_key_prefixed")
            .expect("prefixed item lookup succeeds")
            .expect("prefixed item exists")
            .extract::<String>()
            .expect("prefixed multihash is string");
        let public_key_sec1 =
            hex::decode(public_key_sec1_hex).expect("fixture SEC1 public key hex decodes");
        let payload = encode_sm2_public_key_payload(distid, &public_key_sec1)
            .expect("fixture SM2 public key payload encodes");
        let public_key = PublicKey::from_bytes(Algorithm::Sm2, &payload)
            .expect("fixture SM2 public key constructs");

        assert_eq!(
            sm2_public_key_multihash_py(&public_key_sec1, Some(distid))
                .expect("SM2 public key multihash formats"),
            public_key_multihash
        );
        assert_eq!(
            sm2_fixture_public_key_multihashes(&public_key)
                .expect("fixture public key multihashes format")
                .1,
            public_key_prefixed
        );
    });
}

#[test]
fn keypair_and_account_public_exports_use_checked_payloads() {
    ensure_python();
    let key_pair = KeyPair::try_from_seed(b"python-keypair-export".to_vec(), Algorithm::Ed25519)
        .expect("derive Python keypair export fixture key");
    let (_, expected_public) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
        .expect("fixture public key is well-formed");
    let expected_public = expected_public.to_vec();
    let (_, expected_private) = key_pair.private_key().to_bytes();
    let authority = AccountId::new(key_pair.public_key().clone())
        .canonical_i105()
        .expect("canonical authority");

    Python::attach(|py| {
        let (private_py, public_py) = keypair_to_py(py, key_pair.clone()).expect("keypair exports");
        assert_eq!(public_py.bind(py).as_bytes(), expected_public.as_slice());
        assert_eq!(private_py.bind(py).as_bytes(), expected_private.as_slice());
    });

    let account = PyAccountId::new(&authority).expect("account id parses");
    assert_eq!(
        account.public_key_hex().expect("public key hex"),
        hex::encode(expected_public)
    );
}

#[test]
fn sorafs_alias_proof_fixture_generates_servable_checked_signer() {
    ensure_python();
    Python::attach(|py| {
        let fixture =
            sorafs_alias_proof_fixture_py(py, None).expect("alias proof fixture generates");
        let fixture = fixture.bind(py);
        let proof_b64 = fixture
            .get_item("proof_b64")
            .expect("proof item lookup succeeds")
            .expect("proof item exists")
            .extract::<String>()
            .expect("proof is string");
        let generated_at_unix = fixture
            .get_item("generated_at_unix")
            .expect("generated item lookup succeeds")
            .expect("generated item exists")
            .extract::<u64>()
            .expect("generated timestamp is integer");

        let evaluation =
            sorafs_evaluate_alias_proof_py(py, &proof_b64, None, Some(generated_at_unix))
                .expect("alias proof evaluates");
        let evaluation = evaluation.bind(py);
        let state = evaluation
            .get_item("state")
            .expect("state item lookup succeeds")
            .expect("state item exists")
            .extract::<String>()
            .expect("state is string");
        let servable = evaluation
            .get_item("servable")
            .expect("servable item lookup succeeds")
            .expect("servable item exists")
            .extract::<bool>()
            .expect("servable is boolean");

        assert_eq!(state, "fresh");
        assert!(servable);
    });
}

#[test]
fn decode_transaction_receipt_json_roundtrip() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let payload = iroha_data_model::transaction::TransactionSubmissionReceiptPayload {
        tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
        entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
        signed_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed([0xB6; 32]))),
        submitted_at_ms: 42,
        submitted_at_height: 7,
        signer: key_pair.public_key().clone(),
    };
    let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
    let bytes = to_bytes(&receipt).expect("encode receipt");
    let decoded = decode_transaction_receipt_json_py(&bytes).expect("decode receipt json");
    let expected = json::to_json(&receipt).expect("serialize receipt");
    assert_eq!(decoded, expected);
}

#[test]
fn privacy_bridge_abi_version_python_function_matches_first_release() {
    assert_eq!(privacy_bridge_abi_version_py(), 21);
}

#[test]
fn privacy_compiled_profile_catalog_python_validator_calls_the_exact_local_boundary() {
    let catalog = privacy_compiled_profile_catalog().expect("compiled-profile catalog");
    let archive = norito::encode_canonical(&catalog).expect("canonical compiled catalog");
    assert_eq!(
        privacy_validate_compiled_profile_catalog_v1_py(&archive),
        iroha_data_model::privacy::PrivacyCompiledProfileCatalogArchiveValidationStatusV1::Valid
            .code()
    );
    let mut one_byte_fake = norito::encode_canonical(&0_u8).expect("one-byte fake");
    one_byte_fake[6..22].copy_from_slice(&archive[6..22]);
    assert_ne!(
        privacy_validate_compiled_profile_catalog_v1_py(&one_byte_fake),
        iroha_data_model::privacy::PrivacyCompiledProfileCatalogArchiveValidationStatusV1::Valid
            .code()
    );

    let mut substituted = catalog;
    let profile = substituted
        .protocols
        .iter_mut()
        .find_map(|row| match &mut row.compiled_profile {
            iroha_data_model::privacy::PrivacyCompiledProfileResultV1::Available(profile) => {
                Some(profile)
            }
            iroha_data_model::privacy::PrivacyCompiledProfileResultV1::Unavailable(_) => None,
        })
        .expect("at least one compiled profile");
    let mut digest = *profile.parameter_digest.as_bytes();
    digest[0] ^= 0x80;
    profile.parameter_digest = iroha_data_model::privacy::PrivacyParameterDigestV1::new(digest);
    profile
        .validate()
        .expect("substituted profile remains structurally valid");
    let substituted = norito::encode_canonical(&substituted).expect("encode substitution");
    assert_eq!(
        iroha_data_model::privacy::validate_privacy_compiled_profile_catalog_archive_v1(
            &substituted,
        ),
        iroha_data_model::privacy::PrivacyCompiledProfileCatalogArchiveValidationStatusV1::Valid,
        "the generic validator must accept the structurally valid substitution",
    );
    assert_eq!(
            privacy_validate_compiled_profile_catalog_v1_py(&substituted),
            iroha_data_model::privacy::PrivacyCompiledProfileCatalogArchiveValidationStatusV1::InvalidCatalog
                .code()
        );
}

#[test]
fn jindo_python_result_separates_classification_from_ledger_effect() {
    assert_eq!(
        JINDO_ACTION_EXECUTION_CLASSIFICATION_V1,
        "action_verification_and_finality_only"
    );
    assert_eq!(jindo_action_ledger_effect_v1(), None);
}

#[test]
fn jindo_python_owned_witness_buffers_are_explicitly_erased() {
    let mut witness = ZeroizingJindoWitnessBytes(vec![
        vec![vec![0xA5; 32], vec![0x5A; 32]],
        vec![vec![0x3C; 32]],
    ]);
    witness.erase();
    assert!(witness.0.iter().flatten().flatten().all(|byte| *byte == 0));
}

#[test]
fn zk_ace_python_result_and_witness_boundary_is_exact() {
    assert_eq!(
        ZK_ACE_ACTION_EXECUTION_CLASSIFICATION_V1,
        "authorization_action"
    );
    assert_eq!(
        ZK_ACE_TRANSFER_LEDGER_EFFECT_V1,
        "zk_ace_transparent_transfer"
    );

    let mut witness = ZeroizingZkAceWitnessBytes {
        identity_root: vec![0x11; 32],
        identity_blinding: vec![0x22; 32],
        replay_secret: vec![0x33; 32],
    };
    witness.erase();
    assert!(witness.identity_root.iter().all(|byte| *byte == 0));
    assert!(witness.identity_blinding.iter().all(|byte| *byte == 0));
    assert!(witness.replay_secret.iter().all(|byte| *byte == 0));
}

#[test]
fn zk_ace_python_policy_archive_is_canonical_and_self_authenticating() {
    use iroha_data_model::privacy::{
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1, PrivacyPolicyIdV1, PrivacyZkAcePolicyLifecycleV1,
        PrivacyZkAcePolicyRecordDigestV1,
    };

    let source = sample_account(0x51);
    let witness = ZkAcePrivacyWitnessV1::try_new([0x11; 32], [0x22; 32], [0x33; 32])
        .expect("valid ZK-ACE policy witness");
    let policy = PrivacyZkAcePolicyRecordV1::new(
        PrivacyPolicyIdV1::new([0x41; 32]),
        witness.identity_commitment_v1(),
        PrivacyPolicyDigestV1::new([0x42; 32]),
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        AssetDefinitionId::from_str("7MBRDd8cGFBZkFGdDMwV7S6FPwbw")
            .expect("fixture asset definition"),
        vec![source],
        PrivacyZkAcePolicyLifecycleV1::Active,
    )
    .expect("valid canonical ZK-ACE policy");
    let archive = norito::encode_canonical(&policy).expect("canonical policy archive");
    assert_eq!(
        python_zk_ace_policy_v1(&archive).expect("canonical policy accepted"),
        policy
    );

    for malformed in [
        Vec::new(),
        vec![0xA5],
        vec![0xA5; ZK_ACE_POLICY_ARCHIVE_MAX_BYTES_V1 + 1],
    ] {
        assert!(python_zk_ace_policy_v1(&malformed).is_err());
    }

    let mut trailing = archive.clone();
    trailing.push(0);
    assert!(
        python_zk_ace_policy_v1(&trailing).is_err(),
        "trailing archive bytes must not be accepted"
    );

    let mut digest_tampered = policy;
    digest_tampered.record_digest = PrivacyZkAcePolicyRecordDigestV1::new([0x7F; 32]);
    let digest_tampered_archive =
        norito::encode_canonical(&digest_tampered).expect("encode tampered policy");
    assert!(
        python_zk_ace_policy_v1(&digest_tampered_archive).is_err(),
        "a canonical wire with a substituted self-digest must fail"
    );
}

#[test]
fn zk_ace_python_builder_rejects_secret_shapes_before_proving() {
    use iroha_data_model::privacy::{
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1, PrivacyPolicyIdV1, PrivacyZkAcePolicyLifecycleV1,
    };

    ensure_python();
    let authority = canonical_i105_from_seed(0x51);
    let destination = canonical_i105_from_seed(0x52);
    let source = parse_account_id(&authority).expect("authority account parses");
    let witness = ZkAcePrivacyWitnessV1::try_new([0x11; 32], [0x22; 32], [0x33; 32])
        .expect("valid ZK-ACE policy witness");
    let policy = PrivacyZkAcePolicyRecordV1::new(
        PrivacyPolicyIdV1::new([0x41; 32]),
        witness.identity_commitment_v1(),
        PrivacyPolicyDigestV1::new([0x42; 32]),
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        AssetDefinitionId::from_str("7MBRDd8cGFBZkFGdDMwV7S6FPwbw")
            .expect("fixture asset definition"),
        vec![source],
        PrivacyZkAcePolicyLifecycleV1::Active,
    )
    .expect("valid canonical ZK-ACE policy");
    let policy_archive = norito::encode_canonical(&policy).expect("canonical policy archive");

    Python::attach(|py| {
        for (identity_root, identity_blinding, replay_secret, expected) in [
            (
                vec![0x11; 31],
                vec![0x22; 32],
                vec![0x33; 32],
                "identity_root must be exactly 32 bytes",
            ),
            (
                vec![0x11; 32],
                vec![0x22; 33],
                vec![0x33; 32],
                "identity_blinding must be exactly 32 bytes",
            ),
            (
                vec![0x11; 32],
                vec![0x22; 32],
                vec![0x33; 0],
                "replay_secret must be exactly 32 bytes",
            ),
            (
                vec![0; 32],
                vec![0x22; 32],
                vec![0x33; 32],
                "identity root witness must be non-zero",
            ),
        ] {
            let mut builder =
                TransactionBuilder::new("test-chain", &authority, authority_fee_payment_json())
                    .expect("builder constructs");
            let error = builder
                .sign_privacy_zk_ace_transfer_action_v1(
                    py,
                    &[0x51; 32],
                    &[0xA5; 32],
                    &policy_archive,
                    &authority,
                    &destination,
                    "1",
                    identity_root,
                    identity_blinding,
                    replay_secret,
                )
                .err()
                .expect("invalid witness must fail before proof construction");
            assert!(
                error.to_string().contains(expected),
                "expected {expected:?}, got {error}"
            );
        }
    });
}

#[test]
fn component_python_results_separate_classification_from_ledger_effect() {
    assert_eq!(
        VERANGE_ACTION_EXECUTION_CLASSIFICATION_V1,
        "action_verification_and_finality_only"
    );
    assert_eq!(
        VEGA_ACTION_EXECUTION_CLASSIFICATION_V1,
        "action_verification_and_finality_only"
    );
}

#[test]
fn component_python_owned_witness_buffers_are_explicitly_erased() {
    let mut verange = ZeroizingVeRangeWitnessBytes {
        values: vec![7, u64::MAX],
        blindings: vec![vec![0xA5; 32], vec![0x5A; 32]],
    };
    verange.erase();
    assert!(verange.values.iter().all(|value| *value == 0));
    assert!(verange.blindings.iter().flatten().all(|byte| *byte == 0));

    let mut vega = ZeroizingVegaWitnessBytes {
        issuer_authentication_sig_structure: vec![0x11; 67],
        mobile_security_object_payload: vec![0x22; 91],
        birth_date_issuer_signed_item: vec![0x33; 41],
        issuer_signature: vec![0x44; 64],
        device_signature: vec![0x55; 64],
    };
    vega.erase();
    assert!(
        vega.issuer_authentication_sig_structure
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(
        vega.mobile_security_object_payload
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(
        vega.birth_date_issuer_signed_item
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(vega.issuer_signature.iter().all(|byte| *byte == 0));
    assert!(vega.device_signature.iter().all(|byte| *byte == 0));
}

#[test]
fn component_python_nonzero_digest_boundary_rejects_ambiguous_encodings() {
    assert!(python_nonzero_privacy_digest_v1(&[0x11; 32], "digest").is_ok());
    assert!(python_nonzero_privacy_digest_v1(&[0x11; 31], "digest").is_err());
    assert!(python_nonzero_privacy_digest_v1(&[0x11; 33], "digest").is_err());
    assert!(python_nonzero_privacy_digest_v1(&[0; 32], "digest").is_err());
}

#[test]
fn vega_python_device_digest_binds_intent_and_session() {
    let profile =
        python_compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0, "Vega")
            .expect("compiled Vega profile");
    let issuer_public_key = *VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32)
        .expect("P-256 parameters")
        .value_generator()
        .as_bytes();
    let context = |intent: [u8; 32]| PrivacyStatementContextV1 {
        chain_id: parse_chain_id("test-chain").expect("chain id"),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(intent),
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
    };
    let statement = |intent, challenge| {
        python_vega_statement_v1(
            context(intent),
            [0xA5; 32],
            [0xA1; 32],
            1,
            [0xA2; 32],
            issuer_public_key,
            2026,
            7,
            28,
            18,
            challenge,
            [0xC3; 32],
        )
        .expect("valid Vega public statement")
    };

    let bound_intent = statement([0xD2; 32], [0xB4; 32]);
    let other_intent = statement([0xD3; 32], [0xB4; 32]);
    let other_session = statement([0xD2; 32], [0xB5; 32]);
    assert_ne!(
        bound_intent.device_authentication_digest, other_intent.device_authentication_digest,
        "H_dev must bind the canonical transaction intent"
    );
    assert_ne!(
        bound_intent.device_authentication_digest, other_session.device_authentication_digest,
        "H_dev must bind the reader challenge"
    );
    assert!(
        python_vega_statement_v1(
            context([0; 32]),
            [0xA5; 32],
            [0xA1; 32],
            1,
            [0xA2; 32],
            issuer_public_key,
            2026,
            7,
            28,
            18,
            [0xB4; 32],
            [0xC3; 32],
        )
        .is_err(),
        "the low-level helper must reject an unprepared all-zero intent"
    );
    assert!(
        python_vega_statement_v1(
            context([0xD2; 32]),
            [0xA5; 32],
            [0xA1; 32],
            1,
            [0xA2; 32],
            issuer_public_key,
            2026,
            13,
            28,
            18,
            [0xB4; 32],
            [0xC3; 32],
        )
        .is_err()
    );
    assert!(
        python_vega_statement_v1(
            context([0xD2; 32]),
            [0xA5; 32],
            [0xA1; 32],
            1,
            [0xA2; 32],
            [0; 33],
            2026,
            7,
            28,
            18,
            [0xB4; 32],
            [0xC3; 32],
        )
        .is_err()
    );
}

#[test]
fn vega_python_preparation_freezes_nonzero_intent_and_is_single_use() {
    ensure_python();
    let authority = canonical_i105_from_seed(0x51);
    let issuer_public_key = *VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32)
        .expect("P-256 parameters")
        .value_generator()
        .as_bytes();
    Python::attach(|py| {
        let builder =
            TransactionBuilder::new("test-chain", &authority, authority_fee_payment_json())
                .expect("builder constructs");
        let mut preparation = builder
            .prepare_privacy_vega_action_v1(
                &[0xA5; 32],
                &[0xA1; 32],
                1,
                &[0xA2; 32],
                &issuer_public_key,
                2026,
                7,
                28,
                18,
                &[0xB4; 32],
                &[0xC3; 32],
            )
            .expect("valid Vega preparation");
        let prepared = preparation
            .inner
            .as_ref()
            .expect("preparation remains available");
        assert_ne!(
            prepared
                .statement
                .context
                .transaction_intent_digest
                .as_bytes(),
            &[0; 32]
        );
        assert_ne!(
            prepared.statement.device_authentication_digest.as_bytes(),
            &[0; 32]
        );

        assert!(
            preparation
                .finalize_privacy_vega_action_v1(
                    py,
                    &[0],
                    0,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
                .is_err()
        );
        assert!(preparation.consumed());
        let replay = preparation
            .finalize_privacy_vega_action_v1(
                py,
                &[0],
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .err()
            .expect("consumed preparation must reject replay");
        assert!(replay.to_string().contains("already been consumed"));
    });
}

#[test]
fn component_python_builders_reject_adversarial_shapes_before_proving() {
    ensure_python();
    let authority = canonical_i105_from_seed(0x51);
    Python::attach(|py| {
        let new_builder = || {
            TransactionBuilder::new("test-chain", &authority, authority_fee_payment_json())
                .expect("builder constructs")
        };
        let mut empty = new_builder();
        assert!(
            empty
                .sign_privacy_verange_action_v1(
                    py,
                    &[0; 32],
                    &[0xA5; 32],
                    "asset#domain",
                    &[0x11; 32],
                    64,
                    Vec::new(),
                    Vec::new(),
                )
                .err()
                .expect("empty VeRange witness must reject")
                .to_string()
                .contains("between 1 and 8")
        );
        let mut mismatched = new_builder();
        assert!(
            mismatched
                .sign_privacy_verange_action_v1(
                    py,
                    &[0; 32],
                    &[0xA5; 32],
                    "asset#domain",
                    &[0x11; 32],
                    64,
                    vec![1],
                    Vec::new(),
                )
                .err()
                .expect("mismatched VeRange witness must reject")
                .to_string()
                .contains("matching lengths")
        );
        let mut wrong_width = new_builder();
        assert!(
            wrong_width
                .sign_privacy_verange_action_v1(
                    py,
                    &[0; 32],
                    &[0xA5; 32],
                    "asset#domain",
                    &[0x11; 32],
                    48,
                    vec![1],
                    vec![vec![1; 32]],
                )
                .err()
                .expect("unsupported VeRange width must reject")
                .to_string()
                .contains("exactly 32 or 64")
        );
        let mut out_of_range = new_builder();
        assert!(
            out_of_range
                .sign_privacy_verange_action_v1(
                    py,
                    &[0; 32],
                    &[0xA5; 32],
                    "asset#domain",
                    &[0x11; 32],
                    32,
                    vec![u64::from(u32::MAX) + 1],
                    vec![vec![1; 32]],
                )
                .err()
                .expect("out-of-range VeRange value must reject")
                .to_string()
                .contains("[0, 2^32)")
        );

        let mut mixed = new_builder();
        mixed
            .add_instruction(&batch_test_instruction("forged prefix"))
            .expect("instruction");
        assert!(
            mixed
                .sign_privacy_verange_action_v1(
                    py,
                    &[0; 32],
                    &[0xA5; 32],
                    "asset#domain",
                    &[0x11; 32],
                    64,
                    vec![1],
                    vec![vec![1; 32]],
                )
                .err()
                .expect("mixed action builder must reject")
                .to_string()
                .contains("otherwise empty")
        );
    });
}

#[test]
fn component_python_inspectors_reject_empty_and_malformed_signed_wire() {
    ensure_python();
    Python::attach(|py| {
        for malformed in [&[][..], &[0xA5, 0x5A][..]] {
            assert!(inspect_signed_privacy_verange_action_v1_py(py, malformed).is_err());
            assert!(inspect_signed_privacy_vega_action_v1_py(py, malformed).is_err());
            assert!(
                inspect_signed_privacy_zk_x509_identity_presentation_action_v1_py(
                    py,
                    malformed,
                    &[0xA5; 32],
                )
                .is_err()
            );
            assert!(
                inspect_signed_privacy_zk_ams_batch_admission_action_v1_py(py, malformed).is_err()
            );
            assert!(
                inspect_signed_privacy_zk_ams_provision_account_action_v1_py(py, malformed)
                    .is_err()
            );
            assert!(
                inspect_signed_privacy_bootle_lantern_presentation_action_v1_py(py, malformed)
                    .is_err()
            );
        }
    });
}

#[test]
fn x509_statement_archive_boundary_is_nonempty_fixed_capacity_and_canonical() {
    assert!(python_zk_x509_statement_archive_v1(&[]).is_err());
    assert!(python_zk_x509_statement_archive_v1(&[0xA5]).is_err());
    assert!(
        python_zk_x509_statement_archive_v1(&vec![
            0xA5;
            crate::privacy_native_actions::PRIVACY_ZK_X509_MAX_STATEMENT_ARCHIVE_BYTES_V1
                + 1
        ])
        .is_err()
    );
}

#[test]
fn wave2_python_results_keep_exact_action_and_ledger_semantics() {
    assert_eq!(
        ZK_AMS_ACTION_EXECUTION_CLASSIFICATION_V1,
        "admission_action"
    );
    assert_eq!(
        ZK_AMS_BATCH_ADMISSION_LEDGER_EFFECT_V1,
        "zk_ams_batch_admission"
    );
    assert_eq!(
        ZK_AMS_PROVISION_ACCOUNT_LEDGER_EFFECT_V1,
        "zk_ams_provision_account"
    );
    assert_eq!(
        BOOTLE_LANTERN_ACTION_EXECUTION_CLASSIFICATION_V1,
        "presentation_action"
    );
    assert_eq!(
        ZK_X509_ACTION_EXECUTION_CLASSIFICATION_V1,
        "presentation_action"
    );
    assert_eq!(ZK_X509_LEDGER_EFFECT_V1, "zk_x509_certificate_nullifier");
}

#[test]
fn wave2_python_owned_witness_buffers_are_explicitly_erased() {
    let mut batch = ZeroizingZkAmsBatchWitnessBytes {
        subject_commitments: vec![vec![0x11; 32]],
        credential_nonces: vec![vec![0x22; 32]],
        seed_secrets: vec![vec![0x33; 32]],
        issuer_signatures: vec![vec![0x44; 64]],
    };
    batch.erase();
    assert!(
        batch
            .subject_commitments
            .iter()
            .chain(&batch.credential_nonces)
            .chain(&batch.seed_secrets)
            .chain(&batch.issuer_signatures)
            .flatten()
            .all(|byte| *byte == 0)
    );

    let mut provision = ZeroizingZkAmsProvisionWitnessBytes(vec![0x55; 32]);
    provision.erase();
    assert!(provision.0.iter().all(|byte| *byte == 0));

    let mut bootle = ZeroizingBootleLanternWitnessBytes {
        secret_polynomials: vec![vec![7; 64]; BOOTLE_LANTERN_SECRET_POLYNOMIALS_V1],
        attributes: vec![vec![0x66; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
    };
    bootle.erase();
    assert!(
        bootle
            .secret_polynomials
            .iter()
            .flatten()
            .all(|coefficient| *coefficient == 0)
    );
    assert!(bootle.attributes.iter().flatten().all(|byte| *byte == 0));
}

#[test]
fn zk_ams_python_governance_rejects_substituted_artifact_digests() {
    let issuer_id = PrivacyIssuerIdV1::new([0x11; 32]);
    let issuer_public_key = PrivacyP256PointV1::new(
        *VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32)
            .expect("P-256 parameters")
            .value_generator()
            .as_bytes(),
    );
    let registry_id = PrivacyZkAmsRegistryIdV1::new([0x22; 32]);
    let policy_id = PrivacyPolicyIdV1::new([0x33; 32]);
    let policy_digest = PrivacyPolicyDigestV1::new([0x44; 32]);
    let root = PrivacyRootV1::new([0x55; 32]);
    let issuer_record = zk_ams_issuer_policy_record_digest_v1(
        issuer_id,
        policy_id,
        issuer_public_key,
        policy_digest,
    );
    let registry_record = zk_ams_registry_record_digest_v1(
        issuer_id,
        registry_id,
        policy_id,
        issuer_record,
        policy_digest,
        root,
        7,
    );
    let parse = |issuer_record_bytes: &[u8], registry_record_bytes: &[u8], epoch| {
        python_zk_ams_governance_v1(
            issuer_id.as_bytes(),
            issuer_public_key.as_bytes(),
            issuer_record_bytes,
            registry_id.as_bytes(),
            registry_record_bytes,
            policy_id.as_bytes(),
            policy_digest.as_bytes(),
            root.as_bytes(),
            epoch,
        )
    };
    assert!(parse(issuer_record.as_bytes(), registry_record.as_bytes(), 7).is_ok());

    let mut substituted_issuer = *issuer_record.as_bytes();
    substituted_issuer[0] ^= 1;
    assert!(parse(&substituted_issuer, registry_record.as_bytes(), 7).is_err());
    let mut substituted_registry = *registry_record.as_bytes();
    substituted_registry[0] ^= 1;
    assert!(parse(issuer_record.as_bytes(), &substituted_registry, 7).is_err());
    assert!(parse(issuer_record.as_bytes(), registry_record.as_bytes(), 0).is_err());
    assert!(parse(issuer_record.as_bytes(), registry_record.as_bytes(), 8).is_err());
}

#[test]
fn bootle_python_polynomial_boundary_is_exact_and_erases_rejected_rows() {
    let mut valid = vec![vec![0_u16; BOOTLE_LANTERN_POLYNOMIAL_COEFFICIENTS_V1]; 8];
    assert!(python_bootle_lantern_polynomials_v1::<8>(&mut valid, "polynomials").is_ok());
    assert!(valid.iter().flatten().all(|coefficient| *coefficient == 0));

    let mut noncanonical = vec![vec![0_u16; BOOTLE_LANTERN_POLYNOMIAL_COEFFICIENTS_V1]; 8];
    noncanonical[3][9] = 12_289;
    assert!(python_bootle_lantern_polynomials_v1::<8>(&mut noncanonical, "polynomials").is_err());
    assert!(noncanonical[3].iter().all(|coefficient| *coefficient == 0));

    let mut wrong_count = vec![vec![0_u16; BOOTLE_LANTERN_POLYNOMIAL_COEFFICIENTS_V1]; 7];
    assert!(python_bootle_lantern_polynomials_v1::<8>(&mut wrong_count, "polynomials").is_err());
}

#[test]
fn jindo_python_builder_binds_native_and_submitted_encoding_metrics() {
    ensure_python();
    let signing = SigningKey::from_bytes(&[0x31; 32]);
    let private_key = parse_private_key(signing.as_bytes()).expect("ed25519 private key parses");
    let authority = AccountId::new(PublicKey::from(private_key))
        .canonical_i105()
        .expect("canonical I105 authority");
    let mut coefficient = vec![0_u8; 32];
    coefficient[0] = 7;
    let mut evaluation_point = [0_u8; 32];
    evaluation_point[0] = 3;

    Python::attach(|py| {
        let mut builder =
            TransactionBuilder::new("test-chain", &authority, authority_fee_payment_json())
                .expect("builder constructs");
        let result = builder
            .sign_privacy_jindo_action_v1(
                py,
                signing.as_bytes(),
                &[0xA7; 32],
                vec![vec![coefficient]],
                &evaluation_point,
            )
            .expect("native Jindo action builds");
        let envelope = result.envelope.bind(py);
        let adaptive = envelope
            .getattr("signed_transaction")
            .expect("adaptive encoding getter")
            .extract::<Vec<u8>>()
            .expect("adaptive encoding bytes");
        let submitted_versioned = envelope
            .getattr("signed_transaction_versioned")
            .expect("versioned encoding getter")
            .extract::<Vec<u8>>()
            .expect("versioned encoding bytes");

        assert_eq!(
            result.adaptive_signed_transaction_bytes,
            u32::try_from(adaptive.len()).expect("bounded adaptive encoding")
        );
        assert!(!submitted_versioned.is_empty());
        assert_eq!(
            canonical_signed_transaction_hash_v1(&submitted_versioned)
                .expect("submitted encoding authenticates"),
            <[u8; 32]>::try_from(
                result
                    .envelope
                    .bind(py)
                    .getattr("hash")
                    .expect("hash getter")
                    .extract::<Vec<u8>>()
                    .expect("hash bytes"),
            )
            .expect("hash is exactly 32 bytes")
        );
        assert!(result.statement_bytes > 0);
        assert!(result.proof_bytes > 0);
        assert!(result.encoded_proof_envelope_bytes >= result.proof_bytes);
        assert_eq!(
            result.execution_classification(),
            "action_verification_and_finality_only"
        );
        assert_eq!(result.ledger_effect(), None);
    });
}
