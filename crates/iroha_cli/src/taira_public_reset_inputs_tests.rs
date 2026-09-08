use super::*;

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
        31_260_000,
    );

    // One physical host requires exactly 42,000 action seconds, plus the
    // fifteen-minute admission window and five-minute safety margin.
    inventory.timeouts = TimeoutsV1 {
        stop_secs: 2,
        install_secs: 600,
        reset_secs: 1,
        preseed_secs: 3_599,
        start_secs: 1,
        convergence_secs: 1,
        canary_secs: 323,
        restart_secs: 1,
        edge_secs: 1,
        cleanup_secs: 2,
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
        verify_authorization(&inventory, &sha256_hex(&bytes), &envelope, &trusted, issued_at)
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
        sign_inventory(
            &inventory,
            &bytes,
            &trusted,
            &key,
            last_issued_at + 1,
        )
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

#[test]
fn assembler_rejects_incomplete_topology_before_reading_runtime_inputs() {
    let mut inventory = sample_inventory_fixture();
    inventory.validators.pop();
    let inputs = LocalInputs {
        runtime_client_config: PathBuf::from("/missing"),
        validator_client_config: vec![],
        onboarding_token: PathBuf::from("/missing"),
        inrou_stage_dir: PathBuf::from("/missing"),
        validator_unit: vec![],
        edge_unit: PathBuf::from("/missing"),
        known_hosts: PathBuf::from("/missing"),
    };
    let error = derive_inventory(&mut inventory, &inputs)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "assembly requires exactly four ordered validator inputs"
    );
}

#[cfg(unix)]
#[test]
fn aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads() {
    let directory = private_custody_test_dir("taira-timeout-inputs-");
    let root = directory.path().canonicalize().expect("direct test root");
    let absent = root.join("absent");
    let mut inventory = sample_inventory_fixture();
    inventory.timeouts = TimeoutsV1 {
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
        runtime_client_config: absent.join("runtime-client.toml"),
        validator_client_config: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.toml")))
            .collect(),
        onboarding_token: absent.join("onboarding-token"),
        inrou_stage_dir: absent.join("stage"),
        validator_unit: VALIDATOR_SLUGS
            .iter()
            .map(|slug| absent.join(format!("{slug}.service")))
            .collect(),
        edge_unit: absent.join("edge.service"),
        known_hosts: absent.join("known-hosts"),
    };
    let expected = "bounded execution plan requires 78600 seconds (actions: 77400 seconds, admission: 900 seconds, safety: 300 seconds), exceeding the 43200-second limit by 35400 seconds";
    assert_eq!(
        derive_inventory(&mut inventory, &local())
            .expect_err("budget must fail before opening the absent source manifest")
            .to_string(),
        expected,
    );
    assert_eq!(
        validate_inventory(&inventory)
            .expect_err("signed inventory admission must reject the same budget")
            .to_string(),
        expected,
    );

    let draft = root.join("draft.json");
    write_new_private(
        &draft,
        &canonical_inventory_bytes(&inventory).expect("typed draft"),
    )
    .expect("private retained draft");
    let output = root.join("inventory.json");
    assert_eq!(
        assemble(&Assemble {
            inventory_draft: draft.clone(),
            local: local(),
            output: output.clone(),
        })
        .expect_err("assembly must reject before reading absent runtime inputs")
        .to_string(),
        expected,
    );
    let authorization = root.join("authorization.json");
    assert_eq!(
        authorize(&Authorize {
            inventory: draft,
            local: local(),
            trusted_public_key: absent.join("owner-public-key.json"),
            signing_key_fd: 0,
            output: authorization.clone(),
        })
        .expect_err("authorization must reject before reading absent custody or invalid signer FD")
        .to_string(),
        expected,
    );
    assert!(!absent.exists());
    assert!(!output.exists());
    assert!(!authorization.exists());
}

#[test]
fn aggregate_timeout_policy_accepts_deployment_defaults_and_preserves_individual_bounds() {
    let mut inventory = sample_inventory_fixture();
    inventory.timeouts = TimeoutsV1 {
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
    validate_inventory(&inventory).expect("deployment defaults pass structural admission");
    assert_eq!(
        execution_lifetime_ms(&inventory).expect("bounded deployment lease"),
        16_680_000,
    );
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).expect("inventory");
    let issued_at = 1_000_000;
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, issued_at)
        .expect("admitted defaults are signable under the same budget");
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms - issued_at,
        16_680_000,
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
