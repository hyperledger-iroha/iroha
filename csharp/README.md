# Hyperledger Iroha C# SDK

Preview `.NET 8` SDK for Hyperledger Iroha.

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
- canonical Torii request signing headers with exact account, matching
  Ed25519 private seed, method, path, 16-byte lowercase-hex nonce, canonical
  64-byte signature-header base64, and positive timestamp validation before
  signing
- canonical request credentials and Ed25519 key pairs require exact 32-byte
  Ed25519 private seeds, require canonical request accounts to match the seed
  public key, and defensively copy private seed and public-key byte arrays on
  construction and access; Ed25519 signing and verification use
  exception-safe zeroing for managed temporary seed, expanded-key, message,
  signature, and public-key copies after use or failure
- a `LedgerClient` plus `TransactionBuilder` that can build, sign, and submit canonical asset/domain/asset-definition/NFT transfer transactions, asset mint/burn transactions, `SetAssetKeyValue`, `RemoveAssetKeyValue`, `SetDomainKeyValue`, `RemoveDomainKeyValue`, `SetAccountKeyValue`, `RemoveAccountKeyValue`, `SetAssetDefinitionKeyValue`, `RemoveAssetDefinitionKeyValue`, `SetNftKeyValue`, `RemoveNftKeyValue`, `SetTriggerKeyValue`, `RemoveTriggerKeyValue`, `MintTriggerRepetitions`, `BurnTriggerRepetitions`, and `ExecuteTrigger` transactions with deterministic hashes and pipeline-status polling
- managed transaction builders and Norito encoders validate exact chain/account/asset/domain/NFT/trigger/metadata/numeric/hash boundary fields before signing, including canonical I105 transaction authorities and account-bearing instruction fields, noncanonical numeric aliases such as signed positives, missing integer/fraction digits, leading integer zeros, and negative zero, transaction creation times must be positive Unix milliseconds, asset transfer/mint/burn quantities must be positive, and trigger repetition mints/burns must be positive before signing; JSON metadata values remain free-form but are defensively snapshotted by transaction builders and JSON-bearing instruction records so later caller mutation cannot alter signed payloads, public transaction-builder instruction/metadata accessors return detached snapshots, and signed transaction/query envelopes defensively copy all exposed byte arrays while rejecting empty, mismatched, malformed encoded-body, or wrong-size direct constructor byte fields before callers submit or trust them
- typed Torii runtime and account-query models for capabilities, ABI, account pages, explorer account/domain/asset inventory pages and details, explorer QR snapshots, explorer asset-definition econometrics and holder snapshots, explorer block/transaction/instruction pages, details, latest snapshots, health, metrics, and instruction contract-view reads, typed contract metadata/code-bytes/instance/state reads, write-side contract deploy/instance-activate/call/multisig propose/approve helpers, verifying-key registry register/update helpers, read-only contract-view execution under `/v1/contracts/view`, typed verified-source job submit/status helpers, typed contract code-view reads under `/v1/contracts/code/{code_hash}/contract-view`, asset balances, transaction summaries, permissions, identifier policy listing, identifier resolution, reverse alias lookup, account and contract alias resolution, alias-index lookup, account onboarding, faucet puzzle and claim flows, multisig onboarding, UAID portfolio reads, and space-directory bindings and manifest inventory reads; node capabilities responses plus raw node capability DTO deserialization reject null nested capability objects, duplicate/type-confused capability fields, duplicate keys inside ignored capability extension JSON, missing or malformed signed-transaction schema hashes, missing or malformed query/projection capability adverts, drifted query projection constants/feature flags, missing or malformed crypto capability labels/lists, duplicate or missing control-plane signing labels, SM2/default-hash mismatches, inconsistent SM acceleration adverts, inconsistent curve allowlist id/bitmap pairs, non-`1` ABI versions, and negative data-model values; runtime metadata responses and raw runtime metadata DTO deserialization reject duplicate raw runtime properties at any nested depth, missing runtime ABI version/policy/hash fields, malformed runtime ABI policy/hash text, negative runtime counters, and inconsistent runtime upgrade counter totals; onboarding requests require an explicit canonical UAID, exactly one account material field (`account_id` or `public_key_hex`), canonical I105 `account_id` values when supplied, digest-only `identity_commitment_hex` instead of raw identity metadata, exact permissions, canonical I105 multisig member account ids, and checked multisig member/weight shapes before HTTP dispatch; faucet puzzle responses and raw faucet puzzle DTO deserialization reject missing or non-exact PoW algorithms and anchor hashes, duplicate raw properties, duplicate keys inside ignored puzzle extension JSON, malformed numeric shapes, malformed salt hex, and unsafe scrypt parameters before solving, and direct faucet-puzzle DTO construction rejects malformed algorithms, anchors, hashes, salts, age windows, and scrypt work factors; onboarding, faucet, contract-call, and multisig responses plus raw onboarding/faucet DTO deserialization reject duplicate/type-confused envelopes, duplicate keys inside ignored onboarding/faucet response extension JSON, missing or noncanonical onboarding/faucet account ids, malformed UAID, asset, amount, and status strings, and non-exact non-empty transaction hashes before callers trust queued or executed transaction ids, and direct onboarding/faucet response DTO construction rejects malformed account ids, UAID, asset, amount, status, and transaction hash metadata; UAID route literals and UAID portfolio asset filters reject padded, whitespace-containing, and control-character values before dispatch while still canonicalizing accepted bare/`uaid:` hex casing, and UAID portfolio/bindings/manifest responses plus raw UAID DTO deserialization reject duplicate/type-confused envelopes, duplicate keys inside ignored UAID portfolio/bindings/manifest extension JSON, noncanonical UAID literals, noncanonical nested account ids, malformed nested dataspace/asset text, noncanonical quantities, null nested lists/items, non-exact manifest hashes, duplicate object keys inside preserved manifest JSON, and negative counters before callers trust space-directory inventory; contract write and multisig propose/approve requests validate exact selectors, aliases, signer material, canonical I105 contract authorities, contract-call fee sponsors, and multisig fee sponsors, canonical I105 explicit multisig account selectors and signer account ids, canonical base64 frames, 32-byte hashes, and positive gas limits before HTTP dispatch, deploy/activation responses plus raw deployment/activation DTO deserialization reject false `ok` envelopes, duplicate/type-confused fields, duplicate keys inside ignored deployment/activation extension JSON, missing or malformed deploy nonces, missing or non-exact returned contract address, dataspace, namespace, contract id, and deployment hash material, contract-call responses plus raw contract-call DTO deserialization reject false `ok` envelopes, duplicate/type-confused fields, duplicate keys inside ignored contract-call extension JSON, non-exact returned dataspace/contract-id/address/entrypoint text, returned code/ABI/transaction hashes, creation-time counters, and canonical scaffold/signed-transaction/signing-message base64 material, contract-view success responses plus raw success DTO deserialization reject false `ok` envelopes, duplicate/type-confused fields, duplicate keys inside ignored contract-view success extension JSON, non-exact returned dataspace/contract-id/address/entrypoint text and returned code/ABI hashes while preserving opaque non-duplicate result JSON, contract-view error responses plus raw error/VM-diagnostic DTO deserialization reject true `ok` envelopes, duplicate/type-confused fields, duplicate keys inside ignored error/VM-diagnostic extension JSON, non-exact returned dataspace/contract-id/address/entrypoint/error text, VM diagnostic text/counter inconsistencies, missing VM diagnostic counters/flags, non-positive VM diagnostic gas/cycle/stack limits, and returned code/ABI hashes, generic and contract-call multisig responses plus raw multisig response DTO deserialization reject false `ok` envelopes, duplicate/type-confused fields, duplicate keys inside ignored multisig extension JSON, noncanonical I105 resolved multisig account ids, non-exact proposal ids, instructions hashes, transaction hashes, creation-time counters, and canonical `signing_message_b64` text before callers trust returned signing material, and contract deploy uses Torii's current `contract_alias` request field; contract read/admin routes validate exact 32-byte code hashes, namespace/job ids, state paths, and instance/state query filters, including positive limits, before HTTP dispatch, contract metadata/code-view/verified-source/runtime-ABI responses plus raw contract manifest/code-view/source-reference/job DTO deserialization reject duplicate/type-confused manifests, access hints, entrypoints, params, analysis/memory/syscall counters, verified-source refs and jobs, non-exact returned hash fields and runtime ABI policy labels, missing or non-exact instance inventory namespace/contract-id text, inconsistent instance pagination counters, missing or malformed verified-source job ids/status/timestamps/messages, nested verified-source provenance digests/text, and malformed code-view permissions, access hints, entrypoints, analysis/syscall lists, warnings, and rendered-source fields, contract code-byte responses reject non-exact `code_b64` before returning code material, and contract state responses reject missing or non-exact entry paths/path-list elements, non-exact path/prefix/decode-error text, malformed/null state entries, duplicate object keys inside decoded `value_json`, not-found entries carrying value material, stale `next_offset`, over-limit item counts, non-exact `value_b64`, and mismatched decoded lengths before returning state material
- direct generic and contract-call multisig response DTO construction rejects
  false `ok`, malformed resolved account ids, proposal/instruction hashes,
  transaction hashes, creation times, and signing-message base64 before callers
  can serialize or trust manually constructed signing material
