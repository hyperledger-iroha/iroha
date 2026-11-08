//! Offline exact authority drafts and authenticated reads for the additive Parliament host.

use std::path::{Path, PathBuf};

use eyre::{Result, WrapErr, eyre};
use iroha_data_model::{
    NetworkId,
    governance::types::ProposalContentId,
    isi::{
        InstructionBox, framed_instruction_payload,
        sorafs_authority::{
            CreateSorafsAuthorityParliamentAttemptV1, ProposeSorafsAuthorityGovernanceV1,
            SorafsAuthorityProposalV1,
        },
    },
};
use norito::json::Value;

use super::shared::parse_governance_proposal_id_v1;
use crate::{Run, RunContext};

/// Draft or observe one exact operating-authority decision.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Build one canonical proposal instruction locally, without submitting it.
    DraftProposal(DraftProposalArgs),
    /// Build one shared-engine attempt instruction locally, without submitting it.
    DraftAttempt(DraftAttemptArgs),
    /// Read an exact hosted proposal and its current status.
    GetProposal(GetArgs),
    /// Read an exact authority subject, revision, and last successful receipt.
    GetHead(GetArgs),
    /// Read exact lease authorizations and actual installation or removal receipts.
    GetLease(GetArgs),
}

/// Exact proposal JSON input for a local instruction draft.
#[derive(clap::Args, Debug)]
pub struct DraftProposalArgs {
    /// File containing one strict `SorafsAuthorityProposalV1` JSON value.
    #[arg(long, value_name = "PATH")]
    pub proposal_json: PathBuf,
}

/// Exact proposal and retry sequence for a local attempt draft.
#[derive(clap::Args, Debug)]
pub struct DraftAttemptArgs {
    /// File containing one strict `SorafsAuthorityProposalV1` JSON value.
    #[arg(long, value_name = "PATH")]
    pub proposal_json: PathBuf,
    /// Zero-based attempt sequence for this exact proposal content.
    #[arg(long, default_value_t = 0)]
    pub attempt_sequence: u32,
}

/// Exact lowercase content, subject, or normalized lease plan identifier.
#[derive(clap::Args, Debug)]
pub struct GetArgs {
    /// Exactly 64 lowercase hexadecimal characters, without a prefix.
    #[arg(long, value_parser = parse_governance_proposal_id_v1)]
    pub id: String,
}

fn read_proposal(path: &Path, network: &NetworkId) -> Result<SorafsAuthorityProposalV1> {
    let bytes = std::fs::read(path)
        .wrap_err_with(|| format!("failed to read authority proposal `{}`", path.display()))?;
    let proposal: SorafsAuthorityProposalV1 = norito::json::from_slice(&bytes)
        .wrap_err("failed to decode strict SorafsAuthorityProposalV1 JSON")?;
    if &proposal.network != network {
        return Err(eyre!(
            "authority proposal network does not match the configured network"
        ));
    }
    if let Some(error) = proposal.invariant_error() {
        return Err(eyre!(error));
    }
    Ok(proposal)
}

