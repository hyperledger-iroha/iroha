//! Local compiled execution-profile discovery, proving and verification.

use crate::{Run, RunContext};
use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha_core::execution_proofs::{
    RACE_MAX_PROOF_BYTES_V1, compiled_execution_profiles_v1, export_race_kernels_json_v1,
    export_race_parity_json_v1, prove_race_v1, race_profile_id_v1, verify_execution_proof_v1,
};
use iroha_crypto::Hash;
use iroha_data_model::execution_proofs::{ExecutionProofEnvelopeV1, RaceProverRequestV1};
use norito::codec::{Decode, Encode};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

/// Credential-free helpers for the closed native execution-profile catalog.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List compiled profiles and their explicit qualification state.
    Profiles,
    /// Export browser integer kernels from the compiled native RaceV1 relation.
    ExportKernels(ExportKernelArgs),
    /// Export native reference transcripts, snapshots and outcomes for all 21 browser grids.
    ExportParity(ExportKernelArgs),
    /// Prove a canonical request using its exact immutable compiled profile.
    Prove(ProveArgs),
    /// Verify a generic execution envelope locally and print its authenticated outcome.
    Verify(VerifyArgs),
    /// Verify finalized game settlement against an independently pinned network context.
    VerifySettlement(crate::execution_finality::VerifySettlementArgs),
}

/// Destination for the closed native browser-kernel catalog.
#[derive(Debug, Args)]
pub struct ExportKernelArgs {
    /// JSON destination bundled by the browser frontend.
    #[arg(long, value_name = "PATH")]
    output_file: PathBuf,
}

/// Paths for a local native proof job. All witness inputs are public application data.
#[derive(Debug, Args)]
pub struct ProveArgs {
    /// Canonical Norito RaceProverRequestV1 input path.
    #[arg(long, value_name = "PATH")]
    request: PathBuf,
    /// Destination for the canonical generic ExecutionProofEnvelopeV1.
    #[arg(long, value_name = "PATH")]
    output_file: PathBuf,
}

/// Exact generic proof envelope to verify, without submitting a transaction.
#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Canonical Norito ExecutionProofEnvelopeV1 path.
    #[arg(long, value_name = "PATH")]
    proof: PathBuf,
}

fn read_canonical<T: Decode + Encode>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(RACE_MAX_PROOF_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)
        .wrap_err("read bounded execution input")?;
    if bytes.is_empty() || bytes.len() > RACE_MAX_PROOF_BYTES_V1 {
        return Err(eyre!(
            "execution input is empty or exceeds the compiled proof cap"
        ));
    }
    let mut remaining = bytes.as_slice();
    let value = T::decode(&mut remaining).wrap_err("decode execution input")?;
    if !remaining.is_empty() || value.encode() != bytes {
        return Err(eyre!("execution input is not exact canonical Norito"));
    }
    Ok(value)
}

#[derive(Debug, PartialEq, Eq)]
enum RaceProver {
    V1,
}

impl RaceProver {
    fn for_profile(profile_id: &Hash) -> Result<Self> {
        if *profile_id == race_profile_id_v1() {
            Ok(Self::V1)
        } else {
            Err(eyre!(
                "requested racing proof profile is not compiled into this prover"
            ))
        }
    }

    fn prove(self, request: RaceProverRequestV1) -> Result<ExecutionProofEnvelopeV1> {
        match self {
            Self::V1 => prove_race_v1(request),
        }
        .wrap_err("native execution proof generation failed")
    }
}

impl Command {
    /// Execute without loading client configuration, account keys, operator keys or metadata.
    pub fn run_without_client_config(self, mut output: impl Write) -> Result<()> {
        let value = self.execute()?;
        writeln!(output, "{}", norito::json::to_json_pretty(&value)?)?;
        Ok(())
    }
    fn execute(self) -> Result<norito::json::Value> {
        match self {
            Self::Profiles => Ok(norito::json::to_value(&compiled_execution_profiles_v1())?),
            Self::ExportKernels(args) => {
                fs::write(&args.output_file, export_race_kernels_json_v1())
                    .wrap_err("write native kernel catalog")?;
                Ok(
                    norito::json!({"kernel_file":(args.output_file.display().to_string()),"version":1}),
                )
            }
            Self::ExportParity(args) => {
                let corpus =
                    export_race_parity_json_v1().wrap_err("generate native parity corpus")?;
                fs::write(&args.output_file, norito::json::to_json(&corpus)?)
                    .wrap_err("write native reference parity corpus")?;
                Ok(
                    norito::json!({"fixture_file":(args.output_file.display().to_string()),"version":1,"grids":21}),
                )
            }
            Self::Prove(args) => {
                let request = read_canonical::<RaceProverRequestV1>(&args.request)?;
                let prover = RaceProver::for_profile(&request.manifest.profile_id)?;
                let started = Instant::now();
                let proof = prover.prove(request)?;
                let bytes = proof.encode();
                if bytes.len() > RACE_MAX_PROOF_BYTES_V1 {
                    return Err(eyre!("encoded envelope exceeds the compiled wire cap"));
                }
                fs::write(&args.output_file, &bytes)
                    .wrap_err_with(|| format!("write {}", args.output_file.display()))?;
                Ok(
                    norito::json!({"proof_file":(args.output_file.display().to_string()),"proof_bytes":(bytes.len()),"profile_id":(proof.profile_id.to_string()),"elapsed_ms":(started.elapsed().as_millis() as u64)}),
                )
            }
            Self::Verify(args) => {
                let proof = read_canonical::<ExecutionProofEnvelopeV1>(&args.proof)?;
                let outcome = verify_execution_proof_v1(&proof)
                    .wrap_err("native execution verification failed")?;
                Ok(norito::json::to_value(&outcome)?)
            }
            Self::VerifySettlement(args) => args.verify(),
        }
    }
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        context.print_data(&self.execute()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }
    #[test]
    fn prover_preserves_exact_profile_and_rejects_unknown_ids() {
        assert_eq!(
            RaceProver::for_profile(&race_profile_id_v1()).unwrap(),
            RaceProver::V1
        );
        assert!(RaceProver::for_profile(&Hash::new(b"uncompiled-racing-profile")).is_err());
    }
    #[test]
    fn local_proof_commands_require_explicit_paths() {
        let parsed = TestCli::try_parse_from([
            "execution",
            "prove",
            "--request",
            "job.nrt",
            "--output-file",
            "proof.nrt",
        ])
        .expect("parse prove");
        assert!(matches!(parsed.command, Command::Prove(_)));
        assert!(TestCli::try_parse_from(["execution", "prove", "--request", "job.nrt"]).is_err());
        assert!(matches!(
            TestCli::try_parse_from(["execution", "verify", "--proof", "proof.nrt"])
                .expect("parse verify")
                .command,
            Command::Verify(_)
        ));
        assert!(matches!(
            TestCli::try_parse_from(["execution", "profiles"])
                .expect("parse profiles")
                .command,
            Command::Profiles
        ));
        assert!(matches!(
            TestCli::try_parse_from([
                "execution",
                "export-parity",
                "--output-file",
                "race-parity.json"
            ])
            .expect("parse native reference export")
            .command,
            Command::ExportParity(_)
        ));
        assert!(TestCli::try_parse_from(["execution", "export-parity"]).is_err());
    }
}
