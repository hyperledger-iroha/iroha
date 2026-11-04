//! Read-only Ironwood predecessor check of an exported `/v1/runtime/upgrades` response.
//! No network, credentials, submission or scheduling. Endpoint observations are not finality.

use iroha_core::runtime::ironwood::activation_change_set_for_selection;
use iroha_data_model::runtime::definition::RootSelectionV1;

type Projection = (
    String,
    RootSelectionV1,
    std::collections::BTreeMap<
        iroha_data_model::runtime::definition::DefinitionId,
        iroha_data_model::runtime::definition::DefinitionDescriptorV1,
    >,
);
fn check_projection(text: &str, activation_height: u64) -> Result<Projection, String> {
    let value: norito::json::Value = norito::json::from_str(text).map_err(|e| e.to_string())?;
    let selection: RootSelectionV1 = norito::json::from_str(
        &norito::json::to_json(
            value
                .get("selection")
                .ok_or("missing committed selection")?,
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let actual = selection.id().map_err(|e| e.to_string())?.to_string();
    if value
        .get("selection_id_hex")
        .and_then(norito::json::Value::as_str)
        != Some(actual.as_str())
    {
        return Err("reported selection_id_hex does not match the complete selection".into());
    }
    if activation_height == 0 {
        return Err("activation height must be positive".into());
    }
    let items = value
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or("missing committed change-set inventory")?;
    // Compiled support includes future definitions; publication starts at the founded
    // baseline and the activated records in this complete inventory. A selected definition
    // must also be published by closure, including configured Time activation, which
    // intentionally does not manufacture a governed change-set record.
    let catalog = iroha_core::runtime::release_catalog().map_err(|e| e.to_string())?;
    let mut definitions: std::collections::BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    let mut due = Vec::new();
    for item in items {
        let record: iroha_data_model::runtime::lifecycle::ChangeSetRecordV1 =
            norito::json::from_str(
                &norito::json::to_json(item.get("record").ok_or("missing change-set record")?)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        let id = record.change_set.id().map_err(|e| e.to_string())?;
        if item.get("id_hex").and_then(norito::json::Value::as_str) != Some(id.to_string().as_str())
        {
            return Err("reported change-set identity does not match its record".into());
        }
        if matches!(
            record.status,
            iroha_data_model::runtime::lifecycle::ChangeSetStatusV1::Activated(_)
        ) {
            for definition in &record.change_set.publish {
                let id = definition.id().map_err(|e| e.to_string())?;
                if definitions.insert(id, definition.clone()).is_some() {
                    return Err("activated inventory repeats a published definition".into());
                }
            }
        }
        if record.status == iroha_data_model::runtime::lifecycle::ChangeSetStatusV1::Scheduled {
            if record.change_set.effective_height == activation_height {
                return Err(format!(
                    "another change set {id} is already scheduled at activation height {activation_height}"
                ));
            }
            if record.change_set.effective_height < activation_height {
                due.push((id, record));
            }
        }
    }
    let selected: std::collections::BTreeSet<_> = [
        selection.kernel,
        selection.commitment_profile,
        selection.services_profile,
    ]
    .into_iter()
    .chain(selection.installed_storage.iter().copied())
    .chain(selection.active.iter().copied())
    .chain(
        selection
            .bindings
            .iter()
            .flat_map(|binding| [binding.interface, binding.implementation]),
    )
    .collect();
    for id in selected {
        let compiled = catalog.definitions.get(&id).ok_or_else(|| {
            format!("selected definition {id} is not implemented by this release")
        })?;
        if let Some(published) = definitions.get(&id) {
            if published != compiled {
                return Err(format!(
                    "selected definition {id} differs from its compiled identity"
                ));
            }
        } else {
            // Infer only references of the reported committed selection, never an unused
            // compiled successor. The operator still authenticates this observation.
            definitions.insert(id, compiled.clone());
        }
    }
    selection
        .check_closure(&definitions)
        .map_err(|e| e.to_string())?;
    due.sort_by_key(|(id, record)| (record.change_set.effective_height, *id));
    let mut projected = selection;
    for (id, record) in &due {
        let applied = record
            .change_set
            .apply(&projected, &definitions)
            .map_err(|e| format!("scheduled predecessor projection at {id} failed: {e}"))?;
        definitions.extend(applied.published);
        projected = applied.selection;
    }
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"preflight-only")),
    );
    activation_change_set_for_selection(
        network,
        activation_height,
        [1; 32],
        &projected,
        &definitions,
    )
    .map_err(|e| format!("Ironwood activation predecessor mismatch: {e}"))?;
    Ok((
        format!(
            "MATCH: observed selection {actual} and its projection through {activation_height} ({count} scheduled changes) admit a current-selection Ironwood composition. Observation only: finality, compiled composition support, decoder readiness and governance approval remain required.",
            count = due.len()
        ),
        projected,
        definitions,
    ))
}

