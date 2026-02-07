---
lang: zh-hant
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI 和 SDK — 發行說明 (v0.1.0)

## 亮點
- `sorafs_cli` 現在包含整個打包管道（`car pack`、`manifest build`、
  `proof verify`、`manifest sign`、`manifest verify-signature`），因此 CI 運行程序調用
  單個二進製文件而不是定制的助手。新的無密鑰簽名流程默認為
  `SIGSTORE_ID_TOKEN`，了解 GitHub Actions OIDC 提供程序，並發出確定性信號
  摘要 JSON 與簽名包一起。
- 多源獲取 *記分板* 作為 `sorafs_car` 的一部分提供：它標準化
  提供商遙測、強制執行能力處罰、保留 JSON/Norito 報告，以及
  通過共享註冊表句柄提供編排器模擬器 (`sorafs_fetch`)。
  `fixtures/sorafs_manifest/ci_sample/` 下的夾具展示了確定性
  CI/CD 預計會進行比較的輸入和輸出。
- 發布自動化被編入 `ci/check_sorafs_cli_release.sh` 和
  `scripts/release_sorafs_cli.sh`。現在每個版本都會存檔清單包，
  簽名，`manifest.sign/verify`摘要，以及記分板快照等治理
  審閱者可以追踪人工製品，而無需重新運行管道。

## 升級步驟
1. 更新工作區中對齊的 crate：
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. 在本地（或在 CI 中）重新運行發布門以確認 fmt/clippy/test 覆蓋率：
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. 使用策劃的配置重新生成簽名的工件和摘要：
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   如果出現以下情況，則將刷新的捆綁包/校樣複製到 `fixtures/sorafs_manifest/ci_sample/` 中：
   發布更新規範賽程。

## 驗證
- 發布門提交：`c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  （門成功後立即`git rev-parse HEAD`）。
- `ci/check_sorafs_cli_release.sh` 輸出：存檔於
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log`（附加到發行包中）。
- 清單捆綁摘要：`SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  （`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`）。
- 證明摘要摘要：`SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  （`fixtures/sorafs_manifest/ci_sample/proof.json`）。
- 清單摘要（用於下游證明交叉檢查）：
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  （來自 `manifest.sign.summary.json`）。

## 操作人員注意事項
- Torii 網關現在強制執行 `X-Sora-Chunk-Range` 功能標頭。更新
  允許列表，以便允許提供新流令牌範圍的客戶端；舊代幣
  如果沒有範圍聲明將會受到限制。
- `scripts/sorafs_gateway_self_cert.sh` 集成清單驗證。跑步時
  自認證工具，提供新生成的清單包，以便包裝器可以
  在簽名漂移上快速失敗。
- 遙測儀表板應將新的記分板導出 (`scoreboard.json`) 攝取到
  協調提供者的資格、權重分配和拒絕原因。
- 每次推出時存檔四個規範摘要：
  `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json`、
  `manifest.verify.summary.json`。治理票證在期間引用了這些確切的文件
  批准。

## 致謝
- 存儲團隊 — 端到端 CLI 整合、塊計劃渲染器和記分板
  遙測管道。
- 工具工作組 — 發布管道（`ci/check_sorafs_cli_release.sh`，
  `scripts/release_sorafs_cli.sh`) 和確定性夾具捆綁包。
- 網關操作 — 能力門控、流令牌策略審查和更新
  自我認證手冊。