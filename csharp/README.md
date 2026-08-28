# Hyperledger Iroha C# SDK

Preview `.NET 8` SDK for Hyperledger Iroha.

`TairaTestnetProfile` exposes the public Torii origin, address discriminant,
Digital Shekel, and XOR metadata without embedding runtime secrets. Create a
client with the current deployment's genesis-derived network identity; the
stable chain UUID is not a signing identity:

```csharp
var networkId = NetworkId.Parse(configuredNetworkIdLiteral);
using var torii = TairaTestnetProfile.CreateClient(networkId);
```

## Scope

This initial slice provides the foundation needed for a usable managed SDK:

- canonical account-address parsing and I105 rendering with exact I105 literal,
  invariant numeric chain-discriminant parsing, case-exact signing-algorithm
  label validation, and detached public/canonical/controller byte snapshots
- Norm v1 domain normalization helpers
- managed BLAKE2b-256/Iroha hash and Norito framing primitives used by the
  SDK's transaction path, with Norito header schema hashes snapshotted on
  construction, initialization, and access plus strict v1 frame decode checks
  for magic/version, schema, compression, reserved layout flags, zero padding,
  length, and CRC
- lossless Kotodama numeric V1 values for signed 512-bit `int`, exact
  scale-bounded `decimal`, and nominal non-negative `quantity`, with canonical
  string-only JSON, minimal two's-complement Norito frames, authenticated
  pointer envelopes, and no conversion through CLR floating-point types
- canonical Torii request signing headers with an exact canonical I105 account
  or a bounded structurally valid lowercase-ASCII account alias, an HTTP-token method, an
  exact percent-encoded root-relative ASCII wire path without query or fragment text, a 1--256 byte
  printable-ASCII nonce, canonical 64-byte signature-header base64, and
  positive timestamp validation before signing; I105 identities must match the
  Ed25519 private seed and are emitted as portable lowercase canonical-address
  hex, while aliases are resolved and authorized by Torii. `BuildHeaders`
  signs the same `Uri.AbsolutePath` and percent-encoded query spelling that
  `HttpRequestMessage` sends; the pure message/query helpers continue to accept
  an already exact wire target.
  This first-release
  helper is single-signature only; it does not yet construct or encode
  `X-Iroha-Witness` multisignature proofs
- canonical request credentials and Ed25519 key pairs require exact 32-byte
  Ed25519 private seeds, require canonical I105 request accounts to match the
  seed public key, and defensively copy private seed and public-key byte arrays
  on construction and access; canonical aliases are state-bound and therefore
  remain Torii-authoritative for UTS-46, active-catalog resolution, and
  controller verification. Ed25519 signing and verification use
  exception-safe zeroing for managed temporary seed, expanded-key, message,
  signature, and public-key copies after use or failure
- a `LedgerClient` plus `TransactionBuilder` that can build, sign, and submit canonical asset/domain/asset-definition/NFT transfer transactions, asset mint/burn transactions, `SetAssetKeyValue`, `RemoveAssetKeyValue`, `SetDomainKeyValue`, `RemoveDomainKeyValue`, `SetAccountKeyValue`, `RemoveAccountKeyValue`, `SetAssetDefinitionKeyValue`, `RemoveAssetDefinitionKeyValue`, `SetNftKeyValue`, `RemoveNftKeyValue`, `SetTriggerKeyValue`, `RemoveTriggerKeyValue`, `MintTriggerRepetitions`, `BurnTriggerRepetitions`, and `ExecuteTrigger` transactions with deterministic hashes and pipeline-status polling
- managed transaction builders and Norito encoders validate exact chain/account/asset/domain/NFT/trigger/metadata/numeric/hash boundary fields before signing, including canonical I105 transaction authorities and account-bearing instruction fields, noncanonical numeric aliases such as signed positives, exponent notation, missing integer/fraction digits, leading integer zeros, trailing fractional zeros, negative values, scale overflow, and 512-bit mantissa overflow; asset transfer/mint/burn inputs are stored as nominal `NumericV1.QuantityValue` instances and accept only canonical non-negative V1 quantity strings or the validated lossless type, transaction creation times must be positive Unix milliseconds, and trigger repetition mints/burns must be positive before signing; JSON metadata values remain free-form but are defensively snapshotted by transaction builders and JSON-bearing instruction records so later caller mutation cannot alter signed payloads, public transaction-builder instruction/metadata accessors return detached snapshots, and signed transaction/query envelopes defensively copy all exposed byte arrays while rejecting empty, mismatched, malformed encoded-body, or wrong-size direct constructor byte fields before callers submit or trust them
- typed Torii runtime and account-query models for capabilities, ABI, account pages, explorer inventory, contract reads and writes, identifier and alias resolution, sponsored account-onboarding preparation, faucet preparation, multisig transaction proposals, UAID portfolios, and space-directory inventory. Sponsored onboarding is a closed plan → prepare → persist → submit protocol: the SDK verifies the pinned plan receipt, reset binding, exact signed transaction wire, fee intent, metadata, transaction signature, and server-authenticated V1 transcript before returning a persistable prepared envelope. A signed `ProofRequired` result is nonterminal: `ProveAccountOnboardingCurrentStateAsync` must re-authenticate it and POST one closed request to `/v1/accounts/onboarding/current-state`; the returned single-snapshot anchor classifies the alias as `Applied`, `AliasAbsent`, or `AliasConflict`. Faucet mutation follows the same one-envelope prepare/persist/submit rule after solving PoW. The submit APIs accept only the exact prepared envelopes; there are no direct registration, direct faucet-claim, or multisig-specific onboarding routes.
- typed ABI-23 native-Norito Hijiri validation-fee quotes through
  `PostValidationFeeHijiriQuoteAsync(...)`. The client signs the exact bounded
  request only over HTTPS, accepts only one non-redirected HTTP 200
  `application/x-norito` response, requires every response to be private and
  `no-store`, rejects encoded, oversized, noncanonically framed, or
  length-mismatched bodies and success rejection headers, bounds error bodies,
  and returns the projection only after native canonical decoding and exact
  request, height, hash, and aggregate-Q16 validation. The assurance is an
  authenticated same-snapshot evaluation rather than an independent state
  witness, so transaction admission remains authoritative for stale policy or
  Hijiri bindings
- direct generic and contract-call multisig response DTO construction rejects
  false `ok`, malformed resolved account ids, proposal/instruction hashes,
  transaction hashes, creation times, and signing-message base64 before callers
  can serialize or trust manually constructed signing material
- direct contract-call, contract-view success, contract-view error, and
  contract-view VM diagnostic response DTO construction rejects malformed
  `ok` envelopes, dataspace/contract-id/address/entrypoint/error text,
  returned code/ABI/transaction hashes, creation-time counters, canonical
  transaction base64 material, VM diagnostic text, and non-positive VM
  diagnostic gas/cycle/stack limits before callers can serialize or trust
  manually constructed execution results
- direct contract code-byte and contract state entry/response DTO construction
  rejects non-exact `code_b64`, malformed state paths/path lists, noncanonical
  state `value_b64`, and malformed decode-error text before callers can
  serialize or trust manually constructed contract code/state results
- direct contract instance and instance-inventory response DTO construction
  rejects malformed contract ids, code hashes, and namespace text before
  callers can serialize or trust manually constructed instance listings
- contract manifest/code-record DTOs preserve the complete Kotodama V1 shape:
  `seiyaku_name`, branded entrypoints, exact flat-preorder argument/return schemas,
  dynamic access hints and completeness, triggers, state, error codes, `kotoba`,
  and typed provenance; parsing rejects unknown fields, malformed checksummed
  Norito hashes, wrapper/manifest hash mismatches, and inconsistent schemas
  before callers trust metadata records; aggregate types use one flat preorder
  tape where a `List` node carries only `capacity` and its element subtree
  immediately follows, while retired nested `element` payloads, truncated or
  overlong tapes, and forged core-query `View`/`QueryPage<View>` shapes are
  rejected
- direct contract code-view DTO construction rejects malformed access-hint
  keys, entrypoint params, entrypoint metadata, syscall names, analysis memory
  and syscall lists, top-level code/ABI hashes, permissions, warnings, and
  rendered-source fields before callers can serialize or trust manually
  constructed code-view metadata
- direct node capability DTO construction rejects malformed first-release ABI
  and data-model counters, schema hashes, nested capability objects,
  SM/default/policy labels, curve id lists, query aggregate labels, projection
  constants/codecs, checkpoint heights, and checkpoint block hashes before
  callers can serialize or trust manually constructed capability metadata
- node capabilities, active runtime ABI, and runtime metrics fail before dispatch
  unless both canonical request credentials and an immutable local signing context
  with the deployment's exact `NetworkId` are configured; their empty `GET`
  requests are signed over the exact method/path/query/body and dispatched once,
  while `/v1/runtime/abi/hash` remains a public read; direct runtime ABI/metrics DTO construction rejects non-v1 ABI versions,
  missing metrics counters, negative upgrade counters, non-`V1` ABI hash
  policies, and malformed ABI hashes before callers can serialize or trust
  manually constructed runtime metadata
