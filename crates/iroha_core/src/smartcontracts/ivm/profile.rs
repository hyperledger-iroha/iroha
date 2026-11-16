//! Resolution of the IVM execution profile of an artifact, contract or invocation.
//!
//! A profile decides the header layout, instruction set, admission order, fault semantics and
//! metering of an execution. It is resolved here, from authenticated context, before any header
//! byte is parsed:
//!
//! 1. The interface. Every first-release registration and execution interface names the
//!    baseline policy, which runs the frozen baseline engine
//!    ([`ResolvedIvmProfile::interface_default`]).
//! 2. Another named profile. An artifact advertises it by the ABI hash in its fixed header, read
//!    at the layout of that profile's own engine, and it resolves only when the root's selection
//!    activates a host profile with that hash ([`resolve_artifact`]).
//! 3. An authenticated ABI hash of a manifest or deployed binding ([`resolve_abi_hash`]).
//! 4. A prepared object or warm cache entry: its bound profile, accepted only when the root
//!    selects it ([`resolve_selected`]).
//!
//! Resolution never authorizes an invocation. The effective restrictions of the selected
//! definition are rechecked at every invocation (`host_profile`), and cache residency grants
//! nothing. No path decodes an artifact under one engine and then retries under another.

use std::sync::OnceLock;

use iroha_data_model::runtime::definition::{
    DefinitionDescriptorV1, DefinitionId, HostProfileDescriptorV1,
};
use ivm::{IvmEngine, IvmProfileId, SyscallPolicy};
use mv::storage::StorageReadOnly as _;

use crate::state::WorldReadOnly;

/// ABI version every implemented profile declares.
const ABI_VERSION: u16 = 1;

/// One execution profile resolved for a root.
///
/// `definition_id` and `metering_id` are those of the host-profile definition this release
/// publishes for the profile. A root activates the profile by selecting a host-profile
/// definition that carries the same ABI version and ABI hash ([`activates`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedIvmProfile {
    id: IvmProfileId,
    definition_id: DefinitionId,
    metering_id: [u8; 32],
}

impl ResolvedIvmProfile {
    /// The compiled profile of a named host policy.
    ///
    /// This names an implemented profile; it does not establish that any root selects it.
    pub(crate) fn of_policy(policy: SyscallPolicy) -> Self {
        Self::of_id(IvmProfileId::of_policy(policy))
    }

    /// The compiled profile of a cache or VM identity.
    ///
    /// The identity names its engine: C10's own ABI V1 surface
    /// ([`IvmProfileId::C10_ABI_V1`]) is a profile of its own, never the interface profile of the
    /// same policy.
    pub(crate) fn of_id(id: IvmProfileId) -> Self {
        static BASELINE_ABI_V1: OnceLock<ResolvedIvmProfile> = OnceLock::new();
        static C10_ABI_V1: OnceLock<ResolvedIvmProfile> = OnceLock::new();
        static C10_RUNTIME_LIFECYCLE: OnceLock<ResolvedIvmProfile> = OnceLock::new();
        let compiled = match (id.engine(), id.policy()) {
            (IvmEngine::Baseline, _) => &BASELINE_ABI_V1,
            (IvmEngine::C10, SyscallPolicy::AbiV1) => &C10_ABI_V1,
            (IvmEngine::C10, SyscallPolicy::AbiV1RuntimeLifecycle) => &C10_RUNTIME_LIFECYCLE,
        };
        *compiled.get_or_init(|| Self::compile(id))
    }

    /// The profile every first-release registration and execution interface names: contract
    /// code registration, raw IVM execution, trigger registration and the executor upgrade.
    ///
    /// An interface for another engine names its own profile here; existing interfaces keep
    /// this one.
    pub(crate) fn interface_default() -> Self {
        Self::of_policy(SyscallPolicy::AbiV1)
    }

    fn compile(id: IvmProfileId) -> Self {
        let metering_id: [u8; 32] = ivm::gas::schedule_hash_for(id.engine()).into();
        let descriptor = DefinitionDescriptorV1::HostProfile(HostProfileDescriptorV1 {
            abi_version: ABI_VERSION,
            abi_hash: id.abi_hash(),
            metering: metering_id,
        });
        // A host-profile descriptor is three fixed-width fields: it always validates and
        // encodes. The all-zero identity names no definition, so a failure selects nothing.
        let definition_id = descriptor.id().unwrap_or(DefinitionId([0; 32]));
        Self {
            id,
            definition_id,
            metering_id,
        }
    }

