//! Explicit immutable artifact preparation and exact native journal recovery.
use super::*;
use iroha_contract_deploy::{DeploymentRequest, DeploymentService, MAX_DEPLOYMENT_ARTIFACT_BYTES};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard, smart_contract::ContractAlias,
    transaction::FeePaymentIntent,
};

#[derive(Args, Debug)]
pub(super) struct ArtifactArgs {
    /// Explicit native client configuration; process environment cannot override it.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Fresh owner-private signed-plan journal for this exact deployment.
    #[arg(long, value_name = "DIRECTORY")]
    journal: PathBuf,
    #[command(subcommand)]
    action: ArtifactAction,
}

#[derive(Subcommand, Debug)]
enum ArtifactAction {
    /// Verify, quote, sign and persist exact native intents without dispatch.
    Prepare {
        /// Exact complete compiled artifact whose embedded manifest is authoritative.
        #[arg(long, value_name = "PATH")]
        artifact: PathBuf,
        /// Exact owner-approved contract alias and dataspace.
        #[arg(long)]
        alias: ContractAlias,
        /// Explicit native fee payment intent for every lifecycle transaction.
        #[arg(long, value_name = "PATH")]
        fee_payment: PathBuf,
    },
    /// Authenticate existing signed intents without signing, dispatch or journal changes.
    Inspect,
    /// Dispatch or recover only the exact retained signed plan.
    Resume,
}

pub(super) fn run_artifact(args: &ArtifactArgs, progress: &mut dyn FnMut(&str)) -> CommandResult {
    let config_bytes = read_bounded_single_link_regular_file_v1(&args.config, 1024 * 1024)
        .map_err(|_| artifact_error("native client configuration could not be read"))?;
    let (config, _) =
        iroha::config::Config::load_bytes_with_musubi_publication(&args.config, &config_bytes)
            .map_err(|_| {
                artifact_error("native client configuration could not be authenticated")
            })?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let service =
        DeploymentService::new(config).map_err(|error| artifact_error(&error.to_string()))?;
    match &args.action {
        ArtifactAction::Prepare {
            artifact,
            alias,
            fee_payment,
        } => {
            let artifact = read_bounded_single_link_regular_file_v1(
                artifact,
                MAX_DEPLOYMENT_ARTIFACT_BYTES as u64,
            )
            .map_err(|_| artifact_error("immutable contract artifact could not be read"))?;
            let fee_bytes = read_bounded_single_link_regular_file_v1(fee_payment, 64 * 1024)
                .map_err(|_| artifact_error("native fee intent could not be read"))?;
            let fee_payment: FeePaymentIntent =
                norito::json::from_slice(&fee_bytes).map_err(|_| {
                    artifact_error("fee payment does not use the current native schema")
                })?;
            let prepared = service
                .prepare(&DeploymentRequest {
                    artifact,
                    alias: alias.clone(),
                    fee_payment,
                    governance_approvers: Vec::new(),
                })
                .map_err(|error| artifact_error(&error.to_string()))?;
            service
                .persist(&prepared, &args.journal)
                .map_err(|error| artifact_error(&error.to_string()))?;
            Ok(Success {
                message:
                    "Prepared exact signed native deployment; retain its intents before resume."
                        .to_owned(),
                data: object([
                    ("journal", Value::from(args.journal.display().to_string())),
                    (
                        "preflight",
                        prepared
                            .preflight()
                            .to_json()
                            .map_err(|error| artifact_error(&error.to_string()))?,
                    ),
                    (
                        "intended_transactions",
                        norito::json::to_value(
                            &prepared
                                .intended_transactions()
                                .map_err(|error| artifact_error(&error.to_string()))?,
                        )
                        .map_err(|error| artifact_error(&error.to_string()))?,
                    ),
                ]),
            })
        }
        ArtifactAction::Inspect => Ok(Success {
            message: "Authenticated the exact retained native deployment intents.".to_owned(),
            data: object([
                ("journal", Value::from(args.journal.display().to_string())),
                (
                    "preflight",
                    service
                        .retained_preflight(&args.journal)
                        .map_err(|error| artifact_error(&error.to_string()))?
                        .to_json()
                        .map_err(|error| artifact_error(&error.to_string()))?,
                ),
                (
                    "intended_transactions",
                    norito::json::to_value(
                        &service
                            .retained_intended_transactions(&args.journal)
                            .map_err(|error| artifact_error(&error.to_string()))?,
                    )
                    .map_err(|error| artifact_error(&error.to_string()))?,
                ),
            ]),
        }),
        ArtifactAction::Resume => {
            let receipt = service
                .resume(&args.journal, &mut |event| progress(&format!("{event:?}")))
                .map_err(|error| artifact_error(&error.to_string()))?;
            Ok(Success {
                message: "Exact native deployment completed and its readback verified.".to_owned(),
                data: object([
                    ("journal", Value::from(args.journal.display().to_string())),
                    (
                        "receipt",
                        norito::json::to_value(&receipt)
                            .map_err(|error| artifact_error(&error.to_string()))?,
                    ),
                ]),
            })
        }
    }
}

fn artifact_error(message: &str) -> Diagnostic {
    Diagnostic::new(ErrorCode::Network, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_artifact_grammar_requires_config_journal_and_current_closed_inputs() {
        assert!(
            Cli::try_parse_from([
                "musubi",
                "artifact",
                "--config",
                "client.toml",
                "--journal",
                "owned",
                "prepare",
                "--artifact",
                "contract.to",
                "--alias",
                "contract::is",
                "--fee-payment",
                "fee.json"
            ])
            .is_ok()
        );
        for action in ["inspect", "resume"] {
            assert!(
                Cli::try_parse_from([
                    "musubi",
                    "artifact",
                    "--config",
                    "client.toml",
                    "--journal",
                    "owned",
                    action
                ])
                .is_ok()
            );
            assert!(Cli::try_parse_from(["musubi", "artifact", action]).is_err());
        }
        assert!(
            Cli::try_parse_from([
                "musubi",
                "artifact",
                "--config",
                "client.toml",
                "--journal",
                "owned",
                "resume",
                "--artifact",
                "replacement.to"
            ])
            .is_err()
        );
    }
}
