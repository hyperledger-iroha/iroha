# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-bb7e359d30e1afb9f224c5bbc5688244de8fc7e6c3fedbe6cb991acbe2852cdc"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- The canonical SCCP first-release release corridor is limited to Ethereum,
  BSC, TRON, and TON mainnet. Solana, TON testnet, SORA-return, and other lane
  descriptions retained in older roadmap/status history are non-normative
  research history: they must not replace a required mainnet row or add release
  obligations beyond the exact four-profile inventory. Do not add any
  additional network family until the launch scope is explicitly expanded.


<a id="record-aad23167e2a1a3e8e13b395914c7f2d5a0dfcfeddd9f11926ad138b42d244027"></a>

- SCCP .NET Windows recertification contract (the current-release run is
  complete): on a real Windows host with
  stable `.NET 8`, restore `csharp/Hyperledger.Iroha.Sdk.sln`, build the native
  `connect_norito_bridge.dll`, run the full SCCP C# test filter with
  `--logger "trx;LogFileName=sccp-dotnet-sdk.trx"`, and, when Git Bash or WSL
  is available, run
  `bash scripts/check_sccp_production_corridor.sh --phase dotnet-sdk`.
  Record `dotnet --version`, `dotnet --info`, Windows OS/RID/architecture,
  bridge DLL path/hash, VSTest pass/fail/skip totals, the strict
  `SCCP .NET SDK TRX: .../sccp-dotnet-sdk.trx` marker, the positive
  `SCCP .NET SDK TRX bytes: <positive integer>` marker, and the resulting log
  artifact paths in `status.md`.
  The local corridor verifier now rejects XML comments, non-declaration XML
  processing instructions, non-whitespace XML text/tail nodes, namespaced
  trusted VSTest attributes, unexpected trusted-element attributes,
	  non-printable or non-ASCII ignored metadata attributes even after bounded
	  percent decoding, and raw or bounded-percent-decoded sensitive metadata in
	  schema-known trusted attributes in direct TRX XML before parsing or TRX
	  marker publication. The decoder now walks up to eight percent-decoding
	  rounds and rejects values that still decode beyond that bound, so deeply
	  nested percent-encoded sensitive metadata cannot satisfy the Windows
	  handoff. Optional direct-TRX `duration`, `startTime`, and
	  `endTime` metadata must also remain canonical VSTest TimeSpan/ISO timestamp
  values across raw and bounded-percent-decoded forms. Optional `TestRun.id`,
  `testListId`, and `testType` metadata must likewise be lowercase non-zero
  GUIDs across raw and bounded-percent-decoded forms. Optional
  `relativeResultsDirectory` metadata must remain a single printable leaf with
  no path separators, URI delimiters, percent-encoded aliases, or empty dotted
  components. Non-critical VSTest
  extension XML elements may still appear, but their element names, namespace
  URIs, attribute names, and attribute values must stay printable ASCII and free
  of raw or bounded-percent-decoded sensitive metadata; keep those regressions
  pinned when collecting the Windows handoff evidence. Present `codeBase` and
  `storage` assembly-reference attributes must also obey the same printable,
  non-sensitive, non-URI, non-smuggled path-safety envelope and resolve to a
  single `.dll` leaf with no pre-leaf `.dll` segment even when they name decoy
  or non-SCCP assemblies. The final direct-TRX inventory also counts symlinked
  `sccp-dotnet-sdk.trx` matches before marker publication, so hidden nested
  TestResults symlinks cannot coexist with the expected direct evidence file.
  The native bridge digest step must compute the freshly built
  `connect_norito_bridge.dll` digest through the configured Python `hashlib`
  runner instead of PATH `sha256sum`/`shasum`, require one canonical lowercase
  SHA-256 digest line, and suppress raw digest-runner diagnostics before
  emitting `connect_norito_bridge native bridge sha256:`.
  Native .NET proof-request canonical replay is now certified by the
  2026-06-29 stable `.NET 8.0.422` SDK Windows host release evidence. For any
  future recertification on a Windows machine, prefer
  `bash scripts/check_sccp_production_corridor.sh --phase dotnet-sdk` from Git
  Bash or WSL so the strict evidence markers are emitted by the same runner as
  the release checklist. For direct reproduction, record `dotnet --version`,
  `dotnet --info`, OS version, RID, and CPU architecture; run
  `CARGO_TARGET_DIR=<windows-target-dir> cargo build -p connect_norito_bridge`;
  record the freshly built `connect_norito_bridge.dll` path plus lowercase
  SHA-256; prepend the bridge `debug` directory to `PATH`; run
  `dotnet restore csharp/Hyperledger.Iroha.Sdk.sln`; then run
  `dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj --artifacts-path <windows-target-dir>/dotnet-artifacts --filter "FullyQualifiedName~Sccp" -p:ProduceReferenceAssembly=false --nologo --logger "trx;LogFileName=sccp-dotnet-sdk.trx"`.
  The 2026-06-29 blocker cleared after the Windows result counts, the strict
  `SCCP .NET SDK TRX: .../sccp-dotnet-sdk.trx` marker, the positive
  `SCCP .NET SDK TRX bytes: <positive integer>` marker, and TRX/corridor log
  paths were recorded in `status.md`; fixture parity or a non-Windows run must
  not be treated as final release evidence for this item. The source tree now
  includes .NET SCCP canonical-case rejection coverage for proof-request fixed
  hashes, local-admission source-material hashes, message-bundle/source-proof
  binding, and optional Groth16 artifact hash fields; the Windows pass executed
  those tests and surfaced the uppercase byte alias plus `0X`
  public-input, statement, source-material, proof-artifact, and proving-key hash
  negatives in the `sccp-dotnet-sdk.trx` evidence. Required release evidence now
  names that canonical-case coverage explicitly in both readiness Markdown and
  strict bundle verification, so removing the Windows handoff requirement is
  treated as public evidence drift. The corridor and
  strict readiness/bundle verifiers now enforce this handoff by requiring
  `dotnet --version`, `dotnet --info`, `cargo build -p connect_norito_bridge`,
  `dotnet restore`, the full `FullyQualifiedName~Sccp` `.NET` test command with
  `sccp-dotnet-sdk.trx`, a traced `CARGO_TARGET_DIR` bridge build, and
  canonical `.NET 8` Windows/RID/architecture plus canonical native bridge
  debug-output path/hash success markers before
  `dotnet-sdk` evidence can pass. The SDK version marker must be a stable canonical `.NET 8.0.x` SDK
  version with a non-zero patch segment, no preview/prerelease suffix, and no
  zero-padded numeric segments. The production corridor runner must also reject
  multi-line `dotnet --version` output before `dotnet --info`, native bridge
  build, restore, or test execution, so noisy or forged version probes cannot be
  collapsed into a single release marker. The `dotnet --info` output must expose
  exactly one `OS Name:`, one `OS Platform:`, one `RID:`, and either one
  `OS Architecture:` field or, when that field is absent, one Host
  `Architecture:` field; both OS fields must be exactly `Windows`, duplicate
  or missing OS-name/platform fields, including missing `OS Name:`, duplicate
  `OS Name:`, missing `OS Platform:`, and duplicate `OS Platform:` cases,
  missing or duplicate RID fields, duplicate OS-architecture fields, and missing
  or duplicate Host architecture fields when `OS Architecture:` is absent fail
  before bridge build, restore, or test execution. Uppercase RID,
  foreign-platform RID, and alias-architecture RID values such as `WIN-x64`,
  `linux-x64`, or `win-amd64` fail before any release markers are emitted. The
  same is true for architecture alias values such as `amd64`, `x86_64`, or
  `aarch64`; the runner must reject them instead of normalizing them to
  canonical release markers. Colon-injected metadata values such as
  `Windows: Linux`, `win-x64:linux-x64`, or `x64:arm64` must also remain
  visible to the canonical-value checks and fail before release markers. The OS
  marker must be exactly
  `SCCP .NET SDK OS: Windows`, the RID marker must be a
  canonical lower-case Windows RID (`win-x64`, `win-x86`, `win-arm64`, or
  `win-arm`), and the architecture marker must carry a canonical lower-case SDK
  architecture value (`x64`, `x86`, `arm64`, or `arm`) that agrees with the RID
  architecture segment. The production corridor runner must reject uppercase or
  mixed-case architecture values from `dotnet --info` before native bridge
  build, restore, or test execution instead of normalizing them into canonical
  release markers. The final `.NET test`
  success marker must be a VSTest-style summary with `Failed: 0`, a non-zero
  passed-test count, `Skipped: 0`, `Total == Passed`, a numeric unit duration,
  visible label/value and number/unit separators, ordinary-space-only padding,
  and the expected
  `Hyperledger.Iroha.Sdk.Tests.dll (net8.0)` assembly suffix;
  the TRX marker must full-match the direct C# test project
  `TestResults/sccp-dotnet-sdk.trx` path, the TRX bytes marker must be a
  positive integer, the TRX `UnitTestResult` count must exactly match the
  VSTest summary passed count, and named or traversal subdirectories before or after
  `TestResults` remain forged evidence.
  TRX marker path components must be direct and canonical, so
  `TestResults/../sccp-dotnet-sdk.trx`,
  `forged/TestResults/sccp-dotnet-sdk.trx`, and
  `TestResults/forged/sccp-dotnet-sdk.trx` remain forged evidence even when the
  basename is correct. The production corridor runner must reject nested
  VSTest-created TRX files before emitting `SCCP .NET SDK TRX:` or TRX byte
  markers, so runtime evidence and copied release transcripts enforce the same
  direct-project `TestResults` boundary. The existing ancestors of the direct
  `TestResults` path, the direct `TestResults` directory, and
  `sccp-dotnet-sdk.trx` leaf must also be non-symlinked before the strict
  `dotnet test` command can run and again before TRX markers are emitted, so a
  stale symlink cannot redirect VSTest output into copied release evidence.
  The native bridge and TRX local path preflights normalize Windows drive-rooted
  slash and backslash spellings before walking existing components, so the
  Windows host check uses the same ancestor rule as POSIX paths.
  Windows backslash-separated or drive-qualified TRX
  marker paths remain forged evidence too; the release transcript must use the
  canonical project-relative forward-slash marker. The runner must also inspect
  the direct TRX XML before emitting release markers: the file must name
  `Hyperledger.Iroha.Sdk.Tests.dll`, contain at least one passed SCCP
  `UnitTestResult`, and contain no failed, skipped, timed-out, or aborted SCCP
  test results. The runner parses the TRX as XML, requires a VSTest `TestRun`
  root, rejects forged VSTest local names from arbitrary XML namespaces and
  mixed-namespace TRX files while accepting only fully unnamespaced TRX or fully
  VSTest 2010 namespaced TRX, requires exactly one root-level `Results` section
  and exactly one root-level `TestDefinitions` section, rejects nested section
  splices, trusts only `UnitTestResult` rows directly under `Results`, rejects
  any other direct `Results` child, requires every `UnitTestResult` row to be a
  leaf element, trusts only real `UnitTest` definitions directly under
  `TestDefinitions`, rejects any other direct `TestDefinitions` child, requires
  each `UnitTest` definition to contain only direct leaf `Execution` and
  `TestMethod` children, with exactly one direct `TestMethod` and at most one
  direct `Execution` per `UnitTest` definition, and every `TestMethod`
  definition must carry `className` and be a leaf element with a `name`. A passed SCCP result must bind
  by `testId` or `executionId` to a SCCP test definition whose
  `codeBase`/`storage` basename is exactly `Hyperledger.Iroha.Sdk.Tests.dll`,
  and if both identifiers are present they must resolve to the same SCCP test
  definition rather than mixing a valid `testId` with a forged or cross-bound
  `executionId`; every `UnitTestResult` must carry a unique `testName`, that
  name must match the bound SCCP definition rather than a copied non-SCCP or
  different SCCP-looking result name, must carry an exact `Sccp...` token itself,
  and any present `isExecuted` flag must be unpadded literal lowercase `true`
  rather than a truthy numeric, padded, control-bearing, or case-variant alias,
  SCCP TRX definition/result names used for binding must come from the actual
  `TestMethod className.name` pair rather than only a spoofable outer
  `UnitTest name`, must share the same expected assembly evidence from that
  `TestMethod` or its parent `UnitTest`, and must be unpadded, ASCII-only,
  whitespace-free, and control-character-free,
  every TRX `UnitTestResult` row must bind to that same assembly-backed SCCP
  definition set, and SCCP definitions must expose an exact `Sccp...` test-name
  token in the actual test name/class rather than a bare `Sccp` segment,
  embedded or lowercase substring, or runner adapter metadata, TRX `UnitTest`
  and `Execution` ids must be canonical, alphanumeric-ending,
  empty-component-free, and unique, each present `UnitTestResult` `testId` and
  `executionId` value must be canonical, alphanumeric-ending,
  empty-component-free, and unique, every `UnitTest` definition must carry an
  id, every present
  `Execution` definition must carry an id, each fallback `testId`/`executionId`
  binding must be unique,
  the TRX `UnitTestResult` count must exactly match the VSTest summary passed count,
	  and TRX XML is capped at 16777216 bytes with DTD/entity declarations rejected
	  before parsing, including NUL-interleaved declarations from UTF-16 TRX files,
	  so comment-spoofed assembly names, arbitrary helper attributes,
	  single-quoted failed outcomes, missing outcomes, lowercase or padded
	  passed-outcome aliases,
	  deeply percent-encoded sensitive metadata, over-depth percent encoding,
	  non-SCCP passed results, unbound SCCP-looking results,
  wrong-assembly SCCP definitions, embedded `Sccp` substrings,
  lowercase `sccp` tokens, bare `Sccp` segment spoofing,
  `adapterTypeName` spoofing, execution-id drift, mixed
  SCCP plus non-SCCP result sets, mixed mapped and unmapped execution ids,
  missing, duplicate, or mismatched `UnitTestResult@testName` values,
  duplicate `UnitTest`/`Execution` ids, path-like or XML-delimiter-punctuated
  `UnitTest`/`Execution`/`UnitTestResult` identifiers, traversal, URL-style,
  nested `.dll`, or XML-delimiter-punctuated TRX `TestMethod`/`UnitTest`
  assembly-reference paths,
  forged VSTest summaries,
  TRX/count mismatches, oversized TRX files,
  non-`TestRun` roots, forged VSTest namespaces, `UnitTestResult` rows outside
  `Results`, extra direct `Results` children, `UnitTest` definitions outside
  `TestDefinitions`, extra direct `TestDefinitions` children, DTD/entity
  declarations, and malformed XML remain forged
  evidence even when the VSTest console summary looks successful. The release corridor
  phase-transcript source inventory now pins the runner's structured TRX XML
  validator and malformed-TRX negative cases so public readiness cannot pass if
  that local handoff check is removed.
  All canonical `.NET` SCCP marker lines must use a single literal space after
  the colon; VSTest summary label/value and number/unit separators must be
  present, padding must use ordinary spaces only, and tab/control-whitespace
  separators remain forged evidence.
  The native bridge path marker must match the traced `CARGO_TARGET_DIR` value
  followed by `debug/connect_norito_bridge.dll`, and the release
  phase-transcript source inventory must pin that helper plus the
  runner's empty-`PATH` preflight, fake-Windows adversarial path cases, native
  bridge target/output path canonical-character regressions, and the
  readiness/bundle drift regressions. The runner rejects whitespace or control
  characters, empty, dot, or parent segments, and non-portable component
  characters outside the strict bridge marker character set in computed native
  bridge target/output paths before those bridge markers can be printed.
  The VSTest summary, TRX path, and TRX bytes markers must each appear exactly
  once after the strict `dotnet test` command in the transcript;
  success-looking output before that command or duplicate success markers inside
  that command window remain forged evidence. Bare `Passed!` labels,
  zero-passed summaries, skipped summaries,
  wrong-assembly summaries, malformed duration summaries, forged totals, failed
  summaries, transcripts that run or report `dotnet test` before `dotnet restore`,
  the old ETH/BSC-mainnet-only `.NET` filter, extra non-canonical `.NET`
  setup/test commands before or beside the strict SCCP command sequence, uppercase or mixed-case
  RID/architecture markers, mismatched RID/architecture marker pairs,
  malformed version/OS/RID/architecture lines, host markers printed before
  `dotnet --info`, missing TRX markers, zero or
  malformed TRX byte markers, arbitrary TRX-looking paths, traversal-bearing,
  empty-component, whitespace/control-bearing, or punctuation-injected native
  bridge paths, bare bridge build commands, bridge path/target-dir drift, and
  pre-command test success/TRX lines remain not-ready
  evidence.


