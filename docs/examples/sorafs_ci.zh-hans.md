---
lang: zh-hans
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

# SoraFS CI 食谱

此片段反映了 `docs/source/sorafs_ci_templates.md` 中的指导和
演示如何将签名、验证和证明检查集成到
单个 GitHub Actions 作业。

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

## 注释

- `sorafs_cli` 必须在运行器上可用（例如，执行这些步骤之前的 `cargo install --path crates/sorafs_car --features cli`）。
- 工作流程必须提供明确的 OIDC 受众（此处为 `sorafs`）；调整 `--identity-token-audience` 以匹配您的 Fulcio 策略。
- 发布管道应存档 `artifacts/manifest.bundle.json`、`artifacts/manifest.sig` 和 `artifacts/proof.json` 以供治理审查。
- 确定性样本工件位于 `fixtures/sorafs_manifest/ci_sample` 中；当您需要黄金清单、块计划或捆绑 JSON 时，将它们复制到测试中，而无需重新计算管道。

## 夹具验证

此工作流程的确定性工件位于
`fixtures/sorafs_manifest/ci_sample`。管道可以重播上述步骤并
将其输出与规范文件进行比较，例如：

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

空差异确认构建产生了字节相同的清单、计划和
签名包。完整信息请参见 `fixtures/sorafs_manifest/ci_sample/README.md`
目录列表和有关从捕获的模板化发行说明的提示
总结。