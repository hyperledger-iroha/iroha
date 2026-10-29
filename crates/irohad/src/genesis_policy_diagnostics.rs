//! Field-level explanation of a signed genesis policy mismatch.
//!
//! Native genesis execution refuses a node whose configuration derives other policy commitments
//! than the signed genesis, and reports two hash pairs. `iroha3d --check-config` turns that
//! refusal into the differing fields: Kagami publishes the exact Nexus/AMX context preimage the
//! genesis commits as [`PUBLISHED_NEXUS_AMX_CONTEXT_FILE`] beside the signed genesis, and the
//! refusal retains the preimage this node derived. A published file is used only when its hash
//! equals the signed commitment. The execution policy has no published preimage, so it is
//! explained by section.
use iroha_config::parameters::actual::Root as Config;
use iroha_core::{block::BlockValidationError, sumeragi::startup::StartupError};
use iroha_crypto::Hash;
use iroha_data_model::nexus::{
    MAX_NEXUS_AMX_CONTEXT_BYTES_V1, NexusAmxContextFieldDiffV1, NexusAmxContextFieldOriginV1,
    NexusAmxContextPreimageV1, diff_nexus_amx_context_v1,
};
use std::{fmt::Write as _, io::Read as _, path::Path};

/// File name of the published Nexus/AMX context preimage, beside the signed genesis file.
pub(crate) const PUBLISHED_NEXUS_AMX_CONTEXT_FILE: &str = "nexus-amx-context.v1.bin";

/// Execution-policy inputs that still come from node configuration.
const EXECUTION_POLICY_SECTIONS: &str = "`gov`, `oracle`, `content`, `settlement`, and the `nexus` \
     keys outside the Nexus/AMX context (`endorsement`, `governance`, `compliance`, `registry` \
     lane manifests, `uploaded_models`, `hf_shared_leases`, `atomic_private_settlement`)";

/// Explain a refused genesis execution when it is a signed genesis policy mismatch.
pub(crate) fn explain_genesis_startup_error(
    config: &Config,
    error: &StartupError,
) -> Option<String> {
    let StartupError::InvalidGenesis(error) = error else {
        return None;
    };
    let BlockValidationError::GenesisPolicyMismatch {
        expected_execution,
        actual_execution,
        expected_nexus,
        actual_nexus_preimage,
        ..
    } = error.as_ref()
    else {
        return None;
    };
    let genesis_file = config
        .genesis
        .file
        .as_ref()
        .map(|file| file.resolve_relative_path());
    Some(explain_genesis_policy_mismatch(
        genesis_file.as_deref(),
        *expected_execution != *actual_execution,
        *expected_nexus,
        actual_nexus_preimage,
    ))
}

/// Name what differs between this node and its signed genesis.
///
/// `signed_genesis_file` locates the published preimage; `expected_nexus` is the signed
/// commitment it must hash to.
pub(crate) fn explain_genesis_policy_mismatch(
    signed_genesis_file: Option<&Path>,
    execution_differs: bool,
    expected_nexus: Hash,
    actual_nexus: &NexusAmxContextPreimageV1,
) -> String {
    let mut out = String::new();
    if actual_nexus.hash() == expected_nexus {
        out.push_str("this node derives the signed Nexus/AMX context");
    } else {
        explain_nexus_mismatch(&mut out, signed_genesis_file, expected_nexus, actual_nexus);
    }
    if execution_differs {
        let _ = write!(
            out,
            "\nthe execution policy differs too; besides the Nexus/AMX context it covers \
             {EXECUTION_POLICY_SECTIONS}: copy those from the network's reference node \
             configuration"
        );
    }
    out
}

fn explain_nexus_mismatch(
    out: &mut String,
    signed_genesis_file: Option<&Path>,
    expected_nexus: Hash,
    actual_nexus: &NexusAmxContextPreimageV1,
) {
    let Some(published) =
        signed_genesis_file.map(|file| file.with_file_name(PUBLISHED_NEXUS_AMX_CONTEXT_FILE))
    else {
        out.push_str(
            "the Nexus/AMX context differs; configure `genesis.file` and place the network's \
             `nexus-amx-context.v1.bin` beside it to list the differing fields",
        );
        return;
    };
    let display = published.display();
    let bytes = match read_bounded(&published) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let _ = write!(
                out,
                "the Nexus/AMX context differs; copy the network's `{PUBLISHED_NEXUS_AMX_CONTEXT_FILE}` \
                 (written by `kagami network create`, `kagami localnet` and \
                 `kagami genesis sign --nexus-context-output`) \
                 to {display} to list the differing fields"
            );
            return;
        }
        Err(error) => {
            let _ = write!(
                out,
                "the Nexus/AMX context differs; {display} cannot be read: {error}"
            );
            return;
        }
    };
    let published_hash = Hash::new(&bytes);
    if published_hash != expected_nexus {
        let _ = write!(
            out,
            "the Nexus/AMX context differs; {display} hashes to {published_hash}, not to the \
             signed commitment {expected_nexus}, so it belongs to another genesis"
        );
        return;
    }
    let diffs = match diff_nexus_amx_context_v1(&bytes, actual_nexus.as_bytes()) {
        Ok(diffs) => diffs,
        Err(error) => {
            let _ = write!(
                out,
                "the Nexus/AMX context differs; it cannot be compared field by field: {error}"
            );
            return;
        }
    };
    let _ = write!(
        out,
        "this node's Nexus/AMX context differs from the signed one ({display}) in {} field{}:",
        diffs.len(),
        if diffs.len() == 1 { "" } else { "s" }
    );
    for diff in &diffs {
        let _ = write!(out, "\n  {}", render_diff(diff));
    }
    out.push_str(
        "\nset the node-configuration values above to the signed genesis values; fields fixed \
         by genesis execution or the protocol are derived and are not node settings",
    );
}

