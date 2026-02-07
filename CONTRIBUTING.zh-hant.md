---
lang: zh-hant
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-29T18:16:34.772429+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 貢獻指南

感謝您花時間為 Iroha 2 做出貢獻！

請閱讀本指南，了解您如何做出貢獻以及我們希望您遵循哪些準則。這包括有關代碼和文檔的指南以及有關 git 工作流程的約定。

閱讀這些指南將為您節省以後的時間。

## 我如何做出貢獻？

您可以通過多種方式為我們的項目做出貢獻：

- 報告[錯誤](#reporting-bugs) 和[漏洞](#reporting-vulnerabilities)
- [建議改進](#suggesting-improvements)並實施
- [提出問題](#asking-questions) 並與社區互動

我們的項目是新的嗎？ [做出你的第一個貢獻](#your-first-code-contribution)！

### 太長了；博士

- 找到[ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240)。
- 叉子 [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main)。
- 解決您的選擇問題。
- 確保您遵循我們的代碼和文檔的[風格指南](#style-guides)。
- 編寫[測試](https://doc.rust-lang.org/cargo/commands/cargo-test.html)。確保它們全部通過 (`cargo test --workspace`)。如果您接觸 SM 加密堆棧，還可以運行 `cargo test -p iroha_crypto --features "sm sm_proptest"` 來執行可選的模糊/屬性工具。
  - 注意：如果 `defaults/executor.to` 不存在，則執行 IVM 執行器的測試將自動合成最小的確定性執行器字節碼。運行測試不需要任何預先步驟。要生成奇偶校驗的規範字節碼，您可以運行：
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- 如果您更改derive/proc-macro crates，請通過以下方式運行trybuild UI套件
  `make check-proc-macro-ui`（或
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) 並刷新
  `.stderr` 在診斷更改時固定以保持消息穩定。
- 運行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包裝）以使用 `--locked` 和 `swift test` 執行 fmt/clippy/build/test；預計 `cargo test --workspace` 需要幾個小時，並且僅將 `--skip-tests` 用於快速本地循環。有關完整的操作手冊，請參閱 `docs/source/dev_workflow.md`。
- 使用 `make check-agents-guardrails` 強制執行護欄，以阻止 `Cargo.lock` 編輯和新的工作區 crate，`make check-dependency-discipline` 在新依賴項上失敗（除非明確允許），並使用 `make check-missing-docs` 來防止新的 `#[allow(missing_docs)]` 墊片、觸摸的 crate 上缺少 crate 級文檔或新的公共項目沒有文檔註釋（守衛通過 `scripts/inventory_missing_docs.py` 刷新 `docs/source/agents/missing_docs_inventory.{json,md}`）。添加 `make check-tests-guard`，除非單元測試引用它們（內聯 `#[cfg(test)]`/`#[test]` 塊或包 `tests/`；現有覆蓋率計數）和 `make check-docs-tests-metrics`，否則更改的功能將失敗，因此路線圖更改與文檔、測試和指標/儀表板配對。通過 `make check-todo-guard` 保持 TODO 強制執行，以便在沒有隨附文檔/測試的情況下不會刪除 TODO 標記。 `make check-env-config-surface` 重新生成環境切換清單，現在當相對於 `AGENTS_BASE_REF` 出現新的 **生產** 環境墊片時會失敗；僅在遷移跟踪器中記錄有意添加後才設置 `ENV_CONFIG_GUARD_ALLOW=1`。 `make check-serde-guard` 刷新 serde 庫存，並在過時快照或新生產 `serde`/`serde_json` 命中時失敗；僅在經過批准的遷移計劃時設置 `SERDE_GUARD_ALLOW=1`。通過 TODO 麵包屑和後續工單讓大額延期可見，而不是默默延期。運行 `make check-std-only` 以捕獲 `no_std`/`wasm32` cfgs 和 `make check-status-sync` 以確保 `roadmap.md` 未清項目保持僅開放狀態，並且路線圖/狀態一起更改；僅針對固定 `AGENTS_BASE_REF` 後罕見的僅狀態拼寫錯誤修復設置 `STATUS_SYNC_ALLOW_UNPAIRED=1`。對於單次調用，請使用 `make agents-preflight` 一起運行所有護欄。
- 在推送之前運行本地序列化防護：`make guards`。
  - 這會拒絕生產代碼中的直接 `serde_json`，不允許在允許列表之外添加新的直接 Serde 依賴項，並阻止 `crates/norito` 之外的臨時 AoS/NCB 幫助程序。
- 可選擇本地試運行 Norito 特徵矩陣：`make norito-matrix`（使用快速子集）。
  - 要獲得完整覆蓋，請運行 `scripts/run_norito_feature_matrix.sh`，而不運行 `--fast`。
  - 每個組合包含下游煙霧（默認板條箱 `iroha_data_model`）：`make norito-matrix-downstream` 或 `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`。
- 對於 proc-macro 包，添加 `trybuild` UI 線束 (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) 並針對失敗案例提交 `.stderr` 診斷。保持診斷穩定且不恐慌；使用 `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` 刷新燈具並使用 `cfg(all(feature = "trybuild-tests", not(coverage)))` 保護它們。
- 執行預提交例程，例如格式化和工件重新生成（請參閱 [`pre-commit.sample`](./hooks/pre-commit.sample)）
- 將 `upstream` 設置為跟踪 [Hyperledger Iroha 存儲庫](https://github.com/hyperledger-iroha/iroha)、`git pull -r upstream main`、`git commit -s`、`git push <your-fork>` 和 [創建拉取請求](https://github.com/hyperledger-iroha/iroha/compare) 到 `main` 分支。確保它遵循 [拉取請求指南](#pull-request-etiquette)。

### 代理工作流程快速入門

- 運行 `make dev-workflow`（`scripts/dev_workflow.sh` 的包裝，記錄在 `docs/source/dev_workflow.md` 中）。它包含 `cargo fmt --all`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build/test --workspace --locked`（測試可能需要幾個小時）和 `swift test`。
- 使用 `scripts/dev_workflow.sh --skip-tests` 或 `--skip-swift` 實現更快的迭代；在打開拉取請求之前重新運行完整序列。
- Guardrails：避免接觸 `Cargo.lock`、添加新的工作區成員、引入新的依賴項、添加新的 `#[allow(missing_docs)]` 墊片、省略箱級文檔、更改功能時跳過測試、在沒有文檔/測試的情況下刪除 TODO 標記，或未經批准重新引入 `no_std`/`wasm32` cfgs。運行 `make check-agents-guardrails`（或 `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`）加上 `make check-dependency-discipline`、`make check-missing-docs`（刷新 `docs/source/agents/missing_docs_inventory.{json,md}`）、`make check-tests-guard`（當生產函數在沒有單元測試證據的情況下發生更改時失敗 - 差異中的測試更改或現有測試必須引用函數）、`make check-docs-tests-metrics`（當路線圖更改缺少文檔/測試/指標更新時失敗）、`make check-todo-guard`、`make check-env-config-surface`（在過時的庫存或新的生產環境切換上失敗；僅在更新文檔後使用 `ENV_CONFIG_GUARD_ALLOW=1` 覆蓋）和 `make check-serde-guard`（失敗在過時的 Serde 庫存或新的生產 Serde 命中上；僅使用已批准的遷移計劃在本地覆蓋 `SERDE_GUARD_ALLOW=1` 以獲取早期信號，`make check-std-only` 用於僅標准保護，並使 `roadmap.md`/`status.md` 與 `make check-status-sync` 保持同步（設置） `STATUS_SYNC_ALLOW_UNPAIRED=1` 僅適用於固定 `AGENTS_BASE_REF` 後罕見的僅狀態拼寫錯誤修復）。如果您希望在打開 PR 之前使用單個命令運行所有防護，請使用 `make agents-preflight`。

### 報告錯誤

*bug* 是 Iroha 中的錯誤、設計缺陷、故障或故障，導致其產生不正確、意外或非預期的結果或行為。

我們通過標有 `Bug` 標籤的 [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) 跟踪 Iroha 錯誤。

當您創建新問題時，有一個模板供您填寫。以下是報告錯誤時應執行的操作的清單：
- [ ] 添加 `Bug` 標籤
- [ ] 解釋一下問題
- [ ] 提供一個最小的工作示例
- [ ] 附上截圖

<details> <summary>最小工作示例</summary>

對於每個錯誤，您應該提供一個[最小工作示例](https://en.wikipedia.org/wiki/Minimal_working_example)。例如：

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</詳情>

---
**注意：** 文檔過時、文檔不足或功能請求等問題應使用 `Documentation` 或 `Enhancement` 標籤。它們不是蟲子。

---

### 報告漏洞

雖然我們積極預防安全問題，但您可能會先於我們遇到安全漏洞。

- 在第一個主要版本 (2.0) 之前，所有漏洞都被視為錯誤，因此請隨意將它們作為錯誤提交[按照上述說明](#reporting-bugs)。
- 第一個主要版本發布後，使用我們的[漏洞賞金計劃](https://hackerone.com/hyperledger) 提交漏洞並獲得獎勵。

：感嘆：為了最大程度地減少未修補的安全漏洞造成的損害，您應該盡快直接向 Hyperledger 披露該漏洞，並在合理的時間內**避免公開披露相同的漏洞**。

如果您對我們處理安全漏洞有任何疑問，請隨時通過私信聯繫 Rocket.Chat 目前活躍的維護者。

### 提出改進建議

使用適當的標籤（`Optimization`、`Enhancement`）在 GitHub 上創建[問題](https://github.com/hyperledger-iroha/iroha/issues/new)，並描述您建議的改進。您可以將這個想法留給我們或其他人來開發，或者您可以自己實現。

如果您打算自己實施該建議，請執行以下操作：

1. 在開始解決問題之前，將您創建的問題分配給自己。
2. 使用您建議的功能並遵循我們的[代碼和文檔指南](#style-guides)。
3. 當您準備好打開拉取請求時，請確保遵循 [拉取請求指南](#pull-request-etiquette) 並將其標記為實現之前創建的問題：

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. 如果您的更改需要 API 更改，請使用 `api-changes` 標籤。

   **注意：** 需要 API 更改的功能可能需要更長的時間來實施和批准，因為它們需要 Iroha 庫製造商更新其代碼。### 提問

問題是指既不是錯誤也不是功能或優化請求的任何討論。

<詳細信息> <摘要> 我如何提問？ </摘要>

請將您的問題發佈到【我們的即時通訊平台之一】(#contact)，以便社區工作人員和成員及時為您提供幫助。

作為上述社區的一部分，您也應該考慮幫助其他人。如果您決定提供幫助，請以[尊重的方式](CODE_OF_CONDUCT.md) 提供幫助。

</詳情>

## 您的第一個代碼貢獻

1. 在帶有 [good-first-issue](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue) 標籤的問題中找到適合初學者的問題。
2. 確保沒有其他人正在處理您選擇的問題，方法是檢查該問題是否未分配給任何人。
3. 將問題分配給自己，以便其他人可以看到有人正在處理該問題。
4. 在開始編寫代碼之前，請閱讀我們的 [Rust 風格指南](#rust-style-guide)。
5. 當您準備好提交更改時，請閱讀 [拉取請求指南](#pull-request-etiquette)。

## 拉取請求禮儀

請 [fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [存儲庫](https://github.com/hyperledger-iroha/iroha/tree/main) 並[創建功能分支](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) 做出貢獻。使用**來自叉子的 PR** 時，請查看[本手冊](https://help.github.com/articles/checking-out-pull-requests-locally)。

#### 致力於代碼貢獻：
- 遵循 [Rust 風格指南](#rust-style-guide) 和 [文檔風格指南](#documentation-style-guide)。
- 確保您編寫的代碼已被測試覆蓋。如果您修復了錯誤，請將重現該錯誤的最小工作示例轉變為測試。
- 當觸摸derive/proc-macro crates時，運行`make check-proc-macro-ui`（或
  使用 `PROC_MACRO_UI_CRATES="crate1 crate2"` 進行過濾），因此嘗試構建 UI 裝置
  保持同步，診斷保持穩定。
- 記錄新的公共 API（新項目上的板條箱級 `//!` 和 `///`），並運行
  `make check-missing-docs` 驗證護欄。調出您的文檔/測試
  添加到您的拉取請求描述中。

#### 提交你的工作：
- 遵循 [Git 風格指南](#git-workflow)。
- [在合併之前](https://www.git-tower.com/learn/git/faq/git-squash/) 或[在合併期間](https://rietta.com/blog/github-merge-types/) 壓縮您的提交。
- 如果在準備拉取請求期間您的分支已過時，請使用 `git pull --rebase upstream main` 在本地重新建立基礎。或者，您可以使用 `Update branch` 按鈕的下拉菜單並選擇 `Update with rebase` 選項。

  為了讓每個人都更輕鬆地完成此過程，請盡量不要對拉取請求進行多次提交，並避免重複使用功能分支。

#### 創建拉取請求：
- 按照 [拉取請求禮儀](#pull-request-etiquette) 部分中的指導，使用適當的拉取請求描述。如果可能，請避免偏離這些準則。
- 添加適當格式的[拉取請求標題](#pull-request-titles)。
- 如果您覺得您的代碼尚未準備好合併，但您希望維護人員查看它，請創建草稿拉取請求。

#### 合併你的工作：
- 拉取請求在合併之前必須通過所有自動檢查。至少，代碼必須經過格式化、通過所有測試，並且沒有突出的 `clippy` lint。
- 如果沒有活躍維護者的兩次批准評論，則無法合併拉取請求。
- 每個拉取請求都會自動通知代碼所有者。當前維護者的最新列表可以在 [MAINTAINERS.md](MAINTAINERS.md) 中找到。

####複習禮儀：
- 不要自行解決對話。讓審稿人做出決定。
- 確認審稿意見並與審稿人互動（同意、不同意、澄清、解釋等）。不要忽視評論。
- 對於簡單的代碼更改建議，如果直接應用它們，就可以解決對話。
- 推送新更改時避免覆蓋以前的提交。它混淆了自上次審核以來發生的變化，並迫使審核者從頭開始。提交在自動合併之前會被壓縮。

### 拉取請求標題

我們解析所有合併的拉取請求的標題以生成變更日誌。我們還通過 *`check-PR-title`* 檢查來檢查標題是否遵循約定。

要通過 *`check-PR-title`* 檢查，拉取請求標題必須遵循以下準則：

<details> <summary> 展開以閱讀詳細的標題指南</summary>

1.遵循[常規提交](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers)格式。

2. 如果拉取請求只有一個提交，則 PR 標題應與提交消息相同。

</詳情>

### Git 工作流程

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) [存儲庫](https://github.com/hyperledger-iroha/iroha/tree/main) 和[創建功能分支](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) 供您貢獻。
- [配置遠程](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) 將您的 fork 與 [Hyperledger Iroha 存儲庫](https://github.com/hyperledger-iroha/iroha/tree/main) 同步。
- 使用 [Git Rebase 工作流程](https://git-rebase.io/)。避免使用 `git pull`。請改用 `git pull --rebase`。
- 使用提供的 [git hooks](./hooks/) 來簡化開發過程。

請遵循以下提交指南：

- **簽署每個提交**。如果您不這樣做，[DCO](https://github.com/apps/dco) 將不會讓您合併。

  使用 `git commit -s` 自動添加 `Signed-off-by: $NAME <$EMAIL>` 作為提交消息的最後一行。您的姓名和電子郵件應與您的 GitHub 帳戶中指定的相同。

  我們還鼓勵您使用 `git commit -sS` 使用 GPG 密鑰簽署您的提交（[了解更多](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)）。

  您可以使用 [`commit-msg` 掛鉤](./hooks/) 自動簽署您的提交。

- 提交消息必須遵循 [常規提交](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) 以及與 [拉取請求標題](#pull-request-titles) 相同的命名架構。這意味著：
  - **使用現在時**（“添加功能”，而不是“添加功能”）
  - **使用祈使語氣**（“部署到 docker...”而不是“部署到 docker...”）
- 編寫有意義的提交消息。
- 嘗試保持提交消息簡短。
- 如果您需要更長的提交消息：
  - 將提交消息的第一行限制為 50 個字符或更少。
  - 提交消息的第一行應包含您已完成工作的摘要。如果您需要多行，請在每個段落之間留一個空行，並在中間描述您的更改。最後一行必須是結束行。
- 如果您修改架構（通過使用 `kagami schema` 和 diff 生成架構進行檢查），您應該在單獨的提交中對架構進行所有更改，並顯示消息 `[schema]`。
- 嘗試堅持對每個有意義的更改進行一次提交。
  - 如果您在一個 PR 中修復了多個問題，請分別提交它們。
  - 如前所述，對 `schema` 和 API 的更改應在與其餘工作分開的適當提交中完成。
  - 在與該功能相同的提交中添加功能測試。

## 測試和基準

- 要運行基於源代碼的測試，請在 Iroha 根目錄中執行 [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html)。請注意，這是一個漫長的過程。
- 要運行基準測試，請從 Iroha 根目錄執行 [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)。為了幫助調試基準測試輸出，請設置 `debug_assertions` 環境變量，如下所示：`RUSTFLAGS="--cfg debug_assertions" cargo bench`。
- 如果您正在處理特定組件，請注意，當您在[工作空間](https://doc.rust-lang.org/cargo/reference/workspaces.html)中運行`cargo test`時，它只會運行該工作空間的測試，通常不包括任何[集成測試](https://www.testingxperts.com/blog/what-is-integration-testing)。
- 如果您想在最小網絡上測試您的更改，提供的 [`docker-compose.yml`](defaults/docker-compose.yml) 在 Docker 容器中創建一個由 4 個 Iroha 對等點組成的網絡，可用於測試共識和資產傳播相關邏輯。我們建議使用 [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) 或隨附的 Iroha 客戶端 CLI 與該網絡進行交互。
- 不要刪除失敗的測試。即使被忽略的測試最終也會在我們的管道中運行。
- 如果可能，請在進行更改之前和之後對您的代碼進行基準測試，因為顯著的性能下降可能會破壞現有用戶的安裝。

### 序列化防護檢查

運行 `make guards` 以在本地驗證存儲庫策略：

- 將生產源中的直接 `serde_json` 列入拒絕名單（首選 `norito::json`）。
- 禁止在允許列表之外直接進行 `serde`/`serde_json` 依賴項/導入。
- 防止在 `crates/norito` 之外重新引入臨時 AoS/NCB 幫助程序。

### 調試測試

<details> <summary> 展開以了解如何更改日誌級別或將日誌寫入 JSON。 </summary>

如果您的其中一項測試失敗，您可能需要降低最大日誌記錄級別。默認情況下，Iroha 僅記錄 `INFO` 級別消息，但保留生成 `DEBUG` 和 `TRACE` 級別日誌的能力。可以使用 `LOG_LEVEL` 環境變量進行基於代碼的測試，或使用已部署網絡中對等點之一上的 `/configuration` 端點來更改此設置。雖然以 `stdout` 打印的日誌就足夠了，但您可能會發現將 `json` 格式的日誌生成到單獨的文件中並使用 [node-bunyan](https://www.npmjs.com/package/bunyan) 或 [rust-bunyan](https://crates.io/crates/bunyan) 解析它們更方便。

將 `LOG_FILE_PATH` 環境變量設置為適當的位置來存儲日誌並使用上述包解析它們。

</詳情>

### 使用 tokio 控制台進行調試

<details> <summary> 展開以了解如何使用 tokio 控制台支持編譯 Iroha。 </summary>

有時，使用 [tokio-console](https://github.com/tokio-rs/console) 分析 tokio 任務可能有助於調試。

在這種情況下，您應該在 tokio 控制台的支持下編譯 Iroha，如下所示：

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

tokio 控制台的端口可以通過 `LOG_TOKIO_CONSOLE_ADDR` 配置參數（或環境變量）進行配置。
使用tokio控制台需要日誌級別為`TRACE`，可以通過配置參數或環境變量`LOG_LEVEL`啟用。

使用 `scripts/test_env.sh` 在 tokio 控制台支持下運行 Iroha 的示例：

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</詳情>

### 分析

<details> <summary> 展開以了解如何分析 Iroha。 </摘要>

為了優化性能，分析 Iroha 非常有用。

分析構建當前需要夜間工具鏈。要準備一個，請使用 `profiling` 配置文件和功能來編譯 Iroha：

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

然後啟動 Iroha 並將您選擇的分析器附加到 Iroha pid。

或者，可以在 docker 內部構建 Iroha，並使用探查器支持並以這種方式分析 Iroha。

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

例如使用 perf（僅在 Linux 上可用）：

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

為了能夠在 Iroha 分析期間觀察執行器的配置文件，應在不剝離符號的情況下編譯執行器。
可以通過運行以下命令來完成：

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

啟用分析功能後，Iroha 會公開端點以廢棄 pprof 配置文件：

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</詳情>

## 風格指南

當您向我們的項目貢獻代碼時，請遵循以下準則：

### Git 風格指南

:book: [閱讀 git 指南](#git-workflow)

### Rust 風格指南

<details> <summary> :book: 閱讀代碼指南</summary>

- 使用 `cargo fmt --all`（2024 版）格式化代碼。

代碼指南：

- 除非另有說明，請參閱[Rust 最佳實踐](https://github.com/mre/idiomatic-rust)。
- 使用 `mod.rs` 樣式。 [自命名模塊](https://rust-lang.github.io/rust-clippy/master/) 將不會通過靜態分析，但 [`trybuild`](https://crates.io/crates/trybuild) 測試除外。
- 使用領域優先的模塊結構。

  示例：不要執行 `constants::logger`。相反，反轉層次結構，將使用它的對象放在前面：`iroha_logger::constants`。
- 使用帶有明確錯誤消息或無誤證明的 [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/)，而不是 `unwrap`。
- 永遠不要忽略錯誤。如果不能`panic`並且無法恢復，至少需要記錄在日誌中。
- 更願意返回 `Result` 而不是 `panic!`。
- 在空間上對相關功能進行分組，最好在適當的模塊內。

  例如，最好在其旁邊放置與 `struct` 相關的 `impl`，而不是使用具有 `struct` 定義的塊，然後為每個單獨的結構提供 `impl`。
- 實現前聲明：`use` 語句和常量位於頂部，單元測試位於底部。
- 如果導入的名稱僅使用一次，請盡量避免 `use` 語句。這使得將代碼移動到不同的文件中變得更容易。
- 不要隨意靜音 `clippy` lints。如果您這樣做，請通過註釋（或 `expect` 消息）解釋您的推理。
- 如果 `#[outer_attribute]` 可用，則優先選擇 `#![inner_attribute]`。
- 如果您的函數不會改變其任何輸入（並且不應改變其他任何內容），請將其標記為 `#[must_use]`。
- 如果可能的話，避免使用 `Box<dyn Error>`（我們更喜歡強類型）。
- 如果您的函數是 getter/setter，請將其標記為 `#[inline]`。
- 如果您的函數是構造函數（即，它從輸入參數創建一個新值並調用 `default()`），請將其標記為 `#[inline]`。
- 避免將代碼與具體的數據結構聯繫起來； `rustc` 足夠智能，可以在需要時將 `Vec<InstructionExpr>` 轉換為 `impl IntoIterator<Item = InstructionExpr>`，反之亦然。

命名准則：
- 在 *public* 結構、變量、方法、特徵、常量和模塊名稱中僅使用完整單詞。但是，在以下情況下允許使用縮寫：
  - 名稱是本地的（例如閉包參數）。
  - 該名稱按照 Rust 約定縮寫（例如 `len`、`typ`）。
  - 名稱是可接受的縮寫（例如 `tx`、`wsv` 等）；有關規範縮寫，請參閱[項目詞彙表](https://docs.iroha.tech/reference/glossary.html)。
  - 全名將被局部變量隱藏（例如 `msg <- message`）。
  - 如果全名超過 5-6 個單詞，則代碼會變得很麻煩（例如 `WorldStateViewReceiverTrait -> WSVRecvTrait`）。
- 如果您更改命名約定，請確保您選擇的新名稱比我們之前的名稱_更_清晰。

評論指南：
- 在編寫非文檔註釋時，不要描述你的函數做了什麼，而是嘗試解釋它為什麼以特定的方式做某事。這將節省您和審稿人的時間。
- 只要您引用為其創建的問題，您就可以在代碼中留下 `TODO` 標記。不創建問題意味著它不會被合併。

我們使用固定依賴項。請遵循以下版本控制指南：

- 如果您的工作依賴於特定的板條箱，請查看是否尚未使用 [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) 安裝（使用 `bat` 或 `grep`），並嘗試使用該版本，而不是最新版本。
- 在 `Cargo.toml` 中使用完整版本“X.Y.Z”。
- 在單獨的 PR 中提供版本升級。

</詳情>

### 文檔風格指南

<details> <summary> :book：閱讀文檔指南</summary>


- 使用 [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) 格式。
- 更喜歡單行註釋語法。對內聯模塊使用 `///`，對基於文件的模塊使用 `//!`。
- 如果您可以鏈接到結構/模塊/函數的文檔，請鏈接到該文檔。
- 如果您可以提供使用示例，請提供。這[也是一個測試](https://doc.rust-lang.org/rustdoc/documentation-tests.html)。
- 如果函數可能出錯或出現緊急情況，請避免使用情態動詞。示例：`Fails if disk IO fails` 而不是 `Can possibly fail, if disk IO happens to fail`。
- 如果某個函數可能因多種原因而出錯或出現緊急情況，請使用故障條件的項目符號列表以及相應的 `Error` 變體（如果有）。
- 函數*做*事情。使用祈使語氣。
- 結構*是*事物。進入正題吧。例如，`Log level for reloading from the environment` 優於 `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`。
- 結構有字段，字段也是事物。
- 模塊*包含*東西，我們知道這一點。進入正題吧。示例：使用 `Logger-related traits.` 代替 `Module which contains logger-related logic`。


</詳情>

## 聯繫方式

我們的社區成員活躍於：

|服務 |鏈接 |
|--------------|------------------------------------------------------------------------|
|堆棧溢出 | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
|郵件列表 | https://lists.lfdecentralizedtrust.org/g/iroha |
|電報 | https://t.me/hyperledgeriroha |
|不和諧 | https://discord.com/channels/905194001349627914/905205848547155968 |

---