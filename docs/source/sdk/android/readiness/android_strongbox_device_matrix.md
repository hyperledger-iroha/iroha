# Android StrongBox Offline Payments Device Matrix

Last updated: 2026-06-12

This matrix gates production readiness for Android offline-offline payment
flows. A device row is ready only after the lab attaches signed evidence for
StrongBox/KeyMint attestation, one-use key rotation, rollback rejection, ABI-6
recursive spend, ABI-7 one-hop recursive compact-token proof probing, and
ABI-7 package-backed multi-hop recursive compact proof probing.

| Device family | Minimum OS | StrongBox / KeyMint gate | Kagemusha recursive compact gate | Status |
| --- | --- | --- | --- | --- |
| Google Pixel 6 / 6a | Android 14 | Physical Pixel 6 / Android 16 StrongBox export captured and signed in slot `google-pixel-6-6a-physical-1781077370103` | Focused production command passes with ABI-6/ABI-7 JNI load assertions; signed slot carries one-hop `recursive_compact_v1` JNI probe and package-backed multi-hop probe state | Blocked by remaining standard-matrix families |
| Google Pixel 7 / 7 Pro | Android 14 | Pending lab attestation export | One-hop `recursive_compact_v1` proof probe required; package-backed multi-hop proof probe required | Blocked |
| Google Pixel 8 / 8a / 8 Pro | Android 15 | Pending lab attestation export | One-hop `recursive_compact_v1` proof probe required; package-backed multi-hop proof probe required | Blocked |
| Google Pixel Fold / Tablet | Android 15 | Pending lab attestation export | One-hop `recursive_compact_v1` proof probe required; package-backed multi-hop proof probe required | Blocked |
| Samsung Galaxy S23 | Android 14 | Pending lab attestation export | One-hop `recursive_compact_v1` proof probe required; package-backed multi-hop proof probe required | Blocked |
| Samsung Galaxy S24 | Android 15 | Pending lab attestation export | One-hop `recursive_compact_v1` proof probe required; package-backed multi-hop proof probe required | Blocked |

Production release criteria:

- ABI 6 recursive spend JNI probes pass on every required device family.
- ABI 7 recursive compact-token JNI probes prove and verify the packaged
  one-hop LEN=4 path on every required device family.
- Slot probe-state fields (`abi6_recursive_spend_jni_probe`,
  `abi7_recursive_compact_jni_probe`, and
  `abi7_recursive_compact_prover_state`) must be exact lowercase strings with no
  surrounding whitespace or control characters. The ABI-6 recursive-spend probe
  must be exactly `passed`; `ok` is not accepted as a production alias.
- ABI 7 recursive compact prover calls that require multi-hop append-batch
  composition produce package-backed compact tokens when the key package is
  supplied, while empty, malformed, or dummy-proof local archives remain
  caller-input errors or soft-invalid verifier results. Kotlin/JVM and Java Android validate recursive compact-token and
  record-backed recursive aggregation inputs as non-empty Norito archives before
  JNI dispatch.
- Wallet rollback tests prove that old encrypted wallet state cannot be restored
  after one-use key rotation. The wallet integrity transcript is bound by
  `slot.json` with a wallet integrity transcript path and SHA-256; it must prove
  one-use key rotation, old-key invalidation, stale-snapshot rejection, changed
  key ids, changed wallet state after rotation, and preserved active state after
  the rollback attempt is rejected.
