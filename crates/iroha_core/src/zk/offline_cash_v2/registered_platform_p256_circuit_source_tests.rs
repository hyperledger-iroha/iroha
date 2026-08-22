use super::*;

use halo2_proofs::halo2curves::pasta::{Fp, Fq};

use super::super::{
    registered_platform_p256_statement::{
        registered_platform_p256_source_pair_for_test_v2,
        REGISTERED_PLATFORM_P256_ARTIFACTS_AUTHENTICATED_V2,
        REGISTERED_PLATFORM_P256_BACKEND_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_COMPILED_PROTOCOL_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_CURRENT_HELPER_AUTHENTICATION_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_FRESHNESS_AUTHORITY_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_GUARD_BUNDLE_ADAPTER_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_PRODUCTION_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_READINESS_AVAILABLE_V2,
        REGISTERED_PLATFORM_P256_RELEASE_ELIGIBLE_V2,
        REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2,
    },
    state_terminal_candidate::STATE_TERMINAL_CANDIDATE_ORDER_V2,
};

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("fixture is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("fixture has exactly {N} bytes"))
}

fn exact_statement() -> [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2] {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let prehash =
        decode_hex::<32>("90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC");
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let low_s =
        decode_hex::<32>("0834E36AD29A83BF2BC9385E491D6099C8FDF9D1ED67AA7EA5F51F93782857A9");

    let mut statement = [0_u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    statement[0] = 4;
    statement[1..33].copy_from_slice(&x);
    statement[33..65].copy_from_slice(&y);
    statement[65..97].copy_from_slice(&prehash);
    statement[97..129].copy_from_slice(&r);
    statement[129..].copy_from_slice(&low_s);
    statement
}

fn exact_pair() -> UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
    registered_platform_p256_source_pair_for_test_v2(exact_statement())
}

#[test]
fn exact_eq_then_ep_role_six_pair_transfers_and_retains_typed_provenance() {
    let expected = exact_statement();
    let candidates =
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(exact_pair())
            .expect("exact typed pair constructs opaque candidates");
    let [eq, ep] = candidates.provenance();

    assert_eq!(eq.parity(), OfflineCashHalo2ParityV2::Eq);
    assert_eq!(ep.parity(), OfflineCashHalo2ParityV2::Ep);
    assert_eq!(eq.role(), OfflineCashHalo2CircuitRoleV2::P256Signature);
    assert_eq!(ep.role(), OfflineCashHalo2CircuitRoleV2::P256Signature);
    assert_eq!(eq.statement_bytes().len(), 161);
    assert_eq!(ep.statement_bytes().len(), 161);
    assert_eq!(eq.statement_bytes(), &expected);
    assert_eq!(ep.statement_bytes(), &expected);
}

#[test]
fn swapped_pair_is_rejected_before_candidate_construction() {
    let mut statements = exact_pair();
    statements.swap_statements_for_test_v2();
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(statements),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::EqParityMismatch)
    ));
}

#[test]
fn mismatched_pair_is_rejected_before_candidate_construction() {
    let mut statements = exact_pair();
    statements.xor_ep_statement_byte_for_test_v2(65, 1);
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(statements),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::StatementBytesMismatch)
    ));
}

#[test]
fn identically_changed_prehashes_are_rejected_by_authenticated_context() {
    let mut statements = exact_pair();
    statements.xor_both_statement_bytes_for_test_v2(REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2, 1);
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(statements),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::AuthenticatedContextMismatch)
    ));
}

#[test]
fn zeroed_and_identically_tampered_pairs_are_rejected_before_construction() {
    let mut zeroed = exact_pair();
    zeroed.zero_statements_for_test_v2();
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(zeroed),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::MalformedTypedStatement)
    ));

    let mut tampered = exact_pair();
    tampered.xor_both_statement_bytes_for_test_v2(0, 1);
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(tampered),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::MalformedTypedStatement)
    ));
}

#[test]
fn a_wrong_parity_tag_is_rejected_without_namespace_casts() {
    let mut statements = exact_pair();
    statements.set_eq_parity_for_test_v2(OfflineCashHalo2ParityV2::Ep);
    assert!(matches!(
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(statements),
        Err(RegisteredPlatformP256CircuitSourceErrorV2::EqParityMismatch)
    ));
}

