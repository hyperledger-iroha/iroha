# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-8c98aea0540ba6d828f8b28ed7f0d433d48d23270989ac40c2fe6ba1ba467713"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SCCP source-material evidence must reject built-in template verifier hashes as
  a release gate, not only as local script behavior. The
  `source_material_template_rejection_gate` source inventory pins ETH, BSC,
  and TRON evidence-script guards plus negative tests so
  template-derived source verifier material cannot satisfy production readiness
  silently. The aggregate all-lanes evidence validator also rejects copied
  source-material records that replay those built-in template component hashes
  before release readiness can pass. Source-adapter deployment records that
  replay those template hashes are rejected at the same boundary, including
  same-value material/deployment replays. Copied release/public JSON source
  records now also reject template-derived `source_verifier_material_hash` or
  `source_adapter_engine_deployment_hash` values before a copied ready lane can
  satisfy strict bundle verification. The standalone all-lanes copied-summary
  preflight now rejects the same source-record template replay for ready and
  not-ready summaries before emitting public JSON. Copied source-adapter gate
  hashes and audit verifier hashes for
  ETH, BSC, and TRON are rejected at the same boundary if they
  replay built-in source-material template component hashes, and strict
  release-bundle public JSON validation rejects the same replay in copied
  cryptographic-evidence rows and all-lanes source-adapter gate summaries. The
  release-bundle builder and generated strict verifier now also reject copied
  public cryptographic-evidence source-record template replay directly at the
  row schema boundary, before relying on embedded-lane binding drift, and the
  standalone readiness-report negative coverage now exercises that replay
  rejection across every launch-domain row.
  Standalone release-readiness public cryptographic-evidence validation now
  rejects copied source-record hashes plus those same source-adapter gate and
  audit template replays before copied readiness JSON can publish, even when
  the copied row claims the gate is not required or uses malformed
  gate-required metadata; its negative coverage now exercises source-gate
  template replay rejection across every launch-domain row too. The
  copied active-lane validators must keep reporting those template-derived
  source-adapter gate and audit hashes even when an operator-forged public row
  claims the gate is not required, supplies a non-boolean ready flag, or
  otherwise tries to short-circuit the gate branch before governed deployment
  evidence is checked. The
  all-lanes release checklist now rejects copied source-adapter gate hashes and
  audit hashes that replay the same built-in template material across ETH, BSC,
  and TRON before governed deployment readiness can pass. The
  source-gate audit-key emission controls must also stay exact booleans before
  bundle-builder or strict-verifier helper paths can suppress audit-key shape
  diagnostics. The source-material role validation release inventory now also
  pins the Rust production-material placeholder role replay regression directly,
  so that adversarial coverage cannot disappear while the broader
  source-material gate remains present. That regression now includes the BSC
  governed config-bound source-bridge network, owner, and config placeholders,
  while BSC's material-only envelope profile stays fail-closed. Rust source-adapter
  readiness now also
  tests material-only admission across every active remote launch lane, so
  deployed source material for ETH, BSC, or TRON can be recognized
  as well-shaped without opening production readiness until the external
  consensus, inclusion, and trust-anchor engines are ready. The BSC fixture in
  that guard uses source bridge network/owner/config-bound deployed material
  rather than the BSC material-only envelope profile, keeping the remaining
  recursive source-adapter verifier deployment blocker visible. The BSC
  source-SDK facade and Core bridge-proof helper now use the same config-bound
  constructor and reject coherent source-bridge network/owner replay before
  deployment-bound admission can pass. The generic
  source-adapter deployment builder and matcher now require non-placeholder
  material, and production/local-admission paths require deployment evidence,
  so the BSC material-only envelope profile cannot pass production source-proof
  admission or mint/match a governed deployment descriptor. Release-readiness
  and strict-bundle source inventories must pin those all-lane and BSC
  fail-closed/config-bound readiness markers before
  production evidence can pass. ETH, BSC, and TRON source
  bridge/state evidence must also keep wrong
  source/target lane-domain diagnostics tied to named `SCCP_DOMAIN_*` and
  `SCCP_DOMAIN_SORA` constants, with readiness and strict-bundle source
  inventories pinning those negative paths so active source-to-SORA routes cannot
  fall back to numeric-only checks. Rust
  deployment-backed source-adapter readiness now also loops every active launch
  lane and opens only matching governed source material plus SORA-targeted
  deployment descriptors, while rejecting wrong-domain material and target-domain
  drift before external-engine readiness can turn production-ready. The same
  all-lane Rust regression now mutates descriptor schema version, source domain,
  source-chain label, proof plan, finality model, proof family, circuit id,
  adapter verifier hash, and deployment receipt hash, and each mutation keeps the
  gate closed. It now also mutates every governed source-role binding copied into
  the deployment descriptor: source trust-anchor, consensus, message-inclusion,
  finality-policy, source-state, and source-bridge id/hash/address/config fields.
  The same all-lane loop now mutates the governed Solana Tower replay, full
  AccountsDB lattice, and bank/fork-choice audit hashes plus the TON
  masterchain-config, validator-set transition, and shard-accounts dictionary
  audit hashes, so lane-local audit drift and foreign audit-field injection keep
  readiness closed. Solana/TON production unblocking now requires the governed
  audit hash profile explicitly, so structurally inspectable full-light-client
  audit descriptors with changed role hashes cannot open readiness. Full lane
  production-readiness coverage now also rebuilds production-shaped destination
  rollout and route-canary records for Ethereum, BSC, and TRON, then
  proves replayed source-adapter deployment receipt drift and shape-valid
  route-canary evidence drift both keep the route allowlist closed at the
  canonical lane-evidence join. The Python all-lanes evidence regression now also
  mutates ETH and BSC deployment receipt hashes while refreshing canonical
  source-record hash comments, so raw evidence validation proves EVM source-gate
  transcripts bind the exact receipt. The release source-inventory gate now pins
  the added source-domain, adapter-verifier, non-zero receipt, role-field,
  audit-profile, audit-field, route/deployment binding, canary-evidence drift,
  and EVM receipt-transcript markers beside the existing descriptor-drift markers.
  This closes the previous ETH-only deployment-backed descriptor drift blind spot
  without closing the live verifier deployment tasks.
  Readiness and strict-bundle sparse tests must remove every uniquely detectable
  template-rejection marker from each inventory row, so lane-specific template
  hash guards, copied
  all-lanes evidence guards, public JSON guards, and the release-gate
  self-inventory cannot silently degrade to one-marker-per-file coverage.
  The same gate now also pins lane-coverage sentinels for every supported launch
  source family (`eth`, `bsc`, `sol`, `ton`, and `tron`), with negative
  readiness and strict-bundle tests that remove an entire lane's sentinel set so
  a future inventory edit cannot silently drop one active SCCP launch lane while
  leaving the remaining markers intact.
  The companion `source_material_role_validation_gate` pins zero-hash,
  role-reuse, canonical source-adapter verifier, and full-light-client audit
  role-separation guards across the same source families before source material
  can satisfy release readiness. Readiness and strict-bundle sparse tests must
  remove every uniquely detectable role-validation marker from each inventory
  row, so zero/reused role-hash guards, canonical adapter verifier checks,
  full-light-client audit separation, public diagnostic redaction wrappers, and
  Rust canonical source-state proof preflights cannot silently degrade to
  sampled coverage. The role-validation inventory shares the same active-lane
  coverage check, including Solana, TON, and TRON source-gate sentinels, before
  release readiness can accept the source-material evidence inventory.
  Standalone source-adapter deployment descriptors must also reject built-in
  placeholder ID and hash replay directly across ETH, BSC, Solana, TON, and
  TRON. All-lanes deployment admission must also reject template-derived source
  material hashes replayed into `adapter_verifier_vk_hash` or
  `deployment_receipt_hash`, and the same role-validation inventory pins those
  descriptor-level adversarial checks. Route allowlist hash derivation must also
  reject built-in source-material template hashes supplied as source material,
  source deployment, or destination binding inputs before placeholder evidence
  can seed governed route allowlist digests. Route-canary evidence and
  transcript hashes must also reject built-in source-material template hashes
  directly, so placeholder source components cannot be relabelled as live
  canary evidence or message/finality transcript material in raw all-lanes
  evidence, copied all-lanes public summaries, bundled public
  `cryptographic_evidence`, or standalone readiness public crypto rows; the
  standalone readiness CLI must also suppress the forged hash from public JSON
  when that rejection fires. Raw all-lanes evidence now rejects EVM/BSC, TON, and
  TRON route-canary transcript fields that replay built-in source-material
  template hashes directly, and the template-rejection plus route-canary
  inventories pin those exact regressions. Direct all-lanes release checklists
  and copied all-lanes public-summary preflight now also reject those template
  hashes when copied into destination-binding or route-allowlist actual/expected
  hash roles, so copied public summaries cannot relabel source templates as
  governed destination or route-binding evidence. Release-bundle pre-render and
  strict verification now mirror the same destination/route template boundary
  for embedded readiness evidence and standalone all-lanes summaries. Strict
  verification also runs template-only checks before relaxing non-active
  not-ready lanes, and pre-render validation rejects not-ready source-record and
  source-gate template replays, so diagnostic lanes cannot park source-template
  hashes in nested evidence while deferring full readiness. Pre-render
  validation and strict verification also apply public-schema checks to copied
  non-active not-ready nested lane evidence before their relaxed exits, so
  malformed or hostile unknown nested fields, plus missing nested fields in
  present copied evidence sections, cannot bypass bundle generation or
  verification. Copied not-ready nested sections must also keep expected
  destination/route hashes and route-canary lane binding hashes coherent before
  the relaxed not-ready exit, and their `expected_*_hash_matches` flags must be
  true when the copied expected hash equals the copied lane hash. Destination
  `recomputed` must follow the same rule for copied not-ready destination
  bindings. Copied not-ready destination bindings also preserve lane-specific
  semantics: EVM-family lanes require canonical non-zero network-id and
  bridge-address fields, TRON requires a canonical non-zero network id and no
  bridge-address field, and Solana/TON lanes reject those EVM/TRON-only fields.
  Copied not-ready route-canary common fields also keep bounded
  live-evidence semantics: present `status` must be `passed`, present
  `evidence_source` must match the lane source-adapter class, and present
  boolean `evidence_bound` must be true. For EVM-family and TRON copied
  not-ready canaries, present message-proof, finalized-receipt, owner-match,
  and signature-recovery truth flags must also stay true. Copied not-ready
  route-canary proof context also preserves lane-specific semantics for
  EVM-family receipt/log/proof-domain fields, TRON block/log/proof-domain and
  owner/recovered-owner fields, Solana ProgramData address/slot fields, and TON
  last-transaction logical time. The all-lanes CLI pins the same copied
  not-ready proof-context semantics across BSC, Solana, TON, and TRON before
  returning public summaries, including non-zero canonical base58 Solana
  ProgramData address validation and canonical non-zero `0x41` TRON
  owner/recovered-owner address validation. Copied not-ready route-canary hash
  roles also stay separated from governed source/gate/destination/route hashes
  and sibling transcript hashes before public summaries can render. Release source
  inventories now pin those
  direct, copied-summary, pre-render, strict-verifier, and not-ready
  template/schema/hash/flag-coherence/destination-domain/proof-context/hash-role/common
  and truth-semantic helpers and regressions beside the source-record,
  source-gate, and route-canary guards; the all-lanes evidence-root schema
  inventory also pins the direct and pre-render destination-domain,
  route-canary proof-context, and route-canary hash-role regressions plus
  lane-specific diagnostic sentinels so that coverage cannot collapse to
  function-name-only markers. Generated
  Required Release Evidence now names those not-ready nested schema,
  hash/flag-coherence, and route-canary
  proof-context/hash-role/common/truth-semantic blockers explicitly, and strict
  Markdown checks reject public release evidence that drops that phrase.
  The same source-material template replay paths
  must convert template-loader `SystemExit`, `RuntimeError`, `TypeError`, and
  `ValueError` failures into fixed template-material validation blockers in
  copied all-lanes summaries, release bundle pre-render checks, strict bundle
  verification, and standalone readiness public crypto validation, so helper
  drift cannot leak exception text or skip the fail-closed template gate.
  Source-adapter gate audit-requirement helper failures must follow the same
  fixed-blocker rule in copied all-lanes summaries and release-bundle/strict
  verifier source-gate template checks, so lane audit policy drift cannot leak
  exception text or suppress template-audit replay validation. Release-bundle
  and strict-verifier source-gate audit-key lookup failures must also become
  fixed blockers before copied crypto rows or copied all-lanes gates are
  validated, so imported audit-key policy drift cannot leak exception text or
  skip source-gate public-row checks. Source-gate hash-key lookup failures must
  use the same fixed-blocker path before copied crypto rows or all-lanes gate
  summaries compare gate hashes to audit roles, so hash-role policy drift cannot
  leak helper exceptions or silently skip source-gate hash matching. Core
  source-material and source-adapter deployment config admission now also
  rejects noncanonical hex spellings for governed hash and EVM address fields,
  including uppercase text and repeated `0x` prefixes,
  before decoded bytes can satisfy production material matching. User-level
  SCCP route manifests now reject noncanonical BSC/TRON route hashes, BSC EVM
  addresses, chain ids, optional proof/deployment evidence hashes, and BSC
  explorer transaction hashes instead of normalizing uppercase, padded, or
  repeated-prefix operator input into production readiness. EVM destination
  rollout evidence helpers now also reject bare lowercase hex for fixed hashes,
  EVM addresses, and runtime bytecode, with release inventory markers pinning
  the parser and adversarial tests. Direct ETH/BSC source-bridge evidence
  helpers now apply the same canonical `0x` prefix requirement to source bridge
  addresses, fixed component hashes, and runtime bytecode before source
  material or deployment-record TOML can be rendered. The TRON source-bridge
  evidence helper applies that canonical prefix rule to fixed component hashes
  and runtime bytecode while keeping TRON address decoding on its separate
  Base58/`0x41` address path. ETH, BSC, EVM-destination, and TRON
  runtime-bytecode file inputs now also preflight every existing path component
  as non-symlinked and require the target to be a regular file before parsing
  bytes into governed evidence. All-lanes evidence validation now rejects bare
  lowercase fixed-hash aliases, including EVM source deployment transaction
  input SHA-256 metadata, before public readiness summaries can be rendered.
  Direct Solana and TON destination evidence helpers now apply the same
  canonical lowercase `0x` rule to fixed verifier hashes and inline verifier
  program or code-BoC hex preimages, with bare, `0X`, and uppercase-byte
  aliases pinned as release-inventory regressions. Their binary program/code
  file inputs also preflight every existing path component as non-symlinked and
  require the target to be a regular file before parsing destination evidence.
  Live TON API-key files and TRON Pro API-key/witness-schedule files apply the
  same non-symlink regular-file preflight before runtime evidence collection.
  Solana and TON source-state evidence helpers now apply that same fixed-hash
  rule before source material or deployment TOML can be rendered, and their
  bare, `0X`, and uppercase-byte source-state hash negatives are pinned in
  release inventory as well.
  TON live `accountStates` hash decoding now also rejects `0X` prefixes and
  uppercase hex-byte aliases before account, transaction, or code hashes are
  normalized into rollout evidence, while preserving canonical base64/base64url
  and lowercase hex forms used by public TON APIs.
  Core SCCP recorded-payload collection and committed-block validation are now
  canonical-only: bare lowercase hex, lowercase `0x`, padded, uppercase, `0X`,
  odd-length, and non-hex record-payload aliases are rejected before they can
  contribute SCCP messages or commitment roots.
  Common source-verifier evidence shape must also reject nonzero role-hash reuse
  before OpenVerify wrapper rebuilds or material/deployment matching can run,
  and the role-validation inventory pins the Rust guard plus explicit
  trust-anchor/consensus and message/finality replay cases.
  Deployment-bound verifier evidence must also keep the deployment hash and
  deployment receipt hash role-separated from source verifier hashes and each
  other before OpenVerify wrapper rebuilds, and source-adapter deployment
  descriptors must reject deployment receipt hashes that replay source roles.
  Proof-request source-adapter deployment bindings must likewise reject unpaired
  deployment/receipt hashes and deployment-hash-as-receipt replay while keeping
  the explicit zero/zero diagnostic fixture path hashable. The Rust, JS,
  Python, Swift, Kotlin/JVM, and Java Android proof-request SDK tests must all
  cover deployment-only, receipt-only, and equal deployment/receipt replay
  cases, with the proof-request bundle inventory pinning the SDK markers as
  well as the Rust request-admission markers.
  Python, JavaScript, Swift, Kotlin/JVM, and Java Android proof-request,
  message-bundle, and source-proof hex inputs also reject `0X` prefixes and
  uppercase byte aliases for public-input hashes, statement hashes, optional
	  Groth16 artifact hashes, fixed source-proof hashes, and canonical SCCP
	  message-bundle hash fields; keep the SDK tests and strict bundle/readiness
	  inventory markers pinned. The focused JavaScript/Python, Swift,
  Kotlin/JVM, Java Android, and contract-smoke SCCP corridor phases now pass
  locally, with Homebrew OpenJDK 21 pinned via `JAVA_HOME` for the Java phases;
  the Windows `.NET 8.0.422` corridor/TRX evidence pass is collected for this
  release, and future `.NET` SCCP recertification remains on the same
  Windows-machine handoff path.
  Deployment-derived Rust bindings must additionally require the full
  standalone deployment descriptor shape before minting binding hashes, including
  TON partial-audit, receipt/VK replay, and adapter verifier-key drift
  negatives.
  Python, JavaScript, Swift, Kotlin/JVM, and Java Android TON proof-request builders
  now mirror that descriptor-derived path: `sourceAdapterDeployment` input
  derives the binding, request `sourceStateVerifierHash` must match the
  descriptor, and raw binding/hash overrides cannot drift from the
  descriptor-derived binding.
  Sub&#115;trate/Pol&#107;adot networks are explicitly out of scope for the current SCCP
  launch set; do not add them to production-readiness blockers until the
  launch-scope network policy is expanded.
  All-lanes source-gate recompute wrappers must also convert
  `TypeError` helper/signature drift into category-only blockers instead of
  leaking parser details or tracebacks. That redaction is now pinned through an
  aggregate public-summary regression that injects wrong-signature gate helpers
  for every launch lane and requires fixed, secret-free lane blockers. Copied
  all-lanes canonical base64
  metadata helpers must likewise convert `argparse.ArgumentTypeError`,
  `SystemExit`, `RuntimeError`, `TypeError`, and `ValueError` decoder failures
  into fixed base64 blockers before public readiness output is rendered; the
  shared all-lanes hex/base64 parse helpers now use the same bounded path for
  parser-style `ArgumentTypeError` failures. Direct ETH/BSC/TRON source-bridge
  fixed-hex and runtime-bytecode parsers now also normalize delegated
  `argparse.ArgumentTypeError` failures to fixed hex diagnostics, with release
  inventory pins for the no-leak regressions. TRON source-bridge u32/u64
  decimal parser helpers now also require exact `str` instances or exact `int`
  values before canonical decimal validation, so hostile string subclasses
  cannot run equality, indexing, ASCII, or decimal hooks while public scalar
  arguments are checked. Release-bundle and standalone
  readiness strict hex/Solana public-key helpers now also collapse delegated
  `argparse.ArgumentTypeError` failures to canonical public hex/base58
  diagnostics, so parser details cannot leak through copied public-scalar
  checks. The same parser-error parity now covers non-live EVM destination,
  EVM receipt-proof, Solana destination,
  Solana source-state, TON destination, and TON source-state helpers, including
  Solana program base64 and TON code-BoC base64/base64url decoder failures.
  Live EVM destination/source, Solana, TON, and TRON evidence collectors now
  apply the same bounded `argparse.ArgumentTypeError` path across hex/base64
  metadata, receipt readback, protobuf/result-byte, and route-canary prefilter
  helpers, with all-lanes summary and release-inventory pins for the
  corresponding no-leak regressions.
  SCCP SDK source-gate transcript parity must stay pinned: Python Torii client,
  JavaScript source/dist package, Kotlin/JVM, Android Java, and Swift Solana/TON
  full-light-client gate helpers must write the source-adapter verifier VK hash
  and deployment receipt hash directly beside the aggregate source-adapter
  deployment hash. Keep receipt-drift negative checks and the Solana audit-role
  public-input gate column vectors aligned with the Rust/Python evidence
  transcript.
  Destination binding recompute, route-allowlist recompute, and destination
  verifier identity checks must apply the same category-only handling for
  helper `argparse.ArgumentTypeError`, `SystemExit`, `TypeError`, `ValueError`,
  and `RuntimeError` failures before public blockers are emitted. Source-gate
  recompute wrappers now use the same parser-style `ArgumentTypeError` bounded
  path, and release inventories pin the exact no-leak regressions. It also pins the Rust source-state and
  source-adapter verifier preflights that reject opaque or compressed nested
  FastPQ backend bytes inside OpenVerify envelopes, plus deployment-matcher
  rejection of replayed source-adapter verifier-key hashes before the wider
  production verifier path is consulted; release readiness must fail if that
  direct deployment-matcher regression is removed. The source-material
  role-validation inventory now pins the replayed `adapter_verifier_vk_hash`
  fixture, the direct `sccp_source_chain_proof_matches_adapter_deployment`
  rejection, and the follow-on production verifier rejection as separate
  markers so sparse inventories cannot keep only the wider verifier path.


