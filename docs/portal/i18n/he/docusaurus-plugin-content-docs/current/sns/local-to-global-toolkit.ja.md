---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a76c1fa49afdd9aa5ec0a894547dfbfd12813e90079981a27ecc79d812e07f
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# ערכת כלים לכתובות Local -> Global

עמוד זה משקף את `docs/source/sns/local_to_global_toolkit.md` מה-mono-repo. הוא מרכז CLI helpers ו-runbooks הנדרשים לפריט הדרך **ADDR-5c**.

## סקירה

- `scripts/address_local_toolkit.sh` עוטף את ה-CLI `iroha` כדי להפיק:
  - `audit.json` -- פלט מובנה של `iroha tools address audit --format json`.
  - `normalized.txt` -- I105 (מועדף) / I105 (אפשרות שנייה) literals לכל selector בתחום Local.
- השתמשו בסקריפט יחד עם dashboard ingest לכתובות (`dashboards/grafana/address_ingest.json`)
  וכללי Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) כדי להוכיח שה-cutover של Local-8 /
  Local-12 בטוח. עקבו אחרי פאנלי collision של Local-8 ו-Local-12 וההתראות
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, ו-`AddressInvalidRatioSlo` לפני
  קידום שינויי manifest.
- עיינו ב-[Address Display Guidelines](address-display-guidelines.md) וב-
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) להקשר UX ותגובה לאירועים.

## שימוש

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

אפשרויות:

- `--format I105` ליציאת `sora...` במקום I105.
- `domainless output (default)` להפקת literals ללא domain.
- `--audit-only` לדילוג על שלב ההמרה.
- `--allow-errors` להמשך סריקה גם כשיש שורות פגומות (כמו התנהגות CLI).

הסקריפט כותב את נתיבי ה-artefacts בסוף הריצה. צרפו את שני הקבצים
ל-change-management ticket יחד עם צילום Grafana שמוכיח אפס
זיהויי Local-8 ואפס התנגשויות Local-12 במשך >=30 ימים.

## אינטגרצית CI

1. הריצו את הסקריפט ב-job ייעודי והעלו את הפלטים.
2. חסמו merges כאשר `audit.json` מדווח על Local selectors (`domain.kind = local12`).
   בערך ברירת המחדל `true` (החליפו ל-`false` רק ב-dev/test לאבחון רגרסיות) והוסיפו
   `iroha tools address normalize` ל-CI כדי שניסיונות
   רגרסיה ייכשלו לפני production.

לפרטים נוספים, checklists של evidence ו-release-note snippet שניתן להשתמש בו בהכרזת cutover,
ראו את מסמך המקור.