- node capability list DTOs snapshot assigned arrays and return detached arrays
  on access for signing labels, curve ids/bitmaps, row enrichment fields,
  aggregate resources, projection metadata keys, and export resources while
  preserving malformed null/missing list rejection in raw converters
- raw contract instance inventory DTO deserialization shares the same fail-closed checks as `ToriiClient`, rejecting null, duplicate, or type-confused list/item fields, duplicate keys inside ignored instance/response extension JSON, missing or malformed unsigned counters, missing or non-exact namespace/contract-id/code-hash text, and inconsistent pagination counters before callers trust instance listings
- sponsored onboarding starts with the secret-free
  `ToriiAccountOnboardingPlanRequest`, then passes the signed receipt and an
  exact `ToriiTairaPublicResetMutationBindingV1` plus the caller-selected
  `FeePaymentIntent` to
  `PrepareAccountOnboardingAsync`. Persist either the returned
  `ToriiAccountOnboardingPreparedTransactionV1` or authenticated nonterminal
  `ToriiAccountOnboardingProofRequiredPrepareResponseV1`. A proof-required
  result cannot be submitted or treated as applied; call
  `ProveAccountOnboardingCurrentStateAsync` on every open/reopen. It performs
  exactly one atomic current-state POST, verifies the network and committed
  block anchor, and classifies the exact alias as applied, absent, or conflicting.
  Preparation, proof, and submission all require
  the original `ToriiAccountOnboardingPlanRequest` as an independent intent
  pin, including its exact ordered permission set. Only the prepared
  transaction and the same independent fee intent may be passed to
  `SubmitPreparedAccountOnboardingAsync`. Planning and preparation
  require explicit authority and chain pins plus a canonical Norito body
  encoder, recompute the signed receipt hash, and reject request substitution.
  The
  dedicated `X-Iroha-Onboarding-Token` argument remains 32–256 printable ASCII
  bytes, stays separate from global API authentication, is emitted exactly
  once outside the JSON body, and is never replayed across redirects. There is
  no direct-registration overload or multisig-specific onboarding route.

When injecting a custom `HttpClient`, configure its complete handler chain as an
identity, one-shot transport for signed, nonce-bearing, and credential-bearing
requests: set `HttpClientHandler.AllowAutoRedirect` to `false` and do not attach
automatic retry/resilience handlers. Redirect and retry policy belongs to the
injected transport and cannot be inspected or changed after `HttpClient`
construction. `ToriiClient` invokes the handler chain once and surfaces 3xx,
non-success, and network failures. For an ambiguous transaction outcome, query
the pipeline status by transaction hash instead of replaying the body; a
deliberate signed-query retry must use a freshly signed envelope and nonce.

`SubmitTransactionAsync(...)` posts only to the first-release
`/v1/pipeline/transactions` route after an exact V1 data-model and signed-schema
contract check. Its body is exactly the V1 version byte followed by the canonical
`SignedTransaction`, and HTTP 202 is the sole successful admission acknowledgement;
there is no unversioned body, legacy endpoint, or fallback selector. Public
transaction submission also rejects a caller-injected `HttpClient` before network
dispatch because its redirect/retry behavior cannot be proven; use the internally
managed one-shot transport.
`LedgerClient.WaitForAsync(...)` succeeds only for a global, state-resolved
`Applied` status with a positive block height. State-resolved `Rejected` and
`Expired` fail, while every queue- or cache-resolved status remains pending.

The public pipeline-status response is metadata-only: an exact prefixless 64-character
lowercase hexadecimal hash, closed status kind,
optional committed height, scope, and resolution source. Retired rejection, diagnostic,
trigger, and batch fields are rejected. Status lookup defaults to the exact `global` scope;
callers may request exact `local`, while retired `auto` and case-normalized spellings are
rejected in requests and responses. For details, build an exact-entrypoint predicate with
`SignedIterableQueryBuilder.FindTransactionDetails(...)` and pass the resulting envelope to
`GetPipelineTransactionDetailsAsync(...)`. The signature is bound to the deployment's exact
genesis-derived `NetworkId`; Torii admits only an involved account or operator, and the SDK
sends the nonce-bearing body once without redirects or retries.
- multisig propose/approve helpers reject zero `creation_time_ms` when supplied, and generic/contract-call multisig response DTOs reject zero returned `creation_time_ms` before callers trust signing material
- native multisig propose instruction-list DTOs snapshot assigned arrays, return
  detached arrays on access, reject null instruction elements during direct
  initialization, and keep null distinct from empty so request validation still
  rejects missing lists before dispatch
- generic and contract-call multisig response DTO deserialization requires
  `resolved_multisig_account_id` to be present and non-null before exact account
  id validation runs
- raw contract state entry/response DTO deserialization shares the same fail-closed checks as `ToriiClient`, rejecting null, duplicate, or type-confused entries, paths, counters, and booleans, duplicate keys inside ignored state entry/response extension JSON, missing or non-exact entry paths/path-list elements, non-exact path/prefix/decode-error text, stale `next_offset`, over-limit item counts, not-found entries with value material, noncanonical `value_b64`, and decoded-length mismatches before callers trust state material
- contract state query/response path-list DTOs snapshot assigned arrays and
  return detached arrays on access while preserving nullable single-path,
  path-list, and prefix query modes
- raw contract state response DTO deserialization requires returned `offset` and
  `limit` counters to be present instead of defaulting absent wire fields to
  trusted zero values
- raw contract-view VM diagnostic DTO deserialization requires returned
  gas/cycle/stack limit fields to be present instead of defaulting absent limit
  fields to trusted zero values
- raw contract code-byte DTO deserialization rejects null, non-object, missing, duplicate, duplicate keys inside ignored extension JSON, and type-confused `code_b64` envelopes before preserving the existing exact base64 `FormatException` behavior for malformed code material
- contract code-view access-hint, entrypoint, analysis, permission, and warning
  list DTOs snapshot assigned arrays and return detached arrays on access while
  preserving malformed missing/null list rejection in the raw converters
- verifying-key registry read/write helpers validate exact backend/name route text, canonical I105 authority, circuit, gas-schedule, curve, CID, commitment, and status request fields, plus positive version, `vk_len`, and max proof-size fields before HTTP dispatch, and inline verifying-key byte arrays are snapshotted on assignment and access before serialization; request models contain no private-key field, accepted 32-byte hex casing/prefixes and status labels are canonicalized only after exact text preflight, verifying-key read responses validate the detail envelope, id/record backend-name material, version/height/status fields, lowercase hashes, inline key base64, and record metadata before returning raw JSON to callers, and verifying-key register/update responses return typed unsigned drafts only after exact-field, canonical-base64, payload-prehash, configured-chain, authority, single-operation, identifier, and full-record validation
- identifier resolve requests require `CanonicalRequestCredentials` plus an immutable exact-`NetworkId` local signing context, reject precomputed signing headers, sign the exact POST path and JSON body once, and validate exact policy ids and encrypted-input text before dispatch; identifier policy listing responses plus raw policy summary/page DTO deserialization validate exact policy ids, resolver public keys, page totals, list items, and duplicate object keys inside decoded policy parameter or ignored extension JSON, and both ToriiClient/raw resolve receipts require the current nested `payload`/`attestation` envelope, reject retired flat receipt fields, and validate exact payload identifiers, attestation signatures, canonical proof base64, execution/opening metadata, duplicate object keys, and positive canonical timestamp strings before callers trust Torii JSON responses; padded, whitespace-containing, control-character, malformed, noncanonical, zero-timestamp, unknown-extension, or mixed signed/proof receipt shapes fail during deserialization
- identifier policy, contract instance inventory, and contract state response list
  DTOs snapshot assigned arrays and return detached arrays on access while
  preserving malformed null/missing list rejection in raw converters
