# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-5ed6bfd0080eccc3662bbb8bdeb08b2df546b751972b3c802bdc53fde2550dac"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
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
  source bridge bytecode sampling to the `finalized` block tag, and the release
  source-live inventory now pins explicit ETH and BSC lane sentinels for
  source-bridge module dispatch plus finalized/latest block-tag defaults so one
  EVM launch lane cannot lose live source evidence coverage unnoticed. The
  release destination-live inventory applies the same ETH/BSC lane coverage rule
  to destination RPC chain-id sentinels and finalized/latest block-tag defaults.
  The Ethereum block-tag metadata inventory now also has explicit ETH/BSC lane
  sentinels for source and destination finalized/latest default tests, so the
  release gate cannot pass with only one EVM lane still guarded.
  Ethereum source and destination production TOML now carries explicit block-tag metadata that
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
  now also reject Node/TAP diagnostic output in copied JS phase transcripts,
  including `not ok`, `ERR_*`, JavaScript exception names,
  `unhandledRejection`, and non-zero error-count lines, so pasted `fail 0`
  success summaries cannot hide failed JavaScript SDK evidence. The same
  release-readiness tests scan the Ethereum and BSC JavaScript, Python, Swift,
  Kotlin/JVM, Java Android, and .NET facade sources for missing files or forbidden
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


<a id="record-342cda41a942d1583ea218120adc6558402fa10adc5f60d01c9f80e3fbf374df"></a>

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


<a id="record-1972d0f28455fb398edc94ab4352c7215bdc16891f7950d54c187736aae95a29"></a>

- Keep Ethereum mainnet SDK local-admission packaging first-class across every
  user prover surface: JavaScript/browser, Python, Swift, Kotlin/JVM, Java
  Android, and .NET now expose ETH -> SORA local-admission builders that bind
  the Ethereum source domain, SORA target domain, canonical `SubmitBridgeProof`
  metadata, normalized verifier/deployment hashes, and copied native verifier
  artifact bytes without WASM or remote-prover fallback.


<a id="record-1387d6780743cf34254bb49e337ac9e8948fbce2cd97a876290a91ea9e9e7fc9"></a>

- Keep TRON route-canary helper and runtime transcript policy aligned: the
  source bridge evidence helper, all-lanes preflight, release-bundle verifier,
  and Rust runtime now all reject finality-height replay across TRON v3
  route-canary hash roles before full rollout TOML or launch readiness can pass.


<a id="record-348d07eb1fdc914661c72ed0537ec1fe8ee23ecca2944401815f5c4aa4fae80c"></a>

- Keep SCCP linked-prover callback snapshots immutable across production
  destinations; JavaScript, Python, Swift, Kotlin/JVM, Java Android, and .NET
  callback regressions now assert frozen request metadata where exposed and
  copy-backed bundle and source-proof bytes across EVM-family, TRON, TON, and
  Ethereum/BSC mainnet facade witness-provider paths. The .NET Ethereum/BSC
  inbound callback snapshots now also clone nested mutable dictionary and
  enumerable evidence values before app-linked callbacks return proof bytes.


<a id="record-9fe28a269d01ad6d17827d6805254662682494b8a8ec32a7d72be4b28626f482"></a>

- Keep public SCCP phase evidence bound to executed production-corridor
  commands; release-readiness and release-bundle checks now require expected
  phase command fragments to appear on traced `+ ...` command lines inside the
  claimed phase block, not merely in incidental test output. The public bundle
  verifier also rejects prefix-alias phase markers, completion sentinels copied
  from a different phase block, and success markers that appear only on traced
  command lines instead of phase output. The verifier owns its required phase
  and phase transcript inventories independently of the report generator, with
  parity tests preventing drift.


<a id="record-8a5127471670106e490197126c80751ca7917a156bb93ecd8d5c9380ca35db31"></a>

