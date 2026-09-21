//! CLI parsing and rendering remain adapters around shared wallet operations.
use super::*;
use clap::Parser as _;
#[derive(clap::Parser)]
struct TestCli {
    #[command(subcommand)]
    command: AccountCommand,
}
#[test]
fn public_account_cli_requires_explicit_phase_and_independent_trust() {
    let prefix = [
        "account",
        "onboard",
        "prepare",
        "--alias",
        "coffee@universal",
        "--issuer",
        "operator",
        "--token-file",
        "/runtime/token",
        "--journal",
        "/runtime/coffee",
    ];
    let parsed = TestCli::try_parse_from(prefix).unwrap();
    let AccountCommand::Onboard(OnboardCommand::Prepare(args)) = parsed.command else {
        panic!("prepare variant");
    };
    assert_eq!(args.prepare.expires_in_secs, 120);
    assert!(args.prepare.request_id.is_none());
    assert!(TestCli::try_parse_from(["account", "onboard"]).is_err());
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "prepare",
            "--alias",
            "coffee@universal",
            "--journal",
            "/runtime/coffee"
        ])
        .is_err()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "resume",
            "--journal",
            "/runtime/coffee"
        ])
        .is_ok()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "submit",
            "--journal",
            "/runtime/coffee"
        ])
        .is_err()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "faucet",
            "prepare",
            "--journal",
            "/runtime/coffee",
            "--issuer",
            "operator",
            "--asset-definition",
            DEFAULT_GAS_ASSET_ID,
            "--amount",
            "10"
        ])
        .is_ok()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "faucet",
            "resume",
            "--journal",
            "/runtime/coffee",
            "--submit"
        ])
        .is_err()
    );
}

#[test]
fn token_selection_and_request_timeout_reject_invalid_inputs_before_io() {
    assert!(
        TokenArgs {
            token_file: None,
            token_fd: None
        }
        .read()
        .is_err()
    );
    assert!(
        TokenArgs {
            token_file: Some(PathBuf::from("unused")),
            token_fd: Some(3)
        }
        .read()
        .is_err()
    );
}
struct ReportContext {
    config: Config,
    format: CliOutputFormat,
    i18n: iroha_i18n::Localizer,
    lines: Vec<String>,
    documents: Vec<Value>,
}

impl ReportContext {
    fn new(config: Config, format: CliOutputFormat) -> Self {
        Self {
            config,
            format,
            i18n: iroha_i18n::Localizer::new(
                iroha_i18n::Bundle::Cli,
                iroha_i18n::Language::English,
            ),
            lines: Vec::new(),
            documents: Vec::new(),
        }
    }
}

impl RunContext for ReportContext {
    fn config(&self) -> &Config {
        &self.config
    }
    fn transaction_metadata(&self) -> Option<&Metadata> {
        None
    }
    fn input_instructions(&self) -> bool {
        false
    }
    fn output_instructions(&self) -> bool {
        false
    }
    fn i18n(&self) -> &iroha_i18n::Localizer {
        &self.i18n
    }
    fn output_format(&self) -> CliOutputFormat {
        self.format
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.documents.push(json::to_value(value)?);
        Ok(())
    }
    fn println(&mut self, value: impl std::fmt::Display) -> Result<()> {
        self.lines.push(value.to_string());
        Ok(())
    }
}

#[test]
fn command_rejects_another_chain_before_creating_or_opening_journals() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("never-created");
    let command = AccountCommand::Faucet(FaucetCommand::Resume(JournalArgs {
        journal: path.clone(),
        timeout_secs: 30,
    }));
    let mut context = ReportContext::new(crate::fallback_config(), CliOutputFormat::Json);
    context.config.account_chain_discriminant = 753;
    assert!(command.run(&mut context).is_err());
    assert!(!path.exists());
    assert!(context.documents.is_empty());
}

#[test]
fn rendering_preserves_native_review_fields_and_json_document() {
    let data = norito::json!({"operation":"onboarding","status":"Prepared","account_id":"account","network_id":"network","chain_discriminant":369,"expires_at_unix_ms":120000,"fee_payment":{},"transaction_hash":"hash","alias":"coffee@universal","issuer":"issuer","permissions_requested":[],"owner_auto_renew_follow_up":false,"journal":"/private/runtime/journal"});
    let report = OperationReport {
        status: OperationStatus::Prepared,
        data: data.clone(),
    };
    let mut text = ReportContext::new(crate::fallback_config(), CliOutputFormat::Text);
    render_report(&mut text, &report).unwrap();
    let rendered = text.lines.join("\n");
    for field in [
        "Prepared",
        "Transaction:",
        "Alias:",
        "Issuer:",
        "Journal:",
        "onboard submit",
        "Network: network",
        "Address profile: 369",
        "Expires at (Unix ms): 120000",
        "Fee intent: {}",
    ] {
        assert!(rendered.contains(field), "missing {field}");
    }
    let mut json = ReportContext::new(crate::fallback_config(), CliOutputFormat::Json);
    render_report(&mut json, &report).unwrap();
    assert_eq!(json.documents, vec![data]);
    assert!(json.lines.is_empty());
    for forbidden in [
        "private_key",
        "onboarding_token",
        "signed_transaction_wire_hex",
    ] {
        assert!(!rendered.contains(forbidden));
        assert!(
            !json::to_string(&json.documents[0])
                .unwrap()
                .contains(forbidden)
        );
    }
}