- direct contract deploy, deploy-and-activate, and activate response DTO
  construction rejects false `ok`, malformed contract addresses, dataspace,
  namespace, contract id, and code/ABI hash metadata before callers can
  serialize or trust manually constructed deployment results
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
- direct contract manifest/code-record and verified-source reference/job DTO
  construction rejects malformed manifest hashes, source provenance text,
  verified-source job ids/status/timestamps/messages, and actual-code hashes
  before callers can serialize or trust manually constructed metadata records
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
- direct runtime ABI/metrics DTO construction rejects non-v1 ABI versions,
  missing metrics counters, negative upgrade counters, non-`V1` ABI hash
  policies, and malformed ABI hashes before callers can serialize or trust
  manually constructed runtime metadata
- node capability list DTOs snapshot assigned arrays and return detached arrays
  on access for signing labels, curve ids/bitmaps, row enrichment fields,
  aggregate resources, projection metadata keys, and export resources while
  preserving malformed null/missing list rejection in raw converters
- raw contract instance inventory DTO deserialization shares the same fail-closed checks as `ToriiClient`, rejecting null, duplicate, or type-confused list/item fields, duplicate keys inside ignored instance/response extension JSON, missing or malformed unsigned counters, missing or non-exact namespace/contract-id/code-hash text, and inconsistent pagination counters before callers trust instance listings
- account and multisig onboarding request DTOs snapshot permission, member
  account, and member-weight lists on assignment and access, and direct
  permission/member-account list initialization rejects null elements; multisig
  normalization also copies the member lists into dispatch snapshots before
  HTTP serialization
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
- raw contract deploy and state response DTO deserialization requires returned `deploy_nonce`, `offset`, and `limit` counters to be present instead of defaulting absent wire fields to trusted zero values
- raw contract-view VM diagnostic DTO deserialization requires returned
  gas/cycle/stack limit fields to be present instead of defaulting absent limit
  fields to trusted zero values
- raw contract code-byte DTO deserialization rejects null, non-object, missing, duplicate, duplicate keys inside ignored extension JSON, and type-confused `code_b64` envelopes before preserving the existing exact base64 `FormatException` behavior for malformed code material
- contract code-view access-hint, entrypoint, analysis, permission, and warning
  list DTOs snapshot assigned arrays and return detached arrays on access while
  preserving malformed missing/null list rejection in the raw converters
- verifying-key registry read/write helpers validate exact backend/name route text, canonical I105 authority, private-key, circuit, gas-schedule, curve, CID, commitment, and status request fields, plus positive version, `vk_len`, and max proof-size fields before HTTP dispatch, and inline verifying-key byte arrays are snapshotted on assignment and access before serialization; accepted 32-byte hex casing/prefixes and status labels are canonicalized only after exact text preflight, verifying-key read responses validate the detail envelope, id/record backend-name material, version/height/status fields, lowercase hashes, inline key base64, and record metadata before returning raw JSON to callers, and verifying-key register/update responses reject malformed or non-accepted envelopes before returning raw JSON to callers
- identifier resolve requests validate exact policy ids and encrypted-input text before dispatch; identifier policy listing responses plus raw policy summary/page DTO deserialization validate exact policy ids, resolver public keys, page totals, list items, and duplicate object keys inside decoded policy parameter or ignored extension JSON, and both ToriiClient/raw resolve receipts require the current nested `payload`/`attestation` envelope, reject retired flat receipt fields, and validate exact payload identifiers, attestation signatures, canonical proof base64, execution/opening metadata, duplicate object keys, and positive canonical timestamp strings before callers trust Torii JSON responses; padded, whitespace-containing, control-character, malformed, noncanonical, zero-timestamp, unknown-extension, or mixed signed/proof receipt shapes fail during deserialization
- identifier policy, contract instance inventory, and contract state response list
  DTOs snapshot assigned arrays and return detached arrays on access while
  preserving malformed null/missing list rejection in raw converters
- explorer and account route reads validate exact path-segment identifiers before HTTP dispatch, including account/domain/asset/NFT/RWA/block/transaction/instruction identifiers, while account asset/transaction/permission routes require canonical I105 account ids; explorer transaction/instruction authority filters, instruction account filters, and explorer owner filters for domains, asset definitions, assets, NFTs, and RWAs require canonical I105 account ids, and other explorer/account query filters for domain, asset, transaction, instruction kind, and account asset scope/id fields reject blank, padded, whitespace-containing, and control-character values before dispatch; account list responses plus raw account summary/page DTO deserialization reject null list/items, duplicate/type-confused fields, missing or noncanonical I105 account ids, and negative totals, explorer account/domain directory responses plus raw directory DTO deserialization reject null pagination/list/items, duplicate/type-confused envelopes, malformed pagination, zero page/per-page counters, item counters, noncanonical account/I105/owner account ids, and malformed domain or logo text, explorer asset-definition/asset/NFT/RWA inventory responses plus raw inventory DTO deserialization reject null pagination/list/items, duplicate/type-confused envelopes, malformed pagination, zero page/per-page counters, item counters, malformed RWA booleans, malformed identifiers/status/reference fields, noncanonical owner and asset account ids, noncanonical quantity/value fields, and malformed RWA parent lists before callers trust indexed inventory, explorer QR/health/metrics responses plus raw QR/health/metrics/duration DTO deserialization reject duplicate/type-confused fields, duplicate keys inside ignored snapshot extension JSON, missing or noncanonical QR canonical account ids, malformed QR literal/rendering text, malformed QR dimensions, missing health sample timestamps, unsigned counters, durations, and malformed timestamp text before callers trust rendered or aggregated explorer projections, explorer econometrics/holder snapshot responses plus raw econometrics/snapshot DTO deserialization reject duplicate/type-confused windows, counters, ratios, and holder/distribution lists, noncanonical holder account ids, malformed definition identifiers, null econometrics and holder-distribution lists/items, noncanonical numeric strings, out-of-range distribution ratios, and null distributions before callers trust aggregated explorer projections, account asset-balance responses plus raw asset balance/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or malformed asset/scope/name text, noncanonical I105 account ids, malformed alias text, missing or noncanonical quantity text, and negative totals, account permission responses plus raw permission/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or malformed permission names, duplicate object keys inside arbitrary payloads, and negative totals while preserving arbitrary non-duplicate permission payload JSON, and account transaction-summary responses plus raw transaction summary/page DTO deserialization reject null list items, duplicate/type-confused fields, missing or non-exact entrypoint hashes, noncanonical I105 authority text, boolean confusion, non-positive timestamps, and negative totals before callers trust account history; explorer block/transaction/instruction REST, SSE, and raw DTO paths reject malformed, JSON-null, or duplicate-key SSE data frames, duplicate raw DTO properties, missing required summary string fields, noncanonical transaction/instruction authority text, non-exact block/transaction hashes, zero page/per-page pagination metadata, malformed summary `created_at` timestamps, transaction-detail nonce and TTL type confusion, instruction/rejection encodings with uppercase `0X` prefix aliases, lowercase transaction signatures, null stream/list items, missing or malformed instruction boxes, and malformed unsigned counters before callers trust indexed ledger projections
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
  `JsonObject` values and return detached clones on access for onboarding
  identity, account permission payloads, explorer metadata/rejection/instruction
  payloads, UAID manifests, identifier parameter/signature payloads, contract
  state/view/call payloads, multisig contract-call payloads, and SSE JSON data
