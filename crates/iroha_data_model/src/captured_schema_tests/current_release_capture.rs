//! Current first-release identity checks and explicit native capture inventory.
//!
//! Ordinary checks and the ignored capture printer share the same typed inventory.

use super::Case;

const CASES: &[Case] = &[
    Case::bidirectional::<crate::fastpq::FastpqTransitionBatch>(
        "iroha_data_model::fastpq::FastpqTransitionBatch",
    ),
    Case::bidirectional::<crate::kagemusha::KagemushaGovernedVerifierReleaseV1>(
        "iroha:kagemusha:governed-verifier-release:v1",
    ),
    Case::bidirectional::<crate::kagemusha::KagemushaGovernedVerifierRegistryV1>(
        "iroha:kagemusha:governed-verifier-registry:v1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierPolicyInstallProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierPolicyInstallProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierReleaseInstallProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierReleaseInstallProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierReleaseActivateProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierReleaseActivateProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierPolicyInstallV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierPolicyInstallV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierReleaseInstallV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseInstallV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierReleaseActivateV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseActivateV1",
    ),
];

super::native_capture::owner_printer!(CASES);

#[test]
fn current_release_identities_match_native_capture() {
    for case in CASES {
        case.check();
    }
}

// Separately captured retirement identities retain their own native producer evidence;
// historical identities and their original report remain byte-for-byte intact.
#[cfg(feature = "governance")]
mod retirement_capture {
    use super::Case;

    const CASES: &[Case] = &[
        Case::bidirectional::<crate::governance::types::KagemushaVerifierReleaseRetireProposalV1>(
            "iroha_data_model::parliament_types::KagemushaVerifierReleaseRetireProposalV1",
        ),
        Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierReleaseRetireV1>(
            "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseRetireV1",
        ),
    ];

    crate::captured_schema_tests::native_capture::owner_printer!(CASES);

    // This fixture is the exact paired native owner output. Historical capture
    // rows/report remain unchanged; this checks only the two new typed owners.
    #[test]
    fn retirement_identities_match_genuine_native_capture() {
        use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};
        use sha2::{Digest, Sha256};
        use std::collections::BTreeSet;

        fn check<T>(nominal: &str, rows: &[Value])
        where
            T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
        {
            let row = rows
                .iter()
                .find(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal))
                .expect("genuine captured retirement type");
            assert_eq!(
                row.as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["nominal", "root", "serialize_hash", "deserialize_hash"])
            );
            assert_eq!(T::nominal_name(), nominal);
            assert_eq!(
                T::frame_name(),
                row.get("root").and_then(Value::as_str).unwrap()
            );
            let hash = norito::schema::identity::frame_hash::<T>();
            assert_eq!(hash, super::super::expected_hash(row, "serialize_hash"));
            assert_eq!(hash, super::super::expected_hash(row, "deserialize_hash"));
        }
        let source =
            include_str!("../../tests/fixtures/native_governed_retirement_codec_identities.json");
        assert_eq!(
            hex::encode(Sha256::digest(source.as_bytes())),
            "3bfcd19b58eac35a5f008f65a33ec9b2a59dfd1f92e0395add7d747aff16a732"
        );
        let document: Value =
            norito::json::from_str(source).expect("genuine native retirement capture");
        assert_eq!(
            document
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["schema", "owner", "rows"])
        );
        assert_eq!(document.get("schema").and_then(Value::as_u64), Some(1));
        assert_eq!(
            document.get("owner").and_then(Value::as_str),
            Some(module_path!())
        );
        let rows = document.get("rows").and_then(Value::as_array).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .map(|row| row.get("nominal").and_then(Value::as_str).unwrap())
                .collect::<BTreeSet<_>>(),
            CASES.iter().map(|case| case.nominal).collect()
        );
        check::<crate::governance::types::KagemushaVerifierReleaseRetireProposalV1>(
            "iroha_data_model::parliament_types::KagemushaVerifierReleaseRetireProposalV1",
            rows,
        );
        check::<crate::isi::governance::ProposeKagemushaVerifierReleaseRetireV1>(
            "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseRetireV1",
            rows,
        );
    }
}
