<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 439248de434ac053bdf457055a2ac9ff501d19b371a435521e61e6a33566a1f6
source_last_modified: "2025-11-07T10:31:34.047816+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے ماخوذ۔

# SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان

یہ پلان permissive provider adverts سے مکمل طور پر governed `ProviderAdvertV1`
surface پر cut-over کو کوآرڈی نیٹ کرتا ہے جو multi-source chunk retrieval کے لیے
ضروری ہے۔ یہ تین deliverables پر فوکس کرتا ہے:

- **Operator guide.** وہ قدم بہ قدم اقدامات جو storage providers کو ہر gate فلپ ہونے
  سے پہلے مکمل کرنا ہیں۔
- **Telemetry coverage.** dashboards اور alerts جنہیں Observability اور Ops استعمال
  کرتے ہیں تاکہ نیٹ ورک صرف compliant adverts قبول کرے۔
- **Compatibility timeline.** legacy envelopes کو reject کرنے کی واضح تاریخیں تاکہ
  SDKs اور tooling ٹیمیں اپنی releases پلان کر سکیں۔

یہ rollout [SoraFS migration roadmap](./migration-roadmap) کی SF-2b/2c milestones
کے ساتھ align ہے اور فرض کرتا ہے کہ [provider admission policy](./provider-admission-policy)
پہلے سے نافذ ہے۔

## Phase Timeline

| Phase | Window (target) | Behaviour | Operator Actions | Observability Focus |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 – Baseline observation** | Through **2025-03-31** | Torii دونوں governance-approved adverts اور legacy payloads (جو `ProviderAdvertV1` سے پہلے کے ہوں) قبول کرتا ہے۔ جب adverts میں `chunk_range_fetch` یا canonical `profile_aliases` نہ ہوں تو ingestion logs warning دیتے ہیں۔ | - provider advert publishing pipeline (ProviderAdvertV1 + governance envelope) کے ذریعے adverts دوبارہ generate کریں، `profile_id=sorafs.sf1@1.0.0`, canonical `profile_aliases` اور `signature_strict=true` یقینی بنائیں۔ <br />- نئے `sorafs_fetch` tests لوکل چلائیں؛ unknown capabilities کی warnings کو triage کریں۔ | عارضی Grafana panels شائع کریں (نیچے دیکھیں) اور alert thresholds set کریں مگر انہیں warning-only موڈ میں رکھیں۔ |
| **R1 – Warning gate** | **2025-04-01 → 2025-05-15** | Torii legacy adverts قبول کرتا رہتا ہے مگر جب payload میں `chunk_range_fetch` موجود نہ ہو یا `allow_unknown_capabilities=true` کے بغیر unknown capabilities ہوں تو `torii_sorafs_admission_total{result="warn"}` بڑھاتا ہے۔ CLI tooling اب canonical handle کے بغیر regeneration پر fail کرتا ہے۔ | - staging اور production میں adverts rotate کریں تاکہ `CapabilityType::ChunkRangeFetch` payloads شامل ہوں، اور GREASE testing پر `allow_unknown_capabilities=true` سیٹ کریں۔ <br />- operations runbooks میں نئی telemetry queries نوٹ کریں۔ | dashboards کو on-call rotation میں promote کریں؛ جب `warn` events 15 منٹ میں 5% سے اوپر جائیں تو warnings configure کریں۔ |
| **R2 – Enforcement** | **2025-05-16 → 2025-06-30** | Torii ایسے adverts reject کرتا ہے جن میں governance envelopes، canonical profile handle یا `chunk_range_fetch` capability نہ ہو۔ legacy `namespace-name` handles اب parse نہیں ہوتے۔ GREASE opt-in کے بغیر unknown capabilities اب `reason="unknown_capability"` کے ساتھ fail کرتی ہیں۔ | - تصدیق کریں کہ production envelopes `torii.sorafs.admission_envelopes_dir` کے تحت موجود ہیں اور باقی legacy adverts rotate کریں۔ <br />- SDKs کی تصدیق کریں کہ وہ صرف canonical handles اور optional aliases emit کرتے ہیں۔ | pager alerts فعال کریں: `torii_sorafs_admission_total{result="reject"}` > 0 اگر 5 منٹ رہے تو operator action ضروری ہے۔ acceptance ratio اور admission reason histograms ٹریک کریں۔ |
| **R3 – Legacy shutdown** | **2025-07-01 onwards** | Discovery ایسے binary adverts کی سپورٹ ختم کرتا ہے جو `signature_strict=true` سیٹ نہ کریں یا `profile_aliases` چھوڑ دیں۔ Torii discovery cache ایسے stale entries پاک کرتا ہے جن کی refresh deadline renewal کے بغیر گزر جائے۔ | - legacy provider stacks کے لیے آخری decommission window شیڈول کریں۔ <br />- `--allow-unknown` GREASE runs صرف controlled drills میں کریں اور log کریں۔ <br />- incident playbooks اپڈیٹ کریں تاکہ `sorafs_fetch` warning output کو releases سے پہلے blocker سمجھا جائے۔ | alerts سخت کریں: کوئی بھی `warn` on-call کو alert کرے۔ synthetic checks شامل کریں جو discovery JSON fetch کریں اور provider capability lists validate کریں۔ |

