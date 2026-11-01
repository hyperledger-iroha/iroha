# Intel arithmetic execution

On 2026-10-31 the current shared `iroha_fhe` kernels executed AVX2 on the Intel
Core i7-11800H under WSL2 (Linux `6.18.54.1-microsoft-standard-WSL2`, Microsoft
hardware virtualization). Rust was `1.99.1 (58cf8f9c3 2026-10-13)`, host
`x86_64-unknown-linux-gnu`, LLVM 23.1.1. The host exposes AVX2 and SHA-NI.

These are component correctness results. They do not refresh the separately
recorded native Windows/MSVC images, qualify bare-metal operating-system behavior,
replace the Xeon Silver 4310 or Xeon Gold 6430 baselines, or qualify a shipping
release, Metal/CUDA or whole-node determinism.

The checkout baseline was `54f4c647e9` plus the current working tree. After the
shared Norito/schema cleanup settled, the focused commands below were rerun.
The 56-file FHE owner/build-input inventory has SHA-256
`55453b176fd498a24743fd1a315575706af897f600fd0d4dc987f1a91ff766a1`;
the 938-file IVM inventory has SHA-256
`5ac544927ea9181cb7049972cf50d63c7faba6a04903b754dbe3cac0e8d4c66d`.
Each covers that crate's owned source/tests/fixtures (including added sources),
root build manifests/configuration and the local build wrapper. These are not
complete transitive-dependency custody inventories. Digests use sorted
`relative-path + NUL + file-sha256 + newline` records.

Full inventories, compiler/host identity, command logs, terminal results and
binary/log digests remain untracked under
`dist/intel-platform/current-20261031/`, in `fhe-evidence.json` and
`ivm-evidence.json`. They are local execution evidence, not signed release pins.

Each command used the prefix
`scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --incremental --jobs 2 --`.

| Cargo command | Result |
| --- | --- |
| `test --locked -p iroha_fhe --lib accel::tests::required_avx2_execution -- --ignored --exact --nocapture` | 1 passed; requires selected AVX2 before executing direct kernel parity checks. |
| `test --locked -p iroha_fhe --lib --test canonical_vectors --test clearing --test rns_chain_vectors` | 171 unit, 9 canonical-vector, 7 clearing and 4 RNS-chain tests passed; the explicit hardware qualifier is ignored in this ordinary suite. |
| Same suite with `--no-default-features` | The same 191 tests passed with scalar production dispatch. |
| Explicit qualifier with `--no-default-features` | Expected rejection: `Scalar != Avx2`, Cargo exit 101. Scalar-only builds cannot qualify AVX2. |

Direct checks cover addition, subtraction, products, scalar products and cyclic
NTT, including modulus boundaries, zeroes, carries, non-lane-aligned tails and
transform lengths through 1,024. Independent arbitrary-precision arithmetic
backs the canonical vectors. The mandatory test refuses a portable lane model
or scalar fallback. Logs use `final-required-avx2.log`, `final-simd.log`,
`final-scalar.log` and `final-required-avx2-scalar-refusal.log`.

## IVM SHA-NI execution

The final-source production-path gate also passed:

```sh
scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --incremental --jobs 2 -- test --locked -p ivm --lib vector::sha256_cpu::tests::required_x86_sha_ni_executes_production_path_and_matches_scalar -- --ignored --exact --nocapture
```

Its isolated child emitted `IVM_NATIVE_SHA256_RECEIPT architecture=x86_64
completed_blocks=462`. The test counts actual native completions, compares
compression states and padded messages against scalar/independent references,
and checks scalar and file-policy opt-outs. The passing binary has SHA-256
`e3101ab6df66c63130a9492a09fddba9d389c9f54e9b4f9ad25439e9268f088a`.
The current log is `final-ivm-sha-ni.log`; compiler warnings were retained, so
this is not a strict IVM lint result.

The native workflow now uses the repository's Rust version, triggers on IVM
changes, requires a nonempty AVX2 result, and requires the SHA-NI receipt on its
Linux/Windows runners. Missing instructions fail that gate. Intel Mac runners
retain their AVX2/scalar scope; SHA-NI support is not assumed.

## Benchmark policy controls

