//! Public retention admission, immutable input custody and worker restart identities.

use super::*;
use std::os::unix::fs::PermissionsExt as _;

fn admission_fixture() -> (
    PolicyV1,
    DeploymentTrustV1,
    iroha::config::Config,
    iroha_crypto::KeyPair,
) {
    let (block, administrator) =
        crate::taira_public_reset::deployment_genesis_administrator_fixture();
    let network = NetworkId::from_genesis_hash(block.hash());
    let mut trust = finality::test_trust();
    trust.genesis_public_key = administrator.public_key().clone();
    trust.genesis_signed_wire_hex = hex::encode(block.encode_wire().unwrap());
    let (mut config, _) = iroha::config::Config::load_bytes_with_musubi_publication(
        "/epoch-admission-fixture.toml",
        include_bytes!("../../../defaults/client.toml"),
    )
    .unwrap();
    config.chain = "fc56984b-2be7-431d-840e-21514d1883f0".into();
    config.network_id = network;
    config.account = AccountId::new(administrator.public_key().clone());
    config.account_chain_discriminant = 369;
    config.key_pair = administrator;
    config.basic_auth = None;
    config.torii_api_url = trust.peers[0].torii_origin.parse().unwrap();
    let policy = PolicyV1 {
        schema_version: 1,
        intent: IntentV1 {
            authorization: "until_stopped".into(),
            network_id: network,
            administrator: config.account.clone(),
            first_epoch: 1,
        },
        release_source_commit: "a".repeat(40),
        iroha_sha256: "b".repeat(64),
        observation_trust_sha256: digest(&json::to_vec(&trust).unwrap()),
    };
    let operator =
        iroha_crypto::KeyPair::try_from_seed(vec![37; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    (policy, trust, config, operator)
}

fn admit_fixture(
    policy: &PolicyV1,
    trust: &DeploymentTrustV1,
    config: &iroha::config::Config,
    operator: &iroha_crypto::KeyPair,
) -> Result<()> {
    super::super::supervisor_generation_admission(
        &json::to_vec(policy)?,
        &json::to_vec(trust)?,
        config,
        operator,
    )
}

#[test]
fn epoch_supervisor_generation_admission_accepts_exact_public_inputs_without_files() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, mut config, operator) = admission_fixture();
    for peer in &trust.peers {
        config.torii_api_url = peer.torii_origin.parse().unwrap();
        admit_fixture(&policy, &trust, &config, &operator).unwrap();
        assert_eq!(config.torii_api_url.as_str(), peer.torii_origin);
    }
}

#[test]
fn epoch_supervisor_generation_admission_rejects_foreign_origin_and_taira_profile() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, config, operator) = admission_fixture();
    for origin in [
        "http://127.0.0.1:8099/",
        "http://127.0.0.1:8080/unadmitted/",
        "http://127.0.0.1:8080/?redirect=1",
    ] {
        let mut wrong = config.clone();
        wrong.torii_api_url = origin.parse().unwrap();
        assert!(admit_fixture(&policy, &trust, &wrong, &operator).is_err());
        assert_eq!(wrong.torii_api_url.as_str(), origin);
    }
    let mut wrong = config.clone();
    wrong.chain = "foreign-chain".into();
    assert!(admit_fixture(&policy, &trust, &wrong, &operator).is_err());
    wrong = config;
    wrong.account_chain_discriminant = 753;
    assert!(admit_fixture(&policy, &trust, &wrong, &operator).is_err());
}

#[test]
fn epoch_supervisor_generation_admission_rejects_administrator_identity_drift() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, config, operator) = admission_fixture();
    let mut wrong = config.clone();
    wrong.account = AccountId::new(operator.public_key().clone());
    assert!(admit_fixture(&policy, &trust, &wrong, &operator).is_err());
    wrong = config.clone();
    wrong.key_pair = operator.clone();
    assert!(admit_fixture(&policy, &trust, &wrong, &config.key_pair).is_err());
}

#[test]
fn epoch_supervisor_generation_admission_rejects_shared_operator_key() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, config, _) = admission_fixture();
    assert!(admit_fixture(&policy, &trust, &config, &config.key_pair).is_err());
}

#[test]
fn epoch_supervisor_generation_admission_rejects_changed_trust_and_network() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, config, operator) = admission_fixture();
    let mut wrong = trust.clone();
    wrong.peers[0].config_fingerprint = iroha_crypto::Hash::new(b"unadmitted-current-config");
    assert!(admit_fixture(&policy, &wrong, &config, &operator).is_err());
    wrong = trust;
    wrong.genesis_public_key = operator.public_key().clone();
    let mut selected = policy.clone();
    selected.observation_trust_sha256 = digest(&json::to_vec(&wrong).unwrap());
    assert!(admit_fixture(&selected, &wrong, &config, &operator).is_err());
    selected = policy;
    selected.intent.network_id = finality::test_network_id();
    assert!(admit_fixture(&selected, &wrong, &config, &operator).is_err());
}

