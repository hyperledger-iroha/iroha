//! IVM/ABI helper subcommands for the CLI.
use crate::{Run, RunContext, cli_output::print_with_optional_text};
use eyre::{Result, eyre};
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Print the current ABI hash for a given policy (default: v1)
    AbiHash(AbiHashArgs),
    /// Print the canonical syscall list (min or markdown table)
    Syscalls(SyscallsArgs),
    /// Generate a minimal manifest (`code_hash` + `abi_hash`) from a compiled .to file
    ManifestGen(ManifestGenArgs),
}
#[derive(clap::Args, Debug)]
pub struct AbiHashArgs {
    /// Policy: v1
    #[arg(long, value_name = "POLICY", default_value = "v1")]
    policy: String,
    /// Uppercase hex output (default: lowercase)
    #[arg(long)]
    uppercase: bool,
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::AbiHash(args) => args.run(context),
            Command::Syscalls(args) => args.run(context),
            Command::ManifestGen(args) => args.run(context),
        }
    }
}
impl Run for AbiHashArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        // Parse policy
        let pol = parse_policy(&self.policy)?;
        let hash = ivm::syscalls::compute_abi_hash(pol);
        let s = if self.uppercase {
            hex_upper(&hash)
        } else {
            hex_lower(&hash)
        };
        let mut value = norito::json::Map::new();
        value.insert(
            "policy".into(),
            norito::json::Value::String(self.policy.to_ascii_lowercase()),
        );
        value.insert("abi_hash".into(), norito::json::Value::String(s.clone()));
        print_with_optional_text(context, Some(s), &norito::json::Value::Object(value))
    }
}
fn parse_policy(s: &str) -> Result<ivm::SyscallPolicy> {
    match s.to_ascii_lowercase().as_str() {
        "v1" => Ok(ivm::SyscallPolicy::AbiV1),
        other => Err(eyre!(
            "unsupported ABI policy `{other}`; expected `v1` for the first release"
        )),
    }
}
fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use core::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}
fn hex_upper(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use core::fmt::Write as _;
        let _ = write!(&mut s, "{b:02X}");
    }
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_parsing_variants() {
        assert!(matches!(parse_policy("v1"), Ok(ivm::SyscallPolicy::AbiV1)));
        assert!(parse_policy("exp:2").is_err());
        assert!(parse_policy("unknown").is_err());
    }
    #[test]
    fn abi_hash_has_64_hex_chars_and_is_stable() {
        let h1 = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let h2 = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        assert_eq!(h1, h2);
        let lower = hex_lower(&h1);
        let upper = hex_upper(&h1);
        assert_eq!(lower.len(), 64);
        assert_eq!(upper.len(), 64);
        assert!(
            lower
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
        assert!(
            upper
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_lowercase())
        );
    }
}
#[derive(clap::Args, Debug)]
pub struct SyscallsArgs {
    /// Rendering of the syscall list: one entry per line, or a Markdown table
    #[arg(long, value_name = "FORMAT", value_enum, default_value_t = SyscallListFormat::Min)]
    format: SyscallListFormat,
}
/// Rendering of `iroha tools ivm syscalls`.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum SyscallListFormat {
    /// One `- <number> <NAME>` entry per line.
    Min,
    /// A Markdown table.
    Markdown,
}
impl Run for SyscallsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let out = match self.format {
            SyscallListFormat::Markdown => ivm::syscalls::render_syscalls_markdown_table(),
            SyscallListFormat::Min => ivm::syscalls::render_syscalls_min_list(),
        };
        context.println_data(out.trim_end())
    }
}
#[derive(clap::Args, Debug)]
pub struct ManifestGenArgs {
    /// Path to compiled IVM bytecode (.to)
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
}
impl Run for ManifestGenArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = std::fs::read(&self.file)?;
        let parsed = ivm::ProgramMetadata::parse(&bytes)
            .map_err(|_| eyre::eyre!("failed to parse IVM header"))?;
        let meta = parsed.metadata;
        if meta.abi_version != 1 {
            return Err(eyre::eyre!(
                "unsupported abi_version {}; first release requires 1",
                meta.abi_version
            ));
        }
        let code_hash = ivm::contract_code_hash(&bytes);
        let abi_hash: [u8; 32] = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        // Build a small JSON object for convenience
        let mut manifest = norito::json::Map::new();
        manifest.insert(
            "code_hash".into(),
            norito::json::Value::String(hex_lower(code_hash.as_ref())),
        );
        manifest.insert(
            "abi_hash".into(),
            norito::json::Value::String(hex_lower(&abi_hash)),
        );
        let mut top = norito::json::Map::new();
        top.insert("manifest".into(), norito::json::Value::Object(manifest));
        context.print_data(&norito::json::Value::Object(top))
    }
}
