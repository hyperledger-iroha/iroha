Concread
========

Concurrently readable datastructures for Rust.

Concurrently readable is often referred to as Copy-On-Write, Multi-Version-Concurrency-Control.

These structures allow multiple readers with transactions
to proceed while single writers can operate. A reader is guaranteed the content
will remain the same for the duration of the read, and readers do not block writers.
Writers are serialised, just like a mutex.

This library contains concurrently readable Cell types and Map/Cache types.

When do I want to use these?
----------------------------

You can use these in place of a RwLock, and will likely see improvements in
parallel throughput.

The best use is in place of mutex/rwlock, where the reader exists for a
non-trivial amount of time.

For example, if you have a RwLock where the lock is taken, data changed or read, and dropped
immediately, this probably won't help you.

However, if you have a RwLock where you hold the read lock for any amount of time,
writers will begin to stall - or inversely, the writer will cause readers to block
and wait as the writer proceeds.

Concurrently readable avoids this because readers never stall readers/writers, writers
never stall or block a readers. This means that you gain in parallel throughput
as stalls are reduced.

This library also has a concurrently readable BTreeMap, HashMap and Adaptive Replacement Cache.
These are best used when you have at least 512 bytes worth of data in your Cell, as they only copy
what is required for an update.

If you do not required key-ordering, then the HashMap will likely be the best choice
for most applications.

What is concurrently readable?
------------------------------

In a multithread application, data is commonly needed to be shared between threads.
In sharing this there are multiple policies for this - Atomics for single integer
reads, Mutexes for single thread access, RwLock for many readers or one writer,
all the way to Lock Free which allows multiple read and writes of queues.

Lock Free however has the limitation of being built on Atomics. This means it can
really only update small amounts of data at a time consistently. It also means
that you don't have transactional behaviours. While this is great for queues,
it is not so good for a tree or hashmap where you want the state to be consistent
from the state to the end of an operation. In the few places that lock free trees
exist, they have the properly that as each thread is updating the tree, the changes
are visible immediately to all other readers. Your data could change before you
know it.

Mutexes and RwLock on the other hand allow much more complex structures to be protected.
The guarantee that all readers see the same data, always, and that writers are
the only writer. But they cause stalls on other threads waiting to access them.
RwLock for example can see large delays if a reader won't yield, and OS policy
can cause reader/writer to starve if the priority favours the other.

Concurrently readable structures sit in between these two points. They provide
multiple concurrent readers, with transactional behaviour, while allowing single
writers to proceed simultaneously.

This is achieved by having writers copy the internal data before they modify
it. This allows readers to access old data, without modification, and allows
the writer to change the data inplace before committing. Once the new data is
stored, old readers continue to access their old data - new readers will
see the new data.

This is a space-time trade off, using more memory to achieve better parallel
behaviour.

Safety
------

This library has extensive testing, and passes its test suite under [miri], a rust
undefined behaviour checker. If you find an issue however, please let us know so we can
fix it!

To check with miri OR asan on nightly:

    # Follow the miri readme setup steps
    cargo clean && MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-disable-stacked-borrows" cargo +nightly miri test
    RUSTC_FLAGS="-Z sanitizer=address" cargo test

Note: Miri requires isolation to be disabled so that clock monotonic can be used in ARC for cache channels.

[miri]: https://github.com/rust-lang/miri

SIMD
----

There is support for SIMD in ARC if you are using a nightly compiler. To use this, you need to compile
with:

    RUSTFLAGS="-C target-feature=+avx2,+avx" cargo ... --features=concread/simd_support

Contributing
------------

Please open an issue, pr or contact me directly by email (see github)
