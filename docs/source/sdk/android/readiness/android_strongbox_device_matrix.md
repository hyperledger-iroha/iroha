# Android StrongBox Offline Payments Device Matrix

Last updated: 2026-06-07

This matrix gates production readiness for Android offline-offline payment
flows. A device row is ready only after the lab attaches signed evidence for
StrongBox/KeyMint attestation, one-use key rotation, rollback rejection, ABI-6
recursive spend, and ABI-7 recursive compact-token availability probing.

| Device family | Minimum OS | StrongBox / KeyMint gate | Kagemusha recursive compact gate | Status |
| --- | --- | --- | --- | --- |
| Google Pixel 6 / 6a | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel 7 / 7 Pro | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel 8 / 8a / 8 Pro | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel Fold / Tablet | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Samsung Galaxy S23 | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Samsung Galaxy S24 | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |

Production release criteria:

- ABI 6 recursive spend JNI probes pass on every required device family.
- ABI 7 recursive compact-token JNI probes fail closed with the unavailable
  status until `kagemusha-recursive-compact-v1` is circuit-backed.
- ABI 7 recursive compact prover calls that reach the proof-composition
  reservation are reported as unavailable state, while empty or malformed local
  archives remain caller-input errors. Kotlin/JVM and Java Android validate
  recursive compact-token and record-backed recursive aggregation inputs as
  non-empty Norito archives before JNI dispatch.
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
  `telemetry/status.ndjson`, `attestation/result.json`,
  `queue/pending_queue.json`, and `logs/runtime.log`; signed evidence rejects
  refreshed manifests that omit any of those base artifacts. Those required
  base artifacts must be non-empty and no larger than 16 MiB each. Telemetry
  JSON must bind to the slot id, status NDJSON must include an `ok` status and
  no failure status, and `logs/runtime.log` must carry the Kagemusha device-lab
  completion marker without build/test failure markers.
  The device-lab root, operator-supplied root ancestors, slot parent
	  directories, slot path ancestors, slot directories, slot metadata, the
	  SHA-256 manifest, evidence directories, and artifact files must be ordinary
	  directories or regular files and must not be symlinks or hardlinks; the
	  scanner and signing helper reject linked or special-file slot artifacts
	  instead of following or hashing external aliases. The shared device-lab JSON
	  loader also rejects symlinked ancestor directories before parsing JSON, so
	  direct validation of slot metadata, attestation, transcript, or signed
	  evidence files cannot read through aliased directories.
	  Lower-level direct symlink, hardlink, and regular-file artifact validators
	  reject secret-looking slot paths before traversing, stat-ing, or
	  classifying slot artifacts.
	  Direct SHA-256 manifest parser and verifier helper calls reject
	  secret-looking slot paths, symlinked slot roots, and symlinked slot
	  ancestors before parsing `sha256sum.txt` or traversing slot artifacts.
	  Direct slot-file discovery returns no artifacts for secret-looking slot
	  paths, symlinked slot ancestors, missing roots, non-directory roots, or
	  symlinked slot roots before traversal, and skips symlinked artifact
	  directories instead of discovering files through them.
	  Direct manifest verification rejects entries under symlinked artifact
	  directories before reading or hashing bytes.
	  Direct attestation, D2D handoff, wallet-integrity, required-artifact,
	  signed-evidence, and production-metadata validator helper calls repeat the
	  same slot-path rejection before parsing artifacts, reading transcript
	  bindings, or hashing signed evidence.
- StrongBox/KeyMint attestation chains bind the app challenge and device
  security level expected by the offline wallet policy and must come from a
  physical device attestation, not an emulator or simulator run.
  The production attestation summary at `attestation/result.json` must report
  ok status, a StrongBox/KeyMint security level,
  `physical_device_attestation: true`, the attestation certificate chain path
  and SHA-256, and the same slot id, device fingerprint, OS build id, app
  package, app signing certificate, attestation challenge, and offline wallet
  policy hashes as `slot.json`.
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
  even if each individual slot is otherwise signed and hash-consistent.
  The signed raw command list must exactly match the canonical Android
  production device-lab command: it runs the release assembly steps
  `:client-android:assembleRelease` and
  `:offline-wallet-android:assembleRelease`, then `connectedAndroidTest` with
  the focused
  `org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest`
  plus `OfflineNoteTransferHandoff` harnesses. Marker-only commands, comments,
  or `echo` wrappers are rejected even if they contain those strings.
