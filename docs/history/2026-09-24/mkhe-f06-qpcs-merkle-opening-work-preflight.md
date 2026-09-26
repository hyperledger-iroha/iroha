# F06 qPCS Merkle-opening work preflight, 2026-09-24

This `optimizations` slice promotes the existing exact six-lane payload,
indexed-leaf and node-frame work inspection to production-compiled private
code. A new private `RnsNativeQpcsOpeningHashWorkV1` computes a conservative
work charge for one already-shaped Merkle multiproof: every opened payload and
index frame, plus at most one ancestor per tree level for each opened leaf.
It debits only a supplied `RnsNativeProofResourceBudgetV1`; it cannot create or
reset a budget, issue a receipt, or authorize composite admission. For 320
initial leaves the charge is 4,597,355,520 field additions/multiplications;
for all four terminal leaves it is 25,040,328. The charge deliberately does not
discount public-leaf cache hits.

The original budget is currently owned by
`GlobalLookupCommitmentSessionLiveV1::proof_resources`, created at the
original materialized-source entropy handoff. The later
`RnsNativeCompletedQpcsSourceReadV2` and
`RnsNativeCrossFieldRlweCompositeInputV2` do not retain it. Their qPCS
authentication calls therefore cannot use this preflight without a genuine
move-only owner transition. The new helper has **no production caller**. Its
scope also excludes query-opening rehashes, continuation and transcript hashes,
FRI arithmetic, source replay, and whole-session memory/spool/I/O. It is a
resource-admission prerequisite, not a complete qPCS meter or a redesign.
The current initial-tree construction still costs 3,032,072,370,498 tracked
operations against the unchanged 128,000,000,000 whole-proof ceiling and
rejects before source access. Production composite admission remains closed.

Focused validation against the combined source:

- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_zkp_halo2 rns_native_qpcs_opening_work -- --nocapture`: 3 passed. Exact-cap and one-over checks share the supplied ledger, overflow rejects, and invalid oracles/counts reject.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_zkp_halo2 rns_native_qpcs_tree::tests -- --nocapture`: 7 passed, including refusal before source access and work preservation on failure/unwind.
- Scoped Rust formatting and `git diff --check` passed.

Next integration must retain the original resource owner through source read,
qPCS proof authentication and composite verification, add **all** qPCS hashes
and arithmetic to the same additive whole-proof meter before hashing, and
resolve the independently reviewed source-bound commitment/opening redesign.
No limit, wire version, fallback or release gate changes in this slice.
