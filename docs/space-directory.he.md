---
lang: he
direction: rtl
source: docs/space-directory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c94fe3437675143bc62c57f9e1eaf27c0ccaae72cea9b6271c9007df19d19136
source_last_modified: "2025-12-06T09:39:38.926713+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/space-directory.md -->

# מדריך תפעול Space Directory

מדריך זה מסביר כיצד לכתוב, לפרסם, לבצע ביקורת ולסובב רשומות **Space Directory**
עבור dataspaces של Nexus. הוא משלים את הערות הארכיטקטורה ב-
`docs/source/nexus.md` ואת תוכנית ההטמעה של CBDC
(`docs/source/cbdc_lane_playbook.md`) באמצעות נהלים מעשיים, fixtures ותבניות
ממשל.

> **Scope.** ה-Space Directory משמש כרישום הקנוני של manifests ל-dataspace,
> מדיניות יכולות UAID (Universal Account ID), ותיעוד הביקורת עליו רגולטורים
> מסתמכים. בעוד שחוזה הרקע עדיין בפיתוח פעיל (NX-15), ה-fixtures והנהלים
> המפורטים כאן מוכנים לשילוב בכלים ובבדיקות אינטגרציה.

## 1. מושגי יסוד

| מונח | תיאור | הפניות |
|------|---------|------------|
| Dataspace | הקשר ביצוע/Lane שמריץ סט חוזים שאושר בממשל. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (hash מסוג blake2b-32) שמשמש לעיגון הרשאות בין dataspaces. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` המתאר כללי Allow/Deny דטרמיניסטיים עבור זוג UAID/dataspace (Deny מנצח). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | מטא-דטה של ממשל + DA שמפורסם לצד manifests כך שאופרטורים יוכלו לשחזר סטים של ולידטורים, רשימות קומפוזביליות ואודיִט הוקים. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | אירועים בקידוד Norito שנפלטים כאשר manifests מופעלים/פגים/מבוטלים. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. מחזור חיי Manifest

ה-Space Directory אוכף **ניהול מחזור חיים מבוסס epoch**. כל שינוי מייצר חבילת
manifest חתומה יחד עם אירוע:

| אירוע | טריגר | פעולות נדרשות |
|-------|---------|----------------|
| `ManifestActivated` | manifest חדש מגיע ל-`activation_epoch`. | להפיץ את החבילה, לעדכן מטמונים, לארכב אישור ממשל. |
| `ManifestExpired` | `expiry_epoch` עבר ללא חידוש. | להודיע לאופרטורים, לנקות UAID handles, להכין manifest חלופי. |
| `ManifestRevoked` | החלטת Deny דחופה לפני תאריך התוקף. | לבטל UAID מיידית, להפיק דוח אירוע, לתזמן סקירת ממשל המשך. |

Subscribers צריכים להשתמש ב-`DataEventFilter::SpaceDirectory` כדי לעקוב אחרי
dataspaces או UAIDs ספציפיים. דוגמה לפילטר (Rust):

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. זרימת עבודה לאופרטורים

| שלב | בעלי תפקיד | צעדים | ראיות |
|-------|----------|-------|----------|
| Draft | בעל dataspace | לשכפל fixture, לערוך הרשאות/ממשל, להריץ `cargo test -p iroha_data_model nexus::manifest`. | diff ב-git, לוג בדיקות. |
| Review | Governance WG | לאמת JSON של manifest + Norito bytes, לחתום על יומן החלטות. | פרוטוקולים חתומים, hash של manifest (BLAKE3 + Norito `.to`). |
| Publish | מפעילי lane | לפרסם באמצעות CLI (`iroha app space-directory manifest publish`) תוך שימוש ב-Norito `.to` או JSON גולמי **או** POST `/v1/space-directory/manifests` עם JSON של manifest + סיבה אופציונלית, לאמת תגובת Torii, וללכוד `SpaceDirectoryEvent`. | קבלה מ-CLI/Torii, לוג אירועים. |
| Expire | מפעילי lane / Governance | להריץ `iroha app space-directory manifest expire` (UAID, dataspace, epoch) כאשר manifest מגיע לסוף החיים המתוכנן, לוודא `SpaceDirectoryEvent::ManifestExpired`, ולארכב ראיות ניקוי binding. | פלט CLI, לוג אירועים. |
| Revoke | Governance + מפעילי lane | להריץ `iroha app space-directory manifest revoke` (UAID, dataspace, epoch, reason) **או** POST `/v1/space-directory/manifests/revoke` עם אותו payload ל-Torii, לוודא `SpaceDirectoryEvent::ManifestRevoked`, ולעדכן חבילת ראיות. | קבלה מ-CLI/Torii, לוג אירועים, הערת כרטיס. |
| Monitor | SRE/Compliance | לנטר טלמטריה ולוגים של ביקורת, להגדיר התראות ל-revocation/expiry. | צילום Grafana, לוגים מאוחסנים. |
| Rotate/Revoke | מפעילי lane + Governance | להכין manifest חלופי (epoch חדש), לבצע tabletop, לתעד incident (אם בוטל). | כרטיס רוטציה, פוסטמורטם. |

כל הארטיפקטים עבור rollout נשמרים תחת `artifacts/nexus/<dataspace>/<timestamp>/`
יחד עם manifest של checksum כדי לענות על דרישות הראיות של רגולטורים.

### 3.1 אוטומציית Audit bundle

השתמשו ב-`iroha app space-directory manifest audit-bundle` כדי להרכיב חבילת ראיות
עבור כל capability manifest. הכלי מקבל JSON או payload של Norito, יחד עם JSON
של פרופיל dataspace, ומוציא חבילה עצמאית:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z \
  --notes "CBDC -> wholesale rotation drill"
```

