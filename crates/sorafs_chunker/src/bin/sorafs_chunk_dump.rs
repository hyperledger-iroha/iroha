//! Emits deterministic chunk metadata (offsets, lengths, BLAKE3 digests) for a
//! file or standard input. By default it uses the canonical SoraFS profile, but
//! callers can override the chunking parameters via CLI flags to experiment
//! with alternate layouts.
use std::{
    env,
    fmt::Write as _,
    fs::File,
    io::{self, Read},
    path::Path,
    process,
};

use sorafs_chunker::{ChunkDigest, ChunkProfile, chunk_bytes_with_digests_profile};

#[derive(Debug, Clone)]
struct CliConfig {
    profile: ChunkProfile,
    input: String,
}

#[derive(Debug)]
enum ParseError {
    Help,
    Message(String),
}

const USAGE: &str = "\
usage: sorafs-chunk-dump [OPTIONS] <path|->

Options:
    --profile <id>        Chunking profile preset (sf1, sorafs.sf1@1.0.0)
    --min-size <bytes>    Override minimum chunk size (in bytes)
    --target-size <bytes> Override target chunk size (in bytes)
    --max-size <bytes>    Override maximum chunk size (in bytes)
    --break-mask <mask>   Override rolling-hash break mask (decimal or 0x-prefixed hex)
    -h, --help            Show this help message

Examples:
    sorafs-chunk-dump --profile sf1 fixtures/sorafs_chunker/sf1_profile_v1_input.bin
    sorafs-chunk-dump --min-size 4096 --target-size 32768 --max-size 65536 myfile.bin
";

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        eprintln!("{USAGE}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = match parse_cli(env::args().skip(1)) {
        Ok(config) => config,
        Err(ParseError::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(ParseError::Message(err)) => return Err(err),
    };

    let input = read_input(&config.input)?;
    let output = generate_output(config.profile, &input);
    println!("{output}");
    Ok(())
}

fn parse_cli<I>(args: I) -> Result<CliConfig, ParseError>
where
    I: IntoIterator<Item = String>,
{
    let mut profile = ChunkProfile::DEFAULT;
    let mut min_override: Option<usize> = None;
    let mut target_override: Option<usize> = None;
    let mut max_override: Option<usize> = None;
    let mut mask_override: Option<u64> = None;
    let mut input: Option<String> = None;

    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(ParseError::Help),
            "--profile" => {
                let value = take_value(&mut iter, "--profile")?;
                profile = resolve_profile(&value)?;
            }
            "--min-size" => {
                let value = take_value(&mut iter, "--min-size")?;
                min_override = Some(parse_usize(&value, "--min-size")?);
            }
            "--target-size" => {
                let value = take_value(&mut iter, "--target-size")?;
                target_override = Some(parse_usize(&value, "--target-size")?);
            }
            "--max-size" => {
                let value = take_value(&mut iter, "--max-size")?;
                max_override = Some(parse_usize(&value, "--max-size")?);
            }
            "--break-mask" => {
                let value = take_value(&mut iter, "--break-mask")?;
                mask_override = Some(parse_mask(&value)?);
            }
            value if value.starts_with("--profile=") => {
                let raw = &value["--profile=".len()..];
                profile = resolve_profile(raw)?;
            }
            value if value.starts_with("--min-size=") => {
                let raw = &value["--min-size=".len()..];
                min_override = Some(parse_usize(raw, "--min-size")?);
            }
            value if value.starts_with("--target-size=") => {
                let raw = &value["--target-size=".len()..];
                target_override = Some(parse_usize(raw, "--target-size")?);
            }
            value if value.starts_with("--max-size=") => {
                let raw = &value["--max-size=".len()..];
                max_override = Some(parse_usize(raw, "--max-size")?);
            }
            value if value.starts_with("--break-mask=") => {
                let raw = &value["--break-mask=".len()..];
                mask_override = Some(parse_mask(raw)?);
            }
            value if value.starts_with('-') => {
                return Err(ParseError::Message(format!("unknown flag `{value}`")));
            }
            path => {
                if input.is_some() {
                    return Err(ParseError::Message(
                        "multiple input paths provided".to_owned(),
                    ));
                }
                input = Some(path.to_owned());
            }
        }
    }

    if let Some(min) = min_override {
        profile.min_size = min;
    }
    if let Some(target) = target_override {
        profile.target_size = target;
    }
    if let Some(max) = max_override {
        profile.max_size = max;
    }
    if let Some(mask) = mask_override {
        profile.break_mask = mask;
    }

    if profile.min_size == 0 {
        return Err(ParseError::Message(
            "--min-size must be greater than zero".to_owned(),
        ));
    }
    if profile.min_size > profile.target_size || profile.target_size > profile.max_size {
        return Err(ParseError::Message(
            "--min-size <= --target-size <= --max-size must hold".to_owned(),
        ));
    }
    if profile.break_mask == 0 {
        return Err(ParseError::Message(
            "--break-mask must be non-zero".to_owned(),
        ));
    }

    let input = input.ok_or_else(|| ParseError::Message("missing input path".to_owned()))?;
    Ok(CliConfig { profile, input })
}

