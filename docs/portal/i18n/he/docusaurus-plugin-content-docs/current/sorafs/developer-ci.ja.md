---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/developer-ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: caf2508834aae073d914438632db5ae933182df0bee4aaeed9b7e45e40cb0a5b
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: developer-ci
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/ci.md`. שמרו על שתי הגרסאות מסונכרנות עד שהמסמכים הישנים ייצאו משימוש.
:::

# מתכוני CI

pipelines של SoraFS נהנים מ-chunking דטרמיניסטי, חתימת manifest ואימות הוכחות. משטח הפקודות
`sorafs_cli` שומר את השלבים האלה ניידים בין ספקי CI. העמוד הזה מדגיש את המתכונים הקנוניים ומפנה
לתבניות מוכנות לשימוש.

## GitHub Actions (ללא מפתחות)

```yaml
name: sorafs-artifacts

on:
  push:
    branches: [ main ]

jobs:
  build-and-publish:
    runs-on: ubuntu-latest
    permissions:
      id-token: write
      contents: read
    env:
      RUSTFLAGS: "-C target-cpu=native"
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: stable
      - name: Build CLI
        run: cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
      - name: Pack payload and manifest
        run: |
          sorafs_cli car pack \
            --input fixtures/site.tar.gz \
            --car-out artifacts/site.car \
            --plan-out artifacts/site.plan.json \
            --summary-out artifacts/site.car.json
          sorafs_cli manifest build \
            --summary artifacts/site.car.json \
            --chunk-plan artifacts/site.plan.json \
            --manifest-out artifacts/site.manifest.to
      - name: Sign manifest (Sigstore OIDC)
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/site.manifest.to \
            --bundle-out artifacts/site.manifest.bundle.json \
            --signature-out artifacts/site.manifest.sig \
            --identity-token-provider=github-actions
      - name: Submit manifest
        env:
          TORII_URL: https://gateway.example/v1
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest artifacts/site.manifest.to \
            --chunk-plan artifacts/site.plan.json \
            --torii-url "$TORII_URL" \
            --authority ih58... \
            --private-key "$IROHA_PRIVATE_KEY" \
            --summary-out artifacts/site.submit.json
      - name: Stream PoR proofs
        env:
          GATEWAY_URL: https://gateway.example/v1/sorafs/proof/stream
          STREAM_TOKEN: ${{ secrets.SORAFS_STREAM_TOKEN }}
        run: |
          sorafs_cli proof stream \
            --manifest artifacts/site.manifest.to \
            --gateway-url "$GATEWAY_URL" \
            --provider-id provider::alpha \
            --samples 64 \
            --stream-token "$STREAM_TOKEN" \
            --summary-out artifacts/site.proof_stream.json
      - uses: actions/upload-artifact@v4
        with:
          name: sorafs-artifacts
          path: artifacts/
```

נקודות מפתח:

- לא נשמרים מפתחות חתימה סטטיים; טוקנים של OIDC נמשכים לפי דרישה.
- ארטיפקטים (CAR, manifest, bundle, סיכומי proofs) מועלים לבדיקה.
- ה-job משתמש באותן סכמות Norito שמשמשות ב-rollouts בפרודקשן.

## GitLab CI

```yaml
stages:
  - build
  - publish

variables:
  RUSTFLAGS: "-C target-cpu=native"

sorafs:build:
  stage: build
  image: rust:1.81
  script:
    - cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
    - sorafs_cli car pack --input fixtures/site.tar.gz --car-out artifacts/site.car --plan-out artifacts/site.plan.json --summary-out artifacts/site.car.json
    - sorafs_cli manifest build --summary artifacts/site.car.json --chunk-plan artifacts/site.plan.json --manifest-out artifacts/site.manifest.to
  artifacts:
    paths:
      - artifacts/

sorafs:publish:
  stage: publish
  needs: ["sorafs:build"]
  image: rust:1.81
  script:
    - sorafs_cli manifest sign --manifest artifacts/site.manifest.to --bundle-out artifacts/site.manifest.bundle.json --signature-out artifacts/site.manifest.sig --identity-token-env SIGSTORE_ID_TOKEN
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority ih58... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- הקצו `SIGSTORE_ID_TOKEN` באמצעות federation של workload identity ב-GitLab או sealed secret לפני ביצוע שלב ה-publish.
- כישלון של כל שלב CLI עוצר את ה-pipeline, ושומר על ארטיפקטים עקביים.

## משאבים נוספים

- תבניות end-to-end (כוללות helpers של Bash, קונפיגורציית זהות פדרטיבית ושלבי ניקוי): `docs/examples/sorafs_ci.md`
- רפרנס CLI שמכסה כל אפשרות: `docs/source/sorafs_cli.md`
- דרישות governance/alias לפני ההגשה:
  `docs/source/sorafs/provider_admission_policy.md`
