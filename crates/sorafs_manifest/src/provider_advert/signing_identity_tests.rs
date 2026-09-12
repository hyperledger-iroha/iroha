//! Capture the provider advertisement's existing domain-separated signing projection.

use super::*;
use crate::signing_identity_test_support::shapes_decodable;
use norito::json::Value;

pub(crate) fn record(rows: &mut Vec<Value>) {
    let advert = tests::signed_sample_advert(1_700_000_000);
    let owned = advert.signature_payload();
    let mut expected = PROVIDER_ADVERT_SIGNATURE_DOMAIN_V1.to_vec();
    expected.extend_from_slice(&norito::encode_canonical(&owned).expect("owned advert frame"));
    assert_eq!(
        advert.signature_payload_bytes().expect("signing bytes"),
        expected
    );
    shapes_decodable(
        rows,
        "provider-advert/signature/populated",
        || ProviderAdvertSignaturePayloadViewV1::from(&advert),
        owned,
    );
}
