//! Observed identities and exact bytes for the existing borrowed signing frames.

use std::{collections::BTreeSet, fmt::Debug};

use norito::{core as ncore, json::Value};

use super::{
    OrderCancelSigningViewV1, OrderRequestSigningViewV1, OrderbookSignatureSigningViewV1,
    SettlementReceiptSigningViewV1,
    tests::{cancel, order, receipt, sign_cancel, sign_order, sign_receipt},
};

fn record<T, U>(rows: &mut Vec<Value>, case: &str, borrowed: &T, owned: &U, root: bool)
where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema + PartialEq + Debug,
    for<'de> U: norito::NoritoDeserialize<'de>,
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
        // Decode through the existing owned record only. Borrowed signing views have no decoder.
        assert_eq!(
            norito::decode_canonical::<U>(&canonical).expect("decode projected owned frame"),
            *owned
        );
        let mut wrong_schema = canonical.clone();
        wrong_schema[6] ^= 1;
        assert!(matches!(
            norito::decode_canonical::<U>(&wrong_schema),
            Err(norito::Error::SchemaMismatch)
        ));
    } else {
        // A container retains its borrowed element's nominal identity, not its root projection.
        assert_eq!(T::frame_name(), T::nominal_name());
        assert_ne!(active_hash, norito::schema::identity::frame_hash::<U>());
        assert!(matches!(
            norito::decode_canonical::<U>(&canonical),
            Err(norito::Error::SchemaMismatch)
        ));
    }
    assert!(ncore::from_bytes_view(&canonical[..canonical.len() - 1]).is_err());

    let mut layouts = Vec::new();
    for requested_flags in crate::canonical_test_support::supported_layouts() {
        let _layout = ncore::DecodeFlagsGuard::enter(requested_flags);
        let frame = norito::to_bytes(borrowed).expect("borrowed frame under explicit layout");
        let owned_frame = norito::to_bytes(owned).expect("owned frame under explicit layout");
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
            norito::encode_canonical(borrowed).expect("canonical encoding ignores ambient layout"),
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
        if let Some(exact) = borrowed.encoded_len_exact() {
            assert_eq!(exact, bare.len());
        }
        let (decoded, used) = ncore::decode_field_canonical::<U>(archive.as_bytes())
            .expect("decode signing fields through existing owned payload implementation");
        assert_eq!(decoded, *owned);
        assert_eq!(used, archive.as_bytes().len());
        layouts.push(norito::json!({
            "requested_flags": requested_flags,
            "advertised_flags": (archive.flags()),
            "frame_hex": (hex::encode(&frame)),
            "bare_hex": (hex::encode(&bare)),
        }));
    }
    rows.push(norito::json!({
        "case": case,
        // These fixture fields retain the original compiler observations. Compare the declared
        // protocol identity so physical source relocation cannot silently change framed bytes.
        "compiler_nominal_name": (T::nominal_name()),
        "compiler_nominal_hash": (hex::encode(ncore::schema_hash_for_name(&T::nominal_name()))),
        "active_serialize_hash": (hex::encode(active_hash)),
        "owned_frame_name": (U::frame_name()),
        "root_projection": root,
        "canonical_flags": (view.flags()),
        "canonical_frame_hex": (hex::encode(&canonical)),
        "canonical_bare_hex": (hex::encode(view.as_bytes())),
        "layouts": layouts,
    }));
}

fn record_shapes<T, U>(rows: &mut Vec<Value>, case: &str, make: impl Fn() -> T, owned: U)
where
    T: norito::NoritoSerialize + norito::NoritoSchema,
    U: norito::NoritoSerialize + norito::NoritoSchema + Clone + PartialEq + Debug,
    for<'de> U: norito::NoritoDeserialize<'de>,
{
    record(rows, &format!("{case}/root"), &make(), &owned, true);
    record(
        rows,
        &format!("{case}/option-none"),
        &None::<T>,
        &None::<U>,
        false,
    );
    record(
        rows,
        &format!("{case}/option-some"),
        &Some(make()),
        &Some(owned.clone()),
        false,
    );
    record(
        rows,
        &format!("{case}/vec-empty"),
        &Vec::<T>::new(),
        &Vec::<U>::new(),
        false,
    );
    record(
        rows,
        &format!("{case}/vec-populated"),
        &vec![make(), make()],
        &vec![owned.clone(), owned],
        false,
    );
}

fn observed_values() -> Vec<Value> {
    let signed_order = sign_order(order(), 0x11);
    let signed_cancel = sign_cancel(cancel(), 0x12);
    let signed_receipt = sign_receipt(receipt(), 0x14);
    assert!(!signed_order.signature.signature.is_empty());
    assert!(!signed_cancel.signature.signature.is_empty());
    assert!(!signed_receipt.settlement_signature.signature.is_empty());
    let mut owned_order = signed_order.clone();
    owned_order.signature.signature.clear();
    let mut owned_cancel = signed_cancel.clone();
    owned_cancel.signature.signature.clear();
    let mut owned_receipt = signed_receipt.clone();
    owned_receipt.settlement_signature.signature.clear();
    let mut rows = Vec::new();
    record_shapes(
        &mut rows,
        "order",
        || OrderRequestSigningViewV1::from_order(&signed_order),
        owned_order,
    );
    record_shapes(
        &mut rows,
        "cancel",
        || OrderCancelSigningViewV1::from_cancel(&signed_cancel),
        owned_cancel,
    );
    record_shapes(
        &mut rows,
        "receipt",
        || SettlementReceiptSigningViewV1::from_receipt(&signed_receipt),
        owned_receipt,
    );
    assert_eq!(rows.len(), 15);
    rows
}

#[test]
fn signing_view_frames_match_captured_identities() {
    let expected: Value = norito::json::from_str(include_str!(
        "../../tests/fixtures/orderbook_signing_identity_frames.json"
    ))
    .expect("immutable pre-declaration signing-view capture");
    let rows = observed_values();
    let cases: BTreeSet<_> = rows
        .iter()
        .map(|row| row["case"].as_str().expect("case identifier"))
        .collect();
    assert_eq!(cases.len(), 15);
    for row in &rows {
        assert_eq!(
            row["layouts"]
                .as_array()
                .expect("explicit layout rows")
                .len(),
            8
        );
    }
    let actual = norito::json!({
        "format_version": 1,
        "purpose": "existing orderbook signing frame identities before declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": rows,
    });
    assert_eq!(actual, expected);
}

#[test]
fn nested_signature_view_preserves_payload_without_a_frame_owner() {
    let signed = sign_order(order(), 0x11);
    let mut owned = signed.signature.clone();
    owned.signature.clear();
    let borrowed = OrderbookSignatureSigningViewV1::from_signature(&signed.signature);
    crate::canonical_test_support::assert_same_payload(&borrowed, &owned);
}
