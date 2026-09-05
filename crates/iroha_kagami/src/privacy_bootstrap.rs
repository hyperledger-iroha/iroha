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
    isi::{InstructionBox, privacy::RegisterPrivacyProtocolActivationV1},
    privacy::{
        PRIVACY_COMPILED_PROFILE_CATALOG_SCHEMA_NAME_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1, PrivacyCompiledProfileCatalogRowV1,
        PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileResultV1,
        PrivacyProposedLifecycleV1, PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1,
        PrivacyProtocolLifecycleV1,
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
const NOTICE_INTERVAL_BLOCKS_V1: u64 = 300;
const OBSERVATION_INTERVAL_BLOCKS_V1: u64 = 300;
const WAVE_PROPOSED_AT_HEIGHTS_V1: [u64; 4] = [1, 602, 1_203, 1_804];
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
    /// Emit all twelve compiled governance activation templates atomically.
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
                    "instruction_count": (PrivacyProtocolIdV1::COUNT as u64),
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
                    "instruction_count": (PrivacyProtocolIdV1::COUNT as u64),
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
const fn rollout_wave_index_v1(protocol: PrivacyProtocolIdV1) -> usize {
    match protocol {
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV1
        | PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        | PrivacyProtocolIdV1::VeRangeTransparentRangeV1
        | PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => 0,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1
        | PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        | PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        | PrivacyProtocolIdV1::PqMaspStarkV1 => 1,
        PrivacyProtocolIdV1::VegaExistingCredentialZkV1
        | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => 2,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 | PrivacyProtocolIdV1::IrohaZkAmsV1 => 3,
    }
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
    let mut instructions = Vec::with_capacity(PrivacyProtocolIdV1::COUNT);
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
        let wave = rollout_wave_index_v1(profile.protocol_id);
        let proposed_at_height = WAVE_PROPOSED_AT_HEIGHTS_V1[wave];
        let lifecycle = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height,
            activate_at_height: proposed_at_height + NOTICE_INTERVAL_BLOCKS_V1,
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
    let mut instructions_json = String::new();
    genesis_instructions_json::serialize(&instructions, &mut instructions_json);
    instructions_json.push('\n');
    let instructions_json = instructions_json.into_bytes();
    let mut labels = Vec::with_capacity(instructions.len());
    let mut instruction_norito_base64 = Vec::with_capacity(instructions.len());
    let mut instruction_norito_sha256 = Vec::with_capacity(instructions.len());
    for (index, instruction) in instructions.iter().enumerate() {
        let activation = privacy_activation_at_v1(instruction, index)?;
        let encoded = norito::to_bytes(instruction).wrap_err_with(|| {
            format!("failed to encode privacy bootstrap instruction {index} as Norito")
        })?;
        labels.push(activation.protocol_id.canonical_label().to_owned());
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
            "genesis_activation_forbidden": true,
            "notice_interval_blocks": (NOTICE_INTERVAL_BLOCKS_V1),
            "observation_interval_blocks": (OBSERVATION_INTERVAL_BLOCKS_V1),
            "instruction_wire_id": (RegisterPrivacyProtocolActivationV1::WIRE_ID),
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
    if instructions.len() != PrivacyProtocolIdV1::COUNT {
        bail!(
            "privacy bootstrap must contain exactly {} activation registrations, got {}",
            PrivacyProtocolIdV1::COUNT,
            instructions.len()
        );
    }
    if expected.len() != PrivacyProtocolIdV1::COUNT {
        bail!("internal exact-12 privacy bootstrap expectation is incomplete");
    }
    let mut seen = BTreeSet::new();
    for (index, ((instruction, expected_instruction), expected_protocol)) in instructions
        .iter()
        .zip(expected)
        .zip(PrivacyProtocolIdV1::ALL)
        .enumerate()
    {
        let actual = privacy_activation_at_v1(instruction, index)?;
        let expected_activation = privacy_activation_at_v1(expected_instruction, index)?;
        if !seen.insert(actual.protocol_id) {
            bail!(
                "privacy bootstrap contains duplicate protocol `{}` at index {index}",
                actual.protocol_id.canonical_label()
            );
        }
        if actual.protocol_id != expected_protocol {
            bail!(
                "privacy bootstrap protocol order mismatch at index {index}: expected `{}`, got `{}`",
                expected_protocol.canonical_label(),
                actual.protocol_id.canonical_label()
            );
        }
        actual.validate().map_err(|source| {
            eyre!(
                "privacy bootstrap activation `{}` is structurally invalid: {source}",
                actual.protocol_id.canonical_label()
            )
        })?;
        if actual != expected_activation {
            bail!(
                "privacy bootstrap activation `{}` differs from the exact local compiled profile",
                actual.protocol_id.canonical_label()
            );
        }
    }
    Ok(())
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
    let labels = report_string_array_v1(registration, "protocol_labels")?;
    let base64_values = report_string_array_v1(registration, "instruction_norito_base64")?;
    let hashes = report_string_array_v1(registration, "instruction_norito_sha256")?;
    for (field, len) in [
        ("protocol_labels", labels.len()),
        ("instruction_norito_base64", base64_values.len()),
        ("instruction_norito_sha256", hashes.len()),
    ] {
        if len != PrivacyProtocolIdV1::COUNT {
            bail!(
                "privacy bootstrap report `{field}` must contain exactly {} entries, got {len}",
                PrivacyProtocolIdV1::COUNT
            );
        }
    }
    for (index, (((label, encoded), claimed_hash), instruction)) in labels
        .iter()
        .zip(&base64_values)
        .zip(&hashes)
        .zip(instructions)
        .enumerate()
    {
        let activation = privacy_activation_at_v1(instruction, index)?;
        if *label != activation.protocol_id.canonical_label() {
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
        let decoded_activation = privacy_activation_at_v1(&decoded_instruction, index)?;
        if decoded_activation != activation {
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
