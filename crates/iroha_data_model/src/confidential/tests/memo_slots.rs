//! Fixed slot storage, closed JSON projection, and schema invariants.
use super::*;

fn slots() -> ConfidentialMemoRecipientSlotsV1 {
    core::array::from_fn(|index| {
        memo_slot(
            u8::try_from(index).unwrap(),
            ConfidentialMemoSuiteV1::MlKem768XChaCha20Poly1305,
        )
    })
    .into()
}

fn fields(value: &ConfidentialMemoRecipientSlotsV1) -> Vec<String> {
    value
        .iter()
        .enumerate()
        .map(|(index, slot)| format!("\"slot_{index}\":{}", norito::json::to_json(slot).unwrap()))
        .collect()
}

#[test]
fn fixed_storage_preserves_all_positions_and_mutation() {
    let mut value = slots();
    assert_eq!(value.len(), 8);
    assert!(!value.is_empty());
    for (index, slot) in value.iter().enumerate() {
        assert_eq!(value.get(index), Some(slot));
        assert_eq!(&value[index], slot);
    }
    for index in [8, usize::MAX] {
        assert!(value.get(index).is_none());
        assert!(value.get_mut(index).is_none());
    }
    let replacement = memo_slot(42, ConfidentialMemoSuiteV1::MlKem1024XChaCha20Poly1305);
    value[7] = replacement.clone();
    assert_eq!(value.iter().next_back(), Some(&replacement));
    assert_eq!(value.clone().into_array()[7], replacement);
    assert_eq!(
        ConfidentialMemoRecipientSlotsV1::from(value.clone().into_array()),
        value
    );
    assert_eq!(ConfidentialMemoRecipientSlotsV1::default().len(), 8);
}

#[test]
fn named_json_preserves_order_and_exact_output_budget() {
    let value = slots();
    let expected = format!("{{{}}}", fields(&value).join(","));
    assert_eq!(norito::json::to_json(&value).unwrap(), expected);
    assert_eq!(
        norito::json::to_json_bounded(&value, expected.len()).unwrap(),
        expected
    );
    assert!(norito::json::to_json_bounded(&value, expected.len() - 1).is_err());
    assert_eq!(
        norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>(&expected).unwrap(),
        value
    );
    let reversed = format!(
        "{{{}}}",
        fields(&value)
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>(&reversed).unwrap(),
        value
    );
}

#[test]
fn closed_json_rejects_every_missing_duplicate_and_extra_position() {
    let entries = fields(&slots());
    for index in 0..8 {
        let missing = format!(
            "{{{}}}",
            entries
                .iter()
                .enumerate()
                .filter(|(position, _)| *position != index)
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>(&missing)
                .unwrap_err()
                .to_string(),
            norito::json::Error::missing_field(format!("slot_{index}")).to_string()
        );
        let duplicate = format!("{{{},\"slot_{index}\":null}}", entries.join(","));
        assert_eq!(
            norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>(&duplicate)
                .unwrap_err()
                .to_string(),
            norito::json::Error::duplicate_field(format!("slot_{index}")).to_string()
        );
    }
    for key in ["slot_8", "slots", "slot_00"] {
        let extra = format!("{{{},\"{key}\":null}}", entries.join(","));
        assert_eq!(
            norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>(&extra)
                .unwrap_err()
                .to_string(),
            norito::json::Error::unknown_field(key).to_string()
        );
    }
    assert!(norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>("[]").is_err());
    assert_eq!(
        norito::json::from_str::<ConfidentialMemoRecipientSlotsV1>("{}")
            .unwrap_err()
            .to_string(),
        norito::json::Error::missing_field("slot_0").to_string()
    );
}

#[test]
fn slot_schema_remains_the_eight_named_wire_fields() {
    use iroha_schema::{Declaration, Metadata, NamedFieldsMeta, TypeId};
    assert_eq!(
        ConfidentialMemoRecipientSlotsV1::id(),
        "ConfidentialMemoRecipientSlotsV1"
    );
    assert_eq!(
        ConfidentialMemoRecipientSlotsV1::type_name(),
        "ConfidentialMemoRecipientSlotsV1"
    );
    let mut map = ConfidentialMemoRecipientSlotsV1::schema();
    let expected = Metadata::Struct(NamedFieldsMeta {
        declarations: (0..8)
            .map(|index| Declaration {
                name: format!("slot_{index}"),
                ty: core::any::TypeId::of::<ConfidentialMemoRecipientSlotV1>(),
            })
            .collect(),
    });
    assert_eq!(
        map.get::<ConfidentialMemoRecipientSlotsV1>(),
        Some(&expected)
    );
    assert!(map.contains_key::<ConfidentialMemoRecipientSlotV1>());
    let once = map.clone();
    ConfidentialMemoRecipientSlotsV1::update_schema_map(&mut map);
    assert_eq!(map, once);
}