- raw explorer block, transaction, and instruction page/latest DTO deserialization requires pagination, item lists, latest sample timestamps, summary creation timestamps, and unsigned projection counters to be present instead of defaulting absent page envelopes, item arrays, block height, transaction block, instruction block/index, block transaction-count, or summary `created_at` fields to trusted zero/empty values
- raw explorer QR/health/metrics/duration DTO deserialization requires QR
  account/rendering text, QR network prefixes, QR modules/versions, health head
  heights/sample timestamps, metrics aggregate counters, and duration `ms`
  fields to be present instead of defaulting absent snapshot fields to trusted
  empty/zero values
- raw contract-call, alias binding, and SoraFS denylist catalog DTO
  deserialization requires positive creation-time, binding timestamp, and
  catalog-version fields to be present instead of defaulting absent wire fields
  to trusted zero values
- contract-call/view response DTO deserialization requires returned dataspace,
  contract id, code/ABI hash, view entrypoint/error text, and VM diagnostic
  trap/message text to be present and non-null before exact validation runs;
  nullable returned contract addresses and call entrypoints remain optional but
  exact when present
- raw SoraFS denylist pack summary/response DTO deserialization requires
  `pack_id`, response `source_path`, `default_enabled`, `active`, and
  `entry_count` fields to be present instead of defaulting absent pack policy
  fields to trusted empty, `false`, or zero values
- raw VPN profile, quote, session, receipt, and receipt-list DTO deserialization
  requires operational timing, MTU, fee/grace, flow-label, and padding fields to
  be present instead of defaulting absent wire fields to trusted zero values
- raw faucet puzzle DTO deserialization requires algorithm, anchor hash, and
  numeric challenge/work-factor fields to be present instead of defaulting
  absent PoW parameters to trusted empty/zero values
- raw account transaction, account-alias lookup, VPN profile, contract deploy,
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
  returned UAID literals, portfolio asset/account/quantity strings, manifest
  hashes/status text, and account-list entries to be present, non-null, and
  canonical I105
  before exact validation runs; aliases, labels, revocation reasons, and
  preserved manifest JSON remain nullable where modeled optional
- UAID portfolio, bindings, and manifest inventory list DTOs snapshot assigned
  arrays and return detached arrays on access while preserving malformed
  null/missing list rejection in the raw converters
- direct UAID portfolio, bindings, manifest lifecycle/record, and manifest-list
  response DTO construction rejects malformed UAID literals, dataspace counters,
  canonical I105 account lists, asset ids, canonical quantities, manifest
  hashes/status text, lifecycle counters, and revocation metadata before callers
  serialize or trust manually constructed space-directory metadata
- account/asset/contract alias lookup and resolve helpers validate exact aliases, dataspace, and domain filters before HTTP dispatch, with by-account lookup requiring canonical I105 account ids; alias lookup/resolve responses plus raw account alias lookup, alias index, alias resolution, and alias binding DTO deserialization reject duplicate/type-confused envelopes, duplicate keys inside ignored alias lookup/index/resolution/binding extension JSON, missing or malformed aliases, noncanonical account ids, malformed asset/contract ids, dataspace/source/status fields, null lookup lists/items, required alias-index indexes, negative index fields, and non-positive binding timestamp fields before callers trust alias metadata; direct alias lookup/resolution/binding DTO metadata rejects malformed aliases, dataspace/domain/source/status fields, account ids, asset/contract identifiers, negative indexes, and non-positive binding timestamps before callers can serialize or trust manually constructed alias results
- typed Torii VPN and SoraFS helpers for `/v1/vpn/profile`, signed VPN quote/session/receipt flows under `/v1/vpn/quotes`, `/v1/vpn/sessions`, and `/v1/vpn/receipts`, `/v1/sorafs/cid/{cid}`, `/v1/sorafs/denylist/catalog`, `/v1/sorafs/denylist/packs/{pack_id}`, and CID content reads under `/sorafs/cid/{cid}/...`; VPN quote/session/receipt writes validate exact exit-class text, 32-byte quote/payment/metering/lease hex fields, and receipt/voucher hex payload shape before HTTP dispatch; VPN profile, quote, session, receipt, and receipt-list responses plus raw VPN response/native-instruction DTO deserialization reject null, duplicate, or type-confused envelopes, duplicate keys inside ignored VPN response/native-instruction extension JSON, noncanonical account, escrow, and operator account ids, missing or malformed route/DNS/tunnel lists, required native-instruction arrays, and counters, non-positive lease/DNS/MTU operational fields, zero or reversed VPN quote/session/receipt timestamps, missing VPN accounting counters, inconsistent receipt-list totals, and non-exact lowercase SPKI/key/hash/id/native-instruction hex fields before callers trust tunnel or settlement material; VPN session reads/deletes validate 32-byte hex route ids, SoraFS CID lookup/content reads validate lowercase multibase base32 CID route ids, and SoraFS denylist-pack/content relative path identifiers validate exact path segments before dispatch while preserving valid internal SoraFS pack/path text; SoraFS CID lookup responses plus raw CID lookup/file-entry DTO deserialization reject duplicate/type-confused fields, duplicate keys inside ignored CID lookup/file-entry extension JSON, missing or non-exact content CIDs, missing or malformed lowercase manifest digest/id hex, malformed path components, and missing or negative file geometry before callers trust gateway listings; buffered SoraFS content responses reject malformed or duplicated `sora-content-cid` headers and direct malformed content CIDs or negative content lengths before returning content metadata, and defensively copy buffered response bytes; SoraFS denylist catalog/pack responses plus raw denylist pack/catalog DTO deserialization reject duplicate/type-confused fields, duplicate keys inside ignored denylist catalog/pack extension JSON, null lists/items, missing or malformed pack ids/source paths, non-exact manifest CIDs, malformed metadata text, boolean confusion, non-positive catalog versions, and negative counts before callers trust pack policy metadata; SoraFS pin registration validates canonical I105 authority, exact signer, chunker, storage-class, alias, digest, canonical manifest/proof base64, and successor fields before HTTP dispatch, snapshots inline manifest byte arrays before serialization, and retains explicit digest and storage-class canonicalization after exact preflight; raw chunker/storage-class/pin-policy DTO deserialization rejects null, duplicate, case-drifted, or type-confused fields, duplicate keys inside ignored extension JSON, invalid counters, non-exact chunker text, and unknown storage-class values while canonicalizing omitted optional multihash/retention counters to zero; and pin alias/register responses plus raw pin DTO deserialization reject duplicate/type-confused envelopes, duplicate keys inside ignored pin alias/register extension JSON, noncanonical returned manifest/successor hashes, malformed alias namespace/name/proof base64, unsigned epoch/length/fee counters, malformed fee ids, noncanonical I105 fee treasury account ids, and non-exact `chunker_handle` values before returning trusted DTOs
- SoraFS CID lookup file path/file-list DTOs and denylist catalog opt-out,
  extra-pack, and pack-summary list DTOs snapshot assigned arrays and return
  detached arrays on access while preserving malformed null/missing list
  rejection in the raw converters
