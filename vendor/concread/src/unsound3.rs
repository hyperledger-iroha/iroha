#![forbid(unsafe_code)]
extern crate concread;

#[cfg(not(feature = "unsoundness"))]
fn main() {
    eprintln!("Recompile with --features unsoundness");
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "unsoundness")]
enum RefOrInt<'a> {
    Ref(&'a u64),
    Int(u64),
}

#[cfg(feature = "unsoundness")]
fn main() {
    use concread::arcache::ARCache;
    use std::cell::Cell;
    use std::sync::Arc;

    static PARENT_STATIC: u64 = 1;

    // `Cell` is `Send` but not `Sync`.
    let item_not_sync = Cell::new(RefOrInt::Ref(&PARENT_STATIC));

    let cache = ARCache::<i32, Cell<RefOrInt>>::new_size(5, 5);
    let mut writer = cache.write();
    writer.insert(0, item_not_sync);
    writer.commit();

    let arc_parent = Arc::new(cache);
    let arc_child = arc_parent.clone();

    std::thread::spawn(move || {
        let arc_child = arc_child;
        // new `Reader` of `ARCache`
        let reader = arc_child.read();
        let ref_to_smuggled_cell = reader.get(&0).unwrap();

        static CHILD_STATIC: u64 = 1;
        loop {
            ref_to_smuggled_cell.set(RefOrInt::Ref(&CHILD_STATIC));
            ref_to_smuggled_cell.set(RefOrInt::Int(0xDEADBEEF));
        }
    });

    let reader = arc_parent.read();
    let ref_to_inner_cell = reader.get(&0).unwrap();
    loop {
        if let RefOrInt::Ref(addr) = ref_to_inner_cell.get() {
            if addr as *const _ as usize == 0xDEADBEEF {
                println!("We have bypassed enum checking");
                println!("Dereferencing `addr` will now segfault : {}", *addr);
            }
        }
    }
}
