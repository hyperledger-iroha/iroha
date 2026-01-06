//! Generic data-availability CLI helpers (DA-8 roadmap workstream).

use super::{
    da_common::{
        DaManifestFetchBundle, DaManifestFetcher, DaPublisher, DaPublisherReceipt,
        load_metadata_from_path, parse_blob_class, parse_fec_scheme, parse_storage_class,
    },
    sorafs::FetchArgs,
};
use crate::{Run, RunContext};
use base64::{Engine, engine::general_purpose::STANDARD as Base64Standard};
#[cfg(test)]
use blake3::hash as blake3_hash;
use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha::da::{self, DaIngestParams, DaRentLedgerPlan};
use iroha::data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    da::{
        ingest::DaIngestReceipt,
        manifest::DaManifestV1,
        types::{
            BlobCodec, BlobDigest, DaRentLedgerProjection, DaRentPolicyV1, DaRentQuote,
            ErasureProfile, ExtraMetadata, GovernanceTag, RetentionPolicy,
        },
    },
    nexus::LaneId,
};
use iroha_crypto::Hash;
use iroha_torii_shared::da::sampling::build_sampling_plan;
use norito::{
    codec::Encode,
    decode_from_bytes,
    json::{self, Map, Number, Value},
};
#[cfg(test)]
use sorafs_car::sorafs_chunker::ChunkProfile;
use sorafs_car::{CarBuildPlan, ChunkStore, FilePayload, PorProof};
use sorafs_manifest::deal::{MICRO_XOR_PER_XOR, XorAmount};
use std::{
    collections::HashSet,
    convert::{TryFrom, TryInto},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Submit a raw blob to `/v1/da/ingest` and capture the signed receipt.
    Submit(SubmitArgs),
    /// Fetch blobs via the multi-source orchestrator (thin wrapper over `sorafs fetch`).
    Get(FetchArgs),
    /// Download manifest + chunk plan artifacts for an existing DA storage ticket.
    GetBlob(GetBlobArgs),
    /// Generate Proof-of-Retrievability witnesses for a manifest/payload pair.
    Prove(ProveArgs),
    /// Download + verify availability for a storage ticket using a Torii manifest.
    ProveAvailability(ProveAvailabilityArgs),
    /// Quote rent/incentive breakdown for a blob size/retention combo.
    RentQuote(RentQuoteArgs),
    /// Convert a rent quote into deterministic ledger transfer instructions.
    RentLedger(RentLedgerArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Submit(args) => args.run(context),
            Command::Get(args) => args.run(context),
            Command::GetBlob(args) => args.run(context),
            Command::Prove(args) => args.run(context),
            Command::ProveAvailability(args) => args.run(context),
            Command::RentQuote(args) => args.run(context),
            Command::RentLedger(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct SubmitArgs {
    /// Path to the blob payload (CAR, manifest bundle, governance file, etc.).
    #[arg(long, value_name = "PATH")]
    pub payload: PathBuf,
    /// Lane identifier recorded in the DA request.
    #[arg(long, default_value_t = 0)]
    pub lane_id: u32,
    /// Epoch identifier recorded in the DA request.
    #[arg(long, default_value_t = 0)]
    pub epoch: u64,
    /// Monotonic sequence scoped to (lane, epoch).
    #[arg(long, default_value_t = 0)]
    pub sequence: u64,
    /// Blob-class label (`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, `custom:<id>`).
    #[arg(long = "blob-class", default_value = "nexus_lane_sidecar")]
    pub blob_class: String,
    /// Codec label describing the payload.
    #[arg(long = "blob-codec", default_value = "custom.binary")]
    pub blob_codec: String,
    /// Chunk size in bytes used for DA chunking.
    #[arg(long = "chunk-size", default_value_t = 262_144)]
    pub chunk_size: u32,
    /// Number of data shards in the erasure profile.
    #[arg(long = "data-shards", default_value_t = 10)]
    pub data_shards: u16,
    /// Number of parity shards in the erasure profile.
    #[arg(long = "parity-shards", default_value_t = 4)]
    pub parity_shards: u16,
    /// Chunk alignment (chunks per availability slice).
    #[arg(long = "chunk-alignment", default_value_t = 10)]
    pub chunk_alignment: u16,
    /// FEC scheme label (`rs12_10`, `rswin14_10`, `rs18_14`, `custom:<id>`).
    #[arg(long = "fec-scheme", default_value = "rs12_10")]
    pub fec_scheme: String,
    /// Hot retention in seconds.
    #[arg(long = "hot-retention-secs", default_value_t = 604_800)]
    pub hot_retention_secs: u64,
    /// Cold retention in seconds.
    #[arg(long = "cold-retention-secs", default_value_t = 7_776_000)]
    pub cold_retention_secs: u64,
    /// Required replica count enforced by retention policy.
    #[arg(long = "required-replicas", default_value_t = 3)]
    pub required_replicas: u16,
    /// Storage-class label (`hot`, `warm`, `cold`).
    #[arg(long = "storage-class", default_value = "warm")]
    pub storage_class: String,
    /// Governance tag recorded in the retention policy.
    #[arg(long = "governance-tag", default_value = "da.generic")]
    pub governance_tag: String,
    /// Optional metadata JSON file providing string key/value pairs.
    #[arg(long = "metadata-json", value_name = "PATH")]
    pub metadata_json: Option<PathBuf>,
    /// Optional pre-generated Norito manifest to embed in the request.
    #[arg(long = "manifest", value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
    /// Override for the Torii DA ingest endpoint (defaults to `$TORII/v1/da/ingest`).
    #[arg(long = "endpoint", value_name = "URL")]
    pub endpoint: Option<String>,
    /// Override the caller-supplied blob identifier (hex). Defaults to BLAKE3(payload).
    #[arg(long = "client-blob-id", value_name = "HEX")]
    pub client_blob_id: Option<String>,
    /// Directory for storing Norito/JSON artefacts (defaults to `artifacts/da/submission_<timestamp>`).
    #[arg(long = "artifact-dir", value_name = "PATH")]
    pub artifact_dir: Option<PathBuf>,
    /// Skip HTTP submission and only emit the signed request artefacts.
    #[arg(long = "no-submit")]
    pub no_submit: bool,
    /// Fixture file providing a mocked DA receipt (test-only helper; bypasses HTTP submit).
    #[arg(long = "receipt-fixture", value_name = "PATH", hide = true)]
    pub receipt_fixture: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct GetBlobArgs {
    /// Storage ticket identifier (hex string) issued by Torii.
    #[arg(long = "storage-ticket", value_name = "HEX")]
    pub storage_ticket: String,
    /// Optional block hash used to seed deterministic sampling in the manifest response.
    #[arg(long = "block-hash", value_name = "HEX")]
    pub block_hash: Option<String>,
    /// Optional override for the Torii manifest endpoint (defaults to `$TORII/v1/da/manifests/`).
    #[arg(long = "endpoint", value_name = "URL")]
    pub endpoint: Option<String>,
    /// Directory for storing the fetched manifest + chunk plan artefacts.
    #[arg(long = "output-dir", value_name = "PATH")]
    pub output_dir: Option<PathBuf>,
}

impl GetBlobArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let normalized_ticket = normalize_ticket_hex(&self.storage_ticket)?;
        let normalized_block_hash = if let Some(block_hash) = self.block_hash {
            Some(normalize_block_hash_hex(&block_hash)?)
        } else {
            None
        };
        let fetcher = DaManifestFetcher::new(context.config(), self.endpoint.as_deref())?;
        let bundle = fetcher.fetch(&normalized_ticket, normalized_block_hash.as_deref())?;
        let manifest_label = bundle.manifest_hash_hex.to_ascii_lowercase();
        let persisted =
            persist_manifest_bundle(context, &bundle, self.output_dir, &manifest_label)?;
        if let Some(path) = &persisted.sampling_plan {
            context.println(format_args!(
                "sampling plan JSON saved to `{}`",
                path.display()
            ))?;
        }
        Ok(())
    }
}

impl SubmitArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let payload_bytes = fs::read(&self.payload)
            .wrap_err_with(|| format!("failed to read payload `{}`", self.payload.display()))?;
        let metadata = match &self.metadata_json {
            Some(path) => load_metadata_from_path(path)?,
            None => ExtraMetadata { items: Vec::new() },
        };
        let manifest_bytes = match &self.manifest_path {
            Some(path) => Some(
                fs::read(path)
                    .wrap_err_with(|| format!("failed to read manifest `{}`", path.display()))?,
            ),
            None => None,
        };
        let params = build_da_params(&self)?;
        let request = da::build_da_request(
            payload_bytes,
            &params,
            metadata,
            &context.config().key_pair,
            manifest_bytes,
        );
        let request_bytes = request.encode();
        let request_json = norito::json::to_json_pretty(&request)
            .map_err(|err| eyre!("failed to render DA request JSON: {err}"))?;
        let artifact_root = default_artifact_root(self.artifact_dir)?;
        fs::create_dir_all(&artifact_root).wrap_err_with(|| {
            format!(
                "failed to create artifact directory `{}`",
                artifact_root.display()
            )
        })?;
        let request_path = artifact_root.join("da_request.norito");
        let request_json_path = artifact_root.join("da_request.json");
        fs::write(&request_path, &request_bytes)
            .wrap_err_with(|| format!("failed to write DA request `{}`", request_path.display()))?;
        fs::write(&request_json_path, request_json.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write DA request JSON `{}`",
                request_json_path.display()
            )
        })?;
        context.println(format_args!(
            "prepared DA request blob={} bytes={} request=`{}`",
            hex::encode(request.client_blob_id.as_bytes()),
            request_bytes.len(),
            request_path.display(),
        ))?;
        if self.no_submit {
            if self.receipt_fixture.is_some() {
                return Err(eyre!(
                    "--receipt-fixture cannot be combined with --no-submit (fixture already skips HTTP)"
                ));
            }
            context.println("skipping Torii publish (--no-submit)")?;
            return Ok(());
        }
        let receipt = if let Some(path) = &self.receipt_fixture {
            load_da_receipt_fixture(path)?
        } else {
            let publisher = DaPublisher::new(context.config(), self.endpoint.as_deref())?;
            publisher.publish(&request_bytes)?
        };
        let receipt_path = artifact_root.join("da_receipt.norito");
        let receipt_json_path = artifact_root.join("da_receipt.json");
        fs::write(&receipt_path, &receipt.bytes)
            .wrap_err_with(|| format!("failed to write DA receipt `{}`", receipt_path.display()))?;
        fs::write(&receipt_json_path, receipt.json.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write DA receipt JSON `{}`",
                receipt_json_path.display()
            )
        })?;
        if let Some(header_value) = &receipt.pdp_commitment_header {
            persist_receipt_headers(&artifact_root, header_value)?;
        }
        context.println(format_args!(
            "Torii accepted blob {} storage_ticket={} queued_at={}",
            hex::encode(receipt.receipt.client_blob_id.as_bytes()),
            hex::encode(receipt.receipt.storage_ticket.as_bytes()),
            receipt.receipt.queued_at_unix,
        ))?;
        context.println(format_args!(
            "{}",
            format_rent_quote_summary(&receipt.receipt.rent_quote)
        ))?;
        context.print_data(&receipt.receipt)?;
        Ok(())
    }
}

