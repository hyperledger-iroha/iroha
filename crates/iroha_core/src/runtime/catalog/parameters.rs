//! Typed committed parameters this release adds after its frozen baseline
//! (`specs/live_runtime_upgrades.md` §3.1, §4; `specs/genesis_bound_node_configuration.md`,
//! "Frozen-baseline constraint on G2").
//!
//! The frozen baseline stores every unreserved custom parameter without interpreting it, so
//! recognizing a new typed parameter is new behaviour. The release publishes each one as a single
//! Parameter definition: the custom parameter identifier, the Norito schema of its typed payload
//! and the canonical bytes of its protocol default. It publishes no implementation, interface,
//! rebind or storage for it, so the definition activates from every selection a root may run.
//! A root recognizes the parameter only while its selection activates the definition and no
//! `Disabled` restriction names it ([`crate::runtime::committed_policy`]); the change set that makes
//! it recognized writes the definition's initial value over whatever the root stored under the
//! identifier before.

use std::collections::BTreeMap;

use iroha_data_model::{
    NetworkId,
    content_policy::ContentPolicyV1,
    nexus_quota_policy::NexusQuotaPolicyV1,
    parameter::CustomParameter,
    runtime::{
        definition::{
            DefinitionDescriptorV1, DefinitionId, DefinitionLookup, ParameterDescriptorV1,
            RootSelectionV1, SchemaV1,
        },
        lifecycle::{ChangeSetV1, RootScopeV1, SelectionTransitionV1},
    },
    settlement_router_policy::SettlementRouterPolicyV1,
};

use super::{CatalogError, ReleaseCatalog, layout_suffix, sorted};

/// Module name an operator drafts the activation of [`PARAMETERS`] under
/// ([`crate::runtime::draft_addition`]). It names the first two parameters it published; the
/// draft activates every parameter of [`PARAMETERS`] the root does not recognize yet.
pub const MODULE: &str = "iroha.parameter.content_and_router_policy.v1";

/// One typed committed parameter a release adds after the frozen baseline.
#[derive(Clone, Copy, Debug)]
pub struct ParameterAddition {
    /// The custom parameter identifier; it names the Parameter definition.
    pub(crate) name: &'static str,
    /// Norito nominal name of the typed payload.
    pub(crate) nominal: fn() -> String,
    /// Canonical Norito bytes of the value activation writes: the protocol default.
    pub(crate) initial: fn() -> Result<Vec<u8>, String>,
    /// Decode published initial bytes into the custom parameter activation writes.
    pub(crate) install: ParameterInstaller,
    /// The canonical Norito bytes of the valid typed policy a custom parameter carries under the
    /// identifier, comparable with [`Self::initial`].
    pub(crate) canonical: fn(&CustomParameter) -> Result<Vec<u8>, String>,
}

/// Decodes a Parameter definition's published initial bytes into the custom parameter
/// activation writes.
pub type ParameterInstaller = fn(&[u8]) -> Result<CustomParameter, String>;

/// The typed committed parameters this release adds, in publication order.
pub const PARAMETERS: &[ParameterAddition] =
    &[CONTENT_POLICY, SETTLEMENT_ROUTER_POLICY, NEXUS_QUOTA_POLICY];

/// The committed content-bundle limits `PublishContentBundle` applies (`content_policy_v1`).
pub const CONTENT_POLICY: ParameterAddition = ParameterAddition {
    name: ContentPolicyV1::PARAMETER_ID_STR,
    nominal: || norito::schema::identity::nominal_name::<ContentPolicyV1>().into_owned(),
    initial: || {
        norito::encode_canonical(&ContentPolicyV1::protocol_default())
            .map_err(|error| error.to_string())
    },
    install: |bytes| {
        let policy = norito::decode_canonical::<ContentPolicyV1>(bytes)
            .map_err(|error| error.to_string())?;
        policy.validate().map_err(|error| error.to_string())?;
        Ok(policy.into_custom_parameter())
    },
    canonical: |parameter| match ContentPolicyV1::from_custom_parameter(parameter) {
        Some(Ok(policy)) => norito::encode_canonical(&policy).map_err(|error| error.to_string()),
        Some(Err(error)) => Err(error.to_string()),
        None => Err(format!(
            "{} is not the content policy parameter",
            parameter.id().name()
        )),
    },
};

