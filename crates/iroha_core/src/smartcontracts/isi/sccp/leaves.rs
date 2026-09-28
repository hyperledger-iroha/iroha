//! Dense per-block SCCP leaf allocation (`specs/sccp.md` §4.4 steps 7 and 12, §4.5, §4.14.6).
//! Owner: ws20 (complete).
//!
//! Every SCCP leaf of block `h` (a transfer or a destination control) is recorded at
//! `sccp_block_leaves[(h, commitment_index)]`, where `commitment_index` is the number of leaves
//! already recorded in `h`. All SCCP instructions execute serially in the universal dataspace,
//! and due Parliament certificates execute at block start, so the indices are deterministic and
//! control leaves precede transfer leaves. A failed transaction releases its indices by state
//! rollback. The post-execution hook asserts that the indices of `h` are exactly `0..m`.

use super::{Error, store};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::sccp::{control::SccpLeafRefV1, params::SCCP_MESSAGES_MAX_PER_BLOCK_V1};

/// Maximum number of SCCP leaves (transfers and controls) in one block (§3.4).
pub const MAX_LEAVES_PER_BLOCK: u32 = SCCP_MESSAGES_MAX_PER_BLOCK_V1;

/// Return the number of leaves recorded at `height`.
#[must_use]
pub fn leaf_count_at(world: &(impl WorldReadOnly + ?Sized), height: u64) -> usize {
    store::block_leaves::range(world, (height, 0)..=(height, u32::MAX)).count()
}

/// Return the leaves recorded at `height` in `commitment_index` order.
#[must_use]
pub fn leaves_at(world: &(impl WorldReadOnly + ?Sized), height: u64) -> Vec<SccpLeafRefV1> {
    store::block_leaves::range(world, (height, 0)..=(height, u32::MAX))
        .map(|(_, leaf)| *leaf)
        .collect()
}

/// Record `leaf` in the executing block at the next dense `commitment_index` and return it.
///
/// # Errors
///
/// Fails when the block already holds [`MAX_LEAVES_PER_BLOCK`] leaves, or when the recorded
/// indices of the block are not dense (an execution invariant violation).
pub fn allocate_leaf(
    state_transaction: &mut StateTransaction<'_, '_>,
    leaf: SccpLeafRefV1,
) -> Result<u32, Error> {
    let height = state_transaction._curr_block.height().get();
    let count = leaf_count_at(&*state_transaction.world, height);
    let index = u32::try_from(count)
        .ok()
        .filter(|index| *index < MAX_LEAVES_PER_BLOCK)
        .ok_or_else(|| {
            Error::InvariantViolation(
                format!(
                    "SCCP: block {height} already holds the maximum of {MAX_LEAVES_PER_BLOCK} leaves"
                )
                .into(),
            )
        })?;
    if store::block_leaves::contains(&*state_transaction.world, &(height, index)) {
        return Err(Error::InvariantViolation(
            format!("SCCP: leaf indices of block {height} are not dense").into(),
        ));
    }
    store::block_leaves::insert(state_transaction, (height, index), leaf)?;
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};
    use iroha_data_model::bridge::SccpNetworkV1;

    fn transfer(seed: u32) -> SccpLeafRefV1 {
        let mut id = [0_u8; 32];
        id[..4].copy_from_slice(&seed.to_be_bytes());
        SccpLeafRefV1::transfer(id)
    }

    #[test]
    fn allocation_is_dense_per_height_and_mixes_leaf_kinds_in_order() {
        let state = blank_state();
        let mut block = state.block(header(9));
        let mut stx = block.transaction();
        let control = SccpLeafRefV1::control(SccpNetworkV1::TonMainnet, 2, 1);
        assert_eq!(allocate_leaf(&mut stx, control), Ok(0));
        assert_eq!(allocate_leaf(&mut stx, transfer(1)), Ok(1));
        assert_eq!(allocate_leaf(&mut stx, transfer(2)), Ok(2));
        assert_eq!(leaf_count_at(&*stx.world, 9), 3);
        assert_eq!(
            leaves_at(&*stx.world, 9),
            vec![control, transfer(1), transfer(2)]
        );
        assert!(leaves_at(&*stx.world, 8).is_empty());
        assert!(leaves_at(&*stx.world, 10).is_empty());
    }

    #[test]
    fn a_dropped_transaction_releases_its_indices() {
        let state = blank_state();
        let mut block = state.block(header(9));
        {
            let mut stx = block.transaction();
            assert_eq!(allocate_leaf(&mut stx, transfer(1)), Ok(0));
            stx.apply();
        }
        {
            let mut failed = block.transaction();
            assert_eq!(allocate_leaf(&mut failed, transfer(2)), Ok(1));
            // Dropped without apply: a failed transaction.
        }
        let mut stx = block.transaction();
        assert_eq!(allocate_leaf(&mut stx, transfer(3)), Ok(1));
        assert_eq!(leaves_at(&*stx.world, 9), vec![transfer(1), transfer(3)]);
    }

    #[test]
    fn the_513th_leaf_of_a_block_is_refused() {
        let state = blank_state();
        let mut block = state.block(header(9));
        let mut stx = block.transaction();
        for seed in 0..MAX_LEAVES_PER_BLOCK {
            assert_eq!(allocate_leaf(&mut stx, transfer(seed)), Ok(seed));
        }
        let error = allocate_leaf(&mut stx, transfer(512)).expect_err("512 leaves is the cap");
        assert!(
            format!("{error}").contains("maximum of 512 leaves"),
            "{error}"
        );
        assert_eq!(leaf_count_at(&*stx.world, 9), 512);
    }

    #[test]
    fn a_gap_in_the_recorded_indices_is_an_invariant_violation() {
        let state = blank_state();
        let mut block = state.block(header(9));
        let mut stx = block.transaction();
        store::block_leaves::insert(&mut stx, (9, 1), transfer(7)).expect("index below the limit");
        let error = allocate_leaf(&mut stx, transfer(8)).expect_err("indices must be dense");
        assert!(format!("{error}").contains("not dense"), "{error}");
    }
}
