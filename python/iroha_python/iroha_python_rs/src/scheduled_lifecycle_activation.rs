//! Authenticate native runtime anchors without enumerating unrelated World entries.
//!
//! The selected service commits its execution and final selection IDs at a
//! guest-unforgeable ordinary-witness key. Each bounded sparse branch joins the
//! same certified execution result as the complete block's lifecycle coverage.
//! Neither a V2 tag nor an algebraic remainder of the World LtHash is authority.

use super::*;
use iroha_data_model::{
    runtime::{
        TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
        definition::{DefinitionDescriptorV1, ImplementationCodeV1, RootSelectionV1, SelectionId},
    },
    sumeragi_finality::{VerifiedSumeragiBlock, lifecycle_runtime::LifecycleRuntimeProofV1},
};

#[cfg(test)]
use iroha_data_model::runtime::definition::{ServiceEntryV1, ServicesProfileDescriptorV1};

const MAX_ACTIVATION_BYTES: usize = 32 * 1024 * 1024;

// Exact portable identities emitted from the compiled merged release catalog.
// No Core dependency is linked into the Python extension's production verifier.
include!("scheduled_lifecycle_supported_pairs.rs");

#[derive(norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct ActivationWitness {
    runtime_proof: LifecycleRuntimeProofV1,
    continuation_proofs: Vec<LifecycleRuntimeProofV1>,
    selection: RootSelectionV1,
    services: DefinitionDescriptorV1,
    implementation: DefinitionDescriptorV1,
}

impl ActivationWitness {
    pub(super) fn parse(input: Option<&str>) -> PyResult<Option<Self>> {
        input
            .map(|input| {
                if input.is_empty() || input.len() > MAX_ACTIVATION_BYTES {
                    return Err(refusal("lifecycle activation witness exceeds its bounds"));
                }
                json::preflight_slice(
                    input.as_bytes(),
                    json::JsonPreflightLimits::new(
                        MAX_ACTIVATION_BYTES,
                        1_200_000,
                        4096,
                        4096,
                        MAX_ACTIVATION_BYTES,
                        131_072,
                        400_000,
                        800_000,
                        1_200_000,
                        48,
                    ),
                )
                .map_err(refusal)?;
                let witness: Self = json::from_json(input).map_err(refusal)?;
                if witness.continuation_proofs.len() >= MAX_PROOFS
                    || std::iter::once(&witness.runtime_proof)
                        .chain(&witness.continuation_proofs)
                        .any(|proof| proof.siblings.len() > 256)
                {
                    return Err(refusal(
                        "lifecycle activation branch count exceeds its bounds",
                    ));
                }
                Ok(witness)
            })
            .transpose()
    }

    pub(super) fn require_continuation_count(&self, count: usize) -> PyResult<()> {
        if count >= MAX_PROOFS || self.continuation_proofs.len() != count {
            return Err(refusal(
                "lifecycle activation needs exactly one native branch per continuation block",
            ));
        }
        Ok(())
    }

    /// Hash the shared typed originals once, before the bounded per-cut branch walk.
    pub(super) fn selected_identity(&self) -> PyResult<SelectionId> {
        let bytes = norito::canonical_frame_len(&self.selection)
            .map_err(refusal)?
            .checked_add(norito::canonical_frame_len(&self.services).map_err(refusal)?)
            .and_then(|n| n.checked_add(norito::canonical_frame_len(&self.implementation).ok()?));
        if !bytes.is_some_and(|n| n <= MAX_ACTIVATION_BYTES) {
            return Err(refusal(
                "lifecycle activation originals exceed their hashing bound",
            ));
        }
        let services_id = self.services.id().map_err(refusal)?;
        let implementation_id = self.implementation.id().map_err(refusal)?;
        let DefinitionDescriptorV1::ServicesProfile(services) = &self.services else {
            return Err(refusal("activation witness has no services profile"));
        };
        let DefinitionDescriptorV1::Implementation(implementation) = &self.implementation else {
            return Err(refusal("activation witness has no native implementation"));
        };
        // A service name or closure alone cannot qualify a native implementation.
        // Accept only complete services/implementation pairs compiled into this SDK;
        // the certified native branch below still authenticates the full selection.
        let closure: [u8; 32] = Hash::new(TRIGGER_LIFECYCLE_SERVICE_CLOSURE).into();
        let mut lifecycle = services
            .services
            .iter()
            .filter(|service| service.name == "trigger_lifecycle");
        let supported_lifecycle = lifecycle
            .next()
            .is_some_and(|service| service.reference_closure == closure)
            && lifecycle.next().is_none();
        if self.selection.services_profile != services_id
            || implementation.services_profile != services_id
            || !self.selection.active.contains(&implementation_id)
            || self.selection.bindings.is_empty()
            || self
                .selection
                .bindings
                .iter()
                .any(|binding| binding.implementation != implementation_id)
            || !supported_lifecycle
            || !matches!(&implementation.code, ImplementationCodeV1::Native { .. })
            || !SUPPORTED_LIFECYCLE_PAIRS.contains(&(services_id.0, implementation_id.0))
        {
            return Err(refusal(
                "registration did not select the supported lifecycle runtime",
            ));
        }
        self.selection.id().map_err(refusal)
    }

