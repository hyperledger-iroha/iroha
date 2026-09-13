//! Package-aware native deployment and authenticated on-chain views.
use super::*;
use crate::compiler::CompilerArtifactV1;
use iroha_contract_deploy::{
    DeploymentError, DeploymentPreflight, DeploymentProgress, DeploymentReceipt, DeploymentRequest,
    DeploymentService, JournalDisposition,
};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, smart_contract::ContractAlias,
    transaction::FeePaymentIntent,
};

#[derive(Args, Debug)]
pub(super) struct DeployArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Named configured network; uses the workspace default when omitted.
    #[arg(long)]
    network: Option<String>,
    /// Explicit native client file, which must match the pinned network identity.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Require the existing dependency lock to remain unchanged before building.
    #[arg(long)]
    locked: bool,
    /// One declared contract target; may be omitted when exactly one target is built.
    #[arg(long)]
    contract: Option<String>,
    /// Prepare and persist the exact signed plan without submitting it.
    #[arg(long, conflicts_with_all = ["resume", "cancel"])]
    prepare: bool,
    /// Resume this exact deployment journal, without rebuilding or re-signing its transactions.
    #[arg(long, value_name = "JOURNAL", conflicts_with_all = ["cancel", "contract", "locked", "workspace", "packages", "exclude"])]
    resume: Option<PathBuf>,
    /// Cancel an unattempted local plan; attempted transactions still require exact-hash recovery.
    #[arg(long, value_name = "JOURNAL", conflicts_with_all = ["contract", "locked", "workspace", "packages", "exclude"])]
    cancel: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(super) struct ViewArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Named network; uses the configured workspace default when omitted.
    #[arg(long)]
    network: Option<String>,
    /// Declared contract target with an exact alias binding for this network.
    #[arg(long)]
    contract: String,
    /// Public kotodama view entrypoint.
    #[arg(long)]
    entrypoint: String,
    /// JSON object containing named view arguments, for example {"coffees":"3"}.
    #[arg(long, default_value = "{}", value_name = "JSON")]
    args: String,
    /// VM execution budget for the read-only view.
    #[arg(long, default_value_t = 1_000_000, value_parser = clap::value_parser!(u64).range(1..=10_000_000))]
    gas_limit: u64,
}

