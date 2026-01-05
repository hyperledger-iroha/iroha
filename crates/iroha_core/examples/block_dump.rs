//! Dump basic metadata from Norito-encoded block files.
//! Handy for inspecting test-network pipeline artifacts.

use std::{env, fs, path::Path};

use iroha_core::kura::{BlockStore, PipelineRecoverySidecar};
use iroha_data_model::{
    asset::AssetId,
    block::{SignedBlock, decode_framed_signed_block},
    isi::{InstructionBox, Mint, MintBox},
    transaction::Executable,
};
use iroha_primitives::numeric::Numeric;
use norito::decode_from_bytes;

struct DumpContext {
    verbose: bool,
    sum_asset: Option<AssetId>,
    minted_sum: Numeric,
}

impl DumpContext {
    fn record_mint(&mut self, instruction: &InstructionBox) {
        let Some(target_asset) = &self.sum_asset else {
            return;
        };
        let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() else {
            return;
        };

        if let MintBox::Asset(Mint {
            object,
            destination,
        }) = mint
            && destination == target_asset
            && let Some(new_sum) = self.minted_sum.clone().checked_add(object.clone())
        {
            self.minted_sum = new_sum;
        }
    }
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!("usage: block_dump <block_file> [block_file...]");
        std::process::exit(1);
    }
    let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();
    let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")
        .ok()
        .map(|raw| raw.parse())
        .transpose()
        .expect("invalid BLOCK_DUMP_SUM_ASSET");

    let mut context = DumpContext {
        verbose,
        sum_asset,
        minted_sum: Numeric::zero(),
    };
    let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {
        raw.split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect::<std::collections::BTreeSet<_>>()
    });

    if args.iter().any(|arg| arg == "--schema-hash") {
        let hash = <SignedBlock as norito::NoritoSerialize>::schema_hash();
        let mut hex = String::new();
        for byte in &hash {
            use core::fmt::Write;
            let _ = write!(hex, "{byte:02x}");
        }
        println!("SignedBlock schema_hash=0x{hex}");
    }

    for path_str in args.into_iter().filter(|arg| arg != "--schema-hash") {
        let path = Path::new(&path_str);
        let result = if path.is_dir()
            && path.join("blocks.data").exists()
            && path.join("blocks.index").exists()
        {
            dump_store(path, &mut context, height_filter.as_ref())
        } else {
            dump_file(path, &mut context)
        };

        if let Err(err) = result {
            eprintln!("{path_str}: {err:?}");
        }
    }

    if let Some(asset_id) = context.sum_asset {
        println!("mint sum for {asset_id} = {}", context.minted_sum);
    }
}

fn dump_store(
    path: &Path,
    ctx: &mut DumpContext,
    filter: Option<&std::collections::BTreeSet<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = BlockStore::new(path);
    store.create_files_if_they_do_not_exist()?;

    let mut idx = 0u64;
    loop {
        let index = match store.read_block_index(idx) {
            Ok(index) => index,
            Err(err) => {
                // Stop when we've exhausted the index file; surface other errors.
                if matches!(err, iroha_core::kura::Error::IO(_, _)) {
                    break;
                }
                return Err(Box::new(err));
            }
        };
        if let Some(filter) = filter
            && !filter.contains(&idx)
        {
            idx += 1;
            continue;
        }

        let len: usize = index.length.try_into()?;
        let mut buf = vec![0u8; len];
        store
            .read_block_data(index.start, &mut buf)
            .map_err(Box::<dyn std::error::Error>::from)?;
        let block = decode_framed_signed_block(&buf)?;
        dump_block(path.join(format!("height-{idx}")).as_path(), &block, ctx);
        idx += 1;
    }

    Ok(())
}

fn dump_file(path: &Path, ctx: &mut DumpContext) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;

    if let Ok(block) = decode_framed_signed_block(&bytes) {
        dump_block(path, &block, ctx);
        return Ok(());
    }
    // Only framed v1 payloads are supported.
    if let Ok(block) = decode_from_bytes::<SignedBlock>(&bytes) {
        dump_block(path, &block, ctx);
        return Ok(());
    }

    // Final attempt: interpret the payload as a pipeline recovery sidecar so that
    // extracted entries from `pipeline/sidecars.norito` can be inspected without a
    // custom tool.
    if let Ok(sidecar) = decode_from_bytes::<PipelineRecoverySidecar>(&bytes) {
        dump_pipeline(path, &sidecar);
        return Ok(());
    }

    // Bubble up the first decode error to aid debugging.
    let block: SignedBlock = decode_from_bytes(&bytes)?;
    dump_block(path, &block, ctx);
    Ok(())
}

fn dump_block(path: &Path, block: &SignedBlock, ctx: &mut DumpContext) {
    let header = block.header();
    let txs = block.transactions_vec();
    let sig_count = block.signatures().count();
    println!(
        "{}: height={} view={} txs={} errors={} signatures={}",
        path.display(),
        header.height(),
        header.view_change_index(),
        txs.len(),
        block.errors().count(),
        sig_count
    );
    for (idx, tx) in txs.iter().enumerate() {
        println!("  tx[{idx}] hash={}", tx.hash());
        match tx.instructions() {
            Executable::Instructions(instrs) => {
                for (instr_idx, instr) in instrs.iter().enumerate() {
                    ctx.record_mint(instr);
                    if ctx.verbose {
                        println!("    instr[{instr_idx}]: {instr:?}");
                    }
                }
            }
            Executable::Ivm(bytecode) => {
                if ctx.verbose {
                    println!("    ivm bytecode: {} bytes", bytecode.size_bytes());
                }
            }
        }
    }
}

fn dump_pipeline(path: &Path, sidecar: &PipelineRecoverySidecar) {
    println!(
        "{}: pipeline format={} height={} txs={}",
        path.display(),
        sidecar.format_label(),
        sidecar.height,
        sidecar.txs.len()
    );
    for (idx, tx) in sidecar.txs.iter().enumerate() {
        println!(
            "  tx[{idx}] hash={} reads={} writes={}",
            tx.hash,
            format_args!("{:?}", tx.reads),
            format_args!("{:?}", tx.writes)
        );
    }
}
