mod scaling_evidence;

mod beacon_history;

use crate::{Outcome, RunArgs, tui};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, eyre};
use iroha_core::kura::{BlockIndex, BlockStore};
use iroha_data_model::block::{
    consensus_v2::MAX_EXECUTED_BLOCK_WIRE_BYTES, decode_framed_signed_block,
};
use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

const BLOCK_INDEX_BATCH_LEN: usize = 256;
const BLOCK_INDEX_BATCH_LEN_U64: u64 = 256;
const MAX_FINALITY_PREFIX_HEIGHT: u64 = 4_096;
const MAX_FINALITY_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
/// Kura inspector
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    #[clap(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Prepare, export or independently replay canonical scaling evidence
    ScalingEvidence(Box<scaling_evidence::command::Args>),
    /// Project bounded typed public beacon candidates, with explicit coverage limits.
    BeaconHistory {
        /// Exact lane directory containing the canonical block journals.
        path_to_block_store: PathBuf,
        /// First block height in the exact inspection interval.
        #[clap(short, long, value_name = "BLOCK_HEIGHT")]
        from: u64,
        #[clap(flatten)]
        options: beacon_history::Args,
    },
    /// Print contents of a certain length of the blocks
    Print {
        /// Exact lane directory containing the canonical block journals
        path_to_block_store: PathBuf,
        /// Height of the block from which start the inspection. Defaults to the latest block height
        #[clap(short, long, name = "BLOCK_HEIGHT")]
        from: Option<u64>,
        /// Number of the blocks to print. The excess will be truncated
        #[clap(short = 'n', long, default_value_t = 1)]
        length: u64,
        /// Where to write the results of the inspection If omitted, writes to stdout
        #[clap(short = 'o', long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
    /// Verify a locally anchored retained prefix and export its exact finality proof.
    Finality {
        /// Exact lane directory containing the canonical block journals.
        path_to_block_store: PathBuf,
        /// Verify all heights from genesis through this height (1..=4096).
        #[clap(short = 'H', long, value_name = "HEIGHT")]
        height: u64,
        /// Write the public JSON outside the inspected store (default: stdout).
        #[clap(short = 'o', long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
    /// Print the pipeline recovery sidecar JSON for a given height
    Sidecar {
        /// Exact lane directory containing the canonical block journals
        path_to_block_store: PathBuf,
        /// The block height whose sidecar to print
        #[clap(short = 'H', long, value_name = "HEIGHT")]
        height: u64,
        /// Where to write the sidecar JSON (default: stdout)
        #[clap(short = 'o', long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self.command {
            Command::ScalingEvidence(args) => (*args).run(writer),
            Command::BeaconHistory {
                path_to_block_store,
                from,
                options,
            } => {
                let from_height = from
                    .checked_sub(1)
                    .ok_or_else(|| eyre!("the first block height is 1; from must be positive"))?;
                write_inspection_output(
                    writer,
                    &path_to_block_store,
                    options.output.clone(),
                    |out| {
                        beacon_history::inspect(
                            out,
                            &path_to_block_store,
                            Some(from_height),
                            &options,
                        )
                    },
                )
            }
            Command::Finality {
                path_to_block_store,
                height,
                output,
            } => write_inspection_output(writer, &path_to_block_store, output, |out| {
                print_finality(out, &path_to_block_store, height)
            }),
            Command::Print {
                path_to_block_store,
                from,
                length,
                output,
            } => {
                let from_height = from
                    .map(|height| {
                        height.checked_sub(1).ok_or_else(|| {
                            eyre!("the first block height is 1; from must be positive")
                        })
                    })
                    .transpose()?;
                tui::status("Inspecting Kura block store");
                write_inspection_output(writer, &path_to_block_store, output, |out| {
                    print_blockchain(
                        out,
                        &path_to_block_store,
                        from_height.unwrap_or(u64::MAX),
                        length,
                    )
                    .wrap_err("failed to print blockchain")
                })?;
                tui::success("Block inspection complete");
                Ok(())
            }
            Command::Sidecar {
                path_to_block_store,
                height,
                output,
            } => {
                tui::status(format!("Retrieving pipeline sidecar for height {height}"));
                write_inspection_output(writer, &path_to_block_store, output, |out| {
                    print_sidecar(out, &path_to_block_store, height)
                        .wrap_err("failed to print sidecar")
                })?;
                tui::success("Sidecar exported");
                Ok(())
            }
        }
    }
}

