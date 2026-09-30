# ZK completion continuation — September 30

The completion goal is active. ZK01 and ZK02 retain their implementation status;
ZK03 through ZK08 remain unfinished. This is a running validation record, not a
release certificate. The owner is the [ZK goal tracker](../../../specs/zk_first_release_goals.md).

## Candidate custody

The initial checkout was clean at `89efeb5f734e6782f818e1bf98c21870c7888e9b`.
It already contained the five Core fixture API corrections and the prepared SHA
cyclic-padding repair. Concurrent merges subsequently moved privacy code to
`iroha_core_privacy` and generic ZK code to `iroha_core_zk`; the shared checkout
reached `f7444cd4e4afbc9e41772264dbbde3173e5e2c48` during inspection.
The user subsequently required all work to use only the primary checkout on
`optimizations`. Follow-on scheduling from the earlier copies was disabled;
already-running Cargo processes were allowed to finish without signaling them.
Their results are historical only. New builds run directly in the requested
checkout, with input guards that withhold qualification if relevant source changes.

The primary build lanes are `primary-core-kagami/`,
`x509-current-privacy-opt3/`, and `sdk-qualification/optimizations-apple-run3/`.
They build the current extracted crates and Apple bridge with normal Cargo
profiles in separate ignored target directories inside this checkout.

Local ignored evidence lives under `dist/zk-remediation/2026-09-30/`.
The following earlier captures are retained for diagnosis, not current-branch
qualification:

- `current-core-kagami/`: a cold, normal-profile build from an immutable clone
  of the initial revision, with two build jobs, retained compiler messages,
  dependency hashes and source/binary guards. The old 247-name census included
  a deleted Kagami `scaling_evidence` producer. The revised plan retains the 246
  Core controls and separately labels three current finality controls; they are
  not equivalent replacements for genuine fixture generation. Actual native
  execution evidence, signed one/four-lane Kagami controls, `kotlin-fixture-gen`
  and ABI-25 JNI consumers are separate follow-up selections.
- `x509-sha-cyclic-native/`: normal optimized native validation of the retained
  X509 candidate and its exact amendments. Source manifest
  `160eada1c83279d532493ce9e54a4f9923a400bcfe854dab3b9d281e79148380`
  covers 20,882 files. The four-file padding repair has a source-only review;
  it does not assert native or complete-proof success. The review checks the
  verifier-owned disjoint terminal/padding selectors, retained first/terminal
  product equations, zero padding, unchanged degree, and changed profile identity.
- `sdk-qualification/`: continuation of the earlier five-slice Apple build and
  current consumer work. Historical Apple artifacts are not current integrated
  source qualification. No physical-device execution has been recorded.

## Completed local checks

The initial revision passes 22 focused Python source/geometry checks. After the
crate extraction, the same file selection passes 25 checks:

```sh
python3 -m pytest scripts/tests/zk_source_tokens_test.py \
  scripts/tests/halo2_backend_02_compaction_source_test.py \
  scripts/tests/halo2_backend_shared_circuit_source_test.py \
  scripts/tests/check_note_stark_profile_constraint_dedup_test.py \
  scripts/tests/check_zk_x509_proof_geometry_test.py -q
```

FASTPQ's selected construction, geometry, resource and evidence tooling passes
194 checks:

```sh
python3 -m pytest scripts/fastpq/tests/test_deep_hiding_candidate.py \
  scripts/fastpq/tests/test_compact_source_budget.py \
  scripts/fastpq/tests/test_compact_geometry_screen.py \
  scripts/fastpq/tests/test_review_boundary.py \
  scripts/fastpq/tests/test_digest384_evidence.py -q
```

These runs check source contracts and tooling, not complete native proofs or
finalized network behavior. Their counts overlap neither with a claimed complete
workspace pass nor with physical-device qualification.

`bash scripts/check_no_legacy_codec.sh` passes on the primary checkout. The
prescribed history verification could not run: the current checkout has no
`scripts/archive_project_history.py`. No historical archive verification is
claimed from that failed invocation.

The first direct-checkout Core build exposed an overflowing unsuffixed fixture
literal in the extracted privacy crate. Giving that fixture its intended `u32`
type fixed the compilation error. Follow-up builds exposed production recovery
helpers hidden behind test-only configuration and test consumers of formerly
private Core helpers. Those extraction fixes are being validated in normal
builds; failed build attempts remain retained.

