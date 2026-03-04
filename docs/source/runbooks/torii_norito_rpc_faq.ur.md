---
lang: ur
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# Norito-RPC آپریٹر FAQ

یہ FAQ **NRPC-2** اور **NRPC-4** میں درج rollout/rollback knobs، ٹیلی میٹری اور
ایویڈنس artefacts کو سمیٹتی ہے تاکہ canary, brownout یا incident drills کے دوران
آپریٹرز کے پاس ایک ہی صفحہ ہو۔ اسے on‑call handoff کے لیے فرنٹ ڈور سمجھیں؛
تفصیلی طریقہ کار `docs/source/torii/norito_rpc_rollout_plan.md` اور
`docs/source/runbooks/torii_norito_rpc_canary.md` میں موجود ہے۔

## 1. Configuration knobs

| راستہ | مقصد | اجازت شدہ ویلیوز / نوٹس |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Norito ٹرانسپورٹ کا ہارڈ آن/آف سوئچ۔ | `true` ہینڈلرز کو رجسٹر رکھتا ہے؛ `false` اسٹیج سے قطع نظر بند کر دیتا ہے۔ |
| `torii.transport.norito_rpc.require_mtls` | Norito endpoints کے لیے mTLS لازمی۔ | ڈیفالٹ `true`۔ صرف isolated staging میں بند کریں۔ |
| `torii.transport.norito_rpc.allowed_clients` | Norito کے لیے service accounts / API tokens کی allowlist۔ | CIDR blocks، token hashes یا OIDC client IDs فراہم کریں۔ |
| `torii.transport.norito_rpc.stage` | SDKs کو اعلان کردہ rollout stage۔ | `disabled` (Norito رد، JSON مجبور)، `canary` (صرف allowlist + enhanced telemetry)، `ga` (ہر authenticated client کے لیے ڈیفالٹ)۔ |
| `torii.preauth_scheme_limits.norito_rpc` | per‑scheme concurrency + burst بجٹ۔ | HTTP/WS throttles والی keys (مثلاً `max_in_flight`, `rate_per_sec`) استعمال کریں۔ Alertmanager اپ ڈیٹ کیے بغیر cap بڑھانا guard کو ناکارہ بناتا ہے۔ |
| `transport.norito_rpc.*` in `docs/source/config/client_api.md` | client‑facing overrides (CLI/SDK discovery)۔ | Torii کو push کرنے سے پہلے `cargo xtask client-api-config diff` سے تبدیلیاں دیکھیں۔ |

**Recommended brownout flow**

1. `torii.transport.norito_rpc.stage=disabled` سیٹ کریں۔
2. `enabled=true` رکھیں تاکہ probes/alert tests ہینڈلرز کو exercise کرتے رہیں۔
3. فوری روک کی ضرورت ہو تو `torii.preauth_scheme_limits.norito_rpc.max_in_flight` صفر کریں
   (مثلاً config propagation کے انتظار میں)۔
4. آپریٹر لاگ اپ ڈیٹ کریں اور نئی config digest کو stage report میں attach کریں۔

## 2. Operational checklists