- End-to-end D2D payment transcripts prove the payer and payee wallets stayed
  offline during the handoff, the received payload hash equals the sent payload
  hash, the receiver accepted the redeem path, a duplicate spend was rejected,
  payer/payee wallet-state hashes changed, the one-use key id and transport
  session id are hash-bound, and the resulting queue digest matches
  `queue/pending_queue.json`.
  The d2d payment transcript is bound by `slot.json` with a D2D payment
  transcript path and SHA-256, and that path must stay under `handoff/`.
  Each production slot must also keep `telemetry/telemetry.json`,
  `telemetry/status.ndjson`, `attestation/harness-result.json`,
  `attestation/result.json`,
  `attestation/report.json`, `queue/pending_queue.json`, and
  `logs/runtime.log`; signed evidence rejects refreshed manifests that omit any
  of those base artifacts. Those required base artifacts must be non-empty and
  no larger than 16 MiB each. Telemetry JSON must bind exactly to the slot id
  without whitespace, control-character, or type normalization and must use the exact `kagemusha-device-lab` suite value.
  status NDJSON must include an `ok` status as an exact lowercase value,
  must use LF line endings with a trailing newline, nonblank status lines must
  not rely on surrounding whitespace being stripped, failure statuses must be
  absent, and any `slot_id` field in a status record must bind exactly to the
  slot id without whitespace or control-character normalization. `logs/runtime.log` must carry the Kagemusha device-lab
  completion marker without build/test failure markers. Attestation harness, result, and
  verifier-report strings are exact: slot bindings, status values, identity
  digests, and StrongBox labels are rejected if they require whitespace
  trimming, case normalization, or control-character filtering. Scanner,
  raw-puller, attestation-report, readiness, and release-bundle diagnostics must redact
  control-character-bearing JSON keys, summary fields, and artifact labels
  instead of echoing unsafe terminal strings, and raw ADB stderr details with
  control characters must be redacted before CLI display.
  The device-lab root, operator-supplied root ancestors, slot parent
	  directories, slot path ancestors, slot directories, slot metadata, the
	  SHA-256 manifest, evidence directories, and artifact files must be ordinary
	  directories or regular files and must not be symlinks or hardlinks; the
	  scanner also rejects unreadable slot directory or parent metadata before
	  slot traversal, and the scanner and signing helper reject linked or
	  special-file slot artifacts instead of following or hashing external aliases.
	  Scanner and rollup missing-root decisions consume `lstat()`-classified root presence
	  instead of calling `Path.exists()`. The shared device-lab JSON
	  loader also rejects symlinked ancestor directories before parsing JSON and
	  decodes bytes from one opened regular file after path-identity
	  revalidation, so direct validation of slot metadata, attestation,
	  transcript, or signed evidence files cannot read through aliased directories
	  or post-preflight leaf aliases.
	  Lower-level direct symlink, hardlink, and regular-file artifact validators
	  reject secret-looking slot paths before traversing, stat-ing, or
	  classifying slot artifacts. The symlink validator now reports unreadable
	  slot-metadata, artifact-directory, and nested-artifact metadata before alias
	  classification, and the regular-file validator classifies leaves with
	  `lstat()` before any `exists()` preflight can mask unreadable metadata.
	  Hardlink and regular-file validators also classify artifact directories
	  with `lstat()` before any `exists()` preflight, and the regular-file
	  validator classifies nested artifacts before any `is_symlink()` preflight.
	  Manifest artifact digest validation classifies slot-relative ancestor
	  directories with `lstat()` before symlink checks, so nested artifact paths
	  do not depend on `Path.is_symlink()`, and binds each `sha256sum.txt`
	  digest read to the opened file identity.
	  Required-artifact shape checks, required status/runtime text reads, the
	  D2D queue digest binding, and the signed-evidence artifact binding also
	  classify artifacts with `lstat()` before any `is_file()` preflight, and
	  signed-evidence `artifact_digests` bind each hashed artifact to the opened
	  file identity. Signed-evidence string fields are exact: surrounding
	  whitespace and non-printing control characters are rejected before matching
	  them against `slot.json` or signature metadata. The signed-evidence
	  generator enforces the same exactness for slot metadata strings,
	  `signed_evidence_artifact_path`, `attestation_certificate_chain_path`,
	  raw test commands, and signer key ids before it can emit signed evidence.
	  Direct SHA-256 manifest parser and verifier helper calls reject
	  secret-looking slot paths, unreadable slot-root metadata, symlinked slot
	  roots, and symlinked slot ancestors before parsing `sha256sum.txt` or
	  traversing slot artifacts, and reject unreadable-metadata and hardlinked
	  `sha256sum.txt` manifests before reading manifest bytes or discovering slot
	  files. The manifest parser binds `sha256sum.txt` bytes to the opened file
	  identity so post-preflight regular-file swaps fail closed, and nonblank
	  manifest lines must not rely on leading or trailing whitespace
	  normalization or leading `*` path normalization before digest/path parsing.
	  Direct slot-file discovery reports unreadable slot-root and
	  artifact-directory metadata through caller error lists, returns no artifacts
	  for secret-looking slot paths, symlinked slot ancestors, missing roots,
	  non-directory roots, or symlinked slot roots before traversal, and skips symlinked artifact
	  directories instead of discovering files through them.
	  Direct manifest verification rejects entries under symlinked artifact
	  directories before reading or hashing bytes, and revalidates every
	  `sha256sum.txt` artifact entry for secret-looking names, symlinks,
	  hardlinks, and non-regular files immediately before digesting it; read-time
	  byte failures become structured validation errors after that preflight.
	  Direct attestation, D2D handoff, wallet-integrity, required-artifact,
	  signed-evidence, and production-metadata validator helper calls repeat the
	  same slot-path rejection before parsing artifacts, reading transcript
	  bindings, or hashing signed evidence. Signed-evidence artifact digest
	  verification also revalidates required artifact paths for secret-looking
	  names, symlinks, hardlinks, and non-regular files immediately before
	  hashing the bytes claimed by `artifact_digests`. Slot-metadata digest
	  checks also revalidate `slot.json`-referenced attestation-chain,
	  offline-wallet APK, and signed-evidence artifact paths before reading bytes
	  for SHA-256 comparison, then bind the bytes to the opened file identity so
	  post-preflight regular-file swaps fail closed. D2D handoff and
	  wallet-integrity transcript bindings, including `queue/pending_queue.json`,
	  use the same digest-time revalidation before comparing SHA-256 values.
	  Required status NDJSON and runtime log marker checks also revalidate their
	  slot-relative files for symlinks, hardlinks, symlinked artifact directories,
	  non-regular files, and secret-looking names immediately before text
	  decoding, with the same opened-file identity binding.
- StrongBox/KeyMint attestation chains bind the app challenge and device
  security level expected by the offline wallet policy and must come from a
  physical device attestation, not an emulator or simulator run.
  The production attestation summary at `attestation/result.json` must report
  ok status, a StrongBox/KeyMint security level,
  `physical_device_attestation: true`, the attestation certificate chain path
  and SHA-256, and the same slot id, device fingerprint, OS build id, app
  package, app signing certificate, attestation challenge, and offline wallet
  policy hashes as `slot.json`.
  The attestation verifier report at `attestation/report.json` is a separate
  closed-schema artifact: it repeats the slot id, device fingerprint, OS build
  id, app package, attestation challenge, attestation certificate-chain path,
  and certificate-chain SHA-256 from `slot.json`, names the verifier, and
  reports `verification.status` as exact `ok` with StrongBox/KeyMint and
  physical-device attestation set to true. The signer refuses to create
  `evidence/signed-evidence.json` when this verifier report is missing,
  malformed, weakly attested, or not bound to the slot metadata.
  Generate that closed report from the host-side StrongBox verifier output with
  `python3 scripts/kagemusha_android_attestation_report.py --harness-result <android_keystore_attestation_result.json> --slot-id <slot-id> --device-fingerprint <adb-ro.build.fingerprint> --os-build-id <adb-ro.build.id> --attestation-certificate-chain <chain.pem> --physical-device-attestation --out <report.json>`.
  The writer refuses non-StrongBox verifier results, unexpected verifier-result fields,
  noncanonical harness alias, level, or challenge strings, noncanonical expected
  challenge hex, whitespace-normalized or control-character-bearing identity
  arguments, normalized StrongBox/KeyMint level labels, challenge digest drift,
  PEM chain-length mismatches, whitespace-normalized, control-character, or unsafe
  chain paths, aliased or secret-looking harness-result source paths, and
  reports that do not carry an explicit physical-device assertion. It writes the
  report through a
  fsynced same-directory temporary file, atomically replaces the output,
  identity-checks failed temporary cleanup, syncs the captured output-parent
  identity, and then reads the report back before success.
  The referenced chain artifact must be a non-empty `.pem` or `.der` file under
  `attestation/`; PEM chains must contain certificate boundaries, DER chains
  must start with an ASN.1 SEQUENCE byte, and oversized chain payloads are
  rejected.
  If the attestation exporter emits both `slot` and `slot_id`, both names must
  match each other and the slot directory.
  The summary is a closed schema, and all SHA-256 values in it must be
  canonical lowercase hex.