- explorer and account route reads validate exact path-segment identifiers before HTTP dispatch, including account/domain/asset/NFT/RWA/block/transaction/instruction identifiers, while account asset/transaction/permission routes require canonical I105 account ids; explorer transaction/instruction authority filters, instruction account filters, and explorer owner filters for domains, asset definitions, assets, NFTs, and RWAs require canonical I105 account ids, and other explorer/account query filters for domain, asset, transaction, instruction kind, and account asset scope/id fields reject blank, padded, whitespace-containing, and control-character values before dispatch; the six account/domain/asset-definition/asset/NFT/RWA world-list routes use bounded `cursor`/`limit` continuation (default 25, maximum 100) and reject empty, padded, noncanonical, or oversized base64url cursors before HTTP dispatch; account list responses plus raw account summary/page DTO deserialization reject null list/items, duplicate/type-confused fields, missing or noncanonical I105 account ids, and negative totals, explorer account/domain directory and asset-definition/asset/NFT/RWA inventory responses reject retired or unknown page fields, missing cursor metadata, out-of-range limits, noncanonical cursors, inconsistent `has_more`/`next_cursor` pairs, and item counts beyond the advertised limit, along with malformed identifiers, owners, quantities, RWA status/reference fields, and RWA parent lists before callers trust indexed inventory; explorer QR/health/metrics responses plus raw QR/health/metrics/duration DTO deserialization reject duplicate/type-confused fields, duplicate keys inside ignored snapshot extension JSON, missing or noncanonical QR canonical account ids, malformed QR literal/rendering text, malformed QR dimensions, missing health sample timestamps, unsigned counters, durations, and malformed timestamp text before callers trust rendered or aggregated explorer projections, explorer econometrics/holder snapshot responses plus raw econometrics/snapshot DTO deserialization reject duplicate/type-confused windows, counters, ratios, and holder/distribution lists, noncanonical holder account ids, malformed definition identifiers, null econometrics and holder-distribution lists/items, noncanonical numeric strings, out-of-range distribution ratios, and null distributions before callers trust aggregated explorer projections, account asset-balance responses plus raw asset balance/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or malformed asset/scope/name text, noncanonical I105 account ids, malformed alias text, missing or noncanonical quantity text, and negative totals, account permission responses plus raw permission/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or malformed permission names, duplicate object keys inside arbitrary payloads, and negative totals while preserving arbitrary non-duplicate permission payload JSON, and account transaction-summary responses plus raw transaction summary/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or non-exact entrypoint hashes, noncanonical I105 authority text, boolean confusion, non-positive timestamps, and negative totals before callers trust account history; explorer block/transaction/instruction REST, SSE, and raw DTO paths retain page/per-page pagination and reject malformed, JSON-null, or duplicate-key SSE data frames, duplicate raw DTO properties, missing required summary string fields, noncanonical transaction/instruction authority text, non-exact block/transaction hashes, zero page/per-page pagination metadata, malformed summary `created_at` timestamps, transaction-detail nonce and TTL type confusion, instruction/rejection encodings with uppercase `0X` prefix aliases, lowercase transaction signatures, null stream/list items, missing or malformed instruction boxes, and malformed unsigned counters before callers trust indexed ledger projections
- explorer directory and inventory list DTOs snapshot assigned arrays and return
  detached arrays on access for account/domain/asset-definition/asset/NFT/RWA
  pages and nested RWA parents while preserving malformed null/missing list
  rejection in raw converters
- explorer econometrics list DTOs snapshot assigned arrays and return detached
  arrays on access for velocity windows, issuance windows, issuance series,
  Lorenz points, and top-holder rows while preserving malformed null/missing
  list rejection in raw converters
- explorer ledger projection list DTOs snapshot assigned arrays and return
  detached arrays on access for block, transaction, latest-transaction,
  instruction, and latest-instruction pages while preserving malformed
  null/missing list rejection in raw converters
- raw account query DTO deserialization rejects duplicate keys inside ignored extension JSON for account summaries/pages, asset balances/pages, permission items/pages, and transaction summaries/pages, including the same nested page-item contexts returned by Torii account query endpoints, and requires required item strings and page `total` fields to be present instead of defaulting absent fields to trusted empty/zero values; direct account-query DTO metadata rejects malformed canonical account ids, asset/scope/name/alias text, canonical quantity text, permission names, entrypoint hashes, and non-positive timestamps before callers can serialize or trust manually constructed account query results
- account query page and alias lookup list DTOs snapshot assigned arrays and
  return detached arrays on access for account summaries, asset balances,
  account permissions, account transactions, and by-account alias lookup items
  while preserving malformed null/missing list rejection in raw converters
- Torii DTOs that expose arbitrary JSON nodes snapshot assigned `JsonNode` and
  `JsonObject` values and return detached clones on access for account
  permission payloads, explorer metadata/rejection/instruction
  payloads, UAID manifests, identifier parameter/signature payloads, contract
  state/view/call payloads, multisig contract-call payloads, and SSE JSON data
- raw explorer block, transaction, and instruction page/latest DTO deserialization requires pagination, item lists, latest sample timestamps, summary creation timestamps, and unsigned projection counters to be present instead of defaulting absent page envelopes, item arrays, block height, transaction block, instruction block/index, block transaction-count, or summary `created_at` fields to trusted zero/empty values
- raw explorer QR/health/metrics/duration DTO deserialization requires QR
  account/rendering text, QR network prefixes, QR modules/versions, health head
  heights/sample timestamps, metrics aggregate counters, and duration `ms`
  fields to be present instead of defaulting absent snapshot fields to trusted
  empty/zero values
- raw contract-call and alias binding DTO deserialization requires positive
  creation-time and binding timestamp fields to be present instead of defaulting
  absent wire fields to trusted zero values
- contract-call/view response DTO deserialization requires returned dataspace,
  contract id, code/ABI hash, view entrypoint/error text, and VM diagnostic
  trap/message text to be present and non-null before exact validation runs;
  nullable returned contract addresses and call entrypoints remain optional but
  exact when present
- `CallContractAsync` accepts a contract-call receipt only when
  `payload_digest_hex` is the exact lowercase BLAKE3-256 digest of the
  canonical UTF-8 JSON request payload; an omitted payload hashes the empty
  byte sequence, and a mismatch fails closed
- raw VPN profile, quote, session, receipt, and receipt-list DTO deserialization
  requires operational timing, MTU, fee/grace, flow-label, and padding fields to
  be present instead of defaulting absent wire fields to trusted zero values
- raw faucet puzzle DTO deserialization requires algorithm, anchor hash, and
  numeric challenge/work-factor fields to be present instead of defaulting
  absent PoW parameters to trusted empty/zero values
- raw account transaction, account-alias lookup, VPN profile,
  contract-call/view, and contract-state DTO deserialization requires returned
  boolean fields to be present instead of defaulting absent wire fields to
  trusted `false` values
- raw node capability DTO deserialization requires first-release ABI/data-model
  and curve-registry counters, SM/query/aggregate/projection capability flags,
  projection string labels/list elements, and projection numeric constants to
  be present instead of defaulting absent or null wire fields to trusted
  empty/zero/`false` values
- raw runtime metadata DTO deserialization requires `abi_version`, `policy`,
  `abi_hash_hex`, `upgrade_events_total`, and upgrade-counter `proposed`,
  `activated`, and `canceled` fields to be present instead of defaulting
  absent runtime metadata to trusted zero or empty values
- raw identifier-policy, account-alias lookup, UAID manifest, contract-instance,
  and VPN receipt-list DTO deserialization requires page counters and item
  arrays to be present instead of defaulting absent totals, offsets, limits, or
  lists to trusted zero/empty values
- raw UAID portfolio totals and nested UAID dataspace/manifest-revocation DTO
  deserialization requires `accounts`, `positions`, `dataspace_id`, and
  revocation `epoch` counters to be present instead of defaulting absent
  counters to trusted zero values
- UAID portfolio/bindings/manifest response DTO deserialization requires
  returned UAID literals, full portfolio asset ids bound to their exact
  definition/account/dataspace, canonical quantities, manifest hashes/status,
  and canonical I105 account-list entries to use the exact current fields.
  Manifest pages require `uaid`, `total`, `has_more`, `count_mode`, and
  `manifests`; the embedded manifest is numeric V1 with required `issued_ms`,
  `activation_epoch`, and `entries`, and optional fields must be omitted rather
  than encoded as null. Required nullable aliases, labels, and revocation reasons
  are preserved byte-for-byte instead of trimmed or defaulted
- UAID portfolio, bindings, and manifest inventory list DTOs snapshot assigned
  arrays and return detached arrays on access while preserving malformed
  null/missing list rejection in the raw converters
- direct UAID portfolio, bindings, manifest lifecycle/record, and manifest-list
  response DTO construction rejects malformed UAID literals, dataspace counters,
  canonical I105 account lists, asset ids, canonical quantities, manifest
  hashes/status text, lifecycle counters, and revocation metadata before callers
  serialize or trust manually constructed space-directory metadata
