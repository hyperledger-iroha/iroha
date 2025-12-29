---
lang: he
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# FAQ תפעולי ל‑Norito-RPC

FAQ זה מרכז את תצורת ה‑rollout/rollback, הטלמטריה וארטיפקטי ההוכחות המוזכרים
בפריטים **NRPC-2** ו‑**NRPC-4** כדי שלמפעילים תהיה נקודת ייחוס אחת במהלך
canaries, brownouts או drills של תקריות. התייחסו אליו כדלת הכניסה למסירות
on‑call; הנהלים המפורטים נמצאים ב‑`docs/source/torii/norito_rpc_rollout_plan.md`
וב‑`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. מקשי תצורה

| נתיב | מטרה | ערכים מותריים / הערות |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | מתג on/off קשיח לתעבורת Norito. | `true` משאיר את ה‑handlers רשומים; `false` מכבה אותם ללא קשר ל‑stage. |
| `torii.transport.norito_rpc.require_mtls` | אכיפת mTLS לנקודות Norito. | ברירת מחדל `true`. לכבות רק ב‑staging מבודד. |
| `torii.transport.norito_rpc.allowed_clients` | רשימת היתרים של חשבונות שירות / טוקנים המורשים להשתמש ב‑Norito. | ספקו CIDR, hashes של טוקנים או OIDC client IDs בהתאם לפריסה. |
| `torii.transport.norito_rpc.stage` | שלב rollout המוכרז ל‑SDKs. | `disabled` (דוחה Norito, כופה JSON), `canary` (allowlist בלבד + טלמטריה מוגברת), `ga` (ברירת מחדל לכל לקוח מאומת). |
| `torii.preauth_scheme_limits.norito_rpc` | תקציב concurrency + burst לפי סכימה. | מראה את מפתחות ה‑HTTP/WS throttles (למשל `max_in_flight`, `rate_per_sec`). העלאת cap ללא עדכון Alertmanager שוברת את ההגנה. |
| `transport.norito_rpc.*` ב‑`docs/source/config/client_api.md` | Overrides בצד הלקוח (CLI / SDK discovery). | השתמשו ב‑`cargo xtask client-api-config diff` לפני דחיפה ל‑Torii. |

**זרימת brownout מומלצת**

1. הגדירו `torii.transport.norito_rpc.stage=disabled`.
2. השאירו `enabled=true` כדי ש‑probes/alert tests ימשיכו לאמן את ה‑handlers.
3. אם נדרש עצירה מיידית, קבעו `torii.preauth_scheme_limits.norito_rpc.max_in_flight` ל‑0.
4. עדכנו את יומן המפעיל וצרפו digest של התצורה החדשה לדו"ח השלב.

## 2. Checklists תפעוליים

- **Canary / staging** — עקבו אחרי `docs/source/runbooks/torii_norito_rpc_canary.md`.
  הראנבוק מפנה לאותן מפתחות תצורה ומפרט את ארטיפקטי ההוכחה מ‑
  `scripts/run_norito_rpc_smoke.sh` + `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Promotion לפרודקשן** — השתמשו בתבנית stage report ב‑
  `docs/source/torii/norito_rpc_stage_reports.md`. רשמו hash תצורה, hash allowlist,
  digest של smoke bundle, hash של export Grafana, ו‑ID של drill התראות.
- **Rollback** — החזירו את `stage` ל‑`disabled`, השאירו את ה‑allowlist ורשמו את
  השינוי ב‑stage report + יומן האירוע. לאחר תיקון השורש, הריצו שוב את checklist
  ה‑canary לפני `stage=ga`.

## 3. טלמטריה והתראות