- direct SoraFS CID lookup/file, pin-register response, denylist summary,
  catalog, and pack response DTO construction rejects malformed CIDs, hashes,
  path components, noncanonical fee treasury accounts, alias proofs, pack
  metadata, required pin counters, and negative or non-positive counts before
  callers serialize or trust manually constructed SoraFS metadata
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
  `SignedIterableQueryBuilder` for the current fast_dsl iterable subset
  (`FindDomains`, `FindAccounts`, `FindAssets`, `FindAssetDefinitions`,
  `FindRepoAgreements`, `FindNfts`, `FindRwas`, `FindTransactions`,
  `FindRoles`, `FindRoleIds`, `FindPeers`, `FindActiveTriggerIds`,
  `FindTriggers`, `FindAccountsWithAsset`, `FindPermissionsByAccountId`,
  `FindRolesByAccountId`, `FindBlocks`, `FindBlockHeaders`,
  `FindProofRecords`, and cursor `Continue(...)`), and typed
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
  explorer pagination/latest wrapper metadata rejects zero `page`/`per_page`
  values and malformed `sampled_at` text before serialization; direct explorer
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
  transaction-status reads validate exact
  hash/scope request text before dispatch while retaining explicit 32-byte hash
  casing/`0x` canonicalization and empty-scope defaulting, and reject malformed
  status envelopes, non-exact returned hash/scope/resolution text, negative
  heights, and non-exact or noncanonical rejection-content base64 before
  returning status objects; `ToriiClient` construction validates absolute
  HTTP(S) base URIs without user-info, query, or fragment components while
  preserving exact path prefixes; low-level request setup validates exact
  HTTP Bearer token grammar, HTTP-token method text, and root-relative path
  request parts, rejects
  relative, absolute, scheme-relative, raw-colon, raw-backslash, malformed
  percent-escape, percent-decoded control-byte, and raw/percent-encoded
  dot-segment paths before dispatch or canonical signing, and validates
  optional query and single-media-range `Accept` text; the replay-capable
  explorer stream helpers separately validate `Last-Event-ID` text, with
  optional query text
  rejecting raw whitespace, ambiguous empty segments, empty/blank decoded
  parameter names, malformed percent escapes, invalid percent-encoded UTF-8,
  and percent-decoded controls before URI construction while still allowing
  percent-encoded or `+`-encoded spaces in values; public `configureRequest`
  hooks may add ordinary headers but cannot mutate the validated method, URI,
  content object, Authorization/Accept headers, signed content bytes, or
  canonical signing headers after request setup; canonical request signing
  requires generated or caller-supplied 16-byte lowercase-hex nonces and
  applies the same root-relative path guard and canonical query signing rejects
  the same ambiguous segments/names plus malformed-escape/control-byte drift
  before sorting or signing; SSE event-filter query preflight rejects malformed
  percent escapes, invalid percent-encoded UTF-8, percent-decoded control bytes,
  malformed JSON-shaped filter payloads, padded JSON filter payloads, and
  duplicate-key JSON filters before production filter validation; raw
  signed-query/transaction submissions reject empty Norito payloads before
  binary content is created
- `SignedIterableQueryBuilderTests` pin non-zero cursor, continue gas-budget,
  limit, and fetch-size preflight, the 10,000-row fetch-size ceiling, nullable pagination
  clearing, explicit sort clearing, and stale selector/parameter reset before
  signed Norito query bytes are produced
- `ToriiClientOptions` snapshots custom `JsonSerializerOptions` on assignment,
  public access, and `ToriiClient` construction so caller-owned option
  collection mutations cannot drift the effective client serializer
- faucet PoW solving validates `StartNonce`/`MaxAttempts` ranges before scrypt
  derivation so nonce enumeration cannot overflow mid-search
- a managed faucet PoW solver for `scrypt-leading-zero-bits-v1`, plus `ToriiClient` helpers that can fetch the current puzzle and prepare or submit a faucet claim for a canonical I105 account id; account ids, puzzle algorithm labels, anchor/salt/nonce hex, positive anchor-age bounds, bounded scrypt work factors including parallelization and ROMix memory, and claim PoW field pairs are validated before hashing or HTTP dispatch
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
- managed Offline Note receipt ACK value and Norito/text codecs for the
  `OfflineNoteReceiptAckEnvelope` handoff payload with exact compact
  layout-flag validation
- managed Offline Note canonical payload models and compact Norito codecs for
  key-certificate payloads, issued claims, redeem public inputs, audit public
  inputs, note-commitment preimages, input-nullifier preimages, and
  payment-token-id preimages, with exact text fields rejecting blank, padded,
  whitespace-containing, and control-character values plus forged packed-layout
  header adverts
- managed Offline Note receive-request Norito/text handoff codec with canonical
  account, asset, key-certificate, amount, and output-commitment binding plus
  exact chain/payment/account/asset/amount text preflight and forged
  packed-layout header rejection
- managed Offline Note payment-token Norito/text handoff codec with opaque
  audit-bundle bytes, exact chain/payment text and positive creation-time
  preflight, and recipient ACK matching, rejecting forged packed-layout headers
  on the token envelope and embedded audit bundles
- managed Offline Note wallet-note persistence record and JSON codec using the
  same field names as Swift/Kotlin/Android secure stores, with exact persisted
  scope/replay-prevention text fields and duplicate-property rejection before
  persisted JSON fields are trusted. Retired `spendPending`, `SPEND_PENDING`, `changePending`, and `CHANGE_PENDING`
  wallet-note state names are rejected; first-release records must use current state names.
- `IrohaClient`, `LedgerClient`, and `ToriiClient` entry points, with raw JSON helpers still available for uncovered endpoints
- fixture-backed unit tests against the repo's canonical address vectors

Broader iterable families beyond the current fast_dsl subset, richer typed event coverage beyond the current pipeline/proof/explorer SSE projections, broader contract admin/lifecycle helpers beyond deploy/activate/call/multisig plus verified-source job helpers, Connect, Nexus, and the remaining parity work are still planned.

`CreateVpnQuoteAsync(...)`, `CreateVpnSessionAsync(...)`,
`SubmitVpnReceiptAsync(...)`, and `DeleteVpnSessionAsync(...)` call signed Torii
routes, so set `ToriiClientOptions.CanonicalRequestCredentials` with a
canonical I105 account id before using those helpers. Session creation requires the quote id, committed
`OpenVpnLeaseEscrow` transaction hash, and the same metering public key that was
bound into the quote. Empty quote/session exit class still selects Torii's
profile default, and an empty receipt lease id still lets Torii derive the
settlement lease id from the relay receipt. Session deletion and operator
receipt submission return canonical receipt DTOs with earned/refund XOR fields
and any `SettleVpnLease` instruction skeleton Torii produced.

For SoraFS content, use `OpenSoraFsCidContentAsync(...)` when you want the raw
HTTP response/stream, or `GetSoraFsCidContentAsync(...)` when buffering the
payload into memory is acceptable. The buffered helper treats an absent
`sora-content-cid` header as absent optional metadata, but rejects duplicated or
noncanonical CID header values before returning content metadata, and the
returned byte array is snapshotted on assignment and access. Optional content
relative paths must already be exact relative paths: padded values, empty
segments, `.`/`..` traversal components, backslash separators, and control
characters fail before the gateway request is sent.

## Torii Offline API

The C# SDK exposes only the canonical first-release Offline lifecycle:

- `GET /v1/offline/readiness`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

There is no nested `/offline/v2`, note-issuer route, whole-payload base64
wrapper, or caller-supplied operation-id argument. `OfflineTopUpRequest` and
`OfflineRedeemRequest` accept the direct canonical Norito archive, require the
stable public request schema, uncompressed compact field framing, an exact
8/11-field root, no padding or trailing data, and derive `Idempotency-Key` from
the embedded nonzero 32-byte operation id.

```csharp
using Hyperledger.Iroha.Offline;
using Hyperledger.Iroha.Torii;

using var torii = new ToriiClient(new Uri("https://torii.example"));

var readiness = await torii.GetOfflineReadinessAsync("xor#wonderland");
Console.WriteLine($"evaluated at {readiness.EvaluatedBlockHeight}: {readiness.EvaluatedBlockHash}");
if (!readiness.Ready)
{
    foreach (var blocker in readiness.Blockers)
    {
        Console.WriteLine($"{blocker.Code}: {blocker.Message}");
    }
}

// Produced by the typed wallet/prover path; this is the request itself, not a wrapper.
byte[] canonicalTopUpArchive = GetCanonicalTopUpArchive();
var accepted = await torii.SubmitOfflineTopUpAsync(
    new OfflineTopUpRequest(canonicalTopUpArchive));

OfflineOperationStatus status =
    await torii.GetOfflineOperationStatusAsync(accepted.OperationId);
switch (status)
{
    case OfflineOperationStatus.Pending pending:
        Console.WriteLine($"pending: {pending.TransactionHash}");
        break;
    case OfflineOperationStatus.Applied applied:
        Console.WriteLine($"applied: {applied.Result.GetType().Name}");
        break;
    case OfflineOperationStatus.Rejected rejected:
        Console.WriteLine($"{rejected.Error.Code}: {rejected.Error.Message}");
        break;
}
```

