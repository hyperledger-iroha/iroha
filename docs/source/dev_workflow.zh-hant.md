---
lang: zh-hant
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 代理開發工作流程

該操作手冊整合了 AGENTS 路線圖中的貢獻者護欄，以便
新補丁遵循相同的默認門。

## 快速入門目標

- 運行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包裝）來執行：
  1.`cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` 來自 `IrohaSwift/`
- `cargo test --workspace` 長時間運行（通常是幾個小時）。為了快速迭代，
  使用 `scripts/dev_workflow.sh --skip-tests` 或 `--skip-swift`，然後運行完整的
  發貨前的順序。
- 如果 `cargo test --workspace` 在構建目錄鎖上停止，請重新運行
  `scripts/dev_workflow.sh --target-dir target/codex-tests`（或設置
  `CARGO_TARGET_DIR` 到隔離路徑）以避免爭用。
- 所有貨物步驟都使用 `--locked` 來尊重存儲庫策略
  `Cargo.lock` 未受影響。更喜歡擴展現有的板條箱而不是添加
  新的工作區成員；在引入新的板條箱之前尋求批准。

## 護欄- `make check-agents-guardrails`（或 `ci/check_agents_guardrails.sh`）失敗，如果
  分支修改 `Cargo.lock`，引入新的工作區成員，或添加新的
  依賴關係。該腳本將工作樹和 `HEAD` 進行比較
  默認為`origin/main`；設置 `AGENTS_BASE_REF=<ref>` 以覆蓋基數。
- `make check-dependency-discipline`（或 `ci/check_dependency_discipline.sh`）
  比較 `Cargo.toml` 與基礎的依賴關係，並在新的箱子上失敗；設置
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` 承認故意
  補充。
- `make check-missing-docs`（或`ci/check_missing_docs_guard.sh`）阻止新的
  `#[allow(missing_docs)]` 條目，標記觸及板條箱（最近的 `Cargo.toml`）
  其 `src/lib.rs`/`src/main.rs` 缺少 crate 級 `//!` 文檔，並拒絕新的
  相對於基本參考沒有 `///` 文檔的公共項目；設置
  `MISSING_DOCS_GUARD_ALLOW=1` 僅經審閱者批准。守衛也
  驗證 `docs/source/agents/missing_docs_inventory.{json,md}` 是否新鮮；
  使用 `python3 scripts/inventory_missing_docs.py` 重新生成。
- `make check-tests-guard`（或 `ci/check_tests_guard.sh`）標記其
  更改後的 Rust 函數缺乏單元測試證據。守衛地圖改變了路線
  對於函數，如果差異中的板條箱測試發生變化，則通過，否則掃描
  用於匹配函數調用的現有測試文件，因此預先存在的覆蓋範圍
  計數；沒有任何匹配測試的板條箱將會失敗。設置 `TEST_GUARD_ALLOW=1`
  僅當更改真正與測試無關且審閱者同意時。
- `make check-docs-tests-metrics`（或 `ci/check_docs_tests_metrics_guard.sh`）
  強制執行里程碑與文檔一起移動的路線圖政策，
  測試和指標/儀表板。當 `roadmap.md` 相對於
  `AGENTS_BASE_REF`，警衛預計至少有一項文檔更改、一項測試更改，
  以及一項指標/遙測/儀表板更改。設置 `DOC_TEST_METRIC_GUARD_ALLOW=1`
  僅經審稿人批准。
- 當 TODO 標記時，`make check-todo-guard`（或 `ci/check_todo_guard.sh`）失敗
  在沒有伴隨文檔/測試更改的情況下消失。添加或更新覆蓋範圍
  解決 TODO 時，或設置 `TODO_GUARD_ALLOW=1` 進行有意刪除。
- `make check-std-only`（或 `ci/check_std_only.sh`）塊 `no_std`/`wasm32`
  cfgs 以便工作區保持僅 `std`。僅設置 `STD_ONLY_GUARD_ALLOW=1`
  批准的 CI 實驗。
- `make check-status-sync`（或 `ci/check_status_sync.sh`）保持路線圖開放
  部分不含已完成的項目，並需要 `roadmap.md`/`status.md`
  一起改變，使計劃/狀態保持一致；設置
  `STATUS_SYNC_ALLOW_UNPAIRED=1` 僅適用於罕見的僅狀態拼寫錯誤修復
  固定 `AGENTS_BASE_REF`。
- `make check-proc-macro-ui`（或 `ci/check_proc_macro_ui.sh`）運行 trybuild
  用於導出/過程宏 crate 的 UI 套件。當觸摸 proc-macros 時運行它
  保持 `.stderr` 診斷穩定並捕獲令人恐慌的 UI 回歸；設置
  `PROC_MACRO_UI_CRATES="crate1 crate2"` 專注於特定的板條箱。
- `make check-env-config-surface`（或`ci/check_env_config_surface.sh`）重建
  環境切換庫存 (`docs/source/agents/env_var_inventory.{json,md}`)，
  如果它是陳舊的，則失敗，**並且**當新的生產環境墊片出現時失敗
  相對於 `AGENTS_BASE_REF`（自動檢測；需要時明確設置）。
  添加/刪除環境查找後刷新跟踪器
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`；
  僅在記錄有意的環境旋鈕後才使用 `ENV_CONFIG_GUARD_ALLOW=1`在遷移跟踪器中。
- `make check-serde-guard`（或`ci/check_serde_guard.sh`）重新生成serde
  使用庫存 (`docs/source/norito_json_inventory.{json,md}`) 到臨時
  位置，如果提交的庫存過時則失敗，並拒絕任何新的庫存
  生產 `serde`/`serde_json` 相對於 `AGENTS_BASE_REF` 命中。套裝
  `SERDE_GUARD_ALLOW=1` 僅適用於提交遷移計劃後的 CI 實驗。
- `make guards` 強制執行 Norito 序列化策略：它拒絕新的
  `serde`/`serde_json` 用法、臨時 AoS 幫助程序和外部 SCALE 依賴項
  Norito 長凳 (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`、`scripts/deny_handrolled_aos.sh`、
  `scripts/check_no_scale.sh`）。
- **Proc-macro UI 政策：** 每個 proc-macro 箱都必須運送 `trybuild`
  `trybuild-tests` 後面的線束（帶有通過/失敗球的 `tests/ui.rs`）
  功能。將快樂路徑樣本放在 `tests/ui/pass` 下，將拒絕案例放在
  `tests/ui/fail` 具有承諾的 `.stderr` 輸出，並保持診斷
  無恐慌且穩定。刷新燈具
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests`（可選
  `CARGO_TARGET_DIR=target-codex` 以避免破壞現有版本）和
  避免依賴覆蓋範圍構建（預計有 `cfg(not(coverage))` 防護）。
  對於不發出二進制入口點的宏，更喜歡
  夾具中的 `// compile-flags: --crate-type lib` 以保持錯誤集中。添加
  每當診斷發生變化時，就會出現新的陰性病例。
- CI 通過 `.github/workflows/agents-guardrails.yml` 運行護欄腳本
  因此，當違反策略時，拉取請求會快速失敗。
- 示例 git hook (`hooks/pre-commit.sample`) 運行護欄、依賴項、
  缺少文檔、僅 std、環境配置和狀態同步腳本，因此貢獻者
  在 CI 之前發現政策違規行為。保留 TODO 麵包屑以備不時之需
  後續行動，而不是默默推遲重大變更。