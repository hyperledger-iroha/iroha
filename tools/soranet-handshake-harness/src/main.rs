use std::{env, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use soranet_handshake_harness::{
    CapabilitySummary, CapabilityTlv, HandshakeSuite, HarnessError, HexInput,
    SaltAnnouncementParams, SimulationParams, TelemetryReport, TranscriptInputs, decode_hex,
    decode_salt_hex, diff_capabilities, format_capabilities,
    generate_capability_fixtures as harness_generate, parse_capabilities, salt_announcement_json,
    simulate_handshake, simulation_report_json, soranet_telemetry_json,
    verify_fixtures as harness_verify, verify_salt_vector,
};
use soranet_pq::{
    MlKemSuite, SuiteParseError, validate_mlkem_ciphertext, validate_mlkem_public_key,
    validate_mlkem_secret_key,
};

/// Command-line interface for the (still evolving) SoraNet handshake harness.
#[derive(Parser, Debug)]
#[command(author, version, about = "SoraNet handshake harness", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect capability TLVs and compute the transcript hash.
    Inspect {
        /// Client capability vector (hex)
        #[arg(long)]
        client_hex: String,
        /// Relay capability vector (hex)
        #[arg(long)]
        relay_hex: String,
        /// Descriptor commitment (hex)
        #[arg(long)]
        descriptor_commit_hex: String,
        /// Client nonce (hex)
        #[arg(long)]
        client_nonce_hex: String,
        /// Relay nonce (hex)
        #[arg(long)]
        relay_nonce_hex: String,
        /// KEM identifier (decimal)
        #[arg(long)]
        kem_id: u8,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Signature identifier (decimal)
        #[arg(long)]
        sig_id: u8,
        /// Optional resume hash (hex)
        #[arg(long)]
        resume_hash_hex: Option<HexInput>,
    },
    /// Emit a JSON summary of a capability vector.
    Summary {
        /// Capability vector (hex)
        #[arg(long)]
        vector_hex: String,
    },
    /// Render a SaltAnnouncementV1 payload as JSON.
    Salt {
        #[arg(long)]
        epoch_id: u32,
        #[arg(long)]
        valid_after: String,
        #[arg(long)]
        valid_until: String,
        #[arg(long)]
        salt_hex: String,
        #[arg(long)]
        previous_epoch: Option<u32>,
        #[arg(long, default_value_t = false)]
        emergency: bool,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Verify a SaltAnnouncementV1 fixture on disk.
    SaltVerify {
        /// Path to the salt fixture (JSON).
        vector: PathBuf,
    },
    /// Render a SoraNetTelemetryV1 payload as JSON.
    Telemetry {
        #[arg(long)]
        epoch: u32,
        #[arg(long)]
        downgrade_attempts: u32,
        #[arg(long)]
        pq_disabled_sessions: u32,
        #[arg(long)]
        cover_ratio: f32,
        #[arg(long)]
        lagging_clients: u32,
        #[arg(long)]
        max_latency_ms: u32,
        #[arg(long)]
        incident_reference: Option<String>,
        #[arg(long)]
        signature: Option<String>,
        #[arg(long)]
        witness_signature: Option<String>,
        #[arg(long)]
        relay_static_sk_hex: Option<String>,
    },
    /// Generate or verify the reference handshake fixtures.
    Fixtures {
        /// Output directory for capability fixtures (defaults to fixtures/soranet_handshake/capabilities)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Verify existing fixtures instead of regenerating them.
        #[arg(long, default_value_t = false)]
        verify: bool,
    },
    /// Validate ML-KEM key material against the configured suite.
    KemValidate {
        /// ML-KEM identifier (decimal). Required unless `--kem-suite` is supplied.
        #[arg(long)]
        kem_id: Option<u8>,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Public key bytes to validate (hex).
        #[arg(long)]
        public_hex: Option<String>,
        /// Secret key bytes to validate (hex).
        #[arg(long)]
        secret_hex: Option<String>,
        /// Ciphertext bytes to validate (hex).
        #[arg(long)]
        ciphertext_hex: Option<String>,
    },
    /// Run the Noise XX handshake simulation pipeline (work in progress).
    Simulate {
        /// Client capability vector (hex)
        #[arg(long)]
        client_hex: String,
        /// Relay capability vector (hex)
        #[arg(long)]
        relay_hex: String,
        /// Client static secret key (hex)
        #[arg(long)]
        client_static_sk_hex: String,
        /// Relay static secret key (hex)
        #[arg(long)]
        relay_static_sk_hex: String,
        /// Optional resume hash (hex)
        #[arg(long)]
        resume_hash_hex: Option<HexInput>,
        /// Descriptor commitment (hex)
        #[arg(long)]
        descriptor_commit_hex: String,
        /// Client nonce (hex)
        #[arg(long)]
        client_nonce_hex: String,
        /// Relay nonce (hex)
        #[arg(long)]
        relay_nonce_hex: String,
        /// KEM identifier (decimal)
        #[arg(long)]
        kem_id: u8,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Signature identifier (decimal)
        #[arg(long)]
        sig_id: u8,
        /// Optional path to write a JSON report (use '-' for stdout)
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Optional directory to dump binary handshake frames.
        #[arg(long)]
        frames_out: Option<PathBuf>,
        /// Optional path to write the first telemetry payload JSON.
        #[arg(long)]
        telemetry_out: Option<PathBuf>,
        /// Print placeholder handshake steps to stdout.
        #[arg(long, default_value_t = false)]
        show_steps: bool,
        /// Only report warnings/capabilities for the specified types (hex like 0x0101 or decimal).
        #[arg(long = "only-capability")]
        only_capabilities: Vec<String>,
    },
}

fn main() -> Result<(), HarnessError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect {
            client_hex,
            relay_hex,
            descriptor_commit_hex,
            client_nonce_hex,
            relay_nonce_hex,
            kem_id,
            kem_suite,
            sig_id,
            resume_hash_hex,
        } => {
            let client_bytes = decode_hex(&client_hex)?;
            let relay_bytes = decode_hex(&relay_hex)?;
            let desc_commit = decode_hex(&descriptor_commit_hex)?;
            let client_nonce = decode_hex(&client_nonce_hex)?;
            let relay_nonce = decode_hex(&relay_nonce_hex)?;
            let resume_hash = resume_hash_hex.map(|h| h.0);
            let kem_id = resolve_kem_id(kem_id, kem_suite.as_deref())?;

            let client_caps = parse_capabilities(&client_bytes)?;
            let relay_caps = parse_capabilities(&relay_bytes)?;
            println!(
                "Client capabilities:\n{}",
                format_capabilities(&client_caps)
            );
            println!("Relay capabilities:\n{}", format_capabilities(&relay_caps));

            let handshake_suite = negotiate_handshake_suite(&client_caps, &relay_caps)?;
            let transcript = TranscriptInputs {
                descriptor_commit: &desc_commit,
                client_nonce: &client_nonce,
                relay_nonce: &relay_nonce,
                capability_bytes: &client_bytes, // include client TLVs per draft; harness will evolve
                kem_id,
                sig_id,
                handshake_suite,
                resume_hash: resume_hash.as_deref(),
            };
            let hash = transcript.compute_hash();
            println!("Transcript hash: 0x{}", hex::encode(hash));
            println!("Transcript handshake suite: {handshake_suite}");
            println!(
                "Selected ML-KEM suite: {}",
                mlkem_suite_from_id(kem_id)
                    .map(|suite| suite.to_string())
                    .unwrap_or_else(|| format!("unknown({kem_id})"))
            );

            let warnings = diff_capabilities(&client_caps, &relay_caps);
            if warnings.is_empty() {
                println!("All required capabilities satisfied.");
            } else {
                for warning in warnings {
                    println!("warning: {warning:?}");
                }
            }
        }
        Commands::Summary { vector_hex } => {
            let bytes = decode_hex(&vector_hex)?;
            let caps = parse_capabilities(&bytes)?;
            let summary = CapabilitySummary { tlvs: &caps };
            println!(
                "{}",
                summary.to_pretty_json().unwrap_or_else(|_| "{}".into())
            );
        }
        Commands::Salt {
            epoch_id,
            valid_after,
            valid_until,
            salt_hex,
            previous_epoch,
            emergency,
            notes,
        } => {
            let salt = decode_salt_hex(&salt_hex)?;
            let json = salt_announcement_json(&SaltAnnouncementParams {
                epoch_id,
                previous_epoch,
                valid_after: &valid_after,
                valid_until: &valid_until,
                blinded_cid_salt: &salt,
                emergency_rotation: emergency,
                notes: notes.as_deref(),
                signature: None,
            })?;
            println!("{json}");
        }
        Commands::SaltVerify { vector } => {
            let validation = verify_salt_vector(&vector)?;
            println!(
                "salt vector {} OK (epoch {}, signature: {})",
                vector.display(),
                validation.epoch_id,
                if validation.has_signature {
                    "present"
                } else {
                    "missing"
                }
            );
        }
        Commands::Telemetry {
            epoch,
            downgrade_attempts,
            pq_disabled_sessions,
            cover_ratio,
            lagging_clients,
            max_latency_ms,
            incident_reference,
            signature,
            witness_signature,
            relay_static_sk_hex,
        } => {
            let signing_key = if let Some(sk_hex) = relay_static_sk_hex.as_deref() {
                let bytes = decode_hex(sk_hex)?;
                if bytes.len() != 32 {
                    return Err(HarnessError::Validation(format!(
                        "relay-static-sk-hex must decode to 32 bytes, got {}",
                        bytes.len()
                    )));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Some(key)
            } else {
                None
            };
            let signature_ref = if signing_key.is_some() {
                None
            } else {
                signature.as_deref()
            };
            let witness_signature_ref = if signing_key.is_some() {
                None
            } else {
                witness_signature.as_deref()
            };
            let json = soranet_telemetry_json(
                &TelemetryReport {
                    epoch,
                    downgrade_attempts,
                    pq_disabled_sessions,
                    cover_ratio,
                    lagging_clients,
                    max_latency_ms,
                    incident_reference: incident_reference.as_deref(),
                    signature: signature_ref,
                    witness_signature: witness_signature_ref,
                },
                signing_key.as_ref(),
            )?;
            println!("{json}");
        }
        Commands::Fixtures { out, verify } => {
            let default = env::current_dir()?.join("fixtures/soranet_handshake/capabilities");
            let target = out.unwrap_or(default);
            if verify {
                harness_verify(&target)?;
            } else {
                harness_generate(&target)?;
            }
        }
        Commands::KemValidate {
            kem_id,
            kem_suite,
            public_hex,
            secret_hex,
            ciphertext_hex,
        } => {
            let (resolved_id, suite) = resolve_kem_suite(kem_id, kem_suite.as_deref())?;
            let public = decode_optional_hex("public key", public_hex)?;
            let secret = decode_optional_hex("secret key", secret_hex)?;
            let ciphertext = decode_optional_hex("ciphertext", ciphertext_hex)?;
            let results = run_kem_validation(
                suite,
                public.as_deref(),
                secret.as_deref(),
                ciphertext.as_deref(),
            )?;
            println!("ML-KEM suite {suite} (id {resolved_id}) validation succeeded.");
            for line in results {
                println!("  - {line}");
            }
        }
        Commands::Simulate {
            client_hex,
            relay_hex,
            client_static_sk_hex,
            relay_static_sk_hex,
            resume_hash_hex,
            descriptor_commit_hex,
            client_nonce_hex,
            relay_nonce_hex,
            kem_id,
            kem_suite,
            sig_id,
            json_out,
            frames_out,
            telemetry_out,
            show_steps,
            only_capabilities,
        } => {
            let client_caps = decode_hex(&client_hex)?;
            let relay_caps = decode_hex(&relay_hex)?;
            let client_sk = decode_hex(&client_static_sk_hex)?;
            let relay_sk = decode_hex(&relay_static_sk_hex)?;
            let resume_hash = resume_hash_hex.map(|h| h.0);
            let descriptor_commit = decode_hex(&descriptor_commit_hex)?;
            let client_nonce = decode_hex(&client_nonce_hex)?;
            let relay_nonce = decode_hex(&relay_nonce_hex)?;
            let kem_id = resolve_kem_id(kem_id, kem_suite.as_deref())?;
            let capability_filter = parse_capability_filters(&only_capabilities)?;
            let capability_filter_vec = capability_filter
                .as_ref()
                .map(|set| set.iter().copied().collect::<Vec<_>>());

            let result = simulate_handshake(&SimulationParams {
                client_capabilities: &client_caps,
                relay_capabilities: &relay_caps,
                client_static_sk: &client_sk,
                relay_static_sk: &relay_sk,
                resume_hash: resume_hash.as_deref(),
                descriptor_commit: &descriptor_commit,
                client_nonce: &client_nonce,
                relay_nonce: &relay_nonce,
                kem_id,
                sig_id,
            })?;

            println!("Transcript hash: 0x{}", hex::encode(result.transcript_hash));
            println!("Negotiated handshake suite: {}", result.handshake_suite);
            println!(
                "Configured ML-KEM suite: {}",
                mlkem_suite_from_id(kem_id)
                    .map(|suite| suite.to_string())
                    .unwrap_or_else(|| format!("unknown({kem_id})"))
            );
            let filtered_warnings: Vec<_> = result
                .warnings
                .iter()
                .filter(|warning| {
                    capability_filter
                        .as_ref()
                        .map(|set| set.contains(&warning.capability_type))
                        .unwrap_or(true)
                })
                .collect();
            if filtered_warnings.is_empty() {
                if capability_filter.is_some() && !result.warnings.is_empty() {
                    println!("No warnings matched the provided capability filter.");
                } else if result.warnings.is_empty() {
                    println!("No warnings generated during simulation.");
                }
            } else {
                for warning in filtered_warnings {
                    println!("warning: {}", warning.message);
                }
            }
            if result.telemetry_payloads.is_empty() {
                println!("No telemetry payloads generated.");
            } else {
                println!(
                    "Generated {} telemetry payload(s).",
                    result.telemetry_payloads.len()
                );
            }

            if show_steps {
                println!("Handshake steps:");
                for step in &result.handshake_steps {
                    println!(
                        "  - {role}::{action}: 0x{msg}",
                        role = step.role,
                        action = step.action,
                        msg = step.message_hex
                    );
                    println!("    note: {}", step.note);
                }
            }

            if let Some(dir) = frames_out {
                fs::create_dir_all(&dir)?;
                for step in &result.handshake_steps {
                    let filename = format!(
                        "{}_{}.bin",
                        step.role.to_lowercase(),
                        step.action.to_lowercase()
                    );
                    let bytes = hex::decode(&step.message_hex).map_err(|err| {
                        HarnessError::Validation(format!(
                            "failed to decode {}/{} frame: {err}",
                            step.role, step.action
                        ))
                    })?;
                    fs::write(dir.join(filename), bytes)?;
                }
                println!("wrote handshake frames to {}", dir.display());
            }

            if let Some(path) = json_out {
                let json = simulation_report_json(&result, capability_filter_vec.as_deref())?;
                if path == std::path::Path::new("-") {
                    println!("{json}");
                } else {
                    fs::write(&path, json + "\n")?;
                    println!("wrote {}", path.display());
                }
            }

            if let Some(path) = telemetry_out {
                if result.telemetry_payloads.is_empty() {
                    println!("No telemetry payloads available to write.");
                } else {
                    let mut payload = result.telemetry_payloads[0].clone();
                    if !payload.ends_with(b"\n") {
                        payload.push(b'\n');
                    }
                    fs::write(&path, payload)?;
                    println!("wrote telemetry {}", path.display());
                }
            }
        }
    }

    Ok(())
}