#[test]
fn eq_and_ep_sources_are_one_shot_and_zero_the_second_destination() {
    let statements = exact_pair();
    let statements = statements.statements();
    let mut eq = RegisteredPlatformP256EqSourceV2::from_validated(&statements[0]);
    let mut ep = RegisteredPlatformP256EpSourceV2::from_validated(&statements[1]);
    let mut eq_destination = [0xA5; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    let mut ep_destination = [0x5A; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];

    eq.read_exact_statement(&mut eq_destination)
        .expect("Eq first read");
    ep.read_exact_statement(&mut ep_destination)
        .expect("Ep first read");
    assert_eq!(eq_destination, exact_statement());
    assert_eq!(ep_destination, exact_statement());

    eq_destination.fill(0xA5);
    ep_destination.fill(0x5A);
    assert_eq!(
        eq.read_exact_statement(&mut eq_destination),
        Err(SOURCE_ALREADY_POISONED)
    );
    assert_eq!(
        ep.read_exact_statement(&mut ep_destination),
        Err(SOURCE_ALREADY_POISONED)
    );
    assert_eq!(
        eq_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );
    assert_eq!(
        ep_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );
}

#[test]
fn source_error_and_caught_unwind_remain_permanently_poisoned() {
    let statements = exact_pair();
    let statements = statements.statements();
    let mut error_source = RegisteredPlatformP256EqSourceV2::from_validated(&statements[0]);
    error_source
        .0
        .inject_fault_for_test(RegisteredPlatformP256SourceFaultV2::Error);
    let mut error_destination = [0xA5; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    assert_eq!(
        error_source.read_exact_statement(&mut error_destination),
        Err(SOURCE_INJECTED_ERROR)
    );
    assert_eq!(
        error_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );
    error_destination.fill(0xA5);
    assert_eq!(
        error_source.read_exact_statement(&mut error_destination),
        Err(SOURCE_ALREADY_POISONED)
    );
    assert_eq!(
        error_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );

    let mut panic_source = RegisteredPlatformP256EpSourceV2::from_validated(&statements[1]);
    panic_source
        .0
        .inject_fault_for_test(RegisteredPlatformP256SourceFaultV2::Panic);
    let mut panic_destination = [0x5A; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = panic_source.read_exact_statement(&mut panic_destination);
    }));
    assert!(unwind.is_err());
    assert_eq!(
        panic_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );
    panic_destination.fill(0x5A);
    assert_eq!(
        panic_source.read_exact_statement(&mut panic_destination),
        Err(SOURCE_ALREADY_POISONED)
    );
    assert_eq!(
        panic_destination,
        [0; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]
    );
}

#[test]
fn v3_row_report_and_every_production_gate_remain_closed() {
    let candidates =
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(exact_pair())
            .expect("structural circuit pair");
    assert!(candidates.eq_fp().row_report_is_closed_for_test());
    assert!(candidates.ep_fq().row_report_is_closed_for_test());
    assert!(REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_STRUCTURAL_CONTRACT_IMPLEMENTED_V2);
    assert!(!REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_CURRENT_HELPER_AUTHENTICATION_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_FRESHNESS_AUTHORITY_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_COMPILED_PROTOCOL_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!REGISTERED_PLATFORM_P256_BACKEND_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_GUARD_BUNDLE_ADAPTER_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_READINESS_AVAILABLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_RELEASE_ELIGIBLE_V2);
    assert!(!REGISTERED_PLATFORM_P256_PRODUCTION_AVAILABLE_V2);
    assert_eq!(STATE_TERMINAL_CANDIDATE_ORDER_V2.len(), 12);
}

#[test]
#[ignore = "builds both complete packed P-256 traces; run only in the serialized Core window"]
fn eq_fp_and_ep_fq_instances_start_with_the_exact_shared_161_byte_statement() {
    let expected = exact_statement();
    let candidates =
        assemble_unverified_registered_platform_p256_circuit_candidates_v2(exact_pair())
            .expect("structural circuit pair");
    let eq_instances = candidates
        .eq_fp()
        .instances_for_test()
        .expect("Eq/Fp instances");
    let ep_instances = candidates
        .ep_fq()
        .instances_for_test()
        .expect("Ep/Fq instances");

    assert!(eq_instances.len() >= expected.len());
    assert!(ep_instances.len() >= expected.len());
    for (index, byte) in expected.into_iter().enumerate() {
        assert_eq!(
            eq_instances[index],
            Fp::from(u64::from(byte)),
            "Eq byte {index}"
        );
        assert_eq!(
            ep_instances[index],
            Fq::from(u64::from(byte)),
            "Ep byte {index}"
        );
    }
}

