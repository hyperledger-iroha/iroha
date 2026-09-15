//! Package build and test presentation with explicit network context and complete diagnostics.
use super::*;
use crate::{compiler::CompilerArtifactV1, test_runner::WorkspaceTestReportV1};

pub(super) fn run_build(
    explicit_manifest: Option<&Path>,
    command: &'static str,
    args: &BuildArgs,
) -> CommandResult {
    let PreparedBuild {
        workspace,
        selected_names,
        network,
        graph,
        cache,
        archives,
        execution,
    } = prepare_build(
        explicit_manifest,
        args,
        if command == "build" {
            CompilerActionV1::Build
        } else {
            CompilerActionV1::Check
        },
    )?;
    let chain_discriminant = graph.account_chain_discriminant();
    let mut data = Map::from_iter([
        ("network".to_owned(), network.json()),
        (
            "validated_packages".to_owned(),
            Value::from(execution.validated_packages as u64),
        ),
        (
            "contract_targets".to_owned(),
            Value::from(execution.contract_targets as u64),
        ),
        (
            "warnings".to_owned(),
            Value::from(execution.warnings as u64),
        ),
        ("archives".to_owned(), Value::Array(archives)),
        ("lock".to_owned(), lockfile_json(&graph.lock)),
    ]);
    let mut human = format!(
        "Network: {} (address profile {chain_discriminant})\n",
        network.name
    );
    if let Some(id) = network.network_id {
        let _ = writeln!(human, "Network identity: {id}");
    } else {
        human.push_str("Local compilation; deployment network is not configured.\n");
    }
    if command == "test" {
        let report = execute_workspace_tests_v1(
            cache.as_ref(),
            &workspace,
            &selected_names,
            &graph.lock,
            &WorkspaceTestOptionsV1::new(chain_discriminant),
        )
        .map_err(|error| graph_mode_test_diagnostic(&error, args.mode))?;
        let (summary, tests) = render_test_report(&report);
        human.push_str(&summary);
        data.extend(tests);
        if !report.is_success() {
            return Err(
                Diagnostic::new(ErrorCode::Compiler, "Kotodama tests failed")
                    .with_details(human, Value::Object(data)),
            );
        }
    } else {
        let _ = writeln!(
            human,
            "{command} completed: {} package(s), {} contract target(s), {} warning(s)",
            execution.validated_packages, execution.contract_targets, execution.warnings
        );
        for artifact in &execution.artifacts {
            human.push_str(&render_artifact(artifact));
        }
        if let [artifact] = execution.artifacts.as_slice() {
            human.push_str(&deployment_next_step(
                workspace.root_manifest_path(),
                &network,
                artifact,
            ));
        }
        data.insert(
            "artifacts".to_owned(),
            Value::Array(execution.artifacts.iter().map(artifact_json).collect()),
        );
        data.insert(
            "interfaces".to_owned(),
            Value::Array(
                execution
                    .package_interfaces
                    .iter()
                    .map(|interface| {
                        object([
                            ("package", Value::from(interface.package.to_string())),
                            (
                                "digest",
                                Value::from(hex::encode(interface.digest.as_bytes())),
                            ),
                        ])
                    })
                    .collect(),
            ),
        );
    }
    Ok(Success {
        message: human,
        data: Value::Object(data),
    })
}

