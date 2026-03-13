---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0d5b4309217b9ad3547a93c0f06713a35c76cd8b072f94ff33842265e684d90
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: registrar-api
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# SNS Registrar API اور گورننس hooks (SN-2b)

**حالت:** 2026-03-24 مسودہ - Nexus Core ریویو کے تحت  
**روڈمیپ لنک:** SN-2b "Registrar API & governance hooks"  
**پیشگی شرائط:** اسکیمہ تعریفیں [`registry-schema.md`](./registry-schema.md) میں۔

یہ نوٹ Torii endpoints، gRPC services، request/response DTOs اور governance
artefacts کی وضاحت کرتا ہے جو Sora Name Service (SNS) registrar چلانے کے لئے
درکار ہیں۔ یہ SDKs، wallets اور automation کے لئے مستند معاہدہ ہے جو SNS نام
رجسٹر، renew یا manage کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق

| شرط | تفصیل |
|-----|-------|
| پروٹوکولز | REST `/v2/sns/*` کے تحت اور gRPC service `sns.v1.Registrar`۔ دونوں Norito-JSON (`application/json`) اور Norito-RPC بائنری (`application/x-norito`) قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` tokens یا mTLS certificates ہر suffix steward کی طرف سے جاری۔ governance-sensitive endpoints (freeze/unfreeze, reserved assignments) کے لئے `scope=sns.admin` لازم ہے۔ |
| ریٹ حدود | Registrars `torii.preauth_scheme_limits` buckets JSON callers کے ساتھ شیئر کرتے ہیں اور ہر suffix کے لئے burst caps: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`۔ |
| ٹیلیمیٹری | Torii registrar handlers کے لئے `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ظاہر کرتا ہے (`scheme="norito_rpc"` پر filter)؛ API بھی `sns_registrar_status_total{result, suffix_id}` بڑھاتی ہے۔ |

## 2. DTO خلاصہ

فیلڈز [`registry-schema.md`](./registry-schema.md) میں متعین canonical structs کو refer کرتی ہیں۔ تمام payloads `NameSelectorV1` + `SuffixId` شامل کرتی ہیں تاکہ مبہم routing سے بچا جا سکے۔

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

## 3. REST endpoints

| Endpoint | طریقہ | Payload | تفصیل |
|----------|-------|---------|-------|
| `/v2/sns/registrations` | POST | `RegisterNameRequestV1` | نام رجسٹر یا دوبارہ کھولنا۔ pricing tier حل کرتا ہے، payment/governance proofs کی توثیق کرتا ہے، registry events emit کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | مدت بڑھاتا ہے۔ پالیسی سے grace/redemption windows نافذ کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | حکمرانی approvals لگنے کے بعد ownership منتقل کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | controllers کا سیٹ بدلتا ہے؛ signed account addresses کی توثیق کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | guardian/council freeze۔ guardian ticket اور governance docket کا حوالہ درکار۔ |
| `/v2/sns/registrations/{selector}/freeze` | DELETE | `GovernanceHookV1` | remediation کے بعد unfreeze؛ council override ریکارڈ ہونے کو یقینی بناتا ہے۔ |
| `/v2/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | reserved names کی steward/council کی طرف سے assignment۔ |
| `/v2/sns/policies/{suffix_id}` | GET | -- | `SuffixPolicyV1` موجودہ حاصل کرتا ہے (cacheable)۔ |
| `/v2/sns/registrations/{selector}` | GET | -- | موجودہ `NameRecordV1` + موثر حالت (Active, Grace وغیرہ) واپس کرتا ہے۔ |

**Selector encoding:** `{selector}` path segment I105، I105 یا canonical hex ADDR-5 کے مطابق قبول کرتا ہے؛ Torii `NameSelectorV1` سے normalize کرتا ہے۔

**Error model:** تمام endpoints Norito JSON `code`, `message`, `details` واپس کرتے ہیں۔ Codes میں `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` شامل ہیں۔

### 3.1 CLI helpers (N0 دستی registrar ضرورت)

Closed-beta stewards اب CLI کے ذریعے registrar استعمال کر سکتے ہیں بغیر ہاتھ سے JSON بنانے کے:

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

- `--owner` بطور ڈیفالٹ CLI config account؛ اضافی controller accounts کے لئے `--controller` کو دہرائيں (default `[owner]`).
- Inline payment flags براہ راست `PaymentProofV1` سے map ہوتے ہیں؛ جب structured receipt ہو تو `--payment-json PATH` دیں۔ Metadata (`--metadata-json`) اور governance hooks (`--governance-json`) بھی اسی انداز میں ہیں۔

Read-only helpers rehearsals کو مکمل کرتے ہیں:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Implementation کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں؛ commands اس دستاویز میں بیان کردہ Norito DTOs دوبارہ استعمال کرتے ہیں تاکہ CLI output Torii responses کے ساتھ byte-for-byte میل کھائے۔

