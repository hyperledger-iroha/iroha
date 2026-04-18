//! Regenerate grouped Nexus and streaming golden fixtures from canonical Rust encoders.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

#[path = "../../tests/streaming/mod.rs"]
mod streaming;

use hex::encode as hex_encode;
use iroha_data_model::DomainId;
use iroha_data_model::prelude::{
    AccountId, AssetDefinitionId, AssetId, Burn, InstructionBox, Mint, Numeric, TriggerId,
};
use norito::codec::Encode;

const FIXTURE_PUBLIC_KEY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

struct InstructionFixture<'a> {
    file_name: &'a str,
    fixture_id: &'a str,
    description: &'a str,
    instruction: InstructionBox,
}

fn main() -> Result<(), Box<dyn Error>> {
    refresh_norito_instruction_fixtures()?;
    refresh_streaming_snapshot_fixtures()?;
    Ok(())
}

fn refresh_norito_instruction_fixtures() -> Result<(), Box<dyn Error>> {
    for fixture in instruction_fixtures()? {
        let path = norito_instruction_fixture_path(fixture.file_name);
        let document = instruction_fixture_document(&fixture)?;
        write_fixture(&path, &document)?;
        println!("updated {}", path.display());
    }
    Ok(())
}

fn refresh_streaming_snapshot_fixtures() -> Result<(), Box<dyn Error>> {
    let baseline = streaming::baseline_test_vector().snapshot_json()?;
    let baseline_path = streaming_fixture_path("baseline.json");
    write_fixture(&baseline_path, &baseline)?;
    println!("updated {}", baseline_path.display());

    let bundled = streaming::bundled_test_vector().snapshot_json()?;
    let bundled_path = streaming_fixture_path("bundled.json");
    write_fixture(&bundled_path, &bundled)?;
    println!("updated {}", bundled_path.display());

    Ok(())
}

fn instruction_fixture_document(
    fixture: &InstructionFixture<'_>,
) -> Result<String, Box<dyn Error>> {
    let instruction = norito::json::to_value(&fixture.instruction)?;
    let mut bytes = Vec::new();
    fixture.instruction.encode_to(&mut bytes);

    let mut document = norito::json::Map::new();
    document.insert(
        "fixture_id".into(),
        norito::json::Value::from(fixture.fixture_id),
    );
    document.insert(
        "description".into(),
        norito::json::Value::from(fixture.description),
    );
    document.insert("instruction".into(), instruction);
    document.insert(
        "encoded_hex".into(),
        norito::json::Value::from(hex_encode(bytes)),
    );

    Ok(format!(
        "{}\n",
        norito::json::to_string_pretty(&norito::json::Value::Object(document))?
    ))
}

fn write_fixture(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    let Some(parent) = path.parent() else {
        return Err(format!("fixture path has no parent: {}", path.display()).into());
    };
    fs::create_dir_all(parent)?;
    fs::write(path, contents)?;
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("integration_tests has repository parent")
        .to_path_buf()
}

fn norito_instruction_fixture_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("norito_instructions")
        .join(file_name)
}

fn streaming_fixture_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("integration_tests")
        .join("fixtures")
        .join("norito_streaming")
        .join("rans")
        .join(file_name)
}

fn instruction_fixtures() -> Result<Vec<InstructionFixture<'static>>, Box<dyn Error>> {
    let asset_id = fixture_asset_id()?;
    let burn_numeric = Numeric::from_str("4")?;
    let burn_fractional = Numeric::from_str("3.1415")?;
    let mint_numeric = Numeric::from_str("4")?;
    let trigger_id = TriggerId::from_str("reconciliation_guard")?;

    Ok(vec![
        InstructionFixture {
            file_name: "burn_asset_numeric.json",
            fixture_id: "burn-asset-numeric-v1",
            description: "Canonical Norito encoding for a Burn::Asset numeric instruction burning 4 units.",
            instruction: Burn::asset_numeric(burn_numeric, asset_id.clone()).into(),
        },
        InstructionFixture {
            file_name: "burn_asset_fractional.json",
            fixture_id: "burn-asset-fractional-v1",
            description: "Canonical Norito encoding for a Burn::Asset fractional instruction burning 3.1415 units.",
            instruction: Burn::asset_numeric(burn_fractional, asset_id.clone()).into(),
        },
        InstructionFixture {
            file_name: "mint_asset_numeric.json",
            fixture_id: "mint-asset-numeric-v1",
            description: "Canonical Norito encoding for a Mint::Asset numeric instruction minting 4 units.",
            instruction: Mint::asset_numeric(mint_numeric, asset_id).into(),
        },
        InstructionFixture {
            file_name: "burn_trigger_repetitions.json",
            fixture_id: "burn-trigger-repetitions-v1",
            description: "Canonical Norito encoding for a Burn::TriggerRepetitions instruction burning 7 repetitions for trigger reconciliation_guard.",
            instruction: Burn::trigger_repetitions(7, trigger_id).into(),
        },
    ])
}

fn fixture_asset_id() -> Result<AssetId, Box<dyn Error>> {
    let public_key = FIXTURE_PUBLIC_KEY.parse()?;
    let account = AccountId::new(public_key);
    let definition = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    Ok(AssetId::new(definition, account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_fixture_document_matches_canonical_hex() {
        let fixture = instruction_fixtures()
            .expect("instruction fixtures")
            .into_iter()
            .find(|fixture| fixture.file_name == "burn_asset_numeric.json")
            .expect("burn fixture present");
        let document = instruction_fixture_document(&fixture).expect("document");
        let value: norito::json::Value = norito::json::from_str(&document).expect("json");
        let object = value.as_object().expect("fixture object");

        let encoded_hex = object
            .get("encoded_hex")
            .and_then(norito::json::Value::as_str)
            .expect("encoded_hex string");
        let mut bytes = Vec::new();
        fixture.instruction.encode_to(&mut bytes);
        assert_eq!(encoded_hex, hex_encode(bytes));
    }

    #[test]
    fn streaming_snapshot_refresh_emits_manifest_template_hex() {
        let snapshot = streaming::baseline_test_vector()
            .snapshot_json()
            .expect("baseline snapshot");
        assert!(snapshot.contains("\"manifest_template_hex\""));
        assert!(snapshot.contains("\"chunk_commitments\""));
    }
}
