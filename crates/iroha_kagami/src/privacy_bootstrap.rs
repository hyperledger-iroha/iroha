//! Fail-closed Exact12 governance activation templates for the Taira testnet.
use crate::{Outcome, RunArgs};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::privacy_profiles::{
    CompiledPrivacyProfileV1, compiled_privacy_profile_catalog_v1, compiled_privacy_profile_v1,
};
use iroha_crypto::sha256;
use iroha_data_model::{
    isi::{
        InstructionBox,
        privacy::{RegisterPrivacyProtocolActivationV1, TransitionPrivacyProtocolLifecycleV1},
    },
    privacy::{
        PRIVACY_COMPILED_PROFILE_CATALOG_SCHEMA_NAME_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1, PrivacyActiveLifecycleV1,
        PrivacyCompiledProfileCatalogRowV1, PrivacyCompiledProfileCatalogV1,
        PrivacyCompiledProfileResultV1, PrivacyProposedLifecycleV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
    },
};
use iroha_genesis::genesis_instructions_json;
use norito::json::Value as JsonValue;
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write},
    path::{Component, Path, PathBuf},
};
mod release;
const REPORT_SCHEMA_V1: &str = "iroha.taira.privacy-governance-templates.v1";
const BOOTSTRAP_EXECUTION_HEIGHT_V1: u64 = 1;
const BOOTSTRAP_INSTRUCTION_COUNT_V1: usize = PrivacyProtocolIdV1::COUNT * 2;
const MAX_INSTRUCTIONS_JSON_BYTES_V1: u64 = 4 * 1024 * 1024;
const MAX_REPORT_JSON_BYTES_V1: u64 = 8 * 1024 * 1024;
/// Emit or validate the exact first-release Taira privacy bootstrap.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Emit one height-1 template of twelve ordered registration/activation pairs.
    #[command(name = "emit-taira-v1")]
    EmitTairaV1(EmitTairaV1Args),
    /// Validate an emitted exact-12 instruction set and its digest inventory.
    #[command(name = "validate-taira-v1")]
    ValidateTairaV1(ValidateTairaV1Args),
    /// Validate a reviewed Taira NEVO genesis source template without creating release artifacts.
    #[command(name = "validate-taira-nevo-review-v1")]
    ValidateTairaNevoReviewV1(release::ValidateTairaNevoReviewV1Args),
    /// Compose a secret-free Taira release plan, config, and non-signable genesis source template.
    #[command(name = "render-taira-release-v1")]
    RenderTairaReleaseV1(Box<release::RenderTairaReleaseV1Args>),
}
#[derive(Debug, ClapArgs)]
struct EmitTairaV1Args {
    /// New file receiving the canonical governance-template instruction array.
    #[arg(long)]
    instructions_output: PathBuf,
    /// New file receiving base64 Norito instructions and deterministic digests.
    #[arg(long)]
    report_output: PathBuf,
}
#[derive(Debug, ClapArgs)]
struct ValidateTairaV1Args {
    /// Canonical genesis instruction JSON array emitted by this command group.
    #[arg(long)]
    instructions: PathBuf,
    /// Canonical digest inventory emitted alongside the instruction array.
    #[arg(long)]
    report: PathBuf,
}
#[derive(Clone, Debug)]
struct TairaPrivacyBootstrapArtifactsV1 {
    instructions: Vec<InstructionBox>,
    catalog: PrivacyCompiledProfileCatalogV1,
    instructions_json: Vec<u8>,
    report_json: Vec<u8>,
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        match self.command {
            Command::EmitTairaV1(args) => {
                let artifacts = build_taira_privacy_bootstrap_v1()?;
                write_new_artifact_pair(
                    &args.instructions_output,
                    &artifacts.instructions_json,
                    &args.report_output,
                    &artifacts.report_json,
                )?;
                let status = norito::json!({
                    "status": "emitted",
                    "instructions_path": (args.instructions_output.display().to_string()),
                    "instructions_json_sha256": (hex::encode(sha256(&artifacts.instructions_json))),
                    "report_path": (args.report_output.display().to_string()),
                    "report_json_sha256": (hex::encode(sha256(&artifacts.report_json))),
                    "instruction_count": (BOOTSTRAP_INSTRUCTION_COUNT_V1 as u64),
                });
                writeln!(writer, "{}", norito::json::to_json(&status)?)?;
            }
            Command::ValidateTairaV1(args) => {
                let instructions_json = read_bounded(
                    &args.instructions,
                    MAX_INSTRUCTIONS_JSON_BYTES_V1,
                    "privacy bootstrap instructions",
                )?;
                let report_json = read_bounded(
                    &args.report,
                    MAX_REPORT_JSON_BYTES_V1,
                    "privacy bootstrap report",
                )?;
                validate_taira_privacy_bootstrap_v1(&instructions_json, &report_json)?;
                let status = norito::json!({
                    "status": "validated",
                    "instructions_path": (args.instructions.display().to_string()),
                    "instructions_json_sha256": (hex::encode(sha256(&instructions_json))),
                    "report_path": (args.report.display().to_string()),
                    "report_json_sha256": (hex::encode(sha256(&report_json))),
                    "instruction_count": (BOOTSTRAP_INSTRUCTION_COUNT_V1 as u64),
                });
                writeln!(writer, "{}", norito::json::to_json(&status)?)?;
            }
            Command::ValidateTairaNevoReviewV1(args) => {
                release::validate_taira_nevo_review_v1(&args, writer)?;
            }
            Command::RenderTairaReleaseV1(args) => {
                release::render_taira_release_v1(&args, writer)?;
            }
        }
        Ok(())
    }
}
fn bootstrap_active_lifecycle_v1() -> PrivacyProtocolLifecycleV1 {
    PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: BOOTSTRAP_EXECUTION_HEIGHT_V1,
        activated_at_height: BOOTSTRAP_EXECUTION_HEIGHT_V1,
        state_since_height: BOOTSTRAP_EXECUTION_HEIGHT_V1,
    })
}
fn build_taira_privacy_bootstrap_v1() -> color_eyre::Result<TairaPrivacyBootstrapArtifactsV1> {
    let profiles = PrivacyProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| {
            compiled_privacy_profile_v1(protocol_id).map_err(|source| {
                eyre!(
                    "compiled privacy profile `{}` is unavailable on this release candidate: {source}",
                    protocol_id.canonical_label()
                )
            })
        })
        .collect::<color_eyre::Result<Vec<_>>>()?;
    let artifacts = build_artifacts_from_profiles_v1(&profiles)?;
    let local_catalog = compiled_privacy_profile_catalog_v1()
        .map_err(|source| eyre!("local compiled privacy catalog is invalid: {source}"))?;
    if artifacts.catalog != local_catalog {
        bail!(
            "profiles compiled for the Taira bootstrap differ from the local compiled-profile catalog"
        );
    }
    Ok(artifacts)
}
fn build_artifacts_from_profiles_v1(
    profiles: &[CompiledPrivacyProfileV1],
) -> color_eyre::Result<TairaPrivacyBootstrapArtifactsV1> {
    if profiles.len() != PrivacyProtocolIdV1::COUNT {
        bail!(
            "Taira privacy bootstrap requires exactly {} compiled profiles, got {}",
            PrivacyProtocolIdV1::COUNT,
            profiles.len()
        );
    }
    let mut seen = BTreeSet::new();
    let mut instructions = Vec::with_capacity(BOOTSTRAP_INSTRUCTION_COUNT_V1);
    let mut catalog_rows = Vec::with_capacity(PrivacyProtocolIdV1::COUNT);
    for (index, (profile, expected_protocol)) in profiles
        .iter()
        .copied()
        .zip(PrivacyProtocolIdV1::ALL)
        .enumerate()
    {
        if !seen.insert(profile.protocol_id) {
            bail!(
                "duplicate compiled privacy profile `{}` at index {index}",
                profile.protocol_id.canonical_label()
            );
        }
        if profile.protocol_id != expected_protocol {
            bail!(
                "compiled privacy profile order mismatch at index {index}: expected `{}`, got `{}`",
                expected_protocol.canonical_label(),
                profile.protocol_id.canonical_label()
            );
        }
        let lifecycle = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height: BOOTSTRAP_EXECUTION_HEIGHT_V1,
        });
        let activation = profile.activation_record(lifecycle);
        activation.validate().map_err(|source| {
            eyre!(
                "compiled activation `{}` is invalid: {source}",
                profile.protocol_id.canonical_label()
            )
        })?;
        instructions.push(InstructionBox::from(
            RegisterPrivacyProtocolActivationV1::new(activation),
        ));
        instructions.push(InstructionBox::from(
            TransitionPrivacyProtocolLifecycleV1::new(
                profile.protocol_id,
                bootstrap_active_lifecycle_v1(),
            ),
        ));
        catalog_rows.push(PrivacyCompiledProfileCatalogRowV1 {
            protocol_id: profile.protocol_id,
            compiled_profile: PrivacyCompiledProfileResultV1::Available(profile.into()),
        });
    }
    let catalog = PrivacyCompiledProfileCatalogV1 {
        version: PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1,
        protocols: catalog_rows,
    };
    catalog.validate().map_err(|source| {
        eyre!("derived exact-12 compiled-profile catalog is invalid: {source}")
    })?;
    render_artifacts_v1(instructions, catalog)
}
fn render_artifacts_v1(
    instructions: Vec<InstructionBox>,
    catalog: PrivacyCompiledProfileCatalogV1,
) -> color_eyre::Result<TairaPrivacyBootstrapArtifactsV1> {
    validate_instruction_semantics_v1(&instructions, &instructions)?;
    let mut instructions_json = String::new();
    genesis_instructions_json::serialize(&instructions, &mut instructions_json);
    instructions_json.push('\n');
    let instructions_json = instructions_json.into_bytes();
    let labels = PrivacyProtocolIdV1::ALL.map(|id| id.canonical_label().to_owned());
    let mut instruction_labels = Vec::with_capacity(instructions.len());
    let mut wire_ids = Vec::with_capacity(instructions.len());
    let mut instruction_norito_base64 = Vec::with_capacity(instructions.len());
    let mut instruction_norito_sha256 = Vec::with_capacity(instructions.len());
    for (index, instruction) in instructions.iter().enumerate() {
        let (protocol, wire_id) = privacy_instruction_identity_at_v1(instruction, index)?;
        let encoded = norito::to_bytes(instruction).wrap_err_with(|| {
            format!("failed to encode privacy bootstrap instruction {index} as Norito")
        })?;
        instruction_labels.push(protocol.canonical_label().to_owned());
        wire_ids.push(wire_id);
        instruction_norito_base64.push(BASE64_STANDARD.encode(&encoded));
        instruction_norito_sha256.push(hex::encode(sha256(&encoded)));
    }
    let instruction_set_norito = norito::to_bytes(&instructions)
        .wrap_err("failed to encode exact-12 privacy instruction set as Norito")?;
    let catalog_norito = norito::to_bytes(&catalog)
        .wrap_err("failed to encode exact-12 compiled-profile catalog as Norito")?;
    let report = norito::json!({
        "schema": (REPORT_SCHEMA_V1),
        "schema_version": 1_u64,
        "governance_activation_templates": {
            "deployment_state": "not-executed",
            "execution_mode": "explicit-register-then-activate",
            "execution_height": (BOOTSTRAP_EXECUTION_HEIGHT_V1),
            "instruction_count": (BOOTSTRAP_INSTRUCTION_COUNT_V1 as u64),
            "instruction_wire_ids": (wire_ids),
            "instruction_protocol_labels": (instruction_labels),
            "instruction_encoding": "norito-instruction-box-base64",
            "protocol_count": (PrivacyProtocolIdV1::COUNT as u64),
            "protocol_labels": (labels),
            "instruction_norito_base64": (instruction_norito_base64),
            "instruction_norito_sha256": (instruction_norito_sha256),
            "instruction_set_norito_sha256": (hex::encode(sha256(&instruction_set_norito))),
            "genesis_instructions_json_sha256": (hex::encode(sha256(&instructions_json))),
        },
        "privacy_catalog": {
            "schema": (PRIVACY_COMPILED_PROFILE_CATALOG_SCHEMA_NAME_V1),
            "norito_sha256": (hex::encode(sha256(&catalog_norito))),
        },
    });
    let mut report_json = norito::json::to_json(&report)
        .wrap_err("failed to encode canonical Taira privacy bootstrap report")?;
    report_json.push('\n');
    Ok(TairaPrivacyBootstrapArtifactsV1 {
        instructions,
        catalog,
        instructions_json,
        report_json: report_json.into_bytes(),
    })
}
fn validate_taira_privacy_bootstrap_v1(
    instructions_json: &[u8],
    report_json: &[u8],
) -> color_eyre::Result<()> {
    if u64::try_from(instructions_json.len()).unwrap_or(u64::MAX) > MAX_INSTRUCTIONS_JSON_BYTES_V1 {
        bail!("privacy bootstrap instructions exceed the fixed byte limit");
    }
    if u64::try_from(report_json.len()).unwrap_or(u64::MAX) > MAX_REPORT_JSON_BYTES_V1 {
        bail!("privacy bootstrap report exceeds the fixed byte limit");
    }
    let expected = build_taira_privacy_bootstrap_v1()?;
    validate_artifacts_against_v1(instructions_json, report_json, &expected)
}
fn validate_artifacts_against_v1(
    instructions_json: &[u8],
    report_json: &[u8],
    expected: &TairaPrivacyBootstrapArtifactsV1,
) -> color_eyre::Result<()> {
    iroha_genesis::init_instruction_registry();
    let instructions_value: JsonValue = norito::json::from_slice(instructions_json)
        .wrap_err("privacy bootstrap instructions are not valid Norito JSON")?;
    let instructions = genesis_instructions_json::from_value(&instructions_value)
        .wrap_err("privacy bootstrap instruction JSON cannot be decoded canonically")?;
    validate_instruction_semantics_v1(&instructions, &expected.instructions)?;
    let mut canonical_instructions = String::new();
    genesis_instructions_json::serialize(&instructions, &mut canonical_instructions);
    canonical_instructions.push('\n');
    if canonical_instructions.as_bytes() != instructions_json {
        bail!("privacy bootstrap instruction JSON is not in canonical emitted form");
    }
    let report_value: JsonValue = norito::json::from_slice(report_json)
        .wrap_err("privacy bootstrap report is not valid Norito JSON")?;
    validate_report_inventory_v1(&report_value, &instructions)?;
    if report_json != expected.report_json {
        bail!("privacy bootstrap report differs from the exact local compiled-profile inventory");
    }
    if instructions_json != expected.instructions_json {
        bail!("privacy bootstrap instructions differ from the exact local compiled profiles");
    }
    Ok(())
}
fn validate_instruction_semantics_v1(
    instructions: &[InstructionBox],
    expected: &[InstructionBox],
) -> color_eyre::Result<()> {
    if instructions.len() != BOOTSTRAP_INSTRUCTION_COUNT_V1 {
        bail!(
            "privacy bootstrap must contain exactly {} ordered registration/activation instructions, got {}",
            BOOTSTRAP_INSTRUCTION_COUNT_V1,
            instructions.len()
        );
    }
    if expected.len() != BOOTSTRAP_INSTRUCTION_COUNT_V1 {
        bail!("internal exact-12 privacy bootstrap expectation is incomplete");
    }
    let mut seen = BTreeSet::new();
    for (index, ((pair, expected_pair), expected_protocol)) in instructions
        .chunks_exact(2)
        .zip(expected.chunks_exact(2))
        .zip(PrivacyProtocolIdV1::ALL)
        .enumerate()
    {
        let actual = privacy_activation_at_v1(&pair[0], index * 2)?;
        let expected_activation = privacy_activation_at_v1(&expected_pair[0], index * 2)?;
        if !seen.insert(actual.protocol_id) {
            bail!("privacy bootstrap contains duplicate protocol at pair {index}");
        }
        if actual.protocol_id != expected_protocol {
            bail!("privacy bootstrap protocol order mismatch at pair {index}");
        }
        actual.validate().map_err(|source| {
            eyre!("privacy bootstrap activation at pair {index} is structurally invalid: {source}")
        })?;
        let proposed = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height: BOOTSTRAP_EXECUTION_HEIGHT_V1,
        });
        if actual.lifecycle != proposed || actual != expected_activation {
            bail!(
                "privacy bootstrap registration at pair {index} differs from the exact compiled profile or execution height"
            );
        }
        let transition = privacy_transition_at_v1(&pair[1], index * 2 + 1)?;
        let expected_transition = privacy_transition_at_v1(&expected_pair[1], index * 2 + 1)?;
        if transition.protocol_id != actual.protocol_id
            || transition.next_lifecycle != bootstrap_active_lifecycle_v1()
            || transition != expected_transition
        {
            bail!(
                "privacy bootstrap transition at pair {index} is not the exact same-height Active transition"
            );
        }
        actual
            .lifecycle
            .validate_transition_to(&transition.next_lifecycle)
            .map_err(|source| {
                eyre!("privacy bootstrap transition at pair {index} is invalid: {source}")
            })?;
    }
    Ok(())
}
fn privacy_transition_at_v1(
    instruction: &InstructionBox,
    index: usize,
) -> color_eyre::Result<&TransitionPrivacyProtocolLifecycleV1> {
    instruction
        .as_any()
        .downcast_ref::<TransitionPrivacyProtocolLifecycleV1>()
        .ok_or_else(|| {
            eyre!(
                "privacy bootstrap instruction {index} is not `{}`",
                TransitionPrivacyProtocolLifecycleV1::WIRE_ID
            )
        })
}
fn privacy_instruction_identity_at_v1(
    instruction: &InstructionBox,
    index: usize,
) -> color_eyre::Result<(PrivacyProtocolIdV1, &'static str)> {
    if index % 2 == 0 {
        Ok((
            privacy_activation_at_v1(instruction, index)?.protocol_id,
            RegisterPrivacyProtocolActivationV1::WIRE_ID,
        ))
    } else {
        Ok((
            privacy_transition_at_v1(instruction, index)?.protocol_id,
            TransitionPrivacyProtocolLifecycleV1::WIRE_ID,
        ))
    }
}

