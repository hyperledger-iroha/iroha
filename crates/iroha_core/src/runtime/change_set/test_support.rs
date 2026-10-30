//! Change sets and notice policies for tests across the crate.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    isi::runtime_upgrade::AttestRuntimeSupportV1,
    parameter::{
        Parameter, runtime_provenance_policy::RuntimeProvenancePolicyV1,
        runtime_upgrade_policy::RuntimeUpgradePolicyV1,
    },
    runtime::{
        definition::{
            BindingV1, DefinitionDescriptorV1, EntrypointV1, ImplementationCodeV1,
            ImplementationDescriptorV1, InterfaceDescriptorV1, RestrictionEntryV1, SchemaV1,
            SelectionId,
        },
        lifecycle::{
            ChangeSetEndorsementV1, ChangeSetId, ChangeSetV1, RestrictionKindV1, RootScopeV1,
            RuntimeNoticeV1, RuntimeUpgradeActionV1, SelectionTransitionV1, SupportAttestationV1,
        },
    },
};

use crate::runtime::release_catalog;

/// Wire identity of the instruction [`add_instruction`] adds.
pub(crate) const ADDED_WIRE_ID: &str = "iroha.test.added.v1";

/// A change set of `network`'s universal root, effective at `effective_height`, that adds one
/// instruction answered by a new native implementation to the baseline selection.
pub(crate) fn add_instruction(network: NetworkId, effective_height: u64) -> ChangeSetV1 {
    let catalog = release_catalog().expect("release catalog");
    let predecessor = catalog.baseline.clone();
    let bundle = predecessor.bindings[0].implementation;
    let Some(DefinitionDescriptorV1::Implementation(template)) = catalog.definitions.get(&bundle)
    else {
        panic!("the baseline binds a native bundle");
    };
    let interface = DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
        wire_id: ADDED_WIRE_ID.into(),
        payload: SchemaV1::Codec("norito:iroha_test::Added@0.0.0".into()),
        rejection_codes: Vec::new(),
        events: Vec::new(),
        metering_class: "native.test".into(),
        contract_statement: [0x31; 32],
    });
    let interface_id = interface.id().expect("interface identity");
    let implementation = DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
        entrypoint: EntrypointV1::Instructions(vec![interface_id]),
        code: ImplementationCodeV1::Native {
            module: "iroha.test.added".into(),
            reference_closure: [0x32; 32],
        },
        reads: Vec::new(),
        writes: Vec::new(),
        scans: Vec::new(),
        events: Vec::new(),
        permissions: Vec::new(),
        parameters: Vec::new(),
        invokes: Vec::new(),
        host_profile: None,
        services_profile: template.services_profile,
        limits: template.limits,
    });
    let implementation_id = implementation.id().expect("implementation identity");
    let binding = BindingV1 {
        wire_id: ADDED_WIRE_ID.into(),
        interface: interface_id,
        implementation: implementation_id,
    };
    let mut successor = predecessor.clone();
    successor.active.extend([interface_id, implementation_id]);
    successor.active.sort();
    successor.bindings.push(binding.clone());
    successor
        .bindings
        .sort_by(|left, right| left.wire_id.cmp(&right.wire_id));
    let mut publish = vec![
        (interface_id, interface),
        (implementation_id, implementation),
    ];
    publish.sort_by_key(|(id, _)| *id);
    ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: predecessor.id().expect("baseline identity"),
        successor: successor.id().expect("successor identity"),
        effective_height,
        publish: publish
            .into_iter()
            .map(|(_, descriptor)| descriptor)
            .collect(),
        transitions: vec![
            SelectionTransitionV1::Activate(interface_id),
            SelectionTransitionV1::Activate(implementation_id),
            SelectionTransitionV1::Bind(binding),
        ],
        qualification: [0x33; 32],
    }
}

