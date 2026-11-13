//! A separately named ABI V1 host contract for kernel-owned contract obligations.

use super::{AbiSurfaceError, append_abi_field, build_abi_surface_descriptor};
use iroha_data_model::{
    isi::runtime_obligation::{CompleteRuntimeObligationV1, RegisterContractRuntimeObligationV1},
    runtime::obligation::{MAX_NESTED_BINDINGS, MAX_OBLIGATION_LIFETIME_MS, MAX_TERMINAL_EFFECTS},
};

/// Exact named ABI V1 host profile; activation is governed by the execution root.
pub const RUNTIME_LIFECYCLE_HOST_PROFILE: &str = "iroha.ivm.runtime_lifecycle.v1";
/// `0xA0/r11` tag for registration by the authenticated executing contract.
pub const SMARTCONTRACT_INSTRUCTION_TAG_REGISTER_RUNTIME_OBLIGATION: u64 = 3;
/// `0xA0/r11` tag for ending the authenticated executing contract's own obligation.
pub const SMARTCONTRACT_INSTRUCTION_TAG_COMPLETE_RUNTIME_OBLIGATION: u64 = 4;

pub(super) fn descriptor() -> Result<Vec<u8>, AbiSurfaceError> {
    let mut bytes = b"IVM_ABI_V1_RUNTIME_LIFECYCLE\0".to_vec();
    // Include the exact baseline, without changing its encoding, hash or operation tags.
    // This named definition changes only the two appended kernel operations and the
    // header's explicit named-profile selection. No new ABI version is introduced.
    append_abi_field(&mut bytes, RUNTIME_LIFECYCLE_HOST_PROFILE.as_bytes())?;
    append_abi_field(
        &mut bytes,
        &build_abi_surface_descriptor(crate::SyscallPolicy::AbiV1)?,
    )?;
    append_abi_field(&mut bytes, b"header:abi_version=1;abi_hash=exact-named-profile;CNTR-required;CNTR.abi_hash=header.abi_hash;manifest.abi_hash=header.abi_hash;generic-and-test-harness-artifacts-refused;unknown-hash-refused-without-fallback")?;
    append_abi_field(&mut bytes, b"0xA0:r10=&NoritoBytes(canonical-InstructionBox);r11=3:RegisterContractRuntimeObligationV1|4:CompleteRuntimeObligationV1;all-other-tags-unchanged;r10=0-on-queued-success;return-is-not-execution-success")?;
    append_abi_field(
        &mut bytes,
        &norito::schema::identity::frame_hash::<RegisterContractRuntimeObligationV1>(),
    )?;
    append_abi_field(
        &mut bytes,
        &norito::schema::identity::frame_hash::<CompleteRuntimeObligationV1>(),
    )?;
    append_abi_field(&mut bytes, b"register:no-caller-supplied-address-or-owner;contract=authenticated-current-address;owner=opaque-current-contract-subject;exact-retained-entrypoint-authorization-and-code/lifecycle-revision-revalidated-before-and-after-apply;caller-cannot-select-another-address-or-owner;no-transaction-authority-fallback;subject-binding-required;Parliament-owned-contracts-use-same-authenticated-entrypoint-policy")?;
    append_abi_field(&mut bytes, b"register-new-work:canonical-root-selection-and-actual-execution-context-must-agree;active-deployed-instance-required;pending-lifecycle-hooks-refused;selected-contract-register-interface-and-runtime-lifecycle-storage-required;ordinary-register-interface-and-direct-terminal-admission-before-mutation;captured-transitive-bindings-with-ordinary-runtime-restrictions;exact-terminal-instructions-once;expiry-and-kind-validated;one-record-per-owner;register-replaces-own-record;capture-retains-execution-definitions-not-deployed-contract-code;complete:owner=opaque-current-subject;normal-registered-completion-handler;ordered-atomic-host-effects-with-original-gas/output/accounting-and-authorization")?;
    append_abi_field(&mut bytes, b"named-profile-restrictions:actual-execution-height;Deprecated-executes;AdmissionClosed-requires-original-captured-Continue-or-Complete;CompletionOnly-requires-original-captured-Complete;Disabled-always-refuses;native-Step-overrides-registered-consent;deferred-bodies-never-borrow-completion;new-registration-or-native-custody-capture-must-admit-every-retained-named-profile-as-new-work-before-mutation-or-funding;conservative-broad-capture-refuses-new-obligations-while-retained-named-profile-closed;baseline-profile-unchanged")?;
    append_abi_field(&mut bytes, &(MAX_TERMINAL_EFFECTS as u64).to_le_bytes())?;
    append_abi_field(&mut bytes, &(MAX_NESTED_BINDINGS as u64).to_le_bytes())?;
    append_abi_field(&mut bytes, &MAX_OBLIGATION_LIFETIME_MS.to_le_bytes())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SyscallPolicy,
        syscalls::{compute_abi_hash, policy_for_abi_hash},
    };

    #[test]
    fn named_lifecycle_profile_has_distinct_exact_identity_with_same_abi_v1_surface() {
        let baseline = build_abi_surface_descriptor(SyscallPolicy::AbiV1).unwrap();
        let lifecycle = descriptor().unwrap();
        assert!(
            lifecycle
                .windows(baseline.len())
                .any(|window| window == baseline)
        );
        let baseline_hash = compute_abi_hash(SyscallPolicy::AbiV1);
        let lifecycle_hash = compute_abi_hash(SyscallPolicy::AbiV1RuntimeLifecycle);
        assert_ne!(baseline_hash, lifecycle_hash);
        assert_eq!(
            policy_for_abi_hash(&baseline_hash),
            Some(SyscallPolicy::AbiV1)
        );
        assert_eq!(
            policy_for_abi_hash(&lifecycle_hash),
            Some(SyscallPolicy::AbiV1RuntimeLifecycle)
        );
        assert_eq!(policy_for_abi_hash(&[0; 32]), None);
        assert_eq!(
            super::super::syscalls_for_policy(SyscallPolicy::AbiV1),
            super::super::syscalls_for_policy(SyscallPolicy::AbiV1RuntimeLifecycle)
        );
    }
}