- Lab reports include raw test commands, device fingerprints, OS build IDs, and
  signed evidence artifact hashes.
  Across the required standard matrix, production slots must not reuse the same
  device fingerprint or attestation challenge; copied lab evidence is blocked
  even if each individual slot is otherwise signed and hash-consistent. The
  scanner and release-bundle validator expose this only as hash-backed
  `duplicate_bindings` metadata with unique sorted slot lists, so raw
  fingerprints or challenge material do not leak into release summaries.
  Existing release manifests must also retain exact standard-matrix family
  coverage, an empty missing-family list, and canonical trusted-signer SHA-256
  pins during `--verify-existing`.
  The signed raw command list must exactly match the canonical Android
  production device-lab commands: the first runs the release assembly steps
  `:client-android:assembleRelease` and
  `:offline-wallet-android:assembleRelease`, then
  `:offline-wallet-android:connectedDebugAndroidTest` with the focused
  `org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest`
  plus `org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest`
  harnesses. The second installs the lab app with
  `:offline-wallet-lab-app:assembleRelease`,
  `:offline-wallet-lab-app:installRelease`, and
  `:offline-wallet-lab-app:installReleaseAndroidTest`, then runs
  `adb shell am instrument -w -e class
  org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest
  org.hyperledger.iroha.sdk.offline.wallet.lab.test/androidx.test.runner.AndroidJUnitRunner`.
  The non-export class filters resolve to instrumentation tests under
  `kotlin/offline-wallet-android/src/androidTest/java/org/hyperledger/iroha/android/offline/`.
  Marker-only commands, comments, or `echo` wrappers are rejected even if they
  contain those strings.