<a id="record-8b2f6efc42be79974b1bf4d78f33cf459424c92025e019d88f4689b3e38de5fc"></a>

- SCCP all-lanes governed evidence blockers must stay schema-aware: destination
  rollout and route allowlist `blockers` fields must be empty lists of
  non-empty canonical strings with no duplicate values, and scalar, empty,
  padded, duplicate, or non-string entries must remain production blockers
  instead of being collapsed into generic not-ready state.
  Public blocker validators now also re-run control-character, printable-ASCII,
  and Markdown-unsafe-character checks after bounded HTML-entity/URL-percent
  decoding, so encoded newline, RTL/non-ASCII, pipe, or angle-bracket payloads
  are category-only blockers rather than safe public text.
  Release-readiness and bundle verification now pin that
  governed blocker schema as a required source-inventory gate before governed
  evidence can pass; public sparse tests must keep destination-rollout and
  route-allowlist blocker-list hooks, empty-ready diagnostics, sensitive public
  blocker markers, and scalar/numeric/empty/padded/sensitive/Markdown-unsafe/
  confusable/duplicate/non-empty-ready adversarial blocker inputs pinned.
  Readiness and strict-bundle sparse tests must remove every uniquely
  detectable governed blocker marker across implementation, adversarial test,
  gate, and self-inventory rows.
  The governed-blocker schema inventory now also pins one malformed blocker
  sentinel for every active launch lane, including TON, with
  readiness/strict-bundle negative tests that remove a lane sentinel before the
  remaining governed-blocker marker set can pass.