fn write_inspection_output<T: Write>(
    writer: &mut BufWriter<T>,
    block_store_path: &Path,
    output: Option<PathBuf>,
    render: impl FnOnce(&mut dyn Write) -> Outcome,
) -> Outcome {
    let Some(output) = output else {
        return render(writer);
    };
    let block_store = resolve_block_store_dir(block_store_path)?;
    let output =
        crate::atomic_output::resolve_outside_directory(&block_store, &output, "block store")?;
    crate::atomic_output::write_file(&output, ".kagami-kura-", render)
}
fn resolve_block_store_dir(block_store_path: &Path) -> color_eyre::Result<PathBuf> {
    let metadata = fs::symlink_metadata(block_store_path).wrap_err_with(|| {
        format!(
            "failed to inspect block-store directory {}",
            block_store_path.display()
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(eyre!(
            "block-store path must be an explicit non-symlink lane directory: {}",
            block_store_path.display()
        ));
    }
    for file_name in ["blocks.index", "blocks.data", "blocks.hashes"] {
        let path = block_store_path.join(file_name);
        let metadata = fs::symlink_metadata(&path).wrap_err_with(|| {
            format!(
                "block-store directory is missing required file {}",
                path.display()
            )
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(eyre!(
                "block-store file must be a non-symlink regular file: {}",
                path.display()
            ));
        }
    }
    fs::canonicalize(block_store_path).wrap_err_with(|| {
        format!(
            "failed to resolve block-store directory {}",
            block_store_path.display()
        )
    })
}
fn print_blockchain(
    writer: &mut dyn Write,
    block_store_path: &Path,
    from_height: u64,
    block_count: u64,
) -> Outcome {
    if block_count == 0 {
        return Err(eyre!("block count must be at least one"));
    }
    let block_store_path = resolve_block_store_dir(block_store_path)?;
    let mut block_store = BlockStore::open_read_only(&block_store_path)
        .wrap_err("failed to open canonical Kura journals read-only")?;
    let index_count = block_store
        .read_index_count()
        .wrap_err("failed to read index count from block store {block_store_path:?}.")?;
    if index_count == 0 {
        return Err(eyre!(
            "Index count is zero. This could be because there are no blocks in the store: {block_store_path:?}"
        ));
    }
    let from_height = if from_height >= index_count {
        index_count - 1
    } else {
        from_height
    };
    // Clamp to available blocks and avoid u64 addition overflow when length is untrusted user input.
    let requested = from_height.saturating_add(block_count);
    let block_count = if requested > index_count {
        index_count.saturating_sub(from_height)
    } else {
        block_count
    };
    let mut block_indices = vec![
        BlockIndex {
            start: 0,
            length: 0
        };
        BLOCK_INDEX_BATCH_LEN
    ];
    writeln!(writer, "Index file says there are {index_count} blocks.",)?;
    writeln!(
        writer,
        "Printing blocks {}-{}...",
        from_height + 1,
        from_height + block_count
    )?;
    let mut next_height = from_height;
    let mut remaining = block_count;
    let mut block_buf = Vec::new();
    while remaining != 0 {
        let batch_len_u64 = remaining.min(BLOCK_INDEX_BATCH_LEN_U64);
        let batch_len = usize::try_from(batch_len_u64).expect("fixed batch length fits usize");
        let batch = &mut block_indices[..batch_len];
        block_store
            .read_block_indices(next_height, batch)
            .wrap_err("failed to read block indices")?;
        for (offset, idx) in batch.iter().copied().enumerate() {
            let offset = u64::try_from(offset).expect("fixed batch offset fits u64");
            let meta_index = next_height + offset;
            writeln!(
                writer,
                "Block#{} starts at byte offset {} and is {} bytes long.",
                meta_index + 1,
                idx.start,
                idx.length
            )?;
            if idx.length == 0 || idx.length > MAX_EXECUTED_BLOCK_WIRE_BYTES {
                return Err(eyre!(
                    "block № {} has invalid wire length {}; expected 1..={MAX_EXECUTED_BLOCK_WIRE_BYTES}",
                    meta_index + 1,
                    idx.length
                ));
            }
            let len = usize::try_from(idx.length).wrap_err("block length does not fit usize")?;
            if len > block_buf.capacity() {
                block_buf
                    .try_reserve_exact(len - block_buf.len())
                    .wrap_err_with(|| {
                        format!(
                            "failed to reserve {} bytes for block № {}",
                            len,
                            meta_index + 1
                        )
                    })?;
            }
            block_buf.resize(len, 0);
            block_store
                .read_block_data(idx.start, &mut block_buf)
                .wrap_err(format!("failed to read block № {} data.", meta_index + 1))?;
            let block = decode_framed_signed_block(&block_buf)
                .map_err(|err| eyre!("Failed to decode block № {}: {err}", meta_index + 1))?;
            writeln!(writer, "Block#{} :", meta_index + 1)?;
            writeln!(writer, "{block:#?}")?;
        }
        next_height += batch_len_u64;
        remaining -= batch_len_u64;
    }
    Ok(())
}
fn print_finality(writer: &mut dyn Write, block_store_path: &Path, height: u64) -> Outcome {
    use iroha_data_model::bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof, BridgeFinalityVerifier,
    };
    if !(1..=MAX_FINALITY_PREFIX_HEIGHT).contains(&height) {
        return Err(eyre!(
            "finality height must be in 1..={MAX_FINALITY_PREFIX_HEIGHT}"
        ));
    }
    let directory = resolve_block_store_dir(block_store_path)?;
    let mut store = BlockStore::open_read_only(&directory)
        .wrap_err("failed to open canonical Kura journals read-only")?;
    let (block_header, finality_artifact) = store
        .read_verified_v2_finality(1)
        .wrap_err("failed to read verified genesis finality")?;
    let local_context = finality_artifact.context_id();
    let mut verifier = BridgeFinalityVerifier::with_context(
        finality_artifact.height_context.network_id,
        local_context,
    );
    let mut selected = BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header,
        finality_artifact,
    };
    verifier
        .verify(&selected)
        .wrap_err("invalid genesis finality")?;
    for current in 2..=height {
        let (block_header, finality_artifact) = store
            .read_verified_v2_finality(current)
            .wrap_err_with(|| format!("failed to read verified finality at height {current}"))?;
        selected = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header,
            finality_artifact,
        };
        verifier
            .verify(&selected)
            .wrap_err_with(|| format!("invalid finality successor at height {current}"))?;
    }
    // The local genesis context supplies comparison evidence only. No externally
    // authenticated network identity or genesis context was supplied to this command.
    let report = norito::json!({
        "schema": "iroha.kura.finality-inspection.v1",
        "verified_prefix_start": 1_u64,
        "verified_prefix_end": height,
        "external_trust_anchor": false,
        "local_genesis_context_id": local_context,
        "finality_proof": selected
    });
    let encoded = norito::json::to_json_bounded(&report, MAX_FINALITY_OUTPUT_BYTES)
        .wrap_err("finality inspection exceeds output bound")?;
    writer.write_all(encoded.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}
fn print_sidecar(writer: &mut dyn Write, block_store_path: &Path, height: u64) -> Outcome {
    // Resolve the concrete lane directory when multilane layout is in use.
    let block_store_path = resolve_block_store_dir(block_store_path)?;
    let mut block_store = BlockStore::open_read_only(&block_store_path)
        .wrap_err("failed to open canonical Kura journals read-only")?;
    if let Some(sidecar) = block_store
        .read_pipeline_metadata(height)
        .wrap_err("failed to read canonical pipeline sidecar")?
    {
        let json = sidecar.to_json_value();
        let serialized =
            norito::json::to_json_pretty(&json).wrap_err("failed to serialize pipeline sidecar")?;
        writer.write_all(serialized.as_bytes())?;
        return Ok(());
    }
    Err(eyre!(
        "no indexed pipeline sidecar found under {:?} for height {}",
        block_store_path,
        height
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{block::BlockBuilder, kura::PipelineDagSnapshot, tx::AcceptedTransaction};
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::{BlockHeader, SignedBlock},
        prelude::*,
    };
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    use std::{borrow::Cow, fs, sync::Arc};
    #[test]
    fn inspection_commands_require_their_own_store_and_range() {
        use clap::Parser as _;

        for args in [
            vec!["beacon-history", "lane0", "--from", "1", "--length", "4"],
            vec!["finality", "lane0", "--height", "4"],
        ] {
            assert!(
                crate::Cli::try_parse_from(["kagami", "advanced", "kura"].into_iter().chain(args))
                    .is_ok()
            );
        }
        for args in [
            vec!["beacon-history", "lane0", "--length", "4"],
            vec!["finality", "lane0", "--height", "4", "--from", "2"],
            vec!["lane0", "finality", "--height", "4"],
        ] {
            assert!(
                crate::Cli::try_parse_from(["kagami", "advanced", "kura"].into_iter().chain(args))
                    .is_err()
            );
        }
    }
    #[test]
    fn beacon_history_rejects_zero_start_before_accessing_store() {
        let args = Args {
            command: Command::BeaconHistory {
                path_to_block_store: PathBuf::from("missing-store"),
                from: 0,
                options: beacon_history::Args {
                    length: 1,
                    merge_sidecars: Vec::new(),
                    output: None,
                },
            },
        };
        let error = args.run(&mut BufWriter::new(Vec::new())).unwrap_err();
        assert!(error.to_string().contains("from must be positive"));
    }
    fn fixture_block(prev: Option<&SignedBlock>) -> Arc<SignedBlock> {
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"kagami-kura-fixture-network",
            )));
        let authority = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        // A simple instruction is enough; validity is not exercised here.
        let tx = iroha_data_model::transaction::TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "test".to_owned())])
        .try_sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .expect("sign Kagami Kura fixture transaction");
        tx.verify_signature()
            .expect("Kagami Kura fixture transaction signature verifies");
        let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let sb: SignedBlock = BlockBuilder::new(vec![acc])
            .chain(0, prev)
            .try_sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .expect("sign Kagami Kura fixture block")
            .unpack(|_| {})
            .into();
        Arc::new(sb)
    }
    fn append_block(store: &mut BlockStore, prev: Option<&SignedBlock>) -> Arc<SignedBlock> {
        let block = fixture_block(prev);
        store.append_block_to_chain(&block).expect("append");
        block
    }
    #[test]
    fn appended_block_uses_verifiable_checked_signature() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist().unwrap();
        let block = append_block(&mut store, None);
        let signature = block
            .signatures()
            .next()
            .expect("fixture block carries signature");
        signature
            .signature()
            .verify_hash(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(), block.hash())
            .expect("Kagami Kura fixture block signature verifies");
        let wrong_key =
            KeyPair::try_random_with_algorithm(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.algorithm())
                .expect("generate wrong-key verifier");
        signature
            .signature()
            .verify_hash(wrong_key.public_key(), block.hash())
            .expect_err("Kagami Kura fixture block rejects wrong key");
    }
    #[test]
    fn print_latest_block_from_store_dir() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist().unwrap();
        let first = append_block(&mut store, None);
        let _second = append_block(&mut store, Some(first.as_ref()));
        let mut buf = Vec::new();
        // Use a large from_height to select the latest block per inspector logic
        print_blockchain(&mut buf, temp.path(), u64::MAX, 1).unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Basic shape assertions
        assert!(s.contains("Index file says there are 2 blocks."));
        assert!(s.contains("Printing blocks 2-2"));
        assert!(s.contains("Block#2 starts at byte offset"));
    }
    #[cfg(unix)]
    #[test]
    fn print_accepts_an_immutable_read_only_snapshot() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist().unwrap();
        append_block(&mut store, None);
        drop(store);
        for file_name in ["blocks.index", "blocks.data", "blocks.hashes"] {
            fs::set_permissions(
                temp.path().join(file_name),
                fs::Permissions::from_mode(0o444),
            )
            .expect("protect Kura journal snapshot");
        }
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555))
            .expect("protect Kura snapshot directory");

        let mut output = Vec::new();
        print_blockchain(&mut output, temp.path(), 0, 1).expect("inspect read-only Kura snapshot");
        assert!(String::from_utf8(output).unwrap().contains("Block#1"));
        assert_eq!(
            fs::metadata(temp.path().join("blocks.index"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444,
            "inspection must not rewrite journal permissions"
        );
    }
    #[test]
    fn print_writes_to_output_file() {
        // Prepare a temporary block store with two blocks.
        let temp = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist().unwrap();
        let first = append_block(&mut store, None);
        let _second = append_block(&mut store, Some(first.as_ref()));
        // Keep inspection output separate from the protected store tree.
        let output = tempfile::tempdir().unwrap();
        let out_path = output.path().join("out.txt");
        // Build Kagami args (use output some file; writer should be ignored in this branch)
        let args = Args {
            command: Command::Print {
                from: None,
                path_to_block_store: temp.path().to_owned(),
                length: 1,
                output: Some(out_path.clone()),
            },
        };
        let mut sink = std::io::BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink).expect("print ok");
        // Validate file content contains the expected prelude and the latest block number
        let s = std::fs::read_to_string(&out_path).expect("read output");
        assert!(s.contains("Index file says there are 2 blocks."));
        assert!(s.contains("Printing blocks 2-2"));
    }
    #[test]
    fn sidecar_prints_to_file() {
        use iroha_config::{
            base::WithOrigin,
            kura::FsyncMode,
            parameters::{
                actual::{Kura as KuraConfig, LaneConfig},
                defaults::kura::{BLOCKS_IN_MEMORY, FSYNC_INTERVAL, MERGE_LEDGER_CACHE_CAPACITY},
            },
        };
        use iroha_core::kura::{Kura, PipelineRecoverySidecar};
        // Prepare a temp store and write metadata for a canonical block.
        let temp = tempfile::tempdir().unwrap();
        let lane_config = LaneConfig::default();
        let block_store_path = Kura::canonical_storage_paths(temp.path()).0;
        let (kura, _count) = Kura::new_fresh_single_lane(
            &KuraConfig {
                init_mode: iroha_config::kura::InitMode::Strict,
                store_dir: WithOrigin::inline(temp.path().to_owned()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: FsyncMode::Batched,
                fsync_interval: FSYNC_INTERVAL,
                lane_history_retention:
                    iroha_config::parameters::defaults::kura::LANE_HISTORY_RETENTION,
                fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
                replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            },
            &lane_config,
        )
        .unwrap();
        let block = fixture_block(None);
        kura.store_block(block.clone())
            .expect("store sidecar fixture block through authorized Kura mutation");
        let mut fingerprint = [0u8; 32];
        fingerprint[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let sidecar = PipelineRecoverySidecar::new(
            1,
            block.hash(),
            PipelineDagSnapshot {
                fingerprint,
                key_count: 0,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);
        let output = tempfile::tempdir().unwrap();
        let out_path = output.path().join("sidecar.json");
        let args = Args {
            command: Command::Sidecar {
                path_to_block_store: block_store_path,
                height: 1,
                output: Some(out_path.clone()),
            },
        };
        let mut sink = std::io::BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink).expect("sidecar ok");
        let read = std::fs::read_to_string(out_path).unwrap();
        assert!(read.contains("\"pipeline.recovery\""));
        assert!(read.contains("\"height\": 1"));
        assert!(read.contains(&block.hash().to_string()));
    }
    #[test]
    fn print_clamps_overflowing_length() {
        // Prepare a temporary block store with two blocks.
        let temp = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(temp.path());
        store.create_files_if_they_do_not_exist().unwrap();
        let first = append_block(&mut store, None);
        let _second = append_block(&mut store, Some(first.as_ref()));
        let mut buf = Vec::new();
        // Request an absurdly large length; logic should clamp to the available blocks.
        print_blockchain(&mut buf, temp.path(), 0, u64::MAX).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Printing blocks 1-2"));
    }
    #[test]
    fn sidecar_print_rejects_invalid_block_layout() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("blocks.index"), b"").expect("seed index");
        fs::write(temp.path().join("blocks.data"), b"").expect("seed data");
        fs::write(temp.path().join("blocks.hashes"), b"").expect("seed hashes");
        let pipeline_dir = temp.path().join("pipeline");
        fs::create_dir_all(&pipeline_dir).expect("pipeline dir");
        fs::write(pipeline_dir.join("block_1.norito"), b"invalid").expect("invalid sidecar");
        let mut sink = std::io::BufWriter::new(Vec::<u8>::new());
        let err = print_sidecar(&mut sink, temp.path(), 1).expect_err("invalid layout should fail");
        assert!(
            err.to_string().contains("no indexed pipeline sidecar"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn print_rejects_oversized_index_entry_before_allocating() {
        let temp = tempfile::tempdir().unwrap();
        let mut index = Vec::new();
        index.extend_from_slice(&0_u64.to_le_bytes());
        index.extend_from_slice(&(MAX_EXECUTED_BLOCK_WIRE_BYTES + 1).to_le_bytes());
        fs::write(temp.path().join("blocks.index"), index).expect("seed index");
        fs::write(temp.path().join("blocks.data"), b"").expect("seed data");
        fs::write(temp.path().join("blocks.hashes"), b"").expect("seed hashes");
        let error = print_blockchain(&mut Vec::new(), temp.path(), 0, 1)
            .expect_err("oversized block must be rejected");
        assert!(error.to_string().contains("invalid wire length"));
    }
    #[test]
    fn finality_inspection_rejects_invalid_height_before_store_access() {
        let missing = Path::new("/missing-kagami-finality-store");
        for height in [0, MAX_FINALITY_PREFIX_HEIGHT + 1, u64::MAX] {
            let mut output = Vec::new();
            let error = print_finality(&mut output, missing, height)
                .expect_err("invalid height must fail before opening a store");
            assert!(error.to_string().contains("finality height must be in"));
            assert!(output.is_empty());
        }
    }
    #[test]
    fn finality_inspection_failure_preserves_output_and_store() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(directory.path());
        store.create_files_if_they_do_not_exist().unwrap();
        append_block(&mut store, None);
        let before = ["blocks.index", "blocks.data", "blocks.hashes"]
            .map(|name| (name, fs::read(directory.path().join(name)).unwrap()));
        let output = tempfile::tempdir().unwrap();
        let output_path = output.path().join("finality.json");
        fs::write(&output_path, b"previous output").unwrap();
        let args = Args {
            command: Command::Finality {
                path_to_block_store: directory.path().to_path_buf(),
                height: 1,
                output: Some(output_path.clone()),
            },
        };
        let mut sink = BufWriter::new(Vec::new());
        assert!(args.run(&mut sink).is_err(), "missing finality must fail");
        assert!(sink.into_inner().unwrap().is_empty());
        assert_eq!(fs::read(&output_path).unwrap(), b"previous output");
        for (name, bytes) in before {
            assert_eq!(fs::read(directory.path().join(name)).unwrap(), bytes);
        }
        assert!(!directory.path().join("v2_finality").exists());
    }
    #[test]
    fn finality_command_rejects_output_inside_store() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = BlockStore::new(directory.path());
        store.create_files_if_they_do_not_exist().unwrap();
        let output = directory.path().join("finality.json");
        let args = Args {
            command: Command::Finality {
                path_to_block_store: directory.path().to_path_buf(),
                height: 1,
                output: Some(output.clone()),
            },
        };
        let error = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("output inside store must fail");
        assert!(error.to_string().contains("inside the block store"));
        assert!(!output.exists());
    }
    #[test]
    fn output_is_atomic_and_cannot_target_the_block_store() {
        let store = tempfile::tempdir().unwrap();
        fs::write(store.path().join("blocks.index"), b"original index").expect("seed index");
        fs::write(store.path().join("blocks.data"), b"").expect("seed data");
        fs::write(store.path().join("blocks.hashes"), b"").expect("seed hashes");
        let output = tempfile::tempdir().unwrap();
        let output_path = output.path().join("inspection.txt");
        fs::write(&output_path, b"previous output").expect("seed output");
        let mut sink = BufWriter::new(Vec::new());
        let error = write_inspection_output(
            &mut sink,
            store.path(),
            Some(output_path.clone()),
            |_writer| Err(eyre!("synthetic render failure")),
        )
        .expect_err("failed render must not publish");
        assert!(error.to_string().contains("synthetic render failure"));
        assert_eq!(
            fs::read(&output_path).expect("read output"),
            b"previous output"
        );

        let index_path = store.path().join("blocks.index");
        let error = write_inspection_output(
            &mut sink,
            store.path(),
            Some(index_path.clone()),
            |_writer| Ok(()),
        )
        .expect_err("block-store target must be rejected");
        assert!(error.to_string().contains("inside the block store"));
        assert_eq!(fs::read(index_path).expect("read index"), b"original index");
    }
    #[test]
    fn block_store_path_must_name_an_explicit_lane_directory() {
        let root = tempfile::tempdir().unwrap();
        let lane = root.path().join("blocks").join("lane0");
        fs::create_dir_all(&lane).expect("create lane");
        for file_name in ["blocks.index", "blocks.data", "blocks.hashes"] {
            fs::write(lane.join(file_name), b"").expect("seed lane file");
        }
        let error = resolve_block_store_dir(root.path())
            .expect_err("store root must not silently select its first lane");
        assert!(error.to_string().contains("missing required file"));
        assert_eq!(
            resolve_block_store_dir(&lane).expect("explicit lane"),
            fs::canonicalize(lane).expect("canonical lane")
        );
    }
    #[test]
    fn print_from_zero_fails_before_store_or_output_admission() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("never-created.txt");
        let args = Args {
            command: Command::Print {
                path_to_block_store: directory.path().join("missing-store"),
                from: Some(0),
                length: 1,
                output: Some(output.clone()),
            },
        };
        let mut writer = BufWriter::new(Vec::new());
        let error = args.run(&mut writer).unwrap_err().to_string();
        assert!(error.contains("from must be positive"), "{error}");
        assert!(!output.exists());
        assert!(writer.into_inner().unwrap().is_empty());
    }
}
