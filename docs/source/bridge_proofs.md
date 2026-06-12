# Bridge proofs

Bridge proof submissions travel through the standard instruction path (`SubmitBridgeProof`) and land in the proof registry with a verified status. The current surface covers ICS-style Merkle proofs and transparent-ZK payloads with pinned retention and manifest binding. Non-SORA SCCP message bundle readback from verified bridge records also requires the stored bridge-proof range to match the transparent artifact's finality height.

Torii now exposes two SCCP bundle families:

- `burn` bundles for the legacy fixed-width burn message path
- `message` bundles for the generic multi-chain SCCP payload family
  (`asset_register`, `route_activate`, `transfer`, `token_add`, `token_pause`,
  `token_resume`)

## SCCP launch scope

The active SCCP surface is limited to Ethereum, BSC, Solana, TON, and TRON.
Retired runtime-network families outside that launch scope are not supported
for now.
SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now.
No current source proof, manifest, SDK helper, or Torii route should be treated
as Sub&#115;trate/Pol&#107;adot-compatible.
That exclusion is intentional current-launch scope, not a hidden compatibility
lane.
Torii public SCCP discovery, proof manifests, route readiness, SDK helpers, and
operator scripts must advertise only those lanes. Unsupported domain ids fail at
the absent-manifest/backend boundary rather than routing through diagnostic
relay paths.
The release source inventory pins the Torii OpenAPI SCCP capability and
manifest descriptions to the same no-support sentence so public discovery cannot
silently imply hidden Sub&#115;trate/Pol&#107;adot compatibility.
Retired runtime-network families are not supported for now. Future support
requires a new source-proof design, fresh fixtures, SDK/Torii surface review,
and explicit governance approval rather than reviving diagnostic code paths.
The retired-network surface guard requires explicit no-support launch-scope
wording in the docs and status files before release evidence can pass.
Generated release-readiness Markdown and verifier-owned release-bundle Markdown
also carry the exact no-support sentence so public operator artifacts cannot
imply hidden Sub&#115;trate/Pol&#107;adot compatibility.
The active launch policy is Ethereum-mainnet lane readiness. The active Ethereum launch lane
can open from complete mainnet source-proof, source-adapter deployment,
destination rollout, and route-canary evidence without waiting for future
lanes. Non-Ethereum lanes remain fail-closed until their own launch policy opens,
with the first-release Ethereum-mainnet launch policy preserving the same
unsupported-domain gates described above.
The release-readiness checklist treats that active-lane identity as evidence:
the normalized lane summary must stay on domain `1`, chain `eth`, report
`production_ready = true`, and carry boolean `true` flags for source verifier
material, source-adapter deployment, destination rollout, and route allowlist
records before the required-records item can become ready.
Stringified domain ids, padded chain labels, and stringified production-ready
flags are pinned adversarial cases for that active identity, so copied summaries
cannot satisfy the required-records item through type coercion or text
normalization.
Active launch readiness also treats route allowlist, source verifier material,
source-adapter deployment, and destination binding as separate evidence roles:
their canonical bytes32 hashes must all be non-zero and must not reuse the same
value before governed deployment, route-allowlist binding, or route-canary
checks can pass.
Rust route-allowlist and route-canary helper entry points apply the same role
separation when attaching lane canaries or hashing EVM/TRON transaction, Solana
ProgramData, and TON live-account canary transcripts. The Python operator
evidence scripts mirror those checks before rendering route allowlists or route
canaries, so direct helper calls cannot publish canary evidence with reused
route-allowlist/source-material/source-deployment/destination-binding hashes.
Python Torii-client, JavaScript source/dist, Swift, Kotlin/JVM, and Java Android
route-canary helpers must mirror that governed-hash separation before app-side
proof packaging, including TRON route-allowlist hashes derived from source,
deployment, and destination roles.
The release-readiness and release-bundle source inventory pins that
role-separation helper, SDK route-canary guard strings, and governed-deployment
plus route-allowlist hash-reuse regressions. The all-lanes release-checklist
inventory also pins the Rust direct-helper route-canary regressions for
route-allowlist and destination-binding replay, so removing report-level,
helper-level, or SDK packaging guards becomes a public release blocker.
The no-unresolved-blockers checklist also inspects the active lane's own
blocker list instead of trusting only the top-level aggregate, so lane-local
operator holds or malformed blocker entries keep release readiness blocked even
if a copied summary omits them from the aggregate list.
Release-bundle public blocker lists are schema-owned evidence: manifest,
readiness-report, corridor, release-checklist, embedded evidence, standalone
all-lanes, and lane blocker arrays must contain non-empty strings with no
surrounding whitespace, so hand-edited padded blockers cannot be normalized into
accepted release metadata.
Release-readiness and bundle verification pin that public blocker-list schema
as source inventory before published bundle readiness can pass, including
duplicate rejection, ready-surface empty-blocker checks, and invalid-marker
rendering for malformed blocker containers.
Sparse inventory checks remove root blocker, copied-corridor blocker,
padded/duplicate blocker, active-lane blocker, all-lanes root blocker,
release-note invalid-marker, readiness Markdown invalid-marker, and native
prover blocker regressions directly, so deleting any malformed-blocker test
blocks release readiness.
The bundle builder's not-ready preflight also treats empty, numeric, null,
padded, or duplicate root blocker entries as malformed before release notes or
public artifacts can be written.
Public scalar metadata uses the same rule for release-checklist item
ids/titles, corridor phase keys, cryptographic-evidence chain and route-canary
source labels, user-prover submission surface text, all-lanes lane chain
labels, destination-binding keys, and route-canary status/source fields.
Release-checklist item ids are a fixed public gate set; malformed ids are
classified before duplicate, drift, or Markdown-presence checks, while safe
unknown ids remain readable operator diagnostics.
Release-readiness and bundle verification pin that public scalar-text schema as
source inventory before published bundle readiness can pass, including exact
padded-value regressions for release-checklist titles, all-lanes chain labels,
destination-binding keys, route-canary status/source fields, cryptographic
route-canary source labels, and submission-surface text.
Sparse inventory checks remove the direct copied scalar field-type, padded
value, malformed field-name, malformed phase-key, copied corridor phase-map,
copied crypto-evidence, copied submission-surface, and top-level CLI redaction
regressions, so deleting those public scalar tests blocks readiness.
The same public schema classifies unknown all-lanes object keys and
source-adapter audit-hash keys before semantic matching, so padded,
control-character, whitespace, Markdown-unsafe, malformed, or
Unicode-confusable keys cannot leak into release diagnostics.
The all-lanes release checklist also treats route-canary `status` and
`evidence_source` as canonical strings before semantic matching, so padded or
non-string canary scalars stay schema blockers instead of ambiguous not-ready
text.
Release-readiness and bundle verification pin that all-lanes route-canary
scalar schema as source inventory before production evidence can pass.
All-lanes evidence-root schema rejection is pinned the same way and must remain
part of the strict release-bundle verifier's global source-marker sweep, so
malformed roots, unknown sections and their literal blocker assertions, or
non-string section-key tests cannot be removed while a hand-edited
source-inventory row still claims readiness.
The release-bundle builder applies the same copied-summary shape gate before
rendering public artifacts, including duplicate `required_domains`,
`supported_launch_domains`, and `unsupported_launch_domains` entries.
Copied input provenance is checked before rendering too: padded input paths and
percent-encoded traversal in either `inputs` or `input_artifacts` are structured
blockers, and the raw copied path text is not echoed in operator diagnostics.
Copied `source_inventory` gate names are classified the same way before release
Markdown can render: non-string, padded, control-character, Markdown-unsafe, and
non-ASCII gate names become category diagnostics instead of raw public text.
It also blocks copied route-canary evidence hashes that replay another lane's
canary evidence, source-material, destination-binding, or route-allowlist hash
before release Markdown or JSON is rendered.
Destination rollout and route allowlist blocker containers are pinned the same
way: source inventory must retain the canonical blocker-list rejection path and
adversarial governed-blocker tests, including padded route-allowlist blocker
entries, before governed evidence can pass.
The active launch route-canary evidence source is also schema-owned evidence:
missing, empty, padded, or non-string values block release readiness before the
exact `evm_message_proof_accepted_transaction` source match is checked.
Python package-root route-canary helpers also exercise the governed hash
role-reuse negatives for Solana, TON, and TRON, so public `iroha_torii_client`
imports cannot bypass the deep SCCP helper role-separation tests.
The JavaScript published package root and checked-in package-dist entrypoint
mirror those same Solana, TON, and TRON role-reuse negatives.
The JavaScript published package root also mirrors the EVM-family/TRON
proof-request bundle gate, rejecting source-domain drift through
`buildEvmSccpProofRequest` and `buildTronSccpProofRequest` before app-side
prover callbacks can run.
Native EVM prover SDK artifact ids also reject surrounding whitespace in both
readiness generation and release-bundle verification, so a padded SDK name
cannot be reported as an unknown SDK while hiding the required canonical row.
Portal/mobile runtime SDK selectors for direct byte verification,
resolver-backed bundle loading, and native prover self-test preflights follow
the same canonical text policy before SDK artifact lookup or callbacks run.
Release-readiness and bundle verification pin those canonical native SDK-id
regressions across the public JavaScript, Kotlin/JVM, Java Android, and Swift
SDK tests before native prover evidence can pass, with sparse inventory checks
covering Kotlin/JVM and Java Android padded self-test callback non-run markers.
The native no-WASM/no-remote sparse checks also remove the browser no-WASM
guard, BSC browser guard, URI proof-artifact, WASM proof-artifact, and
remote-prover identifier markers from the JavaScript package distribution test,
so public browser package regressions cannot disappear while source inventory
still claims readiness.
Top-level all-lanes, release-readiness, and release-bundle CLI exception
handlers preserve structured validation categories but replace secret-looking,
control-character, empty, or OS-error payloads with fixed diagnostics before
stderr output.
Cryptographic-evidence rows preserve raw route-canary and source-adapter gate
boolean/container values from the normalized evidence summary, so malformed
truthy strings or wrong-shaped audit hash containers remain visible to release
bundle schema checks instead of being coerced into ready-looking values.
Missing future-lane route-canary bindings render as explicit boolean `false`;
present malformed binding values remain preserved for verifier rejection. Source
inventory pins the readiness-side malformed audit-container preservation
assertion before public cryptographic-evidence readiness can pass.