- Generate signed lab evidence from a completed slot with
  `python3 scripts/sign_android_device_lab_evidence.py --slot artifacts/android/device_lab/<slot-id> --private-key <runtime-only-lab-private-key.pem> --public-key <lab-public-key.pem> --signer-key-id <lab-signer-id>`.
  The helper writes the signed evidence artifact, refreshes the slot artifact
	  hash, and rewrites `sha256sum.txt`; private key paths are runtime inputs only
	  and are not written to slot metadata or JSON summaries. Runtime private-key
	  inputs and trusted lab public-key inputs must have symlink-free ancestors and
	  be regular, non-symlink, non-hardlinked files before OpenSSL is invoked.
	  Secret-looking key path strings are rejected before OpenSSL is invoked, so
	  operator-local tokens cannot be echoed by key parsing diagnostics. The
	  signer helper also rejects secret-looking `--slot`, `--output`, and
	  `--signer-key-id` runtime arguments before reading slot metadata.
	  Device-lab JSON summaries also carry a local root label instead of the
	  absolute lab path, and the validator does not print the absolute
	  `--json-out` path. Secret-looking `--root` and `--json-out` argument
	  strings are rejected before root discovery or summary writes, and the
	  direct root validator repeats the secret-path check before slot discovery.
	  The direct summary writer repeats the `--json-out` secret-path check before
	  writing JSON. Symlinked, hardlinked, or non-regular `--json-out` aliases are rejected before the
	  scanner writes a summary. Discovered slot directory names that contain
	  secret-looking material are rejected and redacted before artifact traversal
	  or summary serialization. The helper also rechecks the signed-evidence and
	  SHA-256 manifest output ancestors, parents, and leaves immediately before
	  writing, so symlinked, hardlinked, or non-regular output aliases are rejected
	  even if earlier slot validation has already passed.
	  Before parsing `slot.json`, the helper also preflights the slot directory,
	  slot path ancestors, metadata, manifest, and current slot artifacts for
	  symlink, hardlink, special-file aliases, and secret-looking artifact names,
	  so metadata-derived signing work cannot start from an aliased or
	  secret-bearing lab bundle.
	  Direct signer metadata-loader and manifest-rewrite helper calls also reject
	  secret-looking slot paths before metadata parsing, artifact traversal,
	  hashing, or manifest replacement can occur.
	  Low-level signer output writers reject secret-looking signed-evidence and
	  manifest paths before creating output parents or writing files.
	  Direct SHA-256 manifest rewrites run the same slot/artifact shape preflight
	  before hashing or replacing `sha256sum.txt`, so secret-looking artifact
	  names cannot be serialized into the manifest.
	  The shared JSON loader rejects secret-looking direct file paths and
	  symlinked ancestors before parsing metadata, attestation, handoff,
	  wallet-integrity, or signed-evidence JSON.