#[cfg(test)]
fn check(text: &str, activation_height: u64) -> Result<String, String> {
    check_projection(text, activation_height).map(|p| p.0)
}

// Qualification binds the exact retained evidence artifact, not a claimed passing status.
fn emit_change_set(
    height: u64,
    network: &str,
    evidence: &[u8],
    previous: &RootSelectionV1,
    definitions: &std::collections::BTreeMap<
        iroha_data_model::runtime::definition::DefinitionId,
        iroha_data_model::runtime::definition::DefinitionDescriptorV1,
    >,
) -> Result<String, String> {
    if evidence.is_empty() || evidence.len() > 16 * 1024 * 1024 {
        return Err("qualification evidence must be 1..16MiB".into());
    }
    let bytes: [u8; 32] = hex::decode(network)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "network ID must be 32 bytes")?;
    if bytes[31] & 1 == 0 || hex::encode(bytes) != network {
        return Err("network ID must be canonical lowercase marked hash".into());
    }
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(bytes)),
    );
    let qualification = iroha_crypto::Hash::new_from_chunks(&[
        b"iroha:ironwood:qualification-evidence:v1\0",
        evidence,
    ])
    .into();
    let change =
        activation_change_set_for_selection(network, height, qualification, previous, definitions)
            .map_err(|e| e.to_string())?;
    norito::json::to_json(&change).map_err(|e| e.to_string())
}

