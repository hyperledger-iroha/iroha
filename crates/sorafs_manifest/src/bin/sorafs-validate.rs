//! SoraFS reference validator CLI.
//!
//! This binary implements the first SF-11 validator slice without adding a new
//! workspace crate. It validates Norito-encoded provider adverts and
//! replication orders, then emits stable `ValidationOutcomeV1`
//! JSON/table/YAML output.

use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signer, SigningKey};
use norito::json;
use sorafs_manifest::{
    AdvertSignature, FixtureBundlePayloadKindV1, FixtureBundlePayloadV1, GovernanceLogNodeV1,
    GovernanceLogSignatureV1, GovernanceSignatureAlgorithm, OrderbookValidationPayloadKindV1,
    ProofStreamTier, ProviderAdvertV1, RepairValidationPayloadKindV1, ReplicationOrderSignatureV1,
    ReplicationOrderV1, SIGNED_REPLICATION_ORDER_VERSION_V1, SignatureAlgorithm,
    SignedReplicationOrderV1, ValidationContextFieldV1, ValidationInputV1, ValidationOutcomeV1,
    validate_fixture_bundle_payloads, validate_governance_dag_block_bytes,
    validate_governance_dag_head_chain_bytes, validate_governance_log_node_bytes,
    validate_orderbook_payload_bytes, validate_pdp_challenge_bytes,
    validate_pdp_challenge_proof_bytes, validate_pdp_commitment_bytes,
    validate_pdp_commitment_challenge_bytes, validate_pdp_commitment_challenge_proof_bytes,
    validate_pdp_proof_bytes, validate_por_challenge_proof_bytes, validate_potr_receipt_bytes,
    validate_provider_admission_envelope_bytes, validate_provider_admission_renewal_bytes,
    validate_provider_admission_revocation_bytes, validate_provider_advert_bytes,
    validate_repair_payload_bytes, validate_replication_order_bytes,
    validate_signed_replication_order_bytes,
};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<ExitCode, CliError> {
    let args: Vec<String> = args.into_iter().collect();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return Ok(ExitCode::SUCCESS);
    }

    let command = args
        .first()
        .ok_or(CliError::Config("missing command".to_owned()))?;
    match command.as_str() {
        "advert" => run_advert(AdvertArgs::parse(&args[1..])?),
        "admission" => run_admission(AdmissionArgs::parse(&args[1..])?),
        "order" => run_order(OrderArgs::parse(&args[1..])?),
        "orderbook" => run_orderbook(OrderbookArgs::parse(&args[1..])?),
        "pdp" => run_pdp(PdpArgs::parse(&args[1..])?),
        "por" => run_por(PorArgs::parse(&args[1..])?),
        "potr" => run_potr(PotrArgs::parse(&args[1..])?),
        "repair" => run_repair(RepairArgs::parse(&args[1..])?),
        "bundle" => run_bundle(BundleArgs::parse(&args[1..])?),
        "governance" => run_governance(GovernanceArgs::parse(&args[1..])?),
        "sign" => run_sign(SignArgs::parse(&args[1..])?),
        other => Err(CliError::Config(format!(
            "unsupported sorafs-validate command `{other}`; implemented commands: advert, admission, order, orderbook, pdp, por, potr, repair, bundle, governance, sign"
        ))),
    }
}