- account/asset/contract alias lookup and resolve helpers validate exact aliases, dataspace, and domain filters before HTTP dispatch, with by-account lookup requiring canonical I105 account ids; alias lookup/resolve responses plus raw account alias lookup, alias index, alias resolution, and alias binding DTO deserialization reject duplicate/type-confused envelopes, duplicate keys inside ignored alias lookup/index/resolution/binding extension JSON, missing or malformed aliases, noncanonical account ids, malformed asset/contract ids, dataspace/source/status fields, null lookup lists/items, required alias-index indexes, negative index fields, and non-positive binding timestamp fields before callers trust alias metadata; direct alias lookup/resolution/binding DTO metadata rejects malformed aliases, dataspace/domain/source/status fields, account ids, asset/contract identifiers, negative indexes, and non-positive binding timestamps before callers can serialize or trust manually constructed alias results
- typed Torii VPN and SoraFS helpers for `/v1/vpn/profile`, signed VPN quote/session/receipt flows under `/v1/vpn/quotes`, `/v1/vpn/sessions`, and `/v1/vpn/receipts`, `/v1/sorafs/cid/{cid}`, and CID content reads under `/sorafs/cid/{cid}/...`; VPN quote/session/receipt writes validate exact exit-class text, 32-byte quote/payment/metering/lease hex fields, and receipt/voucher hex payload shape before HTTP dispatch; VPN profile, quote, session, receipt, and receipt-list responses plus raw VPN response/native-instruction DTO deserialization reject null, duplicate, or type-confused envelopes, duplicate keys inside ignored VPN response/native-instruction extension JSON, noncanonical account, escrow, and operator account ids, missing or malformed route/DNS/tunnel lists, required native-instruction arrays, and counters, non-positive lease/DNS/MTU operational fields, zero or reversed VPN quote/session/receipt timestamps, missing VPN accounting counters, inconsistent receipt-list totals, and non-exact lowercase SPKI/key/hash/id/native-instruction hex fields before callers trust tunnel or settlement material; VPN session responses and read routes require canonical 16-byte session ids encoded as 32 lowercase hex characters, SoraFS CID lookup/content reads validate lowercase multibase base32 CID route ids, and content relative path identifiers validate exact path segments before dispatch while preserving valid internal SoraFS path text; SoraFS CID lookup responses plus raw CID lookup/file-entry DTO deserialization reject duplicate/type-confused fields, duplicate keys inside ignored CID lookup/file-entry extension JSON, missing or non-exact content CIDs, missing or malformed lowercase manifest digest/id hex, malformed path components, and missing or negative file geometry before callers trust gateway listings; buffered SoraFS content responses reject malformed or duplicated `sora-content-cid` headers and direct malformed content CIDs or negative content lengths before returning content metadata, and defensively copy buffered response bytes; SoraFS pin registration accepts an already signed `SignedTransactionEnvelope`, posts only its Norito bytes, requires HTTP 202, and returns the strict admission identity (`status: submitted`, transaction hash, and manifest digest) without claiming finality, fees, custody, or pin status; raw chunker/storage-class/pin-policy DTO deserialization rejects null, duplicate, case-drifted, or type-confused fields, duplicate keys inside ignored extension JSON, invalid counters, non-exact chunker text, and unknown storage-class values while canonicalizing omitted optional multihash/retention counters to zero
- SoraFS CID lookup file path/file-list DTOs snapshot assigned arrays and return
  detached arrays on access while preserving malformed null/missing list
  rejection in the raw converters
- direct SoraFS CID lookup/file response DTO construction rejects malformed CIDs,
  hashes, path components, and negative counts before callers serialize or trust
  manually constructed SoraFS metadata
- VPN profile, quote, and session network route/DNS/tunnel list DTOs snapshot
  assigned list arrays, return detached arrays on access, and reject null
  elements during direct initialization while preserving missing/null list
  validation
- VPN quote/receipt native-instruction list DTOs and receipt-list item DTOs
  snapshot assigned arrays, return detached arrays on access, and reject null
  native-instruction or receipt-list elements during direct initialization
  while preserving malformed missing/null list rejection in raw converters
- direct VPN profile, quote, session, receipt, receipt-list, and native
  instruction response DTO construction rejects malformed route/DNS/tunnel list
  elements, noncanonical account ids, non-exact ids/hashes/keys/SPKI material,
  malformed native instruction payloads, non-exact status/text fields, and
  non-positive operational counters before callers can serialize or trust
  manually constructed tunnel or settlement metadata
- VPN response DTO deserialization requires profile, quote, session, receipt,
  list item, string-list item, and native-instruction identity/status/hash/text
  fields to be present and non-null before exact validation runs; optional
  receipt `lease_id_hex`, optional TLS SPKI hashes, and nullable instruction
  objects retain their existing nullable semantics
- low-level `ToriiClient.SubmitSignedQueryAsync(...)`,
  `SubmitTransactionAsync(...)`, `GetPipelineTransactionStatusAsync(...)`,
  `OpenEventSseAsync(...)`, and parsed `StreamEventsAsync(...)` helpers plus a
  managed `SignedQueryBuilder` for the full current singular-query set
  (`FindExecutorDataModel`, `FindParameters`, `FindAliasesByAccountId`,
  `FindProofRecordById`, `FindContractManifestByCodeHash`, `FindAbiVersion`,
  `FindAssetById`, `FindAssetDefinitionById`, `FindTwitterBindingByHash`,
  `FindDomainEndorsements`, `FindDomainEndorsementPolicy`,
  `FindDomainCommittee`, `FindDaPinIntentByTicket`, `FindDaPinIntentByManifest`,
  `FindDaPinIntentByAlias`, `FindDaPinIntentByLaneEpochSequence`,
  `FindSorafsProviderOwner`, `FindDataspaceNameOwnerById`), a managed
  `SignedIterableQueryBuilder` for the current canonical iterable subset
  (`FindDomains`, `FindAccounts`, `FindAssets`, `FindAssetDefinitions`,
  `FindRepoAgreements`, `FindNfts`, `FindRwas`, `FindTransactions`,
  exact-entrypoint `FindTransactionDetails`,
  `FindRoles`, `FindRoleIds`, `FindPeers`, `FindActiveTriggerIds`,
  `FindTriggers`, `FindAccountsWithAsset`, `FindPermissionsByAccountId`,
  `FindRolesByAccountId`, `FindBlocks`, `FindBlockHeaders`,
  `FindProofRecords`, and cursor `Continue(...)`); both builders require the
  nominal `NetworkId` type (raw strings and retired chain aliases are not an
  overload) and bind its exact genesis-header identity
  into every signature together with the authority, current Unix time, a
  non-zero bounded lifetime, and a fresh 32-byte operating-system random nonce;
  explicit offline replay contexts receive the same strict validation, and
  signed query bodies are submitted only once because a lost response may
  follow successful admission; custom `HttpClient` handlers must therefore
  disable both automatic redirects and transport retries for these one-shot
  bodies; typed
  `StreamPipelineEventsAsync(...)` / `StreamProofEventsAsync(...)` plus typed
  explorer block/transaction/instruction SSE projections; raw
  `GetJsonDocumentAsync(...)`, `PostJsonDocumentAsync(...)`, signed-query JSON,
  and pipeline-status JSON responses reject duplicate object keys before
  returning `JsonDocument` data or parsed status objects, and typed
  `GetAsync(...)`/`PostAsync(...)` response deserialization applies the same
  duplicate-key gate before DTO materialization; `StreamEventsAsync(...)`
  rejects invalid UTF-8 event-stream bytes before replacement decoding can occur
  and preserves raw duplicate-key data frames without exposing collapsed
  `JsonData`; direct `ToriiServerSentEvent` metadata rejects empty,
  surrounding-whitespace, or control-character `Event`/`Id` values plus
  negative retry milliseconds, with inbound parser guard failures reported as
  `JsonException` stream errors; the canonical `/v1/events/sse` helpers expose
  no resume argument and never emit `Last-Event-ID`, because that live feed has
  no replay log; typed pipeline/proof streams surface a terminal
  `event: stream_error` as `ToriiStreamException` with its stable code, message,
  dropped-message count, and replay flag before ordinary category filtering;
  malformed terminal envelopes fail closed as `JsonException`; typed
  pipeline/proof SSE payloads reject
  malformed, JSON-null, or duplicate-key raw data frames plus missing
  proof-event selector text before projection, and raw
  `ToriiPipelineEvent`/`ToriiProofEvent` DTO deserialization
  rejects duplicate properties including duplicate keys inside extension data,
  exposes detached extension/removed-list snapshots, rejects missing or
  malformed required category/event labels, required proof backends, non-exact
  hashes, verifying-key reference text, prune counters, removed proof records,
  and malformed SSE metadata including noncanonical retry milliseconds before
  yielding trusted typed events; direct typed pipeline/proof DTO metadata
  rejects malformed category/event/backend/hash text, malformed projection
  metadata, negative retry values, and malformed removed-record copies before
  callers can serialize or trust manually constructed events; direct explorer
  block/transaction/instruction SSE DTO metadata rejects malformed hashes,
  timestamps, canonical authorities, status/executable/kind labels, and nested
  instruction box/JSON hex payloads before callers can serialize or trust
  manually constructed explorer events, and direct explorer transaction detail
  and rejection DTO metadata rejects malformed authority/hash/timestamp/status,
  signature, rejection hex, and message fields before serialization; direct
  explorer ledger pagination/latest wrapper metadata rejects zero `page`/`per_page`
  values and malformed `sampled_at` text before serialization; direct explorer
  world-list cursor metadata rejects limits outside `1..=100`, noncanonical
  continuation cursors, and inconsistent continuation flags; direct explorer
  asset-definition econometrics and snapshot DTO metadata rejects malformed
  definition ids, window keys, canonical quantity text, top-holder account ids,
  distribution ratios, optional percentile quantities, and null distribution
  snapshots before serialization; direct explorer directory/inventory DTO
  metadata rejects malformed account ids, token fields, logo/mintability/status
  labels, asset quantities, NFT/RWA ownership, and RWA parent/reference fields
  before serialization; direct explorer account QR, health, and metrics DTO
  metadata rejects malformed canonical ids, token labels, non-positive QR
  geometry, negative network prefixes, SVG text, and sample/block timestamp text
  before serialization;
  signed-query builders validate canonical I105 authorities and account-id
  selectors plus exact selector/filter, cursor, and sort-key text before
  encoding or signing while preserving accepted proof-hash casing/`0x`
  canonicalization, and iterable continue gas budgets must be positive when
  supplied; pipeline
  transaction-status reads accept only prefixless 64-character lowercase
  hexadecimal hashes and exact `local`/`global` scope text before dispatch, with
  omission selecting `global`; they perform no hash/scope coercion and reject malformed
  status envelopes, unknown status kinds, non-exact returned hash/scope/resolution
  text, non-positive heights, and retired rejection/diagnostic/trigger/batch fields
  before returning status objects; `ToriiClient` construction validates absolute
  HTTP(S) base URIs without user-info, query, or fragment components while
  preserving exact path prefixes; low-level request setup validates exact
  HTTP Bearer token grammar, HTTP-token method text, and root-relative path
  request parts, rejects
  relative, absolute, scheme-relative, raw-backslash, malformed percent-escape,
  and raw/percent-encoded dot-segment paths before dispatch or canonical
  signing, while preserving other exact ASCII path text and opaque percent
  escapes, and validates
  optional query and single-media-range `Accept` text; the replay-capable
  explorer stream helpers separately validate `Last-Event-ID` text, with
  optional query text; canonical form processing ignores empty `&` segments,
  permits empty names and values, treats malformed percent escapes literally,
  and follows Rust's byte-by-byte lossy UTF-8 replacement and UTF-8 sorting
  before URI construction; public `configureRequest`
  hooks may add ordinary headers but cannot mutate the validated method, URI,
  content object, Authorization/Accept headers, signed content bytes, or
  canonical signing headers after request setup; canonical request signing
  requires an immutable exact `NetworkId`, generated or caller-supplied
  1--256 byte printable-ASCII nonces, and applies the same root-relative path
  guard and canonical form-query processing before sorting or signing; SSE
  event-filter query preflight separately rejects malformed
  percent escapes, invalid percent-encoded UTF-8, percent-decoded control bytes,
  malformed JSON-shaped filter payloads, padded JSON filter payloads, and
  duplicate-key JSON filters before production filter validation; raw signed-query
  submissions reject empty Norito payloads, while raw transaction submissions
  require a non-empty canonical V1 versioned wire before binary content is created
