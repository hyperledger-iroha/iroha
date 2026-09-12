//! Original compiler frames for exact nested-invocation rejection expectations.

use super::*;

const FIXTURE: &str = include_str!("../../../../fixtures/kotodama/compiler_frame_identity_v1.json");

fn fixture_groups(owner: &str) -> Vec<norito::json::Value> {
    let fixture: norito::json::Value = norito::json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["schema"].as_str(),
        Some("iroha.compiler.frame-observations.v1")
    );
    assert_eq!(
        fixture["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    fixture["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| group["owner"].as_str() == Some(owner))
        .cloned()
        .collect()
}

fn check_frame<T>(row: &norito::json::Value, value: T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + std::fmt::Debug + Eq,
{
    assert_eq!(T::nominal_name(), row["nominal"].as_str().unwrap());
    assert_eq!(T::frame_name(), row["nominal"].as_str().unwrap());
    let frame_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(
        hex::encode(frame_hash),
        row["serialize_schema_hash"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(frame_hash),
        row["deserialize_schema_hash"].as_str().unwrap()
    );
    let captured = hex::decode(row["frame_hex"].as_str().unwrap()).unwrap();
    let header = norito::core::Header::read(captured.as_slice()).unwrap();
    assert_eq!(header.schema, frame_hash);
    // An unrelated ambient layout must not change the canonical ABI boundary.
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(norito::encode_canonical(&value).unwrap(), captured);
    assert_eq!(norito::decode_canonical::<T>(&captured).unwrap(), value);
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

fn check_group<T>(group: &norito::json::Value, value: T)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + Clone
        + std::fmt::Debug
        + Eq,
{
    let frames = group["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    for (row, expected) in
        frames
            .iter()
            .zip(["root", "option_none", "option_some", "vec_empty", "vec_two"])
    {
        assert_eq!(row["shape"].as_str(), Some(expected));
    }
    check_frame(&frames[0], value.clone());
    check_frame(&frames[1], None::<T>);
    check_frame(&frames[2], Some(value.clone()));
    check_frame(&frames[3], Vec::<T>::new());
    check_frame(&frames[4], vec![value.clone(), value]);
}

#[test]
fn original_compiler_rejection_frames_are_exact() {
    let groups = fixture_groups("rejection_expectation");
    assert_eq!(groups.len(), 5);
    for group in groups {
        let value = match group["case"].as_str().unwrap() {
            "any" => RejectionExpectation::Any,
            "permission_denied" => RejectionExpectation::PermissionDenied,
            "invalid_arguments" => RejectionExpectation::InvalidArguments,
            "contract" => {
                let descriptor = ivm_abi::error_types::list_error_type();
                RejectionExpectation::Contract {
                    code: descriptor.variants[0].code,
                    descriptor,
                }
            }
            "trap" => RejectionExpectation::Trap(RejectionTrap::OutOfGas),
            case => panic!("unexpected captured rejection case: {case}"),
        };
        assert!(value.validate());
        check_group(&group, value);
    }
}