הפקודה כותבת `manifest.json`, את הבייטים של Norito (`manifest.to`), ואת ה-hash
(`manifest.hash`), מעתיקה את פרופיל ה-dataspace, ומייצרת `audit_bundle.json`
שמסכם UAID, מזהה dataspace, epoch הפעלה/תוקף, hash של manifest, audit hooks,
חותמת זמן של יצירה, והערות אופציונליות. פרופילים שחסרים `audit_hooks.events`
או ששוכחים להירשם ל-`SpaceDirectoryEvent.ManifestActivated` ול-
`SpaceDirectoryEvent.ManifestRevoked` נדחים מראש כדי שהקומפליינס יוכל להסתמך
על החבילה ללא lint ידני. הניחו את תיקיית החבילה תחת
`artifacts/nexus/<dataspace>/<stamp>/` כדי שחבילת הרגולטור תכלול את אותם בייטים
שה-CLI הפיק.

### 3.2 שלד Manifest ו-Profile

פריט NX-16 ברודמאפ דורש שלדים דטרמיניסטיים ל-manifest/profile כדי שאופרטורים
ואוטומציית SDK יוכלו לאתחל חבילות יכולות UAID ללא עריכת fixtures ידנית. השתמשו
ב-`iroha app space-directory manifest scaffold` כדי להפיק זוג תבניות JSON (manifest
ו-dataspace profile) שכבר עומדות בסכמת Space Directory ונרשמות ל-audit hooks
הנדרשים:

```bash
iroha app space-directory manifest scaffold \
  --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
  --dataspace 11 \
  --activation-epoch 4097 \
  --manifest-out artifacts/nexus/cbdc/scaffold/manifest.json \
  --profile-out artifacts/nexus/cbdc/scaffold/profile.json \
  --allow-program cbdc.transfer \
  --allow-method transfer \
  --allow-asset cbdc#centralbank \
  --allow-role initiator \
  --allow-max-amount 500000000 \
  --allow-window per-day \
  --deny-program cbdc.kit \
  --deny-method withdraw \
  --deny-reason "Withdrawals disabled for this UAID." \
  --profile-governance-issuer parliament@cbdc \
  --profile-governance-ticket gov-2026-02-rotation \
  --profile-validator cbdc-validator-1@cbdc \
  --profile-validator cbdc-validator-2@cbdc \
  --profile-da-attester da-attester-1@cbdc
```

הפקודה כותבת `manifest.json` ו-`profile.json` (ברירת מחדל:
`artifacts/space_directory/scaffold/<dataspace>_<activation>/`) ומדפיסה את
הנתיבים כך ש-playbooks של ריליס יוכלו לקשר ישירות לתוצרים שנוצרו. ספקו דגלי
allow/deny אופציונליים כדי לאכלס כללים מראש; שדות שלא הוגדרו שומרים על אותם
placeholders כמו ה-fixtures המכוּרָתים, ושני הפלטים כוללים את ה-hookים המחייבים
של `SpaceDirectoryEvent.ManifestActivated/Revoked` כך שהם עוברים את אותן בדיקות
אימות כמו manifests ייצוריים. ערכו את הקבצים שנוצרו במקום, הריצו שוב
`manifest encode/audit-bundle` עבור ראיות, והוסיפו אותם לבקרת גרסאות אחרי אישור
ממשל לקשר UAID-dataspace.

## 4. תבנית Manifest ו-Fixtures

השתמשו ב-fixtures המכוּרָתים כייחוס סכמטי קנוני. הדוגמה של CBDC wholesale
(`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) כוללת גם
רשומות allow וגם deny:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

כללים מרכזיים:

- **Deny מנצח.** הציבו deny מפורשים אחרי allow תואם כדי שאופרטורים יוכלו להבין
  את קדימות הכללים.
- **סכומים דטרמיניסטיים.** שמרו על `max_amount` כמחרוזת דצימלית כדי להימנע
  מעמימות של float כאשר `Numeric` מפרש את הערך.
- **תוקף מגובה Scheduler.** ה-runtime הליבתי מפוגג manifests אוטומטית לאחר
  שהגובה מגיע ל-`expiry_epoch`, מפיק `SpaceDirectoryEvent::ManifestExpired`,
  מעלה את `nexus_space_directory_revision_total`, וקושר מחדש UAIDs לפני ש-
  Torii/CLI משקפים את העדכון. השתמשו ב-CLI רק עבור overrides ידניים או ראיות
  הדורשות רישום פג תוקף בדיעבד.
- **שערי Epoch.** השתמשו ב-`activation_epoch` + `expiry_epoch` כדי לתאר קדנציות
  רוטציה. ביטולים דחופים מפיקים `ManifestRevoked`.

ה-manifest של אפליקציית retail dApp נמצא תחת
`fixtures/space_directory/capability/retail_dapp_access.manifest.json` לצורך
תרחישי קומפוזביליות.

לכל capability manifest מצורפת תאומת Norito מקודדת באותה תיקייה (לדוגמה,
`fixtures/space_directory/capability/cbdc_wholesale.manifest.to` עם digest BLAKE3
`11a47182ab51e845d53f40f12387caef1e609585a824c0a4feab38f0922859fe`). קבצי
`.to` נוצרים עם `cargo xtask space-directory encode`, והבדיקה
`cbdc_capability_manifests_enforce_policy_semantics` מאמתת ש-JSON וה-payload
של Norito נשארים נעולים זה לזה.

</div>
