//! Named-resume absence response is exact request-bound scheduling DATA only.
use super::*;

fn absent() -> SuffixUnloadStatusV1 {
    SuffixUnloadStatusV1 {
        format_version: 1,
        discriminator: 15,
        state: 7,
        reason: 0,
        current_sequence: Some(0),
        folded_sequence: None,
        request_id: Some([1; 32]),
        completed_steps: 0,
        total_steps: 0,
    }
}

#[test]
fn absence_response_requires_the_named_resume_exact_request() {
    let expected = absent();
    let response = absent_response(expected, Some([1; 32])).unwrap();
    assert_eq!(response.kind, 77);
    assert_eq!(response.sequence, 0);
    assert_eq!(response.detail, 0);
    assert_eq!(
        SuffixUnloadStatusV1::decode_canonical(&response.bytes).unwrap(),
        expected
    );
    assert!(absent_response(expected, None).is_err());
    assert!(absent_response(expected, Some([2; 32])).is_err());
    let mut wrong = expected;
    wrong.current_sequence = None;
    assert!(absent_response(wrong, Some([1; 32])).is_err());
    let mut wrong = expected;
    wrong.state = 3;
    wrong.total_steps = 1;
    assert!(absent_response(wrong, Some([1; 32])).is_err());
}
