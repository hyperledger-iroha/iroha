use crate::{Outcome, RunArgs, tui};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::{
    eyre::{Result, eyre},
    owo_colors::OwoColorize,
};
use iroha_data_model::{account::NewAccount, asset::AssetId, domain::Domain, peer::Peer};
use iroha_genesis::RawGenesisTransaction;
use norito::{
    codec::{DecodeAll, Encode},
    json::{JsonDeserializeOwned, JsonSerialize},
};
use std::{
    collections::BTreeMap,
    fmt::{self, Debug, Write as _},
    fs::File,
    io,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
// RawGenesisTransaction is the largest type registered by this first-release diagnostic. Reuse
// the signed-genesis corridor so stdin, local files, decoded values, and rendered output cannot
// grow beyond the artifact class the command is intended to inspect.
const MAX_CODEC_INPUT_BYTES_V1: usize = iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1;
const MAX_CODEC_OUTPUT_BYTES_V1: usize = iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1;
const CODEC_DECODE_LIMITS_V1: norito::DecodeLimits =
    iroha_genesis::signed_genesis_decode_limits_v1();
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
    insert_converter::<AssetId>(&mut map);
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
struct BoundedDebugString {
    value: String,
    max_bytes: usize,
}
impl BoundedDebugString {
    const fn new(max_bytes: usize) -> Self {
        Self {
            value: String::new(),
            max_bytes,
        }
    }
    fn finish(self) -> String {
        self.value
    }
}
impl fmt::Write for BoundedDebugString {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next_len = self
            .value
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if next_len > self.max_bytes {
            return Err(fmt::Error);
        }
        self.value
            .try_reserve_exact(value.len())
            .map_err(|_| fmt::Error)?;
        self.value.push_str(value);
        Ok(())
    }
}
fn charge_guessed_output(
    retained_bytes: usize,
    type_name_bytes: usize,
    formatted_bytes: usize,
    max_bytes: usize,
) -> Result<usize> {
    let candidate_bytes = type_name_bytes
        .checked_add(formatted_bytes)
        .and_then(|length| length.checked_add(3))
        .ok_or_else(|| eyre!("guessed codec output length overflow"))?;
    let retained_bytes = retained_bytes
        .checked_add(candidate_bytes)
        .ok_or_else(|| eyre!("guessed codec output length overflow"))?;
    if retained_bytes > max_bytes {
        return Err(eyre!(
            "combined guessed codec output exceeds the first-release {max_bytes}-byte limit"
        ));
    }
    Ok(retained_bytes)
}
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
        let object =
            norito::with_decode_limits_scope(CODEC_DECODE_LIMITS_V1, || T::decode_all(&mut input))?;
        let mut output = BoundedDebugString::new(MAX_CODEC_OUTPUT_BYTES_V1);
        write!(&mut output, "{object:#?}").map_err(|_| {
            eyre!(
                "Rust debug output exceeds the first-release {}-byte codec limit",
                MAX_CODEC_OUTPUT_BYTES_V1
            )
        })?;
        Ok(output.finish())
    }
    fn norito_to_json(&self, input: &[u8]) -> Result<String> {
        let object = norito::with_decode_limits_scope(CODEC_DECODE_LIMITS_V1, || {
            norito::decode_from_bytes::<T>(input)
        })?;
        let json = norito::json::to_json_bounded(&object, MAX_CODEC_OUTPUT_BYTES_V1)?;
        Ok(json)
    }
    fn json_to_norito(&self, input: &str) -> Result<Vec<u8>> {
        norito::json::preflight_slice(
            input.as_bytes(),
            norito::json::JsonPreflightLimits::from_decode_limits(
                MAX_CODEC_INPUT_BYTES_V1,
                CODEC_DECODE_LIMITS_V1,
            ),
        )?;
        let object: T = norito::with_decode_limits_scope(CODEC_DECODE_LIMITS_V1, || {
            norito::json::from_str(input)
        })?;
        norito::core::to_bytes_bounded(&object, MAX_CODEC_OUTPUT_BYTES_V1).map_err(Into::into)
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
fn read_codec_input_bounded<R: Read + ?Sized>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let read_len = remaining.min(chunk.len());
        let count = loop {
            match reader.read(&mut chunk[..read_len]) {
                Ok(count) => break count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        };
        if count == 0 {
            return Ok(bytes);
        }
        bytes
            .try_reserve_exact(count)
            .map_err(|error| eyre!("failed to reserve {label} buffer storage: {error}"))?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let mut growth_probe = [0_u8; 1];
    let extra = loop {
        match reader.read(&mut growth_probe) {
            Ok(count) => break count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    };
    if extra != 0 {
        return Err(eyre!(
            "{label} exceeds the first-release {max_bytes}-byte codec limit"
        ));
    }
    Ok(bytes)
}
fn read_codec_file_bounded(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(eyre!(
            "codec input is not a regular file: {}",
            path.display()
        ));
    }
    if before.len() > MAX_CODEC_INPUT_BYTES_V1 as u64 {
        return Err(eyre!(
            "codec input {} exceeds the first-release {}-byte limit",
            path.display(),
            MAX_CODEC_INPUT_BYTES_V1
        ));
    }
    let bytes = read_codec_input_bounded(&mut file, MAX_CODEC_INPUT_BYTES_V1, "codec input")?;
    let after = file.metadata()?;
    if before.len() != after.len() || after.len() != bytes.len() as u64 {
        return Err(eyre!(
            "codec input changed while it was being read: {}",
            path.display()
        ));
    }
    Ok(bytes)
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
        let bytes = read_codec_file_bounded(&self.args.binary)?;
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
        // Guessing is deliberately sequential. Every successful converter can render up to the
        // full output corridor, so parallel collection would multiply peak retention by the
        // number of registered types even though the final output is one stream.
        let mut matches = Vec::new();
        let mut retained_bytes = 0_usize;
        for (type_name, converter) in self.map {
            let mut buf = Vec::new();
            if Self::dump_decoded(converter.as_ref(), bytes, &mut buf).is_err() {
                continue;
            }
            let formatted = match String::from_utf8(buf) {
                Ok(value) => value,
                Err(_) => continue,
            };
            retained_bytes = charge_guessed_output(
                retained_bytes,
                type_name.len(),
                formatted.len(),
                MAX_CODEC_OUTPUT_BYTES_V1,
            )?;
            matches.push((type_name.clone(), formatted));
        }
        for (type_name, formatted) in &matches {
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
            Some(path) => {
                let file = File::open(&path)?;
                let metadata = file.metadata()?;
                if !metadata.is_file() {
                    return Err(eyre!(
                        "codec input is not a regular file: {}",
                        path.display()
                    ));
                }
                if metadata.len() > MAX_CODEC_INPUT_BYTES_V1 as u64 {
                    return Err(eyre!(
                        "codec input {} exceeds the first-release {}-byte limit",
                        path.display(),
                        MAX_CODEC_INPUT_BYTES_V1
                    ));
                }
                Box::new(BufReader::new(file))
            }
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
        let input = read_codec_input_bounded(
            reader.as_mut(),
            MAX_CODEC_INPUT_BYTES_V1,
            "Norito codec input",
        )?;
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
        let input = read_codec_input_bounded(
            reader.as_mut(),
            MAX_CODEC_INPUT_BYTES_V1,
            "JSON codec input",
        )?;
        let input = String::from_utf8(input)
            .map_err(|error| eyre!("JSON codec input is not valid UTF-8: {error}"))?;
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
    use super::{
        BoundedDebugString, Converter, ConverterImpl, ConverterMap, NoritoToRustArgs,
        NoritoToRustDecoder, charge_guessed_output, generate_map, read_codec_input_bounded,
    };
    use color_eyre::eyre::Result as EyreResult;
    use iroha_data_model::{account::NewAccount, asset::AssetId, peer::Peer};
    use iroha_genesis::RawGenesisTransaction;
    use iroha_schema::{Compact, TypeId};
    use std::{fmt::Write as _, path::PathBuf, sync::Arc};
    fn normalize_roundtrip_json(value: &mut norito::json::Value) {
        let norito::json::Value::Object(map) = value else {
            return;
        };
        if matches!(map.get("domain"), Some(norito::json::Value::Null)) {
            map.remove("domain");
        }
    }
    #[test]
    fn bounded_codec_reader_accepts_exact_limit() {
        let input = [0xA5; 32];
        let mut reader = input.as_slice();
        let bytes = read_codec_input_bounded(&mut reader, input.len(), "test input")
            .expect("exact limit is accepted");
        assert_eq!(bytes, input);
    }
    #[test]
    fn bounded_codec_reader_rejects_limit_plus_one() {
        let input = [0xA5; 33];
        let mut reader = input.as_slice();
        let error = read_codec_input_bounded(&mut reader, input.len() - 1, "test input")
            .expect_err("limit plus one must be rejected");
        assert!(error.to_string().contains("32-byte codec limit"));
    }
    #[test]
    fn bounded_debug_writer_rejects_growth_before_append() {
        let mut output = BoundedDebugString::new(3);
        output.write_str("abc").expect("exact limit");
        assert!(output.write_str("d").is_err());
        assert_eq!(output.finish(), "abc");
    }
    #[test]
    fn guessed_output_charge_accepts_exact_limit_and_rejects_plus_one() {
        assert_eq!(
            charge_guessed_output(4, 2, 1, 10).expect("exact aggregate limit"),
            10
        );
        assert!(charge_guessed_output(4, 2, 2, 10).is_err());
    }
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
        let mut expected: norito::json::Value = norito::json::from_str(&json).unwrap();
        let mut actual: norito::json::Value = norito::json::from_str(&json_out).unwrap();
        normalize_roundtrip_json(&mut expected);
        normalize_roundtrip_json(&mut actual);
        assert_eq!(expected, actual);
    }
    #[test]
    fn generate_map_covers_schema_types() {
        let map = generate_map();
        let expected = [
            <NewAccount as TypeId>::id(),
            <AssetId as TypeId>::id(),
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
