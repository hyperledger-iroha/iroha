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

    fn assert_known(
        known: &HashSet<core::any::TypeId>,
        owner_name: &str,
        owner_type_id: core::any::TypeId,
        context: &str,
        referenced: core::any::TypeId,
    ) {
        assert!(
            known.contains(&referenced),
            "Schema entry {owner_name} ({owner_type_id:?}) references missing {context} type {referenced:?}",
        );
    }

    for (type_id, entry) in map.iter() {
        let owner_id = *type_id;
        match &entry.metadata {
            Metadata::Struct(struct_meta) => {
                for declaration in &struct_meta.declarations {
                    let context = format!("field `{}`", declaration.name);
                    assert_known(&known, &entry.type_name, owner_id, &context, declaration.ty);
                }
            }
            Metadata::Tuple(tuple_meta) => {
                for (idx, ty) in tuple_meta.types.iter().enumerate() {
                    let context = format!("tuple field {idx}");
                    assert_known(&known, &entry.type_name, owner_id, &context, *ty);
                }
            }
            Metadata::Enum(enum_meta) => {
                for variant in &enum_meta.variants {
                    if let Some(ty) = variant.ty {
                        let context = format!("enum variant `{}`", variant.tag);
                        assert_known(&known, &entry.type_name, owner_id, &context, ty);
                    }
                }
            }
            Metadata::FixedPoint(fixed_meta) => {
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "fixed-point base",
                    fixed_meta.base,
                );
            }
            Metadata::Array(array_meta) => {
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "array element",
                    array_meta.ty,
                );
            }
            Metadata::Vec(vec_meta) => {
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "Vec element",
                    vec_meta.ty,
                );
            }
            Metadata::Map(map_meta) => {
                assert_known(&known, &entry.type_name, owner_id, "map key", map_meta.key);
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "map value",
                    map_meta.value,
                );
            }
            Metadata::Option(ty) => {
                assert_known(&known, &entry.type_name, owner_id, "Option value", *ty);
            }
            Metadata::Result(result_meta) => {
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "Result ok",
                    result_meta.ok,
                );
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "Result err",
                    result_meta.err,
                );
            }
            Metadata::Bitmap(bitmap_meta) => {
                assert_known(
                    &known,
                    &entry.type_name,
                    owner_id,
                    "bitmap repr",
                    bitmap_meta.repr,
                );
            }
            Metadata::Int(_) | Metadata::String | Metadata::Bool => {}
        }
    }
}