fn deployment_next_step(
    manifest: &Path,
    network: &network::SelectedNetwork,
    artifact: &CompilerArtifactV1,
) -> String {
    let key = network::contract_key(&artifact.package, &artifact.target);
    let invocation = format!(
        "musubi --manifest-path {}",
        quote_cli_argument(&manifest.display().to_string())
    );
    let client = network.config.as_ref().map(|path| {
        format!(
            " --config {}",
            quote_cli_argument(&path.display().to_string())
        )
    });
    if network.network_id.is_some()
        && network.fee_payment.is_some()
        && network.contracts.contains_key(&key)
    {
        format!(
            "Next: {invocation} deploy --network {}{} --package {} --contract {}\n",
            network.name,
            client.unwrap_or_default(),
            quote_cli_argument(&artifact.package.to_string()),
            quote_cli_argument(&artifact.target)
        )
    } else {
        let (setup, client) = match client {
            Some(client) => (
                "Bind an alias in a domain owned by the selected account; the existing fee policy is retained.\n".to_owned(),
                client,
            ),
            None if network.name == "taira" => (
                "For a new Taira account, run `musubi wallet create`, `musubi wallet fund`, and `musubi wallet namespace <your-domain>`.\n".to_owned(),
                " --wallet default".to_owned(),
            ),
            None => (
                format!("Select a funded wallet for {} and acquire a domain for that account.\n", network.name),
                " --wallet <wallet-for-network>".to_owned(),
            ),
        };
        format!(
            "{setup}Next: {invocation} network configure {}{client} --package {} --contract {} --alias <name::your-domain>\n",
            network.name,
            quote_cli_argument(&artifact.package.to_string()),
            quote_cli_argument(&artifact.target)
        )
    }
}

pub(super) struct PreparedBuild {
    pub(super) workspace: Workspace,
    pub(super) selected_names: Vec<MusubiPackageSelectorV1>,
    pub(super) network: network::SelectedNetwork,
    pub(super) graph: ResolvedWorkspaceGraphV1,
    pub(super) cache: Option<MusubiCache>,
    pub(super) archives: Vec<Value>,
    pub(super) execution: crate::compiler::CompilerExecutionV1,
}

pub(super) fn prepare_build(
    explicit_manifest: Option<&Path>,
    args: &BuildArgs,
    action: CompilerActionV1,
) -> Result<PreparedBuild, Diagnostic> {
    let (workspace, selected_names) = load_selected_workspace(explicit_manifest, &args.selection)?;
    let network = network::select_network(
        workspace.root(),
        args.network.as_deref(),
        args.registry.config.as_deref(),
        args.chain_discriminant,
    )?;
    let previous = read_optional_workspace_lock(&workspace)?;
    let graph = resolve_and_persist_graph(
        &workspace,
        &selected_names,
        previous,
        None,
        WorkspaceResolutionOptionsV1 {
            config_image: network.config_image.clone(),
            expected_network_id: network.network_id,
            mode: args.mode,
            config: network.config.as_deref(),
            fresh_only: false,
            purpose: GraphPurposeV1::Workspace,
            requested_chain_discriminant: Some(network.chain_discriminant),
        },
    )?;
    let cache = if graph.lock.nodes.is_empty() {
        None
    } else {
        Some(open_user_cache()?)
    };
    let archives = match &cache {
        Some(cache) => ensure_graph_archives(cache, &graph, args.mode)?,
        None => Vec::new(),
    };
    let chain_discriminant = graph.account_chain_discriminant();
    let execution = execute_compiler_graph(
        cache.as_ref(),
        &workspace,
        &selected_names,
        &graph.lock,
        action,
        chain_discriminant,
    )
    .map_err(|error| graph_mode_compiler_diagnostic(&error, args.mode))?;
    Ok(PreparedBuild {
        workspace,
        selected_names,
        network,
        graph,
        cache,
        archives,
        execution,
    })
}

fn artifact_json(artifact: &CompilerArtifactV1) -> Value {
    object([
        ("package", Value::from(artifact.package.to_string())),
        ("target", Value::from(artifact.target.clone())),
        ("source", Value::from(artifact.source.clone())),
        (
            "artifact",
            Value::from(artifact.artifact.display().to_string()),
        ),
        (
            "manifest",
            Value::from(artifact.manifest.display().to_string()),
        ),
        (
            "interface",
            Value::from(artifact.interface.display().to_string()),
        ),
        ("code_hash", Value::from(artifact.artifact_hash.clone())),
        ("abi_hash", Value::from(artifact.abi_hash.clone())),
        (
            "entrypoints",
            Value::Array(
                artifact
                    .entrypoints
                    .iter()
                    .cloned()
                    .map(Value::from)
                    .collect(),
            ),
        ),
        ("fresh", Value::from(artifact.fresh)),
    ])
}