/// The committed inputs of the deterministic XOR settlement router
/// (`settlement_router_policy_v1`).
pub const SETTLEMENT_ROUTER_POLICY: ParameterAddition = ParameterAddition {
    name: SettlementRouterPolicyV1::PARAMETER_ID_STR,
    nominal: || norito::schema::identity::nominal_name::<SettlementRouterPolicyV1>().into_owned(),
    initial: || {
        norito::encode_canonical(&SettlementRouterPolicyV1::protocol_default())
            .map_err(|error| error.to_string())
    },
    install: |bytes| {
        let policy = norito::decode_canonical::<SettlementRouterPolicyV1>(bytes)
            .map_err(|error| error.to_string())?;
        policy.validate().map_err(|error| error.to_string())?;
        Ok(policy.into_custom_parameter())
    },
    canonical: |parameter| match SettlementRouterPolicyV1::from_custom_parameter(parameter) {
        Some(Ok(policy)) => norito::encode_canonical(&policy).map_err(|error| error.to_string()),
        Some(Err(error)) => Err(error.to_string()),
        None => Err(format!(
            "{} is not the settlement router policy parameter",
            parameter.id().name()
        )),
    },
};

/// The committed uploaded-model ceilings `RegisterSoracloudUploadedModelBundle` applies and the
/// per-account DA ingest quota ceilings (`nexus_quota_policy_v1`).
pub const NEXUS_QUOTA_POLICY: ParameterAddition = ParameterAddition {
    name: NexusQuotaPolicyV1::PARAMETER_ID_STR,
    nominal: || norito::schema::identity::nominal_name::<NexusQuotaPolicyV1>().into_owned(),
    initial: || {
        norito::encode_canonical(&NexusQuotaPolicyV1::protocol_default())
            .map_err(|error| error.to_string())
    },
    install: |bytes| {
        let policy = norito::decode_canonical::<NexusQuotaPolicyV1>(bytes)
            .map_err(|error| error.to_string())?;
        policy.validate().map_err(|error| error.to_string())?;
        Ok(policy.into_custom_parameter())
    },
    canonical: |parameter| match NexusQuotaPolicyV1::from_custom_parameter(parameter) {
        Some(Ok(policy)) => norito::encode_canonical(&policy).map_err(|error| error.to_string()),
        Some(Err(error)) => Err(error.to_string()),
        None => Err(format!(
            "{} is not the Nexus quota policy parameter",
            parameter.id().name()
        )),
    },
};

impl ParameterAddition {
    /// The Parameter definition this release publishes for the parameter.
    ///
    /// # Errors
    /// The initial value cannot be encoded (a build defect).
    pub(crate) fn descriptor(&self) -> Result<DefinitionDescriptorV1, CatalogError> {
        Ok(DefinitionDescriptorV1::Parameter(ParameterDescriptorV1 {
            name: self.name.to_owned(),
            schema: SchemaV1::Codec(format!(
                "norito:{}@{}",
                (self.nominal)(),
                layout_suffix(crate::state::authority_registry::V1_LAYOUT)
            )),
            initial: (self.initial)().map_err(CatalogError::Parameters)?,
        }))
    }

    /// Identity of [`Self::descriptor`].
    ///
    /// # Errors
    /// The descriptor cannot be built or is invalid (a build defect).
    pub(crate) fn definition_id(&self) -> Result<DefinitionId, CatalogError> {
        Ok(self.descriptor()?.id()?)
    }
}

impl ReleaseCatalog {
    /// Publish one Parameter definition per entry of `parameters` after the baseline (§4, "Add
    /// an instruction": its new parameters are published with it).
    ///
    /// # Errors
    /// A definition is invalid, or two entries name the same parameter.
    pub(super) fn publish_parameter_additions(
        &mut self,
        parameters: &[ParameterAddition],
    ) -> Result<(), CatalogError> {
        for addition in parameters {
            let id = self.publish(addition.descriptor()?)?;
            if self.parameter_additions.insert(id, *addition).is_some()
                || self.baseline_definitions.binary_search(&id).is_ok()
            {
                return Err(CatalogError::Addition {
                    module: MODULE.to_owned(),
                    reason: format!("parameter {} is published twice", addition.name),
                });
            }
        }
        Ok(())
    }

    /// The typed committed parameter `definition` publishes, if it is one this release adds.
    #[must_use]
    pub(crate) fn parameter_addition(
        &self,
        definition: &DefinitionId,
    ) -> Option<&ParameterAddition> {
        self.parameter_additions.get(definition)
    }

    /// The Parameter definitions this release adds, by identity.
    #[must_use]
    pub fn parameter_definitions(&self) -> Vec<DefinitionId> {
        self.parameter_additions.keys().copied().collect()
    }

