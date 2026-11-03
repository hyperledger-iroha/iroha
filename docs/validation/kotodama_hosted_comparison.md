# Kotodama hosted component comparison

The `representative-regression` job in `.github/workflows/kotodama_perf.yml`
measures the pinned, untouched baseline and current candidate serially on the
same Ubuntu VM, with the same explicit Rust toolchain and one Cargo build job.
The shared native setup installs dependencies from the `candidate` checkout.
Cached Criterion measurements are cleared before sampling; baseline `new`
slots are cleared before the candidate runs. Source, inventory, original lock,
timed-body, and complete artifact checks remain mandatory.

The approved performance baseline is the `optimizations` commit
`06912ee4f8f9423b458d72039dbd0a1c8fde7b43`, with its original `Cargo.lock`
SHA-256 `1ce796ba4fe8769f4d298a0ce9fe52acff39a012f5ce6cf4323fbd50c2028b55`.
Its three benchmark source files retain all 46 pinned timed bodies and the
complete inventory, including the exact typed-query raw-entity workload.

`scripts/check_kotodama_perf.py` retains the 5% regression ceiling and the
independent zero-slowdown List-sugar median ceiling. Paired revision
qualification also requires the candidate median confidence upper bound divided
by the baseline lower bound (minus one) to fit the 5% ceiling. The List policy
still compares point medians: equality is allowed, and overlapping or touching
intervals are reported explicitly without demanding that sugar be statistically
faster. The report records both List medians, all interval bounds and their
relation. Each Criterion median must have a finite, positive,
enclosing confidence interval at a reported confidence level of at least 95%.
This conservative rule uses individual intervals, not a claimed joint
confidence level for their ratio. Missing or malformed intervals and
inconclusive paired-regression evidence fail without producing a qualification
receipt. Accepted receipts bind these
bounds and all consumed estimates along with source and lock identities.

This is a component regression comparison on one VM. It does not replace the
separate named-hardware numeric calibration or establish release qualification.
The immutable baseline and timed-body policy are not refreshed by CI. A genuine
workload change must receive an explicit performance-baseline decision before
its measurements can qualify; source-baseline approval alone is insufficient.
