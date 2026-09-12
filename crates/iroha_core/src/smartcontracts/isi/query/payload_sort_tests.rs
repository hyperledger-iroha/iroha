//! Deterministic sorting uses ordered resident keys without a binary codec requirement.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ResidentKey(u8);

struct Row(u8);

impl SortableQueryOutput for Row {
    type TiebreakKey = ResidentKey;

    fn get_metadata_sorting_key(&self, _: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> ResidentKey {
        ResidentKey(self.0)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.0, limit)
    }
}

#[test]
fn incremental_sort_compares_nonserializable_keys_across_chunks() {
    for (order, expected) in [
        (SortOrder::Asc, [5, 3, 9, 1]),
        (SortOrder::Desc, [3, 9, 5, 1]),
    ] {
        let rows = [9, 5, 3, 1].map(|id| Some(Row(id))).into();
        let keys = [9, 5, 3, 1].map(ResidentKey).into();
        let metadata = vec![
            Some(Json::new(2)),
            Some(Json::new(1)),
            Some(Json::new(2)),
            None,
        ];
        let sorted = IncrementalSortedValues::new(
            rows,
            metadata,
            keys,
            iroha_data_model::query::parameters::Pagination::default(),
            NonZeroU64::new(2).unwrap(),
            order,
        );
        assert_eq!(sorted.map(|row| row.0).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn resident_key_measurement_retains_byte_limit() {
    let row = Row(7);
    assert_eq!(
        row.bounded_tiebreak_key_len(0),
        Err(Error::GasBudgetExceeded)
    );
    assert_eq!(row.bounded_tiebreak_key_len(1), Ok(1));
    assert_eq!(row.tiebreak_cmp(&Row(9)), core::cmp::Ordering::Less);
    assert_eq!(row.tiebreak_cmp(&Row(7)), core::cmp::Ordering::Equal);
}
