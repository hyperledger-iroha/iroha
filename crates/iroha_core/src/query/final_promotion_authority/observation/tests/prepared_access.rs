//! Prepared getters expose original expectations without renewing or qualifying the Check.
use super::*;

#[test]
fn prepared_check_getters_retain_exact_binding_account_and_one_use_instruction() {
    let f = Fixture::new();
    let expected = f.expected();
    let binding = expected.binding.clone();
    let observer = expected.observer.clone();
    let operator = expected.expected_operator.clone();
    let prepared =
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .unwrap();
    let instruction = norito::encode_canonical(prepared.instruction()).unwrap();
    let snapshot = f.snapshot();
    for _ in 0..3 {
        assert_eq!(prepared.binding(), &binding);
        assert_eq!(prepared.observer(), &observer);
        assert_eq!(prepared.expected_operator(), &operator);
        prepared.ensure_live().unwrap();
        assert_eq!(
            norito::encode_canonical(prepared.instruction()).unwrap(),
            instruction
        );
    }
    assert_eq!(f.snapshot(), snapshot);
    let signed = f.sign(prepared.instruction().clone().into(), 3, NOW);
    let pending = prepared.bind_signed_transaction(signed.clone()).unwrap();
    assert_eq!(pending.signed_transaction(), &signed);
    pending.ensure_live().unwrap();
}

#[test]
fn prepared_getters_and_repeated_liveness_checks_cannot_renew_an_expired_round() {
    let f = Fixture::new();
    let mut prepared = f.prepared();
    let binding = prepared.binding().clone();
    let observer = prepared.observer().clone();
    let operator = prepared.expected_operator().clone();
    let instruction = norito::encode_canonical(prepared.instruction()).unwrap();
    let signed = f.sign(prepared.instruction().clone().into(), 3, NOW);
    prepared.round.expire_for_test();
    for _ in 0..3 {
        assert_eq!(prepared.binding(), &binding);
        assert_eq!(prepared.observer(), &observer);
        assert_eq!(prepared.expected_operator(), &operator);
        assert_eq!(prepared.ensure_live(), Err(Error::Expired));
        assert_eq!(
            norito::encode_canonical(prepared.instruction()).unwrap(),
            instruction
        );
    }
    assert_eq!(
        prepared.bind_signed_transaction(signed).err(),
        Some(Error::Expired)
    );
}
