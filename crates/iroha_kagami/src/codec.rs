use std::{
    collections::BTreeMap,
    fmt::Debug,
    fs,
    fs::File,
    io,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    marker::PhantomData,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::{Args as ClapArgs, Subcommand};
use color_eyre::{
    eyre::{Result, eyre},
    owo_colors::OwoColorize,
};
use iroha_data_model::{account::NewAccount, domain::Domain, peer::Peer};
use iroha_genesis::RawGenesisTransaction;
use norito::{
    codec::{DecodeAll, Encode},
    json::{JsonDeserializeOwned, JsonSerialize},
};

use crate::{Outcome, RunArgs, tui};

/// Generate map with types and converter trait object
fn generate_map() -> ConverterMap {
    fn insert_converter<T>(map: &mut ConverterMap)
    where
        T: Debug + Encode + DecodeAll + JsonSerialize + JsonDeserializeOwned,
        T: iroha_schema::TypeId + Send + Sync + 'static,
    {
        let type_id = <T as iroha_schema::TypeId>::id();
        map.entry(type_id).or_insert_with(ConverterImpl::<T>::boxed);
    }

    let mut map = ConverterMap::new();

    insert_converter::<NewAccount>(&mut map);
    insert_converter::<Domain>(&mut map);
    insert_converter::<Peer>(&mut map);

    insert_converter::<RawGenesisTransaction>(&mut map);

    macro_rules! register_compact_as {
        ($compact:ty => $inner:ty) => {{
            let type_id = <$compact as iroha_schema::TypeId>::id();
            map.entry(type_id)
                .or_insert_with(ConverterImpl::<$inner>::boxed);
        }};
    }

    register_compact_as!(iroha_schema::Compact<u128> => u128);
    register_compact_as!(iroha_schema::Compact<u64> => u64);
    register_compact_as!(iroha_schema::Compact<u32> => u32);

    map
}

type ConverterMap = BTreeMap<String, Arc<dyn Converter>>;

struct ConverterImpl<T>(PhantomData<T>);

impl<T> ConverterImpl<T>
where
    T: Debug + Encode + DecodeAll + JsonSerialize + JsonDeserializeOwned,
    T: Send + Sync + 'static,
{
    fn boxed() -> Arc<dyn Converter> {
        Arc::new(Self(PhantomData))
    }
}

trait Converter: Send + Sync {
    fn norito_to_rust(&self, input: &[u8]) -> Result<String>;
    fn norito_to_json(&self, input: &[u8]) -> Result<String>;
    fn json_to_norito(&self, input: &str) -> Result<Vec<u8>>;
}

impl<T> Converter for ConverterImpl<T>
where
    T: Debug + Encode + DecodeAll + JsonSerialize + JsonDeserializeOwned,
    T: Send + Sync + 'static,
{
    fn norito_to_rust(&self, mut input: &[u8]) -> Result<String> {
        let object = T::decode_all(&mut input)?;
        Ok(format!("{object:#?}"))
    }
    fn norito_to_json(&self, input: &[u8]) -> Result<String> {
        let object = norito::decode_from_bytes::<T>(input)?;
        let json = norito::json::to_json(&object)?;
        Ok(json)
    }
    fn json_to_norito(&self, input: &str) -> Result<Vec<u8>> {
        let object: T = norito::json::from_str(input)?;
        norito::to_bytes(&object).map_err(Into::into)
    }
}

/// Norito decoder for Iroha data types
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    /// Show all available types
    ListTypes,
    /// Decode Norito to Rust debug format from binary file
    NoritoToRust(NoritoToRustArgs),
    /// Decode Norito to JSON. By default uses stdin and stdout
    NoritoToJson(NoritoJsonArgs),
    /// Encode JSON as Norito. By default uses stdin and stdout
    JsonToNorito(NoritoJsonArgs),
}

#[derive(Debug, ClapArgs, Clone)]
struct NoritoToRustArgs {
    /// Path to the binary with encoded Iroha structure
    binary: PathBuf,
    /// Type that is expected to be encoded in binary.
    /// If not specified then a guess will be attempted
    #[clap(short, long = "type")]
    type_name: Option<String>,
}

#[derive(Debug, ClapArgs, Clone)]
struct NoritoJsonArgs {
    /// Path to the input file
    #[clap(short, long)]
    input: Option<PathBuf>,
    /// Path to the output file
    #[clap(short, long)]
    output: Option<PathBuf>,
    /// Type that is expected to be encoded in input
    #[clap(short, long = "type")]
    type_name: String,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let map = generate_map();