fn parse_capability_filters(
    values: &[String],
) -> Result<Option<std::collections::BTreeSet<u16>>, HarnessError> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut out = std::collections::BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        let ty = if let Some(rest) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            u16::from_str_radix(rest, 16)
        } else {
            trimmed.parse::<u16>()
        }
        .map_err(|_| HarnessError::CapabilityType(trimmed.to_string()))?;
        out.insert(ty);
    }
    Ok(Some(out))
}

const CAPABILITY_SUITE_LIST: u16 = 0x0104;

fn suite_list_from_caps(
    caps: &[CapabilityTlv],
) -> Result<Option<Vec<HandshakeSuite>>, HarnessError> {
    let cap = caps.iter().find(|cap| cap.ty == CAPABILITY_SUITE_LIST);
    let Some(cap) = cap else {
        return Ok(None);
    };
    if cap.value.is_empty() {
        return Err(HarnessError::Validation(
            "suite_list capability must contain at least one identifier".into(),
        ));
    }
    let mut suites = Vec::with_capacity(cap.value.len());
    for &raw in &cap.value {
        let suite = HandshakeSuite::try_from(raw)?;
        if !suites.contains(&suite) {
            suites.push(suite);
        }
    }
    Ok(Some(suites))
}

fn describe_suites(suites: &[HandshakeSuite]) -> String {
    suites
        .iter()
        .map(|suite| suite.label())
        .collect::<Vec<_>>()
        .join(", ")
}