pub(super) fn run_deploy(
    manifest: Option<&Path>,
    args: &DeployArgs,
    progress: &mut dyn FnMut(&str),
) -> CommandResult {
    progress(if args.cancel.is_some() {
        "Validating unattempted deployment cancellation..."
    } else if args.resume.is_some() {
        "Recovering the exact retained deployment plan..."
    } else {
        "Preparing the contract, network binding, permissions, and exact fee quotes..."
    });
    if let Some(journal) = args.resume.as_ref().or(args.cancel.as_ref()) {
        let (workspace, _) = load_selected_workspace(manifest, &args.selection)?;
        let network = network::select_network(
            workspace.root(),
            args.network.as_deref(),
            args.config.as_deref(),
            None,
        )?;
        let _profile = ChainDiscriminantGuard::enter(network.chain_discriminant);
        let service =
            DeploymentService::new(network.load_client()?).map_err(deployment_diagnostic)?;
        if args.cancel.is_some() {
            let cancellation = service.cancel(journal).map_err(deployment_diagnostic)?;
            return Ok(Success {
                message: format!(
                    "Cancelled unattempted deployment plan: {}",
                    journal.display()
                ),
                data: object([
                    ("status", Value::from("cancelled")),
                    ("journal", Value::from(journal.display().to_string())),
                    ("cancellation", deployment_json(&cancellation)?),
                ]),
            });
        }
        let receipt = service
            .resume(journal, &mut |event| progress(&render_progress(event)))
            .map_err(|error| {
                deployment_diagnostic(error).with_context("journal", journal.display().to_string())
            })?;
        return receipt_output(&receipt, journal);
    }
    let build_args = BuildArgs {
        selection: args.selection.clone(),
        mode: GraphModeArgs {
            locked: args.locked,
            offline: false,
            frozen: false,
        },
        registry: RegistryReadArgs {
            config: args.config.clone(),
        },
        network: args.network.clone(),
        chain_discriminant: None,
    };
    let build = build::prepare_build(manifest, &build_args, CompilerActionV1::Build)?;
    let artifact = select_artifact(&build.execution.artifacts, args.contract.as_deref())?;
    let alias = bound_alias(&build.network, &artifact.package, &artifact.target)?;
    let artifact_bytes = read_selected_artifact(artifact)?;
    let fee_payment = selected_fee_payment(&build.network)?;
    let _profile = ChainDiscriminantGuard::enter(build.network.chain_discriminant);
    let service =
        DeploymentService::new(build.network.load_client()?).map_err(deployment_diagnostic)?;
    let slot = deployment_slot(
        build.workspace.root(),
        &build.network.name,
        &artifact.package,
        &artifact.target,
    )?;
    let writer = AtomicWriteRoot::open_or_create_private(&slot).map_err(atomic_diagnostic)?;
    let _slot_lock = writer
        .lock_exclusive(Path::new("deployment.lock"))
        .map_err(atomic_diagnostic)?;
    ensure_previous_terminal(&writer, &service)?;
    let prepared = service
        .prepare(&DeploymentRequest {
            artifact: artifact_bytes,
            alias,
            fee_payment,
            governance_approvers: Vec::new(),
        })
        .map_err(deployment_diagnostic)?;
    let hash = prepared
        .preflight()
        .transaction_hashes
        .last()
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::Internal,
                "deployment plan contains no atomic commit",
            )
        })?;
    let hash = hash
        .parse::<iroha::crypto::HashOf<iroha_data_model::transaction::SignedTransaction>>()
        .map_err(|_| {
            Diagnostic::new(
                ErrorCode::Internal,
                "deployment plan contains an invalid transaction hash",
            )
        })?;
    let journal_id = hex::encode(hash.as_ref());
    let journal = slot.join(&journal_id);
    service
        .persist(&prepared, &journal)
        .map_err(deployment_diagnostic)?;
    writer
        .replace(Path::new("active-journal"), journal_id.as_bytes())
        .map_err(atomic_diagnostic)?;
    if args.prepare {
        return Ok(Success {
            message: format!(
                "Prepared {} for {}\n{}Plan: {}\nNext: musubi deploy --network {} --resume {}",
                artifact.target,
                build.network.name,
                render_preflight(prepared.preflight()),
                journal.display(),
                build.network.name,
                quote_cli_argument(&journal.display().to_string())
            ),
            data: object([
                ("journal", Value::from(journal.display().to_string())),
                ("preflight", deployment_json(prepared.preflight())?),
            ]),
        });
    }
    let receipt = service
        .execute(&prepared, &journal, &mut |event| {
            progress(&render_progress(event))
        })
        .map_err(|error| {
            deployment_diagnostic(error).with_context("journal", journal.display().to_string())
        })?;
    receipt_output(&receipt, &journal)
}

fn render_progress(event: DeploymentProgress) -> String {
    match event {
        DeploymentProgress::Prepared(preflight) => {
            format!(
                "Deployment plan\n{}",
                render_preflight(&preflight).trim_end()
            )
        }
        DeploymentProgress::Submitting(stage) => format!(
            "Stage {}/{} — submitting {}\n  Transaction: {}",
            stage.number,
            stage.total,
            stage.name.replace('_', " "),
            stage.hash
        ),
        DeploymentProgress::Recovering(stage) => format!(
            "Stage {}/{} — recovering {} by exact hash\n  Transaction: {}",
            stage.number,
            stage.total,
            stage.name.replace('_', " "),
            stage.hash
        ),
        DeploymentProgress::Applied { stage, evidence } => format!(
            "Stage {}/{} — Applied at height {} ({}, {})",
            stage.number,
            stage.total,
            evidence.block_height,
            evidence.scope,
            evidence.resolved_from
        ),
        DeploymentProgress::ReadingBack { alias, address } => format!(
            "Verifying stored artifact and alias readback\n  Alias: {alias}\n  Contract: {address}"
        ),
    }
}