#[test]
fn source_declaration_privacy_move_only_and_namespace_guards_are_exact() {
    let source = include_str!("registered_platform_p256_circuit_source.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    let v1_parent = include_str!("../offline_cash_v1.rs");
    let nested_v3 = include_str!("../offline_cash_v1/p256_packed_affine_v3.rs");

    assert_eq!(
        parent
            .lines()
            .filter(|line| { line.trim() == "mod registered_platform_p256_circuit_source;" })
            .count(),
        1
    );
    assert!(!parent.contains("pub mod registered_platform_p256_circuit_source"));
    assert!(v1_parent.contains("pub(super) trait P256PackedStatementSourceV3"));
    assert!(v1_parent.contains("pub(super) struct P256PackedAffineEqCircuitCandidateV3("));
    assert!(v1_parent.contains("pub(super) struct P256PackedAffineEpCircuitCandidateV3("));
    assert!(!v1_parent.contains("pub use p256_packed_affine_v3"));
    assert!(nested_v3.contains("pub(super) fn new(sec1_uncompressed:"));
    assert!(!nested_v3.contains("pub(crate) fn new(sec1_uncompressed:"));

    for move_only in [
        "struct RegisteredPlatformP256OneShotSourceV2<'a> {",
        "struct RegisteredPlatformP256EqSourceV2<'a>(",
        "struct RegisteredPlatformP256EpSourceV2<'a>(",
        "struct UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {",
    ] {
        assert!(source.contains(move_only), "missing {move_only}");
    }
    let contextual_candidates = source
        .split_once("struct UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {")
        .and_then(|(_, tail)| {
            tail.split_once("impl UnverifiedRegisteredPlatformP256CircuitCandidatesV2")
        })
        .map(|(fields, _)| fields)
        .expect("contextual candidates remain source-visible");
    assert!(contextual_candidates
        .contains("source_pair: UnverifiedRegisteredPlatformP256StatementSourcePairV2"));
    for forbidden in [
        "impl Clone for RegisteredPlatformP256OneShotSourceV2",
        "impl Copy for RegisteredPlatformP256OneShotSourceV2",
        "impl Clone for RegisteredPlatformP256EqSourceV2",
        "impl Copy for RegisteredPlatformP256EqSourceV2",
        "impl Clone for RegisteredPlatformP256EpSourceV2",
        "impl Copy for RegisteredPlatformP256EpSourceV2",
        "impl Clone for UnverifiedRegisteredPlatformP256CircuitCandidatesV2",
        "impl Copy for UnverifiedRegisteredPlatformP256CircuitCandidatesV2",
        "fn from_bytes(",
        "fn from_parts(",
        "fn new(",
        " as u8",
        "OfflineCashHalo2CircuitRoleV1",
        "OfflineCashHalo2ParityV1",
        "protocol_digest",
        "create_proof",
        "verify_proof",
        "MockProver",
        "pub(crate)",
        "pub fn ",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden source surface: {forbidden}"
        );
    }
    assert!(source.contains("source_pair: UnverifiedRegisteredPlatformP256StatementSourcePairV2"));
    assert!(!source.contains("statements: [UnverifiedRegisteredPlatformP256StatementV2; 2]"));
    assert!(source.contains("validate_statement_pair(source_pair.statements())?;"));
    assert!(source.contains("validate_registered_platform_p256_source_pair_context_v2"));
    assert!(source.contains("if eq.role() != OfflineCashHalo2CircuitRoleV2::P256Signature"));
    assert!(source.contains("if ep.role() != OfflineCashHalo2CircuitRoleV2::P256Signature"));
    assert!(source.contains("if eq.statement_bytes() != ep.statement_bytes()"));
    assert!(source.contains("REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2 == 161"));
}