- `SignedIterableQueryBuilderTests` pin non-zero cursor, continue gas-budget,
  limit, and fetch-size preflight, the 10,000-row fetch-size ceiling, nullable pagination
  clearing, explicit sort clearing, and stale selector/parameter reset before
  signed Norito query bytes are produced
- `ToriiClientOptions` snapshots custom `JsonSerializerOptions` on assignment,
  public access, and `ToriiClient` construction so caller-owned option
  collection mutations cannot drift the effective client serializer
- faucet PoW solving validates `StartNonce`/`MaxAttempts` ranges before scrypt
  derivation so nonce enumeration cannot overflow mid-search
- a managed faucet PoW solver for the first-release `scrypt-leading-zero-bits-v1` algorithm and `iroha:accounts:faucet:pow:v1` challenge domain. `PrepareAccountFaucetAsync` accepts one solved `ToriiAccountFaucetClaimV1`, reset binding, caller-selected `FeePaymentIntent`, required `ToriiAccountFaucetPolicyV1`, and pinned network, then returns one authenticated exact envelope for durable persistence; `SubmitPreparedAccountFaucetAsync` requires that same independent fee intent and policy and submits only that envelope. Both paths reject payer, sponsor-revision, gas-bound, faucet-authority, asset-definition, or amount substitution before dispatch. The policy must come from trusted deployment configuration, never the prepared response. Claim PoW anchor height and canonical lowercase nonce hex are required direct V1 fields (never null or optional transcript wrappers). Puzzles carry the exact checksummed `NetworkId` and I105 chain discriminant, and the challenge binds the raw 32-byte network identity before the account and anchor; difficulty is mandatory and positive, while account ids, puzzle algorithm labels, anchor/salt/nonce hex, positive anchor-age bounds, bounded scrypt work factors including parallelization and ROMix memory, and mandatory claim PoW fields are validated before hashing or HTTP dispatch; pre-release labels are rejected
- `ToriiApiException` for non-success HTTP responses, preserving status code, request URI, and valid UTF-8 response bodies while redacting malformed UTF-8 bodies
- native Ethereum and BSC mainnet SCCP helpers for execution-provider chain-id
  validation, inbound receipt/source-event evidence, outbound Groth16 calldata,
  outbound proof request input bundle/source-proof byte arrays snapshotted before
  request construction, detached outbound request/result/submission array
  snapshots, null-element rejection for outbound public signal words, public
  input words, and submission arguments during direct DTO construction,
  explicit null byte-array rejection during direct local-admission, outbound,
  and Beacon REST DTO construction, outbound call/envelope hex views derived
  from current submission byte snapshots, detached local-admission
  input/payload/submission byte snapshots,
  BSC/Ethereum local-admission companion hex views derived from the current byte snapshots,
  source-proof Norito frame validation with canonical compact lengths, strict
  UTF-8 strings, and a 64-byte zero header-padding cap,
  TokenAdd fixed name/symbol fields constrained to canonical zero-padded
  printable ASCII,
  detached inbound receipt/trie/evidence byte-list snapshots, detached inbound
  evidence receipt/block/finality dictionaries and Ethereum block-receipt lists,
  typed receipt RLP/MPT proof construction from `eth_getBlockReceipts`, and source
  verifier/source-adapter material hashes bound to canonical mainnet chain ids,
  source bridge addresses, deployed bridge code hashes, and exact
  destination-binding keys; buffered Beacon REST response bodies are detached
  snapshots, and Beacon REST responses plus native EVM prover
  bundle/parity/self-test manifests decode byte payloads as strict UTF-8,
  expose required implementation/audit-role collections as runtime read-only
  views, snapshot artifact rows, public signal words, and SDK result maps, and
  reject duplicate JSON keys at any depth, and EVM JSON-RPC quantities plus
  Beacon REST and direct finality
  integer fields reject leading-zero or overflowing decimal/hex text before
  proof material is trusted
- `IrohaClient`, `LedgerClient`, and `ToriiClient` entry points, with raw JSON helpers still available for uncovered endpoints
- fixture-backed unit tests against the repo's canonical address vectors

Broader iterable families beyond the current canonical subset, richer typed event coverage beyond the current pipeline/proof/explorer SSE projections, broader contract admin/lifecycle helpers beyond deploy/activate/call/multisig plus verified-source job helpers, Connect, Nexus, and the remaining parity work are still planned.

## Kagemusha Torii transport

The managed SDK exposes universal offline discovery through
`GetOfflineCapabilityAsync`, plus `SubmitKagemushaTopUpV4Async`,
`SubmitKagemushaRedeemV4Async`, and `GetKagemushaOperationStatusAsync`.
Discovery is asset-neutral and accepts only the exact four-field
`cash_handoff_v1` capability response for bridge ABI 23.

The C# surface is transport-only and does not claim a native prover. Top-up and
redemption methods accept bounded canonical Norito archives created by a
supported wallet/prover implementation, snapshot the bytes, and bind
`Idempotency-Key` to the signed operation id. Witness construction, recursive
artifact installation, and device-key handling remain outside this SDK. The
transport accepts at most 512 KiB for top-up and 48 MiB for redemption, matching
Torii's route-specific request limits.
Kagemusha command submission uses `ToriiClient`'s internally managed one-shot,
no-redirect transport. A caller-injected `HttpClient` remains valid for the
read-only routes but is rejected before a signed command body is dispatched,
because the SDK cannot inspect an arbitrary handler chain for redirects or
automatic retries.

Each request constructor validates the route-specific Norito schema hash,
version, compression, CRC64, compact layout flag, non-empty payload, and exactly
eight zero alignment bytes before snapshotting the archive. HTTP 202 responses
must carry the matching `Location`, a positive unsigned `Retry-After`, and a
positive submission time. Polled statuses require marker-bearing transaction
hashes and positive finality values; applied top-ups additionally bind both the
V4 anchor and V1 finality-proof anchor to the requested operation id. Rejection
envelopes are exact, messages are bounded, and error codes use a stable
lowercase grammar. Optional details remain opaque JSON objects inside the
shared 256 KiB response-body and 128-level nesting limits.

`CreateVpnQuoteAsync(...)`, `CreateVpnSessionAsync(...)`, and
`SubmitVpnReceiptAsync(...)` call signed Torii
routes, so set `ToriiClientOptions.CanonicalRequestCredentials` with a
canonical I105 account id and set `LocalSigningContext` to the exact
genesis-derived `NetworkId` before using those helpers. Human-readable labels
never substitute for that signing domain. Session creation requires the quote id, committed
`OpenVpnLeaseEscrow` transaction hash, and the same metering public key that was
bound into the quote. Empty quote/session exit class still selects Torii's
profile default, and an empty receipt lease id still lets Torii derive the
settlement lease id from the relay receipt. Operator receipt submission returns
canonical receipt DTOs with earned/refund XOR fields
and any `SettleVpnLease` instruction skeleton Torii produced. A submission
receipt has exact status `settlement_pending` until that instruction commits;
only the committed WSV receipt has status `settled`. The validators retain the
exact `disconnected`, `expired`, and `replaced` lifecycle statuses.

For SoraFS content, use `OpenSoraFsCidContentAsync(...)` when you want the raw
HTTP response/stream, or `GetSoraFsCidContentAsync(...)` when buffering the
payload into memory is acceptable. The buffered helper treats an absent
`sora-content-cid` header as absent optional metadata, but rejects duplicated or
noncanonical CID header values before returning content metadata, and the
returned byte array is snapshotted on assignment and access. Optional content
relative paths must already be exact relative paths: padded values, empty
segments, `.`/`..` traversal components, backslash separators, and control
characters fail before the gateway request is sent.


