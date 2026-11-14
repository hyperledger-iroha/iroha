//! Anonymous Parliament ballot commands: account- and session-free submission and the two
//! public whole-record reads (`specs/parliament_private_ballot_design.md` sections 6 and 12.3,
//! slice 5).
//!
//! `submit` sends one canonical envelope frame that a wallet produced; this command proves and
//! stamps nothing. The request carries none of the configured client credentials (see
//! `iroha::client::Client::submit_anonymous_ballot`). Network-origin hiding needs a qualified
//! anonymity transport, which the CLI does not provide.

use super::shared::print_with_summary;
use crate::{Run, RunContext};
use eyre::{Result, WrapErr};
use iroha::{
    client::Client,
    data_model::governance::{
        anonymous_ballot::{
            BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1, ParliamentBallotBoardRecordV1,
        },
        types::GovernanceAttemptId,
    },
};

fn parse_scope(input: &str) -> Result<BallotScopeV1, String> {
    let mut bytes = [0_u8; 64];
    if input.len() != 128
        || !input
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        || hex::decode_to_slice(input, &mut bytes).is_err()
    {
        return Err(
            "must be exactly 128 lowercase hexadecimal characters without a prefix".to_owned(),
        );
    }
    Ok(BallotScopeV1::new(bytes))
}

fn parse_attempt_id(input: &str) -> Result<GovernanceAttemptId, String> {
    input.parse::<GovernanceAttemptId>().map_err(|_| {
        "must be exactly 64 lowercase hexadecimal characters without a prefix".to_owned()
    })
}

/// Submit one canonical anonymous-ballot envelope frame produced by a wallet.
#[derive(clap::Args, Debug)]
pub struct SubmitArgs {
    /// File holding exactly one canonical envelope frame (raw bytes).
    #[arg(long, value_name = "PATH")]
    pub envelope: std::path::PathBuf,
}

impl Run for SubmitArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = std::fs::read(&self.envelope).wrap_err_with(|| {
            format!(
                "failed to read anonymous-ballot envelope `{}`",
                self.envelope.display()
            )
        })?;
        let envelope = CanonicalAnonymousBallotEnvelopeV1::from_canonical_bytes(bytes)
            .wrap_err("the file is not one canonical anonymous-ballot envelope frame")?;
        let client: Client = context.client_from_config()?;
        client.submit_anonymous_ballot(&envelope)?;
        print_with_summary(
            context,
            Some(
                "handled: confirm acceptance from the scope's corpus (anonymous-ballot board)"
                    .to_owned(),
            ),
            &norito::json!({ "handled": true }),
        )
    }
}

/// Read the public board of one ballot scope and write its canonical Norito bytes.
#[derive(clap::Args, Debug)]
pub struct BoardArgs {
    /// Ballot scope: 128 lowercase hexadecimal characters.
    #[arg(long, value_parser = parse_scope)]
    pub scope: BallotScopeV1,
    /// File to write the board record's canonical Norito bytes to.
    #[arg(long, value_name = "PATH")]
    pub output: std::path::PathBuf,
}

impl Run for BoardArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config()?;
        let bytes = client.anonymous_ballot_board(&self.scope)?;
        let board: ParliamentBallotBoardRecordV1 = norito::decode_from_bytes(&bytes)
            .wrap_err("the node returned bytes that are not one board record")?;
        std::fs::write(&self.output, &bytes)
            .wrap_err_with(|| format!("failed to write `{}`", self.output.display()))?;
        let summary = format!(
            "corpus_entries={} opened={} closed={} bytes={}",
            board.entries.len(),
            board.opening.is_some(),
            board.closure.is_some(),
            bytes.len()
        );
        print_with_summary(
            context,
            Some(summary),
            &norito::json!({
                "corpus_entries": board.entries.len(),
                "opened": board.opening.is_some(),
                "closed": board.closure.is_some(),
                "bytes": bytes.len(),
                "output": self.output.display().to_string(),
            }),
        )
    }
}

/// Read one anonymous Parliament attempt and write its canonical Norito bytes.
#[derive(clap::Args, Debug)]
pub struct AttemptArgs {
    /// Governance attempt identifier: 64 lowercase hexadecimal characters.
    #[arg(long, value_parser = parse_attempt_id)]
    pub governance_attempt_id: GovernanceAttemptId,
    /// File to write the attempt state's canonical Norito bytes to.
    #[arg(long, value_name = "PATH")]
    pub output: std::path::PathBuf,
}

impl Run for AttemptArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config()?;
        let bytes = client.anonymous_parliament_attempt(&self.governance_attempt_id)?;
        std::fs::write(&self.output, &bytes)
            .wrap_err_with(|| format!("failed to write `{}`", self.output.display()))?;
        print_with_summary(
            context,
            Some(format!("bytes={}", bytes.len())),
            &norito::json!({
                "bytes": bytes.len(),
                "output": self.output.display().to_string(),
            }),
        )
    }
}

/// Anonymous Parliament ballot commands.
#[derive(clap::Subcommand, Debug)]
pub enum AnonymousBallotCommand {
    /// Submit one canonical envelope frame, without any client credential.
    Submit(SubmitArgs),
    /// Read the public board (sealed context, casting window and corpus) of one scope.
    Board(BoardArgs),
    /// Read one attempt (elections, assignments, bodies and schedule) of the family.
    Attempt(AttemptArgs),
}

impl Run for AnonymousBallotCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Submit(args) => args.run(context),
            Self::Board(args) => args.run(context),
            Self::Attempt(args) => args.run(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_exact_lowercase_hexadecimal() {
        assert_eq!(
            parse_scope(&"ab".repeat(64)).expect("a scope").as_bytes(),
            &[0xab; 64]
        );
        for invalid in [
            "AB".repeat(64),
            format!("0x{}", "ab".repeat(63)),
            "ab".repeat(63),
            "zz".repeat(64),
        ] {
            assert!(parse_scope(&invalid).is_err(), "{invalid}");
        }
        assert!(parse_attempt_id(&"5a".repeat(32)).is_ok());
        assert!(parse_attempt_id(&"5A".repeat(32)).is_err());
    }

    #[test]
    fn subcommands_parse_their_arguments() {
        use clap::Parser as _;
        #[derive(clap::Parser, Debug)]
        struct Cli {
            #[command(subcommand)]
            command: AnonymousBallotCommand,
        }
        let parsed = Cli::try_parse_from([
            "cli",
            "board",
            "--scope",
            &"0f".repeat(64),
            "--output",
            "board.nrt",
        ])
        .expect("board arguments");
        assert!(
            matches!(parsed.command, AnonymousBallotCommand::Board(args) if args.scope.as_bytes() == &[0x0f; 64])
        );
        assert!(
            Cli::try_parse_from(["cli", "submit"]).is_err(),
            "an envelope is required"
        );
    }
}