fn take_value<I>(iter: &mut std::iter::Peekable<I>, flag: &str) -> Result<String, ParseError>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .ok_or_else(|| ParseError::Message(format!("{flag} requires a value")))
}

fn resolve_profile(value: &str) -> Result<ChunkProfile, ParseError> {
    match value {
        "sf1" | "sorafs.sf1@1.0.0" => Ok(ChunkProfile::DEFAULT),
        "sf2" | "sorafs-sf2" | "sorafs.sf2@1.0.0" => Ok(ChunkProfile::SF2),
        other => Err(ParseError::Message(format!("unknown profile `{other}`"))),
    }
}

fn parse_usize(value: &str, flag: &str) -> Result<usize, ParseError> {
    value
        .parse::<usize>()
        .map_err(|err| ParseError::Message(format!("{flag} expects an integer value: {err}")))
}

fn parse_mask(value: &str) -> Result<u64, ParseError> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|err| {
            ParseError::Message(format!("--break-mask invalid hex value `{trimmed}`: {err}"))
        })
    } else {
        trimmed.parse::<u64>().map_err(|err| {
            ParseError::Message(format!("--break-mask expects an integer value: {err}"))
        })
    }
}

fn read_input(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .map_err(|err| format!("failed to read stdin: {err}"))?;
        return Ok(buf);
    }

    let path_ref = Path::new(path);
    let mut file = File::open(path_ref).map_err(|err| format!("failed to open {path}: {err}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|err| format!("failed to read {path}: {err}"))?;
    Ok(buf)
}

fn generate_output(profile: ChunkProfile, input: &[u8]) -> String {
    let chunks = chunk_bytes_with_digests_profile(profile, input);
    build_payload(profile, input, &chunks)
}

fn build_payload(profile: ChunkProfile, input: &[u8], chunks: &[ChunkDigest]) -> String {
    let mut out = String::new();
    writeln!(
        &mut out,
        "{{\n  \"profile\": {{\"min_size\": {}, \"target_size\": {}, \"max_size\": {}, \"break_mask\": \"0x{:04x}\"}},",
        profile.min_size, profile.target_size, profile.max_size, profile.break_mask
    )
    .unwrap();
    writeln!(&mut out, "  \"input_length\": {},", input.len()).unwrap();
    writeln!(&mut out, "  \"chunk_count\": {},", chunks.len()).unwrap();
    out.push_str("  \"chunks\": [\n");
    for (idx, chunk) in chunks.iter().enumerate() {
        writeln!(
            &mut out,
            "    {{\"offset\": {}, \"length\": {}, \"digest_blake3\": \"{}\"}}{}",
            chunk.offset,
            chunk.length,
            hex(&chunk.digest),
            if idx + 1 == chunks.len() { "" } else { "," }
        )
        .unwrap();
    }
    out.push_str("  ]\n}");
    out
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliConfig, ParseError> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_cli(owned)
    }

    #[test]
    fn parse_cli_defaults() {
        let config = parse(&["input.bin"]).expect("parse default");
        assert_eq!(config.input, "input.bin");
        assert_eq!(config.profile.min_size, ChunkProfile::DEFAULT.min_size);
        assert_eq!(
            config.profile.target_size,
            ChunkProfile::DEFAULT.target_size
        );
        assert_eq!(config.profile.max_size, ChunkProfile::DEFAULT.max_size);
        assert_eq!(config.profile.break_mask, ChunkProfile::DEFAULT.break_mask);
    }

    #[test]
    fn parse_cli_overrides_profile_and_params() {
        let config = parse(&[
            "--profile",
            "sorafs.sf1@1.0.0",
            "--min-size",
            "256",
            "--target-size",
            "512",
            "--max-size",
            "1024",
            "--break-mask",
            "0xff",
            "payload.bin",
        ])
        .expect("parse overrides");
        assert_eq!(config.input, "payload.bin");
        assert_eq!(config.profile.min_size, 256);
        assert_eq!(config.profile.target_size, 512);
        assert_eq!(config.profile.max_size, 1024);
        assert_eq!(config.profile.break_mask, 0xff);
    }

    #[test]
    fn generate_output_respects_profile() {
        let profile = ChunkProfile {
            min_size: 2,
            target_size: 2,
            max_size: 3,
            break_mask: 0x0001,
        };
        let data = b"abcdef";
        let output = generate_output(profile, data);
        let value: norito::json::Value =
            norito::json::from_str(&output).expect("output should be valid JSON");
        assert_eq!(
            value
                .get("profile")
                .and_then(|p| p.get("min_size"))
                .and_then(norito::json::Value::as_u64)
                .expect("min_size present"),
            2
        );
        assert_eq!(
            value
                .get("chunk_count")
                .and_then(norito::json::Value::as_u64)
                .expect("chunk count present"),
            2,
            "custom max_size should split into two chunks"
        );
    }

    #[test]
    fn parse_cli_handles_help() {
        match parse(&["--help"]) {
            Err(ParseError::Help) => {}
            other => panic!("expected help, got {other:?}"),
        }
    }

    #[test]
    fn parse_cli_rejects_invalid_mask() {
        match parse(&["--break-mask", "0xZZ", "input.bin"]) {
            Err(ParseError::Message(msg)) => {
                assert!(
                    msg.contains("invalid hex"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }
}