<a id="record-bed4dbfe55e8d26868dea6786e3b6d810cd9f8d501b9f897c721e79da39e680d"></a>

- SCCP active-launch governed-deployment readiness metadata must stay
  canonical: release notes cannot report the governed deployment ready unless
  the normalized source-material, source-deployment, destination-binding, and
  recomputed expected destination-binding hashes are canonical non-zero bytes32
  values, the supplied binding hash matches the expected value, the expected
  match flag is exact boolean `true`, and source-material/source-deployment
  record hashes remain role-separated. The destination-binding hash must not
  replay either source hash, and the active source-adapter gate diagnostics must
  derive their lane label from `ACTIVE_LAUNCH_DISPLAY` while the gate summary
  remains required with a canonical non-zero `evm_source_gate_hash` audit entry
  that matches the published gate hash without reusing source or
  destination-binding hash roles.
  Readiness and strict bundle source-inventory tests must keep the exact flag,
  role-reuse, required, gate-hash, audit-key, audit-hash, destination-binding
  replay, and source-adapter gate replay blockers pinned.
  The active-checklist recomputation also rejects copied source-record hashes,
  destination-binding hashes, source-adapter gate hashes,
  `evm_source_gate_hash` audit hashes, active route-allowlist hashes, and
  active route-canary hash roles that replay built-in template material, so
  canonical-looking template hashes cannot satisfy governed deployment,
  destination-binding, route-binding, or live canary evidence.
  Copied active destination-binding summaries must also fail closed if an
  operator or bundle injects a destination rollout `blockers` container:
  missing remains equivalent to an empty list, but scalar, empty-string,
  padded, non-string, sensitive, or otherwise non-empty blocker lists now keep
  the governed-deployment checklist item blocked and are pinned by the
  active-checklist source inventory. The missing-container path is now covered
  as empty-equivalent in the generator and strict-verifier recomputed checklist
  paths, and the helper defaults are pinned by source inventory.
  Copied active source-adapter gate summaries must follow the same blocker
  container rule: `blockers` may be absent or empty, but scalar, empty-string,
  padded, non-string, sensitive, or otherwise non-empty blocker lists now keep
  the governed-deployment checklist item blocked and are pinned by the
  active-checklist source inventory. The same tests cover missing source-gate
  blocker containers as empty-equivalent while keeping malformed/nonempty
  containers fail-closed.


