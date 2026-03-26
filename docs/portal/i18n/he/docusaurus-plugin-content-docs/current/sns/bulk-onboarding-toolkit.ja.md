---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cf8f5b2d560dd72c519f4ff52582bcd8f04fe0f399d6118a5d0d446fcf1c84e2
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: bulk-onboarding-toolkit
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/bulk_onboarding_toolkit.md` כדי שמפעילים חיצוניים
יראו את אותה הנחיה של SN-3b בלי לשכפל את המאגר.
:::

# ערכת Bulk Onboarding ל-SNS (SN-3b)

**הפניה לרודמאפ:** SN-3b "Bulk onboarding tooling"  
**ארטיפקטים:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

registrars גדולים נוטים להכין מראש מאות רישומים של `.sora` או `.nexus` עם אותן
אישורי ממשל ותעלות settlement. בניית payloads JSON ידנית או הרצת ה-CLI שוב לא
משתלמת בקנה מידה, ולכן SN-3b מספק builder דטרמיניסטי מ-CSV ל-Norito שמכין
מבני `RegisterNameRequestV1` עבור Torii או ה-CLI. העוזר מאמת כל שורה מראש,
מפיק גם manifest מצטבר וגם JSON אופציונלי מפוצל שורות, ויכול לשלוח payloads
אוטומטית תוך רישום קבלות מובנות לצורכי ביקורת.

## 1. סכמת CSV

הפרסר דורש שורת כותרת הבאה (הסדר גמיש):

| עמודה | נדרש | תיאור |
|-------|------|-------|
| `label` | כן | תווית מבוקשת (מותר mixed case; הכלי מנרמל לפי Norm v1 ו-UTS-46). |
| `suffix_id` | כן | מזהה סיומת מספרי (עשרוני או `0x` hex). |
| `owner` | כן | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | כן | מספר שלם `1..=255`. |
| `payment_asset_id` | כן | נכס settlement (לדוגמה `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | כן | מספרים שלמים ללא סימן המייצגים יחידות נכס טבעיות. |
| `settlement_tx` | כן | ערך JSON או מחרוזת מילולית המתארת את טרנזקציית התשלום או hash. |
| `payment_payer` | כן | AccountId שאישר את התשלום. |
| `payment_signature` | כן | JSON או מחרוזת מילולית המכילה הוכחת חתימת steward או treasury. |
| `controllers` | אופציונלי | רשימה מופרדת בנקודה-פסיק או פסיק של כתובות חשבון controller. ברירת מחדל `[owner]` כאשר חסר. |
| `metadata` | אופציונלי | JSON inline או `@path/to/file.json` המספק רמזי resolver, רשומות TXT וכו'. ברירת מחדל `{}`. |
| `governance` | אופציונלי | JSON inline או `@path` המצביע ל-`GovernanceHookV1`. `--require-governance` מחייב את העמודה הזו. |

כל עמודה יכולה להפנות לקובץ חיצוני על ידי קידומת `@` לערך התא.
הנתיבים נפתרים יחסית לקובץ ה-CSV.

## 2. הרצת העוזר

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

אפשרויות מרכזיות:

- `--require-governance` דוחה שורות ללא hook ממשל (שימושי למכרזים premium או
  הקצאות שמורות).
- `--default-controllers {owner,none}` קובע אם תאי controllers ריקים יחזרו לחשבון
  owner.
- `--controllers-column`, `--metadata-column`, ו-`--governance-column` מאפשרים
  לשנות את שמות העמודות האופציונליות בעבודה עם exports חיצוניים.

בהצלחה הסקריפט כותב manifest מצטבר:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

אם `--ndjson` סופק, כל `RegisterNameRequestV1` נכתב גם כמסמך JSON בשורה אחת כדי
שאוטומציות יזרימו בקשות ישירות ל-Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. שליחות אוטומטיות

### 3.1 מצב Torii REST

ספקו `--submit-torii-url` יחד עם `--submit-token` או `--submit-token-file` כדי
לדחוף כל רשומה במניפסט ישירות ל-Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- העוזר שולח `POST /v1/sns/names` לכל בקשה ומפסיק בשגיאת HTTP הראשונה.
  התגובות נצמדות ללוג כרשומות NDJSON.
- `--poll-status` מבצע שאילתה חוזרת ל-`/v1/sns/names/{namespace}/{literal}` אחרי כל
  שליחה (עד `--poll-attempts`, ברירת מחדל 5) כדי לאשר שהרשומה נראית. ספקו
  `--suffix-map` (JSON ממפה `suffix_id` לערכי "suffix") כדי שהכלי יגזור
  ליטרלים `{label}.{suffix}` לצורך polling.
- פרמטרים: `--submit-timeout`, `--poll-attempts`, ו-`--poll-interval`.

### 3.2 מצב iroha CLI

כדי להעביר כל רשומת manifest דרך ה-CLI, ספקו את נתיב הבינארי:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- controllers חייבים להיות מסוג `Account` (`controller_type.kind = "Account"`)
  כי ה-CLI כרגע חושף רק controllers מבוססי חשבון.