#[derive(Args, Debug)]
pub struct ProveArgs {
    /// Path to the Norito-encoded manifest describing the chunk layout.
    #[arg(long, value_name = "PATH")]
    pub manifest: PathBuf,
    /// Path to the assembled payload bytes that match the manifest.
    #[arg(long, value_name = "PATH")]
    pub payload: PathBuf,
    /// Optional JSON output path; defaults to stdout only.
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
    /// Number of random leaves to sample for `PoR` proofs (0 disables sampling).
    #[arg(long = "sample-count", default_value_t = 8)]
    pub sample_count: usize,
    /// Seed used for deterministic `PoR` sampling.
    #[arg(long = "sample-seed", default_value_t = 0)]
    pub sample_seed: u64,
    /// Optional block hash used to derive deterministic sampling (overrides sample-count/seed).
    #[arg(long = "block-hash", value_name = "HEX")]
    pub block_hash: Option<String>,
    /// Explicit `PoR` leaf indexes to prove (0-based flattened index).
    #[arg(long = "leaf-index", value_name = "INDEX")]
    pub leaf_indexes: Vec<usize>,
}

impl ProveArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest = load_manifest(&self.manifest)?;
        let plan = build_car_plan_from_manifest(&manifest)?;
        let mut chunk_store = ChunkStore::with_profile(plan.chunk_profile);

        let mut ingest_source = FilePayload::open(&self.payload)
            .wrap_err_with(|| format!("failed to open payload `{}`", self.payload.display()))?;
        chunk_store
            .ingest_plan_source(&plan, &mut ingest_source)
            .wrap_err("failed to ingest payload for proof generation")?;

        validate_manifest_consistency(&manifest, &chunk_store)?;

        let mut proof_source = FilePayload::open(&self.payload).wrap_err_with(|| {
            format!(
                "failed to reopen payload `{}` for PoR sampling",
                self.payload.display()
            )
        })?;
        let sampling = self.resolve_sampling(&manifest)?;
        let por_root = chunk_store.por_tree().root().to_owned();
        let root_hex = hex::encode(por_root);
        let leaf_total = chunk_store.por_tree().leaf_count();
        let segment_total = chunk_store.por_tree().segment_count();
        let chunk_total = chunk_store.por_tree().chunks().len();
        let proofs = self.collect_proofs(&sampling, &chunk_store, &mut proof_source, &por_root)?;

        let summary_inputs = ProofSummaryInputs {
            manifest: &manifest,
            manifest_path: &self.manifest,
            payload_path: &self.payload,
            por_root_hex: root_hex,
            leaf_total,
            segment_total,
            chunk_total,
            sample_count: sampling.sample_count,
            sample_seed: sampling.sample_seed,
        };
        let summary = build_proof_summary(summary_inputs, &proofs);

        context.print_data(&summary)?;

        if let Some(path) = &self.json_out {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to create proof output directory `{}`",
                        parent.display()
                    )
                })?;
            }
            let rendered = norito::json::to_json_pretty(&summary)
                .wrap_err("failed to render proof summary JSON")?;
            fs::write(path, rendered).wrap_err_with(|| {
                format!("failed to write proof summary to `{}`", path.display())
            })?;
        }

        Ok(())
    }

    fn resolve_sampling(&self, manifest: &DaManifestV1) -> Result<ProofSampling> {
        if let Some(block_hash_hex) = &self.block_hash {
            let normalized = normalize_block_hash_hex(block_hash_hex)?;
            let block_hash =
                Hash::from_str(&normalized).map_err(|err| eyre!("invalid block hash: {err}"))?;
            let plan = build_sampling_plan(manifest, &block_hash);
            if plan.samples.is_empty() {
                return Err(eyre!("sampling plan produced no chunks for manifest"));
            }
            return Ok(ProofSampling {
                sample_count: plan.samples.len(),
                sample_seed: plan.por_seed(),
            });
        }

        Ok(ProofSampling {
            sample_count: self.sample_count,
            sample_seed: self.sample_seed,
        })
    }

    fn collect_proofs(
        &self,
        sampling: &ProofSampling,
        chunk_store: &ChunkStore,
        proof_source: &mut FilePayload,
        por_root: &[u8; 32],
    ) -> Result<Vec<ProofReport>> {
        let mut proofs = Vec::new();
        let mut seen = HashSet::new();
        proofs.extend(Self::sampled_proofs(
            sampling.sample_count,
            sampling.sample_seed,
            chunk_store,
            proof_source,
            por_root,
            &mut seen,
        )?);
        proofs.extend(self.explicit_proofs(chunk_store, proof_source, por_root, &mut seen)?);
        if proofs.is_empty() {
            return Err(eyre!(
                "no proofs were generated; provide --sample-count or --leaf-index"
            ));
        }
        Ok(proofs)
    }

    fn sampled_proofs(
        sample_count: usize,
        sample_seed: u64,
        chunk_store: &ChunkStore,
        proof_source: &mut FilePayload,
        por_root: &[u8; 32],
        seen: &mut HashSet<usize>,
    ) -> Result<Vec<ProofReport>> {
        if sample_count == 0 {
            return Ok(Vec::new());
        }
        let sampled = chunk_store
            .sample_leaves_with(sample_count, sample_seed, proof_source)
            .wrap_err("failed to sample PoR leaves")?;
        let mut proofs = Vec::with_capacity(sampled.len());
        for (leaf_index, proof) in sampled {
            if !seen.insert(leaf_index) {
                continue;
            }
            let verified = proof.verify(por_root);
            proofs.push(ProofReport {
                origin: ProofOrigin::Sampled,
                leaf_index,
                proof,
                verified,
            });
        }
        Ok(proofs)
    }

    fn explicit_proofs(
        &self,
        chunk_store: &ChunkStore,
        proof_source: &mut FilePayload,
        por_root: &[u8; 32],
        seen: &mut HashSet<usize>,
    ) -> Result<Vec<ProofReport>> {
        if self.leaf_indexes.is_empty() {
            return Ok(Vec::new());
        }
        let tree = chunk_store.por_tree();
        let leaf_count = tree.leaf_count();
        let mut proofs = Vec::new();
        for &leaf_index in &self.leaf_indexes {
            if leaf_index >= leaf_count {
                return Err(eyre!(
                    "leaf-index {} out of range (tree tracks {leaf_count} leaves)",
                    leaf_index
                ));
            }
            if !seen.insert(leaf_index) {
                continue;
            }
            let (chunk_idx, segment_idx, inner_idx) = tree
                .leaf_path(leaf_index)
                .ok_or_else(|| eyre!("failed to resolve leaf-index {leaf_index}"))?;
            let proof = tree
                .prove_leaf_with(chunk_idx, segment_idx, inner_idx, proof_source)
                .wrap_err_with(|| format!("failed to build PoR proof for leaf-index {leaf_index}"))?
                .ok_or_else(|| eyre!("missing PoR proof for leaf-index {leaf_index}"))?;
            let verified = proof.verify(por_root);
            proofs.push(ProofReport {
                origin: ProofOrigin::Explicit,
                leaf_index,
                proof,
                verified,
            });
        }
        Ok(proofs)
    }
}

