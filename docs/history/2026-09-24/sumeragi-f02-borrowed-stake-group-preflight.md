# F02 borrowed stake-group preflight, 2026-09-24

Scope: the existing `optimizations` checkout. This is a source-integrity and
resource-ordering cut inside `PublicLaneStakeIndex::from_world`, not full stake
or execution admission.

The stake index already counts its flat share and group backings and nested
account copies before reserving the original State pool. It previously looked
up each `(lane, validator)` record only after that reservation and the first
group key clone. An orphan share group or a matching record with an invalid
storage-key/stake-account binding could therefore be masked by a capacity
refusal and trigger a local retry of malformed committed source.

The borrowed demand pass now advances a second canonical validator iterator
only when it encounters a new share group. It requires the matching validator
record and validates its key/stake-account binding before adding group demand
or making any stake-index pool reservation. The cursor uses no cloned lookup
key, retained collection, or additional pass over the share table. The
materialization lookup and final validator-total checks remain, so a source
change between the demand and fill could not silently become an accepted
index. Empty and unrelated validator records retain their later validation;
this cut does not fund the world table's source lookup, `Quantity` limbs,
arithmetic scratch, or per-member execution work.

The orphan and foreign-stake-account tests now invoke the index directly with
a zero-byte stake-index pool and require the specific source-integrity refusal
before a capacity refusal. They also retain the parent-snapshot and unchanged
source checks. On this checkout, the Core `stake_index_demand_` selector passes
8/8, `parent_snapshot_rejects_` passes 5/5, the two-pass exact-exposure test
passes 1/1, and the original-owner capacity-refusal/retry test passes 1/1.
These are focused results, not a release or F02 gate closure.
