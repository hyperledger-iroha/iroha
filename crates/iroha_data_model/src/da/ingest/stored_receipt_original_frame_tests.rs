//! Exact original Torii producer frames decoded by the canonical model owner.
//!
//! These populated DTO fixtures cover encoding and framing. Their deterministic
//! signatures sign a fixed label, not the receipt protocol signing preimage.

use super::{StoredDaReceipt, stored_receipt_frame_tests::fixture};

const FIXTURE: &str =
    include_str!("../../../../../fixtures/da/stored_receipt_original_frames.v1.json");

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

#[test]
fn original_producer_root_and_container_frames_are_exact() {
    let rows: norito::json::Value = norito::json::from_str(FIXTURE).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for (case_index, (case, pdp)) in [("pdp_none", None), ("pdp_some", Some(vec![1, 2, 3]))]
        .into_iter()
        .enumerate()
    {
        let group = &rows[case_index * 5..(case_index + 1) * 5];
        for (row, shape) in
            group
                .iter()
                .zip(["root", "option_none", "option_some", "vec_empty", "vec_two"])
        {
            assert_eq!(row["case"].as_str(), Some(case));
            assert_eq!(row["shape"].as_str(), Some(shape));
        }
        let stored = fixture(pdp);
        check_frame(&group[0], &stored);
        check_frame(&group[1], &Option::<StoredDaReceipt>::None);
        check_frame(&group[2], &Some(stored.clone()));
        check_frame(&group[3], &Vec::<StoredDaReceipt>::new());
        check_frame(&group[4], &vec![stored.clone(), stored]);
    }
}
