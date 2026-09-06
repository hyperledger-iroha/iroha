# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-dd53710bed24d643932eedb3f8c3dfc3bc5dd41016cdf4f7b33cf41b50976bc7"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP release readiness now treats Ethereum outbound provider validation as a
  production gate: public SDK and facade regressions must continue validating
  app-supplied Ethereum mainnet execution providers before outbound submitter
  callbacks can run. The readiness and bundle-verifier inventory tests now
  remove validate-before-submit markers directly across JavaScript source/dist,
  Python implementation/tests, Swift, Kotlin/JVM, Java Android, and C#.
  Strict release-bundle verifier inventory now follows the current multi-line
  JavaScript source/dist `validateExecutionProviderMainnet` call shape so
  formatting drift cannot block bundle readiness or hide removal of the guard.
  It also pins the bundle-level outbound-provider sparse-inventory regression
  directly, not just the missing source-inventory row.


<a id="record-6c04b1c940a5da84357ffcde6a7d3c4888a6bc94b428a20f6aed2f49a5515564"></a>

- SCCP release readiness now treats Ethereum local-admission coverage as a
  production gate: public SDK regressions must continue rejecting mutated proof
  bytes, all-zero proof/public-input/bundle/envelope bytes, empty envelopes,
  zero statement/source-material/source-adapter hashes, and stale proof-family
  metadata before local admission payloads are submitted.


<a id="record-19d3940e793a5b8639bcc48dd39b2a9f458ba022913c251e58f477a2b2090e27"></a>

- SCCP release readiness now treats Ethereum receipt-root zero rejection as a
  production gate: public SDK regressions must continue rejecting all-zero typed
  receipt roots before receipt-proof bytes can be built.


<a id="record-cb91960f74ddea99c4191c885e65c4de2e1fad638773bbe4525b6bb39d65ad31"></a>

- SCCP release readiness now treats Ethereum receipt RLP zero-topic handling as
  a production gate: public SDK and evidence-helper regressions must continue
  preserving zero log topics in generic receipt RLP before SCCP source-event
  ABI filtering runs.


<a id="record-1d2b6b80bef64ff1e8f715684ee281002fa9c243e5bd16cd9e79009d94b05541"></a>

- SCCP release readiness now treats Ethereum receipt RLP zero-address handling
  as a production gate: public SDK and evidence-helper regressions must
  continue preserving zero log addresses in generic receipt RLP before SCCP
  source-event ABI filtering runs.


<a id="record-1d12c7624b9b0693b884c014fe8eec1c3665811a389fe410e0075f86af98aa4f"></a>

- SCCP release readiness now treats Ethereum source-event context binding as a
  production gate: receipt-proof evidence regressions must continue binding
  source-event logs to receipt transaction hash, block hash, and block number
  before source-event evidence is accepted.


<a id="record-818f1b8c11b1d44330847b2a69f648a1a1a348000c5c7c9866ccbf9ab83cda15"></a>

- SCCP release readiness now treats Ethereum source-event evidence mode as a
  production gate: receipt-proof evidence regressions must continue requiring
  source-bridge validation and rejecting the removed receipt-only mode before
  receipt proof summaries can be emitted. The release-readiness and
  bundle-verifier inventory tests now remove the evidence script's
  `source_bridge_address` fail-closed marker and the removed-mode regression
  directly, so this gate cannot be satisfied only by unrelated Python test
  names.


<a id="record-3df2345872c7c694878e321a99d5d577ae21654404544ef670f8d8f2f0346226"></a>

- SCCP release readiness now treats Ethereum source-event zero-digest rejection
  as a production gate: receipt-proof evidence regressions must continue
  rejecting all-zero source-event digests before source-event evidence is
  accepted. The release-readiness and bundle-verifier inventory tests now remove
  the evidence script's zero-data `RuntimeError` marker directly, so all-zero
  source-event digest rejection cannot be satisfied only by the Python regression
  name.


<a id="record-38f4ef514c1423ef9f6b7922a133a7f88a2936fe8ec1c9b0f6a678805510a789"></a>

