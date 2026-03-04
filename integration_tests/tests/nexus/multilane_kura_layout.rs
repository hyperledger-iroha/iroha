#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Multilane storage layout smoke test: verifies that a multi-lane config
//! provisions per-lane Kura segments with deterministic names.

use eyre::Result;
use iroha_config::parameters::actual::Root as Config;
use iroha_config_base::toml::TomlSource;
use iroha_core::kura::Kura;
use std::path::Path;
use tempfile::tempdir;

fn multilane_profile(store_dir: &Path) -> String {
    format!(
        r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[kura]
store_dir = "{}"

[nexus]
enabled = true
lane_count = 2

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance lane"
dataspace = "governance"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance dataspace"

[nexus.routing_policy]
default_lane = 0
default_dataspace = "universal"

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
"#,
        store_dir.display()
    )
}

#[test]
fn kura_prepares_multilane_storage_layout() -> Result<()> {
    let tmp = tempdir()?;
    let toml = multilane_profile(tmp.path());
    let config =
        Config::from_toml_source(TomlSource::inline(toml)).expect("multilane config must parse");
    assert!(config.nexus.enabled, "nexus must be enabled for multilane profile");
    assert_eq!(
        config.nexus.lane_catalog.lane_count().get(),
        2,
        "expected two lanes in the catalog"
    );

    let lane_config = config.nexus.lane_config.clone();
    let (_kura, _) = Kura::new(&config.kura, &lane_config)
        .expect("kura must initialise per-lane segments");

    let blocks_root = tmp.path().join("blocks");
    let merge_root = tmp.path().join("merge_ledger");

    let core_blocks = blocks_root.join("lane_000_core");
    let gov_blocks = blocks_root.join("lane_001_governance");
    assert!(
        core_blocks.is_dir(),
        "core lane blocks directory should exist: {}",
        core_blocks.display()
    );
    assert!(
        gov_blocks.is_dir(),
        "governance lane blocks directory should exist: {}",
        gov_blocks.display()
    );

    let core_merge = merge_root.join("lane_000_core_merge.log");
    let gov_merge = merge_root.join("lane_001_governance_merge.log");
    assert!(
        core_merge.is_file(),
        "core lane merge log should exist: {}",
        core_merge.display()
    );
    assert!(
        gov_merge.is_file(),
        "governance lane merge log should exist: {}",
        gov_merge.display()
    );

    Ok(())
}
