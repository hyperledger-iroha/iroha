//! Public preparation tests use native signed genesis fixtures and owner-controlled ancestor custody.

use super::*;
use iroha_crypto::HashOf;
use iroha_data_model::parameter::system::SumeragiConsensusMode;

use crate::deployment_genesis_fixture::{Fixture, key};

fn line(value: impl std::fmt::Display) -> Vec<u8> {
    format!("{value}\n").into_bytes()
}

impl Fixture {
    fn network(&self) -> NetworkId {
        NetworkId::from_genesis_hash(self.block.hash())
    }

    /// Raw manifest bytes with Taira account literals, as the generator writes them.
    fn manifest_json(&self) -> Vec<u8> {
        let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        json_line(&self.manifest).unwrap()
    }

    fn derive(&self) -> Result<PublicInputsV1> {
        derive(
            &self.block.encode_wire().unwrap(),
            &self.manifest_json(),
            &line(self.network()),
            &line(self.genesis.public_key()),
            &line(self.canary.public_key()),
            &self.nexus_context,
        )
    }

    #[cfg(unix)]
    fn write(&self, root: &Path) -> PreparePublicInputs {
        let localnet = root.join("localnet");
        fs::create_dir(&localnet).unwrap();
        fs::set_permissions(&localnet, fs::Permissions::from_mode(0o700)).unwrap();
        for (name, bytes) in [
            ("genesis.signed.nrt", self.block.encode_wire().unwrap()),
            ("genesis.json", self.manifest_json()),
            ("nexus-amx-context.v1.bin", self.nexus_context.clone()),
            ("genesis.expected_hash", line(self.network())),
            ("genesis.public_key", line(self.genesis.public_key())),
            (
                "peer0.toml",
                crate::taira::parliament_seating::canonical_seating_config_toml().into_bytes(),
            ),
        ] {
            fs::write(localnet.join(name), bytes).unwrap();
        }
        let canary_public_key = root.join("canary.public_key");
        fs::write(&canary_public_key, line(self.canary.public_key())).unwrap();
        PreparePublicInputs {
            localnet_dir: localnet,
            canary_public_key: Some(canary_public_key),
            intent: None,
            output_dir: root.join("public-inputs"),
        }
    }
}

#[test]
fn derives_native_genesis_identity_and_exact_canary_request() {
    let fixture = Fixture::new();
    let report = fixture.derive().unwrap();
    assert_eq!(report.genesis_hash, fixture.block.hash().to_string());
    assert_ne!(report.genesis_hash, report.signed_genesis_sha256);
    assert_eq!(report.network_id, fixture.network());
    assert_eq!(
        report.canary_onboarding_request.alias,
        crate::taira::canary_alias(fixture.canary.public_key())
    );
    assert!(report.canary_onboarding_request.permissions.is_empty());
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    assert_eq!(
        report.canary_onboarding_request.account_id,
        AccountId::new(fixture.canary.public_key().clone()).to_string()
    );
    let decoded: PublicInputsV1 = json::from_slice(&json_line(&report).unwrap()).unwrap();
    assert_eq!(decoded, report);
}

#[test]
fn rejects_wrong_network_key_and_resultless_genesis() {
    let fixture = Fixture::new();
    let wire = fixture.block.encode_wire().unwrap();
    let wrong =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"wrong network")));
    assert!(
        derive(
            &wire,
            &json_line(&fixture.manifest).unwrap(),
            &line(wrong),
            &line(fixture.genesis.public_key()),
            &line(fixture.canary.public_key()),
            &fixture.nexus_context,
        )
        .unwrap_err()
        .to_string()
        .contains("differs from checked network")
    );
    assert!(
        derive(
            &wire,
            &json_line(&fixture.manifest).unwrap(),
            &line(fixture.network()),
            &line(fixture.canary.public_key()),
            &line(fixture.canary.public_key()),
            &fixture.nexus_context,
        )
        .is_err()
    );
    let resultless = fixture
        .block
        .canonical_resultless_proposal()
        .expect("valid original proposal")
        .encode_wire()
        .unwrap();
    assert!(
        derive(
            &resultless,
            &json_line(&fixture.manifest).unwrap(),
            &line(fixture.network()),
            &line(fixture.genesis.public_key()),
            &line(fixture.canary.public_key()),
            &fixture.nexus_context,
        )
        .is_err()
    );
}