- SCCP release readiness now treats Ethereum receipt RPC duplicate-JSON
  rejection as a production gate: receipt-proof evidence regressions must
  continue rejecting duplicate JSON-RPC result or receipt keys and redacting
  receipt RPC transport/error details before receipt proof evidence is parsed.
  The inventory tests now remove the
  `object_pairs_hook=_json_object_without_duplicate_keys` parser hook directly,
  so duplicate-key fail-closed parsing cannot disappear while duplicate-key test
  names remain.


<a id="record-d1a5f2961ad6d12773625d0383b55e208250e9ebaf4d69de50094b10d835eee3"></a>

- SCCP release readiness now treats Ethereum block receipt transaction-hash
  uniqueness as a production gate: receipt-proof evidence regressions must
  continue rejecting duplicate transaction hashes in block receipt lists before
  receipt trie proofs can be built. The release-readiness and bundle-verifier
  inventory tests now remove the Python evidence-script uniqueness check and the
  JavaScript SDK `seenTransactionHashes` guard directly, so SDK-side uniqueness
  enforcement cannot disappear while the Python-only regression remains.
  Strict release-bundle verifier inventory now also pins the bundle-level sparse
  guard regressions for local admission, receipt-root zero, receipt RLP
  zero-topic/zero-address, source-event context/mode/zero-digest, duplicate
  JSON-RPC parsing, and block receipt transaction-hash uniqueness directly.


<a id="record-c2c4ee8064e0e8c4ffae0c6af75b2caf252398467ba1be6a71a190504954d044"></a>

- SCCP release readiness now treats Ethereum JavaScript receipt admission as a
  production gate: browser proof regressions must continue rejecting receipt
  metadata drift, missing beacon finality, typed receipts, and mutable prover
  callback evidence before local proving can run. The release-readiness
  inventory test now removes the beacon-finality, immutable-callback, and
  browser finality-regression markers directly, matching the bundle verifier's
  marker checks.


<a id="record-e2790c39e980ffe220c42d856fe4c8de6140ab76c10726ebf23ff8a8282615fd"></a>

- SCCP release readiness now treats Ethereum SDK receipt metadata binding as a
  production gate: public SDK regressions must continue rejecting
  block-receipt metadata drift and typed receipts before receipt proof builders
  can run. The release-readiness and bundle-verifier inventory tests now remove
  JavaScript receipt-RLP binding and Swift canonical receipt-RLP markers
  directly, so cross-SDK metadata validation cannot disappear while a Kotlin-only
  typed-receipt marker remains.
  Strict release-bundle verifier inventory now also pins the bundle-level sparse
  guard regressions for JavaScript receipt admission, SDK receipt metadata,
  native receipt finality, noncanonical chain ids, Beacon REST header shape,
  Beacon execution-payload binding, and sync-committee roster coverage.


<a id="record-2f1020943db1099e96a1f6e6c29bb326eac9f33cea647307b5b47fa7ad57dcdf"></a>

- SCCP release readiness reports and release bundles must keep native EVM
  prover artifact paths role-unique across the attached native prover manifest
  and published readiness summary, so proof artifacts, proving/verifier keys,
  parity fixtures, self-tests, and per-SDK implementation artifacts cannot
  silently reuse another role's file. The release bundle builder rejects that
  path reuse during input validation before copying native prover payloads,
  including not-ready diagnostic bundles.


<a id="record-82574a4d618f3ec7a65b870d8a09b2f2d9402affca88a4452d2f238a3dd65811"></a>

- SCCP native EVM prover cross-SDK parity fixtures and native self-test
  fixtures must keep semantic digest roles separated inside the fixture body:
  receipt/source proof, request/witness, calldata, Torii payload, and proof
  hashes cannot reuse another role even when manifests and SDK result rows are
  rehashed consistently.


<a id="record-40abed62182812d5ff98a06c6b9d9d8a8d56093d2cc121ab08202cac566fc6d0"></a>

