---
lang: he
direction: rtl
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_elastic_lane.md -->

# ערכת הקמה ל-Elastic Lane Provisioning (NX-7)

> **קישור לרודמפ:** NX-7 — Elastic lane provisioning tooling  
> **סטטוס:** הכלי הושלם — מייצר manifests, catalog snippets, Norito payloads ו-smoke tests,
> ו-helper של load-test bundle מחבר כעת gating של slot latency עם manifests של evidence כדי לפרסם
> הרצות עומס ל-validator בלי scripting מותאם אישית.

המדריך הזה מסביר על `scripts/nexus_lane_bootstrap.sh`, שמאוטומט את יצירת manifests ל-lane,
snippets של catalog ל-lane/dataspace, וראיות rollout. המטרה היא לאפשר הקמה מהירה של Nexus lanes
(ציבוריות או פרטיות) בלי לערוך הרבה קבצים ידנית או לגזור מחדש את גאומטריית הקטלוג.

## 1. תנאי קדם

1. אישור ממשל ל-alias של lane, dataspace, סט validators, fault tolerance (`f`) ו-policy של settlement.
2. רשימת validators סופית (account IDs) ורשימת namespaces מוגנים.
3. גישה ל-repo של קונפיגורציות כדי להוסיף את ה-snippets שנוצרו.
4. נתיבים ל-registry של manifests (ראו `nexus.registry.manifest_directory` ו-`cache_directory`).
5. אנשי קשר לטלמטריה/PagerDuty עבור ה-lane כדי לחבר התראות מיד עם ההפעלה.

## 2. יצירת lane artifacts

הריצו את ה-helper משורש הריפו:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

דגלים מרכזיים:

- `--lane-id` חייב להתאים לאינדקס של הרשומה החדשה ב-`nexus.lane_catalog`.
- `--dataspace-alias` ו-`--dataspace-id/hash` שולטות בכניסת dataspace catalog (ברירת מחדל היא lane id).
- `--validator` ניתן לחזרה או לקריאה מ-`--validators-file`.
- `--route-instruction` / `--route-account` מפיקים חוקי routing מוכנים להדבקה.
- `--metadata key=value` (או `--telemetry-contact/channel/runbook`) משמרים אנשי קשר של runbook כדי
  שהדשבורדים יציגו את ה-owners הנכונים.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` מוסיפים runtime-upgrade hook ל-manifest כאשר
  ה-lane דורשת בקרות מפעיל מורחבות.
- `--encode-space-directory` מריץ אוטומטית `cargo xtask space-directory encode`. השתמשו ב-
  `--space-directory-out` אם רוצים את קובץ `.to` בנתיב אחר.

הסקריפט יוצר שלושה artifacts בתוך `--output-dir` (ברירת מחדל: ספריית העבודה), ועוד רביעי אופציונלי
כש-encoding מופעל:

1. `<slug>.manifest.json` — manifest של lane עם quorum של validators, namespaces מוגנים ו-metadata
   אופציונלית ל-runtime-upgrade hook.
2. `<slug>.catalog.toml` — snippet TOML עם `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`
   וחוקי routing שביקשתם. ודאו ש-`fault_tolerance` מוגדר בכניסת dataspace כדי להגדיר את ועדת
   lane-relay ל-`3f+1`.
3. `<slug>.summary.json` — audit summary שמפרט את הגאומטריה (slug/segments/metadata), שלבי rollout
   והפקודה המדויקת של `cargo xtask space-directory encode` (בשדה `space_directory_encode.command`).
   צירפו JSON זה לטיקט onboarding כהוכחה.
4. `<slug>.manifest.to` — נוצר כש-`--encode-space-directory` פעיל; מוכן לזרימה
   `iroha app space-directory manifest publish` של Torii.

השתמשו ב-`--dry-run` כדי להציג JSON/snippets בלי כתיבה, וב-`--force` כדי לדרוס artifacts קיימים.

## 3. החלת השינויים

1. העתיקו את manifest JSON ל-`nexus.registry.manifest_directory` שהוגדר (ול-`cache_directory` אם
   ה-registry משקף bundles מרוחקים). בצעו commit אם manifests מנוהלים ב-repo.
2. הוסיפו את catalog snippet ל-`config/config.toml` (או `config.d/*.toml`). ודאו ש-`nexus.lane_count`
   לפחות `lane_id + 1`, ועדכנו כל `nexus.routing_policy.rules` שצריכים להפנות ל-lane החדשה.
3. בצעו encoding (אם דילגתם על `--encode-space-directory`) ופרסמו את manifest ב-Space Directory לפי
   הפקודה מתוך summary (`space_directory_encode.command`). זה מייצר את `.manifest.to` הדרוש ל-Torii
   ומספק evidence לאודיטורים; שלחו עם `iroha app space-directory manifest publish`.
4. הריצו `irohad --sora --config path/to/config.toml --trace-config` וארכבו את פלט ה-trace בטיקט
   rollout. זה מוכיח שהגאומטריה החדשה תואמת ל-slug/segments שנוצרו.
5. אתחלו מחדש את ה-validators שמוקצים ל-lane לאחר פריסת manifest/catalog. שמרו את summary JSON
   בטיקט לצורך ביקורות עתידיות.

## 4. בניית registry distribution bundle

לאחר ש-manifest, catalog snippet ו-summary מוכנים, ארזו אותם להפצה ל-validators. ה-bundler החדש
מעתיק manifests לפריסה המצופה על ידי `nexus.registry.manifest_directory` / `cache_directory`,
מייצר governance catalog overlay כדי להחליף מודולים בלי לגעת ב-config הראשי, ומאחסן bundle
באופן אופציונלי:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

תוצרים:

1. `manifests/<slug>.manifest.json` — להעתיק ל-`nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — להציב ב-`nexus.registry.cache_directory` כדי לבצע override או
   החלפה של מודולי governance (`--module ...` מחליף את ה-cached catalog). זהו הנתיב ה-pluggable של
   NX-2: מחליפים הגדרת module, מריצים bundler מחדש ומפיצים overlay של cache בלי לשנות `config.toml`.
3. `summary.json` — כולל digests מסוג SHA-256 / Blake2b לכל manifest וכן metadata של overlay.
4. `registry_bundle.tar.*` אופציונלי — מוכן ל-Secure Copy או אחסון artifacts.

אם הפריסה משקפת bundles ל-hosts מבודדים (air-gapped), סנכרנו את כל תיקיית ה-output (או tarball).
Nodes אונליין יכולים למפות את manifest directory ישירות, ואילו nodes אופליין צורכים את tarball,
מחלצים ומעתיקים manifests + cache overlay לנתיבים שהוגדרו.

## 5. Smoke tests ל-validators

אחרי אתחול Torii, הריצו את smoke helper כדי לוודא שה-lane מדווחת `manifest_ready=true`, שה-metrics
מציגות את מספר ה-lanes הצפוי, ושה-sealed gauge נקי. lanes שדורשות manifest חייבות לחשוף
`manifest_path` לא ריק — ה-helper נכשל מהר אם חסרה ראיה, כדי שכל שינוי NX-7 יכלול references של
bundle חתום.

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

הוסיפו `--insecure` בסביבות self-signed. הסקריפט יוצא עם non-zero אם ה-lane חסרה, sealed, או אם
ה-metrics/telemetry חורגים מהציפיות. השתמשו ב-`--min-block-height`, `--max-finality-lag`,
`--max-settlement-backlog`, ו-`--max-headroom-events` כדי לשמור על telemetria של
height/finality/backlog/headroom בטווחים תפעוליים. שלבו עם `--max-slot-p95/--max-slot-p99`
(וגם `--min-slot-samples`) כדי לאכוף SLO של NX-18 slot-duration ישירות ב-helper, והשתמשו
ב-`--allow-missing-lane-metrics` רק כאשר clusters של staging עדיין לא מציגים את ה-gauges הללו
(בפרודקשן השאירו את ברירות המחדל).

ה-helper כעת אוכף גם telemetria של load-test עבור scheduler. השתמשו ב-`--min-teu-capacity` כדי
לוודא שכל lane מדווחת `nexus_scheduler_lane_teu_capacity`, הגבילו slot utilization דרך
`--max-teu-slot-commit-ratio` (משווה `nexus_scheduler_lane_teu_slot_committed` לקיבולת), ושמרו על
מוני deferral/truncation באפס באמצעות `--max-teu-deferrals` ו-`--max-must-serve-truncations`.
כך הופכת דרישת NX-7 ל"deeper validator load tests" ל-check CLI חוזר: ה-helper נכשל כאשר lane דוחה
עבודת PQ/TEU או כאשר committed TEU לכל slot חוצה את ה-headroom שהוגדר, וה-CLI מדפיס תקציר לכל
lane כך שחבילת evidence תתעד את אותם מספרים ש-CI בדקה.

עבור בדיקות air-gapped (או CI) ניתן להריץ מול תשובה מוקלטת מ-Torii במקום מול node חי:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

ה-fixtures תחת `fixtures/nexus/lanes/` משקפות את artifacts שיוצאים מה-helper כדי לאפשר lint ל-manifests
חדשים בלי scripting ייעודי. CI מריצה את אותו flow דרך `ci/check_nexus_lane_smoke.sh` וגם
`ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) כדי לוודא שה-helper של
NX-7 נשאר תואם לפורמט ה-payload שפורסם ושה-digests/overlays של bundle נשארים reproducible.

כאשר משנים שם lane, אספו אירועי telemetria `nexus.lane.topology` (למשל עם
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) והזינו אותם ל-helper.
הדגלים `--telemetry-file/--from-telemetry` מקבלים log newline-delimited, ו-`--require-alias-migration old:new`
מאמתים שאירוע `alias_migrated` תיעד את ה-rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

ה-fixture `telemetry_alias_migrated.ndjson` כולל דוגמת rename קנונית כדי ש-CI תוכל לאמת parsing של
telemetria בלי לגשת ל-node חי.

## 6. Validator load tests (ראיות NX-7)

Roadmap **NX-7** מחייבת מפעילי lane לתעד הרצת עומס שחזורית לפני שמסמנים lane כ-production ready.
המטרה היא להעמיס מספיק כדי למדוד slot duration, settlement backlog, DA quorum, oracles,
scheduler headroom ו-TEU metrics, ואז לארכב את התוצאה כך שאודיטורים יוכלו לשחזר בלי tooling מותאם.
ה-helper `scripts/nexus_lane_load_test.py` מאגד smoke checks, slot-duration gating ו-slot bundle
manifest לסט artifacts אחד כדי לפרסם עומסים ישירות ל-tickets של governance.

### 6.1 הכנת workload

1. צרו תיקיית run וצלמו fixtures קנוניים עבור ה-lane הנבדקת:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   ה-fixtures משקפות `fixtures/nexus/lane_commitments/*.json` ומספקות seed דטרמיניסטי למחולל העומס
   (רשמו את ה-seed ב-`artifacts/.../README.md`).
2. בצעו baseline לפני ההרצה:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   שמרו stdout/stderr בתיקיית run כדי שה-thresholds של smoke יהיו audit-able.
3. אספו telemetry log להזנת `--telemetry-file` (ראיות alias migration) ו-`validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. הריצו את ה-workload (k6 profile, replay harness או federation ingestion tests) ושמרו seed +
   טווח slots; ה-metadata נצרכת על ידי telemetry manifest validator בסעיף 6.3.

5. ארזו את ראיות ה-run עם ה-helper החדש. ספקו payloads של status/metrics/telemetry, lane aliases,
   ואירועי alias migration. ה-helper מייצר `smoke.log`, `slot_summary.json`, slot bundle manifest
   ו-`load_test_manifest.json` שמאגד הכל לביקורת governance:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   הפקודה אוכפת את אותם gates של DA quorum, oracle, settlement buffer, TEU ו-slot-duration כמו
   במדריך, ומייצרת manifest מוכן לצירוף בלי scripting מותאם.

### 6.2 הרצה מדודה

בזמן שה-workload בעומס:

1. קחו snapshot של Torii status + metrics:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. חשבו slot-duration quantiles וארכבו את ה-summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. ייצאו lane-governance snapshot ל-JSON + Parquet עבור audits לטווח ארוך:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   ה-JSON/Parquet snapshot רושם TEU utilization, רמות trigger של scheduler, מוני RBC chunk/byte
   וסטטיסטיקות גרף טרנזקציות לכל lane כדי שהראיות יראו גם backlog וגם לחץ ביצוע.

4. הריצו smoke helper שוב בשיא העומס כדי לבדוק thresholds תחת לחץ (כתבו ל-`smoke_during.log`),
   ואז הריצו שוב לאחר סיום ה-workload (`smoke_after.log`).

### 6.3 Telemetry pack ו-governance manifest

תיקיית ה-run חייבת להכיל telemetry pack (`prometheus.tgz`, OTLP stream, structured logs ותוצרי
harness). אימתו את ה-pack וסמנו metadata שנדרשת ע"י governance:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

לבסוף, צרפו את telemetry log שנאסף ודרשו evidence של alias migration כאשר lane משתנה במהלך הבדיקה:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

ארכבו את artifacts הבאים עבור טיקט governance:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + תוכן ה-pack (`prometheus.tgz`, `otlp.ndjson`, וכו')
- `nexus.lane.topology.ndjson` (או slice telemetria רלוונטי)

כעת ניתן להפנות להרצה בתוך Space Directory manifests וב-governance trackers כ-load test קנוני
ל-NX-7 עבור ה-lane.

## 7. Telemetry ו-governance follow-ups

- עדכנו את dashboards של lane (`dashboards/grafana/nexus_lanes.json` ו-overlays קשורים) עם lane id
  חדש ו-metadata. keys שנוצרו (`contact`, `channel`, `runbook`, וכו') מקלים על מילוי labels.
- חברו כללי PagerDuty/Alertmanager ל-lane החדשה לפני שמפעילים admission. `summary.json` משקף את
  ה-checklist ב-`docs/source/nexus_operations.md`.
- רשמו את manifest bundle ב-Space Directory כאשר סט validators פעיל. השתמשו באותו manifest JSON
  שנוצר על ידי ה-helper, חתום לפי runbook governance.
- עקבו אחרי `docs/source/sora_nexus_operator_onboarding.md` עבור smoke tests (FindNetworkStatus,
  Torii reachability) ותעדו את ה-evidence עם סט artifacts שהופק לעיל.

## 8. דוגמת dry-run

כדי להציג artifacts בלי לכתוב קבצים:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

הפקודה מדפיסה את JSON summary ואת ה-TOML snippet ל-stdout, ומאפשרת איטרציה מהירה בתכנון.

---

להקשר נוסף ראו:

- `docs/source/nexus_operations.md` — checklist תפעולי ודרישות telemetria.
- `docs/source/sora_nexus_operator_onboarding.md` — זרימת onboarding מפורטת שמפנה ל-helper החדש.
- `docs/source/nexus_lanes.md` — geometry של lanes, slugs, ו-storage layout שהכלי משתמש בו.

</div>
