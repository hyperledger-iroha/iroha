# Wallet and X509 kernel validation, September 28

This is scoped implementation evidence from the shared checkout. It does not
qualify a complete credential proof, a packaged native SDK, or release activation.

## JavaScript wallet boundary

From `javascript/iroha_js`:

```sh
node --test test/confidentialProver.test.js test/confidentialProofCardinality.test.js
```

All 13 checks pass. These exercise the asynchronous public API, exact amount and
input cardinality checks, automatic relation selection, deferred native success,
worker rejection, disposal during queued work, immediate FFI-key cleanup, output
root/cardinality checks, disposal during input normalization, non-Error input
failures, and the TypeScript promise contract. Native bindings are
mocked in these boundary tests. Native Rust worker and packaged-addon execution
are separate checks; their results must not be inferred from these passes.

## Native wallet worker

```sh
cargo test --locked --offline -p iroha_js_host --lib -- \
  confidential_wallet confidential_proof_boundary_tests --nocapture
```

All nine checks pass (31.01 seconds after a 4m24s build). These include strict
network/key/note parsing, failed-input cleanup, consuming a worker job exactly
once, note derivation parity and redacted Debug output. The positive worker test
produces a real full-redemption proof and checks its self-verification, public
relation and cardinalities. This directly executes the native Rust task on a
worker thread; it does not exercise a packaged N-API addon or its JavaScript
event-loop integration. The local log is
`/tmp/iroha_js_wallet_20260928_final.log`.

A separate transaction-finalizer regression initially failed an obsolete error
message assertion: the canonical ordinary decoder now rejects the genesis domain
before the finalizer's later check. Its assertion now checks that exact decoder
failure, retaining the independent retired-admission and signature controls. The
fresh retry passes (1 check, 0.05 seconds after a 3m56s build), recorded in
`/tmp/iroha_js_finalizer_20260928_retry2.log`.
The same freshly rebuilt host binary then passes all nine wallet controls again
in 32.81 seconds (`/tmp/iroha_js_wallet_20260928_current.log`).

## Source and geometry contracts

```sh
python3 -m pytest \
  scripts/tests/halo2_backend_02_compaction_source_test.py \
  scripts/tests/halo2_backend_shared_circuit_source_test.py \
  scripts/tests/check_note_stark_profile_constraint_dedup_test.py \
  scripts/tests/check_zk_x509_proof_geometry_test.py \
  scripts/tests/zk_source_tokens_test.py -q
```

All 21 checks pass. Source contracts preserve substantive circuit assertions;
the geometry checks derive the codec ceiling without producing a valid proof.
The latest same-selection rerun passes in 2.69 seconds.

The complete renamed CLI graph suite also passes all ten controls with
`PYTHONPATH=pytests/scripts python3.12 -m unittest test_taira_release_check_cli_graph -q`
in 1.135 seconds. The earlier four fixture-boundary failures no longer reproduce
on this source. Python 3.9 cannot collect this suite because its standard library
lacks `tomllib`; the result uses the required newer interpreter.

## Python wallet boundary

`python3.12 python/iroha_python/tests/confidential_wallet_test.py -v` passes
seven mocked controls covering the three relation workflows, full-capacity
one-note forwarding, path/cardinality bounds, close/copy/pickle behavior,
redacted representations, typed native failures and malformed public outputs.
The new native owner uses bounded concrete-container parsing and clearing note
owners, then delegates to Core while releasing the GIL. Real native tests,
including a proof and second Python thread progress, are pending. These boundary
results do not qualify an installed Python extension or a complete proof.
The combined Python boundary and migrated NetworkId contract selection passes
all 13 checks in 0.63 seconds. The four superseded caller-key proof builders,
exports and duplicate note parsers are removed without compatibility aliases.
The independent note-derivation helpers now decode directly into clearing
private buffers; native parity and rejection tests are included in the pending run.

## Isolated native quotient and ownership kernels

Six optimized Rust tests pass using the repository's actual Goldilocks/Fp4 and
FFT helpers, private-buffer owner implementations, and the included quotient
stripe and private-owner test modules. The temporary harness supplies only the
surrounding error namespace. Tests compare folded stripe evaluation to independent
Horner evaluation and a complete coset, check actual log-19/log-22 masked tails and
native-next translation, reject invalid geometry/field words, and verify clearing
and consuming allocation handoff. Runtime: 0.07 seconds.

| Included source | SHA-256 at execution |
| --- | --- |
| `stark/main_quotient_stripes.rs` | `4c543e61936fc355aec74f63d7410b8f476225c6a41ed2348cfcd1af11be6325` |
| `stark/main_secret_ownership_tests.rs` | `c4e0faf5b2094a7f3fc2d88b306baff70920c553a9ef6379c56b444b8d3121d9` |
| `transparent_stark.rs` helper source | `ceba00b1f6f26330663db4e5b46934eed3ff287d36eab4498a76c2c49d562da2` |
| `stark.rs` owner source | `354846b6336e9e495bc0bc7339bcc9f6c60caa7d120dae7c7e2bb71e2639e525` |

The local harness and receipt are under `/tmp/iroha-zk-stripe-kernel-20260928`;
they are not repository build prerequisites. This isolated result does not prove
that Core links or that the complete source/replay/resource integration works.

