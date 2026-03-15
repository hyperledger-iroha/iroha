use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, eyre};
use iroha_core::kura::{BlockIndex, BlockStore, PipelineRecoverySidecar};
use iroha_data_model::block::decode_framed_signed_block;

use crate::{Outcome, RunArgs, tui};

/// Kura inspector
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    /// Height of the block from which start the inspection.
    /// Defaults to the latest block height
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
        /// Number of the blocks to print.
        /// The excess will be truncated
        #[clap(short = 'n', long, default_value_t = 1)]
        length: u64,
        /// Where to write the results of the inspection
        /// If omitted, writes to stdout
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
                let result = if let Some(path) = output {
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&path)
                        .wrap_err(format!("failed to open output file: {path:?}"))?;
                    let mut out = BufWriter::new(file);
                    print_blockchain(
                        &mut out,
                        &args.path_to_block_store,
                        from_height.unwrap_or(u64::MAX),
                        length,
                    )
                    .wrap_err("failed to print blockchain")
                } else {
                    print_blockchain(
                        writer,
                        &args.path_to_block_store,
                        from_height.unwrap_or(u64::MAX),
                        length,
                    )
                    .wrap_err("failed to print blockchain")
                };
                result?;
                tui::success("Block inspection complete");
                Ok(())
            }
            Command::Sidecar { height, output } => {
                tui::status(format!("Retrieving pipeline sidecar for height {height}"));
                let result = if let Some(path) = output {
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&path)
                        .wrap_err(format!("failed to open output file: {path:?}"))?;
                    let mut out = BufWriter::new(file);
                    print_sidecar(&mut out, &args.path_to_block_store, height)
                        .wrap_err("failed to print sidecar")
                } else {
                    print_sidecar(writer, &args.path_to_block_store, height)
                        .wrap_err("failed to print sidecar")
                };
                result?;
                tui::success("Sidecar exported");
                Ok(())
            }
        }
    }
}

fn resolve_block_store_dir(block_store_path: &Path) -> color_eyre::Result<PathBuf> {
    let mut normalized: std::borrow::Cow<'_, Path> = block_store_path.into();

    if let Some(os_str_file_name) = normalized.file_name() {
        let file_name_str = os_str_file_name.to_str().unwrap_or("");
        if matches!(
            file_name_str,
            "blocks.data" | "blocks.index" | "blocks.hashes"
        ) {
            normalized.to_mut().pop();
        }
    }

    let candidate = normalized.into_owned();
    if candidate.join("blocks.index").exists() {
        return Ok(candidate);
    }

    let blocks_dir = candidate.join("blocks");
    if blocks_dir.is_dir() {
        let mut entries = std::fs::read_dir(&blocks_dir)
            .wrap_err_with(|| format!("failed to read blocks directory {:?}", blocks_dir))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let lane_dir = entry.path();
            if lane_dir.join("blocks.index").exists() {
                return Ok(lane_dir);
            }
        }
    }

    Err(color_eyre::eyre::eyre!(
        "failed to locate block store files under {:?}",
        candidate
    ))
}

fn print_blockchain(
    writer: &mut dyn Write,
    block_store_path: &Path,
    from_height: u64,
    block_count: u64,
) -> Outcome {
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
        block_count
            .try_into()
            .wrap_err("block_count didn't fit in 32-bits")?
    ];
    block_store
        .read_block_indices(from_height, &mut block_indices)
        .wrap_err("failed to read block indices")?;
    let block_indices = block_indices;

    // Now for the actual printing
    writeln!(writer, "Index file says there are {index_count} blocks.",)?;
    writeln!(
        writer,
        "Printing blocks {}-{}...",
        from_height + 1,
        from_height + block_count
    )?;

    for i in 0..block_count {
        let idx = block_indices[usize::try_from(i).wrap_err("index didn't fit in 32-bits")?];
        let meta_index = from_height + i;

        writeln!(
            writer,
            "Block#{} starts at byte offset {} and is {} bytes long.",
            meta_index + 1,
            idx.start,
            idx.length
        )?;
        let mut block_buf =
            vec![0_u8; usize::try_from(idx.length).wrap_err("index_len didn't fit in 32-bits")?];
        block_store
            .read_block_data(idx.start, &mut block_buf)
            .wrap_err(format!("failed to read block № {} data.", meta_index + 1))?;
        let block = decode_framed_signed_block(&block_buf)
            .map_err(|err| eyre!("Failed to decode block № {}: {err}", meta_index + 1))?;
        writeln!(writer, "Block#{} :", meta_index + 1)?;
        writeln!(writer, "{block:#?}")?;
    }

    Ok(())
}