fn privacy_activation_at_v1(
    instruction: &InstructionBox,
    index: usize,
) -> color_eyre::Result<&PrivacyProtocolActivationRecordV1> {
    instruction
        .as_any()
        .downcast_ref::<RegisterPrivacyProtocolActivationV1>()
        .map(|registration| &registration.activation)
        .ok_or_else(|| {
            eyre!(
                "privacy bootstrap instruction {index} is not `{}`",
                RegisterPrivacyProtocolActivationV1::WIRE_ID
            )
        })
}
fn validate_report_inventory_v1(
    report: &JsonValue,
    instructions: &[InstructionBox],
) -> color_eyre::Result<()> {
    let fields = report
        .as_object()
        .ok_or_else(|| eyre!("privacy bootstrap report root must be an object"))?;
    let schema = fields
        .get("schema")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("privacy bootstrap report schema must be a string"))?;
    if schema != REPORT_SCHEMA_V1 {
        bail!("privacy bootstrap report schema is not `{REPORT_SCHEMA_V1}`");
    }
    let registration = fields
        .get("governance_activation_templates")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            eyre!("privacy bootstrap report governance_activation_templates must be an object")
        })?;
    if registration
        .get("deployment_state")
        .and_then(JsonValue::as_str)
        != Some("not-executed")
        || registration
            .get("execution_mode")
            .and_then(JsonValue::as_str)
            != Some("explicit-register-then-activate")
        || registration
            .get("execution_height")
            .and_then(JsonValue::as_u64)
            != Some(BOOTSTRAP_EXECUTION_HEIGHT_V1)
        || registration
            .get("instruction_count")
            .and_then(JsonValue::as_u64)
            != Some(BOOTSTRAP_INSTRUCTION_COUNT_V1 as u64)
        || registration
            .get("protocol_count")
            .and_then(JsonValue::as_u64)
            != Some(PrivacyProtocolIdV1::COUNT as u64)
    {
        bail!(
            "privacy bootstrap report does not bind the unexecuted height-1 explicit activation contract"
        );
    }
    let labels = report_string_array_v1(registration, "protocol_labels")?;
    if labels.len() != PrivacyProtocolIdV1::COUNT
        || labels
            .iter()
            .zip(PrivacyProtocolIdV1::ALL)
            .any(|(label, protocol)| *label != protocol.canonical_label())
    {
        bail!("privacy bootstrap report must bind the unique ordered Exact12 protocol inventory");
    }
    let instruction_labels = report_string_array_v1(registration, "instruction_protocol_labels")?;
    let wire_ids = report_string_array_v1(registration, "instruction_wire_ids")?;
    let base64_values = report_string_array_v1(registration, "instruction_norito_base64")?;
    let hashes = report_string_array_v1(registration, "instruction_norito_sha256")?;
    for (field, len) in [
        ("instruction_protocol_labels", instruction_labels.len()),
        ("instruction_wire_ids", wire_ids.len()),
        ("instruction_norito_base64", base64_values.len()),
        ("instruction_norito_sha256", hashes.len()),
    ] {
        if len != BOOTSTRAP_INSTRUCTION_COUNT_V1 {
            bail!(
                "privacy bootstrap report `{field}` must contain exactly {} entries, got {len}",
                BOOTSTRAP_INSTRUCTION_COUNT_V1
            );
        }
    }
    if instructions.len() != BOOTSTRAP_INSTRUCTION_COUNT_V1 {
        bail!("privacy bootstrap report instruction population is incomplete");
    }
    for (index, ((((label, wire_id), encoded), claimed_hash), instruction)) in instruction_labels
        .iter()
        .zip(&wire_ids)
        .zip(&base64_values)
        .zip(&hashes)
        .zip(instructions)
        .enumerate()
    {
        let (protocol, expected_wire_id) = privacy_instruction_identity_at_v1(instruction, index)?;
        if *label != protocol.canonical_label() || *wire_id != expected_wire_id {
            bail!("privacy bootstrap report label mismatch at index {index}");
        }
        let decoded = BASE64_STANDARD.decode(encoded).map_err(|source| {
            eyre!("privacy bootstrap report base64 at index {index} is invalid: {source}")
        })?;
        if BASE64_STANDARD.encode(&decoded) != *encoded {
            bail!("privacy bootstrap report base64 at index {index} is not canonical");
        }
        let actual_hash = hex::encode(sha256(&decoded));
        if *claimed_hash != actual_hash {
            bail!("privacy bootstrap report Norito SHA-256 mismatch at index {index}");
        }
        let decoded_instruction =
            norito::decode_from_bytes::<InstructionBox>(&decoded).map_err(|source| {
                eyre!("privacy bootstrap report Norito instruction {index} is invalid: {source}")
            })?;
        let decoded_identity = privacy_instruction_identity_at_v1(&decoded_instruction, index)?;
        if decoded_identity != (protocol, expected_wire_id)
            || norito::to_bytes(instruction)? != decoded
        {
            bail!("privacy bootstrap report Norito instruction mismatch at index {index}");
        }
        let reencoded = norito::to_bytes(&decoded_instruction).wrap_err_with(|| {
            format!("failed to re-encode privacy bootstrap report instruction {index}")
        })?;
        if reencoded != decoded {
            bail!("privacy bootstrap report Norito at index {index} is not canonical");
        }
    }
    Ok(())
}
fn report_string_array_v1<'a>(
    fields: &'a norito::json::Map,
    field: &str,
) -> color_eyre::Result<Vec<&'a str>> {
    fields
        .get(field)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("privacy bootstrap report `{field}` must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().ok_or_else(|| {
                eyre!("privacy bootstrap report `{field}` entry {index} must be a string")
            })
        })
        .collect()
}
fn read_bounded(path: &Path, max_bytes: u64, description: &str) -> color_eyre::Result<Vec<u8>> {
    let before =
        fs::symlink_metadata(path).wrap_err_with(|| format!("failed to inspect {description}"))?;
    if !before.is_file() || before.file_type().is_symlink() {
        bail!("{description} must be one non-symlink regular file");
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&before) != 1 {
        bail!("{description} must have exactly one filesystem link");
    }
    if before.len() > max_bytes {
        bail!("{description} exceeds the fixed {max_bytes}-byte limit");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {description}"))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {description}"))?;
    if !same_input_metadata_v1(&before, &opened) {
        bail!("{description} changed before its immutable snapshot was opened");
    }
    let mut bytes = Vec::new();
    (&file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {description}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        bail!("{description} exceeds the fixed {max_bytes}-byte limit");
    }
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {description}"))?;
    if !same_input_metadata_v1(&opened, &after) {
        bail!("{description} changed while its immutable snapshot was read");
    }
    Ok(bytes)
}
#[cfg(unix)]
fn same_input_metadata_v1(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.is_file()
        && right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(not(unix))]