<a id="record-36cd105094c6353ca9044d7ba1f7e135572af09772b9cf4d5c94ed5845efad67"></a>

- SCCP active-launch readiness metadata must stay canonical: EVM live source
  and destination chain ids in readiness summaries are decimal-only (`1` for
  Ethereum mainnet, `56` for BSC mainnet), so JSON-RPC quantity spellings such
  as `0x1`, leading-zero values such as `01`, whitespace-padded values, and
  plus-signed, decimal-looking, Unicode-confusable, numeric JSON, or oversized
  JSON-RPC quantity values remain evidence blockers. C# destination-binding
  expected-key checks now reject padded or control-character-suffixed expected
  keys instead of trimming them before comparison. The readiness-report and
  strict bundle tests also mutate source and destination metadata independently
  so one canonical side cannot hide the other side's drift. Malformed active
  live-metadata root diagnostics must derive their lane label from
  `ACTIVE_LAUNCH_DISPLAY`, so public release blockers track the configured active
  launch lane instead of carrying stale family wording. Active launch-domain
  diagnostics must also bind the readiness/report active domain to
  `SCCP_DOMAIN_ETH` and report the expected `SCCP_DOMAIN_ETH (1)` label instead
  of a bare numeric `1`; the shared label helper must reject boolean domains as
  non-integer so `True` cannot alias Ethereum in future diagnostic callers. The
  release-bundle builder must import verifier-owned domain constants, domain
  lists, route-canary source map keys, and EVM RPC chain ids through exact-integer
  loaders instead of `int(...)` coercion so verifier drift cannot promote boolean
  constants into SCCP domain ids; the public cryptographic-evidence source
  inventory must pin those loader helpers plus their boolean-alias regression
  test. Readiness public cryptographic-evidence TRON route-canary and non-TRON
  cleanup branches must use `SCCP_DOMAIN_TRON` rather than a literal domain id,
  with source-inventory markers pinning both branches.


