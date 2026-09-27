//! Golden parity of the compiled `sora-nexus-v1` profile with today's Kagami Taira output.
//!
//! `specs/network_deployment.md` §13 P1 "Golden parity": with fixed seeds, the `sora-nexus-v1`
//! render plus genesis must reproduce the `execution_policy_hash` and `nexus_amx_context_hash`
//! that Kagami's canonical Taira localnet signs into genesis. Only the intentional differences
//! are normalized, each exactly once:
//!
//! - **Catalog** (listed in the spec). The profile carries the core system lanes plus the public
//!   `nexus` lane; Kagami bakes the Taira customer catalog into genesis. The profile render is
//!   given Kagami's catalog ([`CATALOG_FIELDS`]) and both are staged against Kagami's own signed
//!   genesis, so no other Nexus value is copied.
//! - **Cadence** (listed). Kagami is generated with the profile's `block_cadence_ms`.
//! - **Authenticated capacity 4** (listed). `sumeragi.queues.authenticated_non_validator_sources`
//!   and the ingress geometry derived from it are bound by neither hash; nothing is normalized.
//! - **Protocol custody account** (not listed in the spec; found by this test). Kagami derives a
//!   keyless custody account from each genesis public key; a compiled profile cannot know that
//!   key, so it fixes one keyless account ([`iroha_config::profile::protocol_custody_account`]).
//!   The test first proves both sides are exactly those keyless derivations in all five
//!   [`CUSTODY_ACCOUNT_FIELDS`], then copies Kagami's account.
//! - **Governance accounts** (not listed in the spec; a security fix). Kagami's Taira names the
//!   published sample key (`iroha_test_samples` ALICE) for every governance escrow, receiver,
//!   pool and treasury; the profile gives each role its own keyless account
//!   ([`iroha_config::profile::keyless_role_account`]). The test proves Kagami's side is the
//!   code-default sample account and the profile's side is the keyless derivation in all six
//!   [`GOVERNANCE_ACCOUNT_FIELDS`], then copies Kagami's accounts.
//!
//! Every other input of the two hashes comes from the profile unchanged. On a mismatch the test
//! prints the differing hash-input sections line by line.
//!
//! TODO(P8): `kagami localnet` is deleted at the cutover. The P2 genesis builder then provides the
//! genesis half, and this test moves to `iroha_deploy` with Kagami's two hashes pinned.

use super::*;
use iroha_config::{
    node_config::{NodeConfigOptions, NodeFile, open_node_config},
    parameters::{actual::NodeSecretFile, user},
    profile::{KeylessRole, Profile, ProfileId, ProfileRole, keyless_role_account},
};
use iroha_data_model::block::SignedBlock;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

/// Fixed peer, operator and genesis seed shared by both halves.
const SEED: &str = "sora-nexus-v1-golden-parity";
/// Peer whose rendered configuration is compared.
const PEER: usize = 0;
/// Nexus fields that make up "the catalog": lanes, dataspaces, routes, the lane-manifest registry
/// and the governance modules its restricted lanes name. They are copied from Kagami into the
/// profile render by [`copy_kagami_catalog`]; this list documents that exact set.
const CATALOG_FIELDS: &[&str] = &[
    "nexus.lane_catalog",
    "nexus.configured_lane_catalog",
    "nexus.lane_config",
    "nexus.dataspace_catalog",
    "nexus.configured_dataspace_catalog",
    "nexus.routing_policy",
    "nexus.registry",
    "nexus.governance",
];

/// Custody roles that name one keyless protocol account on both sides. They are copied from Kagami
/// into the profile render by [`copy_kagami_custody_account`] after both are checked.
const CUSTODY_ACCOUNT_FIELDS: &[&str] = &[
    "pipeline.gas.tech_account_id",
    "nexus.fees.fee_sink_account_id",
    "nexus.fees.sponsor_vault_custody_account_id",
    "nexus.staking.stake_escrow_account_id",
    "nexus.staking.slash_sink_account_id",
];