<a id="record-d9c4353bf8d12aafa94439790042cf2e93beb23eea1246fb82717c232001ef38"></a>

- SCCP active-launch route-allowlist readiness metadata must stay canonical:
  release notes cannot report the launch route binding ready unless the
  normalized source-material, source-deployment, destination-binding,
  route-allowlist, and recomputed expected route-allowlist hashes are canonical
  non-zero bytes32 values and the route hash matches the expected binding tuple.
  The expected-match flag must be exactly boolean `true`, and the strict bundle
  verifier must reject source verifier material/source-adapter deployment hash
  role reuse for the route-allowlist item just as the readiness generator does.
  The route-allowlist hash must also stay distinct from the source verifier
  material, source-adapter deployment, and destination-binding hashes it binds.
  Optional top-level route-canary summary fields in the Rust route-allowlist
  profile gate must also stay exact when present: `status` must be `passed`,
  hashes must be canonical non-zero bytes32 values, the canary route hash must
  match `route_allowlist_hash`, and canary route/evidence/destination hashes
  must remain role-separated.
  Release-readiness and strict release-bundle inventories must pin this
  route-allowlist canary-summary gate so failed, partial, replayed, or drifted
  top-level summaries cannot satisfy production evidence.
  Source-inventory tests must keep the recomputed route-hash mismatch,
  exact expected-match-flag, source-record role-reuse, route-hash replay, and
  adversarial `route_allowlist.hash_mismatch` markers pinned.
  Copied active route-allowlist summaries must also fail closed if an operator
  or bundle injects a `blockers` container: missing remains equivalent to an
  empty list, but scalar, empty-string, padded, non-string, sensitive, or
  otherwise non-empty blocker lists now keep the route-allowlist checklist item
  blocked and are pinned by the active-checklist source inventory. The
  missing-container path is now covered as empty-equivalent in generator and
  strict-verifier recomputed checklist tests, and the helper default is pinned by
  source inventory.


