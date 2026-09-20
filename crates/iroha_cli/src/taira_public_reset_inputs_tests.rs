use super::*;

fn faucet_config_fixture(policy: &FaucetPolicyV1) -> String {
    format!(
        "[torii.faucet]\nenabled = true\nauthority = {:?}\nprivate_key_file = \"/fixture-secret/nonexistent-faucet-key\"\nasset_definition_id = {:?}\namount = {:?}\n",
        policy.authority,
        policy.asset_definition_id,
        policy.amount.to_string(),
    )
}

fn mismatched_faucet_configs(policy: &FaucetPolicyV1) -> Vec<(&'static str, String, &'static str)> {
    let config = faucet_config_fixture(policy);
    let other_key = KeyPair::from_seed(
        b"distinct faucet policy fixture".to_vec(),
        Algorithm::Ed25519,
    );
    let other_authority = AccountId::new(other_key.public_key().clone()).to_string();
    let other_asset = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("feetest", "universal").unwrap(),
        "other".parse().unwrap(),
    )
    .to_string();
    assert_ne!(policy.authority, other_authority);
    assert_ne!(policy.asset_definition_id, other_asset);
    vec![
        (
            "disabled",
            config.replace("enabled = true", "enabled = false"),
            "validator faucet must be enabled for public reset",
        ),
        (
            "authority",
            config.replace(&policy.authority, &other_authority),
            "validator faucet authority differs from signed intent",
        ),
        (
            "asset",
            config.replace(&policy.asset_definition_id, &other_asset),
            "validator faucet asset differs from signed intent",
        ),
        (
            "amount",
            config.replace(
                &format!("amount = {:?}", policy.amount.to_string()),
                "amount = \"1\"",
            ),
            "validator faucet amount differs from signed intent",
        ),
    ]
}

#[test]
fn validator_faucet_policy_requires_enabled_exact_signed_intent() {
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let policy = sample_inventory_fixture().faucet_policy;
    let config = faucet_config_fixture(&policy);
    validate_validator_faucet_config(config.as_bytes(), &policy)
        .expect("matching public policy needs no signer file");
    validate_validator_faucet_config(config.replace("enabled = true\n", "").as_bytes(), &policy)
        .expect("preserve the native faucet enabled default");
    for (label, invalid, expected_error) in mismatched_faucet_configs(&policy) {
        let error = validate_validator_faucet_config(invalid.as_bytes(), &policy).unwrap_err();
        assert_eq!(error.to_string(), expected_error, "{label}");
        assert!(!format!("{error:#}").contains("fixture-secret"), "{label}");
    }
    for invalid in [
        String::new(),
        format!("extends = [\"/fixture-secret/unbound.toml\"]\n{config}"),
        "[torii.faucet]\nauthority = \"fixture-secret\"\namount = [".to_owned(),
        config.replace(&policy.authority, "fixture-secret-invalid-authority"),
        config.replace(&policy.asset_definition_id, "fixture-secret-invalid-asset"),
        config.replace(
            &format!("amount = {:?}", policy.amount.to_string()),
            "amount = \"fixture-secret-invalid-quantity\"",
        ),
    ] {
        let error = validate_validator_faucet_config(invalid.as_bytes(), &policy).unwrap_err();
        assert!(!format!("{error:#}").contains("fixture-secret"));
    }
    let mut invalid_intent = policy.clone();
    invalid_intent.authority = "fixture-secret-invalid-intent".to_owned();
    let error = validate_validator_faucet_config(config.as_bytes(), &invalid_intent).unwrap_err();
    assert_eq!(
        error.to_string(),
        "signed faucet policy failed canonical admission"
    );
    assert!(!format!("{error:#}").contains("fixture-secret"));
    let mut zero_intent = policy;
    zero_intent.amount = Quantity::from(0_u32);
    let zero_config = faucet_config_fixture(&zero_intent);
    assert!(validate_validator_faucet_config(zero_config.as_bytes(), &zero_intent).is_err());
}