- Production lab bundles must pass
  `python3 scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab --require-slot --require-kagemusha-production-evidence --require-kagemusha-standard-matrix --trusted-signer-public-key <lab-public-key.pem>`.
  When selecting explicit slots, each `--slot` value must be a single safe slot
  directory name under the lab root, not a filesystem path.
  Release evidence rollups should then run
  `python3 scripts/kagemusha_production_readiness.py --device-lab-root artifacts/android/device_lab --trusted-signer-public-key <lab-public-key.pem> --min-signed-at-utc 2026-06-06T00:00:00Z --max-signed-at-future-skew-seconds 300 --max-lineage-proof-evidence-future-skew-seconds 300 --summary-out dist/kagemusha-production-readiness.json`,
  which combines the ABI-6 Reserved-lineage manifest, ABI-7 fail-closed
	  contract, Reserved-lineage proof evidence, signed Android evidence, and standard device-family coverage into a
	  strict ready/blocked JSON summary. The checked-in ABI-6 manifest must be a
	  regular non-symlink, non-hardlinked file with symlink-free ancestors before
	  its release contract is trusted. The checked-in ABI-7 fail-closed and
	  Reserved-lineage release-tooling marker source files must also be ordinary
	  non-symlink, non-hardlinked files before their marker text can satisfy
	  readiness, and direct helper calls reject secret-looking trust-root file
	  paths before metadata checks or JSON/source parsing. Evidence signed before the release cutoff
  or future-dated beyond the release validator clock-skew allowance remains
  blocked even when its signature and hashes are otherwise valid. Freshness checks
  use the scanner-validated signed-evidence timestamp from the slot report rather
  than re-opening `slot.json` or `evidence/signed-evidence.json` in the rollup.
  The Reserved-lineage proof evidence JSON must live beside the
  `artifacts/kagemusha` `.norito`, `.record.norito`, `.vk`, and `.pk` files it
  declares, plus `record-archive-proof.log`; it must keep the canonical
  `lineage-proof-evidence.json` filename because renamed or copied evidence
  JSON files are rejected, and symlinked evidence files or symlinked evidence
  ancestors are also blocked. The rollup recomputes their SHA-256 digests from
  local bytes, requires the adjacent artifacts and proof log to be regular
  non-symlink, non-hardlinked files, and re-checks the proof log's passing cargo
  result as one expected `test ... ok` line plus one one-test cargo result, and
  it re-checks the exact production `cargo test -p iroha_core` command, with no
  appended shell commands. Marker-stuffed proof logs with extra passing tests
  are rejected even when their digest matches the evidence JSON. Duplicate JSON
  object keys in readiness evidence are invalid, so operators cannot rely on
  last-key-wins parser behavior. The device-lab scanner applies the same rule
  to `slot.json`, `attestation/result.json`, signed evidence, D2D handoff
  transcripts, and wallet-integrity transcripts before release summaries are
  accepted.
  Reserved-lineage proof evidence before the release cutoff or future-dated
  beyond the validator clock-skew allowance is also blocked, and
	  `generated_at_utc` must use canonical UTC
	  `YYYY-MM-DDTHH:MM:SSZ` form. The lineage evidence helper rejects
	  noncanonical `--generated-at-utc` input, including `+00:00` offsets or
	  surrounding whitespace, instead of normalizing it, and rejects symlinked
	  output ancestors before creating missing `--out` parent directories or
	  reading release artifact and proof-log inputs. The shared evidence builder
	  also rejects secret-looking artifact/proof-log paths and detached proof logs
	  before hashing artifacts or reading the proof log; direct artifact-dir,
	  proof-log corridor, and output-preflight helpers also reject secret-looking
	  artifact, proof-log, and output paths before resolving corridors, creating
	  temporary directories, creating output parents, or writing evidence JSON. The lower-level
	  lineage local-file validator rejects secret-looking evidence, artifact, or
	  proof-log paths and symlinked local-file ancestors before JSON parsing,
	  digest calculation, or proof-log reads.
	  Ready summaries expose only sanitized SHA-256 maps for accepted
	  Reserved-lineage artifacts and the captured proof log, not the local artifact
	  directory path.
	  The rollup rejects secret-looking `--repo-root`, `--device-lab-root`,
	  `--trusted-signer-public-key`, or `--summary-out` path arguments before
	  writing summaries so operator-local tokens are not persisted in evidence
	  packets, and the direct summary writer repeats the `--summary-out`
	  secret-path check before creating or writing the JSON file. `--repo-root` must also be an existing non-symlink directory with
	  symlink-free ancestors before checked-in readiness trust roots are read, and
	  the direct repo-root validator and trust-root section checks repeat the
	  secret-path preflight before those trust roots are resolved.
	  Successful summaries carry a local device-lab root label instead of
  the absolute lab filesystem path, include a per-slot signed-evidence map with
  `signed_at_utc`, artifact SHA-256, and trusted signer public-key SHA-256 for
  validated slots, and the rollup does not print the absolute summary output
  path. As a second-line guard, any secret-looking string that reaches an
  Android scanner report is redacted before readiness summary serialization and
  blocks the rollup; symlinked output ancestors plus symlinked, hardlinked, or non-regular
  summary output aliases are rejected, so the summary output path remains a
  local operator detail.
  The strict slot metadata lives in `slot.json` and must bind the device family,
  the family-specific minimum OS from the table, fingerprint, OS build id,
  app package name, app signing certificate, attestation challenge, offline
  wallet policy, attestation certificate chain path and SHA-256, release APK
  path and SHA-256, D2D payment transcript path and SHA-256, native bridge ABI
  version, wallet integrity transcript path and SHA-256, StrongBox/KeyMint
  status, one-use key rotation, physical device attestation, rollback rejection,
  ABI-6 recursive spend probe, ABI-7 recursive compact unavailable probe, raw
  test commands, signed evidence artifact path, and signed evidence artifact
  hash.
  Production `slot.json` is a closed schema: unexpected fields are rejected
  before signed evidence can pass or be generated.
  The release APK path and SHA-256 plus native bridge ABI version are
  signature-bound production claims.
  The release APK path must point to bytes inside the slot and
  `offline_wallet_apk_sha256` must match those bytes. The native bridge ABI
  version is pinned to the ABI-7 surface so the signed ABI-7 fail-closed probe
  cannot come from a stale bridge build.
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
