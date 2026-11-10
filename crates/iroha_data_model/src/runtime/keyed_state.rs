//! Immutable identities and change-set rules for adopting already-existing State owners.
//!
//! The foundation kernel can install cells only with an explicit initial value.
//! A separately selected kernel additionally recognizes one exact set of existing
//! storage owners whose contents are imported from native execution during the
//! governed commitment-profile transition. The predecessor selects this rule.

use super::{
    definition::{
        CommitmentProfileDescriptorV1, DefinitionDescriptorV1, DefinitionId, KernelDescriptorV1,
    },
    lifecycle::{ChangeSetV1, SelectionTransitionV1},
};
use std::sync::LazyLock;

/// The governed profile identity used by the runtime upgrade draft and its descriptor.
pub const PROFILE_NAME: &str = "iroha.keyed-state.radix-blake3.v1";

/// Immutable keyed State commitment descriptor. The digest domain and the profile's
/// closure identify the tree, key order, hash framing and witness interpretation.
pub fn profile_descriptor() -> DefinitionDescriptorV1 {
    DefinitionDescriptorV1::CommitmentProfile(CommitmentProfileDescriptorV1 {
        scheme: PROFILE_NAME.into(),
        parameters: blake3::derive_key(
            "iroha keyed State commitment profile v1",
            b"radix-binary-compressed/v1;table-nul-bare-key/msb9;key/1;value/2;leaf/3;branch-counts-split/4;schema-count-root/5",
        ),
    })
}

/// Identity of the commitment definition independently compiled into the native verifier.
pub fn profile_id() -> DefinitionId {
    static PROFILE: LazyLock<DefinitionId> = LazyLock::new(|| {
        profile_descriptor()
            .id()
            .expect("valid keyed State commitment")
    });
    *PROFILE
}

/// A successor kernel with authority to import the indicated complete sorted set of
/// pre-existing State storage. Changing one storage identity yields a different kernel.
pub fn adoption_kernel(storage: &[DefinitionId]) -> DefinitionDescriptorV1 {
    let mut rules = blake3::Hasher::new_derive_key("iroha keyed State adoption kernel v1");
    rules.update(&profile_id().0);
    rules.update(&(storage.len() as u64).to_le_bytes());
    for id in storage {
        rules.update(&id.0);
    }
    DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
        name: "iroha.kernel.adopt-keyed-state.v1".into(),
        reference_closure: *rules.finalize().as_bytes(),
    })
}

/// The original kernel never accepts an uninitialized cell installation. The
/// new kernel imports only its closed set of existing owners in the profile cutover.
pub(super) fn imports_existing_storage(
    change: &ChangeSetV1,
    previous_kernel: DefinitionId,
) -> bool {
    if !change
        .transitions
        .iter()
        .any(|step| *step == SelectionTransitionV1::SetCommitmentProfile(profile_id()))
    {
        return false;
    }
    let mut storage: Vec<_> = change
        .transitions
        .iter()
        .filter_map(|step| match step {
            SelectionTransitionV1::InstallStorage(id) => Some(*id),
            _ => None,
        })
        .collect();
    storage.sort_unstable();
    !storage.is_empty() && adoption_kernel(&storage).id().ok() == Some(previous_kernel)
}