#[test]
#[cfg(unix)]
fn pinned_validator_configs_reject_faucet_policy_mismatch_before_dispatch() {
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let inventory = sample_inventory_fixture();
    let network = iroha::data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(inventory.next_genesis_hash.parse().unwrap()),
    );
    let matching = faucet_config_fixture(&inventory.faucet_policy);
    let mut cases = vec![("matching", matching.clone(), "")];
    cases.extend(mismatched_faucet_configs(&inventory.faucet_policy));
    for (label, selected_policy, expected_error) in cases {
        let directory = private_custody_test_dir("taira-pinned-faucet-");
        let root = directory.path().canonicalize().unwrap();
        let pinned = inventory.validators.iter().enumerate().map(|(index, validator)| {
            let genesis = artifact(&validator.artifacts, "genesis").unwrap();
            let policy = if index == 2 { &selected_policy } else { &matching };
            let config = format!(
                "[genesis]\nfile = {:?}\nexpected_hash = {:?}\n[torii.operator_signatures]\nenabled = true\nallowed_public_keys = [{:?}]\n{policy}",
                genesis.remote_path, network.to_string(), inventory.operator_public_key,
            );
            let path = root.join(format!("{}.toml", validator.slug));
            write_new_private(&path, config.as_bytes()).unwrap();
            let (file, snapshot) = open_pinned_regular(&path, "test validator config").unwrap();
            let mut entry = artifact(&validator.artifacts, "config").unwrap().clone();
            entry.local_path = path.to_str().unwrap().to_owned();
            entry.sha256 = sha256_hex(config.as_bytes());
            entry.size = config.len().try_into().unwrap();
            PinnedArtifact {
                slug: validator.slug.clone(),
                role: "config".to_owned(),
                artifact: entry,
                input: PinnedInput { path, file, snapshot },
            }
        }).collect::<Vec<_>>();
        // This is the admission seam shared by preflight and fresh apply, before
        // any transport is created. A later validator cannot evade the policy join.
        let result = validate_pinned_validator_genesis_configs(&inventory, &pinned);
        if label == "matching" {
            result.expect("all four held configs match independently signed policy");
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.to_string(), expected_error, "{label}");
            assert!(!format!("{error:#}").contains("fixture-secret"), "{label}");
        }
    }
}

#[test]
fn validator_pin_fee_asset_must_match_the_typed_faucet_funding_asset() {
    let inventory = sample_inventory_fixture();
    let faucet = inventory.faucet_policy.asset_definition_id.parse().unwrap();
    validate_validator_pin_fee_asset(&faucet, &inventory.faucet_policy.asset_definition_id)
        .expect("faucet funds the exact validator pin-fee asset");
    let other = iroha::data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("feetest", "universal").unwrap(),
        "other".parse().unwrap(),
    );
    assert_ne!(faucet, other);
    assert!(
        validate_validator_pin_fee_asset(&other, &inventory.faucet_policy.asset_definition_id)
            .is_err()
    );
    let error =
        validate_validator_pin_fee_asset(&faucet, "fixture-secret-not-runtime").unwrap_err();
    assert!(!format!("{error:#}").contains("fixture-secret"));
}

fn owner() -> (KeyPair, TrustedKeyV1) {
    let key = KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("test owner");
    let trusted = TrustedKeyV1 {
        schema: TRUSTED_KEY_SCHEMA_V1.to_owned(),
        algorithm: "ed25519".to_owned(),
        public_key: key.public_key().to_string(),
    };
    (key, trusted)
}