#[test]
fn epoch_supervisor_generation_admission_requires_bounded_closed_schemas() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (policy, trust, config, operator) = admission_fixture();
    let original = [
        json::to_vec(&policy).unwrap(),
        json::to_vec(&trust).unwrap(),
    ];
    for index in 0..2 {
        let mut values = original.clone();
        let mut value: json::Value = json::from_slice(&values[index]).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json::Value::Bool(true));
        values[index] = json::to_vec(&value).unwrap();
        assert!(generation_admission(&values[0], &values[1], &config, &operator).is_err());
        values[index] = vec![b' '; MAX_BYTES + 1];
        assert!(generation_admission(&values[0], &values[1], &config, &operator).is_err());
    }
}

#[test]
fn epoch_supervisor_status_parser_has_no_seed_or_mutation_inputs() {
    use clap::Parser as _;
    #[derive(clap::Parser)]
    struct Harness {
        #[command(subcommand)]
        command: super::super::Command,
    }
    let arguments = [
        "test",
        "supervisor-status",
        "--policy",
        "/policy",
        "--trust",
        "/trust",
        "--journal-dir",
        "/journals",
        "--boot-id",
        "01234567-89ab-cdef-0123-456789abcdef",
        "--pid",
        "42",
        "--start-time-ticks",
        "7",
    ];
    assert!(matches!(
        Harness::try_parse_from(arguments).unwrap().command,
        super::super::Command::SupervisorStatus(_)
    ));
    let mut extra = arguments.to_vec();
    extra.extend(["--custody", "/private"]);
    assert!(Harness::try_parse_from(extra).is_err());
    let mut zero = arguments.to_vec();
    zero.extend(["--timeout-ms", "0"]);
    assert!(Harness::try_parse_from(zero).is_err());
}

#[test]
fn epoch_supervisor_readiness_names_bind_policy_and_process_incarnation() {
    let identity = WorkerIdentityV1 {
        boot_id: "01234567-89ab-cdef-0123-456789abcdef".into(),
        pid: 42,
        start_time_ticks: 9,
    };
    let name = readiness_name(&"a".repeat(64), &identity);
    let mut changed = identity.clone();
    changed.start_time_ticks += 1;
    assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
    changed = identity.clone();
    changed.pid += 1;
    assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
    changed = identity.clone();
    changed.boot_id = "11234567-89ab-cdef-0123-456789abcdef".into();
    assert_ne!(name, readiness_name(&"a".repeat(64), &changed));
    assert_ne!(name, readiness_name(&"b".repeat(64), &identity));
}

#[test]
fn retention_input_custody_rejects_mutation_shared_links_and_writable_ancestors() {
    use std::os::unix::fs::symlink;
    let directory = crate::taira_public_reset::private_custody_test_dir("retention-input-");
    let path = directory.path().join("input.json");
    fs::write(&path, b"{\"schema_version\":1}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut input = PinnedFile::open(&path, true).unwrap();
    assert_eq!(input.public_bytes().unwrap(), b"{\"schema_version\":1}");
    for mode in [0o720, 0o702, 0o1777] {
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(mode)).unwrap();
        assert!(PinnedFile::open(&path, true).is_err());
        assert!(PinnedFile::open(&path, false).is_err());
    }
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&path, b"{\"schema_version\":2}").unwrap();
    assert!(
        input.public_bytes().is_err(),
        "held evidence must reject changed bytes"
    );
    let original = directory.path().join("original.json");
    fs::rename(&path, &original).unwrap();
    fs::write(&path, b"{\"schema_version\":1}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        input.revalidate().is_err(),
        "held evidence must reject a rebound pathname"
    );
    let alias = directory.path().join("alias.json");
    symlink(&path, &alias).unwrap();
    assert!(PinnedFile::open(&alias, true).is_err());
    fs::hard_link(&path, directory.path().join("shared.json")).unwrap();
    assert!(PinnedFile::open(&path, true).is_err());
    assert!(PinnedFile::open(&path, false).is_err());
    let oversized = directory.path().join("oversized.json");
    fs::write(&oversized, vec![b' '; MAX_BYTES + 1]).unwrap();
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        PinnedFile::open(&oversized, true)
            .unwrap()
            .public_bytes()
            .is_err()
    );
}
