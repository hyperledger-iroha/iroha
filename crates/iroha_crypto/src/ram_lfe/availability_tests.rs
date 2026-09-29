//! Production refusal is separate from retained exact-arithmetic diagnostics.

use super::*;

fn diagnostic_policy(backend: RamLfeBackend) -> PolicyCommitment {
    PolicyCommitment {
        backend,
        policy_hash: Hash::new(b"diagnostic-policy"),
        public_parameters: vec![0xff],
    }
}

#[test]
fn production_admission_rejects_both_exact_lift_backends() {
    for backend in [RamLfeBackend::BfvAffineV1, RamLfeBackend::BfvProgrammedV1] {
        assert_eq!(
            backend.require_production_support(),
            Err(RamLfeError::InsecureBfvProfile)
        );
    }
    RamLfeBackend::HkdfSha3_512PrfV1
        .require_production_support()
        .unwrap();
    assert!(
        RamLfeError::InsecureBfvProfile
            .to_string()
            .contains("noiseless public-key equation")
    );
}

#[test]
fn public_evaluators_reject_before_secret_request_or_program_validation() {
    let request = ClientRequest {
        normalized_input: Vec::new(),
        associated_data: Vec::new(),
    };
    for backend in [RamLfeBackend::BfvAffineV1, RamLfeBackend::BfvProgrammedV1] {
        let policy = diagnostic_policy(backend);
        assert_eq!(
            evaluate_commitment(&[], &policy, &request),
            Err(RamLfeError::InsecureBfvProfile)
        );
        assert_eq!(
            evaluate_commitment_with_hidden_program(&[], &policy, &request, None),
            Err(RamLfeError::InsecureBfvProfile)
        );
    }
}

#[test]
fn public_trace_cannot_bypass_production_admission() {
    let policy = diagnostic_policy(RamLfeBackend::BfvProgrammedV1);
    let request = ClientRequest {
        normalized_input: vec![0xff],
        associated_data: Vec::new(),
    };
    let error = evaluate_programmed_with_trace(
        &[],
        &policy,
        &request,
        &default_bfv_programmed_hidden_program(),
    )
    .unwrap_err();
    assert_eq!(error, RamLfeError::InsecureBfvProfile);
}

#[test]
fn explicit_prf_evaluation_still_uses_public_facade() {
    let secret = b"public-test-prf-secret";
    let policy = policy_commitment(secret, b"prf-context".to_vec()).unwrap();
    let request = ClientRequest {
        normalized_input: b"public-request".to_vec(),
        associated_data: b"public-associated-data".to_vec(),
    };
    let first = evaluate_commitment(secret, &policy, &request).unwrap();
    let second = evaluate_commitment_with_hidden_program(secret, &policy, &request, None).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.backend, RamLfeBackend::HkdfSha3_512PrfV1);
}
