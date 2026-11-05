//! Read-only lifecycle sizing from committed Torii parameter and trigger JSON exports.

use iroha_data_model::{account::AccountId, parameter::Parameters, trigger::Trigger};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    println!("{}", inspect(&args)?);
    Ok(())
}

fn inspect(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    if args.len() != 4 && (args.len() != 6 || args[4] != "--rekey-to") {
        return Err("usage: trigger_lifecycle_preflight PARAMETERS.json TRIGGERS.json CUTS ORDINARY_WRITES_PER_BLOCK [--rekey-to CANONICAL_ACCOUNT_ID]".into());
    }
    let destination = args
        .get(5)
        .map(|literal| AccountId::parse_encoded(literal))
        .transpose()?;
    // The caller authenticates one common committed cut. No URL, credentials,
    // signing key or mutation endpoint is accepted by this program.
    let parameters: Parameters = norito::json::from_slice(&std::fs::read(&args[0])?)?;
    let triggers = parse_exact_trigger_export(&std::fs::read(&args[1])?)?;
    let report = iroha_core::runtime::lifecycle_preflight::lifecycle_preflight(
        &parameters,
        &triggers,
        args[2].parse()?,
        args[3].parse()?,
        destination.as_ref(),
    )
    .map_err(std::io::Error::other)?;
    Ok(norito::json::to_json_pretty(&report)?)
}

/// Preserve the supplied canonical Trigger frames before measuring anything.
/// Trigger JSON is a base64 string containing the entire canonical Trigger,
/// including its Action. Equality therefore also binds the Action representation.
/// This detects decoder normalization, not an upstream exporter that already
/// changed or omitted source data; the caller must authenticate an exact export.
fn parse_exact_trigger_export(bytes: &[u8]) -> Result<Vec<Trigger>, Box<dyn std::error::Error>> {
    let original: norito::json::Value = norito::json::from_slice(bytes)?;
    let originals = original
        .as_array()
        .ok_or("preflight trigger export must be an array")?;
    let triggers: Vec<Trigger> = norito::json::from_slice(bytes).map_err(|error| {
        std::io::Error::other(format!(
            "preflight trigger source changed or refused during canonical decoding: {error}"
        ))
    })?;
    if originals.len() != triggers.len() {
        return Err(
            "preflight trigger source changed during canonical decoding: entry count".into(),
        );
    }
    for (index, (original, trigger)) in originals.iter().zip(&triggers).enumerate() {
        let canonical: String = norito::json::from_json(&norito::json::to_json(trigger)?)?;
        if original.as_str() != Some(canonical.as_str()) {
            return Err(format!(
                "preflight trigger source changed during canonical decoding: entry {index}"
            )
            .into());
        }
    }
    Ok(triggers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_preflight_cli_reads_torii_exports_without_mutation() {
        let directory =
            std::env::temp_dir().join(format!("iroha-lifecycle-preflight-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let parameters = directory.join("parameters.json");
        let triggers = directory.join("triggers.json");
        let original = norito::json::to_json(&Parameters::default()).unwrap();
        std::fs::write(&parameters, &original).unwrap();
        std::fs::write(&triggers, "[]").unwrap();
        let args = vec![
            parameters.display().to_string(),
            triggers.display().to_string(),
            "4096".into(),
            "10000".into(),
        ];
        let report: norito::json::Value =
            norito::json::from_json(&inspect(&args).unwrap()).unwrap();
        assert_eq!(report["registered_triggers"].as_u64(), Some(0));
        assert_eq!(
            report["block_byte_visit_budget"].as_u64(),
            Some(1024 * 1024 * 1024)
        );
        assert_eq!(std::fs::read_to_string(&parameters).unwrap(), original);
        assert_eq!(std::fs::read_to_string(&triggers).unwrap(), "[]");
        assert!(inspect(&args[..3]).is_err());
        let key =
            iroha_crypto::KeyPair::from_seed(vec![0x65; 32], iroha_crypto::Algorithm::Ed25519);
        let destination = AccountId::new(key.public_key().clone()).to_string();
        let mut with_target = args.clone();
        with_target.extend(["--rekey-to".into(), destination.clone()]);
        let targeted: norito::json::Value =
            norito::json::from_json(&inspect(&with_target).unwrap()).unwrap();
        assert_eq!(
            targeted["rekey_destination"].as_str(),
            Some(destination.as_str())
        );
        assert_eq!(targeted["all_sources"]["triggers"].as_u64(), Some(0));
        with_target[5] = "not-a-canonical-account".into();
        assert!(inspect(&with_target).is_err());
        with_target[4] = "--wrong-option".into();
        assert!(inspect(&with_target).is_err());
        assert!(inspect(&with_target[..5]).is_err());

        use iroha_data_model::prelude::*;
        let authority = AccountId::new(key.public_key().clone());
        let mut action = Action::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(Level::INFO, "export".into()))].into(),
            ),
            Repeats::Indefinitely,
            authority,
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        let exact_export = norito::json::to_json(&vec![Trigger::new(
            "exported".parse().unwrap(),
            action.clone(),
        )])
        .unwrap();
        std::fs::write(&triggers, &exact_export).unwrap();
        let accepted: norito::json::Value =
            norito::json::from_json(&inspect(&args).unwrap()).unwrap();
        assert_eq!(accepted["registered_triggers"].as_u64(), Some(1));
        assert_eq!(
            norito::json::to_json(&parse_exact_trigger_export(exact_export.as_bytes()).unwrap())
                .unwrap(),
            exact_export
        );
        assert_eq!(std::fs::read_to_string(&triggers).unwrap(), exact_export);

        // A stored authorityless ByCall source must not silently become bound
        // during model decoding before the production rekey measurement.
        action.filter = ExecuteTriggerEventFilter::new().into();
        let authorityless_export =
            norito::json::to_json(&vec![Trigger::new("exported".parse().unwrap(), action)])
                .unwrap();
        std::fs::write(&triggers, &authorityless_export).unwrap();
        let error = inspect(&args).unwrap_err().to_string();
        assert!(
            error.contains("preflight trigger source changed"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(&triggers).unwrap(),
            authorityless_export
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