The first fresh Core test build encountered unrelated concurrent reputation/archive
API migration errors before running tests. No new Core profile digest or
transcript known-answer pin was captured from that failed build. Full Core,
maximum-shape proofs, whole-prover memory/time, and independent cryptographic
qualification remain outstanding.

A later Core retry compiled and started its 479 selected tests. A separate exact
native profile test computed the manifest with both production and independent
29-field encoders and confirmed their equality before exposing the stale literal
pin. The replacement is
`a9530f225b112f50e1ed113b285eb1ec32b7a4e8ccdea7676b97afa6afff934e`;
the source pin is updated and its fresh assertion remains pending. This binds the
reviewed joined-root/reduced-opening construction and unchanged unavailable
activation policy; it does not qualify release activation. The capture is in
`/tmp/iroha_core_x509_profile_pin_20260928.log`.

Four compact-CA controls pass in that native binary: canonical roundtrip and
binding, credential pre-auxiliary context mutations, public/root/DEEP/FRI/query/
frontier mutations, and fixed-parameter/resource gates. The native DER column
transpose also passes. These are actual proofs and adversarial verification,
not only source-shape checks.

Separate real I/O and projection proof KAT runs complete verification, canonical
re-encoding and query-uniqueness checks before failing their obsolete digest
literals (252.66 seconds combined). Replacement literals are captured below;
fresh pinned assertions remain pending. Log:
`/tmp/iroha_core_x509_protocol_kats_20260928.log`.

| Known-answer proof | Previous digest | Captured replacement |
| --- | --- | --- |
| I/O | `7e283b62798b5a626a71fad660a19a455e6fb389dd268e320a0841e83af5f94d` | `d7d747959c5632f147be02cbaae61e8f1f5dd04691477058ce1b758c13e506aa` |
| Projection | `94f29e8e0b3f9cc444794905125f5b36c03fe4b9e36e095c74aec10a10344261` | `33a900ac3f49acbf3cb92e292525fcd2c7f3d069f6f71cf365a5cf9c0f13f78d` |

The complete broad run finishes with 466 passes, ten failures and three ignored
checks in 999.67 seconds. Its ten failures are the three old-profile-pin cases,
two old protocol KAT literals, one stale 65-block expectation where the exact
framed CRL requires 66, and four positive fixture relabels using a retired
circuit-ID shorthand. Those sources are repaired; a fresh targeted run is
pending. Full-capacity one-input proofs, regenerated key goldens, the three
wallet relations, relation-confusion negatives and the complete 49-family OODS
differential test pass. This is a scoped Core result, not a passing workspace.

The separate native wallet build found one normal-library error: proved-IVM
execution called `TxOverlay::from_queued_execution`, which was incorrectly
restricted to test builds. That restriction is removed; the successful retry is
reported above. A test-only `expect_err` was also replaced with `err().expect`
to avoid requiring Debug on a signed-transaction result, preserving its assertion.
The next retry encountered two concurrent SCCP build errors using `as_slice()`
on `ConstVec`; only those accesses were changed to its existing `AsRef<[T]>`
implementation. The passing finalizer build includes that minimal compile repair.
The historical archive verifier also passes with `--check-current`, including
the 300-line root-document limit.

## Fresh Core and maximum assembly results

The fresh repair/ownership run compiles in 17m41s and finishes with **236 passes,
two failures and three ignored checks** in 302.29 seconds. All ZK repairs above
pass, including the independent engine pin, real I/O and projection KATs,
pre-auxiliary credential binding, confidential proofs, typed relation checks,
and new SHA/P-256 private-buffer controls. The two remaining failures are direct
SCCP component fixtures opening a transaction without the newly mandatory
source owner. Their constructors now use the existing explicit component-test
API; this does not change production authentication. A focused rerun remains
pending. Log: `/tmp/iroha_core_zk_20260928_erasure_retry.log`.
The separate full P-256 trace and cross-table selection passes **19 tests** in
12.55 seconds (`/tmp/iroha_core_p256_full_20260928.log`).

The optimized X509 diagnostic binary is
`target/release/deps/iroha_core-d3a5913883ee08ea`, built against the source capture
`/tmp/iroha_x509_release_retry2_source_capture_20260928.json` before the subsequent
GPU cleanup changes. The structural maximum assembly owns **288,345,698 bytes**
against the existing **600,114,496-byte** source allowance. Construction takes
4.733595 seconds and cleanup 0.007703 seconds; process maximum RSS is 474,808,320
bytes. This admits the assembly only, not the full proof.

Eight native-log-19/common-log-22 columns take 0.247473 seconds to interpolate
and mask and 0.304524 seconds for the forward CPU transform. Scaling the latter
linearly over the required 11,246 transforms estimates **428.085 seconds** before
other proof work, exceeding the 300-second whole-proof target. This is an
extrapolation from measured components, not a completed proof time. A bounded
exact-root GPU adapter and explicit device-scratch admission remain in progress.
The actual current X509 hash is SHA3-384; an earlier tentative Poseidon cost
estimate is inapplicable. A 128-row single-worker SHA3 control measures batch-1
at 0.013590 seconds and batch-8 at 0.010419 seconds with equal roots. No parallel
or whole-proof throughput claim follows. Logs:
`/tmp/iroha_x509_release_assembly_20260928.log`,
`/tmp/iroha_x509_release_fft_20260928.log`, and
`/tmp/iroha_x509_release_hash_20260928.log`.

