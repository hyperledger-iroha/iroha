# Kagemusha ABI-21 Android candidate staging

The physical-device lab accepts only a candidate generated from the exact
current Iroha checkout. It does not synthesize proofs, consensus data,
openings, Merkle paths, recipient requests, or lifecycle request/result
archives.

## Source identity

The candidate `source_tree_sha256` is the output of the full source-tree seal,
not the Apple bridge dependency-closure fingerprint. The following is the
internal worker command, not an operator command: the root supervisor may run
it only under the exclusive no-login `boi-build` UID and its inaccessible
private target.

```sh
BUILD_CLOSURE=<root-published-content-addressed-closure>
/usr/bin/env -i HOME=/var/empty LANG=C LC_ALL=C \
  PATH=/usr/bin:/bin TMPDIR=/private/tmp TZ=UTC \
  KAGEMUSHA_BUILD_USER_NAME=boi-build KAGEMUSHA_BUILD_UID=<numeric-uid> \
  "$BUILD_CLOSURE/toolchain/python/bin/python3" -I \
  "$BUILD_CLOSURE/source/scripts/build_kagemusha_v4_candidate_bundle.py" \
  --root "$BUILD_CLOSURE/source" \
  --target-dir <fresh-external-target-directory> \
  --reviewed-source-closure "$BUILD_CLOSURE/reviewed-source-closure.json" \
  --reviewed-source-closure-sha256 <independently-pinned-64-lower-hex> \
  --toolchain-provenance "$BUILD_CLOSURE/production-build-closure.json" \
  --toolchain-provenance-sha256 <independently-pinned-64-lower-hex> \
  > <boi-build-private-target>/sealed-kagemusha-candidate-build.json
```

The production closure is prepared by a separately installed root provisioner
in a non-traversable root-only staging directory, verified before publication,
made `root:wheel` with no writable entries, and atomically renamed beneath a
root-owned non-user-writable parent. It contains the reviewed source and
descriptor, Python bootstrap, Cargo/rustc plus the complete sysroot and
non-system dynamic-library closure, canonical offline Cargo vendor/config,
exact Git and builtin helper directory, Apple developer/SDK/clang-resource
closure, linker, GPG verifier, and canonical keyring. Developer
Rustup/Homebrew/Xcode paths, online registries, ambient loader/Python state, and
a user-owned disk image are not production inputs.

The supervisor stable-hashes and copies the worker binary/report through
root-only staging, removes every write bit, and atomically publishes them with
`root-published-candidate-build.json` under the artifact tree digest. Android
staging accepts only the receipt-named root-owned binary, passed to
`scripts/run_kagemusha_v4_generation.py` together with
`--root-published-build-receipt` and its independent SHA-256 pin. A binary path
from the mutable worker report is never a cross-stage input.

Generation is a second exclusive `boi-build` worker phase, not an operator
command. The root supervisor must prove that the build-receipt UID has no other
process or session and launch the admitted immutable Python runner with
`/usr/bin/env -i`, `HOME=/var/empty`, `LANG=C`, `LC_ALL=C`,
`PATH=/usr/bin:/bin`, `TMPDIR=/private/tmp`, and `TZ=UTC`. The launcher itself
creates a previously absent mode-`0700` single-use output parent and rejects a
pre-existing or reusable parent. Its `candidate` and `resource-report`
directories must both be direct children of that parent.

Those bytes are explicitly
`provisional_boi_generation_worker_output`, not Android-lab or release input.
The root supervisor must keep the worker UID quarantined, stable-hash and
descriptor-copy the candidate and resource evidence through root-only staging,
remove write bits, and atomically publish the normalized exact tree with
`generation-worker-launch.json` and
`root-published-generated-candidate.json`. The root inventory is exactly those
two files plus `candidate/` and `resource-report/`. The launch schema
`boi.taira.generation_worker_launch.v1` binds the canonical command digest,
worker root/device/inode, and storage admission/reserve values. The generated
receipt schema is
`iroha.kagemusha.root_published_generated_candidate.v1`; the consumer requires
its independent SHA-256 pin, exact candidate/report inventories, successful
generation summary, launch and candidate-build receipt bindings, and matching
UID/source/toolchain identities. The existing candidate-*build* receipt covers
only the generator executable and sealed build report. Production finalization
runs under `/usr/bin/env -i`, admits both root-published receipt descriptors,
and commits both hashes to the authenticated release. It rejects the
provisional worker path; diagnostic Android staging remains non-production
evidence.