Operation references and statuses are negotiated as
`application/x-norito`. The decoder returns closed pending/applied/rejected
models, distinct top-up/redemption results, a schema-bound top-up anchor for
the wallet prover, and closed queue/AXT error details. It rejects checksum,
schema, padding, non-minimal length, unknown variant, trailing-data, response
route/id, media-type, status-code, and `Location` inconsistencies before the
result reaches application code.

## Offline Cash Lifecycle

Use `OfflineCashLifecycleController` around the app's offline wallet for load
actions. It syncs pending audit receipts before issuing more cash, while local
device-to-device exchange should validate cached setup and avoid fresh
capability fetches.
Cached configuration snapshots reject blank, padded, whitespace-containing,
control-character, or non-ASCII chain, asset, artifact-set, and circuit
identifiers before offline exchange. Cached issuer public keys must be
canonical non-empty base64 text. Future-created snapshots,
expiry-at-or-before-created timestamps, expired snapshots, and zero or too-old
native bridge ABI gates also fail closed.

```csharp
using Hyperledger.Iroha.Offline;

var snapshot = new OfflineCashConfigurationSnapshot(
    ChainId: "00000042",
    AssetDefinitionId: "pkr#sbp",
    OfflinePaymentsEnabled: true,
    IssuerPublicKeyBase64: cachedIssuerPublicKeyBase64,
    NativeBridgeAbiVersion: 7,
    CreatedAtMs: cachedAtMs,
    ExpiresAtMs: expiresAtMs);
snapshot.RequireUsableForOfflineExchange(
    nowMs: currentTimeMs,
    requiredNativeBridgeAbiVersion: 7);

var controller = new OfflineCashLifecycleController(
    offlineWallet,
    auditReceiptSynchronizer);
await controller.LoadAsync("pkr#sbp", "500");

var transports = new OfflineCashTransportCapabilities(
    QrStreaming: true,
    Nfc: appHasHceEntitlement && deviceSupportsNfc
        ? OfflineCashNfcCapability.Available
        : OfflineCashNfcCapability.Unavailable("missing HCE"),
    Nearby: true);
```

UI layers must hide NFC when `SupportedTransportKinds()` omits `nfc`; non-NFC
devices and app builds without HCE should expose QR or Nearby only.

## Offline Note Receive Requests

`OfflineNoteReceiveRequest` and `OfflineNoteReceiveRequestCodec` expose the
Swift/Kotlin/Android-compatible receive-request handoff envelope. The codec uses
Norito schema `iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelope`,
compact lengths, type `offline_receive_request`, and text prefix
`wallet-offline-bearer-cash-receive:`.

```csharp
using Hyperledger.Iroha.Offline;

var request = new OfflineNoteReceiveRequest(
    chainId: "iroha-mainnet",
    paymentRequestId: "payment-request-7",
    accountId: accountId,
    assetDefinitionId: assetDefinitionId,
    assetId: assetId,
    amount: "15.7500",
    keyCertificateNorito: keyCertificateNorito,
    outputCommitment: outputCommitment);

var norito = OfflineNoteReceiveRequestCodec.EncodeNorito(request);
var text = OfflineNoteReceiveRequestCodec.EncodeText(request);
var decoded = OfflineNoteReceiveRequestCodec.DecodeNorito(norito);
```

The constructor and decoder reject wrong Norito schema/layout/checksum,
malformed compact fields, padded/blank/control metadata, non-canonical account
or asset ids, asset-definition/account mismatches, key certificates for a
different account, invalid key-certificate archives, non-canonical amounts,
noncanonical dataspace scope suffixes, malformed output commitments, trailing
bytes, whitespace-padded text envelopes, embedded-whitespace text envelopes,
and ambiguous or noncanonical base64url text.

## Offline Note Payment Tokens

`OfflineNotePaymentToken` and `OfflineNotePaymentTokenCodec` expose the
Swift/Kotlin/Android-compatible payment-token handoff envelope. The codec uses
Norito schema `iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope`,
compact lengths, type `offline_payment_token`, and text prefix
`wallet-offline-bearer-cash-payment:`.

```csharp
using Hyperledger.Iroha.Offline;

var token = new OfflineNotePaymentToken(
    chainId: "iroha-mainnet",
    paymentRequestId: "payment-request-7",
    createdAtMs: createdAtMs,
    tokenNonce: tokenNonce,
    tokenId: tokenId,
    auditNorito: auditBundleNorito);

var norito = OfflineNotePaymentTokenCodec.EncodeNorito(token);
var text = OfflineNotePaymentTokenCodec.EncodeText(token);
var decoded = OfflineNotePaymentTokenCodec.DecodeNorito(norito);
```

The managed surface keeps audit bundles as opaque Norito bytes, but decodes
enough of each audit bundle to reject wrong schemas/layouts, malformed compact
fields, zero `created_at_ms`, mismatched token ids, empty or unordered audit
vectors, mismatched output commitments/claims, unsupported recursive proof
metadata, empty proof bytes, and bearer trails that do not end with the token
audit. Receipt ACKs can be built from or matched against a token only when the
recipient account id is present in the token audit outputs. Text decoding
rejects surrounding or embedded whitespace before prefix parsing and rejects
noncanonical base64url payloads.

## Offline Note Receipt ACK

`OfflineNoteReceiptAck` and `OfflineNoteReceiptAckCodec` expose the
Swift/Kotlin/Android-compatible recipient acknowledgement payload. The codec
uses Norito schema
`iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope`, compact
lengths, type `offline_receipt_ack`, and text prefix
`wallet-offline-bearer-cash-ack:`.

```csharp
using Hyperledger.Iroha.Offline;

var ack = new OfflineNoteReceiptAck(
    chainId: "iroha-mainnet",
    paymentRequestId: "payment-request-7",
    tokenId: tokenIdBytes,
    recipientAccountId: "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
    acceptedAtMs: acceptedAtMs);

var norito = OfflineNoteReceiptAckCodec.EncodeNorito(ack);
var text = OfflineNoteReceiptAckCodec.EncodeText(ack);
var decoded = OfflineNoteReceiptAckCodec.DecodeNorito(norito);
```

The constructor and decoder require a canonical I105 `recipient_account_id` and
reject blank or whitespace-padded `chain_id` and `payment_request_id`, token ids
that are not 32 bytes, zero `accepted_at_ms`, wrong Norito schema/layout/checksum, malformed
compact lengths, invalid UTF-8 string payloads, trailing field bytes, and
whitespace-padded, embedded-whitespace, padded, or noncanonical base64url text
payloads.

## Offline Note Canonical Payloads

`OfflineNoteCanonicalPayloadCodec` exposes the shared compact Norito payload
surface for `OfflineNoteKeyCertificatePayload`, `OfflineNoteIssuedClaim`,
`OfflineNoteRedeemPublicInputs`, `OfflineNoteAuditPublicInputs`,
`OfflineNoteCommitmentPreimage`, `OfflineNoteInputNullifierPreimage`, and
`OfflineNotePaymentTokenIdPreimage`.

```csharp
using Hyperledger.Iroha.Offline;

var claim = new OfflineNoteIssuedClaim(
    noteCommitment: noteCommitment,
    keyCertificatePayloadHash: keyCertificatePayloadHash,
    assetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorau...",
    amount: "15.7500");

var norito = OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(claim);
var decoded = OfflineNoteCanonicalPayloadCodec.DecodeIssuedClaim(norito);
```