- **Canary / staging runs** — `docs/source/runbooks/torii_norito_rpc_canary.md` فالو کریں۔
  یہی keys استعمال ہوتی ہیں اور `scripts/run_norito_rpc_smoke.sh` +
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` کے ایویڈنس artefacts درج ہیں۔
- **Production promotion** — `docs/source/torii/norito_rpc_stage_reports.md` میں stage report
  ٹیمپلیٹ چلائیں۔ config hash، allowlist hash، smoke bundle digest، Grafana export hash، اور
  alert drill ID ریکارڈ کریں۔
- **Rollback** — `stage` کو `disabled` پر واپس کریں، allowlist برقرار رکھیں، اور سوئچ کو
  stage report + incident log میں دستاویز کریں۔ root cause ٹھیک ہو جائے تو `stage=ga`
  سے پہلے canary checklist دوبارہ چلائیں۔

## 3. Telemetry & alerts

| اثاثہ | مقام | نوٹس |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | ریکویسٹ ریٹ، error codes، payload sizes، decode failures، اور adoption % کو ٹریک کرتا ہے۔ |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, `NoritoRpcFallbackSpike` gates۔ |
| Chaos script | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | alert expressions drift ہونے پر CI فیل کرتا ہے؛ ہر config تبدیلی کے بعد چلائیں۔ |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | ہر promotion کے ایویڈنس بنڈل میں logs شامل کریں۔ |

Dashboards کو export کر کے release ticket کے ساتھ attach کریں (`make
docs-portal-dashboards` in CI) تاکہ on‑call بغیر production Grafana رسائی کے
میٹرکس ری پلے کر سکے۔

## 4. Frequently asked questions

**Canary میں نیا SDK کیسے allow کریں؟**  
`torii.transport.norito_rpc.allowed_clients` میں service account/token شامل کریں،
Torii reload کریں، اور `docs/source/torii/norito_rpc_tracker.md` میں NRPC-2R کے تحت
تبدیلی درج کریں۔ SDK owner کو `scripts/run_norito_rpc_fixtures.sh --sdk <label>` سے
fixture run بھی محفوظ کرنی ہوگی۔

**اگر rollout کے دوران Norito decode فیل ہو جائے؟**  
`stage=canary` اور `enabled=true` رکھیں، failures کو `torii_norito_decode_failures_total` سے ٹرائیج کریں۔
SDKs JSON پر واپس جا سکتے ہیں اگر `Accept: application/x-norito` ہٹا دیں؛ Torii JSON جاری رکھے گا
جب تک stage دوبارہ `ga` پر نہ ہو۔

**Gateway صحیح manifest سرو کر رہا ہے یہ کیسے ثابت کریں؟**  
`scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host <gateway-host>` چلائیں تاکہ
probe `Sora-Proof` headers کے ساتھ Norito config digest ریکارڈ کرے۔ JSON آؤٹ پٹ کو stage report میں attach کریں۔

**Redaction overrides کہاں capture کریں؟**  
ہر عارضی override کو stage report کے `Notes` کالم میں لکھیں اور Norito config patch کو change control میں لاگ کریں۔
Overrides خودکار طور پر ختم ہوتے ہیں؛ یہ FAQ on‑call کو cleanup یاد دلاتا ہے۔

یہاں شامل نہ ہونے والے سوالات کے لیے canary runbook کے چینلز سے escalations کریں
(`docs/source/runbooks/torii_norito_rpc_canary.md`)۔

## 5. Release note snippet (OPS-NRPC follow-up)

روڈمیپ آئٹم **OPS-NRPC** ایک تیار شدہ release note مانگتا ہے تاکہ آپریٹرز Norito‑RPC
rollout کو یکساں انداز میں announce کریں۔ نیچے والے بلاک کو اگلی release پوسٹ میں
کاپی کریں (bracketed فیلڈز بدلیں) اور نیچے بیان کردہ evidence bundle attach کریں۔

> **Torii Norito-RPC transport** — Norito envelopes اب JSON API کے ساتھ سرور ہوتے ہیں۔
> `torii.transport.norito_rpc.stage` کو **[stage: disabled/canary/ga]** پر سیٹ کر کے
> `docs/source/torii/norito_rpc_rollout_plan.md` کی staged checklist فالو کریں۔
> آپریٹرز عارضی طور پر `torii.transport.norito_rpc.stage=disabled` کر کے opt‑out کر سکتے ہیں
> جبکہ `torii.transport.norito_rpc.enabled=true` برقرار رکھیں؛ SDKs خودکار طور پر JSON پر
> واپس آ جائیں گے۔ Telemetry dashboards
> (`dashboards/grafana/torii_norito_rpc_observability.json`) اور alert drills
> (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) stage بڑھانے سے پہلے لازمی ہیں،
> اور `python/iroha_python/scripts/run_norito_rpc_smoke.sh` کے canary/smoke artefacts
> release ticket کے ساتھ منسلک ہونے چاہئیں۔

شائع کرنے سے پہلے:

1. **[stage: …]** کو Torii میں اعلان شدہ stage سے بدلیں۔
2. Release ticket کو تازہ ترین stage report (`docs/source/torii/norito_rpc_stage_reports.md`) سے لنک کریں۔
3. اوپر ذکر کردہ Grafana/Alertmanager exports کو `scripts/run_norito_rpc_smoke.sh` کے smoke bundle hashes کے ساتھ اپ لوڈ کریں۔

یہ snippet OPS-NRPC کی ضرورت پوری کرتی ہے بغیر اس کے کہ incident commanders ہر بار rollout status کو دوبارہ لکھیں۔

</div>
