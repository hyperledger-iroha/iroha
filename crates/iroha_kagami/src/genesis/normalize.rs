use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use color_eyre::eyre::eyre;
use iroha_genesis::{NormalizedGenesis, RawGenesisTransaction};

use crate::{Outcome, RunArgs, tui};

/// Show the fully expanded genesis block (after injections and ordering).
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Path to genesis json file
    genesis_file: PathBuf,
    /// Output format (`json` for structured output, `text` for a compact summary)
    #[clap(long, value_enum, default_value = "json")]
    format: OutputFormat,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Normalizing genesis manifest");
        let manifest = RawGenesisTransaction::from_path(&self.genesis_file)?;
        let normalized = manifest.normalize()?;

        match self.format {
            OutputFormat::Json => {
                let json = normalized
                    .to_pretty_json()
                    .map_err(|err| eyre!("serialize normalized genesis: {err}"))?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::Text => render_text(&normalized, writer)?,
        }

        tui::success("Genesis manifest normalized");
        Ok(())
    }
}

fn render_text<T: Write>(
    normalized: &NormalizedGenesis,
    writer: &mut BufWriter<T>,
) -> color_eyre::Result<()> {
    writeln!(writer, "chain: {}", normalized.chain)?;
    writeln!(
        writer,
        "executor: {}",
        normalized
            .executor
            .as_ref()
            .map_or_else(|| "none".into(), |p| p.as_path().display().to_string())
    )?;
    writeln!(writer, "ivm_dir: {}", normalized.ivm_dir.display())?;
    writeln!(writer, "consensus_mode: {:?}", normalized.consensus_mode)?;
    writeln!(writer, "bls_domain: {}", normalized.bls_domain)?;
    writeln!(
        writer,
        "wire_proto_versions: {:?}",
        normalized.wire_proto_versions
    )?;
    writeln!(
        writer,
        "consensus_fingerprint: {}",
        normalized.consensus_fingerprint
    )?;
    writeln!(
        writer,
        "crypto.default_hash: {}",
        normalized.crypto.default_hash
    )?;
    writeln!(
        writer,
        "crypto.allowed_signing: {:?}",
        normalized.crypto.allowed_signing
    )?;

    for (tx_idx, instructions) in normalized.transactions.iter().enumerate() {
        writeln!(
            writer,
            "tx[{tx_idx}] ({} instructions):",
            instructions.len()
        )?;
        for (instr_idx, instr) in instructions.iter().enumerate() {
            let value = iroha_genesis::genesis_instructions_json::instruction_value(instr);
            let rendered = norito::json::to_json(&value)
                .unwrap_or_else(|err| format!("\"<render error: {err}>\""));
            writeln!(writer, "  [{instr_idx}] {rendered}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::NamedTempFile;

    use super::*;

    fn minimal_genesis() -> NamedTempFile {
        let genesis_file = NamedTempFile::new().expect("create temp genesis");
        let genesis_json = r#"{
            "chain": "test-chain",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Permissioned",
            "transactions": [
                {}
            ]
        }"#;
        File::create(genesis_file.path())
            .and_then(|mut f| f.write_all(genesis_json.as_bytes()))
            .expect("write genesis json");
        genesis_file
    }

    #[test]
    fn emits_json() {
        let genesis = minimal_genesis();
        let args = Args {
            genesis_file: genesis.path().to_path_buf(),
            format: OutputFormat::Json,
        };
        let mut sink = BufWriter::new(Vec::new());
        args.run(&mut sink).expect("normalize json");
        let out = String::from_utf8(sink.into_inner().expect("buf")).expect("utf8");
        assert!(
            out.contains("consensus_fingerprint"),
            "output should include metadata"
        );
    }

    #[test]
    fn emits_text_summary() {
        let genesis = minimal_genesis();
        let args = Args {
            genesis_file: genesis.path().to_path_buf(),
            format: OutputFormat::Text,
        };
        let mut sink = BufWriter::new(Vec::new());
        args.run(&mut sink).expect("normalize text");
        let out = String::from_utf8(sink.into_inner().expect("buf")).expect("utf8");
        assert!(out.contains("tx[0]"), "expected transaction listing");
    }
}
