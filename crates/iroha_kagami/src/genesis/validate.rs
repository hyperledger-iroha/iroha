use std::{
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use iroha_data_model::name::Name;
use iroha_genesis::{ManifestCrypto, genesis_instructions_json};

use crate::{Outcome, RunArgs, tui};

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

        let offenses = collect_offenses_from_value(&json);

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

fn collect_offenses_from_value(json: &norito::json::Value) -> Vec<Offense> {
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
    use super::*;

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
        let offenses = collect_offenses_from_value(&json);
        let out = offenses
            .into_iter()
            .map(|o| format!("{}: {}", o.path, o.message))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(out.contains("/transactions/0/parameters/custom/bad key"));
        assert!(out.contains("invalid Name"));
    }
}