## Operator Checklist

1. **Inventory adverts.** ہر published advert کی فہرست بنائیں اور ریکارڈ کریں:
   - Governing envelope path (`defaults/nexus/sorafs_admission/...` یا production equivalent).
   - advert `profile_id` اور `profile_aliases`.
   - capability list (کم از کم `torii_gateway` اور `chunk_range_fetch`).
   - `allow_unknown_capabilities` flag (جب vendor-reserved TLVs ہوں تو ضروری ہے)۔
2. **Regenerate with provider tooling.**
   - اپنے provider advert publisher سے payload دوبارہ بنائیں، اور یقینی بنائیں:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` اور واضح `max_span`
     - GREASE TLVs کی صورت میں `allow_unknown_capabilities=<true|false>`
   - `/v1/sorafs/providers` اور `sorafs_fetch` سے validate کریں؛ unknown
     capabilities کی warnings کو triage کریں۔
3. **Validate multi-source readiness.**
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں؛ اب `chunk_range_fetch`
     نہ ہونے پر CLI fail کرتا ہے اور ignored unknown capabilities کے لیے warnings دیتا ہے۔
     JSON report محفوظ کریں اور operations logs کے ساتھ archive کریں۔
4. **Stage renewals.**
   - gateway enforcement (R2) سے کم از کم 30 دن پہلے `ProviderAdmissionRenewalV1`
     envelopes جمع کریں۔ renewals میں canonical handle اور capability set برقرار
     رہنا چاہئے؛ صرف stake، endpoints یا metadata بدلے۔
5. **Communicate with dependent teams.**
   - SDK owners کو ایسی releases دینا ہوں گی جو adverts reject ہونے پر operators کو warnings دکھائیں۔
   - DevRel ہر phase transition announce کرے؛ dashboard links اور threshold logic شامل کریں۔
6. **Install dashboards & alerts.**
   - Grafana export امپورٹ کریں اور **SoraFS / Provider Rollout** کے تحت رکھیں، UID
     `sorafs-provider-admission` رکھیں۔
   - یقینی بنائیں کہ alert rules staging اور production میں shared
     `sorafs-advert-rollout` notification channel پر جائیں۔

## Telemetry & Dashboards

یہ metrics پہلے ہی `iroha_telemetry` کے ذریعے دستیاب ہیں:

- `torii_sorafs_admission_total{result,reason}` — accepted, rejected اور warning
  outcomes گنتا ہے۔ reasons میں `missing_envelope`, `unknown_capability`, `stale`
  اور `policy_violation` شامل ہیں۔

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
فائل کو shared dashboards repository (`observability/dashboards`) میں import کریں اور
شائع کرنے سے پہلے صرف datasource UID اپڈیٹ کریں۔

بورڈ Grafana folder **SoraFS / Provider Rollout** میں stable UID
`sorafs-provider-admission` کے ساتھ publish ہوتا ہے۔ alert rules
`sorafs-admission-warn` (warning) اور `sorafs-admission-reject` (critical)
پہلے سے `sorafs-advert-rollout` notification policy استعمال کرنے کے لیے configured ہیں؛
اگر destination list بدلے تو dashboard JSON کو edit کرنے کے بجائے contact point
update کریں۔

Recommended Grafana panels:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart جو accept vs warn vs reject دکھاتا ہے۔ warn > 0.05 * total (warning) یا reject > 0 (critical) پر alert۔ |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Single-line timeseries جو pager threshold کو feed کرتی ہے (15 منٹ میں 5% warning rate)۔ |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | runbook triage کے لیے؛ mitigation steps کے لنکس شامل کریں۔ |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | refresh deadline miss کرنے والے providers کو ظاہر کرتا ہے؛ discovery cache logs کے ساتھ cross-reference کریں۔ |

Manual dashboards کے لیے CLI artefacts:

- `sorafs_fetch --provider-metrics-out` ہر provider کے لیے `failures`, `successes`,
  اور `disabled` counters لکھتا ہے۔ orchestrator dry-runs کو monitor کرنے کے لیے
  ad-hoc dashboards میں import کریں۔
- JSON report کے `chunk_retry_rate` اور `provider_failure_rate` fields throttling یا
  stale payload symptoms دکھاتے ہیں جو اکثر admission rejects سے پہلے آتے ہیں۔

### Grafana dashboard layout

Observability ایک dedicated board — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — **SoraFS / Provider Rollout** کے تحت
publish کرتا ہے، اور اس کے canonical panel IDs یہ ہیں:

- Panel 1 — *Admission outcome rate* (stacked area, یونٹ "ops/min").
- Panel 2 — *Warning ratio* (single series)، اظہار
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (`reason` کے حساب سے time series)، `rate(...[5m])`
  کے مطابق sort کی گئی۔
- Panel 4 — *Refresh debt* (stat)، اوپر والی table query کو mirror کرتا ہے اور
  migration ledger سے حاصل کردہ advert refresh deadlines کے ساتھ annotated ہے۔

JSON skeleton کو infrastructure dashboards repo میں `observability/dashboards/sorafs_provider_admission.json`
پر copy یا create کریں، پھر صرف datasource UID اپڈیٹ کریں؛ panel IDs اور alert rules
نیچے runbooks میں referenced ہیں، اس لیے انہیں renumber کرنے سے پہلے docs اپڈیٹ کریں۔

سہولت کے لیے ریپو `docs/source/grafana_sorafs_admission.json` میں reference dashboard
definition دیتا ہے؛ لوکل testing کے لیے اسے اپنے Grafana folder میں copy کر لیں۔

### Prometheus alert rules

مندرجہ ذیل rule group کو `observability/prometheus/sorafs_admission.rules.yml`
میں شامل کریں (اگر یہ پہلا SoraFS rule group ہے تو فائل بنائیں) اور Prometheus
configuration سے include کریں۔ `<pagerduty>` کو اپنے on-call routing label سے بدلیں۔

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
چلا کر تصدیق کریں کہ syntax `promtool check rules` پاس کر رہا ہے۔

## Compatibility Matrix

| Advert Characteristics | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` present, canonical aliases, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| `chunk_range_fetch` capability غائب | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| `allow_unknown_capabilities=true` کے بغیر unknown capability TLVs | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| Legacy handle فقط (`profile_id = sorafs.sf1@1.0.0`) | ⚠️ Warn | ❌ Reject | ❌ Reject | ❌ Reject |
| `refresh_deadline` ختم | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (diagnostic fixtures) | ✅ (صرف development) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