    pub(super) fn authenticate(
        &self,
        block: &VerifiedSumeragiBlock,
        offset: usize,
        network: NetworkId,
        selection: SelectionId,
    ) -> PyResult<()> {
        let proof = if offset == 0 {
            &self.runtime_proof
        } else {
            self.continuation_proofs
                .get(offset - 1)
                .ok_or_else(|| refusal("lifecycle continuation native branch is missing"))?
        };
        if proof.height != block.height() {
            return Err(refusal("lifecycle runtime height differs at certified cut"));
        }
        if proof.execution_selection != selection || proof.post_selection != selection {
            return Err(refusal(
                "lifecycle runtime selection differs at certified cut",
            ));
        }
        if !proof.verify(
            network,
            block.height(),
            selection,
            block.execution().ordinary_writes_root,
        ) {
            return Err(refusal("lifecycle runtime root differs at certified cut"));
        }
        Ok(())
    }
}

/// Baseline-derived reference profile for the catalog identity regression.
/// Both standalone and composed profiles additionally require their exact supported pair.
#[cfg(test)]
fn supported_services() -> DefinitionDescriptorV1 {
    let mut services = [
        "authorization",
        "events",
        "fees",
        "hooks",
        "metering",
        "queries",
        "recursion",
    ]
    .into_iter()
    .map(|name| ServiceEntryV1 {
        name: name.into(),
        reference_closure: Hash::new_from_chunks(&[
            b"iroha:runtime:baseline:v1\0",
            b"service",
            name.as_bytes(),
        ])
        .into(),
    })
    .collect::<Vec<_>>();
    services.push(ServiceEntryV1 {
        name: "trigger_lifecycle".into(),
        reference_closure: Hash::new(TRIGGER_LIFECYCLE_SERVICE_CLOSURE).into(),
    });
    DefinitionDescriptorV1::ServicesProfile(ServicesProfileDescriptorV1 { services })
}

