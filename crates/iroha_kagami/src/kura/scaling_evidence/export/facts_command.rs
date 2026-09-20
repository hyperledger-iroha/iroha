//! Explicit launch bindings and bounded replies for authenticated facts publication.
//!
//! The caller independently selects the complete launch geometry, original pins and
//! stopped-store tip. The retained publisher authenticates those files and the real
//! canonical transcript before creating any facts stage. This command does not
//! establish process readiness, shutdown provenance or resource measurements.

use super::{
    filesystem::{FactsInputBindings, PreparedTransportIdentity, ProofInputBinding, produce_facts},
    launcher::{
        journal::{
            JournalAccount, JournalBounds, JournalExpectations, JournalSampling, JournalTiming,
            JournalVariant, admit_expectations,
        },
        prepare::assemble::{FactsAssemblyCaps, GenesisExpectations},
    },
};
use crate::{
    Outcome, RunArgs,
    kura::scaling_evidence::{
        VerificationLimits,
        command::{ReaderArgs, parse_sha256},
    },
};
use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Result, ensure, eyre};
use iroha_core::{kura::CanonicalKuraEvidenceLimits, queue::RoutingDecision};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
};
use iroha_model_base::{
    chain::ChainId,
    topology::{DataSpaceId, LaneId},
};
use std::{
    collections::BTreeSet,
    io::{BufWriter, Write},
    path::PathBuf,
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_REPLY_BYTES: usize = 512;
const MAX_REQUESTS: usize = 1_000_000;

/// Authenticate ten original files and the complete stopped Kura interval.
#[derive(Clone, Debug, ClapArgs)]
pub(crate) struct Args {
    /// Independently selected lowercase SHA-256 invocation identity
    #[arg(long, value_parser = parse_sha256)]
    invocation_id: [u8; 32],
    #[command(flatten)]
    originals: OriginalArgs,
    #[command(flatten)]
    genesis: GenesisArgs,
    #[command(flatten)]
    journal: JournalArgs,
    #[command(flatten)]
    verification: VerificationArgs,
    #[command(flatten)]
    reader: ReaderArgs,
    /// Exact absolute stopped validator directory containing canonical block journals
    #[arg(long)]
    block_store: PathBuf,
    /// Exact absolute original canonical merge log
    #[arg(long)]
    merge_log: PathBuf,
    /// New absolute canonical facts output; existing stage or destination fails
    #[arg(long)]
    facts_output: PathBuf,
    /// Reserved cumulative original-file bytes, at most 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    source_max_bytes: u64,
    /// Reserved canonical facts output bytes, at most 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    facts_max_bytes: u64,
    /// Aggregate source and facts reservations, at most 268435456
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    total_max_bytes: u64,
    /// Cumulative Norito allocation budget for the entire assembly, at most 536870912
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=536870912))]
    assembly_decode_max_bytes: u64,
    /// Maximum complete five-field JSON reply including its final newline
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=512))]
    reply_max_bytes: u64,
}

#[derive(Clone, Debug, ClapArgs)]
struct OriginalArgs {
    /// Original final manifest absolute path; external artifact references are rejected
    #[arg(long)]
    manifest: PathBuf,
    /// Independently pinned raw SHA-256 of the final manifest
    #[arg(long, value_parser = parse_sha256)]
    manifest_sha256: [u8; 32],
    /// Maximum original final manifest bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    manifest_max_bytes: u64,
    /// Original canonical signed genesis absolute path
    #[arg(long)]
    signed_genesis: PathBuf,
    /// Independently pinned raw SHA-256 of canonical signed genesis
    #[arg(long, value_parser = parse_sha256)]
    signed_genesis_sha256: [u8; 32],
    /// Maximum original signed genesis bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    signed_genesis_max_bytes: u64,
    /// Four original final peer config absolute paths, in independently selected validator order
    #[arg(long, required = true, num_args = 4)]
    peer_config: Vec<PathBuf>,
    /// Four independently pinned raw config SHA-256 values, in the same order
    #[arg(long, required = true, num_args = 4, value_parser = parse_sha256)]
    peer_config_sha256: Vec<[u8; 32]>,
    /// Maximum bytes for each of the four original peer configs
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    peer_config_max_bytes: u64,
    /// Original independent canonical genesis HeightContext absolute path
    #[arg(long)]
    context: PathBuf,
    /// Independently pinned raw SHA-256 of the original context
    #[arg(long, value_parser = parse_sha256)]
    context_sha256: [u8; 32],
    /// Maximum original context bytes, at most 8388608
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=8388608))]
    context_max_bytes: u64,
    /// Original complete signed-request collector journal absolute path
    #[arg(long)]
    journal: PathBuf,
    /// Independently pinned raw SHA-256 of the complete original journal
    #[arg(long, value_parser = parse_sha256)]
    journal_sha256: [u8; 32],
    /// Maximum complete original journal bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    journal_max_bytes: u64,
    /// Original canonical Vec<FinalizedNativeContextV1> absolute path
    #[arg(long)]
    finality: PathBuf,
    /// Independently pinned raw SHA-256 of the complete finality vector
    #[arg(long, value_parser = parse_sha256)]
    finality_sha256: [u8; 32],
    /// Maximum complete finality vector bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    finality_max_bytes: u64,
    /// Original canonical Vec<CommittedTransaction> absolute path, preserving every query
    #[arg(long)]
    queries: PathBuf,
    /// Independently pinned raw SHA-256 of the complete query vector
    #[arg(long, value_parser = parse_sha256)]
    queries_sha256: [u8; 32],
    /// Maximum complete query vector bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    queries_max_bytes: u64,
}