## Verifying Key Registry

`ToriiClient.RegisterVerifyingKeyAsync(...)` and
`UpdateVerifyingKeyAsync(...)` post Torii's `/v1/zk/vk/register` and
`/v1/zk/vk/update` payloads. These requests contain only the public authority
and verifier metadata; private keys are never modeled or sent. The client
validates production verifier backends, canonical I105 authority values, height
ranges, and inline verifier-key commitments before the HTTP request is sent.
Clients preparing bytes for local signing require one immutable
`ToriiLocalSigningContext`; read-only clients may omit it. Production
backend labels reject blank, padded, and whitespace-containing text before
unsupported-backend classification.
Production `VerifyingKey`/`Proof` SSE event filters apply the same fail-closed
policy to JSON filter text, verifier-key names, and proof hash matchers, so
padded filter JSON, padded/whitespace/control-character names, and uppercase or
`0x`-prefixed proof hashes fail before the subscription request is sent.

```csharp
using var torii = new ToriiClient(
    toriiUri,
    options: new ToriiClientOptions
    {
        LocalSigningContext = new ToriiLocalSigningContext(NetworkId.Parse(networkId)),
    });

var draft = await torii.RegisterVerifyingKeyAsync(new ToriiVerifyingKeyRegisterRequest
{
    Authority = canonicalAuthorityAccountId,
    Backend = "halo2/ipa",
    Name = "vk_main",
    Version = 1,
    CircuitId = "halo2/ipa::transfer_v1",
    PublicInputsSchemaHashHex = new string('a', 64),
    GasScheduleId = "halo2_default",
    VerifyingKeyBytes = new byte[] { 1, 2, 3 },
    Status = "Active",
});
```

Torii returns HTTP 200 with `Submitted == false`, a canonical Norito
`TransactionPayload`, and its validated 32-byte Iroha prehash. Pass
`draft.TransactionPayload` to SDK signers that apply the Iroha prehash
themselves. Use `draft.SigningMessage` only with raw signature primitives or
HSM APIs that explicitly accept an already-prehashed message; otherwise the
message would be hashed twice. Assemble and submit the locally signed
transaction through the normal pipeline API. Before exposing the draft, the
client decodes the transaction and requires the configured chain, requested
authority, and exactly one register/update instruction whose identifier and
complete verifying-key record equal the normalized request.


## Native Privacy Bridge

`Hyperledger.Iroha.Privacy.PrivacyNative` is selector-free.
`CompiledProfileCatalogV1()` returns this binary's CRC-checked canonical
`PrivacyCompiledProfileCatalogV1` Norito archive. `PrivacyProtocolsV1.All` exposes
the closed `PrivacyProtocolIdV1 : uint` enum with explicit discriminants 0 through
11 in exact wire order. `CanonicalTypedVariantLabel()` mirrors the native
statement/proof variant tag for each row, while both canonical parsers reject
nulls, aliases, retired tags, case changes, and whitespace.
The local catalog contains no governance or readiness state; fetch a fresh
committed `PrivacyCapabilitySnapshotV1` from live Torii before proof submission.
`Exact12FixtureBundleV1()` returns the byte-complete Rust-derived statements,
envelopes, submit instructions, intent projections and digests, unsigned
payloads, versioned signed transactions, and pipeline hashes for all twelve
rows; `ValidateExact12FixtureBundleV1(...)` accepts only the canonical bundle
and enforces a 2 MiB input ceiling. Native
availability requires ABI 23, both compiled-catalog symbols, both exact-12 fixture
symbols, the zeroizing-free symbol, and successful typed probes. Generic
request/build/verify dispatch and free-form algorithm selectors are absent;
proofs use protocol-specific typed APIs.

`PrivacyExact12FixtureCodecV1` is the native-independent managed conformance
codec for the checked-in
`fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64` archive. It accepts
only schema-bound, checksum-valid, uncompressed canonical Norito with version 1,
the exact twelve ordered protocol rows, the first-release submit-proof wire ID,
its exact 29-byte UTF-8 / 30-byte compact-length-prefixed wire-ID layout,
bounded non-empty opaque fields, and exact 32-byte digests. Its immutable row
and bundle models defensively copy all byte arrays and lists. Use
`RequireTrustedCanonical(candidate, trustedArchive)` when the opaque inner bytes
must be bound to the reviewed Rust fixture: structural decode validates the
outer format, while the trusted path additionally requires complete byte
identity and therefore rejects row swaps, cross-row substitutions, and
checksum-repaired mutations. `DecodeCanonicalBase64` expects a single padded
standard-Base64 string with no whitespace; remove the fixture file's one final
LF only after checking that it is the sole line terminator.

The enum contains exactly twelve IDs: `zk-ace-pq-authorization-v1`,
`anonymous-pgc-k-out-of-n-v1`, `verange-transparent-range-v1`,
`iroha-zk-ams-v1`, `vega-existing-credential-zk-v1`,
`iroha-zk-x509-stark-p256-v1`,
`iroha-jindo-polynomial-commitment-v1`,
`iroha-bootle-lantern-anoncred-v1`, `orchard-halo2-actions-v1`,
`monero-fcmp-plus-plus-v1`, `iroha-ivm-private-note-stark-v1`, and
`pq-masp-stark-v1`. `ParseCanonicalLabel` rejects aliases, retired IDs, case
changes, and whitespace normalization.

## Native SoraFS Reference Validation

`Hyperledger.Iroha.SoraFs.SoraFsReferenceValidators` validates canonical
`GovernanceDagBlockV1` bytes and signed `GovernanceDagHeadV1` chains through
`connect_norito_bridge` ABI 23. `ValidateGovernanceDagBlockJson(...)` accepts an
optional expected block CID, while `ValidateGovernanceDagHeadChainJson(...)`
accepts at most 64 root-to-head `SoraFsGovernanceDagBlockInput` snapshots. Both
return the native `ValidationOutcomeV1` JSON only after strict UTF-8, exact V1
field/type, duplicate-field, version, and caller-bound `generated_at` checks.

Inputs are defensively copied, labels are exact UTF-8 without padding or
control characters and are capped at 1,024 bytes, and each call is capped at
64 MiB aggregate input. Native output is bounded and released with
`connect_norito_free` on success and error paths. `IsAvailable()` requires the
ABI probe plus both Governance DAG symbols; no managed fallback reimplements
Norito or governance signature validation.

## Live Testnet Smoke

The integration project is opt-in and can target the public Taira testnet:

```bash
export PATH="$HOME/.dotnet:$PATH"
export IROHA_CSHARP_RUN_LIVE_TESTS=1
export IROHA_CSHARP_TORII_BASE_URL=https://taira.sora.org
cd csharp
dotnet test tests/Hyperledger.Iroha.Sdk.IntegrationTests/Hyperledger.Iroha.Sdk.IntegrationTests.csproj -c Release
```

The live smoke probes public reads plus these account-authenticated runtime reads:

- `/v1/node/capabilities` (canonical account signature required)
- `/v1/runtime/abi/active` (canonical account signature required)
- `/v1/accounts`
- `/v1/explorer/accounts/{account_id}/qr`
- `/v1/explorer/accounts`, `/v1/explorer/domains`, `/v1/explorer/asset-definitions`, `/v1/explorer/assets`, `/v1/explorer/nfts`, and `/v1/explorer/rwas` with first-item detail reads when present
- `/v1/contracts/instances/{ns}` when the deployment exposes contract metadata, using `universal` by default, with `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view` when a code hash is available from that namespace or the override env var below
- `/v1/identifier-policies`
- `/v1/vpn/profile`
- `/v1/aliases/by-account`
- `/v1/accounts/faucet/puzzle`
- `/v1/space-directory/uaids/{uaid}`
- `/v1/space-directory/uaids/{uaid}/manifests`

Optional live-smoke environment variables:

- `IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE` to override the default contract-instance namespace (`universal`)
- `IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH` to override the code hash used for `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view`
- `IROHA_CSHARP_SMOKE_SORAFS_CID` and optional `IROHA_CSHARP_SMOKE_SORAFS_PATH` to also probe `/v1/sorafs/cid/{cid}` plus `/sorafs/cid/{cid}/...`

The live smoke requires runtime-only `IROHA_CSHARP_CANONICAL_ACCOUNT_ID`,
`IROHA_CSHARP_PRIVATE_KEY_SEED_HEX`, and the deployment's exact checksummed
`IROHA_CSHARP_NETWORK_ID`. They authenticate node-capability and active-ABI reads
and also enable the signed VPN quote probe; do not persist these values in source.

## Fee quotes and sponsor programs

Every `TransactionBuilder` requires the exact checksummed `NetworkId` derived
from the deployment's genesis header and a `FeePaymentIntent`. Ordinary client
transactions cannot use the genesis transaction-domain marker. The guided ledger
flow freezes the unsigned payload, account-signs `POST /v1/fees/quote`, verifies
that the quote retained the payer, exact sponsor program/revision, and gas
bound, then replaces only the charge maxima before signing:

```csharp
using Hyperledger.Iroha;
using Hyperledger.Iroha.Transactions;

var requested = FeePaymentIntent.Sponsor(
    new FeeSponsorProgramId(sponsorAccountId, "wallet_payments"),
    programRevision: 3,
    chargeLimits: Array.Empty<FeeChargeLimit>());

var transaction = client.Ledger
    .BuildTransaction(NetworkId.Parse(networkId), authorityAccountId, requested)
    .TransferAsset(assetId, NumericV1.QuantityValue.ParseCanonical("1"), destinationAccountId)
    .SetCreationTime(DateTimeOffset.UtcNow)
    .SetTimeToLiveMilliseconds(30_000);

var quoted = await client.Ledger.QuoteAndSignAsync(transaction, privateKeySeed);
```

Configure `ToriiClientOptions.CanonicalRequestCredentials` for the same
authority and `LocalSigningContext` for the exact network before calling the
guided flow. Use
`client.Torii.GetFeeSponsorProgramAsync(programId)` to inspect one exact
lifecycle record. Contract/IVM intents also require a positive gas bound.
Metadata keys `fee_sponsor`, `gas_asset_id`, and `gas_limit` are retired and
rejected, and sponsor rejection never falls back to the authority.

### Atomic mixed executable batches

Append native instructions and deployed-contract calls to the same builder in
their intended execution order:

```csharp
var transaction = client.Ledger
    .BuildTransaction(NetworkId.Parse(networkId), authorityAccountId, feeIntentWithGasLimit)
    .AddInstruction(registerInstruction)
    .AddContractCall(new TransactionContractInvocation(
        contractAddress,
        expectedCodeHash,
        "apply",
        argumentRecord))
    .AddInstruction(transferInstruction);
```

The builder emits `Executable::Batch` as soon as a contract call is present.
The node applies all items atomically, and the batch shares one positive,
signature-bound gas limit. Empty batches and noncanonical contract addresses
are rejected before signing; contract addresses must use lowercase V1 Bech32m.

## Sample

```csharp
using Hyperledger.Iroha;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

var accountId = Environment.GetEnvironmentVariable("IROHA_CSHARP_CANONICAL_ACCOUNT_ID")!;
var privateKeySeed = Convert.FromHexString(
    Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX")!);
var networkId = NetworkId.Parse(
    Environment.GetEnvironmentVariable("IROHA_CSHARP_NETWORK_ID")!);
using var client = new IrohaClient(
    new Uri("https://taira.sora.org"),
    toriiOptions: new ToriiClientOptions
    {
        LocalSigningContext = new ToriiLocalSigningContext(networkId),
        CanonicalRequestCredentials = new CanonicalRequestCredentials(
            accountId,
            privateKeySeed),
    });
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();
    if (faucetPuzzle.NetworkId != networkId)
    {
        throw new InvalidOperationException(
            $"Faucet puzzle targets {faucetPuzzle.NetworkId}, not configured network {networkId}.");
    }

    Console.WriteLine($"ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"First page size: {accounts.Items.Count}");
    Console.WriteLine($"Alias count for first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty: {faucetPuzzle.DifficultyBits}");
    Console.WriteLine($"Faucet puzzle exact network: {faucetPuzzle.NetworkId}");

    // Pin this independently from trusted deployment configuration and pass the
    // same value to PrepareAccountFaucetAsync and SubmitPreparedAccountFaucetAsync.
    var faucetPolicy = new ToriiAccountFaucetPolicyV1(
        Environment.GetEnvironmentVariable("IROHA_CSHARP_FAUCET_AUTHORITY")!,
        Environment.GetEnvironmentVariable("IROHA_CSHARP_FAUCET_ASSET_DEFINITION_ID")!,
        NumericV1.QuantityValue.ParseCanonical(
            Environment.GetEnvironmentVariable("IROHA_CSHARP_FAUCET_AMOUNT")!));

    // Transaction building is available through client.Ledger.
    //     var transaction = client.Ledger
    //         .BuildTransaction(
    //             networkId,
    //             accounts.Items[0].Id,
    //             FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
    //         .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", NumericV1.QuantityValue.ParseCanonical("1"), accounts.Items[0].Id)
    //         .SetCreationTime(DateTimeOffset.UtcNow)
    //         .SetTimeToLiveMilliseconds(5_000)
    //         .SetNonce(1);
    //     var signed = await client.Ledger.QuoteAndSignAsync(
    //         transaction,
    //         privateKeySeed);
    //
    //     Console.WriteLine($"Signed tx hash: {signed.Transaction.TransactionHashHex}");
    //     // await client.Ledger.SubmitAsync(signed.Transaction);
    //     // var status = await client.Ledger.WaitForAsync(signed.Transaction.TransactionHashHex);
}
catch (ToriiApiException exception)
{
    Console.WriteLine($"Torii status: {(int?)exception.StatusCode}");
    Console.WriteLine(exception.ResponseBody);
}
```

## Layout

- `src/Hyperledger.Iroha.Sdk/` - package source
- `tests/Hyperledger.Iroha.Sdk.Tests/` - unit and fixture-parity tests
- `tests/Hyperledger.Iroha.Sdk.IntegrationTests/` - live-network integration test lane
- `samples/Hyperledger.Iroha.Sdk.Sample/` - minimal sample app

## Current Ledger Coverage

- `TransactionBuilder.TransferAsset(...)`
- `TransactionBuilder.MintAsset(...)`
- `TransactionBuilder.BurnAsset(...)`
- `TransactionBuilder.SetAssetKeyValue(...)`
- `TransactionBuilder.RemoveAssetKeyValue(...)`
- `TransactionBuilder.SetDomainKeyValue(...)`
- `TransactionBuilder.RemoveDomainKeyValue(...)`
- `TransactionBuilder.SetAccountKeyValue(...)`
- `TransactionBuilder.RemoveAccountKeyValue(...)`
- `TransactionBuilder.SetAssetDefinitionKeyValue(...)`
- `TransactionBuilder.RemoveAssetDefinitionKeyValue(...)`
- `SignedQueryBuilder.FindExecutorDataModel(...)`
- `SignedQueryBuilder.FindParameters(...)`
- `SignedQueryBuilder.FindAbiVersion(...)`
- `SignedQueryBuilder.FindAliasesByAccountId(...)`
- `SignedQueryBuilder.FindProofRecordById(...)`
- `SignedQueryBuilder.FindAssetById(...)`
- `SignedQueryBuilder.FindAssetDefinitionById(...)`
- `SignedQueryBuilder.FindContractManifestByCodeHash(...)`
- `SignedQueryBuilder.FindTwitterBindingByHash(...)`
- `SignedQueryBuilder.FindDomainEndorsements(...)`
- `SignedQueryBuilder.FindDomainEndorsementPolicy(...)`
- `SignedQueryBuilder.FindDomainCommittee(...)`
- `SignedQueryBuilder.FindDaPinIntentByTicket(...)`
- `SignedQueryBuilder.FindDaPinIntentByManifest(...)`
- `SignedQueryBuilder.FindDaPinIntentByAlias(...)`
- `SignedQueryBuilder.FindDaPinIntentByLaneEpochSequence(...)`
- `SignedQueryBuilder.FindSorafsProviderOwner(...)`
- `SignedQueryBuilder.FindDataspaceNameOwnerById(...)`
- `SignedIterableQueryBuilder.FindDomains(...)`
- `SignedIterableQueryBuilder.FindAccounts(...)`
- `SignedIterableQueryBuilder.FindAssets(...)`
- `SignedIterableQueryBuilder.FindAssetDefinitions(...)`
- `SignedIterableQueryBuilder.FindRepoAgreements(...)`
- `SignedIterableQueryBuilder.FindNfts(...)`
- `SignedIterableQueryBuilder.FindRwas(...)`
- `SignedIterableQueryBuilder.FindTransactions(...)`
- `SignedIterableQueryBuilder.FindRoles(...)`
- `SignedIterableQueryBuilder.FindRoleIds(...)`
- `SignedIterableQueryBuilder.FindPeers(...)`
- `SignedIterableQueryBuilder.FindActiveTriggerIds(...)`
- `SignedIterableQueryBuilder.FindTriggers(...)`
- `SignedIterableQueryBuilder.FindAccountsWithAsset(...)`
- `SignedIterableQueryBuilder.FindPermissionsByAccountId(...)`
- `SignedIterableQueryBuilder.FindRolesByAccountId(...)`
- `SignedIterableQueryBuilder.FindBlocks(...)`
- `SignedIterableQueryBuilder.FindBlockHeaders(...)`
- `SignedIterableQueryBuilder.FindProofRecords(...)`
- `SignedIterableQueryBuilder.Continue(...)`
- `LedgerClient.SubmitAsync(...)`
- `LedgerClient.SubmitAndWaitAsync(...)`
- `ToriiClient.GetPipelineTransactionStatusAsync(...)`
- `ToriiClient.GetPipelineTransactionDetailsAsync(...)`
- `ToriiClient.SubmitSignedQueryAsync(...)`
- `ToriiClient.OpenEventSseAsync(...)`
- `ToriiClient.StreamEventsAsync(...)`
- `ToriiClient.StreamPipelineEventsAsync(...)`
- `ToriiClient.StreamProofEventsAsync(...)`
- `ToriiClient.GetExplorerAccountsAsync(...)`
- `ToriiClient.GetExplorerDomainsAsync(...)`
- `ToriiClient.GetExplorerAssetDefinitionsAsync(...)`
- `ToriiClient.GetExplorerAssetsAsync(...)`
- `ToriiClient.GetExplorerNftsAsync(...)`
- `ToriiClient.GetExplorerRwasAsync(...)`
- `ToriiClient.OpenExplorerBlocksSseAsync(...)`
- `ToriiClient.StreamExplorerBlocksAsync(...)`
- `ToriiClient.GetExplorerBlocksAsync(...)`
- `ToriiClient.GetExplorerBlockAsync(...)`
- `ToriiClient.OpenExplorerTransactionsSseAsync(...)`
- `ToriiClient.StreamExplorerTransactionsAsync(...)`
- `ToriiClient.GetExplorerTransactionsAsync(...)`
- `ToriiClient.GetExplorerLatestTransactionsAsync(...)`
- `ToriiClient.GetExplorerTransactionAsync(...)`
- `ToriiClient.OpenExplorerInstructionsSseAsync(...)`
- `ToriiClient.StreamExplorerInstructionsAsync(...)`
- `ToriiClient.GetExplorerInstructionsAsync(...)`
- `ToriiClient.GetExplorerLatestInstructionsAsync(...)`
- `ToriiClient.GetExplorerInstructionAsync(...)`
- `ToriiClient.GetExplorerHealthAsync(...)`
- `ToriiClient.GetExplorerMetricsAsync(...)`

