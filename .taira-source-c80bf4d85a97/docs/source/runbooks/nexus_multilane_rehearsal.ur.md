---
lang: ur
direction: rtl
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# Nexus ملٹی‑لین لانچ رِیہرسل رن بُک

یہ رن بُک Phase B4 Nexus رِیہرسل کی رہنمائی کرتی ہے۔ اس میں یہ تصدیق کی جاتی ہے کہ
گورننس سے منظور شدہ `iroha_config` بنڈل اور ملٹی‑لین genesis مینی فیسٹ ٹیلی میٹری،
روٹنگ اور رول بیک ڈرلز میں deterministic طور پر برتاؤ کرتے ہیں۔

## دائرہ کار

- Nexus کے تینوں lanes (`core`, `governance`, `zk`) کو mixed Torii ingress
  (ٹرانزیکشنز، کنٹریکٹ ڈیپلائمنٹس، گورننس ایکشنز) کے ساتھ چلائیں اور signed seed
  `NEXUS-REH-2026Q1` استعمال کریں۔
- B4 acceptance کے لیے درکار ٹیلی میٹری/ٹریس artefacts جمع کریں (Prometheus
  scrape، OTLP export، structured logs، Norito admission traces، RBC metrics)۔
- dry‑run کے فوراً بعد rollback drill `B4-RB-2026Q1` چلائیں اور single‑lane پروفائل
  کی صاف re‑apply تصدیق کریں۔

## پیشگی شرائط

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` میں GOV-2026-03-19
   منظوری (signed manifests + reviewer initials) درج ہوں۔
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, جس میں
   `nexus.enabled = true` شامل ہے) اور `defaults/nexus/genesis.json` منظور شدہ hashes
   سے میل کھائیں؛ `kagami genesis bootstrap --profile nexus` وہی digest دکھائے جو
   tracker میں درج ہے۔
3. lane catalog منظور شدہ تین‑لین لے آؤٹ سے میل کھائے؛ `irohad --sora
   --config defaults/nexus/config.toml` کو Nexus router banner دکھانا چاہیے۔
4. Multi‑lane CI گرین ہو: `ci/check_nexus_multilane_pipeline.sh`
   (`integration_tests/tests/nexus/multilane_pipeline.rs` کو
   `.github/workflows/integration_tests_multilane.yml` کے ذریعے چلاتا ہے) اور
   `ci/check_nexus_multilane.sh` (router coverage) دونوں پاس ہوں تاکہ Nexus پروفائل
   multi‑lane‑ready رہے (`nexus.enabled = true`, Sora catalog hashes برقرار، lane
   storage `blocks/lane_{id:03}_{slug}` میں، اور merge logs تیار)۔ defaults bundle
   بدلنے پر artefact digests tracker میں درج کریں۔
5. Nexus metrics کے dashboards + alerts رِیہرسل Grafana فولڈر میں امپورٹ ہوں؛ alert
   routes رِیہرسل PagerDuty سروس کی طرف ہوں۔
6. Torii SDK lanes routing policy ٹیبل کے مطابق سیٹ ہوں اور رِیہرسل ورک لوڈ لوکل چل سکے۔

## ٹائم لائن اوورویو

| فیز | ہدف ونڈو | اونرز | خروج معیار |
|-------|---------------|----------|---------------|
| تیاری | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed شائع، dashboards تیار، rehearsal nodes provisioned۔ |
| Staging freeze | Apr 8 2026 18:00 UTC | @release-eng | Config/genesis hashes دوبارہ verify؛ freeze نوٹس بھیجا۔ |
| اجرا | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist بلاکنگ انسیڈنٹس کے بغیر مکمل؛ telemetry pack محفوظ۔ |
| Rollback drill | فوراً بعد اجرا | @sre-core | `B4-RB-2026Q1` مکمل؛ rollback telemetry محفوظ۔ |
| Retrospective | Apr 15 2026 تک | @program-mgmt, @telemetry-ops, @governance | Retro/lessons doc + blocker tracker شائع۔ |

## Execution Checklist (Apr 9 2026 15:00 UTC)

1. **Config attestation** — ہر نوڈ پر `iroha_cli config show --actual`؛ hashes کو tracker entry سے ملائیں۔
2. **Lane warm‑up** — 2 slots کے لیے seed ورک لوڈ replay کریں، `nexus_lane_state_total`
   میں تینوں lanes کی activity دیکھیں۔
3. **Telemetry capture** — Prometheus `/metrics` snapshots، OTLP packet samples، Torii structured logs
   (lane/dataspace کے حساب سے)، اور RBC metrics ریکارڈ کریں۔
4. **Governance hooks** — governance ٹرانزیکشنز کا subset چلائیں اور lane routing + telemetry tags چیک کریں۔
5. **Incident drill** — پلان کے مطابق lane saturation simulate کریں؛ alerts firing اور response log یقینی بنائیں۔
6. **Rollback drill `B4-RB-2026Q1`** — single‑lane پروفائل لگائیں، rollback checklist replay کریں،
   telemetry evidence جمع کریں، اور Nexus bundle دوبارہ apply کریں۔
7. **Artefact upload** — telemetry pack، Torii traces، اور drill log کو Nexus evidence bucket میں اپ لوڈ کریں؛
   `docs/source/nexus_transition_notes.md` میں لنک کریں۔
8. **Manifest/validation** — `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` چلائیں تاکہ `telemetry_manifest.json`
   + `.sha256` بنے اور اسے tracker entry میں attach کریں۔ helper slot boundaries کو
   integers کے طور پر normalize کرتا ہے اور اگر کوئی hint غائب ہو تو فوری fail کرتا ہے۔

## Outputs

- سائن شدہ rehearsal checklist + incident drill log۔
- Telemetry pack (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`)۔
- Validation script سے تیار telemetry manifest + digest۔
- Retrospective doc جو blockers, mitigations اور owners سمری کرے۔