fn run_advert(args: AdvertArgs) -> Result<ExitCode, CliError> {
    let input = args.input.ok_or(CliError::Config(
        "advert requires --input <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let now = match args.now {
        Some(now) => now,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let outcome =
        validate_provider_advert_bytes(&bytes, input.display().to_string(), now, generated_at);
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_admission(args: AdmissionArgs) -> Result<ExitCode, CliError> {
    let input = args.input.ok_or(CliError::Config(
        "admission requires --input <path> or --envelope <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let outcome = if let Some(renewal) = args.renewal {
        let renewal_bytes = fs::read(&renewal)
            .map_err(|err| CliError::Io(format!("failed to read {}: {err}", renewal.display())))?;
        validate_provider_admission_renewal_bytes(
            &bytes,
            &renewal_bytes,
            input.display().to_string(),
            renewal.display().to_string(),
            generated_at,
        )
    } else if let Some(revocation) = args.revocation {
        let revocation_bytes = fs::read(&revocation).map_err(|err| {
            CliError::Io(format!("failed to read {}: {err}", revocation.display()))
        })?;
        validate_provider_admission_revocation_bytes(
            &bytes,
            &revocation_bytes,
            input.display().to_string(),
            revocation.display().to_string(),
            generated_at,
        )
    } else {
        validate_provider_admission_envelope_bytes(
            &bytes,
            input.display().to_string(),
            generated_at,
        )
    };
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_order(args: OrderArgs) -> Result<ExitCode, CliError> {
    let input = args.order.ok_or(CliError::Config(
        "order requires --order <path> or --signed-order <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let outcome = if args.signed {
        validate_signed_replication_order_bytes(&bytes, input.display().to_string(), generated_at)
    } else {
        validate_replication_order_bytes(&bytes, input.display().to_string(), generated_at)
    };
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_orderbook(args: OrderbookArgs) -> Result<ExitCode, CliError> {
    let input = args.input.ok_or(CliError::Config(
        "orderbook requires --input <path> or a payload alias such as --receipt <path>".to_owned(),
    ))?;
    let kind = args.kind.ok_or(CliError::Config(
        "orderbook requires --kind <payload-kind> unless a payload alias such as --receipt is used"
            .to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let outcome =
        validate_orderbook_payload_bytes(kind, &bytes, input.display().to_string(), generated_at);
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_pdp(args: PdpArgs) -> Result<ExitCode, CliError> {
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let outcome = match (&args.commitment, &args.challenge, &args.proof) {
        (None, None, None) => {
            return Err(CliError::Config(
                "pdp requires at least one of --commitment, --challenge, or --proof".to_owned(),
            ));
        }
        (Some(_commitment), None, Some(_)) => {
            return Err(CliError::Config(
                "pdp requires --challenge when validating --commitment with --proof".to_owned(),
            ));
        }
        (Some(commitment), Some(challenge), Some(proof)) => {
            let commitment_bytes = read_cli_bytes(commitment)?;
            let challenge_bytes = read_cli_bytes(challenge)?;
            let proof_bytes = read_cli_bytes(proof)?;
            validate_pdp_commitment_challenge_proof_bytes(
                &commitment_bytes,
                &challenge_bytes,
                &proof_bytes,
                commitment.display().to_string(),
                challenge.display().to_string(),
                proof.display().to_string(),
                generated_at,
            )
        }
        (Some(commitment), Some(challenge), None) => {
            let commitment_bytes = read_cli_bytes(commitment)?;
            let challenge_bytes = read_cli_bytes(challenge)?;
            validate_pdp_commitment_challenge_bytes(
                &commitment_bytes,
                &challenge_bytes,
                commitment.display().to_string(),
                challenge.display().to_string(),
                generated_at,
            )
        }
        (None, Some(challenge), Some(proof)) => {
            let challenge_bytes = read_cli_bytes(challenge)?;
            let proof_bytes = read_cli_bytes(proof)?;
            validate_pdp_challenge_proof_bytes(
                &challenge_bytes,
                &proof_bytes,
                challenge.display().to_string(),
                proof.display().to_string(),
                generated_at,
            )
        }
        (Some(commitment), None, None) => {
            let commitment_bytes = read_cli_bytes(commitment)?;
            validate_pdp_commitment_bytes(
                &commitment_bytes,
                commitment.display().to_string(),
                generated_at,
            )
        }
        (None, Some(challenge), None) => {
            let challenge_bytes = read_cli_bytes(challenge)?;
            validate_pdp_challenge_bytes(
                &challenge_bytes,
                challenge.display().to_string(),
                generated_at,
            )
        }
        (None, None, Some(proof)) => {
            let proof_bytes = read_cli_bytes(proof)?;
            validate_pdp_proof_bytes(&proof_bytes, proof.display().to_string(), generated_at)
        }
    };

    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn read_cli_bytes(path: &Path) -> Result<Vec<u8>, CliError> {
    fs::read(path).map_err(|err| CliError::Io(format!("failed to read {}: {err}", path.display())))
}

fn run_por(args: PorArgs) -> Result<ExitCode, CliError> {
    let challenge = args.challenge.ok_or(CliError::Config(
        "por requires --challenge <path>".to_owned(),
    ))?;
    let proof = args
        .proof
        .ok_or(CliError::Config("por requires --proof <path>".to_owned()))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let challenge_bytes = fs::read(&challenge)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", challenge.display())))?;
    let proof_bytes = fs::read(&proof)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", proof.display())))?;
    let outcome = validate_por_challenge_proof_bytes(
        &challenge_bytes,
        &proof_bytes,
        challenge.display().to_string(),
        proof.display().to_string(),
        generated_at,
    );
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_potr(args: PotrArgs) -> Result<ExitCode, CliError> {
    let receipt = args.receipt.ok_or(CliError::Config(
        "potr requires --receipt <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&receipt)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", receipt.display())))?;
    let outcome = validate_potr_receipt_bytes(
        &bytes,
        receipt.display().to_string(),
        args.profile,
        generated_at,
    );
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_repair(args: RepairArgs) -> Result<ExitCode, CliError> {
    let input = args.input.ok_or(CliError::Config(
        "repair requires --input <path> or a payload alias such as --task <path>".to_owned(),
    ))?;
    let kind = args.kind.ok_or(CliError::Config(
        "repair requires --kind <payload-kind> unless a payload alias such as --task is used"
            .to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let outcome =
        validate_repair_payload_bytes(kind, &bytes, input.display().to_string(), generated_at);
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_bundle(args: BundleArgs) -> Result<ExitCode, CliError> {
    let bundle = args.bundle.ok_or(CliError::Config(
        "bundle requires --bundle <directory>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let now = match args.now {
        Some(now) => now,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let payloads = read_bundle_payloads(&bundle)?;
    let borrowed = payloads
        .iter()
        .map(|payload| {
            FixtureBundlePayloadV1::new(
                payload.kind,
                payload.label.clone(),
                payload.bytes.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    let outcome = validate_fixture_bundle_payloads(&borrowed, now, generated_at);
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_governance(args: GovernanceArgs) -> Result<ExitCode, CliError> {
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let modes = usize::from(args.node.is_some())
        + usize::from(args.block.is_some())
        + usize::from(args.head.is_some());
    if modes != 1 {
        return Err(CliError::Config(
            "governance requires exactly one of --node <path>, --block <path>, or --head <path>"
                .to_owned(),
        ));
    }

    let outcome = if let Some(node) = args.node {
        if !args.blocks.is_empty() {
            return Err(CliError::Config(
                "governance --node does not accept additional --block inputs".to_owned(),
            ));
        }
        let bytes = fs::read(&node)
            .map_err(|err| CliError::Io(format!("failed to read {}: {err}", node.display())))?;
        validate_governance_log_node_bytes(
            &bytes,
            node.display().to_string(),
            args.cid.as_deref().map(str::as_bytes),
            generated_at,
        )
    } else if let Some(block) = args.block {
        if !args.blocks.is_empty() {
            return Err(CliError::Config(
                "governance --block validates one block; use --head with repeated --block inputs for chain validation"
                    .to_owned(),
            ));
        }
        let bytes = fs::read(&block)
            .map_err(|err| CliError::Io(format!("failed to read {}: {err}", block.display())))?;
        let expected_cid = args.cid.as_deref().map(parse_cid_arg_bytes).transpose()?;
        validate_governance_dag_block_bytes(
            &bytes,
            block.display().to_string(),
            expected_cid.as_deref(),
            generated_at,
        )
    } else if let Some(head) = args.head {
        if args.blocks.is_empty() {
            return Err(CliError::Config(
                "governance --head requires at least one --block <path>".to_owned(),
            ));
        }
        if args.cid.is_some() {
            return Err(CliError::Config(
                "governance --head does not accept --cid; the signed head carries the expected block CID"
                    .to_owned(),
            ));
        }
        let head_bytes = fs::read(&head)
            .map_err(|err| CliError::Io(format!("failed to read {}: {err}", head.display())))?;
        let mut block_payloads = Vec::with_capacity(args.blocks.len());
        for block in &args.blocks {
            let bytes = fs::read(block).map_err(|err| {
                CliError::Io(format!("failed to read {}: {err}", block.display()))
            })?;
            block_payloads.push((bytes, block.display().to_string()));
        }
        let refs: Vec<(&[u8], String)> = block_payloads
            .iter()
            .map(|(bytes, label)| (bytes.as_slice(), label.clone()))
            .collect();
        validate_governance_dag_head_chain_bytes(
            &head_bytes,
            head.display().to_string(),
            &refs,
            generated_at,
        )
    } else {
        unreachable!("governance mode count checked above")
    };
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_sign(args: SignArgs) -> Result<ExitCode, CliError> {
    match args.kind {
        Some(SignKind::Advert) => run_sign_advert(args),
        Some(SignKind::Order) => run_sign_order(args),
        Some(SignKind::Governance) => run_sign_governance(args),
        None => Err(CliError::Config(
            "sign requires --kind advert, --kind order, or --kind governance".to_owned(),
        )),
    }
}

fn run_sign_advert(args: SignArgs) -> Result<ExitCode, CliError> {
    let input = args.input.clone().ok_or(CliError::Config(
        "sign --kind advert requires --input <path>".to_owned(),
    ))?;
    let output = args.out.clone().ok_or(CliError::Config(
        "sign --kind advert requires --out <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let now = match args.now {
        Some(now) => now,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let seed = read_signing_seed(&args)?;
    let input_bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let mut advert = match norito::decode_from_bytes::<ProviderAdvertV1>(&input_bytes) {
        Ok(advert) => advert,
        Err(_) => {
            let outcome = validate_provider_advert_bytes(
                &input_bytes,
                input.display().to_string(),
                now,
                generated_at,
            );
            if let Some(path) = args.telemetry_out {
                write_json_outcome(&path, &outcome)?;
            }
            print_outcome(&outcome, format)?;
            return Ok(ExitCode::from(2));
        }
    };

    sign_provider_advert(&mut advert, &seed)?;
    let signed_bytes = norito::to_bytes(&advert).map_err(|err| {
        CliError::Internal(format!("failed to encode signed provider advert: {err}"))
    })?;
    let mut outcome = validate_provider_advert_bytes(
        &signed_bytes,
        output.display().to_string(),
        now,
        generated_at,
    );
    outcome
        .telemetry_tags
        .push("sorafs.reference.sign.advert".to_owned());
    outcome
        .context
        .push(ValidationContextFieldV1::new("operation", "sign"));
    outcome.context.push(ValidationContextFieldV1::new(
        "public_key_hex",
        hex::encode(&advert.signature.public_key),
    ));
    outcome.inputs.push(ValidationInputV1::new(
        "signed_provider_advert",
        output.display().to_string(),
    ));

    if outcome.is_ok() {
        fs::write(&output, signed_bytes)
            .map_err(|err| CliError::Io(format!("failed to write {}: {err}", output.display())))?;
    }
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_sign_order(args: SignArgs) -> Result<ExitCode, CliError> {
    let input = args.input.clone().ok_or(CliError::Config(
        "sign --kind order requires --input <path>".to_owned(),
    ))?;
    let output = args.out.clone().ok_or(CliError::Config(
        "sign --kind order requires --out <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let seed = read_signing_seed(&args)?;
    let input_bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let order = match norito::decode_from_bytes::<ReplicationOrderV1>(&input_bytes) {
        Ok(order) => order,
        Err(_) => {
            let outcome = validate_replication_order_bytes(
                &input_bytes,
                input.display().to_string(),
                generated_at,
            );
            if let Some(path) = args.telemetry_out {
                write_json_outcome(&path, &outcome)?;
            }
            print_outcome(&outcome, format)?;
            return Ok(ExitCode::from(2));
        }
    };

    let signed_order = sign_replication_order(order, &seed)?;
    let signed_bytes = norito::to_bytes(&signed_order).map_err(|err| {
        CliError::Internal(format!("failed to encode signed replication order: {err}"))
    })?;
    let mut outcome = validate_signed_replication_order_bytes(
        &signed_bytes,
        output.display().to_string(),
        generated_at,
    );
    outcome
        .telemetry_tags
        .push("sorafs.reference.sign.order".to_owned());
    outcome
        .context
        .push(ValidationContextFieldV1::new("operation", "sign"));
    outcome.context.push(ValidationContextFieldV1::new(
        "public_key_hex",
        hex::encode(&signed_order.signature.public_key),
    ));
    outcome.inputs.push(ValidationInputV1::new(
        "signed_replication_order",
        output.display().to_string(),
    ));

    if outcome.is_ok() {
        fs::write(&output, signed_bytes)
            .map_err(|err| CliError::Io(format!("failed to write {}: {err}", output.display())))?;
    }
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn run_sign_governance(args: SignArgs) -> Result<ExitCode, CliError> {
    let input = args.input.clone().ok_or(CliError::Config(
        "sign --kind governance requires --input <path>".to_owned(),
    ))?;
    let output = args.out.clone().ok_or(CliError::Config(
        "sign --kind governance requires --out <path>".to_owned(),
    ))?;
    let format = args.format.unwrap_or(OutputFormat::Table);
    let generated_at = match args.generated_at {
        Some(generated_at) => generated_at,
        None => unix_time_now()
            .ok_or_else(|| CliError::Internal("system time is before the UNIX epoch".to_owned()))?,
    };

    let seed = read_signing_seed(&args)?;
    let input_bytes = fs::read(&input)
        .map_err(|err| CliError::Io(format!("failed to read {}: {err}", input.display())))?;
    let mut node = match norito::decode_from_bytes::<GovernanceLogNodeV1>(&input_bytes) {
        Ok(node) => node,
        Err(_) => {
            let outcome = validate_governance_log_node_bytes(
                &input_bytes,
                input.display().to_string(),
                None,
                generated_at,
            );
            if let Some(path) = args.telemetry_out {
                write_json_outcome(&path, &outcome)?;
            }
            print_outcome(&outcome, format)?;
            return Ok(ExitCode::from(2));
        }
    };

    sign_governance_log_node(&mut node, &seed)?;
    let signed_bytes = norito::to_bytes(&node).map_err(|err| {
        CliError::Internal(format!(
            "failed to encode signed governance log node: {err}"
        ))
    })?;
    let mut outcome = validate_governance_log_node_bytes(
        &signed_bytes,
        output.display().to_string(),
        None,
        generated_at,
    );
    outcome
        .telemetry_tags
        .push("sorafs.reference.sign.governance".to_owned());
    outcome
        .context
        .push(ValidationContextFieldV1::new("operation", "sign"));
    outcome.context.push(ValidationContextFieldV1::new(
        "public_key_hex",
        hex::encode(&node.publisher_signature.public_key),
    ));
    outcome.inputs.push(ValidationInputV1::new(
        "signed_governance_log_node",
        output.display().to_string(),
    ));

    if outcome.is_ok() {
        fs::write(&output, signed_bytes)
            .map_err(|err| CliError::Io(format!("failed to write {}: {err}", output.display())))?;
    }
    if let Some(path) = args.telemetry_out {
        write_json_outcome(&path, &outcome)?;
    }
    print_outcome(&outcome, format)?;

    if outcome.is_ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

#[derive(Debug, Default)]
struct AdvertArgs {
    input: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    now: Option<u64>,
    generated_at: Option<u64>,
}

impl AdvertArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = AdvertArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--input=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--now=") {
                parsed.now = Some(parse_u64_flag(value, "--now")?);
            } else if arg == "--now" {
                index += 1;
                parsed.now = Some(parse_u64_flag(
                    require_value(args, index, "--now")?,
                    "--now",
                )?);
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown advert option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct AdmissionArgs {
    input: Option<PathBuf>,
    renewal: Option<PathBuf>,
    revocation: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl AdmissionArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = AdmissionArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--input=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--envelope=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--envelope" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--envelope")?));
            } else if let Some(value) = arg.strip_prefix("--renewal=") {
                parsed.renewal = Some(PathBuf::from(value));
            } else if arg == "--renewal" {
                index += 1;
                parsed.renewal = Some(PathBuf::from(require_value(args, index, "--renewal")?));
            } else if let Some(value) = arg.strip_prefix("--revocation=") {
                parsed.revocation = Some(PathBuf::from(value));
            } else if arg == "--revocation" {
                index += 1;
                parsed.revocation =
                    Some(PathBuf::from(require_value(args, index, "--revocation")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown admission option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        if parsed.renewal.is_some() && parsed.revocation.is_some() {
            return Err(CliError::Config(
                "admission accepts either --renewal or --revocation, not both".to_owned(),
            ));
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct PdpArgs {
    commitment: Option<PathBuf>,
    challenge: Option<PathBuf>,
    proof: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl PdpArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = PdpArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--commitment=") {
                parsed.commitment = Some(PathBuf::from(value));
            } else if arg == "--commitment" {
                index += 1;
                parsed.commitment =
                    Some(PathBuf::from(require_value(args, index, "--commitment")?));
            } else if let Some(value) = arg.strip_prefix("--challenge=") {
                parsed.challenge = Some(PathBuf::from(value));
            } else if arg == "--challenge" {
                index += 1;
                parsed.challenge = Some(PathBuf::from(require_value(args, index, "--challenge")?));
            } else if let Some(value) = arg.strip_prefix("--proof=") {
                parsed.proof = Some(PathBuf::from(value));
            } else if arg == "--proof" {
                index += 1;
                parsed.proof = Some(PathBuf::from(require_value(args, index, "--proof")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown pdp option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct PorArgs {
    challenge: Option<PathBuf>,
    proof: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl PorArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = PorArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--challenge=") {
                parsed.challenge = Some(PathBuf::from(value));
            } else if arg == "--challenge" {
                index += 1;
                parsed.challenge = Some(PathBuf::from(require_value(args, index, "--challenge")?));
            } else if let Some(value) = arg.strip_prefix("--proof=") {
                parsed.proof = Some(PathBuf::from(value));
            } else if arg == "--proof" {
                index += 1;
                parsed.proof = Some(PathBuf::from(require_value(args, index, "--proof")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown por option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct PotrArgs {
    receipt: Option<PathBuf>,
    profile: Option<ProofStreamTier>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

#[derive(Debug, Default)]
struct RepairArgs {
    input: Option<PathBuf>,
    kind: Option<RepairValidationPayloadKindV1>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl RepairArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = RepairArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--input=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--kind=") {
                parsed.kind = Some(parse_repair_kind(value)?);
            } else if arg == "--kind" {
                index += 1;
                parsed.kind = Some(parse_repair_kind(require_value(args, index, "--kind")?)?);
            } else if let Some(value) = arg.strip_prefix("--task=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::TaskRecord,
                    PathBuf::from(value),
                    "--task",
                )?;
            } else if arg == "--task" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::TaskRecord,
                    PathBuf::from(require_value(args, index, "--task")?),
                    "--task",
                )?;
            } else if let Some(value) = arg.strip_prefix("--evidence=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::Evidence,
                    PathBuf::from(value),
                    "--evidence",
                )?;
            } else if arg == "--evidence" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::Evidence,
                    PathBuf::from(require_value(args, index, "--evidence")?),
                    "--evidence",
                )?;
            } else if let Some(value) = arg.strip_prefix("--report=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::Report,
                    PathBuf::from(value),
                    "--report",
                )?;
            } else if arg == "--report" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::Report,
                    PathBuf::from(require_value(args, index, "--report")?),
                    "--report",
                )?;
            } else if let Some(value) = arg.strip_prefix("--slash-proposal=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::SlashProposal,
                    PathBuf::from(value),
                    "--slash-proposal",
                )?;
            } else if arg == "--slash-proposal" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::SlashProposal,
                    PathBuf::from(require_value(args, index, "--slash-proposal")?),
                    "--slash-proposal",
                )?;
            } else if let Some(value) = arg.strip_prefix("--policy=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::EscalationPolicy,
                    PathBuf::from(value),
                    "--policy",
                )?;
            } else if arg == "--policy" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::EscalationPolicy,
                    PathBuf::from(require_value(args, index, "--policy")?),
                    "--policy",
                )?;
            } else if let Some(value) = arg.strip_prefix("--approval=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::EscalationApproval,
                    PathBuf::from(value),
                    "--approval",
                )?;
            } else if arg == "--approval" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::EscalationApproval,
                    PathBuf::from(require_value(args, index, "--approval")?),
                    "--approval",
                )?;
            } else if let Some(value) = arg.strip_prefix("--signed-auditor-request=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::SignedAuditorRequest,
                    PathBuf::from(value),
                    "--signed-auditor-request",
                )?;
            } else if arg == "--signed-auditor-request" || arg == "--signed-auditor" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::SignedAuditorRequest,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--worker-signature=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::WorkerSignaturePayload,
                    PathBuf::from(value),
                    "--worker-signature",
                )?;
            } else if arg == "--worker-signature" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::WorkerSignaturePayload,
                    PathBuf::from(require_value(args, index, "--worker-signature")?),
                    "--worker-signature",
                )?;
            } else if let Some(value) = arg.strip_prefix("--event=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::TaskEvent,
                    PathBuf::from(value),
                    "--event",
                )?;
            } else if arg == "--event" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::TaskEvent,
                    PathBuf::from(require_value(args, index, "--event")?),
                    "--event",
                )?;
            } else if let Some(value) = arg.strip_prefix("--audit-event=") {
                parsed.set_payload(
                    RepairValidationPayloadKindV1::AuditEvent,
                    PathBuf::from(value),
                    "--audit-event",
                )?;
            } else if arg == "--audit-event" {
                index += 1;
                parsed.set_payload(
                    RepairValidationPayloadKindV1::AuditEvent,
                    PathBuf::from(require_value(args, index, "--audit-event")?),
                    "--audit-event",
                )?;
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown repair option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn set_payload(
        &mut self,
        kind: RepairValidationPayloadKindV1,
        path: PathBuf,
        flag: &str,
    ) -> Result<(), CliError> {
        if self.input.is_some() || self.kind.is_some() {
            return Err(CliError::Config(format!(
                "repair accepts one payload input; `{flag}` conflicts with an earlier repair input"
            )));
        }
        self.input = Some(path);
        self.kind = Some(kind);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct OrderbookArgs {
    input: Option<PathBuf>,
    kind: Option<OrderbookValidationPayloadKindV1>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl OrderbookArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = OrderbookArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--input=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--kind=") {
                parsed.kind = Some(parse_orderbook_kind(value)?);
            } else if arg == "--kind" {
                index += 1;
                parsed.kind = Some(parse_orderbook_kind(require_value(args, index, "--kind")?)?);
            } else if let Some(value) = arg.strip_prefix("--order=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderRequest,
                    PathBuf::from(value),
                    "--order",
                )?;
            } else if let Some(value) = arg.strip_prefix("--order-request=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderRequest,
                    PathBuf::from(value),
                    "--order-request",
                )?;
            } else if arg == "--order" || arg == "--order-request" {
                index += 1;
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderRequest,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--cancel=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderCancel,
                    PathBuf::from(value),
                    "--cancel",
                )?;
            } else if let Some(value) = arg.strip_prefix("--order-cancel=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderCancel,
                    PathBuf::from(value),
                    "--order-cancel",
                )?;
            } else if arg == "--cancel" || arg == "--order-cancel" {
                index += 1;
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::OrderCancel,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--trade=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::TradeEvent,
                    PathBuf::from(value),
                    "--trade",
                )?;
            } else if let Some(value) = arg.strip_prefix("--trade-event=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::TradeEvent,
                    PathBuf::from(value),
                    "--trade-event",
                )?;
            } else if arg == "--trade" || arg == "--trade-event" {
                index += 1;
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::TradeEvent,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--channel=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementChannel,
                    PathBuf::from(value),
                    "--channel",
                )?;
            } else if let Some(value) = arg.strip_prefix("--settlement-channel=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementChannel,
                    PathBuf::from(value),
                    "--settlement-channel",
                )?;
            } else if arg == "--channel" || arg == "--settlement-channel" {
                index += 1;
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementChannel,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--receipt=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementReceipt,
                    PathBuf::from(value),
                    "--receipt",
                )?;
            } else if let Some(value) = arg.strip_prefix("--settlement-receipt=") {
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementReceipt,
                    PathBuf::from(value),
                    "--settlement-receipt",
                )?;
            } else if arg == "--receipt" || arg == "--settlement-receipt" {
                index += 1;
                parsed.set_payload(
                    OrderbookValidationPayloadKindV1::SettlementReceipt,
                    PathBuf::from(require_value(args, index, arg)?),
                    arg,
                )?;
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown orderbook option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn set_payload(
        &mut self,
        kind: OrderbookValidationPayloadKindV1,
        path: PathBuf,
        flag: &str,
    ) -> Result<(), CliError> {
        if self.input.is_some() || self.kind.is_some() {
            return Err(CliError::Config(format!(
                "orderbook accepts one payload input; `{flag}` conflicts with an earlier orderbook input"
            )));
        }
        self.input = Some(path);
        self.kind = Some(kind);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct BundleArgs {
    bundle: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    now: Option<u64>,
    generated_at: Option<u64>,
}

impl BundleArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = BundleArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--bundle=") {
                parsed.bundle = Some(PathBuf::from(value));
            } else if arg == "--bundle" {
                index += 1;
                parsed.bundle = Some(PathBuf::from(require_value(args, index, "--bundle")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--now=") {
                parsed.now = Some(parse_u64_flag(value, "--now")?);
            } else if arg == "--now" {
                index += 1;
                parsed.now = Some(parse_u64_flag(
                    require_value(args, index, "--now")?,
                    "--now",
                )?);
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown bundle option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct GovernanceArgs {
    node: Option<PathBuf>,
    block: Option<PathBuf>,
    head: Option<PathBuf>,
    blocks: Vec<PathBuf>,
    cid: Option<String>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl GovernanceArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = GovernanceArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--node=") {
                parsed.node = Some(PathBuf::from(value));
            } else if arg == "--node" {
                index += 1;
                parsed.node = Some(PathBuf::from(require_value(args, index, "--node")?));
            } else if let Some(value) = arg.strip_prefix("--block=") {
                if parsed.head.is_some() {
                    parsed.blocks.push(PathBuf::from(value));
                } else if parsed.block.is_none() {
                    parsed.block = Some(PathBuf::from(value));
                } else {
                    parsed.blocks.push(PathBuf::from(value));
                }
            } else if arg == "--block" {
                index += 1;
                let value = require_value(args, index, "--block")?;
                if parsed.head.is_some() {
                    parsed.blocks.push(PathBuf::from(value));
                } else if parsed.block.is_none() {
                    parsed.block = Some(PathBuf::from(value));
                } else {
                    parsed.blocks.push(PathBuf::from(value));
                }
            } else if let Some(value) = arg.strip_prefix("--head=") {
                parsed.head = Some(PathBuf::from(value));
                if let Some(block) = parsed.block.take() {
                    parsed.blocks.push(block);
                }
            } else if arg == "--head" {
                index += 1;
                parsed.head = Some(PathBuf::from(require_value(args, index, "--head")?));
                if let Some(block) = parsed.block.take() {
                    parsed.blocks.push(block);
                }
            } else if let Some(value) = arg.strip_prefix("--input=") {
                parsed.node = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.node = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--cid=") {
                parsed.cid = Some(value.to_owned());
            } else if arg == "--cid" {
                index += 1;
                parsed.cid = Some(require_value(args, index, "--cid")?.to_owned());
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown governance option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct SignArgs {
    kind: Option<SignKind>,
    input: Option<PathBuf>,
    out: Option<PathBuf>,
    key_hex: Option<String>,
    key: Option<PathBuf>,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    now: Option<u64>,
    generated_at: Option<u64>,
}

impl SignArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = SignArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--kind=") {
                parsed.kind = Some(parse_sign_kind(value)?);
            } else if arg == "--kind" {
                index += 1;
                parsed.kind = Some(parse_sign_kind(require_value(args, index, "--kind")?)?);
            } else if let Some(value) = arg.strip_prefix("--input=") {
                parsed.input = Some(PathBuf::from(value));
            } else if arg == "--input" {
                index += 1;
                parsed.input = Some(PathBuf::from(require_value(args, index, "--input")?));
            } else if let Some(value) = arg.strip_prefix("--out=") {
                parsed.out = Some(PathBuf::from(value));
            } else if arg == "--out" {
                index += 1;
                parsed.out = Some(PathBuf::from(require_value(args, index, "--out")?));
            } else if let Some(value) = arg.strip_prefix("--key-hex=") {
                parsed.key_hex = Some(value.to_owned());
            } else if arg == "--key-hex" {
                index += 1;
                parsed.key_hex = Some(require_value(args, index, "--key-hex")?.to_owned());
            } else if let Some(value) = arg.strip_prefix("--key=") {
                parsed.key = Some(PathBuf::from(value));
            } else if arg == "--key" {
                index += 1;
                parsed.key = Some(PathBuf::from(require_value(args, index, "--key")?));
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--now=") {
                parsed.now = Some(parse_u64_flag(value, "--now")?);
            } else if arg == "--now" {
                index += 1;
                parsed.now = Some(parse_u64_flag(
                    require_value(args, index, "--now")?,
                    "--now",
                )?);
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown sign option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignKind {
    Advert,
    Order,
    Governance,
}

impl PotrArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = PotrArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--receipt=") {
                parsed.receipt = Some(PathBuf::from(value));
            } else if arg == "--receipt" {
                index += 1;
                parsed.receipt = Some(PathBuf::from(require_value(args, index, "--receipt")?));
            } else if let Some(value) = arg.strip_prefix("--profile=") {
                parsed.profile = Some(parse_profile(value)?);
            } else if arg == "--profile" {
                index += 1;
                parsed.profile = Some(parse_profile(require_value(args, index, "--profile")?)?);
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown potr option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }
}

#[derive(Debug, Default)]
struct OrderArgs {
    order: Option<PathBuf>,
    signed: bool,
    format: Option<OutputFormat>,
    telemetry_out: Option<PathBuf>,
    generated_at: Option<u64>,
}

impl OrderArgs {
    fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut parsed = OrderArgs::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(value) = arg.strip_prefix("--order=") {
                parsed.set_order(PathBuf::from(value), false, "--order")?;
            } else if arg == "--order" {
                index += 1;
                parsed.set_order(
                    PathBuf::from(require_value(args, index, "--order")?),
                    false,
                    "--order",
                )?;
            } else if let Some(value) = arg.strip_prefix("--input=") {
                parsed.set_order(PathBuf::from(value), false, "--input")?;
            } else if arg == "--input" {
                index += 1;
                parsed.set_order(
                    PathBuf::from(require_value(args, index, "--input")?),
                    false,
                    "--input",
                )?;
            } else if let Some(value) = arg.strip_prefix("--signed-order=") {
                parsed.set_order(PathBuf::from(value), true, "--signed-order")?;
            } else if arg == "--signed-order" {
                index += 1;
                parsed.set_order(
                    PathBuf::from(require_value(args, index, "--signed-order")?),
                    true,
                    "--signed-order",
                )?;
            } else if let Some(value) = arg.strip_prefix("--format=") {
                parsed.format = Some(OutputFormat::parse(value)?);
            } else if arg == "--format" {
                index += 1;
                parsed.format = Some(OutputFormat::parse(require_value(
                    args, index, "--format",
                )?)?);
            } else if let Some(value) = arg.strip_prefix("--telemetry-out=") {
                parsed.telemetry_out = Some(PathBuf::from(value));
            } else if arg == "--telemetry-out" {
                index += 1;
                parsed.telemetry_out = Some(PathBuf::from(require_value(
                    args,
                    index,
                    "--telemetry-out",
                )?));
            } else if let Some(value) = arg.strip_prefix("--generated-at=") {
                parsed.generated_at = Some(parse_u64_flag(value, "--generated-at")?);
            } else if arg == "--generated-at" {
                index += 1;
                parsed.generated_at = Some(parse_u64_flag(
                    require_value(args, index, "--generated-at")?,
                    "--generated-at",
                )?);
            } else {
                return Err(CliError::Config(format!(
                    "unknown order option `{arg}`; run `sorafs-validate --help`"
                )));
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn set_order(&mut self, path: PathBuf, signed: bool, flag: &str) -> Result<(), CliError> {
        if self.order.is_some() {
            return Err(CliError::Config(format!(
                "order accepts one input path; duplicate `{flag}`"
            )));
        }
        self.order = Some(path);
        self.signed = signed;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Json,
    Table,
    Yaml,
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            "yaml" => Ok(Self::Yaml),
            other => Err(CliError::Config(format!(
                "unsupported --format `{other}`; expected json, table, or yaml"
            ))),
        }
    }
}

#[derive(Debug)]
enum CliError {
    Config(String),
    Io(String),
    Internal(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            CliError::Config(_) => 4,
            CliError::Io(_) => 3,
            CliError::Internal(_) => 10,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Config(message) | CliError::Io(message) | CliError::Internal(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for CliError {}

fn require_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, CliError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| CliError::Config(format!("{flag} requires a value")))
}

fn parse_u64_flag(value: &str, flag: &str) -> Result<u64, CliError> {
    value
        .parse::<u64>()
        .map_err(|err| CliError::Config(format!("{flag} must be an unsigned integer: {err}")))
}

fn parse_cid_arg_bytes(value: &str) -> Result<Vec<u8>, CliError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::Config(
            "governance --cid must not be empty".to_owned(),
        ));
    }
    if let Some(hex_value) = trimmed.strip_prefix("hex:") {
        return hex::decode(hex_value).map_err(|err| {
            CliError::Config(format!("invalid governance --cid hex `{trimmed}`: {err}"))
        });
    }
    if trimmed.len() % 2 == 0
        && trimmed
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return hex::decode(trimmed).map_err(|err| {
            CliError::Config(format!("invalid governance --cid hex `{trimmed}`: {err}"))
        });
    }
    Ok(trimmed.as_bytes().to_vec())
}

fn parse_profile(value: &str) -> Result<ProofStreamTier, CliError> {
    match value {
        "hot" => Ok(ProofStreamTier::Hot),
        "warm" => Ok(ProofStreamTier::Warm),
        "archive" | "cold" => Ok(ProofStreamTier::Archive),
        other => Err(CliError::Config(format!(
            "unsupported --profile `{other}`; expected hot, warm, archive, or cold"
        ))),
    }
}

fn parse_repair_kind(value: &str) -> Result<RepairValidationPayloadKindV1, CliError> {
    match value {
        "evidence" | "repair-evidence" => Ok(RepairValidationPayloadKindV1::Evidence),
        "report" | "repair-report" => Ok(RepairValidationPayloadKindV1::Report),
        "task" | "task-record" | "repair-task" | "repair-task-record" => {
            Ok(RepairValidationPayloadKindV1::TaskRecord)
        }
        "slash" | "slash-proposal" | "repair-slash-proposal" => {
            Ok(RepairValidationPayloadKindV1::SlashProposal)
        }
        "policy" | "escalation-policy" | "repair-escalation-policy" => {
            Ok(RepairValidationPayloadKindV1::EscalationPolicy)
        }
        "approval" | "escalation-approval" | "repair-escalation-approval" => {
            Ok(RepairValidationPayloadKindV1::EscalationApproval)
        }
        "signed-auditor" | "signed-auditor-request" => {
            Ok(RepairValidationPayloadKindV1::SignedAuditorRequest)
        }
        "worker" | "worker-signature" | "worker-signature-payload" => {
            Ok(RepairValidationPayloadKindV1::WorkerSignaturePayload)
        }
        "event" | "task-event" | "repair-task-event" => {
            Ok(RepairValidationPayloadKindV1::TaskEvent)
        }
        "audit-event" | "repair-audit-event" => Ok(RepairValidationPayloadKindV1::AuditEvent),
        other => Err(CliError::Config(format!(
            "unsupported repair --kind `{other}`; expected task, evidence, report, slash-proposal, policy, approval, signed-auditor-request, worker-signature, event, or audit-event"
        ))),
    }
}

fn parse_orderbook_kind(value: &str) -> Result<OrderbookValidationPayloadKindV1, CliError> {
    match value {
        "order" | "order-request" | "request" => Ok(OrderbookValidationPayloadKindV1::OrderRequest),
        "cancel" | "order-cancel" | "cancel-request" => {
            Ok(OrderbookValidationPayloadKindV1::OrderCancel)
        }
        "trade" | "trade-event" => Ok(OrderbookValidationPayloadKindV1::TradeEvent),
        "channel" | "settlement-channel" => Ok(OrderbookValidationPayloadKindV1::SettlementChannel),
        "receipt" | "settlement-receipt" => Ok(OrderbookValidationPayloadKindV1::SettlementReceipt),
        other => Err(CliError::Config(format!(
            "unsupported orderbook --kind `{other}`; expected order-request, order-cancel, trade-event, settlement-channel, or settlement-receipt"
        ))),
    }
}

fn parse_sign_kind(value: &str) -> Result<SignKind, CliError> {
    match value {
        "advert" | "provider-advert" => Ok(SignKind::Advert),
        "order" | "replication-order" => Ok(SignKind::Order),
        "governance" | "governance-log-node" => Ok(SignKind::Governance),
        other => Err(CliError::Config(format!(
            "unsupported sign --kind `{other}`; expected advert, order, or governance"
        ))),
    }
}

fn read_signing_seed(args: &SignArgs) -> Result<[u8; 32], CliError> {
    match (&args.key_hex, &args.key) {
        (Some(_), Some(_)) => Err(CliError::Config(
            "sign accepts either --key-hex or --key, not both".to_owned(),
        )),
        (Some(key_hex), None) => parse_ed25519_seed_hex(key_hex, "--key-hex"),
        (None, Some(path)) => {
            let contents = fs::read_to_string(path).map_err(|err| {
                CliError::Io(format!(
                    "failed to read signing key {}: {err}",
                    path.display()
                ))
            })?;
            parse_ed25519_seed_hex(&contents, "--key")
        }
        (None, None) => Err(CliError::Config(
            "sign requires --key-hex <32-byte-hex-seed> or --key <path>".to_owned(),
        )),
    }
}

fn parse_ed25519_seed_hex(value: &str, flag: &str) -> Result<[u8; 32], CliError> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix("ed25519:").unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes = hex::decode(trimmed).map_err(|err| {
        CliError::Config(format!(
            "{flag} must contain a 32-byte Ed25519 seed encoded as hex: {err}"
        ))
    })?;
    let seed: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        CliError::Config(format!(
            "{flag} must contain exactly 32 seed bytes, got {}",
            bytes.len()
        ))
    })?;
    Ok(seed)
}