/// A change set of `network`'s universal root, effective at `effective_height`, that only
/// deprecates the baseline's native bundle: every definition stays one this release implements.
pub(crate) fn deprecate_baseline_bundle(network: NetworkId, effective_height: u64) -> ChangeSetV1 {
    let predecessor = release_catalog().expect("release catalog").baseline.clone();
    let bundle = predecessor.bindings[0].implementation;
    let mut successor = predecessor.clone();
    successor.restrictions.push(RestrictionEntryV1 {
        definition: bundle,
        restriction: RestrictionKindV1::Deprecated.at(effective_height),
    });
    successor.restrictions.sort_by_key(|entry| entry.definition);
    ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: predecessor.id().expect("baseline identity"),
        successor: successor.id().expect("successor identity"),
        effective_height,
        publish: Vec::new(),
        transitions: vec![SelectionTransitionV1::Restrict {
            definition: bundle,
            restriction: RestrictionKindV1::Deprecated,
        }],
        qualification: [0x34; 32],
    }
}

/// The governance action that schedules [`add_instruction`].
pub(crate) fn schedule(network: NetworkId, effective_height: u64) -> RuntimeUpgradeActionV1 {
    RuntimeUpgradeActionV1::Schedule(add_instruction(network, effective_height))
}

/// A policy of `blocks` blocks of notice and no time floor, for both kinds of change.
pub(crate) fn short_policy(blocks: u64) -> RuntimeUpgradePolicyV1 {
    let notice = RuntimeNoticeV1 {
        blocks,
        ms: 0,
        approval_cushion_ms: 0,
    };
    RuntimeUpgradePolicyV1 {
        activation: notice,
        disable: notice,
    }
}

/// Commit `policy` as the universal root's runtime upgrade policy.
pub(crate) fn commit_policy(
    state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    policy: &RuntimeUpgradePolicyV1,
) {
    state_transaction
        .world
        .set_parameter_for_testing(Parameter::Custom(
            policy.to_custom_parameter().expect("valid policy"),
        ))
        .expect("commit the runtime upgrade policy");
}

/// `count` release signers, sorted by public key as a provenance policy lists them.
pub(crate) fn release_signers(count: u8) -> Vec<KeyPair> {
    let mut keys: Vec<KeyPair> = (1..=count)
        .map(|seed| KeyPair::from_seed(vec![0x40 + seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

/// The provenance policy that trusts `signers` and needs `threshold` of them.
pub(crate) fn provenance_policy(signers: &[KeyPair], threshold: u32) -> RuntimeProvenancePolicyV1 {
    RuntimeProvenancePolicyV1 {
        signers: signers.iter().map(|key| key.public_key().clone()).collect(),
        threshold,
    }
}

/// Commit `policy` as the universal root's runtime provenance policy.
pub(crate) fn commit_provenance(
    state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    policy: &RuntimeProvenancePolicyV1,
) {
    state_transaction
        .world
        .set_parameter_for_testing(Parameter::Custom(
            policy
                .to_custom_parameter()
                .expect("valid provenance policy"),
        ))
        .expect("commit the runtime provenance policy");
}

/// `signer`'s endorsement of the change set `action` schedules.
pub(crate) fn endorse(action: &RuntimeUpgradeActionV1, signer: &KeyPair) -> ChangeSetEndorsementV1 {
    let RuntimeUpgradeActionV1::Schedule(change_set) = action else {
        panic!("only a change set to schedule is endorsed");
    };
    ChangeSetEndorsementV1::sign(
        change_set.id().expect("change set identity"),
        signer.public_key().clone(),
        signer.private_key(),
    )
    .expect("endorse the change set")
}

/// A validator's signed support of an arbitrary change set, as the instruction that records it.
pub(crate) fn attest_instruction() -> AttestRuntimeSupportV1 {
    let key = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
    let attestation = SupportAttestationV1 {
        network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0x11; 32],
        ))),
        root: RootScopeV1::Universal,
        change_set: ChangeSetId([1; 32]),
        predecessor: SelectionId([2; 32]),
        successor: SelectionId([3; 32]),
        committee: [4; 32],
        tested_checkpoint: 10,
        valid_from: 11,
        valid_until: 20,
        release: [5; 32],
        validator: key.public_key().clone(),
    }
    .sign(key.private_key())
    .expect("sign attestation");
    AttestRuntimeSupportV1 { attestation }
}
