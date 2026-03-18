use std::{
    collections::HashSet,
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3Hasher;
use hex::encode as hex_encode;
use iroha_config::parameters::actual::SorafsRolloutPhase;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use norito::json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use time::{
    Duration as TimeDuration, OffsetDateTime,
    format_description::{FormatItem, well_known::Rfc3339},
};

pub struct PlanOptions {
    pub label: String,
    pub environment: String,
    pub phase: SorafsRolloutPhase,
    pub start: OffsetDateTime,
    pub regions: Vec<String>,
    pub window: TimeDuration,
    pub spacing: TimeDuration,
    pub client_offset: Option<TimeDuration>,
    pub output_json: PathBuf,
    pub output_markdown: Option<PathBuf>,
}

pub fn generate_plan(options: PlanOptions) -> Result<(), Box<dyn Error>> {
    if options.regions.is_empty() {
        return Err("at least one region must be supplied".into());
    }

    if options.window.is_negative() || options.window.is_zero() {
        return Err("window duration must be positive".into());
    }

    if options.spacing.is_negative() || options.spacing.is_zero() {
        return Err("region spacing duration must be positive".into());
    }

    if let Some(offset) = options.client_offset
        && offset.is_negative()
    {
        return Err("client offset must be positive".into());
    }

    let generated_at = OffsetDateTime::now_utc();

    let phase_label = options.phase.label().to_string();
    let effective_stage = options.phase.default_anonymity_policy().label().to_string();
    let window_label = format_duration(options.window);
    let spacing_label = format_duration(options.spacing);
    let client_offset_label = options.client_offset.map(format_duration);

    let mut events = Vec::new();
    let mut region_start = options.start;
    for region in &options.regions {
        let relay_start = region_start;
        let relay_end = relay_start + options.window;
        events.push(event_entry(
            "relay",
            region,
            phase_label.clone(),
            relay_start,
            relay_end,
        )?);

        if let Some(offset) = options.client_offset {
            let client_start = relay_start + offset;
            let client_end = client_start + options.window;
            events.push(event_entry(
                "client",
                region,
                phase_label.clone(),
                client_start,
                client_end,
            )?);
        }

        region_start += options.spacing;
    }

    fs::create_dir_all(
        options
            .output_json
            .parent()
            .unwrap_or_else(|| Path::new(".")),
    )
    .map_err(|err| {
        format!(
            "failed to create directory for {}: {err}",
            options.output_json.display()
        )
    })?;

    let mut metadata = JsonMap::new();
    metadata.insert("label".into(), JsonValue::String(options.label));
    metadata.insert("environment".into(), JsonValue::String(options.environment));
    metadata.insert("phase".into(), JsonValue::String(phase_label));
    metadata.insert(
        "effective_anonymity_stage".into(),
        JsonValue::String(effective_stage),
    );
    metadata.insert(
        "generated_at".into(),
        JsonValue::String(generated_at.format(&Rfc3339)?),
    );
    metadata.insert(
        "start".into(),
        JsonValue::String(options.start.format(&Rfc3339)?),
    );
    metadata.insert("window".into(), JsonValue::String(window_label.clone()));
    metadata.insert(
        "region_spacing".into(),
        JsonValue::String(spacing_label.clone()),
    );
    metadata.insert(
        "regions".into(),
        JsonValue::Array(
            options
                .regions
                .iter()
                .map(|r| JsonValue::String(r.clone()))
                .collect(),
        ),
    );
    if let Some(offset) = client_offset_label.as_ref() {
        metadata.insert("client_offset".into(), JsonValue::String(offset.clone()));
    }
    metadata.insert("events".into(), JsonValue::Array(events.clone()));

    let metadata_value = JsonValue::Object(metadata);
    let mut json_text = norito::json::to_string_pretty(&metadata_value)?;
    json_text.push('\n');
    fs::write(&options.output_json, json_text)?;

    if let Some(markdown_path) = options.output_markdown {
        fs::create_dir_all(markdown_path.parent().unwrap_or_else(|| Path::new("."))).map_err(
            |err| {
                format!(
                    "failed to create directory for {}: {err}",
                    markdown_path.display()
                )
            },
        )?;
        let mut markdown = String::new();
        markdown.push_str("# SoraNet PQ Rollout Schedule\n\n");
        markdown.push_str(&format!(
            "- **Label:** {}\n- **Environment:** {}\n- **Phase:** {}\n- **Effective Anonymity Stage:** {}\n- **Start:** {}\n- **Window:** {}\n- **Region Spacing:** {}\n",
            metadata_value["label"]
                .as_str()
                .unwrap_or(""),
            metadata_value["environment"]
                .as_str()
                .unwrap_or(""),
            metadata_value["phase"]
                .as_str()
                .unwrap_or(""),
            metadata_value["effective_anonymity_stage"]
                .as_str()
                .unwrap_or(""),
            metadata_value["start"]
                .as_str()
                .unwrap_or(""),
            window_label,
            spacing_label
        ));
        if let Some(offset) = client_offset_label.as_ref() {
            markdown.push_str(&format!("- **Client Offset:** {}\n", offset));
        }
        markdown.push('\n');
        markdown.push_str("| Wave | Region | Start | End |\n");
        markdown.push_str("|------|--------|-------|-----|\n");
        for event in events {
            let wave = event.get("wave").and_then(JsonValue::as_str).unwrap_or("-");
            let region = event
                .get("region")
                .and_then(JsonValue::as_str)
                .unwrap_or("-");
            let start = event
                .get("start")
                .and_then(JsonValue::as_str)
                .unwrap_or("-");
            let end = event.get("end").and_then(JsonValue::as_str).unwrap_or("-");
            markdown.push_str(&format!("| {wave} | {region} | {start} | {end} |\n"));
        }
        fs::write(markdown_path, markdown)?;
    }

    println!(
        "Generated SoraNet rollout plan at {}",
        options.output_json.display()
    );
    Ok(())
}

fn event_entry(
    wave: &str,
    region: &str,
    phase: String,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<JsonValue, Box<dyn Error>> {
    let mut map = JsonMap::new();
    map.insert("wave".into(), JsonValue::String(wave.to_string()));
    map.insert("region".into(), JsonValue::String(region.to_string()));
    map.insert("phase".into(), JsonValue::String(phase));
    map.insert("start".into(), JsonValue::String(start.format(&Rfc3339)?));
    map.insert("end".into(), JsonValue::String(end.format(&Rfc3339)?));
    Ok(JsonValue::Object(map))
}

fn format_duration(duration: TimeDuration) -> String {
    let seconds = duration.whole_seconds();
    if seconds % 3600 == 0 {
        format!("{}h", seconds / 3600)
    } else if seconds % 60 == 0 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

pub struct ArtifactInput {
    pub kind: String,
    pub path: PathBuf,
}

pub struct CaptureOptions {
    pub base_output_dir: PathBuf,
    pub label: Option<String>,
    pub environment: String,
    pub phase: SorafsRolloutPhase,
    pub log_path: PathBuf,
    pub additional_artifacts: Vec<ArtifactInput>,
    pub key_path: PathBuf,
    pub note: Option<String>,
}

pub fn capture_rollout(options: CaptureOptions) -> Result<(), Box<dyn Error>> {
    let generated_at = OffsetDateTime::now_utc();

    fs::create_dir_all(&options.base_output_dir).map_err(|err| {
        format!(
            "failed to create output directory {}: {err}",
            options.base_output_dir.display()
        )
    })?;

    let timestamp = generated_at.format(&compact_timestamp_format())?;
    let label_fragment = options
        .label
        .as_deref()
        .map(sanitize_label)
        .filter(|value| !value.is_empty());
    let dir_name = match label_fragment {
        Some(ref label) => format!("{timestamp}_{label}"),
        None => timestamp,
    };
    let run_dir = options.base_output_dir.join(dir_name);
    fs::create_dir_all(&run_dir).map_err(|err| {
        format!(
            "failed to create capture directory {}: {err}",
            run_dir.display()
        )
    })?;

    let mut artifacts = Vec::new();
    let mut used_names = HashSet::new();

    let log_record = ingest_artifact(&options.log_path, "rollout_log", &run_dir, &mut used_names)?;
    artifacts.push(log_record);

    for artifact in options.additional_artifacts {
        let record = ingest_artifact(&artifact.path, &artifact.kind, &run_dir, &mut used_names)?;
        artifacts.push(record);
    }

    let key_text = fs::read_to_string(&options.key_path).map_err(|err| {
        format!(
            "failed to read signing key {}: {err}",
            options.key_path.display()
        )
    })?;
    let private_key_hex = key_text.trim();
    if private_key_hex.is_empty() {
        return Err("signing key file is empty".into());
    }
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, private_key_hex).map_err(|err| {
        format!(
            "failed to parse signing key {}: {err}",
            options.key_path.display()
        )
    })?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|err| format!("invalid signing key: {err}"))?;

    let mut metadata = JsonMap::new();
    if let Some(label) = options.label {
        metadata.insert("label".into(), JsonValue::String(label));
    }
    metadata.insert("environment".into(), JsonValue::String(options.environment));
    metadata.insert(
        "phase".into(),
        JsonValue::String(options.phase.label().to_string()),
    );
    metadata.insert(
        "effective_anonymity_stage".into(),
        JsonValue::String(options.phase.default_anonymity_policy().label().to_string()),
    );
    metadata.insert(
        "generated_at".into(),
        JsonValue::String(generated_at.format(&Rfc3339)?),
    );
    metadata.insert(
        "artifact_root".into(),
        JsonValue::String(run_dir.display().to_string()),
    );
    if let Some(note) = options.note {
        metadata.insert("note".into(), JsonValue::String(note));
    }
    metadata.insert(
        "artifacts".into(),
        JsonValue::Array(artifacts.iter().map(artifact_to_value).collect()),
    );

    let metadata_value = JsonValue::Object(metadata);
    let payload = norito::json::to_vec(&metadata_value)?;
    let mut payload_hasher = Blake3Hasher::new();
    payload_hasher.update(&payload);
    let payload_digest = payload_hasher.finalize();
    let signature = Signature::new(key_pair.private_key(), &payload);
    let signature_bytes = signature.payload();
    let sig_algo = Algorithm::Ed25519;
    let (_, public_key_bytes) = key_pair.public_key().to_bytes();

    let mut signed_object = match metadata_value {
        JsonValue::Object(map) => map,
        _ => unreachable!("metadata_value is always an object"),
    };

    let mut signature_entry = JsonMap::new();
    signature_entry.insert("algorithm".into(), JsonValue::String(sig_algo.to_string()));
    signature_entry.insert(
        "public_key_hex".into(),
        JsonValue::String(hex_encode(public_key_bytes)),
    );
    signature_entry.insert(
        "signature_hex".into(),
        JsonValue::String(hex_encode(signature_bytes)),
    );
    signature_entry.insert(
        "payload_digest_blake3".into(),
        JsonValue::String(payload_digest.to_hex().to_string()),
    );
    signed_object.insert(
        "signatures".into(),
        JsonValue::Array(vec![JsonValue::Object(signature_entry)]),
    );

    let final_metadata = JsonValue::Object(signed_object);
    let mut json_text = norito::json::to_string_pretty(&final_metadata)?;
    json_text.push('\n');
    let metadata_path = run_dir.join("rollout_capture.json");
    fs::write(&metadata_path, json_text)?;

    println!("Captured rollout artefacts in {}", run_dir.display());
    Ok(())
}

struct ArtifactRecord {
    kind: String,
    file_name: String,
    source: String,
    bytes: u64,
    blake3_hex: String,
}

fn artifact_to_value(record: &ArtifactRecord) -> JsonValue {
    let mut map = JsonMap::new();
    map.insert("kind".into(), JsonValue::String(record.kind.clone()));
    map.insert("file".into(), JsonValue::String(record.file_name.clone()));
    map.insert("source".into(), JsonValue::String(record.source.clone()));
    map.insert(
        "bytes".into(),
        JsonValue::Number(JsonNumber::from(record.bytes)),
    );
    map.insert(
        "blake3".into(),
        JsonValue::String(record.blake3_hex.clone()),
    );
    JsonValue::Object(map)
}

fn ingest_artifact(
    source_path: &Path,
    kind: &str,
    run_dir: &Path,
    used_names: &mut HashSet<String>,
) -> Result<ArtifactRecord, Box<dyn Error>> {
    if !source_path.exists() {
        return Err(format!("artifact source {} does not exist", source_path.display()).into());
    }

    let file_name = unique_file_name(
        source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(kind),
        used_names,
    );
    let dest_path = run_dir.join(&file_name);

    let (hash, bytes) = copy_with_hash(source_path, &dest_path)?;

    Ok(ArtifactRecord {
        kind: kind.to_string(),
        file_name,
        source: source_path.display().to_string(),
        bytes,
        blake3_hex: hash,
    })
}

fn unique_file_name(base: &str, used: &mut HashSet<String>) -> String {
    let mut candidate = base.to_string();
    let mut counter = 1;
    while used.contains(&candidate) {
        candidate = format!("{base}.{counter}");
        counter += 1;
    }
    used.insert(candidate.clone());
    candidate
}

fn copy_with_hash(source: &Path, dest: &Path) -> Result<(String, u64), Box<dyn Error>> {
    fs::create_dir_all(dest.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|err| format!("failed to create directory for {}: {err}", dest.display()))?;
    let mut reader =
        File::open(source).map_err(|err| format!("failed to open {}: {err}", source.display()))?;
    let mut writer =
        File::create(dest).map_err(|err| format!("failed to create {}: {err}", dest.display()))?;
    let mut hasher = Blake3Hasher::new();
    let mut buffer = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    writer.flush()?;
    Ok((hasher.finalize().to_hex().to_string(), total))
}

fn sanitize_label(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            result.push('-');
            last_dash = true;
        }
    }
    while result.ends_with('-') {
        result.pop();
    }
    result
}

fn compact_timestamp_format() -> &'static [FormatItem<'static>] {
    static FORMAT: once_cell::sync::Lazy<Vec<FormatItem<'static>>> =
        once_cell::sync::Lazy::new(|| {
            time::format_description::parse("[year][month][day]T[hour][minute][second]Z")
                .expect("valid timestamp format")
        });
    &FORMAT
}

pub fn parse_duration_spec(spec: &str) -> Result<TimeDuration, Box<dyn Error>> {
    if spec.is_empty() {
        return Err("duration specifier must not be empty".into());
    }
    let (value_part, unit) = spec.split_at(spec.len() - 1);
    let value: i64 = value_part
        .parse()
        .map_err(|err| format!("invalid duration value `{value_part}`: {err}"))?;
    if value <= 0 {
        return Err("duration value must be positive".into());
    }
    match unit {
        "s" | "S" => Ok(TimeDuration::seconds(value)),
        "m" | "M" => Ok(TimeDuration::minutes(value)),
        "h" | "H" => Ok(TimeDuration::hours(value)),
        "d" | "D" => Ok(TimeDuration::days(value)),
        _ => Err(format!("unrecognised duration unit `{unit}`; expected s/m/h/d").into()),
    }
}