#[derive(Clone, Debug, ClapArgs)]
struct GenesisArgs {
    /// Independently selected canonical chain label
    #[arg(long)]
    chain_id: ChainId,
    /// Exact expected genesis-header NetworkId in canonical checked hash literal form
    #[arg(long)]
    network_id: NetworkId,
    /// Independently selected I105 chain discriminant
    #[arg(long)]
    chain_discriminant: u16,
    /// Independently selected public genesis signer; no private signing input is accepted
    #[arg(long)]
    genesis_public_key: PublicKey,
    /// Four selected validator public keys in the same original role order as peer configs
    #[arg(long, required = true, num_args = 4)]
    validator: Vec<PublicKey>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Variant {
    /// One fixed active execution lane.
    #[value(name = "1")]
    One,
    /// Four fixed active execution lanes.
    #[value(name = "4")]
    Four,
}

#[derive(Clone, Debug, ClapArgs)]
struct JournalArgs {
    /// Fixed execution-lane count, either 1 or 4
    #[arg(long, value_enum)]
    lanes: Variant,
    /// Public workload seed as exactly 64 lowercase hex characters; never the development key seed
    #[arg(long, value_parser = parse_sha256)]
    workload_seed: [u8; 32],
    /// Independent paired-run index from 1 through 5
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=5))]
    pair_index: u8,
    /// Ordered canonical I105 account pool, 4 through 64 accounts in complete groups of four
    #[arg(long, required = true, num_args = 4..=64)]
    account: Vec<String>,
    /// Exact positive rational offered-rate numerator, in requests per second
    #[arg(long, value_parser = parse_positive_u128)]
    rate_numerator: u128,
    /// Exact positive rational offered-rate denominator
    #[arg(long, value_parser = parse_positive_u128)]
    rate_denominator: u128,
    /// Independent warmup duration in nanoseconds
    #[arg(long)]
    warmup_ns: i64,
    /// Independent measurement duration in nanoseconds
    #[arg(long)]
    measurement_ns: i64,
    /// Independent drain duration in nanoseconds
    #[arg(long)]
    drain_ns: i64,
    /// Maximum allowed submission lag in nanoseconds
    #[arg(long)]
    submission_lag_bound_ns: i64,
    /// Independently selected signed-request preparation lookahead
    #[arg(long)]
    preparation_lookahead: usize,
    /// Independently selected preparation concurrency
    #[arg(long)]
    preparation_concurrency: usize,
    /// Maximum preparation lead time in nanoseconds
    #[arg(long)]
    preparation_ahead_ns: i64,
    /// Maximum concurrent submissions selected before collection
    #[arg(long)]
    max_submissions: usize,
    /// Maximum in-flight requests selected before collection
    #[arg(long)]
    max_in_flight: usize,
    /// Maximum concurrent status requests selected before collection
    #[arg(long)]
    max_status_requests: usize,
    /// Independent status poll interval in nanoseconds
    #[arg(long)]
    poll_interval_ns: i64,
    /// Maximum complete journal schedule requests
    #[arg(long)]
    journal_max_requests: usize,
    /// Independent resource sampling interval in nanoseconds
    #[arg(long)]
    resource_interval_ns: i64,
    /// Independent resource response deadline in nanoseconds
    #[arg(long)]
    resource_response_deadline_ns: i64,
    /// Maximum resource sampling start lag in nanoseconds
    #[arg(long)]
    resource_max_start_lag_ns: i64,
}