<a id="record-41d156b8a45ca82f11143ef8c4ed1ac3621287f8f6f3c19e0c59bf93b8e91b3e"></a>

- SCCP Ethereum source-event context inventory must keep the Rust EVM receipt
  duplicate matching-log rejection pinned alongside receipt-log RPC context
  checks, so one source receipt cannot satisfy admission with multiple matching
  SCCP logs. The release-readiness and bundle-verifier inventory tests now
  remove that Rust marker directly and fail the gate, so duplicate-log coverage
  cannot be satisfied only by the Python receipt-context script tests.


<a id="record-d158a463ec98acd89047b8bb8b7abb99277a9fc294d42064a22b36311349fca5"></a>

- SCCP TON and Solana UI prover requests must stay deployment-bound: JavaScript,
  Python, Swift, Kotlin, and Java Android request builders reject zero/zero
  source-adapter deployment bindings, while the low-level binding normalizers
  keep zero/zero available only for diagnostic fixtures and canonical hashing
  checks, and strict release inventory now pins those SDK guards.


<a id="record-5c11fb5f216767693fe14314be3151a2e8430f0c8b0042be3681746341b62660"></a>

- SCCP JavaScript EVM-family and TRON Groth16 proof request builders must keep
  the canonical bundle gate aligned with Python and Rust: source and dist
  normalizers reject arbitrary bundle bytes, public-input drift, missing
  non-SORA source proofs, and `bundleBytes.sourceDomain` drift before local
  prover callbacks run. JavaScript package-root regressions now exercise the
  same EVM-family and TRON `bundleBytes.sourceDomain` drift rejection through
  the published `dist/index.js` entrypoint.