impl ProveAvailabilityArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.gateway_provider.is_empty() {
            return Err(eyre!("at least one --gateway-provider must be supplied"));
        }

        let normalized_ticket = normalize_ticket_hex(&self.storage_ticket)?;
        let artifact_root = default_prove_artifact_root(self.artifact_dir.clone())?;
        fs::create_dir_all(&artifact_root).wrap_err_with(|| {
            format!(
                "failed to create prove-availability artifact directory `{}`",
                artifact_root.display()
            )
        })?;

        let manifest_target_dir = self
            .manifest_cache_dir
            .clone()
            .unwrap_or_else(|| artifact_root.clone());
        let normalized_block_hash = if let Some(block_hash) = self.block_hash.as_ref() {
            Some(normalize_block_hash_hex(block_hash)?)
        } else {
            None
        };
        let fetcher = DaManifestFetcher::new(context.config(), self.manifest_endpoint.as_deref())?;
        let bundle = fetcher.fetch(&normalized_ticket, normalized_block_hash.as_deref())?;
        let plan_sampling = if normalized_block_hash.is_none() {
            bundle
                .sampling_plan_typed
                .as_ref()
                .map(sampling_from_plan)
                .transpose()?
        } else {
            None
        };
        let persisted = persist_manifest_bundle(
            context,
            &bundle,
            Some(manifest_target_dir.clone()),
            &normalized_ticket,
        )?;
        if let Some(path) = &persisted.sampling_plan {
            context.println(format_args!(
                "sampling plan JSON saved to `{}`",
                path.display()
            ))?;
        }

        let payload_path = artifact_root.join("payload.car");
        let scoreboard_path = self
            .scoreboard_out
            .clone()
            .unwrap_or_else(|| artifact_root.join("scoreboard.json"));
        if let Some(parent) = scoreboard_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create scoreboard directory `{}`",
                    parent.display()
                )
            })?;
        }

        let fetch_args = FetchArgs {
            manifest: Some(persisted.manifest.clone()),
            plan: Some(persisted.chunk_plan.clone()),
            manifest_id: Some(bundle.manifest_hash_hex.clone()),
            gateway_provider: self.gateway_provider.clone(),
            storage_ticket: None,
            manifest_endpoint: None,
            manifest_cache_dir: None,
            client_id: Some(format!("prove-availability:{normalized_ticket}")),
            manifest_envelope: None,
            manifest_cid: None,
            blinded_cid: None,
            salt_epoch: None,
            salt_hex: None,
            chunker_handle: None,
            max_peers: self.max_peers,
            retry_budget: None,
            transport_policy: None,
            anonymity_policy: None,
            write_mode: None,
            transport_policy_override: None,
            anonymity_policy_override: None,
            guard_cache: None,
            guard_cache_key: None,
            guard_directory: None,
            guard_target: None,
            guard_retention_days: None,
            output: Some(payload_path.clone()),
            json_out: None,
            scoreboard_out: Some(scoreboard_path.clone()),
            scoreboard_now: None,
            telemetry_source_label: None,
            telemetry_region: None,
        };
        fetch_args.run(context)?;

        context.println(format_args!(
            "downloaded payload to `{}`; scoreboard persisted at `{}`",
            payload_path.display(),
            scoreboard_path.display()
        ))?;
        if let Some(sampling) = plan_sampling {
            context.println(format_args!(
                "using Torii sampling plan: samples={} seed=0x{:016x}",
                sampling.sample_count, sampling.sample_seed
            ))?;
        }

        let prove_args = ProveArgs {
            manifest: persisted.manifest,
            payload: payload_path,
            json_out: self.json_out,
            sample_count: plan_sampling.map_or(self.sample_count, |sampling| sampling.sample_count),
            sample_seed: plan_sampling.map_or(self.sample_seed, |sampling| sampling.sample_seed),
            block_hash: normalized_block_hash,
            leaf_indexes: self.leaf_indexes,
        };
        prove_args.run(context)
    }
}

#[derive(Args, Debug)]
pub struct ProveAvailabilityArgs {
    /// Storage ticket issued by Torii (hex string).
    #[arg(long = "storage-ticket", value_name = "HEX")]
    pub storage_ticket: String,
    /// Gateway provider descriptor reused by `sorafs fetch` (name=... , provider-id=... , base-url=... , stream-token=...).
    #[arg(long = "gateway-provider", value_name = "SPEC", required = true)]
    pub gateway_provider: Vec<String>,
    /// Optional override for Torii manifest endpoint.
    #[arg(long = "manifest-endpoint", value_name = "URL")]
    pub manifest_endpoint: Option<String>,
    /// Directory where manifests and plans downloaded from Torii are cached (defaults to `artifacts/da/fetch_<ts>`).
    #[arg(long = "manifest-cache-dir", value_name = "PATH")]
    pub manifest_cache_dir: Option<PathBuf>,
    /// JSON output path for the combined proof summary (defaults to stdout).
    #[arg(long = "json-out", value_name = "PATH")]
    pub json_out: Option<PathBuf>,
    /// Path to persist the orchestrator scoreboard (defaults to temp dir if omitted).
    #[arg(long = "scoreboard-out", value_name = "PATH")]
    pub scoreboard_out: Option<PathBuf>,
    /// Optional limit on concurrent provider downloads.
    #[arg(long = "max-peers", value_name = "COUNT")]
    pub max_peers: Option<usize>,
    /// Proof sampling count for `PoR` verification (defaults to 8, set 0 to disable random sampling).
    #[arg(long = "sample-count", default_value_t = 8)]
    pub sample_count: usize,
    /// Seed used for deterministic `PoR` sampling during verification.
    #[arg(long = "sample-seed", default_value_t = 0)]
    pub sample_seed: u64,
    /// Optional block hash used to derive deterministic sampling (overrides sample-count/seed).
    #[arg(long = "block-hash", value_name = "HEX")]
    pub block_hash: Option<String>,
    /// Explicit `PoR` leaf indexes to verify in addition to sampled values.
    #[arg(long = "leaf-index", value_name = "INDEX")]
    pub leaf_indexes: Vec<usize>,
    /// Directory for storing assembled payload/artefacts (defaults to `artifacts/da/prove_availability_<ts>`).
    #[arg(long = "artifact-dir", value_name = "PATH")]
    pub artifact_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RentQuoteArgs {
    /// Logical GiB stored in the blob (post-chunking).
    #[arg(long = "gib", value_name = "GIB")]
    pub gib: u64,
    /// Retention duration measured in months.
    #[arg(long = "months", value_name = "MONTHS")]
    pub months: u32,
    /// Optional path to a JSON-encoded `DaRentPolicyV1`.
    #[arg(long = "policy-json", value_name = "PATH")]
    pub policy_json: Option<PathBuf>,
    /// Optional path to a Norito-encoded `DaRentPolicyV1`.
    #[arg(long = "policy-norito", value_name = "PATH")]
    pub policy_norito: Option<PathBuf>,
    /// Optional human-readable label recorded in the quote metadata (defaults to source path).
    #[arg(long = "policy-label", value_name = "TEXT")]
    pub policy_label: Option<String>,
    /// Optional path for persisting the rendered quote JSON.
    #[arg(long = "quote-out", value_name = "PATH")]
    pub quote_out: Option<PathBuf>,
}

impl RentQuoteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.gib == 0 {
            return Err(eyre!("--gib must be greater than zero"));
        }
        if self.months == 0 {
            return Err(eyre!("--months must be greater than zero"));
        }
        let (policy, auto_label) = load_rent_policy_from_paths(
            self.policy_json.as_deref(),
            self.policy_norito.as_deref(),
        )?;
        let source_label = policy_label_or_default(self.policy_label.as_deref(), auto_label)?;
        let quote = policy
            .quote(self.gib, self.months)
            .wrap_err("failed to compute rent quote")?;
        let value = build_rent_quote_value(&policy, self.gib, self.months, &quote, &source_label)?;
        if let Some(path) = self.quote_out.as_deref() {
            write_rent_quote_artifact(path, &value)?;
        }
        context.println(format_rent_quote_summary(&quote))?;
        context.print_data(&value)
    }
}