    /// The change set that makes the universal root of `current` recognize every typed
    /// committed parameter of this release (§4): it publishes the Parameter definitions
    /// `published` lacks and activates those `current` does not, effective at
    /// `effective_height`. It installs, binds and restricts nothing, so it applies to every
    /// selection a root may run.
    ///
    /// # Errors
    /// Every parameter is already active, or the change set does not apply to `current`.
    pub fn parameter_activation(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        let fail = |reason: String| CatalogError::Addition {
            module: MODULE.to_owned(),
            reason,
        };
        let activated: Vec<DefinitionId> = self
            .parameter_additions
            .keys()
            .copied()
            .filter(|id| current.active.binary_search(id).is_err())
            .collect();
        if activated.is_empty() {
            return Err(fail("every parameter is already active".to_owned()));
        }
        let publish = activated
            .iter()
            .filter(|id| published.definition(id).is_none())
            .map(|id| self.definitions[id].clone())
            .collect();
        let mut successor = current.clone();
        successor.active.extend(activated.iter().copied());
        successor.active = sorted(successor.active);
        let change_set = ChangeSetV1 {
            network,
            root: RootScopeV1::Universal,
            predecessor: current.id()?,
            successor: successor.id()?,
            effective_height,
            publish,
            transitions: activated
                .into_iter()
                .map(SelectionTransitionV1::Activate)
                .collect(),
            qualification,
        };
        change_set
            .apply(current, published)
            .map_err(|error| fail(error.to_string()))?;
        Ok(change_set)
    }
}

/// The parameter additions a catalog publishes, by Parameter definition.
pub(super) type ParameterAdditions = BTreeMap<DefinitionId, ParameterAddition>;

#[cfg(test)]
mod tests {
    use iroha_data_model::runtime::definition::DefinitionKind;
    use iroha_primitives::json::Json;

