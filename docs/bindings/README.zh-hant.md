---
lang: zh-hant
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK 綁定和 Fixture 治理

路線圖上的 WP1-E 稱“文檔/綁定”為保存
跨語言綁定狀態。該文件記錄了綁定庫存，
再生命令、漂移防護和證據位置，以便 GPU 奇偶校驗
門 (WP1-E/F/G) 和跨 SDK 節奏委員會有一個參考。

## 共用護欄
- **規範劇本：** `docs/source/norito_binding_regen_playbook.md` 闡明
  Android 的輪換政策、預期證據和升級工作流程，
  Swift、Python 和未來的綁定。
- **Norito 模式奇偶校驗：** `scripts/check_norito_bindings_sync.py` （通過調用
  `scripts/check_norito_bindings_sync.sh` 並在 CI 中通過
  `ci/check_norito_bindings_sync.sh`) 當 Rust、Java 或 Python 時阻止構建
  模式工件漂移。
- **Cadence 看門狗：** `scripts/check_fixture_cadence.py` 讀取
  `artifacts/*_fixture_regen_state.json` 文件並強制執行週二/週五（Android、
  Python）和星期三（Swift）窗口，因此路線圖門具有可審核的時間戳。

## 結合矩陣

|綁定|切入點|夾具/再生命令 |漂移護衛|證據|
|--------|--------------|------------------------------------|----------------|----------|
|安卓（Java）| `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`、`ci/check_android_fixtures.sh`、`java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
|斯威夫特 (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh`（可選 `SWIFT_FIXTURE_ARCHIVE`）→ `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`、`ci/check_swift_fixtures.sh`、`scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`，`docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
|蟒蛇 | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`，`python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`，`docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`、`scripts/js_sbom_provenance.sh`、`scripts/js_signed_staging.sh` | `npm run test`、`javascript/iroha_js/scripts/verify-release-tarball.mjs`、`javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`、`artifacts/js/npm_staging/`、`artifacts/js/verification/`、`artifacts/js/sbom/` |

## 綁定細節

### 安卓（Java）
Android SDK 位於 `java/iroha_android/` 下並使用規範的 Norito
由 `scripts/android_fixture_regen.sh` 生產的燈具。那個助手導出
Rust 工具鏈中的新鮮 `.norito` blob，更新
`artifacts/android_fixture_regen_state.json`，並記錄節奏元數據
`scripts/check_fixture_cadence.py` 和治理儀表板消耗。漂移是
由 `scripts/check_android_fixtures.py` 檢測到（也連接到
`ci/check_android_fixtures.sh`) 和 `java/iroha_android/run_tests.sh`，其中
練習 JNI 綁定、WorkManager 隊列重放和 StrongBox 回退。
輪換證據、失敗註釋和重新運行記錄位於
`artifacts/android/fixture_runs/`。

### Swift (macOS/iOS)
`IrohaSwift/` 通過 `scripts/swift_fixture_regen.sh` 鏡像相同的 Norito 有效負載。
該腳本記錄旋轉所有者、節奏標籤和源（`live` 與 `archive`）
在 `artifacts/swift_fixture_regen_state.json` 內並將元數據饋送到
節奏檢查器。 `scripts/swift_fixture_archive.py` 允許維護人員攝取
Rust 生成的檔案； `scripts/check_swift_fixtures.py` 和
`ci/check_swift_fixtures.sh` 強制執行字節級奇偶校驗加上 SLA 期限限制，同時
`scripts/swift_fixture_regen.sh` 支持 `SWIFT_FIXTURE_EVENT_TRIGGER` 手動
輪換。升級工作流程、KPI 和儀表板記錄在
`docs/source/swift_parity_triage.md` 和下面的節奏內褲
`docs/source/sdk/swift/`。

###Python
Python 客戶端 (`python/iroha_python/`) 共享 Android 設備。跑步
`scripts/python_fixture_regen.sh`拉取最新的`.norito`有效負載，刷新
`python/iroha_python/tests/fixtures/`，並將節奏元數據發送到
`artifacts/python_fixture_regen_state.json` 第一次路線圖後輪換
被捕獲。 `scripts/check_python_fixtures.py` 和
`python/iroha_python/scripts/run_checks.sh` 門 pytest、mypy、ruff 和夾具
本地和 CI 中的平等。端到端文檔 (`docs/source/sdk/python/…`) 和
綁定再生手冊描述瞭如何與 Android 協調旋轉
業主。

### JavaScript
`javascript/iroha_js/` 不依賴本地 `.norito` 文件，但 WP1-E 跟踪
其發布證據使 GPU CI 通道繼承完整的來源。每次發布
通過 `npm run release:provenance` 捕獲出處（由
`javascript/iroha_js/scripts/record-release-provenance.mjs`)，生成並簽名
SBOM 與 `scripts/js_sbom_provenance.sh` 捆綁在一起，運行簽名的暫存試運行
(`scripts/js_signed_staging.sh`)，並驗證註冊表工件
`javascript/iroha_js/scripts/verify-release-tarball.mjs`。生成的元數據
落在 `artifacts/js-sdk-provenance/`、`artifacts/js/npm_staging/` 下，
`artifacts/js/sbom/` 和 `artifacts/js/verification/`，提供確定性
路線圖 JS5/JS6 和 WP1-F 基準測試運行的證據。出版劇本
`docs/source/sdk/js/` 將自動化聯繫在一起。