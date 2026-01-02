---
lang: ur
direction: rtl
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-11-20T12:36:56.774738+00:00"
translation_last_reviewed: 2026-01-01
---

# SNS Metrics اور onboarding kit

Roadmap آئٹم **SN-8** میں دو وعدے شامل ہیں:

1. `.sora`, `.nexus`, اور `.dao` کے لیے registrations, renewals, ARPU, disputes اور freeze windows دکھانے والے dashboards شائع کرنا۔
2. ایسا onboarding kit فراہم کرنا تاکہ registrars اور stewards DNS, pricing اور APIs کو مستقل انداز میں جوڑ سکیں، اس سے پہلے کہ کوئی suffix live ہو۔

یہ صفحہ سورس ورژن
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
کی عکاسی کرتا ہے تاکہ بیرونی reviewers اسی طریقہ کار پر عمل کر سکیں۔

## 1. Metric bundle

### Grafana dashboard اور portal embed

- `dashboards/grafana/sns_suffix_analytics.json` کو Grafana (یا کسی اور analytics host) میں
  standard API کے ذریعے import کریں:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- یہی JSON اس پورٹل پیج کے iframe کو بھی چلاتا ہے (دیکھیں **SNS KPI Dashboard**).
  dashboard اپ ڈیٹ کرنے کے بعد `docs/portal` میں
  `npm run build && npm run serve-verified-preview` چلائیں تاکہ Grafana اور embed کی sync تصدیق ہو۔

### Panels اور evidence

| Panel | Metrics | Governance evidence |
|-------|---------|---------------------|
| Registrations & renewals | `sns_registrar_status_total` (success + renewal resolver labels) | ہر suffix کے لیے throughput + SLA tracking۔ |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance registrar manifests کو revenue سے match کر سکتی ہے۔ |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | فعال freezes، arbitration cadence اور guardian load دکھاتا ہے۔ |
| SLA / error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | API regressions کو customer impact سے پہلے نمایاں کرتا ہے۔ |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, `manifest_id` labels کے ساتھ payment metrics | CSV drops کو settlement tickets سے جوڑتا ہے۔ |

ماہانہ KPI review کے دوران Grafana (یا embedded iframe) سے PDF/CSV export کریں
اور اسے `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` میں متعلقہ annex entry کے ساتھ attach کریں۔
stewards بھی `docs/source/sns/reports/` کے تحت exported bundle کا SHA-256 محفوظ کرتے ہیں
(مثال: `steward_scorecard_2026q1.md`) تاکہ audits evidence path کو دوبارہ بنا سکیں۔

### Annex automation

dashboard export سے براہ راست annex فائلیں بنائیں تاکہ reviewers کو consistent digest ملے:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- helper export کا hash بناتا ہے، UID/tags/panel count ریکارڈ کرتا ہے، اور
  `docs/source/sns/reports/.<suffix>/<cycle>.md` میں Markdown annex لکھتا ہے (مثال `.sora/2026-03`).
- `--dashboard-artifact` export کو
  `artifacts/sns/regulatory/<suffix>/<cycle>/` میں کاپی کرتا ہے تاکہ annex canonical evidence path کو reference کرے؛
  اگر out-of-band archive درکار ہو تو `--dashboard-label` استعمال کریں۔
- `--regulatory-entry` regulatory memo کی طرف اشارہ کرتا ہے۔ helper `KPI Dashboard Annex` بلاک
  insert/replace کرتا ہے جس میں annex path، dashboard artefact، digest اور timestamp محفوظ ہوتے ہیں۔
- `--portal-entry` Docusaurus کاپی
  (`docs/portal/docs/sns/regulatory/*.md`) کو sync رکھتا ہے تاکہ reviewers دستی طور پر annex summaries compare نہ کریں۔
- اگر `--regulatory-entry`/`--portal-entry` چھوڑ دیں تو فائل دستی طور پر attach کریں اور Grafana سے PDF/CSV snapshots بھی upload کریں۔
- recurrent exports کے لیے suffix/cycle جوڑوں کو `docs/source/sns/regulatory/annex_jobs.json` میں درج کریں اور
  `python3 scripts/run_sns_annex_jobs.py --verbose` چلائیں۔ helper ہر entry پر چلتا ہے، dashboard export کاپی کرتا ہے
  (default `dashboards/grafana/sns_suffix_analytics.json` اگر specify نہ ہو)، اور ہر regulatory memo (اور دستیاب portal memo) میں annex بلاک refresh کرتا ہے۔
- `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (یا `make check-sns-annex`) چلائیں تاکہ job list order/dedup، ہر memo میں `sns-annex` marker، اور annex stub کی موجودگی ثابت ہو۔ helper `artifacts/sns/annex_schedule_summary.json` لکھتا ہے جو governance packets میں locale/hash summaries کے ساتھ جاتا ہے۔
یہ manual copy/paste ختم کرتا ہے اور SN-8 annex evidence کو consistent رکھتا ہے، ساتھ ہی schedule/marker/localization drift سے CI میں حفاظت کرتا ہے۔

## 2. Onboarding kit components

### Suffix wiring

- Registry schema + selector rules:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  اور [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS skeleton helper:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  اور rehearsal flow جو [gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md) میں بیان ہے۔
- ہر registrar launch کے لیے `docs/source/sns/reports/` میں ایک مختصر نوٹ شامل کریں جس میں selector samples، GAR proofs، اور DNS hashes ہوں۔

### Pricing cheatsheet

| Label length | Base fee (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Suffix coefficients: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Term multipliers: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). Negotiated deviations کو registrar ticket میں درج کریں۔

### Premium auctions vs renewals

1. **Premium pool** -- sealed-bid commit/reveal (SN-3). bids کو `sns_premium_commit_total` سے track کریں اور
   manifest کو `docs/source/sns/reports/` کے تحت publish کریں۔
2. **Dutch reopen** -- grace + redemption ختم ہونے کے بعد 7-day Dutch sale شروع کریں
   جو 10x سے شروع ہو کر روزانہ 15% کم ہوتا ہے۔ manifests کو `manifest_id` سے tag کریں تاکہ
   dashboard progress دکھا سکے۔
3. **Renewals** -- `sns_registrar_status_total{resolver="renewal"}` مانیٹر کریں اور
   autorenew checklist (notifications, SLA, fallback payment rails) کو registrar ticket میں محفوظ کریں۔

### Developer APIs اور automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Bulk helper اور CSV schema:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- مثال کمانڈ:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

KPI dashboard filter میں manifest ID ( `--submission-log` output) شامل کریں تاکہ finance release کے مطابق revenue panels reconcile کر سکے۔

### Evidence bundle

1. Registrar ticket جس میں contacts, suffix scope اور payment rails ہوں۔
2. DNS/resolver evidence (zonefile skeletons + GAR proofs)۔
3. Pricing worksheet + governance-approved overrides۔
4. API/CLI smoke-test artefacts (`curl` samples, CLI transcripts)۔
5. KPI dashboard screenshot + CSV export، monthly annex کے ساتھ منسلک۔

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Dashboard imported | Product Analytics | Grafana API response + dashboard UID |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` submissions log |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

Registrar یا suffix activate کرنے سے پہلے checklist مکمل کریں۔ Signed bundle SN-8 gate کو clear کرتا ہے اور marketplace launches کے review میں auditors کو ایک ہی reference فراہم کرتا ہے۔
