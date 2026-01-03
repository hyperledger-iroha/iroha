<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ur
direction: rtl
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_settlement_faq.md -->

# Nexus سیٹلمنٹ FAQ

**روڈ میپ لنک:** NX-14 - Nexus دستاویزات اور آپریٹر runbooks  
**اسٹیٹس:** مسودہ 2026-03-24 (settlement router اور CBDC playbook specs کے مطابق)  
**سامعین:** آپریٹرز، SDK مصنفین، اور گورننس ریویو کرنے والے جو Nexus (Iroha 3) لانچ کی تیاری کر رہے ہیں۔

یہ FAQ NX-14 ریویو کے دوران سامنے آنے والے سوالات کا جواب دیتا ہے، جن میں settlement routing،
XOR conversion، telemetry، اور audit evidence شامل ہیں۔ مکمل specification کے لئے
`docs/source/settlement_router.md` اور CBDC مخصوص policy knobs کے لئے
`docs/source/cbdc_lane_playbook.md` دیکھیں۔

> **TL;DR:** تمام settlement flows Settlement Router کے ذریعے گزرتے ہیں، جو پبلک lanes پر XOR
> buffers کو debit کرتا ہے اور lane-specific fees لاگو کرتا ہے۔ آپریٹرز کو routing config
> (`config/config.toml`)، telemetry dashboards، اور audit logs کو published manifests کے ساتھ
> sync رکھنا ہوگا۔

## Frequently asked questions

### کون سی lanes settlement ہینڈل کرتی ہیں اور میں کیسے جانوں کہ میرا DS کہاں فٹ ہوتا ہے؟

- ہر dataspace اپنے manifest میں `settlement_handle` declare کرتا ہے۔ Default handles کا mapping:
  - `xor_global` default public lanes کے لئے۔
  - `xor_lane_weighted` public custom lanes کے لئے جو liquidity کہیں اور سے لیتی ہیں۔
  - `xor_hosted_custody` private/CBDC lanes کے لئے (escrowed XOR buffer)۔
  - `xor_dual_fund` hybrid/confidential lanes کے لئے جو shielded + public flows mix کرتی ہیں۔
- lane classes کے لئے `docs/source/nexus_lanes.md` اور تازہ ترین catalog approvals کے لئے
  `docs/source/project_tracker/nexus_config_deltas/*.md` دیکھیں۔ `irohad --sora --config ... --trace-config`
  runtime میں effective catalog پرنٹ کرتا ہے تاکہ audits ہو سکیں۔

### Settlement Router conversion rates کیسے طے کرتا ہے؟

- router ایک واحد راستہ اور deterministic pricing نافذ کرتا ہے:
  - public lanes کے لئے on-chain XOR liquidity pool (public DEX) استعمال ہوتا ہے۔ جب liquidity کم
    ہو تو price oracles گورننس منظور شدہ TWAP پر fallback کرتے ہیں۔
  - private lanes XOR buffers کو پہلے سے fund کرتی ہیں۔ جب settlement debit ہوتا ہے تو router
    conversion tuple `{lane_id, source_token, xor_amount, haircut}` log کرتا ہے اور اگر buffers drift
    کریں تو governance-approved haircuts (`haircut.rs`) لاگو کرتا ہے۔
- configuration `config/config.toml` کے `[settlement]` حصے میں ہے۔ گورننس کی ہدایت کے بغیر custom
  edits سے گریز کریں۔ فیلڈ تفصیل کے لئے `docs/source/settlement_router.md` دیکھیں۔

### فیس اور rebates کیسے لاگو ہوتے ہیں؟

- فیسیں manifest میں per-lane بیان ہوتی ہیں:
  - `base_fee_bps` — ہر settlement debit پر لگتی ہے۔
  - `liquidity_haircut_bps` — shared liquidity providers کی تلافی کرتا ہے۔
  - `rebate_policy` — اختیاری (مثلاً CBDC promotional rebates)۔
- router `SettlementApplied` events (Norito format) emit کرتا ہے جن میں fee breakdowns ہوتے ہیں، تاکہ
  SDKs اور auditors ledger entries کو reconcile کر سکیں۔

### کون سی telemetry ثابت کرتی ہے کہ settlements صحت مند ہیں؟