#[test]
fn rejects_noncanonical_identity_and_non_ed25519_canary() {
    let fixture = Fixture::new();
    for text in [
        Vec::new(),
        b"invalid\n".to_vec(),
        line(fixture.network()).repeat(2),
        fixture.network().to_string().into_bytes(),
    ] {
        assert!(
            derive(
                &fixture.block.encode_wire().unwrap(),
                &json_line(&fixture.manifest).unwrap(),
                &text,
                &line(fixture.genesis.public_key()),
                &line(fixture.canary.public_key()),
                &fixture.nexus_context,
            )
            .is_err()
        );
    }
    let unsupported = key(104, Algorithm::Secp256k1);
    assert!(
        derive(
            &fixture.block.encode_wire().unwrap(),
            &json_line(&fixture.manifest).unwrap(),
            &line(fixture.network()),
            &line(fixture.genesis.public_key()),
            &line(unsupported.public_key()),
            &fixture.nexus_context,
        )
        .unwrap_err()
        .to_string()
        .contains("must use Ed25519")
    );
    assert!(canonical_public_key(b"not-a-key\n").is_err());
    assert!(canonical_line(&vec![b'a'; 1025]).is_err());
}

#[cfg(unix)]
#[test]
fn publishes_complete_public_bundle_and_reuses_identical_request() {
    let temp = private_custody_test_dir("taira-public-inputs-publish-");
    let root = temp.path();
    let fixture = Fixture::seated();
    let args = fixture.write(root);
    let original = fs::read(args.localnet_dir.join("genesis.signed.nrt")).unwrap();
    let mut first = Vec::new();
    prepare(&args, &mut first).unwrap();
    let before = fs::metadata(args.output_dir.join("public-inputs.json"))
        .unwrap()
        .ino();
    let mut second = Vec::new();
    prepare(&args, &mut second).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        before,
        fs::metadata(args.output_dir.join("public-inputs.json"))
            .unwrap()
            .ino()
    );
    assert_eq!(load(&args.output_dir).unwrap(), fixture.derive().unwrap());
    assert_eq!(
        fs::read(args.localnet_dir.join("genesis.signed.nrt")).unwrap(),
        original
    );
    for name in OUTPUT_FILES {
        assert_eq!(
            fs::metadata(args.output_dir.join(name)).unwrap().mode() & 0o7777,
            0o644
        );
    }
}

#[cfg(unix)]
#[test]
fn refuses_changed_bundle_and_never_overwrites_existing_output() {
    let temp = private_custody_test_dir("taira-public-inputs-refusal-");
    let args = Fixture::seated().write(temp.path());
    prepare(&args, &mut Vec::new()).unwrap();
    let original = fs::read(args.output_dir.join("public-inputs.json")).unwrap();
    fs::write(
        args.canary_public_key.as_ref().unwrap(),
        line(key(103, Algorithm::Ed25519).public_key()),
    )
    .unwrap();
    let error = prepare(&args, &mut Vec::new()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "output already contains a different public input bundle"
    );
    assert_eq!(
        fs::read(args.output_dir.join("public-inputs.json")).unwrap(),
        original
    );
    fs::write(args.output_dir.join("genesis.hash"), b"wrong\n").unwrap();
    let error = load(&args.output_dir).unwrap_err();
    assert_eq!(
        error.to_string(),
        "public input bundle artifact differs from its identity"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlink_input_and_partial_or_surplus_output() {
    let temp = private_custody_test_dir("taira-public-inputs-invalid-");
    let args = Fixture::seated().write(temp.path());
    let key = fs::read(args.canary_public_key.as_ref().unwrap()).unwrap();
    fs::remove_file(args.canary_public_key.as_ref().unwrap()).unwrap();
    std::os::unix::fs::symlink(
        args.localnet_dir.join("genesis.public_key"),
        args.canary_public_key.as_ref().unwrap(),
    )
    .unwrap();
    let error = prepare(&args, &mut Vec::new()).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "public reset input must be a direct regular file: `{}`",
            args.canary_public_key.as_ref().unwrap().display()
        )
    );
    assert!(!args.output_dir.exists());
    fs::remove_file(args.canary_public_key.as_ref().unwrap()).unwrap();
    fs::write(args.canary_public_key.as_ref().unwrap(), key).unwrap();
    prepare(&args, &mut Vec::new()).unwrap();
    fs::write(args.output_dir.join("unexpected"), b"extra").unwrap();
    let error = load(&args.output_dir).unwrap_err();
    assert_eq!(
        error.to_string(),
        "public input bundle has missing or unexpected files"
    );
    fs::remove_file(args.output_dir.join("unexpected")).unwrap();
    fs::remove_file(args.output_dir.join("genesis.hash")).unwrap();
    let error = load(&args.output_dir).unwrap_err();
    assert_eq!(
        error.to_string(),
        "public input bundle has missing or unexpected files"
    );
}

