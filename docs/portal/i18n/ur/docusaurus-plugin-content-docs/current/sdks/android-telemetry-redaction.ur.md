---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/android-telemetry-redaction.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f22e82241a07052e62531058ef612cbd99bfe52a0bb0cf53ec6d2e28bd6bf389
source_last_modified: "2025-11-18T05:53:43.294424+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Android ٹیلیمیٹری ریڈیکشن پلان
sidebar_label: Android ٹیلیمیٹری
slug: /sdks/android-telemetry
---

:::note کینونیکل ماخذ
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android ٹیلیمیٹری ریڈیکشن پلان (AND7)

## دائرہ کار

یہ دستاویز Android SDK کے لئے مجوزہ ٹیلیمیٹری ریڈیکشن پالیسی اور enablement artefacts کو ریکارڈ کرتی ہے جیسا کہ roadmap آئٹم **AND7** میں درکار ہے۔ یہ موبائل انسٹرومنٹیشن کو Rust نوڈ baseline کے ساتھ align کرتی ہے جبکہ ڈیوائس کی مخصوص پرائیویسی گارنٹیز کو مدنظر رکھتی ہے۔ یہ آؤٹ پٹ فروری 2026 کی SRE governance review کے لئے pre‑read کے طور پر استعمال ہوتی ہے۔

مقاصد:

- ہر Android‑emitted signal کی فہرست بنانا جو مشترکہ observability backends تک پہنچتا ہے (OpenTelemetry traces، Norito‑encoded logs، metrics exports)۔
- ایسے fields کو درجہ بند کرنا جو Rust baseline سے مختلف ہیں اور redaction یا retention controls کو دستاویز کرنا۔
- enablement اور testing کام کو واضح کرنا تاکہ support teams ریڈیکشن سے متعلق alerts پر deterministic ردعمل دیں۔

## Signal Inventory (Draft)

منصوبہ بندی شدہ instrumentation چینلز کے مطابق گروپ کی گئی ہے۔ تمام field names Android SDK telemetry schema (`org.hyperledger.iroha.android.telemetry.*`) کے مطابق ہیں۔ Optional fields `?` سے نشان زد ہیں۔

