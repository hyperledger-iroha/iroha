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
/// Kura inspector
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    /// Height of the block from which start the inspection. Defaults to the latest block height
    #[clap(short, long, name = "BLOCK_HEIGHT")]
    from: Option<u64>,
    #[clap()]
    path_to_block_store: PathBuf,
    #[clap(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Print contents of a certain length of the blocks
    Print {
        /// Number of the blocks to print. The excess will be truncated
        #[clap(short = 'n', long, default_value_t = 1)]
        length: u64,
        /// Where to write the results of the inspection If omitted, writes to stdout
        #[clap(short = 'o', long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
    /// Print the pipeline recovery sidecar JSON for a given height
    Sidecar {
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
        let args = self;
        let from_height = args.from.map(|height| {
            if height == 0 {
                Err(eyre!("The genesis block has the height 1. Therefore, the \"from height\" you specify must not be 0 ({} is provided). ", height))
            } else {
                // Kura starts counting blocks from 0 like an array while the outside world counts the first block as number 1.
                Ok(height - 1)
            }
        }).transpose()?;
        match args.command {
            Command::Print { length, output } => {
                tui::status("Inspecting Kura block store");
                write_inspection_output(writer, &args.path_to_block_store, output, |out| {
                    print_blockchain(
                        out,
                        &args.path_to_block_store,
                        from_height.unwrap_or(u64::MAX),
                        length,
                    )
                    .wrap_err("failed to print blockchain")
                })?;
                tui::success("Block inspection complete");
                Ok(())
            }
            Command::Sidecar { height, output } => {
                tui::status(format!("Retrieving pipeline sidecar for height {height}"));
                write_inspection_output(writer, &args.path_to_block_store, output, |out| {
                    print_sidecar(out, &args.path_to_block_store, height)
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
    let mut block_store = BlockStore::new(&block_store_path);
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
                    .try_reserve_exact(len - block_buf.capacity())
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
fn print_sidecar(writer: &mut dyn Write, block_store_path: &Path, height: u64) -> Outcome {
    // Resolve the concrete lane directory when multilane layout is in use.
    let block_store_path = resolve_block_store_dir(block_store_path)?;
    let mut block_store = BlockStore::new(&block_store_path);
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
    fn append_block(store: &mut BlockStore, prev: Option<&SignedBlock>) -> Arc<SignedBlock> {
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
        store.append_block_to_chain(&sb).expect("append");
        Arc::new(sb)
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
            from: None,
            path_to_block_store: temp.path().to_owned(),
            command: Command::Print {
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
                replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            },
            &LaneConfig::default(),
        )
        .unwrap();
        let mut store = BlockStore::new(temp.path());
        let block = append_block(&mut store, None);
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
            from: None,
            path_to_block_store: temp.path().to_owned(),
            command: Command::Sidecar {
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
}
