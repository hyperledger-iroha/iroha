//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveTier>(
        "iroha_data_model::sorafs::reserve::ReserveTier",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveDuration>(
        "iroha_data_model::sorafs::reserve::ReserveDuration",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ClassRentRate>(
        "iroha_data_model::sorafs::reserve::ClassRentRate",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DurationFactorSet>(
        "iroha_data_model::sorafs::reserve::DurationFactorSet",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveTierConfig>(
        "iroha_data_model::sorafs::reserve::ReserveTierConfig",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReservePolicyV1>(
        "iroha_data_model::sorafs::reserve::ReservePolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveQuote>(
        "iroha_data_model::sorafs::reserve::ReserveQuote",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveLedgerProjection>(
        "iroha_data_model::sorafs::reserve::ReserveLedgerProjection",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveLifecycleStage>(
        "iroha_data_model::sorafs::reserve::ReserveLifecycleStage",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveLifecycleProjection>(
        "iroha_data_model::sorafs::reserve::ReserveLifecycleProjection",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveAuthorityPolicyV1>(
        "iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveAuthorityPolicyRecordV1>(
        "iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveProviderTermsV1>(
        "iroha_data_model::sorafs::reserve::ReserveProviderTermsV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveProviderAccountV1>(
        "iroha_data_model::sorafs::reserve::ReserveProviderAccountV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveMovementKindV1>(
        "iroha_data_model::sorafs::reserve::ReserveMovementKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveMovementStatusV1>(
        "iroha_data_model::sorafs::reserve::ReserveMovementStatusV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveMovementRecordV1>(
        "iroha_data_model::sorafs::reserve::ReserveMovementRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveAppealStatusV1>(
        "iroha_data_model::sorafs::reserve::ReserveAppealStatusV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveAppealRecordV1>(
        "iroha_data_model::sorafs::reserve::ReserveAppealRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveFinalizedCursorV1>(
        "iroha_data_model::sorafs::reserve::ReserveFinalizedCursorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveFinalizedEventCursorV1>(
        "iroha_data_model::sorafs::reserve::ReserveFinalizedEventCursorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveFinalizedEventV1>(
        "iroha_data_model::sorafs::reserve::ReserveFinalizedEventV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveFinalizedEventPageV1>(
        "iroha_data_model::sorafs::reserve::ReserveFinalizedEventPageV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveProviderAccountPageV1>(
        "iroha_data_model::sorafs::reserve::ReserveProviderAccountPageV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveMovementPageV1>(
        "iroha_data_model::sorafs::reserve::ReserveMovementPageV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReserveAppealPageV1>(
        "iroha_data_model::sorafs::reserve::ReserveAppealPageV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);

/// Current first-release policy proof owners, separate from historical capture evidence.
mod current_policy_proof {
    use std::{collections::BTreeMap, io::Read};

    use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

    use crate::{
        captured_schema_tests::Case,
        sorafs::reserve::{
            history::{ReserveEventJournalHeadV1, ReserveStateV1},
            proof::ReservePolicyProofV1,
        },
    };

    const MAX_CAPTURE_BYTES: usize = 64 * 1024;

    fn assert_row<T>(rows: &mut BTreeMap<String, Value>, nominal: &str)
    where
        T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
    {
        let row = rows.remove(nominal).expect("native capture includes owner");
        assert_eq!(row.as_object().unwrap().len(), 4);
        assert_eq!(T::nominal_name(), nominal);
        assert_eq!(row.get("nominal").and_then(Value::as_str), Some(nominal));
        assert_eq!(
            row.get("root").and_then(Value::as_str),
            Some(T::frame_name().as_str())
        );
        let hash = hex::encode(norito::schema::identity::frame_hash::<T>());
        for direction in ["serialize_hash", "deserialize_hash"] {
            assert_eq!(
                row.get(direction).and_then(Value::as_str),
                Some(hash.as_str())
            );
        }
    }

    // One typed inventory drives both maintenance output and ordinary assertions.
    macro_rules! inventory {
        ($($ty:ty => $nominal:literal),+ $(,)?) => {
            const CURRENT_CASES: &[Case] = &[
                $(Case::bidirectional::<$ty>($nominal)),+
            ];

            fn assert_rows(mut rows: BTreeMap<String, Value>) {
                $(assert_row::<$ty>(&mut rows, $nominal);)+
                assert!(rows.is_empty(), "capture contains an unregistered owner");
            }
        };
    }

    inventory! {
        ReserveEventJournalHeadV1 => "iroha_data_model::sorafs::reserve::history::ReserveEventJournalHeadV1",
        ReserveStateV1 => "iroha_data_model::sorafs::reserve::history::ReserveStateV1",
        ReservePolicyProofV1 => "iroha_data_model::sorafs::reserve::proof::ReservePolicyProofV1",
    }

    crate::captured_schema_tests::native_capture::owner_printer!(CURRENT_CASES);

    #[test]
    fn current_policy_proof_identities_match_native_capture() {
        // Read at runtime so the compiled native printer can create the first fixture.
        // A missing fixture is a failure; ordinary tests never create or update it.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native_reserve_policy_codec_identities.json");
        let file = std::fs::File::open(path).expect("run the native reserve capture printer");
        let metadata = file.metadata().unwrap();
        assert!(metadata.is_file());
        assert!(metadata.len() > 0 && metadata.len() <= MAX_CAPTURE_BYTES as u64);
        let mut bytes = Vec::new();
        file.take((MAX_CAPTURE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(!bytes.is_empty() && bytes.len() <= MAX_CAPTURE_BYTES);
        let document: Value = norito::json::from_slice(&bytes).expect("native capture JSON");
        assert_eq!(document.as_object().unwrap().len(), 3);
        assert_eq!(document.get("schema").and_then(Value::as_u64), Some(1));
        assert_eq!(
            document.get("owner").and_then(Value::as_str),
            Some(module_path!())
        );
        let captured = document.get("rows").and_then(Value::as_array).unwrap();
        assert_eq!(captured.len(), CURRENT_CASES.len());
        let mut rows = BTreeMap::new();
        for row in captured {
            let nominal = row.get("nominal").and_then(Value::as_str).unwrap();
            assert!(rows.insert(nominal.to_owned(), row.clone()).is_none());
        }
        assert_rows(rows);
    }
}