The seal hashes the canonical Git-index path, mode, and exact regular-file
bytes or symlink-target bytes for the complete source tree. The helper verifies
that identity and the exact admitted GPG signer before, during, and after a
locked release build; rejects ambient compiler controls; requires at least
24 GiB of installed physical memory; and prints canonical JSON containing the
exact binary and closure identities. The memory check is build admission, not
an OS-hard compiler limit. Pass only the root-published receipt-named binary
and the independently pinned receipt through
`scripts/run_kagemusha_v4_generation.py` with `generate-candidate`.

The separate `kagemusha_recursive_spend_v4_memory_benchmark` is calibration
only. It emits no candidate and its report is rejected as Android-lab,
promotion, or release evidence.

Direct unsupervised generation is rejected; the launcher rejects Cargo, shell
wrappers, and every subcommand except `generate-candidate`. The guarded
generator emits a worker-only owner-private directory containing exactly:

- `candidate-manifest.norito`, the canonical `CandidateV4` record;
- its canonical JSON view and SHA-256 sidecar;
- `topup-finality-roster-v4.norito`; and
- the canonical Eq-then-Ep inventory of eight framed `KRV4KEY` artifacts.

## Operator-supplied scenario seeds

Create a separate owner-private directory (`0700`, files `0600`) with exactly
the following proof-independent inputs:

```text
init-top-up-anchor-v4.norito
init-top-up-finality-proof-v2.norito
init-top-up-finality-roster-artifact-v2.norito
init-opening-v2.norito
init-output-membership-v4.norito
transfer-verifier-commitment-v2.bin
append-hop-01-recipient-request-v2.norito
append-hop-01-recipient-opening-v2.norito
append-hop-01-change-opening-v2.norito
append-hop-01-output-membership-v4.norito
append-hop-01-operation-id.bin
append-hop-01-block-height.txt
append-hop-01-verified-at-ms.txt
append-hop-02-recipient-request-v2.norito
append-hop-02-recipient-opening-v2.norito
append-hop-02-change-opening-v2.norito
append-hop-02-output-membership-v4.norito
append-hop-02-operation-id.bin
append-hop-02-block-height.txt
append-hop-02-verified-at-ms.txt
redeem-recipient-account-id.txt
unshield-verifier-commitment-v2.bin
redeem-hop-01-operation-id.bin
redeem-hop-01-block-height.txt
redeem-hop-02-operation-id.bin
redeem-hop-02-block-height.txt
redeem-sender-change-operation-id.bin
redeem-sender-change-block-height.txt
duplicate-input-recipient-request-v2.norito
duplicate-input-output-membership-v4.norito
duplicate-input-operation-id.bin
duplicate-input-block-height.txt
duplicate-input-verified-at-ms.txt
```

The finality roster must be byte-identical to the candidate roster. The anchor,
Commit QC, inclusion proof, and roster must come from an external finality
ceremony or capture that already commits to this exact candidate manifest. The
stager does not contact a live chain, manufacture consensus evidence, or define
a live-chain capture procedure. Consequently, a successful stage means that
the supplied external evidence was cryptographically verified and
candidate-manifest-bound; it does not claim that this repository captured it.

Operation IDs are distinct nonzero 32-byte values. The two verifier commitments
must equal the current source's canonical transfer and unshield verifier-key
hashes. Height and time files contain one positive canonical Android `Long`
decimal line and follow lifecycle/release-window order. Openings are disposable,
candidate-only secret material owned by the operator.