fn render_artifact(artifact: &CompilerArtifactV1) -> String {
    format!(
        "{} {}::{}\n  bytecode: {}\n  manifest: {}\n  interface: {}\n  code hash: {}\n  ABI hash: {}\n  entrypoints: {}\n",
        if artifact.fresh { "Fresh" } else { "Built" },
        artifact.package,
        artifact.target,
        artifact.artifact.display(),
        artifact.manifest.display(),
        artifact.interface.display(),
        artifact.artifact_hash,
        artifact.abi_hash,
        artifact.entrypoints.join(", ")
    )
}

fn render_test_report(report: &WorkspaceTestReportV1) -> (String, Map) {
    let mut human = String::new();
    let targets = report
        .targets
        .iter()
        .map(|target| {
            let _ = writeln!(
                human,
                "{}::{} ({})",
                target.package, target.target, target.source
            );
            let cases = target
                .report
                .cases
                .iter()
                .map(|case| {
                    let _ = writeln!(
                        human,
                        "  {} {} (line {})",
                        if case.passed { "PASS" } else { "FAIL" },
                        case.name,
                        case.line
                    );
                    if let Some(failure) = &case.failure {
                        for line in failure.lines() {
                            let _ = writeln!(human, "    {line}");
                        }
                    }
                    object([
                        ("name", Value::from(case.name.clone())),
                        ("line", Value::from(u64::from(case.line))),
                        ("passed", Value::from(case.passed)),
                        (
                            "failure",
                            case.failure
                                .as_ref()
                                .map_or(Value::Null, |failure| Value::from(failure.clone())),
                        ),
                    ])
                })
                .collect();
            object([
                ("package", Value::from(target.package.to_string())),
                ("target", Value::from(target.target.clone())),
                ("source", Value::from(target.source.clone())),
                ("cases", Value::Array(cases)),
            ])
        })
        .collect();
    let _ = writeln!(
        human,
        "test completed: {} passed; {} failed",
        report.passed(),
        report.failed()
    );
    (
        human,
        Map::from_iter([
            ("passed".to_owned(), Value::from(report.passed() as u64)),
            ("failed".to_owned(), Value::from(report.failed() as u64)),
            ("targets".to_owned(), Value::Array(targets)),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copyable_commands_quote_shell_syntax_in_package_paths_and_target_names() {
        for (input, expected) in [
            ("demo/coffee::target", "demo/coffee::target"),
            ("a b", "'a b'"),
            ("quote'one", "'quote'\"'\"'one'"),
            ("quote;echo", "'quote;echo'"),
            ("$(echo)", "'$(echo)'"),
            ("", "''"),
        ] {
            assert_eq!(quote_cli_argument(input), expected);
        }
    }
    use crate::test_runner::WorkspaceTestTargetReportV1;
    use ivm::koto_test_driver::{KotoTestCaseOutcomeV1, KotoTestRunReportV1};

    #[test]
    fn all_named_test_failures_retain_source_line_and_vm_reason() {
        let report = WorkspaceTestReportV1 {
            targets: vec![WorkspaceTestTargetReportV1 {
                package: "demo/coffee-club".parse().expect("package"),
                target: "quote".to_owned(),
                source: "tests/quote.test.ko".to_owned(),
                report: KotoTestRunReportV1 {
                    target: PathBuf::from("tests/quote.test.ko"),
                    seed: 0,
                    cases: vec![
                        KotoTestCaseOutcomeV1 {
                            name: "first".to_owned(),
                            line: 3,
                            passed: false,
                            failure: Some("actual 29; expected 30".to_owned()),
                        },
                        KotoTestCaseOutcomeV1 {
                            name: "second".to_owned(),
                            line: 8,
                            passed: false,
                            failure: Some("VM gas exhausted".to_owned()),
                        },
                        KotoTestCaseOutcomeV1 {
                            name: "third".to_owned(),
                            line: 13,
                            passed: true,
                            failure: None,
                        },
                    ],
                },
            }],
        };
        let (human, json) = render_test_report(&report);
        for detail in [
            "FAIL first (line 3)",
            "actual 29; expected 30",
            "FAIL second (line 8)",
            "VM gas exhausted",
            "PASS third",
            "1 passed; 2 failed",
            "tests/quote.test.ko",
        ] {
            assert!(human.contains(detail), "missing {detail}");
        }
        let json = Value::Object(json);
        assert_eq!(
            json.pointer("/targets/0/cases/1/failure")
                .and_then(Value::as_str),
            Some("VM gas exhausted")
        );
        assert_eq!(
            json.pointer("/targets/0/cases/2/failure"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn artifact_output_exposes_verified_deployment_inputs_and_freshness() {
        let artifact = CompilerArtifactV1 {
            package: "demo/coffee-club".parse().expect("package"),
            target: "coffee-club".to_owned(),
            source: "contracts/coffee-club.ko".to_owned(),
            artifact: PathBuf::from("target/coffee-club.to"),
            manifest: PathBuf::from("target/coffee-club.manifest.json"),
            interface: PathBuf::from("target/coffee-club.interface.json"),
            artifact_hash: "code-digest".to_owned(),
            abi_hash: "abi-digest".to_owned(),
            entrypoints: vec!["quote".to_owned()],
            fresh: true,
        };
        let human = render_artifact(&artifact);
        for detail in [
            "Fresh",
            "coffee-club.to",
            "coffee-club.manifest.json",
            "coffee-club.interface.json",
            "code-digest",
            "abi-digest",
            "quote",
        ] {
            assert!(human.contains(detail));
        }
        let json = artifact_json(&artifact);
        assert_eq!(
            json.get("code_hash").and_then(Value::as_str),
            Some("code-digest")
        );
        assert_eq!(
            json.pointer("/entrypoints/0").and_then(Value::as_str),
            Some("quote")
        );
        let dir = tempfile::tempdir().expect("workspace");
        let mut network = network::select_network(dir.path(), None, None, None).expect("network");
        let manifest = dir.path().join("Musubi.toml");
        let unbound = deployment_next_step(&manifest, &network, &artifact);
        for expected in [
            "musubi wallet create",
            "musubi wallet fund",
            "musubi wallet namespace <your-domain>",
            "network configure taira --wallet default",
            "--alias <name::your-domain>",
        ] {
            assert!(unbound.contains(expected), "missing {expected}: {unbound}");
        }
        assert!(!unbound.contains("<client.toml>"));
        assert!(!unbound.contains("--fee-payer"));
        network.name = "custom".to_owned();
        let custom = deployment_next_step(&manifest, &network, &artifact);
        assert!(custom.contains("--wallet <wallet-for-network>"));
        assert!(!custom.contains("musubi wallet fund"));
        network.name = "taira".to_owned();
        network.network_id = Some(iroha_data_model::NetworkId::from_genesis_hash(
            iroha::crypto::HashOf::from_untyped_unchecked(iroha::crypto::Hash::new(
                b"test genesis",
            )),
        ));
        network.fee_payment = Some(iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            None,
        ));
        network.config = Some(PathBuf::from("/runtime/selected wallet/client.toml"));
        let missing_alias = deployment_next_step(&manifest, &network, &artifact);
        assert!(missing_alias.contains("--config '/runtime/selected wallet/client.toml'"));
        assert!(missing_alias.contains("existing fee policy is retained"));
        assert!(!missing_alias.contains("--wallet default"));
        assert!(!missing_alias.contains("--fee-payer"));
        network.contracts.insert(
            network::contract_key(&artifact.package, &artifact.target),
            "coffee-club::universal".parse().expect("alias"),
        );
        assert_eq!(
            deployment_next_step(&manifest, &network, &artifact),
            format!(
                "Next: musubi --manifest-path {} deploy --network taira --config '/runtime/selected wallet/client.toml' --package demo/coffee-club --contract coffee-club\n",
                quote_cli_argument(&manifest.display().to_string())
            )
        );
    }
}
