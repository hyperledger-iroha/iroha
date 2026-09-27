# F02 non-Copy charged-buffer foundation, 2026-09-24

Scope: the existing `optimizations` checkout. This is a reusable fixed-backing
allocation owner, not production penalty-index admission.

The existing `mv::allocation::ChargedBuffer<T>` now accepts non-`Copy` elements.
Its original exact `AllocationCharge`, typed layout validation, backing
allocation, and deallocation owner are unchanged. `try_push` moves one element
into previously admitted capacity and returns that same unconsumed element when
the fixed logical capacity is full. The existing borrowed-slice `append` remains
available only for `Copy` elements. Zero-sized elements obey the logical bound
without a backing allocation. Element destructors and backing deallocation run
before the original charge refunds its pool.

Validation: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p mv --test
charged_buffer_custody -- --nocapture` passed 30/30. The new aligned non-`Copy`
test observes exact requested/deallocated layout, return of an undropped
capacity-refused element, destructor counts through truncation and retirement,
and deallocation before the refund wake. The new non-`Copy` zero-sized test
checks capacity, alignment, refusal, and exactly-once destruction without an
allocator request. Direct `rustfmt --edition 2024 --check` on the three changed
Rust files and `git diff --check` passed.

This owner accounts only its backing layout. Nested `AccountId`/`Quantity`
storage, `PublicLaneStakeIndex` maps and share-key vectors, validator locators,
scratch blocks, action vectors, and local-retry custody remain unfunded. No
production Core caller was changed by this slice. F02 and release gates remain
open.
