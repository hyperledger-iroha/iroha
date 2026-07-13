use std::{
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use color_eyre::eyre::{WrapErr as _, eyre};
use iroha_data_model::{account::address::ChainDiscriminantGuard, name::Name};
use iroha_genesis::{ManifestCrypto, RawGenesisTransaction, genesis_instructions_json};

use crate::{
    Outcome, RunArgs,
    genesis::{ConsensusPolicy, build_line_from_env, validate_consensus_mode_for_line},
    tui,
};

/// Validate a genesis JSON file and report offending fields (e.g., invalid `Name`s)
#[derive(Clone, Debug, Parser)]
pub struct Args {
    /// Path to genesis json file
    genesis_file: PathBuf,
}

struct Offense {
    path: String,
    message: String,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Validating genesis manifest");
        let bytes = fs::read(&self.genesis_file)?;
        let json: norito::json::Value = norito::json::from_slice(&bytes)?;
        let consensus_mode = json.get("consensus_mode");
        if consensus_mode.is_none() || consensus_mode.is_some_and(norito::json::Value::is_null) {
            return Err(eyre!(
                "genesis manifest missing consensus_mode; regenerate with `kagami genesis generate --consensus-mode <mode>`"
            ));
        }
        let manifest = RawGenesisTransaction::from_path(&self.genesis_file)
            .wrap_err("genesis manifest failed structural validation")?;
        validate_consensus_manifest(&manifest, build_line_from_env())?;

        let offenses = collect_offenses_from_value(&json, Some(manifest.chain_discriminant()));

        if offenses.is_empty() {
            writeln!(writer, "OK: no offending identifiers found")?;
            tui::success("Genesis manifest validated");
        } else {
            writeln!(writer, "Found {} offending field(s):", offenses.len())?;
            for off in offenses {
                writeln!(writer, "- {}: {}", off.path, off.message)?;
            }
            // Non-zero exit to integrate with CI or scripts
            tui::warn("Validation failed");
            color_eyre::eyre::bail!("genesis validation failed")
        }

        Ok(())
    }
}

fn validate_consensus_manifest(
    manifest: &RawGenesisTransaction,
    build_line: iroha_version::BuildLine,
) -> color_eyre::Result<()> {
    super::require_v2_wire_protocol_only(manifest)?;
    let consensus_mode = manifest.consensus_mode();
    validate_consensus_mode_for_line(build_line, consensus_mode, ConsensusPolicy::Any)?;
    let has_npos = super::has_npos_parameters(manifest)?;
    match (consensus_mode, has_npos) {
        (iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned, false)
        | (iroha_data_model::parameter::system::SumeragiConsensusMode::Npos, true) => {}
        (iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned, true) => {
            return Err(eyre!(
                "permissioned genesis must omit `sumeragi_npos_parameters`"
            ));
        }
        (iroha_data_model::parameter::system::SumeragiConsensusMode::Npos, false) => {
            return Err(eyre!("NPoS genesis requires `sumeragi_npos_parameters`"));
        }
    }
    Ok(())
}

fn collect_offenses_from_value(
    json: &norito::json::Value,
    chain_discriminant: Option<u16>,
) -> Vec<Offense> {
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    let mut offenses: Vec<Offense> = Vec::new();

    // Validate top-level fields if present
    validate_parameters("/parameters", json.get("parameters"), &mut offenses);
    validate_instructions_array("/instructions", json.get("instructions"), &mut offenses);
    validate_crypto("/crypto", json.get("crypto"), &mut offenses);

    // Validate per-transaction fields
    if let Some(txs) = json.get("transactions").and_then(|v| v.as_array()) {
        for (i, tx) in txs.iter().enumerate() {
            validate_parameters(
                &format!("/transactions/{i}/parameters"),
                tx.get("parameters"),
                &mut offenses,
            );
            validate_instructions_array(
                &format!("/transactions/{i}/instructions"),
                tx.get("instructions"),
                &mut offenses,
            );
        }
    }

    offenses
}

fn validate_parameters(
    path: &str,
    params: Option<&norito::json::Value>,
    offenses: &mut Vec<Offense>,
) {
    let Some(params) = params else {
        return;
    };
    let Some(custom) = params.get("custom") else {
        return;
    };
    if let Some(map) = custom.as_object() {
        for key in map.keys() {
            if let Err(err) = key.parse::<Name>() {
                offenses.push(Offense {
                    path: format!("{path}/custom/{}", key),
                    message: format!("invalid Name: {}", err),
                });
            }
        }
    }
}

fn validate_crypto(path: &str, value: Option<&norito::json::Value>, offenses: &mut Vec<Offense>) {
    let Some(value) = value else {
        return;
    };
    match norito::json::value::from_value::<ManifestCrypto>(value.clone()) {
        Ok(crypto) => {
            if let Err(err) = crypto.validate() {
                offenses.push(Offense {
                    path: path.to_owned(),
                    message: err.to_string(),
                });
            }
        }
        Err(err) => offenses.push(Offense {
            path: path.to_owned(),
            message: format!("invalid crypto configuration: {err}"),
        }),
    }
}

