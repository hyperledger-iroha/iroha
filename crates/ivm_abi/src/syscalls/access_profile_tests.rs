//! Exact ABI-v1 helper, host-private, generic-program, AXT, and access-profile tests.

use super::*;

#[test]
fn canonical_helper_syscall_maps_direct_aliases() {
    let direct_pairs = [
        (SYSCALL_JSON_GET_JSON_DIRECT, SYSCALL_JSON_GET_JSON),
        (SYSCALL_JSON_GET_NAME_DIRECT, SYSCALL_JSON_GET_NAME),
        (
            SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT,
            SYSCALL_JSON_GET_ACCOUNT_ID,
        ),
        (SYSCALL_JSON_GET_NFT_ID_DIRECT, SYSCALL_JSON_GET_NFT_ID),
        (SYSCALL_JSON_GET_BLOB_HEX_DIRECT, SYSCALL_JSON_GET_BLOB_HEX),
        (SYSCALL_JSON_GET_INT_DIRECT, SYSCALL_JSON_GET_INT),
        (SYSCALL_JSON_GET_DECIMAL_DIRECT, SYSCALL_JSON_GET_DECIMAL),
        (SYSCALL_JSON_GET_QUANTITY_DIRECT, SYSCALL_JSON_GET_QUANTITY),
        (
            SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT,
            SYSCALL_JSON_GET_ASSET_DEFINITION_ID,
        ),
        (SYSCALL_JSON_SET_I64_DIRECT, SYSCALL_JSON_SET_I64),
        (
            SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT,
            SYSCALL_JSON_SET_ACCOUNT_ID,
        ),
        (
            SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT,
            SYSCALL_BUILD_PATH_KEY_NORITO,
        ),
        (SYSCALL_SCHEMA_INFO_DIRECT, SYSCALL_SCHEMA_INFO),
        (SYSCALL_SCHEMA_ENCODE_DIRECT, SYSCALL_SCHEMA_ENCODE),
        (SYSCALL_SCHEMA_DECODE_DIRECT, SYSCALL_SCHEMA_DECODE),
    ];

    for (direct, canonical) in direct_pairs {
        assert_eq!(canonical_helper_syscall(direct), canonical);
        assert_eq!(canonical_helper_syscall(canonical), canonical);
    }

    assert_eq!(
        canonical_helper_syscall(SYSCALL_STATE_GET),
        SYSCALL_STATE_GET
    );
}

#[test]
fn koto_test_syscalls_are_host_private() {
    let private = [
        SYSCALL_KOTO_TEST_ACTOR_ACCOUNT,
        SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY,
        SYSCALL_KOTO_TEST_ACTOR_SIGN,
        SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS,
        SYSCALL_KOTO_TEST_EXPECT_REJECT_AS,
    ];

    for syscall in private {
        assert!(is_koto_test_syscall(syscall));
        assert!(!is_syscall_allowed(crate::SyscallPolicy::AbiV1, syscall));
        assert!(syscall > u8::MAX as u32);
    }
}

#[test]
fn generic_program_syscall_profile_is_sorted_complete_and_fail_closed() {
    assert!(
        GENERIC_PROGRAM_DENIED_SYSCALLS_V1
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "ABI-bound denylist must remain strictly sorted"
    );
    assert_eq!(
        GENERIC_PROGRAM_DENIED_SYSCALLS_V1,
        &[
            SYSCALL_GRANT_CONTRACT_ENTRYPOINT,
            SYSCALL_REVOKE_CONTRACT_ENTRYPOINT,
            SYSCALL_DEACTIVATE_CONTRACT_INSTANCE,
            SYSCALL_REMOVE_SMART_CONTRACT_BYTES,
            SYSCALL_REGISTER_SMART_CONTRACT_CODE,
            SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
            SYSCALL_STATE_GET,
            SYSCALL_STATE_SET,
            SYSCALL_STATE_DEL,
            SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            SYSCALL_CALL_CONTRACT,
            SYSCALL_SYSVAR_CONTRACT_ADDRESS,
            SYSCALL_SYSVAR_ENTRYPOINT,
            SYSCALL_SYSVAR_CONTRACT_SUBJECT,
            SYSCALL_CALL_CONTRACT_QUANTITY2,
            SYSCALL_STATE_KEYS,
            SYSCALL_STATE_HAS,
            SYSCALL_STATE_LEN,
            SYSCALL_STATE_COUNT,
        ]
    );
    for &syscall in GENERIC_PROGRAM_DENIED_SYSCALLS_V1 {
        assert!(is_syscall_allowed(crate::SyscallPolicy::AbiV1, syscall));
        assert!(!is_generic_program_syscall_allowed(
            crate::SyscallPolicy::AbiV1,
            syscall
        ));
    }
    for syscall in [
        SYSCALL_REGISTER_DOMAIN,
        SYSCALL_INT_ADD,
        SYSCALL_SUBSCRIPTION_BILL,
        SYSCALL_SUBSCRIPTION_RECORD_USAGE,
        SYSCALL_AXT_BEGIN,
        SYSCALL_AXT_TOUCH,
        SYSCALL_AXT_COMMIT,
        SYSCALL_VERIFY_DS_PROOF,
        SYSCALL_USE_ASSET_HANDLE,
    ] {
        assert!(is_generic_program_syscall_allowed(
            crate::SyscallPolicy::AbiV1,
            syscall
        ));
    }
    assert!(!is_generic_program_syscall_allowed(
        crate::SyscallPolicy::AbiV1,
        u32::MAX
    ));
}