fn sign_provider_advert(advert: &mut ProviderAdvertV1, seed: &[u8; 32]) -> Result<(), CliError> {
    let signing_key = SigningKey::from_bytes(seed);
    let body_bytes = norito::to_bytes(&advert.body).map_err(|err| {
        CliError::Internal(format!(
            "failed to encode provider advert body for signing: {err}"
        ))
    })?;
    let signature = signing_key.sign(&body_bytes);
    advert.signature = AdvertSignature {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    advert.signature_strict = true;
    Ok(())
}

fn sign_replication_order(
    order: ReplicationOrderV1,
    seed: &[u8; 32],
) -> Result<SignedReplicationOrderV1, CliError> {
    let signing_key = SigningKey::from_bytes(seed);
    let mut signed_order = SignedReplicationOrderV1 {
        version: SIGNED_REPLICATION_ORDER_VERSION_V1,
        order,
        signature: ReplicationOrderSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: vec![0; 64],
        },
    };
    let payload_bytes = signed_order.signature_payload_bytes().map_err(|err| {
        CliError::Internal(format!(
            "failed to encode replication order payload for signing: {err}"
        ))
    })?;
    let signature = signing_key.sign(&payload_bytes);
    signed_order.signature.signature = signature.to_bytes().to_vec();
    Ok(signed_order)
}