fn validate_instructions_array(
    path: &str,
    maybe_instrs: Option<&norito::json::Value>,
    offenses: &mut Vec<Offense>,
) {
    let Some(instrs) = maybe_instrs else {
        return;
    };
    let Some(arr) = instrs.as_array() else {
        return;
    };
    let wrapped = norito::json::Value::Array(arr.clone());
    if let Err(err) = genesis_instructions_json::from_value(&wrapped) {
        offenses.push(Offense {
            path: path.to_owned(),
            message: format!("invalid instructions: {err}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{io::BufWriter, path::PathBuf};

    use iroha_data_model::{
        ChainId,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters},
        },
    };
    use iroha_genesis::GenesisBuilder;
    use tempfile::NamedTempFile;

    use super::*;

    fn manifest_file_with_protocols(versions: &[u32]) -> NamedTempFile {
        let manifest = GenesisBuilder::new_without_executor(ChainId::from("v2-only"), ".")
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned)
            .with_consensus_meta();
        let mut value = norito::json::value::to_value(&manifest).expect("serialize manifest");
        value
            .as_object_mut()
            .expect("manifest serializes as object")
            .insert(
                "wire_protocol_version".to_owned(),
                norito::json::value::to_value(&versions.to_vec()).expect("serialize protocol list"),
            );
        let file = NamedTempFile::new().expect("create genesis fixture");
        fs::write(
            file.path(),
            norito::json::to_json_pretty(&value).expect("serialize manifest JSON"),
        )
        .expect("write genesis fixture");
        file
    }

    #[test]
    fn validation_rejects_legacy_mixed_empty_and_duplicate_protocol_lists() {
        for versions in [Vec::new(), vec![1], vec![1, 2], vec![2, 1], vec![2, 2]] {
            let file = manifest_file_with_protocols(&versions);
            let error = Args {
                genesis_file: file.path().to_owned(),
            }
            .run(&mut BufWriter::new(Vec::<u8>::new()))
            .expect_err("non-canonical protocol list must fail validation");
            assert!(
                error
                    .to_string()
                    .contains("exactly wire_protocol_version = [2]"),
                "unexpected error for {versions:?}: {error}"
            );
        }
    }

    #[test]
    fn detects_invalid_custom_parameter_key() {
        let json = norito::json!({
            "chain": "0",
            "executor": "executor.to",
            "ivm_dir": ".",
            "transactions": [{
                "parameters": {
                    "custom": { "bad key": 10 }
                },
                "instructions": []
            }]
        });

        // Call internal validator directly to avoid filesystem use in tests
        let offenses = collect_offenses_from_value(&json, None);
        let out = offenses
            .into_iter()
            .map(|o| format!("{}: {}", o.path, o.message))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(out.contains("/transactions/0/parameters/custom/bad key"));
        assert!(out.contains("invalid Name"));
    }

    #[test]
    fn instruction_validation_respects_manifest_chain_discriminant() {
        let json = norito::json!({
            "transactions": [{
                "instructions": [{
                    "Mint": {
                        "Asset": {
                            "destination": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#testuﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                            "object": "13"
                        }
                    }
                }]
            }]
        });

        let offenses = collect_offenses_from_value(&json, Some(369));

        assert!(
            offenses.is_empty(),
            "Taira account literals must validate under the manifest chain discriminant"
        );
    }

    #[test]
    fn run_rejects_missing_consensus_mode() {
        let manifest = r#"{
            "chain": "0",
            "chain_discriminant": 1,
            "executor": null,
            "ivm_dir": ".",
            "transactions": [
                {}
            ]
        }"#;

        let temp = NamedTempFile::new().expect("create temp file");
        fs::write(temp.path(), manifest).expect("write manifest");

        let args = Args {
            genesis_file: temp.path().to_path_buf(),
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = args
            .run(&mut sink)
            .expect_err("missing consensus_mode should be rejected");
        assert!(
            err.to_string().contains("consensus_mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_accepts_permissioned_on_iroha3() {
        let manifest = GenesisBuilder::new_without_executor(ChainId::from("0"), PathBuf::from("."))
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned)
            .with_consensus_meta();
        let manifest = norito::json::to_json_pretty(&manifest).expect("serialize manifest");

        let temp = NamedTempFile::new().expect("create temp file");
        fs::write(temp.path(), manifest).expect("write manifest");

        let args = Args {
            genesis_file: temp.path().to_path_buf(),
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink)
            .expect("permissioned consensus should be allowed on Iroha3");
    }

    #[test]
    fn run_accepts_canonical_npos_on_iroha3() {
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("npos-validate"),
            PathBuf::from("."),
        )
        .append_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta();
        let json = norito::json::to_json_pretty(&manifest).expect("serialize manifest");

        let temp = NamedTempFile::new().expect("create temp file");
        fs::write(temp.path(), json).expect("write manifest");

        let args = Args {
            genesis_file: temp.path().to_path_buf(),
        };

        let mut sink = BufWriter::new(Vec::<u8>::new());
        args.run(&mut sink)
            .expect("canonical NPoS consensus should be accepted on Iroha3");
    }
}
