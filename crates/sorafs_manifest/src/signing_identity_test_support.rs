//! Declared identities and immutable wire checks for manual SoraFS signing owners.

use std::{collections::BTreeSet, fmt::Debug};

use norito::{core as ncore, json::Value};

fn record<T, U>(
    rows: &mut Vec<Value>,
    case: &str,
    borrowed: &T,
    owned: &U,
    root: bool,
    decoded: bool,
    check_decode: impl Fn(&[u8], &[u8], u8, bool),
) where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema,
{
    let canonical = norito::encode_canonical(borrowed).expect("canonical borrowed signing frame");
    let owned_canonical = norito::encode_canonical(owned).expect("canonical owned signing value");
    let view = ncore::from_bytes_view(&canonical).expect("validated borrowed archive");
    let active_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(norito::schema::identity::frame_hash::<T>(), active_hash);
    assert_eq!(view.schema(), active_hash);
    assert_eq!(
        norito::canonical_frame_len(borrowed).expect("count exact canonical frame"),
        canonical.len()
    );
    if root {
        assert_eq!(T::frame_name(), U::frame_name());
        assert_ne!(T::nominal_name(), T::frame_name());
        assert_eq!(active_hash, norito::schema::identity::frame_hash::<U>());
        assert_eq!(canonical, owned_canonical);
    } else {
        assert_eq!(T::frame_name(), T::nominal_name());
        assert_ne!(active_hash, norito::schema::identity::frame_hash::<U>());
    }
    assert!(ncore::from_bytes_view(&canonical[..canonical.len() - 1]).is_err());
    check_decode(&canonical, view.as_bytes(), view.flags(), true);

    let mut layouts = Vec::new();
    for requested_flags in crate::canonical_test_support::supported_layouts() {
        let _layout = ncore::DecodeFlagsGuard::enter(requested_flags);
        let frame = norito::to_bytes(borrowed).expect("borrowed explicit-layout frame");
        let owned_frame = norito::to_bytes(owned).expect("owned explicit-layout frame");
        let archive = ncore::from_bytes_view(&frame).expect("validated explicit-layout frame");
        let owned_archive =
            ncore::from_bytes_view(&owned_frame).expect("owned explicit-layout frame");
        assert_eq!(archive.schema(), active_hash);
        assert_eq!(archive.flags(), owned_archive.flags());
        assert_eq!(archive.as_bytes(), owned_archive.as_bytes());
        if root {
            assert_eq!(frame, owned_frame);
        }
        assert_eq!(
            norito::encode_canonical(borrowed).expect("canonical ignores ambient layout"),
            canonical
        );
        let _advertised_layout = ncore::DecodeFlagsGuard::enter(archive.flags());
        let mut bare = Vec::new();
        ncore::serialize_to_buffer(borrowed, &mut bare).expect("serialize advertised layout");
        assert_eq!(bare, archive.as_bytes());
        assert_eq!(
            ncore::encoded_payload_len(borrowed).expect("count borrowed payload"),
            bare.len()
        );
        assert_eq!(borrowed.encoded_len_exact(), owned.encoded_len_exact());
        if let Some(exact) = borrowed.encoded_len_exact() {
            assert_eq!(exact, bare.len());
        }
        check_decode(&frame, &bare, archive.flags(), false);
        layouts.push(norito::json!({
            "requested_flags": requested_flags,
            "advertised_flags": (archive.flags()),
            "frame_hex": (hex::encode(&frame)),
            "bare_hex": (hex::encode(&bare)),
        }));
    }
    rows.push(norito::json!({
        "case": case,
        "compiler_nominal_name": (T::nominal_name()),
        "compiler_nominal_hash": (hex::encode(ncore::schema_hash_for_name(&T::nominal_name()))),
        "active_serialize_hash": (hex::encode(active_hash)),
        "owned_frame_name": (U::frame_name()),
        "root_projection": root,
        "owned_decoder_checked": decoded,
        "canonical_flags": (view.flags()),
        "canonical_frame_hex": (hex::encode(&canonical)),
        "canonical_bare_hex": (hex::encode(view.as_bytes())),
        "layouts": layouts,
    }));
}

fn record_decodable<T, U>(rows: &mut Vec<Value>, case: &str, borrowed: &T, owned: &U, root: bool)
where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema + PartialEq + Debug,
    for<'de> U: norito::NoritoDeserialize<'de>,
{
    record(
        rows,
        case,
        borrowed,
        owned,
        root,
        true,
        |frame, bare, flags, canonical| {
            let _layout = ncore::DecodeFlagsGuard::enter(flags);
            let (decoded, used) = ncore::decode_field_canonical::<U>(bare)
                .expect("decode via the existing owned payload implementation");
            assert_eq!(decoded, *owned);
            assert_eq!(used, bare.len());
            if canonical {
                if root {
                    assert_eq!(
                        norito::decode_canonical::<U>(frame).expect("decode projected owned frame"),
                        *owned
                    );
                    let mut wrong_schema = frame.to_vec();
                    wrong_schema[6] ^= 1;
                    assert!(matches!(
                        norito::decode_canonical::<U>(&wrong_schema),
                        Err(norito::Error::SchemaMismatch)
                    ));
                } else {
                    assert!(matches!(
                        norito::decode_canonical::<U>(frame),
                        Err(norito::Error::SchemaMismatch)
                    ));
                }
            }
        },
    );
}