- Capture raw attached-device artifacts with
  `org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest`.
  This instrumentation harness is executed through the dedicated
  `kotlin/offline-wallet-lab-app` application module so the target context is
  `org.hyperledger.iroha.sdk.offline.wallet.lab`, not the oversized androidTest
  package. It generates a StrongBox-backed Android Keystore key with a slot
  challenge, exports `attestation/keymint-certificate-chain.pem` plus
  `attestation/harness-result.json` and `attestation/result.json`, and writes pullable D2D, wallet-integrity,
  telemetry, queue, and runtime-log artifacts under the app files directory at
  `kagemusha-device-lab/<slot-id>`. It also writes
  `kagemusha-device-lab/latest-slot.txt` so the host can locate the newest raw
  export before running the host-side attestation verifier report writer and
  slot assembler. The exporter hash-binds the installed package returned by
  `Context.getPackageCodePath()` as the offline-wallet APK digest; deterministic
  slot-id placeholder digests and hashes of the androidTest APK are not
  production evidence.
  Pull the newest raw slot from an attached physical device with
  `python3 scripts/kagemusha_pull_android_device_lab_raw_slot.py --serial <adb-serial> --run-as-package org.hyperledger.iroha.sdk.offline.wallet.lab --out-root target/kagemusha-android-raw --summary-out target/kagemusha-android-raw-pull-summary.json`.
  The puller rejects empty, surrounding-whitespace-normalized,
  control-character, or secret-looking ADB executable, serial, run-as package,
  and device-root arguments before building any `adb` command. It then reads
  `latest-slot.txt` through `run-as`, requires that query output to be exactly
  one slot id plus a trailing newline, streams the selected slot with
  `adb exec-out ... tar`, refuses to overwrite an existing local raw slot, and
  rejects symlink, hardlink, special-file, traversal, duplicate, oversized,
  directory-colliding, unreviewed extra-artifact, and slot-mismatched tar
  members before the raw artifacts can be assembled into
  signed production evidence. The
  `latest-slot.txt` included in the tar stream must also be exactly the selected
  slot id plus a trailing newline; surrounding whitespace or otherwise
  normalized matches are rejected. After raw-slot validation, the host puller
  rechecks the final destination, creates that slot directory exclusively with
  owner-only permissions, moves only the expected top-level artifact
  directories into it through opened stage and final directory descriptors,
  binds the created slot-directory identity through each parent-fd slot-entry
  stat, move, and slot fsync, binds the output-root identity through the parent fsync,
  and removes partial installs through the identity-bound output-root file
  descriptor only when the destination entry still names the directory created
  by the puller. Temporary extraction cleanup also revalidates the captured
  temp-directory identity through its parent descriptor before removing
  anything, so a swapped staging path is left untouched. The host-side
  `latest-slot.txt` writer uses the same
  fail-closed output discipline: it fsyncs the file bytes, atomically replaces
  the output, verifies readback through an opened-file identity binding that
  rejects symlinks, hardlinks, and path swaps, and fsyncs the identity-bound
  output root.
  A raw pull is not assembly-ready unless it contains
  `attestation/harness-result.json`; the puller verifies that
  the harness challenge and `chain_length` match the pulled challenge and PEM
  certificate count, and requires both `slot` and `slot_id` in
  `attestation/result.json` to match the selected slot id. The raw result JSON
  is closed-schema; unexpected fields fail before assembly. The raw
  `attestation_certificate_chain_sha256` and `attestation_challenge_sha256`
  fields must be canonical lowercase SHA-256 hex digests and must match the
  pulled certificate-chain and challenge bytes before the host verifier report
  renderer consumes that file to produce `attestation/report.json`. The raw
  `attestation/challenge.hex` file must also be lowercase hexadecimal with
  exactly one trailing newline; uppercase or whitespace-normalized challenge
  files are rejected. Raw result identity strings must be non-empty,
  trim-stable, and free of secret-looking material; app-signing and offline
  wallet policy digests must also be canonical lowercase SHA-256 hex, and raw
  KeyMint/security levels must be exact `STRONGBOX`. The puller also parses
  `queue/pending_queue.json`, `telemetry/telemetry.json`,
  `handoff/d2d-payment.json`, and `wallet/integrity.json` as strict JSON before
  assembly: each slot-bound artifact must match the selected slot id exactly, D2D
  transcript booleans must prove offline payer/payee transport and double-spend
  rejection, and wallet integrity must prove one-use key rotation plus rollback
  rejection. D2D and wallet transcript string fields must match slot metadata
  exactly without whitespace or control-character normalization.
  `telemetry/telemetry.json` must carry the exact
  `kagemusha-device-lab` suite label. `telemetry/status.ndjson` is parsed
  line-by-line with duplicate-key rejection; files must use LF line endings
  with a trailing newline, nonblank lines must not contain surrounding
  whitespace, status strings must be exact lowercase values with no surrounding
  whitespace or control characters, failure statuses are rejected, and any
  status-record `slot_id` must be an exact selected-slot binding with no
  whitespace or control-character normalization; `logs/runtime.log` must contain
  the completion marker and must not contain build, test, panic, traceback, or fatal
  exception markers. Harness strings must be canonical before raw pulls,
  signed-slot assembly, or signed-slot scanning can accept them: aliases cannot
  carry surrounding whitespace or control characters, StrongBox levels must use
  exact accepted labels without surrounding whitespace, control characters, or
  secret-looking material, and `challenge_hex` must be lowercase hexadecimal
  without whitespace. Signed slot metadata must also keep
  `keymint_security_level` as an exact accepted StrongBox label; lowercase or
  otherwise case-normalized values are rejected. The host attestation report renderer enforces the same
  alias, StrongBox/KeyMint level, and canonical challenge format, including
  `--expected-challenge-hex`, and rejects whitespace-normalized or
  control-character-bearing slot id, device fingerprint, OS build, app package,
  verifier names, StrongBox/KeyMint level labels, harness strings, chain paths,
  and PEM certificate-count mismatches before writing
  `attestation/report.json`. When `--summary-out`
  is supplied, the raw-pull summary is
  serialized as strict JSON, capped before temporary-file creation, atomically
  replaced after fsync, read back through an opened-file identity binding that
  rejects symlinks, hardlinks, and path swaps, and followed by an
  identity-bound parent directory fsync. The summary's `artifact_sha256`
  inventory must cover every
  required raw artifact, and each digest is read through a separate opened-file
  identity binding that rejects symlinks, hardlinks, and file swaps.
- Assemble a production slot from completed attached-device artifacts with
  `python3 scripts/kagemusha_android_device_lab_slot.py --slot-root artifacts/android/device_lab --slot-id <slot-id> --device-family "<standard-family>" --attestation-result <result.json> --attestation-harness-result <harness-result.json> --attestation-report <report.json> --attestation-certificate-chain <chain.pem> --offline-wallet-apk <offline-wallet-release.apk> --d2d-payment-transcript <d2d-payment.json> --wallet-integrity-transcript <integrity.json> --telemetry-json <telemetry.json> --status-ndjson <status.ndjson> --pending-queue-json <pending_queue.json> --runtime-log <runtime.log> --private-key <runtime-only-lab-private-key.pem> --public-key <lab-public-key.pem> --signer-key-id <lab-signer-id>`.
  The assembler reads the attached device identity from ADB unless explicit
  device fingerprint and OS build overrides are supplied; each `getprop`
  response must be exactly one LF-terminated value and the value must not rely
  on trimming surrounding whitespace. It refuses to overwrite an existing slot
  directory and requires signing inputs by default; unsigned staging slots
  require the explicit `--allow-unsigned` flag and remain rejected by production
  readiness. Every source artifact copied by the
  assembler is read through symlink-free ancestors and an opened-file identity
  binding, then the staged copy is parent-synced and read back through its own
  opened-file identity binding, so symlinked source directories, hardlinked
  leaves, post-preflight source swaps, and copied-byte drift fail before a
  signed slot can be installed. The normalized
  `attestation/result.json`, `attestation/report.json`, and `slot.json` writes
  use fsynced temporary files, identity-bound temporary cleanup on failed
  writes, identity-bound parent fsync, and opened-file readback before
  manifesting. The final stage publish uses directory file
  descriptors pinned to the captured device-lab root, temp-parent, and
  staged-slot identities and fsyncs the root descriptor, so path swaps before
  final publish fail closed. Temporary staging cleanup also checks the captured
  temp-parent identity before removing anything. The preserved
  `attestation/harness-result.json` is revalidated during assembly with the
  same exact-string StrongBox level and lowercase challenge-hex policy enforced
  by the raw puller and production scanner.