#[test]
fn owner_authorization_uses_real_norito_claims_and_existing_admission_verifier() {
    let inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).expect("inventory");
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, 1_000_000).expect("sign");
    assert_eq!(envelope.claims.inventory_sha256, sha256_hex(&bytes));
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms,
        1_000_000 + execution_lifetime_ms(&inventory).unwrap()
    );
    let wire = canonical_bytes(&envelope).expect("authorization");
    let decoded: AuthorizationEnvelopeV1 = json::from_slice(&wire).expect("typed authorization");
    verify_authorization(
        &inventory,
        &sha256_hex(&bytes),
        &decoded,
        &trusted,
        1_000_000,
    )
    .expect("real admission");
    let signature = ed25519_parse_signature(&hex::decode(&decoded.signature_hex).unwrap()).unwrap();
    // Independent concatenation checks the exact existing domain and Norito claim bytes.
    let mut message = b"iroha:taira:public-reset:authorization:v1\0".to_vec();
    message.extend_from_slice(json::to_json(&decoded.claims).unwrap().as_bytes());
    verify_signature_for_admission(&signature, key.public_key(), &message)
        .expect("exact endorsement bytes");
    assert!(
        verify_signature_for_admission(
            &signature,
            key.public_key(),
            json::to_json(&decoded.claims).unwrap().as_bytes()
        )
        .is_err()
    );
}

#[test]
fn inventory_reformatting_and_claim_tampering_invalidate_authorization() {
    let inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).unwrap();
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, 1_000_000).unwrap();
    let mut reformatted = bytes.clone();
    reformatted.push(b'\n');
    assert!(
        verify_authorization(
            &inventory,
            &sha256_hex(&reformatted),
            &envelope,
            &trusted,
            1_000_000
        )
        .is_err()
    );
    let mut tampered = envelope.clone();
    tampered.claims.fee_intent.payer.push('x');
    assert!(
        verify_authorization(
            &inventory,
            &sha256_hex(&bytes),
            &tampered,
            &trusted,
            1_000_000
        )
        .is_err()
    );
    let mut tampered = envelope.clone();
    tampered.claims.execution_expires_at_unix_ms += 1;
    assert!(
        verify_authorization(
            &inventory,
            &sha256_hex(&bytes),
            &tampered,
            &trusted,
            1_000_000
        )
        .is_err()
    );
    let mut tampered = envelope;
    let replacement = if tampered.signature_hex.starts_with('0') {
        "1"
    } else {
        "0"
    };
    tampered.signature_hex.replace_range(..1, replacement);
    assert!(
        verify_authorization(
            &inventory,
            &sha256_hex(&bytes),
            &tampered,
            &trusted,
            1_000_000
        )
        .is_err()
    );
}

#[test]
fn independently_trusted_owner_key_cannot_be_replaced_by_signer() {
    let inventory = sample_inventory_fixture();
    let (key, _) = owner();
    let (_, other_trusted) = owner();
    assert!(
        sign_inventory(
            &inventory,
            &canonical_inventory_bytes(&inventory).unwrap(),
            &other_trusted,
            &key,
            1_000_000
        )
        .is_err()
    );
    let mut wrong_algorithm = other_trusted;
    wrong_algorithm.algorithm = "secp256k1".to_owned();
    assert!(trusted_key(&wrong_algorithm).is_err());
}

