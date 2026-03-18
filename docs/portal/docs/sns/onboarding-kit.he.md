---
lang: he
direction: rtl
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-11-20T12:36:56.774738+00:00"
translation_last_reviewed: 2026-01-01
---

# מדדי SNS ו-kit onboarding

פריט roadmap **SN-8** כולל שני הבטחות:

1. לפרסם dashboards שמציגים registrations, renewals, ARPU, disputes וחלונות freeze עבור `.sora`, `.nexus`, ו-`.dao`.
2. לספק kit onboarding כדי ש-registrars ו-stewards יחברו DNS, pricing ו-APIs בצורה עקבית לפני השקת כל suffix.

עמוד זה משקף את גרסת המקור
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
כדי שמבקרים חיצוניים יוכלו לעקוב אחרי אותו תהליך.

## 1. Metric bundle

### Dashboard Grafana ו-portal embed

- ייבאו את `dashboards/grafana/sns_suffix_analytics.json` ל-Grafana (או analytics host אחר)
  דרך ה-API הסטנדרטי:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- אותו JSON מזין את ה-iframe בעמוד הזה (ראו **SNS KPI Dashboard**).
  בכל עדכון dashboard הריצו
  `npm run build && npm run serve-verified-preview` בתוך `docs/portal` כדי
  לוודא ש-Grafana וה-embed נשארים מסונכרנים.

### Panels ו-evidence

| Panel | Metrics | Evidence governance |
|-------|---------|---------------------|
| Registrations & renewals | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput לכל suffix + מעקב SLA. |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance יכולה לשדך manifests של registrar להכנסות. |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | מציג freezes פעילים, קצב בוררות ועומס guardian. |
| SLA / error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | מדגיש רגרסיות API לפני פגיעה בלקוחות. |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, metrics תשלום עם labels `manifest_id` | מחבר CSV drops ל-settlement tickets. |

ייצאו PDF/CSV מ-Grafana (או מה-iframe) בזמן סקירת KPI חודשית
וצירפו ל-annex המתאים תחת
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. stewards גם שומרים SHA-256
של ה-bundle המיוצא תחת `docs/source/sns/reports/` (לדוגמה,
`steward_scorecard_2026q1.md`) כדי שאפשר יהיה לשחזר את נתיב ה-evidence.

### אוטומציה של annex

צרו קבצי annex ישירות מ-export של dashboard כדי לספק summary עקבי:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- ה-helper מבצע hash ל-export, אוסף UID/tags/panel count וכותב annex Markdown תחת
  `docs/source/sns/reports/.<suffix>/<cycle>.md` (ראו דוגמת `.sora/2026-03`).
- `--dashboard-artifact` מעתיק את ה-export אל
  `artifacts/sns/regulatory/<suffix>/<cycle>/` כדי שה-annex יפנה לנתיב evidence קנוני.
- `--regulatory-entry` מצביע ל-memo רגולטורי. ה-helper מוסיף (או מחליף)
  בלוק `KPI Dashboard Annex` עם נתיב annex, dashboard artefact, digest ו-timestamp.
- `--portal-entry` מסנכרן את עותק Docusaurus
  (`docs/portal/docs/sns/regulatory/*.md`).
- אם מדלגים על `--regulatory-entry`/`--portal-entry`, צרפו את הקובץ ידנית ועדיין העלו PDF/CSV snapshots.
- עבור exports מחזוריים, רשמו suffix/cycle ב-`docs/source/sns/regulatory/annex_jobs.json`
  והריצו `python3 scripts/run_sns_annex_jobs.py --verbose`.
- הריצו `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports`
  (או `make check-sns-annex`) כדי לאשר סדר/ללא כפילויות, סימון `sns-annex`, ו-stub קיים. ה-helper כותב `artifacts/sns/annex_schedule_summary.json` ליד סיכומי locale/hash לחבילות governance.
זה מסיר שלבי copy/paste ידניים ומונע drift של schedule/marker/localization ב-CI.

## 2. רכיבי onboarding kit

### Suffix wiring

- Registry schema + selector rules:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  ו-`docs/source/sns/local_to_global_toolkit.md`.
- DNS skeleton helper:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  עם rehearsal flow ב-[gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- לכל השקת registrar, שמרו note קצר ב-`docs/source/sns/reports/` עם selector samples, GAR proofs ו-DNS hashes.

### Pricing cheatsheet

| אורך label | Base fee (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Suffix coefficients: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Term multipliers: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). רשמו חריגים מוסכמים ב-registrar ticket.

### Premium auctions vs renewals

1. **Premium pool** -- sealed-bid commit/reveal (SN-3). עקבו אחרי bids דרך
   `sns_premium_commit_total` ופרסמו manifest תחת `docs/source/sns/reports/`.
2. **Dutch reopen** -- אחרי grace + redemption התחילו Dutch sale של 7-day
   ב-10x שיורד 15% ביום. תייגו manifests עם `manifest_id` כדי שה-dashboard יציג התקדמות.
3. **Renewals** -- עקבו אחרי `sns_registrar_status_total{resolver="renewal"}` ותעדו checklist autorenew (notifications, SLA, fallback payment rails) בתוך registrar ticket.

### Developer APIs & automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Bulk helper & CSV schema:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- דוגמת פקודה:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

כללו את manifest ID (פלט `--submission-log`) ב-KPI dashboard filter כדי ש-finance תוכל לשדך revenue panels לפי release.

### Evidence bundle

1. Registrar ticket עם אנשי קשר, scope suffix ו-payment rails.
2. DNS/resolver evidence (zonefile skeletons + GAR proofs).
3. Pricing worksheet + governance-approved overrides.
4. API/CLI smoke-test artefacts (`curl` samples, CLI transcripts).
5. KPI dashboard screenshot + CSV export, מצורף ל-annex החודשי.

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Dashboard imported | Product Analytics | Grafana API response + dashboard UID |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` submissions log |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

סיימו את checklist לפני הפעלת registrar או suffix. bundle חתום משחרר את gate של SN-8 ומספק למבקרים reference יחיד בעת בדיקת השקות marketplace.