fn render_diff(diff: &NexusAmxContextFieldDiffV1) -> String {
    let side = |value: &Option<String>| value.as_deref().unwrap_or("absent").to_owned();
    let origin = match diff.origin {
        NexusAmxContextFieldOriginV1::NodeConfig => "",
        NexusAmxContextFieldOriginV1::Genesis => " (fixed by genesis execution or protocol)",
    };
    format!(
        "{}{origin}: signed genesis {}, this node {}",
        diff.path,
        side(&diff.expected),
        side(&diff.actual)
    )
}

/// Read at most [`MAX_NEXUS_AMX_CONTEXT_BYTES_V1`] bytes; a longer file is refused.
fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    let limit = u64::try_from(MAX_NEXUS_AMX_CONTEXT_BYTES_V1).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_NEXUS_AMX_CONTEXT_BYTES_V1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("it exceeds {MAX_NEXUS_AMX_CONTEXT_BYTES_V1} bytes"),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::{Nexus, Pipeline, sumeragi_nexus_amx_context_preimage};

    fn preimage(exit_teu: u32) -> NexusAmxContextPreimageV1 {
        let mut nexus = Nexus::default();
        nexus.fusion.exit_teu = exit_teu;
        NexusAmxContextPreimageV1::new(sumeragi_nexus_amx_context_preimage(
            &nexus,
            None,
            &Pipeline::default(),
            &[],
            &[],
        ))
    }

    #[test]
    fn a_published_signed_context_names_the_differing_field_and_values() {
        let directory = tempfile::tempdir().unwrap();
        let genesis = directory.path().join("genesis.signed.nrt");
        let signed = preimage(50_000_000);
        std::fs::write(
            directory.path().join(PUBLISHED_NEXUS_AMX_CONTEXT_FILE),
            signed.as_bytes(),
        )
        .unwrap();
        let explanation =
            explain_genesis_policy_mismatch(Some(&genesis), true, signed.hash(), &preimage(6_000));
        assert!(
            explanation.contains(
                "in 1 field:\n  nexus.fusion.exit_teu: signed genesis 50000000, this node 6000\n"
            ),
            "{explanation}"
        );
        assert!(explanation.contains("`gov`, `oracle`"), "{explanation}");
    }

    #[test]
    fn a_missing_or_foreign_published_context_is_never_compared() {
        let directory = tempfile::tempdir().unwrap();
        let genesis = directory.path().join("genesis.signed.nrt");
        let published = directory.path().join(PUBLISHED_NEXUS_AMX_CONTEXT_FILE);
        let signed = preimage(50_000_000);
        let explain = || {
            explain_genesis_policy_mismatch(Some(&genesis), false, signed.hash(), &preimage(6_000))
        };
        let missing = explain();
        assert!(
            missing.contains(&format!("to {} to list", published.display())),
            "{missing}"
        );
        // A context of another genesis would name wrong fields; its hash refuses it.
        std::fs::write(&published, preimage(7_000).as_bytes()).unwrap();
        let foreign = explain();
        assert!(foreign.contains("belongs to another genesis"), "{foreign}");
        assert!(!foreign.contains("exit_teu"), "{foreign}");
        assert!(!foreign.contains("execution policy"), "{foreign}");
        std::fs::write(&published, vec![0; MAX_NEXUS_AMX_CONTEXT_BYTES_V1 + 1]).unwrap();
        let oversized = explain();
        assert!(oversized.contains("cannot be read"), "{oversized}");
        let unconfigured =
            explain_genesis_policy_mismatch(None, false, signed.hash(), &preimage(6_000));
        assert!(
            unconfigured.contains("configure `genesis.file`"),
            "{unconfigured}"
        );
    }

    #[test]
    fn an_execution_only_mismatch_names_the_sections_outside_the_context() {
        let context = preimage(6_000);
        let explanation = explain_genesis_policy_mismatch(None, true, context.hash(), &context);
        assert!(
            explanation.starts_with("this node derives the signed Nexus/AMX context\n"),
            "{explanation}"
        );
        assert!(
            explanation.contains(EXECUTION_POLICY_SECTIONS),
            "{explanation}"
        );
    }

    #[test]
    fn derived_fields_are_not_presented_as_editable_node_settings() {
        let mut diff = NexusAmxContextFieldDiffV1 {
            path: "nexus.lane_catalog[index=5].metadata".to_owned(),
            origin: NexusAmxContextFieldOriginV1::Genesis,
            expected: Some("{}".to_owned()),
            actual: None,
        };
        assert_eq!(
            render_diff(&diff),
            "nexus.lane_catalog[index=5].metadata (fixed by genesis execution or protocol): \
             signed genesis {}, this node absent"
        );
        diff.path = "nexus.fusion.exit_teu".to_owned();
        diff.origin = NexusAmxContextFieldOriginV1::NodeConfig;
        diff.expected = Some("50000000".to_owned());
        diff.actual = Some("6000".to_owned());
        assert_eq!(
            render_diff(&diff),
            "nexus.fusion.exit_teu: signed genesis 50000000, this node 6000"
        );
    }
}
