//! Initialized lookup-tile cleanup and allocation/error boundary checks.
//!
//! Observations happen before Vec deallocation and cover initialized owned slots only. These
//! checks do not claim erasure of compiler copies, allocator slack, backend buffers or process RSS.

use super::*;
use halo2curves::pasta::{Fp, Fq};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn reset_observations() {
    FIELD_CLEAR_OBSERVATION.with(|record| record.set((0, true)));
    ENCODED_CLEAR_OBSERVATION.with(|record| record.set((0, true)));
}

fn assert_observations(fields: usize, encoded: usize) {
    FIELD_CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (fields, true)));
    ENCODED_CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (encoded, true)));
}

fn clear_entire_tiles<F: StoredAssignmentFieldV1>() {
    reset_observations();
    let mut fields = FieldTile::<F>::new().unwrap();
    let mut encoded = EncodedTile::new().unwrap();
    assert_eq!(fields.0.len(), TILE);
    assert_eq!(encoded.0.len(), TILE);
    assert!(fields.0.iter().all(|value| *value == F::ZERO));
    assert!(
        encoded
            .0
            .iter()
            .all(|value| *value == [0; STORED_SCALAR_BYTES_V1])
    );
    // Include the entire inactive suffix of a short tile in the cleanup observation.
    fields.0.fill(F::from(17));
    encoded.0.fill([0xab; STORED_SCALAR_BYTES_V1]);
    fields.clear();
    encoded.clear();
    assert!(fields.0.iter().all(|value| *value == F::ZERO));
    assert!(
        encoded
            .0
            .iter()
            .all(|value| *value == [0; STORED_SCALAR_BYTES_V1])
    );
    assert_observations(TILE, TILE);
    drop(fields);
    drop(encoded);
    assert_observations(2 * TILE, 2 * TILE);
}

#[test]
fn guarded_tiles_initialize_and_clear_every_owned_slot_in_both_fields() {
    clear_entire_tiles::<Fp>();
    clear_entire_tiles::<Fq>();
}

#[derive(Debug)]
struct TileUnwind;

fn drop_paths<F: StoredAssignmentFieldV1>() {
    for mode in 0..3 {
        reset_observations();
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), StoredLookupErrorV1> {
            let mut fields = FieldTile::<F>::new()?;
            let mut encoded = EncodedTile::new()?;
            fields.0.fill(F::from(29));
            encoded.0.fill([0xcd; STORED_SCALAR_BYTES_V1]);
            if mode == 1 {
                return Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Storage));
            }
            if mode == 2 {
                std::panic::panic_any(TileUnwind);
            }
            Ok(())
        }));
        match mode {
            0 => assert_eq!(result.unwrap(), Ok(())),
            1 => assert_eq!(
                result.unwrap(),
                Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Storage))
            ),
            2 => assert!(result.unwrap_err().is::<TileUnwind>()),
            _ => unreachable!(),
        }
        assert_observations(TILE, TILE);
    }
}

#[test]
fn guarded_tiles_clear_before_deallocation_on_success_error_and_typed_unwind() {
    drop_paths::<Fp>();
    drop_paths::<Fq>();
}

#[test]
fn tile_budget_allocation_overflow_and_terminal_error_mapping_are_explicit() {
    assert_eq!(tile_payload_bytes::<Fp>().unwrap(), 16 * 1024);
    assert_eq!(tile_payload_bytes::<Fq>().unwrap(), 16 * 1024);
    assert_eq!(
        reserved::<u8>(usize::MAX),
        Err(StoredLookupErrorV1::Allocation)
    );
    assert!(reserved::<u8>(0).unwrap().is_empty());
    assert_eq!(
        StoredLookupErrorV1::from(StoredPolynomialErrorV1::Authentication),
        StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Authentication)
    );
    assert_eq!(
        StoredLookupErrorV1::from(StoredPhaseErrorV1::Poisoned),
        StoredLookupErrorV1::Phase(StoredPhaseErrorV1::Poisoned)
    );
    assert_eq!(
        StoredLookupErrorV1::from(StoredExpressionErrorV1::ScratchLimit),
        StoredLookupErrorV1::ScratchLimit
    );
    assert_eq!(
        StoredLookupErrorV1::from(StoredExpressionErrorV1::Consumer),
        StoredLookupErrorV1::Expression(StoredExpressionErrorV1::Consumer)
    );
}