The fourth primary Core/Kagami build passes its source and artifact guards.
The source-state and source-map groups pass 15 and 12 controls. The inventory
group passes 98 and fails one fee assertion: its fixture selected an arbitrary
asset while current fee charging pins the canonical network XOR asset. The
fixture correction preserves the balance assertions. The fresh retry6 inventory
passes all 99 tests; source-state 15, source-map 12 and native-routing three also
pass under the unchanged source guard. The output-producer group passes 81
individual tests before a fixture self-deadlock: its periodic test reacquires
the nonreentrant recorder guard already returned by `recorded_component_block`.
A retained read-only thread sample locates that wait. Removing the redundant
acquisition and drop preserves every assertion. The old process is untouched
and its group remains incomplete; retry7 rebuilds and runs that exact regression
before the complete census.
The current census is 251 controls: 246 retained Core controls, two added
source-state controls and three current Kagami finality controls. The retired
complete exporter is explicitly excluded, not counted as replaced.

The authentic current `kotlin-fixture-gen` passes all ten supported modes under
an unchanged source manifest. This does not supply a complete finalized network
corpus. Its executable SHA-256 is
`58bc6337cc288aa921f62febe0afebebc622fd0a77fd05066cd28fc23c6eb2aa`.

Native X509 validation found a second cyclic-padding defect in four carried
word-memory products. Logical call rows now use the existing public row-kind
selectors minus the call-final selector; physical padding contributes zero.
This preserves the degree bound and live recurrence checks. The new compiled
profile is `19aa35927ebc6e0ff800a0c80b6b315c2615350f35c82afdb9eae609d4c9ca9c`.
The retained optimized binary
`3ecdf115e8af4bfebf2e29008a67c2d6036bc51abaca10e38573c62731153b44`
passes 44 focused native controls, including the previously failing actual
maximum SHA boundary and three explicitly run expensive parity tests. Native
component proofs supplied the new fixture digests. A subsequent normal optimized
binary, `b71ba937e207347697f80bee14851094ba35f2887735592a5b14dd1030008385`,
passes both updated fixtures, both profile controls and all 49 actual maximum-source
native registration boundaries. The latter checks 2,831 edges, per-registration
mutation sensitivity and exact visitor coverage in 85.915 seconds, with peak RSS
5,896,126,464 bytes. The final test-only cleanup also owns an unexpected extra
stream row in a clearing wrapper before rejecting it. Its final helper hash is
`24e5156bc99e1692de167c60557e0b70db4b1da250652006e8b68f7b283fbbce`;
the guarded final rebuild passes. Its retained binary SHA-256 is
`a66a519824602248428e6e972393314e41734ca45ee54ab76624c7c301991da1`.
Both fixture controls, both profile controls and the 49-registration regression
pass again; the latter takes 87.722 seconds and 5,895,077,888 bytes peak RSS.
The complete maximum proof is running with its original limits. It has already
exceeded 300 seconds, so a later valid artifact alone cannot qualify performance.

The first primary Apple build fails with two production calls to the test-only
`FreshIssuerAdmissionV1::deadline` accessor. Both calls now use the existing
production `require_live` check before and after qualification construction;
the handoff regression additionally checks fresh acceptance and expired
rejection. These changes await the rebuilt bridge tests. The failed build also
observed source changes and published no package. Its 58 compiler outputs have
authenticated provenance for normal Cargo revalidation, not package qualification.
The five-slice primary Apple retry4 is running against the settled inputs, with
selected-source manifest
`1691761ec056e2adb23b5696af1fafb8c5591c8f600f37999c708c546f47c1d4`.
Its normal builder stages a prospective Swift pin projection separately; it does
not rewrite the tracked loader during the concurrent Core validation epoch.

Authentic Rust status and lane generator outputs match their current committed
fixture bytes exactly. Multisig-account and FASTPQ balance-key JSON match every
decoded value, with formatting differences only. The retained comparison receipt
is `sdk-qualification/current-canonical-fixture-comparison.json`. Kotlin's current
host runner now passes against the authenticated primary ABI-25 JNI capture:
all 1,522 JVM tests, including four mandatory native wallet controls and direct
generator parity, plus all 25 attestation-tool tests. The executable example
locally verifies a 13,741-byte one-input full redemption. No test skips or source
drift are recorded. After a test-only nonce-substitution expectation correction,
the Android rerun passes all 210 managed and 71 host-JNI tests without skips.
The same native capture also supports a warning-free C# Release build, all
5,865 unit tests and the public two-proof redemption example. The SDK record
retains exact artifact bindings, commands and the earlier failed Android run.