| Signal ID | Channel | Key Fields | PII/PHI Classification | Redaction / Retention | Notes |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Trace span | `authority_hash`, `route`, `status_code`, `latency_ms` | Authority public ہے؛ route میں کوئی راز نہیں | Export سے پہلے hashed authority (`blake2b_256`) emit کریں؛ 7 دن retain کریں | Rust `torii.http.request` کو mirror کرتا ہے؛ hashing موبائل alias کی پرائیویسی یقینی بناتا ہے۔ |
| `android.torii.http.retry` | Event | `route`, `retry_count`, `error_code`, `backoff_ms` | کوئی نہیں | کوئی redaction نہیں؛ 30 دن retain کریں | deterministic retry audits کے لئے استعمال؛ Rust fields کے مطابق۔ |
| `android.pending_queue.depth` | Gauge metric | `queue_type`, `depth` | کوئی نہیں | کوئی redaction نہیں؛ 90 دن retain کریں | Rust `pipeline.pending_queue_depth` سے مطابقت رکھتا ہے۔ |
| `android.keystore.attestation.result` | Event | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (derived)، device metadata | Alias کو deterministic label سے بدلیں، brand کو enum bucket میں redact کریں | AND2 attestation readiness کے لئے ضروری؛ Rust nodes device metadata emit نہیں کرتے۔ |
| `android.keystore.attestation.failure` | Counter | `alias_label`, `failure_reason` | Alias redaction کے بعد کوئی نہیں | کوئی redaction نہیں؛ 90 دن retain کریں | Chaos drills کو سپورٹ کرتا ہے؛ `alias_label` hashed alias سے مشتق ہے۔ |
| `android.telemetry.redaction.override` | Event | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Actor role operational PII ہے | Masked role category export کریں؛ audit log کے ساتھ 365 دن retain کریں | Rust میں موجود نہیں؛ operators کو support کے ذریعے overrides فائل کرنے ہوتے ہیں۔ |
| `android.telemetry.export.status` | Counter | `backend`, `status` | کوئی نہیں | کوئی redaction نہیں؛ 30 دن retain کریں | Rust exporter status counters کے ساتھ parity۔ |
| `android.telemetry.redaction.failure` | Counter | `signal_id`, `reason` | کوئی نہیں | کوئی redaction نہیں؛ 30 دن retain کریں | Rust `streaming_privacy_redaction_fail_total` کو mirror کرنے کے لئے ضروری۔ |
| `android.telemetry.device_profile` | Gauge | `profile_id`, `sdk_level`, `hardware_tier` | Device metadata | Coarse buckets (SDK major, hardware tier) emit کریں؛ 30 دن retain کریں | OEM details ظاہر کیے بغیر parity dashboards ممکن بناتا ہے۔ |
| `android.telemetry.network_context` | Event | `network_type`, `roaming` | Carrier ممکنہ PII ہے | `carrier_name` مکمل طور پر حذف کریں؛ باقی fields 7 دن retain کریں | `ClientConfig.networkContextProvider` sanitized snapshot فراہم کرتا ہے تاکہ apps network type + roaming emit کر سکیں بغیر subscriber data ظاہر کیے؛ parity dashboards اس signal کو Rust `peer_host` کے موبائل analog کے طور پر دیکھتے ہیں۔ |
| `android.telemetry.config.reload` | Event | `source`, `result`, `duration_ms` | کوئی نہیں | کوئی redaction نہیں؛ 30 دن retain کریں | Rust config reload spans کو mirror کرتا ہے۔ |
| `android.telemetry.chaos.scenario` | Event | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Device profile bucketized ہے | `device_profile` جیسا؛ 30 دن retain کریں | AND7 readiness کے لئے chaos rehearsals کے دوران لاگ ہوتا ہے۔ |
| `android.telemetry.redaction.salt_version` | Gauge | `salt_epoch`, `rotation_id` | کوئی نہیں | کوئی redaction نہیں؛ 365 دن retain کریں | Blake2b salt rotation کو track کرتا ہے؛ Android hash epoch Rust سے مختلف ہونے پر parity alert۔ |
| `android.crash.report.capture` | Event | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Crash fingerprint + process metadata | Shared redaction salt کے ساتھ `crash_id` hash کریں، watchdog state کو bucket کریں، export سے پہلے stack frames حذف کریں؛ 30 دن retain کریں | `ClientConfig.Builder.enableCrashTelemetryHandler()` پر خودکار طور پر فعال ہوتا ہے؛ device‑identifying traces کے بغیر parity dashboards کو feed کرتا ہے۔ |
| `android.crash.report.upload` | Counter | `crash_id`, `backend`, `status`, `retry_count` | Crash fingerprint | Hashed `crash_id` دوبارہ استعمال کریں، صرف status emit کریں؛ 30 دن retain کریں | `ClientConfig.crashTelemetryReporter()` یا `CrashTelemetryHandler.recordUpload` کے ذریعے emit کریں تاکہ uploads دیگر ٹیلیمیٹری کی طرح Sigstore/OLTP guarantees شیئر کریں۔ |

### Implementation Hooks