Additional helpers renewals, transfers اور guardian actions کو کور کرتے ہیں:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (proposal id، vote hashes، steward/guardian signatures)۔ ہر کمانڈ متعلقہ `/v2/sns/registrations/{selector}/...` endpoint کی عکاسی کرتی ہے تاکہ beta operators بالکل وہی Torii surfaces rehearse کر سکیں جو SDKs کال کریں گے۔

## 4. gRPC service

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

Wire-format: compile-time Norito schema hash
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (rows `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, وغیرہ).

## 5. Governance hooks اور evidence

ہر mutating call کو replay کے لئے موزوں evidence منسلک کرنا ہوتا ہے:

| Action | Required governance data |
|--------|--------------------------|
| Standard register/renew | Settlement instruction کو refer کرنے والی payment proof؛ council vote کی ضرورت نہیں جب تک tier کو steward approval نہ چاہئے۔ |
| Premium tier register / reserved assignment | `GovernanceHookV1` جو proposal id + steward acknowledgement refer کرتا ہے۔ |
| Transfer | Council vote hash + DAO signal hash؛ guardian clearance جب transfer dispute resolution سے trigger ہو۔ |
| Freeze/Unfreeze | Guardian ticket signature کے ساتھ council override (unfreeze)۔ |

Torii proofs کو جانچتے ہوئے چیک کرتا ہے:

1. proposal id governance ledger (`/v2/governance/proposals/{id}`) میں موجود ہے اور status `Approved` ہے۔
2. hashes ریکارڈ شدہ vote artefacts سے match کرتے ہیں۔
3. steward/guardian signatures `SuffixPolicyV1` سے متوقع public keys کو refer کرتے ہیں۔

Failed checks `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. Workflow examples

### 6.1 Standard Registration

1. Client `/v2/sns/policies/{suffix_id}` کو query کرتا ہے تاکہ pricing، grace اور دستیاب tiers حاصل کرے۔
2. Client `RegisterNameRequestV1` بناتا ہے:
   - `selector` ترجیحی I105 یا second‑best I105 label سے derived ہے۔
   - `term_years` پالیسی حدود میں۔
   - `payment` treasury/steward splitter transfer کو refer کرتا ہے۔
3. Torii validate کرتا ہے:
   - Label normalization + reserved list۔
   - Term/gross price vs `PriceTierV1`.
   - Payment proof amount >= computed price + fees۔
4. کامیابی پر Torii:
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` emit کرتا ہے۔
   - `RevenueAccrualEventV1` emit کرتا ہے۔
   - نیا record + events واپس کرتا ہے۔

### 6.2 Renewal During Grace

Grace renewals میں standard request کے ساتھ penalty detection شامل ہے:

- Torii `now` vs `grace_expires_at` چیک کرتا ہے اور `SuffixPolicyV1` سے surcharge tables شامل کرتا ہے۔
- Payment proof کو surcharge cover کرنا ہوگا۔ Failure => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` نیا `expires_at` ریکارڈ کرتا ہے۔

### 6.3 Guardian Freeze & Council Override

1. Guardian `FreezeNameRequestV1` submit کرتا ہے جس میں incident id کا حوالہ دینے والا ticket ہوتا ہے۔
2. Torii record کو `NameStatus::Frozen` میں منتقل کرتا ہے، `NameFrozen` emit کرتا ہے۔
3. Remediation کے بعد council override جاری کرتا ہے؛ operator DELETE `/v2/sns/registrations/{selector}/freeze` کو `GovernanceHookV1` کے ساتھ بھیجتا ہے۔
4. Torii override validate کرتا ہے، `NameUnfrozen` emit کرتا ہے۔

## 7. Validation & Error Codes

| Code | Description | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Label reserved یا blocked ہے۔ | 409 |
| `sns_err_policy_violation` | Term, tier یا controllers کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | Payment proof میں value یا asset mismatch۔ | 402 |
| `sns_err_governance_missing` | Required governance artefacts غائب/invalid ہیں۔ | 403 |
| `sns_err_state_conflict` | موجودہ lifecycle state میں operation allowed نہیں۔ | 409 |

تمام codes `X-Iroha-Error-Code` اور structured Norito JSON/NRPC envelopes کے ذریعے surface ہوتے ہیں۔

## 8. Implementation Notes

- Torii pending auctions کو `NameRecordV1.auction` میں رکھتا ہے اور `PendingAuction` کے دوران direct registration attempts کو reject کرتا ہے۔
- Payment proofs Norito ledger receipts دوبارہ استعمال کرتے ہیں؛ treasury services helper APIs (`/v2/finance/sns/payments`) فراہم کرتی ہیں۔
- SDKs کو ان endpoints کو strongly typed helpers سے wrap کرنا چاہئے تاکہ wallets واضح error reasons (`ERR_SNS_RESERVED`, وغیرہ) دکھا سکیں۔

## 9. Next Steps

- SN-3 auctions کے بعد Torii handlers کو اصل registry contract سے wire کریں۔
- SDK-specific guides (Rust/JS/Swift) شائع کریں جو اس API کو refer کریں۔
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو governance hook evidence fields کے cross-links کے ساتھ extend کریں۔
