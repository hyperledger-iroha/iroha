# Kotodama hosted component comparison

The `representative-regression` job in `.github/workflows/kotodama_perf.yml`
measures the pinned, untouched baseline and current candidate serially on the
same Ubuntu VM. Separate `prepare-baseline` and `prepare-candidate` jobs compile
all three original feature selections with Rust 1.99.1, one Cargo build job,
incremental compilation disabled, and the original warning policy. Their
360-minute budgets cover compilation independently. The required
`kotodama-artifact-gate` job builds the canonical release tools and runs the
complete documentation and artifact checks within its own bounded budget.

`scripts/kotodama_perf_bundle.py` forces fresh compilation of each selected
benchmark while preserving warm dependency artifacts. A temporary Cargo runner
captures the actual executable, arguments (including `--bench`), package working
directory, and a closed allowlist of Cargo and loader environment fields. It
does not execute the benchmark or collect samples. Each three-benchmark bundle
binds the exact workflow run and attempt, source tree and original lock,
compiler and build environment, installed native package versions, selected
Cargo artifacts, executable bytes, and resolved runtime libraries. Compiler children use a closed environment too;
unreviewed compile controls and host-native tuning are refused. Bundles contain
only the fixed executable and provenance inventory, with no Criterion results.

The measurement job downloads the two exact same-run artifact IDs and admits
both complete bundles, including fresh query compiler receipts, before timing.
It retains the same absolute workspace layout and serially installs each
admitted executable into its original Cargo output slot, preserving the full
captured launch arguments and directory. Separately archived baseline and
candidate bytes cannot overwrite each other's evidence even when Cargo uses the
same output filename. Executable, source, compiler, and library identities are
checked again after measurement. These are ordinary CI build artifacts, not
signed release artifacts.

Cached Criterion measurements are cleared before sampling; baseline `new`
slots are cleared before the candidate runs. All six benchmark launches remain
serial on the measurement VM, with unchanged sampling and all 46 workloads.
Only successful comparison runs proceed to the complete-evidence seal and upload,
which includes preparation manifest hashes and artifact IDs. Failed or cancelled
runs attempt a separate, explicitly nonqualifying diagnostic upload retained for
14 days; its five-minute bound or host shutdown may prevent delivery. Preparation
bundles have three-day retention and never substitute for fresh measurements.

The approved performance baseline is the `optimizations` commit
`06912ee4f8f9423b458d72039dbd0a1c8fde7b43`, with its original `Cargo.lock`
SHA-256 `1ce796ba4fe8769f4d298a0ce9fe52acff39a012f5ce6cf4323fbd50c2028b55`.
Its three benchmark source files retain all 46 pinned timed bodies and the
complete inventory, including the exact typed-query raw-entity workload.

Typed queries use `scripts/kotodama_query_driver.py` to derive a measurement
driver outside each clean checkout. The reviewed derivation selects the typed-query
main dispatch and grants the existing `typed-query-authority` fixture reader
exactly `CanReadAllLedgerData` before constructing the raw-response baseline or
entering any timed loop. Both revisions receive the same grant through their
own native state implementation. The entire original fixture is hash-pinned;
every other setup byte, all raw and typed page cardinality assertions, and all
46 timed bodies remain unchanged. Receipts bind both original and derived
fixture hashes and identify the added permission. No original source file or
lockfile is edited, and a missing, changed or additional grant is refused by the
exact derivation check. The v3 receipts separately bind fresh compilation and
the successful launch of those exact archived bytes.

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