fn record_encode_only<T, U>(rows: &mut Vec<Value>, case: &str, borrowed: &T, owned: &U, root: bool)
where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema,
{
    record(rows, case, borrowed, owned, root, false, |_, _, _, _| {});
}

/// Check the five carrier shapes without adding a borrowed decoder.
pub(crate) fn shapes_decodable<T, U>(
    rows: &mut Vec<Value>,
    case: &str,
    make: impl Fn() -> T,
    owned: U,
) where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema + Clone + PartialEq + Debug,
    for<'de> U: norito::NoritoDeserialize<'de>,
{
    record_decodable(rows, &format!("{case}/root"), &make(), &owned, true);
    record_decodable(
        rows,
        &format!("{case}/option-none"),
        &None::<T>,
        &None::<U>,
        false,
    );
    record_decodable(
        rows,
        &format!("{case}/option-some"),
        &Some(make()),
        &Some(owned.clone()),
        false,
    );
    record_decodable(
        rows,
        &format!("{case}/vec-empty"),
        &Vec::<T>::new(),
        &Vec::<U>::new(),
        false,
    );
    record_decodable(
        rows,
        &format!("{case}/vec-populated"),
        &vec![make(), make()],
        &vec![owned.clone(), owned],
        false,
    );
}

/// Check existing serialize-only owned projections without inventing a decoder.
pub(crate) fn shapes_encode_only<T, U>(
    rows: &mut Vec<Value>,
    case: &str,
    make: impl Fn() -> T,
    owned: U,
) where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema + Clone,
{
    record_encode_only(rows, &format!("{case}/root"), &make(), &owned, true);
    record_encode_only(
        rows,
        &format!("{case}/option-none"),
        &None::<T>,
        &None::<U>,
        false,
    );
    record_encode_only(
        rows,
        &format!("{case}/option-some"),
        &Some(make()),
        &Some(owned.clone()),
        false,
    );
    record_encode_only(
        rows,
        &format!("{case}/vec-empty"),
        &Vec::<T>::new(),
        &Vec::<U>::new(),
        false,
    );
    record_encode_only(
        rows,
        &format!("{case}/vec-populated"),
        &vec![make(), make()],
        &vec![owned.clone(), owned],
        false,
    );
}

/// Check a local preflight sentinel's identity without attempting serialization.
pub(crate) fn check_rejected_identity<T: norito::NoritoSerialize + norito::NoritoSchema>(
    case: &str,
) {
    let expected: Vec<Value> =
        include_str!("../tests/fixtures/sorafs_signing_identity_sentinels.jsonl")
            .lines()
            .map(|line| norito::json::from_str(line).expect("immutable sentinel identity record"))
            .collect();
    assert_eq!(expected.len(), 3);
    let cases: BTreeSet<_> = expected
        .iter()
        .map(|row| row["case"].as_str().expect("sentinel case"))
        .collect();
    assert_eq!(cases.len(), expected.len());
    let expected = expected
        .iter()
        .find(|row| row["case"].as_str() == Some(case))
        .expect("captured sentinel");
    let active_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(T::nominal_name(), T::frame_name());
    assert_eq!(norito::schema::identity::frame_hash::<T>(), active_hash);
    let actual = norito::json!({
        "case": case,
        "compiler_nominal_name": (T::nominal_name()),
        "compiler_nominal_hash": (hex::encode(ncore::schema_hash_for_name(&T::nominal_name()))),
        "active_serialize_hash": (hex::encode(active_hash)),
        "serialized": false,
    });
    assert_eq!(actual, *expected);
}

#[test]
fn manual_signing_frames_match_captured_identities() {
    let expected: Value = norito::json::from_str(include_str!(
        "../tests/fixtures/sorafs_signing_identity_frames.json"
    ))
    .expect("immutable pre-declaration signing frame capture");
    let mut rows = Vec::new();
    crate::governance::signing_identity_tests::record(&mut rows);
    crate::pop_credentials::signing_identity_tests::record(&mut rows);
    crate::por::signing_identity_tests::record(&mut rows);
    crate::potr::signing_identity_tests::record(&mut rows);
    crate::provider_advert::signing_identity_tests::record(&mut rows);
    crate::reputation::signed::signing_identity_tests::check_rejected_sentinel();
    assert_eq!(rows.len(), 130, "complete fixture/shape inventory");
    let cases: BTreeSet<_> = rows
        .iter()
        .map(|row| row["case"].as_str().expect("case"))
        .collect();
    assert_eq!(cases.len(), rows.len(), "capture cases must be unique");
    let root_names: BTreeSet<_> = rows
        .iter()
        .filter(|row| row["root_projection"].as_bool() == Some(true))
        .map(|row| {
            row["compiler_nominal_name"]
                .as_str()
                .expect("observed root name")
        })
        .collect();
    assert_eq!(root_names.len(), 12);
    let actual = norito::json!({
        "format_version": 1,
        "purpose": "remaining SoraFS manual signing owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": rows,
    });
    assert_eq!(actual, expected);
}