## CUDA private workspace cleanup

`python3 -m pytest pytests/test_fastpq_cuda_cleanup.py -q` passes its compiled
host harness in 2.90 seconds. The harness compiles actual CUDA host cleanup code
and the pending-wait entrypoint against an instrumented fake CUDA runtime; it
does not replace their implementation with a model. Controls cover copying the
result before erasure, full-capacity device/pinned-host clearing, pool reuse,
invalid-output/drop cleanup, pre-submit failure, workspace growth before free,
and older Poseidon private buffers while preserving public constants. Injected
erase/completion failures quarantine allocations. Unknown in-flight completion
never clears, frees or recycles potentially active device memory. This is host
control-flow and memory-observation evidence; no NVCC or physical CUDA result is
claimed. Log: `/tmp/fastpq_cuda_cleanup_host_20260928.log`.

## Python packaging and mobile boundaries

The first fresh Python native run passes six of seven controls. The remaining
real proof control supplied `Vec<u8>` directions, which PyO3 materialized as
Python bytes despite the public API requiring a concrete integer list. Its
fixture now calls the actual exported membership-path helper; the parser was
not relaxed. The fresh retry compiles in 17m53s and passes **all seven tests** in
30.86 seconds, including actual self-verified full redemption while another
Python thread acquires the GIL. Retry log:
`/tmp/iroha_python_confidential_wallet_paths_retry2_20260928.log`.
Log: `/tmp/iroha_python_confidential_wallet_retry_20260928.log`.

All four Python wheels build and install into the isolated
`/tmp/iroha-zk-python-wallet-20260928` environment. The normal installed-loader
example then fails before execution because macOS rejects the extension's
misaligned Mach-O LINKEDIT string pool. The normal Maturin configuration now
preserves target symbols (`-C strip=none`); the wheel/example retry is pending.
No loader bypass, manual binary patch or successful packaged proof is claimed.

Swift's five injected-driver tests pass in 0.006 seconds after a 25.31-second
build (`/tmp/iroha_swift_confidential_wallet_retry_20260928.log`). They cover
full-u128 amounts, redaction, background work and progress, accepted-job lifetime
after close/cancellation, partial setup cleanup, exact relation/count selection,
and invalid evidence/output rejection. Kotlin's six boundary controls also pass
with its JDK-8 API guard retained. These are wrapper checks. The shared C/JNI
owner's real proof controls, JNI consumers and authenticated XCFramework Swift
consumer remain separate pending checks. New executable examples in both SDKs
use the public interfaces, secure random private inputs and no network writes.

The Swift executable initially exposed duplicate Swift compatibility-library
symbols under the SDK's global `-all_load`. The linker now force-loads only
`NoritoBridge` (`-force-lNoritoBridge`), preserving its dynamic symbol lookups.
The executable links, and all **12** scoped Swift tests pass: five wallet wrapper
controls plus seven real native note derivation/encryption controls, including
the new direct commitment helper. Log:
`/tmp/iroha_swift_confidential_wallet_scoped_current_20260928.log`.
This uses the existing authenticated September-25 artifact for note helpers;
it does not execute the new wallet C API. A broader BridgeAvailability selection
fails in account-ID setup with `nativeBridgeUnavailable` before its assertion;
that independent failure is recorded in
`/tmp/iroha_swift_confidential_wallet_scoped_link_retry_20260928.log`.

### Installed Python retry

The unstripped native wheel builds successfully and the normal macOS loader
accepts it. Importing the SDK then exposes a new export-order defect: the wallet
exports extended `__all__` before the package initialized it. The wallet names
now enter `_BASE_EXPORTS`, and the pure SDK wheel was rebuilt and reinstalled.
The ordinary installed example produces and locally verifies a **13,741-byte**
full-redemption proof with one input note.

Both installed public tests pass in **35.069 seconds**, covering actual proof
generation while an independent Python thread progresses, typed invalid-amount
preflight, closure rejection and all seven public exports. Commands ran with
the isolated installed interpreter's `-I` option from `/tmp`, with no source
path injection or mock. Python 3.12.14, arm64 macOS 27.0. The native wheel SHA-256
is `9e42ffcbc8c408c902c2de87be652ca4f1151e38a82f41857067b3e494af4f2a`;
the loaded extension SHA-256 is
`87eee33d4391653edafaf73daf4a8002c87b4b0126345a12af689a546991fdb0`.
The installed package initializer and wallet module are byte-identical to their
captured source. Logs and receipt:
`/tmp/iroha_python_wallet_packaged_example_export_retry_20260928.log`,
`/tmp/iroha_python_wallet_installed_native_tests_20260928.log`,
`/tmp/iroha_python_wallet_installed_receipt_20260928.json`.
This is local SDK/native proof evidence, not ledger admission or other-host
qualification. Scoped wallet Ruff passes; linting the entire package initializer
also reports an existing duplicate Sumeragi import outside this change.

## JavaScript hard cut and actual addon loader

