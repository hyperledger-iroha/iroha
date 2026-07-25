//! Provision one exact Taira fee sponsor program revision and isolated vault.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;
use eyre::{Context, Result, bail};
use iroha::{
    client::Client,
    config::{self, AnonymityPolicy, Config},
    crypto::{ExposedPrivateKey, KeyPair},
    data_model::{
        ChainId,
        account::{AccountAddress, AccountId},
        isi::{
            InstructionBox,
            nexus::{
                ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
                EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
            },
        },
        metadata::Metadata,
        nexus::{FeeSponsorProgram, FeeSponsorProgramRevision},
        transaction::FeePaymentIntent,
    },
};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_primitives::numeric::Quantity;
use toml::Value;
use url::Url;

#[derive(Debug, Parser)]
#[command(
    about = "Create, stage, fund, enroll, and activate an exact Taira fee sponsor program",
    version
)]
struct Args {
    #[arg(long, default_value = "https://taira.sora.org")]
    torii_url: Url,
    #[arg(long, default_value = "fc56984b-2be7-431d-840e-21514d1883f0")]
    chain_id: ChainId,
    #[arg(long, default_value_t = 369)]
    chain_discriminant: u16,
    /// Canonical Norito JSON document containing one immutable program revision.
    #[arg(long)]
    revision_json: PathBuf,
    /// Canonical Norito JSON selecting the authority or sponsor, revision, and gas bound.
    /// The command quotes and signs the exact recommended charge limits before submission.
    #[arg(long)]
    fee_payment_json: PathBuf,
    /// Asset amount transferred from the sponsor into the program-isolated vault.
    #[arg(long)]
    fund_amount: Quantity,
    /// Consensus height at which revision 1 becomes active.
    #[arg(long)]
    activate_at_height: u64,
    /// Exact beneficiary to enroll. May be repeated. The sponsor is enrolled when omitted.
    #[arg(long = "beneficiary", value_name = "I105")]
    beneficiaries: Vec<String>,
    /// Runtime-only client profile containing the sponsor signer. This command never modifies it.
    #[arg(long, default_value = "defaults/kagami/iroha3-taira/config.toml")]
    profile_config: PathBuf,
    #[arg(long, default_value_t = 600)]
    status_timeout_secs: u64,
}

fn table<'a>(value: &'a Value, key: &str) -> Result<&'a toml::value::Table> {
    value
        .get(key)
        .and_then(Value::as_table)
        .ok_or_else(|| eyre::eyre!("missing [{key}] table"))
}

fn string_at<'a>(table: &'a toml::value::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| eyre::eyre!("missing `{key}`"))
}

fn taira_profile_signer(path: &Path) -> Result<(String, String)> {
    let raw = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("read Taira profile {}", path.display()))?;
    let value = toml::from_str::<Value>(&raw).wrap_err("parse Taira profile TOML")?;
    let torii = table(&value, "torii")?;
    let onboarding_value = torii.get("account_onboarding").ok_or_else(|| {
        eyre::eyre!("missing structurally enabled [torii.account_onboarding] table")
    })?;
    let onboarding = onboarding_value
        .as_table()
        .ok_or_else(|| eyre::eyre!("invalid [torii.account_onboarding] table"))?;
    let authority = string_at(onboarding, "authority")?.to_owned();
    if onboarding.contains_key("private_key") {
        bail!("inline torii.account_onboarding.private_key is forbidden; use private_key_file");
    }
    let configured_key_path = PathBuf::from(string_at(onboarding, "private_key_file")?);
    let key_path = if configured_key_path.is_absolute() {
        configured_key_path
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(configured_key_path)
    };
    let metadata = std::fs::symlink_metadata(&key_path)
        .wrap_err_with(|| format!("inspect onboarding signer {}", key_path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("torii.account_onboarding.private_key_file must name a regular non-symlink file");
    }
    let raw_private_key = std::fs::read_to_string(&key_path)
        .wrap_err_with(|| format!("read onboarding signer {}", key_path.display()))?;
    let private_key = raw_private_key.trim_end_matches(['\r', '\n']);
    if private_key.is_empty()
        || private_key.trim() != private_key
        || private_key.chars().any(char::is_control)
    {
        bail!("onboarding signer file must contain one exact private key literal");
    }
    let private_key = private_key.to_owned();
    Ok((authority, private_key))
}

fn parse_taira_account(account: &str, discriminant: u16) -> Result<AccountId> {
    if account.contains('@') {
        bail!(
            "expected an encoded Taira account address, got unsupported account literal `{account}`"
        );
    }
    AccountAddress::parse_encoded(account, Some(discriminant))
        .and_then(|address| address.to_account_id())
        .wrap_err_with(|| format!("parse Taira account address `{account}`"))
}