The models enforce exact derivation domains for
`iroha:offline-note:key-certificate-payload`,
`iroha:offline-note:issued-claim`,
`iroha:offline-note:redeem-public-inputs`,
`iroha:offline-note:audit-public-inputs`,
`iroha:offline-note:note-commitment`,
`iroha:offline-note:input-nullifier`, and
`iroha:offline-note:payment-token-id`; hash fields must be 32-byte Iroha
prehashes, note secrets and token nonces must be exactly 32 bytes, vectors must
be non-empty where the protocol requires it, audit claims must line up with
commitments, account ids must be canonical I105, asset definition ids must pass
the canonical base58/BLAKE3 checksum, asset-id dataspace scopes must use
canonical unsigned decimal text, and amounts are normalized to canonical
numeric text. Payment-token-id preimages also require a positive
`created_at_ms`. The preimage models derive note commitments, input
nullifiers, and payment token ids by hashing the wrapped compact Norito archive. The
decoders reject wrong schemas, non-exact compact layout flags including forged
packed-layout adverts, malformed compact lengths, invalid UTF-8, padded or
forged domains, trailing nested bytes, unsupported
account-controller tags, invalid option/bool tags, unsupported commitment
origin tags, empty vectors, and mismatched audit outputs before wallet code
trusts public inputs.

## Offline Note Wallet Notes

`OfflineNoteWalletNote` stores one wallet-owned Offline Note record with exact
`chain_id`, `account_id`, `asset_id`, optional `spent_payment_request_id`,
canonical numeric `amount`, opaque key-certificate Norito bytes, note
commitment/secret bytes, commitment origin, state, timestamps, and optional
bearer audit-trail Norito bytes. It is an opaque persistence surface for C#
apps; the full Offline Note proof model is still tracked separately.

```csharp
using Hyperledger.Iroha.Offline;

var note = new OfflineNoteWalletNote(
    chainId: "iroha-mainnet",
    accountId: "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
    assetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
    amount: "1000",
    keyCertificateNorito: keyCertificateBytes,
    noteCommitment: noteCommitmentBytes,
    noteSecret: noteSecretBytes,
    origin: new OfflineNoteCommitmentOrigin.IssuerLoad("load-op-1", "lineage-1", 3),
    state: OfflineNoteWalletNoteState.Spendable,
    createdAtMs: createdAtMs,
    updatedAtMs: updatedAtMs);

var stored = OfflineNoteWalletNoteJsonCodec.Encode(note);
var restored = OfflineNoteWalletNoteJsonCodec.Decode(stored);
```

The JSON codec writes version `1` with `chain_id`, `account_id`, `asset_id`,
`amount`, `key_certificate_norito_base64`, `note_commitment_hex`,
`note_secret_base64`, `origin`, `bearer_audit_trail_norito_base64`, `state`,
`created_at_ms`, `updated_at_ms`, and optional `spent_payment_request_id`.
It rejects noncanonical I105 account IDs, noncanonical asset IDs, mismatched
asset account components, padded or blank scope fields, invalid UTF-8 JSON,
duplicate root or nested object properties, unknown version-1 root fields,
origin fields that do not belong to the decoded origin variant, malformed or
noncanonical base64 payloads, malformed hex payloads, non-32-byte note
commitment or secret bytes, unknown origins or states, unsupported in-memory
state values, control-character scope fields, uppercase or prefixed
`note_commitment_hex`, noncanonical amount text, noncanonical or overflowing
numeric-string counters, zero `created_at_ms`, backwards `updated_at_ms`
values, and oversized origin output indices.

## Verifying Key Registry

`ToriiClient.RegisterVerifyingKeyAsync(...)` and
`UpdateVerifyingKeyAsync(...)` post Torii's `/v1/zk/vk/register` and
`/v1/zk/vk/update` payloads. The client validates production verifier backends,
canonical I105 authority values, required signing fields, height ranges, and
inline verifier-key commitments before the HTTP request is sent. Production
backend labels reject blank, padded, and whitespace-containing text before
unsupported-backend classification.
Production `VerifyingKey`/`Proof` SSE event filters apply the same fail-closed
policy to JSON filter text, verifier-key names, and proof hash matchers, so
padded filter JSON, padded/whitespace/control-character names, and uppercase or
`0x`-prefixed proof hashes fail before the subscription request is sent.

