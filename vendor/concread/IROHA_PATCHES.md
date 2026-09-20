# Iroha allocation ownership patch

This is the source of the locked `concread` 0.5.10 crate (crates.io archive
SHA-256 `6588e9e68e11207fb9a5aabd88765187969e6bcba98763c40bcad87b2a73e9f5`),
with its MPL-2.0 license retained in `LICENSE.md`.

The local EBR change binds typed resource custody to the real allocated generation,
including abandoned writers and epoch-delayed reclamation. It does not measure
nested payloads or establish an aggregate Iroha memory quota. Consumer regression
tests live in `crates/mv/tests/ebr_allocation_custody.rs`.

Unpublished EBR generations can also detach and reacquire their exact original
allocation without cloning or allocating a successor. This low-level transfer
grants no semantic predecessor authority; the MV layer authenticates the exact
original current/undo pair while holding both writers. Its acquisition wrapper
also reports poison after a raw clone panic that occurs before a writer guard
can be returned. These paths are covered by allocator and MV regressions.

Synchronous B+tree writers now retain their original cursor through detach/adopt.
Because that cursor shares untouched nodes, its move-only owner retains both the
exact base reader and a shared root owner. Adoption rejects a foreign root or
changed base while holding the original writer mutex. Initial acquisition also
preallocates the next reader Arc shell; publication initializes and links it
without allocating a replacement shell. Detached work drops before its base and
root. Existing node transfer still occurs only in `pre_commit`. Black-box controls
live in `crates/mv/tests/map_owned_generations.rs`. Node/nested/cursor charges and
complete State resource admission remain separate unfinished work.

The synchronous cursor box is allocated during initial writer acquisition and
moves unchanged through handoff. Reader links and B+tree retirement vectors use
write-once storage; the permanent reader mutex is initialized at construction.
This removes observed macOS lazy native mutex allocation during commit without
prewarming locks after ownership transfer. Tests cover a retained reader and a
fresh map's first commit with no prior read, retaining strict zero-allocation
assertions for both paths.