- Prometheus metrics (`iroha_telemetry` اور settlement router کے ذریعے):
  - `nexus_settlement_latency_seconds{lane_id}` — P99 public lanes کے لئے 900 ms سے کم اور private
    lanes کے لئے 1200 ms سے کم رہنا چاہئے۔
  - `settlement_router_conversion_total{source_token}` — ہر token کے لئے conversion volumes کی تصدیق۔
  - `settlement_router_haircut_total{lane_id}` — اگر governance note کے بغیر non-zero ہو تو alert۔
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` — per-lane XOR debits (micro units) دکھاتا
    ہے؛ <25 %/10 % of Bmin پر alert۔
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` — realised haircut variance کو treasury P&L سے
    reconcile کرنے کے لئے۔
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` — latest block میں لاگو ہونے والا effective
    epsilon؛ auditors router policy سے compare کرتے ہیں۔
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` — sponsor/MM credit line
    usage؛ 80 % سے اوپر alert۔
- `nexus_lane_block_height{lane,dataspace}` — lane/dataspace جوڑے کے لئے latest block height؛ پڑوسی
  peers کو چند slots کے اندر رکھیں۔
- `nexus_lane_finality_lag_slots{lane,dataspace}` — global head اور اس lane کے تازہ ترین block کے
  درمیان slots؛ drills کے باہر >12 پر alert۔
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` — XOR میں settlement backlog؛ CBDC/private
  workloads کو regulator thresholds سے پہلے gate کریں۔

`settlement_router_conversion_total` میں `lane_id`, `dataspace_id`, اور `source_token` labels ہوتے
ہیں تاکہ ہر conversion میں استعمال ہونے والا gas asset ثابت ہو سکے۔ `settlement_router_haircut_total`
XOR units جمع کرتا ہے (raw micro amounts نہیں)، جس سے Treasury براہ راست Prometheus سے haircut ledger
reconcile کر سکتی ہے۔
- `lane_settlement_commitments[*].swap_metadata.volatility_class` دکھاتا ہے کہ router نے `stable`,
  `elevated`, یا `dislocated` margin bucket لاگو کیا۔ elevated/dislocated entries کو incident log یا
  governance note سے جوڑنا ضروری ہے۔
- Dashboards: `dashboards/grafana/nexus_settlement.json` اور `nexus_lanes.json` overview۔ Alerts کو
  `dashboards/alerts/nexus_audit_rules.yml` کے ساتھ tie کریں۔
- settlement telemetry degrade ہونے پر `docs/source/nexus_operations.md` کے runbook کے مطابق incident
  log کریں۔

### regulators کے لئے lane telemetry کیسے export کریں؟

جب بھی regulator lane table طلب کرے تو نیچے والا helper چلائیں:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` وہ JSON blob قبول کرتا ہے جو `iroha status --format json` واپس کرتا ہے۔
* `--json-out` ہر lane کے لئے canonical JSON array بناتا ہے (aliases, dataspace, block height,
  finality lag, TEU capacity/utilization, scheduler trigger + utilization counters, RBC throughput,
  backlog, governance metadata وغیرہ)۔
* `--parquet-out` اسی payload کو Parquet فائل (Arrow schema) کے طور پر لکھتا ہے، جو regulators کے
  لئے columnar evidence مہیا کرتی ہے۔
* `--markdown-out` readable summary بناتا ہے جو lagging lanes، non-zero backlog، missing compliance
  evidence، اور pending manifests کو flag کرتا ہے؛ default `artifacts/nexus_lane_audit.md` ہے۔
* `--lane-compliance` اختیاری ہے؛ اگر دیا جائے تو اسے compliance doc میں بیان کردہ JSON manifest کی
  طرف اشارہ کرنا چاہئے تاکہ exported rows matching lane policy، reviewer signatures، metrics snapshot
  اور audit log excerpts شامل کریں۔

`artifacts/` میں دونوں outputs کو routed-trace evidence کے ساتھ archive کریں ( `nexus_lanes.json`
کے screenshots، Alertmanager state، اور `nexus_lane_rules.yml` )۔

### auditors کیا evidence توقع کرتے ہیں؟

1. **Config snapshot** — `config/config.toml` کے ساتھ `[settlement]` section اور موجودہ manifest سے
   refer ہونے والا lane catalog محفوظ کریں۔
2. **Router logs** — `settlement_router.log` روزانہ archive کریں؛ اس میں hashed settlement IDs، XOR
   debits، اور haircuts کے اطلاق کی جگہیں شامل ہیں۔
3. **Telemetry exports** — اوپر بیان کردہ metrics کا weekly snapshot۔
4. **Reconciliation report** — اختیاری مگر تجویز کردہ: `SettlementRecordV1` entries export کریں
   ( `docs/source/cbdc_lane_playbook.md` دیکھیں ) اور treasury ledger کے ساتھ compare کریں۔

### کیا SDKs کو settlement کے لئے خاص handling کی ضرورت ہے؟