The existing IVM policy is exercised through a benchmark-only helper shared by
the IVM and ISI gas benches. Three grouped integration controls pass, including
invalid-setting refusal in an isolated process. Both the scalar and runtime IVM
smokes pass all 37 cases and emit identical arithmetic gas denominators. The
scalar receipt selects `Scalar` with SIMD/Metal/CUDA disabled; the runtime
receipt selects `Avx512` on this host. Selection is not evidence that the
arithmetic workloads executed vector instructions, and the helper controls only
IVM, not other libraries' dispatch. See the [calibration ledger](../../specs/confidential_assets_calibration.md)
for the exact scope and remaining neutral-profile requirements. These concurrent
development-host smokes provide no timing baseline.

The current ISI fixture also passes all eight Criterion smoke cases under both
policies, with identical per-case gas quotes. Its five `bench_repro` regressions
pass, including immutable genesis authority, domain-owned asset registration,
role delegation and one-shot callback rollback. The command is
`cargo bench --locked --profile dev -p iroha_core --features bench,iroha-core-tests --bench isi_gas_calibration -- --test`,
run once for each policy. The component invocation owner grants no Network or
block-publication authority. Exact commands, executable hashes and emitted
quotes are retained in
`dist/linux-native-consumers-20261031/iroha-linux-isi-gas-final-result.json`.
The final run binds one unchanged image (`8eaaf28e…`) and 2,447 selected inputs;
the eight gas quotes total 33,979 under either policy.
These development-profile checks leave the named Xeon timing baselines open.

## Test-only Goldilocks FFT experiment

The archived AVX2 DIT candidate in `transparent_stark_avx2_test_kernel.rs` was compiled
only for x86-64 tests and left production DIT dispatch unchanged. Two explicitly invoked
native controls pass against independent modular arithmetic and the scalar/FASTPQ
FFT oracles, executing 14,302 and 3,586 four-lane blocks. Edge residues, unaligned
buffers, scalar tails, forward/inverse transforms and invalid inputs are covered.

The registered-shape diagnostic also passes: 312 alternating paired records across
13 fixed/native inverse, forward and quotient-transition cases, widths 1/8 and
8/16 workers. Every output matches the independent oracle; 560,877,696 vector
blocks execute. Disassembly of the called window contains four `vpmuludq`
instructions and YMM arithmetic. The existing log-5 scalar fallback reports zero
vector blocks. Existing public power tables and clearing owners remain in use;
the maximum recorded private matrices total 134,218,624 bytes and public tables
4,194,384 bytes. These allocation values are not process RSS.

The build uses the warmed `csharp-platform` target, default privacy features and
`--profile local-release --config profile.local-release.package.iroha_core_privacy.opt-level=3`;
only the privacy test crate uses optimization level 3, while dependencies retain
level 1. Run the two ignored controls with `-p iroha_core_privacy --lib avx2_test_kernel
-- --ignored --nocapture --test-threads=1`, then the exact ignored
`avx2_registered_fixed_native_fft_reports_exact_parity_and_paired_cost` test.

On this WSL2 development host, two other compiler processes remained active.
Summed candidate/current elapsed ratios over the admitted shapes were 0.888/0.905
for widths 1/8 at eight workers and 0.860/0.920 at sixteen workers. These are
unweighted diagnostic comparisons of complete bounded transform paths, not a
reference baseline or whole-proof speedup. Table construction/drop is timed;
input copying, independent oracle evaluation and private erasure are outside that
interval, with erasure measured separately. The candidate directly indexes the
supplied powers, whereas the scalar generic stage also computes nominal stage
root powers; the comparison does not isolate butterfly instructions alone.

The optimized image has SHA-256
`8375baf88164ef5013fc0457b998ba89101cfad346ce4443115448839ef710bd`.
All 582 selected privacy-owner/build inputs, that image and the production bridge
remained unchanged across that execution. This inventory is not a complete transitive
compiler-input closure. Commands, exact inventories, paired records, disassembly
and limitations are retained under `dist/intel-platform/avx2-fft-20261031/` in
`summary.json`, `opt3-controls-build-evidence.json` and
`opt3-timing-evidence.json`.

## Joined DIF transform