- SCCP native EVM prover manifests must carry exactly one artifact row for each
  required public SDK; duplicate SDK rows and missing SDK rows remain readiness
  and published-bundle blockers even if the native manifest is rehashed.
  Manifest and public readiness-report SDK artifact rows must also stay in the
  verifier-owned sorted SDK order before Markdown rendering or bundle
  verification can pass. Manifest SDK artifact rows must also remain canonical
  objects with all required fields, no unknown fields, approved
  SDK/implementation bindings, and artifact hashes matching their committed
  roles. SDK implementation artifact paths and hashes are checked with the same
  local-artifact and hash-binding rules as the primary prover payloads.
  Duplicate JSON keys inside SDK artifact rows must fail before implementation
  hashes are trusted.
  Public native EVM artifact, audit-hash, and SDK-artifact helper controls
  such as `require_hash_match` and `require_complete` must be exact booleans
  before hash-match or completeness policy can be selected.


<a id="record-b579bf4ab0eb9c097ebedfef85275b99529d1ab7dd4e39439934e251da9457ee"></a>

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
  Copied public native EVM bundle roots, artifact maps, audit maps, SDK rows,
  implementation artifacts, and validation-blocker lists must also be exact
  builtin containers before readiness JSON redaction or schema validation can
  traverse them.
  Native prover bundle booleans must remain exact: `no_wasm` is accepted only
  as boolean `true`, and `remote_prover_required` is accepted only as boolean
  `false`, with string, numeric, null, and missing variants pinned as blockers
  in readiness and strict verifier tests.
  BSC native-prover bundle artifact admission must also reject `remote_prover`
  markers inside proof artifacts, so local native prover payloads cannot carry
  dormant remote-prover dependency evidence.
  Native EVM role helpers, Markdown cells, destination status comparisons, and
  SDK-result maps must also keep exact-key hostile alias regressions pinned in
  the native-prover bundle schema inventory on both readiness and strict
  verifier paths.
  Native fixture exact-key helpers, summary audit-role alias checks, and
  hostile-key status redaction must stay pinned in the same schema inventory so
  malformed copied fixture/result keys cannot satisfy required native-prover
  fields or leak through public blockers.
  Generator-side native EVM payload-source, validation-blocker, summary-schema,
  status-payload, fixture-status, and artifact-summary helper boundaries must
  also stay pinned so copied container subclasses cannot reach native-prover
  bundle rendering or payload copying.
  The bundle builder must reject copied non-empty native summary extra fields,
  noncanonical validation blocker lists, passed summaries with blockers,
  malformed artifact rows, noncanonical proof/key/destination hash text,
	  artifact/hash drift, missing or duplicated audit roles, audit hash reuse,
	  SDK id/implementation drift, missing required SDK rows, SDK implementation
	  artifact/hash drift, and duplicate artifact path roles before
	  `--allow-not-ready` diagnostics can render or write public artifacts.
	  Copied native prover summaries must also reject scalar identifier drift in
	  `schema`, `bundle_id`, `lanes`, and `proof_backend` before diagnostic
	  Markdown can be rendered, and Required Release Evidence must explicitly
	  name the native-prover bundle schema source-inventory row and
	  copied-summary scalar exactness before public bundle readiness can pass.
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


<a id="record-ce288b1eb81f943e2eb58c3b331729b68d98ea7a0b9b95e395a7e4ce568651ff"></a>

- SCCP native EVM prover parity and self-test fixtures must also carry exactly
  one result row for each required public SDK; rehashed fixture artifacts that
  omit SDK rows, add unknown SDK rows, or replace the SDK result map with a
  malformed/empty container remain readiness and published-bundle blockers.
  Individual SDK result rows must also remain canonical objects with all
  required fields, no unknown fields, and values matching the shared fixture
  hashes and public-signal vector. Duplicate JSON keys inside fixture roots,
  `sdk_results` maps, or SDK result rows must fail before row values are
  trusted, so row-level shape drift is rejected before release evidence can
  pass.
  Native EVM cross-SDK parity fixtures use only the production
  `sccp-ethereum-mainnet-native-evm-cross-sdk-parity-v1` schema; JavaScript
  package exports and TypeScript declarations must not reintroduce legacy
  `*_PARITY_FIXTURE_SCHEMA_V1` schema constants or union acceptance.
  Fixture root objects and SDK result rows also reject malformed unknown field
  names with structured blockers, so control characters, whitespace,
  Markdown-unsafe characters, and non-ASCII/confusable keys are never echoed in
  public readiness or bundle-verifier diagnostics.