    /// Compiled identity bound into cache keys.
    pub(crate) const fn id(&self) -> IvmProfileId {
        self.id
    }

    /// Named host policy.
    pub(crate) const fn policy(&self) -> SyscallPolicy {
        self.id.policy()
    }

    /// Engine that admits and executes the profile.
    pub(crate) const fn engine(&self) -> IvmEngine {
        self.id.engine()
    }

    /// ABI descriptor of the profile, as artifacts and manifests carry it.
    pub(crate) fn abi(&self) -> [u8; 32] {
        self.id.abi_hash()
    }

    /// Host-profile definition this release publishes for the profile.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the selection predicate still matches the ABI hash; the baseline cutover binds roots to this identity"
        )
    )]
    pub(crate) const fn definition_id(&self) -> DefinitionId {
        self.definition_id
    }

    /// Metering schedule the published definition binds.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the selection predicate matches on the ABI hash; the engine prices its own runs"
        )
    )]
    pub(crate) const fn metering_id(&self) -> [u8; 32] {
        self.metering_id
    }

    /// Whether every root may use the profile without selecting a definition for it.
    ///
    /// The interface profile, the frozen baseline, keeps its original admission rule: it is not
    /// looked up in the selection, exactly as the anchor admitted it. C10's own ABI V1 surface
    /// shares the policy but is not implicit: no published definition activates it.
    // TODO: binding the interface profile to its frozen catalog definition (`d50c38a8…`) instead
    // of this implicit case changes admission for a root whose selection drops that definition;
    // it is left to the governed retirement plan of the baseline.
    pub(crate) fn is_implicit(&self) -> bool {
        self.id == IvmProfileId::BASELINE_ABI_V1
    }
}

/// The profile a path with no root in scope applies to artifact bytes: the interface profile
/// (stateless transaction acceptance, stateless overlays, access-set fences derived before a root
/// is chosen). A path with a root resolves with [`resolve_artifact`].
pub(crate) fn interface_profile_id() -> IvmProfileId {
    ResolvedIvmProfile::interface_default().id()
}

/// Whether the published definition `id` is a host profile that activates `profile`.
///
/// The match is on ABI version and ABI hash, as before this resolver existed. The metering
/// hash of the selected definition is not compared.
pub(crate) fn activates(
    world: &impl WorldReadOnly,
    id: &DefinitionId,
    profile: ResolvedIvmProfile,
) -> bool {
    matches!(
        world.runtime_definitions().get(id),
        Some(DefinitionDescriptorV1::HostProfile(descriptor))
            if descriptor.abi_version == ABI_VERSION && descriptor.abi_hash == profile.abi()
    )
}

/// Whether this root may resolve `profile`: the implicit baseline, or a named profile that an
/// active definition of the root's selection activates.
pub(crate) fn is_selected(world: &impl WorldReadOnly, profile: ResolvedIvmProfile) -> bool {
    profile.is_implicit()
        || world
            .runtime_selection()
            .active
            .iter()
            .any(|id| activates(world, id, profile))
}

/// Resolve the bound profile of a prepared object, a warm cache entry or a VM for this root.
///
/// `None` means the root does not select the profile: the object must not be used, whatever
/// cache holds it.
pub(crate) fn resolve_selected(
    world: &impl WorldReadOnly,
    id: IvmProfileId,
) -> Option<ResolvedIvmProfile> {
    let profile = ResolvedIvmProfile::of_id(id);
    is_selected(world, profile).then_some(profile)
}

/// Resolve the profile of artifact bytes submitted through a first-release interface.
///
/// The interface names the baseline policy. Another named profile is resolved only when the
/// artifact advertises its exact ABI hash, read at the fixed-header position of that profile's
/// own engine, and the root selects it. Everything else, including a short or unknown header,
/// resolves to the interface profile, whose parser then authenticates every header field in
/// its own order and refuses a foreign hash. This compares one authenticated hash per selected
/// profile at a fixed position; it parses no header and never retries a decoder.
pub(crate) fn resolve_artifact(world: &impl WorldReadOnly, bytes: &[u8]) -> ResolvedIvmProfile {
    let interface = ResolvedIvmProfile::interface_default();
    IvmProfileId::IMPLEMENTED
        .into_iter()
        .filter(|id| *id != interface.id())
        .map(ResolvedIvmProfile::of_id)
        .find(|named| {
            named.engine().header_layout().advertised_abi_hash(bytes) == Some(&named.abi())
                && is_selected(world, *named)
        })
        .unwrap_or(interface)
}

