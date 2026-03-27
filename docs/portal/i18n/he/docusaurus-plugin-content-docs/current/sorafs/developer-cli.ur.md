---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3b189b9fd08acfa2726602385e722bef89a2356262a448f224c63c6d1f8a914e
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-cli
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/cli.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של Sphinx יופסק.
:::

המשטח המאוחד `sorafs_cli` (מסופק על ידי crate `sorafs_car` עם feature `cli` מופעלת) חושף כל שלב הדרוש להכנת ארטיפקטים של SoraFS. השתמשו בקוקבוק הזה כדי לקפוץ ישירות לזרימות נפוצות; שלבו אותו עם ה-pipeline של manifest ועם runbooks של האורקסטרטור להקשר תפעולי.

## אריזת payloads

השתמשו ב-`car pack` כדי להפיק ארכיוני CAR דטרמיניסטיים ותוכניות chunk. הפקודה בוחרת אוטומטית את chunker SF-1 אלא אם מסופק handle.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Handle ברירת המחדל של chunker: `sorafs.sf1@1.0.0`.
- קלטי תיקיות נסרקים בסדר לקסיקוגרפי כדי שה-checksums יישארו יציבים בין פלטפורמות.
- הסיכום JSON כולל digests של payload, מטאדאטה לכל chunk וה-CID השורשי שמוכר על ידי הרישום והאורקסטרטור.

## בניית manifests

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- אפשרויות `--pin-*` ממופות ישירות לשדות `PinPolicy` בתוך `sorafs_manifest::ManifestBuilder`.
- ספקו `--chunk-plan` כאשר תרצו שה-CLI יחישב מחדש את digest ה-SHA3 של chunk לפני ההגשה; אחרת הוא עושה שימוש ב-digest המוטמע בסיכום.
- הפלט JSON משקף את payload של Norito כדי לבצע diffs פשוטים במהלך סקירות.

## חתימת manifests ללא מפתחות ארוכי טווח

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- מקבל tokens inline, משתני סביבה או מקורות מבוססי קבצים.
- מוסיף מטאדאטה של provenance (`token_source`, `token_hash_hex`, digest של chunk) בלי לשמור את ה-JWT הגולמי אלא אם `--include-token=true`.
- עובד היטב ב-CI: שלבו עם OIDC של GitHub Actions באמצעות `--identity-token-provider=github-actions`.

## שליחת manifests ל-Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- מבצע דקוד Norito עבור alias proofs ומוודא שהם תואמים ל-digest של manifest לפני POST ל-Torii.
- מחשב מחדש את digest ה-SHA3 של chunk מהתוכנית כדי למנוע מתקפות mismatch.
- סיכומי התגובה לוכדים סטטוס HTTP, headers ו-payloads של הרישום לצורך ביקורת מאוחרת.

## אימות תוכן CAR ו-proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- בונה מחדש את עץ ה-PoR ומשווה digests של payload מול סיכום ה-manifest.
- לוכד ספירות ומזהים הנדרשים בעת שליחת proofs של רפליקציה לממשל.

## הזרמת טלמטריית proofs

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- פולט פריטי NDJSON לכל proof שמוזרם (כיבוי replay עם `--emit-events=false`).
- מאגד ספירות הצלחה/כשל, היסטוגרמות לטנטיות וכשלים מדוגמים בסיכום JSON כך ש-dashboards יוכלו לשרטט תוצאות בלי לסרוק לוגים.
- מסתיים בקוד לא אפס כאשר ה-gateway מדווח על כשלים או כשאימות ה-PoR המקומי (באמצעות `--por-root-hex`) דוחה proofs. התאימו את הספים עם `--max-failures` ו-`--max-verification-failures` להרצות חזרה.
- תומך ב-PoR כיום; PDP ו-PoTR יעשו שימוש באותה מעטפת כאשר SF-13/SF-14 יגיעו.
- `--governance-evidence-dir` כותב את הסיכום המרונדר, מטאדאטה (timestamp, גרסת CLI, כתובת ה-gateway, digest של manifest) ועותק של ה-manifest לתיקייה שסופקה כדי שחבילות ממשל יוכלו לארכב ראיות proof-stream בלי לשחזר את הריצה.

## מקורות נוספים

- `docs/source/sorafs_cli.md` — תיעוד flags מקיף.
- `docs/source/sorafs_proof_streaming.md` — סכמת טלמטריית proofs ותבנית דשבורד Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — צלילה עמוקה ל-chunking, להרכבת manifest ולניהול CAR.
