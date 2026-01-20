//! Account address tooling (IH58/compressed/canonical conversions).

use super::*;
use clap::ValueEnum;
use iroha::{
    account_address::{
        AccountAddressError, AccountAddressFormat, AddressDomainKind, ParsedAccountAddress,
        parse_account_address,
    },
    data_model::domain::DomainId,
};
use norito::json::{self, JsonSerialize};
use std::{
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

/// Default IH58 prefix for Sora Nexus (see `address_prefix_registry.json`).
const DEFAULT_IH58_PREFIX: u16 = 753;
const LOCAL_SELECTOR_WARNING: &str = "local-domain selector detected: register the domain \
with the Nexus registry and refresh IH58/compressed copies before Local selectors are blocked. \
Refer to docs/source/sns/address_display_guidelines.md for the cutover schedule.";

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
    /// Address literal to parse (IH58, `snx1…`, or canonical `0x…`).
    #[arg(value_name = "ADDRESS")]
    input: String,
    /// Require IH58 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Network prefix to use when emitting IH58 output.
    #[arg(
        long = "network-prefix",
        value_name = "PREFIX",
        default_value_t = DEFAULT_IH58_PREFIX
    )]
    network_prefix: u16,
    /// Desired output format (defaults to IH58).
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::Ih58)]
    format: OutputFormat,
    /// Append the provided domain to the output (requires `<address>@<domain>` input).
    #[arg(long = "append-domain")]
    append_domain: bool,
}

impl Convert {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let input = parse_address_input(self.input.as_str(), self.expect_prefix)
            .wrap_err("failed to parse address literal")?;
        if let Some(message) = local_selector_warning(input.parsed.domain_kind(), context.i18n()) {
            eprintln!("{message}");
        }

        if self.format == OutputFormat::Json {
            let summary = AddressSummary::build(&input, self.network_prefix)
                .wrap_err("failed to build address summary")?;
            return context.print_data(&summary);
        }

        let mut output = encode_address_literal(&input, self.network_prefix, self.format)
            .wrap_err("failed to encode address output")?;

        if self.append_domain {
            output = append_domain_literal(&output, &input)?;
        }

        context.println(output)
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab_case")]
enum OutputFormat {
    Ih58,
    Compressed,
    CanonicalHex,
    Json,
}

#[derive(clap::Args, Debug)]
pub struct Audit {
    /// Path to a file containing newline-separated addresses (defaults to STDIN).
    #[arg(long = "input", value_name = "PATH")]
    input: Option<PathBuf>,
    /// Require IH58 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Network prefix to use when emitting IH58 output.
    #[arg(
        long = "network-prefix",
        value_name = "PREFIX",
        default_value_t = DEFAULT_IH58_PREFIX
    )]
    network_prefix: u16,
    /// Return a non-zero status code when Local-domain selectors are detected.
    #[arg(long = "fail-on-warning")]
    fail_on_warning: bool,
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

        let mut stats = AddressAuditStats::default();
        let mut entries = Vec::with_capacity(inputs.len());
        for (index, raw) in inputs.into_iter().enumerate() {
            match parse_address_input(raw.as_str(), self.expect_prefix) {
                Ok(parsed) => {
                    if let Some(message) =
                        local_selector_warning(parsed.parsed.domain_kind(), context.i18n())
                    {
                        eprintln!("{message}");
                    }
                    let summary = AddressSummary::build(&parsed, self.network_prefix)
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
        if report.stats.warnings > 0 && self.fail_on_warning {
            eyre::bail!(
                "address audit detected {} Local-domain selector(s); convert them before enabling --fail-on-warning",
                report.stats.warnings
            );
        }

        Ok(())
    }

    fn print_csv<C: RunContext>(report: &AddressAuditReport, context: &mut C) -> Result<()> {
        const HEADER: &str = "input,status,format,domain_kind,domain_warning,ih58,canonical_hex,compressed,error_code,error_message";
        context.println(HEADER.to_owned())?;
        for entry in &report.entries {
            let summary = entry.summary.as_ref();
            let error = entry.error.as_ref();
            let fields = [
                entry.input.as_str(),
                entry.status,
                summary.map_or("", |s| s.detected_format.kind),
                summary.map_or("", |s| s.domain.kind),
                summary.and_then(|s| s.domain.warning).unwrap_or(""),
                summary.map_or("", |s| s.ih58.value.as_str()),
                summary.map_or("", |s| s.canonical_hex.as_str()),
                summary.map_or("", |s| s.compressed.as_str()),
                error.map_or("", |err| err.code),
                error.map_or("", |err| err.message.as_str()),
            ];
            let row = fields
                .iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(",");
            context.println(row)?;
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
    /// Require IH58 inputs to match the provided network prefix.
    #[arg(long = "expect-prefix", value_name = "PREFIX")]
    expect_prefix: Option<u16>,
    /// Network prefix to use when emitting IH58 output.
    #[arg(
        long = "network-prefix",
        value_name = "PREFIX",
        default_value_t = DEFAULT_IH58_PREFIX
    )]
    network_prefix: u16,
    /// Desired output format (defaults to IH58).
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::Ih58)]
    format: OutputFormat,
    /// Append the provided domain to the output (requires `<address>@<domain>` input).
    #[arg(long = "append-domain")]
    append_domain: bool,
    /// Only emit conversions for Local-domain selectors.
    #[arg(long = "only-local")]
    only_local: bool,
    /// Succeed even if parse errors were encountered (allow auditing large dumps).
    #[arg(long = "allow-errors")]
    allow_errors: bool,
}

