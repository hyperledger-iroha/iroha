---
lang: he
direction: rtl
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sorafs_release_notes.md -->

# הערות שחרור SoraFS CLI ו-SDK (v0.1.0)

## נקודות עיקריות
- `sorafs_cli` מכסה כעת את כל צינור האריזה (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) כך ש-Runners של CI מפעילים
  בינארי אחד במקום helpers ייעודיים. זרימת חתימה ללא מפתחות משתמשת כברירת מחדל ב-
  `SIGSTORE_ID_TOKEN`, מבינה ספקי OIDC של GitHub Actions ומפיקה סיכום JSON דטרמיניסטי
  לצד חבילת החתימות.
- *scoreboard* של fetch רב-מקורות מסופק כחלק מ-`sorafs_car`: הוא מנרמל טלמטריה של ספקים,
  אוכף קנסות יכולת, משמר דוחות JSON/Norito ומזין את סימולטור האורקסטרטור (`sorafs_fetch`)
  דרך registry handle משותף. Fixtures תחת `fixtures/sorafs_manifest/ci_sample/` מדגימים
  את הקלטים והפלטים הדטרמיניסטיים ש-CI/CD אמור להשוות מולם.
- אוטומציית השחרור מקודדת ב-`ci/check_sorafs_cli_release.sh` וב-`scripts/release_sorafs_cli.sh`.
  כל שחרור מאחסן כעת את manifest bundle, החתימה, סיכומי `manifest.sign/verify` ו-snapshot של
  scoreboard כדי ש-reviewers של governance יוכלו לעקוב אחר artefacts בלי להריץ מחדש את הצינור.

## תאימות
- שינויים שוברים: **אין.** כל ההרחבות ל-CLI הן דגלים/תתי-פקודות אדיטיביים; קריאות קיימות
  ממשיכות לעבוד ללא שינוי.
- גרסאות מינימום gateway/node: נדרש Torii `2.0.0-rc.2.0` (או חדש יותר) כדי ש-API של chunk-range,
  מכסות stream-token וכותרות יכולת שמפורסמות ע"י `crates/iroha_torii` יהיו זמינות. צמתי אחסון
  חייבים להריץ את SoraFS host stack מה-commit
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (כולל קלטים חדשים של scoreboard וחיווט טלמטריה).
- תלות upstream: אין עדכוני צד שלישי מעבר לבסיס ה-workspace; השחרור עושה שימוש חוזר בגרסאות
  המקובעות של `blake3`, `reqwest` ו-`sigstore` ב-`Cargo.lock`.

## שלבי שדרוג
1. עדכן את ה-crates המתואמים ב-workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. הרץ מחדש את release gate מקומית (או ב-CI) כדי לאשר כיסוי fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. הפק מחדש artefacts חתומים וסיכומים עם הקונפיג שנבחר:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   העתק bundles/proofs מעודכנים אל `fixtures/sorafs_manifest/ci_sample/` אם השחרור
   מעדכן fixtures קנוניים.

## אימות
- Commit של release gate: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` מיד לאחר שה-gate הצליח).
- פלט `ci/check_sorafs_cli_release.sh`: נשמר ב-
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (מצורף ל-release bundle).
- Digest של manifest bundle: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Digest של proof summary: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Digest של manifest (ל-cross-checks של attestation downstream):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (מתוך `manifest.sign.summary.json`).

## הערות למפעילים
- ה-gateway של Torii אוכף כעת את capability header `X-Sora-Chunk-Range`. עדכנו allowlists כדי
  לאפשר לקוחות שמציגים scopes חדשים של stream token; טוקנים ישנים ללא range claim ייחסמו.
- `scripts/sorafs_gateway_self_cert.sh` משלב אימות manifest. בעת הרצת harness self-cert,
  ספקו manifest bundle שנוצר זה עתה כדי שה-wrapper יכשל במהירות במקרה של signature drift.
- לוחות טלמטריה צריכים ingest את export החדש של scoreboard (`scoreboard.json`) כדי ליישר
  זכאות ספקים, הקצאת משקלים וסיבות סירוב.
- ארכבו את ארבעת ה-summaries הקנוניים בכל rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. כרטיסי governance מפנים לקבצים הללו במהלך האישור.

## תודות
- Storage Team - איחוד CLI end-to-end, renderer ל-chunk-plan וחיווט טלמטריה של scoreboard.
- Tooling WG - release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) וחבילת fixtures דטרמיניסטית.
- Gateway Operations - capability gating, סקירת מדיניות stream-token, ו-playbooks מעודכנים
  ל-self-cert.

</div>