Core retry7 passes the previously deadlocked producer regression and 250 of the
251 selected controls. The remaining fixture uses `physical-elastic` where the
current resolver requires `elastic-lane-1`. Supplemental genuine signed Kagami
one/four-lane controls pass. Native evidence passes six of seven controls; the
last expects retired error wording instead of the original archive's propagated
`Io(NotFound)`. A bridge unit-test build also finds a missing fourth qualification
argument in an existing test. All three test-only corrections are prepared and
held for the next source-seal window; these partial results do not close ZK08.

## Pending work

TODO: retain terminal results of the native builds, exact control censuses and
authentic fixture producers; repair real failures without weakening assertions.
Finish X509 dependent native profile fixtures and the full maximum proof under
the existing byte and memory limits, then independently verify the artifact.
Complete same-candidate SDK packages, host and connected-device tests, workspace
checks and four-validator qualification.

The FASTPQ implementation audit is running as Codex Security scan
`3cf7dd26-5de8-448a-afdd-3f8451b33dfe`, scoped to `crates/fastpq_prover`.
Its saved draft is partial; no completed audit or cryptographic qualification is
claimed. Separate fresh-context verifier and transfer/hash/SMT relation reviews
have completed their bounded source inspections without establishing an
acceptance bypass. Parent review covers DEEP geometry, transcript ordering,
masking ownership, Merkle openings and accelerator cleanup. A further independent
cryptographic review has completed independent arithmetic checks and a
conditional interactive soundness outline. Its 31 digest vectors, matrix
minors, extension/domain checks, masking ranks and exact query-sampler
calculations also pass the parent rerun. No false proof or witness recovery was
established. The inherited 54-target, 2^32-query quadratic compiler budget fails
the requested aggregate 128-bit target even using the optimistic query-only
Johnson limit: the resulting screen is approximately 117.663 bits. This is a
shortfall in that reduction, not an attack-success lower bound. A naive increase
to 68 queries exceeds the current child cap. Full transcript hiding, concrete
hash/qROM compilation, remaining source, adversarial execution and hardware
timing still require evidence. The retained mathematical report is
`dist/zk-remediation/2026-09-30/fastpq-crypto-review/review.md`.

The Swift runner now requires authenticated current native artifacts before its
68 policy tests, two real wallet proof tests and executable example. A separate
generic iOS build check must pass before connected-device execution. One Apple
development signing identity is available, but no physical device was connected
at the read-only preflight. No device run or release signing is claimed.

RAM-LFE still needs a selected secure encryption construction and its complete
program relation. Native IVM execution still needs the complete transition
relation and authenticated finalized State binding. Both remain unavailable.
No source hash, unavailable marker, leaf proof or diagnostic trace completes
either algorithm.


## Further same-checkout continuation

The unchanged-source maximum X509 run completes unsuccessfully at the producer
self-check. The retained binary is SHA-256
`a66a519824602248428e6e972393314e41734ca45ee54ab76624c7c301991da1`;
all 7,003 selected input identities remain unchanged. It spends 6,241.900 seconds
proving (6,242.011 seconds process wall time), peaks at 10,295,918,592 bytes RSS,
and returns `ProverSelfCheckFailed`. Memory remains below 12 GiB; the unchanged
300-second target fails and no verified proof establishes the byte limit.
The next test-only diagnostic retains the bounded encoded public fixture before
self-check and records its exact verifier failure, permitting verifier replay.
It records no private witness and grants no proof capability.

The reviewed performance proposals preserve the relation and existing limits:
periodic quotient denominators, batched value auxiliary generation, authenticated
terminal reuse, batched public fixed-polynomial replay, skipping writer work
only for completely inactive public events, retaining the clearing FRI input, and
batching native arithmetic/value base columns. The seven-change candidate has
52 required native controls, including maximum fixed/OODS parity and every
1,395 arithmetic/value base column before and after binding. Native controls
and a complete proof remain pending. The baseline base-mask and commitment
phases alone took 375.937 seconds under the observed contention; source-level
replay reductions do not establish a measured speedup.