/// Resolve the profile an authenticated ABI hash names: the hash of a manifest or of a
/// deployed binding.
///
/// `None` for a hash no implemented profile carries and for a profile this root does not
/// select. There is no fallback profile.
pub(crate) fn resolve_abi_hash(
    world: &impl WorldReadOnly,
    hash: &[u8; 32],
) -> Option<ResolvedIvmProfile> {
    IvmProfileId::IMPLEMENTED
        .into_iter()
        .find(|id| id.abi_hash() == *hash)
        .and_then(|id| resolve_selected(world, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMED: SyscallPolicy = SyscallPolicy::AbiV1RuntimeLifecycle;

    fn selected_world() -> crate::state::World {
        let world = crate::state::World::new();
        for module in [
            crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
            crate::runtime::catalog::additions::CONTRACT_LIFECYCLE.module,
        ] {
            crate::runtime::activate_additions_for_testing(&world, module)
                .expect("activate the lifecycle additions");
        }
        world
    }

    fn header(policy: SyscallPolicy) -> Vec<u8> {
        ivm::ProgramMetadata {
            max_cycles: 1,
            ..ivm::ProgramMetadata::default()
        }
        .encode_for_profile(IvmProfileId::of_policy(policy), 0)
    }

    #[test]
    fn interfaces_resolve_the_frozen_baseline_profile() {
        let interface = ResolvedIvmProfile::interface_default();
        assert_eq!(interface.policy(), SyscallPolicy::AbiV1);
        assert_eq!(interface.engine(), IvmEngine::Baseline);
        assert_eq!(interface.id(), IvmProfileId::BASELINE_ABI_V1);
        assert_eq!(
            interface.abi(),
            ivm::syscalls::compute_abi_hash(SyscallPolicy::AbiV1)
        );
        assert_eq!(
            hex::encode(interface.definition_id().0),
            "d50c38a8d0d0a229f54a43f9f7568fd45561774cb58f40668eb703ae79611b99",
            "the interface profile is the installed baseline host profile"
        );
        assert!(interface.is_implicit());
        for id in IvmProfileId::IMPLEMENTED {
            let profile = ResolvedIvmProfile::of_id(id);
            assert_eq!(profile, ResolvedIvmProfile::of_policy(id.policy()));
            assert_eq!(
                profile.metering_id(),
                <[u8; 32]>::from(ivm::gas::schedule_hash_for(id.engine())),
                "each profile is priced by its engine's schedule"
            );
            assert_eq!(profile.is_implicit(), id == IvmProfileId::BASELINE_ABI_V1);
        }
        let c10 = ResolvedIvmProfile::of_id(IvmProfileId::C10_ABI_V1);
        assert_eq!(c10.engine(), IvmEngine::C10);
        assert_ne!(
            c10, interface,
            "C10's own surface is not the interface profile"
        );
        assert!(!c10.is_implicit());
        assert_eq!(
            ResolvedIvmProfile::of_policy(NAMED).engine(),
            IvmEngine::C10
        );
        assert_ne!(
            ResolvedIvmProfile::of_policy(NAMED).definition_id(),
            interface.definition_id()
        );
    }

    #[test]
    fn resolved_definitions_are_the_host_profiles_the_release_publishes() {
        let catalog = crate::runtime::catalog::release_catalog().expect("release catalog");
        let published = crate::runtime::attestation::release_host_profiles(catalog);
        for id in IvmProfileId::IMPLEMENTED {
            let profile = ResolvedIvmProfile::of_id(id);
            assert!(
                published.contains(&profile.definition_id()),
                "the release publishes the definition of {id:?}"
            );
            assert_eq!(
                catalog.definitions.get(&profile.definition_id()),
                Some(&DefinitionDescriptorV1::HostProfile(
                    HostProfileDescriptorV1 {
                        abi_version: 1,
                        abi_hash: profile.abi(),
                        metering: profile.metering_id(),
                    }
                ))
            );
        }
        // The baseline selection activates exactly the interface profile's definition.
        let baseline = crate::state::World::new();
        let view = baseline.view();
        let interface = ResolvedIvmProfile::interface_default();
        assert_eq!(
            crate::runtime::attestation::committed_host_profiles(&view),
            vec![interface.definition_id()]
        );
        assert!(activates(&view, &interface.definition_id(), interface));
        let named = ResolvedIvmProfile::of_policy(NAMED);
        assert!(!activates(&view, &interface.definition_id(), named));
        let selected = selected_world();
        let view = selected.view();
        assert!(
            view.runtime_selection()
                .active
                .contains(&named.definition_id())
        );
        assert!(activates(&view, &named.definition_id(), named));
        assert!(!activates(&view, &named.definition_id(), interface));
    }

    #[test]
    fn artifact_resolution_follows_the_interface_the_advertised_hash_and_the_selection() {
        let baseline = crate::state::World::new();
        let selected = selected_world();
        let interface = ResolvedIvmProfile::interface_default();
        let named = ResolvedIvmProfile::of_policy(NAMED);
        let baseline_header = header(SyscallPolicy::AbiV1);
        let named_header = header(NAMED);
        let mut unknown = header(SyscallPolicy::AbiV1);
        unknown[ivm_abi::baseline::HEADER_ABI_HASH_OFFSET] ^= 1;
        let mut code_after_header = header(NAMED);
        code_after_header.extend_from_slice(&[0xFF; 9]);
        let cases: [(&[u8], ResolvedIvmProfile, ResolvedIvmProfile); 8] = [
            (baseline_header.as_slice(), interface, interface),
            // An unselected named profile resolves to the interface profile, whose own header
            // check then refuses the artifact.
            (named_header.as_slice(), interface, named),
            (code_after_header.as_slice(), interface, named),
            (unknown.as_slice(), interface, interface),
            (&named_header[..47], interface, interface),
            // C10's own ABI V1 surface is never selected.
            (
                &ivm::ProgramMetadata::default().encode_for_policy(SyscallPolicy::AbiV1),
                interface,
                interface,
            ),
            (b"short".as_slice(), interface, interface),
            (&[], interface, interface),
        ];
        for (bytes, in_baseline, in_selected) in cases {
            assert_eq!(resolve_artifact(&baseline.view(), bytes), in_baseline);
            assert_eq!(resolve_artifact(&selected.view(), bytes), in_selected);
            for world in [&baseline, &selected] {
                assert_eq!(
                    super::super::artifact_policy(&world.view(), bytes),
                    resolve_artifact(&world.view(), bytes).policy(),
                    "the retained wrapper is this resolver"
                );
            }
        }
    }

    #[test]
    fn hash_and_prepared_resolution_require_the_roots_selection() {
        let baseline = crate::state::World::new();
        let selected = selected_world();
        let interface = ResolvedIvmProfile::interface_default();
        let named = ResolvedIvmProfile::of_policy(NAMED);
        for (world, named_selected) in [(&baseline, false), (&selected, true)] {
            let view = world.view();
            assert_eq!(resolve_abi_hash(&view, &interface.abi()), Some(interface));
            assert_eq!(
                resolve_abi_hash(&view, &named.abi()),
                named_selected.then_some(named)
            );
            assert_eq!(resolve_abi_hash(&view, &[0; 32]), None);
            assert_eq!(resolve_selected(&view, interface.id()), Some(interface));
            assert_eq!(
                resolve_selected(&view, named.id()),
                named_selected.then_some(named)
            );
            assert!(is_selected(&view, interface));
            assert_eq!(is_selected(&view, named), named_selected);
            for profile in [interface, named] {
                assert_eq!(
                    super::super::known_selected_abi(&view, &profile.abi()),
                    resolve_abi_hash(&view, &profile.abi()).is_some()
                );
            }
            assert!(!super::super::known_selected_abi(&view, &[0; 32]));
        }
        // An empty selection still resolves the implicit baseline and nothing else.
        let mut block = baseline.block();
        block.runtime_selection.get_mut().active.clear();
        assert!(is_selected(&block, interface));
        assert!(!is_selected(&block, named));
        assert_eq!(resolve_artifact(&block, &header(NAMED)), interface);
        // C10's own ABI V1 surface resolves in no root.
        let c10 = ResolvedIvmProfile::of_id(IvmProfileId::C10_ABI_V1);
        assert_eq!(resolve_selected(&block, c10.id()), None);
        assert_eq!(resolve_selected(&selected.view(), c10.id()), None);
        assert_eq!(resolve_abi_hash(&selected.view(), &c10.abi()), None);
    }
}
