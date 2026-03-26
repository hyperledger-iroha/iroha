---
title: Sora Name Service Registrar API & Governance Hooks
summary: Torii REST surfaces, Norito DTOs, and governance artifacts for ledger-backed SNS names (SN-2b).
---

# SNS Registrar API & Governance Hooks (SN-2b)

**Status:** Drafted 2026-03-24 -- under Nexus Core review  
**Roadmap link:** SN-2b “Registrar API & governance hooks”  
**Prerequisites:** Schema definitions in `docs/source/sns/registry_schema.md`

This note specifies the Torii endpoints, request/response DTOs, and governance
artifacts required to operate the Sora Name Service (SNS) registrar. It is the
authoritative contract for SDKs, wallets, and automation that need to
register, renew, or manage ledger-backed SNS names.

## 1. Transport & Authentication

| Requirement | Detail |
|-------------|--------|
| Protocols | REST under `/v1/sns/*`. JSON payloads use Norito-JSON (`application/json`). |
| Auth | `Authorization: Bearer` tokens or mTLS certificates issued per namespace steward. Governance-sensitive endpoints (freeze/unfreeze, governed transfers) require `scope=sns.admin`. |
| Rate limits | Registrars share the `torii.preauth_scheme_limits` buckets with JSON callers plus per-suffix burst caps: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetry | Torii exposes `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` for the registrar handlers (filter on `scheme="norito_rpc"`); the API also increments `sns_registrar_status_total{result, suffix_id}`. |

## 2. DTO Overview

Fields reference the canonical structs defined in `registry_schema.md`. All
payloads embed `NameSelectorV1`; the selector's `suffix_id` must use one of the
fixed namespace ids:

- `0x1001` for `account-alias`
- `0x1002` for `domain`
- `0x1003` for `dataspace`

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

```

## 3. REST Endpoints

| Endpoint | Method | Payload | Description |
|----------|--------|---------|-------------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Register or reopen a ledger-backed name. Validates the namespace selector, payment proof, and embedded governance hook. |
| `/v1/sns/names/{namespace}/{literal}` | GET | — | Return the current `NameRecordV1` and effective lifecycle state for the canonical literal. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Extend term. Enforces grace/redemption windows from policy. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | Transfer ownership once governance approvals attach. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | POST | `UpdateControllersRequestV1` | Replace controller set; validates signed account addresses. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Guardian/council freeze. Requires guardian ticket and reference to governance docket. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze after remediation; ensures council override recorded. |
| `/v1/sns/policies/{suffix_id}` | GET | — | Fetch current `SuffixPolicyV1` (cacheable). |

**Path encoding:** `{namespace}` must be one of `account-alias`, `domain`, or
`dataspace`. `{literal}` is the lowercase canonical label for that namespace:
the full alias string for `account-alias`, the domain literal for `domain`, and
the dataspace alias for `dataspace`.

**Error model:** all endpoints return Norito JSON with `code`, `message`, `details`. Codes include `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI helpers (N0 manual registrar requirement)

Closed-beta operations now have a thin CLI wrapper so registrars can exercise the REST routes without manually crafting JSON:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 0x1002 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` defaults to the CLI config account; repeat `--controller` to add additional controller accounts (defaults to `[owner]`).
- `--payment-json` accepts an entire `PaymentProofV1` blob when registrars already have a structured receipt; otherwise the inline flags map 1:1 to the DTO fields.
- `--metadata-json` (object) and `--governance-json` (full `GovernanceHookV1`) make it easy to attach TXT records or council artefacts during rehearsals.

Read-only helpers round out the workflow:

```bash
iroha sns registration --selector makoto.domain
iroha sns policy --suffix-id 0x1002
```

The implementation lives in `crates/iroha_cli/src/commands/sns.rs` and reuses the same Norito DTOs described in this document, so the CLI output remains byte-for-byte with the Torii responses.

Additional helpers cover the rest of the registrar lifecycle:

```bash
# Renew an expiring name (selectors map to the REST path)
iroha sns renew \
  --selector makoto.domain \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership (requires a GovernanceHookV1 JSON file)