The managed transaction encoder is deterministic and now covers the current
asset quantity, metadata, native SoraFS replication-order slice, and strict
asset-lock cancellation. The Torii client can also parse generic SSE frames,
project the common pipeline, proof, and explorer
block/transaction/instruction streams into typed models, read the core
explorer JSON endpoints including latest/health/metrics snapshots with typed
DTOs, and build/sign the current singular set plus the first canonical
iterable-query subset, but broader iterable families, richer instruction
families beyond that slice, broader typed event families, and the broader
parity surfaces are still open work.

## SoraFS replication-order instructions

The managed encoder exposes all three canonical V1 instructions through both
`TransactionInstruction` and fluent `TransactionBuilder` factories:

```csharp
var issue = TransactionInstruction.IssueReplicationOrder(
    orderId,
    replicationOrderBytes,
    issuedEpoch: 20,
    deadlineEpoch: 28,
    musubiArchiveId: archiveId);
var complete = TransactionInstruction.CompleteReplicationOrder(
    orderId,
    providerId,
    completionEpoch: 27,
    expectedAuthority: new ProviderIngestCompletionAuthorityV1(
        providerOwner,
        new ProviderIngestCompletionSignerPolicyV1(
            policyId,
            revision: 2,
            predecessorDigest,
            policyDigest)),
    expectedAssignmentRevision: 3,
    finalizedAnchor: new ProviderIngestFinalizedAnchorV1(
        height: 41,
        blockHash));
var expire = TransactionInstruction.ExpireReplicationOrder(
    orderId,
    expirationEpoch: 29);
```

IDs must be non-zero lowercase 64-hex strings. Issue accepts bytes or canonical
standard base64, caps the decoded archive at 1 MiB, and validates canonical
`ReplicationOrderV1` framing, ID binding, target/provider ordering, and
deadlines. Its fifth field is an optional `ArchiveId`: use `null` for a generic
order or a canonical archive ID to install the immutable Musubi purpose binding
atomically with the order. Completion requires the exact six-field authority
hard cut, including the provider owner, four-part signer-policy chain,
assignment revision, and finalized anchor. Missing, retired three-field, and
alias forms have no overload.

## Asset-lock cancellation

The managed transaction encoder exposes the strict compare-and-cancel V1
instruction through both `TransactionInstruction` and `TransactionBuilder`:

```csharp
var cancel = TransactionInstruction.CancelAssetLock(
    "merchant-lock-001",
    expectedRemainingAmount: "1500");
```

The builder derives the native `EscrowId` with Blake2b-256 and emits exactly
`escrow_id` plus `expected_remaining_amount`. The amount is mandatory, positive,
and canonically spelled. Lock-id preimages are capped at 4096 UTF-8 bytes and
must not have leading or trailing Unicode whitespace (including U+FEFF).
`CancelAssetLockInstruction.DecodeNorito(...)`, `DecodeInstructionJson(...)`,
and `DecodePayloadJson(...)` provide bounded strict decoding for instruction
envelopes and the shared V1 fixtures; they reject the retired one-field shape,
noncanonical quantities, zero preconditions, alternate layout flags, and
trailing bytes.

## Build

```bash
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$DOTNET_ROOT:$PATH"
cd csharp
dotnet restore Hyperledger.Iroha.Sdk.sln
dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore -warnaserror
```

## Test

```bash
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$DOTNET_ROOT:$PATH"
cd csharp
dotnet test Hyperledger.Iroha.Sdk.sln -c Release
```

The test projects use xUnit v3 executable test assemblies. When the .NET SDK is
installed in a non-system prefix, set `DOTNET_ROOT` to the SDK root so the
generated test app hosts can find the same runtime as the `dotnet` CLI.

For Windows native bridge release evidence, run the repository-root
`scripts/check_sccp_production_corridor.sh --phase dotnet-sdk` from a Windows
host with a stable .NET 8 SDK. That runner builds `connect_norito_bridge.dll`,
prepends its `debug` directory to the .NET restore/test `PATH`, and rejects
ambiguous inherited `PATH` values with empty path-list segments before bridge
build or test execution.

## Pack

NuGet files are authenticated release outputs and intentionally remain
untracked. Never reuse or publish a package produced for an older bridge ABI or
privacy surface; rebuild it from the exact clean revision and five-host evidence
described below.

The NuGet package is a five-RID hard cut. It contains exactly these native
assets:

| Rust target | NuGet RID | Package asset |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `linux-x64` | `runtimes/linux-x64/native/libconnect_norito_bridge.so` |
| `aarch64-unknown-linux-gnu` | `linux-arm64` | `runtimes/linux-arm64/native/libconnect_norito_bridge.so` |
| `x86_64-apple-darwin` | `osx-x64` | `runtimes/osx-x64/native/libconnect_norito_bridge.dylib` |
| `aarch64-apple-darwin` | `osx-arm64` | `runtimes/osx-arm64/native/libconnect_norito_bridge.dylib` |
| `x86_64-pc-windows-msvc` | `win-x64` | `runtimes/win-x64/native/connect_norito_bridge.dll` |

Build each library on a runner whose Rust host exactly matches its target. Each
runner must record and reverify the copied release library with
`scripts/check_native_sdk_abi23_artifact.py --sdk csharp`. Merge the five
uploads into an input root containing exactly
`<target>/<library>` and `<target>/native-sdk-abi23.json`, then assemble the
pack tree:

```bash
python3 scripts/package_csharp_native_artifacts.py stage \
  --input-root /absolute/path/to/csharp-native-inputs \
  --output-root csharp/artifacts/native-package \
  --source-root .

dotnet pack csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj \
  -c Release \
  --no-build \
  --output csharp/artifacts/packages \
  -p:IrohaNativePackageRoot=csharp/artifacts/native-package \
  -p:IrohaNativePackagePython=python3
```

Both commands require the same clean Git revision recorded by all five
canonical ABI-23 manifests. Missing, extra, noncanonical, substituted, or stale
inputs fail before NuGet packing. The project invokes `verify-stage` again
immediately before `GenerateNuspec`; CI then runs `verify-package` against the
primary `.nupkg` and exercises that package on all five native hosts. The
workflow is source-complete, but it is not evidence that a five-host run has
passed until the corresponding CI artifacts exist.

From the `csharp/` directory, the release package and package-consumer gate use
the same paths as CI:

```bash
dotnet restore Hyperledger.Iroha.Sdk.sln
dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore -warnaserror
dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj -c Release --no-build --output artifacts/packages
CSHARP_SDK_PACKAGE_CONSUMER_RUNTIME_IDENTIFIER=linux-x64 \
  ../ci/check_csharp_sdk_package_consumer.sh
```

The package-consumer guard creates an isolated temporary `net8.0` application,
installs `Hyperledger.Iroha.Sdk` from `csharp/artifacts/packages`, verifies the
consumer project uses `PackageReference` rather than `ProjectReference`, builds
with warnings as errors, and runs managed Ed25519, canonical request, and SCCP
route checks through the packed NuGet assembly. It copies `csharp/global.json`
into the temporary consumer so newer SDKs installed on hosted runners cannot
supersede the reviewed .NET 8 SDK corridor. Set
`CSHARP_SDK_PACKAGE_CONSUMER_RUNTIME_IDENTIFIER` to the current reviewed RID;
the guard also verifies the NuGet runtime inventory and requires the packaged
ABI-23 appeal-finance bridge:

From the repository root, the equivalent consumer invocation is:

```bash
CSHARP_SDK_PACKAGE_CONSUMER_RUNTIME_IDENTIFIER=linux-x64 \
  ci/check_csharp_sdk_package_consumer.sh
```