The source-built scenario authority canonically decodes every `.norito` file
into the same private/public carrier types used by the bridge. It verifies the
candidate-bound finality proof and roster PoPs/BLS aggregate/inclusion path,
derives the anchor and output notes from their openings, validates signed
recipient requests and opaque prover-material bindings, checks value
conservation, and runs the real output-membership circuit checks across init,
both hops, and the duplicate-input negative branch. Arbitrary text with a
`.norito` suffix is rejected. Raw secret buffers are zeroized on every return
path.

Prebuilt append/verify/redeem requests, proofs, bundles, results, or consensus
evidence are forbidden. Any missing or extra entry, symlink, hard link, public
file mode, source change, or content-address mismatch fails staging.

## Validate and stage

```sh
python3 scripts/stage_kagemusha_candidate_android_lab.py \
  --candidate-dir /absolute/path/to/generated-candidate \
  --scenario-seed-dir /absolute/path/to/operator-seeds
```

The stager builds two authorities in a new owner-private Cargo home and target,
offline and `--locked`, with two jobs and reduced scheduling priority. It does
not use a shared target or `cargo run`: it invokes the exact newly built
candidate and scenario binaries directly, hashes them before and after use, and
records both binary hashes plus Cargo/rustc identities. Wrapper, runner, target,
linker, preload, compiler-flag, Git, and Python environment injection is
removed; the source seal runs with isolated Python mode.

The candidate authority canonically decodes `CandidateV4`, extracts and
serializes its embedded manifest, validates the exact inventory and each
framed/payload content address, and revalidates the roster. Both authorities,
all input descriptors, the complete source identity, and the staged tree are
rechecked immediately before exclusive publication. The stager publishes once,
without overwrite, at:

```text
artifacts/kagemusha-candidate-evidence/<candidate-record-sha256>/<stage-manifest-sha256>/
  candidate-stage-manifest-v1.json
  evidence/candidate/candidate-v4.norito
  evidence/candidate/manifest-v4.norito
  evidence/candidate/candidate-validation-v1.json
  evidence/candidate/artifacts/<exact-eight-KRV4-files>
  scenario/<exact-seed-inventory>
```

`candidate-stage-manifest-v1.json` is canonical compact JSON with a fixed-point
self size. Its external SHA-256 is the second path component. It lists exactly
44 non-self files (three candidate records, eight artifacts, and 33 scenario
files), each with relative path, `0600` mode, size, and SHA-256; self-inclusion,
missing files, and extras are rejected. It also binds the candidate record,
embedded manifest, candidate validation report, complete scenario inventory,
source commit/tree pair, and both validator/toolchain identities.

The scenario inventory digest is SHA-256 over the domain
`iroha.kagemusha.android-candidate-scenario-inventory.v1\0`, big-endian `u32`
file count, then each `scenario/<name>` in UTF-8 byte order framed as big-endian
`u32` path length, path bytes, big-endian `u64` file size, and the raw 32-byte
file SHA-256.

`manifest-v4.norito` is the canonical Norito serialization of the embedded
manifest. It is deliberately distinct from the complete CandidateV4 record;
JSON is never re-encoded into either trust anchor.

After staging, build the marker-bearing candidate-only ARM64 library and use
`scripts/run_kagemusha_candidate_android_lab.sh`. Neither the stager nor the
lab output is a production release or a substitute for authenticated promotion
evidence.

## Compile-only Android contract

`ci/check_kagemusha_candidate_android_lab_compile.sh` performs an actual AGP
main and `androidTest` Kotlin compilation against a private, exact 44-entry
compile fixture. The Gradle property used by that check admits exactly
`compileDebugKotlin` and `compileDebugAndroidTestKotlin`; APK packaging,
staging, installation, instrumentation, and evidence export are rejected. The
physical-evidence runner never enables this property. This check catches Kotlin
or Android plugin integration failures without pretending to be device
evidence.

## Physical-device sequence and authority inputs

Run the source-sealed build phase first:

```sh
scripts/run_kagemusha_candidate_android_lab.sh --build-only \
  --candidate-sha256 "$CANDIDATE_SHA256" \
  --stage-sha256 "$STAGE_SHA256" \
  --source-commit "$SOURCE_COMMIT" \
  --source-tree-sha256 "$SOURCE_TREE_SHA256" \
  --generation "$GENERATION" --slot-id "$SLOT_ID"
```

This builds offline with bounded workers, retains separately signed main and
`androidTest` APKs, verifies their v2/v3 signing certificate and bytes, and
prints the exact 32-byte candidate-stage challenge. Collect a fresh hardware
StrongBox/KeyMint attestation for those challenge bytes. The authorized
external device-lab capture must then execute and export the complete canonical
lifecycle with those exact APKs and produce a complete candidate-bound slot;
the lab evidence authority signs that complete slot. An attestation-only or
partially populated slot is not a valid input to the next command.

The full phase requires explicit, offline authority material. Paths must be
canonical absolute regular files and every digest is supplied by the caller:

```sh
scripts/run_kagemusha_candidate_android_lab.sh \
  --candidate-sha256 "$CANDIDATE_SHA256" \
  --stage-sha256 "$STAGE_SHA256" \
  --source-commit "$SOURCE_COMMIT" \
  --source-tree-sha256 "$SOURCE_TREE_SHA256" \
  --generation "$GENERATION" --slot-id "$SLOT_ID" \
  --attestation-slot "$SLOT_PATH" \
  --trusted-signer-public-key "$TRUSTED_SIGNER_PUBLIC_KEY" \
  --apksigner "$PINNED_APKSIGNER" \
  --apksigner-sha256 "$PINNED_APKSIGNER_SHA256" \
  --openssl "$PINNED_OPENSSL" \
  --openssl-sha256 "$PINNED_OPENSSL_SHA256" \
  --android-attestation-trust-root "$ANDROID_ATTESTATION_ROOT" \
  --android-attestation-trust-root-sha256 "$ANDROID_ATTESTATION_ROOT_SHA256" \
  --android-attestation-revocation-status "$ANDROID_REVOCATION_STATUS" \
  --android-attestation-revocation-status-sha256 "$ANDROID_REVOCATION_STATUS_SHA256"
```

Repeat the aligned trust-root path/digest pair when more than one root is
authorized. The runner never discovers trust roots or revocation status from
the SDK, PATH, or network. Before any `adb` command it invokes the authoritative
slot validator in isolated Python mode. That validator verifies the signed
evidence, exact candidate/APK/source bindings, APK signatures, KeyMint
certificate chain and challenge, attestation application package/signer,
offline revocation snapshot, and authority pins. The runner consumes only the
validator's successful one-slot machine summary.

The subsequent instrumentation is an independent confirmation rerun, not the
original capture that produced the signed reference slot. Its complete binding
and lifecycle semantics are checked against the authenticated reference. Every
deterministic field and causal digest link must match; only positive
`duration_nanos` measurements may differ. The final confirmation receipt binds
the pulled binding/transcript sizes and SHA-256 values plus the authoritative
`candidate-confirmation-comparison-v1.json` report. The comparison is another
isolated invocation of the same digest-pinned authority: it fully revalidates
the signed reference slot, measures both reference and confirmation files, and
fails before receipt publication unless the machine report is successful and
its paths, sizes, hashes, authority projection, and comparison policy all
match the retained files.

Instrumentation then verifies the installed base APK hashes before and after
each lifecycle/export process. The exported transcript contains the exact
28-operation causal sequence across the process restart, including digest
links between built requests, native results, restored branches, proof checks,
the observed-branch duplicate rejection, and three redemptions. Cleanup is
scoped to the two non-shipping lab packages; it never signals unrelated apps,
builds, shells, or Codex processes.

The immutable build-only/full receipts bind source seals, tool identities,
authority digest projections, canonical four-command templates, and the exact
lifecycle/export argv arrays. Existing different receipts or toolchain audits
are never overwritten.