#[derive(clap::Args, Debug)]
pub struct RentLedgerArgs {
    /// Path to the rent quote JSON file (output of `iroha da rent-quote`).
    #[arg(long = "quote", value_name = "PATH")]
    pub quote_path: PathBuf,
    /// Account responsible for paying the rent and funding bonus pools.
    #[arg(long = "payer-account", value_name = "ACCOUNT@DOMAIN")]
    pub payer_account: String,
    /// Treasury or escrow account receiving the base rent before distribution.
    #[arg(long = "treasury-account", value_name = "ACCOUNT@DOMAIN")]
    pub treasury_account: String,
    /// Protocol reserve account that receives the configured reserve share.
    #[arg(long = "protocol-reserve-account", value_name = "ACCOUNT@DOMAIN")]
    pub protocol_reserve_account: String,
    /// Provider payout account that receives the base rent remainder.
    #[arg(long = "provider-account", value_name = "ACCOUNT@DOMAIN")]
    pub provider_account: String,
    /// Account earmarked for PDP bonus payouts.
    #[arg(long = "pdp-bonus-account", value_name = "ACCOUNT@DOMAIN")]
    pub pdp_bonus_account: String,
    /// Account earmarked for `PoTR` bonus payouts.
    #[arg(long = "potr-bonus-account", value_name = "ACCOUNT@DOMAIN")]
    pub potr_bonus_account: String,
    /// Asset definition identifier used for XOR transfers (e.g., `xor#sora`).
    #[arg(long = "asset-definition", value_name = "NAME#DOMAIN")]
    pub asset_definition: String,
}

impl RentLedgerArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let quote_contents = fs::read_to_string(&self.quote_path).wrap_err_with(|| {
            format!("failed to read rent quote `{}`", self.quote_path.display())
        })?;
        let quote_value: Value =
            json::from_str(&quote_contents).wrap_err("failed to parse rent quote JSON")?;
        let projection = extract_rent_ledger_projection(&quote_value)?;
        let payer: AccountId = self
            .payer_account
            .parse()
            .wrap_err("failed to parse --payer-account")?;
        let treasury: AccountId = self
            .treasury_account
            .parse()
            .wrap_err("failed to parse --treasury-account")?;
        let protocol_reserve: AccountId = self
            .protocol_reserve_account
            .parse()
            .wrap_err("failed to parse --protocol-reserve-account")?;
        let provider: AccountId = self
            .provider_account
            .parse()
            .wrap_err("failed to parse --provider-account")?;
        let pdp_bonus: AccountId = self
            .pdp_bonus_account
            .parse()
            .wrap_err("failed to parse --pdp-bonus-account")?;
        let potr_bonus: AccountId = self
            .potr_bonus_account
            .parse()
            .wrap_err("failed to parse --potr-bonus-account")?;
        let asset_definition: AssetDefinitionId = self
            .asset_definition
            .parse()
            .wrap_err("failed to parse --asset-definition")?;
        let accounts = da::DaRentLedgerAccounts {
            payer: &payer,
            treasury: &treasury,
            protocol_reserve: &protocol_reserve,
            provider: &provider,
            pdp_bonus: &pdp_bonus,
            potr_bonus: &potr_bonus,
        };
        let plan =
            build_rent_ledger_plan(&self.quote_path, projection, accounts, &asset_definition)?;
        context.print_data(&plan)
    }
}

fn build_da_params(args: &SubmitArgs) -> Result<DaIngestParams> {
    let blob_class = parse_blob_class(&args.blob_class)?;
    let blob_codec = BlobCodec::new(args.blob_codec.clone());
    let fec_scheme = parse_fec_scheme(&args.fec_scheme)?;
    let erasure_profile = ErasureProfile {
        data_shards: args.data_shards,
        parity_shards: args.parity_shards,
        row_parity_stripes: 0,
        chunk_alignment: args.chunk_alignment,
        fec_scheme,
    };
    let storage_class = parse_storage_class(&args.storage_class)?;
    let retention_policy = RetentionPolicy {
        hot_retention_secs: args.hot_retention_secs,
        cold_retention_secs: args.cold_retention_secs,
        required_replicas: args.required_replicas,
        storage_class,
        governance_tag: GovernanceTag::new(args.governance_tag.clone()),
    };
    let client_blob_id = match &args.client_blob_id {
        Some(hex) => Some(BlobDigest::new(parse_fixed_hex(hex, "client-blob-id")?)),
        None => None,
    };
    Ok(DaIngestParams {
        lane_id: LaneId::new(args.lane_id),
        epoch: args.epoch,
        sequence: args.sequence,
        blob_class,
        blob_codec,
        erasure_profile,
        retention_policy,
        chunk_size: args.chunk_size,
        client_blob_id,
    })
}

fn default_artifact_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut root = std::env::current_dir().wrap_err("failed to determine current directory")?;
    root.push("artifacts");
    root.push("da");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?
        .as_secs();
    root.push(format!("submission_{stamp}"));
    Ok(root)
}

fn default_fetch_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut root = std::env::current_dir().wrap_err("failed to determine current directory")?;
    root.push("artifacts");
    root.push("da");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?
        .as_secs();
    root.push(format!("fetch_{stamp}"));
    Ok(root)
}

fn default_prove_artifact_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut root = std::env::current_dir().wrap_err("failed to determine current directory")?;
    root.push("artifacts");
    root.push("da");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?
        .as_secs();
    root.push(format!("prove_availability_{stamp}"));
    Ok(root)
}

pub(super) fn normalize_ticket_hex(input: &str) -> Result<String> {
    if input.trim().is_empty() {
        return Err(eyre!("--storage-ticket must be provided"));
    }
    let trimmed = input
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.len() != 64 {
        return Err(eyre!(
            "--storage-ticket must contain 64 hexadecimal characters (got {})",
            trimmed.len()
        ));
    }
    hex::decode(trimmed).map_err(|err| eyre!("invalid storage ticket hex: {err}"))?;
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_block_hash_hex(input: &str) -> Result<String> {
    if input.trim().is_empty() {
        return Err(eyre!("--block-hash must be provided when set"));
    }
    let trimmed = input
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.len() != 64 {
        return Err(eyre!(
            "--block-hash must contain 64 hexadecimal characters (got {})",
            trimmed.len()
        ));
    }
    hex::decode(trimmed).map_err(|err| eyre!("invalid block hash hex: {err}"))?;
    Ok(trimmed.to_ascii_lowercase())
}

#[derive(Debug)]
pub(super) struct PersistedManifestPaths {
    pub(super) manifest: PathBuf,
    pub(super) chunk_plan: PathBuf,
    pub(super) sampling_plan: Option<PathBuf>,
}

pub(super) fn persist_manifest_bundle<C: RunContext>(
    context: &mut C,
    bundle: &DaManifestFetchBundle,
    output_dir: Option<PathBuf>,
    ticket_label: &str,
) -> Result<PersistedManifestPaths> {
    let root = default_fetch_root(output_dir)?;
    fs::create_dir_all(&root).wrap_err_with(|| {
        format!(
            "failed to create manifest fetch directory `{}`",
            root.display()
        )
    })?;
    let manifest_path = root.join(format!("manifest_{ticket_label}.norito"));
    let manifest_json_path = root.join(format!("manifest_{ticket_label}.json"));
    let chunk_plan_path = root.join(format!("chunk_plan_{ticket_label}.json"));
    let sampling_plan_path = bundle
        .sampling_plan
        .as_ref()
        .map(|_| root.join(format!("sampling_plan_{ticket_label}.json")));

    fs::write(&manifest_path, &bundle.manifest_bytes)
        .wrap_err_with(|| format!("failed to write `{}`", manifest_path.display()))?;
    let manifest_json = norito::json::to_json_pretty(&bundle.manifest_json)
        .wrap_err("failed to render manifest JSON")?;
    fs::write(&manifest_json_path, manifest_json)
        .wrap_err_with(|| format!("failed to write `{}`", manifest_json_path.display()))?;
    let chunk_plan_json = norito::json::to_json_pretty(&bundle.chunk_plan)
        .wrap_err("failed to render chunk plan JSON")?;
    fs::write(&chunk_plan_path, chunk_plan_json)
        .wrap_err_with(|| format!("failed to write `{}`", chunk_plan_path.display()))?;
    if let Some(path) = sampling_plan_path.as_ref() {
        let sampling_json =
            norito::json::to_json_pretty(bundle.sampling_plan.as_ref().unwrap_or(&Value::Null))
                .wrap_err("failed to render sampling plan JSON")?;
        fs::write(path, sampling_json)
            .wrap_err_with(|| format!("failed to write `{}`", path.display()))?;
    }

    context.println(format_args!(
        "wrote manifest bytes to `{}`",
        manifest_path.display()
    ))?;
    context.println(format_args!(
        "wrote manifest JSON to `{}`",
        manifest_json_path.display()
    ))?;
    context.println(format_args!(
        "wrote chunk plan JSON to `{}`",
        chunk_plan_path.display()
    ))?;
    context.println(format_args!(
        "blob hash: {}",
        bundle.blob_hash_hex
    ))?;
    context.println(format_args!(
        "manifest hash (blob id): {}",
        bundle.manifest_hash_hex
    ))?;
    context.println(format_args!(
        "storage ticket: {}",
        bundle.storage_ticket_hex
    ))?;
    context.println(format_args!(
        "next step: `iroha da get --manifest {manifest} --plan {plan} --manifest-id {blob}`",
        manifest = manifest_path.display(),
        plan = chunk_plan_path.display(),
        blob = bundle.manifest_hash_hex,
    ))?;
    Ok(PersistedManifestPaths {
        manifest: manifest_path,
        chunk_plan: chunk_plan_path,
        sampling_plan: sampling_plan_path,
    })
}

fn persist_receipt_headers(root: &Path, header_value: &str) -> Result<()> {
    let mut headers = Map::new();
    headers.insert(
        "sora-pdp-commitment".to_string(),
        Value::from(header_value.to_string()),
    );
    let rendered = norito::json::to_json_pretty(&Value::Object(headers))
        .map_err(|err| eyre!("failed to render DA response header JSON: {err}"))?;
    let headers_path = root.join("da_response_headers.json");
    fs::write(&headers_path, rendered).wrap_err_with(|| {
        format!(
            "failed to write DA response headers `{}`",
            headers_path.display()
        )
    })
}

fn load_da_receipt_fixture(path: &Path) -> Result<DaPublisherReceipt> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read DA receipt fixture `{}`", path.display()))?;
    let value: Value = norito::json::from_slice(&bytes).map_err(|err| {
        eyre!(
            "failed to parse DA receipt fixture `{}` as JSON: {err}",
            path.display()
        )
    })?;
    let receipt_b64 = value
        .get("receipt_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "receipt fixture `{}` is missing `receipt_base64` field",
                path.display()
            )
        })?;
    let receipt_bytes = Base64Standard
        .decode(receipt_b64.as_bytes())
        .map_err(|err| {
            eyre!(
                "receipt fixture `{}` contains invalid base64 payload: {err}",
                path.display()
            )
        })?;
    let receipt: DaIngestReceipt = decode_from_bytes(&receipt_bytes).map_err(|err| {
        eyre!(
            "failed to decode DA receipt from fixture `{}`: {err}",
            path.display()
        )
    })?;
    let json = norito::json::to_json_pretty(&receipt).map_err(|err| {
        eyre!(
            "failed to render DA receipt JSON from fixture `{}`: {err}",
            path.display()
        )
    })?;
    let header_value = value
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| headers.get("sora-pdp-commitment"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(DaPublisherReceipt {
        bytes: receipt_bytes,
        json,
        receipt,
        pdp_commitment_header: header_value,
    })
}