fn sign_governance_log_node(
    node: &mut GovernanceLogNodeV1,
    seed: &[u8; 32],
) -> Result<(), CliError> {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = node.signature_payload_bytes().map_err(|err| {
        CliError::Internal(format!(
            "failed to encode governance log node payload for signing: {err}"
        ))
    })?;
    let signature = signing_key.sign(&payload_bytes);
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    Ok(())
}

#[derive(Debug)]
struct OwnedBundlePayload {
    kind: FixtureBundlePayloadKindV1,
    label: String,
    bytes: Vec<u8>,
}

const BUNDLE_PAYLOAD_CANDIDATES: &[(FixtureBundlePayloadKindV1, &[&str])] = &[
    (
        FixtureBundlePayloadKindV1::ProviderAdvert,
        &[
            "provider_admission/advert_v1.to",
            "advert_v1.to",
            "advert.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::ProviderAdmissionEnvelope,
        &[
            "provider_admission/envelope_v1.to",
            "envelope_v1.to",
            "envelope.to",
            "admission.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::ReplicationOrder,
        &["replication_order/order_v1.to", "order_v1.to", "order.to"],
    ),
    (
        FixtureBundlePayloadKindV1::PdpCommitment,
        &[
            "pdp/commitment_v1.to",
            "pdp_commitment_v1.to",
            "commitment_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::PdpChallenge,
        &[
            "pdp/challenge_v1.to",
            "pdp_challenge_v1.to",
            "pdp-challenge.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::PdpProof,
        &["pdp/proof_v1.to", "pdp_proof_v1.to", "pdp-proof.to"],
    ),
    (
        FixtureBundlePayloadKindV1::PorChallenge,
        &["por/challenge_v1.to", "challenge_v1.to", "challenge.to"],
    ),
    (
        FixtureBundlePayloadKindV1::PorProof,
        &["por/proof_v1.to", "proof_v1.to", "proof.to"],
    ),
    (
        FixtureBundlePayloadKindV1::PotrReceipt,
        &["potr/receipt_v1.to", "receipt_v1.to", "potr_receipt_v1.to"],
    ),
    (
        FixtureBundlePayloadKindV1::RepairEvidence,
        &[
            "repair/evidence_v1.to",
            "evidence_v1.to",
            "repair_evidence_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::RepairReport,
        &["repair/report_v1.to", "report_v1.to", "repair_report_v1.to"],
    ),
    (
        FixtureBundlePayloadKindV1::RepairTaskRecord,
        &["repair/task_v1.to", "task_v1.to", "repair_task_v1.to"],
    ),
    (
        FixtureBundlePayloadKindV1::RepairSlashProposal,
        &[
            "repair/slash_proposal_v1.to",
            "slash_proposal_v1.to",
            "repair_slash_proposal_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::RepairTaskEvent,
        &["repair/event_v1.to", "event_v1.to", "repair_event_v1.to"],
    ),
    (
        FixtureBundlePayloadKindV1::OrderbookOrderRequest,
        &[
            "orderbook/order_request_v1.to",
            "order_request_v1.to",
            "orderbook_order_request_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::OrderbookOrderCancel,
        &[
            "orderbook/order_cancel_v1.to",
            "order_cancel_v1.to",
            "orderbook_order_cancel_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::OrderbookTradeEvent,
        &[
            "orderbook/trade_event_v1.to",
            "trade_event_v1.to",
            "orderbook_trade_event_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::OrderbookSettlementChannel,
        &[
            "orderbook/settlement_channel_v1.to",
            "settlement_channel_v1.to",
            "orderbook_settlement_channel_v1.to",
        ],
    ),
    (
        FixtureBundlePayloadKindV1::OrderbookSettlementReceipt,
        &[
            "orderbook/settlement_receipt_v1.to",
            "settlement_receipt_v1.to",
            "orderbook_settlement_receipt_v1.to",
        ],
    ),
];

fn read_bundle_payloads(bundle: &Path) -> Result<Vec<OwnedBundlePayload>, CliError> {
    if !bundle.is_dir() {
        return Err(CliError::Io(format!(
            "bundle path {} is not a directory",
            bundle.display()
        )));
    }

    let mut payloads = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for &(kind, candidates) in BUNDLE_PAYLOAD_CANDIDATES {
        for relative_path in candidates {
            let path = bundle.join(relative_path);
            if !path.is_file() || !seen_paths.insert(path.clone()) {
                continue;
            }
            let bytes = fs::read(&path)
                .map_err(|err| CliError::Io(format!("failed to read {}: {err}", path.display())))?;
            payloads.push(OwnedBundlePayload {
                kind,
                label: path.display().to_string(),
                bytes,
            });
        }
    }

    Ok(payloads)
}

fn write_json_outcome(path: &PathBuf, outcome: &ValidationOutcomeV1) -> Result<(), CliError> {
    let mut json = json::to_string_pretty(outcome)
        .map_err(|err| CliError::Internal(format!("failed to render outcome JSON: {err}")))?;
    json.push('\n');
    fs::write(path, json)
        .map_err(|err| CliError::Io(format!("failed to write {}: {err}", path.display())))
}

fn print_outcome(outcome: &ValidationOutcomeV1, format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => {
            let mut rendered = json::to_string_pretty(outcome).map_err(|err| {
                CliError::Internal(format!("failed to render outcome JSON: {err}"))
            })?;
            rendered.push('\n');
            print!("{rendered}");
        }
        OutputFormat::Table => {
            println!("STATUS\tCODE\tCATEGORY\tMESSAGE");
            println!(
                "{}\t{}\t{}\t{}",
                outcome.status, outcome.code, outcome.category, outcome.message
            );
        }
        OutputFormat::Yaml => {
            print!("{}", render_yaml(outcome));
        }
    }
    io::Write::flush(&mut io::stdout())
        .map_err(|err| CliError::Io(format!("failed to flush stdout: {err}")))
}

fn render_yaml(outcome: &ValidationOutcomeV1) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!("status: {}\n", yaml_string(&outcome.status)));
    rendered.push_str(&format!("code: {}\n", yaml_string(&outcome.code)));
    rendered.push_str(&format!("category: {}\n", yaml_string(&outcome.category)));
    rendered.push_str(&format!("message: {}\n", yaml_string(&outcome.message)));
    match &outcome.action {
        Some(action) => rendered.push_str(&format!("action: {}\n", yaml_string(action))),
        None => rendered.push_str("action: null\n"),
    }
    match &outcome.docs_url {
        Some(docs_url) => rendered.push_str(&format!("docs_url: {}\n", yaml_string(docs_url))),
        None => rendered.push_str("docs_url: null\n"),
    }
    rendered.push_str("telemetry_tags:\n");
    for tag in &outcome.telemetry_tags {
        rendered.push_str(&format!("  - {}\n", yaml_string(tag)));
    }
    rendered.push_str("context:\n");
    for field in &outcome.context {
        rendered.push_str(&format!("  - key: {}\n", yaml_string(&field.key)));
        rendered.push_str(&format!("    value: {}\n", yaml_string(&field.value)));
    }
    rendered.push_str("inputs:\n");
    for input in &outcome.inputs {
        rendered.push_str(&format!("  - kind: {}\n", yaml_string(&input.kind)));
        rendered.push_str(&format!("    path: {}\n", yaml_string(&input.path)));
    }
    rendered.push_str(&format!("version: {}\n", outcome.version));
    rendered.push_str(&format!("generated_at: {}\n", outcome.generated_at));
    rendered
}

fn yaml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

fn unix_time_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn print_usage() {
    println!(
        "\
Usage:
  sorafs-validate advert --input <path> [--format table|json|yaml] [--telemetry-out <path>] [--now <unix-seconds>]
  sorafs-validate admission --input <path> [--renewal <path> | --revocation <path>] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate order (--order <path> | --signed-order <path>) [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate orderbook --kind <payload-kind> --input <path> [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate orderbook --order <path> | --cancel <path> | --trade <path> | --channel <path> | --receipt <path>
  sorafs-validate pdp [--commitment <path>] [--challenge <path>] [--proof <path>] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate por --challenge <path> --proof <path> [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate potr --receipt <path> [--profile hot|warm|archive|cold] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate repair --kind <payload-kind> --input <path> [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate repair --task <path> | --evidence <path> | --report <path> | --signed-auditor-request <path>
  sorafs-validate bundle --bundle <dir> [--format table|json|yaml] [--telemetry-out <path>] [--now <unix-seconds>]
  sorafs-validate governance --node <path> [--cid <node-cid>] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate governance --block <path> [--cid <block-cid|hex:HEX>] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate governance --head <path> --block <path> [--block <path>...] [--format table|json|yaml] [--telemetry-out <path>]
  sorafs-validate sign --kind advert --input <advert.to> --out <signed-advert.to> (--key-hex <hex> | --key <path>) [--format table|json|yaml] [--now <unix-seconds>]
  sorafs-validate sign --kind order --input <order.to> --out <signed-order.to> (--key-hex <hex> | --key <path>) [--format table|json|yaml]
  sorafs-validate sign --kind governance --input <node.to> --out <signed-node.to> (--key-hex <hex> | --key <path>) [--format table|json|yaml]

Exit codes:
  0  validation succeeded
  2  validation, policy, signature, or Norito payload error
  3  input/output error
  4  command-line configuration error
  10 internal fault
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }

    #[test]
    fn output_format_parse_accepts_supported_values() {
        assert!(matches!(
            OutputFormat::parse("json"),
            Ok(OutputFormat::Json)
        ));
        assert!(matches!(
            OutputFormat::parse("table"),
            Ok(OutputFormat::Table)
        ));
        assert!(matches!(
            OutputFormat::parse("yaml"),
            Ok(OutputFormat::Yaml)
        ));
    }

    #[test]
    fn output_format_parse_rejects_unknown_values() {
        assert!(matches!(
            OutputFormat::parse("xml"),
            Err(CliError::Config(message)) if message.contains("unsupported --format")
        ));
    }

    #[test]
    fn advert_args_parse_reads_input_format_and_timestamps() {
        let args = [
            "--input=advert.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--now=5".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = AdvertArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("advert.to")));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.now, Some(5));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn admission_args_parse_reads_input_format_and_generated_at() {
        let args = [
            "--input=envelope.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = AdmissionArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("envelope.to")));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn admission_args_parse_accepts_envelope_alias() {
        let args = ["--envelope=envelope.to".to_owned()];
        let parsed = AdmissionArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("envelope.to")));
    }

    #[test]
    fn admission_args_parse_reads_renewal_and_revocation_flags() {
        let args = [
            "--envelope=envelope.to".to_owned(),
            "--renewal=renewal.to".to_owned(),
        ];
        let parsed = AdmissionArgs::parse(&args).expect("parse renewal args");
        assert_eq!(parsed.input, Some(PathBuf::from("envelope.to")));
        assert_eq!(parsed.renewal, Some(PathBuf::from("renewal.to")));
        assert_eq!(parsed.revocation, None);

        let args = [
            "--envelope=envelope.to".to_owned(),
            "--revocation=revocation.to".to_owned(),
        ];
        let parsed = AdmissionArgs::parse(&args).expect("parse revocation args");
        assert_eq!(parsed.input, Some(PathBuf::from("envelope.to")));
        assert_eq!(parsed.renewal, None);
        assert_eq!(parsed.revocation, Some(PathBuf::from("revocation.to")));
    }

    #[test]
    fn admission_args_parse_rejects_renewal_revocation_conflict() {
        let args = [
            "--envelope=envelope.to".to_owned(),
            "--renewal=renewal.to".to_owned(),
            "--revocation=revocation.to".to_owned(),
        ];
        assert!(matches!(
            AdmissionArgs::parse(&args),
            Err(CliError::Config(message))
                if message.contains("either --renewal or --revocation")
        ));
    }

    #[test]
    fn por_args_parse_reads_inputs_format_and_generated_at() {
        let args = [
            "--challenge=challenge.to".to_owned(),
            "--proof=proof.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = PorArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.challenge, Some(PathBuf::from("challenge.to")));
        assert_eq!(parsed.proof, Some(PathBuf::from("proof.to")));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn pdp_args_parse_reads_inputs_format_and_generated_at() {
        let args = [
            "--commitment=commitment.to".to_owned(),
            "--challenge=challenge.to".to_owned(),
            "--proof=proof.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = PdpArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.commitment, Some(PathBuf::from("commitment.to")));
        assert_eq!(parsed.challenge, Some(PathBuf::from("challenge.to")));
        assert_eq!(parsed.proof, Some(PathBuf::from("proof.to")));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn potr_args_parse_reads_receipt_profile_format_and_generated_at() {
        let args = [
            "--receipt=receipt.to".to_owned(),
            "--profile=cold".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = PotrArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.receipt, Some(PathBuf::from("receipt.to")));
        assert!(matches!(parsed.profile, Some(ProofStreamTier::Archive)));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn bundle_args_parse_reads_directory_format_and_timestamps() {
        let args = [
            "--bundle=fixtures/sorafs_manifest".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--now=5".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = BundleArgs::parse(&args).expect("parse args");
        assert_eq!(
            parsed.bundle,
            Some(PathBuf::from("fixtures/sorafs_manifest"))
        );
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.now, Some(5));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn governance_args_parse_reads_node_cid_format_and_generated_at() {
        let args = [
            "--node=governance.to".to_owned(),
            "--cid=bafygovernancelognode".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = GovernanceArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.node, Some(PathBuf::from("governance.to")));
        assert_eq!(parsed.cid.as_deref(), Some("bafygovernancelognode"));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn governance_args_parse_reads_head_and_block_chain() {
        let args = [
            "--block=block-0.to".to_owned(),
            "--head=head.to".to_owned(),
            "--block=block-1.to".to_owned(),
            "--format=json".to_owned(),
        ];
        let parsed = GovernanceArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.node, None);
        assert_eq!(parsed.block, None);
        assert_eq!(parsed.head, Some(PathBuf::from("head.to")));
        assert_eq!(
            parsed.blocks,
            vec![PathBuf::from("block-0.to"), PathBuf::from("block-1.to")]
        );
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
    }

    #[test]
    fn parse_cid_arg_bytes_accepts_hex_and_raw_cids() {
        assert_eq!(
            parse_cid_arg_bytes("hex:0a0b").expect("parse prefixed hex"),
            vec![0x0A, 0x0B]
        );
        assert_eq!(
            parse_cid_arg_bytes("0a0b").expect("parse bare hex"),
            vec![0x0A, 0x0B]
        );
        assert_eq!(
            parse_cid_arg_bytes("bafygovernance").expect("parse raw CID"),
            b"bafygovernance".to_vec()
        );
    }

    #[test]
    fn sign_args_parse_reads_advert_signing_flags() {
        let args = [
            "--kind=advert".to_owned(),
            "--input=advert.to".to_owned(),
            "--out=signed-advert.to".to_owned(),
            "--key-hex=ed25519:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--now=5".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = SignArgs::parse(&args).expect("parse args");
        assert!(matches!(parsed.kind, Some(SignKind::Advert)));
        assert_eq!(parsed.input, Some(PathBuf::from("advert.to")));
        assert_eq!(parsed.out, Some(PathBuf::from("signed-advert.to")));
        assert_eq!(
            parsed.key_hex.as_deref(),
            Some("ed25519:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.now, Some(5));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn parse_sign_kind_accepts_supported_aliases() {
        assert_eq!(parse_sign_kind("advert").unwrap(), SignKind::Advert);
        assert_eq!(
            parse_sign_kind("replication-order").unwrap(),
            SignKind::Order
        );
        assert_eq!(
            parse_sign_kind("governance-log-node").unwrap(),
            SignKind::Governance
        );
    }

    #[test]
    fn parse_ed25519_seed_hex_accepts_prefixes() {
        let seed = parse_ed25519_seed_hex(
            "ed25519:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--key-hex",
        )
        .expect("parse seed");
        assert_eq!(seed, [0xAA; 32]);
    }

    #[test]
    fn parse_ed25519_seed_hex_rejects_wrong_length() {
        assert!(matches!(
            parse_ed25519_seed_hex("abcd", "--key-hex"),
            Err(CliError::Config(message)) if message.contains("exactly 32 seed bytes")
        ));
    }

    #[test]
    fn read_signing_seed_accepts_key_hex_and_rejects_conflicts() {
        let parsed = SignArgs {
            key_hex: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ),
            ..SignArgs::default()
        };
        assert_eq!(read_signing_seed(&parsed).unwrap(), [0xAA; 32]);

        let conflict = SignArgs {
            key_hex: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ),
            key: Some(PathBuf::from("key.hex")),
            ..SignArgs::default()
        };
        assert!(matches!(
            read_signing_seed(&conflict),
            Err(CliError::Config(message)) if message.contains("either --key-hex or --key")
        ));
    }

    #[test]
    fn sign_provider_advert_replaces_signature_with_verified_ed25519_signature() {
        let fixture = workspace_fixture("fixtures/sorafs_manifest/provider_admission/advert_v1.to");
        let bytes = fs::read(fixture).expect("read advert fixture");
        let mut advert: ProviderAdvertV1 =
            norito::decode_from_bytes(&bytes).expect("decode advert fixture");
        let original_public_key = advert.signature.public_key.clone();
        let seed = [0xA5; 32];

        sign_provider_advert(&mut advert, &seed).expect("sign advert");

        let expected_key = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        assert_eq!(advert.signature.public_key, expected_key.to_vec());
        assert_ne!(advert.signature.public_key, original_public_key);
        assert!(advert.signature_strict);
        advert.verify_signature().expect("signed advert verifies");
    }

    #[test]
    fn sign_replication_order_returns_verified_ed25519_envelope() {
        let fixture = workspace_fixture("fixtures/sorafs_manifest/replication_order/order_v1.to");
        let bytes = fs::read(fixture).expect("read order fixture");
        let order: ReplicationOrderV1 =
            norito::decode_from_bytes(&bytes).expect("decode order fixture");
        let seed = [0xA7; 32];

        let signed_order = sign_replication_order(order.clone(), &seed).expect("sign order");

        let expected_key = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        assert_eq!(signed_order.version, SIGNED_REPLICATION_ORDER_VERSION_V1);
        assert_eq!(signed_order.order, order);
        assert_eq!(signed_order.signature.public_key, expected_key.to_vec());
        assert_eq!(
            signed_order.signature.algorithm,
            SignatureAlgorithm::Ed25519
        );
        signed_order
            .verify_signature()
            .expect("signed replication order verifies");
    }

    #[test]
    fn sign_governance_log_node_replaces_signature_with_verified_ed25519_signature() {
        let fixture = workspace_fixture("fixtures/sorafs_manifest/governance/node_v1.to");
        let bytes = fs::read(fixture).expect("read governance fixture");
        let mut node: GovernanceLogNodeV1 =
            norito::decode_from_bytes(&bytes).expect("decode governance fixture");
        let original_public_key = node.publisher_signature.public_key.clone();
        let seed = [0xA6; 32];

        sign_governance_log_node(&mut node, &seed).expect("sign governance node");

        let expected_key = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        assert_eq!(node.publisher_signature.public_key, expected_key.to_vec());
        assert_ne!(node.publisher_signature.public_key, original_public_key);
        node.verify_publisher_signature()
            .expect("signed governance node verifies");
    }

    #[test]
    fn parse_profile_accepts_archive_aliases() {
        assert!(matches!(parse_profile("hot"), Ok(ProofStreamTier::Hot)));
        assert!(matches!(parse_profile("warm"), Ok(ProofStreamTier::Warm)));
        assert!(matches!(
            parse_profile("archive"),
            Ok(ProofStreamTier::Archive)
        ));
        assert!(matches!(
            parse_profile("cold"),
            Ok(ProofStreamTier::Archive)
        ));
    }

    #[test]
    fn repair_args_parse_reads_kind_input_format_and_generated_at() {
        let args = [
            "--kind=task".to_owned(),
            "--input=repair-task.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = RepairArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("repair-task.to")));
        assert!(matches!(
            parsed.kind,
            Some(RepairValidationPayloadKindV1::TaskRecord)
        ));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn repair_args_parse_accepts_task_alias() {
        let args = ["--task=repair-task.to".to_owned()];
        let parsed = RepairArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("repair-task.to")));
        assert!(matches!(
            parsed.kind,
            Some(RepairValidationPayloadKindV1::TaskRecord)
        ));
    }

    #[test]
    fn repair_args_parse_rejects_multiple_payload_aliases() {
        let args = [
            "--task=repair-task.to".to_owned(),
            "--report=repair-report.to".to_owned(),
        ];
        assert!(matches!(
            RepairArgs::parse(&args),
            Err(CliError::Config(message)) if message.contains("repair accepts one payload input")
        ));
    }

    #[test]
    fn parse_repair_kind_accepts_supported_aliases() {
        assert!(matches!(
            parse_repair_kind("task-record"),
            Ok(RepairValidationPayloadKindV1::TaskRecord)
        ));
        assert!(matches!(
            parse_repair_kind("signed-auditor-request"),
            Ok(RepairValidationPayloadKindV1::SignedAuditorRequest)
        ));
        assert!(matches!(
            parse_repair_kind("audit-event"),
            Ok(RepairValidationPayloadKindV1::AuditEvent)
        ));
    }

    #[test]
    fn orderbook_args_parse_reads_kind_input_format_and_generated_at() {
        let args = [
            "--kind=settlement-receipt".to_owned(),
            "--input=receipt.to".to_owned(),
            "--format=json".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = OrderbookArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("receipt.to")));
        assert!(matches!(
            parsed.kind,
            Some(OrderbookValidationPayloadKindV1::SettlementReceipt)
        ));
        assert!(matches!(parsed.format, Some(OutputFormat::Json)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn orderbook_args_parse_accepts_receipt_alias() {
        let args = ["--receipt=receipt.to".to_owned()];
        let parsed = OrderbookArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.input, Some(PathBuf::from("receipt.to")));
        assert!(matches!(
            parsed.kind,
            Some(OrderbookValidationPayloadKindV1::SettlementReceipt)
        ));
    }

    #[test]
    fn orderbook_args_parse_rejects_multiple_payload_aliases() {
        let args = [
            "--order=order.to".to_owned(),
            "--receipt=receipt.to".to_owned(),
        ];
        assert!(matches!(
            OrderbookArgs::parse(&args),
            Err(CliError::Config(message)) if message.contains("orderbook accepts one payload input")
        ));
    }

    #[test]
    fn parse_orderbook_kind_accepts_supported_aliases() {
        assert!(matches!(
            parse_orderbook_kind("order-request"),
            Ok(OrderbookValidationPayloadKindV1::OrderRequest)
        ));
        assert!(matches!(
            parse_orderbook_kind("order-cancel"),
            Ok(OrderbookValidationPayloadKindV1::OrderCancel)
        ));
        assert!(matches!(
            parse_orderbook_kind("trade-event"),
            Ok(OrderbookValidationPayloadKindV1::TradeEvent)
        ));
        assert!(matches!(
            parse_orderbook_kind("settlement-channel"),
            Ok(OrderbookValidationPayloadKindV1::SettlementChannel)
        ));
        assert!(matches!(
            parse_orderbook_kind("settlement-receipt"),
            Ok(OrderbookValidationPayloadKindV1::SettlementReceipt)
        ));
    }

    #[test]
    fn order_args_parse_reads_order_format_and_generated_at() {
        let args = [
            "--order=order.to".to_owned(),
            "--format=yaml".to_owned(),
            "--telemetry-out=out.json".to_owned(),
            "--generated-at=6".to_owned(),
        ];
        let parsed = OrderArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.order, Some(PathBuf::from("order.to")));
        assert!(!parsed.signed);
        assert!(matches!(parsed.format, Some(OutputFormat::Yaml)));
        assert_eq!(parsed.telemetry_out, Some(PathBuf::from("out.json")));
        assert_eq!(parsed.generated_at, Some(6));
    }

    #[test]
    fn order_args_parse_accepts_input_alias() {
        let args = ["--input=order.to".to_owned()];
        let parsed = OrderArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.order, Some(PathBuf::from("order.to")));
        assert!(!parsed.signed);
    }

    #[test]
    fn order_args_parse_accepts_signed_order_alias() {
        let args = ["--signed-order=signed-order.to".to_owned()];
        let parsed = OrderArgs::parse(&args).expect("parse args");
        assert_eq!(parsed.order, Some(PathBuf::from("signed-order.to")));
        assert!(parsed.signed);
    }

    #[test]
    fn order_args_parse_rejects_multiple_inputs() {
        let args = [
            "--order=order.to".to_owned(),
            "--signed-order=signed-order.to".to_owned(),
        ];
        assert!(matches!(
            OrderArgs::parse(&args),
            Err(CliError::Config(message)) if message.contains("one input path")
        ));
    }

    #[test]
    fn cli_error_exit_code_matches_contract() {
        assert_eq!(CliError::Config("x".to_owned()).exit_code(), 4);
        assert_eq!(CliError::Io("x".to_owned()).exit_code(), 3);
        assert_eq!(CliError::Internal("x".to_owned()).exit_code(), 10);
    }

    #[test]
    fn yaml_string_quotes_control_characters() {
        assert_eq!(yaml_string("a\n\"b\""), "\"a\\n\\\"b\\\"\"");
    }
}