<a id="record-5b092c949bbf82d54e05f5c5f81940db399af835bf3d35cd62bf003cd52d39ce"></a>

- SCCP native EVM prover parity and self-test fixture public-signal vectors
  must keep the canonical nine 32-byte word shape; rehashed fixture artifacts
  with shortened vectors or malformed signal words remain readiness and
  published-bundle blockers even when SDK rows are kept in sync.


<a id="record-d7594c3ca03b2b7e9c927f10d82f8963e061b13b57a236ebb10f19339b26b9a7"></a>

- SCCP native EVM prover manifest paths must remain local artifact names:
  public SDK manifest parsers, readiness rendering, bundle generation, and
  bundle verification reject URI/drive-prefix style paths and
  WASM/remote-prover markers in filenames before any no-WASM/no-remote
  evidence can be accepted. Keep SDK implementations free of raw forbidden
  dependency tokens so the source inventory remains fail-closed.


<a id="record-d2cc5b596dc2394bc981f5a020f72930b79a069ad3505cddf2c65d34d0524dd7"></a>

- SCCP release readiness reports now promote the native no-WASM/no-remote
  source inventory to a production gate: public SDK parsers, artifact
  verifiers, self-tests, browser distribution guards, and adversarial native
  prover manifest coverage must remain pinned in the JSON report and published
  bundle evidence. JavaScript native artifact verifier diagnostics must keep
  the field-qualified `nativeProverArtifacts.sdk` rejection for missing or
  padded SDK ids, BSC mainnet/testnet forged descriptor regressions must prove
  plain spread descriptors cannot reach self-test or prover callbacks, and
  bundle-verifier sparse fixtures must prove those markers remain enforced.
  Readiness-report and strict bundle helper tests now remove every uniquely
  detectable native no-WASM/no-remote inventory marker across all rows, so this
  gate cannot degrade to sampled marker coverage. Strict release-bundle verifier
  inventory now also pins the native canonical-SDK and no-WASM sparse guard
  tests directly.
  The JavaScript SCCP package public surface must stay production-only for
  source-chain proof helpers: `buildBscSourceChainProofEnvelope` remains the
  exported BSC constructor and must require explicit source validator private
  keys plus real `blockReceipts`, while placeholder/testnet source-chain proof
  builders, deterministic placeholder validator-secret derivation, synthetic
  receipt-root fallback, and `Placeholder` declaration names stay out of shipped
  runtime code, root exports, and subpath exports.
  BSC Groth16 material `productionBlockers` must stay public-safe across
  generated material manifests, proof-self-test, preflight report validation,
  attestation handoff/request summaries, and finalization/materialization
  errors: HTML entities and bounded URL-percent encodings are decoded before
  sensitive-name matching, and encoded or repeated-space secret/private-key
  blocker text must fail closed with fixed diagnostics. Duplicate blocker
  rejection must compare the same decoded, ASCII-space-collapsed, lowercased
  public text before direct proof-self-test or copied preflight report
  diagnostics can echo repeated operator blockers. Direct proof-self-test
  manifest blockers must be non-empty canonical printable ASCII strings without
  control characters after bounded decoding before preflight diagnostics can
  quote any blocker text. Generated SnarkJS self-check blockers must be
  canonicalized to single-line printable public text before manifest writing,
  while copied blocker arrays keep the stricter fail-closed decoded boundary.
  BSC Groth16 attestation request role blockers must enforce the same
  printable/no-control/canonical/duplicate boundary in `attestation-status` and
  signing/finalization validation before role diagnostics can echo blocker
  text; BSC and TRON route-config production blocker lists must enforce the same
  decoded ASCII-space-collapsed sensitive-name and duplicate-key boundary plus
  the same printable/no-control boundary after bounded decoding too, so encoded
  newline/tab/DEL or non-ASCII/RTL text fails before route-config diagnostics or
  generated TOML can preserve post-deploy blocker text. BSC Groth16 material
  helper operator booleans must also stay exact for remaining overwrite
  switches: missing options may take their documented fallback, but explicitly
  supplied empty, null, object-wrapper, padded, uppercase, alias, boolean, or
  numeric values must fail closed before artifact writes can proceed. Removed
  local setup switches must fail before PTAU reads, and `generate` must require
  externally supplied Powers of Tau material. Removed unready-candidate
  proof-self-test flags must fail before manifest reads, and proof-self-test
  reports must require productionReady material. Template writer regressions
  must prove pre-existing transcript index, evidence index, and handoff outputs
  stay untouched when `overwrite` is malformed. The Groth16 material evidence
  guard inventory must pin those parser and adversarial-test markers before
  release evidence can pass.
  Native EVM release bundles must also keep role-specific artifact byte floors:
  64 KiB for proof/proving material, 128 bytes for verifier/support fixtures,
  and 1024 bytes for SDK implementation artifacts. Public Swift, Kotlin,
  Java/Android, and .NET native EVM artifact verifiers must enforce those same
  floors with hash-consistent below-floor negative tests so mobile or .NET
  callers cannot approve bundles the release gate rejects.