- `ClientConfig` اب manifest‑derived telemetry data کو `setTelemetryOptions(...)`/`setTelemetrySink(...)` کے ذریعے thread کرتا ہے، اور `TelemetryObserver` کو خودکار طور پر register کرتا ہے تاکہ hashed authorities اور salt metrics bespoke observers کے بغیر فلو کریں۔ تفصیل کیلئے `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` اور `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/` دیکھیں۔
- Applications `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` کال کر سکتی ہیں تاکہ reflection‑based `AndroidNetworkContextProvider` register ہو، جو runtime میں `ConnectivityManager` کو query کرتا ہے اور `android.telemetry.network_context` event emit کرتا ہے بغیر compile‑time Android dependencies کے۔
- Unit tests `TelemetryOptionsTests` اور `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) hashing helpers اور ClientConfig integration hook کو guard کرتے ہیں تاکہ manifest regressions فوراً سامنے آئیں۔
- Enablement kit/labs اب pseudocode کے بجائے concrete APIs حوالہ دیتے ہیں، اس دستاویز اور runbook کو shipping SDK کے ساتھ ہم آہنگ رکھتے ہیں۔

> **Operations note:** owner/status worksheet `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` میں ہے اور ہر AND7 checkpoint پر اس جدول کے ساتھ اپڈیٹ ہونی چاہیے۔

## Parity allowlists & schema-diff workflow

Governance ایک دوہری allowlist کا تقاضا کرتی ہے تاکہ Android exports ایسے identifiers لیک نہ کریں جو Rust services جان بوجھ کر surface کرتے ہیں۔ یہ سیکشن runbook entry (`docs/source/android_runbook.md` §2.3) کو mirror کرتا ہے مگر AND7 ریڈیکشن پلان کو self‑contained رکھتا ہے۔

| Category | Android exporters | Rust services | Validation hook |
|----------|-------------------|---------------|-----------------|
| Authority / route context | Blake2b-256 کے ذریعے `authority`/`alias` کو hash کریں اور raw Torii hostnames export سے پہلے drop کریں؛ salt rotation ثابت کرنے کیلئے `android.telemetry.redaction.salt_version` emit کریں۔ | Correlation کیلئے مکمل Torii hostnames اور peer IDs emit کریں۔ | `docs/source/sdk/android/readiness/schema_diffs/` کے latest schema diff میں `android.torii.http.request` کو `torii.http.request` سے compare کریں، پھر `scripts/telemetry/check_redaction_status.py` چلائیں تاکہ salt epochs confirm ہوں۔ |
| Device & signer identity | `hardware_tier`/`device_profile` کو bucket کریں، controller aliases hash کریں، اور serial numbers کبھی export نہ کریں۔ | Validator `peer_id`, controller `public_key`, اور queue hashes verbatim emit کریں۔ | `docs/source/sdk/mobile_device_profile_alignment.md` سے align کریں، `java/iroha_android/run_tests.sh` میں alias hashing tests چلائیں، اور labs کے دوران queue‑inspector outputs archive کریں۔ |
| Network metadata | صرف `network_type` + `roaming` export کریں؛ `carrier_name` drop کریں۔ | Peer hostname/TLS metadata برقرار رکھیں۔ | ہر schema diff کو `readiness/schema_diffs/` میں اسٹور کریں اور اگر Grafana کا “Network Context” widget carrier strings دکھائے تو alert کریں۔ |
| Override / chaos evidence | `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` masked actor roles کے ساتھ emit کریں۔ | Unmasked override approvals emit کریں؛ chaos‑specific spans نہیں۔ | Drills کے بعد `docs/source/sdk/android/readiness/and7_operator_enablement.md` cross‑check کریں تاکہ override tokens اور chaos artefacts unmasked Rust events کے ساتھ موجود ہوں۔ |

Workflow:

1. ہر manifest/exporter تبدیلی کے بعد `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` چلائیں اور JSON کو `docs/source/sdk/android/readiness/schema_diffs/` میں رکھیں۔
2. Diff کو اوپر والی table کے خلاف review کریں۔ اگر Android Rust‑only field emit کرے (یا بالعکس) تو AND7 readiness bug فائل کریں اور اس پلان اور runbook کو اپڈیٹ کریں۔
3. ہفتہ وار ops reviews میں `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` چلائیں اور salt epoch + schema‑diff timestamp کو readiness worksheet میں لاگ کریں۔
4. کسی بھی deviation کو `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` میں ریکارڈ کریں تاکہ governance packets parity decisions capture کر سکیں۔

> **Schema reference:** canonical field identifiers `android_telemetry_redaction.proto` سے آتے ہیں (Android SDK build کے دوران Norito descriptors کے ساتھ materialize ہوتے ہیں)۔ Schema میں `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket`, اور `actor_role_masked` fields ظاہر ہیں جو SDK اور telemetry exporters میں استعمال ہوتے ہیں۔

`authority_hash` Torii authority value کا fixed 32‑byte digest ہے۔ `attestation_digest` canonical attestation statement fingerprint کو capture کرتا ہے، جبکہ `device_brand_bucket` raw Android brand string کو approved enum (`generic`, `oem`, `enterprise`) میں map کرتا ہے۔ `actor_role_masked` raw user identifier کے بجائے redaction override actor category (`support`, `sre`, `audit`) لے جاتا ہے۔

### Crash Telemetry Export Alignment

Crash telemetry اب Torii networking signals کے ساتھ وہی OpenTelemetry exporters اور provenance pipeline استعمال کرتا ہے، جس سے duplicate exporters والے governance follow‑up کا اختتام ہوتا ہے۔ Crash handler `android.crash.report.capture` event کو hashed `crash_id` (Blake2b-256، shared redaction salt کے ساتھ جو `android.telemetry.redaction.salt_version` میں track ہوتا ہے)، process‑state buckets اور sanitized ANR watchdog metadata کے ساتھ feed کرتا ہے۔ Stack traces device پر رہتے ہیں اور export سے پہلے صرف `has_native_trace` اور `anr_watchdog_bucket` fields میں summarise ہوتے ہیں، اس طرح کوئی PII یا OEM strings device سے باہر نہیں جاتیں۔

Crash upload `android.crash.report.upload` counter entry بناتا ہے، جس سے SRE backend reliability audit کر سکتا ہے بغیر user یا stack trace جانے۔ چونکہ دونوں signals Torii exporter کو reuse کرتے ہیں، وہ AND7 کیلئے طے شدہ Sigstore signing، retention policy اور alerting hooks inherit کرتے ہیں۔ Support runbooks اس لئے Android اور Rust evidence bundles کے درمیان hashed crash identifier correlate کر سکتے ہیں بغیر bespoke crash pipeline کے۔

Handler کو `ClientConfig.Builder.enableCrashTelemetryHandler()` کے ذریعے enable کریں جب telemetry options اور sinks configure ہوں؛ crash upload bridges `ClientConfig.crashTelemetryReporter()` (یا `CrashTelemetryHandler.recordUpload`) کو reuse کر سکتے ہیں تاکہ backend outcomes اسی signed pipeline میں emit ہوں۔

## Policy Deltas vs Rust Baseline

Android اور Rust telemetry پالیسیوں کے درمیان اختلافات اور mitigation steps۔

| Category | Rust Baseline | Android Policy | Mitigation / Validation |
|----------|---------------|----------------|-------------------------|
| Authority / peer identifiers | Plain authority strings | `authority_hash` (Blake2b-256, rotated salt) | Shared salt `iroha_config.telemetry.redaction_salt` کے ذریعے شائع؛ parity test support کے لئے reversible mapping یقینی بناتا ہے۔ |
| Host / network metadata | Node hostnames/IPs export ہوتے ہیں | صرف network type + roaming | Network health dashboards کو hostnames کی بجائے availability categories پر اپڈیٹ کیا گیا۔ |
| Device characteristics | N/A (server-side) | Bucketed profile (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Chaos rehearsals bucket mapping verify کرتے ہیں؛ support runbook escalation path دستاویز کرتا ہے۔ |
| Redaction overrides | Supported نہیں | Manual override token Norito ledger میں محفوظ (`actor_role_masked`, `reason`) | Overrides کیلئے signed request ضروری؛ audit log ایک سال تک محفوظ۔ |
| Attestation traces | Server attestation صرف SRE کے ذریعے | SDK sanitized attestation summary emit کرتا ہے | Rust attestation validator کے ساتھ hashes cross‑check کریں؛ hashed alias leak سے بچاتا ہے۔ |

Validation checklist:

- ہر signal کیلئے redaction unit tests جو exporter submission سے پہلے hashed/masked fields verify کریں۔
- Schema diff tool (Rust nodes کے ساتھ مشترکہ) nightly چلائیں تاکہ field parity confirm ہو۔
- Chaos rehearsal script override workflow exercise کرے اور audit logging confirm کرے۔

## Implementation Tasks (Pre-SRE Governance)

1. **Inventory Confirmation** — اوپر والی table کو حقیقی Android SDK instrumentation hooks اور Norito schema definitions کے ساتھ cross‑verify کریں۔ Owners: Android Observability TL, LLM۔
2. **Telemetry Schema Diff** — مشترکہ diff tool کو Rust metrics کے خلاف چلائیں تاکہ SRE review کیلئے parity artefacts بنیں۔ Owner: SRE privacy lead۔
3. **Runbook Draft (Completed 2026-02-03)** — `docs/source/android_runbook.md` اب end‑to‑end override workflow (Section 3) اور expanded escalation matrix plus role responsibilities (Section 3.1) دستاویز کرتا ہے، اور CLI helpers، incident evidence، اور chaos scripts کو governance policy سے جوڑتا ہے۔ Owners: LLM مع Docs/Support ایڈیٹنگ۔
4. **Enablement Content** — فروری 2026 سیشن کیلئے briefing slides، lab instructions، اور knowledge‑check سوالات تیار کریں۔ Owners: Docs/Support Manager, SRE enablement team۔

## Enablement Workflow & Runbook Hooks

### 1. Local + CI smoke coverage

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` Torii sandbox چلاتا ہے، canonical multi‑source SoraFS fixture replay کرتا ہے ( `ci/check_sorafs_orchestrator_adoption.sh` کے حوالے سے )، اور synthetic Android telemetry seed کرتا ہے۔
  - Traffic generation `scripts/telemetry/generate_android_load.py` سے ہوتی ہے جو `artifacts/android/telemetry/load-generator.log` میں request/response transcript ریکارڈ کرتی ہے اور headers، path overrides، یا dry‑run mode کو honor کرتی ہے۔
  - Helper SoraFS scoreboard/summaries کو `${WORKDIR}/sorafs/` میں کاپی کرتا ہے تاکہ AND7 rehearsals موبائل clients پر جانے سے پہلے multi‑source parity ثابت کریں۔