- Generate signed lab evidence from an already completed slot with
  `python3 scripts/sign_android_device_lab_evidence.py --slot artifacts/android/device_lab/<slot-id> --private-key <runtime-only-lab-private-key.pem> --public-key <lab-public-key.pem> --signer-key-id <lab-signer-id>`.
  Before signing or writing outputs, the helper validates the preserved
  `attestation/harness-result.json` against the slot challenge and copied
  certificate-chain count.
  The helper writes the signed evidence artifact, refreshes the slot artifact
	  hash, and rewrites `sha256sum.txt`; private key paths are runtime inputs only
	  and are not written to slot metadata or JSON summaries. Runtime private-key
	  inputs and trusted lab public-key inputs must have symlink-free ancestors and
	  be regular, non-symlink, non-hardlinked files before OpenSSL is invoked;
	  unreadable private-key and trusted public-key leaf metadata is rejected
	  before the helper classifies the key as missing or malformed.
		  Secret-looking key path strings are rejected before OpenSSL is invoked, so
		  operator-local tokens cannot be echoed by key parsing diagnostics. Signature
		  verification preserves key-path validation failures separately from
		  private/public key mismatch failures, and temporary OpenSSL staging or
		  signature-output failures become structured signer/verifier errors after
		  staged-byte readback and signature-output reads are bound to opened file
		  identities. The
		  signer helper also rejects secret-looking `--slot`, `--output`, and
		  `--signer-key-id` runtime arguments before reading slot metadata, and
	  rejects padded or control-character signer key ids before metadata reads.
	  The signed-slot assembler also rejects padded or control-character
	  `--slot-id`, requested device-family, and device identity override inputs
	  before path construction or ADB fallback.
	  Device-lab JSON summaries also carry a local root label instead of the
	  absolute lab path, and the validator does not print the absolute
	  `--json-out` path. Secret-looking `--root` and `--json-out` argument
	  strings are rejected before root discovery or summary writes, and the
	  direct root validator repeats the secret-path and readable-metadata checks
	  before slot discovery.
		  The direct summary writer repeats the `--json-out` secret-path check before
		  writing JSON, unlinks failed temporary outputs only after matching the
		  captured temp-file identity through the parent descriptor, then binds
		  summary readback to the opened file identity.
		  Symlinked, hardlinked, non-regular, or unreadable-metadata `--json-out`
		  aliases are rejected before the scanner writes a summary.
		  The shared slot JSON loader binds parsed JSON bytes to the preflight
		  `lstat()` identity so post-preflight regular-file swaps fail closed.
		  Discovered slot directory names that contain whitespace,
		  non-printing control characters, or secret-looking material are rejected
		  before artifact traversal; unsafe names are redacted before summary
		  serialization. The helper also rechecks the signed-evidence and
	  SHA-256 manifest output ancestors, parents, and leaves immediately before
	  writing, so symlinked, hardlinked, or non-regular output aliases are rejected
	  even if earlier slot validation has already passed; missing output parents
	  are checked again immediately after creation, and dangling symlink output
	  leaves are treated as aliases even when their targets are missing.
	  Before parsing `slot.json`, the helper also preflights the slot directory,
	  slot path ancestors, metadata, manifest, and current slot artifacts for
	  symlink, hardlink, special-file aliases, and secret-looking artifact names;
	  the signer slot preflight also classifies the slot directory and parent
	  with `lstat()`, so unreadable slot or parent metadata fails closed before
	  metadata-derived signing work can start from an aliased or secret-bearing
	  lab bundle.
	  Direct signer metadata-loader and manifest-rewrite helper calls also reject
	  secret-looking slot paths plus unreadable slot or parent metadata before
	  metadata parsing, artifact traversal, hashing, or manifest replacement can
	  occur. The lower-level signer
	  artifact-digest builder also reruns the slot preflight before hashing
	  required signed-evidence artifacts, so direct calls cannot hash through
	  secret-bearing or aliased slot paths. The per-artifact digest helper also
	  rechecks each relative artifact path for secret-looking names, unreadable
	  leaf metadata, symlinks, hardlinks, and non-regular files immediately before
	  digest reads used by signed evidence and manifest rewrites, then binds each
	  digest read to the opened regular-file identity.
		  Low-level signer output writers reject secret-looking signed-evidence and
		  manifest paths before creating output parents or writing files, reject
		  absolute signed-evidence output path resolver failures with the structured
		  `signed evidence output path could not be resolved` error, reject unreadable
		  output parent or leaf metadata before write or digest reads, classify
		  output parents with `lstat()` before any `Path.is_dir()` preflight, reject
		  dangling symlink output leaves before following them, bind post-write
		  readback verification to the opened output file identity, rerun parent and
		  ancestor checks after creating missing output parents, sync the captured
		  output-parent identity after atomic replacement, and the signing helper revalidates the
		  signed-evidence output as a regular non-symlink, non-hardlinked file before
		  hashing it back into `slot.json`, then bind that digest read to the
		  opened file identity.
	  Direct SHA-256 manifest rewrites run the same slot/artifact shape preflight
	  before hashing or replacing `sha256sum.txt`, so secret-looking artifact
	  names cannot be serialized into the manifest. Scanner and signing-helper
		  direct calls also guard top-level slot artifact enumeration; if a slot
		  directory cannot be listed after path preflight, validation reports
		  `slot directory could not be listed` and does not silently produce a
		  partial manifest or signed evidence digest set. Manifest inventory
		  discovery also reports unreadable artifact metadata as structured
		  slot-artifact blockers instead of omitting those files from
		  `sha256sum.txt` coverage, and direct hardlink artifact validation reports
		  unreadable file metadata before hardlink checks. Digest-time validators
		  for manifest entries, slot metadata bindings, and signed-evidence artifact
		  digests also separate missing artifacts from unreadable leaf metadata before
		  symlink, non-regular, hardlink, or read checks.
			  The shared JSON loader rejects secret-looking direct file paths and
		  symlinked ancestors before parsing metadata, attestation, handoff,
		  wallet-integrity, or signed-evidence JSON, and converts unreadable leaf
		  metadata, unreadable bytes, or non-UTF-8 JSON bytes into structured read
		  errors instead of tracebacks.
