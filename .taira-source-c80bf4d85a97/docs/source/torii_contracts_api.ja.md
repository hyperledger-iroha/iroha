<!-- Japanese translation of docs/source/torii_contracts_api.md -->

---
lang: ja
direction: ltr
source: docs/source/torii_contracts_api.md
status: needs-translation
translator: manual
source_hash: 457eadb1a14964e738f069e958282138e4c5a8af7dee56b2f9c2c0d2ab75a7fd
source_last_modified: "2026-07-11T04:43:34.096342+00:00"
translation_last_reviewed: null
---

> Translation status note (2026-07-12): the canonical English source has changed. This stale English-language mirror is retained only as localization input and must not be treated as synchronized.

# Torii Contracts API (Artifact Reads and Invocation)

Torii exposes registered contract artifacts and invocation endpoints. Contract
deployment is not a server-side HTTP operation: private keys remain with the
client, and the client submits locally signed consensus transactions through
the standard transaction pipeline.

## Endpoints

- GET `/v1/contracts/code/{code_hash}`
  - Fetches the canonical on-chain `ContractManifest` by its content-addressed
    `code_hash`.
- GET `/v1/contracts/code-bytes/{code_hash}`
  - Fetches the registered bytecode as `{ "code_b64": "…" }`.
- POST `/v1/contracts/aliases/resolve`
  - Resolves an active contract alias using canonical account-signed request
    headers and returns the exact consensus binding and contract subject.
- POST `/v1/contracts/call` and POST `/v1/contracts/view`
  - Invoke or read an already registered contract by canonical address or active
    alias.

The retired `/v1/contracts/deploy`, `/v1/contracts/deploy-bundle`, and
`/v1/contracts/deploy-bundles/{bundle_digest}` routes are not part of the
first-release API.

## Locally signed deployment

A deployment client must:

1. Verify the self-describing `.to` artifact locally and retain its exact
   `code_hash`, manifest, and ABI hash.
2. Submit bounded `UploadSmartContractCodeChunk` instructions followed by
   `FinalizeSmartContractCodeUpload`.
3. Sign the verified manifest locally and submit `RegisterSmartContractCode`.
4. Read the authority's exact `contract_deploy_nonce` and the current signed
   alias binding.
5. Derive the canonical address from `(chain_discriminant, authority, nonce,
   dataspace)` and submit one `CommitContractDeployment` containing the
   expected nonce and expected previous alias target.

`CommitContractDeployment` validates the nonce, derived address, registered
artifact, and alias compare-and-swap in one consensus transition. Rotation
clears and deactivates the previous address and binds the new one atomically.
Generic account metadata writes cannot modify the reserved deployment nonce.

### Type encodings (JSON)

- `Hash` values inside a canonical manifest (for example `code_hash` and
  `abi_hash`) use the checksummed Norito JSON literal
  `hash:<64 uppercase hex>#<4 uppercase checksum hex>`. SDK convenience APIs
  may validate that literal and expose the underlying 64-character lowercase
  hex, but must not accept malformed checksums or non-canonical spellings.
- Receipt fields whose names end in `_hex` remain raw 64-character lowercase
  hex by definition; they are not `Hash` JSON values.
- `AccountId` strings use canonical I105 literals (domainless encoded literal).
  Strict parser paths accept only canonical I105 literals (no `@<domain>` suffix).
- `ExposedPrivateKey` accepts either a bare multihash hex string or its algorithm-prefixed variant (e.g., `ed25519:…`). Responses normalise to bare multihash hex. Multihash hex is canonical: varint bytes are lowercase, payload bytes are uppercase, and `0x` prefixes are rejected.

### GET response: ContractCodeRecordDto

```jsonc
{
  "code_hash": "0123…cdef",
  "abi_hash": "89ab…7654",
  "manifest": {
    "seiyaku_name": "Treasury",
    "code_hash": "hash:0123…CDEF#ABCD",
    "abi_hash":  "hash:89AB…7654#1234",
    "compiler_fingerprint": "kotodama_lang/…",
    "features_bitmap": 0,
    "access_set_hints": { "read_keys": [], "write_keys": [], "dynamic_reads": [], "dynamic_writes": [] },
    "entrypoints": [],
    "states": [],
    "error_codes": null,
    "kotoba": null,
    "provenance": null
  }
}
```

The `manifest` value is the complete canonical Norito JSON representation of
`ContractManifest`; Torii does not truncate or rename its fields. Entrypoint
descriptors therefore retain exact argument/return schemas, access metadata,
and trigger declarations. The top-level `code_hash` and `abi_hash` convenience
fields are raw lowercase hex derived from the exact same manifest values; the
optional `code_bytes` field is omitted by this endpoint.

### Norito payloads

All DTOs derive both `JsonSerialize` and `NoritoSerialize`. Clients may submit either plain JSON or Norito-backed JSON. When emitting Norito via Kotodama tests or automation, use `norito::json::json!` with the same field names and encodings shown above so the `NoritoJson<T>` extractor can decode the payload deterministically.

## Examples

Fetch a manifest by hash:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code/<32-byte-hex> | jq .
```

Fetch the registered code bytes:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code-bytes/<32-byte-hex> | jq .
```

Resolve the currently active alias using the canonical account-signature
headers produced by an SDK client:

```bash
iroha contract alias resolve router::universal
```

### Computing `abi_hash` for manifests

Manifests may include an `abi_hash` that binds the program to the node’s IVM ABI policy. You can compute this hash locally using the CLI:

```bash
# ABI v1
iroha tools ivm abi-hash --policy v1 --uppercase
```

The command prints a 32‑byte hex digest. Embed this value in `manifest.abi_hash`. Nodes verify that `abi_hash` equals their runtime policy hash and reject mismatches at admission.

## Security and governance

- Manifest registration, bytecode registration, activation, deactivation, and
  bytecode removal require the signing authority to hold
  `CanRegisterSmartContractCode`. The sole first-release bootstrap exception is
  an absent transaction authority whose transaction begins with the exact
  ordered `Register<Account>(self)`,
  `Grant<CanRegisterSmartContractCode>(self)`, then native upload (or manifest
  registration when matching code is already stored) prefix.
  Both executor paths reject that self-grant for a pre-existing account and
  reject changed destinations, permission payloads, or instruction order.
- `ContractAddress` remains immutable per deployment, while `ContractAlias` is
  the stable public handle. Deployment and rotation use locally signed native
  instructions; governance-controlled namespace binding remains a consensus
  concern, not a server-signing contracts API.
- GET is read-only and content‑addressed by `code_hash`. Nodes may still apply access controls consistent with their governance policies.
