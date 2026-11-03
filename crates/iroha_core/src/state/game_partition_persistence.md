# Native game-partition persistence boundary

`World.game_partitions` is an authoritative canonical table that the release
addition `iroha.game_partition.v1` adds after the frozen baseline
(`crate::runtime::catalog::additions`, `specs/runtime_upgrades.md`). Until a
governed change set activates the addition, its six instructions resolve as
`definition_inactive`, the table stays empty and uncommitted, and snapshots
leave it out; a restore under such a selection reads it empty, exactly as a
release without the addition does. Activation installs the table empty.

Once installed, the table is included in the ordinary World transaction
overlay, apply/rollback lifecycle, canonical table capture and complete World
state commitment. It is not a client cache or a replacement for source-root
finality authentication. A snapshot whose selection installs the table must
carry the field, even when the table is empty. Restore validates the current and
retained predecessor cuts independently, without consuming the actual MV undo
journal. Both cuts enforce the world-count bound, exact table key/world identity
equality and the record's custody/sequence/retention checks. These local
consistency checks do not authenticate a snapshot by themselves.

TODO(`specs/live_runtime_upgrades.md` §7): an export removes the unit at its
source while the destination's import is still pending, but runtime obligations
track only escrow and AMX State. In-flight exports must become live obligations,
so admission closure, completion-only restrictions and retirement cannot strand
an import, before governance activates the addition on a live network.

`deserialize_world_game_partition_tests.rs` covers exact current/predecessor
roundtrip, the member's selection-dependent presence (required once installed,
left out and restored empty before), substituted keys, invalid watermarks in
either cut, the world-count bound, abandoned writes and canonical projection
sensitivity to every unit field. Real independently signed root transfer and
restart evidence belongs to the multi-validator integration test, not these
state tests.
