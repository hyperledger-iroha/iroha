# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-ad2803dec3ac45510dfd4aa044130333b6c09a73bbd90682602fb435879835b9"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
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
  Native C# Ethereum/BSC outbound request helpers must also keep rejecting
  empty or all-zero `BundleBytes` before request hashing or app-owned prover
  callbacks, with the release inventory pinning both the Ethereum and BSC
  adversarial test markers.
  JS/browser, Swift, Kotlin/JVM, Java Android, and C# EVM-family outbound proof
  requests/results now also support paired non-zero `proofArtifactHash` and
  `provingKeyHash` metadata; when present, the request hash binds both values,
  wrapped results carry the same pair, and release inventories pin browser and
  native regressions for missing, zero, or mismatched artifact metadata. Strict
  release-bundle verifier inventory now also pins the outbound pre-callback
  bundle-level sparse guard directly. The release bundle/readiness path now
  also fails closed unless an audited
  `sccp-native-evm-groth16-prover-bundle-v1` manifest is attached, hash-bound,
  no-WASM, no-remote-prover, and tied to the active Ethereum destination
  binding, proof artifact, and proving key hashes. JS/browser, Swift,
  Kotlin/JVM, Java Android, and C# now expose that native prover bundle as a
  first-class SDK descriptor, validate the per-SDK native implementation rows
  and audit hashes locally, decode manifest and parity/self-test fixture bytes
  with strict UTF-8 before JSON parsing, parse the signed JSON manifest with the
  same camelCase/snake_case release-tooling aliases, and let Ethereum mainnet
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
  run, and C# now rejects leading-zero or overflowing decimal/hex strings for
  direct and Beacon REST u64 slot/counter fields instead of parsing them under
  an alternate canonical value. Accepted direct finality maps now strip known
  alias spellings from callback-facing evidence while preserving unknown
  extension fields for app
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
	  The diagnostic `sccp_allow_unready_transparent_proofs` bypass surface is
	  removed from `iroha_config`, Taira launch units, and route-config output,
	  and release inventory rejects case-variant, split-token, or source-escaped
	  attempts to reintroduce the old environment override. Production-ready
  BSC/TRON route-config renderers reject any `--allow-unready` option and
  non-production manifests before writing runtime overlays.
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
  source-event validation as the only collection mode: receipt-only output and
  its diagnostic CLI/API opt-ins have been removed, with release inventories
  pinning both attack shapes and the source-event mode/zero-digest guards; the
  Python receipt-proof CLI
  parsers now reject non-string transaction-hash, domain, and expected-chain-id
  values before invoking string methods; and the release inventory now pins
  noncanonical `eth_chainId` rejection, including leading-zero `0x01`, across
  SDK and Python receipt-proof collection tests. The JS/browser
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


<a id="record-f81b0e4889ac5b8c9519c234fe88ed0e3403b9e57fc7e0f034d124fbb4db913a"></a>

- Keep UI-side SCCP proof-generation SDK inputs fail-closed for ambiguous
  aliases; the current TON shard-state source-state path rejects duplicate
  camelCase/snake_case names inside nested validator-set transition proofs,
  including the transition-signature hash committed into the transition-chain
  witness.


<a id="record-4e9edc5616f05f988ba243ac95d4b3bd44a6db4010643b90c9cae9d7a112ba2b"></a>

- Keep public SCCP release evidence tied to every UI-side full-light-client
  role helper, not only aggregate request builders; Solana and TON readiness
  rows now require the per-role audit proof request symbols across web, Python,
  Swift, Kotlin/JVM, and Java Android.

