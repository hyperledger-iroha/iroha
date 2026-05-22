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
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

/// Minamoto I105 prefix used by tests (runtime commands require an explicit context).
#[cfg(test)]
const DEFAULT_I105_PREFIX: u16 = 753;

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
    profile: Option<String>,
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
                profile: Some(expected.name.to_owned()),
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
                profile: Some(expected.name.to_owned()),
                chain_discriminant: expected.chain_discriminant,
            })
        }
        (None, Some(network_prefix)) => Ok(AddressNetworkContext {
            profile: None,
            chain_discriminant: network_prefix,
        }),
        (None, None) => default_network_prefix.map_or_else(
            || Err(eyre::eyre!("provide --profile or --network-prefix")),
            |chain_discriminant| {
                Ok(AddressNetworkContext {
                    profile: None,
                    chain_discriminant,
                })
            },
        ),
    }
}

fn resolve_address_expect_prefix(
    context: &AddressNetworkContext,
    expect_prefix: Option<u16>,
) -> Result<u16> {
    if let Some(actual) = expect_prefix
        && actual != context.chain_discriminant
    {
        if let Some(profile) = context.profile.as_deref() {
            eyre::bail!(
                "network profile mismatch: profile `{profile}` expected chain_discriminant={}, actual expect_prefix={actual}",
                context.chain_discriminant
            );
        }
        eyre::bail!(
            "network prefix mismatch: network_prefix={} actual expect_prefix={actual}",
            context.chain_discriminant
        );
    }
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
    let mut buffer = String::new();
    match source {
        Some(path) if path == Path::new("-") => {
            io::stdin()
                .read_to_string(&mut buffer)
                .wrap_err("failed to read addresses from stdin")?;
        }
        Some(path) => {
            buffer = fs::read_to_string(path)
                .wrap_err_with(|| format!("failed to read addresses from {}", path.display()))?;
        }
        None => {
            io::stdin()
                .read_to_string(&mut buffer)
                .wrap_err("failed to read addresses from stdin")?;
        }
    }

    let entries = buffer
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
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
    use iroha::account_address::default_domain_name;
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use iroha_data_model::{account::AccountId, domain::DomainId};
    use iroha_i18n::{Bundle, Language, Localizer};

    fn account_id_for_domain(label: &str, seed: u8) -> AccountId {
        let _ = DomainId::try_new(label, "universal").expect("domain label canonicalises");
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
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
    fn address_summary_suppresses_warning_for_default_domain() {
        let default_label = default_domain_name();
        let account = account_id_for_domain(default_label.as_ref(), 2);
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
        let key_pair = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
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
        let key_pair = KeyPair::from_seed(vec![10; 32], Algorithm::Ed25519);
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
}