- CI بھی وہی tooling reuse کرتا ہے: `ci/check_android_dashboard_parity.sh`، `scripts/telemetry/compare_dashboards.py` کو `dashboards/grafana/android_telemetry_overview.json`، Rust reference dashboard، اور allowlist فائل `dashboards/data/android_rust_dashboard_allowances.json` کے خلاف چلاتا ہے، اور signed diff snapshot `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json` emit کرتا ہے۔
- Chaos rehearsals `docs/source/sdk/android/telemetry_chaos_checklist.md` کے مطابق ہوتی ہیں؛ sample‑env script اور dashboard parity check مل کر “ready” evidence bundle بناتے ہیں جو AND7 burn‑in audit کو feed کرتا ہے۔

### 2. Override issuance and audit trail

- `scripts/android_override_tool.py` overrides جاری اور revoke کرنے کے لئے canonical CLI ہے۔ `apply` signed request ingest کرتا ہے، manifest bundle (`telemetry_redaction_override.to` بطور default) emit کرتا ہے، اور `docs/source/sdk/android/telemetry_override_log.md` میں hashed token row append کرتا ہے۔ `revoke` اسی row میں revocation timestamp لگاتا ہے، اور `digest` governance کیلئے sanitised JSON snapshot لکھتا ہے۔
- CLI audit log میں تبدیلی سے انکار کرتی ہے جب تک Markdown table header موجود نہ ہو، جو `docs/source/android_support_playbook.md` کے compliance requirement سے match کرتا ہے۔ Unit coverage `scripts/tests/test_android_override_tool_cli.py` میں table parser، manifest emitters، اور error handling کو protect کرتی ہے۔
- Operators generated manifest، updated log excerpt **اور** digest JSON کو `docs/source/sdk/android/readiness/override_logs/` میں attach کرتے ہیں جب بھی override استعمال ہو؛ log 365 دن کی تاریخ رکھتا ہے۔