fn run() -> Result<String, String> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or(
        "usage: ironwood_activation_preflight <runtime-upgrades.json> <activation-height>",
    )?;
    let height = args
        .next()
        .ok_or("missing activation height")?
        .to_str()
        .ok_or("invalid activation height")?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let emit = args.next();
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let (checked, previous, definitions) = check_projection(&text, height)?;
    let Some(emit) = emit else {
        return Ok(checked);
    };
    if emit != "--emit-change-set" {
        return Err(
            "expected --emit-change-set <network-id-hex> <qualification-evidence-file>".into(),
        );
    }
    let network = args
        .next()
        .ok_or("missing network ID")?
        .into_string()
        .map_err(|_| "invalid network ID")?;
    let evidence = args.next().ok_or("missing qualification evidence file")?;
    if args.next().is_some() {
        return Err("unexpected preflight arguments".into());
    }
    eprintln!("{checked}");
    emit_change_set(
        height,
        &network,
        &std::fs::read(evidence).map_err(|e| e.to_string())?,
        &previous,
        &definitions,
    )
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(message) => {
            println!("{message}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ironwood_preflight_composes_after_configured_time_without_inventing_future_publication() {
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let current = &catalog.lifecycle;
        // Configured activation publishes the actual selected definitions but no governed record.
        let report = norito::json::to_json(&norito::json!({
            "selection": current, "selection_id_hex": (current.id().unwrap().to_string()),
            "items": []
        }))
        .unwrap();
        let (_, projected, definitions) = check_projection(&report, 200).unwrap();
        assert_eq!(&projected, current);
        assert_eq!(definitions.len(), catalog.baseline_definitions.len() + 2);
        let ironwood = catalog.ironwood.as_ref().unwrap();
        assert!(
            ironwood
                .definitions
                .iter()
                .all(|id| !definitions.contains_key(id))
        );
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"configured Time preflight",
            )),
        );
        let change =
            activation_change_set_for_selection(network, 200, [0x94; 32], &projected, &definitions)
                .unwrap();
        let applied = change.apply(&projected, &definitions).unwrap();
        assert!(iroha_core::runtime::trigger_lifecycle_enabled(&applied.selection).unwrap());
        assert_eq!(
            applied.selection.installed_storage,
            ironwood.selection.installed_storage
        );
        let mut builder = catalog.clone();
        assert_eq!(
            applied.selection,
            builder.lifecycle_successor(&ironwood.selection).unwrap()
        );
        // Existing complete-inventory rules still apply; a missing inventory is not inferred.
        let missing = norito::json::to_json(&norito::json!({
            "selection": current, "selection_id_hex": (current.id().unwrap().to_string())
        }))
        .unwrap();
        assert!(
            check_projection(&missing, 200)
                .unwrap_err()
                .contains("missing committed change-set inventory")
        );
    }

    #[test]
    fn ironwood_preflight_matches_complete_predecessor_and_refuses_stale_or_forged_reports() {
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        for (selection, matches) in [
            (&catalog.baseline, true),
            (&catalog.ironwood.as_ref().unwrap().selection, false),
        ] {
            let json = norito::json!({"selection": selection, "selection_id_hex": (selection.id().unwrap().to_string()), "items": []});
            assert_eq!(
                check(&norito::json::to_json(&json).unwrap(), 200).is_ok(),
                matches
            );
        }
        let forged = norito::json!({"selection": (&catalog.baseline), "selection_id_hex": ("00".repeat(32))});
        assert!(
            check(&norito::json::to_json(&forged).unwrap(), 200)
                .unwrap_err()
                .contains("does not match")
        );
        assert!(check("{}", 200).is_err());
    }
    #[test]
    fn ironwood_preflight_projects_scheduled_predecessors_and_rejects_height_collisions() {
        use iroha_data_model::runtime::lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, RuntimeNoticeV1,
        };
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"preflight")),
        );
        let change =
            iroha_core::runtime::ironwood::activation_change_set(network, 150, [0x79; 32]).unwrap();
        let id = change.id().unwrap().to_string();
        let mut record = ChangeSetRecordV1 {
            change_set: change,
            status: ChangeSetStatusV1::Scheduled,
            proposed_ms: 0,
            approved_height: 100,
            approved_ms: 0,
            attestations: vec![],
            notice: RuntimeNoticeV1 {
                blocks: 1,
                ms: 0,
                approval_cushion_ms: 0,
            },
        };
        let response = |record: &ChangeSetRecordV1| {
            norito::json::to_json(&norito::json!({
            "selection": (&catalog.baseline), "selection_id_hex": (catalog.baseline_id.to_string()),
            "items": [{"id_hex": (&id), "record": record}]
        })).unwrap()
        };
        assert!(
            check(&response(&record), 200)
                .unwrap_err()
                .contains("predecessor mismatch")
        );
        assert!(
            check(&response(&record), 150)
                .unwrap_err()
                .contains("already scheduled")
        );
        assert!(check(&response(&record), 140).is_ok());
        record.status = ChangeSetStatusV1::Lapsed(150);
        assert!(check(&response(&record), 200).is_ok());
    }
}

#[cfg(test)]
#[test]
fn ironwood_preflight_emits_exact_json_and_binds_qualification_bytes() {
    let network = iroha_crypto::Hash::new(b"preflight-network").to_string();
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    let emit = |evidence: &[u8]| {
        emit_change_set(
            200,
            &network,
            evidence,
            &catalog.baseline,
            &catalog.definitions,
        )
    };
    let first = emit(b"qualified artifact inventory").unwrap();
    let change: iroha_data_model::runtime::lifecycle::ChangeSetV1 =
        norito::json::from_str(&first).unwrap();
    assert_eq!(change.effective_height, 200);
    assert_eq!(
        change.predecessor,
        iroha_core::runtime::release_catalog().unwrap().baseline_id
    );
    assert_ne!(first, emit(b"different evidence").unwrap());
    assert!(emit(&[]).is_err());
}
