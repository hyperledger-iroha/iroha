---
lang: zh-hans
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI 样品夹具

该目录打包从示例生成的确定性工件
`fixtures/sorafs_manifest/ci_sample/` 下的有效负载。该捆绑包演示了
CI 工作流程执行的端到端 SoraFS 打包和签名管道。

## 文物库存

|文件|描述 |
|------|-------------|
| `payload.txt` |夹具脚本使用的源有效负载（纯文本示例）。 |
| `payload.car` | `sorafs_cli car pack` 发出的 CAR 存档。 |
| `car_summary.json` |由 `car pack` 捕获块摘要和元数据生成的摘要。 |
| `chunk_plan.json` |描述块范围和提供者期望的获取计划 JSON。 |
| `manifest.to` | Norito 由 `sorafs_cli manifest build` 生成的清单。 |
| `manifest.json` |用于调试的人类可读清单渲染。 |
| `proof.json` | `sorafs_cli proof verify` 发布的 PoR 摘要。 |
| `manifest.bundle.json` |由 `sorafs_cli manifest sign` 生成的无密钥签名包。 |
| `manifest.sig` |分离了与清单相对应的 Ed25519 签名。 |
| `manifest.sign.summary.json` |签名期间发出的 CLI 摘要（哈希值、捆绑元数据）。 |
| `manifest.verify.summary.json` |来自 `manifest verify-signature` 的 CLI 摘要。 |

发行说明和文档中引用的所有摘要均来自
这些文件。 `ci/check_sorafs_cli_release.sh` 工作流程重新生成相同的
工件并将其与提交的版本进行比较。

## 夹具再生

从存储库根运行以下命令以重新生成夹具集。
它们反映了 `sorafs-cli-fixture` 工作流程使用的步骤：

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

如果任何步骤产生不同的哈希值，请在更新装置之前进行调查。
CI 工作流程依赖确定性输出来检测回归。

## 未来的报道

随着更多的分块配置文件和证明格式从路线图中毕业，
他们的规范装置将添加到此目录下（例如，
`sorafs.sf2@1.0.0`（参见 `fixtures/sorafs_manifest/ci_sample_sf2/`）或 PDP
流证明）。每个新配置文件将遵循相同的结构——有效负载、CAR、
计划、清单、证明和签名工件 - 因此下游自动化可以
diff 版本无需自定义脚本。