## User-side prover SDKs

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
The package root also exports `BscTestnetSccp` and `BscTestnetSccpProver` for
BSC testnet rollout; those helpers validate canonical `eth_chainId == 0x61`,
bind outbound proofs to network id `97`, and reuse the BSC-family Parlia
receipt-proof corridor for BSC -> SORA admission.
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
row, a matching implementation-byte hash from the signed native prover bundle,
and the bytes for the bundle's `cross_sdk_fixture_parity_artifact` and
`native_prover_self_test_artifact`; each SDK hashes those files against
`audit_hashes.cross_sdk_fixture_parity` and
`audit_hashes.native_prover_self_test`, parses the fixtures locally, and carries
the normalized parity and self-test vectors in the verified descriptor. The
self-test fixture also binds the request, witness, source-proof, proof,
calldata, Torii payload, destination-binding, bundle-hash, and public-signal
outputs that every native SDK must reproduce. The outbound Ethereum mainnet
facades now require an SDK-owned native prover self-test runner for that vector,
fail before production proof execution when the runner is absent, and reject
any runner output that drifts from the verified descriptor. JS/browser apps can
call the same check directly with `runEthereumMainnetNativeProverSelfTest(...)`
or through a configured facade's `runNativeProverSelfTest(...)`; Swift,
Kotlin/JVM, Java Android, and .NET expose matching startup preflight helpers
for native app integrations. Product integrations can fail closed during
startup before the first outbound proof request. JS/browser, Swift, Kotlin/JVM,
Java Android, and C# also expose
bundle-resolver helpers that accept
an app-owned local artifact resolver, load the manifest-declared proof artifact,
proving key, verifier key, cross-SDK parity fixture, native prover self-test
fixture, and selected SDK implementation bytes, and then run the same
hash/vector checks without a WASM or remote-prover dependency. JS/browser,
Swift, Kotlin/JVM, and Java Android
provide `EthereumMainnetSccp.fromNativeProverBundle(...)` helpers that verify
those local bundle resources and return Ethereum mainnet facades already bound
to the verified native artifacts; the C# static API exposes
`ProveOutboundToEthereumFromNativeProverBundleAsync(...)`,
`BuildEthereumCalldataFromNativeProverBundle(...)`, and
`SubmitOutboundToEthereumFromNativeProverBundleAsync(...)` for the same
resolver-backed proof, calldata, and submission path. The same verified
descriptor gate also runs when Ethereum mainnet calldata is built or submitted:
JS/browser, Swift, Kotlin/JVM, Java Android, and C# refuse the easy product path
unless the wrapped proof result is bound to matching native prover artifacts,
while the older generic EVM helpers remain available only for callers that
choose those explicit APIs.
The signed native prover bundle manifest parsers in JS/browser, Swift,
Kotlin/JVM, Java Android, and C# also reject duplicate JSON object keys before
building descriptor objects, including escaped-key aliases, so app-side bundle
loading cannot depend on last-key-wins parsing. Release-readiness and strict
release-bundle verification also keep malformed or non-UTF-8 native prover
manifest, cross-SDK parity fixture, and native self-test fixture diagnostics
category-only, so parser exception payloads or local artifact text cannot leak
through public readiness blockers.
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
The JavaScript BSC testnet facade mirrors those binding checks for EVM chain id
`97`, rejects mainnet proof results on the testnet calldata path, and includes
`chainId: "0x61"` when it falls back to an EIP-1193 `eth_sendTransaction`
submission. JavaScript BSC mainnet and testnet outbound submit paths also
validate any configured execution provider before invoking app-owned submit
callbacks, so a wrong-chain provider cannot be silently bypassed by a custom
transaction hook.
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
matching the presence-aware Solana and TON wrappers.
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
source-adapter deployment-binding hash, so stale or mutated UI prover results
cannot be silently rewrapped for a different on-chain submission. For
request/envelope hash, public-input, proof-context,
statement/destination-binding hash, and EVM-family/TRON public-signal fields
are strict when present; a `null`/`None` field is rejected instead of being
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
request byte fields and proof bytes, so mobile callers cannot wrap proof bytes
proof-result wrappers reject all-zero external proof bytes before deriving the
request-bound envelope, and TON message-body builders apply the same proof-byte
preflight before packaging BOC submissions. The EVM-family, TON, and TRON
request preimages length-prefix both `bundleBytes` and `sourceProofBytes`, so
the same raw byte sequence cannot be replayed under a different
bundle/source-proof split, and those boundaries are enforced across web,
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
- Transparent OpenVerify summary helpers use the same production-shaped wrapper
  policy before reporting metadata: exact transparent circuit id, non-zero
  verifier-key hash, non-empty schema and public-input columns, no auxiliary
  envelope bytes, and non-empty nonzero backend proof bytes are all required.
- Artifact-level transparent summary helpers also validate the full typed SCCP
  message proof artifact wrapper before reading proof metadata, so manifest
  field drift, public-input drift, or tampered submission packages cannot be
  summarized from otherwise well-shaped OpenVerify proof bytes. Torii and CLI
  artifact JSON/summary renderers consume this gated helper and omit the
  OpenVerify summary when the typed artifact wrapper is inconsistent.
- Source-adapter OpenVerify proof envelopes are capped at 2 MiB before decode or
  FastPQ replay, matching the bound used for source-state proof capsules.
- Source-adapter verifier-commitment metadata helpers apply the same outer
  `SccpSourceAdapterVerificationProofV1` and inner OpenVerify/STARK shape gate
  before returning an embedded verifier-key hash, so malformed proof wrappers,
  opaque proof bytes, zero verifier keys, auxiliary envelope bytes, or empty
  STARK public-input columns cannot be reported as deployment commitments.
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
- Non-SORA-origin messages (`remote -> SORA/Nexus or another supported target`)
  carry a Norito-encoded `SccpSourceChainProofEnvelopeV1`. Raw Nexus finality
  bytes are rejected for these messages, and source-chain envelopes are rejected
  for SORA-origin messages. Source-aware bundle validation, public-input
  derivation, package builders, proof-job builders, transparent-proof builders,
  and final transparent-proof verification also reject any caller-supplied
  source verifier material or source-adapter deployment context on SORA-origin
  bundles before deriving public inputs, so external source-adapter evidence
  cannot be spliced into Nexus-finality-backed outbound messages.

`SccpSourceChainProofEnvelopeV1` has this canonical data shape:

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
Material-only evidence verification also requires those deployment fields to
remain zero; deployment-looking hashes without the matching deployment context
are rejected instead of being ignored.
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
rendered. The receipt-proof evidence regression pins leading-zero, uppercase,
whitespace-wrapped, and numeric `eth_chainId` responses before local source
proof evidence can be accepted, and the same noncanonical vector is now pinned
across Swift, Kotlin/JVM, Java Android, and C# inbound-collection tests. It also
caps successful JSON-RPC responses and HTTP error details before decoding,
rejects duplicate JSON object keys instead
of accepting last-value-wins parsing, and requires every success envelope to echo
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
proof nodes, and verified receipts root pair. Receipt JSON-RPC diagnostics are
category-only for duplicate keys, HTTP bodies, transport reasons, and error
objects so provider details cannot enter public readiness output. By default,
the helper requires
`--source-bridge-address` and exactly one matching canonical
`SccpSourceEvent(bytes32)` log in the receipt before
`source_event_digest` is rendered. `--allow-receipt-only-evidence` is available
only for generic receipt-trie diagnostics; do not use receipt-only output as
SCCP source proof material.
Release-readiness and strict bundle verification also pin Python canonical
ETH/BSC receipt-proof transcript tests that reject cross-lane `sourceDomain`
values before a receipt-proof hash can be accepted.

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

