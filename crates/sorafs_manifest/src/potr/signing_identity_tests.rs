//! Capture PoTR signing frames and distinguish optional context from a required request ID.

use super::*;
use crate::signing_identity_test_support::shapes_decodable;
use norito::json::Value;

pub(crate) fn record(rows: &mut Vec<Value>) {
    let populated = tests::base_receipt();
    let mut absent = populated.clone();
    absent.trace_id = None;
    absent.note = None;
    tests::resign(&mut absent);
    for (case, receipt) in [("optional-some", &populated), ("optional-none", &absent)] {
        receipt.validate().expect("complete signed receipt fixture");
        record_receipt(rows, case, receipt);
    }

    let mut missing_request = absent;
    missing_request.request_id = None;
    assert_eq!(
        missing_request.validate_unsigned(),
        Err(PotrReceiptValidationError::MissingRequestId)
    );
    assert_eq!(
        missing_request.validate(),
        Err(PotrReceiptValidationError::MissingRequestId)
    );
    // The wire can represent None, but this is not a valid signed receipt operation.
    record_receipt(rows, "missing-request-rejected", &missing_request);
}

fn record_receipt(rows: &mut Vec<Value>, case: &str, receipt: &PotrReceiptV1) {
    let mut unsigned = receipt.clone();
    unsigned.gateway_signature = None;
    unsigned.provider_signature = None;
    let mut expected = POTR_RECEIPT_SIGNATURE_DOMAIN_V1.to_vec();
    expected.extend_from_slice(&norito::encode_canonical(&unsigned).expect("owned receipt frame"));
    assert_eq!(
        receipt.signing_payload_bytes().expect("signing bytes"),
        expected
    );
    shapes_decodable(
        rows,
        &format!("potr/receipt/{case}"),
        || PotrReceiptSigningViewV1::from_receipt(receipt),
        unsigned,
    );
}
