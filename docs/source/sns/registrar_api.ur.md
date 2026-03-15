---
lang: ur
direction: rtl
source: docs/source/sns/registrar_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c26544d4d69e452110dc79e1a997dac1b257d046dc37433a8b8cbc3ef6078bec
source_last_modified: "2026-01-28T17:58:57.295476+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Sora Name Service Registrar API & Governance Hooks
summary: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
---

# SNS Registrar API & Governance Hooks (SN-2b)

**Status:** Drafted 2026-03-24 -- under Nexus Core review  
**Roadmap link:** SN-2b “Registrar API & governance hooks”  
**Prerequisites:** Schema definitions in `docs/source/sns/registry_schema.md`

This note specifies the Torii endpoints, gRPC services, request/response DTOs, and governance artifacts required to operate the Sora Name Service (SNS) registrar. It is the authoritative contract for SDKs, wallets, and automation that need to register, renew, or manage SNS names.

## 1. Transport & Authentication

| Requirement | Detail |
|-------------|--------|
| Protocols | REST under `/v2/sns/*` and gRPC service `sns.v1.Registrar`. Both accept Norito-JSON (`application/json`) and Norito-RPC binary (`application/x-norito`). |
| Auth | `Authorization: Bearer` tokens or mTLS certificates issued per suffix steward. Governance-sensitive endpoints (freeze/unfreeze, reserved assignments) require `scope=sns.admin`. |
| Rate limits | Registrars share the `torii.preauth_scheme_limits` buckets with JSON callers plus per-suffix burst caps: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetry | Torii exposes `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` for the registrar handlers (filter on `scheme="norito_rpc"`); the API also increments `sns_registrar_status_total{result, suffix_id}`. |

## 2. DTO Overview

Fields reference the canonical structs defined in `registry_schema.md`. All payloads embed `NameSelectorV1` + `SuffixId` to avoid ambiguous routing.

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

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST Endpoints

| Endpoint | Method | Payload | Description |
|----------|--------|---------|-------------|
| `/v2/sns/registrations` | POST | `RegisterNameRequestV1` | Register or reopen a name. Resolves pricing tier, validates payment/governance proofs, emits registry events. |
| `/v2/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | Extend term. Enforces grace/redemption windows from policy. |
| `/v2/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | Transfer ownership once governance approvals attach. |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Replace controller set; validates signed account addresses. |
| `/v2/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | Guardian/council freeze. Requires guardian ticket and reference to governance docket. |
| `/v2/sns/registrations/{selector}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze after remediation; ensures council override recorded. |
| `/v2/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Steward/council assignment of reserved names. |
| `/v2/sns/policies/{suffix_id}` | GET | — | Fetch current `SuffixPolicyV1` (cacheable). |
| `/v2/sns/registrations/{selector}` | GET | — | Returns current `NameRecordV1` + effective state (Active, Grace, etc.). |

**Selector encoding:** the `{selector}` path segment accepts I105, I105, or canonical hex per ADDR-5; Torii normalises it via `NameSelectorV1`.

**Error model:** all endpoints return Norito JSON with `code`, `message`, `details`. Codes include `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI helpers (N0 manual registrar requirement)

Closed-beta operations now have a thin CLI wrapper so registrars can exercise the REST routes without manually crafting JSON:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` defaults to the CLI config account; repeat `--controller` to add additional controller accounts (defaults to `[owner]`).
- `--payment-json` accepts an entire `PaymentProofV1` blob when registrars already have a structured receipt; otherwise the inline flags map 1:1 to the DTO fields.
- `--metadata-json` (object) and `--governance-json` (full `GovernanceHookV1`) make it easy to attach TXT records or council artefacts during rehearsals.

Read-only helpers round out the workflow:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

The implementation lives in `crates/iroha_cli/src/commands/sns.rs` and reuses the same Norito DTOs described in this document, so the CLI output remains byte-for-byte with the Torii responses.

Additional helpers cover the rest of the registrar lifecycle:

```bash
# Renew an expiring name (selectors map to the REST path)
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership (requires a GovernanceHookV1 JSON file)
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows (guardian ticket and governance hook respectively)
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` must point to a `GovernanceHookV1` JSON document (see the DTO table above). The CLI forwards those payloads to `/v2/sns/registrations/{selector}/{renew,transfer,freeze}` so operators can rehearse the same flows the SDKs will exercise.

## 4. gRPC Service

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

Wire-format: compile-time Norito schema hash recorded under
`fixtures/norito_rpc/schema_hashes.json` (rows `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Governance Hooks & Evidence

Every mutating call must attach evidence suitable for replay:

| Action | Required governance data |
|--------|-------------------------|
| Standard register/renew | Payment proof referencing a settlement instruction; no council vote needed unless tier requires steward approval. |
| Premium tier register / reserved assignment | `GovernanceHookV1` referencing proposal id + steward acknowledgement. |
| Transfer | Council vote hash + DAO signal hash; guardian clearance when transfer triggered by dispute resolution. |
| Freeze/Unfreeze | Guardian ticket signature plus council override (unfreeze). |

Torii verifies proofs by checking:

1. Proposal id exists in governance ledger (`/v2/governance/proposals/{id}`) and status is `Approved`.
2. Hashes match the recorded vote artifacts.
3. Steward/guardian signatures reference the expected public keys from `SuffixPolicyV1`.

Failed checks return `sns_err_governance_missing`.

## 6. Workflow Examples

### 6.1 Standard Registration

1. Client queries `/v2/sns/policies/{suffix_id}` to fetch pricing, grace, and available tiers.
2. Client builds `RegisterNameRequestV1`:
   - `selector` derived from the preferred I105 label.
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
3. After remediation, council issues override; operator sends DELETE `/v2/sns/registrations/{selector}/freeze` with `GovernanceHookV1`.
4. Torii validates override, emits `NameUnfrozen`.

## 7. Validation & Error Codes

| Code | Description | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Label is reserved or blocked. | 409 |
| `sns_err_policy_violation` | Term, tier, or controller set violates policy. | 422 |
| `sns_err_payment_mismatch` | Payment proof value or asset mismatch. | 402 |
| `sns_err_governance_missing` | Required governance artifacts absent/invalid. | 403 |
| `sns_err_state_conflict` | Operation not allowed in current lifecycle state. | 409 |

All codes surface via `X-Iroha-Error-Code` and structured Norito JSON/NRPC envelopes.

## 8. Implementation Notes

- Torii stores pending auctions under `NameRecordV1.auction` and rejects direct registration attempts while `PendingAuction`.
- Payment proofs reuse Norito ledger receipts; treasury services provide helper APIs (`/v2/finance/sns/payments`).
- SDKs should wrap these endpoints with strongly typed helpers so wallets can present clear error reasons (`ERR_SNS_RESERVED`, etc.).

## 9. Next Steps

- Wire the Torii handlers to the actual registry contract once SN-3 auctions land.
- Publish SDK-specific guides (Rust/JS/Swift) referencing this API.
- Extend `docs/source/sns_suffix_governance_charter.md` with cross-links to the governance hook evidence fields.
