// A nested contract call stays within the caller's exact execution profile.
mod nested_profile_tests {
    //! Mixed-profile nested calls (the frozen baseline and a C10 profile, in either direction)
    //! are refused deterministically at the authenticated callee-preparation boundary, before
    //! any argument is copied, runtime checked out or host state changed. Same-profile calls are
    //! unchanged.
    use super::*;
    use iroha_data_model::executor::fault::IvmFaultKindV1;

    const CALLER: &str = r"
seiyaku ProfileCaller { view fn main() authorize(anyone) -> int { return 0; } }
";
    const CALLEE: &str = r"
seiyaku ProfileCallee { view fn value() authorize(anyone) -> int { return 42; } }
";

    /// A contract test state whose root selects the runtime lifecycle host profile (C10).
    fn lifecycle_state(authority: &AccountId) -> State {
        let state = contract_test_state(authority);
        for module in [
            crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
            crate::runtime::catalog::additions::CONTRACT_LIFECYCLE.module,
        ] {
            crate::runtime::activate_additions_for_testing(&state.world, module)
                .expect("activate the lifecycle additions");
        }
        state
    }

    /// Install a contract compiled for the runtime lifecycle host profile on the C10 engine.
    fn install_lifecycle_contract(
        state: &State,
        authority: &AccountId,
        source: &str,
        nonce: u64,
    ) -> ContractAddress {
        let compiler = kotodama_lang::compiler::Compiler::new_with_options(
            kotodama_lang::compiler::CompilerOptions {
                mode: kotodama_lang::compiler::CompilerMode::Production,
                host_policy: ivm::SyscallPolicy::AbiV1RuntimeLifecycle,
                ..kotodama_lang::compiler::CompilerOptions::c10()
            },
        );
        let (code, _manifest) = compiler
            .compile_source_with_manifest(source)
            .expect("compile lifecycle-profile contract");
        assert_eq!(
            ivm::IvmProfileId::declared(&code),
            Some(ivm::IvmProfileId::C10_RUNTIME_LIFECYCLE)
        );
        install_contract_artifact_with_interface_and_lifecycle(
            state,
            authority,
            code,
            nonce,
            false,
            |_| {},
        )
    }

    fn artifact_of(state: &State, contract: &ContractAddress) -> (Hash, Vec<u8>) {
        let view = state.view();
        let code_hash = *view
            .world()
            .contract_instances()
            .get(contract)
            .expect("installed contract");
        let artifact = ContractArtifactId::new(contract.dataspace_id().unwrap(), code_hash);
        let bytes = view
            .world()
            .contract_code()
            .get(&artifact)
            .expect("installed code")
            .to_vec();
        (code_hash, bytes)
    }

    /// A prepared-contract cache that already holds `contract` under the profile its root
    /// resolves for it.
    fn warm_cache(state: &State, contract: &ContractAddress) -> PreparedContractCache {
        let cache = PreparedContractCache::default();
        let (code_hash, bytes) = artifact_of(state, contract);
        let view = state.view();
        let prepared = cache
            .get_or_prepare_in_world(view.world(), code_hash, &bytes)
            .expect("warm the callee");
        assert_eq!(
            Some(prepared.profile_id()),
            ivm::IvmProfileId::declared(&bytes)
        );
        cache
    }

    /// The call is refused with the same metered `PermissionDenied`, whether the callee was
    /// prepared before or not, and publishes no State, no return value and no descriptor change.
    fn assert_mixed_call_refused(
        state: &State,
        authority: &AccountId,
        caller: &ContractAddress,
        callee: &ContractAddress,
    ) {
        let mut refusals = Vec::new();
        for cache in [PreparedContractCache::default(), warm_cache(state, callee)] {
            let (result, vm, overlay) = call_contract_syscall_with_prepared_cache(
                state,
                authority,
                caller,
                callee,
                "value",
                &Json::new(()),
                cache,
            );
            let error = result.expect_err("a mixed-profile nested call is refused");
            assert!(
                matches!(error.as_unmetered(), ivm::VMError::PermissionDenied),
                "the refusal is a deterministic permission fault, not a deferral: {error:?}"
            );
            assert!(error.execution_deferral().is_none());
            let ivm::VMError::Metered { gas, .. } = &error else {
                panic!("the boundary charge is metered: {error:?}");
            };
            assert!(*gas > 0, "the boundary is charged");
            // The caller's engine projects the refusal onto its own wire fault.
            assert_eq!(
                vm.engine().fault_kind(&error),
                Some(IvmFaultKindV1::PermissionDenied)
            );
            assert!(overlay.is_empty(), "a refused call publishes no State");
            assert!(
                matches!(vm.load_u64(vm.register(14)), Ok(0)),
                "a refused call writes no return value"
            );
            refusals.push(format!("{error:?}"));
        }
        assert_eq!(
            refusals[0], refusals[1],
            "cache warmth changes neither the refusal nor its charge"
        );
    }

    #[test]
    fn a_baseline_caller_cannot_call_a_c10_callee() {
        let authority = fixture_account("alice");
        let state = lifecycle_state(&authority);
        let caller = install_contract(&state, &authority, CALLER, 0);
        let callee = install_lifecycle_contract(&state, &authority, CALLEE, 1);
        assert_eq!(
            ivm::IvmProfileId::declared(&artifact_of(&state, &caller).1),
            Some(ivm::IvmProfileId::BASELINE_ABI_V1)
        );
        assert_mixed_call_refused(&state, &authority, &caller, &callee);
    }

    #[test]
    fn a_c10_caller_cannot_call_a_baseline_callee() {
        let authority = fixture_account("alice");
        let state = lifecycle_state(&authority);
        let caller = install_lifecycle_contract(&state, &authority, CALLER, 0);
        let callee = install_contract(&state, &authority, CALLEE, 1);
        assert_mixed_call_refused(&state, &authority, &caller, &callee);
    }

    #[test]
    fn same_profile_calls_are_unchanged() {
        let authority = fixture_account("alice");
        let state = lifecycle_state(&authority);
        let pairs = [
            (
                install_contract(&state, &authority, CALLER, 0),
                install_contract(&state, &authority, CALLEE, 1),
            ),
            (
                install_lifecycle_contract(&state, &authority, CALLER, 2),
                install_lifecycle_contract(&state, &authority, CALLEE, 3),
            ),
        ];
        for (caller, callee) in pairs {
            for cache in [
                PreparedContractCache::default(),
                warm_cache(&state, &callee),
            ] {
                let (result, vm, overlay) = call_contract_syscall_with_prepared_cache(
                    &state,
                    &authority,
                    &caller,
                    &callee,
                    "value",
                    &Json::new(()),
                    cache,
                );
                result.expect("a same-profile nested call completes");
                assert!(overlay.is_empty(), "a view call publishes no State");
                assert_eq!(
                    render_nested_result(
                        &vm,
                        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int
                    ),
                    norito::json!("42")
                );
            }
        }
    }
}