#[derive(Clone, Debug, ClapArgs)]
struct VerificationArgs {
    /// Independent admitted canonical proof byte allocation
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    proof_max_bytes: u64,
    /// Maximum cumulative verifier input bytes including the signed schedule
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    verification_input_max_bytes: u64,
    /// Reserved canonical proof and complete result-row output bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    verification_output_max_bytes: u64,
    /// Maximum contiguous global heights, at most 65536
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=65536))]
    max_heights: u64,
    /// Maximum complete scheduled request count, at most 1000000
    #[arg(long)]
    max_requests: usize,
    /// Maximum ordinary and merged leaves in one carrier, at most 1000000
    #[arg(long)]
    max_leaves_per_carrier: usize,
}

struct AdmittedArgs {
    inputs: FactsInputBindings,
    genesis: GenesisExpectations,
    journal: JournalExpectations,
    verification: VerificationLimits,
    reader: CanonicalKuraEvidenceLimits,
    caps: FactsAssemblyCaps,
    block_store: PathBuf,
    merge_log: PathBuf,
    output: PathBuf,
    invocation_id: [u8; 32],
    reply_max_bytes: u64,
}

impl Args {
    fn into_inputs(self) -> Result<AdmittedArgs> {
        ensure!(
            (1..=MAX_REPLY_BYTES as u64).contains(&self.reply_max_bytes),
            "invalid facts reply reservation"
        );
        ensure!(
            [
                self.source_max_bytes,
                self.facts_max_bytes,
                self.total_max_bytes
            ]
            .iter()
            .all(|n| (1..=MAX_BYTES).contains(n))
                && self
                    .source_max_bytes
                    .checked_add(self.facts_max_bytes)
                    .is_some_and(|n| n <= self.total_max_bytes)
                && (1..=MAX_BYTES * 2).contains(&self.assembly_decode_max_bytes),
            "invalid facts assembly reservations"
        );
        let genesis = self.genesis.into_expectations()?;
        let journal = self
            .journal
            .into_expectations(genesis.network_id, genesis.chain_discriminant)?;
        admit_expectations(&journal)?;
        let verification = self.verification.into_limits()?;
        ensure!(
            journal.bounds.max_requests <= verification.requests,
            "journal request bound exceeds verifier reservation"
        );
        let reader = self.reader.into_limits()?;
        ensure!(
            reader.first_height == 1
                && reader.last_height > 0
                && reader.last_height <= verification.heights
                && reader.last_height <= reader.max_committed_blocks,
            "facts require the complete independently bounded genesis-to-tip interval"
        );
        let inputs = self.originals.into_bindings(self.source_max_bytes)?;
        Ok(AdmittedArgs {
            inputs,
            genesis,
            journal,
            verification,
            reader,
            caps: FactsAssemblyCaps {
                input_bytes: self.source_max_bytes,
                facts_bytes: self.facts_max_bytes,
                total_bytes: self.total_max_bytes,
                decode_bytes: self.assembly_decode_max_bytes,
            },
            block_store: self.block_store,
            merge_log: self.merge_log,
            output: self.facts_output,
            invocation_id: self.invocation_id,
            reply_max_bytes: self.reply_max_bytes,
        })
    }
}

