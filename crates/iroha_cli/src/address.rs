//! Account address tooling (canonical I105 and public-key input/output).
use super::*;
use clap::ValueEnum;
use iroha::account_address::{
    AccountAddress, AccountAddressError, AddressDomainKind, ParsedAccountAddress,
};
use iroha::data_model::account::AccountId;
use iroha_crypto::PublicKey;
use norito::json::{self, JsonSerialize};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};
/// Minamoto I105 prefix used by tests (runtime commands require an explicit context).
#[cfg(test)]
const DEFAULT_I105_PREFIX: u16 = 753;
// A canonical V1 controller payload is at most 1,024 bytes. Four KiB leaves
// ample room for I105/base-105 or public-key text, while the aggregate and
// row ceilings keep audit/normalization reports from retaining an arbitrary
// pipe or file before address validation begins.
const ADDRESS_INPUT_MAX_LINE_BYTES_V1: usize = 4 * 1024;
const ADDRESS_INPUT_MAX_ENTRIES_V1: usize = 16_384;
const ADDRESS_INPUT_MAX_BYTES_V1: usize = 16 * 1024 * 1024;
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Convert account addresses between supported textual encodings.
    Convert(Convert),
    /// Scan a list of addresses and emit conversion summaries.
    Audit(Audit),
    /// Rewrite newline-separated addresses into canonical encodings.
    Normalize(Normalize),
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Convert(cmd) => cmd.run(context),
            Self::Audit(cmd) => cmd.run(context),
            Self::Normalize(cmd) => cmd.run(context),
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct Convert {
    /// Address literal to parse (canonical I105 or public key).
    #[arg(value_name = "ADDRESS")]
    input: String,
    /// Require I105 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Public network profile to use for I105 parsing/rendering.
    #[arg(long)]
    profile: Option<String>,
    /// Network prefix to use when emitting i105 output.
    #[arg(long = "network-prefix", value_name = "PREFIX")]
    network_prefix: Option<u16>,
    /// Desired output format (defaults to I105).
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::I105)]
    format: OutputFormat,
}
impl Convert {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let network_context = resolve_address_network_context(
            self.profile.as_deref(),
            self.network_prefix,
            Some(context.config().account_chain_discriminant),
        )?;
        let expect_prefix = resolve_address_expect_prefix(&network_context, self.expect_prefix)?;
        let input = parse_address_input(self.input.as_str(), Some(expect_prefix))
            .wrap_err("failed to parse address literal")?;
        if self.format == OutputFormat::Json {
            let summary = AddressSummary::build(&input, network_context.chain_discriminant)
                .wrap_err("failed to build address summary")?;
            return context.print_data(&summary);
        }
        let output =
            encode_address_literal(&input, network_context.chain_discriminant, self.format)
                .wrap_err("failed to encode address output")?;
        context.println_data(output)
    }
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab_case")]
enum OutputFormat {
    I105,
    CanonicalHex,
    Json,
}
#[derive(clap::Args, Debug)]
pub struct Audit {
    /// Path to a file containing newline-separated addresses (defaults to STDIN).
    #[arg(long = "input", value_name = "PATH")]
    input: Option<PathBuf>,
    /// Require I105 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Public network profile to use for I105 parsing/rendering.
    #[arg(long)]
    profile: Option<String>,
    /// Network prefix to use when emitting i105 output.
    #[arg(long = "network-prefix", value_name = "PREFIX")]
    network_prefix: Option<u16>,
    /// Succeed even if parse errors were encountered (allow auditing large dumps).
    #[arg(long = "allow-errors")]
    allow_errors: bool,
    /// Output format (`json` for structured reports, `csv` for spreadsheet ingestion).
    #[arg(long = "format", value_enum, default_value_t = AuditOutputFormat::Json)]
    format: AuditOutputFormat,
}
impl Audit {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let inputs =
            read_address_inputs(self.input.as_ref()).wrap_err("failed to load address list")?;
        if inputs.is_empty() {
            eyre::bail!("no addresses provided");
        }
        let network_context = resolve_address_network_context(
            self.profile.as_deref(),
            self.network_prefix,
            Some(context.config().account_chain_discriminant),
        )?;
        let expect_prefix = resolve_address_expect_prefix(&network_context, self.expect_prefix)?;
        let mut stats = AddressAuditStats::default();
        let mut entries = Vec::with_capacity(inputs.len());
        for (index, raw) in inputs.into_iter().enumerate() {
            match parse_address_input(raw.as_str(), Some(expect_prefix)) {
                Ok(parsed) => {
                    let summary =
                        AddressSummary::build(&parsed, network_context.chain_discriminant)
                            .wrap_err_with(|| {
                                format!("failed to summarise address at index {index}")
                            })?;
                    stats.record_summary(&summary);
                    entries.push(AddressAuditEntry {
                        input: raw,
                        status: "parsed",
                        summary: Some(summary),
                        error: None,
                    });
                }
                Err(err) => {
                    stats.record_error();
                    let code = err
                        .downcast_ref::<AccountAddressError>()
                        .map_or("ERR_ADDRESS_PARSE", AccountAddressError::code_str);
                    let message = err.to_string();
                    entries.push(AddressAuditEntry {
                        input: raw,
                        status: "error",
                        summary: None,
                        error: Some(AddressAuditError { code, message }),
                    });
                }
            }
        }
        stats.finalize(entries.len());
        let report = AddressAuditReport { entries, stats };
        match self.format {
            AuditOutputFormat::Json => context.print_data(&report)?,
            AuditOutputFormat::Csv => Self::print_csv(&report, context)?,
        }
        if report.stats.errors > 0 && !self.allow_errors {
            eyre::bail!(
                "address audit encountered {} parse error(s); rerun with --allow-errors to suppress the failure",
                report.stats.errors
            );
        }
        Ok(())
    }
    fn print_csv<C: RunContext>(report: &AddressAuditReport, context: &mut C) -> Result<()> {
        const HEADER: &str =
            "input,status,format,domain_kind,i105,canonical_hex,error_code,error_message";
        context.println_data(HEADER.to_owned())?;
        for entry in &report.entries {
            let summary = entry.summary.as_ref();
            let error = entry.error.as_ref();
            let fields = [
                entry.input.as_str(),
                entry.status,
                summary.map_or("", |s| s.detected_format.kind),
                summary.map_or("", |s| s.domain.kind),
                summary.map_or("", |s| s.i105.value.as_str()),
                summary.map_or("", |s| s.canonical_hex.as_str()),
                error.map_or("", |err| err.code),
                error.map_or("", |err| err.message.as_str()),
            ];
            let row = fields
                .iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(",");
            context.println_data(row)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab_case")]
enum AuditOutputFormat {
    Json,
    Csv,
}
#[derive(clap::Args, Debug)]
pub struct Normalize {
    /// Path to a file containing newline-separated addresses (defaults to STDIN).
    #[arg(long = "input", value_name = "PATH")]
    input: Option<PathBuf>,
    /// Write the converted addresses to a file (defaults to STDOUT).
    #[arg(long = "output", value_name = "PATH")]
    output: Option<PathBuf>,
    /// Require I105 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Public network profile to use for I105 parsing/rendering.
    #[arg(long)]
    profile: Option<String>,
    /// Network prefix to use when emitting i105 output.
    #[arg(long = "network-prefix", value_name = "PREFIX")]
    network_prefix: Option<u16>,
    /// Desired output format (defaults to I105).
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::I105)]
    format: OutputFormat,
    /// Succeed even if parse errors were encountered (allow auditing large dumps).
    #[arg(long = "allow-errors")]
    allow_errors: bool,
}
impl Normalize {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let inputs =
            read_address_inputs(self.input.as_ref()).wrap_err("failed to load address list")?;
        if inputs.is_empty() {
            eyre::bail!("no addresses provided");
        }
        let network_context = resolve_address_network_context(
            self.profile.as_deref(),
            self.network_prefix,
            Some(context.config().account_chain_discriminant),
        )?;
        let expect_prefix = resolve_address_expect_prefix(&network_context, self.expect_prefix)?;
        let outputs = self.process_entries(
            &inputs,
            context.i18n(),
            network_context.chain_discriminant,
            expect_prefix,
        )?;
        if outputs.is_empty() {
            return Ok(());
        }
        if let Some(path) = self.output.as_ref() {
            if path == Path::new("-") {
                for line in outputs {
                    context.println_data(line)?;
                }
            } else {
                write_lines_to_file(path, &outputs)?;
            }
        } else {
            for line in outputs {
                context.println_data(line)?;
            }
        }
        Ok(())
    }
    fn process_entries(
        &self,
        inputs: &[String],
        i18n: &Localizer,
        network_prefix: u16,
        expect_prefix: u16,
    ) -> Result<Vec<String>> {
        let mut outputs = Vec::new();
        for (index, raw) in inputs.iter().enumerate() {
            let parsed = match parse_address_input(raw.as_str(), Some(expect_prefix)) {
                Ok(parsed) => parsed,
                Err(err) => {
                    if self.allow_errors {
                        eprintln!("{}", normalize_skipped_address_message(index, &err, i18n));
                        continue;
                    }
                    return Err(err)
                        .wrap_err_with(|| format!("failed to parse address at index {index}"));
                }
            };
            let rendered = self.render_output(&parsed, network_prefix)?;
            outputs.push(rendered);
        }
        Ok(outputs)
    }
    fn render_output(&self, parsed: &ParsedAddressInput, network_prefix: u16) -> Result<String> {
        if self.format == OutputFormat::Json {
            let summary = AddressSummary::build(parsed, network_prefix)
                .wrap_err("failed to build address summary")?;
            let summary_value = json::to_value(&summary)
                .map_err(|err| eyre::eyre!("failed to encode address summary: {err}"))?;
            return json::to_string(&summary_value)
                .map_err(|err| eyre::eyre!("failed to serialise address summary: {err}"));
        }
        encode_address_literal(parsed, network_prefix, self.format)
            .map_err(|err| eyre::eyre!(err.to_string()))
    }
}
#[derive(Debug)]
struct AddressNetworkContext {
    chain_discriminant: u16,
}
fn resolve_address_network_context(
    profile: Option<&str>,
    network_prefix: Option<u16>,
    default_network_prefix: Option<u16>,
) -> Result<AddressNetworkContext> {
    match (
        profile.map(str::trim).filter(|value| !value.is_empty()),
        network_prefix,
    ) {
        (Some(profile_name), Some(actual)) => {
            let expected = iroha_torii_shared::network_profile(profile_name).ok_or_else(|| {
                eyre::eyre!(
                    "unknown network profile `{profile_name}` (supported: {})",
                    iroha_torii_shared::network_profile_names()
                )
            })?;
            if expected.chain_discriminant != actual {
                eyre::bail!(
                    "network profile mismatch: profile `{}` expected chain_discriminant={}, actual chain_discriminant={}",
                    expected.name,
                    expected.chain_discriminant,
                    actual
                );
            }
            Ok(AddressNetworkContext {
                chain_discriminant: actual,
            })
        }
        (Some(profile_name), None) => {
            let expected = iroha_torii_shared::network_profile(profile_name).ok_or_else(|| {
                eyre::eyre!(
                    "unknown network profile `{profile_name}` (supported: {})",
                    iroha_torii_shared::network_profile_names()
                )
            })?;
            Ok(AddressNetworkContext {
                chain_discriminant: expected.chain_discriminant,
            })
        }
        (None, Some(network_prefix)) => Ok(AddressNetworkContext {
            chain_discriminant: network_prefix,
        }),
        (None, None) => default_network_prefix.map_or_else(
            || Err(eyre::eyre!("provide --profile or --network-prefix")),
            |chain_discriminant| Ok(AddressNetworkContext { chain_discriminant }),
        ),
    }
}
fn resolve_address_expect_prefix(
    context: &AddressNetworkContext,
    expect_prefix: Option<u16>,
) -> Result<u16> {
    Ok(expect_prefix.unwrap_or(context.chain_discriminant))
}
#[derive(JsonSerialize)]
struct AddressSummary {
    detected_format: DetectedFormat,
    domain: DomainSummary,
    canonical_hex: String,
    i105: I105Encoding,
}
impl AddressSummary {
    fn build(
        parsed: &ParsedAddressInput,
        network_prefix: u16,
    ) -> Result<Self, AccountAddressError> {
        Ok(Self {
            detected_format: parsed.detected_format,
            domain: DomainSummary::from_kind(parsed.parsed.domain_kind()),
            canonical_hex: parsed.parsed.canonical_hex()?,
            i105: I105Encoding {
                value: parsed
                    .parsed
                    .address
                    .to_i105_for_discriminant(network_prefix)?,
                network_prefix,
            },
        })
    }
}
#[derive(Clone, Copy, Debug, JsonSerialize)]
struct DetectedFormat {
    kind: &'static str,
    network_prefix: Option<u16>,
}
impl DetectedFormat {
    const fn i105() -> Self {
        Self {
            kind: "i105",
            network_prefix: None,
        }
    }
    const fn public_key() -> Self {
        Self {
            kind: "public_key",
            network_prefix: None,
        }
    }
}
#[derive(JsonSerialize)]
struct I105Encoding {
    value: String,
    network_prefix: u16,
}
#[derive(JsonSerialize)]
struct DomainSummary {
    kind: &'static str,
}
impl DomainSummary {
    const fn from_kind(kind: AddressDomainKind) -> Self {
        Self {
            kind: kind.as_str(),
        }
    }
}
fn normalize_skipped_address_message(index: usize, err: &eyre::Report, i18n: &Localizer) -> String {
    let index_text = index.to_string();
    let error_text = err.to_string();
    i18n.t_with(
        "warning.normalize_skipped_address",
        &[
            ("index", index_text.as_str()),
            ("error", error_text.as_str()),
        ],
    )
}
#[derive(Debug)]
struct ParsedAddressInput {
    parsed: ParsedAccountAddress,
    detected_format: DetectedFormat,
}
fn parse_address_input(
    input: &str,
    expect_prefix: Option<u16>,
) -> Result<ParsedAddressInput, eyre::Report> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        eyre::bail!("address literal is empty");
    }
    if trimmed.contains('@') {
        eyre::bail!("address literal must not include '@domain'");
    }
    if trimmed
        .get(..2)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x"))
    {
        eyre::bail!("address literal must be canonical I105; canonical hex is not accepted");
    }
    if let Ok(public_key) = trimmed.parse::<PublicKey>() {
        let account = AccountId::new(public_key);
        let address = AccountAddress::from_account_id(&account)
            .wrap_err("failed to encode account address from public key")?;
        let parsed = ParsedAccountAddress {
            domain_kind: address.domain_kind(),
            address,
        };
        return Ok(ParsedAddressInput {
            parsed,
            detected_format: DetectedFormat::public_key(),
        });
    }
    let address = AccountAddress::parse_encoded(trimmed, expect_prefix)
        .wrap_err("failed to parse account address")?;
    let parsed = ParsedAccountAddress {
        domain_kind: address.domain_kind(),
        address,
    };
    Ok(ParsedAddressInput {
        parsed,
        detected_format: DetectedFormat::i105(),
    })
}
#[derive(JsonSerialize)]
struct AddressAuditReport {
    entries: Vec<AddressAuditEntry>,
    stats: AddressAuditStats,
}
#[derive(JsonSerialize)]
struct AddressAuditEntry {
    input: String,
    status: &'static str,
    summary: Option<AddressSummary>,
    error: Option<AddressAuditError>,
}
#[derive(JsonSerialize)]
struct AddressAuditError {
    code: &'static str,
    message: String,
}
#[derive(Default, JsonSerialize)]
struct AddressAuditStats {
    total: usize,
    parsed: usize,
    errors: usize,
}
impl AddressAuditStats {
    fn record_summary(&mut self, _summary: &AddressSummary) {
        self.parsed += 1;
    }
    fn record_error(&mut self) {
        self.errors += 1;
    }
    fn finalize(&mut self, total_entries: usize) {
        self.total = total_entries;
    }
}
fn read_address_inputs(source: Option<&PathBuf>) -> Result<Vec<String>> {
    let bytes = match source {
        Some(path) if path == Path::new("-") => read_cli_input_bounded(
            &mut io::stdin().lock(),
            ADDRESS_INPUT_MAX_BYTES_V1,
            "address input",
        )
        .wrap_err("failed to read addresses from stdin")?,
        Some(path) => {
            let mut file = File::open(path)
                .wrap_err_with(|| format!("failed to open addresses from {}", path.display()))?;
            let metadata = file
                .metadata()
                .wrap_err_with(|| format!("failed to inspect address input {}", path.display()))?;
            if !metadata.is_file() {
                eyre::bail!("address input must be a regular file: {}", path.display());
            }
            if metadata.len() > ADDRESS_INPUT_MAX_BYTES_V1 as u64 {
                eyre::bail!(
                    "address input {} exceeds the first-release limit of {} bytes",
                    path.display(),
                    ADDRESS_INPUT_MAX_BYTES_V1
                );
            }
            let bytes =
                read_cli_input_bounded(&mut file, ADDRESS_INPUT_MAX_BYTES_V1, "address input")
                    .wrap_err_with(|| {
                        format!("failed to read addresses from {}", path.display())
                    })?;
            let after = file.metadata().wrap_err_with(|| {
                format!("failed to reinspect address input {}", path.display())
            })?;
            if after.len() != metadata.len() || after.len() != bytes.len() as u64 {
                eyre::bail!("address input changed while reading: {}", path.display());
            }
            bytes
        }
        None => read_cli_input_bounded(
            &mut io::stdin().lock(),
            ADDRESS_INPUT_MAX_BYTES_V1,
            "address input",
        )
        .wrap_err("failed to read addresses from stdin")?,
    };
    parse_address_input_lines(bytes, "address input")
}
fn parse_address_input_lines(bytes: Vec<u8>, label: &str) -> Result<Vec<String>> {
    let buffer = String::from_utf8(bytes)
        .map_err(|error| eyre::eyre!("{label} is not valid UTF-8: {error}"))?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(ADDRESS_INPUT_MAX_ENTRIES_V1.min(buffer.lines().count()))
        .map_err(|error| eyre::eyre!("failed to reserve address entry storage: {error}"))?;
    for (line_index, line) in buffer.lines().enumerate() {
        if line.len() > ADDRESS_INPUT_MAX_LINE_BYTES_V1 {
            eyre::bail!(
                "{label} line {} exceeds the first-release limit of {} bytes",
                line_index + 1,
                ADDRESS_INPUT_MAX_LINE_BYTES_V1
            );
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if entries.len() == ADDRESS_INPUT_MAX_ENTRIES_V1 {
            eyre::bail!(
                "{label} exceeds the first-release limit of {} address entries",
                ADDRESS_INPUT_MAX_ENTRIES_V1
            );
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(trimmed.len())
            .map_err(|error| eyre::eyre!("failed to reserve address line storage: {error}"))?;
        owned.push_str(trimmed);
        entries.push(owned);
    }
    Ok(entries)
}
fn encode_address_literal(
    parsed: &ParsedAddressInput,
    network_prefix: u16,
    format: OutputFormat,
) -> Result<String, AccountAddressError> {
    match format {
        OutputFormat::I105 => parsed
            .parsed
            .address
            .to_i105_for_discriminant(network_prefix),
        OutputFormat::CanonicalHex => parsed.parsed.canonical_hex(),
        OutputFormat::Json => unreachable!("JSON encoding handled separately"),
    }
}
fn write_lines_to_file(path: &Path, lines: &[String]) -> Result<()> {
    let file =
        File::create(path).wrap_err_with(|| format!("failed to create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            writer
                .write_all(b"\n")
                .wrap_err("failed to write newline")?;
        }
        writer
            .write_all(line.as_bytes())
            .wrap_err("failed to write line")?;
    }
    writer
        .write_all(b"\n")
        .wrap_err("failed to terminate newline")?;
    writer.flush().wrap_err("failed to flush writer")
}
fn csv_escape(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let needs_quotes = value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'));
    if !needs_quotes {
        return value.to_string();
    }
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        if ch == '"' {
            escaped.push('"');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    escaped
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha::account_address::DEFAULT_DOMAIN_NAME;
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use iroha_data_model::{account::AccountId, domain::DomainId};
    use iroha_i18n::{Bundle, Language, Localizer};
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    fn account_id_for_domain(label: &str, seed: u8) -> AccountId {
        let _ = DomainId::try_new(label, "universal").expect("domain label canonicalises");
        let key_pair = fixture_key_pair(seed);
        AccountId::new(key_pair.public_key().clone())
    }
    fn test_i18n() -> Localizer {
        Localizer::new(Bundle::Cli, Language::English)
    }
    #[test]
    fn address_summary_reports_default_domain_kind() {
        let account = account_id_for_domain("treasury", 1);
        let address = AccountAddress::from_account_id(&account).expect("address encoding");
        let parsed = ParsedAddressInput {
            parsed: ParsedAccountAddress {
                domain_kind: address.domain_kind(),
                address,
            },
            detected_format: DetectedFormat::i105(),
        };
        let summary = AddressSummary::build(&parsed, DEFAULT_I105_PREFIX).expect("summary");
        assert_eq!(summary.domain.kind, "default");
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(0x11).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn address_summary_suppresses_warning_for_default_domain() {
        let account = account_id_for_domain(DEFAULT_DOMAIN_NAME, 2);
        let address = AccountAddress::from_account_id(&account).expect("address encoding");
        let parsed = ParsedAddressInput {
            parsed: ParsedAccountAddress {
                domain_kind: address.domain_kind(),
                address,
            },
            detected_format: DetectedFormat::i105(),
        };
        let summary = AddressSummary::build(&parsed, DEFAULT_I105_PREFIX).expect("summary");
        assert_eq!(summary.domain.kind, "default");
    }
    #[test]
    fn normalize_serialises_json_summary() {
        let account = account_id_for_domain("treasury", 5);
        let literal = AccountAddress::from_account_id(&account)
            .expect("address encoding")
            .to_i105_for_discriminant(DEFAULT_I105_PREFIX)
            .expect("i105");
        let cmd = Normalize {
            input: None,
            output: None,
            expect_prefix: Some(DEFAULT_I105_PREFIX),
            profile: None,
            network_prefix: Some(DEFAULT_I105_PREFIX),
            format: OutputFormat::Json,
            allow_errors: false,
        };
        let i18n = test_i18n();
        let outputs = cmd
            .process_entries(&[literal], &i18n, DEFAULT_I105_PREFIX, DEFAULT_I105_PREFIX)
            .expect("normalize succeeds");
        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].starts_with('{') && outputs[0].contains("\"canonical_hex\""),
            "json summary should include canonical_hex field"
        );
    }
    #[test]
    fn normalize_warning_messages_include_context() {
        let i18n = test_i18n();
        let err = eyre::eyre!("bad input");
        let message = normalize_skipped_address_message(2, &err, &i18n);
        assert!(message.contains("index 2"), "unexpected message: {message}");
        assert!(
            message.contains("bad input"),
            "unexpected message: {message}"
        );
    }
    #[test]
    fn address_network_context_requires_profile_or_prefix() {
        let err = resolve_address_network_context(None, None, None)
            .expect_err("missing network context should fail");
        assert!(
            err.to_string()
                .contains("provide --profile or --network-prefix")
        );
    }
    #[test]
    fn address_network_context_uses_config_default_prefix() {
        let context = resolve_address_network_context(None, None, Some(DEFAULT_I105_PREFIX))
            .expect("default prefix should resolve");
        assert_eq!(context.chain_discriminant, DEFAULT_I105_PREFIX);
    }
    #[test]
    fn address_network_context_resolves_profile_and_rejects_mismatch() {
        let context =
            resolve_address_network_context(Some("taira"), None, None).expect("profile resolves");
        assert_eq!(
            context.chain_discriminant,
            iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
        );
        let err = resolve_address_network_context(Some("taira"), Some(DEFAULT_I105_PREFIX), None)
            .expect_err("profile mismatch should fail");
        assert!(
            err.to_string()
                .contains("profile `taira` expected chain_discriminant=369")
        );
    }
    #[test]
    fn explicit_expect_prefix_allows_reencoding_between_networks() {
        let account = account_id_for_domain("treasury", 7);
        let input_prefix = DEFAULT_I105_PREFIX;
        let output_prefix = iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT;
        let input = AccountAddress::from_account_id(&account)
            .expect("address encoding")
            .to_i105_for_discriminant(input_prefix)
            .expect("input i105");
        let context = resolve_address_network_context(None, Some(output_prefix), None)
            .expect("output prefix resolves");
        let expect_prefix =
            resolve_address_expect_prefix(&context, Some(input_prefix)).expect("expect prefix");
        let parsed = parse_address_input(&input, Some(expect_prefix)).expect("input parses");
        let rendered = encode_address_literal(&parsed, output_prefix, OutputFormat::I105)
            .expect("output i105");
        assert_ne!(rendered, input);
        assert!(
            AccountAddress::parse_encoded(&rendered, Some(output_prefix)).is_ok(),
            "rendered address should parse under output prefix"
        );
    }
    #[test]
    fn parse_address_input_rejects_canonical_hex() {
        let account = account_id_for_domain("treasury", 8);
        let canonical = AccountAddress::from_account_id(&account)
            .expect("address encoding")
            .canonical_hex()
            .expect("canonical hex");
        let err = parse_address_input(&canonical, Some(DEFAULT_I105_PREFIX))
            .expect_err("canonical hex must be rejected");
        assert!(
            err.to_string().contains("canonical hex is not accepted"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn parse_address_input_accepts_public_key_and_marks_detected_format() {
        let key_pair = fixture_key_pair(9);
        let public_key = key_pair.public_key().to_string();
        let parsed =
            parse_address_input(&public_key, Some(DEFAULT_I105_PREFIX)).expect("public key parses");
        assert_eq!(parsed.detected_format.kind, "public_key");
        let expected = AccountAddress::from_account_id(&AccountId::new(
            public_key
                .parse::<PublicKey>()
                .expect("public key literal should parse"),
        ))
        .expect("address encoding");
        assert_eq!(parsed.parsed.address, expected);
    }
    #[test]
    fn convert_public_key_input_emits_canonical_i105() {
        let key_pair = fixture_key_pair(10);
        let public_key = key_pair.public_key().to_string();
        let parsed =
            parse_address_input(&public_key, Some(DEFAULT_I105_PREFIX)).expect("public key parses");
        let rendered = encode_address_literal(&parsed, DEFAULT_I105_PREFIX, OutputFormat::I105)
            .expect("i105 render");
        assert_eq!(
            rendered,
            AccountId::new(
                public_key
                    .parse::<PublicKey>()
                    .expect("public key literal should parse"),
            )
            .canonical_i105()
            .expect("canonical I105"),
        );
    }
    #[test]
    fn address_input_line_and_entry_bounds_are_exact() {
        let exact_line = "a".repeat(ADDRESS_INPUT_MAX_LINE_BYTES_V1);
        assert_eq!(
            parse_address_input_lines(exact_line.clone().into_bytes(), "fixture")
                .expect("exact line bound is admitted"),
            vec![exact_line]
        );
        let over_line = "a".repeat(ADDRESS_INPUT_MAX_LINE_BYTES_V1 + 1);
        let error = parse_address_input_lines(over_line.into_bytes(), "fixture")
            .expect_err("first over-limit line must fail");
        assert!(error.to_string().contains("line 1"));
        let exact_entries = "a\n".repeat(ADDRESS_INPUT_MAX_ENTRIES_V1);
        assert_eq!(
            parse_address_input_lines(exact_entries.into_bytes(), "fixture")
                .expect("exact entry bound is admitted")
                .len(),
            ADDRESS_INPUT_MAX_ENTRIES_V1
        );
        let over_entries = "a\n".repeat(ADDRESS_INPUT_MAX_ENTRIES_V1 + 1);
        let error = parse_address_input_lines(over_entries.into_bytes(), "fixture")
            .expect_err("first over-limit entry must fail");
        assert!(error.to_string().contains("address entries"));
    }
    #[test]
    fn address_file_size_is_rejected_before_reading() {
        let directory = tempfile::tempdir().expect("create address input directory");
        let path = directory.path().join("addresses.txt");
        let file = File::create(&path).expect("create sparse address input");
        file.set_len((ADDRESS_INPUT_MAX_BYTES_V1 + 1) as u64)
            .expect("extend sparse address input");
        let error = read_address_inputs(Some(&path))
            .expect_err("oversized address file must fail before allocation");
        assert!(error.to_string().contains("first-release limit"));
    }
}