#[test]
fn cli_public_input_preparation_never_accepts_private_credentials() {
    use clap::Parser as _;
    let args = [
        "iroha",
        "taira",
        "public-reset",
        "prepare-public-inputs",
        "--localnet-dir",
        "/localnet",
        "--canary-public-key",
        "/canary.pub",
        "--output-dir",
        "/output",
    ];
    assert!(crate::Args::try_parse_from(args).is_ok());
    let draft_args = [
        "iroha",
        "taira",
        "public-reset",
        "prepare-public-inputs",
        "--localnet-dir",
        "/localnet",
        "--intent",
        "/unsigned.json",
        "--output-dir",
        "/output",
    ];
    assert!(crate::Args::try_parse_from(draft_args).is_ok());
    let mut both = draft_args.to_vec();
    both.extend(["--canary-public-key", "/canary.pub"]);
    assert!(crate::Args::try_parse_from(both).is_err());
    assert!(
        crate::Args::try_parse_from([
            "iroha",
            "taira",
            "public-reset",
            "prepare-beacon-inputs",
            "--intent",
            "/unsigned.json",
            "--public-inputs",
            "/public-inputs",
            "--output",
            "/beacon-inputs.json"
        ])
        .is_ok()
    );
    let mut private = args.to_vec();
    private.extend(["--private-key", "forbidden"]);
    assert!(crate::Args::try_parse_from(private).is_err());
}

#[test]
fn beacon_bootstrap_window_reserves_real_current_canary_and_install() {
    // Each case executes and signs its own real genesis; changing only a raw
    // manifest would fail authentication before reaching the production guard.
    for epoch in [6, 7, 12] {
        let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let fixture = Fixture::build_with_epoch(epoch);
        let mut inventory = sample_inventory_fixture();
        let roster =
            iroha_core::sumeragi::startup::genesis_committee_peers(&fixture.block).unwrap();
        for (client, peer) in inventory.validator_clients.iter_mut().zip(roster) {
            client.peer_id = peer.to_string();
        }
        let result = host::beacon::prepare_public_beacon_inputs(
            &fixture.block.encode_wire().unwrap(),
            &json_line(&fixture.manifest).unwrap(),
            fixture.genesis.public_key(),
            fixture.block.hash(),
            &inventory.authorization_nonce,
            &inventory.validators,
            &inventory.validator_clients,
        );
        if epoch == 6 {
            let error = result.expect_err("pulse 5 leaves no preceding installation carrier");
            assert!(format!("{error:#}").contains("first mandatory beacon pulse after height 5"));
        } else {
            result.expect("the native signed epoch permits installation before the pulse");
        }
    }
}

