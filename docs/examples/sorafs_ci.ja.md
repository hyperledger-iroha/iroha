---
lang: ja
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

---
title: SoraFS CI クックブック
summary: 署名と検証をまとめた GitHub Actions workflow とレビュー用メモ。
---

# SoraFS CI クックブック

このスニペットは `docs/source/sorafs_ci_templates.md` のガイドを反映し、署名、検証、
proof checks を 1 つの GitHub Actions job に統合する方法を示します。

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

## ノート

- `sorafs_cli` は runner で利用可能である必要があります (例: `cargo install --path crates/sorafs_car --features cli` を事前に実行)。
- Workflow は明示的な OIDC audience (ここでは `sorafs`) を提供する必要があります。`--identity-token-audience` を Fulcio ポリシーに合わせて調整してください。
- Release pipeline は `artifacts/manifest.bundle.json`, `artifacts/manifest.sig`, `artifacts/proof.json` を governance レビューのためにアーカイブします。
- 決定的なサンプル artefacts は `fixtures/sorafs_manifest/ci_sample` にあります。golden manifests, chunk plans, bundle JSON が必要な場合は pipeline を再計算せずにテストへコピーしてください。

## Fixture 検証

この workflow の決定的 artefacts は
`fixtures/sorafs_manifest/ci_sample` にあります。pipeline は上記の手順を再現し、
出力を canonical ファイルと diff できます。例えば:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

diff が空であれば、ビルドが byte-identical な manifests、plans、signature bundles を生成したことを示します。
`fixtures/sorafs_manifest/ci_sample/README.md` にディレクトリ一覧と、capture された summaries から
release notes をテンプレート化するためのヒントがあります。