impl Normalize {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.format == OutputFormat::Json && self.append_domain {
            eyre::bail!("--append-domain cannot be combined with --format json");
        }

        let inputs =
            read_address_inputs(self.input.as_ref()).wrap_err("failed to load address list")?;
        if inputs.is_empty() {
            eyre::bail!("no addresses provided");
        }

        let outputs = self.process_entries(&inputs, context.i18n())?;
        if outputs.is_empty() {
            if self.only_local {
                eprintln!("{}", normalize_no_local_selectors_message(context.i18n()));
            }
            return Ok(());
        }

        if let Some(path) = self.output.as_ref() {
            if path == Path::new("-") {
                for line in outputs {
                    context.println(line)?;
                }
            } else {
                write_lines_to_file(path, &outputs)?;
            }
        } else {
            for line in outputs {
                context.println(line)?;
            }
        }

        Ok(())
    }

    fn process_entries(&self, inputs: &[String], i18n: &Localizer) -> Result<Vec<String>> {
        let mut outputs = Vec::new();
        for (index, raw) in inputs.iter().enumerate() {
            let parsed = match parse_address_input(raw.as_str(), self.expect_prefix) {
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
            if self.only_local
                && !matches!(
                    parsed.parsed.domain_kind(),
                    AddressDomainKind::LocalDigest12
                )
            {
                continue;
            }
            if let Some(message) = local_selector_warning(parsed.parsed.domain_kind(), i18n) {
                eprintln!("{message}");
            }
            let rendered = self.render_output(&parsed)?;
            outputs.push(rendered);
        }
        Ok(outputs)
    }

    fn render_output(&self, parsed: &ParsedAddressInput) -> Result<String> {
        if self.format == OutputFormat::Json {
            if self.append_domain {
                eyre::bail!("--append-domain cannot be combined with --format json");
            }
            let summary = AddressSummary::build(parsed, self.network_prefix)
                .wrap_err("failed to build address summary")?;
            let summary_value = json::to_value(&summary)
                .map_err(|err| eyre::eyre!("failed to encode address summary: {err}"))?;
            return json::to_string(&summary_value)
                .map_err(|err| eyre::eyre!("failed to serialise address summary: {err}"));
        }
        let rendered = encode_address_literal(parsed, self.network_prefix, self.format)
            .map_err(|err| eyre::eyre!(err.to_string()))?;
        if self.append_domain {
            return append_domain_literal(&rendered, parsed);
        }
        Ok(rendered)
    }
}

#[derive(JsonSerialize)]
struct AddressSummary {
    detected_format: DetectedFormat,
    domain: DomainSummary,
    canonical_hex: String,
    ih58: Ih58Encoding,
    compressed: String,
    input_domain: Option<String>,
}

impl AddressSummary {
    fn build(
        parsed: &ParsedAddressInput,
        network_prefix: u16,
    ) -> Result<Self, AccountAddressError> {
        Ok(Self {
            detected_format: DetectedFormat::from(parsed.parsed.format),
            domain: DomainSummary::from_kind(parsed.parsed.domain_kind()),
            canonical_hex: parsed.parsed.canonical_hex()?,
            ih58: Ih58Encoding {
                value: parsed.parsed.address.to_ih58(network_prefix)?,
                network_prefix,
            },
            compressed: parsed.parsed.address.to_compressed_sora()?,
            input_domain: parsed.domain.as_ref().map(ToString::to_string),
        })
    }
}

