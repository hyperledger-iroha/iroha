//! Explicit materialization of non-signable genesis source templates.

use super::generate::load_kagemusha_mint_finality_parameters;
use crate::{Outcome, RunArgs, tui};
use clap::Parser;
use color_eyre::eyre::WrapErr as _;
use iroha_genesis::{GenesisSourceTemplate, validate_genesis_manifest_json};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

/// Materialize a `.template.json` source with operator-provisioned public authority.
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Incomplete genesis source file; the name must end in `.template.json`.
    template_file: PathBuf,
    /// Explicitly provisioned public KAGEMUSHA mint-finality genesis parameters.
    #[arg(long, value_name = "PATH")]
    kagemusha_mint_finality_parameters: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Materializing genesis source template");
        let parameters =
            load_kagemusha_mint_finality_parameters(&self.kagemusha_mint_finality_parameters)?;
        let manifest = GenesisSourceTemplate::from_path(&self.template_file)?
            .materialize(parameters)
            .wrap_err("materialize complete genesis manifest")?;
        super::ensure_kagemusha_mint_finality_schedule_matches_consensus(&manifest)?;
        let topology = manifest
            .transactions()
            .iter()
            .flat_map(|transaction| transaction.topology())
            .map(|entry| entry.peer.clone())
            .collect::<Vec<_>>();
        if !topology.is_empty() {
            super::ensure_kagemusha_mint_finality_epoch_zero_authority_matches_topology(
                &manifest, &topology,
            )?;
        }
        let mut json = norito::json::to_json_pretty(&manifest)?;
        json.push('\n');
        validate_genesis_manifest_json(json.as_bytes())
            .wrap_err("materialized genesis exceeds fixed resource bounds")?;
        writer
            .write_all(json.as_bytes())
            .wrap_err("write materialized genesis manifest")?;
        tui::success("Genesis source template materialized");
        Ok(())
    }
}
