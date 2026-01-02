---
lang: he
direction: rtl
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sorafs_ci_sample/README.md -->

# Fixtures לדוגמה ל-SoraFS CI

ספריה זו אורזת ארטיפקטים דטרמיניסטיים שנוצרו מה-payload הדוגמה תחת `fixtures/sorafs_manifest/ci_sample/`. החבילה מדגימה את צינור האריזה והחתימה מקצה לקצה של SoraFS שעליו מסתמכים תהליכי CI.

## מלאי ארטיפקטים

| קובץ | תיאור |
|------|-------|
| `payload.txt` | Payload מקור שהופק ע"י סקריפטי fixture (דוגמת טקסט). |
| `payload.car` | ארכיון CAR שנוצר ע"י `sorafs_cli car pack`. |
| `car_summary.json` | סיכום מ-`car pack` שתופס digests של chunks ומטא-נתונים. |
| `chunk_plan.json` | JSON של תוכנית fetch המתארת טווחי chunks וציפיות ספקים. |
| `manifest.to` | Manifest Norito שנוצר ע"י `sorafs_cli manifest build`. |
| `manifest.json` | תצוגת manifest קריאה לצורך debug. |
| `proof.json` | סיכום PoR שנוצר ע"י `sorafs_cli proof verify`. |
| `manifest.bundle.json` | bundle חתימה keyless שנוצר ע"י `sorafs_cli manifest sign`. |
| `manifest.sig` | חתימת Ed25519 מנותקת התואמת ל-manifest. |
| `manifest.sign.summary.json` | סיכום CLI שנוצר בזמן החתימה (hashes, מטא-נתוני bundle). |
| `manifest.verify.summary.json` | סיכום CLI מתוך `manifest verify-signature`. |

כל ה-digests שמוזכרים בהערות גרסה ובתיעוד מקורם בקבצים אלו. תהליך `ci/check_sorafs_cli_release.sh` מייצר מחדש את אותם ארטיפקטים ומשווה לגרסאות המחויבות.

## יצירת fixtures מחדש

הריצו את הפקודות הבאות משורש המאגר כדי לייצר מחדש את ה-fixtures. הן משקפות את השלבים של `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

אם צעד כלשהו מייצר hashes שונים, חקרו לפני עדכון ה-fixtures. תהליכי CI מסתמכים על פלט דטרמיניסטי כדי לזהות רגרסיות.

## כיסוי עתידי

ככל שפרופילי chunker נוספים ופורמטי proof יבשילו מה-roadmap, ה-fixtures הקנוניים שלהם יתווספו תחת ספריה זו (למשל,
`sorafs.sf2@1.0.0` (ראו `fixtures/sorafs_manifest/ci_sample_sf2/`) או proof streaming של PDP). כל פרופיל חדש יעקוב אחר
אותה מבנה — payload, CAR, plan, manifest, proofs, ו-artefacts חתימה — כדי שאוטומציה downstream תוכל להשוות גרסאות ללא
סקריפט ייעודי.

</div>