## Execution Summary — Apr 9 2026

- رِیہرسل 15:00 UTC–16:12 UTC تک `NEXUS-REH-2026Q1` seed کے ساتھ ہوا؛ تینوں lanes
  نے ~2.4k TEU فی slot برقرار رکھا اور `nexus_lane_state_total` نے balanced envelopes دکھائیں۔
- Telemetry pack `artifacts/nexus/rehearsals/2026q1/` میں archive ہوا (جس میں
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log اور
  rollback evidence شامل ہیں)؛ checksums `docs/source/project_tracker/nexus_rehearsal_2026q1.md` میں درج ہیں۔
- Rollback drill `B4-RB-2026Q1` 16:18 UTC پر مکمل ہوا؛ single‑lane پروفائل 6m42s میں
  بغیر lanes کے رکے دوبارہ apply ہوا، پھر telemetry تصدیق کے بعد Nexus bundle دوبارہ فعال ہوا۔
- Slot 842 پر lane saturation incident (forced headroom clamp) نے متوقع alerts فائر کیے؛
  mitigation playbook نے 11m میں page بند کیا، PagerDuty ٹائم لائن دستاویزی۔
- کوئی blocker نہ تھا؛ follow‑up items (TEU headroom logging automation، telemetry pack validator)
  Apr 15 retrospective میں ٹریک ہیں۔

## Escalation

- بلاکنگ انسیڈنٹس یا ٹیلی میٹری ریگریشن رِیہرسل روک دیتی ہے اور 4 کاروباری گھنٹوں میں
  گورننس اسکیلشن ضروری ہے۔
- منظور شدہ config/genesis bundle سے کوئی بھی انحراف رِیہرسل کو دوبارہ چلانے کا تقاضا کرتا ہے۔

## Telemetry Pack Validation (Completed)

ہر رِیہرسل کے بعد `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں تاکہ
telemetry bundle میں canonical artefacts (Prometheus export, OTLP NDJSON, Torii structured logs, rollback log)
کی موجودگی ثابت ہو اور SHA‑256 digests محفوظ ہوں۔ helper `telemetry_manifest.json` اور اس کی
`.sha256` فائل لکھتا ہے تاکہ governance retro packet میں evidence hashes براہِ راست cite کر سکے۔

Apr 9 2026 کی رِیہرسل کے لیے validated manifest `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`
کے ساتھ ہے اور اس کا digest `telemetry_manifest.json.sha256` میں ہے۔ دونوں فائلیں tracker entry میں attach کریں۔

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

CI میں `--require-slot-range` / `--require-workload-seed` استعمال کریں تاکہ
جن اپ لوڈز میں یہ hints غائب ہوں وہ بلاک ہو جائیں۔ اضافی artefacts (مثلاً DA receipts)
کے لیے `--expected <name>` استعمال کریں۔

</div>