The production full-output CPU owner now admits one shared AVX2 Goldilocks
window after validating the complete batch. Admission requires the existing
`simd` feature, actual AVX2, eight columns, common log22, native log
5/8/15/16/18/19 and every complete `2^native + MASK_DEGREE + 1` coefficient
length. Other valid full shapes retain the original scalar implementation;
smaller, selected-row and device routes retain their existing owners. The field
arithmetic lives beside its scalar owner in `transparent_stark_goldilocks_avx2.rs`.
Duplicated prototype evaluators and recursion have been removed. Scheduling,
masking coefficients, exact output allocations and clearing remain unchanged.

The current default-feature image passes 85 ordinary and seven explicit native
controls. Actual full-eight-column dispatch exercises 375,317,728 vector blocks
across all six registered shapes, with both counted and ordinary outputs matching
the scalar reference. The feature-off image passes 86 ordinary controls,
including the measured full batch against sequential dense oracles; all five
mandatory DIF controls explicitly fail native admission with exit 101. Those
expected refusals provide zero native execution passes.

The test image is `f36563762b284336f4cd4a55201ed2e9292811b35329103395ae8cd085e39632`;
the non-test library is `04e46c0044d1f1e345f194874e1f683eb4c8f3c3a2b221a1427c550c27926c6a`.
Both use the matched local-release owner opt-level 3 configuration, not the
canonical maximum-proof release profile or uniformly optimized dependencies.
The 710 selected source/build inputs, 712 default archive members and 711
feature-off archive members are verified under
`dist/dif-unit-output-controls-20261031/`. The feature-off test image is
`6a81deff009b334e0a3c1c8e1408add588b1eeb57296a72db71d7e5a3d6abf08`.
These inventories are not a complete transitive compiler-input closure.

Compiled non-test inspection confirms the batch gate, native/scalar branches,
bounded canonical YMM arithmetic and original clearing paths, with no test
counters. A sealed compile-time output policy now gives ordinary calls a unit
result in both test and production builds; explicit controls retain actual
counts. The shared full-output owner has a typed unit-result regression covering
dense/Horner parity, original allocation transfer and clearing. Inspection of
the called window, recursive joins/jobs and full-owner success return confirms
that the old counter-result transport is gone. The window and unit join/job
instruction shapes match production after address normalization. The recursive
caller still differs: iterator callback preparation reserves 16 additional local
bytes in the test image. Other outer test receipts and observation hooks remain,
so this is not whole-body or whole-libtest equivalence. Neither inspection nor
these controls establish runtime cost, total stack/RSS or register erasure.
The separate [integrated maximum run](maximum_x509_intel.md) verifies its proof
but still exceeds the unchanged time and phase-detail limits. Earlier component artifacts remain preserved in
`dist/intel-platform/production-dif-controls-20261031/` and
`dist/csharp-production-dif-20261031/feature-off-privacy-attempt1/`.
The native workflow now requires five
DIF passes, three feature-off passes and five explicit native refusals on all
three x86-64 runners; actual CI execution and runtime within its unchanged
60-minute limit remain unqualified.

The full-output joined transform uses a different, prefix-pruned DIF path from
the DIT experiment above. The preceding archived test-only trial replaced its butterfly
window with four adjacent AVX2 field operations. It preserved the original
1,024-pair windows, recursive prefix bounds, Rayon scheduling boundaries, bit
reversal and eight output allocations. It added no power table or wider batch;
that trial itself did not alter production dispatch.

Four required native controls pass, including independent modular arithmetic,
full common22 scalar/dense equality at all six registered native logs, refusal
ordering, original allocation transfer and populated error/unwind erasure.
The two original AVX2 controls and six ordinary full-coset controls, including
disabled-native fallback, also pass. Linked-image inspection confirms actual
YMM arithmetic, original scalar paths and allocation clearing. At least 472 bytes
of immediate nested frames are visible; this is not a total stack/RSS bound or
a register/spill erasure guarantee. The original 1 GiB runtime reserve remains.

