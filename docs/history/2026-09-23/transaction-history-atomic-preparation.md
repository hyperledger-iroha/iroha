# Transaction-history atomic preparation slice

The current `optimizations` source gives one original
`TransactionsStorage::prepare_next_block` attempt a checked reservation for the
previous-tip `ChargedBuffer<Key>` backing, next charged identity shell, and
Concread's actual no-edit writer demand. Concread supplies that cursor demand
under its original writer lock before constructing its cursor and reader shells.
The same immutable `kura.transaction_history_bytes` pool funds all three;
configuration defaults and limits are unchanged.

The batch and identity split their concrete layouts from that reservation, and
the remainder funds the Concread cursor. The identity shell is now allocated
fallibly before batch copying or sorting. Pool-capacity refusal constructs none
of the three and retains the same pending predecessor and prior tip for retry.
An allocator refusal refunds unused credit while preserving any already built
pending sibling. Later private-tree inserts still use their existing separately
admitted exact layouts; this change does not fund a whole future block at once.

Focused tests cover one-byte-short aggregate refusal with no partial allocation,
release-driven retry against the original predecessor, hidden private promotion,
and the exact retained/refunded byte sum for an empty successor. Formatting has
passed. The focused command
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib state::storage_transactions::history::tests:: -- --nocapture`
passed 18/18 tests on the corrected checkout. The controlled unwind tests print
their expected panic messages and still pass.
The adjacent `state::storage_transactions::tests::` suite passed 18/18, and
`transaction_history` passed 10/10 restore, Kura-pool and history controls on
the same compiled source.

This is a local F02 resource slice. It does not charge the latest-tip `HashSet`,
snapshot parse/comparison scratch, Queue retained memory, Kura disk side effects,
or the full retained carrier. Concread's prepaid cursor-shell allocation still
uses its existing process allocator policy after successful quota admission.
Native Validate-to-Apply ownership and production/network qualification remain
open.
