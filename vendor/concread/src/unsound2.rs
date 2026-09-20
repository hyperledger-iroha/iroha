#![forbid(unsafe_code)]
extern crate concread;

#[cfg(not(feature = "unsoundness"))]
fn main() {
    eprintln!("Recompile with --features unsoundness");
}

#[cfg(feature = "unsoundness")]
fn main() {
    use concread::arcache::ARCache;
    use std::rc::Rc;
    use std::sync::Arc;

    let non_sync_item = Rc::new(0); // neither `Send` nor `Sync`
    assert_eq!(Rc::strong_count(&non_sync_item), 1);

    let cache = ARCache::<i32, Rc<u64>>::new_size(5, 5);
    let mut writer = cache.write();
    writer.insert(0, non_sync_item);
    writer.commit();

    let arc_parent = Arc::new(cache);

    let mut handles = vec![];
    for _ in 0..5 {
        let arc_child = arc_parent.clone();
        let child_handle = std::thread::spawn(move || {
            let reader = arc_child.read(); // new Reader of ARCache
            let smuggled_rc = reader.get(&0).unwrap();

            for _ in 0..1000 {
                let _dummy_clone = Rc::clone(&smuggled_rc); // Increment `strong_count` of `Rc`
                                                            // When `_dummy_clone` is dropped, `strong_count` is decremented.
            }
        });
        handles.push(child_handle);
    }
    for handle in handles {
        handle.join().expect("failed to join child thread");
    }

    assert_eq!(Rc::strong_count(arc_parent.read().get(&0).unwrap()), 1);
}