#[test]
fn authorization_cannot_extend_the_bounded_plan() {
    let mut inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    let issued_at = 1_000_000;
    inventory.timeouts.install_secs = 600;
    let envelope = sign_inventory(
        &inventory,
        &canonical_inventory_bytes(&inventory).unwrap(),
        &trusted,
        &key,
        issued_at,
    )
    .expect("the maximum install timeout fits within the complete execution budget");
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms - issued_at,
        36_270_000,
    );

    // One physical host requires exactly 42,000 action seconds, plus the
    // fifteen-minute admission window and five-minute safety margin.
    inventory.timeouts = TimeoutsV1 {
        epoch_supervisor_pause_secs: 2,
        epoch_supervisor_start_secs: 1,
        stop_secs: 2,
        install_secs: 600,
        reset_secs: 1,
        preseed_secs: 3_599,
        start_secs: 1,
        convergence_secs: 1,
        canary_secs: 193,
        restart_secs: 1,
        edge_secs: 1,
        cleanup_secs: 3,
        rollback_secs: 1,
    };
    validate_timeouts(&inventory.timeouts).expect("every individual timeout is legal");
    let bytes = canonical_inventory_bytes(&inventory).unwrap();
    let mut envelope = sign_inventory(&inventory, &bytes, &trusted, &key, issued_at)
        .expect("the exact twelve-hour execution limit is signable");
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms - issued_at,
        MAX_EXECUTION_LIFETIME_MS,
    );
    envelope.claims.execution_expires_at_unix_ms += 1;
    let signature = Signature::try_new(
        key.private_key(),
        &authorization_message(&envelope.claims).unwrap(),
    )
    .unwrap();
    envelope.signature_hex = hex::encode(signature.payload());
    assert_eq!(
        verify_authorization(
            &inventory,
            &sha256_hex(&bytes),
            &envelope,
            &trusted,
            issued_at
        )
        .expect_err("even the trusted owner cannot sign a longer execution lease")
        .to_string(),
        "authorization execution lease does not exactly cover the bounded execution plan",
    );

    inventory.timeouts.preseed_secs += 1;
    validate_timeouts(&inventory.timeouts).expect("the longer preseed timeout remains legal");
    assert_eq!(
        sign_inventory(
            &inventory,
            &canonical_inventory_bytes(&inventory).unwrap(),
            &trusted,
            &key,
            issued_at,
        )
        .expect_err("one additional preseed second exceeds the complete execution budget")
        .to_string(),
        "bounded execution plan requires 43202 seconds (actions: 42002 seconds, admission: 900 seconds, safety: 300 seconds), exceeding the 43200-second limit by 2 seconds",
    );
}

#[test]
fn authorization_cannot_overflow_admission_or_execution_expiry() {
    let inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).unwrap();
    let last_issued_at = u64::MAX - execution_lifetime_ms(&inventory).unwrap();
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, last_issued_at)
        .expect("the last representable execution expiry is signable");
    assert_eq!(envelope.claims.execution_expires_at_unix_ms, u64::MAX);
    assert_eq!(
        sign_inventory(&inventory, &bytes, &trusted, &key, last_issued_at + 1,)
            .expect_err("one millisecond later overflows the execution expiry")
            .to_string(),
        "execution expiry overflow",
    );
    assert_eq!(
        sign_inventory(&inventory, &bytes, &trusted, &key, u64::MAX)
            .expect_err("the admission expiry must also use checked arithmetic")
            .to_string(),
        "authorization expiry overflow",
    );
}

#[cfg(unix)]
fn private_directory() -> tempfile::TempDir {
    let parent = tempfile::tempdir_in(std::env::current_dir().unwrap()).expect("temporary parent");
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
    parent
}