تمام اوقات UTC میں ہیں۔ enforcement کی تاریخیں migration ledger میں mirror ہیں اور council
ووٹ کے بغیر تبدیل نہیں ہوں گی؛ کسی بھی تبدیلی کے لیے اسی PR میں یہ فائل اور ledger اپڈیٹ کرنا ہوگا۔

> **Implementation note:** R1 `torii_sorafs_admission_total` میں `result="warn"`
> series متعارف کراتا ہے۔ Torii ingestion patch جو یہ label شامل کرتا ہے SF-2 telemetry
> tasks کے ساتھ track ہوتا ہے؛ اس کے آنے تک legacy adverts کی نگرانی کے لیے log sampling استعمال کریں۔

## Communication & Incident Handling

- **Weekly status mailer.** DevRel admission metrics، outstanding warnings اور آنے والی deadlines کا خلاصہ شیئر کرتا ہے۔
- **Incident response.** اگر `reject` alerts فائر ہوں تو on-call انجینئر:
  1. Torii discovery (`/v1/sorafs/providers`) سے offending advert fetch کریں۔
  2. provider pipeline میں advert validation دوبارہ چلائیں اور `/v1/sorafs/providers` سے compare کریں تاکہ error reproduce ہو۔
  3. provider کے ساتھ coordinate کریں تاکہ اگلی refresh deadline سے پہلے advert rotate ہو جائے۔
- **Change freezes.** R1/R2 کے دوران capability schema میں تبدیلیاں نہ کریں جب تک rollout کمیٹی منظوری نہ دے؛ GREASE trials کو ہفتہ وار maintenance window میں schedule کریں اور migration ledger میں log کریں۔

## References

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