<a id="record-ee351bdb0f22ae86af8764db566501bae18adc02f8096826c164dc2cf204287d"></a>

- SCCP active-launch route-canary readiness metadata must stay canonical:
  release notes cannot report the launch lane ready unless the EVM
  `MessageProofAccepted` evidence source, non-zero transaction hash, finalized
  receipt block number/hash, receipts root, and message id are present in the
  normalized route-canary summary.
  The active route-canary hash roles must stay pairwise distinct: evidence hash,
  transaction hash, receipt block hash, block receipts root, and message id
  cannot replay one another in readiness or strict bundle recomputation. The
  canary hashes also cannot replay the upstream source verifier material,
  source-adapter deployment, destination-binding, source-adapter gate, or route
  allowlist hashes that they are meant to certify.
  Readiness, all-lanes, and strict-bundle adversarial coverage now sweep every
  active EVM canary hash role against every upstream hash role rather than
  proving only `evidence_hash` replay.
  Public Required Release Evidence must also name the upstream route-canary hash
  replay rejection explicitly, and the strict Markdown invariant must fail if
  that phrase is removed.
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
  and booleans are release blockers, and `message_proof_used` plus
  `receipt_block_finalized` must be exactly boolean `true`, not false,
  missing/null, truthy text, or numeric values. Non-boolean copied
  `message_proof_used` and `receipt_block_finalized` values are explicit schema
  blockers before the message-proof-used or finalized-receipt blocker is
  emitted.
  `evidence_bound` must also be exact boolean `true`; non-boolean copied values
  are schema blockers before the not-bound blocker is emitted; copied truthy
  strings, numeric values, false, and missing/null flags must remain
  live-route-canary blockers in readiness and strict bundle
  recomputation. Readiness and strict-bundle recomputation tests must keep
  present JSON `null` distinct from missing fields for evidence binding,
  message-proof usage, receipt finality, and evidence-source validation.
  Source-inventory tests must also keep the evidence-source, transaction hash,
  receipt-block hash/root, message-id, positive block-number,
  message-proof-used, finalized-block, and adversarial block-receipts-root
  markers pinned.