/// Governance roles that name the published sample key in Kagami's Taira and a keyless role
/// account in the profile, in [`KeylessRole::ALL`] order. They are copied from Kagami into the
/// profile render by [`copy_kagami_governance_accounts`] after both are checked.
const GOVERNANCE_ACCOUNT_FIELDS: &[&str] = &[
    "gov.citizenship_escrow_account",
    "gov.bond_escrow_account",
    "gov.slash_receiver_account",
    "gov.viral_incentives.incentive_pool_account",
    "gov.viral_incentives.escrow_account",
    "gov.sorafs_pin_fee_treasury_account",
];

/// Today's canonical Taira localnet, generated with the profile's cadence.
struct KagamiTaira {
    dir: tempfile::TempDir,
    manifest: RawGenesisTransaction,
    signed: SignedBlock,
}

impl KagamiTaira {
    fn generate(block_cadence_ms: u64) -> Self {
        let dir = tempfile::tempdir().expect("temporary Taira directory");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(TAIRA_TESTNET_PEERS).expect("four peers"),
            seed: Some(SEED.to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 8_080,
            base_p2p_port: 1_337,
            out_dir: dir.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: Some(block_cadence_ms),
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet_inner(
            &opts,
            &mut BufWriter::new(Vec::new()),
            Some(PUBLIC_TAIRA_CHAIN_ID),
        )
        .expect("generate canonical Taira localnet");
        let _discriminant = ChainDiscriminantGuard::enter(taira_discriminant());
        let manifest = RawGenesisTransaction::from_path(dir.path().join("genesis.json"))
            .expect("bound Taira genesis manifest");
        let signed = read_signed_genesis(&dir.path().join("genesis.signed.nrt"))
            .expect("signed Taira genesis");
        Self {
            dir,
            manifest,
            signed,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn peer_table(&self, peer: usize) -> toml::Table {
        toml::from_str(
            &fs::read_to_string(self.path(&format!("peer{peer}.toml"))).expect("Taira peer"),
        )
        .expect("Taira peer TOML")
    }

    /// The signed `(nexus_amx_context_hash, execution_policy_hash)`.
    fn signed_hashes(&self) -> (Hash, Hash) {
        let context = self.manifest.sumeragi_v2_context_parameters();
        (
            Hash::prehashed(context.nexus_amx_context_hash),
            Hash::prehashed(context.execution_policy_hash),
        )
    }

    /// Restage the signed genesis under `config`, as prepared-bundle admission does.
    fn staged_hashes(&self, config: &actual::Root, side: &str) -> (Hash, Hash) {
        crate::genesis::staged_signed_sumeragi_v2_context_hashes(
            &self.manifest,
            &self.signed,
            config,
        )
        .unwrap_or_else(|error| {
            panic!("restage the signed Taira genesis under the {side}: {error:?}")
        })
    }
}

fn taira_discriminant() -> u16 {
    known_chain_discriminant_for_chain_id(PUBLIC_TAIRA_CHAIN_ID).expect("Taira discriminant")
}

/// Read a node file exactly as `iroha3d` does, including `--sora` for Kagami's flat files.
fn read_node_config(path: &Path, sora: bool) -> actual::Root {
    let _discriminant = ChainDiscriminantGuard::enter(taira_discriminant());
    let (reader, _binding) = open_node_config(
        NodeFile::Path(path.to_path_buf()),
        NodeConfigOptions { sora },
    )
    .expect("open node file")
    .into_parts();
    let mut config = reader
        .read_and_complete::<user::Root>()
        .expect("read node file")
        .parse()
        .expect("parse node file");
    if sora {
        config.apply_sora_profile();
    }
    config
}

fn owner_only_dir(path: &Path) {
    fs::create_dir_all(path).expect("create directory");
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("owner-only directory");
}

/// Write the `sora-nexus-v1` validator node file of Kagami peer `peer` under `root`.
///
/// It carries only per-node values the profile allowlist admits, taken from Kagami's peer
/// config; the node's keys go to the fixed `data_dir` secret files.
fn render_profile_node(kagami: &KagamiTaira, peer: usize, root: &Path) -> PathBuf {
    let source = kagami.peer_table(peer);
    let data_dir = actual::DataDir::new(root.join("data"));
    owner_only_dir(&data_dir.secrets_dir());
    let text = |table: &toml::Table, key: &str| -> String {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("Kagami peer omitted `{key}`"))
            .to_owned()
    };
    let streaming = source["streaming"].as_table().expect("streaming table");
    for (file, key) in [
        (NodeSecretFile::Validator, text(&source, "private_key")),
        (
            NodeSecretFile::Transport,
            text(&source, "soranet_transport_private_key"),
        ),
        (
            NodeSecretFile::Streaming,
            text(streaming, "identity_private_key"),
        ),
    ] {
        let path = data_dir.secret(file);
        fs::write(&path, format!("{key}\n")).expect("write node secret");
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("owner-only secret");
    }
    let mut node = toml::Table::new();
    for (key, value) in [
        ("profile", ProfileId::SoraNexusV1.as_str()),
        ("role_overlay", ProfileRole::Validator.as_str()),
    ] {
        node.insert(key.into(), toml::Value::String(value.to_owned()));
    }
    node.insert("profile_roster_size".into(), toml::Value::Integer(4));
    node.insert(
        "data_dir".into(),
        toml::Value::String(data_dir.root().to_string_lossy().into_owned()),
    );
    for key in [
        "chain",
        "chain_discriminant",
        "public_key",
        "trusted_peers",
        "trusted_peers_pop",
    ] {
        node.insert(key.into(), source[key].clone());
    }
    let pick = |section: &str, keys: &[&str]| {
        let from = source[section].as_table().expect("section table");
        toml::Value::Table(
            keys.iter()
                .map(|key| ((*key).to_owned(), from[*key].clone()))
                .collect(),
        )
    };
    node.insert(
        "network".into(),
        pick("network", &["address", "public_address"]),
    );
    node.insert("torii".into(), pick("torii", &["address"]));
    let expected_hash =
        fs::read_to_string(kagami.path("genesis.expected_hash")).expect("Kagami genesis identity");
    let mut genesis = source["genesis"].as_table().expect("genesis").clone();
    genesis.remove("expected_hash_file");
    genesis.insert(
        "expected_hash".into(),
        toml::Value::String(expected_hash.trim().to_owned()),
    );
    node.insert("genesis".into(), toml::Value::Table(genesis));
    let signer = source["soracloud_runtime"]["submission"]["signer"].clone();
    let submission = toml::Table::from_iter([("signer".to_owned(), signer)]);
    node.insert(
        "soracloud_runtime".into(),
        toml::Value::Table(toml::Table::from_iter([(
            "submission".to_owned(),
            toml::Value::Table(submission),
        )])),
    );
    let path = root.join("node.toml");
    fs::write(&path, toml::to_string(&node).expect("encode node file")).expect("write node file");
    path
}

/// Replace exactly the [`CATALOG_FIELDS`] of the profile render with Kagami's.
fn copy_kagami_catalog(to: &mut actual::Nexus, from: &actual::Nexus) {
    to.lane_catalog = from.lane_catalog.clone();
    to.configured_lane_catalog = from.configured_lane_catalog.clone();
    to.lane_config = from.lane_config.clone();
    to.dataspace_catalog = from.dataspace_catalog.clone();
    to.configured_dataspace_catalog = from.configured_dataspace_catalog.clone();
    to.routing_policy = from.routing_policy.clone();
    to.registry = from.registry.clone();
    to.governance = from.governance.clone();
}

/// The account of every [`CUSTODY_ACCOUNT_FIELDS`] role, in that order.
fn custody_accounts(pipeline: &actual::Pipeline, nexus: &actual::Nexus) -> [AccountId; 5] {
    let _discriminant = ChainDiscriminantGuard::enter(taira_discriminant());
    let parse = |literal: &str| AccountId::parse_encoded(literal).expect("custody account literal");
    [
        parse(&pipeline.gas.tech_account_id),
        parse(&nexus.fees.fee_sink_account_id),
        nexus.fees.sponsor_vault_custody_account_id.clone(),
        parse(&nexus.staking.stake_escrow_account_id),
        parse(&nexus.staking.slash_sink_account_id),
    ]
}

/// Copy Kagami's [`CUSTODY_ACCOUNT_FIELDS`] values into the render, verbatim.
fn copy_kagami_custody_account(
    (pipeline, nexus): (&mut actual::Pipeline, &mut actual::Nexus),
    (from_pipeline, from_nexus): (&actual::Pipeline, &actual::Nexus),
) {
    pipeline.gas.tech_account_id = from_pipeline.gas.tech_account_id.clone();
    nexus.fees.fee_sink_account_id = from_nexus.fees.fee_sink_account_id.clone();
    nexus.fees.sponsor_vault_custody_account_id =
        from_nexus.fees.sponsor_vault_custody_account_id.clone();
    nexus.staking.stake_escrow_account_id = from_nexus.staking.stake_escrow_account_id.clone();
    nexus.staking.slash_sink_account_id = from_nexus.staking.slash_sink_account_id.clone();
}

/// The account of every [`GOVERNANCE_ACCOUNT_FIELDS`] role, in that order.
fn governance_accounts(gov: &actual::Governance) -> [AccountId; 6] {
    [
        gov.citizenship_escrow_account.clone(),
        gov.bond_escrow_account.clone(),
        gov.slash_receiver_account.clone(),
        gov.viral_incentives.incentive_pool_account.clone(),
        gov.viral_incentives.escrow_account.clone(),
        gov.sorafs_pin_fee_treasury_account.clone(),
    ]
}

/// Copy Kagami's [`GOVERNANCE_ACCOUNT_FIELDS`] values into the render, verbatim.
fn copy_kagami_governance_accounts(to: &mut actual::Governance, from: &actual::Governance) {
    to.citizenship_escrow_account = from.citizenship_escrow_account.clone();
    to.bond_escrow_account = from.bond_escrow_account.clone();
    to.slash_receiver_account = from.slash_receiver_account.clone();
    to.viral_incentives.incentive_pool_account =
        from.viral_incentives.incentive_pool_account.clone();
    to.viral_incentives.escrow_account = from.viral_incentives.escrow_account.clone();
    to.sorafs_pin_fee_treasury_account = from.sorafs_pin_fee_treasury_account.clone();
}

/// Line diff (longest common subsequence) of two pretty `Debug` renderings.
fn line_diff(left: &str, right: &str) -> Vec<String> {
    let left: Vec<&str> = left.lines().collect();
    let right: Vec<&str> = right.lines().collect();
    let mut lcs = vec![vec![0_u32; right.len() + 1]; left.len() + 1];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            lcs[i][j] = if left[i] == right[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < left.len() || j < right.len() {
        if i < left.len() && j < right.len() && left[i] == right[j] {
            i += 1;
            j += 1;
        } else if j < right.len() && (i == left.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push(format!("  + profile: {}", right[j].trim()));
            j += 1;
        } else {
            out.push(format!("  - kagami:  {}", left[i].trim()));
            i += 1;
        }
    }
    out
}

/// Every configuration section either hash reads, rendered for a diff.
fn hash_input_sections(config: &actual::Root) -> Vec<(&'static str, String)> {
    let nexus = &config.nexus;
    vec![
        ("pipeline", format!("{:#?}", config.pipeline)),
        ("oracle", format!("{:#?}", config.oracle)),
        ("crypto", format!("{:#?}", config.crypto)),
        (
            "fraud_monitoring",
            format!("{:#?}", config.fraud_monitoring),
        ),
        ("gov", format!("{:#?}", config.gov)),
        ("content", format!("{:#?}", config.content)),
        ("settlement", format!("{:#?}", config.settlement)),
        ("zk", format!("{:#?}", config.zk)),
        ("nexus.staking", format!("{:#?}", nexus.staking)),
        ("nexus.fees", format!("{:#?}", nexus.fees)),
        (
            "nexus.hf_shared_leases",
            format!("{:#?}", nexus.hf_shared_leases),
        ),
        (
            "nexus.uploaded_models",
            format!("{:#?}", nexus.uploaded_models),
        ),
        ("nexus.endorsement", format!("{:#?}", nexus.endorsement)),
        ("nexus.axt", format!("{:#?}", nexus.axt)),
        (
            "nexus.atomic_private_settlement",
            format!("{:#?}", nexus.atomic_private_settlement),
        ),
        (
            "nexus.dataspace_fee_sponsor_program_ids",
            format!("{:#?}", nexus.dataspace_fee_sponsor_program_ids),
        ),
        ("nexus.governance", format!("{:#?}", nexus.governance)),
        ("nexus.compliance", format!("{:#?}", nexus.compliance)),
        ("nexus.fusion", format!("{:#?}", nexus.fusion)),
        ("nexus.autoscale", format!("{:#?}", nexus.autoscale)),
        ("nexus.commit", format!("{:#?}", nexus.commit)),
        ("nexus.da", format!("{:#?}", nexus.da)),
        (
            "nexus.lane_relay_emergency",
            format!("{:#?}", nexus.lane_relay_emergency),
        ),
        (
            "nexus.catalog",
            format!(
                "{:#?}\n{:#?}\n{:#?}\n{:#?}",
                nexus.configured_lane_catalog,
                nexus.configured_dataspace_catalog,
                nexus.routing_policy,
                nexus.registry
            ),
        ),
    ]
}

fn section_differences(kagami: &actual::Root, profile: &actual::Root) -> String {
    use std::fmt::Write as _;
    let mut report = String::new();
    for ((name, left), (_, right)) in hash_input_sections(kagami)
        .into_iter()
        .zip(hash_input_sections(profile))
    {
        if left != right {
            writeln!(report, "[{name}]").expect("writing to a String cannot fail");
            for line in line_diff(&left, &right) {
                report.push_str(&line);
                report.push('\n');
            }
        }
    }
    report
}

#[test]
fn line_diff_reports_only_changed_lines() {
    assert!(line_diff("a\nb\nc", "a\nb\nc").is_empty());
    assert_eq!(
        line_diff("a\nb\nc", "a\nx\nc"),
        vec!["  + profile: x".to_owned(), "  - kagami:  b".to_owned()]
    );
    assert_eq!(line_diff("a", "a\nb"), vec!["  + profile: b".to_owned()]);
    assert_eq!(line_diff("a\nb", "b"), vec!["  - kagami:  a".to_owned()]);
}

#[test]
fn catalog_normalization_copies_exactly_the_catalog_fields() {
    let mut kagami = actual::Nexus::default();
    kagami.routing_policy.default_lane = LaneId::new(3);
    kagami.staking.max_slash_bps = kagami.staking.max_slash_bps.wrapping_add(1);
    let mut profile = actual::Nexus::default();
    copy_kagami_catalog(&mut profile, &kagami);
    assert_eq!(profile.routing_policy, kagami.routing_policy);
    assert_eq!(
        profile.staking.max_slash_bps,
        actual::Nexus::default().staking.max_slash_bps,
        "a non-catalog field must not be copied"
    );
    assert_eq!(CATALOG_FIELDS.len(), 8);
    assert!(
        CATALOG_FIELDS
            .iter()
            .all(|field| field.starts_with("nexus."))
    );
}

#[test]
fn custody_normalization_sets_exactly_the_five_custody_roles() {
    let kagami = localnet_gas_account_id(KeyPair::random().public_key());
    let literal = kagami
        .to_i105_for_discriminant(taira_discriminant())
        .expect("I105 literal");
    let (mut from_pipeline, mut from_nexus) =
        (actual::Pipeline::default(), actual::Nexus::default());
    from_pipeline.gas.tech_account_id.clone_from(&literal);
    from_nexus.fees.fee_sink_account_id.clone_from(&literal);
    from_nexus.fees.sponsor_vault_custody_account_id = kagami.clone();
    from_nexus
        .staking
        .stake_escrow_account_id
        .clone_from(&literal);
    from_nexus
        .staking
        .slash_sink_account_id
        .clone_from(&literal);
    from_nexus.fees.fee_asset_id = "not-a-custody-field".to_owned();
    let (mut pipeline, mut nexus) = (actual::Pipeline::default(), actual::Nexus::default());
    let fee_asset = nexus.fees.fee_asset_id.clone();
    copy_kagami_custody_account((&mut pipeline, &mut nexus), (&from_pipeline, &from_nexus));
    let accounts = custody_accounts(&pipeline, &nexus);
    assert_eq!(accounts.len(), CUSTODY_ACCOUNT_FIELDS.len());
    assert!(accounts.iter().all(|account| *account == kagami));
    assert_eq!(
        nexus.fees.fee_asset_id, fee_asset,
        "a non-custody field must not change"
    );
}

#[test]
fn governance_normalization_sets_exactly_the_six_governance_roles() {
    let kagami = localnet_gas_account_id(KeyPair::random().public_key());
    let defaults = actual::Governance::default();
    let from = actual::Governance {
        citizenship_escrow_account: kagami.clone(),
        bond_escrow_account: kagami.clone(),
        slash_receiver_account: kagami.clone(),
        sorafs_pin_fee_treasury_account: kagami.clone(),
        viral_incentives: actual::ViralIncentives {
            incentive_pool_account: kagami.clone(),
            escrow_account: kagami.clone(),
            halt: !defaults.viral_incentives.halt,
            ..defaults.viral_incentives.clone()
        },
        ..defaults
    };
    let mut to = actual::Governance::default();
    let halt = to.viral_incentives.halt;
    copy_kagami_governance_accounts(&mut to, &from);
    let accounts = governance_accounts(&to);
    assert_eq!(accounts.len(), GOVERNANCE_ACCOUNT_FIELDS.len());
    assert_eq!(
        KeylessRole::ALL
            .iter()
            .filter(|role| role.is_static())
            .count(),
        GOVERNANCE_ACCOUNT_FIELDS.len()
    );
    assert!(accounts.iter().all(|account| *account == kagami));
    assert_eq!(
        to.viral_incentives.halt, halt,
        "a non-account field must not change"
    );
}

/// The profile render plus Kagami's Taira genesis reproduces both signed hashes.
#[test]
fn sora_nexus_v1_render_reproduces_kagami_taira_policy_and_amx_hashes() {
    let profile = Profile::compiled(ProfileId::SoraNexusV1).expect("compiled profile");
    let kagami = KagamiTaira::generate(profile.genesis_recipe().block_cadence_ms);
    let signed = kagami.signed_hashes();
    let kagami_config = read_node_config(&kagami.path(&format!("peer{PEER}.toml")), true);
    assert_eq!(
        kagami.staged_hashes(&kagami_config, "Kagami peer config"),
        signed,
        "Kagami's own peer config must reproduce its signed genesis hashes"
    );
    let root = tempfile::tempdir().expect("profile node directory");
    let root = fs::canonicalize(root.path())
        .map(|path| (root, path))
        .expect("canonical root");
    let node_file = render_profile_node(&kagami, PEER, &root.1);
    let mut rendered = read_node_config(&node_file, false);
    copy_kagami_catalog(&mut rendered.nexus, &kagami_config.nexus);
    let genesis_public_key: iroha_crypto::PublicKey =
        fs::read_to_string(kagami.path("genesis.public_key"))
            .expect("Kagami genesis public key")
            .trim()
            .parse()
            .expect("genesis public key");
    let kagami_custody = localnet_gas_account_id(&genesis_public_key);
    let profile_custody = iroha_config::profile::protocol_custody_account(ProfileId::SoraNexusV1);
    for (field, (kagami_account, profile_account)) in CUSTODY_ACCOUNT_FIELDS.iter().zip(
        custody_accounts(&kagami_config.pipeline, &kagami_config.nexus)
            .into_iter()
            .zip(custody_accounts(&rendered.pipeline, &rendered.nexus)),
    ) {
        assert_eq!(
            kagami_account, kagami_custody,
            "Kagami {field} is not keyless"
        );
        assert_eq!(
            profile_account, profile_custody,
            "profile {field} is not keyless"
        );
    }
    copy_kagami_custody_account(
        (&mut rendered.pipeline, &mut rendered.nexus),
        (&kagami_config.pipeline, &kagami_config.nexus),
    );
    let published_sample = iroha_config::parameters::defaults::governance::bond_escrow_account_id();
    let keyless_roles = KeylessRole::ALL.into_iter().filter(|role| role.is_static());
    for ((field, role), (kagami_account, profile_account)) in
        GOVERNANCE_ACCOUNT_FIELDS.iter().zip(keyless_roles).zip(
            governance_accounts(&kagami_config.gov)
                .into_iter()
                .zip(governance_accounts(&rendered.gov)),
        )
    {
        assert_eq!(
            kagami_account, published_sample,
            "Kagami {field} is not the published sample account"
        );
        assert_eq!(
            profile_account,
            keyless_role_account(ProfileId::SoraNexusV1, role),
            "profile {field} is not its keyless role account"
        );
    }
    copy_kagami_governance_accounts(&mut rendered.gov, &kagami_config.gov);
    let staged = kagami.staged_hashes(&rendered, "sora-nexus-v1 render");
    assert!(
        staged == signed,
        "sora-nexus-v1 differs from Kagami Taira outside the intentional differences\n\
         nexus_amx_context_hash: kagami {} profile {}\n\
         execution_policy_hash: kagami {} profile {}\n\
         differing hash inputs:\n{}",
        signed.0,
        staged.0,
        signed.1,
        staged.1,
        section_differences(&kagami_config, &rendered)
    );
}

/// The profile's genesis recipe is Kagami's signed Taira genesis, except for the cadence.
#[test]
fn sora_nexus_v1_genesis_recipe_matches_kagami_taira_genesis() {
    let profile = Profile::compiled(ProfileId::SoraNexusV1).expect("compiled profile");
    let recipe = profile.genesis_recipe();
    let geometry = profile.derive(4).expect("derive(4)");
    let kagami = KagamiTaira::generate(recipe.block_cadence_ms);
    let _discriminant = ChainDiscriminantGuard::enter(taira_discriminant());
    let parameters = kagami
        .manifest
        .effective_parameters()
        .expect("Kagami genesis parameters");
    assert_eq!(
        kagami.manifest.consensus_mode(),
        SumeragiConsensusMode::Npos
    );
    assert_eq!(recipe.consensus_mode, "npos");
    assert_eq!(
        parameters.sumeragi().block_cadence_ms().get(),
        recipe.block_cadence_ms
    );
    let npos = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .expect("signed NPoS parameters");
    assert_eq!(
        npos.epoch_length_blocks().get(),
        recipe.epoch_length_blocks,
        "epoch_length_blocks"
    );
    assert_eq!(npos.max_validators(), geometry.npos_max_validators);
    assert_eq!(npos.seat_band_pct(), recipe.npos_seat_band_pct);
    assert_eq!(
        npos.min_self_bond(),
        &Quantity::from(recipe.npos_min_self_bond)
    );
    assert_eq!(
        parameters.block().max_transactions().get(),
        recipe.block_max_transactions
    );
    let ivm_gas_limit_per_block: u64 = parameters
        .custom()
        .get(&localnet_custom_parameter_id("ivm_gas_limit_per_block"))
        .expect("signed IVM gas limit")
        .payload()
        .try_into_any_norito()
        .expect("IVM gas limit payload");
    assert_eq!(ivm_gas_limit_per_block, recipe.ivm_gas_limit_per_block);
}