Normal current host JNI ABI-25 consumers pass 1,522 Kotlin/JVM tests and 25
attestation-tool tests, plus the real confidential wallet example. Android managed
and host-JNI consumers pass 281 tests (178 client, 32 wallet, 71 JNI), with zero
skips. These are macOS host JNI tests, not Android physical-device execution.
The C# host suite passes all 5,865 tests, including four actual native wallet
controls, and its public example. Fresh installed Python wheels pass three real
native wallet controls and the public example. Each lane retains its build,
source and artifact identities; none establishes a signed multiplatform release.

The first full JavaScript run executes 3,633 tests: 3,608 pass and 25 fail, with
zero skips. Authentic fixture/layout corrections and the SoraFS asynchronous
input-ownership repair are integrated. The latter copies bounded caller bytes
and options before lazy module loading. The next two full runs each execute
3,637 tests with 3,636 passing, one failing and zero skips. The first failure is
the exact browser-size baseline; updating its measured eager/combined bytes
exposes a second, derived headroom literal in the next run. Both are explained
by the 29-byte required `appPolicyBindingDigest` field; all ceilings, lazy chunks
and the 103-input closure are unchanged. The final headroom correction requires
a focused rerun. Normal native rebuilds pass in 696.463 and 678.091 seconds;
full-run failures remain preserved and are not reported as complete SDK passes.

The normal Apple ABI-25 builder completes all five slices and artifact checks
in 8,522.237 seconds with no source drift or dependency errors. Ordinary Swift
host tests then fail to link because SwiftPM omits the native Metal framework.
A diagnostic with an explicit linker flag runs 1,901 core SDK tests and 68 mobile
transport tests. The latter all pass; the former has 23 assertion failures in
12 cases, with zero skips. Both real native wallet proof tests pass in 10.214
and 13.127 seconds. Packaging and fixture corrections are prepared for ordinary
validation; the diagnostic is not a package qualification. One signature test
also assumes repeated Apple Ed25519 signatures have identical bytes: on this
host they differ while both verify. Its repair verifies the actual signature
over the exact body/network/freshness and rejects a changed body. Generic iOS
preparation and a physical-phone run remain outstanding. The current universal
macOS archive also exceeds the unchanged CocoaPods entry ceiling; no cap is
raised and no signed release package is claimed.

All four actual Metal parity controls pass with zero skips on the physical
M1 Ultra. Their receipt retains the exact executable and proves the intervening
JavaScript/CI edits changed no Rust/Cargo input. Five borrowed-signature controls
also pass, including 44 fresh-process cases across all 11 shipping algorithms.
These runs do not establish CUDA, another processor family, side-channel freedom,
or the proposed replacement FASTPQ transcript.

The q77 compact FASTPQ investigation has a conditional finite soundness ledger
within the existing child/envelope limits. A held SHA3-256/SHAKE256 primitive,
whole-tape sampler and opaque 32-byte commitment owner preserve the existing
shared Digest384 users. Full transcript/codec/FRI integration, native known-answer
checks, resource measurements and replacement hardware kernels remain unfinished;
no proposed construction is activated by an arithmetic screen or source review.

RAM-LFE public BFV experiments also expose correctness failures: the valid packed
input fails on the seventh square under the coprime-modulus candidate, and on the
eighth square under the candidate with a factor of 257. Corrected lattice attack
estimates do not repair those decryption failures or establish circuit privacy.
The complete program still needs a secure refresh/circuit-privacy construction
and authenticated malicious-key policy before activation.

Six native fixture corrections, combined scalar ABS/MEAN constraints, checked RAM planning,
a finalized predecessor World prerequisite, Musubi signature custody and governed
runtime lifecycle reconciliation are prepared for one normal integrated build.
The scalar candidate constrains all inactive workspace cells and preserves
gas-first traps plus pre-fetch cycle admission. Its derived 4,141,952-byte bound
remains below 4 MiB at 136 queries; native degree and proof admission are pending.
The predecessor receipt is explicitly World-only; it cannot promote complete
State authority. Runtime lifecycle tests cannot substitute placeholder artifacts
for an authentically qualified complete release. Full IVM and RAM-LFE execution
capabilities remain unavailable, and all six completion goals remain active.
