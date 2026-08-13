//! State lock-order regression tests.
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use core::num::NonZeroU64;
use super::*;
fn test_state() -> State {
    let kura = crate::kura::Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    State::new_for_testing(World::default(), Arc::clone(&kura), query)
}
#[test]
fn state_block_orders_block_hashes_before_world() {
    let state = Arc::new(test_state());
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block_hashes_view = state.block_hashes.view();
    let (start_tx, start_rx) = mpsc::channel();
    let (block_tx, block_rx) = mpsc::channel();
    let state_for_block = Arc::clone(&state);
    let block_thread = thread::spawn(move || {
        let _ = start_tx.send(());
        let _block = state_for_block.block(header);
        let _ = block_tx.send(());
    });
    start_rx
        .recv_timeout(Duration::from_millis(200))
        .expect("block thread did not start");
    thread::sleep(Duration::from_millis(10));
    let (world_tx, world_rx) = mpsc::channel();
    let state_for_world = Arc::clone(&state);
    let world_thread = thread::spawn(move || {
        let _view = state_for_world.world.view();
        let _ = world_tx.send(());
    });
    let world_ready = world_rx.recv_timeout(Duration::from_millis(500)).is_ok();
    drop(block_hashes_view);
    let _ = block_rx.recv_timeout(Duration::from_secs(1));
    let _ = block_thread.join();
    let _ = world_thread.join();
    assert!(
        world_ready,
        "world view blocked while block hashes read lock held; lock order may be inverted"
    );
}
