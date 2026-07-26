---
lang: he
direction: rtl
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# ראנבוק Canary ל‑Torii Norito-RPC (NRPC-2C)

ראנבוק זה מממש את תוכנית **NRPC-2** בכך שהוא מתאר כיצד לקדם את תעבורת Norito‑RPC
מאימות במעבדת staging לשלב “canary” בפרודקשן. מומלץ לקרוא יחד עם:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (חוזה הפרוטוקול)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## תפקידים וקלטים

| תפקיד | אחריות |
|------|----------------|
| Torii Platform TL | מאשר דלתאות תצורה וחותם על smoke tests. |
| NetOps | מיישם שינויי ingress/envoy ומנטר בריאות pool canary. |
| Observability liaison | מאמת dashboards/alerts ואוסף ראיות. |
| Platform Ops | מוביל את טיקט השינוי, מתאם רהרסל rollback ומעדכן טרקרים. |

ארטיפקטים נדרשים:

- הפאץ' האחרון של `iroha_config` עם `transport.norito_rpc.stage = "canary"` ו‑
  `transport.norito_rpc.allowed_clients` מאוכלס.
- קטע תצורת Envoy/Nginx ששומר `Content-Type: application/x-norito` ומחיל את
  פרופיל ה‑mTLS של לקוחות canary (`defaults/torii_ingress_mtls.yaml`).
- Allowlist של טוקנים (YAML או מניפסט Norito) ללקוחות canary.
- URL Grafana + API token עבור `dashboards/grafana/torii_norito_rpc_observability.json`.
- גישה ל‑smoke harness של פריטי‑Parity
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) ולסקריפט drill התראות
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Checklist לפני‑טיסה

1. **אישור הקפאת spec.** ודאו שה‑hash של `docs/source/torii/nrpc_spec.md` תואם לריליס
   החתום האחרון ושאין PRs ממתינים שמשנים את header/layout של Norito.
2. **אימות תצורה.** הריצו
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   כדי לוודא שה‑`transport.norito_rpc.*` נפרס כראוי.
3. **קפסים לסכימה.** הגדירו `torii.preauth_scheme_limits.norito_rpc` בצורה שמרנית
   (למשל 25 חיבורים מקבילים) כדי שהקריאות הבינאריות לא יחנקו תעבורת JSON.
4. **Rehearsal ל‑ingress.** החילו את פאץ' Envoy ב‑staging, הריצו את הבדיקה השלילית
   (`cargo test -p iroha_torii -- norito_ingress`) וודאו ש‑headers חסרים נדחים עם HTTP 415.
5. **בדיקת טלמטריה.** ב‑staging הריצו `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` וצרפו את חבילת הראיות שנוצרה.
6. **מלאי טוקנים.** ודאו שה‑allowlist של canary כולל לפחות שני אופרטורים לכל אזור;
   שמרו את המניפסט ב‑`artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** פתחו טיקט שינוי עם חלון התחלה/סיום, תוכנית rollback וקישורים
   לראנבוק זה ולראיות הטלמטריה.

## תהליך קידום Canary

1. **החלת פאץ' תצורה.**
   - פרסו את דלתת `iroha_config` (stage=`canary`, allowlist מאוכלס, גבולות סכימה מוגדרים)
     דרך admission.
   - בצעו restart או hot‑reload ל‑Torii ואמתו שהפאץ' נקלט דרך לוגי `torii.config.reload`.
2. **עדכון ingress.**
   - פרסו את תצורת Envoy/Nginx שמפעילה routing של Norito וה‑mTLS לפרופיל canary.
   - ודאו ש‑`curl -vk --cert <client.pem>` מחזיר את כותרות Norito `X-Iroha-Error-Code` כשצריך.
3. **Smoke tests.**
   - הריצו `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     מה‑canary bastion. שמרו תמלילי JSON + Norito תחת
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - רשמו hashes ב‑`docs/source/torii/norito_rpc_stage_reports.md`.
4. **תצפית טלמטריה.**
   - עקבו אחר `torii_active_connections_total{scheme="norito_rpc"}` ו‑
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` לפחות 30 דקות.
   - ייצאו את דשבורד Grafana דרך API וצרפו לטיקט השינוי.
5. **Drill התראות.**
   - הריצו `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` כדי
     להזריק מעטפות Norito שגויות; ודאו ש‑Alertmanager רושם אירוע סינתטי ומסיים אותו אוטומטית.
6. **איסוף ראיות.**
   - עדכנו `docs/source/torii/norito_rpc_stage_reports.md` עם:
     - Digest תצורה
     - Hash למניפסט allowlist
     - Timestamp של smoke test
     - Checksum ל‑Grafana export
     - ID של drill התראות
   - העלו ארטיפקטים ל‑`artifacts/norito_rpc/<YYYYMMDD>/`.

## ניטור וקריטריוני יציאה

להישאר ב‑canary עד שכל התנאים הבאים מתקיימים למשך ≥72 שעות:

- שיעור שגיאה (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % וללא
  קפיצות מתמשכות ב‑`torii_norito_decode_failures_total`.
- פריטי‑Latency (`p95` Norito לעומת JSON) בתוך 10 %.
- דשבורד ההתראות שקט מלבד drills מתוזמנים.
- אופרטורים ב‑allowlist מספקים דוחות פריטי ללא mismatch של סכימה.

תעדו סטטוס יומי בטיקט השינוי וצרו snapshots ב‑`docs/source/status/norito_rpc_canary_log.md` (אם קיים).

## תהליך rollback

1. החזירו את `transport.norito_rpc.stage` ל‑`"disabled"` ונקו `allowed_clients`; החילו דרך admission.
2. הסירו את route/mTLS stanza מ‑Envoy/Nginx, רעננו פרוקסים וודאו שחיבורים חדשים של Norito נדחים.
3. בטלו טוקני canary (או השביתו bearer credentials) כדי לסגור סשנים קיימים.
4. עקבו אחרי `torii_active_connections_total{scheme="norito_rpc"}` עד לאפס.
5. הריצו מחדש את ה‑JSON‑only smoke harness כדי לוודא תפקוד בסיסי.
6. צרו stub post‑mortem תחת `docs/source/postmortems/norito_rpc_rollback.md`
   בתוך 24 שעות ועדכנו את טיקט השינוי עם סיכום השפעה + מטריקות.

## סגירה לאחר Canary

כאשר קריטריוני היציאה מתקיימים:

1. עדכנו `docs/source/torii/norito_rpc_stage_reports.md` עם המלצת GA.
2. הוסיפו ערך ב‑`status.md` שמסכם תוצאות canary וחבילות ראיות.
3. הודיעו ל‑SDK leads להעביר fixtures של staging ל‑Norito לבדיקות parity.
4. הכינו פאץ' תצורה ל‑GA (stage=`ga`, הסרת allowlist) ותזמנו את הקידום לפי NRPC-2.

הקפדה על ראנבוק זה מבטיחה איסוף ראיות אחיד, rollback דטרמיניסטי ועמידה בקריטריוני NRPC-2.

</div>