#[derive(JsonSerialize)]
struct DetectedFormat {
    kind: &'static str,
    network_prefix: Option<u16>,
}

impl From<AccountAddressFormat> for DetectedFormat {
    fn from(format: AccountAddressFormat) -> Self {
        match format {
            AccountAddressFormat::IH58 { network_prefix } => Self {
                kind: "ih58",
                network_prefix: Some(network_prefix),
            },
            AccountAddressFormat::Compressed => Self {
                kind: "compressed",
                network_prefix: None,
            },
            AccountAddressFormat::CanonicalHex => Self {
                kind: "canonical-hex",
                network_prefix: None,
            },
        }
    }
}

#[derive(JsonSerialize)]
struct Ih58Encoding {
    value: String,
    network_prefix: u16,
}

#[derive(JsonSerialize)]
struct DomainSummary {
    kind: &'static str,
    warning: Option<&'static str>,
}

impl DomainSummary {
    const fn from_kind(kind: AddressDomainKind) -> Self {
        let warning = match kind {
            AddressDomainKind::LocalDigest12 => Some(LOCAL_SELECTOR_WARNING),
            AddressDomainKind::Default | AddressDomainKind::GlobalRegistry => None,
        };
        Self {
            kind: kind.as_str(),
            warning,
        }
    }
}

fn local_selector_warning(kind: AddressDomainKind, i18n: &Localizer) -> Option<String> {
    matches!(kind, AddressDomainKind::LocalDigest12).then(|| {
        i18n.t_with(
            "warning.local_selector_warning",
            &[("warning", LOCAL_SELECTOR_WARNING)],
        )
    })
}

fn normalize_no_local_selectors_message(i18n: &Localizer) -> String {
    i18n.t("warning.normalize_no_local_selectors")
}

fn normalize_skipped_address_message(
    index: usize,
    err: &eyre::Report,
    i18n: &Localizer,
) -> String {
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

struct ParsedAddressInput {
    parsed: ParsedAccountAddress,
    domain: Option<DomainId>,
}

fn parse_address_input(
    input: &str,
    expect_prefix: Option<u16>,
) -> Result<ParsedAddressInput, eyre::Report> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        eyre::bail!("address literal is empty");
    }
    if let Some((address_literal, domain_literal)) = trimmed.rsplit_once('@') {
        if address_literal.is_empty() {
            eyre::bail!("address literal must contain characters before '@'");
        }
        if domain_literal.is_empty() {
            eyre::bail!("domain literal missing after '@'");
        }
        let domain: DomainId = domain_literal
            .parse()
            .wrap_err("failed to parse domain literal")?;
        let parsed = parse_account_address(address_literal, expect_prefix)
            .wrap_err("failed to parse account component")?;
        parsed
            .address
            .to_account_id(&domain)
            .wrap_err("address domain selector does not match provided domain")?;
        Ok(ParsedAddressInput {
            parsed,
            domain: Some(domain),
        })
    } else {
        let parsed = parse_account_address(trimmed, expect_prefix)
            .wrap_err("failed to parse account address")?;
        Ok(ParsedAddressInput {
            parsed,
            domain: None,
        })
    }
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
    warnings: usize,
    errors: usize,
}

impl AddressAuditStats {
    fn record_summary(&mut self, summary: &AddressSummary) {
        self.parsed += 1;
        if summary.domain.warning.is_some() {
            self.warnings += 1;
        }
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
        OutputFormat::Ih58 => parsed.parsed.address.to_ih58(network_prefix),
        OutputFormat::Compressed => parsed.parsed.address.to_compressed_sora(),
        OutputFormat::CanonicalHex => parsed.parsed.canonical_hex(),
        OutputFormat::Json => unreachable!("JSON encoding handled separately"),
    }
}

