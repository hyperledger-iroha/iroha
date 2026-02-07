---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-bootstrap-plan
title: Sora Nexus بوٹ اسٹریپ اور آبزرویبیلٹی
description: Nexus کے بنیادی validator cluster کو آن لائن لانے سے پہلے SoraFS اور SoraNet خدمات شامل کرنے کا آپریشنل پلان۔
---

:::note דף הבית
یہ صفحہ `docs/source/soranexus_bootstrap_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ورژنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus אתחול ותוכנית צפייה

## اہداف
- ממשקי API של Torii, ניטור קונצנזוס של Sora Nexus מאמת/תצפיתן. کریں۔
- بنیادی سروسز (Torii، consensus، persistence) کو SoraFS/SoraNet piggyback deployments سے پہلے ویلیڈیٹ کریں۔
- זרימות עבודה של CI/CD או לוחות מחוונים/התראות של צפיות.

## پیشگی شرائط
- Governance key material (council multisig، committee keys) HSM یا Vault میں دستیاب ہو۔
- بنیادی انفراسٹرکچر (Kubernetes clusters یا bare-metal nodes) بنیادی/ثانوی ریجنز میں موجود ہو۔
- اپ ڈیٹ شدہ bootstrap configuration (`configs/nexus/bootstrap/*.toml`) جو جدید consensus parameters دکھائے۔

## نیٹ ورک ماحول
- סביבות Nexus עם קידומות קידומות:
- **Sora Nexus (mainnet)** - پروڈکشن نیٹ ورک prefix `nexus` جو canonical governance اور SoraFS/SoraNet piggyback services میزبان بناتا 20 (מזהה שרשרת `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - staging نیٹ ورک prefix `testus` جو mainnet configuration کو integration testing اور pre-release validation کے لئے mirror کرتا ہے (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- ہر environment کے لئے الگ genesis files، governance keys، اور infrastructure footprints رکھیں۔ Testus تمام SoraFS/SoraNet rollouts کے لئے proving ground ہے، Nexus میں promote کرنے سے پہلے۔
- CI/CD pipelines پہلے Testus پر deploy کریں، automated smoke tests چلائیں، اور checks پاس ہونے پر Nexus میں manual promotion درکار ہو۔
- Reference configuration bundles `configs/soranexus/nexus/` (mainnet) اور `configs/soranexus/testus/` (testnet) کے تحت ہیں، ہر ایک میں نمونہ `config.toml`, `genesis.json` اور Torii admission directories شامل ہیں۔

## مرحلہ 1 - Configuration Review
1. תיעוד וידאו של ביקורת:
   - `docs/source/nexus/architecture.md` (קונצנזוס, פריסת Torii).
   - `docs/source/nexus/deployment_checklist.md` (דרישות אינפרא).
   - `docs/source/nexus/governance_keys.md` (נהלי משמורת מפתח).
2. Genesis files (`configs/nexus/genesis/*.json`) کو validate کریں کہ وہ موجودہ validator roster اور staking weights سے align ہیں۔
3. פרמטרים שונים:
   - גודל ועדת קונצנזוס או מניין.
   - מרווח חסימה / ספי סופיות.
   - יציאות שירות Torii או תעודות TLS.## مرحلہ 2 - Bootstrap Cluster Deployment
1. מתן צומת אימות:
   - `irohad` instances (validators) کو persistent volumes کے ساتھ deploy کریں۔
   - חוקי חומת אש של נהריים ותקנות קונצנזוס ב-Torii צמתי תעבורה מותרים 60 .
2. ہر validator پر Torii services (REST/WebSocket) کو TLS کے ساتھ شروع کریں۔
3. اضافی resilience کے لئے observer nodes (read-only) deploy کریں۔
4. Bootstrap scripts (`scripts/nexus_bootstrap.sh`) چلائیں تاکہ genesis تقسیم ہو، consensus شروع ہو، اور nodes رجسٹر ہوں۔
5. בדיקות עשן:
   - Torii کے ذریعے test transactions submit کریں (`iroha_cli tx submit`).
   - Telemetry کے ذریعے block production/finality verify کریں۔
   - Validators/observers کے درمیان ledger replication چیک کریں۔

## مرحلہ 3 - Governance اور Key Management
1. Council multisig configuration لوڈ کریں؛ تصدیق کریں کہ governance proposals submit اور ratify ہو سکتی ہیں۔
2. מפתחות קונצנזוס/וועדה. access logging کے ساتھ automatic backups configure کریں۔
3. Emergency key rotation procedures (`docs/source/nexus/key_rotation.md`) سیٹ اپ کریں اور runbook verify کریں۔

## مرحلہ 4 - CI/CD Integration
1. צינורות מגדירים את ההגדרות:
   - Validator/Torii images build اور publish کریں (GitHub Actions یا GitLab CI).
   - אימות תצורה אוטומטי (יצירת מוך, אימות חתימות).
   - Deployment pipelines (Helm/Kustomize) برائے staging اور production clusters.
2. CI میں smoke tests شامل کریں (ephemeral cluster اٹھائیں، canonical transaction suite چلائیں).
3. ناکام deployments کے لئے rollback scripts شامل کریں اور runbooks دستاویز کریں۔

## مرحلہ 5 - Observability اور Alerts
1. Monitoring stack (Prometheus + Grafana + Alertmanager) ہر region میں deploy کریں۔
2. מדדי ליבה:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii اور consensus services کے لئے Loki/ELK کے ذریعے logs۔
3. לוחות מחוונים:
   - בריאות קונצנזוס (גובה בלוק, סופיות, מצב עמיתים).
   - שיעורי השהיה/שגיאות של Torii API.
   - עסקאות ממשל או סטטוסי הצעות.
4. התראות:
   - דוכן ייצור בלוק (>2 מרווחי בלוק).
   - Peer count quorum سے نیچے گر جائے۔
   - Torii עליות בשיעור השגיאה.
   - צבר תור להצעות ממשל.

## مرحلہ 6 - Validation & Handoff
1. אימות מקצה לקצה:
   - Governance proposal submit کریں (مثلاً parameter change).
   - Council approval کے ذریعے process کریں تاکہ governance pipeline درست ہو۔
   - Ledger state diff چلائیں تاکہ consistency یقینی ہو۔
2. On-call runbook دستاویز کریں (incident response, failover, scaling).
3. SoraFS/SoraNet ٹیموں کو readiness بتائیں؛ تصدیق کریں کہ piggyback deployments Nexus nodes کو point کر سکتے ہیں۔## רשימת רשימת יישום
- [ ] Genesis/configuration audit مکمل۔
- [ ] Validator اور observer nodes deploy ہوئے اور consensus صحت مند ہے۔
- [ ] Governance keys لوڈ ہوئے، proposal test ہوا۔
- [ ] CI/CD pipelines چل رہے ہیں (build + deploy + smoke tests).
- [ ] Observability dashboards فعال ہیں اور alerting موجود ہے۔
- [ ] Handoff documentation downstream ٹیموں کو دے دی گئی۔