- Production lab bundles must pass
  `python3 scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab --require-slot --require-kagemusha-production-evidence --require-kagemusha-standard-matrix --trusted-signer-public-key <lab-public-key.pem>`.
  When selecting explicit slots, each `--slot` value must be a single safe slot
  directory name under the lab root, not a filesystem path, and it must not
  contain whitespace.
  Release evidence rollups should then run
  `python3 scripts/kagemusha_production_readiness.py --device-lab-root artifacts/android/device_lab --trusted-signer-public-key <lab-public-key.pem> --min-signed-at-utc 2026-06-06T00:00:00Z --max-signed-at-future-skew-seconds 300 --max-lineage-proof-evidence-future-skew-seconds 300 --max-compact-key-evidence-future-skew-seconds 300 --summary-out dist/kagemusha-production-readiness.json`,
  which combines the ABI-6 Reserved-lineage manifest, ABI-7 fail-closed
  contract, Reserved-lineage proof evidence, ABI-7 recursive compact key evidence,
  signed Android evidence, and standard device-family coverage into a
  strict ready/blocked JSON summary. A ready summary must then be packaged with
  `python3 scripts/kagemusha_release_bundle.py --repo-root . --bundle-root . --readiness-summary dist/kagemusha-production-readiness.json --lineage-proof-evidence artifacts/kagemusha/lineage-proof-evidence.json --compact-key-evidence artifacts/kagemusha/recursive-compact-key-evidence.json --device-lab-root artifacts/android/device_lab --trusted-signer-public-key <lab-public-key.pem> --out dist/kagemusha-production-release-bundle.json`.
  The release bundle manifest uses
  `iroha.kagemusha.production_release_bundle.v1`, recomputes the checked-in
  ABI-6, ABI-7, and lineage release-tooling trust roots, hash-binds the
  readiness summary, Reserved-lineage proof evidence, ABI-7 compact key
  evidence, and scanner-validated Android signed-evidence map. It records
  bundle-relative per-slot Android signed-evidence artifact paths and SHA-256
  digests after revalidating each slot name, keeps the Reserved-lineage and
  ABI-7 compact artifact size maps from the recomputed readiness summary, records
  every packaged lineage artifact, compact key artifact, and production proof
  log plus the compact key generator log with bundle-relative path, SHA-256
  digest, and byte size, and
  rejects summary drift,
  duplicate JSON keys, unexpected top-level or section-level summary fields,
  per-section blockers in a ready summary, secret-looking paths,
  secret-looking strings anywhere inside the readiness summary, plain-text
  placeholder compact key artifacts in the compact-key artifact inventory, evidence
  outside `--bundle-root`, symlinked bundle roots, and symlinked or hardlinked
  manifest outputs. Secret-looking trusted signer key paths are rejected before
  key loading.
  Newly-created manifest output parents are revalidated before writing, then the
  manifest is written through a fsynced temporary file, atomically replaced into
  place, synced through an identity-bound parent directory handle, and read back
  before success is reported. The
  checked-in ABI-6 manifest must be a regular non-symlink, non-hardlinked file
  with symlink-free ancestors before its release contract is trusted. The
  checked-in ABI-7 fail-closed and Reserved-lineage release-tooling marker
  source files must also be ordinary non-symlink, non-hardlinked files before
  their marker text can satisfy readiness, and direct helper calls reject
	  secret-looking trust-root file paths before metadata checks or JSON/source
	  parsing. Source-marker text reads also rerun that validator immediately before
	  loading marker text, and unreadable or non-UTF-8 ABI-7 and Reserved-lineage
  marker files become structured blockers instead of decode tracebacks. ABI-7 compact readiness then checks the concrete core and
  bridge function contracts: one-hop prove/preverify/verify paths must route
  through the compact verifier-slice contract, tiny dummy proof payloads must
  fail the compact proof-size floor before expensive backend verification,
  multi-hop proving must produce package-backed compact tokens with the
  matching key artifacts, and bridge wrappers must preserve fail-closed
  malformed-input handling. Evidence signed before the release cutoff
  or future-dated beyond the release validator clock-skew allowance remains
  blocked even when its signature and hashes are otherwise valid. Freshness checks
  use the scanner-validated signed-evidence timestamp from the slot report rather
  than re-opening `slot.json` or `evidence/signed-evidence.json` in the rollup.
  The Reserved-lineage proof evidence JSON must live beside the
  `artifacts/kagemusha` `.norito`, `.record.norito`, `.vk`, and `.pk` files it
  declares, plus `record-archive-proof.log`; it must keep the canonical
  `lineage-proof-evidence.json` filename because renamed or copied evidence
  JSON files are rejected, and symlinked evidence files or symlinked evidence
  ancestors are also blocked. The rollup recomputes their SHA-256 digests and
  artifact byte sizes from local bytes, requires the adjacent artifacts and proof log to be regular
  non-symlink, non-hardlinked files, classifies artifact/log
  missing-vs-unreadable state from the lstat-backed local-file validators rather
  than `Path.is_file()`, and re-checks the proof log's passing cargo result as
  one expected `test ... ok` line plus one one-test cargo result after
  rerunning the lineage local-file validator immediately before reading proof-log
  text, and it re-checks the exact production `cargo test -p iroha_core` command,
  with no appended shell commands. Marker-stuffed proof logs with extra passing tests
  are rejected even when their digest matches the evidence JSON. Duplicate JSON
  object keys in readiness evidence are invalid, so operators cannot rely on
  last-key-wins parser behavior. Unreadable or non-UTF-8 ABI-6 manifest and
  proof-evidence JSON files fail closed as structured read blockers. The ABI-7
  compact key evidence JSON must live beside `recursive-compact-len4.vk`,
  `recursive-compact-len4.pk`,
  `recursive-compact-key-artifacts.norito`,
  `recursive-compact-verifier-keys.norito`, and
  `recursive-compact-len4.record.norito`, keep the canonical
  `recursive-compact-key-evidence.json` filename, advertise LEN=4, IPA `k = 8`,
  `halo2/ipa`, `kagemusha-recursive-compact-v1`, `offline_kagemusha`, record
  version `1`, and record the exact canonical
  `iroha app zk kagemusha recursive-compact-key-artifacts` command with no
  aliases or appended shell commands. The rollup recomputes the compact key
  artifact SHA-256 values and byte sizes from adjacent non-empty regular files
  and hash-binds `recursive-compact-key-artifacts.log`, requiring exactly the
  canonical CLI summary line with `.vk`, `.pk`, package, verifier-key package,
  and `.record.norito` sizes that match the local artifact bytes,
  and rejects stale, future-dated, renamed, symlinked, hardlinked,
  size-mismatched, digest-mismatched, extra-field, or obvious plain-text
  placeholder compact key evidence. The compact key evidence helper applies the
  same placeholder-artifact and generator-log rejection before it emits evidence JSON.
  When ABI-7 key material is generated in a detached staging directory, run
  `python3 scripts/kagemusha_run_recursive_compact_keygen_staged.py --staged-artifact-dir <staged>/artifacts/kagemusha --exit-file <staged-exit-file>`
  first so the real keygen exit code and generator log are captured, then
  finalize it only after the staged process writes a zero exit marker:
  `python3 scripts/kagemusha_finalize_recursive_compact_key_staged_run.py --staged-artifact-dir <staged>/artifacts/kagemusha --exit-file <staged-exit-file> --artifact-dir artifacts/kagemusha --out artifacts/kagemusha/recursive-compact-key-evidence.json`.
  If path flags are omitted, both compact staged commands use the symlink-free
  resolution of `/tmp`, for example `/private/tmp` on macOS, so the default
  finalizer reads the default runner output without tripping the symlink-ancestor
  guard.
  The finalizer refuses missing or nonzero exit markers, symlinked or
  hardlinked staged artifacts, destination overwrites unless `--replace` is
  explicit, and generator-log size or digest drift before publishing the
  canonical `recursive-compact-key-evidence.json`. It also syncs the captured
  published artifact-directory identity after install, so directory swaps before
  final fsync fail closed.
  The
  device-lab scanner applies the same rule to `slot.json`,
  `attestation/result.json`, signed evidence, D2D handoff
  transcripts, and wallet-integrity transcripts before release summaries are
  accepted.
  Reserved-lineage proof evidence before the release cutoff or future-dated
  beyond the validator clock-skew allowance is also blocked, and
	  `generated_at_utc` must use canonical UTC
		  `YYYY-MM-DDTHH:MM:SSZ` form without whitespace or control-character
		  normalization. The lineage evidence helper rejects
		  noncanonical `--generated-at-utc` input, including `+00:00` offsets or
		  surrounding whitespace, instead of normalizing it, and rejects symlinked
			  output ancestors before creating missing `--out` parent directories or
			  reading release artifact and proof-log inputs. It also rejects dangling
			  symlink and unreadable-metadata output parents or leaves before following
			  or writing them, classifies `--out` parents with `lstat()` before any
			  `Path.is_dir()` preflight, binds output readback to the opened file
			  identity, rejects post-replace regular-file swaps as changed output, and
			  rechecks created output parents before
			  direct helper preflight returns. Input and output corridor resolver failures become structured
			  helper blockers instead of tracebacks.
		  Detached Reserved-lineage proof runs must be captured with
		  `python3 scripts/kagemusha_run_lineage_proof_staged.py --repo-root . --staged-artifact-dir <staged>/artifacts/kagemusha --exit-file <staged-exit-file> --elapsed-seconds-file <staged-elapsed-seconds-file>`
		  and promoted with
		  `python3 scripts/kagemusha_finalize_lineage_proof_staged_run.py --staged-artifact-dir <staged>/artifacts/kagemusha --exit-file <staged-exit-file> --elapsed-seconds-file <staged-elapsed-seconds-file> --artifact-dir artifacts/kagemusha --out artifacts/kagemusha/lineage-proof-evidence.json`;
		  if path flags are omitted, both commands share the symlink-free
		  resolution of `/tmp`, for example `/private/tmp` on macOS, so the
		  default finalizer reads the default runner output without tripping the
		  symlink-ancestor guard;
		  the finalizer requires a zero exit marker, reruns the proof-log and
		  artifact checks, and refuses destination overwrites unless `--replace`
		  is explicit. It also syncs the captured published artifact-directory
		  identity after install, so directory swaps before final fsync fail
		  closed. The staged runner first runs the canonical init and append
		  `iroha app zk kagemusha lineage-key-artifacts` commands from the
		  staged root, then preserves the real cargo exit code in the exit
		  marker and refuses to overwrite previous staged key artifacts, keygen
		  logs, proof logs, run reports, or elapsed-time files without
		  `--replace`. The keygen and proof children write combined
		  stdout/stderr directly to the temporary staged log files, which are
		  flushed, fsynced, and installed through a captured-parent identity
		  sync after child exit before any marker or report can become final
		  evidence.
		  The shared evidence builder
	  also rejects secret-looking artifact/proof-log paths and detached proof logs
	  before hashing artifacts or reading the proof log; direct artifact-dir,
	  proof-log corridor, and output-preflight helpers also reject secret-looking
	  artifact/proof-log/output paths and unreadable artifact-dir metadata before
	  resolving corridors, creating temporary directories, creating output parents,
	  or writing evidence JSON. The lower-level
	  lineage local-file validator rejects secret-looking evidence, artifact, or
	  proof-log paths and symlinked local-file ancestors before JSON parsing,
	  digest calculation, or proof-log reads, and both the rollup and helper
	  direct SHA-256 readers repeat that shape validation before returning
	  artifact digests.
	  Ready summaries expose only sanitized SHA-256 maps for accepted
	  Reserved-lineage artifacts and the captured proof log, not the local artifact
	  directory path.
	  The rollup rejects secret-looking `--repo-root`, `--device-lab-root`,
	  `--trusted-signer-public-key`, or `--summary-out` path arguments before
	  writing summaries so operator-local tokens are not persisted in evidence
		  packets, and the direct summary writer repeats the `--summary-out`
			  secret-path, dangling-symlink, and unreadable-metadata parent/leaf checks before
			  creating or writing the JSON file, classifies summary output parents
			  with `lstat()` before any `Path.is_dir()` preflight, then rechecks
			  created output parents and ancestors before writing, and binds
			  post-write readback to the opened summary file identity; final summary write failures become structured
			  `--summary-out` blockers. Scanner slot inventory also classifies expected
			  directories, `sha256sum.txt`, and recursive file-count entries with
			  `lstat()`, so summary presence/count fields do not follow symlinks or
			  hide unreadable metadata behind `Path.is_dir()` or `Path.is_file()`.
			  Automatic slot discovery classifies each device-lab root entry with
			  `lstat()`, preserves symlinked slot entries for fail-closed
			  `scan_slot(...)` rejection, and reports unreadable slot-entry metadata
			  without falling back to `Path.is_dir()`.
			  `--repo-root` resolver failures become the
		  structured `--repo-root could not be resolved` blocker before relative
		  lab or lineage evidence paths are expanded. Shared Android ancestor
		  validation also turns cwd metadata failures for relative helper inputs into
		  structured path blockers. Shared ancestor validation classifies each
		  ancestor with `lstat()` so symlink and metadata checks do not depend on
		  `Path.is_symlink()` or `Path.exists()` preflights. `--repo-root` must
		  also be an existing non-symlink directory with readable metadata and
		  symlink-free ancestors before checked-in readiness trust roots are read,
		  and the direct repo-root
		  validator and trust-root section checks repeat the secret-path preflight
		  before those trust roots are resolved.
	  Successful summaries carry a local device-lab root label instead of
  the absolute lab filesystem path, include a per-slot signed-evidence map with
  `signed_at_utc`, artifact SHA-256, and trusted signer public-key SHA-256 for
  validated slots, and the rollup does not print the absolute summary output path.
  As a second-line guard, any secret-looking string that reaches an
  Android scanner report is redacted before readiness summary serialization and
  blocks the rollup; symlinked output ancestors plus symlinked, hardlinked, or non-regular
  summary output aliases are rejected, and the summary writer fsyncs through an
  identity-bound output parent after atomic replacement, so the summary output path
  remains a local operator detail.
  The strict slot metadata lives in `slot.json` and must bind the device family,
  the family-specific minimum OS from the table, fingerprint, OS build id,
  app package name, app signing certificate, attestation challenge, offline
  wallet policy, attestation certificate chain path and SHA-256, release APK
  path and SHA-256, D2D payment transcript path and SHA-256, native bridge ABI
  version, wallet integrity transcript path and SHA-256, StrongBox/KeyMint
  status, one-use key rotation, physical device attestation, rollback rejection,
  ABI-6 recursive spend probe, ABI-7 recursive compact one-hop and multi-hop
  probe state (`abi7_recursive_compact_jni_probe = one_hop_verified` and
  `abi7_recursive_compact_prover_state = multi_hop_proof_composed`),
  raw test commands, signed evidence artifact path, and signed evidence artifact
  hash. Required `slot.json` string fields are exact: surrounding whitespace and
  non-printing control characters are rejected before path or digest validation.
  Production `slot.json` is a closed schema: unexpected fields are rejected
  before signed evidence can pass or be generated.
  The release APK path and SHA-256 plus native bridge ABI version are
  signature-bound production claims.
  The release APK path must point to bytes inside the slot and
  `offline_wallet_apk_sha256` must match those bytes. The APK artifact is capped
  separately at 64 MiB so real arm64 JNI proof builds fit while telemetry,
  transcript, report, and log artifacts retain the 16 MiB cap. The native bridge
  ABI version is pinned to the ABI-7 surface so the signed ABI-7 fail-closed
  probe cannot come from a stale bridge build.
  The hash must match the referenced artifact bytes inside the slot. The signed evidence artifact schema must repeat the slot identity fields, carry signer
  and signature metadata, including `signer_public_key_sha256` and
  `signature_payload_sha256`, include artifact digests for the required telemetry, attestation, queue, log, wallet integrity, and D2D handoff files, and verify against a trusted signer public key.
  The signed evidence artifact path must be the canonical
  `evidence/signed-evidence.json` path; renamed or copied signed evidence
  artifacts under `evidence/` are rejected.
  The D2D payment transcript must use schema
  `iroha.android.device_lab.kagemusha.d2d_payment.v1`, bind the same slot,
  device, app signing certificate, attestation challenge, offline wallet policy,
  and release APK digest as `slot.json`, report an offline `nfc_hce`, `qr`, or
  `nearby_offline` transport, keep the payload under the release payload ceiling,
  hash-bind the transport session, one-use key, and receiver ACK, and prove the
  payer wallet state, payee wallet state, and queue changed after receipt. Its
  `slot.json` path binding must stay under `handoff/`.
  It also carries `signed_at_utc` as raw canonical UTC
  `YYYY-MM-DDTHH:MM:SSZ` with no surrounding whitespace and repeats the
  StrongBox, one-use key, rollback, ABI probe, release APK, D2D transcript,
  native bridge ABI, physical-device, and raw command claims from `slot.json`
  so those production claims are signature-bound.
  `signer_public_key_sha256` is the SHA-256 of the trusted public key DER
  produced by `openssl pkey -pubin -pubout -outform DER`.