<a id="record-60f27d3b5344aea115d2ae4cd6332d5e3b21afebde8e013621841be0d41e70a2"></a>

- SCCP release readiness reports now also promote the native EVM Groth16 prover
  bundle schema to a production gate: manifest schema checks, readiness summary
  schema checks, artifact hash/path binding, and bundled-manifest drift
  rejection must remain pinned before public bundle readiness can pass. Native
  prover bundle schema sparse inventory checks must remove every uniquely
  detectable marker across the verifier, readiness gate, release builder,
  adversarial bundle tests, SDK artifact/order tests, duplicate-key redaction,
  path-redaction tests, and self-inventory rows.
  no-WASM/no-remote manifest flags must be exact booleans, not truthy or falsy
  scalar substitutes. The
  release bundle builder must also compare copied public artifact rows against
  the copied file byte lengths and SHA-256 hashes before Markdown rendering or
  public JSON writes, so forged input, corridor, or native artifact hashes cannot
  publish before final bundle verification.


<a id="record-e18b806c1e70108701fa9ccef3869fe0dc439c78c7e60c2e0ce4ed5bd1399b38"></a>

- SCCP release readiness reports now also promote the active Ethereum EVM
  source-adapter deployment source inventory to a production gate, pinning the
  deployment-unblocks-production helper, source-bridge network/config binding,
  and negative drift tests before active Ethereum launch evidence can pass.
  Deployment-bound EVM source readiness must also reject replayed source trust
  anchor, message-inclusion verifier, finality-policy, and source bridge runtime
  code hashes before source-adapter readiness or deployment-bound proof matching
  can pass.


<a id="record-18dfb22b641fcdea53334f69964dffab4b457427892e637424736bfa59dec0b5"></a>

- SCCP release readiness reports now also promote the Ethereum no-proxy
  data-collection source inventory to a production gate, so app-owned execution
  and Beacon provider reads, provider markers, and no Torii proxy/embedded
  HTTP-client fallbacks stay pinned across public SDKs before active Ethereum
  launch evidence can pass. The readiness and bundle-verifier inventory tests
  now exercise every configured SDK region directly, including JavaScript
  source/dist, Python, Swift, Kotlin/JVM, Java Android, and C#. Strict
  release-bundle verifier inventory now also pins the bundle-level no-proxy
  sparse guard directly.


<a id="record-ccf885e2d54291854bf4769cef1ea134a388738c760d3f44c9d0d6ce7271206e"></a>

