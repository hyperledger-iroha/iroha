use std::{collections::HashSet, fs::File, path::PathBuf};

use iroha_genesis::RawGenesisTransaction;
use iroha_schema::{IntoSchema, MetaMap, Metadata};

use super::*;
use crate::tui;

#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Optional path to output genesis schema
    #[clap(long = "genesis-out")]
    genesis_out: Option<PathBuf>,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Generating Iroha schema descriptors");
        let schemas = iroha_schema_gen::build_schemas();
        validate_schema(&schemas);
        writeln!(writer, "{}", norito::json::to_json_pretty(&schemas)?)
            .wrap_err("Failed to write schema.")?;

        if let Some(path) = self.genesis_out {
            let mut genesis_map = MetaMap::new();
            RawGenesisTransaction::update_schema_map(&mut genesis_map);
            let mut file = BufWriter::new(File::create(path)?);
            writeln!(file, "{}", norito::json::to_json_pretty(&genesis_map)?)
                .wrap_err("Failed to write genesis schema.")?;
        }

        tui::success("Schema generation complete");
        Ok(())
    }
}

fn validate_schema(map: &MetaMap) {
    let known: HashSet<_> = map.iter().map(|(id, _)| *id).collect();
    for (type_id, entry) in map.iter() {
        if let Metadata::Vec(vec_meta) = &entry.metadata {
            assert!(
                known.contains(&vec_meta.ty),
                "Schema entry {} ({type_id:?}) references missing Vec element type {:?}",
                entry.type_name,
                vec_meta.ty
            );
        }
    }
}
