//! FX classification must preserve original nested multisig decode deferrals.

use super::*;

#[test]
fn fx_scan_keeps_nested_custom_decode_refusal_through_transaction_and_retries() {
    let (authority, signer) = gen_account_in("fx_scan_refusal");
    let custom = CustomInstruction::new(iroha_primitives::json::Json::new(
        MultisigInstructionBox::Propose(MultisigPropose::new(authority.clone(), vec![], None)),
    ));
    let nested: InstructionBox =
        MultisigPropose::new(authority.clone(), vec![custom.clone().into()], None).into();
    let executable = Executable::Instructions(vec![nested.clone()].into());
    let tx = sample_transaction(&authority, signer.private_key(), vec![nested.clone()]);
    let check = |result: Result<bool, RoutingResolveError>| {
        assert!(
            matches!(result, Err(RoutingResolveError::Deferred(ref reason))
            if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
            "unfinished FX classification became a routing verdict: {result:?}"
        );
    };
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            check(instruction_contains_fx_corridor_settlement(&custom));
            check(instruction_contains_fx_corridor_settlement(&*nested));
            check(executable_contains_fx_corridor_settlement(&executable));
            check(transaction_contains_fx_corridor_settlement(&tx));
        },
    );
    assert!(!instruction_contains_fx_corridor_settlement(&custom).unwrap());
    assert!(!instruction_contains_fx_corridor_settlement(&*nested).unwrap());
    assert!(!executable_contains_fx_corridor_settlement(&executable).unwrap());
    assert!(!transaction_contains_fx_corridor_settlement(&tx).unwrap());
}