fn same_input_metadata_v1(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file()
        && right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}
fn write_new_artifact_pair(
    first_path: &Path,
    first_bytes: &[u8],
    second_path: &Path,
    second_bytes: &[u8],
) -> color_eyre::Result<()> {
    if resolved_new_output_path_v1(first_path)? == resolved_new_output_path_v1(second_path)? {
        bail!("privacy bootstrap instructions and report paths must differ");
    }
    let mut first = create_new_file(first_path, "privacy bootstrap instructions")?;
    let mut second = match create_new_file(second_path, "privacy bootstrap report") {
        Ok(file) => file,
        Err(error) => {
            remove_created_file_if_unchanged_v1(first_path, &first);
            drop(first);
            return Err(error);
        }
    };
    let result = (|| -> color_eyre::Result<()> {
        first
            .write_all(first_bytes)
            .wrap_err("failed to write privacy bootstrap instructions")?;
        first
            .sync_all()
            .wrap_err("failed to sync privacy bootstrap instructions")?;
        second
            .write_all(second_bytes)
            .wrap_err("failed to write privacy bootstrap report")?;
        second
            .sync_all()
            .wrap_err("failed to sync privacy bootstrap report")?;
        Ok(())
    })();
    if let Err(error) = result {
        remove_created_file_if_unchanged_v1(first_path, &first);
        remove_created_file_if_unchanged_v1(second_path, &second);
        drop(first);
        drop(second);
        return Err(error);
    }
    Ok(())
}
fn create_new_file(path: &Path, description: &str) -> color_eyre::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .wrap_err_with(|| format!("failed to create new {description} at `{}`", path.display()))
}
fn resolved_new_output_path_v1(path: &Path) -> color_eyre::Result<PathBuf> {
    let Some(Component::Normal(file_name)) = path.components().next_back() else {
        bail!("new artifact output must end in one normal file name");
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let resolved_parent = fs::canonicalize(parent).wrap_err_with(|| {
        format!(
            "failed to resolve artifact output parent `{}`",
            parent.display()
        )
    })?;
    if !resolved_parent.is_dir() {
        bail!("artifact output parent must be a directory");
    }
    Ok(resolved_parent.join(file_name))
}
fn remove_created_file_if_unchanged_v1(path: &Path, file: &File) {
    let Ok(named) = fs::symlink_metadata(path) else {
        return;
    };
    let Ok(opened) = file.metadata() else {
        return;
    };
    if same_input_metadata_v1(&named, &opened) {
        let _ = fs::remove_file(path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    // Structural fixture only: never substitutes for the production all-engine gate.
    fn structural_profiles_v1() -> Vec<CompiledPrivacyProfileV1> {
        use iroha_data_model::privacy::{
            AnonymousPgcActivationLimitsV1, FcmpActivationLimitsV1,
            IvmPrivateNoteActivationLimitsV1, JindoActivationLimitsV1, OrchardActivationLimitsV1,
            PqMaspActivationLimitsV1, PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1,
            PrivacyParameterIdV1, PrivacyProtocolActivationLimitsV1 as Limits,
            PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1, VeRangeActivationLimitsV1,
            ZkAmsActivationLimitsV1,
        };
        PrivacyProtocolIdV1::ALL
            .into_iter()
            .map(|protocol_id| {
                let protocol_limits = match protocol_id {
                    PrivacyProtocolIdV1::ZkAcePqAuthorizationV1 => Limits::ZkAcePqAuthorizationV1,
                    PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                        Limits::AnonymousPgcKOutOfNV1(AnonymousPgcActivationLimitsV1 {
                            max_anonymity_set_size: 16,
                            max_recipient_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
                        Limits::VeRangeTransparentRangeV1(VeRangeActivationLimitsV1 {
                            max_aggregation_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::IrohaZkAmsV1 => {
                        Limits::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                            max_batch_size: 1,
                            max_ring_size: 16,
                        })
                    }
                    PrivacyProtocolIdV1::VegaExistingCredentialZkV1 => {
                        Limits::VegaExistingCredentialZkV1
                    }
                    PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 => Limits::IrohaZkX509StarkP256V1,
                    PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => {
                        Limits::IrohaJindoPolynomialCommitmentV1(JindoActivationLimitsV1 {
                            max_polynomial_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                        Limits::IrohaBootleLanternAnoncredV1
                    }
                    PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
                        Limits::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                            max_action_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                        Limits::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                            max_input_count: 1,
                            max_output_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                        Limits::IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteActivationLimitsV1 {
                            max_input_count: 1,
                            max_output_count: 1,
                        })
                    }
                    PrivacyProtocolIdV1::PqMaspStarkV1 => {
                        Limits::PqMaspStarkV1(PqMaspActivationLimitsV1 {
                            max_input_count: 1,
                            max_output_count: 1,
                        })
                    }
                };
                CompiledPrivacyProfileV1 {
                    protocol_id,
                    proof_system_id: protocol_id.expected_proof_system(),
                    engine_id: protocol_id.expected_engine(),
                    parameter_id: PrivacyParameterIdV1::new([1; 32]),
                    parameter_digest: PrivacyParameterDigestV1::new([2; 32]),
                    verifier_digest: PrivacyVerifierDigestV1::new([3; 32]),
                    statement_schema_digest: PrivacyStatementSchemaDigestV1::new([4; 32]),
                    engine_manifest_digest: PrivacyEngineManifestDigestV1::new([5; 32]),
                    protocol_limits,
                }
            })
            .collect()
    }
    #[test]
    fn exact12_templates_pair_registration_and_same_height_explicit_activation() {
        let profiles = structural_profiles_v1();
        let artifacts = build_artifacts_from_profiles_v1(&profiles).expect("structural fixture");
        assert_eq!(artifacts.instructions.len(), 24);
        assert_eq!(artifacts.catalog.protocols.len(), 12);
        for (index, (pair, profile)) in artifacts
            .instructions
            .chunks_exact(2)
            .zip(&profiles)
            .enumerate()
        {
            let registration = privacy_activation_at_v1(&pair[0], index * 2).expect("registration");
            assert_eq!(registration.protocol_id, profile.protocol_id);
            let transition = privacy_transition_at_v1(&pair[1], index * 2 + 1).expect("transition");
            assert_eq!(transition.protocol_id, profile.protocol_id);
            assert_eq!(transition.next_lifecycle, bootstrap_active_lifecycle_v1());
            registration
                .lifecycle
                .validate_transition_to(&transition.next_lifecycle)
                .expect("explicit same-height edge");
        }
        validate_artifacts_against_v1(
            &artifacts.instructions_json,
            &artifacts.report_json,
            &artifacts,
        )
        .expect("canonical JSON and Norito report roundtrip");
        let report: JsonValue = norito::json::from_slice(&artifacts.report_json).expect("report");
        for (key, count) in [
            ("protocol_labels", 12),
            ("instruction_protocol_labels", 24),
            ("instruction_wire_ids", 24),
            ("instruction_norito_base64", 24),
            ("instruction_norito_sha256", 24),
        ] {
            assert_eq!(
                report
                    .get("governance_activation_templates")
                    .and_then(|value| value.get(key))
                    .and_then(JsonValue::as_array)
                    .expect("inventory array")
                    .len(),
                count
            );
        }
    }
    #[test]
    fn explicit_bootstrap_rejects_missing_duplicate_reordered_and_rebound_pairs() {
        let artifacts = build_artifacts_from_profiles_v1(&structural_profiles_v1())
            .expect("structural fixture");
        let expected = &artifacts.instructions;
        let mut cases = Vec::new();
        for index in [0, 1, 23] {
            let mut changed = expected.clone();
            changed.remove(index);
            cases.push(changed);
        }
        let mut changed = expected.clone();
        changed.extend_from_slice(&expected[..2]);
        cases.push(changed);
        let mut changed = expected.clone();
        changed.swap(0, 1);
        cases.push(changed);
        let mut changed = expected.clone();
        changed[22] = expected[0].clone();
        changed[23] = expected[1].clone();
        cases.push(changed);
        let mut changed = expected.clone();
        changed[1] = InstructionBox::from(TransitionPrivacyProtocolLifecycleV1::new(
            PrivacyProtocolIdV1::ALL[1],
            bootstrap_active_lifecycle_v1(),
        ));
        cases.push(changed);
        let mut changed = expected.clone();
        changed[1] = InstructionBox::from(TransitionPrivacyProtocolLifecycleV1::new(
            PrivacyProtocolIdV1::ALL[0],
            PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            }),
        ));
        cases.push(changed);
        let mut changed = expected.clone();
        let mut registration = *privacy_activation_at_v1(&changed[0], 0).expect("registration");
        registration.parameter_digest =
            iroha_data_model::privacy::PrivacyParameterDigestV1::new([9; 32]);
        changed[0] = InstructionBox::from(RegisterPrivacyProtocolActivationV1::new(registration));
        cases.push(changed);
        for (index, changed) in cases.iter().enumerate() {
            assert!(
                validate_instruction_semantics_v1(changed, expected).is_err(),
                "accepted hostile pair population {index}"
            );
        }
    }
    #[test]
    fn explicit_bootstrap_report_rejects_every_incomplete_identity_population() {
        let artifacts = build_artifacts_from_profiles_v1(&structural_profiles_v1())
            .expect("structural fixture");
        for key in [
            "protocol_labels",
            "instruction_protocol_labels",
            "instruction_wire_ids",
            "instruction_norito_base64",
            "instruction_norito_sha256",
        ] {
            let mut report: JsonValue =
                norito::json::from_slice(&artifacts.report_json).expect("report");
            report
                .get_mut("governance_activation_templates")
                .and_then(|value| value.get_mut(key))
                .and_then(JsonValue::as_array_mut)
                .expect("inventory array")
                .pop();
            assert!(
                validate_report_inventory_v1(&report, &artifacts.instructions).is_err(),
                "accepted incomplete {key}"
            );
        }
        let mut report: JsonValue =
            norito::json::from_slice(&artifacts.report_json).expect("report");
        report
            .get_mut("governance_activation_templates")
            .and_then(|value| value.get_mut("instruction_wire_ids"))
            .and_then(JsonValue::as_array_mut)
            .expect("wire IDs")
            .swap(0, 1);
        assert!(validate_report_inventory_v1(&report, &artifacts.instructions).is_err());
    }
    #[test]
    fn explicit_bootstrap_report_rejects_promoted_or_retimed_execution_metadata() {
        let artifacts = build_artifacts_from_profiles_v1(&structural_profiles_v1())
            .expect("structural fixture");
        for (key, value) in [
            ("deployment_state", norito::json!("executed")),
            ("execution_mode", norito::json!("governance-four-wave")),
            ("execution_height", norito::json!(2_u64)),
            ("instruction_count", norito::json!(12_u64)),
            ("protocol_count", norito::json!(24_u64)),
        ] {
            let mut report: JsonValue =
                norito::json::from_slice(&artifacts.report_json).expect("report");
            report
                .get_mut("governance_activation_templates")
                .and_then(JsonValue::as_object_mut)
                .expect("contract")
                .insert(key.to_owned(), value);
            assert!(
                validate_report_inventory_v1(&report, &artifacts.instructions).is_err(),
                "accepted hostile {key}"
            );
        }
    }
    #[test]
    fn exact12_bootstrap_fails_closed_when_any_engine_is_unavailable() {
        let error = build_taira_privacy_bootstrap_v1()
            .expect_err("an incomplete Exact12 engine set must not emit activation templates");
        assert!(error.to_string().contains("is unavailable"), "{error:?}");
    }
    #[test]
    fn paired_writer_never_overwrites_and_cleans_first_file_on_second_open_failure() {
        let directory = tempfile::tempdir().expect("tempdir");
        let first = directory.path().join("instructions.json");
        let second = directory.path().join("report.json");
        fs::write(&second, b"occupied").expect("occupy report path");
        assert!(write_new_artifact_pair(&first, b"first", &second, b"second").is_err());
        assert!(
            !first.exists(),
            "partially created first artifact must be removed"
        );
        assert_eq!(fs::read(&second).expect("read occupied file"), b"occupied");
        fs::remove_file(&second).expect("remove occupied path");
        write_new_artifact_pair(&first, b"first", &second, b"second").expect("write fresh pair");
        assert!(write_new_artifact_pair(&first, b"x", &second, b"y").is_err());
        assert_eq!(fs::read(&first).expect("read first"), b"first");
        assert_eq!(fs::read(&second).expect("read second"), b"second");
    }
    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_symlinks_hardlinks_and_oversized_inputs() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("tempdir");
        let source = directory.path().join("source.json");
        let symlink_path = directory.path().join("symlink.json");
        let hardlink_path = directory.path().join("hardlink.json");
        fs::write(&source, b"{}\n").expect("write source");
        symlink(&source, &symlink_path).expect("create symlink");
        assert!(
            read_bounded(&symlink_path, 16, "symlinked test input")
                .expect_err("reject symlink")
                .to_string()
                .contains("non-symlink regular file")
        );
        fs::hard_link(&source, &hardlink_path).expect("create hardlink");
        assert!(
            read_bounded(&source, 16, "hardlinked test input")
                .expect_err("reject hardlink")
                .to_string()
                .contains("exactly one filesystem link")
        );
        fs::remove_file(&hardlink_path).expect("remove hardlink");
        assert!(
            read_bounded(&source, 2, "oversized test input")
                .expect_err("reject oversize")
                .to_string()
                .contains("fixed 2-byte limit")
        );
        assert_eq!(
            read_bounded(&source, 3, "bounded test input").expect("read exact bound"),
            b"{}\n"
        );
    }
    #[test]
    fn cleanup_never_removes_a_replacement_path() {
        let directory = tempfile::tempdir().expect("tempdir");
        let original = directory.path().join("artifact.json");
        let moved = directory.path().join("created-artifact.json");
        let created = create_new_file(&original, "test artifact").expect("create artifact");
        fs::rename(&original, &moved).expect("move opened artifact");
        fs::write(&original, b"replacement").expect("install replacement path");
        remove_created_file_if_unchanged_v1(&original, &created);
        assert_eq!(
            fs::read(&original).expect("read replacement"),
            b"replacement",
            "cleanup must never unlink a path that no longer names the created inode"
        );
        assert!(moved.exists());
    }
    #[cfg(unix)]
    #[test]
    fn paired_writer_rejects_symlinked_parent_aliases() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("tempdir");
        let real = directory.path().join("real");
        let alias = directory.path().join("alias");
        fs::create_dir(&real).expect("create real output directory");
        symlink(&real, &alias).expect("create directory alias");
        let first = real.join("artifact.json");
        let second = alias.join("artifact.json");
        assert!(write_new_artifact_pair(&first, b"a", &second, b"b").is_err());
        assert!(!first.exists());
    }
}
