//! Borrowed callback parity, consumed failures and exact ordered geometry.

use super::*;
use iroha_data_model::prelude::TransactionEntrypoint;
use mv::allocation::AllocationBudget;
type Key = iroha_crypto::HashOf<TransactionEntrypoint>;

fn allowance() -> TypedPairedRowAllowance {
    TypedPairedRowAllowance {
        streamed_bytes: 65536,
        ordered_bytes: 65536,
    }
}

fn limits() -> LeafLimits {
    LeafLimits {
        max_tables: 1,
        max_rows: 8,
        max_payload_bytes: 4096,
        max_ordered_table_bytes: 65536,
        max_streamed_value_bytes: 65536,
    }
}

#[test]
fn stack_values_finish_identically_to_borrowed_iterator_and_own_their_credit() {
    let pool = AllocationBudget::new(1 << 20);
    let keys = [
        Key::from_untyped_unchecked(Hash::new(b"first")),
        Key::from_untyped_unchecked(Hash::new(b"second")),
    ];
    let mut builder =
        TypedPairedTableBuilder::<Key, u64>::new("state.transactions.current", limits(), &pool)
            .unwrap();
    for (index, key) in keys.iter().enumerate() {
        let height = index as u64 + 1;
        builder = builder.push(key, &height, allowance()).unwrap();
    }
    let usage = builder.usage().unwrap();
    assert_eq!(usage.0, 2);
    assert!(usage.1 > 0 && usage.2 > 0);
    let callback = builder.finish().unwrap();
    let values = [1_u64, 2];
    let iterator = CanonicalTableLeafSet::paired_table_from_rows(
        "state.transactions.current",
        limits(),
        &pool,
        keys.iter().zip(&values),
    )
    .unwrap();
    assert_eq!(callback.root(), iterator.root());
    let retained = pool.reserved_bytes();
    assert!(retained > 0);
    pool.set_limit_bytes(0);
    assert_eq!(pool.reserved_bytes(), retained);
    drop(callback);
    drop(iterator);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn failed_typed_rows_and_duplicate_finish_discard_complete_prefix() {
    let pool = AllocationBudget::new(1 << 20);
    let key = Key::from_untyped_unchecked(Hash::new(b"duplicate"));
    let builder = TypedPairedTableBuilder::<Key, u64>::new(
        "state.transactions.current",
        LeafLimits {
            max_rows: 1,
            ..limits()
        },
        &pool,
    )
    .unwrap()
    .push(&key, &1, allowance())
    .unwrap();
    assert!(matches!(
        builder.push(&key, &2, allowance()),
        Err(TypedPairedRowError::Table(LeafError::RowLimit))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    let wrong =
        TypedPairedTableBuilder::<Key, i64>::new("state.transactions.current", limits(), &pool)
            .unwrap();
    assert!(matches!(
        wrong.push(&key, &-1, allowance()),
        Err(TypedPairedRowError::Table(LeafError::TypeMismatch(_)))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    let repeated =
        TypedPairedTableBuilder::<Key, u64>::new("state.transactions.current", limits(), &pool)
            .unwrap()
            .push(&key, &1, allowance())
            .unwrap()
            .push(&key, &1, allowance())
            .unwrap();
    assert!(repeated.finish().is_err());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn ordered_byte_geometry_includes_padding_and_rejects_overflow() {
    assert_eq!(ordered_retained_bytes(0, 0).unwrap(), 0);
    assert_eq!(ordered_retained_bytes(1, 12).unwrap(), 12 + Hash::LENGTH);
    assert_eq!(
        ordered_retained_bytes(3, 12).unwrap(),
        12 + 7 * Hash::LENGTH
    );
    assert!(ordered_retained_bytes(usize::MAX, 0).is_err());
    assert!(ordered_retained_bytes(1, usize::MAX).is_err());
}

struct Recording<'a, T> {
    value: &'a T,
    calls: std::cell::Cell<usize>,
}

impl<T: NoritoSchema> NoritoSchema for Recording<'_, T> {
    fn nominal_name() -> String {
        T::nominal_name()
    }
}

impl<T: norito::SerializePayload> norito::SerializePayload for Recording<'_, T> {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.calls.set(self.calls.get() + 1);
        self.value.serialize(encoder)
    }
}

#[test]
fn aggregate_stream_admission_bounds_both_passes_before_value_work() {
    let key = Key::from_untyped_unchecked(Hash::new(b"stream"));
    let height = 7_u64;
    let exact = 2 * u64::try_from(norito::codec::encode_adaptive(&height).len()).unwrap();
    for available in [exact - 1, exact] {
        let pool = AllocationBudget::new(1 << 20);
        let value = Recording {
            value: &height,
            calls: std::cell::Cell::new(0),
        };
        let builder = TypedPairedTableBuilder::<Key, Recording<'_, u64>>::new(
            "state.transactions.current",
            limits(),
            &pool,
        )
        .unwrap();
        let result = builder.push(
            &key,
            &value,
            TypedPairedRowAllowance {
                streamed_bytes: available,
                ..allowance()
            },
        );
        if available == exact {
            let builder = result.unwrap();
            assert_eq!(value.calls.get(), 2);
            assert_eq!(builder.usage().unwrap().1, exact);
            drop(builder.finish().unwrap());
        } else {
            assert!(matches!(
                result,
                Err(TypedPairedRowError::StreamedAllowance)
            ));
            assert_eq!(
                value.calls.get(),
                1,
                "refused first pass never starts the second"
            );
        }
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn aggregate_ordered_admission_precedes_key_backing_and_value_work() {
    let key = Key::from_untyped_unchecked(Hash::new(b"ordered"));
    let height = 7_u64;
    let exact = 2 * norito::codec::encode_adaptive(&key).len() + 4 * Hash::LENGTH;
    for available in [exact - 1, exact] {
        let pool = AllocationBudget::new(1 << 20);
        let key = Recording {
            value: &key,
            calls: std::cell::Cell::new(0),
        };
        let value = Recording {
            value: &height,
            calls: std::cell::Cell::new(0),
        };
        let builder = TypedPairedTableBuilder::<Recording<'_, Key>, Recording<'_, u64>>::new(
            "state.transactions.current",
            limits(),
            &pool,
        )
        .unwrap();
        let result = builder.push(
            &key,
            &value,
            TypedPairedRowAllowance {
                ordered_bytes: available,
                ..allowance()
            },
        );
        if available == exact {
            let builder = result.unwrap();
            assert_eq!((key.calls.get(), value.calls.get()), (2, 2));
            assert_eq!(builder.usage().unwrap().2, exact);
            drop(builder.finish().unwrap());
        } else {
            assert!(matches!(result, Err(TypedPairedRowError::OrderedAllowance)));
            assert_eq!((key.calls.get(), value.calls.get()), (1, 0));
            assert_eq!(
                pool.peak_reserved_bytes(),
                0,
                "no key backing before admission"
            );
        }
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn next_padding_geometry_is_admitted_before_any_new_key_encoding() {
    let pool = AllocationBudget::new(1 << 20);
    let mut builder = TypedPairedTableBuilder::<Recording<'_, Key>, u64>::new(
        "state.transactions.current",
        limits(),
        &pool,
    )
    .unwrap();
    let keys = [
        Key::from_untyped_unchecked(Hash::new(b"one")),
        Key::from_untyped_unchecked(Hash::new(b"two")),
        Key::from_untyped_unchecked(Hash::new(b"three")),
    ];
    for key in &keys[..2] {
        let key = Recording {
            value: key,
            calls: std::cell::Cell::new(0),
        };
        builder = builder.push(&key, &1, allowance()).unwrap();
    }
    let key = Recording {
        value: &keys[2],
        calls: std::cell::Cell::new(0),
    };
    // The third row expands the padded tree from three to seven nodes, plus
    // three retained row digests, before accounting for either key copy.
    assert!(matches!(
        builder.push(
            &key,
            &1,
            TypedPairedRowAllowance {
                ordered_bytes: 7 * Hash::LENGTH - 1,
                ..allowance()
            }
        ),
        Err(TypedPairedRowError::OrderedAllowance)
    ));
    assert_eq!(key.calls.get(), 0);
    assert_eq!(pool.reserved_bytes(), 0);
}