```csharp
await torii.RegisterVerifyingKeyAsync(new ToriiVerifyingKeyRegisterRequest
{
    Authority = canonicalAuthorityAccountId,
    PrivateKey = "ed25519:...",
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

## Native Kagemusha Recursive Spend

The optional `Hyperledger.Iroha.Offline.KagemushaRecursiveSpendNative` wrapper
calls the ABI-6 `connect_norito_bridge` recursive spend surface. `IsAvailable()`
requires native bridge ABI 6 or later plus `init`, `append`, both transition-profile
helpers, the append-boundary helper, both lineage-witness helpers, `verify`,
and `redeem` before reporting recursive spend support. Each entry point accepts
raw Norito request archives and returns raw Norito archive bytes; the C# SDK does
not reimplement prover internals.
`TransitionProfileInit(...)` and `TransitionProfileAppend(...)` return the
canonical Reserved-lineage accumulator transition profile as raw Norito
archives for fixture generation and circuit preflight.
`LineageAppendBoundary(...)` derives the compact append-boundary Norito archive
from a full append transition profile with native opening preflight material;
the C# SDK treats the result as opaque verifier material.
`ValidateRedeemLineagePreflight(...)` and
`ValidateRedeemChangeOutputPreflight(...)` let callers that already decoded
bundle metadata reject missing lineage material, partial-without-change,
over-amount, and full-with-change redeem requests before P/Invoke dispatch; the
metadata-bound `Redeem(...)` overloads apply those checks before calling the
native bridge. Amount metadata must be canonical positive unsigned decimal u128
text: no padding, sign characters, decimal points, zero, or overflow values.
`DecodeBundleSummary(...)` parses a recursive spend bundle archive in managed
code and returns defensive-copy summary metadata for hop count, proof circuit,
asset, chain id, roots, top-up anchor nullifiers, and the current note. It
rejects non-compact or wrong-type Norito bundles, unknown previous proof circuit
ids, non-`halo2/ipa` proof backends, empty proof bytes, empty proof public
inputs, zero proof public-input hashes, mismatched proof public-input hashes,
invalid accumulator domains, malformed fixed-size accumulator asset/root fields,
and `hop_count` values outside `1..64` before wallet code trusts the bundle.
Top-up anchor nullifiers must contain `1..FoldStepMaxInputs` non-zero 32-byte
values in strictly sorted unique order, and must not reuse the current note
commitment or spend nullifier; these accumulator checks run before recursive
proof parsing can mask them. Current-note summaries also reject all-zero note
commitments, all-zero spend nullifiers, note/nullifier aliasing, zero amounts,
and malformed fixed-array note commitment or spend nullifier encodings.
`DecodeTransitionProfileSummary(...)` parses ABI-6 init/append transition
profile archives in managed code and returns defensive-copy previous top-up
anchor and current-hop output commitment metadata. Init profiles must not carry
previous top-up anchors; append profiles must carry `1..FoldStepMaxInputs`
non-zero sorted unique previous anchors, and neither current-hop outputs nor
the current note commitment/spend nullifier may reuse a carried previous anchor.
Use `ValidateRedeemChangeOutputNotReserved(...)` or the bundle-summary
`Redeem(...)` overloads when a wallet has decoded the bundle and change
commitment: they reject a change output that reuses the current note commitment,
current note spend nullifier, or any top-up anchor nullifier before native
dispatch.
Native Kagemusha bridge outputs are rejected if they are empty, null, or larger
than 64 MiB before the wrapper copies them into managed memory. Bounded native
output buffers are zeroed before release on success, malformed-output, and
bridge-error paths, and managed native-call input archive copies are zeroed
after dispatch or sanitized native-call failure. Unexpected native availability
probe output buffers follow the same bounded zero-before-release rule.
The append-boundary digest uses the public
`RecursiveSpendLineageAppendBoundaryDomainV1` domain, plus
`RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1` and
`RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1` for chain/asset
and final-root/current-note binding.

Transaction builders expose the same Kagemusha instruction surface without
asking wallet code to reframe native archives. Use
`TransactionInstruction.KagemushaInstructionArchive(...)` or
`KagemushaInstructionArchiveInstruction` for a typed `KagemushaTransfer` or
`RedeemKagemushaRecursive` instruction archive,
`TransactionBuilder.KagemushaInstructionArchive(...)` to add a single archived
instruction, and `TransactionBuilder.KagemushaRecursiveRedeem(...)` to
derive the redeem instruction from a native recursive redeem request before
signing. Metadata-bound `KagemushaRecursiveRedeem(...)` overloads also apply
the managed lineage and amount/change-output preflight before native request
parsing or builder mutation. These builders require valid Norito archives,
reject empty, malformed, tampered, or wrong-type instruction archives, and keep
recursive redeem derivation inside the native bridge.

Use `PreferredMode(...)` to select `recursive_compact_v1` when the complete
ABI-7 compact-token native surface is available, `recursive_spend_v1` when only
the complete ABI-6-or-later native surface is available, and otherwise `null`.
Zero-argument selection probes the native bridge; explicit capability selection
must pass both `recursiveCompactAvailable` and `recursiveSpendAvailable`.
The ABI-7 `recursive_compact_v1` compact-token symbols remain source-stable and
probe `kagemusha-recursive-compact-v1` separately from ABI 6 recursive spend.
Use `BuildPallasOpenEnvelopesArchive(...)` for the current-hop record bundle
and `BuildPreviousProofOpenEnvelopesArchive(...)` for the previous recursive
proof bundle; gate both builders with
`IsPallasOpenEnvelopeBuilderAvailable()`. The returned opaque Pallas opening
archives are native-owned Norito bytes and should be passed through unchanged.
Use `EncodeInitRequest(...)` or `EncodeInitRequestWithLineageMaterials(...)`
to build compact `KagemushaRecursiveSpendInitRequestV1` archives from a
validated record bundle, current-hop Pallas opening archive, spendable note
descriptor, and one-hop lineage key artifacts. Use `EncodeAppendRequest(...)`
or `EncodeAppendRequestWithLineageMaterials(...)` to build compact
`KagemushaRecursiveSpendAppendRequestV1` archives from the previous bundle,
new hop record bundle, current-hop Pallas opening archive, spendable note
descriptor, selected output circuit, previous-lineage verifier record, previous
proof opening archive, and append lineage key artifacts. These request encoders
validate the record-bundle hop count, Pallas envelope count/shape, previous
lineage verifier-record selection, previous-proof opening selection, and
lineage key artifact profile before any native bridge dispatch.
The append output circuit selector is required: pass
`RecursiveAggregationProofCircuitIdV1` for the semantic output path or
`RecursiveSpendLineageAppendProofCircuitIdV1` for the Reserved-lineage append
output path. Missing, empty, and unsupported selectors are rejected before
native dispatch.
Use `EncodeInitRequestWithGeneratedPallas(...)` or
`EncodeAppendRequestWithGeneratedPallas(...)` when the SDK should derive the
current-hop and, when required, previous-proof Pallas opening archives. These
helpers still validate typed or raw lineage key material and previous-lineage
record selection before invoking the Pallas builders, so builder availability
or malformed record bundles cannot mask lineage-admission errors.
The raw recursive init/append wrappers and record-backed bridge helpers decode
`KagemushaVerifiedFoldRecordBundle` enough to reject over-limit `steps` count
prefixes before loading the native bridge. Raw init/append request wrappers
and the direct record-backed recursive aggregation/compact prover helpers also
reject Pallas open-envelope archives whose top-level native vector schema or
envelope count does not match the folded record-bundle hop count. The same C#
managed preflight decodes each supplied Pallas `OpenVerifyEnvelope` far enough
to reject malformed IPA params/public/proof counts, missing or malformed
`vk_commitment` / `public_inputs_schema_hash` / `domain_tag` metadata options,
empty or over-limit transcript labels, and trailing envelope bytes before
native dispatch.
Use `ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(...)`
with record-bundle, Pallas open-envelope, and recursive compact key-artifact
archives, and `VerifyRecursiveCompactPaymentToken(...)` with compact-token and
recursive compact verifier-key archives; gate them with
`IsRecursiveCompactPaymentTokenProverAvailable()` and
`IsRecursiveCompactPaymentTokenVerifierAvailable()`. The recursive-spend
compact projection verifier is exposed separately as
`VerifyRecursiveSpendCompactPaymentTokenProjection(...)`; gate it with
`IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable()`. It accepts
raw Norito compact-token and verifier-record archives, rejects empty,
malformed, oversized, or invalid-height inputs before P/Invoke dispatch, and
returns the native boolean receiver result. ABI 7 now carries one-hop LEN=4 and
package-backed multi-hop compact-token proof paths when the native bundle
includes packaged compact proving-key archives and matching verifier-slice
material. Production defaults still stay on ABI 6 Reserved-lineage recursive
spend until that artifact set is shipped and signed for release. Empty,
malformed, missing, or oversized local archives fail as `ArgumentException`;
bridge error code `-312` remains classified as reserved ABI-7 state. The
ABI-7 launch boundary remains explicit: the one-hop LEN=4 compact-token proof
path uses a packaged compact one-hop proving-key, while release evidence
continues to track the proof-composition reservation, generic compact-token
reservation, and multi-hop verifier-batch reservation. Missing native symbols
still surface as `InvalidOperationException`. For
Reserved-lineage branching, use `CanRedeemWitnessless(...)`,
`RequiresLineageWitnessForRedeem(...)`, `PreferredAppendOutputCircuitId(...)`,
`CanProveAppendOutputCircuitId(...)`, and
`CanSelectAppendOutputCircuitId(...)` instead of duplicating circuit-id rules in
app code. `ValidateRedeemLineagePreflight(...)` or the metadata-bound
`Redeem(...)` overload rejects missing semantic lineage witnesses and missing
Reserved-lineage verifier records before native dispatch. Redeem request
archives may carry additional `lineage_verifier_records`; use the overloads
with `lineageVerifierRecordsCount` and
`lineageWitnessHasReservedPreviousProofs` when a record-backed lineage witness
spans multiple Reserved-lineage proof profiles. The single-record
`lineage_verifier_record` field is the first-release scalar record slot, not a
an alternate decode route.
`IsSupportedPreviousProofCircuitId(...)` and
`RequiresPreviousLineageVerifierRecordForAppend(...)` tell app code when to
reject an unknown previous proof circuit and when to include
`previous_lineage_verifier_record`. `RecursiveSpendLineageWitnesslessMaxHopsV1`
is `64`, but it is only the protocol bound.
`RecursiveSpendLineageTransitionCircuitWiredV1` is `false`, so witnessless
Reserved-lineage redeem and append fail closed for every circuit and hop count,
and redeem requires a record-backed lineage witness.
`CanAppendWitnesslessLineage(...)` returns `false` for every input, and
`PreferredAppendOutputCircuitId(...)` selects semantic recursive aggregation
while transition verification is unavailable.
`previous_recursive_proof_open_envelopes_archive` is opaque native prover
material: C# wallet code must pass it through Norito unchanged and must not
construct, rewrite, or mutate it. The native bridge validates `vk_commitment`,
`public_inputs_schema_hash`, and `domain_tag` against the exact previous bundle
before proving or returning output bytes.
Native append streams the previous recursive proof bytes and per-hop accumulator
material into native-owned accumulator digests (`recursive_proof_chain_digest`,
lineage/aggregation transcript, fixed-window schedule/shared-manifest/table-base,
verifier-witness batch, transition-profile, append-opening-preflight,
append-boundary, scalar-projection, and previous/resulting accumulator digests);
SDK code must not derive, supply, or patch accumulator state.
generic proof-state (`proofState`, `ProofState`, `proof_state`),
recursive/lineage proof-state, aggregation-transcript, fixed-window
table-schedule/shared-manifest/table-base, verifier-witness batch,
transition-profile binding, append-opening preflight, recursive verifier
scalar-projection, and previous/resulting accumulator aliases are native-owned
material, not C# request fields.
Verify request archives must pass the same public-binding preflight before the
native bridge returns a recursive spend verify result: Reserved-lineage bundles
require a matching active `lineage_verifier_record`, semantic bundles must omit
it, and unsupported proof attachments are rejected as malformed requests rather
than soft invalid proof results. Redeem request archives keep the same
single-record path and also carry the trailing defaulted
`lineage_verifier_records` vector for additional Reserved-lineage verifier
records; C# metadata-bound `Redeem(...)` overloads accept either source through
the plural record-count preflight while the single-record boolean overloads
are part of the first-release API.
Init requests may omit both packaged lineage key artifacts to select the
semantic recursive aggregation path. That bundle is valid for offline
acceptance and re-spending, while online redemption must carry the
record-backed lineage witness. Supplying exactly one of `lineage_verifier_key`
or `lineage_proving_key_archive` is rejected. Reserved-lineage append-output
requests must still include the append lineage key artifacts in the raw Norito
request. Use `LineageKeyArtifactsForInit(...)` and
`LineageKeyArtifactsForAppend(...)` to package and validate these
verifier/proving key artifacts before building a witnessless Reserved-lineage
request once transition verification is wired. Semantic append is bounded by
the separate `CompactTokenMaxHops` constant; the witnessless max-hop constant
does not enable witnessless admission.
Reserved-lineage append output is valid only when the previous bundle is
already Reserved-lineage; semantic previous bundles keep using semantic append
plus a record-backed lineage witness.

## Native Privacy Bridge

`Hyperledger.Iroha.Privacy.PrivacyNative` exposes the privacy FFI surface as
generic raw Norito archives: `CapabilitiesV1()`, `BuildProofV1(requestArchive)`,
and `VerifyProofV1(requestArchive)`. Approved typed proof-builder aliases for
admitted production privacy entrypoints, including
`BuildZkAceAuthorizationProofV1(requestArchive)`, dispatch through the same
production archive paths and remain fail-closed while the privacy rows are
gated. Planned catalog entrypoints stay unexported until their production gates
pass. Native availability requires ABI 7 or later, the privacy
capability/build/verify symbols, and successful Norito probe outputs whose
operation-specific result schema bytes match the called entry point.

All privacy request and response payloads must stay as raw Norito archives. C#
validates archive magic, length, CRC, the 64 MiB native size cap, and the
operation-specific result schema before returning bytes to callers, and clears
bounded native output buffers before releasing them on both success and bridge
error paths; managed temporary copies of invalid native output are cleared
before validation errors are rethrown. Managed temporary copies of proof
request selector text, public inputs, witnesses, proofs, request archives, and
probe archives are zeroed after native dispatch or sanitized native-call
failure. Proof request selector text
(`algorithmId`, `entrypoint`, and `vkRef`) must be exact, non-empty, free of
whitespace/control characters, and at most 1024 UTF-8 bytes before native
dispatch. Capability
metadata reports `privacy-production-gate-v1`, keeps `ProductionReady = false`,
and remains fail-closed with missing production gates and no audit references
until real proving, verification, chain admission, witness privacy checks,
deterministic testing, negative/adversarial testing, replay/nullifier rejection
testing, parser/verifier fuzzing, performance gates, and external audit signoff
are complete.

`PrivacyProofRequestV1(...)` performs managed preflight before loading the
native bridge: `publicInputs` must be non-empty, `witness` and `proof` are
capped at 33,554,432 bytes, and diagnostics name the offending component (for
example, `proof must not exceed 33554432 bytes`).

C# also exposes the deterministic privacy FFI status/error-code contract for
diagnostics and cross-language parity: `StatusError`, `ErrorNullPointer`,
`ErrorMalformedNorito`, `ErrorUnsupportedAlgorithm`,
`ErrorProductionDisabled`, and `ErrorInvalidRequest`. The stable wire values
are `status_error = 1`, `null_pointer = 1`, `malformed_norito = 2`,
`unsupported_algorithm = 3`, `production_disabled = 4`, and
`invalid_request = 5`; treat them as sanitized status metadata, not proof success.

## Live Testnet Smoke

The integration project is opt-in and can target the public Taira testnet:

```bash
export PATH="$HOME/.dotnet:$PATH"
export IROHA_CSHARP_RUN_LIVE_TESTS=1
export IROHA_CSHARP_TORII_BASE_URL=https://taira.sora.org
cd csharp
dotnet test tests/Hyperledger.Iroha.Sdk.IntegrationTests/Hyperledger.Iroha.Sdk.IntegrationTests.csproj -c Release
```

The live smoke currently probes unauthenticated read endpoints:

- `/v1/node/capabilities`
- `/v1/runtime/abi/active`
- `/v1/accounts`
- `/v1/explorer/accounts/{account_id}/qr`
- `/v1/explorer/accounts`, `/v1/explorer/domains`, `/v1/explorer/asset-definitions`, `/v1/explorer/assets`, `/v1/explorer/nfts`, and `/v1/explorer/rwas` with first-item detail reads when present
- `/v1/contracts/instances/{ns}` when the deployment exposes contract metadata, using `universal` by default, with `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view` when a code hash is available from that namespace or the override env var below
- `/v1/identifier-policies`
- `/v1/vpn/profile`
- `/v1/sorafs/denylist/catalog` when the deployment exposes the denylist surface
- `/v1/sorafs/denylist/packs/{pack_id}` when the catalog is available and non-empty
- `/v1/aliases/by-account`
- `/v1/accounts/faucet/puzzle`
- `/v1/space-directory/uaids/{uaid}`
- `/v1/space-directory/uaids/{uaid}/manifests`

Optional live-smoke environment variables:

- `IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE` to override the default contract-instance namespace (`universal`)
- `IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH` to override the code hash used for `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view`
- `IROHA_CSHARP_SMOKE_SORAFS_CID` and optional `IROHA_CSHARP_SMOKE_SORAFS_PATH` to also probe `/v1/sorafs/cid/{cid}` plus `/sorafs/cid/{cid}/...`
- `IROHA_CSHARP_CANONICAL_ACCOUNT_ID` plus `IROHA_CSHARP_PRIVATE_KEY_SEED_HEX` to also create a signed VPN quote and verify that Torii returns an `OpenVpnLeaseEscrow` instruction skeleton

## Sample

```csharp
using Hyperledger.Iroha;
using Hyperledger.Iroha.Torii;