    use super::*;
    use crate::runtime::release_catalog;

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([7; 32]),
        ))
    }

    #[test]
    fn each_parameter_is_published_once_outside_the_baseline() {
        let catalog = release_catalog().expect("release catalog");
        let definitions = catalog.parameter_definitions();
        assert_eq!(definitions.len(), PARAMETERS.len());
        for addition in PARAMETERS {
            let id = addition.definition_id().expect("definition");
            assert!(definitions.contains(&id));
            assert_eq!(
                catalog.parameter_addition(&id).map(|found| found.name),
                Some(addition.name)
            );
            assert!(catalog.baseline_definitions.binary_search(&id).is_err());
            assert!(catalog.baseline.active.binary_search(&id).is_err());
            assert!(catalog.lifecycle.active.binary_search(&id).is_err());
            let descriptor = &catalog.definitions[&id];
            assert_eq!(descriptor.kind(), DefinitionKind::Parameter);
            let DefinitionDescriptorV1::Parameter(parameter) = descriptor else {
                unreachable!("kind checked above")
            };
            assert_eq!(parameter.name, addition.name);
            // The initial value is the protocol default, which activation writes.
            let installed = (addition.install)(&parameter.initial).expect("initial value");
            assert_eq!(installed.id().name().as_ref(), addition.name);
            assert_eq!(
                (addition.canonical)(&installed).expect("the initial value is valid"),
                parameter.initial,
                "the installed value is exactly the published initial value"
            );
        }
    }

    #[test]
    fn installers_write_the_protocol_defaults_and_validators_refuse_invalid_values() {
        let content = (CONTENT_POLICY.install)(&(CONTENT_POLICY.initial)().unwrap()).unwrap();
        assert_eq!(
            content,
            ContentPolicyV1::protocol_default().into_custom_parameter()
        );
        let router =
            (SETTLEMENT_ROUTER_POLICY.install)(&(SETTLEMENT_ROUTER_POLICY.initial)().unwrap())
                .unwrap();
        assert_eq!(
            router,
            SettlementRouterPolicyV1::protocol_default().into_custom_parameter()
        );
        let invalid_content = ContentPolicyV1 {
            chunk_size_bytes: 0,
            ..ContentPolicyV1::protocol_default()
        };
        assert!((CONTENT_POLICY.canonical)(&invalid_content.into_custom_parameter()).is_err());
        let invalid_router = SettlementRouterPolicyV1 {
            buffer_halt_pct: 99,
            ..SettlementRouterPolicyV1::protocol_default()
        };
        assert!(
            (SETTLEMENT_ROUTER_POLICY.canonical)(&invalid_router.into_custom_parameter()).is_err()
        );
        let malformed =
            CustomParameter::new(ContentPolicyV1::parameter_id(), Json::new("not a policy"));
        assert!((CONTENT_POLICY.canonical)(&malformed).is_err());
        // A value of the other parameter is not this policy.
        assert!(
            (CONTENT_POLICY.canonical)(
                &SettlementRouterPolicyV1::protocol_default().into_custom_parameter()
            )
            .is_err()
        );
        // Malformed initial bytes are refused, never installed.
        assert!((CONTENT_POLICY.install)(&[0, 1, 2]).is_err());

        let quotas =
            (NEXUS_QUOTA_POLICY.install)(&(NEXUS_QUOTA_POLICY.initial)().unwrap()).unwrap();
        assert_eq!(
            quotas,
            NexusQuotaPolicyV1::protocol_default().into_custom_parameter()
        );
        let invalid_quotas = NexusQuotaPolicyV1 {
            max_da_ingest_bytes_per_account: 0,
            ..NexusQuotaPolicyV1::protocol_default()
        };
        assert!((NEXUS_QUOTA_POLICY.canonical)(&invalid_quotas.into_custom_parameter()).is_err());
        assert!(
            (NEXUS_QUOTA_POLICY.canonical)(
                &ContentPolicyV1::protocol_default().into_custom_parameter()
            )
            .is_err()
        );
        assert!((NEXUS_QUOTA_POLICY.install)(&[0, 1, 2]).is_err());
    }

    #[test]
    fn the_draft_activates_every_parameter_from_the_baseline_once() {
        let catalog = release_catalog().expect("release catalog");
        let published: BTreeMap<DefinitionId, DefinitionDescriptorV1> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let change = catalog
            .parameter_activation(&catalog.baseline, &published, network(), 9, [3; 32])
            .expect("draft");
        assert_eq!(change.effective_height, 9);
        assert_eq!(change.publish.len(), PARAMETERS.len());
        assert_eq!(change.transitions.len(), PARAMETERS.len());
        assert!(change.transitions.iter().all(|transition| matches!(
            transition,
            SelectionTransitionV1::Activate(id) if catalog.parameter_addition(id).is_some()
        )));
        let applied = change
            .apply(&catalog.baseline, &published)
            .expect("the draft applies to the baseline");
        for id in catalog.parameter_definitions() {
            assert!(applied.selection.active.binary_search(&id).is_ok());
        }
        // Nothing else changes: no storage, binding, services or restriction.
        assert_eq!(
            applied.selection.installed_storage,
            catalog.baseline.installed_storage
        );
        assert_eq!(applied.selection.bindings, catalog.baseline.bindings);
        assert_eq!(
            applied.selection.services_profile,
            catalog.baseline.services_profile
        );
        let mut after = published.clone();
        after.extend(applied.published);
        assert!(
            catalog
                .parameter_activation(&applied.selection, &after, network(), 10, [3; 32])
                .is_err(),
            "a root that recognizes every parameter has nothing to activate"
        );
    }

    #[test]
    fn the_draft_activates_only_the_parameters_a_root_does_not_recognize() {
        // A root that activated the content and router policies before this release added the
        // quota policy drafts the quota policy alone.
        let catalog = release_catalog().expect("release catalog");
        let mut published: BTreeMap<DefinitionId, DefinitionDescriptorV1> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let mut current = catalog.baseline.clone();
        for addition in [CONTENT_POLICY, SETTLEMENT_ROUTER_POLICY] {
            let id = addition.definition_id().expect("definition");
            published.insert(id, catalog.definitions[&id].clone());
            current.active.push(id);
        }
        current.active = sorted(current.active);
        let quotas = NEXUS_QUOTA_POLICY.definition_id().expect("definition");
        let change = catalog
            .parameter_activation(&current, &published, network(), 9, [3; 32])
            .expect("draft");
        assert_eq!(
            change.transitions,
            [SelectionTransitionV1::Activate(quotas)]
        );
        assert_eq!(change.publish, [catalog.definitions[&quotas].clone()]);
        let applied = change
            .apply(&current, &published)
            .expect("the draft applies to a root that recognizes two parameters");
        for id in catalog.parameter_definitions() {
            assert!(applied.selection.active.binary_search(&id).is_ok());
        }
    }

    #[test]
    fn the_draft_applies_to_the_lifecycle_successor_too() {
        let catalog = release_catalog().expect("release catalog");
        let published: BTreeMap<DefinitionId, DefinitionDescriptorV1> = catalog
            .definitions
            .iter()
            .filter(|(id, _)| catalog.parameter_addition(id).is_none())
            .map(|(id, descriptor)| (*id, descriptor.clone()))
            .collect();
        let change = catalog
            .parameter_activation(&catalog.lifecycle, &published, network(), 4, [5; 32])
            .expect("draft over the lifecycle successor");
        let applied = change
            .apply(&catalog.lifecycle, &published)
            .expect("applies");
        assert_eq!(
            applied.selection.services_profile,
            catalog.lifecycle.services_profile
        );
    }
}
