---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ur
direction: rtl
source: docs/amx.md
status: complete
translator: manual
source_hash: 563ec4d7d4d96fa04a7b210a962b9927046985eb01d1de0954575cf817f9f226
source_last_modified: "2025-11-09T19:43:51.262828+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/amx.md (AMX Execution & Operations Guide) کا اردو ترجمہ -->

# AMX ایکزیکیوشن اور آپریشنز گائیڈ

**حالت:** ڈرافٹ (NX‑17)  
**سامعین:** core protocol ٹیم، AMX/consensus انجینئرز، SRE/Telemetry، SDK اور
Torii کی ٹیمیں  
**کانٹیکسٹ:** roadmap آئٹم “Documentation (owner: Docs) — `docs/amx.md` کو
timing diagrams، error catalog، operator expectations، اور PVOs کی
generation/usage کے developer guidance کے ساتھ اپ ڈیٹ کرنا” کو مکمل کرتا ہے۔

## خلاصہ

Atomic cross‑data‑space transactions (AMX) ایک single submission کو اس قابل
بناتی ہیں کہ وہ متعدد data spaces (DS) کو ٹچ کرے، جبکہ 1 s slot finality،
deterministic failure codes، اور private DS fragments کے لیے confidentiality
برقرار رہے۔ یہ گائیڈ timing model، canonical error handling، operator evidence
requirements، اور Proof Verification Objects (PVOs) کے بارے میں developer
expectations کو capture کرتی ہے، تاکہ roadmap deliverable، Nexus design
paper (`docs/source/nexus.md`) سے باہر بھی self‑contained رہے۔

اہم گارنٹیز:

- ہر AMX submission کو prepare/commit کے deterministic budgets ملتے ہیں؛
  بجٹ سے تجاوز ہونے کی صورت میں lanes hang ہونے کے بجائے documented codes کے
  ساتھ abort ہوتا ہے۔
- DA samples جو budget miss کرتے ہیں، ٹرانزیکشن کو اگلے slot پر roll کر دیتے
  ہیں اور خاموشی سے throughput stall کرنے کے بجائے explicit telemetry
  (`missing-availability warning`) emit کرتے ہیں۔
- Proof Verification Objects (PVOs) بھاری proofs کو 1 s slot سے decouple
  کرتے ہیں، اس طرح clients/batchers، artifacts کو pre‑register کر سکتے ہیں
  جنہیں nexus host slot کے اندر تیزی سے verify کرتا ہے۔

## Slot timing model

### ٹائم لائن

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- budgets، global ledger پلان کے مطابق ہیں: mempool 70 ms، DA commit
  ≤300 ms، consensus 300 ms، IVM/AMX 250 ms، settlement 40 ms، guard 40 ms۔
- جو ٹرانزیکشنز DA window breach کرتی ہیں انہیں deterministic انداز میں اگلے
  slot پر reschedule کیا جاتا ہے؛ دیگر breaches، `AMX_TIMEOUT` یا
  `SETTLEMENT_ROUTER_UNAVAILABLE` جیسے codes میں surface ہوتے ہیں۔
- guard slice telemetry export اور final auditing کو absorb کرتی ہے، تاکہ
  exporters میں تھوڑا delay ہونے پر بھی slot 1 s میں close ہو سکے۔

### Cross‑DS swim lane

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

ہر DS fragment کو slot assemble ہونے سے پہلے اپنے 30 ms prepare window کے
اندر complete ہونا پڑتا ہے۔ جو proofs miss ہو جائیں، وہ peers کو block
کرنے کے بجائے اگلے slot کے لیے mempool میں ہی رہتی ہیں۔

### Instrumentation checklist

| میٹرک / ٹریس | سورس | SLO / Alert | نوٹس |
|-------------|-------|-------------|------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | CI gate `ans3.md` میں بیان ہے۔ |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (commit hook) | فی 30 منٹ ونڈو ≥0.95 | DA reschedule telemetry سے derive ہوتی ہے؛ ہر block gauge کو اپ ڈیٹ کرتا ہے۔ |
| `iroha_amx_prepare_ms` | IVM host | فی DS scope p95 ≤ 30 ms | `AMX_TIMEOUT` aborts کو drive کرتا ہے۔ |
| `iroha_amx_commit_ms` | IVM host | فی DS scope p95 ≤ 40 ms | delta merge + trigger execution کور کرتا ہے۔ |
| `iroha_ivm_exec_ms` | IVM host | فی lane >250 ms پر alert | IVM overlay execution window کو mirror کرتا ہے۔ |
| `iroha_amx_abort_total{stage}` | Executor | alert اگر >0.05 aborts/slot یا single stage پر sustainable spikes | `stage`: `prepare`, `exec`, `commit`۔ |
| `iroha_amx_lock_conflicts_total` | AMX scheduler | alert اگر >0.1 conflicts/slot | غلط R/W sets کی نشاندہی کرتا ہے۔ |

## AMX error catalog اور operator expectations

باقاعدہ errors:

- `AMX_TIMEOUT` – کسی fragment نے اپنا prepare/exec/commit budget
  exceed کر دیا۔
- `AMX_LOCK_CONFLICT` – scheduler نے lock conflict detect کیا۔
- `SETTLEMENT_ROUTER_UNAVAILABLE` – settlement router slot کے اندر
  آپریشن route نہیں کر سکا۔
- `PVO_MISSING` – preregistered PVO کی توقع تھی، مگر نہیں ملا۔

Operators کو چاہیے:

- اوپر بیان کردہ metrics پر نظر رکھیں اور Grafana میں AMX‑specific
  dashboards برقرار رکھیں۔
- `AMX_TIMEOUT` یا `AMX_LOCK_CONFLICT` کے spikes کے لیے runbooks رکھیں
  (مثلاً access hints کا جائزہ، budgets کو tune کرنا وغیرہ)۔
- AMX incidents کے لیے evidence (logs، traces، PVOs) کو archive کریں، تاکہ
  انہیں offline دوبارہ بنایا اور analyze کیا جا سکے۔

## Developers کے لیے گائیڈ: PVOs

- Proof Verification Objects heavy proofs (مثلاً private fragments کے لیے
  ZK proofs) کو ایک artifact میں encapsulate کرتے ہیں، جسے nexus host slot
  کے اندر تیزی سے verify کر سکتا ہے۔
- Clients کو چاہیے کہ slot کے critical path سے باہر PVOs preregister کریں،
  اور AMX transactions کے اندر انہیں stable identifiers کے ذریعے reference
  کریں۔
- PVO design کو یہ یقینی بنانا چاہیے:
  - canonical Norito encapsulation (versioned fields، Norito header، اور
    checksum)۔
  - public اور private data کے درمیان واضح separation، تاکہ confidentiality
    قائم رہے۔

مزید design details اور PVO کے exact formats کے لیے
`docs/source/nexus.md` ملاحظہ کریں۔

</div>