#[cfg(unix)]
#[test]
fn inherited_owner_key_is_bounded_private_and_matches_the_independent_public_key() {
    use std::os::fd::AsRawFd as _;
    let directory = private_directory();
    let path = directory.path().join("key");
    let (key, _) = owner();
    let encoded =
        Zeroizing::new(iroha_crypto::ExposedPrivateKey(key.private_key().clone()).to_string());
    fs::write(&path, encoded.as_bytes()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let file = File::open(&path).unwrap();
    let fd = u32::try_from(file.as_raw_fd()).unwrap();
    assert_eq!(
        inherited_signing_key(fd, key.public_key())
            .unwrap()
            .public_key(),
        key.public_key()
    );
    let (other, _) = owner();
    assert!(inherited_signing_key(fd, other.public_key()).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(inherited_signing_key(fd, key.public_key()).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, vec![b'a'; 513]).unwrap();
    assert!(inherited_signing_key(fd, key.public_key()).is_err());
    assert!(inherited_signing_key(0, key.public_key()).is_err());
}

#[cfg(unix)]
#[test]
fn signing_key_rejects_hardlinks_and_nonregular_descriptors_without_reading_them() {
    use std::os::fd::AsRawFd as _;
    let directory = private_directory();
    let path = directory.path().join("key");
    let (key, _) = owner();
    fs::write(&path, b"invalid-test-key").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(&path, directory.path().join("alias")).unwrap();
    let file = File::open(&path).unwrap();
    assert!(inherited_signing_key(file.as_raw_fd() as u32, key.public_key()).is_err());
    let file = File::open(directory.path()).unwrap();
    assert!(inherited_signing_key(file.as_raw_fd() as u32, key.public_key()).is_err());
}

#[cfg(unix)]
#[test]
fn fresh_output_is_private_durable_and_never_overwrites_or_follows_a_symlink() {
    use std::os::unix::fs::symlink;
    let directory = private_directory();
    // macOS's system temporary root can itself be an alias; normalize only this test root.
    let parent = directory.path().canonicalize().unwrap();
    let output = parent.join("inventory.json");
    write_new_private(&output, b"first\n").unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(write_new_private(&output, b"replacement\n").is_err());
    assert_eq!(fs::read(&output).unwrap(), b"first\n");
    let alias = parent.join("alias.json");
    symlink(&output, &alias).unwrap();
    assert!(write_new_private(&alias, b"replacement\n").is_err());
    assert_eq!(fs::read(&output).unwrap(), b"first\n");
}

#[test]
fn taira_accepts_signed_npos_mode_and_rejects_permissioned_mode() {
    use iroha::data_model::parameter::system::SumeragiConsensusMode;
    validate_taira_genesis_mode(SumeragiConsensusMode::Npos).expect("canonical Taira mode");
    assert!(validate_taira_genesis_mode(SumeragiConsensusMode::Permissioned).is_err());
}

#[cfg(unix)]
#[test]
fn assembler_rejects_incomplete_topology_before_reading_runtime_inputs() {
    let directory = private_custody_test_dir("taira-incomplete-topology-");
    let absent = directory.path().join("absent");
    let mut inventory = sample_inventory_fixture();
    inventory.validators.pop();
    let inputs = LocalInputs {
        public_inputs: absent.join("public-inputs"),
        beacon_inputs: absent.join("beacon-inputs.json"),
        beacon_validator_unit: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.beacon.service")))
            .collect(),
        runtime_client_config: absent.join("runtime-client.toml"),
        maintenance_admin_config: absent.join("maintenance-admin.toml"),
        epoch_seed_sources: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.seed")))
            .collect(),
        epoch_supervisor_plan: absent.join("epoch-supervisor.json"),
        validator_client_config: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.toml")))
            .collect(),
        onboarding_token: absent.join("onboarding-token"),
        validator_operator_key: absent.join("operator.key"),
        inrou_stage_dir: Some(absent.join("stage")),
        validator_unit: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.service")))
            .collect(),
        edge_unit: absent.join("edge.service"),
        known_hosts: absent.join("known-hosts"),
    };
    let error = derive_inventory(&mut inventory, &inputs)
        .expect_err("incomplete topology must fail before opening absent runtime inputs")
        .to_string();
    assert_eq!(
        error,
        "native reset context requires four exact validator inputs"
    );
    assert!(!absent.exists());
}

#[cfg(unix)]
#[test]
fn aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads() {
    let directory = private_custody_test_dir("taira-timeout-inputs-");
    let root = directory.path().canonicalize().expect("direct test root");
    let absent = root.join("absent");
    let mut inventory = sample_inventory_fixture();
    inventory.timeouts = TimeoutsV1 {
        epoch_supervisor_pause_secs: 600,
        epoch_supervisor_start_secs: 600,
        stop_secs: 600,
        install_secs: 600,
        reset_secs: 600,
        preseed_secs: 3_600,
        start_secs: 600,
        convergence_secs: 600,
        canary_secs: 600,
        restart_secs: 600,
        edge_secs: 600,
        cleanup_secs: 600,
        rollback_secs: 600,
    };
    validate_timeouts(&inventory.timeouts).expect("every individual timeout is legal");
    inventory.revision.source_root = absent.join("source").display().to_string();
    inventory.revision.source_manifest_path = absent.join("source.json").display().to_string();
    for artifact in inventory
        .validators
        .iter_mut()
        .flat_map(|validator| validator.artifacts.iter_mut())
        .chain(inventory.edge.artifacts.iter_mut())
    {
        artifact.local_path = absent.join(&artifact.role).display().to_string();
    }
    let local = || LocalInputs {
        public_inputs: absent.join("public-inputs"),
        beacon_inputs: absent.join("beacon-inputs.json"),
        beacon_validator_unit: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.beacon.service")))
            .collect(),
        runtime_client_config: absent.join("runtime-client.toml"),
        maintenance_admin_config: absent.join("maintenance-admin.toml"),
        epoch_seed_sources: Vec::new(),
        epoch_supervisor_plan: absent.join("epoch-supervisor.json"),
        validator_client_config: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.toml")))
            .collect(),
        onboarding_token: absent.join("onboarding-token"),
        validator_operator_key: absent.join("operator.key"),
        inrou_stage_dir: Some(absent.join("stage")),
        validator_unit: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.service")))
            .collect(),
        edge_unit: absent.join("edge.service"),
        known_hosts: absent.join("known-hosts"),
    };
    let expected = "bounded execution plan requires 85800 seconds (actions: 84600 seconds, admission: 900 seconds, safety: 300 seconds), exceeding the 43200-second limit by 42600 seconds";
    assert_eq!(
        derive_inventory(&mut inventory, &local())
            .expect_err("budget must fail before opening the absent source manifest")
            .to_string(),
        expected,
    );
    assert_eq!(
        validate_inventory_structure(&inventory)
            .expect_err("structural inventory admission must reject the same budget")
            .to_string(),
        expected,
    );

    let draft = root.join("draft.json");
    let topology = ResetTopologyIntentV1::from(&inventory);
    let draft_bytes = canonical_bytes(&topology).unwrap();
    write_new_private(&draft, &draft_bytes).expect("private topology intent");
    let signed_inventory = root.join("signed-inventory.json");
    write_new_private(
        &signed_inventory,
        &canonical_inventory_bytes(&inventory).unwrap(),
    )
    .unwrap();
    let output = root.join("inventory.json");
    assert_eq!(
        assemble(&Assemble {
            intent: draft.clone(),
            local: local(),
            output: output.clone(),
        })
        .expect_err("assembly must reject before reading absent runtime inputs")
        .to_string(),
        expected,
    );
    let authorization = root.join("authorization.json");
    let error = authorize(&Authorize {
        inventory: signed_inventory,
        local: local(),
        trusted_public_key: absent.join("owner-public-key.json"),
        signing_key_fd: 0,
        output: authorization.clone(),
    })
    .expect_err("authorization must reject before reading absent custody or invalid signer FD");
    assert_compiled_admission_error(&error, expected);
    assert!(!absent.exists());
    assert!(!output.exists());
    assert!(!authorization.exists());
}

#[test]
fn aggregate_timeout_policy_accepts_deployment_defaults_and_preserves_individual_bounds() {
    let mut inventory = sample_inventory_fixture();
    inventory.timeouts = TimeoutsV1 {
        epoch_supervisor_pause_secs: 60,
        epoch_supervisor_start_secs: 120,
        stop_secs: 60,
        install_secs: 90,
        reset_secs: 60,
        preseed_secs: 1_800,
        start_secs: 120,
        convergence_secs: 180,
        canary_secs: 120,
        restart_secs: 120,
        edge_secs: 60,
        cleanup_secs: 60,
        rollback_secs: 120,
    };
    validate_timeout_policy(&inventory).expect("complete deployment timeout policy");
    validate_inventory_structure(&inventory)
        .expect("deployment defaults pass structural admission");
    assert_eq!(
        execution_lifetime_ms(&inventory).expect("bounded deployment lease"),
        17_820_000,
    );
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).expect("inventory");
    let issued_at = 1_000_000;
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, issued_at)
        .expect("admitted defaults are signable under the same budget");
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms - issued_at,
        17_820_000,
    );

    inventory.timeouts.stop_secs = 0;
    assert_eq!(
        validate_timeout_policy(&inventory)
            .expect_err("aggregate admission must retain the individual lower bound")
            .to_string(),
        "stop timeout must be within 1..=600 seconds",
    );
    inventory.timeouts.stop_secs = 601;
    assert_eq!(
        validate_timeout_policy(&inventory)
            .expect_err("aggregate admission must retain the individual upper bound")
            .to_string(),
        "stop timeout must be within 1..=600 seconds",
    );
}