### 3. Evidence capture & retention

- ہر rehearsal یا incident `artifacts/android/telemetry/` کے تحت ایک structured bundle بناتا ہے جس میں شامل ہے:
  - `generate_android_load.py` سے load‑generator transcript اور aggregate counters۔
  - Dashboard parity diff (`android_vs_rust-<stamp>.json`) اور allowance hash جو `ci/check_android_dashboard_parity.sh` نے emit کیے۔
  - Override log delta (اگر override دیا گیا)، متعلقہ manifest، اور refreshed digest JSON۔
- SRE burn‑in report ان artefacts کے ساتھ `android_sample_env.sh` سے کاپی شدہ SoraFS scoreboard کو refer کرتا ہے، جو AND7 review کے لئے telemetry hashes → dashboards → override status کی deterministic chain فراہم کرتا ہے۔

## Cross-SDK Device Profile Alignment

Dashboards Android کے `hardware_tier` کو canonical `mobile_profile_class` میں translate کرتے ہیں جو `docs/source/sdk/mobile_device_profile_alignment.md` میں تعریف ہے تاکہ AND7 اور IOS7 telemetry ایک ہی cohorts compare کرے:

- `lab` — `hardware_tier = emulator` کے طور پر emit ہوتا ہے، Swift کے `device_profile_bucket = simulator` سے match کرتا ہے۔
- `consumer` — `hardware_tier = consumer` (SDK major suffix کے ساتھ) کے طور پر emit ہوتا ہے اور Swift کے `iphone_small`/`iphone_large`/`ipad` buckets کے ساتھ گروپ ہوتا ہے۔
- `enterprise` — `hardware_tier = enterprise` کے طور پر emit ہوتا ہے، Swift کے `mac_catalyst` bucket اور مستقبل کے managed/iOS desktop runtimes کے ساتھ align ہوتا ہے۔