the finality authority-set hash from ordered non-zero 32-byte Ed25519 authority
keys and non-zero weights, requires it to match the adapter and configured
source trust anchor for non-placeholder material, recomputes the finalized-header
precommit-message hash, checks the justification hash, verifies Ed25519
signatures over the precommit hash, and enforces strict `> 2/3` signed
authority weight before accepting the source-adapter evidence. If the configured
source trust anchor is a parent authority set rather than the active set, the
transition carries the canonical next authority-set payload
(`0x01 || authority_count_le || (ed25519_authority_key || weight_le)[0..n]`),
match the signed next set, and then requires the parent authority set to justify
that transition with strict `> 2/3` weight before the next set becomes eligible.
authorities, 64 authority-set transitions, canonical authority payloads no
larger than `1 + 4 + 2,048 * 40` bytes, exact-width signer bitmaps with no
padding/out-of-roster bits, non-empty signer sets, signature counts that match
selected signers, claimed total/signed weights that match the authority roster
and selected signers, strict `> 2/3` signed-weight quorum, and 64-byte Ed25519
signatures. The web, Python, Swift, Kotlin, and Java Android UI prover helpers
enforce the same bounds before deriving authority-set or transition transcript
hashes, and reject all-zero authority keys both from canonical inputs and raw
authority-set payloads.
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
EVM source-adapter deployment readiness also rejects replayed source trust
anchor, message-inclusion verifier, finality-policy, and source bridge runtime
code hashes before those records can satisfy source-adapter readiness or
deployment-bound proof matching.
BSC deployment-bound facade coverage also rejects coherent alternate
production-ready source material/deployment pairs for source trust anchor,
consensus verifier, message-inclusion verifier, finality policy, governed
source bridge emitter address/runtime code hash, and deployment receipt. Those
alternate BSC deployments remain generally source-adapter ready, but the
original source proof and local-admission artifact cannot match them, pass
deployment-aware production verification, pass bundle extraction, or survive
verifier-evidence splicing after OpenVerify binding.
Solana and TON deployment-bound production coverage also rejects replayed
full-light-client audit verifier role hashes for every governed audit role:
Tower replay, full AccountsDB lattice, and bank/fork-choice on Solana, and
masterchain config, validator-set transition, and shard-accounts dictionary on
TON. A replayed audited deployment may remain generally well-shaped and
source-adapter ready, but it cannot match a previously built source proof,
satisfy deployment-aware production verification, pass bundle extraction, or
survive verifier-evidence splicing after the OpenVerify statement is bound.
TRON deployment-bound production coverage applies the same exact-deployment
rule to coherent alternative source deployments: source trust anchor, consensus
verifier, message-inclusion verifier, source bridge emitter address, runtime
code hash, network id, owner, finality policy, and deployment receipt replays
are built as production-ready material/deployment pairs with valid DPoS source
gate hashes, then rejected for proof matching, deployment-aware verification,
bundle extraction, and verifier-evidence splicing.
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
network id, canonical TRON binding key, and binding hash, and runtime lane
readiness also requires that network id to match the governed source bridge
static destination binding key/hash and must not carry EVM/TRON network or
bridge-wrapper fields.
The ZK consensus policy hash includes the fields, so governed destination
binding evidence is committed by the policy digest instead of relying only on
operator comments. Runtime readiness requires exact verifier identities across
destination families: padded EVM addresses, Solana program ids, TON raw
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
Copied EVM destination runtime bytecode evidence is reparsed with category-only
diagnostics for both bridge and verifier runtime bytecode; malformed bytecode
parser details are not propagated into public TOML blockers, and release
source inventory pins adversarial copied evidence for both bytecode roles.
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
for JSON dry-runs. The release-readiness governed-deployment checklist also
requires the normalized active launch summary to carry canonical non-zero
source-material, source-deployment, destination-binding, and expected
destination-binding hashes, with the supplied destination binding matching its
recomputed expected value and the match flag set to exact boolean `true`.
Source-material and source-adapter deployment record hashes must also remain
role-separated, so copied summaries cannot reuse one hash across both records.
For the active EVM launch lane, source-adapter gate
metadata must remain absent/empty because no full-light-client source gate is
required, and source inventory pins the required/gate-hash/audit-hash blocker
strings plus the adversarial non-empty gate-audit regression before readiness or
strict bundle verification can pass. Public release-bundle source inventory also
pins copied cryptographic-evidence audit-key classification, including
Markdown-unsafe audit labels and confusable audit-key non-leak assertions, before
readiness or strict bundle verification can pass. The same public source
inventory pins copied submission-surface SDK/backend classification and
confusable SDK-key non-leak assertions, plus validation-status and
validation-blocker shape/coupling markers, before readiness or strict bundle
verification can pass. Native EVM prover source inventory pins malformed
validation-blocker container tests, no-character-expansion assertions, and the
blocked copied-summary pre-render regression for both readiness generation and
verifier recomputation. The release-readiness checklist
independently requires the normalized active launch summary to carry canonical
non-zero source-material, source-deployment, destination-binding,
route-allowlist, and expected
route-allowlist hashes, with the route hash matching its recomputed expected
value before the route-allowlist item can become ready. Source inventory also
pins the route hash mismatch, exact boolean expected-match flag, source-record
hash role-reuse rejection, and `route_allowlist.hash_mismatch` adversarial
regression before that checklist can pass. The strict bundle verifier mirrors
the same active source-material/source-deployment role separation, so a copied
summary cannot keep the governed-deployment item blocked while letting the
route-allowlist item appear ready. Omitting route allowlist arguments keeps JSON output in
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
The release-readiness checklist also gates the active launch lane on the
normalized route-canary transaction metadata: the evidence source must be the
EVM `MessageProofAccepted` transaction, the transaction hash, receipt block
hash, receipts root, and message id must be canonical non-zero bytes32 values,
the receipt block number must be positive, and the receipt block must be marked
finalized before the route-canary item can become ready.
Source inventory pins those transaction metadata blockers and the adversarial
block-receipts-root regression before readiness or strict bundle verification
can pass.
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
JSON-RPC collector caps successful responses before decoding and reports HTTP
failures, transport failures, duplicate JSON keys, and RPC error objects as
category-only diagnostics so provider payloads cannot enter public readiness
output. This matches the direct helper's exact integer policy for imported
ProgramData metadata:

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
hex, or base64 artifacts that carry ordinary file formatting.
Copied TON destination code BoC base64 evidence is reparsed with category-only
diagnostics, so malformed base64 parser details are not propagated into public
TOML blockers. The helper pins the exact `TonContractNativeRecursive`
destination plan, TON
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
characters. HTTP failures, transport failures, duplicate JSON keys, and TON
Center error objects are reported as category-only diagnostics, while duplicate
JSON object keys are rejected rather than parsed with last-value-wins semantics.
TOML output requires both
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