<a id="record-f404a7907b96f9674d174772dd20327b780f03ad9981385c5650f5e3645a8322"></a>

- SCCP release readiness now treats Ethereum noncanonical chain-id coverage as
  a production gate: public SDK and evidence-script regressions must continue
  rejecting padded, uppercase, numeric, and whitespace-wrapped `eth_chainId`
  values before local source-proof evidence is accepted. The Python receipt-proof
  evidence test vector is source-inventory pinned alongside the public SDK
  vectors, and Swift, Kotlin/JVM, Java Android, and C# must keep the same
  uppercase/whitespace/numeric vector markers in the release source inventory.


<a id="record-e960a1b74e32c90f43e2505ffa3d1544df8047f2a1a8e29069642226cb155669"></a>

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
  `sourceDomain` values. Strict release-bundle verifier inventory now also pins
  the bundle-level sparse guard regressions for Ethereum inbound adversarial
  SDK coverage, missing source-event context, Python wrong-domain receipt
  transcripts, and receipt-proof-hash-only coverage directly.


<a id="record-004d1fea09990e040f80882ec53b364a2fc1d9a4f5362c18be327fbd5d3eb771"></a>

- SCCP release readiness now treats BSC inbound adversarial coverage as a
  production gate: public SDK regressions must continue rejecting hash-only
  proof bypasses, receipt-proof metadata drift, source-event digest drift,
  malformed source logs, and missing source-event validation before BSC inbound
  source proofs are accepted. The readiness inventory now removes representative
  BSC inbound markers directly across JavaScript, Python, Kotlin/JVM, Swift,
  Java Android, and C# so the generator gate cannot pass with only one SDK's
  adversarial coverage intact. The Python row must keep the canonical BSC
  receipt-proof transcript rejection for ETH `sourceDomain` values. Strict
  release-bundle verifier inventory now also pins the bundle-level sparse guard
  regressions for BSC inbound adversarial SDK coverage and Python wrong-domain
  receipt transcripts directly.


