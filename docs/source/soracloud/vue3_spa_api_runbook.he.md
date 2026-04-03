<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + API Runbook

ספר הפעלה זה מכסה פריסה ותפעול מוכווני ייצור עבור:

- אתר סטטי Vue3 (`--template site`); ו
- שירות Vue3 SPA + API (`--template webapp`),

באמצעות ממשקי API של Soracloud Control-plane ב-Iroha 3 עם הנחות SCR/IVM (לא
תלות בזמן ריצה של WASM וללא תלות Docker).

## 1. צור פרויקטי תבנית

פיגום אתר סטטי:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

פיגום SPA + API:

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

כל ספריית פלט כוללת:

- `container_manifest.json`
- `service_manifest.json`
- קובצי מקור של תבנית תחת `site/` או `webapp/`

## 2. בניית חפצי יישום

אתר סטטי:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

ממשק ספא + API:

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. ארוז ופרסם נכסי קצה

עבור אירוח סטטי דרך SoraFS:

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

עבור חזית ספא:

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. פרוס למטוס בקרה חי של Soracloud

פריסת שירות אתר סטטי:

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

פריסת שירות SPA + API:

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

אימות מחייב מסלול ומצב השקה:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

בדיקות צפויות במטוסי בקרה:

- סט `control_plane.services[].latest_revision.route_host`
- סט `control_plane.services[].latest_revision.route_path_prefix` (`/` או `/api`)
- `control_plane.services[].active_rollout` קיים מיד לאחר השדרוג

## 5. שדרוג עם השקה מבוססת בריאות

1. Bump `service_version` במניפסט השירות.
2. הפעל שדרוג:

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. קדם את ההשקה לאחר בדיקות בריאות:

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. אם הבריאות נכשלת, דווח על מצב לא בריא:```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

כאשר דיווחים לא בריאים מגיעים לסף המדיניות, Soracloud מתגלגל אוטומטית
חזרה לגרסה הבסיסית ומתעדת אירועי ביקורת חוזרת.

## 6. חזרה ידנית ותגובה לאירועים

חזרה לגרסה הקודמת:

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

השתמש בפלט סטטוס כדי לאשר:

- `current_version` הוחזר
- `audit_event_count` מוגדל
- `active_rollout` נוקה
- `last_rollout.stage` הוא `RolledBack` עבור החזרות אוטומטיות

## 7. רשימת פעולות

- שמור על מניפסטים שנוצרו על ידי תבנית תחת בקרת גרסאות.
- הקלט `governance_tx_hash` עבור כל שלב השקה כדי לשמור על העקיבות.
- לטפל ב-`service_health`, `routing`, `resource_pressure`, ו
  `failed_admissions` ככניסות לשער יציאה.
- השתמש באחוזים קנריים ובקידום מפורש במקום בגזרה מלאה ישירה
  שדרוגים עבור שירותים פונים למשתמש.
- אימות התנהגות/אימות ואימות חתימה ב
  `webapp/api/server.mjs` לפני הייצור.