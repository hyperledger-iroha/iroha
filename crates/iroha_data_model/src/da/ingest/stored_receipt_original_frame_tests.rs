//! Exact Torii producer frames decoded by the canonical model owner.
//!
//! `stored_receipt_frames.v2.json` pins the version-2 writer's root and container
//! frames. `stored_receipt_original_frames.v1.json` keeps the original version-1
//! frames, written before receipts carried `retention_policy`, as refusal vectors.
//! These populated DTO fixtures cover encoding and framing. Their deterministic
//! signatures sign a fixed label, not the receipt protocol signing preimage.

use super::{StoredDaReceipt, StoredDaReceiptFrameError, stored_receipt_frame_tests::fixture};

const FIXTURE_V2_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/da/stored_receipt_frames.v2.json"
);
const RETIRED_V1: &str =
    include_str!("../../../../../fixtures/da/stored_receipt_original_frames.v1.json");

const CASES: [(&str, Option<[u8; 3]>); 2] = [("pdp_none", None), ("pdp_some", Some([1, 2, 3]))];
const SHAPES: [&str; 5] = ["root", "option_none", "option_some", "vec_empty", "vec_two"];

fn check_frame<T>(row: &norito::json::Value, value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + std::fmt::Debug + Eq,
{
    assert_eq!(T::nominal_name(), row["nominal"].as_str().unwrap());
    let hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(hex::encode(hash), row["serialize_hash"].as_str().unwrap());
    assert_eq!(hex::encode(hash), row["deserialize_hash"].as_str().unwrap());
    let captured = hex::decode(row["frame_hex"].as_str().unwrap()).unwrap();
    let header = norito::core::Header::read(captured.as_slice()).unwrap();
    assert_eq!(header.schema, hash);
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(norito::encode_canonical(value).unwrap(), captured);
    assert_eq!(&norito::decode_canonical::<T>(&captured).unwrap(), value);
    for length in [0, captured.len() / 2, captured.len() - 1] {
        assert!(norito::decode_canonical::<T>(&captured[..length]).is_err());
    }
    let mut trailing = captured.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let mut corrupt = captured;
    corrupt[0] ^= 1;
    assert!(norito::decode_canonical::<T>(&corrupt).is_err());
}

fn frame_row<T>(case: &str, shape: &str, value: &T) -> norito::json::Value
where
    T: norito::NoritoSerialize,
{
    let hash = hex::encode(norito::schema::identity::frame_hash::<T>());
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    let frame_hex = hex::encode(norito::encode_canonical(value).unwrap());
    let nominal = T::nominal_name();
    let deserialize_hash = hash.clone();
    norito::json!({
        "case": case,
        "deserialize_hash": deserialize_hash,
        "frame_hex": frame_hex,
        "nominal": nominal,
        "serialize_hash": hash,
        "shape": shape
    })
}

fn current_rows() -> Vec<norito::json::Value> {
    let mut rows = Vec::new();
    for (case, pdp) in CASES {
        let stored = fixture(pdp.map(|bytes| bytes.to_vec()));
        rows.push(frame_row(case, SHAPES[0], &stored));
        rows.push(frame_row(case, SHAPES[1], &Option::<StoredDaReceipt>::None));
        rows.push(frame_row(case, SHAPES[2], &Some(stored.clone())));
        rows.push(frame_row(case, SHAPES[3], &Vec::<StoredDaReceipt>::new()));
        rows.push(frame_row(case, SHAPES[4], &vec![stored.clone(), stored]));
    }
    rows
}

#[test]
fn version_2_producer_root_and_container_frames_are_exact() {
    let fixture_text =
        std::fs::read_to_string(FIXTURE_V2_PATH).expect("version-2 stored receipt frame fixture");
    let rows: norito::json::Value = norito::json::from_str(&fixture_text).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for (case_index, (case, pdp)) in CASES.into_iter().enumerate() {
        let group = &rows[case_index * 5..(case_index + 1) * 5];
        for (row, shape) in group.iter().zip(SHAPES) {
            assert_eq!(row["case"].as_str(), Some(case));
            assert_eq!(row["shape"].as_str(), Some(shape));
        }
        let stored = fixture(pdp.map(|bytes| bytes.to_vec()));
        check_frame(&group[0], &stored);
        check_frame(&group[1], &Option::<StoredDaReceipt>::None);
        check_frame(&group[2], &Some(stored.clone()));
        check_frame(&group[3], &Vec::<StoredDaReceipt>::new());
        check_frame(&group[4], &vec![stored.clone(), stored]);
        let root = hex::decode(group[0]["frame_hex"].as_str().unwrap()).unwrap();
        assert_eq!(
            StoredDaReceipt::decode_frame(&root).unwrap(),
            fixture(pdp.map(|bytes| bytes.to_vec()))
        );
    }
}

#[test]
fn retired_version_1_populated_frames_are_refused() {
    let rows: norito::json::Value = norito::json::from_str(RETIRED_V1).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 10);
    let mut populated = 0;
    for row in rows {
        let frame = hex::decode(row["frame_hex"].as_str().unwrap()).unwrap();
        match row["shape"].as_str().unwrap() {
            "root" => {
                assert_eq!(
                    row["nominal"].as_str(),
                    Some(StoredDaReceipt::RETIRED_V1_FRAME_NAME)
                );
                assert!(matches!(
                    StoredDaReceipt::decode_frame(&frame),
                    Err(StoredDaReceiptFrameError::RetiredVersion1)
                ));
                assert!(matches!(
                    norito::decode_canonical::<StoredDaReceipt>(&frame),
                    Err(norito::Error::SchemaMismatch)
                ));
                populated += 1;
            }
            "option_some" => {
                assert!(matches!(
                    norito::decode_canonical::<Option<StoredDaReceipt>>(&frame),
                    Err(norito::Error::SchemaMismatch)
                ));
                populated += 1;
            }
            "vec_two" => {
                assert!(matches!(
                    norito::decode_canonical::<Vec<StoredDaReceipt>>(&frame),
                    Err(norito::Error::SchemaMismatch)
                ));
                populated += 1;
            }
            // Empty version-1 containers carry no receipt, but their headers also name
            // the retired owner, so they are refused too.
            "option_none" => assert!(matches!(
                norito::decode_canonical::<Option<StoredDaReceipt>>(&frame),
                Err(norito::Error::SchemaMismatch)
            )),
            "vec_empty" => assert!(matches!(
                norito::decode_canonical::<Vec<StoredDaReceipt>>(&frame),
                Err(norito::Error::SchemaMismatch)
            )),
            other => panic!("unexpected retired fixture shape {other}"),
        }
    }
    assert_eq!(populated, 6);
}

#[test]
#[ignore = "creates the version-2 stored receipt frame fixture only; refuses to replace existing evidence"]
fn capture_version_2_stored_receipt_frames() {
    use std::io::Write as _;
    let mut text =
        norito::json::to_json_pretty(&norito::json::Value::Array(current_rows())).unwrap();
    text.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(FIXTURE_V2_PATH)
        .expect("create the version-2 fixture; existing evidence must not be overwritten");
    file.write_all(text.as_bytes()).unwrap();
}
