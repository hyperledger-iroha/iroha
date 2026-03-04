---
lang: he
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# נהלי חירום ותירגולי rollback

## מטרה

פריט הרודמפ **DOCS-9** דורש playbooks מעשיים ותוכנית חזרות כדי שמפעילי הפורטל יוכלו
להתאושש מכשלי הפצה בלי לנחש. מסמך זה מכסה שלושה אירועים בעלי אות חזק - פריסות כושלות,
הידרדרות רפליקציה והשבתות אנליטיקה - ומתעד תרגולים רבעוניים שמוכיחים כי rollback של alias
ואימות סינתטי ממשיכים לעבוד end to end.

### חומר קשור

- [`devportal/deploy-guide`](./deploy-guide) — workflow של packaging, signing, וקידום alias.
- [`devportal/observability`](./observability) — release tags, analytics ו-probes שמוזכרים בהמשך.
- `docs/source/sorafs_node_client_protocol.md`
  ו [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — טלמטריה של הרגיסטרי וספי הסלמה.
- `docs/portal/scripts/sorafs-pin-release.sh` ו-`npm run probe:*` helpers
  המוזכרים לאורך הצ'קליסטים.

### טלמטריה וכלים משותפים

| Signal / Tool | מטרה |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | מזהה עצירות רפליקציה והפרות SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | מודד עומק backlog והשהיית השלמה לצורך triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | מציג תקלות בצד gateway שמופיעות לעיתים אחרי deploy גרוע. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | probes סינתטיים שמבצעים gate לריליסים ומאמתים rollbacks. |
| `npm run check:links` | gate של קישורים שבורים; בשימוש אחרי כל mitigation. |
| `sorafs_cli manifest submit ... --alias-*` (עטוף ב-`scripts/sorafs-pin-release.sh`) | מנגנון קידום/החזרה של alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | מרכז טלמטריה של refusals/alias/TLS/replication. התראות PagerDuty מפנות לפאנלים הללו כראיה. |

## Runbook - פריסה כושלת או artefact פגום

### תנאי טריגר

- Probes של preview/production נכשלים (`npm run probe:portal -- --expect-release=...`).
- התראות Grafana על `torii_sorafs_gateway_refusals_total` או
  `torii_sorafs_manifest_submit_total{status="error"}` אחרי rollout.
- QA ידני מבחין בנתיבים שבורים או כשלי proxy Try it מיד אחרי promotion של alias.

### הכלה מיידית

1. **הקפאת פריסות:** לסמן את pipeline ה-CI ב-`DEPLOY_FREEZE=1` (input של GitHub workflow)
   או לעצור את Jenkins job כך שלא יצאו artefacts נוספים.
2. **איסוף artefacts:** להוריד `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, ופלט ה-probes של ה-build הכושל כדי שה-rollback
   יתבסס על digests מדויקים.
3. **הודעה לבעלי ענין:** storage SRE, מוביל Docs/DevRel ו-duty officer של governance
   (במיוחד כש-`docs.sora` מושפע).

### הליך rollback

1. לזהות את manifest ה-last-known-good (LKG). ה-workflow של production שומר אותם תחת
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. לקשור מחדש את ה-alias ל-manifest באמצעות shipping helper:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. לרשום את summary של ה-rollback בטיקט האירוע יחד עם digests של manifest LKG ושל manifest הכושל.

### אימות

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` ו-`sorafs_cli proof verify ...`
   (ראו את מדריך ההפצה) כדי לוודא שה-manifest שקודם מחדש עדיין תואם ל-CAR המאוחסן.
4. `npm run probe:tryit-proxy` כדי לוודא ש-proxy Try-It ב-staging חזר.

### אחרי האירוע

1. להפעיל מחדש את pipeline ההפצה רק אחרי שה-root cause ברור.
2. להשלים רשומות "Lessons learned" ב-[`devportal/deploy-guide`](./deploy-guide)
   עם נקודות חדשות במידת הצורך.
3. לפתוח defects ל-suite של בדיקות שנכשלו (probe, link checker, וכו').

## Runbook - הידרדרות רפליקציה

### תנאי טריגר

- התראה: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` למשך 10 דקות.
- `torii_sorafs_replication_backlog_total > 10` למשך 10 דקות (ראו
  `pin-registry-ops.md`).
- governance מדווחת על זמינות alias איטית אחרי release.

### Triage

1. בדקו dashboards של [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) כדי להבין אם
   ה-backlog מרוכז ב-storage class או בצי provider יחיד.
2. בדקו לוגים של Torii עבור `sorafs_registry::submit_manifest` כדי לקבוע אם submissions נכשלים.
3. דגמו בריאות רפליקות עם `sorafs_cli manifest status --manifest ...` (מציג תוצאות לכל provider).

### Mitigation

1. להנפיק מחדש manifest עם מספר רפליקות גבוה יותר (`--pin-min-replicas 7`) באמצעות
   `scripts/sorafs-pin-release.sh` כדי שה-scheduler יפזר עומס על יותר providers. לרשום digest חדש בלוג.
2. אם ה-backlog קשור ל-provider יחיד, להשביתו זמנית דרך replication scheduler
   (מופיע ב-`pin-registry-ops.md`) ולהגיש manifest חדש שמאלץ providers אחרים לרענן alias.
3. כאשר טריות ה-alias חשובה יותר מ-parity הרפליקציה, rebind ל-manifest חם שכבר staged (`docs-preview`),
   ואז לפרסם manifest מעקב אחרי ש-SRE מנקה backlog.

### Recovery & closure

1. לנטר `torii_sorafs_replication_sla_total{outcome="missed"}` כדי לוודא שהמונה מתייצב.
2. לשמור את פלט `sorafs_cli manifest status` כראיה שכל replica חזרה לעמידה בדרישות.
3. לפתוח או לעדכן post-mortem ל-backlog עם צעדים הבאים (provider scaling, tuning של chunker, וכו').

## Runbook - השבתת analytics או telemetria

### תנאי טריגר

- `npm run probe:portal` מצליח אך dashboards מפסיקים לקלוט אירועי `AnalyticsTracker` ליותר מ-15 דקות.
- Privacy review מדווחת על עליה בלתי צפויה ב-dropped events.
- `npm run probe:tryit-proxy` נכשל בנתיבי `/probe/analytics`.

### Response

1. לאמת inputs בזמן build: `DOCS_ANALYTICS_ENDPOINT` ו-
   `DOCS_ANALYTICS_SAMPLE_RATE` ב-release artifact (`build/release.json`).
2. להריץ שוב `npm run probe:portal` עם `DOCS_ANALYTICS_ENDPOINT` שמופנה ל-staging collector
   כדי לאשר שה-tracker עדיין שולח payloads.
3. אם collectors למטה, להגדיר `DOCS_ANALYTICS_ENDPOINT=""` ולבצע rebuild כדי שה-tracker יעצור;
   לתעד את חלון ה-outage בציר הזמן של האירוע.
4. לוודא ש-`scripts/check-links.mjs` עדיין עושה fingerprint ל-`checksums.sha256`
   (השבתת analytics *לא* אמורה לחסום אימות sitemap).
5. לאחר התאוששות collector, להריץ `npm run test:widgets` כדי לבדוק unit tests של analytics helper
   לפני republish.

### אחרי האירוע

1. לעדכן [`devportal/observability`](./observability) עם מגבלות collector חדשות או דרישות sampling.
2. להוציא governance notice אם נתוני analytics נפלו או עברו redaction מחוץ למדיניות.

## תרגילי חוסן רבעוניים

להריץ את שני ה-drills ב-**יום שלישי הראשון בכל רבעון** (Jan/Apr/Jul/Oct)
או מיד אחרי כל שינוי תשתית משמעותי. לשמור artefacts תחת
`artifacts/devportal/drills/<YYYYMMDD>/`.

| תרגיל | שלבים | ראיות |
| ----- | ----- | -------- |
| חזרה על rollback של alias | 1. לשחזר את rollback של "Failed deployment" עם ה-manifest האחרון ב-production.<br/>2. לקשור חזרה ל-production לאחר מעבר probes.<br/>3. לרשום `portal.manifest.submit.summary.json` ולוגים של probes בתיקיית drill. | `rollback.submit.json`, probe output, ו-release tag של החזרה. |
| ביקורת אימות סינתטי | 1. להריץ `npm run probe:portal` ו-`npm run probe:tryit-proxy` נגד production ו-staging.<br/>2. להריץ `npm run check:links` ולארכב `build/link-report.json`.<br/>3. לצרף screenshots/exports של פאנלי Grafana המאשרים הצלחת probes. | לוגי probes + `link-report.json` שמפנה ל-fingerprint של manifest. |

יש להסלים drills שלא בוצעו למנהל Docs/DevRel ולסקירת governance של SRE, מאחר שה-roadmap
דורש הוכחות רבעוניות דטרמיניסטיות שה-alias rollback ו-portal probes נשארים תקינים.

## תיאום PagerDuty ו-on-call

- שירות PagerDuty **Docs Portal Publishing** אחראי להתראות שנוצרות מתוך
  `dashboards/grafana/docs_portal.json`. הכללים `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, ו-`DocsPortal/TLSExpiry` עושים page ל-primary של Docs/DevRel
  עם Storage SRE כ-secondary.
- בעת page, לכלול `DOCS_RELEASE_TAG`, לצרף screenshots של פאנלי Grafana המושפעים,
  ולצרף פלט probe/link-check בהערות incident לפני תחילת mitigation.
- לאחר mitigation (rollback או redeploy), להריץ שוב `npm run probe:portal`,
  `npm run check:links`, ולצלם Grafana snapshots חדשים המראים שהמדדים חזרו לספים.
  לצרף את כל ה-evidence ל-incident של PagerDuty לפני סגירה.
- אם שתי התראות נורות בו זמנית (למשל TLS expiry ועוד backlog), לבצע triage ל-refusals קודם
  (להפסיק publishing), לבצע rollback, ואז לטפל ב-TLS/backlog יחד עם Storage SRE על ה-bridge.