The caller-key transfer/unshield builders and native exports are removed. The
owned wallet now rejects retired caller-key, circuit and root-hint options;
TypeScript also rejects the old export and caller-key construction. All 21
focused boundary/cardinality/type checks pass. The public asynchronous Merkle
root helper and redemption recipe use bounded inputs. The recipe closes its
wallet after queuing a proof and measures event-loop progress until completion.
These source checks do not substitute for executing its native addon.

An isolated managed checkout built the addon in 34m55s, then its normal publisher
rejected the actual `dlopen` probe with a misaligned Mach-O LINKEDIT string pool.
The normal macOS builder now uses `cargo rustc` with a final-addon-only
`-C strip=none`; dependency profiles, source seals, export probes and loader
verification remain enforced. All 133 build/provenance controls pass, with zero
skips, in 7.59 seconds. Scoped ESLint passes. A source-refreshed addon and recipe
run remain pending. Logs:
`/tmp/iroha_js_isolated_build_retry_20260928.log`,
`/tmp/iroha_js_native_build_strip_regression_20260928.log`.

### Frozen JavaScript candidate: verified loader and real recipe

The normal unstripped retry completes in 29m01s and publishes the verified addon
from the isolated `zk-sdk-qualification` checkout. With Node 24, `npm run
build:dist` passes, and the captured native wallet/root, boundary, cardinality,
and TypeScript selection passes **24 checks, zero failures and zero skips** in
8.00 seconds. Importing `@iroha/iroha-js` resolves its `dist/index.js` entry,
computes an actual native Merkle root, and confirms all three retired caller-key
builders are absent. No native loader override or verification bypass is used.

The captured source recipe then generates a locally verified **13,741-byte**
full-redemption proof with one nullifier, no output commitments and the expected
root. It disposes the wallet immediately after queueing the accepted job. Its
10-ms timer records **2,717 event-loop ticks during proving**; the timer starts
after Merkle-root computation. This establishes local off-thread native proof
execution and accepted-job lifetime for that capture, not ledger authorization.

| Captured artifact | SHA-256 |
| --- | --- |
| Native addon | `605c821effa1293d32290858ff58c6715850468c065fa5208c8f247944d761c6` |
| Native checksum/provenance manifest | `872cd7fb2c551d4d88a4f86fe7099057b3de89304e9b1e0b7ec1388f4d6fca47` |
| Source seal | `f445777640a086733be07d8623ec540458c3280f3185cb0db449c0f49ce18815` |
| `src/confidentialProofBuilders.js` | `9d7736c0720c2581bf7fb4035ea389de900973312f08965649bd2f25cd39e05c` |
| `dist/index.js` | `e9f077438db44b734a51504b5954f16598916563c6d8f794f210304334462245` |
| Local npm tarball | `5b164b353f93ed6552303477e0f123bfab7830f5ac68991679fc98bcaa63dbd6` |

The source revision is `77f8b948e3ca19eaad458deab0b5d092d570ddda`, with a dirty
source seal checked by the ordinary debug-artifact policy. `npm pack
--ignore-scripts` succeeds, but the package deliberately excludes the native
`.node` artifact. Dirty native artifacts also require the matching checkout's
source-state verifier, which is absent from a clean installed package. Therefore
these results cover the authenticated **source/dist loader**, not an installed
native npm consumer. The source-only recipe is also excluded from the package.
Logs and the machine-readable receipt:
`/tmp/iroha_js_isolated_unstripped_build_20260928.log`,
`/tmp/iroha_js_isolated_wallet_tests_20260928.log`,
`/tmp/iroha_js_isolated_recipe_20260928.log`,
`/tmp/iroha_js_isolated_dist_smoke_20260928.log`, and
`/tmp/iroha_js_wallet_frozen_evidence_20260928.json`.

### Change-opening helpers and JNI integration boundary

Later source adds JS `defaultConfidentialDiversifier()` backed by Core and
`confidentialChangeToInput(retainedChange, authenticatedLeafIndex)`, plus Kotlin
`ConfidentialChangeNote.toInput(index)`. They select Core's default change
ownership, preserve exact unsigned-128 amounts, and require bounded indices.
Kotlin conversion owns independent clearing copies and clears its temporary
native diversifier. Its documentation requires secure persistence before proving
consumes the change owner, then reconstruction after the actual authenticated
index is known. Conversion itself does not authenticate membership.

The current JS source/TypeScript selection passes **24 checks**, scoped ESLint
passes, and Kotlin passes **eight** boundary/ownership checks with its JDK-8 API
guard intact. These additions postdate the frozen addon above. New native N-API
Core/default-owner parity and JVM conversion tests are written but **pending**;
no native helper result is inferred from the source passes. Logs:
`/tmp/iroha_js_wallet_change_source_20260928.log`,
`/tmp/iroha_js_wallet_change_lint_20260928.log`, and
`/tmp/iroha_kotlin_confidential_change_20260928.log`.