fn read_selected_artifact(artifact: &CompilerArtifactV1) -> Result<Vec<u8>, Diagnostic> {
    let bytes = read_bounded_single_link_regular_file_v1(
        &artifact.artifact,
        iroha_contract_deploy::MAX_DEPLOYMENT_ARTIFACT_BYTES as u64,
    )
    .map_err(|error| io_diagnostic("read built contract", &artifact.artifact, &error))?;
    let verified = ivm::verify_contract_artifact(&bytes)
        .map_err(|error| Diagnostic::new(ErrorCode::PackageInvalid, error.to_string()))?;
    if verified.code_hash.to_string() != artifact.artifact_hash
        || verified.abi_hash.to_string() != artifact.abi_hash
    {
        return Err(Diagnostic::new(ErrorCode::PackageInvalid, "the built contract file no longer matches the selected compiler artifact")
            .with_context("contract", &artifact.target)
            .with_help("rebuild the selected package and inspect any concurrent process changing its target directory"));
    }
    Ok(bytes)
}

fn render_preflight(preflight: &DeploymentPreflight) -> String {
    let _profile = ChainDiscriminantGuard::enter(preflight.chain_discriminant);
    let mut output = format!(
        "Network identity: {}\nAddress profile: {}\nAuthority: {}\nAlias: {}\nContract: {}\nCode hash: {}\nABI hash: {}\nObserved height: {}\n",
        preflight.network_id,
        preflight.chain_discriminant,
        preflight.authority,
        preflight.contract_alias,
        preflight.contract_address,
        preflight.code_hash,
        preflight.abi_hash,
        preflight.observed_block_height,
    );
    for (index, (hash, quote)) in preflight
        .transaction_hashes
        .iter()
        .zip(&preflight.fee_quotes)
        .enumerate()
    {
        let _ = writeln!(output, "Transaction {}: {hash}", index + 1);
        if quote.components.is_empty() {
            output.push_str("  Quoted fee: no charge components\n");
        }
        for component in &quote.components {
            let _ = writeln!(
                output,
                "  {:?}: at most {} {}",
                component.kind, component.max_amount, component.asset_definition_id
            );
        }
    }
    output
}

pub(super) fn run_view(manifest: Option<&Path>, args: &ViewArgs) -> CommandResult {
    let (workspace, _) = load_selected_workspace(manifest, &args.selection)?;
    let selected = select_members(&workspace, &args.selection)?;
    let package = network::select_contract_package(&selected, &args.contract)?;
    let payload = parse_view_payload(&args.args)?;
    if args.entrypoint.is_empty() || args.entrypoint.trim() != args.entrypoint {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "view entrypoint must be a non-empty canonical selector",
        ));
    }
    let network = network::select_network(workspace.root(), args.network.as_deref(), None, None)?;
    let alias = bound_alias(&network, package, &args.contract)?;
    let _profile = ChainDiscriminantGuard::enter(network.chain_discriminant);
    let config = network.load_client()?;
    let authority = config.account.clone();
    let client = iroha::blocking::Client::new(config).map_err(view_diagnostic)?;
    client.refresh_capabilities().map_err(view_diagnostic)?;
    let result = client
        .client()
        .post_contract_view_json(
            &authority,
            None,
            Some(&alias),
            &args.entrypoint,
            Some(&payload),
            args.gas_limit,
        )
        .map_err(view_diagnostic)?;
    let rendered = norito::json::to_string_pretty(&result).map_err(|_| {
        Diagnostic::new(
            ErrorCode::Internal,
            "contract view result could not be rendered",
        )
    })?;
    Ok(Success {
        message: format!(
            "{} · {} · {}\n{rendered}",
            network.name, alias, args.entrypoint
        ),
        data: object([
            ("network", network.json()),
            ("contract", Value::from(args.contract.clone())),
            ("alias", Value::from(alias.to_string())),
            ("entrypoint", Value::from(args.entrypoint.clone())),
            ("result", result),
        ]),
    })
}

fn view_diagnostic(error: eyre::Report) -> Diagnostic {
    Diagnostic::new(ErrorCode::Network, format!("{error:#}"))
}

