#[cfg(test)]
fn advance_block_height(
    block_height: &mut BlockHeight,
    next_height: &mut u64,
    observed_height: u64,
    is_empty: bool,
) {
    if observed_height != *next_height {
        warn!(
            expected = *next_height,
            observed = observed_height,
            "missed block height update; resynchronising block watcher"
        );
    }
    block_height.total = observed_height;
    if !is_empty {
        block_height.non_empty = block_height.non_empty.saturating_add(1);
        if block_height.non_empty > block_height.total {
            block_height.non_empty = block_height.total;
        }
    }
    *next_height = block_height
        .total
        .checked_add(1)
        .expect("block height overflow when subscribing to blocks");
}
