use std::{
    fmt,
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::PathBuf,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::WrapErr as _;
use inquire::{Confirm, CustomType, Select, Text};
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use iroha_test_samples::ALICE_ID;

use crate::{
    Outcome, RunArgs,
    localnet::{
        AssetSpec, BuildLineArg, DEFAULT_BIND_HOST, DEFAULT_PUBLIC_HOST,
        LOCALNET_SAMPLE_ASSET_NAME, LocalnetOptions, SoraProfile,
        canonical_asset_definition_literal, generate_localnet,
    },
    tui,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoraProfileChoice {
    None,
    Dataspace,
    Nexus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsensusModeChoice {
    Permissioned,
    Npos,
}

impl fmt::Display for SoraProfileChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoraProfileChoice::None => write!(f, "none (single-lane)"),
            SoraProfileChoice::Dataspace => write!(f, "dataspace (multi-lane defaults)"),
            SoraProfileChoice::Nexus => write!(f, "Sora Nexus (public dataspace)"),
        }
    }
}

impl fmt::Display for ConsensusModeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusModeChoice::Permissioned => write!(f, "permissioned"),
            ConsensusModeChoice::Npos => write!(f, "npos"),
        }
    }
}

impl From<ConsensusModeChoice> for SumeragiConsensusMode {
    fn from(value: ConsensusModeChoice) -> Self {
        match value {
            ConsensusModeChoice::Permissioned => SumeragiConsensusMode::Permissioned,
            ConsensusModeChoice::Npos => SumeragiConsensusMode::Npos,
        }
    }
}

struct ConsensusModePrompt {
    choices: Vec<ConsensusModeChoice>,
    default_index: usize,
    locked: bool,
}

fn consensus_mode_prompt(
    build_line: iroha_version::BuildLine,
    sora_profile: Option<SoraProfile>,
) -> ConsensusModePrompt {
    if !build_line.is_iroha3() {
        return ConsensusModePrompt {
            choices: vec![ConsensusModeChoice::Permissioned],
            default_index: 0,
            locked: true,
        };
    }
    if matches!(sora_profile, Some(SoraProfile::Nexus)) {
        return ConsensusModePrompt {
            choices: vec![ConsensusModeChoice::Npos],
            default_index: 0,
            locked: true,
        };
    }
    ConsensusModePrompt {
        choices: vec![ConsensusModeChoice::Permissioned, ConsensusModeChoice::Npos],
        default_index: 0,
        locked: false,
    }
}

/// Interactive TUI to generate a bare-metal local network (no Docker).
#[derive(ClapArgs, Debug, Clone)]
pub struct LocalnetWizardArgs;

impl<T: Write> RunArgs<T> for LocalnetWizardArgs {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let peers: NonZeroU16 = CustomType::new("How many peers?")
            .with_default(4u16)
            .prompt()?
            .try_into()
            .map_err(|_| color_eyre::eyre::eyre!("peer count must be > 0"))?;
        let build_line = Select::new(
            "Build line?",
            vec![BuildLineArg::Iroha3, BuildLineArg::Iroha2],
        )
        .with_starting_cursor(0)
        .prompt()?;
        let build_line = iroha_version::BuildLine::from(build_line);
        let sora_profile = if build_line.is_iroha3() {
            let choice = Select::new(
                "Sora profile?",
                vec![
                    SoraProfileChoice::None,
                    SoraProfileChoice::Dataspace,
                    SoraProfileChoice::Nexus,
                ],
            )
            .with_starting_cursor(0)
            .prompt()?;
            match choice {
                SoraProfileChoice::None => None,
                SoraProfileChoice::Dataspace => Some(SoraProfile::Dataspace),
                SoraProfileChoice::Nexus => Some(SoraProfile::Nexus),
            }
        } else {
            None
        };
        let consensus_prompt = consensus_mode_prompt(build_line, sora_profile);
        let consensus_mode = if consensus_prompt.locked {
            consensus_prompt.choices[consensus_prompt.default_index].into()
        } else {
            Select::new("Consensus mode?", consensus_prompt.choices)
                .with_starting_cursor(consensus_prompt.default_index)
                .prompt()?
                .into()
        };
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
                let default_id =
                    canonical_asset_definition_literal("wonderland", &format!("asset{i}"));
                let id = Text::new("Asset definition id (Base58)")
                    .with_default(&default_id)
                    .prompt()?;
                let name = Text::new("Asset display name")
                    .with_default(LOCALNET_SAMPLE_ASSET_NAME)
                    .with_validator(|input: &str| {
                        use iroha_data_model::asset::definition::validate_asset_name;
                        match validate_asset_name(input) {
                            Ok(()) => Ok(inquire::validator::Validation::Valid),
                            Err(e) => Ok(inquire::validator::Validation::Invalid(
                                e.to_string().into(),
                            )),
                        }
                    })
                    .prompt()?;
                let qty: u64 = CustomType::new("Mint quantity to Alice (I105)?")
                    .with_default(100u64)
                    .prompt()?;
                assets.push(AssetSpec {
                    id,
                    name,
                    mint_to: ALICE_ID
                        .clone()
                        .to_account_id("wonderland".parse().expect("valid domain")),
                    quantity: qty,
                });
            }
        }

        let opts = LocalnetOptions {
            build_line,
            sora_profile,
            perf_profile: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_prompt_locks_permissioned_for_iroha2() {
        let prompt = consensus_mode_prompt(iroha_version::BuildLine::Iroha2, None);
        assert!(prompt.locked);
        assert_eq!(prompt.choices, vec![ConsensusModeChoice::Permissioned]);
        assert_eq!(prompt.default_index, 0);
    }

    #[test]
    fn consensus_prompt_locks_npos_for_sora_nexus() {
        let prompt =
            consensus_mode_prompt(iroha_version::BuildLine::Iroha3, Some(SoraProfile::Nexus));
        assert!(prompt.locked);
        assert_eq!(prompt.choices, vec![ConsensusModeChoice::Npos]);
        assert_eq!(prompt.default_index, 0);
    }

    #[test]
    fn consensus_prompt_allows_choice_for_non_nexus_iroha3() {
        let prompt = consensus_mode_prompt(iroha_version::BuildLine::Iroha3, None);
        assert!(!prompt.locked);
        assert_eq!(
            prompt.choices,
            vec![ConsensusModeChoice::Permissioned, ConsensusModeChoice::Npos]
        );
        assert_eq!(prompt.default_index, 0);
    }
}
