//! Integration tests for the Nexus lane manifest registry.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use eyre::Result;
use iroha_config::parameters::actual::{GovernanceCatalog, GovernanceModule, LaneRegistry};
use iroha_core::governance::manifest::LaneManifestRegistry;
use iroha_data_model::nexus::{LaneCatalog, LaneConfig, LaneId, LaneStorageProfile};
use nonzero_ext::nonzero;

fn fixtures_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root exists")
        .join(relative)
}

#[test]
#[allow(clippy::too_many_lines)]
fn lane_manifest_registry_loads_fixture_manifests() -> Result<()> {
    let _alias_guard = install_fixture_alias_resolver();

    let lane_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![
            LaneConfig {
                id: LaneId::new(0),
                alias: "core".to_string(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(1),
                alias: "governance".to_string(),
                governance: Some("parliament".to_string()),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(2),
                alias: "zk".to_string(),
                governance: Some("council".to_string()),
                storage: LaneStorageProfile::CommitmentOnly,
                ..LaneConfig::default()
            },
        ],
    )?;

    let mut governance_catalog = GovernanceCatalog::default();
    governance_catalog.modules.insert(
        "parliament".to_string(),
        GovernanceModule {
            module_type: Some("parliament".to_string()),
            params: BTreeMap::new(),
        },
    );

    let registry_cfg = LaneRegistry {
        manifest_directory: Some(fixtures_path("fixtures/nexus/lanes/manifests")),
        cache_directory: Some(fixtures_path("fixtures/nexus/lanes/cache")),
        poll_interval: Duration::from_secs(0),
    };

    let registry =
        LaneManifestRegistry::from_config(&lane_catalog, &governance_catalog, &registry_cfg);

    let governance_status = registry
        .status(LaneId::new(1))
        .expect("governance lane status present");
    let governance_path = governance_status
        .manifest_path
        .as_ref()
        .expect("governance manifest should load");
    assert!(
        governance_path
            .file_name()
            .is_some_and(|name| name == "governance.manifest.json"),
        "expected governance manifest file",
    );
    let governance_rules = governance_status.rules().expect("governance rules parsed");
    assert_eq!(governance_rules.validators.len(), 3);
    assert_eq!(governance_rules.quorum, Some(2));
    assert!(
        governance_status
            .governance
            .as_deref()
            .is_some_and(|id| id == "parliament"),
        "governance lane should advertise the configured module",
    );
    assert!(
        governance_rules.hooks.runtime_upgrade.is_some(),
        "governance manifest should surface runtime upgrade hook",
    );
    assert!(
        governance_rules
            .protected_namespaces
            .iter()
            .any(|ns| ns.as_ref() == "apps"),
        "governance manifest should protect the `apps` namespace",
    );

    let zk_status = registry
        .status(LaneId::new(2))
        .expect("zk lane status present");
    let zk_path = zk_status
        .manifest_path
        .as_ref()
        .expect("zk manifest should load from cache");
    assert!(
        zk_path
            .file_name()
            .is_some_and(|name| name == "zk.manifest.json"),
        "expected zk manifest file",
    );
    let zk_rules = zk_status.rules().expect("zk rules parsed");
    assert_eq!(zk_rules.validators.len(), 3);
    assert_eq!(zk_rules.quorum, Some(2));
    assert!(
        zk_status
            .governance
            .as_deref()
            .is_some_and(|id| id == "council"),
        "overlay should provide the council governance module",
    );
    assert!(
        zk_rules
            .protected_namespaces
            .iter()
            .any(|ns| ns.as_ref() == "confidential"),
        "zk manifest should protect the `confidential` namespace",
    );
    assert_eq!(
        zk_status.privacy_commitments().len(),
        1,
        "zk manifest should advertise a privacy commitment"
    );

    assert!(
        registry.missing_aliases().is_empty(),
        "all governance lanes should have manifests"
    );
    assert!(
        registry.ensure_lane_ready(LaneId::new(0)).is_ok(),
        "lane without governance should be considered ready"
    );
    assert!(
        registry.ensure_lane_ready(LaneId::new(1)).is_ok(),
        "governance lane should be ready once manifest loads"
    );
    assert!(
        registry.ensure_lane_ready(LaneId::new(2)).is_ok(),
        "overlay manifest should ready the zk lane"
    );

    Ok(())
}

struct AliasResolverGuard;

impl Drop for AliasResolverGuard {
    fn drop(&mut self) {
        iroha_data_model::account::clear_account_alias_resolver();
    }
}

fn install_fixture_alias_resolver() -> AliasResolverGuard {
    use std::{collections::BTreeMap, sync::Arc};

    use iroha_data_model::account::{self, AccountId};

    let mut entries: BTreeMap<String, AccountId> = BTreeMap::new();
    entries.insert("alice".into(), iroha_test_samples::ALICE_ID.clone());
    entries.insert("bob".into(), iroha_test_samples::BOB_ID.clone());
    entries.insert("charlie".into(), seeded_account(0x11));
    entries.insert("dana".into(), seeded_account(0x22));
    entries.insert("erin".into(), seeded_account(0x33));
    entries.insert("frank".into(), seeded_account(0x44));

    let table = Arc::new(entries);
    account::clear_account_alias_resolver();
    account::set_account_alias_resolver(Arc::new(move |label, domain| {
        let key = label.to_ascii_lowercase();
        table.get(&key).and_then(|account| {
            if account.domain() == domain {
                Some(account.clone())
            } else {
                None
            }
        })
    }));

    AliasResolverGuard
}

fn seeded_account(seed: u8) -> iroha_data_model::account::AccountId {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, domain::DomainId};

    let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    let domain = DomainId::from_str("wonderland").expect("fixture domain");
    AccountId::new(domain, key_pair.public_key().clone())
}
