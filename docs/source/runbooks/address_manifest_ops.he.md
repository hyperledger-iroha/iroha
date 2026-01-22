---
lang: he
direction: rtl
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# ראנבוק תפעול מניפסט כתובות (ADDR-7c)

ראנבוק זה מפעיל את פריט ה‑roadmap **ADDR-7c** בכך שהוא מפרט כיצד לאמת, לפרסם
ולהוציא מן השירות רשומות במניפסט החשבונות/כינויים של Sora Nexus. הוא משלים את
החוזה הטכני ב־[`docs/account_structure.md`](../../account_structure.md) §4 ואת
ציפיות הטלמטריה המתועדות ב־`dashboards/grafana/address_ingest.json`.

## 1. היקף וקלטים

| קלט | מקור | הערות |
|-------|--------|-------|
| חבילת מניפסט חתומה (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin של SoraFS (`sorafs://address-manifests/<CID>/`) ומראת HTTPS | החבילות מופקות ע"י אוטומציית ריליס; שמרו על מבנה הספרייה בזמן שיקוף. |
| Digest + רצף של המניפסט הקודם | חבילה קודמת (אותו דפוס נתיב) | נדרש להוכחת מונוטוניות/אי‑שינוי. |
| גישה לטלמטריה | לוח `address_ingest` ב‑Grafana + Alertmanager | נדרש לניטור פרישת Local‑8 וקפיצות של כתובות לא תקינות. |
| כלי עבודה | `cosign`, `shasum`, `b3sum` (או `python3 -m blake3`), `jq`, CLI `iroha`, `scripts/account_fixture_helper.py` | להתקין לפני ביצוע הצ’ק־ליסט. |

## 2. מבנה ארטיפקטים

כל חבילה חייבת לעקוב אחר המבנה הבא; אל תשנו שמות קבצים בעת העתקה בין סביבות.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

שדות כותרת ב־`manifest.json`:

| שדה | תיאור |
|-------|-------------|
| `version` | גרסת הסכמה (נכון לעכשיו `1`). |
| `sequence` | מספר רוויזיה מונוטוני; חייב לעלות בדיוק באחד. |
| `generated_ms` | חותמת זמן UTC לפרסום (מילישניות מאז epoch). |
| `ttl_hours` | אורך חיים מקסימלי של קאש ש‑Torii/SDKs רשאים לכבד (ברירת מחדל 24). |
| `previous_digest` | BLAKE3 של גוף המניפסט הקודם (hex). |
| `entries` | מערך מסודר של רשומות (`global_domain`, `local_alias`, או `tombstone`). |

## 3. נוהל אימות

1. **הורדת החבילה.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Guardrail של checksum.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   כל הקבצים חייבים לדווח `OK`; אי‑התאמות נחשבות לשיבוש.

3. **אימות Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **הוכחת אי‑שינוי.** השוו `sequence` ו־`previous_digest` מול המניפסט
   המאורכב:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   ה‑digest שמודפס חייב להתאים ל־`previous_digest`. קפיצות רצף אסורות; יש
   להנפיק מחדש את המניפסט במקרה של חריגה.

5. **עמידת TTL.** ודאו ש־`generated_ms + ttl_hours` מכסה את חלונות ההפצה
   הצפויים; אחרת על ההנהלה לפרסם מחדש לפני פקיעת הקאש.

6. **תקינות רשומות.**
   - רשומות `global_domain` חייבות לכלול `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - רשומות `local_alias` חייבות לכלול digest באורך 12 בתים שמיוצר ע"י Norm v1
     (אשרו עם `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`;
     סיכום ה‑JSON משקף את הדומיין דרך `input_domain` ו‑`--append-domain` משחזר את
     הקידוד כ‑`<ih58>@<domain>` עבור מניפסטים).
   - רשומות `tombstone` חייבות להפנות ל‑selector המדויק, ולכלול `reason_code`,
     `ticket` ו־`replaces_sequence`.

7. **תאימות fixtures.** הריצו מחדש את הווקטורים הקנוניים וודאו שטבלת ה‑digest
   המקומית לא השתנתה באופן לא צפוי:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Guardrail אוטומציה.** הריצו את מאמת המניפסט כדי לבדוק את החבילה מקצה
   לקצה (סכימת כותרת, צורת רשומות, checksums וחיבור previous‑digest):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   הדגל `--previous` מצביע על החבילה הקודמת כך שהכלי יאשר את המונוטוניות של
   `sequence` ויחשב מחדש את ה‑BLAKE3 של `previous_digest`. הפקודה נכשלת מיד עם
   סטיית checksum או חסר שדות ב‑`tombstone`, לכן צרפו את הפלט לטיקט השינוי לפני
   בקשת חתימות.

## 4. זרימת שינויי alias ו‑tombstone

1. **הצעת שינוי.** פתחו טיקט governance עם קוד סיבה (`LOCAL8_RETIREMENT`,
   `DOMAIN_REASSIGNED`, וכו') וה‑selectors המושפעים.
2. **גזירת payloads קנוניים.** לכל alias שעובד עדכון, הריצו:

   ```bash
   iroha tools address convert snx1...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **טיוטת רשומת מניפסט.** הוסיפו רשומת JSON כמו:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   בעת החלפת alias מקומי ב‑Global, הוסיפו גם `tombstone` וגם את רשומת
   `global_domain` העוקבת עם המבדיל של Nexus.

4. **אימות החבילה.** הריצו מחדש את שלבי האימות מול הטיוטה לפני בקשת חתימות.
5. **פרסום ומעקב.** לאחר חתימת governance, פעלו לפי §3 והשאירו את דגל
   `true` ב‑production לאחר שהמדדים מאשרים אפס שימוש ב‑Local‑8. העבירו ל‑`false`
   רק ב‑dev/test כאשר צריך זמן soak נוסף.

## 5. ניטור ו‑rollback

- Dashboards: `dashboards/grafana/address_ingest.json` (פאנלים עבור
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`,
  `torii_address_collision_domain_total{endpoint,domain}`) חייבים להישאר ירוקים
  30 ימים לפני חסימת Local‑8/Local‑12 לצמיתות.
- הוכחת gating: יצאו טווח 30 יום מ‑Prometheus עבור `torii_address_local8_total`
  ו‑`torii_address_collision_total` (למשל `promtool query range --output=json ...`) והריצו
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`;
  צרפו את ה‑JSON + פלט CLI לטיקטי rollout כדי שה‑governance תראה את חלון הכיסוי
  ותאשר שהמונה נשאר שטוח.
- התראות (ראו `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — מפעיל paging בכל עלייה חדשה של Local‑8. התייחסו
    כאל חסם ריליס, עצרו rollouts של strict‑mode וקבעו זמנית
    כשהטלמטריה נקייה.
  - `AddressLocal12Collision` — נורה מייד כששתי תוויות Local‑12 מתחככות לאותו digest.
    עצרו קידומי מניפסט, הריצו `scripts/address_local_toolkit.sh` לאימות המיפוי,
    ותאמו עם governance Nexus לפני הוצאה מחדש של הרשומה.
  - `AddressInvalidRatioSlo` — אזהרה כאשר שליחות IH58/דחוסות לא תקינות
    (למעט דחיות Local‑8/strict‑mode) עוברות 0.1 % SLO ל‑10 דקות. בדקו
    `torii_address_invalid_total` לפי הקשר/סיבה ותאמו עם צוות ה‑SDK לפני החזרת strict‑mode.
- לוגים: שמרו שורות `manifest_refresh` של Torii ואת מספר טיקט ה‑governance ב‑`notes.md`.
- Rollback: פרסמו מחדש את החבילה הקודמת (אותם קבצים, טיקט חדש המצביע על rollback)
  ואז החזירו ל‑`true`.

## 6. מקורות

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (חוזה).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (סנכרון fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (digests קנוניים).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (טלמטריה).

</div>