fn print_sidecar(writer: &mut dyn Write, block_store_path: &Path, height: u64) -> Outcome {
    // Resolve the concrete lane directory when multilane layout is in use.
    let block_store_path = resolve_block_store_dir(block_store_path)?;

    // First try the current indexed sidecar store (`pipeline/sidecars.{norito,index}`).
    if let Some(sidecar) = read_indexed_sidecar(&block_store_path, height) {
        let json = sidecar.to_json_value();
        let serialized = norito::json::to_json_pretty(&json).expect("serialize pipeline sidecar");
        writer.write_all(serialized.as_bytes())?;
        return Ok(());
    }

    Err(eyre!(
        "no indexed pipeline sidecar found under {:?} for height {}",
        block_store_path,
        height
    ))
}

/// Attempt to read a pipeline recovery sidecar from the indexed sidecar store.
fn read_indexed_sidecar(block_store_path: &Path, height: u64) -> Option<PipelineRecoverySidecar> {
    const PIPELINE_DIR_NAME: &str = "pipeline";
    const PIPELINE_SIDECARS_DATA_FILE: &str = "sidecars.norito";
    const PIPELINE_SIDECARS_INDEX_FILE: &str = "sidecars.index";
    const PIPELINE_INDEX_ENTRY_SIZE: u64 = 16; // two u64s: offset + len

    if height == 0 {
        return None;
    }

    let pipeline_dir = block_store_path.join(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);

    let mut index = File::open(&index_path).ok()?;
    let index_meta = index.metadata().ok()?;
    if index_meta.len() % PIPELINE_INDEX_ENTRY_SIZE != 0 {
        return None;
    }
    let entries = index_meta.len() / PIPELINE_INDEX_ENTRY_SIZE;
    if height > entries {
        return None;
    }

    let mut entry_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE as usize];
    index
        .seek(SeekFrom::Start((height - 1) * PIPELINE_INDEX_ENTRY_SIZE))
        .ok()?;
    index.read_exact(&mut entry_buf).ok()?;
    let offset = u64::from_le_bytes(entry_buf[..8].try_into().expect("slice len"));
    let len = u64::from_le_bytes(entry_buf[8..].try_into().expect("slice len"));
    if len == 0 {
        return None;
    }
    let len_usize = usize::try_from(len).ok()?;

    let mut data = File::open(&data_path).ok()?;
    let data_len = data.metadata().ok()?.len();
    if offset.saturating_add(len) > data_len {
        return None;
    }

    let mut payload = vec![0u8; len_usize];
    data.seek(SeekFrom::Start(offset)).ok()?;
    data.read_exact(&mut payload).ok()?;

    norito::decode_from_bytes::<PipelineRecoverySidecar>(&payload).ok()
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, fs, sync::Arc};

    use iroha_core::{block::BlockBuilder, kura::PipelineDagSnapshot, tx::AcceptedTransaction};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::{BlockHeader, SignedBlock},
        prelude::*,
    };
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    fn append_block(store: &mut BlockStore, prev: Option<&SignedBlock>) -> Arc<SignedBlock> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let authority = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        // A simple instruction is enough; validity is not exercised here.
        let tx = iroha_data_model::transaction::TransactionBuilder::new(chain_id, authority)
            .with_instructions([Log::new(Level::INFO, "test".to_owned())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let sb: SignedBlock = BlockBuilder::new(vec![acc])
            .chain(0, prev)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        store.append_block_to_chain(&sb).expect("append");
        Arc::new(sb)
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

        // Prepare output file
        let out_path = temp.path().join("out.txt");

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
            kura::{FsyncMode, InitMode},
            parameters::{
                actual::{Kura as KuraConfig, LaneConfig},
                defaults::kura::{BLOCKS_IN_MEMORY, FSYNC_INTERVAL, MERGE_LEDGER_CACHE_CAPACITY},
            },
        };
        use iroha_core::kura::Kura;

        // Prepare a temp store and write a synthetic sidecar via Kura
        let temp = tempfile::tempdir().unwrap();
        let (kura, _count) = Kura::new(
            &KuraConfig {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp.path().to_owned()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: FsyncMode::Batched,
                fsync_interval: FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
            &LaneConfig::default(),
        )
        .unwrap();
        let mut fingerprint = [0u8; 32];
        fingerprint[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32]));
        let sidecar = PipelineRecoverySidecar::new(
            7,
            block_hash,
            PipelineDagSnapshot {
                fingerprint,
                key_count: 0,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);

        let out_path = temp.path().join("sidecar.json");
        let args = Args {
            from: None,
            path_to_block_store: temp.path().to_owned(),
            command: Command::Sidecar {
                height: 7,
                output: Some(out_path.clone()),
            },
        };
        let mut sink = std::io::BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink).expect("sidecar ok");

        let read = std::fs::read_to_string(out_path).unwrap();
        assert!(read.contains("\"pipeline.recovery\""));
        assert!(read.contains("\"height\": 7"));
        assert!(read.contains(&block_hash.to_string()));
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
}