کوئی بھی نیا tier dashboards کے استعمال سے پہلے alignment document اور schema diff artefacts میں شامل ہونا چاہیے۔

## Governance & Distribution

- **Pre-read package** — یہ دستاویز اور appendix artefacts (schema diff, runbook diff, readiness deck outline) **2026-02-05** تک SRE governance mailing list میں تقسیم کیے جائیں گے۔
- **Feedback loop** — Governance کے دوران جمع ہونے والے تبصرے `AND7` JIRA epic میں جائیں گے؛ blockers `status.md` اور Android weekly stand‑up notes میں سامنے آئیں گے۔
- **Publishing** — منظوری کے بعد پالیسی summary کو `docs/source/android_support_playbook.md` سے لنک کیا جائے گا اور `docs/source/telemetry.md` میں shared telemetry FAQ میں حوالہ دیا جائے گا۔

## Audit & Compliance Notes

- پالیسی GDPR/CCPA کی تعمیل کرتی ہے کیونکہ موبائل subscriber data export سے پہلے ہٹایا جاتا ہے؛ hashed authority salt ہر سہ ماہی rotate ہوتا ہے اور shared secrets vault میں محفوظ ہوتا ہے۔
- Enablement artefacts اور runbook updates کو compliance registry میں لاگ کیا جاتا ہے۔
- Quarterly reviews تصدیق کرتے ہیں کہ overrides closed‑loop رہیں (کوئی stale access نہیں)۔

## Governance Outcome (2026-02-12)

**2026-02-12** کی SRE governance session نے Android redaction پالیسی کو بغیر تبدیلی کے منظور کیا۔ کلیدی فیصلے (دیکھیں `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Policy acceptance.** Hashed authority، device‑profile bucketing، اور carrier names کے اخراج کی منظوری دی گئی۔ `android.telemetry.redaction.salt_version` کے ذریعے salt rotation tracking اب quarterly audit آئٹم ہے۔
- **Validation plan.** Unit/integration coverage، nightly schema diff runs، اور quarterly chaos rehearsals کی توثیق ہوئی۔ Action: ہر rehearsal کے بعد dashboard parity report شائع کریں۔
- **Override governance.** Norito‑recorded override tokens کو 365‑day retention window کے ساتھ منظور کیا گیا۔ Support engineering ماہانہ operations syncs میں override log digest review کی مالک ہے۔

## Follow-up Status

1. **Device‑profile alignment (due 2026-03-01).** ✅ Completed — `docs/source/sdk/mobile_device_profile_alignment.md` میں shared mapping وضاحت کرتا ہے کہ Android `hardware_tier` values کیسے canonical `mobile_profile_class` میں map ہوتے ہیں جو parity dashboards اور schema diff tooling استعمال کرتے ہیں۔

## Upcoming SRE Governance Brief (Q2 2026)

Roadmap item **AND7** کا تقاضا ہے کہ اگلی SRE governance session کو Android telemetry redaction کا مختصر pre‑read ملے۔ اس حصے کو living brief کے طور پر استعمال کریں اور ہر council meeting سے پہلے اپڈیٹ رکھیں۔

### Prep checklist

1. **Evidence bundle** — latest schema diff، dashboard screenshots، اور override log digest (نیچے matrix دیکھیں) export کریں اور انہیں dated فولڈر میں رکھیں (مثلاً `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) پھر invite بھیجیں۔
2. **Drill summary** — تازہ ترین chaos rehearsal log اور `android.telemetry.redaction.failure` metric snapshot attach کریں؛ Alertmanager annotations کو اسی timestamp سے جوڑیں۔
3. **Override audit** — تصدیق کریں کہ تمام active overrides Norito registry میں درج ہیں اور meeting deck میں summarised ہیں۔ expiry dates اور متعلقہ incident IDs شامل کریں۔
4. **Agenda note** — میٹنگ سے 48 گھنٹے پہلے SRE chair کو brief link کے ساتھ ping کریں، اور مطلوبہ فیصلوں (نئے signals، retention changes، یا override policy updates) کو نمایاں کریں۔