- Keep Ethereum mainnet inbound source-proof support on the active
  local-admission proof path: core admits configured Ethereum proofs when the
  governed Ethereum source, deployment, destination-rollout, route-allowlist,
  and canary records are present under `EthereumMainnetLane`; BSC and other
  supported lanes remain gated until their lane policy opens. The BSC Parlia
  receipt/validator fixture remains as non-active lane coverage, including
  replayed deployment-receipt rejection before public-input extraction.
  Remaining release work is broader live deployment artifacts.


<a id="record-ad3516857e0cd3ce81e771378b175eb23f1b6dc288f6bdb7b3704be4d5fb0ee9"></a>

- Keep Python SCCP package-root exports aligned with the public user-prover
  rows; release-readiness tests now import `iroha_torii_client` and require
  every non-callback Python helper/class to be exposed through `__all__`.


<a id="record-220f90b57e8ce6e43dbcba7dadcaa946682ab316bf0c7283985a7ffc8b76031c"></a>

- Keep public SCCP user-prover helper rows one-to-one with real UI hooks;
  release-readiness tests and the release-bundle verifier now reject duplicate
  helper symbols in default and per-SDK rows so repeated names cannot stand in
  for omitted proof-generation entrypoints. The public bundle verifier owns the
  SDK phase inventory independently of the report generator, with parity tests
  preventing drift.


<a id="record-58a9a1486cfad464753e45b19a089ed4c20e9925eac6ad1610ef374ee7e06a14"></a>

- Keep public SCCP user-prover rows tied to UI-owned proof hooks, not only
  request builders; readiness evidence and strict bundle verification now name
  the web/Python witness and prove callbacks, Swift witness/prove typealiases,
  Kotlin proof engines, Java Android nested proof engines, and Solana/TON
  source-state audit engines. The public bundle verifier now also owns the
  lane/SDK helper inventory for those proof-generation and on-chain submission
  entrypoints, plus the exact expected row construction and submission text, so
  weakening the report generator cannot remove cryptographic prover helpers or
  define a shorter portal/mobile table as canonical for published rows.


<a id="record-797ff0308e58c3f726a63babb5f64633f2b4e225533b5beadeef49be9dac2911"></a>

- Keep public SCCP user-prover rows gated by the real release phases; Ethereum
  mainnet source proofs now use lane-local configured readiness instead of the
  global all-lanes gate, so ETH can open with complete mainnet source,
  destination, route allowlist, and canary evidence while other advertised
  lanes remain fail-closed until their own launch policy opens. Core, Torii,
  and bridge-proof regressions now exercise that ETH-only launch gate. Strict
  bundle verification now rejects duplicate, unknown, or missing required
  phases, requires every SDK plus core-admission on each row, and keeps
  EVM/TRON proof backends tied to contract-smoke evidence.


<a id="record-e16d1e6fea1bbf9328967ca8676db102d57b0261aaf46cfe2730fc99dd41e4fa"></a>

- Keep the public SCCP user-prover lane inventory fixed to production
  lane/backend pairs; strict bundle verification now rejects duplicate,
  unknown, or missing rows and backend-id drift for EVM/BSC, TRON, Solana, TON,


<a id="record-7a7b4d94e8645599da75e6e4ef6f0899ee87f8c6cb1a7cda65dbe72f597192a4"></a>

- Keep the public SCCP cryptographic evidence inventory fixed to production
  domains; strict bundle verification now rejects duplicate, unknown, or
  missing domain rows plus chain-label drift before comparing rows with
  embedded all-lanes evidence.


<a id="record-6652cae5f305b43bf67d7307c41c4a08e8a1d968deb6fa2a589243ea02ef8019"></a>

- Keep public SCCP cryptographic evidence rows tied to domain-specific route
  canary and source-gate policy; strict bundle verification now rejects
  incorrect canary sources, impossible source-gate requirements, and missing or
  unexpected named source-gate audit hashes.


<a id="record-83fbe737ee14d552a6b5a7b44ff8d2dc1238692295e46f0a8a22a42b7c6a9961"></a>