<a id="record-7ad19d0cdbf214e0b56289b20abb2d76b8e94355d305b7892c45882deef70a7e"></a>

- SCCP Swift, Kotlin/JVM, Java Android, and C#/.NET EVM-family/TRON Groth16
  proof request evidence must stay on the same canonical bundle gate: outbound
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
  appear populated. TON native bundle tests also reject noncanonical EIP-55 EVM
  source senders before non-SORA source proofs can satisfy request building, and
  release readiness pins the Swift, Kotlin/JVM, and Java Android TON parser
  implementation markers directly so test-only rewrites cannot hide parser
  regressions. Public evidence for this gate now names C#/.NET alongside Rust,
  JavaScript, Python, Swift, Kotlin/JVM, and Java Android, and the release-bundle
  native no-WASM inventory now pins package-root JavaScript rejection for tiny
  production `crossSdkParityBytes`, not only legacy
  `crossSdkFixtureParityBytes`. The
  Markdown invariant rejects dropping that suffix. The C#/.NET evidence is now
  backed by `SccpMessageProofBundles.RequireMatchesPublicInputs` in the
  Ethereum/BSC production request builders, plus non-SORA canonical bundle tests
  that reject stripped source proofs, mismatched finality proof bytes,
  public-input drift, payload-body tampering, commitment-root tampering,
  malformed builder bundles, and BSC builder source-domain drift.