| נכס | מיקום | הערות |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | עוקב אחר קצב בקשות, קודי שגיאה, גודל payloads, כשלי דיקוד ו‑% אימוץ. |
| Alerts | `dashboards/alerts/torii_norito_rpc_rules.yml` | שערים `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, `NoritoRpcFallbackSpike`. |
| סקריפט Chaos | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | מפיל CI אם ביטויי ההתראה נסחפים. להריץ לאחר כל שינוי תצורה. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | לכלול לוגים בחבילת ההוכחות לכל promotion. |

יש לייצא dashboards ולצרף לכרטיס release (`make docs-portal-dashboards` ב‑CI)
כדי שמפעילי on‑call יוכלו לשחזר מדדים ללא גישה ל‑Grafana פרודקשן.

## 4. שאלות נפוצות

**איך מתירים SDK חדש בזמן canary?**  
הוסיפו את החשבון/טוקן ל‑`torii.transport.norito_rpc.allowed_clients`, טענו מחדש
את Torii ותעדו את השינוי ב‑`docs/source/torii/norito_rpc_tracker.md` תחת NRPC-2R.
בעלי ה‑SDK חייבים גם לרשום ריצה של fixtures דרך `scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**מה קורה אם דיקוד Norito נכשל במהלך rollout?**  
השאירו `stage=canary` ו‑`enabled=true`, וטרג’ו את הכשלים דרך
`torii_norito_decode_failures_total`. בעלי SDK יכולים לחזור ל‑JSON אם יסירו
`Accept: application/x-norito`; Torii ימשיך להגיש JSON עד ש‑stage יחזור ל‑`ga`.

**איך מוכיחים שה‑gateway מגיש את המניפסט הנכון?**  
הריצו `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host <gateway-host>`
כדי שה‑probe יתעד את כותרות `Sora-Proof` לצד digest התצורה של Norito. צרפו את
פלט ה‑JSON לדו"ח השלב.

**היכן לתעד overrides של redaction?**  
תעדו כל override זמני בעמודת `Notes` בדו"ח השלב ורשמו את patch התצורה של Norito
ב‑change control. ה‑overrides פוקעים אוטומטית בקובץ התצורה; ה‑FAQ הזה מוודא
שצוותי on‑call יזכרו לנקות לאחר תקריות.

לשאלות שאינן מכוסות כאן, הסלימו דרך הערוצים המופיעים בראנבוק ה‑canary
(`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. קטע להערת שחרור (מעקב OPS-NRPC)

פריט **OPS-NRPC** מחייב קטע release‑note מוכן לשימוש כדי לאפשר הודעות עקביות
על rollout של Norito‑RPC. העתיקו את הבלוק הבא לפוסט השחרור הבא (החליפו את
השדות בסוגריים) וצרפו את חבילת ההוכחות המתוארת מתחת.

> **Torii Norito-RPC transport** — מעטפות Norito מוגשות כעת לצד ה‑JSON API.
> הדגל `torii.transport.norito_rpc.stage` נשלח כשהוא מוגדר ל‑
> **[stage: disabled/canary/ga]** ופועל לפי checklist השלבים ב‑
> `docs/source/torii/norito_rpc_rollout_plan.md`. מפעילים יכולים לצאת זמנית
> ע"י `torii.transport.norito_rpc.stage=disabled` תוך שמירה על
> `torii.transport.norito_rpc.enabled=true`; ה‑SDKs יחזרו ל‑JSON אוטומטית.
> Dashboards טלמטריה (`dashboards/grafana/torii_norito_rpc_observability.json`)
> ו‑alert drills (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) נותרו
> חובה לפני העלאת stage, וארטיפקטי canary/smoke של
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` חייבים להיות מצורפים
> לכרטיס השחרור.

לפני פרסום:

1. החליפו את **[stage: …]** בשלב שמפורסם ב‑Torii.
2. קשרו את כרטיס השחרור ל‑stage report האחרון ב‑`docs/source/torii/norito_rpc_stage_reports.md`.
3. העלו את exports של Grafana/Alertmanager שהוזכרו לעיל יחד עם hashes של smoke bundle
   מ‑`scripts/run_norito_rpc_smoke.sh`.

הקטע הזה מספק את דרישת OPS-NRPC מבלי להכריח incident commanders לשכתב את סטטוס
ה‑rollout בכל פעם.

</div>
