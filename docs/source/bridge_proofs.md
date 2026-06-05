# Bridge proofs

Bridge proof submissions travel through the standard instruction path (`SubmitBridgeProof`) and land in the proof registry with a verified status. The current surface covers ICS-style Merkle proofs and transparent-ZK payloads with pinned retention and manifest binding. Non-SORA SCCP message bundle readback from verified bridge records also requires the stored bridge-proof range to match the transparent artifact's finality height.

Torii now exposes two SCCP bundle families:

- `burn` bundles for the legacy fixed-width burn message path
- `message` bundles for the generic multi-chain SCCP payload family
  (`asset_register`, `route_activate`, `transfer`, `token_add`, `token_pause`,
  `token_resume`)

## Human relay model

SCCP relay to the SORA2 `sccp-bridge` pallet is a manual operator flow. The
production path does not assume an off-chain worker, node-side daemon, or
automated relayer service. A human relay operator uses a bridge web interface to
review a Nexus/Iroha SCCP message, fetch the pallet-ready proof envelope, and
sign the corresponding SORA2 extrinsic through a wallet.

The relay operator is only a courier and transaction fee payer. Authorization
comes from source-chain finality, Nexus commitment binding, and the
cryptographic proof artifacts checked by the destination verifier. Parliament
may govern channel configuration, but it does not approve bridge transactions.
A malformed or unauthorized relay transaction is expected to be rejected
on-chain.

The bridge UI should perform the following checks before preparing a wallet
transaction:

- fetch `/v1/sccp/capabilities` and confirm `runtime_proof_family =
  runtime-scale-v1` and `runtime_verifier_backend = sora-nexus-runtime-v1`;
- fetch the human-readable JSON bundle for the selected message and display the
  payload, message id, commitment root, finality epoch, and finality height;
- fetch the matching runtime SCALE envelope from
  `/v1/sccp/proofs/message/{message_id}/runtime-scale`;
- check that SORA2 already has the required `TrustedNexusFinalityAnchors` and
  destination verifier/trust-anchor configuration; and
- prepare the correct SORA2 call for wallet signing:
  `submit_message_proof`, `submit_token_add_proof`, `submit_token_pause_proof`,
  or `submit_token_resume_proof`.

For the runtime SCALE path, the SORA2 call uses `proof_family =
runtime-scale-v1`, `verifier_backend = sora-nexus-runtime-v1`, and
`bundle_bytes` equal to the raw response body from the `/runtime-scale` endpoint.
`proof_bytes` and `public_inputs` are retained for non-runtime verifier backends
and may be empty for this runtime envelope path.

## User-side prover SDKs

Web portals and mobile apps are expected to gather source-chain witness data,
invoke an app-linked prover, and submit the resulting proof package on-chain.
The JavaScript, Python, Swift, Kotlin, and Java Android SDKs expose local-first
SCCP proof request wrappers for Solana, TON, EVM-family ETH/BSC, TRON, and
Substrate-family runtime destination flows, plus source-adapter transcript
helpers for the witness hashes consumed by those lanes and Substrate-family
GRANDPA lanes. The wrappers do not fabricate cryptographic proofs: they
normalize the canonical transparent public inputs, SCCP bundle bytes, source
proof bytes, statement hash, destination binding hash, and any verifier
deployment binding material before calling the prover supplied by the app.
The ETH/BSC receipt-proof, TON shard-proof, and Substrate-family storage-proof
transcript helpers fail closed on an all-zero source event digest before
hashing source witness material, matching the on-chain requirement that a source
proof commits to a concrete emitted SCCP event.
BSC mainnet inbound SDK facades, including the native Swift, Kotlin/JVM, Java
Android, and .NET surfaces, also fail closed on malformed receipt-observed
source events before local prover callbacks: a log from the configured source
bridge with the SCCP source-event topic must carry exactly two topics, empty
`0x` data, a non-zero digest, and matching transaction/block context, and
duplicate or removed source-event logs are rejected.
The dynamic Python witness-provider path snapshots app-owned request data before
calling the UI resolver, including accepted non-string sequence byte inputs, so
provider-side normalization or mutation cannot alter the proof request that the
portal or mobile app is displaying to the user.
Dynamic JavaScript and Python linked-prover callbacks for TON, EVM-family,
TRON, and Substrate-family flows share the same callback snapshot contract:
callback-visible request objects and nested metadata are frozen where those
flows expose structured request metadata, and `bundleBytes`/`sourceProofBytes`
accessors return defensive copies before proof wrapping.
Swift, Kotlin/JVM, and Java Android final-proof callback regressions now pin
the same source-proof byte snapshot behavior for EVM-family, TRON, TON, and
Substrate-family proof engines alongside their existing bundle-byte snapshot
checks.
Core admission tests pin the same production gate ordering: lane-specific
source-adapter evidence is checked before destination or route activation. The
active launch policy is Ethereum-mainnet lane readiness, so complete Ethereum
mainnet source-proof, source-adapter deployment, destination-rollout,
route-allowlist, and route-canary records can open without waiting for BSC,
Solana, TON, TRON, or Substrate-family lanes. Non-Ethereum lanes remain
fail-closed until their own launch policy opens, while the all-lanes checker
remains as a diagnostic and release-evidence consistency helper. Strict
release-bundle verification applies complete cryptographic-evidence row checks
to the active Ethereum launch lane and keeps future-lane rows diagnostic until
their launch policy opens. Core admission regressions now assert that BSC,
Solana, TON, and TRON route-canary, route-allowlist, and destination-rollout
drift checks remain behind that non-Ethereum lane-launch gate in the
first-release policy.
The Rust helper API exposes `build_sccp_eth_mainnet_source_adapter_deployment`,
`verified_sccp_eth_mainnet_source_chain_proof_envelope_for_production`, and
`verify_sccp_eth_mainnet_source_chain_proof_envelope_production` for the
deployment-bound ETH -> SORA source-admission path. BSC keeps the parallel
helper family, but BSC -> SORA source proofs remain behind the non-active lane
gate until a BSC launch policy opens with governed source-adapter deployment
evidence.
Release-readiness user-prover surface rows therefore require the
`core-admission` corridor phase in addition to the web, Python,
Swift, Kotlin, Java Android, and .NET SDK phases, so a portal/mobile proof path
cannot be marked validated until generated proofs also pass the on-chain
admission surface.
Ethereum mainnet Beacon REST finality collectors in the browser and native
SDKs fail closed when safety flags are malformed: present
`execution_optimistic`, `executionOptimistic`, `finalized`, and finalized-header
`canonical` fields must be JSON booleans before the SDK accepts finalized
header, checkpoint, or receipt-bound finality evidence. The JavaScript
collector also requires `verifyFinalityCheckpoint` / `verify_finality_checkpoint`
overrides to be real booleans, so dynamic browser code cannot accidentally
disable checkpoint matching with numeric or string coercion. Its Beacon REST
provider resolves the target Beacon block from an explicit slot/root/id or from
the execution block timestamp, then fetches that target header, block root, and
block body instead of binding the receipt to the moving finalized head. The
current finalized header and finalized checkpoint still bound the target as
finalized, while generated `beaconFinality.finalizedHeaderRoot` and
`beaconFinality.beaconSlot` identify the finalized header covered by the
Beacon REST light-client finality update. SDK Beacon REST URL builders preserve
endpoint query strings when
appending finalized-header and checkpoint paths, allowing apps to use provider
URLs that carry query-scoped credentials while still sending headers separately.
Browser fetch adapters also
validate Response-like `ok` and `status` fields before parsing JSON, so
malformed custom stubs or non-2xx status codes cannot be treated as finalized
Beacon REST evidence; real browser `fetch` responses prefer bounded
`ReadableStream` reads capped at 1 MiB, fall back to size-checked `text()` when
streams are unavailable, and parse locally before the object-root check, while
lightweight `json()` stubs remain supported for tests.
Native Beacon REST parsers require object-root JSON responses before inspecting
safety fields, keeping malformed roots on the same fail-closed path as
incomplete finality data. Swift, Kotlin/JVM, Java Android, and C# cap Beacon
REST finality response bodies at 1 MiB before JSON parsing, matching the
bounded-response model used by the live evidence tooling; Kotlin/JVM, Java
Android, C#, and Swift also bound their default HTTP transport reads, with
Swift using a URLSession async-byte reader plus an early declared
`Content-Length` rejection and keeping the provider body cap as the final parse
guard.
For Solana source-state proofs, those wrappers also reject prover output unless
the SDK-built request still binds the expected Solana source-domain and
mainnet-genesis public-input columns, recomputes the AccountsLtHash or
full-light audit statement hash from `statementBytes`, and checks FastPQ
`dsid`/`txSetHash` against that same canonical statement before the proof bytes
are packaged for on-chain submission. The dynamic JavaScript and Python Solana
request wrappers also reject duplicate camelCase/snake_case aliases on
top-level request fields, nested FastPQ public inputs, and FastPQ transition
fields before any app-linked prover output is wrapped.
For Solana full-light audit requests, the web, Python, Swift, Kotlin, and Java
Android direct request builders and externally supplied source-state proof
wrappers require the role verifier hash to remain role-separated from the
request-bound source-state verifier, material, deployment, gate, finality,
vote-message, nested AccountsLtHash proof, and audit-statement hashes before a
UI/mobile prover is invoked or proof bytes can be packaged for on-chain
submission.
When user-side proof engines echo optional result metadata, JavaScript and
Python compare numeric slots, role codes, canonical hex hashes, and Solana
audit roles after normalization against the request-bound values, while still
rejecting padded plain string metadata for circuit ids, parameter sets,
verifier ids, and roles. Structured source-state prover result version aliases
(`version`, `proofVersion`, and `proof_version`) are single-alias checked and
must normalize to `v1`, matching the source-state proof capsule normalizer.
FastPQ public-input and transition echoes are compared after canonicalizing
camel/snake aliases, numeric slots, uppercase hex roots, DSIDs, tx-set hashes,
and byte/hex transition values. Public-input columns remain exact request
transcript values, so padded display strings still fail before proof bytes are
wrapped for wallet or RPC submission.
The TypeScript declarations expose that result-metadata shape explicitly for
browser proof engines, while keeping SDK-built request objects readonly and
canonical. The FastPQ metadata declaration requires one accepted alias for each
root/hash and transition byte field, so strict portal builds catch missing
fields before the runtime guard rejects them.
The same declaration layer models source-state proof capsules with required
proof bytes and exactly one circuit-id alias, while structured prover results
require proof bytes but keep circuit-id echoes optional. This matches the
runtime split between standalone proof capsule decoding and request-bound UI
prover callbacks.
Their Solana AccountsLtHash and role-separated full-light audit request
builders now apply the same duplicate-alias rule before deriving the prover
request itself, covering finalized slots, bank-state hashes, blockhash bytes,
source-state verifier metadata, finality-context fields, vote-message hashes,
source-material/deployment selectors, and full-light gate/material/deployment
hash echoes.
The direct v1 Solana finality-context canonicalizers in the JavaScript and
Python SDKs also reject duplicate camelCase/snake_case aliases before hashing
portal-supplied context objects, including Tower vote slots, parent-bank
hashes, AccountsLtHash roots/checksums, stake roots, and Tower replay/bank-fork
transcript hashes.
The Rust `iroha_sccp` verifier regression now also mutates and re-signs the
Solana finality context with a mismatched
`accounts_lt_hash_proof_public_inputs_hash` and asserts that the finalized-vote
proof is rejected, pinning verifier-side binding to the recomputed
AccountsLtHash public-input transcript.
Dynamic JavaScript and Python source verifier material and source-adapter
deployment normalizers also reject duplicate camelCase/snake_case aliases
before deriving governed material hashes. This covers source-domain, verifier,
trust-anchor, finality, source-bridge, target-domain, adapter verifier-key,
Solana/TON audit-role, and deployment-receipt fields, so a portal cannot
display one deployment spelling while hashing another into on-chain evidence.
For optional Solana/TON audit hashes, only omitted fields normalize to the zero
hash; explicit `null`/`None` values are rejected before deployment-record
hashing.
Across JavaScript, Python, Swift, Kotlin, and Java Android, source-material
helpers also reject non-zero lane-inapplicable fields: source-state verifier
hashes on non-source-state lanes, source-bridge emitter fields on non-bridge
lanes, and source-bridge config fields on lanes that do not bind config.
Those same SDKs expose the TRON v3 transaction route-canary evidence helper:
it recomputes the governed destination binding and route allowlist hash from
deployment material, binds transaction/block/log metadata and source-message
public inputs, and requires the recovered TRON signature owner before returning
the canary hash used by production route activation.
Native recursive proof payloads packaged for Solana verifier programs, TON
contracts, and Substrate-family runtimes are capped at 2 MiB by Rust admission
and transparent-proof structure checks. The Solana, TON, and Substrate-family
web/mobile proof-result and submission wrappers mirror that bound before
deriving base64 text, request-bound envelope hashes, or wallet/RPC packages, so
browser portals and mobile apps fail locally instead of producing oversized
proof packages that the chain will reject.
For Solana verifier program and Substrate runtime-call submissions, the
SDK-built `bundleBytes` payload now shares that same native recursive payload
corridor: it must be non-empty, non-all-zero, and no larger than 2 MiB before
any `borsh_instruction_v1` instruction data or SCALE runtime-call envelope is
emitted.
The JavaScript, Python, Swift, Kotlin, and Java Android wrappers also re-check
the canonical FastPQ parameter set, deployed Solana AccountsDB verifier id and
non-template verifier hash, AccountsLtHash direct-parent slot and non-zero
residual hashes, full-light audit role metadata, and the corresponding
OpenVerify public-input columns before accepting externally generated proof
bytes.
Swift, Kotlin, and Java Android Solana source-state prover facades now perform
that canonical request validation before invoking the app-linked AccountsLtHash
or full-light audit proof engine. Malformed mobile requests therefore fail
inside the SDK preflight instead of handing stale `statementBytes`,
OpenVerify columns, or FastPQ transitions to a user-side prover that might
display and prove the wrong transcript.
For dynamic JavaScript and Python final Solana proof requests, duplicate
camelCase/snake_case witness aliases are rejected before the request hash or
app-linked prover input is built. This covers slots, bank-state hashes,
blockhash spellings, message ids, source-state verifier metadata, deployment
material, AccountsLtHash fields, inclusion branches, and nested proof-context
fields.
For dynamic JavaScript and Python Solana prover callbacks, any optional
object-shaped result metadata must also match the SDK-built request; mismatched
source-proof public inputs, proof context, source-state verifier material, or
deployment binding material is rejected instead of being silently replaced.
Those dynamic SDKs also reject duplicate camelCase/snake_case aliases in
Solana proof results and submission inputs, including proof bytes,
proof-context/envelope/deployment hashes, source-state verifier echoes, and
nested source-proof public-input fields, before any wallet/RPC package is
derived for on-chain submission.
Dynamic JavaScript and Python SDK helpers parse SCCP domain ids only from exact
integers or canonical ASCII decimal strings, so `"05"`, `"0x5"`, signed text,
whitespace-padded text, floats, and booleans cannot be coerced into production
lane ids before request or transcript hashing. The JavaScript, Python, Kotlin,
and Java Android unsigned-integer normalizers apply the same canonical
decimal-string rule, with Python also rejecting float truncation, before block
numbers, slots, weights, indexes, or proof public inputs are committed to
transcript hashes. Python EVM-family, TRON, and Substrate proof-request
builders and EVM/TRON destination-binding helpers now also use presence-aware
defaults for backend/proof-family/context fields, so explicit falsey values
cannot fall through to production defaults or top-level proof-context fields
before request hashes or binding hashes are derived. The Python shared Groth16
signal helper, Solana witness/proof request builders, and TON metadata/request
builders apply the same rule to nested `publicInputs`, `proofContext`,
`blockhashBytes`, source-adapter deployment binding, and TON manifest metadata,
so explicit empty objects or falsey byte/hash fields are rejected instead of
being replaced by adjacent top-level values. JavaScript TON and Solana
submission builders also reject explicit non-object nested proof contexts, and
the JavaScript TON request builder rejects explicit non-object deployment
bindings before any request hash or wallet envelope is derived.
Dynamic JavaScript and Python Solana, TON, EVM-family, and TRON submission
builders distinguish omitted fields from explicit `null`/`None`: omitted
fields may be derived from a wrapped proof result, but explicit null
`proofResult`, `publicInputs`, `proofBytes`, `proofContext`, `statementHash`,
`destinationBindingHash`, `proofContextHash`, `publicSignalWords`, or
`bundleBytes` values are rejected before wallet instruction, BOC, or verifier
calldata packaging.
Dynamic JavaScript and Python EVM-family/TRON bridge-proof submit payload
helpers can also build Torii `/v1/bridge/proofs/submit` payloads directly from
the generated verifier-contract submission plus the governed destination
binding. The helpers derive `proof_bytes_hex`, verifier deployment fields, and
the expected destination binding hash, require `message_bundle.commitment`
message id plus `message_bundle.commitment_root`, and bind the Groth16 ABI tuple
to those message-bundle fields plus the SORA source-domain word before a web
portal or mobile-backed prover submits anything on-chain.
Swift, Kotlin, and Java Android typed bridge-proof submit DTO builders enforce
the same message-bundle binding before returning an app-submittable request:
the proof bytes must be a non-zero 384-byte BN254 Groth16 ABI tuple, the tuple
must pass local G1/G2 curve preflight, word 0 must be proof ABI version `1`,
word 1 must match `message_bundle.commitment.message_id`, word 2 must be the
SORA source-domain word, and word 3 must match
`message_bundle.commitment_root`. Their raw JSON submit helpers likewise fail
closed when `proof_bytes_hex` is supplied with a `message_bundle` that omits
that commitment context, so mobile UIs cannot post proof bytes detached from
the bundle shown to the user.
Dynamic JavaScript and Python EVM-family/TRON/Substrate-family proof-result
wrappers now also reject duplicate `requestHash`/`request_hash` and
`envelopeHash`/`envelope_hash` aliases from app-linked prover output. Their
EVM/TRON contract-call and Substrate runtime-call submission builders apply the
same single-alias rule to wrapped proof results, proof context hashes, proof
bytes, bundle/source-proof bytes, public inputs, source domains, and public
signal words before deriving verifier calldata or SCALE runtime calls, so portal
or mobile submission UIs cannot display one field spelling while packaging
another for the counterparty chain.
The shared dynamic JavaScript and Python transparent public-input normalizers now
also reject duplicate aliases inside the public input object itself, covering
message ids, payload hashes, target domains, commitment roots, finality heights,
and finality block hashes before any EVM/TRON/Substrate request hash, public
signal word list, or submission envelope is derived.
JavaScript and Python ETH/EVM receipt-proof helpers now reject duplicate aliases
for source domains, source event digests, beacon slots, execution block/finality
numbers and hashes, receipt roots, beacon finalized roots, sync-committee roots,
receipt proof nodes, and inclusion branches. They also reject non-ETH source
domains before deriving ETH receipt-proof transcript hashes.
The ETH sync-committee payload, transition-message, and transition-signature
helpers apply the same rule to committee public keys/weights/PoPs, transition
periods and slots, finalized roots, parent/next committee hashes, payload hashes,
branch hashes, transition-message hashes, signers bitmaps, aggregate signatures,
and nested proof weight fields; ETH transition transcripts likewise reject
non-ETH source domains before hashing.
The ETH beacon block-header root helpers also reject duplicate slot, proposer,
parent-root, state-root, and body-root aliases before SSZ root derivation, so UI
evidence cannot mix generic and beacon-prefixed header spellings.
JavaScript and Python BSC Parlia receipt-proof, validator-set payload,
validator-set metadata/transition, commit-message, and commit-seal helpers now
apply the same single-alias rule to source domains, source event digests,
validator epochs, block/finality numbers and hashes, receipt roots, proof nodes,
inclusion branches, validator addresses/powers, validator-set storage roots,
slots, values, value hashes, payload hashes, metadata proof hashes, total/signed
power, commit-message hashes, validator public keys, signers bitmaps, and
optional validator-set hash echoes before deriving BSC source proof hashes.
JavaScript and Python Solana message-proof, transaction-status leaf, and
transaction-status root helpers now reject duplicate aliases for source event
digests, transaction-status/receipt-message roots, transaction signatures,
emitter program ids, and inclusion branches before deriving Solana source proof
hashes.
Their semantic vote-account and stake-account data canonicalizers also reject
duplicate aliases for node/voter/withdrawer keys, collector and commission
fields, Tower vote slots, delegated stake, activation/deactivation epochs,
warmup/cooldown bytes, credit counters, and stake flags before deriving the
account-data hashes used by AccountsLtHash proof requests.
Their epoch-stake, stake-activation, stake-account-state, StakeHistory, and
StakeHistory-sysvar transcript helpers apply the same guard to epoch/slot
fields, validator account address/hash vectors, delegated-stake vectors, and
StakeHistory vectors before deriving Solana finality and source-state hashes.
Their Solana active-stake, stake-activation, and stake-history helpers also reject duplicate
aliases for validator public-key rosters, validator stake weights, activation
epochs, and deactivation epochs before deriving Solana finality/source-state
transcripts.
Their Solana account-opening, AccountsLtHash opening-normalization, and
account-inclusion leaf helpers now apply the same duplicate-alias guard to
account addresses, owner program ids, rent epochs, account-data hashes,
finalized slots, opening objects, raw account data, raw-data hashes, and nested
opening addresses before deriving opened-account or inclusion transcripts. When
both raw account data and a raw-data hash are supplied, the JavaScript and
Python helpers recompute the hash from the raw bytes and reject mismatches
before any proof request can be shown, signed, or submitted.
Their Solana opened-AccountsLtHash contribution, opened-account inclusion
witness, and Agave bank-hash helpers now extend that guard to opened vote/stake
array aliases, StakeHistory sysvar opening/raw-data aliases, account-inclusion
root aliases, AccountsLtHash checksum/root aliases, full AccountsLtHash bytes,
parent bank hashes, bank signature counts, blockhash bytes, and optional
hard-fork hash data before deriving residual, inclusion-tree, or bank-state
transcripts.
The lower-level Solana Tower lockout, Tower replay, bank-fork, and
AccountsLtHash recursive public-input transcript helpers also reject duplicate
aliases for finalized slots, epochs, rooted/parent slots, parent-bank hashes,
bank-fork hashes, Tower vote slots, bank hashes, transaction-status roots,
account-inclusion roots, AccountsLtHash checksum/root fields, full
AccountsLtHash bytes, and hard-fork data before deriving the hashes handed to
user-side proof engines.
JavaScript and Python TRON receipt, receipt-state, and transaction-source proof
helpers now reject duplicate aliases for source event digests, receipt/message
roots, transaction roots, transaction indexes/counts/bytes, transaction Merkle
branches, receipt-MPT proof nodes, optional expected bridge emitter/owner
addresses, and inclusion branches before deriving TRON source proof hashes.
Their TRON raw block-header, solid-block header proof, solid-block message,
witness-schedule payload, witness-seal, and witness-schedule transition helpers
apply the same guard to block ids, raw-data hashes, header roots/signatures,
witness rosters/weights, signers bitmaps, transition epochs, transition block
hashes, schedule hashes/payload hashes, nested seal proofs, and transition
message hashes before deriving TRON source-finality evidence.
JavaScript and Python Substrate storage-proof, runtime-storage request,
authority-set payload, authority transition, GRANDPA justification, and
transition-justification helpers now reject duplicate aliases for source
domains, source event indexes, finalized block fields, GRANDPA set ids, storage
roots, authority rosters/weights, payload hashes, transition hashes, signers
bitmaps, nested verifier material, and runtime storage proof hashes before
deriving Substrate source-proof or OpenVerify request material.
Swift, Kotlin, and Java Android EVM-family, TON, TRON, and Substrate-family
mobile prover request builders reject padded fixed-width
public-input/proof-context hashes before request hashes, proof envelopes, or
verifier calldata are derived. Their shared SCCP source-proof helpers apply
the same exact hash rule to source-adapter deployment binding and source-proof
transcript hashes. Kotlin and Java Android additionally reject non-canonical
decimal finality heights at the text parser boundary; Swift exposes typed
`UInt64` finality heights.
The JavaScript web SDK uses the same shared exact parser for portal TON proof
requests and source-adapter deployment bindings, so padded fixed-width hashes
or leading-zero decimal finality heights fail before app-linked prover
callbacks or wallet envelopes are derived.
The operator evidence helpers use the same canonical decimal rule for Solana
ProgramData slots, TON last-transaction logical times, TON workchain ids, and
Substrate-family runtime version fields, rejecting non-ASCII digits,
leading-zero values, and signed forms before destination rollout evidence is
rendered or summarized. The same rule applies to source-domain CLI fields,
EVM-family deployment block numbers, live EVM source/destination RPC chain ids,
live Solana/TON/Substrate metadata, and the all-lanes activation preflight,
including its fallback TOML integer parser, so reviewed operator evidence cannot
change meaning through Python or JSON numeric coercion. Solana, TON, and
Substrate live destination wrappers also require the canonical destination
summary's readiness flag to be the literal boolean `true`; truthy strings or
other malformed summary values remain fail-closed and do not produce offline
TOML hashes. TON and Substrate wrappers revalidate direct live-evidence
dictionaries before deriving destination args, so caller-supplied account
status, BoC hash-match flags, runtime-code metadata, verifier entrypoints, and
hash algorithm labels cannot be forged after bypassing the network collector.
TON destination rollout config now also carries the live `ton_account_status`,
`ton_account_state_hash`, `ton_last_transaction_lt`,
`ton_last_transaction_hash`, `ton_verifier_code_boc_root_hash`, and
`ton_verifier_code_boc` fields. Runtime readiness recomputes the verifier code
BoC root from that staged hex BoC and requires it to match `verifier_code_hash`,
so comment-only or hash-only TON destination records remain diagnostic.
TON route allowlist config also carries
`ton_route_canary_account_state_hash`,
`ton_route_canary_last_transaction_lt`, and
`ton_route_canary_last_transaction_hash`; the runtime route canary gate
recomputes the canonical TON live-account canary hash from those fields, the
destination binding, the source material/deployment record hashes, and the
verifier code BoC root before opening the SORA -> TON lane.
Solana and Substrate-family route canary gates likewise keep executable code
identity separate from governed evidence roles: the Solana verifier code hash
and Substrate finalized runtime code hash must not reuse the route allowlist,
destination binding, source material, or source deployment hashes before the
canary transcript can be accepted.
The EVM live destination TOML renderer likewise revalidates imported summaries
by recomputing bridge and verifier runtime bytecode hashes, backend and
proof-family identities, destination binding hashes/keys, source/target domain
metadata, canonical RPC chain ids, and expected-pin metadata before rendering
production TOML. The EVM source-live TOML renderer now mirrors that import
gate for source bridge runtime bytecode, receipt contract metadata, canonical
ETH/BSC RPC chain ids, source material and source-adapter deployment hashes,
and expected-pin metadata before it can emit production source TOML.
The all-lanes activation preflight now also treats every fixed-width hex value
as exact evidence: surrounding or embedded whitespace in structured hashes,
hash comments, route allowlist hashes, or route canary hashes fails instead of
being normalized during final readiness evaluation. If route canary data is
present both as real `route_canary_*` TOML fields and imported metadata
comments, those values must also match exactly before the lane can report
production readiness.
For lanes where the source proof is optional at request-build time, the SDKs
preserve an omitted `sourceProofBytes`/`source_proof_bytes` field instead of
fabricating placeholder bytes; non-empty all-zero source proofs are still
rejected before request hashing and before submission packaging. Non-empty
source proofs are also capped at the 2 MiB source-state proof corridor before
they can influence the request hash or be handed to a user-side prover. Swift,
Kotlin/JVM, and Java Android apply that same cap to EVM-family, TRON, TON, and
Substrate-family request/proof-result wrappers, matching the shared Rust and
dynamic SDK request builders. The JavaScript
TON request builder now presence-checks `sourceProofBytes` like the EVM, TRON,
and Substrate builders, so falsey non-byte values such as `false`, `0`, or an
empty hex string cannot be treated as an omitted source proof. TON submission
metadata bytes use the same presence check before BOC packaging. App-linked
prover calls, proof-result wrappers, and EVM/TRON/TON submission constructors
carry omitted source-proof bytes through to the proof package so web and mobile
prover UIs can submit externally generated proofs on-chain without fabricating
source-chain witness bytes.
The same Swift, Kotlin, and Java Android mobile SDKs now join the Rust,
JavaScript, and Python clients in exposing raw JSON submit helpers for
`POST /v1/bridge/proofs/submit` and `POST /v1/bridge/messages`, so app-side
proof generation can hand the resulting SCCP proof package directly to Torii
for on-chain submission. Rust, JavaScript, Python, Swift, Kotlin, and Java
Android raw submit preflights now also bind `proof_bytes_hex` to the local
`message_bundle` when the bundle carries `commitment.message_id` and
`commitment_root`, rejecting a tuple with the wrong version, message id, SORA
source-domain word, or commitment root before the request is posted. Those
raw-submit preflights also enforce the same two-way deployment relationship as
Torii: destination proof material requires `proof_bytes_hex`, and
`proof_bytes_hex` requires destination proof material. Recognized raw
destination fields are shape-checked locally as 32-byte network IDs and hashes,
20-byte EVM addresses, or exact non-padded TRON Base58Check verifier addresses
before any bridge-submit request is sent. When proof bytes are present, the SDKs also
require a complete EVM tuple
(`network_id_hex`, `verifier_address_hex`, `bridge_address_hex`,
`verifier_code_hash_hex`, `verifier_key_hash_hex`,
`expected_destination_binding_hash_hex`) or a complete TRON tuple
(`network_id_hex`, `tron_verifier_address`, `verifier_code_hash_hex`,
`verifier_key_hash_hex`, `expected_destination_binding_hash_hex`) and reject
mixed EVM/TRON material locally. For TRON raw-submit tuples, those SDK
preflights also recompute the canonical TRON destination binding hash from the
network id, Base58Check verifier address, verifier code hash, and verifier-key
hash, and reject a forged `expected_destination_binding_hash_hex` before the
request is posted. Bridge-proof submit clients now also enforce
the endpoint shape locally: callers must provide exactly one of `burn_bundle` or
`message_bundle`, and deployment destination fields plus `proof_bytes_hex` are
accepted only with `message_bundle` submissions.
Rust, JavaScript, Python, Swift, Kotlin, and Java Android UI prover helpers now
derive the EVM-family or TRON `destinationBindingHash` from the governed
deployment tuple itself. The EVM-family tuple binds the network id, SORA source
domain, ETH/BSC target domain, Groth16 BN254 backend, `stark-fri-v1`, verifier
address, bridge-wrapper address, verifier bytecode hash, and verifier-key hash;
the TRON tuple binds the same backend/proof-family/domain/code/key fields and
requires an exact, non-padded, valid non-zero TRON Base58Check verifier address.
Web/Python request
builders reject a raw `destinationBindingHash` that disagrees with that derived
material, Rust request/result wrappers bind the same deployment object into the
request hash, public signal words, and envelope hash, and Swift, Kotlin, and
Java Android request/submission constructors can accept the derived binding
object directly, reject mismatched binding metadata, and thread the derived hash
into request hashing or verifier-call packaging. The JavaScript package exposes those
helpers through both the package root and `@iroha/iroha-js/sccp`, matching the
published TypeScript declarations used by portal builds.
Dynamic JavaScript and Python EVM-family/TRON destination binding helpers also
reject duplicate camelCase/snake_case aliases for network ids, verifier
addresses, verifier-code/key hashes, backend/proof-family selectors, binding
hashes, and proof-context destination-binding fields before deriving request
hashes or invoking user-side provers. Their EVM/TRON/Substrate-family proof
request builders apply the same guard to `publicInputs`, `bundleBytes`,
`sourceProofBytes`, `sourceDomain`, and `proofContext`, and the Substrate
proof context also rejects duplicate nested binding-hash aliases. Top-level UI
request aliases therefore cannot select one payload for display while hashing
another.
Rust, Python, and JavaScript artifact/job query builders plus the bridge-feature
CLI also require `proof_bytes_hex` whenever deployment destination fields are
supplied and reject standalone `proof_bytes_hex` without those deployment
fields. They apply the same complete EVM/TRON tuple and mixed-material guard,
so operator-side package fetches cannot advertise incomplete EVM/TRON package
material to Torii.
Solana proof-result wrappers and Solana program-instruction submission builders
now reject raw proof-byte-only submissions across JavaScript, Python, Swift,
Kotlin, and Java Android. Production Solana wallet/RPC instruction bytes must be
built from a wrapped SDK `proofResult`, and that wrapped result rejects empty or
all-zero proof bytes before on-chain submission.
The final Solana, TON, and Substrate runtime submission builders also preflight
`bundleBytes` with the same non-empty, non-all-zero, 2 MiB native recursive
payload gate, so a portal or mobile app cannot package an inert or oversized
Solana verifier-program bundle, TON message-body BOC, or Substrate runtime-call
payload for wallet/RPC signing.
The JavaScript distributable and TypeScript declarations expose the same rule,
so portal builds cannot type-check a Solana submission without a wrapped
`proofResult`. The package-dist regression suite also exercises the published
`dist/index.js` Solana, TON, EVM/TRON, and Substrate submission guards, so
release packaging cannot lag the source-side proof-result or `bundleBytes`
guards unnoticed.
Substrate-family submission builders across JavaScript, Python, Swift,
Kotlin/JVM, and Java Android reject non-empty standalone `sourceProofBytes`
unless a wrapped `proofResult` is supplied. The final runtime-call payload
includes proof bytes, transparent public inputs, and the recursive bundle, but
not those request-bound source-proof bytes, so accepting them without a wrapped
request hash would let a portal or mobile app display proof material that is
not actually submitted.
The Rust `iroha_sccp` Solana, TON, and Substrate-family counterparty submission
package builders enforce the same cap on canonical bundle bytes before emitting
`SolanaProgramInstruction`, `TonInternalMessage`, or `SubstrateRuntimeCall`
payloads, keeping chain-side release tooling aligned with the portal and mobile
SDK submission surface.
The JavaScript local-prover callback declarations also expose request-bound
Solana source public inputs, source-state verifier ids/hashes, proof context,
source-adapter deployment binding, proof base64, and envelope hashes. Browser
proof engines can return that metadata with the proof bytes, and the SDK
rechecks it against the canonical request before producing the wrapped result
used for wallet/RPC submission.
The JavaScript and Python Solana source-state prover callback paths apply the
same fail-closed rule to structured OpenVerify/FastPQ prover results: optional
source-state verifier, AccountsLtHash residual, audit-role, verifier-hash,
public-input-column, FastPQ transition, and statement/context/schema/commitment
byte metadata must match the SDK-built request before the source proof capsule
is accepted.
The shared callback guard applies the same request-bound check to TON
source-state result objects for TON audit-role aliases, masterchain/shard
seqnos, shard-state public-input/proof hashes, public-input columns, FastPQ
transitions, and statement/context/schema/commitment bytes. JavaScript
TypeScript declarations expose that structured TON result object so browser
portal proof engines can type-check the metadata they return before the SDK
wraps proof bytes for on-chain submission.
Those submission builders also require the wrapped proof result's
`publicInputs.sourceStateVerifierId` and `publicInputs.sourceStateVerifierHash`
to match the top-level wrapped source verifier fields, preventing tampered
portal/mobile proof-result metadata from presenting a different source-state
verifier before wallet submission.
They now also pin wrapped result, proof-context, deployment-binding, and
transparent-public-input versions to `v1`, require `proofBase64` to match
`proofBytes`, and reject source-proof public inputs whose finalized/parent slot
pair is not adjacent, whose bank signature count is zero, or whose
bank/source-state hash fields are zero.
Swift, Kotlin/JVM, and Java Android Solana submission wrappers compare
proof-context statement hashes, destination-binding hashes, transparent public
inputs, and submission public inputs after canonical hex or slot
normalization. Mobile proof engines can therefore echo equivalent canonical
metadata while padded or genuinely different fields still fail before wallet
instruction bytes are produced.
JavaScript and Python submission builders also recompute the canonical
transparent public-input bytes and reject caller-supplied `publicInputsBytes`
that differ from the structured public inputs, even if the byte length is
correct. They also distinguish omitted submission fields from explicit
`null`/`None`: a portal cannot null out `publicInputs`, `proofBytes`,
`proofContext`, `statementHash`, or `proofContextHash` and silently fall back
to wrapped proof-result metadata. Swift, Kotlin, and Java Android derive those
bytes internally through typed non-null submission inputs. Java Android EVM and
TRON proof results also snapshot and freeze public-signal word lists, matching
Kotlin's immutable wrapper behavior so app-side list mutation cannot alter a
wrapped proof package after construction.
Kotlin/JVM Solana source-state prover callbacks receive cloned AccountsLtHash
and full-light audit OpenVerify/FastPQ request objects, with byte payloads and
transition values copied before the app-linked proof engine runs; returned
proof bytes are still wrapped against the original canonical request.
Solana `borsh_instruction_v1` submission builders are locked to the SORA ->
Solana destination lane: the transparent `publicInputs.targetDomain` must be
Solana, and the proof-context `destinationBindingHash` must equal the canonical
SORA -> Solana destination binding, before wallet/RPC instruction bytes are
produced. The JavaScript TypeScript submission declarations also require
exactly one wrapped proof-result alias, `proofResult` or `proof_result`, so a
portal cannot type-check an on-chain Solana submission input with duplicate
proof-result spellings.
Solana source-proof witness and request builders in those same SDKs are also
locked to the production Solana -> SORA lane: any non-SORA target domain fails
before request hashing or app-linked prover invocation.
TON proof-request builders now apply the same source-lane discipline to their
source-adapter deployment binding: the request continues to expose the
transparent public-input target domain for TON submission, but the deployment
binding hash is always keyed as TON -> SORA and caller-supplied nested bindings
with any other target domain are rejected before prover invocation.
TON source transcript helpers now fail fast on internally inconsistent
masterchain config and validator-set transition material before app-linked proof
generation starts. They require version-1 TON source metadata, nonzero
masterchain/config/validator hashes, config parameter 34 openings whose BoC
payload hash, validator-set hash, and config leaf hash match the caller-supplied
fields, adjacent validator-set sequence numbers, and an inner validator
signature proof bound to the same transition message hash.
JavaScript TON proof requests and proof results now use the same immutable
browser-prover contract as the hardened Solana path: callback-visible request
objects, nested proof-context and deployment-binding records, and returned proof
results are frozen, while byte fields are exposed through defensive-copy
getters. TON wallet/liteserver submission envelopes produced by the JavaScript
SDK now freeze the envelope and argument metadata and expose BOC/envelope bytes
through defensive-copy getters as well. JavaScript Substrate-family runtime
proof requests and results now apply the same frozen object and defensive byte
getter contract for callback-visible request bytes and returned proof bytes,
and optional callback result metadata for transparent public inputs, proof
context, statement hash, and destination binding hash is rechecked against the
canonical request before an envelope hash is derived,
and the JavaScript package root re-exports that backend id, request builder,
and prover facade so web portal imports are runtime-available from the same
entrypoint described by the TypeScript declarations. The same package root
also re-exports the source-adapter OpenVerify circuit id, FastPQ parameter-set
id, and verifier VK hash helper used by portal evidence checks.
JavaScript TypeScript declarations now publish named local-prover callback
result types for Solana, TON, EVM-family, TRON, and Substrate-family prover
facades. Those types include the optional request hash, envelope hash,
backend, binding-hash, proof-context, public-input, and public-signal metadata
that the runtime validates when app-linked web prover UIs return proof bytes.
Those JavaScript and Python prover facades also accept either a plain
witness-provider function or an object exposing `resolveWitness` or
`resolve_witness`, so browser portals, backend relays, and mobile-adjacent
tooling can use the same witness hook shape before canonical request preflight.
The JavaScript runtime now rejects duplicate prover hook aliases
(`witnessProvider`/`witness_provider`, `resolveWitness`/`resolve_witness`, and
`prove`/`proveFn`/`prove_fn`) instead of applying precedence, and the published
TypeScript declarations model those hooks as exactly-one alias unions. Python
portal-backend witness-provider objects likewise reject duplicate
`resolve_witness`/`resolveWitness` methods before request construction.
TON declarations now also separate the pre-proof request input from the
post-proof message-body submission input: `buildTonSccpProofRequest` and
`TonSccpProver` accept `TonSccpProofRequestInput`, while
`TonSccpMessageBodyInput` keeps proof bytes, wrapped proof results, metadata,
manifest, and query-id fields for wallet submission packaging.
The Python package root now also exports every public SCCP helper/class/constant
from `iroha_torii_client.sccp`, including the Solana submit entrypoint, fixed
transparent public-input byte length, and TON audit-role verifier ids used by
portal backends.
The JavaScript package entrypoint mirrors those portal constants so TypeScript
and runtime imports agree for the fixed public-input byte length, Solana submit
entrypoint, and TON full-light-client audit verifier ids.
It also re-exports the Solana full-light audit request builders, source-state
capsule canonicalizers, finality/vote transcript helpers, and account-inclusion
tree helpers from the package root, matching the TypeScript declarations used by
web portal prover UIs.
Rust counterparty submission package builders and transparent-proof structure
verification now apply the same non-empty, non-all-zero proof-byte preflight to
Solana, TON, and Substrate-family native recursive submission payloads before
encoding wallet/RPC envelopes. The same transparent inner-proof and package
builders now reject native recursive submissions when the transparent
public-input target domain is not one of the manifest lane endpoints. This
keeps outbound SORA -> counterparty proofs and inbound counterparty -> SORA
proofs valid while preventing a well-formed bundle from being wrapped under a
sibling lane's verifier manifest. Default destination rollout blockers for
those native lanes now stay focused on missing live verifier deployment and
trust-anchor evidence instead of claiming the already-modeled submission package
path is unwired.
Solana local-prover facades and proof-result wrappers now also require the
production AccountsDB source-state verifier id, a non-zero source-state
verifier hash that is not the Rust template verifier hash, a non-zero
source-adapter deployment hash, and a non-zero deployment receipt hash before
invoking or packaging app-generated proof bytes.
Rust source proof admission now also requires governed Solana full-light-client
audit deployments to be represented by role-separated proof capsules in the
source proof itself. When the Solana full-light-client gate is present, the
Tower replay, full AccountsDB lattice, and bank/fork-choice verifier hashes in
the deployment record must each match a corresponding OpenVerify/FastPQ proof
capsule; missing capsules, cross-role-spliced Tower/AccountsDB/bank-fork
capsules, or tampered capsules fail before production submission can pass.
TON uses the same proof-data posture for its governed full-light-client audit
bundle. When the TON gate is present, the source proof must carry separate
OpenVerify/FastPQ capsules for masterchain config, validator-set transition,
and shard-accounts dictionary verification, each bound to the governed source
material hash, source-adapter deployment hash, gate hash, and role verifier
hash. Missing capsules, role-spliced capsules, tampered proof bytes, duplicate
audit verifier hashes, audit hashes that reuse existing verifier material, or
audit hashes that reuse built-in TON template component hashes fail before
deployment-aware production admission can pass.
TON source-state JSON summaries now mirror the Solana operator diagnostics:
`source_verifier_material_ready`, `source_adapter_engine_deployment_ready`,
`source_adapter_gate_ready_with_full_light_client_evidence`,
`source_adapter_gate_blockers`, and `full_toml_ready` distinguish pinned
material, pinned source-adapter deployment, and complete full-light-client TOML
readiness before rollout automation stages governed source records.
The JavaScript, Python, Swift, Kotlin, and Java Android SDKs expose the
matching second-stage request builders for user-side UI provers: after the
browser or mobile app generates the AccountsLtHash source-state proof, it can
build the Tower replay, full AccountsDB lattice, and bank/fork-choice audit
requests with the same finality-context hash, vote-message hash, AccountsLtHash
proof hash, deployment hash, and full-light-client gate hash that Rust
admission rechecks. Those request builders reject empty or all-zero nested
AccountsLtHash proof bytes before deriving the audit-role statement hashes, so
browser and mobile provers cannot cascade placeholder source-state proof
material into the second-stage full-light-client audit. They also reject
duplicate Solana audit verifier hashes, and the Swift, Kotlin, and Java Android
builders recompute the Solana full-light-client gate hash from the source
material hash, source-adapter deployment hash, and three role verifier hashes
before accepting direct UI inputs. The standalone mobile gate-hash helpers
rerun the same role-separation check before returning a gate commitment. Those
full AccountsDB lattice audit statements bind the nested AccountsLtHash proof
capsule hash directly, rather than substituting only its public-input
transcript hash, so the second-stage proof is tied to the actual completed
source-state capsule generated by the user's prover. Those
mobile paths also reject direct UI inputs that reuse any source-adapter material
role hash as an audit role verifier: source trust-anchor, consensus,
message-inclusion, finality-policy, source-state verifier, adapter verifier-key,
or deployment-receipt material.
Release-readiness JSON now records those portal/mobile entrypoints per SDK in
`user_prover_submission_surfaces[*].sdk_helper_symbols_by_sdk`, keyed by the
same `js-sdk`, `python-sdk`, `swift-sdk`, `kotlin-sdk`, and `java-android`
phases that gate release evidence. The legacy `sdk_helper_symbols` field remains
the JavaScript/web symbol list, and release-bundle verification rejects drift
between that list, the per-SDK helper map, the rendered helper string, and the
current corridor phase results before release notes can be accepted. The public
release-bundle verifier owns the same SDK phase inventory, so a weakened report
generator cannot shrink the per-SDK helper-map coverage expected in published
attachments. It also owns the lane/SDK helper inventory for the cryptographic
proof-generation entrypoints that must be visible to portal and mobile apps, so
a weakened report generator cannot drop Solana/TON full-light-client proof
builders, source-state provers, EVM/TRON receipt/source-proof helpers,
Substrate runtime-storage proof builders, or on-chain submission helpers from
copied release rows. It also requires every user-prover row to stay gated by the web,
Python, Swift, Kotlin, Java Android, and core-admission phases, with
contract-smoke evidence still mandatory for EVM-family and TRON contract-backed
proof backends. It independently pins the public row inventory to the production
lane/backend pairs (`eth,bsc`/EVM Groth16, `tron`/TRON Groth16, `sol`/Solana
recursive, `ton`/TON contract, and `substrate`/Substrate runtime), rejecting
duplicates, unknown lanes, missing rows, or backend id drift before release
attachments can pass verification. The same public bundle verifier pins the
cryptographic evidence table to the production SCCP domain/chain inventory,
rejecting duplicate domains, unknown domains, missing domains, and chain-label
drift before comparing each row to embedded all-lanes evidence. Each public
cryptographic evidence row must also use the route-canary source and
source-adapter gate policy for its production domain, including exact named
audit hashes for source-gated Solana, TON, TRON, and Substrate-family lanes.
Those maps also require the user-owned prover hooks, including JavaScript/web
`witnessProvider` and `proveFn`, Python `witness_provider` and `prove`,
Swift witness-provider protocols and `ProveFunction` typealiases, Kotlin proof
engine interfaces, Java Android nested `ProofEngine` interfaces, and the
Solana/TON source-state proof engines used for full-light-client audit roles.
Release-readiness tests also reject duplicate helper symbols in any public
per-SDK row, so a repeated helper cannot mask an omitted proof-generation hook
in the portal or mobile submission surface. The public release-bundle verifier
applies the same fail-closed checks to attached release artifacts: duplicate
helper symbols and rows missing the required UI-owned witness/prover hook
markers are rejected even before the row is compared with the current generated
surface.
The same public release evidence now binds hashed corridor phase logs to an
exact claimed phase-marker line, the expected traced phase commands, a
phase-local completion sentinel, and phase-specific success output inside the
claimed phase block. The public bundle verifier owns the required corridor
phase inventory as well as the transcript inventory, so a weakened report
generator cannot shrink the production corridor by omitting a required SDK,
contract-smoke, Rust verifier, evidence-script, or core-admission phase. A
phase artifact with only the phase marker and the completion sentinel is
rejected unless the same block also contains the Rust, script, SDK,
contract-smoke, or core-admission command fragments on the
corridor script's `+ ...` command lines and non-command success output that
proves the claimed phase actually ran and passed. Prefix-alias phase markers,
completion sentinels copied from another phase block, and success text echoed
only on a traced command line are rejected. The JS SDK phase is also checked
specifically for the packaged `dist` and package-root SCCP export tests, so
release notes cannot claim web-portal proof-generation readiness without
evidence that app-facing imports were tested. The report and bundle-verifier
helper inventories also require the native Ethereum beacon-finality helper
symbols, so release evidence cannot omit the typed provider-evidence builders
and inbound-evidence construction helpers that Swift, Kotlin/JVM, Java Android,
and .NET apps use for ETH source proofs.
The packaged JavaScript SCCP tests now also assert that the browser-facing
Ethereum and BSC mainnet artifacts contain no `WebAssembly`, `wasm`, `snarkjs`,
remote prover, prover URL, or prover endpoint dependency markers, keeping the
easy web path tied to app-owned local proof generation. The release report and
strict bundle verifier require both the Ethereum and BSC no-WASM test names as
JS phase output before accepting a published readiness bundle; they also
require the package declaration test name for BSC mainnet Parlia finality
evidence hooks so typed browser evidence fields cannot drift silently.
The release-readiness script tests also scan the Ethereum and BSC SDK facade
source files for JavaScript, Python, Swift, Kotlin/JVM, Java Android, and .NET,
rejecting missing facade files or any `WebAssembly`, `wasm`, `snarkjs`,
remote-prover, prover-URL, or prover-endpoint dependency marker. That keeps the
Ethereum and BSC mainnet SDK launch paths native or local-prover owned across
browser and native SDKs. Native EVM Groth16 prover bundles must also attach
non-empty proof-artifact, proving-key, verifier-key, and per-SDK implementation
payload files; readiness generation, bundle generation, and strict bundle
verification reject a manifest that merely hashes an empty payload. All bundle
hash fields must be canonical lowercase `0x`-prefixed 32-byte hex values, and
the JS/browser, Swift, Kotlin/JVM, Java Android, and .NET SDK bundle parsers
enforce the same canonical form before exposing descriptor hashes to apps. The
same parsers treat the signed bundle manifest as a closed schema: unknown
top-level fields, unknown per-SDK artifact fields, and duplicate accepted
camelCase/snake_case aliases are rejected before descriptor hashes can reach
app prover code. Manifest domain strings must also be canonical decimal text,
so leading-zero forms such as `"01"` cannot be accepted as Ethereum mainnet.
Readiness generation and public release-bundle generation also reject duplicate
JSON keys in the signed native prover manifest before schema, hash, or payload
path checks run, so reviewed bundle fields cannot rely on last-key-wins JSON
parsing.
The
proof-artifact hash, proving-key hash, verifier-key hash, destination-binding
hash, and per-SDK implementation hashes are also role-separated, so one
manifest hash cannot stand in for another. The SDK parsers enforce that same
role separation before app prover callbacks can observe descriptor hashes.
Bundle `audit_hashes` are treated as a separate evidence role: they must be
canonical, unique, and must not reuse the proof-artifact hash, proving-key hash,
verifier-key hash, destination-binding hash, or any per-SDK implementation
hash. Replayed audit hashes are rejected in the SDK parser path, not only by
release tooling. JS/browser, Swift, Kotlin/JVM, Java Android, and .NET verified
artifact descriptors must also bind the bundle verifier-key hash, a non-empty
SDK id, the manifest's expected implementation label, and the SHA-256 hash of
that SDK's implementation bytes before app prover callbacks can run. When a
native prover bundle is applied to an Ethereum mainnet outbound proof request,
the SDKs also require the bundle verifier-key hash to match the request's
destination binding verifier-key hash, so a manifest tied to another verifier
key cannot ride on a matching destination-binding hash. The strict
release-bundle verifier also
inventories those
readiness guard definitions themselves, so dropping the source-scan tests,
empty-payload tests, native hash-role tests, SDK parser closed-schema helpers,
SDK parser canonical-domain helpers, SDK parser canonical-hash helpers, SDK
parser role-separation helpers, canonical-hash tests, audit-hash
role-separation tests, or the common remote-prover/prover-endpoint spelling
checks blocks a published BSC release bundle.
BSC mainnet inbound proving now follows the same receipt-observed source-event
binding model as Ethereum: JavaScript, Python, Swift, Kotlin/JVM, Java Android,
and .NET derive `sourceEventDigest` from the BSC receipt log emitted by the
configured source bridge, compare it to `receiptProof.sourceEventDigest`, and
reject full receipt-proof evidence before local prover callbacks if the source
event was not validated. This keeps prebuilt BSC `receiptProof` material from
bypassing SDK-side source-admission checks while preserving hash-only
`receiptProofHash` collection as diagnostic evidence only.
The public release-bundle verifier
owns the same phase command and success-marker inventory instead of trusting the
report generator for those transcript requirements; parity tests keep the
report and verifier inventories aligned, while a weakened report module cannot
relax copied bundle-log checks. The verifier applies those traced command-line
and package-root export transcript checks to copied corridor logs before
accepting a published attachment bundle.
Swift, Kotlin, and Java Android request builders also bind the mobile witness
view to the opened full-bank `AccountsLtHash`: if a caller supplies an explicit
`witness.accountsLtHash`, it must match the opened contribution hash, and absent
witness values are filled from the opened witness before the canonical
OpenVerify/FastPQ request is hashed.
JavaScript and Python derive opened AccountsLtHash contribution and residual
hashes from the same normalized full-bank fields used by the source-state and
audit request hashes, while still rejecting duplicate camelCase/snake_case
aliases. A portal or backend can therefore use either supported field spelling
without letting UI-visible raw alias shape diverge from the hashed request.
Their Solana full-light audit builders also require the completed nested
AccountsLtHash proof capsule itself and recompute `accountsLtHashProofHash`
from it; a standalone hash is not enough to construct a second-stage UI proof
request. The JavaScript TypeScript declarations model the same production
contract: `SolanaSccpFullLightClientAuditProofRequestInput` requires exactly
one nested proof capsule alias, either `accountsLtHashProof` or
`accounts_lt_hash_proof`, while `accountsLtHashProofHash` remains only an
optional consistency echo.
The same JavaScript, Python, Swift, Kotlin, and Java
Android TON request builders reject duplicate audit verifier hashes, audit
hashes that reuse existing source-adapter material, and audit hashes that reuse
built-in TON template component hashes before deriving user-side proof requests
or full-light-client gate hashes. They also reject role verifier hashes replayed
from nonzero request-bound material, including source-state proof hashes,
shard-state public-input hashes, deployment/material/gate hashes, role columns,
and audit-statement hashes, before invoking a web or mobile proof engine. The
dynamic JavaScript and Python capsule
canonicalizers are now chain-specific too: Solana canonicalization accepts only the
AccountsLtHash `stark-fri-v1` circuit, while TON source-state canonicalization
accepts the shard-state and full-light audit role `stark-fri-v1` circuits; the
TON shard-state proof hash helper remains shard-state-only. Both reject empty,
all-zero, or over-2 MiB proof bytes, plus proof-family or circuit-id labels over
128 UTF-8 bytes, before a UI can hash the completed capsule.
JavaScript and Python Solana source-state capsule normalization also rejects a
supplied `proofBase64` / `proof_base64` field unless it exactly matches
`proofBytes` / `proof_bytes`, so a web portal or mobile-facing backend cannot
display or forward stale proof text while hashing different proof bytes. The
size cap is checked before base64 comparison so oversized UI proof output fails
without first encoding the payload for display checks, and the JavaScript/Python
app-linked Solana source-state prover result paths apply the same ordering to
structured callback results. Swift, Kotlin, and Java Android source-state
capsule types do not accept caller-supplied base64 aliases;
they derive `proofBase64` from defensive copies of the stored proof bytes, with
regressions pinning that mutating a returned byte view cannot change the
capsule or its displayed base64. That parity covers the Solana AccountsLtHash
and full-light audit capsules as well as the TON shard-state and full-light
audit capsules used by mobile proof UIs. Solana source-state wrappers in all
five SDKs now also recompute the request's statement hash from `statementBytes`
and require FastPQ `dsid`/`txSetHash` to derive from that hash, closing stale
request-metadata drift before a UI-generated proof can be wrapped. The
JavaScript FastPQ
source-state and full-light audit request builders for Solana, TON, and
Substrate-family runtime-storage proofs also freeze their returned request
objects, public-input columns, transition metadata, and aggregate request maps,
while exposing statement, context, schema, commitment, and witness bytes through
defensive-copy getters. Mutating a byte view visible to a browser portal
therefore cannot change the transcript already derived for the linked local
prover. The TypeScript declarations mark those FastPQ request objects, nested
arrays, transition entries, and aggregate request maps as readonly so portal code
sees the same contract at compile time. Python portal-backend builders use the
same read-only dict/list-compatible envelope shape and immutable byte values for
those Solana, TON, and Substrate-family FastPQ requests, so backend callbacks
cannot rewrite request metadata after transcript derivation either. The
JavaScript and Python Solana/TON source-state prover facades also snapshot
caller-supplied `proveRequest(...)` inputs into frozen callback requests with
defensive-copy byte getters before invoking the app-linked prover and before
wrapping returned proof bytes, so a browser or portal-backend callback cannot
mutate a manually supplied FastPQ request into a different on-chain capsule.
Kotlin/JVM and Java Android prover facades now mirror that handoff for mobile
apps across TON, Solana, EVM-family, TRON, and Substrate-family proof flows:
linked source-state and production proof engines see fresh request snapshots,
while returned proof bytes are wrapped against the original canonical request.
Swift, Kotlin/JVM, and Java Android TON source-state facades also run the
canonical shard-state or full-light audit OpenVerify/FastPQ request preflight
before invoking the app-linked mobile prover, matching the JavaScript/Python
portal path and preventing malformed direct request objects from reaching the
user-facing proof engine.
JavaScript and Python source-state prover result objects may include request
metadata for UI bookkeeping, but `version`, `proofFamily`/`proof_family`,
`circuitId`/`circuit_id`, and `proofBase64`/`proof_base64` must match the active
request and returned proof bytes before the SDK wraps the capsule. Solana and
TON full-light audit requests expose canonical snake_case role ids
(`tower_replay`, `full_accountsdb_lattice`, `bank_fork_choice`,
`masterchain_config`, `validator_set_transition`, and
`shard_accounts_dictionary`) to linked browser and mobile prover callbacks while
retaining language-native aggregate result properties for SDK callers. JavaScript
and Python Solana full-light audit request builders also preflight audit-role
hash separation before deriving the gate/request transcripts, rejecting
duplicate audit verifier hashes or hashes reused from governed source-adapter
material before a browser portal or portal backend calls the app-linked prover.
Those same JS/Python SDKs, plus Swift, Kotlin, and Java Android, now wrap
completed Solana and TON OpenVerify/FastPQ proof bytes into checked
source-state proof capsules pinned to the originating AccountsLtHash,
shard-state, or full-light audit role request's proof family and circuit id, so
browser, portal backend, and mobile app code does not hand-assemble downstream
source proof capsules after the user-side prover returns bytes. Solana's
canonical source-state capsule byte helpers accept the AccountsLtHash circuit
and the three full-light audit circuits, while the AccountsLtHash proof-hash
helper remains restricted to the nested AccountsLtHash circuit. The SDKs now
also expose source-state prover facades (`SolanaSccpSourceStateProver`,
`TonSccpSourceStateProver`, and mobile `SourceStateProver` variants) that
build the nested AccountsLtHash or TON shard-state request, invoke the
app-linked prover, wrap the returned proof bytes, then build and prove the
three full-light audit role requests for that source chain. The
dynamic JavaScript and Python wrappers require the full SDK-built
OpenVerify/FastPQ request shape, including the source domain, statement
bytes, verification context, schema descriptor, public-input columns, and
FastPQ payloads, so portal backends cannot wrap source-state proof bytes around
a minimal or hand-written circuit-id object. Swift, Kotlin, and Java Android
apply the same guard to their typed AccountsLtHash, TON shard-state, and
full-light audit request overloads: manually constructed request values must
still carry non-empty SDK-built statement/context/schema bytes, public-input
columns, FastPQ public inputs, and transitions before proof bytes can be wrapped
for downstream on-chain submission. For Solana, those wrappers also rederive
the canonical FastPQ transition bindings for the AccountsLtHash request and
each full-light audit role, then reject proof bytes if any transition key,
operation, old value, or new value no longer matches the SDK-built request
transcript. For
TON, those wrappers also rederive the
canonical FastPQ transition bindings for the shard-state request and each
full-light audit role, then require every transition key, operation, old value,
and new value to match before accepting externally generated proof bytes.
Kotlin mobile request models now mirror the Java Android defensive-copy contract for
Solana AccountsLtHash, Solana/TON full-light audit, and Substrate-family
runtime-storage request bytes and transition values, so Android proof apps
cannot mutate request byte arrays after construction. The Java Android
Substrate-family runtime-storage mirror exposes statement, context, and schema
bytes through copy-returning accessors and freezes the public-input and
transition lists. Swift regression tests pin the same mobile value-snapshot
contract for Solana, TON, and Substrate-family proof requests. Rust
source-adapter preflight
applies the same fail-closed
posture to non-empty source-state proof capsules by requiring version `1`, proof
family `stark-fri-v1`, a non-empty circuit id, and non-empty/non-all-zero proof
bytes before Norito/OpenVerify decoding. The Solana full-light-client audit proof
builders also re-check the nested AccountsLtHash OpenVerify/FastPQ proof before
deriving Tower replay, full AccountsDB lattice, or bank/fork-choice audit role
proofs, so a shaped but invalid nested capsule cannot be cascaded into
second-stage proof material.
The same SDK source-material helpers reject every deterministic Solana template
component hash: source trust anchor, consensus verifier, message-inclusion
verifier, AccountsDB source-state verifier, and finality policy. A browser,
Python portal backend, or mobile app must therefore supply governed live
component hashes before deriving a source verifier material hash or
full-light-client gate commitment.
Solana submission constructors now re-derive the source-adapter deployment
binding hash from the wrapped proof result's embedded binding and require the
source-proof public inputs to echo the same deployment hash, receipt hash, and
binding hash. This keeps web and mobile wallets from submitting a proof envelope
whose top-level binding hash, nested binding record, and public inputs describe
different source-adapter deployments.
Raw proof bytes cannot bypass those request/envelope/deployment checks because
the submission constructors require the wrapped result before producing the
canonical `borsh_instruction_v1` envelope.
They also require both the request and witness `mainnetGenesisHash` to equal
Solana mainnet-beta's canonical genesis hash, so a devnet or localnet witness
cannot be wrapped under the production Solana lane.
Production wrappers also require the full 2,048-byte nonzero AccountsLtHash
witness before invoking linked provers or wrapping externally generated proof
bytes, so browser and mobile flows cannot package a bank-state proof request
that only carries the checksum.
The diagnostic zero-binding request builders remain available for fixtures, but
web portals and mobile apps cannot accidentally produce an on-chain Solana proof
package from that diagnostic material.
The JavaScript Solana prover path also freezes request, result, and submission
objects and exposes proof/instruction byte fields through defensive-copy
getters, so browser callers cannot mutate the request hashes or wrapped
program-instruction bytes after the local prover has been invoked. Its
callback-visible witness snapshot now also deep-freezes nested UI payload
metadata and copies nested byte buffers before the app-linked prover is called,
so a browser prover cannot mutate portal-owned payload state while generating
the proof. The
TypeScript declarations mark those Solana SDK objects as readonly to surface the
same contract at compile time.
The Kotlin mobile Solana prover facade likewise passes a byte-array snapshot of
the canonical request into the app-linked proof engine and wraps returned proof
bytes against the original request, so mutable callback arrays cannot corrupt
the request hash used for the submitted proof envelope.
JavaScript, Python, Swift, Kotlin, and Java Android also expose the canonical
`fastpq-lane-balanced` OpenVerify source-adapter verifier-key commitment
helper for ETH, BSC, Solana, TON, TRON, and Substrate-family source lanes. Portal
and mobile tooling can derive the same `adapter_verifier_vk_hash` that
governance evidence renderers and Rust admission require, instead of copying
lane commitments by hand.
The same SDKs now expose canonical `SccpSourceVerifierMaterialV1` and
`SccpSourceAdapterEngineDeploymentV1` byte/hash helpers, so a web portal or
mobile app can audit the governed source material and deployment record hashes
before it invokes the user-side prover and submits the resulting proof on-chain.
Those helpers also reject reused non-zero role hashes across source trust
anchors, consensus verifiers, message-inclusion verifiers, finality policies,
source-state verifiers, source bridge code/network/config fields, adapter VKs,
and deployment receipts, matching the Rust admission and evidence preflight
rules.
The offline evidence renderers apply the same production checks on their direct
helper APIs as on their CLI paths: governed source component hashes, source
bridge addresses/code hashes, adapter verifier-key hashes, and deployment
receipt hashes must be non-zero before a record hash is derived. Solana and TON
full-light-client audit bundles are all-or-nothing when they are bound into a
source-adapter deployment record. Solana audit verifier hashes must also be
role-separated: the Tower replay, full AccountsDB lattice, and bank/fork-choice
hashes are pairwise distinct and cannot reuse existing source-adapter material,
the adapter verifier-key hash, or the deployment receipt hash. Rust deployment
admission applies the same role-separation rule to the TON masterchain config,
validator-set transition, and shard-accounts dictionary audit verifier hashes.
For TON, those source-material helpers now reject the same template-derived
source trust-anchor, consensus-verifier, message-inclusion, source-state
verifier, and finality-policy hashes that Rust admission and the offline
source-state evidence renderer reject.
For Solana, they also reject the Rust template AccountsDB source-state verifier
hash, so app-side audits cannot accidentally promote the profile template as
deployed verifier material.
For native destination lanes, the same JavaScript, Python, Swift, Kotlin, and
Java Android SDKs expose canonical destination binding key/hash helpers for
SORA -> Solana, SORA -> TON, and SORA -> SORA Kusama/SORA Polkadot/SORA2
Substrate-family runtimes. Portal and mobile proof generators can derive the
same `SccpDestinationBindingV1` hash checked by rollout evidence tooling instead
of copying destination binding constants by hand.

For EVM-family and TRON Groth16 flows, the SDK request builders derive the same
nine BN254 public signal words consumed by the destination verifiers. The
request hash commits to those signals plus the canonical public inputs, bundle
bytes, source proof bytes, statement hash, and destination binding hash; the
proof result then wraps externally generated proof bytes with an envelope hash
bound to that request. The Rust proof-result wrapper applies the same
transparent-public-input preflight when wrapping external EVM-family/TRON
Groth16 bytes, so a manually rebuilt request hash cannot carry an unsupported
public-input version or zero message, payload, commitment, finality-height, or
finality-block fields into a wrapped result. JavaScript, Python, Swift, Kotlin,
and Java Android portal/mobile helpers also derive the canonical EVM/TRON
deployment binding key, whose network-id segment is raw lowercase bytes32 hex
without `0x` to match Rust rollout metadata. JavaScript and Python portal
helpers now also derive the
EVM-family and TRON destination binding hash from the governed deployment tuple
before request hashing: network id, source/target domain, verifier backend,
proof family, verifier address, deployed verifier code hash, verifier key hash,
and, for EVM-family lanes, the bridge-wrapper address are all included. If a
portal supplies both `destinationBinding` material and a raw
`destinationBindingHash`, the request builder rejects mismatches before invoking
the user-linked prover. The JavaScript `EthereumMainnetSccp` facade is exported
from the package root and validates `eth_chainId` as a canonical JSON-RPC
quantity, so padded values such as `0x01` cannot alias the Ethereum mainnet
launch lane. The JavaScript `BscMainnetSccp` facade is also exported from the
package root, validates canonical `eth_chainId == 0x38`, and exposes the same
easy inbound receipt-collection and outbound calldata paths for BSC mainnet.
Both browser receipt collectors reject failed receipts, non-canonical
transaction/block hashes, missing or zero `receipt.blockNumber`, missing or
zero `block.number`, receipt transaction-hash drift, block hash/number drift,
and block objects without a canonical `receiptsRoot` before calling the
app-linked local prover; the BSC collector preserves Parlia finality evidence
from the app-linked consensus provider. The BSC browser submit helper also
rejects empty, all-zero, or over-2 MiB inbound proof bytes and copies accepted
proof bytes before invoking the app-linked Iroha submitter, and the browser
prove helper applies the same native-recursive proof-byte corridor before
returning local prover output to callers.
The Python, Swift, Kotlin/JVM, Java Android,
and .NET `EthereumMainnetSccp` facades expose the same easy inbound method shape
(`collect_inbound_evidence_from_receipt`/`collectInboundEvidenceFromReceipt`/
`CollectInboundEvidenceFromReceiptAsync`,
`prove_inbound_to_sora`/`proveInboundToSora`/`ProveInboundToSoraAsync`, and
`submit_inbound_to_iroha`/`submitInboundToIroha`/`SubmitInboundToIrohaAsync`)
for local SDK users: execution data must come from an app-supplied Ethereum
JSON-RPC provider that validates canonical `eth_chainId == 0x1`, and collected
receipts/blocks are checked for failed status, transaction-hash drift, block
hash/number drift, missing or zero receipt/block numbers, non-canonical hashes,
and missing receipt roots before native proof code is invoked. The Ethereum
mainnet browser, Python, Swift, Kotlin/JVM, Java Android, and .NET facades also
reject inbound prover output and submit payloads that are empty, all-zero, or
larger than the 2 MiB native-recursive payload cap before app submitter
callbacks can see them. Swift, Kotlin/JVM,
Java Android, and .NET callers
can also link an app-supplied Ethereum consensus/finality provider so
`collectInboundEvidenceFromReceipt` attaches beacon finality evidence from the
same local collection path when the caller did not pre-supply it. Before those
app-owned consensus-provider callbacks run, browser and native collectors pass a
detached snapshot of the validated receipt/block evidence and return detached
collected evidence, so mutable RPC response containers and byte buffers cannot be
changed after SDK validation by callback code or caller-owned references. Browser
and native Ethereum collectors now bind any supplied or collected beacon finality
evidence back to the execution block by requiring the finality execution block
number, execution block hash, and execution receipts root to match the validated
receipt/block before local source-prover callbacks run. The
JavaScript `proveInboundToSora` easy path now runs the same collection step
before invoking the app-linked prover, and precomputed `receiptProofHash`
values are accepted as already-collected receipt proof material. Across
JavaScript, Swift, Kotlin/JVM, Java Android, and .NET, Ethereum inbound proving
also requires beacon finality to be present before the app-linked source prover
is called; Swift, Kotlin/JVM, and Java Android now also accept per-call
execution and consensus providers on `proveInboundToSora`, matching the
JavaScript and .NET prove-time collection path without requiring apps to build a
new facade for each provider pair. The JavaScript package declarations expose
`EthereumMainnetBeaconFinalityEvidenceInput`,
`EthereumMainnetBeaconFinalityEvidence`, and
`EthereumMainnetConsensusProviderInput`, so browser applications see the
required execution block number, execution block hash, and execution receipts
root fields at compile time. Swift, Kotlin/JVM, Java Android, and .NET also
expose typed native beacon-finality helpers
(`EthereumMainnetBeaconFinalityEvidence` and Java Android's nested
`EthereumMainnetSccp.BeaconFinalityEvidence`) that build the canonical
map/dictionary shape for those three required execution fields plus optional
provider metadata. Swift, Kotlin/JVM, Java Android, and .NET also provide typed
inbound-evidence construction helpers for feeding that finality object into
ETH -> SORA source proving without hand-copying the finality map. Native .NET
callers also get the same Ethereum-mainnet route guard for chain id `1`,
ETH -> SORA inbound routing, SORA -> ETH outbound routing, canonical
SORA -> ETH destination-binding derivation, and the exact canonical bytes32
network-id string; uppercase or padded network ids are rejected. The .NET
Ethereum facade now also exposes `BuildOutboundProofRequest`,
`ProveOutboundToEthereumAsync`, `BuildEthereumCalldata`, and
`SubmitOutboundToEthereumAsync` so C# apps can keep SORA -> Ethereum proof
generation and submission local to native code without a WASM or remote prover
dependency. Swift, Kotlin/JVM, and Java Android now also provide
`submitOutboundToEthereum` methods backed by app-owned outbound submitter
callbacks, so their Ethereum mainnet easy paths build and validate verifier
calldata before handing it to the wallet/RPC integration supplied by the app.
Those Ethereum mainnet easy proof paths now require verified native prover
artifact descriptors before local proof execution: JS/browser, Swift,
Kotlin/JVM, and Java Android reject missing or mismatched artifact descriptors
after Ethereum request construction and before invoking the app-owned prover
callback, while .NET/C# exposes an artifact-bound
`ProveOutboundToEthereumAsync` overload that applies the verified bundle
before calling the native prover interface. A descriptor is not considered
verified unless it also carries the verifier-key hash, the SDK implementation
row, and a matching implementation-byte hash from the signed native prover
bundle. The same verified descriptor gate also runs when Ethereum mainnet
calldata is built or submitted: JS/browser, Swift, Kotlin/JVM, Java Android,
and C# refuse the easy product path unless the wrapped proof result is bound to
matching native prover artifacts, while the older generic EVM helpers remain
available only for callers that choose those explicit APIs.
The signed native prover bundle manifest parsers in JS/browser, Swift,
Kotlin/JVM, Java Android, and C# also reject duplicate JSON object keys before
building descriptor objects, including escaped-key aliases, so app-side bundle
loading cannot depend on last-key-wins parsing.
When an Ethereum execution provider is configured on those facades, the
outbound submitter path checks `eth_chainId == 1` before invoking the
app-owned submit callback, so a configured BSC or non-mainnet provider cannot
be silently ignored during SORA -> Ethereum submission.
The Python Ethereum mainnet facade mirrors that final step with
`submit_outbound_to_ethereum`, which passes the validated calldata package to an
app-owned transaction hook.
The BSC mainnet facades now expose the same app-owned outbound submit step on
JavaScript, Python, Swift, Kotlin/JVM, Java Android, and .NET: callers build the
governed SORA -> BSC verifier calldata through `buildBscCalldata` or
`build_bsc_calldata`, then hand it to `submitOutboundToBsc` or
`submit_outbound_to_bsc` only after the wrapped proof result has passed the BSC
destination-binding checks.
Python, Swift, Kotlin, Java Android, and JavaScript mobile/web
proof-request and submission constructors now apply the same source-domain,
target-domain, backend, `stark-fri-v1` proof-family, and Ethereum-mainnet
destination-binding checks when the app passes a derived binding object instead
of a raw hash; their Ethereum calldata helpers require wrapped proof results
that carry the chain-id-1 binding before calldata is built. The JavaScript,
Python, Swift, Kotlin, Java Android, and Rust BSC mainnet facades now pin the
EVM chain id to `56`, require the deployment-bound SORA -> BSC destination
binding before request, prebuilt-result wrapping, proof-job, or submission
packaging, and require wrapped proof results to carry the same binding before
calldata is built.
The Python package exposes the same easy `BscMainnetSccp` facade shape as the
native SDKs, with static BSC chain-id and destination-binding guards,
`build_outbound_proof_request`, `prove_outbound_to_bsc`, `build_bsc_calldata`,
`submit_outbound_to_bsc`, `collect_inbound_evidence_from_receipt`,
`prove_inbound_to_sora`, and `submit_inbound_to_iroha`; the older
`BscMainnetSccpProver` name remains as a compatibility wrapper. Python,
Swift, Kotlin/JVM, Java Android, and .NET now also expose native
`BscMainnetSccp` receipt facades mirroring the Ethereum mobile path: they
validate canonical BSC `eth_chainId` values, collect receipts and receipt
blocks through app-supplied JSON-RPC providers, bind supplied or app-collected
Parlia finality evidence to the execution block number, block hash, and
receipts root, reject failed receipts, missing or zero receipt/block numbers,
and drifted transaction/block/finality evidence, and
submit only non-empty non-zero copied source proofs to the app-linked
submitter.
The JavaScript browser, Python, Swift, Kotlin/JVM, Java Android, and .NET BSC
easy inbound proving paths now also require Parlia finality before the
app-linked source prover callback runs; each SDK can collect that evidence
through an app-supplied consensus provider or validate caller-supplied finality
against the collected execution receipt block. JavaScript, Python, Swift,
Kotlin/JVM, Java Android, and .NET BSC proving additionally require full
`receiptProof`/`receipt_proof` material before calling the app-linked native
prover; hash-only `receiptProofHash` evidence remains usable for collection
diagnostics, but cannot drive proof generation. The JavaScript package
declarations expose the BSC Parlia finality
evidence and consensus-provider input shapes so browser applications see the
required execution block number, execution block hash, and receipts-root fields
at compile time.
Native .NET callers additionally get BSC-mainnet route guards for chain id
`56`, BSC -> SORA inbound routing, SORA -> BSC outbound routing, the exact
canonical bytes32 network-id string, native BSC destination-binding/hash
construction using the same Keccak preimage as the JavaScript, Python, Swift,
Kotlin, Java Android, and Rust helpers, and app-owned outbound
proof-request/prove/calldata/submit hooks for SORA -> BSC. Release-readiness
user-prover rows now
name the Ethereum/BSC facade methods, Ethereum beacon-finality helpers and
consensus-provider hooks, native BSC Parlia consensus-provider hooks, typed
native BSC Parlia finality helper records/builders, and the .NET app-owned
execution/prover/submitter interfaces alongside the other SDK helper symbols,
so the `dotnet-sdk` corridor phase cannot stand in for an undocumented native
C# surface. The Java Android
Ethereum and BSC mainnet facades also snapshot `bundleBytes` and
`sourceProofBytes` before witness-provider callbacks, so app-owned request
bytes cannot be mutated while source evidence is being resolved. Rust also
requires BSC -> SORA source proofs to bind governed source-adapter deployment
evidence. The EVM-family and TRON result wrappers
reject empty, all-zero, or non-384-byte Groth16 proof bytes across JavaScript,
Python, Swift, Kotlin, Java Android, and .NET, and the .NET Ethereum mainnet
outbound wrapper also validates the BN254 proof tuple plus message-id,
commitment-root, and source-domain binding before verifier calldata is emitted,
so placeholder or non-canonical ABI proof output cannot be packaged by SDK
callers before Torii's deployment-bound preflight.
JavaScript and Python EVM-family, TRON, and Substrate-family local prover
callbacks may also return `proofBase64` / `proof_base64` for UI bookkeeping,
but the SDK rejects that metadata unless it exactly matches the returned proof
bytes before wrapping the proof result for submission.
The same JavaScript, Python, Swift, Kotlin, and Java Android SDK surfaces now
package wrapped EVM-family and TRON Groth16 proof results into
`submitSccpMessageProof(bytes,bytes32[6],bytes32)` contract-call calldata,
rederiving the six transparent ABI public-input words and rechecking proof
context, proof bytes, `proofBase64`, statement hashes, destination-binding
hashes, public signal words, non-zero request hashes, and envelope hashes
recomputed from the request hash plus proof bytes before wallet or relayer
submission.
JavaScript also exports the low-level ABI public-input word and
`submitSccpMessageProof(...)` calldata helpers from the package entrypoint, so
web portals can use the same checked encoder directly when they do not need the
higher-level submission envelope. The JavaScript, Python, Swift, Kotlin, and
Java Android direct calldata encoders now apply the same SORA source-domain
proof-tuple check before they emit wallet calldata, so callers that bypass the
higher-level submission wrapper still reject a Groth16 tuple whose embedded
source-domain word is not SORA. JavaScript and Python EVM-family/TRON
submission builders reject standalone `bundleBytes` or `sourceProofBytes`
without a wrapped `proofResult`, because raw contract-call payloads cannot bind
those bytes to the original request hash. JavaScript and Python EVM/TRON and
Substrate submission builders also reject explicit null proof-result values,
matching the presence-aware Solana and TON wrappers.
Substrate-family submission builders in JavaScript, Python, Swift, Kotlin/JVM,
and Java Android also reject non-empty standalone `sourceProofBytes`; those
bytes are request-bound prover input and are not encoded into the final
runtime-call argument list.
JavaScript web portals, Python portal backends, and Swift/Kotlin/Java Android
mobile SDKs also parse the 12-word Groth16 ABI tuple before wrapping or
submitting EVM-family/TRON proof bytes: the tuple version must be 1, the
embedded message id and commitment root must be non-zero and match the
transparent public inputs, the embedded source domain must fit `u32` and match
the request source domain for wrapped/submitted packages, and each BN254 proof
coordinate must be a non-zero point limb below the BN254 base-field modulus.
JavaScript and Python portal helpers also reject in-field G1/G2 coordinates
that are not on the BN254 curves before wallet calldata is emitted, and the
JavaScript, Python, Swift, Kotlin, and Java Android proof wrappers require the
G2 point to pass the BN254 prime-order subgroup check before UI-generated proof
bytes can be wrapped or submitted. Rust/Torii submission packaging, the Rust
typed Torii client, and the bridge-feature CLI apply the same point and
subgroup preflight before building contract-call payloads or forwarding
deployment-bound proof material, matching the pairing-validation subgroup
guard.
The Rust, JavaScript, and Python Torii clients now apply the same BN254 tuple
preflight to raw `proofBytesHex` / `proof_bytes_hex` query and submit fields
before artifact, proof-job, bridge-proof, or bridge-message HTTP requests are
sent; the bridge-feature CLI inherits that query validation. Swift, Kotlin, and
Java Android raw bridge-submit clients apply the same tuple preflight before
posting deployment-bound bridge DTOs. Lower-level relay code therefore cannot
forward arbitrary 384-byte blobs even when it bypasses the higher-level prover
wrappers. When a local `message_bundle` carries
`commitment.message_id` and `commitment_root`, those clients still bind the
tuple's version, SORA source-domain word, message id, and commitment root to
the bundle before posting.
JavaScript and Python prover facades
also isolate the request object handed to app-linked prover callbacks and reject
callback results that carry a mismatched backend, request hash, envelope hash,
EVM-family/TRON transparent public inputs, EVM-family/TRON proof context,
EVM-family/TRON public signal words, Substrate-family transparent public inputs,
Substrate-family proof context, Solana proof-context hash, or TON/Solana
source-adapter deployment-binding hash, so stale or mutated UI prover results
cannot be silently rewrapped for a different on-chain submission. For
EVM-family/TRON/Substrate-family callback results, optional backend,
request/envelope hash, public-input, proof-context,
statement/destination-binding hash, and EVM-family/TRON public-signal fields
are strict when present; a `null`/`None` field is rejected instead of being
treated as omitted. JavaScript and Python EVM-family, TRON, and Substrate-family
prover surfaces now also rebuild the canonical production request before
deriving envelope hashes or invoking app-linked callbacks, so web portals and
portal backends cannot wrap proof bytes around mutated request hashes, public
signals, proof contexts, lane backends, or target domains.
JavaScript and
Python TON wallet/liteserver
submission builders and the Swift, Kotlin, and Java Android TON message-body
constructors can also consume a wrapped TON proof result directly and recheck
its proof bytes, transparent public inputs, request hash, deployment-binding
hash, envelope hash, statement hash, destination binding, and proof context
before constructing the BOC payload. Python prover requests, callback inputs,
proof-result envelopes, and Solana submission packages are returned as
dict/list-compatible read-only envelopes, so portal backends keep normal
mapping/list inspection while request hashes and nested context fields cannot be edited
after derivation. Swift, Kotlin, and Java Android proof-result
wrappers now recompute the canonical request before deriving the envelope hash.
Java Android returns defensive copies for EVM-family, Solana, TON, and TRON
proof/submission bytes, and Kotlin EVM-family, Solana, TON, TRON, and
Substrate-family request/result/submission objects now return fresh copies for
request byte fields and proof bytes, so mobile callers cannot wrap proof bytes
around a manually constructed or mutated request context. TON/Substrate
proof-result wrappers reject all-zero external proof bytes before deriving the
request-bound envelope, and TON message-body builders apply the same proof-byte
preflight before packaging BOC submissions. The EVM-family, TON, and TRON
request preimages length-prefix both `bundleBytes` and `sourceProofBytes`, so
the same raw byte sequence cannot be replayed under a different
bundle/source-proof split, and those boundaries are enforced across web,
Python, and mobile callers. Wrapped EVM-family, TRON, and Substrate-family
proof results also carry the original request bundle and source-proof bytes;
EVM-family/TRON proof-result submission builders rebuild the canonical request
hash from those bytes before emitting verifier-contract calldata, rejecting a
stale or manually swapped bundle even when proof bytes and envelope hashes are
otherwise self-consistent. The SDK request builders preserve omitted
source-proof bytes for diagnostic request hashes, while JavaScript, Python,
Swift, Kotlin, and Java Android production wrappers preserve omitted source
proof bytes and still reject non-empty all-zero source proof placeholders before
app-linked prover output can be treated as submit-ready. The JavaScript
TypeScript declarations and Python
package export list now expose those request-byte fields and proof-result
wrapper helpers, so portal and mobile prover integrations see the same
request-binding surface that runtime code enforces.
TRON destination proof-request builders are additionally locked to the deployed
TVM wrapper's production lane: `sourceDomain = SORA` and
`publicInputs.targetDomain = TRON`.
EVM-family destination proof-request builders are likewise locked to the
governed EVM destination lanes: `sourceDomain = SORA` and
`publicInputs.targetDomain` equal to either ETH or BSC. Web, Python, Swift,
Kotlin, and Java Android helpers reject non-SORA source domains and non-EVM
targets before deriving request hashes or invoking app-linked provers.
Rust EVM-family and TRON Groth16 contract submission builders apply the same
destination-side rule before emitting package payloads: the transparent
`target_domain` must equal the manifest counterparty domain, so local-domain
SORA public inputs cannot be packaged for counterparty contract calls even
though generic transparent proofs may name either manifest endpoint. Their
submission templates use the canonical `submitSccpMessageProof(bytes,bytes32[6],bytes32)`
signature, pinning emitted EVM/TVM calldata to selector `0xbd57826c`.
The Solidity contract smoke now also exercises the positive BN254 path with a
deterministic self-consistent test proof, so the EVM wrapper and TRON wrapper
both cover pairing acceptance plus replay rejection instead of only malformed
proof failures. The same smoke now rejects zero proof points, off-curve G2
points, and on-curve G2 points outside the BN254 prime-order subgroup before the
accepted-proof path. Accepted-proof negatives also mutate payload,
finality-height, and finality-block public signals after proof generation. The
TRON wrapper smoke also covers its public-input preflight guards, cleartext
source-domain overflow rejection, wrong-statement rejection, and rejection of a
valid proof replayed against a different TRON deployment binding. The smoke
script pins
its temporary `solc`, `ganache`, and `ethers` dependencies so this verifier
check is reproducible across operator machines without checked-in Node
artifacts.

For Substrate-family destination flows, the JavaScript, Python, Swift, Kotlin,
and Java Android SDK request builders use the fixed `substrate-runtime-v1`
backend and only accept
SORA-Kusama, SORA-Polkadot, or SORA2 as the transparent-public-input target
domain. They also require `sourceDomain = SORA` before deriving request hashes
or invoking app-linked runtime provers. The request hash commits to the source
domain, canonical public inputs, length-prefixed SCCP bundle bytes,
length-prefixed source proof bytes,
statement hash, and destination binding hash. Returned runtime proof bytes are
wrapped in an envelope hash bound to that request, and explicit prover callback
metadata for the backend, request hash, or envelope hash is rejected when it
does not match on the web/Python async callback path. JavaScript, Python, Swift,
Kotlin, and Java Android proof-result wrappers rederive the canonical request
before hashing the proof envelope so UI and mobile apps cannot wrap runtime
proof bytes around a mutated request context.
These SDK builders also fail closed before invoking the
user-supplied prover when the backend is not the canonical lane backend
(`evm-groth16-bn254-v1` for EVM-family flows or
`tron-groth16-bn254-v1` for TRON), the statement or destination-binding hash is
zero, a required public input is zero, the Groth16 target domain is zero, or the
source and target domains are identical. TRON proof-request builders
additionally require the target domain to be TRON, so portal and mobile code
cannot package a TRON/TVM Groth16 request for a different destination lane.
EVM-family, TON, and TRON request builders also reject empty SCCP bundle bytes before
deriving the request hash, matching Solana's bundle preflight and preventing
deployment-agnostic local prover requests. TON proof-request builders also
require the mainnet shard-state light-client verifier id and a non-zero
source-state verifier hash, and they reject the TON template-derived
source-state verifier hash, so UI/mobile provers cannot emit proof requests
bound to diagnostic zero verifier material or profile-template material. Their
source-adapter deployment binding is fixed to TON -> SORA, even when the
transparent public inputs target the TON destination verifier, so a UI cannot
hash proof bytes against an ungoverned TON -> TON source-adapter deployment. The
Substrate-family runtime-storage source-state request builders apply the same
deployment-material preflight: they require the domain-specific runtime-storage
source-state verifier id, a non-zero verifier hash, and reject the Rust
template-derived verifier hash before invoking the app-linked prover.
Those request builders now derive the `sccp-substrate-runtime-storage-v1`
OpenVerify/FastPQ request directly from UI-collected `System.Events` storage
proof witness material: canonical statement bytes, verification context, schema
descriptor, public-input columns, FastPQ public inputs, and deterministic
metadata transitions. They also verify an optional caller-supplied
`storage_proof_hash`/`storageProofHash` against the canonical transcript hash,
so web portals and mobile apps cannot submit a runtime-storage source-state
proof request for a different storage proof than the one checked by Rust
admission. Web and Python request builders canonicalize the storage proof's
source domain before merging flat or nested source verifier material, while
still rejecting nested material whose source-domain aliases are duplicated or
do not match the proven Substrate-family lane.
JavaScript SDK declarations expose those backend ids, plus the TON
`ton-contract-v1` backend, as literal types so TypeScript portal code gets the
same contract before runtime. Kotlin and Java Android also revalidate those
backend ids in their public proof-result wrappers, so callers cannot bypass the
request builders by manually constructing a request with a debug backend.
The same web, Python, Swift, Kotlin, and Java Android source-proof helpers
derive ETH Deneb/Fulu `ExecutionPayloadHeader`, beacon-body execution-payload
branch, and `BeaconBlockHeader` SSZ roots from UI witness material so browser
and mobile provers submit the roots checked by the source adapter.
Their TRON transcript helpers also reject all-zero `0x41`-prefixed witness
addresses before hashing raw headers, solid-block header proofs, or witness
schedule payloads, matching the Rust source adapter's fail-closed preflight.
Core SCCP payload validation applies the same non-zero TRON address rule to
base58check account codec values, so `0x41` plus twenty zero bytes cannot be
accepted as a TRON sender or recipient.
The Solana helpers also derive the mainnet-beta epoch for a slot and the
`sccp:solana:epoch-stake-root:v1` active-stake root from the UI/mobile
validator roster witness, matching the finality-context check enforced by the
source adapter. They also derive the `sccp:solana:stake-activation:v1`
active-window transcript from the epoch, active vote roster, activation epochs,
and deactivation epochs so the signed finality context commits to the stake
schedule submitted on-chain. The companion
`sccp:solana:stake-account-state:v1` transcript binds those active stake entries
to vote account, stake account, vote account state hash, and stake account state
hash openings collected by UI/RPC code. They also derive the
`sccp:solana:bank-fork:v1` transcript from the finalized slot, direct parent
slot, bank signature count, parent bank hash, finalized bank hash, blockhash,
transaction-status root, SCCP account-inclusion root, Solana AccountsLtHash
checksum, and optional Agave hard-fork hash data so UI/mobile provers submit
the same bank-fork context bound into finalized-slot votes. The same helper
surface now derives
`sccp:solana:accounts-lt-proof-public-inputs:v1`, a canonical recursive-proof
public-input transcript that binds the Solana source domain, backend id,
mainnet genesis hash, epoch, finalized/direct-parent slots, bank signature
count, parent/finalized bank hashes, blockhash, transaction-status root,
account-inclusion root, AccountsLtHash checksum, optional hard-fork hash data,
and the derived bank-fork hash. When UI/mobile tooling supplies the full
2,048-byte AccountsLtHash to the public-input transcript helpers, Rust and the
JavaScript, Python, Swift, Kotlin, and Java Android SDKs replay both the BLAKE3
AccountsLtHash checksum and Agave bank hash before returning transcript bytes,
so direct helper calls cannot package checksum-only bank-state material. Those
SDKs also expose Agave-compatible account AccountsLtHash contribution helpers
for UI/RPC-collected account openings and raw account data. JavaScript and
Python now require the AccountLtHash `executable` flag to be an actual boolean
instead of coercing strings or numeric placeholders before deriving
Agave-compatible account rows; all SDKs expose the Agave bank-hash helper that
recomputes the SHA-256 `Bank::hash_internal_state` chain from parent bank hash,
bank signature count, blockhash, raw AccountsLtHash, and optional hard-fork hash
data; JavaScript and Python reject duplicate bank-state aliases before that hash
is derived. The Python and Swift SDKs now include the same pure BLAKE3 checksum and
XOF path for AccountsLtHash values, so Python portal tooling and iOS
proof-request builders do not need optional native BLAKE3/Norito bindings to
validate source-state request checksums or derive opened-account contributions.
The verifier requires every opened Solana vote
account, stake account, and StakeHistory sysvar account address in that witness
set to be globally unique, so a proof cannot present one AccountsDB address as
multiple opened roles. It also recomputes the deterministic directionless
Merkle tree for exactly that opened account set and requires every supplied
branch to match the canonical branch for the opened leaves, so a subset witness
cannot use a root that also commits to extra unopened leaves. Account-inclusion
root helpers reject zero leaf hashes and cap sibling branches at 64 nodes, the
same bound enforced by Rust admission. JavaScript,
Python, Swift, Kotlin, and Java Android now expose an opened-account inclusion
witness helper that derives this exact root plus vote-account, stake-account,
and StakeHistory sysvar branch splits from the UI/mobile account openings and
raw account data, rejecting any caller-supplied account-inclusion root that does
not match. JavaScript and Python account-opening and account-inclusion leaf
helpers also reject duplicate UI/RPC aliases for the account address, owner
program id, rent epoch, account-data hash, finalized slot, opening object, raw
account data, raw-data hash, and nested opening address. If a UI supplies both
raw account bytes and a raw-data hash, the helpers recompute the raw-data hash
and reject mismatches before deriving the account-inclusion leaf. Vote-account
and stake-account opened vectors are also capped at
8,192 entries per role before account-inclusion or AccountsLtHash proof
material is derived, matching the Rust source-adapter validator bound. The same
JavaScript and Python opened witness helpers reject duplicate vote/stake opening
array aliases, vote/stake raw-data array aliases, StakeHistory sysvar
opening/raw-data aliases, account-inclusion root aliases, AccountsLtHash
checksum/root aliases, and full AccountsLtHash aliases before deriving residual
or branch material. Solana message-proof helpers derive the transaction-status Merkle
leaf from the source-event digest, decoded 64-byte transaction signature, and
decoded 32-byte emitter program id, then recompute the transaction-status root
from the UI-supplied branch with the SCCP source-node Blake2b prefix
`sccp:source:node:v1`. They also reject zero source-event digests, zero
transaction-status roots, all-zero decoded transaction signatures, all-zero
decoded emitter program ids, root/branch mismatches, and empty
transaction-status inclusion branches before deriving the UI-submitted request
hash. JavaScript, Python, Swift, Kotlin, and Java Android cap those branches at
64 siblings, matching the source-envelope admission gate and avoiding any
SSZ/SHA-256 branch-fold ambiguity. The
same Solana helper surface binds
`source_state_verifier_id` and `source_state_verifier_hash` into the canonical
local proof request and public inputs, defaulting the verifier id to
`sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1` and rejecting
non-zero verifier hashes under any other id. The same Solana helper surface
now builds the nested `sccp-solana-accounts-lt-hash-v1` source-state proof
request for UI-linked provers, exposing the canonical statement bytes, opened
account commitment, verification-context bytes, OpenVerify schema descriptor,
public-input columns, and FastPQ transition payloads used by the Rust capsule
verifier. The same Solana helper surface
derives `sccp:solana:tower-replay:v1` from the rooted slot, direct parent slot,
finalized slot, and explicit 31-vote active post-root Tower stack transcript
so UI and mobile provers bind the replayed lockout stack submitted on-chain;
the rooted slot supplies the 32nd confirmation.
The JavaScript, Python, Swift, Kotlin, and Java Android source-proof helpers
also expose typed EVM-family and TRON receipt-root MPT value envelopes
`[ "sccp:evm:receipt-root-value:v1", receipt_root ]` and
`[ "sccp:tron:receipt-root-value:v1", receipt_root ]`, so relayers can generate
the verifier-accepted structural receipt-root values without duplicating RLP
encoding. They also expose the TRON
`submitSccpSourceEvent(uint32,uint32,bytes32)` calldata helper and
`sccp:tron:transaction-source-proof:v1` hash helper so relayers can bind
java-tron transaction Merkle witnesses, source-domain word, and target-domain
word before invoking the source prover. The transaction-source hash helpers
recompute the java-tron Merkle root from `transaction_bytes`,
`transaction_index`, `transaction_count`, and `transaction_merkle_branch` before
they hash the transcript, so a caller cannot bind arbitrary transaction roots to
otherwise well-formed transaction bytes. The TRON calldata helper is locked to
the production TRON -> SORA source lane and rejects a zero source-event digest
before local prover transcript derivation, matching the owner-gated TVM source
bridge and Rust source-call verifier preflight. Rust also exposes
source-bridge-bound transaction-source bytes/hash helpers that keep the
canonical transcript unchanged while rejecting a mismatched governed bridge or
owner before returning it; material-bound variants accept
`SccpSourceVerifierMaterialV1` directly and fail unless it is production-ready
TRON source material. The same SDK surfaces expose
BSC ValidatorSet storage-value,
metadata-proof, and transition-message transcript helpers so web portals,
operator tooling, and mobile apps bind the user-collected ValidatorSet account
and storage witnesses to the exact hashes verified on-chain. For deployed
ETH/BSC source material, the same receipt-trie proof
must instead open an actual successful legacy or typed EVM receipt whose log
contains the canonical SCCP source event ABI:
`topic0 = keccak256("SccpSourceEvent(bytes32)")`, `topic1 =
source_event_digest`, and empty event data. The verifier checks receipt shape,
success status, minimally encoded cumulative gas, the 256-byte logs bloom, and
well-formed log entries with 20-byte emitters and 32-byte topics. Valid
unrelated `LOG0` entries may appear in the receipt, but any log with more than
four topics is rejected and the SCCP source event must still be the exact
two-topic ABI log. For non-placeholder ETH/BSC material, the log emitter must
match the governed `source_bridge_emitter_address` carried by the production
source-verifier material and source-adapter deployment evidence.

## Acceptance rules

- Ranges must be ordered/non-empty and respect `zk.bridge_proof_max_range_len` (0 disables the cap).
- Optional height windows reject stale/future proofs: `zk.bridge_proof_max_past_age_blocks` and `zk.bridge_proof_max_future_drift_blocks` are measured against the block height that ingests the proof (0 disables the guardrails).
- Bridge proofs may not overlap an existing proof for the same backend (pinned proofs are preserved and block overlaps).
- Manifest hashes must be non-zero; payloads are size-capped by `zk.max_proof_size_bytes`.
- ICS payloads honour the configured Merkle depth cap and verify the path using the declared hash function.
- Transparent payloads must declare a non-empty backend label.
- Transparent payloads under the SCCP `sccp/stark-fri-v1/*` family must now decode as a typed SCCP message proof artifact, not an opaque byte blob.
- Typed SCCP message artifacts now validate `proof_bytes` as a real
  Norito-encoded `OpenVerifyEnvelope` whose inner payload is the canonical
  SCCP FASTPQ proof and public-input column wrapper.
- The outer `OpenVerifyEnvelope`, nested STARK proof wrapper, and raw FastPQ
  backend proof bytes are re-encoded and byte-compared before verification, so
  compressed or otherwise alternate Norito framings are rejected even if they
  decode to the same fields.
- Source-adapter OpenVerify proof envelopes are capped at 2 MiB before decode or
  FastPQ replay, matching the bound used for source-state proof capsules.
- SCCP transparent-proof verification reconstructs the canonical SCCP statement
  batch from the embedded bundle plus the shared manifest table, checks the
  `OpenVerifyEnvelope` metadata (`circuit_id`, schema descriptor, verifier
  commitment, and wrapped public inputs), and then replays
  `fastpq_prover::verify(...)` against that batch.
- Legacy 32-byte placeholder digests are no longer accepted.
- Raw SCCP message bundle bytes are not accepted as transparent bridge-proof
  payloads; Torii/core validation requires the typed artifact and replays the
  embedded cryptographic proof.
- Pinned proofs are exempt from retention pruning; unpinned proofs still respect the global `zk.proof_history_cap`/grace/batch settings.

## SCCP message proof formats

`NexusSccpMessageProofV1.finality_proof` is direction-sensitive:

- SORA-origin messages (`SORA -> remote`) carry a Norito-encoded
  `NexusBridgeFinalityProofV1`. The verifier checks the Nexus commit QC,
  commitment root, target-domain commitment, payload hash, message id, and
  Merkle root.
  The commit QC projection includes the full Sumeragi vote-signing material
  (`subject_block_hash`, `parent_state_root`, `post_state_root`,
  `chain_order_hash`, `rechain_seq`, and optional `highest_qc`) plus the
  validator set, validator PoPs, signer bitmap, and BLS aggregate signature.
  `verify_nexus_bridge_finality_proof_cryptographic(...)` verifies the
  BLS-normal aggregate signature over the canonical Sumeragi vote preimage,
  validates every validator PoP, and enforces the same two-thirds-plus-one
  signer quorum used by core finality checks. Builds without the SCCP `bls`
  feature fail closed for this cryptographic helper.
- non-SORA source messages (`ETH/BSC/Solana/TON/TRON/Substrate-family ->
  SORA/Nexus or another supported target`) carry a Norito-encoded
  `SccpSourceChainProofEnvelopeV1`. Raw Nexus finality bytes are rejected for
  these messages, and source-chain envelopes are rejected for SORA-origin
  messages.

`SccpSourceChainProofEnvelopeV1` has this canonical data shape:

- `version = 1`
- `source_domain` and `target_domain` as SCCP numeric domain ids
- `source_chain` as the canonical chain key (`eth`, `bsc`, `sol`, `ton`,
  `tron`, `sora2`, `sora-kusama`, or `sora-polkadot`)
- `source_proof_plan`:
  `EthereumBeaconReceiptProof`, `BscValidatorSetReceiptProof`,
  `SolanaFinalizedTransactionProof`, `TonMasterchainShardProof`,
  `TronDposReceiptProof`, or `SubstrateGrandpaEventProof`
- `finality_model` matching the source domain
- `message_id`, `payload_hash`, and `commitment_root` from the SCCP hub
  commitment
- `source_event_digest =
  blake2b256("sccp:source:event:v1" || 1 || source_domain ||
  target_domain || message_id || payload_hash)`, with integer fields encoded in
  little-endian order
- `finality_height`, `finality_block_hash`, `finalized_header_hash`, and
  `receipt_or_message_root`
- `consensus_proof`, a Norito-encoded `SccpSourceConsensusProofV1`
- `message_inclusion_proof`, a Norito-encoded
  `SccpSourceMessageInclusionProofV1`
- non-empty `inclusion_branch` entries, each exactly 32 bytes

`SccpSourceConsensusProofV1` binds the source domain, chain key, proof plan,
finality model, finality height, finality block hash, receipt/message root,
finalized header hash, a plan-specific `SccpSourceAdapterProofV1`,
`adapter_transcript_hash`, `SccpSourceVerifierEvidenceV1`, and
`SccpSourceAdapterVerificationProofV1`, which wraps a STARK/FastPQ
`OpenVerifyEnvelope` for the canonical source-adapter statement. The finalized
header hash is recomputed as
`blake2b256("sccp:source:header:v1" || 1 || source_domain || finality_model ||
finality_height || finality_block_hash || receipt_or_message_root)`, with
integer fields encoded in little-endian order.

`SccpSourceVerifierEvidenceV1` is the explicit verifier/trust-anchor binding
for the adapter transcript. It carries:

- `version = 1`
- `source_domain`, `source_chain`, `source_proof_plan`, and `finality_model`
- `adapter_proof_hash =
  blake2b256("sccp:source-adapter-proof:v1" || canonical_adapter_proof)`
- `adapter_transcript_hash`
- `adapter_circuit_id = "sccp-source-adapter-v1"`
- `source_trust_anchor_id` and `source_trust_anchor_hash`
- `consensus_verifier_id` and `consensus_verifier_hash`
- `message_inclusion_verifier_id` and `message_inclusion_verifier_hash`
- `source_bridge_emitter_id`, `source_bridge_emitter_address`, and
  `source_bridge_emitter_code_hash`
- `source_bridge_network_id`, `source_bridge_owner_address`, and
  `source_bridge_config_hash`
- `finality_policy_id` and `finality_policy_hash`
- `source_state_verifier_id` and `source_state_verifier_hash`
- `source_adapter_deployment_hash` and
  `source_adapter_deployment_receipt_hash`

The evidence record is canonicalized and hashed as
`blake2b256("sccp:source-verifier-evidence:v1" ||
canonical_verifier_evidence)`. The structural verifier recomputes the expected
evidence for the source domain, chain key, proof plan, finality model,
adapter proof hash, adapter transcript, and adapter circuit id. Any zero hash,
empty id, domain replay, circuit replay, stale adapter hash, stale transcript,
or mismatched finality policy is rejected.
Material-only diagnostic evidence sets both deployment fields to zero.
Deployment-aware production evidence must set both fields to non-zero values and
must match the configured `SccpSourceAdapterEngineDeploymentV1` hash plus its
`deployment_receipt_hash`. The source adapter OpenVerify statement includes the
hash of this full evidence record, so a source proof generated without the
deployment hash and receipt cannot satisfy a configured production source
adapter deployment.

### Source-chain trust boundary

The SDK receipt, block, and finality objects are witness inputs, not trust
anchors. SDK facades reject malformed or inconsistent evidence before invoking
an app-owned prover, but a JSON-RPC endpoint, `eth_chainId`, receipt metadata,
or app-supplied finality map is never enough to prove that evidence came from
the real source chain. The production trust boundary starts at the configured
source verifier material and source-adapter deployment admitted by Iroha
governance.

For every non-SORA source, `SubmitBridgeProof` must see a source-chain proof
envelope whose verifier evidence recomputes to the configured
`SccpSourceAdapterEngineDeploymentV1` hash. That deployment record binds the
source domain and chain key, source proof plan, finality model, source trust
anchor id/hash, consensus verifier id/hash, message-inclusion verifier id/hash,
finality-policy id/hash, source bridge emitter id/address/code hash/network id,
bridge owner/config hash, adapter verifier key hash, and mined deployment
receipt hash. If any of those governed values drift, admission rejects the
bundle before treating it as a production source proof.

This is the fake-chain rejection rule. A fake BSC-like or ETH-like chain may
claim the same EVM chain id, expose a bridge contract at the same address, and
return self-consistent receipts and block roots through RPC. That data is still
only a witness. To pass admission, the proof must verify under the configured
source-adapter verifier key and must connect its consensus/finality path to the
configured `source_trust_anchor_hash`, while the receipt proof must open the
governed source bridge emitter and SCCP source-event digest under that
consensus-approved receipt root. Chain-id checks and SDK preflight prevent
obvious lane mixups; the configured trust anchor, verifier hashes, and
deployment hash are the security root.

The IDs and hashes used by the evidence record come from
`SccpSourceVerifierMaterialV1`. Today the built-in material catalog is
explicitly marked `placeholder_material = true`; those records are accepted
only for diagnostic source-proof artifacts and cannot make a lane
production-ready. A source material record must be non-placeholder, match the
source domain/chain/proof plan/finality model/circuit id, carry non-empty ids
plus non-zero hashes for the source trust anchor, consensus verifier,
message-inclusion verifier, and finality policy, carry any source-state
verifier required by the source profile, and satisfy the exact domain-profile
gate. Reusing any built-in placeholder id or digest keeps the
record non-production even if `placeholder_material` is set to `false`. The
Rust deployment-material constructors reject all-zero, template-derived, or
reused role hashes before returning EVM-family, Solana, TON, TRON, or
Substrate-family source material, so callers cannot accidentally pass
profile-template verifier commitments or aliased verifier roles into later
deployment steps. This keeps the source-adapter statement format stable while
leaving the production gate closed until real light-client anchors and immutable
verifier code hashes are installed.
ETH and BSC configured material can satisfy the source-material gate only when
it matches the canonical mainnet EVM-family profiles and carries
deployment-supplied component hashes. The
`sccp_evm_family_mainnet_source_verifier_material_v1(...)` helper exposes the
ETH/BSC profile templates, while
`sccp_evm_family_mainnet_source_verifier_material_with_hashes_v1(...)` installs
the operator-provided trust-anchor, consensus-verifier,
receipt-inclusion-verifier, and finality-policy hashes. Production ETH/BSC
material must then be completed with
`sccp_evm_family_mainnet_source_verifier_material_with_hashes_and_emitter_v1(...)`,
which adds the governed `source_bridge_emitter_id` plus the non-zero 20-byte
source bridge emitter address and non-zero runtime code hash expected in
receipt logs. The templates bind the source domain/chain,
`EthereumBeaconReceiptProof` or
`BscValidatorSetReceiptProof` plan, the corresponding finality model,
`sccp-source-adapter-v1` circuit, `evm-groth16-bn254-v1` backend, and the
canonical `sccp:evm:receipt-proof:v1` or `sccp:bsc:receipt-proof:v1`
inclusion-witness layout. The ETH template also binds
`sccp:eth:sync-committee:v1`,
`sccp:eth:sync-committee-payload:v1`,
`sccp:eth:sync-committee-message:v1`, and
`sccp:eth:sync-committee-aggregate:v1`,
`sccp:eth:sync-committee-transition-message:v1`, and
`sccp:eth:sync-committee-transition-signature:v1`; the BSC template also
binds `sccp:bsc:validator-set:v1`,
`sccp:bsc:validator-set-payload:v1`, `sccp:bsc:commit-message:v1`, and
`sccp:bsc:commit-seal:v1`,
`sccp:bsc:validator-set-transition-message:v1`, and
`sccp:bsc:validator-set-transition-seal:v1`,
`sccp:bsc:validator-set-metadata:v1`, and
`sccp:bsc:validator-set-storage-value:v1`. Generic ready-looking ETH/BSC
ids/hashes and the template-derived component hashes still fail closed.
The ETH adapter proof now also carries a `SccpEthBeaconSyncCommitteeProofV1`
certificate. The verifier derives the sync-committee hash from ordered
BLS-normal public keys, proof-of-possession values, and non-zero weights;
requires that hash to match both the adapter `sync_committee_root` and the
configured source trust anchor for non-placeholder material; recomputes the
sync-committee message hash from beacon/execution/receipt-proof material;
checks the aggregate-signature hash; verifies BLS proof-of-possession values
and the aggregate sync-committee signature; and enforces strict `> 2/3` signed
committee weight. The ETH adapter now also carries raw execution-header RLP and
the beacon header witness (`beacon_proposer_index`, `beacon_parent_root`,
`beacon_state_root`, `beacon_body_root`, and `execution_payload_branch`). The
verifier Keccak-hashes that RLP to the claimed execution block hash, parses the
header fields, and requires the RLP block-number and receipts-root fields to
match the SCCP finality height and adapter `execution_receipts_root`. It then
derives the Deneb/Fulu SSZ `ExecutionPayloadHeader` root from the RLP header
fields, opens the fixed execution-payload body field index through the
4-sibling `execution_payload_branch`, requires the result to match
`beacon_body_root`, and recomputes the SSZ `BeaconBlockHeader` root from
`beacon_slot`, proposer, parent, state, and body roots before accepting
`beacon_finalized_root`. The adapter also carries `receipt_root_index` plus bounded
`receipt_trie_proof_nodes`; the verifier derives the RLP transaction-index key,
opens the receipt trie under the execution-header receipts root, and requires
non-placeholder material to prove an actual successful EVM receipt whose log
contains the SCCP source-event topic
`keccak256("SccpSourceEvent(bytes32)")`, the SCCP `source_event_digest` as the
second topic, empty event data, and a log emitter equal to the governed source
bridge emitter address before checking the receipt-proof transcript hash. The
receipt decoder accepts legacy receipts and EIP-2718 typed receipts
with type bytes `0x01..=0x7f`; it rejects failed receipts, malformed or
non-minimal cumulative gas, non-256-byte blooms, malformed logs, logs with more
than four topics, non-32-byte topics, digest-only topic matches, non-empty SCCP
source event data, and typed-prefix byte `0x00`. Unrelated valid `LOG0`
entries are allowed; they simply cannot satisfy the two-topic SCCP source-event
ABI. More than one matching SCCP source-event log in the same receipt is
rejected as ambiguous duplicate evidence. Placeholder structural fixtures may
still use the typed EVM-family
receipt-root envelope carrying the SCCP receipt/message root. This closes the
first ETH consensus-signature,
execution-header, beacon execution-payload inclusion, receipt-trie, and
receipt-log ABI/emitter binding slices, while full production still needs
recursive verifier deployment and any production light-client update/state
branches not discharged inside that deployed source-adapter circuit. If the
configured source trust anchor is a parent sync committee rather than the active
committee, the adapter can now carry a `SccpEthSyncCommitteeTransitionProofV1`
chain. Each transition carries the canonical raw next sync-committee payload,
hashes it under `sccp:eth:sync-committee-payload:v1`, derives the next
committee hash from that payload, and binds the payload hash, parent committee
hash, next committee hash, next-committee branch hash, finalized beacon root,
transition slot, and sync-period range under a transition-message hash. The
parent committee must sign that message with strict `> 2/3` weight before the
next committee becomes eligible. Transition chains must advance one Ethereum
mainnet sync-committee period at a time; the verifier uses the consensus
mainnet presets `SLOTS_PER_EPOCH = 32` and
`EPOCHS_PER_SYNC_COMMITTEE_PERIOD = 256` as the fixed period geometry and
rejects skipped-period transition evidence.

Operators can now render the ETH -> SORA source material and source-adapter
deployment records from governed live evidence with
`scripts/sccp_eth_source_bridge_evidence.py`. The helper is intentionally
limited to `source_domain = 1` and `target_domain = 0`, rejects boolean or
non-`u32` programmatic domain values, zero hashes, and wrong-width hashes,
rejects the EVM-family template-derived Ethereum source trust-anchor,
consensus-verifier, message-inclusion, and finality-policy hashes, and emits
only the EVM-family Ethereum fields that Rust production admission expects. The
CLI evidence strings are exact: surrounding whitespace on the bridge address,
component hashes, runtime bytecode, source/target domains, or deployment block
number is rejected before source material or deployment records are rendered. The
direct material and deployment record hash helpers apply the same
live-component check, so programmatic rollout tooling cannot bypass the
renderer and derive governed ETH records from template source components.
`source_bridge_emitter_address` is the governed
20-byte Ethereum source bridge contract address and
`source_bridge_emitter_code_hash` is the Keccak-256 hash of its deployed
runtime bytecode. Hash-only input is a diagnostic JSON path; production TOML
requires `--source-bridge-runtime-bytecode-hex` or
`--source-bridge-runtime-bytecode-file` so the helper derives the hash from the
runtime bytecode preimage. Inline runtime bytecode must use a lowercase `0x`
prefix with lowercase hex; `0X` or uppercase bytecode text is rejected before
production TOML is rendered. TOML carries
`sccp_evm_source_bridge_runtime_bytecode_hex` so the all-lanes preflight can
replay the Keccak-256 hash instead of trusting the standalone code-hash
comment. `adapter_verifier_vk_hash` must still match the canonical
`fastpq-lane-balanced` OpenVerify verifier commitment for the ETH -> SORA lane;
the helper recomputes that value and rejects mismatches before rendering
governance TOML. Production TOML also requires mined deployment receipt
metadata: the deployment transaction hash, receipt contract address, receipt
block hash, an exact positive integer receipt block number, and the non-zero
receipt block `receiptsRoot`. Boolean or truthy placeholders for the block
number are rejected before receipt metadata can be marked ready.

```bash
python3 scripts/sccp_eth_source_bridge_evidence.py \
  --bridge-address <eth-source-bridge-address> \
  --source-bridge-runtime-bytecode-file <deployed-runtime-bytecode-hex-file> \
  --source-trust-anchor-hash <beacon-finalized-checkpoint-or-sync-committee-hash> \
  --consensus-verifier-hash <beacon-sync-committee-execution-verifier-hash> \
  --message-inclusion-verifier-hash <execution-receipt-trie-verifier-hash> \
  --finality-policy-hash <beacon-finality-policy-hash> \
  --adapter-verifier-vk-hash <openverify-vk-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --deployment-transaction-hash <source-adapter-deployment-tx-hash> \
  --deployment-receipt-contract-address <eth-source-bridge-address> \
  --deployment-receipt-block-hash <source-adapter-deployment-block-hash> \
  --deployment-receipt-block-number <source-adapter-deployment-block-number> \
  --deployment-receipt-block-receipts-root <source-adapter-deployment-receipts-root> \
  --toml
```

The BSC adapter proof now also carries a `SccpBscValidatorSetSealProofV1`
certificate. The verifier derives the validator-set hash from secp256k1
validator public keys, EVM addresses, and non-zero voting powers; requires that
hash to match both the adapter `validator_set_hash` and the configured source
trust anchor for non-placeholder material; recomputes the BSC commit-message
hash from epoch, block number/hash, receipts root, and validator-set hash;
verifies the commit-seal hash; recovers every 65-byte secp256k1 seal signature
to the expected validator address; rejects high-S malleable recoverable
signatures; and enforces a strict `> 2/3` signed-power threshold. If the
configured source trust anchor is a parent validator set rather than the active
set, the adapter can now carry a
`SccpBscValidatorSetTransitionProofV1` chain. Each transition binds the parent
set hash, next set hash, next-set payload hash, transition block, and epoch
range under a transition-message hash, then requires the parent validator set
to seal that message with strict `> 2/3` power before the final transition can
be used as the active block validator set. Multi-step transition chains must
also be ordered by strictly increasing transition block number, so an otherwise
valid chain cannot replay a later validator update before an earlier one. The
verifier also enforces the BSC mainnet Parlia epoch window: an active receipt
block must satisfy `validator_epoch = block_number / 200`, and each
validator-set transition must advance exactly one epoch on that epoch's start
block. The canonical transition-message helpers in Rust and the JavaScript,
Python, Swift, Kotlin, and Java Android SDKs enforce the same preflight before
hashing: `source_domain` must be BSC, `to_validator_epoch` must equal
`from_validator_epoch + 1`, and `transition_block_number` must equal
`to_validator_epoch * 200`. The
transition proof also carries the raw transition header RLP and canonical
next-validator-set payload. The verifier requires
`keccak256(transition_header_rlp)` to equal
`transition_block_hash`, parses the header as an Ethereum-compatible RLP
header, extracts the Parlia epoch-header validator list from field 12
`extraData`, and requires that extracted canonical address list, with unit
block-signing power, to match the carried next-validator-set payload. It then
hashes the payload under `sccp:bsc:validator-set-payload:v1`, decodes the
versioned address/power list, rejects duplicate or zero validators, and
requires the decoded payload to derive the advertised
`next_validator_set_hash`. The transition also carries a
`SccpBscValidatorSetMetadataProofV1` that opens the BSC ValidatorSet system
contract account (`0x0000000000000000000000000000000000001000`) under the
transition header `stateRoot`, verifies the account `storageRoot`, opens the
`currentValidatorSet.length` slot, and opens each carried validator's
`currentValidatorSet[index].consensusAddress` storage slot. The verifier
requires those proven storage addresses, in order, to match the Parlia
header-derived next-validator-set payload before the parent set signature can
activate the transition. BSC production admission requires the governed
source-adapter deployment record to bind the canonical recursive verifier key
and mined deployment receipt evidence.
The BSC adapter also carries `receipt_root_index` plus bounded
`receipt_trie_proof_nodes`; the verifier derives the RLP transaction-index key,
opens the receipt trie under the BSC header `receipts_root`, and requires
non-placeholder material to prove an actual successful EVM receipt whose log
contains the canonical SCCP source event ABI topic, the SCCP
`source_event_digest` as the second topic, empty event data, and a log emitter
equal to the governed source bridge emitter address before checking the
receipt-proof transcript hash. The same strict receipt decoder rejects
failed or malformed receipts, rejects logs with more than four topics, permits
unrelated valid `LOG0` entries, and accepts the typed EVM-family receipt-root
envelope only for placeholder structural fixtures. Under the active Ethereum
mainnet launch policy, the BSC lane remains fail-closed until a BSC launch
policy opens and configured governance records supply the source material,
source-adapter deployment evidence, destination rollout, and route allowlist.

For BSC specifically, the root of trust is not `eth_chainId == 56` and not a
provider's claim that a block is finalized. The configured BSC
`source_trust_anchor_hash` names the approved validator-set/checkpoint anchor,
and the `BscValidatorSetReceiptProof` must show either the active validator set
matches that anchor or a strictly ordered epoch-by-epoch
`SccpBscValidatorSetTransitionProofV1` chain derives the active set from it.
The active set must then seal the receipt block, or the final transition set
must seal the message that authorizes the next set, with strict `> 2/3` signed
power and canonical secp256k1 recovery. Only after that validator-set path is
accepted does the receipt-trie proof bind the SCCP source-event log to the
governed BSC source bridge emitter. A fake chain that merely returns
BSC-shaped headers, receipts, validator-set hashes, or commit-seal hashes
cannot satisfy this path unless it can produce the required source-adapter
proof against the governed deployment and trust anchor.

Operators can now render the BSC -> SORA source material and source-adapter
deployment records from governed live evidence with
`scripts/sccp_bsc_source_bridge_evidence.py`. The helper is intentionally
limited to `source_domain = 2` and `target_domain = 0`, rejects boolean or
non-`u32` programmatic domain values, zero hashes, and wrong-width hashes,
rejects the EVM-family template-derived BSC source trust-anchor,
consensus-verifier, message-inclusion, and finality-policy hashes, and emits
only the EVM-family BSC fields that Rust production admission expects. The
CLI evidence strings are exact: surrounding whitespace on the bridge address,
component hashes, runtime bytecode, source/target domains, or deployment block
number is rejected before source material or deployment records are rendered. The
direct material and deployment record hash helpers apply the same
live-component check, so programmatic rollout tooling cannot bypass the
renderer and derive governed BSC records from template source components.
`source_bridge_emitter_address` is the governed
20-byte BSC source bridge contract address and
`source_bridge_emitter_code_hash` is the Keccak-256 hash of its deployed
runtime bytecode. Hash-only input is a diagnostic JSON path; production TOML
requires `--source-bridge-runtime-bytecode-hex` or
`--source-bridge-runtime-bytecode-file` so the helper derives the hash from the
runtime bytecode preimage. Inline runtime bytecode must use a lowercase `0x`
prefix with lowercase hex; `0X` or uppercase bytecode text is rejected before
production TOML is rendered. TOML carries
`sccp_evm_source_bridge_runtime_bytecode_hex` so the all-lanes preflight can
replay the Keccak-256 hash instead of trusting the standalone code-hash
comment. `adapter_verifier_vk_hash` must still match the canonical
`fastpq-lane-balanced` OpenVerify verifier commitment for the BSC -> SORA lane;
the helper recomputes that value and rejects mismatches before rendering
governance TOML. Production TOML also requires mined deployment receipt
metadata: the deployment transaction hash, receipt contract address, receipt
block hash, an exact positive integer receipt block number, and the non-zero
receipt block `receiptsRoot`. Boolean or truthy placeholders for the block
number are rejected before receipt metadata can be marked ready.

```bash
python3 scripts/sccp_bsc_source_bridge_evidence.py \
  --bridge-address <bsc-source-bridge-address> \
  --source-bridge-runtime-bytecode-file <deployed-runtime-bytecode-hex-file> \
  --source-trust-anchor-hash <active-validator-set-hash> \
  --consensus-verifier-hash <validator-set-seal-verifier-hash> \
  --message-inclusion-verifier-hash <receipt-trie-verifier-hash> \
  --finality-policy-hash <validator-set-finality-policy-hash> \
  --adapter-verifier-vk-hash <openverify-vk-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --deployment-transaction-hash <source-adapter-deployment-tx-hash> \
  --deployment-receipt-contract-address <bsc-source-bridge-address> \
  --deployment-receipt-block-hash <source-adapter-deployment-block-hash> \
  --deployment-receipt-block-number <source-adapter-deployment-block-number> \
  --deployment-receipt-block-receipts-root <source-adapter-deployment-receipts-root> \
  --toml
```

For deployed ETH/BSC source emitters, operators can collect the emitter code
hash directly from read-only JSON-RPC with
`scripts/sccp_evm_source_live_evidence.py`. The helper verifies `eth_chainId`
against the requested source lane (`eth = 1`, `bsc = 56`), reads the governed
source bridge runtime bytecode with `eth_getCode`, derives the same
Keccak-256 runtime code hash used by source material, and can optionally read a
deployment transaction receipt to check `status = 0x1` and the deployed
contract address. When deployment receipt evidence is supplied, the helper also
fetches the receipt block by number, requires its canonical block number and
hash to match the receipt, requires the receipt block `receiptsRoot` to be a
non-zero bytes32 value, re-reads the source bridge bytecode at that block, and
requires the hash and bytecode to match the selected collection block tag.
Production TOML rendering requires that receipt-block hash/root/code-hash check,
an explicit `--expected-source-bridge-code-hash` pin, plus expected source
material and source-adapter deployment record hashes, so the observed live
bytecode cannot self-authorize governance evidence. Ethereum source TOML also
requires the collection block tag to be `finalized`; explicit `latest` or
`safe` Ethereum reads remain available for JSON inspection, but they cannot
render governed production TOML. BSC source TOML keeps its `latest` default.
Both the live collector and direct audited ETH/BSC source renderer emit
`sccp_evm_source_block_tag` metadata; the all-lanes preflight requires that
comment and rejects Ethereum source material unless it is `finalized`. The
public all-lanes lane summary also exposes this as
`evm_live_metadata.source_block_tag`, so release-bundle verification can reject
forged summaries that hide non-finalized Ethereum source reads. The
release-readiness report also mirrors this value in each cryptographic-evidence
row as `evm_source_block_tag`, making the finalized source pin visible in both
JSON and Markdown release artifacts.
The helper treats chain-id selectors, fixed-width component hashes, and
JSON-RPC returned quantities or hex byte strings as exact evidence; surrounding
whitespace fails before live source material or deployment receipt metadata is
rendered. It also caps successful JSON-RPC responses and HTTP error details
before decoding, rejects duplicate JSON object keys instead of accepting
last-value-wins parsing, and requires every success envelope to echo
`jsonrpc = "2.0"` with the request id `1` before using the result.
Operator-supplied source bridge code-hash, deployment transaction, verifier
component, deployment receipt, and expected record hash pins must also be
lowercase hex with a lowercase `0x` prefix when provided; `0X` and uppercase
aliases fail before live source TOML can be rendered.

```bash
python3 scripts/sccp_evm_source_live_evidence.py \
  --rpc-url <ethereum-or-bsc-rpc-url> \
  --domain eth \
  --bridge-address <eth-source-bridge-address> \
  --expected-rpc-chain-id 1 \
  --expected-source-bridge-code-hash <runtime-bytecode-hash> \
  --deployment-transaction-hash <source-adapter-deployment-tx-hash> \
  --source-trust-anchor-hash <source-trust-anchor-hash> \
  --consensus-verifier-hash <source-consensus-verifier-hash> \
  --message-inclusion-verifier-hash <source-message-inclusion-verifier-hash> \
  --finality-policy-hash <source-finality-policy-hash> \
  --adapter-verifier-vk-hash <openverify-vk-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --expected-source-verifier-material-hash <source-material-record-hash> \
  --expected-source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --toml
```

Apps and operators that need concrete receipt-inclusion material can use
`scripts/sccp_evm_receipt_proof_evidence.py` against their own read-only
Ethereum/BSC JSON-RPC endpoint. The helper enforces the selected mainnet chain
id, fetches the successful transaction receipt, fetches the containing block
and full block receipt list, reconstructs the EIP-2718-aware receipt trie from
typed receipt RLP, verifies the computed root against the block
`receiptsRoot`, and emits the receipt RLP, RLP transaction-index trie key,
proof nodes, and verified receipts root pair. By default, the helper requires
`--source-bridge-address` and exactly one matching canonical
`SccpSourceEvent(bytes32)` log in the receipt before
`source_event_digest` is rendered. `--allow-receipt-only-evidence` is available
only for generic receipt-trie diagnostics; do not use receipt-only output as
SCCP source proof material.

```bash
python3 scripts/sccp_evm_receipt_proof_evidence.py \
  --rpc-url <ethereum-or-bsc-rpc-url> \
  --domain eth \
  --expected-rpc-chain-id 1 \
  --transaction-hash <successful-source-bridge-tx-hash> \
  --source-bridge-address <eth-source-bridge-address>
```

The browser, Swift, Kotlin/JVM, Java Android, and .NET SDKs expose the same
receipt-side construction without WASM through `canonicalEvmReceiptRlp`,
`evmReceiptTrieKey`, and `buildEvmReceiptTrieProofFromReceipts` (or native
PascalCase equivalents). `EthereumMainnetSccp` uses those helpers to build
`receiptProof` locally when the app supplies an SCCP `inclusionBranch`; if
`blockReceipts` are absent, it fetches `eth_getBlockReceipts` through the
app-owned execution provider and verifies the computed root and target
transaction hash before invoking the local prover callback.

The generated source TOML carries metadata comments for the observed RPC chain
id, source bridge address, source bridge runtime code hash, and replayable
source bridge runtime bytecode, plus the verified deployment transaction,
receipt contract address, receipt block hash, and exact positive receipt block
number and block `receiptsRoot`. The live summary also carries
`deployment_receipt_block_code_hash_matches = true` and
`deployment_receipt_block_receipts_root_verified = true`; imported summaries
without those receipt-block rechecks do not render production TOML. The
all-lanes preflight requires those comments for ETH/BSC source material, decodes
the bytecode, recomputes Keccak-256, and checks that both the live metadata and
replayed hash match the governed
`source_bridge_emitter_address` and `source_bridge_emitter_code_hash`. Hash-only
offline source material is rejected for production launch even if the record
hashes are internally consistent.

Solana configured material can satisfy the source-material gate only when it
matches the canonical mainnet-beta profile and carries deployment-supplied
component hashes. The deployment-backed production gate additionally requires a
complete audited full-light-client verifier bundle before it can open. The
`sccp_solana_mainnet_source_verifier_material_v1()` helper exposes the template
profile, while
`sccp_solana_mainnet_source_verifier_material_with_hashes_v1(...)` installs the
operator-provided source trust-anchor, consensus-verifier,
message-inclusion-verifier, and finality-policy hashes, and
`sccp_solana_mainnet_source_verifier_material_with_hashes_and_accounts_db_v1(...)`
also installs the governed AccountsDB source-state verifier hash while rejecting
all-zero or template-derived AccountsDB verifier hashes. Operators can
render the matching material and source-adapter deployment records with:

```bash
python3 scripts/sccp_solana_source_state_evidence.py \
  --source-trust-anchor-hash <active-vote-roster-hash> \
  --consensus-verifier-hash <finalized-slot-verifier-hash> \
  --message-inclusion-verifier-hash <transaction-status-verifier-hash> \
  --source-state-verifier-hash <accountsdb-verifier-hash> \
  --finality-policy-hash <finalized-slot-policy-hash> \
  --adapter-verifier-vk-hash <openverify-vk-hash> \
  --tower-replay-verifier-hash <tower-replay-verifier-hash> \
  --full-accountsdb-lattice-verifier-hash <full-accountsdb-lattice-verifier-hash> \
  --bank-fork-choice-verifier-hash <bank-fork-choice-verifier-hash> \
  --expected-full-light-client-gate-hash <governed-solana-full-light-client-gate-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --toml
```

The renderer rejects wrong domains, boolean or non-`u32` programmatic domain
values, zero hashes, and template-derived Solana component hashes using the
same finalized-vote template transcript as `iroha_sccp`. The direct material
and deployment inputs are exact strings: surrounding whitespace on fixed-width
component hashes or source/target domains is rejected before source material,
deployment records, or gate hashes are rendered. The direct material
and deployment record hash helpers apply the
same deployed-component check, so programmatic evidence tooling cannot bypass
the renderer and derive governed Solana record hashes from profile-template
source components. `adapter_verifier_vk_hash` must match the canonical
`fastpq-lane-balanced` OpenVerify verifier commitment for the Solana -> SORA
lane; the renderer recomputes that commitment and rejects mismatches before
rendering source-adapter deployment TOML or compact JSON summaries. Solana
full-light-client audit inputs are all-or-nothing in both output modes: a
direct JSON summary cannot carry only one of the Tower replay, full AccountsDB
lattice, or bank/fork-choice verifier hashes. The template binds the
Solana domain/chain, `SolanaFinalizedTransactionProof` plan,
`SolanaFinalizedSlot` finality model, `sccp-source-adapter-v1` circuit,
`sccp-solana-recursive-mainnet-v1` backend, Solana mainnet-beta genesis hash
`5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp`, and the canonical
`sccp:solana:message-proof:v1` inclusion-witness layout plus the
`sccp:solana:transaction-status-leaf:v1` leaf layout. It also binds the generic
SCCP source-event Merkle prefixes `sccp:source:event-leaf:v1` and
`sccp:source:node:v1`, so Solana material cannot be reused with a different
source-event commitment tree. The profile also binds Solana mainnet-beta's
432,000-slot epoch length,
`sccp:solana:finality-context:v1`,
`sccp:solana:epoch-stake-root:v1`,
`sccp:solana:stake-activation:v1`,
`sccp:solana:stake-account-state:v1`,
`sccp:solana:account-opening:v1`,
`sccp:solana:account-raw-data:v1`,
`sccp:solana:account-inclusion-leaf:v1`,
`sccp:solana:account-inclusion-node:v1`,
`sccp:solana:vote-account-data:v1`,
`sccp:solana:stake-account-data:v1`,
`sccp:solana:stake-history-sysvar-data:v1`,
`sccp:solana:stake-history:v1`,
`sccp:solana:accounts-lt-proof-public-inputs:v1`,
`sccp:solana:accounts-lt-opened-contributions:v1`,
`sccp:solana:mainnet-genesis:v1`,
`sccp-solana-accounts-lt-hash-v1`, the `fastpq-lane-balanced` AccountsLtHash
parameter set, all AccountsLtHash FastPQ transition keys,
`sccp:solana:tower-lockout:v1`, `sccp:solana:tower-replay:v1`,
`sccp:solana:bank-fork:v1`, `sccp:solana:vote-roster:v1`, and
`sccp:solana:finalized-vote:v1`, plus the 32-slot Tower lockout confirmation
depth and 31-slot active post-root vote stack depth, so configured material
cannot replay a deployment that was committed only to the old inclusion
transcript. Generic ready-looking SOL ids/hashes and the template-derived
component hashes still fail closed. Compact JSON can be used without the audit
bundle for diagnostic hash discovery, but production TOML requires the Tower
replay, full AccountsDB lattice, and bank/fork-choice verifier hashes plus an
independent expected gate pin. With all three audit hashes supplied, the renderer
emits a deterministic
`sccp:solana:full-light-client-gate:v1` hash over those verifier commitments
plus the canonical source-material and source-adapter deployment record hashes.
Those three verifier hashes are also appended to the canonical
`SccpSourceAdapterEngineDeploymentV1` hash for Solana deployments when present,
so proof-level `source_adapter_engine_deployment_hash` values bind the same
full-light-client audit bundle. The derived gate hash stays outside the
deployment record to avoid a circular deployment-hash dependency.
Supplying only part of that full-light-client evidence fails locally. Reusing
one audit verifier hash for another audit role, or reusing an existing source
material, adapter verifier-key, or deployment receipt hash as an audit verifier
hash, also fails locally and in Rust admission before the Solana production
gate can open. Compact JSON summaries may compute the audit gate hash for
operator comparison, but production TOML rendering now rejects gate-closed
records and requires `--expected-full-light-client-gate-hash` to match the
complete audit bundle. JSON summaries expose
`source_verifier_material_ready`, `source_adapter_engine_deployment_ready`,
`source_adapter_gate_ready_with_full_light_client_evidence`,
`source_adapter_gate_blockers`, and `full_toml_ready` alongside the existing
full-light audit fields, so rollout automation can distinguish
material/deployment pins from the complete source TOML gate. The
`source_adapter_gate_closed_until_full_light_client` field remains the
fail-closed policy label; the ready bit and blockers report whether the
independently pinned full-light-client evidence satisfies that gate. This keeps
governed Solana audit TOML from being staged from a self-derived gate commitment
or without deployed full-light-client evidence. The audit
hash and the three component hashes are emitted into the
`zk.sccp_source_adapter_engine_deployments` record when supplied, are committed
by the node's ZK consensus policy hash, and are recomputed during configured
source-adapter parsing. Runtime admission consumes the audited deployment
predicate before opening the Solana source-adapter gate, while proof
verification still checks the concrete Tower replay, full-bank AccountsDB, and
bank/fork-choice evidence carried by the submitted source proof. The Rust gate
helper also requires the canonical production AccountsDB source-state verifier
id/hash before deriving the gate hash, and deployment-aware proof verification
rejects a proof if its embedded verifier evidence is replayed under a different
audited Solana deployment hash or spliced after OpenVerify binding. Rust exposes
the same `sccp_solana_full_light_client_gate_hash_v1(...)` transcript helper
and keeps a
golden vector in the SCCP test suite so operator evidence tooling cannot drift
from the admission crate. The JavaScript, Python, Swift, Kotlin, and Java
Android proof-helper SDKs mirror the same deployment-record audit suffix and
expose the same full-light-client gate hash; they reject partial,
non-Solana, zero-hash, duplicate, or source-material-reused audit bundles before
portal/mobile proof tooling submits the deployment and gate hashes on-chain.
All Solana full-light audit role schemas expose `mainnet_genesis_hash` after
`source_domain`, then the common `epoch`, `rooted_slot`, `parent_slot`,
`vote_message_hash`, and `accounts_lt_hash_proof_hash` public-input columns
before role-specific inputs, so portal/mobile provers and on-chain verification
can inspect the chain identity, finality window, voted message commitment, and
nested AccountsLtHash proof commitment directly. The bank/fork-choice audit
role also names
`account_inclusion_root`, `bank_signature_count`,
`bank_hash_hard_fork_data_hash`, and `tower_replay_hash` in its OpenVerify
schema and exposes those fields as public-input columns alongside the
transaction-status root, AccountsLtHash checksum, and bank-fork hash. The
hard-fork-data column is the BLAKE2b-256 hash of
`sccp:solana:bank-hash-hard-fork-data:v1 || bank_hash_hard_fork_data`, so the
fork-choice capsule cannot hide the common finality/audit anchors, the
opened-account root, Agave signature count, optional hard-fork bytes, or Tower
replay root only inside the aggregate statement hash.
The Tower replay audit role also names `stake_account_state_hash`,
`stake_history_sysvar_account_hash`, and `account_inclusion_root` in its
OpenVerify schema and exposes them as public-input columns alongside
`tower_lockout_hash`, `tower_replay_hash`, `bank_fork_hash`,
`epoch_stake_root`, `stake_activation_hash`, and `stake_history_hash`, so the
user-side Tower proof is directly bound to opened vote/stake account state, the
StakeHistory sysvar account, and the account-inclusion root instead of only the
aggregate finality-context hash.
The all-lanes preflight JSON now also reports each lane's
`source_adapter_gate` object with `required`, `ready`, `gate_hash`,
`audit_hashes`, and `blockers`; Solana and TON lanes expose the recomputed
full-light-client gate status directly, TRON exposes its DPoS/source-call gate
hash, Substrate-family lanes expose the derived runtime-storage source gate,
and lanes without a source gate report `required = false` and empty blockers.
The all-lanes preflight promotes required source-gate blockers into
the lane blockers. For Substrate-family lanes, it also requires the
source-adapter deployment evidence to carry the audited
`sccp_substrate_runtime_storage_gate_hash` comment, rejects a missing or zero
pin, recomputes the gate from the governed source material plus deployment
records, and rejects any mismatch before the lane can report ready. Public
release-bundle verification rejects required gates with empty, zero, or
unaudited `gate_hash` values, rejects missing or zero verifier/audit hashes
inside `audit_hashes`, rejects unexpected or missing lane-specific audit keys,
rejects non-required lanes that carry gate material,
rejects source-gate audit hashes that replay source material, source-adapter
deployment, destination binding, route allowlist, route canary evidence, or
sibling audit hash roles,
rejects any gate marked ready while blockers remain, and rejects required
source gates that are still blocked in public production release bundles.
The Solana adapter proof now also carries a
`SccpSolanaFinalityContextV1` plus a `SccpSolanaFinalizedVoteProofV1`
certificate. The verifier recomputes the domain-separated vote message hash
from the finalized slot, blockhash, bank hash, transaction-status root,
adapter-bound `message_proof_hash`, and finality-context hash; checks unique
non-zero 32-byte Ed25519 validator keys and non-zero stakes; caps the active
validator roster at 8,192 entries before hashing or signature checks; derives
signed stake from the signer bitmap; enforces a strict `> 2/3` stake threshold;
verifies all signatures deterministically; and requires the roster hash to
match the configured source trust-anchor hash for non-placeholder material. The
Solana message-proof transcript rejects zero source-event digests, zero
transaction-status roots, all-zero transaction signatures, all-zero emitter
program ids, and empty transaction-status inclusion branches before hashing the
transaction signature, emitter program id, and inclusion branch. The
source adapter also derives the transaction-status leaf from the source-event
digest, transaction signature, and emitter program id, then reconstructs the
transaction-status root from that leaf and branch with the same
`sccp:source:node:v1` Blake2b node prefix exposed by the web and mobile SDKs.
The claimed
transaction-status/message root must match, so a tampered branch cannot be
hidden by recomputing only `message_proof_hash`. The
finality context is shape-checked and binds the epoch, rooted slot, parent
slot/bank hash, explicit Tower vote slots, epoch stake root, stake activation
hash, stake account state hash, stake-history hash, StakeHistory sysvar account
hash, Tower lockout hash, Tower replay hash, account-inclusion root,
AccountsLtHash checksum, and bank-fork hash into the vote preimage. The
verifier rejects
signed finality contexts whose `epoch` does not equal
`finalized_slot / 432000`, whose `parent_slot` is not the direct parent
(`parent_slot + 1 == finalized_slot`), and requires `epoch_stake_root` to
derive from that same epoch plus the active vote roster under
`sccp:solana:epoch-stake-root:v1`. It also requires `stake_activation_hash` to
derive under `sccp:solana:stake-activation:v1` from the epoch, active vote
roster, activation epochs, and deactivation epochs, with every validator
activated before the signed epoch (`activation_epoch < epoch`) and a
deactivation epoch greater than its activation epoch. Cooling-down stake can be
included only through the StakeHistory replay below. It also
requires `stake_account_state_hash` to derive under
`sccp:solana:stake-account-state:v1` from the stake-activation hash plus each
validator's authorized voter key, delegated stake, activation/deactivation
epochs, vote account address, stake account address, vote account state hash,
and stake account state hash; vote and stake account addresses must be unique,
non-zero, distinct per validator, and disjoint across both roles. Each supplied vote-account state hash and
stake-account state hash must now equal
`blake2b256("sccp:solana:account-opening:v1" || account_opening_bytes)`, where
the canonical opening bytes bind the account address, owner program id,
lamports, rent epoch, executable flag, and account-data hash. Vote-account
openings must be owned by Solana's Vote program id, stake-account openings must
be owned by Solana's Stake program id, both accounts must be non-executable,
and lamports plus account-data hash must be non-zero. Each vote-account
opening's account-data hash must derive under
`sccp:solana:vote-account-data:v1` from the node pubkey, authorized voter,
authorized withdrawer, inflation and block-revenue collector pubkeys,
commission basis-point fields, pending delegator rewards, optional compressed
BLS pubkey, rooted slot, and explicit 31-slot active post-root vote stack; the
authorized voter must match the validator public key, and the rooted slot plus
Tower stack must match the signed finality context. The vote proof also carries
each raw 3,762-byte Solana `VoteStateVersions::V1_14_11`, `V3`, or `V4` account
buffer; the verifier parses the raw buffer, requires it to produce the same
semantic vote-account transcript, and rejects mismatched or missing raw data.
For V1/V3 inputs the verifier derives Solana's SIMD-0185 V4 defaults from the
vote account address and node pubkey; for V4 inputs it binds the two collector
pubkeys, both commission basis-point values, pending delegator rewards, and the
optional 48-byte compressed BLS pubkey directly from the account buffer. V4 raw
commission fields must not exceed 10,000 bps before transcript derivation. If
that optional V4 BLS pubkey is present it must be non-zero; absent BLS keys use
the empty transcript field.
App-side SDK helpers expose the same parser/hash path for portal and mobile
provers by checking the variant discriminator, rooted slot option, 31 active
post-root vote entries with descending Solana confirmation counts, strict
post-root Tower slot ordering, and the authorized-voter epoch map. Every
authorized-voter key in that raw map, including future scheduled rotations that
are not the selected current voter, must be non-zero. V4 raw vote accounts are
additionally capped to the four authorized-voter epoch entries covered by the
current Anza vote-interface V4 max-size fixture before portal/mobile helpers
derive the transcript. The raw parser also consumes the
remaining VoteState suffix: V1/V3 prior-voter cursor data must have a valid
circular buffer index and boolean empty flag; prior-voter entries with a zero
pubkey must also have zero epoch bounds, and non-zero prior-voter keys must
have increasing epoch bounds. Epoch-credit history is capped to Solana's
64-entry bound, must be sorted/monotonic, and must not include epochs after the
signed finalized-bank epoch. The final timestamp tuple must either be Solana's
default `(0, 0)` or point at-or-before the newest parsed Tower vote slot with a
non-negative timestamp. The rest of the fixed account buffer must be zero
padding. This keeps
malformed raw VoteState suffixes, repeated/equal vote slots, root slots that
overlap the active Tower stack, or alternate padded bytes from being treated as
valid portal/mobile witness material while the full Tower replay remains future
work.
Each stake-account
opening's account-data hash must derive under
`sccp:solana:stake-account-data:v1` from the staker, withdrawer, delegated vote
account pubkey, delegated stake, activation/deactivation epochs, and
the 8-byte legacy/current Solana warmup-cooldown-rate layout slot,
`credits_observed`, plus the Solana `StakeFlags` byte; the delegated vote
account pubkey, stake, and activation/deactivation epochs must match the
finality-context validator arrays, while the warmup/cooldown slot bytes must
match the raw account buffer and one of Solana's known little-endian `f64`
encodings (`0.25` legacy or `0.09` current). The vote proof also carries each raw 200-byte Solana
`StakeStateV2::Stake` account buffer; the verifier parses the raw buffer,
requires it to produce the same semantic stake-account transcript, and rejects
mismatched or missing raw data. App-side SDK helpers expose the same parser and
hash path by checking the variant discriminator, fixed public-key and `u64`
offsets, known warmup/cooldown-rate slot bytes, currently known `StakeFlags`
bits, and zero account padding. It also requires
`stake_history_hash` to
derive under `sccp:solana:stake-history:v1` from the signed epoch, the
vote-weighted effective stakes, the stake-account delegated stakes, activation
and deactivation epochs, the stake-account state hash, and a sorted
StakeHistory sysvar window containing the signed epoch. The verifier replays
the Tower-era 900 bps warmup/cooldown schedule over that bounded window with
integer arithmetic, requires each submitted effective validator stake to equal
the replayed status, requires the signed-epoch StakeHistory effective total to
equal the replayed validator effective total, and requires the signed-epoch
activating and deactivating totals to cover the replayed validators. It also
requires the signed `stake_history_sysvar_account_hash` to equal
`blake2b256("sccp:solana:account-opening:v1" || account_opening_bytes)` for
the fixed `SysvarStakeHistory1111111111111111111111111` account owned by
`Sysvar1111111111111111111111111111111111111`; that opening must be
non-executable and its `data_hash` must derive under
`sccp:solana:stake-history-sysvar-data:v1` from Solana's bincode vector account
data layout: a little-endian `u64` entry count followed by newest-first
`(epoch, effective, activating, deactivating)` `u64` records. The vote proof
also carries the raw StakeHistory sysvar data bytes; the verifier validates the
bincode vector framing, requires the raw bytes to equal the canonical bytes
derived from the bounded witness entries, and hashes those raw bytes under the
sysvar-data domain separator. SDK helpers expose the same raw sysvar-data hash
path for portal and mobile provers, and the raw helper rejects sysvar vectors
whose records are not newest-first. The verifier still accepts the bounded
witness entries in strictly increasing epoch order for deterministic replay,
then reverses them when deriving the sysvar account data hash. The vote proof
also carries deterministic SCCP account-inclusion branches for every vote
account opening, stake account opening, and the StakeHistory sysvar opening.
The verifier hashes exact raw account/sysvar bytes under
`sccp:solana:account-raw-data:v1`, derives account-inclusion leaves from the
finalized slot, account opening hash, and raw-data hash under
`sccp:solana:account-inclusion-leaf:v1`, folds sorted branch siblings under
`sccp:solana:account-inclusion-node:v1`, and requires every branch to resolve
to the `account_inclusion_root` bound into the signed finality context. For
each opened account, the verifier also checks that the Solana Agave
AccountsLtHash account contribution can be derived from the exact preimage
`lamports || data || executable || owner || pubkey` and that zero-lamport
generic accounts contribute the identity value. Vote-account, stake-account,
and StakeHistory sysvar openings are stricter: they must prove non-zero
lamports before the verifier or SDK transcript builders accept them as opened
roles, so a live validator/sysvar witness cannot be replaced by a neutral
zero-lamport row. The vote proof carries the 2,048-byte
bank `accounts_lt_hash`; the verifier recomputes its BLAKE3 checksum, requires
it to match `accounts_lt_hash_checksum`, and binds that checksum into the
bank-fork hash. Full-bank `accounts_lt_hash` witnesses must be non-zero at the
Agave bank-hash, source-state commitment, and SDK request boundaries; generic
checksum helpers still accept a zero 2,048-byte value for diagnostics, but
opened-subset source-proof transcripts reject an all-zero residual so the
vote/stake/sysvar role rows cannot claim to exhaust the finalized bank lattice.
The finality
context also carries `bank_signature_count` and
optional `bank_hash_hard_fork_data`; the verifier recomputes Agave's SHA-256
bank internal-state hash from the parent bank hash, signature count,
blockhash, raw AccountsLtHash, and optional hard-fork bytes, then requires
`adapter.bank_hash` to equal that derived value. It also requires
`tower_lockout_hash` to
derive under `sccp:solana:tower-lockout:v1` from the epoch, finalized slot,
rooted slot, parent slot, parent bank hash, and 32-slot lockout depth, and it
requires `tower_replay_hash` to derive under `sccp:solana:tower-replay:v1`
from the epoch, finalized slot, rooted slot, direct parent slot, and explicit
31-vote active post-root Tower stack, with the final vote equal to the
finalized slot and the penultimate vote equal to the direct parent. The Tower
replay transcript also includes the derived `bank_fork_hash`, so a valid
rooted-vote stack cannot be reused against a different finalized bank-state
statement; the Tower replay audit statement exposes the stake-account-state,
StakeHistory sysvar account, and account-inclusion anchors as first-class public
inputs for UI/mobile prover requests. It also requires `bank_fork_hash` to derive under
`sccp:solana:bank-fork:v1` from the
epoch, finalized slot, direct parent slot, bank signature count, parent bank
hash, finalized bank hash, blockhash, transaction-status root,
account-inclusion root, AccountsLtHash checksum, and optional hard-fork hash
data. It also requires `accounts_lt_hash_proof_public_inputs_hash` to derive
under `sccp:solana:accounts-lt-proof-public-inputs:v1` from that same bank-state
tuple plus the Solana source domain, recursive backend id, mainnet genesis hash,
and derived bank-fork hash, so a UI-generated recursive AccountsDB proof cannot
be rebound to a different bank-state statement. For production-ready Solana
source material, the vote proof must also carry `accounts_lt_hash_proof`, a
`SccpSourceStateVerificationProofV1` OpenVerify/FastPQ capsule with proof family
`stark-fri-v1` and circuit id `sccp-solana-accounts-lt-hash-v1`. The verifier
decodes that capsule, checks its verifier-key hash against
`source_state_verifier_hash`, checks its public input schema and columns against
the finalized-bank statement, and replays the FastPQ proof before accepting the
source adapter proof. The OpenVerify schema descriptor now also carries the
governed `source_state_verifier_id` and exact `source_state_verifier_hash`,
matching the FastPQ verification context, so source-state proof capsules are
self-describing for portal/mobile provers as well as replay-checked by the Rust
verifier. The same canonical Norito byte check is applied to the
outer envelope, STARK wrapper, and backend FastPQ proof before those fields are
trusted. The Rust AccountsLtHash proof builder now applies the same
production-ready source-state verifier check before packaging a UI/backend
proof capsule, so template-derived Solana AccountsDB verifier hashes cannot
produce proof bytes even for diagnostic helper calls.
Wrong circuit ids, backend tags, schema descriptors, auxiliary
envelope data, public-input columns, or backend proof bytes fail the Solana
source-state gate before the vote certificate is accepted. Source-state and
source-adapter OpenVerify proof envelopes are also capped at 2 MiB before
decode, so oversized adapter or AccountsLtHash proof bytes fail before Norito
or FastPQ work begins. The capsule also binds
`opened_accounts_lt_hash_contributions_hash`, derived under
`sccp:solana:accounts-lt-opened-contributions:v1` from the exact opened
vote-account, stake-account, and StakeHistory sysvar rows already present in the
adapter witness: each row records its role, account address, account opening
hash, raw-data hash, and Agave account `AccountLtHash` contribution, plus the
opened-subset aggregate AccountsLtHash checksum. Duplicate opened account
addresses across vote, stake, and StakeHistory sysvar rows are rejected by the
Rust verifier path and by the JavaScript, Python, Swift, Kotlin, and Java
Android transcript builders before this hash is derived. The same transcript also binds
`opened_accounts_lt_hash_residual_checksum`, the BLAKE3 checksum of the
Agave-wrapping `u16` lattice residual obtained by subtracting the opened-subset
aggregate from the supplied full-bank `accounts_lt_hash`; recombining the opened
aggregate and residual must yield the bank value whose checksum is signed in the
finality context, and the residual itself must be nonzero. This keeps the
nested source-state proof tied to the same
opened account subset as the outer account-inclusion checks and records the
algebraic bridge between that subset and the full-bank AccountsLtHash without
requiring extra SDK witness fields. Solana
SDK proof-generation helpers mirror this transcript boundary: JavaScript,
Python, Swift, Kotlin, and Java Android derive opened contribution hashes and
residual checksums directly from the opened account openings/raw data plus the
full-bank AccountsLtHash, and the mobile SDKs still accept precomputed
2048-byte Agave `AccountLtHash` rows when a linked proof engine supplies them
directly. Those supplied rows are verified byte-for-byte against the
deterministic `AccountLtHash` derived from the same account opening and raw
data before request hashing; stale RPC cache rows, mismatched proof-engine
material, or wrong StakeHistory sysvar rows fail before the UI/mobile prover can
submit bytes on-chain. The same helpers reject vote-account or stake-account
opened vectors above 8,192 entries before deriving contribution rows or local
proof requests.
All five SDK surfaces also expose the exact opened-account inclusion
witness helper for deriving the verifier-side root and split branches before
invoking the linked local prover. The helper now rejects duplicate opened
account addresses across vote, stake, and StakeHistory sysvar roles before
deriving the account-inclusion leaves and branch vectors, matching the
verifier-side duplicate-address preflight. The JavaScript web SDK freezes the
returned account-inclusion tree and opened-account witness objects plus their
branch arrays, and its TypeScript declarations mark those branch vectors
readonly, so browser portal code cannot mutate the derived root/branch package
after transcript construction.
Solana
production source material and source-adapter deployment records also carry
`source_state_verifier_id` =
`sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1` plus a
non-template `source_state_verifier_hash`; readiness stays closed if that
deployed AccountsDB recursive verifier commitment is absent or replayed. With
matching source material, audited source-adapter deployment metadata, and a
proof carrying the corresponding Tower replay, AccountsDB, and bank/fork-choice
evidence, the Solana source-adapter gate can now pass. The Rust proof builders
and verifier then repeat request-bound role separation against the selected
audit verifier hash, rejecting verifier hashes replayed from the request's
source-state verifier, material, deployment, full-light gate, finality context,
vote-message, nested AccountsLtHash proof, or audit-statement hash before proof
generation or on-chain admission. Broader launch admission
still enforces the all-lanes governance policy before live submission. This is a production
cryptographic source-engine
slice for Solana, but it is not a full Solana light client yet: full Tower BFT
vote-account/state replay beyond the bound 31-vote active post-root stack plus
rooted confirmation transcript, verification of the recursive proof that the
supplied bank AccountsLtHash is the canonical AccountsDB lattice hash for the
full finalized bank, including the residual account set, full Solana
bank-state/fork-choice rule evaluation beyond this hash binding, and deployed
recursive verifier integration remain required rollout work.
TON configured material can satisfy the same gate only when it matches the
canonical mainnet masterchain/shard profile and carries deployment-supplied
component hashes. The `sccp_ton_mainnet_source_verifier_material_v1()` helper
exposes the profile template, while
`sccp_ton_mainnet_source_verifier_material_with_hashes_v1(...)` installs the
operator-provided source trust-anchor, masterchain consensus-verifier,
shard-message-inclusion-verifier, and finality-policy hashes, and
`sccp_ton_mainnet_source_verifier_material_with_hashes_and_shard_state_v1(...)`
also installs the governed shard-state source-state verifier hash while
rejecting all-zero or template-derived shard-state verifier hashes. The template
binds the TON domain/chain, `TonMasterchainShardProof` plan,
`TonMasterchain` finality model, `sccp-source-adapter-v1` circuit,
`ton-contract-v1` backend, and the canonical
`sccp:ton:shard-proof:v1`, `sccp:ton:validator-set:v1`,
`sccp:ton:validator-set-payload:v1`,
`sccp:ton:masterchain-config-leaf:v1`,
`sccp:ton:masterchain-config-proof:v1`,
`sccp:ton:masterchain-block-message:v1`,
`sccp:ton:masterchain-signatures:v1`,
`sccp:ton:validator-set-transition-message:v1`, and
`sccp:ton:validator-set-transition-signatures:v1` layouts. Generic
ready-looking TON ids/hashes and the template-derived component hashes still
fail closed. The JavaScript, Python, Swift, Kotlin, and Java Android request
builders now reject the template-derived shard-state source-state verifier hash
before invoking an app-linked TON prover, matching the governed evidence
preflight.
Production TON source-adapter readiness also requires a complete audited
full-light-client verifier bundle. The deployment record stores
`sccp:ton:light-client:masterchain-config-mainnet:v1`,
`sccp:ton:light-client:validator-set-transition-mainnet:v1`, and
`sccp:ton:light-client:shard-accounts-dictionary-mainnet:v1` verifier hashes
as an all-or-nothing suffix, and runtime admission recomputes
`sccp:ton:full-light-client-gate:v1` from those hashes plus the governed TON
source material and source-adapter deployment hash. Partial TON audit evidence,
non-TON audit evidence, or a gate hash replayed from another deployment keeps
the source adapter closed. Core bridge-proof admission has focused regressions
for that configured TON path: audited source deployment material reaches the
all-lanes launch policy gate, while partial or mismatched TON audit records fail
before structural proof evaluation.
The TON source adapter now also carries a
`SccpTonMasterchainValidatorSignaturesProofV1` certificate. The verifier
derives the validator-set hash from ordered 32-byte Ed25519 validator keys and
non-zero weights, requires it to match the adapter and configured source trust
anchor for non-placeholder material, recomputes the masterchain block-message
hash, checks the masterchain signature hash, verifies Ed25519 signatures over
the block-message hash, and enforces strict `> 2/3` signed validator weight.
The signed block-message transcript now includes the TON masterchain
`BlockIdExt` shape: workchain id `-1`, shard
`0x8000000000000000`, root hash, and non-zero file hash. Any non-masterchain
workchain/shard or zero file hash fails before validator signatures can
authorize the proof. If the configured source trust anchor is a parent
validator set rather than the active set, the adapter can
carry a `SccpTonValidatorSetTransitionProofV1` chain. Each transition binds the
parent set hash, canonical next validator-set payload hash, payload-derived
next set hash, next-set config hash, masterchain block, masterchain seqno, and
validator-set seqno range under a transition-message hash, then requires the
parent validator set to sign that transition with strict `> 2/3` weight before
the next set becomes eligible. Transition chains must advance exactly one
validator-set seqno per step and must present strictly increasing masterchain
transition seqnos, so a chain cannot skip a validator-set update or replay a
later transition before an earlier one. The transition-signature transcript
also binds the raw next validator-set payload, so a prover cannot substitute an
opaque next-set hash that was not decoded from the submitted payload.
The adapter also carries a masterchain config proof transcript that binds the
masterchain seqno/block hash, shard state root, config root, active
validator-set hash, active validator-set payload hash, config leaf hash, config
leaf index, config value hash, and a bounded TON `HashmapE 32 ^Cell`
dictionary proof BoC. The config leaf index is fixed to TON config parameter
`34`, the active validator-set parameter; arbitrary config leaves fail before
transcript hashing. The verifier derives the dictionary proof root from the
submitted BoC, requires it to match the masterchain config root, opens key
`34` to the submitted config value hash, decodes the selected value as TON
`validators#11` or `validators_ext#12`, converts its Ed25519 validator
descriptors into SCCP's canonical validator-set payload, and requires that
payload to match the validator-signature certificate roster. The verifier also
recomputes the active validator-set payload hash, recomputes the config leaf
hash, and then binds the config root and config-proof hash into the signed
masterchain block-message hash.
TRON configured material can satisfy the source-material gate only when it
matches the canonical mainnet DPoS solid-block/receipt profile and carries
deployment-supplied component hashes. The
`sccp_tron_mainnet_source_verifier_material_v1()` helper exposes the profile
template, while `sccp_tron_mainnet_source_verifier_material_with_hashes_v1(...)`
installs the operator-provided witness-schedule trust-anchor, solid-block
consensus-verifier, transaction-source/message-inclusion verifier, and
finality-policy hashes.
Production TRON material must then be completed with
`sccp_tron_mainnet_source_verifier_material_with_hashes_and_emitter_v1(...)`,
which adds the governed source bridge emitter id plus the non-zero 20-byte TVM
source bridge contract address and non-zero runtime code hash expected in the
proven transaction call. The reference
`contracts/tron/sccp/SccpTronSourceBridge.sol` contract is the TVM
source-emitter artifact for that address: it stores lane-specific immutable
metadata, allows only its owner to submit a non-zero digest, rejects digest
replay, requires submitted source/target domain arguments to match the immutable
lane metadata, and emits the canonical indexed `SccpSourceEvent(bytes32)` log
shape. Owner rotations also emit the new `SourceBridgeConfigHash`, so rollout
evidence can bind the current owner without relying on a separate manual query.
The production transaction-source proof authenticates the successful owner-gated
call itself under the TRON transaction Merkle root; receipt logs remain
diagnostic for the legacy structural path only. For the TRON -> SORA lane this
source bridge uses TRON as the non-zero source domain and SORA as target domain
`0`; the TVM source bridge therefore requires `targetDomain = 0` and rejects
any non-TRON source domain, any non-SORA target-domain id, and
same-source/target deployments.
The template binds the TRON
domain/chain, `TronDposReceiptProof` plan,
`TronDpos` finality model, `sccp-source-adapter-v1` circuit,
`tron-groth16-bn254-v1` backend, and the canonical
`sccp:tron:receipt-proof:v1`,
`sccp:tron:receipt-state-proof:v1`,
`sccp:tron:transaction-source-proof:v1`,
`sccp:tron:event-log-source-policy:v1`,
`sccp:tron:solid-block-header-proof:v1`,
`sccp:tron:witness-schedule:v1`,
`sccp:tron:witness-schedule-payload:v1`,
`sccp:tron:solid-block-message:v1`, `sccp:tron:witness-seal:v1`,
`sccp:tron:witness-schedule-transition-message:v1`, and
`sccp:tron:witness-schedule-transition-seal:v1` layouts. Generic ready-looking
TRON ids/hashes and the template-derived component hashes still fail closed.
The TRON evidence script and the JavaScript, Python, Swift, Kotlin, and Java
Android source-material/deployment hash helpers reject template-derived source
trust-anchor, consensus-verifier, message-inclusion, and finality-policy hashes
before emitting production-looking material. Those SDK helpers also recompute
the TRON source bridge config hash from the bridge address, network id, TRON ->
SORA lane ids, and owner address, rejecting mismatched caller-supplied config
hashes before deriving record hashes. The direct TRON evidence helper parses
source and destination domain CLI values as canonical ASCII decimal `u32`
values only, and its importable hash/calldata functions require exact Python
integers, so booleans and alternate spellings such as `0x5` or `05` cannot
alias production lane ids. It also treats fixed-width source component hashes
and network ids as exact CLI evidence; surrounding whitespace fails before
source bridge config, source material, deployment, destination binding, or
route records can be rendered.
The canonical TRON receipt-proof, receipt-state transcript, and typed
receipt-root MPT value helpers reject zero `source_event_digest`,
`receipt_root`, and `transaction_root` values before hashing or envelope
construction in Rust and the JavaScript, Python, Swift, Kotlin, and Java SDKs;
the receipt-proof, receipt-state, and transaction-source transcript helpers
also require a non-empty SCCP source inclusion branch, matching source-envelope
admission.
Rust also rejects wrong source domains, zero solid-block heights, and zero
block/schedule/receipt/transaction/proof hashes before deriving the
`sccp:tron:solid-block-message:v1` transcript. Witness-seal and
witness-schedule-transition seal hashes are emitted only after the signed
certificate is internally valid, exceeds the strict `> 2/3` witness-weight
threshold, and binds the signed message plus next-schedule payload hash.
Matching TRON source-adapter deployment metadata can now satisfy production
source admission only through the transaction-root-safe path: the adapter must
prove the submitted `transaction_bytes` under the signed-header `txTrieRoot`
with `transaction_index`, `transaction_count`, and a bounded
`transaction_merkle_branch`, and the transaction must call the governed source
bridge contract with the source domain, target domain, and SCCP source digest.
The TRON DPoS verifier also runs the bounded adapter-shape gate directly, so
mixed transaction-source proofs with legacy receipt-root branches, receipt MPT
nodes, receipt-root indexes, wrong adapter domains, zero block/root/seal/proof
hashes, non-canonical witness bitmaps, all-zero TRON witness addresses,
insufficient signed witness weight, truncated/non-canonical header and witness
signatures, or stale witness-schedule transition metadata fail before
witness-seal acceptance.
This TRON source plan is intentionally source-message-call-only. Current TRON
`accountStateRoot` is authenticated by the signed header proof, but it is not
treated as an Ethereum-style world-state root that opens TVM contract
`storageRoot` or `codeHash` values. State-derived TRON claims must fail closed
until a separate source proof plan and material profile bind a
consensus-authenticated contract-state root.
The JavaScript, Python, Swift, Kotlin, and Java Android TRON Groth16 proof
request builders reject empty SCCP bundle bytes before request-hash derivation
and length-prefix both `bundleBytes` and `sourceProofBytes` inside the request
hash preimage, so portal/mobile provers cannot accidentally sign a
descriptor-only request or an ambiguous bundle/source-proof split.
The TRON/TVM verifier wrapper also preflights the Groth16 envelope version,
message id, cleartext source-domain word, and commitment root against the
configured SORA -> TRON lane before verifier dispatch, and rejects
source-domain words wider than `uint32` before they can reach pairing
verification. Its accepted-proof event now carries the SCCP statement hash and
destination binding hash, letting live canary logs be matched to the exact
governed statement and deployed verifier binding.
The same SDKs now harden the TRON source-call calldata helper used by UI/mobile
witness collection: it accepts only `sourceDomain = TRON`,
`targetDomain = SORA`, and a non-zero 32-byte source-event digest before
deriving `submitSccpSourceEvent(uint32,uint32,bytes32)` calldata. The offline
operator helper now mirrors that derivation: passing
`--source-event-digest` to
`scripts/sccp_tron_source_bridge_evidence.py` in compact JSON mode emits
`source_event_call_data` and an unsigned `source_event_call.trigger_request`
body for the owner transaction, while TOML modes reject the one-off call payload
so governance records remain deployment evidence only.
The live collector mirrors the same JSON-only behavior when a deployed source
bridge is queried: `--source-event-digest` emits a `source_event_call` block and
`offline_source_event_args`, but is rejected with `--full-toml`. The replay
arguments are rebuilt from the saved source bridge domains, owner, digest, and
canonical calldata, and
post-submit replay requires the saved raw transaction/signature summary to
recompute before `offline_source_event_args` are emitted.
Before emitting the transaction calldata, the live collector also queries
`submittedSourceEvents(bytes32)`. Pre-submit JSON aborts if the digest is
already recorded on the deployed bridge; post-submit JSON with
`--source-event-transaction-id` requires it to be recorded and then verifies the
transaction readback. Fresh pre-submit JSON includes an unsigned
`source_event_call.trigger_request` body for `/wallet/triggersmartcontract`;
operators still sign and broadcast outside the helper. After broadcast,
operators pass `--source-event-transaction-id` with the same digest to read
`/wallet/gettransactioninfobyid`, require a successful receipt, and verify the
`SccpSourceEvent(bytes32)` log address, exact two-topic shape, digest, and empty
data. The helper also reads the raw transaction through
`/wallet/gettransactionbyid` and requires a single `TriggerSmartContract` call
whose `contractRet` is `SUCCESS`; an explicitly present top-level `ret` must be
java-tron's default `SUCESS = 0`, while an omitted default `ret` is accepted.
The owner address, contract address, and calldata must match the governed source
bridge owner, bridge address, and source-event digest. That raw transaction
readback must also carry
`raw_data_hex` whose SHA-256 equals the requested `txID`, plus exactly one
canonical 65-byte low-S TRON recoverable secp256k1 signature that recovers to
the source bridge owner. The visible transaction `data` field and event log
address/data are accepted only as lowercase exact hex, so uppercase or `0X`
aliases cannot be normalized into source-event evidence; any supported
result-extension byte fields used while reconstructing block transaction bytes
follow the same rule. The helper parses the signed `raw_data_hex` protobuf and
requires the embedded `TriggerSmartContract` owner, contract, calldata,
ref-block, expiration, timestamp, and fee-limit fields to satisfy the same
source-call profile as the production transaction-source verifier. The JSON
readback also emits canonical transaction protobuf bytes, their SHA-256
transaction hash, and the transaction Merkle branch used by the
transaction-source proof builder. When operators provide `--receipt-root` and
repeated `--source-inclusion-branch-hex` values, the helper derives canonical
`sccp:tron:transaction-source-proof:v1` bytes/hash and compares
`--receipt-proof-hash` when present. It also fetches the containing block,
rebuilds the canonical block-header `raw_data` hash and TRON
block id, fetches the immediate parent block, verifies the child `parentHash`
and monotonic timestamp, recovers both child and parent header signatures to
their declared TRON witness addresses, and recomputes java-tron's transaction
Merkle root from canonical transaction protobuf bytes to match the signed-header
`txTrieRoot`. When the child and parent headers carry the non-zero
account-state roots required by the shared SCCP transcript helper, JSON also
emits the canonical `solid_block_header_proof` bytes/hash; otherwise it reports
the blocker. Operators can additionally supply
`--witness-schedule-payload-hex` or `--witness-schedule-payload-file`; live JSON
then derives the `sccp:tron:witness-schedule:v1` hash, optionally checks
`--expected-witness-schedule-hash`, and requires both recovered block witnesses
to be members of that active schedule. Operators can also pass `--receipt-root`,
`--receipt-proof-hash`, `--witness-seal-signers-bitmap-hex`, and repeated
`--witness-seal-signature-hex` values; live JSON then derives the canonical
`sccp:tron:solid-block-message:v1` bytes/hash and
`sccp:tron:witness-seal:v1` bytes/hash, verifies signatures recover to the
selected active-schedule witnesses before canonical seal serialization,
enforces strict `> 2/3` signed weight, and optionally checks
`--expected-witness-seal-hash`. Missing seal inputs remain visible as a proof
blocker. `--solid-block-ancestor-depth` and
`--solid-block-confirmation-depth` fetch the bounded signed header chains needed
by non-placeholder TRON material; the helper checks backward ancestor linkage,
forward confirmation linkage, active witness membership, monotonic timestamps,
and strict `> 2/3` unique confirmation weight before marking those header proofs
ready. If the active witness schedule differs from the governed trust-anchor
hash, repeated `--witness-schedule-transition-json` objects provide the
parent/next schedule payloads, transition block, signer bitmap, and signatures;
the helper anchors each transition block to the solid, parent, or ancestor
header evidence, requires transition signatures to recover to the selected
parent-schedule witnesses with strict `> 2/3` signed weight, and only marks
production readiness when the canonical transition message/seal chain ends at
the active schedule. Inline and file-backed transition JSON rejects duplicate
object keys before any transition message or seal hash is derived. With
`--solid`, those readbacks use
`/walletsolidity/gettransactioninfobyid`,
`/walletsolidity/gettransactionbyid`, and `/walletsolidity/getblockbynum`.
The SDK transaction-source proof helpers also recompute the java-tron transaction
Merkle root from the supplied full transaction bytes, index/count, and branch
before returning the `sccp:tron:transaction-source-proof:v1` transcript hash,
and they reject transaction signatures that are not canonical TRON recoverable
secp256k1 signatures before deriving that hash. When callers provide the
governed `sourceBridgeEmitterAddress` and `sourceBridgeOwnerAddress`, those
helpers also reject source-call transactions whose embedded
`TriggerSmartContract` contract or owner address differs from the production
source material before any transcript hash is returned.
The live TRON evidence collector reconstructs java-tron transaction bytes for
every transaction in the containing block before deriving the transaction
Merkle branch. Non-source transactions may therefore carry the normal
`Transaction.Result` market-order `orderDetails` extension or the stake-v2
`cancel_unfreezeV2_amount` map without blocking source-event evidence
collection. Result and block-header integer fields may be supplied either as
JSON numbers or canonical decimal strings, but booleans and values outside
non-negative `int64` bounds fail closed before any Merkle transcript is
derived; non-ASCII or leading-zero decimal strings are not canonical and are
rejected.
TRON HTTP API success bodies are capped before JSON decoding, HTTP error
details are capped before diagnostics are rendered, and duplicate JSON object
keys are rejected rather than parsed with last-value-wins semantics. Runtime
TronGrid API keys must be exact non-empty ASCII tokens without whitespace or
control characters; file-backed keys may only carry terminal newlines.
Source-event success enums accept the canonical Java-Tron names and their
numeric values (`ret = 0`, `contractRet = 1`) so protobuf JSON enum rendering
differences do not break otherwise canonical transaction evidence, while
non-ASCII or leading-zero numeric enum strings fail closed. The top-level
`ret` enum must use java-tron's `SUCESS` spelling when rendered by name; the
`SUCCESS` alias is rejected for that field.
The repeated `--source-inclusion-branch-hex` Merkle siblings may be any
canonical 32-byte value, including all-zero, because branch elements are
transcript data rather than roots or identifiers.
Live block-header reconstruction rejects all-zero `0x41`-prefixed witness
addresses before raw-header bytes or proof hashes are derived, matching the
Rust and SDK raw-header parsers.
Witness schedule payloads still require non-zero per-witness weights, and the
Rust, Python, JavaScript, Swift, Kotlin, Java Android, and live collector
helpers reject schedules whose total weight cannot fit the `u64` `totalWeight`
committed by TRON witness seals.
The public Rust transaction-source bytes/hash helper has matching
source-bridge-bound variants for operator tooling that already has governed
source material; the bound variants return the same transcript for matching
material and fail closed on bridge, owner, zero-address, or non-production
source-material drift.
The Rust verifier rejects non-canonical protobuf varints before parsing TRON
transaction, raw-data, result, log, or block-header fields, so overlong or
overflow-shaped protobuf encodings cannot create alternate accepted signed
transcripts. The JavaScript, Python, Swift, Kotlin, and Java Android
solid-block header helpers decode the same raw header fields, reject
non-canonical protobuf varints, and recompute raw-data hashes, block ids,
parent links, trie roots, witness address, timestamp, and header version before
returning `sccp:tron:solid-block-header-proof:v1` bytes.
The production TRON adapter carries `transaction_index`, `transaction_count`,
bounded `transaction_bytes`, and a bounded `transaction_merkle_branch`; when
that source proof is present, legacy receipt fields must stay canonical:
`receipt_root_index` must be zero, `receipt_root_branch` must be empty, and
`receipt_trie_proof_nodes` must be empty. The
verifier hashes `transaction_bytes` with SHA-256, replays java-tron's
left/right binary Merkle tree with odd unpaired leaves carried upward, and
requires the computed root to equal the adapter `transaction_root` that was
bound to the signed header's `txTrieRoot`. It then parses the transaction
protobuf: exactly one `raw_data` field, exactly one canonical recoverable
low-S secp256k1 signature over `sha256(raw_data)` that recovers to the
non-zero `TriggerSmartContract.owner_address`, exactly one result whose
`contractRet` is `SUCCESS`, and, when present, whose top-level `ret` is
java-tron's default `SUCESS = 0`; canonical java-tron transactions that omit
that default `ret` field are accepted. Only the canonical optional
`Result.fee` field may accompany those success fields. Unknown
`Transaction.Result` fields and unknown
top-level `Transaction` fields are rejected. The transaction must carry exactly one
`raw_data.contract` of type `31`
(`TriggerSmartContract`) with the exact Any type URL
`type.googleapis.com/protocol.TriggerSmartContract`. The trigger must carry
non-zero `0x41` owner and contract addresses, zero
`call_value`, `call_token_value`, and `token_id` when present, and calldata
equal to `keccak256("submitSccpSourceEvent(uint32,uint32,bytes32)")[0..4] ||
abi_word_u32(source_domain) || abi_word_u32(target_domain) ||
source_event_digest`. The signed `raw_data` envelope only accepts the canonical
single-contract fields `ref_block_bytes`, optional deprecated `ref_block_num`,
`ref_block_hash`, `expiration`, `contract`, `timestamp`, and `fee_limit`; the
ref-block bytes/hash, expiration, timestamp, and fee limit must be present and
non-zero, and expiration must be strictly after timestamp. Unknown `raw_data`,
`Transaction.Contract`, `Any`, and `TriggerSmartContract` envelope fields are
rejected so future java-tron call extensions fail closed until they are profiled
and bound. The governed emitter configured in source material is matched against
the trailing 20 bytes of the TRON contract address, and the source/target ABI
words must match the proof lane.
The legacy TRON adapter path still carries `receipt_root_index` plus bounded
`receipt_trie_proof_nodes`. The verifier derives the RLP transaction-index key
and checks the legacy Ethereum-style Merkle-Patricia-Trie transcript as a
structural diagnostic only. When used by placeholder or structural material,
the proven value may decode as a bounded TRON `TransactionInfo` protobuf with
exactly one `result = SUCESS` field and a log carrying
`topic0 = keccak256("SccpSourceEvent(bytes32)")`, `topic1 =
source_event_digest`, and empty event data before checking the receipt-state
transcript hash. Each matched legacy log may contain only the canonical address,
topics, and data protobuf fields; unknown per-log fields fail closed. This
structural `TransactionInfo` path is not production
admission because TRON `txTrieRoot` is a transaction Merkle root and does not
authenticate those logs or this Ethereum-style MPT opening. Missing or repeated
result fields are rejected. Placeholder structural fixtures may still use the
typed RLP
envelope `[ "sccp:tron:receipt-root-value:v1", receipt_root ]`; bare 32-byte
roots are rejected. Legacy binary `receipt_root_branch` openings are rejected
for the TRON adapter path.
The MPT verifier now accepts canonical inline child nodes by traversing the raw
embedded RLP node from its parent, while rejecting duplicate unused inline proof
entries.
The adapter also carries `SccpTronSolidBlockHeaderProofV1`, which binds the
raw TRON `BlockHeader.raw_data` bytes into the same transcript. The verifier
parses the canonical protobuf fields for timestamp, `txTrieRoot`, parent block
id, block number, optional `witness_id`, witness address, header version, and
`accountStateRoot`; rejects any other unknown raw-header fields; requires
`sha256(raw_data)` to match `raw_data_hash`; derives the TRON block id by
placing the block number in the first eight bytes of the raw-data hash; checks
that the derived block id equals both the header proof and adapter block hash;
requires `txTrieRoot == transaction_root`; and recovers the block-header
secp256k1 signature to the declared 21-byte TRON witness address. The proof
also carries the immediate parent raw header, parent signature, and parent
raw-data hash. The verifier derives the parent block id from that signed parent
header, requires it to equal the child header's `parentHash`, requires the
child height to be exactly parent height + 1, requires monotonic timestamps,
and recovers the parent header signature to the parent witness address. Both
child and parent witness addresses must be present in the active witness
schedule before the DPoS seal is accepted. The adapter can also carry bounded
`solid_block_ancestor_headers`, ordered from the immediate parent's parent
backwards. Each ancestor header is independently decoded, hashed, signed by a
listed active witness, linked by parent block id, required to decrease by one
height per step, and required to have a strictly older timestamp than the
previous header. The adapter also carries bounded
`solid_block_confirmation_headers`, ordered forward from the solid block's
child. Each confirmation header is independently decoded, hash-checked, linked
by parent block id, required to increase by one height with a strictly newer
timestamp, and signed by an active witness. Non-placeholder TRON
source-verifier material requires the ancestor chain to be present and requires
the unique confirmation-header witnesses' schedule weight to exceed two thirds
of the active witness schedule weight before the DPoS seal is accepted. TRON
recoverable secp256k1 header and witness signatures are accepted only when `r`
is non-zero and below the secp256k1 scalar order, `s` is non-zero low-S, and the
recovery id is one of java-tron's raw ids `0..=3` or normalized ids `27..=30`.
High-S malleable encodings, invalid `r` scalars, or out-of-range recovery ids
cannot produce alternate accepted proofs. Raw header, parent header, ancestor header, and
witness-schedule payload decoding also rejects any declared witness address that
is `0x41` followed by twenty zero bytes before the address can contribute to a
solid-block or schedule transcript.
The generic TRON adapter-binding check also runs this header proof, so a
malformed signed solid-block header, ancestor-chain link, or confirmation-chain
link is rejected before production source material or recursive verifier
evidence is evaluated. The same structural adapter check also recomputes the
witness schedule hash, solid-block message hash, and witness seal hash, so a
swapped schedule, seal digest, or signed message transcript cannot satisfy the
generic binding path before full source-material verification runs.
The TRON source adapter now also carries a `SccpTronDposWitnessSealProofV1`
certificate. The verifier derives the witness schedule hash from 21-byte TRON
witness addresses and weights, requires it to match the configured source trust
anchor for non-placeholder material, recomputes the solid-block message hash,
checks the witness seal hash, recovers secp256k1 public keys from the
signatures, maps them to TRON addresses, rejects high-S malleable recoverable
signatures, and enforces strict `> 2/3` signed witness weight before accepting
the source-adapter evidence. If the configured source trust anchor is a parent
witness schedule rather than the active one, the adapter can carry a
`SccpTronWitnessScheduleTransitionProofV1` chain. Each
transition carries the canonical next witness-schedule payload
(`0x01 || witness_count_le || (tron_address || weight_le)[0..n]`), binds its
`sccp:tron:witness-schedule-payload:v1` hash into the transition message,
requires the payload-derived `sccp:tron:witness-schedule:v1` hash to match the
signed next schedule, and then requires the parent witness schedule to seal that
transition with strict `> 2/3` weight before the next schedule becomes
eligible. Each transition block hash must also be anchored to the supplied
solid, parent, or signed ancestor header evidence. Multi-step transition chains
are bounded to at most 64 transitions and must be epoch-contiguous and strictly
increasing by transition block number. The canonical Rust transition-message
helper is defined only for TRON
source-domain messages that advance exactly one schedule epoch, carry a
non-zero transition block number, and bind non-zero transition, parent, next,
and payload hashes; the transition-seal helper rederives that message hash
before accepting the parent-schedule signatures.
Substrate-family configured material can satisfy the same gate only when it
matches the canonical SORA Kusama, SORA Polkadot, or SORA2 GRANDPA/event-storage
profile and carries deployment-supplied component hashes. The
`sccp_substrate_family_runtime_source_verifier_material_v1(...)` helper exposes
the profile templates, while
`sccp_substrate_family_runtime_source_verifier_material_with_hashes_v1(...)`
is the legacy structural helper that does not make material production-ready
because it leaves the runtime storage-proof verifier on its template hash.
Production-ready Substrate-family material must use
`sccp_substrate_family_runtime_source_verifier_material_with_hashes_and_runtime_storage_v1(...)`
so the operator-provided GRANDPA authority-set trust-anchor,
finalized-header consensus-verifier, event-storage inclusion-verifier, runtime
storage-proof verifier, and finality-policy hashes are all non-template
governed values. The templates bind the Substrate-family domain/chain,
`SubstrateGrandpaEventProof` plan, `SubstrateGrandpa` finality model,
`sccp-source-adapter-v1` circuit, `sccp-substrate-runtime-storage-v1`
source-state circuit, `fastpq-lane-balanced` source-state parameter set,
`substrate-runtime-v1` backend, and the canonical
`sccp:substrate:storage-proof:v1`,
`sccp:substrate:authority-set:v1`,
`sccp:substrate:authority-set-payload:v1`,
`sccp:substrate:grandpa-precommit:v1`,
`sccp:substrate:grandpa-justification:v1`,
`sccp:substrate:authority-set-transition-message:v1`, and
`sccp:substrate:authority-set-transition-justification:v1` layouts. Generic
ready-looking Substrate-family ids/hashes and the template-derived component
hashes still fail closed. Matching Substrate-family verifier material and
source-adapter deployment records can open production source-adapter admission
only when the submitted source proof also carries the
`runtime_storage_verification_proof: SccpSourceStateVerificationProofV1`
OpenVerify/FastPQ capsule for the runtime-storage statement. The gate
recomputes the runtime-storage public inputs from the source event digest,
`System.Events` storage key, finalized height, GRANDPA set id, finalized block
hash, authority-set hash, events root, and inclusion branch, then checks the
capsule circuit id, schema descriptor, verifying-key hash, and FastPQ proof
against the governed source-state verifier hash in the material and deployment
records.
The JavaScript, Python, Swift, Kotlin, and Java Android SDKs expose the same
runtime-storage request pieces, so browser and mobile proof engines build the
exact statement/context/schema/column set that production Rust admission
recomputes before accepting the source-state capsule.
The all-lanes readiness summary derives and publishes the same
`substrate_runtime_storage_gate_hash` for SORA-Kusama, SORA-Polkadot, and SORA2
from the governed material plus source-adapter deployment records. Release
bundle verification treats that value as a required source-adapter gate audit
hash for each Substrate-family lane, so a production bundle cannot omit the
runtime-storage proof gate while still claiming the lane is ready. The
Substrate source-evidence renderer applies the same rule to production TOML:
JSON dry-runs remain available for diagnostics, but `toml_ready` stays false
and `--toml` output is rejected until the source-material hash,
source-adapter deployment hash, and runtime-storage gate hash are all supplied
and match the canonical values.
Operators can render the governed source material and source-adapter
deployment records for a Substrate-family lane with
`scripts/sccp_substrate_source_evidence.py`. The renderer pins the same
domain-specific profile ids and rejects template-derived source trust-anchor,
consensus-verifier, message-inclusion, runtime storage-proof verifier, and
finality-policy hashes using the same runtime-storage circuit id, FastPQ
parameter, public-input prefix, and transcript-prefix preimage as Rust before
emitting governance TOML, so operator material cannot carry non-zero hashes
that Rust production admission treats as template-only evidence. It also
recomputes the canonical `fastpq-lane-balanced` OpenVerify source-adapter
verifier commitment for the selected Substrate-family lane and rejects
non-canonical `adapter_verifier_vk_hash` values before rendering source-adapter
deployment TOML. Boolean or non-`u32` programmatic target-domain values are
rejected before direct hash derivation as well, so `False` cannot be treated as
the SORA domain id. CLI evidence strings are exact: surrounding whitespace on
the selected runtime lane, fixed-width component hashes, or the target domain is
rejected before source material or deployment records are rendered. The direct
material and deployment record hash helpers
apply the same live-component check, so programmatic rollout tooling cannot
derive governed Substrate-family records from template source components:

```bash
python3 scripts/sccp_substrate_source_evidence.py \
  --domain sora2 \
  --source-trust-anchor-hash <grandpa-authority-set-hash> \
  --consensus-verifier-hash <grandpa-finalized-header-verifier-hash> \
  --message-inclusion-verifier-hash <events-storage-proof-verifier-hash> \
  --source-state-verifier-hash <runtime-storage-proof-verifier-hash> \
  --finality-policy-hash <grandpa-finality-policy-hash> \
  --adapter-verifier-vk-hash <source-adapter-openverify-vk-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --expected-source-verifier-material-hash <source-material-record-hash> \
  --expected-source-adapter-engine-deployment-hash <source-adapter-deployment-record-hash> \
  --expected-runtime-storage-gate-hash <runtime-storage-source-gate-hash> \
  --toml
```

The Substrate-family source adapter now also carries a
`SccpSubstrateGrandpaJustificationProofV1` certificate. The verifier derives
the GRANDPA authority-set hash from ordered non-zero 32-byte Ed25519 authority
keys and non-zero weights, requires it to match the adapter and configured
source trust anchor for non-placeholder material, recomputes the finalized-header
precommit-message hash, checks the justification hash, verifies Ed25519
signatures over the precommit hash, and enforces strict `> 2/3` signed
authority weight before accepting the source-adapter evidence. If the configured
source trust anchor is a parent authority set rather than the active set, the
adapter can carry a `SccpSubstrateAuthoritySetTransitionProofV1` chain. Each
transition carries the canonical next authority-set payload
(`0x01 || authority_count_le || (ed25519_authority_key || weight_le)[0..n]`),
binds its `sccp:substrate:authority-set-payload:v1` hash into the transition
message, requires the payload-derived `sccp:substrate:authority-set:v1` hash to
match the signed next set, and then requires the parent authority set to justify
that transition with strict `> 2/3` weight before the next set becomes eligible.
Substrate GRANDPA source proofs are preflight-bounded to at most 2,048
authorities, 64 authority-set transitions, canonical authority payloads no
larger than `1 + 4 + 2,048 * 40` bytes, signer bitmaps no larger than 256
bytes, and 64-byte Ed25519 signatures. The web, Python, Swift, Kotlin, and Java
Android UI prover helpers enforce the same bounds before deriving authority-set
or transition transcript hashes, and reject all-zero authority keys both from
canonical inputs and raw authority-set payloads.
`iroha_sccp` also exposes explicit-material production helpers:
`build_sccp_source_verifier_evidence_with_material(...)`,
`build_sccp_source_adapter_verification_proof_with_material(...)`,
`verify_sccp_source_chain_proof_envelope_production_with_material(...)`, and
`verified_sccp_message_source_chain_proof_envelope_for_production_with_material(...)`.
Deployment-aware variants additionally take
`SccpSourceAdapterEngineDeploymentV1` and require a non-zero deployment receipt
hash that exactly mirrors the configured trust anchor, consensus verifier,
message-inclusion verifier, finality policy, proof family, circuit id, target
domain, and the lane-specific OpenVerify verifier-key commitment carried by the
source adapter proof envelope.
Those variants also require the submitted source proof's verifier evidence to
bind the exact deployment hash and receipt hash; a material-only proof remains
valid only for diagnostics and is rejected by the deployment-aware production
path even when the configured material otherwise matches. The public
`sccp_source_chain_proof_matches_adapter_deployment(...)` helper applies the
same deployment-evidence binding, so callers cannot mistake a material-only
proof with the right adapter verifier commitment for deployment-bound evidence;
it also rechecks the adapter OpenVerify proof against the recomputed deployment
evidence hash, so post-construction evidence splices fail.
The evidence and adapter-proof helpers let governance/config-sourced material
replace the placeholder catalog without changing the proof envelope; the
production verification helpers also require the source-adapter readiness gate.
Node configuration now exposes `zk.sccp_source_verifier_materials`,
`zk.sccp_source_adapter_engine_deployments`,
`zk.sccp_destination_rollouts`, and `zk.sccp_route_allowlists`; all four lists
are part of the ZK consensus policy hash and are converted into SCCP V1 records
at bridge proof admission. A configured non-SORA lane opens only when exactly
one source material record, source deployment record, destination rollout
record, and route allowlist record match the source/counterparty domain, all
digest fields are valid 32-byte hex and non-zero, the source material is
non-placeholder, the source material passes the exact domain-profile gate, the
deployment receipt is non-zero, the deployment exactly matches the
material/proof family/circuit tuple, governed source bridge emitter id/address
and runtime code hash where a source-emitter contract is required, ETH/BSC
source-emitter metadata comments from the live JSON-RPC collector matching the
governed emitter address plus replayable runtime bytecode whose Keccak-256 hash
matches the governed runtime code hash, TRON
source material and deployment records carry the same non-zero
`source_bridge_network_id`, `source_bridge_owner_address`, and
`source_bridge_config_hash`, the config hash recomputes from those values and
targets SORA for inbound production admission, the deployment carries
`tron_dpos_source_gate_hash`, and that hash exactly recomputes from the
governed source material hash, source-adapter deployment hash, adapter verifier
key, DPoS/witness/source-call verifier role hashes, source bridge config,
canonical TRON verifier transcript prefixes, and bounded TRON proof shapes. The
same field must be empty on non-TRON lanes, and it is included in the ZK
consensus policy hash so configured admission cannot drop this DPoS/source-call
deployment gate after TOML preflight. TRON production TOML renderers require
the expected DPoS source gate hash to match before reporting source TOML or full
rollout readiness, while JSON dry-runs may still derive the hash for operator
review. The deployment's
`adapter_verifier_vk_hash` equals the canonical lane-specific
FastPQ/OpenVerify verifier commitment and the submitted source proof's
OpenVerify `vk_hash`, the source material and source-adapter deployment hash
metadata comments match the canonical record hashes recomputed from the
structured fields, the destination rollout is exact for the counterparty
verifier profile, the route allowlist is exact for the governed route profile,
and the route allowlist hash is the canonical hash over the source material
record hash, source-adapter deployment record hash, and destination binding
hash; those three governed inputs must be non-zero and pairwise distinct before
the route allowlist transcript is accepted. The public release-bundle verifier
recomputes the same transcript from embedded all-lanes JSON before accepting a
published release attachment, so a forged `expected_route_allowlist_hash_matches`
flag cannot self-attest the route binding. Route canary evidence must bind
that route allowlist hash and destination
binding hash without reusing the source material record hash, source-adapter
deployment record hash, route allowlist hash, or destination binding hash, and
it must be unique across advertised lanes. Cross-lane canary replay is attached
to the target lane's all-lanes JSON blockers, so per-lane `production_ready`
cannot remain true for a rejected bundle. The
submitted source proof's evidence and adapter OpenVerify statement bind to that
material and deployment. Duplicate, placeholder, malformed,
wrong-domain, built-in-placeholder-reused, generic-profile, replayed verifier
material, missing deployment, zero deployment receipt, replayed deployment
material, replayed adapter verifier commitment, non-SORA source-adapter target,
missing source-record hash comments, stale source-record hash comments,
missing destination rollout, missing route allowlist, or replayed route material
fails closed. Admission also enforces the configured launch policy against
configured material: with the first-release Ethereum-mainnet launch policy,
complete Ethereum evidence can open the Ethereum inbound lane independently,
while BSC, Solana, TON, TRON, and Substrate-family lanes remain blocked until
their own lane policy opens. The all-lanes checker remains available as a
diagnostic for future coordinated launches. The default production verifier
continues to use the built-in catalog and therefore remains closed when no
explicit lane material is configured.

Production destination rollout records must carry explicit binding evidence in
`destination_network_id`, `destination_bridge_address`,
`destination_binding_key`, and `destination_binding_hash` as required by the
lane. ETH/BSC records must include the destination network id, bridge wrapper
address, canonical EVM deployment binding key, and binding hash. Public
release-bundle verification rejects a zero `destination_bridge_address` whenever
that bridge-wrapper field is published, so an all-zero EVM destination address
cannot pass attachment review. It also requires ETH/BSC attachments to publish
both lane-specific fields, requires TRON attachments to publish only the network
id, and rejects network or bridge-wrapper fields on static Solana, TON, and
Substrate-family destination bindings. TRON records must include the destination
network id, canonical TRON binding key, and binding hash, and runtime lane
readiness also requires that network id to match the governed source bridge
material. Solana, TON, and Substrate-family records must include the canonical
static destination binding key/hash and must not carry EVM/TRON network or
bridge-wrapper fields.
The ZK consensus policy hash includes the fields, so governed destination
binding evidence is committed by the policy digest instead of relying only on
operator comments. Runtime readiness requires exact verifier identities across
destination families: padded EVM addresses, Solana program ids, TON raw
addresses, TRON Base58Check addresses, and Substrate runtime entrypoints are
rejected instead of being trimmed into production-ready rollout material. The
offline destination evidence helpers apply the same exact-input posture to
verifier identities, fixed-width hashes, lane selectors, and deployment metadata
before rendering governance TOML.

For ETH/BSC destination rollout evidence, operators can use
`scripts/sccp_evm_destination_evidence.py` to recompute the same destination
binding hash as `SccpMessageBridge`, then render the matching
`zk.sccp_destination_rollouts` and `zk.sccp_route_allowlists` TOML records. The
helper is limited to the SORA -> ETH and SORA -> BSC EVM-family destination
lanes, rejects boolean or non-`u32` programmatic source/target domain values,
zero addresses and hashes, derives `verifier_code_hash` from runtime bytecode
when requested, can derive the bridge wrapper runtime hash from
`--bridge-runtime-bytecode-hex` or `--bridge-runtime-bytecode-file`, rejects
non-canonical direct-helper backend or proof-family labels, and requires
`--expected-destination-binding-hash` to match before emitting production TOML.
Inline `--bridge-runtime-bytecode-hex` and
`--verifier-runtime-bytecode-hex` values are exact evidence: surrounding or
embedded whitespace is rejected instead of being normalized into runtime
preimages, and inline values must use a lowercase `0x` prefix with lowercase
hex. Use the corresponding `--*-runtime-bytecode-file` inputs for formatted hex
artifacts.
The CLI and reusable `render_toml(...)` / `_json_summary(...)` entrypoints run
the same derivation and mismatch checks, so portal backends cannot skip
runtime-bytecode verification by importing the helper module directly. The rendered destination rollout stores
the network id, bridge wrapper address, canonical binding key, and canonical
binding hash as explicit fields and retains the same values in comments for
audit comparison. It also emits the canonical `evm-groth16-bn254-v1` verifier
backend hash and `stark-fri-v1` proof-family hash comments consumed by
all-lanes preflight. For production TOML, the rendered output also carries
`sccp_evm_bridge_runtime_bytecode_hex` and
`sccp_evm_verifier_runtime_bytecode_hex`; all-lanes decodes those comments and
recomputes Keccak-256 before accepting ETH/BSC destination rollout evidence.
The rendered route allowlist must also bind to the exact
source-material record hash, source-adapter deployment record hash, and
destination binding hash; any supplied `--route-allowlist-hash` that does not
match that canonical lane evidence tuple fails before JSON or TOML is emitted,
and route allowlist evidence requires `--expected-destination-binding-hash` even
for JSON dry-runs. Omitting route allowlist arguments keeps JSON output in
binding-only audit mode so operators can compute the expected binding before
staging governed route evidence. Production TOML also requires replayable
bridge-wrapper and verifier runtime bytecode, a non-zero bridge runtime code
hash derived from that bytecode, and EVM route canary metadata derived from a
successful `MessageProofAccepted` transaction. The direct renderer requires the
transaction hash, log index, calldata SHA-256, message id, payload hash, target
domain, statement hash, commitment root, finality height, finality block hash,
receipt block number/hash/`receiptsRoot`, proof version, proof source domain, and a live
`usedMessageProofs(messageId) = true` assertion; if a
`--route-canary-evidence-hash` is supplied, it must match that v3
transaction-derived transcript. Rust `iroha_sccp` route-allowlist admission
stores and recomputes the same v3 canary fields before treating EVM route
evidence as bound. JSON summaries stay in diagnostic mode with
`toml_ready = false`
until the expected destination binding pin, both runtime bytecode blobs, the
bridge wrapper runtime hash, route tuple, and transaction-derived route canary
evidence are all present. Direct destination and full-lane renderers also
reject route canary evidence hashes that reuse the governed
source-material record hash, source-adapter deployment record hash, route
allowlist hash, or destination binding hash before JSON summaries or production
TOML are emitted.
The all-lanes preflight also rejects EVM/TRON route-canary transaction hash
roles and TON live-account route-canary hashes when they alias the governed
source-material, source-deployment, route-allowlist, or destination-binding
hashes they are supposed to prove. Public release-bundle verification mirrors
the TON non-zero and lane-hash alias checks for the published live-account
route-canary fields.
The all-lanes preflight extends that aliasing guard across lanes: a route
canary evidence hash for one domain cannot replay another domain's source
material hash, source-adapter deployment hash, destination binding hash, or
route allowlist hash.
Rust
EVM Groth16 package construction and verification parse the resulting
`evm:<source>:<target>:<network>:<verifier>:<bridge>:<code-hash>:<key-hash>`
key, where the `<network>` segment is lowercase 32-byte hex without `0x` while
address/code/key hash segments retain their canonical prefixes. It recomputes
the canonical binding hash from that deployment tuple and rejects forged
key/hash pairs before accepting relay material. The Rust binding
builder also rejects forked proof-family strings, forked verifier backend keys,
non-SORA local domains, non-ETH/BSC counterparties, non-EVM verifier targets,
zero deployment material, and reference verifier key hashes outside the
production Groth16 backend before a binding record can be constructed:

```bash
python3 scripts/sccp_evm_destination_evidence.py \
  --domain eth \
  --network-id <evm-network-id-bytes32> \
  --verifier-address <groth16-verifier-contract-address> \
  --bridge-address <sccp-message-bridge-wrapper-address> \
  --bridge-runtime-bytecode-hex <bridge-wrapper-runtime-bytecode> \
  --verifier-runtime-bytecode-hex <verifier-runtime-bytecode> \
  --verifier-key-hash <groth16-verifying-key-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-destination-binding-hash <bridge-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --route-canary-transaction-hash <message-proof-accepted-tx-hash> \
  --route-canary-log-index <message-proof-accepted-log-index> \
  --route-canary-receipt-block-number <receipt-block-number> \
  --route-canary-receipt-block-hash <receipt-block-hash> \
  --route-canary-block-receipts-root <receipt-block-receipts-root> \
  --route-canary-call-data-sha256 <submit-calldata-sha256> \
  --route-canary-message-id <accepted-message-id> \
  --route-canary-payload-hash <decoded-payload-hash> \
  --route-canary-target-domain <decoded-target-domain> \
  --route-canary-statement-hash <accepted-statement-hash> \
  --route-canary-commitment-root <accepted-commitment-root> \
  --route-canary-finality-height <decoded-finality-height-word> \
  --route-canary-finality-block-hash <decoded-finality-block-hash> \
  --route-canary-proof-version <decoded-proof-version> \
  --route-canary-proof-source-domain <decoded-proof-source-domain> \
  --route-canary-used-message-proof true \
  --toml
```

For deployed ETH/BSC wrappers, operators can collect those same values directly
from read-only JSON-RPC calls with `scripts/sccp_evm_live_evidence.py`. The
live helper calls `SccpMessageBridge` immutable views, verifies that the bridge
is bound to `evm-groth16-bn254-v1` and `stark-fri-v1`, checks the JSON-RPC
endpoint `eth_chainId` against the requested SCCP lane (`eth` defaults to
chain id 1 and `bsc` defaults to 56; `--expected-rpc-chain-id` can make that
canonical pin explicit but cannot override it), checks
`verifierCodeHash()` against `eth_getCode` runtime bytecode, checks
`verifierKeyHash()` against the verifier contract's `verifyingKeyHash()`, and
recomputes the destination binding key/hash from the live network id, bridge
address, verifier address, verifier code hash, and verifier key hash. The
wrapper's `destinationBindingHash()` view must match that recomputed value, so
live collection fails if the deployed wrapper reports a binding that no longer
matches its immutable deployment inputs. With a
governed route allowlist hash plus the source material and deployment record
hashes, it recomputes the canonical route allowlist hash. Live full-TOML
rendering also requires the RPC chain-id match, `--expected-network-id`, and
`--expected-bridge-code-hash`, so both the bridge wrapper runtime observed
through `eth_getCode` and the wrapper's governed network id are pinned to
audited values before the same TOML shape consumed by the all-lanes preflight is
emitted. Ethereum destination full TOML also requires the collection block tag
to be `finalized`; explicit `latest` or `safe` Ethereum reads remain available
for JSON inspection, but they cannot render governed production TOML. BSC
destination TOML keeps its `latest` default. Both the live collector and direct
audited ETH/BSC destination renderer emit `sccp_evm_block_tag` metadata; the
all-lanes preflight requires that comment and rejects Ethereum destination
rollout evidence unless it is `finalized`. The public all-lanes lane summary
also exposes this as `evm_live_metadata.destination_block_tag`, so
release-bundle verification can reject forged summaries that hide
non-finalized Ethereum destination reads. The release-readiness report mirrors
this value in each cryptographic-evidence row as `evm_destination_block_tag`,
making the finalized destination pin visible in both JSON and Markdown release
artifacts. Production full TOML also requires
the same route canary evidence hash
as the direct helper, but live collection derives it by fetching the supplied
`--route-canary-transaction-hash`, checking the receipt status, the
`MessageProofAccepted` log at `--route-canary-log-index`, the receipt block
number/hash against `eth_getBlockByNumber`, a non-zero block `receiptsRoot`, the
submitted `submitSccpMessageProof(bytes,bytes32[6],bytes32)` calldata, the
384-byte proof tuple header, and `usedMessageProofs(messageId)`. Duplicate
matching `MessageProofAccepted` events at the supplied log index are rejected as
ambiguous receipt evidence. The canonical EVM
route-canary transcript uses the `v3` hash label and commits the receipt block
number, receipt block hash, block `receiptsRoot`, exact submitted calldata
SHA-256, decoded payload hash, ETH/BSC target-domain word, finality height,
finality block hash, proof ABI version `1`, SORA proof source-domain word, and
`usedMessageProofs` consumption flag alongside the accepted event tuple and
destination binding/backend/network pins. The direct renderer, public hash
helper, runtime config gate, and all-lanes preflight also reject reuse between
the distinct transaction hash, receipt block hash, block `receiptsRoot`,
calldata, message id, payload, statement, commitment, finality height, and
finality block hash roles, so a canary cannot be replayed across proof tuple
versions, EVM-family lanes, drifted Groth16 public inputs, stale receipt
blocks, or synthetic transcript-role aliases. Public release-bundle
verification applies the same non-zero rule to each EVM route-canary
transaction/public-input word published in readiness and all-lanes JSON. A
manually supplied canary hash is only accepted as a pin to that derived value.
The live TOML
carries the observed RPC chain id, bridge wrapper runtime code hash, verifier
runtime code hash, their observed
`eth_getCode` bytecode, verifier backend hash, and proof-family hash as
metadata comments; the all-lanes preflight requires those comments to replay to
the canonical EVM production profile, rejects non-canonical uppercase runtime
bytecode preimages in staged evidence, and recomputes the EVM route canary hash
from the transaction calldata, public inputs, proof header, event metadata, and
consumed-message state before ETH/BSC destination rollout records can pass launch
readiness.
The live collector also treats the explicit expected RPC chain id and JSON-RPC
returned quantities or hex byte strings as exact evidence; surrounding
whitespace fails instead of being normalized into destination rollout metadata.
It also caps successful JSON-RPC responses and HTTP error details before
decoding, rejects duplicate JSON object keys instead of accepting
last-value-wins parsing, and requires every success envelope to echo
`jsonrpc = "2.0"` with the request id `1` before using the result.
The diagnostic `offline_evidence_args` replay the observed deployment material,
but only include `--expected-destination-binding-hash` and route
allowlist/source-record hash arguments after the operator supplied that expected
binding pin and it matched the live binding:
the paired `torii_destination_query_params` are withheld until the same pin
matches, and when emitted the summary marks
`torii_destination_query_proof_bytes_hex_required = true` because artifact/job
queries still need the prover-produced Groth16 tuple as `proof_bytes_hex`.

```bash
python3 scripts/sccp_evm_live_evidence.py \
  --rpc-url <eth-or-bsc-json-rpc-url> \
  --domain eth \
  --bridge-address <sccp-message-bridge-wrapper-address> \
  --expected-rpc-chain-id 1 \
  --expected-network-id <evm-network-id-bytes32> \
  --expected-bridge-code-hash <bridge-wrapper-runtime-code-hash> \
  --expected-destination-binding-hash <expected-binding-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --route-canary-transaction-hash <message-proof-accepted-tx-hash> \
  --route-canary-log-index <message-proof-accepted-log-index> \
  --full-toml
```

For Solana destination rollout evidence, operators can use
`scripts/sccp_solana_destination_evidence.py` to validate the deployed Solana
verifier program id and non-zero verifier code hash, then render the matching
`zk.sccp_destination_rollouts` and `zk.sccp_route_allowlists` TOML records for
the SORA -> Solana lane. The helper can derive the verifier code hash as
BLAKE2b-256 over deployed program bytes supplied with
`--verifier-program-bytes-hex`, `--verifier-program-bytes-base64`, or
`--verifier-program-bytes-file`, preserves that executable preimage in the
runtime `solana_programdata_executable_base64` rollout field, mirrored TOML
comments, and JSON summaries, and rejects any mismatch with an explicitly
supplied `--verifier-code-hash`. Production TOML cannot be rendered from a
copied `--verifier-code-hash` alone; it requires the replayable BPF ELF
ProgramData executable bytes so Core, Torii, and all-lanes preflight can
recompute the hash from the same material. Inline
`--verifier-program-bytes-hex` and
`--verifier-program-bytes-base64` values are exact evidence: surrounding or
embedded whitespace is rejected instead of being normalized into executable
preimages. Use `--verifier-program-bytes-file` for raw deployed byte artifacts.
The helper pins the exact
`SolanaProgramNativeRecursive` destination plan, Solana mainnet-beta anchor id,
governed route allowlist id, and canonical destination binding hash before
emitting production TOML. The route allowlist hash must recompute from the
source material record hash, source-adapter deployment record hash, and
canonical SORA -> Solana destination binding hash. Production TOML also
requires `--route-canary-evidence-hash` to match the canonical live-program
canary hash over the governed route hash, destination binding, source
material/deployment record hashes, verifier program id/code hash, finalized RPC
commitment, BPF-loader ownership and immutability, the upgradeable Program
account preimage, ProgramData address, slot pins, finalized read context slots,
immutable ProgramData metadata, and deployed executable bytes. Binding-only JSON
summaries may be rendered without route arguments or executable bytes so
operators can compute the expected destination binding first. If route
allowlist or paired source record hashes are supplied, the helper requires
`--expected-destination-binding-hash` to match before route evidence is
accepted; `toml_ready` remains false and production TOML is rejected until that
pin, the executable byte preimage, the route canary evidence hash, and audited
immutable ProgramData metadata are present. JSON diagnostics also expose
`route_allowlist_evidence_ready`, `route_canary_ready`,
`programdata_metadata_ready`, `verifier_program_bytes_present`, and
`full_toml_ready`. Public release-bundle verification checks the published
ProgramData route-canary address as a non-zero canonical Solana base58 address
before accepting the attachment. Complete route evidence without ProgramData pins remains a
diagnostic JSON summary with `programdata_metadata_ready = false`, while
present but stale ProgramData metadata still fails closed. `full_toml_ready` is
true only on the same complete path that can render production TOML:

```bash
python3 scripts/sccp_solana_destination_evidence.py \
  --verifier-program-id <solana-verifier-program-id> \
  --verifier-program-bytes-file <deployed-verifier-program.so> \
  --programdata-address <pinned-programdata-account> \
  --programdata-slot <pinned-programdata-slot> \
  --program-account-context-slot <finalized-program-read-slot> \
  --programdata-account-context-slot <finalized-programdata-read-slot> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-destination-binding-hash <sora-solana-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

This direct helper is useful for hash discovery and offline review, but the
all-lanes production preflight requires Solana live ProgramData metadata from
`scripts/sccp_solana_live_evidence.py` before the SORA -> Solana destination
record can pass launch readiness. That metadata must show finalized RPC reads,
BPF upgradeable-loader ownership for both Program and ProgramData accounts,
`program_immutable = true`, the canonical 36-byte upgradeable Program account
layout as a base64 preimage that points to the claimed ProgramData account,
matching ProgramData slot pins, immutable ProgramData header bytes plus their
BLAKE2b-256 hash, fresh read-context slots encoded as positive integer JSON
numbers, and a ProgramData executable preimage with a BPF ELF header whose
BLAKE2b-256 hash matches `verifier_code_hash`.
The offline Solana destination helper applies the same exact-integer policy to
importable ProgramData slot and context-slot arguments before it derives
ProgramData metadata or reports TOML readiness, so boolean values cannot stand in
for slot numbers in backend automation.

For deployed Solana verifier programs, operators can collect the verifier code
hash directly from read-only Solana JSON-RPC with
`scripts/sccp_solana_live_evidence.py`. The live helper follows the BPF
upgradeable-loader `Program` account to its `ProgramData` account, requires both
accounts to be owned by `BPFLoaderUpgradeab1e11111111111111111111111`,
requires the program account to be executable and the ProgramData account to be
non-executable, rejects non-canonical Program account data lengths, rejects any
remaining upgrade authority, and derives `verifier_code_hash` as BLAKE2b-256
over the deployed ProgramData executable bytes only after those bytes start with
the BPF ELF magic (`0x7f454c46`). Production TOML rendering requires
`--expected-verifier-code-hash`, `--expected-programdata-address`, and
`--expected-programdata-slot` to match the live account data, in addition to the
governed destination-binding and route-allowlist pins and route canary evidence
hash. Production TOML requires the read commitment to be `finalized`;
`confirmed` reads stay diagnostic. The rendered metadata also carries
`solana_program_account_data_base64`,
`solana_programdata_metadata_base64`,
`solana_programdata_executable_base64`, their BLAKE2b-256 hashes, the pinned
ProgramData address and slot, and finalized RPC context slots as actual
`zk.sccp_destination_rollouts` fields. The helper keeps the matching
`sccp_solana_*` comments for audit review, but Core and Torii read the
configured `solana_*` fields when deciding production readiness. The rendered
metadata also carries the canonical
`sccp_solana_program_account_data_len = "36"` value and finalized JSON-RPC
context slots used for the Program and ProgramData account reads, and all-lanes
preflight rejects ProgramData evidence whose Program account preimage
does not encode an upgradeable Program pointing to the claimed ProgramData
account, whose ProgramData header preimage does not encode the same deployment
slot with no upgrade authority, whose executable preimage is not BPF ELF-shaped,
whose RPC read context is earlier than the ProgramData deployment slot, or whose
RPC context slot is a boolean instead of a positive integer JSON number. The
live and direct helpers also preserve the ProgramData executable bytes as
`solana_programdata_executable_base64` and mirrored
`sccp_solana_programdata_executable_base64`, so the all-lanes preflight decodes
that preimage and recomputes BLAKE2b-256 instead of trusting a manually copied
executable hash. JSON dry runs also include `destination_toml_ready` for the
offline Solana destination material and `full_toml_ready` for the finalized,
live-pinned path, plus replayable `offline_evidence_args` and, once fully pinned, an
`offline_toml_sha256` digest for the deterministic TOML payload. The reusable
live `_summary(...)` and `render_toml(...)` entrypoints revalidate imported
live dictionaries for BPF-loader ownership, immutable ProgramData, canonical
36-byte Program layout, fresh context slots, BPF ELF executable bytes,
executable length, and executable/code-hash consistency before emitting JSON or
TOML, so backend automation cannot bypass RPC collection by passing forged
metadata. ProgramData slot arguments, direct inline executable byte arguments,
and live executable base64 metadata are exact: surrounding whitespace fails
before TOML readiness or all-lanes fields/comments can be derived. The Solana
JSON-RPC collector also caps successful responses and HTTP error details before
decoding, and rejects duplicate JSON object keys instead of accepting
last-value-wins parsing. This matches the direct helper's exact integer policy
for imported ProgramData metadata:

```bash
python3 scripts/sccp_solana_live_evidence.py \
  --rpc-url <solana-json-rpc-url> \
  --verifier-program-id <solana-verifier-program-id> \
  --expected-programdata-address <pinned-programdata-account> \
  --expected-programdata-slot <pinned-programdata-slot> \
  --expected-verifier-code-hash <pinned-programdata-executable-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-destination-binding-hash <sora-solana-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

The SDK destination binding helpers derive the same SORA -> Solana binding key
and hash for user-side proof requests. JavaScript, Python, Swift, Kotlin, and
Java Android route-canary ProgramData helpers default their expected destination
binding to that canonical hash and reject both supplied destination-binding
hashes and explicit expected hashes that would bind a Solana canary to any other
lane.

For TON destination rollout evidence, operators can use
`scripts/sccp_ton_destination_evidence.py` to validate the deployed TON
verifier contract raw address on basechain workchain `0`, non-zero verifier
code hash, governed source material record hash, and governed source-adapter
deployment record hash, then render the matching `zk.sccp_destination_rollouts`
and `zk.sccp_route_allowlists` TOML records for the SORA -> TON lane. The
raw address, fixed-width hash, and last-transaction logical-time fields must be
exact canonical strings; surrounding whitespace is rejected before any TOML or
hash material is rendered. The
helper can derive `verifier_code_hash` from a deployed single-root TON code BoC
(`--verifier-code-boc-base64`, `--verifier-code-boc-hex`, or
`--verifier-code-boc-file`) using the TON representation hash, including
CRC32C-checked BoCs and strict cell padding checks; if a manual
`--verifier-code-hash` is also supplied it must match the BoC root hash. Inline
`--verifier-code-boc-hex` and `--verifier-code-boc-base64` values are exact
evidence: surrounding or embedded whitespace is rejected instead of being
normalized into code-BoC preimages. Use `--verifier-code-boc-file` for raw,
hex, or base64 artifacts that carry ordinary file formatting. The helper pins
the exact `TonContractNativeRecursive` destination plan, TON
mainnet anchor id, governed route allowlist id, canonical destination binding
hash, and route allowlist hash recomputed from the
source/deployment/destination tuple before emitting production TOML.
Binding-only JSON summaries may be rendered without route arguments so
operators can compute the expected destination binding first. If route
allowlist or paired source record hashes are supplied, the helper requires
`--expected-destination-binding-hash` to match before route evidence is
accepted; `toml_ready` remains false and production TOML is rejected until that
pin, the route canary evidence hash, audited active account-status metadata,
account-state metadata, and
matching code-BoC root evidence are present. All-lanes preflight also requires
the TON account-state and last-transaction hashes carried as canary evidence to
remain distinct from the governed source, deployment, route, destination, and
verifier-code hash roles before accepting the live-account canary:

```bash
python3 scripts/sccp_ton_destination_evidence.py \
  --verifier-contract-address <0:account_hex> \
  --verifier-code-boc-base64 <ton-verifier-code-boc-from-accountStates> \
  --source-verifier-material-hash <ton-source-material-record-hash> \
  --source-adapter-engine-deployment-hash <ton-source-adapter-deployment-record-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --expected-destination-binding-hash <sora-ton-destination-binding-hash> \
  --account-status active \
  --account-state-hash <live-account-state-hash> \
  --last-transaction-lt <live-last-transaction-lt> \
  --last-transaction-hash <live-last-transaction-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

The TON `--route-canary-evidence-hash` is not an arbitrary post-deploy marker.
It must equal the canonical live-account route canary hash over the governed
route allowlist hash, SORA -> TON destination binding hash, TON source material
record hash, audited source-adapter deployment record hash, verifier raw
address, verifier code hash, active account status, live account-state hash,
canonical last transaction LT, last-transaction hash, and verifier code-BoC root
hash. The live account-state hash and last-transaction hash must be distinct,
so a single digest cannot be replayed across the two TON snapshot roles. The
helper emits the matching `ton_route_canary_*` route allowlist fields and
metadata comments so the runtime and all-lanes preflight can recompute the same
transcript. Release-bundle verification rejects all-zero published TON
live-account route-canary hashes and TON route-canary hashes that reuse governed
source, route, or destination hash roles.

JavaScript portals, Python relay services, and mobile apps can derive that same
transcript with `canonicalTonSccpRouteCanaryEvidenceBytes` /
`tonSccpRouteCanaryEvidenceHash`,
`canonical_ton_sccp_route_canary_evidence_bytes` /
`ton_sccp_route_canary_evidence_hash`, Swift's
`canonicalTonSccpRouteCanaryEvidenceBytes` /
`tonSccpRouteCanaryEvidenceHash`, Kotlin's
`SccpTon.canonicalRouteCanaryEvidenceBytes` /
`SccpTon.routeCanaryEvidenceHash`, and Java Android's
`TonSccpProver.canonicalRouteCanaryEvidenceBytes` /
`TonSccpProver.routeCanaryEvidenceHash`. These SDK helpers use the same
fail-closed checks as the Rust/operator path before a user-generated proof is
submitted on-chain.

This direct helper is useful for hash discovery and offline review, but the
all-lanes production preflight requires explicit active TON account-status
metadata, account-state metadata, and a code-BoC base64 comment that can be
decoded and replayed to the same root hash as both the BoC-root comment and the
rollout `verifier_code_hash`; the `sccp_ton_code_boc_hash_matches` comment
must also be present and `true`.
Direct/manual TON records that only copy a code hash, without replayable
BoC bytes and root-match evidence, remain diagnostic and do not pass launch
readiness.

For deployed TON verifier contracts, operators can collect the verifier code
hash directly from read-only TON Center v3 `accountStates` responses with
`scripts/sccp_ton_live_evidence.py`. The live helper requires the verifier
account to be `active`, requires a present `code_boc`, normalizes the returned
`code_hash`, `account_state_hash`, and last-transaction hash to 32-byte hex,
and recomputes the `code_boc` single-root representation hash to reject any
remote `code_hash` drift. Padded raw addresses or padded remote hash text fail
closed instead of being trimmed into rollout evidence, and padded `code_boc`
base64 fails both live collection and imported summary rendering before offline
replay arguments or TOML comments are produced. The collector also rejects API
URLs that embed credentials, params, queries, or fragments and caps the
`accountStates` JSON response before decoding, so hidden request state or
oversized remote payloads cannot be normalized into rollout evidence. Runtime
API keys must be exact non-empty ASCII tokens without whitespace or control
characters, oversized HTTP error details are truncated before diagnostics are
rendered, and duplicate JSON object keys are rejected rather than parsed with
last-value-wins semantics. TOML output requires both
`--expected-verifier-code-hash` and `--expected-account-state-hash` plus
`--route-canary-evidence-hash` matching the same canonical live-account canary
transcript, and emits `sccp_ton_code_boc_base64`,
`sccp_ton_code_boc_root_hash`, and `sccp_ton_code_boc_hash_matches` comments.
JSON dry runs include replayable `offline_evidence_args` and, once fully
pinned, an `offline_toml_sha256` digest for the deterministic TOML payload.
They also split `destination_toml_ready` from `full_toml_ready`: the former
means the live destination/route evidence is structurally complete, while the
latter additionally requires independent verifier-code and account-state pins:

```bash
python3 scripts/sccp_ton_live_evidence.py \
  --api-url <toncenter-v3-api-root-or-accountStates-url> \
  --verifier-contract-address <0:account_hex> \
  --expected-account-state-hash <pinned-ton-account-state-hash> \
  --expected-verifier-code-hash <pinned-ton-code-hash> \
  --source-verifier-material-hash <ton-source-material-record-hash> \
  --source-adapter-engine-deployment-hash <ton-source-adapter-deployment-record-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --expected-destination-binding-hash <sora-ton-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

The SDK destination binding helpers derive the same SORA -> TON binding key and
hash for user-side proof requests.

For Substrate-family destination rollout evidence, operators can use
`scripts/sccp_substrate_destination_evidence.py` to render the SORA -> SORA
Kusama, SORA Polkadot, or SORA2 runtime rollout records. The helper pins the
exact `SubstrateRuntimeNativeRecursive` destination plan,
`SccpBridge.submit_message_proof` runtime entrypoint, lane-specific runtime
anchor id, governed route allowlist id, and canonical destination binding hash
before emitting production TOML. The helper can derive the verifier code hash as
BLAKE2b-256 over deployed runtime code supplied with `--runtime-code-hex`,
`--runtime-code-base64`, or `--runtime-code-file`, and rejects any mismatch
with an explicitly supplied `--verifier-code-hash`. When runtime code is
supplied, TOML also carries `sccp_substrate_runtime_code_base64` so all-lanes
can replay the BLAKE2b-256 hash instead of trusting the standalone runtime
code-hash comment. Inline `--runtime-code-hex` and `--runtime-code-base64`
values are exact evidence: surrounding or embedded whitespace is rejected
instead of being normalized into runtime-code preimages. Use
`--runtime-code-file` for raw finalized runtime artifacts. The route allowlist
hash must recompute from the source
material record hash, source-adapter deployment record hash, and the selected
canonical SORA -> Substrate-family destination binding hash. Finalized runtime
metadata must carry the lane's exact `specName` plus exact nonnegative integer
`specVersion` and `transactionVersion` values; boolean or truthy placeholders
are rejected before TOML readiness can be derived.
Binding-only JSON summaries may be rendered without route arguments so
operators can compute the expected destination binding first. If route
allowlist or paired source record hashes are supplied, the helper requires
`--expected-destination-binding-hash` to match before route evidence is
accepted; `toml_ready` remains false and production TOML is rejected until that
pin and a route canary evidence hash recomputed from the finalized runtime
metadata are present. The canary hash binds the governed route hash,
destination binding, source material/deployment record hashes, runtime
entrypoint, verifier code hash, finalized head, runtime `specName`,
`specVersion`, `transactionVersion`, and finalized runtime bytes. The all-lanes
summary publishes the finalized head and runtime code hash in the route-canary
row, and public release-bundle verification rejects zero or governed-hash-reused
values for every Substrate-family route canary:

```bash
python3 scripts/sccp_substrate_destination_evidence.py \
  --domain sora2 \
  --runtime-code-file <finalized-runtime-code.wasm> \
  --finalized-head <pinned-finalized-block-hash> \
  --runtime-spec-name sora2 \
  --runtime-spec-version <runtime-spec-version> \
  --runtime-transaction-version <runtime-transaction-version> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-destination-binding-hash <sora-substrate-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

This direct helper is useful for hash discovery and offline review, but the
all-lanes production preflight requires Substrate live finalized-runtime
metadata from `scripts/sccp_substrate_live_evidence.py` before a SORA -> SORA
Kusama/SORA Polkadot/SORA2 destination record can pass launch readiness.
The direct helper treats the selected runtime lane and `runtime-spec-name` as
exact evidence; surrounding whitespace fails before destination rollout or
route-allowlist metadata can be rendered.

For deployed Substrate-family runtime destinations, operators can collect the
runtime code hash directly from read-only JSON-RPC with
`scripts/sccp_substrate_live_evidence.py`. The live helper reads the finalized
head with `chain_getFinalizedHead`, reads `state_getRuntimeVersion` and the
well-known `:code` storage key at that finalized head, and derives
`verifier_code_hash` as BLAKE2b-256 over the finalized runtime WASM bytes. The
live summary, offline replay arguments, and rendered TOML preserve those
runtime bytes as base64 in `sccp_substrate_runtime_code_base64`.
Production TOML rendering requires the finalized head, runtime code hash,
`specName`, `specVersion`, and `transactionVersion` to be pinned explicitly, in
addition to the governed destination-binding, route-allowlist, and route canary
evidence pins. The pinned runtime version values must be exact nonnegative
integers, not booleans. The live helper treats `specName`, expected `specName`,
runtime version text, finalized head hex, and runtime `:code` hex as exact
evidence; surrounding whitespace fails before runtime metadata or TOML
readiness can be rendered. The Substrate-family JSON-RPC collector caps
successful responses and HTTP error details before decoding, and rejects
duplicate JSON object keys instead of accepting last-value-wins parsing. JSON
dry runs include replayable `offline_evidence_args` and, once fully pinned, an
`offline_toml_sha256` digest for the deterministic TOML payload:

```bash
python3 scripts/sccp_substrate_live_evidence.py \
  --rpc-url <substrate-json-rpc-url> \
  --domain sora2 \
  --expected-finalized-head <pinned-finalized-block-hash> \
  --expected-runtime-code-hash <pinned-runtime-code-hash> \
  --expected-spec-name <runtime-spec-name> \
  --expected-spec-version <runtime-spec-version> \
  --expected-transaction-version <runtime-transaction-version> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --source-verifier-material-hash <source-material-record-hash> \
  --source-adapter-engine-deployment-hash <source-deployment-record-hash> \
  --expected-destination-binding-hash <sora-substrate-destination-binding-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --toml
```

The SDK destination binding helpers derive the same SORA -> SORA Kusama,
SORA Polkadot, or SORA2 runtime binding key and hash for user-side proof
requests.

All destination evidence direct output helpers recompute their canonical
SORA -> destination binding hash during direct TOML rendering and JSON summary
generation, then reject caller-supplied binding hashes that do not match the
governed lane even when the CLI expected-hash guard is bypassed. Production TOML
rendering requires the explicit expected binding pin, and route-bearing JSON is
accepted only after that pin matches; unpinned JSON summaries remain
binding-only diagnostics. The helpers also revalidate direct-output deployment
and route allowlist inputs, including native verifier identities, verifier
code/key hashes, source record hashes, and governed route allowlist hashes, so
automation cannot bypass CLI parsing and emit zero destination rollout or
allowlist material.
Native destination rollout profiles must not carry a `verifier_key_hash`;
that field is reserved for EVM-family/TRON Groth16 verifier deployments and is
rejected on Solana, TON, and Substrate-family runtime rollouts.

For the production TRON -> SORA source lane, the configured source records have
the following shape. Hash and address values are deployment evidence, not
defaults: `source_bridge_emitter_address` is the trailing 20-byte contract
address used by TVM/EVM logs, `source_bridge_emitter_code_hash` is the deployed
runtime bytecode hash, `source_bridge_network_id` is the bytes32 network id
used by the deployed source bridge, `source_bridge_owner_address` is the
trailing 20-byte governed owner account address from the TRON
`TriggerSmartContract.owner_address`, and `source_bridge_config_hash` is the
value returned by `sourceBridgeConfigHash()` and emitted by
`SourceBridgeConfigured` or `SourceBridgeConfigHash`.

After collecting the live source bridge address, current owner, network id,
runtime bytecode hash, component hashes, and deployment receipt hash, operators
can render just the source material and source-adapter deployment records with
`--toml`, or all four records required by lane readiness with `--full-toml`.
Direct TRON source evidence accepts only lowercase exact inline runtime
bytecode, runtime-bytecode files, fixed-width hashes, and hex-form TRON
addresses, while runtime-bytecode files may still contain ordinary whitespace
such as line breaks around otherwise canonical lowercase hex.
Full-TOML rendering recomputes the governed route allowlist hash from the
canonical TRON source material record hash, source-adapter deployment record
hash, and SORA -> TRON destination binding hash, then rejects a supplied
`--route-allowlist-hash` that does not match that exact evidence tuple.
Direct JSON dry-runs apply the same route check only after
`--expected-destination-binding-hash` pins the recomputed SORA -> TRON binding,
so route evidence cannot be staged against unapproved destination material.
Rust route-evidence helpers derive that hash only after the source material,
source-adapter deployment, destination rollout, and TRON network-id coherence
are all production-ready, so governance tooling cannot mint an evidence-bound
route record from replayed or internally incomplete lane components.
The source-adapter verifier key hash is derived from the canonical
TRON -> SORA FastPQ/OpenVerify profile; a supplied `--adapter-verifier-vk-hash`
is only an audit check. The TOML modes are intentionally limited to
the production TRON -> SORA source lane (`source_domain = 5`,
`target_domain = 0`) so the helper cannot emit records that Rust production
admission will reject. Full rollout TOML also fixes the destination verifier
side to the paired SORA -> TRON lane (`destination_source_domain = 0`,
`destination_target_domain = 5`) and the canonical `stark-fri-v1` proof family.
The renderer also rejects template-derived TRON source trust-anchor, consensus,
message-inclusion, or finality-policy hashes before emitting governance TOML, so
operators must provide live deployed component hashes. The direct material and
deployment record hash helpers apply the same live-component check, so
programmatic evidence tooling cannot bypass the renderer and derive governed
record hashes from template source components. JavaScript, Python, Swift,
Kotlin, and Java Android direct helpers also recompute
`source_bridge_config_hash` from bridge address, network id, TRON -> SORA lane
ids, and owner address before emitting TRON source material hashes.

Operators can collect the live view values with the read-only
`scripts/sccp_tron_live_evidence.py` helper before rendering governance TOML.
It queries `networkId()`, `sourceDomain()`, `targetDomain()`, `owner()`,
`sourceBridgeConfigHash()`, `verifierCodeHash()`, `verifierKeyHash()`, and
`destinationBindingHash()` through TRON constant calls, optionally reads
`/wallet/getcontract` bytecode metadata, recomputes the source config and
destination binding hashes, and emits the matching offline-renderer arguments.
Each constant call must return an explicit successful `result.result = true`
response as well as one ABI word, so malformed node responses with only
`constant_result` data fail closed.
When destination verifier evidence is collected, it also emits the
`network_id_hex`, `tron_verifier_address`, `verifier_code_hash_hex`, and
`verifier_key_hash_hex` fields accepted by Torii SCCP artifact/job requests,
plus the mandatory `expected_destination_binding_hash_hex` pin only after the
operator supplied `--expected-destination-binding-hash` and it matched the live
verifier view. `tron_verifier_address` is emitted and accepted only as an exact
checksummed TRON Base58Check address with a `0x41` payload prefix; surrounding
whitespace is rejected instead of normalized. The JSON also sets
`torii_destination_query_proof_bytes_hex_required = true` when these fields are
present because the Groth16 proof tuple must still come from the prover and be
sent as `proof_bytes_hex`. These Torii query parameters are withheld unless
`/wallet/getcontract` destination bytecode metadata is present, the runtime
bytecode preimage is preserved, and its recomputed hash matches
`verifierCodeHash()`; `--no-getcontract` remains diagnostic JSON, not
submit-ready query material. The importable query-param projection re-parses the
summary fields and recomputes the SORA -> TRON destination binding before
exposing Torii artifact/job query material, so hand-built summaries cannot
bypass the live collector's backend, proof-family, code-hash, or binding checks.
It also parses summary source/target domains with the same canonical ASCII u32
rule, so boolean or leading-zero domain values are ignored rather than coerced.
Torii refuses EVM/TRON deployment-bound artifact/job
requests without that independent binding hash and proof tuple, rejects
deployment material that does not recompute to the hash read from the live
verifier, and requires a configured production SCCP destination rollout for the
counterparty. The caller's deployment-derived
destination binding key/hash must match that rollout before Torii returns
artifacts, proof jobs, bridge-proof submissions, or bridge-message settlement
scaffolds.
When both the source bridge and destination verifier are queried, their
`networkId()` values must match before offline arguments or route evidence are
accepted, matching the runtime readiness gate that binds TRON destination
rollouts back to the governed source bridge network id.
The same fields are available as CLI flags.
The helper performs no signing, deployment, broadcast, or state mutation.
When operators provide the governed source component hashes,
`--deployment-receipt-hash`, and the two expected source record hashes, the live
helper recomputes the canonical TRON source verifier material and
source-adapter deployment record hashes in the same read-only pass. When
metadata lookup is enabled, `/wallet/getcontract` must return source-bridge
runtime bytecode for the same contract address being queried, and a manually
supplied `--source-bridge-emitter-code-hash` must match that observed bytecode
hash. Operators must explicitly pass `--no-getcontract` when using an
independently audited source-bridge code hash. Destination verifier bytecode
metadata must also identify the queried verifier address, be present, and match
the verifier's `verifierCodeHash()` view when metadata lookup is enabled. The
live and direct helpers carry the source bridge and destination verifier runtime
bytecode as `sccp_tron_source_bridge_runtime_bytecode_hex` and
`sccp_tron_destination_verifier_runtime_bytecode_hex`, and all-lanes preflight
recomputes the deployed code hashes from those bytecode preimages before a TRON
lane can pass launch readiness. Direct `--full-toml` requires those source and
destination runtime bytecode preimages too; hash-only direct CLI runs remain
diagnostic JSON and do not set `full_toml_ready`.
A supplied
`--expected-source-bridge-config-hash` pins the deployment or governed
`SourceBridgeConfigured` value, so owner or lane drift fails before production
full-TOML output; the diagnostic `offline_evidence_args` only include
`--expected-config-hash` after that explicit pin matches. This pin is also
required for direct full-TOML output.
If the same run also includes destination verifier evidence,
`--expected-destination-binding-hash` matching the verifier's
`destinationBindingHash()`, `--route-allowlist-hash`, and
`--route-canary-transaction-id`, the JSON includes `offline_full_toml_args`, the
exact offline renderer argument list with the verified
`--route-canary-transaction-owner-address` and `--full-toml` appended, and
`offline_full_toml_sha256`, the SHA-256 of the internally rendered governance
TOML; `full_toml_ready` is true only on that same fully pinned path. The live
helper reads the TRON transaction receipt, verifies the destination verifier's
`MessageProofAccepted` log against `destinationBindingHash()`,
`verifierBackendHash()`, `proofFamilyHash()`, and `networkId()`, binds the
validated route allowlist hash, requires exactly one matching accepted-proof log,
then fetches the raw `TriggerSmartContract` transaction, verifies that the
visible and raw-data owner addresses match, requires the canonical transaction
signature to recover to that same owner, and verifies the
`submitSccpMessageProof(bytes,bytes32[6],bytes32)` selector, ABI public inputs,
statement hash, 384-byte proof tuple, and proof header against the accepted
event and deployed verifier domains. The visible `TriggerSmartContract.data`
field for that raw transaction must also be lowercase exact hex, so the canary
path cannot normalize uppercase or `0X` calldata aliases before replaying the
signed `raw_data_hex`. It also queries
`usedMessageProofs(messageId)` on the same verifier and requires the accepted
message id to be marked consumed in current contract state before deriving the
route canary evidence hash. That canonical
`iroha:sccp:tron-route-canary-evidence:v3` hash commits the exact
`submitSccpMessageProof(...)` calldata SHA-256, decoded payload hash, target
domain, finality height, finality block hash, proof version, proof source
domain, transaction owner address, transaction block number, transaction block
timestamp, raw-data owner binding flag, signature SHA-256, recovered signer
address, and owner-recovery flag alongside the
accepted event tuple and governed verifier/backend/network pins. Rust
`iroha_sccp` canonical TRON canary helpers reject replay across the transaction
id, calldata hash, message id, payload hash, statement hash, commitment root,
finality height, finality block hash, and signature SHA-256 roles before
route evidence can bind. If
operators provide both canary flags, the supplied hash must match the
transaction-derived hash. The rendered live TOML carries the transaction id,
transaction owner address, log index, message id, calldata SHA-256, payload
hash, target domain, statement hash, commitment root, finality height, finality
block hash, proof version, proof source domain, raw-data owner binding flag,
signature hash, recovered address, and signature-owner recovery flag as
TRON-specific canary metadata comments;
all-lanes preflight now requires those comments for TRON, requires the
recovered address to equal the transaction owner, and recomputes the same
canary evidence hash before marking the route evidence bound. It also rejects
reuse between the distinct transaction id, message id, calldata, payload,
statement, commitment, finality block, and signature hash roles before the
canary hash can be accepted. This prevents a
receipt-only canary from satisfying the full-TOML gate when the submitted
verifier call drifts from the emitted event, owner/signature evidence, or
`usedMessageProofs` state is not carried. Source-event transaction readback uses
the same single-governed-log policy for `SccpSourceEvent(bytes32)` before it can
emit replayable offline source-event args. The offline direct TRON renderer
requires the same `--route-canary-transaction-*` fields plus
`--route-canary-used-message-proof` for full TOML, rejects reused transcript
hash roles, derives the canary hash when `--route-canary-evidence-hash` is
omitted, and rejects a supplied hash that does not match the transaction
metadata. Live full-TOML rendering requires
`/wallet/getcontract` bytecode metadata for both the source bridge and the
destination verifier. Destination verifier evidence also reads
`verifierBackendHash()` and `proofFamilyHash()` and requires the canonical
`tron-groth16-bn254-v1` backend plus `stark-fri-v1` proof family before using
the verifier fields in rollout or Torii query material. Verified full-TOML
output carries those backend and proof-family hashes as destination rollout
metadata, and the all-lanes preflight requires them to match the canonical
profile. For the destination verifier, the full-TOML helper also
requires the live metadata hash to be marked as matching `verifierCodeHash()`
and rechecks the two hash strings before exposing `offline_full_toml_args` or
rendering TOML; hand-edited JSON cannot bypass that evidence. Diagnostic JSON
may still use `--no-getcontract` with an independently audited source code
hash, but that path cannot produce production-ready full TOML. The live helper
only accepts that route allowlist
hash when complete source record preflight material and destination verifier
evidence are present, the expected destination binding pin has matched, and the
hash matches the same canonical source-material, source-adapter-deployment, and
destination-binding tuple. The route canary evidence hash is required for
production full TOML and is replayed in the offline argument list only after it
has been derived from a verified `MessageProofAccepted` canary transaction. The
live verifier checks that transaction's hashed `raw_data_hex`, owner/contract
fields, lowercase exact `submitSccpMessageProof(...)` calldata, and single
canonical low-S recoverable secp256k1 signature recovering to the transaction
owner. The canary transaction-info response must also carry exact positive
integer `blockNumber` and exact nonnegative integer `blockTimeStamp` metadata,
and saved full-TOML replay revalidates those carried fields, so a receipt-only
or hand-edited response cannot be promoted into release evidence.
Live TOML also carries the route-canary block number, block timestamp,
signature hash, recovered address, and
`sccp_tron_route_canary_signature_recovers_to_owner` audit comments; all-lanes
preflight promotes the block fields into
`route_canary.block_number`/`route_canary.block_timestamp`, and release-bundle
verification requires them to remain positive/nonnegative integers before
accepting TRON route-canary evidence. The same checks reject zero TRON
owner/recovered addresses, zero route-canary evidence/route/destination hashes,
zero TRON route-canary transcript words, reused TRON route-canary hash roles, or
a recovered TRON address that differs from the transaction owner.
If TRON transaction-info logs include an explicit `logIndex` or `log_index`,
the live collector now requires it to match the log's list position before the
source-event or route-canary transcript can bind that index into release
evidence.
The canary hash recomputation uses those call-transcript and owner/signature
fields, so a reused receipt tuple without the matching submitted calldata and
signer evidence does not satisfy the production gate.
Passing `--full-toml` to the live collector prints that verified TOML directly
instead of JSON, and fails closed until the source config, source-record, DPoS
source-gate, expected destination binding, route allowlist, and route canary
checks are all pinned and verified. As with the source config pin, diagnostic
offline arguments only carry `--expected-destination-binding-hash`, the route
allowlist hash, and the route canary evidence hash after the operator supplied
the binding pin and it matched the live verifier view.
When production TOML is rendered from a saved live JSON summary, the renderer
reparses the saved `raw_data_hex` and raw signature, then revalidates the
carried route-canary selector, block number, block timestamp, Groth16 tuple
length/version/source-domain header, public-input message id, target domain,
commitment root, statement hash, owner/signature fields, event source domain,
destination binding, backend, proof family, network id, and recomputed canary
hash instead of trusting the `call_data_matches_event` flag by itself.
The all-lanes rollout preflight accepts the verified TRON live full-TOML output
as the TRON slice of a complete SCCP evidence bundle and recomputes the TRON
source record hashes from those records before reporting the lane ready. It
requires live source-bridge and destination-verifier address/code-hash metadata,
those source record hashes, and the expected destination binding hash to be
present and non-zero before recomputing route evidence, matching the Rust
deployment-material readiness path instead of producing synthetic route hashes
from incomplete or offline-only lane components. The TRON destination rollout
TOML is pinned to one `verifier_code_hash` key per rollout record so the
governance bundle remains valid for strict TOML parsers; the all-lanes
fallback TOML loader also rejects duplicate keys when `tomllib` is unavailable.
For the governed source-event transaction rollout, the same live collector
accepts `--source-event-digest` only in JSON mode with a queried source bridge.
It emits `source_event_call.source_event_call_data` plus
`offline_source_event_args` for reproducing the calldata through the direct
helper, rederives those replay arguments from the saved source bridge domains,
owner, digest, and calldata, first checks `submittedSourceEvents(bytes32)`,
includes an unsigned `source_event_call.trigger_request` body for
`/wallet/triggersmartcontract` only for fresh pre-submit digests, can verify a
post-submit transaction id against the successful exact two-topic
`SccpSourceEvent(bytes32)` log plus the raw
`TriggerSmartContract` ret/owner/contract/calldata tuple, requires
`gettransactioninfobyid` and `gettransactionbyid` transaction-id aliases
(`id`, `txID`, and `txid`) to agree, requires raw transaction readback to carry
canonical `txID`, requires `raw_data_hex` to hash to that `txID`, requires
exactly one canonical 65-byte low-S TRON recoverable secp256k1 signature that
recovers to the source bridge owner, rejects explicit source-event
`logIndex`/`log_index` metadata that drifts from the transaction-info log list
position, requires saved replay JSON to parse the source-proof transaction
`Result` bytes as a canonical successful contract result,
parses the signed `raw_data_hex` protobuf for the same
owner/contract/calldata/ref-block/timing/fee source-call profile that Rust
production admission checks, requires exact positive integer `blockNumber` and
exact nonnegative integer `blockTimeStamp` values in transaction-info readback,
cross-checks that `blockTimeStamp` against the fetched block header timestamp,
and requires saved replay JSON to preserve the same block metadata and
solid-block timestamp binding,
uses solidity transaction readback endpoints when `--solid` is set, keeps
`full_toml_ready` visible on JSON dry-runs, and rejects
`--source-event-digest --full-toml` so one-off execution payloads are not mixed
into governance TOML. Post-submit JSON now also carries canonical transaction
protobuf bytes, the SHA-256 transaction hash, and the transaction Merkle branch;
when `--receipt-root` plus repeated `--source-inclusion-branch-hex` values are
supplied, it derives canonical `sccp:tron:transaction-source-proof:v1`
bytes/hash and compares `--receipt-proof-hash` when present. It then fetches the
containing block, verifies the TRON block id against canonical block-header
bytes, fetches and links the immediate parent header material, recovers child
and parent header signatures to their declared TRON witness addresses, and
rebuilds java-tron's transaction Merkle root against `txTrieRoot` from
transaction entries whose `txID`/`txid`/`id` aliases agree; when
account-state roots are present it also emits the canonical solid-block header
proof bytes/hash, while missing roots stay visible as a blocker. Supplied
witness-schedule payloads are also
canonicalized and checked against child/parent block witnesses. Supplied
receipt roots, receipt-proof hashes, signer bitmaps, and witness signatures are
bound into the canonical solid-block message and witness-seal certificate; the
helper verifies signer recovery to selected witnesses, strict `> 2/3` signed
weight, and optional expected seal hash before marking the witness-seal proof
ready, otherwise it reports the missing seal material as a blocker. Optional
ancestor and confirmation header depths
collect the same bounded signed header chains the Rust verifier requires for
non-placeholder TRON material; those depth arguments and all signed header
versions must be exact integers, and missing or insufficient chains remain JSON
blockers instead of being hidden behind a generic rollout note. If the active
witness schedule no longer equals the governed source trust-anchor hash,
repeated `--witness-schedule-transition-json` entries derive and verify the
parent-signed transition message/seal chain, including signer recovery and
strict `> 2/3` signed parent-schedule weight, before the post-submit
source-event evidence can report production readiness.
Operators can pass `--solid` to read source/destination view functions through
`/walletsolidity/triggerconstantcontract` when the rollout snapshot must use
TRON confirmed state. For TronGrid production endpoints,
`--tron-pro-api-key-file` and
`--tron-pro-api-key` send `TRON-PRO-API-KEY` without printing the key in the
evidence JSON.

```bash
# Diagnostic live readback. This prints JSON and is useful for discovering
# governed hashes, but production cutover must also supply the source pins,
# expected DPoS source-gate hash, route allowlist, canary transaction id, and
# --full-toml.
python3 scripts/sccp_tron_live_evidence.py \
  --tron-node-url https://api.trongrid.io \
  --solid \
  --tron-pro-api-key-file <runtime-secret-file> \
  --source-bridge-address <deployed-source-bridge> \
  --destination-verifier-address <deployed-destination-verifier> \
  --expected-destination-binding-hash <sccp-tron-destination-binding-hash>
```

```bash
python3 scripts/sccp_tron_source_bridge_evidence.py \
  --bridge-address <tron-source-bridge-address> \
  --owner-address <current-owner-address> \
  --network-id <tron-network-id-bytes32> \
  --expected-config-hash <sourceBridgeConfigHash-or-event-configHash> \
  --source-bridge-runtime-bytecode-hex <source-bridge-runtime-bytecode> \
  --source-trust-anchor-hash <witness-schedule-hash> \
  --consensus-verifier-hash <solid-block-verifier-hash> \
  --message-inclusion-verifier-hash <transaction-source-verifier-hash> \
  --finality-policy-hash <solid-block-finality-policy-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --expected-source-verifier-material-hash <sccp-tron-source-material-record-hash> \
  --expected-source-adapter-engine-deployment-hash <sccp-tron-source-deployment-record-hash> \
  --expected-tron-dpos-source-gate-hash <sccp-tron-dpos-source-gate-hash> \
  --destination-verifier-address <tron-destination-verifier-base58> \
  --destination-verifier-runtime-bytecode-hex <destination-runtime-bytecode> \
  --destination-verifier-key-hash <destination-groth16-vk-hash> \
  --expected-destination-binding-hash <sccp-tron-destination-binding-hash> \
  --route-allowlist-hash <governed-route-allowlist-hash> \
  --route-canary-evidence-hash <post-deploy-route-canary-evidence-hash> \
  --route-canary-transaction-id <tron-message-proof-txid> \
  --route-canary-transaction-owner-address <0x41-prefixed-transaction-owner> \
  --route-canary-block-number <positive-transaction-block-number> \
  --route-canary-block-timestamp <transaction-block-timestamp-ms> \
  --route-canary-log-index <message-proof-log-index> \
  --route-canary-message-id <accepted-message-id> \
  --route-canary-call-data-sha256 <submit-message-proof-calldata-sha256> \
  --route-canary-payload-hash <submitted-public-input-payload-hash> \
  --route-canary-target-domain 5 \
  --route-canary-statement-hash <accepted-statement-hash> \
  --route-canary-commitment-root <accepted-commitment-root> \
  --route-canary-finality-height <submitted-finality-height-word> \
  --route-canary-finality-block-hash <submitted-finality-block-hash> \
  --route-canary-proof-version 1 \
  --route-canary-proof-source-domain 0 \
  --route-canary-used-message-proof \
  --route-canary-raw-data-owner-matches-transaction \
  --route-canary-signature-sha256 <transaction-signature-sha256> \
  --route-canary-signature-recovered-address <0x41-prefixed-owner-address> \
  --route-canary-signature-recovers-to-owner \
  --full-toml
```

`--expected-config-hash` is required for both TOML modes and must be the value
queried from `sourceBridgeConfigHash()` or recorded from
`SourceBridgeConfigured`. `--expected-source-verifier-material-hash` and
`--expected-source-adapter-engine-deployment-hash` are required for production
source TOML, including `--toml` and `--full-toml`; both TOML modes also require
`--expected-tron-dpos-source-gate-hash` to pin the canonical DPoS source gate.
`--expected-destination-binding-hash`, `--route-allowlist-hash`, and complete
`--route-canary-transaction-*` metadata are also required for `--full-toml`.
That canary transaction metadata must include the live verifier state check,
the raw transaction owner binding, and the canonical secp256k1 signature
recovery hash/address proof.
If `--route-canary-evidence-hash` is supplied, it must match the hash derived
from that transaction metadata, including the owner, raw-owner binding, signature
hash, recovered signer, and recovery flag. The
destination binding hash must be the value queried from
`destinationBindingHash()`, emitted by a post-deploy
`emitDestinationBindingConfigured()` call, or recorded from governed
destination material. The source record hashes must match the canonical source
material and source-adapter deployment record hashes rendered by the helper.
The expected source-record checks are still optional in JSON mode, where
operators can use unpinned output for diagnostics and hash discovery, but
complete JSON summaries report `toml_ready = false` until both independent
source-record pins and the DPoS source-gate pin are supplied and matched.
JSON dry-runs also report `full_toml_ready = true` only when the source TOML
pins, destination binding pin, route allowlist hash, and route canary evidence
are all present and match, and when both source and destination runtime
bytecode preimages were supplied; destination material without the expected
binding pin is marked with `expected_destination_binding_hash_matches = false`.
The helper recomputes
the same deployment-specific binding that `SccpTronGroth16Bn254MessageVerifier`
derives from the TRON network id, source/target domains, base58 verifier
address, deployed bytecode hash, proof family, and Groth16 verifier-key hash.
The source bridge config-hash helpers are intentionally lane-scoped: Ethereum
computes only the ETH mainnet -> SORA source lane, binding EIP-155 chain id `1`,
the source bridge address, and its runtime code hash, while TRON computes only
the TRON -> SORA source lane accepted by `SccpTronSourceBridge`.
The compact JSON mode also fails closed if the destination side is retargeted
away from SORA -> TRON or a non-production proof family is supplied, so dry-run
hashes match the same lane admitted by the TVM wrapper and Rust helper. A
supplied `--route-allowlist-hash` is also treated as destination-side evidence:
compact JSON dry-runs reject route-only summaries without the paired destination
verifier material, and include the route hash only after that material is
complete.
`--source-bridge-runtime-bytecode-hex` /
`--source-bridge-runtime-bytecode-file` and
`--destination-verifier-runtime-bytecode-hex` /
`--destination-verifier-runtime-bytecode-file` can derive the corresponding
runtime bytecode hashes directly from deployed bytecode; if an explicit hash is
also supplied, the helper fails unless the derived hash matches it.
The CLI and reusable `render_toml(...)` / `render_full_toml(...)` functions run
the same derivation, so programmatic rollout tooling cannot skip the
runtime-bytecode mismatch check by bypassing argument parsing.
The helper's reusable source-config and destination-binding hash functions also
reject zero or wrong-width direct byte material before computing evidence values.
When complete source material is present, compact JSON dry-runs include the
canonical `SccpSourceVerifierMaterialV1` and
`SccpSourceAdapterEngineDeploymentV1` record hashes. Both TOML modes also emit
those two hashes as comments immediately before the records they digest, so the
rendered evidence can be compared with Rust's
`sccp_source_verifier_material_hash(...)` and
`sccp_source_adapter_engine_deployment_hash(...)` before governance submission.
The ETH, BSC, Solana, TON, and Substrate-family source evidence renderers emit
the same record hashes in compact JSON dry-runs and TOML audit comments for
their source-material and source-adapter deployment records, so governed
evidence can be compared with Rust admission digests before it is copied into
node configuration. Those renderers also accept
`--expected-source-verifier-material-hash` and
`--expected-source-adapter-engine-deployment-hash`; when supplied, any mismatch
against the canonical record digest fails before JSON or TOML is emitted.
Production TOML from those renderers requires both expected source-record pins;
JSON remains available without them for diagnostics and reports
`toml_ready = false` until the pins are supplied and match.
Substrate-family source TOML additionally requires
`--expected-runtime-storage-gate-hash`, emits that exact value as the
`sccp_substrate_runtime_storage_gate_hash` source-adapter audit comment, and
its JSON summary only reports `toml_ready = true` when that independent
runtime-storage gate pin also matches. The all-lanes preflight consumes the
commented pin rather than trusting a locally derived unstaged value, then
recomputes it from the source material and deployment records.
For ETH/BSC source evidence, the CLI and reusable `render_toml(...)` /
`_json_summary(...)` paths derive `source_bridge_emitter_code_hash` from
supplied source bridge runtime bytecode before recomputing the canonical source
material and source-adapter deployment record hashes. TOML rendered with
runtime bytecode also carries `sccp_evm_source_bridge_runtime_bytecode_hex`;
the all-lanes preflight requires this bytecode preimage for ETH/BSC source
material, requires lowercase canonical hex in staged evidence, and rejects any
staged source record whose decoded bytecode does not hash back to the live
source bridge runtime code hash and governed `source_bridge_emitter_code_hash`.
The source renderers and runtime SCCP source-material/deployment gates also
require non-zero role digests to be pairwise distinct across trust anchors,
consensus verifiers, message-inclusion verifiers, source-state verifiers,
source bridge code/config hashes, adapter verifier keys, deployment receipts,
and Solana/TON full-light-client audit verifier hashes, preventing one
governed digest from standing in for multiple verifier roles.
After collecting the per-chain source, destination, and route snippets, run the
all-lanes preflight before governance staging:

```bash
python3 scripts/sccp_all_lanes_evidence.py path/to/source-*.toml path/to/rollout-*.toml
```

The preflight merges the TOML snippets, requires one source verifier material,
one source-adapter deployment, one destination rollout, and one route allowlist
for each advertised SCCP remote domain, and exits non-zero with JSON blockers
when any lane is incomplete. It also invokes each lane's canonical source
evidence validator, rejecting non-canonical source-adapter verifier keys and
template-derived component hashes before governance staging. It also rejects
source-material, source-adapter deployment, and Solana/TON audit records that
reuse a non-zero hash across verifier roles. Destination rollout records are
also checked against each lane's canonical verifier identity format instead of
accepting arbitrary non-empty verifier strings. The aggregate check recomputes
the audited Solana and TON full-light-client gate hashes from the
source/deployment/audit fields and the TRON source bridge config hash from the
governed bridge address, network id, and owner, so arbitrary non-zero
placeholders cannot pass the preflight. Solana audit fields are only valid on
SORA-bound Solana source-adapter deployments, and TON audit fields are only
valid on SORA-bound TON source-adapter deployments; lane-foreign and
foreign-target audit fields are rejected before governance staging to match
runtime source-adapter deployment admission, and core all-lanes launch
regressions pin the same rejection path before audit gate hashes are
recomputed. Public release-bundle verification also requires each lane's
`source_adapter_gate.gate_hash` and cryptographic-evidence
`source_adapter_gate_hash` to match the named final gate transcript
(`solana_full_light_client_gate_hash`, `ton_full_light_client_gate_hash`,
`tron_dpos_source_gate_hash`, or `substrate_runtime_storage_gate_hash`),
not just any role hash in the audit bundle. It also preserves the destination
binding metadata comments emitted by the rollout helpers. EVM-family rollout
snippets carry explicit destination network id, bridge wrapper address, and
binding hash fields, so the all-lanes preflight recomputes the SORA -> ETH/BSC
binding from the governed verifier address, bridge wrapper, network id, code
hash, and Groth16 key hash. It also requires the EVM live helper's RPC chain id
and bridge/verifier runtime code-hash metadata comments plus replayable
runtime-bytecode hex comments whose Keccak-256 hashes match those code hashes,
rejecting offline/manual ETH/BSC destination TOML that lacks live bytecode
evidence. ETH/BSC source snippets likewise require the source live helper's RPC
chain id, bridge address, runtime code-hash metadata, and replayable source
bridge runtime-bytecode comment before source material can pass launch
preflight. Solana, TON,
and Substrate-family rollout snippets
carry explicit canonical destination binding key/hash fields, while TRON
rollout snippets carry the binding hash, binding key, and destination network
id derived from the governed TRON network id and destination verifier code/key
hashes. The preflight rejects a TRON destination rollout whose explicit
`destination_network_id` drifts from the governed source bridge network id even
if its destination binding hash/key still match. For Solana, the preflight also
requires the configured `solana_*` rollout fields and mirrored live-helper
comments for the ProgramData account address, ProgramData slot, Program account
preimage, immutable ProgramData metadata preimage, and ProgramData executable
BLAKE2b-256 plus base64 executable preimage. It checks that the ProgramData slot
is positive and matches the pinned expected slot while the Solana live RPC
commitment is `finalized`, the ProgramData RPC read context slot is at or after
that deployment slot, and the ProgramData executable hash equals the rollout
`verifier_code_hash` when recomputed from the configured base64 executable
preimage; offline
Solana destination TOML without that live immutable-deployment evidence remains
diagnostic and is rejected for all-lanes launch. Solana route allowlists must
also carry a canary hash recomputed from that same finalized ProgramData
transcript. Core and Torii perform the same recomputation from configured
runtime fields, so generic non-zero canary hashes are rejected before production
readiness is reported. Public release-bundle verification also rejects zero or
non-canonical Solana ProgramData addresses in the published route-canary
summary, so a non-empty placeholder address cannot pass attachment review. For
TON, the preflight requires
the live/direct helper's active account status, account-state hash, last
transaction LT/hash, code hash, code-BoC root hash, and code-BoC match metadata
comments, and checks that both the live TON code hash and BoC root hash equal
the rollout `verifier_code_hash`; offline TON destination TOML without that
account-state and BoC-root evidence remains diagnostic and is rejected for
all-lanes launch. TON route allowlists must also carry the
`ton_route_canary_*` live-account snapshot fields emitted by the direct/live
helpers. The preflight requires those fields to match the destination rollout
live account evidence and recomputes the canonical TON route canary hash from
the route hash, destination binding hash, source material/deployment record
hashes, verifier identity/code hash, active account status, account-state hash,
last transaction LT/hash, and verifier code-BoC root hash before the lane can
report production readiness. For Substrate-family lanes, the preflight requires the live
helper's finalized head, runtime `specName`, `specVersion`,
`transactionVersion`, runtime code-hash metadata, and replayable runtime-code
base64 comments. It decodes the runtime code, recomputes BLAKE2b-256, and
checks that the replayed hash equals both the live runtime code hash and the
rollout `verifier_code_hash`; offline Substrate destination TOML without that
finalized runtime evidence remains diagnostic and is rejected for all-lanes
launch. Substrate-family route allowlists must also carry a canary hash
recomputed from that same finalized runtime evidence, and public release-bundle
verification rejects zero or governed-hash-reused finalized-head/runtime-code
route-canary fields in the published readiness JSON. Missing or drifting
destination binding metadata fails the preflight before governance staging. The
route allowlist hash is also recomputed from the canonical source material
record hash, source-adapter deployment record hash, and destination binding
hash, so a stale or unrelated route policy hash cannot open a different lane
evidence tuple. If any of those component hashes cannot be recomputed from
production-shaped records, route evidence is reported as unbound instead of
being recomputed from zero placeholders. Each route allowlist record must also
carry post-deploy canary metadata in the `[[zk.sccp_route_allowlists]]` table:
`route_canary_status = "passed"`, a non-zero
`route_canary_evidence_hash`, `route_canary_route_allowlist_hash`, and
`route_canary_destination_binding_hash`. Evidence renderers keep the older
`# sccp_route_canary_*` comments beside those config keys for offline review,
but runtime readiness and the ZK policy hash consume the real `route_canary_*`
fields. The canary route hash must match the table's `route_allowlist_hash`,
and the canary destination binding hash must match the recomputed destination
binding hash. Public release-bundle verification repeats that role separation:
the canary evidence hash must also be distinct from every advertised source
material record hash, source-adapter deployment record hash, route allowlist
hash, destination binding hash, and domain-specific route-canary transcript hash
published for EVM/TRON, TON, or Substrate-family lanes, so a source, route, or
destination digest cannot be replayed as the post-deploy canary evidence. The
all-lanes preflight, core configured runtime admission gate, and Torii
configured proof APIs also require that canary evidence hashes are unique
across all advertised lanes and do not reuse another lane's source material or
source-adapter deployment record hash, and public release-bundle verification
repeats those cross-lane checks against the published all-lanes JSON. One
successful post-deploy route canary therefore cannot be replayed as proof for
another lane. The preflight is an offline operator check; it does not need
signing keys or live-chain credentials.
Ready lanes include the canonical source verifier material, source-adapter
deployment record hashes, and destination binding, route canary, and route
allowlist summaries in the JSON output for governance comparison.
The same JSON output now includes a `release_checklist` object whose items
separate required lane records, governed deployment evidence, route allowlist
binding, live route canary evidence, and any unresolved all-lanes blockers.
Release automation should gate on `release_checklist.ready == true` after
reviewing the per-item blockers, instead of scraping lane-specific error text.
For release validation, operators can run the focused production corridor from
the repository root:

```bash
bash scripts/check_sccp_production_corridor.sh
```

Use `bash scripts/check_sccp_production_corridor.sh --list` to view the
available phases, or repeat `--phase <name>` to run only selected slices such
as `evidence-scripts`, `js-sdk`, `kotlin-sdk`, `java-android`, or
`dotnet-sdk`. Add `--dry-run` to print the exact selected command plan
without resolving local Java/Android toolchains or executing heavyweight
phases; release operators should use that mode to review the corridor before a
full run. For Gradle-backed mobile phases, the runner resolves `JAVA_HOME`
from an explicit environment value, the repo-local `target/java/jdk-21` bundle,
macOS `/usr/libexec/java_home -v 21`, or Homebrew `openjdk@21`, so local
release rehearsals do not fail with an empty Java path when Apple's Java
locator is absent. The full corridor covers the Rust SCCP verifier crate, all
operator evidence script tests plus the corridor runner self-check, JavaScript
and Python portal-facing proof generation, Swift and Kotlin mobile proof
generation, the mirrored Java Android SDK checks, the native .NET/C# ETH/BSC
facade tests, the EVM/TRON Groth16 contract smoke, and core bridge-proof
admission. The `eth,bsc` public release row is blocked unless the `dotnet-sdk`
phase also passes, so the native C# BSC facade cannot be validated only by
ad-hoc local output.
`.github/workflows/sccp_production_corridor.yml` attaches the same
phase list to pull requests touching SCCP surfaces, a nightly scheduled run,
and manual `workflow_dispatch` runs for either the full corridor or one named
phase. Each phase job writes its runner transcript to
`dist/sccp-production-corridor/<phase>.log` and uploads it as the
`sccp-production-corridor-<phase>` artifact; release operators should use
those artifacts as the `--phase-evidence <phase>=<log>` inputs when rendering
strict public readiness notes. For local release rehearsal, the same transcript
layout can be produced directly with
`bash scripts/check_sccp_production_corridor.sh --log-dir
dist/sccp-production-corridor`; this runs every selected phase as its own
corridor invocation and writes strict `<phase>.log` artifacts containing the
phase marker, completion sentinel, command fragments, and success markers
expected by the release report and bundle verifier. The Java Android phase runs
the main-method SCCP classes through
`GradleHarnessTests` and runs
`SolanaSccpProverTests` directly through Gradle's JUnit selector, because the
Solana prover test is JUnit-only and no longer exposes a harness `main`.
The Swift phase now also runs
`ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions`
after the prover/source-state batch, so iOS release evidence covers the
portal/mobile path that turns user-generated EVM/TRON proof submissions into
Torii bridge-proof submit payloads.
Strict release reports also inspect each passed phase's transcript: the hashed
artifact must contain the exact `==> SCCP production corridor: <phase>` marker,
the non-dry-run `SCCP production corridor completed.` sentinel, and the
expected command fragments plus phase-specific success markers inside the
claimed phase block. This keeps `--phase-result <phase>=passed` from turning an
arbitrary hashed file, a dry-run plan, a command-only transcript, or a
transcript with commands under a different phase marker into release evidence.
The corridor runner self-check loads the same command fragment table and
compares it with full `--dry-run` phase output, so release
evidence expectations stay synced to the actual runner commands.
After collecting the final evidence bundle and validation results, operators
should render public release-readiness notes with
`python3 scripts/sccp_release_readiness_report.py --require-phase-evidence
--phase-result all=passed --phase-evidence-dir <downloaded-ci-artifacts>
<evidence.toml>`. The report stays fail-closed unless the all-lanes evidence
preflight is production-ready, every corridor phase is recorded as passing, and
each passed phase has a hashed corridor evidence artifact. The readiness
report accepts the local `--log-dir dist/sccp-production-corridor` layout,
downloaded CI artifact folders named `sccp-production-corridor-<phase>`, and
explicit repeated `--phase-result <phase>=passed --phase-evidence
<phase>=<log>` arguments when phases are run separately. Attach the generated
Markdown or JSON report to release notes so governance reviewers can inspect
the exact SCCP evidence, structured release checklist, validation state, and
production-corridor transcripts. The report includes each input evidence
file's byte length and SHA-256 digest, and in strict release mode it also lists
the byte length and SHA-256 digest of the ordinary-file corridor artifact for
each passed phase, so reviewers can match public release notes to the exact
TOML and validation artifacts that passed all-lanes validation without
following symlinks or mutable aliases. Release-bundle verification requires
those manifest and report digest fields to use canonical lowercase
64-character SHA-256 hex text, so uppercase, short, or otherwise ambiguous
digest strings cannot be published as artifact bindings. It also renders a per-lane
cryptographic evidence table with
the source verifier material hash, source adapter deployment hash, destination
binding hash, source-adapter gate hash, source-adapter gate audit hash set,
route allowlist hash, route canary evidence hash, and route canary evidence
source used for the readiness decision. For TRON lanes that table also carries
the route-canary block number and block timestamp as verifier-bound JSON fields,
while non-TRON lanes must keep those fields null. Reviewers can therefore
compare the release note against the governed hashes, the required source-gate
audit material, and the post-deploy canary transcript without digging through
the raw TOML first. Public release-bundle
verification also enforces source-adapter gate audit hash role separation, so
gate evidence cannot be replayed from source material, source deployment,
destination, route, or sibling audit digests in either embedded all-lanes view.
It also records the user-prover SDK submission
surface for every lane, separating the EVM/TRON Torii bridge-proof submit
payload path from native Solana instruction, TON BOC, and Substrate
runtime-call envelopes that portal or mobile provers submit on-chain. Each
surface row lists the required web, Python, Swift, Kotlin, and Java Android
corridor phases, the core-admission phase, plus the EVM/TRON contract-smoke
phase where contract verifier calldata is produced, so release reviewers can
see whether a user-facing proof path is validated or still blocked. The JSON
report also carries the exact
`sdk_helper_symbols` list behind the rendered helper string, and public
release-bundle verification requires the text to match that list. This keeps
the portal/mobile proof-generation surface machine-auditable instead of only
publishing prose.
For public release notes, prefer the self-contained bundle builder:
`python3 scripts/sccp_release_bundle.py --output-dir dist/sccp-release-bundle
--phase-result all=passed --phase-evidence-dir <downloaded-ci-artifacts>
<evidence.toml>`. The bundle builder copies the final TOML evidence and
downloaded corridor logs, regenerates the strict Markdown and JSON readiness
report, writes `sccp-all-lanes-summary.json`, and creates
`sccp-release-notes-attachment.md` plus `manifest.json` with byte lengths and
SHA-256 hashes for every attachment. The release-note attachment explicitly
names `manifest.json` as the verifier root, so reviewers know to publish the
manifest alongside the hashed artifacts. It exits non-zero unless the strict
report is production-ready, so missing governed deployment evidence, missing
live canary evidence, or missing phase logs cannot be accidentally published as
a ready release. If `--force` is used to replace an output directory, the
builder refuses dangerous targets and refuses any output directory that contains
the input TOML or phase transcript sources, so evidence cannot be deleted before
it is copied into the bundle. For production-ready bundles, the builder now runs
the strict verifier against its own output and prints the verified
`manifest_sha256` root before reporting success; run
`python3 scripts/sccp_verify_release_bundle.py <bundle-dir>` again after upload
or when reviewing downloaded attachments to recompute every attachment hash and
confirm that the readiness report, all-lanes summary, and manifest all agree on
`production_ready`. With `--json`, the verifier also emits `manifest_sha256` so
reviewers can archive the verified manifest root while the manifest remains
outside its own artifact table. Strict verification rejects any artifact row
that tries to list `manifest.json` as a hash-bound attachment. The manifest,
readiness-report, and
all-lanes summary JSON roots must keep the bundle builder's canonical sorted-key
serialization and reject duplicate JSON object keys before semantic review, so
duplicate-key smuggling or hand-edited formatting drift cannot be published
after attachment hashes are refreshed. The public verifier no longer exposes
release-report or release-bundle module hooks for the artifact shapes it owns;
the copied-evidence summary is recomputed through the all-lanes evidence
validator directly, while readiness Markdown, release-note attachments,
corridor phases, phase transcripts, cryptographic evidence rows, and
user-prover submission surfaces are verifier-owned. The manifest and
readiness-report JSON roots reject missing or unknown top-level fields, and
manifest/report artifact entries reject unknown fields and require artifact
`bytes`/`sha256` claims to keep canonical JSON
integer/string types, so unreviewed operator claims or malformed metadata
cannot be hidden in attachments that the canonical Markdown ignores. The
readiness
report and all-lanes summary JSON roots must be non-empty objects before any
production-ready flags are considered; manifest, report, embedded-evidence,
corridor, checklist, and checklist-item readiness flags must be real JSON
booleans; and nested readiness sections such as report `evidence`,
`release_checklist`, `corridor`, and summary `release_checklist` must also keep
their object shape while top-level `input_artifacts` must remain a list. The
verifier recomputes the all-lanes
summary by loading the all-lanes evidence validator directly against the copied
TOML evidence files before comparing it with the standalone summary and
embedded report evidence, so a published bundle cannot hide stale or tampered
evidence inputs behind unchanged JSON reports or a weakened release-bundle
builder. The
report `inputs` provenance list must also match the copied evidence artifact
paths used for recomputation, so JSON release notes cannot claim a different
operator-side evidence source after the bundle is built. The
manifest root must include the fixed readiness fields, and its
`production_ready`, `release_checklist_ready`, `corridor_ready`, and `blockers`
claims must match the readiness report and all-lanes summary instead of being
accepted as standalone release-manager assertions. The
release checklist table must match the embedded all-lanes evidence summary, so
public release notes cannot rename, omit, or reorder checklist gates while
keeping the underlying evidence unchanged; checklist roots and gate rows also
reject unknown fields, malformed gate ids/titles, and malformed blocker lists
so operator approvals cannot be hidden in ignored JSON members. The manifest
artifact set and order must
exactly match the required reports, copied evidence inputs, copied corridor
logs referenced by known passed phases in the readiness report, and final
release-notes attachment, so a hash-bound but unreviewed appendix, unknown
phase log, or regenerated artifact table cannot be smuggled into an
otherwise verified bundle. The verifier owns the production-corridor phase
inventory and requires every known phase to be marked `passed` with a
hash-bound artifact at the canonical
`corridor/<phase>.log` path, so a tampered readiness JSON cannot skip, move, or
remove one phase while leaving top-level ready flags true. The corridor section
also rejects unknown root fields and non-empty blockers, so operator
attestations or unresolved phase blockers cannot be hidden beside the phase
status and evidence maps. The verifier also owns and recomputes the exact
user-prover SDK submission surface table from the corridor phase
results, the user-side proof backend labels,
the full per-lane/per-SDK helper inventory,
and the expected on-chain submission text
(`sccp-solana-recursive-mainnet-v1`, `ton-contract-v1`,
`substrate-runtime-v1`, `evm-groth16-bn254-v1`, and
`tron-groth16-bn254-v1`), so public release notes cannot claim a portal or
mobile proof path is validated unless its required SDK and contract-smoke phases
actually passed, and a weakened report generator cannot define a shorter
helper table as canonical. The Solana destination manifest still uses `solana-program-v1`
as the target verifier backend; the release-readiness surface uses the recursive
backend id that browser and mobile provers must put in the proof request. The
surface rows also name the lane-local user proof-generation helpers: EVM/BSC
canonical receipt-proof byte/hash helpers, TRON receipt-state and
transaction-source proof helpers, Solana and TON source-state request builders,
per-role Solana and TON full-light-client audit request builders, aggregate
full-light-client audit request builders and source-state prover facades, and
the Substrate runtime-storage proof request builder. A release bundle therefore
cannot present the final
EVM/TRON submit payload, Solana instruction, TON BOC packaging, or Substrate
runtime call envelope as portal/mobile-ready while omitting the user-side native
proof-generation helpers. Those
portal/mobile submission rows
must also keep canonical JSON field shapes: lane/backend/helper/submission
labels are non-empty strings and required phases are lists of non-empty strings.
For a production release bundle, the row-level validation status must be
`passed` and `validation_blockers` must be empty, so a blocked portal/mobile
proof path cannot hide behind top-level ready flags.
The verifier also
rejects
non-directory or symlinked bundle roots, non-canonical or escaping manifest
paths, a symlinked `manifest.json`, self-listed `manifest.json` artifact rows,
symlinked artifacts, unmanifested directories, duplicate, unmanifested, or
omitted required artifacts,
non-canonical manifest/readiness-report/summary JSON serialization,
duplicate keys in public JSON roots,
non-UTF-8 public JSON and Markdown roots,
control characters in manifest, readiness-report, or extracted bundle artifact
paths,
unknown corridor phase statuses or evidence keys,
blocked corridor roots,
non-canonical corridor phase-log paths,
malformed artifact byte/hash JSON types,
malformed readiness/checklist boolean JSON types,
report/manifest byte or SHA-256 drift for input and corridor phase artifacts,
release notes that omit the manifest handoff, standalone-summary drift from the
report's embedded evidence, empty or non-object report/summary JSON roots,
malformed readiness sections, missing or empty copied input-artifact lists,
malformed or duplicate input-provenance paths, input-provenance drift from the
copied evidence artifacts, copied evidence layout drift from `evidence/NN-*.toml`,
non-canonical readiness-report artifact paths,
missing or unknown manifest/readiness-report top-level fields,
unknown embedded or standalone all-lanes summary root or lane fields,
malformed all-lanes required-domain or blocker scalar lists,
all-lanes required-domain drift from published lane domains,
all-lanes domain roster or chain-label drift from the production remote lanes,
non-ready or blocked all-lanes root or lane summaries,
missing-record lane flags,
blocked required source-adapter gates,
blocked release-checklist items,
blocked portal/mobile submission surface rows,
malformed nested all-lanes lane
record/hash/source-gate/destination-binding/route/route-canary transcript
sections,
zero governed source/destination/route hashes in public all-lanes lane
summaries,
missing or misplaced lane-specific destination binding network/bridge fields,
malformed lane-specific route-canary transcript sections,
route-canary evidence hashes that replay governed source/deployment,
destination, route, lane-specific canary hash roles, another lane's canary
evidence hash, or another lane's governed hash roles,
EVM-family route-canary zero transaction/public-input words or reused
route-canary hash roles, including finality-height replay,
TRON route-canary zero owner/recovered addresses, zero transcript words, or
zero binding hashes, reused TRON route-canary hash roles including
finality-height replay, or recovered-signer drift from the transaction owner,
Substrate route-canary zero finalized-head/runtime-code hashes or reused
Substrate route-canary hash roles,
expected destination/route hash drift,
route-canary route/destination hash drift from sibling lane evidence,
duplicate, unknown, or missing required cryptographic evidence domains,
cryptographic evidence row domain/chain drift, or per-field
source/destination/source-gate/route/canary drift from embedded lane rows,
unknown manifest or report artifact fields,
unknown corridor root fields,
unknown or malformed release-checklist fields,
unknown or malformed portal/mobile submission-surface fields,
manifest readiness-header drift from the report and summary,
report/summary drift from verifier-owned direct recomputation of the copied
evidence TOML, a Markdown
readiness report that is missing its canonical title, status line, public
evidence sections, input/corridor/checklist/crypto/user-prover/lane/blocker
values, required release-evidence markers, or verifier-owned canonical render
of the JSON readiness report,
a release-notes attachment that is not the verifier-owned canonical
manifest/report artifact table, manifest artifact-order drift from the bundle builder's public
attachment order, and missing, malformed, unbound, lane-mismatched, or extra-field
per-lane cryptographic evidence rows.
That final check recomputes the public cryptographic table from the embedded
all-lanes lane evidence, requires one row for every required production domain
with no duplicate or unknown domains, requires exact JSON domain/chain types
plus canonical non-zero bytes32 hash text, and reports field-specific failures
when a public row's source-material, source-deployment, destination-binding,
route-allowlist, source-gate required flag/hash/audit hashes, route-canary
hash/source, or route-canary binding flag differs from the lane
that actually passed preflight. For passed phases, the verifier independently
re-reads each copied corridor log and requires an exact phase-marker line,
phase-local completion sentinel, phase-block traced command fragments, and
non-command phase-specific success output as the report generator. The verifier
also owns the canonical public
Markdown renderer and parses the rendered readiness report independently of the
report generator, requiring the published sections to carry the copied evidence
paths and hashes, corridor artifact hashes, release-checklist gate statuses,
per-domain cryptographic hashes and canary metadata, every lane's portal/mobile
helper symbols and required phases, lane readiness rows, blocker summary, and
release-evidence handoff text. A weakened report renderer therefore cannot
publish a shorter or structurally incomplete reviewer-facing report even when
the JSON report still hashes correctly.
The EVM-family, Solana, TON, and Substrate-family destination renderers and
their live evidence wrappers, plus the TRON full-lane direct and live
renderers, now require this comment block for production TOML when route
allowlist records are emitted.
`--full-toml` additionally keeps the recomputed destination binding hash as a
comment next to the explicit rollout fields so the TOML evidence can be compared
with chain governance, the wrapper views, and the post-deploy canary event.
The source verifier material and source-adapter deployment hash comments are
audited the same way: they must be emitted by the corresponding source evidence
helper and match the canonical record hashes recomputed from the TOML body.

```toml
# sccp_tron_source_verifier_material_hash = "0x..."
[[zk.sccp_source_verifier_materials]]
version = 1
source_domain = 5
source_chain = "tron"
source_proof_plan = "TronDposReceiptProof"
finality_model = "TronDpos"
adapter_circuit_id = "sccp-source-adapter-v1"
source_trust_anchor_id = "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1"
source_trust_anchor_hash = "0x..."
consensus_verifier_id = "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1"
consensus_verifier_hash = "0x..."
message_inclusion_verifier_id = "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1"
message_inclusion_verifier_hash = "0x..."
source_bridge_emitter_id = "sccp:tron:source-bridge-emitter:tron-mainnet:v1"
source_bridge_emitter_address = "0x..."
source_bridge_emitter_code_hash = "0x..."
source_bridge_network_id = "0x..."
source_bridge_owner_address = "0x..."
source_bridge_config_hash = "0x..."
finality_policy_id = "sccp:tron:finality-policy:solid-block-mainnet:v1"
finality_policy_hash = "0x..."
placeholder_material = false

# sccp_tron_source_adapter_engine_deployment_hash = "0x..."
[[zk.sccp_source_adapter_engine_deployments]]
version = 1
source_domain = 5
target_domain = 0
source_chain = "tron"
source_proof_plan = "TronDposReceiptProof"
finality_model = "TronDpos"
adapter_proof_family = "stark-fri-v1"
adapter_circuit_id = "sccp-source-adapter-v1"
adapter_verifier_vk_hash = "0x..."
source_trust_anchor_id = "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1"
source_trust_anchor_hash = "0x..."
consensus_verifier_id = "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1"
consensus_verifier_hash = "0x..."
message_inclusion_verifier_id = "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1"
message_inclusion_verifier_hash = "0x..."
source_bridge_emitter_id = "sccp:tron:source-bridge-emitter:tron-mainnet:v1"
source_bridge_emitter_address = "0x..."
source_bridge_emitter_code_hash = "0x..."
source_bridge_network_id = "0x..."
source_bridge_owner_address = "0x..."
source_bridge_config_hash = "0x..."
finality_policy_id = "sccp:tron:finality-policy:solid-block-mainnet:v1"
finality_policy_hash = "0x..."
deployment_receipt_hash = "0x..."

# sccp_tron_destination_binding_hash = "0x..."
[[zk.sccp_destination_rollouts]]
version = 1
domain = 5
chain = "tron"
verifier_plan = "TronContractGroth16Bn254"
immutable_verifier_ready = true
anchors_ready = true
verifier_identity = "T..."
verifier_code_hash = "0x..."
verifier_key_hash = "0x..."
destination_network_id = "0x..."
destination_binding_key = "tron:0:5:..."
destination_binding_hash = "0x..."
anchor_id = "sccp:tron:destination-anchor:tron-mainnet:v1"
blockers = []

# sccp_route_canary_status = "passed"
# sccp_route_canary_evidence_hash = "0x..."
# sccp_route_canary_route_allowlist_hash = "0x..."
# sccp_route_canary_destination_binding_hash = "0x..."
# sccp_tron_route_canary_transaction_id = "0x..."
# sccp_tron_route_canary_transaction_owner_address = "0x41..."
# sccp_tron_route_canary_block_number = "..."
# sccp_tron_route_canary_block_timestamp = "..."
# sccp_tron_route_canary_log_index = "0"
# sccp_tron_route_canary_message_id = "0x..."
# sccp_tron_route_canary_statement_hash = "0x..."
# sccp_tron_route_canary_commitment_root = "0x..."
# sccp_tron_route_canary_used_message_proof = "true"
# sccp_tron_route_canary_raw_data_owner_matches_transaction = "true"
# sccp_tron_route_canary_signature_sha256 = "0x..."
# sccp_tron_route_canary_signature_recovered_address = "0x41..."
# sccp_tron_route_canary_signature_recovers_to_owner = "true"
[[zk.sccp_route_allowlists]]
version = 1
domain = 5
chain = "tron"
activation_policy = "GovernanceAllowlist"
route_allowlist_id = "sccp:tron:route-allowlist:tron-mainnet:v1"
route_allowlist_hash = "0x..."
route_canary_status = "passed"
route_canary_evidence_hash = "0x..."
route_canary_route_allowlist_hash = "0x..."
route_canary_destination_binding_hash = "0x..."
tron_route_canary_transaction_id = "0x..."
tron_route_canary_transaction_owner_address = "0x41..."
tron_route_canary_block_number = 123456
tron_route_canary_block_timestamp = 1234567890
tron_route_canary_log_index = 0
tron_route_canary_message_id = "0x..."
tron_route_canary_statement_hash = "0x..."
tron_route_canary_commitment_root = "0x..."
tron_route_canary_used_message_proof = true
tron_route_canary_raw_data_owner_matches_transaction = true
tron_route_canary_signature_sha256 = "0x..."
tron_route_canary_signature_recovered_address = "0x41..."
tron_route_canary_signature_recovers_to_owner = true
routes_allowlisted = true
blockers = []
```

For TRON -> SORA, `scripts/sccp_tron_source_bridge_evidence.py` derives
`adapter_verifier_vk_hash` from the canonical `fastpq-lane-balanced`
source-adapter verifier profile. If an operator supplies
`--adapter-verifier-vk-hash`, the helper treats it as an audit check and fails
unless it matches the canonical value before rendering production TOML.
Full rollout TOML also requires transaction-derived route-canary evidence from
a successful `MessageProofAccepted` submission and the matching
`usedMessageProofs(messageId)` live-state check. The derived hash also commits
the TRON transaction owner and signature recovery evidence; a route hash alone
remains diagnostic and must not be staged as production-ready governance. Direct
and live full-TOML rendering now also carry the route-canary transaction block
number and block timestamp in both audit comments and structured route
allowlist fields, and the live replay path forwards those values back through
the offline renderer so a saved summary cannot drop the block metadata required
by all-lanes readiness.

Bridge admission may rebuild the source-bound submission package while an
outbound destination manifest is still disabled, but only after configured
source material, matching source-adapter deployment material, destination
rollout material, and route allowlist material are present; outbound
submission-package builders still keep the disabled destination manifest gate
closed.
The on-chain admission verifier uses a narrower production path for this case:
it may relax only the destination manifest readiness bit needed for
deployment-governed lanes, while non-SORA source proofs must still verify
against production-ready source-adapter material and deployment evidence.

Readiness reporting separates this material gate from the external verifier
engine gate. `source_verifier_material_ready` can become true for exact
deployment material, but `external_consensus_verifier_ready`,
`external_message_inclusion_verifier_ready`, and `source_trust_anchor_ready`
become true only when the matching `SccpSourceAdapterEngineDeploymentV1`
record is configured. Deployed-looking source material alone never marks a
source adapter production-ready.
Operator tooling can evaluate all local lane gates together with
`sccp_lane_production_readiness_with_deployment_materials_for_domain(...)`.
That helper accepts exact source verifier material, matching source-adapter
deployment evidence, destination rollout material, and route allowlist material,
then returns the same `SccpLaneProductionReadinessV1` shape used by the default
readiness surface. Cross-domain replay of any one component, or a
retargeted source-adapter deployment, leaves the lane non-production and
records the failing gate in `blockers`. Evidence-bound route hashes are required
on this deployment-material path; if the canonical route hash cannot be derived
from production-ready, mutually bound lane records, readiness reports the route
evidence as unbound instead of treating a standalone route profile as enough.
TRON source-adapter deployment builders fail even earlier for non-SORA targets,
matching the TRON -> SORA-only TVM source bridge contract.

Solana proof generation is a user-side workflow. Web portal and mobile SDKs
construct a canonical local proof request from UI/RPC-collected witness fields
(`finalized_slot`, `blockhash`, `bank_hash`, `transaction_status_root`,
`message_proof_hash`, Solana finality context fields, transaction signature,
emitter program id, `message_id`, `payload_hash`, `commitment_root`, and
`source_event_digest`). SDK helpers derive `message_proof_hash` from
`blake2b256("sccp:solana:message-proof:v1" || 0x01 ||
source_event_digest || transaction_status_root || sig_len_le ||
raw_transaction_signature || program_len_le || raw_emitter_program_id ||
branch_len_le || inclusion_branch[0..n])`, where the UI-provided Solana
transaction signature and emitter program id are decoded from base58 into fixed
64-byte and 32-byte raw values before hashing. The inclusion branch is required
to be non-empty, and the Rust source adapter independently derives the
transaction-status/message root from that branch before accepting the adapter
proof, so portal/mobile provers must supply the branch that actually opens the
SCCP source event. JavaScript and Python also reject duplicate UI/RPC aliases
for the Solana source-event digest, transaction-status root, transaction
signature, emitter program id, and inclusion branch before deriving the
message-proof hash, transaction-status leaf, or branch root, so a portal cannot
display one value while hashing another. JavaScript, Python, Swift, Kotlin, and
Java Android production prover wrappers reject a missing, empty, or oversized
branch before invoking the linked prover or wrapping externally generated proof
bytes. Solana blockhash inputs may arrive as base58 or hex, but SDK
normalization stores the canonical `0x`-prefixed 32-byte value and hashes the
raw 32 bytes in the local witness transcript so alternate textual encodings
cannot produce divergent proof requests. The helpers hash the canonical
witness and expose the matching epoch-stake-root, stake-activation,
stake-account-state, account-opening, vote-account-data, stake-account-data,
StakeHistory, StakeHistory-sysvar-data, account-raw-data, account-inclusion
leaf/node/root, Tower-lockout, Tower-replay, and bank-fork transcript helpers
for finality-context witnesses. The JavaScript and Python active-stake and
stake-history helpers reject duplicate validator-roster and stake-activation
epoch aliases before hashing, so UI-collected validator identities and weights
cannot be rebound by an alternate field spelling. The JavaScript and Python
account-opening, AccountsLtHash opening-normalization, and account-inclusion
leaf helpers likewise reject duplicate account-address, owner-program,
rent-epoch, data-hash, finalized-slot, opening-object, raw-data, raw-data-hash,
and nested opening-address aliases. If both raw account data and a raw-data hash
are present, they recompute and require equality before deriving the
account-inclusion transcript. The opened-AccountsLtHash contribution and
opened-account inclusion witness helpers also reject duplicate opened-account
array, StakeHistory sysvar, account-inclusion root, AccountsLtHash checksum,
and full AccountsLtHash aliases; the Agave bank-hash helper rejects duplicate
parent-bank, signature-count, blockhash, full-AccountsLtHash, and hard-fork data
aliases before deriving bank-state transcripts. The Tower lockout/replay,
bank-fork, and AccountsLtHash public-input helpers now apply the same guard to
finalized slots, epochs, rooted/parent slots, parent-bank hashes, bank hashes,
bank-fork hashes, Tower vote slots, transaction-status roots, account-inclusion
roots, AccountsLtHash checksum/root fields, full AccountsLtHash bytes, and
hard-fork data before deriving recursive proof public inputs. The stake-activation helper
binds the active vote roster to activation/deactivation epochs at the signed
epoch; the stake-account-state helper binds those entries to vote/stake account
openings; the account-opening helper hashes the account address, owner program
id, lamports, rent epoch, executable flag, and account-data hash under
`sccp:solana:account-opening:v1`; the vote-account-data helper hashes the
semantic vote-account transcript under `sccp:solana:vote-account-data:v1` and
can parse raw `VoteStateVersions::V1_14_11`/`V3`/`V4` account bytes into that
transcript for portal/mobile provers, deriving V4-default collector and
commission fields for legacy variants and binding V4 collector, basis-point,
pending-reward, and optional BLS-key fields directly when present, while also
validating the prior-voter, bounded epoch-credit, bounded last-timestamp, and
zero-padding suffix layout; the
stake-account-data helper hashes the semantic stake-account transcript under
`sccp:solana:stake-account-data:v1` and can parse raw
`StakeStateV2::Stake` account bytes into that transcript for portal/mobile
provers, binding the exact 8-byte legacy/reserved warmup-cooldown-rate layout
provers, accepting only Solana's known legacy/current little-endian `f64`
encodings (`0.25` or `0.09`) for that slot, and binding the `StakeFlags` byte
while rejecting reserved flag bits; the
StakeHistory-sysvar-data helper hashes
Solana's bincode vector sysvar account-data layout under
`sccp:solana:stake-history-sysvar-data:v1`, with the witness accepted in sorted
ascending order but serialized newest-first for the account-data hash; the raw
StakeHistory-sysvar-data helper validates that bincode vector framing and
newest-first epoch order before hashing exact raw sysvar bytes for portal/mobile
provers; and the StakeHistory
helper binds
effective voting stake, delegated stake, and the signed-epoch StakeHistory
sysvar window. The
StakeHistory helper also replays the Tower-era 900 bps warmup/cooldown schedule
over the supplied bounded window and rejects effective stakes that do not match
the replayed status, or whose signed-epoch effective total does not exactly
match the replayed active roster. JavaScript and Python helpers can derive the
exact Agave account AccountsLtHash contribution and mix/checksum a complete
witness set; all SDKs expose the Agave finalized-bank hash helper and require
`bankSignatureCount` plus `accountsLtHashChecksum` in the bank-fork transcript
so portal and app proof requests cannot omit that bank-root binding.
The Tower replay helpers now also require the bank-fork hash and include it in
the `sccp:solana:tower-replay:v1` transcript, matching the Rust verifier's
direct binding between rooted Tower evidence and the finalized bank-state
statement. The Rust-compatible replay transcript always commits to the
32-confirmation Tower lockout depth, even though the explicit post-root active
vote stack contains 31 votes. Cross-SDK golden-vector tests in JavaScript,
Python, Swift, Kotlin, and Java Android pin that finality-context column plus
the Tower replay, full AccountsDB lattice, and bank/fork-choice audit statement
hashes so portal and mobile provers cannot drift from on-chain admission.
They also derive `accountsLtHashProofPublicInputsHash` from the canonical
recursive AccountsDB proof public-input transcript and include it in the local
proof request's public inputs, while the full AccountsDB audit statement binds
`accountsLtHashProofHash`, the hash of the completed nested proof capsule. The
Solana request and public-input surfaces also
carry the explicit `mainnet_genesis_hash` column, derived as BLAKE2b-256 over
`sccp:solana:mainnet-genesis:v1 || 5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp`, plus
`sourceStateVerifierId`/`source_state_verifier_id` and
`sourceStateVerifierHash`/`source_state_verifier_hash`, defaulting to the mainnet
AccountsDB verifier profile with a zero hash for diagnostic fixtures and
rejecting non-zero hashes bound to any other verifier id. JavaScript and Python
additionally verify the checksum against raw `accountsLtHash` when it is
supplied, while Swift, Kotlin, and Java Android
verify `bankHash` against raw `accountsLtHash` without adding a mobile BLAKE3
runtime dependency. These helpers are not substitutes for proving the full
Solana bank AccountsLtHash from all AccountsDB entries, full Solana
bank-state/fork-choice verification, or recursive verifier integration.
They wrap
externally generated proof bytes, but they do not fabricate proofs or infer the
source event digest. JavaScript, Python, and Swift now expose explicit
proof-result wrappers for Solana, TON, EVM-family, TRON, and
Substrate-family requests, while Kotlin and Java Android expose the same flow
through their public `wrapProofResult` helpers. The direct wrappers bind
externally generated UI prover bytes to the canonical request before deriving
the envelope hash, enforce non-zero proof bytes, and enforce the 384-byte
BN254 Groth16 ABI proof length for EVM-family and TRON lanes. The Solana
wrappers additionally rebuild the request from its witness/context before
accepting proof bytes so mutated UI request objects cannot be packaged.
Applications must link the real local prover and submit the resulting proof
artifact on-chain. Solana proof requests now also require a
proof context containing the canonical SCCP statement hash and destination
binding hash; SDKs expose a `proof_context_hash` and include it in wrapped proof
results so UI-linked provers bind the same deployment context consumed by the
Solana program submission package. The proof request public inputs expose the
adapter-bound `bank_hash`, `transaction_status_root`, `message_proof_hash`,
`statement_hash`, and `destination_binding_hash` so portal/mobile submissions
can assemble the same source-adapter and relay-context fields the on-chain
verifier checks. The same Solana SDK request surfaces also carry a
source-adapter deployment binding for UI/mobile provers:
`blake2b256("sccp:source-adapter-deployment-binding:v1" || version ||
source_domain || target_domain || source_adapter_deployment_hash ||
source_adapter_deployment_receipt_hash)`, with integer fields encoded in
little-endian order. Both deployment fields default to zero for diagnostic
fixtures, exactly-one-zero bindings are rejected, and production portal/mobile
flows must pass the configured deployment hash and deployment receipt hash.
This deployment-binding hash is part of the SDK proof request public inputs and
wrapped proof result envelope; the Solana verifier-program `proof_context_hash`
remains derived only from the canonical SCCP statement hash and destination
binding hash so the existing submission argument layout stays unchanged. When
production material supplies a deployed AccountsDB verifier hash, the user-side
prover must also include the nested `accounts_lt_hash_proof` source-state capsule
in the submitted Solana adapter proof; the Rust verifier rejects missing, empty,
malformed, wrong-circuit, or wrong-verifier-key capsules. The JavaScript,
Python, Swift, Kotlin, and Java Android SDKs expose matching AccountsLtHash
source-state request builders that derive the exact statement bytes, account
commitment bytes, verification-context bytes, OpenVerify schema descriptor,
public-input columns, and FastPQ transition payloads from the same
UI/RPC-collected witness before the app-linked prover runs. The SDK descriptor
builders mirror Rust by embedding the AccountsDB verifier id and verifier hash
in the schema descriptor, so an app-linked prover receives the same governed
source-state verifier binding that will be checked on-chain. The JavaScript,
Python, Swift, Kotlin, and Java Android SDKs also build the Solana `borsh_instruction_v1`
verifier instruction data in the
same argument order advertised by the Rust submission template: proof bytes,
canonical transparent public inputs, SCCP bundle bytes, statement hash,
destination binding hash, and proof-context hash. SDK submission builders
derive the proof-context hash from the statement and destination binding, reject
a caller-supplied hash that does not match, and the dynamic JS/Python surfaces
reject caller-supplied public-input bytes that do not equal the canonical
structured public inputs. All JavaScript, Python, Swift, Kotlin, and Java
Android Solana submission builders require both those transparent public inputs
and a wrapped SDK `proofResult` explicitly; `proofResult.publicInputs` are
Solana source-proof inputs and are not accepted as a substitute. The same
builders require those transparent public inputs to target Solana and require
the destination-binding hash to match the
canonical Solana destination binding, matching the Rust SORA -> Solana
destination submission template. Portal and mobile code can hand wallet/RPC
layers canonical instruction bytes instead of rebuilding the envelope ad hoc.
The submission builder or proof-result convenience constructor also requires
the wrapped-result backend,
non-zero witness hash, non-zero proof-context hash, recomputed non-zero envelope
hash, source-adapter deployment-binding hash, source-state verifier id/hash,
submitted proof bytes, and source-proof statement/destination binding to match
the submission context. Swift, Kotlin, and Java Android also reject zero
statement or destination-binding hashes in the typed proof-context helpers, so
mobile apps use the same non-zero proof-context boundary as JavaScript and
Python.
JavaScript, Python, Swift, Kotlin, and Java Android also compare the explicit
transparent public inputs submitted on-chain with the proof-result source public
inputs (`message_id`, `payload_hash`, `commitment_root`, finalized slot, and
bank hash), so portal and mobile UI code cannot pair a valid wrapped proof with
a different transparent SCCP message envelope or replace the wrapped proof bytes
after proof generation.
Those same wrapped-result guards now reject non-`v1` proof-result,
proof-context, source-adapter deployment-binding, and Solana transparent-public
input versions; stale or tampered `proofBase64` aliases; non-adjacent
parent/finalized slots; zero bank signature counts; and zero parent-bank,
blockhash, bank, transaction-status, message-proof, account-inclusion,
AccountsLtHash, nested AccountsLtHash-public-input, or source-event digest
fields before instruction data is built.
JavaScript returns those submission byte fields through defensive-copy getters
and freezes the submission object before it reaches wallet/RPC code. The Python
helper now builds the same `ton_message_body_boc_v1` internal-message body,
canonical witness, message-proof hash, proof-context hash, wrapped proof result,
and submission-envelope layout as read-only dict/list-compatible envelopes so
operator and portal backends can share fixtures with the web SDK without
mutating the derived submission context. The JavaScript and Python Torii
clients preserve the same
Solana submission fields when decoding typed SCCP artifact/job responses, so
the web portal and operator tooling can inspect `destination_binding`,
`destination_binding_hash`, `statement_hash`, and `proof_context_hash` before
handing canonical instruction bytes to a wallet. The Rust source-adapter
verifier recomputes the same
Solana message-proof hash and rejects any adapter proof whose hash does not bind
to the source event digest, transaction-status root, and inclusion branch
carried by the envelope. It also binds the signed vote hash to the Solana
finality context, including the Tower lockout, Tower replay stack, active stake
root, stake-activation, stake account state, account-inclusion root,
AccountsLtHash checksum, and bank-fork transcripts, verifies the embedded
finalized-slot vote certificate against the configured validator roster hash,
and rejects malformed context, tampered signatures, under-quorum stake,
mismatched trust anchors, or replayed
vote-message hashes before source-adapter evidence is accepted. The finality
context epoch is checked against Solana mainnet-beta slot arithmetic before the
signed vote certificate is accepted.
Offline Solana source-state evidence rendering is also fail-closed for direct
tooling callers, not only the CLI parser: the material record hash,
source-adapter deployment record hash, and full-light-client gate hash helpers
reject zero source trust-anchor, consensus, message-inclusion, source-state,
finality-policy, adapter verifier-key, deployment receipt, Tower replay,
AccountsDB lattice, and bank/fork-choice hashes before deriving governed
records.

TON proof generation follows the same user-side model. The JavaScript, Swift,
Kotlin, and Java Android SDK surfaces build the canonical TON public inputs from
UI/RPC-collected masterchain and shard witness fields, derive a stable
`query_id`, and package the externally generated proof as a TON message body
BOC; the Python portal/backend helper mirrors the same proof request and wrapped
proof-result binding. TON proof-request builders are locked to
`sourceDomain = TON` and `backend = ton-contract-v1` so portal/mobile code
cannot accidentally bind a TON local prover request to another source lane,
empty SCCP bundle, or non-production verifier backend. Their source-adapter
deployment binding is additionally locked to the governed TON -> SORA source
lane, and JavaScript/Python reject nested deployment-binding input that tries to
declare a different target domain instead of silently overriding it. The same SDK surfaces
also reject empty SCCP bundles when building the TON message-body BOC directly,
so wallet/liteserver submission packaging follows the same preflight. TON
message-body BOC construction also fails closed once the generated public-input,
proof, bundle, and metadata snake-cell graph would exceed the bounded 4096-cell
TON BoC cap used by the local parser and root-hash helpers. The same SDK
surfaces now also require TON transparent public inputs to target the TON domain
before request hashing or BOC packaging, and proof-result based submission
builders recheck the wrapped result's shard-state verifier id,
non-template verifier hash, and canonical TON -> SORA deployment binding hash
before accepting the request-bound envelope hash. Wrapped TON proof results now
also carry the original SCCP bundle bytes and source-proof bytes; proof-result
based submission builders rebuild the canonical proof request and reject any
result whose request hash no longer matches those bytes. A manually assembled
TON proof result therefore cannot carry stale governed source-state,
deployment metadata, or a swapped SCCP bundle into a user wallet message even
when the proof bytes and envelope hash are otherwise structurally well formed.
The TON app-linked prover and proof-result/message-body paths also preserve
omitted source proof bytes for UI-generated proofs while rejecting non-empty
all-zero placeholders and over-2 MiB source proof payloads before producing a
submit-ready wallet payload. Python
TON submission packaging now also resolves proof-result statement and
destination-binding hashes with presence-aware fallbacks, so explicit falsey
values cannot be ignored in favor of nested proof context fields, and its local
BOC cell encoder rejects falsey non-byte cell data or non-sequence refs instead
of treating them as empty cells. JavaScript and Python TON submission metadata
canonicalizers require an explicit V1 SORA -> TON manifest, `stark-fri-v1`
proof family, `ton-contract-v1` backend, TON-targeted transparent public
inputs, and a manifest-consistent destination binding before deriving wallet
metadata; BOC builders also require the root `destinationBindingHash` to match
that metadata binding. Swift, Kotlin/JVM, and Java Android expose matching
typed metadata canonicalizers for mobile wallet packaging, and the TypeScript
manifest declaration exposes the required V1 field to portal callers. The same
bundle/source-proof binding applies to EVM-family and TRON wrapped Groth16
contract-call submissions in the web, Python, Swift, Kotlin, and Java Android
SDKs, and Substrate-family proof results expose the original request bytes for
runtime-proof chaining.
These SDK surfaces expose the canonical
`shard_proof_hash`, validator-set hash, validator-set transition-message hash,
validator-set transition-signature hash, masterchain config leaf hash,
masterchain config proof hash, masterchain block-message hash, and masterchain
validator-signature hash transcript helpers so
portal/mobile provers derive those hashes from `source_event_digest`,
masterchain seqno, masterchain workchain/shard ids, masterchain root/file
hashes, shard block hash, shard state root, transaction root, shard-state leaf
indices, shard-state inclusion branches, shard workchain id, shard id, shard
seqno, shard file hash, message
inclusion branches, Ed25519 validator keys, validator weights, validator-set
seqno ranges, canonical next validator-set payloads, transition config hashes,
config roots, TON config parameter `34` as the config leaf index, config
dictionary proof BoCs, opened config value hashes, signer bitmaps, and 64-byte
validator signatures rather than supplying opaque placeholders. TON
validator-set payload and signature helpers are resource-bounded to at most
1024 validators, and source-adapter validation caps ordered validator-set
transition chains plus shard-state/config source Merkle branches at 64 entries
before evidence hashing. SDK proof builders enforce the same branch cap before
serializing portal/mobile witness transcripts. The config-proof helpers now
require a bounded TON `HashmapE 32 ^Cell` proof BoC that opens config parameter
`34`, bind the 32-bit key width and opened value hash into the transcript,
decode the opened `ValidatorSet` cell into SCCP's canonical payload, and
require the decoded payload hash to match the supplied validator-set payload hash; the legacy
abstract config inclusion branch must be empty for SDK-generated proofs. The
TON validator-set helpers also reject all-zero Ed25519 validator keys on both
structured input and raw validator-set payloads, and TON signature-proof helpers
require the signer bitmap, signature count, claimed total/signed weights, and
strict `> 2/3` signed-weight threshold to agree before serializing a transcript.
Transition-signature helpers additionally require the outer parent validator-set
hash and transition-message hash to match the validator proof and transition
fields.
TON shard-proof transcripts can optionally bind a bounded selected-account
opening by including a `ShardStateUnsplit` proof BoC, the `ShardAccounts` root,
selected key bit length, canonical account key bytes, and dictionary proof BoC
before the shard-state Merkle branch. The selected `ShardAccounts` key must be
the 256-bit TON account id; short test keys and arbitrary dictionary widths are
rejected before transcript serialization. The Rust source adapter parses the
`ShardStateUnsplit` proof, requires the proven state root to match the submitted
shard-state root, validates the embedded `ShardIdent` constructor and its
`shard_pfx_bits <= 60` bound, requires `global_id = -239` for TON mainnet,
requires `workchain_id = 0` for the TON basechain source-bridge account,
decodes the shard-state `seq_no`, `gen_utime`, `gen_lt`, and
`min_ref_mc_seqno` metadata, rejects zero sequence/generation/logical-time
placeholders, rejects MasterChain-only `custom:(Maybe ^McStateExtra)` refs on
basechain shard states, and requires `min_ref_mc_seqno` to be no greater than
the signed masterchain seqno,
extracts the `accounts:^ShardAccounts` reference hash, requires it to match the
submitted `ShardAccounts` root, requires the selected 256-bit account key
prefix to match the proven `ShardIdent` shard prefix, then opens the selected
dictionary account, skips `DepthBalanceInfo`, parses the selected
`ShardAccount`, and requires `last_trans_hash` and `last_trans_lt` to equal the
submitted transaction root and `transaction_lt`. When this account opening is
present, the legacy shard-state Merkle branch must be empty; the
`ShardStateUnsplit.accounts` proof is the only accepted shard-state binding for
that transcript shape.
This lets web portal and mobile provers submit on-chain proofs that bind the
transaction/message root to user-side TON dictionary proof material rather than
only to an opaque transcript hash. The JavaScript, Python, Swift, Kotlin, and
Java Android SDKs expose matching shard-state proof-root and
`accounts:^ShardAccounts` root helpers so UI provers can preflight the same
bounded `ShardStateUnsplit` proof material before submitting it on-chain. The
SDK shard-proof builders also fail closed when dictionary-backed inputs carry a
non-empty shard-state Merkle branch, the shard-state proof root does not match
`shardStateRoot`, the selected dictionary key is not exactly 256 bits, the
extracted accounts root does not match `shardStateDictionaryRoot`, the proof
does not carry TON mainnet `global_id = -239`, the proven `ShardIdent` is not
the TON basechain `workchain_id = 0`, the selected key prefix does not match
the proven `ShardIdent` shard prefix, the proven shard id or shard-state
`seq_no` does not match the explicit shard BlockIdExt fields, the shard-state
`seq_no`, `gen_utime`, or `gen_lt` metadata is zero, `min_ref_mc_seqno` is
ahead of `masterchainSeqno`, the basechain `ShardStateUnsplit` carries a
MasterChain-only `custom` ref, or the selected `ShardAccount.last_trans_hash`
or `ShardAccount.last_trans_lt` does not match `transactionRoot` and
`transactionLt`. When TON source material advertises a deployed source-state
verifier hash, the Rust verifier also requires an embedded
`shard_state_verification_proof` OpenVerify/FastPQ capsule using circuit id
`sccp-ton-shard-state-light-client-v1`. That capsule binds the masterchain
`BlockIdExt`, active validator/config proof, shard block/state root,
`ShardStateUnsplit.accounts` root, selected `ShardAccount.last_trans_hash` and
`last_trans_lt`, bounded shard-state/config BoCs, and validator-set transition
chain identity. Missing, empty, wrong-circuit, wrong-backend,
wrong-verifier-key, wrong-schema, auxiliary-data, public-input-column, or
backend-proof-byte tampered capsules are rejected before the TON adapter proof
is accepted, and source-state proof family/circuit labels are bounded before
adapter transcript hashing. Deployment-backed TON source-adapter readiness can now open only
when the source verifier material and source-adapter deployment both bind the
same non-template shard-state verifier id/hash; the capsule is the proof input
boundary that makes that deployed verifier mandatory rather than advisory.
The Rust SCCP crate and JavaScript, Python, Swift, Kotlin, and Java Android
SDKs now build this shard-state source-state proof request directly from
UI/mobile witness material, exposing the canonical public-input statement
bytes, witness commitment bytes, verification-context bytes, OpenVerify schema
descriptor, public-input columns, and FastPQ transition payloads with the same
`sccp-ton-shard-state-light-client-v1` circuit id and
`fastpq-lane-balanced` parameter set that the Rust verifier accepts. The SDKs
require the completed `shardStateVerificationProof` /
`shard_state_verification_proof` capsule when constructing TON full-light audit
requests; `shardStateVerificationProofHash` remains only a consistency echo and
cannot replace UI-provided proof material. The SDKs
recheck those transition payloads during proof wrapping, so a web portal,
backend, or mobile app cannot mutate a TON shard-state or audit-role transition
after request construction and still produce a source-state capsule. The
JavaScript and Python SDKs expose explicit TON source-state capsule
canonicalizers for the completed proof, so portal code no longer has to route
TON shard-state proof capsules through the Solana AccountsLtHash helper.
JavaScript TON shard-state and full-light audit request builders share the same
frozen FastPQ request shape and defensive-copy byte getters as Solana, so
browser proof engines cannot mutate canonical statement, witness, context, or
schema bytes after request construction. Python exposes the same TON request
metadata as read-only mapping/list envelopes with immutable byte payloads. The
shared portal/mobile vector uses the production-consistent masterchain config
proof hash for the same `ShardStateUnsplit` root, so fixture drift between the
shard-state opening and the config-proof transcript is caught before proof
generation.
TON BoC material can also be locally root-hashed by the Rust verifier
helpers and JavaScript, Python, Swift, Kotlin, and Java Android SDKs: the helpers parse
bounded complete BoCs, reject unsupported header flags, verify CRC32C trailers
when present, reject explicit hash descriptors, unsupported exotic cell types,
non-forward references, malformed descriptors, and invalid partial-byte cell
padding, and support ordinary cells plus pruned-branch, Merkle-proof, and
Merkle-update exotic cells before deriving the canonical SHA-256 cell root hash
used by TON proof material. The pruned-branch parser accepts both the
mask-bearing form and the legacy 280-bit maskless form emitted by existing TON
proof tooling, treating the latter as level-mask `1`. The Rust verifier helper
and JavaScript, Python, Swift, Kotlin, and Java Android SDKs also expose a
bounded `HashmapE n ^Cell` inclusion helper that decodes TON short, long, and
same-bit dictionary labels, follows Merkle-proof-wrapped selected paths, permits
pruned siblings only off the selected path, and returns the referenced value
cell representation hash for UI proof generation.
For shard-state account openings, the same SDKs expose a bounded
`ShardAccounts` helper that reuses the dictionary path, requires a 256-bit
account key, and parses the selected account value instead of accepting a
generic value-cell hash.
This helper remains scoped to the bounded `ShardStateUnsplit.accounts` path
needed for selected `ShardAccount` openings. Production source admission for a
caller-supplied TON deployment requires matching source verifier material,
adapter verifier commitment, deployment receipt hashes, and a deployment-bound
proof carrying the source-state capsule; the default built-in catalog remains
closed until real deployment material is configured through governance.
TON proof requests also carry the SCCP
statement hash, destination binding hash, and the same canonical
source-adapter deployment binding used by Solana:
`blake2b256("sccp:source-adapter-deployment-binding:v1" || version ||
source_domain || target_domain || source_adapter_deployment_hash ||
source_adapter_deployment_receipt_hash)`. The request hash commits to the
canonical public inputs, bundle bytes, source proof bytes, source-state verifier
id, source-state verifier hash, statement hash, destination binding hash, and
deployment-binding hash, and wrapped proof results expose the same source-state
verifier fields plus the request bundle/source-proof bytes before adding an
envelope hash over the request hash, deployment-binding hash, and proof bytes.
TON proof-result submission helpers rebuild this request hash before BOC
packaging, so wallet/liteserver callers cannot pair proof bytes with a
different SCCP bundle after the local prover returns. TON proof-request builders require
`sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1` with a
non-zero source-state verifier hash before local prover invocation. They also
reject zero/zero deployment bindings so UI/mobile provers cannot produce
deployment-agnostic proof bytes; the lower-level source-adapter binding
normalizer still accepts zero/zero only for diagnostic fixtures.
The TON source-adapter deployment binding inside proof requests is always
`source_domain = TON`, `target_domain = SORA`; the request-level `targetDomain`
continues to mirror the transparent public inputs used for TON destination
submission. Exactly-one-zero bindings are rejected. The TON
submission template is
`ton_message_body_boc_v1` and has one argument, `message_body_boc`, encoded as
`ton_boc`. The BOC root cell stores the SCCP operation code, schema version,
query id, destination binding hash, statement hash, proof hash, public-input
hash, bundle hash, and three snake-cell references containing proof bytes,
public inputs, and the SCCP bundle. JavaScript and Python clients decode the
production `ton_internal_message` platform payload as `message_body_boc`,
`query_id`, destination binding fields, proof bytes, public-input bytes, bundle
bytes, and statement hash. JavaScript, Python, Swift, Kotlin, and Java Android
submission builders expose the same `ton_message_body_boc_v1` envelope metadata:
version `1`, `internal_message`, `op::submit_sccp_message_proof`, one
`message_body_boc` / `ton_boc` argument, and the envelope bytes/hex. Swift,
Kotlin, and Java Android additionally provide a direct proof-result to
message-body input constructor, matching Solana's local-first flow and avoiding
manual proof-context field copying in mobile apps. Apps must link the real TON
prover and send the resulting internal message body to the configured verifier
contract; nodes do not synthesize TON proofs or destination messages.
On the source-adapter side, TON envelopes now also carry a masterchain
validator-signature certificate and optional validator-set transition proofs.
The verifier derives the ordered validator-set hash from non-zero 32-byte
Ed25519 validator keys and non-zero weights, requires that hash to match the
configured trust anchor for non-placeholder material or be derived from it
through a valid transition chain, recomputes the masterchain block-message hash
over the signed masterchain BlockIdExt fields, validator-set hash,
masterchain config root, masterchain config proof hash, authenticated basechain
shard BlockIdExt fields, shard state root, transaction root, and shard proof
hash, validates the config proof for TON config parameter `34` and the
signature-capsule hashes, opens the
message root into the submitted shard state root through the shard-state
inclusion branch, verifies Ed25519 validator signatures, and enforces strict
`> 2/3` signed validator weight. Validator-set transition messages similarly
bind the masterchain workchain, masterchain shard, block hash, and file hash
instead of accepting deployment-generic transition payloads.
The TON source-adapter gate is deployment-backed rather than enabled by the
placeholder catalog. Full lane readiness still also depends on destination
rollout material, route allowlists, governed deployment evidence for the live
verifier contracts, and the configured `ton_full_light_client_gate_hash`
matching the derived audit bundle for the TON -> SORA source deployment.
After collecting the live TON mainnet source trust-anchor hash, masterchain
consensus verifier hash, shard-message-inclusion verifier hash, shard-state
source verifier hash, finality-policy hash, TON full-light-client audit
verifier hashes, adapter verifier key hash, and deployment receipt hash,
operators can render the governed source material and source-adapter deployment
records with:

```bash
python3 scripts/sccp_ton_source_state_evidence.py \
  --source-trust-anchor-hash <masterchain-trust-anchor-hash> \
  --consensus-verifier-hash <masterchain-consensus-verifier-hash> \
  --message-inclusion-verifier-hash <shard-transaction-inclusion-verifier-hash> \
  --source-state-verifier-hash <shard-state-light-client-verifier-hash> \
  --finality-policy-hash <masterchain-finality-policy-hash> \
  --masterchain-config-verifier-hash <config-34-verifier-hash> \
  --validator-set-transition-verifier-hash <validator-set-transition-verifier-hash> \
  --shard-accounts-dictionary-verifier-hash <shard-accounts-dictionary-verifier-hash> \
  --adapter-verifier-vk-hash <source-adapter-openverify-vk-hash> \
  --deployment-receipt-hash <source-adapter-deployment-receipt-hash> \
  --toml
```

The helper is intentionally limited to the production TON -> SORA source lane
(`source_domain = 4`, `target_domain = 0`) and rejects boolean or non-`u32`
programmatic domain values, zero hashes, and wrong-width hashes before
rendering TOML. The CLI evidence strings are exact: surrounding whitespace on
fixed-width component hashes or source/target domains is rejected before source
material, deployment records, or full-light-client gate hashes are rendered. It
also recomputes the canonical
`fastpq-lane-balanced` OpenVerify source-adapter verifier commitment for the
TON -> SORA lane and rejects non-canonical `adapter_verifier_vk_hash` values
before rendering deployment records. The helper also recomputes the same TON
template component hashes used by `iroha_sccp` and rejects those
template-derived
source trust-anchor, consensus-verifier, message-inclusion, source-state
verifier, and finality-policy hashes, so it cannot emit records that Rust
production admission would reject for placeholder source-state verifier or
deployment evidence. Production TOML rendering now requires all three TON
full-light-client audit hashes plus an operator-supplied expected gate hash;
without that complete audit bundle the helper only emits compact diagnostic JSON
and reports `toml_ready = false`. The JSON readiness path also reports
`source_adapter_gate_ready_with_full_light_client_evidence` and
`source_adapter_gate_blockers`, matching the Solana source-state helper's
positive gate-ready bit and missing-evidence reasons. When complete TON
full-light-client audit
hashes are supplied, the helper appends them to the canonical deployment record,
emits `ton_full_light_client_gate_hash`, and compares the expected gate hash
before rendering TOML. It also rejects audit hashes that reuse the built-in TON
template source trust-anchor, consensus, message-inclusion, source-state, or
finality component hashes. The direct material and deployment record hash
helpers apply the same live-component and audit-bundle checks, so programmatic
rollout tooling cannot derive governed TON records from template source
components or partial audit evidence.

`SccpSourceAdapterProofV1` is an enum whose variant must match
`source_proof_plan` and `source_domain`:

- `EthereumBeaconReceipt`: `source_domain`, `beacon_slot`,
  `execution_block_number`, `execution_block_hash`,
  `execution_header_rlp`, `execution_receipts_root`, `beacon_finalized_root`,
  `beacon_proposer_index`, `beacon_parent_root`, `beacon_state_root`,
  `beacon_body_root`, `execution_payload_branch`, `sync_committee_root`,
  `sync_committee_signature_hash`, `receipt_root_index`, `receipt_trie_proof_nodes`,
  `receipt_trie_proof_hash`, `sync_committee_proof`, and
  `sync_committee_transition_proofs`.
- `BscValidatorSetReceipt`: `source_domain`, `validator_epoch`,
  `block_number`, `block_hash`, `receipts_root`, `validator_set_hash`,
  `commit_seal_hash`, `receipt_root_index`, `receipt_trie_proof_nodes`,
  `receipt_trie_proof_hash`, and
  `validator_set_transition_proofs`.
- `SolanaFinalizedTransaction`: `source_domain`, `finalized_slot`,
  `blockhash`, `bank_hash`, `transaction_status_root`, and
  `message_proof_hash`.
- `TonMasterchainShard`: `source_domain`, `masterchain_seqno`,
  `masterchain_block_hash`, `validator_set_hash`, `shard_block_hash`,
  `shard_state_root`, `transaction_root`, `masterchain_signature_hash`,
  `shard_proof_hash`, `validator_signature_proof`, and
  `validator_set_transition_proofs`.
- `TronDposReceipt`: `source_domain`, `solid_block_number`, `block_hash`,
  `witness_schedule_hash`, `witness_seal_hash`, `receipt_root`,
  `transaction_root`, `solid_block_header_proof`,
  `solid_block_ancestor_headers`, `solid_block_confirmation_headers`,
  `receipt_root_index`, legacy `receipt_root_branch`, `receipt_trie_proof_nodes`,
  `transaction_index`, `transaction_count`, `transaction_bytes`,
  `transaction_merkle_branch`,
  `receipt_proof_hash`, `witness_seal_proof`, and
  `witness_schedule_transition_proofs`. The TRON verifier requires the legacy
  branch to be empty, verifies either the transaction source proof or the
  bounded structural MPT proof nodes, and caps solid-block ancestor headers,
  solid-block confirmation headers, and witness-schedule transition chains at
  64 steps each.
- `SubstrateGrandpaEvent`: `source_domain`, `finalized_block_number`,
  `grandpa_set_id`, `block_hash`, `authority_set_hash`, `events_root`,
  `grandpa_justification_hash`, `storage_proof_hash`,
  `grandpa_justification`, and `authority_set_transition_proofs`.

All source inclusion branches and nested H256 branches are bounded to at most
64 32-byte siblings. EVM-family and TRON MPT openings are separately bounded to
at most 64 non-empty RLP proof nodes of 16 KiB each. ETH execution header RLP
and BSC transition header RLP inputs are bounded to 16 KiB before adapter
transcript hashing. ETH sync-committee source proofs are preflight-bounded to
at most 512 authorities, 64 transition proofs, canonical next-committee
payloads no larger than `1 + 4 + 512 * (4 + 96 + 8 + 4 + 256)` bytes, signer
bitmaps no larger than 64 bytes, aggregate signatures no larger than 192 bytes,
authority public keys no larger than 96 bytes, and PoPs no larger than 256
bytes. BSC validator-set source proofs are preflight-bounded to at most 255
validators, 64 validator-set transitions, canonical next-validator payloads no
larger than `1 + 4 + 255 * 28` bytes, signer bitmaps no larger than 32 bytes,
validator public keys no larger than 65 bytes, and signatures no larger than 65
bytes. Solana finalized-transaction source proofs are preflight-bounded before
adapter transcript hashing: transaction signatures are capped at 64 bytes,
emitter program ids at 32 bytes, validator vectors at 8,192 entries, account
raw-data witnesses at the protocol account widths used by the verifier
(3,762-byte vote accounts and 200-byte stake accounts, with the StakeHistory
sysvar capped at 65,536 bytes), AccountsLtHash witnesses at 2,048 bytes, signer
bitmaps at the exact byte width implied by the validator roster with zero
padding bits, Ed25519 signatures at 64 bytes each, StakeHistory entries at 512,
hard-fork hash data at 1,024 bytes, Tower vote slots at 31,
account-inclusion branches at 64 H256 siblings, and nested AccountsLtHash
source-state OpenVerify capsules at 2 MiB with bounded labels and nonzero proof
bytes whenever the capsule is present.
ETH/BSC receipt-trie
openings for non-placeholder material must prove an actual successful legacy or
typed EVM receipt whose logs contain
`topic0 = keccak256("SccpSourceEvent(bytes32)")`, `topic1 =
source_event_digest`, empty event data, and the governed source bridge emitter.
Placeholder structural fixtures may prove the typed RLP envelope
`[ "sccp:evm:receipt-root-value:v1", receipt_or_message_root ]`; raw 32-byte
values are rejected. Proven EVM-family receipt values and TRON receipt-root
values are bounded to 16 KiB before RLP decoding. Production TRON transaction
source proofs bound `transaction_bytes` to 64 KiB, require exactly one
canonical recoverable secp256k1 signature over `sha256(raw_data)` from the
non-zero trigger owner address, and bound the transaction Merkle branch to at
most 64 32-byte siblings. TRON witness schedules are bounded to at most 64
unique 21-byte witness addresses, TRON TransactionInfo values are bounded to 16
KiB, require exactly one successful result field, and cap each value to at most
128 logs and four 32-byte topics per log. TRON child/parent and ancestor raw
header vectors are bounded to 16 KiB before
solid-block header transcripts are hashed, and TRON solid-block ancestor and
confirmation chains are capped at 64 signed headers each. Empty source
inclusion branches, malformed sibling sizes, over-depth branches, malformed RLP
nodes, empty or over-depth MPT openings, malformed or oversized EVM-family
receipt/log values, malformed or oversized TRON transaction source calls,
unknown TRON transaction result extensions beyond `Result.fee`, receipt-root
values, or TransactionInfo values, oversized TRON witness rosters,
oversized TRON raw headers, oversized TRON ancestor or confirmation-header
chains, or oversized Solana vote/finality/source-state witness material fail
before the verifier hashes transcripts or accepts roots. TRON
source-adapter material verification
also runs a bounded shape preflight before transcript/evidence hashing so
partial TRON transaction-source fields, oversized legacy branches, MPT nodes,
ancestor headers, confirmation headers, wrong adapter domains, zero
block/root/seal/proof hashes, empty witness rosters, non-canonical signer
bitmaps, mismatched witness weights/signature counts, all-zero TRON witness
addresses, insufficient signed witness weight, truncated or non-canonical
header/witness signatures, stale transition-domain/message/seal metadata,
transition chains, or transition payloads are rejected before canonical adapter
bytes are serialized.

For ETH, `receipt_trie_proof_hash` is derived from
`blake2b256("sccp:evm:receipt-proof:v1" || 0x01 || source_domain_le ||
source_event_digest || beacon_slot_le || execution_block_number_le ||
execution_block_hash || execution_receipts_root || beacon_finalized_root ||
sync_committee_root || receipt_root_index_le ||
receipt_trie_proof_node_count_le ||
len_prefixed_receipt_trie_proof_nodes[0..n] || branch_len_le ||
inclusion_branch[0..n])`. The ETH
sync-committee hash is derived from
`blake2b256("sccp:eth:sync-committee:v1" || version ||
committee_count_le || (bls_public_key || weight_le || proof_of_possession)[0..n])`,
the signed message hash is derived from
`blake2b256("sccp:eth:sync-committee-message:v1" || version ||
source_domain_le || beacon_slot_le || execution_block_number_le ||
execution_block_hash || execution_receipts_root || beacon_finalized_root ||
sync_committee_root || receipt_trie_proof_hash)`, and the aggregate-signature
hash binds the signed message, committee, signer bitmap, and aggregate BLS
signature under `sccp:eth:sync-committee-aggregate:v1`.
ETH sync-committee transition-message hashes are derived from
`blake2b256("sccp:eth:sync-committee-transition-message:v1" || version ||
source_domain_le || from_sync_period_le || to_sync_period_le ||
transition_slot_le || finalized_beacon_root || parent_sync_committee_hash ||
next_sync_committee_hash || next_sync_committee_payload_hash ||
next_sync_committee_branch_hash)`. The next committee payload hash is
`blake2b256("sccp:eth:sync-committee-payload:v1" ||
next_sync_committee_payload)`, and the verifier separately derives
`next_sync_committee_hash` from the same canonical payload under
`sccp:eth:sync-committee:v1`. The transition-signature hash binds that message,
the raw next sync-committee payload, its payload hash, parent committee, signer
bitmap, and aggregate BLS signature under
`sccp:eth:sync-committee-transition-signature:v1`.
The verifier additionally requires `keccak256(execution_header_rlp)` to equal
`execution_block_hash`, parses `execution_header_rlp` as an Ethereum RLP header
list, and checks the canonical receipts-root field at index 5 plus the
block-number field at index 8 against the adapter and source-chain envelope.
For Deneb/Fulu execution headers, the same RLP fields are converted into the
consensus SSZ `ExecutionPayloadHeader` root: parent hash, fee recipient, state
root, receipts root, logs bloom, `prev_randao`, block number, gas limit, gas
used, timestamp, bounded `extra_data`, base fee, execution `block_hash`,
transactions root, withdrawals root, blob gas used, and excess blob gas. The
adapter's `execution_payload_branch` must be exactly four H256 siblings for the
fixed beacon-body execution-payload field index 9. The reconstructed body root
must match `beacon_body_root`, and the SSZ `BeaconBlockHeader` root recomputed
from slot, proposer index, parent root, state root, and body root must match
`beacon_finalized_root`.
Before the receipt-proof transcript hash is accepted, the verifier opens the
receipt trie rooted at `execution_receipts_root` with the RLP-encoded
`receipt_root_index`. For non-placeholder material, the proven value must
decode as an actual successful legacy or typed EVM receipt with a 256-byte logs
bloom, the canonical SCCP source-event topic, `source_event_digest` as topic 1,
empty event data, and the governed source bridge emitter. Exactly one matching
SCCP source-event log must be present; duplicate matches are rejected even when
each log is individually well formed. Placeholder
structural fixtures may instead decode as the typed EVM-family receipt-root
envelope whose 32-byte root equals
`receipt_or_message_root`.

For BSC, `receipt_trie_proof_hash` is derived from
`blake2b256("sccp:bsc:receipt-proof:v1" || 0x01 || source_domain_le ||
source_event_digest || validator_epoch_le || block_number_le || block_hash ||
receipts_root || validator_set_hash || commit_seal_hash ||
receipt_root_index_le || receipt_trie_proof_node_count_le ||
len_prefixed_receipt_trie_proof_nodes[0..n] || branch_len_le ||
inclusion_branch[0..n])`. These receipt-proof hashes bind the source event,
receipt root, finality witness, receipt-trie opening, and message inclusion
branch into the adapter proof before production material is evaluated. Before
the receipt-proof transcript hash is accepted, the verifier opens the receipt
trie rooted at `receipts_root` with the RLP-encoded `receipt_root_index` and
requires non-placeholder material to decode as an actual successful legacy or
typed EVM receipt with a 256-byte logs bloom, the canonical SCCP source-event
topic, `source_event_digest` as topic 1, empty event data, and a log emitter
equal to the governed source bridge emitter address. Placeholder structural
fixtures may instead decode as the typed EVM-family receipt-root envelope whose
32-byte root equals `receipt_or_message_root`.
BSC transition-message hashes are derived from
`keccak256("sccp:bsc:validator-set-transition-message:v1" || version ||
source_domain_le || from_validator_epoch_le || to_validator_epoch_le ||
transition_block_number_le || transition_block_hash ||
parent_validator_set_hash || next_validator_set_hash ||
next_validator_set_payload_hash || validator_set_metadata_proof_hash)`. The
canonical message is defined only for the BSC source domain, adjacent validator
epochs, and the Parlia epoch-start transition block
`transition_block_number = to_validator_epoch * 200`; Rust and SDK helpers
reject any other transition before hashing or proof packaging. The
transition-seal hash then binds that message, the raw transition header RLP,
the raw next-validator-set payload, the BSC ValidatorSet metadata proof, the
parent validator set, signer bitmap, and recoverable secp256k1 signatures under
`sccp:bsc:validator-set-transition-seal:v1`. The next-validator-set payload
hash is `keccak256("sccp:bsc:validator-set-payload:v1" || payload)`, and the
payload itself is the canonical `0x01 || validator_count_le ||
(validator_evm_address || power_le)[0..n]` transcript used for the
`sccp:bsc:validator-set:v1` hash. The metadata proof hash is
`keccak256("sccp:bsc:validator-set-metadata:v1" || version || state_root ||
next_validator_set_payload_hash || validator_contract_address ||
account_proof_nodes || storage_root || length_slot || length_value ||
length_value_hash || length_storage_proof_nodes || validator_storage_proofs)`,
where each storage value hash is
`keccak256("sccp:bsc:validator-set-storage-value:v1" || storage_value)`. The
SDK helper extracts the same payload from Parlia header `extraData`, supporting
the legacy address-only epoch layout and the post-Luban count/address/BLS-key
layout, while the verifier accepts only a transition whose extracted payload
matches the signed next-set transcript and the proven ValidatorSet storage.

JavaScript, Python, Swift, Kotlin, and Java Android SDKs expose matching
user-side helpers for these adapter-bound proof hashes: EVM and BSC
receipt-proof transcripts, EVM-family structural receipt-root MPT values, ETH
sync-committee transition payload transcripts, BSC validator-set payload,
storage-value, metadata-proof, and transition-message transcripts, Solana
message-proof transcripts, TON shard-proof
transcripts, TON masterchain block-message/signature transcripts, TON
validator-set transition payload transcripts, TRON
receipt-root MPT values plus receipt-proof and receipt-state MPT transcripts,
TRON witness-schedule transition payload transcripts, and Substrate
storage-proof/authority-set transcripts. The ETH sync-committee and BSC
validator-set helper surfaces enforce the same Rust verifier bounds before
hashing UI witness material, so browser and mobile proof generators reject
oversized committee payloads, signer bitmaps, signatures, and transition inputs
before invoking an app-linked prover. Solana Rust adapter preflight now applies
the same bounded-shape gate before source-adapter transcript hashing, so
oversized UI-collected finalized-vote, finality-context, account raw-data,
inclusion-branch, AccountsLtHash, or source-state proof material is rejected
before canonical adapter bytes are serialized. Substrate storage-proof helpers
require the canonical `frame_system::Events` storage key and the source-event
leaf index as first-class UI witness material, so the same runtime storage item
and path bits used to reconstruct the events root are also signed by the
GRANDPA precommit transcript. When deployed Substrate-family source-state
material is configured, the adapter must additionally carry the
`sccp-substrate-runtime-storage-v1` OpenVerify/FastPQ capsule that binds those
public inputs to the governed runtime storage-proof verifier hash. The
portal, operator tooling, and mobile apps should derive these hashes and the
runtime-storage OpenVerify/FastPQ request from RPC/liteserver/full-node witness
material before invoking the linked prover and before submitting the
source-chain envelope on-chain. The
helpers intentionally do not validate
external consensus or finality; that remains the job of the source-chain
verifier engine selected by the lane profile.

For TON, `shard_proof_hash` is derived from
`blake2b256("sccp:ton:shard-proof:v1" || 0x01 || source_event_digest ||
masterchain_seqno_le || masterchain_block_hash || shard_workchain_id_le ||
shard_shard_le || shard_seqno_le || shard_block_hash || shard_file_hash ||
shard_state_root || transaction_root || transaction_lt_le ||
[shard_state_proof_boc_len_le || shard_state_proof_boc] ||
[shard_accounts_root || key_bit_len_le || key_len_le || key ||
proof_boc_len_le || proof_boc] || shard_state_leaf_index_le ||
shard_state_branch_len_le || shard_state_inclusion_branch[0..n] ||
message_branch_len_le || inclusion_branch[0..n])`, where the two bracketed
sections are present only for dictionary-backed `ShardStateUnsplit.accounts`
openings. The verifier also recomputes the dictionary or Merkle opening and
requires the selected `ShardAccount.last_trans_hash` plus `last_trans_lt` to
match `transaction_root` and `transaction_lt`. This binds the masterchain,
shard BlockIdExt, shard state, transaction identity, shard-state opening, and
message inclusion branch into the same adapter witness that the destination
proof request exposes to user-side provers.
The TON validator-set hash is derived from
`blake2b256("sccp:ton:validator-set:v1" || version || validator_count_le ||
(ed25519_validator_key || weight_le)[0..n])`, the masterchain block-message
hash is derived from `blake2b256("sccp:ton:masterchain-block-message:v1" ||
version || source_domain_le || masterchain_seqno_le ||
masterchain_workchain_id_le || masterchain_shard_le ||
masterchain_block_hash || masterchain_file_hash || validator_set_hash ||
masterchain_config_root || masterchain_config_proof_hash ||
shard_workchain_id_le || shard_shard_le || shard_seqno_le ||
shard_block_hash || shard_file_hash || shard_state_root || transaction_root ||
shard_proof_hash)`, and the
masterchain signatures hash binds the block-message hash, validator set,
signer bitmap, and Ed25519 signatures under
`sccp:ton:masterchain-signatures:v1`. Production source material rejects
replayed TON proofs unless the derived validator-set hash equals the configured
TON source trust anchor or is derived from it by a valid transition chain, and
the signed weight is strictly greater than two thirds of the declared validator
weight. TON transition-message hashes are derived under
`sccp:ton:validator-set-transition-message:v1` from the source domain,
validator-set seqno range, full masterchain `BlockIdExt` fields
(workchain, shard, seqno, root hash, and file hash), parent validator-set hash,
next validator-set hash, canonical next validator-set payload hash, and next
validator-set config hash. The next validator-set payload is
`0x01 || validator_count_le || (ed25519_validator_key || weight_le)[0..n]`;
its payload hash is `blake2b256("sccp:ton:validator-set-payload:v1" ||
payload)`, and the next validator-set hash must be the payload-derived
`sccp:ton:validator-set:v1` hash. Transition signature hashes bind that
message, the raw next validator-set payload, the parent validator set, signer
bitmap, and Ed25519 signatures under
`sccp:ton:validator-set-transition-signatures:v1`. The validator-set
transition-chain hash used by shard-state and full-light audit statements now
commits to each complete canonical transition proof, including the signed
BlockIdExt, raw next-set payload, signer bitmap, validator keys/weights, and
signature bytes, rather than only the summary hashes.
The web, Python, Swift, Kotlin, and Java Android proof-generation tests include
non-empty transition chains so UI/mobile provers exercise this binding before
submitting generated proofs on-chain.
The active masterchain config leaf hash is
`blake2b256("sccp:ton:masterchain-config-leaf:v1" || version ||
source_domain_le || masterchain_seqno_le || masterchain_block_hash ||
shard_state_root || validator_set_hash || validator_set_payload_hash)`. The
config proof hash is
`blake2b256("sccp:ton:masterchain-config-proof:v1" || version ||
source_domain_le || masterchain_seqno_le || masterchain_block_hash ||
shard_state_root || config_root || validator_set_hash ||
validator_set_payload_hash || config_leaf_hash || config_key_bits_le ||
config_leaf_index_le || config_value_hash || len_prefixed_config_dictionary_proof_boc ||
branch_len_le || len_prefixed_branch_siblings[0..n])`. The verifier recomputes
the active payload hash from the validator-signature certificate, pins the
dictionary opening to TON config parameter `34` in a `HashmapE 32`, recomputes
the config dictionary root and opened value-cell hash from the supplied proof
BoC, and rejects source-Merkle fallback branches for config proofs before
accepting the proof hash.

For TRON production source proofs, the adapter verifies bounded
`transaction_bytes` against the signed-header `txTrieRoot`/adapter
`transaction_root` with a java-tron transaction Merkle branch. The Merkle leaf
hash is java-tron's `TransactionCapsule.getMerkleHash`, i.e. SHA-256 over the
full serialized `Transaction` bytes. It is intentionally not the public TRON
transaction id, which hashes only `raw_data`; using the txID would fail to bind
the `ret` execution result and signatures carried in `transaction_bytes`.
`receipt_proof_hash` is derived from
`blake2b256("sccp:tron:transaction-source-proof:v1" || 0x01 ||
source_event_digest || receipt_root || transaction_root ||
transaction_index_le || transaction_count_le ||
len_prefixed_transaction_bytes || transaction_merkle_branch_len_le ||
transaction_merkle_branch[0..n] || inclusion_branch_len_le ||
inclusion_branch[0..n])` only after the helper/verifier recomputes the
java-tron transaction Merkle root from the supplied full transaction bytes,
index/count, and branch. This binds the source digest, receipt/message root,
transaction Merkle root, authenticated transaction bytes, and source inclusion
branch. The transaction bytes must decode to one successful
`TriggerSmartContract` call to the governed source bridge contract with calldata
`keccak256("submitSccpSourceEvent(uint32,uint32,bytes32)")[0..4] ||
abi_word_u32(source_domain) || abi_word_u32(target_domain) ||
source_event_digest`; for production material the `owner_address` inside that
`TriggerSmartContract` must equal the configured
`source_bridge_owner_address` and the transaction must carry exactly one
recoverable signature whose signer equals that owner. Permissioned or multisig
TRON account transactions fail
closed until SCCP has a consensus-authenticated account-permission proof. The
public Rust transcript helper applies the same successful-call check before it
returns canonical transaction-source proof bytes, and its source-bridge-bound
variant additionally preflights the governed source bridge and owner addresses
without changing the resulting transcript hash. The material-bound Rust variant
extracts those pins from production TRON source verifier material. JavaScript,
Python, Swift,
Kotlin, and Java Android SDK helpers preflight the serialized `Transaction`
protobuf shape, success result, signature count/length, non-zero owner/contract
addresses, source-call calldata, and optional governed source bridge
emitter/owner address expectations before transcript hashing, leaving the
production Rust verifier as the owner-signature authority. Across those SDKs,
`contractRet = SUCCESS` is required, an explicit top-level `ret` must be
java-tron's default `SUCESS = 0`, and canonical transactions that omit that
default field are accepted. The Rust source-call
calldata helper is also lane-locked to TRON -> SORA and
requires a non-zero source-event digest before it returns canonical
`submitSccpSourceEvent(uint32,uint32,bytes32)` calldata. When
this transaction source proof is present,
legacy `receipt_trie_proof_nodes` must be empty. The reference source bridge
makes that successful call lane-bound, owner-governed, and replay-protected, so
a proof for the configured deployment also commits to the deployed source
bridge's governance surface through the recorded source-emitter address,
runtime code hash, network id, owner address, config hash, and deployment
material. The source bridge also exposes
`sourceBridgeConfigHash()`, computed as
`keccak256(abi.encode(keccak256("iroha:sccp:tron-source-bridge-config:v1"),
address(this), networkId, sourceDomain, targetDomain, owner))`; Rust exposes
`sccp_tron_source_bridge_config_hash_v1(...)` so operators can check the queried
on-chain hash for the production TRON -> SORA source lane before recording
source-bridge rollout evidence. TRON source
verifier material and source adapter deployment records must store the
network id, owner address, and that value as `source_bridge_network_id`,
`source_bridge_owner_address`, and `source_bridge_config_hash`; readiness fails
closed if the material/deployment hashes differ, if the production material
leaves any of those fields empty or zero, or if the config hash does not
recompute from the source bridge address, TRON -> SORA domains, network id, and
owner. The all-lanes production preflight also requires live TOML metadata for
the source bridge address, runtime code hash, and queried
`sourceBridgeConfigHash()` value to match the structured TRON source material,
so hand-edited rollout files cannot replace the governed config hash without
preserving the live source-bridge query evidence. The verifier evidence shape
gate applies the same recomputation whenever
TRON source bridge config fields are populated, even for material-only structural
fixtures without source-adapter deployment fields, so a mismatched rollout hash
cannot be carried as a merely structural evidence value.
Ethereum source verifier material follows the same deployment-record discipline
without an owner field: the governed ETH mainnet source bridge config hash
recomputes from chain id `1`, ETH -> SORA domains, the bridge address, and the
runtime code hash, and production-ready material/deployment records must carry
that network id and config hash.
The TRON source bridge emits the config hash in `SourceBridgeConfigured` at
deployment, and the owner can emit the current hash later through
`emitSourceBridgeConfigHash()` after an ownership transfer or rollout audit.
Its constructor accepts SORA's target domain id `0` for TRON -> SORA source
lanes, rejects any non-TRON source domain, rejects any non-SORA target-domain
id, and rejects same-source/target deployments.

The legacy TRON adapter path structurally verifies bounded
`receipt_trie_proof_nodes` as an Ethereum-style Merkle-Patricia-Trie transcript
using the RLP-encoded `receipt_root_index` as the trie key. The proven trie
value may be the placeholder-only typed RLP root envelope or a TRON
`TransactionInfo` protobuf whose single `result` field is explicitly
`SUCESS = 0` and whose bounded log list contains the SCCP source-event ABI
topic, source-event digest, and empty data. Matching log addresses compare on
the trailing 20 bytes when they carry TRON's `0x41` prefix. Missing, repeated,
or failed transaction info result values, malformed log addresses, wrong
emitters, topics with lengths other than 32 bytes, non-empty SCCP event data,
and over-cap log or topic lists are rejected. Because signed TRON headers
authenticate `txTrieRoot` as a transaction Merkle root, not an Ethereum-style
MPT root or `TransactionInfo`/log root, this `TransactionInfo` path is
structural only and cannot open production source admission. Its structural
`receipt_proof_hash` remains
`blake2b256("sccp:tron:receipt-state-proof:v1" || 0x01 ||
source_event_digest || receipt_root || transaction_root ||
receipt_root_index_le || receipt_trie_proof_node_count_le ||
len_prefixed_receipt_trie_proof_nodes[0..n] || inclusion_branch_len_le ||
inclusion_branch[0..n])`; zero source-event digests, receipt roots, or
transaction roots and empty SCCP source inclusion branches are rejected before
deriving that transcript.
The witness schedule hash is derived from
`blake2b256("sccp:tron:witness-schedule:v1" || version ||
witness_count_le || (tron_address || weight_le)[0..n])`, the witness-schedule
transition payload hash is derived from
`blake2b256("sccp:tron:witness-schedule-payload:v1" || version ||
witness_count_le || (tron_address || weight_le)[0..n])`. Both encodings reject
empty rosters, duplicate addresses, zero weights, and rosters above 64
witnesses before hashing. The solid-block
message hash is `keccak256("sccp:tron:solid-block-message:v1" || version ||
source_domain_le || solid_block_number_le || block_hash ||
witness_schedule_hash || receipt_root || transaction_root ||
receipt_proof_hash)` and is canonical only for `source_domain = TRON`, a
non-zero solid-block number, and non-zero block/schedule/receipt/transaction
hash inputs. The witness seal hash binds the signed message, schedule, signer
bitmap, and recoverable witness signatures only after the signers recover to
the declared schedule and exceed strict `> 2/3` witness weight. TRON
witness-schedule transition messages are derived under
`sccp:tron:witness-schedule-transition-message:v1` from the source domain,
schedule-epoch range, transition block number/hash, parent witness-schedule
hash, next witness-schedule hash, and next witness-schedule payload hash.
Only `source_domain = TRON`, `to_witness_schedule_epoch =
from_witness_schedule_epoch + 1`, non-zero transition blocks, and non-zero
hash fields are canonical. Transition seal hashes bind that message, the raw
next witness-schedule payload, the parent witness schedule, signer bitmap, and
recoverable witness signatures under
`sccp:tron:witness-schedule-transition-seal:v1`; stale next-schedule payloads,
payload-hash mismatches, message/seal mismatches, and under-quorum transition
seals fail before hashing. Ordered transition chains reject transition block
hashes that are not anchored to the supplied solid, parent, or signed ancestor
header evidence, epoch gaps, epoch overlaps, and non-increasing transition
block numbers.
The TRON solid-block header proof hash is
`blake2b256("sccp:tron:solid-block-header-proof:v1" || canonical_header_proof)`,
where `canonical_header_proof` includes raw header bytes, the witness header
signature, parent raw header bytes, parent witness signature, raw-data hashes,
derived block id, `txTrieRoot`, TRON `accountStateRoot` (protobuf field 11),
parent block id, witness address, timestamp, and header version. The decoder
rejects duplicate required header fields, malformed field types, unknown
raw-header fields other than optional `witness_id`, missing or zero
transaction/account-state roots, zero heights/timestamps, and witness addresses
outside the TRON `0x41` address namespace. The header-proof hash path also
fails closed unless both child and parent header signatures are low-S,
recovery-id-valid, and recover to the declared TRON witness addresses. The
adapter binding verifier
requires this proof to match the adapter block hash, solid block number, active
witness schedule, and `transaction_root`, to carry a non-zero authenticated
account-state root, and to prove the immediate signed parent link. When
ancestor headers are present, the same verifier also checks that the chain
starts from the immediate parent's parent block id, each ancestor is signed by
an active witness, block ids and parent ids link backward, heights decrease by
one, and timestamps move strictly backward. When confirmation headers are
present, it also checks that the chain starts from the solid block id, each
confirmation is signed by an active witness, block ids and parent ids link
forward, heights increase by one, and timestamps move strictly forward.
Non-placeholder TRON material requires at least one signed ancestor header and
requires the unique confirmation-header witnesses' schedule weight to exceed
two thirds of the active witness schedule weight. Header, witness-seal, and
witness-schedule-transition signatures accept java-tron's raw
`r || s || recoveryId` form with `recoveryId` in `0..=3`, as well as the
Ethereum-style `27..=30` form used by local fixtures; both forms are normalized
before secp256k1 public-key recovery and must use non-zero, in-range `r`
scalars plus low-S signatures. The same
binding verifier recomputes the witness schedule hash, the
`sccp:tron:solid-block-message:v1` hash, and the witness seal hash from the
adapter fields before accepting the TRON adapter proof.
JavaScript, Python, Swift, Kotlin, and Java Android transcript builders now
mirror that first-pass header-signature gate by rejecting zero or out-of-range
`r`, high-S, zero-S, and out-of-range recovery-id child/parent header signatures
before they hash TRON solid-block header proof material for a local prover.
Those same SDK surfaces also expose the TRON solid-block message, witness seal,
and witness-schedule transition message/seal builders; they verify
payload/hash consistency, signer recovery, bitmap weight, parent-schedule
binding, and strict `> 2/3` signed witness weight before returning transcript
hashes.
The TRON mainnet source-material template binds all eleven transcript families:
`sccp:tron:receipt-proof:v1`,
`sccp:tron:receipt-state-proof:v1`,
`sccp:tron:transaction-source-proof:v1`,
`sccp:tron:event-log-source-policy:v1`,
`sccp:tron:solid-block-header-proof:v1`,
`sccp:tron:witness-schedule:v1`,
`sccp:tron:witness-schedule-payload:v1`,
`sccp:tron:solid-block-message:v1`, `sccp:tron:witness-seal:v1`,
`sccp:tron:witness-schedule-transition-message:v1`, and
`sccp:tron:witness-schedule-transition-seal:v1`.

For Substrate-family source proofs, `storage_proof_hash` is derived from
`blake2b256("sccp:substrate:storage-proof:v1" || 0x01 ||
source_domain_le || source_event_digest || system_events_storage_key ||
source_event_leaf_index_le || finalized_block_number_le || grandpa_set_id_le ||
block_hash || authority_set_hash || events_root || branch_len_le ||
inclusion_branch[0..n])`, where `system_events_storage_key` is
`twox_128("System") || twox_128("Events")`
(`0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7`).
This binds the storage/event witness to the canonical runtime event storage
item, leaf index, and envelope branch used to reconstruct the events root. It
is a canonical transcript binding for adapter evidence, not a standalone
production runtime trie/storage-proof verifier. The
GRANDPA
authority-set hash is derived from
`blake2b256("sccp:substrate:authority-set:v1" || version ||
authority_count_le || (ed25519_authority_key || weight_le)[0..n])`, the
authority-set transition payload hash is derived from
`blake2b256("sccp:substrate:authority-set-payload:v1" || version ||
authority_count_le || (ed25519_authority_key || weight_le)[0..n])`, the
precommit-message hash is derived from
`blake2b256("sccp:substrate:grandpa-precommit:v1" || version ||
source_domain_le || finalized_block_number_le || grandpa_set_id_le ||
block_hash || authority_set_hash || events_root || storage_proof_hash)`, and
the justification hash binds the precommit hash, authority set, signer bitmap,
and Ed25519 signatures under
`sccp:substrate:grandpa-justification:v1`. Authority-set transition-message
hashes are derived under
`sccp:substrate:authority-set-transition-message:v1` from the source domain,
GRANDPA set-id range, transition block number/hash, parent authority-set hash,
next authority-set hash, and the next authority-set payload hash. Transition
justification hashes bind that message, the parent authority set, signer bitmap,
and Ed25519 signatures under
`sccp:substrate:authority-set-transition-justification:v1`.
The JavaScript, Python, Swift, Kotlin, and Java Android SDK helpers expose the
same authority-set payload, transition-message, and transition-justification
transcript hashes so UI/mobile provers can derive the exact Substrate GRANDPA
transition inputs that the Rust adapter verifies.

The generic structural verifier does not claim to validate external consensus
by itself. It does require the adapter variant to match the source plan, the
variant's finality height/block hash/root fields to match the enclosing
envelope, chain-specific witness hashes to be non-zero or exactly derived from
their canonical witness material, and
`adapter_transcript_hash` to equal
`blake2b256("sccp:source-adapter-transcript:v1" || source_domain ||
target_domain || source_proof_plan || finality_model || finality_height ||
finality_block_hash || receipt_or_message_root || source_event_digest ||
len(canonical_adapter_proof) || canonical_adapter_proof)`. Integer fields and
the adapter-proof length prefix are little-endian. This prevents a generic
self-consistency blob, stale adapter proof, or witness-substituted adapter
proof from being replayed as a different source-chain proof shape before the
deployment-backed readiness gate evaluates the lane's live adapter material.

`SccpSourceAdapterVerificationProofV1` has:

- `version = 1`
- `proof_family = "stark-fri-v1"`
- `circuit_id = "sccp-source-adapter-v1"`
- `proof_bytes`, non-empty and non-all-zero Norito-encoded
  `OpenVerifyEnvelope` bytes whose backend is `Stark`

The OpenVerify envelope must use the same circuit id, the lane-specific
source-adapter verifier-key commitment, the canonical FastPQ parameter set
`fastpq-lane-balanced`, an empty auxiliary payload, the canonical schema
descriptor, and public input columns for `source_domain`, `target_domain`,
`message_id`, `payload_hash`, `source_event_digest`, `finality_height`,
`finality_block_hash`, `receipt_or_message_root`, `adapter_transcript_hash`,
and `source_verifier_evidence_hash`. Wrong verifier-key hashes, backend tags,
schema descriptors, auxiliary envelope data, public-input columns, and backend
proof bytes are rejected. The outer envelope, nested STARK wrapper, and backend
FastPQ proof must all match their canonical Norito byte encodings before
metadata is trusted. The embedded FastPQ proof is verified against a
deterministic batch containing the
canonical adapter statement, the canonical adapter proof hash plus byte length,
and adapter context bytes. The full typed adapter proof stays in the envelope;
the OpenVerify batch commits to its hash so large committee-transition proofs
do not exceed the source-adapter proof-capsule limits. The canonical adapter
statement includes the verifier-evidence hash, so stale OpenVerify metadata,
wrong public inputs, unanchored verifier evidence, or tampered FastPQ proof
public IO fail structural verification.

Because the source-adapter capsule is embedded inside the SCCP transparent
bridge artifact, production configs must size bridge proof limits for
multi-proof SCCP artifacts. Taira sets `confidential.max_proof_size_bytes =
8388608` and `confidential.max_proof_bytes_block = 16777216`; smaller caps can
reject a valid SCCP artifact before source-adapter readiness is evaluated.

`SccpSourceMessageInclusionProofV1` binds the source/target domains, message
id, payload hash, source event digest, source event leaf hash,
receipt/message root, and leaf index. The source event leaf is recomputed as
`blake2b256("sccp:source:event-leaf:v1" || source_event_digest)`. The verifier
then folds `inclusion_branch` as a binary Merkle path using
`blake2b256("sccp:source:node:v1" || left || right)`, interpreting
`leaf_index` bits from least significant to most significant, and requires the
reconstructed root to equal `receipt_or_message_root`.

The structure gate rejects unsupported domains, source/target equality, a SORA
source in the source-chain envelope, chain-key/proof-plan/finality-model
mismatches, zero hashes or height, malformed typed proof blobs, bad 32-byte
branch shape, Merkle root mismatches, finalized-header/root mismatches, and a
bad `source_event_digest` or wrong plan-specific adapter proof. The bundle
binding gate then requires the envelope's
source domain, target domain, message id, payload hash, and commitment root to
match the embedded SCCP bundle exactly, preventing cross-lane, cross-target,
message-id, payload-hash, and commitment-root replay.

This envelope is the stable typed binding layer for source-chain adapters. The
generic verifier now consumes and cryptographically checks the typed proof blobs
instead of accepting arbitrary non-empty bytes. Production admission requires
the submitted bridge proof to persist the real source-chain proof bytes and to
match a deployment-backed source-adapter readiness record for the source lane.
A structurally valid `SccpSourceChainProofEnvelopeV1` is not sufficient by
itself: the lane must carry non-placeholder source verifier material, a matching
source-adapter engine deployment, a production destination rollout, a governed
route allowlist, and the required live route-canary evidence. The built-in
placeholder catalog and material-only readiness helpers remain fail-closed.
That readiness table exposes `source_adapter_engine` separately from the
destination rollout, so the adapter statement binding and FastPQ/OpenVerify
capsule can be marked ready while the lane remains disabled unless the external
consensus verifier, external receipt/message inclusion verifier, and
source-chain trust anchor are all active for the specific source domain.

On-chain `SubmitBridgeProof` validation is also direction-sensitive for SCCP
message proofs. SORA-origin bundles must expose a locally anchored
`NexusBridgeFinalityProofV1`, while non-SORA-origin bundles must expose a
verified `SccpSourceChainProofEnvelopeV1`; the runtime no longer tries to parse
external-source messages as Nexus finality. The non-SORA path is still gated by
lane production readiness, so missing deployment material, placeholder
components, or route-canary drift keep admission closed even when the typed
source proof is structurally valid.
Torii's message-proof read paths now search verified on-chain bridge proof
records for non-SORA bundles before falling back to local block reconstruction
or the in-memory SORA bundle cache. A registry candidate is served only when the
record is `Verified`, the typed transparent artifact backend, manifest hash,
and message id match, and the embedded source-chain proof envelope validates
against production source-lane material. Torii's local block reconstruction
still refuses to synthesize non-SORA source-chain envelopes from Iroha finality
data; production callers must submit the source-chain proof envelope produced
by the source adapter.

`RecordSccpMessage` is only valid for SORA-origin payloads. Non-SORA source
messages are admitted through `POST /v1/bridge/messages` with their
source-chain proof envelope and bridge proof artifact; they are not reconstructed
as Nexus-origin messages from block-level SCCP records.

## Torii API surface

- `GET /v1/sccp/capabilities` returns the relay-operator-facing SCCP capability snapshot:
  - local hub domain/chain identity (`SORA`);
  - the SCCP burn registry backend;
  - the generic message proof family (`stark-fri-v1`);
  - the SORA2 runtime proof family (`runtime-scale-v1`) and verifier backend
    (`sora-nexus-runtime-v1`);
  - the runtime SCALE envelope paths used by the bridge UI for wallet
    submission:
    - `/v1/sccp/proofs/message/{message_id}/runtime-scale`
  - the typed SCCP message proof-artifact discovery path (`/v1/sccp/artifacts/message/{message_id}`);
  - the normalized SCCP counterparty proof-job discovery path (`/v1/sccp/jobs/message/{message_id}`);
  - the SCCP proof-manifest discovery path (`/v1/sccp/manifests`);
  - supported codec ids/keys; and
  - the per-counterparty generic message backends / registry backends for `eth`,
    `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and
    `sora-polkadot`.
  - the production launch policy: the first-release runtime admits the Ethereum
    mainnet lane when its governed source material, source-adapter deployment,
    destination rollout, route allowlist, and route-canary evidence are
    complete; the all-lanes readiness checker remains available as a
    diagnostic, proof submission is permissionless, routes are allowlisted by
    deployment-time governance, and per-message human approval is never part of
    verification.
  - every currently advertised lane is marked `production_ready = false` with a
    `disabled_reason` and production-readiness blockers until source-chain
    finality/inclusion verification, source trust anchors, immutable
    destination verifiers, cryptographic anchors, and route allowlists are all
    live. The nested `source_adapter_engine` object shows that the typed adapter
    statement binding and FastPQ/OpenVerify capsule are present, while the real
    external consensus, inclusion, and source-anchor engines still block
    production. The nested `destination_rollout` object is bound to the
    counterparty domain and chain key; production readiness rejects rollout
    records with the wrong domain, wrong chain, wrong verifier plan, missing or
    empty verifier identity, missing anchor id, non-hex/zero verifier code
    hash, missing or zero Groth16 verifier key hash for EVM-family/TRON lanes,
    unexpected verifier key hashes on native Solana/TON/Substrate-family
    rollouts, or any remaining rollout blocker. Destination rollout readiness
    is also profile-bound for every advertised SCCP domain: ETH/BSC require non-zero EVM
    contract addresses plus their exact mainnet anchor ids, Solana requires a
    non-zero program id plus
    `sccp:sol:destination-anchor:solana-mainnet-beta:v1`, TON requires a
    non-zero raw basechain `0:account_hex` contract address plus
    `sccp:ton:destination-anchor:ton-mainnet:v1`, TRON requires a checksummed
    base58 contract address plus
    `sccp:tron:destination-anchor:tron-mainnet:v1`, and Substrate-family lanes
    require the exact `SccpBridge.submit_message_proof` runtime entrypoint.
    Generic anchor metadata, cross-chain verifier identities, zero addresses,
    malformed addresses, and wrong profile ids fail closed.
    The nested `route_allowlist` object is also profile-bound: readiness
    requires the exact per-domain route allowlist id, a non-zero 32-byte policy
    hash, the governance-allowlist activation policy, and no remaining
    blockers. For configured production readiness, that policy hash must also
    match the canonical route hash for the exact source material,
    source-adapter deployment, and destination binding records. Missing
    allowlist material, generic profile ids, zero or malformed policy hashes,
    stale evidence-bound route hashes, and cross-domain allowlist replay all
    fail closed.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_capabilities_json(...)` and `get_sccp_capabilities(...)`;
    - JavaScript: `ToriiClient.getSccpCapabilities(...)`; and
    - Python: `ToriiClient.get_sccp_capabilities()`.
- `GET /v1/sccp/manifests` returns the typed SCCP proof manifests for the same
  counterparty set. Each manifest binds together:
  - the chain key and counterparty domain id;
  - the target verifier backend key for that counterparty lane (`evm-groth16-bn254-v1`, `tron-groth16-bn254-v1`, `solana-program-v1`, `ton-contract-v1`, or `substrate-runtime-v1`);
  - the declared SCCP proof security model (`RecursiveZk`) and anchor mode (`CryptographicProof`);
  - a typed destination binding (`version`, `key`, `binding_hash`) that scopes proofs to the intended verifier deployment/runtime context for that lane;
  - the chain-specific message backend / registry backend pair;
  - the canonical counterparty account codec;
  - the intended verifier target (`EVM`, `Solana`, `TON`, `TRON`, or
    Substrate-style runtime);
  - the finality model label used by proof tooling; and
  - the manifest seed used to derive the bridge proof manifest hash, plus the
    required SCCP public inputs (`message_id`, `payload_hash`, `target_domain`,
    `commitment_root`, `finality_height`, `finality_block_hash`).
  - each manifest now also carries a chain-specific `submission_template`
    describing the expected verifier entrypoint, envelope encoding, submission
    kind, and required argument keys for relay tooling targeting that chain.
    Unsupported or unencodable submission-template encodings fail closed during
    package construction and verifier-side envelope reconstruction; relayers do
    not receive an empty or generic fallback envelope.
  - Torii capability discovery and proof-material routing treat a manifest as
    production-ready only when its `production_ready` flag still matches the
    canonical verifier backend, verifier target, and `stark-fri-v1` proof
    family for the counterparty lane.
  - the reference EVM wrapper contracts for that template now live under
    `contracts/evm/sccp` in this repo.
  - ETH and BSC currently share the same reference EVM wrapper entrypoint:
    `submitSccpMessageProof(bytes,bytes32[6],bytes32)`, whose canonical
    Solidity selector is `0xbd57826c`.
    The wrapper exposes `destinationBindingHash()` for live deployment audits,
    and `MessageProofAccepted` carries the accepted statement hash plus
    destination binding hash alongside the message id, source domain,
    commitment root, backend, proof family, and network id.
  - for ETH/BSC, production manifests target the `evm-groth16-bn254-v1`
    immutable verifier adapter. `contracts/evm/sccp` now includes
    `SccpGroth16Bn254MessageVerifier`, which verifies ABI-encoded Groth16
    proof points against an immutable constructor-supplied BN254 verifying key
    and binds the proof to the six SCCP public-input words, source domain,
    statement hash, and destination binding hash. Constructor G1 points are
    `(x, y)`, constructor G2 points are `(x_0, x_1, y_0, y_1)`, and the
    flattened IC vector must contain exactly ten G1 points: the constant term
    plus one point for each of the nine SCCP public signals. `proof_bytes` for
    this backend must ABI-decode as `(uint256 version, bytes32 message_id,
    uint256 source_domain, bytes32 commitment_root, uint256[2] a, uint256[4]
    b, uint256[2] c)`, with `version = 1`, and the deployed verifier requires
    the exact 384-byte static ABI tuple with no trailing bytes. The nine public
    signals are
    `uint256(keccak256(abi.encode(label, value))) mod r`, reduced modulo the
    BN254 scalar field, for `message_id`, `payload_hash`, `target_domain`,
    `commitment_root`, `finality_height`, `finality_block_hash`,
    `source_domain`, `statement_hash`, and `destination_binding_hash`, in that
    exact order. Rust exposes
    `sccp_groth16_bn254_public_signal_words(...)`, JavaScript and Swift expose
    `sccpGroth16Bn254PublicSignalWords(...)`, Kotlin exposes
    `SccpEvm.groth16Bn254PublicSignalWords(...)` and
    `SccpTron.groth16Bn254PublicSignalWords(...)`, Java Android exposes
    `EvmSccpProver.groth16Bn254PublicSignalWords(...)` and
    `TronSccpProver.groth16Bn254PublicSignalWords(...)`, and the Python Torii SDK exposes
    `sccp_groth16_bn254_public_signal_words(...)` so prover tooling derives the
    exact field words consumed by the EVM and TRON Groth16 destination
    verifiers. The Rust submission-package builder has a signer-free
    `EvmGroth16ContractCall` path for this backend. It accepts only the exact
    ABI tuple above, rejects malformed length, wrong version, source-domain
    overflow, message-id/source-domain/commitment-root replay, zero or
    out-of-field proof points, signer-supplied production packages,
    destination-binding metadata with the wrong version or zero hash, and any
    package whose deployment binding reuses the generic manifest binding hash.
    The legacy reference verifier still accepts an EVM-native secp256k1
    attestation envelope over the native SCCP proof hash and canonical
    fixed-width public inputs in direct fixture tests. It rejects non-canonical attestation ABI
    length/offsets, zero native-proof hashes, zero SCCP statement/public-input
    fields, mismatched message/commitment public inputs, zero target domains,
    target-domain overflow, and same-source/target domain attestations before
    signature recovery. That envelope also commits a `destination_binding_hash`
    derived from the wrapper address, immutable verifier address, verifier
    bytecode hash, optional verifier key hash, verifier backend, proof family,
    network id, and the bound SCCP source/target domains so one attestation
    cannot be replayed across sibling deployments, rebound to a different
    verifier deployment or key, or reused for a different lane on the same
    network. The EVM wrapper constructor rejects missing or mismatched verifier
    bytecode hashes, any backend other than `evm-groth16-bn254-v1`, missing or
    mismatched immutable `verifyingKeyHash()` material, empty backend labels,
    any proof family other than `stark-fri-v1`, zero network ids, non-SORA
    source domains, target domains outside ETH/BSC, and same-source/target
    deployments at construction time. Rust deployment-binding helpers mirror
    that contract gate and do not derive deployable EVM destination bindings
    for the secp256k1 reference backend. Submission fails closed
    before verifier dispatch on zero statement hashes, zero message, payload,
    commitment, or finality public-input fields, or a target-domain word that
    does not match the configured lane. The wrapper still enforces the configured
    source/target domains before accepting a proof. The reference attestation
    path is still explicitly non-production and cannot be bound through
    `SccpMessageBridge`. JavaScript, Python, Swift, Kotlin, and Java Android
    SDKs now expose EVM-family contract-call submission builders that package
    UI-generated 384-byte Groth16 proof results into
    `submitSccpMessageProof(bytes,bytes32[6],bytes32)` calldata, rederive the
    six transparent `bytes32` public-input words, recheck statement and
    destination-binding hashes, proof context, non-zero request hash, and the
    envelope hash recomputed from request hash plus proof bytes, and reject
    proof bytes or public signal words that do not match the wrapped
    local-prover result.
  - TRON now advertises the `tron-groth16-bn254-v1` verifier backend and follows
    the same fixed-word BN254 verifier shape on the TVM side. The Rust
    submission-package builder has a signer-free `TronContractCall` path for
    this backend; it accepts only the ABI tuple above plus a deployment-specific
    TRON destination binding, rejects the generic manifest binding, rejects
    generic FastPQ/OpenVerify bytes, malformed Groth16 points,
    source/message/root replays, any signer-supplied package, and verifier-side
    packages whose deployment binding reuses the generic manifest binding hash.
    The TRON package builder and verifier parse the deployment binding key,
    recompute the canonical TRON destination binding hash from the embedded
    network id, base58 verifier address, code hash, and verifier-key hash, and
    reject tampered or hand-written binding hashes even when the envelope
    arguments are otherwise internally consistent.
    Rust packaging also rejects zero deployment network ids, zero statement
    hashes, zero required public-input fields, wrong target domains, and
    same-source/target domain public inputs before it emits EVM/TRON Groth16
    relay packages. The Rust TRON destination-binding helper also refuses
    non-SORA source-domain ids, same-source/target manifests, non-TRON manifest
    target domains, and non-`stark-fri-v1` proof families before deriving the
    deployment-specific binding.
    The TRON deployment binding is derived from the target network id,
    checksummed base58 verifier contract address, verifier code hash, and
    Groth16 verifier key hash. The JavaScript, Swift, Kotlin, Java Android, and
    Python SDK surfaces now expose TRON proof-request/prover wrappers so browser
    portals, mobile apps, and Python tooling can bind the canonical public
    inputs, SCCP bundle bytes, source proof bytes, statement hash, destination
    binding hash, and fixed BN254 signal words before invoking an app-linked
    Groth16 prover, and they reject non-TRON target-domain public inputs before
    constructing the request. The same SDK wrappers reject empty, all-zero, or
    non-384-byte external Groth16 proof bytes before deriving the request-bound
    envelope hash. JavaScript, Python, Swift, Kotlin, and Java Android also expose TRON
    contract-call submission builders for the same
    `submitSccpMessageProof(bytes,bytes32[6],bytes32)` ABI, producing selector,
    calldata/envelope bytes, ABI argument metadata, public-input words, and
    proof-result binding checks that revalidate proof context, non-zero request
    hash, and envelope hash before a wallet or relayer submits the proof
    on-chain. The JavaScript package
    entrypoint re-exports the TRON helpers at runtime, matching the TypeScript
    declarations. Solana, TON, and Substrate keep their own platform-native
    instruction/cell/call encodings.
    `contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol` provides the
    TRON/TVM deployment entrypoint for the same immutable BN254 verifier logic.
    Its `submitSccpMessageProof(bytes,bytes32[6],bytes32)` entrypoint derives
    the TRON destination binding from `address(this)`, target network id,
    source/target domains, verifier backend, proof family, the actual deployed
    runtime bytecode hash exposed by `verifierCodeHash()`, and immutable
    `verifyingKeyHash()`, then records accepted message ids to block replay. The
    constructor rejects missing or mismatched key hashes, empty proof-family
    labels, proof families other than `stark-fri-v1`, zero network ids,
    non-SORA source-domain ids, non-TRON target domains, and
    same-source/target domains; submission rejects zero
    statement/public-input fields, wrong target-domain words, malformed Groth16
    ABI tuple lengths, and Groth16 proof envelopes whose cleartext source-domain
    word does not match the configured lane before verifier dispatch. The
    wrapper exposes `destinationBindingHash()`
    and a post-deploy `emitDestinationBindingConfigured()` canary call so
    tooling can capture the exact hash before submitting a proof. Rollout still
    requires recording the
    deployed TVM bytecode hash and `verifyingKeyHash()` in the governed
    destination binding material; the code hash is not a constructor parameter
    because embedding a contract's own code hash would make the runtime hash
    self-referential. The offline TRON evidence
    helper recomputes this binding hash from the base58 wrapper address,
    deployment hashes, target network id, source/target domains, and
    proof-family label, can compare it with
    `--expected-destination-binding-hash`, and includes the recomputed value in
    `--full-toml` output for rollout audits. Compact JSON and full rollout TOML
    also include the canonical `SccpDestinationBindingV1.key` derived from the
    same destination material, so the relay key and governed hash are captured
    together. Its compact JSON, source TOML, and full rollout TOML output modes
    are mutually exclusive.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_proof_manifests_json(...)` and `get_sccp_proof_manifests(...)`;
    - JavaScript: `ToriiClient.getSccpProofManifests(...)`; and
    - Python: `ToriiClient.get_sccp_proof_manifests()`.
- `GET /v1/sccp/proofs/burn/{message_id}` and `GET /v1/sccp/proofs/message/{message_id}` return the live SCCP bundle keyed by canonical message id. The generic `message` route remains the raw bundle/debug fetch surface for multi-chain SCCP transfer, registry, and token-control traffic.
- `GET /v1/sccp/artifacts/message/{message_id}` returns the typed SCCP transparent proof artifact for the same canonical message id. Each artifact now bundles:
  - the target verifier backend metadata for the counterparty lane;
  - the chain-specific `message_backend` / `registry_backend`;
  - the shared SCCP security model / cryptographic anchor mode and the destination binding carried through from the manifest;
  - the finality model and verifier target derived from the shared manifest table;
  - the canonical public inputs (`message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`);
  - `proof_bytes` containing either a real Norito-encoded `OpenVerifyEnvelope`
    over the canonical SCCP statement batch derived from the bundle and
    manifest, or the explicit Groth16/bn254 ABI tuple for EVM-family and TRON
    production backends;
  - JSON responses for this route also expose `proof_envelope_summary`, which
    reports the decoded open-verify backend, circuit id, verifier commitment
    hash, schema hash, public-input column/word counts, and wrapper/backend
    proof lengths without changing the underlying Norito wire artifact;
  - a generated `submission_package` carrying the target verifier entrypoint,
    envelope encoding, raw argument blobs, prebuilt relay envelope bytes, and
    a typed `platform_payload` view for that lane:
    - ETH/BSC production backend: `evm_groth16_contract_call` carrying the
      Groth16 ABI proof tuple directly, with no attestor signatures;
    - ETH/BSC reference backend: `evm_contract_call`, retained only for
      non-production secp256k1 fixture payloads and not accepted as a
      production-ready manifest backend;
    - TRON: `tron_contract_call`, carrying the Groth16 ABI proof tuple, fixed
      TVM ABI public-input words, statement hash, and destination binding;
    - Solana: `solana_program_instruction`, carrying proof bytes, canonical
      public-input bytes, the canonical SCCP bundle bytes, the destination
      binding, destination binding hash, statement hash, and proof context hash
      so the verifier program is scoped to the same deployment context used by
      UI-linked Solana provers;
    - TON: `ton_internal_message`, carrying a
      `ton_message_body_boc_v1` `message_body_boc` plus its `query_id`,
      destination binding hash, statement hash, proof bytes, public inputs,
      and SCCP bundle bytes;
    - Substrate-family lanes: `substrate_runtime_call`, carrying a
      `scale_call_v1` runtime-call envelope for
      `SccpBridge.submit_message_proof` with SCALE vectors for proof bytes,
      canonical SCCP transparent public-input bytes, and the SCCP bundle bytes;
  - JSON responses for EVM and TRON Groth16 artifacts expose
    `groth16_proof_summary` instead of `proof_envelope_summary`. The summary
    reports `platform_payload`, `version`, `proof_len_bytes`,
    `public_input_word_count = 6`, `groth16_public_signal_count = 9`,
    `message_id`, `source_domain`, `commitment_root`,
    `destination_binding_key`, and `destination_binding_hash`.
  - the embedded Nexus SCCP message bundle so verifiers can reconstruct the exact statement being proven.
  - `iroha_sccp` now also exposes a normalized counterparty proof-job projection over that artifact:
    - `decode_sccp_normalized_codec_value(...)` decodes codec-bearing SCCP fields into typed EVM / Solana / TON / Tron / logical-text values; and
    - `build_sccp_counterparty_proof_job_from_artifact(...)` /
      `build_sccp_counterparty_proof_job_from_artifact_allow_unready(...)` /
      `build_sccp_counterparty_proof_job_from_bundle(...)` produce a
      prover-oriented job with the normalized payload projection plus the
      original typed bundle. For production EVM/BSC Groth16 lanes, proof tools
      must use
      `build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding(...)`
      or its `_allow_unready` diagnostic variant so the actual Groth16 proof
      bytes are supplied explicitly and the signer path remains closed. For the
      TRON Groth16 lane, proof tools must use
      `build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof(...)`
      or the destination-binding/`_allow_unready` variants; generic proof-job
      construction no longer packages native FastPQ bytes as a TVM contract
      proof.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_artifact_json(...)`,
      `get_sccp_message_proof_artifact(...)`, and their `_with_params`
      variants using `SccpMessageProofQueryParams`;
    - Python: `ToriiClient.get_sccp_message_proof_artifact(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofArtifact(...)`.
    Rust, Python, and JavaScript helpers forward production destination
    parameters when callers need Torii to construct a deployment-specific
    Groth16 submission package. EVM/BSC uses `network_id_hex` /
    `networkIdHex`, `verifier_address_hex` / `verifierAddressHex`,
    `bridge_address_hex` / `bridgeAddressHex`, `verifier_code_hash_hex` /
    `verifierCodeHashHex`, and `verifier_key_hash_hex` /
    `verifierKeyHashHex`. TRON uses the same network id and code/key hash
    fields plus `tron_verifier_address` / `tronVerifierAddress`, which SDK
    helpers and the bridge-feature CLI validate as a checksummed TRON
    Base58Check address. EVM/TRON
    deployment-bound packaging requires `expected_destination_binding_hash_hex`
    / `expectedDestinationBindingHashHex` so Torii can compare the recomputed
    binding against live verifier evidence before returning an artifact or job.
    The node must also have a configured production destination rollout for
    that lane, and the same recomputed key/hash must match the configured
    rollout; otherwise Torii rejects the deployment-bound request even if the
    caller's expected hash matches its own query fields. Supplying a deployment
    destination binding also triggers the configured launch policy for that
    lane, so Ethereum packaging can open with complete ETH evidence while
    non-ETH destination rollouts remain blocked until their own lane policy
    opens.
    EVM/TRON Groth16 packaging also requires `proof_bytes_hex` / `proofBytesHex`
    for externally generated proof bytes; without those bytes Torii returns a
    bad request instead of attempting a generic signer/FastPQ package builder.
    The submitted proof bytes must decode to the exact 384-byte BN254 Groth16
    ABI tuple, cannot be all zero, must roundtrip canonically, must pass BN254
    G1/G2 curve-membership preflight, and must bind the tuple version, message
    id, SORA source-domain word, and commitment root to the SCCP message public
    inputs, so Torii rejects placeholder, truncated, off-curve, replayed, or
    cross-source Groth16 material before constructing a deployment-bound
    package. Supplied
    EVM/TRON destination and proof fields are validated before the disabled-lane
    readiness fallback, so malformed relay material cannot be hidden behind a
    lane-not-ready response. Direct Torii requests now reject deployment
    destination fields without `proof_bytes_hex`, standalone `proof_bytes_hex`
    without deployment destination fields, and mixed EVM/TRON destination
    tuples before destination binding construction. Direct Torii requests also validate
    `tron_verifier_address` as a non-zero checksummed TRON Base58Check address
    before destination binding construction, so malformed addresses produce a
    field-specific bad request even on unready lanes. When
    production readiness is still false and
    unready diagnostics are not enabled, valid deployment bindings and proof
    bytes are discarded rather than returned to callers; Torii still retains the
    validated binding internally long enough to enforce configured rollout and
    all-lanes launch checks before returning the disabled-lane response. Rust
    artifact/proof-job helpers and the
    bridge-feature CLI apply the same 384-byte BN254 tuple and curve preflight
    for query `proof_bytes_hex`; Rust, Swift, Kotlin, and Java Android raw JSON
    bridge submit helpers also apply the same tuple rule to snake-case
    `proof_bytes_hex` before posting DTOs; when the DTO includes a
    `message_bundle`, those mobile/raw helpers also reject proof tuples whose
    version, message id, SORA source-domain word, or commitment root does not
    match the bundle, and they validate recognized raw destination hash/address
    fields before network I/O. The raw and typed SDK preflights now require a
    complete EVM or TRON deployment tuple when proof bytes are present,
    recompute canonical TRON destination binding hashes from the supplied
    network id, Base58Check verifier address, verifier code hash, and
    verifier-key hash, and recompute EVM-family hashes for the supported ETH/BSC
    target-domain bindings from the supplied network id, verifier/bridge
    addresses, verifier code hash, and verifier-key hash. Forged
    `expected_destination_binding_hash_hex` pins are rejected before network I/O,
    and mixed EVM/TRON destination fields are rejected before network I/O. Bridge-proof
    submit preflights also require exactly one of `burn_bundle` or
    `message_bundle` and reject deployment proof material on burn-bundle
    submissions before network I/O. JavaScript and Python typed Torii
    clients apply the same placeholder, canonical length, and BN254 curve
    preflight before sending artifact, proof-job, bridge-proof, or
    bridge-message requests with
    `proofBytesHex` / `proof_bytes_hex`, and their bridge-proof/message submit
    helpers additionally reject locally supplied EVM/TRON Groth16 tuples whose
    version, message id, SORA source-domain word, or commitment root does not
    match the accompanying `message_bundle`.
  - current production behavior: this route can expose production packaging for
    BSC mainnet only when the configured BSC source-chain finality/inclusion
    material, immutable destination verifier deployment, active cryptographic
    anchors, route allowlist, and route canary are all present. Other
    counterparty lanes remain behind their future lane launch policies.
- `GET /v1/sccp/jobs/message/{message_id}` returns the normalized SCCP counterparty proof job for the same canonical message id. Each job bundles:
  - the chain family, chain key, backend labels, verifier backend, manifest seed, finality model, verifier target, and canonical SCCP public inputs;
  - the same SCCP security model / cryptographic anchor mode and destination binding that the artifact and manifest commit into the canonical statement hash;
  - a normalized payload projection with typed codec values for EVM / Solana / TON / Tron / logical-text surfaces; and
  - the same chain-specific `submission_template` advertised by the manifest, so proof tooling can derive the target verifier entrypoint and argument list without hard-coding per-chain packaging; and
  - the generated `submission_package` for the chain-specific relay/verifier
    lane, including the same typed `platform_payload` projection surfaced on the
    artifact route; and
  - production-ready EVM/BSC lanes require callers to provide
    `network_id_hex`, `verifier_address_hex`, `bridge_address_hex`, and
    `verifier_code_hash_hex`; `evm-groth16-bn254-v1` lanes also require
    `verifier_key_hash_hex`. Those fields bind only the EVM deployment
    submission package, while the transparent proof artifact keeps the manifest
    destination binding; and
  - JSON responses for OpenVerify-backed jobs expose `proof_envelope_summary`,
    derived from the canonical native SCCP proof for the bundled message so
    operators can inspect the bound circuit/verifier/schema metadata before
    submission. EVM Groth16 jobs expose `groth16_proof_summary` instead, using
    the same fields as the artifact route; and
  - the original typed Nexus SCCP message bundle so proof tooling can keep both the normalized view and the canonical committed preimage in one document.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_job_json(...)`,
      `get_sccp_message_proof_job(...)`, and their `_with_params` variants;
    - Python: `ToriiClient.get_sccp_message_proof_job(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofJob(...)`.
    Rust, Python, and JavaScript helpers accept the same production
    EVM/TRON destination parameters as the artifact route so proof-job callers
    do not have to build query strings by hand, and they fail locally for
    empty or all-zero external `proof_bytes_hex` / `proofBytesHex` on that
    query path. Python and JavaScript typed response parsers also validate
    `TronBase58Check.payload` projections and expose the decoded 21-byte
    `0x41`-prefixed TRON payload as lowercase hex; malformed Base58Check
    strings, bad checksums, short payloads, and the all-zero TRON account
    payload are rejected during response normalization.
  - current production behavior: this route can expose production packaging for
    BSC mainnet only when the configured BSC source-chain finality/inclusion
    material, immutable destination verifier deployment, active cryptographic
    anchors, route allowlist, and route canary are all present. Other
    counterparty lanes remain behind their future lane launch policies.
- `GET /v1/sccp/proofs/message/{message_id}` now reconstructs the proof from committed blocks that contain `RecordSccpMessage` instructions and a non-null `sccp_commitment_root` in the finalized block header. The in-memory bundle registry is retained only for unit tests and never bypasses typed artifact or finality verification.
- Generic SCCP `message` payloads now enforce explicit v1 codec families during structural verification instead of accepting arbitrary nonzero codec ids:
  - `1`: generic UTF-8 logical identifiers;
  - `2`: EVM `0x`-prefixed 20-byte hex addresses;
  - `3`: Solana base58 public keys;
  - `4`: TON raw `workchain:account_hex` addresses; and
  - `5`: TRON base58check account addresses, excluding the all-zero
    `0x41`-prefixed payload.
- `POST /v1/bridge/proofs/submit` accepts exactly one of `burn_bundle` or `message_bundle`. Token add, pause, and resume operations are submitted as SCCP message bundles; the bridge does not accept parliament certificates as transaction proofs. `message_bundle` is converted into a typed SCCP transparent proof artifact and then wrapped in a bridge proof with backend label `bridge/sccp/stark-fri-v1/<chain>`.
  - client helpers exist for relays that submit the DTO directly:
    - Rust: `iroha::client::Client::post_bridge_proof_submit_json(...)`;
    - Python: `ToriiClient.submit_bridge_proof(...)`; and
    - JavaScript: `ToriiClient.submitBridgeProof(...)`;
    - Swift: `ToriiClient.postBridgeProofSubmitJson(...)`;
    - Kotlin: `HttpClientTransport.postBridgeProofSubmitJson(...)`; and
    - Java Android: `HttpClientTransport.postBridgeProofSubmitJson(...)`.
    The helpers forward the same EVM/TRON deployment material accepted by the
    artifact and proof-job discovery routes, including `proof_bytes_hex` /
    `proofBytesHex`, `tron_verifier_address` / `tronVerifierAddress`, and the
    required `expected_destination_binding_hash_hex` /
    `expectedDestinationBindingHashHex` pin. Rust, JavaScript, Python, Swift,
    Kotlin, and Java Android relay helpers reject empty or all-zero external
    proof bytes and malformed or all-zero TRON Base58Check verifier addresses
    before sending submit requests when those fields are present.
  - current production behavior: generic SCCP `message_bundle` conversion is
    disabled for all live counterparties until every advertised chain satisfies
    the all-lanes production readiness gate. Governance installs verifier
    identities, code hashes, anchors, and route allowlists; after activation,
    message submission remains permissionless and no human approval is part of
    the per-message validity path.
  - SCCP artifact/job discovery, state-changing SCCP submit endpoints, and
    on-chain `SubmitBridgeProof` validation ignore
    `sccp_allow_unready_transparent_proofs`; that flag cannot make disabled
    lanes consumable.
    On-chain admission uses a manifest-only unready allowance for configured
    deployment lanes and still fails material-only or otherwise unready
    non-SORA source-chain envelopes.
  - non-SORA `message_bundle` submission must carry a verified source-chain
    proof envelope; Torii no longer manufactures synthetic external-chain
    finality from local Nexus/Iroha finality evidence.
- `POST /v1/bridge/proofs/submit` now derives chain-specific SCCP transparent backends for generic `message` bundles:
  - outbound `SORA -> ETH` and inbound `ETH -> SORA` messages use `bridge/sccp/stark-fri-v1/eth`;
  - the same pattern applies to `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and `sora-polkadot`;
  - the bridge proof manifest hash is derived from the same domain suffix, so proof IDs and registry queries split cleanly by counterparty chain instead of collapsing all SCCP traffic into one generic backend bucket.
- ETH/BSC message-proof building previously depended on Torii's
  `da_receipt_signer` using `secp256k1`, because the EVM submission package was
  a signer-backed attestation envelope over the canonical SCCP proof-envelope hash and
  canonical public inputs. That path is now disabled for production because it
  is not destination-native cryptographic verification.
- `POST /v1/bridge/proofs/submit` and `POST /v1/bridge/messages` now also return normalized SCCP counterparty metadata in the response:
  - `counterparty_domain` is the numeric SCCP domain id; and
  - `counterparty_chain` is the canonical domain key (`eth`, `bsc`, `sol`, `ton`, `tron`, `sora2`, etc.).
- `GET /v1/zk/proof/{backend}/{hash}` and `GET /v1/zk/proofs` now mirror that metadata inside `bridge.payload` for SCCP transparent proofs when the backend matches the chain-split SCCP family.
  - when the stored payload decodes as a typed SCCP artifact, the bridge summary now also exposes `message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`, and `proof_artifact_len_bytes`.
  - the bridge summary additionally exposes `verifier_backend`, `inner_verifier_backend`, `inner_chain_family`,
    `inner_payload_kind`, and `inner_statement_hash`, derived from the
    canonical SCCP statement context rather than from an embedded placeholder
    envelope.
- `POST /v1/bridge/messages` accepts an inbound `message_bundle` targeted at SORA, records the corresponding transparent-ZK bridge proof, and emits a typed `BridgeReceipt` for `transfer` payloads. It accepts the same optional EVM/TRON deployment material as `POST /v1/bridge/proofs/submit`, including `proof_bytes_hex` and the checksummed TRON Base58Check `tron_verifier_address`, so a relay can submit and settle deployment-bound Groth16 message proofs in one transaction.
  - raw SDK helpers are available for relays that submit the DTO directly:
    Rust: `iroha::client::Client::post_bridge_message_submit_json(...)`,
    Python: `ToriiClient.submit_bridge_message(...)`, JavaScript:
    `ToriiClient.submitBridgeMessage(...)`, Swift:
    `ToriiClient.postBridgeMessageSubmitJson(...)`, Kotlin:
    `HttpClientTransport.postBridgeMessageSubmitJson(...)`, and Java Android:
    `HttpClientTransport.postBridgeMessageSubmitJson(...)`.
- `GET /v1/sccp/messages/recent` exposes newest-first committed SCCP message discovery with compact metadata, decoded payload projections when available, and direct links to the existing bundle / artifact / job endpoints.
- `POST /v1/bridge/messages` now also accepts an optional `settlement` object:
  - it resolves a deployed contract target by `contract_address` or `contract_alias`;
  - it appends an ephemeral by-call trigger after proof verification so settlement can happen in the same submitted transaction; and
  - when `payload` is omitted for `finalize_inbound`, Torii auto-builds `finalize_inbound(route, message_id, recipient, amount)` from the `transfer` message bundle and requires the proof-derived `route_id` to decode as a logical `Name`;
  - when `payload` is omitted for `activate_route_governed`, Torii auto-builds `activate_route_governed(message_id, route, asset_key, remote_domain)` from the `route_activate` message bundle and requires both the proof-derived `route_id` and `asset_id` to decode as logical `Name`s;
  - explicit `settlement.payload` is rejected for those proof-managed bridge entrypoints, so callers cannot bypass the proof-derived settlement inputs with raw custom payloads.
- Automatic settlement is still opt-in per request. Cross-node policy for always-on contract dispatch remains a higher-level integration choice outside this endpoint.
- The CLI now exposes read-only SCCP discovery helpers under the bridge feature:
  - `iroha ops bridge sccp capabilities`
  - `iroha ops bridge sccp manifests`
  - `iroha ops bridge sccp artifact --message-id <hex>`
  - `iroha ops bridge sccp job --message-id <hex>`
  - `artifact` / `job` also accept `--network-id-hex`,
    `--verifier-address-hex`, `--bridge-address-hex`,
    `--verifier-code-hash-hex`, `--verifier-key-hash-hex`,
    `--tron-verifier-address`, and `--proof-bytes-hex` so operators can fetch
    deployment-bound EVM/TRON Groth16 submission packages without dropping to a
    language SDK;
  - text mode prints compact chain/proof summaries, and `artifact` / `job` now also decode the normalized payload projection, verifier backend, and generated chain-specific submission package when they are present;
  - JSON mode emits the raw typed payload/JSON route response.

- `GET /v1/zk/proofs` and `GET /v1/zk/proofs/count` accept bridge-aware filters:
  - `bridge_only=true` returns only bridge proofs.
  - `bridge_pinned_only=true` narrows to pinned bridge proofs.
  - `bridge_start_from_height` / `bridge_end_until_height` clamp the bridge range window.
- `GET /v1/zk/proof/{backend}/{hash}` returns bridge metadata (range, manifest hash, payload summary) alongside the proof id/status/VK bindings.
- The full Norito proof record (including payload bytes) remains available via `GET /v1/proofs/{proof_id}` for off-node verifiers.

## Bridge receipt events

Bridge lanes emit typed receipts via the `RecordBridgeReceipt` instruction. Executing this instruction
records a `BridgeReceipt` payload and emits `DataEvent::Bridge(BridgeEvent::Emitted)` on the event
stream, replacing the prior log-only stub. The CLI `iroha bridge emit-receipt` helper submits the
typed instruction so indexers can consume receipts deterministically.

Outbound SCCP traffic is recorded separately through `RecordSccpMessage`. The instruction carries
canonical SORA-origin SCCP payload bytes and remains permissionless for valid bridge flows, but it
is accepted only while applying a verified `Executable::IvmProved` overlay. Bare
`RecordSccpMessage` transactions and non-SORA-origin payloads fail during execution, still follow
the normal rejected-transaction fee path, and do not contribute to the block-level
`sccp_commitment_root`. Proposal assembly derives the root only from proved-overlay records.

## External verification sketch (ICS)

```rust
use iroha_data_model::bridge::{BridgeHashFunction, BridgeProofPayload, BridgeProofRecord};
use iroha_crypto::{Hash, HashOf, MerkleTree};

fn verify_ics(record: &BridgeProofRecord) -> bool {
    let BridgeProofPayload::Ics(ics) = &record.proof.payload else {
        return false;
    };
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(ics.leaf_hash));
    let root =
        HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(ics.state_root));
    match ics.hash_function {
        BridgeHashFunction::Sha256 => ics.proof.clone().verify_sha256(&leaf, &root, ics.proof.audit_path().len()),
        BridgeHashFunction::Blake2b => ics.proof.clone().verify(&leaf, &root, ics.proof.audit_path().len()),
    }
}
```