        match self.command {
            Command::NoritoToRust(decode_args) => {
                tui::status("Decoding Norito payload to Rust debug view");
                let decoder = NoritoToRustDecoder::new(decode_args, &map);
                decoder.decode(writer)?;
                tui::success("Decoded payload");
                Ok(())
            }
            Command::NoritoToJson(args) => {
                tui::status("Decoding Norito payload to JSON");
                let mut file_writer = match args.output.clone() {
                    None => None,
                    Some(path) => Some(BufWriter::new(File::create(path)?)),
                };

                let writer: &mut dyn Write = file_writer
                    .as_mut()
                    .map_or(writer, |file_writer| file_writer);
                let decoder = NoritoJsonDecoder::new(args, &map, writer)?;
                decoder.norito_to_json()?;
                tui::success("Converted to JSON");
                Ok(())
            }
            Command::JsonToNorito(args) => {
                tui::status("Encoding JSON payload to Norito");
                let mut file_writer = match args.output.clone() {
                    None => None,
                    Some(path) => Some(BufWriter::new(File::create(path)?)),
                };

                let writer: &mut dyn Write = file_writer
                    .as_mut()
                    .map_or(writer, |file_writer| file_writer);
                let decoder = NoritoJsonDecoder::new(args, &map, writer)?;
                decoder.json_to_norito()?;
                tui::success("Encoded Norito payload");
                Ok(())
            }
            Command::ListTypes => {
                tui::status("Listing supported Norito types");
                list_types(&map, writer)?;
                tui::success("Type list complete");
                Ok(())
            }
        }
    }
}

/// Type decoder
struct NoritoToRustDecoder<'map> {
    args: NoritoToRustArgs,
    map: &'map ConverterMap,
}

impl<'map> NoritoToRustDecoder<'map> {
    /// Create new `Decoder` with `args` and `map`
    pub fn new(args: NoritoToRustArgs, map: &'map ConverterMap) -> Self {
        Self { args, map }
    }

    /// Decode type and print to `writer`
    pub fn decode<W: io::Write>(&self, writer: &mut W) -> Result<()> {
        let bytes = fs::read(self.args.binary.clone())?;

        if let Some(type_name) = &self.args.type_name {
            return self.decode_by_type(type_name, &bytes, writer);
        }
        self.decode_by_guess(&bytes, writer)
    }

    /// Decode concrete `type` from `bytes` and print to `writer`
    fn decode_by_type<W: io::Write>(
        &self,
        type_name: &str,
        bytes: &[u8],
        writer: &mut W,
    ) -> Result<()> {
        self.map.get(type_name).map_or_else(
            || Err(eyre!("Unknown type: `{type_name}`")),
            |converter| Self::dump_decoded(converter.as_ref(), bytes, writer),
        )
    }

    /// Try to decode every type from `bytes` and print to `writer`
    fn decode_by_guess<W: io::Write>(&self, bytes: &[u8], writer: &mut W) -> Result<()> {
        let entries: Vec<_> = self.map.iter().enumerate().collect();

        let workers = thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        let task_count = entries.len();
        let active_workers = workers.min(task_count.max(1));
        let chunk_size = task_count.div_ceil(active_workers);

        let results = Mutex::new(Vec::new());

        // Partition converters across workers and keep results ordered by the
        // original map index so output stays deterministic.
        thread::scope(|scope| {
            for chunk in entries.chunks(chunk_size.max(1)) {
                if chunk.is_empty() {
                    continue;
                }

                let results = &results;
                scope.spawn(move || {
                    let mut local = Vec::new();

                    for &(index, (type_name, converter)) in chunk {
                        let mut buf = Vec::new();
                        if Self::dump_decoded(converter.as_ref(), bytes, &mut buf).is_err() {
                            continue;
                        }
                        let formatted = match String::from_utf8(buf) {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        local.push((index, type_name.clone(), formatted));
                    }

                    if local.is_empty() {
                        return;
                    }

                    results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .extend(local);
                });
            }
        });

        let mut matches: Vec<(usize, String, String)> = results
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        matches.sort_by_key(|(index, _, _)| *index);

        for (_, type_name, formatted) in &matches {
            writeln!(
                writer,
                "{}:\n{}",
                type_name.as_str().italic().cyan(),
                formatted
            )?;
        }

        match matches.len() {
            0 => writeln!(writer, "No compatible types found"),
            1 => writeln!(writer, "{} compatible type found", "1".bold()),
            n => writeln!(writer, "{} compatible types found", n.to_string().bold()),
        }
        .map_err(Into::into)
    }

    fn dump_decoded(converter: &dyn Converter, input: &[u8], w: &mut dyn io::Write) -> Result<()> {
        let result = converter.norito_to_rust(input)?;
        writeln!(w, "{result}")?;
        Ok(())
    }
}

struct NoritoJsonDecoder<'map, 'w> {
    reader: Box<dyn BufRead>,
    writer: &'w mut dyn Write,
    converter: &'map dyn Converter,
}

impl<'map, 'w> NoritoJsonDecoder<'map, 'w> {
    fn new(
        args: NoritoJsonArgs,
        map: &'map ConverterMap,
        writer: &'w mut dyn Write,
    ) -> Result<Self> {
        let reader: Box<dyn BufRead> = match args.input {
            None => Box::new(io::stdin().lock()),
            Some(path) => Box::new(BufReader::new(File::open(path)?)),
        };
        let Some(converter) = map.get(&args.type_name) else {
            return Err(eyre!("Unknown type: `{}`", args.type_name));
        };
        Ok(Self {
            reader,
            writer,
            converter: converter.as_ref(),
        })
    }

