---
lang: he
direction: rtl
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/sorafs_ci.md -->

---
title: ספר מתכונים ל-CI של SoraFS
summary: דוגמת workflow של GitHub Actions שמאגדת חתימה ואימות עם הערות סקירה.
---

# ספר מתכונים ל-CI של SoraFS

הקטע הזה משקף את ההנחיות ב-`docs/source/sorafs_ci_templates.md` ומדגים כיצד לשלב חתימה,
אימות ובדיקות proof בתוך job יחיד של GitHub Actions.

```yaml
name: sorafs-cli-release

on:
  push:
    branches: [main]

permissions:
  contents: read
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-rust@v1
        with:
          rust-version: 1.92

      - name: Package payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack             --input payload.bin             --car-out artifacts/payload.car             --plan-out artifacts/chunk_plan.json             --summary-out artifacts/car_summary.json
          sorafs_cli manifest build             --summary artifacts/car_summary.json             --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign             --manifest artifacts/manifest.to             --chunk-plan artifacts/chunk_plan.json             --bundle-out artifacts/manifest.bundle.json             --signature-out artifacts/manifest.sig             --identity-token-provider=github-actions             --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature             --manifest artifacts/manifest.to             --bundle artifacts/manifest.bundle.json             --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify             --manifest artifacts/manifest.to             --car artifacts/payload.car             --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## הערות

- `sorafs_cli` חייב להיות זמין על ה-runner (לדוגמה, `cargo install --path crates/sorafs_car --features cli` לפני השלבים האלה).
- ה-workflow חייב לספק audience OIDC מפורש (כאן `sorafs`); התאימו את `--identity-token-audience` למדיניות Fulcio שלכם.
- ה-release pipeline צריך לארכב את `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` ו-`artifacts/proof.json` לצורך סקירת governance.
- artefacts דטרמיניסטיים לדוגמה נמצאים ב-`fixtures/sorafs_manifest/ci_sample`; העתיקו אותם לטסטים כשנדרשים manifests, chunk plans או bundle JSON golden בלי לחשב מחדש את ה-pipeline.

## אימות fixtures

artefacts דטרמיניסטיים ל-workflow הזה נמצאים תחת
`fixtures/sorafs_manifest/ci_sample`. ניתן להריץ מחדש את השלבים למעלה ולהשוות את התוצרים
מול הקבצים הקנוניים, לדוגמה:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Diffs ריקים מאשרים שה-build יצר manifests, plans ו-signature bundles זהים ברמת byte.
ראו `fixtures/sorafs_manifest/ci_sample/README.md` לרשימה מלאה וטיפים על תבניות
release notes מתוך ה-summaries שנלכדו.

</div>