<a id="record-5ff9c48473cf454ec75c517ad4f61a97afed12042efd1ac94ce874013a22da10"></a>

- SCCP network scope for the current release remains Ethereum, BSC, TRON, and
  TON mainnet only. Solana, TON testnet, Sub&#115;trate/Pol&#107;adot, and every other
  network family are intentionally outside the production release corridor;
  do not let their public evidence rows, route manifests, deployment
  checklists, or SDK readiness tasks satisfy the four mandatory mainnet lanes.
  Historical Solana and TON-testnet hardening notes below describe
  non-production coverage, not additional launch lanes.


<a id="record-799a116905578d90b6a7b6c6bb1d722be1cd4cc4dddb3919b203bf859bf07c01"></a>

- SCCP client SDK route-canary helper parity must stay pinned: Python Torii
  client, JavaScript source/dist, Swift, Kotlin/JVM, and Java Android helpers
  reject reused route-allowlist, destination-binding, source-material, and
  source-deployment hashes before app-side canary evidence is packaged. Python
  package-root regressions now exercise Solana, TON, and TRON governed hash
  role-reuse negatives through `iroha_torii_client`, so root-import coverage
  cannot be satisfied only by deep `sccp` module tests. JavaScript package-root
  and package-dist regressions now exercise the same Solana, TON, and TRON
  governed hash role-reuse negatives through the published entrypoints.


<a id="record-9238a0ca6ec5697345d551f0f2a198742d73fe08ec8f181987ce468b0231ff54"></a>

- SCCP production-corridor Gradle phases must be self-contained under default
  runner settings: Kotlin/JVM and Java Android phases export a default
  `GRADLE_OPTS` heap corridor (`-Xmx6g` for Gradle and the Kotlin daemon) before
  invoking Gradle, while operator-provided `GRADLE_OPTS` still override those
  defaults. This keeps SCCP SDK validation from producing local memory false
  negatives before tests run. The corridor runner must also reject empty
  `--log-dir` values so local release rehearsals cannot silently skip strict
  phase transcript collection. Release-readiness and strict bundle source
  inventory must pin those direct runner regressions so deleting the heap or
  log-dir guards block public readiness. The inventory now also pins the exact
  operator override input/output and the assertion that default Kotlin daemon
  heap args are absent when `GRADLE_OPTS` is supplied.