runtime code hash directly from read-only JSON-RPC with
head with `chain_getFinalizedHead`, reads `state_getRuntimeVersion` and the
well-known `:code` storage key at that finalized head, and derives
`verifier_code_hash` as BLAKE2b-256 over the finalized runtime WASM bytes. The
live summary, offline replay arguments, and rendered TOML preserve those
Production TOML rendering requires the finalized head, runtime code hash,
`specName`, `specVersion`, and `transactionVersion` to be pinned explicitly, in
addition to the governed destination-binding, route-allowlist, and route canary
evidence pins. The pinned runtime version values must be exact nonnegative
integers, not booleans. The live helper treats `specName`, expected `specName`,
runtime version text, finalized head hex, and runtime `:code` hex as exact
evidence; surrounding whitespace fails before runtime metadata or TOML
successful responses and HTTP error details before decoding, and rejects
duplicate JSON object keys instead of accepting last-value-wins parsing. JSON
dry runs include replayable `offline_evidence_args` and, once fully pinned, an
`offline_toml_sha256` digest for the deterministic TOML payload:

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
Runtime route-manifest parsing applies the same launch-lane pin before accepting
a production-ready TAIRA XOR TRON record: `route_id`, `counterparty_domain`,
`asset_key`, `tron_network`, `chain`, `chain_id_hex`, `network_id_hex`,
`destination_binding_key`, `verifier_target`, and the TAIRA burn-record
settlement asset/verifier-key/gas profile must match the governed mainnet lane.
The parser normalizes uppercase or whitespace-wrapped mainnet chain/network ids
to their canonical values, recomputes the dynamic TRON destination-binding key
and hash from network id, verifier address, verifier code hash, and verifier
key hash, then rejects foreign/testnet ids or stale binding/settlement metadata. Before
those governed metadata checks, TRON runtime parsing also requires the token,
bridge, source bridge, and destination verifier contract literals to be
canonical non-zero TRON Base58Check mainnet addresses and rejects duplicate
contract-role addresses.
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
TRON API failures, transport failures, duplicate JSON keys, and error objects
are reported as category-only diagnostics so provider payloads and duplicate
field names cannot leak into public readiness blockers. Top-level TRON live CLI
collection failures that contain sensitive operator context are likewise reduced
to a fixed collection-failed diagnostic.
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
emit replayable offline source-event args. TAIRA XOR route-manifest production
readiness also requires that source-event transaction readback to carry an empty
blocker list when `source_event_transaction_production_ready` is true; malformed
or contradictory blocker containers fail before a production-ready route manifest
can be rendered. Release-readiness and strict bundle verification pin those
TRON source-event transaction blocker regressions in the route-config source
inventory. The offline direct TRON renderer
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
rendering TOML; hand-edited JSON cannot bypass that evidence. Malformed
destination verifier runtime-bytecode metadata now reports a category-only
blocker rather than raw parser text. Diagnostic JSON may still use
`--no-getcontract` with an independently audited source code hash, but that
path cannot produce production-ready full TOML. The live helper
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
Standard `tomllib` parser failures are reported as category-only invalid-TOML
blockers, so parser payloads do not leak into all-lanes diagnostics.
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
source-event evidence can report production readiness. Duplicate-key failures
in those transition JSON inputs report a fixed category-only blocker, so the
duplicated key name is not echoed into operator or release diagnostics.
Solid-block header proof canonicalization failures also report a fixed
category-only blocker instead of copying lower-level proof encoder exception
text into public live-evidence summaries.
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
`--expected-runtime-storage-gate-hash`, emits that exact value as the
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
when any lane is incomplete. Direct validator calls also convert malformed
evidence roots or non-string section keys into structured blockers before
lane-level missing-evidence diagnostics are emitted. Destination rollout and
route allowlist `blockers` fields must be empty lists of non-empty canonical
strings; scalar, empty, padded, or non-string blocker entries become explicit
all-lanes blockers before launch readiness can pass. It also invokes each lane's canonical source
evidence validator, rejecting non-canonical source-adapter verifier keys and
template-derived component hashes before governance staging. It also rejects
source-material, source-adapter deployment, and Solana/TON audit records that
reuse a non-zero hash across verifier roles. Canonical source-validator
failures, source-record/source-gate/source bridge config hash recomputation
failures, destination binding hash recomputation failures, and destination
verifier identity parser failures are reported as category-only blockers in
all-lanes and release-readiness JSON, without echoing helper exception payloads
or operator-local context. TRON source-bridge and destination-verifier live
metadata parser failures and EVM source/destination runtime bytecode metadata
parser failures follow the same category-only rule, as do Solana ProgramData
account/executable, live route-canary ProgramData, TON code BoC, and TON
route-canary verifier identity parser failures, plus TRON route-canary
verifier-address parser failures. The TON live evidence helper applies the same
category-only rule to live accountStates address, live `code_boc`, and imported
`code_boc_base64` parser failures before rendering destination or full-rollout
TOML. The SCCP source, destination, receipt-proof, and live-evidence helper CLIs
also redact sensitive top-level failures to fixed evidence categories before
emitting public operator errors. The release public scalar-text source inventory
now pins the adversarial `secret-token` inputs for EVM live/source-live runtime
metadata, TON live transport and imported address/LT parser failures, TRON
live duplicate-key/solid-block proof failures, and all-lanes Solana
live/route-canary ProgramData base64/parser failures, so keeping only a generic
redaction assertion is not enough to satisfy readiness. EVM receipt/live/source-live,
Solana live, and TON live duplicate-JSON diagnostics also use fixed method or
endpoint categories with suppressed exception chaining, so duplicate key names
and nested parser context cannot leak through operator tracebacks. Imported live
metadata reparsing for EVM hex fields, Solana verifier identity/executable
fields, and TON live address/transaction-LT fields follows the same fixed-category
rule before public TOML or release summaries are produced. Direct Solana and TON
destination verifier identity parser redaction tests are also source-inventory
pinned with their adversarial parser-detail payloads, so those importable helper
paths cannot silently lose parser-detail coverage. Direct Solana and TON destination
verifier identity reparsing also reports fixed metadata categories instead of
copying lower-level parser text. The all-lanes preflight likewise reduces
Solana live ProgramData and route-canary base64 comment decode failures to fixed
metadata categories before release blockers are aggregated.
Destination rollout records are also checked against each lane's canonical verifier identity format instead of
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
(`evm_source_gate_hash`, `solana_full_light_client_gate_hash`,
`ton_full_light_client_gate_hash`, or `tron_dpos_source_gate_hash`), not just
any role hash in the audit bundle. It also preserves the destination
binding metadata comments emitted by the rollout helpers. EVM-family rollout
snippets carry explicit destination network id, bridge wrapper address, and
binding hash fields, so the all-lanes preflight recomputes the SORA -> ETH/BSC
binding from the governed verifier address, bridge wrapper, network id, code
hash, and Groth16 key hash. It also requires the EVM live helper's RPC chain id
and bridge/verifier runtime code-hash metadata comments plus replayable
runtime-bytecode hex comments whose Keccak-256 hashes match those code hashes,
rejecting offline/manual ETH/BSC destination TOML that lacks live bytecode
evidence. EVM live JSON-RPC diagnostics are category-only for HTTP bodies,
transport reasons, duplicate keys, and error objects so provider payloads cannot
leak into public readiness blockers. Imported EVM live source/destination
summary runtime-bytecode metadata is reparsed with category-only diagnostics, so
malformed copied bytecode cannot leak parser text into public TOML blockers.
ETH/BSC source snippets likewise require the source live helper's RPC chain id,
bridge address, runtime code-hash metadata, replayable source bridge
runtime-bytecode comment, and governed `evm_source_gate_hash` before source
material can pass launch preflight. Solana, TON,
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
helper's finalized head, runtime `specName`, `specVersion`,
`transactionVersion`, runtime code-hash metadata, and replayable runtime-code
base64 comments. It decodes the runtime code, recomputes BLAKE2b-256, and
checks that the replayed hash equals both the live runtime code hash and the
finalized runtime evidence remains diagnostic and is rejected for all-lanes
recomputed from that same finalized runtime evidence, and public release-bundle
verification rejects zero or governed-hash-reused finalized-head/runtime-code
route-canary fields in the published readiness JSON. Missing or drifting
destination binding metadata fails the preflight before governance staging. The
route allowlist hash is also recomputed from the canonical source material
record hash, source-adapter deployment record hash, and destination binding
hash, so a stale or unrelated route policy hash cannot open a different lane
evidence tuple. If any of those component hashes cannot be recomputed from
production-shaped records, route evidence is reported as unbound instead of
being recomputed from zero placeholders; the public blocker stays category-only
so recomputation helper exceptions cannot leak operator-local context through
all-lanes or release-readiness JSON. Each route allowlist record must also
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
destination digest cannot be replayed as the post-deploy canary evidence. The
all-lanes preflight, core configured runtime admission gate, and Torii
configured proof APIs also require that canary evidence hashes are unique
across all advertised lanes and do not reuse another lane's source material or
source-adapter deployment record hash, and public release-bundle verification
repeats those cross-lane checks against the published all-lanes JSON. One
successful post-deploy route canary therefore cannot be replayed as proof for
another lane. The preflight is an offline operator check; it does not need
signing keys or live-chain credentials.
TAIRA BSC XOR route-overlay generation also treats explicit post-deploy blocker
metadata as production evidence: `productionReady` route manifests require
`postDeployLiveEvidence` blocker arrays such as `productionBlockers`,
`postDeployProductionBlockers`, `fullTomlProductionBlockers`,
`sourceEventTransactionProductionBlockers`, and `routeCanaryProductionBlockers`
to be absent or empty, and malformed blocker containers fail closed before
Torii TOML is rendered. Route-manifest JSON string
fields are canonical at the record boundary: surrounding whitespace in route
ids, asset keys, network ids, post-deploy transaction ids, or offline full-TOML
hashes is rejected instead of being trimmed into accepted production metadata.
The release-readiness source inventory pins those BSC route-config guards,
including lowercase bytes32, lowercase EVM address, network metadata, and
adversarial manifest tests for uppercase `bscNetwork`, `chainIdHex`,
post-deploy transaction/offline-TOML hashes, and source-event transaction
blocker contradiction, post-deploy blocker contradiction, full-TOML blocker
malformation, or route-canary blocker malformation, before governed TAIRA XOR
overlays can satisfy production readiness.
TRON route-overlay generation is pinned the same way: release-readiness and
bundle verification require the source inventory to keep canonical JSON string,
lowercase bytes32, canonical Base58 address, network metadata, and adversarial
manifest tests in place before governed TAIRA XOR TRON overlays can satisfy
production readiness. The TRON route-config inventory also pins every
post-deploy blocker alias accepted by the route-manifest generator, including
source-event transaction, route-canary, full-TOML, and generic post-deploy
blocker arrays, plus the negative tests for scalar, malformed, and
contradictory blocker evidence.
Runtime TRON route manifests parsed from node configuration are pinned by
source inventory as well: the Rust parser must retain mainnet metadata checks,
dynamic destination-binding recomputation, Base58 address canonicalization, and
post-deploy anchor rejection before runtime config evidence can pass.
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
locator is absent. When `GRADLE_OPTS` is unset, Kotlin/JVM and Java Android
phases also export a default `-Xmx6g` Gradle and Kotlin-daemon heap corridor;
operator-provided `GRADLE_OPTS` continues to override those defaults. The full
corridor covers the Rust SCCP verifier crate, all
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
expected by the release report and bundle verifier. Empty `--log-dir` values
are rejected instead of falling back to a no-log run, so local release
rehearsals cannot silently skip transcript collection. Release source inventory
pins both the Gradle heap defaults and empty-log-dir rejection tests before
public readiness can pass. The Java Android phase runs
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
<phase>=<log>` arguments when phases are run separately. Use phase names
exactly as listed by the corridor runner and unpadded phase-result statuses;
padded, whitespace, Markdown-unsafe, or malformed phase names are rejected
instead of being trimmed into canonical phase names, padded or whitespace
statuses are rejected instead of being trimmed into canonical statuses, and
unknown phase names or statuses use category-only errors rather than echoing
operator-supplied text. Empty, control-character, non-ASCII, and
embedded-whitespace phase-result statuses are also classified before report
rendering or bundle copying, and the source inventory pins those CLI
regressions. Duplicate phase-evidence diagnostics redact local paths as
`<path>`, and missing `--phase-evidence-dir` logs report standard checked
layouts without echoing the operator-supplied directory. Attach the generated
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
For ETH/BSC rows, that public gate audit set is the single canonical
`evm_source_gate_hash`; the active Ethereum release checklist requires it to be
present, non-zero, and equal to the cryptographic-evidence gate hash before a
bundle can be marked ready.
It also records the user-prover SDK submission
surface for every lane, separating the EVM/TRON Torii bridge-proof submit
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
SHA-256 hashes for every attachment. Strict bundle verification re-reads the
copied phase logs after manifest generation, so a vanished or unreadable
corridor transcript blocks publication instead of relying on stale report
metadata. The release-note attachment explicitly
names `manifest.json` as the verifier root, so reviewers know to publish the
manifest alongside the hashed artifacts. It exits non-zero unless the strict
report is production-ready, so missing governed deployment evidence, missing
live canary evidence, or missing phase logs cannot be accidentally published as
a ready release. If `--force` is used to replace an output directory, the
builder refuses dangerous targets and refuses any output directory that contains
the input TOML or phase transcript sources, so evidence cannot be deleted before
it is copied into the bundle. Dangerous-root, repository-containing output,
`--force` containment, and existing-output diagnostics are category-only, so
rejected output roots do not disclose local output or input evidence paths. The
output directory itself and any existing
non-root output-path ancestor must not be a symlink before creation or forced
replacement, keeping release artifacts out of filesystem aliases. Those
output-path symlink diagnostics are category-only, so rejected bundle targets do
not disclose operator-local directory names. For
production-ready bundles, the builder now runs
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
user-prover submission surfaces are verifier-owned. Release-readiness plus
bundle verification pin the corridor phase-transcript
checks as required source inventory: exact phase markers, traced command
fragments, observed non-command completion and success output, dry-run
rejection, and forged-block rejection must stay in place before corridor logs
can satisfy public bundle readiness. The manifest and readiness-report JSON
roots reject missing or unknown top-level fields, and
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
their object shape while top-level `input_artifacts` must remain a list.
All-lanes source-adapter gate summaries use the same exact-boolean rule:
malformed `required` or `ready` values become governed-deployment blockers
instead of clearing through truthiness, and manifest comparisons against the
recomputed active launch checklist use the exact readiness value. Malformed
source-adapter gate blocker containers, including empty blocker entries, are
source-inventory pinned across direct checklist and generated-summary paths.
Malformed lane record, destination-binding, route-allowlist, route-canary, or
lane-local blocker containers also become explicit checklist blockers instead of
raising, hiding route-canary gaps, or letting no-unresolved-blockers pass. The all-lanes
route-canary scalar inventory pins the adversarial numeric and padded
`status`/`evidence_source` cases before release readiness or strict bundle
verification can pass. The
standalone readiness report also requires the active launch checklist `ready`
value to be exactly boolean `true` before top-level `production_ready` can be
published. Active EVM live source/destination chain ids must be canonical
decimal strings; JSON-RPC quantity spelling, leading-zero, whitespace-padded,
plus-signed, decimal-looking, Unicode-confusable, or numeric JSON values remain
readiness blockers. Source and destination values are checked independently so a
valid value on one side cannot mask noncanonical text on the other. The public verifier's
recomputed active launch checklist now mirrors
the report generator's exact metadata blockers for required record flags,
governed source/deployment/destination hashes, empty active EVM
source-adapter gate metadata, route allowlist binding, and route-canary
transaction evidence, so hand-edited truthy or malformed values cannot satisfy
manifest-vs-summary readiness comparisons. Required record flags must be exact
boolean `true`; copied `"true"`, numeric, false, or missing/null flags remain
all-required-record blockers in both readiness generation and strict bundle
recomputation. Route-canary status must be exactly `passed`; missing, empty,
padded, or non-string values keep the live-route-canary checklist item blocked
before transaction metadata can make the lane ready. Route-canary evidence
source must be exactly `evm_message_proof_accepted_transaction`; missing,
empty, padded, non-string, or canonical-looking wrong labels remain
live-route-canary blockers. Route-canary evidence hash,
transaction hash, receipt block hash, block receipts root, and message id must
all be canonical lowercase non-zero `0x` bytes32 strings; missing, zero,
uppercase, or non-string values are live-route-canary blockers. Route-canary receipt block numbers
must be exact positive integers, not numeric-looking strings, hex text,
plus-signed text, Unicode-confusable text, or booleans, and finalized receipt
metadata must be exactly boolean `true`; false, missing/null, copied truthy
strings, or numeric values remain live-route-canary blockers. Route-canary evidence binding must
also be exact boolean `true`; truthy strings, numeric values, false, or
missing/null flags keep the live-route-canary checklist item blocked.
Release-readiness and
bundle verification pin that active checklist schema as source inventory before
production evidence can pass. Required-record summary unknown keys are classified
before checklist text is rendered, so padded, control-character, whitespace,
Markdown-unsafe, or Unicode-confusable local record names become category-only
blockers instead of raw public diagnostics. The
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
accepted as standalone release-manager assertions. Bundle generation preserves
the report's exact readiness values in those manifest fields instead of
truthy-coercing malformed strings or numbers into public `true` claims, leaving
schema validation to reject wrong-shaped readiness metadata. Release-readiness
and bundle verification pin exact manifest readiness flag generation, verifier
boolean rejection, manifest/report equality checks, and all-lanes readiness
recomputation as source inventory before published bundle readiness can pass.
Sparse inventory checks remove the malformed readiness-value, boolean-type
drift, manifest-claim drift, pre-write manifest drift, and summary launch-ready
regression tests directly, so deleting any of those adversarial checks blocks
release readiness.
Release-readiness and bundle verification also pin required artifact paths,
manifest-root exclusion, unmanifested artifact/directory rejection,
report-referenced artifact closure, and canonical attachment order as source
inventory before published bundle readiness can pass.
Sparse inventory checks remove the direct manifest-root, symlink-root,
missing-manifest, duplicate-artifact, unmanifested-entry, unsupported-entry,
phase-artifact, extra-artifact, unknown-phase, order-drift, malformed copied
artifact, copied-hash drift, and pre-write manifest drift regressions, so the
bundle file set cannot lose those adversarial tests silently.
Strict bundle verification also keeps root-shape, missing-manifest,
unsupported-entry, bundle-enumeration, and unreadable phase-transcript
diagnostics category-only so local release paths cannot leak through public
verification errors.
Release-note
status rendering, bundle preflight publication checks, and verifier-owned
not-ready checks use the same exact-boolean rule, so a malformed truthy
`production_ready` value cannot publish or render as `READY`. Readiness report
objects returned from both the initial preflight build and the copied-evidence
bundle-local rebuild must also keep the required release-bundle structure before
Markdown rendering or manifest generation, so malformed internal reports fail as
explicit preflight errors instead of uncaught indexing exceptions. Readiness
report Markdown rows also use exact booleans for checklist items, lane production
status, lane record flags, route-canary binding labels, and native-prover
required labels, so malformed truthy row values render as blocked, unbound, or
record-missing rather than ready. Malformed top-level readiness blockers,
release-note blockers, release-bundle preflight blockers, native-prover
blockers, source-inventory blockers, or user-prover blocker containers render
as explicit invalid blocker cells/items instead of being flattened as strings
or raising during verifier-owned Markdown generation. Embedded readiness
evidence and standalone all-lanes root blocker summaries plus active-lane
blocker containers must also be list-shaped before active-launch blocker
collection runs, so malformed strings cannot become character-by-character
blockers or disappear from verifier checks. The active-launch checklist also
treats malformed active-lane blocker containers as schema blockers in the
governed-deployment, route-allowlist, and live-route-canary buckets before
category matching runs, so non-list, padded, or non-string entries cannot leave
category readiness looking clean while only the aggregate blocker gate fails.
The active no-unresolved-blockers collector uses the same canonical string rule
for embedded evidence root blockers and active-lane blockers, so empty, padded,
numeric, null, or otherwise non-string entries produce schema diagnostics
instead of ambiguous free-form blocker text.
The all-lanes checklist uses the same canonical string policy for lane-local
blocker summaries: scalar, padded, or non-string entries become explicit
live-route-canary and unresolved-blocker diagnostics, while valid route-canary
blockers remain visible in both buckets.
Verifier-owned Markdown
invariants also require checklist, lane, native-prover, source-inventory,
user-prover, and top-level blocker text or invalid-marker cells/items to remain
visible, so a hand-edited attachment cannot hide readiness blockers while
preserving the surrounding table structure. Release-readiness and bundle
verification pin those public Markdown invariants as required source inventory,
so required sections, checklist/source-inventory blocker visibility,
invalid-marker rendering, malformed source-inventory gate-name, report-artifact
path, and cryptographic-evidence row-domain/audit-key suppression, and canonical
Markdown drift rejection cannot
be dropped before public bundle readiness passes. Sparse inventory checks remove
the direct public-section, blocker-text, invalid-marker, and malformed-label
redaction regression tests to prove those tests remain required.
Release-notes attachment invariants
likewise require the canonical title, exact readiness status line, manifest
handoff, artifact table entries, and blocker lines or invalid-marker bullets
before the canonical attachment comparison runs. Release-readiness and bundle
verification pin those attachment invariants as required source inventory, so
canonical title/status rendering, manifest handoff, artifact hash rows, blocker
visibility, and canonical drift rejection cannot be dropped before public
bundle readiness passes. Release-readiness and bundle verification also pin the
native EVM Groth16 prover manifest schema, readiness summary schema, artifact
hash/path binding, and bundled-manifest drift rejection as required source
inventory before public bundle readiness can pass. Native prover manifests must
publish exact booleans for `no_wasm = true` and
`remote_prover_required = false`; string, numeric, null, or missing variants
remain native prover bundle blockers in readiness generation and strict bundle
verification. Native prover manifest and
payload artifact path metadata failures now use fixed blockers in readiness
generation and strict bundle verification, so local path-validation details do
not leak through native prover validation output. The all-lanes evidence
summary uses exact
booleans for release-checklist aggregation, lane record-presence gates, and the
CLI success exit, and it requires route-canary summaries to carry canonical
non-zero evidence hashes plus the expected live evidence source for each lane,
so malformed truthy summary values cannot clear release preflight or CI checks.
Release-readiness and bundle verification pin that exact-boolean all-lanes
checklist surface, source-adapter gate hash/audit replay rejection, and
route-canary hash replay rejection as required source inventory before all-lanes
evidence can satisfy production readiness. The all-lanes evidence-root schema
is also pinned as release-critical source
inventory: malformed roots, unknown sections, and non-string section keys must
become structured blockers rather than raising or disappearing before lane-level
blockers are emitted, and the unknown-section blocker assertion is itself
source-inventory pinned. The
release checklist table must match the embedded all-lanes evidence summary, so
public release notes cannot rename, omit, or reorder checklist gates while
keeping the underlying evidence unchanged; checklist roots and gate rows also
reject unknown fields, malformed gate ids/titles, and malformed blocker lists
plus duplicate gate ids and duplicate blocker strings in the readiness report,
embedded all-lanes evidence, and standalone all-lanes summary, so operator
approvals cannot be hidden in ignored JSON members or ambiguous repeated rows.
The manifest
artifact set and order must
exactly match the required reports, copied evidence inputs, copied corridor
logs referenced by known passed phases in the readiness report, and final
release-notes attachment, so a hash-bound but unreviewed appendix, unknown
phase log, or regenerated artifact table cannot be smuggled into an
otherwise verified bundle. Manifest/report artifact membership, copied-artifact
integrity, manifest artifact-row validation, extracted bundle-entry validation,
manifested artifact symlink checks, and release-notes attachment diagnostics
are category-only for untrusted artifact paths, so an adversarial attachment
path cannot leak into public verifier output.
Native EVM prover bundle artifacts are also role-separated by path: the bundle
manifest, proof artifact, proving key, verifier key, cross-SDK parity fixture,
self-test fixture, and per-SDK implementation artifacts cannot reuse another
native prover role's file path in either the attached native manifest or the
published readiness-report summary, and the standalone readiness generator
reports the same blocker before a release bundle is built. The release bundle
builder also rejects duplicated native prover payload paths during input
validation, including `--allow-not-ready` runs, before creating a partially
copied bundle. These role-reuse blockers name the conflicting roles but do not
echo the reused manifest-relative path.
Duplicate top-level evidence input diagnostics redact local paths as
`<path> duplicates <path>` before source copying, so canonical-path aliases
cannot leak operator directory names while still failing closed.
Symlinked source-input and symlinked source-ancestor diagnostics are also
category-only before source copying, so evidence, phase-log, native prover
manifest, and native prover payload source checks do not expose operator-local
paths while still rejecting the bundle.
The verifier owns the production-corridor phase
inventory and requires every known phase to be marked `passed` with a
hash-bound artifact at the canonical
`corridor/<phase>.log` path, so a tampered readiness JSON cannot skip, move, or
remove one phase while leaving top-level ready flags true. The corridor section
also rejects unknown root fields and non-empty blockers, so operator
attestations or unresolved phase blockers cannot be hidden beside the phase
status and evidence maps. The verifier also owns and recomputes the exact
user-prover SDK submission surface table from the corridor phase results, the
user-side proof backend labels, the full per-lane/per-SDK helper inventory, and
the expected on-chain submission text (`sccp-solana-recursive-mainnet-v1`,
`ton-contract-v1`, `evm-groth16-bn254-v1`, and `tron-groth16-bn254-v1`), so
public release notes cannot claim a portal or mobile proof path is validated
unless its required SDK and contract-smoke phases
actually passed, and a weakened report generator cannot define a shorter
helper table as canonical. The Solana destination manifest still uses `solana-program-v1`
as the target verifier backend; the release-readiness surface uses the recursive
backend id that browser and mobile provers must put in the proof request. The
surface rows also name the lane-local user proof-generation helpers: EVM/BSC
canonical receipt-proof byte/hash helpers, TRON receipt-state and
transaction-source proof helpers, Solana and TON source-state request builders,
per-role Solana and TON full-light-client audit request builders, aggregate
full-light-client audit request builders and source-state prover facades, and
cannot present the final
runtime call envelope as portal/mobile-ready while omitting the user-side native
proof-generation helpers. Those
portal/mobile submission rows
must also keep canonical JSON field shapes: lane/backend/helper/submission
labels are non-empty strings and required phases are lists of non-empty strings.
Their required-phase lists must also match the verifier-owned lane policy
exactly, including `dotnet-sdk` only for the EVM/BSC native surface and
`contract-smoke` only for contract-backed EVM-family/TRON rows; extra,
reordered, or missing known phases fail before public artifacts can pass.
For a production release bundle, the row-level validation status must be
`passed` and `validation_blockers` must be empty, so a blocked portal/mobile
proof path cannot hide behind top-level ready flags.
Native EVM prover summaries follow the same public bundle rule: copied summaries
with blocked validation status or non-empty validation blockers fail before
readiness Markdown or JSON can be rendered.
The bundle builder also rejects symlinked source inputs or source-path
ancestors before copying evidence TOML, corridor phase logs, native prover
manifests, or native prover payloads into the release attachment, including
`--allow-not-ready` diagnostic bundles. Source paths and output directories
containing ASCII control characters are likewise rejected during input
validation before any bundle directory is created, with category-only
diagnostics that preserve the offending control-byte label without disclosing
local path text. Public release artifact
paths, copied source filenames, and native prover manifest-relative payload
paths containing Markdown-unsafe characters (`|`, backticks, `<`, or `>`) are
also rejected before they can enter readiness Markdown, release-note tables, or
strict-verifier diagnostics. Generated release artifact-path diagnostics and
copied source filename diagnostics for those characters are category-only, so
local filenames do not leak through stderr or public verifier output. The same
path-text gate rejects percent-encoded
and recursively over-encoded parent-directory segments in copied source
filenames and native EVM prover manifest-relative payload paths before source
copying, with category-only diagnostics, and in generated artifacts, manifest
rows, readiness-report provenance paths, and extracted bundle entries. Source
inventory pins the percent-encoded native EVM prover payload regressions in both
readiness generation and strict release-bundle verification. Sparse inventory
checks now remove the direct release-artifact path, copied filename,
manifest/report path, native prover payload path, symlinked artifact, extracted
bundle entry, and secret path-redaction regressions, so deleting those
artifact-path text tests blocks readiness.
Native EVM prover manifest-relative payload path diagnostics are also
category-only for control characters and Markdown-unsafe characters across the
bundle builder, readiness generator, and strict verifier, so a malformed
published native prover manifest cannot echo operator-supplied artifact names.
Missing, non-regular, unreadable, or forbidden-marker-scan-failed native prover
payload diagnostics are category-only as well.
Readiness-report input and input-artifact provenance diagnostics are also
category-only for duplicate, escaping, layout, control-character,
Markdown-unsafe, padded, percent-encoded, and copied-input recomputation
failures, so untrusted JSON path values cannot leak into bundle or verifier
output.
The verifier also
rejects
non-directory or symlinked bundle roots, non-canonical or escaping manifest
paths, a symlinked `manifest.json`, self-listed `manifest.json` artifact rows,
symlinked artifacts, unmanifested directories, duplicate, unmanifested, or
omitted required artifacts,
non-canonical manifest/readiness-report/summary JSON serialization,
duplicate keys and malformed duplicate-key names in public JSON roots,
non-UTF-8 public JSON and Markdown roots,
control characters or Markdown-unsafe characters in manifest,
readiness-report, or extracted bundle artifact paths, surrounding whitespace in
manifest, readiness-input, copied source filenames, readiness-report artifact,
generated artifact, or native prover manifest/payload paths, percent-encoded traversal
in copied source filenames, manifest, readiness-input, readiness-report
artifact, generated artifact, extracted bundle, or native prover manifest/payload paths,
unknown corridor phase statuses or evidence keys,
blocked corridor roots,
non-canonical corridor phase-log paths,
zero or malformed artifact byte counts and malformed artifact hash JSON types,
malformed readiness/checklist boolean JSON types,
report/manifest byte or SHA-256 drift for input and corridor phase artifacts,
release notes that omit the manifest handoff, standalone-summary drift from the
report's embedded evidence, empty or non-object report/summary JSON roots,
malformed readiness sections, missing or empty copied input-artifact lists,
malformed or duplicate input-provenance paths, input-provenance drift from the
copied evidence artifacts, copied evidence layout drift from `evidence/NN-*.toml`,
non-canonical readiness-report artifact paths,
missing or unknown manifest/readiness-report top-level fields,
malformed manifest/readiness-report top-level field names,
unknown embedded or standalone all-lanes summary root or lane fields,
malformed all-lanes required-domain or blocker scalar lists,
all-lanes required-domain drift from published lane domains,
all-lanes domain roster or chain-label drift from the production remote lanes,
non-ready or blocked all-lanes root or lane summaries,
missing-record lane flags,
blocked required source-adapter gates, required source-adapter gate summaries
that omit the named gate hash or expected audit hash roles, duplicate or
governed-hash-replayed source-gate audit roles, non-required gate summaries
that carry forged hash material,
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
expected destination/route hash drift,
route-canary route/destination hash drift from sibling lane evidence,
duplicate, unknown, or missing required cryptographic evidence domains,
malformed cryptographic-evidence row field names,
cryptographic evidence row domain/chain drift, or per-field
source/destination/source-gate/route/canary drift from embedded lane rows,
unknown or malformed manifest or report artifact fields,
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
The release-bundle builder applies the same malformed-name classification to
copied report-artifact rows, release-checklist root/item fields, and corridor
root fields before Markdown rendering, so hostile local labels become
category-only blockers instead of raw diagnostics.
Release-readiness and bundle verification pin the public artifact-row schema as
required source inventory, so unknown artifact claims, zero, negative, or
non-integer byte counts, and noncanonical SHA-256 text regressions cannot be
removed before published bundle readiness passes.
Release-readiness and bundle verification pin the copied input-provenance schema
as required source inventory, so canonical copied input paths, unique
input/input-artifact provenance, `evidence/NN-*.toml` layout, and recomputation
from copied TOML cannot be dropped before public bundle readiness passes.
Sparse inventory checks remove the direct missing-input, malformed copied
provenance, input path drift, provenance schema drift, report-artifact path
drift, copied layout drift, no-usable-input, and secret path-redaction
regressions, so deleting those copied-input tests blocks readiness.
Release-readiness and bundle verification also pin the public JSON-root schema
as required source inventory, so canonical manifest/readiness/all-lanes JSON
serialization, duplicate-key rejection with malformed-key classification,
category-only non-UTF-8, load, parse, and canonicalization diagnostics that do
not echo local bundle paths or parser exception payloads, and malformed
manifest/readiness root-field classification cannot be dropped before public
bundle readiness passes. Strict verifier source-inventory read and UTF-8
decode failures follow the same category-only rule, so a broken marker scan
cannot disclose local source paths or OS/decoder exception payloads.
Copied readiness-report root unknown field names are classified before bundle
rendering, so padded, control-character, whitespace, Markdown-unsafe, or
Unicode-confusable local root claims become category-only blockers instead of
raw public diagnostics. Copied source-inventory row unknown fields use the same
classifier before source-inventory blocker rendering, preserving readable
operator notes while suppressing hostile row names, and copied source-inventory
rows with blocked statuses or non-empty blockers fail before rendering public
artifacts. The JSON-root source inventory pins those copied-row status,
blocker-shape, duplicate-blocker, and empty-blocker regressions directly.
Release-readiness
source-inventory gate helper failures are also category-only, so missing or
failing verifier helper calls cannot echo local paths or exception payloads into
public readiness blockers. The
strict verifier also reports duplicate integer entries in copied all-lanes
domain lists directly, so duplicated `supported_launch_domains` or
`unsupported_launch_domains` fail before relying only on later launch-scope set
checks. Copied all-lanes summary, lane, and record unknown field names are
classified before bundle rendering, so padded, control-character, whitespace,
Markdown-unsafe, or Unicode-confusable names become category-only blockers
instead of raw public diagnostics.
The manifest artifact-set/order inventory also pins malformed public artifact
field-name classification before release-note artifact tables can pass.
Release-readiness and bundle verification pin the public Markdown text schema
as required source inventory, so UTF-8 readiness/release-note Markdown loading
and canonical text drift rejection cannot be dropped before public bundle
readiness passes. The marker set also pins the bundle builder's pre-write
readiness Markdown and release-notes attachment drift rejections, so weakened
local renderers cannot publish public text before final verification. Both the
readiness Markdown and release-notes attachment tests require the builder to
abort before writing drifted public Markdown files. Strict
verifier and release-bundle builder diagnostics for
failed public Markdown loads, verifier-owned readiness/report rendering,
release-notes attachment rendering, release-checklist recomputation, native
prover summary recomputation, and user-prover submission-surface recomputation
are category-only; they do not echo local bundle paths or raw exception
payloads.
The cryptographic-evidence source inventory also pins row-key and audit-key
classification, including safe diagnostics for readable operator fields,
category-only diagnostics for malformed or Unicode-confusable row names, and
Markdown suppression for malformed row domains or source-adapter audit keys.
The release-bundle builder applies the same row-key classifier to copied
cryptographic-evidence rows before Markdown rendering, so padded,
control-character, whitespace, Markdown-unsafe, or Unicode-confusable local
row claims become category-only blockers instead of raw diagnostics.
The same pre-render copied-row gate rejects malformed domain/chain scalars,
boolean/null drift, optional bytes32 text, optional block-number fields, and
source-adapter audit-hash maps/keys before public Markdown or JSON artifacts are
written.
Release-readiness and bundle verification also pin public cryptographic-evidence
binding as required source inventory, so production-domain row inventory,
lane-field binding, strict row-schema enforcement, active-row audit-key
classification, canonical row recomputation, Markdown row-domain/audit-key
suppression, zero-hash/domain-policy/type-drift regressions, BSC testnet row
shape, and active route-canary binding rejection cannot be dropped before
public bundle readiness passes. Public cryptographic-evidence rows for EVM
route canaries must also keep `route_canary_evidence_bound` and
`route_canary_receipt_block_finalized` exactly `true` and keep
`route_canary_evidence_source` exactly
`evm_message_proof_accepted_transaction`; their
`route_canary_receipt_block_number` must also stay a positive u32 integer.
False, wrong-source, or oversized copied rows fail before release Markdown can
be rendered or strict bundle verification can pass.
Public cryptographic-evidence rows for TRON route canaries must keep
`route_canary_block_number` as a positive u64 integer and
`route_canary_block_timestamp` as a non-negative u64 integer; oversized copied
TRON metadata fails at the same pre-render and strict-verification boundary.
Public cryptographic-evidence source-adapter gate rows also enforce the
domain-specific audit-key policy before rendering, so Solana, TON, and TRON
rows cannot carry `source_adapter_gate_required = false`, stale gate hashes, or
unexpected/missing audit hashes through non-active public evidence paths.
Release-readiness and bundle verification also pin public submission-surface
binding as required source inventory, so lane/backend inventory, per-SDK helper
inventory, verifier-owned surface recomputation, and corridor-phase binding
cannot be dropped before public bundle readiness passes. Surface recomputation
failures use the same category-only public diagnostic policy as release
Markdown rendering failures. Copied rows with blocked validation status or
non-empty validation blockers are also pinned by source inventory and fail before
public Markdown or JSON can be rendered.
Unknown submission-surface row fields use the same structured field-name
classification in the verifier and the release-bundle builder before Markdown
rendering, so safe operator notes remain readable while padded,
control-character, whitespace, Markdown-unsafe, or Unicode-confusable row
claims become category-only blockers.
Submission-surface lane labels are schema-classified before lane inventory,
backend matching, helper inventory, or Markdown-presence diagnostics run, so
padded, control-character, whitespace, Markdown-unsafe, malformed, or
Unicode-confusable lane labels become category-only blockers instead of raw
public diagnostics. Submission-surface `proof_backend` values use the same
classification before backend mismatch or Markdown-presence checks, so hostile
backend ids cannot leak through secondary verifier messages while safe unknown
backend ids remain readable operator diagnostics. Submission-surface
`on_chain_submission` text is likewise checked against the verifier-owned
expected text for the row's lane before Markdown-presence checks, so copied
operator text or hostile submission labels cannot leak through public
diagnostics. Default and per-SDK helper symbols are also classified before
helper-string derivation, helper inventory, UI-hook matching, or
Markdown-presence checks, so table-breaking or confusable helper names become
category-only blockers. If a copied report corrupts the per-SDK helper map or
any helper entry, the readiness Markdown renderer emits an invalid-marker cell
instead of falling back to raw `sdk_helpers` text or rendering the raw helper.
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
Serialized SCCP transparent artifacts expose the same split through
deployment-bound recovery helpers: callers can recover typed proof artifacts
against exact governed source material and source-adapter deployment evidence,
and the diagnostic variant relaxes only the destination manifest gate. Backend
label drift, replayed deployment receipts, and source-adapter evidence mismatch
still fail before the artifact is returned.
Serialized source-chain proof envelope bytes now have the same governed recovery
surface. Raw finality-proof bytes must decode to the expected source and target
domains and pass the exact material plus source-adapter deployment gate before
the recovered envelope is returned; production recovery also requires the
inbound launch scope, SORA target, and deployment receipt binding. ETH/BSC/TRON
mainnet helpers, plus audited Solana/TON mainnet helpers, keep material-only
proofs, copied cross-domain proof bytes, non-SORA targets, and replayed
deployment receipts fail-closed; the TRON facade also keeps production recovery
on the transaction-Merkle source-call proof path instead of the retired
receipt-MPT fixture path.
Solana and TON audited source-adapter deployment builders attach the required
full-light-client role hashes while deriving the same canonical gate hashes used
by readiness. They reject non-lane material, all-zero role hashes, duplicate
role hashes, and hashes replayed from existing source material before SDK or
operator code can treat the deployment as production evidence.
ETH/BSC source-adapter deployment readiness also derives a canonical
`sccp:evm-family:source-gate:v1` transcript before opening the source lane.
That gate binds the governed source bridge address/code hash, source material
record hash, source-adapter deployment hash and receipt, adapter verifier
commitment, receipt-log policy, and the chain-specific Ethereum beacon or BSC
Parlia verifier prefixes. Target-domain drift, verifier-key replay,
source-bridge runtime-code drift, and role-hash reuse leave the gate absent.

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
little-endian order. The lower-level binding normalizers and diagnostic witness
fixtures may still represent both deployment fields as zero, exactly-one-zero
bindings are rejected, and JavaScript, Python, Swift, Kotlin, and Java Android
Solana proof request builders now reject zero/zero bindings so UI/mobile provers
cannot produce deployment-agnostic proof bytes. Production portal/mobile flows
must pass the configured deployment hash and deployment receipt hash.
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
before evidence hashing. Adapter-level preflight also rejects non-V1/non-TON
envelopes, wrong masterchain/basechain identifiers, zero masterchain/shard
sequence numbers, zero masterchain/shard file or block hashes, and zero
validator-set, config, shard-state, transaction, signature, or shard-proof roots
before shard-state, config, or validator-signature verification runs. SDK proof
builders enforce the same branch cap before serializing portal/mobile witness
transcripts. The config-proof helpers now require a bounded TON
`HashmapE 32 ^Cell` proof BoC that opens config parameter `34`, bind the
32-bit key width and opened value hash into the transcript,
decode the opened `ValidatorSet` cell into SCCP's canonical payload, and
require the decoded payload hash to match the supplied validator-set payload hash; the legacy
abstract config inclusion branch must be empty for SDK-generated proofs. The
TON validator-set helpers also reject all-zero Ed25519 validator keys on both
structured input and raw validator-set payloads, and TON signature-proof helpers
require the signer bitmap to have the exact validator-set width with no
padding/out-of-roster bits, a non-empty selected signer set, signature count,
claimed total/signed weights, and strict `> 2/3` signed-weight threshold to
agree before serializing a transcript.
Transition-signature helpers additionally require the outer parent validator-set
hash and transition-message hash to match the validator proof and transition
fields. TON validator-set transition structural preflight now applies the same
canonical payload decode, payload-hash, next-set hash, parent-roster hash,
transition message, nested validator-message, and transition-signature transcript
checks before Ed25519 validator verification. Non-empty transition chains must
also be internally adjacent by validator-set hash and seqno, use strictly
increasing masterchain seqnos, and end at the adapter's declared active
validator-set hash.
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
The JavaScript, Python, Swift, Kotlin/JVM, and Java Android canonicalizers also
reject TON capsules whose proof family is not `stark-fri-v1`, so a TON circuit
id alone cannot promote debug or alternate proof-family bytes into the
source-state transcript. JavaScript applies that gate before canonical source
and package-dist capsule bytes are produced, with package root and `./sccp`
package tests pinning the `debug-proof-family` rejection. Python also pins the
same rejection through the public `iroha_torii_client` package root.
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
different SCCP bundle after the local prover returns. The Rust TON proof-request
and proof-result path, plus the JavaScript, Python, Swift, Kotlin/JVM, and Java
Android SDK TON request builders, now decode `bundle_bytes` as canonical SCCP message
bundle bytes, match them to the transparent public inputs, and require non-SORA
source bundles to carry non-empty source-proof bytes before local proof
generation or wrapped-result submission. TON proof-request builders require
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
  `finality_set_id`, `block_hash`, `authority_set_hash`, `events_root`,
  `finality_justification_hash`, `storage_proof_hash`,
  `finality_justification`, and `authority_set_transition_proofs`.

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
header/witness signatures, stale adapter witness-schedule roots, solid-block
message hashes, witness-seal hashes, transition-domain/message/seal metadata,
transition chains, or transition payloads are rejected before canonical adapter
bytes are serialized. The adapter-level preflight recomputes the witness
schedule hash from the declared witness roster, the solid-block message hash
from the adapter roots, and the witness-seal transcript hash from the declared
seal material before deeper witness-signature verification runs.

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
ETH source-adapter preflight recomputes the active sync-committee root from the
declared committee roster, the signed sync-committee message hash from the
adapter finality/execution fields, and the aggregate-signature transcript hash
before deeper BLS signature verification runs.
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
The adapter-level preflight requires non-empty transition chains to be
internally adjacent by parent committee hash and sync period, keeps transition
slots no later than the adapter beacon slot, rejects transitions beyond the
adapter sync period, and requires the final transition to terminate at the
adapter's active sync-committee root and sync period before BLS transition-chain
verification runs.
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
deployment verifier also requires the ordered transition chain to terminate at
the adapter's declared `validator_epoch`; a chain that derives the same active
validator-set hash at an earlier epoch is rejected as stale. The
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
BSC source-adapter structural preflight applies the same signer certificate
shape to final and transition seals before transcript hashing: the signer
bitmap must have the exact roster width with no padding/out-of-roster bits, the
signer set must be non-empty, the signature count must match the selected
signers, claimed total/signed powers must equal the roster and selected signer
powers, and the selected power must satisfy a strict `> 2/3` quorum. The
adapter-level preflight also rejects non-V1/non-BSC envelopes and zero
block/receipt/validator-set/commit-seal roots before receipt MPT, Parlia seal,
or transition-chain verification runs. It also checks that the block number is
inside the declared Parlia validator epoch, recomputes the validator-set hash
from the declared validator roster, recomputes the commit-message hash from the
adapter block and receipt roots, and recomputes the commit-seal transcript hash
before deeper secp256k1 seal verification runs.
BSC transition structural preflight now also rejects non-V1/non-BSC transition
envelopes, non-adjacent validator epochs, transition blocks that are not the
Parlia epoch-start block for `to_validator_epoch`, empty transition
header/payload material, zero transition hashes, and transition seals whose
commit-message hash does not match the transition message hash before
transition-step verification runs. That preflight also decodes the advertised
next-validator payload, requires the payload hash and payload-derived next-set
hash to match the transition fields, parses the transition header RLP to prove
the same Parlia payload was advertised on-chain, recomputes the nested
ValidatorSet metadata proof hash from the transition state root, recomputes the
transition message hash, and recomputes the transition seal hash before deeper
MPT or secp256k1 signature verification. At the adapter level, non-empty
transition chains must be internally adjacent, strictly increasing by
transition block, no later than the adapter block, and terminate at the
adapter's declared active validator epoch and validator-set hash.
The nested BSC ValidatorSet metadata preflight similarly requires the V1
mainnet ValidatorSet contract address, canonical length slot, non-zero storage
root and metadata/value hashes, non-empty bounded length and per-validator
storage proofs, canonical per-validator storage slots, and storage-value hash
agreement before MPT metadata verification runs.
ETH sync-committee transition structural preflight now also decodes the
advertised next-committee payload, requires parent-roster, next-committee, and
payload-hash agreement, recomputes the transition message hash, checks the
nested sync-committee message hash, and checks the transition signature-hash
transcript before BLS transition verification runs. Non-empty ETH transition
chains must also remain period-contiguous, internally adjacent by committee
hash, no later than the adapter beacon slot, and terminal at the adapter's
active sync-committee root and sync period.

JavaScript, Python, Swift, Kotlin, and Java Android SDKs expose matching
user-side helpers for these adapter-bound proof hashes: EVM and BSC
receipt-proof transcripts, EVM-family structural receipt-root MPT values, ETH
sync-committee transition payload transcripts, BSC validator-set payload,
storage-value, metadata-proof, and transition-message transcripts, Solana
message-proof transcripts, TON shard-proof
transcripts, TON masterchain block-message/signature transcripts, TON
validator-set transition payload transcripts, TRON
receipt-root MPT values plus receipt-proof and receipt-state MPT transcripts,
storage-proof/authority-set transcripts. The ETH sync-committee and BSC
validator-set helper surfaces enforce the same Rust verifier bounds before
hashing UI witness material, so browser and mobile proof generators reject
oversized committee payloads, non-canonical signer bitmaps, claimed quorum
weight drift, sub-quorum certificates, signatures, and transition inputs before
invoking an app-linked prover. Solana Rust adapter preflight now applies
the same bounded-shape gate before source-adapter transcript hashing, so
oversized UI-collected finalized-vote, finality-context, account raw-data,
inclusion-branch, AccountsLtHash, or source-state proof material is rejected
before canonical adapter bytes are serialized. Adapter-envelope preflight also
requires V1 Solana source proofs, the Solana domain, non-zero finalized slot,
blockhash, bank hash, transaction-status root, message-proof hash, exact
non-zero transaction signature and emitter program id widths, a mainnet epoch
matching the finalized slot, adjacent parent/finalized slots, non-zero
finality-context roots, and a positive bank-signature count. The Solana
vote-certificate preflight also requires the vote-message hash to match the
Solana domain, finalized slot, blockhash, bank hash, transaction-status root,
message-proof hash, and finality-context hash, an exact-width signer bitmap
with no padding/out-of-roster bits, a non-empty selected signer set, signature
count equal to selected signers, claimed total/signed stake equal to the
validator roster and selected signers, strict `> 2/3` signed-stake quorum,
non-empty StakeHistory sysvar data, and the exact 2,048-byte non-zero
AccountsLtHash before deeper account/finality checks run.
canonical runtime events storage key and the source-event leaf index as
first-class UI witness material, so the same runtime storage item and path bits
used to reconstruct the events root are also signed by the finality precommit
the adapter must additionally carry the
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
`sccp:ton:masterchain-signatures:v1`. TON source-adapter preflight recomputes
the validator-set hash from the declared validator roster, the signed
masterchain block-message hash from the adapter BlockIdExt/config/shard fields,
and the masterchain signatures transcript hash before deeper Ed25519
certificate verification runs. Production source material rejects
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
seals fail before hashing. Ordered transition chains reject disconnected parent
witness-schedule hashes, transition block hashes that are not anchored to the
supplied solid, parent, or signed ancestor header evidence, epoch gaps, epoch
overlaps, and non-increasing transition block numbers.
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
hashes. Rust adapter preflight now mirrors the deterministic parts of that
binding before verifier work by requiring parent-schedule hash agreement,
payload-hash agreement, payload-derived next-schedule agreement, transition
message-hash agreement, epoch-contiguous transition chains, strictly increasing
transition block numbers, and final next-schedule hash equality with the
adapter's active witness schedule.
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
  - no runtime proof family, verifier backend, or runtime envelope path
  - the typed SCCP message proof-artifact discovery path (`/v1/sccp/artifacts/message/{message_id}`);
  - the normalized SCCP counterparty proof-job discovery path (`/v1/sccp/jobs/message/{message_id}`);
  - the SCCP proof-manifest discovery path (`/v1/sccp/manifests`);
  - supported codec ids/keys; and
  - the per-counterparty generic message backends / registry backends for
    supported launch lanes only: `eth`, `bsc`, `sol`, `ton`, and `tron`.
    returned by public capabilities while launch scope is closed.
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
    unexpected verifier key hashes on native Solana/TON rollouts, or any
    remaining rollout blocker. Destination rollout readiness
    is also profile-bound for every advertised SCCP domain: ETH/BSC require non-zero EVM
    contract addresses plus their exact mainnet anchor ids, Solana requires a
    non-zero program id plus
    `sccp:sol:destination-anchor:solana-mainnet-beta:v1`, TON requires a
    non-zero raw basechain `0:account_hex` contract address plus
    `sccp:ton:destination-anchor:ton-mainnet:v1`, and TRON requires a
    checksummed base58 contract address plus
    `sccp:tron:destination-anchor:tron-mainnet:v1`.
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
  - the target verifier backend key for that counterparty lane
    (`evm-groth16-bn254-v1`, `tron-groth16-bn254-v1`, `solana-program-v1`,
    or `ton-contract-v1`);
  - the declared SCCP proof security model (`RecursiveZk`) and anchor mode (`CryptographicProof`);
  - a typed destination binding (`version`, `key`, `binding_hash`) that scopes proofs to the intended verifier deployment/runtime context for that lane;
  - the chain-specific message backend / registry backend pair;
  - the canonical counterparty account codec;
  - the intended verifier target (`EVM`, `Solana`, `TON`, or `TRON`);
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
    scope is closed.
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
	    the active Ethereum mainnet launch lane only when the configured Ethereum
	    source-chain finality/inclusion material, immutable destination verifier
	    deployment, active cryptographic anchors, route allowlist, and route canary
	    are all present. Other counterparty lanes remain behind their future lane
	    launch policies. Strict proof-job builders also apply the production
	    source-proof gate: non-SORA source bundles must carry production-ready
	    source-chain evidence before a job can be exposed without the explicit
	    diagnostic `allow_unready` path, while SORA-origin jobs do not require
	    source-chain evidence. The internal proof-byte job path also requires the
	    bundle-derived counterparty domain, manifest counterparty domain, and
	    supplied job counterparty domain to match, so diagnostic callers cannot
	    mix a bundle with another lane's manifest or job metadata. Reusable
	    transparent-statement and submission-package builders apply the same
	    bundle-to-manifest counterparty binding before deriving statement hashes,
	    native proof bytes, or relay envelopes, so inbound messages whose public
	    inputs target SORA cannot be packaged under another remote lane's
	    manifest.
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
    the active Ethereum mainnet launch lane only when the configured Ethereum
    source-chain finality/inclusion material, immutable destination verifier
    deployment, active cryptographic anchors, route allowlist, and route canary
    are all present. Other counterparty lanes remain behind their future lane
    launch policies.
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
    lanes consumable. BSC and TRON route-config renderers also reject
    production-ready manifests that explicitly force `--allow-unready true`, so
    governed runtime overlays cannot re-enable diagnostic transparent-proof
    admission while claiming production readiness. Release-readiness and strict
    release-bundle source inventory pin both the default `false` overlay and
    direct/merged route-config rejection tests for those production-ready
    manifests.
    On-chain admission uses a manifest-only unready allowance for configured
    deployment lanes and still fails material-only or otherwise unready
    non-SORA source-chain envelopes.
  - non-SORA `message_bundle` submission must carry a verified source-chain
    proof envelope; Torii no longer manufactures synthetic external-chain
    finality from local Nexus/Iroha finality evidence.
- `POST /v1/bridge/proofs/submit` now derives chain-specific SCCP transparent backends for generic `message` bundles:
  - outbound `SORA -> ETH` and inbound `ETH -> SORA` messages use `bridge/sccp/stark-fri-v1/eth`;
  - the same pattern applies to supported `bsc`, `sol`, `ton`, and `tron`
    targets while launch scope is closed;
  - the bridge proof manifest hash is derived from the same domain suffix, so proof IDs and registry queries split cleanly by counterparty chain instead of collapsing all SCCP traffic into one generic backend bucket.
- ETH/BSC message-proof building previously depended on Torii's
  `da_receipt_signer` using `secp256k1`, because the EVM submission package was
  a signer-backed attestation envelope over the canonical SCCP proof-envelope hash and
  canonical public inputs. That path is now disabled for production because it
  is not destination-native cryptographic verification.
- `POST /v1/bridge/proofs/submit` and `POST /v1/bridge/messages` now also return normalized SCCP counterparty metadata in the response:
  - `counterparty_domain` is the numeric SCCP domain id; and
  - `counterparty_chain` is the canonical supported launch-domain key (`eth`,
    `bsc`, `sol`, `ton`, or `tron`).
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