- SCCP release readiness reports now also promote the Ethereum native
  receipt-finality source inventory to a production gate, so Swift, Kotlin/JVM,
  Java Android, and .NET receipt-proof builders must keep finalized-header root,
  sync-committee root, and Beacon-slot prerequisites pinned before active
  Ethereum launch evidence can pass. The inventory tests now remove Swift
  `strictFirstPresent` finalized-root and C# normalized finalized-root markers
  directly in addition to Kotlin finality markers.


<a id="record-708f400fcd06bfd96aed4ac881b669ee13bc4e5111a693998c51ab7a6e1f0c48"></a>

- SCCP release readiness reports now also promote the Ethereum Beacon REST
  finalized-header shape source inventory to a production gate, so public SDK
  validators and negative tests for non-zero parent/state/body roots plus
  96-byte finalized-header signatures must stay pinned before active Ethereum
  launch evidence can pass. Beacon REST response parsers must also reject
  duplicate JSON keys at the root, nested-object, and array-contained-object
  boundaries before finality evidence fields are trusted.


<a id="record-e0ed842240c7f2876eceb547185d2c504ea5c69b36ba865c7e8c8e72bb9eb066"></a>

- SCCP release readiness reports now also promote the Ethereum Beacon REST
  execution-payload binding source inventory to a production gate, so Beacon
  target-header/root/block reads, light-client finality-update evidence,
  execution block-hash/receipts-root binding, and C# SSZ root parity vectors
  must stay pinned before active Ethereum launch evidence can pass.


<a id="record-8c8f3d8e49899248f6dcfe5603df29a7df7a34a3ddb7d547a2232f61cc269ae7"></a>

- SCCP release readiness reports now also promote the Ethereum sync-committee
  roster source inventory to a production gate, so exact 512-authority mainnet
  rosters, unit validator weights, 342-participant quorum fixtures, and
  81,925-byte next-sync-committee payload vectors must stay pinned across public
  SDKs before active Ethereum launch evidence can pass.


<a id="record-9fb0a38700f2712a3c58e3972bd094026a50f8541ee7ef64b6f057fb9cbc35e1"></a>

- SCCP release readiness reports now also promote the Ethereum source-bridge
  config source inventory to a production gate, so bridge-address, network-id,
  code-hash config hashing, and negative config-drift tests must stay pinned
  before active Ethereum launch evidence can pass. Readiness and strict-bundle
  sparse tests must remove every source-bridge config marker across Python,
  all-lanes import, JavaScript source/dist, Swift, Kotlin/JVM, Java Android,
  C#, readiness, and bundle rows, and the strict-bundle verifier inventory must
  pin its own sparse guard so cross-SDK config-hash coverage cannot be dropped
  while the inventory row remains present.


<a id="record-777b260d8c6ef6084c1b814314dc97458a6465eae2f5abd3e2fe3bb098403c09"></a>

- SCCP release readiness reports now also promote the EVM contract-smoke
  Ethereum-mainnet network-id and production-surface inventories to production
  gates, so ETH/BSC chain-id rejection vectors, accepted-event network ids,
  verifier code/key binding, destination-binding, domain-overflow, proof-shape,
  cross-deployment, and replay-rejection smoke coverage must stay pinned before
  active Ethereum launch evidence can pass. Readiness and strict-bundle sparse
  tests must remove every uniquely detectable network-id and production-surface
  marker across the EVM smoke tests, bridge replay guard, readiness wiring,
  readiness tests, and bundle tests, with the strict-bundle sparse guards pinned
  in their own inventories.


<a id="record-ac0cd1d947d3442c0760c9bde7e2c7aedec04cc4ab53c1d98a41cf5d1dd1afa8"></a>

- SCCP release readiness reports now also promote the Ethereum Core
  range/finality binding source inventory to a production gate, so message proof
  ranges must stay bound to artifact finality height and negative outer-range
  replay tests before active Ethereum launch evidence can pass. Readiness and
  strict-bundle sparse tests must pin every Core implementation marker and
  negative outer-range replay marker directly, and the strict-bundle verifier
  inventory must pin its own sparse guard so marker-level coverage cannot be
  dropped while the inventory row remains present.