fn negotiate_handshake_suite(
    client_caps: &[CapabilityTlv],
    relay_caps: &[CapabilityTlv],
) -> Result<HandshakeSuite, HarnessError> {
    let client_list = suite_list_from_caps(client_caps)?;
    let relay_list = suite_list_from_caps(relay_caps)?;
    match (client_list, relay_list) {
        (Some(client), Some(relay)) => {
            for suite in &client {
                if relay.contains(suite) {
                    return Ok(*suite);
                }
            }
            Err(HarnessError::Validation(format!(
                "no overlapping handshake suite between client ({}) and relay ({})",
                describe_suites(&client),
                describe_suites(&relay)
            )))
        }
        (Some(client), None) => Err(HarnessError::Validation(format!(
            "relay omitted suite_list capability; client advertised {}",
            describe_suites(&client)
        ))),
        (None, Some(relay)) => Err(HarnessError::Validation(format!(
            "client omitted suite_list capability; relay advertised {}",
            describe_suites(&relay)
        ))),
        (None, None) => Err(HarnessError::Validation(
            "suite_list capability is required for handshake negotiation".into(),
        )),
    }
}

fn resolve_kem_id(base: u8, suite_label: Option<&str>) -> Result<u8, HarnessError> {
    if let Some(label) = suite_label {
        let suite = parse_kem_suite(label)?;
        let derived = suite_to_kem_id(suite);
        if derived != base {
            return Err(HarnessError::Validation(format!(
                "--kem-suite {label} maps to id {derived}, but --kem-id {base} was supplied"
            )));
        }
        Ok(derived)
    } else {
        Ok(base)
    }
}

