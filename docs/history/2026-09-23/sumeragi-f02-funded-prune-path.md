# F02 funded committed-evidence prune path

2026-09-23, `optimizations`. This record describes a bounded production source
slice. It does not close F02 or qualify the multilane release.

The parent-State committed-evidence prune planner now reserves one fixed
`ChargedBuffer<Hash>` before opening `StateView`. It scans borrowed rows,
checks the existing 124-record, 4 MiB per-proof and 16 MiB aggregate caps,
and stores only terminal out-of-horizon keys. It sorts those keys in place and
retains the same charged buffer through ordinary, Native and merge-beacon
pristine application. The merge-beacon capability moves the original buffer
without replacing its allocation or charge. Malformed over-capacity tables
yield no partial prune plan, preserving the previous planner's behavior.
Incoming admissions never evict replay fences.

Candidate evidence-admission validation now scans committed rows by borrow
under its existing State view. It checks the same row and proof-byte limits,
retained capacity, replay keys and offender-roster fences without cloning up
to 124 complete nested proofs into an intermediate snapshot. A new regression
fixes the error ordering: at 123 rows a repeated key is rejected as already
committed; at exactly 124 rows an empty admission passes while one more is
table-full; row 125 is table-full even for an empty admission. The separate
snapshot helper remains for local proposal selection and penalty derivation.

Evidence-key and offender-roster key derivation now stream the exact domain
prefix and bare Norito encoding into the incremental Blake2b hash writer.
This removes the full encoded-proof buffer and its second concatenated
preimage buffer from evidence keys, plus the encoded-roster buffer from roster
keys. A three-variant regression compares proposal, phase-vote and timeout
digests with the prior encoded-preimage formula; canonical key bytes are
unchanged. These hash operations do not introduce a new allocation refusal.

Local evidence retention now checks committed row and byte caps, replay keys,
and offender-roster fences through a borrowed State view before locking the
pending table. It no longer clones the complete committed proof table or
constructs a prune-key set for local ingress. The view is dropped before the
pending lock, preserving the State/Kura and pending lock order. A regression
accepts the exact 124-row boundary and rejects row 125.

`nexus.storage.consensus_evidence_preparation_bytes` configures a separate
process-local pool. One maximum prune backing is 124 × 32 = 3,968 bytes; the
finite default funds eight simultaneously retained plans (31,744 bytes).
The user config rejects a pool below one plan. State installs the configured
pool before startup replay, preserves its identity across unchanged config and
isolated replay publication, and refuses a changed limit while charges are
live. Ordinary runtime catalog updates retain the configured pool limit.
Cold snapshot restoration initializes the pool from the restored Nexus storage
policy; isolated replay publication takes the original live State pool handle.

Capacity refusal carries the original pool's release observation to the Apply
service as local retry. Layout/allocator and permanent-limit refusal require
local recovery. None produces a block rejection event or consensus-invalid
transaction result. Focused tests cover ordering, horizon boundaries, row and
proof-byte limits, exact and short pools, eight simultaneous owners, config
replacement, replay-pool identity, and error classification. The focused Core
`funded_prune_scan` selector passed 2/2 with
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib funded_prune_scan -- --nocapture`.
After the admission edit, a fresh Core build ran
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib borrowed_admission_scan_preserves_table_and_duplicate_precedence -- --nocapture`
and passed 1/1. Its direct `sumeragi::evidence::tests::` selector passed
35/35. After the streaming-hash edit, a fresh Core build ran
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib streaming_evidence_and_roster_hashes_match_encoded_preimages -- --nocapture`
and passed 1/1; its direct evidence selector passed 36/36. Before the
admission edit, direct selectors from the preceding Core binary
passed `state::runtime_configuration_tests::` 9/9, the merge-beacon move-owner test
1/1, `sumeragi::v2_apply::tests::native_preparation_errors::` 5/5,
the bridge suite 77/77, and the Native pre-payload timeout, staking
cancel-evidence, and overdue-pending evidence snapshot restore tests 1/1 each.
The configuration fixture selector passed 1/1, and
`cargo test --offline --locked -p iroha_config --test iroha_config_integration`
passed 243/243. These are scoped checks; neither full F02 resource-admission
and restart qualification nor the broader release gate has passed.

After the local-retention edit, the combined role-11/F02 Core test binary
compiled with `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core
--lib sorafs_stream_token_authority -- --nocapture` (role-11 selector 2/2).
Its direct `sumeragi::evidence::tests::` selector passed 37/37, including the
new exact-boundary local-retention regression 1/1. This does not qualify the
remaining admission or recovery paths.

Proposal selection and `PenaltyApplier::derive_from_stable_parent` still clone full evidence records,
including nested proofs, without exact allocation funding. The admission
result vector, canonicalization and offender-roster set also still allocate
without exact charges. Candidate-builder local refusal routing for future
funded projections is unfinished. Nested roster, proof, index, scratch and
action allocations still need funded owners before acquisition; encoded proof
caps do not reserve those layouts.
F02 remains open until those and the other resource owners are funded and tested.
