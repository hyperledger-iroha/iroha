// Actual CoreHost contract dispatch: a nested run draws only from its parent's gas.
mod nested_call_gas_tests {
    //! Low-level real nested execution; source admission/State policy are owned separately.
    use super::*;

    #[test]
    fn actual_nested_contract_run_draws_only_from_the_parent_gas() {
        let authority = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller = install_contract(
            &state,
            &authority,
            r"
seiyaku GasCaller { view fn main() authorize(anyone) -> int { return 0; } }
",
            0,
        );
        let target_contract = install_contract(
            &state,
            &authority,
            r"
seiyaku GasCallee { view fn value() authorize(anyone) -> int { return 42; } }
",
            1,
        );
        let run = |gas_limit| {
            dispatch_call_contract_syscall(
                &state,
                &authority,
                &caller,
                &target_contract,
                "value",
                &(Json::new(())),
                gas_limit,
            )
        };
        let (result, parent, overlay, _) = run(1_000_000);
        result.unwrap();
        assert!(overlay.is_empty());
        let consumed = 1_000_000 - parent.remaining_gas();
        assert!(consumed > 0);
        // The parent's gas is the whole allowance of the call tree: exactly the measured
        // amount completes the nested call, and the same work never needs more.
        let (result, parent, overlay, _) = run(consumed);
        result.unwrap();
        assert!(overlay.is_empty());
        assert_eq!(parent.remaining_gas(), 0);
        assert_eq!(
            render_nested_result(
                &parent,
                iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int
            ),
            norito::json!("42")
        );
        // One gas less cannot be borrowed from anywhere else: the tree runs out of gas.
        let (result, parent, overlay, _) = run(consumed - 1);
        assert!(matches!(
            result.unwrap_err().as_unmetered(),
            ivm::VMError::OutOfGas
        ));
        // The missing unit is the last one the tree spends: the parent's own epilogue after the
        // child has returned. The parent therefore completes no result table, whatever its
        // registers hold, and the exhausted invocation publishes no state.
        assert!(parent.call_result_word_count().is_err());
        assert!(overlay.is_empty());
    }

    #[test]
    fn actual_nested_contract_failure_keeps_gas_and_rolls_back_child_effects() {
        let authority = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller = install_contract(
            &state,
            &authority,
            r"
seiyaku GasCaller { view fn main() authorize(anyone) -> int { return 0; } }
",
            0,
        );
        let target_contract = install_contract(
            &state,
            &authority,
            r"
seiyaku GasFailure { permission AssetOps;
  error enum Failure { Refused = 1 }
  state int counter;
  hajimari() { counter = 0; }
  kotoage fn fail_after_write() authorize(AssetOps) -> int {
    counter = 9;
    require(false, Failure::Refused);
    return 0;
  }
}
",
            1,
        );
        grant_asset_ops_to_account(&state, &authority, &(caller.subject_id()));
        let (result, parent, overlay, target) = dispatch_call_contract_syscall(
            &state,
            &authority,
            &caller,
            &target_contract,
            "fail_after_write",
            &(Json::new(())),
            1_000_000,
        );
        assert!(matches!(
            result.unwrap_err().as_unmetered(),
            ivm::VMError::ContractAbort { code: 1, .. }
        ));
        assert!(
            overlay.is_empty(),
            "nested writes roll back on an actual failure"
        );
        assert_eq!(
            parent.register(10),
            target,
            "failed child publishes no return value"
        );
        assert!(
            parent.remaining_gas() < 1_000_000,
            "actual boundary/child work remains charged"
        );
    }
}