fn resolve_kem_suite(
    kem_id: Option<u8>,
    suite_label: Option<&str>,
) -> Result<(u8, MlKemSuite), HarnessError> {
    match (kem_id, suite_label) {
        (Some(id), Some(label)) => {
            let suite = parse_kem_suite(label)?;
            let derived = suite_to_kem_id(suite);
            if derived != id {
                return Err(HarnessError::Validation(format!(
                    "--kem-suite {label} maps to id {derived}, but --kem-id {id} was supplied"
                )));
            }
            Ok((derived, suite))
        }
        (Some(id), None) => {
            let suite = mlkem_suite_from_id(id).ok_or_else(|| {
                HarnessError::Validation(format!("unsupported ML-KEM identifier {id}"))
            })?;
            Ok((id, suite))
        }
        (None, Some(label)) => {
            let suite = parse_kem_suite(label)?;
            let id = suite_to_kem_id(suite);
            Ok((id, suite))
        }
        (None, None) => Err(HarnessError::Validation(
            "specify either --kem-id or --kem-suite".to_string(),
        )),
    }
}

fn parse_kem_suite(label: &str) -> Result<MlKemSuite, HarnessError> {
    label
        .parse::<MlKemSuite>()
        .map_err(|SuiteParseError(value)| {
            HarnessError::Validation(format!("unsupported ML-KEM suite '{value}'"))
        })
}

