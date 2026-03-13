#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Multilane storage layout smoke test: verifies that a multi-lane config
//! provisions per-lane Kura segments with deterministic names.

use eyre::Result;
use iroha_config::parameters::actual::Root as Config;
use iroha_config_base::toml::TomlSource;
use iroha_core::kura::Kura;
use std::path::Path;
use tempfile::tempdir;
use toml::Table;

fn multilane_profile(store_dir: &Path) -> String {
    format!(
        r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers = [
  "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2@127.0.0.1:1337",
]

[[trusted_peers_pop]]
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08"

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[streaming]
identity_public_key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B"
identity_private_key = "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83"

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
visibility = "public"
metadata = {{}}

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance lane"
dataspace = "governance"
visibility = "restricted"
metadata = {{}}

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance dataspace"
fault_tolerance = 1

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
    let table: Table = toml.parse().expect("multilane profile must parse");
    let config =
        Config::from_toml_source(TomlSource::inline(table)).expect("multilane config must parse");
    assert!(
        config.nexus.enabled,
        "nexus must be enabled for multilane profile"
    );
    assert_eq!(
        config.nexus.lane_catalog.lane_count().get(),
        2,
        "expected two lanes in the catalog"
    );

    let lane_config = config.nexus.lane_config.clone();
    let (_kura, _) =
        Kura::new(&config.kura, &lane_config).expect("kura must initialise per-lane segments");

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
