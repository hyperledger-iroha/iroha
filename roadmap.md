# Roadmap

Last updated: 2026-06-13

This roadmap is the public, high-level view of current Hyperledger Iroha work.
The detailed engineering backlog lives in
[`docs/source/engineering_backlog.md`](./docs/source/engineering_backlog.md),
and completed history lives in [`status.md`](./status.md).

## Release and Stabilization

**Status:** active.

- Move the shared Iroha 2 / Iroha 3 codebase toward a broadly consumable
  release with clear release notes, SDK parity, and operator documentation.
- ZK asset light-client readiness now has a Torii `POST /v1/zk/merkle-path`
  endpoint for current confidential-v2 commitment inclusion paths, and the
  Kotlin/JVM plus Android Java Torii Merkle providers call it directly. Keep
  local providers limited to audited caller-supplied frontier material.
- Confidential-v2 SDK note derivation and encrypted note payload handling now
  exist for Kotlin/JVM and Android Java, with Rust-vector parity for owner tags,
  note commitments, nullifiers, asset tags, and chain tags, Rust-fixture parity
  for the `ConfidentialEncryptedPayload` wire envelope, low-order X25519
  public-key rejection parity, canonical ciphertext-length rejection parity,
  and a shared
  deterministic X25519/HKDF-SHA256/XChaCha20-Poly1305 plaintext vector. Keep
  the higher-level wallet flows pinned to this contract when wiring shield-note
  recovery into production clients.
- Kagemusha SDK parity must keep ABI-7 compact projection verifier surfaces
  aligned across package roots and native hosts. Python now exposes both the
  optional-height verifier and the explicit
  `kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height(...)`
  root helper. The JS SDK validates compact projection `blockHeight` values
  before native probing or dispatch, accepting safe non-negative numbers and
  bounded `u64` bigints only, and the SDK parity guard must continue pinning
  those surfaces.
- Kagemusha C# SDK validation remains a Windows-machine follow-up because this
  macOS host does not have `dotnet` installed. On Windows, install or select a
  .NET 8 SDK, run the standalone C# Kagemusha guard
  `ci/check_kagemusha_recursive_spend_csharp_sdk.sh` or its direct
  `dotnet test` equivalent, and preserve the selected `dotnet --version`
  evidence in the output. The focused pass should cover
  `csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaRecursiveSpendNativeTests.cs`,
  `PrivacyNativeTests.cs`, and `TransactionBuilderTests.cs`, with native bridge
  loading and P/Invoke symbol probing enabled for the ABI-6 recursive spend and
  ABI-7 compact-token, recursive aggregation, recursive compact
  verifier/projection, and instruction transaction-builder surfaces. The
  standalone runner now builds `connect_norito_bridge`, resolves the
  platform-specific native library name, fails if the freshly built artifact is
  missing, prints the selected native bridge path, and prepends that directory
  to the macOS, Linux, and Windows loader paths before invoking `dotnet test`.
  The
  Windows pass must also pin the C# negative controls for malformed Norito
  input/output headers, caller archive-copy immutability, verifier-unavailable
  status mapping, transaction-builder schema and wire-name drift, and
  package/evidence parity. After the Windows run passes, update `status.md`
  with the C# SDK evidence and rerun the Kagemusha SDK parity or production
  readiness guards needed to clear the C# row.
  Windows-machine TODOs:
  - Select a .NET 8 SDK and capture `dotnet --version` in the run log.
  - Capture the Windows `dotnet --info` output, including RID/architecture, so
    the native C# pass is tied to the host that loaded the bridge.
  - Run `ci/check_kagemusha_recursive_spend_csharp_sdk.sh`, or the equivalent
    direct `dotnet test` command with the same native bridge path setup.
  - Confirm the Windows runner log prints `connect_norito_bridge native bridge:`
    and `connect_norito_bridge native bridge sha256:` for the freshly built
    `connect_norito_bridge.dll` before the P/Invoke tests start.
  - Confirm the pass includes `KagemushaRecursiveSpendNativeTests`,
    `PrivacyNativeTests`, and `TransactionBuilderTests`.
  - Confirm `KagemushaRecursiveSpendNativeTests` exercises
    `KagemushaOverlongCompactLength`,
    `KagemushaOversizedTerminalCompactLength`,
    `KagemushaHugeCanonicalCompactLength`,
    `overlongVersionLengthArchive`, `oversizedTerminalCompactLengthArchive`,
    `hugeCanonicalCompactLengthArchive`, `overlongCircuitStringArchive`, and
    `invalidUtf8CircuitArchive` so the C# parser rejects non-canonical,
    address-space oversized, u64-overflowing compact lengths and invalid UTF-8
    lineage archive circuit fields on Windows.
  - Add the Windows C# negative that a whitespace-padded `CID1` circuit id in
    the lineage verifier key rejects as `lineage_verifier_key` before native
    bridge dispatch, even when the proving-key archive commits to that padded
    verifier key.
  - Confirm whether the C# SDK has or adds an Offline Note V1/V2 canonical
    model/decoder surface; if so, mirror the Swift/Kotlin/Android exact-domain
    negatives for key-certificate payload, issued-claim, redeem-public-inputs,
    and audit-public-inputs domains, including padded-domain rejection.
  - Confirm whether the C# SDK has or adds an Offline Note receipt ACK
    model/codec surface; if so, mirror the Swift/Kotlin/Android exactness
    negatives for `chain_id`, `payment_request_id`, `recipient_account_id`,
    and positive `accepted_at_ms` validation at construction and decode
    boundaries.
  - Confirm whether the C# SDK has or adds an Offline Note wallet-note
    persistence surface; if so, mirror the Swift/Kotlin/Android exactness
    negatives for persisted `chain_id`, `account_id`, and optional
    `spent_payment_request_id`, rejecting padded or blank values instead of
    normalizing them across account-scope or replay-prevention boundaries.
  - Mirror the non-C# identifier receipt hardening on Windows so C# canonical
    attestation builders and Torii JSON receipt parsing reject padded or
    mixed-case attestation `kind` tags before selecting signed/proof behavior.
  - Mirror the non-C# identifier receipt proof-attestation hardening on Windows
    so C# canonical attestation builders and Torii JSON receipt parsing reject
    padded `proof_b64` before base64 decoding or selecting proof behavior.
  - Mirror the non-C# identifier receipt signature hardening on Windows so C#
    canonical payload/attestation builders, Torii JSON receipt parsing, and
    verifier inputs reject padded `payload.opening.signature` and signed
    attestation `signature` before hex decoding or receipt verification.
  - Mirror the non-C# identifier receipt policy-id hardening on Windows so C#
    canonical payload builders and Torii JSON receipt parsing reject padded
    `payload.policy_id` text and padded `kind`/`rule` components before
    canonical receipt bytes are encoded or verified.
  - Mirror the non-C# identifier receipt program-id hardening on Windows so C#
    canonical payload builders and Torii JSON receipt parsing reject padded
    `payload.execution.program_id` and `payload.opening.payload.program_id`
    before canonical receipt bytes are encoded or verified.
  - Mirror the non-C# identifier receipt account-id hardening on Windows so C#
    canonical payload builders and Torii JSON receipt parsing reject padded
    `payload.account_id` before canonical receipt bytes are encoded or verified.
  - Mirror the non-C# identifier receipt hash-field hardening on Windows so C#
    canonical payload builders and Torii JSON receipt parsing reject padded
    `payload.opaque_id`, `payload.receipt_hash`, `payload.uaid`, execution
    digest fields, and opening payload digest fields before canonical receipt
    bytes are encoded or verified.
  - Mirror the non-C# identifier receipt timestamp hardening on Windows so C#
    canonical payload builders and Torii JSON receipt parsing reject padded
    numeric-string receipt times for `payload.execution.executed_at_ms`,
    `payload.execution.expires_at_ms`, `payload.opening.payload.opened_at_ms`,
    and `payload.opening.payload.expires_at_ms`, and reject negative receipt
    times before canonical u64 receipt bytes are encoded or verified.
  - Tighten the C# `TransactionBuilder`/`TransactionEncodingContext` Windows
    pass so chain ids, authority/account ids, asset/domain ids, metadata keys,
    optional memo/fee-sponsor strings, and label-like fields reject surrounding
    whitespace before Norito transaction bytes are encoded or signed, instead
    of normalizing with `Trim()`. Add focused `TransactionBuilderTests`
    negatives for padded constructor, instruction, metadata, and encoding
    fields after selecting the Windows .NET SDK.
  - Tighten the C# canonical request auth Windows pass so
    `CanonicalRequestCredentials`, `CanonicalRequest.BuildHeaders`, and
    `CanonicalRequestHeaders` reject padded or explicitly blank account ids,
    nonces, and signature/header fields before signing or header emission,
    instead of accepting `ArgumentException.ThrowIfNullOrWhiteSpace` or
    generating a fresh nonce for caller-supplied blank values. Add focused
    `CanonicalRequestTests` and `ToriiClientTests` negatives for padded account
    ids/nonces after selecting the Windows .NET SDK.
  - Confirm the Windows C# SDK lane includes `PrivacyNativeTests`, whose
    cross-platform .NET 8 pass now covers the public VeRange V1 aliases
    `BuildVeRangeProofV1`, `buildVeRangeProofV1`, `VerifyVeRangeProofV1`, and
    `verifyVeRangeProofV1` alongside the generic `BuildProofV1` and
    `VerifyProofV1` archive paths.
  - Re-run `ci/check_kagemusha_recursive_spend_sdk_parity.sh` after recording
    the Windows evidence so C# SDK parity status can be cleared explicitly.
- Kagemusha JVM SDK validation must keep the focused runner aligned with the
  parity inventory: Kotlin/JVM runs recursive spend, canonical request auth,
  instruction archive, Offline Cash lifecycle, Offline Note, Offline Note V2,
  and privacy native bridge tests, while the Android Java harness runs
  recursive spend, canonical request auth, Offline Cash lifecycle, Offline Note
  V2, Offline Note, privacy native bridge, and transaction-builder archive
  tests. Kotlin/JVM and Android Java privacy capability APIs must continue
  deriving bridge readiness from the native Norito capability archive when the
  bridge is loaded, while malformed, duplicate, incomplete, or absent evidence
  keeps the SDK capability surface fail-closed. Privacy proof dispatch must keep
  the `privacy-production-enabled` feature opt-in, preserve default
  production-disabled serialized results, and retain focused coverage for real
  confidential-transfer-v2/unshield proving, verification, and checked unshield
  input-sum overflow rejection. The focused runner now also executes the
  Kotlin/JVM and Android Java
  account-literal, canonical request auth, and Offline Cash issuer-key
  exactness tests, plus Torii event-stream verifier-filter, signing-algorithm,
  verifier-key backend/instruction, and verifier record-description/status
  exactness tests, so padded I105 account IDs, padded request auth fields,
  malformed cached issuer keys, padded Torii event verifier filters, padded
  signing algorithm labels, padded verifier record fields, and padded verifier
  backend/status labels stay covered by the same mobile SDK gate.
  Kotlin/JVM and Android Java recursive-spend request codecs must keep
  init/append/verify/redeem archive schemas, compact request payload layouts,
  raw embedded archive payloads, Norito `Option` child-length framing, Rust
  `[u8; N]` fixed-array byte layout, bundle/result decoders, lineage gap checks,
  and nonnegative block-height guards pinned in the focused JVM SDK runner and
  parity inventory. Kotlin/JVM and Android Java now also expose typed
  `RegisterZkAsset`, `Shield`, and `Unshield` builders plus native
  signed-transaction wrappers that preserve private change outputs and return
  canonical versioned transaction bytes with native hashes. The JVM roots/Merkle
  slice now exposes typed `/v1/zk/roots` clients and local zk_assets path
  providers for audited frontier material; remaining SDK gaps are
  confidential-v2 note derivation/decryption after canonical derivation
  confirmation, Torii-backed path acquisition when the node endpoint exists,
  and audited end-to-end localnet coverage.
- Kagemusha JavaScript SDK validation must keep the focused Node 20 runner
  aligned with the parity inventory by executing the Kagemusha recursive spend,
  account-address exactness, Offline Cash issuer-key configuration snapshot,
  canonical request auth exactness, Torii event-filter verifier/proof
  exactness, verifier-key selector exactness, identifier-receipt adversarial
  and shared-vector exactness, package/browser, privacy native bridge, and
  transaction-builder archive test names together.
- Kagemusha Swift SDK validation must keep the macOS parse runner aligned with
  the parity inventory by parsing every Kagemusha/Offline Note source and test
  file tracked for Swift, including canonical request auth helpers, recursive
  compact, instruction transaction encoder, privacy native bridge coverage,
  Offline Note issuer-key parsing, text-transfer contracts, receipt challenges,
  wallet/redeem/QR helpers, signing-algorithm discriminants,
  verifier-backend labels, Torii verifier-key request/event validation, and
  Offline Cash/Kagemusha ABI-7 support files. The
  payload-bench workflow path inventory and JavaScript parity meta-test must
  stay in lockstep with that expanded Swift parse surface.
- Kagemusha Python SDK validation must keep the focused Python 3.11 runner on
  the Kagemusha, privacy catalog, crypto algorithm, Offline Cash, and
  address-format pytest files because those files cover the Python transaction
  helpers, native archive guards, Offline Cash issuer-key exactness,
  account-address exactness, Torii canonical request auth exactness, Torii
  identifier-receipt payload/attestation exactness, and package export surfaces
  used by the SDK parity inventory. The workflow path inventory must also
  watch the Python privacy catalog, Offline Cash, address, crypto helper, Torii
  canonical request, and Torii identifier-receipt source/test files so changes
  to those runner-covered surfaces trigger the focused SDK pass. Python Torii
  identifier receipt helpers must keep
  `encode_identifier_resolution_receipt_payload`,
  `encode_identifier_resolution_receipt_attestation`, and
  `verify_identifier_resolution_receipt` aligned with the shared fixture; proof
  attestations remain external-verifier-only and signed receipts use Iroha
  Blake2b prehash plus Ed25519 verification through `iroha_python.crypto`.
  Python receipt tests must keep adversarial coverage for padded RAM-LFE
  backend/mode tags, padded opening signatures, non-exact attestation kind tags,
  padded signed-attestation signatures, padded receipt `policy_id` text and
  `kind`/`rule` components, padded execution/opening `program_id` values,
  padded `account_id` values, padded receipt hash-like fields
  (`payload.opaque_id`, `payload.receipt_hash`, `payload.uaid`, execution
  digests, and opening payload digests), padded numeric-string receipt times,
  negative receipt times for u64 fields, padded proof-backend tags, padded
  proof-base64 text, malformed proof base64, and signed-vs-proof attestation
  confusion.
  JavaScript, Swift, Kotlin/JVM, and Android Java must keep matching
  non-exact-kind, non-exact-signature, non-exact-policy-id,
  non-exact-program-id, non-exact-account-id, non-exact-hash-field,
  non-exact-timestamp, non-exact-proof-base64, and malformed-proof-base64
  receipt coverage so receipt attestations cannot be accepted from Torii JSON or
  canonical builders after whitespace/case normalization or with
  padded/non-base64 proof bytes.
  The SDK parity guard must keep the
  `--negative-control-identifier-receipt-proof-base64-guard`,
  `--negative-control-identifier-receipt-proof-base64-exactness-guard`, and
  `--negative-control-identifier-receipt-kind-exactness-guard`, and
  `--negative-control-identifier-receipt-signature-exactness-guard`, and
  `--negative-control-identifier-receipt-policy-id-exactness-guard`, and
  `--negative-control-identifier-receipt-policy-summary-id-exactness-guard`, and
  `--negative-control-identifier-receipt-program-id-exactness-guard`, and
  `--negative-control-identifier-receipt-account-id-exactness-guard`, and
  `--negative-control-identifier-receipt-hash-exactness-guard`, and
  `--negative-control-identifier-receipt-timestamp-exactness-guard`, and
  `--negative-control-identifier-receipt-timestamp-u64-guard`, and
  `--negative-control-identifier-receipt-resolver-key-exactness-guard` drift checks wired
  into the workflow and JavaScript meta-test so that validation cannot be
  removed silently.
  C# Windows TODO: mirror resolver public-key exactness for identifier receipt
  policy summaries and verifier inputs, including padded-key negative vectors.
  C# Windows TODO: mirror policy-summary `policy_id` exactness for identifier
  receipt verification, including padded policy-id negative vectors.
  C# Windows TODO: mirror signed-attestation `signature` verifier-input
  exactness for identifier receipt verification, including padded signature
  negative vectors.
  Python crypto algorithm labels must remain exact at the public SDK
  boundary: aliases can normalize, but empty or padded labels must fail before
  key generation, key loading, multihash, sign, verify, or key-pair construction
  reaches native code. Production verifier backend labels must keep the same
  exactness across Python, JavaScript, Kotlin/JVM, Android Java, Swift, and C#
  Torii/instruction-builder surfaces: padded labels fail with a
  surrounding-whitespace error before unsupported backend classification,
  verifier-key id construction, event-filter dispatch, or request dispatch.
  JavaScript, Python, Kotlin/JVM, and Android Java Kagemusha validation must
  also keep identifier-receipt canonical payload tests in the focused runners,
  and Kotlin/JVM plus Android Java must keep the claim-identifier wire encoder
  tests there too, so RAM-LFE identifier receipts used by SDK instruction
  builders keep exact backend, verification-mode, proof-backend, receipt
  payload, attestation framing, signature, and shared-vector semantics.
  Account-address/signing algorithm selectors must also stay exact across Python, JavaScript,
  Kotlin/JVM, Android Java, Swift, and C#: omitted default parameters can still
  select Ed25519, but explicit blank or padded labels must fail before alias
  normalization, public-key address construction, key-generation selection, or
  native bridge dispatch. Offline-cash configuration snapshots must keep issuer
  public keys exact across SDKs as non-empty printable ASCII with no whitespace
  or non-ASCII normalization before offline exchange, and Swift's standalone
  issuer-key parser must reject padded base64/base64url text before decoding.
  Swift counterparty offline proof verification must also dispatch only exact
  `ios`/`android` platform labels so padded values cannot enter challenge or
  binding verification paths. Kotlin/JVM and Android Java canonical I105
  account-id literal helpers now reject surrounding whitespace before
  transaction, Connect, and instruction-builder boundaries so padded account
  strings cannot be normalized into signed payloads.
- Kagemusha Android production readiness now has host-side verifier-report
  rendering, a signed-slot assembler, a physical-device raw artifact exporter,
  a strict host puller for those raw slots, and a dedicated
  `:offline-wallet-lab-app` target whose release APK is hash-bound by the
  physical exporter. The signed-slot assembler binds every copied source
  artifact to symlink-free ancestors and the opened file identity, uses a
  separate 64 MiB cap for the JNI-bearing offline wallet APK while retaining
  16 MiB caps for smaller evidence artifacts, and rejects source-directory
  aliases or post-preflight source swaps before signed slot installation. It
  now syncs copied-artifact parents and reads staged copies back through
  opened-file identity bindings before manifesting. Its normalized attestation
  and slot metadata JSON writes use fsynced temporary files, identity-bound
  parent fsync, and opened-file readback before manifesting. It publishes the completed stage through directory file
  descriptors pinned to the captured device-lab root, temp-parent, and
  staged-slot identities, and cleanup checks the captured temp-parent identity
  before removing staging directories, reporting removal failures while
  preserving identity-swapped staging directories, so path swaps before final
  fsync or cleanup fail closed. Fresh
  raw exports now include `attestation/harness-result.json`, and the raw puller
  requires that harness result to match the slot challenge before the host
  verifier report and signed slot can be assembled. The raw puller also
  requires both the `run-as cat` latest-slot query and the tarred
  `latest-slot.txt` to be exactly the selected slot id plus a trailing newline,
  rejecting whitespace-normalized matches, reports tar directory collisions
  as structured blockers instead of tracebacks, moves top-level raw artifacts
  through opened stage/final directory descriptors, and revalidates the
  captured temporary extraction directory identity before cleanup while
  reporting removal failures before latest-slot or summary publication. It
  accepts only the uncompressed `tar -cf -` stream emitted by the Android
  exporter, so compressed archive streams fail before extraction, and rejects
  noncanonical tar member spellings such as `./` or repeated separators before
  they can normalize into accepted evidence paths. It also rejects
  control-character output-root, summary-output, raw-slot, and raw artifact
  path strings, plus parent-segment and backslash-bearing output-root,
  summary-output, and raw-slot aliases, before ADB access, metadata reads,
  directory creation, or raw-byte error reporting; rejects unreviewed extra files or directories under
  the raw slot; redacts control-character or secret-looking unexpected
  top-level install-source names before reporting install failures; requires both `slot` and
  `slot_id` in raw `attestation/result.json` to match the selected slot id, and
  requires canonical lowercase SHA-256 chain/challenge digests matching the
  pulled StrongBox certificate-chain and challenge bytes. Raw
  `attestation/result.json` is closed-schema, and
  `attestation/challenge.hex` must be lowercase hexadecimal with exactly one
  trailing newline so challenge bytes are never accepted through whitespace or
  case normalization. Raw identity strings and raw tar-member paths must be
  trim-stable and free of control characters before evidence assembly or tar
  path normalization, SDK/policy digests must be canonical lowercase SHA-256
  hex, and raw security levels must remain exact `STRONGBOX`. Raw harness StrongBox levels now explicitly reject
  surrounding whitespace, control characters, and secret-looking material before
  exact label membership is checked. Queue, telemetry, D2D handoff, and wallet integrity
  JSON artifacts are now parsed as slot-bound strict JSON with exact slot-id
  bindings and exact `kagemusha-device-lab` telemetry suite identity before
  assembly; D2D must remain offline-offline and wallet
  integrity must prove key rotation plus rollback rejection, with transcript
  string bindings matched without whitespace or control-character
  normalization. Raw status NDJSON
  must use LF line endings with a trailing newline, reject nonblank lines with
  surrounding whitespace, accept only exact `ok` status strings without
  surrounding whitespace or control characters, and must require exact
  non-empty slot bindings while rejecting non-string, whitespace-normalized,
  control-character, or mismatched slot bindings. Slot metadata ABI probe states
  must also be exact lowercase strings
  without surrounding whitespace or control characters, while runtime logs must
  contain the completion marker
  without build/test/panic/traceback/fatal failure markers. Raw puller ADB
  executable, serial, run-as package, and device-root inputs must be exact
  non-empty strings with no surrounding whitespace, control characters, or
  secret-looking material before any device command is built. Raw pull summaries
  now reject non-finite or oversized JSON before temp-file creation and fsync
  summary bytes before atomic replacement, then verify summary readback through
  opened-file identity checks that reject symlinks, hardlinks, and path swaps
  before the identity-bound parent directory fsync. Summary artifact digests
  must cover every required raw artifact and are collected through separate
  opened-file identity checks that reject symlinks, hardlinks, and file swaps
  after raw slot validation. Raw slot installation now rechecks the final
  destination, creates the installed slot directory exclusively with owner-only
  permissions, moves only expected top-level artifact directories, binds the
  created slot-directory identity through each parent-fd slot-entry stat, move,
  and slot fsync, binds the output-root identity through the parent fsync, and
  removes partial installs through the identity-bound output-root file
  descriptor only when the destination entry still names the directory created
  by the puller, reporting cleanup removal failures with the install error. The
  host `latest-slot.txt` writer now follows the same
  output-readback contract, with byte fsync, atomic replace, opened-file
  identity readback that rejects symlinks, hardlinks, and path swaps, and an
  identity-bound output-root fsync. The raw puller's host `latest-slot.txt` and
  summary writers now also report identity-bound temp cleanup failures and
  refuse to unlink a temp output whose file identity changed before cleanup.
  Explicit scanner `--slot` values now fail closed unless they are already
  exact safe single-directory names without whitespace, so whitespace-normalized
  slot selection cannot choose a production evidence bundle.
  Signed slots now preserve the same `attestation/harness-result.json`, include
  it in signed
  `artifact_digests`, reject legacy signed evidence that drops the raw
  StrongBox harness output, and require preserved harness aliases, StrongBox
  levels, and challenge hex to be exact canonical strings during both assembly
  and scan. Signed slot metadata `keymint_security_level` must also remain an
  exact accepted StrongBox label instead of passing through case normalization.
  Signed slot metadata `abi6_recursive_spend_jni_probe` must now be exact
  `passed`; an `ok` alias is rejected even when signed evidence repeats it.
  The verifier `attestation/report.json` must include all three StrongBox level
  bindings (`keymint_security_level`, `attestation_security_level`, and
  `keymaster_security_level`) during both scan and signing-helper validation, so
  incomplete verifier reports cannot pass by omitting downgraded levels. The
  scanner and signed-slot assembler now also require verifier report
  app-package, status, and level bindings to match `attestation/result.json`
  exactly before acceptance, and scanner validation binds `attestation/result.json`
  `keymint_security_level` back to `slot.json` exactly, preventing accepted
  app-package substitutions, non-`ok` status aliases, or StrongBox alias
  spellings from hiding a cross-artifact splice. The signed-slot assembler also
  rejects unexpected attestation result, report, verifier, D2D transcript, or
  wallet-integrity transcript fields, report schema/verifier drift, plus D2D
  and wallet transcript schema-id drift, before publishing source artifacts. It
  now also runs the scanner D2D and wallet transcript semantic validators on
  staged copies before publish, so queue splices, wallet state non-rotation,
  and other scanner-only transcript failures cannot be staged into unsigned
  production slots. Required telemetry, status NDJSON, queue, attestation, and
  runtime-log artifact shape checks now also run on staged assembler output
  before publish, so failed status records, missing runtime completion markers,
  malformed telemetry, noncanonical telemetry identity strings, unexpected
  telemetry, status-event, or pending queue fields, non-`ok` status events,
  non-empty post-handoff pending transactions, or malformed pending queue JSON
  cannot be installed as unsigned production slots. The raw Android puller
  applies the same telemetry field allowlist, status-event field and value
  allowlists, telemetry identity exactness, telemetry app-package binding,
  pending queue field allowlist, and queue empty-after-handoff check before raw
  artifacts can be promoted into a signed slot.
  The slot assembler also requires attached-device ADB `getprop`
  identity reads to be exact one-LF values whose contents do not need trimming
  before metadata binding. The standalone Android scanner also rejects copied
  Kagemusha matrix rows by reporting hash-only duplicate device fingerprints or
  attestation challenges across otherwise-valid slots, and the production
  readiness rollup mirrors that non-secret duplicate inventory with
  release-bundle schema validation, verify-existing validation, exact standard
  matrix and signer-pin manifest checks, drift checks, and an identity-bound
  scanner JSON summary parent sync. The Android attestation report writer now
  rejects whitespace-normalized or uppercase attestation challenge hex, including
  noncanonical `--expected-challenge-hex`, control-character-bearing harness
  aliases, levels, challenges, expected challenge hex, slot id, device
  fingerprint, OS build, app package, verifier, and
  `--attestation-certificate-chain-path` values, rejects control-character
  local certificate-chain source paths plus parent-segment and backslash source
  aliases before ancestor validation or metadata reads, plus
  whitespace-normalized identity fields, StrongBox/KeyMint level
  labels, PEM chain-length mismatches, and unsafe chain paths before writing
  `attestation/report.json`. The report writer and signed-evidence helper also
  identity-bind their
  post-replace output-parent syncs before accepting local JSON or manifest
  outputs, and the signed-slot assembler now identity-binds local JSON temp
  cleanup before accepting slot metadata outputs. The lineage plus
  compact-key staged runners apply the same gate to their child-log installs,
  marker, and metadata outputs before readback. The lineage and compact-key
  staged runners and finalizers also reject parent-segment and backslash
  aliases in explicit staged path flags before ancestor validation or metadata
  reads. Those staged runners also
  identity-bind resume/replace cleanup and temporary log/output cleanup before
  unlinking stale staged paths. Lineage and compact-key finalizers now require
  successful staged exit markers to be the exact `0\n` line, and compact-key
  `--resume-keygen` reruns instead of reusing a padded zero marker; finalizer
  and resume diagnostics redact control-character or secret-looking marker
  values, and staged run-report JSON duplicate or unexpected field diagnostics
  redact unsafe keys, including nested lineage key-log profile and entry-field
  names. Readiness evidence unexpected-field diagnostics now redact both
  control-character and secret-looking field names before blocker serialization.
  Release-bundle summary and manifest verification now rejects control-character
  strings anywhere inside those JSON roots, matching the existing global
  secret-material gate.
  The lineage finalizer also requires the staged
  elapsed-seconds sidecar to be the runner's exact six-fractional-digit
  positive decimal line before it can bind the run report. The lineage and
  compact-key evidence helpers
  now identity-bind validation scratch-file cleanup under `--artifact-dir`
  before unlinking those temp files. The staged finalizers also
  identity-bind the published artifact directory before their final fsync and
  revalidate temporary staging directory identity before cleanup, and their
  rollback cleanup unlinks only published files whose current identity still
  matches the identity captured immediately after install while reporting
  rollback unlink failures with the original publish failure. Finalizer
  temporary staging cleanup now also reports removal failures while preserving a
  temp directory whose identity changed before removal. The
  latest attached Pixel 6 / Android 16 slot
  `google-pixel-6-6a-physical-1781077370103` verifies and signs successfully
  through the lab-app path; remaining Android release work is evidence
  acquisition for the rest of the standard matrix: Pixel 7, Pixel 8, Pixel
  Fold/Tablet, Samsung Galaxy S23, and Samsung Galaxy S24.
- Kagemusha Reserved-lineage table-base handling must stay proof-witness
  specific: lineage witnesses may carry previous recursive proofs whose
  fixed-window table-base public input differs from the current bundle proof,
  while opening length, parameter fingerprint, schedule, shared manifest,
  scalar projection, transition-profile, and proof-hash checks remain stable
  verifier-context gates.
- Kagemusha Reserved-lineage proof evidence now has a staged-run finalizer that
  requires a zero exit marker, validates staged lineage artifacts and the
  captured production proof log, writes canonical `lineage-proof-evidence.json`,
  and refuses destination overwrites by default. The matching staged runner
  first runs the canonical init and append `lineage-key-artifacts` commands
  into its staging root, then captures the canonical ignored cargo proof
  command, preserves the real exit code, writes elapsed-time metadata, and emits
  `lineage-proof-staged-run.json` so the finalizer can bind the canonical
  command, exit code, elapsed seconds, proof-log filename, proof-log byte
  count, and init/append lineage-key-artifact log byte counts before publishing.
  The readiness rollup now also rejects lineage and compact-key evidence
  `generated_at_utc` values with surrounding whitespace or control characters
  before timestamp parsing or freshness-window checks, and rejects canonical
  evidence command strings that contain surrounding whitespace, control
  characters, or secret-looking material before accepting proof or keygen
  evidence. The direct lineage and compact-key evidence helpers also reject
  helper-supplied `generated_at_utc` values more than 300 seconds ahead of the
  helper clock by default before emitting evidence JSON. The readiness CLI and
  local trust-root/evidence file validators now
  reject control-character paths before repo-root resolution, summary-output
  parent creation, signer/evidence loading, JSON parsing, or artifact hashing,
  and reject parent-segment or backslash-bearing `--repo-root` aliases before
  repo-root metadata reads, resolver normalization, or trust-root section reads.
  The same parent-segment and backslash alias gate now covers
  `--device-lab-root`, `--lineage-proof-evidence`, and
  `--compact-key-evidence` before Android root classification, readiness rollup
  construction, or evidence JSON reads. Trusted signer public-key paths reject
  those aliases before key loading, OpenSSL lookup, Android slot metadata reads,
  or summary rendering. Lower-level ABI-6 release JSON, ABI/source marker, and
  shared local lineage file validators reject those aliases before metadata
  reads, content parsing, artifact hashing, or proof-log reads. The direct
  lineage-proof and compact-key evidence helpers also reject parent-segment and
  backslash aliases in
  `--artifact-dir`, `--proof-log`, `--generator-log`, and `--out` before
  path resolution or metadata reads.
  Existing Kagemusha readiness summaries and release-bundle manifests must keep
  top-level timestamps, lineage/compact evidence-section `generated_at_utc`
  values, and Android readiness timestamp bounds canonical and within the
  release validator clock-skew window where they represent generated or maximum
  accepted evidence times;
  `--verify-existing` still allows ordinary top-level release-manifest timestamp
  refresh, but future-dated summary/manifest timestamps fail closed before
  stable manifest drift comparison. The release bundle gate now also requires
  Android readiness `slots` entries to be accepted, safe, unique, sorted, and
  inventory-matched to the signed-evidence map before any Android release
  artifacts are copied into the bundle. Slot-level Kagemusha details must carry
  canonical timestamps, non-zero lowercase SHA-256 digests, safe artifact paths,
  the required native ABI, a standard device family, and fields bound to the
  signed-evidence summary, and the slot family inventory must exactly match
  `covered_device_families`; both the slot entries and Kagemusha detail objects
  are closed schemas. Accepted slots must carry no errors, must mark every
  release-critical artifact group present, and must publish positive file counts
  for each release-critical artifact group, with accepted slot metadata bound to
  the freshly scanned device-lab evidence. Signed Android evidence must also name
  a signer digest present in the trusted signer digest list. The gate also
  rejects missing readiness-summary top-level fields, non-object required
  sections, and missing section fields with explicit missing-field/shape
  blockers before deeper per-section validation; existing release manifests also
  reject missing top-level fields, `ready=false`, and non-empty blocker lists
  with explicit blockers during verification, and saved Android signed-evidence
  and slot-artifact map keys must be safe slot ids before evidence binding
  proceeds. Fixed release-manifest evidence inventories also reject unexpected
  artifact/log item keys with a dedicated redacted inventory-item blocker.
  Direct lineage and compact-key evidence helper path preflights now also reject
  control-character paths before resolving corridors, creating output parents,
  or traversing release artifacts. The lineage helper now rejects unsafe
  `--proof-log` strings before artifact-directory metadata reads, and the
  compact-key helper rejects secret-looking, control-character, missing,
  symlinked, hardlinked, non-regular, or unreadable `--generator-log` paths
  before reading artifacts.
  Its generator-log validator also rejects unsafe `--artifact-dir` strings
  before resolving and resolves only the generator log parent before local file
  shape validation. Direct compact-key evidence builder calls reject explicitly
  unsafe `generator_log_path` strings before artifact-directory metadata reads.
  Staged lineage and compact-key runner/finalizer path validators also reject
  control-character staging, exit-marker, elapsed-seconds, artifact, and output
  paths before ancestor validation, metadata reads, or staged cleanup.
  The Kagemusha release-bundle CLI and writer now also reject control-character
  paths before bundle-root resolution, readiness or manifest JSON loading,
  trusted-signer key loading, output parent creation, or manifest writes. The
  release-bundle relative-path helper rejects secret-looking or
  control-character evidence and bundle-root strings before resolving release
  inventory paths, rejects parent-segment aliases and backslash-bearing evidence
  paths before resolver normalization, rejects parent-segment and
  backslash-bearing `--bundle-root` aliases before bundle-root metadata reads or
  shared bundle-relative path resolution, applies the same alias gate to `--out`
  before manifest writes and to `--verify-existing` before manifest loading, and
  release evidence entries run that containment check before hashing evidence
  bytes.
  Direct Android device-lab scanner path preflights now reject
  control-character roots, slot paths, JSON artifact paths, trusted signer
  public keys, and JSON summary outputs, plus parent-segment and
  backslash-bearing root, slot, JSON artifact, and JSON summary output aliases,
  before metadata reads, JSON parsing, signer loading, slot discovery, or
  output parent creation. Explicit scanner
  slot ids are now validated and deduplicated before root classification.
  The direct discovery helper repeats that validation before joining explicit
  ids to the root. Direct trusted-signer maps now reject unsafe public-key path
  strings before slot metadata reads. The production readiness rollup applies
  the same explicit slot-id validation before root classification.
  Root-discovered scanner slots and top-level slot artifact entries are sorted
  by directory name before scanning so JSON summaries, release inputs, and slot
  diagnostics stay deterministic across filesystems.
  Android device-lab and readiness-rollup summary construction now copy direct
  report dictionaries through a secret/control-string/non-finite-number
  sanitizer before release-facing JSON rendering, preserve the first value and
  emit explicit diagnostics for redacted report-key collisions, normalize
  malformed direct report statuses to failed rows, normalize non-string direct report keys
  before JSON rendering, redact non-finite direct report numbers, normalize unsupported direct report values,
  normalize malformed direct report error lists to explicit safe placeholders,
  normalize malformed direct Kagemusha report sections, render duplicate-binding
  slot lists through safe slot labels, redact unsafe direct binding slot labels in duplicate and
  malformed-digest blockers, reject
  malformed direct binding digests before duplicate checks, require canonical device-family strings before matrix
  coverage, and only reflect
  duplicate-binding values and trusted-signer summary keys that are canonical
  lowercase SHA-256 hex digests. Direct signed-evidence summary fields are also revalidated before
  reflection: timestamps must be canonical UTC, digest fields must be non-zero
  lowercase SHA-256, artifact paths must be canonical safe relative paths, and
  multiple validated reports must not collapse to the same redacted signed-evidence
  summary slot label.
  The readiness rollup also validates direct trusted-signer maps before Android
  root classification and only renders canonical signer-key SHA-256 ids. Direct
  release-bundle builders mirror that map preflight before bundle-root metadata
  reads, the verify-existing path mirrors it before manifest loading, and
  blocked manifests only render canonical signer-key digests. Release-bundle
  verification now also requires lineage and compact section digest/size maps to
  exactly match the canonical artifact and proof-log inventories, and requires
  `checked_files` to exactly match the lineage key release-tooling inventory.
  ABI-6 section constants, projected ABI-6 limits/modes, and the ABI-7
  recursive compact circuit id now fail as field-specific section-value
  blockers before generic manifest drift. Direct release-bundle build and
  verify calls now validate unsafe `repo_root` values, including parent-segment
  and backslash-bearing aliases, before bundle-root metadata checks or
  readiness/release manifest loading. Android signed-evidence
  release entries are now bound back to the signed-evidence summary path and
  digest plus freshly computed release-evidence size before generic manifest
  drift, and Android slot artifact release entries now have the same
  path/digest/size binding.
  Android summary fields, including `duplicate_bindings`, the per-slot `signed_evidence` map,
  device-family lists, and trusted signer digest fields, are also bound to freshly computed device-lab evidence during both
  readiness-summary comparison and existing-manifest verification before
  generic summary or manifest drift.
  Android covered-family summary drift now fails with an Android-specific
  blocker before generic summary drift.
  Release-bundle readiness-summary shape checks now also reject malformed
  readiness-summary Android matrix lists and trusted-signer digest lists before
  drift comparison.
  Release-bundle readiness-summary shape checks also require the top-level
  `generated_at` timestamp to use canonical UTC `YYYY-MM-DDTHH:MM:SSZ` form
  and stay within the allowed future-skew window before release evidence is
  packaged.
  Android signed-evidence slot inventory and per-slot field drift are also
  rejected with Android-specific blockers before generic summary drift during
  build-time readiness-summary comparison.
  Lineage and compact readiness-summary evidence digest/size maps now fail with
  section-evidence drift blockers before generic summary drift.
  ABI-6, ABI-7, lineage release-tooling, lineage metadata including the
  required test inventory, and compact metadata including record
  namespace/version fields now fail with section-value drift blockers before
  generic summary drift.
  Existing release-bundle manifests also bind ABI-6, ABI-7, lineage tooling,
  lineage proof evidence, and compact-key evidence section values back to
  freshly computed release evidence before generic manifest drift.
  Lineage and compact release artifact entries, proof-log entries, and the
  compact generator-log entry now bind to expected bundle-relative paths plus
  release-section digest and size fields before generic manifest drift.
  Compact generator-log artifact digest and size maps are also bound to
  freshly computed compact evidence before generic manifest drift.
  Top-level readiness-summary, lineage evidence, compact evidence JSON, and
  compact generator-log entries are now pinned to the canonical release-packet
  filenames, digest, and size fields from freshly computed release evidence
  before generic manifest drift.
  Release-bundle build validation now compares per-slot Android signed-evidence
  summary fields against freshly validated device-lab evidence before generic
  summary drift, so safe but forged slot artifact paths fail with a
  signed-evidence drift blocker.
  Slot-relative artifact path
  normalizers now also reject control-character or surrounding-whitespace
  relative paths before stripping, manifest, metadata, signed-evidence, or
  signer digest reads.
  Signed-evidence helper path preflights now also reject control-character
  slot, private-key, public-key, and output paths before metadata reads,
  OpenSSL lookup, JSON parsing, or output parent creation. Absolute
  signed-evidence output paths also reject symlinked ancestors and symlinked
  leaves, and output paths reject backslashes plus absolute parent-segment
  aliases before resolver normalization can map them onto the canonical slot
  evidence output.
  Device-lab scanner summary construction now normalizes finite float values in
  direct report inputs as unsupported summary values and redacts non-finite
  numbers, keeping release-facing summaries free of injected floating-point
  scalars outside the scanner-produced schema.
  The attestation report helper now rejects noncanonical slot IDs and
  slot-relative certificate-chain path spellings before writing
  `attestation/report.json`, so `./slot`, trailing slash slot aliases,
  `attestation/./...`, repeated separators, and trailing slash chain-path forms
  cannot be normalized into release metadata.
  It also rejects backslash-bearing certificate-chain paths before
  `PurePosixPath` handling, matching the shared slot-relative metadata path
  policy.
  The signed-slot assembler now applies the same canonical single-name rule to
  `--slot-id` before joining or creating slot paths, so `./slot`, `slot/`, and
  `slot/.` aliases fail before publish staging starts.
  Shared Android device-lab slot-id validation now applies that exact spelling
  rule to raw-puller `--slot-id` and scanner `--slot` inputs as well, so
  explicit slot aliases fail before ADB, slot discovery, or path joins.
  The signed-slot assembler and attestation report helper also reject
  backslash-bearing `--slot-id` values through the same safe-name gate, matching
  the shared scanner/raw-puller validator.
  Filesystem-discovered scanner slot directories now reject backslash-bearing
  names before metadata reads, keeping implicit discovery aligned with explicit
  slot-id validation.
  Shared slot-relative artifact path validation now also rejects dot-segment,
  repeated-separator, and trailing-slash aliases before `sha256sum.txt`,
  `slot.json`, signed-evidence, or release-bundle paths can normalize into
  digest-bound evidence.
  Android signed-evidence helper JSON output validators now reject
  parent-segment and backslash-bearing output aliases before output parent
  metadata reads, matching the CLI output normalizer.
  Signed-slot assembler source-copy preflights now reject control-character,
  parent-segment, and backslash-bearing artifact source paths before ancestor
  validation, metadata reads, or destination directory creation, and signed-slot
  assembler root preflights reject control-character, parent-segment, and
  backslash-bearing device-lab roots before root classification or directory
  creation. Signed-slot assembler source metadata strings also reject
  control characters before metadata binding. Signed-slot assembler source digest preflights now reject blank or noncanonical attestation challenge, app-signing, and offline-policy SHA-256 fields before unsigned staging output or signed evidence can be published.
  It also emits per-phase closed-schema execution reports for init, append, and
  proof attempts so signal-style failures are diagnosable without becoming
  publishable evidence; each execution report now includes an execution-report
  SHA-256 of the child log so resume rejects digest drift as well as byte-count
  drift. The wrapper exits conventionally on failure while
  preserving the exact subprocess status in the staged marker, execution
  reports, and run report.
  The runner can now resume at init/append key-artifact phase boundaries:
  `--resume-key-artifacts` reuses only phases whose artifacts, log, canonical
  zero-exit execution report, and log byte count validate, then reruns missing
  or failed regular phase outputs while still rejecting symlinked or hardlinked
  staged material. `--resume-key-artifacts` is mutually exclusive with
  `--replace`, so a caller cannot mix selective phase-boundary resume with full
  staged-output replacement.
  Staged metadata writes are now self-verifying: after the atomic rename the
  runner reopens marker, elapsed, and JSON report files, checks the opened file
  identity, and compares exact bytes before returning.
  Long-running child commands write stdout/stderr directly to their temporary
  staged log file instead of through a supervisor-owned pipe, so interrupted
  supervisor sessions are less likely to strand keygen/proof children behind a
  broken output pipe while the existing exit-marker/report gates continue to
  prevent ambiguous partial runs from becoming release evidence. The production
  guard forbids the old `subprocess.PIPE`, `process.stdout.read`, and
  `sys.stdout.buffer` staged-runner patterns so Python-side output mirroring
  cannot silently return. Resume/replace cleanup and temporary log/output
  cleanup now unlink staged files only through an identity-checked parent file
  descriptor, so a path swapped after validation is reported as cleanup drift
  instead of being removed. Evidence-helper validation scratch files use the
  same parent-fd identity check before cleanup.
  The finalizer also verifies each published file after install by reopening it
  through the identity-bound artifact reader and comparing it with the staged
  source bytes before the final evidence check, then identity-binds the
  published artifact directory before the final fsync and revalidates temporary
  staging directory identity before cleanup. Rollback cleanup after failed
  copies, readback verification, or final evidence publication also refuses to
  unlink a path unless its current identity still matches the publish-time file
  identity captured after install.
  Older staged attempts, including one retry with the existing release `iroha`
  binary, exited `-9` during init LEN=128 key generation after producing only
  the init key-log. A lower-memory key-generation-only verifier-slice shape path
  is now implemented for the one-hop and append circuits, the processed
  verifier-key bytes have been checked against the full circuits in explicit
  expensive equivalence tests, and the release CLI has been rebuilt with that
  path. A replacement production-width staged run is in progress; the remaining
  lineage release blocker is successful init/append key-artifact generation plus
  the heavy ignored proof run, followed by finalization into
  `artifacts/kagemusha`.
- Kagemusha ABI-7 recursive compact key evidence now has a staged-run finalizer
  that requires a zero exit marker, validates staged artifacts and the generator
  log, writes canonical `recursive-compact-key-evidence.json`, and refuses
  destination overwrites by default. The matching staged runner captures the
  canonical ABI-7 recursive compact key-generation command, preserves the real
  exit code in the marker, execution report, and run report while returning a
  conventional wrapper failure status, and refuses staged overwrites by default.
  `--resume-keygen` now reuses only a complete zero-exit compact keygen whose
  artifacts, generator log, execution report, run report, marker, canonical
  command, generator-log byte count, and execution-report SHA-256 validate;
  staged execution-report and run-report command fields now reject surrounding
  whitespace, control characters, and secret-looking material before canonical
  command matching. Failed or malformed regular
  staged outputs are replaced and rerun, while unsafe aliases still fail closed.
  `--resume-keygen` is mutually exclusive with `--replace`, so operators must
  choose validated resume or full staged-output replacement before cleanup.
  The runner also identity-binds the generator-log parent sync after log
  install and self-verifies marker and JSON report writes after atomic rename
  so post-write metadata drift is rejected before finalization.
  The keygen child writes stdout/stderr directly to the temporary generator log
  file rather than a supervisor-owned pipe, preserving the staged log on
  multi-hour runs while still requiring the zero-exit marker, execution report,
  run report, and byte-count validation before reuse or finalization. The
  production guard also forbids the old staged-runner pipe-read/stdout-mirror
  patterns from returning.
  The finalizer reopens every published key artifact, generator log, and
  evidence JSON after install and compares the identity-bound readback with the
  staged source bytes before reporting success, then identity-binds the
  published artifact directory before the final fsync and revalidates temporary
  staging directory identity before cleanup.
  The previous staged production-width keygen attempt exited nonzero (`143`)
  after about 9h26m with no artifacts. A detached replacement retry is in
  progress; the remaining compact-key release blocker is a successful rerun that
  produces artifacts and is finalized into `artifacts/kagemusha`.
- Continue reducing local/CI compile memory after the WSL cargo-test hardening
  and Kagemusha record-bound compact preflight isolation: plain default tests no
  longer run the heavy ABI-7 recursive compact record-bound Pallas proof matrix
  or the oversized private Sumeragi main-loop unit-test harness.
  `iroha_data_model` still has a single stripped-debuginfo compile phase that
  can peak around `10.5 GiB` RSS, so future work should split or simplify that
  compile surface rather than reintroducing broad Cargo parallelism or
  one-file-one-binary integration-test discovery.
- Native asset locks are now first-class ISIs for escrow-style conditional
  custody, including optional release authority, expiry, partial drawdown,
  deterministic custody, Python SDK helpers, negative/adversarial unit tests, and
  a 4-peer localnet coverage path. Keep future escrow work on this native
  instruction surface unless it explicitly needs IVM contract semantics.
- BFV full-bootstrap release artifact binding now includes the typed arithmetic
  AIR constraint-system artifact in governed circuit material and
  artifact-bundle digests, and proof-key material now binds the non-circular
  evaluator artifact set it verifies. Core's STARK/FRI AIR builder and verifier
  now accept explicit caller-owned trace rows and composition vectors, and the
  Soracloud release-prover handoff feeds typed BFV AIR evaluation material into
  finalized BFV-native execution proof attachments accepted by the governed
  verifier under the configured STARK enablement and proof/envelope byte caps.
  Material-native AIR uses the same explicit verifier corridor with
  verifier-reconstructed zero composition values, preserving the v1 FRI
  final-zero invariant while binding typed material through trace and
  composition roots. A deterministic release audit evidence payload and digest
  now bind the generated artifact-bundle digest, evaluator artifact-set digest,
  prover/verifier pair commitment, native payload digests, and proof-profile
  field counts for release bundles, and a signed release-audit signoff payload
  now binds that evidence digest to the external audit report/archive digests
  and reviewer public key. Signoff validation can rederive the evidence from
  governed material and concrete artifacts before accepting the reviewer
  signature, and a canonical release-audit record now packages evidence plus
  signoff under its own digest domain for release archives. A release-audit
  package now carries the external report/archive bytes, checks them against the
  signed hashes, rejects empty or all-zero audit artifacts, enforces bounded
  byte payloads, requires canonical v1 report/archive byte headers with
  nonempty nonzero bodies, rejects blank or sub-64-byte audit artifact bodies,
  rejects canonical nested audit headers even after leading body whitespace,
  rejects placeholder-style audit artifact bodies across the full bounded body
  including draft, `not for production`, `not production ready`, and
  `replace before production` markers with dash/underscore variants, and copied
  report/archive bodies, including edge-whitespace-decorated copies,
  and requires
  caller-supplied trusted reviewer id/key validation before publication. The
  same package now carries a machine-checkable release audit manifest and
  manifest digest that
  require an approving verdict, canonical audit scope, signed record digest,
  evidence, artifact, evaluator-set, proof-key, prover/verifier-key,
  native-circuit, and report/archive commitment binding, and reviewer id/key
	  agreement before publication. The crypto release-audit validator now offers a
	  single governed-artifacts/trusted-reviewer/caller-pinned-digest gate that
	  rejects zero or known placeholder pinned package digests before comparison, and
	  preflights caller-supplied reviewer id/key inputs, including malformed or
	  all-zero reviewer public-key payloads, before package or artifact validation
	  can mask malformed trust configuration. Standalone release-audit
	  signoff, record, and manifest trusted-reviewer validators now share that
	  caller trust-anchor preflight before stale signed objects are parsed, and
	  signoff payload construction preflights reviewer id/key plus external
	  report/archive digests before stale evidence can mask malformed operator
	  inputs. Release-audit record and package construction now reject malformed
	  reviewer ids before evidence derivation or audit-byte validation, and
	  package construction now shares the report/archive byte-pair preflight so
	  edge-whitespace-normalized copied audit bodies fail before evidence
	  derivation or record signing.
		  Core's audited material and execution prover wrappers require that gate before
		  native BFV proof attachments are emitted, including copied report/archive body
		  rejection after edge-whitespace normalization plus refresh transcript
		  public-key digest validation at the wrapper boundary; the material-native
		  AIR builder now replays generated envelope bytes against the governed
		  material AIR context before wrapping; execution witness material also carries a domain-separated
	  Galois-key-set digest so artifact-aware replay rejects same-shape stale
	  automorphism key substitutions before proof-input or release-prover package
	  hashing; lower-level typed material/prover-input helpers stay internal, and
	  the material and execution public-input schemas advertise the package-digest
	  pin, package-level header-only/nested-header/whitespace-prefixed
	  nested-header, zero-body, blank-body, padded zero/blank-body, and
	  placeholder/case-decorated/whitespace-prefixed placeholder
	  external-digest rejection,
	  nested, whitespace-prefixed nested, blank, sub-64-byte, and
	  full-body delayed-placeholder audit-artifact body rejection, canonical audit
	  artifact-header/body and distinct-body requirements, and the execution
	  witness Galois-key-set binding. Standalone release audit evidence
	  validation now also rejects reused artifact/profile/native-payload commitments
	  plus empty/all-zero and short, long, padded, binary-decorated,
	  case-decorated, whitespace-prefixed, draft, `not for production`,
	  `not production ready`, or `replace before production`
	  dash/underscore-variant placeholder native-payload digest sentinels,
	  governed material digest admission rejects the same
	  draft/not-for-production/replacement marker family before circuit material,
	  proof-key material envelope/profile metadata, blind-rotation accumulator
	  material, caller-expected material proof-profile digests, material/execution
	  proof-input statement hashes, public-padding AIR rows, release-audit evidence,
	  signoff, manifest, or caller-pinned package digest slots can pass,
	  standalone record construction plus signoff/manifest validation rejects
	  external audit digest aliasing with signed release commitments plus known header-only, nested-header,
	  whitespace-prefixed nested-header, padded zero/blank-body, and
	  short/long/padded/binary-decorated/case-decorated/whitespace-prefixed
	  placeholder report/archive digests, and
	  public crypto helpers build canonical report/archive bytes from
	  externally supplied bodies while shared body extraction enforces nested
	  headers and delayed placeholder text before release tooling packages them. Manifest adversarial coverage now also exercises stale manifest
	  version/field-count values, stale manifest-authorized package version/count
  values, padded scopes, and rejected verdicts through direct validation,
  manifest digesting, package validation, and package digesting. Core audited
  material and execution prover regressions now also prove rejected manifest
  verdicts stop at the release-audit gate before native proof attachments are
  emitted. The execution
  public-input schema advertises that distinct evidence-commitment requirement.
  The BFV AIR composition evaluator now derives
	  per-row/column challenges from the public statement hash, canonical
	  row-major trace-material digest, row index, and column index, remapping zero
	  challenges to one so residuals are bound to the evaluated witness package,
	  and the release-prover
	  input digest path now hashes AIR evaluation material only after the same
	  trace-bound composition-vector validation and hashes witness, embedded
	  proof-input, plus outer release-prover material only after artifact-aware
	  prefix-trace replay; material-proof input digests now also have a
	  caller-bound path that reconstructs the package from caller-owned
	  evaluation keys and artifacts before hashing, and Core's material native
	  AIR handoff consumes that caller-bound digest before proof emission. Core's
	  execution native AIR handoff now consumes the artifact-bound release-prover
	  digest before proof emission as well. The material and execution native AIR
	  proof wrappers also decode the native STARK/AIR envelope before attachment
	  construction and reject transcript-label, circuit-id, missing-AIR-section,
	  or public-digest/statement-hash drift before proof validation can rely on
	  the wrapper. The
	  shared STARK/AIR
			  prover and verifier now derive duplicate-free query schedules by
			  bound-specific transcript rejection sampling without replacement,
					  require noncanonical transcript labels, including padded BFV
					  native AIR retry-label aliases, malformed domain tags,
					  and malformed AIR or verifier-key circuit ids to fail closed before query replay or envelope verification,
					  keep caller-provided verifier limits from relaxing canonical
					  STARK structure and envelope-byte caps, and reject
					  blowup/domain parameter pairs where `blowup_log2` exceeds
					  `n_log2` before proof synthesis, verifier-key admission, or
					  envelope verification,
			  while failing closed when a duplicate-free schedule cannot exist, so
			  duplicate openings cannot reduce effective sampling. Native-AIR proof
		  synthesis now also rejects nonzero final FRI folds before BFV-native or
		  public AIR proof bytes are returned. The BFV material and execution
		  native-AIR builders still retry bounded statement/material-domain
		  query nonces for privacy-policy public-row constraints before returning
		  duplicate-free proof envelopes, and the BFV execution wrapper now has a
		  structurally valid generic-AIR negative control proving allowed transcript
		  labels with private-row openings fail the BFV no-unmasked-private-row
		  policy before native acceptance. The ZK-ACE native AIR prover now routes
		  generated query chains through the same duplicate-free validator and
			  self-verifies encoded envelopes before returning proof bytes. BFV native
			  STARK/FRI proof-key material, verifier payloads, and release-audit proof
			  profiles now also reject blowup/domain parameter pairs where
			  `blowup_log2` exceeds `n_log2` before key material or evidence can be
			  admitted. Shared STARK/FRI verification now keeps auxiliary generic
			  composition payloads (`comp_root`/`comp_values`) scoped to the generic
				  binding AIR context and rejects them for caller-owned explicit AIR and
				  ZK-ACE AIR before statement replay, and generic sidecars must rederive
				  the AIR public digest from strictly ordered auxiliary terms before their
				  composition leaf is accepted. Caller-owned explicit AIR trace roots now
				  reject non-canonical Goldilocks row elements before hashing, so malformed
				  row material cannot be bound under an otherwise valid explicit AIR
				  verifier context. STARK `OpenVerifyEnvelope` wrapper
			  verification rejects inner auxiliary sidecars for both generic binding
			  and ZK-ACE wrappers, keeping generated wrapper proofs canonical.
			  Generic STARK `OpenVerifyEnvelope` construction and verification also
			  require verifier-key payloads to meet the ledger-grade production FRI
			  floor and verifier-key backend labels to exactly match the requested
			  proof backend before wrapper proofs can be emitted or accepted.
			  ZK-ACE STARK circuit ids are classified after backend normalization,
			  and generic STARK wrapper construction rejects ZK-ACE circuit aliases
			  before they can fall back to binding AIR.
			  BFV full-bootstrap STARK circuit ids are now classified the same way,
			  so generic wrapper construction and verification reject BFV aliases
			  before the generic binding AIR can admit native full-bootstrap proof
			  attachments without BFV-specific public-opening checks.
			  Wrapper verification also pins inner AIR transcript labels to the
			  canonical generic binding or ZK-ACE domain so regenerated proofs under
			  alternate STARK transcript domains fail closed.
			  The STARK `ivm-execution-v1` helper also checks the normalized circuit
			  id before wrapper construction, so matching non-IVM verifier keys cannot
			  be used to emit IVM-shaped proofs for another circuit.
			  Governed
			  full-bootstrap material admission
			  now also rejects known nonzero pending, placeholder, native proof-key
			  payload, draft, not-for-production, and replacement digest literals before
			  artifact, proof-key pair, key-material envelope/profile metadata, blind-rotation
			  accumulator material, coefficient/slot linear-transform diagonals,
			  sample-extraction switch-key digit limbs, all-zero/malformed/stale
			  evaluator artifact-set envelopes, opaque evaluator artifact payloads,
			  placeholder or duplicate evaluator/bundle digest-material fields,
			  extra or missing full-bootstrap execution Galois keys, inert all-zero
			  Galois/relinearization key-switch entries,
			  BFV public-key digest, seeded-encryption, identifier
			  public-parameter/ciphertext slots, all-zero BFV public-key
			  components, bootstrap statement,
			  full-bootstrap material statement, refresh-transcript, public/secret
			  consistency public-key material, all-zero secret-key material, and full-bootstrap execution
			  claim/trace ciphertext and raw-sample material,
			  bootstrap public-key digest metadata,
			  evaluation-key bundle digest refresh masks, bootstrap zero-refresh
			  proof-statement refresh ciphertexts,
			  aliased execution witness digest commitments,
			  caller-expected material proof-profile digests,
			  material/execution proof-input statement hashes, public-padding AIR rows,
			  or release-audit evidence
			  commitments can be accepted, including standalone release-audit signoff
			  and manifest commitments, release-audit key evidence rejects placeholder key
			  digest/material commitments plus inert native-payload digest sentinels
			  including generic proof-key placeholders, native proof-key material admission rejects digest-correct inert native
			  payloads including the same generic placeholder bytes, and execution proof
			  statement hashing rejects the known pending execution witness digest literal. Generated
			  execution claims now use a non-sentinel transient witness digest before
			  deriving the governed digest. The BFV native
			  STARK/AIR prover/verifier wrapper now derives the domain tag from the
			  execution statement hash, pins the canonical circuit id and FRI profile,
			  and rejects sampled openings unless they are the statement-bound
			  public-padding rows; generic STARK `OpenVerifyEnvelope` admission now
			  refuses to route that BFV circuit through the binding AIR fallback.
			  Soracloud BFV input-admission, bootstrap-key, full-bootstrap material,
			  and execution proof attachments now require the canonical BFV STARK/FRI backend
			  (`stark/fri/sha256-goldilocks`) and advertise that backend in their
			  public-input schema descriptors, so alternate production STARK profiles
			  cannot satisfy governed BFV proof gates.
			  BFV full-bootstrap proof-key profile validation also rejects known
			  placeholder/draft/not-production sentinel hashes in the registered
			  parameter/RNS/decomposition profile, pair, and material commitment slots
			  before commitment recomputation or governed material matching, and
			  generated proof-key construction no longer uses a known pending
			  material-commitment sentinel while deriving canonical pair and per-key
			  commitments.
			  Artifact-aware BFV execution witness validation now reports the first
			  mismatched governed trace/bound field, and regressions pin diagnostic
			  slot-to-coefficient plus sample-switch output drift as artifact-only
			  replay failures rather than shape-only witness-material failures.
			  The BFV AIR composition challenge contract/schema now pin the challenge
			  domain, statement hash, trace-material digest, row index, column index,
			  and nonzero remapping policy, so regenerated artifacts cannot silently
			  fall back to statement-only or coordinate-agnostic composition streams.
		  Remaining BFV full-bootstrap
		  production work is the audited arithmetic proof-producing backend plus
	  externally audited generated prover/verifier artifacts and report/archive
	  production with canonical v1 headers, nonzero bodies, and
	  leading-whitespace-tolerant full-body delayed-placeholder rejection for the
	  generated circuit, not the
	  artifact/material/schema/native-envelope/audited-wrapper binding corridor.
- SoraFS/SoraNet first-release KDF identifier cleanup is complete: SoraFS
  envelopes remain V1/version 1 with the transcript-bound hybrid suite label,
  SoraNet advertises only NK2/NK3 suite IDs `0x04`/`0x05`, and old pre-release
  suite labels/IDs are intentionally rejected. Keep future fixture and SDK work
  aligned with the regenerated `snnet-interop-nk{2,3}-v1.json` contents rather
  than adding compatibility aliases.
- SCCP launch scope is limited to Ethereum, BSC, Solana, TON, and TRON. Proof
  manifests, checked encoders, verifier dispatch, Torii public discovery, SDK
  helpers, and production readiness surfaces must stay limited to those lanes.
  Retired runtime-network families outside that launch scope are explicitly
  unsupported for now.
  SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now.
  No current source proof, manifest, SDK helper, or Torii route should be
  treated as Sub&#115;trate/Pol&#107;adot-compatible.
  That exclusion is intentional current-launch scope, not a hidden
  compatibility lane.
  Torii OpenAPI SCCP discovery descriptions must carry the same no-support
  sentence so relayers see the exclusion before reading proof manifests.
  Release-readiness and strict bundle source inventories now pin both Torii
  OpenAPI SCCP capability/manifest descriptions to that sentence.
  The retired-network surface guard must require explicit no-support
  launch-scope wording in each launch-scope file, including the exact escaped
  Sub&#115;trate/Pol&#107;adot no-support sentence.
  Generated release-readiness Markdown and verifier-owned release-bundle
  Markdown must also carry that exact sentence in the Required Release Evidence
  section before public artifacts can satisfy readiness.
  Reintroducing any such family requires a new design pass, fresh fixtures, and
  explicit governance approval rather than reviving diagnostic code paths.
- SCCP TRON route-config production blockers must stay fail-closed at the
  route-manifest boundary. Release-readiness and strict release-bundle source
  inventory now pin the post-deploy blocker key list and adversarial route
  overlay tests for source-event, route-canary, full-TOML, generic
  post-deploy, scalar, malformed, and contradictory blocker evidence.
- SCCP destination evidence reparse diagnostics must stay category-only before
  public TOML blockers are emitted: EVM copied bridge/verifier runtime bytecode
  evidence and TON copied verifier code BoC base64 evidence must not propagate
  raw parser details or operator-provided bytes into readiness artifacts.
  The EVM destination helper must keep separate adversarial copied-bytecode
  regressions for the bridge and verifier runtime roles, both pinned by
  release-readiness and strict bundle source inventory.
- SCCP imported EVM live summary reparse diagnostics must stay category-only
  before public TOML blockers are emitted: source bridge, destination bridge,
  and destination verifier runtime bytecode metadata must not propagate raw
  parser details or operator-provided bytes from copied live evidence.
- SCCP helper CLI diagnostics must stay bounded before operator logs enter
  release artifacts: source, destination, receipt-proof, EVM live, Solana/TON/TRON
  live, and TRON source bridge helpers must redact sensitive top-level failures
  to fixed evidence categories, with adversarial `secret-token` regressions pinned
  in the release public scalar-text source inventory. That inventory must pin
  the actual adversarial token inputs for EVM live/source-live, TON live, and
  TRON live parser/transport failures, not only generic `secret-token` absence
  assertions.
- SCCP duplicate-JSON diagnostics must stay fixed and traceback-safe before
  public operator output is emitted: EVM receipt, EVM live/source-live, Solana
  live, and TON live helpers must report method or endpoint categories while
  suppressing duplicate-key exception chains.
- SCCP imported live metadata reparsing must stay category-only before TOML or
  release summaries are produced: EVM live/source-live hex fields, Solana live
  verifier identity/executable fields, and TON live address/transaction-LT
  fields must suppress lower-level parser exception chains.
- SCCP direct Solana/TON destination verifier identity reparsing must stay
  category-only before TOML or JSON summary rendering, with parser exception
  chains suppressed and adversarial `secret-token` regressions pinned. Release
  public scalar-text source inventory must pin the direct destination
  parser-detail payloads themselves for both Solana and TON.
- SCCP all-lanes Solana live ProgramData and route-canary base64 comment
  diagnostics must stay category-only before aggregate blockers are emitted; raw
  canonical-base64 parser details must not be copied into release readiness
  output. Release public scalar-text source inventory must pin the Solana
  all-lanes adversarial base64 and ProgramData parser payloads themselves, not
  only generic absence assertions.
- SCCP TRON solid-block header proof canonicalization failures must stay
  category-only before live-evidence summaries or full-TOML blockers are
  emitted; lower-level proof encoder exception text must not be copied into
  public readiness output.
- SCCP source-material evidence must reject built-in template verifier hashes as
  a release gate, not only as local script behavior. The
  `source_material_template_rejection_gate` source inventory pins ETH, BSC,
  Solana, TON, and TRON evidence-script guards plus negative tests so
  template-derived source verifier material cannot satisfy production readiness
  silently.
  The companion `source_material_role_validation_gate` pins zero-hash,
  role-reuse, canonical source-adapter verifier, and full-light-client audit
  role-separation guards across the same source families before source material
  can satisfy release readiness. It also pins the Rust source-state and
  source-adapter verifier preflights that reject opaque or compressed nested
  FastPQ backend bytes inside OpenVerify envelopes, plus deployment-matcher
  rejection of replayed source-adapter verifier-key hashes before the wider
  production verifier path is consulted; release readiness must fail if that
  direct deployment-matcher regression is removed.
- SCCP active-launch readiness metadata must stay canonical: EVM live source
  and destination chain ids in readiness summaries are decimal-only (`1` for
  Ethereum mainnet, `56` for BSC mainnet), so JSON-RPC quantity spellings such
  as `0x1`, leading-zero values such as `01`, whitespace-padded values, and
  plus-signed, decimal-looking, Unicode-confusable, or numeric JSON values
  remain evidence blockers. The readiness-report and strict bundle tests also
  mutate source and destination metadata independently so one canonical side
  cannot hide the other side's drift.
- SCCP Ethereum source-event context inventory must keep the Rust EVM receipt
  duplicate matching-log rejection pinned alongside receipt-log RPC context
  checks, so one source receipt cannot satisfy admission with multiple matching
  SCCP logs. The release-readiness and bundle-verifier inventory tests now
  remove that Rust marker directly and fail the gate, so duplicate-log coverage
  cannot be satisfied only by the Python receipt-context script tests.
- SCCP Solana UI prover requests must stay deployment-bound: JavaScript,
  Python, Swift, Kotlin, and Java Android request builders reject zero/zero
  source-adapter deployment bindings, while the low-level binding normalizers
  keep zero/zero available only for diagnostic fixtures and canonical hashing
  checks.
- SCCP JavaScript EVM-family and TRON Groth16 proof request builders must keep
  the canonical bundle gate aligned with Python and Rust: source and dist
  normalizers reject arbitrary bundle bytes, public-input drift, missing
  non-SORA source proofs, and `bundleBytes.sourceDomain` drift before local
  prover callbacks run. JavaScript package-root regressions now exercise the
  same EVM-family and TRON `bundleBytes.sourceDomain` drift rejection through
  the published `dist/index.js` entrypoint.
- SCCP Swift, Kotlin/JVM, and Java Android EVM-family/TRON Groth16 proof
  request builders must stay on the same canonical bundle gate: outbound
  builders reject unsupported non-SORA source domains before bundle parsing,
  decode only canonical SCCP message-proof bundles, require transparent
  public-input matches, and reject `bundleBytes.sourceDomain` drift. Broad
  Swift, Kotlin/JVM, and Java Android SCCP suites now pass locally on the Java
  21 and Swift harnesses, including the separate Java Android Solana JUnit
  class that is not part of the main-based Gradle harness. Release-bundle source
  inventory now also deletes native SDK proof-request markers file-by-file in an
  adversarial regression before this gate can pass. The same inventory now pins
  native Swift, Kotlin/JVM, and Java Android canonical EIP-55 EVM account-field
  validation inside shared SCCP bundle parsers, plus NUL-prefixed fixed token
  name/symbol rejection so hidden post-NUL text cannot make empty token fields
  appear populated. TON native bundle tests also reject noncanonical EIP-55
  EVM source senders before non-SORA source proofs can satisfy request
  building, and release readiness pins the Swift, Kotlin/JVM, and Java Android
  TON parser implementation markers directly so test-only rewrites cannot hide
  parser regressions.
- SCCP client SDK route-canary helper parity must stay pinned: Python Torii
  client, JavaScript source/dist, Swift, Kotlin/JVM, and Java Android helpers
  reject reused route-allowlist, destination-binding, source-material, and
  source-deployment hashes before app-side canary evidence is packaged. Python
  package-root regressions now exercise Solana, TON, and TRON governed hash
  role-reuse negatives through `iroha_torii_client`, so root-import coverage
  cannot be satisfied only by deep `sccp` module tests. JavaScript package-root
  and package-dist regressions now exercise the same Solana, TON, and TRON
  governed hash role-reuse negatives through the published entrypoints.
- SCCP production-corridor Gradle phases must be self-contained under default
  runner settings: Kotlin/JVM and Java Android phases export a default
  `GRADLE_OPTS` heap corridor (`-Xmx6g` for Gradle and the Kotlin daemon) before
  invoking Gradle, while operator-provided `GRADLE_OPTS` still override those
  defaults. This keeps SCCP SDK validation from producing local memory false
  negatives before tests run. The corridor runner must also reject empty
  `--log-dir` values so local release rehearsals cannot silently skip strict
  phase transcript collection. Release-readiness and strict bundle source
  inventory must pin those direct runner regressions so deleting the heap or
  log-dir guards block public readiness.
- SCCP release-bundle corridor schema must classify unknown corridor root
  fields and corridor `phases`/`evidence_artifacts` keys before semantic phase
  lookup, manifest artifact ownership, transcript inspection, or Markdown
  invariant checks. Safe ASCII operator names may remain readable in
  diagnostics, but padded, control-character, whitespace, Markdown-unsafe,
  malformed, or Unicode-confusable keys must be category-only blockers. The
	  bundle builder must also require the canonical corridor root shape,
	  classify malformed copied corridor root fields before render, require
	  canonical corridor blocker lists, reject malformed copied phase-map keys,
	  reject invalid copied phase statuses, and require hashed evidence artifacts
	  for copied passed phases before `--allow-not-ready` diagnostics can render or
	  write public artifacts. Copied corridor summaries must also keep
	  `production_ready = true` and `require_phase_evidence = true` before public
	  output is written, matching the strict verifier's readiness requirements.
- SCCP all-lanes public JSON schemas must classify unknown summary, lane,
  nested evidence-object, route-canary, and source-adapter audit-hash keys
  before semantic matching or hash-role checks. Safe ASCII operator names may
  remain readable, but malformed or Unicode-confusable keys must never be
  echoed from readiness-report embedded evidence or the standalone all-lanes
	  summary. The bundle builder must reject copied embedded evidence root/lane
	  unknown fields, malformed domain/chain/production-ready scalars, noncanonical
	  blocker lists, malformed record-flag containers, and nested evidence-object
	  shape drift before `--allow-not-ready` diagnostics can render or write public
	  artifacts. Copied embedded evidence must also recompute from the copied TOML
	  evidence input artifacts before rendering, so root-level summary drift cannot
	  publish before the strict bundle verifier compares the final JSON files.
		  The strict verifier must also reject duplicate integer entries in copied
		  all-lanes domain lists, including `supported_launch_domains` and
		  `unsupported_launch_domains`, before relying only on launch-scope set
		  comparison diagnostics. The bundle builder's pre-render copied-evidence
		  schema regression now pins duplicate `required_domains`,
		  `supported_launch_domains`, and `unsupported_launch_domains` blockers
		  before public artifacts are written. Copied all-lanes summary, lane, and `records`
		  unknown field names must use the same public-field classifier before
		  bundle rendering, so padded, control-character, whitespace,
	  Markdown-unsafe, or Unicode-confusable names cannot leak raw public
	  diagnostics.
	  Copied embedded evidence must also classify nested
	  `source_record_hashes`, `source_adapter_gate.audit_hashes`,
  `evm_live_metadata`, `destination_binding`, `route_allowlist`, and
  `route_allowlist.route_canary` field/key/hash/scalar drift before public
  diagnostics can be rendered. Active EVM copied metadata must also keep
  `required = true`, active readiness, canonical decimal source/destination
  chain ids, and Ethereum `finalized` block tags before not-ready diagnostics
  can be emitted. Future diagnostic lanes may stay incomplete during the
  first-launch bundle, but any copied nested fields they do include must still
  pass canonical shape checks. Copied source-adapter gates for active or
  production-ready lanes must also preserve domain-specific required/empty gate policy,
  expected audit-key sets, gate-hash-to-audit matching, and empty ready-gate
  blockers before public bundle output is written. Copied route-canary records
  for those lanes must preserve common semantic bindings as well: `status =
  passed`, expected lane evidence source, `evidence_bound = true`, and
  route/destination hashes matching the sibling lane records before Markdown or
  public JSON output is written. Copied route-canary evidence hashes must also
  stay distinct from same-lane governed hashes, same-lane canary roles, other
  lane canary evidence hashes, and other lane governed hashes before public
  output is written. The bundle builder's pre-render regression now exercises
  cross-lane replay against another lane's canary evidence hash, source-material
  hash, destination-binding hash, and route-allowlist hash. EVM-family copied route canaries must also
  preserve non-zero and distinct transcript hash roles, positive/u32 receipt
  metadata, lane-bound target domain, `proof_version = 1`, SORA proof source,
  and finalized message-proof booleans before bundle output is written.
  TRON copied route canaries must likewise preserve canonical signer addresses,
  recovered-signer ownership, non-zero and distinct transcript/governed hash
  roles, positive u64 block numbers, non-negative u64 block timestamps, TRON
  target domain, `proof_version = 1`, SORA proof source, and true owner/proof
  verification booleans before public output is written. Solana copied route
  canaries must preserve a non-zero canonical
  ProgramData address and canonical positive ProgramData slot, while TON copied
  route canaries must preserve non-zero live-account hashes, canonical positive
  transaction LT, and governed hash-role separation before public output is
  written.
- SCCP release-bundle manifest/readiness roots and public artifact rows must
  classify unknown top-level and artifact field names before artifact closure,
  manifest order, or Markdown table checks. Safe ASCII operator names may remain
  readable, while padded, control-character, whitespace, Markdown-unsafe,
  malformed, or Unicode-confusable keys must be category-only blockers. The
  bundle builder must classify copied report-artifact row field names and reject
  copied report-artifact rows with unknown fields, malformed bundle-relative
  path text, zero, negative, or non-integer byte counts, or noncanonical
  SHA-256 text before `--allow-not-ready` diagnostics can render or write
  public artifacts.
  Release-readiness and strict bundle source inventory must pin those direct
  artifact-row regressions so deleting the unknown-field, byte-count, or
  digest-canonicality guards blocks public readiness.
- SCCP cryptographic-evidence public rows must classify unknown row field names
  before lane binding, route-canary binding, Markdown checks, or source-adapter
  audit semantics. Safe ASCII operator names may remain readable, while
  padded, control-character, whitespace, Markdown-unsafe, malformed, or
  Unicode-confusable row names must never be echoed. The bundle builder must
  also classify copied cryptographic-evidence row field names and reject copied
  cryptographic-evidence rows with malformed domain/chain scalars,
  boolean/null fields, optional bytes32 text, optional block-number fields,
  source-adapter audit-hash maps, or audit-hash keys before
  `--allow-not-ready` diagnostics can render or write public artifacts.
  Release-readiness and strict bundle source inventory must pin the direct
  public-row schema regressions as well, including zero governed hashes,
  route-canary/source-gate domain-policy drift, exact JSON type drift, and BSC
  testnet row-shape checks.
- SCCP BSC TAIRA XOR route-config generation must reject contradictory
  post-deploy readiness: production-ready route manifests cannot carry non-empty
  `postDeployLiveEvidence` production blocker arrays, and malformed blocker
  containers must fail closed before a governed Torii overlay can be rendered.
  Route-manifest JSON string fields must also be canonical before route-config
  normalization: surrounding whitespace in route ids, asset keys, network ids,
  post-deploy transaction ids, offline TOML hashes, uppercase bytes32 metadata,
  uppercase or `0X` EVM address metadata, and non-lowercase `bscNetwork`,
  `chain`, or `chainIdHex` values are rejected instead of being normalized into
  accepted production metadata. Route-config generation also requires
  production-ready BSC manifests to carry profile-bound `explorerUrl` and
  `explorerHost` metadata, while disabled legacy drafts can be backfilled to
  the selected profile and contradictory explorer aliases still fail closed.
  Release-readiness and bundle verification now pin those BSC route-config
  implementation and exact uppercase-network, `0X` chain-id, uppercase
  post-deploy transaction, and uppercase offline-TOML adversarial-test markers,
  plus post-deploy, full-TOML, source-event transaction, route-canary blocker
  contradiction, scalar, malformed-entry, and explorer-metadata markers, as a
  required source-inventory gate before production evidence can pass.
- SCCP TRON TAIRA XOR route-config generation follows the same canonical
  manifest text policy before TOML rendering. Padded route ids, asset keys,
  network ids, destination rollout network ids, post-deploy transaction ids,
  and offline TOML hashes, plus uppercase bytes32 metadata, are rejected as
  malformed manifest input rather than being normalized into accepted route
  metadata. Non-lowercase `tronNetwork`, `chain`, and `chainIdHex` manifest
  values are rejected at the same boundary. Release-readiness and bundle
  verification now pin those TRON route-config implementation and
  adversarial-test markers as a required source-inventory gate before
  production evidence can pass.
- SCCP active-launch required-record metadata must stay exact: release notes
  cannot report the active required-records item ready unless the normalized
  lane summary is domain `1`, chain `eth`, production-ready, and each required
  record flag is boolean `true` with no unknown record fields. Stringified
  domain ids, padded chain labels, and stringified production-ready flags are
  pinned as adversarial blockers in both readiness and strict bundle tests.
  Required record flags must also reject copied truthy strings, numeric values,
  `false`, and missing/null values in both readiness and strict bundle
  recomputation.
  Unknown required-record summary keys must be schema-classified before checklist text
  is rendered, preserving safe operator diagnostics while padded,
  control-character, whitespace, Markdown-unsafe, malformed, or
  Unicode-confusable keys become category-only blockers.
- SCCP active-launch unresolved-blocker metadata must stay lane-local:
  release notes cannot report the no-unresolved-blockers item ready if the
  active lane carries lane-local blockers, malformed blocker containers, or
  non-string/empty blocker entries even when the top-level aggregate blocker
  list is missing those entries.
  The governed-deployment, route-allowlist, and live-route-canary checklist
  buckets must also fail closed on malformed active-lane blocker containers
  before category matching runs, so scalar, padded, or non-string entries cannot
  disappear from category readiness while only the aggregate blocker gate fails.
  The active no-unresolved-blockers collector must apply the same canonical
  string policy to embedded evidence root blockers and active-lane blockers, so
  empty, padded, or non-string entries remain schema diagnostics rather than
  unstructured blocker text. Numeric and null blocker entries are pinned in
  readiness and strict bundle inventory checks for the active no-unresolved
  blocker collector.
- SCCP release-bundle public string-list fields must reject trim-normalized
  evidence. Manifest, readiness-report, corridor, release-checklist, embedded
  evidence, standalone all-lanes, and lane blocker arrays must contain
  non-empty strings with no surrounding whitespace, so padded blocker text or
  helper-symbol names cannot pass schema validation by being trimmed later.
- SCCP release-bundle public scalar string fields must follow the same
  canonical-text rule. Release-checklist ids/titles, cryptographic-evidence
  chain and route-canary source labels, user-prover submission surface text,
  all-lanes lane chain labels, destination-binding keys, and route-canary
  status/source fields must reject surrounding whitespace instead of relying on
  later normalization. The scalar-text schema must stay pinned as a readiness
  source-inventory gate before published bundle readiness can pass. Exact
  padded-value regressions for release-checklist titles, all-lanes chain labels,
  destination-binding keys, route-canary status/source fields, cryptographic
  route-canary source labels, and submission-surface text are now source-inventory
  markers. Sparse inventory checks now remove the direct copied scalar
  field-type, padded value, malformed field-name, malformed phase-key, copied
  corridor phase-map, copied crypto-evidence, copied submission-surface, and
  top-level CLI redaction regressions directly.
  Release-checklist item ids must also stay in the fixed public gate set and
  classify malformed ids before duplicate, drift, or Markdown-presence checks.
  Release-checklist root and item unknown fields must also use structured
  malformed-name diagnostics, preserving safe operator field names while
  blocking raw malformed public key echoes. The bundle builder must classify
	  malformed copied release-checklist root/item unknown fields before render,
	  reject unknown release-checklist root or item fields, malformed item ids/titles,
	  duplicate item ids, non-exact ready booleans, and noncanonical or non-empty
	  ready-item blockers before `--allow-not-ready` diagnostics can render or write
	  public artifacts. Copied readiness-report release checklists must also
	  recompute from embedded all-lanes evidence plus native prover status before
	  rendering, so syntactically valid item drift cannot publish before strict
	  verification.
- SCCP release-bundle public blocker-list schemas must stay pinned as a
  readiness source-inventory gate: manifest, readiness-report, corridor,
  release-checklist, embedded evidence, standalone all-lanes, and lane blocker
  arrays must keep canonical non-empty strings, duplicate rejection, ready-surface
  empty-blocker checks, and invalid-marker rendering for malformed blocker
  containers before published bundle readiness can pass. The bundle builder must
  reject malformed, empty, numeric, null, padded, or duplicate root blockers before
  `--allow-not-ready` diagnostics can render or write public artifacts. Sparse
  inventory checks now remove root blocker, copied-corridor blocker,
  padded/duplicate blocker, active-lane blocker, all-lanes root blocker,
  release-note invalid-marker, readiness Markdown invalid-marker, and native
  prover blocker regressions directly.
- SCCP release-bundle input provenance must stay pinned as a readiness
  source-inventory gate: copied evidence inputs must use canonical bundle paths,
  unique `inputs` and `input_artifacts`, the `evidence/NN-*.toml` layout, and
  verifier recomputation from copied TOML before published bundle readiness can
  pass. The bundle builder must also require and validate copied report
  `inputs` before rendering release notes or writing public readiness artifacts,
  rejecting empty/malformed input lists, duplicate input paths, escaped or
  noncanonical paths, copied-layout drift, duplicate input artifact paths, and
  `inputs`/`input_artifacts` mismatches. Padded copied paths and
  percent-encoded traversal in both `inputs` and `input_artifacts` are pinned as
  pre-render blockers without raw path leakage. Sparse inventory checks now
  remove missing-input, malformed copied provenance, input path drift,
  provenance schema drift, report-artifact path drift, copied layout drift,
  no-usable-input, and secret path-redaction regressions directly.
- SCCP release-bundle public JSON roots must stay pinned as a readiness
  source-inventory gate: manifest, readiness-report, and all-lanes JSON roots
  must keep canonical serialization, duplicate-key rejection, and non-UTF-8
  fail-closed diagnostics before published bundle readiness can pass. Strict
  verifier source-inventory read and UTF-8 decode failures must also stay
  category-only without echoing local source paths or OS/decoder exception
  payloads. The bundle
  builder must classify unknown readiness-report root field names and reject
  unknown readiness-report root fields before `--allow-not-ready` diagnostics
  can render or write public JSON artifacts; safe operator notes may remain
  readable while padded, control-character, whitespace, Markdown-unsafe,
  malformed, or Unicode-confusable root claims must be category-only blockers.
  Readiness-report `source_inventory` gate names and known-gate row fields must
  be schema-classified before unknown-gate or unknown-field diagnostics, so safe
  operator notes remain readable while padded, control-character, whitespace,
  Markdown-unsafe, malformed, or Unicode-confusable public keys are never echoed
  raw. The bundle builder must also classify copied source-inventory row field
  names and reject malformed, padded, control-character, Markdown-unsafe,
  non-ASCII, or otherwise unknown copied source-inventory gate names, unknown row
  fields, non-passed validation status, and noncanonical or non-empty row blockers
  before `--allow-not-ready` diagnostics can render or write public artifacts;
  the source-inventory marker set must pin that copied blocker rejection
  explicitly. Readiness-report and strict bundle sparse inventory checks now
  remove the copied-row status and blocker-shape markers so deleting those
  pre-render guards blocks public readiness.
- SCCP release-bundle public Markdown roots must stay pinned as a readiness
  source-inventory gate: readiness Markdown and release-note attachments must
  keep UTF-8 loading plus canonical text drift rejection before published
  bundle readiness can pass. Strict verifier and release-bundle builder
  load/render/recompute diagnostics for those public surfaces must remain
  category-only, including missing files, readiness Markdown rendering,
  release-notes attachment rendering, release-checklist recomputation, native
  prover summary recomputation, and user-prover submission-surface
  recomputation failures. The bundle builder must validate in-memory readiness
  Markdown against the verifier-owned
  invariants and canonical render before writing `sccp-release-readiness.md`,
  so a weakened report renderer cannot publish drift before final bundle
  verification. The source-inventory marker set must explicitly pin the bundle
  builder's pre-write readiness Markdown and release-notes attachment drift
  rejections and the tests that assert no drifted public Markdown file is
  written. Readiness-report and strict bundle sparse inventory checks now remove
  both public Markdown drift strings and both pre-write regression test markers
  directly. Readiness Markdown
  source-inventory blocker checks must suppress malformed source-inventory gate
  names before emitting secondary missing-cell diagnostics, and copied
  input/corridor report-artifact paths must pass path classification before
  Markdown path/hash presence checks.
  Cryptographic-evidence row domains and source-adapter audit keys must also be
  schema-classified before stale Markdown presence checks can mention row or
  audit labels.
- SCCP release-bundle public cryptographic-evidence binding must stay pinned as
  a readiness source-inventory gate: production-domain row inventory,
  lane-field binding, canonical row recomputation, and active route-canary
  binding rejection must remain required before published bundle readiness can
  pass. Active-row `source_adapter_gate_audit_hashes` keys must be
  schema-classified before semantic hash or unexpected-field checks so control
  characters, whitespace, Markdown-unsafe characters, and Unicode confusables
  cannot leak through raw public diagnostics. Malformed cryptographic-evidence
  row domains and audit keys must also be suppressed before Markdown
  row/audit-presence diagnostics. The bundle builder must reject unknown or
  malformed cryptographic-evidence row fields, row scalars, audit-hash entries,
  boolean/null drift, optional bytes32 and block-number drift, duplicate
  domains, row/lane count drift, and copied row-to-embedded-lane
  binding drift before `--allow-not-ready` diagnostics can render or write
  public artifacts. EVM route-canary public rows must also require exact
  `evm_message_proof_accepted_transaction` evidence source plus exact `true`
  evidence-bound and finalized-receipt booleans, and positive u32 receipt block
  numbers, before public Markdown can be rendered or strict bundle verification
  can pass. TRON route-canary public rows must also keep block numbers as
  positive u64 integers and block timestamps as non-negative u64 integers before
  that same public-output boundary. Public source-adapter gate rows must also
  enforce domain-specific audit-key policy for Solana, TON, and TRON rows before
  non-active copied evidence can pass. The inventory self-tests must also
  sparse-check the copied
  cryptographic-evidence confusable audit-key non-leak marker so adversarial
  audit-key suppression remains part of the release gate.
- SCCP release-bundle public submission-surface binding must stay pinned as a
  readiness source-inventory gate: lane/backend inventory, per-SDK helper
  inventory, verifier-owned surface recomputation, and corridor-phase binding
  must remain required before published bundle readiness can pass. Public
  `user_prover_submission_surfaces[].lanes` labels must be schema-classified
  before lane inventory, backend, helper, or Markdown-presence checks, so
  padded, control-character, whitespace, Markdown-unsafe, malformed, or
  non-ASCII/confusable lane labels cannot leak raw public diagnostics.
  `user_prover_submission_surfaces[].proof_backend` values must use the same
  classification before backend mismatch or Markdown-presence checks, preserving
  readable safe unknown backend diagnostics while suppressing hostile backend
  ids.
  `user_prover_submission_surfaces[].on_chain_submission` text must match the
  verifier-owned lane submission text before Markdown-presence checks, so copied
  operator text or hostile submission labels cannot leak raw public diagnostics.
  Default and per-SDK helper symbols must be schema-classified before helper
  string derivation, helper inventory, UI-hook matching, or Markdown-presence
  checks, so table-breaking or confusable helper names become category-only
  blockers. If a copied report corrupts the per-SDK helper map or any helper
  entry, readiness Markdown must render an invalid-marker cell instead of
  falling back to raw `sdk_helpers` text or rendering the raw helper.
  Public `sdk_helper_symbols_by_sdk` map keys must be schema-classified before
  unknown-SDK, helper-list, or Markdown-presence diagnostics, so malformed
  padded, control-character, whitespace, Markdown-unsafe, and
  non-ASCII/confusable SDK keys cannot leak raw public diagnostics.
  `required_phases` values use the same classification before unknown-phase,
  duplicate, missing-phase, contract-smoke, or Markdown-presence checks, while
  canonical safe unknown phases remain readable operator diagnostics. The strict
  verifier and bundle builder must also compare required phases to the
  verifier-owned lane policy exactly, so reordered or extra known phases such as
  a non-EVM `dotnet-sdk` cannot satisfy public portal/mobile rows through a
  generic row recomputation check. The bundle builder must reject unknown or
  malformed submission-surface row fields,
  scalar text, helper-symbol lists, per-SDK helper maps, SDK keys, required
  phase values, validation status, and validation blockers before
  `--allow-not-ready` diagnostics can render or write public artifacts.
  Copied submission rows with `validation_status = blocked` or non-empty
  validation blockers are now rejected directly before Markdown or JSON output is
  written, even when the row shape is otherwise canonical; sparse inventory tests
  must keep the blocked copied-row pre-render regression, malformed
  validation-status marker, validation-status/blocker coupling marker, and
  blocker marker pinned. Sparse inventory tests must also keep copied
  submission-surface
  confusable SDK-key suppression pinned so hostile SDK/backend labels remain
  category-only diagnostics before readiness can pass.
  Unknown submission-surface row fields use the same structured field-name
  classification in the verifier and release-bundle builder before render, so
  valid operator notes stay readable while padded, control-character,
  whitespace, Markdown-unsafe, malformed, or Unicode-confusable field names
  never leak raw public diagnostics. Copied submission-surface rows must also
  recompute from copied corridor phases and reject duplicate/unknown/missing lane
  rows, backend drift, missing required SDK helpers, and blocked validation rows
  before public output is written.
- SCCP native EVM prover validation blockers must stay schema-aware in both
  readiness generation and release-bundle verification: scalar blocker
  containers, non-string entries, empty strings, and padded strings must become
  explicit readiness blockers rather than being filtered, character-expanded, or
  silently treated as ready. Sparse inventory tests must pin the malformed
  native-blocker regressions and no-character-expansion assertions in both
  readiness generation and strict bundle verification.
  Copied release-bundle native prover summaries with `validation_status =
  blocked` or non-empty validation blockers are now rejected directly before
  public Markdown or JSON output is written, even when the summary shape is
  otherwise minimal and canonical; sparse inventory tests must keep the blocked
  copied-summary pre-render regression and blocker marker pinned, and strict
  bundle inventory now removes that marker directly to prove the gate fails.
  Native EVM prover SDK artifact ids must follow the same canonical text policy:
  whitespace-padded, control-character, internal-whitespace,
  non-ASCII/confusable, and malformed lowercase-id SDK ids are rejected as
  malformed rows instead of being treated as unknown SDK names or hidden
  missing-SDK evidence.
  Published readiness-report `native_evm_prover_bundle.sdk_artifacts[].sdk`
  rows enforce the same canonical SDK-id policy so tampered report JSON cannot
  downgrade malformed ids into raw unknown-SDK diagnostics.
  SDK artifact row schemas also reject malformed unknown field names with the
  same structured policy in readiness generation, bundled native-manifest
  verification, and published readiness-report summary verification.
  Native prover parity and self-test fixture `sdk_results` keys follow the same
  policy in readiness generation and strict bundle verification, so padded SDK
  result keys surface as schema blockers before fixture rows can be treated as
  unknown or missing SDK evidence.
  Control-character, internal-whitespace, non-ASCII/confusable, and malformed
  lowercase-id SDK result keys are also rejected as malformed keys before
  unknown-SDK classification, keeping public diagnostics structured and
  release-bundle fixture reviews fail-closed.
  Native EVM prover bundle generators must also keep route/deployment JSON and
  every cryptographic or SDK artifact input regular-file-only, with symlinks and
  out-of-root realpaths rejected before hashing or attaching production route
  manifest material. Release builder, readiness-report, and verifier path-text
  gates now reject raw, percent-encoded, and recursively over-encoded
  parent-directory segments before generated artifacts, manifest rows,
  report-provenance paths, extracted bundle entries, or native prover payload
  paths are published for browser runtime consumption; keep the inventory gate
  pinned as regression coverage, including sparse checks for percent-encoded
  native payload regressions in readiness generation and strict bundle
  verification. Native prover payload artifact path metadata
  failures now render fixed blockers in readiness generation and strict bundle
  verification, so local path-validation details cannot leak into published
  native prover diagnostics.
  Portal/mobile runtime SDK selectors for direct byte verification,
  resolver-backed bundle loading, and native prover self-test preflights must
  follow the same canonical text policy before SDK artifact lookup or callbacks
  run. Native no-WASM/no-remote source inventory must pin those padded-SDK
  negative tests across JavaScript, Kotlin/JVM, Java Android, and Swift before
  native prover evidence can pass, and sparse inventory self-tests must cover the
  Kotlin/JVM and Java Android padded self-test callback non-run markers directly.
  Sparse inventory checks must also remove the browser no-WASM guard, BSC
  browser guard, URI proof-artifact, WASM proof-artifact, and remote-prover
  identifier markers from the JavaScript package distribution test directly.
- SCCP readiness cryptographic-evidence rows must preserve raw boolean and
  container values from the normalized lane summary. Truthy strings, numeric
  gate hashes, or malformed source-gate audit containers must remain visible to
  release-bundle schema checks instead of being coerced into ready-looking
  values. Missing future-lane route-canary bindings must render as explicit
  boolean `false`, while present malformed binding values remain preserved for
  verifier rejection. Source inventory must sparse-check the readiness-side
  malformed audit-container preservation assertion before public
  cryptographic-evidence readiness can pass.
- SCCP all-lanes release checklist source-adapter gates must use exact boolean
  semantics: malformed `required` or `ready` fields must produce governed
  deployment blockers rather than clearing through truthiness, and manifest
  comparisons against recomputed active launch readiness must use exact values.
  The all-lanes evidence-root schema is release-critical: malformed roots,
  unknown sections and their literal blocker assertions, and non-string section
  keys must remain structured blockers and are now pinned in release-readiness
  and strict bundle source inventories.
  The strict release-bundle verifier must also invoke that root-schema
  source-marker sweep directly, so missing implementation or adversarial-test
  markers cannot be hidden behind a present `source_inventory` row.
  Copied active-lane evidence must also keep destination-binding and
  route-allowlist expected-hash pins semantic before public bundle rendering:
  expected hashes must equal their governed hashes, match flags must be exact
  `true`, and destination binding recomputation must remain exact `true`.
  Source-adapter gate hash/audit replay regressions are part of the required
  release source inventory, so deleting the direct replay tests blocks readiness
  and strict release-bundle verification.
  Source-adapter gate `blockers` containers must also stay schema-aware:
  scalar, empty, padded, or non-string entries become explicit governed
  deployment blockers, while valid gate blockers remain visible instead of
  being filtered or expanded character-by-character. Direct checklist and
  generated-summary empty-entry regressions are pinned in the release source
  inventory.
  Lane-local blocker containers must use the same canonical string policy in
  the all-lanes checklist: scalar, padded, or non-string entries become live
  route-canary and unresolved-blocker diagnostics, while valid route-canary
  blockers remain visible in both buckets.
  Route-canary summary scalars in the all-lanes checklist must also stay
  canonical: padded or non-string `status` and `evidence_source` values become
  schema blockers before the checklist compares them with the expected passed
  status or lane-specific evidence source. Release-readiness and bundle
  verification pin that all-lanes route-canary scalar schema as a required
  source-inventory gate before production evidence can pass; strict
  release-bundle inventory tests must keep the adversarial numeric/padded
  `status` and `evidence_source` test markers pinned.
  The standalone readiness report must also require the active launch checklist
  `ready` value to be exactly boolean `true` before top-level
  `production_ready` can become true. Malformed lane record, destination-binding,
  route-allowlist, route-canary, or lane-local blocker containers must likewise
  become explicit checklist blockers rather than tracebacks, hidden route-canary
  gaps, or falsely ready no-unresolved-blockers state. Release-readiness and
  bundle verification now pin that active checklist schema as a required
  source-inventory gate before production evidence can pass.
- SCCP all-lanes governed evidence blockers must stay schema-aware: destination
  rollout and route allowlist `blockers` fields must be empty lists of
  non-empty canonical strings, and scalar, empty, padded, or non-string entries
  must remain production blockers instead of being collapsed into generic
  not-ready state. Release-readiness and bundle verification now pin that
  governed blocker schema as a required source-inventory gate before governed
  evidence can pass; strict release-bundle inventory tests must keep the padded
  route-allowlist blocker adversarial marker pinned.
- SCCP active-launch governed-deployment readiness metadata must stay
  canonical: release notes cannot report the governed deployment ready unless
  the normalized source-material, source-deployment, destination-binding, and
  recomputed expected destination-binding hashes are canonical non-zero bytes32
  values, the supplied binding hash matches the expected value, the expected
  match flag is exact boolean `true`, and source-material/source-deployment
  record hashes remain role-separated. The active EVM source-adapter gate
  summary must remain explicitly not required with empty gate metadata.
  Readiness and strict bundle source-inventory tests must keep the exact flag,
  role-reuse, required, gate-hash, and audit-hash source-adapter gate blockers
  pinned.
- SCCP active-launch route-allowlist readiness metadata must stay canonical:
  release notes cannot report the launch route binding ready unless the
  normalized source-material, source-deployment, destination-binding,
  route-allowlist, and recomputed expected route-allowlist hashes are canonical
  non-zero bytes32 values and the route hash matches the expected binding tuple.
  The expected-match flag must be exactly boolean `true`, and the strict bundle
  verifier must reject source verifier material/source-adapter deployment hash
  role reuse for the route-allowlist item just as the readiness generator does.
  Source-inventory tests must keep the recomputed route-hash mismatch,
  exact expected-match-flag, source-record role-reuse, and adversarial
  `route_allowlist.hash_mismatch` markers pinned.
- SCCP active-launch route-canary readiness metadata must stay canonical:
  release notes cannot report the launch lane ready unless the EVM
  `MessageProofAccepted` evidence source, non-zero transaction hash, finalized
  receipt block number/hash, receipts root, and message id are present in the
  normalized route-canary summary.
  The route-canary evidence source must first be a non-empty canonical string:
  missing, non-string, empty, or whitespace-padded values are release blockers
  before the exact `evm_message_proof_accepted_transaction` source match runs.
  Canonical-looking wrong source labels, including case drift or operator notes,
  remain live-route-canary blockers in the readiness generator and strict bundle
  recomputation.
  Route-canary `status` must also be exactly `passed`; missing, empty, padded,
  or non-string status values remain live-route-canary blockers in readiness and
  strict bundle recomputation.
  Route-canary evidence hash, transaction hash, receipt block hash, block
  receipts root, and message id must all be canonical lowercase non-zero `0x`
  bytes32 strings; missing, zero, uppercase, or non-string values remain
  live-route-canary blockers in readiness and strict bundle recomputation.
  Route-canary receipt block numbers must also stay exact positive integers:
  numeric-looking strings, hex text, plus-signed text, Unicode-confusable text,
  and booleans are release blockers, and `receipt_block_finalized` must be
  exactly boolean `true`, not false, missing/null, truthy text, or numeric
  values. `evidence_bound` must also be exact boolean `true`;
  copied truthy strings, numeric values, false, and missing/null flags must
  remain live-route-canary blockers in readiness and strict bundle
  recomputation.
  Source-inventory tests must also keep the transaction hash, receipt-block
  hash/root, message-id, positive block-number, finalized-block, and adversarial
  block-receipts-root markers pinned.
- SCCP release readiness now treats Ethereum noncanonical chain-id coverage as
  a production gate: public SDK and evidence-script regressions must continue
  rejecting padded, uppercase, numeric, and whitespace-wrapped `eth_chainId`
  values before local source-proof evidence is accepted. The Python receipt-proof
  evidence test vector is source-inventory pinned alongside the public SDK
  vectors, and Swift, Kotlin/JVM, Java Android, and C# must keep the same
  uppercase/whitespace/numeric vector markers in the release source inventory.
- SCCP release readiness now treats Ethereum inbound adversarial coverage as a
  production gate: public SDK regressions must continue rejecting failed
  receipts, source-event drift, hash-only proof bypasses, mutable evidence
  aliases, oversized proof bytes, finality mismatches, weak sync-committee
  evidence, and wrong-domain receipt transcripts before inbound source proofs
  are accepted. The readiness inventory now removes representative Ethereum
  inbound markers directly across JavaScript, Python implementation/tests,
  Swift, Kotlin/JVM, Java Android, and C# so the generator gate cannot pass with
  only one SDK's adversarial coverage intact. The Python implementation/test row
  must keep the canonical ETH receipt-proof transcript rejection for BSC
  `sourceDomain` values.
- SCCP release readiness now treats BSC inbound adversarial coverage as a
  production gate: public SDK regressions must continue rejecting hash-only
  proof bypasses, receipt-proof metadata drift, source-event digest drift,
  malformed source logs, and missing source-event validation before BSC inbound
  source proofs are accepted. The readiness inventory now removes representative
  BSC inbound markers directly across JavaScript, Python, Kotlin/JVM, Swift,
  Java Android, and C# so the generator gate cannot pass with only one SDK's
  adversarial coverage intact. The Python row must keep the canonical BSC
  receipt-proof transcript rejection for ETH `sourceDomain` values.
- SCCP TRON TAIRA XOR route-config generation now rejects production-ready
  route manifests that still carry `disabledReason` or `disabled_reason`, and
  rejects contradictory disabled-reason aliases before a governed Torii overlay
  can advertise a route as live.
- SCCP TRON TAIRA XOR route-config generation now requires production-ready
  manifests to carry post-deploy live evidence, `fullTomlReady: true`, and the
  offline full-TOML SHA-256 before a governed Torii overlay can advertise a
  route as live.
- SCCP TRON TAIRA XOR route-config generation now rejects malformed or foreign
  route manifests before overlay rendering: route id, asset key,
  counterparty-domain, verifier target, TRON profile, chain id, and network id
  must match the governed TAIRA XOR TRON lane, and production-ready overlays
  remain mainnet-only.
- SCCP TRON TAIRA XOR route-config generation now recomputes destination
  binding keys and hashes from the declared network, verifier address,
  verifier code hash, and verifier key hash, then rejects rollout or
  `destinationBinding` drift before any governed Torii overlay is emitted.
- SCCP TRON TAIRA XOR route-config generation now rejects stale manifest
  payloads whose destination verifier backend/proof family, contract-address
  uniqueness, TAIRA burn-record artifact digest, or settlement route/asset
  metadata drift from the governed TAIRA XOR lane before overlay rendering.
- SCCP TRON TAIRA XOR route-config generation now rejects destination-network,
  verifier-identity, root destination-verifier alias, settlement submit path,
  and settlement mode drift before a hand-edited manifest can produce a
  governed Torii overlay.
- SCCP TRON TAIRA XOR route-config generation now pins route, rollout, and
  destination-binding schema versions to v1, requires canonical burn-record
  code hashes, and rejects malformed burn-record VK references before overlay
  rendering.
- SCCP TRON TAIRA XOR route-config generation now requires production-ready
  manifests to retain the post-deploy live-readback acknowledgement, and rejects
  missing, false, or contradictory camel/snake readback markers before overlay
  rendering.
- SCCP TRON route manifests parsed from runtime configuration now normalize
  core/post-deploy hash fields as non-zero bytes32 values and require
  production-ready TRON routes to carry full post-deploy evidence anchors before
  the route can enter the node config.
- SCCP TRON route manifests parsed from runtime configuration now also reject
  production-ready TAIRA XOR TRON metadata drift: route id, counterparty domain,
  asset key, TRON network, chain key, chain id, and verifier target must remain
  pinned to the governed mainnet lane before the route can enter the node config.
- SCCP TRON route manifests parsed from runtime configuration now also recompute
  the dynamic TRON destination-binding key from normalized mainnet network id,
  destination verifier address, verifier code hash, and verifier key hash, and
  reject TAIRA burn-record settlement asset, verifier-key backend/name, or gas
  limit drift before a production-ready route can enter the node config.
- SCCP TRON route manifests parsed from runtime configuration now also require
  token, bridge, source bridge, and destination verifier literals to be
  canonical non-zero TRON Base58Check mainnet addresses and reject duplicate
  contract-role addresses before the route can enter the node config.
- SCCP TRON route manifests parsed from runtime configuration must also reject
  production-ready post-deploy blocker drift: scalar, empty, padded, or
  non-string `postDeployLiveEvidence` blocker containers fail closed, and
  non-empty blocker lists cannot coexist with `productionReady: true`.
- Release-readiness and bundle verification now pin the runtime TRON
  route-manifest parser and adversarial parser tests as a required
  source-inventory gate before runtime config evidence can pass.
- SCCP TRON TAIRA XOR route-manifest generation must reject contradictory
  source-event transaction readiness: production-ready live evidence cannot carry
  non-empty `source_event_transaction_production_blockers`, and malformed blocker
  containers must fail closed before a production-ready route manifest can be
  rendered. Release-readiness and bundle verification must pin the implementation
  aliases plus the production-ready, scalar, and malformed-entry adversarial
  route-config tests before governed TRON overlays can satisfy readiness.
- SCCP release readiness now treats corridor phase-evidence source handling as
  a production gate: readiness-report and release-bundle CLI regressions must
  continue rejecting duplicate phase evidence assignments and directory
  override collisions before corridor evidence can satisfy production
  readiness. Empty, control-character, non-ASCII, and embedded-whitespace
  phase-result status regressions are source-inventory pinned so those
  category-only checks cannot be removed without failing release verification.
- SCCP release readiness now treats corridor phase-transcript semantics as a
  production gate: readiness-report and release-bundle regressions must keep
  exact phase markers, phase-specific traced command shapes with
  exact pytest positional inputs, option-bound test/filter selectors, exact
  Gradle test command parsing, broad Kotlin package-suite selectors, exact
  Swift filter commands, exact Android harness class membership, positional
  contract-smoke Node test/check commands, exact .NET project/filter/nologo
  commands, exact no-suffix
  cargo/bash/java commands, no bare-fragment shortcuts, no
  shell-comment-hidden command fragments, and only the runner's `cd <dir> &&`
  wrapper tolerated, observed
  non-negated/non-diagnostic shell-xtrace-free
	  phase-local ordered completion/success output after required commands in
	  per-phase and full-corridor logs after terminal-control normalization, dry-run
	  rejection, terminal-control/Unicode-format-normalized failure marker scans,
	  and forged-block rejection pinned before corridor logs can satisfy public
	  bundle readiness. The release bundle builder also runs those verifier-owned
	  transcript checks against copied phase artifacts before Markdown rendering or
	  public JSON writes, so dry-run, missing, unreadable, or forged copied phase
	  logs cannot publish before final bundle verification.
- SCCP release readiness now treats release bundle source-copy preflights as a
  production gate: bundle CLI regressions must continue rejecting symlinked or
  control-character evidence inputs, duplicate evidence input sources including
  canonical path aliases, phase evidence, native EVM prover manifests, and
  native prover payload sources before bundle copy can run. Duplicate evidence
  input diagnostics must redact local paths as `<path> duplicates <path>`.
  Control-character source-path diagnostics must report the offending
  control-byte label without local path text.
  Symlinked source-path and source-ancestor diagnostics must stay category-only
  for evidence inputs, phase logs, native prover manifests, and native prover
  payload sources.
  Markdown-unsafe copied source filename diagnostics must stay category-only
  before source copying.
  Percent-encoded traversal in copied source filenames must fail before source
  copying with category-only diagnostics.
- SCCP release readiness now treats release bundle output-path preflights as a
  production gate: bundle CLI regressions must continue rejecting symlinked
  output directories, symlinked output ancestors, and control-character output
  directory paths before bundle generation can create or overwrite release
  artifacts. Symlinked output-directory and output-ancestor diagnostics must
  stay category-only so local release target paths do not leak.
  Forced-replacement containment diagnostics must also stay category-only so
  local output roots and protected evidence paths do not leak.
  Existing-output diagnostics without `--force` must stay category-only too.
  Dangerous-root and repository-containing output diagnostics must also avoid
  printing local output paths.
  Control-character output-path diagnostics must likewise keep local release
  target paths out of stderr.
- SCCP release readiness now treats release artifact path text preflights as a
  production gate: bundle and readiness regressions must continue rejecting
  Markdown-unsafe or surrounding-whitespace artifact paths, native prover
  payload paths, copied filenames, readiness input paths, manifest paths, and
  bundle filesystem entries before release notes can render artifact tables.
  Generated release artifact path diagnostics must remain category-only and
  must not echo local artifact paths.
  Top-level all-lanes, release-readiness, and release-bundle CLI exception
  handlers must preserve structured validation categories while redacting
  secret-looking, control-character, empty, and OS-error payloads before stderr.
  Native prover role-reuse diagnostics, copied artifact-integrity diagnostics,
  manifest/report artifact membership diagnostics, and release-notes attachment
  artifact-list diagnostics must stay category-only for untrusted artifact path
  text. Manifest artifact-row validation, extracted bundle-entry validation,
  duplicate/unmanifested bundle entry diagnostics, and manifested artifact
  symlink checks must also stay category-only.
  Readiness-report input and input-artifact provenance diagnostics, including
  copied-input recomputation failures, must stay category-only for duplicate,
  escaping, layout, control-character, Markdown-unsafe, padded, and
  percent-encoded path drift so untrusted JSON path values are never echoed.
  Native EVM prover manifest-relative payload path diagnostics must also stay
  category-only for control-character and Markdown-unsafe path drift in bundle,
  readiness, and strict-verifier paths. Missing, non-regular, unreadable, or
  forbidden-marker-scan-failed native prover payload diagnostics must not echo
  the operator-supplied manifest-relative path or local exception text. Native
  prover manifest, cross-SDK parity fixture, and native self-test fixture JSON
  load/parse diagnostics must also stay category-only and avoid parser
  exception payloads.
  All-lanes source-record hash, source-gate/config hash, destination-binding
  hash, and route-allowlist recomputation failures must stay category-only in
  public all-lanes/release-readiness blockers and must not append helper
  exception text. Canonical source-validator and destination verifier identity
  parser failures must follow the same category-only rule, including TRON
  source-bridge, TRON destination-verifier, and EVM source/destination runtime
  bytecode metadata parser failures, plus Solana ProgramData account,
  executable, route-canary live ProgramData, TON code BoC, and TON
  route-canary verifier identity parser failures, plus TRON route-canary
  verifier-address parser failures. The TON live evidence helper must apply
  the same category-only rule to live accountStates address, live `code_boc`,
  and imported `code_boc_base64` parser failures before rendering governed
  TOML.
- SCCP release-bundle manifest readiness flags must preserve exact report
  booleans: bundle generation must not truthy-coerce malformed
  `production_ready`, `release_checklist.ready`, or corridor readiness values
  into public manifest `true` claims. Release-note status rendering, bundle
  preflight publication checks, verifier not-ready checks, and generated-bundle
  self-verification should only treat report `production_ready is True` as
  ready. Readiness Markdown row rendering must likewise use exact booleans for
  checklist items, lane production status, lane record flags, route-canary
  binding labels, and native-prover required labels, and must mark malformed
  top-level readiness, release-note, release-bundle preflight, native-prover,
	  source-inventory, and user-prover blocker containers as invalid cells/items
	  instead of flattening strings or raising during verifier-owned Markdown
	  generation. Embedded readiness evidence and standalone all-lanes root
	  blocker summaries plus active-lane blocker containers must also be list-shaped
	  before active-launch blocker collection runs, so malformed strings cannot
	  become character-by-character blockers or disappear from verifier checks.
	  Release-bundle generation must validate the structure of both the initial
	  preflight report and the copied-evidence bundle-local report before Markdown
	  rendering or manifest creation, so malformed report objects fail with explicit
	  preflight diagnostics instead of uncaught indexing exceptions.
	  Verifier-owned Markdown invariants must independently require
	  checklist, lane, native-prover, source-inventory, user-prover, and top-level
	  blocker text or invalid-marker cells/items so a hand-edited attachment cannot
	  hide readiness blockers while preserving the surrounding table structure.
  Release-readiness and bundle verification now pin those public Markdown
  invariants as a required source-inventory gate before public bundle readiness
  can pass; sparse inventory checks now remove the direct public-section,
  blocker-text, invalid-marker, and malformed-label redaction regression tests
  so deleting those tests blocks readiness.
  Release-notes attachment invariants must likewise require the canonical
  title, exact readiness status line, manifest handoff, artifact table entries,
  and blocker lines or invalid-marker bullets before the canonical attachment
  comparison runs. The release bundle builder must validate the in-memory
  release-notes attachment with those verifier-owned invariants and canonical
  rendering before writing `sccp-release-notes-attachment.md`, so release-manager
  note injection or table drift cannot publish before final bundle verification.
  Release-readiness and bundle verification now pin those release-notes
  attachment invariants as a required source-inventory gate before public
  bundle readiness can pass, including sparse checks for status, blocker,
  malformed-blocker, and exact-ready comparison regressions.
		  Release-readiness and bundle verification now pin exact manifest readiness
		  flag generation, boolean rejection, manifest/report equality, and all-lanes
		  readiness recomputation as a required source-inventory gate before published
		  bundle readiness can pass. The release bundle builder must validate the
		  in-memory manifest against those readiness flags before writing
		  `manifest.json`, so readiness-flag drift cannot publish before final bundle
		  verification. Sparse inventory checks now remove the malformed readiness
		  value, boolean-type drift, manifest-claim drift, pre-write manifest
		  drift, and summary launch-ready regression tests directly.
		  Release-readiness and bundle verification now also pin required artifact
		  paths, manifest-root exclusion, unmanifested artifact/directory rejection,
		  report-referenced artifact closure, and canonical attachment order as a
		  required source-inventory gate before published bundle readiness can pass.
		  Sparse inventory checks now remove the direct manifest-root, symlink-root,
		  missing-manifest, duplicate-artifact, unmanifested-entry, unsupported-entry,
		  phase-artifact, extra-artifact, unknown-phase, order-drift, malformed
		  copied artifact, copied-hash drift, and pre-write manifest drift
		  regressions directly.
		  Strict bundle verification must keep root-shape, missing-manifest,
		  unsupported-entry, bundle-enumeration, and unreadable phase-transcript
		  diagnostics category-only so local release paths cannot leak through
		  public verifier output.
		  Public JSON root non-UTF-8, load, parse, and canonical serialization
		  diagnostics must also stay category-only and avoid echoing local bundle
		  paths or parser exception payloads.
		  Strict verifier source-inventory read and UTF-8 decode failures must
		  likewise stay category-only without appending local source paths or
		  OS/decoder exception payloads.
		  Release-readiness source-inventory gate helper failures must stay
		  category-only too, without appending helper exception text or local path
		  payloads to public readiness blockers.
			  The release bundle builder must also validate artifact closure, copied-file
			  artifact rows, and canonical artifact ordering before writing the manifest.
			  The release bundle builder must also validate the generated
			  `sccp-all-lanes-summary.json` payload against the copied report evidence
			  before writing public report or summary artifacts, so summary drift cannot
			  publish before final bundle verification.
		  All-lanes evidence summaries must also use exact booleans for
		  checklist aggregation, record-presence gates, and CLI success exits, and
  require canonical non-zero route-canary evidence hashes plus the expected
  live evidence source for each lane before canary readiness can pass. Malformed
  lane record, destination-binding, route-allowlist, route-canary, or lane-local
  blocker containers must become explicit checklist blockers instead of raising
  during summary rendering or letting no-unresolved-blockers pass. The
  release-readiness report and bundle verifier now pin the all-lanes exact
  checklist aggregation, record-presence gates, CLI production-ready exit, and
  route-canary hash replay rejection as a required source-inventory gate before
  all-lanes evidence can satisfy production readiness. The
  public release-bundle verifier's
  recomputed active launch checklist must mirror the generator's exact required
  record, governed-deployment, route-allowlist, and route-canary metadata
  blockers before comparing manifest readiness against the all-lanes summary.
- SCCP release readiness now treats Ethereum outbound pre-callback coverage as
  a production gate: public SDK regressions must continue rejecting foreign-lane
  outbound requests, forged destination bindings, missing or partial
  proof-artifact hashes, zero proof-artifact hashes, and callback-visible proof
  material before outbound prover callbacks can run. The source inventory also
  pins implementation-side native Groth16 artifact normalization and request
  hash preimage ordering across JavaScript, Python, Swift, Kotlin/JVM, Java
  Android, and C# so proof artifact bytes cannot drift behind public signal
  words without failing readiness and bundle verification.
- SCCP release readiness now treats Ethereum outbound provider validation as a
  production gate: public SDK and facade regressions must continue validating
  app-supplied Ethereum mainnet execution providers before outbound submitter
  callbacks can run. The readiness and bundle-verifier inventory tests now
  remove validate-before-submit markers directly across JavaScript source/dist,
  Python implementation/tests, Swift, Kotlin/JVM, Java Android, and C#.
- SCCP release readiness now treats Ethereum local-admission coverage as a
  production gate: public SDK regressions must continue rejecting mutated proof
  bytes, all-zero proof/public-input/bundle/envelope bytes, empty envelopes,
  zero statement/source-material/source-adapter hashes, and stale proof-family
  metadata before local admission payloads are submitted.
- SCCP release readiness now treats Ethereum receipt-root zero rejection as a
  production gate: public SDK regressions must continue rejecting all-zero typed
  receipt roots before receipt-proof bytes can be built.
- SCCP release readiness now treats Ethereum receipt RLP zero-topic handling as
  a production gate: public SDK and evidence-helper regressions must continue
  preserving zero log topics in generic receipt RLP before SCCP source-event
  ABI filtering runs.
- SCCP release readiness now treats Ethereum receipt RLP zero-address handling
  as a production gate: public SDK and evidence-helper regressions must
  continue preserving zero log addresses in generic receipt RLP before SCCP
  source-event ABI filtering runs.
- SCCP release readiness now treats Ethereum source-event context binding as a
  production gate: receipt-proof evidence regressions must continue binding
  source-event logs to receipt transaction hash, block hash, and block number
  before source-event evidence is accepted.
- SCCP release readiness now treats Ethereum source-event evidence mode as a
  production gate: receipt-proof evidence regressions must continue requiring
  source-bridge validation or an explicit receipt-only mode before receipt
  proof summaries can be emitted. The release-readiness and bundle-verifier
  inventory tests now remove the evidence script's `source_bridge_address`
  fail-closed marker directly, so this gate cannot be satisfied only by the
  Python receipt-only mode regression names.
- SCCP release readiness now treats Ethereum source-event zero-digest rejection
  as a production gate: receipt-proof evidence regressions must continue
  rejecting all-zero source-event digests before source-event evidence is
  accepted. The release-readiness and bundle-verifier inventory tests now remove
  the evidence script's zero-data `RuntimeError` marker directly, so all-zero
  source-event digest rejection cannot be satisfied only by the Python regression
  name.
- SCCP release readiness now treats Ethereum receipt RPC duplicate-JSON
  rejection as a production gate: receipt-proof evidence regressions must
  continue rejecting duplicate JSON-RPC result or receipt keys and redacting
  receipt RPC transport/error details before receipt proof evidence is parsed.
  The inventory tests now remove the
  `object_pairs_hook=_json_object_without_duplicate_keys` parser hook directly,
  so duplicate-key fail-closed parsing cannot disappear while duplicate-key test
  names remain.
- SCCP release readiness now treats Ethereum block receipt transaction-hash
  uniqueness as a production gate: receipt-proof evidence regressions must
  continue rejecting duplicate transaction hashes in block receipt lists before
  receipt trie proofs can be built. The release-readiness and bundle-verifier
  inventory tests now remove the Python evidence-script uniqueness check and the
  JavaScript SDK `seenTransactionHashes` guard directly, so SDK-side uniqueness
  enforcement cannot disappear while the Python-only regression remains.
- SCCP release readiness now treats Ethereum JavaScript receipt admission as a
  production gate: browser proof regressions must continue rejecting receipt
  metadata drift, missing beacon finality, typed receipts, and mutable prover
  callback evidence before local proving can run. The release-readiness
  inventory test now removes the beacon-finality, immutable-callback, and
  browser finality-regression markers directly, matching the bundle verifier's
  marker checks.
- SCCP release readiness now treats Ethereum SDK receipt metadata binding as a
  production gate: public SDK regressions must continue rejecting
  block-receipt metadata drift and typed receipts before receipt proof builders
  can run. The release-readiness and bundle-verifier inventory tests now remove
  JavaScript receipt-RLP binding and Swift canonical receipt-RLP markers
  directly, so cross-SDK metadata validation cannot disappear while a Kotlin-only
  typed-receipt marker remains.
- Keep the direct-Serde migration closed: `scripts/serde_allowlist.txt` is
  empty, and `make guards` keeps new direct `serde_json` usage and retired
  non-Norito codec dependencies, including renamed retired-codec package
  aliases, out of the workspace. `tools/soranet-relay`, `crates/iroha_core`,
  `crates/iroha_cli`, `crates/iroha_torii`, and `crates/iroha_sccp` have been
  removed from that allowlist after their relay JSON paths, STARK/FRI envelope
  types, contract-app TOML manifest decoder, Torii SCCP query DTOs, Torii
  tx-history/push JWT JSON paths, and SCCP public payload/proof DTOs were
  verified to build without direct Serde dependencies.
  The stale workspace-level `bincode` dependency is removed; remaining Solana
  bincode-layout wording refers to hand-validated external protocol bytes, not
  a production codec dependency.
- SCCP release readiness reports and release bundles must keep native EVM
  prover artifact paths role-unique across the attached native prover manifest
  and published readiness summary, so proof artifacts, proving/verifier keys,
  parity fixtures, self-tests, and per-SDK implementation artifacts cannot
  silently reuse another role's file. The release bundle builder rejects that
  path reuse during input validation before copying native prover payloads,
  including not-ready diagnostic bundles.
- SCCP native EVM prover cross-SDK parity fixtures and native self-test
  fixtures must keep semantic digest roles separated inside the fixture body:
  receipt/source proof, request/witness, calldata, Torii payload, and proof
  hashes cannot reuse another role even when manifests and SDK result rows are
  rehashed consistently.
- SCCP native EVM prover manifests must carry exactly one artifact row for each
  required public SDK; duplicate SDK rows and missing SDK rows remain readiness
  and published-bundle blockers even if the native manifest is rehashed.
  Manifest SDK artifact rows must also remain canonical objects with all
  required fields, no unknown fields, approved SDK/implementation bindings, and
  artifact hashes matching their committed roles. SDK implementation artifact
  paths and hashes are checked with the same local-artifact and hash-binding
  rules as the primary prover payloads. Duplicate JSON keys inside SDK artifact
  rows must fail before implementation hashes are trusted.
- SCCP native EVM prover manifests must also keep their root object and
  `audit_hashes` map schemas exact. Rehashed manifests that add unknown root
  fields, add unexpected audit-hash roles, or omit required audit-hash roles are
  readiness and published-bundle blockers. Duplicate JSON keys at the manifest
  root or inside `audit_hashes` must fail before any last-key-wins parser can
  trust overwritten audit evidence, and those negative cases remain pinned by
  the native no-WASM/no-remote readiness inventory.
  Unknown root fields and unexpected audit roles with surrounding whitespace,
  internal whitespace, control characters, Markdown-unsafe characters, or
  non-ASCII/confusable spellings are rejected with structured malformed
  field-name blockers instead of echoing operator-controlled names.
  Unexpected audit roles must not enter later semantic hash checks or published
  audit summaries; required audit roles alone are checked for canonical hashes,
  duplicate role reuse, and payload artifact binding.
  Published readiness-report `native_evm_prover_bundle` summaries must enforce
  the same malformed root and `audit_hashes` field-name policy after bundle
  generation, so rehashed report JSON cannot bypass the native manifest gate.
  Native prover bundle booleans must remain exact: `no_wasm` is accepted only
  as boolean `true`, and `remote_prover_required` is accepted only as boolean
  `false`, with string, numeric, null, and missing variants pinned as blockers
  in readiness and strict verifier tests.
  The bundle builder must reject copied non-empty native summary extra fields,
  noncanonical validation blocker lists, passed summaries with blockers,
  malformed artifact rows, noncanonical proof/key/destination hash text,
	  artifact/hash drift, missing or duplicated audit roles, audit hash reuse,
	  SDK id/implementation drift, missing required SDK rows, SDK implementation
	  artifact/hash drift, and duplicate artifact path roles before
	  `--allow-not-ready` diagnostics can render or write public artifacts.
	  Copied native prover summaries must also recompute from the bundled native
	  manifest and payload artifacts before rendering, so syntactically valid
	  top-level drift such as a swapped destination binding hash cannot publish
	  before the strict bundle verifier runs.
	  Nested artifact summary objects in that published report, including proof,
	  parity/self-test fixture, and SDK implementation artifacts, must reject
	  malformed unknown field names with the same structured diagnostics.
  Duplicate-key blockers for native prover manifests and nested parity/self-test
  fixture JSON must also use structured malformed-key diagnostics for control
  characters, whitespace, Markdown-unsafe characters, and non-ASCII/confusable
  keys rather than echoing operator-controlled duplicate names.
- SCCP native EVM prover parity and self-test fixtures must also carry exactly
  one result row for each required public SDK; rehashed fixture artifacts that
  omit SDK rows, add unknown SDK rows, or replace the SDK result map with a
  malformed/empty container remain readiness and published-bundle blockers.
  Individual SDK result rows must also remain canonical objects with all
  required fields, no unknown fields, and values matching the shared fixture
  hashes and public-signal vector. Duplicate JSON keys inside fixture roots or
  SDK result rows must fail before row values are trusted, so row-level shape
  drift is rejected before release evidence can pass.
  Fixture root objects and SDK result rows also reject malformed unknown field
  names with structured blockers, so control characters, whitespace,
  Markdown-unsafe characters, and non-ASCII/confusable keys are never echoed in
  public readiness or bundle-verifier diagnostics.
- SCCP native EVM prover parity and self-test fixture public-signal vectors
  must keep the canonical nine 32-byte word shape; rehashed fixture artifacts
  with shortened vectors or malformed signal words remain readiness and
  published-bundle blockers even when SDK rows are kept in sync.
- SCCP native EVM prover manifest paths must remain local artifact names:
  public SDK manifest parsers, readiness rendering, bundle generation, and
  bundle verification reject URI/drive-prefix style paths and
  WASM/remote-prover markers in filenames before any no-WASM/no-remote
  evidence can be accepted. Keep SDK implementations free of raw forbidden
  dependency tokens so the source inventory remains fail-closed.
- SCCP release readiness reports now promote the native no-WASM/no-remote
  source inventory to a production gate: public SDK parsers, artifact
  verifiers, self-tests, browser distribution guards, and adversarial native
  prover manifest coverage must remain pinned in the JSON report and published
  bundle evidence. JavaScript native artifact verifier diagnostics must keep
  the field-qualified `nativeProverArtifacts.sdk` rejection for missing or
  padded SDK ids, BSC mainnet/testnet forged descriptor regressions must prove
  plain spread descriptors cannot reach self-test or prover callbacks, and
  bundle-verifier sparse fixtures must prove those markers remain enforced.
  Native EVM release bundles must also keep role-specific artifact byte floors:
  64 KiB for proof/proving material, 128 bytes for verifier/support fixtures,
  and 1024 bytes for SDK implementation artifacts. Public Swift, Kotlin,
  Java/Android, and .NET native EVM artifact verifiers must enforce those same
  floors with hash-consistent below-floor negative tests so mobile or .NET
  callers cannot approve bundles the release gate rejects.
- SCCP release readiness reports now also promote the native EVM Groth16 prover
  bundle schema to a production gate: manifest schema checks, readiness summary
  schema checks, artifact hash/path binding, and bundled-manifest drift
  rejection must remain pinned before public bundle readiness can pass. Native
  no-WASM/no-remote manifest flags must be exact booleans, not truthy or falsy
  scalar substitutes. The
  release bundle builder must also compare copied public artifact rows against
  the copied file byte lengths and SHA-256 hashes before Markdown rendering or
  public JSON writes, so forged input, corridor, or native artifact hashes cannot
  publish before final bundle verification.
- SCCP release readiness reports now also promote the active Ethereum EVM
  source-adapter deployment source inventory to a production gate, pinning the
  deployment-unblocks-production helper, source-bridge network/config binding,
  and negative drift tests before active Ethereum launch evidence can pass.
  Deployment-bound EVM source readiness must also reject replayed source trust
  anchor, message-inclusion verifier, finality-policy, and source bridge runtime
  code hashes before source-adapter readiness or deployment-bound proof matching
  can pass.
- SCCP release readiness reports now also promote the Ethereum no-proxy
  data-collection source inventory to a production gate, so app-owned execution
  and Beacon provider reads, provider markers, and no Torii proxy/embedded
  HTTP-client fallbacks stay pinned across public SDKs before active Ethereum
  launch evidence can pass. The readiness and bundle-verifier inventory tests
  now exercise every configured SDK region directly, including JavaScript
  source/dist, Python, Swift, Kotlin/JVM, Java Android, and C#.
- SCCP release readiness reports now also promote the Ethereum native
  receipt-finality source inventory to a production gate, so Swift, Kotlin/JVM,
  Java Android, and .NET receipt-proof builders must keep finalized-header root,
  sync-committee root, and Beacon-slot prerequisites pinned before active
  Ethereum launch evidence can pass. The inventory tests now remove Swift
  `strictFirstPresent` finalized-root and C# normalized finalized-root markers
  directly in addition to Kotlin finality markers.
- SCCP release readiness reports now also promote the Ethereum Beacon REST
  finalized-header shape source inventory to a production gate, so public SDK
  validators and negative tests for non-zero parent/state/body roots plus
  96-byte finalized-header signatures must stay pinned before active Ethereum
  launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum Beacon REST
  execution-payload binding source inventory to a production gate, so Beacon
  target-header/root/block reads, light-client finality-update evidence,
  execution block-hash/receipts-root binding, and C# SSZ root parity vectors
  must stay pinned before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum sync-committee
  roster source inventory to a production gate, so exact 512-authority mainnet
  rosters, unit validator weights, 342-participant quorum fixtures, and
  81,925-byte next-sync-committee payload vectors must stay pinned across public
  SDKs before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum source-bridge
  config source inventory to a production gate, so bridge-address, network-id,
  code-hash config hashing, and negative config-drift tests must stay pinned
  before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the EVM contract-smoke
  Ethereum-mainnet network-id and production-surface inventories to production
  gates, so ETH/BSC chain-id rejection vectors, accepted-event network ids,
  verifier code/key binding, destination-binding, domain-overflow, proof-shape,
  cross-deployment, and replay-rejection smoke coverage must stay pinned before
  active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum Core
  range/finality binding source inventory to a production gate, so message proof
  ranges must stay bound to artifact finality height and negative outer-range
  replay tests before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum Core message
  replay source inventory to a production gate, so durable pinned-record replay
  protection and negative replay/history tests must stay pinned before active
  Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum Torii pinned
  message-proof source inventory to a production gate, so public readback keeps
  serving only pinned bridge records and negative unpinned-record serving tests
  remain pinned before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the active Ethereum EVM live
  source and destination evidence inventories to production gates, so canonical
  live RPC chain ids, finalized block tags, deployment receipt binding, runtime
  bytecode hashes, route canary calldata, and proof tuple drift regressions must
  stay pinned before active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum launch-policy
  selector source inventory to a production gate, so the `EthereumMainnetLane`
  selector and negative cross-lane policy regressions must stay pinned before
  active Ethereum launch evidence can pass.
- SCCP release readiness reports now also promote the Ethereum route-canary
  finalized receipt-block source inventory to a production gate, so finalized
  receipt-block binding, route-canary TOML fields, all-lanes comments, runtime
  hashing, and negative drift tests must stay pinned before active Ethereum
  launch evidence can pass.
- SCCP release readiness reports now also promote the active Ethereum EVM
  block-tag metadata source inventory to a production gate, so finalized source
  and destination block-tag evidence and negative drift tests must stay pinned
  before active Ethereum launch evidence can pass.
- SCCP corridor phase evidence must also stay source-unique: downloaded
  `--phase-evidence-dir` logs and explicit `--phase-evidence` assignments
  cannot set the same phase twice, so release reports and bundles cannot
  silently replace one hashed phase transcript with another. `--phase-result`
  and `--phase-evidence` phase names, plus `--phase-result` status values,
  must also reject padded or whitespace spellings instead of trim-normalizing
  them into canonical corridor phases or statuses, and phase names must reject
  Markdown-unsafe or malformed values before diagnostics can echo them. Unknown
  phase names and unknown phase-result statuses must use category-only
  diagnostics instead of echoing operator-supplied Markdown-unsafe text, and
  duplicate phase-evidence diagnostics must redact local evidence paths as
  `<path>`. Missing
  `--phase-evidence-dir` logs must report the standard checked layouts without
  echoing the operator-supplied directory.
- Keep Kagemusha offline-offline payments production-routed through the
  Reserved-lineage recursive spend path. Production packaging now has a
  portable Norito `KagemushaRecursiveSpendLineageKeyArtifactsV1` artifact for
  one-hop init and append verifier/proving keys, with request builders rejecting
  wrong profiles, unsupported opening lengths, bad backends, empty artifacts,
  and semantic append attachment; Python, Swift, Kotlin/JVM, Java Android, C#,
  and JavaScript Node/browser/dist expose typed package validators with the same
  fail-closed rules, including ZK1 `CID1` profile binding and Norito
  proving-archive circuit/commitment binding. JavaScript, Android Java, and
  Kotlin/JVM Kagemusha native archive wrappers also copy caller-owned
  `Buffer`, typed-array, and mutable byte-array archives before native bridge
  calls across recursive spend, record-backed compact, recursive aggregation,
  recursive compact, and verifier paths, with JavaScript/Python lineage witness
  and JVM/Android copy-helper regressions mutating caller archives after
  invocation to pin that boundary. Kotlin/JVM public archive entry points also
  route Java-null inputs through the same named local archive validators, so
  null arrays fail with stable `... must not be empty` input errors before
  native availability or generated parameter checks can affect diagnostics.
  Kotlin/JVM and Java Android standalone `VerifyingKeyBoxCodec` helpers also
  decode chain-supplied Norito verifier-key boxes with exact backend
  validation, non-empty key bytes, defensive copies, and trailing child-field
  rejection, and the JavaScript SDK parity guard pins that decode surface.
  Kotlin/JVM and Java Android Offline Note V2 Halo2 helpers now decode and
  verify `OpenVerifyEnvelope` wrappers with exact backend tag, circuit id,
  verifier-key hash, public-input schema, non-empty proof payload, and
  trailing-field checks, with lightweight fake-ZK1 regressions and parity
  guards pinning the non-prover decoder path.
  Kotlin/JVM and Java Android recursive
  compact-token and record-backed recursive aggregation wrappers now also
  validate local ABI-7 inputs as non-empty Norito archives before native
  dispatch, so malformed archives fail as caller input before reserved
  proof-composition state is classified as unavailable.
  ABI-6 verify
  request archives now fail closed at the C bridge, Node host, and PyO3 host
  before returning a diagnostic result when Reserved-lineage verifier records,
  proof backends, or proof attachments are malformed. The ABI-7
  `kagemusha-recursive-compact-v1` compact-token symbols now route one-hop
  LEN=4 record-backed Pallas openings through the compact verifier-slice proof
  path and route package-aware multi-hop compact proving through the append
  verifier-slice loop while keeping production default selection reserved. Core projection tests bind
  folded public-input hash limbs, transcript limbs, witness count, hop count,
  verifier-key CID/hash, verifier-record windows, the one-hop verifier-slice
  side column, and compact verifier-key shape; compact-token envelope
  preverification also rejects noncanonical u64 public-instance limbs,
  unsupported verifier opening lengths, multi-row semantic-prefix padding, zero
  recursive verifier metadata digest groups, stale recursive public-input hash
  limbs reconstructed from the envelope columns, and semantic compact-CID
	  envelopes that omit the one-hop verifier-slice profile. The C bridge, Node
	  host, and PyO3 host ABI-7 compact-token verifiers still hard-fail malformed
	  bindings before returning soft invalid results for backend-invalid compact
	  proofs. ABI-7 compact verification also rejects inner Halo2 IPA proof
	  payloads below the compact proof-size floor after ZK1/instance parsing and
	  before compact verifier-key construction, so syntactically shaped dummy
	  tokens cannot force expensive one-hop verifier work.
	  Native core plus C/JNI/Node/PyO3 compact prover preflights decode Pallas
  opening archives as Halo2 Pallas `OpenVerifyEnvelope` values before
  returning the explicit unavailable code, including rejection of Norito-valid
  chain proof-envelope archives, tampered Pallas openings, extra or missing
  proof-derived Pallas openings, current-hop opening metadata splices in
  Reserved-lineage append preflights, detached Pallas archives that do not bind
  to the supplied record bundle, and append verifier-slice witness/preflight
  splices where the previous recursive proof opening is not the detached
  preflight or the current-hop opening is not bound to the current hop proof
  hash. One-hop verifier-slice evidence binding now also rejects proof-count,
  verifier-witness profile, parameter-fingerprint, fixed-window schedule,
  shared-table manifest, opening-length, table-base, and hop-bound batch-digest
  splices before receiver admission can trust that metadata. Core checked-fold
  preflight also rejects same-hop and cross-hop
  input/output overlap before hop proof decoding, and lineage witness replay
  applies full-bundle fold metadata, verifier-record set, and current-note
  shape/binding plus append-handoff and final-bundle gates before Pallas
  archive or previous-proof parsing. Record-bound multi-hop compact archives
  now require explicit packaged compact key artifacts on package-backed native
  and SDK surfaces, preserving verifier-slice binding coverage while
  production default selection remains reserved. Multi-hop
  Pallas archives with missing openings, forged metadata, duplicated openings,
  or reordered openings reject as record-backed preflight drift before the
  proving path, while the native bridge maps exactly ordered valid multi-hop
  batches plus the matching key package to produced recursive compact tokens
  rather than treating them as malformed input. The height-aware core compact prover
  enforces the same missing-opening, forged-metadata, duplicated-opening, and
  reordered-opening multi-hop preflight boundary and also rejects detached
  Pallas archives and extra one-hop Pallas openings before verifier-batch
  composition diagnostics. Bridge fail-closed/adversarial ABI-7 compact package
  tests should keep using full-width, package-shaped dummy key archives for
  malformed-input coverage so they do not materialize production recursive
  compact keys unless the test is explicitly generating release evidence.
  Kotlin/JVM and Java Android wrappers
  classify the same unavailable diagnostics as reserved proof-composition state
  while keeping malformed local archive validation, compact-token
  public-instance row-shape diagnostics, and compact-token verifier-key hash
  mismatches as caller input errors. Swift, Kotlin/JVM, Java
  Android, JavaScript/Node, Python, and C# lineage key artifact helpers also
  require proving-key archive payloads to contain the selected circuit id bytes
  and verifier-key commitment before native request construction.
  Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C# additionally
  parse the canonical `KagemushaRecursiveSpendLineageKeyArtifactsV1` archive
  fields and reject stale schemas, unsupported flags, byte-smuggled bindings,
  wrong versions, empty proving keys, trailing payloads, non-canonical compact
  Norito length encodings, u64-overflowing terminal compact length bytes, and
  invalid UTF-8 circuit family fields before native loading. Swift,
  Kotlin/JVM, Java Android, JavaScript/Node, Python, and C# compact-token,
  recursive aggregation,
  recursive compact, and recursive spend validators also reject over-cap caller
  archives with explicit `must not exceed` diagnostics before owned byte copies,
  Norito parsing, native availability checks, or native dispatch. The Node NAPI
  and Python PyO3 native hosts also enforce the same 64 MiB archive cap for
  direct native-host entrypoints that bypass the high-level SDK wrappers, with
  ABI-7 recursive-spend regressions covering the single-archive operations and
  every multi-archive lineage witness input slot before Norito decode. Those
  hosts and the shared C bridge also preflight nested current-hop Pallas
  open-envelope archives inside init/append request archives, so empty Pallas
  material fails before core prover or nested decode paths run. The
  Swift native bridge now caps Kagemusha output lengths before copying native
  pointers into `Data`, and Swift/C# verifier normalizers keep the `-312`
  recursive compact unavailable status distinct from `-311` malformed proof
  rejection.
  The Kagemusha payload workflow and SDK parity guard now
  also pin the mobile Halo2 canonical verifier-key hash across Swift,
  Kotlin/JVM, and Java Android, with a negative control that rejects stale hash
  drift before release evidence can pass. Android device-lab readiness is now
  machine-checked through hash-backed slot manifests and strict Kagemusha
  `slot.json` production metadata, including family-specific minimum OS
  enforcement, app/signing-certificate/attestation-challenge/offline-policy
  binding, attestation certificate-chain path/hash-to-bytes binding with
  non-empty PEM/DER shape and payload-size checks, signed evidence artifact
  path/hash-to-bytes binding pinned to `evidence/signed-evidence.json`, a
  closed `slot.json` field allowlist, signed `artifact_digests` coverage for
  release APK, certificate-chain, D2D handoff, and wallet-integrity bytes,
  structured
	  signed-evidence schema checks for slot identity, release APK
	  path/hash-to-bytes binding, native bridge ABI version, physical-device
	  attestation, ABI-7 `one_hop_verified` recursive compact JNI probe state plus
	  `multi_hop_proof_composed` prover state, production pass/fail
	  claims, raw command claims, canonical UTC
  `signed_at_utc`, D2D payment transcript path/hash binding under `handoff/`,
  wallet-integrity transcript path/hash binding for one-use key rotation and rollback rejection,
  required telemetry/attestation/queue/log base artifacts that cannot be
  omitted by regenerating manifests, non-empty/size-capped base artifact
  shapes, telemetry/status/runtime completion markers, symlink- and
  hardlink-free device-lab roots, operator-supplied root ancestors, slot parent
  directories, slot path ancestors, and slot directories plus regular-file slot
  metadata/manifests/artifacts, scanner rejection of unreadable slot directory
  or parent metadata before traversal, and wallet/handoff artifact digests,
  plus exact raw-command checks for the canonical release assembly,
  `:offline-wallet-android:connectedDebugAndroidTest` harness run, and the
  lab-app `installRelease`/`installReleaseAndroidTest` plus
  `adb shell am instrument` export run. Production slots must also carry a separate
  `attestation/report.json` verifier report that repeats the slot identity,
  device fingerprint, OS build, app package, attestation challenge, and
  certificate-chain path/hash from `slot.json`, names the verifier, and reports
  exact `ok` StrongBox/KeyMint plus physical-device attestation before the signer
  can emit `evidence/signed-evidence.json`. The report writer also rejects
  parent-segment, backslash, control-character, and secret-looking
  harness-result source paths before metadata reads or JSON parsing. The
  attestation summary is now a
  closed schema with canonical lowercase SHA-256 hash fields, and it has to
  repeat the slot id, device, OS build, app, challenge, policy,
  attestation-chain path/hash, and StrongBox/KeyMint bindings plus
  `physical_device_attestation: true` from `slot.json`; if both `slot` and
  `slot_id` aliases are emitted, both must match the slot directory.
	  Trusted-signer public-key pinning with symlink-free key-path ancestors,
	  regular non-symlink/non-hardlink key file validation, unreadable key leaf
	  metadata rejection, and Ed25519 signature
		  verification, plus private/public key path validation before OpenSSL lookup,
		  pre-metadata rejection of textual private/public key path aliases, and
		  pre-OpenSSL rejection of secret-looking key path strings while
		  preserving key-path failures separately from private/public key mismatches
		  and treating temporary OpenSSL staging/write/read failures as structured
		  signer/verifier errors after fsynced staged-byte writes and opened-file
		  identity-bound, non-hardlinked readback verification, with signature output
		  bytes also read through opened-file identity and hardlink rejection while
		  bounded to one byte beyond the 64-byte Ed25519 shape check.
	  standalone scanner rejection of secret-looking `--root`/`--json-out`
	  arguments before root discovery or summary writes, direct root-validator
	  rejection of secret-looking paths and unreadable root metadata before slot
	  discovery, direct summary-writer rejection of secret-looking output paths and
	  unreadable output leaf metadata before JSON writes, scanner `--json-out`
	  fsynced temp-file writes with atomic replace, identity-bound temporary
	  cleanup on failed writes, and opened-file identity-bound readback
	  verification capped at 16 MiB,
	  discovered slot-name whitespace/control rejection and unsafe-name
	  redaction before artifact traversal or summary serialization,
	  a signed-slot assembler that consumes completed attached-device
	  attestation, verifier-report, certificate-chain, release APK, D2D handoff,
	  wallet-integrity, telemetry, queue, status, and runtime-log artifacts,
	  reads ADB device identity unless explicit overrides are supplied, refuses
	  existing-slot overwrite, requires signing inputs by default, verifies
	  copied artifacts through destination parent sync and opened-file readback,
	  writes normalized JSON through fsynced temporary files with identity-bound parent
	  sync and readback, publishes the completed stage through directory file
	  descriptors pinned to the captured device-lab root, temp-parent, and
	  staged-slot identities, cleans temporary staging directories only after
	  identity revalidation, and leaves explicitly unsigned staging slots
	  rejected by the production readiness rollup,
	  signer-helper rejection
	  of secret-looking `--slot`, `--output`, and `--signer-key-id` runtime
	  arguments plus padded/control-character signer key ids before metadata
	  reads, signer-side exactness checks for slot metadata strings,
	  signed-evidence artifact paths, attestation-chain paths, and raw test
	  commands before evidence emission, signed-slot assembler rejection of
	  padded/control-character slot ids, requested device families, and device
	  identity overrides before path construction or ADB fallback, standard device-family coverage,
		  cross-slot duplicate device-fingerprint and attestation-challenge rejection,
		  explicit slot-id path-safety checks, and a signer helper that emits the
			  canonical signed artifact from completed slots without persisting private key
		  paths, rechecks signed-evidence/manifest output ancestor/parent/leaf aliases
			  and secret-looking direct output paths
				  immediately before writes, maps absolute signed-evidence output resolver
				  failures to structured signer errors, classifies signer-controlled
				  output parents with `lstat()` before any `Path.is_dir()` preflight,
				  rejects unreadable output parent or leaf metadata before write or
				  digest reads, writes signed-evidence and `sha256sum.txt` outputs
				  through fsynced same-directory temp files with atomic replace and
				  opened-file identity-bound readback verification, rejects
				  `signed-evidence.json` and `slot.json` JSON above 16 MiB before
				  temporary-file creation, rejects `sha256sum.txt` rewrite text above
				  the 1 MiB manifest cap before temporary-file creation and during
				  opened-file readback, revalidates signed-evidence output shape before
				  hashing the written artifact back into `slot.json` and binds that
				  digest read to the opened file identity, classifies slot directory
				  and parent metadata with `lstat()` before parsing slot metadata or rewriting
				  manifests, preflights slot/artifact shape before parsing slot
		  metadata, makes lower-level direct symlink/hardlink/regular-file artifact
		  validators reject secret-looking slot paths before traversal/stat/classification,
		  makes the signer artifact-digest builder rerun the slot preflight before
		  hashing required signed-evidence artifacts, and revalidates each
		  per-artifact digest path, including unreadable leaf metadata, immediately
		  before signed-evidence and manifest rewrite digest reads,
		  makes the direct symlink artifact validator report unreadable
		  slot-metadata, artifact-directory, and nested-artifact metadata before
		  alias classification,
		  makes the direct regular-file artifact validator classify leaves with
		  `lstat()` before any `exists()` preflight can mask unreadable metadata,
		  makes the hardlink and regular-file validators classify artifact
		  directories with `lstat()` before any `exists()` preflight and makes the
		  regular-file validator classify nested artifacts before any
		  `is_symlink()` preflight,
		  makes required-artifact shape checks, required status/runtime text reads,
		  the D2D queue digest binding, and the signed-evidence artifact binding
		  classify artifacts with `lstat()` before any `is_file()` preflight,
		  rejects secret-looking direct slot paths and artifact names in
		  metadata-loader, SHA-256 manifest parser/verifier, and SHA-256 manifest
		  rewrite helper calls, rejects unreadable signer slot/parent metadata before
		  direct metadata parsing or manifest rewrites, rejects unreadable slot-root metadata
		  plus symlinked slot roots and ancestors before direct SHA-256
		  manifest parser/verifier reads, rejects unreadable-metadata
		  and hardlinked `sha256sum.txt` manifests before direct manifest parsing
		  or discovery, binds `sha256sum.txt` parser bytes to the opened file
		  identity so post-preflight swaps fail closed, rejects nonblank manifest
		  lines with leading/trailing whitespace or leading `*` path normalization
		  before digest/path parsing,
		  makes scanner and rollup missing-root decisions consume
		  `lstat()`-classified root presence instead of `Path.exists()`,
		  makes scanner slot inventory classify expected top-level directories,
		  `sha256sum.txt`, and recursive file-count entries with `lstat()` before
		  any `Path.is_dir()` or `Path.is_file()` preflight,
		  makes automatic slot discovery classify device-lab root entries with
		  `lstat()`, preserve symlinked slot entries for fail-closed scan rejection,
		  and report unreadable slot-entry metadata before any `Path.is_dir()`
		  fallback,
		  makes shared Android ancestor validation classify each ancestor with
		  `lstat()` before any `Path.is_symlink()` or `Path.exists()` preflight,
		  makes manifest artifact digest validation classify slot-relative
		  ancestors with `lstat()` before any `Path.is_symlink()` preflight,
		  repeats that slot-path guard across direct
		  attestation, handoff, wallet, required-artifact, signed-evidence, and
		  production-metadata validator helpers, and repeats that
		  preflight before direct SHA-256 manifest rewrites are also now guarded.
		  Signed-evidence artifact digest verification now also revalidates each
		  required artifact path immediately before hashing the bytes claimed by
		  `artifact_digests`, binds each digest read to the opened file identity,
		  and caps reads at 16 MiB. Slot-metadata digest checks now also revalidate
			  `slot.json`-referenced attestation-chain, offline-wallet APK, and
				  signed-evidence artifact paths immediately before SHA-256 reads, then
				  bind bytes to the opened regular-file identity with a 16 MiB cap. D2D handoff,
				  wallet-integrity, and `queue/pending_queue.json` transcript
				  digest checks now use the same pre-read path revalidation and
				  opened-file binding. Shared Android slot JSON loads bind parsed bytes
				  to the preflight `lstat()` identity. Android scanner raw-test-command
				  validation now rejects padded or control-character command entries
				  before exact production-command matching. Android scanner, raw-puller,
				  attestation-report, readiness, and release-bundle diagnostics now redact
				  control-character-bearing JSON keys, summary fields, and artifact
				  labels instead of echoing unsafe terminal strings, and raw ADB
				  stderr details with control characters are redacted before CLI
				  display. Required status/runtime marker text reads now also revalidate the slot-relative
				  artifacts immediately before decoding and marker checks.
			  Direct slot-file discovery reports unreadable slot-root and
		  artifact-directory metadata through caller error lists, returns no
		  artifacts for secret-looking slot paths, symlinked slot ancestors,
		  missing roots, non-directory roots, symlinked slot roots, or symlinked
		  artifact directories before traversal, and
		  direct manifest verification rejects entries under symlinked artifact
		  directories before hashing. The manifest verifier now also revalidates each
		  `sha256sum.txt` artifact path immediately before digesting and binds the
		  read to the opened file identity with a 16 MiB cap, so direct verifier
		  calls cannot reuse
		  stale parse-time checks after a path mutation.
			  The shared Android device-lab JSON loader rejects secret-looking direct file paths and
		  symlinked ancestor directories before parsing direct metadata, attestation,
		  handoff, wallet-integrity, or signed-evidence JSON, decodes bytes from one
		  opened regular file after path-identity revalidation, and fails closed with a
		  structured read error for unreadable or non-UTF-8 JSON bytes. The D2D transcript is
  closed-schema and has to
  prove an offline payer/payee handoff, matching sent/received payload hashes,
  receiver redeem acceptance, duplicate-spend rejection, hash-bound transport
  session/one-use-key/receiver ACK values, changed payer/payee wallet-state
  digests, a queue digest that matches `queue/pending_queue.json`, and a
  `slot.json` path binding that stays under `handoff/`. The
  wallet integrity transcript is also closed-schema and has to prove one-use
  key rotation, old-key invalidation, stale-snapshot rejection, changed key ids,
  changed wallet state after rotation, and active-state preservation after the
  rollback attempt is rejected. Readiness summaries now use local device-lab root
  labels, include a compact per-slot signed-evidence map of timestamp, artifact
  hash, and signer hash for validated slots, and avoid printing absolute summary
  output paths. A strict readiness rollup now combines the ABI-6 manifest,
  ABI-7 fail-closed contract,
  Reserved-lineage proof evidence, ABI-7 recursive compact key evidence, signed Android evidence, release-cutoff
  freshness, future-date clock-skew rejection, and standard family coverage into
		  a ready/blocked JSON summary; the checked-in ABI-6 manifest must be a regular
		  non-symlink, non-hardlinked file with symlink-free ancestors before its
		  release contract is trusted, with manifest JSON decoded only when the
		  opened regular file matches the preflight `lstat()` identity and remains
		  path-bound after the read, and
	  the checked-in ABI-7 fail-closed and Reserved-lineage release-tooling marker
	  source files must also be ordinary non-symlink, non-hardlinked files before
	  their marker text can satisfy readiness, with marker bytes decoded from the
	  same opened regular file after path-identity revalidation, capped at 8 MiB,
	  and unreadable
	  or non-UTF-8 marker bytes fail closed as structured blockers. The proof evidence has to hash-bind adjacent
	  `.norito`, `.record.norito`, `.vk`, `.pk`, and production proof-log artifact
	  bytes plus artifact sizes from the same opened regular files, with the
  final readiness rollup revalidating artifact path identity around the opened
  file before trusting digest or size, while rejecting empty, symlinked, hardlinked, or
  metadata-unreadable proof artifacts/logs, classifies artifact/log missing-vs-unreadable state from the
  lstat-backed local-file validators instead of `Path.is_file()`, re-check
  that the proof log is hashed and parsed from the same opened regular file and
  contains only the exact single expected proof test line and one-test cargo result
  with canonical LF line endings, strict UTF-8 bytes, a final LF
  terminator, and no trailing whitespace or forged suffixes, satisfy
  release-cutoff and future-date skew bounds, keep the
	  canonical `lineage-proof-evidence.json` filename while rejecting symlinked
	  evidence files or symlinked evidence ancestors, and remain
	  closed-schema with duplicate JSON object keys rejected at every nested
	  evidence object and non-standard `NaN`/`Infinity` JSON constants rejected
	  before schema checks after proof-evidence JSON is parsed from the same
	  opened regular file with path-identity revalidation and the explicit 16
	  MiB evidence JSON cap. The compact key evidence hash-binds the ABI-7 LEN=4
	  `recursive-compact-len4.record.norito`, `.vk`, `.pk`,
	  `recursive-compact-key-artifacts.norito`, and
	  `recursive-compact-verifier-keys.norito` artifacts, validates compact key artifact byte sizes
	  against the same opened local bytes with the same readiness-side
	  path-identity revalidation, rejects empty compact key artifacts, all-zero lineage
	  artifacts, and plain-text or all-zero placeholder compact key artifacts,
	  hashes and parses `recursive-compact-key-artifacts.log` from the same
	  opened regular file, requires exactly the canonical CLI summary line with
	  canonical LF line endings, strict UTF-8
	  bytes, a final LF terminator, and no trailing whitespace, and checks the reported `.vk`,
	  `.pk`, `.record.norito`, key-artifacts package, and verifier-keys package
	  sizes and SHA-256 digests against local bytes. The staged compact-key
	  runner also writes `recursive-compact-key-staged-run.json`, and the
	  finalizer applies that runner-report binding for successful exit markers so
	  the canonical command, exit code, elapsed seconds, generator-log filename,
	  and generator-log byte count are bound before staged artifacts can be
	  published.
	  The proof-log and compact generator-log byte caps are enforced from the
	  opened file metadata used for hashing and decoding, so readiness does not
	  trust a separate path-size lookup for replacement log bytes; the checked-in
	  ABI-6 manifest is likewise capped at 1 MiB before JSON decoding. The canonical command must provide
	  `--key-artifacts-out` and `--verifier-keys-out` together, and the CLI now
	  rejects missing or one-sided package-output flags before starting expensive
	  key generation; old-shape commands remain release evidence blockers until
	  a fresh canonical artifact run replaces the currently
	  running old-shape generator. The evidence file preserves the canonical
		  `recursive-compact-key-evidence.json` filename and validates the canonical
		  release command instead of runtime key generation, while release
		  key-artifact writers, evidence JSON helper outputs, and readiness
		  summary output now use same-directory temporary files, byte fsync, atomic
		  rename, opened-file identity-bound readback verification pinned by helper-level, readiness-summary,
		  release-bundle, and Android scanner/signer mismatch/read-failure/open-path/regular-file-swap regressions, post-rename output
		  revalidation pinned by symlink-swap regressions, and parent-directory sync
		  to avoid trusting partial lineage, compact package, evidence, or summary
		  artifacts after interrupted generator runs. The readiness-summary,
		  Android device-lab summary, Android signed-evidence helper,
		  evidence-helper, and release-bundle JSON writers also reject non-finite
		  `NaN`/`Infinity` values before creating temporary outputs, and the
		  evidence-helper validation scratch files now use the same strict JSON
		  preflight before touching `--artifact-dir` and report scratch-file
		  cleanup failures even when the scratch write itself fails. The readiness
		  guard pins the evidence helpers' writer-specific strict JSON blocks so
		  validation scratch-file serialization cannot mask writer drift. The readiness
		  summary writer enforces a 16 MiB `--summary-out` cap before
		  temporary-file creation, during final opened-file readback, and
		  reports identity-bound temporary-file cleanup failures after write or post-stage
		  output-validation errors. The lineage and compact-key
		  evidence helpers also enforce their readiness evidence JSON byte caps
		  before creating `--out` temporary files and during final opened-file
		  readback after atomic replacement, and report identity-bound temporary-file cleanup
		  failures after output write or post-stage output-validation errors. The Android device-lab summary
		  writer enforces the 16 MiB JSON cap before creating `--json-out`
		  temporary files plus during final opened-file readback and reports
		  identity-bound temporary-file cleanup failures after write or post-stage
		  output-validation errors. The Android
		  signed-evidence helper output writer applies the same cleanup failure
		  reporting and temp-file identity checks to its atomic JSON and manifest text writes, while the
		  release-bundle writer enforces its 16 MiB manifest cap before
		  temporary-file creation and during final opened-file readback and
		  reports identity-bound temporary-file cleanup failures after write or post-stage
		  output-validation errors as structured blockers. The release-output
		  writers now fail closed on parent-directory sync failures after atomic
		  replacement rather than accepting an unsynced directory entry, and the
		  readiness summary writer now identity-binds that parent-directory sync
		  before readback. The Reserved-lineage proof evidence helper applies the
		  same identity-bound parent sync before publishing
		  `lineage-proof-evidence.json`, and the ABI-7 compact-key evidence helper
		  does the same before publishing `recursive-compact-key-evidence.json`.
		  The Android attestation report writer and signed-evidence helper now
		  apply that same parent-identity gate to local report, signed-evidence,
		  and manifest outputs. The staged lineage and compact-key runners also
		  identity-bind parent syncs before accepting marker and JSON metadata
		  outputs, and the staged finalizers apply the same gate to the published
		  artifact directory before final fsync. Android signed-evidence
		  canonical signature payloads also reject non-finite values before
		  hashing, signing, or verification.
	  The Kagemusha release
	  bundle manifest now uses
	  `iroha.kagemusha.production_release_bundle.v1` to recompute the checked-in
	  ABI-6, ABI-7, and lineage release-tooling trust roots, hash-bind the ready
	  readiness summary, Reserved-lineage proof evidence, ABI-7 compact key
	  evidence, and scanner-validated Android signed-evidence inventory, while
	  recording bundle-relative per-slot Android signed-evidence artifact paths
	  and SHA-256 digests plus the Reserved-lineage and compact key artifact size maps, and listing packaged lineage artifacts,
	  compact key artifacts, the compact key generator log, production proof logs,
	  release APKs, D2D handoff transcripts, wallet-integrity transcripts, and
	  attestation certificate-chain files with
	  bundle-relative paths, SHA-256 digests, and byte sizes computed from bytes
	  whose opened file identity matches the preflight `lstat()` identity and
	  remains path-bound after the read, while readiness-summary and
	  verify-existing manifest JSON inputs are capped at 16 MiB before decoding,
	  then revalidating each slot name while rejecting summary drift
	  across repo-trust and external-evidence sections, duplicate JSON keys,
	  unexpected top-level, section-level, or per-slot Android signed-evidence
	  summary fields, missing Android signed-evidence summary fields, malformed
	  summary digests or timestamps, per-section blockers
	  in a ready summary, non-string, unsafe, or noncanonical nested evidence
	  inventory paths, malformed nested evidence digests, or missing/boolean/non-integer/non-positive
	  nested evidence sizes in existing release manifests, secret-looking paths, secret-looking
	  strings anywhere inside the readiness summary, plain-text or all-zero placeholder
	  compact key artifacts in the compact-key artifact inventory, evidence outside `--bundle-root`, symlinked
	  bundle roots, symlinked or hardlinked bundle outputs, and secret-looking
	  trusted signer key paths before loading signer keys, with newly-created
	  bundle-output parents revalidated before 16 MiB-capped fsynced temporary-file writes,
	  atomic replacement, final output-path revalidation, identity-bound
	  parent-directory sync,
	  and 16 MiB-capped readback verification pinned by read-failure, oversized
	  readback, post-replace symlink-swap, and parent-directory swap
	  regressions. The same helper can verify existing manifests
	  by parsing readiness summaries and existing manifests from opened regular
	  JSON files whose identities match their preflight `lstat()` checks,
	  preflighting the
	  manifest path before local evidence scanners run, and
	  then validating nested evidence inventory path, digest, and required-size shape before using a
	  stable manifest comparison that ignores only the verifier run
	  timestamp. Any release input
	  path that escapes the bundle root stops all readiness/evidence/device-lab
	  loading for that bundle run, and `--out` cannot overwrite any
	  hash-bound readiness summary, evidence JSON, proof log, key artifact, or
	  Android signed-evidence file.
	  Unreadable or non-UTF-8 ABI-6 manifest and proof-evidence
	  JSON files fail closed as structured read blockers. The lineage evidence helper also rejects noncanonical raw
	  `--generated-at-utc` input instead of normalizing it before writing release
	  JSON, and refuses symlinked artifact directories or aliased evidence output
	  leaves and ancestors before creating missing `--out` parents or reading
	  release artifact/proof-log inputs; the shared evidence builder enforces the
	  same secret-path and canonical proof-log-under-artifact-dir corridor before
	  hashing artifacts or reading proof logs, and direct artifact-dir,
	  proof-log corridor, and output-preflight helpers reject secret-looking
	  artifact/proof-log/output paths plus unreadable artifact-dir or `--out`
	  parent metadata before resolving corridors, creating temporary files,
	  creating output parents, or writing evidence JSON, and classify `--out`
	  parents with `lstat()` before any `Path.is_dir()` preflight, with
	  final output write failures reported as structured blockers; the shared local lineage file validator rejects
	  secret-looking, parent-segment, or backslash-bearing evidence, artifact,
	  or proof-log paths and symlinked local-file ancestors before JSON parsing,
	  digest calculation, or proof-log
	  reads, and both the readiness rollup's direct SHA-256 reader and the
	  lineage helper's direct SHA-256 reader repeat that validation before
	  returning artifact digests, with the readiness, lineage-helper, and
	  compact-key helper readers binding each digest/text read to the first
	  validated `lstat()` identity so post-preflight regular-file replacements
	  fail closed, and the Reserved-lineage all-zero plus compact-key
	  placeholder checks consume the prefix captured by the same artifact hash
	  read instead of reopening the artifact path. The compact key evidence helper
	  also hashes, sizes, decodes, and parses the generator log from one opened
	  regular file, with read-time byte failures reported as
	  structured blockers instead of tracebacks. Ready rollup summaries also publish
		  sanitized SHA-256 maps for the
		  accepted Reserved-lineage artifacts and proof log without preserving local
		  artifact paths, reuse the scanner-validated signed-evidence timestamp for
		  Android freshness instead of re-opening slot files, refuse symlinked or
			  symlink-ancestor `--repo-root` aliases, unreadable repo-root metadata,
			  plus direct secret-looking repo-root validator inputs before resolving
			  checked-in trust roots, repeat that preflight inside direct ABI/source
			  trust-root section checks, map
			  `--repo-root` resolver failures to structured rollup blockers before
			  relative evidence paths are expanded, and make shared Android ancestor
			  validation fail closed on cwd metadata failures for relative helper
			  inputs,
				  reject secret-looking, parent-segment, or backslash-bearing direct
			  ABI-6 release JSON and ABI/source marker file paths plus unreadable
			  ABI-6 release JSON and source-marker leaf metadata before content parsing,
			  bind ABI/source marker reads to the preflight
			  `lstat()` identity so post-preflight source swaps fail closed, cap
			  source-marker text at 8 MiB,
			  redact and block any secret-looking string that reaches an Android scanner
		  report before summary serialization,
			  reject symlinked rollup summary output ancestors plus symlinked or
			  hardlinked summary output aliases, reject dangling symlink summary
			  output leaves, unreadable summary output parent or leaf metadata, and
			  secret-looking direct rollup summary output paths, classify summary output parents
			  with `lstat()` before any `Path.is_dir()` preflight,
			  recheck created output parents and ancestors before writing summaries,
			  and make the Android device-lab
			  scanner reject aliased and unreadable-metadata `--json-out` summary
			  targets before writing.
			  Android slot artifact enumeration now fails closed on top-level slot
			  directory list failures in both scanner and signing-helper manifest rewrite
			  paths, and manifest inventory discovery reports artifact metadata read
			  failures; direct hardlink artifact validation reports unreadable file
			  metadata before hardlink checks, and digest-time manifest, metadata, and
			  signed-evidence artifact validators distinguish missing files from
			  unreadable leaf metadata, so release evidence cannot pass with a partial
			  artifact inventory. The shared Android JSON loader also reports
			  unreadable JSON leaf metadata before parsing or duplicate-key checks.
		  The signing helper and Reserved-lineage proof helper now reject dangling
			  symlink output leaves before writing signed evidence, manifest refreshes,
			  or lineage proof evidence JSON, and the lineage proof helper rejects
			  unreadable output leaf metadata before evidence writes; signer output
			  writers also bind slot artifact digest reads to the opened regular-file
			  identity, cap signer-side slot-artifact and signed-evidence output
			  digest readbacks at 16 MiB, cap signer JSON outputs before
			  temporary-file creation, cap `sha256sum.txt` manifest rewrites at
			  1 MiB, and recheck created output parents and
			  ancestors before writing,
			  and the lineage helper's direct output preflight rechecks created parents
			  before returning success.
			  Android signer absolute output corridor resolver failures now return
			  structured signer blockers before write attempts.
			  Lineage helper proof-log and output corridor resolver failures now return
			  structured blockers instead of raw resolver errors.
  The Android device-lab scanner also rejects duplicate JSON
	  object keys and non-standard `NaN`/`Infinity` constants in slot metadata,
	  attestation, signed-evidence, D2D handoff, and wallet-integrity artifacts
	  before those rows can move from blocked to ready; those JSON inputs are
	  also capped at 16 MiB before parsing, while direct `sha256sum.txt` manifests
	  are capped at 1 MiB using opened-file metadata and streamed byte counts.
	  A physical Pixel 6 / Android 16 smoke run now passes the focused
	  production command with ABI-6/ABI-7 JNI load assertions plus the full
	  offline-wallet connected suite, but real signed lab evidence and the
	  remaining Android family matrix are still required before release readiness
	  can pass.
	  C/JNI/Node/PyO3 receiver verification rejects malformed compact-token
	  bindings before returning a soft invalid result. The C bridge now carries a
	  shape-valid ABI-7 compact-token fixture that returns `valid = 0` while
	  proof-composition is unavailable, plus a stale folded-token binding mutation
	  and non-canonical compact verifier-key/hash regressions that hard-fail
	  before the soft-invalid path.
  Remaining compact-token release work is to attach signed device-lab evidence
  and release evidence for packaged one-hop and append proving-key artifacts
  before enabling SDK default selection. The
  production-readiness CI guard now treats that as the release contract and
  checks concrete ABI-7 core and bridge function bodies, not only loose marker
  text: ABI-6 Reserved-lineage may be advertised as the production
  offline-offline route, while ABI-7 recursive compact is implemented as an
  explicit key-package-backed surface until release evidence opens default
  selection.
- Active BSC mainnet SCCP SDK hardening now directly gates malformed
  receipt-observed source-event logs: browser, Python, Swift, Kotlin/JVM, Java
  Android, and .NET tests reject matching BSC source-bridge logs with extra
  topics, non-empty data, zero digests, duplicate/removed events, or missing
  transaction context, and the release bundle verifier requires those markers
  before the BSC lane can be advertised as ready.
- Rust SCCP canonical transcript packaging now uses checked `u32`
  length-prefix writers on production `Option` admission paths, so oversized
  Merkle-proof, bundle, finality-proof, transparent-statement, and
  source-chain proof-envelope transcript fields fail closed before TON,
  native/local, platform, TAIRA diagnostic packaging, or runtime finality
  export. SCCP source-adapter verification statement, adapter-commitment, and
  FastPQ context packaging also reject unbounded adapter-proof shapes and
  oversized checked length prefixes before proof batch construction.
- Ethereum mainnet SCCP release gating now treats the published JS browser
  artifact as a first-class launch surface: the strict release bundle verifier
  scans both source and `dist` for receipt-proof admission guards that bind
  block receipts, receipt roots, execution headers, finalized Beacon roots,
  and sync committee roots before browser evidence can be advertised as
  production-ready. It also scans native SDK receipt-proof builders for
  block-receipt metadata binding and typed receipt rejection, and requires
  every outbound Ethereum SDK facade, including Python, to validate the
  configured mainnet execution provider before caller-supplied submission
  callbacks can run. The Rust source-adapter readiness gate now also checks
  ETH/BSC EVM deployment material explicitly, so Ethereum mainnet readiness
  rejects replayed source bridge network ids, config hashes, and emitters
  before proof packaging, with strict release-bundle markers pinning the gate
  and regression coverage. Core `SubmitBridgeProof` admission now also binds
  typed SCCP message proof ranges to artifact finality height, with an ETH
  local-admission range-replay regression and strict release-bundle markers;
  the same core path requires SCCP message proof records to be pinned, keeps
  pinned bridge proofs out of manual pruning, and rejects a second retained
  proof for the same `(source_domain, target_domain, messageId)` only when the
  retained record is verified, pinned, and internally consistent. Torii's
  SCCP message-bundle submission path now emits pinned bridge proofs as well,
  so app-facing `/v1/bridge/proofs/submit` payloads remain compatible with
  core replay protection; its proof-registry read side also refuses to serve
  unpinned SCCP message records as non-SORA source-chain envelopes.
  Live Ethereum evidence scripts and all-lanes imports now keep finalized
  block-tag metadata under the same strict release-bundle verification, and
  the diagnostic unready transparent-proof bypass stays config-owned with the
  production Taira config pinned false and the old environment override
  rejected by the release verifier. Production-ready BSC/TRON route-config
  renderers must also reject explicit `--allow-unready true` so governed runtime
  overlays cannot re-enable diagnostic transparent-proof admission while
  claiming production readiness. Release-readiness and strict release-bundle
  source inventory must pin the direct and merged route-config rejection tests,
  plus the default `sccp_allow_unready_transparent_proofs = false` overlay
  assertion, before this gate can pass.
- For SCCP Ethereum mainnet launch, keep the product SDK path source-material
  checks aligned with Rust/Python evidence tooling: JS/browser, Swift, Kotlin,
  Java Android, and C# now require the Ethereum mainnet network id plus an
  address/domain/code-hash-bound ETH source bridge config hash before emitting
  source verifier material or source adapter deployment hashes, with strict
  release-bundle/readiness inventory guards pinning those config-hash checks
  across SDKs and Python evidence tooling. The same Ethereum-mainnet inbound
  facades now bind app-supplied beacon finality
  finalized-header root, sync-committee root, and finalized beacon slot to the
  receipt proof, requiring those fields before local proving/submission
  callbacks can run. The Swift SCCP corridor, Kotlin/JVM and Java Android
  suites have now been rerun, with OpenJDK 21 for the Gradle phases and the
  local .NET 8 SDK for C# validation. The JS/browser, Swift, Kotlin/JVM, Java
  Android, and C# SDKs also now expose Ethereum mainnet Beacon REST consensus
  providers so apps can
  collect finalized Beacon REST evidence from their own consensus endpoint,
  fail closed on optimistic/unfinalized or checkpoint-mismatched data, and keep
  sync committee material/proving local without a Torii proxy or WASM prover;
  the strict release bundle verifier now also pins the published JS
  `dist/sccp.js`, `dist/index.js`, and `index.d.ts` artifacts plus package
  no-WASM tests under the native/local-prover gate, and its regression suite
  proves all three public artifacts plus remote-prover identifier variants stay
  pinned, so the browser product surface cannot silently fall out of the
  no-WASM inventory. Ethereum mainnet
  local-admission packaging is now pinned by the same strict verifier across
  JS/browser, Python, Swift, Kotlin/JVM, Java Android, and C#, including
  ETH -> SORA routing, immutable native proof bytes, non-zero statement/source
  material/deployment hashes, and canonical metadata/proof-family checks; the
  JS/browser and Python regressions now explicitly cover stale proof-family
  metadata alongside the native/mobile suites. Browser receipt-proof
  auto-construction from user JSON-RPC receipts now also rejects incomplete
  app-supplied Beacon finality before emitting proof material unless the
  finalized header root, sync committee root, and Beacon slot are present, with
  source, published `dist`, and release-bundle markers pinning that guard.
  The browser product path also now rejects manually supplied receipt-proof
  transcripts that drift from app-supplied Beacon finality or the validated
  source-event digest, and strict release/readiness inventories pin those
  adversarial checks.
  Browser Ethereum inbound collection and prover callbacks now receive
  deep-copied immutable evidence snapshots, so app-owned consensus-provider or
  local prover code cannot mutate caller-owned receipt logs, block/finality
  extension fields, receipt-proof trie nodes, inclusion branches, byte buffers,
  or Beacon finality branches after SDK validation; the published JS/dist and
  release-readiness inventories pin these callback-boundary guards. The
  browser outbound Ethereum regression now also mutates copied request byte
  getters and frozen public-signal words inside the app-linked prover callback
  while proving the wrapped result retains the validated request bytes.
  JS/browser, Swift, Kotlin/JVM, Java Android, and C# EVM-family outbound proof
  requests/results now also support paired non-zero `proofArtifactHash` and
  `provingKeyHash` metadata; when present, the request hash binds both values,
  wrapped results carry the same pair, and release inventories pin browser and
  native regressions for missing, zero, or mismatched artifact metadata. The
  release bundle/readiness path now also fails closed unless an audited
  `sccp-native-evm-groth16-prover-bundle-v1` manifest is attached, hash-bound,
  no-WASM, no-remote-prover, and tied to the active Ethereum destination
  binding, proof artifact, and proving key hashes. JS/browser, Swift,
  Kotlin/JVM, Java Android, and C# now expose that native prover bundle as a
  first-class SDK descriptor, validate the per-SDK native implementation rows
  and audit hashes locally, parse the signed JSON manifest with the same
  camelCase/snake_case release-tooling aliases, and let Ethereum mainnet
  outbound facades bind the descriptor hashes into proof requests while
  rejecting loose hash conflicts. The bundle parsers now reject noncanonical
  hash evidence, including uppercase or mixed-case `audit_hashes`, before apps
  can use descriptor hashes; release/readiness inventories pin the SDK
  canonical-hash helper names across SDK source, JS `dist`, and TypeScript
  declarations so the signed-manifest product path cannot silently regress
  before release. The SDK parsers also enforce native bundle hash role
  separation across proof-artifact, proving-key, verifier-key,
  destination-binding, per-SDK implementation, and audit hashes before app
  prover callbacks run, so replayed audit hashes fail inside the product path
  instead of only during release verification. The same parsers now treat the
  signed native bundle manifest as a closed schema, rejecting unknown top-level
  or per-SDK artifact fields and duplicate accepted aliases before descriptor
  hashes can reach app prover code. Native manifest domains now also reject
  noncanonical decimal text such as `"01"` before the Ethereum-mainnet domain
  check, keeping signed manifest review and SDK binding on the same field
  value. Those
  SDKs now also verify local native prover artifact bytes against the bundle's
  SHA-256 proof-artifact, proving-key, verifier-key, per-SDK implementation,
  `cross_sdk_fixture_parity_artifact`, and
  `native_prover_self_test_artifact` hashes before reporting artifact
  readiness. Those SDKs parse the parity and self-test fixture bytes locally,
  carry the normalized vectors in the verified descriptor, and reject
  hash-consistent proof-artifact/proving-key payloads below `64 KiB`,
  verifier/support fixture payloads below `128` bytes, or implementation
  payloads below `1024` bytes before reporting artifact readiness. They
  also reject hash-consistent local payloads that still contain forbidden WASM,
  `snarkjs`, or remote-prover dependency markers; release/readiness inventories
  and package-dist tests pin those verifier APIs across the same
  browser/mobile/native surfaces. Those same SDKs now expose resolver-based
  helpers that load the manifest-declared proof artifact, proving key, verifier
  key, cross-SDK parity fixture, native prover self-test fixture, and selected
  SDK implementation from app-owned local bundle resources before running the
  byte verifier, so product apps do not need side metadata, WASM, or a remote
  prover to assemble a verified descriptor. The same release/readiness
  inventories now also pin SDK-owned BN254 Groth16 tuple validation and
  malformed-proof regressions across JS/browser, Swift, Kotlin/JVM, Java
  Android, and C#, so wrong tuple versions, out-of-range field words, invalid
  curve points, or public-input/domain mismatches cannot silently fall out of
  the product submission path. JS/browser, Swift, Kotlin/JVM, and Java Android
  now add
  `EthereumMainnetSccp.fromNativeProverBundle(...)` product entry points for
  that flow, returning facades already bound to the verified artifacts before
  outbound proof/calldata/submission guards run; C# exposes
  `ProveOutboundToEthereumFromNativeProverBundleAsync(...)`,
  `BuildEthereumCalldataFromNativeProverBundle(...)`, and
  `SubmitOutboundToEthereumFromNativeProverBundleAsync(...)` for the same
  resolver-backed proof, calldata, and submission path. The SDK marker tables and hash-consistent
  regression payloads now use numeric byte construction so those runtime
  checks stay present without putting forbidden dependency identifiers in the
  source artifacts scanned by the no-WASM/no-remote-prover inventories. Those
  facades now also require the manifest-bound native prover self-test to run
  through the SDK-owned/app-linked self-test hook before production proof
  output is requested, rejecting missing hooks or drifted self-test rows before
  native prover callbacks execute. The JS/browser SDK now exposes the same
  check as `runEthereumMainnetNativeProverSelfTest(...)` and
  `EthereumMainnetSccp.runNativeProverSelfTest(...)`; Swift, Kotlin/JVM, Java
  Android, and .NET now expose matching native prover self-test preflight
  methods, so product apps can verify the native prover bundle at startup
  before the first outbound proof request. The release readiness report and
  strict bundle verifier now also classify those startup preflight methods as
  required Ethereum/BSC user-prover helper symbols, so removing the easy
  public check path fails release verification before the lane is advertised.
  The
  verified descriptor path now also requires a concrete SDK id plus matching
  verifier-key and per-SDK implementation bytes before reporting native
  artifact readiness, so product apps cannot satisfy the easy Ethereum proof
  path with proof/key bytes alone. The native prover bundle application paths
  now also reject verifier-key hashes that do not match the Ethereum mainnet
  destination binding, so a bundle for another verifier key cannot reach app
  prover callbacks by reusing a matching destination-binding hash. The
  Ethereum mainnet easy outbound proof facades now require those verified
  artifact descriptors at proof time before app-owned prover callbacks run:
  JS/browser, Swift, Kotlin/JVM, and Java Android reject missing or mismatched
  descriptors after request construction but before proof execution, while C#
  exposes an artifact-bound `ProveOutboundToEthereumAsync` overload for the
  same product path. The verified descriptor gate now also applies at
  Ethereum-mainnet calldata/submission time, so hand-wrapped proof results
  cannot bypass the native artifact checks before app-owned Ethereum submitter
  callbacks run. Release/readiness inventories pin those proof-time and
  submission-time gates across source, JS `dist`, and TypeScript declarations.
  The release-bundle builder now also copies the manifest-declared proof, key,
  and per-SDK implementation payload bytes
  into the public attachment bundle, and the strict verifier rehashes those
  copied files so metadata-only, tampered, or path-escaping native prover
  descriptors cannot satisfy launch readiness. Readiness generation, release
  bundle generation, and strict bundle verification now also scan those payload
  bytes for forbidden WASM, `snarkjs`, and remote-prover dependency markers, so
  hash-consistent payloads that still reference `proof.wasm` or remote prover
  endpoints remain blocked; they also reject proof-artifact/proving-key
  payloads below `64 KiB`, verifier/support fixture payloads below `128`
  bytes, and per-SDK implementation payloads below `1024` bytes even when the
  manifest hashes are self-consistent. The native prover
  bundle's proof-artifact, proving-key, verifier-key, destination-binding, and
  per-SDK implementation hashes are role-separated as well, so one manifest hash
  cannot stand in for another. Bundle `audit_hashes` now must be a named evidence
  object with
  `circuit_security_audit`, `native_implementation_audit`,
  `reproducible_build_attestation`, `cross_sdk_fixture_parity`, and
  `no_wasm_no_remote_scan`; every value must be unique, cannot reuse artifact,
  key, binding, or implementation hashes, and must use canonical lowercase
  `0x`-prefixed 32-byte hex before readiness can pass. The
  `cross_sdk_fixture_parity` hash must now bind a public
  `cross_sdk_fixture_parity_artifact` JSON vector that repeats the active
  Ethereum mainnet artifact hashes, receipt-proof hash, source-proof hash, nine
  public signal words, destination-binding hash, calldata hash, and Torii
  submit-payload hash for every required SDK (`javascript`, `swift`, `kotlin`,
  `java-android`, and `dotnet`); missing vectors, tampered vector bytes, and
  per-SDK drift block readiness and strict release-bundle verification. The
  JS/browser, Swift, Kotlin/JVM, Java Android, and C# signed-manifest parsers
  now expose that release-bundled parity-vector path as part of the native
  prover descriptor and parse the parity fixture locally with the same
  schema/domain/backend/hash, nine-word public-signal, and per-SDK drift
  checks. Strict verifier source-inventory markers pin the scanner,
  empty-payload blockers, native hash-role blockers, canonical-hash blockers,
  audit-hash role blockers, parity-vector blockers, SDK parity parsers, and
  adversarial regressions.
  The same
  manifest-declared
  artifact paths are now parsed, validated as safe manifest-relative POSIX
	  paths, and exposed by JS/browser, Swift, Kotlin/JVM, Java Android, and C#
	  bundle descriptors so apps can locate the public release-bundled prover
	  files without side metadata. Readiness generation and release-bundle copying
	  now reject duplicate JSON keys in the signed native prover manifest before
	  path, schema, or hash checks run, keeping reviewed fields from depending on
	  last-key-wins parsing. The release/readiness tooling now applies the same
	  duplicate-key rejection to the hash-bound parity and native self-test JSON
	  artifacts after their SHA-256 evidence is matched, so fixture vectors cannot
	  smuggle reviewed fields through last-key-wins parsing either. The
	  JS/browser, Swift, Kotlin/JVM, Java Android, and
	  C# SDK manifest parsers now enforce the same duplicate-key rejection before
	  descriptor materialization, and their parity/self-test fixture parser
	  regressions now pin duplicate `schema` rejection in the same product path,
	  including escaped-key aliases in the string parser paths, so product apps
	  get the same signed-manifest and fixture semantics as the release tooling.
	  Readiness and strict bundle verification now also reject non-empty native
	  prover proof/proving payloads below `64 KiB`, verifier/support fixture
	  payloads below `128` bytes, and implementation payloads below `1024`
	  bytes, so hash-consistent label strings cannot stand in for audited proof,
	  proving-key, verifier-key, parity/self-test, or per-SDK implementation
	  payloads. The JS/browser product verifier now enforces the same role floors
	  on app-loaded local proof, proving-key, verifier-key, support fixture, and
	  JavaScript implementation bytes before accepting a manifest-bound native
	  prover descriptor.
	  The remaining SDK gap is still implementing and shipping the actual audited
	  browser/native Groth16 circuit/prover artifacts rather than app-linked local
	  prover callbacks.
  Python Ethereum inbound evidence collection and prover callbacks now use the
  same immutable evidence snapshot boundary, detaching nested receipt logs,
  block/finality extension fields, receipt-proof trie nodes, inclusion
  branches, and bytearray payloads before app-owned consensus-provider or
  native-prover code runs; Python Ethereum inbound prove/submit helpers also
  enforce the native-recursive proof-byte corridor (non-empty, non-all-zero,
  and at most 2 MiB) before returning prover output or invoking app submitters.
  The shared Python BSC inbound facade uses the same evidence snapshot boundary
  and release/readiness inventories pin the Ethereum regressions.
	  Swift Ethereum inbound collection and prover callbacks now receive native
	  evidence snapshots as well, recursively detaching Foundation mutable
	  dictionaries/arrays/data, receipt-proof byte buffers, inclusion branches, and
	  block-receipt lists before app-owned consensus-provider or local-prover code
	  runs; the shared Swift BSC inbound facade uses the same boundary and the
	  release/readiness inventory pins the adversarial regressions.
	  Kotlin/JVM and Java Android Ethereum inbound collection/prover callbacks now
	  rebuild native evidence snapshots too, detaching mutable maps/lists and byte
	  arrays, receipt-proof trie nodes, inclusion branches, and block-receipt lists
	  before app-owned consensus-provider or prover code runs; Kotlin/JVM applies
	  the same callback boundary to the shared BSC inbound facade. Their Beacon
	  REST root-drift regressions now use deterministic 512-member Ethereum
	  mainnet sync committee payloads, and the release/readiness inventory pins
	  those JVM/Android adversarial fixtures.
	  .NET Ethereum inbound collection/prover callbacks now receive the same
	  detached native evidence snapshot, including copied dictionaries/lists,
	  string finality branches, byte arrays, receipt-proof bytes, inclusion
	  branches, and block-receipt lists; the .NET BSC inbound facade shares that
	  collection/prover boundary and release/readiness inventories pin the
	  adversarial regressions as part of the no-WASM native SDK path. The .NET
	  Ethereum outbound regression now also mutates the app-linked proof-engine
	  callback request snapshot across public-input, signal-word, bundle, and
	  source-proof fields while verifying the wrapped result retains the
	  validated request bytes.
  The core launch selector now has direct regression plus release-verifier and
  readiness-report coverage proving `EthereumMainnetLane` opens only a
  production-ready ETH lane independently of unfinished future lanes, while
  incomplete ETH evidence, BSC-shaped lanes, and the `AllLanesAtOnce` policy
  remain fail-closed.
  Swift, Kotlin/JVM, Java Android, and C# auto receipt-proof builders now mirror
  the same finalized-root, sync-root, and Beacon-slot prerequisites in their
  block-receipt construction tests, and the release-bundle verifier has a
  dedicated native receipt-finality inventory pinning those source/test guards,
  with readiness-report coverage for the same native launch invariant.
  JS/browser Ethereum mainnet proving now also has explicit regressions that
  missing inbound/outbound local proof callbacks fail before any execution
  provider fallback is attempted, with release inventories pinning the
  no-fallback errors; direct finality maps and Beacon REST finality updates now
  also require at least 342 of 512 sync committee participants before
  ETH -> SORA local proving can observe the evidence, with under-quorum
  negatives pinned across JS/browser, Swift, Kotlin/JVM, Java Android, and C#;
  Kotlin/JVM now also fails directly when explicit
  `syncCommitteeParticipation` is present without `syncCommitteeBits`, and the
  JVM/Android alias-only finality regressions carry the required
  `finalityBranch` before local prover callbacks can run;
  all five SDKs now also reject present-but-malformed Beacon REST boolean
  safety fields (`execution_optimistic`, `executionOptimistic`, `finalized`,
  and finalized-header `canonical`) and require canonical non-zero finalized
  header message roots (`parent_root`, `state_root`, `body_root`) plus the
  96-byte BLS `signature` before accepting finality evidence; the JS/browser
  SDK now also rejects all-zero Beacon finalized header/block/checkpoint roots,
  sync-committee roots, and direct app-supplied beacon finality roots before
  local proving, and now rejects all-zero direct
  `beaconFinality.executionBlockHash` and
  `beaconFinality.executionReceiptsRoot` values before matching proof material,
  matching the native SDK root normalizers; the browser execution-provider
  path also rejects zero direct transaction/block hashes plus zero receipt
  transaction hashes, receipt block hashes, fetched block hashes, and block
  receipt roots while preserving canonical lowercase `0x` JSON-RPC hex
  enforcement before any local proving callback can run, and the browser
  outbound Ethereum provider path validates optional `from` senders as
  canonical non-zero 20-byte addresses and pins `chainId: "0x1"` before
  `eth_sendTransaction`, with strict release-bundle/readiness inventories
  pinning those header/root-shape checks;
  release-bundle/readiness inventories now also pin proof-time regressions
  proving browser, Swift, Kotlin/JVM, Java Android, and C# reject hash-only
  Ethereum `receiptProofHash` evidence before local inbound prover callbacks,
  so hash-only display evidence cannot become source proof material; JS/browser,
  Swift, Kotlin/JVM, Java Android, and C# now also require validated SCCP
  source bridge log context before receipt-proof-backed Ethereum inbound
  proving can invoke local prover callbacks, preventing prebuilt receiptProof
	  material from bypassing source-event admission checks, and release
	  inventories now pin malformed source-event negatives for extra topics,
	  non-empty data, zero digests, duplicate matches, and removed logs across all
	  primary SDKs; browser, Python, Swift, Kotlin/JVM, Java Android, and C# Ethereum
	  inbound prove/submit helpers also enforce the native-recursive proof-byte
	  corridor (non-empty, non-all-zero, and at most 2 MiB) before returning local
	  prover output or invoking app submitters; SDK receipt-proof
	  transcript helpers now also require non-empty receipt-trie proof nodes and
  non-empty consensus inclusion branches plus the correct ETH/BSC source
  domain before deriving receipt-proof hashes, with browser/native release
  inventories pinning empty-node, empty-branch, and cross-domain negatives;
  Swift, Kotlin/JVM, Java Android, and C# now reject forged Ethereum outbound
  `destinationBindingHash` values at the facade `wrapProofResult` boundary,
  Swift also rejects forged binding hashes at
  `EthereumMainnetSccp.buildOutboundProofRequest(...)` before returning a
  request to callers, and the Python Torii client regression now pins both
  forged binding-hash rejection and BSC pre-callback rejection for the Ethereum
  facade; those explicit forged-request regressions are pinned by release
  inventories;
  the public active Ethereum mainnet launch checklist now also treats source
  and destination live-read `eth_chainId == 0x1` (1), `finalized` block tags,
  and finalized route-canary receipt-block metadata as governed-deployment
  blockers before the lane can be advertised, with the
  standalone release-bundle verifier recomputing the same active checklist
  from embedded evidence, and its
  cryptographic-evidence table exposes the EVM route-canary transaction hash,
  receipt block number/hash, finalized receipt-block flag, block
  `receiptsRoot`, and `messageId` with bundle-verifier checks binding those
  public fields back to embedded lane evidence, while the live route-canary
  adversarial suite now also pins `eth_getTransactionByHash(...).to` to the
  governed destination bridge address, and release/readiness inventories now
  also pin live source deployment receipt/transaction readback, receipt-block
  `receiptsRoot` verification, finalized deployment-block binding, source
  record hashes, destination bytecode-hash, `verifyingKeyHash()`,
  `destinationBindingHash()`, binding-key, calldata, and
  `usedMessageProofs(bytes32)` replay guards, with route-canary adversarial
  coverage for malformed `submitSccpMessageProof(bytes,bytes32[6],bytes32)`
  ABI/proof/public-input words plus BN254 base-field, G1, G2, and G2
  prime-subgroup validation of the embedded Groth16 tuple; EVM route-canary
  evidence hashes are now `v4`
  digests that commit to the finalized receipt-block readback flag, so
  non-finalized diagnostic reads cannot reuse finalized canary hashes, and the
  same flag is threaded through typed Rust config, SCCP readiness, core launch
  readiness, and Torii mappings with a Rust/Python ETH vector parity regression
  so production EVM lanes reject missing or non-finalized route-canary receipt
  evidence; the
  same Beacon REST providers now resolve the target Beacon block from
  app-supplied slot/root/id metadata or timestamp-derived mainnet slot evidence,
  require the target header/root to be finalized relative to the current
  finalized head and checkpoint, reject historical target slots without an
  ancestry proof in every native SDK, then require the target block body's
  execution payload slot, `block_hash`, `block_number`, and `receipts_root` to
  match the execution RPC block before emitting finality evidence, with release
  inventories pinning the historical-target ancestry rejection, the current
  finalized-slot target markers, and that execution-payload binding plus
  native timestamp-derived target-slot regressions, and
  the dynamic JS/browser provider requires real boolean
  `verifyFinalityCheckpoint` overrides rather than coercing strings or numbers;
  its fetch adapter validates Response-like `ok`/`status` fields before JSON
  parsing, and real browser `fetch` responses prefer bounded `ReadableStream`
  reads with a size-checked `text()` fallback before local `JSON.parse`; native
  parsers reject non-object Beacon REST JSON roots before safety-field
  inspection and cap Beacon REST response bodies at 1 MiB before parsing, with
  bounded default HTTP transport reads in
  Kotlin/JVM, Java Android, C#, and Swift; Swift additionally rejects oversized
  declared `Content-Length` values; Beacon REST URL builders preserve endpoint
  query strings when appending finalized-header and checkpoint paths, and now
  treat endpoint roots ending in `/eth/vN` as version roots so finalized-header
  and finalized-block calls are sibling Beacon API paths instead of nested
  `/eth/v1/eth/v2/...` paths; all five SDKs verify local
  `syncCommitteePayload` bytes against the derived sync-committee root when
  payload material is supplied, and now fetch
  `/eth/v1/beacon/light_client/finality_update` to pin
  `sync_aggregate.sync_committee_bits`, `sync_committee_signature`, and the
  signature slot to the finalized header, and require the six-sibling
  `finality_branch` to be normalized as `finalityBranch` before evidence leaves
  the SDK while rejecting empty sync-committee participation and all-zero
  aggregate signatures; the Ethereum mainnet inbound facades also require those
  finality-update fields and reject stale direct `syncSignatureSlot` values
  plus all-zero direct `syncCommitteeSignature` values before invoking
  app-owned prover callbacks, and the TypeScript declarations plus typed Swift,
  Kotlin/JVM, Java Android, and C# finality-evidence surfaces expose those
  fields directly; Python evidence tooling now mirrors the same finality gate
  and binds prebuilt receipt proofs back to finalized-header roots,
  sync-committee roots, and beacon slots before its native prover callback can
  run; native direct `beaconFinality` maps now reject duplicate
  camelCase/snake_case aliases for the same finality value and normalize direct
  sync-aggregate fields before any receipt-proof or local-prover callback can
  observe them, and browser/native SDKs now also reject direct finality maps
  whose `syncCommitteeParticipation` does not match the popcount of
  `syncCommitteeBits` or whose `syncSignatureSlot` does not cover the
  finalized `beaconSlot`; those direct finality maps must also carry
  `finalityBranch`/`finality_branch` before easy inbound prover callbacks can
  run. Accepted direct finality maps now strip known alias spellings from
  callback-facing evidence while preserving unknown extension fields for app
  proof context. The EVM contract smoke path now also binds its `networkId`
  vector to a 32-byte Ethereum mainnet chain-id `1` value instead of a devnet
  label before exercising constructor and destination-binding checks, and the
  wrapper constructor now rejects ETH/BSC deployments whose nonzero network id
  does not match the target domain's canonical mainnet EIP-155 chain id word.
  The release bundle verifier now also requires EVM contract smoke markers for
  verifier code/key hash binding, incompatible verifier contracts without
  `verifyingKeyHash()`, `destinationBindingHash()`, malformed Groth16 proof
  words, source/target domain overflow and same-domain proof-word rejection,
  nonzero wrong destination-binding rejection, cross-wrapper Groth16 replay
  failure, `MessageProofAccepted` payload fields, and replayed `messageId`
  rejection before verifier execution. Ethereum mainnet cannot be advertised
  as ready unless those smoke markers remain present.
  The diagnostic `sccp_allow_unready_transparent_proofs` bypass is now
  config-only, with the old runtime environment override removed from
  `iroha_config` and Taira launch units. Production-ready BSC/TRON route-config
  renderers reject explicit `--allow-unready true` before writing runtime
  overlays.
  The offline EVM destination evidence helper now applies the same mainnet
  network-id guard when deriving destination binding keys, so TOML/JSON evidence
  cannot carry a hash checked against one network id and a key rendered from
  another.
  The Python evidence tooling
  and pure JS/browser,
  Swift, Kotlin/JVM, Java Android, and C# SDKs now also reconstruct typed
  receipt RLP, RLP transaction-index
  receipt-trie keys, proof nodes, and receipt roots from user-supplied mainnet
  JSON-RPC via `eth_getBlockReceipts`, so the product path can carry locally
  verified receipt inclusion material into the same SDK prover flow instead of
  relying on a remote prover or Torii proxy; the Python evidence collector and
  JS/browser, Swift, Kotlin/JVM, Java Android, and C# receipt-proof builders now
  also reject malformed block receipt sets with duplicate `transactionHash`
  values before deriving receipt-trie proof material; the Python collector also
  rejects duplicate JSON keys in top-level JSON-RPC responses and nested
  receipt objects before semantic evidence review, and now treats SCCP
  source-event validation as the default collection mode: receipt-only output
  requires an explicit diagnostic opt-in and is labeled separately in the
  emitted evidence, with release inventories pinning both attack shapes and
  the source-event mode/zero-digest guards; and the release inventory now pins noncanonical
  `eth_chainId` rejection, including leading-zero `0x01`, across SDK and
  Python receipt-proof collection tests. The JS/browser
  receipt-proof encoder now also rejects all-zero source event digests,
  execution block hashes, execution receipt roots, Beacon finalized roots, and
  sync-committee roots before hashing or local proving callbacks, matching the
  native SDK receipt-proof normalizers and keeping the no-WASM browser path
  fail-closed on direct app-supplied proof material. The standalone SCCP
  release-bundle verifier also keeps malformed active-lane schema diagnostics
  primary by suppressing derived release-checklist drift checks once embedded
  all-lanes evidence or checklist schema validation has already failed. Native
  receipt-proof cross-binding now applies the same duplicate-alias rejection to
  finalized-header roots, sync-committee roots, and beacon slots before local
  prover callbacks can observe direct app-supplied evidence, and browser/native
  source-event log validators reject conflicting receipt-log transaction,
  block-hash, and block-number aliases before deriving SCCP source event
  digests. Browser, Swift, Kotlin/JVM, Java Android, and C# receipt-proof
  collectors and trie helpers now also reject conflicting aliases for receipt,
  block, `eth_getBlockReceipts` target receipt metadata, and receipt-RLP
  gas-used/logs-bloom fields before
  constructing Ethereum mainnet source-proof transcripts. Kotlin/JVM and Java
  Android now also reject prebuilt Ethereum `receiptProof` plus Beacon finality
  evidence that lacks validated `sourceEventDigest` before any local inbound
  prover callback can observe the proof material. Rust/core Ethereum mainnet
  source-proof verification now also rejects replayed source-adapter deployment
  receipt material at the structure, production, and bundle-helper gates, even
  when the replayed deployment descriptor is internally production-shaped, and
  source-adapter deployment evidence now only unblocks production for explicit
  ETH/BSC EVM lane branches so future or unsupported domains fail closed until
  they receive audited lane-specific policy. Ethereum mainnet sync-committee
  helpers in Rust/core, JS/browser, Python, Swift, Kotlin/JVM, Java Android,
  and C# now also reject compressed or weighted committee rosters: payloads
  must carry exactly 512 authorities with unit weights and proofs must use the
  fixed 64-byte mainnet signer bitmap before any transcript hash or local
  prover callback can observe them. Release-bundle and release-readiness
  marker inventories now pin those exact-roster guards across Rust/core and
  every SDK artifact before publication, and the Java Android corridor
  transcript gate now also requires the `SourceSccpProofsTests` harness marker
  so source-proof hardening cannot be dropped while advertising SDK coverage.
- Keep focused validation green for the core transaction pipeline, Torii query
  and control-plane APIs, Norito wire formats, and SDK fixtures before broader
  workspace test runs. Torii app-query pages now expose concrete OpenAPI page
  schemas and SDK parsers for bounded `has_more`/`count_mode` metadata across
  the account, domain, asset, NFT, RWA, asset-holder, and repo-agreement
  list/query surfaces, and `torii_hot_paths` now includes sustained concurrent
  HTTP handler-path profiles for signed stored-cursor continuations, account
  alias projections, account-asset predicates, asset holders, committed-history
  contract activity, and generic aggregates, plus localhost socket profiles for
  the same workload set. Caller-scoped account reads now also require canonical
  request signatures or witnesses for private dataspace visibility, so bare
  `X-Iroha-Account` headers no longer create caller identity, and the Torii
	  library suite is green after aligning stale SCCP, SoraFS, ISO20022, and ZK
	  fixtures with current production admission rules. Torii API-token-gated
	  Sumeragi/SCCP/bridge endpoints now also emit bounded endpoint/token-state
	  telemetry counters without exporting raw token material, and Torii ZK prover
	  report list/count/bulk-delete filters now reject malformed `has_tag` values
	  unless they are exactly four printable ASCII ZK1 TLV tag characters. Default
	  Torii builds now omit disabled telemetry, schema, profiling, and ZK
	  batch-verify routes instead of exposing placeholder `501 Not Implemented`
	  handlers, and account-alias resolution returns a documented `409 Conflict`
	  for stored non-account alias-service targets instead of a `501` fallback.
	  Routed-query unsupported-shape responses now also use `409 Conflict`, while
	  no-`app_api` inbound Torii proxy read/fanout/hosted-HTTP requests report
	  `503 route_unavailable`; the feature-minimal connect corridor now passes
	  check, library tests, and all-target strict clippy with app-only proof,
	  hosted-proxy, integration-test, binary, and bench targets gated behind
	  their owning features. SoraFS proof streaming rejects reserved
	  `proof_kind=pdp` as `400 Bad Request` until the SF-13 provider protocol
	  lands. The code-only placeholder/TODO sweep now leaves only intentional
	  negative tests, fail-closed placeholder-material guards, fallback skeleton
	  naming, manifest-derived source rendering, and telemetry peer compatibility
	  handling. The
	  `iroha_cli --all-targets` strict clippy gate now
  covers the governance-instruction, IVM contract deploy, and Taikai helper
  targets with checked length/time arithmetic in the previously failing paths.
  The `iroha_crypto --all-targets` strict clippy gate is also green, covering
  the SoraNet token/handshake and RAM-LFE test targets beyond the library-only
  checks. The non-default GOST, SM, forced-NEON SM, SM OpenSSL provider,
  Rayon-backed Merkle, secp256k1 MSM-batch, BLS multi-pairing, FFI export, and
  crypto parity-test feature corridors now also pass strict `iroha_crypto
  --all-targets` clippy plus focused library tests, with SM acceleration and
  OpenSSL preview tests serialized around their test-only runtime dispatch
  overrides. The combined `iroha_crypto --all-features` all-targets clippy,
  library, and integration-test corridors are also green after the BFV
  adversarial evaluation-key metadata tests were split below strict
  test-target line limits and forced-NEON SM acceleration tests were serialized
  around their shared runtime override state; the all-features pass fixed SM
  dispatch precedence so `sm-neon-force` force-enables only the `Auto`
  policy and explicit `force-disable` still pins the scalar fallback. The
  `iroha_data_model --all-targets` strict clippy gate is green after clearing
  the Kagemusha/ZK-ACE test/bench lint surface, and the touched-package
  all-target gate for `iroha_data_model`, `connect_norito_bridge`,
  `iroha_js_host`, `iroha_kagami`, and `sorafs_orchestrator` now also passes
  with `--no-deps`. The full `soranet-relay` strict clippy gate now reaches and
  passes relay diagnostics without `--no-deps`.
- Keep crypto primitives fail-closed at the crypto boundary. The
  secp256k1 recoverable prehash helper now emits canonical low-S signatures and
  rejects high-S malleable recoverable inputs before public-key/EVM-address
  recovery, complementing the SCCP caller-side canonical-signature preflights.
  Ed25519 uncached batch verification now preflights signature `R` encodings so
  non-canonical or small-order representations fail before the batch backend,
  and the direct byte-key/preparsed batch APIs filter exact verify-cache hits
  before signature parsing and backend setup. The thread-local Ed25519 exact
  verify-ok cache now keeps two entries per exact slot, reducing collision
  churn for 32-byte transaction-hash verification tuples without introducing a
  process-wide cache. SoraFS proof-token decode now reads fixed-width
  moderation-token fields through checked cursor helpers so malformed token
  prefixes return `DecodeError::Truncated` instead of relying on manual slice
  invariants, and unrepresentable issued/expiry UNIX-second fields now fail
  closed at decode time before `SystemTime` conversion. Proof-token body
  encoding now exposes `try_encode`, routes mint/signature/digest helpers through
  checked entry-count and entry-length narrowing, and makes the compatibility
  `encode` path fail closed to a malformed frame for impossible direct token
  states. Proof-token minting now also reports token-id RNG failures through a
  labelled `MintError::RandomBytes` before blinded digest or signature material
  is produced. Proof-token base64 header encoding/decoding now uses the `base64`
  crate's checked no-alloc slice helpers with invariant-sized buffers, and the
  encoder no longer falls back to an empty header on internal encode or UTF-8
  conversion failures. Proof-token binary/base64 decode and direct signature
  verification now reject all-zero Ed25519 signature placeholders before
  accepting or verifying externally supplied moderation-token signature
  material.
  The SoraFS paid-pin validation corridor is
  green across data-model SoraFS/DA-pin, Core pin-registry, Torii
  storage-pin/discovery, and gateway conformance filters as of 2026-06-04;
  Torii DA commitment proof/verify routes are now also pinned with committed
  block-backed Merkle proof round-trip coverage and OpenAPI/MCP contract text,
  DA pin-intent proof/verify handlers are pinned against the live indexed
  location store with tampered-location rejection and matching contract text,
  and Torii SoraFS CAR range coverage now verifies a non-full middle-window
  response spanning exactly two aligned chunks with `Content-Range` and
  `X-Sora-Chunk-Range` metadata. Remaining breadth is SDK validation once Java
  is available plus wider admission/manifest-envelope/full-corridor reruns not
  covered by the current focused Torii SoraFS checks. SoraNet relay handshake frame length-prefix
  writes now use an explicit checked helper plus a compile-time `u16` maximum
  assertion so oversized relay hellos return `FrameTooLarge` instead of relying
  on a narrowing assertion. SoraNet constant-rate scheduler dequeue now handles
  unexpected empty queues explicitly and falls through to the dummy-cell path
  instead of using panic-only queue-pop assertions.
  ML-DSA public-key reconstruction from private-key material now has a
  fallible API, and `KeyPair::from_private_key` uses it so length-valid but
  internally inconsistent ML-DSA secrets return `KeyGen` instead of panicking;
  `KeyPair::try_from_seed`, `KeyPair::try_random`, and
  `KeyPair::try_random_with_algorithm` give ML-DSA/GOST/SM2/BLS key generation
  non-panicking routes, with ML-DSA seeded-keygen rejecting non-empty all-zero
  seed material before HKDF, random ML-DSA keygen drawing checked OS seed
  material through the same constructor instead of the infallible PQ random
  keypair path, HKDF expansion now propagated as `Error::KeyGen` instead of
  relying on a panic-only assertion, and the S2 nonce offset conversion using
  the same `Error::KeyGen` route, and GOST
  deterministic nonce generation now feeds the domain tag, private scalar,
  message scalar, and optional extra entropy into HMAC-Streebog as separate
  components while preserving the previous contiguous seed transcript, and
  digest-length mismatches return `Error::Signing` instead of panicking;
  Ed25519 and secp256k1 now expose checked `try_keypair` paths, and top-level
  `KeyPair::try_from_seed` routes their seeded branches through those helpers
  while `KeyPair::try_random_with_algorithm` routes OS-backed Ed25519 seed bytes and
  secp256k1 candidate scalar bytes through `OsRng::try_fill_bytes` so
  entropy-source failures or bounded scalar-sampling exhaustion surface as
  `Error::KeyGen` instead of the infallible compatibility RNG adapter;
  standalone X25519 key exchange now exposes `KeyExchangeScheme::try_keypair`,
  draws OS-backed private-key bytes through `OsRng::try_fill_bytes`, and routes
  P2P, native Connect bridge, and Python Connect keypair generation through
  fallible error surfaces instead of the infallible compatibility adapter;
  Connect Norito bridge C/Java keypair-from-seed helpers and the Swift parity
  regeneration utility now use `KeyPair::try_from_seed`, returning existing
  bridge/key-derivation errors instead of panic-only seed expansion, while the
  bridge's generic C/JNI detached-signing helpers route through
  `Signature::try_new` and the secp256k1 signing entrypoint calls `try_sign` so
  backend signing failures return existing bridge errors without first
  collapsing to an empty signature; Torii DA ingest receipt construction now
  encodes unsigned receipts and signs them through fallible routes so receipt
  encoding/signing failures return HTTP errors instead of unwinding; Torii
  operator signed-header generation now uses `Signature::try_new` and returns
  the operator-signature HTTP error shape on signing backend failures; SoraFS
  gateway PoR proof signing now uses `Signature::try_new` and propagates
  backend failures through the gateway proof-builder error path; DA SDK/CLI
  ingest request builders now return errors from `Signature::try_new` instead
  of panicking on payload-signing backend failures; Iroha client account and
  operator signed-request builders now propagate `Signature::try_new` failures
  through their existing `eyre` result paths; JS host crypto, Soracloud
  provenance, and alias-proof fixture signers now propagate `Signature::try_new`
  failures through N-API errors; the Connect Soracloud upload request signer
  now returns command errors from `Signature::try_new` for init/finalize
  provenance signatures; SoraFS Taikai cache admission envelopes and gossip
  wrappers now return `CacheAdmissionError::Signing` from `Signature::try_new`
  instead of unwinding on backend signing failures; the SoraFS fixture manifest
  exporter, CLI domain-endorsement preparation, and SoraFS repair worker
  claim/complete/fail payload signing now also propagate
  `Signature::try_new`/`SignatureOf::try_new` failures through command errors;
  transaction submission receipts now expose
  `TransactionSubmissionReceipt::try_sign` and Torii submission responses use it
  to return formatted internal errors on receipt-signing backend failures;
  the wired Torii Offline Notes issuer now signs JSON payloads and
  key-certificate material through `Signature::try_new`, returning contextual
  internal query errors on backend signing failures while the currently unwired
  v2 issuer source mirrors the same helper shape; streaming `KeyUpdate` frame
  construction now maps `Signature::try_new` failures into
  `HandshakeError::Signing` instead of unwinding during local control-plane
  signing; P2P versioned handshake hello signing now uses
  `Signature::try_new` and propagates backend failures through the existing
  handshake `Result` path; embedded `irohad` Soracloud runtime model-host
  heartbeat and Inrou host advert provenance signing now shares a fallible
  `Signature::try_new` helper with contextual `eyre` errors;
  `QueryRequestWithAuthority` now exposes `try_sign` for
  `SignatureOf::try_new` failures, and the CLI JSON-stdin and
  cursor-continuation query paths use it to return contextual command errors
  instead of relying on the infallible compatibility wrapper;
  transaction builders, multisig signature bundles, and sealed transaction
  commitments now route through fallible `SignatureOf::try_new` APIs while
  retaining compatibility wrappers for existing callers;
  local Sumeragi VRF material derivation plus local VRF commit/reveal metadata
  signing now use `Signature::try_new` and propagate contextual `eyre` errors
  through the emission `Result` path;
  GOST random scalar sampling and per-signature extra entropy now also use
  checked OS fills, random scalar sampling rejects all-zero OS material before
  retry-budget exhaustion, per-signature entropy rejects all-zero OS material
  before falling back to deterministic nonce derivation, and GOST deterministic
  key generation rejects non-empty all-zero seed material before scalar sampling, while both BLS backends derive
  random keys from checked OS
  seed material after rejecting all-zero OS seed output and the default w3f
  backend seeds its key-splitting/signing RNGs only after checked OS fills,
  with both backend test/clippy lanes pinned in release-readiness validation
  while leaving the compatibility `os_rng()` adapter
  test-only. SM2 top-level random key generation now routes through
  `Sm2PrivateKey::try_random`, fallible `TryCryptoRng` byte draws, and bounded
  scalar validation before returning key material, while SM2 deterministic seed
  derivation rejects non-empty all-zero seed material and validates
  distinguishing identifiers before hashing candidates; P2P SoraNet runtime
  handshakes now seed their local `StdRng`
  through `SeedableRng::try_from_os_rng` and surface entropy-source failures as
  `HandshakeSoranet` instead of panicking; Taikai ingest-edge drift jitter now
  keeps explicit seeds deterministic while routing unseeded `StdRng` setup
  through `SeedableRng::try_from_os_rng` and the CLI `Result` path, and CEK
  rotation receipt HKDF salts now use direct checked OS RNG fills when an
  explicit `--hkdf-salt` is not supplied; Kagami keypair, PoP, client-config,
  genesis-signing including NPoS bootstrap escrow, wizard, localnet
  peer/genesis/gas/extra-account key generation, and the Taira Kaigi localnet
  overlay example's seed-derived genesis signer now route
  random, seeded, and private-key-derived material through `KeyPair`'s fallible
  APIs and BLS PoP `Result`s instead of compatibility panic
  wrappers; irohad's ephemeral Torii receipt-signer fallback now uses checked
  secp256k1 key generation and surfaces entropy/keygen failures as `StartTorii`,
  while `iroha_swarm` peer/genesis key generation, seeded network material, and
  BLS PoP proving now return `Error::KeyGeneration` through `Swarm::new`
  instead of panicking; the CLI offline fallback config and governance council
  VRF candidate-account derivation now use `KeyPair::try_from_seed`, surfacing
  config/candidate derivation errors through existing `Result` paths, and
  Izanami workload, Nexus gas, NPoS validator, post-topology, and network-builder
  key material now uses `KeyPair::try_random` / `KeyPair::try_from_seed` with
  explicit `Result` propagation instead of panic-only `KeyPair` wrappers;
  `MultisigRegister::from_spec` now also returns `Result` and generates its
  temporary registration anchor account through checked default key generation;
  the transaction-gossip frame-cap probe now uses a fixed checked Ed25519 seed
  instead of drawing a runtime dummy key;
  Private Kaigi fee-spend execution now derives its synthetic fee-payer account
  through checked Ed25519 seed expansion from the action hash; SoraFS hybrid
  KEM derived material now binds the recipient public keys and encapsulated
  public transcript components through length-prefixed HKDF input with checked
  capacity accounting, and SoraNet session-key HKDF extraction now
  domain-separates and length-prefixes IKM components before expansion, with
  NK2/NK3 interop vectors refreshed under both checked-in fixture bundles;
  SoraNet deterministic SHAKE expansion now also frames its domain, label, part
  count, and every absorbed component before deriving deterministic KEM,
  simulated ML-DSA, dual-mix, or Noise-seed material, with checked-in fixture
  bundles regenerated from the framed outputs;
  `PublicKey::try_to_*` and
  `ExposedPrivateKey::try_to_*` give public/private key formatting
  non-panicking routes, public-key Norito serialization now routes
  full-to-compact conversion through a checked payload extractor, and
  `PublicKey::to_prefixed_string` now reuses the malformed compact-key marker
  instead of unwrapping invalid internal key state, while `ExposedPrivateKey`
  display and prefixed compatibility formatting now return a non-secret
  invalid-private-key marker instead of unwrapping checked private-key
  formatting; `Signature::try_new` now routes SM2 through checked private-key
  rebuild/signing helpers and SM2 key-pair/public-key derivation now routes
  through `try_public_key`; SM2 concrete public-key prefixed formatting now
  returns a deterministic invalid-key marker instead of unwrapping checked
  multihash encoding; SM2 private-key byte export now exposes
  `PrivateKey::try_to_bytes` and routes exposed private-key multihash formatting
  through checked payload extraction; the compatibility `PrivateKey::to_bytes`
  wrapper no longer falls back to an empty private-key payload if checked export
  fails; secp256k1 message signing now exposes
  `try_sign` and routes `Signature::try_new` through the fallible helper,
  deterministic secp256k1 key generation now rejects explicit all-zero
  32-byte seed material before DRBG expansion, direct secp256k1 verification
  maps malformed and all-zero compact signatures
  to `Error::BadSignature`, the compatibility `sign` helper no longer falls
  back to an empty signature if checked signing fails, and
  secp256k1 recoverable prehash signing now checks the low-S recovery-id parity
  flip before emitting EVM-compatible signatures; SM2 embedded-distid payload
  decoding now returns `ParseError` for short length prefixes instead of relying
  on a panic-only fixed-slice assertion; SM2 PEM export now wraps the already
  encoded base64 `String` without a panic-only UTF-8 reconversion; SM2 DER
  signature export now exposes `try_as_der` with checked short-form length
  encoding, the compatibility `as_der` helper no longer falls back to an empty
  payload if that invariant is broken, and routes the OpenSSL bridge through
  that fallible exporter before DER parsing; SM2 signature decoding now rejects
  all-zero and zero-scalar encodings before backend parsing, and SM2 verifier
  boundaries map malformed signature material to `Error::BadSignature`; SM2
  random private-key generation now rejects all-zero RNG seed material
  immediately before scalar parsing or retry-budget exhaustion;
  generic ML-DSA public/private key import and direct batch verification now
  reject all-zero public-key, private-key, and detached-signature material before
  backend parsing;
  SM4-CCM now checks tag, nonce,
  AAD, payload, and counter-block
  length narrowing through its existing encrypt/decrypt `Result` paths; the SM
  signature shim's SM4 self-test block now uses the infallible fixed-key
  constructor instead of
  `new_from_slice(...).expect(...)`; and ML-DSA import, `Signature::try_new`, and
  typed `SignatureOf::try_*` constructors also reject secrets whose recomputed
  public material or embedded `tr = H(pk)` public hash is inconsistent before
  signing.
  SoraNet PQ ML-DSA helpers now apply the same secret-key consistency check to
  direct validation and direct/OS-backed signing, reject all-zero standalone
  public-key, secret-key, and detached-signature material before backend use,
  reject all-zero deterministic `HedgedRngSeed` material before seeded keygen,
  reject all-zero caller/OS seed draws before `*_from_rng` keygen or signing,
  and expose fallible public-key reconstruction from secret material. SoraNet PQ
  labeled-HKDF derivation now streams the namespace, separator, label,
  separator, and context components through `expand_multi_info`, preserving the
  previous contiguous info layout without manual capacity arithmetic.
  Session-key zeroization debug telemetry
  now recovers poisoned mutex state before recording or reading scrubbed bytes,
  and Torii identifier RAM-LFE receipt/output-opening signing uses
  `SignatureOf::try_new` so attestation signing failures surface as
  `IdentifierResolutionError::Signing` instead of unwinding.
  BLS same-message aggregate and preaggregated verification now reject
  duplicate public keys and public-key aggregates that cancel to the identity
  before verification, and the public PoP-gated same-message wrappers reject
  duplicate signer keys before PoP verification/cache work and no longer fall
  back to per-signature verification after aggregate rejection; distinct-message
  aggregate verification rejects duplicate messages and aggregate signatures
  that cancel to the identity before batch verification. The blstrs feature
  backend also reuses the w3f signing/message semantics for normal, small,
  same-message, preaggregated, and distinct-message aggregate verification so
  backend choice does not change accepted signatures, and its compressed G1/G2
  public-key decoders now use explicit `CtOption` to `Option` handling instead
  of panic-only unwrap assumptions; the feature-gated
  `iroha_crypto --all-targets` strict clippy corridor now covers the blstrs BLS
  test targets as well, and the default w3f `bls` all-targets corridor is green
  after removing an unused panic-only secret-key wrapper. The default w3f BLS
  backend also exposes fallible secret reload, signing, and public-key
  derivation helpers, both BLS backends expose checked keypair generation, the
  public backend helper names `keypair` and `sign` now return `Result`, both
  backends reject non-empty all-zero deterministic seed material before deriving
  a secret, and the w3f stored-secret `public_key` helper is fallible too.
  `KeyPair::try_random_with_algorithm`, `KeyPair::try_from_seed`,
  `Signature::try_new`, BLS PoP proving, and `PublicKey::from_private_key` now
  route through checked BLS paths so corrupted stored secret bytes become
  signing/key-generation errors on `Result`-returning APIs. BLS VRF proof
  construction now returns `Result`, rejects invalid stored
  secret scalars before signing for both Normal and Small variants, and uses
  checked compressed-proof decoding so malformed G1/G2 proof encodings fail
  closed without `CtOption::unwrap`;
  governance VRF candidate generation handles those errors directly instead of
  relying on `catch_unwind`, and the governance council CLI plus core/Torii
  fixtures now propagate the fallible BLS keypair/signing API directly. The
  public `PublicKey::to_bytes` compatibility helper delegates to the checked
  compact-key parser, keeping fallible public-key expansion live in
  BLS-enabled builds. Merkle leaf iteration now stops cleanly on an
  unexpected missing leaf slot instead of relying on panic-only internal layout
  assertions, and parent recomputation now stops if malformed in-memory state
  lacks a computed parent slot. Compact Merkle proof conversion and verification
  now share a fixed direction-bitset depth cap instead of converting
  `u32::BITS` through panic-only assertions, while decoded tree layout
  validation remains strict. The multihash `VarUint` codec now decodes through checked `u128`
  accumulation plus final bounded conversion, accepts valid max-width integer
  encodings, rejects oversized canonical varints including high final-chunk
  bits above `u128::MAX`, and constructs continuation bits without unchecked
  tail mutation. SoraNet SRCv2 certificate issue and
  verification now use checked CBOR serialization/digest helpers, with
  canonical integer emission and checked byte/text/array length conversion
  replacing panic-only encoder assumptions. Core `Hash` and `HashWriter`
  hashing now use the fixed-output Blake2b-32 digest type, preserving the
  historical digest bytes while removing panic-only variable-output
  initialization/finalization assumptions; Ed25519 and default w3f BLS
  verify-ok cache keys now use the same fixed-output Blake2b-32 route while
  preserving their domain-separated transcripts; Ed25519 public-key parse,
  public-key-full fast-cache, and exact verify cache index helpers now use
  checked little-endian chunk
  extraction and invalid cache-size fallback to index `0`, eliminating
  panic-only cache-index assumptions while preserving the configured
  power-of-two masks, and `Signature::verify` now routes compact public-key
  expansion through checked parsing so malformed in-memory public keys return
  `Error::Parse` instead of reaching Ed25519 invariant panics, and rejects
  non-empty all-zero signature payloads before backend verifier dispatch.
  `KeyPair::new`
  now validates compact public-key payloads through the same checked parser
  before algorithm comparison or GOST pair validation, so malformed in-memory
  public keys return `Error::Parse` instead of panic-compatible full-key
  expansion. Norito streaming key-update verification now extracts remote
  Ed25519 identities through checked compact-key parsing, so malformed
  in-memory identity keys fail as `HandshakeError::BadSignature` before
  signature verification, suite negotiation, or transport-key state changes.
	  BLS PoP verification, PoP proving, and PoP-gated aggregate public-key
	  collection now use checked compact-key extraction, so malformed in-memory
	  BLS public keys surface through `Error::Parse` before proof verification,
	  duplicate-key caching, or aggregate backend work. Public-key fallible string
	  encoders now validate compact payloads through full public-key parsing
	  before multihash formatting, so malformed in-memory keys return
	  `ParseError` instead of canonical-looking bare or prefixed strings.
	  `PublicKey` Norito serialization now reuses the cached full-key parser
	  before writing compact wire bytes, so malformed in-memory keys return a
	  Norito error and no exact encoded length instead of emitting invalid
	  archives. Direct `PublicKeyCompact` Norito serialization now applies the
	  same full-key validation before writing tag+payload bytes, so malformed
	  compact state cannot bypass the checked `PublicKey` wrapper. The private
	  compact-to-full conversion is now `TryFrom<&PublicKeyCompact>` and uses
	  checked tag/payload accessors, so malformed compact state returns
	  `ParseError` instead of relying on panic-only invariant accessors.
	  `KeyPair::new` also reuses the checked public-key payload for ML-DSA
	  pair validation instead of re-entering the compatibility
	  `PublicKey::to_bytes()` helper after compact parsing has succeeded.
	  `PublicKey::try_to_bytes()` is now public, giving downstream
	  `Result`-returning paths a checked algorithm/payload accessor without
	  relying on the infallible compatibility wrapper. The legacy signer-backed
	  SCCP EVM submission helper now uses that checked accessor when deriving
	  Secp256k1 signer public-key bytes, so malformed or non-Secp256k1 signer
	  state fails closed before address derivation. `PublicKey` hashing and
	  ordering now also use checked tag/payload extraction with a deterministic
	  raw compact fallback for malformed in-memory envelopes, so peer maps and
	  sorted target sets no longer reach the infallible compatibility accessor.
	  `PublicKey::try_algorithm()` now exposes checked tag access, while
	  infallible `Display`, `Debug`, and Norito JSON formatting emit a
	  deterministic invalid-public-key marker for malformed in-memory compact
	  envelopes instead of panicking. The `iroha_core` single-Ed25519 admission
	  precheck, parsed-key cache, and allowed-signing admission gate now use
	  checked public-key accessors for fast-path eligibility and signing
	  algorithm checks, so malformed in-memory compact public-key state misses
	  the optimization or returns a structured malformed-signature rejection
	  instead of touching unchecked key invariant accessors. Block
	  commit/signature subset validation, native AMX attestation signer checks,
	  vNext aggregate-certificate signer classification, lane-relay QC key
	  collection, consensus peer registration, active-roster filtering, and
	  admission-time signature batch prechecks now share checked
	  algorithm/payload extraction for consensus and transaction signer keys, so
	  malformed in-memory keys are rejected through existing signature and policy
	  error surfaces before BLS role checks, PoP lookup, or batch key-byte
	  collection. Account controller multisig policy construction, canonical
	  member sorting, CTAP2 policy encoding/digesting, and account-address
	  controller encoding now extract compact public-key payloads through checked
	  accessors, so malformed in-memory controller keys return
	  `MalformedPublicKey` or `InvalidPublicKey` on result-returning paths
	  instead of reaching compatibility invariant accessors. Trusted-peer PoP
	  config parsing, trusted-roster validation, daemon NPoS validator status
	  counting, genesis trusted-peer PoP verification, and Torii Sumeragi
	  BLS-key operator views now also classify BLS-normal keys through checked
	  accessors, turning malformed in-memory keys into config errors or
	  non-BLS status entries instead of compatibility accessor panics.
	  Restricted transaction-gossip target scoring and NPoS validator-election
	  tie-break scoring now also read peer public-key bytes through checked
	  accessors, falling back to the deterministic invalid-key marker for
	  malformed in-memory peer keys while preserving valid-peer score inputs.
	  SoraDNS resolver-directory signing payloads and Torii VPN quote response
	  metering-key hex rendering now also extract public-key payloads through
	  checked accessors, returning existing invalid-parameter/conversion-error
	  surfaces for malformed in-memory keys. SCCP EVM digest signing and Torii
	  SCCP proof-build diagnostics now also require checked Secp256k1 public-key
	  classification before EVM address/signature handling. Config parsing for
	  streaming identity, Torii receipt signer, and Torii offline issuer public
	  keys now also uses checked algorithm access before allow-list decisions.
	  Reusable core/Torii/config/client/SoraFS Rust fixtures now also extract
	  public-key payloads and algorithms through checked accessors, leaving the
	  targeted compatibility-accessor scan clean across those source roots.
	  Operator tooling and daemon paths for SoraDNS resolver signing payloads,
	  SoraNet relay/puzzle identity derivation, Kagami PoP/genesis helpers,
	  Taira canaries, Soracloud release governance proofs, CLI governance/account
	  controller display, and ephemeral Torii receipt-signer logging now also use
	  checked public-key accessors and propagate their existing error surfaces.
	  Taira write-canary generated signers now also use checked Ed25519 keypair
	  generation and surface OS entropy failures through the canary command
	  result path.
	  Oracle default reward/slash accounts now derive their fixed Ed25519 ids
	  through the checked seed-expansion helper while preserving infallible config
	  defaults.
	  The `iroha_genesis` manifest-normalize helper now generates its temporary
	  signing key through checked default key generation and reports entropy
	  failures with binary-specific context.
	  The `iroha_crypto` SoraNet handshake-check helper now derives its fixed
	  client/relay Ed25519 keys through checked seed expansion and reports
	  failures through the handshake harness error path.
	  Offline v1/v2 interop vector generators now derive their fixed issuer,
	  account, and note Ed25519 keys through checked seed-expansion helpers with
	  fixture-specific error context.
	  The `iroha` dev key-material example now generates its Ed25519 keypair
	  through checked randomness and propagates entropy failures from `main`.
	  The `iroha` Nexus app transfer and tutorial, `iroha_data_model`
	  signed-block/I105 vector, and `iroha_torii_shared` permissions-preimage
	  examples now also use checked Ed25519 generation or seed derivation,
	  surfacing entropy or fixture-key failures through their example `main`
	  result paths.
	  `iroha_js_host` N-API Ed25519/generic keypair exports and the relay
	  envelope sample now also use checked random generation or seed derivation,
	  mapping failures into N-API errors instead of relying on panic-only
	  keypair wrappers.
	  Offline deterministic escrow account derivation now also uses checked
	  Ed25519 seed expansion while preserving the fixed-seed infallible API.
	  Account-address vector and compliance-vector fixture public keys now also
	  use checked Ed25519 seed expansion while preserving their fixed seed bytes.
	  Norito fixture-export and trigger-print scripts now also derive their
	  fixed Ed25519 fixture authorities through checked seed expansion.
	  Generic Ed25519 deterministic key generation and private-key parsing now
	  reject all-zero 32-byte seed material before accepting caller-supplied
	  signing keys.
	  X25519 deterministic key generation, imported static-secret admission, and
	  OS-backed private-key generation now reject all-zero 32-byte
	  seed/private-key material before public-key derivation.
	  `iroha_test_samples` sample-account generation now exposes a fallible
	  helper and routes seeded/random test key material through checked
	  key-generation APIs.
	  `iroha_core` tx-size and memory examples now also use checked random key
	  generation, with `tx_size` surfacing entropy/keygen failures through its
	  example `main` result.
	  The custom data-model sample fault-injection smoke test now also uses
	  checked random key generation for its transaction signer.
	  Confidential keyset generation now accepts fallible `rand_core` 0.9
	  crypto RNGs and maps spend-key entropy failures to
	  `ConfidentialKeyError::RandomBytes`; confidential keyset derivation now
	  rejects all-zero 32-byte spend keys before HKDF expansion.
	  SoraNet client and relay handshake construction now also use fallible
	  `TryCryptoRng` draws for nonce, Noise secret, and client ML-KEM seed
	  material, returning labelled `HarnessError::RandomBytes` failures and
	  rejecting all-zero generated material before nonce, Noise, or ML-KEM seed
	  state can be emitted.
  SoraNet PoW and Argon2 puzzle ticket minting now also use fallible
  `TryCryptoRng` draws and preserve labelled nonce-generation failures
  through `MintError::RandomBytes` and the p2p challenge wrapper, with
  all-zero nonce draws rejected as inert random material.
  SoraNet admission-token minting and SoraFS proof-token minting now also use
  fallible `TryCryptoRng` draws and return labelled `MintError::RandomBytes`
  failures for admission-token nonce and proof-token id generation, including
  all-zero random draws.
  SoraNet request blinding nonce generation now also accepts fallible
  `TryCryptoRng` inputs and reports entropy failures through
  `BlindingError::RandomBytes`, while all-zero generated nonces fail through
  the existing weak-input gate.
  AEAD convenience encryption now keeps caller-supplied nonce compatibility
  unchanged while generated `encrypt_easy`/`encrypt_easy_into` nonces reject
  inert all-zero material through `Error::InertNonce`.
  P2P handshake hello construction now also extracts local peer key metadata
  through checked accessors and reports malformed local keys through a
  dedicated handshake error, while multisig members expose a fallible
  checked algorithm accessor for result-returning callers.
	  Python native bridge keypair export, account public-key hex, transaction
	  envelope public-key embedding, public-key multihash parsing,
	  public/private multihash formatting, SM2 fixture public-key formatting,
	  and SoraFS alias-proof fixture signer extraction now also use checked
	  public-key payload/formatting access and return Python errors on malformed
	  compact key state. SM2 typed formatter export, Connect C SM2 prefixed
	  formatting, JavaScript native generic/SM2 multihash helpers, Kagami
	  prefixed key JSON output, SoraFS manifest-sign key formatting, and ADDR-2
	  fixture multihash/prefixed fields now also use checked formatter APIs
	  before emitting operator or SDK-facing strings.
	  xtask SoraNet drill bundles, FastPQ manifests, Taikai anchor summaries,
	  OpenAPI manifests, SoraNet rollout captures, SoraDNS release signing
	  payloads, and SoraFS admission/pin fixture generators now also extract
	  embedded Ed25519 public-key payloads through checked accessors before
	  writing operator artifacts.
	  Offline note tests, ADDR-2 compliance vectors, and Offline V1/V2 interop
	  vector generators now also extract fixture public-key payloads through
	  checked accessors before embedding certificate, address, or offline FI
	  public-key fields. The remaining SoraFS conformance/chunker/pin/discovery
	  fixtures, gov draw fixtures, bridge proof vectors, config/test-network
	  assertions, dev key example, Swift parity generator, and offline-note
	  integration certificate helpers now also use checked public-key accessors,
	  leaving the compatibility-accessor scan confined to `iroha_crypto`
	  internals, tests, and benches. Inside `iroha_crypto`, BLS PoP fixtures,
	  generated public-key roundtrips, Ed25519 aggregate/batch fixtures,
	  ML-DSA/PQC fixtures, and the Ed25519 hot-path benchmark setup now also use
	  checked public-key payload extraction, while ML-DSA public/private
	  formatter roundtrips and SM2 public-key formatter fixtures now use checked
	  multihash/prefixed formatter APIs; `PublicKeyFull` normalization
	  internals now use a fallible borrowed canonical-payload path for formatter
	  encoders, and the blstrs typed BLS backend plus default w3f BLS
	  `PublicKeyFull` variants now borrow stored canonical public-key payloads,
	  clearing the targeted BLS formatter compatibility-accessor scan for both
	  backends.
	  X25519 public-key decoders for the hybrid KEM and standalone key exchange now
	  reject low-order encodings before ECDH through the shared standalone X25519
	  predicate, with standalone regressions covering every distinct
	  dalek-torsion-derived Montgomery encoding while retaining shared-secret
	  checks as defense in depth, and X25519 session-key derivation now maps HKDF expansion
  failures through the shared-secret `Result` path instead of using a panic-only
  assertion. SoraNet PQ ML-KEM key generation now exposes checked direct and
  seeded constructors, routes OS-backed keygen through key-pair validation, and
  hybrid X25519/ML-KEM `try_generate` consumes that checked path before
  reconstructing the hybrid secret. The public `HybridKeyPair::generate` helper
  now returns `Result` instead of panicking after checked generation; the public
  hybrid key-generation, encapsulation, and SoraFS hybrid payload envelope paths
  now consume fallible `TryCryptoRng` draws and return labelled RNG errors
  before key, ciphertext, or AEAD nonce material is emitted, while hybrid
  generated X25519 secret and ML-KEM seed draws now reject all-zero material
  before key generation or encapsulation can derive transport keys. The public
  direct and seeded
  `generate_mlkem_keypair*` wrappers now return `Result` instead of panicking
  after validation, and deterministic ML-KEM keygen/encapsulation reject
  all-zero `HedgedRngSeed` material before seeded RNG construction while
  ML-KEM caller/OS seed draws reject all-zero material before `*_from_rng`
  keygen or encapsulation and seeded encapsulation preserves invalid-public-key
  preflight order. Nonzero PQClean ML-KEM backend statuses now
  surface as `MlKemError::BackendFailure` through keygen, encapsulation, and
  decapsulation `Result` paths instead of panic-only assertions, and ML-KEM
  12-bit coefficient validators now reject partial byte groups as
  `BadEncoding` instead of relying on debug-only divisibility assertions.
  Kotlin/Java
  Connect X25519
  direction-key derivation now maps
  provider-level low-order agreement failures into
  `ConnectProtocolException` instead of leaking provider exceptions while the
  native Connect bridge FFI rejects the same low-order peer key without touching
  output buffers. Kotlin/Java Connect nonce, frame/envelope codec, and queue
  journal paths now reject negative signed sequence values, high-bit `uint64`
  frame/envelope sequences fail closed, and ciphertext-frame encoding
  explicitly uses the canonical zero-flag Connect Norito field layout.
  Kotlin Connect approval preimages now canonicalize `accountId` through the
  shared I105 account-literal helper before binding it into wallet
  authorization bytes, matching Java Android and rejecting domain-qualified
  aliases.
  Soracloud uploaded-model `X25519HkdfSha256` bundle
  admission now requires 32-byte recipient and ephemeral public keys and routes
  both through the same low-order decoder before registration. Confidential
  key hierarchy derivation now reports HKDF expansion failures through
  `Result`-returning helpers instead of panic-only assertions, and the CLI
  `create-keys` path now propagates those failures through normal command
  errors instead of a post-length-check `expect`. BFV identifier slot encoding
  and per-slot seed derivation now propagate conversion failures through
  `BfvError` instead of panic-only `usize` to `u64` assumptions, and BFV scalar
  modular helpers now avoid panic-only post-reduction integer conversions while
  preserving max-width modulus behavior. The RAM-LFE default programmed BFV
	  hidden program now uses profile-sized `u16` constants instead of panic-only
	  index conversion assumptions, and its memory RNG transcript binds `u64` step
	  values directly; BFV/RAM-LFE domain-separated digest, receipt, and RNG-seed
	  transcripts now stream hash chunks directly while preserving the previous
	  contiguous byte layout. BFV `RotateLeft` outer-slot step normalization now also uses
	  `u64` modulo arithmetic before converting back to `usize`, avoiding
	  target-width-dependent behavior for large public rotation-key step counts.
	  Programmed RAM-LFE BFV hidden-program admission now also caps v1 instruction
	  tapes at the canonical 64-slot, four-instruction shape before execution, and
	  requires `LoadConst`, `AddPlain`, `SubPlain`, and `MulPlain` immediates to be
	  canonical `F_257` values before public-program digests or programmed parameters
	  are admitted. The
	  feature-gated BFV acceleration selector now falls back
	  to deterministic scalar schoolbook multiplication for zero or overflowed
  derived convolution lengths, and its CRT-NTT helper path now rejects invalid
  operand lengths, unsupported NTT lengths, and CRT reconstruction overflow
  before using that same deterministic scalar fallback instead of relying on
  panic-only degree or NTT arithmetic. Confidential
  encrypted shield payloads now require supported versions, non-empty
  ciphertext, and low-order-free X25519 ephemeral keys before `Shield`
  execution burns public balance or records note commitments; CLI envelope and
  shield construction plus Connect/Norito bridge shield transaction encoders now
  run the same payload preflight before instruction construction, raw payload
  emission, or signing, and the Swift SDK fallback serializer applies matching
  empty-ciphertext and X25519 low-order admission.
  Standalone ML-KEM public-key validation, secret-key validation,
  encapsulation, and decapsulation now reject all-zero public keys, all-zero
  secret keys, all-zero embedded secret-key public keys, noncanonical 12-bit
  public-key coefficients, and noncanonical secret-key private coefficients,
  and secret-key validation plus decapsulation reject corrupted embedded `H(ek)`
  public-key hashes before implicit rejection can derive divergent transport
  keys. Hybrid envelope constructors and Norito streaming Kyber key-material,
  fingerprint, session, snapshot, encapsulation, and decapsulation admission now
  also reject all-zero ML-KEM public or secret key material before accepting
  fingerprints, transport state, or envelope keys, and Norito streaming
  generated X25519 ephemeral secrets plus GCK wrap nonces reject all-zero
  material before key-update or content-key update state is emitted.
  Changing the streaming ML-KEM profile on key material or live sessions now
  clears configured Kyber public keys, fingerprints, and local decapsulation
  secrets before any later HPKE use, and direct local ephemeral-payload
  precomputation no longer commits Kyber transport keys, negotiated-suite, STS,
  or snapshot state before a signed key update is built or accepted.
	  Norito streaming
	  X25519 key-update processing now requires prepared local ephemeral material and
	  applies the same low-order ephemeral-key preflight before transport-key
	  derivation or committing session state; X25519 ephemeral generation and
	  outbound content-key nonce generation now propagate OS RNG failures as
	  `HandshakeError::Randomness` instead of relying on the infallible RNG
	  compatibility wrapper; and
	  signed remote key updates verify signatures and stage key-counter, suite, and
  ephemeral-shape admission on a local copy before X25519 shared-secret
  derivation, ML-KEM decapsulation, transport-key derivation, resetting, or
  committing session state, and successful remote key updates now return the
  inserted transport keys directly instead of relying on a panic-only option
  readback. Outbound key-update
  construction also stages ephemeral generation, transcript signing, and Kyber
  transport derivation before committing session state, and rejects zero or
  same-session non-increasing counters before ephemeral generation. Direct Norito
  key-update state admission now rejects zero counters and suite/payload length
  mismatches before accepting counters, requiring 32-byte X25519 public keys or
  1088-byte Kyber768 ciphertexts. Streaming snapshot restore also rejects zero
  key counters before replacing live session state. Direct Norito key-update
  state restore/from-snapshot paths reject zero counters before replacing replay
  state. KeyUpdate and capability
  negotiation admission now reject zero protocol versions before committing
  suite, counter, transport-key, or ACK state. Capability reports must carry the
  viewer endpoint role before p2p or core ACK construction records negotiation
  state. Viewer-side capability ACKs must echo the report stream id, protocol
  version, negotiated DATAGRAM size, and DPLPMTUD flag before transport state or
  callbacks are updated. Direct Norito STS derivation
  now rejects non-32-byte
  handshake shared secrets before HKDF.
  Norito streaming
  content-key updates now authenticate and unwrap the GCK before recording
  accepted rotation state, and outbound content-key construction rejects
  regressed rotations before nonce generation or AEAD wrapping. Inbound,
  outbound, and restored snapshot GCKs must now be exactly 32 bytes, including
  direct Norito GCK wrap/unwrap helpers, and direct Norito content-key
  state restore/from-snapshot paths reject partial id/valid-from metadata before
  replacing replay state, so malformed wrapped keys or persisted keys cannot
  poison replay windows.
  Streaming snapshot
  restore also stages KEM-suite id validation, transport-key derivation, and
  Kyber public-key/fingerprint validation before replacing live session state,
  rejects partial content-key or Kyber metadata, and now binds Kyber768 suites to
  ML-KEM-768 snapshot metadata plus either the validated remote fingerprint for
  inbound state or the validated local fingerprint for outbound state, with local
  Kyber metadata requiring an installed decapsulation secret whose embedded
  public key and `H(ek)` public-key hash match before restore can replace state.
  Transport
  capability recording and snapshot restore
  now reject DATAGRAM/fallback shape drift before updating live session state or
  capability hashes. Streaming
  feedback admission now clamps inbound
  `parity_chunks`, receiver `parity_applied`, and `fec_budget` to the 6-chunk
  FEC ceiling, and caps inbound loss samples at Q16.16 100% before updating
  snapshot or outbound hint state. The first accepted feedback hint or receiver
  report now binds feedback state to that stream id, and later feedback frames
  with a different stream id are rejected before counters, EWMA loss, parity, or
  snapshot-visible fields change.
  SoraNet NK2/NK3
  handshake parsers now apply the same low-order X25519 policy to decoded Noise
  static and ephemeral public keys,
  reject malformed Dilithium3/Ed25519 handshake signature field lengths and
  all-zero signature payloads, require 1024-byte zero-padded frames, and reject
  selected KEM/signature ids that are absent from either peer's advertised
  capability TLVs, including the relay capability vector echoed in `RelayHello`;
  unsupported KEM ids fail at the KEM profile gate before downgrade telemetry is
  built.
  Relay capability advertisement and runtime GREASE append now check TLV payload
  lengths before writing the two-byte length field, and relay config validation
  rejects configured GREASE payloads that cannot fit that wire field.
  SoraNet signed-ticket signing now preflights ML-DSA-44 secret-key lengths,
  and signed-ticket decode/direct verification now reject ML-DSA-44 verifier
  public-key and signature vectors whose lengths disagree with the suite
  metadata, and all-zero signed-ticket signature material, before signing
  payloads, accepting tokens, or entering backend verification, while
  signed-ticket relay/transcript binding checks now run
  before signature work in the full verifier, and signed-ticket policy metadata
  now rejects unsupported versions, difficulty mismatches, expiry, and TTL
  window failures before signature work. Signed-ticket ML-DSA payloads now use
  a fixed-size buffer with explicit used length for the optional transcript
  binding while preserving the previous contiguous signed payload layout.
  SoraNet PQ helpers now validate ML-KEM
  encapsulation public-key lengths and ML-DSA signing context/secret-key
  lengths before drawing direct or OS-backed randomness for malformed inputs,
  and SoraNet runtime client-hello processing preflights NK2/NK3 client ML-KEM
  public keys before capability telemetry, relay Noise key generation, OS-backed
  ML-KEM key generation, or encapsulation; runtime handshake descriptor
  commitments and resume hashes now must be 32-byte transcript-binding fields
  before client RNG, relay RNG, transcript hashing, KEM key generation, or
  encapsulation, client/relay capability vectors now must fit the
  length-prefixed handshake field before client RNG or frame construction,
  transcript hashing now rejects capability vectors that cannot fit its fixed
  `u32` length field before hashing, len-prefixed handshake message parsing
  now reads frame fields through checked cursor ranges, capability TLV parsing
  reads headers and value spans through checked cursor helpers, and suite-list
  capability TLV re-encoding now rejects oversized values through
  `update_suite_list` before encoded capabilities are emitted; deterministic
  handshake fixture and telemetry signature rendering now uses checked base64
  output lengths and fallible slice encoding before returning `prefix:base64`
  witness strings. PoW
  ticket parsing now reads fixed fields through checked cursor helpers, ticket verification,
  signed-ticket verification, ticket
  minting, and Argon2 puzzle verification/minting now reject malformed
  descriptor, relay-id, or transcript binding field lengths before challenge
  derivation, solution search, Argon2 work, or public-key validation. PoW and
  Argon2 puzzle policy parameters now expose fallible constructors for runtime
  config loaders so zero minimum TTLs and inverted future-skew bounds fail
  closed without panicking, and their compatibility `new` constructors now
  return fail-closed policies instead of unwinding on invalid timing bounds. PoW
  ticket minting, Argon2 puzzle minting, and revocation-store insertion now
  reject malformed or all-zero raw signatures and unrepresentable expiry
  timestamps through checked `SystemTime` conversion. PoW challenge,
  solution-digest, and
  revocation fingerprints plus Argon2 puzzle challenges now feed BLAKE3
  incrementally while preserving the previous contiguous transcript layout, and
  Argon2 puzzle solution salts now use a fixed-size stack buffer. P2P SoraNet
  plus relay runtime construction now use those fallible constructors for
  config-derived PoW/puzzle bounds; relay
  replay-filter bit counts are now bounded before power-of-two rounding, and
  direct replay-filter construction plus `DoSControls::new` now propagate
  oversized filter shapes as `ConfigError::ReplayFilter` instead of reaching
  overflow-prone arithmetic. P2P
  QUIC/TCP happy-eyeballs dialing now records the first branch failure and
  returns the second branch failure directly when both dials fail, avoiding
  panic-only option readbacks in the fallback path. SoraNet CID
  blinding key derivation now rejects
  all-zero epoch salts or all-zero circuit secrets before HKDF, and
  request-scoped blinding nonce generation now reports RNG failures without
  panicking. SoraNet
  revocation-store reload now rejects duplicate persisted fingerprints, rejects
  overflowing expiry timestamps, and bounds loaded active records to the
  configured capacity. SoraNet guard-directory
  snapshots now reject duplicate or key-mismatched issuer fingerprints and
  enforce ML-DSA-65 issuer public-key length/phase requirements plus all-zero
  issuer ML-DSA public-key rejection at decode time, with issuer key shape,
  inert-key rejection, and the fingerprint `u32` key-length field now checked
  before fingerprint derivation; the public directory issuer-fingerprint helper
  now returns `Result`, rejects all-zero nonempty ML-DSA public keys, and
  orchestrator guard-directory admission maps fingerprint recomputation errors
  before advertised fingerprint comparison;
  relay directory build and snapshot rotation now propagate those
  fingerprint-computation errors with issuer context before signing or
  publishing a snapshot, and guard-pinning fixtures derive ML-KEM public-key
  lengths from the advertised suite instead of stale constants;
  snapshots also reject empty issuer or relay sets before trust-map construction
  or relay certificate verification, and guard-directory expected-hash config
  parsing is now fallible before snapshot hash comparison. SoraNet
  admission-token decode now reads fixed-width body fields and trailing
  signature spans through checked cursor helpers so malformed token prefixes
  return decode errors instead of relying on manual slice invariants; admission
  tokens now expose `try_encode`, and the compatibility encoder fails closed to
  a malformed frame when impossible direct token state cannot fit the v1
  signature-length prefix. Admission-token ML-DSA signing bodies now use a
  fixed-size stack buffer for the domain-separated body bytes shared by minting,
  verification, and token-id derivation while preserving the previous
  contiguous transcript layout. SoraNet
  admission-token replay-store
  reload now rejects duplicate persisted token IDs and overflowing expiry
  timestamps, and admission-token verification now rejects zero-length or
  inverted validity windows and preflights ML-DSA issuer public-key and
  detached-signature lengths before classifying full-length all-zero detached
  signatures, all before backend verification or replay-store mutation. Torii
  SoraFS stream-token issuance now
  generates token IDs through checked OS RNG fills and returns labelled issuance
  errors before signed token bodies are emitted; Torii internal operator-signature request headers now
  generate their base64url nonces through checked OS RNG fills and return
  labelled signing-header errors before canonical request signing, and ZK IVM
  prove job creation now generates public job ids through checked OS RNG fills
  before inserting async job state; Rust client account-signed multisig and
  operator-signed admin request headers now also generate their base64url
  request nonces through checked OS RNG fills and propagate entropy failures
  before request builders are emitted; SoraFS orchestrator guard-cache
  persistence now generates authentication-tag nonces through checked OS RNG
  fills and returns labelled persistence errors before tagged cache bytes are
  emitted, and Taikai cache-admission gossip bodies now generate replay nonces
  through checked OS RNG fills before signed gossip entries are emitted;
  SoraFS orchestrator fetch job IDs now use checked OS RNG fills and return
  `OrchestratorError::JobIdRandomness` before fetch telemetry or provider
  selection continues on entropy failure; local QUIC proxy browser-manifest
  session IDs and cache-tag salts now also use checked OS RNG fills and return
  `ProxyError::RandomBytes` before manifest previews or handshake
  acknowledgements are emitted; Torii MCP async job IDs and Connect session
  SID fallbacks now also use checked OS RNG fills and fail closed with
  JSON-RPC/tool errors before async job state or Connect requests are emitted;
  Torii operator-auth WebAuthn challenge bytes and session tokens now also use
  checked OS RNG fills and fail closed with operator-auth errors before
  challenge or session state is inserted; Torii Connect session app, wallet,
  management, and relay bearer tokens now also use checked OS RNG fills and
  fail closed with internal Connect-session errors before response tokens are
  emitted;
  embedded Soracloud
  uploaded-model X25519 upload-key persistence now generates the local static
  secret seed through checked OS RNG fills and returns a labelled `io::Error`
  before the key file is written; CLI SM2 keygen and confidential `create-keys`
  random seed paths now generate 32-byte seed material through checked OS RNG
  fills and return normal command errors on entropy failure; SoraFS CLI repair
  idempotency keys, storage-token nonces, GAR receipt IDs, and admission-token
  RNG seeding now use checked OS RNG paths and return command errors on
  entropy failure, while hybrid manifest envelope encryption uses the
  already-fallible `OsRng` path; Soracloud CLI mutation-auth signature nonces
  and staging temporary directory suffixes now use checked OS RNG fills and
  return command errors before request signing or staging on entropy failure;
  Rust client transaction nonces now use checked OS RNG reads through fallible
  `try_build_transaction*` APIs, and client submission plus CLI transaction
  creation paths propagate those entropy failures before submit; SoraNet PQ
  hedged seed construction now accepts caller-supplied `TryCryptoRng` seed
  entropy, and ML-DSA keypair/signing plus ML-KEM keypair/encapsulation OS
  helpers delegate through the same fail-closed required-seed boundary before
  deriving PQ material;
  transaction gossiper public/restricted shuffle seeds now derive
  deterministically from chain/local-peer/max-peer identity material and plane
  domains instead of reading process RNG during actor construction;
  telemetry future ids now use a process-local atomic counter instead of random
  ids; unseeded persisted-RBC chunk sampling now seeds `StdRng` through checked
  OS entropy and reports `SamplingError::RandomSeed` on failure while explicit
  seeds remain deterministic; proactive block-sync gossip now derives
  target-selection seeds from local-peer, height, gossip round, gossip size,
  candidate, and world-peer material instead of reading thread RNG; P2P connect
  scheduling and reconnect backoff jitter now derive bounded delays from
  domain-separated local-peer, remote-peer, address, and attempt-context
  material instead of reading thread RNG; Iroha core queue/storage tests now
  use deterministic counters for synthetic domain names, transaction hashes,
  and stress-test delays instead of process or thread RNG; operator-signature
  integration and Torii fixture helpers now use monotonic deterministic nonces
  instead of thread RNG in test-only signed requests; `iroha_test_network`
  peer selection now uses deterministic round-robin order instead of thread
  RNG; Iroha core memory-example synthetic asset/NFT values now use
  deterministic counters instead of process RNG; Izanami chaos keeps explicit
  seeds deterministic while routing unseeded `StdRng` setup through checked OS
  entropy and returning setup errors on entropy failure; CLI multisig
  auto-account registration now uses checked key generation and returns command
  errors on entropy failure; JS-host SM2 keypair generation now uses checked OS
  entropy through `Sm2PrivateKey::try_random_from_os` and returns N-API errors
  on entropy or key-generation failure; Rust SDK SM2
  `Sm2KeyPair::generate_with_distid` now uses the same checked OS helper and
  returns `ParseError` on entropy or scalar-generation failure;
  verifier
  construction exposes a fallible path
  that rejects malformed issuer public keys before fingerprint derivation or
  runtime state admission, and the compatibility constructor now keeps
  malformed issuer keys as fail-closed verifier state that is rejected during
  ML-DSA preflight before backend signature work or replay-store mutation. The
  relay runtime token-policy
  loader uses the fallible path for config-derived verifier keys while direct
  token-policy construction rejects missing issuer keys without panicking.
  Admission-token decode now also rejects unrepresentable
  `issued_at`/`expires_at` UNIX-second fields before downstream relay tools can
  attempt unchecked `SystemTime` conversion.
  Admission-token minting now preflights
  issuer ML-DSA secret-key length before nonce generation, body construction,
  or backend signing, and reports nonce RNG failures as typed mint errors.
  Relay VPN overlay construction now has fallible config
  accessors for billing meter hashes, helper-ticket secrets, backend endpoints,
  and TCP bootstrap secrets, and runtime startup uses the fallible overlay
  constructor before committing VPN state; relay VPN settlement artifacts,
  helper-ticket admission, and VPN accounting also saturate pre-epoch
  UNIX-millisecond conversion instead of panicking on a mis-set host clock.
  Relay incentive uptime/scheduled-uptime and verified-bandwidth epoch
  accumulators now saturate on overflow instead of panicking on extreme
  telemetry or proof totals. Relay adaptive PoW success/failure window counters
  and difficulty-step arithmetic now saturate before min/max clamping, avoiding
  panic-only overflow paths under extreme counters or oversized adaptive-step
  config.
  SoraNet SRCv2 bundle verification
  now also re-runs canonical certificate-payload admission for in-memory
  bundles, rejects weak Ed25519
  verifier keys, and preflights ML-DSA-65 issuer public-key and
  detached-signature lengths plus all-zero Ed25519/ML-DSA signature
  placeholders before backend verification, and local SRCv2 issuance reuses
  certificate-payload admission plus ML-DSA-65 issuer
  secret-key length preflight before signing bundles. Phase 2 SRCv2 rollout
  accepts Ed25519-only relay certificates while Phase 3 remains the
  dual-signature gate. SoraNet SRCv2 certificate decode now rejects unknown
  ML-KEM suite ids and key-material length drift for ML-DSA-65 identity keys
  and advertised ML-KEM relay public keys, rejects malformed/noncanonical/weak
  Ed25519 identity public keys, rejects all-zero ML-DSA identity and all-zero
  or noncanonical ML-KEM relay public-key material, rejects ML-DSA-65 detached
  signature length drift and all-zero Ed25519/ML-DSA signature fields, and its
  canonical CBOR parser rejects trailing certificate/bundle data
  plus non-shortest integer/length encodings and duplicate nested
  bundle/signature/endpoint/KEM-policy fields, with byte/text/exact payload
  reads routed through checked cursor helpers. SRCv2 validity-duration accessors
  now use checked signed timestamp subtraction, expose a checked route for
  callers, and fail closed to `Duration::ZERO` for directly constructed inverted
  or unrepresentable windows. Guard-directory relay entries
  now parse as SRCv2 bundles and must bind to a known snapshot issuer, the
  snapshot directory hash, and a unique relay ID, with relay certificate
  signatures verified against embedded issuer keys under the snapshot validation
  phase; zero-length or inverted snapshot validity windows now fail closed, and
  relay certificate validity must cover the full snapshot window without being
  published after the snapshot.
  SRCv2 role/capability bitmask decode now rejects unsupported bits
  instead of masking them away, and validity windows fail closed when inverted
  or published after expiry. KEM rotation policies now reject static
  fallback/rotation/grace metadata, staged policies without fallbacks, rolling
  policies without nonzero cadence, and preferred/fallback suite equality.
  Handshake-suite preference lists must be non-empty and duplicate-free, and
  endpoint URL lists must be non-empty and duplicate-free with non-empty,
  whitespace/control-free URL strings. Endpoint tags, when present, must also
  be non-empty, whitespace/control-free, and duplicate-free per endpoint.
- Keep hardening the ISO 20022 bridge after the new inbound lifecycle endpoints
  and durable outbox helpers for `pacs.002`, `pacs.004`, `camt.029`, `camt.056`,
  `sese.023`, `sese.024`, `sese.025`, and `colr.012`; remaining TradFi work is
  tracked in the engineering backlog for deeper XMLDSig/XAdES path-policy processing
  beyond the implemented trust-anchor, signer-admission, key-identifier, and
  revocation corridor; official XMLDSig/XAdES trust-anchor packages; CRL/OCSP
  or rail revocation-feed fixtures; complete canonical XML coverage; and
  broader MDR/XSD validation breadth beyond the checked-in live-profile fixture
  corridor, which now covers `pacs.002`, `pacs.004`, `camt.056`, `sese.023`,
  `sese.024`, `sese.025`, and `colr.012` payment, securities, and collateral
  lifecycle XML, including official-MDR XSD assertions for
  profile-advertised `pacs.004.001.09`/`pacs.004.001.10` and
  `camt.056.001.08`/`camt.056.001.09` return/cancellation variants. An offline
  XSD/XML fixture-manifest preflight now pins checked-in
	  schema target namespaces, `Document` payload roots, fixture namespaces, and
	  reviewed missing-schema exceptions, while requiring schema/fixture identifier
	  material and schema attribute names to remain printable ASCII before mismatch
	  diagnostics can quote them, rejecting copied XML fixtures with duplicate
	  fixture SHA-256 values, non-canonical schema/fixture path segments, and
	  optionally validating schema-backed XML fixtures against their checked-in XSDs
	  with `xmllint --nonet`; it also
  requires canonical repository/commit/path/license/source-SHA provenance for
  every checked-in XSD with source repository URLs and source paths capped at
  2048 characters, placeholder repository owners or names rejected during
  preflight and readiness replay, secret-looking repository coordinates
  rejected before archived-summary output, and non-ASCII or
  identifier-style secret-looking path material rejected before summary
  emission, while requiring the
  `blocked_schema_sources` review list to be recorded explicitly even when
  empty and to match a current fixture/schema gap or, with a profile catalog, a
  current profile-version gap,
  rejects XSD files with known restricted Standards
  Editor redistribution terms, parses the embedded default rail profile catalog
  on demand, and records which concrete advertised message versions are
  schema-backed while rejecting unknown profile/message catalog keys before
  release evidence is emitted; catalog `versions` lists can skip schema-backed
  checks only for the exact message-family alias, not arbitrary strings, and
  runtime-required catalog fields are required while optional catalog fields are
  shape-checked when present, including fail-closed trust/revocation pin overlap
  and bounded CRL/OCSP DER-sequence material checks; optional manifest/profile
  fields are optional only when omitted, so present `null` reviewed reasons,
  trust/revocation material lists, booleans, numeric caps, business-service
  arrays, or amount minor-unit arrays fail before digest-bound XSD/profile
  evidence is emitted.
  All checked-in
  XSDs now
  have standalone XML fixtures that pass XML schema validation, and remaining
  MDR/XSD work is locating redistributable official packages and making the
  strict schema-backed and profile-version release flags pass. Blocked public
  XSD candidate evidence now must carry at least one explicit redistribution or
  public-distribution restriction marker, so copyright-only provenance cannot
  satisfy missing-package blocker evidence. The legacy
  `colr.007` collateral parser and
  route are now local-compatibility only; operator receipt/evidence/readiness
  gates reject the explicit `--allow-legacy-colr007` override for production. An
  aggregate ISO production-readiness rollup now requires explicit expected
  provider/environment context, non-empty strict XSD proof, operator evidence
  summaries, and digest-bound direct receipt-archive verification with
  unique canonical per-receipt `*.receipt.json` paths, digests, and successful
  2xx receipt status plus kind-specific notary/rail metadata into one release
  gate; remaining readiness work is making that gate pass without diagnostic
  overrides and with real provider evidence.
  Durable ISO state now has
  versioned per-record digests plus a local
  tamper-evident audit index exposed through the
  `GET /v1/iso20022/audit/messages` route, with config-backed age/count
  retention/compaction, an `audit_export_dir` manifest/notary-preimage spool,
	  and an operator adapter that verifies and publishes those preimages to clean
		  raw-whitespace-free, canonical-host/label/port/path,
		  printable-ASCII-path, raw-delimiter/percent-smuggling-free,
		  overlong-URL/host-rejecting,
		  localhost/private-IP/rebinding-host/legacy-IPv4/IPv6-transition-rejecting,
		  duplicate-free HTTPS archival/notary endpoints with
				  regular non-symlink bounded exact runtime bearer-token files, rail drop
					  roots and inputs that reject symlink leaves and symlinked ancestors,
					  including explicit message leaves with
			  whitespace/leading-dash-segment/backslash/semicolon/empty-segment/dot-segment smuggling rejected
			  before reads and duplicate payload digests or duplicate rail message ids
			  rejected before network delivery, with live rail and notary redirects
			  archived as failed receipts instead of followed, and with live rail, notary, and archived
			  receipt endpoint URLs capped for full URL length and DNS-host length
			  and rejecting reserved placeholder hosts such as `.example` or
			  `example.invalid` plus raw, encoded, or non-ASCII path delimiter smuggling,
	  bearer-token files for the live rail and notary adapters capped before
	  decoding,
			  regular non-symlink notary export roots/source files with symlinked
			  ancestors rejected, 64 MiB caps for anchor/index JSON artifacts,
			  1 MiB caps for persisted record-source JSON artifacts, and local
			  receipts that do
			  not persist token material, reject secret-looking or control-bearing
			  successful remote response bodies before receipt persistence, redact
			  failed remote response previews and secret-looking or control-bearing
			  transport errors, and preflight receipt output
			  directories/leaves before input loading, publication, or Torii submission, rejecting
		  control characters, whitespace, leading-dash segments, backslashes,
		  semicolon parameters, URI/drive prefixes, malformed or smuggled percent escapes, empty segments, dot/parent traversal, symlinked existing
		  ancestors, hard-linked outputs, or symlinked outputs, and using
		  owner-private descriptor-checked same-directory temporary files
			  with bounded digest-derived names plus atomic replacement where
			  available, and live rail/notary adapter timeout and byte-cap CLI
				  values now fail closed on non-positive or non-finite inputs before
				  local reads or network delivery, every ISO operator CLI rejects
				  control-bearing unknown raw arguments before argparse diagnostics,
				  requires unknown raw arguments to remain printable ASCII,
				  preflights required notary/rail URL values for control,
				  non-ASCII, whitespace-padded, and non-URL-shaped secret-looking
				  material before unrelated local path checks,
					  keeps URL host labels printable ASCII before host/IP
					  numeric-label checks can accept Unicode digit confusables,
					  rejects archived canary command flags with secret-looking or
					  non-ASCII spellings before echoing unsupported flag names,
					  reports non-ASCII, overlong, too numerous, or collectively oversized unknown JSON keys with label-only diagnostics,
					  rejects non-ASCII receipt-kind spellings before unsupported-kind
					  diagnostics can echo them,
					  rejects non-ASCII trust embedded-signature policies before
					  unsupported-policy blockers can preserve them,
					  keeps trust source authority/version provenance printable ASCII
					  before direct or archived summaries can preserve it,
					  keeps direct and archived trust DER labels printable ASCII
					  before summaries can preserve them,
					  keeps provider/environment context labels printable ASCII from
					  canary/trust generation through evidence verification and release
					  readiness and reports context mismatches without printing observed
					  or expected values, rejects archived canary stage names with non-ASCII
					  confusables before unsupported-stage diagnostics can echo them,
						  keeps canary runbook paths and archived child-command paths
						  printable ASCII and within the 4096-character local path cap,
						  while readiness compact summary/config/receipt paths stay
						  within the stricter 2048-character archive cap
						  before release evidence can preserve path confusables,
					  the unsupported `--` argument terminator before trailing values can
				  bypass raw secret, boolean, path, context, or numeric preflights
				  or be echoed by argparse, disable argparse long-option abbreviation
				  so partial flag spellings cannot bypass exact preflight matching,
				  and live
				  adapter local diagnostic
				  flags now reject unused `--allow-insecure-http`,
				  `--allow-default-profile`, `--allow-legacy-colr007`, and
				  notary `--allow-missing-record-sources` before dry-run summaries
				  or network delivery; a
	  read-only receipt verifier now gates those receipts for canary use and emits
	  a digest-bound summary with per-receipt `receipt_sha256` entries while
	  rejecting unused local verifier overrides for failed receipts, insecure/local
	  endpoints, legacy `colr.007`, and missing rail profiles,
		  closing raw receipt and notary source schemas including duplicate-free nested audit records,
		  complete audit-index record key sets,
			  complete persisted record/context/metadata/history key sets for source replay and
			  4096-character clean metadata string caps across notary/receipt audit
			  indexes, persisted records, nullable context/metadata/history fields, and
			  rail sidecars,
			  4096-character direct trust-bundle generic string/OID-list, XSD
			  profile-catalog generic string/list, canary runbook generic
			  string/list, evidence replay clean string/list, and readiness
			  compact clean string/list caps before trust preflight, XSD profile
			  validation, planning, archive replay, or final readiness replay, with
			  embedded trust/profile DER base64 retaining its decoded-size guard,
			  final-readiness `xsd.repository_fixture_manifest` blockers for
			  summaries still generated from the checked-in ISO fixture manifest
			  unless the run is explicitly local diagnostic mode, plus
			  `xsd.repository_xsd_summary` blockers for archived summary paths
			  under the checked-in ISO fixture corpus, and
			  `xsd.repository_profile_catalog` blockers for archived
			  profile-catalog paths that point back at those fixtures,
			  evidence/readiness blockers for canary `config_path` values that still
			  point at checked-in `fixtures/iso20022/operator_canary/` runbook
			  templates, plus live canary preflight failures for non-plan
			  config/stage/explicit verifier receipt paths under
			  `fixtures/iso20022/` and evidence replay failures for executed or
			  planned child-command path flags that reintroduce those fixtures, plus
			  direct receipt-verifier and evidence-gate selector failures for
			  `--receipt` and `--receipt-dir` paths under `fixtures/iso20022/`
			  before discovery or child verifier launch,
			  evidence/readiness blockers for compact XSD/evidence/canary/trust
			  summary paths under repository ISO fixture coordinates,
			  trust-bundle source-path retention plus evidence/readiness blockers
			  for compact trust profiles that still point at checked-in
			  `fixtures/iso20022/trust_bundles/` templates,
			  rail receipt `source_path` retention plus receipt/evidence/readiness
			  blockers and adapter preflight failures for checked-in
			  `fixtures/iso20022/*.xml` payload fixtures,
			  notary receipt `anchor_path`/`store_dir`/`index_path` retention
			  plus evidence/readiness replay that binds `latest.notary.json` or
			  digest-addressed `anchors/<index_sha256>.notary.json` paths,
			  `messages.index.json` peers, and source stores into direct archive
			  metadata matching and rejects checked-in `fixtures/iso20022/`
			  anchor/store/index artifacts, with adapter preflight failures for
			  checked-in notary anchor/store fixture inputs,
			  Torii durable-store reload,
	  audit record filename/message-id bindings, Torii reload clean-string enforcement,
	  Torii reload filename/message-id binding, symlink-free regular-file-only Torii record
	  directory/loading, symlink-free Torii durable-output directories, bounded Torii
	  persisted-record persist/reload, endpoint-digest bindings,
	  timestamp/status consistency, required HTTP response digest/error metadata, bounded response
				  metadata with redacted-marker rejection for successful receipts, canonical receipt endpoint,
				  timestamp, canonical notary/rail source paths, including `store_dir`,
				  that are not flag-shaped, `.xml` rail payload leaves,
			  and whitespace-free rail
				  metadata identifiers, with live rail sidecars rejecting non-ASCII
				  or malformed `message_type` values before unsupported-message echo,
				  explicit `null` `profile`/`rail_message_id`
				  values, non-canonical profile IDs, and overlong or non-canonical
				  ASCII rail-message identifiers before submission, with oversized or
				  unknown-field sidecars rejected as malformed,
	  ASCII-only rail `message_type` digit validation across direct receipt
	  verification, evidence replay, readiness replay, and XSD profile catalogs,
	  ASCII-only XSD profile-catalog `message_def_id` and version validation,
	  overlong XSD profile-catalog profile IDs, enum values, and
	  business-service entries rejected before duplicate-ID, missing-schema-version,
	  unknown-value, or summary echo,
	  overlong XSD/XML schema and fixture identifiers rejected before mismatch echo,
	  overlong trust-bundle/evidence/readiness compact trust profile IDs,
	  override IDs, policies, and trust-source authority/version/timestamp
	  provenance rejected before trust replay, summary archive, or blocker echo,
	  plus generic evidence/readiness archive/canary kind, filename, or metadata
	  mismatch blockers that do not print receipt kind values, receipt leaf names, or invalid metadata tuples,
	  rail receipt metadata recording for nullable raw receipt fields and retained
	  archived receipt-summary identifiers, rail sidecar source bindings that reject
	  explicit-null optional metadata instead of treating it as omission,
	  notary anchor-path shape checks even when
	  source files are not required, notary anchor/index source bindings that
	  require regular non-symlink files, notary adapter publication that requires
	  `store_dir/messages` for non-empty anchors by default, persisted notary
			  record-source bindings from each explicit index `records[]` row's
			  `record_sha256` to clean `store_dir/messages` paths, production evidence rejection of the adapter's
		  local `--allow-missing-record-sources` diagnostic override,
		  status-history timestamp binding for persisted record sources, and
		  symlink-free receipt archive directories,
	  and a strict JSON-runbook canary runner rejects
  whitespace-padded or control-bearing runbook strings, non-ASCII runbook path strings, present-null optional
	  path/numeric limit fields, embedded-whitespace/leading-dash-segment/backslash/semicolon/dot-segment
	  path smuggling including raw URI/drive prefixes, encoded control/space
	  bytes, encoded dot/separator bytes, encoded semicolon parameters, encoded
	  URL delimiters, encoded percent bytes, malformed bracketed hosts, overlong endpoint URLs or DNS
	  hosts, localhost/private-IP/rebinding/legacy-IPv4/IPv6-transition endpoint URLs, and duplicate endpoint and receipt inputs before
	  executing the rail/notary/verify path with one
	  bounded summary, bounding each child stage with positive finite
	  `--stage-timeout-secs`, recording `timed_out` for killed children, draining
	  child stdout/stderr through a configured preview cap, treating preview
	  truncation, unsafe control-character previews, and successful child stderr
	  as failed canaries, requiring explicit
	  notary and verify receipt-selector arrays under
	  `--require-explicit-policy`, and capping runbook
	  JSON at 64 KiB before parsing.
	  The operator scripts reject duplicate JSON object keys, non-standard
	  `NaN`/`Infinity` JSON constants, and lone UTF-16 surrogate escapes across
	  runbooks, sidecars, anchors/indexes, receipts, trust bundles, XSD
	  manifests/profile catalogs, evidence summaries, readiness summaries,
	  embedded receipt-verifier stdout, and direct archive receipt-verifier stdout
	  before semantic validation, so shadowed keys, non-finite numbers, and invalid
	  Unicode strings cannot rewrite release evidence. Direct numeric CLI
	  preflights also reject Unicode digit confusables before Python parsers can
	  accept them as timeouts, byte limits, or evidence age budgets. Those gates
	  also reject symlinked or
  non-regular canary runbooks, trust bundles, evidence/readiness summaries, XSD
	  manifests, profile catalogs, schema files, and XML fixtures before digest,
	  provenance, or policy checks run, opening those inputs through no-follow file
	  descriptors where available. Summary/profile/receipt output paths now also
	  reject checked-in `fixtures/iso20022/` artifact destinations during
	  run-level preflight and again before parent creation or temporary output
	  writes. Production-readiness direct `run(args)` calls now also preflight
	  XSD summary, evidence summary, and summary-output path smuggling before
	  input loading while keeping checked-in fixture summary inputs as structured
	  release blockers; direct XSD/trust verifier `run(args)` calls also
	  preflight manifest/profile-catalog, bundle, profile-output, and
	  summary-output path smuggling before manifest or bundle loading, and
	  direct canary/rail/notary adapter `run(args)` calls mirror their CLI
	  path-smuggling guards before config, inbox/export, receipt, token, or
	  network loading. Live rail/notary adapter runs also reject inbox/export
	  roots under checked-in `fixtures/iso20022/` artifacts before discovery,
	  fixture parsing, or network delivery. Direct CLI artifact paths for live rail inbox
  roots, live notary export roots, rail/notary bearer-token files, canary
  configs, trust bundles, XSD manifests/profile catalogs, receipt
	  files/directories, canary/trust summaries, and XSD/evidence summaries reject
	  control characters, whitespace, leading-dash segments, backslashes, semicolon
	  parameters, empty segments, and dot/parent traversal before argparse `Path`
			  normalization or file discovery, and direct local CLI/output/artifact
			  path strings are capped at 4096 characters before secret scanning,
			  filesystem expansion, summary emission, child command construction, or
			  archive replay. Live rail/notary adapter timeouts also
		  reject non-positive or non-finite CLI values, and byte caps reject
		  non-positive values before local reads or network delivery. The receipt
		  verifier caps raw receipt JSON at 4 MiB, notary anchor/index JSON at
		  64 MiB, persisted notary record-source JSON at 1 MiB, rail source XML
		  at 4 MiB, and rail source-sidecar JSON at 16 KiB before source replay,
			  while the notary adapter and downstream evidence gates require positive
			  notary record counts, canonical audit-index lifecycle states, and
			  state-compatible pacs.002 summary/status-history codes before publication
			  and during source-file or production-evidence replay. The
		  evidence gate caps
	  direct receipt-verifier stdout/stderr at 4 MiB before JSON parsing, bounds
	  direct verifier runtime with positive finite `--receipt-verifier-timeout-secs`,
	  redacts secret-looking or control-bearing verifier stderr diagnostics, and
	  rejects control characters, whitespace, leading-dash segments, backslashes,
  semicolon parameters, empty segments, dot/parent traversal, and symlinked
  receipt, summary, and emitted profile-override outputs before writing them.
  Canary summary outputs and runbook artifact paths
  are preflighted before subprocess stages, and canary relative paths preserve
  final leaves after parent containment checks so child scripts can still reject
  symlinked leaves. Archived canary/trust summaries consumed by the evidence
	  gate and XSD/evidence summaries consumed by the readiness gate are capped at
	  4 MiB before parsing, optional `xmllint` stdout/stderr is capped at 64 KiB
	  and runtime is bounded by positive finite `--xmllint-timeout-secs` capped
	  at 300 seconds during
  XSD fixture validation, with successful validator output limited to empty
	  output or the normal `<fixture> validates` line and secret-looking,
	  control-bearing, or non-ASCII validator diagnostics redacted, operator trust-bundle JSON is capped at
  64 MiB before trust-preflight parsing, and trust DER base64 is capped before
  decoding to the 1 MiB DER material limit while requiring each DER object to
  declare a matching SHA-256 digest. Trust-bundle preflight now requires an
  explicit `embedded_signature_policy` instead of inferring `require-verified`
  from omission, and every list-typed trust-material field must be recorded as
  an array so intentionally empty pin/DER collections cannot be confused with
  omitted production evidence.
  An offline
  evidence gate now requires exact expected provider/environment context,
  records that context in its digest-bound policy, recomputes
  canary/trust/receipt summary digests, rejects repeated or copied
  canary/trust summaries, rejects non-canonical or duplicate receipt paths or
  receipt digests, rejects duplicate archived trust profile IDs and bundle
  digests across trust summaries, and rejects
  plan-only, dry-run, control-bearing or whitespace-padded child-command entries,
  child-command arrays that do not start with the runner-emitted Python
  interpreter with ASCII-only numeric version suffixes plus expected stage
  script path or that carry extra positional arguments after that prefix,
  non-canonical or command-mismatched rail/notary receipt directories,
  verify commands that omit generated rail/notary receipt directories,
	  insecure-HTTP, default-profile, secret-leaking,
	  smuggled, raw-whitespace-bearing, empty/zero/leading-zero/malformed/default-port,
	  non-canonical-host, invalid-label, localhost/private-IP/rebinding-host,
	  legacy-IPv4, IPv6-transition, percent-escape, non-ASCII-path,
	  numeric-host-spoofed, or traversal-bearing URLs,
	  non-canonical canary runbook config paths, unknown upstream summary fields
	  plus live adapter, receipt, trust-bundle, and XSD JSON fields without
	  echoing control-bearing key names, secret-bearing audit-index/source
	  strings and source paths during notary publication or archived receipt
	  replay, recursive unsafe-control strings,
	  synthetic-trust, record-only, or receipt-verifier-output-free evidence before
	  archival, and requires trust-summary and receipt-summary policy booleans,
	  trust profile JSON emission booleans plus a digest recomputed from archived
	  profile overrides, trust revocation booleans/counts, bundle SHA-256 values,
	  duplicate-free supported receipt-kind lists and compact receipt entry kinds,
	  per-receipt `ok=true` plus 2xx `status_code` success metadata,
	  kind-specific compact notary anchor/index/count and rail
	  message/profile/payload metadata,
	  exact direct-archive receipt digest/kind/status/endpoint-policy/metadata binding to canary summaries, no copied
	  receipt paths or digests reused across canary summaries,
	  and plan-only status booleans to be
	  present explicitly so omissions cannot become production defaults. Archived
	  profile overrides must also keep
  matching profile/rail/policy identities, canonical policy OIDs and CRL/OCSP
  bounded canonical base64 DER SEQUENCEs, material-count agreement, CRL/OCSP DER
  digest/byte-length agreement, and non-overlapping trusted/revoked pins.
  Canary summaries must also prove the runner used
	  `--require-explicit-policy`, recorded complete stdout/stderr previews for
		  every executed child stage without unsafe control characters or
		  identifier-style secret-looking material, and avoided duplicate singleton child-command
		  flags, boolean child-command flags spelled with `=value`, and non-positive
		  or non-finite numeric child-command values plus Unicode digit confusables
		  in floating timeout flags, non-ASCII or non-canonical child-command path values,
		  unsupported positional command entries, wrong stage script
		  prefixes, or missing required child-command inputs. Trust-bundle preflight
		  now treats profile override emission as production-only: complete
			  source authority/version provenance is required, and `--emit-profile-json`
			  refuses local-audit record-only or insecure-source overrides plus
			  placeholder source metadata, missing source freshness budgets, or stale
			  source retrieval timestamps before writing profile JSON, then records the
			  selected `max_source_age_days` budget in the trust summary. It also
			  rejects unused local-audit `--allow-record-only`,
			  `--allow-insecure-source-url`, and `--allow-synthetic-der` flags unless
			  a verified bundle actually carries matching non-production policy,
			  insecure source URL, or synthetic DER evidence, while stripping the
			  private synthetic-DER marker from emitted summaries. The evidence gate requires
	  explicit freshness
	  budgets for canary, trust-summary, and trust-source evidence plus direct
		  receipt archive verification covering canary receipt digests and receipt
		  kinds before archival, rejects an unused `--allow-plan-only` override
		  unless at least one canary summary records `plan_only=true`, rejects
		  `--allow-partial-canary` unless at least one canary summary is missing
			  a rail or notary stage, rejects unused legacy/default-profile receipt
			  overrides unless compact rail receipts actually carry legacy
			  `colr.007` or missing profile evidence, rejects unused
			  record-only/synthetic/missing-source trust overrides unless compact
			  trust summaries carry the corresponding diagnostic trust material,
			  binds compact record-only and insecure-source trust policy flags to
			  actual non-production signature policy or `http://` or local/private
			  source provenance per trust summary,
			  rejects unused dry-run, failed-receipt, insecure-HTTP, and receipt-source-missing diagnostic
			  overrides unless the archived canary command, receipt summary, or trust
			  summary carries that policy or a receipt summary records
			  `require_source_files=false`, requires failed-receipt policy to bind to
			  a failed receipt entry rather than a summary flag alone, requires
			  insecure-HTTP receipt policy to bind to compact
			  `endpoint_requires_insecure_http` evidence, requires executed
			  rail/notary child commands to carry the matching
			  `--allow-insecure-http` flag plus matching compact receipt-kind
			  endpoint evidence, requires executed rail default-profile and
			  legacy `colr.007` commands to carry matching compact rail receipt
			  evidence for the same diagnostic condition, requires executed
			  rail/notary stage names to match compact `receipt_kind` evidence so
			  partial canaries cannot borrow receipts from absent stages, scopes
			  verify-stage `--receipt-dir` values to the recorded rail/notary stages
			  for executed and plan-only canaries, scopes direct verify-stage
			  `--receipt` files under recorded stage receipt directories, rejects hidden endpoint
			  evidence when the summary flag is false, and binds
			  canary verify-stage
			  receipt-verifier command flags to captured receipt-verifier JSON policy
			  booleans, with production-readiness replay rejecting compact
			  failed-receipt, insecure-HTTP endpoint, legacy `colr.007`, and
			  default-profile policy flags that contain no matching receipt entry,
			  rejects the canary-stage-only diagnostic
		  override when direct `--receipt` or `--receipt-dir` archive inputs are
		  supplied, preserves compact trust bundle SHA-256, source
		  authority/version and URL/retrieval provenance, trust source freshness
		  emission budgets, source trust-verifier diagnostic flags,
		  rejects an unused `--allow-profile-json-not-emitted` override unless
		  at least one trust summary records `profile_json_emitted=false`,
		  revoked-certificate pin
	  counts, certificate-policy OID counts,
	  CRL/OCSP material-class proof, and compact
	  trust-anchor/revoked/CRL/OCSP DER proof digests, byte lengths, and
	  cross-role uniqueness for release review, rejects profile-emittable drift and
	  emitted-but-not-emittable contradictions against the archived trust source
	  policy,
	  and the aggregate readiness gate rechecks that proof plus the evidence policy
		  context, requires explicit freshness budgets for
		  XSD/evidence/canary/trust/trust-source timestamps, blocks stale
		  digest-correct summaries and archive freshness policies weaker than the final
			  release budgets, rejects stale, placeholder, or smuggled compact trust
			  source provenance including `dummy`, `fake`, `placeholder`,
			  `replace-before-production`, `sample`, `template`, reserved hosts
			  such as `.example`, `example.com`, `example.net`, `example.org`, and
			  `example.invalid`, overlong URLs, invalid or overlong host labels,
			  non-ASCII host labels, numeric-host/legacy-IPv4 spoofing, IPv6 transition embedded-IPv4
			  smuggling, percent-escape smuggling, and omitted,
			  malformed, or release-weaker trust source freshness budgets, rejects
			  omitted trust-bundle source provenance separately from explicit null
			  source objects,
			  compact profile-emittable drift or emitted-but-not-emittable
		  contradictions against trust source policy or replayed trust-verifier
		  diagnostic flags, reports explicit diagnostic `source: null` compact
		  trust profiles as blockers while keeping omitted source keys malformed,
		  requires canary-stage-only evidence to record explicit
		  `receipt_verification: null` plus the matching archived
		  `allow_canary_stage_receipts_only` policy flag instead of omitting the
		  archive field or forging production policy, while still blocking that
		  policy flag when forged direct archive verification is present, rejects
		  unused final-readiness `--allow-reviewed-xsd-gaps` and
		  `--allow-canary-stage-receipts-only` overrides unless a reviewed XSD warning
		  beyond an unreviewed profile-version gap or canary-stage-only receipt
		  evidence is actually present, and
	  rechecks compact trust profile JSON emission and digest, CRL/OCSP revocation
	  posture, direct archive/canary receipt digest/kind/status/response-body digest/endpoint-policy/metadata binding,
	  empty successful direct-verifier stderr, trust
	  profile-count binding, and label-only missing-trust coverage blockers that
	  do not print compact receipt profile IDs or canary environment labels,
	  while rejecting repeated or copied
  XSD/evidence and compact canary/trust summaries, rejecting nested
  canary/trust/receipt/profile replay across evidence summaries, requiring compact
	  canary/trust source paths to be control-free, trim-free, not flag-shaped, and
	  traversal-free `.json` summary files, requiring compact canary runbook config paths to remain
	  traversal-free JSON pointers, requiring compact canary stage names to remain
	  unique production stages in rail/notary/verify order, rejecting raw canary
		  summaries that carry both executed and plan-only stage branches and
		  non-null successful-stage `reason` fields,
				  accepting plan-only compact summaries only with empty `stage_windows`,
				  explicitly recorded null `receipt_summary`, and canary runbook
				  planning plus planned verify commands that cover every non-dry-run
				  planned rail/notary receipt directory with unique stage receipt
				  directories, null verify-stage `receipt_dir` fields, raw plan-only
				  `dry_run` booleans that match the planned child command flags, and
				  unique, non-overlapping receipt selectors so they become production
				  blockers instead of malformed executed-evidence claims,
		  requiring summary digests, rejecting duplicate receipt paths or receipt digests,
	  rejecting rail/notary source path or source digest replay across canary summaries during evidence verification and across distinct evidence summaries during readiness replay,
	  rejecting non-canonical compact receipt paths, rejecting duplicate compact
	  trust profile IDs or bundle digests across trust summaries, rejecting control-bearing or whitespace-padded
  compact identity strings, rejecting non-canonical compact trust profile IDs,
  and rejecting compact trust rail IDs outside `generic-iso20022`,
  `swift-cbpr-plus`, `fedwire-funds`, `sepa-sct-inst`, and `securities-csd`,
  rejecting unknown compact evidence fields,
  rechecking XSD schema/fixture summary arrays for count, digest, and
  cross-summary replay, schema-path/message-id, fixture-path segment canonicality, canonical fixture
  schema-reference strings, and schema-reference consistency,
  rejecting DTD/entity declarations before schema or fixture XML parsing,
  rejecting ambiguous schema `Document` declarations or prefixed `Document`
  type spoofing, rejecting payload `ref` indirection, weakened payload
  occurrence attributes, prefixed payload types, and missing or duplicate
  payload complex types or payload complex types without exactly one direct
  sequence, rejecting XSD composition and foreign-namespace direct children in
  schema/`Document`/payload structures, rejecting schema roots with attributes
  beyond `elementFormDefault` and `targetNamespace`, rejecting fixture
  `Document`/payload root attributes, binding parsed XML and summary digests to
  the same checked bytes, capping manifest JSON and profile catalog source at
  4 MiB and schema/fixture XML inputs at 8 MiB before parsing,
	  requiring XML schema-validation proof for every
	  schema-backed fixture, rejecting unknown XSD summary fields, recomputing
	  schema-only flags/reasons and reviewed gap-list paths/reasons from the schema/fixture
	  relationship while rejecting padded, control-bearing, non-ASCII,
	  secret-looking, or overlong reviewed reason strings plus present
	  empty/non-string archived reviewed reasons in both the XSD preflight and readiness rollup,
		  rejecting stale missing-schema reasons
			  on schema-backed archived fixtures, rejecting embedded
			  non-ASCII characters, overlong source or relative paths, overlong
			  archived XML identifiers, whitespace,
			  leading-dash path segments, semicolon path parameters, URI/drive prefixes,
			  or malformed/smuggled percent escapes in checked-in XSD source provenance,
			  rejects omitted checked-in and blocked-source `source` keys separately
		  from explicit null source objects,
		  manifest schema, fixture, fixture schema-reference, and archived
	  profile-catalog paths during preflight and archived-summary readiness
	  rechecks, requiring archived summaries to retain the emitted manifest
	  path and explicit profile-catalog object/null state, binding archived schema
	  namespaces, fixture schema message ids, and fixture payload roots back to
	  their referenced schemas, requiring
	  profile-catalog source and embedded JSON
  digest provenance from exactly one active Rust `DEFAULT_PROFILES_JSON`
	  raw-string declaration plus duplicate-free profile/message/direction/version shape
	  and canonical skipped family-version aliases
	  with unknown source catalog keys rejected by the XSD preflight and
	  runtime catalog-field shapes checked before summary emission, requiring
	  profile-catalog enum and list values to remain printable ASCII before
	  unknown-value diagnostics or summary recording, rechecking
	  canonical profile ids, ISO family message types, allowed directions, and
  message-definition family binding in consumed summaries, and schema-backed
  proof for advertised concrete message versions, recomputing
  profile-catalog missing-version lists and represented profile-id counts, and
  requiring timezone-aware non-future XSD/evidence/trust verification
  timestamps and ordered canary and non-overlapping per-stage start/finish
  windows for final evidence traceability. Compact stage-window names must
  match the recorded stage sequence, and compact stage names are rechecked as
  unique production rail/notary/verify stages. Compact canary/trust summary
  paths, canary config paths, receipt paths, and child receipt-directory
  arguments reject embedded whitespace, leading-dash path segments, semicolon
  path parameters, empty segments, raw backslashes, and traversal segments before aggregation. The repository also
  carries plan-valid templates for Swift CBPR+, Fedwire Funds, SEPA SCT Inst,
  and securities CSD operator canaries. Remaining persistence work is
  provider-specific live service canaries and vendor evidence that passes the
  aggregate production-readiness gate.
	  ISO rail ingress now has
	  an operator file-drop adapter that verifies sidecar-pinned message
		  type/profile/payload digests, rejects unsupported or non-ASCII rail
		  message types, non-canonical sidecar profile IDs, and non-canonical
		  rail-message IDs, and closes the live sidecar schema while bounding sidecar JSON before
	  submitting to clean Torii base URLs and writing receipts, plus the same
	  receipt verifier and runbook runner for canary evidence.
  Remaining rail-connectivity work is provider-specific live gateway canaries
  and archived rail evidence. Live
  securities lifecycle profile admission now checks
  local ISIN/CUSIP, MIC, BIC/LEI, CSD venue, settlement-account, and cash-leg
  snapshots before durable `sese.023` recording; remaining securities work is
  live-rail adapter coverage around production CSD/account/cash-leg sources.
  `require-verified` profiles now require profile-specific public-key pins or
  linked terminal CA trust-anchor DER SHA-256 pins before a P-256/SHA-256
  enveloped signature can pass, with deterministic leaf/issuer
  distinguished-name binding, non-CA leaf enforcement, critical leaf
  `keyUsage`/`digitalSignature`, critical issuer CA basicConstraints, critical
  issuer `keyUsage`/`keyCertSign`, issuer path-length constraint enforcement,
  required certificate-policy continuity through intermediate CAs below the
  terminal trust anchor, fail-closed rejection of policy mappings, policy
  constraints, and inhibit-any-policy extensions,
  bounded duplicate-free `X509Data` chains, certificate-chain
  ECDSA-with-SHA256/secp256r1 enforcement with uncompressed P-256 SEC1 SPKI
  bytes, unsupported-critical-extension rejection, and validity-at-signing checks
  for X.509 chains, explicit certificate revocation pins, configured and
  signature-scoped embedded CRL/OCSP signer revocation checks evaluated against verified XAdES
  `SigningTime` or BAH `CreDt` rather than local wall clock, plus an offline
  trust-bundle verifier with semantic DER-shape checks, required clean
  provenance URL and retrieval-time fields, trim-free source
  authority/version provenance for archives, duplicate-label rejection,
  DER-object digest keys
  that fail closed when present as `null` or another non-string value, omitted
  absent labels in trust summaries, archived-summary `label: null` rejection,
  repeated-separator URL path rejection, and
  repeated-path/copied-bundle/duplicate-profile rejection, canonical lowercase
  trust profile ID enforcement, known ISO rail ID enforcement, plus
  profile-family templates for operator PKI preflight. The templates are
  schema/CI scaffolding
  only, require an explicit synthetic-template flag, and cannot emit profile
  overrides; remaining trust work is replacing them with official rail packages
  and archiving live provenance evidence that passes the production evidence
  gate. Canonical
  lowercase SHA-256 trust/revocation pin admission, shortest-form DER
  length/minimal positive-integer admission for parsed OCSP responses,
  fail-closed rejection of unsupported OCSP response/single-response extensions, low-S
  fixed-width P-256 `r || s` or low-S DER ECDSA signature-value decoding, and a
  deterministic supported canonical XML subset that expands empty elements,
  normalizes attribute quotes, and sorts namespace
  declarations plus unprefixed, declared prefixed, and implicit `xml:` namespace
  attributes while omitting the fixed legal `xmlns:xml` declaration from
  canonical output, decoding predefined/numeric XML character references and
  applying root namespace declarations inherited from an enclosing XMLDSig `Signature`
  according to the declared C14N mode: all inherited root declarations for
  inclusive C14N and only visibly used inherited root declarations for exclusive
  C14N. Non-empty same-document payload References must strictly enclose the
  verified signature carrier so partial subtree signatures cannot authenticate
  unsigned payment fields. Payload References may now add one final supported
  C14N transform after the required enveloped-signature transform to drive
  digest canonicalization.
  XMLDSig method and transform elements remain parameter-free and fail closed on
  non-whitespace child content such as `InclusiveNamespaces`, XPath, HMAC, or
  digest parameters; critical method elements must appear exactly once, Reference
  transforms must be enclosed in exactly one attribute-free `Transforms` wrapper,
  only implemented ordinary attributes are accepted (`Algorithm`, payload
  Reference `URI`, and XAdES Reference `URI`/`Type`), those policy attributes
  are read by exact XML attribute name only, and supported Reference
  children must remain ordered as `Transforms`, `DigestMethod`, then
  `DigestValue`; top-level `Signature` and `SignedInfo` children must also stay
  in the supported XMLDSig order. Payloads may contain exactly one supported
  signature carrier: either a bare XMLDSig `Signature` or an ISO `Sgntr` wrapper
  with exactly one direct XMLDSig `Signature` child. Any additional
  `Signature`/`Sgntr` element outside the verified carrier fails closed.
  Prefixed XMLDSig structural elements must
  resolve to the XMLDSig namespace across the supported Signature, SignedInfo,
  Reference, digest/transform, and KeyInfo subtrees, and supported XML element
  spans require exact qualified-name matches between opening and closing tags.
  Selected structural QNames must also pass the supported XML name policy, so
  malformed local-name matches such as double-colon XMLDSig tags fail closed
  before namespace, child-shape, digest, or signature handling continues.
  Unprefixed XMLDSig/XAdES structural elements reject explicit default
  namespaces that conflict with the supported XMLDSig or XAdES namespace.
  Required base64 values are singleton attribute-free text leaves without nested
  markup or comments; `PublicKey`/`X509Certificate` credential leaves follow the
  same no-markup rule. Public-key material cannot be mixed with
  `X509Certificate` material in one `KeyInfo`; key material must be scoped to
  exactly one structured `KeyInfo` using either `KeyValue/ECKeyValue` with
  P-256 `NamedCurve` whose `PublicKey` bytes parse as an uncompressed P-256
  SEC1 point, or one bounded duplicate-free `X509Data` wrapper, with
  unsupported direct child elements, unsupported ordinary attributes, and
  non-whitespace wrapper text rejected. The XAdES `SigningCertificateV2` subset
  uses a non-empty,
  duplicate-free ordered prefix of the verified certificate-chain digests with
  attribute-free direct `Cert`/`CertDigest` wrappers, `DigestMethod` with only
  `Algorithm`, and singleton attribute-free signed `SigningTime` text; prefixed
  XAdES structural elements must resolve to the ETSI XAdES v1.3.2 namespace.
  It still fails closed for inherited namespace context beyond root
  declarations, unbound prefixed attributes, reserved namespace rebindings,
  CDATA/CDEnd tokens, uppercase `#X` numeric character references,
  DTD/general/custom entity expansion, and all other XML outside the
  implemented subset.
- Keep UI-side SCCP proof-generation SDK inputs fail-closed for ambiguous
  aliases; the current TON shard-state source-state path rejects duplicate
  camelCase/snake_case names inside nested validator-set transition proofs,
  including the transition-signature hash committed into the transition-chain
  witness.
- Keep SoraFS paid-pin SDK builders fail-closed before submit. Java/Kotlin
  pin-manifest builders now validate manifest digest, chunk digest, optional
  successor digest, alias-proof hex shape, and `Hot`/`Warm`/`Cold`
  storage-class policy values in both builder and argument decoding paths,
  alongside the existing content length, epoch, replica, and partial-alias
  guards. Python now also exposes raw and typed
  `register_sorafs_pin_manifest` helpers that validate/canonicalize manifest,
  chunk, successor, pin-policy, chunker, alias-proof, and credential-alias
  inputs before the Torii register request is emitted, and rejects duplicate
  camelCase/snake_case paid-pin request and typed-response aliases before any
  precedence rule can hide conflicting caller data. C# now exposes the same
  Torii register path through typed models and validates/canonicalizes request
  and response digest, policy, chunker, alias-proof, and fee-receipt fields.
  Swift now exposes matching async/completion register helpers and typed
  request/response models with the same digest, policy, chunker, alias-proof,
  and typed-response normalization before SDK callers observe a paid-pin
  receipt. The JavaScript Torii register helper now also rejects contradictory
  camelCase/snake_case paid-pin request and response aliases before submit or
  typed decoding, while accepting canonical snake_case successor and policy
  inputs. Torii gateway policy admission now also treats only an explicit,
  signed `X-SoraFS-Manifest-Envelope` bound to a paid-pin registry record as
  manifest-envelope evidence, so alias proof headers, malformed envelopes, and
  missing registry records cannot satisfy `require_manifest_envelope`, and a
  stale envelope no longer passes after registry chunk/profile metadata or
  approved envelope-digest rotation.
- Keep standalone JVM SDK guard scripts on the same toolchain contract as CI:
  privacy, Kagemusha recursive spend, and SoraFS pin-register JVM guards now
  reject inherited non-21 `JAVA_HOME` values, accept documented per-lane
  Java-home override variables, resolve JDK 21 explicitly, and print
  `java -version` before Gradle or `javac` runs. Their meta-guards and
  workflows now run negative controls that mutate the workflow away from
  Temurin/Java 21, move JVM tests before Java setup, remove the
  selected-Java-version evidence, rename the Java-home override variables, or
  remove inherited non-21 `JAVA_HOME` rejection evidence, and require the drift
  to be rejected before the main guard passes. The JavaScript meta-tests also
  derive each workflow's required negative-control modes and pull-request path
  coverage from the guard inventories, so newly added guard modes or guarded
  files cannot be omitted from workflow execution silently, and duplicate
  negative-control mode entries or guarded path inventory entries now fail the
  meta-test. They must also exercise fake non-21 JDK homes against the
  standalone JVM runners so the early Java runtime gate is proven behaviorally.
- Keep standalone Swift SDK parse scripts on the same selected-compiler
  evidence contract: privacy, Kagemusha recursive spend, and SoraFS
  pin-register guards now require `"${SWIFTC_BIN}" --version` before parsing,
  keep their documented compiler override variables stable, prove
  override-variable drift is rejected, and SoraFS fails closed instead of
  skipping Swift parsing when the compiler is unavailable. The JS meta-tests
  must also exercise fake `swiftc` parse failures against the standalone Swift
  runners so compiler-version evidence and parse-failure propagation are proven
  behaviorally.
- Keep privacy, Kagemusha, and SoraFS C# SDK workflow lanes on the .NET 8
  contract; their guards now reject missing setup-dotnet, non-8.0.x SDK pins,
  C# test commands that run before the .NET setup step, and standalone C# SDK
  scripts that stop printing the selected `dotnet --version` evidence before
  filtered tests run. The standalone scripts must also reject non-8.0.x
  `dotnet` selections before restore/test, keep their documented `dotnet`
  override variables stable, and prove matcher or override drift is detected.
  The JS meta-tests must exercise fake `.NET` 7 commands against those runners
  so the early runtime gate is proven behaviorally.
- Keep privacy, Kagemusha, and SoraFS JavaScript workflow lanes on the Node 20
  package-lock cache contract; their guards now reject Node-version drift,
  cache-dependency-path drift, and `npm ci` running before setup-node. The
  standalone JavaScript SDK runners must also print the selected Node runtime
  and reject non-`v20.*` runtimes before focused tests, keep their documented
  Node override environment variables stable, prefer override variables and
  known Node 20 candidates before falling back to `node` for the fail-closed
  major-version check, and prove override-variable or resolver drift is
  rejected. The JS meta-tests must also exercise fake non-20 Node overrides
  against the standalone runners so the early runtime gate is proven
  behaviorally, with Kagemusha using a dedicated runner instead of an inline
  workflow test command. The focused Kagemusha and SoraFS JS runner patterns
  must include their runtime-gate meta tests so those lane checks prove the
	  runtime preflights directly. The JavaScript privacy helper lane must keep
	  Jindo, SIS-with-hints, and the research adapter source/package-dist exports
	  covered against class-instance option objects while retaining raw envelope
	  bytes verifier shortcuts for canonical proof bytes. It must also keep
	  research catalog labels for Orchard, FCMP++, Miden, Aztec, and PQ MASP
	  pinned to their dedicated OpenVerify backend tags.
- Keep privacy, Kagemusha, and SoraFS Python workflow lanes on the Python 3.11
  contract; their guards now reject Python-version drift and Python SDK test
  commands that run before setup-python, and the standalone Python SDK scripts
  must keep printing both selected and venv interpreter versions while rejecting
  non-3.11 interpreters before pytest. They now prefer documented Python
  override variables, then available `python3.11`/Homebrew Python 3.11
  candidates before falling back to `python3` for the existing fail-closed
  version check, with override and resolver drift pinned by negative controls.
  The JS meta-tests must also exercise fake non-3.11 Python overrides against
  the standalone runners so the early interpreter gate is proven before any
  venv setup or native build can run.
  All three Python SDK lanes also rebuild stale non-3.11 venvs after selecting
  a valid 3.11 interpreter, with rebuild drift pinned by the guard workflows.
  Privacy and Kagemusha native-backed
  Python lanes must also build the PyO3 extension with that selected venv via
  `maturin develop --release`, cache Rust artifacts, and keep 45-minute
  workflow timeouts for native builds. All standalone Python SDK scripts must
  also suppress bytecode writes during validation so generated cache files do
  not dirty tracked artifacts, and CI must reject ignored Python bytecode files
  if they are tracked. The Python native loader must keep stale macOS extension
	  artifacts fail-closed instead of aborting package imports. The Python privacy
	  lane must also keep VeRange, Anonymous PGC, zkAt, ZK-AMS, Vega, Silent
	  Threshold, ZK-X.509, Jindo, SIS-with-hints, and the Orchard/Penumbra/FCMP++/
	  Miden/Aztec/PQ MASP research adapters on the plain-dict contract, including
	  nested commitment descriptors and adapter metadata, while retaining the raw
	  envelope bytes verifier shortcut for callers that already hold canonical
	  proof bytes.
- Keep public SCCP release evidence tied to every UI-side full-light-client
  role helper, not only aggregate request builders; Solana and TON readiness
  rows now require the per-role audit proof request symbols across web, Python,
  Swift, Kotlin/JVM, and Java Android.
- Keep the web portal SCCP proof-generation surface aligned with package
  artifacts; release-readiness tests now require every JavaScript/web helper
  named in the public user-prover rows to exist in source, packaged `dist`,
  package entrypoints, and TypeScript declarations. The JavaScript
  Ethereum-mainnet facade is now exported from the package root, rejects
  non-mainnet `eth_chainId` values before treating a provider as ready, and
  keeps the easy outbound path ETH-only. Swift, Kotlin/JVM, Java Android, and
  .NET now expose the same easy Ethereum-mainnet inbound method shape with
  app-supplied execution providers and fail-closed receipt/block drift checks
  before native prover or submitter callbacks run; those native Ethereum
  facades also require receipt-backed proving to carry a validated SCCP source
  bridge log digest before local/native prover callbacks run, and matching
  source bridge logs must explicitly encode empty event data as `0x` rather
  than relying on missing RPC fields, with cross-SDK regressions for duplicate
  source events, removed logs, non-object log entries, and missing log `data`.
  Swift, Kotlin/JVM, Java Android, and .NET now also accept configured
  Ethereum source-bridge emitter addresses at the facade/call boundary, derive
  receipt source-event digests without forcing every evidence object to repeat
  the bridge address, and reject configured/per-evidence bridge-address drift
  before source proving can run.
  They also accept app-supplied
  consensus/finality providers so collected mainnet receipts can
  attach beacon-finality evidence before local source proving,
  browser/provider chain-id parsing is canonical, JavaScript,
  Python, Swift, Kotlin/JVM, Java Android, and .NET execution-provider
  `eth_chainId` responses must be canonical JSON-RPC quantities rather than
  decimal strings or decoded numeric values, EVM live-evidence
  block tags fail closed on unstable or noncanonical values before JSON-RPC,
  EVM destination/source live-evidence collectors now reject wrong
  mainnet-chain `eth_chainId` values before `eth_getCode`, receipt, block, or
  contract-state sampling, the Ethereum source live-evidence CLI defaults
  source bridge bytecode sampling to the `finalized` block tag, Ethereum source
  and destination production TOML now carries explicit block-tag metadata that
  all-lanes rejects unless it is `finalized`, and public all-lanes summaries
  plus release-readiness cryptographic-evidence rows expose that
  source/destination tag pair under strict release-bundle schema, source
  deployment evidence now binds `eth_getTransactionByHash` readback to the
  verified deployment receipt block and contract-creation input, all-lanes
  evidence now preserves and validates those source deployment transaction
  readback fields, direct ETH/BSC source bridge TOML renderers require the same
  transaction block/input metadata, Ethereum source live evidence also proves
  the deployment receipt block is not newer than the finalized execution head
  before governed TOML can be rendered, route-canary
  transaction receipts now reject
  non-object or removed logs before accepting `MessageProofAccepted`, the
  accepted route-canary log must carry receipt-matching `transactionHash`,
  `blockHash`, and `blockNumber` metadata, `eth_getTransactionByHash` readback
  must carry the same receipt block hash and number, direct EVM destination
  TOML plus all-lanes replay now preserve and revalidate that route-canary
  transaction readback block metadata, and core regressions prove
  the same EVM canary evidence is rejected under changed
  source/deployment-bound route hashes. Browser/native collectors now also
  reject matching Ethereum source
  bridge logs whose `transactionHash`, `blockHash`, or `blockNumber` metadata
  drifts from the normalized receipt. Browser/native collectors now reject
  beacon-finality evidence
  whose execution block number, execution block hash, or execution receipts root
  does not match the validated execution receipt/block. The JavaScript
  Ethereum `proveInboundToSora` path now runs that collection and binding step
  before invoking app-owned prover callbacks, including inputs that already
  carry a typed `receiptProof` or a precomputed `receiptProofHash`, and
  JavaScript/native Ethereum
  inbound proving rejects missing beacon finality before app-owned prover
  callbacks can run. Python now matches that Ethereum-mainnet inbound shape
  with execution/consensus provider injection, receipt/block collection,
  beacon-finality binding, non-zero proof-byte copying, and a prove-time
  missing-finality guard. Swift, Kotlin/JVM, and Java Android now accept per-call
  execution/consensus providers on `proveInboundToSora`, matching the
  JavaScript/.NET prove-time collection path. The JavaScript package declarations now expose typed
  Ethereum beacon-finality evidence and consensus-provider input shapes so
  browser apps see the required execution block number/hash and receipts-root
  fields before runtime, and Swift, Kotlin/JVM, Java Android, and .NET now
  expose typed beacon-finality helper records/builders that produce the same
  canonical native map/dictionary shape for provider-collected evidence, plus
  typed inbound-evidence construction helpers for feeding that finality object
  into ETH -> SORA source proving without manual map copying. The
  release-readiness report and strict bundle verifier now require those native
  helper symbols in the `eth,bsc` SDK rows. The JavaScript package-dist tests
  now also guard the browser Ethereum and BSC mainnet SCCP artifacts against
  `WebAssembly`, `wasm`, `snarkjs`, remote prover, snake-case/hyphenated
  remote-prover aliases, prover URL, and prover endpoint dependency markers,
  and package declaration tests require the full Ethereum-mainnet browser facade
  method list plus typed local proof bytes for inbound proving/submission.
  Release-bundle verification requires both
  no-WASM guard test names plus the Ethereum facade declaration and BSC Parlia
  declaration test names in the JS phase transcript. Release-readiness tests
  also scan the Ethereum and BSC JavaScript, Python, Swift, Kotlin/JVM, Java
  Android, and .NET facade sources for missing files or forbidden
  WASM/snarkjs/remote-prover dependency markers and common identifier variants,
  keeping those mainnet SDK paths native or local-prover owned; strict
  release-bundle verification now source-inventories those native no-WASM
  readiness guards as well, so removing the BSC/ETH facade source scans or the
  common remote-prover/prover-endpoint spellings blocks publication. They now also
  guard the SDK test sources, with strict release-bundle verification mirroring
  the same inventory, so Ethereum mainnet inbound adversarial coverage keeps
  failed receipt, receipt-root/finality drift, wrong source-event topic, and
  duplicate source-bridge log cases across browser and native SDKs. They now
  also guard the Ethereum mainnet
  evidence-collection regions, including the published JS `dist` artifact, and
  the standalone release-bundle verifier now mirrors that scan so published
  bundles keep using app-owned execution/consensus providers and cannot grow
  Torii, proxy, or embedded HTTP-client fallbacks.
  Core Ethereum beacon-receipt source-adapter preflight now rejects empty
  execution headers, empty sync committee rosters, and mismatched roster,
  weight, or proof-of-possession vectors before admission or cryptographic
  verification; it also rejects zero total/signed weights, zero per-validator
  weights, all-zero signer bitmaps, impossible signed-weight totals, and signer
  bits outside the advertised roster, plus zero sync-committee message hashes.
  Sync-committee transition preflight now requires version/domain consistency,
  adjacent sync periods, period-bound nonzero transition slots, nonzero
  transition roots and hashes, bounded next committee payloads, and a
  well-formed signing committee before verifier execution. Top-level Ethereum
  beacon-receipt adapter preflight now also requires version/domain consistency
  and nonzero finality, execution, sync-committee, and receipt-proof hashes; the
  EVM-family receipt-root MPT value helpers now also reject all-zero receipt
  roots at Rust construction/decode time and in the JavaScript source/dist,
  Swift, Kotlin/JVM, Java Android, and .NET SDK helper/transcript surfaces;
  release-readiness and strict bundle inventories now require those SDK
  implementation and regression markers. The generic Ethereum receipt RLP
  builders in JavaScript, Swift, Kotlin/JVM, Java Android, .NET, and the Python
  receipt-proof evidence script now explicitly allow all-zero log topics for
  ordinary receipt reconstruction and also allow all-zero log addresses in
  generic receipt reconstruction, while keeping SCCP source-event bridge-address
  and digest checks strict; release-readiness and strict bundle inventories
  require those zero-topic and zero-address acceptance markers as well. The
  Python receipt-proof evidence path now also requires source-event log
  `transactionHash`, `blockHash`, and `blockNumber` to match the enclosing
  receipt/block context, with release inventories guarding that fail-closed
  source-event binding. JavaScript, Swift, Kotlin/JVM, Java Android, and .NET
  SDK adversarial suites now also include missing source-event log
  `transactionHash`/`blockHash`/`blockNumber` cases, and strict inventories
  require those cross-SDK markers. JavaScript, Swift, Kotlin/JVM, Java Android,
  and .NET now also pin hash-only receipt-proof-hash evidence handling, with JS
  covering snake-case `receipt_proof_hash` normalization and zero/noncanonical
  rejection while native suites cover hash-only acceptance plus zero and
  noncanonical hash rejection in the strict release inventories.
  Configured Ethereum source-adapter
  production admission now has a regression proving deployment-tagged legacy
  receipt-root-only fixtures still fail unless the governed source-bridge log
  path is present.
  Swift, Kotlin/JVM, and Java Android Ethereum inbound facades now reject
  empty/all-zero app-owned prover output and return copied proof bytes before
  Iroha submission, matching the JS/Python/.NET local-prover path. JavaScript,
  Swift, Kotlin/JVM, Java Android, and .NET Ethereum outbound facades now pin
  both the request source domain and destination binding source domain to SORA
  before allowing the Ethereum mainnet verifier-calldata path. They now also
  have explicit pre-callback regression coverage proving BSC/foreign-source
  requests cannot reach app-owned prover code through the Ethereum facade, and
  release-readiness/bundle inventory guards require those markers alongside the
  existing Ethereum inbound adversarial SDK tests.
  Release-readiness and strict
  release-bundle verifier helper inventories now require the native typed
  Ethereum receipt-proof evidence helpers for Swift, Kotlin/JVM, Java Android,
  and .NET, and now require the full Swift/Kotlin/JVM/Java Android native
  Ethereum outbound facade methods by name. The active production launch policy
  targets the Ethereum mainnet lane while BSC and TRON remain coherent
  first-class SCCP SDK/prover/evidence surfaces; non-active lanes stay
  launch-gated until their lane policy opens. The release-readiness renderer
  plus strict release-bundle verifier pin their active launch constants to
  `EthereumMainnetLane`/domain `1`/`eth`. Those release evidence paths also
  carry and strictly verify EVM source/destination RPC chain IDs in the
  all-lanes summary and cryptographic evidence table, requiring Ethereum
  mainnet live reads to report canonical chain id `1` (`0x1`) alongside the
  `finalized` block tags before the active launch lane can be published.
  Ethereum source verifier material and
  source-adapter deployment records now also carry a recomputable source bridge
  config hash bound to EIP-155 chain id `1`, ETH -> SORA domains, the governed
  bridge address, and its runtime code hash. The Python, Swift,
  Kotlin/JVM, Java Android, and JavaScript Ethereum-mainnet calldata helpers
  now also require wrapped proof results carrying the chain-id-1 destination
  binding before verifier calldata is emitted. Python, Swift, Kotlin/JVM, Java Android, and
  .NET now expose matching Ethereum-mainnet guards/facades over their native
  EVM proof surfaces; the .NET guard rejects uppercase or padded network-id
  strings before treating destination material as canonical, the C# Ethereum
  facade now exposes native outbound proof-request/prove/calldata/submit hooks
  with BN254 tuple and public-input binding checks before calldata emission,
  Swift/Kotlin/JVM/Java Android Ethereum facades now expose app-owned outbound
  submit hooks after calldata validation, Python now exposes the same
  app-owned Ethereum outbound submit hook, and the release inventories require
  the JavaScript/Python Ethereum outbound methods by name. The JavaScript,
  Python, Swift, Kotlin/JVM, Java Android, and .NET BSC facades now also expose or
  require app-owned BSC outbound submit hooks after BSC calldata validation, and
  the release inventories require those BSC calldata/submit symbols by SDK. The
  C# facade unit suite now validates the ETH/BSC bindings on .NET 8 without
  relying on newer try-style hex conversion APIs, while Rust, JavaScript,
  Python, Swift, Kotlin/JVM, and Java Android ETH/BSC receipt-proof transcript
  builders now reject zero source-event digests before deriving source witness
  hashes. The
  storage-proof transcripts across Rust and the web/Python/native SDK surfaces.
  Strict release evidence plus published release-bundle verification must
  include the package-root SCCP export test transcript, the JavaScript
  Ethereum/BSC mainnet facade transcripts, and the BSC-mainnet
  facade/prover/submission helpers plus the concrete BSC inbound
  collect/prove/submit facade methods across JavaScript, Python, Swift,
  Kotlin/JVM, Java Android, and .NET, plus BSC outbound calldata/submit facade
  methods in the `eth,bsc` row for JavaScript, Python, Swift, Kotlin/JVM, Java
  Android, and .NET, plus the concrete Python
  Ethereum inbound collect/prove/submit facade methods, including the Ethereum
  beacon-finality consensus-provider hook symbols, native BSC Parlia
  consensus-provider hook symbols, and typed native BSC Parlia finality
  helper records/builders on the SDKs that collect finality evidence,
  and the JavaScript/Python/Swift BSC prove-time guards that require Parlia
  finality before app-owned source prover callbacks run. Swift now also
  supports a BSC consensus-provider collection hook and binds supplied or
  collected Parlia finality to the collected receipt block number, block hash,
  and receipts root, while the JavaScript declarations expose the BSC Parlia
  finality evidence and consensus-provider input shapes used by that runtime
  path.
  The strict bundle
  verifier's canonical Markdown renderer now emits the `.NET` helper set for
  that row, and Python hook validation requires the exact app-owned `prove`
  callback rather than accepting method names that merely contain the word.
- Keep Ethereum mainnet source-adapter transition chains period-contiguous:
  sync-committee updates now advance exactly one mainnet period at a time, using
  the consensus `32 * 256` slot period geometry, so skipped-period transition
  evidence cannot satisfy the ETH source proof verifier. The source-adapter
  shape gate also requires non-empty transition chains to be internally
  adjacent by committee hash and sync period, no later than the adapter beacon
  slot, and terminal at the adapter's active sync-committee root and sync
  period before BLS transition-chain verification runs. The Rust helper API now
  exposes Ethereum-mainnet-specific source-adapter deployment and
  deployment-bound source-proof verification helpers so the first-lane
  ETH -> SORA path does not rely on generic EVM-family plumbing.
- Keep Ethereum mainnet SDK local-admission packaging first-class across every
  user prover surface: JavaScript/browser, Python, Swift, Kotlin/JVM, Java
  Android, and .NET now expose ETH -> SORA local-admission builders that bind
  the Ethereum source domain, SORA target domain, canonical `SubmitBridgeProof`
  metadata, normalized verifier/deployment hashes, and copied native verifier
  artifact bytes without WASM or remote-prover fallback.
- Keep EVM route-canary live evidence bound to the receipt block: the live
  helper now checks receipt block number/hash against `eth_getBlockByNumber`,
  requires a non-zero block `receiptsRoot`, rejects duplicate matching
  `MessageProofAccepted` events at the supplied log index, and refuses imported
  full-TOML summaries whose route-canary block verification metadata was
  forged. Strict release-bundle verification now also owns regressions for
  positive receipt block numbers, non-zero receipt block hashes, non-zero block
  `receiptsRoot` values, receipt-block hash-role separation, and direct helper
  parity with the runtime's finality-height hash-role rejection. Runtime and
  builder-side canary checks also pin the EVM proof transcript to target-domain
  ETH/BSC, proof version `1`, proof source-domain SORA, and a consumed
  `usedMessageProofs(messageId)` replay guard.
- Keep TRON route-canary helper and runtime transcript policy aligned: the
  source bridge evidence helper, all-lanes preflight, release-bundle verifier,
  and Rust runtime now all reject finality-height replay across TRON v3
  route-canary hash roles before full rollout TOML or launch readiness can pass.
- Keep SCCP linked-prover callback snapshots immutable across production
  destinations; JavaScript, Python, Swift, Kotlin/JVM, Java Android, and .NET
  callback regressions now assert frozen request metadata where exposed and
  copy-backed bundle and source-proof bytes across EVM-family, TRON, TON, and
  Ethereum/BSC mainnet facade witness-provider paths. The .NET Ethereum/BSC
  inbound callback snapshots now also clone nested mutable dictionary and
  enumerable evidence values before app-linked callbacks return proof bytes.
- Keep TAIRA-to-TRON XOR source records economically bound at consensus
  admission; `taira_tron_xor` record overlays must include same-overlay
  whole-unit XOR burns by the payload sender, with the TAIRA burn-record
  contract and deployment evidence flow used for activation. Live route
  activation still needs the browser-safe TRON prover bundle, deployed TAIRA
  settlement contract evidence, and bidirectional smoke transfers.
- Keep public SCCP phase evidence bound to executed production-corridor
  commands; release-readiness and release-bundle checks now require expected
  phase command fragments to appear on traced `+ ...` command lines inside the
  claimed phase block, not merely in incidental test output. The public bundle
  verifier also rejects prefix-alias phase markers, completion sentinels copied
  from a different phase block, and success markers that appear only on traced
  command lines instead of phase output. The verifier owns its required phase
  and phase transcript inventories independently of the report generator, with
  parity tests preventing drift.
- Keep Ethereum mainnet inbound source-proof support on the active
  local-admission proof path: core admits configured Ethereum proofs when the
  governed Ethereum source, deployment, destination-rollout, route-allowlist,
  and canary records are present under `EthereumMainnetLane`; BSC and other
  supported lanes remain gated until their lane policy opens. The BSC Parlia
  receipt/validator fixture remains as non-active lane coverage, including
  replayed deployment-receipt rejection before public-input extraction.
  Remaining release work is broader live deployment artifacts.
- Keep Python SCCP package-root exports aligned with the public user-prover
  rows; release-readiness tests now import `iroha_torii_client` and require
  every non-callback Python helper/class to be exposed through `__all__`.
- Keep Python UI witness-provider inputs isolated from app-owned mutable
  objects; the SDK snapshot path now clones accepted non-string sequence inputs
  before user-provided witness resolvers run, so portal/mobile witness
  preparation cannot mutate the original proof request that the UI displays.
- Keep public SCCP user-prover helper rows one-to-one with real UI hooks;
  release-readiness tests and the release-bundle verifier now reject duplicate
  helper symbols in default and per-SDK rows so repeated names cannot stand in
  for omitted proof-generation entrypoints. The public bundle verifier owns the
  SDK phase inventory independently of the report generator, with parity tests
  preventing drift.
- Keep public SCCP user-prover rows tied to UI-owned proof hooks, not only
  request builders; readiness evidence and strict bundle verification now name
  the web/Python witness and prove callbacks, Swift witness/prove typealiases,
  Kotlin proof engines, Java Android nested proof engines, and Solana/TON
  source-state audit engines. The public bundle verifier now also owns the
  lane/SDK helper inventory for those proof-generation and on-chain submission
  entrypoints, plus the exact expected row construction and submission text, so
  weakening the report generator cannot remove cryptographic prover helpers or
  define a shorter portal/mobile table as canonical for published rows.
- Keep public SCCP user-prover rows gated by the real release phases; Ethereum
  mainnet source proofs now use lane-local configured readiness instead of the
  global all-lanes gate, so ETH can open with complete mainnet source,
  destination, route allowlist, and canary evidence while other advertised
  lanes remain fail-closed until their own launch policy opens. Core, Torii,
  and bridge-proof regressions now exercise that ETH-only launch gate. Strict
  bundle verification now rejects duplicate, unknown, or missing required
  phases, requires every SDK plus core-admission on each row, and keeps
  EVM/TRON proof backends tied to contract-smoke evidence.
- Keep the public SCCP user-prover lane inventory fixed to production
  lane/backend pairs; strict bundle verification now rejects duplicate,
  unknown, or missing rows and backend-id drift for EVM/BSC, TRON, Solana, TON,
- Keep the public SCCP cryptographic evidence inventory fixed to production
  domains; strict bundle verification now rejects duplicate, unknown, or
  missing domain rows plus chain-label drift before comparing rows with
  embedded all-lanes evidence.
- Keep public SCCP cryptographic evidence rows tied to domain-specific route
  canary and source-gate policy; strict bundle verification now rejects
  incorrect canary sources, impossible source-gate requirements, and missing or
  unexpected named source-gate audit hashes.
- Keep public SCCP readiness Markdown verifier-owned and reviewer-complete;
  strict bundle verification now owns the canonical Markdown renderer, parses
  the Markdown sections independently, and requires copied evidence hashes,
  corridor artifacts, checklist statuses, cryptographic evidence rows,
  portal/mobile helper symbols, lane readiness rows, blockers, and
  release-evidence handoff text to appear in the public report.
- Keep public SCCP bundle verification free of generator backdoors for owned
  release artifacts; the verifier no longer exposes report/bundle module hooks
  for canonical Markdown, release-note attachments, copied-evidence summary
  recomputation, corridor inventories, crypto rows, or user-prover surfaces.
- Keep public SCCP release bundles rooted in immutable extracted directories;
  strict bundle verification now rejects a symlinked bundle root or a
  non-directory verifier input before reading the manifest. The bundle builder
  now rejects symlinked source inputs or source-path ancestors before copying
  evidence TOML, phase logs, native prover manifests, or native prover
  payloads, including `--allow-not-ready` diagnostic bundles. Bundle output
  directories and existing non-root output-path ancestors must not be symlinks
  before creation or forced replacement, with category-only diagnostics. Bundle
  source paths and output directories containing ASCII control characters are
  rejected during input validation before any bundle directory is created, with
  category-only diagnostics.
- Keep public SCCP release manifests as verifier roots, not published artifacts;
  strict bundle verification now rejects any `manifest.json` row inside the
  manifest artifact table.
- Keep public SCCP release bundles free of unreviewed filesystem entries; strict
  bundle verification now rejects empty or otherwise unmanifested directories
  instead of comparing only files.
- Keep public SCCP release artifact paths printable and reviewer-safe; the
  readiness report, bundle builder, and strict verifier now reject ASCII control
  characters and Markdown-unsafe path characters (`|`, backticks, `<`, and `>`)
  in public artifact paths, copied source filenames, native prover
  manifest-relative payload paths, manifest/report metadata, and extracted
  bundle entries before they can reach Markdown tables or diagnostics. The
  bundle builder also rejects copied source filenames with surrounding
  whitespace or percent-encoded traversal before evidence inputs, corridor phase
  logs, or native prover manifests can be copied into public bundle paths.
  Copied source filename diagnostics for Markdown-unsafe characters are
  category-only before source copying.
  Source symlink and source-ancestor diagnostics redact operator-local paths
  before source copying.
  Readiness-report input and input-artifact provenance diagnostics redact
  malformed copied-evidence path text and copied-input recomputation exception
  text before bundle or verifier output is emitted.
  Native prover manifest-relative payload paths reject percent-encoded traversal
  before payload source resolution or copying, and their control-character and
  Markdown-unsafe diagnostics redact the rejected path text. Missing,
  non-regular, unreadable, or forbidden-marker-scan-failed native prover
  payload diagnostics are category-only too. Sparse inventory checks remove the
  direct release-artifact path, copied filename, manifest/report path, native
  prover payload path, symlinked artifact, extracted bundle entry, and secret
  path-redaction regressions directly.
- Keep public SCCP release evidence UTF-8 fail-closed; strict verification now
  reports non-UTF-8 manifest JSON, readiness JSON, all-lanes summary JSON,
  readiness Markdown, and release-note attachments as structured bundle
  failures instead of raising out of the verifier.
- Keep DA/RBC runtime hardening focused on protocol-quorum and
  roster-verified evidence boundaries. Recent cleanup removed stale commit
  quorum-bypass plumbing, an unused READY-quorum progress-sync argument, and a
  dead near-tip backpressure branch that referenced the debug-aware quorum
  helper; future DA/RBC work should keep receiver-side availability/finality
  gates on protocol quorum and avoid reintroducing debug-shortcut dependencies
  outside local emission/scheduling helpers. The 2026-06-12 broad
  `cargo test -p iroha_core --lib -- --nocapture` run is green (`4647` passed,
  `262` ignored) after the retained-summary evidence hardening and
  default-feature STARK-only fixture gating.
- Keep extending the Sumeragi formal corridor with independent TLC
  cross-checks; the current local TLC slice covers the top-level commit-path
  fast model under the fairness-backed `Spec`, including finality and
  finality latch/phase equivalence, commit-certificate finality equivalence,
  live commit-gate finality equivalence, NPoS stake-quorum fork-safety via
  `fork-npos`,
  live commit-gate RBC evidence binding,
	  inbound RBC READY/DELIVER key-header-signature evidence binding,
	  inbound RBC CHUNK key-header-signature evidence binding and digest matching,
	  RBC READY gate full-chunk matching,
	  RBC DELIVER gate complete-evidence matching,
	  RBC DELIVER finality buffered-commit matching,
	  RBC DELIVER finality-step commit-artifact installation,
	  RBC DELIVER finality-step committed-delivery completion,
	  RBC DELIVER finality-step complete committed-delivery entry,
	  RBC DELIVER pending-branch missing-commit-evidence matching,
	  RBC DELIVER pending-step commit-artifact preservation,
	  RBC DELIVER pending-step delivered-evidence/no-finality handoff,
	  RBC DELIVER pending-step complete wait-state entry,
	  RBC DELIVER delivery-entry finality/wait-state outcome split,
	  RBC DELIVER delivery-entry commit-artifact outcome matching,
	  RBC DELIVER delivery-entry post-gate surface matching,
	  RBC DELIVER delivery-entry consensus-frame outcome matching,
	  RBC DELIVER delivery-entry certified source-stack matching,
	  RBC DELIVER delivery-entry committed post-state invariant bundle,
	  RBC DELIVER delivery-entry finality post-state gate split,
	  RBC DELIVER delivery-entry pre-GST finality post-state gate branch,
	  RBC DELIVER delivery-entry post-GST finality terminal branch,
	  RBC DELIVER delivery-entry pending non-final wait surface,
	  RBC DELIVER delivery-entry pending timer-gate split,
	  RBC DELIVER delivery-entry pending pre-GST wait timers,
	  RBC DELIVER delivery-entry pending post-GST timeout/progress split,
	  RBC DELIVER delivery-entry pending delivered-wait predicate bridge,
	  RBC DELIVER delivery-entry pending continuation surface,
	  RBC DELIVER delivery-entry commit-evidence exact continuation split,
	  RBC DELIVER delivery-entry commit-evidence exclusive outcome discriminator,
	  RBC DELIVER delivery-entry commit-evidence exclusive gate outcome,
	  RBC DELIVER delivery-entry commit-evidence exact consensus frame,
	  RBC DELIVER delivery-entry commit-evidence exact action source,
	  RBC DELIVER delivery-entry commit-evidence certified/pending stack split,
	  RBC DELIVER delivery-entry commit-evidence exact witness surface,
	  RBC DELIVER delivery-entry commit-evidence live commit gate crossing,
	  RBC DELIVER delivery-entry commit-evidence continuation mode,
	  RBC DELIVER delivery-entry commit-evidence view handoff surface,
	  RBC DELIVER delivery-entry commit-evidence delivered evidence surface,
	  RBC DELIVER delivery-entry commit-evidence GST/timer surface,
	  RBC DELIVER delivery-entry commit-evidence progress action surface,
	  RBC DELIVER delivery-entry commit-evidence vote/stake budget surface,
	  RBC DELIVER delivery-entry commit-evidence threshold classifier,
	  RBC DELIVER delivery-entry commit-evidence pending commit-vote progress split,
	  RBC DELIVER delivery-entry commit-evidence pending non-commit-vote progress split,
	  RBC DELIVER delivery-entry commit-evidence pending progress partition,
	  RBC DELIVER delivery-entry commit-evidence post-state classifier,
	  RBC DELIVER delivery-entry commit-evidence certificate/progress disjointness,
	  RBC DELIVER delivery-entry commit-evidence action-family classifier,
	  RBC DELIVER delivery-entry commit-evidence Byzantine commit-vote boundary,
	  RBC DELIVER delivery-entry commit-evidence residual gate partition,
	  RBC DELIVER delivery-entry commit-evidence complete handoff,
	  RBC DELIVER delivery-entry commit-evidence continuation-state seed,
	  RBC DELIVER delivery-entry commit-evidence pending action-surface seed,
	  RBC DELIVER delivery-entry commit-evidence pending timer-surface seed,
	  RBC DELIVER delivery-entry commit-evidence pending counter-frame seed,
	  RBC DELIVER delivery-entry commit-evidence pending complete wait-state seed,
	  RBC DELIVER delivery-entry commit-evidence delivered-pending wait-state handoff,
	  RBC DELIVER commit-evidence branch handoff,
	  RBC delivered-pending commit-evidence wait-state handoff,
	  RBC delivered-pending named complete wait-state closure,
	  RBC delivered-pending named commit-vote split,
	  RBC delivered-pending named commit-vote preservation handoff,
	  RBC delivered-pending named commit-vote finality handoff,
	  RBC delivered-pending named prepare-vote split,
	  RBC delivered-pending named timeout/NewView handoff,
	  RBC delivered-pending named NewView-vote split,
	  RBC delivered-pending named proposal handoff,
	  RBC delivered-pending named GST preservation,
	  RBC delivered-pending named exact action-branch classifier,
	  RBC delivered-pending commit-vote preservation handoff,
	  RBC delivered-pending commit-vote finality handoff,
	  RBC delivered-pending prepare-vote handoff,
	  RBC delivered-pending timeout/NewView handoff,
	  RBC delivered-pending NewView-vote handoff,
	  RBC delivered-pending proposal handoff,
	  RBC delivered-pending GST preservation,
	  RBC delivered-pending Next coverage,
	  RBC delivered-pending spec-step closure,
	  RBC delivered-pending spec-step outcome split,
	  RBC delivered-pending spec-step delivered-evidence preservation,
	  RBC delivered-pending spec-step commit-artifact outcome,
	  RBC delivered-pending spec-step GST boundary,
	  RBC delivered-pending spec-step view boundary,
	  RBC delivered-pending spec-step view-evidence boundary,
	  RBC delivered-pending spec-step vote-counter handoff,
	  RBC delivered-pending spec-step post-gate handoff,
	  RBC delivered-pending spec-step timer-gate handoff,
	  RBC delivered-pending spec-step finality-source handoff,
	  RBC delivered-pending spec-step finality witness-frame,
	  RBC delivered-pending spec-step finality-stack outcome,
	  RBC delivered-pending spec-step finality-gate outcome,
	  RBC delivered-pending spec-step finality-quorum outcome,
	  RBC delivered-pending spec-step non-final handoff phase shape,
	  RBC delivered-pending spec-step action-surface closure,
	  RBC delivered-pending spec-step phase-change source,
	  RBC delivered-pending spec-step counter-change source,
	  RBC delivered-pending spec-step exclusive action source,
	  RBC delivered-pending spec-step stutter action-surface preservation,
	  RBC delivered-pending spec-step commit-artifact change source,
	  RBC delivered-pending spec-step commit-artifact certified-delivery bundle,
	  RBC delivered-pending spec-step exact-source certified-delivery bundle,
	  RBC delivered-pending spec-step stable-artifact non-final handoff,
	  RBC delivered-pending spec-step stable-artifact non-final source,
	  RBC delivered-pending spec-step stable-artifact counter footprint,
	  RBC delivered-pending spec-step stable-artifact phase/gate footprint,
	  RBC delivered-pending spec-step stable-artifact timer footprint,
	  RBC delivered-pending spec-step stable-artifact view/evidence footprint,
	  RBC delivered-pending spec-step stable-artifact finality footprint,
	  RBC delivered-pending spec-step stable-artifact RBC surface,
	  RBC delivered-pending spec-step stable-artifact complete wait state,
	  Byzantine fault corruptible-RBC gate matching,
	  Byzantine fault digest-only corruption step,
	  RBC INIT gate repairable-state matching,
	  RBC INIT step header/digest evidence installation,
	  RBC CHUNK step chunk-evidence advancement,
	  RBC CHUNK partial/full-coverage handoff,
	  RBC READY step ready-evidence advancement,
	  RBC READY partial/quorum handoff,
	  RBC READY quorum-step DELIVER handoff,
	  live RBC header/digest handoff gating,
  live RBC chunk handoff gating,
  live RBC READY handoff gating,
  committed-phase terminality,
  committed consensus-state stability,
  committed post-finality GST-only movement,
  committed+GST full-state quiescence,
  committed+GST disabled action guards,
  committed+GST Next rejection,
  GST observation provenance,
  timeout no-progress preemption,
  view-advance timeout provenance,
  live-progress timeout-reset provenance,
  view-evidence quorum/timeout provenance,
  NewView vote-counter provenance,
  prepare-vote counter provenance,
  commit-vote/stake counter provenance,
  phase-transition provenance,
  prepare-phase entry provenance,
  commit-vote phase entry provenance,
  propose-phase entry provenance,
  NewView phase entry provenance,
  committed-phase finality-source entry provenance,
  committed-phase certified finality-stack entry,
  committed-phase commit-certificate witness installation,
  committed-phase commit-certificate witness-change equivalence,
  committed-phase commit-view witness-change matching,
  committed-phase commit-view witness installation,
  committed-phase live-commit gate crossing,
  committed-phase commit-artifact installation equivalence,
  committed-phase exact finality-source effects,
  committed-phase NewView handoff exclusion,
  committed-phase post-entry progress-gate closure,
  committed-state Byzantine commit-vote gate closure,
  RBC state protocol/fault provenance,
  RBC global state-change exit classification,
  RBC evidence protocol/fault provenance,
  RBC global evidence-change effect classification,
  RBC header installation provenance,
  RBC header evidence monotonicity,
  RBC digest installation provenance,
  RBC digest invalidation fault provenance,
  RBC corruption entry provenance,
  RBC corruption repair exit classification,
  RBC Idle exit classification,
  RBC INIT entry provenance,
  RBC INIT exit classification,
  RBC chunk-counter increase provenance,
  RBC chunk-counter reset provenance,
  RBC chunking entry provenance,
  RBC chunking exit classification,
  RBC chunk-completion entry provenance,
  RBC chunk-complete exit classification,
  RBC READY-vote increase provenance,
  RBC READY-vote reset provenance,
  RBC READY partial-entry provenance,
  RBC READY partial exit classification,
  RBC READY quorum-entry provenance,
  RBC READY quorum exit classification,
  RBC delivered evidence stability,
  RBC DELIVER step complete-evidence preservation,
  RBC delivery-entry provenance,
  RBC delivery-entry ReadyQuorum/finality branch classification,
  RBC Withheld unreachable-state proof,
  RBC Withheld transition-target exclusion,
  commit-artifact finality-only installation,
  commit-artifact finality-source provenance,
  commit-artifact certified finality-stack change,
  finality-latch complete-stack installation,
  finality-latch/commit-artifact coupling,
  committed-phase complete-stack entry,
  committed-phase/finality-latch entry coupling,
  finality-latch committed-transition monotonicity,
  finality-latch/live-commit-gate crossing equivalence,
  finality-latch commit-certificate witness installation,
  commit-certificate witness component coupling,
  commit-certificate witness certified finality-stack change,
  commit-certificate witness commit-view installation,
  nonzero finality commit-view witness installation,
  finality-latch commit-view witness installation,
  commit-view witness certified finality-stack change,
  commit-view witness commit-certificate installation,
  finality-latch NewView handoff isolation,
  finality-latch source classification,
  finality-latch source-effect exactness,
  finality-latch source quorum-gate evidence,
  finality-latch certified source-stack classification,
  committed-view witness stability,
  committed-view witness step stability,
	  commit-view future-view exclusion,
	  GST elapsed pre-GST gate matching,
	  GST elapsed flag-only step,
	  GST monotonicity,
	  view monotonicity,
	  commit-view monotonicity,
	  commit-evidence monotonicity,
	  timeout stalled-progress gate matching,
	  timeout Byzantine-only commit progress independence,
	  timeout-step fresh NewView reset,
	  timeout-step commit-vote gate clearing,
  timeout-step fresh NewView vote handoff,
	  timeout-step RBC evidence preservation,
  view-change quorum evidence for nonzero active views,
  NewView quorum handoff and complete-only view evidence,
	  nonzero view-evidence active-view witness,
	  NewView vote quorum-step proposal handoff,
	  proposal handoff evidence matching,
	  proposal-step prepare/RBC installation,
	  proposal-step prepare-vote handoff,
	  live NewView vote handoff and fresh-evidence gate matching,
	  NewView vote quorum-branch evidence matching,
	  NewView vote quorum-step view-evidence installation,
	  NewView vote pending-branch missing-evidence matching,
	  NewView vote pending-step view-evidence preservation,
	  live prepare-vote handoff and proposal-evidence gate matching,
	  prepare-vote quorum-branch evidence matching,
	  prepare-vote quorum-step commit-vote handoff,
	  prepare-vote quorum-step commit-vote gate handoff,
	  prepare-vote pending-branch missing-evidence matching,
	  prepare-vote pending-step commit-artifact preservation,
	  prepare-vote pending-step prepare handoff preservation,
	  live commit-vote handoff and prepare-evidence gate matching,
	  Byzantine commit-vote prepare-evidence gate matching,
	  honest commit-vote finality-branch evidence matching,
	  honest commit-vote finality-step commit-artifact installation,
	  honest commit-vote finality-step committed-delivery completion,
	  honest commit-vote pending-branch missing-evidence matching,
	  honest commit-vote pending-step commit-artifact preservation,
	  honest commit-vote pending-step commit-vote handoff preservation,
	  Byzantine commit-vote finality-branch evidence matching,
	  Byzantine commit-vote finality-step commit-artifact installation,
	  Byzantine commit-vote finality-step committed-delivery completion,
	  Byzantine commit-vote pending-branch missing-evidence matching,
	  Byzantine commit-vote pending-step commit-artifact preservation,
	  Byzantine commit-vote pending-step commit-vote handoff preservation,
	  pending protocol GST preservation,
	  delivered RBC progress-gate closure,
	  complete-only commit evidence,
  pre-commit stale commit-vote reset across view changes,
  pre-prepare stale prepare-vote reset across view changes,
  pre-finality commit-artifact absence,
  finality certificate-stack completeness,
  finality certificate-stack exactness,
  finality NewView handoff cleanup,
  finality-source exact-source committed-delivery completion,
  finality-source certified-source stack classification,
  finality-source finality-latch change matching,
  finality-source committed-phase entry matching,
  finality-source finality-certificate stack installation,
  finality-source commit-or-delivery source classification,
  finality-source exact source-effect classification,
  finality-source quorum-gate satisfaction,
  finality-source commit-artifact change matching,
  finality-source live commit-gate crossing,
  finality-source post-commit progress quiescence,
  finality-source GST preservation,
  finality-source GST-only remaining gate,
  finality-source commit-certificate witness installation,
  finality-source commit-certificate witness-change matching,
  finality-source commit-view witness-change matching,
  finality-source commit-view witness installation,
  finality-source NewView handoff isolation,
  finality-source current-view commit witness exactness,
  committed-phase current-view commit witness exactness,
  committed-phase GST preservation,
  committed-phase GST-only remaining gate,
  commit-artifact exact-source committed-delivery completion,
  commit-artifact current-view witness exactness,
  commit-artifact GST preservation,
  commit-artifact GST-only remaining gate,
  commit-certificate exact-source committed-delivery completion,
  commit-certificate GST preservation,
  commit-certificate GST-only remaining gate,
  commit-view exact-source committed-delivery completion,
  commit-view GST preservation,
  commit-view GST-only remaining gate,
  finality-latch exact-source committed-delivery completion,
  finality-latch GST preservation,
  finality-latch GST-only remaining gate,
  committed-phase exact-source committed-delivery completion,
  live commit-vote prepare-quorum gating,
  commit-evidence roster-budget boundedness,
  run-level prepare-quorum commit gating, commit-certificate evidence stability,
  commit-certificate vote/stake traceability,
  live stake-accounting traceability,
  live stake roster-budget boundedness,
  honest commit-support preservation,
  live vote/stake quorum preservation,
  RBC finality evidence preservation,
  RBC progress-state evidence causality,
  RBC partial-progress counter causality,
  RBC corrupted digest invalidation,
  RBC ready-quorum deliver-gate availability,
  RBC delivered-without-finality certificate absence,
  RBC delivered finality commit-vote source,
  RBC delivered finality committed-delivery completion,
  RBC delivered finality current-view binding,
  RBC delivered finality GST-only remaining gate,
  RBC delivered finality commit-certificate witness installation,
  RBC delivered finality commit-certificate witness-change matching,
  RBC delivered finality commit-view witness-change matching,
  RBC delivered finality live commit-gate crossing,
  RBC delivered finality post-commit progress quiescence,
  RBC delivered finality certified source-stack matching,
  RBC delivered finality finality-certificate stack installation,
  RBC delivered finality committed-phase entry matching,
  RBC delivered finality commit-artifact change matching,
  RBC delivered finality latch-artifact coupling,
  RBC delivered finality exact commit-vote witnesses,
  RBC delivered finality delivered-RBC evidence preservation,
  RBC delivered finality view/prepare handoff evidence preservation,
  RBC delivered finality exact protocol frame,
  RBC delivered finality exact commit-vote action frame,
  RBC delivered finality committed post-state safety bundle,
  RBC delivered finality post-state gate split,
  RBC delivered finality pre-GST post-state gate branch,
  RBC delivered finality post-GST terminal branch,
  post-finality pre-GST only-enabled gate invariant,
  post-finality pre-GST GST-elapsed terminalization,
  post-finality pre-GST Next/GST-elapsed exclusivity,
  post-finality pre-GST spec-step stutter/GST split,
  committed+GST spec-step terminal stuttering,
  committed spec-step non-stuttering GST observation exclusivity,
  committed spec-step stutter/GST closure,
  committed spec-step finality-stack preservation,
  committed spec-step GST-only data-change footprint,
  committed spec-step no protocol-action footprint,
  post-finality progress-action quiescence,
  committed spec-step progress-gate quiescence, honest/fault roster-budgeted
  vote counters, RBC delivery stability, committed spec-step budgeted-RBC
  evidence stability, fast
  canonical frontier recovery, small exhaustive frontier recovery,
  frontier committed source future-stage isolation,
  frontier view-bound drop future-stage isolation,
  frontier zero-evidence drop future-stage isolation,
  frontier zero-evidence staged-future expected-failure mutation,
  frontier zero-evidence drop consensus-evidence absence,
  frontier future-promotion fresh second-slot installation,
  frontier terminal outcome exclusivity,
  frontier rotated source future-stage isolation,
  frontier promotion-ready rotation isolation,
  frontier promotion-ready active-marker cleanup,
  frontier promotion-ready active-marker expected-failure mutation,
  frontier promotion-ready rotated-marker expected-failure mutation,
  frontier rotated terminal retransmit evidence,
  frontier promotion-ready wrapper cleanup,
  frontier quorum-retransmit window cleanup,
  frontier quorum-retransmit window cleanup expected-failure mutation,
  frontier payload recovery ownership,
  frontier payload recovery ownership expected-failure mutation,
  frontier stale-recovery unlock owner cleanup,
  frontier stale-recovery unlock owner cleanup expected-failure mutation,
  frontier view-bound drop retransmit evidence,
  frontier view-bound drop retransmit evidence expected-failure mutation,
  direct validation redrive labels, direct raw QC signer-bitmap population counting, and
  direct signer-index normalization, precommit vote-progress counting, precommit
  locked-payload vote gating, commit-QC
  signer quorum gating, commit-QC cache/history lookup, precommit signer record
  admission, validation ownership cleanup, direct stable worker-loop stage helpers,
  direct worker tick-gap scheduling, direct vNext performance config conversion,
  direct pending-block validation worker config derivation, commit-worker channel
  capacity normalization, slow commit-stage timing threshold detection,
  commit-inflight timeout reporting, commit-inflight timeout mark persistence
  expected-failure mutation, post-commit pacemaker kickstart gating,
  post-commit no-queue hard-stop expected-failure mutation, idle-view proposal
  budget preservation, idle-view no-queue hard-stop expected-failure mutation,
  cached-slot timeout selection, cached-slot streak saturation
  expected-failure mutation,
  pending fast-path timeout derivation, pending fast-path DA-floor cap
  expected-failure mutation, stalled pending-block timeout
  decisions, stalled pending commit-pipeline evidence expected-failure
  mutations, stalled pending-frontier timeout derivation, exact-frontier
  proposal grace derivation, frontier proposal full-grace transaction-budget
  expected-failure mutations, exact-frontier slot helper semantics,
  frontier slot body-available helper expected-failure mutations,
  frontier slot same-candidate peer-evidence expected-failure mutations,
  exact-frontier slot tracker FSM behavior, exact-frontier apply-wrapper
  slot lifecycle expected-failure mutations, code-level exact-frontier slot
  single-source state cleanup, formal nested slot-state consistency alignment,
  slot tracker state map semantics, proposal-seen horizon expected-failure
  mutations,
  timeout/cooldown derivation semantics, round/view helper semantics,
  PhaseTracker mutable state semantics, direct failed-commit/block-sync helper
  semantics, missing-QC timing derivation, idle backlog signal derivation,
  proposal-liveness state transitions, direct actionable vote-backed proposal
  evidence admission, direct slot proposal evidence no-bug lookup/fall-through,
  direct round-liveness no-bug evidence aggregation, direct
  roster-unavailability recovery FSM no-bug transitions, consensus-recovery
  clear/prune retention semantics,
  direct frontier live-owner work preservation semantics, frontier live-owner
  conflict-adapter expected-failure mutations, direct keep-frontier pending-active
  preservation semantics, direct stale-view pending prune no-bug cleanup
  semantics, direct superseded frontier payload retention semantics, direct stale
  missing-block request prune no-bug semantics, direct stale missing commit-QC
  request prune no-bug semantics, direct stale RBC session prune no-bug
  semantics, direct highest-QC defer-marker prune semantics,
  fast-finality inline validation component/anchor semantics,
  observer signature-mismatch recovery semantics,
  direct validation failure finalization semantics,
  direct validation reject reason-label classification semantics,
  validation reject status accounting component/anchor semantics,
  peer-key policy status accounting component/anchor semantics,
  view-change cause status accounting component/anchor semantics,
  view-change proof status accounting component/anchor semantics,
  QC status projection component/anchor semantics,
  commit-quorum status projection component/anchor semantics,
  commit-inflight status projection component/anchor semantics,
  history status projection component/anchor semantics,
  RBC abort status accounting component/anchor semantics,
  RBC mismatch status accounting component/anchor semantics,
  direct RBC progress-stage synchronization semantics,
  direct RBC hot-repair/backpressure semantics,
  direct RBC repair request cooldown/targeting semantics,
  direct RBC targeted READY/DELIVER repair semantics,
  direct RBC outbound chunk flush semantics,
  direct RBC chunk post scheduling/debug-mask semantics,
  direct RBC READY/DELIVER deferral throttle semantics,
  direct RBC missing-INIT broad rebroadcast semantics,
  round-gap marker/snapshot/EMA status component/anchor semantics,
  direct RBC missing BlockCreated recovery and authoritative-only
  materialization semantics,
  direct RBC unverified-roster escape-hatch semantics,
  RBC signing-preimage component/anchor binding semantics,
  classic Vote/VRF signing-preimage aggregate exactness,
  classic Vote/QC signature-verification component/anchor semantics,
  direct invalid-signature telemetry label semantics,
  invalid-signature throttle/penalty component/anchor semantics,
  direct penalty offender-selection attribution semantics,
  consensus penalty-action derivation/application semantics,
  penalty status projection component/anchor semantics,
  local peer removed flag component/anchor semantics,
  direct execution-witness root projection component/anchor semantics,
  direct RBC compact block-message exactness/component semantics,
  direct consensus block-message priority exactness/component semantics,
  direct block-message height/view exactness/component semantics,
  direct block-message log/status kind exactness/component semantics,
  direct consensus message projection semantics,
  pipeline event emission semantics,
  direct cached block-message wire-frame component/anchor semantics,
  direct BlockCreated frontier metadata wire/rebuild component/anchor semantics,
  direct BlockCreated payload admission aggregate exactness,
  direct cached proposal rebroadcast component/anchor semantics,
  direct exact-slot frontier recovery activity semantics, exact-slot aggregate
  activity-source exactness,
  direct frontier reassembly activity semantics, frontier reassembly aggregate
  activity-source exactness,
  direct frontier quorum-owner cleanup preservation semantics, frontier
  quorum-owner aggregate cleanup exactness,
  direct contiguous-frontier sidecar retarget semantics, contiguous-frontier
  sidecar retarget aggregate exactness,
  direct contiguous-frontier sidecar expected-hash semantics,
  contiguous-frontier sidecar expected-hash aggregate exactness,
  direct contiguous-frontier payload-hint selection semantics,
  contiguous-frontier payload-hint aggregate exactness,
  direct contiguous-frontier parent-QC hint retarget semantics,
  contiguous-frontier parent-QC hint retarget aggregate exactness,
  direct vote-verification worker config derivation, direct vote-verification worker config
  aggregate exactness,
  QC aggregate-verification worker config derivation, QC aggregate-verification
  worker config aggregate exactness,
  voting-roster support counting, voting-roster support-count aggregate
  exactness, collector retry/gossip plans, collector retry/gossip plan
  aggregate exactness, direct collector fanout/selection semantics, plus
  collector fanout/selection direct exactness,
  direct topology ordered-roster mutation no-bug semantics, topology
  ordered-roster aggregate exactness, PRF leader/shuffle topology semantics,
  PRF leader/shuffle
  aggregate exactness, direct topology fanout/redundant-send semantics,
  direct topology fanout/redundant-send aggregate exactness, topology role-filter
  semantics, topology role-filter aggregate exactness, active topology-selection
  semantics, active topology-selection aggregate exactness, trusted-peer P2P
  topology semantics, trusted-peer P2P topology aggregate exactness, P2P
  topology refresh semantics, P2P topology refresh aggregate exactness, quorum
  retransmit target semantics, direct quorum retransmit target aggregate exactness,
  direct retransmit backpressure aggregate exactness, direct paced retransmit
  target aggregate exactness, direct quorum reschedule backoff aggregate exactness,
  direct RBC availability reschedule aggregate exactness, direct vote-backed reassembly stall
  aggregate exactness, direct completed quorum view-advance component semantics,
  direct quorum rebroadcast dispatch aggregate exactness, isolated vote-backed handoff
  aggregate exactness, direct preemptive vote-backed retransmit aggregate
  exactness, direct near-quorum preemptive escalation aggregate exactness, manifest-gate
  reschedule aggregate exactness, QC signer-bitmap admission aggregate
  exactness, direct raw QC signer-count aggregate exactness, QC signer-bitmap
  construction aggregate exactness, direct signer-index normalization aggregate
  exactness, commit-root consistency aggregate exactness, commit-pipeline
  recovery aggregate exactness, direct known-block commit-QC recovery aggregate
  exactness, stale-view commit-QC fetch aggregate exactness, direct commit-anchor QC
  promotion aggregate exactness, committed-height QC admission aggregate
  exactness, direct empty-block QC drop component semantics, pending-progress
  accounting aggregate exactness, direct pending-block lifecycle no-bug exactness,
  direct pending-block marker/cooldown no-bug exactness, pending-block Kura retry
  aggregate exactness, commit-pipeline scheduling aggregate exactness,
  commit-QC cache/history lookup aggregate exactness, cached-QC precommit
  signer-record aggregate exactness, roster-validation memo cache aggregate
  exactness, cached roster-validation wrapper aggregate exactness, core
  roster-validation aggregate exactness, roster artifact selection aggregate
  exactness, block roster cache aggregate exactness, block-sync roster evidence
  aggregate exactness, block-sync history roster aggregate exactness,
  persisted block-sync roster selection aggregate exactness, BlockSyncUpdate
  roster hydration aggregate exactness, direct roster index projection no-bug
  exactness, direct membership-view hash no-bug exactness, membership mismatch
  status aggregate exactness, membership advert publication aggregate
  exactness, membership mismatch ingress/fail-closed aggregate exactness,
  consensus-params ingress aggregate exactness, prevalidated commit artifact
  trust aggregate exactness, commit-job dispatch aggregate exactness,
  commit-worker config aggregate exactness.
- Sumeragi prepare-quorum phase-gating validation is closed for the current
  formal slice: the dedicated 2026-06-03 Apalache fast run reached `NoError`
  up to computation length `10` with `CommitPhasesNeverBypassPrepareQuorum`
	  loaded, and the formal coverage guard now reports `505` PR modes,
	  `9873` expected-failure modes, and `10379` documented modes.
- The focused SCCP prover corridor is green for the current production-hardening
  slice across JavaScript, Python, Swift, Kotlin/JVM, Java Android, the Rust
  `iroha_sccp` verifier crate, core bridge-proof admission tests, and on-chain
  EVM/TRON Groth16 contract smoke coverage for post-generation payload,
  finality-height, and finality-block public-signal drift.
- Supported EVM/BSC, TRON, Solana, and TON user-prover readiness rows now
  include per-SDK helper symbol maps for JavaScript/web, Python, Swift,
  Kotlin/JVM, and Java Android. Those maps carry the native source-proof,
  source-state, or full-light-client audit proof-generation helpers where
  applicable alongside the final proof request and submission helpers. Release
  bundles therefore cannot claim the portal/mobile native proof paths without
  explicitly carrying the UI proof-generation surfaces for each consumer SDK;
  recursive payload corridor to verifier-program/message-body/runtime-call
  `bundleBytes` as they already apply to proof bytes: bundles must be non-empty,
  non-all-zero, and no larger than 2 MiB before JavaScript, Python, Swift,
  Kotlin/JVM, or Java Android SDKs emit wallet/RPC instruction,
  internal-message, or runtime-call packages. The
  optional `sourceProofBytes` carried by SDK proof requests now share the same
  2 MiB source-proof corridor: omitted values remain valid, but non-empty
  source proofs must be non-all-zero and bounded before request hashing or
  app-linked user-prover invocation. The
  JavaScript and Python EVM-family/TRON contract-call submission builders now
  reject standalone `bundleBytes` or `sourceProofBytes` unless a wrapped
  `proofResult` is supplied, because raw Groth16 calldata cannot bind those
  request bytes back to the user-generated request hash. JavaScript and Python
  explicit `proofResult: null` / `proof_result=None` instead of treating it as
  an omitted proof result, keeping null/omitted semantics aligned with Solana
  builders across JavaScript, Python, Swift, Kotlin/JVM, and Java Android now
  also reject non-empty standalone `sourceProofBytes` unless a wrapped
  `proofResult` is supplied, because the final runtime-call payload carries the
  recursive bundle but not those request-bound source-proof bytes. The
  tracked JavaScript `dist/` package artifact is regenerated from that source
  and the package-dist suite now exercises the published `dist/index.js`
  portal SDK artifact aligned with the source guard. Public readiness reports
  now also require the
  JS corridor transcript to include the source SCCP tests, `package_dist`, and
  package export tests in the claimed `js-sdk` phase, so a release bundle cannot
  prove only source-side helper tests while omitting the dist artifact surface
  counterparty package builders now apply the same native recursive payload cap
  to canonical bundle bytes before emitting `SolanaProgramInstruction`,
  tooling and portal/mobile SDKs on the same submission corridor.
- The all-lanes readiness and release-bundle verifier now derive a required
  deployment records. Ready release bundles must carry that gate in the
  proofs the same machine-audited gate surface as the Solana, TON, and TRON
  JSON `toml_ready` false and refuses production TOML unless the governed
  runtime-storage gate hash is supplied and matches, so source material plus
  preflight now imports that same
  instead of treating a locally recomputed value as sufficient release evidence.
  Direct ETH/BSC source-evidence renderers now apply the same preimage rule to
  production TOML: hash-only source bridge code metadata remains diagnostic
  JSON, while `--toml` and JSON `toml_ready` require
  `--source-bridge-runtime-bytecode-hex` or
  `--source-bridge-runtime-bytecode-file` so the Keccak-256 runtime code hash is
  replayable from operator evidence.
- EVM route-canary evidence now uses a v4 transcript aligned with the TRON
  hardening model: ETH/BSC canary hashes bind the receipt block
  number/hash/`receiptsRoot`, submitted calldata SHA-256, decoded
  payload/finality public inputs, proof version/source domain, target domain,
  consumed-message state, and finalized receipt-block readback flag before
  all-lanes preflight or Rust `iroha_sccp` route admission can mark route
  evidence launch-ready. TRON live
  evidence also requires source-event and
  route-canary transaction readback to contain exactly one matching governed log
  and rejects explicit `logIndex`/`log_index` metadata that disagrees with the
  log list position or supplies both aliases before production TOML can be
  emitted. TRON `gettransactioninfobyid` and `gettransactionbyid` source-event
  and route-canary readback now also reject conflicting `txID`/`txid`/`id`
  aliases before trusting receipt logs, raw-data hashes, or signature metadata.
  Raw transaction readback requires canonical `txID`, so an `id`-only response
  cannot be mistaken for a full transaction object. Source-event and
  route-canary transaction-info readback now require exact `blockNumber` and
  `blockTimeStamp` metadata, and source-event evidence cross-checks that
  timestamp against the fetched canonical block header. Saved source-event
  replay JSON and route-canary full-TOML replay now revalidate the same carried
  block metadata before producing offline arguments, so hand-edited summaries
  cannot bypass the live readback contract. Direct and live TRON full-lane TOML
  now also carry the route-canary block number and timestamp in audit comments
  plus structured route-allowlist fields, live replay forwards those values
  through the offline renderer before all-lanes readiness can pass, and
  release-bundle verification rejects missing, non-positive block numbers or
  negative timestamps before route evidence can be published. The public
  release-readiness cryptographic-evidence table now also carries the TRON
  route-canary block number and timestamp as verifier-bound JSON fields, while
  non-TRON lanes must keep those fields null, so release notes cannot publish a
  forged or lane-shifted canary height after refreshing attachment hashes.
  Source-event block transactions apply the same alias binding before deriving
  java-tron transaction Merkle leaves for source proofs. The EVM/BSC v3
  route-canary fields, including the receipt block number/hash/`receiptsRoot`,
  are also first-class config and ZK policy-hash material, keeping Core/Torii
  configured admission bound to the same calldata, payload, finality, and proof
  transcript that `iroha_sccp` validates. The full SCCP production corridor
  passes end to end with Rust SCCP verification, operator evidence scripts,
  JS/Python/Swift/Kotlin/Java Android/.NET SDK prover surfaces, EVM/TRON
  contract smoke, and core bridge-proof admission.
  alongside the finalized head and runtime versions in public readiness JSON;
  release-bundle verification rejects zero or governed-hash-reused
  finalized-head/runtime-code canary fields before release notes can pass.
- The focused SCCP production corridor is now captured by
  `scripts/check_sccp_production_corridor.sh`, with phase selection for the
  Rust verifier crate, operator evidence scripts, web/Python/Swift/Kotlin/Java
  Android SDK proof generators, native .NET/C# ETH/BSC facade tests, the
  BSC/TRON deployment evidence tests, EVM/TRON Groth16 contract smoke, and core
  bridge-proof admission target. Release transcript gates now also require
  phase-local Node zero-failure output plus named success output for the BSC
  deploy/config test, TRON route-manifest deployment-evidence test, and shared
  TAIRA XOR contract test before `contract-smoke` evidence can pass. The same
  transcript gate rejects phase-local failure summaries, including mixed
  `failed`/`passed` pytest output, non-zero Node failure counts, failed Cargo
  summaries, Gradle `BUILD FAILED`, Swift failure counts, and failed .NET
  summaries, rather than trusting a positive success substring alone. It also
  rejects duplicate claimed phase markers so a clean first phase block cannot
  hide a later duplicate failed block in the same release artifact. The
  full-corridor completion fallback now requires every phase block to carry its
  own traced commands, success markers, and failure-free output, so marker-only
  full-corridor stubs cannot satisfy a per-phase release artifact. Completion
  sentinels must also be exact output lines, not substrings embedded in other
  output, must appear after the commands and success output they certify, and
  must be terminal for non-empty output in the completed transcript. Only exact
  known corridor phase markers may delimit phase blocks, and non-empty output
  before the first phase marker is rejected;
  prefix-like marker output is a blocker instead of a way to hide later failure
  lines. Any transcript containing multiple exact known phase markers must
  satisfy the full-corridor validator, so partial multi-phase logs cannot pass
  as complete single-phase evidence, and full-corridor logs must keep the
  production runner's canonical phase order. The
  Java Android phase now matches the current
  test surface by running the
  main-method SCCP classes through `GradleHarnessTests` and the Solana prover
  through its direct JUnit selector, with the evidence-scripts phase also
  running the corridor runner self-check so phase drift is caught before
  release validation. The Swift phase now runs the Torii bridge-proof submit
  payload test alongside the prover/source-state batch, so iOS release
  evidence covers the final EVM/TRON user-prover submission package handed to
  Torii. The runner can now print the exact selected command plan with
  `--dry-run`, so operators can review heavyweight Rust, mobile, and
  EVM/TRON contract-smoke phases before executing the production corridor.
  Gradle-backed Kotlin and Java Android phases now also fall back from explicit
  `JAVA_HOME` to the repo-local JDK bundle, macOS `java_home`, and Homebrew
  `openjdk@21`, so local mobile SDK corridor runs do not silently execute with
  an empty Java path. The GitHub Actions attachment now uploads one
  `sccp-production-corridor-<phase>` log artifact per phase so strict release
  reports can bind CI transcripts by byte length and SHA-256 digest. The local
  runner can now produce the same
  strict per-phase transcript layout with `--log-dir
  dist/sccp-production-corridor`, so release rehearsals no longer depend on
  manually teeing each selected phase. Public release-bundle verification also
  rejects noncanonical manifest and report SHA-256 text, keeping artifact
  bindings to lowercase 64-character digests.
  `scripts/sccp_release_readiness_report.py` now converts the all-lanes
  evidence bundle plus per-phase corridor results, including the structured
  release checklist, into fail-closed Markdown or JSON release notes for
  governance review. Those reports now bind every input evidence file by byte
  length and SHA-256 digest; in strict release mode they also require a hashed
  production-corridor artifact for every passed phase, with `all=<log>`
  supported for full-run transcripts. They can also consume the same per-phase
  log directory layout produced by the local corridor runner's `--log-dir`
  option or by downloaded CI artifacts, so release notes and the self-contained
  bundle builder use the same phase-transcript source format. User-prover
  submission surfaces now carry machine-readable `sdk_helper_symbols` lists as
  well as rendered helper text, and the public release-bundle verifier rejects
  drift between those fields so web/mobile proof-generation coverage remains
  auditable. Those surfaces now also require `core-admission`, preventing
  portal/mobile proof generation from being marked validated until the
  generated proof path reaches on-chain admission. Strict reports
  now inspect each passed phase artifact for the exact corridor phase marker,
  the non-dry-run completion sentinel, and the expected command fragments inside
  the claimed phase block plus phase-specific success markers, so declared
  passed status cannot be backed by an arbitrary marker-only hashed file,
  command-only transcript, or transcript with commands under another phase
  marker. The corridor runner self-check now compares the same
  required-fragment table against full `--dry-run` phase output, keeping release
  evidence expectations synced to the actual runner command plan. Report tests
  cover blocked evidence, missing strict phase artifacts, forged phase logs,
  missing-command phase logs, wrong-block command phase logs, downloaded
  phase-artifact directories, and a complete synthetic governed bundle with
  every corridor phase marked passing and bound to a corridor log. The report also renders a
  per-lane cryptographic evidence table so public release notes expose the
  source material, source deployment, destination binding, source-gate hash and
  audit hashes, route allowlist, route canary hash, and canary evidence source
  behind each ready lane. The
  all-lanes gate also rejects cross-lane route-canary hash aliasing against
  another lane's governed source, destination binding, or route allowlist
  hashes.
  `scripts/sccp_release_bundle.py` now turns the same strict inputs into a
  self-contained public release-note attachment directory containing the
  Markdown/JSON readiness report, all-lanes summary JSON, copied evidence
  TOML, copied corridor logs, `sccp-release-notes-attachment.md`, and a
  SHA-256 manifest; the evidence-scripts corridor tests that declared-only
  phase status cannot produce a production bundle. Ready bundles now run the
  strict release-bundle verifier against their generated output before the
  builder reports success, so report/manifest/all-lanes drift fails during
  release packaging instead of only during later review. The builder now refuses
  dangerous `--force` output targets and refuses to replace a directory that
  contains the input TOML or phase transcript sources, preventing evidence loss
  during release packaging. Successful production-ready bundle generation now
  prints the verified `manifest_sha256` root directly, and reviewers can run
  `scripts/sccp_verify_release_bundle.py` against the published bundle to
  recompute every attachment hash, emit the verified `manifest_sha256` root for
  archival release review, and catch extra manifest artifacts that are not
  referenced by the readiness report, unknown corridor evidence phase keys,
  skipped or missing required corridor phases hidden behind top-level ready
  flags, non-canonical phase-log destinations, copied TOML evidence drift,
  tampered logs, symlinked manifests or artifacts, unsafe manifest paths,
  unmanifested or omitted required/phase artifacts, non-canonical
  manifest/readiness-report/summary JSON serialization, duplicate keys and
  malformed duplicate-key names in public JSON roots, manifest artifact-order
  drift from the bundle builder's
  public attachment order, release notes that omit the manifest handoff, embedded
  report/summary drift, empty or non-object report/summary JSON roots,
  malformed readiness sections, missing or empty copied input-artifact
  lists, malformed or duplicate input-provenance paths, input-provenance drift
  from the copied evidence artifacts, copied evidence layout drift from
  `evidence/NN-*.toml`, non-canonical readiness-report artifact paths, missing
  or unknown manifest/readiness-report top-level fields, manifest readiness
  header drift from the report and summary, unknown embedded or standalone
  all-lanes summary root or lane fields, malformed all-lanes required-domain
  or blocker scalar lists, all-lanes required-domain drift from published lane
  domains, all-lanes domain roster or chain-label drift from the production
  remote lanes, non-ready or blocked all-lanes root or lane summaries,
  missing-record lane flags, blocked release-checklist items, duplicate
  release-checklist gate ids in report/embedded-evidence/summary roots,
  duplicate public blocker strings, malformed all-lanes lane
  record/hash/source-gate/destination-binding/route sections, zero governed
  source/destination/route hashes, zero destination bridge addresses, missing
  or misplaced lane-specific destination binding network/bridge fields,
  empty/zero/unbacked required source-adapter gate hashes, missing or zero
  required gate audit hashes, unexpected or missing lane-specific gate audit
  keys, blocked required source-adapter gates, non-required lanes carrying gate
  material, and ready source gates with blockers in public all-lanes lane
  summaries, malformed
  lane-specific route-canary transcript sections, expected destination/route
  hash drift, route-canary evidence hashes that replay governed
  source/deployment, destination, route, lane-specific canary hash roles,
  another lane's canary evidence hash, or another lane's governed hash roles,
  EVM-family route-canary zero transaction/public-input words or
  reused route-canary hash roles, including finality-height replay,
  Solana route-canary zero or non-canonical ProgramData addresses,
  TON zero or governed-hash-reused live-account route-canary hashes,
  finalized-head/runtime-code hashes,
  TRON zero owner/recovered route-canary addresses, zero transcript words, zero
  route-canary binding hashes, reused canary hash roles including
  finality-height replay, or recovered signer drift from the transaction owner,
  route-canary route/destination hash drift from sibling lane evidence,
  zero cryptographic evidence row hashes, cryptographic evidence row
  domain/chain or per-field source/destination/source-gate/route/canary drift from
  embedded lane rows, unknown
  manifest
  or report artifact fields, zero or malformed artifact byte counts, malformed
  artifact hash JSON types, malformed
  readiness/checklist boolean JSON types, unknown or blocked corridor root fields, unknown
  or malformed release-checklist fields, unknown or malformed portal/mobile
  submission-surface fields, report/summary drift from verifier-owned direct
  recomputation of the copied evidence TOML, Markdown readiness-report drift
  from the JSON report,
  release-checklist drift from the embedded all-lanes evidence, release-notes
  attachment drift from the verifier-owned canonical manifest/report table,
  non-canonical public JSON root serialization or duplicate JSON root keys,
  manifest artifact-order drift from the canonical release-bundle order,
  user-prover submission-surface drift from the corridor phase results, and
  missing, duplicate, unknown, malformed, unbound, or extra-field per-lane
  cryptographic evidence metadata. The verifier also requires those public
  cryptographic rows to cover every required production domain exactly once,
  keep exact domain/chain types and canonical bytes32 hash text before
  recomputing the public
  cryptographic evidence table from the embedded lane evidence and emits
  field-specific failures for any source-material, source-deployment,
  destination-binding, source-gate required flag/hash/audit hashes,
  route-allowlist, route-canary hash/source, or canary binding mismatch, so a
  release note cannot drift from the governed source, destination, source gate,
  route, or canary hashes that passed all-lanes preflight, and it
  revalidates each copied phase log's canonical path plus corridor marker,
  completion sentinel, phase-block command fragments, and phase-specific
  success markers during public bundle review.
  The report now also renders the user-prover SDK submission surfaces for each
  supported production lane, distinguishing EVM/TRON Torii bridge-proof submit
  payloads from native Solana instruction and TON BOC envelopes that
  networks are outside launch scope. Each surface row uses the
  user-side proof backend labels consumed by the SDK request builders
  (`sccp-solana-recursive-mainnet-v1`, `ton-contract-v1`,
  `evm-groth16-bn254-v1`, and `tron-groth16-bn254-v1`) and is tied back to the
  required JavaScript, Python,
  Swift, Kotlin, Java Android, and core-admission corridor phases, with EVM/TRON
  additionally requiring contract-smoke coverage. The Solana destination
  manifest still binds the `solana-program-v1` target verifier backend, while
  the user-prover surface advertises the recursive backend id consumed by
  browser/mobile proof requests; release-bundle verification now rejects any
  blocked submission-surface row or non-empty validation blocker before the
  surface can be published as validated.
- The all-lanes evidence preflight now emits an explicit `release_checklist`
  that separates required lane records, governed deployment evidence, route
  allowlist binding, live route canary evidence, and unresolved blockers for
  release automation.
- Complete the first-release Offline Bearer Cash pilot over the ZK note and
  nullifier engine. Swift, Kotlin, and Java Android now expose the Bearer Cash
  v1 wallet, note, receive-request, payment-token, ACK, text-codec, and policy
	  names; QR/NFC/Nearby app payloads use only the
	  `wallet-offline-bearer-cash-*` prefixes; and shared fixtures publish
	  `offline_bearer_cash_v1` policy defaults for custody hops, lineage steps,
	  QR/stream payload limits, and Android one-use-key pool sizing. Torii no
	  longer publishes the legacy offline transfer/revocation HTTP compatibility
	  routes or the v1 redeem/audit issuer-unavailable stubs, and the versioned
	  Offline V2 route surface now exposes readiness, key refill, note issue, note
	  redeem, and audit handlers under `/v1/offline/v2/*`. Governance council
	  persist/replace/derive-vrf mutation helpers are no longer advertised in
	  default Torii builds unless `gov_vrf` is compiled, avoiding mounted
	  not-implemented fallbacks in the production route/tool surface. The shared
	  Offline V2 interop fixture now uses the chain-admissible key-certificate
	  version directly, and Swift, Kotlin/JVM, and Java Android SDK constructors
	  mirror that version for wallet-side fixture parity.
  Shared chain-side `OpenVerifyEnvelope` admission now requires exact active
  verifier-key commitment binding and canonical empty auxiliary bytes for
  generic `VerifyProof`, governance voting proofs, STARK shielded
  transfer/unshield wrappers, IVM-proved overlays, IVM host registered-key
  verify syscalls, Kaigi privacy proofs, RAM-LFE proof receipts, identifier
  proof receipts, confidential-transfer-v2 transfer/unshield admission, and
  the Offline/Kagemusha flows. The common chain proof metadata helper used by
  voting, generic proof records, and STARK shielded wrappers now also runs the
  shared envelope validator before circuit/schema/commitment matching. Private
  confidential-transfer-v2 transfer and unshield v2/v3 admission paths run the
  same shared shape gate before confidential schema/circuit interpretation, with
  matched adversarial coverage for blank circuit ids, empty or oversized public
  inputs, empty proof bytes, auxiliary metadata, verifier-key hashes, and active
  circuit indexes. ZK-ACE authorized-transfer admission runs it before
  public-input decoding or STARK wrapper checks.
  Private Kaigi fee admission validates its
  fee-binding auxiliary metadata at the transaction boundary and then
  canonicalizes the internal `ZkTransfer` proof to empty auxiliary bytes, while
  anonymous escrow close prechecks validate the confidential-transfer-v2 proof
  envelope before trusting parsed input commitments. IVM-proved overlay
  admission now reuses the shared envelope validator with node and
  verifier-record proof-byte bounds before semantic replay or verifier dispatch.
  IVM host registered-key verify syscalls also run the same shared validator
  before registry binding, schema matching, and backend verifier dispatch while
  preserving the syscall error-code contract. The generic verifier guardrail
  wrapper now runs the shared envelope validator for decoded Halo2/STARK
  `OpenVerifyEnvelope` payloads before verifier dispatch, while preserving raw
  Halo2 best-effort behavior and STARK inner-proof byte-limit semantics. Direct
  Halo2 IPA and STARK/FRI backend verifier dispatch now also runs the shared
  envelope validator before verifier-key matching and proof verification,
  rejecting blank circuit ids, empty or oversized public inputs, empty proof
  bytes, forbidden auxiliary metadata, zero verifier-key hashes, and mismatched
  verifier-key hashes while preserving the STARK inner-native-proof byte-limit
  split; low-level backend dispatch also rejects proof boxes whose embedded
  backend label differs from the requested verifier backend. The lightweight
  preverify/dedup cache also
  runs the shared envelope validator for recognized `OpenVerifyEnvelope`
  wrappers and rejects malformed backend tags, blank circuit ids, empty or
  oversized public inputs, empty proof bytes, auxiliary bytes, zero verifier-key
  hashes, and verifier-key commitment mismatches before cache insertion, while
  Groth16, Halo2/BN254, and Halo2/KZG labels remain unsupported before dedup
  insertion, preventing failed preverify attempts from poisoning later valid
  proofs. The checked verifier guardrail wrapper rejects the same trusted-setup
  labels before backend dispatch. Developer-only fallback Halo2 fixtures now use
  the same shifted Pow5 pair hash for commitment, nullifier, and Merkle2
  relations instead of additive/unshifted placeholders, with stale-placeholder
  regressions covering the commit-open, anon-transfer, tiny Merkle2, and
  vote-commit Merkle2 samples while those labels remain outside public
  production backend admission.
  The production audit path is now topup-anchored and rejects unbound input
  claims, exact-claim mutations under an issued topup certificate, hidden output
  commitments, cross-asset audits, and public amount mismatches; audit output
  certificates are signature-checked against their declared output account
  before lineage is issued. Audit inputs now require both the exact issued-claim
  replay key and an issued note-commitment replay key from the online-to-offline
  topup or a prior audited output before proof verification. `RedeemOfflineNote`
  applies the same source-commitment anchor before final
  redemption, so claim-only metadata cannot redeem a note whose commitment was
  never issued by topup or prior audit lineage; focused redeem coverage also
  rejects a forged source commitment even when the forged claim key has been
  anchored separately. Audit output
  certificate replay keys are checked against existing topup/audit lineage before
  recursive proof verification, so a one-use certificate anchored by the
  online-to-offline topup cannot be recycled as a new bearer output. Note
  commitments are also replay-checked across both topup issue and audit-output
  domains, so commitments cannot move between online-to-offline loading and P2P
  bearer outputs.
  Recursive proof envelopes now require exact active verifier-key commitment
  binding, inline verifier-key length consistency, the literal canonical
  `offline-note-recursive` circuit id with alias spellings rejected, canonical
  empty auxiliary bytes, and shared trusted-setup/developer-only
  backend classification before verifier-registry lookup.
  Verifying-key registry admission now rejects inline verifier records with
  inconsistent published key lengths on both register and update, and rejects
  explicit trusted-setup backend labels such as Groth16, Halo2/BN254,
  Halo2/BLS12, and Halo2/KZG before they can enter registry or proof-attachment
  admission; standalone setup labels such as `kzg`, `bn254`, `bn256`, and
  `bls12_381`, explicit SRS/CRS/PTAU/ceremony labels, and colon-delimited
  profiles such as `halo2/ipa:kzg` or `halo2/ipa:universal-srs`, are now caught
  by the same shared classifier before broad allowlists can admit them.
  Generic proof attachments also reject developer-only labels before
  envelope matching, and STARK/FRI registry admission applies the same
  trusted-setup label rejection even for keyless records. Verifier-key
  register/update admission also rejects developer-only labels containing
  `debug` or `mock`, including legacy seeded records attempting to refresh
  through update. IVM host verifier snapshots and Torii's
  non-consensus proof/prover worker enforce the same trusted-setup and
  developer-only label policy before
  syscall proof verification or broad backend allowlist matching, and Torii
  prover-worker backend mismatches now stop before verifier-registry lookup.
  The core preverify cache and guardrail dispatch wrappers also reject those
  developer-only labels before dedup insertion or verifier dispatch.
  Torii-generated IVM proof
  attachments include the checked verifier-key commitment for downstream
  proof-submission binding.
  The `zk-preverify` block sidecar path records verified trace digests only;
  the background trace lane revalidates queued traces but no longer emits
  `zk-trace/mock-proof` artifacts while the real transparent IVM trace prover
  remains future work.
  `KagemushaTransfer` is now the chain-side shielded
  offline-offline instruction: it is default-on through settlement config, with
  real execution coverage asserting the default-enabled/non-legacy state; it uses
  the existing ZK asset nullifier/commitment/root accumulator, requires an
  asset-bound confidential-transfer-v2 Halo2 IPA verifier and root hint, rejects
  trusted-setup proof labels, checks the submitted `OpenVerifyEnvelope` backend
  tag, literal
  `halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified` circuit id,
  schema, and verifier-key hash against the active asset binding, rejects
  normalized confidential-transfer-v2 circuit aliases before proof decoding,
  requires inline verifier-key bytes with matching length, commitment, canonical
  `pallas` curve label, non-zero proof-size cap, active circuit/version index, and the canonical
  confidential-transfer-v2 semantic circuit key before proof envelope decoding,
  and caches canonical confidential-transfer-v2/unshield verifier and proving
  keys so production guard/prover paths do not repeatedly regenerate
  no-trusted-setup Halo2 IPA keys. Canonical confidential verifier-key
  envelopes also carry a `CID1` circuit-id TLV, keeping unshield v2/v3
  commitments separate even when the raw Halo2 payloads are structurally
  parseable across related circuits, and the Halo2 IPA verifier rejects a
  verifier-key envelope whose `CID1` names a different circuit than the proof
  envelope. It leaves legacy bearer-audit forcing available only as an explicit
  migration fallback.
  Offline recursive prover entry points, proving-key derivation, and chain-side
  verifier resolution now also pin inline verifier keys to the canonical
  `offline-note-recursive` semantic circuit key, rejecting self-consistent
  forged records or local keys before proof creation or backend verification.
  Offline recursive verifier-key envelopes also carry a `CID1` circuit-id TLV,
  and the backend verifier rejects a raw Halo2 verifier-key replay whose `CID1`
  names a different circuit family.
  Compact multi-hop Kagemusha tokens now have a deterministic folded
  public-input transcript in the Rust data model; the transcript canonicalizes
  bounded private hops, rejects duplicate nullifiers/commitments and root
  discontinuities, and binds chain id, asset definition, roots, hop count, and
  aggregate folded-hop digests. The folded statement now carries a proved
  aggregation-mode column; checked transparent pre-fold v1 remains supported
  while ABI-7 recursive compact mode `2` remains reserved behind the
  fail-closed `kagemusha-recursive-compact-v1` proof/verifier surface. A Poseidon2
  aggregation transcript digest is derived from the same canonical hop sequence
  as the recursive verifier's public accumulator. Checked fold construction verifies each private hop
  proof and binds its verified public-input statement plus verifier-key
  id/commitment and a Poseidon2 digest of the verifier-key backend/bytes before
  hashing it into the transcript; optional envelope-hash metadata must match
  the submitted envelope bytes before private hops or chain-side transfers are
  accepted, and raw checked folding enforces the confidential-transfer-v2
  literal circuit id with alias spellings rejected alongside the proof-size cap,
  per-hop shape bounds, root continuity, duplicate-set checks,
  non-zero set entries, mandatory verifier-key commitment and verifier-key-id
  metadata, non-empty verifier-key bytes, canonical confidential-transfer-v2
  verifier-key bytes, canonical empty envelope auxiliary bytes, and the 64-hop
  compact-token corridor before parsing private-hop envelopes. Chain-side
  Kagemusha transfers now apply the same duplicate/non-zero set invariants
  before proof envelope decoding.
  The proof-statement
  preimage is exposed in the data model as `KagemushaProofPublicInputsStatement`
  and hashed through the canonical Norito/Poseidon2 helper, which now rejects
  non-empty envelope auxiliary bytes, zero verifier-key hashes, and Halo2 IPA
  confidential-transfer-v2 circuit-id aliases before transcript material is
  derived at the core verifier helper. The public data-model API enforces the
  shared auxiliary-byte and verifier-key hash rule, so SDKs and future recursive
  circuits share the same canonical target format. The public Kagemusha
  transcript helpers also reject
  unsupported, trusted-setup, and developer-only backend labels before hashing
  per-hop verifier-key material or folding verifier-key ids. The
  proof-statement helper now also rejects empty circuit ids, schema bytes,
  missing or empty instance columns, empty verifier-key bytes, and empty
  folded-hop verifier-key id names before they can become wildcard transcript
  material. The shared STARK/FRI
  classifier also rejects the profile-less `stark/fri/` prefix, whitespace-only
  STARK/FRI profile suffixes, padded or embedded-whitespace profile labels,
  punctuation-bearing or nested suffixes, non-ASCII suffixes, trusted-setup
  STARK/FRI profiles such as KZG, BN254, and BLS12, and any
  STARK/FRI profile containing developer-only `debug` or mock labels before
  verifier admission reaches proof decoding, and Torii proof/prover paths
  mirror that rule while fatal prover-worker attachment classification errors
  return before registry lookup. The shared trusted-setup classifier also
  covers standalone KZG/pairing labels and colon-profile setup labels with
  ASCII-case-insensitive matching, so registry admission, preverify, guardrails,
  and Torii's broad prover allowlists all fail closed on mixed-case forms such
  as `halo2/ipa:KZG` or `halo2/ipa:Mock-Proof`. It also tokenizes setup markers
  across every non-alphanumeric delimiter, so padded and punctuation-spliced
  setup labels such as `halo2/ipa: KZG`, `stark/fri/prod;kzg`,
  `stark/fri/prod+bn254`, and `stark/fri/prod-bls12-381` fail closed at the
  same boundaries. The classifier also normalizes delimiter-inserted setup
  spellings such as `stark/fri/prod-bn-254`,
  `stark/fri/prod-groth-16`, and `stark/fri/prod-k-z-g` before broad
  STARK/FRI profile admission. Developer-only markers are normalized the same
  way, so delimiter-inserted `d-e-b-u-g` and `m-o-c-k` spellings cannot pass
  production allowlists. Gas metering and generic proof envelope metadata helpers now
  apply the same gate before decoding pre-validation Halo2 metadata. Kotlin/JVM
  and Java Android Offline Note recursive proof models now trim verifier/proof
  backend metadata and reject malformed verifier-key separators before SDK
  proof-binding validation.
  Compact folded-token verification now also has
  explicit final-proof coverage rejecting trusted-setup and developer-only
  backend labels in both direct and record-backed verifier paths. Direct
  Poseidon2 aggregation transcript hashing now validates
  canonical mode/count/index/root continuity, sorted non-zero folded sets,
  duplicate-free membership, and
  supported transparent verifier-key backends before hashing, and rejects
  zero proof public-input digests, verifier-key commitments, or verifier-key
  Poseidon2 digests as wildcard binding material. The folded public-input model
  also rejects zero or over-64 hop counts, all-zero initial/final roots, and
  unchanged hop/public root transitions, and all-zero aggregation transcript
  digests during data-model context validation, and exposes a 1 KiB encoded-size
  budget plus `norito_encoded_len()` helpers so
  mobile transports can enforce compact QR/NFC payload corridors before adding
  backend proof bytes; folded-context validation rejects over-budget public
  transcripts. The Poseidon2 aggregation statement is exposed publicly as
  `KagemushaPoseidonAggregationTranscriptStatement` with a canonical builder
  and digest helper, plus host-side projection helpers that recompute every
  folded public-input digest column from a full aggregation statement. This
  gives SDKs and future recursive circuits the same canonicalized target layout
  and catches transcript/public-input mismatches before proof generation. The
  high-level compact-token
  prover uses that checked path before
  emitting the first
  `kagemusha-folded-v1` transparent Halo2 IPA proof, which proves and verifies
  the 30-column folded public statement without a trusted setup, constrains the
  public-input hash, initial/final roots, and aggregate digest columns to be
  non-zero inside Halo2 via inverse witnesses, proves the final folded root
  differs from the initial root with a selected-limb inverse witness, and pins
  the final folded proof envelope to canonical empty auxiliary bytes and the
  literal `kagemusha-folded-v1` circuit id. Folded-token proving, proving-key
  derivation, and direct or record-backed token verification now also require
  the canonical `kagemusha-folded-v1` semantic verifier key before backend
  verification, so a self-consistent forged verifier record cannot substitute a
  recursive aggregation or other cross-circuit key. Folded verifier-key
  envelopes carry a `CID1` circuit-id TLV too, and backend verification rejects
  a structurally compatible raw Halo2 key replayed under another circuit id. The
  Pasta circuit module now also contains reusable non-native foundations for the
  future in-circuit Vesta/Fq IPA verifier: a `u64` limb decomposition gadget
  that proves 64 boolean
  little-endian bits and rejects high residue above bit 63, a native Pasta/Fp
  scalar decomposition gadget that supports public or private scalar exposure,
  binds the scalar to four private `u64` limbs, proves the canonical 255-bit
  representation below the Fp modulus, and rejects `value + modulus` aliases, a
  canonical Vesta/Fq range gadget that proves four limbs are below the Vesta
  base-field modulus through a private slack and borrow chain, modular Vesta/Fq
  addition with
  unreduced-sum and reduction carry-chain checks, and modular Vesta/Fq
  multiplication with schoolbook product limbs, private `u128` carry chains, and
  a private canonical quotient, plus a Vesta affine on-curve check that links
  public `x/y` coordinates to private `x*x`, `y*y`, `x^2*x`, and `x^3 + 5`
  witnesses and a distinct affine point-addition gadget that composes on-curve
  checks with private denominator-inverse, slope, and output-coordinate
  equations, plus an affine point-doubling gadget that proves an invertible
  `2*y(P)` denominator and links `lambda * (2*y(P)) = 3*x(P)^2`, and a
  point-or-identity validity gadget with canonical `(0, 0, 1)` identity
  encoding. Complete point-or-identity addition now covers identity
  passthrough, inverse-pair output identity, doubling, and distinct affine
  addition under one-hot branch selectors. A conditional-add layer now binds a
  private selected addend to a boolean scalar bit, and the first bounded
  scalar-multiplication wrapper links public scalar-limb decomposition, the
  addend doubling ladder, private accumulator steps, and public base/output
  point encodings. A native-scalar scalar-multiplication wrapper now consumes
  canonical Pasta/Fp scalar decomposition bits directly, enforces high-bit
  zeroing for bounded widths, and proves the same private addend-doubling ladder
  from the public base. A fixed-window Pasta/Fp scalar decomposition gadget now
  proves deterministic little-endian window digits for the production
  windowed-MSM path, links every digit bit to the canonical private scalar bit,
  and constrains high scalar bits above the configured window width to zero. A
  non-native Vesta fixed-window point selector now proves that a selected
  private point-or-identity comes from a private `2^WINDOW_BITS` table through a
  quadratic binary selection network. A companion table-derivation gadget now
  proves the private table is exactly `[0, B, 2B, ...]` for a public base point
  by linking entry zero to identity, entry one to the public base, and later
  entries to a complete-add chain. A fixed-window native-scalar multiplication
  wrapper now composes scalar windows, shifted-base tables, selectors,
  per-window base doublings, and selected-point accumulation into a public
  `output = scalar * base` statement. The remaining windowed-MSM layer composes
  multiple windowed scalar-multiplication terms into one public multi-scalar
  accumulator. A bounded native-scalar Vesta MSM wrapper now composes
  private canonical Pasta/Fp scalar witnesses, public base encodings, per-term
  private scalar-multiplication ladders, and a private running sum into one
  public output point, rejecting public base/output substitution, scalar-bit
  tampering, noncanonical scalar aliases, broken double ladders, and unchained
  private MSM accumulators. The first IPA-specific composition wrapper now
  proves the final verifier comparison `Q = a*G + b*H + (a*b)*U` by reusing the
  three-term bounded MSM and constraining the third scalar to the native-field
  product of the first two, so a self-consistent MSM cannot forge the IPA
  product term. The per-round accumulator update `Q' = x^2*L + Q + x^{-2}*R`
  now uses the fixed-window MSM path with private canonical `x` and `x^{-1}`
  witnesses constrained as inverses and linked to the MSM scalars `x^2`, `1`,
  and `x^{-2}`. Generator folding now also has a shared-challenge wrapper
  proving `G' = x^{-1}*G_L + x*G_R` and `H' = x*H_L + x^{-1}*H_R` with two
  linked fixed-window two-term MSMs. The native-field IPA `b`-vector fold
  `b' = b_L*x^{-1} + b_R*x` now has public-input scalar and fixed-size
  segment-vector gadgets with one shared private canonical challenge pair and
  adversarial coverage for inverse, input/output, and noncanonical scalar
  tampering. A multi-round `b`-vector reduction gadget now folds the whole
  power-of-two public vector to the final public scalar while keeping
  intermediate vectors private and canonical; its round challenges and inverses
  are public circuit inputs linked to private canonical decompositions so the
  recursive circuit can bind externally projected Fiat-Shamir challenges. The
  native transparent IPA verifier now derives the same projection and rejects
  substituted `proof.b_final` values. Native IPA vector commitments now use backend-level
  deterministic MSM hooks, with Pallas and BN254 using `halo2curves::msm_best`
  and simple backends retaining the generic deterministic fallback. A
  fixed-window native-scalar MSM wrapper now composes private canonical scalar
  windows, shifted-base tables, table selections, private per-term outputs, and
  the final public multi-scalar accumulator, with adversarial coverage for
  substitution and splice attacks. The IPA final comparison now also has a
  fixed-window `Q = a*G + b*H + (a*b)*U` wrapper with the same third-scalar
  product-link invariant. The bounded, fixed-window, and shared-table
  fixed-window final MSM wrappers now also have explicit identity-output
  coverage, proving that the verifier comparison can end at the canonical point
  at infinity without leaving the complete-add/point-or-identity path. The
  composed one-round/generic verifier wrappers now feed the round accumulator,
  generator folds, and final comparison through the fixed-window MSM path. The
  native accumulation projection also rejects
  mismatched challenge inverse witnesses.
  A one-round in-circuit verifier composition slice now shares one canonical
  challenge/inverse pair across `b` folding, the `Q` accumulator update,
  generator folding, and final MSM comparison, with direct advice links for
  folded `b`, `Q'`, `G'`, and `H'`. A native transparent IPA
  round-transcript projection helper records the `ipa.n` state boundary, each
  round's `L/R` bytes, round-byte digest, transcript states, challenges,
  challenge inverses, and final transcript state, and a native verifier
  accumulation projection records `Q`, folded `g/h`, challenge squares, final
  folded generators, and the final expected term. A combined native verifier
  witness now validates those transcript, reduction, accumulation, and final
  scalar projections together for future recursive-verifier witnesses, all
  without adding a trusted setup. A field-friendly transcript-binding projection
  now maps the SHA3-validated transcript header, complete round projections,
  challenge/inverse pairs, and final transcript state into Pasta/Fp scalars and
  folds them through a transparent Pow5 accumulator; a matching native Pasta/Fp
  circuit enforces that accumulator over public projection/challenge inputs and
  rejects public substitution or intermediate-state tampering. The generic
  multi-round non-native Vesta IPA verifier now composes that accumulator and
  links its challenge rows back to the verifier's decomposed `b`-reduction
  challenge columns, so self-consistent transcript witnesses cannot be spliced
  onto a verifier using different challenges. The host bridge now accepts native
  Pallas IPA verifier witnesses, validates their transcript projection,
  re-derived transcript binding, `b`-reduction and accumulator projections,
  round ordering, and canonical compressed point encodings through a cheap
  preflight path, recomputes the native Pallas `b`, `Q`, `G`, `H`, and
  final-term fold relations with the deterministic optimized Pallas MSM backend,
  and translates their scalars and compressed Vesta points through canonical
  byte encodings before building the recursive Vesta verifier witness. The same
  bridge now validates ordered batches of native Pallas
  verifier witnesses and emits a compact streaming Poseidon2
  domain-separated aggregate digest that binds the transparent parameter
  fingerprint, verifier opening length, witness order, transcript projections,
  `b` reductions, accumulator folds, final terms, and proof-final scalars after
  each witness passes preflight. The data model now exposes a
  reserved-mode recursive aggregation evidence statement that
  Norito/Poseidon-binds that batch digest, parameter fingerprint, and canonical
  `pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3` verifier-witness
  profile plus the declared verifier opening length to the same ordered hop
  transcript. Reserved compact projection checks validate mode `2` against that
  recursive evidence and the compact token's folded public inputs, but public
  compact-token admission stays disabled until the composed verifier-slice proof
  replaces the semantic aggregation proof. The
  Poseidon2 aggregation transcript digest accepts checked mode `1` and
  recursive compact mode `2` but rejects unknown modes, plus Norito roundtrip,
  decoded-profile/opening-length/schedule/base, and truncated-archive negative
  coverage plus unsupported or non-power-of-two opening length, zero table
  commitments, empty-transcript, over-cap, duplicate-nullifier, and
  duplicate-commitment rejection. Core record-backed
  evidence builders now enforce active WSV-style confidential-transfer-v2
  verifier records, verify every private hop proof, reject mismatched witness
  counts, unsupported opening lengths, all-zero native batch metadata, or
  opening-length/schedule-digest mismatches before hop proof decoding, and then
  bind the batch preflight digest to the canonical hop transcript for both
  borrowed and serializable record bundles. A public Pallas
  IPA batch preflight helper now accepts only the current production
  no-trusted-setup width corridor `2..=128` plus the 64-hop compact-token cap,
  keeps the aggregate batch digest on the same Poseidon2-backed transcript
  family as reserved recursive evidence, and the combined record-backed
  builders can take native verifier witnesses directly, re-derive the stored
  batch digest with the ordered checked-hop proof hashes, and reject detached,
  wrong-width, or spliced batch evidence before hop proof decoding. The native
  batch digest now also binds the recursive fixed-window table profile and the
  Poseidon2 digest of the deterministic shared-table schedule plus a Poseidon2
  digest of the ordered fixed-window table bases used by the validated native
  witnesses, and a proof-derived Pallas opening-envelope path reconstructs
  native verifier witnesses from the transparent IPA proof envelope before
  applying the same preflight and hop-proof-hash binding. That path now applies
  the Kagemusha `2..=128` power-of-two opening corridor and `k = 7`
  transparent-envelope resource limit before verifier-witness derivation, so
  unsupported or oversized opening envelopes, mismatched wire versions,
  generator vectors, and proof-round vectors reject before parameter/proof
  reconstruction. Hop-proof hash count mismatches now reject before native
  witness preflight or proof-derived envelope witness derivation, keeping
  detached reserved-evidence batches from forcing unnecessary verifier work.
  Kagemusha proof-derived preflight also rejects empty or over-128 byte
  transcript labels and requires non-zero verifier-key commitment,
  public-input schema, and hop-domain metadata before verifier-witness
  derivation, so detached generic polynomial-opening envelopes cannot enter
  reserved recursive evidence.
  The record-backed
  proof-derived path
  now also requires each Pallas opening envelope to carry hop-derived transcript
  metadata: verifier-key commitment, confidential-transfer-v2 schema hash, and a
  Poseidon2 domain tag over chain, asset, hop index, roots, nullifiers, outputs,
  proof hash, public-input digest, and verifier-key binding. This closes
  profile-string/table-accounting substitution, detached native-opening witness
  replay, and unbound opening-envelope replay at the reserved-evidence layer.
  Deriving each IPA verifier witness inside the recursive circuit from the
  corresponding compact-hop Halo2 proof envelope remains part of the mode-2
  recursive circuit work. A cheap
  production-layout guard now pins
  the `n = 128` recursive verifier shape (seven rounds, `[64, 32, 16, 8, 4, 2,
  1]` generator-fold layers, 85-by-3 scalar coverage, and 262 represented
  windowed MSM gadgets), and a fixed-window table plan pins the shared-table
  target at 532 table families versus 90,440 naive point-table copies
  (723,520 duplicated point rows) with `trusted_setup_required = false`. Both
  guards reject unsupported widths; full production-width witness
  materialization remains too large for regular tests until the circuit uses
  shared/compressed fixed-window table evidence. A deterministic shared-table
  schedule and Poseidon2 schedule digest now enumerate those 532 table families
  and 45,220 shifted-window tables explicitly, giving the compressed recursive
  verifier work a no-trusted-setup commitment target instead of only a row-count
  estimate. A concrete shared-table manifest now assigns those families to
  contiguous shared-row ranges, binds that row layout with Poseidon2, and keeps
  `trusted_setup_required = false`; reserved recursive evidence now carries the
  opening length plus schedule, shared-table manifest, and table-base digests
  explicitly with unsupported-opening, non-power-of-two-opening,
  zero-schedule/manifest/base, and schedule/manifest-mismatch rejection before
  hop proof decoding. Native batch preflight also exposes that manifest digest,
  and hop-bound batch preflight binds it with the schedule digest and ordered
  checked-hop proof hashes. The data model now also has a proof-carrying
  recursive aggregation public-input bundle whose 59 public instance columns
  bind transparent no-trusted-setup proof metadata to the recursive evidence
  digest, folded public input hash, aggregation transcript digest,
  verifier-parameter fingerprint, fixed-window schedule digest,
  shared-table manifest digest, table-base digest, native witness-batch digest,
  recursive spend proof-chain digest, non-circular transition-profile binding
  digest, Reserved-lineage append opening preflight digest, compact
  Reserved-lineage append-boundary digest, reserved recursive verifier
  scalar-projection digest, opening length, witness count, and hop count while
  rejecting backend, circuit-id, public-input-hash, empty proof payloads, and
  evidence-field substitution. The proof-bundle guard is
  pinned to the
  canonical transparent Halo2 IPA/Pasta recursive aggregation circuit; supported
  STARK/FRI labels remain hop-transcript material only and cannot stand in for
  this in-tree recursive proof. Core now also pins the schedule and shared-table manifest
  digests to the declared opening width during recursive proof generation,
  preverification, and the transparent semantic circuit, so a self-consistent
  forged envelope cannot swap those verifier-layout commitments. Recursive
  aggregation preverification also rejects cross-circuit verifier keys even when
  the supplied inline key, verifier-key commitment, and proof-envelope `vk_hash`
  are internally consistent. Recursive verifier-key envelopes carry `CID1`
  circuit-id TLVs, and backend verification rejects structurally matching raw
  verifier-key payloads whose `CID1` names another circuit family. Core now
  prevalidates that bundle against active Kagemusha verifier records by checking
  the non-empty transparent Halo2 IPA envelope, canonical circuit id, verifier-key hash,
  public-input schema,
  empty auxiliary bytes, exactly 59 one-row Pasta instance columns, proof-size
  cap, inline key length, and verifier-key commitment, rejecting shortened,
  extended, or multi-row recursive instance vectors before semantic public-input
  comparison. The shared Halo2 proof-envelope parser also rejects trailing
  unbound suffix bytes after the declared proof payload, and Kagemusha
  recursive preverification treats ZK1 inner proof envelopes as canonical
  `PROF + I10P` material, rejecting unexpected or duplicate TLVs. Core exposes
  a canonical verifier-record helper for supplied transparent Halo2 IPA
  recursive aggregation key bytes.
  The detached-evidence prover and raw metadata evidence builder are
  crate-private implementation helpers, leaving the public proof-bundle API on
  the record-backed native Pallas preflight/open-envelope paths. The
  ZK1 public-instance parser remains bounded but now admits this 59-column
  recursive aggregation envelope through core and the native bridge. This
  remains a reserved evidence surface until the recursive verifier proof
  itself is complete. Recursive spendable offline cash now uses a separate
  production accumulator and Norito bundle, `KagemushaRecursiveSpendBundleV1`.
  It carries the public accumulator state, current spendable note descriptor,
  chain/asset/final-root/final-commitment binding, verifier references, hop
  count, and one recursive proof instead of prior hop bundles. Recursive spend
  redemption now binds a compact
  chain-visible top-up anchor set into the accumulator: the first-hop input
  nullifiers from the online-to-offline top-up lineage are sorted, included in
  the accumulator digest and recursive proof public inputs, and consumed
  alongside the final spendable note nullifier at redemption. This prevents two
  hidden recursive branches from redeeming from the same top-up anchor while
  preserving hop-count-independent payload size. Append hops are restricted to
  consuming the previous spendable note nullifier only, so they cannot merge
  fresh external inputs whose nullifiers are not part of the original top-up
  anchor set or create a current note that reuses the consumed append nullifier.
  Accumulator context validation also rejects forged top-up-anchor/current-note
  commitment collisions and current note spend nullifiers that collide with any
  output commitment in the hop that created the note.
  Recursive spend nullifier, output, and fold-transcript streams now use
  accumulator-only domain tags instead of the checked folded-token
  list/transcript digest tags; the C bridge and Python native redeem helpers
  also reject zero or mismatched public amounts before instruction emission. The
  first shared-table
  circuit primitive is now in
  place as well: fixed-window selection can reference an already-derived Vesta
  table directly instead of assigning a duplicate private selection-table copy,
  and shared-table native-scalar multiplication now composes that selector with
  scalar decomposition, shifted-base table derivation, window-base doubling, and
  selected-point accumulation. Shared-table multi-term MSM composition now
  chains those scalar-multiplication terms into one public MSM output while
  keeping term outputs private, and shared-table final IPA MSM composition adds
  the native-field `a * b` product link for the `U` term. Shared-table
  round-accumulator and generator-fold composition now binds those MSM scalars
  back to the transcript challenge and inverse for per-round IPA verification,
  and the one-round shared-table verifier slice now preserves the `b`-reduction,
  final-output, and folded-generator cross-links across those shared-table
  components. The multi-round shared-table verifier builder now mirrors the full
  transcript-binding, `b`-reduction, accumulator-chain, folded-generator-chain,
  and final-MSM host-link structure without duplicated selection-table copies;
  full LEN=4 MockProver coverage is present for an honest synthetic statement,
  real Pallas opening translation, public-instance substitution, `Q` splice,
  generator-fold splice, challenge splice, and final-MSM splice, but those tests
  are ignored by default because the composed non-native layout is too expensive
  for routine validation. The normal suite keeps builder-level, native Pallas
  preflight, batch-preflight, and host-link adversarial coverage active.
  The first recursive-aggregation verifier-slice composition is also in place:
  a one-hop circuit composes the recursive aggregation semantic metadata proof
  with the shared-table `LEN = 2` IPA verifier, including transcript-binding
  accumulator checks, and links the public opening length, witness count, and
  hop count to the single-hop profile. Its active coverage checks builder
  acceptance, profile and metadata-witness mismatch rejection, stale semantic
  non-zero inverse rejection, zero public digest-group rejection, native Pallas
  preflight metadata binding, preflight digest/fingerprint substitution
  rejection, zeroed preflight digest rejection, rejection when a valid Pallas
  preflight is paired with an invalid verifier witness, and rejection when a
  production Pallas preflight is paired with a non-production fixed-window
  profile. The shared-table host-link guard also enforces the expected IPA round
  count and per-round `b`-reduction and generator-fold layer widths, so a
  composed recursive verifier witness cannot omit an initial `b` layer or
  generator-fold layer and rely only on the final MSM relation. The native and
  shared-table verifier `synthesize` paths now repeat those witness/config
  shape checks before assignment, so malformed direct circuit witnesses fail
  closed instead of silently skipping omitted `b`-reduction or generator-fold
  regions. The composed circuit now also exposes a public verifier
  transcript-binding digest instance and links it to the embedded verifier's
  transcript-binding accumulator. It also exposes a
  public scalar-projection digest over that digest, the public `b`-reduction
  input scalars, challenge, inverse, and final folded `b` scalar, and
  constrains that projection with the same field-friendly Pow5 compressor.
  One-hop constructors now recompute the scalar projection from the host
  verifier witness and reject semantic public-input limb splices before circuit
  assignment.
  The shared-table verifier host-link guard also mirrors the final IPA MSM
  product relation, rejecting `a_final * b_final` product splices before a
  composed one-hop recursive verifier witness can be accepted.
  The production one-hop host API can now re-derive the Pallas preflight from
  the supplied witness and require an exact digest/fingerprint match before
  materializing the recursive Vesta verifier, so metadata from one valid
  witness cannot be paired with another self-consistent verifier witness. It
  can also re-derive the reserved hop-proof-hash-bound preflight from the
  supplied witness plus the expected hop proof hash, so one-hop mode-2 evidence
  cannot accidentally validate against the detached native batch digest. The
  hop-bound path now has a dedicated constructor separate from the detached
  native-batch constructor, so reserved evidence callers cannot materialize a
  one-hop verifier slice through the wrong preflight shape. The hop-bound guard
  now derives the native preflight through the verifier slice's declared opening
  length before binding the hop proof hash, so a LEN=4 witness cannot validate
  through a LEN=2 one-hop slice. The constructor also revalidates recursive
  semantic non-zero inverse witnesses and rejects all-zero preflight
  fingerprint, table-schedule, shared-table-manifest, table-base, or
  verifier-batch digests before accepting a composed witness.
  Digest-splice, scalar-projection side-instance splice, scalar-projection
  semantic public-input splice, and direct verifier synthesis-shape MockProver
  cases for omitted `b`-reduction and
  generator-fold layers, production fixed-window Pallas verifier
  materialization, and composed MockProver acceptance/public-count-splice tests
  remain ignored heavyweight coverage.
  This is still not mode-2 admission because the private-hop verifier batch and
  complete Poseidon2 witness-batch digest relation are not yet proved inside the
  compact-token circuit.
  The same shared-table verifier can now be constructed from proof-derived
  native Pallas verifier witnesses after the existing native Pallas preflight
  validates transcript, `b`-reduction, accumulator, and generator-fold
  consistency, and the public Kagemusha Pallas batch-preflight path now
  dispatches through the shared-table verifier entry point while exposing the
  same ordered schedule, shared-table manifest, table-base, and aggregate digest
  metadata used by reserved recursive aggregation evidence. Recursive preflight
  Poseidon2 transcripts now field-label length encodings, closing same-length
  cross-field replay before mode-2 evidence becomes admissible. Production-width
  coverage now binds the shared-table verifier shape to the `n = 128` manifest
  without materializing every recursive witness object in routine tests.
  Alias
  spellings are rejected at compact-token proving
  and verification boundaries. Derived Halo2 IPA proving keys for IVM,
  Offline Note, and Kagemusha now use Norito archives
  that bind the canonical circuit family and verifier-key commitment before raw
  Halo2 key bytes are decoded, rejecting raw or cross-circuit key material while
  preserving production key caching. Mobile and bridge callers must use the
  record-backed compact-token prover
  `connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`
  so private hops are tied to active WSV-style confidential-transfer-v2 verifier
  metadata, including canonical `offline_kagemusha` namespace, backend tag,
  canonical curve label, circuit id, schema hash, verifier-key commitment, key
  length, proof-size cap, optional inline-key consistency, and exact record-set
  matching with no
  unrelated records at the FFI boundary, while raw folded-input proof
  construction stays crate-local. The final folded-token record verifier applies
  the same canonical namespace and registry metadata gate before backend proof
  verification, and recursive final redeem/unshield verifier records apply the
  same namespace/backend/curve, active circuit/version, and canonical inline
  unshield-key gate before the public mint path runs. The older unanchored C
  symbol and Rust compact-token proving entry points remain present for ABI
  compatibility but reject even valid `KagemushaVerifiedFoldBundle` input
  without returning a token.
  Bridge ABI 6 adds recursive spend `init`, `append`, both transition-profile
  helpers, append-boundary derivation, both lineage-witness assembly helpers,
  `verify`, and `redeem` entry points over raw Norito archives, and the C header
  plus Swift, Kotlin/JVM, Java Android/JNI, JavaScript/Node NAPI, Python/PyO3,
  and C# surfaces mirror them with empty-input and malformed-archive rejection.
  Native bridge tests now also seed stale output pointer/length slots across
  recursive-spend, lineage-witness, unanchored compact-token, record-backed
  compact-token, record-backed recursive aggregation, and recursive compact
  adversarial paths, so every malformed or semantic rejection must clear
  archive outputs before returning. The C bridge now also routes Kagemusha
  ABI-6/ABI-7 archive inputs through a shared bounded reader, with native
  tests pinning oversized-length rejection before raw slice construction.
  The
  SDK/native-output guard now also distinguishes missing native proof archives
  from zero-length archives across Python, Swift, JavaScript/Node, Kotlin/JVM,
  and Java Android, keeping native prover boundaries fail-closed without
  disabling recursive Kagemusha by default. The
  shared redeem-request validator now also rejects final redeem proof
  attachments that leave the current transparent `halo2/ipa` production
  corridor, carry inconsistent attachment/proof/verifier-key backends, have
  empty proof bytes, omit or publish a zero verifier-key commitment, or carry a
  mismatched envelope hash before native/bridge instruction construction. It
  also rejects recursive spend bundle proofs that are not `halo2/ipa` or carry
  empty proof bytes, while ledger-side transfer and recursive redeem admission
  now independently reject all-zero verifier-key commitments and all-zero proof
  envelope verifier-key hashes before WSV verifier-record comparison. Legacy
  Offline recursive verifier resolution now applies the same zero envelope-hash
  rejection before verifier-record comparison. Record-backed compact-token
  verification, recursive aggregation preverification, and checked fold-hop
  admission now also reject all-zero verifier-record commitments explicitly
  before inline key commitment comparison. These guards keep native/bridge
  redeem construction inside the same production corridor as chain-side
  verifier-record admission. Chain-side recursive redemption now has a
  production record-backed admission path for current semantic
  `kagemusha-recursive-aggregation-v1` spend proofs: the redeem instruction can
  carry a full lineage witness with the checked hop record bundle, Pallas
  open-envelope archive, per-hop current-note descriptors, and the intermediate
  recursive proofs committed by `recursive_proof_chain_digest`. Execution
  first requires the supplied hop verifier-record snapshots to be the exact
  currently registered WSV records, then verifies the private hop records and
  envelopes, replays the accumulator, verifies those intermediate recursive
  proofs, and requires the recomputed accumulator to equal the redeem bundle
  before nullifier consumption or public minting. Semantic v1 spend proofs
  without that witness still fail closed as admission-neutral, because they do
  not prove every private hop and accumulator
  transition in-circuit. The reserved chain-admission circuit id for the
  witnessless constant-size proof is
  `kagemusha-recursive-spend-lineage-v1`; profile attempts under that id must
  stay in the transparent `halo2/ipa` corridor, carry non-empty proof bytes,
  bind the accumulator-derived recursive public inputs through a fresh
  public-input hash, include a non-zero recursive verifier scalar-projection
  digest, and expose an inner `OpenVerifyEnvelope` whose backend tag, lineage
  circuit id, schema, empty auxiliary metadata, non-zero verifier-key hash, and
  public instance columns match that reserved profile. Those instance columns
  must now come from a strict ZK1 no-trusted-setup inner proof envelope; legacy
  Halo2 proof-envelope wrappers remain accepted only for semantic v1
  preverification and are rejected under the reserved lineage id.
  Record-backed preverification
  also requires the inline verifier-key envelope to be a strict
  no-trusted-setup Halo2 IPA ZK1 key container: exactly one matching lineage
  `CID1`, exactly one bounded `IPAK` degree, exactly one non-empty `H2VK`, and
  no unrelated key TLVs. The guard reads the cheap Halo2/Pasta verifier-key
  header and requires the `H2VK` domain degree to match the bounded `IPAK`, so
  relabelled semantic keys reject before the heavy one-hop verifier-slice
  circuit is materialized; it also requires the payload to contain the declared
  fixed-column commitments so truncated processed verifier keys fail during
  cheap preflight. It also rejects zero verifier-record commitments explicitly
  and pins the lineage proof envelope verifier-key hash to the verifier-record
	  commitment. That preverification remains admission-neutral for registry-only
	  callers. Chain admission validates the current Reserved-lineage profile and
	  admits witnessless redemption for profile-valid bundles inside the configured
	  64-hop cap after the active lineage verifier record, final proof, root,
	  asset, commitment, and nullifier checks pass.
  Missing `IPAK`,
  missing `H2VK`, wrong IPA degree, duplicate
  verifier-key `CID1` tags, unexpected verifier-key TLVs, malformed trailing
  TLV material, truncated fixed-column commitments, and legacy semantic inner
  proof envelopes now reject as malformed instead of allowing last-tag-wins,
  prefix-only circuit identity, or cross-profile proof payload replay.
	  The recursive spend accumulator, append proof-artifact digest, and bridge
	  redeem request validation now understand both semantic v1 and reserved
	  lineage proof ids, so lineage-profile states can be represented in the
		  accumulator and verified as D2D payloads. Direct redeem instruction
		  serialization still requires a record-backed lineage witness for semantic v1
		  bundles, while metadata-valid Reserved-lineage bundles redeem witnesslessly
		  inside the configured 64-hop cap after active lineage-verifier-record and
		  final-proof verification.
  Recursive spend append requests now carry an optional previous-lineage
  verifier record. Semantic v1 previous proofs must leave it empty and continue
  through the canonical recursive aggregation verifier; reserved-lineage
  previous proofs must provide the active lineage verifier record, while
  missing records, semantic verifier records, malformed records, and tampered
  previous proofs fail closed before the next hop is folded. Recursive spend
  verify requests likewise carry an optional `lineage_verifier_record` for the
  received bundle: reserved-lineage D2D payloads require a matching active
  record, semantic v1 payloads must omit it, and the bridge, JavaScript host,
  and Python PyO3 host reject malformed verify request archives before returning
  a diagnostic result.
  Data-model init/append request validation now runs before the recursive
  prover in the C bridge, JavaScript host, and Python PyO3 host. Init preflight
  rejects malformed one-hop fragments, Pallas envelope archive count mismatches,
  bad verifier-record sets, and spendable-note output/nullifier splices. Append
  preflight rejects previous-proof attachment drift, missing or forbidden
  previous lineage records, malformed lineage record metadata, chain/asset/root
  discontinuity, amount drift, missing previous-note nullifier consumption, and
  top-up-anchor output reuse before Halo2 proving starts.
  Rust-facing recursive spend init wrappers and ABI-6 native spend init now
  default to the reserved `kagemusha-recursive-spend-lineage-v1` first-hop
  prover in core, the native bridge, JavaScript host, and Python PyO3 host. The
  core wrapper derives the Pallas open-envelope width from the raw Norito
  archive, selects the matching no-trusted-setup one-hop lineage verifier key,
  and emits bundles whose public scalar-projection digest is bound to the
  embedded IPA verifier slice. ABI-6 native append now reads the append
  request's defaulted `output_proof_circuit_id`: missing or semantic selectors
  preserve the legacy semantic `kagemusha-recursive-aggregation-v1` output, while
  explicit Reserved-lineage selectors derive the lineage verifier key from the
  hop's Pallas open-envelope archive and enter the guarded Reserved-lineage
  output path. This keeps SDK-facing D2D payloads hop-count-independent across
  offline re-spends, and witnessless multi-hop Reserved-lineage output is
  available for supported transitions inside the configured 64-hop cap.
  The same init/append preflight now runs inside record-backed lineage-witness
  assembly before the helpers merge hop envelopes or carry previous recursive
  proofs, so the redeem-side witness cannot be assembled from a request that
  recursive proving would reject.
  Bridge, JavaScript host, and Python PyO3 recursive-spend verification now
  report offline spendability separately from chain admission: a locally
  verifying semantic v1 recursive proof without a record-backed lineage witness
  returns `valid = true` for receiver-side offline acceptance, while
  `chain_admissible = false` carries the private-hop-lineage diagnostic that
  redeem would emit.
  The recursive-spend redeem bridge, JavaScript host, and Python PyO3 host now
  apply the same gate after public-binding validation: semantic v1 requests with
  a verified record-backed lineage witness and a verifying final recursive proof
  serialize instructions. Metadata-valid Reserved-lineage requests serialize
  witnessless redeem instructions inside
  `KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` when the
  transition circuit flag is wired, the active lineage verifier record matches,
  and chain/asset, root, final commitment, proof, and nullifier checks pass.
  Witnessless semantic v1 requests, Reserved-lineage requests missing or
  mismatching that record, tampered final recursive-proof requests, malformed
  Reserved-lineage requests, and over-cap Reserved-lineage requests return no
  instruction bytes.
  Ledger recursive redeem execution now checks the final unshield/redeem proof
  public binding before reserved-lineage chain-admission and backend proof
  verification, preserving the final-note mismatch diagnostic even when a
  reserved-lineage verifier-record fixture is malformed.
  Rust data-model helpers and all SDK wrappers now assemble the separate
  record-backed redeem witness alongside recursive spend `init` and `append`:
  they validate the recursive bundle public-input binding, one-hop fragments,
  exact verifier-record sets, Pallas envelope archive decoding/counts, root
  continuity, duplicate or overlapping lineage nullifiers/commitments,
  accumulator initial/final root binding, current-note/output collisions, and
  proof-attachment/backend/inline-key shape, plus inactive, missing-key,
  over-proof-cap, commitment-mismatched, key-length-mismatched,
  namespace-mismatched, backend/curve-mismatched, empty-circuit, and
  zero-schema verifier-record snapshots. They carry ordered semantic previous
  recursive proofs forward, merge the archive, and reject verifier record
  conflicts, chain/asset-spliced append fragments, stale previous bundles, and
  stale appended bundle results before the witness is attached to redeem.
  Reserved-lineage previous proofs are accepted at this helper boundary only
  when the append or redeem request carries the active lineage verifier record;
  append attempts that select the Reserved-lineage output circuit through
  `output_proof_circuit_id` also require the previous-proof opening archive that
  the production witnessless lineage circuit will consume. Unsupported output
  selectors fail at request preflight. Native append preflight now treats that
  archive as bounded Pallas IPA witness material and rejects oversized archives
  before decode, missing or zero metadata, non-Pallas curves, opening-shape
  mismatches, over-count archives, and IPA proof tampering before append proving
  starts.
  The same 8 MiB archive cap is enforced at the data-model request boundary and
  exposed by every SDK recursive-spend helper. Witnessless Reserved-lineage
  redeem serialization is enabled inside the 64-hop cap, and all SDKs now expose
  `KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` so wallets can
  branch without duplicating the chain-admission rule.
  SDKs also expose matching circuit-id/hop-count helpers
  (`canRedeem...Witnessless` / `requires...LineageWitnessForRedeem`) so wallet
  code does not duplicate the current Reserved-lineage branch. Native verify
  results now carry defaulted `witnessless_redeem_supported` and
  `lineage_witness_required_for_redeem` booleans for SDKs that only consume raw
  Norito verify-result archives.
  Witnessless Reserved-lineage redeem serialization is available inside the
  configured 64-hop cap.
  Direct redeem-request
  validation applies the same archive decode/count, transcript-shape, note-binding,
  record-snapshot, attachment-shape, and previous-proof
  semantic/hop-order guards so malformed, count-mismatched, root-spliced,
  duplicate-output, inactive-record, missing-key, backend-spliced,
  namespace/curve-spliced, empty-circuit, zero-schema, forged-lineage-record,
  scalar-spliced, or out-of-order lineage witnesses fail at the data-model
  boundary. Core record-backed replay also preflights previous proof
  backend/profile/hash/scalar/hop-order
  invariants before reconstructing Pallas
  hop evidence. Chain execution now also has adversarial coverage proving that
  missing registered lineage verifier records, stale WSV record snapshots,
  missing witness records, duplicate witness records, and unreferenced witness
  records all reject through `invalid_recursive_lineage` before final-note or
  top-up-anchor nullifiers are consumed and before public balances are minted.
  Recursive spend availability
  probes now require the complete ABI-6 native surface - init, append, both
  transition-profile helpers, append-boundary derivation, both lineage-witness
  helpers, verify, and redeem - so old native libraries cannot claim
  `recursive_spend_v1` support without the witness path and append-boundary
  surface needed for safe redemption. Python direct helper calls and the
  optional C# P/Invoke wrapper now apply the same complete-surface guard before
  producing recursive spend output, and Python/Kotlin/JVM/Java Android
  availability probes now fail closed on malformed native loading or ABI-version
  probes before symbol probing.
  JavaScript/Node, Python, Swift, Kotlin/JVM, Java Android, and C# also reject
  a native probe that accepts empty or malformed archives instead of producing
  the expected Kagemusha rejection without output bytes, and their focused tests
  now assert native recursive-spend redeem/output rejection is propagated rather
  than converted into fallback bytes.
  Swift exposes the witness helpers as
  `lineageWitnessFromInitResult` and `lineageWitnessAppendResult`; Kotlin/JVM,
  Java Android, JavaScript/Node, Python, and C# expose matching raw-archive
  wrappers on their native recursive-spend surfaces.
  Swift, Kotlin/JVM, Java Android, JavaScript,
  Python, and C# now also expose stable constants for the semantic
  v1 and reserved lineage circuit ids. Swift, JavaScript/Node, Python, and C#
  now expose the recursive-spend minimum native bridge ABI-6 requirement beside those
  constants while accepting additive ABI-7 bridge advertisements; Swift
  bridge-loader tests pin packaged artifacts to at least ABI 6, the Node NAPI
  host exports `connectNoritoBridgeAbiVersion`, and the Python PyO3 extension
  exports `kagemusha_recursive_spend_native_bridge_abi_version`. The SDK surfaces also
  expose a common preferred offline spend-mode selector: `recursive_spend_v1`
  when the ABI-6-or-later recursive spend surface is available and
  `checked_prefold_v1` as the compatibility fallback; Kotlin/JVM and Java
  Android probe the native bridge ABI version plus verify and both lineage
  witness JNI symbols, C# probes the matching P/Invoke symbols, and
  JavaScript/Node plus Python probe their native hosts before reporting
  recursive spend availability. The recursive spend data model
  now round-trips the raw ABI-6 Norito archives for init, append,
  lineage-witness assembly, verify-result, and redeem requests so SDKs share
  one archive contract. The
  recursive D2D payload benchmark records 1,553-byte fixture archives for hop
  counts 1, 2, 3, 5, 8, 13, 21, 34, 55, and 64 with a fixed 256-byte proof
  payload, pins that exact fixture length in CI, applies a 1,600-byte
  material-growth ceiling, and asserts that archive length remains
  hop-count-independent. The
  recursive spend accumulator now validates that its aggregation transcript
  digest equals its lineage digest, keeping the recursive proof public input
  attached to the spend-lineage accumulator rather than a detached digest.
  Recursive spend append validation now also rejects carried-state output
  collisions where the next spend nullifier or any new output commitment reuses
  the previous spendable commitment, or where a new output commitment reuses a
  carried top-up anchor nullifier. Chain-side recursive redeem now reaches the
  private-hop lineage admission gate before semantic recursive backend proof
  verification, so admission-neutral v1 spend proofs fail closed before verifier
  work, nullifier consumption, redeem-proof verification, or minting while still
  preserving earlier metadata and final-binding diagnostics.
  Python, Swift, Kotlin/JVM, and Java Android now expose record-backed compact-token
  prover wrappers over that ABI, so mobile wallets can pass
  `KagemushaVerifiedFoldRecordBundle` Norito bytes through the native bridge
  while Kotlin/JVM and Java Android recursive-spend evidence helpers keep
  `VerifiedFoldHopEvidence` explicit for `chain_id`, asset, and `root_after`,
  validate privacy build results, nested `OpenVerifyEnvelope`s, active
  Kagemusha verifier records, schema hashes, verifier-key commitments, strict
  ZK1 public-instance columns, and root continuity before emitting checked
  fold-record bundles or redeem proof attachments. Proof-output-only helpers
  must remain fail-closed because privacy proof outputs do not carry Pallas IPA
  opening envelopes or enough chain context to derive a production-safe record
  bundle; init/append request helpers must likewise require explicit checked
  hop evidence plus caller-supplied Pallas open-envelopes archives before
  serializing recursive-spend request archives instead of constructing
  preverified folded public inputs themselves. The same
  SDK surfaces now expose the ABI-6 recursive aggregation proof-bundle prover,
  which accepts record-backed bundle bytes plus proof-derived Pallas
  open-envelope archive bytes and returns an admission-neutral
  `KagemushaRecursiveAggregationProofBundle` for future mode-2 work.
  Swift, Kotlin/JVM, and Java Android Offline Note proof binding now also
  rejects substituted recursive verifier ids or proof backend labels before
  accepting wallet-side validation, keeping mobile checks aligned with the
  chain's `halo2/ipa:offline-note-recursive` trust anchor. Draft wallet and
  redeem-planner bundles now carry the explicit unsupported
  `offline-note/draft-placeholder` backend until a real proof provider replaces
  them.
  Offline note key certificates now fail closed when an exposed hardware
  usage-count limit is anything other than exactly `1`, so a certificate cannot
  claim one-use semantics while carrying a multi-use or zero-use platform
  counter. The Torii offline issuer enforces that rule while minting the
  online-to-offline topup certificate, and Swift, Kotlin/JVM, and Java Android
  constructors mirror it before wallet-side serialization. Their Torii
  issuer-response parsers also reject malformed, overflowed, or non-numeric
  certificate versions and counter values before narrowing JSON into SDK
  certificate models. Torii topup issuance also derives the wallet JSON
  certificate and `IssueOfflineNote` chain certificate from the same signed
  object, so the wallet's offline trust anchor is the exact certificate payload
  recorded by the online-to-offline transaction. Legacy Offline audit metadata now mirrors the Kagemusha
  nullifier/commitment separation rule by rejecting any byte-identical overlap
  between consumed input nullifiers and new output commitments before recursive
  proof decoding.
  Chain-side Kagemusha transfer admission also rejects any byte-identical
  overlap between consumed input nullifiers and newly created output commitments
  before proof decoding, keeping ledger admission aligned with the proof
  system's nullifier/commitment domain separation. The shared folded-public-input
  and Poseidon aggregation-transcript validators mirror that rule for same-hop
  and cross-hop overlap before compact-token or reserved recursive evidence is
  built.
  Current release evidence covers physical iOS App Attest/HCE/CardSession
  availability and Android StrongBox/KeyMint one-use-key validation. The open
  physical gap is the end-to-end cross-platform NFC/HCE payment
  exchange with both devices unlocked and ready; recursive aggregation of the
  private per-hop proofs into the compact folded-token mode-2 proof remains
  follow-up work for a later version, while spend-again-offline cash uses the
  recursive spend bundle and ABI-6 append path now. Native Pasta/Fp scalar
  decomposition, fixed-window scalar decomposition, fixed-window Vesta point
  selection, table derivation, and scalar-multiplication composition, and
  fixed-window multi-term MSM plus bounded native-scalar MSM, fixed-window IPA
  final-comparison MSM, IPA scalar/vector-fold, full `b`-vector reduction,
  generator-fold, round-accumulator, final-comparison composition, and one-round
  and generic multi-round verifier composition with transcript binding plus
  native Pallas verifier-witness translation, batch preflight binding,
  reserved-mode recursive aggregation evidence/proof-public-input binding, and a
  transparent Halo2 IPA semantic proof for the recursive evidence layout are
  present. Record-backed combined proof-bundle builders now derive evidence from
  active hop verifier records and Pallas witness/open-envelope material before
  proving it, but private-hop Pallas IPA witness verification inside the
  compact-token recursive circuit and mode-2 compact-token admission remain
  reserved, explicitly rejected wire values until that verifier evidence exists.
- Continue dependency, documentation, and release hygiene work required by LF
  Decentralized Trust project expectations.

**Next checkpoints:** governed deployment evidence and live canary evidence for
operator-provided rollout bundles.

## SORA Nexus and Taira

**Status:** active pre-release hardening.

- Use the public Taira testnet to harden consensus, routing, lane-aware
  execution, data availability, operator workflows, and SDK integration.
- Complete the remaining independent-lane consensus, DA/RBC, and cross-lane
  relay validation needed for the first public Nexus release.
- Keep the rotating Byzantine 30 TPS NPoS soak in the stabilization corridor:
  the snapshot-enabled strict 7,200 second 4-peer transfer run now passes under
  the broadened `conflicting-ready`, `duplicate-inits`, and
  `drop-validator-chunks` matrix with `snapshot_mode=read_write`,
  `snapshot_create_every_ms=30000`, 72 non-overlapping fault windows evenly
  split across the three fault kinds, `submit_elapsed=7200.110828917s`,
  `submitted=216000`, final `min_approved=216016`, `max_rejected=0`, final
  queue size `0`, final convergence at height `1576`,
  `load_submitted_tps=29.999538`, `load_committed_tps=29.951370`, and final
  committed TPS `29.774000`. The matching snapshot-enabled 900 second bounded
  recovery gate also passes with
  `IROHA_REALISTIC_30TPS_RECOVERY_BOUND_SECS=120`; the full gate recorded 72
  successful restart recoveries with max status wait `30691ms`, max total
  restart/status duration `39070ms`, complete snapshot bundles after every
  shutdown/status recovery, and max strict convergence wait `63648ms`. This
  resolves the earlier multi-minute full-replay startup tail; remaining
  measured latency is bounded restarted-peer catch-up/convergence while the
  healthy quorum continues committing. Replay validation still avoids
  replay-only FASTPQ witness/transcript generation without weakening committed
  result or root verification, and replay catch-up persists the query-index
  journal once per Kura replay range while logging per-phase timings. The
  remaining checkpoint for this corridor is keeping the broadened strict soak
  in regression rotation while release packaging and operator runbooks
  converge.
- Continue native AMX hardening beyond the implemented attestation data model,
  control-plane message handling, deterministic per-leg vote cache,
  proposer-side prepare/commit gating, 4-peer convergence proof,
  queue-journal restart replay, and routing-plan projection with longer-running
  soak, fault injection, and independent participant-lane finality work.
- Keep SCCP bridge submission permissionless while requiring outbound message
  records to originate from verified IVM-proved overlays, route allowlists to
  be deployment-governed, and production activation to wait for all advertised
  lanes to have cryptographic source-chain proof adapters plus immutable
  destination verifiers. ETH/BSC production targets `evm-groth16-bn254-v1`,
  TRON/TVM production targets `tron-groth16-bn254-v1`, and the secp256k1
  attestation verifier remains direct-fixture-only; web/mobile SDK proof
  request builders now reject any EVM-family or TRON backend string outside
  those canonical Groth16 backends before invoking the app-linked prover, and
  the EVM wrapper constructor now rejects any non-Groth16 backend, proof
  families other than `stark-fri-v1`, missing or mismatched verifier-key
  hashes, non-SORA sources, and non-ETH/BSC targets before any deployment can
  advertise mismatched binding metadata. Rust destination-binding helpers now
  also refuse to derive deployable EVM bindings for the reference secp256k1
  backend. Rust manifest readiness ignores a flipped `production_ready` flag
  when the manifest backend, verifier target, or proof family has been mutated
  away from the canonical production lane, and Torii proof-material routing
  plus capability discovery now use that effective readiness check. Rust
  destination rollout readiness now also rejects padded EVM addresses, Solana
  runtime entrypoints instead of trimming them into production verifier
  renderers now mirror that exact-input posture for verifier identities,
  fixed-width hashes, lane selectors, and deployment metadata before they can
  emit production TOML, and the all-lanes preflight now requires the
  helper-emitted TON live account comments to remain attached to governed
  account status, account-state, and last-transaction fields. Rust,
  JavaScript, Python, Swift, Kotlin, and Java
  Android portal/mobile helpers now derive EVM-family and TRON destination
  binding hashes from governed deployment tuples; web/Python request builders
  reject mismatched raw `destinationBindingHash` values before app-linked
  prover callbacks run, Rust request/result wrappers bind the same deployment
  object into request hashes, public signal words, and envelope hashes, and
  Swift, Kotlin, and Java Android request/submission constructors can now accept
  the same derived binding object directly, reject mismatched binding metadata,
  and thread the derived hash into request hashing or verifier-call packaging.
  Swift/Kotlin/Java Android bridge-proof submit DTOs and JavaScript/Python
  Torii submit payload helpers can also be built directly from the generated
  EVM-family/TRON submission plus that governed binding object, deriving the
  on-chain `proof_bytes_hex` and destination tuple instead of requiring UI code
  to manually copy proof material. Raw EVM-family/TRON submit preflights in
  those SDKs now also recompute destination binding hashes from the supplied
  deployment tuple before any Torii request is posted, and JavaScript/Python raw
  message submissions now require message-bundle commitment metadata before
  `proofBytesHex`/`proof_bytes_hex` can be bound and posted. The JavaScript web
  package root and `./sccp` subpath now publish the same EVM-family/TRON
  bridge-proof submit payload
  builders in `dist` plus TypeScript declarations, and the Python package root
  exports the matching helpers, so browser portals and mobile-backed user
  provers can hand generated Groth16 proof submissions to Torii without
  manually copying destination deployment fields. Those dynamic helpers plus
  the Swift/Kotlin/Java Android typed submit DTO builders now require the SCCP
  message bundle commitment root, locally re-check the BN254 Groth16 tuple, and
  bind the tuple to `message_bundle.commitment.message_id`,
  `message_bundle.commitment_root`, and the SORA source-domain word before
  returning a submit payload. Torii
  production bridge-proof submit, artifact, proof-job, and runtime proof
  envelope generation now also require SORA-origin message bundles to pass the
  BLS-backed Nexus finality verifier before packaging, and burn bridge-proof
  submission rejects structurally valid but unsigned Nexus finality proofs.
  SORA-origin message lookup now resolves locally published/cacheable bundles
  before the non-SORA proof registry, keeping runtime proof export available for
  freshly published local messages without weakening external-source proof
  admission. Core SCCP finality admission now also runs the embedded Nexus
  finality through the BLS aggregate verifier before local block/QC anchoring,
  so structural-only finality is rejected directly on-chain.
  Dynamic JavaScript and Python EVM-family/TRON destination binding helpers now
  also reject duplicate aliases for network ids, verifier addresses,
  verifier-code/key hashes, backend/proof-family selectors, binding hashes, and
  proof-context destination-binding fields before request hashing or app-linked
	  apply the same top-level guard to `publicInputs`, `bundleBytes`,
	  contexts reject duplicate nested binding-hash aliases. The dynamic
	  JavaScript and Python EVM/TRON proof-result wrappers and contract-call
	  submission builders now also reject duplicate aliases for request hashes,
	  envelope hashes, proof contexts, proof bytes, bundle/source-proof bytes,
	  public inputs, source domains, and public signal words before a web portal
	  or mobile backend packages user-generated proofs into counterparty calldata.
	  The JavaScript Groth16 result wrappers also copy and freeze request-derived
	  public inputs, public signal words, and proof contexts, so mutating a
	  manually supplied request object after wrapping cannot change what the
	  portal submits on-chain.
	  builders now apply the same guard before packaging user-generated proofs into
	  Python transparent public-input normalizers now also reject duplicate aliases
	  inside message ids, payload hashes, target domains, commitment roots,
	  finality heights, and finality block hashes before any request hash, public
	  signal word list, or submission envelope is derived. JavaScript and Python
	  ETH/EVM receipt-proof helpers now reject duplicate aliases for source
	  domains, source event digests, beacon slots, execution block/finality
	  numbers and hashes, receipt roots, beacon finalized roots, sync-committee
	  roots, receipt proof nodes, and inclusion branches, and they reject non-ETH
	  source domains before deriving ETH receipt-proof hashes. The ETH
	  sync-committee payload, transition-message, and transition-signature helpers
	  apply the same guard to committee public keys/weights/PoPs, transition
	  periods and slots, finalized roots, parent/next committee hashes, payload
	  hashes, branch hashes, transition-message hashes, signers bitmaps, aggregate
	  signatures, and nested proof weight fields before hashing. ETH beacon
	  block-header root helpers now reject duplicate slot, proposer, parent-root,
	  state-root, and body-root aliases before SSZ root derivation. JavaScript and Python
	  BSC Parlia receipt-proof, validator-set payload, validator-set
	  metadata/transition, commit-message, and commit-seal helpers now apply the
	  same guard to source domains, source event digests, validator epochs,
	  block/finality numbers and hashes, receipt roots, proof nodes, inclusion
	  branches, validator addresses/powers, validator-set storage roots, slots,
	  values, value hashes, payload hashes, metadata proof hashes, total/signed
	  power, commit-message hashes, validator keys, signers bitmaps, and
	  validator-set hash echoes before deriving BSC source proof hashes.
	  JavaScript and Python Solana message-proof, transaction-status leaf, and
	  transaction-status root helpers now reject duplicate aliases for source
	  event digests, transaction-status/receipt-message roots, transaction
	  signatures, emitter program ids, and inclusion branches before deriving
	  Solana source proof hashes.
	  Their semantic vote-account and stake-account data canonicalizers now
	  reject duplicate aliases for node/voter/withdrawer keys, collector and
	  commission fields, Tower vote slots, delegated stake,
	  activation/deactivation epochs, warmup/cooldown bytes, credit counters,
	  and stake flags before deriving AccountsLtHash account-data inputs.
	  Their epoch-stake, stake-activation, stake-account-state, StakeHistory,
	  and StakeHistory-sysvar transcript helpers apply the same guard to
	  epoch/slot fields, validator account address/hash vectors,
	  delegated-stake vectors, and StakeHistory vectors before deriving Solana
	  finality/source-state hashes.
	  Their Solana active-stake, stake-activation, and stake-history helpers also reject
	  duplicate aliases for validator public-key rosters, validator stake
	  weights, activation epochs, and deactivation epochs before deriving Solana
	  finality/source-state transcripts.
	  Their Solana account-opening, AccountsLtHash opening-normalization, and
	  account-inclusion leaf helpers now apply the same alias guard to account
	  addresses, owner program ids, rent epochs, account-data hashes, finalized
	  slots, opening objects, raw account data, raw-data hashes, and nested
	  opening addresses; if both raw account data and a raw-data hash are
	  supplied, JavaScript and Python recompute and require equality before
	  deriving the account-inclusion transcript.
	  Their opened-AccountsLtHash contribution, opened-account inclusion witness,
	  and Agave bank-hash helpers now reject duplicate aliases for opened
	  vote/stake arrays, StakeHistory sysvar fields, account-inclusion roots,
	  AccountsLtHash checksum/root fields, full AccountsLtHash bytes, parent bank
	  hashes, bank signature counts, blockhash bytes, and optional hard-fork hash
	  data before deriving Solana residual, branch, or bank-state transcripts.
	  The lower-level Solana Tower lockout/replay, bank-fork, and AccountsLtHash
	  recursive public-input helpers now reject duplicate aliases for finalized
	  slots, epochs, rooted/parent slots, parent-bank hashes, bank hashes,
	  bank-fork hashes, Tower vote slots, transaction-status roots,
	  account-inclusion roots, AccountsLtHash checksum/root fields, full
	  AccountsLtHash bytes, and hard-fork data before hashing.
	  Their direct v1 Solana finality-context canonicalizers now apply the same
	  strict alias guard to portal-supplied context objects before hashing,
	  covering Tower vote slots, parent-bank hashes, bank signature counts,
	  optional hard-fork data, AccountsLtHash roots/checksums, stake roots, and
	  Tower replay/bank-fork transcript hashes.
	  JavaScript and Python TRON receipt, receipt-state, and transaction-source
	  proof helpers now reject duplicate aliases for source event digests,
	  receipt/message roots, transaction roots, transaction indexes/counts/bytes,
	  transaction Merkle branches, receipt-MPT proof nodes, optional expected
	  bridge emitter/owner addresses, and inclusion branches before deriving TRON
	  source proof hashes.
	  Their TRON raw block-header, solid-block header proof, solid-block message,
	  witness-schedule payload, witness-seal, and witness-schedule transition
	  helpers apply the same guard to block ids, raw-data hashes, header
	  roots/signatures, witness rosters/weights, signers bitmaps, transition
	  epochs, transition block hashes, schedule hashes/payload hashes, nested
	  seal proofs, and transition message hashes before deriving TRON
	  source-finality evidence.
	  authority-set payload, authority transition, finality justification, and
	  transition-justification helpers now reject duplicate aliases for source
	  domains, source event indexes, finalized block fields, finality set ids,
	  storage roots, authority rosters/weights, payload hashes, transition
	  hashes, signers bitmaps, nested verifier material, and runtime storage
	  material.
  JavaScript, Python, Kotlin, and Java Android prover callbacks now pass defensive
  request snapshots into app-linked proof engines, and the Kotlin/JVM plus Java
  Android final-proof regressions now pin actual snapshot delivery for Solana,
	  engines plus the Solana source-state proof engines. The Java Android
	  Ethereum mainnet outbound wrapper now shares the same EVM callback-request
	  snapshot path before invoking app-linked proof engines. Kotlin Solana final-proof
	  witness objects now also defensively copy AccountsLtHash, bank hard-fork data,
	  and inclusion-branch byte buffers on construction and access, so a mobile UI
	  prover cannot mutate request witness bytes while proof-result wrapping still
	  uses the original canonical request. The Kotlin Solana prover also snapshots
	  raw witness input before app-controlled witness-provider resolution, preventing
	  resolver-side mutation of caller-owned AccountsLtHash or inclusion-branch
	  buffers before the canonical proof request is built; Java Android Solana
	  now also passes a distinct defensive `WitnessInput` snapshot to witness
	  providers. Kotlin/JVM and Java Android EVM-family, TON, TRON, and
	  into app-controlled witness providers before canonical request construction.
	  witness-provider calls through explicit input snapshot helpers as well.
	  JavaScript and Python portal facades now apply the same mutable
	  deep-snapshot boundary before invoking witness providers, so resolver-side
	  edits to nested public-input objects or byte buffers cannot alter
	  caller-owned UI state.
	  Swift, Kotlin, and Java Android Solana source-state facades also validate
	  canonical AccountsLtHash and role-separated full-light audit requests before
	  invoking app-linked proof callbacks, so malformed OpenVerify/FastPQ transcript
	  bytes cannot reach mobile proof engines and then be rejected only after proof
	  generation. JavaScript, Python, Swift, Kotlin, and Java Android direct
	  request builders plus source-state wrappers now also reject Solana
	  full-light audit requests whose role verifier hash reuses the request-bound
	  source-state, material, deployment, gate, finality, vote-message, nested
	  AccountsLtHash proof, or audit-statement hashes before any UI-generated
	  proof bytes are requested or wrapped.
	  Swift, Kotlin, and Java Android TON source-state facades now apply the same
	  pre-callback request validation to direct shard-state and role-separated
	  full-light audit requests, so tampered TON FastPQ metadata, statement bytes,
	  public-input columns, or verifier material cannot reach user-facing mobile
	  proof engines. The Swift TON facade also passes copied callback snapshots
	  into linked final-proof, shard-state, and audit proof engines, matching the
	  hardened Solana and JVM/Android callback surfaces before wrapping
	  UI-generated proof bytes against the original canonical request.
	  Web/Python source-state and destination proof-result wrappers now reject
	  padded or mismatched `proofBase64`, `proofFamily`, circuit id, request echo,
  structured public-input, proof-context, and Groth16 public-signal metadata
  before wrapping UI-generated proofs; duplicate camelCase/snake_case result
  aliases for those fields fail instead of letting one value be displayed while
  another is submitted. JavaScript and Python source-state result metadata now
  compares normalized numeric slots, role codes, canonical hex hashes, and
  Solana audit-role aliases against the request while still rejecting padded
  plain string metadata. Their canonical Solana/TON source-state proof capsule
  parsers now apply the same duplicate-alias rule to proof version, proof
  family, circuit id, proof bytes, and proof base64 before hashing source proof
  capsules. JavaScript and Python Solana final proof-result and submission
  builders now also apply that alias guard to wrapped proof bytes,
  proof-context/envelope/deployment hashes, source-state verifier echoes, and
  nested source-proof public-input fields before deriving wallet/RPC packages.
  JavaScript and Python Solana final proof-request builders now reject duplicate
  witness and nested proof-context aliases before request hashing or app-linked
  prover invocation, covering slots, bank-state hashes, blockhash spellings,
  message ids, deployment material, source-state verifier metadata,
  AccountsLtHash fields, and inclusion branches. Their Solana AccountsLtHash
  source-state and role-separated full-light audit request builders now apply
  the same duplicate-alias guard before deriving FastPQ statement/context/schema
  bytes, vote-message hashes, finality-context fields, source
  material/deployment selectors, or full-light gate/material/deployment hash
  echoes for browser and portal-backend provers.
  The Rust `iroha_sccp` finalized-vote verifier regression now also rejects a
  re-signed Solana finality context whose
  `accounts_lt_hash_proof_public_inputs_hash` no longer matches the recomputed
  AccountsLtHash public-input transcript.
  Their generic source-verifier material and source-adapter deployment
  normalizers now also reject duplicate aliases across source-domain,
  verifier-hash, source-bridge, target-domain, adapter verifier-key, Solana/TON
  audit-role, and deployment-receipt fields before deriving governed material
  or deployment record hashes, and explicit null audit-role hashes now fail
  instead of being treated as omitted zero-hash fields.
  JavaScript, Python, Swift, Kotlin, and Java Android source-material helpers
  now also reject non-zero lane-inapplicable source-state, bridge-emitter, and
  bridge-config fields before hashing, while still accepting canonical zero or
  empty fields emitted by normalized records.
  JavaScript and Python TON submission builders apply the same guard while
  packaging wrapped proof results into wallet/liteserver message-body BOCs,
  including top-level `proofResult`, proof bytes, request/envelope/deployment
  hashes, verifier echoes, proof context, bundle/source proof bytes,
  statement/destination hashes, metadata bytes, and `queryId`. Those builders
  now require the wrapped UI prover result before BOC construction, and the
  Swift/iOS, Kotlin/JVM, and Java Android message-body input types mirror that
  requirement instead of exposing standalone raw proof-byte submission
  constructors. Raw native-recursive proof bytes can no longer bypass request,
  verifier, or source-adapter deployment binding checks in web/backend or
  mobile wallet packaging. Their TON
  submission metadata canonicalizers now reject duplicate aliases across
  manifest fields, destination binding hashes, public inputs, and statement
  hashes before metadata bytes are hashed into those BOCs.
  authority-transition builders also bind signer bitmaps to exact signer
  counts, signed and total weights, and a strict `> 2/3` quorum before hashing
  UI witness material, while web/mobile TON validator-signature transcripts
  bind validator public keys, signer bitmaps, total/signed weights, strict
  `> 2/3` quorum, and non-zero 64-byte signatures across the dedicated TON
  prover path plus the shared Kotlin/Java Android source-proof facade. Python
  and JavaScript TON shard-state request builders now also recompute nested
  validator-set `transitionSignatureHash` values before the transition chain is
  hashed, matching the Swift/Kotlin/Java Android TON prover path. TON
  source-state proof wrappers across web/mobile now recompute statement-byte
  hashes plus FastPQ `dsid`/`txSetHash` before accepting user-prover output, so
  callback metadata cannot drift from the request bytes sent to the prover.
  JavaScript and Python TON shard-state source proof input normalizers now
  reject duplicate aliases across masterchain/shard coordinates, BoC proof
  openings, config-proof BoC sources, verifier material, and finality metadata
  before deriving FastPQ public inputs or UI prover request hashes.
  Their raw TON shard proof transcript builders now apply the same duplicate
  alias rule to source-event digests, masterchain/finality aliases, shard
  transaction fields, dictionary openings, and inclusion branches before
  hashing branch witness material.
  Their TON full-light audit request builders also reject duplicate
  source-verifier material, source-adapter deployment, flattened/nested
  masterchain-config witness, shard-state public-input, and verification-proof
  hash aliases before deriving role-separated audit statements, OpenVerify
  columns, FastPQ metadata, or prover requests.
  Their TON validator-set, masterchain config, block-message,
  validator-signature, transition-message, and transition-signature transcript
  builders now also reject duplicate aliases across validator rosters, weights,
  signer bitmaps, quorum weights, config proof fields, masterchain/shard block
  coordinates, and validator-set transition payload hashes before hashing
  trust-anchor witness material.
  Python and JavaScript TON source-state wrappers also reject duplicate
  camelCase/snake_case request aliases, including nested FastPQ aliases, so UI
  proof requests cannot display one field spelling while hashing another. The
  same web/Python TON final proof-result wrappers now reject duplicate result
  aliases and recheck optional public-input, proof-context, statement,
  destination-binding, source-state verifier, and deployment-binding echoes
  before wrapping recursive proof bytes for submission. Web/Python TON final
  proof-request builders now apply the same alias guard before request-hash
  derivation and require direct/nested proof-context destination binding hashes
  plus top-level/nested source-adapter deployment hashes to agree. Swift,
  Kotlin/JVM, and Java Android TON proof-request inputs now accept the typed
  source-adapter deployment binding directly and enforce TON -> SORA binding
  domains before hashing mobile prover requests. Swift,
  Kotlin, and Java Android BSC ValidatorSet metadata builders now reject
  omitted or oversized account/storage MPT proof vectors and non-20-byte
  ValidatorSet contract addresses before hashing UI-submitted transition
  metadata; Python, JavaScript, Swift, Kotlin, and Java Android also recompute
  BSC storage-value hashes from the opened storage bytes so displayed metadata
  cannot diverge from the values submitted on-chain. Rust commit-seal transcript
  hashing now uses the same BSC validator-set, signer-bitmap, recovered-address,
  total/signed-power, and strict `> 2/3` quorum checks as the verifier; Python,
  JavaScript plus `dist`, Swift, Kotlin, and Java Android expose matching
  BSC commit-message and commit-seal helpers so portal and mobile UI provers
  derive the seal hash locally before submitting source proofs on-chain.
  The JavaScript TypeScript declarations now publish a shared
  `SccpDomainIdInput` for SCCP source, target, local, and counterparty domain
  request fields plus `SccpVersionInput` for v1-only proof/request inputs,
  including source-state prover `proofVersion` aliases. Package declaration
  tests pin those domain/version signatures plus the BSC commit-message and
  commit-seal inputs, including camelCase/snake_case aliases and canonical
  decimal string/bigint numeric forms, so TypeScript portal code can call the
  same proof-generation helpers that runtime validation accepts.
  Rust core plus web/mobile ETH/BSC sync-committee builders now require exact
  BLS public-key, proof-of-possession, and aggregate-signature widths, reject
  all-zero committee or aggregate material, bind signer bitmaps and padding to
  the committee size, bind total/signed weights, and require strict `> 2/3`
  quorum before hashing UI witness material or deriving aggregate transcript
  hashes.
  The JavaScript package root now re-exports the same helpers as the `./sccp`
  subpath, so
  published portal bundles can use the normal package entrypoint. Swift,
  Kotlin, and Java Android now also use the same Rust/Python/JavaScript
  destination-binding key format for EVM/TRON rollout metadata, with the
  network-id segment rendered as raw lowercase bytes32 hex while normalized
  returned `networkId` fields keep their `0x` prefix. TRON Rust destination
  binding, Swift Torii raw-submit, and operator evidence helpers now reject
  whitespace-padded Base58Check verifier addresses instead of normalizing
  them into governed SORA -> TRON binding metadata or UI prover request
  hashes. The reference
  TRON source-bridge evidence helper now rejects embedded whitespace inside
  inline or file-backed runtime bytecode, fixed-width deployment hashes, and
  hex-form TRON addresses, plus uppercase hex or `0X` aliases for those exact
  fields, before Python `bytes.fromhex()` can normalize the value into shorter
  material, keeping direct TOML generation exact. TRON live source-event
  readback now applies the same exact lowercase hex policy to log addresses,
  empty log data, and visible `TriggerSmartContract` calldata, rejects uppercase
  or `0X` aliases for exact transaction, signature, block-header, and
  route-canary hex, plus generic result-extension hex used while reconstructing
  unrelated block transactions, before transaction evidence can be accepted. The reference
  secp256k1 verifier now
	  also rejects non-canonical attestation ABI, zero native-proof hashes, zero
	  statement/public-input fields, and mismatched message/commitment public
  inputs before signature recovery. Direct/live ETH/BSC source and destination
  rollout TOML now carry replayable source-bridge, bridge-wrapper, and
  verifier runtime bytecode plus the canonical EVM backend and proof-family
  hash comments, and the all-lanes preflight rejects missing or drifting values
  before activation. EVM-family live JSON-RPC reads now require lowercase
  `0x` data and shortest-form quantities before runtime bytecode, route canary
  logs, transaction calldata, or deployment receipts can feed proof evidence;
  source and destination live reads also bound successful response and HTTP
  error bodies and reject duplicate JSON keys before decoding.
  Direct ETH/BSC source receipt metadata now also requires exact positive
  integer deployment block numbers, so boolean placeholders cannot make
  mined-receipt evidence look production-ready. Direct ETH/BSC
  source evidence helpers now also require exact `u32` source/target domain ids
  before verifier-key, material, or deployment hashes are derived, so
  `target_domain = False` cannot stage SORA-bound source evidence. They also
  reject padded bridge addresses, component hashes, domains, and deployment
  block numbers before deriving source material or deployment-record hashes.
  same exact-input posture to fixed-width component hashes, source/target
  material, source-adapter deployment records, or full-light-client gate hashes.
  The
  Source, destination, live, and all-lanes evidence helpers now also require
  canonical ASCII decimal text for source-domain fields, EVM live/source-live
  RPC chain ids, deployment block numbers, audited ProgramData slots, TON
  workchain ids and last-transaction logical times, runtime version fields,
  and fallback all-lanes TOML integers, so non-ASCII digits, leading-zero
  values, hex forms, or signed forms cannot drift from reviewed operator
  evidence. EVM live and source-live collectors now also reject whitespace-padded
  JSON-RPC quantities and hex byte strings before rendering runtime bytecode,
  finalized runtime `:code` hex before rendering production TOML metadata.
  non-canonical base64 pad-bit aliases for verifier program bytes, Solana
  JSON-RPC account data, ProgramData metadata, TON code BoCs, and finalized
  runtime code before TOML rendering or all-lanes preflight can normalize
  copied evidence. TON code BoC text files now also reject internal whitespace
  instead of joining it into deployable code evidence, and the TON live
  collector returns accepted remote code BoCs as canonical standard base64.
  collectors now bound successful HTTP response bodies and HTTP error details
  before decoding, and reject duplicate keys in remote JSON objects so live
  evidence cannot depend on last-value-wins parsing. TON and TRON runtime API
  keys must be exact non-empty ASCII tokens without whitespace or control
  characters; file-backed keys may only carry terminal newlines.
  The all-lanes activation preflight now also rejects padded fixed-width
  structured hashes, hash comments, route allowlist hashes, route canary
  hashes, and non-canonical uppercase EVM runtime-bytecode preimages plus
  duplicate known metadata comments before final production readiness can be
  reported. The direct ETH/BSC source and EVM destination offline renderers
  now apply the same lowercase `0x`/lowercase-hex policy before emitting
  production TOML, and the EVM-family source live collector applies that rule
  to operator-supplied hash pins before rendering source TOML, so CLI input
  cannot be normalized after review; chain-specific metadata comment aliases
  that map to the same internal field also fail instead of overwriting earlier
  reviewed values. Strict release-bundle verification also keeps complete
  cryptographic-evidence row checks scoped to the active Ethereum launch lane
  while retaining future-lane rows as diagnostic evidence until their launch
  policies open, and now inventories the public bridge-proof launch-policy
  documentation so stale BSC-active wording cannot ship in release
  attachments. When
  both real
  `route_canary_*` config fields and imported canary metadata comments are
  present, the all-lanes gate now requires exact agreement so a direct
  `passed` value cannot override contradictory imported evidence.
  boolean readiness from the canonical destination summaries, so truthy strings
  direct caller-supplied live metadata before deriving destination args, so
  forged account status, BoC hash-match flags, runtime-code metadata, verifier
  entrypoints, or hash algorithm labels fail before TOML readiness. EVM live
  destination TOML rendering now also recomputes imported summary bytecode
  hashes, backend/proof-family hashes, binding hashes/keys, domain metadata,
  canonical RPC chain ids, and expected-pin metadata before rendering. EVM
  source-live TOML rendering now also recomputes imported source bridge
  bytecode hashes, receipt deployment metadata, source record hashes, canonical
  ETH/BSC RPC chain ids, and expected-pin metadata before rendering. The
  JavaScript proof-request and payload/submission helper runtime rejects
  boolean domain values instead of
  coercing them into SORA/ETH domain ids, and its shared SCCP hex parser now
  rejects surrounding whitespace before TRON proof-request hashes,
  source-event digests, destination binding hashes, or proof transcript fields
  are normalized. Swift TRON proof helpers now mirror that exact fixed-width
  hex policy before request hashes, public signal words, proof envelopes, or
  verifier-call calldata are derived. Python SDK shared hex parsing now applies
  the same exact inline policy before TRON proof-request payload/statement
  hashes, receipt/source-event transcripts, destination binding material, and
  raw-header fields are normalized. Python Torii typed artifact, proof-job,
  bridge-proof, and bridge-message helpers now also reject padded TRON
  Base58Check verifier addresses, padded normalized TRON codec payloads, and
  surrounding/internal whitespace in deployment or proof hex before network I/O.
  JavaScript, Swift, Kotlin, and Java Android Torii client preflights now match
  that exact-input rule for TRON verifier addresses and deployment/proof hex
  while preserving byte-array proof inputs. Kotlin and Java Android TRON mobile
  prover request builders now also reject padded fixed-width payload and
  statement hashes before proof requests or envelopes are derived.
  JavaScript and Python SDK domain normalizers now also reject non-canonical
  string/number spellings such as `05`, `0x5`, `+5`, whitespace-padded text, or
  floats before any SCCP request or transcript hash can bind the lane id, and
  the shared dynamic unsigned-integer normalizers and Kotlin/Java Android
  string-based SCCP source-hash helpers plus TRON mobile prover request
  builders now apply the same exact integer/canonical-decimal policy before
  block numbers, slots, weights, indexes, finality heights, or proof public
  inputs are hashed. Swift, Kotlin, and Java Android source-proof helpers now also
  reject padded fixed-width TRON source-event and raw-header hex before
  transcript hashing. TRON live source-event readback now also rejects padded
  transaction ids, raw transaction bytes, signatures, event log addresses,
  topics, block transaction ids, trigger calldata, constant-call ABI words,
  `/wallet/getcontract` runtime bytecode, and internal whitespace inside hex
  payloads, and it applies the same exactness to source-event transaction
  `raw_data_hex` and source-proof `Result` bytes plus live block-header
  `blockID`, `txTrieRoot`, `parentHash`, `witness_address`,
  `witness_signature`, and `accountStateRoot` fields plus non-canonical high-S
  recoverable secp256k1 signatures before a transaction or deployment readback
  can be treated as replayable source-event evidence. Saved route-canary
  full-TOML replay now also reparses the carried `raw_data_hex` and raw
  recoverable signature, requiring the canonical low-S form before accepting
  owner, selector, proof-header, and signature-recovery metadata.
  Operator-supplied TRON source-event proof inputs now also reject padded
  digests, receipt roots, inclusion branches, witness schedule payloads,
  witness-seal bitmaps, non-canonical witness-seal signatures, and expected
  proof hashes before live proof material is summarized. All-lanes
  TRON ingestion preserves that exactness, including lowercase fixed-width hex,
  for live metadata comments and structured source/destination hashes before
  recomputing source material, destination bindings, or route readiness. Direct EVM-family
  source/destination evidence helpers now also reject padded inline runtime
  bytecode arguments before deriving deployment code hashes, while runtime
  bytecode files remain tolerant of ordinary file whitespace such as newlines.
  JavaScript, Python, Swift, Kotlin, and Java Android proof-request builders
  also reject non-empty all-zero optional source proof bytes before request
  hashing while preserving absent source proofs through diagnostic request
  hashes, app-linked prover calls, proof-result wrappers, and EVM/TRON/TON
  submission constructors, so portal/mobile proof UIs can submit externally
	  generated proof packages without fabricated source-chain witness bytes.
		  The JavaScript TON portal request builder now presence-checks
		  cannot silently become an omitted source proof before request hashing, and
		  TON submission metadata bytes use the same presence check before BOC
		  packaging. The JavaScript TON and Solana submission builders now also
		  reject explicit non-object nested proof contexts, and the TON request
		  builder rejects explicit non-object source-adapter deployment bindings
		  before hashing or BOC packaging.
	  Python TON submission packaging now mirrors that presence-aware treatment
	  for proof-result statement hashes, destination-binding hashes, proof
	  context, and local BOC cell serialization, rejecting explicit falsey
	  values instead of falling through to defaults, nested proof context, or
	  empty cell fields.
	  EVM/TRON destination-binding helpers now apply the same rule to
	  backend/proof-family/context inputs, and Solana/TON source-state verifier
	  defaults plus Solana genesis defaults no longer mask explicit falsey UI
	  values before request or witness hashing.
	  Python Groth16 public-signal derivation, Solana blockhash/witness context,
	  TON proof-context/deployment-binding, and TON submission metadata parsing
	  now also reject explicit falsey nested inputs rather than replacing them
	  with adjacent top-level fields or defaults.
	  Python Solana submission packaging now treats explicit empty proof/context
	  overrides as invalid input instead of falling back to wrapped proof-result
  fields, and Python source-state proof capsule/deployment normalization no
  longer promotes explicit zero versions or empty proof-family fields to the
  production defaults. Python SCCP transcript builders now also reject explicit
  zero versions instead of silently promoting them to production `v1`, and the
  JavaScript web SDK now applies the same v1-only preflight before deriving
  source transcript bytes for portal-generated proofs. Swift, Kotlin, and Java
  Android now mirror that first-release policy for public source-state proof
  mobile prover UIs. JavaScript, Python, and Java Android Solana source-state
  proof capsules now also reject explicit null proof-version/proof-family
  metadata instead of promoting it to production defaults. JavaScript and
  Python source-state capsule normalizers also reject supplied `proofBase64` /
  `proof_base64` text unless it matches the proof bytes before canonicalization
  or AccountsLtHash proof hashing, binding UI-visible proof text to the bytes
  submitted on-chain. JavaScript, Python, Swift, Kotlin, and Java Android
  Solana source-state proof capsules now also mirror Rust's 2 MiB source-state
  proof cap plus the 128-byte proof-family/circuit-id label cap before wrapping
  or canonical hashing, and the dynamic web/Python normalizers apply the byte
  cap before base64 comparison so oversized UI prover output fails without
  extra display encoding. Python TON source-state direct wrapping and linked
  prover callbacks now also route raw proof bytes through the same source-state
  proof-byte cap before a shard-state or full-light audit proof capsule can be
  emitted, with release inventory pinning the implementation and adversarial
  oversized-proof regression. Kotlin/JVM and Java Android TON source-state
  wrappers now expose the same explicit source-state cap and pin direct-wrapper
  plus linked-prover oversized proof regressions in release inventory. Swift
  TON source-state canonicalization and wrappers now use the same explicit
  source-state proof-byte cap, with direct-wrapper and callback oversized-proof
  regressions pinned alongside the other native SDK markers. JavaScript TON
  source-state wrapping now uses the same cap for direct proof bytes and raw
  linked-prover callback bytes, with release inventory pinning the source/dist
  wrapper block plus the oversized direct-wrapper and callback regressions. The
  published JavaScript package-dist entrypoint now has its own TON source-state
  cap regression that builds a real shard-state request and rejects oversized
  direct wrapper and callback proof bytes through `dist/index.js`.
  The JavaScript package-root export suite now exercises the same cap through
  the package root wrapper and `TonSccpSourceStateProver`, so package-root
  evidence cannot be satisfied by symbol presence alone. Both package surfaces
  also reject TON source-state capsules carrying `debug-proof-family`, so the
  published root and `./sccp` entrypoints keep the same `stark-fri-v1`
  proof-family gate as the source canonicalizer. The Python package-root TON
  source-state regression now also rejects `debug-proof-family` through
  `iroha_torii_client`, so root-import evidence covers the same proof-family
  gate as the deep SCCP module.
  JavaScript, Python, Swift, Kotlin, and Java Android
  Solana source-state wrappers now recompute the AccountsLtHash public-input
  hash or full-light audit statement hash from `statementBytes` and require
  FastPQ `dsid`/`txSetHash` to derive from that canonical statement before
  wrapping UI-generated proof bytes. JavaScript and Python Solana source-state
  request wrappers now also reject duplicate top-level, nested FastPQ
  public-input, and FastPQ transition aliases before proof wrapping, so portal
  displays cannot drift from the fields hashed into the source-state transcript.
  Swift, Kotlin, and Java Android AccountsLtHash and full-light audit request
  builders now also reject explicit witness/opened `accountsLtHash` mismatches
  and normalize absent witness values from the opened full-bank hash before
  canonical request construction. JavaScript and Python AccountsLtHash and
  full-light audit request builders now derive opened contribution/residual
  hashes from canonical normalized full-bank fields before request hashing, so
  supported alias spelling cannot change the portal/backend hash boundary while
  duplicate aliases remain rejected. Their Solana full-light audit builders now
  also require the completed nested AccountsLtHash proof capsule and recompute
  `accountsLtHashProofHash` from it, rejecting proof-hash-only second-stage
  request construction. The JavaScript TypeScript declarations now model that
  contract with a required `accountsLtHashProof`/`accounts_lt_hash_proof`
  alias union and keep `accountsLtHashProofHash` as an optional consistency
  echo instead of an alternate input.
  JavaScript and Python
  app-linked Solana source-state prover result parsers use the same ordering
  and require returned scalar request echoes, audit roles, structured
  public-input/FastPQ metadata, and proof-family/circuit-id metadata to be
  exact, unpadded request matches before checking optional returned base64
  metadata, and they reject duplicate camelCase/snake_case proof-byte or
  proof-base64 aliases in dynamic result maps. Rust native recursive proof
  packaging and transparent-proof structure checks now also cap
  proof-result/submission wrappers mirror that bound before deriving envelope
  hashes, accepting app-linked prover output, or packaging wallet/RPC payloads.
  Their default destination rollout blockers now track only missing live native
  verifier deployment and trust-anchor evidence, not stale relayer-wiring
  blockers for the already-modeled program instruction, TON internal-message,
  JavaScript and Python EVM-family, TRON, and
  check to optional returned `proofBase64` / `proof_base64` metadata before
  proof-result wrapping, so a browser proof UI or portal backend cannot display
  or forward stale base64 while submitting different proof bytes. Swift, Kotlin,
  and Java Android source-state capsule surfaces derive base64 from defensive
  proof-byte copies rather than accepting
  caller-supplied aliases for Solana and TON capsules, and tests now pin that
  returned byte views cannot mutate the capsule or derived base64. JavaScript
  and Python now apply the same
  omitted-vs-explicit-null distinction to public UI
  transcript version fields, require wrapped Solana proof-result
  context/deployment versions during submission packaging, and reject explicit
  null source-adapter `adapterProofFamily` metadata. Those web/Python proof
  request and deployment normalizers now also reject explicit null source or
  target domains instead of promoting them to lane defaults. The JavaScript
  TypeScript submission declarations now mirror runtime Solana packaging by
  requiring exactly one wrapped proof-result alias (`proofResult` or
  `proof_result`) for SORA -> Solana on-chain submissions.
  Dynamic JavaScript and Python submission builders for Solana, TON,
  EVM-family, and TRON now also keep omitted fields distinct from explicit
  null proof, public-input, proof-context, statement, destination-binding,
  proof-context-hash, public-signal, and bundle overrides before wallet
  instruction, BOC, or verifier calldata packaging. The Rust TON
  internal-message builder now applies the same submit-ready context gate
  directly, rejecting non-TON public inputs, non-`ton-contract-v1` manifests,
  mismatched destination bindings, zero statement hashes, and empty bundle bytes
  before a wallet BOC can be emitted. Rust now also exposes the same
  UI-prover TON proof request/result wrapper path as the web, Python, and
  mobile SDKs, binding request hashes, envelope hashes, source-state verifier
  material, and governed TON -> SORA source-adapter deployment bindings before
  proof-result-based BOC/submission packaging. A Rust golden vector now pins
  the TON public-input bytes, deployment-binding hash, request hash, and
  envelope hash against the Python SDK implementation, and matching
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android SDK tests now pin
  that same vector so UI-prover transcript hashing cannot silently drift.
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android now also expose
  the canonical TON live-account route-canary evidence bytes/hash used by Rust
  and operator evidence scripts, giving web portals, relay backends, and mobile
  apps the same pre-submit rollout transcript checks before user-generated
  proofs are sent on-chain.
  optional returned transparent public inputs, proof context, statement hash,
  and destination-binding hash before wrapping proof bytes. JavaScript, Python,
  Swift, Kotlin, and Java Android production proof wrappers preserve omitted
  source proof bytes through app-linked prover output and submission packaging,
  while rejecting non-empty all-zero placeholders.
	  JavaScript and Python local-prover facades now also accept plain async
	  witness-provider functions and `resolve_witness` objects in addition to
	  `resolveWitness`, and tests pin that browser/backend relay providers resolve
	  snapshots before linked prover callbacks receive canonical requests; package
	  declaration tests pin the same witness-provider hooks for TypeScript portal
	  consumers. JavaScript now rejects duplicate hook aliases
	  (`witnessProvider`/`witness_provider`, `resolveWitness`/`resolve_witness`,
	  and `prove`/`proveFn`/`prove_fn`) before request construction, and the
	  TypeScript declarations model those hooks as exactly-one alias unions.
	  Python portal-backend witness-provider objects now reject duplicate
	  `resolve_witness`/`resolveWitness` methods before request construction too.
	  Swift/iOS, Kotlin/JVM, and Java Android prover tests pin the same UI-owned
	  ordering contract for mobile proof wrappers, with Kotlin/JVM and Java Android
	  also mutating provider-visible byte snapshots to prove caller-owned bundle
	  arrays remain unchanged.
  Public Kotlin/Java proof-result wrappers recheck TON/EVM/TRON backend ids
  before packaging proof bytes. Swift/iOS, Kotlin/JVM, and Java Android now
  mirror the Rust/JavaScript/Python BN254 G1/G2 curve and G2 prime-order
  subgroup preflight for EVM-family and TRON Groth16 proof bytes before mobile
  proof wrappers package UI-generated output. Java Android EVM-family and TRON
  proof-result records now also snapshot and freeze public-signal word lists,
  matching Kotlin's immutable mobile wrapper behavior so caller-side list
  mutation cannot change a wrapped UI proof package after construction. Python,
  JavaScript, Swift, Kotlin, and Java Android TRON
  transaction-source proof helpers now also recover the signer from
  `sha256(raw_data)` and require it to equal the source-call owner address
  before deriving source-proof bytes, matching Rust/core admission and keeping
  wrong-key-but-canonical source-call signatures out of portal/mobile
  transcripts. Python, JavaScript, Swift, Kotlin, and Java Android now also
  expose the TRON v3 transaction route-canary transcript helper, with shared
  vectors for the governed destination binding, route allowlist hash,
  source-message public inputs, block metadata, and recovered-owner evidence.
  Route allowlist activation now also consumes
  real `route_canary_*` config fields, and the runtime/ZK policy hash bind the
  post-deploy route canary evidence to the canonical route allowlist hash and
  destination binding hash before a lane can become production-ready. Core
  readiness also rejects route canary evidence that reuses the governed source
  material record hash or source-adapter deployment record hash, and all-lanes
  preflight rejects EVM/TRON transaction canary fields plus TON live-account
  canary hashes when they alias governed source, deployment, route, or
  destination hash roles. The
  EVM-family operator helpers and all-lanes preflight now require route canary
  evidence to be derived from a successful `MessageProofAccepted` transaction:
  the receipt log, receipt block number/hash/`receiptsRoot`, submitted
  `submitSccpMessageProof` calldata, 384-byte proof tuple header, deployed
  binding/backend/family/network tuple, and `usedMessageProofs(messageId)`
  state must all agree before the ETH/BSC route canary hash is accepted. The
  canonical EVM canary transcript now uses the `v3` evidence label and commits
  proof ABI version `1`, the SORA proof source-domain word, the ETH/BSC
  target-domain word, and the receipt block tuple, preventing proof-version,
  stale-receipt-block, or EVM-family lane replay. The direct renderer, public
  hash helper, runtime config gate, and all-lanes preflight now also reject
  reuse across distinct EVM canary
  transcript hash roles, including transaction hash, calldata, message id,
  payload, statement, commitment, and finality block fields.
  Rust/Core/Torii configured readiness now carries the
  same `evm_route_canary_*` transcript fields, recomputes the EVM canary hash
  from configured ETH/BSC rollout material, and rejects generic EVM canary
  hashes before all-lanes launch; direct EVM TOML now emits those runtime config
  fields instead of leaving them only as comments. TRON route-canary readiness
  now also requires the transaction owner address, signature SHA-256, recovered
  TRON address, and positive owner-recovery flag from the verified
  TriggerSmartContract transaction before configured launch can pass. The
  SCCP source proof envelope now binds non-SORA
  source messages to source/target domains,
  proof plan, finality model, message id, payload hash, commitment root, and
  source-event digest before public inputs are derived. Source consensus proof
  material now also carries a plan-specific adapter proof variant for
  blobs or stale witness substitutions cannot masquerade as another chain's
  proof shape. Each adapter statement is now additionally wrapped in a
  FastPQ/OpenVerify proof capsule so adapter metadata, public inputs, and proof
  public IO are cryptographically bound before lane readiness can be considered.
  Shared STARK/OpenVerify decoding now requires canonical Norito bytes for the
  outer envelope, nested STARK wrapper, and backend FastPQ proof, rejecting
  alternate compressed framings before metadata is trusted.
  Source consensus proofs also carry explicit trust-anchor/verifier evidence
  for the source anchor, consensus verifier, message-inclusion verifier,
  finality policy, adapter proof, adapter transcript, and adapter circuit, with
  the evidence hash included in the adapter OpenVerify statement. Those
  evidence records are now sourced from typed `SccpSourceVerifierMaterialV1`
  records; the built-in catalog is placeholder-only and cannot satisfy the
  production gate until real source-chain trust anchors and immutable verifier
  hashes replace it; flipping the placeholder flag or reusing any built-in
  placeholder component still fails closed. Explicit-material production helpers
  now verify source envelopes against caller-supplied material, but production
  readiness also requires an exact domain profile: today ETH mainnet, BSC
  mainnet, Solana mainnet-beta, TON mainnet masterchain/shard, TRON mainnet
  profiles can satisfy the material gate only with deployment-supplied
  component hashes, while generic ids and hashes remain fail-closed unless they
  match an exact profile and avoid template-derived hashes. The TRON mainnet
  message-inclusion profile id now explicitly names the governed
  transaction-source verifier instead of the legacy receipt-root-branch label,
  keeping Rust, portal, mobile, and evidence vectors aligned with the
  production adapter proof shape. The offline
  source-evidence regression suite now exercises every template-derived
  record hashing plus governance TOML rendering, so one live component cannot
  hide another placeholder component in production material. The same direct
  evidence helpers reject all-zero production component, bridge, adapter
  verifier-key, and deployment receipt hashes instead of relying only on CLI
  parsing, and the Rust source-material constructors now reject all-zero or
  template-derived role hashes plus reused non-zero role hashes before
  returning deployment-shaped material.
  Solana and TON deployment-backed production readiness now
  additionally require complete governed full-light-client audit bundles before
  the source-adapter gate can open, and Solana/TON full-light-client audit hashes
  must be role-separated from each other and from existing source-adapter
  material and cannot reuse built-in template source-material component hashes.
  Solana and TON deployment-bound proof regressions now mutate every governed
  full-light-client audit verifier role hash after proof construction and
  require deployment matching, deployment-aware production verification, bundle
  extraction, and verifier-evidence splices to fail even when the replayed
  deployment remains generally well-shaped and source-adapter ready.
  TRON deployment-bound proof regressions now build coherent alternative
  production-ready source material/deployment pairs for source trust anchor,
  consensus, message-inclusion, source bridge emitter/code/network/owner,
  finality-policy, and deployment-receipt replay, then require the original
  proof and any post-construction evidence splice to fail against those
  alternate DPoS source-gate deployments.
  `zk.sccp_source_verifier_materials`,
  `zk.sccp_source_adapter_engine_deployments`,
  `zk.sccp_destination_rollouts`, and `zk.sccp_route_allowlists` now thread
  configured source-verifier material, matching source-adapter deployment
  receipts, destination rollout records, and governed route allowlists into
  on-chain bridge proof admission and the ZK consensus policy hash. Material
  alone can no longer open a non-SORA source lane: admission also requires an
  exact deployment record for the same domain/profile/circuit with matching
  SORA target domain, trust-anchor, consensus-verifier,
  message-inclusion-verifier, finality-policy hashes, a non-zero deployment
  receipt hash, and an `adapter_verifier_vk_hash` equal to the canonical
  lane-specific source-adapter verifier commitment and the OpenVerify `vk_hash`
  embedded in the user-submitted source proof. The ETH/BSC, Solana, TON, TRON,
  source/deployment role-hash separation before returning canonical record
  hashes, keeping live collectors and programmatic governance tooling aligned
  with TOML rendering and all-lanes preflight.
  BSC deployment-bound facade regressions now build coherent alternate
  production-ready source material/deployment pairs for source trust anchor,
  consensus, message-inclusion, finality-policy, governed source bridge emitter
  address/runtime code hash, and deployment-receipt replay, then require the
  original proof, local-admission artifact, bundle extraction, and any
  post-construction evidence splice to fail against those alternate EVM-family
  deployments.
  JavaScript, Python, Swift,
  Kotlin, and Java Android SDKs now derive the same canonical source-material
  and source-adapter deployment record bytes/hashes for portal and mobile proof
  UIs, and they reject reused non-zero source/deployment role hashes before
  request hashing or app-linked prover invocation. Solana full-light-client
  audit request builders across JavaScript, Python, Swift, Kotlin, and Java
  Android now derive governed source-material, source-adapter deployment, and
  audit gate hashes from component/deployment records, reject stale precomputed
  annotations, and require the witness deployment hash and deployment receipt
  to match the derived deployment record before user-side proof generation.
  TypeScript declarations expose the same flattened component hashes,
  annotation fields, and witness deployment hash/receipt inputs for strict web
  portal callers.
  Their UI/mobile
  source-adapter deployment-binding normalizers also reject a non-zero
  deployment hash that equals its deployment receipt hash, matching the core
  production evidence role-separation rule before user-generated proofs are
  submitted on-chain. The all-lanes preflight
  also parses the source record hash comments emitted by those evidence helpers
  and rejects missing or stale material/deployment hash annotations, so
  user-side provers can audit governed lane evidence before submitting on-chain
  proofs. The same SDKs
  also derive canonical native destination
  binding keys and hashes for SORA -> Solana, SORA -> TON, and SORA ->
  retired runtime-network lanes, aligning user-side proof
  requests with the destination rollout evidence helpers. For EVM-family and
  TRON source
  lanes, material
  and deployment records must also carry the same governed source-bridge
  emitter id, address, and non-zero runtime code hash; non-emitter source
  domains must leave those fields empty/zero. It also requires an exact
  destination rollout and an exact activated route allowlist whose policy hash
  is bound to the canonical source-material record hash, source-adapter
  deployment record hash, and destination binding hash for that lane. The
  all-lanes evidence preflight now additionally requires each route allowlist
  table to carry passed post-deploy canary metadata bound to the same route
  allowlist hash and destination binding hash, so a stale canary from another
  route cannot make a lane production-ready; the canary evidence hash must also
  be distinct from every advertised source material record hash, source-adapter
  deployment record hash, route allowlist hash, and destination binding hash,
  and unique across all advertised lanes. Cross-lane canary replay blockers now
  attach to the target lane summary as well as the bundle summary, so per-lane
  rollout automation cannot treat a globally rejected lane as production-ready.
  Core and Torii configured runtime all-lanes admission mirror the same global
  replay checks, and the lane-aware Rust route-canary builder refuses source
  record hash replay before config objects are minted.
	  reusable render/summary evidence APIs now run the same deployed bytecode,
	  program bytes, runtime code, or code BoC hash derivation as their CLI paths,
	  so portal backends and SDK automation cannot bypass byte/hash mismatch checks
	  by importing helper modules directly. TON production TOML and all-lanes
	  readiness now preserve that derivation as explicit code-BoC base64,
	  root-hash, and match metadata, and the all-lanes gate decodes the staged BoC
	  to recompute the TON representation root. A copied TON code hash without
	  destination evidence now preserves finalized runtime code as base64, and the
	  all-lanes gate decodes it to recompute the BLAKE2b-256 runtime code hash
	  before accepting SORA-family runtime rollouts.
  The EVM-family, Solana, TON, and
  metadata for production TOML via `--route-canary-evidence-hash`, keeping
  operator TOML generation aligned with the stricter all-lanes launch gate. The
  direct destination and TRON full-lane renderers also reject route canary
  hashes that reuse any governed source material record hash, source-adapter
  deployment record hash, route allowlist hash, or destination binding hash
  before JSON summaries or production TOML are emitted. Solana and
  canary hash from immutable ProgramData or finalized runtime metadata, and
  the all-lanes gate rejects generic non-zero canary hashes for those lanes.
  JavaScript, Python, Swift, Kotlin, and Java Android SDKs now expose the same
  Solana immutable ProgramData route-canary transcript and hash derivation, so
  web portals and mobile apps can verify governed lane evidence before their
  app-linked provers submit proofs on-chain. Those helpers now fail closed on
  non-canonical Solana destination bindings by default and reject explicit
  expected destination-binding hashes that would steer route-canary evidence
  away from the governed SORA -> Solana rollout. Rust SCCP regression coverage
  also pins the same canonical binding rule at destination-rollout readiness,
  route-canary hash derivation, and route-allowlist evidence derivation.
  The
  default
  production path remains closed on the placeholder catalog when no complete
  configured lane material is present, and configured bridge-proof admission
  now uses Ethereum mainnet as the first production lane. ETH can open with
  complete source material, source-adapter deployment, destination rollout,
  route allowlist, and route-canary evidence while other advertised remote SCCP
  domains remain behind their future lane policies. The all-lanes gate remains
  available as the diagnostic release check when operators need to prove every
  advertised lane at once. TRON source material and deployment
  records must additionally carry the same non-zero source bridge network id,
  governed owner address, and config hash derived from the deployed bridge
  address, network id, source/target domains, and owner, so a reused emitter
  address or bytecode hash cannot satisfy the source lane without the matching
  governed bridge configuration. The all-lanes evidence preflight now also
  rejects unsupported remote domains, unsupported `zk.*` evidence sections,
  malformed direct evidence sections, non-integer domain fields, and unexpected
  fields outside each section's exact evidence schema before lane matching,
  including JSON/TOML boolean values that would otherwise alias domain ids in
  Python. TRON transaction-source proofs for production
  material also require the authenticated `TriggerSmartContract.owner_address`
  to match that configured owner. TRON sender
  and recipient codec
  validation now also rejects the checksummed all-zero `0x41` address payload,
  keeping the account surface aligned with the non-zero witness/verifier/source
  bridge address gates. The EVM destination side now has a
  `SccpGroth16Bn254MessageVerifier` implementation for the
  `evm-groth16-bn254-v1` backend, and the wrapper binds deployments to the
  expected verifier bytecode hash plus the Groth16 verifier's immutable
  verifying-key hash. The wrapper now rejects empty backend/proof-family
  labels, zero network ids, zero target domains, same-domain deployments, zero
  statement hashes, zero required public-input fields, and target-domain words
  that do not match the governed lane before verifier dispatch. Rust/Torii EVM
  destination-binding helpers now require
  the same verifier code hash and, for Groth16, a non-zero verifier key hash
  before producing deployment-specific bindings, and Rust EVM Groth16 package
  construction plus verification now parse the deployment-binding key and
  recompute the canonical binding hash before accepting a supplied relay
  package; ETH/BSC default destination blockers now track only missing live
  verifier deployment and trust-anchor evidence, not a stale relayer-wiring
  blocker. The offline
  `scripts/sccp_evm_destination_evidence.py` helper now recomputes the
  SORA -> ETH/BSC EVM Groth16 destination binding hash from network id,
	  verifier address, bridge wrapper address, verifier code hash, and verifier
	  key hash, rejects boolean or non-`u32` programmatic domain ids,
	  rejects non-canonical direct-helper backend/proof-family labels,
	  and renders the governed destination rollout plus route allowlist TOML only
	  after `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash` are present. Its direct TOML and JSON helpers
	  also reject caller-supplied destination binding hashes that differ from the
	  canonical lane binding, report unpinned, bridge-runtime-hash-missing, or
	  canary-missing JSON as not TOML-ready, and require the governed route
	  allowlist hash to recompute from the canonical source-material record hash,
	  source-adapter deployment record hash, and SORA -> ETH/BSC destination
	  binding hash before emitting route summaries. The direct helper can now
	  derive the bridge wrapper runtime hash from bytecode and rejects mismatches
	  with a supplied `--bridge-code-hash`. Binding-only JSON now omits route
	  evidence until the expected
	  destination binding pin is supplied and matched.
		  The live collector carries the same check before producing offline TOML
		  arguments, requires the wrapper's `destinationBindingHash()` view to match
		  the recomputed immutable deployment inputs, rejects supplied route allowlist
		  evidence before the expected destination binding pin matches the live
			  deployment, and only then carries the route allowlist/source-record hashes
			  plus the route canary evidence hash in its diagnostic offline argument
		  bundle. It now also withholds Torii
		  artifact/job destination query fields until that explicit binding pin
		  matches and marks emitted query fields as requiring the prover-produced
		  `proof_bytes_hex`, so EVM live evidence cannot look package-ready without
		  the external Groth16 tuple. ETH/BSC source live TOML now also requires
		  a fetched deployment transaction receipt whose status is `0x1`, contract
		  address is present as a non-zero EVM address matching the governed source
		  bridge, block hash is non-zero, block number is positive, and
		  `transactionHash` echoes the operator-supplied deployment transaction;
		  the all-lanes preflight treats missing source receipt metadata as a
		  rollout blocker. EVM destination live TOML now carries the verifier
		  runtime code hash and verifier key hash observed from JSON-RPC, and the
		  all-lanes preflight requires those live comments to match the structured
		  verifier fields. Destination rollout comment metadata must also echo the
		  structured network-id, bridge-address, binding-key, and binding-hash fields
		  when both are present, so stale operator TOML comments cannot hide behind
		  canonical fields during all-lanes activation. Direct EVM destination TOML now
		  emits the same RPC chain-id, bridge runtime code hash, verifier runtime code
		  hash, and verifier key hash comments required by all-lanes, and includes
		  replayable bridge/verifier runtime-bytecode comments when bytecode is
		  supplied. The live wrapper now carries `eth_getCode` bytecode through the
		  shared offline renderer, and all-lanes decodes those comments to recompute
		  Keccak-256 before ETH/BSC destination rollout evidence can pass. Direct
		  ETH/BSC source TOML now requires audited deployment
		  transaction, receipt contract address, receipt block hash, and receipt block
		  number metadata plus source bridge runtime-bytecode preimages before
		  rendering, emits the same EVM source live comments required by all-lanes,
		  and the EVM source live wrapper suppresses duplicates after reusing the
		  direct renderer. The offline
  `scripts/sccp_tron_source_bridge_evidence.py` renderer now applies the same
  route allowlist evidence binding on the TRON full-rollout path: a supplied
  `--route-allowlist-hash` must recompute from the canonical TRON source
  material record hash, source-adapter deployment record hash, and SORA -> TRON
  destination binding hash before JSON, direct TOML, or live-rendered full TOML
  can be emitted, rejects padded fixed-width component hashes and network ids
  before those records are derived, and JSON route checks now require the
  expected destination binding pin as well. Direct and live full TOML now also require
  `--route-canary-evidence-hash`, aligning TRON rollout generation with the
  all-lanes canary gate. The live collector also rejects a queried destination verifier
  whose `networkId()` differs from the queried source bridge `networkId()` and
  now requires an explicit `--expected-destination-binding-hash` match before
  emitting live full-TOML rollout records. The direct TRON helper also now
  accepts only canonical ASCII decimal `u32` domain text on the CLI and exact
  Python `int` domain values in importable hash/calldata APIs, preventing
  boolean, hex, leading-zero, or signed spellings from aliasing production lane
  ids.
  Rust route-evidence helpers now derive an evidence-bound allowlist hash only
  from production-ready source material, matching source-adapter deployment,
  production destination rollout material, and coherent TRON network ids; replayed
  or internally incomplete lane components leave route evidence unbound.
  The all-lanes evidence preflight mirrors this by refusing to recompute route
  hashes unless source material/deployment record hashes and the destination
  binding hash are present and non-zero.
  Core bridge-proof admission now has TRON regressions proving exact configured
  source material, source-adapter deployment, destination rollout, and
  route-allowlist evidence reaches the all-lanes launch gate, while a replayed
  route allowlist hash and a production-shaped destination rollout with a
  mismatched TRON network id are rejected after that source-adapter gate opens.
  Production still needs the real
  recursive SCCP circuit verifying key and governed deployment material before
  routes can be marked ready. The offline
	  destination evidence helpers now validate the deployed verifier identity and
	  non-zero code material before rendering exact SORA -> counterparty
	  destination rollout plus route allowlist TOML. Production TOML now requires a matching
	  `--expected-destination-binding-hash` and a non-zero
	  `--route-canary-evidence-hash`; unpinned or canary-missing JSON remains
	  diagnostic and is reported as not TOML-ready. Their direct TOML and JSON helpers
  reject mismatched caller-supplied binding hashes and require the route
  allowlist hash to bind the source material record hash, source-adapter
  deployment record hash, and SORA -> Solana destination binding hash. The live
  `scripts/sccp_solana_live_evidence.py` helper now collects the deployed
  Solana verifier ProgramData through read-only JSON-RPC, rejects mutable
  upgrade-authority programs and non-canonical Program account layouts, derives
  the verifier code hash as BLAKE2b-256
	  over ProgramData executable bytes, preserves those executable bytes as base64
	  in live summaries, offline replay arguments, and TOML metadata, and requires
	  pinned ProgramData plus code
	  hash values before rendering production TOML, including a positive
	  `--expected-programdata-slot` that must match the live ProgramData account
	  and the same route canary evidence hash required by all-lanes preflight.
		  Solana live JSON-RPC errors now redact HTTP bodies, transport reasons,
		  duplicate key names, and error objects before public diagnostics are emitted.
		  The live CLI also redacts sensitive top-level collection failures to a fixed
		  Solana evidence-collection diagnostic before printing operator errors.
		  That route canary hash is now recomputed from the governed route tuple,
	  verifier program id/code hash, finalized RPC commitment, immutable
	  ProgramData account metadata, read context slots, and deployed executable
	  bytes before direct/live Solana TOML or all-lanes readiness can pass. It
	  also rejects verifier code hash reuse across route allowlist, destination
	  binding, source material, and source deployment roles before the route
	  canary transcript is accepted.
	  The direct Solana destination helper and its importable render/summary APIs
	  can also derive the verifier code hash from supplied program bytes and
	  reject mismatches with an explicit `--verifier-code-hash`, so offline review
	  no longer depends on a manually transcribed executable hash. Inline direct
	  Solana verifier program bytes are now exact evidence: padded
	  `--verifier-program-bytes-hex` or `--verifier-program-bytes-base64` values
	  fail instead of being normalized into executable preimages.
	  Direct Solana destination TOML now requires audited ProgramData address,
	  ProgramData slot, and finalized RPC context slots before rendering and emits
	  the same immutable ProgramData comments required by all-lanes, including the
	  canonical 36-byte upgradeable Program account length.
	  The live evidence now also records BPF upgradeable-loader ownership,
	  immutable-program status, and finalized JSON-RPC context slots for the
	  Program and ProgramData account reads, keeps `confirmed` reads
	  diagnostic-only, and rejects ProgramData reads whose context slot is older
	  than the ProgramData deployment slot or Program account reads whose context
	  slot is older than that same deployment slot. RPC context slots must be
	  positive integer JSON numbers; booleans are rejected before evidence is
	  summarized or rendered. The offline Solana destination helper applies the
	  same exact-integer rule to importable ProgramData slot and context-slot
	  arguments before deriving ProgramData metadata or reporting TOML readiness.
	  The live helper now also rejects padded ProgramData slot arguments and
	  executable base64 metadata before deriving immutable ProgramData comments.
	  Its JSON dry runs include
	  replayable offline evidence arguments and a deterministic TOML digest after
	  all live and governance pins match. The all-lanes evidence preflight now
	  requires finalized Solana live RPC commitment, BPF-loader ownership,
	  immutable-program status, ProgramData address, pinned positive slot,
	  positive RPC read context slots at or after the ProgramData deployment
	  slot, and executable BLAKE2b-256 plus base64 executable-preimage metadata
	  comments. It decodes that executable preimage to recompute the hash and rejects
	  offline/manual Solana destination records that lack that
	  immutable-deployment evidence.
	  Solana direct JSON now also surfaces route-allowlist, route-canary,
	  ProgramData metadata, executable-preimage, and `full_toml_ready`
	  readiness booleans. Complete route evidence without ProgramData pins now
	  remains diagnostic with `programdata_metadata_ready = false`; stale
	  ProgramData metadata still fails closed. Live JSON distinguishes
	  `destination_toml_ready` from the final finalized/pinned
	  `full_toml_ready` gate.
	  The offline `scripts/sccp_ton_destination_evidence.py` helper now validates
	  TON raw verifier contract addresses as basechain workchain `0` addresses
	  and can derive non-zero verifier code hashes from single-root TON code BoCs
	  before rendering exact SORA -> TON destination rollout plus route allowlist
	  TOML. TON raw addresses, fixed-width hashes, last-transaction logical-time
	  text, live remote hash strings, and live/imported code-BoC base64 now reject
	  surrounding whitespace before they can enter rollout evidence. Production
	  TOML now requires a matching `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash`; unpinned or canary-missing JSON remains
	  diagnostic and is reported as not TOML-ready. Its direct TOML and JSON
	  helpers now also require the governed route allowlist hash to recompute from
	  the TON source-material record hash, audited source-adapter deployment record
	  hash, and SORA -> TON destination binding hash before emitting records or
	  summaries. The live `scripts/sccp_ton_live_evidence.py` helper now
	  collects TON Center v3 account-state evidence for the deployed verifier
	  contract, requires an active account with code BOC, recomputes the code BOC
	  root hash against the returned code hash, pins code hash plus account-state
	  hash plus the same route canary evidence before production TOML, and emits
	  replayable offline evidence arguments plus a deterministic TOML digest
	  after all pins match. Its JSON dry-run now splits
	  `destination_toml_ready` from `full_toml_ready`, so rollout automation can
	  distinguish complete live destination/route evidence from the additional
	  independent code-hash and account-state pins required for production TOML.
		  TON live accountStates diagnostics now redact HTTP bodies, transport
		  reasons, duplicate key names, and TON Center error objects before public
		  blockers are emitted. The live CLI also redacts sensitive top-level
		  collection failures to a fixed TON evidence-collection diagnostic before
		  printing operator errors.
	  Direct inline `--verifier-code-boc-hex` and
	  `--verifier-code-boc-base64` values now reject surrounding or embedded
	  whitespace instead of normalizing padded code-BoC preimages; file inputs
	  remain suitable for raw, hex, or base64 artifacts. Offline replay arguments
	  include the returned code BoC
	  so direct TOML generation can rederive and emit the same code-BoC root
	  evidence.
	  The all-lanes preflight now requires those live TON account-state values
	  as governed destination rollout fields, keeps the imported comments in
	  agreement with the config fields when present, and decodes the staged
	  verifier code BoC plus the required live base64/hash-match comments to
	  recompute the TON representation root before launch readiness can pass.
	  Direct TON destination TOML now requires explicit
	  active account status, audited account-state hash, last transaction LT,
	  last transaction hash, and matching code-BoC bytes/root metadata; it emits
	  both replay comments and runtime `ton_*` rollout fields, while
	  offline/manual TON destination records that lack that status, audit, or BoC
	  replay evidence remain diagnostic and do not pass launch readiness.
	  TON route allowlists now also carry `ton_route_canary_*` live-account
	  snapshot fields. Runtime lane readiness and the all-lanes preflight both
	  recompute the route canary hash from the governed route hash, destination
	  binding hash, source material/deployment hashes, verifier identity/code
	  hash, active account status, account-state hash, last transaction LT/hash,
	  and code-BoC root hash, so a generic non-zero canary hash or drifted
	  live-account metadata cannot open the SORA -> TON lane. Direct TON
	  evidence and all-lanes validation also reject reuse between the live
	  account-state hash and last-transaction hash snapshot roles.
	  renders exact SORA -> retired runtime-network destination
	  rollout plus route allowlist TOML with the fixed
	  `SccpBridge.submit_message_proof` verifier entrypoint. Production TOML now
	  requires a matching `--expected-destination-binding-hash` and
	  `--route-canary-evidence-hash` for the selected runtime lane; unpinned or
	  canary-missing JSON remains diagnostic and is reported as not TOML-ready.
	  It now rejects padded runtime-lane selectors and runtime `specName` values
	  before destination rollout or route metadata can be rendered. Its
	  direct TOML and JSON helpers reject mismatched caller-supplied binding hashes
		  and revalidate the fixed entrypoint and deployment code hash, then require
		  the route allowlist hash to bind the source material record hash,
		  source-adapter deployment record hash, and selected SORA ->
		  hash roles required to be non-zero and pairwise distinct before the
		  transcript is accepted. Public release-bundle verification now
		  recomputes that route-allowlist transcript from embedded all-lanes
		  evidence instead of trusting the self-reported expected-hash match
		  destination evidence can derive the runtime verifier code hash from supplied
		  runtime bytes and rejects mismatches with an explicit
		  `--verifier-code-hash`, matching the live finalized `:code` hash
		  derivation used for production evidence. Inline
		  `--runtime-code-hex` and `--runtime-code-base64` values now reject
		  surrounding or embedded whitespace instead of normalizing padded
		  runtime-code preimages.
		  destination TOML now also requires audited finalized head, runtime spec
	  name/version, and transaction version metadata, rejects runtime `specName`
	  values that do not match the selected destination lane, rejects boolean
	  runtime version placeholders before readiness is derived, and emits the
	  same runtime comments required by all-lanes. The live
	  head, runtime spec/version fields, and BLAKE2b-256 hash of finalized
	  `:code`, requires the live `specName` to match the selected destination
	  domain, requires the same route canary evidence before production TOML, and
	  rejects padded `specName`, expected `specName`, runtime version text,
	  non-lowercase or non-`0x` finalized-head hex, and runtime `:code` hex
	  before emitting live metadata
	  comments required by the all-lanes preflight before
	  canary hash from the governed route tuple, runtime entrypoint/code hash,
	  finalized head, runtime version metadata, and finalized runtime bytes
	  before accepting SORA-family runtime readiness. It also rejects runtime code
	  hash reuse across route allowlist, destination binding, source material,
	  and source deployment roles before the route canary transcript is accepted.
	  Configured Rust readiness
	  now carries the same finalized runtime fields in destination rollouts and
	  rejects SORA-family launch without them. The offline
  same three runtime lanes from governed finality/event-storage component
  hashes, adapter verifier key hashes, and deployment receipt hashes, and it
  rejects padded runtime-lane selectors, component hashes, and target domains
  before those record hashes are derived.
  Destination rollout records are now bound
  to domain, chain,
  exact mainnet/runtime anchor id, chain-specific verifier identity format, and
  a non-zero Groth16 verifier-key hash for EVM-family/TRON lanes before they can
  rollout records now reject any unexpected verifier-key hash. ETH/BSC require
  non-zero EVM contract addresses and reject verifier/bridge wrapper address
  aliasing across direct, live, and all-lanes evidence, Solana requires a
  non-zero program id, TON requires a non-zero raw contract address, TRON
  require the exact SCCP runtime entrypoint.
  EVM and TRON Groth16 relay packages are
  signer-free: they carry the verifier proof ABI tuple directly and reject
  attempts to use the reference attestation/signer path for the production
  backend, including verifier-side rejection when a submitted package reuses
  the generic manifest destination-binding hash. The normalized proof-job
  builder now has explicit signer-free Groth16 paths, so production EVM/BSC
  proof tooling must provide the Groth16 proof bytes and deployment binding
  instead of falling back to Torii signer
  attestations, and production TRON tooling must provide the TVM Groth16 proof
  bytes plus a deployment binding derived from the checksummed verifier
  contract address, verifier code hash, and verifier key hash instead of
  falling back to generic FastPQ/OpenVerify bytes or the manifest binding.
  Rust packaging now decodes those proof bytes through BN254 G1/G2
  curve-membership checks, including G2 subgroup preflight, so off-curve or
  non-subgroup 12-word Groth16 tuples are rejected before Torii emits
  deployment-bound EVM or TRON contract-call payloads. JavaScript and Python
  portal helpers mirror the G1/G2 curve-equation and G2 subgroup checks before
  wrapping Groth16 prover results, emitting direct EVM/TRON wallet calldata, or
  forwarding lower-level Torii `proofBytesHex` / `proof_bytes_hex` query and
  submit fields; Swift, Kotlin, and Java Android raw bridge-submit clients
  apply the same checks before posting deployment-bound bridge DTOs.
  Torii artifact, proof-job, bridge-proof submit, and bridge-message submit
  paths now accept external `proof_bytes_hex` plus TRON
  `tron_verifier_address` deployment material, validated as a checksummed TRON
  Base58Check address by the relay clients, so relays can package the same
  deployment-bound proof bytes exposed by the SDK prover wrappers. Torii now
  fails EVM/TRON Groth16 artifact and proof-job packaging as a bad request when
  the deployment material is present but the external Groth16 proof tuple is
  missing, instead of falling through to generic signer/FastPQ package
  construction. Torii now
  rejects empty, all-zero, or non-384-byte external EVM/TRON Groth16 proof
  bytes before constructing a deployment-bound package, and the Rust,
  JavaScript/Python typed Torii clients, Swift SDK, Kotlin SDK, and Java
  Android SDK reject placeholder or non-canonical `proofBytesHex` plus
  malformed TRON verifier addresses before making artifact, proof-job,
  bridge-proof, or bridge-message requests. Torii
  typed artifact and proof-job clients also bind external EVM/TRON Groth16
  `proofBytesHex` / `proof_bytes_hex` to the normalized request message id and
  SORA source-domain word before network I/O, so a valid proof tuple for one
  SCCP message cannot be replayed into another artifact/job query.
  Torii
  typed submit clients now also use the local `message_bundle` to reject
  cross-source or replayed EVM/TRON Groth16 tuples before posting bridge-proof
  or bridge-message DTOs; Rust, Swift, Kotlin, and Java Android raw JSON bridge
  submit helpers enforce the same tuple/message-bundle binding. Torii
  now validates supplied EVM/TRON destination and proof fields before the
  disabled-lane readiness fallback, including canonical tuple roundtrip and
  tuple binding to the SCCP message id, SORA source-domain word, and commitment
  root, so malformed or cross-source relay material is not masked by a generic
  lane-not-ready response; strict disabled lanes still discard validated
  deployment bindings and proof bytes instead of exposing relay material while
  production readiness is false, but Torii retains the validated destination
  binding internally for configured rollout and all-lanes launch checks so the
  disabled-lane discard step cannot bypass rollout governance. Rust, Python,
  and JavaScript typed clients plus the bridge-feature CLI forward the same
  destination/proof query material with canonical 384-byte BN254 tuple,
  G1/G2 curve validation, and G2 subgroup validation before network I/O. Rust,
  JavaScript, and Python query and submit clients plus Swift/Kotlin/Java
  Android raw and typed submit clients now reject off-curve BN254 G1/G2 tuple
  coordinates and on-curve non-subgroup G2 points before network I/O, and
  the typed clients reject deployment destination fields when the required
  `proof_bytes_hex` is absent or a standalone `proof_bytes_hex` lacks
  deployment destination fields, so operators cannot fetch incomplete
  production EVM/TRON submission packages through any primary typed client or
  the bridge-feature CLI. The same Rust, bridge CLI, web, Python, and mobile
  SDK preflight now rejects partial deployment tuples:
  proof bytes must be paired with the full EVM field set
  (`network_id_hex`, `verifier_address_hex`, `bridge_address_hex`,
  `verifier_code_hash_hex`, `verifier_key_hash_hex`,
  `expected_destination_binding_hash_hex`) or the full TRON field set
  (`network_id_hex`, `tron_verifier_address`, `verifier_code_hash_hex`,
  `verifier_key_hash_hex`, `expected_destination_binding_hash_hex`), and mixed
  EVM/TRON destination material is rejected locally. Torii's direct
  destination-material parser now enforces the same all-or-nothing rule before
  destination binding construction or disabled-lane fallback. Rust, web,
  Python, and mobile bridge-proof submit clients also enforce Torii's bundle
  selection before network I/O: exactly one of `burn_bundle` or
  `message_bundle` must be supplied, and deployment destination proof material
  is valid only with `message_bundle`.
  Rust, Swift, Kotlin, and Java Android raw JSON bridge submit helpers now also
  reject empty, all-zero, or non-384-byte snake-case `proof_bytes_hex`, missing
  proof bytes when destination deployment fields are present, or proof bytes
  without destination deployment fields before posting deployment-bound DTOs;
  those raw-submit preflights also require
  `message_bundle.commitment.message_id` and `message_bundle.commitment_root`
  whenever proof bytes are submitted with a message bundle, bind the proof tuple
  to that bundle context, and shape-check recognized destination hashes, EVM
  addresses, network IDs, and TRON verifier addresses before network I/O.
  Python Torii typed artifact, proof-job, bridge-proof, and bridge-message
  clients additionally keep TRON deployment material exact by rejecting padded
  Base58Check verifier addresses and surrounding/internal whitespace in inline
  network id, verifier code/key hash, expected binding hash, and proof-byte
  fields before request serialization. JavaScript, Swift, Kotlin, and Java
  Android Torii submit/query preflights now apply the same exactness to
  string-based TRON verifier addresses and deployment/proof hex before request
  serialization while still accepting already-byte proof tuples. Swift,
  prover request builders also reject padded fixed-width
  payload/proof-context hashes before deriving proof transcripts. Their shared
  SCCP source-proof helpers apply the same exact hash rule to source-adapter
  deployment binding and source-proof transcript hashes. Kotlin and Java
  Android additionally reject non-canonical decimal finality heights at the
  text parser boundary, while Swift keeps finality heights typed as `UInt64`.
  JavaScript web portal TON proof requests and source-adapter deployment
  bindings now have matching regressions for padded fixed-width hashes and
  leading-zero finality heights before app-linked prover callbacks run.
  Swift/Kotlin/Java Android shared SCCP source-proof helpers reject padded TRON
  source-event and raw-header hex before transcript hashing.
  The Rust, JavaScript, Python, Swift, Kotlin, and Java Android clients now
  expose bridge-proof and bridge-message submit helpers for relays and mobile
  apps posting those deployment-bound DTOs; Swift, Kotlin/JVM, and Java Android
  now also provide typed bridge-proof submit request wrappers that encode into
  the same Torii preflight path used by raw JSON submissions.
  Rust/Torii packaging now rejects zero deployment network ids, zero statement
  hashes, zero required public-input fields, wrong target domains, and
  same-source/target domain public inputs before emitting EVM/TRON Groth16
  relay packages. Rust EVM/TRON Groth16 contract submission builders also
  require transparent `target_domain` to equal the manifest counterparty
  domain, so local-domain SORA public inputs cannot be packaged for
  counterparty contract calls. Their submission templates use the canonical
  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` signature, pinning
  emitted EVM/TVM calldata to selector `0xbd57826c`. Counterparty submission
  package construction now also fails closed when the manifest's envelope
  encoding is unsupported or cannot be reconstructed, rather than emitting an
  empty or generic relay envelope. The Rust TRON destination-binding helper
  also refuses non-SORA source-domain ids, same-source/target manifests,
  manifests whose counterparty target domain is not TRON, and non-`stark-fri-v1`
  proof families.
  Rust, JavaScript, Python, Swift, Kotlin, and Java Android now expose the
  canonical BN254 public-signal derivation helper used by those EVM/TRON
  Groth16 circuits, including statement and destination-binding signal words.
  The Rust TRON package builder and proof verifier now parse the
  deployment-binding key and recompute the canonical TRON binding hash before
  accepting relay packages, so tampered binding hashes fail even if the
  submission arguments and envelope are rebuilt consistently.
  The JavaScript, Python, Swift, Kotlin, and Java Android SDK surfaces also
  expose typed EVM-family and TRON Groth16 proof-request/prover wrappers,
  binding the canonical public inputs, SCCP bundle bytes, source proof bytes,
  statement hash, destination binding hash, and fixed BN254 signal words before
  an app-linked Groth16 prover emits proof bytes. Those UI-prover request
  builders and Rust proof-result wrappers now fail closed on unsupported
  transparent-public-input versions, zero statement/destination hashes, zero
  required public inputs, zero Groth16 target domains, and same-source/target
  domains before any app-linked prover result is accepted, and TRON request
  builders also require the paired SORA -> TRON destination lane. Their TRON
  source-call calldata helpers are locked to the production TRON -> SORA source
  lane and reject zero
  source-event digests before UI/mobile prover transcript derivation. Those SDK
  source-proof helpers now also derive ETH Deneb/Fulu execution-payload,
  beacon-body branch, and beacon header SSZ roots from UI/mobile witness
  material, matching the source-adapter checks for `execution_payload_branch`
  and `beacon_finalized_root`; the C# SDK now exposes the same native helpers
  and shared root vector, with release/readiness inventories pinning the
  helper names before Ethereum execution-payload binding can be advertised.
  The JavaScript
  package entrypoint now re-exports those SCCP helpers at runtime, matching the
  TypeScript declarations. JavaScript, Python, Swift, Kotlin, Java Android,
  and C# now also
  package EVM-family and TRON wrapped Groth16 proof results into
  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` contract-call calldata
  with selector/envelope bytes, six transparent ABI public-input words, and
  proof-result binding checks that revalidate proof context, request hashes, and
  envelope hashes before portal and mobile wallet submission.
  JavaScript, Python, Swift, Kotlin, and Java Android
  retired runtime-network destination lanes, locked to SORA-origin
  source domains and binding the source domain, canonical transparent public
  inputs, length-prefixed SCCP bundle/source proof bytes, statement hash, and
  destination binding hash
  before an app-linked runtime prover emits proof bytes. A TRON/TVM Solidity
  deployment entrypoint now wraps
  the shared immutable BN254 verifier under `contracts/tron/sccp/`; its
  `submitSccpMessageProof(...)` path recomputes the self-addressed TRON
  destination binding from the actual deployed runtime code hash, governed key
  hash, and lane metadata, then records accepted message ids to block replay.
  The TRON constructor rejects missing or mismatched key hashes, empty
  proof-family labels, proof families other than `stark-fri-v1`, zero network
  ids, non-SORA source-domain ids, non-TRON target domains, and
  same-source/target domains, and its submission path rejects zero
  statement/public-input fields, wrong target-domain words, and Groth16 proof
  envelopes with non-canonical ABI length or whose version, message id,
  cleartext source-domain word, source-domain width, or commitment root does not
	  match the configured lane and public inputs before verifier dispatch. Accepted
	  EVM and TRON proof events now include the SCCP statement hash and destination
	  binding hash, and the EVM wrapper exposes `destinationBindingHash()`, so live
	  canary logs can be audited against the exact governed statement and deployed
	  binding. The shared contract smoke now pins its temporary `solc`, `ganache`,
	  and `ethers` dependencies and runs with quiet Ganache logging, keeping the
	  deterministic BN254 acceptance/replay check reproducible for operator
	  validation. The TRON source bridge constructor also
  rejects any non-SORA target domain before it can emit governed source-call
  configuration.
  The shared Solidity smoke now builds a deterministic self-consistent BN254
  test proof and submits it through both the EVM Groth16 wrapper and the
  TRON wrapper, covering positive pairing acceptance, accepted-event fields,
  public-input preflight failures, source-domain overflow, and replay rejection
  alongside malformed-proof rejection.
  Production rollout still requires deploying it and recording the deployed
  code/key hashes in governed destination binding material; the offline TRON
  evidence helper can now recompute that destination binding hash, compare it
  with an expected governed value, and operators can query the same value from
  the wrapper's `destinationBindingHash()` view or the post-deploy
  `DestinationBindingConfigured` canary event during rollout.
  The lane readiness surface now separates source material from deployment
  evidence; exact configured source material can set only the source-material
  readiness bit, while external consensus, receipt/message-inclusion, and
  trust-anchor readiness require the matching configured source-adapter engine
  deployment record. Source material by itself cannot mark the source adapter
  production-ready or satisfy the deployment-aware production proof helpers.
  TRON lane-level readiness has explicit regression coverage for the exact
  source deployment, destination rollout, and route allowlist combination, plus
  replayed source, destination, and route material failures.
  Deployment-aware source proofs now bind the configured source-adapter
  deployment hash and deployment receipt hash inside
  `SccpSourceVerifierEvidenceV1`, whose hash is part of the adapter
  OpenVerify statement. Material-only source proofs remain diagnostic artifacts
  and fail the configured production deployment path even when the lane's source
  verifier material otherwise matches. The configured admission verifier now
  splits diagnostic unready handling from production admission: it can tolerate
  an unready outbound destination manifest for deployment-governed lanes, but it
  still requires non-SORA source proofs to satisfy the production
  material-and-deployment gate.
  Bridge proof admission validates SORA-origin Nexus finality separately from
  non-SORA source-chain envelopes. Nexus block-level SCCP message records are
  restricted to
  SORA-origin payloads; external-source messages must enter through bridge proof
  submission with their source-chain envelope. Disabled SCCP lanes remain
  non-consumable in state-changing Torii endpoints and on-chain bridge proof
  admission even if historical unready-proof diagnostics are enabled in config.
  SORA-origin Nexus finality proofs now carry the full Sumeragi vote-signing
  material, including parent/post state roots, chain-order hash, re-chain
  sequence, and optional highest-QC reference. `iroha_sccp` exposes a
  BLS-normal aggregate verifier for those proofs, validates validator PoPs,
  and enforces the same quorum threshold as core finality verification before
  treating the proof as production-grade.
  Torii no longer synthesizes non-SORA source-chain envelopes from local Iroha
  finality; external-source submissions must carry source-adapter proof
  envelopes. Rust, JavaScript, Python, Swift, Kotlin, and Java SDKs now expose
  local-first Solana proof requests plus TON shard-state and TON
  full-light-client audit role proof requests so web and mobile UIs can collect
  source witness data, invoke an app-linked prover, and submit the resulting
  proof on-chain without relying on node-side proof generation. Rust now also
  wraps TON final proof bytes into the same request/envelope-hashed result
  object used for proof-result submission packaging. TON
  proof request builders are now locked to the TON source domain, and Solana
  source-proof witness/request builders are locked to the Solana -> SORA lane,
  preventing portal/mobile code from producing cross-domain local prover
  requests before request hashing or prover invocation. The Solana
  full-light-client audit helpers now share a cross-SDK golden vector for the
  Tower replay, full AccountsDB lattice, and bank/fork-choice roles, including
  statement hashes and FastPQ public-input columns, so web and mobile prover
  transcripts stay byte-identical to the verifier-facing canonical form.
  The TON user-side proof helpers and source adapter now bind full masterchain
  and basechain shard BlockIdExt context, including workchain ids, shard ids,
  seqnos, block hashes, and file hashes, and dictionary-backed
  `ShardStateUnsplit.accounts` openings must match the explicit basechain
  shard id and seqno supplied to the local prover request. The TON masterchain
  config-proof helpers and verifier also pin the active validator-set opening
  to config parameter `34` through a bounded TON `HashmapE 32 ^Cell`
  dictionary proof BoC, bind that 32-bit key width into the config-proof
  transcript, and decode the proven config-34 `ValidatorSet` cell into SCCP's
  canonical validator-set payload, so portal/mobile provers cannot treat an
  arbitrary config leaf, abstract branch, or independently supplied roster as
  the active validator set. The Rust, web, Python, Swift, Kotlin, and Java
	  Android transcript builders now also reject config-proof and transition inputs
	  with wrong versions/domains, zero masterchain/config/validator hashes,
	  mismatched config-34 BoC payload/leaf/validator-set hashes, non-adjacent
	  validator-set sequence numbers, or signature proofs signed over a different
	  transition message. TON transition structural preflight now also decodes the
	  next validator-set payload, binds payload/next-set/parent-roster hashes,
	  recomputes the transition and nested validator-signature messages, checks the
	  transition signature transcript, and rejects non-adjacent or non-monotonic
	  transition chains that do not end at the adapter's active validator set before
	  Ed25519 verifier work. This removes the remaining zero-file-hash,
	  generic-shard, generic-config-leaf, placeholder config-branch, config-roster,
	  and transition-message transcript gaps in the current TON UI/mobile
	  proof-generation surface. TON source-adapter admission now also requires the
  governed full-light-client audit bundle to be present as role-separated
  OpenVerify/FastPQ proof capsules for masterchain config, validator-set
  transition, and shard-accounts dictionary verifiers, so the remaining TON
  production blockers are governed live verifier deployments, canaries, route
  rollout, and destination rollout rather than app-side request binding.
  Readiness diagnostics now report that deployment-evidence blocker instead of
  the already-implemented shard-state proof evaluation path. The
  offline `scripts/sccp_all_lanes_evidence.py` preflight now merges rendered
  source, destination, and route TOML snippets and fails with lane-specific
  blockers unless every advertised SCCP remote domain has source material,
  source-adapter deployment evidence, destination rollout material, and route
  allowlist material before governance staging. The same preflight recomputes
  the audited Solana and TON full-light-client gate hashes plus the TRON source
  bridge config hash from governed fields and invokes each lane's canonical
	  source evidence validator, preventing non-zero placeholders, template-derived
	  component hashes, or non-canonical source-adapter verifier keys from
	  satisfying rollout review. The Rust source-material/deployment gates,
	  standalone source evidence renderers, and aggregate preflight now also
	  reject reused non-zero role digests across trust anchors, consensus
	  verifiers, message-inclusion verifiers, source-state verifiers, source bridge
	  code/config hashes, adapter VKs, deployment receipts, and audited Solana/TON
	  verifier roles before governance staging. Focused TON source-state evidence
	  tests now also pin rejection when a full light-client audit hash is replayed
	  from the source trust anchor, adapter verifier VK, or deployment receipt
	  hash. Public release-bundle and readiness inventory now require the
	  source-adapter deployment receipt/VK role-separation regression plus the
	  BSC and ETH replayed deployment-receipt facade rejections, so those
	  adversarial checks cannot be dropped from production evidence bundles. It
	  also rejects lane-foreign
	  Solana or TON full-light-client audit fields, and SORA-bound audit fields
	  replayed on non-SORA target deployments, before governance staging, matching
	  the runtime deployment-shape gate and its core all-lanes admission regression
	  coverage before audit gate hashes are recomputed. Public release-bundle
	  verification now also requires each source-adapter `gate_hash` to equal the
	  lane's named final gate transcript rather than an arbitrary audit role hash,
	  so Solana tower replay, TON masterchain-config, or other component verifier
	  hashes cannot be promoted into public production evidence. Shared
	  source-adapter OpenVerify admission also
	  rejects all-zero proof bytes before decode, so placeholder adapter proof
	  envelopes cannot reach lane-specific verifier-key, schema, or public-input
  checks. It now also validates destination verifier
  identities with the lane-specific address/program/runtime parsers, preserves
  helper-emitted destination binding metadata comments, stores explicit
  destination binding fields in rollout config, and recomputes or compares the
  binding hashes before accepting rollout records. EVM-family helpers now emit
  the canonical deployment binding key, and both the preflight and runtime
  readiness gates require that key to be present and match the deployment tuple.
  binding key, and runtime readiness rejects native records that include EVM/TRON
  network or bridge-wrapper fields.
  TRON rollout records also fail if their explicit `destination_network_id`
  drifts from the governed source bridge network id used for the SORA -> TRON
  binding. The ZK consensus policy hash includes those destination binding
  fields so governed rollout evidence is committed by policy, not only by
  operator comments.
  Ready lanes report canonical source material, source-adapter deployment
  record hashes, destination binding summaries, and the recomputed
  route-allowlist evidence hash in the preflight JSON for governance
  comparison.
  JavaScript, Python, Swift, Kotlin, and Java Android now also expose the
  EVM-family and TRON Groth16 proof request wrappers for portal and mobile
  prover flows, with TRON wrappers locked to the SORA -> TRON lane, EVM-family
  wrappers locked to the governed SORA -> ETH/BSC destination lanes, and
  EVM-family, TON, and TRON wrappers rejecting empty SCCP bundle bytes before
  all-zero external proof bytes before deriving request-bound envelope hashes;
  EVM-family and TRON wrappers additionally enforce the canonical 384-byte
  Groth16 ABI length, and JavaScript/Python portal surfaces plus
  Swift/Kotlin/Java Android mobile SDKs now parse that tuple before wrapping or
  submitting proofs so the version, embedded message id, source-domain width,
  commitment root, and BN254 coordinate ranges fail closed before wallet
  calldata is emitted. Those same tuple checks now bind the embedded message
  id and commitment root to the transparent public inputs and the embedded
  source domain to the wrapped/submitted request context. JavaScript, Python,
  Swift, Kotlin, and Java Android now
  package those wrapped EVM-family/TRON proof results into production verifier
  contract-call calldata and reject mismatched proof bytes, public inputs,
  statement hashes, destination-binding hashes, or public signal words before
  handing bytes to a wallet or relayer. The JavaScript package entrypoint now
  also exports the low-level transparent public-input ABI-word encoder and
  checked `submitSccpMessageProof(...)` calldata encoder for web portals that
  package wallet calls directly. The JavaScript, Python, Swift, Kotlin, and
  Java Android direct calldata encoders now apply the same SORA source-domain
  proof-tuple check before emitting wallet calldata, so portal and mobile
  callers cannot bypass the higher-level submission wrapper with a mismatched
  Groth16 source-domain word. TON request builders also
  require the exact mainnet shard-state light-client verifier id plus a non-zero
  source-state verifier hash before local prover invocation.
	  JavaScript and Python
	  local-prover facades now also isolate the request object passed into
	  app-linked prover callbacks. The JavaScript and Python Solana/TON
	  source-state prover facades snapshot caller-supplied OpenVerify/FastPQ
	  requests into frozen callback objects with defensive-copy byte getters
	  before proof bytes are wrapped. Kotlin/JVM and Java Android mobile
	  prover facades now also hand app-linked proof engines request snapshots,
	  including Solana AccountsLtHash and full-light OpenVerify/FastPQ
	  source-state callbacks, across TON, Solana, EVM-family, TRON, and
	  while wrapping returned bytes against the original canonical request,
	  and JavaScript/Python source-state callback
	  result metadata
	  (`version`, proof family, circuit id, and exact canonical proof base64)
	  must match the active request and returned proof bytes. The
	  facades reject explicit callback result metadata that does not match the
	  active request hash, envelope hash, backend, EVM-family/TRON transparent
	  public inputs, EVM-family/TRON proof context, EVM-family/TRON public signal
	  words, optional exact proof-base64 text, Solana proof-context hash, or
	  TON/Solana source-adapter deployment-binding hash. Python and JavaScript
	  now reject whitespace-padded proof-base64 aliases instead of trimming them,
	  preventing stale UI prover outputs or callback-side request mutations from
	  being repackaged under a different on-chain submission context.
	  JavaScript and Python Solana proof-result wrappers now also reject
	  object-shaped callback results whose optional source-proof public inputs,
	  proof context, source-state verifier id/hash, or source-adapter deployment
	  binding metadata disagrees with the canonical SDK-built request, so UI
	  prover metadata cannot be silently discarded and replaced before
	  submission packaging.
	  EVM-family/TRON optional callback metadata is
  strict when present, so `null`/`None` backend, request/envelope hash,
  public-input, proof-context, statement/destination-binding hash, or
  public-signal fields fail instead of collapsing to omitted metadata.
  surfaces now also rebuild the canonical production request before invoking
  app-linked callbacks and before deriving proof-result envelope hashes, so web
  portals and portal backends cannot wrap proof bytes around manually mutated
  request hashes, public signal words, proof contexts, lane backends, or target
  domains.
	  JavaScript, Python, Swift, Kotlin, and Java Android EVM-family/TRON
	  submission builders also require wrapped `proofBase64` to match wrapped
	  `proofBytes` before contract-call calldata is emitted, matching the existing
	  Solana proof-result integrity guard. The JavaScript, Python, Swift,
	  Kotlin/JVM, Java Android, and .NET BSC mainnet facades now also pin that
	  check through their BSC-specific destination submission helpers, so generic
	  EVM proof-result validation cannot drift away from the governed BSC
	  outbound path. Those wrapped EVM-family/TRON proof
  results now carry the original request bundle/source-proof bytes, and
  proof-result based submission builders rebuild the canonical request hash
  before emitting calldata, so stale UI/mobile proof results cannot be replayed
  request bytes for runtime-proof chaining, and the JavaScript TypeScript
  declarations plus Python package `__all__` exports now publish those
  proof-result request-byte fields and wrapper helpers to portal/mobile
  integrators. The JavaScript TypeScript declarations also expose named
  local-prover callback result types for Solana, TON, EVM-family, TRON, and
  backend, binding-hash, proof-context, public-input, and public-signal
  metadata that the runtime already validates. TON TypeScript declarations now
  keep pre-proof request construction separate from post-proof message-body
  submission packaging, so `buildTonSccpProofRequest`/`TonSccpProver` no longer
  advertise proof bytes, wrapped proof results, manifest metadata, or query ids
  as prover input fields. The Python package root now exports every public SCCP
  helper/class/constant from `iroha_torii_client.sccp`, including Solana
  submission entrypoint metadata and TON audit-role verifier ids used by portal
  proof backends, and its package-root regression now derives that full public
  surface from the module so future proof helpers cannot be added only behind a
  deep import. The package-root regression also exercises the TON source-state
  proof-byte cap through the exported wrapper and `TonSccpSourceStateProver`, so
  source-only or deep-import-only cap enforcement cannot satisfy the Python SDK
  release row. The JavaScript package entrypoint now exports the same portal
	  constants at runtime and in TypeScript declarations, including the fixed
	  transparent public-input byte length, Solana submit entrypoint, and TON
	  full-light-client audit verifier ids. It also re-exports the Solana
	  full-light audit request builders, source-state capsule canonicalizers,
	  finality/vote transcript helpers, and account-inclusion tree helpers from
		  the package root so TypeScript portal imports match the packaged runtime
		  surface. The package export regression now also compares every runtime
		  SCCP export against `index.d.ts`, so portal TypeScript declarations stay
		  aligned with newly exported proof helpers such as the BSC commit-message
		  and commit-seal transcript builders.
	  JavaScript TON requests and results
	  now also freeze the callback-visible envelope and nested context/binding
  objects, expose request/proof byte fields through defensive-copy getters, and
  Python local-prover requests, callback inputs, proof results, and Solana
  submissions now use dict/list-compatible read-only envelopes so portal
  backends cannot mutate derived request hashes, proof contexts, or submission
  arguments after canonicalization.
  Swift, Kotlin, and Java Android TON wallet/liteserver submissions now expose
  the same version, `internal_message` kind, verifier entrypoint, argument
  vector, and envelope bytes/hex as the web/Python SDKs while retaining
  defensive BOC/envelope byte getters; the same mobile SDKs can build the TON
  message-body submission input directly from a local `TonSccpProofResult`, so
  apps no longer need to manually copy proof-context hashes between proof
  generation and wallet/liteserver packaging. Because SCCP launch support
  excludes retired runtime-network families for now, the SDKs ship no builders,
  prover facades, or retired codec runtime-call submission helpers for them.
  Torii and the SDK release checks now keep the production SCCP surface limited
  to ETH, BSC, Solana, TON, and TRON explicitly.
  The package root also re-exports the SCCP source-adapter OpenVerify circuit id, FastPQ
  parameter-set id, and verifier VK hash helper used by portal evidence
  checks, keeping declared TypeScript imports runtime-available.
  Swift, Kotlin, and Java Android proof-result wrappers now rederive the
  canonical request before hashing the proof envelope, Java Android EVM-family,
  Solana, TON, and TRON proof/submission results return defensive byte copies,
  request/result/submission objects now also return fresh copies for request
  byte fields and proof bytes, closing the mobile path where a manually
  constructed or mutated request object could otherwise supply stale envelope
  context. Kotlin Solana AccountsLtHash and TON shard-state source-state proof
  capsules now also defensively copy prover-returned proof bytes before those
  bytes are hashed into full-light-client audit requests, and Java Android now
  mirrors the TON proof-family/circuit-id null guards before hashing those
  capsules. JavaScript, Python, Swift, Kotlin, and Java Android TON
  local-prover calls now preflight the canonical production request before
  invoking the app-linked proof engine and reapply that guard when wrapping
  proof bytes, matching the Solana SDK guard pattern across web portal,
  backend, and mobile UI proof generation. Swift EVM-family, TRON, TON, and
  bytes through proof wrapping and submission packaging while still rejecting
  non-empty all-zero source-proof placeholders. Deployment-aware SCCP
  production source-proof extraction now enters through the deployment-aware
  bundle-structure gate, so configured material and source-adapter deployment
  evidence are checked consistently before accepting a source-chain proof
  envelope. Torii's app API artifact, proof-job, runtime proof export,
  bridge-proof submit, and bridge-message submit paths now resolve that same
  configured source lane from ZK config before wrapping UI-generated
  source-chain proof envelopes, so production Solana/TON/TRON/EVM-family proofs
  are submitted on-chain against governed source-adapter material instead of the
  static placeholder manifest.
  TON wallet/liteserver message-body builders apply the same non-empty,
  non-all-zero proof-byte and empty-bundle rejection when callers package a
  submission directly, now require TON-targeted transparent public inputs, and
  enforce the bounded 4096-cell TON message-body BOC cap before wallet or
  liteserver payloads are emitted. They also recheck wrapped TON proof results
  against the mainnet shard-state verifier profile plus canonical TON -> SORA
  source-adapter deployment binding before accepting request-bound envelope
  hashes. Wrapped TON proof results now carry the original request
  bundle/source-proof bytes, and proof-result based submission builders rebuild
  the canonical request hash before producing wallet/liteserver payloads.
  JavaScript, Python, Swift/iOS, Kotlin/JVM, and Java Android now reject
  standalone TON proof-byte payloads at submission packaging time, so UI/mobile
  apps cannot submit proof bytes against a swapped SCCP bundle after local
  proof generation.
  EVM-family and
  TRON proof-result submissions now apply the same bundle/source-proof request
  hash reconstruction before contract calldata is emitted. EVM-family, TON, and
  TRON request hashes also length-prefix bundle and source-proof bytes so the
  transcript binds their boundary. JavaScript and Python TON submission
  metadata canonicalizers now also reject versionless or lane-foreign manifests,
  non-`stark-fri-v1` proof families, non-`ton-contract-v1` verifier backends,
  TON public-input domain drift, and destination-binding overrides that differ
  from the manifest before portal/backend BOC packaging. The JavaScript and
  Python BOC builders now pass the root `destinationBindingHash` into that
  metadata canonicalizer and reject any manifest/metadata binding mismatch, and
  Swift, Kotlin/JVM, and Java Android expose matching typed mobile metadata
  canonicalizers for wallet packaging. The JavaScript TypeScript manifest
  declaration now exposes the required V1 field and pinned TON proof/backend
  labels to portal callers. Swift, Kotlin/JVM, and
  Java Android TON proof requests plus direct wallet/liteserver message-body
  builders now also reject all-zero statement and destination-binding hashes,
  matching the web/Python portal guard before mobile proof engines or wallets
  see placeholder submission context.
  The TRON/TVM contract bundle also includes `SccpTronSourceBridge`, an
  owner-governed source emitter for the production
  `submitSccpSourceEvent(uint32,uint32,bytes32)` transaction-call proof. It
  stores lane-specific immutable metadata, rejects mismatched source/target
  domain arguments, rejects zero or replayed source-event digests, and emits the
  canonical indexed `SccpSourceEvent(bytes32)` log shape used by legacy receipt
  diagnostics while the production adapter proves the successful call under
  java-tron's transaction Merkle root. That proof is
  pinned to java-tron's full serialized `Transaction` Merkle leaf hash rather
  than the public raw-data txID, and the Rust/SDK transaction-source helpers
  recompute the java-tron Merkle root from the supplied full transaction bytes,
  index/count, and branch before hashing the source transcript. The Rust
  transcript helper now also invokes the same successful source-call verifier as
  admission, while the JavaScript, Python, Swift, Kotlin, and Java Android SDK
  helpers preflight the serialized `Transaction` protobuf shape, success
  result, signature count/length, non-zero owner/contract addresses, and
  source-call calldata before deriving production transcript hashes. TRON
  header/witness signatures accept java-tron's raw recovery-id encoding while
  retaining low-S malleability checks. Production rollout still needs the live
  deployment address, runtime bytecode hash, live TOML metadata for the queried
  `sourceBridgeConfigHash()`, deployment receipt hash, and governed source
  material/deployment evidence recorded before the lane can be activated.
  The source bridge constructor now matches that production lane shape by
  requiring SORA's target domain id `0` for TRON -> SORA while rejecting any
  non-TRON source domain, any non-SORA target-domain id, and
  same-source/target deployment. Rust and Python source-bridge config-hash
  helpers enforce the same TRON -> SORA shape before deriving rollout evidence.
  JavaScript, Python, Swift, Kotlin, and Java
  Android source-call calldata helpers mirror that lane shape and reject any
  non-TRON source, non-SORA target, or zero source-event digest before
  generating `submitSccpSourceEvent(uint32,uint32,bytes32)` calldata.
  Python now mirrors the Solana local request, proof-context hash, wrapped proof
  result, and `borsh_instruction_v1` submission helper, and it builds the same
  deployment-bound TON request/result envelope for portal/operator backends.
  Python now also exposes the same canonical ETH/BSC receipt-proof, BSC
  validator-set payload, BSC ValidatorSet storage-value, metadata-proof, and
  transition-message, TON shard-proof, TON validator-set transition, TRON
  the web and mobile SDKs, so backend portal tooling can derive adapter-bound
  source proof hashes from collected source-chain witness material instead of
  accepting opaque placeholders.
  Source proof branch witnesses are now centrally bounded to 64 H256 siblings,
  matching the `u64` leaf-index depth used by the verifier, and over-depth or
  malformed branches fail before transcript hashing or root reconstruction.
  The Solana
  source adapter now cryptographically binds `message_proof_hash` to the source
  event digest, transaction-status root, raw 64-byte transaction signature, raw
  32-byte emitter program id, and a non-empty inclusion branch. The
  transaction-status Merkle leaf is derived from the source-event digest plus
  transaction identity, and source-chain inclusion proofs must carry that
  Solana-specific leaf before reconstructing the claimed transaction-status
  root with the SCCP `sccp:source:node:v1` Blake2b node hash. The SDKs expose
  the same helper, reject zero source-event digests,
  transaction-status roots, all-zero decoded transaction signatures, all-zero
  decoded emitter program ids, root/branch mismatches, or empty inclusion
  branches, and decode the UI-provided Solana base58 signature/program id
  before hashing so UI provers do not pass opaque placeholder message proof
  hashes. The JavaScript and Python helpers also reject duplicate camelCase and
  snake_case aliases for the Solana source-event digest, transaction-status
  root, transaction signature, emitter program id, and inclusion branch before
  deriving the message-proof hash or transaction-status branch root. Their
  active-stake and stake-history helpers reject duplicate aliases for validator
  public keys, validator stakes, activation epochs, and deactivation epochs
  before deriving epoch-stake-root, stake-activation, and stake-history
  transcripts.
  The Solana source adapter also verifies an embedded
  stake-weighted Ed25519 finalized-slot vote certificate: it recomputes the
  vote-message hash from the slot/header/status/message-proof material plus a
  shape-checked Solana finality-context hash, checks the unique non-zero
  validator roster hash against configured source trust-anchor material, caps
  the roster at 8,192 entries before expensive proof work, enforces strict
  `> 2/3` signed stake, and rejects malformed context, tampered signatures, or
  replayed vote hashes. The Solana source-material profile now also binds
  mainnet-beta's 432,000-slot epoch length plus the generic SCCP
  source-event leaf/node Merkle prefixes, and signed finality contexts are
  rejected unless
  `epoch == finalized_slot / 432000` and `parent_slot + 1 == finalized_slot`.
  The adapter also requires `epoch_stake_root` to derive from the signed epoch
  plus active vote roster under `sccp:solana:epoch-stake-root:v1`. It now also
  requires
  `tower_lockout_hash` to derive from the signed epoch, finalized/rooted/parent
  slots, parent bank hash, and the 32-slot lockout depth under
  `sccp:solana:tower-lockout:v1`, and the JavaScript, Python, Swift, Kotlin,
  and Java SDKs expose the matching UI/mobile helpers. The adapter now also
  requires `tower_replay_hash` to derive from the signed epoch, rooted slot,
  finalized slot, direct parent slot, and explicit 31-vote active post-root
  Tower stack under
  `sccp:solana:tower-replay:v1`; the same JavaScript, Python, Swift, Kotlin,
  and Java SDK helpers/tests expose that UI/mobile transcript. The rooted slot
  supplies the 32nd Tower confirmation. The adapter now also requires
  `stake_activation_hash` to derive from the signed epoch, active
  vote roster, activation epochs, and deactivation epochs under
  `sccp:solana:stake-activation:v1`, and rejects validators that are not
  activated before that epoch. It also requires `stake_account_state_hash` to derive from the
  stake-activation hash, authorized voter keys, delegated stakes,
  activation/deactivation epochs, vote account addresses, stake account
  addresses, vote account state hashes, and stake account state hashes under
  `sccp:solana:stake-account-state:v1`, with matching JavaScript, Python, Swift,
  Kotlin, and Java helper tests. Those account state hashes must now derive from
  `sccp:solana:account-opening:v1` account-opening metadata that binds the
  account address, expected Vote/Stake owner program id, lamports, rent epoch,
  executable flag, and account-data hash; vote and stake openings owned by the
  wrong Solana program id or marked executable fail closed. The SDKs expose the
  matching account-opening hash helper for UI/mobile proof generation. The
  adapter now also binds vote-account opening data hashes to semantic
  vote-account transcripts under `sccp:solana:vote-account-data:v1` and
  stake-account opening data hashes to semantic stake-account transcripts under
  `sccp:solana:stake-account-data:v1`, with matching SDK helpers/tests. The
  SDKs can also parse raw Solana `VoteStateVersions::V1_14_11`/`V3`/`V4`
  account data into the vote-account transcript for UI/mobile proof
  generation, while the verifier now requires each raw vote-account buffer in
  the finalized vote proof to parse back to the same semantic transcript. Those
  Rust, JavaScript, Python, Swift, Kotlin, and Java Android parsers now reject
  malformed active Tower stacks before transcript hashing: confirmation counts
  must descend exactly, vote slots must remain strictly increasing after the
  rooted slot, the root cannot overlap the active post-root stack, and every
  raw authorized-voter map key, including future scheduled rotations, must be
  non-zero. Focused
  regressions cover parser and source-adapter admission for bad confirmation
  counts, repeated vote slots, and roots that collide with the first active
  vote. V4 vote accounts are now capped to the four-entry authorized-voter
  epoch window used by the current Anza vote-interface V4 max-size fixture
  before transcript hashing, while legacy V1/V3 layouts retain the 32-entry
  prior-voter ring validation.
  The parsers also consume the VoteState
  suffix: V1/V3 prior-voter cursor data
  must have a valid circular-buffer index and boolean empty flag, zero
  prior-voter pubkeys must carry zero epoch bounds, and non-zero prior-voter
  pubkeys must carry increasing epoch bounds. Epoch-credit history is capped to
  Solana's 64-entry bound, must be sorted/monotonic, and must not include
  epochs after the signed finalized-bank epoch; the last-timestamp tuple must
  either be the default `(0, 0)` or stay at-or-before the newest parsed Tower
  vote slot with a non-negative timestamp, and remaining fixed account padding
  must be zero.
  Legacy V1/V3 vote accounts derive Solana's V4 default collector and
  commission fields from the vote account address and node pubkey; V4 account
  buffers bind the collector pubkeys, basis-point commission fields, pending
  delegator rewards, and optional compressed BLS pubkey directly, with raw V4
  commission fields capped to 10,000 bps and present V4 BLS keys required to be
  non-zero across Rust, JavaScript, Python, Swift, Kotlin, and Java Android
  before transcript hashing. The
  finalized vote proof also carries each raw 200-byte
  `StakeStateV2::Stake` account buffer, and the verifier requires the parsed
  raw stake account to match the bound semantic stake-account transcript,
  including the known Solana 8-byte legacy/current warmup-cooldown-rate slot
  and the `StakeFlags` byte; reserved stake-flag bits and unsupported
  warmup/cooldown encodings now fail closed across Rust and the web/mobile SDK
  parsers, with Java Android mirroring Kotlin's supported Solana `0.25`/`0.09`
  byte policy. The
  adapter now also binds the signed finality context to the fixed
  `SysvarStakeHistory1111111111111111111111111` account opening owned by
  `Sysvar1111111111111111111111111111111111111`; that opening's data hash must
  derive from Solana's bincode vector sysvar account-data layout under
  `sccp:solana:stake-history-sysvar-data:v1`: a little-endian `u64` entry count
  followed by newest-first `(epoch, effective, activating, deactivating)` `u64`
  records. SDK helpers still accept sorted ascending witness entries for replay
  and reverse them only for the sysvar account-data hash; SDK raw-data helpers
  and the verifier-side vote proof now also validate and hash the exact raw
  StakeHistory sysvar bytes.
  The
  adapter now also requires
  `stake_history_hash` to derive from the signed epoch, effective voting stakes,
  delegated stake-account stakes, activation/deactivation epochs, the
  stake-account state hash, and a sorted StakeHistory sysvar window containing
  the signed epoch under `sccp:solana:stake-history:v1`. It replays the
  Tower-era 900 bps warmup/cooldown schedule over that bounded window with
  integer arithmetic, requires each submitted effective stake to match the
  replayed validator status, requires the signed-epoch StakeHistory effective
  total to equal the replayed active validator roster, and exposes matching
  JavaScript, Python, Swift, Kotlin, and Java helper tests. The adapter now also
  requires deterministic
  SCCP account-inclusion branches for every vote account opening, stake account
  opening, and the StakeHistory sysvar opening, with vote and stake account
  addresses disjoint across both roles. The verifier hashes exact raw
  account/sysvar data, folds account-inclusion leaves and branch siblings into
  `account_inclusion_root`, and requires that root to be bound into the signed
  finality context. SDK account-inclusion root helpers reject zero leaf hashes
  and cap sibling branches at 64 nodes to match Rust source-adapter admission.
  SDK opened vote-account and stake-account vectors are also capped at 8,192
  entries per role before account-inclusion or AccountsLtHash proof material is
  derived, matching the source-adapter validator bound.
  The same JavaScript, Python, Swift, Kotlin, and Java SDKs
  expose account-raw-data, account-inclusion leaf/node/root, and branch-builder
  helpers for portal and mobile proof generation. Their raw StakeHistory sysvar
  hash helpers also require the bincode vector records to be in Solana's
  canonical newest-first order, matching the Rust verifier-side canonical sysvar
  bytes before UI/mobile proof flows derive the sysvar-data hash. The adapter now also requires
  `bank_fork_hash` to derive from the signed epoch, finalized slot,
  direct parent slot, bank signature count, parent bank hash, finalized bank
  hash, blockhash, transaction-status root, account-inclusion root,
  AccountsLtHash checksum, and optional hard-fork hash data under
  `sccp:solana:bank-fork:v1`. The verifier now recomputes Agave's
  SHA-256 bank internal-state hash from parent bank hash, signature count,
  blockhash, raw AccountsLtHash, and optional hard-fork data, requiring it to
  equal the adapter bank hash. Full-bank AccountsLtHash witnesses are also
  rejected when they are the neutral all-zero vector at the verifier and
  JavaScript/Python/Swift/Kotlin/Java SDK request boundaries. Raw zero
  checksum helpers remain representable for diagnostics, but opened-subset
  proof transcripts now reject an all-zero residual so the vote/stake/sysvar
  rows cannot claim to exhaust the finalized bank lattice. The
  finality context also binds
  `accounts_lt_hash_proof_public_inputs_hash`, derived from the canonical
  `sccp:solana:accounts-lt-proof-public-inputs:v1` recursive proof
  public-input transcript covering the source domain, backend id, genesis hash,
  epoch, finalized/direct-parent slots, bank signature count, bank hashes,
  blockhash, transaction-status root, account-inclusion root,
  AccountsLtHash checksum, optional hard-fork data, and derived bank-fork hash,
  with matching JavaScript, Python, Swift, Kotlin, and Java helper tests. The
  full AccountsDB lattice audit statement now binds the completed nested
  `accounts_lt_hash_proof` capsule hash directly rather than substituting only
  the public-input transcript hash, so second-stage audit proofs are tied to
  the actual user-generated source-state proof bytes. SDK
  proof-request witnesses now canonicalize Solana blockhashes to `0x` 32-byte
  hex and hash the raw blockhash bytes, so base58/hex UI inputs bind to the
  same source proof transcript. JavaScript and Python AccountLtHash helpers now
  also require the account-opening `executable` flag to be a real boolean, so
  UI/backend strings such as `"false"` cannot silently alter Agave-compatible
  lattice rows before source-state proof requests are built. Production Solana SDK prover wrappers also
  reject missing, empty, or oversized transaction-status inclusion branches
  before invoking linked provers or wrapping externally generated proof bytes,
  matching the source adapter's non-empty, 64-sibling branch requirement. They
  also require the request and witness `mainnetGenesisHash` to equal Solana
  mainnet-beta's canonical genesis hash before packaging production proof
  bytes, and the source-state wrapper overloads now fail closed if the
  originating OpenVerify/FastPQ public-input columns no longer bind the Solana
  source domain and mainnet-genesis column. They reject the Rust template
  AccountsDB source-state verifier hash, and require the full 2,048-byte
  nonzero AccountsLtHash witness so portal/mobile proof flows cannot package
  checksum-only bank-state material. The lower-level
  AccountsLtHash public-input transcript helpers now also replay the supplied
  full AccountsLtHash against both the BLAKE3 checksum and Agave bank hash
  before returning bytes/hashes in Rust, JavaScript, Python, Swift, Kotlin, and
  Java Android, so direct helper calls and full-light audit statement builders
  fail closed on checksum-only or stale-bank-hash material.
  Solana source verifier material and source adapter deployment records now
  also carry the mainnet AccountsDB recursive verifier identity plus deployed
  `source_state_verifier_hash`, so production readiness cannot be declared with
  only a generic finalized-slot/status verifier profile. Matching Solana source
  material plus deployment metadata is still structurally verifiable but no
  longer opens production by itself while the full light-client verifier stack
  remains outstanding. Production Solana
  adapter proofs now also carry a nested `accounts_lt_hash_proof`
  `SccpSourceStateVerificationProofV1` OpenVerify/FastPQ capsule with circuit id
  `sccp-solana-accounts-lt-hash-v1`; the verifier checks that capsule against the
  deployed `source_state_verifier_hash`, finalized-bank public-input schema, and
  FastPQ proof before accepting source material that is otherwise
  production-ready, with fail-closed coverage for wrong circuit ids, backend
  tags, schema descriptors, auxiliary envelope data, public-input columns, and
  backend proof bytes; oversized source-state OpenVerify capsules are rejected
  before decode. The OpenVerify schema descriptor now embeds the governed
  AccountsDB source-state verifier id and verifier hash, matching the FastPQ
  context and SDK proof-request builders, so UI/mobile provers cannot receive a
  source-state request whose schema is detached from the deployed verifier
  material. The capsule now also binds
  `opened_accounts_lt_hash_contributions_hash`, a canonical transcript of the
  opened vote/stake/sysvar account rows and their Agave account `AccountLtHash`
  contributions, so a user-generated proof cannot detach the recursive
  AccountsLtHash boundary from the account subset checked by the adapter witness.
  It also binds `opened_accounts_lt_hash_residual_checksum`, derived by
  subtracting the opened-subset aggregate from the supplied full-bank
  `accounts_lt_hash` under Agave's wrapping `u16` lattice arithmetic, so the
  nested proof records the algebraic residual that must be covered by the future
  full AccountsDB lattice proof. That residual must be nonzero before the
  transcript is hashable. JavaScript, Python, Swift, Kotlin, and Java
  Android SDKs now derive this opened contribution transcript from account
  openings/raw data for web portal and mobile provers, while still accepting
  precomputed 2048-byte Agave `AccountLtHash` rows when a proof engine supplies
  them directly. Those supplied rows are no longer trusted as opaque proof
  material: JavaScript, Python, Swift, Kotlin, and Java Android now recompute
  each opened vote, stake, and StakeHistory sysvar `AccountLtHash` from the
  paired opening/raw-data preimage and reject stale or mismatched rows before
  request hashing or local prover invocation. Python and Swift now also have
  pure BLAKE3 checksum and XOF fallbacks for AccountsLtHash request
  validation/derivation, matching the
  Kotlin/Java mobile path without requiring optional native BLAKE3/Norito
  bindings, and max-size raw account data parity vectors now cover the
  many-chunk BLAKE3 tree/XOF path across Python/Swift/Kotlin/Java.
  The verifier also rejects duplicate opened account addresses
  across the vote-account, stake-account, and StakeHistory sysvar roles, and
  the JavaScript, Python, Swift, Kotlin, and Java Android transcript builders
  reject the same duplicate-address witness before hashing or local proof
  packaging. Those opened-role paths also reject zero-lamport vote, stake, and
  StakeHistory sysvar openings even though generic Solana AccountsLtHash
  arithmetic keeps zero-lamport accounts as the neutral contribution, so a
  witness cannot alias one AccountsDB address into multiple roles or hide a
  live Solana account role behind an identity LtHash row. It also recomputes the deterministic account-inclusion tree for
  exactly those opened leaves and rejects branches rooted in a larger tree with
  extra unopened leaves. JavaScript, Python, Swift, Kotlin, and Java Android now
  expose an exact opened-account inclusion witness helper so web portal and
  mobile proof code can derive the verifier-side root and split branches from
  the same opened account inputs instead of hand-assembling tree leaves; those
  helpers now reject duplicate opened account addresses across vote, stake, and
  StakeHistory sysvar roles before deriving branch vectors. The JavaScript web
  and Python SDK account-opening and account-inclusion leaf helpers also reject
  duplicate aliases for account address, owner program id, rent epoch,
  account-data hash, finalized slot, opening object, raw data, raw-data hash,
  and nested opening address fields, and they recompute `rawDataHash` from raw
  account data when both forms are supplied. Their opened contribution, opened
	  inclusion witness, and Agave bank-hash helpers now extend that guard to
	  opened vote/stake arrays, StakeHistory sysvar fields, account-inclusion roots,
	  AccountsLtHash checksum/root fields, full AccountsLtHash bytes, parent bank
	  hashes, bank signature counts, blockhash bytes, and optional hard-fork hash
	  data before deriving residual, branch, or bank-state transcripts. The
	  lower-level Tower lockout/replay, bank-fork, and AccountsLtHash public-input
	  helpers now reject duplicate finalized-slot, epoch, rooted/parent slot,
	  parent-bank hash, bank hash, bank-fork hash, Tower vote-slot,
	  transaction-status root, account-inclusion root, AccountsLtHash
	  checksum/root, full AccountsLtHash, and hard-fork data aliases before
	  hashing. The JavaScript web
	  SDK also freezes the returned account-inclusion tree and opened-account
  witness objects plus their branch arrays and declares them readonly for
  TypeScript portal consumers, preventing post-derivation mutation before local
  proof callbacks consume the witness. The
  Tower replay transcript now also carries the derived bank-fork hash, and all
  SDK helpers require that hash before deriving `sccp:solana:tower-replay:v1`,
  so a rooted-vote stack cannot be replayed against a different finalized
  bank-state statement. All Solana full-light audit roles now expose
  `mainnet_genesis_hash` plus the common `epoch`, `rooted_slot`, `parent_slot`,
  `vote_message_hash`, and `accounts_lt_hash_proof_hash` OpenVerify
  schema/public-input columns across Rust, JavaScript, Python, Swift, Kotlin,
  and Java Android, so portal/mobile provers submit the Solana chain identity,
  finality window, voted message commitment, and nested AccountsLtHash proof
  commitment directly. The bank/fork-choice full-light
  audit role also exposes `account_inclusion_root`, `bank_signature_count`,
  `bank_hash_hard_fork_data_hash`, and `tower_replay_hash` as explicit columns,
  so portal/mobile provers submit the opened-account root, Agave signature
  count, optional hard-fork data hash, and Tower replay root as
  verifier-visible inputs rather than only through the aggregate audit statement
  hash. The Tower replay full-light audit role now also exposes
  `stake_account_state_hash`, `stake_history_sysvar_account_hash`, and
  `account_inclusion_root` as explicit OpenVerify schema/public-input columns,
  so user-side provers submit the opened vote/stake account-state commitment,
  StakeHistory sysvar account commitment, and account-inclusion root directly
  with the Tower lockout/replay hashes.
  This
  is an
  incremental cryptographic source-engine slice, not yet a complete Solana
  light client; full Tower BFT vote-account/state replay beyond the bound
  31-vote active post-root stack plus rooted confirmation transcript,
  replacing the current reference AccountsLtHash OpenVerify/FastPQ capsule with a
  deployed full AccountsDB lattice verifier, and full bank-state/fork-choice rule
  evaluation
  remain open. The same
  JavaScript, Python, Swift, Kotlin, and Java SDK
  surfaces now expose canonical ETH/BSC receipt-proof, BSC validator-set
  payload, BSC ValidatorSet storage-value, metadata-proof, and
  transition-message, ETH sync-committee transition payload, TON shard-proof,
  TON masterchain block-message/signature, TON validator-set transition payload,
  TRON receipt-proof, TRON transaction-source proof, TRON
  authority-set transition-message/justification transcript helpers, so web,
  operator, and mobile tooling can derive every adapter-bound source proof hash
  from collected witness material before invoking the linked prover. The ETH/BSC
  source adapters now derive
  `receipt_trie_proof_hash` from the source event digest, receipt root,
  finality witness, receipt-trie index, bounded MPT proof nodes, and inclusion
  branch instead of accepting any non-zero receipt proof placeholder. They also
  open the receipt trie under the finalized ETH execution receipts root or BSC
  `receipts_root`. Non-placeholder ETH/BSC material now requires the proven
  value to decode as an actual successful legacy or typed EVM receipt whose log
  topics match the canonical SCCP source event ABI
  (`keccak256("SccpSourceEvent(bytes32)")`, `source_event_digest`) with empty
  event data and whose log emitter equals the governed source bridge emitter
  address carried by production source material and source-adapter deployment
  evidence; the parser rejects failed
  receipts, non-minimal cumulative gas, malformed logs, logs with more than
  four topics, non-32-byte topics, digest-only matches, bad bloom lengths,
  wrong emitters, and invalid typed-prefix byte `0x00`, while allowing
  unrelated valid `LOG0` entries that do not satisfy the SCCP source-event ABI.
  Placeholder
  structural fixtures may still use the typed EVM-family receipt-root MPT
  envelope carrying the SCCP receipt/message root. The ETH source adapter now
  also verifies an embedded beacon sync-committee certificate by deriving the
  ordered BLS committee trust-anchor hash, checking proof-of-possession values,
  recomputing the signed sync-committee message hash, verifying the aggregate BLS
	  signature, and enforcing strict `> 2/3` signed committee weight. ETH adapter
	  proofs now also carry raw execution-header RLP; the verifier Keccak-hashes it
	  to the claimed execution block hash, parses the RLP header fields, and checks
	  the block-number and receipts-root fields against the SCCP finality height and
	  adapter execution receipts root. ETH sync-committee transition structural
	  admission now also decodes the next-committee payload, requires parent-roster,
	  next-committee, and payload-hash agreement, recomputes the transition message
	  hash, checks the nested sync-committee message hash, and checks the
	  transition signature-hash transcript before BLS transition-chain work. The BSC
	  source adapter also verifies an embedded secp256k1 validator-set commit-seal
	  certificate by deriving the
  validator-set trust-anchor hash from validator addresses and powers,
  recovering signed validators from 65-byte seals over the BSC commit-message
  hash, binding the seal hash into the adapter transcript, and enforcing strict
  `> 2/3` signed power. BSC active receipt proofs now enforce the Parlia mainnet 200-block
  epoch window, and validator-set transitions must advance exactly one epoch on
  that epoch's start block. The Rust and JavaScript, Python, Swift, Kotlin, and
  Java Android transition-message helpers reject non-BSC source domains,
  non-adjacent validator epochs, and transition blocks that are not exactly
  `to_validator_epoch * 200` before deriving a signed Parlia transcript. BSC
  validator-set transition proofs now carry a
  canonical next-set payload, hash it under
  `sccp:bsc:validator-set-payload:v1`, decode the address/power list, and
  require the decoded payload to derive the advertised next validator-set hash.
  BSC transition structural admission now also rejects malformed transition
  envelopes before verifier work when the transition is not V1/BSC, does not
  advance exactly one validator epoch, does not use the Parlia epoch-start block
  for `to_validator_epoch`, carries empty header/payload material, carries zero
  transition hashes, or embeds a seal commit-message hash that does not match
  the transition message hash. The same BSC adapter preflight now also
  recomputes the next-validator payload hash, payload-derived next-set hash,
  transition-header payload binding, ValidatorSet metadata proof hash,
  transition message hash, and transition seal hash, then requires non-empty
  transition chains to be internally adjacent and terminate at the adapter's
  declared active epoch and validator-set hash.
  The nested ValidatorSet metadata/storage proof preflight now also rejects
  non-V1 metadata, non-mainnet ValidatorSet contracts, wrong length slots, zero
  storage roots or value/metadata hashes, empty length/storage proof material,
  non-canonical per-validator storage slots, and storage-value hash drift before
  MPT metadata verification runs.
  BSC transitions now also prove mainnet ValidatorSet storage parity by opening
  the `0x0000000000000000000000000000000000001000` account under the transition
  header state root, verifying its storage root, and opening
  `currentValidatorSet.length` plus each carried validator
  `currentValidatorSet[index].consensusAddress` slot before the next set can
  activate. The offline
  `scripts/sccp_bsc_source_bridge_evidence.py` helper now renders the governed
  BSC -> SORA source material and source-adapter deployment TOML from live
  validator-set, verifier, source bridge, adapter verifier, and deployment
  receipt hashes, while rejecting non-production lanes and zero evidence. BSC
  still needs recursive verifier deployment before it is a complete
  light-client engine.
  ETH now derives the Deneb/Fulu SSZ `ExecutionPayloadHeader` root from the
  execution RLP header, opens the fixed beacon-body execution-payload branch,
  and recomputes the signed `BeaconBlockHeader` root before accepting the
  finalized root. The offline `scripts/sccp_eth_source_bridge_evidence.py`
  helper now renders the governed ETH -> SORA source material and
  source-adapter deployment TOML from live beacon, verifier, source bridge,
  adapter verifier, and deployment receipt hashes, while rejecting
  non-production lanes and zero evidence. ETH still needs recursive verifier
  deployment and any production light-client update/state branches not
  discharged inside that deployed source-adapter circuit. The ETH/BSC,
  Solana, TON, TRON, and
  binding the planned backend, finality policy, and inclusion-proof layout, and
  the component hashes must be deployment-supplied rather than the built-in
  template hashes; generic source material remains rejected. Destination
  rollout readiness is now likewise profile-bound for all advertised domains:
  generic anchor metadata,
  cross-chain verifier identities, malformed addresses, and zero verifier
  addresses fail closed, and TON destination rollout helpers now pin raw
  contract identities to the exact mainnet anchor id. The EVM destination
  evidence helper now renders exact ETH/BSC destination rollout and route
  allowlist records while recomputing the wrapper-bound destination binding
  hash, closing the hand-assembled EVM destination rollout tooling gap. The
  BSC mainnet SDK facades across Rust, JavaScript, Python, Swift, Kotlin/JVM,
  Java Android, and .NET now also pin chain id `56` and the governed deployment
  binding before request, prebuilt-result wrapping, proof-job, or submission
  packaging. Python
  now exposes the easy `BscMainnetSccp` outbound and inbound facade directly,
  including BSC receipt/block collection, Parlia finality preservation,
  native-prover execution, copied proof-byte submission, and
  `BscMainnetSccpProver` as a compatibility wrapper for older prover-only
  callers. The
  JavaScript package-root `BscMainnetSccp` facade now additionally validates
  canonical `eth_chainId == 0x38`, rejects failed or drifted BSC receipt/block
  evidence before app-linked proving, and builds BSC verifier calldata only
  from wrapped proof results carrying the governed destination binding. Its
  inbound prove/submit helpers now require full BSC receipt-proof material
  before app-linked source proving, reject hash-only proof commitments before
  the prover callback runs, reject empty/all-zero proof bytes, and copy the
  accepted bytes before calling the app-linked Iroha submitter. Python now
  mirrors that full-receipt-proof callback guard while still permitting
  hash-only BSC receipt-proof evidence for collection diagnostics. Swift,
  Kotlin/JVM, Java Android, and .NET now carry typed BSC `receiptProof`
  transcripts, derive and conflict-check `receiptProofHash`, and reject
  hash-only BSC proof input before local prover callbacks. The browser,
  Python, Swift, Kotlin/JVM, Java Android, and .NET BSC inbound facades now
  also derive SCCP source-event evidence from BSC receipt logs, bind it to
  `receiptProof.sourceEventDigest`, and reject full receipt-proof input before
  local prover callbacks when source-event validation is missing or drifted.
  The browser and native ETH/BSC inbound facades also require
  positive canonical `receipt.blockNumber` and `block.number` values whenever
  receipt/block evidence is collected, closing the last optional block-number
  ambiguity in the easy SDK path. The `eth,bsc` public release row now also
  requires the `dotnet-sdk` corridor phase, which runs the native C# Ethereum
  and BSC facade tests before release evidence can pass. The .NET SDK also
  exposes matching BSC-mainnet chain-id, network-id, route, native inbound
  prove/submit, and destination-binding hash guards, and the BSC outbound
  wrapper regressions now cover Rust/core, JavaScript, Python, Swift, and .NET
  paths proving manually forged prebuilt proof requests with mismatched
  `destinationBindingHash`/`DestinationBindingHash` values are rejected before
  generic proof wrapping or verifier calldata can be produced;
  the remaining BSC destination work is live deployment evidence rather than
  hand-rolled SDK chain-id guards. Route
  allowlist readiness is now profile-bound as well: every advertised counterparty requires the exact
  governed route allowlist id plus a non-zero policy hash, while missing,
  generic, malformed, or cross-domain allowlist material remains rejected.
  A combined lane-readiness helper now evaluates source verifier material,
  source-adapter deployment evidence, destination rollout material, and route
  allowlist material together, so operator tooling can prove an individual
  lane's local production readiness while still rejecting cross-domain replay
  of any component. Source-adapter OpenVerify verification now fails closed when
  the lane-specific verifier-key commitment cannot be reconstructed, and
  regression coverage rejects wrong verifier-key hashes, backend tags, schema
  descriptors, auxiliary envelope data, public-input columns, and backend
  proof bytes across the typed source-adapter variants.
  Solana submission
  packages now also carry the statement hash, destination binding hash, and
  proof context hash alongside proof bytes, public inputs, and bundle bytes, so
  verifier-program submissions are
  replay-scoped to the manifest deployment context. Solana SDK proof requests
  require the same statement hash and destination binding hash as a proof
  context, and wrapped UI-prover results commit to that proof-context hash as
  well as the source witness hash. JavaScript, Python, and Swift now expose
  explicit proof-result wrappers for externally generated UI prover bytes
  Kotlin and Java Android expose the same flow through public
  `wrapProofResult` helpers. Those direct wrappers bind proof bytes to the
  canonical request before deriving request-bound envelope hashes; Solana also
  rebuilds the canonical request from witness/context before accepting
  externally generated proof bytes. Those SDK proof request surfaces now also
  carry the configured source-adapter deployment hash, deployment receipt hash,
  and canonical deployment-binding hash in prover public inputs and wrapped
  proof results, while leaving the Solana verifier-program
  `proof_context_hash` tied only to statement and destination binding for
  submission compatibility. JavaScript, Python, Swift, Kotlin, and Java Android
  Solana submission builders now require both explicit transparent SCCP message
  public inputs and a wrapped SDK `proofResult` before wallet/RPC packaging;
  wrapped `proofResult.publicInputs` are source-proof inputs and are not
  accepted as a substitute for transparent submission inputs. The JavaScript
  distributable and TypeScript declarations now require that wrapped result as
  well, so browser portal code cannot compile against the raw proof-byte-only
  path. JavaScript, Python, Swift, Kotlin, and Java Android Solana source-state
  proof capsule wrappers now also require a full SDK-built AccountsLtHash or full-light audit
  OpenVerify/FastPQ request shape before wrapping externally generated proof
  bytes, including the Solana source domain, canonical FastPQ parameter set,
  deployed AccountsDB verifier id/hash, AccountsLtHash direct-parent and
  residual hashes, full-light audit role metadata, and the matching
  OpenVerify public-input columns. The TypeScript declaration
  requires the full request union instead of a minimal circuit-id object.
  Swift, Kotlin, and Java Android typed wrapper overloads now reject
  hand-built request values unless the SDK-built statement/context/schema
  bytes, public-input columns, FastPQ public inputs, and transitions are
  present before source-state proof bytes are wrapped. Those
  builders
  also require the wrapped result backend, proof-context hash, non-zero envelope
  hash, deployment-binding hash, source-state verifier id/hash, submitted proof
  bytes, and source-proof statement/destination binding to match the submission
  context. The JavaScript, Python, Swift, Kotlin, and Java
  Android Solana prover facades now refuse to invoke app-linked provers or wrap
  proof bytes unless that request is bound to the production AccountsDB
  source-state verifier id plus non-zero source-state verifier, deployment, and
  deployment-receipt hashes; diagnostic zero-binding request builders remain
  fixture-only. Those Solana SDK surfaces now also build the
  nested AccountsLtHash source-state proof request for UI/mobile provers,
  including statement bytes, opened-account commitment bytes, verification
  context, OpenVerify schema descriptor, mainnet-genesis public-input binding,
  public-input columns, and FastPQ
  transition payloads. The JavaScript, Python, Swift, Kotlin, and Java
  Android SDKs now also build the matching `borsh_instruction_v1` Solana
  program-instruction envelope from UI-generated proof bytes, canonical
  transparent public inputs, SCCP bundle bytes, statement hash, destination
  binding hash, and proof-context hash, rejecting context-hash mismatches before
  wallet/RPC submission. JavaScript and Python also reject caller-supplied
  Solana `publicInputsBytes` when they do not equal the canonical transparent
  public inputs, while the mobile SDKs derive those bytes internally. The
  JavaScript and Python Solana submission builders now also reject explicit
  `null`/`None` values for `publicInputs`, `proofBytes`, `proofContext`,
  `statementHash`, and `proofContextHash` instead of treating them as omitted
  and falling back to wrapped proof-result metadata. Those submission builders
  now require `publicInputs.targetDomain = Solana`, matching
  the Rust SORA -> Solana verifier-program template, and also reject any
  `destinationBindingHash` other than the canonical SORA -> Solana binding
  hash. JavaScript
  now freezes the Solana local-prover
  request/result/submission objects and returns proof/instruction byte fields
  through defensive-copy getters, preventing browser code from mutating request
  hashes or packaged on-chain bytes after the app-linked prover runs. The
  callback-visible Solana witness snapshot also deep-freezes nested UI payload
  metadata and copies nested byte buffers before invoking the prover, so portal
  state cannot be mutated through the proof callback; the
  TypeScript declarations mark those Solana SDK objects as readonly for portal
  compile-time checks. Kotlin mobile now passes app-linked Solana proof engines
  a byte-array snapshot of the canonical request and wraps returned proof bytes
  against the original request hash, so callback-side array mutation cannot
  corrupt the submitted envelope binding. Python mirrors those Solana
  request/result/submission immutability guarantees with read-only
  dict/list-compatible envelopes for
  backend portal tooling. The JavaScript and Python Torii clients now preserve
  those Solana binding and context fields when decoding typed SCCP artifact/job
  responses, so the web portal and operator tooling can audit the exact
  deployment replay scope before preparing the wallet transaction. TON submission
  packages now carry a real `ton_message_body_boc_v1` message body BOC, with
  proof bytes, public inputs, SCCP bundle bytes, destination binding, and
  statement hash bound into the TON internal-message payload; the Python SDK now
  exposes the same BOC and read-only submission-envelope builders as the web and
  mobile SDKs for portal/backend packaging. JavaScript and Python TON
  submission builders plus the Swift, Kotlin, and Java Android TON message-body
  constructors now also accept wrapped request-bound proof results directly,
  rechecking proof bytes, transparent public inputs, request hash,
  source-adapter deployment-binding hash, envelope hash, statement hash,
  destination binding, and proof context before constructing the BOC payload;
  the dynamic JavaScript/Python path and the Swift/Kotlin/Java mobile typed
  inputs now require that wrapped proof result instead of accepting standalone
  raw proof bytes.
  The TON source
  adapter now derives `shard_proof_hash` from the source event digest,
  masterchain seqno/block hash, shard block hash, shard state root,
  transaction root, and inclusion branch, so the masterchain/shard witness can
  no longer be any non-zero placeholder hash. The signed TON masterchain
  block-message transcript now also binds the masterchain `BlockIdExt`
  workchain id `-1`, masterchain shard `0x8000000000000000`, root hash, and a
  non-zero file hash across Rust plus JavaScript, Python, Swift, Kotlin, and
  Java Android helpers, so a basechain or file-hash-free block id cannot be
  signed as masterchain finality. TON SDK proof requests now also
  bind the SCCP statement hash, destination binding hash, source-state verifier
  id/hash, source-adapter deployment hash, deployment receipt hash, and
  canonical deployment-binding hash into the user-side request/envelope hash,
  length-prefix bundle/source-proof bytes before request hashing, and now reject TON proof
  requests whose backend is not `ton-contract-v1` or whose source-adapter
  deployment binding is zero/zero, so mobile, web, and Python portal provers do
  not generate deployment-agnostic or non-contract TON proof bytes. That
  deployment binding is now fixed to the governed TON -> SORA source lane across
  JavaScript, Python, Swift, Kotlin, and Java Android, and JS/Python reject
  nested binding inputs that try to supply a non-SORA target domain. The
  Python Torii client now also builds the deployment-bound TON local proof
  request/result envelope and decodes the production `ton_message_body_boc_v1`
  platform payload with `message_body_boc`, query id, destination binding, proof
  bytes, public-input bytes, bundle bytes, and statement hash, matching the
  JavaScript portal surface. The TON source adapter now also verifies an
  embedded masterchain validator-signature certificate by deriving the ordered
  Ed25519 validator-set trust-anchor hash, recomputing the signed masterchain
  block-message hash, checking the signature-capsule hash, verifying Ed25519
  validator signatures, and enforcing strict `> 2/3` signed validator weight.
  TON validator-set transition proofs now derive the active validator set from a
  configured parent trust anchor by binding the parent set, canonical next
  validator-set payload hash, payload-derived next set, next-set config hash,
  masterchain block, and seqno range, then requiring a strict `> 2/3`
  parent-set Ed25519 signature capsule. The transition chain now also requires
  adjacent validator-set seqnos and strictly increasing masterchain transition
  seqnos, preventing skipped TON validator updates and out-of-order transition
  replay. TON validator-set payloads/signature proofs are capped at 1024
  validators, reject all-zero Ed25519 validator keys, and ordered
  validator-set transition chains plus shard-state/config source Merkle
  branches are capped at 64 entries before source-adapter evidence hashing. TON
  signature-proof transcript builders now also require signer bitmap padding,
  signature count, claimed total/signed weights, and the strict `> 2/3`
  signed-weight threshold to agree before serialization, and
  transition-signature builders reject parent validator-set hash or
  transition-message hash mismatches before proof submission.
  The TRON source adapter now treats bounded transaction source proofs as the
  production path: it hashes `transaction_bytes`, verifies the java-tron
  transaction Merkle branch against the signed-header `txTrieRoot`/adapter
  `transaction_root`, parses one successful `TriggerSmartContract` transaction
  with exactly one canonical recoverable secp256k1 signature over
  `sha256(raw_data)` from the configured owner, and requires calldata
  `keccak256("submitSccpSourceEvent(uint32,uint32,bytes32)")[0..4] ||
  abi_word_u32(source_domain) || abi_word_u32(target_domain) ||
  source_event_digest` to the governed source bridge contract. The adapter derives
  `receipt_proof_hash` from the transaction-source transcript binding the source
  digest, receipt root, transaction root, transaction index/count, transaction
  bytes, transaction Merkle branch, and source inclusion branch only after
  recomputing the java-tron transaction Merkle root from those transaction bytes
  and branch. The Rust public transcript helper now fails closed unless the
  same transaction bytes satisfy the successful governed source-call verifier,
  and JavaScript, Python, Swift, Kotlin, and Java Android helper surfaces reject
  malformed or non-source-call TRON transactions before UI/mobile prover
  transcript hashing. Bounded MPT
  `receipt_trie_proof_nodes` remain a legacy structural transcript only, where a
  proven `TransactionInfo` value may carry exactly one successful result field
  plus the SCCP source-event ABI topic, `source_event_digest`, and empty event
  data; unknown fields inside each parsed legacy log fail closed. Placeholder
  fixtures may still use the bounded typed RLP
  `sccp:tron:receipt-root-value:v1` envelope; legacy exact 32-byte roots are
  rejected. The DPoS receipt
  proof witness can no longer be any non-zero
  placeholder hash. The shared MPT verifier now handles canonical inline child
  nodes by traversing the raw embedded RLP node and rejects duplicate unused
  inline proof entries. JavaScript, Python, Swift, Kotlin, and Java Android SDK
  helpers now derive the same receipt-state MPT transcript and typed
  receipt-root MPT value envelope so portal and mobile provers do not need to
  hand-roll those encodings. The same TRON receipt-proof and receipt-state
  helpers reject zero source-event digests, receipt roots, and transaction roots
  before deriving transcript hashes; the typed TRON receipt-root MPT value
  helpers reject zero receipt roots as well. Rust and SDK TRON receipt-proof,
  receipt-state, and transaction-source transcript helpers also require a
  non-empty SCCP source inclusion branch before hashing, matching source
  envelope admission. Rust TRON solid-block message
  transcripts also reject wrong source domains, zero block heights, and zero
  block/schedule/receipt/transaction/proof hashes before deriving the signed
  message, while witness-seal and witness-schedule-transition seal hashes now
  require internally valid signed certificates, strict `> 2/3` witness weight,
  message binding, and next-schedule payload/hash consistency before hashing.
  It also verifies a
  stake/weight-style TRON witness seal: the adapter derives the witness schedule
  hash from 21-byte TRON addresses and weights, binds it to the solid-block
  message hash, recovers secp256k1 signers to TRON addresses, checks the
  configured witness-schedule trust anchor, and enforces strict `> 2/3` signed
  witness weight. TRON header and witness recoverable secp256k1 signatures now
  accept java-tron's raw recovery ids plus normalized `27..=30` fixture ids,
  reject invalid `r` scalars, high-S malleable encodings, and out-of-range
  recovery ids before proof acceptance, and require solid-block header proof
  hashes to recover both child and parent signatures to their declared TRON
  witnesses. Witness schedule
  rosters are capped at 64 unique addresses in
  the verifier and SDK payload helpers, and canonical schedule payload builders
  reject non-zero per-witness weights whose sum cannot fit the `u64`
  `totalWeight` committed by witness seals. TRON solid-block header proofs now
  parse the raw `BlockHeader.raw_data` bytes, verify the SHA-256 raw-data hash
  and block-number-derived TRON block id, allow only java-tron's known
  solid-header fields plus optional `witness_id`, bind `txTrieRoot` to the
  adapter `transaction_root`, prove the immediate signed parent header link, and
  recover child, parent, and bounded ancestor block producers' secp256k1 header
  signatures to active 21-byte TRON witness addresses. The ancestor chain is
  capped at 64 signed headers, linked by parent block ids, and required to step
  backward by one height with strictly decreasing timestamps. TRON solid-block
  confirmation headers are also capped at 64 signed descendants, linked forward
  from the solid block id, and non-placeholder material requires unique active
  witness producers in that confirmation chain to carry more than two thirds of
  the active witness schedule weight before the block is treated as solid.
  JavaScript, Python, Swift, Kotlin, and Java Android source-proof helpers now
  reject zero or out-of-range `r`, high-S, zero-S, and out-of-range recovery-id
  child/parent header signatures before local TRON solid-block header proof
  hashing, matching the Rust verifier's first-pass signature canonicalization.
  The same Rust and SDK
  transcript helpers reject all-zero `0x41`-prefixed TRON witness addresses in
  raw block headers, solid-block header proofs, and witness-schedule payloads
  before deriving hashes, so zero witness placeholders cannot satisfy local or
  on-chain preflight.
  TRON witness-schedule transition proofs now derive the active
  schedule from a configured parent trust anchor by binding the parent schedule,
  canonical next witness-schedule payload hash, payload-derived next schedule,
  transition block, and schedule-epoch range, then requiring a strict `> 2/3`
  parent-schedule secp256k1 seal. Multi-step TRON transition chains must be
  bounded to at most 64 hops, epoch-contiguous, strictly increasing by
  transition block number, and anchored to the supplied solid, parent, or signed
  ancestor header evidence. The Rust transition-message and transition-seal
  helpers now reject non-TRON domains, skipped schedule epochs, zero transition
  blocks, zero transcript hashes, and stale transition-message hashes before
  signature verification. TRON source-adapter material verification and the
  TRON DPoS verifier helper now preflight bounded adapter shape before
  transcript/evidence hashing, so oversized or mixed legacy branches, MPT nodes,
  wrong adapter domains, zero block/root/seal/proof hashes, empty witness
  rosters, non-canonical signer bitmaps, mismatched witness weights/signature
	  counts, insufficient signed witness weight, all-zero TRON witness addresses,
	  truncated or non-canonical header/witness signatures, stale
	  transition-domain/message/seal metadata, transition chains, or transition
	  payloads fail before canonical adapter bytes are serialized. Transition
	  preflight now decodes the next witness-schedule payload as the canonical
	  `sccp:tron:witness-schedule:v1` address/weight roster, binds the
	  parent-schedule hash, payload hash, payload-derived next-schedule hash, and
	  transition message hash, and rejects non-contiguous, non-monotonic, or
	  wrong-final-schedule transition chains before transition-step verifier work.
	  The generic TRON source-adapter binding path now also recomputes the witness
  schedule hash, solid-block message hash, and witness seal hash, so swapped
  schedule/seal transcripts fail before recursive verifier material is
  evaluated.
  derive `storage_proof_hash` from the source event digest, finalized block
  number, finality set id, authority set hash, events root, source-event leaf
  index, canonical runtime events storage key, and inclusion branch
  instead of accepting a placeholder storage proof hash. They also verify an
  embedded finality authority certificate by deriving the ordered Ed25519
  authority-set trust-anchor hash, recomputing the finalized precommit-message
  hash, checking the justification hash, verifying Ed25519 signatures, and
  transition proofs now derive the active authority set from a configured parent
  trust anchor by binding the parent set, canonical next authority-set payload
  hash, payload-derived next set, transition block, and finality set-id range,
  then requiring a strict `> 2/3` parent-set finality justification.
  templates now bind
  those validator/vote/witness certificate transcript prefixes as well as their
  inclusion-proof and TRON receipt-state prefixes, closing the inclusion-only
  deployment-hash gap for those lanes. BSC source proofs also support an
  ordered validator-set
  transition chain from a configured parent trust anchor into the active
  validator set, with strictly increasing transition block numbers, raw
  transition header RLP checked against the transition block hash, and Parlia
  `extraData` extraction required to match the signed next validator-set payload
  plus the proven ValidatorSet account/storage metadata proof available through
  web, Python, Swift, Kotlin, and Java Android SDK proof-generation helpers, ETH
  source proofs now support an ordered sync-committee
  transition chain from a configured parent committee into the active committee,
  with the canonical next sync-committee payload available through the web,
  Python, Swift, Kotlin, and Java Android SDK proof-generation helpers,
  TRON source proofs now support an ordered witness-schedule transition chain
  from a configured parent schedule into the active schedule with the next
  witness-schedule payload available through the web, Python, Swift, Kotlin,
  and Java Android SDK proof-generation helpers and a 64-hop verifier cap, and
  TON source proofs now support ordered payload-derived validator-set
  transition chains with the next validator-set payload available through the
  same web, Python, Swift, Kotlin, and Java Android SDK proof-generation
  helpers, a 1024-validator payload/signature cap, zero validator-key
  rejection, signer bitmap/weight preflight, parent/message hash binding, and a
  64-hop transition-chain cap plus a 64-entry cap on shard-state/config source
  Merkle branches. TON masterchain config proof transcripts now bind the active
  validator-set payload hash, TON config parameter `34`, the opened config
  value hash, a bounded TON `HashmapE 32 ^Cell` dictionary proof BoC, and the
  decoded `validators#11`/`validators_ext#12` config-34 payload into the signed
  block-message hash; legacy abstract config inclusion branches must be empty.
  The signed TON masterchain
  block-message now carries the mainchain `BlockIdExt`
  workchain/shard/root/file-hash tuple, SDKs expose the
  signed TON masterchain block-message and
  validator-signature transcript helpers, TON shard-proof transcripts now bind
  and verify a shard-state opening from the message root into the signed shard
  state root, the Rust verifier plus JavaScript, Python, Swift, Kotlin, and
  Java Android SDKs can derive bounded complete TON BoC root hashes for UI proof
  material including CRC32C-checked BoCs, strict partial-byte cell padding
  checks, and pruned-branch/Merkle proof/Merkle update exotic cell hash-depth
  semantics including legacy maskless pruned-branch proof cells emitted by
  existing TON tooling, the same Rust verifier plus JavaScript, Python, Swift,
  Kotlin, and Java Android SDKs can now derive both generic `HashmapE n ^Cell`
  value-cell hashes and selected `ShardAccount.last_trans_hash` /
  `last_trans_lt` identities from bounded TON dictionary proof BoCs while
  failing closed on pruned selected paths and non-256-bit `ShardAccounts`
  account keys, and the TON source
  adapter can bind an optional shard-state
  `ShardStateUnsplit` proof BoC plus ShardAccounts root/key/proof BoC opening
  into the shard-proof transcript, extract the `accounts:^ShardAccounts`
  reference hash, validate the embedded `ShardIdent` constructor and
  `shard_pfx_bits <= 60` bound, require TON mainnet `global_id = -239`,
  require TON basechain `workchain_id = 0`, require the selected 256-bit
  account key to match the proven shard prefix, decode shard-state `seq_no`,
  `gen_utime`, `gen_lt`, and `min_ref_mc_seqno`, reject zero
  sequence/generation/logical-time placeholders, reject MasterChain-only
  `custom:(Maybe ^McStateExtra)` refs on basechain shard states, require
  `min_ref_mc_seqno` not to exceed the signed masterchain seqno, require the
  legacy shard-state branch to be empty in dictionary mode, and verify the selected
  `ShardAccount.last_trans_hash` and `last_trans_lt` before accepting the
  signed shard-state root; the same JavaScript, Python,
  Swift, Kotlin, and Java Android SDKs now expose local `ShardStateUnsplit`
  proof-root, accounts-root, and selected-account helpers for UI prover preflight
  and reject dictionary-backed shard-proof inputs whose shard-state branch is
  non-empty, whose selected account key is not a 256-bit TON account id, whose
  ShardIdent shape is malformed, whose shard-state proof is not from TON
  mainnet `global_id = -239`, whose proven `ShardIdent` is not from TON
  basechain `workchain_id = 0`, whose selected account key prefix does not
  match the proven shard prefix, whose shard-state `seq_no`, `gen_utime`, or
  `gen_lt` is zero, whose basechain `ShardStateUnsplit` carries a
  MasterChain-only `custom` ref, whose `min_ref_mc_seqno` is ahead of the signed
  masterchain seqno, or whose shard-state root, accounts root, or selected
  account last transaction hash/logical time do not match the
  submitted transcript fields, and TON source proofs now carry a
  `shard_state_verification_proof` OpenVerify/FastPQ source-state capsule that
  is mandatory when deployed TON source-state verifier material is configured,
  with JavaScript, Python, Swift, Kotlin, and Java Android TON full-light audit
  request builders deriving `shard_state_verification_proof_hash` from that
  capsule instead of accepting a hash-only stand-in,
  binding the masterchain/shard/config/selected-account proof inputs to the
  advertised verifier hash before the adapter proof is accepted, with
  fail-closed coverage for wrong OpenVerify circuit ids, backend tags, schema
  descriptors, auxiliary data, public-input columns, backend proof bytes, and
  oversized source-adapter OpenVerify envelopes or source-state proof labels
  before decode, plus adapter verifier-commitment helper coverage for malformed
  outer wrappers, opaque proof bytes, zero verifier keys, auxiliary envelope
  data, and empty STARK public-input columns before metadata extraction;
  deployment-backed TON source-adapter readiness can now open when exact
  non-placeholder source verifier material, matching source-state verifier
  deployment fields, adapter verifier commitment, and deployment receipt hashes
  are present;
  chain from a configured parent set into the active set, with the next
  authority-set payload and transition transcript hashes now available through
  the web, Python, Swift, Kotlin, and Java Android SDK proof-generation helpers.
  Torii's SCCP message proof, runtime proof envelope, proof artifact, proof job,
  and recent-message read paths now recover non-SORA bundles from verified
  on-chain bridge proof records, enforcing typed artifact backend/manifest
  binding, stored proof-range/finality-height agreement, and current production
  source-lane proof validation before serving the user-submitted source proof.
  The all-lanes preflight and public release-bundle verification now also reject
  source-adapter gate audit hashes that replay source material, source-adapter
  deployment, destination binding, route allowlist, route canary evidence, or
  sibling audit hash roles, and required source-gate blockers are promoted into
  the lane-level preflight blockers. Built-in SCCP source-verifier material is
  now explicitly template-only: production readiness stays fail-closed unless
  caller-supplied governed material and a matching source-adapter deployment
  descriptor are both present, and adversarial tests prove template material
  cannot be promoted by wrapping it in a matching-looking deployment record.
  Template source-verifier component hashing and source-chain proof-envelope
  shape checks now also reject unmapped source domains instead of falling back
  to empty chain keys.
  The all-lanes release summary now also publishes the supported launch-domain
  set and the unsupported diagnostic-domain set as separate verified fields, so
  release tooling can reject launch-scope tampering instead of inferring scope
  only from lane blockers. The direct all-lanes validator must also convert
  malformed evidence roots or non-string section keys into structured blockers
  instead of raising before lane blockers can be emitted. The readiness source
  inventory now also includes
  `launch_scope_constant_gate`, `retired_network_surface_gate`, and
  `unready_transparent_proof_config_gate`, backed by the same strict source
  marker scans as public release-bundle verification, so the active launch-policy
  constants, supported launch-domain set, launch-scope no-support note,
  exact specific no-support sentence, active-tree scan, and config-owned
  diagnostic transparent-proof toggle must remain present before production
  reports can pass. Public native EVM SDK
  path-marker denylist strings are assembled from split literals so the
  no-WASM/no-remote source inventory can keep catching actual forbidden
  dependency tokens without flagging the guard implementation itself. The
  Ethereum launch-policy documentation inventory now also rejects stale
  BSC-only production-packaging wording, so artifact/job route docs cannot
  silently drift back to the superseded BSC-first policy. Release-readiness
  reports now publish that documentation inventory as a required source gate,
  so production readiness fails before bundle publication if public launch
  policy wording is missing or stale. Public discovery documentation now has
  the same readiness-level source gate, pinning supported-lane and verifier
  target wording before Torii discovery evidence can be published as
  production-ready. The direct all-lanes release checklist now also validates
  required source-adapter gate hashes and expected audit hash roles, rejects
  duplicate or governed-hash-replayed source-gate audit roles, and rejects
  forged source-gate material on lanes whose policy does not require a
  source-adapter gate. The active-launch governed-deployment and
  route-allowlist checklist items now reject source verifier material hashes
  that reuse the same canonical bytes32 value as the source-adapter deployment
  hash, keeping public readiness evidence role-separated before release-bundle
  construction; the active-launch checklist source inventory now pins that
  role-separation helper and both hash-reuse adversarial cases before release
  evidence can pass. Rust route-allowlist attachment plus EVM/TRON transaction,
  Solana ProgramData, and TON live-account canary transcript helpers now enforce
  route-allowlist/source-material/source-deployment/destination-binding hash
  separation on direct helper calls. Python operator evidence scripts mirror
  that separation before rendering route allowlists or canaries, and the
  all-lanes release-checklist source inventory pins those Rust, Python,
  JavaScript, Swift, Kotlin/JVM, and Java Android helper regressions before
  release evidence can pass.
- Keep live-network signing inputs runtime-only and continue using generated
  per-validator deployment bundles rather than hand-edited production configs.

**Next checkpoints:** continue replacing remaining SCCP source-chain verifier
placeholders behind the typed adapter variants so ETH/BSC/Solana/TON/TRON
consensus/finality and receipt/message inclusion are checked against external
chain rules. Shared source-state proof admission now rejects opaque nonzero
bytes and requires canonical STARK OpenVerify/FastPQ capsules before lane-local
verification work, and the Solana/TON source-state transcript hash helpers now
reuse the same canonical-envelope gate before audit-statement binding.
JavaScript, Python, Swift, Kotlin/JVM, and Java Android TON source-state proof
capsule evidence also rejects TON circuit-id capsules that advertise any proof
family other than `stark-fri-v1`, keeping debug or alternate source-state proof
bytes out of production transcripts. JavaScript source and checked-in package
dist canonicalizers now enforce that predicate before producing canonical TON
source-state proof bytes.
Source-adapter verifier commitment helpers use the same nested-FastPQ decoder,
so an otherwise canonical OpenVerify envelope with opaque backend bytes cannot
advertise a production verifier key, and the public proof-level helper now
verifies the adapter FastPQ proof and adapter transcript before returning that
commitment.
Deployment-bound transparent proof recovery is now typed for governed
source-material plus source-adapter deployment evidence. The strict helper keeps
the destination manifest gate closed until rollout, while the local-admission
variant relaxes only that manifest gate and remains fail-closed against backend
label drift or replayed deployment receipts.
Strict SCCP production package and proof-job builders now require non-SORA
source bundles to pass the production source-proof gate before destination
submissions or prover jobs can be constructed, leaving structural source-proof
fixtures behind explicit diagnostic `allow_unready` paths. Proof-byte job
builders also require the bundle-derived counterparty domain, manifest
counterparty domain, and supplied job counterparty domain to match even in
diagnostic mode, so callers cannot combine another lane's manifest with
otherwise valid proof bytes. Reusable transparent-statement, FastPQ proof-byte,
and submission-package builders now apply the same bundle-to-manifest
counterparty binding before deriving statements or relay envelopes, so inbound
SORA-target messages cannot be packaged under another remote lane's manifest.
Rust EVM/TRON Groth16 proof-request builders now reject non-canonical bundle
bytes, bundle/public-input mismatches, and omitted source-proof witness bytes
for non-SORA source bundles before local UI proving or wrapped proof-result
submission. The Rust TON native-recursive
proof-request builder and proof-result wrapper now apply the same canonical
SCCP bundle/public-input/source-proof gate before local proof generation or
wallet/liteserver packaging, and the JavaScript, Python, Swift, Kotlin/JVM, and
Java Android SDK TON request paths now mirror that gate before local prover
callbacks or wrapped-result submission.
Python EVM-family and TRON Groth16 request construction now mirrors the
canonical SCCP bundle/public-input gate as well: valid fixtures carry decoded
canonical bundle bytes, request hashes commit to those bytes, wrapped-result
tests use request-bound Groth16 proof tuples, and adversarial tests reject
arbitrary noncanonical bundle bytes, bundle/public-input mismatches, and
bundle source-domain drift before local prover callbacks or production wrapper
canonicality checks.
SORA-origin message bundle validation, public-input derivation, package
builders, proof-job builders, transparent-proof builders, and final
transparent-proof verification now reject source verifier material and
source-adapter deployment context before deriving public inputs, so external
source-adapter evidence cannot be spliced into Nexus-finality-backed outbound
bundles.
Material-only source-verifier evidence now has to keep both deployment fields
zero under the material-only verifier; deployment-looking hashes are accepted
only on the deployment-bound path that recomputes the configured
source-adapter deployment hash and receipt hash.
Transparent OpenVerify summary helpers now apply the same production-shaped
wrapper policy before reporting proof metadata, so metadata-only or aux-bearing
envelopes cannot be normalized into release/readiness summaries. The
artifact-level summary entry point now also validates the typed transparent
proof artifact wrapper first, so manifest, public-input, or submission-package
metadata drift cannot be hidden behind otherwise valid OpenVerify proof bytes,
and Torii/CLI artifact renderers omit OpenVerify summaries for wrappers that
fail that typed gate.
Keep any diagnostic/backlog-only helpers gated as non-production surfaces, with
production readiness and release-evidence summaries refusing to mark those
lanes ready. Transition verifiers now also require the ordered transition chain
to terminate at the adapter's declared active epoch/set id, so stale chains
cannot pass by replaying the same final validator or authority-set hash. ETH,
BSC, Solana,
non-canonical signer bitmap width/padding, empty signer sets, signature-count
drift, claimed stake/weight drift, and sub-quorum certificates before
transcript hashing. ETH source-adapter preflight also recomputes the active
sync-committee root, signed sync-committee message hash, and aggregate
signature transcript hash before deeper BLS verification. Solana
source-adapter preflight also recomputes the
vote-message hash from the adapter fields and finality-context hash before
deeper account/finality verification. For BSC, use the new offline
source-bridge evidence renderer, which now rejects BSC EVM-family template
component hashes before rendering governance TOML and also rejects
non-canonical BSC source-adapter
OpenVerify VK hashes, while BSC adapter structural admission rejects wrong
version/domain envelopes and zero block, receipt-root, validator-set,
commit-seal, or receipt-proof hashes, checks the Parlia epoch window, and
recomputes the validator-set hash, commit-message hash, and commit-seal
transcript hash before deeper verifier work. TON adapter
structural admission likewise rejects wrong version/domain envelopes, wrong
masterchain/basechain identifiers, zero chain sequence numbers, and zero
masterchain, shard, config, validator-set, transaction, signature, or proof
roots before BOC/config/signature verifiers run, and now recomputes the
validator-set hash, masterchain block-message hash, and masterchain signatures
transcript hash before deeper Ed25519 certificate verification. TON and TRON
transition-chain preflight now also rejects disconnected internal parent hashes
between self-consistent transition steps before deeper Ed25519 or secp256k1
verifier work. TRON source-adapter preflight also recomputes the witness schedule hash
from the declared witness roster, the solid-block message hash from adapter
roots, and the witness-seal transcript hash before deeper witness-signature
verification, and TRON transaction-info receipt admission rejects duplicate
matching SCCP source-event logs before MPT source-value checks can accept the
receipt. Release readiness and strict bundle source inventories now pin that
runtime guard so the duplicate-log rejection cannot be dropped from production
evidence. Use the EVM live source and
destination evidence collectors to query deployed source emitter, bridge, and
verifier views, verify
runtime code/key hashes, require the canonical RPC chain id, governed bridge
`networkId()`, reject verifier/bridge address aliasing, and require audited
source-emitter and bridge-wrapper code-hash pins before live TOML rendering,
and render all-lanes-ready rollout TOML, then finish
recursive verifier deployment
evidence for the current Parlia-header plus ValidatorSet-storage transition
chain. The EVM live/source-live helpers now reject padded CLI chain ids,
component hashes, JSON-RPC quantity/hex results, deployment receipt block
hash/number drift, missing or zero deployment block `receiptsRoot`, and
receipt-block source bytecode drift before rendering receipt, runtime-bytecode,
source, or destination metadata; the source-live collector also rejects
JSON-RPC success envelopes with a missing/padded protocol version or mismatched
response id. EVM live/source-live JSON-RPC errors now redact HTTP bodies,
transport reasons, duplicate key names, and error objects before public
diagnostics are emitted. The EVM live helper's
rendered TOML now preserves the observed RPC chain
id and bridge wrapper runtime code hash as metadata comments, and the all-lanes
preflight rejects ETH/BSC source material and destination rollout records that
lack that live bytecode evidence. For ETH,
use the new offline source-bridge evidence renderer, which now rejects Ethereum
EVM-family template component hashes before rendering governance TOML and also
rejects non-canonical ETH source-adapter OpenVerify VK hashes, and the EVM live
source/destination evidence collectors to collect governed source-emitter,
wrapper, and verifier hashes with the canonical RPC chain id, governed bridge
`networkId()`, and audited source-emitter/bridge-wrapper code-hash pins, then
finish recursive verifier deployment and cover any remaining production
light-client update/state branches not discharged inside that deployed source-adapter
circuit. The ETH/BSC adapter proof shapes are now preflight-bounded across Rust
and the web/mobile SDK helper surfaces: ETH caps sync committees at 512
authorities and 64 transition proofs, while BSC caps validator sets at 255
validators and 64 transition proofs before transcript hashing. Backlog-only
runtime-storage support must remain outside launch scope unless a future design
pass reopens it. If that happens, require the offline evidence renderer to
reject template component hashes, including the runtime storage-proof verifier
hash, and non-canonical source-adapter OpenVerify VK hashes before rendering
governance TOML using the same runtime-storage template preimage as Rust.
JavaScript, Python, Swift, Kotlin, and Java Android runtime-storage request
builders also reject that template source-state verifier hash before invoking
the app-linked prover, and
they derive the exact statement bytes, verification context, schema descriptor,
public-input columns, FastPQ public inputs, and metadata transitions from the
UI-collected `System.Events` storage proof witness. JavaScript and Python also
canonicalize the storage proof's source domain before merging flat or nested
source verifier material, while still rejecting nested material with duplicate
or mismatched source-domain aliases, so portal inputs can use the same flat
witness shape without bypassing material-domain checks. Use
the destination evidence renderer for governed material, runtime rollout, and
allowlist material, then extend the current finality
authority-certificate and authority-set transition checks, which are now
preflight-bounded to 2,048 authorities and 64 transition proofs and reject
all-zero authority keys across Rust and the web/mobile SDK proof helpers. The
storage-proof verifier material, matching source-adapter deployment evidence,
and a submitted `SccpSourceStateVerificationProofV1` OpenVerify/FastPQ capsule
whose circuit id, schema descriptor, public inputs, verifying-key hash, and
FastPQ proof verify against the governed runtime-storage verifier hash. The
source-evidence renderer now mirrors that requirement before production TOML:
the expected source material, source-adapter deployment, and runtime-storage
gate hashes must all be supplied and match before `toml_ready` can become true.
The Rust admission path no longer has a metadata-only fail-closed sentinel for
deployment evidence plus lane rollout across the all-lanes launch policy.
For
Solana, use the offline
`scripts/sccp_solana_source_state_evidence.py` source-state renderer and the
destination evidence renderer to collect governed deployment hashes, then
extend the current stake-weighted Ed25519 vote-certificate and finality-context
binding, whose adapter shape is now preflight-bounded before transcript hashing
for validator vectors, fixed-width vote/stake account raw-data witnesses,
AccountsLtHash proof capsules, signer bitmaps, Tower vote stacks, and hard-fork
data, into a full
mainnet-beta light-client verifier covering Tower BFT
vote-account/state replay beyond the bound 31-vote active post-root stack plus
rooted confirmation transcript, proving that the supplied bank AccountsLtHash
checksum is canonical for the full finalized-bank AccountsDB lattice hash,
replacing the current reference AccountsLtHash OpenVerify/FastPQ capsule with
the deployed full AccountsDB lattice verifier, and full Solana
bank-state/fork-choice rule evaluation beyond the bound Agave internal-state
hash. The source-state renderer now rejects padded component hashes and
source/target domain values before deriving source material, source-adapter
deployment records, or full-light-client gate hashes.
For TON, use the offline source-state and destination evidence renderers to
wire the live deployed full light-client/verifier contract evidence into
governance for the default catalog and keep the deployment-backed source
adapter gate covered by live receipt/material fixtures. The TON live
destination collector now treats `0x` values as exact 32-byte hex and otherwise
requires canonical strict base64/base64url for verifier code, account-state, and
last-transaction hashes, so malformed or non-canonical TON API hash text cannot
be normalized into rollout evidence. It also requires the returned `accountStates` account
address to be canonical and equal to the requested verifier contract before
pinning live hashes, rejects API URLs that carry credentials, params, queries,
or fragments, caps the `accountStates` JSON body and HTTP error details before
decoding, and requires runtime API keys to be exact ASCII tokens without
whitespace or control characters. It also rejects duplicate keys in the remote
JSON object graph instead of accepting last-value-wins parsing. The source-state
verifier id/hash request fields are now mirrored through the JavaScript,
Python, Swift, Kotlin, and Java Android portal/mobile SDK surfaces, and those
builders require the mainnet verifier id plus a non-zero verifier hash before
the UI/mobile prover runs. They now also reject the Rust/evidence-renderer
template-derived TON shard-state verifier hash in both generic source-material
audit helpers and TON shard-state request builders, so app-side proof
generation cannot promote profile-template material before on-chain
submission. JavaScript, Python, Swift, Kotlin, and Java Android
now also expose canonical source-adapter verifier-key commitment helpers for
mobile apps can display or audit the exact `adapter_verifier_vk_hash` accepted
by Rust admission and the offline governance evidence renderers. The Solana
  AccountsLtHash source-state request
  builders now expose the exact OpenVerify/FastPQ statement, schema,
  mainnet-genesis public-input binding, public-input columns, and transition
  payloads to web/mobile proof engines while rejecting
  the Rust template AccountsDB source-state verifier hash. The Solana
  Rust AccountsLtHash proof builder now applies the same production-ready
  source-state verifier predicate before packaging an OpenVerify/FastPQ capsule,
  so template-derived AccountsDB verifier hashes cannot be used even through
  direct helper calls. The Solana
  source-material template now commits to those AccountsLtHash OpenVerify/FastPQ
  constants plus the SCCP source-event leaf/node and Solana transaction-status
  leaf Merkle prefixes. SDK Solana transaction-status branch helpers now fold
  UI-collected inclusion siblings with that same SCCP source-node Blake2b hash,
  and Solana proof-result/submission builders reject all-zero proof bytes, so
  template drift or placeholder local prover output is caught before governed
  evidence rendering or on-chain submission. The same JavaScript, Python,
  Swift, Kotlin, and Java Android submission handoff also rejects wrapped proof
  results whose `publicInputs.sourceStateVerifierId` or
  `publicInputs.sourceStateVerifierHash` diverge from the top-level audited
  source verifier fields, so tampered UI/mobile proof-result metadata cannot
  present a different source-state verifier before wallet submission. They also
  pin wrapped proof-result, proof-context, source-adapter deployment-binding,
  and Solana transparent-public-input versions to `v1`, require `proofBase64`
  to match the proof bytes, require non-zero witness and proof-context hashes,
  and reject non-adjacent finalized/parent slots, zero bank signature counts, or
  zero bank/source-state hash fields in the wrapped source-proof public inputs
  before wallet submission. Rust
  counterparty submission
  package builders and transparent-proof structure verification now enforce the
  same non-empty, non-all-zero proof-byte preflight for Solana, TON, and
  emitted, and transparent inner-proof plus native recursive package builders
  reject any bundle whose transparent public-input target domain is outside the
  verifier manifest's local/counterparty lane endpoints. The offline Solana
source-state evidence renderer now uses the same `finalized-vote` template
prefix as Rust and carries a golden vector for the template-hash rejection path.
Its direct material and source-adapter deployment record hash helpers now apply
that same template-hash rejection, so programmatic rollout tooling cannot
derive production-looking Solana evidence hashes from profile-template source
components. They also reject zero component, adapter verifier-key, deployment
receipt, and full-light-client audit hashes outside the CLI parser path, so
embedded rollout tooling cannot derive governed Solana records from empty
cryptographic evidence. Rust Solana source-material constructors now also
reject all-zero or template-derived AccountsDB source-state verifier hashes
before returning deployment-shaped material. Programmatic Solana source-state
evidence now also
requires exact `u32` source/target domain values, so boolean placeholders cannot
coerce to SORA domain `0` before material or deployment hashes are derived. It
also rejects non-canonical source-adapter OpenVerify
VK hashes before rendering governance TOML, so Solana operator evidence now
fails locally when it would fail Rust deployment matching. The compact JSON summary path now
uses the same Solana validation as the TOML renderer, so programmatic rollout
tooling cannot report partial Tower replay, full AccountsDB lattice, or
bank/fork-choice audit material as a valid dry-run. It now also reports
`source_verifier_material_ready`, `source_adapter_engine_deployment_ready`,
`source_adapter_gate_ready_with_full_light_client_evidence`,
`source_adapter_gate_blockers`, and `full_toml_ready`, so CI can gate source
pins separately from the complete source TOML predicate and report the exact
missing gate evidence. It now also accepts the
remaining Tower replay, full AccountsDB lattice, and bank/fork-choice verifier
hashes as an all-or-nothing audit bundle and emits a deterministic
`sccp:solana:full-light-client-gate:v1` hash over that bundle plus the
canonical source-material and deployment record hashes; production TOML rendering
now requires that complete audit bundle plus an independently supplied
`--expected-full-light-client-gate-hash`, so governed Solana audit records cannot
be staged from a self-derived gate hash alone or without deployed full-light
client evidence. When supplied, those hashes are now emitted as
source-adapter deployment config fields, appended to the canonical
`SccpSourceAdapterEngineDeploymentV1` record hash for Solana deployments,
  committed by the node ZK consensus policy hash, and recomputed by configured
  source-adapter parsing. The derived gate hash remains config/policy evidence
  instead of a deployment-record field to avoid circular hashing. `iroha_sccp`
  exposes the same transcript helper with a golden vector and now keeps the
  helper explicitly tied to the production Solana AccountsDB source-state
  verifier profile. The public source
  proof deployment-match helper now also requires the proof's embedded verifier
  evidence to carry the exact deployment record hash and deployment receipt hash,
  and re-verifies the adapter OpenVerify proof against that evidence so
  post-construction evidence splices or audited-bundle replays cannot be
  misreported as deployment-bound.
  JavaScript, Python,
  Swift, Kotlin, and Java Android now mirror the same Solana audit suffix in
  their canonical source-adapter deployment hash helpers, preserving the
no-audit hash while rejecting partial or non-Solana audit bundles before
UI-generated proofs submit on-chain. The same SDKs now also expose the
audited Solana full-light-client gate hash with the Rust golden vector, so
portals and mobile apps can audit the governed Tower replay, AccountsDB
lattice, and bank/fork-choice verifier bundle locally; those helpers also
reject duplicate audit verifier hashes and hashes that reuse existing
source-adapter material or built-in Solana template source-material component
hashes, and the Swift/Kotlin/Java Android gate-hash helpers now rerun that
check directly before returning a commitment. Rust admission and the offline
Solana source-state evidence renderer apply the same check before deriving the
full-light-client gate hash, so the governed audit suffix cannot be staged from
template verifier material through core, CLI, web, Python, Swift, Kotlin, or
Java Android callers. The ETH, BSC, Solana,
TON, and
`SccpSourceVerifierMaterialV1` and
`SccpSourceAdapterEngineDeploymentV1` record hashes in compact JSON dry-runs
and TOML audit comments, matching `iroha_sccp` helper vectors before governed
evidence is copied into node configuration. Those renderers can now require
operator-supplied expected material/deployment record hashes before JSON or TOML
is emitted, so governance rollout scripts can fail on digest drift before
configuration is staged. Their direct material and source-adapter deployment
record hash helpers now apply the same template-hash rejection as the TOML
renderers, so programmatic rollout tooling cannot derive production-looking
helpers now also require exact `u32` domain ids on their programmatic paths, so
Python boolean placeholders cannot be staged as SORA target-domain evidence.
The Solana submission helpers now require UI/mobile wrapped proof results to
match the explicit transparent SCCP message public inputs before wallet/RPC
submission, including message id, payload hash, commitment root, finalized slot,
and bank hash. They also re-derive the embedded Solana -> SORA source-adapter
deployment binding hash and require the source-proof public inputs to echo the
same deployment hash, receipt hash, and binding hash. They also recompute the
wrapped proof-result envelope hash and reject proof-byte overrides, so a valid
local proof cannot be paired with a different source-adapter deployment,
different on-chain message envelope, or different proof bytes through the SDK
convenience path. Swift, Kotlin/JVM, and Java Android now compare Solana
submission proof-context hashes, destination-binding hashes, transparent public
inputs, and message public inputs after canonical hex/slot normalization, so
mobile proof engines can echo equivalent request metadata without false
rejection while still failing on padded or mismatched fields.
JavaScript TypeScript declarations now also describe the full optional Solana
local-prover result metadata accepted by the runtime: canonical source public
inputs, source-state verifier ids/hashes, proof context, source-adapter
deployment binding, proof base64, and envelope hashes. Browser portal builds
can type-check the same UI prover contract that the wrapper revalidates before
submission.
The JavaScript and Python Solana source-state prover callbacks now also
validate optional structured OpenVerify/FastPQ result metadata against the
SDK-built request, including AccountsLtHash residual hashes, audit
role/verifier hashes, public input columns, FastPQ public inputs/transitions,
and statement/context/schema/commitment bytes. Browser and portal-backend proof
engines may echo equivalent camel/snake FastPQ aliases, numeric slots,
uppercase canonical hex fields, and byte/hex transition values, but
public-input columns remain exact. They therefore fail locally if they return a
proof capsule bound to a stale or padded source-state request transcript.
Structured source-state prover result version aliases are now also single-alias
checked and must normalize to `v1`, so proof engines cannot display one
`proofVersion` while the SDK wraps another version for submission.
The JavaScript package declarations now expose that flexible FastPQ result
metadata type for browser proof engines without widening the readonly
SDK-built request transcript types. The declaration requires one accepted
alias for each FastPQ root/hash and transition byte field, matching the runtime
single-alias guard.
Source-state proof capsule declarations now also require proof bytes and one
circuit-id alias, while structured prover result declarations require proof
bytes but keep circuit-id echoes optional, matching the request-bound runtime
callback behavior.
The same lane-aware helper validates structured TON source-state callback
metadata for TON role aliases, masterchain/shard seqnos, shard-state
public-input/proof hashes, public-input columns, FastPQ transitions, and
statement/context/schema/commitment bytes. The JavaScript declarations expose
that TON result object so portal builds see the same request transcript
contract that the runtime enforces.
Solana deployment-backed production admission now consumes the governed
full-light-client audit hashes as proof data, not only deployment metadata:
the source proof carries separate OpenVerify/FastPQ capsules for Tower replay,
full AccountsDB lattice, and bank/fork-choice verification. Production
verification with an audited Solana deployment rejects missing role capsules,
cross-role-spliced Tower replay, full AccountsDB lattice, or bank/fork-choice
capsules, tampered bank/fork-choice proof bytes, and proofs whose role verifier
hash no longer matches the exact deployment record.
The Rust Solana full-light-client audit role builders now also require the
nested AccountsLtHash OpenVerify/FastPQ capsule to verify cryptographically
against the governed AccountsDB source-state verifier before deriving any
second-stage audit role proof, so a shaped but invalid AccountsLtHash capsule
cannot be cascaded into Tower replay, full AccountsDB lattice, or
bank/fork-choice proof material.
Core bridge-proof admission now also covers the next gate: once an audited
Solana source proof opens the source-adapter gate, a replayed route allowlist
hash is rejected against the canonical source-material, source-deployment, and
destination-binding tuple. Taira's SCCP proof-size cap is sized to let those
audited Solana artifacts reach readiness validation.
The web, Python, Swift, Kotlin, and Java Android SDK surfaces now build the
three matching second-stage audit role proof requests from user UI material,
binding the Solana mainnet genesis public input, completed AccountsLtHash proof
hash, finality-context hash, vote-message hash, source verifier material,
deployment hash, gate hash, and role verifier hash before the proof is handed
to the local prover. The dynamic JavaScript and Python builders now reject
duplicate aliases in the AccountsLtHash source-state request and these
second-stage audit inputs before any FastPQ transcript is derived, including
blockhash/finality-context/vote-message aliases plus material, deployment, and
gate hash echoes. Those builders now reject empty or all-zero nested
AccountsLtHash proof bytes before deriving second-stage audit transcripts,
matching the Rust audit-capsule guard.
The typed Swift, Kotlin/JVM, and Java Android mobile inputs now also derive the
source material hash, source-adapter deployment hash, and full-light-client
gate hash from supplied source-material component hashes plus the deployment
receipt; any supplied precomputed hash is only an annotation and is rejected if
it diverges from the locally derived governed record.
The JavaScript and Python public source-state capsule canonicalizers now match
the mobile/Rust profile checks as well: Solana canonicalization only accepts
the AccountsLtHash `stark-fri-v1` circuit, and explicit TON canonicalizers only
accept the shard-state `stark-fri-v1` circuit before hashing completed proof
capsules for UI prover output. JavaScript FastPQ source-state and
full-light-client audit proof request builders for Solana, TON, and
public-input columns, transition metadata, and aggregate request maps, and
return canonical statement, context, schema, commitment, and witness byte fields
through defensive-copy getters, preventing browser code from mutating a
UI-visible proof request after transcript derivation. Solana and TON
full-light audit requests across JavaScript, Python, Swift, Kotlin, and Java
Android now expose the same snake_case role ids to linked web/mobile prover
callbacks (`tower_replay`, `full_accountsdb_lattice`, `bank_fork_choice`,
`masterchain_config`, `validator_set_transition`, and
`shard_accounts_dictionary`) while preserving language-native aggregate result
properties.
The TypeScript
declarations mark those request objects, nested arrays, transition entries, and
aggregate maps as readonly for portal compile-time checks. Python portal-backend
as read-only dict/list-compatible envelopes with immutable byte payloads,
preserving normal inspection while preventing callback-side metadata rewrites.
Kotlin mobile request models now also store defensive copies and return fresh
byte arrays for Solana AccountsLtHash, Solana/TON full-light audit, and
matching the Java Android record/accessor surface. Java Android also freezes the
tests now pin the same value-snapshot contract for mobile proof requests.
The Swift, Kotlin, and Java Android direct audit-request inputs now also reject
duplicate audit role verifier hashes and reuse of any source-adapter material
role hash, including trust-anchor, consensus, message-inclusion,
finality-policy, source-state verifier, adapter verifier-key, and deployment
receipt material, before handing work to a mobile prover. Those direct mobile
inputs also recompute the Solana full-light-client gate hash from the
source verifier material hash, source-adapter deployment hash, and role verifier
hashes, so a UI cannot hand the prover a stale or unrelated gate commitment.
JavaScript and Python Solana full-light audit request construction now performs
the same role-separation preflight before deriving browser/portal prover
transcripts, including the guard against reusing governed source-state or
deployment material as an audit role verifier hash. Web/Python regressions now
also pin that supplying a source-adapter deployment record without matching
witness deployment hash/receipt fields fails before a user-linked prover can
produce request-bound proof bytes, and Swift/Kotlin/Java Android tests cover
the same stale witness deployment-hash and receipt-hash paths for mobile proof
UIs.
Python Solana witness and deployment-binding normalizers now preserve explicit
empty strings through validation, matching JavaScript's rejection of empty
source-state verifier and source-adapter deployment binding fields instead of
silently treating them as absent.
JavaScript, Python, Swift, Kotlin, and Java Android also now wrap completed
Solana and TON OpenVerify/FastPQ proof bytes into checked source-state proof
capsules pinned to the originating AccountsLtHash, TON shard-state, or
full-light audit request circuit, giving web portals, portal backends, and
mobile apps a request-bound output step before those proof capsules feed the
next audit/source proof stage. Solana canonical source-state capsule bytes now
cover the AccountsLtHash circuit and the three full-light audit circuits, while
the AccountsLtHash proof-hash helper remains restricted to the nested
AccountsLtHash circuit. Solana wrappers now rederive the canonical
FastPQ transition bindings for the AccountsLtHash request and each full-light
audit role, rejecting externally generated proof bytes if any transition key,
operation, old value, or new value no longer matches the SDK-built request
transcript.
TON wrappers now rederive the canonical FastPQ
transition bindings for the shard-state request and each full-light audit role
and reject externally generated proof bytes if any transition key, operation,
old value, or new value has been mutated. Those SDKs now expose app-linked
source-state prover facades as well: web/Python callers can inject a prove
callback for SDK-built Solana AccountsLtHash, TON shard-state, and
role-separated full-light audit requests, while Swift, Kotlin, and Java Android
mobile apps can inject
typed AccountsLtHash, TON shard-state, and audit-role proof engines. The facades
invoke the linked prover only after the request has passed the same request-shape
checks used by the wrap helpers, and the direct Solana full-light audit builders
now reject request-bound role verifier hash reuse before returning requests to a
web portal or mobile app. The TON direct audit request builders now apply the
same separation to source-state proof hashes, shard-state public-input hashes,
deployment/material/gate hashes, role columns, and audit-statement hashes, so a
governed TON role verifier hash cannot be replayed from per-request transcript
material before UI proof generation starts.
TON validator-set transition-chain hashes now commit to the complete canonical
transition proofs across Rust, JavaScript, Python, Swift, Kotlin, and Java
Android, including BlockIdExt fields, next-set payload bytes, signer bitmap,
validator keys/weights, and signature bytes. This makes the shard-state and
full-light audit request transcripts change when a UI/RPC witness mutates any
transition field, not only when the summary hashes change. Web, Python, Swift,
Kotlin, and Java Android regressions now include non-empty transition lists so
portal and mobile proof-generation paths cover the production transition-chain
case instead of only the trust-anchor-direct fixture path.
The same web, Python, Swift, Kotlin, and Java Android source-material helpers
now reject all deterministic Solana template component hashes, including source
trust-anchor, consensus, message-inclusion, AccountsDB source-state, and
finality-policy hashes, so UI provers cannot derive production request
transcripts from placeholder verifier material.
Rust Solana full-light audit proof builders and verification now also reject
governed role verifier hashes replayed from request-bound source-state,
material, deployment, gate, finality, vote-message, nested AccountsLtHash proof,
or audit-statement hashes, so an otherwise ready deployment cannot package or
admit an audit proof whose verifier identity is borrowed from the live request
transcript.
Solana source-adapter structural admission also requires the signer bitmap to
use the exact byte width implied by the validator roster and to keep unused
padding bits zero before vote-proof verification or transcript hashing. The
same adapter-envelope preflight now also rejects non-V1/non-Solana source
proofs, zero finalized slots, zero blockhash/bank/status/message roots,
malformed or zero transaction identities, finality contexts whose epoch or
parent/finalized slots do not match the finalized slot, zero finality-context
roots, zero bank-signature counts, zero vote-message hashes, empty StakeHistory
sysvar data, and malformed or all-zero AccountsLtHash bytes before
source-adapter transcript hashing. The structural preflight now also rejects
present source-state proof capsules unless they are version `1`, use
`stark-fri-v1`, carry a non-empty circuit id, and contain non-empty/non-all-zero
proof bytes before Norito/OpenVerify decoding, covering nested AccountsLtHash
material, full-light-client audit role proofs, TON source-state capsules, and
and role-separated full-light-client audit verifier now apply the same all-zero
capsule guard before decoding, so direct verifier calls cannot bind placeholder
proof bytes outside structural admission.
For the web/mobile portal path, the
remaining Solana blocker is governed all-lanes rollout with live
full-light-client verifier deployments, not request derivation or the
source-adapter production predicate itself. The all-lanes preflight now rejects
Solana destination rollout evidence that was not produced with live immutable
ProgramData metadata, so the remaining rollout blocker is governed deployment
and canary evidence for those live verifier programs. That metadata now pins a
positive expected ProgramData slot alongside the ProgramData address and
executable hash, the base64 ProgramData executable preimage, plus finalized RPC
  context-slot metadata proving the ProgramData account was read at or after the
  deployment slot, requires the canonical 36-byte Program account layout, and
  now carries base64 preimages for that Program account and the immutable
  ProgramData metadata header. The all-lanes gate decodes those preimages to
  prove the Program account points to the claimed ProgramData address and the
  ProgramData header encodes the same deployment slot with no upgrade authority.
  It rejects ProgramData metadata that aliases the verifier program id, carries a
  non-BPF-ELF executable preimage, or tampers with either account/header preimage,
  so confirmed, placeholder, self-referential, stale slot, wrong-layout,
  non-executable-byte, or hash-only comments cannot satisfy all-lanes readiness.
  Those ProgramData pins now render as configured `solana_*` rollout fields, not
  only helper comments, and Rust/Core route readiness recomputes the Solana
  canary evidence from that configured ProgramData transcript before accepting a
  route allowlist for production. Generic non-zero canary hashes are no longer
  sufficient for the SORA -> Solana lane.
  The Solana live evidence direct APIs also
  revalidate imported live dictionaries for BPF-loader owners, immutability,
  canonical Program preimages, immutable ProgramData metadata bytes, fresh slots,
  executable length, and executable/hash consistency before reporting
  `toml_ready` or rendering TOML, so forged live metadata cannot bypass JSON-RPC
  collection. The
direct Solana destination helper and its importable render/summary APIs can
derive the same verifier code hash from supplied program bytes and reject
mismatches with an explicit hash before producing JSON or TOML. Production
TOML now rejects hash-only Solana destination evidence and JSON summaries keep
`toml_ready = false` until the same BPF ELF executable preimage is supplied, so
copied verifier hashes cannot satisfy the direct rollout path. It now applies
the same
launch posture to TON destination rollout evidence by requiring live active
account-state metadata from the TON live helper, so the remaining TON release
blocker is governed canary evidence for the live verifier contract and full
lane rollout. The offline
`scripts/sccp_ton_source_state_evidence.py` helper now renders the governed
TON -> SORA source material and source-adapter deployment TOML from
operator-collected live component hashes plus the required TON full-light-client
audit verifier hashes. It rejects the template-derived TON source trust-anchor,
consensus-verifier, message-inclusion, source-state verifier, and
finality-policy hashes before rendering governance TOML, appends the audited
deployment suffix, and emits the runtime-checked
`ton_full_light_client_gate_hash`, so the remaining TON release blocker is
governed live deployment/canary evidence and lane rollout rather than SDK
request binding or source-adapter admission wiring. The helper and its direct
hash APIs now require exact `u32` source/target domain values, so boolean
placeholders cannot coerce to SORA domain `0` before source material,
deployment, or full-light-client gate hashes are derived. They also reject
padded component hashes and source/target domain values before those record and
gate hashes are derived. Its compact JSON summaries
now also expose `source_verifier_material_ready`,
`source_adapter_engine_deployment_ready`,
  `source_adapter_gate_ready_with_full_light_client_evidence`,
  `source_adapter_gate_blockers`, and `full_toml_ready`, matching the Solana
  source-state diagnostics so CI can distinguish pinned TON material, pinned
  source-adapter deployment, complete full-light-client TOML readiness, and the
  exact missing gate evidence. The all-lanes JSON summary now also emits a
  per-lane `source_adapter_gate` object with `required`, `ready`, `gate_hash`,
  `audit_hashes`, and `blockers`, so Solana/TON rollout automation can read the
  recomputed full-light-client gate state directly from the final launch
  preflight rather than parsing generic lane blockers. Rust source proofs now
  carry the three TON full-light-client audit capsules as proof data, and
  deployment-aware verification rejects missing masterchain-config capsules,
  spliced validator-set-transition capsules, tampered shard-accounts dictionary
capsules, all-zero shard-state or audit-role proof capsules, duplicate audit
verifier hashes, audit hashes that reuse existing verifier material, and audit
hashes that reuse built-in TON template component hashes. Core bridge-proof
admission now has focused coverage that
configured audited TON source deployment material reaches the all-lanes gate,
while mismatched or partial TON full-light-client audit records fail before
structural proof evaluation. The SDK source-material and shard-state
proof-request builders now reject those template-derived TON
component hashes, including the shard-state source-state verifier hash, before
invoking the app-linked prover. Rust TON source-material constructors now also
reject all-zero or template-derived shard-state source-state verifier hashes
before returning deployment-shaped material. The helper also rejects
non-canonical source-adapter OpenVerify VK hashes before rendering governance
TOML, matching the Rust deployment-material gate. The Rust SCCP crate and
JavaScript, Python, Swift, Kotlin, and Java Android SDKs now also build the
exact TON shard-state
source-state OpenVerify/FastPQ request from UI/mobile witness material,
including statement bytes, witness commitment bytes, verification context,
schema descriptor, public-input columns, FastPQ metadata transitions, and the
`sccp-ton-shard-state-light-client-v1`/`fastpq-lane-balanced` identifiers. The
shared vector now uses the production-consistent masterchain config proof hash
for the same `ShardStateUnsplit` root that the TON shard-state opening proves,
matching the Rust FastPQ batch gate. The
`scripts/sccp_ton_destination_evidence.py` helper now covers the SORA -> TON
destination rollout and route allowlist records from the deployed TON verifier
contract address and a code hash derived from the single-root verifier code
BoC, and it recomputes the route allowlist hash from the governed TON
source-material record hash, audited source-adapter deployment record hash, and
canonical SORA -> TON destination binding. The live helper recomputes the
returned `code_boc` root hash before trusting TON Center's `code_hash` and
keeps the actual code BoC in the generated TOML comments so all-lanes can
replay the root-hash derivation. The offline
`scripts/sccp_all_lanes_evidence.py` preflight consumes the rendered source,
destination, and route TOML snippets and reports lane-specific blockers for any
missing or non-production record across ETH, BSC, Solana, TON, TRON, and the
	domain records in the source-material, source-adapter deployment, destination
	rollout, and route-allowlist sections instead of ignoring stray governance
	records while declaring the advertised lanes ready. It also recomputes the
	Solana and TON full-light-client gate hashes and TRON source bridge config hash,
	so governance cannot stage arbitrary non-zero audit placeholders,
	template-derived component hashes, non-canonical source-adapter verifier keys,
	reused non-zero source/deployment/audit role digests, or malformed destination
		verifier identities. Native Solana, TON, and
	expected destination binding hash and route canary evidence are pinned; route
	allowlist hashes and paired source record hashes are rejected before that
	independent binding pin matches. TON
source TOML now matches the
Solana audit-gate discipline by requiring all three TON full-light-client audit
verifier hashes plus an independent `--expected-full-light-client-gate-hash`
before TOML is rendered; JSON summaries stay diagnostic and report
`toml_ready = false` until that complete audit bundle and pin are present. The
TON audit gate now also rejects audit hashes that reuse built-in template
component hashes before deriving production-looking gate hashes, and the
JavaScript, Python, Swift, Kotlin, and Java Android SDK proof-request builders
apply the same rejection before UI/mobile prover invocation. For ready lanes
the same JSON summary reports canonical source material and source-adapter
deployment record hashes for governance comparison. For
JavaScript, Python, Swift, Kotlin, and Java Android portal/mobile SDKs
release blockers are governed runtime verifier deployment evidence and lane
rollout rather than SDK request derivation for either destination runtime
proofs or source-state runtime-storage proofs. Release summaries now also carry
destination helper and its importable render/summary APIs can now derive the
runtime verifier code hash from supplied runtime bytes and reject explicit hash
mismatches before producing JSON or TOML. Rust runtime-storage verification
also rejects all-zero source-state proof capsules before OpenVerify/FastPQ
signed ancestor-linked solid-block header proof,
  which now authenticates both `txTrieRoot` and TRON `accountStateRoot`, witness
  seal, witness-schedule transition certificates, and transaction-Merkle
  source-call proofs over authenticated transaction bytes. Deployment-backed
  TRON source-adapter readiness can open when exact mainnet source material,
  governed source-bridge contract evidence, adapter verifier commitment, and
  deployment receipt hashes match; the mainnet source-material identity now
  uses the `transaction-source-mainnet` verifier profile and commits to the
  transaction source-call transcript prefix as well as the legacy structural
  receipt prefixes, and TRON recoverable signature preflight
  rejects invalid `r` scalars before hashing header or witness-seal material.
  Witness-schedule transition block hashes must be backed by the supplied
  solid, parent, or signed ancestor header evidence before a schedule rotation
  can activate.
  The TRON transaction source-call verifier now requires
  `contractRet = SUCCESS`; explicitly present top-level `ret` values must be
  java-tron's default `SUCESS = 0`, while canonical transactions that omit that
  default field are accepted. Only the canonical optional `Result.fee` field
  may accompany those success fields, and unknown result extensions fail closed.
  JavaScript, Python, Swift, Kotlin, and Java Android transaction-source hash
  helpers now mirror that omitted-default-ret rule against the same canonical
  proof vector.
  The TRON source bridge evidence direct material and
  deployment record hash helpers now reject template-derived source component
  hashes, matching the TOML renderer so programmatic rollout tooling cannot
  derive production-looking records from template material. The same SDK helpers
  now recompute the TRON source bridge config hash from the bridge address,
  network id, TRON -> SORA lane ids, and owner address, rejecting mismatched
  caller-supplied config hashes and padded fixed-width evidence strings before
  deriving production-looking records. The
  authenticated source-call transaction must now carry exactly one recoverable
  signature, and that signer must be the configured source-bridge owner, so
  duplicate, multisig, or non-owner signatures cannot pad the source-call
  transaction; Python, JavaScript, Swift, Kotlin, and Java Android source-proof
  helpers now run the same recovery check before UI/mobile transcript hashing.
  The outer
  `Transaction` parser, the signed `raw_data` parser, and the nested
  `Transaction.Contract`, `Any`, and `TriggerSmartContract` parsers also reject
  unknown fields, forcing future java-tron call extensions to fail closed until
  they are explicitly profiled and bound. The signed `raw_data` parser now also
  requires non-zero ref-block bytes/hash, non-zero expiration/timestamp/fee limit,
  and `expiration > timestamp`, while keeping deprecated `ref_block_num`
  optional. The Rust source-call calldata helper
  and source-adapter deployment builder are now
  locked to the same production TRON -> SORA lane as the SDK helpers and reject
  non-TRON sources, non-SORA targets, and zero source-event digests before
  encoding calldata or deployment evidence. Rust and the JavaScript, Python,
  Swift, Kotlin, and Java Android SDK transaction-source helpers now reject any
  `transaction_root` that does not recompute from
  `transaction_bytes`, `transaction_index`, `transaction_count`, and
  `transaction_merkle_branch`, and the SDK helpers reject non-canonical TRON
  recoverable transaction signatures before deriving production-looking
  source-proof hashes. Those SDK helpers now also accept optional governed
  source bridge emitter/owner address expectations and reject transaction bytes
  whose embedded `TriggerSmartContract` contract or owner address drifts from
  the production source material before deriving those hashes. Rust now exposes
  matching source-bridge-bound transaction-source bytes/hash helpers for
  operator tooling; matching governed material preserves the existing canonical
  transcript, while wrong bridge/owner or zero-address pins fail before any hash
  is returned. Rust also exposes material-bound variants that extract those pins
  from production-ready TRON source verifier material so operators do not have
  to copy bridge and owner addresses by hand. When that transaction-source proof
  is present,
  the legacy receipt proof fields must remain canonical-zero/empty so unused
  receipt-index or trie material cannot be carried through production adapter
  transcripts. The Rust TRON verifier now also rejects non-canonical protobuf
  varints before parsing source-call transactions, raw header data, result
  messages, or legacy receipt logs, preventing overlong or overflow-shaped
  encodings from becoming alternate accepted transcripts. JavaScript, Python,
  Swift, Kotlin, and Java Android solid-block header helpers now decode those
  raw header fields with the same canonical varint rule and recompute the
  supplied raw-data hashes, block ids, parent link, trie roots, witness address,
  timestamp, and header version before returning transcript bytes.
  The live evidence step now has an offline helper,
  `scripts/sccp_tron_source_bridge_evidence.py`, that recomputes the governed
  source bridge config hash and renders the matching source material,
  source-adapter deployment, destination rollout, and route allowlist TOML from
  collected deployment hashes, including the source bridge network id and owner
  address bound by production admission. TRON verifier evidence now recomputes
  that source bridge config hash whenever the config fields are populated, so
  mismatched material-only evidence fails before production deployment gates are
  considered. The paired read-only live collector,
  `scripts/sccp_tron_live_evidence.py`, queries deployed source bridge and
  destination verifier view functions through TRON constant calls, recomputes
  `sourceBridgeConfigHash()` and `destinationBindingHash()`, optionally
  cross-checks `/wallet/getcontract` bytecode metadata, supports
  `/walletsolidity/triggerconstantcontract` confirmed-state reads, supports a
  runtime-only `TRON-PRO-API-KEY` header for TronGrid endpoints, requires each
  constant-call response to carry an explicit successful `result.result = true`
  flag before trusting the returned ABI word, and emits diagnostic
  offline-renderer arguments plus the Torii/CLI destination-query fields for
  artifact/job submission without signing, broadcasting, deploying, or mutating
	  chain state. TRON API diagnostics now redact HTTP bodies, transport reasons,
	  duplicate key names, and error objects before public blockers are emitted.
	  The live CLI also redacts sensitive top-level collection failures to a fixed
	  TRON evidence-collection diagnostic before printing operator errors.
	  Those offline arguments only carry expected source-config and
  destination-binding pins after the corresponding operator-supplied expected
	  values match the live views, and they only carry the route allowlist hash after
	  the expected destination-binding pin matches; the live collector now rejects
	  supplied route allowlist evidence before that pin matches instead of reporting
	  a route hash in diagnostic JSON. They now carry route canary evidence only
	  after that route tuple is verified, and live full-TOML output stays disabled
	  until the same route canary hash is present. When supplied with governed source component hashes,
  the source-adapter deployment receipt hash, and expected source record hashes,
  the live collector also recomputes the canonical TRON source verifier material
  and source-adapter deployment hashes before emitting offline rollout arguments;
  any mismatch fails in the read-only collection step. When metadata lookup is
  enabled, source-bridge live collection requires `/wallet/getcontract`
  source-bridge bytecode to be present and bound to the queried contract address
  so a manual source code hash cannot fill a missing or cross-contract node
  observation; destination metadata applies the same address binding before
  verifier bytecode is trusted. The live collector and direct renderer now
  preserve both runtime bytecode preimages in TOML/offline arguments, and
  all-lanes recomputes the source bridge and destination verifier code hashes
  from those bytecode comments before launch readiness. Operators must pass
  `--no-getcontract` only after an independent code-hash audit. Operators can also pin the deployment/governed
  source bridge config hash with
  `--expected-source-bridge-config-hash`, so owner or lane drift is rejected
  before production full-TOML output; diagnostic offline arguments include that
	  expected pin only after it matches, and route allowlist/canary arguments remain
	  withheld until the destination binding is also independently pinned. Direct
	  full-TOML output requires that config pin plus complete
	  `--route-canary-transaction-*` metadata, and the direct TOML renderers now apply the
	  same runtime-bytecode hash derivation/mismatch checks as the CLI before
	  emitting governance records. Direct full-TOML output now also annotates the
	  source bridge and destination verifier metadata required by all-lanes
	  preflight, so independently audited TOML can satisfy the same launch gate as
	  live-collected evidence. Live full-TOML output also requires an
	  explicit governed `--expected-destination-binding-hash` match,
	  transaction-derived route canary evidence, and `/wallet/getcontract`
	  bytecode metadata for both the source
	  bridge and destination verifier before the destination rollout can be emitted.
	  The live helper now requires a transaction-derived route canary for live
	  production TOML and derives that hash from a supplied
	  `--route-canary-transaction-id` by verifying the destination verifier's
	  `MessageProofAccepted` log against the deployed binding/backend/family/network
	  views, exact transaction-info `blockNumber`/`blockTimeStamp` metadata, and
	  the validated route allowlist hash, then fetching the raw
	  `TriggerSmartContract` transaction, parsing the hashed `raw_data_hex`,
	  requiring its owner address to match the visible transaction owner,
	  requiring the single canonical recoverable secp256k1 signature to recover
	  to that owner, and checking the
	  `submitSccpMessageProof(bytes,bytes32[6],bytes32)` selector, ABI public
	  inputs, statement hash, 384-byte proof tuple, and proof header against the
	  accepted event and deployed verifier domains, plus
	  `usedMessageProofs(messageId)` current-state consumption; if operators also supply
	  `--route-canary-evidence-hash`, it must match the transaction-derived value.
	  All-lanes preflight now requires those TRON canary transaction fields and
	  audit comments for TRON, including the `usedMessageProofs` state result,
	  `tron_route_canary_transaction_owner_address`,
	  `tron_route_canary_raw_data_owner_matches_transaction`, the route-canary
	  signature hash, recovered address, and `signature_recovers_to_owner` audit
	  result. The recovered address must equal the transaction owner before the
	  gate recomputes the canary evidence hash and treats the route evidence as
	  bound, and saved full-TOML replay revalidates the carried route-canary block
	  metadata before offline arguments can be regenerated. All-lanes and the
	  direct renderer also reject reuse across distinct
	  TRON canary transcript hash roles, including transaction id, message id,
	  calldata, payload, statement, commitment, finality height, finality block,
	  and signature hash fields. The canonical TRON canary transcript now uses
	  `iroha:sccp:tron-route-canary-evidence:v3` and commits the exact
	  `submitSccpMessageProof(...)` calldata SHA-256, decoded payload hash,
	  target-domain word, finality-height word, finality block hash, proof
	  version, proof source domain, transaction owner, route-canary block number,
	  route-canary block timestamp, raw-owner binding flag, signature SHA-256,
	  recovered signer, and recovery flag in the evidence hash itself; Rust lane
	  readiness mirrors that transcript and hash-role
	  separation gate from the first-class
	  `tron_route_canary_*` route record fields, and the configured Torii/Core
	  all-lanes path preserves the owner/signature, transaction block metadata,
	  and v3 call-transcript fields into launch readiness. ZK consensus policy
	  hashing now commits those TRON route-canary fields and the existing EVM
	  route-canary transaction fields.
	  The direct TRON renderer now
	  requires the same `--route-canary-transaction-*` metadata plus
	  `--route-canary-used-message-proof` and
			  `--route-canary-raw-data-owner-matches-transaction` plus the
			  `--route-canary-signature-*` recovery metadata for full TOML, requires the
			  recovered address to match `--route-canary-transaction-owner-address`,
			  derives the canary hash when an explicit hash is omitted, and rejects
			  transaction/hash mismatches before emitting full TOML. Saved live JSON
			  replay now also revalidates the route-canary selector, Groth16 tuple
			  length/version/source domain, public-input message id, target domain,
			  payload hash, commitment root, finality words, statement hash,
			  owner/signature fields, submitted calldata hash, and recomputed canary
			  hash before allowing offline production TOML.
		  Swift, Kotlin, Java Android, and the EVM/TRON contract smoke now validate
		  the local TRON proof request, proof wrapping, verifier-call packaging, and
		  wrapper/source-bridge contract paths; the remaining TRON production
		  checkpoint is governed live deployment plus route-canary evidence, not
		  local SDK or contract wiring. Rust deployment-backed TRON source-adapter
		  readiness now also derives the typed `sccp:tron:dpos-source-gate:v1`
		  transcript over the source material, source-adapter deployment, adapter
		  VK, DPoS/witness/source-call role hashes, governed source bridge config,
		  and TRON verifier prefixes/bounds before the source-adapter gate opens.
		  That gate is now first-class configured evidence as
		  `tron_dpos_source_gate_hash`: TRON source-adapter deployments must carry
		  the exact recomputed hash through Torii/core configured-lane admission,
		  the ZK consensus policy hash, the direct/live TRON evidence renderers,
		  and all-lanes preflight; non-TRON lanes must leave it empty. Production
		  TRON source/full-TOML rendering and live full-TOML readiness now also
		  require the operator-supplied
		  `--expected-tron-dpos-source-gate-hash` to match that canonical gate,
		  so JSON dry-runs can derive the hash but rollout TOML cannot become ready
		  until the first-class DPoS/source-call gate is explicitly pinned. Direct
		  `--full-toml` now also requires source-bridge and destination-verifier
		  runtime bytecode preimages, keeping hash-only direct CLI runs diagnostic
		  instead of production-ready.
		  `--no-getcontract` remains a
	  diagnostic JSON path only; it cannot produce production-ready full TOML.
  When the verified live
  evidence is sufficient for full governance TOML, the collector emits
  `offline_full_toml_args` with `--full-toml` already appended, verifies that
  the generated argument list renders through the offline governance TOML
  helper, emits `offline_full_toml_sha256`, and can print that verified TOML
  directly with `--full-toml`. The all-lanes rollout preflight now has a
  regression that accepts this verified live TRON full-TOML output when merged
  into a complete SCCP evidence bundle and checks the resulting TRON source
  record hashes against the live collector output. The public full-TOML renderer
  now also requires those computed source-record match flags in the summary, so
  hand-built summaries with only expected-hash strings cannot bypass live hash
  verification. The all-lanes gate also
  rejects TRON records that lack live source-bridge or destination-verifier
  address/code-hash metadata, so offline/manual TRON records remain diagnostic
  instead of launch-ready. The same live
  pass now fails if destination verifier bytecode metadata is missing or
  disagrees with the deployed `verifierCodeHash()` view, and the public
  full-TOML argument helper now withholds `offline_full_toml_args` unless the
  live metadata match flag is present and the two destination code-hash strings
  are identical. Malformed destination runtime-bytecode metadata now produces a
  category-only blocker in the live full-TOML path, so parser details are not
  copied into release evidence. Live destination evidence now also checks
  `verifierBackendHash()` and `proofFamilyHash()` against the canonical
  `tron-groth16-bn254-v1` / `stark-fri-v1` deployment profile before emitting
  rollout or Torii query material, carries those hashes into full-TOML metadata,
  and the all-lanes preflight requires the metadata to match the canonical
  profile. Torii destination query parameters now also require the destination
  `/wallet/getcontract` bytecode hash to match `verifierCodeHash()`, so
	  signer-free artifact/job material cannot be emitted from `--no-getcontract`
	  diagnostics, and the live helper now re-parses and recomputes the destination
	  binding from summary fields before exposing importable Torii query params,
	  using strict canonical u32 parsing so boolean or leading-zero summary domains
	  cannot be coerced into a production lane.
	  Those Torii/CLI fields
  now include `expected_destination_binding_hash_hex` only after the
  operator-supplied `--expected-destination-binding-hash` matches the live
  verifier view, and Torii now requires that pin for EVM/TRON artifact or job
  requests whenever deployment destination fields are supplied, then rejects the
  request when the live binding hash does not match the binding recomputed from
  those deployment fields. The live summary also marks those query params as
  requiring external `proof_bytes_hex`, so deployment evidence is not mistaken
  for a complete artifact/job request. When a destination rollout
  must also be configured on the node; Torii rejects deployment-bound artifact,
  job, bridge-proof, and bridge-message requests when the rollout is missing or
  when the recomputed destination binding key/hash does not match the configured
  rollout. JavaScript and Python clients expose the
  same guard. JavaScript, Python, Swift, Kotlin, and Java Android
  source-material/deployment hash helpers now apply the same TRON template
  component rejection before emitting production-looking hashes. Source bridge
  ownership transfers also emit the new config hash
  automatically, giving rollout evidence a direct owner-rotation audit event.
  The same helper recomputes the TRON destination Groth16 binding
  hash from the base58 wrapper address, deployment hashes, proof family, network
  id, and source/target domains, requires `--expected-config-hash` for
  production source TOML, can compare
  `--expected-source-verifier-material-hash` and
  `--expected-source-adapter-engine-deployment-hash` plus the governed
  `--expected-tron-dpos-source-gate-hash` before JSON, source TOML, or full
  rollout TOML output, requires both expected source record hashes, the expected
  DPoS source-gate hash, and `--expected-destination-binding-hash` for full
  TOML, derives
  source/destination runtime code hashes from deployed bytecode when supplied,
  derives and verifies the canonical TRON -> SORA
  `adapter_verifier_vk_hash` from the FastPQ/OpenVerify source-adapter verifier
  profile, requires both expected source-record hashes and the DPoS source-gate
  hash for production source TOML while still allowing unpinned diagnostic JSON,
  includes the canonical source material, source-adapter deployment record, and
  DPoS source-gate hashes in complete JSON dry-runs and TOML audit comments,
  and includes the recomputed destination value plus canonical
  `SccpDestinationBindingV1.key` in JSON/full TOML output. The helper's compact
  JSON, source TOML, and full rollout TOML output modes are mutually exclusive,
  and the source/direct full-TOML regressions now pin the destination rollout to
  a single `verifier_code_hash` key so strict TOML parsers do not reject
  governance bundles; the all-lanes fallback TOML loader now rejects duplicate
  keys as well when `tomllib` is unavailable, and standard parser failures now
  render category-only invalid-TOML blockers without appending parser payloads.
  Its production TOML modes are
  locked to the
  TRON -> SORA source lane so operators cannot accidentally render
  non-admissible records for another target domain. Full rollout TOML is also
  locked to the paired SORA -> TRON destination binding and `stark-fri-v1`
  proof family, and compact JSON applies those same source and destination lane
  checks before returning hash dry-runs. Compact JSON also treats
  `--route-allowlist-hash` as destination-side evidence, rejects route-only
  partial summaries without the paired destination verifier material, and emits
  the route hash only when that destination material is complete. Compact JSON
  now reports `full_toml_ready = true` only after the source TOML pins,
  destination binding pin, route allowlist hash, and route canary evidence are
  all present and matched, and marks unpinned destination material with
  `expected_destination_binding_hash_matches = false`. Compact JSON also accepts
  `--source-event-digest` and emits the exact
  `submitSccpSourceEvent(uint32,uint32,bytes32)` owner-call calldata for the
  TRON -> SORA source bridge plus the unsigned `/wallet/triggersmartcontract`
  request body, while TOML modes reject that one-off payload so governance
  evidence stays separate from transaction execution material. The
  live collector mirrors that JSON-only source-event behavior for queried source
  bridges, emits `offline_source_event_args` for reproducible direct-helper
  calldata, rebuilds those replay arguments from the saved source bridge
  domains, owner, digest, and calldata instead of trusting stored replay arrays,
  checks `submittedSourceEvents(bytes32)` before pre-submit trigger rendering,
  includes an unsigned `/wallet/triggersmartcontract` request body
  for fresh digests, verifies a post-submit transaction id against the successful
  exact two-topic `SccpSourceEvent(bytes32)` log and raw TriggerSmartContract
  ret/owner/contract/calldata tuple, requires `gettransactioninfobyid` and
  `gettransactionbyid` transaction-id aliases (`id`, `txID`, and `txid`) to
  agree, requires raw transaction readback to carry canonical `txID`, requires
  the transaction `raw_data_hex` to hash to that `txID`, requires exactly one
  canonical 65-byte TRON recoverable secp256k1 signature that recovers to the
  source bridge owner,
  parses the signed `raw_data_hex` protobuf for the same
  owner/contract/calldata/ref-block/timing/fee source-call profile that Rust
  production admission checks, requires exact source-event transaction-info
  `blockNumber` and `blockTimeStamp` metadata and cross-checks the timestamp
  against the fetched block header, requires saved replay JSON to retain that
  same block metadata/solid-block binding, rejects boolean/truthy source-event block
  numbers, timestamps, header versions, and signed-header depth arguments before
  source-event evidence is summarized, uses solidity transaction readback
  endpoints when `--solid` is set, keeps `full_toml_ready` visible on JSON dry-runs, and
  rejects `--source-event-digest --full-toml`. Post-submit JSON also emits
  canonical transaction protobuf bytes, the SHA-256 transaction hash, and the
  transaction Merkle branch; when `--receipt-root` plus repeated
  `--source-inclusion-branch-hex` values are supplied, it derives canonical
  `sccp:tron:transaction-source-proof:v1` bytes/hash and compares
  `--receipt-proof-hash` when present. It fetches the containing block,
  rebuilds the canonical block-header `raw_data` hash/TRON block id, fetches
  the immediate parent block, verifies the child `parentHash` and monotonic
  timestamp, recovers both child and parent header signatures to their declared
  TRON witness addresses, and recomputes java-tron's transaction Merkle root
  from the block's canonical transaction protobuf bytes whose
  `txID`/`txid`/`id` aliases agree to match `txTrieRoot`;
  that live block reconstruction now serializes market-order
	  `orderDetails` and stake-v2 `cancel_unfreezeV2_amount` result extensions on
	  unrelated block transactions instead of rejecting otherwise usable source
	  evidence, and its integer parser accepts JSON numbers or decimal strings
	  while rejecting booleans, non-ASCII or leading-zero decimal strings, and
	  values outside non-negative `int64` bounds. The source-event success checks
	  also accept canonical enum names or numeric protobuf JSON values (`ret = 0`,
	  `contractRet = 1`) while rejecting the non-canonical top-level `ret =
	  SUCCESS` alias plus non-ASCII or leading-zero numeric enum strings, and
	  source-inclusion branch siblings are allowed to be any canonical 32-byte value, including
	  all-zero. Live block-header reconstruction now rejects all-zero
	  `0x41`-prefixed witness addresses before deriving raw-header bytes, matching
	  the Rust and SDK raw-header parsers. Rust, Python, JavaScript, Swift, Kotlin,
	  Java Android, and live
	  collector witness schedule payload helpers now also fail closed when the sum
	  of non-zero witness weights cannot fit the `u64` total committed by witness
  seals;
  when child and parent account-state roots are present it also emits the
  canonical solid-block header proof bytes/hash, while missing roots remain a
  visible blocker. It also accepts canonical witness-schedule payload hex or a
  payload file, derives the `sccp:tron:witness-schedule:v1` hash, optionally
  checks an expected schedule hash, and requires child/parent block witnesses
  to be active schedule members. It now also accepts receipt-root,
  receipt-proof-hash, witness-signers bitmap, and repeated witness-signature
  inputs, derives canonical `sccp:tron:solid-block-message:v1` and
  `sccp:tron:witness-seal:v1` bytes/hashes, verifies every signature recovers
  to the selected schedule witness and enforces strict `> 2/3` signed weight,
  and keeps missing seal material visible as a rollout blocker. Bounded
  `--solid-block-ancestor-depth` and
  `--solid-block-confirmation-depth` reads now collect the non-placeholder
  signed header chains, verify ancestor/confirmation linkage, active schedule
  membership, monotonic timestamps, and strict `> 2/3` unique confirmation
  weight, and keep missing or insufficient header evidence visible as rollout
  blockers. Post-submit live JSON now also exposes a fail-closed
  `source_event_transaction_production_ready` flag plus blockers, and marks it
  true only when transaction source proof, solid-block header proof, expected
  witness-schedule hash, witness seal, signed ancestors, and signed
  confirmations are all present and verified; source-record preflight now feeds
  `--source-trust-anchor-hash` into that expected witness-schedule pin and
  rejects drift from a separately supplied schedule hash. When the active
  schedule differs from that trust-anchor hash, repeated
  `--witness-schedule-transition-json` inputs now let live evidence prove the
  canonical parent-signed transition chain from trust anchor to active schedule
  before setting the production-ready bit. Duplicate-key failures in those JSON
  inputs now render fixed category-only blockers without echoing the duplicated
  key name. Python, JavaScript, Kotlin/JVM, Java
  Android, and Swift SDK/operator surfaces now expose canonical TRON
  solid-block, witness-seal, and witness-schedule transition message/seal bytes
  and hashes, including fail-closed checks that bind the next schedule
  payload/hash, transition message hash, parent schedule hash, and parent
  witness signatures to selected parent-schedule witnesses before seal hashing.
  The helper now also
  recomputes the Rust TRON template component hashes and rejects template
  source trust-anchor, consensus-verifier, message-inclusion, or
  finality-policy hashes before governance TOML is rendered.
  The TRON Groth16/bn254 wrapper and relayer package path are wired, and
  configured material/deployment admission now rejects even deployment-tagged
  legacy receipt-MPT-only proofs that lack the governed transaction-source
  call. Remaining TRON production work is verifier deployment, collection of
  actual live mainnet evidence with deployed contract addresses, and live
  rollout of the governed source bridge transaction-call path. Live
  route-canary evidence capture also binds the visible
  `wallet/gettransactionbyid` owner to the owner encoded in the hashed
  `raw_data_hex`, so full-rollout evidence cannot mix a valid transaction body
  with drifted JSON metadata, and direct/all-lanes TOML now requires that
  binding as first-class route-canary metadata. The legacy `TransactionInfo`
  receipt-log/MPT path
  remains structural only. TVM
  account/storage proofs remain out of scope
  until TRON exposes a
  consensus-authenticated contract-state root; future state-derived claims need
  a new source proof plan and material prefix instead of reusing
  `TronDposReceiptProof`. Land the production
  Solana, TON, and TRON prover/verifier integrations behind the SDK proof request APIs, deploy
immutable destination verifiers plus TON/TRON verifier-contract bindings,
produce multi-lane integration evidence, publish operator runbooks, and
incorporate testnet-driven feedback
from wallet and service integrations.

## IVM, Kotodama, and Norito

**Status:** active first-release hardening.

- Keep the Iroha Virtual Machine syscall and pointer-ABI surface deterministic
  across hardware and peers.
- Make `iroha contract dev` the default first-release contract workflow,
  including manifest-sourced builds, generated interfaces, schema docs,
  profile-aware doctor/smoke commands, and Kotodama test/debug loops.
- Static compiler-derived access descriptors now cover the formerly opaque
  peer, subscription, VRF epoch seed, AXT, Soracloud host, native/anonymous
  escrow, literal nullifier, transfer-batch, and smart-contract lifecycle
  helper syscalls with literal names or decodable Norito request payloads;
  dynamic, malformed, and test-mode-only helper payloads intentionally remain
  fail-closed instead of reintroducing wildcard production manifests.
- Standalone `DefaultHost` ZK batch verification now mirrors the runtime
  status-vector ABI for Halo2 IPA/Pasta envelopes, including deterministic
  backend, curve, max-k, envelope-size, proof-size, and batch-size gates.
  `CoreHost` remains the registry-bound runtime verifier for node execution.
- `CoreHostImpl` now explicitly covers the ABI-listed Soracloud host syscall
  numbers by validating `SoracloudRequest` TLVs, schema versions, operations,
  and payload variants, then failing closed with metered `NotImplemented` and no
  queued ISI during ordinary contract execution. Dedicated Soracloud handler
  execution continues to use `irohad`'s runtime `SoracloudIvmHost` for
  response-producing dispatch.
- Kotodama test-host entrypoint helpers now support tuple-returning entrypoints
  through deterministic multi-register returns, and the IVM/Kotodama docs now
  describe implemented contract bodies, dynamic contract calls, DefaultHost ZK
  batch verification, and execution-proof helpers as current behavior instead
  of future or placeholder work.
- Literal `create_trigger(json(...))` specs that cannot be decoded for access
  metadata now report a dedicated compiler diagnostic and manifest skip reason,
  while production mode continues to reject the incomplete access metadata.
- The IVM mock WSV unshield path now accounts for `Unshield::outputs` private
  change commitments, advances shielded roots, emits commitment events, and
  preflights duplicate nullifiers before any mutation.
- Kotodama `build_unshield_inline(...)` now keeps the existing 7-argument form
  and also accepts optional `outputs32` private change commitments, encoding
  concatenated 32-byte input/output chunks into the canonical Norito
  `Unshield` instruction used by the vendor bridge.
- Kotodama semantic effect analysis now treats ZK verify latch helpers as
  host-side effects: public entrypoints require `permission(...)`, and `view`
  functions reject direct or transitive calls into those helpers.
- Core contract dispatch now consumes Kotodama manifest entrypoint
  `permission(...)` metadata for both direct `ContractCall` and
  metadata-dispatched IVM execution plus nested `CALL_CONTRACT` calls,
  requiring callers to hold the named permission directly or through an
  assigned role before the VM runs.
- Manifest trigger registration now supports namespaced callbacks by resolving
  the callback target at activation time to an already active contract address
  or alias; unresolved aliases, inactive targets, and non-public callback
  entrypoints fail activation.
- Kotodama internal helper functions can pass durable scalar, map, struct, and
  tuple `state` handles, including maps with aggregate values, through
  deterministic flattened child handles. Public entrypoints still reject
  `state` parameters.
- Preserve canonical Norito headers and wire layouts for blocks, transactions,
  SDK fixtures, and cross-library compatibility tests. The JavaScript pure
  Norito fallback now covers asset-definition registration frames, and
  Java/Kotlin columnar helpers cover optional string/u32 plus bytes+bool row
  shapes, so remaining SDK parity work should focus on new observable wire
  formats as they land. Norito columnar NCB views now read their `u32`
  row-count prefix through a shared checked helper, keeping truncated row-count
  prefixes on `Error::LengthMismatch`. Norito streaming baseline RLE block
  decode now reads DC differences and AC records through checked helpers so
  truncated or overflowed cursor state returns `CodecError::TruncatedBlock`
  before cursor advancement, and baseline chunk frame/chroma metadata now uses
  checked fixed-width readers so truncated frame headers and chroma length
  fields fail before payload slicing. Bundled rANS SIMD stream lane lengths now
  use a checked prefix reader so malformed SIMD headers fail before cursor
  advancement or lane slicing.

**Next checkpoints:** ABI golden updates when the syscall surface changes,
expanded cross-SDK vector coverage, and updated docs for any observable layout
or ABI behavior.

## Privacy, ZK, and FHE

**Status:** active research-to-product integration.

- Replace current deterministic plaintext-modulus-multiple BFV-shaped
  evaluation scaffolding with the full BFV-RNS implementation planned for
  release.
- Broaden cross-SDK deterministic vectors for encrypted payloads, receipts, and
  opening verification.
- Keep Soracloud FHE multi-input behavior covered at the source level while the
  BFV-RNS implementation is still pending. The current Rust corridor covers
  deterministic Add/Multiply folds, malformed late-operand rejection,
  multi-input admission/output projection, output commitment order binding, and
  shared Add/Multiply/RotateLeft/Bootstrap operation-output vectors with
  pinned public-key and evaluation-key bundle metadata, per-entry
  relinearization component digests, Galois automorphism key counts and
  per-entry component digests, rotation/bootstrap refresh `c0`/`c1` component
  digests, and adversarial refresh-material rejection for the public
  Galois/rotation/bootstrap keys.
- Keep BFV encrypted-input SDK vectors shared instead of local-only. The current
  fixture set covers the baseline identifier envelope plus Soracloud three-input
  Add and Multiply operand envelopes in JavaScript, Swift, Kotlin/JVM, and Java
  Android test surfaces, with deterministic `{0, t, -t}` error-polynomial
  sampling instead of zero-error ciphertexts. Those SDK lanes now also validate
  the shared operation fixture's component-level evaluation-key metadata so
  missing, zeroed, duplicate, or count-drifted key-component vectors are caught
  outside Rust, while the Rust executor consumes the same fixture for
  operation-output digests and plaintext-slot checks. Crypto
  identifier-envelope admission now
  rejects structurally valid but unregistered BFV parameter profiles before
  identifier encryption, decryption, or downstream Torii/core validation, caps
  `max_input_bytes` at the registered 63-byte/64-slot RAM-LFE identifier
  profile across Rust, JS, Swift, Kotlin/JVM, and Java Android clients, computes
  identifier envelope slot counts with checked max-input-plus-length-slot
  arithmetic, and identifier slot encoding now reports byte-length and
  slot-index conversion failures through `BfvError` instead of panic-only
  assumptions. Always-built
  BFV scalar modular addition, multiplication, and coefficient reduction now
  avoid post-reduction `expect` conversions while preserving max-width
  `u64::MAX` modulus behavior, and the RAM-LFE default programmed BFV hidden
  program now uses profile-sized `u16` constants instead of runtime
  `usize`-to-`u16` conversion assumptions; programmed BFV hidden-program
  admission now also rejects `LoadInput` indexes above the encrypted envelope's
  advertised `max_input_bytes`; programmed BFV memory RNG transcript
  derivation now binds `u64` step values directly instead of converting through
  a panic-only `expect`; BFV/RAM-LFE domain-separated digest, receipt, and
  RNG-seed transcripts now stream hash chunks directly while preserving the
  previous contiguous byte layout; the feature-gated BFV acceleration selector now
  deterministically falls back to scalar schoolbook multiplication for zero or
  overflowed derived convolution lengths, and the CRT-NTT helper path now
  rejects invalid operand lengths, unsupported NTT lengths, and CRT
  reconstruction overflow before using that same fallback instead of panicking
  on degree or NTT arithmetic.
  Programmed RAM-LFE BFV bundle construction now also keeps only fallible
  production constructors that reject unregistered identifier profiles and
  invalid proof metadata before public-parameter digests are emitted, and
  programmed BFV public-parameter decoding rejects encrypted-envelope
  capacities above the canonical profile slot count. BFV now also has a registered RAM-LFE
  v1 RNS coefficient-modulus chain descriptor whose validation requires
  bounded, strictly increasing odd-prime, NTT-friendly, pairwise-coprime limbs,
  bound primitive `2n`-th negacyclic NTT roots for the registered profile, a
  checked product that covers the active ciphertext modulus, a stable
  domain-separated chain digest, and a separate exact-lift compatibility bound
  while the full BFV-RNS arithmetic engine is still pending. The shared RNS
  validator now also validates BFV parameters directly, so exact-lift and exact
  `Z_q` coverage helpers reject malformed parameter profiles before inspecting
  chain arithmetic bounds, and validated limbs must have bounded concrete
  negacyclic NTT root support. The descriptor now also supports checked limb-major
  polynomial decomposition and CRT
  reconstruction, with malformed residue-shape and unreduced-residue rejection
  before arithmetic code can consume those residues, plus deterministic scalar
  residue addition and per-limb NTT-backed negacyclic multiplication with
  bounded primitive-root discovery and a scalar fallback in the RNS chain
  product ring, and the shared Soracloud
  operation fixture now binds that descriptor, digest,
  decomposition/reconstruction corridor, and residue addition/multiplication
  hashes across Rust plus lightweight JavaScript, Swift, Kotlin/JVM, and Java
  Android shape checks. The same fixture now also binds the canonical Galois
  key-switching bundle shape, including automorphism powers and per-entry
  `b`/`a` coefficient-vector hashes, across those SDK lanes. The RNS chain now
  also exposes guarded exact
  ciphertext-modulus polynomial addition and negacyclic multiplication for
  sufficiently wide chains, plus exact RNS-backed ciphertext addition,
  multiplication, relinearization, and Galois key-switch bridges that match the
  scalar evaluator on small wide-chain profiles. The registered RAM-LFE chain
  is now wide enough for that guarded exact `Z_q` bridge, so Rust exercises
  exact RNS ciphertext addition, multiplication/relinearization, and Galois
  key-switching against the production RAM-LFE parameters while retaining a
  separate rejection test for the narrower exact-lift compatibility corridor.
  The programmed RAM-LFE BFV runtime now uses that registered exact RNS bridge
  for ciphertext add, subtract, multiply/relinearization, and `SelectEqZero`
  exponentiation/selection arithmetic; plaintext-scalar operations remain
  scalar because they do not require RNS polynomial products. The public
  Soracloud BFV job executor now uses the same registered exact RNS bridge for
  Add, Multiply, packed and outer `RotateLeft`, and bounded Bootstrap refresh
  rounds, keeping operation-output vectors on the production job path. The
  deterministic BFV baseline now also has packed-polynomial Galois
  automorphism keys that switch `sigma_k(s)` ciphertexts back to the original
  secret key after applying `x -> x^k`, with regressions covering canonical
  odd powers, malformed key rejection, plaintext automorphism parity, exact-RNS
  scalar parity, and registered-chain exact-RNS parity. Scalar and exact-RNS
  key-switch primitives now also validate decomposition-entry counts and
  operand shapes before zipping digits, so malformed key material cannot
  silently truncate a switch. The registered
  batch-friendly `t = 257`, `n = 64` profile now also has deterministic packed
  plaintext slot encoding/decoding plus shared Soracloud scalar and packed
  Galois execution vectors that encrypt deterministic inputs, apply the public
  Galois key-switch, and verify output ciphertext/plaintext digests plus the
  packed-slot permutation across Rust and SDK fixture-shape checks. JavaScript,
  Swift, Kotlin/JVM, and Java Android now parse `compact-v1` BFV Norito length
  encoding and reproduce the Rust-compatible compact operation-input encryption
  stream for the non-packed Soracloud Add, Multiply, outer `RotateLeft`, and
  Bootstrap input vectors, while packed-slot operation inputs remain Rust
  execution vectors plus SDK fixture-shape/digest checks outside the
  identifier-envelope builders. Public
  bootstrap refresh keys now also bind an explicit `max_refresh_rounds` and
  carry domain-separated public refresh ciphertexts for each authorized round;
  Soracloud runtime rejects `Bootstrap` jobs whose requested count exceeds that
  key capacity, computes bootstrap residual admission through the same
  key-aware capacity check, and consumes refresh material by round index.
  Soracloud runtime now also routes single-ciphertext packed
  `RotateLeft` envelopes through public Galois key switching, including masked
  schedules for rotations that are not one automorphism; raw packed rotation
  helpers validate the complete supplied Galois-key slice for bounds,
  duplicates, and malformed entries before scheduled-key lookup, while missing
	  schedule keys fail closed. Shared BFV key validators also validate parameter
	  sets before key shapes, so malformed profiles cannot reach decomposition
	  math through direct secret/public/rotation/evaluation/bootstrap key checks,
	  bootstrap-key validators reject declared round-refresh count mismatches
	  before inspecting refresh ciphertext shapes, refresh-only bootstrap
	  transcript/proof-statement validation rejects stale public-key digest
	  metadata, and parameter validation now uses checked raw/scaled exact-arithmetic products instead of saturating
	  accumulator guards.
  Key-owner diagnostics now also verify that generated public rotation and
  bootstrap refresh ciphertexts decrypt to zero under the matching secret key,
  including a bundle-level check over every rotation and bootstrap refresh
  mask, and public bootstrap admission now requires a verifier-backed statement
  proof envelope. Public deterministic transcript checks now recompute
  rotation and bootstrap encrypted-zero refresh material from the advertised
  seed, public key, key id, and round count, rejecting wrong-seed,
  key-id-drifted, or tampered refresh ciphertexts without requiring a secret
  key; the same check now runs at the evaluation-key bundle level so admission
  cannot accidentally validate only a subset of public rotation/bootstrap
  refresh masks. The validated transcript inventory now also binds public seed
  metadata bounded by the shared BFV deterministic seed cap, bootstrap key-id
  metadata bounded by the shared BFV bootstrap key cap, and rotation inventory
  metadata bounded by the shared BFV evaluation-key rotation cap plus a stable
  domain-separated digest over the parameter set, public key, evaluation-key
  digest, and transcript seed metadata, giving governance/admission code a
  canonical value to bind in the bootstrap-key proof envelope. The crypto layer
  now also exposes exact-lift and bounded-noise transcript-bound
  bootstrap-key zero-refresh proof statement digests that bind parameters,
  public key, evaluation-key digest, refresh-transcript digest, bootstrap
  transcript seed/key id/round capacity, and every public refresh ciphertext
  under mode-separated domains. `RunSoracloudFheJob` now carries an
  optional bootstrap-key proof attachment, provenance signs it, and Core
  requires it for bootstrap execution while checking the policy-bound
  statement hash against an active Soracloud STARK verifier record or
  preverified proof cache entry. The verifier registry now rejects canonical
  Soracloud bootstrap verifier records whose registry id, namespace, circuit
  version, public-input schema hash, gas schedule, or active inline key
	  material drift from the governed v1 profile, moving those rollout failures
	  to `RegisterVerifyingKey`/`UpdateVerifyingKey` admission. BFV bootstrap keys
	  now carry an explicit `RefreshOnlyV1` mode, and `FullBootstrapV1` keys carry
	  versioned circuit/key-material commitments that bind the canonical circuit id,
	  registered BFV parameter digest, RNS modulus-chain digest, key-switch
	  decomposition-chain digest, bootstrap artifact digests, and proof
	  public-input schema/prover-key/verifier-key digests plus typed
	  prover/verifier key-material commitments. The material validator rejects
	  zero commitments, duplicate artifact/proof/key-material commitments, and
	  artifact or proof commitments that reuse registered profile digests, keeping each
	  governed digest role partitioned at admission. Bundle admission and
	  digesting bind that material, while refresh/proof paths and direct
	  no-artifact registered execution fail closed with an explicit governed
	  artifact requirement, so the current refresh bridge cannot be mislabeled as
	  full bootstrapping. Direct
	  key-authorized refresh execution, bootstrap output-bound helpers, and
	  Soracloud exact/bounded bootstrap execution now use the same mode-aware
	  request preflight, so reserved full-bootstrap keys are rejected before
	  round-count, bound-capacity, ciphertext-shape, or refresh-key entry errors.
	  Bundle validation/digesting applies the same public metadata preflight before
	  the mode/material gate and before transcript-bound bootstrap proof statements
	  can be produced. The crypto layer also exposes a domain-separated
	  full-bootstrap material proof-statement digest that binds the parameter set,
	  public key, evaluation-key bundle digest, bootstrap-key metadata, and
	  material digest for governed prover inventories. The data-model refresh
	  transcript wrapper can derive the same full-bootstrap material statement for
	  manifest callers, and execution policies now require bootstrap-capable
		  bundles to bind exactly one bootstrap statement class: exact or
		  bounded-noise zero-refresh for `RefreshOnlyV1`, or full material for
		  `FullBootstrapV1`. Full-bootstrap
	  refresh transcript digesting omits deterministic zero-refresh bootstrap
	  transcript seeds, and Core rejects missing, mismatched, stale, or cross-mode
	  policy statement bindings before execution. The data model now also exposes a
	  distinct full-bootstrap material proof attachment with canonical
	  STARK/`OpenVerifyEnvelope` circuit id, public-input schema, byte bounds,
	  verifier-key commitment, statement public input, and envelope-hash checks, so
	  governed material proofs no longer reuse the zero-refresh bootstrap proof
	  envelope. Core now decodes material proofs through that material-specific
	  attachment context, and all Soracloud FHE STARK wrappers reject non-empty
	  all-zero native envelope bytes before backend verifier dispatch.
	  `RunSoracloudFheJob` and Torii signed FHE job requests now carry
	  an optional distinct full-bootstrap material proof attachment, provenance
	  signs it, and Core requires it for policy-bound full-bootstrap jobs before
	  dispatching through the active Soracloud verifier record or preverified-proof
	  cache path. Runtime admission rejects absent, mismatched, non-bootstrap, and
	  unverified fake full-material proofs, and
	  `RegisterVerifyingKey`/`UpdateVerifyingKey` admission rejects canonical
	  full-material verifier-profile drift before job execution. Job admission now
	  also requires the material proof schema digest and verifier key-material
	  commitment to match the canonical Soracloud proof schema and proof
	  attachment verifier commitment through the BFV crypto proof-profile
	  validator, and rejects
	  supplied full-material proof attachments that omit `vk_commitment` at the
	  material/profile gate before backend verifier lookup. The Rust, Swift,
	  Kotlin/JVM, and Java Android shared Soracloud BFV operation-fixture validators
	  now pin the full-bootstrap material/profile digest, verifier-key material
	  commitment, artifact-envelope digest, and statement vector so SDK/release
	  validation can reject fixture drift before
		  artifact-aware execution or proof verification. Full-mode exact
		  and bounded runtime bootstrap paths now use dedicated crypto preflight
		  helpers that validate governed material commitments, registered profile
		  digests, ciphertext shape, and exact/bounded metadata before direct
				  no-artifact entry points return the governed-artifact requirement. Crypto now also exposes a typed
				  full-bootstrap artifact bundle validator/digest and artifact-aware execution
				  preflight that bind concrete evaluator/proof-profile bytes to those governed
				  commitments. Each artifact byte field is now a Norito role/profile envelope
				  that declares the canonical circuit id, registered parameter/RNS/decomposition
				  digests, and max bootstrap depth, so malformed, role-swapped, stale-profile,
				  and empty-payload artifact attachments fail before artifact-aware output
					  execution. Coefficient-to-slot and slot-to-coefficient artifacts now carry
					  typed diagonal packed-slot linear transforms, and crypto exposes exact and
					  bounded deterministic evaluators for those transforms through the registered
							  RNS paths. The blind-rotation artifact now carries canonical packed-slot
							  rotation schedules bound to the governed accumulator artifact, and crypto
							  exposes exact and bounded registered-RNS execution helpers plus matching
							  public bound propagation that consume those governed selector schedules
							  directly. The sample-extraction artifact now carries typed source/output
							  ciphertext shape and extracted-coefficient metadata, rejects opaque,
							  wrong-slot-count, bad-component-count, or out-of-range payloads, and crypto
							  can extract the selected RLWE coefficient into a raw LWE-style sample whose
							  decrypt matches the selected `c0 + c1 * s` coefficient under the existing
							  secret polynomial basis, with exact and bounded raw-sample bound propagation.
							  Crypto now composes the governed coefficient-to-slot, blind-rotation, and
							  raw sample-extraction artifacts into an exact/bounded execution-prefix trace
							  with propagated bounds, coefficient-zero diagnostic repack output,
							  slot-to-coefficient diagnostic execution, and missing-key fail-closed checks.
							  Artifact-aware
							  exact/bounded final-output entry points now execute that prefix or its bound
							  propagation through governed sample-switch and slot-to-coefficient output,
							  so missing Galois keys and malformed executable artifacts fail before
							  output. Crypto also exposes an explicit coefficient-zero raw-sample repack
							  diagnostic bridge with exact and bounded coefficient-zero bounds, plus
							  deterministic exact and bounded raw-sample switch-key material,
							  secret-consistency checks, switch execution, public bound propagation,
							  governed artifact carriage, and artifact-aware full-bootstrap output/bound
							  helpers that run through slot-to-coefficient. Full-bootstrap
							  artifact-bundle validation now requires executable sample-extraction
							  switch-key material rather than accepting metadata-only sample-extraction
							  payloads in the governed bundle. Direct no-artifact registered entrypoints
							  now validate preflight, then fail with an explicit governed-artifact
							  requirement; the real proof verifier/prover backend remains unfinished. The
							  accumulator artifact now
							  carries typed packed-slot test-vector material and rejects opaque,
							  wrong-slot-count, malformed, or all-zero accumulator payloads. The proof
							  public-input schema and prover/verifier key artifacts now also carry typed
							  proof-profile payloads that bind the canonical backend, key format,
							  circuit id, statement-hash layout, governed schema digest, and inner
							  prover/verifier key role, with domain-separated key-material commitments
							  over the backend-native key bytes, while
							  rejecting opaque schema/key bytes, empty or all-zero key material, and
							  duplicate prover/verifier key material. Crypto now also exposes a domain-separated
							  full-bootstrap execution proof statement
							  digest that validates and binds the public key, governed bootstrap
							  key/material, concrete artifact bundle, input/output ciphertexts, exact or
							  bounded proof mode, input/output bound metadata, and execution-witness
							  digest for the verifier. The Soracloud execution proof public-input
							  schema and stable hash now advertise that witness digest, so verifier
							  records cannot retain the pre-witness claim layout by metadata accident.
							  `RunSoracloudFheJob` now carries optional full-bootstrap artifacts plus
							  an ordered execution-proof vector; provenance signs both, and Core routes
							  exact/bounded full-mode jobs through artifact-aware full-bootstrap execution
							  and bound propagation through sample-switch and slot-to-coefficient output
							  before requiring one governed execution proof per output slot.
							  Torii signed job-run requests now validate those verifier-backed proof
							  attachments locally before instruction construction, so malformed signed
							  wrappers fail as bad requests before reaching Core. Data-model and Core
							  `OpenVerifyEnvelope` admission now also reject all-zero native STARK
							  envelope bytes for Soracloud FHE input, bootstrap-key, full-bootstrap
							  material, and full-bootstrap execution proofs, and data-model proof
							  envelopes plus FHE parameter-set, execution-policy, Soracloud
							  uploaded-model, private-execution, agent-apartment/autonomy,
							  training metrics, HF source/shared-lease/violation evidence,
							  model provenance, and host request/response envelope digest fields
							  reject the zero prehash statement sentinel before verifier dispatch,
							  parameter admission, policy admission, job input admission,
							  ciphertext-state admission, uploaded-model receipt admission, or model
							  artifact admission.
							  `zk-stark` full-bootstrap fixtures now install governed
							  artifact-backed STARK verifier keys and generate backend-verified
							  binding-AIR `OpenVerifyEnvelope` payloads only as rejection fixtures:
							  the active full-bootstrap material and execution verifier gates reject
							  them before backend dispatch because they do not prove the BFV
							  bootstrap arithmetic. The bootstrap-key proof gate still has positive
							  active-verifier coverage for the shared binding-AIR verifier path.
								  Core now also exposes `zk-stark` full-bootstrap material and
								  execution proof constructors that take the canonical statement hash,
								  preflight the supplied verifier-key backend, circuit id,
								  production-floor STARK/FRI shape, SHA-256 selector, and nonzero
								  statement hash, then fail closed at the dedicated BFV full-bootstrap arithmetic prover
								  boundary until the production prover is available.
								  Companion helpers now derive those statement hashes from the
								  production BFV inputs before invoking the fail-closed constructors:
								  material proofs use the
								  refresh-transcript public key and governed evaluation-key bundle, and
									  execution proofs build one slot-indexed input/output ciphertext claim
									  per output slot with the signed bound mode and bound metadata while
									  rejecting empty input slots, missing/surplus output slots, and
									  caller-supplied verifier keys that do not match the governed
									  artifact-derived execution verifier key before proof construction.
								  Canonical Soracloud FHE STARK verifier-key admission now covers input
								  admission, bootstrap-key, full-bootstrap material, and
								  full-bootstrap execution records with a shared production-floor
								  STARK/FRI payload validator, so below-floor inline verifier keys fail
								  during `RegisterVerifyingKey`/`UpdateVerifyingKey` before runtime
								  proof verification can depend on them.
								  Full-bootstrap material proof verification also preflights the active
								  record's stored STARK/FRI verifier-key payload against the canonical
								  material circuit before backend dispatch, so corrupted state cannot
								  retarget the material verifier key to the execution circuit.
								  Governed full-bootstrap execution verifier-key artifacts also decode
								  and validate the inner STARK/FRI verifier-key payload against that
								  canonical execution circuit under `zk-stark`, so opaque, below-floor,
								  or circuit-retargeted artifact bytes fail before a governed
								  `VerifyingKeyBox` is derived.
								  Core also decodes Soracloud FHE input-admission,
								  bootstrap-key, full-bootstrap material, and full-bootstrap execution
								  native `StarkVerifyEnvelopeV1` payloads before backend verification
								  and adversarially rejects transcript-label, domain-tag, missing-AIR,
								  circuit-id, trace-width, opening-count, composition-root, and
								  public-digest drift. The governed material-native AIR verifier now
								  also has active drift coverage for transcript labels, STARK
								  parameters, trace roots, composition roots, public digests, and opened
								  composition values. For full-bootstrap material and execution proofs,
								  generic binding-AIR fixtures are fully validated before being rejected
								  at the dedicated arithmetic-AIR boundary, while non-generic AIR labels
								  remain fail-closed until the production arithmetic verifier is
								  available. Crypto-side AIR evaluation validation now recomputes the
								  trace-bound composition vector before accepting release-prover input
								  material. The `zk-preverify` path now has poisoned-cache regressions
								  for input-admission and bootstrap-key native AIR drift plus
								  full-bootstrap material-native AIR drift, execution BFV-native AIR
								  drift, required governed execution material, and material/execution
								  generic AIR drift, so cache hits cannot bypass native envelope binding,
								  verifier-owned material checks, the required material context, or the
								  dedicated arithmetic-AIR boundary.
								  `zk-preverify` full-bootstrap regressions now prove preverified cache
								  hits cannot bypass the dedicated arithmetic-AIR boundary for material
								  or execution proof batches.
							  The confidential verifier-call defaults now admit one such Soracloud
							  full-bootstrap execution batch without an operator override.
							  Torii signed job-run preflight now resolves every signed parameter-set
							  descriptor against the registered BFV profile and runs the shared
							  policy/job admission validators plus BFV evaluation-key and
							  refresh-transcript digest checks before proof/artifact validation,
							  recomputes policy proof-statement digests from the signed
							  key/transcript material, requires policy-bound bootstrap-key and
							  full-bootstrap material proofs to be present with matching statement
							  hashes, validates supplied full-bootstrap artifact bundles against
							  the governed request material before instruction construction,
							  requires full-bootstrap execution requests to carry signed circuit
							  artifacts and a non-empty execution-proof vector whose count cannot exceed
							  the signed parameter-set slot count, rejects
							  full-bootstrap material/execution proof attachments outside
							  full-bootstrap job/key context, and rejects execution proofs that omit
							  signed artifact bundle bytes. Parameter, policy, job, key,
							  transcript, digest, or descriptor drift now fails locally before proof
							  or artifact decoding.
							  Crypto now also exposes artifact-aware validation for externally
							  held full-bootstrap execution witness, proof-input, and
							  release-prover input material, recomputing the governed prefix trace
							  from concrete artifacts and Galois keys and requiring prover/verifier
							  proof-key bytes to match the governed artifacts before callers rely
							  on those packages.
							  Core's release-prover execution proof handoff now invokes that
							  artifact-aware prover-input validation before native AIR envelope
							  emission, so self-consistent stale prefix traces fail against the
							  governed artifacts.
							  Core governed execution verifier-key derivation now validates the
							  complete artifact bundle before decoding verifier-key material, so
							  drifted non-verifier artifacts fail at that helper boundary.
							  Core regressions now also prove that correctly shaped full-bootstrap
							  execution proof attachments fail closed before backend verification
							  when the governed verifier record is missing or withdrawn.
							  The Core proof helper now also reruns local job-shape validation,
							  requires input-bound metadata to match the input envelope count,
							  and rejects missing/surplus output slots before deriving proof
							  statements, so stale bound sidecars, stale output sidecars, and
							  multi-input bootstrap drift cannot reach proof verification.
							  It also rejects full-bootstrap execution circuit artifacts outside
							  full-bootstrap proof context even when no execution-proof attachments are
							  supplied, so artifact-only bypass attempts fail at the proof boundary.
							  Core regressions also pin full-bootstrap execution verifier-record
							  metadata drift across namespace, backend, curve, public-input schema,
							  circuit/version, gas schedule, active circuit mapping, proof byte caps,
							  key presence/length, commitment, and governed verifier-key byte binding.
							  Core now also forges governed verifier-key artifacts with empty and
							  all-zero backend-native key bytes, proving inert key material fails
							  before verifier-record lookup even when artifact digest/commitment
							  metadata is recalculated.
							  The typed proof-key artifact encoder now also rejects stale declared
							  key-material commitments before emission, and crypto regressions pin
							  artifact-derived commitment rejection for role swaps, depth drift,
							  stale commitments, and inert backend-native key bytes.
							  Material proof-statement regressions now also drift prover/verifier
							  key-material commitments directly, proving those commitments change
							  both the crypto statement digest and Soracloud's transcript-derived
							  policy digest.
							  Full-bootstrap execution proof statements now bind the zero-based output
							  slot index, and Core rejects slot-position replay even when duplicate
							  ciphertext slots would otherwise produce identical input/output claims.
							  Full-bootstrap jobs now also require `bootstrap_count == 1` in Core
							  execution, bound propagation, proof verification, and Torii
							  signed-request preflight, so the one-proof-per-output-slot statement
							  cannot be replayed as a multi-round full-bootstrap claim.
							  Core now also preflights full-bootstrap execution-proof material after
							  loading FHE inputs and before artifact-aware execution, rejecting proof
							  vectors whose length does not match the actual input/output slot count
							  before the heavier arithmetic path runs.
							  Exact and bounded-noise Core runtime coverage now also rejects drifted
							  signed artifact bundles, role-swapped artifact envelopes, and stale
							  prover/verifier key-material commitments before Galois-key availability
							  or final output execution.
							  Full-bootstrap proof-key payloads now also bind the canonical execution
							  public-input layout and a generated prover/verifier pair commitment;
							  governed material stores that pair commitment and Core/Torii recompute
							  it from decoded proof-key artifacts before accepting signed material.
							  Torii signed-request preflight coverage now also rejects full-bootstrap
							  artifact attachments outside full-bootstrap context and binds a matching
							  signed material digest to a role-swapped artifact envelope before
							  rejecting the wrong declared role or stale prover/verifier key-material
							  commitments locally before instruction construction.
							  The legacy no-artifact Core execution helpers are test-only, so production
							  full-mode jobs must pass through the governed artifact-aware path; the
							  no-artifact residual-bound wrapper is also test-only, keeping the
							  non-test Core path on artifact-aware execution and bound propagation.
							  Direct exact and bounded no-artifact crypto helpers now also reject drifted
							  governed full-bootstrap material before artifact availability, so stale
							  material cannot be masked by the expected artifact-required boundary.
							  Full-bootstrap artifact-bundle digests now use typed digest material with
							  version, artifact-digest count, and per-role artifact hashes, with valid
							  alternate-artifact regressions pinning every mutable artifact role that
							  can vary under the first-release profile.
							  Full-bootstrap execution proof statement tests now also pin canonical
							  exact and bounded proof-mode digest goldens for that typed artifact-bundle
							  layout.
							  Full-bootstrap proof-profile schema/key tests now flip every required
							  statement/claim binding and proof-key commitment component, pinning the
							  prover/verifier artifact contract plus canonical proof schema artifact,
							  governed parameter/profile/depth-bound prover/verifier key-material
								  commitments, and prover-key commitment digests before release-grade keys
								  are admitted. Data-model
								  tests also pin the Soracloud FHE public-input schema hashes that
								  verifier records use for input admission, bootstrap-key proof,
								  full-bootstrap material proof, and full-bootstrap execution proof gates.
								  Bootstrap-key zero-refresh proof statements now also encode a v1
								  statement-material header plus bootstrap refresh-round count,
								  zero-refresh digest, and indexed per-round refresh digests, and the
								  public-input schema hash
								  `39809de5a8ac82f115fc3df08abffb3629adbf9dd227bccf7f9816cbc86e8563`
								  advertises those transcript, refresh-summary, and exact/bounded
								  raw/transcript statement-domain plus refresh-transcript-domain bindings,
								  including the v1 refresh-transcript material header, with the schema
								  regression checking the exported crypto material and digest-domain
								  constants directly.
								  Full-bootstrap execution claims now also carry a deterministic witness
								  digest derived from the governed artifact-aware arithmetic prefix trace
								  and its exact/bounded bound trace. Core proof generation and verification
								  recompute that digest before statement hashing, so output ciphertext,
								  output-bound, or witness-digest drift fails before STARK envelope
								  verification.
								  The proof public-input schema and prover/verifier key profile now also
								  advertise and commit to the witness digest domain, witness material
								  version/count, ciphertext trace stage count, and bound trace stage count,
								  so stale witness-layout metadata fails schema validation, key validation,
								  and key-commitment checks before backend proof work.
								  Proof-key `key_material` now carries a canonical Norito envelope that
								  binds backend-native key bytes to the role, backend, key format, circuit,
								  registered BFV profile, governed schema digest, statement/claim layout,
								  witness layout, hash shape, and supported bound modes; opaque key blobs,
								  all-zero native key bytes, envelope metadata drift, and duplicate native
								  prover/verifier key material fail before governed artifact admission.
								  The envelope's `native_key_material` is now itself typed Norito material:
								  transparent STARK/FRI prover parameters are deterministic, verifier
								  payloads must decode to the canonical SHA-256/Goldilocks FRI floor, and
								  raw, opaque, payload-digest-drifted, non-SHA, or below-floor native bytes
								  fail before Core, Torii, or data-model fixtures admit the governed
								  proof-key pair. The native proof-key envelope now also carries a
								  deterministic full-bootstrap proof-circuit fingerprint, so circuit-shape
								  drift fails before governed proof-key material admission, and generated
								  pair validation rejects prover/verifier native-circuit mismatch before
								  deriving or admitting a proof-key pair commitment. Native proof-key
								  material now also rejects noncanonical native payload circuit ids
								  outright, pinning release artifacts to `iroha_bfv_full_bootstrap_v1`
								  instead of merely requiring prover/verifier pair consistency.
								  Native verifier payloads now also carry the canonical field count,
								  backend, key format, proof system, and field labels, and crypto/Core
								  validation rejects relabeled STARK/FRI verifier payloads before
								  governed proof-key material admission or artifact-derived verifier-key
								  canonicalization. Core's native-verifier fallback now also rejects
								  field-count drift before rewriting native verifier payloads into
								  canonical STARK verifier-key bytes.
								  The arithmetic trace layout is now explicit
								  `BfvFullBootstrapArithmeticTraceProfileV1` material with a canonical
								  digest bound by the proof public-input schema, proof-key material
								  envelope, native prover/verifier payloads, native proof-key material,
								  and native proof-circuit fingerprint. Crypto and Core reject
								  trace-profile digest drift before governed artifact admission or
								  verifier-key canonicalization. The profile now also binds active
								  coefficient rows as private witness rows, public deterministic padding
								  rows, and the rules that transparent native proofs must not open unmasked
									  private rows or duplicate sampled public rows; Crypto's public
									  padding-row helpers now reject zero statement hashes before constructing
									  or validating verifier-facing openings, and Core's native BFV AIR
										  boundary also validates opened public padding rows against canonical
										  statement/slot/mode headers and rejects zero statement hashes,
										  empty/all-zero AIR roots, or auxiliary generic composition-value
										  commitments before the dedicated verifier fallback.
								  Release prover input now has a typed
								  `BfvFullBootstrapMaterialProofInputMaterialV1` boundary for governed
								  full-bootstrap material proofs that binds concrete artifact bundles
								  against governed material, and the material proof public-input schema
								  and stable hash now advertise that typed input contract, including
								  governed full-bootstrap material, public-key, evaluation-key, concrete
								  artifact-bundle, statement-hash, and material proof input package
								  digest-domain bindings. Crypto also exposes a domain-separated Norito
								  digest helper for that typed material proof input package. Release prover input also
								  has a typed
								  `BfvFullBootstrapExecutionProofInputMaterialV1` boundary that binds the
								  public key, validated execution witness material, and canonical statement
								  hash before a dedicated arithmetic prover can consume the material.
								  Release execution prover input now also has a typed
								  `BfvFullBootstrapExecutionProverInputMaterialV1` package that binds the
								  proof input, canonical row-major arithmetic trace material/digest,
								  canonical AIR contract digest, governed AIR artifact digest,
								  zero-residual AIR evaluation material/digest, and governed generated
								  prover/verifier proof-key pair before the dedicated prover boundary.
								  Crypto and Core reject stale trace digests, stale AIR
								  contract/artifact/evaluation material digests, non-zero composition
								  values, stale trace rows, trace/proof-input splicing, and unrelated
								  proof-key material or pair commitments before proof generation is
								  attempted. Core proof-emitting material and execution helpers are now
								  internal, so the callable production material and batch paths validate a
								  release audit package against governed material, concrete artifacts, the
								  caller-trusted reviewer id/key, and caller-pinned package digest, including
								  zero and known placeholder pinned-digest rejection; the
								  internal typed prover-input path still requires the caller-supplied
								  verifier key to match the verifier proof key embedded in the release
								  prover package. Core now canonicalizes governed
									  native verifier-key payloads before caller/prover-input and
									  helper/governed-artifact comparisons, so native BFV verifier-key artifacts
									  and canonical STARK boxes follow the same binding path. The proof public-input schema and Soracloud stable
									  schema hash now also advertise release-prover verifier-key binding,
									  BFV arithmetic AIR contract layout/enforcement flags, including
									  row-kind partitioning, active-row/witness consistency,
									  full-bootstrap arithmetic constraints, nonzero statement
									  hashes, and trace output/bound claim matching, and the duplicate-free
									  native opening policy, execution proof input package digest domain,
									  release-prover AIR constraint-system digest/artifact binding, and the
									  typed crypto schema validates those AIR, release-prover, and execution
									  proof input package digest-domain
									  advertised AIR/release-prover terms directly. The
									  AIR constraint-system digest is also bound through the typed public
									  schema, native prover/verifier payloads, proof-key material envelope,
									  native proof-key material, and native proof-circuit fingerprint. The
									  AIR constraint-system material is now a public typed Norito artifact
									  with a canonical validator and digest-from-material helper for
									  release tooling.
								  Core typed material proof helpers now derive and validate typed
								  material before emitting a material-native STARK/FRI proof; the
								  hash-only material constructor remains fail-closed at the
								  dedicated-prover boundary. The test-only typed execution proof helper also
								  derives and validates the canonical
									  row-major arithmetic trace material from proof input, so stale governed
									  material, witness, statement material, or native trace rows are rejected
									  before proof generation is attempted. The native AIR fixture path now
									  uses a deterministic STARK/FRI envelope builder that commits
									  caller-validated trace rows and explicit typed AIR evaluation
									  composition values, and the Soracloud release-prover handoff builds
									  that BFV-native envelope directly from
									  `BfvFullBootstrapExecutionProverInputMaterialV1`. The active Soracloud
									  verifier path now reconstructs the governed arithmetic trace and AIR
									  evaluation material from the public execution proof input and rejects
									  trace/composition root drift plus opened rows, next rows, or composition
									  values that do not match that verifier-derived material before
									  Merkle/FRI validation or the dedicated-verifier fallback. The BFV AIR
									  composition challenge stream now binds the public statement hash,
									  canonical row-major trace-material digest, row index, and column
									  index, remaps zero challenges to one, and the typed AIR contract plus
									  data-model Soracloud execution proof public-input schema advertise
									  that exact challenge domain and binding policy with AIR material field
									  count 32 and refreshed stable schema hashes. The shared explicit STARK AIR
									  builder now self-verifies generated row/composition envelopes before
									  returning proof bytes to BFV native AIR callers, and the Soracloud
									  release-prover handoff replays encoded envelope bytes against the exact
									  typed trace rows and AIR evaluation composition values before returning
									  proof bytes. Remaining native-AIR
									  production work is the BFV arithmetic proof-producing backend plus
									  release-grade generated prover/verifier artifacts and audit evidence,
									  not hand-built roots, openings, unbound
									  composition vectors, or statement-only composition challenges.
									  Crypto release tooling can now derive governed full-bootstrap circuit
									  material directly from concrete artifact bundles by recomputing every
									  artifact digest, proof-key material commitment, and generated pair
									  commitment before validating the bundle against the derived material.
									  Standalone release audit evidence validation also recomputes the
									  evaluator-artifact-set digest, full artifact-bundle digest, and
									  canonical native proof-circuit fingerprint from its advertised fields,
									  so stale-but-distinct digest summaries or a matched stale
									  prover/verifier fingerprint pair cannot pass as shape-valid release
									  evidence. Standalone signoff payloads and machine-checkable manifests
									  now also recompute that canonical native proof-circuit fingerprint from
									  the release circuit id before accepting or digesting the object.
									  Stale proof-key pair commitments are rejected during derivation even
									  when individual proof-key material commitments are refreshed, and the
									  crypto/Core sample material helpers now fail hard instead of
								  synthesizing malformed sample pair commitments.
									  Core now also requires the full-bootstrap material proof verifier record
									  to carry the canonical material-proof gas schedule id and has
									  adversarial verifier-record drift coverage matching the execution-proof
									  gate. Input-admission and bootstrap-key proof verifier records now
									  likewise require their canonical gas schedule ids rather than any
									  non-empty schedule id. Full-bootstrap material and execution proof
									  statements now encode the advertised statement material version and
									  field count in the canonical hashed bytes, and the Soracloud public-input
									  schemas advertise those self-describing statement headers. The execution
									  public-input schema and stable hash now also advertise the arithmetic
									  trace private/public row policy and proof-key-bound release prover input
									  package.
									  Full-mode bootstrap keys now carry a domain-separated BFV public-key
									  digest, and material/execution statement derivation rejects governed
									  public-key drift before hashing or proof-helper execution. Execution
									  witness material validation also recomputes the artifact-bundle digest
									  implied by governed full-bootstrap material commitments, including the
									  arithmetic AIR constraint-system artifact digest, so nonzero stale
									  artifact-bundle digests fail before public witness hashing or release-prover
									  input packaging, reconstructs the raw extracted sample and raw-sample bound
									  from the blind-rotation stage, and recomputes the deterministic
									  coefficient-zero repack ciphertext plus the coefficient-zero and
									  sample-switch bounds from the raw extracted sample before accepting typed
									  witness material.
									  BFV-shaped native AIR envelopes now preflight the canonical
									  transcript label, statement-bound domain tag, STARK/FRI metadata,
											  public digest binding, proof/commitment version tags,
												  commitment/root shape, exact duplicate-free canonical opening/query
												  count, opened row/path shape, Merkle path-to-root binding,
												  FRI query-chain Merkle/fold validation, auxiliary
											  generic composition-value commitment rejection, AIR-to-FRI
											  base value binding, execution public-padding context, opened public
										  padding-row semantics, and the no-unmasked-private-row plus
										  duplicate-free opening policies before the current dedicated verifier
										  boundary is reported;
										  non-generic full-bootstrap native envelopes with missing, foreign, or
										  contextless BFV AIR sections now fail before that unavailable-verifier
										  boundary. The active Soracloud execution verifier now reconstructs the
										  governed arithmetic trace and AIR evaluation material from public proof
										  input and requires those governed rows plus composition values before
										  explicit STARK/FRI replay, so missing governed material, root drift, or
										  opened row/next-row/composition drift fails closed before verifier
										  acceptance.
								  Refresh-only proof and execution paths still reject `FullBootstrapV1`.
										  Remaining work is the audited full-bootstrap arithmetic witness
									  constraint/proof-producing backend plus release-grade generated
										  proving/verifying artifacts for the actual BFV bootstrap circuit, not
										  the already-shipped Core verifier, proof-key, public-schema/release-prover
										  input, release-prover arithmetic digest sentinel rejection,
										  canonical trace/AIR digest sentinel rejection,
										  arithmetic-trace, AIR contract material/digest binding,
										  typed-witness, typed AIR evaluation material trace-digest sentinel rejection,
										  audited release-package wrapper, release-audit transcript-inventory
										  preflight, native AIR, attachment-finalization,
										  statement-recomputation, verifier-record/verifier-artifact admission-floor,
										  or governed proof-key-pair validation corridors documented above.
	  Soracloud transcript digesting now preflights the advertised BFV public-key
	  shape before evaluation-key bundle validation, so malformed transcript key
	  material is reported at the public-key boundary instead of being masked by
	  unrelated bundle-shape errors. The crypto bundle validator
  applies the same public metadata
  preflight for direct callers. Standalone refresh-key transcript
  generators/validators also reject empty or oversized public seeds before
  deriving or recomputing encrypted-zero masks. Soracloud FHE execution
  policies now carry the refresh-transcript inventory digest,
  `RunSoracloudFheJob` signs the transcript inventory in the provenance
  payload, and core rejects jobs whose supplied refresh transcript does not
  match the governance-bound digest. This hardens the current refresh path
  while the full BFV bootstrapping engine remains open. The same
  bundle-level owner diagnostic now verifies relinearization entries against
  scaled `s^2` residues and Galois entries against scaled automorphed-secret
  residues, rejecting non-plaintext-multiple key-switch residuals and residual
  multiples above the current exact error bound; standalone Galois key
  generation now applies that residual self-check before returning key
  material. Rotation and bootstrap encrypted-zero refresh diagnostics now also
  reject zero-plaintext masks whose residual multiples exceed the deterministic
  `(2n + 1)E` refresh bound for the first-release seeded encryption format. The
  bounded-noise counterparts now also reject zero-plaintext
  rotation/bootstrap refresh masks whose centered rounded noise exceeds the
  fresh BFV noise bound, and bundle-level bounded diagnostics now identify
  indexed rotation/bootstrap refresh masks when nonzero plaintext or oversized
  rounded noise is detected.
  Seeded key generation and public-key encryption now also reject parameter
  sets whose centered `q/t` capacity is below that bound, so structurally valid
  but too-narrow profiles cannot produce first-release ciphertext/key material,
  and deterministic BFV keygen, encryption, Galois-key generation, and
  identifier seed helpers reject empty or oversized seeds before deriving RNG
  material; Soracloud refresh transcript admission derives its public seed,
  bootstrap key-id, and rotation inventory caps from the same crypto constants.
  Registered BFV profile validation and the production digest path now enforce
  the same capacity invariant before admitting the RAM-LFE profile. A separate
  rounded BFV path now generates small-noise public keys, encodes plaintexts as
  `(q / t) * m`, decrypts by deterministic rounding, rejects too-narrow
  decoding capacity, and reports owner-side centered-noise/headroom profiles as
  the migration entry point for the pending BFV-RNS evaluator; rounded
  ciphertext add/subtract, rounded plaintext-scalar addition, plaintext-scalar
  multiplication, and plaintext-polynomial multiplication also have checked
  centered-noise bound propagation. Rounded ciphertext-ciphertext
  multiplication now has a scalar exact-product bridge that performs `t/q`
  scale-and-rounding, bounded-noise relinearization, and conservative output
  budget validation, and rounded Galois key switching now has small-noise key
  generation, secret-key consistency checks, automorphism application, and
  output-bound propagation. Rounded packed `RotateLeft` now also wires those
  bounded-noise Galois switches through the public packed-selector schedule
  with matching output-bound validation before the final RNS basis-extension
  pipeline lands. RNS polynomials now also have an exact CRT basis-extension
  bridge between validated chains with target-product coverage checks, giving
  the BFV-RNS evaluator a deterministic reconstructable conversion primitive
  alongside the target-limb key-switch path. A deterministic target-limb
  basis-extension helper now computes the CRT quotient correction exactly with
  integer arithmetic and reduces source representatives into target limbs
  without requiring the target product to cover the source product; narrow
  target reconstruction remains visibly lossy. Key-switch
  components now decompose directly into RNS digit polynomials, exact RNS key
  switching consumes those digits internally, and basis-extended digit inputs
  are validated against canonical decomposition ranges before use. An explicit
  target-limb basis-extension key-switch path now decomposes in a source
  chain, rejects decomposition chains that can alias base digits,
  basis-extends canonical key-switch digits through the digit-specific
  basis-extension helper without requiring the evaluator target to cover the
  full source-chain product, rejects basis-extended digit-count and RNS
  limb-shape drift at validation, and drives rounded multiplication, Galois,
  and packed `RotateLeft` bridges while matching the scalar bounded-noise
  outputs. Direct key-switch component decomposition and digit
  basis-extension helpers now enforce source/target decomposition-base
  coverage before malformed polynomial shapes can mask the public chain
  descriptor failure.
  Rounded ciphertext multiplication now has an RNS exact raw-product bridge
  that decomposes ciphertext components as centered residues, reconstructs
  signed negacyclic products before `t/q` scale-and-rounding, and relinearizes
  the scaled quadratic component through the RNS digit/key-switch path while
  matching the scalar bounded-noise multiplication output. The RNS chain now
  also exposes an explicit exact scale-round helper for centered RNS product
  polynomials at the rounded BFV `t/q` boundary, and rounded RNS ciphertext
  multiplication uses that helper for direct product components plus a centered
  two-product sum helper for `c1` cross terms, with exact product-sum coverage
  rejecting aliasing before scale-and-rounding. Rounded Galois key
  switching and packed `RotateLeft` now also have RNS exact bridge entry points
  that match the scalar bounded-noise schedule and reject too-narrow chains.
  Outer-slot rotation and bootstrap refresh material can now also be generated
  and publicly transcript-validated with rounded bounded-noise encrypted-zero
  ciphertexts, refreshed through scalar or exact RNS addition, routed through
  registered target-limb RNS basis-extension wrappers for bounded production
  Bootstrap execution, and propagated with centered-noise output bounds.
  Evaluation-key bundles can now validate and digest the bounded-noise
  rotation/bootstrap transcript inventory under a separate domain from the
  exact-lift refresh path, and owner diagnostics can validate bounded relin/Galois key-switch residuals with bundle-owned
  relinearization labels and bundle-indexed Galois diagnostics plus every
  bounded refresh mask in one bundle check.
  Soracloud FHE execution policies now bind the
  refresh transcript mode, data-model digesting routes through exact-lift or
  bounded-noise transcript derivation explicitly, and core runtime admission
  rejects mode/digest mismatches before job execution. Soracloud bounded-noise
  jobs now dispatch to the bounded-noise RNS bridge for Add, outer
  `RotateLeft`, and encrypted-zero Bootstrap refresh when policy/input metadata
  are explicitly bounded, while Multiply and packed `RotateLeft` now call
  registered `iroha_crypto` helper entry points that select the smallest
  registered key-switch decomposition prefix inside the crypto layer before
  invoking the target-limb basis-extension bridge. The crypto layer now exposes
  that registered decomposition chain plus a role-separated digest so runtime
  and admission paths can share the canonical target-limb key-switch source
  basis. Registered helper entry points for bounded-noise Multiply, Galois key
  switching, and packed `RotateLeft` now derive both the canonical evaluator
  chain and source basis inside `iroha_crypto` before invoking the target-limb
  bridge, so runtime callers no longer pass evaluator RNS chains into those
  registered bounded-noise entry points. The explicit basis-extension
  key-switch path now rejects decomposition source chains that are not
  evaluator-chain prefixes while leaving the lower-level target-limb residue
  conversion primitive available for checked RNS arithmetic. Soracloud FHE
  parameter governance plus input-admission statement
  hashes now bind that digest beside the parameter and evaluator RNS-chain
  digests. Portable FHE input-admission proof validation now rejects cheap
  attachment metadata before BFV bound capacity (backend consistency,
  canonical verifier id, verifier-key commitment metadata, and envelope-hash
  presence), while still rejecting over-capacity BFV bounds before decoded
  `OpenVerifyEnvelope` admission, verifier dispatch, and verifier-record
  lookup. Soracloud registered bounded-noise runtime coverage now also drives
  two-round Bootstrap through the registered RNS refresh bridge, verifies the
  decrypted multi-slot output, and checks the propagated key-authorized
  centered-noise bound at the runtime boundary; the same bounded wrapper
  coverage now pins Multiply and packed `RotateLeft` propagated output bounds
  while decrypting the registered target-limb outputs, and ledger-level
  `RunSoracloudFheJob` coverage now persists bounded Multiply, packed
  `RotateLeft`, and two-round Bootstrap output rows with the expected bound
  mode, bound value, payload commitment, and decrypted plaintext. The crypto layer now owns
  scalar and exact-RNS multi-round Bootstrap refresh helpers for exact and
  bounded-noise ciphertexts, rejects zero or over-capacity refresh counts before
  applying any round, and single-round scalar/RNS refresh helpers now preflight
  requested round indices before ciphertext addition. Soracloud routes exact
  and bounded-noise Bootstrap jobs plus shared operation-vector checks through
  those helpers. Registered
  exact and bounded-noise Add/Subtract, exact and bounded-noise Multiply,
  exact and bounded-noise plaintext-polynomial selector products, exact and
  bounded-noise affine row evaluators, exact and bounded-noise packed
  `RotateLeft`, outer-slot `RotateLeft`, and round-zero, indexed-round, and
  consecutive-round Bootstrap refresh helper entry points now derive the
  canonical evaluator RNS chain inside `iroha_crypto`, and Soracloud exact and
  bounded-noise runtime dispatch uses those helpers instead of passing the
  chain through core. The registered-helper rejection regression now also
  covers the decomposition-chain helpers plus exact and bounded-noise
  Subtract, exact and bounded-noise plaintext-polynomial selector products,
  exact and bounded-noise affine row evaluators, exact and bounded-noise
  Bootstrap refresh forms, and the bounded target-limb Multiply, Galois, and
  packed `RotateLeft` entry points, proving structurally
  valid but unregistered profiles fail closed before caller-supplied key
  material is inspected. Direct exact-RNS bounded-noise Add/Subtract,
  affine-row, outer-slot `RotateLeft`, and Bootstrap refresh helpers now share
  a rounded-decoding plus exact-addition RNS corridor preflight before
  supplied-chain accumulation, refresh-key checks, or ciphertext-shape checks,
  while direct exact-RNS bounded-noise Multiply, Galois key-switch, and packed
  `RotateLeft` fallback helpers now also have registered production wrappers,
  so exact-reconstruction and target-limb basis-extension paths both derive
  canonical evaluator chains before inspecting caller-controlled key material.
  Bounded-noise RNS
  packed-selector products now also route through a bounded
  plaintext-polynomial RNS helper with a registered production wrapper, so
  packed `RotateLeft` mask multiplication shares the same rounded-capacity
  preflight in direct RNS, target-limb basis-extension, and registered
  target-limb paths. Public scalar addition and multiplication now also expose
  exact and bounded-noise registered helper entry points that derive the
  canonical BFV evaluator chain before plaintext/ciphertext checks, so public
  plaintext terms fail closed on unregistered profiles; the bounded scalar
  path still preflights rounded decoding capacity before applying public terms
  to bounded ciphertexts. Bounded public affine rows now reuse those helpers with
  registered RNS accumulation and owner-side rounded-noise row-bound
  propagation, so weighted public-row evaluation no longer has only an
  exact-lift surface. Bounded registered Add/Subtract, outer-slot `RotateLeft`,
  and multi-round Bootstrap wrappers now derive the registered evaluator chain
  before bounded-noise capacity checks, keeping production rejection on the
  governed profile gate for structurally valid but unregistered profiles.
  Public bounded-noise output-bound propagation now also preflights fresh
  rounded BFV noise capacity before public arithmetic, key-switch, affine,
  rotation, or bootstrap bound math, so inadmissible rounded profiles fail
  consistently across admission helpers. Scalar bounded-noise ciphertext
  multiplication now also preflights that fresh rounded capacity before
  operand or relinearization-key shape checks, matching the exact-RNS bounded
  multiply bridge, and bounded refresh-transcript validation applies the same
  preflight before bundle key-shape checks. Key-authorized bounded Bootstrap
  output-bound admission now also rejects too-narrow rounded profiles before
  bootstrap-key shape checks and shares the bootstrap round-count validator,
  and the exact residual-bound counterpart now rejects oversized input residual
  bounds or invalid refresh-round metadata before bootstrap-key shape checks.
  Exact and bounded multiply bound propagation now rejects oversized public
  input/output bounds before validating caller-supplied relinearization key
  material.
  Exact and bounded add bound propagation now validates supplied public input
  bounds before enforcing the minimum two-input shape, so oversized bound
  metadata cannot be hidden by an undersized input list.
  Exact and bounded plaintext-polynomial bound propagation now rejects
  oversized public input bounds before validating caller-supplied plaintext
  polynomial shape.
  Exact and bounded Galois key-switch bound propagation now also rejects
  oversized public input bounds before Galois-key shape checks.
  Exact/bounded packed `RotateLeft` bound propagation now also rejects
  oversized public input bounds or invalid rotation schedules before validating
  caller-supplied Galois key sets, and the exact, RNS, bounded-noise, and
  bounded basis-extension execution helpers now perform the same public
  schedule preflight before ciphertext or Galois-key shape checks.
  Exact/bounded outer-slot `RotateLeft` bound propagation now rejects oversized
  public input bounds or full-cycle rotations before validating
  caller-supplied rotation-key refresh ciphertexts.
  Exact/bounded public affine bound propagation now rejects oversized public
  input bounds before validating caller-supplied circuit row and coefficient
  shape, and exact, registered RNS, bounded RNS, and registered bounded affine
  execution helpers now validate public circuit metadata before malformed input
  ciphertext shapes.
  Exact, registered RNS, bounded-noise, direct RNS, and bounded basis-extension
  Galois key-switch execution helpers now validate public automorphism metadata
  before malformed ciphertext shapes.
  Exact, registered RNS, bounded-noise, direct RNS, and registered bounded
  public scalar/plaintext-polynomial execution helpers now validate scalar
  ranges and plaintext coefficient metadata before malformed ciphertext shapes.
  Exact and bounded-noise seeded encryption, plus identifier envelope
  encryption, now validate public plaintext/input, non-empty non-all-zero
  deterministic seed, and identifier envelope metadata before malformed
  public-key shapes.
  Exact/bounded plaintext-scalar bound propagation now rejects oversized public
  input bounds before validating the public scalar range.
  Key-authorized bounded-noise bootstrap output-bound propagation now rejects
  oversized public input bounds or zero-round requests before validating full
  bootstrap-key ciphertext shape.
  Direct exact/bounded bootstrap refresh output-bound propagation now validates
  supplied public input bounds before rejecting zero-round requests, so
  oversized input-bound metadata cannot be hidden by invalid direct refresh
  counts.
  Bounded full-bootstrap linear-transform, raw-sample, and sample-switch bound
  propagation now preflights public artifact metadata before rounded-capacity
  errors while keeping full switch-key entry validation after the capacity
  gate.
  Direct no-artifact bounded full-bootstrap execution and bound helpers now
  preflight FullBootstrapV1 key/material metadata before rounded-capacity
  errors, and artifact-aware bounded full-bootstrap prefix execution/bound
  helpers share that key/material preflight before concrete artifact or
  ciphertext validation.
  Bounded raw-sample coefficient-zero repack and owner diagnostic helpers now
  reject malformed raw-sample metadata before rounded-capacity errors.
  Bounded raw-sample extraction and sample-switch execution helpers now do the
  same for sample/key metadata and key/sample consistency before inspecting
  ciphertexts or full switch-key entries.
  Bootstrap refresh execution now also validates public key metadata plus
  requested round index/count before full refresh-key ciphertext shape across
  scalar, bounded-noise, direct RNS, and registered RNS paths, so malformed
  `round_refreshes` vectors cannot mask out-of-capacity refresh requests.
  Packed `RotateLeft` execution helpers now also preflight Galois key-set
  public metadata before ciphertext shape while keeping full key-switch entry
  validation after ciphertext shape.
  Evaluation-key bundle validation and digest admission now preflight public
  rotation, Galois, and bootstrap inventory metadata before malformed
  relinearization or refresh/key-switch entry shapes.
  Relinearized ciphertext multiplication now preflights public
  relinearization-key digit counts before malformed ciphertext operands across
  exact, RNS, bounded-noise, and bounded basis-extension paths while keeping
  full key-switch entry-polynomial validation after operand-shape checks.
  Direct exact/bounded refresh-transcript validation and digest admission now
  preflight transcript metadata, then advertised public-key shape, before
  evaluation-key bundle validation.
  Owner-side decrypt/profile/residual and bounded-noise diagnostics now validate
  ciphertext shape before secret-key shape, and exact/bounded rotation and
  bootstrap refresh-key generators validate public metadata, non-empty
  non-all-zero deterministic seeds, and public-key shape before deriving
  encrypted-zero refresh masks.
  Soracloud exact and bounded-noise multiply metadata wrappers now preflight
  declared public bounds before their own multiply-arity checks, so oversized
  single-input metadata reports the bound-capacity failure instead of a wrapper
  shape error.
  Soracloud FHE parameter-set admission now rejects non-BFV schemes and
  unregistered BFV backend labels at the shared data-model layer, and
  execution-policy admission now rejects unsupported deterministic rounding
  modes, so first-release BFV manifests cannot carry ignored scheme, backend,
  or rounding metadata.
  Exact and bounded Galois keygen now rejects invalid public automorphism powers
  and non-empty non-all-zero deterministic seed metadata before malformed
  secret-key shapes, and exact/bounded public-key consistency diagnostics reject
  malformed public keys before malformed secret keys. Bounded
  relinearization/Galois consistency
  diagnostics now also reject malformed public evaluation keys before malformed
  owner secrets, and bounded decrypt/profile/ciphertext diagnostics plus
  rotation, bootstrap, and bundle zero-refresh owner diagnostics reject
  too-narrow public rounded BFV profiles and oversized public rounded-noise
  bounds before malformed owner secrets. Exact residual-bound owner diagnostics
  now also reject oversized public residual bounds before malformed owner secrets
  while keeping ciphertext-shape preflight first. Exact bundle/rotation/bootstrap
  zero-refresh owner diagnostics now reject too-narrow public seeded-refresh
  residual profiles before malformed owner secrets while keeping refresh
  ciphertext-shape preflight first. Registered exact and bounded bootstrap
  refresh wrappers now also have round-index/count preflight coverage before
  malformed bootstrap-key or ciphertext shapes, and exact scalar/RNS bootstrap
  execution rejects too-narrow public seeded-refresh profiles before applying
  refresh masks. Exact and bounded direct and bundle refresh-transcript
  admission now preflights public capacity before malformed public-key,
  bundle-key, or refresh-ciphertext entry shapes. Scalar bounded bootstrap
  execution now rejects invalid public key-id and refresh-round requests before
  rounded-capacity failures. Bounded rotation/bootstrap refresh-key generation
  now rejects public step, key-id, round-count, and transcript seed metadata
  before rounded-capacity failures. Exact and bounded seeded keygen/encryption
  now reject public seed and plaintext metadata before exact residual or rounded
  capacity failures. Bounded Galois key generation now rejects public
  automorphism and seed metadata before rounded-capacity failures.
  Bounded Galois switch and packed `RotateLeft` execution wrappers now reject
  public Galois-key metadata, rotation schedules, and key-set metadata before
  rounded-capacity failures. Bounded outer `RotateLeft` execution wrappers now
  reject public rotation metadata before rounded-capacity failures. Bounded
  affine execution wrappers now reject public circuit metadata before
  caller-supplied RNS/capacity corridor failures. Key-authorized exact and
  bounded bootstrap bound propagation now rejects public bootstrap key-id and
  round-count metadata before caller input-bound failures while preserving full
  refresh-key shape validation after public bound checks. Bounded
  plaintext scalar and polynomial execution wrappers now reject public
  scalar/plaintext metadata before rounded-capacity failures. Bounded scalar and
  plaintext-polynomial bound propagation now rejects invalid public
  scalar/plaintext metadata before rounded-capacity failures while preserving
  oversized input-bound precedence on otherwise valid profiles. Bounded
  ciphertext multiplication bound propagation now rejects invalid public
  relinearization-key metadata before rounded-capacity failures while preserving
  oversized input-bound precedence on otherwise valid profiles. Bounded Galois
  key-switch and packed `RotateLeft` bound propagation now reject invalid public
  Galois metadata, rotation schedules, and key-set metadata before
  rounded-capacity failures while preserving oversized input-bound precedence on
  otherwise valid profiles. Bounded affine, outer `RotateLeft`, and bootstrap
  refresh bound propagation now rejects invalid public circuit, rotation,
  round-count, and bootstrap-key-id metadata before rounded-capacity failures
  while preserving the existing valid-profile precedence: oversized input bounds
  remain first for affine, outer-slot, and direct bootstrap bounds, and
  key-authorized bootstrap bounds keep key-id/round metadata ahead of full
  bootstrap-key shape.
  Soracloud BFV refresh-transcript admission now also derives its deterministic
  seed, bootstrap key-id, rotation-transcript, and bootstrap max-round caps from
  the public `iroha_crypto` constants.
  Verifier-backed bounded-noise
  FHE input-admission envelopes now persist bounded metadata after
  bound-capacity, statement-hash, shared `OpenVerifyEnvelope` admission-shape,
  active-verifier, and backend proof checks; the data-model proof validator now
  also rejects exact and bounded-noise input-admission bounds that exceed
  registered RAM-LFE BFV capacity before runtime admission, and persisted FHE
  state rows now reject exact or bounded bound metadata that exceeds the same
  registered capacity. FHE input-admission proof attachments now also require
  `vk_ref.name` to be the canonical v1 circuit id, a supported STARK/FRI v1
  proof backend label from the shared data-model ZK classifier, a decoded STARK
  `OpenVerifyEnvelope` with the canonical v1 circuit/schema, a
  v1 STARK public-input wrapper whose single public input matches the proof
  `statement_hash`, a `vk_commitment` that matches the embedded
  `OpenVerifyEnvelope.vk_hash`, and an `envelope_hash` that matches the embedded
  `OpenVerifyEnvelope` bytes at both data-model validation and Soracloud
  runtime admission; the Core attachment helper now applies the shared
  structural guard before decoding the envelope, and core runtime admission and
  backend pre-verification now also reject matching but unsupported STARK/FRI
  backend labels and portable but non-canonical FHE circuit ids before
  verifier-record lookup. Data-model validation and Core runtime preverification
  now also share Soracloud-specific byte caps for the encoded `OpenVerify`
  envelope, STARK public-input wrapper, and backend-native STARK envelope bytes,
  so proof-carrying ciphertext admission cannot alias the verifier id,
  omit/forge the verifier-key, statement, circuit, or envelope binding, or push
  unbounded proof bytes toward verifier lookup.
  The backend verifier now decodes the
  `OpenVerifyEnvelope` from the attachment proof bytes itself, then re-checks
  the STARK envelope shape, public-input schema, statement public input,
  verifier-id and attachment bindings, plus the single supported v1 verifier
  record version, before verifier lookup, so direct verifier use cannot bypass
  the envelope or statement-hash preflight. Data-model validation, Core
  envelope validation, and backend preverification now also reject empty
  backend-native STARK `envelope_bytes`, so admission cannot carry only
  statement metadata without a native proof envelope. Data-model validation and
  Core runtime admission now also share the exported Soracloud
  `OpenVerifyEnvelope` bounds helper, keeping outer envelope, STARK wrapper,
  canonical circuit/schema ceilings, and auxiliary-byte policy in one place
  before verifier lookup. The Core FHE input-admission verifier helper now also
  recomputes the actual payload length and payload commitment before BFV shape
  checks, statement-hash derivation, envelope validation, or verifier lookup, so
  direct helper use cannot bypass mutation-executor payload metadata binding.
  FHE job execution admission now computes deterministic
  output payload-size projections with checked `u64` arithmetic and rejects
  overflow before comparing the projection with `max_ciphertext_bytes`; the
  legacy infallible projection helper remains conservative by returning
  `u64::MAX` for unrepresentable projections. Direct service-state upserts and
  FHE job output persistence now share checked binding state-total projection,
  so inconsistent existing-item accounting and `u64` total overflows fail
  closed before max-total admission checks. The production bounded-noise
  admission circuit/prover rollout, broader target-limb BFV-RNS evaluator
  hardening, and audited full-bootstrap proof-producing/verifier artifacts
  remain pending.
  Owner-side
  evaluated-output diagnostics can now validate ciphertexts against
  caller-declared exact residual-multiple bounds and reject plaintext-preserving
  residual inflation, with checked helper APIs deriving exact add-output and
  public bootstrap refresh-output bounds before those diagnostics run; the same
  helper surface now covers exact subtract, plaintext addition,
  plaintext-scalar multiplication, plaintext-polynomial multiplication, and
  public affine-circuit row propagation. Outer ciphertext-slot `RotateLeft`
  now also propagates rotated per-slot bounds and one public encrypted-zero
  refresh bound per output slot. Packed `RotateLeft` now also propagates
  conservative exact bounds through the current Galois key-switch bridge,
  plaintext-mask products, and schedule addition, with capacity rejection for
  too-narrow profiles. Soracloud FHE state rows now carry optional exact
  residual-multiple metadata, and `RunSoracloudFheJob` persists propagated
  Add, balanced Multiply/relinearization, outer/packed `RotateLeft`, and
  Bootstrap bounds for chained jobs while rejecting missing or over-capacity
  input bounds before execution. The exact packed `RotateLeft` runtime
  regression now decrypts the scheduled packed output and asserts the persisted
  conservative residual bound. Client FHE state mutations without
  proof-carrying input admission remain metadata-free and cannot feed FHE jobs.
  Proof-carrying Upsert mutations now have a canonical Soracloud FHE
  input-admission statement, provenance binding, STARK/FRI verifier-key lookup,
  canonical V1 circuit binding, restored verifier metadata drift checks,
  ciphertext-shape validation, registered identifier slot-cap enforcement, and
  residual-capacity check before core persists their residual metadata; FHE job
  input loading applies the same slot cap to persisted rows before execution.
  Public production circuit and governed key-material
  rollout for that noise-admission proof remains part of the pending BFV-RNS
  engine.
  BFV key generation now self-checks freshly generated public keys by
  verifying that `b + a*s` is a plaintext-modulus multiple within that bound,
  and checks generated relinearization entries before returning key material;
  public-key owner diagnostics reject shape-valid wrong-secret,
  non-plaintext-multiple, or oversized residuals before publication, while
  public refresh-transcript admission now rejects empty or unbounded seed
  inventories, zero/duplicate rotation steps, noncanonical bootstrap key-id
  metadata, and zero/over-budget bootstrap round metadata before recomputation
  and digest comparison. Public admission
  still needs proof-carrying key-material checks.
  Plaintext, ciphertext, polynomial, Galois-power, affine-circuit, and
  RNS-polynomial shape validators now use the same parameter preflight before
  inspecting caller-controlled shapes. BFV parameter admission now also
  requires enough ciphertext-modulus headroom to keep the deterministic
  `+t`/`-t` error representatives distinct under the configured error bound.
  Key-switch decomposition digit counting now validates parameters and uses
  checked coverage arithmetic, so malformed profiles cannot silently saturate
  relinearization/Galois digit generation; multiply and key-switch residual
  admission also uses checked `t - 1` and decomposition-base-minus-one bounds.
  Secret-key diagnostics now also expose the exact centered residual multiples
  and remaining centered-modulus headroom for the current plaintext-lift
  evaluator, while full bounded-RLWE noise budgeting remains part of the
  pending BFV-RNS engine.
  Outer ciphertext-slot `RotateLeft` now also
  rejects empty slot lists and full-cycle step counts before applying
  rotation-key refresh material, and exact, registered RNS, bounded-noise, and
  bounded RNS execution helpers preflight that public metadata before refresh
  key or slot ciphertext shapes. Packed `RotateLeft` execution helpers now also
  reject invalid public rotation schedules before ciphertext or Galois-key
  shape checks across exact, RNS, bounded-noise, and bounded basis-extension
  paths, and now preflight Galois key-set public metadata before ciphertext
  shape while keeping full key-switch entry validation after ciphertext shape.
  Evaluation-key bundle validation and digest admission now preflight public
  rotation, Galois, and bootstrap inventory metadata before malformed
  relinearization or refresh/key-switch entry shapes.
  Relinearized ciphertext multiplication now preflights public
  relinearization-key digit counts before malformed ciphertext operands across
  exact, RNS, bounded-noise, and bounded basis-extension paths while keeping
  full key-switch entry-polynomial validation after operand-shape checks.
  Direct exact/bounded refresh-transcript validation and digest admission now
  preflight transcript metadata, then advertised public-key shape, before
  evaluation-key bundle validation.
  Galois key-switch execution helpers now reject invalid public
  automorphism metadata before ciphertext shape checks across exact, RNS,
  bounded-noise, registered, and basis-extension paths. Public affine execution
  helpers now likewise reject invalid row or
  coefficient metadata before input ciphertext shape checks across exact,
  registered RNS, bounded RNS, and registered bounded paths. Public
  scalar/plaintext-polynomial execution helpers now reject invalid scalar ranges
  or plaintext coefficient metadata before ciphertext shape checks across exact,
  RNS, bounded-noise, and registered bounded paths. Seeded exact/bounded
  encryption and identifier envelope encryption now reject invalid public
  plaintext/input, deterministic seed, and identifier envelope metadata before
  public-key shape checks. Owner-side decrypt/profile/residual and
  bounded-noise diagnostics now validate ciphertext shape before secret-key
  shape, and exact/bounded rotation and bootstrap refresh-key generators
  validate public metadata, deterministic seeds, and public-key shape before
  deriving encrypted-zero refresh masks. Soracloud Multiply
  now uses a deterministic
  balanced ciphertext tree and rejects jobs whose declared multiplication depth
  underestimates that tree during job-spec validation and again before
  ciphertext evaluation through the same crypto planner, whose operation
  constructors and budget validation now also reject zero-input plans,
  single-input nonzero-depth plans, single-input Add plans, multi-input
  RotateLeft plans, and zero-round or non-single-input Bootstrap plans; the
  planner rejects zero-round Bootstrap metadata before input-shape errors and
  over-budget depth/refresh metadata before secondary operation-shape checks.
  Soracloud Bootstrap job-spec validation and runtime planner admission now
  also reject zero `bootstrap_count` metadata before non-single-input shape
  errors, and Add, Multiply, RotateLeft, and Bootstrap operation metadata is
	  rejected before secondary arity/input-shape errors across manifest
	  validation and runtime planner admission.
	  Soracloud FHE parameter sets and execution policies now also reject
	  advertised multiplication/bootstrap budgets above the exact evaluator budget
	  before governance admission. Soracloud decryption requests and ciphertext
	  query responses now also reject zero-prehash digest sentinels across
	  ciphertext commitments, optional consent evidence, governance linkage,
	  state-key digests, query hashes, and inclusion proof leaves/anchors before
	  private-state admission or query evidence can be trusted. Service
	  deployment, app-infra service/audit, service audit, runtime/Inrou runtime
		  state, service-state governance linkage, mailbox, and runtime-receipt
		  records now reject zero-prehash digest sentinels across manifest,
		  governance, mailbox, receipt, placement, and artifact hashes before those
			  records can be trusted. Container bundle hashes, service/agent
			  container-manifest references, service artifact references, and HF
			  placement identifiers/seeds now reject zero-prehash digest sentinels
			  before deployment, artifact, or placement evidence is admitted.
			  Training job records/audit events, HF source records,
			  shared-lease pool/member/audit records, and
			  model-host violation evidence now reject zero-prehash digest
			  sentinels across metric, source, normalized-runtime, pool,
			  placement, evidence, and slash hashes before training, lease, or
			  host-violation evidence is trusted. Agent apartment records and
			  audit events now reject zero-prehash manifest, mailbox payload,
			  autonomy request/result, runtime receipt, journal, and checkpoint
			  hashes, and apartment records canonical-check embedded manifest and
			  mailbox payload hashes against their preimages before accepting
			  authoritative agent state.
			  Agent apartment runtime records and audit events now reject
			  zero-prehash digest sentinels across manifest, mailbox payload,
			  autonomy request, execution result, runtime receipt, journal, and
			  checkpoint hashes before agent runtime or audit evidence is trusted.
			  Soracloud host request/response envelopes now validate nested
			  operation-specific payloads, operation/payload pairing, host paths,
			  found/payload consistency, delete mutation payload absence, and
			  payload/body hash preimages, while Core host syscalls run that
			  request validation before runtime fallback.
			  Bootstrap fixtures now pin both the zero refresh and each round-indexed
			  public refresh ciphertext. Full bootstrapping circuit/key-material
	  commitments now have a Rust admission/digest/proof-statement surface,
  artifact-aware execution path, verifier-backed Core fixtures, and a
  data-model material proof envelope, while audited prover/verifier artifact
  vectors remain an open release item.
- Keep Soracloud FHE governance parameter fixtures runtime-bound instead of
  descriptor-only. The canonical parameter-set, execution-policy, governance
  bundle, and job-spec fixtures now target the registered `bfv-default`
  RAM-LFE BFV profile; core admission consumes the shared bundle and rejects
  backend, polynomial-degree, slot-count, plaintext-width, ciphertext-chain, and
  parameter digest drift. Parameter-set descriptors now also carry the
  domain-separated registered BFV RNS modulus-chain digest, and core admission
  rejects RNS descriptor drift before FHE jobs can run. The registered RNS
  chain selector now preflights exact-addition and exact negacyclic-product
  coverage plus the concrete negacyclic NTT root table before returning the
  production chain or digest, and the RNS key-switch bridge applies the same
  exact-evaluator chain preflight before consuming key-switch material. Public
  RNS exact evaluator entry points now also preflight their required chain
  coverage before invalid refresh rounds, no-op packed rotations, or
  key-switch schedules can short-circuit validation, and indexed Bootstrap
  refresh helpers preflight requested round capacity before malformed
  ciphertext shapes enter the addition path.
  Bounded exact-RNS ciphertext multiplication now uses the same exact
  evaluator-chain preflight before operand or relinearization-key shape checks.
  Bounded target-limb basis-extension execution wrappers now share one
  rounded-capacity plus decomposition/evaluator prefix corridor, and Bootstrap
  refresh rejects structurally valid non-prefix decomposition chains before
  malformed refresh-key or ciphertext shapes.
  Refresh transcript digest assembly now returns structured shape errors for
  missing or unmatched rotation transcript seeds instead of relying on a
  post-validation panic invariant.
  Execution policies now also
  pin the domain-separated BFV evaluation-key bundle digest, and
  `RunSoracloudFheJob` rejects structurally valid but ungoverned key bundles
  before output state is emitted. The shared operation fixture also carries
  sample residue-arithmetic hashes, Galois key-switch component digests, scalar
  plus packed Galois switch execution vectors, a runtime packed `RotateLeft`
  half-rotation vector, a one-step packed `RotateLeft` mask-and-sum schedule
  vector, and bounded one-/two-round bootstrap refresh vectors so SDK release
  vectors can reject fixture drift before the full BFV-RNS evaluator lands.
- Keep BFV evaluation-key metadata bounded and canonical. The crypto validator
  now caps rotation-key and Galois key bundles, rejects duplicate Galois
  automorphism powers, rejects noncanonical, delimiter-shaped, or oversized
  bootstrap key ids, and rejects zero or oversized bootstrap refresh-round
  capacities before bundle digests or refresh operations admit them. Programmed RAM-LFE public
  parameters now reject Galois, rotation, and bootstrap refresh material
  entirely because first-release identifier programs consume only the
  relinearization key.
- Keep signed and proof-attestation identifier receipt compatibility
  fixture-backed instead of local-only. The current shared fixture pins
  canonical payload bytes, Iroha prehash, resolver signature, signed/proof
  attestation bytes, and adversarial receipt/policy mutations across
  the Rust data model, Torii runtime claim-receipt signing path, JavaScript,
  Python, Swift, Kotlin/JVM, and Java Android.
- Keep proof-carrying RAM-LFE policy metadata canonical and bounded. The crypto
  public-parameter parser now rejects noncanonical verifier backend/circuit ids,
  zero hidden-program digests, zero public-input schema hashes,
  empty/all-zero verifier keys, and oversized verifier keys before proof
  policies are admitted.
- Keep ZK-ACE authorization SDK surfaces aligned with executable chain
  support. Python now exposes identity commitment lifecycle instructions,
  authorized-transfer submission, and fail-closed capability metadata so BOI
  Privacy Lab and other catalog consumers do not advertise ZK-ACE execution
  when the native instruction surface is stale. JavaScript prepared-proof
  builders now also enforce ZK-ACE public-input version `1` and reject
  authorization proofs whose public inputs do not match the requested
  transparent transfer fields before an instruction is emitted. Core
  chain-admission tests now cover rotated and revoked identity commitments,
  unsupported action classes, transaction digest/account substitution, and
  mutated ZK-ACE/STARK public inputs. The shared data-model
  `OpenVerifyEnvelope` now exposes reusable admission validation with
  JS-aligned default bounds for proof bytes, public-input metadata, and
  auxiliary metadata, rejecting unsupported backends, blank circuits, zero
  verifier-key hashes, empty payloads, oversized payloads, and admission
  auxiliary bytes before backend-specific proof logic runs.
- Fold focused ZK/FHE adversarial tests into the long workspace validation
  corridor.

**Next checkpoints:** extend the fixture-bound RNS descriptor/residue corridor
into the broader BFV-RNS evaluator, add audited full-bootstrap prover/verifier
artifact vectors beyond encrypted-zero refresh, and fold the focused ZK/FHE
fixture corridor into broader release validation.

## Consensus, Performance, and Operations

**Status:** active optimization.

- Wire the canonical Sumeragi V1 pure engine through the live network,
  validation, payload, telemetry, and storage adapters while preserving
  deterministic consensus behavior and the hard consensus cadence gates.
- Keep permissioned and NPoS execution on one state machine; validator-set
  source and strict quorum math are the only mode differences.
- Keep top-level RBC evidence causality explicit in the formal model: valid
  digest evidence and nonzero chunk evidence must never appear without matching
  header evidence, nonzero READY evidence must never appear without matching
  header evidence and full chunk coverage, chunk counters must remain confined
  to chunking/chunk-complete/READY/delivered/corrupted states, partial chunk
  evidence must remain confined to chunking/corrupted states, full chunk
  coverage must remain confined to chunk-complete/READY/delivered/corrupted
  states, zero chunk counters must remain confined to idle/init/corrupted
  states, READY counters must remain confined to READY, delivered, or corrupted
  repair states, partial READY evidence must remain confined to
  READY-partial/corrupted states, READY quorum evidence must remain confined to
  READY-quorum/delivered/corrupted states, zero READY counters must remain
  confined to pre-READY/corrupted states, missing header evidence must remain
  confined to idle, present header evidence must remain out of idle, valid
  digest evidence must remain confined to active non-corrupted RBC states,
  corrupted repair states must retain header evidence, corrupted repair states
  must remain non-final with no commit certificate artifacts, corrupted repair
  states must enable explicit INIT repair while disabling CHUNK/READY/DELIVER
  and repeat fault progress, corrupted INIT repair must restore a clean INIT
  state with header/digest evidence and zero CHUNK/READY/finality artifacts, and
  every INIT step must hand off to CHUNK-only progress before READY or DELIVER
  can run, while every CHUNK step must keep CHUNK-only progress until full
  coverage and then enable READY with CHUNK disabled, and every READY step must
  keep READY-only progress below quorum while opening DELIVER once READY quorum
  is reached, while every DELIVER step must close RBC progress and split exactly
  on buffered commit evidence into committed finality or pending delivered
  evidence without commit-certificate artifacts, and delivered-but-uncommitted
  states must remain out of the live commit gate and committed phase while RBC
  progress stays closed; non-final honest or Byzantine commit-vote steps from
  delivered-pending states must preserve delivered evidence and keep the
  wait-state until commit evidence becomes complete, and finalizing commit-vote
  steps from delivered-pending states must install committed finality while
  preserving delivered RBC evidence; delivered-pending prepare-vote steps must
  preserve delivered evidence and either keep the prepare wait state or enter
  the commit-vote wait state at prepare quorum; delivered-pending timeouts must
  preserve delivered evidence while clearing live vote counters and starting a
  fresh NewView vote handoff; delivered-pending NewView votes must preserve
  delivered evidence and either keep the NewView wait state or enter proposal
  handoff with view evidence; delivered-pending proposal must preserve delivered
  evidence while starting a fresh prepare-vote handoff; delivered-pending GST
  observation must preserve delivered evidence while only setting the synchrony
  flag; every delivered-pending `Next` step must be covered by one of those
  handoff/finality/GST obligations, and every delivered-pending `[Next]_vars`
  spec step must either take such an action or stutter while preserving the
  wait state; every delivered-pending spec step must end either in committed
  finality with the certified stack or in the no-commit-certificate wait state;
  every delivered-pending spec step must preserve the exact delivered RBC
  evidence tuple and change commit-certificate artifacts exactly when committed
  finality is reached, and the GST flag may change only through the explicit
  `GstElapsed` action; delivered-pending view changes must come only from the
  timeout recovery branch, and delivered-pending view evidence can change only
  by timeout reset or quorum-forming NewView vote; delivered-pending live vote
  counters must follow the checked handoff/finality branch without stale
  prepare/commit/NewView counter carryover; delivered-pending post-state gates
  must match the resulting handoff phase while RBC progress and Byzantine fault
  gates remain closed, and timer gates must follow the resulting
  GST/finality/progress status; delivered-pending finality must come only from
  the exact commit-vote branch whose added vote completes `CanCommit`, and its
  current-view commit witness frame must be exact while non-final post-states
  keep commit witnesses absent; delivered-pending finality-stack predicates
  must match the committed/non-final outcome exactly; delivered-pending
  finality gates must close every consensus/RBC/fault/timeout path while
  leaving only pre-GST observation, and non-final gate surfaces must keep
  matching their handoff phase; delivered-pending finality quorum witnesses
  must bind committed post-states to exact live vote/stake evidence while
  non-final post-states stay below the live commit gate; non-final
  delivered-pending post-states must stay in exactly one live handoff phase
  with matching vote counters, view evidence, and enabled gates; the
  delivered-pending action surface must either stutter or take one covered
  consensus/timer handoff/finality action while RBC and Byzantine-fault actions
  remain closed; delivered-pending phase changes must come only from the exact
  proposal, prepare-quorum, timeout, NewView-quorum, or commit-vote finality
  source that installs the post-state phase; delivered-pending counter and
  view-evidence changes must have exact vote, timeout, proposal, or
  NewView-quorum sources; delivered-pending `Next` sources must be exactly one
  permitted consensus/timer action with pairwise-exclusive action predicates;
  delivered-pending stutters must preserve the full consensus/RBC/fault/timer
  gate surface and wait state; delivered-pending commit-certificate artifact
  changes must be equivalent to committed finality and come only from the
  exact commit-vote source; any delivered-pending commit-certificate artifact
  delta must install the certified committed-delivery bundle with delivered
  RBC evidence, finality-stack witnesses, exact commit evidence, and closed
  progress gates; non-final RBC DELIVER entries from `ReadyQuorum` must install
  the complete delivered-pending wait-state envelope immediately, preserving
  delivered evidence, keeping commit artifacts and finality certificates absent,
  and exposing only the consensus/GST timeout surface with RBC/fault gates
  closed; that certified bundle must be attributed to the exact honest
  or Byzantine commit-vote source with the source-specific `CanCommit` witness
  and vote/stake deltas; delivered-pending spec steps that do not change commit
  artifacts must remain non-final handoff states with delivered RBC evidence,
  absent finality certificates, stable commit artifacts, and the checked
  handoff/timer gate surface; stable-artifact delivered-pending spec steps must
  be sourced only by stuttering or an exact non-final proposal, prepare,
  commit-vote, timeout, NewView, or GST handoff, never by a finalizing
  commit-vote source; stable-artifact delivered-pending counter footprints must
  match the exact non-final source, advancing commit support only on non-final
  commit-vote handoffs, clearing stale commit support on handoff resets, and
  preserving counters on stutter/GST branches; stable-artifact delivered-pending
  phase/gate footprints must match the exact non-final source, so proposal,
  prepare, commit-vote, timeout, NewView, GST, and stuttering branches expose
  only their checked post-phase and enabled-action surfaces; stable-artifact
  delivered-pending timer footprints must match the post-state GST/progress
  status, keeping pre-GST GST/timeout open, closing timeout for post-GST live
  progress, leaving timeout open for post-GST stalls, and preserving timer gates
  across GST/stuttering branches; stable-artifact delivered-pending
  view/evidence footprints must expose timeout as the only view-advancing and
  view-evidence-clearing branch, quorum-forming NewView handoffs as complete
  view-evidence installers that preserve the view, and all other stable-artifact
  branches as view/evidence preserving; stable-artifact delivered-pending
  finality footprints must keep post-states outside `Committed`, without commit
  certificate artifacts, finality-certificate stacks, or live commit gates, while
  preserving finality/certificate/gate matching invariants; stable-artifact
  delivered-pending RBC surfaces must preserve delivered RBC evidence exactly
  and keep RBC INIT/CHUNK/READY/DELIVER plus Byzantine-fault gates closed;
  stable-artifact delivered-pending complete wait states must compose the source,
  counter, phase/gate, timer, view/evidence, finality, and RBC-surface
  obligations into one non-final envelope with commit artifacts absent and only
  consensus/GST/stutter actions exposed;
  nonzero
  CHUNK/READY counters
  must retain digest validity unless they are in the explicit corrupted repair
  state, while invalid digests must remain confined to idle or corrupted repair
  states; the fast/deep/TLC-fast configs must continue checking those obligations alongside the RBC progress-state
  evidence stack.
- Keep the Sumeragi formal coverage guard in CI so runner modes, CI commands,
  workflow entrypoints, Apalache version pins, README commands,
  conflict-marker-free formal wiring and TLA+/CFG artifact files,
  the TLC-routed top-level commit-path fairness check,
  PR baseline TLC cross-check coverage with documented Apalache-only
  widened-proof exceptions,
  well-formed runner case blocks,
  length-table-derived bidirectional documented TLC fast-mode coverage,
  duplicate-free and shadow-free
  runner case labels, duplicate-free Apalache command lists including
  scheduled/manual workflow commands, exact Apalache runner-mode CI reachability,
  unused runner-branch rejection, documented mutation-mode expected-failure
  coverage, TLC mutation-mode expected-failure runner routing,
  mutation-mode CFG name fragments, Apalache/TLC mutation CFG equivalence,
  expected-failure counterexample semantics,
  baseline expected-failure marker rejection, well-formed
  non-append/non-declaration/non-array/shell-builtin-mutation-free
  single-assignment runner proof inputs and scalar runner assignments, flat
  direct-child formal path and suffix
  containment, runner command shape, runner invocation proof-input binding, TLC
  constraint operator binding, zero-arity and
  nontrivial CFG/TLC runner constraints,
  non-type-only CFG checks, generic `NoBugInvariant`/`Safety`/`SafetyFast`-free
  fast CFG checks, nontrivial CFG-referenced semantic checks,
  top-level-only CFG behavior/check detection, indented CFG directive rejection,
  non-empty multi-line CFG check blocks,
  TLC module identifier and module-file reachability,
  Apalache/TLC TLA module identity, non-reserved static TLA dependency
  resolution, assumption/proof-free TLA modules, Apalache length declarations,
  well-formed purpose-bearing duplicate-free README length rows, and README length table
  agreement, single static top-of-file TLA module-header consistency, single
  terminating TLA `====` markers, duplicate-free TLA constant and static
  recursive/top-level operator declarations, matched recursive operator
  definitions, non-reserved static non-empty TLA declaration blocks, exact TLA
  variable/`vars` tuple consistency, CFG/module
  filename ownership, supported CFG directive validation, CFG behavior/check
  declarations, non-reserved static non-`vars` CFG operator-name syntax,
  zero-arity CFG proof targets, CFG-referenced top-level behavior/check
  operator definitions,
  named `INSTANCE` aliases excluded from proof targets, complete non-reserved
  CFG constant bindings, fail-closed one-binding-per-line CFG constant shape,
  non-empty CFG constant blocks,
  duplicate-free CFG
  `CHECK_DEADLOCK` directives, duplicate-free CFG constant/constraint/check
  targets, single-line and multi-line boolean-wrapper-free vacuity checks for
  semantic checks and constraints, complete recursive TLA+/CFG inventory
  reachability, and referenced TLA+/CFG files stay synchronized as new gates
  land.
- Use measured matrix runs, not speculative settings, before accepting higher
  throughput targets.
- Treat the explicit DA/RBC integration soft fallbacks as closed:
  the chunk-drop, chunk-drop-recovery, chunk-reorder, duplicate-init,
  selective-drop, chunk-equivocation, all-chunks-corrupted, conflicting-READY,
  and partial-erasure adversarial recovery paths now require
  commit-quorum-visible height progress before accepting recovery. The NPoS
  restart/large-payload tests require pre-restart RBC persistence or in-flight
  session evidence before restart, restarted-peer catch-up, recovered-session
  evidence, primary-cluster height, and quorum-visible commit height before
  accepting RBC persistence proofs; cold restart also requires the exact
  recovered session to persist a recovered-from-disk summary before accepting
  terminal delivery and resumed progress. The payload-loss DA-gate scenario
  now requires expected-height RBC session evidence on commit quorum, at least
  one nonterminal/incomplete session, and committed-quorum Sumeragi snapshots
  before accepting commit progress. The confidential downtime plus timeout
  restart-pressure localnet now also requires the restarted peer to catch up to
  the expected non-empty height instead of logging a best-effort waiver, and
  its shield fixtures carry deterministic non-empty encrypted payload envelopes
  so production payload validation cannot silently turn the shield debit into a
  rejected-in-block no-op.
  Required-observation large-payload DA/RBC
  tests, including the tight block queue case, fail closed if neither the session
  endpoint nor quorum-visible persisted RBC snapshots expose same-block-hash
  delivery evidence; conflicting persisted delivered hashes cannot be merged
  into one fallback quorum or selected from ambiguous quorum groups. Terminal
  RBC-state waits use the same quorum-visible persisted fallback instead of a
  single persisted peer when the live endpoint is unavailable. Delivered live
  and persisted fallback observations now reject invalid, zero-chunk, and
  impossible over-counted chunk records before accepting delivery evidence; the
  lower-level session-height and best-persisted-summary helpers also reject
  impossible chunk-count evidence while preserving valid in-flight zero-received
  sessions as nonterminal evidence, and payload-loss session-height waits now
  require the quorum on valid non-invalid session records instead of counting
  invalid-only sessions toward the quorum. The generic runner no longer
  synthesizes commit-timing-only observations. The
  runtime finalize path also retains commit-certified pending blocks instead of
  starting commit work while DA payload availability or strict manifest evidence
  is still missing. Operator-facing RBC status snapshot persistence now keeps
  memory snapshots active but raises the persistence-disabled gauge for setup
  failures as well as fatal write faults, so memory-only status mode is visible
  until explicit reconfiguration. Receiver-side RBC DELIVER acceptance is pinned
  by live-handler and helper regression coverage to require the receiver's
  protocol READY quorum even when local authoritative payload can supply missing
  bytes or adversarial tests enable sender-side debug emission shortcuts. The
  DA/RBC integration corridor no longer enables the test-only
  `force_deliver_quorum_one` shortcut, and the resulting Kura block-body
  rehydration coverage now also guards proposal-cache height cleanup so stale
  observation metadata cannot poison debug-build consensus workers. The
  real-quorum corridor has also been rerun across commit-certificate history,
  tight block-queue commit-QC recovery, synchronous background-worker fallback,
  and six-peer plain/RS16 large-payload coverage, all with zero P2P drops and
  empty peer stderr. Targeted adversarial live-network coverage now also
  revalidates conflicting READY evidence, chunk equivocation, full
  corrupted-shard abort, and partial erasure withholding against bounded
  progress/divergence expectations. The rest of the adversarial sweep has also
  been rerun with exact filters for chunk loss, chunk reorder, witness
  corruption, duplicate INIT pressure, selective validator chunk drop,
  locked-QC conflict gating, and drop-then-clean recovery. The pure engine now
  keeps commit-QC pending finality in `PendingFinality` across timeout/new-view
  noise and refuses competing proposal or prepare-QC ingress until the exact
  DA/RBC payload arrives; while pinned there, new-view highest-QC references
  cannot replace the pending commit QC with a conflicting same-height QC or a
  same-block non-commit QC, while later same-block commit QCs can refresh the
  pinned highest QC without emitting duplicate fetch/finality outputs.
  Finalized current heights are now inert in the pure engine: late proposals and
  pacemaker ticks for that height no longer emit prepare or new-view votes, and
  late payload availability for an already-finalized current height no longer
  populates the availability cache. Cached payload availability now also requires
  the exact `BlockSubject` parent/block/payload tuple before a commit QC can skip
  the DA/RBC fetch path, so mismatched-parent payload signals cannot satisfy
  later finality for the same block/payload hashes. Conflicting proposals now
  require a QC from a strictly newer height/view than the local prepare lock;
  same-round phase promotion alone cannot unlock a different block. The pure
  engine now also mirrors the live QC validation boundary by accepting only
  Prepare/Commit refs as proposal-carried or new-view-carried `highest_qc`
  evidence; NewView-phase refs are ignored for deterministic highest-QC
  selection and rejected at adapter input. NewView certificates carrying a
  `highest_qc` must now name the same subject block as that carried QC, matching
  the live QC validator and preventing mismatched NewView subject/highest-QC
  evidence from advancing the pure engine. NewView certificates without carried
  highest-QC evidence are now rejected by the pure engine instead of advancing
  view state. Prepare and Commit certificates now also reject unexpected
  carried highest-QC evidence before locks, highest-QC state, pending finality,
  or commit outputs can change. The live NewView QC state processor now reuses
  the canonical highest-QC validator before recording tracker support, so
  direct processor calls also reject mismatched subject and future-epoch
  highest-QC evidence. The standalone `ConsensusEngine` module now documents
  its reference-engine scope explicitly instead of carrying a live code
  unfinished-work marker: production network, validation-worker, RBC,
  telemetry, and storage adapters remain owned by the Actor/vNext path and must
  stay mirrored by the pure model when consensus behavior changes.
  Pure-engine committed-block adapter input now also rejects current-height
  wrong epoch/validator-set notifications and ignores future-height
  finality/reconfiguration notifications before the current DA/RBC finality
  boundary is resolved.
  Cached INIT RBC rosters are now pinned by negative coverage to refresh to an
  authoritative derived roster before stale READY or DELIVER evidence can be
  recorded. Durable RBC session recovery now selects the newest valid temp/main
  snapshot by persisted update timestamp, preserving crash-before-rename temp promotion
  without letting stale temp files shadow newer main snapshots; future-dated
  persisted snapshots are rejected before direct restart recovery can select
  them, and non-destructive metadata inspection used by probes reports only the
  newest valid temp/main metadata while preserving peer-owned files. Persisted
  timestamp conversion is checked before recovery or probe evidence is accepted,
  including adversarial max-timestamp snapshots. Operator-facing RBC status
  snapshot recovery now applies the same newest temp/main selection shape and
  rejects future-dated or unrepresentable persisted status timestamps before
  reporting disk snapshots; it also drops impossible chunk-counter rows, and
  both handle-side and in-memory RBC delivery/payload predicates now require
  positive complete non-invalid chunk sets. Durable RBC session recovery now
  rejects zero-chunk persisted records before they can re-enter startup as
  delivered/progressed sessions. Positive-chunk delivered snapshots without
  retained payload bytes now reload as repairable nonterminal sessions instead
  of re-entering the live state as `Delivered`, and lane/dataspace backlog
  accounting keeps their missing chunk pressure visible until complete verified
  payload bytes are present again. Positive-chunk incomplete records are still
  retained for repair continuity. Recovered RBC sessions now persist
  lane/dataspace allocation ownership, including TEU totals, and both direct
  session recovery plus the old status-summary adoption fallback reject
  inconsistent lane/dataspace sums or over-pending backlog rows before they can
  seed lane-local accounting. Direct disk validation also rejects incomplete
  digest vectors whose root contradicts the expected or computed chunk root
  before they can reload as repairable snapshots, and direct reconstruction
  rejects persisted sessions whose explicit expected and computed roots
  conflict. In-memory RBC status updates now reject the same impossible counters
  and inconsistent allocation rows as persisted status recovery, clearing stale
  same-key summaries instead of preserving old delivered-payload proof. RBC INIT
  rejection coverage now also pins digest-count, digest-root, header-hash, and
  invalid leader-signature/layout failures as no-cache paths, so malformed INITs
  cannot leave session-roster or vote-roster evidence behind. Local READY and
  DELIVER signing now shares the same key/header boundary: helpers require
  matching block-header hash/height/view metadata plus a leader signature that
  verifies against the session roster for that height/view before producing new
  local signatures. Local authoritative payload shortcuts now also hydrate-probe
  a cloned session before satisfying missing-chunk progress, so a matching local
  payload hash cannot accept READY/DELIVER progress when the advertised RBC
  chunk root, digest vector, or layout contradicts deterministic local chunking.
  Committed-block cleanup now keeps retained RBC summaries observable without
  synthesizing delivered status unless a matching local payload and positive
  chunk shape back the summary. Live RBC complete payload matches now also hash
  the reconstructed chunk bytes before satisfying DA availability or suppressing
  payload hydration, and summary-only RBC status rows no longer count as DA
  payload proof without byte-carrying live/recovered session evidence. Delivered
  payload-byte telemetry also refuses complete chunk sets whose reconstructed
  bytes do not match the advertised payload hash, so mismatched payload material
  cannot consume or report delivered-byte metrics. Production DA availability
  and authoritative-payload repair suppression now additionally require that
  the live/recovered RBC session's cached leader signature verifies against the
  resolved session roster, so forged signature metadata remains diagnostic-only
  even with complete chunk bytes; the older map-based complete-byte predicate is
  retained only for unit-level byte-shape coverage. Complete chunk sets without an
  advertised payload hash now follow the same nonterminal/unreported path,
  including restart recovery of `delivered=true` persisted sessions.
  RS16 layout payload-size metadata alone is no longer accepted as
  authoritative delivered-byte fallback evidence; incomplete delivered sessions
  must have local block payload bytes tied to the same height, view, and payload
  hash before they can report payload-byte telemetry.
  Retained summaries without live sessions now follow the same exact-evidence
  rule: no payload hash, wrong height, wrong view, or wrong payload hash means
  no delivered-byte metric and no once-only marker consumption.
  Retained summary delivered-state promotion now follows that same rule, so
  missing-hash or wrong-height/wrong-view summaries remain non-delivered during
  committed cleanup.
  Retained recovery snapshot refresh and RBC INIT rebuilds now also require the
  exact block key, block height/view, canonical payload bytes, and matching
  payload hash before rebuilding transport metadata; incomplete retained
  summaries with mismatched advertised hashes remain incomplete.
  Existing-session RBC INIT merge now also preserves already-bound sessions
  against conflicting duplicate INIT payload hashes, while missing-hash sessions
  with complete cached chunks must reconstruct to the INIT payload hash before
  that hash is installed.
  Frontier `BlockCreated` metadata construction applies the same canonical
  payload rule to the roster-hint fallback path, so non-canonical carried bytes
  cannot mint RBC transport metadata.
  RBC payload hydration now applies the same bytes/hash rule before filling a
  missing session payload hash, so carried mismatched evidence marks the session
  invalid without ingesting chunks or stamping false metadata.
  Generic `BlockCreated` metadata refresh now follows that same rule for
  complete cached RBC chunks: reconstructed chunk bytes must match the local
  block payload hash before a missing session hash is installed, while the
  pending block remains bound to canonical local `BlockCreated` bytes.
  Zero-chunk RBC sessions with only chunk-root metadata no
  longer count as authoritative payload progress without separate local payload
  bytes, and complete RBC chunk sets must reconstruct to the advertised payload
  hash before they unlock authoritative validation/recovery progress or advance
  the internal `AuthoritativePayload` stage.
  The DA/RBC availability reschedule gate now follows the same invalid-shape
  invariant: non-invalid zero-chunk or over-counted sessions with READY quorum
  remain unresolved before the availability timeout unless local block payload
  bytes are already available; the timeout boundary still releases the
  reschedule gate. The live reschedule gate also uses protocol READY quorum
  even when `force_deliver_quorum_one` lowers local emission helpers, so
  debug-only one-READY delivery cannot resolve DA availability before
  receiver-side quorum. The direct availability reschedule TLA gate includes
  the over-counted case and expected-failure mutation.
  RBC recovery-helper coverage now also pins non-invalid over-counted metadata
  as payload-repairable, matching zero-chunk metadata rather than treating the
  impossible count as complete recovery evidence.
  Receiver-side RBC DELIVER acceptance now rejects impossible live chunk shapes
  (`total_chunks == 0` or `received_chunks > total_chunks`) as invalid payload
  evidence after READY quorum, including under the DA missing-chunk policy, and
  the direct DELIVER acceptance TLA gate models that invalid-shape branch plus
  an over-counted expected-failure mutation.
	  Delivered-payload telemetry follows the same shape boundary: authoritative
	  local fallback bytes can account for incomplete valid raw deliveries, but
	  zero-total or over-counted delivered sessions cannot emit payload-byte
	  metrics or consume the once-only telemetry marker. The delivered-payload byte
	  TLA gate now also covers invalid-session, missing-hash, invalid-shape, and
	  payload-mismatch fallback rejection, and the actor-level local-payload
	  telemetry fallback now uses the same invalid-shape and cloned
	  hydration-probe chunk-metadata guard before status, cleanup, or DELIVER
	  emission can record bytes; matching local payload hashes alone no longer
	  satisfy delivered-byte telemetry when advertised roots, digest vectors, or
	  layouts contradict deterministic local chunking. Commit cleanup now keeps
	  retained summaries on that boundary as well: live-session summaries retain
	  delivered status and delivered-byte metrics only when complete chunks or the
	  strict local fallback prove the payload, while status-only retained
	  summaries now need exact local height/view/payload-hash evidence plus a
	  successful deterministic exact-frontier RBC snapshot refresh.
	  Live maintenance now carries that invalid-shape invariant through READY and
	  DELIVER emission, rebroadcast scheduling, and operator backlog accounting:
	  malformed zero-total or over-counted sessions first try local-payload
	  hydration; exact authoritative local payloads can rebuild zero-total
	  metadata into the deterministic positive chunk layout, while sessions that
	  remain malformed stay deferred/repair-visible instead of signing from
	  malformed counters or reporting zero missing pressure. Pending local READY
	  and ready-quorum local DELIVER wakeups now use the same roster-verified
	  leader-signature boundary as the local signing helpers, so invalid leader
	  metadata cannot hot-loop the actor before the builders refuse to sign. The RBC
	  backlog-status TLA gate now also models malformed summary/proposal/snapshot
	  pressure so saturating-to-zero accounting and authoritative-payload skips
	  stay pinned as expected failures. A dedicated RBC payload-hydration TLA
		  gate now pins the post-fetch transition as well: zero-total adoption,
		  over-count recounting, invalid/complete skip gates, and empty/hash/count
		  mismatch rejection each have explicit mutations. Direct helper
		  regressions and TLA mutations now also pin hostile zero-total digest/root
		  metadata so local repair cannot silently accept INIT-bound mismatch
		  evidence. The local DELIVER-emission formal gate now also models repaired
		  zero-total and over-counted sessions reaching DELIVER, with mutations for
		  stale deferral and missing broadcast side effects.
		  Core Sumeragi DA/RBC readiness no longer carries obsolete exact-frontier or
	  frontier-first ignored unit tests: stale direct-rotation and generic-handoff
	  fixtures were removed, active `force_view_change_if_idle` coverage now
	  asserts current non-leader timeout rotation semantics, and the only remaining
  ignored core Sumeragi tests are deliberate deep-topology model coverage.
  Sumeragi operator docs now match the manifest guard policy lanes: strict
  lanes keep DA-gated commit/proposal sealing blocked until the manifest guard
  clears, audit-only lanes only allow missing/read/spool-scan warnings, and
  manifest hash mismatches reject in every policy; `manifest_block_guard` and
  `manifest_gate` coverage pin those cases.
  Sumeragi evidence tooling now uses the live operator route consistently:
  MCP submit dispatch, MCP metadata, Torii handler comments, and evidence docs
  agree on `POST /v1/sumeragi/evidence`, while list/count remain GET routes.
  DA-gate finalize telemetry now avoids double-counting missing-local-payload
  deferrals: `finalize_pending_block` relies on the refresh path's gate record,
  and telemetry-enabled coverage pins exactly one `missing_local_data` block
  event per held finalization attempt.
  Pending-block validation priority now uses the same exact-payload
  complete-delivery invariant for both live RBC sessions and retained RBC
  status summaries, so malformed `delivered=true` evidence with missing chunks,
  missing payload hashes, mismatched payload hashes, live delivered chunks for
  locally mismatched pending bodies, or status-only summaries for locally
  mismatched pending bodies cannot schedule validation as `rbc_deliver` or make
  missing-QC cleanup treat repair payloads as available.
  Status-only READY-quorum counters now follow the same local
  height/view/payload-hash binding before they can schedule
  `rbc_ready_quorum` priority or preserve missing-QC repair availability, and
  live READY-quorum priority now also requires the live session payload hash to
  match the pending block's locally verified payload hash. That priority gate
  now uses the protocol READY quorum even when the test-only
  `force_deliver_quorum_one` shortcut lowers local RBC emission helpers, so
  debug settings cannot promote live-session or retained-summary priority below
  receiver-side protocol quorum.
	  DA availability proofing now also requires complete live RBC payload sessions
	  to carry matching block-header height/view/hash metadata and leader-signature
	  metadata before they can clear missing-local-data gates; summary-only status
	  and malformed live sessions remain diagnostic-only.
	  Local RBC READY and DELIVER construction now shares that metadata binding:
	  validators refuse to sign local READY/DELIVER messages for sessions whose
	  INIT/header metadata is absent or keyed to a different block height, hash, or
	  view, while READY relay uses the protocol relay threshold even when
	  `force_deliver_quorum_one` lowers local DELIVER emission helpers. READY
	  rebroadcast suppression, targeted missing-READY payload/body rescue, and
	  cached-slot timeout pressure also use protocol READY quorum, so the debug
	  one-READY DELIVER shortcut cannot suppress READY fanout, unlock targeted
	  payload rescue, or release reduced timeout pressure early. READY rebroadcast
	  bundles stay limited to already-recorded peer signatures. Cached RBC INIT
	  rebuilds and payload-bundle emission apply the
	  same key/header/signature binding before repackaging cached session metadata,
	  including same-height/view headers that are leader-signed for a different
	  block hash. Late missing-BlockCreated repair targeting now also trusts cached
	  leader-signature indices only after that verification, falling back to the
	  real slot leader when a cached index is forged. Inbound RBC chunk repair
	  responses now apply the same boundary before serving cached chunks, so
	  invalid, malformed, key/header-mismatched, or wrong-leader-signature sessions
	  cannot answer `RbcChunkRequest`. Outbound missing-chunk repair now also
	  requires cached header/signature metadata to match the session key before
	  emitting `RbcChunkRequest`, falling back to missing-BlockCreated repair for
	  misbound sessions. Production READY rebroadcast paths now use the same
	  verified session boundary before broad or targeted repair packages recorded
	  READY signatures for outbound traffic.
	  The actor idle scheduler now wakes immediately for complete READY-quorum RBC
	  sessions that still need local DELIVER emission, but keeps observers and
	  non-signing roles from hot-looping on states they cannot advance, and keeps
  complete-but-payload-mismatched chunk evidence passive instead of scheduling
  immediate local DELIVER attempts.
  Live partial DELIVER acceptance through authoritative local payload fallback
  is pinned as diagnostic-only delivery status until complete chunk evidence is
  present, so it can expose READY-quorum progress without masquerading as
  delivered-RBC validation priority.
  The Torii `/v1/sumeragi/rbc/delivered/{height}/{view}` operator endpoint now
  applies that same non-invalid positive-complete chunk invariant to its
  `delivered` flag while keeping incomplete matches visible as diagnostics.
  The Torii `/v1/sumeragi/rbc/sessions` snapshot now also exposes
  `complete_delivery` next to raw `delivered`, so operator tooling can
  distinguish raw DELIVER gossip from non-invalid positive-complete session
  evidence without losing diagnostic visibility.
  NPoS happy-path persisted-delivery fallback proof now also requires complete
  retained chunk metadata, so metadata-only delivered snapshots cannot mask a
  DA/RBC delivery regression.
  The NPoS RBC delivery wait gate now shares that complete-delivery predicate,
  so the pre-metrics readiness poll also ignores older-height, incomplete,
  zero-chunk, or invalid `delivered=true` snapshots.
  DA/RBC repair paths now distinguish raw DELIVER markers from complete local
  delivery as well: incomplete delivered sessions keep payload recovery,
  RBC-aware missing-block retry widening, and near-tip backpressure-exempt
  repair active until the advertised chunks are actually complete.
  Proposal cached-slot and stale-pending reschedule availability gates now share
  a complete-delivery-plus-READY-quorum check, so raw or partial DELIVER state
  cannot unlock reduced timeouts while DA/RBC availability is still incomplete.
  Operator RBC backlog snapshots now use the same complete local chunk-delivery
  guard, so delivered-but-incomplete sessions remain visible in generic missing
  chunk counters until local chunks are complete.
  RBC rebroadcast scheduling now also requires complete local delivery before
  using the DELIVER rebroadcast cadence, so incomplete delivered sessions keep
  missing-chunk repair deadlines active. Authoritative roster refreshes use the
  same complete-delivery guard before ignoring updates, so partial delivered
  sessions clear stale READY/DELIVER evidence when the roster changes.
  RBC chunk ingestion now uses verified complete-delivery state before
  suppressing commit-pipeline wakeups, so a partial delivered session that
  receives its final missing chunk still wakes validation/recovery once local
  chunks become complete and match the advertised payload hash. RBC READY
  ingestion uses the same boundary: accepted READY evidence still refreshes
  progress and wakes commit for delivered-but-incomplete sessions, while
  verified complete delivered sessions continue to suppress duplicate late READY
  churn. Local DELIVER emission and deferral bookkeeping use that same terminal
  boundary, so partial raw-delivered sessions keep repair/deferral state until
  the advertised payload is locally verified. Inbound duplicate/deferred
  DELIVER cleanup now follows the same boundary, recording valid bundled READY
  signatures and keeping READY/repair bookkeeping alive for raw-delivered
  incomplete sessions. Roster-promotion retry gates now also distinguish raw
  from complete delivery, so an init-roster
  source upgrade can retry local READY/DELIVER repair for partial raw-delivered
  sessions while verified delivered sessions still skip duplicate retries. The
  permissioned unverified-roster escape hatch now also requires an exact match
  to the canonical active topology before local READY/DELIVER signing or
  inbound READY/DELIVER acceptance can use an INIT-carried roster, and the
  cached roster source must be recorded as non-authoritative INIT evidence;
  source-less entries, tiny, foreign, same-quorum subset, duplicate, or
  otherwise non-canonical future-height rosters are stashed for recovery instead
  of reducing or reshaping the RBC certificate set. Already-derived rosters stay
  outside the unverified escape hatch and follow the authoritative-roster path.
  INIT-carried unverified rosters no longer
  populate the vote-roster cache; only authoritative derived rosters can seed
  cached vote validation.
  The periodic stalled-RBC repair loop now applies the same rule before
  re-attempting local READY, so raw delivered partial sessions do not stall
  simply because a DELIVER marker arrived before all chunks were verified.
  Count-complete chunk sets whose bytes fail payload-hash verification remain
  availability-incomplete even with READY quorum, including in the stale-pending
  DA/RBC reschedule gate that decides whether reduced timeout recovery may
  proceed before the configured availability timeout.
  Committed-block RBC cleanup now shares that exact complete-delivery boundary:
  raw delivered sessions with missing chunks or mismatched complete bytes remain
  retained after commit, while verified delivered payloads still drain runtime
  session state and retain the final status snapshot. Stale-view RBC pruning now
  uses the same boundary, so raw-delivered incomplete sessions keep runtime and
  repair state even when the payload is locally available. Committed-tip repair
  scheduling and committed-delivery suppression use the same verified boundary,
  so retained raw-delivered tip sessions remain repair-active until chunks
  verify. Session TTL pruning now also ages out retained status summaries and
  persisted snapshots without requiring a live RBC session to still be present,
  so committed-cleanup leftovers do not survive indefinitely on quiet nodes. RBC
  roster refresh now clears stale READY and DELIVER deferrals whenever changed
  roster evidence resets READY signatures, preventing retry bookkeeping from
  leaking across commit-topology changes. Local DELIVER emission now rejects
  complete chunk sets with mismatched chunk roots before arming missing-payload
  retry state, and terminal invalidation paths clear pending READY/DELIVER
  deferrals together with pending RBC messages. The
  four-peer NPoS/DA late-VRF persistence gate now passes on the current tree,
  advancing past the previously documented height-4 RBC stall and finalizing the
  epoch after recording the late reveal. DA-enabled NPoS recovery roster
  shrinkage now also has direct regression coverage proving the baseline
  validator set is restored and the pending restore marker is consumed at the
  committed recovery height.
  The RBC status lookup formal model now matches the current helper contract:
  `is_delivered` requires delivered, non-invalid, complete chunk metadata while
  intentionally not checking payload equality, and the expected-failure configs
  now mutate incomplete/invalid/over-counted acceptance instead of the obsolete
  "requires complete" case.
  Operator backlog aggregation now ignores invalid RBC sessions, so conflicting
  or mismatched evidence stays diagnosable through invalid/mismatch status
  without inflating generic, lane, or dataspace missing-chunk pressure.
  Exact-frontier slot tracking no longer carries a
  compatibility mirror layer: callers now observe canonical nested candidate,
  body, timer, and repair state directly. The live vNext
  proposal/availability adapter now treats reordered or duplicate DA signals
  monotonically, so availability-before-proposal remains ready for validation,
  late availability cannot regress prepared slots, and duplicate proposal or
  availability evidence cannot revive validation-aborted slots. The pure
  Sumeragi V1 engine's pending-finality path now also rejects conflicting
  same-block payload availability without caching it while a commit QC waits
  for a different exact payload hash. DA gate recovery telemetry now tracks
  exact manifest guard changes, so a recovered lane/sequence/kind is surfaced
  even when another DA gate remains active, while unchanged gates still do not
  synthesize progress. Certified-block roster sidecar synthesis now revalidates
  cached commit-QC history and precommit-signer fallback records through the
  block-sync QC validator before persisting sidecars, so under-quorum cached
  evidence or bad aggregate signatures cannot mint recovery roster metadata.
  Pre-INIT pending-RBC chunk cap eviction now releases dedup keys for evicted
  chunks while preserving the accepted survivor's dedup registration, and
  explicit pending-RBC stash clears release discarded CHUNK/READY/DELIVER dedup
  registrations as well. Runtime session clears, chunk-store eviction handling,
  stale-session pruning, committed-block cleanup, roster-change consensus
  resets, and mode-flip resets now route through the same dedup-aware cleanup,
  closing stale-cache pressure paths under adversarial chunk and quorum-message
  floods before INIT arrives.
- Keep hardware acceleration paths feature-gated with deterministic scalar
  fallbacks.

**Next checkpoints:** Sumeragi V1 adapter integration, certified-block
recovery soak coverage, longer-running live adversarial DA/RBC soak and mixed
restart/fault combinations built on the fail-closed session-summary and
status-counter assertions, peer-gap and DA/RBC tail-latency reductions under
the broadened rotating-fault evidence, broader formal coverage beyond the current
commit-path, frontier, TLC-cross-checked fork-safety, TLC-cross-checked
quorum-policy, TLC-cross-checked RBC deliver-quorum,
TLC-cross-checked direct RBC causality component gate, TLC-cross-checked direct RBC DELIVER acceptance gate,
TLC-cross-checked direct RBC commit-processing gate,
TLC-cross-checked direct RBC local READY emission gate (`rbc-ready-emission`),
TLC-cross-checked direct RBC local DELIVER emission gate (`rbc-deliver-emission`),
TLC-cross-checked direct RBC delivered-session rebroadcast gate (`rbc-delivered-rebroadcast`),
TLC-cross-checked direct RBC stalled-rebroadcast cursor gate (`rbc-rebroadcast-cursor`),
TLC-cross-checked direct RBC stalled-rebroadcast action gate (`rbc-rebroadcast-action`),
TLC-cross-checked direct RBC next-due scheduler gate (`rbc-next-due`),
TLC-cross-checked direct RBC chunk target helper gate, TLC-cross-checked direct RBC chunk payload-cap helper gate
(`rbc-chunk-payload-cap`), TLC-cross-checked direct RBC rebroadcaster selection helper gate,
TLC-cross-checked direct RBC weighted chunk allocation helper gate, TLC-cross-checked direct RBC payload chunking helper gate,
TLC-cross-checked direct RBC payload layout helper gate (`rbc-payload-layout`),
TLC-cross-checked direct RBC session chunk-ingest helper gate
(`rbc-session-chunk-ingest`), TLC-cross-checked direct RBC READY/DELIVER session
recording helper gate (`rbc-session-ready-deliver`), TLC-cross-checked direct RBC
delivered-payload byte telemetry helper gate (`rbc-delivered-payload-bytes`),
TLC-cross-checked direct RBC RS16 initial fanout helper
gate, TLC-cross-checked direct RBC chunk broadcast order helper gate,
TLC-cross-checked direct pending-RBC stash component gate, TLC-cross-checked direct pending-RBC status snapshot helper gate
(`pending-rbc-status`), TLC-cross-checked direct ingress dedup cache helper gate
(`ingress-dedup-cache`), TLC-cross-checked inbound consensus status counter helper gate
(`ingress-status-counters`), TLC-cross-checked consensus message
kind/outcome/reason label helper gate (`consensus-message-labels`),
TLC-cross-checked direct phase-latency status projection helper gate
(`phase-latency-status`), TLC-cross-checked direct telemetry availability/QC/RBC/pipeline status
projection helper gate (`telemetry-status`), TLC-cross-checked direct lane-detail status stripping and
projection helper gate (`lane-detail-status`), TLC-cross-checked direct DvP/PvP settlement telemetry
status helper gate (`settlement-status`), TLC-cross-checked direct Nexus fee/staking economics status
helper gate (`nexus-economics-status`), TLC-cross-checked direct NPoS repair fanout coverage status
helper gate (`npos-repair-coverage-status`), TLC-cross-checked direct mode/PRF/mode-flip status
projection helper gate (`mode-status`), TLC-cross-checked direct consensus
capability status projection helper gate (`consensus-caps-status`),
TLC-cross-checked direct effective timing status projection
helper gate (`effective-timing-status`), TLC-cross-checked direct transaction queue backpressure status
projection helper gate (`tx-queue-backpressure-status`), status history
projection helper gate (`history-status`), commit-quorum status projection
helper gate (`commit-quorum-status`), commit-inflight status projection helper gate
(`commit-inflight-status`), TLC-cross-checked direct RBC status lookup helper gate,
TLC-cross-checked direct RBC status retention/update-pruning helper gate (`rbc-status-retention`),
TLC-cross-checked direct RBC status persistence/fallback helper gate (`rbc-status-persistence`),
TLC-cross-checked direct RBC status handle lifecycle helper gate
(`rbc-status-handle`), TLC-cross-checked direct RBC backlog/status snapshot
helper gate (`rbc-backlog-status`), RBC abort status counter/latest-slot
component/anchor gate (`rbc-abort-status`), RBC mismatch status counter/label
component/anchor gate
(`rbc-mismatch-status`), direct RBC progress-stage synchronization helper
gate (`rbc-progress-stage`), direct RBC hot-repair/backpressure helper gate
(`rbc-hot-repair`), TLC-cross-checked direct RBC repair request helper gate (`rbc-repair-request`),
TLC-cross-checked direct RBC targeted READY/DELIVER repair helper gate (`rbc-targeted-repair`),
direct RBC outbound chunk flush helper gate (`rbc-outbound-flush`),
TLC-cross-checked direct RBC chunk post scheduling/debug-mask helper gate (`rbc-chunk-post-debug`),
direct RBC READY/DELIVER deferral throttle helper gate (`rbc-deferral-throttle`),
round-gap marker/snapshot/EMA status helper gate (`round-gap-status`),
TLC-cross-checked direct contiguous-frontier repair view-change suppression helper gate
(`frontier-repair-view-change`),
TLC-cross-checked direct contiguous-frontier recovery advance state-machine helper gate
(`frontier-recovery-advance`),
TLC-cross-checked direct same-height no-proposal storm recovery helper gate
(`same-height-no-proposal-storm`),
TLC-cross-checked direct VRF commit/reveal admission gate, TLC-cross-checked direct VRF
epoch-window arithmetic helper gate
(`vrf-epoch-window`), TLC-cross-checked direct VRF epoch-boundary finalization helper gate
(`vrf-epoch-boundary`), TLC-cross-checked direct VRF epoch restore/snapshot/observation-merge helper gate
(`vrf-epoch-restore`), TLC-cross-checked direct local VRF material derivation helper gate
(`vrf-material-derivation`), TLC-cross-checked direct local VRF emission state helper gate
(`vrf-local-state`), TLC-cross-checked VRF penalties report store helper gate
(`vrf-penalties-report`), TLC-cross-checked direct classic inbound vote-admission gate, vote
TLC-cross-checked duplicate-key helper gate (`vote-duplicate-key`),
TLC-cross-checked evidence freshness horizon helper gate,
TLC-cross-checked direct evidence canonicalization/deduplication helper
gate (`evidence-canonicalization`), TLC-cross-checked direct evidence validation helper gate
(`evidence-validation`), TLC-cross-checked direct double-vote detection/recording helper gate
(`double-vote-recording`), TLC-cross-checked direct invalid-QC shape helper gate
(`invalid-qc-shape`), TLC-cross-checked direct QC validation evidence helper gate
(`qc-validation-evidence`), TLC-cross-checked direct QC validation reason/evidence label helper gate
(`qc-validation-reason`), TLC-cross-checked direct block-sync QC retry/fallback helper gate
(`block-sync-qc-fallback`), TLC-cross-checked direct block-sync QC status helper gate
(`block-sync-qc-status`), TLC-cross-checked direct block-sync locked-QC helper gate
(`block-sync-locked-qc`), TLC-cross-checked direct known-block QC work enqueue gate
(`known-block-qc-enqueue`), TLC-cross-checked direct known-block QC work preparation gate
(`known-block-qc-work`), TLC-cross-checked direct known-block QC work queue drain gate
(`known-block-qc-drain`), TLC-cross-checked direct committed signed-quorum fetch fallback gate
(`signed-quorum-fetch-fallback`), TLC-cross-checked direct commit-QC-only fetch response
dispatch gate (`commit-qc-only-fetch-response`), TLC-cross-checked
direct BlockSyncUpdate gossip target-selection helper gate (`block-sync-update-targets`),
TLC-cross-checked direct cached BlockSyncUpdate proof/vote attachment helper gate
(`apply-cached-qcs`), TLC-cross-checked direct uncertified block-sync roster
admission gate (`block-sync-roster`), TLC-cross-checked direct block-sync roster
source/drop status helper gate (`block-sync-roster-status`), TLC-cross-checked
direct BlockSyncUpdate embedded-vote filtering and deferral handoff gate
(`block-sync-vote-deferral`), TLC-cross-checked direct already-known hintless
BlockSyncUpdate fast-path gate (`block-sync-known-hintless`),
TLC-cross-checked direct DA implicit BlockSyncUpdate recovery gate
(`block-sync-implicit-recovery`),
TLC-cross-checked direct frontier vote-placeholder gate (`block-sync-vote-placeholder`),
TLC-cross-checked direct known-block snapshot-hint gate (`block-sync-snapshot-hint`),
TLC-cross-checked direct known-block snapshot-roster gate (`block-sync-snapshot-roster`),
TLC-cross-checked direct no-verifiable-roster BlockSyncUpdate gate
(`block-sync-no-roster`),
TLC-cross-checked direct selected-roster known-block terminal replay gate
(`block-sync-known-roster`),
TLC-cross-checked direct selected-roster known-block BlockSyncUpdate gate
(`block-sync-known-selected-roster`),
TLC-cross-checked direct selected-roster BlockSyncUpdate signature gate
(`block-sync-selected-signatures`),
TLC-cross-checked direct selected-roster BlockSyncUpdate QC candidate/evidence gate
(`block-sync-selected-qc`),
TLC-cross-checked direct selected-roster BlockSyncUpdate quorum/missing-QC repair gate
(`block-sync-selected-quorum`),
TLC-cross-checked direct stale BlockCreated/recovery-mode helper gate
(`block-sync-recovery-mode`),
TLC-cross-checked direct selected-roster BlockSyncUpdate apply/recovery-mode gate
(`block-sync-selected-apply`),
TLC-cross-checked direct selected-roster BlockSyncUpdate post-apply QC prefilter gate
(`block-sync-selected-qc-prefilter`),
TLC-cross-checked selected-roster BlockSyncUpdate post-prefilter QC process gate
(`block-sync-selected-qc-process`),
TLC-cross-checked selected-roster BlockSyncUpdate unknown-block QC cache gate
(`block-sync-selected-qc-cache`),
TLC-cross-checked direct BlockSyncUpdate stale-view admission gate
(`block-sync-stale-view`),
TLC-cross-checked direct committed-height BlockSyncUpdate conflict/evidence gate
(`block-sync-commit-conflict`),
TLC-cross-checked direct block-sync warning throttle helper gate
(`block-sync-warning-throttle`),
TLC-cross-checked QC-insufficient warning throttle helper gate
(`qc-insufficient-warning`),
TLC-cross-checked direct canonical committed fetch/body response deferral gate
(`fetch-response-deferral`),
TLC-cross-checked direct exact body fetch handler gate
(`fetch-block-body-handle`),
TLC-cross-checked direct background consensus frame-cap preparation gate
(`background-frame-cap`),
TLC-cross-checked direct background request dispatch fallback gate
(`background-dispatch`),
TLC-cross-checked direct background scheduler bypass gate
(`background-bypass`),
TLC-cross-checked direct background fallback network dispatch gate
(`background-fallback`),
TLC-cross-checked direct fetch-pending response send gate
(`fetch-pending-response-send`),
TLC-cross-checked direct fetch-pending batch response fanout gate
(`fetch-pending-responses-batch`),
TLC-cross-checked direct pending fetch/body readiness flush gate
(`pending-response-flush`),
TLC-cross-checked direct deferred BlockSyncUpdate helper gate
(`deferred-block-sync-helper`),
TLC-cross-checked direct deferred BlockSyncUpdate cache/defer integration gate
(`deferred-block-sync-cache`),
TLC-cross-checked direct deferred BlockSyncUpdate replay gate
(`deferred-block-sync-replay`),
TLC-cross-checked direct future BlockSyncUpdate drop/window gate
(`block-sync-future-window`),
TLC-cross-checked direct RBC block-body repair admission gate
(`block-body-repair`),
TLC-cross-checked direct body requester stash-window gate
(`block-body-request-stash`),
TLC-cross-checked direct same-height block-body repair admission gate
(`same-height-block-body-repair`),
TLC-cross-checked direct block-body repair observed epoch source gate
(`block-body-repair-epoch`),
TLC-cross-checked direct commit-QC source selection gate
(`direct-commit-qc-for-block`),
TLC-cross-checked direct QC materialization/Kura recovery gate
(`materialize-qc`),
TLC-cross-checked direct BlockBodyResponse commit-QC extraction gate
(`block-body-direct-commit-qc`),
TLC-cross-checked direct detached BlockBodyResponse commit-QC handling gate
(`block-body-detached-commit-qc`),
TLC-cross-checked direct BlockBodyResponse fallback/companion dispatch gate
(`block-body-response-dispatch`),
TLC-cross-checked direct invalid-proposal evidence builder helper gate
(`invalid-proposal-evidence`),
TLC-cross-checked direct proposal mismatch helper gate (`proposal-mismatch`),
TLC-cross-checked direct proposal cache helper gate (`proposal-cache`),
TLC-cross-checked direct proposal-hint admission gate (`proposal-hint`),
TLC-cross-checked direct stale proposal-hint repair no-bug gate
(`stale-proposal-hint-repair`), TLC-cross-checked direct stale RBC hint repair no-bug gate
(`stale-rbc-hint-repair`),
TLC-cross-checked direct proposal metadata admission gate (`proposal-admission`),
TLC-cross-checked direct peer-admin detection helper gate
(`peer-admin-detection`), TLC-cross-checked QC signer-bitmap admission
(`qc-signers`), TLC-cross-checked direct raw QC signer-count helper gate
(`qc-signer-count`), TLC-cross-checked direct BlockCreated admission aggregate exactness gate
(`block-created-admission`), TLC-cross-checked direct missing-block request clear
helper gate (`missing-request-clear`), TLC-cross-checked direct missing-block clear
reason helper gate
(`missing-block-clear`), TLC-cross-checked direct proposal budget/cap helper gate
(`proposal-budget`), TLC-cross-checked direct non-RBC payload frame budget helper gate
(`non-rbc-payload-budget`), TLC-cross-checked direct proposal backpressure
classification helper gate (`proposal-backpressure`), TLC-cross-checked
proposal-defer warning throttle helper gate (`proposal-defer-warning`),
TLC-cross-checked direct proposal batch trim/canonicalization helper gate
(`proposal-batch`), TLC-cross-checked direct lane/dataspace commitment snapshot
builder gate (`commitment-snapshot-builder`),
TLC-cross-checked collector retry/gossip helper gate (`collector-plan`),
TLC-cross-checked direct lane interleave routing-decision helper gate
(`lane-interleave`), TLC-cross-checked direct collector fanout/selection helper gate
(`collector-selection`), TLC-cross-checked direct topology ordered-roster
mutation no-bug gate (`topology-mutation`), TLC-cross-checked direct PRF leader/shuffle
topology helper gate (`prf-leader-shuffle`), TLC-cross-checked topology
fanout/redundant-send helper gate, TLC-cross-checked direct active topology selection
helper gate
(`active-topology-selection`), TLC-cross-checked direct trusted-peer P2P topology
refresh helper gate
(`p2p-topology-trusted`), TLC-cross-checked P2P topology refresh coordinator
gate
(`p2p-topology-refresh`), TLC-cross-checked direct quorum retransmit target helper
gate,
TLC-cross-checked direct retransmit backpressure pacing helper gate, direct paced retransmit
target selection helper gate (`paced-retransmit-targets`) with TLC
cross-checks, TLC-cross-checked direct quorum reschedule backoff helper
gate, TLC-cross-checked direct DA/RBC availability reschedule gate
(`rbc-availability-reschedule`),
TLC-cross-checked direct vote-backed reassembly stall helper gate
(`vote-backed-reassembly-stall`),
TLC-cross-checked direct completed quorum view-advance helper gate
(`completed-quorum-view-advance`),
TLC-cross-checked direct quorum rebroadcast dispatch helper gate
(`quorum-rebroadcast-dispatch`),
TLC-cross-checked direct isolated vote-backed frontier handoff helper gate
(`isolated-vote-backed-handoff`),
TLC-cross-checked direct pre-timeout vote-backed frontier retransmit handoff gate
(`preemptive-vote-backed-retransmit`),
TLC-cross-checked direct near-quorum preemptive missing-payload escalation coordinator gate
(`near-quorum-preemptive-escalation`),
TLC-cross-checked direct manifest-gated quorum reschedule helper gate,
TLC-cross-checked signer-bitmap construction helper gate
(`build-signers-bitmap`), direct canonical/view signer-index
normalization helper gate (`signer-index-normalization`), TLC-cross-checked
commit-root consistency, TLC-cross-checked commit-pipeline recovery gate,
TLC-cross-checked direct known-block commit-QC recovery
helper gate, TLC-cross-checked stale-view commit-QC fetch admission helper gate
(`stale-view-commit-qc-fetch`), TLC-cross-checked direct commit-anchor QC promotion
helper gate (`commit-anchor-qc`),
TLC-cross-checked direct committed-height QC admission helper gate (`committed-height-qc`),
TLC-cross-checked direct empty-block QC drop helper gate (`empty-block-qc-drop`) with
component/anchor exactness,
TLC-cross-checked direct pending-progress accounting helper gate with aggregate exactness,
TLC-cross-checked direct pending-block lifecycle helper gate with no-bug exactness,
TLC-cross-checked direct pending-block marker/cooldown helper gate with no-bug exactness,
TLC-cross-checked direct pending-block Kura retry no-bug helper gate
(`kura-retry`) with aggregate exactness,
TLC-cross-checked direct commit-pipeline scheduling gate with aggregate exactness,
TLC-cross-checked precommit vote-count helper gate (`precommit-vote-count`),
TLC-cross-checked direct precommit vote lock filter gate
(`drop-precommit-vote-for-lock`),
TLC-cross-checked set-based voting signer-count helper gate
(`voting-signer-count`),
TLC-cross-checked direct cached vote-log epoch replay helper gate
(`distinct-vote-epochs`),
TLC-cross-checked direct NEW_VIEW highest-QC vote-selection helper gate
(`new-view-highest-qc-votes`),
TLC-cross-checked direct frontier NEW_VIEW catch-up helper gate
(`frontier-new-view-catch-up`),
TLC-cross-checked direct late NEW_VIEW near-quorum emission helper gate
(`late-new-view-emission`),
TLC-cross-checked direct near-quorum NEW_VIEW rebroadcast helper gate
(`near-quorum-new-view-rebroadcast`), TLC-cross-checked direct precommit-QC
locked-chain wrapper gate
(`precommit-qc-extends-locked`),
TLC-cross-checked direct requester roster-proof detection helper gate
(`requester-roster-proof`),
TLC-cross-checked direct online-validator and relay counter helper gate
(`online-validator-relay-counters`),
TLC-cross-checked direct commit-result drain component gate (`commit-result-drain`),
TLC-cross-checked direct commit-drain summary aggregation helper gate
(`commit-drain-summary`),
TLC-cross-checked direct commit-pipeline timing sample helper gate
(`commit-pipeline-sample`),
TLC-cross-checked direct commit-pipeline status recorder helper gate
(`commit-pipeline-status`),
TLC-cross-checked direct autoscale transition commit gate
(`autoscale-transition`),
TLC-cross-checked direct commit-QC signer quorum helper gate
(`commit-quorum-signers`),
TLC-cross-checked direct signature-index recovery helper gate
(`signature-index-recovery`),
TLC-cross-checked direct commit-QC cache/history lookup helper gate
(`commit-qc-lookup`),
TLC-cross-checked direct embedded-QC roster bootstrap helper gate
(`embedded-qc-roster`),
TLC-cross-checked direct cached-QC precommit signer record helper gate
(`precommit-signer-record`) with aggregate exactness,
TLC-cross-checked roster-validation memo cache helper gate
(`roster-validation-memo`) with aggregate exactness,
TLC-cross-checked roster-validation cached wrapper helper gate
(`roster-validation-cached`) with aggregate exactness, TLC-cross-checked core
roster-validation helper gate (`roster-validation-core`) with aggregate
exactness, TLC-cross-checked roster artifact selection helper gate
(`roster-artifact-selection`) with aggregate exactness,
TLC-cross-checked block roster cache
helper gate (`block-roster-caches`) with aggregate exactness,
TLC-cross-checked block-sync roster
evidence helper gate (`block-sync-roster-evidence`) with aggregate exactness,
TLC-cross-checked
block-sync history roster helper gate (`block-sync-history-roster`) with
aggregate exactness,
TLC-cross-checked persisted block-sync roster selection helper gate
(`persisted-roster-selection`) with aggregate exactness,
TLC-cross-checked BlockSyncUpdate roster
hydration helper gate (`block-sync-update-roster`) with aggregate exactness,
TLC-cross-checked roster
index projection helper gate (`roster-index-projection`) with aggregate
exactness,
TLC-cross-checked
membership-view hash helper gate (`membership-view-hash`) with direct no-bug
exactness,
TLC-cross-checked
membership mismatch status helper gate (`membership-mismatch-status`) with
aggregate exactness,
TLC-cross-checked membership advert publication helper gate
(`membership-advert`) with aggregate exactness,
TLC-cross-checked membership mismatch
ingress/fail-closed helper gate (`membership-mismatch-ingress`) with aggregate
exactness,
TLC-cross-checked consensus-params ingress helper gate
(`consensus-params-ingress`) with aggregate exactness,
TLC-cross-checked direct prevalidated commit artifact
trust helper gate (`prevalidated-commit-artifact`) with aggregate exactness,
TLC-cross-checked
commit-job dispatch gate with aggregate exactness,
direct commit-worker channel capacity helper gate (`commit-worker-config`), slow
commit-stage timing threshold helper gate (`commit-stage-timing-threshold`) with
aggregate exactness,
commit-inflight timeout gate with aggregate exactness, post-commit pacemaker
kick gate with aggregate exactness, idle-view proposal budget gate with
aggregate exactness,
TLC-cross-checked direct pacemaker core state-machine helper gate
(`pacemaker-core`), TLC-cross-checked direct pacemaker evaluation component gate,
TLC-cross-checked direct pacing governor helper gate,
cached proposal-slot timeout gate with aggregate exactness,
pending fast-path timeout helper gate (`pending-fast-path-timeout`) with
aggregate exactness,
stalled pending-block timeout decision gate (`stalled-pending-timeout`) with
aggregate exactness,
stalled pending-frontier timeout helper gate (`stalled-pending-frontier-timeout`)
with aggregate exactness,
missing-QC timing helper gate with aggregate exactness,
idle backlog signal helper gate (`idle-backlog-signals`) with aggregate
exactness,
proposal-liveness state helper gate (`proposal-liveness`) with aggregate
exactness,
exact-frontier slot tracker FSM gate (`frontier-slot-tracker`) with aggregate
exactness,
exact-frontier slot helper gate (`frontier-slot-helpers`) with aggregate
exactness,
exact-frontier proposal grace helper gate (`frontier-proposal-grace`) with
aggregate exactness,
slot tracker state helper gate (`slot-tracker-state`) with aggregate
exactness,
timeout/cooldown derivation helper gate (`timeout-derivation`) with aggregate
exactness,
round/view helper gate (`round-view-helpers`) with aggregate exactness,
PhaseTracker mutable state helper gate (`phase-tracker`),
TLC-cross-checked direct round-trace status recorder gate (`round-trace-status`),
direct failed-commit/block-sync helper gate (`failure-recovery-helpers`),
TLC-cross-checked direct transaction requeue branch helper gate
(`requeue-transactions`),
TLC-cross-checked direct tick/deadline scheduling helper gate, direct worker tick-gap helper
gate (`worker-tick-gap`),
TLC-cross-checked proposal parent resolution gate with aggregate exactness,
TLC-cross-checked highest-QC dependency deferral gate with aggregate exactness,
TLC-cross-checked precommit-QC view-change selector gate with aggregate exactness,
TLC-cross-checked commit-evidence replay gate with aggregate exactness, TLC-cross-checked block-sync recovery gate with aggregate exactness, TLC-cross-checked direct certified-block fetch gate,
TLC-cross-checked direct missing-block ingress fetch gate, TLC-cross-checked direct payload progress availability gate, TLC-cross-checked direct highest-QC fetch body-known gate, TLC-cross-checked direct local payload availability gate, TLC-cross-checked direct local block-known routing gate, TLC-cross-checked direct lock-safety block-known routing gate, TLC-cross-checked missing locked-QC payload recovery gate (`missing-locked-qc-recovery`), TLC-cross-checked direct local signed-block materialization gate, TLC-cross-checked direct authoritative payload progress gate, TLC-cross-checked direct hash-level authoritative block payload gate, TLC-cross-checked direct pending-block active-for-tip gate, TLC-cross-checked direct pending fast-unblock decision gate, TLC-cross-checked direct blocking pending-block counter gate, TLC-cross-checked direct quorum recovery vote-drain urgency gate, TLC-cross-checked direct frontier body-gap payload-drain urgency gate, TLC-cross-checked direct RBC authoritative payload progress gate, TLC-cross-checked direct slot authoritative payload no-bug gate, TLC-cross-checked missing-block fetch planner, TLC-cross-checked direct recovery status counter helper gate (`recovery-status-counters`), TLC-cross-checked direct QC rebuild status counter helper gate (`qc-rebuild-status`), TLC-cross-checked direct QC rebuild quorum reachability helper gate (`qc-rebuild-quorum`), TLC-cross-checked direct collector-targeting status counter helper gate (`collector-targeting-status`), TLC-cross-checked direct deferred recovery status counter helper gate (`deferred-recovery-status`), TLC-cross-checked direct missing-QC liveness status counter helper gate (`missing-qc-liveness-status`), TLC-cross-checked direct sidecar/no-proposal status counter helper gate (`sidecar-no-proposal-status`), TLC-cross-checked direct deterministic committee status helper gate (`deterministic-committee-status`), TLC-cross-checked direct timing/liveness status counter helper gate (`timing-status-counters`), TLC-cross-checked direct roster-recovery status counter helper gate (`roster-recovery-status`), TLC-cross-checked range-pull recovery helper gate (`range-pull-recovery`), TLC-cross-checked direct range-pull status counter helper gate (`range-pull-status`), TLC-cross-checked round-recovery bundle window helper gate (`round-recovery-bundle-window`),
TLC-cross-checked direct recovery-FSM reason classifier/rank/sort helper gate (`recovery-fsm-reason`),
TLC-cross-checked direct committed-edge conflict suppression gate,
TLC-cross-checked direct lock-rejected branch sink gate, TLC-cross-checked active-height lock-reject recovery gate,
TLC-cross-checked missing-block hard-cap recovery gate,
TLC-cross-checked missing-block hard-cap cleanup gate,
TLC-cross-checked missing-block view-change escalation gate, TLC-cross-checked precommit vote-emission gate,
TLC-cross-checked native AMX attestation gate,
TLC-cross-checked native AMX queue-journal replay gate, TLC-cross-checked native AMX routing-plan projection gate,
TLC-cross-checked native AMX receipt validation gate, TLC-cross-checked native AMX control-plane ingress with aggregate exactness,
TLC-cross-checked direct vNext chain-order component gate, TLC-cross-checked direct vNext stake-weight/quorum helper gate
(`vnext-stake-weight`), TLC-cross-checked direct vNext re-chain helper gate,
TLC-cross-checked direct vNext re-chain error label helper gate, TLC-cross-checked
direct vNext aggregate certificate verification gate, TLC-cross-checked direct vNext signing-preimage gate, TLC-cross-checked direct vNext control-certificate ingress component gate,
TLC-cross-checked direct vNext slot-lifecycle component gate, TLC-cross-checked direct vNext validation
ownership component gate, TLC-cross-checked direct vNext deadline/protection helper gate
(`vnext-deadline-protection`), direct vNext performance-fault config conversion gate
(`vnext-performance-config`), direct pending-block validation worker config helper gate
(`validation-worker-config`), TLC-cross-checked direct validation stall/redrive helper gate
(`validation-stall-redrive`), TLC-cross-checked direct validation redrive reason label/distinctness
helper gate (`validation-redrive-label`), validation ownership cleanup direct
exactness helper gate
(`validation-ownership-cleanup`), TLC-cross-checked direct vote/QC verification cache-key identity helper
gate (`verify-cache-key`), TLC-cross-checked direct async vote-verification ownership gate,
direct vote-signature verification worker config helper gate
(`vote-verify-worker-config`), TLC-cross-checked direct async QC aggregate-verification ownership gate,
TLC-cross-checked direct QC aggregate-verification worker config helper gate (`qc-verify-worker-config`),
TLC-cross-checked direct worker-loop drain scheduler component gate,
TLC-cross-checked actor-gate priority/fairness with aggregate exactness,
TLC-cross-checked direct worker-loop budget/adaptive-cap component gate,
TLC-cross-checked direct worker ingress routing component gate,
direct worker-loop stage helper gate, TLC-cross-checked direct worker-queue status accounting gate,
TLC-cross-checked NPoS VRF epoch-seal staging gate,
direct commit-anchor QC promotion helper gate (`commit-anchor-qc`),
direct committed-height QC admission helper gate (`committed-height-qc`),
TLC-cross-checked proposal assembly gate, TLC-cross-checked Kura durability
commit retry gate, TLC-cross-checked direct Kura persistence status counter/snapshot helper gate
(`kura-store-status`), Kura writer wake coalescing gate, Kura writer periodic
fsync fault regression gate, State DA cursor apply fault regression gate, Kura
pipeline sidecar queue cap gate, Kura durable budget metadata snapshot gate,
Kura pending-budget scan guardrail/benchmark gate, Kura eviction block-store lock split
gate, Kura background budget eviction gate, Kura background budget eviction retry-latency gate,
Kura long-history eviction benchmark gate,
IVM WSV admin syscall permission gate,
IVM WSV checkpoint durable-state dedupe/benchmark gate,
State view generation retry gate, WSV state write lock separation gate,
WSV state write lock telemetry alias gate, WSV heavy-world state-write-lock benchmark gate,
TLC-cross-checked post-commit cleanup gate, TLC-cross-checked frontier-gap
realignment gate, direct frontier block-sync hint/direct-response permit gate,
TLC-cross-checked direct same-height vote conflict helper gate, direct aggregate same-height vote-lock helper gate,
TLC-cross-checked direct proposal stale same-height vote helper gate,
TLC-cross-checked direct same-height vote recovery view-gap helper gate,
TLC-cross-checked direct tip-extension helper gate,
TLC-cross-checked direct DA gate helper gate,
TLC-cross-checked direct DA gate status transition semantics helper gate
(`da-gate-status`),
TLC-cross-checked direct DA manifest guard helper gate,
TLC-cross-checked direct consensus handshake capability construction helper gate,
TLC-cross-checked direct consensus handshake helper gate,
TLC-cross-checked direct runtime mode flip helper gate,
TLC-cross-checked direct effective consensus-mode selection helper gate,
TLC-cross-checked effective consensus timing aggregation helper gate,
TLC-cross-checked direct NEW_VIEW stats helper gate,
TLC-cross-checked direct NEW_VIEW tracker quorum/selection helper gate (`new-view-tracker`),
TLC-cross-checked direct timing monitor no-bug gate,
TLC-cross-checked hotspot summary accumulator helper gate (`hotspot-log-summary`),
TLC-cross-checked direct adaptive observability timing/fanout helper gate (`adaptive-observability`),
TLC-cross-checked direct pacing backpressure helper gate,
TLC-cross-checked counter-driven backpressure cooldown helper gate
(`counter-backpressure-cooldown`),
TLC-cross-checked direct per-reason pacemaker backpressure tracker gate
(`pacemaker-backpressure-tracker`),
TLC-cross-checked direct locked-QC helper gate,
TLC-cross-checked direct stake snapshot no-bug gate,
TLC-cross-checked direct NPoS validator election helper gate (`validator-election`),
TLC-cross-checked topology role/signature filter gate
(`topology-role-filter`),
TLC-cross-checked direct live local-vote roster helper gate (`live-vote-roster`),
TLC-cross-checked direct canonical round-roster helper gate (`canonical-round-roster`),
TLC-cross-checked direct block-specific vote-roster selection gate (`vote-roster-selection`),
TLC-cross-checked direct vote-roster cache/support helper gate (`vote-roster-cache`),
TLC-cross-checked direct commit-topology state/reset helper gate (`commit-topology-state`),
TLC-cross-checked direct roster index projection no-bug gate
(`roster-index-projection`),
TLC-cross-checked direct membership-view hash helper gate (`membership-view-hash`),
TLC-cross-checked membership mismatch status helper gate
(`membership-mismatch-status`),
TLC-cross-checked membership advert publication helper gate
(`membership-advert`),
TLC-cross-checked membership mismatch ingress/fail-closed helper gate
(`membership-mismatch-ingress`),
TLC-cross-checked consensus-params ingress helper gate
(`consensus-params-ingress`),
TLC-cross-checked direct prevalidated commit artifact trust helper gate
(`prevalidated-commit-artifact`),
TLC-cross-checked commit-job dispatch gate,
TLC-cross-checked direct precommit signer-history block-sync fallback gate,
TLC-cross-checked pure engine direct exactness constructor initial-state gate,
TLC-cross-checked pure engine direct read-only accessor gate,
TLC-cross-checked pure engine tick gate,
TLC-cross-checked pure engine tick unrelated-state preservation gate,
TLC-cross-checked pure engine direct NewView subject projection helper gate, pure engine certificate
prefilter dispatch gate, pure engine certificate prefilter state-handoff gate,
TLC-cross-checked pure engine certificate prefilter unrelated-state preservation gate,
TLC-cross-checked pure engine direct view-advance saturation component gate,
TLC-cross-checked engine NewView-QC gate,
TLC-cross-checked pure engine direct exactness NewView-QC highest-QC record gate,
TLC-cross-checked pure engine NewView-QC unrelated-state preservation gate,
TLC-cross-checked pure engine direct exactness NewView-QC advance gate,
TLC-cross-checked pure engine handle-dispatch gate,
TLC-cross-checked pure engine direct top-level argument-forwarding component gate,
TLC-cross-checked pure engine direct top-level output relay component gate,
TLC-cross-checked pure engine proposal-ingress gate,
TLC-cross-checked pure engine exact proposal output-field gate,
TLC-cross-checked pure engine direct exactness proposal state-mutation gate,
TLC-cross-checked pure engine proposal unrelated-state preservation direct component gate,
TLC-cross-checked pure engine direct exactness proposal validation-owner gate,
TLC-cross-checked direct exactness proposal-lock helper gate,
TLC-cross-checked direct QC-round compatibility helper gate,
TLC-cross-checked direct exactness QC reference projection helper gate,
TLC-cross-checked direct exactness QC reference comparator helper gate,
TLC-cross-checked direct exactness highest-QC record helper gate,
TLC-cross-checked commit-subject direct component gate,
TLC-cross-checked direct exactness payload lookup helper gate,
TLC-cross-checked direct validation-priority helper gate,
TLC-cross-checked direct vote-backed evidence no-bug gate,
TLC-cross-checked direct vote payload actionable no-bug gate,
TLC-cross-checked direct actionable vote-backed proposal evidence helper gate,
direct slot proposal evidence no-bug gate,
direct round liveness no-bug gate,
direct roster recovery FSM no-bug gate,
direct consensus recovery prune helper gate,
direct frontier live-owner work helper gate,
direct keep-frontier-pending-active helper gate,
direct stale-view pending prune no-bug gate,
direct superseded frontier payload retention helper gate
(`superseded-frontier-payload-retention`),
direct stale missing-block request prune no-bug gate,
direct stale missing commit-QC request prune no-bug gate,
direct stale RBC session prune no-bug gate,
direct highest-QC defer marker prune helper gate,
fast-finality inline validation helper gate,
direct observer signature-mismatch recovery helper gate (`observer-signature-recovery`),
direct validation failure finalization helper gate (`validation-failure-finalize`),
direct validation-reject reason label helper gate
(`validation-reject-reason-label`),
validation-reject status counter/snapshot component/anchor gate
(`validation-reject-status`),
peer-key policy status counter/snapshot helper gate
(`peer-key-policy-status`),
view-change cause status counter/snapshot component/anchor gate
(`view-change-cause-status`),
view-change proof/index status counter component/anchor gate
(`view-change-proof-status`),
leader/highest-QC/locked-QC status projection component/anchor gate
(`qc-status`),
TLC-cross-checked validation evidence QC selector helper gate (`validation-evidence-qc`),
TLC-cross-checked pure engine prepare-QC gate,
TLC-cross-checked pure engine direct exactness Prepare-QC lock/highest-QC record gate,
TLC-cross-checked pure engine direct exactness Prepare-QC phase-transition gate,
TLC-cross-checked pure engine Prepare-QC unrelated-state preservation gate,
TLC-cross-checked pure engine direct exactness prepare-vote cache/output gate,
TLC-cross-checked pure engine commit-QC gate,
TLC-cross-checked pure engine direct exactness Commit-QC highest-QC record gate,
TLC-cross-checked pure engine direct exactness Commit-QC phase-transition gate,
TLC-cross-checked pure engine Commit-QC unrelated-state preservation gate,
TLC-cross-checked pure engine direct exactness payload-available Commit-QC finality gate,
TLC-cross-checked pure engine direct exactness missing-payload Commit-QC pending/fetch gate,
TLC-cross-checked pure engine Commit-QC validation cleanup gate,
TLC-cross-checked pure engine committed-block gate,
TLC-cross-checked pure engine direct exactness committed-block record gate,
TLC-cross-checked pure engine reconfiguration staging gate,
TLC-cross-checked pure engine direct reconfiguration activation-height dedup component gate,
TLC-cross-checked pure engine committed-block cleanup direct component gate,
TLC-cross-checked pure engine committed-block unrelated-state preservation direct component gate,
TLC-cross-checked pure engine direct exactness payload-availability record gate,
TLC-cross-checked pure engine payload-availability gate,
TLC-cross-checked pure engine payload-availability unrelated-state preservation direct component gate,
TLC-cross-checked pure engine validation-result gate,
TLC-cross-checked pure engine validation-result unrelated-state preservation direct component gate,
TLC-cross-checked pure engine direct exactness validation-owner cleanup gate,
TLC-cross-checked pure engine direct exactness invalid-validation round/output advance gate,
TLC-cross-checked reconfiguration, TLC-cross-checked certified-recovery, TLC-cross-checked view-change, TLC-cross-checked validation-callback,
TLC-cross-checked certificate-admission, TLC-cross-checked highest-QC selection, TLC-cross-checked optional highest-QC selection-filter bounded models,
TLC-cross-checked certified-fetch with aggregate exactness, TLC-cross-checked pure-engine certificate
dispatch with aggregate exactness, TLC-cross-checked pure-engine certificate prefilter state with aggregate exactness,
TLC-cross-checked pure-engine certificate prefilter unrelated-state preservation with aggregate exactness,
TLC-cross-checked frontier-gap realignment with aggregate exactness, TLC-cross-checked Kura commit retry with aggregate exactness,
TLC-cross-checked missing-block fetch with aggregate exactness, TLC-cross-checked missing-block hard-cap cleanup with aggregate exactness,
TLC-cross-checked missing-block hard-cap with aggregate exactness, TLC-cross-checked missing-block view-change with aggregate exactness,
TLC-cross-checked native AMX attestation with aggregate exactness, TLC-cross-checked native AMX ingress with aggregate exactness,
TLC-cross-checked native AMX receipt validation with aggregate exactness,
TLC-cross-checked native AMX routing-plan with aggregate exactness, TLC-cross-checked NPoS VRF epoch seal with aggregate exactness,
TLC-cross-checked post-commit cleanup with aggregate exactness, and TLC-cross-checked restart replay with aggregate exactness,
and updated operator runbooks when defaults change.

## Community and Governance

**Status:** active growth work.

- Use the official X account, [`@hl_iroha`](https://x.com/hl_iroha/), as the
  primary public cadence for recurring X Spaces, demos, and roadmap Q&A.
- Publish recaps or recording links when available so contributors can follow
  progress asynchronously.
- Grow contributor and maintainer diversity by turning testnet interest,
  CBDC/regulated-finance adoption, and LFDT ecosystem connections into repeat
  reviewers and subsystem owners.

**Next checkpoints:** monthly X Spaces cadence, clearer contributor onboarding,
public follow-up notes for LFDT governance review items, and commit/reveal
hardening for SORA Parliament policy juries.