The actual shared C/Rust owner previously passed **nine** controls in 33.45
seconds, including a self-verified proof, close/in-flight lifetime, no handle
reuse, late second-path cleanup, consumed-job rejection, unsigned amount limbs,
and invalid public redemption. Its log is
`/tmp/iroha_mobile_confidential_native_retry_20260928.log`. The subsequent normal
JNI `cargo rustc --locked --offline -p connect_norito_bridge --lib -- -C strip=none`
build fails before the bridge on six concurrent SCCP BSC integration errors:
missing `Bsc` variants in `SccpLcError` and `SccpLcSetDataV1`, and missing
`SccpVerifierWorkV1.bls_vote_attestations`. No SCCP behavior or loader policy was
changed to bypass this blocker. The real JNI consumer/example and fresh integrated
Core selection remain pending coherent source. Log:
`/tmp/iroha_mobile_confidential_cdylib_20260928.log`.

## Root sampler and change-note follow-up

Entropy sampling now gives raw bytes, partial Fp4 coefficients and the final
replayable mask clearing ownership before any fallible draw. The actual-source
optimized sampler harness passes its injected entropy-error, unwind, success-drop
and overflow controls, observing initialized mask cells before/after erasure.
`/tmp/iroha_zk_mask_sampler_actual_source_20260928.log` records the isolated pass;
integrated Core execution remains pending.

Swift's retained-change helper uses the native default diversifier and accepts
only a valid tree index. All eight actual native note/helper tests pass in
1.679 seconds (`/tmp/iroha_swift_change_note_helper_20260928.log`). The added
nondefault-input → private change → full redemption integration test compiles;
its actual proving run awaits the new authenticated XCFramework. A subsequent
compile/helper run passes eight controls in 1.729 seconds. These runs use the
older authenticated note-helper artifact and do not validate the new C proof API.
Python's eight mocked boundary controls pass with the new default-diversifier
and change-to-input helper. Its new installed two-proof change-cycle control
requires the refreshed native wheel and is not included in the earlier two
installed tests.


## Installed Python change-cycle qualification

A new explicitly scoped candidate updates only eight Python-owned files from the
previous frozen SDK checkout. The native default-diversifier getter, pure change
conversion, export-order fix, tests and README are captured before building in
`/tmp/iroha-python-change-candidate-20260928/snapshot.json`, with per-file hashes.
The 9,828-byte Python-only patch has SHA-256
`74290afdd059c8dfdf156b93dff805078d19ab63a962d06d65a13afbf04447e5`.
Core, SCCP, the prior JavaScript source and its original addon are unchanged in
this candidate. The earlier JS receipt remains tied to its original source seal;
it does not qualify this later dirty-source state or new JS helper additions.

Normal Maturin builds the native wheel in **17m40s**, using Rust 1.93.1,
`--locked --offline`, one build job, no incremental compilation, the existing
shared Cargo target, and the captured target-only `-C strip=none` configuration.
The pure wheel uses the standard isolated Python build backend. Both wheels are
installed normally with `pip --force-reinstall --no-index --no-deps` into the
existing disposable validation environment. Tests run with Python 3.12.14 `-I`
from `/tmp`; loaded SDK and native paths are all under that environment's
`site-packages`, with no source-path injection or loader override.

All **three installed native checks pass in 79.919 seconds**. They cover a real
redemption with private change followed by conversion and a real full redemption
of that change, the default-owner binding, GIL progress during native proving,
close behavior, bounded preflight and public exports. The separately executed
installed example passes in **33.528 seconds**, producing a locally verified
13,741-byte full-redemption proof with one input. These are local proof and host
package results; they do not authorize a ledger transition, qualify a mobile
device, or cover the newer MAIN producer modules in the live checkout.

| Installed artifact | SHA-256 |
| --- | --- |
| Native wheel | `d990cd65066700b4389e56520bbc552a850470a2690ffbfb8d6cf9180bbcbe32` |
| Pure SDK wheel | `500197a3aec15e9e59c29f4d23c6cb53a5fc19c6fd1517d5b6e3f33e4161b7f9` |
| Loaded `_crypto.abi3.so` | `42c5053d0a0ec4f28c356965f31353c75e327d93d05c99611b79ed6436ad9f1f` |

The complete commands, module paths, source/artifact hashes and timings are in
`/tmp/iroha-python-change-candidate-20260928/installed-receipt.json`; build output
is `/tmp/iroha_python_change_native_wheel_20260928.log`. The candidate's updated
mocked source wallet suite also passes all eight controls.

## JavaScript snapshot lifetime

The live loader now registers one synchronous normal-exit cleanup callback for
its owned authenticated snapshots. Successful explicit cleanup releases that
ownership; failed removals remain eligible for an exit retry. Windows may still
lock a mapped addon at exit, so removal is best effort there. Verification,
source provenance, the original addon and loaded-byte selection are unchanged.

The complete source/dist verification selection passes **38 checks, zero skips**
in 7.96 seconds, and scoped ESLint passes. New child-process controls cover
natural exit, explicit nonzero exit, twelve snapshots sharing one listener,
immediate failed-load cleanup, preserved originals and no remaining snapshot
folders. They use checksum-verified fixture bytes to exercise the real snapshot
owner, without loading a synthetic native module. This change is not copied into
the frozen SDK candidate. Log:
`/tmp/iroha_js_snapshot_exit_tests_20260928.log`.

