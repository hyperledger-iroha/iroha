//! Read-only Sora Name Service (SNS) inspection commands.
//!
//! Alias acquisition, repair, renewal, and auto-renew configuration live under
//! `iroha app alias` and are submitted as normal locally signed transactions.
//! This command tree deliberately exposes only committed SNS records and live
//! suffix-policy inspection.

use crate::{Run, RunContext};
use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, eyre};
use iroha::sns::SnsNamespacePath;

/// Read-only SNS commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Fetch one committed SNS name record.
    Registration(GetRegistrationArgs),
    /// Fetch the live policy for one numeric suffix identifier.
    Policy(GetPolicyArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Registration(args) => args.run(context),
            Self::Policy(args) => args.run(context),
        }
    }
}

/// Canonical SNS namespace used by the read endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum NamespaceArg {
    /// Full account-alias key such as `merchant@banka.paynet`.
    AccountAlias,
    /// Domain name literal.
    Domain,
    /// Dataspace alias literal.
    Dataspace,
}

impl From<NamespaceArg> for SnsNamespacePath {
    fn from(value: NamespaceArg) -> Self {
        match value {
            NamespaceArg::AccountAlias => Self::AccountAlias,
            NamespaceArg::Domain => Self::Domain,
            NamespaceArg::Dataspace => Self::Dataspace,
        }
    }
}

/// Arguments for committed SNS record lookup.
#[derive(Args, Debug)]
pub struct GetRegistrationArgs {
    /// Explicit SNS namespace; no embedded suffix catalog is consulted.
    #[arg(long, value_enum)]
    pub namespace: NamespaceArg,
    /// Exact canonical literal within the selected namespace.
    #[arg(long, value_name = "LITERAL")]
    pub literal: String,
}

impl Run for GetRegistrationArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_literal(&self.literal)?;
        let record = context
            .client_from_config()
            .sns()
            .get_name(self.namespace.into(), &self.literal)?;
        context.print_data(&record)
    }
}

/// Arguments for live suffix-policy lookup.
#[derive(Args, Debug)]
pub struct GetPolicyArgs {
    /// Numeric on-chain suffix identifier.
    #[arg(long = "suffix-id", value_name = "U16")]
    pub suffix_id: u16,
}

impl Run for GetPolicyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let policy = context
            .client_from_config()
            .sns()
            .get_policy(self.suffix_id)?;
        context.print_data(&policy)
    }
}

fn validate_literal(literal: &str) -> Result<()> {
    if literal.is_empty()
        || literal.trim() != literal
        || literal.chars().any(char::is_control)
        || literal.contains('/')
    {
        return Err(eyre!(
            "SNS literal must be exact, non-empty, free of control characters, and contain no path separators"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser, Debug)]
    struct Wrapper {
        #[command(subcommand)]
        command: Command,
    }

    #[test]
    fn parses_typed_read_commands() {
        let registration = Wrapper::parse_from([
            "iroha",
            "registration",
            "--namespace",
            "account-alias",
            "--literal",
            "merchant@banka.paynet",
        ]);
        match registration.command {
            Command::Registration(args) => {
                assert_eq!(args.namespace, NamespaceArg::AccountAlias);
                assert_eq!(args.literal, "merchant@banka.paynet");
            }
            Command::Policy(_) => panic!("expected registration command"),
        }

        let policy = Wrapper::parse_from(["iroha", "policy", "--suffix-id", "7"]);
        match policy.command {
            Command::Policy(args) => assert_eq!(args.suffix_id, 7),
            Command::Registration(_) => panic!("expected policy command"),
        }
    }

    #[test]
    fn mutation_commands_and_payment_proofs_are_absent() {
        for command in [
            "register",
            "renew",
            "transfer",
            "update-controllers",
            "freeze",
            "unfreeze",
            "governance",
        ] {
            assert!(
                Wrapper::try_parse_from(["iroha", command]).is_err(),
                "retired SNS mutation command `{command}` must stay absent"
            );
        }
        assert!(
            Wrapper::try_parse_from([
                "iroha",
                "registration",
                "--namespace",
                "domain",
                "--literal",
                "merchant",
                "--payment-proof",
                "proof.json",
            ])
            .is_err()
        );
    }

    #[test]
    fn rejects_non_exact_or_path_like_literals() {
        for literal in ["", " merchant", "merchant ", "merchant/name", "merchant\n"] {
            assert!(validate_literal(literal).is_err(), "literal `{literal:?}`");
        }
    }
}
