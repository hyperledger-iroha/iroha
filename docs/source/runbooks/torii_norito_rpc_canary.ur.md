---
lang: ur
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# Torii Norito-RPC کینری رن بُک (NRPC-2C)

یہ رن بُک **NRPC-2** کے rollout پلان کو آپریشنل بناتی ہے اور بتاتی ہے کہ Norito‑RPC
ٹرانسپورٹ کو staging لیب ویلیڈیشن سے پروڈکشن “canary” مرحلے تک کیسے پروموٹ کیا
جائے۔ اسے ان کے ساتھ پڑھیں:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (پروٹوکول کنٹریکٹ)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Roles & Inputs

| رول | ذمہ داری |
|------|----------------|
| Torii Platform TL | config deltas منظور کرتا ہے، smoke tests سائن آف کرتا ہے۔ |
| NetOps | ingress/envoy تبدیلیاں لگاتا ہے اور canary pool صحت مانیٹر کرتا ہے۔ |
| Observability liaison | dashboards/alerts کی تصدیق اور evidence capture کرتا ہے۔ |
| Platform Ops | change ticket ڈرائیو کرتا ہے، rollback rehearsal کوآرڈینیٹ کرتا ہے، trackers اپ ڈیٹ کرتا ہے۔ |

Required artefacts:

- تازہ ترین `iroha_config` Norito patch جس میں `transport.norito_rpc.stage = "canary"` اور
  `transport.norito_rpc.allowed_clients` موجود ہو۔
- Envoy/Nginx config snippet جو `Content-Type: application/x-norito` کو preserve کرے اور
  canary client mTLS profile نافذ کرے (`defaults/torii_ingress_mtls.yaml`)۔
- Canary clients کے لیے token allowlist (YAML یا Norito manifest)۔
- Grafana URL + API token برائے `dashboards/grafana/torii_norito_rpc_observability.json`۔
- Parity smoke harness تک رسائی
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) اور alert drill script
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`)۔

## Pre-flight Checklist

1. **Spec freeze confirmed.** یقینی بنائیں کہ `docs/source/torii/nrpc_spec.md` کا hash
   آخری signed release سے میچ ہو اور Norito header/layout کو چھونے والی کوئی PR pending نہ ہو۔
2. **Config validation.** چلائیں
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   تاکہ `transport.norito_rpc.*` entries parse ہوں۔
3. **Scheme caps.** `torii.preauth_scheme_limits.norito_rpc` کو conservative رکھیں
   (مثلاً 25 concurrent connections) تاکہ binary callers JSON ٹریفک کو نہ دبائیں۔
4. **Ingress rehearsal.** staging میں Envoy patch لگائیں، negative test چلائیں
   (`cargo test -p iroha_torii -- norito_ingress`) اور HTTP 415 پر rejection دیکھیں۔
5. **Telemetry sanity.** staging میں `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` چلائیں اور evidence bundle attach کریں۔
6. **Token inventory.** canary allowlist میں ہر ریجن سے کم از کم دو operators شامل کریں؛
   `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json` میں محفوظ کریں۔
7. **Ticketing.** start/end window، rollback plan اور links کے ساتھ change ticket کھولیں۔

## Canary Promotion Procedure

1. **Config patch لگائیں۔**
   - `iroha_config` delta (stage=`canary`, allowlist populated, scheme limits set) کو admission کے ذریعے رول آؤٹ کریں۔
   - Torii restart یا hot‑reload کریں اور `torii.config.reload` logs میں acknowledge دیکھیں۔
2. **Ingress اپ ڈیٹ کریں۔**
   - Envoy/Nginx config deploy کریں جو Norito header routing/mTLS profile کو canary pool میں فعال کرے۔
   - `curl -vk --cert <client.pem>` responses میں Norito `X-Iroha-Error-Code` headers کی موجودگی چیک کریں۔
3. **Smoke tests۔**
   - `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary` چلائیں اور JSON + Norito transcripts
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/` میں محفوظ کریں۔
   - hashes کو `docs/source/torii/norito_rpc_stage_reports.md` میں درج کریں۔
4. **Telemetry observe کریں۔**
   - `torii_active_connections_total{scheme="norito_rpc"}` اور
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` کم از کم 30 منٹ دیکھیں۔
   - Grafana dashboard API سے export کر کے ticket میں attach کریں۔
5. **Alert rehearsal۔**
   - `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` چلائیں، malformed Norito envelopes inject کریں؛
     Alertmanager کے synthetic incident اور auto‑clear کی تصدیق کریں۔
6. **Evidence capture۔**
   - `docs/source/torii/norito_rpc_stage_reports.md` میں درج کریں:
     - Config digest
     - Allowlist manifest hash
     - Smoke test timestamp
     - Grafana export checksum
     - Alert drill ID
   - artefacts کو `artifacts/norito_rpc/<YYYYMMDD>/` میں اپ لوڈ کریں۔

## Monitoring & Exit Criteria

Canary میں رہیں جب تک یہ سب ≥72 گھنٹے تک درست نہ رہیں:

- Error rate (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % اور
  `torii_norito_decode_failures_total` میں کوئی sustained spikes نہ ہوں۔
- Latency parity (Norito vs JSON p95) 10 % کے اندر ہو۔
- Alert dashboard خاموش رہے (سوائے scheduled drills کے)۔
- Allowlist operators schema mismatch کے بغیر parity reports دیں۔

روزانہ کی حالت change ticket میں لکھیں اور `docs/source/status/norito_rpc_canary_log.md`
(اگر موجود ہو) میں snapshots محفوظ کریں۔

## Rollback Procedure

1. `transport.norito_rpc.stage` کو `"disabled"` پر واپس کریں اور `allowed_clients` صاف کریں؛ admission کے ذریعے اپلائی کریں۔
2. Envoy/Nginx route/mTLS stanza ہٹا دیں، proxies reload کریں اور نئی Norito connections ریفیوز ہونے کی تصدیق کریں۔
3. Canary tokens revoke کریں (یا bearer credentials deactivate کریں) تاکہ موجودہ sessions ختم ہوں۔
4. `torii_active_connections_total{scheme="norito_rpc"}` کو صفر تک مانیٹر کریں۔
5. JSON‑only smoke harness دوبارہ چلائیں تاکہ baseline functionality برقرار رہے۔
6. 24 گھنٹے کے اندر `docs/source/postmortems/norito_rpc_rollback.md` میں post‑mortem stub بنائیں اور ticket کو impact summary + metrics کے ساتھ اپ ڈیٹ کریں۔

## Post-Canary Close-Out

Exit criteria پورے ہونے پر:

1. `docs/source/torii/norito_rpc_stage_reports.md` میں GA recommendation اپ ڈیٹ کریں۔
2. `status.md` میں canary outcomes اور evidence bundles کا خلاصہ شامل کریں۔
3. SDK leads کو اطلاع دیں تاکہ staging fixtures Norito پر سوئچ ہوں۔
4. GA config patch (stage=`ga`, allowlist ہٹائیں) تیار کریں اور NRPC-2 پلان کے مطابق promotion شیڈول کریں۔

یہ رن بُک ہر canary promotion میں یکساں evidence جمع کرنے، deterministic rollback رکھنے اور
NRPC-2 acceptance criteria پورا کرنے کو یقینی بناتی ہے۔

</div>