fn parse_fixed_hex(label: &str, field: &str) -> Result<[u8; 32]> {
    let trimmed = label.strip_prefix("0x").unwrap_or(label);
    let bytes =
        hex::decode(trimmed).map_err(|err| eyre!("invalid hex for `{field}` `{label}`: {err}"))?;
    let byte_len = bytes.len();
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| eyre!("`{field}` must decode to 32 bytes, got {byte_len}"))?;
    Ok(array)
}

fn load_manifest(path: &Path) -> Result<DaManifestV1> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read manifest `{}`", path.display()))?;
    decode_from_bytes(&bytes)
        .map_err(|err| eyre!("failed to decode manifest `{}`: {err}", path.display()))
}

#[cfg(test)]
fn chunk_profile_from_chunk_size(chunk_size: u32) -> Result<ChunkProfile> {
    if chunk_size == 0 {
        return Err(eyre!("manifest chunk_size must be non-zero"));
    }
    let size = usize::try_from(chunk_size).map_err(|_| eyre!("chunk_size exceeds host limits"))?;
    Ok(ChunkProfile {
        min_size: size,
        target_size: size,
        max_size: size,
        break_mask: 1,
    })
}

fn build_car_plan_from_manifest(manifest: &DaManifestV1) -> Result<CarBuildPlan> {
    sorafs_car::build_plan_from_da_manifest(manifest).map_err(|err| eyre!(err))
}

fn validate_manifest_consistency(manifest: &DaManifestV1, store: &ChunkStore) -> Result<()> {
    let blob_hash_bytes = manifest.blob_hash.as_ref();
    if store.payload_digest().as_bytes() != blob_hash_bytes {
        return Err(eyre!(
            "payload hash mismatch: manifest={} computed={}",
            hex::encode(blob_hash_bytes),
            hex::encode(store.payload_digest().as_bytes())
        ));
    }
    let chunk_root_bytes = manifest.chunk_root.as_ref();
    if store.por_tree().root() != chunk_root_bytes {
        return Err(eyre!(
            "chunk root mismatch: manifest={} computed={}",
            hex::encode(chunk_root_bytes),
            hex::encode(store.por_tree().root())
        ));
    }
    Ok(())
}

#[derive(Debug)]
enum ProofOrigin {
    Sampled,
    Explicit,
}

impl ProofOrigin {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Sampled => "sampled",
            Self::Explicit => "explicit",
        }
    }
}

struct ProofReport {
    origin: ProofOrigin,
    leaf_index: usize,
    proof: PorProof,
    verified: bool,
}

#[derive(Clone, Copy)]
struct ProofSampling {
    sample_count: usize,
    sample_seed: u64,
}

fn sampling_from_plan(plan: &da::DaSamplingPlan) -> Result<ProofSampling> {
    let seed_bytes = plan
        .sample_seed
        .ok_or_else(|| eyre!("sampling plan missing sample_seed"))?;
    let seed_prefix: [u8; 8] = seed_bytes[..8]
        .try_into()
        .expect("sample_seed length enforced");
    let sample_count = plan.samples.len();
    if sample_count == 0 {
        return Err(eyre!("sampling plan contained no samples"));
    }
    Ok(ProofSampling {
        sample_count,
        sample_seed: u64::from_le_bytes(seed_prefix),
    })
}

struct ProofSummaryInputs<'a> {
    manifest: &'a DaManifestV1,
    manifest_path: &'a Path,
    payload_path: &'a Path,
    por_root_hex: String,
    leaf_total: usize,
    segment_total: usize,
    chunk_total: usize,
    sample_count: usize,
    sample_seed: u64,
}

fn build_proof_summary(inputs: ProofSummaryInputs<'_>, proofs: &[ProofReport]) -> Value {
    let mut map = Map::new();
    map.insert(
        "manifest_path".into(),
        Value::from(inputs.manifest_path.display().to_string()),
    );
    map.insert(
        "payload_path".into(),
        Value::from(inputs.payload_path.display().to_string()),
    );
    map.insert(
        "blob_hash".into(),
        Value::from(hex::encode(inputs.manifest.blob_hash.as_ref())),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(inputs.manifest.chunk_root.as_ref())),
    );
    map.insert("por_root".into(), Value::from(inputs.por_root_hex));
    map.insert("leaf_count".into(), value_from_usize(inputs.leaf_total));
    map.insert(
        "segment_count".into(),
        value_from_usize(inputs.segment_total),
    );
    map.insert("chunk_count".into(), value_from_usize(inputs.chunk_total));
    map.insert("sample_count".into(), value_from_usize(inputs.sample_count));
    map.insert("sample_seed".into(), Value::from(inputs.sample_seed));
    map.insert("proof_count".into(), value_from_usize(proofs.len()));
    let proof_values = proofs.iter().map(proof_to_json).collect::<Vec<_>>();
    map.insert("proofs".into(), Value::Array(proof_values));
    Value::Object(map)
}