- blobs של metadata ו-governance נכתבים לקבצים זמניים לכל בקשה ומועברים ל-
  `iroha sns register --metadata-json ... --governance-json ...`.
- stdout ו-stderr של ה-CLI יחד עם קודי יציאה נרשמים; קודים שאינם אפס עוצרים
  את הריצה.

שני מצבי השליחה יכולים לפעול יחד (Torii ו-CLI) כדי להצליב פריסות registrar או
לתרגל fallbacks.

### 3.3 קבלות שליחה

כאשר `--submission-log <path>` מסופק, הסקריפט מצרף רשומות NDJSON הכוללות:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

תגובות Torii מוצלחות כוללות שדות מובנים שמופקים מ-`NameRecordV1` או
`RegisterNameResponseV1` (למשל `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) כדי שדשבורדים ודוחות ממשל יוכלו לפרש את הלוג בלי טקסט חופשי. צרפו את
הלוג הזה לכרטיסי registrar יחד עם המניפסט לצורך הוכחה ניתנת לשחזור.

## 4. אוטומציית שחרור פורטל המסמכים

תהליכי CI והפורטל קוראים ל-`docs/portal/scripts/sns_bulk_release.sh`, שעוטף את
העוזר ושומר ארטיפקטים תחת `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

הסקריפט:

1. בונה `registrations.manifest.json`, `registrations.ndjson`, ומעתיק את ה-CSV
   המקורי לתיקיית השחרור.
2. שולח את המניפסט באמצעות Torii ו/או ה-CLI (כשמוגדר), וכותב `submissions.log`
   עם הקבלות המובנות לעיל.
3. מפיק `summary.json` שמתאר את השחרור (נתיבים, URL של Torii, נתיב CLI,
   timestamp) כדי שאוטומציית הפורטל תוכל להעלות את החבילה לאחסון ארטיפקטים.
4. מפיק `metrics.prom` (override דרך `--metrics`) עם מוני Prometheus תואמים
   לסך הבקשות, התפלגות סיומות, סך נכסים ותוצאות שליחה. JSON הסיכום מקשר לקובץ.

Workflows מאחסנים את תיקיית השחרור כארטיפקט יחיד, שמכיל כעת כל מה שהממשל צריך
לצורכי ביקורת.

## 5. טלמטריה ודשבורדים

קובץ המדדים שנוצר על ידי `sns_bulk_release.sh` חושף את הסדרות הבאות:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

הזינו את `metrics.prom` ל-sidecar של Prometheus (למשל דרך Promtail או יבוא batch)
כדי לשמור על התאמה בין registrars, stewards ועמיתי ממשל סביב ההתקדמות ההמונית.
לוח Grafana `dashboards/grafana/sns_bulk_release.json` מציג את אותם הנתונים עם
פאנלים לספירה לפי סיומת, נפח תשלומים ויחסי הצלחה/כשל בשליחה. הלוח מסנן לפי
`release` כדי שמבקרים יוכלו לצלול לריצה אחת של CSV.

## 6. ולידציה ומצבי כשל

- **Canonicalisation של label:** הקלטים מנורמלים עם Python IDNA בתוספת lowercase
  ומסנני תווים Norm v1. תוויות לא תקינות נכשלות מהר לפני קריאות רשת.
- **Guardrails מספריים:** suffix ids, term years, ו-pricing hints חייבים להיות
  בתחום `u16` ו-`u8`. שדות תשלום מקבלים מספרים עשרוניים או hex עד `i64::MAX`.
- **Parsing של metadata או governance:** JSON inline מפוענח ישירות; הפניות לקבצים
  נפתרות יחסית למיקום ה-CSV. metadata שאינו אובייקט מייצר שגיאת ולידציה.
- **Controllers:** תאים ריקים מכבדים את `--default-controllers`. ספקו רשימות
  controller מפורשות (למשל `i105...;i105...`) בעת האצלה לגורמים שאינם owner.

כשלונות מדווחים עם מספרי שורה הקשריים (למשל
`error: row 12 term_years must be between 1 and 255`). הסקריפט יוצא עם קוד `1`
במקרי ולידציה וכך עם `2` כאשר נתיב ה-CSV חסר.

## 7. בדיקות ומקוריות

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` מכסה parsing של CSV,
  הפקת NDJSON, enforcement של governance, ונתיבי שליחה דרך CLI או Torii.
- העוזר הוא Python טהור (ללא תלות נוספת) ורץ בכל מקום שבו `python3` זמין.
  היסטוריית הקומיטים מתועדת לצד ה-CLI במאגר הראשי לצורך שחזור.

בהרצות פרודקשן, צרפו את ה-manifest שנוצר ואת חבילת ה-NDJSON לכרטיס registrar כדי
שה-stewards יוכלו לשחזר את ה-payloads המדויקים שנשלחו ל-Torii.
