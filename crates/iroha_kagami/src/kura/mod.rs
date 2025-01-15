use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{eyre, WrapErr as _};
use iroha_core::kura::{BlockIndex, BlockStore};
use iroha_data_model::block::SignedBlock;
use iroha_version::scale::DecodeVersioned;

use crate::{Outcome, RunArgs};

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
            Command::Print { length } => print_blockchain(
                writer,
                &args.path_to_block_store,
                from_height.unwrap_or(u64::MAX),
                length,
            )
            .wrap_err("failed to print blockchain"),
        }
    }
}

fn print_blockchain(
    writer: &mut dyn Write,
    block_store_path: &Path,
    from_height: u64,
    block_count: u64,
) -> Outcome {
    let mut block_store_path: std::borrow::Cow<'_, Path> = block_store_path.into();

    if let Some(os_str_file_name) = block_store_path.file_name() {
        let file_name_str = os_str_file_name.to_str().unwrap_or("");
        if file_name_str == "blocks.data" || file_name_str == "blocks.index" {
            block_store_path.to_mut().pop();
        }
    }

    let block_store = BlockStore::new(&block_store_path);

    let index_count = block_store
        .read_index_count()
        .wrap_err("failed to read index count from block store {block_store_path:?}.")?;

    if index_count == 0 {
        return Err(eyre!("Index count is zero. This could be because there are no blocks in the store: {block_store_path:?}"));
    }

    let from_height = if from_height >= index_count {
        index_count - 1
    } else {
        from_height
    };

    let block_count = if from_height + block_count > index_count {
        index_count - from_height
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
    writeln!(
        writer,
        "Index file says there are {index_count} blocks.",
    )?;
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
        let block = SignedBlock::decode_all_versioned(&block_buf)
            .wrap_err(format!("Failed to decode block № {}", meta_index + 1))?;
        writeln!(writer, "Block#{} :", meta_index + 1)?;
        writeln!(writer, "{block:#?}")?;
    }

    Ok(())
}
