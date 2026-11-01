# Native game-partition persistence boundary

`World.game_partitions` is an authoritative canonical table. The table is
included in the ordinary World transaction overlay, apply/rollback lifecycle,
canonical table capture and complete World state commitment. It is not a
client cache or a replacement for source-root finality authentication.

Snapshot restore requires the field even when the table is empty. It validates
the current and retained predecessor cuts independently, without consuming the
actual MV undo journal. Both cuts enforce the world-count bound, exact table
key/world identity equality and the record's custody/sequence/retention checks.
These local consistency checks do not authenticate a snapshot by themselves.

A snapshot written by a binary that predates this table is rejected if the
field is absent. This prototype supplies no automatic snapshot upgrade,
empty-table fallback or production upgrade qualification. An existing network
needs an authenticated migration under its governed execution selection, or
replay of the original authenticated journal with the appropriate selected
semantics. Merely injecting an empty JSON field does not provide that authority.

`deserialize_world_game_partition_tests.rs` covers exact current/predecessor
roundtrip, missing-field rejection, substituted keys, invalid watermarks in
either cut, the world-count bound, abandoned writes and canonical projection
sensitivity to every unit field. Real independently signed root transfer and
restart evidence belongs to the multi-validator integration test, not these
state tests.