- SDKs کو:
  - settlement events (`/v1/settlement/records`) query کرنے اور `SettlementApplied` logs interpret کرنے
    کے لئے helpers دینا ہوں گے۔
  - client configuration میں lane IDs + settlement handles دکھانا ہوں گے تاکہ operators
    transactions درست طرح route کر سکیں۔
  - `docs/source/settlement_router.md` میں بیان کردہ Norito payloads (مثلاً `SettlementInstructionV1`)
    کو end-to-end tests کے ساتھ mirror کرنا ہوگا۔
- Nexus SDK quickstart (اگلا سیکشن) عوامی نیٹ ورک onboarding کے لئے per-language snippets فراہم کرتا ہے۔

### settlements گورننس یا emergency brakes کے ساتھ کیسے تعامل کرتے ہیں؟

- گورننس manifest updates کے ذریعے مخصوص settlement handles کو pause کر سکتی ہے۔ router `paused`
  flag کو مانتا ہے اور نئے settlements کو deterministic error (`ERR_SETTLEMENT_PAUSED`) کے ساتھ reject
  کرتا ہے۔
- ایمرجنسی "haircut clamps" ہر block کے لئے زیادہ سے زیادہ XOR debits محدود کرتے ہیں تاکہ shared
  buffers drain نہ ہوں۔
- آپریٹرز کو `governance.settlement_pause_total` monitor کرنا چاہئے اور
  `docs/source/nexus_operations.md` میں incident template کے مطابق عمل کرنا چاہئے۔

### bugs کہاں رپورٹ کریں یا changes کی درخواست کریں؟

- Feature gaps -> `NX-14` tagged issue کھولیں اور roadmap کے ساتھ cross-link کریں۔
- Urgent settlement incidents -> Nexus primary کو page کریں ( `docs/source/nexus_operations.md` دیکھیں )
  اور router logs attach کریں۔
- Documentation corrections -> اس فائل اور portal counterparts ( `docs/portal/docs/nexus/overview.md`,
  `docs/portal/docs/nexus/operations.md` ) کے خلاف PRs بنائیں۔

### کیا آپ example settlement flows دکھا سکتے ہیں؟

نیچے دیے گئے snippets عام lane اقسام کے لئے auditor توقعات دکھاتے ہیں۔ ہر scenario کے لئے router log،
ledger hashes، اور matching telemetry export محفوظ کریں تاکہ reviewers evidence replay کر سکیں۔

#### Private CBDC lane (`xor_hosted_custody`)

ذیل میں hosted custody handle استعمال کرنے والی ایک private CBDC lane کے لئے trimmed router log ہے۔
یہ log deterministic XOR debits، fee composition، اور telemetry IDs ثابت کرتا ہے:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

Prometheus میں آپ کو matching metrics نظر آنے چاہئیں:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

log snippet، ledger transaction hash، اور metrics export کو اکٹھا archive کریں تاکہ auditors flow
reconstruct کر سکیں۔ اگلی مثالیں public اور hybrid/confidential lanes کے لئے evidence ریکارڈ کرنے
کا طریقہ دکھاتی ہیں۔

#### Public lane (`xor_global`)

Public data spaces `xor_global` کے ذریعے route ہوتے ہیں، اس لئے router shared DEX buffer سے debit
کرتا ہے اور live TWAP ریکارڈ کرتا ہے جو transfer کی قیمت طے کرتا ہے۔ جب oracle cached value پر
fallback کرے تو TWAP hash یا governance note attach کریں۔

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

Metrics اسی flow کو ثابت کرتی ہیں:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

TWAP record، router log، telemetry snapshot، اور ledger hash کو ایک ہی evidence bundle میں رکھیں۔
جب lane 0 latency یا TWAP freshness پر alert ہو تو incident ticket کو اس bundle سے link کریں۔

#### Hybrid/confidential lane (`xor_dual_fund`)

Hybrid lanes shielded buffers اور public XOR reserves کو mix کرتی ہیں۔ ہر settlement میں دکھانا ہوگا
کہ XOR کس bucket سے آیا اور haircut policy نے فیس کیسے تقسیم کی۔ router log یہ تفصیل dual-fund
metadata block کے ذریعے ظاہر کرتا ہے:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

router log کو dual-fund policy (governance catalog extract)، lane کے `SettlementRecordV1` export، اور
telemetry snippet کے ساتھ archive کریں تاکہ auditors تصدیق کر سکیں کہ shielded/public split نے
governance limits کی پابندی کی۔

اس FAQ کو اپ ٹو ڈیٹ رکھیں جب settlement router کا رویہ بدلے یا گورننس نئی lane classes/fee policies
متعارف کرائے۔

</div>