fn select_artifact<'a>(
    artifacts: &'a [CompilerArtifactV1],
    requested: Option<&str>,
) -> Result<&'a CompilerArtifactV1, Diagnostic> {
    let selected = artifacts
        .iter()
        .filter(|artifact| requested.is_none_or(|name| name == artifact.target))
        .collect::<Vec<_>>();
    match selected.as_slice() {
        [artifact] => Ok(artifact),
        [] => Err(Diagnostic::new(
            ErrorCode::Usage,
            "no deployable contract matches the selected package and target",
        )),
        _ => Err(Diagnostic::new(
            ErrorCode::Usage,
            "deployment requires one unambiguous contract target",
        )
        .with_help("select a package with --package and a contract with --contract")),
    }
}

fn bound_alias(
    network: &network::SelectedNetwork,
    package: &MusubiPackageSelectorV1,
    target: &str,
) -> Result<ContractAlias, Diagnostic> {
    network.contracts.get(&network::contract_key(package, target)).cloned().ok_or_else(|| Diagnostic::new(ErrorCode::Usage, "the contract has no alias binding on this network")
        .with_context("contract", target).with_context("network", &network.name)
        .with_help(format!("run `musubi network configure {} --config <client.toml> --package {} --contract {} --alias <name::domain>`", network.name, quote_cli_argument(&package.to_string()), quote_cli_argument(target))))
}

fn selected_fee_payment(
    network: &network::SelectedNetwork,
) -> Result<FeePaymentIntent, Diagnostic> {
    network.fee_payment.clone().ok_or_else(|| Diagnostic::new(ErrorCode::Usage, "deployment requires an explicit fee payer")
        .with_help("configure --fee-payer authority, or --fee-payer sponsor with its exact program and revision"))
}

fn parse_view_payload(source: &str) -> Result<Value, Diagnostic> {
    if source.len() > 64 * 1024 {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "view arguments exceed the 64 KiB bound",
        ));
    }
    let value: Value = norito::json::from_str(source)
        .map_err(|_| Diagnostic::new(ErrorCode::Usage, "--args must contain valid JSON"))?;
    if !matches!(value, Value::Object(_)) {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "--args must be a JSON object of named contract arguments",
        ));
    }
    Ok(value)
}

fn deployment_slot(
    root: &Path,
    network: &str,
    package: &MusubiPackageSelectorV1,
    target: &str,
) -> Result<PathBuf, Diagnostic> {
    network::validate_name(network)?;
    network::validate_contract_target(target)?;
    Ok(root.join("target/deploy").join(network).join(
        blake3::hash(network::contract_key(package, target).as_bytes())
            .to_hex()
            .as_str(),
    ))
}

fn validate_journal_id(id: &str) -> Result<(), Diagnostic> {
    if id.parse::<iroha::crypto::Hash>().is_err()
        || id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(Diagnostic::new(
            ErrorCode::PackageInvalid,
            "deployment journal must name one exact transaction hash",
        ));
    }
    Ok(())
}

fn ensure_previous_terminal(
    writer: &AtomicWriteRoot,
    service: &DeploymentService,
) -> Result<(), Diagnostic> {
    let Some(bytes) = writer
        .load_immutable(Path::new("active-journal"), 64)
        .map_err(atomic_diagnostic)?
    else {
        return Ok(());
    };
    let id = std::str::from_utf8(&bytes).map_err(|_| {
        Diagnostic::new(
            ErrorCode::PackageInvalid,
            "invalid active deployment journal",
        )
    })?;
    validate_journal_id(id)?;
    let journal = writer.path().join(id);
    if matches!(
        service
            .inspect_journal(&journal)
            .map_err(deployment_diagnostic)?,
        JournalDisposition::Pending { .. }
    ) {
        return Err(Diagnostic::new(
            ErrorCode::Network,
            "an earlier deployment plan is unresolved",
        )
        .with_help(format!(
            "resume the exact plan with `musubi deploy --resume {}`",
            quote_cli_argument(&journal.display().to_string())
        )));
    }
    Ok(())
}