fn default_alias_cache_policy() -> sorafs_manifest::alias_cache::AliasCachePolicy {
    sorafs_manifest::alias_cache::AliasCachePolicy::new(
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

fn read_norito_json<T>(path: &PathBuf, label: &str) -> Result<T>
where
    T: norito::json::JsonDeserialize,
{
    let bytes = std::fs::read(path).wrap_err_with(|| format!("read {label} {}", path.display()))?;
    norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("parse canonical {label} {}", path.display()))
}

fn provisioning_instructions(
    revision: FeeSponsorProgramRevision,
    beneficiaries: Vec<AccountId>,
    fund_amount: Quantity,
    activate_at_height: u64,
) -> Vec<InstructionBox> {
    let program_id = revision.program_id.clone();
    let asset_definition_id = revision.asset_budgets[0].asset_definition_id.clone();
    let revision_number = revision.revision;
    let mut instructions = vec![
        CreateFeeSponsorProgram {
            program: FeeSponsorProgram::new(program_id.clone()),
        }
        .into(),
        StageFeeSponsorProgramRevision { revision }.into(),
    ];
    instructions.extend(beneficiaries.into_iter().map(|beneficiary| {
        EnrollFeeSponsorBeneficiary {
            program_id: program_id.clone(),
            beneficiary,
        }
        .into()
    }));
    instructions.extend([
        FundFeeSponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id,
            amount: fund_amount,
        }
        .into(),
        ActivateFeeSponsorProgramRevision {
            program_id,
            revision: revision_number,
            activate_at_height,
        }
        .into(),
    ]);
    instructions
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.fund_amount.is_zero() {
        bail!("--fund-amount must be positive");
    }
    let (profile_account, profile_private_key) = taira_profile_signer(&args.profile_config)?;
    let profile_account = parse_taira_account(&profile_account, args.chain_discriminant)?;
    let private_key = profile_private_key
        .parse::<ExposedPrivateKey>()
        .wrap_err("parse profile private key")?
        .0;
    let key_pair = KeyPair::from_private_key(private_key).wrap_err("derive key pair")?;
    let signer = AccountId::new(key_pair.public_key().clone());
    if signer != profile_account {
        bail!(
            "profile signer account `{signer}` does not match profile authority `{profile_account}`"
        );
    }

    let revision: FeeSponsorProgramRevision =
        read_norito_json(&args.revision_json, "program revision JSON")?;
    revision
        .validate()
        .wrap_err("invalid fee sponsor program revision")?;
    if revision.program_id.sponsor != signer {
        bail!(
            "program sponsor `{}` does not match profile signer `{signer}`",
            revision.program_id.sponsor
        );
    }
    if revision.asset_budgets.len() != 1 {
        bail!(
            "Taira provisioning requires exactly one fee-asset budget; found {}",
            revision.asset_budgets.len()
        );
    }
    let fee_payment: FeePaymentIntent =
        read_norito_json(&args.fee_payment_json, "fee payment intent JSON")?;
    fee_payment
        .validate()
        .wrap_err("invalid fee payment intent")?;

    let beneficiaries = if args.beneficiaries.is_empty() {
        vec![signer.clone()]
    } else {
        args.beneficiaries
            .iter()
            .map(|literal| parse_taira_account(literal, args.chain_discriminant))
            .collect::<Result<Vec<_>>>()?
    };
    let instructions = provisioning_instructions(
        revision,
        beneficiaries,
        args.fund_amount,
        args.activate_at_height,
    );

    let client = Client::new(Config {
        chain: args.chain_id,
        account: signer,
        account_chain_discriminant: args.chain_discriminant,
        key_pair,
        basic_auth: None,
        torii_api_url: args.torii_url,
        torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: Duration::from_secs(900),
        transaction_status_timeout: Duration::from_secs(args.status_timeout_secs),
        transaction_add_nonce: true,
        connect_queue_root: config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_cache_policy(),
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::default(),
    });

    let mut payload = client.try_build_transaction_payload_from_items(
        instructions,
        fee_payment.clone(),
        Metadata::default(),
    )?;
    let quote = client.quote_fees(&payload)?;
    if !fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        bail!(
            "fee quote changed the selected payer, sponsor revision, or gas bound; refusing to sign"
        );
    }
    payload.fee_payment = quote.intent.clone();
    let transaction = client.try_sign_transaction_payload(payload)?;
    let hash = client.submit_transaction_blocking(&transaction)?;
    let receipt = norito::json!({
        "hash": hash,
        "transaction": transaction,
        "fee_quote": quote,
    });
    println!("{}", norito::json::to_json_pretty(&receipt)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, num::NonZeroU64, str::FromStr};

    use iroha::{
        crypto::{Algorithm, KeyPair},
        data_model::{
            asset::AssetDefinitionId,
            domain::DomainId,
            name::Name,
            nexus::{
                FeeSponsorAssetBudget, FeeSponsorEligibility, FeeSponsorNativeInstructionSelector,
                FeeSponsorProgramId, FeeSponsorRule, FeeSponsorRuleEffect, FeeSponsorRuleSelector,
            },
        },
    };

    use super::*;

    fn sample_revision() -> FeeSponsorProgramRevision {
        let sponsor = AccountId::new(
            KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519)
                .expect("key pair")
                .public_key()
                .clone(),
        );
        let program_id = FeeSponsorProgramId::new(sponsor, Name::from_str("default").unwrap());
        FeeSponsorProgramRevision {
            program_id,
            revision: 1,
            eligibility: FeeSponsorEligibility::EnrolledOrRouteDefault,
            rules: vec![FeeSponsorRule {
                id: Name::from_str("deploy").unwrap(),
                effect: FeeSponsorRuleEffect::Allow,
                selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                    FeeSponsorNativeInstructionSelector {
                        wire_id: "nexus::EnrollFeeSponsorBeneficiary".to_owned(),
                        asset_definition_id: None,
                    },
                )],
            }],
            asset_budgets: vec![FeeSponsorAssetBudget {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    Name::from_str("rose").expect("asset name"),
                ),
                per_transaction: Quantity::from(10_u64),
                per_block: Quantity::from(100_u64),
                per_program_epoch: Quantity::from(1_000_u64),
                per_beneficiary_epoch: Quantity::from(100_u64),
                reserve_floor: Quantity::from(10_u64),
                epoch_length_blocks: NonZeroU64::new(100).unwrap(),
            }],
        }
    }

    #[test]
    fn fee_quote_selection_rejects_payer_gas_and_revision_substitution() {
        let authority = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(10));
        assert!(authority.has_same_payer_and_gas_bound(&authority));
        assert!(
            !authority.has_same_payer_and_gas_bound(&FeePaymentIntent::authority(
                Vec::new(),
                NonZeroU64::new(11)
            ))
        );

        let program_id = sample_revision().program_id;
        let sponsor =
            FeePaymentIntent::sponsor(program_id.clone(), 1, Vec::new(), NonZeroU64::new(10));
        assert!(!authority.has_same_payer_and_gas_bound(&sponsor));
        assert!(
            !sponsor.has_same_payer_and_gas_bound(&FeePaymentIntent::sponsor(
                program_id,
                2,
                Vec::new(),
                NonZeroU64::new(10)
            ))
        );
    }

    #[test]
    fn provisioning_order_is_create_stage_enroll_fund_activate() {
        let revision = sample_revision();
        let beneficiary = revision.program_id.sponsor.clone();
        let instructions =
            provisioning_instructions(revision, vec![beneficiary], Quantity::from(100_u64), 42);
        let wire_ids = instructions
            .iter()
            .map(|instruction| {
                iroha::data_model::isi::instruction_wire_id(instruction)
                    .expect("registered wire id")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            wire_ids,
            [
                "nexus::CreateFeeSponsorProgram",
                "nexus::StageFeeSponsorProgramRevision",
                "nexus::EnrollFeeSponsorBeneficiary",
                "nexus::FundFeeSponsorProgram",
                "nexus::ActivateFeeSponsorProgramRevision",
            ]
        );
    }

    #[test]
    fn profile_signer_uses_structural_file_backed_onboarding() {
        let directory = tempfile::tempdir().expect("temporary profile directory");
        let signer_path = directory.path().join("onboarding-signer.key");
        fs::write(&signer_path, "private-key-literal\n").expect("write signer sidecar");
        let profile_path = directory.path().join("peer.toml");
        fs::write(
            &profile_path,
            r#"
[torii.account_onboarding]
authority = "canonical-account"
private_key_file = "onboarding-signer.key"
"#,
        )
        .expect("write structural profile");

        assert_eq!(
            taira_profile_signer(&profile_path).expect("read file-backed signer"),
            (
                "canonical-account".to_owned(),
                "private-key-literal".to_owned()
            )
        );
    }

    #[test]
    fn profile_signer_rejects_legacy_inline_private_key() {
        let directory = tempfile::tempdir().expect("temporary profile directory");
        let profile_path = directory.path().join("peer.toml");
        fs::write(
            &profile_path,
            r#"
[torii.account_onboarding]
authority = "canonical-account"
private_key = "must-not-be-read"
private_key_file = "unused.key"
"#,
        )
        .expect("write legacy profile");

        let error = taira_profile_signer(&profile_path)
            .expect_err("inline private key must be rejected")
            .to_string();
        assert!(error.contains("inline"));
        assert!(!error.contains("must-not-be-read"));
    }
}
