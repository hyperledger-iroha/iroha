//! IVM/ABI helper subcommands for the CLI.

use eyre::Result;

use crate::{Run, RunContext};

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
    /// Policy: v1, or exp:<n>
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
        let pol = parse_policy(&self.policy);
        let hash = ivm::syscalls::compute_abi_hash(pol);
        let s = if self.uppercase {
            hex_upper(&hash)
        } else {
            hex_lower(&hash)
        };
        context.println(s)?;
        Ok(())
    }
}

fn parse_policy(s: &str) -> ivm::SyscallPolicy {
    match s.to_ascii_lowercase().as_str() {
        "v1" => ivm::SyscallPolicy::AbiV1,
        other if other.starts_with("exp:") => {
            let n = other[4..].parse::<u8>().unwrap_or(1);
            ivm::SyscallPolicy::Experimental(n)
        }
        _ => ivm::SyscallPolicy::AbiV1,
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
        match parse_policy("v1") {
            ivm::SyscallPolicy::AbiV1 => {}
            other => panic!("expected V1, got {other:?}"),
        }
        match parse_policy("exp:2") {
            ivm::SyscallPolicy::Experimental(2) => {}
            other => panic!("expected Experimental(2), got {other:?}"),
        }
        // Fallback
        match parse_policy("unknown") {
            ivm::SyscallPolicy::AbiV1 => {}
            other => panic!("expected fallback to V1, got {other:?}"),
        }
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
    /// Output format: 'min' (one per line) or 'markdown'
    #[arg(long, value_name = "FORMAT", default_value = "min")]
    format: String,
}

impl Run for SyscallsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let out = match self.format.as_str() {
            "markdown" => ivm::syscalls::render_syscalls_markdown_table(),
            _ => ivm::syscalls::render_syscalls_min_list(),
        };
        context.println(out)?;
        Ok(())
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
        let code_hash: iroha_crypto::Hash = iroha_crypto::Hash::new(&bytes[parsed.header_len..]);
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
        let s = norito::json::to_json_pretty(&norito::json::Value::Object(top))?;
        context.println(s)?;
        Ok(())
    }
}