Exactly three copies from the completed frozen JS qualification were removed
only after matching the recorded 174,844,480-byte addon hash, owner, regular-file
shape, inode and empty `lsof` mapping/open-file checks. Removal reclaimed
524,533,440 bytes. The original verified addon and every earlier receipt remain;
eight unrelated older snapshot directories were untouched. The cleanup receipt
is `/tmp/iroha_js_snapshot_cleanup_20260928.json`.


## DER precursor cleanup and borrowed headers

The final actual-source optimized harness passes **35 controls in 1.50 seconds**:
seven new cleanup/borrow checks, thirteen existing native DER parser tests,
eleven existing pure AIR/RFC adversarial controls, and four private-table owner
checks. It compiles the real DER parser and validator, existing fixtures,
Goldilocks arithmetic and clearing owners with the existing `time`, `thiserror`,
`x509_parser` and `zeroize` dependencies. No parser or arithmetic substitute is
used. I/O adapter functions and the single older I/O-plan-dependent test are
excluded; integrated Core and complete proof execution remain separate.

The controls observe initialized cells on partial parsing, signature rejection,
precursor drop/unwind, byte reconstruction failure, displaced allocation growth,
and late provenance/path failure. Borrow tests confirm exact identifier, long
length and extension slices retain their original pointers and values. Existing
canonicality, topology, grammar, field-residue and public-output mutations remain
in the executed selection. The include-file documentation header was corrected
after the harness found its compiler error.

| Actual source | SHA-256 |
| --- | --- |
| `der_air.rs` | `fda1e7d7820b059bcc82def0e0ed0b55bf26a82db63e782caa29fa2353463ade` |
| `der_air_cleanup_tests.rs` | `84cb33c7666d67f7942b0b00addb4f3f17b40f84e390bad3f6c352ad0c1947a2` |
| `private_table.rs` | `ab7ec1ab00322517cefa694b768dfe84e885c8a78e27dbd157c28058ca3efb54` |

The reusable generator, exact compiler command, complete source hashes and result
are under `/tmp/iroha-der-cleanup-kernel-20260928/`; the machine-readable receipt
is `receipt.json`. The combined run log is
`/tmp/iroha_der_cleanup_actual_source_20260928.log`.


## Retained local evidence bundle

The completed DER and quotient-cache harnesses, generated test sources, compiler
commands, binaries, logs and receipts are also retained under the ignored
`dist/zk-remediation/2026-09-28/` directory. The same bundle retains the refreshed
Python wheels, Python-only source patch, installed-native receipts, completed
JavaScript receipt and entropy-sampler test source/log. Its `evidence-index.json`
records 34 initial files (43,669,278 bytes) with SHA-256 identities and original
paths. These copies preserve the exact historical scope in each receipt; they do
not convert isolated tests or a frozen SDK candidate into current Core or release
qualification. The original `/tmp/` paths remain in the historical commands.


## Public change-note workflow

The sibling `iroha-docs` canonical `src/blockchain/anonymous-transactions.md`
now requires saving the private change opening before proving, reconstructing it
only after authenticating the new leaf index, and using the language-specific
change helper. It explicitly explains that the change owner's default
diversifier can differ from the consumed input's diversifier. Exact current
Python, Swift, Kotlin and JavaScript helper names are included; local proof
construction remains separate from ledger authorization.

The documentation repository requires matching English and all twenty maintained
translations. The added paragraph was translated locally, preserving the helper
identifiers and metadata. Its existing scoped `validateI18n` passes all twenty-one
pages, including source hashes, routes, headings, technical identifiers and prose
structure. Scoped `git diff --check` also passes. No remote translation service,
mutable source refresh or full-site build was run. English SHA-256:
`9046dc42d9c0441533017fd1602dc94682c191837af52750c7e2598f4930c84b`.
The validation log is `/tmp/iroha_wallet_change_docs_validate_20260928.log`.


## Complete private-witness codec cleanup controls

The complete actual `codec.rs` source passes **fourteen optimized tests**: all
seven existing codec controls, three new ownership/size controls, and four
private-table controls. The new selection checks initialized private cells at
every truncated offset, late-invalid fields and suffix rejection, successful
ownership transfer and unwind, and exact 480-byte/16,994-byte encoding shapes.
Canonical round trips and the existing byte-order/index checks remain intact.

The harness includes the complete codec and clearing-owner modules unchanged,
plus source-extracted Goldilocks implementation, Merkle path type, and exact
profile/data-model constants. It uses Rust 1.93.1 and existing dependencies;
there are no parser or arithmetic stubs, new Cargo builds or full-proof claims.
Captured codec SHA-256:
`9b7b894c393153bd7a3c9c7bf576d3aa90596ae85095ad1383078fddaa93c220`.
The reusable generator/runner, logs, complete source hashes and receipt are
retained in ignored `dist/zk-remediation/codec-kernel-20260928/`.

A source review of the live I/O materializer and provider also confirms clearing
owners survive topology and constraint failures before their final transfers.
Their private matrices reserve the complete row capacity before writing, and
the provider preserves its borrowed assembly source. Their new native controls
still require the integrated Core run.


## Private-witness codec precursor lifetime

The canonical witness decoder now creates its recursively clearing owner before
copying certificate or CRL bytes, fills the fixed sibling array directly and
reserves bounded vectors before writing private values. Encoding computes its
complete checked size and reserves one guarded allocation before the first
private byte. The grammar and valid encoded bytes are unchanged; allocation
failure has a distinct error. Existing full-capacity vector erasure remains,
with count-only observations around initialized cells.

