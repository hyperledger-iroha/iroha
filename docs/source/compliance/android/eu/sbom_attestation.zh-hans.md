---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM 和来源证明 — Android SDK

|领域 |价值|
|--------|--------|
|范围 | Android SDK (`java/iroha_android`) + 示例应用程序 (`examples/android/*`) |
|工作流程所有者 |发布工程（Alexei Morozov）|
|最后验证 | 2026-02-11（Buildkite `android-sdk-release#4821`）|

## 1. 生成工作流程

运行帮助程序脚本（为 AND6 自动化添加）：

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

该脚本执行以下操作：

1. 执行 `ci/run_android_tests.sh` 和 `scripts/check_android_samples.sh`。
2. 调用 `examples/android/` 下的 Gradle 包装器来构建 CycloneDX SBOM
   `:android-sdk`、`:operator-console` 和 `:retail-wallet` 与随附的
   `-PversionName`。
3. 使用规范名称将每个 SBOM 复制到 `artifacts/android/sbom/<sdk-version>/` 中
   （`iroha-android.cyclonedx.json` 等）。

## 2. 出处和签名

相同的脚本使用 `cosign sign-blob --bundle <file>.sigstore --yes` 签署每个 SBOM
并在目标目录中发出 `checksums.txt` (SHA-256)。设置 `COSIGN`
如果二进制文件位于 `$PATH` 之外，则为环境变量。脚本完成后，
记录包/校验和路径以及 Buildkite 运行 ID
`docs/source/compliance/android/evidence_log.csv`。

## 3.验证

要验证已发布的 SBOM：

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

将输出 SHA 与 `checksums.txt` 中列出的值进行比较。审阅者还将 SBOM 与之前的版本进行比较，以确保依赖性增量是有意的。

## 4. 证据快照 (2026-02-11)

|组件| SBOM | SHA-256 | Sigstore 捆绑包 |
|------------|------|----------|------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` 捆绑包存储在 SBOM 旁边 |
|操作员控制台样本 | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
|零售钱包样本| `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*（从 Buildkite 运行 `android-sdk-release#4821` 捕获的哈希值；通过上面的验证命令重现。）*

## 5. 出色的工作

- 在 GA 之前自动执行发布管道内的 SBOM + 联合签名步骤。
- 一旦 AND6 标记清单完成，将 SBOM 镜像到公共工件存储桶。
- 与文档协调，从面向合作伙伴的发行说明中链接 SBOM 下载位置。