fn suite_to_kem_id(suite: MlKemSuite) -> u8 {
    match suite {
        MlKemSuite::MlKem512 => 0,
        MlKemSuite::MlKem768 => 1,
        MlKemSuite::MlKem1024 => 2,
    }
}

fn mlkem_suite_from_id(id: u8) -> Option<MlKemSuite> {
    match id {
        0 => Some(MlKemSuite::MlKem512),
        1 => Some(MlKemSuite::MlKem768),
        2 => Some(MlKemSuite::MlKem1024),
        _ => None,
    }
}

fn decode_optional_hex(
    label: &str,
    input: Option<String>,
) -> Result<Option<Vec<u8>>, HarnessError> {
    match input {
        Some(value) => match decode_hex(&value) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) => Err(HarnessError::Validation(format!(
                "failed to decode {label}: {err}"
            ))),
        },
        None => Ok(None),
    }
}

fn run_kem_validation(
    suite: MlKemSuite,
    public: Option<&[u8]>,
    secret: Option<&[u8]>,
    ciphertext: Option<&[u8]>,
) -> Result<Vec<String>, HarnessError> {
    if public.is_none() && secret.is_none() && ciphertext.is_none() {
        return Err(HarnessError::Validation(
            "provide at least one of --public-hex, --secret-hex, or --ciphertext-hex".into(),
        ));
    }

    let mut status = Vec::new();

    if let Some(bytes) = public {
        validate_mlkem_public_key(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("public key valid ({} bytes)", bytes.len()));
    }
    if let Some(bytes) = secret {
        validate_mlkem_secret_key(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("secret key valid ({} bytes)", bytes.len()));
    }
    if let Some(bytes) = ciphertext {
        validate_mlkem_ciphertext(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("ciphertext valid ({} bytes)", bytes.len()));
    }

    Ok(status)
}