    fn norito_to_json(self) -> Result<()> {
        let Self {
            mut reader,
            writer,
            converter,
        } = self;
        let mut input = Vec::new();
        reader.read_to_end(&mut input)?;
        let output = converter.norito_to_json(&input)?;
        writeln!(writer, "{output}")?;
        Ok(())
    }

    fn json_to_norito(self) -> Result<()> {
        let Self {
            mut reader,
            writer,
            converter,
        } = self;
        let mut input = String::new();
        reader.read_to_string(&mut input)?;
        let output = converter.json_to_norito(&input)?;
        writer.write_all(&output)?;
        Ok(())
    }
}

/// Print all supported types from `map` to `writer`
fn list_types<W: io::Write>(map: &ConverterMap, writer: &mut W) -> Result<()> {
    for key in map.keys() {
        writeln!(writer, "{key}")?;
    }
    if !map.is_empty() {
        writeln!(writer)?;
    }

    match map.len() {
        0 => writeln!(writer, "No type is supported"),
        1 => writeln!(writer, "{} type is supported", "1".bold()),
        n => writeln!(writer, "{} types are supported", n.to_string().bold()),
    }
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use color_eyre::eyre::Result as EyreResult;
    use iroha_data_model::{account::NewAccount, peer::Peer};
    use iroha_genesis::RawGenesisTransaction;
    use iroha_schema::{Compact, TypeId};

    use super::{
        Converter, ConverterImpl, ConverterMap, NoritoToRustArgs, NoritoToRustDecoder, generate_map,
    };

    #[test]
    fn json_norito_roundtrip() {
        let converter = ConverterImpl::<NewAccount>::boxed();
        let json = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/codec/account.json"
        ))
        .expect("sample json");
        let norito = converter
            .json_to_norito(&json)
            .expect("encode json to norito");
        let json_out = converter
            .norito_to_json(&norito)
            .expect("decode norito to json");
        let expected: norito::json::Value = norito::json::from_str(&json).unwrap();
        let actual: norito::json::Value = norito::json::from_str(&json_out).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn generate_map_covers_schema_types() {
        let map = generate_map();
        let expected = [
            <NewAccount as TypeId>::id(),
            <Peer as TypeId>::id(),
            <RawGenesisTransaction as TypeId>::id(),
        ];

        for type_id in expected {
            assert!(
                map.contains_key(&type_id),
                "missing converter for {type_id}"
            );
        }
    }

    #[test]
    fn generate_map_contains_compact_numeric_aliases() {
        let map = generate_map();
        let long = <Compact<u128> as TypeId>::id();
        let medium = <Compact<u64> as TypeId>::id();
        let short = <Compact<u32> as TypeId>::id();

        assert!(map.contains_key(&long));
        assert!(map.contains_key(&medium));
        assert!(map.contains_key(&short));
    }

    #[test]
    fn decode_by_guess_preserves_order_and_reports_matches() {
        struct TestConverter {
            render: &'static str,
        }

        impl Converter for TestConverter {
            fn norito_to_rust(&self, _input: &[u8]) -> EyreResult<String> {
                Ok(self.render.to_string())
            }

            fn norito_to_json(&self, _input: &[u8]) -> EyreResult<String> {
                Ok(format!("\"{}\"", self.render))
            }

            fn json_to_norito(&self, _input: &str) -> EyreResult<Vec<u8>> {
                Ok(self.render.as_bytes().to_vec())
            }
        }

        let mut map: ConverterMap = ConverterMap::new();
        let alpha: Arc<dyn Converter> = Arc::new(TestConverter { render: "alpha" });
        let beta: Arc<dyn Converter> = Arc::new(TestConverter { render: "beta" });
        map.insert("Alpha".to_owned(), alpha);
        map.insert("Beta".to_owned(), beta);

        let decoder = NoritoToRustDecoder::new(
            NoritoToRustArgs {
                binary: PathBuf::new(),
                type_name: None,
            },
            &map,
        );

        let mut output = Vec::new();
        decoder
            .decode_by_guess(b"", &mut output)
            .expect("decoder succeeds");
        let output = String::from_utf8(output).expect("valid UTF-8");

        let alpha_pos = output.find("Alpha").expect("Alpha reported");
        let beta_pos = output.find("Beta").expect("Beta reported");
        assert!(
            alpha_pos < beta_pos,
            "type order should remain deterministic"
        );
        assert!(
            output.contains("compatible types found"),
            "summary should mention matching types"
        );
    }
}