fn proof_to_json(report: &ProofReport) -> Value {
    let mut map = Map::new();
    map.insert("origin".into(), Value::from(report.origin.as_str()));
    map.insert("leaf_index".into(), value_from_usize(report.leaf_index));
    map.insert(
        "chunk_index".into(),
        value_from_usize(report.proof.chunk_index),
    );
    map.insert(
        "segment_index".into(),
        value_from_usize(report.proof.segment_index),
    );
    map.insert("leaf_offset".into(), Value::from(report.proof.leaf_offset));
    map.insert(
        "leaf_length".into(),
        value_from_u32(report.proof.leaf_length),
    );
    map.insert(
        "segment_offset".into(),
        Value::from(report.proof.segment_offset),
    );
    map.insert(
        "segment_length".into(),
        value_from_u32(report.proof.segment_length),
    );
    map.insert(
        "chunk_offset".into(),
        Value::from(report.proof.chunk_offset),
    );
    map.insert(
        "chunk_length".into(),
        value_from_u32(report.proof.chunk_length),
    );
    map.insert("payload_len".into(), Value::from(report.proof.payload_len));
    map.insert(
        "chunk_digest".into(),
        Value::from(hex::encode(report.proof.chunk_digest)),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(report.proof.chunk_root)),
    );
    map.insert(
        "segment_digest".into(),
        Value::from(hex::encode(report.proof.segment_digest)),
    );
    map.insert(
        "leaf_digest".into(),
        Value::from(hex::encode(report.proof.leaf_digest)),
    );
    map.insert(
        "leaf_bytes_b64".into(),
        Value::from(Base64Standard.encode(&report.proof.leaf_bytes)),
    );
    map.insert(
        "segment_leaves".into(),
        Value::Array(
            report
                .proof
                .segment_leaves
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_segments".into(),
        Value::Array(
            report
                .proof
                .chunk_segments
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_roots".into(),
        Value::Array(
            report
                .proof
                .chunk_roots
                .iter()
                .map(|digest| Value::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert("verified".into(), Value::from(report.verified));
    Value::Object(map)
}

fn load_rent_policy_from_paths(
    json_path: Option<&Path>,
    norito_path: Option<&Path>,
) -> Result<(DaRentPolicyV1, String)> {
    match (json_path, norito_path) {
        (Some(_), Some(_)) => Err(eyre!(
            "only one of --policy-json or --policy-norito may be supplied"
        )),
        (Some(path), None) => {
            let contents = fs::read_to_string(path).wrap_err_with(|| {
                format!("failed to read rent policy JSON `{}`", path.display())
            })?;
            let policy: DaRentPolicyV1 =
                json::from_str(&contents).wrap_err("failed to parse rent policy JSON")?;
            Ok((policy, format!("policy JSON `{}`", path.display())))
        }
        (None, Some(path)) => {
            let bytes = fs::read(path).wrap_err_with(|| {
                format!("failed to read rent policy Norito `{}`", path.display())
            })?;
            let policy = decode_from_bytes::<DaRentPolicyV1>(&bytes)
                .wrap_err("failed to decode rent policy Norito bytes")?;
            Ok((policy, format!("policy Norito `{}`", path.display())))
        }
        (None, None) => Ok((
            DaRentPolicyV1::default(),
            "embedded default policy".to_string(),
        )),
    }
}

fn policy_label_or_default(custom: Option<&str>, default_label: String) -> Result<String> {
    custom.map_or_else(
        || Ok(default_label),
        |label| {
            let trimmed = label.trim();
            if trimmed.is_empty() {
                Err(eyre!("--policy-label must not be empty"))
            } else {
                Ok(trimmed.to_string())
            }
        },
    )
}

fn format_rent_quote_summary(quote: &DaRentQuote) -> String {
    format!(
        "rent_quote base={} reserve={} provider={} pdp_bonus={} potr_bonus={} egress_credit_per_gib={}",
        format_xor_amount(quote.base_rent, "XOR", "μXOR"),
        format_xor_amount(quote.protocol_reserve, "XOR", "μXOR"),
        format_xor_amount(quote.provider_reward, "XOR", "μXOR"),
        format_xor_amount(quote.pdp_bonus, "XOR", "μXOR"),
        format_xor_amount(quote.potr_bonus, "XOR", "μXOR"),
        format_xor_amount(quote.egress_credit_per_gib, "XOR/GiB", "μXOR/GiB"),
    )
}

fn format_xor_amount(amount: XorAmount, unit: &str, micro_unit: &str) -> String {
    let micro = amount.as_micro();
    let whole = micro / MICRO_XOR_PER_XOR;
    let fractional = micro % MICRO_XOR_PER_XOR;
    format!("{whole}.{fractional:06} {unit} ({micro} {micro_unit})")
}

fn build_rent_quote_value(
    policy: &DaRentPolicyV1,
    gib: u64,
    months: u32,
    quote: &DaRentQuote,
    source_label: &str,
) -> Result<Value> {
    let mut map = Map::new();
    map.insert(
        "policy_source".into(),
        Value::from(source_label.to_string()),
    );
    map.insert("gib".into(), Value::Number(Number::from(gib)));
    map.insert(
        "months".into(),
        Value::Number(Number::from(u64::from(months))),
    );
    let policy_value =
        json::to_value(policy).wrap_err("failed to serialize DA rent policy to JSON")?;
    map.insert("policy".into(), policy_value);
    let quote_value =
        json::to_value(quote).wrap_err("failed to serialize DA rent quote to JSON")?;
    map.insert("quote".into(), quote_value);
    let projection_value = json::to_value(&quote.ledger_projection())
        .wrap_err("failed to serialize DA rent ledger projection to JSON")?;
    map.insert("ledger_projection".into(), projection_value);
    Ok(Value::Object(map))
}

fn extract_rent_ledger_projection(value: &Value) -> Result<DaRentLedgerProjection> {
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("rent quote must be a JSON object"))?;
    let ledger_value = root
        .get("ledger_projection")
        .ok_or_else(|| eyre!("rent quote missing `ledger_projection` block"))?;
    json::from_value(ledger_value.clone())
        .wrap_err("failed to decode DA rent ledger projection from quote")
}

fn build_rent_ledger_plan(
    quote_path: &Path,
    projection: DaRentLedgerProjection,
    accounts: da::DaRentLedgerAccounts<'_>,
    asset_definition: &AssetDefinitionId,
) -> Result<Value> {
    let plan = da::build_da_rent_ledger_plan(&projection, &accounts, asset_definition)?;
    render_rent_ledger_plan(quote_path, &plan)
}

fn render_rent_ledger_plan(quote_path: &Path, plan: &DaRentLedgerPlan) -> Result<Value> {
    let mut serialized_instructions = Vec::with_capacity(plan.instructions.len());
    for instruction in &plan.instructions {
        let rendered = norito::json::to_value(instruction)
            .wrap_err("failed to serialize rent ledger transfer instruction")?;
        serialized_instructions.push(rendered);
    }
    let mut map = Map::new();
    map.insert(
        "quote_path".into(),
        Value::from(quote_path.display().to_string()),
    );
    map.insert(
        "rent_due_micro_xor".into(),
        ledger_micro_value(plan.rent_due),
    );
    map.insert(
        "protocol_reserve_due_micro_xor".into(),
        ledger_micro_value(plan.protocol_reserve_due),
    );
    map.insert(
        "provider_reward_due_micro_xor".into(),
        ledger_micro_value(plan.provider_reward_due),
    );
    map.insert(
        "pdp_bonus_pool_micro_xor".into(),
        ledger_micro_value(plan.pdp_bonus_pool),
    );
    map.insert(
        "potr_bonus_pool_micro_xor".into(),
        ledger_micro_value(plan.potr_bonus_pool),
    );
    map.insert(
        "egress_credit_per_gib_micro_xor".into(),
        ledger_micro_value(plan.egress_credit_per_gib),
    );
    map.insert("instructions".into(), Value::Array(serialized_instructions));
    Ok(Value::Object(map))
}

fn ledger_micro_value(amount: XorAmount) -> Value {
    let micro = amount.as_micro();
    u64::try_from(micro).map_or_else(
        |_| Value::String(micro.to_string()),
        |value| Value::Number(Number::from(value)),
    )
}

fn write_rent_quote_artifact(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!(
                "failed to create rent quote artifact directory `{}`",
                parent.display()
            )
        })?;
    }
    let rendered = json::to_json_pretty(value).wrap_err("serialize rent quote artifact")?;
    fs::write(path, rendered)
        .wrap_err_with(|| format!("failed to write rent quote artifact `{}`", path.display()))
}

fn value_from_usize(value: usize) -> Value {
    Value::from(u64::try_from(value).unwrap_or(u64::MAX))
}