#[cfg(test)]
mod tests {
    use soranet_pq::{encapsulate_mlkem, generate_mlkem_keypair};

    use super::*;

    #[test]
    fn run_kem_validation_accepts_valid_materials() {
        let suite = MlKemSuite::MlKem768;
        let keys = generate_mlkem_keypair(suite);
        let (_, ciphertext) = encapsulate_mlkem(suite, keys.public_key()).unwrap();
        let messages = run_kem_validation(
            suite,
            Some(keys.public_key()),
            Some(keys.secret_key()),
            Some(ciphertext.as_bytes()),
        )
        .expect("validation should pass");
        assert_eq!(messages.len(), 3);
        assert!(messages.iter().all(|msg| msg.contains("valid")));
    }

    #[test]
    fn run_kem_validation_rejects_invalid_public_key() {
        let err =
            run_kem_validation(MlKemSuite::MlKem512, Some(&[0u8; 8]), None, None).unwrap_err();
        assert!(matches!(err, HarnessError::Kem(_)));
    }

    #[test]
    fn run_kem_validation_requires_material() {
        let err = run_kem_validation(MlKemSuite::MlKem512, None, None, None).unwrap_err();
        assert!(matches!(err, HarnessError::Validation(message) if message.contains("provide")));
    }

    #[test]
    fn resolve_kem_suite_from_label_only() {
        let (id, suite) = resolve_kem_suite(None, Some("mlkem1024")).expect("suite should resolve");
        assert_eq!(id, 2);
        assert_eq!(suite, MlKemSuite::MlKem1024);
    }

    #[test]
    fn resolve_kem_suite_rejects_mismatch() {
        let err = resolve_kem_suite(Some(0), Some("mlkem768")).unwrap_err();
        assert!(matches!(err, HarnessError::Validation(message) if message.contains("maps to id")));
    }

    #[test]
    fn suite_list_from_caps_dedupes_entries() {
        let caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                u8::from(HandshakeSuite::Nk2Hybrid),
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
            ],
            required: false,
        }];
        let suites = suite_list_from_caps(&caps)
            .expect("suite list")
            .expect("suite list present");
        assert_eq!(
            suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
    }

    #[test]
    fn negotiate_handshake_suite_prefers_client_order() {
        let client_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                u8::from(HandshakeSuite::Nk2Hybrid),
            ],
            required: true,
        }];
        let relay_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
            ],
            required: true,
        }];
        let selected =
            negotiate_handshake_suite(&client_caps, &relay_caps).expect("suite negotiated");
        assert_eq!(selected, HandshakeSuite::Nk3PqForwardSecure);
    }
}