- Keep public SCCP readiness Markdown verifier-owned and reviewer-complete;
  strict bundle verification now owns the canonical Markdown renderer, parses
  the Markdown sections independently, and requires copied evidence hashes
  bound to evidence-input path/bytes/hash rows, corridor artifacts bound to
  phase/status/hash rows, checklist gate/status rows, checklist blocker cells,
  cryptographic evidence rows with lane-bound live EVM cells, core hashes, and
  route-canary cells, portal/mobile helper symbols, source-inventory gate/status rows
  and blocker cells,
  user-prover helper/phase rows, user-prover validation-status cells,
  user-prover blocker cells,
  native-prover validation-status cells, native-prover blocker cells,
  native-prover artifact/hash rows, native-prover support-artifact rows,
  lane readiness status rows, lane readiness blocker cells, top-level blockers,
  and release-evidence handoff text to appear in the public report. The
  readiness Markdown invariants source inventory must pin the
  evidence-input path/bytes/hash, production-corridor phase/status,
  production-corridor artifact/hash, checklist gate/status, checklist
	  blocker-cell, cryptographic row live-EVM, cryptographic row core-hash,
	  cryptographic row route-canary, cryptographic row route-canary source
	  whitespace suppression, cryptographic row renderer-visible field
	  diagnostics, lane-readiness status,
  lane-readiness blocker-cell, source-inventory row/status, source-inventory blocker-cell,
  user-prover helper/phase row, user-prover validation-status,
  user-prover blocker-cell, and
  native-prover validation-status, native-prover blocker-cell,
  native-prover artifact/hash row, native-prover support-artifact row verifier checks plus their adversarial
  regression tests before public readiness can pass.


<a id="record-79a28f9b9a5a586451286b4dda45a346a02cfdbc9f9a1d38d3ce17b7fcdadca1"></a>

- Keep public SCCP bundle verification free of generator backdoors for owned
  release artifacts; the verifier no longer exposes report/bundle module hooks
  for canonical Markdown, release-note attachments, copied-evidence summary
  recomputation, corridor inventories, crypto rows, or user-prover surfaces.


<a id="record-ffd354562b09dcaff12868e9b06663d0f164b0f69434bc0f7900ed13d8686d0d"></a>

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


<a id="record-0901694a587fdfa3f96e9ad7f3ac3675e3303f1086685c1f25247b9b6f587e72"></a>

- Keep public SCCP release manifests as verifier roots, not published artifacts;
  strict bundle verification now rejects any `manifest.json` row inside the
  manifest artifact table.


<a id="record-bec1a3238ce61d4cbf6f8a26d4db30912c60429ca658eb34b768559b12ded4c6"></a>

- Keep public SCCP release bundles free of unreviewed filesystem entries; strict
  bundle verification now rejects empty or otherwise unmanifested directories
  instead of comparing only files.


<a id="record-a3c9aeebbd60bebd9caac14f7b41d3461b5883fe44e60616193e45a37b9bceeb"></a>

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
  payload diagnostics are category-only too, and forbidden-payload scanner
  helpers must reject symlinked or directory-backed payload paths before reading
  bytes for no-WASM/no-remote marker scans. Bundle builder, strict verifier, and
  standalone readiness artifact metadata helpers also require regular files
  before hashing artifact bytes, so directory-backed artifacts fail before
  `read_bytes` can surface low-level filesystem exceptions; the same helpers
  now reject symlinked artifact path ancestors before byte hashing. Strict bundle
  verification also rejects manifested artifact paths that pass through
  symlinked bundle directories and distinguishes manifested directory artifacts
  from missing files with the same regular-file category. Sparse inventory
  checks remove the direct release-artifact path, copied filename,
  manifest/report path, native prover payload path, symlinked artifact,
  extracted bundle entry, and secret path-redaction regressions directly.


<a id="record-5ea41fdce0e8e2a8798ee16f67ab2eeeb278e78d367567c203581d34f5f74936"></a>

- Keep public SCCP release evidence UTF-8 fail-closed; strict verification now
  reports non-UTF-8 manifest JSON, readiness JSON, all-lanes summary JSON,
  readiness Markdown, and release-note attachments as structured bundle
  failures instead of raising out of the verifier.

