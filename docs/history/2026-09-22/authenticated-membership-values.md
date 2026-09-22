# Authenticated transaction-membership values

The original membership-root reader now returns a usable block height. It first
authenticates the complete tree path against the retained current or rollback
root, then loads and hashes the exact canonical nonzero u64 height preimage.
It checks the height against that cut's frontier and converts to the local
platform size with checked arithmetic. Only authenticated tree absence returns
`None`. Missing values, corrupt values, missing/corrupt nodes and storage failures
remain distinct local errors; they cannot establish transaction nonmembership.

Cold and incremental preparation retain canonical heights before writing leaves
which reference them. The caller's original store and fixed update workspace
remain borrowed through errors and unwind. Failed writes may leave immutable
unreachable records but cannot publish a candidate or alter the old roots.
Replacement restores shadowed heights from its actual original State transition;
unavailable physical height data is never interpreted as an absent member.
The key, height and current/rollback-root commitments retain their exact bytes.
There is no compatibility reader or alternate production encoding.

The complete Core library test target compiles without warnings. All 77 current
transaction-membership controls pass, including the 15 root/value controls.
Core's non-test library check passes with the existing warning for two unchanged
unused State cache helpers. These checks share the same 7,556 captured Rust inputs.
Tests cover present/absent current and rollback reads, zero/wrong/frontier-invalid
preimages, node/value I/O failures, every cold/prepared height-write refusal,
and unwind after earlier changed-key node paths have persisted. Later unwind
cuts retain both original roots and bindings and retry the same preparation.
These controls use the actual membership publication owners with a test store;
opaque alias keys do not replace sealed-transaction authentication coverage.

The first build found three E0716 errors in one new test closure. The correction
adds an explicit borrowed-key parameter type; no production behavior or assertion
was relaxed. Independent read-only review found no concrete defect and prompted
the later-write unwind coverage. Complete commands, source hashes, executable,
failed/successful build logs and structural/hygiene receipts are retained under
`dist/sumeragi-main-work/generation163-core/`. The preceding generation162 final
executable is preserved in a lossless archive with its verified original hash;
its earlier counts remain attached to that earlier candidate.

This is a prerequisite for the [original membership-root publisher](membership-root-publication.md),
not activation of a durable store or the complete State publisher. The physical
storage design must use explicit root/child locations, authenticate each loaded
record, and retain exact segment generations for current, rollback and old-reader
roots. A physical leaf carries the canonical height or its explicit location.
Hash-only node and value interfaces otherwise require a disk content index;
a bounded cache must not disguise unindexed history as missing storage.
Generalize the single traversal/update kernel for these explicit locations and
keep them out of logical commitments. Fund complete write batches and fixed I/O
workspaces before allocation. Install prepared roots inside the sole original
publication kernel before releasing its writer, then complete authenticated
restore and exhaustive incremental State checkpoints. Both production Apply
checkpoint sites still materialize full history. Complete production cutover and
four/seven-validator fault/restart/final-transaction qualification remain open;
no L1–L6 completion is claimed.