### Evidence matrix

| Artefact | Location | Owner | Notes |
|----------|----------|-------|-------|
| Schema diff vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | میٹنگ سے <72 گھنٹے پہلے تیار ہونا چاہیے۔ |
| Dashboard diff screenshots | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | `sorafs.fetch.*`, `android.telemetry.*` اور Alertmanager snapshots شامل کریں۔ |
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | `scripts/android_override_tool.sh digest` چلائیں (README دیکھیں) اور latest `telemetry_override_log.md` کے خلاف رن کریں؛ tokens sharing سے پہلے hashed رہتے ہیں۔ |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | KPI summary attach کریں (stall count, retry ratio, override usage)۔ |

### Open questions for the council

- اب جب digest automated ہے، کیا override retention window کو 365 دن سے کم کرنا چاہیے؟
- کیا `android.telemetry.device_profile` کو اگلی release میں نئے shared `mobile_profile_class` labels adopt کرنے چاہئیں، یا Swift/JS SDKs کے اسی تبدیلی کے ساتھ آنے تک انتظار کریں؟
- جب Torii Norito-RPC events Android پر آ جائیں تو regional data residency کیلئے اضافی رہنمائی درکار ہوگی؟ (NRPC-3 follow‑up)

### Telemetry Schema Diff Procedure

Schema diff tool کو کم از کم ہر release candidate میں ایک بار (اور جب بھی Android instrumentation بدلے) چلائیں تاکہ SRE council کو dashboard diff کے ساتھ تازہ parity artefacts ملیں:

1. Android اور Rust telemetry schemas export کریں۔ CI کیلئے configs `configs/android_telemetry.json` اور `configs/rust_telemetry.json` میں ہیں۔
2. `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json` چلائیں۔
   - متبادل طور پر commits پاس کریں (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) تاکہ configs git سے براہ راست آئیں؛ اسکرپٹ hashes کو artefact میں pin کرتا ہے۔
3. Generated JSON کو readiness bundle کے ساتھ attach کریں اور `status.md` + `docs/source/telemetry.md` میں link کریں۔ Diff added/removed fields اور retention deltas کو highlight کرتا ہے تاکہ auditors بغیر tool دوبارہ چلائے parity confirm کر سکیں۔
4. جب diff اجازت یافتہ divergence دکھائے (مثلاً Android‑only override signals)، `ci/check_android_dashboard_parity.sh` سے referenced allowlist file اپڈیٹ کریں اور schema‑diff directory README میں rationale نوٹ کریں۔

> **Archive rules:** پانچ تازہ ترین diffs کو `docs/source/sdk/android/readiness/schema_diffs/` کے تحت رکھیں اور پرانے snapshots کو `artifacts/android/telemetry/schema_diffs/` میں منتقل کریں تاکہ governance reviewers ہمیشہ تازہ ترین ڈیٹا دیکھ سکیں۔