#[test]
fn preseed_timeout_is_required_and_has_its_own_physical_work_bound() {
    let mut inventory = sample_inventory_fixture();
    let mut encoded = json::to_value(&inventory.timeouts).expect("encode timeout policy");
    encoded
        .as_object_mut()
        .expect("timeout object")
        .remove("preseed_secs");
    assert!(
        json::from_value::<TimeoutsV1>(encoded).is_err(),
        "old timeout policy must not silently borrow reset time"
    );
    for value in [0, 3_601, u64::MAX] {
        inventory.timeouts.preseed_secs = value;
        assert_eq!(
            validate_timeouts(&inventory.timeouts)
                .unwrap_err()
                .to_string(),
            "preseed timeout must be within 1..=3600 seconds"
        );
    }
    for value in [1, 1_800, 3_600] {
        inventory.timeouts.preseed_secs = value;
        validate_timeouts(&inventory.timeouts).expect("independent bounded preseed work");
    }
}

#[test]
fn candidate_runtime_scope_requires_disabled_core_and_exact_full_owner() {
    use iroha_config::parameters::actual::SoracloudRuntimeInrou;
    let mut runtime = SoracloudRuntimeInrou::default();
    runtime.enabled = false;
    assert!(
        validate_candidate_inrou_scope(
            QualificationScopeV1::CoreTestnet,
            "taira-validator-1",
            &runtime
        )
        .is_ok()
    );
    assert!(
        validate_candidate_inrou_scope(
            QualificationScopeV1::FullInrou,
            "taira-validator-1",
            &runtime
        )
        .is_err()
    );
    runtime.enabled = true;
    runtime.portable_vm_uid = std::num::NonZeroU32::new(70000);
    runtime.portable_vm_gid = std::num::NonZeroU32::new(70000);
    assert!(
        validate_candidate_inrou_scope(
            QualificationScopeV1::CoreTestnet,
            "taira-validator-1",
            &runtime
        )
        .is_err()
    );
    assert!(
        validate_candidate_inrou_scope(
            QualificationScopeV1::FullInrou,
            "taira-validator-1",
            &runtime
        )
        .is_ok()
    );
    runtime.portable_vm_gid = std::num::NonZeroU32::new(70001);
    assert!(
        validate_candidate_inrou_scope(
            QualificationScopeV1::FullInrou,
            "taira-validator-1",
            &runtime
        )
        .is_err()
    );
}

