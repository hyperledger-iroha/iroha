---
lang: zh-hant
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e17a07b8d98725e24b75710496b0b69b6d8878160c7f12884e6e1eef0c0a4af7
source_last_modified: "2026-01-03T19:37:11.140795+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CI Cookbook
summary: Reference GitHub Actions workflow bundling sign + verify steps with review notes.
translator: machine-google-reviewed
---

# SoraFS CI 食譜

此片段反映了 `docs/source/sorafs_ci_templates.md` 中的指導和
演示如何將簽名、驗證和證明檢查集成到
單個 GitHub Actions 作業。

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
          sorafs_cli car pack \
            --input payload.bin \
            --car-out artifacts/payload.car \
            --plan-out artifacts/chunk_plan.json \
            --summary-out artifacts/car_summary.json
          sorafs_cli manifest build \
            --summary artifacts/car_summary.json \
            --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/manifest.to \
            --chunk-plan artifacts/chunk_plan.json \
            --bundle-out artifacts/manifest.bundle.json \
            --signature-out artifacts/manifest.sig \
            --identity-token-provider=github-actions \
            --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature \
            --manifest artifacts/manifest.to \
            --bundle artifacts/manifest.bundle.json \
            --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify \
            --manifest artifacts/manifest.to \
            --car artifacts/payload.car \
            --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## 註釋

- `sorafs_cli` 必須在運行器上可用（例如，執行這些步驟之前的 `cargo install --path crates/sorafs_car --features cli`）。
- 工作流程必須提供明確的 OIDC 受眾（此處為 `sorafs`）；調整 `--identity-token-audience` 以匹配您的 Fulcio 策略。
- 發布管道應存檔 `artifacts/manifest.bundle.json`、`artifacts/manifest.sig` 和 `artifacts/proof.json` 以供治理審查。
- 確定性樣本工件位於 `fixtures/sorafs_manifest/ci_sample` 中；當您需要黃金清單、塊計劃或捆綁 JSON 時，將它們複製到測試中，而無需重新計算管道。

## 夾具驗證

此工作流程的確定性工件位於
`fixtures/sorafs_manifest/ci_sample`。管道可以重播上述步驟並
將其輸出與規範文件進行比較，例如：

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

空差異確認構建產生了字節相同的清單、計劃和
簽名包。完整信息請參見 `fixtures/sorafs_manifest/ci_sample/README.md`
目錄列表和有關從捕獲的模板化發行說明的提示
總結。