impl OriginalArgs {
    fn into_bindings(self, source_max_bytes: u64) -> Result<FactsInputBindings> {
        ensure!(
            self.peer_config.len() == 4 && self.peer_config_sha256.len() == 4,
            "facts require exactly four ordered peer configs and pins"
        );
        let caps = [
            self.manifest_max_bytes,
            self.signed_genesis_max_bytes,
            self.peer_config_max_bytes,
            self.peer_config_max_bytes,
            self.peer_config_max_bytes,
            self.peer_config_max_bytes,
            self.context_max_bytes,
            self.journal_max_bytes,
            self.finality_max_bytes,
            self.queries_max_bytes,
        ];
        let total = caps.iter().try_fold(0u64, |total, &n| {
            ensure!(
                (1..=MAX_BYTES).contains(&n),
                "invalid original-file reservation"
            );
            total
                .checked_add(n)
                .ok_or_else(|| eyre!("original-file reservation overflow"))
        })?;
        ensure!(
            (1..=MAX_BYTES).contains(&source_max_bytes)
                && total <= source_max_bytes
                && self.context_max_bytes <= 8 * 1024 * 1024,
            "original-file reservations exceed source allocation"
        );
        let peers = self
            .peer_config
            .into_iter()
            .zip(self.peer_config_sha256)
            .map(|(path, sha256)| ProofInputBinding {
                path,
                sha256,
                max_bytes: self.peer_config_max_bytes,
            })
            .collect::<Vec<_>>();
        let peer_configs: [ProofInputBinding; 4] = peers
            .try_into()
            .map_err(|_| eyre!("peer config arity changed"))?;
        Ok(FactsInputBindings {
            manifest: ProofInputBinding {
                path: self.manifest,
                sha256: self.manifest_sha256,
                max_bytes: self.manifest_max_bytes,
            },
            signed_genesis: ProofInputBinding {
                path: self.signed_genesis,
                sha256: self.signed_genesis_sha256,
                max_bytes: self.signed_genesis_max_bytes,
            },
            peer_configs,
            context: ProofInputBinding {
                path: self.context,
                sha256: self.context_sha256,
                max_bytes: self.context_max_bytes,
            },
            journal: ProofInputBinding {
                path: self.journal,
                sha256: self.journal_sha256,
                max_bytes: self.journal_max_bytes,
            },
            finality: ProofInputBinding {
                path: self.finality,
                sha256: self.finality_sha256,
                max_bytes: self.finality_max_bytes,
            },
            queries: ProofInputBinding {
                path: self.queries,
                sha256: self.queries_sha256,
                max_bytes: self.queries_max_bytes,
            },
        })
    }
}

impl GenesisArgs {
    fn into_expectations(self) -> Result<GenesisExpectations> {
        ensure!(
            self.validator.len() == 4,
            "facts require exactly four selected validators"
        );
        let validators: [PublicKey; 4] = self
            .validator
            .try_into()
            .map_err(|_| eyre!("validator arity changed"))?;
        ensure!(
            validators.iter().collect::<BTreeSet<_>>().len() == 4,
            "selected validators must be four distinct identities"
        );
        Ok(GenesisExpectations {
            chain_id: self.chain_id,
            network_id: self.network_id,
            chain_discriminant: self.chain_discriminant,
            genesis_public_key: self.genesis_public_key,
            validators,
        })
    }
}

impl JournalArgs {
    fn into_expectations(
        self,
        network_id: NetworkId,
        chain_discriminant: u16,
    ) -> Result<JournalExpectations> {
        ensure!(
            (1..=5).contains(&self.pair_index)
                && (4..=64).contains(&self.account.len())
                && self.account.len().is_multiple_of(4),
            "invalid facts run or account geometry"
        );
        ensure!(
            self.rate_numerator > 0 && self.rate_denominator > 0,
            "offered rate must be positive"
        );
        let _discriminant = ChainDiscriminantGuard::enter(chain_discriminant);
        let lane_count = match self.lanes {
            Variant::One => 1usize,
            Variant::Four => 4,
        };
        let mut accounts = Vec::with_capacity(self.account.len());
        let mut seen = BTreeSet::new();
        for (index, literal) in self.account.into_iter().enumerate() {
            ensure!(
                literal.len() <= 4096,
                "account literal exceeds command bound"
            );
            let authority = AccountId::parse_encoded(&literal)
                .map_err(|_| eyre!("invalid canonical account for selected chain discriminant"))?;
            ensure!(
                authority.canonical_i105()? == literal
                    && authority.try_signatory().is_some()
                    && seen.insert(authority.clone()),
                "accounts must be unique canonical single-key identities"
            );
            accounts.push(JournalAccount {
                authority,
                route: RoutingDecision {
                    lane_id: LaneId::new(u32::try_from(index % lane_count)?),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                },
            });
        }
        Ok(JournalExpectations {
            network_id,
            seed: hex::encode(self.workload_seed),
            pair_index: self.pair_index,
            variant: match self.lanes {
                Variant::One => JournalVariant::OneLane,
                Variant::Four => JournalVariant::FourLane,
            },
            accounts,
            timing: JournalTiming {
                rate_numerator: self.rate_numerator,
                rate_denominator: self.rate_denominator,
                warmup_ns: self.warmup_ns,
                measurement_ns: self.measurement_ns,
                drain_ns: self.drain_ns,
                submission_lag_bound_ns: self.submission_lag_bound_ns,
            },
            bounds: JournalBounds {
                preparation_lookahead: self.preparation_lookahead,
                preparation_concurrency: self.preparation_concurrency,
                preparation_ahead_ns: self.preparation_ahead_ns,
                max_submissions: self.max_submissions,
                max_in_flight: self.max_in_flight,
                max_status_requests: self.max_status_requests,
                poll_interval_ns: self.poll_interval_ns,
                max_requests: self.journal_max_requests,
            },
            sampling: JournalSampling {
                interval_ns: self.resource_interval_ns,
                response_deadline_ns: self.resource_response_deadline_ns,
                max_start_lag_ns: self.resource_max_start_lag_ns,
            },
        })
    }
}

