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
