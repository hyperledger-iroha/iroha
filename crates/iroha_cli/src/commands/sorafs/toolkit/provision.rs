//! Offline constructors for ordinary SoraFS provisioning and public signed artifacts.
//!
//! These commands never authenticate ledger state or submit requests. All authority,
//! predecessor, funding and provider bindings remain checked by native execution.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, ensure};
use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
use iroha_data_model::isi::InstructionBox;
use std::{
    path::PathBuf,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    bootstrap::{parse_digest, read_pinned, read_private_key, write_new},
    validation::CliError,
};

mod instruction;
mod request;
mod signed;

pub use request::RequestArgs;
pub use signed::{AcknowledgementArgs, AdvertArgs, CatalogArgs, CompliancePolicyArgs};

const MAX_PUBLIC_BYTES: usize = 8 * 1024 * 1024;

/// Offline public artifacts for native provisioning.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Build one ordinary instruction from the closed typed public request.
    Instruction(InputArgs),
    /// Compute native trust-policy and empty-feed transport digests without signing.
    CompliancePolicy(CompliancePolicyArgs),
    /// Sign a native compliance catalog with the exact configured threshold.
    Catalog(CatalogArgs),
    /// Sign a gateway's explicit acknowledgement of a verified catalog.
    Acknowledgement(AcknowledgementArgs),
    /// Sign an advert matching an initial council-verified bootstrap admission.
    Advert(AdvertArgs),
    /// Prepare SDK-native account headers for one exact compliance request; never send it.
    Request(RequestArgs),
}

/// Independently reviewed public request and fresh output.
#[derive(Debug, clap::Args)]
pub struct InputArgs {
    /// Public JSON request; contains no private credential material.
    #[arg(long, value_name = "PATH")]
    request_json: PathBuf,
    /// BLAKE3 of the exact independently reviewed request bytes.
    #[arg(long, value_parser = parse_digest)]
    request_blake3_hex: [u8; 32],
    /// Fresh owner-only output path, never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

/// Independently pinned compliance trust policy.
#[derive(Debug, clap::Args)]
pub struct PolicyArgs {
    /// Native GatewayComplianceTrustPolicyV1 JSON.
    #[arg(long, value_name = "PATH")]
    policy_json: PathBuf,
    /// Independently reviewed BLAKE3 of the policy file.
    #[arg(long, value_parser = parse_digest)]
    policy_blake3_hex: [u8; 32],
}

impl InputArgs {
    fn read<T: norito::json::JsonDeserialize>(&self) -> Result<T> {
        Ok(norito::json::from_slice(&read_pinned(
            &self.request_json,
            MAX_PUBLIC_BYTES,
            self.request_blake3_hex,
        )?)?)
    }
}

impl Command {
    pub(crate) fn run(self) -> std::result::Result<ExitCode, CliError> {
        self.execute()
            .map(|()| ExitCode::SUCCESS)
            .map_err(|error| CliError::Validation(format!("SoraFS provisioning: {error:#}")))
    }

    fn execute(self) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        match self {
            Self::Instruction(args) => {
                let request = args.read::<instruction::Request>()?;
                write_instruction(&args.output, instruction::build(request, now)?)
            }
            Self::CompliancePolicy(args) => signed::compliance_policy(args),
            Self::Catalog(args) => signed::catalog(args, now),
            Self::Acknowledgement(args) => signed::acknowledgement(args, now),
            Self::Advert(args) => signed::advert(args, now),
            Self::Request(args) => request::run(args),
        }
    }
}

fn write_instruction(path: &std::path::Path, instruction: InstructionBox) -> Result<()> {
    write_new(
        path,
        &norito::json::to_vec(&vec![STANDARD.encode(norito::to_bytes(&instruction)?)])?,
    )
}

fn ed25519_public(key: &PrivateKey) -> Result<[u8; 32]> {
    let public =
        PublicKey::from_private_key(key).map_err(|_| eyre::eyre!("invalid signing credential"))?;
    let (algorithm, bytes) = public.try_to_bytes()?;
    ensure!(
        algorithm == Algorithm::Ed25519,
        "artifact signer must be Ed25519"
    );
    bytes
        .try_into()
        .map_err(|_| eyre::eyre!("invalid Ed25519 public key length"))
}

#[cfg(test)]
mod tests;