fn receipt_output(receipt: &DeploymentReceipt, journal: &Path) -> CommandResult {
    Ok(Success {
        message: format!(
            "Applied: {}\nNetwork identity: {}\nAuthority: {}\nContract: {}\nAlias: {}\nCode hash: {}\nABI hash: {}\nHeight: {} ({}, {})\nStored artifact verified: {}\nReceipt: {}",
            receipt.commit.hash,
            receipt.network_id,
            receipt.authority,
            receipt.contract_address,
            receipt.contract_alias,
            receipt.code_hash,
            receipt.abi_hash,
            receipt.commit.block_height,
            receipt.commit.scope,
            receipt.commit.resolved_from,
            receipt.stored_artifact_matches,
            iroha_contract_deploy::receipt_path(journal).display()
        ),
        data: object([
            ("receipt", deployment_json(receipt)?),
            (
                "receipt_path",
                Value::from(
                    iroha_contract_deploy::receipt_path(journal)
                        .display()
                        .to_string(),
                ),
            ),
        ]),
    })
}

fn deployment_json<T: norito::json::JsonSerialize + ?Sized>(
    value: &T,
) -> Result<Value, Diagnostic> {
    norito::json::to_value(value).map_err(|_| {
        Diagnostic::new(
            ErrorCode::Internal,
            "deployment evidence could not be rendered",
        )
    })
}

