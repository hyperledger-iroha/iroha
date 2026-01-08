use std::{
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::PathBuf,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::WrapErr as _;
use inquire::{Confirm, CustomType, Text};
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use iroha_test_samples::ALICE_ID;

use crate::{
    Outcome, RunArgs,
    genesis::build_line_from_env,
    localnet::{
        AssetSpec, DEFAULT_BIND_HOST, DEFAULT_PUBLIC_HOST, LocalnetOptions, generate_localnet,
    },
    tui,
};

/// Interactive TUI to generate a bare-metal local network (no Docker).
#[derive(ClapArgs, Debug, Clone)]
pub struct LocalnetWizardArgs;

impl<T: Write> RunArgs<T> for LocalnetWizardArgs {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let peers: NonZeroU16 = CustomType::new("How many peers?")
            .with_default(4u16)
            .prompt()?
            .try_into()
            .map_err(|_| color_eyre::eyre::eyre!("peer count must be > 0"))?;
        let seed = if Confirm::new("Use a deterministic seed?")
            .with_default(true)
            .prompt()?
        {
            Some(Text::new("Enter seed").with_default("Iroha").prompt()?)
        } else {
            None
        };
        let base_api: u16 = CustomType::new("Base API port?")
            .with_default(8080u16)
            .prompt()?;
        let base_p2p: u16 = CustomType::new("Base P2P port?")
            .with_default(1337u16)
            .prompt()?;
        let out_dir = Text::new("Output directory?")
            .with_default("/tmp/iroha-localnet")
            .prompt()?;

        let extra_accounts: u16 = CustomType::new("Extra accounts (in wonderland)?")
            .with_default(0u16)
            .prompt()?;

        let mut assets: Vec<AssetSpec> = Vec::new();
        if Confirm::new("Register a sample asset?")
            .with_default(true)
            .prompt()?
        {
            let count: u16 = CustomType::new("How many assets?")
                .with_default(1u16)
                .prompt()?;
            for i in 0..count {
                let default_id = format!("asset{i}#wonderland");
                let id = Text::new("Asset id (asset#domain)")
                    .with_default(&default_id)
                    .prompt()?;
                let qty: u64 = CustomType::new("Mint quantity to alice@wonderland?")
                    .with_default(100u64)
                    .prompt()?;
                assets.push(AssetSpec {
                    id,
                    mint_to: ALICE_ID.clone(),
                    quantity: qty,
                });
            }
        }

        let build_line = build_line_from_env();
        let consensus_mode = if build_line.is_iroha3() {
            SumeragiConsensusMode::Npos
        } else {
            SumeragiConsensusMode::Permissioned
        };
        let opts = LocalnetOptions {
            peers,
            seed,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: base_api,
            base_p2p_port: base_p2p,
            out_dir: PathBuf::from(out_dir),
            extra_accounts,
            assets,
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        tui::status("Generating localnet (interactive)");
        generate_localnet(&opts, writer).wrap_err("localnet wizard")
    }
}