fn append_domain_literal(value: &str, parsed: &ParsedAddressInput) -> Result<String> {
    let domain = parsed.domain.as_ref().ok_or_else(|| {
        eyre::eyre!("--append-domain requires an input of the form <address>@<domain>")
    })?;
    Ok(format!("{value}@{domain}"))
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
    use iroha::account_address::{AccountAddress, default_domain_name};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, domain::DomainId, name::Name};
    use iroha_i18n::{Bundle, Language, Localizer};
    use std::str::FromStr;

    fn account_id_for_domain(label: &str, seed: u8) -> AccountId {
        let domain = DomainId::new(Name::from_str(label).expect("domain label canonicalises"));
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(domain, key_pair.public_key().clone())
    }

    fn test_i18n() -> Localizer {
        Localizer::new(Bundle::Cli, Language::English)
    }

    #[test]
    fn address_summary_marks_local_domain_kind() {
        let account = account_id_for_domain("treasury", 1);
        let address = AccountAddress::from_account_id(&account).expect("address encoding");
        let parsed = ParsedAddressInput {
            parsed: ParsedAccountAddress {
                domain_kind: address.domain_kind(),
                address,
                format: AccountAddressFormat::CanonicalHex,
            },
            domain: None,
        };
        let summary = AddressSummary::build(&parsed, DEFAULT_IH58_PREFIX).expect("summary");
        assert_eq!(summary.domain.kind, "local12");
        assert_eq!(summary.domain.warning, Some(LOCAL_SELECTOR_WARNING));
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
                format: AccountAddressFormat::CanonicalHex,
            },
            domain: None,
        };
        let summary = AddressSummary::build(&parsed, DEFAULT_IH58_PREFIX).expect("summary");
        assert_eq!(summary.domain.kind, "default");
        assert!(summary.domain.warning.is_none());
    }

    #[test]
    fn normalize_filters_local_entries() {
        let local_account = account_id_for_domain("treasury", 3);
        let default_label = default_domain_name();
        let default_account = account_id_for_domain(default_label.as_ref(), 4);

        let local_literal = format!(
            "{}@{}",
            AccountAddress::from_account_id(&local_account)
                .expect("address encoding")
                .to_ih58(DEFAULT_IH58_PREFIX)
                .expect("ih58"),
            local_account.domain()
        );
        let default_literal = format!(
            "{}@{}",
            AccountAddress::from_account_id(&default_account)
                .expect("address encoding")
                .to_ih58(DEFAULT_IH58_PREFIX)
                .expect("ih58"),
            default_account.domain()
        );

        let cmd = Normalize {
            input: None,
            output: None,
            expect_prefix: Some(DEFAULT_IH58_PREFIX),
            network_prefix: DEFAULT_IH58_PREFIX,
            format: OutputFormat::Ih58,
            append_domain: true,
            only_local: true,
            allow_errors: false,
        };

        let i18n = test_i18n();
        let outputs = cmd
            .process_entries(&[local_literal, default_literal], &i18n)
            .expect("normalize succeeds");

        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].ends_with(&format!("@{}", local_account.domain())),
            "expected appended domain"
        );
    }

    #[test]
    fn normalize_serialises_json_summary() {
        let account = account_id_for_domain("treasury", 5);
        let literal = format!(
            "{}@{}",
            AccountAddress::from_account_id(&account)
                .expect("address encoding")
                .to_ih58(DEFAULT_IH58_PREFIX)
                .expect("ih58"),
            account.domain()
        );

        let cmd = Normalize {
            input: None,
            output: None,
            expect_prefix: Some(DEFAULT_IH58_PREFIX),
            network_prefix: DEFAULT_IH58_PREFIX,
            format: OutputFormat::Json,
            append_domain: false,
            only_local: false,
            allow_errors: false,
        };

        let i18n = test_i18n();
        let outputs = cmd
            .process_entries(&[literal], &i18n)
            .expect("normalize succeeds");
        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].starts_with('{') && outputs[0].contains("\"canonical_hex\""),
            "json summary should include canonical_hex field"
        );
    }

    #[test]
    fn local_selector_warning_reports_message() {
        let i18n = test_i18n();
        let message =
            local_selector_warning(AddressDomainKind::LocalDigest12, &i18n).expect("message");
        assert!(
            message.contains("local-domain selector detected"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn normalize_warning_messages_include_context() {
        let i18n = test_i18n();
        let message = normalize_no_local_selectors_message(&i18n);
        assert!(
            message.contains("normalize: no Local-domain selectors"),
            "unexpected message: {message}"
        );

        let err = eyre::eyre!("bad input");
        let message = normalize_skipped_address_message(2, &err, &i18n);
        assert!(message.contains("index 2"), "unexpected message: {message}");
        assert!(message.contains("bad input"), "unexpected message: {message}");
    }
}