fn deployment_diagnostic(error: DeploymentError) -> Diagnostic {
    if let DeploymentError::Failed(failure) = &error {
        let details = deployment_json(failure).unwrap_or(Value::Null);
        return Diagnostic::new(ErrorCode::Network, error.to_string())
            .with_details("The exact transaction is terminal. Correct the reported cause before creating a new deployment.", details);
    }
    let code = match &error {
        DeploymentError::Artifact(_) => ErrorCode::PackageInvalid,
        DeploymentError::InvalidRequest(_) => ErrorCode::Usage,
        DeploymentError::Journal(_) => ErrorCode::Io,
        DeploymentError::Preflight { .. }
        | DeploymentError::Pending { .. }
        | DeploymentError::Failed(_)
        | DeploymentError::Readback(_) => ErrorCode::Network,
    };
    let diagnostic = Diagnostic::new(code, error.to_string());
    match &error {
        DeploymentError::Preflight { source, .. }
        | DeploymentError::Pending { source, .. }
        | DeploymentError::Journal(source)
        | DeploymentError::Readback(source) => {
            diagnostic.with_context("cause", format!("{source:#}"))
        }
        _ => diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_rejects_replaced_artifact_bytes_before_signing() {
        let root = tempfile::tempdir().expect("build directory");
        let compiler = ivm::kotodama::compiler::Compiler::new();
        let original = compiler
            .compile_source("seiyaku Quote { view fn quote() -> int { return 30; } }")
            .expect("original contract");
        let replacement = compiler
            .compile_source("seiyaku Quote { view fn quote() -> int { return 31; } }")
            .expect("replacement contract");
        let verified = ivm::verify_contract_artifact(&original).expect("admitted original");
        let path = root.path().join("quote.to");
        fs::write(&path, &original).expect("build output");
        let mut artifact = CompilerArtifactV1 {
            package: "demo/quote".parse().expect("package"),
            target: "quote".to_owned(),
            source: "contracts/quote.ko".to_owned(),
            artifact: path.clone(),
            manifest: PathBuf::new(),
            interface: PathBuf::new(),
            entrypoints: vec!["quote".to_owned()],
            artifact_hash: verified.code_hash.to_string(),
            abi_hash: verified.abi_hash.to_string(),
            fresh: false,
        };
        assert_eq!(
            read_selected_artifact(&artifact).expect("exact build bytes"),
            original
        );
        fs::write(&path, replacement).expect("concurrent substitution");
        assert!(read_selected_artifact(&artifact).is_err());
        fs::write(&path, &original).expect("restore bytes");
        artifact.abi_hash = iroha::crypto::Hash::new(b"different ABI").to_string();
        assert!(read_selected_artifact(&artifact).is_err());
        fs::write(&path, b"invalid artifact").expect("corrupt artifact");
        assert!(read_selected_artifact(&artifact).is_err());
    }

    #[test]
    fn prepared_output_shows_exact_signer_alias_transactions_and_fee_bounds() {
        use iroha::crypto::{Algorithm, Hash, HashOf, KeyPair};
        use iroha_data_model::{
            NetworkId,
            account::AccountId,
            nexus::FeeDebitSource,
            permission::Permission,
            smart_contract::ContractAddress,
            transaction::{FeeChargeKind, FeeChargeLimit},
        };
        use iroha_model_base::topology::DataSpaceId;
        let _profile = ChainDiscriminantGuard::enter(369);
        let key = KeyPair::try_from_seed(vec![9; 32], Algorithm::Ed25519).expect("test key");
        let authority = AccountId::new(key.public_key().clone());
        let id =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"genesis")));
        let address =
            ContractAddress::derive(&id, &authority, 0, DataSpaceId::UNIVERSAL).expect("address");
        let fee = FeePaymentIntent::authority(
            vec![FeeChargeLimit {
                kind: FeeChargeKind::Nexus,
                asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().expect("asset"),
                max_amount: "2".parse().expect("fee"),
            }],
            None,
        );
        let quote = norito::json::from_value(object([
            ("intent", deployment_json(&fee).expect("intent")),
            (
                "observation",
                object([
                    ("ledger_time_ms", Value::from(1_u64)),
                    ("next_block_height", Value::from(2_u64)),
                    (
                        "route_dataspace_id",
                        deployment_json(&DataSpaceId::UNIVERSAL).expect("dataspace"),
                    ),
                ]),
            ),
            (
                "components",
                deployment_json(&fee.charge_limits().to_vec()).expect("components"),
            ),
            ("capacities", Value::Array(vec![])),
            (
                "decision",
                object([
                    ("status", Value::from("accepted")),
                    (
                        "value",
                        object([
                            (
                                "debit_source",
                                deployment_json(&FeeDebitSource::Account(authority.clone()))
                                    .expect("payer"),
                            ),
                            ("program_revision", Value::Null),
                        ]),
                    ),
                ]),
            ),
        ]))
        .expect("native quote");
        let token = Permission::new(
            "CanRegisterSmartContractCode".into(),
            iroha_primitives::json::Json::new(()),
        );
        let preflight = DeploymentPreflight {
            network_id: id,
            chain_id: "test-chain".to_owned(),
            authority: authority.clone(),
            authorization: iroha_contract_deploy::DeploymentAuthorization {
                account_exists: true,
                register_code_permission: token.clone(),
                manage_alias_permission: token,
            },
            chain_discriminant: 369,
            contract_alias: "coffee::universal".parse().expect("alias"),
            contract_address: address.clone(),
            dataspace_id: DataSpaceId::UNIVERSAL,
            code_hash: Hash::new(b"code"),
            abi_hash: Hash::new(b"abi"),
            deploy_nonce: 0,
            previous_contract_address: None,
            observed_block_height: 1,
            observed_block_hash: Hash::new(b"block").to_string(),
            fee_quotes: vec![quote],
            transaction_hashes: vec![Hash::new(b"transaction").to_string()],
        };
        let signer = authority.to_string();
        let _foreign_profile = ChainDiscriminantGuard::enter(753);
        let human = render_preflight(&preflight);
        for expected in [
            id.to_string(),
            signer,
            address.to_string(),
            "coffee::universal".to_owned(),
            "Transaction 1:".to_owned(),
            "at most 2 6TEAJqbb8oEPmLncoNiMRbLEK6tw".to_owned(),
        ] {
            assert!(human.contains(&expected), "missing {expected}: {human}");
        }
        assert_eq!(
            render_progress(DeploymentProgress::Prepared(Box::new(preflight.clone()))),
            format!("Deployment plan\n{}", human.trim_end())
        );
        let stage = iroha_contract_deploy::DeploymentStage {
            number: 2,
            total: 3,
            name: "register_manifest".to_owned(),
            hash: preflight.transaction_hashes[0].clone(),
        };
        let submitting = render_progress(DeploymentProgress::Submitting(stage.clone()));
        assert!(submitting.contains("Stage 2/3 — submitting register manifest"));
        assert!(submitting.contains(&stage.hash));
        assert!(!submitting.contains("Applied"));
        let recovering = render_progress(DeploymentProgress::Recovering(stage.clone()));
        assert!(recovering.contains("recovering register manifest by exact hash"));
        assert!(!recovering.contains("submitting"));
        let applied = render_progress(DeploymentProgress::Applied {
            stage,
            evidence: iroha_contract_deploy::AppliedEvidence {
                hash: preflight.transaction_hashes[0].clone(),
                terminal_kind: "Applied".to_owned(),
                block_height: 42,
                scope: "global".to_owned(),
                resolved_from: "state".to_owned(),
            },
        });
        assert!(applied.contains("Stage 2/3 — Applied at height 42 (global, state)"));
        let readback = render_progress(DeploymentProgress::ReadingBack {
            alias: preflight.contract_alias,
            address: preflight.contract_address,
        });
        assert!(readback.contains("Verifying stored artifact and alias readback"));
        assert!(readback.contains("coffee::universal"));
    }

    #[test]
    fn deployment_progress_is_explicit_human_output_and_json_remains_one_document() {
        let temporary = tempfile::tempdir().expect("isolated progress invocation");
        let missing = temporary.path().join("missing-Musubi.toml");
        for format in ["human", "json"] {
            let mut progress = Vec::new();
            let invocation = invoke_with_progress(
                [
                    "musubi",
                    "--manifest-path",
                    missing.to_str().expect("path"),
                    "--format",
                    format,
                    "deploy",
                ],
                &mut |message| progress.push(message.to_owned()),
            );
            let output = invocation
                .output
                .render(invocation.format)
                .expect("routed output");
            assert_ne!(
                output.exit_code(),
                0,
                "missing manifest prevents any deployment"
            );
            if format == "human" {
                assert_eq!(progress.len(), 1);
                assert!(progress[0].contains("Preparing the contract"));
            } else {
                assert!(progress.is_empty());
                assert!(output.stderr().is_empty());
                let _: Value = norito::json::from_str(output.stdout()).expect("one JSON document");
            }
        }
        let message = "Alias: private_key=secret-value::universal\n\u{1b}[31mAuthorization: Bearer hidden-token";
        for format in [OutputFormat::Human, OutputFormat::Json] {
            let mut progress = Vec::new();
            report_progress(format, message, &mut |text| progress.push(text.to_owned()));
            if format == OutputFormat::Human {
                assert_eq!(progress.len(), 1);
                assert!(progress[0].contains("[REDACTED]"));
                for forbidden in ["secret-value", "hidden-token", "\u{1b}"] {
                    assert!(
                        !progress[0].contains(forbidden),
                        "leaked {forbidden:?}: {}",
                        progress[0]
                    );
                }
            } else {
                assert!(progress.is_empty());
            }
        }
    }

    #[test]
    fn deployment_keeps_the_actionable_sdk_cause_and_redacts_credentials() {
        let error = DeploymentError::Preflight {
            operation: "fee quote",
            source: eyre::eyre!(
                "route_unavailable at authoritative peer; private_key=secret-value"
            )
            .wrap_err("Torii request failed"),
        };
        let diagnostic = deployment_diagnostic(error);
        for format in [OutputFormat::Human, OutputFormat::Json] {
            let rendered = CommandOutput::failure("deploy", diagnostic.clone())
                .render(format)
                .expect("diagnostic");
            let output = format!("{}{}", rendered.stdout(), rendered.stderr());
            assert!(output.contains("route_unavailable"));
            assert!(!output.contains("secret-value"));
            assert_ne!(rendered.exit_code(), 0);
        }
        let diagnostic = view_diagnostic(
            eyre::eyre!("route_unavailable; private_key=secret-value").wrap_err("view failed"),
        );
        let rendered = CommandOutput::failure("view", diagnostic)
            .render(OutputFormat::Json)
            .expect("view diagnostic");
        assert!(rendered.stdout().contains("route_unavailable"));
        assert!(!rendered.stdout().contains("secret-value"));
        assert_ne!(rendered.exit_code(), 0);
    }

    #[test]
    fn deployment_never_invents_a_fee_payer() {
        let dir = tempfile::tempdir().expect("workspace");
        let mut network = network::select_network(dir.path(), None, None, None).expect("network");
        assert!(selected_fee_payment(&network).is_err());
        let intent = FeePaymentIntent::authority(Vec::new(), None);
        network.fee_payment = Some(intent.clone());
        assert_eq!(
            selected_fee_payment(&network).expect("explicit selection"),
            intent
        );
    }

    #[test]
    fn deploy_rejects_offline_modes_and_ignored_resume_selection() {
        for flag in ["--offline", "--frozen", "--release"] {
            assert!(Cli::try_parse_from(["musubi", "deploy", flag]).is_err());
        }
        assert!(
            Cli::try_parse_from(["musubi", "deploy", "--resume", "journal", "--locked"]).is_err()
        );
        assert!(
            Cli::try_parse_from([
                "musubi",
                "deploy",
                "--resume",
                "journal",
                "--contract",
                "coffee"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "musubi",
                "deploy",
                "--resume",
                "journal",
                "--network",
                "taira"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from(["musubi", "deploy", "--contract", "coffee", "--locked"]).is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "musubi",
                "deploy",
                "--cancel",
                "journal",
                "--network",
                "taira"
            ])
            .is_ok()
        );
        for flag in ["--prepare", "--locked", "--workspace"] {
            assert!(
                Cli::try_parse_from(["musubi", "deploy", "--cancel", "journal", flag]).is_err()
            );
        }
        assert!(
            Cli::try_parse_from([
                "musubi", "deploy", "--cancel", "journal", "--resume", "journal"
            ])
            .is_err()
        );
    }

    #[test]
    fn view_arguments_require_a_bounded_named_object() {
        assert_eq!(
            parse_view_payload("{\"coffees\":\"3\"}")
                .expect("arguments")
                .get("coffees")
                .and_then(Value::as_str),
            Some("3")
        );
        for source in ["null", "[]", "\"secret\"", "{broken"] {
            assert!(parse_view_payload(source).is_err());
        }
        assert!(parse_view_payload(&" ".repeat(65 * 1024)).is_err());
    }

    #[test]
    fn journal_paths_and_ids_cannot_escape_the_package() {
        let root = Path::new("/workspace");
        assert_eq!(
            deployment_slot(
                root,
                "taira",
                &"demo/coffee".parse().expect("package"),
                "coffee-club"
            )
            .expect("slot"),
            root.join("target/deploy/taira")
                .join(blake3::hash(b"demo/coffee::coffee-club").to_hex().as_str())
        );
        for invalid in ["../outside", "", "a/b", ".", "a\\b"] {
            assert!(
                deployment_slot(
                    root,
                    "taira",
                    &"demo/coffee".parse().expect("package"),
                    invalid
                )
                .is_err()
            );
        }
        for target in ["coffee.v1", "coffee::v1", "\u{73c8}\u{7432}"] {
            let slot = deployment_slot(
                root,
                "taira",
                &"demo/coffee".parse().expect("package"),
                target,
            )
            .expect("canonical target");
            assert_eq!(
                slot.parent(),
                Some(root.join("target/deploy/taira").as_path())
            );
            assert_eq!(slot.file_name().expect("hash").to_string_lossy().len(), 64);
        }
        assert!(validate_journal_id(&"ab".repeat(32)).is_ok());
        assert!(validate_journal_id(&"AB".repeat(32)).is_err());
        for invalid in ["../outside", "bad", &"x".repeat(64)] {
            assert!(validate_journal_id(invalid).is_err());
        }
    }

    #[test]
    fn deployment_rejects_absent_or_ambiguous_targets() {
        assert!(select_artifact(&[], None).is_err());
        let artifact = CompilerArtifactV1 {
            package: "demo/coffee".parse().expect("package"),
            target: "coffee".to_owned(),
            source: String::new(),
            artifact: PathBuf::new(),
            manifest: PathBuf::new(),
            interface: PathBuf::new(),
            entrypoints: vec![],
            abi_hash: String::new(),
            artifact_hash: String::new(),
            fresh: false,
        };
        let artifacts = [artifact.clone(), artifact];
        assert!(select_artifact(&artifacts, Some("coffee")).is_err());
        assert!(select_artifact(&artifacts[..1], Some("other")).is_err());
        assert_eq!(
            select_artifact(&artifacts[..1], None)
                .expect("one contract")
                .target,
            "coffee"
        );
    }
}
