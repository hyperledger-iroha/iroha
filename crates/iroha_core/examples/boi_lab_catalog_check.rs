//! Read-only commissioning checks against the compiled release catalog.
//! The caller supplies independently authenticated public inputs.

use std::{error::Error, fs, path::Path};

use iroha_core::runtime::{ReleaseCatalog, release_catalog, trigger_lifecycle_enabled};
use iroha_crypto::Hash;
use iroha_data_model::{
    runtime::{
        TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
        definition::{DefinitionDescriptorV1, DefinitionId, RootSelectionV1},
        lifecycle::SelectionTransitionV1,
    },
    sumeragi_finality::SelectedStateSchemaV1,
};

type CheckResult<T> = Result<T, Box<dyn Error>>;

fn inspect_selection(catalog: &ReleaseCatalog, selection: &RootSelectionV1) -> CheckResult<()> {
    if let Some(id) = catalog.unsupported(selection) {
        return Err(format!("selection requires an unsupported definition {id}").into());
    }
    selection.check_closure(&catalog.definitions)?;
    let lifecycle = trigger_lifecycle_enabled(selection)?;
    println!(
        "selection={} services={} lifecycle={lifecycle} bindings={}",
        selection.id()?,
        selection.services_profile,
        selection.bindings.len()
    );
    if selection.commitment_profile == iroha_data_model::runtime::keyed_state::profile_id() {
        let schema = SelectedStateSchemaV1::from_selection(selection, &catalog.definitions)?;
        println!(
            "selected_keyed_schema_digest={}",
            hex::encode(schema.digest())
        );
    }
    Ok(())
}

fn inspect_service(catalog: &ReleaseCatalog, id: DefinitionId) -> CheckResult<()> {
    let closure = Hash::new(TRIGGER_LIFECYCLE_SERVICE_CLOSURE);
    let enabled = catalog.service_enabled(id, "trigger_lifecycle", closure.into())?;
    println!("historical_services={id} lifecycle={enabled}");
    Ok(())
}

fn inspect_transitions(
    catalog: &ReleaseCatalog,
    transitions: &[SelectionTransitionV1],
) -> CheckResult<()> {
    for transition in transitions {
        if let SelectionTransitionV1::SetServicesProfile(id) = transition {
            inspect_service(catalog, *id)?;
        }
    }
    Ok(())
}

fn main() -> CheckResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [mode, input] = args.as_slice() else {
        return Err(
            "expected selection, descriptor or transitions followed by one public JSON file".into(),
        );
    };
    let bytes = fs::read(Path::new(input))?;
    let catalog = release_catalog()?;
    match mode.to_str() {
        Some("selection") => inspect_selection(catalog, &norito::json::from_slice(&bytes)?),
        Some("descriptor") => {
            let descriptor: DefinitionDescriptorV1 = norito::json::from_slice(&bytes)?;
            descriptor.validate()?;
            if !matches!(descriptor, DefinitionDescriptorV1::ServicesProfile(_)) {
                return Err("expected a services-profile descriptor".into());
            }
            inspect_service(catalog, descriptor.id()?)
        }
        Some("transitions") => {
            let transitions: Vec<SelectionTransitionV1> = norito::json::from_slice(&bytes)?;
            inspect_transitions(catalog, &transitions)
        }
        _ => Err("unknown inspection mode".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::runtime::definition::ServiceEntryV1;

    #[test]
    fn accepts_stock_baseline_and_its_historical_service() {
        let catalog = release_catalog().unwrap();
        inspect_selection(catalog, &catalog.baseline).unwrap();
        inspect_service(catalog, catalog.baseline.services_profile).unwrap();
    }

    #[test]
    fn a_distinct_well_formed_service_is_unsupported_even_after_return_to_baseline() {
        let catalog = release_catalog().unwrap();
        let mut descriptor = catalog.definitions[&catalog.baseline.services_profile].clone();
        let DefinitionDescriptorV1::ServicesProfile(profile) = &mut descriptor else {
            panic!("baseline must name a services descriptor");
        };
        profile.services.push(ServiceEntryV1 {
            name: "boi_lab_commission_test".to_owned(),
            reference_closure: Hash::new(b"BOI lab test service").into(),
        });
        profile.services.sort_by(|a, b| a.name.cmp(&b.name));
        descriptor.validate().unwrap();
        let id = descriptor.id().unwrap();
        let history = [
            SelectionTransitionV1::SetServicesProfile(id),
            SelectionTransitionV1::SetServicesProfile(catalog.baseline.services_profile),
        ];
        let err = inspect_transitions(catalog, &history).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("runtime services profile {id} is unsupported")
        );
        inspect_selection(catalog, &catalog.baseline).unwrap();
    }
}
