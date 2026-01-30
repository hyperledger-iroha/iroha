---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fc03e6bd9335385c639041a9cc3b41b4f3191aeefda7a77e03d4225d48c26204
source_last_modified: "2025-11-14T04:43:20.363899+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/soranexus_bootstrap_plan.md`. שמרו על יישור שתי הגרסאות עד שהגרסאות המקומיות יגיעו לפורטל.
:::

# תוכנית Bootstrap ו-Observability של Sora Nexus

## יעדים
- להקים את רשת הוולידטורים/observes של Sora Nexus עם מפתחות ממשל, ממשקי Torii ומעקב קונצנזוס.
- לאמת שירותי ליבה (Torii, קונצנזוס, persistence) לפני הפעלת פריסות piggyback של SoraFS/SoraNet.
- להקים workflows של CI/CD ודשבורדים/התראות observability כדי להבטיח בריאות רשת.

## דרישות מקדימות
- חומר מפתחות ממשל (multisig של המועצה, מפתחות ועדה) זמין ב-HSM או Vault.
- תשתית בסיסית (אשכולות Kubernetes או nodes bare-metal) באזורים ראשיים/משניים.
- תצורת bootstrap מעודכנת (`configs/nexus/bootstrap/*.toml`) המשקפת פרמטרי קונצנזוס עדכניים.

## סביבות רשת
- להפעיל שתי סביבות Nexus עם prefixes שונים:
- **Sora Nexus (mainnet)** - prefix רשת הפקה `nexus`, מארח ממשל קנוני ושירותי piggyback של SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefix רשת staging `testus`, המשקף את תצורת mainnet לצורך בדיקות אינטגרציה ואימות pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- לשמור על קבצי genesis, מפתחות ממשל וטביעות תשתית נפרדות לכל סביבה. Testus משמשת כקרקע בדיקה לכל rollouts של SoraFS/SoraNet לפני קידום ל-Nexus.
- pipelines של CI/CD צריכים לפרוס תחילה ל-Testus, להריץ smoke tests אוטומטיים, ולדרוש קידום ידני ל-Nexus לאחר שהבדיקות עברו.
- חבילות תצורה רפרנסיות נמצאות תחת `configs/soranexus/nexus/` (mainnet) ו-`configs/soranexus/testus/` (testnet), וכל אחת כוללת `config.toml`, `genesis.json` ותיקיות admission של Torii.

## שלב 1 - סקירת תצורה
1. לבצע audit למסמכים קיימים:
   - `docs/source/nexus/architecture.md` (קונצנזוס, פריסת Torii).
   - `docs/source/nexus/deployment_checklist.md` (דרישות תשתית).
   - `docs/source/nexus/governance_keys.md` (נהלי משמורת מפתחות).
2. לוודא שקבצי genesis (`configs/nexus/genesis/*.json`) תואמים ל-roster הוולידטורים הנוכחי ולמשקלי staking.
3. לאשר פרמטרי רשת:
   - גודל ועדת הקונצנזוס ו-quorum.
   - מרווח בלוקים / ספי finality.
   - פורטי שירות Torii ותעודות TLS.

## שלב 2 - פריסת אשכול bootstrap
1. להקצות nodes של וולידטורים:
   - לפרוס מופעי `irohad` (וולידטורים) עם volumes מתמשכים.
   - להבטיח שחוקי firewall מאפשרים תעבורת קונצנזוס ו-Torii בין nodes.
2. להפעיל שירותי Torii (REST/WebSocket) בכל וולידטור עם TLS.
3. לפרוס nodes של observers (קריאה בלבד) לחוסן נוסף.
4. להריץ סקריפטי bootstrap (`scripts/nexus_bootstrap.sh`) להפצת genesis, התחלת קונצנזוס ורישום nodes.
5. להריץ smoke tests:
   - לשלוח טרנזקציות בדיקה דרך Torii (`iroha_cli tx submit`).
   - לוודא ייצור/finality של בלוקים דרך טלמטריה.
   - לבדוק שכפול ledger בין וולידטורים/observers.

## שלב 3 - ממשל וניהול מפתחות
1. לטעון תצורת multisig של המועצה; לוודא שניתן להגיש ולאשר הצעות ממשל.
2. לאחסן בצורה מאובטחת מפתחות קונצנזוס/ועדה; להגדיר גיבויים אוטומטיים עם logging של גישה.
3. להגדיר נהלי סיבוב מפתחות חירום (`docs/source/nexus/key_rotation.md`) ולאמת את ה-runbook.

## שלב 4 - אינטגרצית CI/CD
1. להגדיר pipelines:
   - בניה ופרסום של תמונות validator/Torii (GitHub Actions או GitLab CI).
   - אימות תצורה אוטומטי (lint ל-genesis, אימות חתימות).
   - pipelines לפריסה (Helm/Kustomize) לאשכולות staging ו-prod.
2. לממש smoke tests ב-CI (להרים אשכול זמני ולהריץ את סוויטת הטרנזקציות הקנונית).
3. להוסיף סקריפטי rollback לפריסות כושלות ולתעד runbooks.

## שלב 5 - Observability והתראות
1. לפרוס סטאק ניטור (Prometheus + Grafana + Alertmanager) לכל אזור.
2. לאסוף מדדים מרכזיים:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs דרך Loki/ELK לשירותי Torii וקונצנזוס.
3. דשבורדים:
   - בריאות קונצנזוס (גובה בלוק, finality, סטטוס peers).
   - לטנטיות ושיעור שגיאות של Torii API.
   - טרנזקציות ממשל וסטטוס הצעות.
4. התראות:
   - עצירת ייצור בלוקים (>2 מרווחי בלוק).
   - ירידת מספר peers מתחת ל-quorum.
   - קפיצות בשיעור שגיאות Torii.
   - backlog של תור הצעות הממשל.

## שלב 6 - אימות והעברה
1. להריץ אימות end-to-end:
   - לשלוח הצעת ממשל (לדוגמה, שינוי פרמטר).
   - להעביר אותה באישור המועצה כדי לוודא שצינור הממשל עובד.
   - לבצע diff למצב ה-ledger כדי לוודא עקביות.
2. לתעד runbook ל-on-call (תגובה לאירועים, failover, scaling).
3. לתקשר מוכנות לצוותי SoraFS/SoraNet; לאשר שפריסות piggyback יכולות להצביע על nodes של Nexus.

## Checklist יישום
- [ ] Audit של genesis/configuration הושלם.
- [ ] Nodes של וולידטורים ו-observers נפרסו עם קונצנזוס תקין.
- [ ] מפתחות ממשל נטענו, הצעה נבדקה.
- [ ] Pipelines של CI/CD פועלים (build + deploy + smoke tests).
- [ ] דשבורדי observability פעילים עם התראות.
- [ ] תיעוד handoff נמסר לצוותי downstream.