<a id="record-f90c801ff8eb807257a41800a441b7e00a1d207d89d376a252ca583c8bfbd991"></a>

- SCCP TRON TAIRA XOR route-config generation now rejects production-ready
  route manifests that still carry `disabledReason` or `disabled_reason`, and
  rejects contradictory disabled-reason aliases before a governed Torii overlay
  can advertise a route as live.


<a id="record-4dec6820cae79d774c96b79d3d5ee509c5d28ce4fed6501f05dc1f8603c335b4"></a>

- SCCP TRON TAIRA XOR route-config generation now requires production-ready
  manifests to carry post-deploy live evidence, `fullTomlReady: true`, and the
  offline full-TOML SHA-256 before a governed Torii overlay can advertise a
  route as live.


<a id="record-d71ded7de0df42f51a74d5cf11e3fe8a288787ba14f1b80e5614a9109aa1e459"></a>

- SCCP TRON TAIRA XOR route-config generation now rejects malformed or foreign
  route manifests before overlay rendering: route id, asset key,
  counterparty-domain, verifier target, TRON profile, chain id, and network id
  must match the governed TAIRA XOR TRON lane, and production-ready overlays
  remain mainnet-only.


<a id="record-4268216cd35aa86a9fcb3cea0464136c38528e5069b171baec44173113b7125a"></a>

- SCCP TRON TAIRA XOR route-config generation now recomputes destination
  binding keys and hashes from the declared network, verifier address,
  verifier code hash, and verifier key hash, then rejects rollout or
  `destinationBinding` drift before any governed Torii overlay is emitted.


<a id="record-c41047060c32b5477d40d2746470eda8a5ff9f6ddea62742a7a3f99c55dbc699"></a>

- SCCP TRON TAIRA XOR route-config generation now rejects stale manifest
  payloads whose destination verifier backend/proof family, contract-address
  uniqueness, TAIRA burn-record artifact digest, or settlement route/asset
  metadata drift from the governed TAIRA XOR lane before overlay rendering.

