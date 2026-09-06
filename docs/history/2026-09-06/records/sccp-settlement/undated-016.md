# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-9b760f71d40648304a3c6080591af25ce0da1813667c1c556410b370d49c5aac"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
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
  JavaScript plus `dist`, Swift, Kotlin, Java Android, and .NET expose matching
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
  reject bare, uppercase, or padded bridge addresses, component hashes, runtime
  bytecode, domains, and deployment block numbers before deriving source
  material or deployment-record hashes. TRON source evidence applies the same
  canonical prefix rule to fixed component hashes and runtime bytecode while
  retaining the separate Base58/`0x41` address parser. All-lanes evidence
  validation rejects bare fixed-hash aliases at the copied-summary boundary,
  including EVM source deployment transaction input SHA-256 metadata.
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
  request hashing or app-linked prover invocation. C#/.NET ETH/BSC
  source-material and source-adapter deployment vector evidence is pinned in the
  public release text and strict Markdown invariant so the native EVM SDK
  surface cannot drift silently. Solana full-light-client
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
		  The live JSON-RPC URL input must also be exact text and public-DNS HTTPS
		  unless loopback HTTP is used for local development, so credentialed,
		  query/fragment-bearing, localhost, IP-literal, single-label, `.local`,
		  malformed DNS-label, padded, or control-bearing provider URLs cannot reach
		  request construction.
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
  The surrounding Solana SDK notes are retained pre-release history and do not
  describe a production SCCP V1 lane. TON is an exported SCCP V1 API and Torii
  plus the SDK release checks keep the production surface exact over ETH, BSC,
  TRON, and TON mainnet.
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
  static placeholder manifest. Static disabled-manifest and destination-query
  bypasses now also require that configured source lane to be production-ready
  for the message bundle's source domain and the bundle to target SORA, so
  placeholder, mismatched, or non-SORA-target configured lane objects stay
  blocked before proof wrapping.
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
  deployment binding is zero/zero. Rust core request admission additionally
  rejects opaque non-SORA source-proof bytes by requiring the canonical
  source-chain proof envelope embedded in the SCCP bundle to match the request
  public inputs, so local prover entrypoints cannot wrap source-proof
  placeholders as production evidence. JavaScript, Python, Swift, Kotlin/JVM,
  and Java Android proof-request builders now also reject non-SORA requests
  unless `source_proof_bytes` exactly equals the finality-proof bytes embedded
  in `bundle_bytes`, so SDK callers cannot pass arbitrary opaque source-proof
  placeholders through the local bundle gate. That
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
  accept only java-tron's raw `0..=3` recovery ids, reject Ethereum-style
  `27..=30` aliases, invalid `r` scalars, high-S malleable encodings, and
  out-of-range recovery ids before proof acceptance, and require solid-block
  header proof hashes to recover both child and parent signatures to their
  declared TRON witnesses. Witness schedule
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
  instead of raising before lane blockers can be emitted. Release-bundle
  pre-render and direct all-lanes public-summary validation now pin
  domain-specific duplicate-domain blockers for required, supported-launch, and
  unsupported-launch domain lists before public artifacts or copied roots are
  written, including mixed malformed roots with repeated integer domains. The
  direct all-lanes validator now also keeps duplicate lane-domain diagnostics
  visible for mixed malformed lane arrays containing a non-object row. The
  readiness source inventory now also includes
  `launch_scope_constant_gate`, `retired_network_surface_gate`, and
  `unready_transparent_proof_config_gate`, backed by the same strict source
  marker scans as public release-bundle verification, so the active launch-policy
  constants, supported launch-domain set, launch-scope no-support note,
  exact specific no-support sentence, active-tree scan, and removed
  diagnostic transparent-proof runtime/config surface must remain pinned before
  production reports can pass. The launch-scope constant, retired-network, and unready
  transparent-proof config gates' sparse tests must remove every uniquely
  detectable marker from each inventory row rather than sampling one marker per
  file. Public native EVM SDK
  path-marker denylist strings are assembled from split literals so the
  no-WASM/no-remote source inventory can keep catching actual forbidden
  dependency tokens without flagging the guard implementation itself. The
  Ethereum launch-policy documentation inventory now also rejects stale
  BSC-only production-packaging wording, so artifact/job route docs cannot
  silently drift back to the superseded BSC-first policy. Release-readiness
  reports now publish that documentation inventory as a required source gate,
  so production readiness fails before bundle publication if public launch
  policy wording is missing or stale. Readiness and strict-bundle sparse tests
  must remove every required Ethereum launch-policy docs marker and inject every
  forbidden stale BSC-first marker directly, so this gate cannot degrade to one
  sampled required sentence or one sampled stale sentence; strict release-bundle
  verifier inventory now pins that launch-policy documentation guard directly.
  Public discovery
  documentation now has the same readiness-level source gate, pinning
  supported-lane and verifier target wording before Torii discovery evidence can
  be published as production-ready. Readiness and strict-bundle sparse tests
  must remove every public-discovery marker across launch-scope docs, Torii
  OpenAPI capability/manifest descriptions, the exact no-support sentence, and
  the readiness/bundle sparse guards. The direct all-lanes release checklist now also validates
  required source-adapter gate hashes and expected audit hash roles, rejects
  duplicate or governed-hash-replayed source-gate audit roles, and rejects
  forged source-gate material on lanes whose policy does not require a
  source-adapter gate. The standalone all-lanes CLI release-checklist redaction
  regression now also injects non-string checklist and checklist-item field names,
  so malformed copied checklist keys suppress the public root with bounded blockers
  before operator text can leak. The release-readiness CLI public JSON path now
  applies the same fail-closed release-checklist schema guard before echoing
  nested checklist fields, including non-string copied keys, sensitive titles, and
  hostile blocker strings; that public JSON guard now also requires the exact
  active-launch checklist item ids, rejects duplicated known ids, and reports
  missing required ids with bounded blockers before publishing the checklist root.
  The same guard now also pins canonical active-launch checklist titles per item
  id so safe-looking copied title text cannot replace governed release criteria
  in the public JSON; the active checklist generator now reads from that same
  canonical title map. The public JSON sanitizer also treats `inputs` as
  canonical path strings, not object rows, so ready reports can pass while
  sensitive copied input text still suppresses the root. Public JSON
  `input_artifacts` now also fail closed on copied row drift, including unknown
  fields, non-string keys, sensitive field names, unsafe paths, non-integer
  byte counts, malformed hashes, and missing required artifact fields before
  any artifact row is echoed. Public JSON `source_inventory` now likewise
  suppresses copied gate drift when gate names, gate rows, validation statuses,
  validation blocker lists, or unknown gate-row fields are malformed or
  sensitive, keeping source-inventory operator text out of CLI output. Public
  JSON `user_prover_submission_surfaces` now fail closed on copied row drift as
  well: unknown fields, forged lane/backend/helper/submission/phase contracts,
  duplicated or missing lane rows, and inconsistent validation status/blocker
  pairs suppress the root before user-prover operator text is echoed.
  Release-bundle pre-render validation now also injects
  non-string copied checklist and checklist-item keys into the unknown-field
  regression, so bundle construction halts before Markdown or JSON rendering can
  echo operator data. The active-launch governed-deployment and
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
		  release evidence can pass. TRON source-bridge material now also rejects
		  source bridge owner/emitter address reuse at the config-hash,
		  source-material constructor, and readiness-predicate layers, and the strict
		  source-material role-validation inventory pins that guard. Direct TRON
		  transaction-source proof helpers now reject source bridge contract/owner
		  address reuse both while parsing `TriggerSmartContract` source calls and
		  before materializing source-bridge-bound canonical proof transcripts, and
		  the same role-validation inventory pins those adversarial guards.

