---
lang: zh-hant
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

# SoraFS CI 樣品夾具

該目錄打包從示例生成的確定性工件
`fixtures/sorafs_manifest/ci_sample/` 下的有效負載。該捆綁包演示了
CI 工作流程執行的端到端 SoraFS 打包和簽名管道。

## 文物庫存

|文件|描述 |
|------|-------------|
| `payload.txt` |夾具腳本使用的源有效負載（純文本示例）。 |
| `payload.car` | `sorafs_cli car pack` 發出的 CAR 存檔。 |
| `car_summary.json` |由 `car pack` 捕獲塊摘要和元數據生成的摘要。 |
| `chunk_plan.json` |描述塊範圍和提供者期望的獲取計劃 JSON。 |
| `manifest.to` | Norito 由 `sorafs_cli manifest build` 生成的清單。 |
| `manifest.json` |用於調試的人類可讀清單渲染。 |
| `proof.json` | `sorafs_cli proof verify` 發布的 PoR 摘要。 |
| `manifest.bundle.json` |由 `sorafs_cli manifest sign` 生成的無密鑰簽名包。 |
| `manifest.sig` |分離了與清單相對應的 Ed25519 簽名。 |
| `manifest.sign.summary.json` |簽名期間發出的 CLI 摘要（哈希值、捆綁元數據）。 |
| `manifest.verify.summary.json` |來自 `manifest verify-signature` 的 CLI 摘要。 |

發行說明和文檔中引用的所有摘要均來自
這些文件。 `ci/check_sorafs_cli_release.sh` 工作流程重新生成相同的
工件並將其與提交的版本進行比較。

## 夾具再生

從存儲庫根運行以下命令以重新生成夾具集。
它們反映了 `sorafs-cli-fixture` 工作流程使用的步驟：

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

如果任何步驟產生不同的哈希值，請在更新裝置之前進行調查。
CI 工作流程依賴確定性輸出來檢測回歸。

## 未來的報導

隨著更多的分塊配置文件和證明格式從路線圖中畢業，
他們的規範裝置將添加到此目錄下（例如，
`sorafs.sf2@1.0.0`（參見 `fixtures/sorafs_manifest/ci_sample_sf2/`）或 PDP
流證明）。每個新配置文件將遵循相同的結構——有效負載、CAR、
計劃、清單、證明和簽名工件 - 因此下游自動化可以
diff 版本無需自定義腳本。