fn value_from_u32(value: u32) -> Value {
    Value::from(u64::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::{
        config::{self, Config},
        crypto::KeyPair,
        data_model::{
            Metadata,
            domain::DomainId,
            prelude::{AccountId, ChainId},
        },
    };
    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::da::{
        manifest::{ChunkCommitment, ChunkRole},
        types::{
            BlobClass, BlobDigest, DaRentQuote, ErasureProfile, ExtraMetadata, FecScheme,
            GovernanceTag, MetadataEntry, MetadataVisibility, RetentionPolicy, StorageTicketId,
        },
    };
    use iroha_data_model::sorafs::pin_registry::StorageClass;
    use iroha_i18n::{Bundle, Language, Localizer};
    use iroha_torii_shared::da::sampling::{build_sampling_plan, sampling_plan_to_value};
    use norito::{json::JsonSerialize, to_bytes};
    use sorafs_manifest::deal::XorAmount;
    use std::{fmt::Display, fs, path::PathBuf};
    use tempfile::{NamedTempFile, tempdir};
    use url::Url;

    struct TestContext {
        cfg: Config,
        printed: Vec<String>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::random();
            let account: AccountId = format!("{}@wonderland", key_pair.public_key())
                .parse()
                .expect("valid account");
            let cfg = Config {
                chain: ChainId::from("test-chain"),
                account,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://localhost/").expect("url"),
                torii_api_version: config::default_torii_api_version(),
                torii_api_min_proof_version: config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &Config {
            &self.cfg
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

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            let bytes = norito::json::to_vec(data)?;
            let out = String::from_utf8(bytes).map_err(|err| eyre!(err.to_string()))?;
            self.printed.push(out);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn parse_fixed_hex_rejects_short_values() {
        let err = parse_fixed_hex("00ff", "demo").expect_err("expected failure");
        assert!(
            err.to_string().contains("must decode to 32 bytes"),
            "{err:?}"
        );
    }

    #[test]
    fn default_artifact_root_builds_path() {
        let root = default_artifact_root(None).expect("root");
        let display = root.to_string_lossy();
        assert!(
            display.contains("artifacts/da") || display.contains("artifacts\\da"),
            "unexpected root: {root:?}"
        );
    }

    #[test]
    fn chunk_profile_from_chunk_size_rejects_zero() {
        let err = chunk_profile_from_chunk_size(0).expect_err("expected failure");
        assert!(
            err.to_string().contains("chunk_size must be non-zero"),
            "{err:?}"
        );
    }

    #[test]
    fn car_plan_matches_manifest_chunks() {
        let manifest = sample_manifest();
        let plan = build_car_plan_from_manifest(&manifest).expect("plan");
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
        assert_eq!(plan.content_length, manifest.total_size);
        assert_eq!(plan.payload_digest.as_bytes(), manifest.blob_hash.as_ref());
    }

    #[test]
    fn proof_summary_renders_expected_fields() {
        let manifest = sample_manifest();
        let proof = ProofReport {
            origin: ProofOrigin::Sampled,
            leaf_index: 0,
            proof: dummy_proof(),
            verified: true,
        };
        let proofs = vec![proof];
        let summary_inputs = ProofSummaryInputs {
            manifest: &manifest,
            manifest_path: Path::new("manifest.to"),
            payload_path: Path::new("payload.bin"),
            por_root_hex: "abcd".into(),
            leaf_total: 1,
            segment_total: 1,
            chunk_total: 1,
            sample_count: 1,
            sample_seed: 42,
        };
        let summary = build_proof_summary(summary_inputs, &proofs);
        let map = summary
            .as_object()
            .expect("summary should be a JSON object");
        assert_eq!(map.get("proof_count").and_then(Value::as_u64), Some(1));
        assert_eq!(map.get("sample_seed").and_then(Value::as_u64), Some(42));
        let proofs = map
            .get("proofs")
            .and_then(Value::as_array)
            .expect("proofs array");
        assert_eq!(proofs.len(), 1);
    }

    #[test]
    fn prove_artifact_root_prefers_explicit_path() {
        let expected = PathBuf::from("/tmp/prove_root_custom");
        let actual = default_prove_artifact_root(Some(expected.clone())).expect("root");
        assert_eq!(actual, expected);
    }

    #[test]
    fn prove_artifact_root_generates_timestamped_directory() {
        let path = default_prove_artifact_root(None).expect("root");
        let rendered = path.to_string_lossy();
        assert!(
            rendered.contains("prove_availability_"),
            "unexpected prove root: {rendered}"
        );
    }

    #[test]
    fn rent_policy_json_source_loads() {
        let policy = DaRentPolicyV1::from_components(500_000, 1_000, 250, 125, 2_500);
        let tmp = NamedTempFile::new().expect("temp file");
        let rendered = norito::json::to_json_pretty(&policy).expect("render JSON for rent policy");
        fs::write(tmp.path(), rendered).expect("write JSON");
        let (loaded, label) = load_rent_policy_from_paths(Some(tmp.path()), None).expect("policy");
        assert_eq!(loaded, policy);
        assert!(
            label.contains("policy JSON"),
            "expected JSON label, got {label}"
        );
    }

    #[test]
    fn rent_policy_norito_source_loads() {
        let policy = DaRentPolicyV1::from_components(100_000, 0, 0, 0, 1_000);
        let tmp = NamedTempFile::new().expect("temp file");
        let bytes = to_bytes(&policy).expect("encode policy");
        fs::write(tmp.path(), bytes).expect("write Norito");
        let (loaded, label) = load_rent_policy_from_paths(None, Some(tmp.path())).expect("policy");
        assert_eq!(loaded, policy);
        assert!(
            label.contains("policy Norito"),
            "expected Norito label, got {label}"
        );
    }

    #[test]
    fn rent_policy_rejects_dual_sources() {
        let tmp = NamedTempFile::new().expect("temp file");
        let err = load_rent_policy_from_paths(Some(tmp.path()), Some(tmp.path()))
            .expect_err("should fail");
        assert!(
            err.to_string()
                .contains("only one of --policy-json or --policy-norito"),
            "{err:?}"
        );
    }

    #[test]
    fn rent_quote_value_contains_expected_fields() {
        let policy = DaRentPolicyV1::default();
        let quote = policy.quote(2, 1).expect("quote");
        let value =
            build_rent_quote_value(&policy, 2, 1, &quote, "unit-test policy").expect("value");
        let obj = value.as_object().expect("object");
        assert_eq!(
            obj.get("policy_source").and_then(Value::as_str),
            Some("unit-test policy")
        );
        assert_eq!(obj.get("gib").and_then(Value::as_u64), Some(2));
        assert_eq!(obj.get("months").and_then(Value::as_u64), Some(1));
        assert!(obj.get("policy").is_some(), "policy missing");
        assert!(obj.get("quote").is_some(), "quote missing");
        let ledger = obj
            .get("ledger_projection")
            .and_then(Value::as_object)
            .expect("ledger projection missing");
        assert!(
            ledger.contains_key("rent_due"),
            "ledger projection must expose rent_due: {ledger:?}"
        );
    }

    #[test]
    fn rent_quote_writer_persists_json() {
        let policy = DaRentPolicyV1::default();
        let quote = policy.quote(8, 3).expect("quote");
        let value = build_rent_quote_value(&policy, 8, 3, &quote, "embedded default policy")
            .expect("value");
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("rent").join("quote.json");
        write_rent_quote_artifact(&path, &value).expect("write artifact");
        let contents = fs::read_to_string(&path).expect("rent quote contents");
        assert!(contents.contains("\"policy_source\""));
        assert!(contents.contains("embedded default policy"));
    }

    #[test]
    fn format_xor_amount_renders_fixed_precision() {
        let amount = XorAmount::from_micro(1_234_567);
        let rendered = format_xor_amount(amount, "XOR", "μXOR");
        assert_eq!(rendered, "1.234567 XOR (1234567 μXOR)");
    }

    #[test]
    fn rent_quote_summary_includes_all_fields() {
        let quote = DaRentQuote {
            base_rent: XorAmount::from_micro(250_000),
            protocol_reserve: XorAmount::from_micro(50_000),
            provider_reward: XorAmount::from_micro(200_000),
            pdp_bonus: XorAmount::from_micro(10_000),
            potr_bonus: XorAmount::from_micro(5_000),
            egress_credit_per_gib: XorAmount::from_micro(1_500),
        };
        let summary = format_rent_quote_summary(&quote);
        assert!(
            summary.contains("base=0.250000 XOR (250000 μXOR)"),
            "missing base rent text: {summary}"
        );
        assert!(
            summary.contains("egress_credit_per_gib=0.001500 XOR/GiB (1500 μXOR/GiB)"),
            "missing egress credit text: {summary}"
        );
    }

    #[test]
    fn rent_quote_run_emits_summary_and_json() {
        let mut ctx = TestContext::new();
        let args = RentQuoteArgs {
            gib: 4,
            months: 2,
            policy_json: None,
            policy_norito: None,
            policy_label: Some("demo policy".to_string()),
            quote_out: None,
        };
        args.run(&mut ctx).expect("rent quote run");
        assert_eq!(ctx.printed.len(), 2, "expected summary + JSON output");
        assert!(
            ctx.printed[0].starts_with("rent_quote base="),
            "summary should include rent base line: {}",
            ctx.printed[0]
        );
        assert!(
            ctx.printed[0].contains("egress_credit_per_gib="),
            "summary should include egress credit text: {}",
            ctx.printed[0]
        );
        let root: Value = json::from_str(&ctx.printed[1]).expect("parse quote JSON");
        assert_eq!(
            root.get("policy_source"),
            Some(&Value::String("demo policy".into()))
        );
        assert_eq!(root.get("gib").and_then(Value::as_u64), Some(4));
        assert_eq!(root.get("months").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn rent_ledger_projection_extracts_values() {
        let policy = DaRentPolicyV1::default();
        let quote = policy.quote(6, 2).expect("quote");
        let value = build_rent_quote_value(&policy, 6, 2, &quote, "embedded default policy")
            .expect("value");
        let projection = extract_rent_ledger_projection(&value).expect("projection");
        assert_eq!(projection.rent_due, quote.base_rent);
        assert_eq!(projection.protocol_reserve_due, quote.protocol_reserve);
        assert_eq!(projection.provider_reward_due, quote.provider_reward);
    }

    #[test]
    fn persist_manifest_bundle_writes_sampling_plan() {
        let manifest = sample_manifest();
        let manifest_bytes = to_bytes(&manifest).expect("encode manifest");
        let manifest_json = norito::json::to_value(&manifest).expect("manifest JSON");
        let sampling_plan = sampling_plan_to_value(&build_sampling_plan(
            &manifest,
            &Hash::new(b"sampling-plan-test"),
        ));
        let manifest_hash_hex = hex::encode(blake3_hash(&manifest_bytes).as_bytes());
        let bundle = DaManifestFetchBundle {
            manifest_bytes: manifest_bytes.clone(),
            manifest_json,
            chunk_plan: Value::Object(Map::new()),
            storage_ticket_hex: "feedface".repeat(8),
            manifest_hash_hex,
            blob_hash_hex: hex::encode(manifest.blob_hash.as_ref()),
            sampling_plan: Some(sampling_plan.clone()),
            sampling_plan_typed: None,
        };

        let dir = tempdir().expect("tempdir");
        let mut ctx = TestContext::new();
        let paths = persist_manifest_bundle(
            &mut ctx,
            &bundle,
            Some(dir.path().to_path_buf()),
            "ticket123",
        )
        .expect("persist manifest bundle");

        let sampling_path = paths
            .sampling_plan
            .expect("sampling plan path should be set");
        let saved = fs::read_to_string(sampling_path).expect("sampling plan file");
        let saved_value: Value = norito::json::from_str(&saved).expect("parse sampling plan");
        let expected_assignment = sampling_plan
            .get("assignment_hash")
            .and_then(Value::as_str)
            .expect("assignment hash field");

        assert_eq!(
            saved_value.get("assignment_hash").and_then(Value::as_str),
            Some(expected_assignment),
            "persisted sampling plan should retain assignment hash"
        );
    }

    #[test]
    fn sampling_from_plan_derives_seed_and_count() {
        let mut sample_seed = [0u8; 32];
        sample_seed[..8].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
        let plan = da::DaSamplingPlan {
            assignment_hash: BlobDigest::new([0x11; 32]),
            sample_window: 2,
            sample_seed: Some(sample_seed),
            samples: vec![
                da::DaSampledChunk {
                    index: 1,
                    role: ChunkRole::Data,
                    group: 0,
                },
                da::DaSampledChunk {
                    index: 2,
                    role: ChunkRole::GlobalParity,
                    group: 0,
                },
            ],
        };

        let sampling = sampling_from_plan(&plan).expect("sampling from plan");
        assert_eq!(sampling.sample_count, 2);
        assert_eq!(sampling.sample_seed, 0x1122_3344_5566_7788_u64);
    }

    #[test]
    fn rent_ledger_plan_records_transfers() {
        let projection = DaRentLedgerProjection {
            rent_due: XorAmount::from_micro(1_000_000),
            protocol_reserve_due: XorAmount::from_micro(200_000),
            provider_reward_due: XorAmount::from_micro(800_000),
            pdp_bonus_pool: XorAmount::from_micro(50_000),
            potr_bonus_pool: XorAmount::from_micro(25_000),
            egress_credit_per_gib: XorAmount::from_micro(1_500),
        };
        let domain: DomainId = "test".parse().expect("test domain");
        let provider_domain: DomainId = "provider".parse().expect("provider domain");
        let payer_key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        let payer = AccountId::new(domain.clone(), payer_key.public_key().clone());
        let treasury_key = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
        let treasury = AccountId::new(domain.clone(), treasury_key.public_key().clone());
        let reserve_key = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        let reserve = AccountId::new(domain.clone(), reserve_key.public_key().clone());
        let provider_key = KeyPair::from_seed(vec![4; 32], Algorithm::Ed25519);
        let provider = AccountId::new(provider_domain.clone(), provider_key.public_key().clone());
        let pdp_key = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
        let pdp = AccountId::new(provider_domain.clone(), pdp_key.public_key().clone());
        let potr_key = KeyPair::from_seed(vec![6; 32], Algorithm::Ed25519);
        let potr = AccountId::new(provider_domain.clone(), potr_key.public_key().clone());
        let asset_definition: AssetDefinitionId = "xor#sora".parse().expect("asset definition");
        let accounts = da::DaRentLedgerAccounts {
            payer: &payer,
            treasury: &treasury,
            protocol_reserve: &reserve,
            provider: &provider,
            pdp_bonus: &pdp,
            potr_bonus: &potr,
        };
        let plan = build_rent_ledger_plan(
            Path::new("quote.json"),
            projection,
            accounts,
            &asset_definition,
        )
        .expect("plan");
        let map = plan.as_object().expect("ledger plan must be a JSON object");
        assert_eq!(
            map.get("rent_due_micro_xor").and_then(Value::as_u64),
            Some(1_000_000)
        );
        assert_eq!(
            map.get("pdp_bonus_pool_micro_xor").and_then(Value::as_u64),
            Some(50_000)
        );
        let instructions = map
            .get("instructions")
            .and_then(Value::as_array)
            .expect("instructions array missing");
        assert_eq!(instructions.len(), 5);
    }

    #[test]
    fn policy_label_override_trims_and_applies_value() {
        let label = policy_label_or_default(Some("  treasury docket 42  "), "fallback".into())
            .expect("label");
        assert_eq!(label, "treasury docket 42");
    }

    #[test]
    fn policy_label_override_rejects_empty_string() {
        let err = policy_label_or_default(Some("   \t"), "fallback".into())
            .expect_err("expected failure");
        assert!(err.to_string().contains("--policy-label"));
    }

    #[test]
    fn policy_label_defaults_when_override_missing() {
        let label = policy_label_or_default(None, "embedded".into()).expect("label");
        assert_eq!(label, "embedded");
    }

    #[test]
    fn resolve_sampling_prefers_block_hash_plan() {
        let manifest = sample_manifest();
        let block_hash = Hash::new(b"da-block-hash");
        let args = ProveArgs {
            manifest: PathBuf::new(),
            payload: PathBuf::new(),
            json_out: None,
            sample_count: 99,
            sample_seed: 777,
            block_hash: Some(hex::encode(block_hash.as_ref())),
            leaf_indexes: Vec::new(),
        };

        let sampling = args.resolve_sampling(&manifest).expect("sampling plan");
        let plan = build_sampling_plan(&manifest, &block_hash);

        assert_eq!(sampling.sample_count, plan.samples.len());
        assert_eq!(sampling.sample_seed, plan.por_seed());
    }

    fn sample_manifest() -> DaManifestV1 {
        let chunk = ChunkCommitment::new_with_role(
            0,
            0,
            4,
            BlobDigest::new([0xAA; 32]),
            ChunkRole::Data,
            0,
        );
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(0),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("demo"),
            blob_hash: BlobDigest::new([0x22; 32]),
            chunk_root: BlobDigest::new([0x33; 32]),
            storage_ticket: StorageTicketId::new([0x44; 32]),
            total_size: 4,
            chunk_size: 4,
            total_stripes: 1,
            shards_per_stripe: 1,
            erasure_profile: ErasureProfile {
                data_shards: 1,
                parity_shards: 0,
                row_parity_stripes: 0,
                chunk_alignment: 1,
                fec_scheme: FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 60,
                cold_retention_secs: 60,
                required_replicas: 1,
                storage_class: StorageClass::Hot,
                governance_tag: GovernanceTag::new("da.test"),
            },
            rent_quote: DaRentQuote::default(),
            chunks: vec![chunk],
            ipa_commitment: BlobDigest::new([0x33; 32]),
            metadata: ExtraMetadata {
                items: vec![
                    MetadataEntry::new(
                        "taikai.event_id",
                        b"demo_event".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.stream_id",
                        b"demo_stream".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.rendition_id",
                        b"demo_rendition".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.segment.sequence",
                        b"1".to_vec(),
                        MetadataVisibility::Public,
                    ),
                ],
            },
            issued_at_unix: 1,
        }
    }

    fn dummy_proof() -> PorProof {
        PorProof {
            payload_len: 4,
            chunk_index: 0,
            chunk_offset: 0,
            chunk_length: 4,
            chunk_digest: [0xAA; 32],
            chunk_root: [0xBB; 32],
            segment_index: 0,
            segment_offset: 0,
            segment_length: 4,
            segment_digest: [0xCC; 32],
            leaf_index: 0,
            leaf_offset: 0,
            leaf_length: 4,
            leaf_bytes: vec![0xDD; 4],
            leaf_digest: [0xEE; 32],
            segment_leaves: vec![[0xEE; 32]],
            chunk_segments: vec![[0xCC; 32]],
            chunk_roots: vec![[0xBB; 32]],
        }
    }
}