fn draft(proposal: SorafsAuthorityProposalV1, sequence: Option<u32>) -> Result<Value> {
    if let Some(error) = proposal.invariant_error() {
        return Err(eyre!(error));
    }
    if sequence.is_some_and(|sequence| {
        sequence > iroha_data_model::governance::types::MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1
    }) {
        return Err(eyre!(
            "authority attempt sequence exceeds the Parliament retry limit"
        ));
    }
    let network = proposal.network;
    let content_id = proposal.proposal_content_id();
    let subject_id = proposal.governed_subject_id_v1();
    let effect_hash = proposal.effect_preimage_hash_v1();
    let plan_id = proposal.plan_id();
    let family_id = proposal.family_id();
    let term = proposal.term;
    let window = proposal
        .grid
        .window(term)
        .ok_or_else(|| eyre!("authority term grid is invalid or overflows"))?;
    let attempt = sequence.map(
        |attempt_sequence| CreateSorafsAuthorityParliamentAttemptV1 {
            proposal: proposal.clone(),
            attempt_sequence,
        },
    );
    let attempt_id = attempt
        .as_ref()
        .map(|instruction| instruction.governance_attempt_id().to_hex());
    let instruction: InstructionBox = match attempt {
        Some(instruction) => instruction.into(),
        None => ProposeSorafsAuthorityGovernanceV1 { proposal }.into(),
    };
    let (wire_id, payload) = framed_instruction_payload(&instruction)
        .ok_or_else(|| eyre!("authority instruction is not in the canonical wire registry"))?;
    let frame = norito::json::Map::from_iter([
        ("wire_id".into(), norito::json::to_value(&wire_id)?),
        (
            "payload_hex".into(),
            norito::json::to_value(&hex::encode(payload))?,
        ),
    ]);
    let window = norito::json::Map::from_iter([
        (
            "admission_height".into(),
            norito::json::to_value(&window.admission_height.to_string())?,
        ),
        (
            "cutoff_height".into(),
            norito::json::to_value(&window.cutoff_height.to_string())?,
        ),
        (
            "start_height".into(),
            norito::json::to_value(&window.start_height.to_string())?,
        ),
        (
            "end_height".into(),
            norito::json::to_value(&window.end_height.to_string())?,
        ),
    ]);
    let fields = [
        ("version", norito::json::to_value(&1_u16)?),
        ("network", norito::json::to_value(&network)?),
        ("plan_id", norito::json::to_value(&hex::encode(plan_id))?),
        (
            "family_id",
            norito::json::to_value(&hex::encode(family_id))?,
        ),
        ("term", norito::json::to_value(&term.to_string())?),
        ("term_window", Value::Object(window)),
        (
            "proposal_content_id",
            norito::json::to_value(&content_id.to_hex())?,
        ),
        (
            "subject_id",
            norito::json::to_value(&hex::encode(subject_id))?,
        ),
        (
            "effect_preimage_hash",
            norito::json::to_value(&hex::encode(effect_hash))?,
        ),
        (
            "governance_attempt_id",
            norito::json::to_value(&attempt_id)?,
        ),
        ("tx_instructions", Value::Array(vec![Value::Object(frame)])),
    ];
    Ok(Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    ))
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::DraftProposal(args) => {
                let proposal = read_proposal(&args.proposal_json, &context.config().network_id)?;
                context.print_data(&draft(proposal, None)?)
            }
            Self::DraftAttempt(args) => {
                let proposal = read_proposal(&args.proposal_json, &context.config().network_id)?;
                context.print_data(&draft(proposal, Some(args.attempt_sequence))?)
            }
            Self::GetProposal(args) => {
                let id = args
                    .id
                    .parse::<ProposalContentId>()
                    .map_err(|error| eyre!(error))?;
                let response = context
                    .client_from_config()?
                    .get_sorafs_authority_proposal(id)?;
                context.print_data(&response)
            }
            Self::GetHead(args) => {
                let mut subject = [0; 32];
                hex::decode_to_slice(args.id, &mut subject)?;
                let response = context
                    .client_from_config()?
                    .get_sorafs_authority_head(subject)?;
                context.print_data(&response)
            }
            Self::GetLease(args) => {
                let mut plan = [0; 32];
                hex::decode_to_slice(args.id, &mut plan)?;
                let response = context
                    .client_from_config()?
                    .get_sorafs_authority_lease(plan)?;
                context.print_data(&response)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha_data_model::{
        account::AccountId,
        isi::sorafs_authority::{SorafsAuthorityActionV1, SorafsOperatingAuthorityV1},
        runtime::lifecycle::RootScopeV1,
    };

    fn proposal() -> SorafsAuthorityProposalV1 {
        let config = crate::fallback_config();
        SorafsAuthorityProposalV1 {
            network: config.network_id,
            root: RootScopeV1::Universal,
            account: AccountId::new(config.key_pair.public_key().clone()),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: SorafsAuthorityActionV1::Grant,
            grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 10,
            },
            term: 1,
            expected_revision: 0,
            completion_policy: None,
        }
    }

    #[test]
    fn local_drafts_bind_exact_content_subject_and_retry() {
        let proposal = proposal();
        let excessive_sequence =
            iroha_data_model::governance::types::MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1 + 1;
        assert!(draft(proposal.clone(), Some(excessive_sequence)).is_err());
        let proposed = draft(proposal.clone(), None).unwrap();
        let attempted = draft(proposal.clone(), Some(0)).unwrap();
        let retried = draft(proposal.clone(), Some(1)).unwrap();
        for response in [&proposed, &attempted, &retried] {
            assert_eq!(
                response["proposal_content_id"].as_str(),
                Some(proposal.proposal_content_id().to_hex().as_str())
            );
            assert_eq!(
                response["subject_id"].as_str(),
                Some(hex::encode(proposal.governed_subject_id_v1()).as_str())
            );
            assert_eq!(response["tx_instructions"].as_array().unwrap().len(), 1);
        }
        assert!(proposed["governance_attempt_id"].is_null());
        assert_eq!(
            proposed["plan_id"].as_str(),
            Some(hex::encode(proposal.plan_id()).as_str())
        );
        assert_eq!(
            proposed["family_id"].as_str(),
            Some(hex::encode(proposal.family_id()).as_str())
        );
        assert_eq!(
            proposed["term_window"]["start_height"].as_str(),
            Some("641")
        );
        assert_eq!(proposed["term_window"]["end_height"].as_str(), Some("1281"));
        assert_eq!(
            proposed["term_window"]["cutoff_height"].as_str(),
            Some("631")
        );
        let mut terminal = proposal.clone();
        terminal.action = SorafsAuthorityActionV1::AuthorizeTerminalRevoke;
        let terminal = draft(terminal, None).unwrap();
        assert_eq!(terminal["plan_id"], proposed["plan_id"]);
        assert_eq!(terminal["family_id"], proposed["family_id"]);
        assert_ne!(
            terminal["proposal_content_id"],
            proposed["proposal_content_id"]
        );
        assert_ne!(
            attempted["governance_attempt_id"],
            retried["governance_attempt_id"]
        );
        assert_eq!(
            proposed["tx_instructions"][0]["wire_id"].as_str(),
            Some(ProposeSorafsAuthorityGovernanceV1::WIRE_ID)
        );
        assert_eq!(
            attempted["tx_instructions"][0]["wire_id"].as_str(),
            Some(CreateSorafsAuthorityParliamentAttemptV1::WIRE_ID)
        );
        let path = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(path.path(), norito::json::to_vec(&proposal).unwrap()).unwrap();
        assert_eq!(
            read_proposal(path.path(), &proposal.network).unwrap(),
            proposal
        );
        let mut invalid = proposal;
        invalid.root = RootScopeV1::DataSpace(iroha_model_base::topology::DataSpaceId::new(1));
        assert!(draft(invalid, None).is_err());
    }

    #[test]
    fn commands_are_registered_and_exact_ids_are_required() {
        #[derive(Parser)]
        struct Fixture {
            #[command(subcommand)]
            command: super::super::Command,
        }
        assert!(
            Fixture::try_parse_from([
                "gov",
                "sorafs-authority",
                "draft-attempt",
                "--proposal-json",
                "proposal.json",
                "--attempt-sequence",
                "1"
            ])
            .is_ok()
        );
        for command in ["get-proposal", "get-head", "get-lease"] {
            assert!(
                Fixture::try_parse_from([
                    "gov",
                    "sorafs-authority",
                    command,
                    "--id",
                    &"ab".repeat(32)
                ])
                .is_ok()
            );
            assert!(
                Fixture::try_parse_from([
                    "gov",
                    "sorafs-authority",
                    command,
                    "--id",
                    &"AB".repeat(32)
                ])
                .is_err()
            );
        }
    }
}
