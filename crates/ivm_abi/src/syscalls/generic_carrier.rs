//! Closed ABI V1 surface for descriptor-selected carrier instructions.
//!
//! The exact invocation owner supplies capabilities. A program header, storage identity or
//! catalog descriptor alone never grants storage access through an ordinary execution path.

use super::{AbiSurfaceError, append_abi_field, build_abi_surface_descriptor};

/// Named restricted carrier host; the engine and root selection must select this exact identity.
pub const GENERIC_CARRIER_HOST_PROFILE: &str = "iroha.ivm.generic_carrier.v1";

pub(super) const SYSCALLS: &[u32] = &[
    super::SYSCALL_STATE_GET,
    super::SYSCALL_STATE_SET,
    super::SYSCALL_STATE_DEL,
    super::SYSCALL_INPUT_PUBLISH_TLV,
    super::SYSCALL_ALLOC,
];

pub(super) fn descriptor() -> Result<Vec<u8>, AbiSurfaceError> {
    let mut bytes = b"IVM_ABI_V1_GENERIC_CARRIER\0".to_vec();
    append_abi_field(&mut bytes, GENERIC_CARRIER_HOST_PROFILE.as_bytes())?;
    // Bind the existing C10 instruction, memory, arithmetic, fault and allocation semantics.
    // The following closed interface explicitly replaces its wider host surface, without
    // changing any byte of the installed parent descriptor or its policy dispatch.
    append_abi_field(
        &mut bytes,
        &build_abi_surface_descriptor(crate::SyscallPolicy::AbiV1)?,
    )?;
    append_abi_field(&mut bytes, b"ABI1:C10-header-exact-profile-hash;CNTR-and-debug-sections-forbidden;raw-main-entry-only;no-ordinary-raw-contract-trigger-admission;private-original-descriptor-invocation-capability-required;initial-r10=&NoritoBytes(original-complete-compact-schema-bound-instruction-frame);successful-HALT-requires-r10=0;no-return-record")?;
    append_abi_field(&mut bytes, b"public-methods-only;payload-has-no-authenticated-caller-authority;no-caller-or-account-permission-inference")?;
    for number in SYSCALLS {
        append_abi_field(&mut bytes, &number.to_le_bytes())?;
    }
    append_abi_field(&mut bytes, b"pointer-allowlist=NoritoBytes-only;0x50:GET:r10=reads-list-index:u64,r11=&NoritoBytes(canonical-bare-carrier-key),returns-r10=&NoritoBytes(complete-canonical-CarrierRecordV1)-or-zero-absent;0x51:SET:r10=writes-list-index:u64,r11=key-pointer,r12=value-record-pointer,returns-r10=0;0x52:DEL:r10=writes-list-index:u64,r11=key-pointer,returns-r10=0;0xE0:INPUT_PUBLISH_TLV=unchanged-C10;0xF0:ALLOC=unchanged-C10;all-other-syscalls-refused")?;
    append_abi_field(&mut bytes, b"capability-index=exact-immutable-descriptor-list-position;reads-and-writes-distinct;no-guest-selected-owner-id;read-own-writes-only-with-read-capability;GET-reverse-reads-existing-ordered-effect-queue-before-original-State;writes-retain-original-allocation-and-invocation-owner-in-existing-CoreHost-effects;no-secondary-journal;discard-unpublished-effects-on-fault;revalidate-original-selection-interface-implementation-and-storage-before-ordered-apply;source-State-World-transaction-own-publication-and-rollback;signed-root-typed-effect-debit=one-item-and-64-plus-key-plus-value-frame-bytes-per-write;one-authored-group-and-one-carrier-artifact-group;original-signed-hash-call-proposal-network-dataspace-and-index-bound")?;
    append_abi_field(&mut bytes, b"carrier=Compact-explicit-schema-bound;max-frame=1048576;max-schema=65536;max-nodes=65536;max-key=4096;max-gas=10000000;max-effects=64;reads<=16;writes<=16;no-events-permissions-parameters-scans-nestedcalls-or-obligations;metering=existing-C10-state_path_gas-and-state_value_gas;GET-reserves-remaining-before-reading-value;SET-and-DEL-preflight-before-queue-mutation;key-and-value-schema-from-exact-installed-storage-definition;selected-carrier-kernel-backing-and-keyed-profile-required")?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IvmEngine, IvmProfileId, SyscallPolicy, pointer_abi::PointerType};

    #[test]
    fn carrier_profile_is_exact_separate_and_has_only_its_closed_surface() {
        let policy = SyscallPolicy::AbiV1GenericCarrier;
        let profile = IvmProfileId::C10_GENERIC_CARRIER;
        assert_eq!(profile.policy(), policy);
        assert_eq!(profile.engine(), IvmEngine::C10);
        assert_eq!(IvmProfileId::of_policy(policy), profile);
        assert_eq!(
            IvmProfileId::of_engine_policy(IvmEngine::Baseline, policy),
            None
        );
        assert_eq!(
            IvmProfileId::of_abi_hash(&profile.abi_hash()),
            Some(profile)
        );
        assert_eq!(
            super::super::policy_for_abi_hash(&profile.abi_hash()),
            Some(policy)
        );
        for previous in [
            IvmProfileId::BASELINE_ABI_V1,
            IvmProfileId::C10_ABI_V1,
            IvmProfileId::C10_RUNTIME_LIFECYCLE,
        ] {
            assert_ne!(profile.abi_hash(), previous.abi_hash());
        }
        assert!(SYSCALLS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(super::super::syscalls_for_policy(policy), SYSCALLS);
        for &number in SYSCALLS {
            assert!(super::super::is_generic_program_syscall_allowed(
                policy, number
            ));
        }
        for number in [
            super::super::SYSCALL_STATE_SCAN,
            super::super::SYSCALL_CALL_CONTRACT,
            super::super::SYSCALL_GET_PUBLIC_INPUT,
            super::super::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            u32::MAX,
        ] {
            assert!(!super::super::is_syscall_allowed(policy, number));
        }
        assert_eq!(crate::pointer_abi::policy_pointer_types(policy).len(), 1);
        assert!(crate::pointer_abi::is_type_allowed_for_policy(
            policy,
            PointerType::NoritoBytes
        ));
        assert!(!crate::pointer_abi::is_type_allowed_for_policy(
            policy,
            PointerType::AccountId
        ));
        assert_eq!(
            profile.abi_hash(),
            <[u8; 32]>::from(iroha_crypto::Hash::new(descriptor().unwrap()))
        );
    }
}