#[test]
fn beacon_public_preparation_derives_native_network_bound_seats_and_rejects_substitution() {
    let fixture = Fixture::new();
    let wire = fixture.block.encode_wire().unwrap();
    let manifest = json_line(&fixture.manifest).unwrap();
    let mut inventory = sample_inventory_fixture();
    let mut ordered =
        iroha_core::sumeragi::startup::genesis_committee_peers(&fixture.block).unwrap();
    ordered.reverse(); // Role order must not be mistaken for native signing order.
    for (client, peer) in inventory.validator_clients.iter_mut().zip(&ordered) {
        client.peer_id = peer.to_string();
    }
    let prepare = |nonce: &str, clients: &[ValidatorClientV1], raw: &[u8]| {
        host::beacon::prepare_public_beacon_inputs(
            &wire,
            raw,
            fixture.genesis.public_key(),
            fixture.block.hash(),
            nonce,
            &inventory.validators,
            clients,
        )
    };
    let first = prepare(
        &inventory.authorization_nonce,
        &inventory.validator_clients,
        &manifest,
    )
    .unwrap();
    let value = json::to_value(&first).unwrap();
    assert_eq!(
        json::to_value(
            &prepare(
                &inventory.authorization_nonce,
                &inventory.validator_clients,
                &manifest
            )
            .unwrap()
        )
        .unwrap(),
        value
    );
    let units = value.get("final_units").unwrap().as_array().unwrap();
    let network_id = iroha_data_model::NetworkId::from_genesis_hash(fixture.block.hash());
    let network_root = hex::encode(network_id.as_bytes());
    let attempt = hex::encode(<[u8; 32]>::from(iroha_crypto::Hash::new_from_chunks(&[
        b"iroha.global-beacon.genesis-attempt.v1\0",
        network_id.as_bytes(),
    ])));
    let roster = iroha_core::sumeragi::startup::genesis_committee_peers(&fixture.block).unwrap();
    for (index, unit) in units.iter().enumerate() {
        let seat = roster
            .iter()
            .position(|peer| peer == &ordered[index])
            .unwrap()
            + 1;
        assert_eq!(
            unit.get("signer_index").unwrap().as_u64(),
            Some(seat as u64)
        );
        assert_eq!(
            unit.get("validator").unwrap().as_str(),
            Some(inventory.validators[index].slug.as_str())
        );
        assert_eq!(
            unit.get("config_file").unwrap().as_str(),
            Some("beacon.toml")
        );
        let expected = format!(
            "/var/lib/taira/.public-reset-control-v1/beacon/{network_root}/ceremony/attempt-{attempt}-seat-{seat}/iroha-global-beacon-partial-signer-v1.norito"
        );
        assert_eq!(
            unit.get("credential_path").unwrap().as_str(),
            Some(expected.as_str())
        );
    }
    let other_nonce = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert_eq!(other_nonce.len(), 32);
    validate_nonce(other_nonce).expect("valid independent fixture nonce");
    assert_ne!(other_nonce, inventory.authorization_nonce.as_str());
    let other =
        json::to_value(&prepare(other_nonce, &inventory.validator_clients, &manifest).unwrap())
            .unwrap();
    assert_eq!(value.get("request"), other.get("request"));
    assert_eq!(value.get("final_units"), other.get("final_units"));
    assert_ne!(
        value.get("authorization_nonce"),
        other.get("authorization_nonce")
    );
    let mut wrong = inventory.validator_clients.clone();
    wrong[0].peer_id = wrong[1].peer_id.clone();
    assert!(prepare(&inventory.authorization_nonce, &wrong, &manifest).is_err());
    let invalid = fixture
        .manifest
        .clone()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned);
    assert!(
        invalid
            .clone()
            .with_consensus_meta()
            .unwrap_err()
            .to_string()
            .contains("permissioned genesis must omit `sumeragi_npos_parameters`")
    );
    assert!(
        prepare(
            &inventory.authorization_nonce,
            &inventory.validator_clients,
            &json_line(&invalid).unwrap()
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn public_bundle_requires_authenticated_raw_manifest_without_four_file_fallback() {
    let directory = private_custody_test_dir("public-manifest-");
    let fixture = Fixture::seated();
    let args = fixture.write(directory.path());
    prepare(&args, &mut Vec::new()).unwrap();
    let raw = fs::read(args.output_dir.join("genesis.json")).unwrap();
    assert_eq!(
        load(&args.output_dir).unwrap().raw_manifest_sha256,
        sha256_hex(&raw)
    );
    fs::remove_file(args.output_dir.join("genesis.json")).unwrap();
    assert!(load(&args.output_dir).is_err());
    fs::write(args.output_dir.join("genesis.json"), &raw).unwrap();
    fs::set_permissions(
        args.output_dir.join("genesis.json"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(load(&args.output_dir).is_ok());
    let mut changed = raw.clone();
    changed.push(b' ');
    fs::write(args.output_dir.join("genesis.json"), changed).unwrap();
    assert!(
        load(&args.output_dir).is_err(),
        "semantically identical but unbound manifest bytes are rejected"
    );
    fs::write(args.output_dir.join("genesis.json"), raw).unwrap();
    let invalid = fixture
        .manifest
        .clone()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned);
    assert!(
        invalid
            .clone()
            .with_consensus_meta()
            .unwrap_err()
            .to_string()
            .contains("permissioned genesis must omit `sumeragi_npos_parameters`")
    );
    fs::write(
        args.localnet_dir.join("genesis.json"),
        json_line(&invalid).unwrap(),
    )
    .unwrap();
    assert!(
        prepare(&args, &mut Vec::new()).is_err(),
        "another manifest cannot replace an authenticated bundle"
    );
}

#[cfg(unix)]
#[test]
fn public_bundle_derives_canary_from_topology_intent_without_key_file() {
    let _chain_guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let directory = private_custody_test_dir("public-draft-canary-");
    let fixture = Fixture::seated();
    let mut args = fixture.write(directory.path());
    let mut inventory = sample_inventory_fixture();
    inventory.canary_onboarding_request = fixture.derive().unwrap().canary_onboarding_request;
    let mut value = json::to_value(&inputs::ResetTopologyIntentV1::from(&inventory)).unwrap();
    let draft = directory.path().join("intent.json");
    inputs::write_new_private(&draft, &json_line(&value).unwrap()).unwrap();
    let key_path = args.canary_public_key.take().unwrap();
    fs::remove_file(key_path).unwrap();
    args.intent = Some(draft.clone());
    prepare(&args, &mut Vec::new()).unwrap();
    assert_eq!(
        load(&args.output_dir).unwrap().canary_public_key,
        *fixture.canary.public_key()
    );
    value.as_object_mut().unwrap().insert(
        "beacon_bootstrap".into(),
        json::to_value(&inventory.beacon_bootstrap).unwrap(),
    );
    fs::write(draft, json_line(&value).unwrap()).unwrap();
    assert!(prepare(&args, &mut Vec::new()).is_err());
}

#[cfg(unix)]
#[test]
fn public_bundle_refuses_a_network_that_cannot_seat_the_parliament() {
    // A genesis without citizens cannot seat any body.
    let directory = private_custody_test_dir("public-unseated-genesis-");
    let args = Fixture::build_with_epoch(20).write(directory.path());
    let error = prepare(&args, &mut Vec::new()).unwrap_err();
    assert!(
        format!("{error:#}").contains("citizens_cover_bodies"),
        "{error:#}"
    );
    assert!(!args.output_dir.exists());

    // Seated citizens under a validator profile that keeps the default bodies and faucet.
    let directory = private_custody_test_dir("public-unseated-profile-");
    let args = Fixture::seated().write(directory.path());
    fs::write(
        args.localnet_dir.join("peer0.toml"),
        "[torii.faucet]\nenabled = true\namount = \"25000\"\n",
    )
    .unwrap();
    let error = format!("{:#}", prepare(&args, &mut Vec::new()).unwrap_err());
    for requirement in [
        "citizens_cover_bodies",
        "coordination_council_explicit",
        "adaptive_faucet_difficulty",
        "citizenship_escrow_is_custodial",
    ] {
        assert!(error.contains(requirement), "{requirement}: {error}");
    }
    assert!(!args.output_dir.exists());

    // A seated profile whose escrow is the published default governance account is refused:
    // anyone holding that key could drain every bond.
    let mut published: toml::Table =
        toml::from_str(&crate::taira::parliament_seating::canonical_seating_config_toml()).unwrap();
    crate::taira::parliament_seating::set_citizenship_escrow(
        &mut published,
        &iroha_config::parameters::defaults::governance::citizenship_escrow_account_id(),
    )
    .unwrap();
    fs::write(
        args.localnet_dir.join("peer0.toml"),
        toml::to_string(&published).unwrap(),
    )
    .unwrap();
    let error = format!("{:#}", prepare(&args, &mut Vec::new()).unwrap_err());
    assert!(error.contains("citizenship_escrow_is_custodial"), "{error}");
    assert!(!error.contains("citizens_cover_bodies"), "{error}");
    assert!(!args.output_dir.exists());

    // Validators that disagree on the consensus seating profile are refused.
    let mut disagreeing = crate::taira::parliament_seating::canonical_seating_config_toml();
    disagreeing = disagreeing.replace("policy_jury_size = 9", "policy_jury_size = 7");
    fs::write(
        args.localnet_dir.join("peer0.toml"),
        crate::taira::parliament_seating::canonical_seating_config_toml(),
    )
    .unwrap();
    fs::write(args.localnet_dir.join("peer1.toml"), disagreeing).unwrap();
    let error = format!("{:#}", prepare(&args, &mut Vec::new()).unwrap_err());
    assert!(error.contains("does not share"), "{error}");
    fs::remove_file(args.localnet_dir.join("peer1.toml")).unwrap();
    prepare(&args, &mut Vec::new()).unwrap();
}