One balanced run produces 96 records: native logs 5/8/15/16/18/19, common log22,
width eight, eight/sixteen workers and four alternating rounds. Each arm clears
and drops its output before the next; only one eight-column output matrix is
resident. Exact output equality is established separately; timing checks public
synthetic output digests outside the work interval and times cleanup separately.
Candidate counter reductions remain timed, without per-window/block atomics.
The work-plus-cleanup sum ratio is 0.917399. All twelve case sums improve, but
eight of 48 individual pairs regress; pair ratios range from 0.604714 to 1.065176.
Both execution orders improve in aggregate. Equal case weighting is not the
actual maximum-proof workload, so this establishes no whole-proof gain or
300-second qualification.

The exact local-release privacy opt-level 3 image is
`a4fa6ef9a7f3aabcc8c37d164aefc729255c48bb2ad6b433a4a8993b28009bbc`.
All 587 selected inputs and the image remained unchanged through that execution;
their archived copies are preserved. This is not a complete transitive
compiler-input closure. Actual candidate counts
are 3,002,541,824 vector blocks and 1,610,612,736 scalar pairs. Raw records,
source/image copies and independent reviews are retained in
`dist/intel-platform/dif96-20261031/`. Its final inventory SHA-256 is
`c263e3a37a1173e0da25f9b2cfd219f06ef2af80a38d83837095da27b7b0068b`.
The original wrapper's overly broad record selector refused parsing; a separate
exact-prefix parser validates the same successful executable run without a
rerun. Both receipts remain intact. The later production integration and its
separate controls are described above; the trial does not qualify whole-proof
performance of that integrated source.

## Test-only SHA3-384 row absorption experiment

The four-row AVX2 candidate in `privacy_outer_hash_avx2_tests.rs` is test-only;
production row-hash dispatch is unchanged. It uses canonical row-stream constructors and
finalizers, supported ephemeral 201-byte SHA3 state serialization, a 104-byte
rate, 48-byte digests and all 24 Keccak-f1600 rounds. Three native controls pass
(all cursor positions, rate crossings, scalar tails/fallback, canonical row roots
and frontiers, pre-mutation refusals and clearing on success/error/unwind), with
2,080 and 8,496 counted four-state permutations. A separate ordinary test passes
for the current registration geometry and resource-plan funding.

The matched local build sets optimization level 3 for `iroha_core_privacy`,
`sha3@0.12.0`, `keccak@0.2.2` and `sponge-cursor@0.1.0`; other dependencies retain
`local-release` level 1. It uses 64 codegen units, so it is not identical to the
frozen maximum-proof release image. Actual linked caller inspection confirms the
baseline's scalar Keccak permutation and the candidate's YMM AVX2 kernel.
The earlier dependency-level-1 run is correctness evidence only.

The complete diagnostic passes in 222.65 seconds: 48 records, 24 alternating
pairs and 179,712,000 four-state permutations, with exact output equality.
Rows 4,096/262,144 and workers 8/16 use source-derived widths 3,213/2,618 and the
actual 404/329 update boundaries. The performance result is negative: at 262,144
rows, summed candidate/current absorption ratios are 1.697/1.700 for base/aux at
eight workers and 1.582/1.582 at sixteen. Every paired absorption comparison is
slower. The experiment does not isolate which included cost dominates.

Only `absorb_ns` compares performance. It includes canonicality checks, big-endian
packing, state conversion, transposition and temporary-owner cleanup; input
generation and input-column cleanup are excluded for both paths. Finalization
and total times are informational because candidate leaf finalization is serial
and the current builder is parallel. The Linux native observer suite remained
active; ten-second load/process samples are retained. This is neither an idle-host
baseline nor a whole-proof speedup or qualification result.

The 3,057-byte named Rust scratch calculation is not a total stack bound.
Disassembly shows immediate reservations of 424/3,512/1,160 bytes along the nested
packing/adapter/kernel chain: at least 5,096 bytes before pushes, call frames,
alignment, other spills and higher callers. The 4 KiB check applies only to named
objects. The existing 1 GiB runtime reserve and proof caps are unchanged; no
complete stack, process-RSS or private-register-erasure claim is made.

The final image SHA-256 is
`819978f1999d168d8567afebc57f0fc8cf9522f1b038aeadc88f18b4c70d023a`.
All 583 selected privacy-owner/build inputs, the image and production bridge
remain unchanged across execution; this is not a complete compiler-input closure.
Exact commands, inventories, paired records, assembly and limitations are retained
in `dist/intel-platform/sha3-avx2-20261031/summary.json` and its referenced artifacts.