#[test]
fn axt_syscall_classifier_is_exact() {
    for syscall in [
        SYSCALL_AXT_BEGIN,
        SYSCALL_AXT_TOUCH,
        SYSCALL_AXT_COMMIT,
        SYSCALL_VERIFY_DS_PROOF,
        SYSCALL_USE_ASSET_HANDLE,
    ] {
        assert!(is_axt_syscall(syscall));
    }
    for syscall in [
        SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE,
        SYSCALL_ESCROW_OPEN_OFFER,
        SYSCALL_STATE_GET,
        u32::MAX,
    ] {
        assert!(!is_axt_syscall(syscall));
    }
}

#[test]
fn syscall_access_classification_is_conservative() {
    assert_eq!(syscall_access(SYSCALL_STATE_GET), SyscallAccess::StateRead);
    assert_eq!(syscall_access(SYSCALL_STATE_SET), SyscallAccess::StateWrite);
    assert_eq!(
        syscall_access(SYSCALL_STATE_MAP_KEY_AT),
        SyscallAccess::None
    );
    assert_eq!(
        syscall_access(SYSCALL_STATE_VALUE_ENCODE),
        SyscallAccess::None
    );
    assert_eq!(
        syscall_access(SYSCALL_STATE_VALUE_DECODE),
        SyscallAccess::None
    );
    assert_eq!(
        syscall_access(SYSCALL_CORE_QUERY_GET),
        SyscallAccess::LedgerRead
    );
    assert_eq!(
        syscall_access(SYSCALL_CORE_QUERY_PAGE),
        SyscallAccess::LedgerRead
    );
    assert_eq!(
        syscall_access(SYSCALL_VRF_EPOCH_SEED),
        SyscallAccess::LedgerRead
    );
    assert_eq!(
        syscall_access(SYSCALL_TRANSFER_ASSET_SCOPED),
        SyscallAccess::LedgerWrite
    );
    assert_eq!(
        syscall_access(SYSCALL_CALL_CONTRACT),
        SyscallAccess::Dynamic
    );
    assert_eq!(
        syscall_access(SYSCALL_CALL_CONTRACT_QUANTITY2),
        SyscallAccess::Dynamic
    );
    assert_eq!(syscall_access(SYSCALL_SHA256_HASH), SyscallAccess::None);
    assert_eq!(syscall_access(0x00ff_fffe), SyscallAccess::Dynamic);

    for number in abi_syscall_list() {
        assert!(
            syscall_name(*number).is_some(),
            "ABI syscall 0x{number:06x} lacks a registry name"
        );
    }
}

#[test]
fn gas_text_is_not_part_of_the_abi_surface_hash() {
    let canonical = &syscalls_doc_gen::DOCS[0];
    let changed_gas = SyscallDoc {
        number: canonical.number,
        args: canonical.args,
        ret: canonical.ret,
        gas: "a deliberately different gas schedule",
    };
    let canonical_surface =
        collect_abi_syscall_surface(&[canonical.number], std::slice::from_ref(canonical))
            .expect("single canonical row");
    let changed_surface = collect_abi_syscall_surface(&[canonical.number], &[changed_gas])
        .expect("single altered-gas row");
    assert_eq!(canonical_surface, changed_surface);
}