impl VerificationArgs {
    fn into_limits(self) -> Result<VerificationLimits> {
        ensure!(
            [
                self.proof_max_bytes,
                self.verification_input_max_bytes,
                self.verification_output_max_bytes
            ]
            .iter()
            .all(|n| (1..=MAX_BYTES).contains(n))
                && self
                    .verification_input_max_bytes
                    .checked_add(self.verification_output_max_bytes)
                    .is_some_and(|n| n <= self.proof_max_bytes)
                && (1..=65536).contains(&self.max_heights)
                && (1..=MAX_REQUESTS).contains(&self.max_requests)
                && (1..=MAX_REQUESTS).contains(&self.max_leaves_per_carrier),
            "invalid facts verification reservations"
        );
        Ok(VerificationLimits {
            admitted_proof_bytes: self.proof_max_bytes,
            input_bytes: self.verification_input_max_bytes,
            output_bytes: self.verification_output_max_bytes,
            heights: self.max_heights,
            requests: self.max_requests,
            leaves_per_carrier: self.max_leaves_per_carrier,
        })
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let args = self.into_inputs()?;
        let published = produce_facts(
            args.inputs,
            &args.output,
            args.genesis,
            args.journal,
            args.verification,
            &args.block_store,
            &args.merge_log,
            args.reader,
            args.caps,
        )?;
        published.finish_reply(|identity| {
            FactsReply::new(args.invocation_id, args.reply_max_bytes, identity)?.write(writer)
        })?;
        Ok(())
    }
}

fn parse_positive_u128(value: &str) -> std::result::Result<u128, String> {
    if value.is_empty()
        || value.len() > 39
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.starts_with('0')
    {
        return Err("expected one canonical positive decimal u128".to_owned());
    }
    value
        .parse()
        .map_err(|_| "positive decimal value exceeds u128".to_owned())
}

// Only fixed literals, two exactly 64-byte digests and one u64 enter this reply.
// The scalar receipt does not echo paths, account keys or configuration content.
struct FactsReply {
    bytes: [u8; MAX_REPLY_BYTES],
    len: usize,
}
impl FactsReply {
    fn new(
        invocation_id: [u8; 32],
        maximum: u64,
        identity: PreparedTransportIdentity,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_REPLY_BYTES as u64).contains(&maximum)
                && (1..=MAX_BYTES).contains(&identity.byte_length),
            "invalid facts reply bounds"
        );
        let mut invocation_hex = [0u8; 64];
        let mut facts_hex = [0u8; 64];
        hex::encode_to_slice(invocation_id, &mut invocation_hex)?;
        hex::encode_to_slice(identity.raw_sha256, &mut facts_hex)?;
        let mut bytes = [0u8; MAX_REPLY_BYTES];
        let mut remaining = &mut bytes[..];
        writeln!(
            remaining,
            "{{\"version\":1,\"operation\":\"facts\",\"invocation_id\":\"{}\",\"facts_sha256\":\"{}\",\"facts_bytes\":{}}}",
            std::str::from_utf8(&invocation_hex)?,
            std::str::from_utf8(&facts_hex)?,
            identity.byte_length
        )?;
        let len = MAX_REPLY_BYTES - remaining.len();
        ensure!(
            u64::try_from(len)? <= maximum,
            "complete facts reply exceeds reservation"
        );
        Ok(Self { bytes, len })
    }
    fn write<T: Write>(self, writer: &mut BufWriter<T>) -> Outcome {
        writer.write_all(&self.bytes[..self.len])?;
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
