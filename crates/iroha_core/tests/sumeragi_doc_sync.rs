//! Doc-sync tests for the Sumeragi governance and evidence documentation.

use std::path::Path;

use iroha_config::parameters::defaults::sumeragi::npos;

#[test]
fn sumeragi_doc_mentions_reconfig_defaults_and_errors() {
    let repo_root = workspace_root();
    let doc = std::fs::read_to_string(repo_root.join("docs/source/sumeragi.md"))
        .expect("read sumeragi.md");

    let horizon = npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS;
    let activation = npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
    let slashing_delay = npos::SLASHING_DELAY_BLOCKS;

    assert!(
        doc.contains(&format!("evidence_horizon_blocks = {horizon}")),
        "docs/source/sumeragi.md is missing the canonical evidence_horizon_blocks default ({horizon})"
    );
    assert!(
        doc.contains(&format!("activation_lag_blocks = {activation}")),
        "docs/source/sumeragi.md is missing the canonical activation_lag_blocks default ({activation})"
    );
    assert!(
        doc.contains(&format!("slashing_delay_blocks = {slashing_delay}")),
        "docs/source/sumeragi.md is missing the canonical slashing_delay_blocks default ({slashing_delay})"
    );
    assert!(
        doc.contains("mode_activation_height requires next_mode to be set in the same block"),
        "docs/source/sumeragi.md must document the joint-consensus staging error message"
    );
}

#[test]
fn governance_api_doc_covers_joint_consensus_flow() {
    let repo_root = workspace_root();
    let doc = std::fs::read_to_string(repo_root.join("docs/source/governance_api.md"))
        .expect("read governance_api.md");

    let horizon = npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS;
    let activation = npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
    let slashing_delay = npos::SLASHING_DELAY_BLOCKS;

    assert!(
        doc.contains(&format!(
            "sumeragi.npos.reconfig.evidence_horizon_blocks` (default `{horizon}` blocks)"
        )),
        "docs/source/governance_api.md must mention the configured evidence horizon ({horizon})"
    );
    assert!(
        doc.contains(&format!(
            "sumeragi.npos.reconfig.activation_lag_blocks` (default `{activation}`)"
        )),
        "docs/source/governance_api.md must mention the configured activation lag ({activation})"
    );
    assert!(
        doc.contains(&format!(
            "sumeragi.npos.reconfig.slashing_delay_blocks` (default `{slashing_delay}`)"
        )),
        "docs/source/governance_api.md must mention the configured slashing delay ({slashing_delay})"
    );
    assert!(
        doc.contains("mode_activation_height requires next_mode to be set in the same block"),
        "docs/source/governance_api.md must document the joint-consensus staging error message"
    );
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
}