The actual-source optimized codec harness passes **14 tests**: all ten codec
controls, including three new all-truncation, late-invalid/drop/unwind and
empty/maximum-shape controls, plus four private-table controls. It uses the
complete codec and source-derived supporting declarations without substituting
codec or arithmetic logic. The codec source SHA-256 is
`9b7b894c393153bd7a3c9c7bf576d3aa90596ae85095ad1383078fddaa93c220`.
The complete harness and receipt are retained under ignored
`dist/zk-remediation/codec-kernel-20260928/`. The retired-codec guard passes.
Integrated Core and the complete credential proof remain pending.

## Apple build cache provenance retry

The first isolated Apple target compiled in 77m18s, then the normal PQClean archive
validator rejected copied Cargo build records referring to the original
checkout. No artifact was accepted or published. The five original package
records are retained under `dist/zk-remediation/2026-09-28/apple-cache-repair/`.
An ordinary target-scoped Cargo clean removed only the isolated candidate
`pqcrypto-internals` release outputs (115 files, 5.7 MiB); the normal five-slice
builder was restarted with unchanged source and provenance checks. That retry
terminated naturally with `ENOSPC` while writing Core metadata for
`aarch64-apple-darwin`; no artifact was accepted. After filesystem headroom
recovered to approximately 604 GiB, a second retry started against the verified
unchanged patch and untracked-file hashes. Its command, environment and result
are recorded in `apple-cache-repair/retry2-command.json`; the result is pending.
This is a local copied-cache repair, not a normalization bypass.


## Integrated cleanup candidate stopped by storage exhaustion

The first integrated cleanup compilation found three test-only comparisons of
new private-table owners against vectors or one another. Comparing their exact
slices preserves those assertions. Subsequent attempts stopped in concurrent
SCCP changes: a TON validator-set vector needed the explicit existing
`SccpLcConsensusSetV1` type, and the deterministic `SyntheticTonChainV1` fixture
needed `Clone, Copy` under the workspace lint. Both incidental repairs are
minimal; no SCCP protocol behavior was changed by this ZK task.

The next Core attempt compiled under Rust 1.93.1 with the normal node
features plus `iroha-core-tests,fastpq-gpu`. Rust compilation completed in
12m23s, but `rust-objcopy` exhausted the filesystem during stripping. Its output
was a 4,000-byte non-executable file and **zero tests ran**. The 399-file relevant capture has
SHA-256 `cb9bc69c7e53cc20a887f063c24f6e168b4f4e7aad515bd0e95de192b8af9f7b`;
its source list is `/tmp/iroha_x509_core_cleanup_retry3_scope_20260928.sha256`
and log is `/tmp/iroha_core_x509_cleanup_retry3_20260928.log`. The ZK source
subset remained unchanged during compilation; concurrent SCCP test-fixture work
changed an incidental captured file. Logs, source hashes and the failure receipt
are preserved under `dist/zk-remediation/2026-09-28/core-cleanup-attempt/`.
No immutable whole-workspace or passing integrated-suite claim is made.

Afterward, the DER provenance destructor was routed through its documented
clearing method, removing the unused-method warning without changing the
underlying erasure. The actual-source DER selection again passes all **35 tests
in 1.48 seconds**. Current DER source SHA-256 is
`7b0529141c9b792fda264fa7aaed219cd1c992e6900b9d276a540f7ca73eb4bf`;
the complete refreshed harness and receipt are retained under
`dist/zk-remediation/2026-09-28/iroha-der-cleanup-final-20260928/`.

The completed Python test environment was removed only after its installed
native library was matched to the retained wheel and no open handles remained.
The wheel artifacts and completed installed-test evidence remain available;
`python-venv-cleanup.json` records the exact removed directory and native hash.
This cleanup does not imply that the installed environment still exists.


## Norito private-witness replacement

The local zk-X509 witness now uses the explicit nominal schema
`iroha_core::privacy_engines::zk_x509::ZkX509WitnessV1` with uncompressed Norito
framing and flags `0`. The retired `IRX509W1` format has no decoder or fallback;
its magic remains only in a rejection test. The field serializer is derived,
while the fixed, nonrecursive decoder borrows field spans and populates one
recursive clearing owner before copying any private bytes. Sequence, field,
allocation, certificate/CRL and attribute bounds remain enforced. This decoder
does not claim dynamic nesting-budget accounting for its fixed borrowed graph.

The writer counts the real derived frame, reserves one clearing allocation, and
streams Norito's checksum/header/payload passes into a destination that rejects
any capacity growth. The maximum frame is **20,398 bytes**; the small fixture
with no attribute openings is **3,776 bytes**. Both were observed from the real
Norito serializer and asserted against the dimensional maximum. This changes a
local witness DTO, not DER preimages or the specified credential-proof codec.