iroha sns transfer \
  --selector makoto.domain \
  --new-owner soraカタカナ... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows (guardian ticket and governance hook respectively)
iroha sns freeze \
  --selector makoto.domain \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.domain \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` must point to a `GovernanceHookV1` JSON document (see the
DTO table above). The CLI normalises the legacy `label.suffix` selector into
`/v1/sns/names/domain/{literal}/{renew,transfer,freeze}` so operators can
rehearse the same ledger-backed flows the SDKs exercise.

## 4. Governance Hooks & Evidence

Every mutating call must attach evidence suitable for replay:

| Action | Required governance data |
|--------|-------------------------|
| Standard register/renew | Payment proof referencing a settlement instruction; no council vote needed unless tier requires steward approval. |
| Premium tier register / guarded reopen | `GovernanceHookV1` referencing proposal id + steward acknowledgement. |
| Transfer | Council vote hash + DAO signal hash; guardian clearance when transfer triggered by dispute resolution. |
| Freeze/Unfreeze | Guardian ticket signature plus council override (unfreeze). |

Torii verifies proofs by checking:

1. Proposal id exists in governance ledger (`/v1/governance/proposals/{id}`) and status is `Approved`.
2. Hashes match the recorded vote artifacts.
3. Steward/guardian signatures reference the expected public keys from `SuffixPolicyV1`.

Failed checks return `sns_err_governance_missing`.

## 5. Workflow Examples

### 6.1 Standard Registration

1. Client queries `/v1/sns/policies/{suffix_id}` to fetch pricing, grace, and available tiers.
2. Client builds `RegisterNameRequestV1`:
   - `selector.suffix_id` set to the fixed namespace id.
   - `selector.label` set to the lowercase canonical literal inside that namespace.
   - `term_years` within policy bounds.
   - `payment` referencing the treasury/steward splitter transfer.
3. Torii validates:
   - Label normalisation + reserved list.
   - Term/gross price vs `PriceTierV1`.
   - Payment proof amount >= computed price + fees.
4. On success Torii:
   - Persists `NameRecordV1`.
   - Emits `RegistryEventV1::NameRegistered`.
   - Emits `RevenueAccrualEventV1`.
   - Returns the new record + events.

### 6.2 Renewal During Grace

Grace renewals include the standard request plus penalty detection:

- Torii checks `now` vs `grace_expires_at` and adds surcharge tables from `SuffixPolicyV1`.
- Payment proof must cover surcharge. Failure => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` records the new `expires_at`.

### 6.3 Guardian Freeze & Council Override

1. Guardian submits `FreezeNameRequestV1` with ticket referencing incident id.
2. Torii moves record to `NameStatus::Frozen`, emits `NameFrozen`.
3. After remediation, council issues override; operator sends DELETE `/v1/sns/names/{namespace}/{literal}/freeze` with `GovernanceHookV1`.
4. Torii validates override, emits `NameUnfrozen`.

## 6. Validation & Error Codes

| Code | Description | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Label is reserved or blocked. | 409 |
| `sns_err_policy_violation` | Term, tier, or controller set violates policy. | 422 |
| `sns_err_payment_mismatch` | Payment proof value or asset mismatch. | 402 |
| `sns_err_governance_missing` | Required governance artifacts absent/invalid. | 403 |
| `sns_err_state_conflict` | Operation not allowed in current lifecycle state. | 409 |

All codes surface via `X-Iroha-Error-Code` and structured Norito JSON/NRPC envelopes.

## 7. Implementation Notes

- Torii stores pending auctions under `NameRecordV1.auction` and rejects direct registration attempts while `PendingAuction`.
- Payment proofs reuse Norito ledger receipts; treasury services provide helper APIs (`/v1/finance/sns/payments`).
- SDKs should wrap these endpoints with strongly typed helpers so wallets can present clear error reasons (`ERR_SNS_RESERVED`, etc.).

## 9. Next Steps

- Wire the Torii handlers to the actual registry contract once SN-3 auctions land.
- Publish SDK-specific guides (Rust/JS/Swift) referencing this API.
- Extend `docs/source/sns_suffix_governance_charter.md` with cross-links to the governance hook evidence fields.
