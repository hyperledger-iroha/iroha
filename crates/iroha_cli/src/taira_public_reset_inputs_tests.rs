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
fn authorization_cannot_extend_the_bounded_plan_or_overflow_time() {
    let mut inventory = sample_inventory_fixture();
    let (key, trusted) = owner();
    assert!(
        sign_inventory(
            &inventory,
            &canonical_inventory_bytes(&inventory).unwrap(),
            &trusted,
            &key,
            u64::MAX
        )
        .is_err()
    );
    inventory.timeouts.install_secs = 600;
    assert!(
        sign_inventory(
            &inventory,
            &canonical_inventory_bytes(&inventory).unwrap(),
            &trusted,
            &key,
            1_000_000
        )
        .is_err()
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
    let expected = "bounded execution plan requires 72000 seconds (actions: 70800 seconds, admission: 900 seconds, safety: 300 seconds), exceeding the 14400-second limit by 57600 seconds";
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
        13_140_000,
    );
    let (key, trusted) = owner();
    let bytes = canonical_inventory_bytes(&inventory).expect("inventory");
    let issued_at = 1_000_000;
    let envelope = sign_inventory(&inventory, &bytes, &trusted, &key, issued_at)
        .expect("admitted defaults are signable under the same budget");
    assert_eq!(
        envelope.claims.execution_expires_at_unix_ms - issued_at,
        13_140_000,
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