#[test]
fn topology_intent_forbids_generated_pins_and_plans() {
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let inventory = sample_inventory_fixture();
    let signed = canonical_inventory_bytes(&inventory).unwrap();
    assert!(decode_reset_topology_intent(&signed).is_err());
    let topology = ResetTopologyIntentV1::from(&inventory);
    let bytes = canonical_bytes(&topology).unwrap();
    let (decoded, _guard) = decode_reset_topology_intent(&bytes).unwrap();
    assert_eq!(canonical_bytes(&decoded).unwrap(), bytes);
    assert!(decode_inventory(&bytes, "topology intent").is_err());
    for field in [
        "beacon_bootstrap",
        "epoch_supervisor",
        "maintenance_admin_identity",
        "maintenance_admin_config_sha256",
        "operator_public_key",
        "runtime_client_config_sha256",
        "next_genesis_hash",
    ] {
        let mut value: json::Value = json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json::Value::Null);
        assert!(
            decode_reset_topology_intent(&canonical_bytes(&value).unwrap()).is_err(),
            "{field}"
        );
    }
    for field in [
        "commit",
        "tree",
        "cargo_lock_sha256",
        "source_manifest_sha256",
    ] {
        let mut value: json::Value = json::from_slice(&bytes).unwrap();
        value
            .get_mut("revision")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json::Value::Null);
        assert!(
            decode_reset_topology_intent(&canonical_bytes(&value).unwrap()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn topology_context_checks_scope_and_budget_before_custody() {
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let inventory = sample_inventory_fixture();
    let mut intent = ResetTopologyIntentV1::from(&inventory);
    intent.validators.pop();
    assert!(reset_context::validate_topology_intent(&intent).is_err());
    intent = ResetTopologyIntentV1::from(&inventory);
    intent.validators.swap(0, 1);
    assert!(reset_context::validate_topology_intent(&intent).is_err());
}

#[test]
fn maintenance_grant_requires_registration_and_survives_no_revocation() {
    use iroha_data_model::{
        account::Account,
        isi::{Grant, InstructionBox, Register, Revoke, Unregister},
        permission::Permission,
    };
    let key = KeyPair::from_seed(
        b"synthetic maintenance owner only".to_vec(),
        Algorithm::Ed25519,
    );
    let account = AccountId::new(key.public_key().clone());
    let permission = Permission::new(
        "CanSetParameters".to_owned(),
        iroha_primitives::json::Json::new(()),
    );
    let register: InstructionBox = Register::account(Account::new(account.clone())).into();
    let grant: InstructionBox =
        Grant::account_permission(permission.clone(), account.clone()).into();
    let revoke: InstructionBox = Revoke::account_permission(permission, account.clone()).into();
    let unregister: InstructionBox = Unregister::account(account.clone()).into();
    assert!(validate_maintenance_grant_instructions([&grant], &account).is_err());
    assert!(validate_maintenance_grant_instructions([&register], &account).is_err());
    validate_maintenance_grant_instructions([&register, &grant], &account)
        .expect("explicit registered grant");
    assert!(
        validate_maintenance_grant_instructions([&register, &grant, &revoke], &account).is_err()
    );
    assert!(
        validate_maintenance_grant_instructions([&register, &grant, &unregister], &account)
            .is_err()
    );
}

#[test]
fn ongoing_supervisor_authorization_is_explicit_and_separate_from_reset_expiry() {
    let inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).unwrap();
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, 1_000_000).unwrap();
    assert_eq!(
        envelope.claims.epoch_supervisor_authorization,
        "until_stopped"
    );
    assert_eq!(
        envelope.claims.epoch_supervisor_policy_sha256,
        inventory.epoch_supervisor.policy_sha256
    );
    let canonical = json::to_value(&envelope.claims).unwrap();
    for field in [
        "epoch_supervisor_authorization",
        "epoch_supervisor_policy_sha256",
        "maintenance_admin_config_sha256",
        "maintenance_admin_identity",
    ] {
        let mut missing = canonical.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(json::from_value::<AuthorizationClaimsV1>(missing).is_err());
    }
    let mut forged = envelope;
    forged.claims.epoch_supervisor_authorization = "until_reset_expires".to_owned();
    forged.signature_hex = hex::encode(
        Signature::try_new(
            key.private_key(),
            &authorization_message(&forged.claims).unwrap(),
        )
        .unwrap()
        .payload(),
    );
    assert!(
        verify_authorization(
            &inventory,
            &sha256_hex(&bytes),
            &forged,
            &trusted,
            1_000_000
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn native_context_rejects_scope_before_opening_actual_inputs() {
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let inventory = sample_inventory_fixture();
    let mut intent = ResetTopologyIntentV1::from(&inventory);
    intent.qualification_scope = QualificationScopeV1::CoreTestnet;
    let path = PathBuf::from("/absent-reset-context-inputs");
    let inputs = ResetContextInputs {
        public_inputs: path.clone(),
        runtime_client_config: path.clone(),
        maintenance_admin_config: path.clone(),
        validator_client_config: vec![path.clone(); 4],
        onboarding_token: path.clone(),
        validator_operator_key: path.clone(),
        inrou_stage_dir: Some(path.clone()),
        validator_unit: vec![path.clone(); 4],
        edge_unit: path.clone(),
        known_hosts: path,
    };
    let error = derive_reset_context(&intent, &inputs)
        .err()
        .expect("scope rejected before source/custody reads");
    assert!(
        error.to_string().contains("inrou") || error.to_string().contains("Inrou"),
        "{error}"
    );
}