/// Derive both actual compiled compositions with the production current-selection builders.
/// Publication maps contain only the definitions available at each predecessor.
#[cfg(test)]
pub(super) fn merged_selections_for_testing() -> [RootSelectionV1; 2] {
    use iroha_core::runtime::ironwood::{
        activation_change_set_for_selection, service_change_set_for_selection,
    };
    use std::collections::BTreeMap;
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"SDK merged lifecycle selection"),
    ));
    let baseline_published: BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    [true, false].map(|time_first| {
        let mut selection = catalog.baseline.clone();
        let mut published = baseline_published.clone();
        for (offset, is_time) in [time_first, !time_first].into_iter().enumerate() {
            let height = 10 + offset as u64;
            let change = if is_time {
                service_change_set_for_selection(
                    network,
                    height,
                    [0x71; 32],
                    &selection,
                    &catalog.lifecycle,
                    &published,
                )
                .unwrap()
            } else {
                activation_change_set_for_selection(
                    network, height, [0x72; 32], &selection, &published,
                )
                .unwrap()
            };
            let applied = change.apply(&selection, &published).unwrap();
            published.extend(applied.published);
            selection = applied.selection;
            selection.check_closure(&published).unwrap();
            assert!(catalog.unsupported(&selection).is_none());
        }
        selection
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witness_for_selection(selection: RootSelectionV1) -> ActivationWitness {
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let services = catalog.definitions[&selection.services_profile].clone();
        let implementation = catalog.definitions[&selection.bindings[0].implementation].clone();
        let selected = selection.id().unwrap();
        ActivationWitness {
            runtime_proof: LifecycleRuntimeProofV1 {
                height: 3,
                execution_selection: selected,
                post_selection: selected,
                siblings: vec![
                    iroha_data_model::sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofStepV1 {
                        level: 255,
                        hash: Hash::new(b"composed cut ordinary writes"),
                    },
                ],
            },
            continuation_proofs: Vec::new(),
            selection,
            services,
            implementation,
        }
    }

    fn rebind(witness: &mut ActivationWitness) {
        let original = witness.selection.bindings[0].implementation;
        witness.selection.services_profile = witness.services.id().unwrap();
        let DefinitionDescriptorV1::Implementation(bundle) = &mut witness.implementation else {
            unreachable!()
        };
        bundle.services_profile = witness.selection.services_profile;
        let replacement = witness.implementation.id().unwrap();
        for active in &mut witness.selection.active {
            if *active == original {
                *active = replacement;
            }
        }
        witness.selection.active.sort_unstable();
        for binding in &mut witness.selection.bindings {
            assert_eq!(binding.implementation, original);
            binding.implementation = replacement;
        }
        let selected = witness.selection.id().unwrap();
        witness.runtime_proof.execution_selection = selected;
        witness.runtime_proof.post_selection = selected;
    }

    #[test]
    fn lifecycle_activation_recognizes_composed_native_service_and_binds_its_exact_selection() {
        pyo3::Python::initialize();
        let witness = witness_for_selection(merged_selections_for_testing()[0].clone());
        let selected = witness.selected_identity().unwrap();
        assert_eq!(selected, witness.selection.id().unwrap());
        assert_ne!(
            selected,
            iroha_core::runtime::release_catalog().unwrap().lifecycle_id
        );
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"composed lifecycle network"),
        ));
        // This is a branch-verifier unit fixture, not a manufactured certificate
        // or a claim that the composition was governed into a live network.
        let proof = &witness.runtime_proof;
        let root = proof.computed_root(network).unwrap();
        assert!(proof.verify(network, proof.height, selected, root));
        assert!(!proof.verify(
            network,
            proof.height,
            selected,
            Hash::new(b"different root")
        ));
        assert!(!proof.verify(network, proof.height, SelectionId([7; 32]), root));
    }

    #[test]
    fn lifecycle_activation_composition_refuses_arbitrary_altered_and_cross_paired_descriptors() {
        pyo3::Python::initialize();
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let combined = merged_selections_for_testing()[0].clone();
        for kind in 0..4 {
            let mut witness = witness_for_selection(combined.clone());
            let DefinitionDescriptorV1::ServicesProfile(profile) = &mut witness.services else {
                unreachable!()
            };
            match kind {
                0 => {
                    profile.services[0].reference_closure =
                        Hash::new(b"arbitrary authorization").into()
                }
                1 => {
                    profile
                        .services
                        .iter_mut()
                        .find(|service| service.name == "trigger_lifecycle")
                        .unwrap()
                        .reference_closure = Hash::new(b"changed lifecycle semantics").into()
                }
                2 => {
                    let DefinitionDescriptorV1::Implementation(bundle) =
                        &mut witness.implementation
                    else {
                        unreachable!()
                    };
                    bundle.code = ImplementationCodeV1::Native {
                        module: "iroha.lifecycle.composition.test.v1".into(),
                        reference_closure: Hash::new(b"arbitrary native implementation").into(),
                    };
                }
                3 => {
                    // Two supported originals do not authorize a newly hashed hybrid.
                    witness.implementation =
                        catalog.definitions[&catalog.lifecycle.bindings[0].implementation].clone();
                }
                _ => unreachable!(),
            }
            rebind(&mut witness);
            let error = witness.selected_identity().unwrap_err();
            assert!(
                error.to_string().contains("supported lifecycle runtime"),
                "{error}"
            );
        }
        // Also refuse a literal cross-pair with otherwise exact compiled originals.
        let mut crossed = witness_for_selection(combined.clone());
        crossed.implementation =
            catalog.definitions[&catalog.lifecycle.bindings[0].implementation].clone();
        assert!(crossed.selected_identity().is_err());

        let mut duplicate = witness_for_selection(combined);
        let DefinitionDescriptorV1::ServicesProfile(profile) = &mut duplicate.services else {
            unreachable!()
        };
        let mut entry = profile
            .services
            .iter()
            .find(|service| service.name == "trigger_lifecycle")
            .unwrap()
            .clone();
        entry.reference_closure = Hash::new(b"conflicting duplicate").into();
        profile.services.push(entry);
        let error = duplicate.selected_identity().unwrap_err();
        assert!(error.to_string().contains("services"), "{error}");
    }

    #[test]
    fn lifecycle_supported_pairs_match_actual_catalog_and_both_activation_orders() {
        pyo3::Python::initialize();
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let [time_then_ironwood, ironwood_then_time] = merged_selections_for_testing();
        let pair = |selection: &RootSelectionV1| {
            (
                selection.services_profile.0,
                selection.bindings[0].implementation.0,
            )
        };
        assert_eq!(
            SUPPORTED_LIFECYCLE_PAIRS,
            [pair(&catalog.lifecycle), pair(&time_then_ironwood)]
        );
        assert_ne!(SUPPORTED_LIFECYCLE_PAIRS[0], SUPPORTED_LIFECYCLE_PAIRS[1]);
        assert_eq!(time_then_ironwood, ironwood_then_time);
        for selection in [
            catalog.lifecycle.clone(),
            time_then_ironwood,
            ironwood_then_time,
        ] {
            selection.check_closure(&catalog.definitions).unwrap();
            assert!(catalog.unsupported(&selection).is_none());
            let witness = witness_for_selection(selection);
            assert_eq!(
                witness.selected_identity().unwrap(),
                witness.selection.id().unwrap()
            );
        }
    }

    #[test]
    fn lifecycle_supported_services_are_exactly_the_native_successor() {
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        println!(
            "M4_WIRE_SCHEMA_HASH={}",
            hex::encode(iroha_core::release_identity::wire_schema_hash())
        );
        println!(
            "M4_IVM_ABI_HASH={}",
            hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
        );
        assert_eq!(
            supported_services(),
            catalog.definitions[&catalog.lifecycle.services_profile]
        );
        let DefinitionDescriptorV1::ServicesProfile(mut altered) = supported_services() else {
            unreachable!()
        };
        altered.services[0].reference_closure = Hash::new(b"changed authorization").into();
        assert_ne!(
            DefinitionDescriptorV1::ServicesProfile(altered),
            supported_services()
        );
    }

    #[test]
    fn lifecycle_activation_json_preflight_rejects_deep_and_long_allocating_values() {
        pyo3::Python::initialize();
        assert!(
            ActivationWitness::parse(Some(&format!("{}0{}", "[".repeat(49), "]".repeat(49))))
                .is_err()
        );
        assert!(ActivationWitness::parse(Some(&format!("\"{}\"", "x".repeat(4097)))).is_err());
        assert!(ActivationWitness::parse(Some("{}")).is_err());
    }
    #[test]
    fn lifecycle_activation_accepts_thousands_of_selective_cuts_with_fixed_world_cost() {
        pyo3::Python::initialize();
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let selection = catalog.lifecycle.clone();
        let selected = selection.id().unwrap();
        let implementation = catalog.definitions[&selection.bindings[0].implementation].clone();
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"large World lineage"),
        ));
        let mut runtime_proof = LifecycleRuntimeProofV1 {
            height: 2, execution_selection: selected, post_selection: selected,
            siblings: vec![iroha_data_model::sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofStepV1 {
                level: 255, hash: Hash::new(b"other per-block write subtree"),
            }],
        };
        let mut continuations = Vec::new();
        for height in 3..=4097 {
            let mut proof = runtime_proof.clone();
            proof.height = height;
            continuations.push(proof);
        }
        let services = supported_services();
        let json = json::to_json(&norito::json!({
            "runtime_proof": runtime_proof,
            "continuation_proofs": continuations,
            "selection": selection,
            "services": services,
            "implementation": implementation,
        }))
        .unwrap();
        println!("M4_ACTIVATION_WITNESS_4096_BYTES={}", json.len());
        assert!(json.len() < 2 * 1024 * 1024, "{}", json.len());
        let witness = ActivationWitness::parse(Some(&json)).unwrap().unwrap();
        witness.require_continuation_count(4095).unwrap();
        assert_eq!(witness.selected_identity().unwrap(), selected);
        assert!(witness.require_continuation_count(4094).is_err());
        assert!(witness.require_continuation_count(4096).is_err());
        for proof in std::iter::once(&witness.runtime_proof).chain(&witness.continuation_proofs) {
            let root = proof.computed_root(network).unwrap();
            assert!(proof.verify(network, proof.height, selected, root));
        }
        // A forged branch remains untrusted regardless of any claimed World size.
        runtime_proof.siblings[0].hash = Hash::new(b"forged subtree");
        assert_ne!(
            runtime_proof.computed_root(network),
            witness.runtime_proof.computed_root(network)
        );
    }
}