## Test-only persistent SHA3 byte absorption

A second test-only candidate retains four SHA3 states between updates. Its measured
864-byte, 32-byte-aligned packet includes 28 explicit padding bytes and replaces
exactly four existing 216-byte row charges. It allocates `floor(rows/4)` packets
plus exactly 0–3 ordinary scalar rows; forced scalar mode retains only the current
scalar owners. Canonical constructors/finalizers remain authoritative. State
conversion occurs once at each boundary, and retained slots, padding and temporary
owners clear in place. Failed finalization poisons and clears the collection;
expected update preflight refusals remain retryable. Production dispatch and the
existing funding caps are unchanged.

All six native controls (the original three plus three persistent-owner controls)
and the separate geometry control pass. New controls cover all 104 cursors,
1–3 scalar tails and global row counters, exact capacities and padding, partial
initialization, query/geometry/underrun refusals, poisoning and unwind cleanup.
They execute 28,015 rate-control and 8,496 canonical-row four-state permutations.
Actual allocation checks cover rows 1–9, 64 and 1,024; registered powers and the
maximum common row count receive checked capacity-arithmetic coverage only.

The matched-profile diagnostic passes in 200.29 seconds: 48 records, 24 pairs,
179,712,000 four-state permutations and 3,194,880 conversions at each boundary.
Its byte-at-a-time absorption remains slower in every pair. At 262,144 rows,
summed candidate/current `absorb_ns` ratios are 1.262/1.238 for base/aux at eight
workers and 1.166/1.192 at sixteen. This compares root-only absorption; the
candidate supplies roots/frontiers, not the builder's opened-row collection.
Boundary conversion is recorded in initialization/finalization, and only
absorption is a comparative metric. Finalization/total timings remain
informational; the native Linux observer run was active and its process/load
samples are retained. These results do not establish a whole-proof speedup.

Disassembly confirms the actual scalar and native call paths and explicit clearing.
The 416-byte named native-update scratch is not a total stack bound: immediate
nested reservations are 472 + 72 + 1,096 = at least 1,640 bytes, plus uncounted
pushes, call/helper/Rayon frames, alignment and other spills. No complete
stack/RSS or register/spill-erasure qualification is claimed.

The image SHA-256 is
`95de512e0b17eaf052b4d7f8a44b5f072cac3ccb3f45d657145207e065634e24`.
All 584 selected inputs, image and bridge stayed unchanged during execution.
The exact executable and changed test sources are preserved with the negative
result in `dist/intel-platform/sha3-persistent-20261031/summary.json` and its
referenced artifacts. The initial compile's denied instrumentation casts and the
subsequent lint-preserving correction are retained separately.

## Test-only persistent SHA3 word absorption

The final narrow variant XORs existing packed bytes using a short byte prefix,
aligned sponge-word `u64::from_le_bytes` updates and a byte suffix inside each
104-byte rate segment. Framing, scalar tails, kernel rounds, packet funding and
clearing remain unchanged. All six native controls and the separate geometry
control pass; the existing all-cursor/rate/tail cases retain their exact counts.
Disassembly confirms the word loads/XORs, actual scalar/native calls and clearing.
The 416-byte named scratch and at-least-1,640-byte nested immediate stack
reservations retain the limitations above.

The complete 48-record/24-pair run passes in 187.76 seconds with exact parity,
179,712,000 four-state permutations and 3,194,880 conversions at each boundary.
At 4,096 rows, summed candidate/current absorption ratios range from 0.960 to
0.983. Every 262,144-row pair remains slower: summed base/aux ratios are
1.093/1.079 at eight workers and 1.076/1.070 at sixteen. The result provides no
basis to promote this path for the maximum workload. Only root-only absorption
is compared; boundary/finalization/total costs remain separately recorded and
informational, with the same observer contention and qualification limits.

Image `1d03082892aca354c7783c28224fe981f626b2955178704b2c021ed09df9ce77`,
all 584 selected inputs and the production bridge stayed unchanged during the
run. The exact executable, source snapshots, command/profile, independent reviews
and all records are retained in `dist/intel-platform/sha3-word-20261031/summary.json`.
These SHA3 experiments have not changed production row-hash dispatch.