`dist/zk-remediation/codec-norito-kernel-20260928/` retains the reproducible
extraction/compiler commands, source and linked-library hashes, logs and
`receipt.json`. Rust 1.93.1 reused existing real Norito/derive dependencies and
compiled the complete actual codec and tests, actual clearing table, and
source-extracted field/path declarations without parser or arithmetic stubs.
All **20 tests passed** (16 codec and 4 private-owner controls; test harness
reported 0.14 seconds). The relevant Norito core, encoder, frame, nominal-schema
and derive sources matched the existing SDK candidate byte for byte. A separate
normal-feature metadata check with `privacy-release-evidence` and denied unused
imports, missing Copy implementations and variant-size differences passed.

The controls retain every prior shape and cleanup assertion, updated for the
Norito layout, and cover all frame truncations, every checksum-valid payload
truncation, nested partial digest/opening cells, wrong schema/flags/compression,
CRC rejection, retired magic, late duplicate/suffix errors, exact lengths,
unaligned input, stricter outer element/field/allocation limits, writer
error/unwind and normal ownership transfer. `scripts/check_no_legacy_codec.sh`,
scoped rustfmt and diff checks passed. The fixture-only unchecked serializer
uses the same derived codec for engine preflight negatives.

Final codec source SHA-256:
`f783b0f4adbabaddc3260299660e96dbaca420db04b3df7520156d32715cb25e`.
Test source SHA-256:
`99ef61fe2692a63308277978f789d8e780acda77af1ab92142900a9fee569835`.
The only Merkle source change adds payload serialization to the existing path.
This receipt does **not** qualify a fresh integrated Core test binary, the
rotated engine preparation-profile pin, or complete credential proof production;
those remain separate evidence owned by the integrated validation run.

## Actual Kotlin JNI wallet and retained-change consumer

The normal `cargo +1.93.1 rustc --locked --offline -p connect_norito_bridge
--lib -- -C strip=none` build passes in 16m18s on the retained SDK candidate.
All nine relevant native bridge/wallet source files match the current checkout
before and after compilation. Other Core dependencies remain scoped to that
candidate; this is not a current whole-Core result. The 146,170,864-byte arm64
Mach-O library is retained under
`dist/zk-remediation/2026-09-28/jni-wallet/native/`, with SHA-256
`25edc97661608af7b4bb863ea61da3bb4d5dadcdff6d2e0bfb68362de0f6275e`.

Using JDK 21 and the normal `IROHA_NATIVE_LIBRARY_PATH` setting,
`:core-jvm:test --tests '*ConfidentialProverNativeTests'` passes **4 tests with
zero failures or skips in 71.751 seconds**. It executes three complete local
proofs: the existing full redemption, followed by a separate two-proof control
that consumes a note with a nondefault diversifier, creates private change,
restores the securely retained opening through `toInput`, and fully redeems it.
The latter checks the default owner commitment, consumption of the original
change owner, fresh nullifier and empty final output set. Native contract/error
codes and direct default-owner conversion also pass.

The executable `:core-jvm:confidentialRedemptionExample` separately succeeds in
a 32-second Gradle run and locally verifies a **13,741-byte proof**. Its cleanup
now surrounds both random fills, so an RNG failure after a partial write still
clears its key and note-secret arrays. Both modified consumers compile with
the existing JDK-8 API guard. Neither run submits a transaction or claims Android
device or signed release-artifact qualification.

Exact commands, source hashes, the test XML, binary, logs and per-file hashes
are retained in `dist/zk-remediation/2026-09-28/jni-wallet/`. The initial compile
attempt without `JAVA_HOME` only reported a missing macOS Java runtime;
the subsequent invocations explicitly select the existing JDK 21 installation.


## Rust public wallet API hard cut

The remaining six `confidential_v2::build_confidential_*_proof_*` functions
accepted caller-supplied circuit identifiers and verifier keys. All workspace
callers are the canonical wallet owner and ZK module tests. They and the three
internal proof-result structs now have `pub(super)` visibility. The public
`ConfidentialProver` owns canonical selection; note/tree primitives and key
registry management retain their separate public roles. No circuit constraint,
proof encoding, key, or arithmetic changed, and every internal circuit assertion
is retained. Six `compile_fail,E0603` rustdoc examples assert that external
callers cannot import the low-level builders.

`dist/zk-remediation/rust-wallet-api-20260928/source.json` captures the seven
relevant source/spec hashes. Scoped rustfmt and `git diff --check` pass.
Compiler/rustdoc execution is pending. This edit followed parsing by Core native
session 23315 and is also newer than the immutable SDK/Apple candidates; those
runs cannot establish the visibility cut's compilation. The final Core rebuild
will include it explicitly.

The existing Halo2/shared-compaction/note-STARK source-contract selection passes
all 10 checks in 1.86 seconds after this cut. Its command and test-source hashes
are retained in `source-contracts.json` beside the visibility source capture.


The Rust wallet now also supplies consuming
`ConfidentialUnshieldOutputV3::into_input(index)`. It validates positive amount
and fixed tree capacity, moves fields out of the clearing change owner, and
selects Core's default change diversifier. This prevents incorrectly inheriting
a prior input's nondefault diversifier when restoring change. One shape test
covers zero value and both out-of-range indices; a real two-proof regression
independently reconstructs the new commitment before fully redeeming the change.
These tests are added to the final Core selection and have not executed yet.
No circuit or encoding changed. `source-with-change-helper.json` captures this
later scope; the initial visibility-only capture is retained separately.