using var client = new IrohaClient(new Uri("https://taira.sora.org"));
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();

    Console.WriteLine($"ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"First page size: {accounts.Items.Count}");
    Console.WriteLine($"Alias count for first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty: {faucetPuzzle.DifficultyBits}");

    // Offline transaction building is available through client.Ledger.
    // var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
    // if (!string.IsNullOrWhiteSpace(seedHex))
    // {
    //     var signed = client.Ledger
    //         .BuildTransaction("00000042", accounts.Items[0].Id)
    //         .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "1.0000", accounts.Items[0].Id)
    //         .SetCreationTime(DateTimeOffset.UtcNow)
    //         .SetTimeToLiveMilliseconds(5_000)
    //         .SetNonce(1)
    //         .BuildSigned(Convert.FromHexString(seedHex));
    //
    //     Console.WriteLine($"Signed tx hash: {signed.TransactionHashHex}");
    //     // await client.Ledger.SubmitAsync(signed);
    //     // var status = await client.Ledger.WaitForAsync(signed.TransactionHashHex);
    // }
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
- `ToriiClient.SubmitSignedQueryAsync(...)`
- `ToriiClient.OpenEventSseAsync(...)`
- `ToriiClient.StreamEventsAsync(...)`
- `ToriiClient.StreamPipelineEventsAsync(...)`
- `ToriiClient.StreamProofEventsAsync(...)`
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

The managed transaction encoder is deterministic and now covers the current asset quantity plus domain, asset, account, and asset-definition metadata slice. The Torii client can also parse generic SSE frames, project the common pipeline, proof, and explorer block/transaction/instruction streams into typed models, read the core explorer JSON endpoints including latest/health/metrics snapshots with typed DTOs, and build/sign the current singular set plus the first fast_dsl iterable-query subset, but broader iterable families, richer instruction families beyond that slice, broader typed event families, and the broader parity surfaces are still open work.

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

```bash
export DOTNET_ROOT="$HOME/.dotnet"
export PATH="$DOTNET_ROOT:$PATH"
cd csharp
dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj -c Release --no-build --output artifacts/packages
cd ..
ci/check_csharp_sdk_package_consumer.sh
```

The package-consumer guard creates an isolated temporary `net8.0` application,
installs `Hyperledger.Iroha.Sdk` from `csharp/artifacts/packages`, verifies the
consumer project uses `PackageReference` rather than `ProjectReference`, builds
with warnings as errors, and runs managed Ed25519, canonical request, and SCCP
route checks through the packed NuGet assembly.
