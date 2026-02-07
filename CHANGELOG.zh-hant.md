---
lang: zh-hant
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-05T09:28:11.640562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 變更日誌

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

該項目的所有顯著更改都將記錄在該文件中。

## [未發布]

- 放下 SCALE 墊片； `norito::codec` 現在通過本機 Norito 序列化來實現。
- 將跨 crate 的 `parity_scale_codec` 用法替換為 `norito::codec`。
- 開始將工具遷移到本機 Norito 序列化。
- 從工作區中刪除剩餘的 `parity-scale-codec` 依賴項，以支持本機 Norito 序列化。
- 用本機 Norito 實現替換殘留的 SCALE 特徵派生，並重命名版本化編解碼器模塊。
- 使用功能門控宏將 `iroha_config_base_derive` 和 `iroha_futures_derive` 合併為 `iroha_derive`。
- *(multisig)* 使用穩定的錯誤代碼/原因拒絕來自多重簽名機構的直接簽名，跨嵌套中繼器強制執行多重簽名 TTL 上限，並在提交之前在 CLI 中顯示 TTL 上限（SDK 奇偶校驗待定）。
- 將 FFI 程序宏移至 `iroha_ffi` 並刪除 `iroha_ffi_derive` 箱。
- *(schema_g​​​​en)* 從 `iroha_data_model` 依賴項中刪除不必要的 `transparent_api` 功能。
- *(data_model)* 緩存用於 `Name` 解析的 ICU NFC 標準化器，以減少重複的初始化開銷。
- 📚 Torii 客戶端的文檔 JS 快速入門、配置解析器、發布工作流程和配置感知配方。
- *(IrohaSwift)* 將最低部署目標提高到 iOS 15 / macOS 12，跨 Torii 客戶端 API 採用 Swift 並發，並將公共模型標記為 `Sendable`。
- *(IrohaSwift)* 添加了 `ToriiDaProofSummaryArtifact` 和 `DaProofSummaryArtifactEmitter.emit`，以便 Swift 應用程序可以構建/發出與 CLI 兼容的 DA 證明包，而無需向 CLI 進行 shell 操作，並包含涵蓋內存中和磁盤上的文檔和回歸測試【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Tests/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* 通過從 `KaigiParticipantCommitment` 中刪除存檔重用標誌來修復 Kaigi 選項序列化，添加本機往返測試，並刪除 JS 解碼回退，以便 Kaigi 現在指示 Norito 往返之前提交。 【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* 允許 `ToriiClient` 調用者刪除默認標頭（通過傳遞 `null`），以便 `getMetrics` 在 JSON 和 Prometheus 文本之間乾淨地切換 接受headers.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* 為 NFT、每個賬戶資產餘額和資產定義持有者（帶有 TypeScript defs、文檔和測試）添加了可迭代幫助程序，因此 Torii 分頁現在涵蓋了剩餘的應用程序端點。 【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* 添加了治理指令/交易構建器以及治理配方，以便 JS 客戶端可以端到端地部署提案、投票、頒布和理事會持久性。 【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* 添加了 ISO 20022 pacs.008 提交/狀態幫助程序和匹配的配方，讓 JS 調用者無需定制 HTTP 即可行使 Torii ISO 橋管道。 【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】
- *(javascript)* 添加了 pacs.008/pacs.009 構建器幫助程序以及配置驅動的配方，以便 JS 調用者可以在點擊之前用經過驗證的 BIC/IBAN 元數據合成 ISO 20022 有效負載橋.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* 完成了 DA 攝取/獲取/證明循環：`ToriiClient.fetchDaPayloadViaGateway` 現在自動派生分塊器句柄（通過新的 `deriveDaChunkerHandle` 綁定），可選證明摘要重用本機 `generateDaProofSummary`，並且刷新了 README/打字/測試，以便 SDK 調用者可以鏡像`iroha da get-blob/prove-availability` 無定制管道.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascript/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* `sorafsGatewayFetch` 記分板元數據現在在使用網關提供商時記錄網關清單 id/CID，以便採用工件與 CLI 捕獲保持一致。 【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* 強制 ISO 人行橫道：Torii 現在拒絕具有未知代理 BIC 的 `pacs.008` 提交，並且 DvP CLI 預覽通過以下方式驗證 `--delivery-instrument-id` `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* 通過 `POST /v1/iso20022/pacs009` 添加 PvP 現金攝取，在構建轉賬之前強制執行 `Purp=SECU` 和 BIC 參考數據檢查。 【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *（工具）* 添加了 `cargo xtask iso-bridge-lint`（加上 `ci/check_iso_reference_data.sh`）以驗證 ISIN/CUSIP、BIC↔LEI 和 MIC 快照以及存儲庫固定裝置。 【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* 通過聲明存儲庫元數據、顯式文件白名單、啟用來源的 `publishConfig`、`prepublishOnly` 變更日誌/測試防護以及在以下位置執行 Node 18/20 的 GitHub Actions 工作流程來強化 npm 發布CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* BN254 字段 add/sub/mul 現在通過 `bn254_launch_kernel` 在新的 CUDA 內核上執行，並通過主機端批處理，為 Poseidon 和 ZK 小工具啟用硬件加速，同時保持確定性回退。 【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 特點

- *(cli)* 添加 `iroha transaction get` 和其他重要命令 (#5289)
- [**破壞**]分離可替代和不可替代資產（#5308）
- [**break**] 通過允許後面有空塊來完成非空塊 (#5320)
- 在架構和客戶端中公開遙測類型 (#5387)
- *(iroha_torii)* 用於功能門控端點的存根 (#5385)
- 添加提交時間指標 (#5380)

### 🐛 錯誤修復

- 修改 NonZeros (#5278)
- 文檔文件中的拼寫錯誤 (#5309)
- *（加密）* 暴露 `Signature::payload` getter (#5302) (#5310)
- *（核心）* 在授予角色之前添加角色存在檢查（#5300）
- *（核心）* 重新連接斷開的對等點 (#5325)
- 修復與商店資產和 NFT 相關的 pytests (#5341)
- *(CI)* 修復詩歌 v2 的 python 靜態分析工作流程 (#5374)
- 提交後出現過期事務事件 (#5396)

### 💼 其他

- 包括 `rust-toolchain.toml` (#5376)
- 對 `unused` 發出警告，而不是 `deny` (#5377)

### 🚜 重構

- 傘 Iroha CLI (#5282)
- *(iroha_test_network)* 使用漂亮的日誌格式 (#5331)
- [**突破**] 簡化 `genesis.json` 中 `NumericSpec` 的序列化 (#5340)
- 改進 p2p 連接失敗的日誌記錄 (#5379)
- 恢復 `logger.level`，添加 `logger.filter`，擴展配置路由 (#5384)

### 📚 文檔

- 將 `network.public_address` 添加到 `peer.template.toml` (#5321)

### ⚡ 性能

- *(kura)* 防止冗餘塊寫入磁盤 (#5373)
- 為交易哈希實現自定義存儲（#5405）

### ⚙️ 雜項任務

- 修復詩歌的使用（#5285）
- 從 `iroha_torii_const` 中刪除冗餘常量 (#5322)
- 刪除未使用的 `AssetEvent::Metadata*` (#5339)
- 碰撞 Sonarqube 動作版本 (#5337)
- 刪除未使用的權限 (#5346)
- 將解壓包添加到 ci-image (#5347)
- 修復一些評論 (#5397)
- 將集成測試從 `iroha` 箱中移出 (#5393)
- 禁用defectdojo工作（#5406）
- 為缺失的提交添加 DCO 簽核
- 重新組織工作流程（第二次嘗試）（#5399）
- 不要在推送到主程序時運行 Pull Request CI (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### 添加

- 通過在其後允許空塊來最終確定非空塊（#5320）

## [2.0.0-rc.1.2] - 2025-02-25

### 已修復

- 重新註冊的對等點現在可以正確反映在對等點列表中（#5327）

## [2.0.0-rc.1.1] - 2025-02-12

### 添加

- 添加 `iroha transaction get` 和其他重要命令 (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### 添加- 實現查詢預測（#5242）
- 使用持久執行器 (#5082)
- 向 iroha cli 添加監聽超時 (#5241)
- 將 /peers API 端點添加到 torii (#5235)
- 地址不可知的 p2p (#5176)
- 提高多重簽名實用性和可用性 (#5027)
- 保護 `BasicAuth::password` 不被打印 (#5195)
- 在 `FindTransactions` 查詢中降序排序 (#5190)
- 將塊頭引入每個智能合約執行上下文中（#5151）
- 基於視圖更改索引的動態提交時間 (#4957)
- 定義默認權限集 (#5075)
- 添加 `Option<Box<R>>` 的 Niche 實現 (#5094)
- 交易和塊謂詞 (#5025)
- 報告查詢中剩餘項目的數量（#5016）
- 有界離散時間 (#4928)
- 將缺失的數學運算添加到 `Numeric` (#4976)
- 驗證塊同步消息（#4965）
- 查詢過濾器（#4833）

### 已更改

- 簡化對等 ID 解析 (#5228)
- 將交易錯誤移出塊有效負載（#5118）
- 將 JsonString 重命名為 Json (#5154)
- 將客戶端實體添加到智能合約中（#5073）
- 作為交易排序服務的領導者（#4967）
- 讓 kura 從內存中刪除舊塊 (#5103)
- 使用 `ConstVec` 來獲取 `Executable` 中的指令 (#5096)
- 最多發送一次八卦 (#5079)
- 減少 `CommittedTransaction` 的內存使用量 (#5089)
- 使查詢游標錯誤更加具體（#5086）
- 重組板條箱（#4970）
- 引入 `FindTriggers` 查詢，刪除 `FindTriggerById` (#5040)
- 不依賴簽名進行更新 (#5039)
- 更改 genesis.json 中的參數格式 (#5020)
- 只發送當前和之前的視圖更改證明 (#4929)
- 未準備好時禁用發送消息以防止繁忙循環 (#5032)
- 將總資產數量移至資產定義（#5029）
- 僅簽署塊的標頭，而不簽署整個有效負載 (#5000)
- 使用 `HashOf<BlockHeader>` 作為塊哈希的類型 (#4998)
- 簡化 `/health` 和 `/api_version` (#4960)
- 將 `configs` 重命名為 `defaults`，刪除 `swarm` (#4862)

### 已修復

- 扁平化 json 中的內部角色 (#5198)
- 修復 `cargo audit` 警告 (#5183)
- 添加範圍檢查到簽名索引（#5157）
- 修復文檔中的模型宏示例 (#5149)
- 在塊/事件流中正確關閉 ws (#5101)
- 損壞的可信對等點檢查 (#5121)
- 檢查下一個塊的高度是否+1 (#5111)
- 修復創世塊的時間戳（#5098）
- 修復沒有 `transparent_api` 功能的 `iroha_genesis` 編譯 (#5056)
- 正確處理 `replace_top_block` (#4870)
- 修復執行器克隆 (#4955)
- 顯示更多錯誤詳細信息 (#4973)
- 對塊流使用 `GET` (#4990)
- 改進隊列事務處理（#4947）
- 防止冗餘的塊同步塊消息（#4909）
- 防止同時發送大消息時出現死鎖 (#4948)
- 從緩存中刪除過期的交易 (#4922)
- 修復牌坊 url 的路徑 (#4903)

### 已刪除

- 從客戶端刪除基於模塊的 api (#5184)
- 刪除 `riffle_iter` (#5181)
- 刪除未使用的依賴項 (#5173)
- 從 `blocks_in_memory` 中刪除 `max` 前綴 (#5145)
- 刪除共識估計（#5116）
- 從塊中刪除 `event_recommendations` (#4932)

### 安全

## [2.0.0-pre-rc.22.1] - 2024-07-30

### 已修復

- 將 `jq` 添加到 docker 鏡像

## [2.0.0-pre-rc.22.0] - 2024-07-25

### 添加

- 在創世中明確指定鏈上參數 (#4812)
- 允許渦輪魚具有多個 `Instruction` (#4805)
- 重新實現多重簽名交易（#4788）
- 實現內置與自定義鏈上參數（#4731）
- 改進自定義指令的使用（#4778）
- 通過實現 JsonString 使元數據動態化 (#4732)
- 允許多個節點提交創世塊（#4775）
- 向對等方提供 `SignedBlock` 而不是 `SignedTransaction` (#4739)
- 執行器中的自定義指令 (#4645)
- 擴展客戶端 cli 來請求 json 查詢 (#4684)
 - 添加對 `norito_decoder` 的檢測支持 (#4680)
- 將權限模式推廣到執行者數據模型 (#4658)
- 在默認執行器中添加了註冊觸發器權限（#4616）
 - 支持 `norito_cli` 中的 JSON
- 引入p2p空閒超時

### 已更改

- 將 `lol_alloc` 替換為 `dlmalloc` (#4857)
- 在架構中將 `type_` 重命名為 `type` (#4855)
- 將架構中的 `Duration` 替換為 `u64` (#4841)
- 使用類似 `RUST_LOG` 的 EnvFilter 進行日誌記錄 (#4837)
- 盡可能保留投票塊 (#4828)
- 從經線遷移到阿克蘇姆（#4718）
- 分割執行器數據模型（#4791）
- 淺層數據模型 (#4734) (#4792)
- 不要發送帶簽名的公鑰 (#4518)
- 將 `--outfile` 重命名為 `--out-file` (#4679)
- 重命名 iroha 服務器和客戶端 (#4662)
- 將 `PermissionToken` 重命名為 `Permission` (#4635)
- 急切地拒絕 `BlockMessages` (#4606)
- 使 `SignedBlock` 不可變 (#4620)
- 將 TransactionValue 重命名為 CommiedTransaction (#4610)
- 通過 ID 驗證個人帳戶 (#4411)
- 對私鑰使用多重哈希格式 (#4541)
 - 將 `parity_scale_decoder` 重命名為 `norito_cli`
- 將區塊發送到 Set B 驗證器
- 使 `Role` 透明 (#4886)
- 從標頭導出塊哈希 (#4890)

### 已修復

- 檢查權限是否擁有要轉移的域 (#4807)
- 刪除記錄器雙重初始化（#4800）
- 修復資產和權限的命名約定 (#4741)
- 在創世塊中的單獨交易中升級執行器（#4757）
- `JsonString` 的正確默認值 (#4692)
- 改進反序列化錯誤消息（#4659）
- 如果傳遞的 Ed25519Sha512 公鑰長度無效，請不要驚慌 (#4650)
- 在初始化塊加載上使用正確的視圖更改索引（#4612）
- 不要在 `start` 時間戳之前過早執行時間觸發器 (#4333)
- 支持 `https` 為 `torii_url` (#4601) (#4617)
- 從 SetKeyValue/RemoveKeyValue 中刪除 serde(flatten) (#4547)
- 觸發器集已正確序列化
- 撤銷在 `Upgrade<Executor>` 上刪除 `PermissionToken` (#4503)
- 報告當前回合的正確視圖變化索引
- 刪除 `Unregister<Domain>` 上相應的觸發器 (#4461)
- 在創世輪中檢查創世公鑰
- 阻止註冊創世域或帳戶
- 刪除角色對實體註銷的權限
- 觸發元數據可在智能合約中訪問
- 使用 rw 鎖來防止不一致的狀態視圖 (#4867)
- 處理快照中的軟分叉 (#4868)
- 修復 ChaCha20Poly1305 的最小尺寸
- 對 LiveQueryStore 添加限制以防止內存使用率過高 (#4893)

### 已刪除

- 從 ed25519 私鑰中刪除公鑰 (#4856)
- 刪除 kura.lock (#4849)
- 恢復配置中的 `_ms` 和 `_bytes` 後綴 (#4667)
- 從創世字段中刪除 `_id` 和 `_file` 後綴 (#4724)
- 通過 AssetDefinitionId 刪除 AssetsMap 中的索引資產 (#4701)
- 從觸發器身份中刪除域 (#4640)
- 從 Iroha 中刪除創世簽名 (#4673)
- 刪除 `Validate` 中綁定的 `Visit` (#4642)
- 刪除 `TriggeringEventFilterBox` (#4866)
- 刪除 p2p 握手中的 `garbage` (#4889)
- 從塊中刪除 `committed_topology` (#4880)

### 安全

- 防止秘密洩露

## [2.0.0-pre-rc.21] - 2024-04-19

### 添加

- 在觸發器入口點中包含觸發器 ID (#4391)
- 將事件集公開為架構中的位字段 (#4381)
- 引入具有精細訪問權限的新 `wsv` (#2664)
- 添加 `PermissionTokenSchemaUpdate`、`Configuration` 和 `Executor` 事件的事件過濾器
- 引入快照“模式”(#4365)
- 允許授予/撤銷角色的權限（#4244）
- 為資產引入任意精度數字類型（刪除所有其他數字類型）（#3660）
- 執行器的不同燃料限制（#3354）
- 集成 pprof 分析器 (#4250)
- 在客戶端 CLI 中添加 asset 子命令 (#4200)
- `Register<AssetDefinition>` 權限 (#4049)
- 添加 `chain_id` 以防止重放攻擊 (#4185)
- 添加子命令以在客戶端 CLI 中編輯域元數據 (#4175)
- 在客戶端 CLI 中實現存儲設置、刪除、獲取操作 (#4163)
- 計算觸發器的相同智能合約（#4133）
- 將子命令添加到客戶端 CLI 中以傳輸域 (#3974)
- 支持 FFI 中的盒裝切片 (#4062)
- git commit SHA 到客戶端 CLI (#4042)
- 默認驗證器樣板的 proc 宏 (#3856)
- 將查詢請求構建器引入客戶端 API (#3124)
- 智能合約內的惰性查詢（#3929）
- `fetch_size` 查詢參數 (#3900)
- 資產商店轉移指令（#4258）
- 防止秘密洩露（#3240）
- 使用相同源代碼刪除重複觸發器 (#4419)

### 已更改- 將 Rust 工具鏈升級為 nightly-2024-04-18
- 將塊發送到 Set B 驗證器 (#4387)
- 將管道事件拆分為塊事件和事務事件 (#4366)
- 將 `[telemetry.dev]` 配置部分重命名為 `[dev_telemetry]` (#4377)
- 使 `Action` 和 `Filter` 非通用類型 (#4375)
- 使用構建器模式改進事件過濾 API (#3068)
- 統一各種事件過濾器API，引入流暢的構建器API
- 將 `FilterBox` 重命名為 `EventFilterBox`
- 將 `TriggeringFilterBox` 重命名為 `TriggeringEventFilterBox`
- 改進過濾器命名，例如`AccountFilter` -> `AccountEventFilter`
- 根據配置 RFC 重寫配置 (#4239)
- 從公共 API 中隱藏版本化結構的內部結構 (#3887)
- 在多次失敗的視圖更改後暫時引入可預測的排序（#4263）
- 在 `iroha_crypto` 中使用具體的密鑰類型 (#4181)
- 分割視圖與普通消息不同 (#4115)
- 使 `SignedTransaction` 不可變 (#4162)
- 通過 `iroha_client` 導出 `iroha_config` (#4147)
- 通過 `iroha_client` 導出 `iroha_crypto` (#4149)
- 通過 `iroha_client` 導出 `data_model` (#4081)
- 從 `iroha_crypto` 中刪除 `openssl-sys` 依賴性，並向 `iroha_client` 引入可配置的 tls 後端 (#3422)
- 用內部解決方案 `iroha_crypto` 替換未維護的 EOF `hyperledger/ursa` (#3422)
- 優化執行器性能（#4013）
- 拓撲對等點更新 (#3995)

### 已修復

- 刪除 `Unregister<Domain>` 上相應的觸發器 (#4461)
- 刪除實體註銷角色的權限 (#4242)
- 斷言創世交易由創世公鑰簽名 (#4253)
- 為 p2p 中無響應的對等點引入超時 (#4267)
- 防止註冊創世域或帳戶 (#4226)
- `MinSize` 為 `ChaCha20Poly1305` (#4395)
- 啟用 `tokio-console` 時啟動控制台 (#4377)
- 用 `\n` 分隔每個項目，並遞歸地為 `dev-telemetry` 文件日誌創建父目錄
- 防止未經簽名的帳戶註冊（#4212）
- 密鑰對生成現在不會出錯 (#4283)
- 停止將 `X25519` 密鑰編碼為 `Ed25519` (#4174)
- 在 `no_std` 中進行簽名驗證 (#4270)
- 在異步上下文中調用阻塞方法（#4211）
- 撤銷實體註銷時的關聯令牌 (#3962)
- 啟動 Sumeragi 時的異步阻塞錯誤
- 修復了 `(get|set)_config` 401 HTTP (#4177)
- Docker 中的 `musl` 存檔器名稱 (#4193)
- 智能合約調試打印（#4178）
- 重啟時拓撲更新 (#4164)
- 新節點的註冊 (#4142)
- 鏈上可預測迭代順序 (#4130)
- 重新構建記錄器和動態配置（#4100）
- 觸發原子性（#4106）
- 查詢存儲消息排序問題 (#4057)
- 為使用 Norito 進行回复的端點設置 `Content-Type: application/x-norito`

### 已刪除

- `logger.tokio_console_address` 配置參數 (#4377)
- `NotificationEvent` (#4377)
- `Value` 枚舉 (#4305)
- iroha 的 MST 聚合 (#4229)
- 智能合約中 ISI 和查詢執行的克隆 (#4182)
- `bridge` 和 `dex` 功能 (#4152)
- 扁平化事件（#3068）
- 表達式 (#4089)
- 自動生成的配置參考
- `warp` 日誌中的噪音 (#4097)

### 安全

- 防止 p2p 中的 pub 密鑰欺騙 (#4065)
- 確保來自 OpenSSL 的 `secp256k1` 簽名標準化 (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### 添加

- 轉讓`Domain`所有權
- `Domain` 所有者權限
- 將 `owned_by` 字段添加到 `Domain`
- 將 `iroha_client_cli` 中的過濾器解析為 JSON5 (#3923)
- 添加對在 serde 部分標記的枚舉中使用 Self 類型的支持
- 標準化區塊 API (#3884)
- 實現 `Fast` kura 初始化模式
- 添加 iroha_swarm 免責聲明標頭
- 初步支持 WSV 快照

### 已修復

- 修復 update_configs.sh 中的執行程序下載 (#3990)
- devShell 中正確的 rustc
- 修復刻錄 `Trigger` 重述
- 修復傳輸 `AssetDefinition`
- 修復 `RemoveKeyValue` 為 `Domain`
- 修復 `Span::join` 的使用
- 修復拓撲不匹配錯誤 (#3903)
- 修復 `apply_blocks` 和 `validate_blocks` 基準測試
- `mkdir -r` 具有存儲路徑，而不是鎖定路徑 (#3908)
- 如果 test_env.py 中存在 dir，則不會失敗
- 修復身份驗證/授權文檔字符串 (#3876)
- 更好的查詢查找錯誤的錯誤消息
- 將創世賬戶公鑰添加到 dev docker compose
- 將權限令牌有效負載與 JSON 進行比較 (#3855)
- 修復 `#[model]` 宏中的 `irrefutable_let_patterns`
- 允許創世執行任何 ISI (#3850)
- 修復創世驗證（#3844）
- 修復 3 個或更少對等點的拓撲
- 更正 tx_amounts 直方圖的計算方式。
- `genesis_transactions_are_validated()` 測試片狀
- 默認驗證器生成
- 修復 iroha 正常關機問題

### 重構

- 刪除未使用的依賴項 (#3992)
- 凹凸依賴項 (#3981)
- 將驗證器重命名為執行器 (#3976)
- 刪除 `IsAssetDefinitionOwner` (#3979)
- 將智能合約代碼包含到工作區中 (#3944)
- 將 API 和遙測端點合併到單個服務器中
- 將表達式 len 從公共 API 移至核心 (#3949)
- 避免角色查找中的克隆
- 角色範圍查詢
- 將帳戶角色移至 `WSV`
- 將 ISI 從 *Box 重命名為 *Expr (#3930)
- 從版本化容器中刪除“Versioned”前綴 (#3913)
- 將 `commit_topology` 移動到塊有效負載中 (#3916)
- 將 `telemetry_future` 宏遷移到 syn 2.0
- 在 ISI 範圍內註冊可識別 (#3925)
- 為 `derive(HasOrigin)` 添加基本泛型支持
- 清理 Emitter API 文檔以使 Clippy 滿意
- 添加對導出（HasOrigin）宏的測試，減少導出（IdEqOrdHash）中的重複，修復穩定版上的錯誤報告
- 改進命名，簡化重複的 .filter_maps 並刪除derive(Filter)中不必要的 . except
- 讓 PartiallyTaggedSerialize/Deserialize 使用親愛的
- 讓derive(IdEqOrdHash)使用親愛的，添加測試
- 讓衍生（過濾器）使用親愛的
- 更新 iroha_data_model_derive 以使用 syn 2.0
- 添加簽名檢查條件單元測試
- 只允許一組固定的簽名驗證條件
- 將 ConstBytes 推廣到保存任何 const 序列的 ConstVec
- 對不改變的字節值使用更有效的表示
- 將最終的 wsv 存儲在快照中
- 添加 `SnapshotMaker` 演員
- 解析的文檔限制源自 proc 宏
- 清理評論
- 提取用於解析 lib.rs 屬性的通用測試實用程序
- 使用 parse_display 並更新 Attr -> Attrs 命名
- 允許在 ffi 函數參數中使用模式匹配
- 減少 getset attrs 解析中的重複
- 將 Emitter::into_token_stream 重命名為 Emitter::finish_token_stream
- 使用parse_display來解析getset標記
- 修復拼寫錯誤並改進錯誤消息
- iroha_ffi_derive：使用darling解析屬性並使用syn 2.0
- iroha_ffi_derive：用 Manyhow 替換 proc-macro-error
- 簡化 kura 鎖文件代碼
- 使所有數值序列化為字符串文字
- 分離出 Kagami (#3841)
- 重寫`scripts/test-env.sh`
- 區分智能合約和触發入口點
- 在 `data_model/src/block.rs` 中刪除 `.cloned()`
- 更新 `iroha_schema_derive` 以使用 syn 2.0

## [2.0.0-pre-rc.19] - 2023-08-14

### 添加

- hyperledger#3309 Bump IVM 運行時改進
- hyperledger#3383 實現宏以在編譯時解析套接字地址
- hyperledger#2398 添加查詢過濾器的集成測試
- 在 `InternalError` 中包含實際錯誤消息
- 使用 `nightly-2023-06-25` 作為默認工具鏈
- hyperledger#3692 驗證器遷移
- [DSL實習] hyperledger#3688：將基本算術實現為 proc 宏
- hyperledger#3371 拆分驗證器 `entrypoint` 以確保驗證器不再被視為智能合約
- hyperledger#3651 WSV 快照，允許在崩潰後快速啟動 Iroha 節點
- hyperledger#3752 將 `MockValidator` 替換為接受所有交易的 `Initial` 驗證器
- hyperledger#3276 添加名為 `Log` 的臨時指令，該指令將指定字符串記錄到 Iroha 節點的主日誌中
- hyperledger#3641 使權限令牌有效負載易於理解
- hyperledger#3324 添加 `iroha_client_cli` 相關的 `burn` 檢查和重構
- hyperledger#3781 驗證創世交易
- hyperledger#2885 區分可以和不能用於觸發器的事件
- hyperledger#2245 基於 `Nix` 的 iroha 節點二進製文件構建為 `AppImage`

### 已修復- hyperledger#3613 回歸可能允許接受錯誤簽名的交易
- 儘早拒絕不正確的配置拓撲
- hyperledger#3445 修復回歸併使 `/configuration` 端點上的 `POST` 再次工作
- hyperledger#3654 修復要部署的基於 `iroha2` `glibc` 的 `Dockerfiles`
- hyperledger#3451 修復 Apple Silicon Mac 上的 `docker` 構建
- hyperledger#3741 修復 `kagami validator` 中的 `tempfile` 錯誤
- hyperledger#3758 修復無法構建單個板條箱的回歸，但可以將其構建為工作區的一部分
- hyperledger#3777 角色註冊中的補丁漏洞未得到驗證
- hyperledger#3805 修復 Iroha 收到 `SIGTERM` 後不關閉的問題

### 其他

- hyperledger#3648 在 CI 流程中包含 `docker-compose.*.yml` 檢查
- 將指令 `len()` 從 `iroha_data_model` 移至 `iroha_core`
- hyperledger#3672 在派生宏中將 `HashMap` 替換為 `FxHashMap`
- hyperledger#3374 統一錯誤的文檔註釋和 `fmt::Display` 實現
- hyperledger#3289 在整個項目中使用 Rust 1.70 工作區繼承
- hyperledger#3654 添加 `Dockerfiles` 以在 `GNU libc <https://www.gnu.org/software/libc/>`_ 上構建 iroha2
- 為 proc 宏引入 `syn` 2.0、`manyhow` 和 `darling`
- hyperledger#3802 Unicode `kagami crypto` 種子

## [2.0.0-rc.18 之前]

### 添加

- hyperledger#3468：服務器端游標，允許延遲評估可重入分頁，這應該對查詢延遲產生重大的積極性能影響
- hyperledger#3624：通用權限令牌；具體來說
  - 權限令牌可以具有任何結構
  - 令牌結構在 `iroha_schema` 中進行自我描述并序列化為 JSON 字符串
  - 令牌值是 `Norito` 編碼的
  - 由於此更改，權限令牌命名約定從 `snake_case` 移至 `UpeerCamelCase`
- hyperledger#3615 驗證後保留 wsv

### 已修復

- hyperledger#3627 現在通過克隆 `WorlStateView` 強制執行事務原子性
- hyperledger#3195 擴展接收被拒絕的創世交易時的恐慌行為
- hyperledger#3042 修復錯誤的請求消息
- hyperledger#3352 將控制流和數據消息拆分到單獨的通道中
- hyperledger#3543 提高指標精度

## 2.0.0-rc.17 之前

### 添加

- hyperledger#3330 擴展 `NumericValue` 反序列化
- FFI 中的 hyperledger#2622 `u128`/`i128` 支持
- hyperledger#3088 引入隊列限制，以防止 DoS
- hyperledger#2373 `kagami swarm file` 和 `kagami swarm dir` 用於生成 `docker-compose` 文件的命令變體
- hyperledger#3597 權限令牌分析（Iroha 側）
- hyperledger#3353 通過枚舉錯誤條件並使用強類型錯誤，從 `block.rs` 中刪除 `eyre`
- hyperledger#3318 Interleave 塊中拒絕和接受的交易以保留交易處理順序

### 已修復

- hyperledger#3075 對 `genesis.json` 中的無效交易產生恐慌，以防止處理無效交易
- hyperledger#3461 正確處理默認配置中的默認值
- hyperledger#3548 修復 `IntoSchema` 透明屬性
- hyperledger#3552 修復驗證器路徑模式表示
- hyperledger#3546 修復時間觸發器卡住的問題
- hyperledger#3162 禁止塊流請求中的高度為 0
- 配置宏初始測試
- hyperledger#3592 修復了 `release` 上更新的配置文件
- hyperledger#3246 不要涉及沒有 `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ 的 `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_
- hyperledger#3570 正確顯示客戶端字符串查詢錯誤
- hyperledger#3596 `iroha_client_cli` 顯示塊/事件
- hyperledger#3473 使 `kagami validator` 從 iroha 存儲庫根目錄外部工作

### 其他

- hyperledger#3063 將交易 `hash` 映射到 `wsv` 中的區塊高度
- `Value` 中的強類型 `HashOf<T>`

## [2.0.0-rc.16 之前]

### 添加

- hyperledger#2373 `kagami swarm` 用於生成 `docker-compose.yml` 的子命令
- hyperledger#3525 標準化交易 API
- hyperledger#3376 添加 Iroha 客戶端 CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ 自動化框架
- hyperledger#3516 在 `LoadedExecutable` 中保留原始 blob 哈希值

### 已修復

- hyperledger#3462 將 `burn` 資產命令添加到 `client_cli`
- hyperledger#3233 重構錯誤類型
- hyperledger#3330 通過手動為 `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` 實現 `serde::de::Deserialize` 修復回歸
- hyperledger#3487 將缺失的類型返回到模式中
- hyperledger#3444 將判別式返回到模式中
- hyperledger#3496 修復 `SocketAddr` 字段解析
- hyperledger#3498 修復軟分叉檢測
- hyperledger#3396 在發出塊提交事件之前將塊存儲在 `kura` 中

### 其他

- hyperledger#2817 從 `WorldStateView` 中刪除內部可變性
- hyperledger#3363 Genesis API 重構
- 重構現有拓撲並補充新的拓撲測試
- 從 `Codecov <https://about.codecov.io/>`_ 切換到 `Coveralls <https://coveralls.io/>`_ 以進行測試覆蓋
- hyperledger#3533 在架構中將 `Bool` 重命名為 `bool`

## [2.0.0-rc.15 之前]

### 添加

- hyperledger#3231 整體驗證器
- hyperledger#3015 支持 FFI 中的利基優化
- hyperledger#2547 將徽標添加到 `AssetDefinition`
- hyperledger#3274 在 `kagami` 中添加一個生成示例的子命令（向後移植到 LTS）
- hyperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ 薄片
- hyperledger#3412 將交易八卦移至單獨的參與者
- hyperledger#3435 引入 `Expression` 訪客
- hyperledger#3168 提供創世驗證器作為單獨的文件
- hyperledger#3454 將 LTS 設置為大多數 Docker 操作和文檔的默認設置
- hyperledger#3090 將鏈上參數從區塊鏈傳播到 `sumeragi`

### 已修復

- hyperledger#3330 使用 `u128` 葉修復未標記枚舉反序列化（向後移植到 RC14）
- hyperledger#2581 減少了日誌中的噪音
- hyperledger#3360 修復 `tx/s` 基準
- hyperledger#3393 打破 `actors` 中的通信死鎖循環
- hyperledger#3402 修復 `nightly` 版本
- hyperledger#3411 正確處理對等點同時連接
- hyperledger#3440 不贊成在轉移期間進行資產轉換，而是由智能合約處理
- hyperledger#3408：修復 `public_keys_cannot_be_burned_to_nothing` 測試

### 其他

- hyperledger#3362 遷移到 `tokio` 參與者
- hyperledger#3349 從智能合約中刪除 `EvaluateOnHost`
- hyperledger#1786 為套接字地址添加 `iroha` 本機類型
- 禁用 IVM 緩存
- 重新啟用 IVM 緩存
- 將權限驗證器重命名為驗證器
- hyperledger#3388 使 `model!` 成為模塊級屬性宏
- hyperledger#3370 將 `hash` 序列化為十六進製字符串
- 將 `maximum_transactions_in_block` 從 `queue` 移動到 `sumeragi` 配置
- 棄用並刪除 `AssetDefinitionEntry` 類型
- 將 `configs/client_cli` 重命名為 `configs/client`
- 更新 `MAINTAINERS.md`

## [2.0.0-rc.14 之前]

### 添加

- hyperledger#3127 數據模型 `structs` 默認情況下不透明
- hyperledger#3122 使用 `Algorithm` 存儲摘要函數（社區貢獻者）
- hyperledger#3153 `iroha_client_cli` 輸出是機器可讀的
- hyperledger#3105 為 `AssetDefinition` 實現 `Transfer`
- 添加了 hyperledger#3010 `Transaction` 過期管道事件

### 已修復

- hyperledger#3113 不穩定網絡測試的修訂版
- hyperledger#3129 修復 `Parameter` 反/序列化
- hyperledger#3141 為 `Hash` 手動實現 `IntoSchema`
- hyperledger#3155 修復測試中的恐慌鉤子，防止死鎖
- hyperledger#3166 不要在空閒時查看更改，提高性能
- hyperledger#2123 從多重哈希返回公鑰反/序列化
- hyperledger#3132 添加 NewParameter 驗證器
- hyperledger#3249 將塊哈希拆分為部分和完整版本
- hyperledger#3031 修復缺少配置參數的 UI/UX
- hyperledger#3247 從 `sumeragi` 中刪除了故障注入。

### 其他

- 添加缺失的 `#[cfg(debug_assertions)]` 以修復虛假故障
- hyperledger#2133 重寫拓撲以更接近白皮書
- 刪除 `iroha_client` 對 `iroha_core` 的依賴
- hyperledger#2943 派生 `HasOrigin`
- hyperledger#3232 共享工作區元數據
- hyperledger#3254 重構 `commit_block()` 和 `replace_top_block()`
- 使用穩定的默認分配器處理程序
- hyperledger#3183 重命名 `docker-compose.yml` 文件
- 改進了`Multihash`顯示格式
- hyperledger#3268 全球唯一的項目標識符
- 新的公關模板

## [2.0.0-rc.13 之前]

### 添加- hyperledger#2399 將參數配置為 ISI。
- hyperledger#3119 添加 `dropped_messages` 指標。
- hyperledger#3094 與 `n` 對等點生成網絡。
- hyperledger#3082 在 `Created` 事件中提供完整數據。
- hyperledger#3021 不透明指針導入。
- hyperledger#2794 在 FFI 中拒絕具有顯式判別式的無字段枚舉。
- hyperledger#2922 將 `Grant<Role>` 添加到默認創世。
- hyperledger#2922 省略 `NewRole` json 反序列化中的 `inner` 字段。
- hyperledger#2922 在 json 反序列化中省略 `object(_id)`。
- hyperledger#2922 在 json 反序列化中省略 `Id`。
- hyperledger#2922 在 json 反序列化中省略 `Identifiable`。
- hyperledger#2963 將 `queue_size` 添加到指標中。
- hyperledger#3027 為 Kura 實現鎖定文件。
- hyperledger#2813 Kagami 生成默認對等配置。
- hyperledger#3019 支持 JSON5。
- hyperledger#2231 生成 FFI 包裝器 API。
- hyperledger#2999 累積區塊簽名。
- hyperledger#2995 軟分叉檢測。
- hyperledger#2905 擴展算術運算以支持 `NumericValue`
- hyperledger#2868 發出 iroha 版本並在日誌中提交哈希值。
- hyperledger#2096 查詢資產總量。
- hyperledger#2899 將多指令子命令添加到“client_cli”中
- hyperledger#2247 消除 websocket 通信噪音。
- hyperledger#2889 在 `iroha_client` 中添加塊流支持
- hyperledger#2280 授予/撤銷角色時生成權限事件。
- hyperledger#2797 豐富事件。
- hyperledger#2725 將超時重新引入 `submit_transaction_blocking`
- hyperledger#2712 配置屬性測試。
- hyperledger#2491 FFi 中的枚舉支持。
- hyperledger#2775 在合成創世中生成不同的密鑰。
- hyperledger#2627 配置完成、代理入口點、kagami docgen。
- hyperledger#2765 在 `kagami` 中生成合成創世
- hyperledger#2698 修復 `iroha_client` 中不清楚的錯誤消息
- hyperledger#2689 添加權限令牌定義參數。
- hyperledger#2502 存儲構建的 GIT 哈希值。
- hyperledger#2672 添加 `ipv4Addr`、`ipv6Addr` 變體和謂詞。
- hyperledger#2626 實現 `Combine` 派生、拆分 `config` 宏。
- hyperledger#2586 `Builder` 和 `LoadFromEnv` 用於代理結構。
- hyperledger#2611 為通用不透明結構派生 `TryFromReprC` 和 `IntoFfi`。
- hyperledger#2587 將 `Configurable` 拆分為兩個特徵。 ＃2587：將 `Configurable` 分成兩個特徵
- hyperledger#2488 在 `ffi_export` 中添加對特徵實現的支持
- hyperledger#2553 向資產查詢添加排序。
- hyperledger#2407 參數化觸發器。
- hyperledger#2536 為 FFI 客戶端引入 `ffi_import`。
- hyperledger#2338 添加 `cargo-all-features` 檢測。
- hyperledger#2564 Kagami 工具算法選項。
- hyperledger#2490 為獨立函數實現 ffi_export。
- hyperledger#1891 驗證觸發器執行。
- hyperledger#1988 派生可識別、Eq、Hash、Ord 的宏。
- hyperledger#2434 FFI 綁定生成庫。
- hyperledger#2073 對於區塊鏈中的類型，更喜歡 ConstString 而不是 String。
- hyperledger#1889 添加域範圍的觸發器。
- hyperledger#2098 區塊頭查詢。 ＃2098：添加塊頭查詢
- hyperledger#2467 將帳戶授予子命令添加到 iroha_client_cli 中。
- hyperledger#2301 在查詢時添加交易的塊哈希。
 - hyperledger#2454 將構建腳本添加到 Norito 解碼器工具。
- hyperledger#2061 導出過濾器宏。
- hyperledger#2228 將未經授權的變體添加到智能合約查詢錯誤。
- hyperledger#2395 如果無法應用創世，請添加恐慌。
- hyperledger#2000 禁止空名稱。 #2000：禁止空名稱
 - hyperledger#2127 添加健全性檢查，以確保消耗由 Norito 編解碼器解碼的所有數據。
- hyperledger#2360 再次使 `genesis.json` 可選。
- hyperledger#2053 向私有區塊鏈中的所有剩餘查詢添加測試。
- hyperledger#2381 統一 `Role` 註冊。
- hyperledger#2053 添加對私有區塊鏈中資產相關查詢的測試。
- hyperledger#2053 將測試添加到“private_blockchain”
- hyperledger#2302 添加“FindTriggersByDomainId”存根查詢。
- hyperledger#1998 向查詢添加過濾器。
- hyperledger#2276 將當前塊哈希包含到 BlockHeaderValue 中。
- hyperledger#2161 句柄 id 和共享 FFI fns。
- 添加句柄 id 並實現共享特徵的 FFI 等效項（Clone、Eq、Ord）
- hyperledger#1638 `configuration` 返回文檔子樹。
- hyperledger#2132 添加 `endpointN` 過程宏。
- hyperledger#2257 Revoke<Role> 發出 RoleRevoked 事件。
- hyperledger#2125 添加 FindAssetDefinitionById 查詢。
- hyperledger#1926 添加信號處理和正常關閉。
- hyperledger#2161 為 `data_model` 生成 FFI 函數
- hyperledger#1149 每個目錄的塊文件計數不超過 1000000。
- hyperledger#1413 添加 API 版本端點。
- hyperledger#2103 支持查詢區塊和交易。添加 `FindAllTransactions` 查詢
- hyperledger#2186 為 `BigQuantity` 和 `Fixed` 添加傳輸 ISI。
- hyperledger#2056 為 `AssetValueType` `enum` 添加派生過程宏包。
- hyperledger#2100 添加查詢以查找所有擁有資產的賬戶。
- hyperledger#2179 優化觸發器執行。
- hyperledger#1883 刪除嵌入的配置文件。
- hyperledger#2105 處理客戶端中的查詢錯誤。
- hyperledger#2050 添加與角色相關的查詢。
- hyperledger#1572 專門的權限令牌。
- hyperledger#2121 檢查密鑰對在構造時是否有效。
 - hyperledger#2003 引入 Norito 解碼器工具。
- hyperledger#1952 添加 TPS 基準作為優化標準。
- hyperledger#2040 添加具有事務執行限制的集成測試。
- hyperledger#1890 引入基於 Orillion 用例的集成測試。
- hyperledger#2048 添加工具鏈文件。
- hyperledger#2100 添加查詢以查找所有擁有資產的賬戶。
- hyperledger#2179 優化觸發器執行。
- hyperledger#1883 刪除嵌入的配置文件。
- hyperledger#2004 禁止 `isize` 和 `usize` 變為 `IntoSchema`。
- hyperledger#2105 處理客戶端中的查詢錯誤。
- hyperledger#2050 添加與角色相關的查詢。
- hyperledger#1572 專門的權限令牌。
- hyperledger#2121 檢查密鑰對在構造時是否有效。
 - hyperledger#2003 引入 Norito 解碼器工具。
- hyperledger#1952 添加 TPS 基準作為優化標準。
- hyperledger#2040 添加具有事務執行限制的集成測試。
- hyperledger#1890 引入基於 Orillion 用例的集成測試。
- hyperledger#2048 添加工具鏈文件。
- hyperledger#2037 引入預提交觸發器。
- hyperledger#1621 引入調用觸發器。
- hyperledger#1970 添加可選的模式端點。
- hyperledger#1620 引入基於時間的觸發器。
- hyperledger#1918 實現 `client` 的基本身份驗證
- hyperledger#1726 實施發布 PR 工作流程。
- hyperledger#1815 使查詢響應更加類型結構化。
- hyperledger#1928 使用 `gitchangelog` 實現變更日誌生成
- hyperledger#1902 裸機 4 點設置腳本。

  添加了 setup_test_env.sh 的版本，該版本不需要 docker-compose 並使用 Iroha 的調試版本。
- hyperledger#1619 引入基於事件的觸發器。
- hyperledger#1195 乾淨地關閉 websocket 連接。
- hyperledger#1606 在域結構中添加 ipfs 鏈接到域徽標。
- hyperledger#1754 添加 Kura 檢查器 CLI。
- hyperledger#1790 通過使用基於堆棧的向量提高性能。
- hyperledger#1805 用於緊急錯誤的可選終端顏色。
- `data_model` 中的超級賬本#1749 `no_std`
- hyperledger#1179 添加撤銷權限或角色指令。
- hyperledger#1782 使 iroha_crypto no_std 兼容。
- hyperledger#1172 實現指令事件。
- hyperledger#1734 驗證 `Name` 以排除空格。
- hyperledger#1144 添加元數據嵌套。
- #1210 塊流（服務器端）。
- hyperledger#1331 實施更多 `Prometheus` 指標。
- hyperledger#1689 修復功能依賴性。 ＃1261：添加貨物膨脹。
- hyperledger#1675 對版本化項目使用類型而不是包裝結構。
- hyperledger#1643 等待同行在測試中提交創世。
- 超級賬本#1678 `try_allocate`
- hyperledger#1216 添加 Prometheus 端點。 #1216：指標端點的初始實現。
- hyperledger#1238 運行時日誌級別更新。創建了基本的基於 `connection` 入口點的重新加載。
- hyperledger#1652 PR 標題格式。
- 將已連接對等點的數量添加到 `Status`

  - 恢復“刪除與連接對等點數量相關的內容”

  這將恢復提交 b228b41dab3c035ce9973b6aa3b35d443c082544。
  - 澄清 `Peer` 僅在握手後才具有真正的公鑰
  - `DisconnectPeer` 未經測試
  - 實現取消註冊對等執行
  - 將（取消）註冊對等子命令添加到 `client_cli`
  - 通過其地址拒絕來自未註冊對等點的重新連接在您的對等點取消註冊並斷開另一個對等點的連接後，
  您的網絡將聽到來自對等方的重新連接請求。
  首先你只能知道端口號是任意的地址。
  因此，請通過端口號以外的部分來記住未註冊的對等點
  並拒絕從那裡重新連接
- 將 `/status` 端點添加到特定端口。

### 修復- hyperledger#3129 修復 `Parameter` 反/序列化。
- hyperledger#3109 防止 `sumeragi` 在角色不可知消息後休眠。
- hyperledger#3046 確保 Iroha 可以在空時正常啟動
  `./storage`
- hyperledger#2599 刪除 Nursery lints。
- hyperledger#3087 在視圖更改後從 Set B 驗證器收集投票。
- hyperledger#3056 修復 `tps-dev` 基準測試掛起。
- hyperledger#1170 實現克隆 wsv 風格的軟分叉處理。
- hyperledger#2456 使創世塊不受限制。
- hyperledger#3038 重新啟用多重簽名。
- hyperledger#2894 修復 `LOG_FILE_PATH` env 變量反序列化。
- hyperledger#2803 返回簽名錯誤的正確狀態代碼。
- hyperledger#2963 `Queue` 正確刪除交易。
- hyperledger#0000 Vergen 破壞 CI。
- hyperledger#2165 刪除工具鏈煩躁。
- hyperledger#2506 修復區塊驗證。
- hyperledger#3013 正確的鏈燃燒驗證器。
- hyperledger#2998 刪除未使用的鏈碼。
- hyperledger#2816 將訪問區塊的責任移交給 kura。
- hyperledger#2384 將解碼替換為decode_all。
- hyperledger#1967 將 ValueName 替換為 Name。
- hyperledger#2980 修復區塊值 ffi 類型。
- hyperledger#2858 引入 parking_lot::Mutex 而不是 std。
- hyperledger#2850 修復 `Fixed` 的反序列化/解碼
- hyperledger#2923 當 `AssetDefinition` 不返回時返回 `FindError`
  存在。
- hyperledger#0000 修復 `panic_on_invalid_genesis.sh`
- hyperledger#2880 正確關閉 websocket 連接。
- hyperledger#2880 修復塊流。
- hyperledger#2804 `iroha_client_cli` 提交交易阻塞。
- hyperledger#2819 將非必要成員移出 WSV。
- 修復表達式序列化遞歸錯誤。
- hyperledger#2834 改進速記語法。
- hyperledger#2379 添加將新 Kura 塊轉儲到blocks.txt 的功能。
- hyperledger#2758 將排序結構添加到模式中。
- CI。
- hyperledger#2548 對大型創世文件發出警告。
- hyperledger#2638 更新 `whitepaper` 並傳播更改。
- hyperledger#2678 修復暫存分支上的測試。
- hyperledger#2678 修復了 Kura 強制關閉時測試中止的問題。
- hyperledger#2607 重構 sumeragi 代碼以使其更加簡單和
  穩健性修復。
- hyperledger#2561 重新引入視圖更改以達成共識。
- hyperledger#2560 添加回 block_sync 和對等斷開連接。
- hyperledger#2559 添加 sumeragi 線程關閉。
- hyperledger#2558 在從 kura 更新 wsv 之前驗證創世。
- hyperledger#2465 將 sumeragi 節點重新實現為單線程狀態
  機。
- hyperledger#2449 Sumeragi 重組的初步實施。
- hyperledger#2802 修復配置的環境加載。
- hyperledger#2787 通知每個監聽器在恐慌時關閉。
- hyperledger#2764 刪除最大消息大小的限制。
- #2571：更好的 Kura Inspector UX。
- hyperledger#2703 修復 Orillion 開發環境錯誤。
- 修復 schema/src 中文檔註釋中的拼寫錯誤。
- hyperledger#2716 公開正常運行時間的持續時間。
- hyperledger#2700 在 docker 鏡像中導出 `KURA_BLOCK_STORE_PATH`。
- hyperledger#0 從構建器中刪除 `/iroha/rust-toolchain.toml`
  圖像。
- hyperledger#0 修復 `docker-compose-single.yml`
- hyperledger#2554 如果 `secp256k1` 種子短於 32，則引發錯誤
  字節。
- hyperledger#0 修改 `test_env.sh` 為每個對等點分配存儲。
- hyperledger#2457 在測試中強制關閉 kura。
- hyperledger#2623 修復 VariantCount 的文檔測試。
- 更新 ui_fail 測試中的預期錯誤。
- 修復權限驗證器中不正確的文檔註釋。
- hyperledger#2422 在配置端點響應中隱藏私鑰。
- hyperledger#2492：修復並非所有正在執行的與事件匹配的觸發器。
- hyperledger#2504 修復失敗的 tps 基準測試。
- hyperledger#2477 修復不計算角色權限時的錯誤。
- hyperledger#2416 修復 macOS 手臂上的 lints。
- hyperledger#2457 修復與恐慌關閉相關的測試不穩定問題。
  ＃2457：添加緊急關閉配置
- hyperledger#2473 解析 rustc --version 而不是 RUSTUP_TOOLCHAIN。
- hyperledger#1480 因恐慌而關閉。 ＃1480：添加恐慌掛鉤以在恐慌時退出程序
- hyperledger#2376 簡化的 Kura，無異步，兩個文件。
- hyperledger#0000 Docker 構建失敗。
- hyperledger#1649 從 `do_send` 中刪除 `spawn`
- hyperledger#2128 修復 `MerkleTree` 構造和迭代。
- hyperledger#2137 為多進程上下文準備測試。
- hyperledger#2227 實現資產註冊和註銷。
- hyperledger#2081 修復角色授予錯誤。
- hyperledger#2358 添加帶有調試配置文件的版本。
- hyperledger#2294 將火焰圖生成添加到 oneshot.rs。
- hyperledger#2202 修復查詢響應中的總計字段。
- hyperledger#2081 修復測試用例以授予角色。
- hyperledger#2017 修復角色註銷問題。
- hyperledger#2303 修復 docker-compose' 對等點無法正常關閉的問題。
- hyperledger#2295 修復取消註冊觸發錯誤。
- hyperledger#2282 改進源自 getset 實現的 FFI。
- hyperledger#1149 刪除 nocheckin 代碼。
- hyperledger#2232 當創世有太多 isi 時，使 Iroha 打印有意義的消息。
- hyperledger#2170 修復 M1 機器上 docker 容器中的構建。
- hyperledger#2215 使 nightly-2022-04-20 對於 `cargo build` 成為可選
- hyperledger#1990 在沒有 config.json 的情況下通過環境變量啟用對等啟動。
- hyperledger#2081 修復角色註冊。
- hyperledger#1640 生成 config.json 和 genesis.json。
- hyperledger#1716 修復 f=0 情況下共識失敗的問題。
- hyperledger#1845 不可鑄造的資產只能鑄造一次。
- hyperledger#2005 修復 `Client::listen_for_events()` 未關閉 WebSocket 流。
- hyperledger#1623 創建一個 RawGenesisBlockBuilder。
- hyperledger#1917 添加 easy_from_str_impl 宏。
- hyperledger#1990 在沒有 config.json 的情況下通過環境變量啟用對等啟動。
- hyperledger#2081 修復角色註冊。
- hyperledger#1640 生成 config.json 和 genesis.json。
- hyperledger#1716 修復 f=0 情況下共識失敗的問題。
- hyperledger#1845 不可鑄造的資產只能鑄造一次。
- hyperledger#2005 修復 `Client::listen_for_events()` 未關閉 WebSocket 流。
- hyperledger#1623 創建一個 RawGenesisBlockBuilder。
- hyperledger#1917 添加 easy_from_str_impl 宏。
- hyperledger#1922 將 crypto_cli 移至工具中。
- hyperledger#1969 使 `roles` 功能成為默認功能集的一部分。
- hyperledger#2013 修補程序 CLI 參數。
- hyperledger#1897 從序列化中刪除 usize/isize。
- hyperledger#1955 修復了在 `web_login` 內部傳遞 `:` 的可能性
- hyperledger#1943 將查詢錯誤添加到架構中。
- hyperledger#1939 `iroha_config_derive` 的正確功能。
- hyperledger#1908 修復遙測分析腳本的零值處理。
- hyperledger#0000 使隱式忽略的 doc-test 顯式忽略。
- hyperledger#1848 防止公鑰被燒毀。
- hyperledger#1811 添加了測試和檢查以刪除受信任的對等密鑰。
- hyperledger#1821 為 MerkleTree 和 VersionedValidBlock 添加 IntoSchema，修復 HashOf 和 SignatureOf 架構。
- hyperledger#1819 從驗證中的錯誤報告中刪除回溯。
- hyperledger#1774 記錄驗證失敗的確切原因。
- hyperledger#1714 僅通過鍵比較 PeerId。
- hyperledger#1788 減少 `Value` 的內存佔用。
- hyperledger#1804 修復了 HashOf、SignatureOf 的模式生成，添加測試以確保沒有模式丟失。
- hyperledger#1802 日誌記錄可讀性改進。
  - 事件日誌移至跟踪級別
  - ctx 從日誌捕獲中刪除
  - 終端顏色是可選的（為了更好地將日誌輸出到文件）
- hyperledger#1783 修復了牌坊基準。
- hyperledger#1772 在#1764 之後修復。
- hyperledger#1755 對 #1743、#1725 進行了小修復。
  - 根據 #1743 `Domain` 結構更改修復 JSON
- hyperledger#1751 共識修復。 ＃1715：共識修復以處理高負載（＃1746）
  - 查看更改處理修復
  - 查看獨立於特定交易哈希的變更證明
  - 減少消息傳遞
  - 收集視圖更改投票而不是立即發送消息（提高網絡彈性）
  - 在 Sumeragi 中充分使用 Actor 框架（將消息安排給自己而不是任務生成）
  - 改進了 Sumeragi 測試的故障注入
  - 使測試代碼更接近生產代碼
  - 刪除過於復雜的包裝
  - 允許 Sumeragi 在測試代碼中使用 actor 上下文
- hyperledger#1734 更新創世以適應新的域驗證。
- hyperledger#1742 `core` 指令中返回具體錯誤。
- hyperledger#1404 驗證已修復。
- hyperledger#1636 刪除 `trusted_peers.json` 和 `structopt`
  #1636：刪除 `trusted_peers.json`。
- hyperledger#1706 通過拓撲更新更新 `max_faults`。
- hyperledger#1698 修復了公鑰、文檔和錯誤消息。
- 鑄幣問題（1593 和 1405）第 1405 期

### 重構- 從 sumeragi 主循環中提取函數。
- 將 `ProofChain` 重構為新類型。
- 從 `Metrics` 中刪除 `Mutex`
- 刪除 adt_const_generics 夜間功能。
- hyperledger#3039 引入多重簽名等待緩衝區。
- 簡化主。
- hyperledger#3053 修復 Clippy lints。
- hyperledger#2506 添加更多關於塊驗證的測試。
- 刪除 Kura 中的 `BlockStoreTrait`。
- 更新 `nightly-2022-12-22` 的 lint
- hyperledger#3022 刪除 `transaction_cache` 中的 `Option`
- hyperledger#3008 將利基價值添加到 `Hash` 中
- 將 lint 更新至 1.65。
- 添加小測試以擴大覆蓋範圍。
- 從 `FaultInjection` 中刪除無效代碼
- 減少從 sumeragi 調用 p2p 的次數。
- hyperledger#2675 驗證項目名稱/ID 而不分配 Vec。
- hyperledger#2974 在沒有完全重新驗證的情況下防止區塊欺騙。
- 組合器中的 `NonEmpty` 更高效。
- hyperledger#2955 從 BlockSigned 消息中刪除塊。
- hyperledger#1868 防止發送經過驗證的交易
  同伴之間。
- hyperledger#2458 實現通用組合器 API。
- 將存儲文件夾添加到 gitignore 中。
- hyperledger#2909 nextest 的硬編碼端口。
- hyperledger#2747 更改 `LoadFromEnv` API。
- 改進配置失敗時的錯誤消息。
- 向 `genesis.json` 添加額外示例
- 在 `rc9` 發布之前刪除未使用的依賴項。
- 完成新 Sumeragi 上的 linting。
- 在主循環中提取子過程。
- hyperledger#2774 將 `kagami` 創世生成模式從標誌更改為
  子命令。
- hyperledger#2478 添加 `SignedTransaction`
- hyperledger#2649 從 `Kura` 中刪除 `byteorder` 箱
- 將 `DEFAULT_BLOCK_STORE_PATH` 從 `./blocks` 重命名為 `./storage`
- hyperledger#2650 添加 `ThreadHandler` 以關閉 iroha 子模塊。
- hyperledger#2482 將 `Account` 權限令牌存儲在 `Wsv` 中
- 在 1.62 中添加新的 lint。
- 改進 `p2p` 錯誤消息。
- hyperledger#2001 `EvaluatesTo` 靜態類型檢查。
- hyperledger#2052 使權限令牌可通過定義進行註冊。
  ＃2052：實施 PermissionTokenDefinition
- 確保所有功能組合均有效。
- hyperledger#2468 從權限驗證器中刪除調試超級特徵。
- hyperledger#2419 刪除顯式 `drop`s。
- hyperledger#2253 將 `Registrable` 特徵添加到 `data_model`
- 對於數據事件實施 `Origin` 而不是 `Identifiable`。
- hyperledger#2369 重構權限驗證器。
- hyperledger#2307 使 `WorldStateView` 中的 `events_sender` 成為非可選。
- hyperledger#1985 減少 `Name` 結構的大小。
- 添加更多 `const fn`。
- 使用 `default_permissions()` 進行集成測試
- 在 private_blockchain 中添加權限令牌包裝器。
- hyperledger#2292 刪除 `WorldTrait`，從 `IsAllowedBoxed` 中刪除泛型
- hyperledger#2204 使資產相關操作變得通用。
- hyperledger#2233 將 `impl` 替換為 `derive`（對於 `Display` 和 `Debug`）。
- 可識別的結構改進。
- hyperledger#2323 增強 kura init 錯誤消息。
- hyperledger#2238 添加對等構建器進行測試。
- hyperledger#2011 更多描述性配置參數。
- hyperledger#1896 簡化 `produce_event` 實施。
- 圍繞 `QueryError` 進行重構。
- 將 `TriggerSet` 移至 `data_model`。
- hyperledger#2145 重構客戶端的 `WebSocket` 端，提取純數據邏輯。
- 刪除 `ValueMarker` 特徵。
- hyperledger#2149 在 `prelude` 中公開 `Mintable` 和 `MintabilityError`
- hyperledger#2144 重新設計客戶端的 http 工作流程，公開內部 api。
- 移至 `clap`。
- 創建 `iroha_gen` 二進製文件，合併文檔，schema_bin。
- hyperledger#2109 使 `integration::events::pipeline` 測試穩定。
- hyperledger#1982 封裝對 `iroha_crypto` 結構的訪問。
- 添加 `AssetDefinition` 構建器。
- 從 API 中刪除不必要的 `&mut`。
- 封裝對數據模型結構的訪問。
- hyperledger#2144 重新設計客戶端的 http 工作流程，公開內部 api。
- 移至 `clap`。
- 創建 `iroha_gen` 二進製文件，合併文檔，schema_bin。
- hyperledger#2109 使 `integration::events::pipeline` 測試穩定。
- hyperledger#1982 封裝對 `iroha_crypto` 結構的訪問。
- 添加 `AssetDefinition` 構建器。
- 從 API 中刪除不必要的 `&mut`。
- 封裝對數據模型結構的訪問。
- 核心，`sumeragi`，實例函數，`torii`
- hyperledger#1903 將事件發射移至 `modify_*` 方法。
- 拆分 `data_model` lib.rs 文件。
- 將 wsv 引用添加到隊列中。
- hyperledger#1210 分割事件流。
  - 將交易相關功能移至 data_model/transaction 模塊
- hyperledger#1725 刪除 Torii 中的全局狀態。
  - 實施 `add_state macro_rules` 並刪除 `ToriiState`
- 修復 linter 錯誤。
- hyperledger#1661 `Cargo.toml` 清理。
  - 整理貨物依賴關係
- hyperledger#1650 整理 `data_model`
  - 將 World 移至 wsv，修復角色功能，為 CommiedBlock 派生 IntoSchema
- `json` 文件和自述文件的組織。更新自述文件以符合模板。
- 1529：結構化日誌記錄。
  - 重構日誌消息
- `iroha_p2p`
  - 添加 p2p 私有化。

### 文檔

- 更新 Iroha 客戶端 CLI 自述文件。
- 更新教程片段。
- 將“sort_by_metadata_key”添加到 API 規範中。
- 更新文檔鏈接。
- 使用與資產相關的文檔擴展教程。
- 刪除過時的文檔文件。
- 檢查標點符號。
- 將一些文檔移至教程存儲庫。
- 暫存分支的片狀報告。
- 為 rc.7 之前的版本生成變更日誌。
- 7 月 30 日的不穩定報告。
- 凹凸版本。
- 更新測試片狀性。
- hyperledger#2499 修復 client_cli 錯誤消息。
- hyperledger#2344 為 2.0.0-pre-rc.5-lts 生成變更日誌。
- 添加教程鏈接。
- 更新有關 git hooks 的信息。
- 片狀測試記錄。
- hyperledger#2193 更新 Iroha 客戶端文檔。
- hyperledger#2193 更新 Iroha CLI 文檔。
- hyperledger#2193 更新宏箱的自述文件。
 - hyperledger#2193 更新 Norito 解碼器工具文檔。
- hyperledger#2193 更新 Kagami 文檔。
- hyperledger#2193 更新基准文檔。
- hyperledger#2192 查看貢獻指南。
- 修復損壞的代碼內引用。
- hyperledger#1280 記錄 Iroha 指標。
- hyperledger#2119 添加有關如何在 Docker 容器中熱重載 Iroha 的指南。
- hyperledger#2181 查看自述文件。
- hyperledger#2113 Cargo.toml 文件中的文檔功能。
- hyperledger#2177 清理 gitchangelog 輸出。
- hyperledger#1991 將自述文件添加到 Kura 檢查器。
- hyperledger#2119 添加有關如何在 Docker 容器中熱重載 Iroha 的指南。
- hyperledger#2181 查看自述文件。
- hyperledger#2113 Cargo.toml 文件中的文檔功能。
- hyperledger#2177 清理 gitchangelog 輸出。
- hyperledger#1991 將自述文件添加到 Kura 檢查器。
- 生成最新的變更日誌。
- 生成變更日誌。
- 更新過時的自述文件。
- 將缺失的文檔添加到 `api_spec.md`。

### CI/CD 更改- 添加另外五個自託管運行器。
- 為Soramitsu 註冊表添加常規圖像標籤。
- libgit2-sys 0.5.0 的解決方法。恢復到 0.4.4。
- 嘗試使用基於拱門的圖像。
- 更新工作流程以處理新的僅夜間容器。
- 從覆蓋範圍中刪除二進制入口點。
- 將開發測試切換到 Equinix 自託管運行器。
- hyperledger#2865 從 `scripts/check.sh` 中刪除 tmp 文件的使用
- hyperledger#2781 添加覆蓋範圍偏移。
- 禁用緩慢的集成測試。
- 用 docker 緩存替換基礎鏡像。
- hyperledger#2781 添加 codecov 提交父功能。
- 將工作轉移到 github runner。
- hyperledger#2778 客戶端配置檢查。
- hyperledger#2732 添加更新 iroha2-base 鏡像的條件並添加
  公關標籤。
- 修復夜間圖像構建。
- 修復 `buildx` 錯誤與 `docker/build-push-action`
- 無功能急救 `tj-actions/changed-files`
- 在 #2662 之後啟用圖像的順序發布。
- 添加港口登記處。
- 自動標記 `api-changes` 和 `config-changes`
- 再次提交圖像中的哈希值、工具鏈文件、UI 隔離、
  模式跟踪。
- 使發布工作流程順序化，並補充#2427。
- hyperledger#2309：在 CI 中重新啟用文檔測試。
- hyperledger#2165 刪除 codecov 安裝。
- 移動到新容器以防止與當前用戶發生衝突。
 - hyperledger#2158 升級 `parity_scale_codec` 和其他依賴項。 （Norito 編解碼器）
- 修復構建。
- hyperledger#2461 改進 iroha2 CI。
- 更新 `syn`。
- 將覆蓋範圍轉移到新的工作流程。
- 反向docker登錄版本。
- 刪除`archlinux:base-devel`的版本規範
- 更新 Dockerfiles 和 Codecov 報告重用和並發性。
- 生成變更日誌。
- 添加 `cargo deny` 文件。
- 添加 `iroha2-lts` 分支，工作流程從 `iroha2` 複製
- hyperledger#2393 更改 Docker 基礎鏡像的版本。
- hyperledger#1658 添加文檔檢查。
- 包的版本提升並刪除未使用的依賴項。
- 刪除不必要的覆蓋率報告。
- hyperledger#2222 根據是否涉及覆蓋範圍進行拆分測試。
- hyperledger#2153 修復#2154。
- 版本碰撞了所有板條箱。
- 修復部署管道。
- hyperledger#2153 修復覆蓋範圍。
- 添加創世檢查和更新文檔。
- 將鐵鏽、黴菌和夜間分別提升至 1.60、1.2.0 和 1.62。
- 負載RS觸發器。
- hyperledger#2153 修復#2154。
- 版本碰撞了所有板條箱。
- 修復部署管道。
- hyperledger#2153 修復覆蓋範圍。
- 添加創世檢查和更新文檔。
- 將鐵鏽、黴菌和夜間分別提升至 1.60、1.2.0 和 1.62。
- load-rs 觸發器。
- load-rs:release 工作流程觸發器。
- 修復推送工作流程。
- 將遙測添加到默認功能。
- 添加適當的標籤以將工作流程推送到主幹上。
- 修復失敗的測試。
- hyperledger#1657 將鏡像更新為 Rust 1.57。 #1630：回到自託管運行器。
- CI 改進。
- 將覆蓋範圍切換為使用 `lld`。
- CI 依賴性修復。
- CI 分段改進。
- 在 CI 中使用固定的 Rust 版本。
- 修復 Docker 發布和 iroha2-dev 推送 CI。將報導和替補轉移到 PR
- 刪除 CI docker 測試中不必要的完整 Iroha 構建。

  Iroha 構建變得毫無用處，因為它現在是在 docker 鏡像本身中完成的。因此 CI 只構建用於測試的客戶端 cli。
- 在 CI 管道中添加對 iroha2 分支的支持。
  - 長時間測試僅在 iroha2 的 PR 上運行
  - 僅從 iroha2 發布 docker 鏡像
- 額外的 CI 緩存。

### 網絡組裝


### 版本顛簸

- rc.13 之前的版本。
- rc.11 之前的版本。
- RC.9 版本。
- RC.8 版本。
- 將版本更新至 RC7。
- 發布前的準備工作。
- 更新模具1.0。
- 碰撞依賴性。
- 更新 api_spec.md：修復請求/響應主體。
- 將 Rust 版本更新至 1.56.0。
- 更新貢獻指南。
- 更新 README.md 和 `iroha/config.json` 以匹配新的 API 和 URL 格式。
- 將 docker 發布目標更新為 hyperledger/iroha2 #1453。
- 更新工作流程，使其與主要工作流程匹配。
- 更新 API 規範並修復運行狀況端點。
- Rust 更新至 1.54。
- 文檔（iroha_crypto）：更新 `Signature` 文檔並對齊 `verify` 的參數
- Ursa 版本從 0.3.5 升級到 0.3.6。
- 更新新跑步者的工作流程。
- 更新 dockerfile 以進行緩存和更快的 ci 構建。
- 更新 libssl 版本。
- 更新 dockerfiles 和 async-std。
- 修復更新的剪輯。
- 更新資產結構。
  - 支持資產中的鍵值指令
  - 資產類型作為枚舉
  - 修復資產ISI溢出漏洞
- 更新貢獻指南。
- 更新過時的庫。
- 更新白皮書並修復 linting 問題。
- 更新 cucumber_rust 庫。
- 密鑰生成的自述文件更新。
- 更新 Github Actions 工作流程。
- 更新 Github Actions 工作流程。
- 更新requirements.txt。
- 更新 common.yaml。
- Sara 的文檔更新。
- 更新指令邏輯。
- 更新白皮書。
- 更新網絡功能描述。
- 根據評論更新白皮書。
- WSV 更新和遷移到 Scale 的分離。
- 更新 gitignore。
- 稍微更新了 WP 中 kura 的描述。
- 更新白皮書中有關 kura 的描述。

### 架構

- hyperledger#2114 模式中的排序集合支持。
- hyperledger#2108 添加分頁。
- hyperledger#2114 模式中的排序集合支持。
- hyperledger#2108 添加分頁。
- 使架構、版本和宏 no_std 兼容。
- 修復架構中的簽名。
- 改變了模式中 `FixedPoint` 的表示。
- 將 `RawGenesisBlock` 添加到架構自省。
- 更改了對像模型以創建模式 IR-115。

### 測試

- hyperledger#2544 教程文檔測試。
- hyperledger#2272 添加“FindAssetDefinitionById”查詢的測試。
- 添加 `roles` 集成測試。
- 標準化 ui 測試格式，將派生 ui 測試移至派生 crate。
- 修復模擬測試（期貨無序錯誤）。
- 刪除了 DSL 箱並將測試移至 `data_model`
- 確保不穩定的網絡測試通過有效代碼。
- 添加了對 iroha_p2p 的測試。
- 捕獲測試中的日誌，除非測試失敗。
- 添加測試輪詢並修復很少破壞的測試。
- 測試並行設置。
- 從 iroha init 和 iroha_client 測試中刪除 root。
- 修復測試剪輯警告並添加對 ci 的檢查。
- 修復基準測試期間的 `tx` 驗證錯誤。
- hyperledger#860：Iroha 查詢和測試。
- Iroha 自定義 ISI 指南和 Cucumber 測試。
- 添加對非標準客戶端的測試。
- 橋樑註冊變更和測試。
- 使用網絡模擬進行共識測試。
- 使用臨時目錄來執行測試。
- 工作台測試陽性病例。
- 帶有測試的初始默克爾樹功能。
- 修復了測試和世界狀態視圖初始化。

＃＃＃ 其他- 將參數化移至特徵並刪除 FFI IR 類型。
- 添加對聯合的支持，引入 `non_robust_ref_mut` * 實現 conststring FFI 轉換。
- 改進 IdOrdEqHash。
- 從（反）序列化中刪除 FilterOpt::BySome。
- 使不透明。
- 使 ContextValue 透明。
- 使 Expression::Raw 標記可選。
- 增加一些說明的透明度。
- 改進 RoleId 的（反）序列化。
- 改進驗證器::Id 的（反）序列化。
- 改進 PermissionTokenId 的（反）序列化。
- 改進 TriggerId 的（反）序列化。
- 改進資產（定義）ID 的（反）序列化。
- 改進 AccountId 的（反）序列化。
- 改進 Ipfs 和 DomainId 的（反）序列化。
- 從客戶端配置中刪除記錄器配置。
- 添加對 FFI 中透明結構的支持。
- 將 &Option<T> 重構為 Option<&T>
- 修復剪輯警告。
- 在 `Find` 錯誤描述中添加更多詳細信息。
- 修復 `PartialOrd` 和 `Ord` 實現。
- 使用 `rustfmt` 代替 `cargo fmt`
- 刪除 `roles` 功能。
- 使用 `rustfmt` 代替 `cargo fmt`
- 將工作目錄作為卷與開發 docker 實例共享。
- 刪除執行中的 Diff 關聯類型。
- 使用自定義編碼而不是多值返回。
- 刪除 serde_json 作為 iroha_crypto 依賴項。
- 僅允許版本屬性中的已知字段。
- 澄清端點的不同端口。
- 刪除 `Io` 派生。
- key_pairs 的初始文檔。
- 回到自託管運行器。
- 修復代碼中新的 Clippy lints。
- 從維護者中刪除 i1i1。
- 添加演員文檔和小修復。
- 輪詢而不是推送最新的區塊。
- 對 7 個對等點中的每一個進行測試的交易狀態事件。
- `FuturesUnordered` 而不是 `join_all`
- 切換到 GitHub Runners。
- 將 VersionedQueryResult 與 QueryResult 用於 /query 端點。
- 重新連接遙測。
- 修復依賴機器人配置。
- 添加 commit-msg git hook 以包含簽核。
- 修復推送管道。
- 升級依賴機器人。
- 檢測隊列推送的未來時間戳。
- hyperledger#1197：Kura 處理錯誤。
- 添加取消註冊對等指令。
- 添加可選的隨機數來區分交易。關閉#1493。
- 刪除了不必要的 `sudo`。
- 域的元數據。
- 修復 `create-docker` 工作流程中的隨機彈跳。
- 按照失敗管道的建議添加了 `buildx`。
- hyperledger#1454：使用特定狀態代碼和提示修復查詢錯誤響應。
- hyperledger#1533：通過哈希查找交易。
- 修復 `configure` 端點。
- 添加基於布爾的資產可鑄造性檢查。
- 添加類型化加密原語並遷移到類型安全加密。
- 日誌記錄改進。
- hyperledger#1458：將參與者通道大小添加到配置中，作為 `mailbox`。
- hyperledger#1451：如果 `faulty_peers = 0` 和 `trusted peers count > 1` 添加有關錯誤配置的警告
- 添加用於獲取特定塊哈希的處理程序。
- 添加了新查詢 FindTransactionByHash。
- hyperledger#1185：更改板條箱名稱和路徑。
- 修復日誌和一般改進。
- hyperledger#1150：將 1000 個塊分組到每個文件中
- 隊列壓力測試。
- 日誌級別修復。
- 將標頭規範添加到客戶端庫。
- 隊列恐慌失敗修復。
- 修復隊列。
- 修復 dockerfile 版本構建。
- HTTPS 客戶端修復。
- 加速ci。
- 1. 刪除了除 iroha_crypto 之外的所有 ursa 依賴項。
- 修復減去持續時間時的溢出問題。
- 在客戶端中公開字段。
- 每晚將 Iroha2 推送到 Dockerhub。
- 修復 http 狀態代碼。
- 將 iroha_error 替換為 thiserror、eyre 和 color-eyre。
- 用橫梁一代替隊列。
- 刪除一些無用的 lint 配額。
- 引入資產定義的元數據。
- 從 test_network 箱中刪除參數。
- 刪除不必要的依賴項。
- 修復 iroha_client_cli::events。
- hyperledger#1382：刪除舊的網絡實現。
- hyperledger#1169：增加了資產的精度。
- 對等啟動的改進：
  - 允許僅從環境加載創世公鑰
  - 現在可以在 cli 參數中指定 config、genesis 和 trust_peers 路徑
- hyperledger#1134：集成 Iroha P2P。
- 將查詢端點更改為 POST 而不是 GET。
- 在actor中同步執行on_start。
- 遷移到扭曲。
- 通過代理錯誤修復重新提交提交。
- 恢復“引入多個代理修復”提交（9c148c33826067585b5868d297dcdd17c0efe246）
- 引入多個代理修復：
  - 在演員停止時取消訂閱經紀人
  - 支持同一參與者類型的多個訂閱（以前是 TODO）
  - 修復了經紀人總是將自己作為演員 ID 的錯誤。
- 經紀人錯誤（測試展示）。
- 添加數據模型的派生。
- 從鳥居中刪除 rwlock。
- OOB 查詢權限檢查。
- hyperledger#1272：對等計數的實現，
- 遞歸檢查指令內的查詢權限。
- 安排停止演員。
- hyperledger#1165：對等計數的實現。
- 檢查torii端點中帳戶的查詢權限。
- 刪除了系統指標中公開的 CPU 和內存使用情況。
 - 將 WS 消息的 JSON 替換為 Norito。
- 存儲視圖更改的證明。
- hyperledger#1168：如果交易未通過簽名檢查條件，則添加日誌記錄。
- 修復了小問題，添加了連接監聽代碼。
- 引入網絡拓撲生成器。
- 為 Iroha 實現 P2P 網絡。
- 添加塊大小指標。
- PermissionValidator 特徵重命名為 IsAllowed。以及相應的其他名稱更改
- API 規範 Web 套接字更正。
- 從 docker 鏡像中刪除不必要的依賴項。
- Fmt 使用 Crate import_grainarity。
- 引入通用權限驗證器。
- 遷移到參與者框架。
- 更改代理設計並為參與者添加一些功能。
- 配置 codecov 狀態檢查。
- 使用 grcov 進行基於源的覆蓋。
- 修復了多個構建參數格式並為中間構建容器重新聲明了 ARG。
- 引入 SubscriptionAccepted 消息。
- 操作後從賬戶中刪除零價值資產。
- 修復了 docker 構建參數格式。
- 修復了未找到子塊時的錯誤消息。
- 添加了供應商的 OpenSSL 來構建，修復了 pkg-config 依賴性。
- 修復 dockerhub 的存儲庫名稱和覆蓋範圍差異。
- 如果無法加載 TrustedPeers，則添加了清晰的錯誤文本和文件名。
- 將文本實體更改為文檔中的鏈接。
- 修復 Docker 發布中錯誤的用戶名密碼。
- 修復白皮書中的小錯字。
- 允許使用 mod.rs 以獲得更好的文件結構。
- 將 main.rs 移至單獨的 crate 中並為公共區塊鏈授予權限。
- 在客戶端 cli 內添加查詢。
- 從 clap 遷移到 cli 的 structopts。
- 將遙測限制為不穩定的網絡測試。
- 將特徵移至智能合約模塊。
- sed -i“s/world_state_view/wsv/g”
- 將智能合約移至單獨的模塊中。
- Iroha 網絡內容長度錯誤修復。
- 為參與者 ID 添加任務本地存儲。對於死鎖檢測很有用。
- 在CI中添加死鎖檢測測試
- 添加內省宏。
- 消除工作流程名稱的歧義並進行格式更正
- 更改查詢 API。
- 從 async-std 遷移到 tokio。
- 向 ci 添加遙測分析。
- 為 iroha 添加期貨遙測。
- 將 iroha futures 添加到每個異步函數中。
- 添加 iroha futures 以方便觀察民意調查數量。
- 自述文件中添加了手動部署和配置。
- 記者修復。
- 添加派生消息宏。
- 添加簡單的演員框架。
- 添加dependabot配置。
- 添加漂亮的恐慌和錯誤報告器。
- Rust 版本遷移到 1.52.1 並進行相應修復。
- 在單獨的線程中生成阻塞 CPU 密集型任務。
- 使用來自 crates.io 的 unique_port 和 Cargo-lints。
- 修復無鎖 WSV：
  - 刪除 API 中不必要的 Dashmap 和鎖定
  - 修復了創建塊數量過多的錯誤（未記錄被拒絕的交易）
  - 顯示錯誤的完整錯誤原因
- 添加遙測訂閱者。
- 角色和權限查詢。
- 將塊從 kura 移動到 wsv。
- 更改為 wsv 內的無鎖數據結構。
- 網絡超時修復。
- 修復健康端點。
- 介紹角色。
- 添加來自 dev 分支的推送 docker 鏡像。
- 添加更積極的 linting 並消除代碼中的恐慌。
- 指令執行特徵的返工。
- 從 iroha_config 中刪除舊代碼。
- IR-1060 添加對所有現有權限的授予檢查。
- 修復 iroha_network 的 ulimit 和超時。
- Ci 超時測試修復。
- 當定義被刪除時，刪除所有資產。
- 修復添加資產時的 wsv 恐慌。
- 刪除通道的 Arc 和 Rwlock。
- Iroha 網絡修復。
- 權限驗證器在檢查中使用引用。
- 授予指令。
- 添加了字符串長度限制的配置以及 NewAccount、Domain 和 AssetDefinition IR-1036 的 ID 驗證。
- 用跟踪庫替換日誌。
- 添加 ci 檢查文檔並拒絕 dbg 宏。
- 引入可授予的權限。
- 添加 iroha_config 箱。
- 添加@alerdenisov 作為代碼所有者以批准所有傳入的合併請求。
- 修復了共識期間交易大小檢查的問題。
- 恢復異步標準的升級。
- 用 2 IR-1035 的冪替換一些常量。
- 添加查詢以檢索交易歷史記錄 IR-1024。- 添加存儲權限驗證和權限驗證器重組。
- 添加NewAccount用於帳戶註冊。
- 添加資產定義類型。
- 引入可配置的元數據限制。
- 引入交易元數據。
- 在查詢中添加表達式。
- 添加 lints.toml 並修復警告。
- 將 trust_peers 與 config.json 分開。
- 修復 Telegram 中 Iroha 2 社區 URL 中的拼寫錯誤。
- 修復剪輯警告。
- 引入了對帳戶的鍵值元數據支持。
- 添加塊的版本控制。
- 修復 ci linting 重複。
- 添加 mul、div、mod、raise_to 表達式。
- 添加 into_v* 用於版本控制。
- 用錯誤宏替換 Error::msg。
- 重寫 iroha_http_server 並修改 torii 錯誤。
 - 將 Norito 版本升級到 2。
- 白皮書版本控制描述。
- 可靠的分頁。修復由於錯誤而可能不需要分頁的情況，而不是返回空集合。
- 為枚舉添加導出（錯誤）。
- 修復夜間版本。
- 添加 iroha_error 箱。
- 版本化消息。
- 引入容器版本控制原語。
- 修復基準。
- 添加分頁。
- 添加 Varint 編碼解碼。
- 將查詢時間戳更改為 u128。
- 為管道事件添加 RejectionReason 枚舉。
- 從創世文件中刪除過時的行。在之前的提交中，目標已從寄存器 ISI 中刪除。
- 簡化註冊和取消註冊 ISI。
- 修復提交超時未在 4 對等網絡中發送的問題。
- 更改視圖時的拓撲隨機播放。
- 為 FromVariant 派生宏添加其他容器。
- 添加對客戶端 cli 的 MST 支持。
- 添加 FromVariant 宏和清理代碼庫。
- 將 i1i1 添加到代碼所有者。
- 八卦交易。
- 添加指令和表達式的長度。
- 添加文檔以阻止時間和提交時間參數。
- 用 TryFrom 替換了驗證和接受特徵。
- 引入僅等待最少數量的對等點。
- 添加 github 操作以使用 iroha2-java 測試 api。
- 添加 docker-compose-single.yml 的起源。
- 帳戶的默認簽名檢查條件。
- 添加對具有多個簽名者的帳戶的測試。
- 添加對 MST 的客戶端 API 支持。
- 在碼頭工人中構建。
- 將創世添加到 docker compose。
- 引入條件 MST。
- 添加 wait_for_active_peers 實現。
- 在 iroha_http_server 中添加 isahc 客戶端測試。
- 客戶端 API 規範。
- 表達式中的查詢執行。
- 集成表達式和 ISI。
- ISI 的表達式。
- 修復帳戶配置基準。
- 為客戶端添加帳戶配置。
- 修復 `submit_blocking`。
- 發送管道事件。
- Iroha 客戶端 Web 套接字連接。
- 管道事件和數據事件的事件分離。
- 權限集成測試。
- 添加對burn 和mint 的權限檢查。
- 取消註冊 ISI 權限。
- 修復世界結構 PR 的基準。
- 引入世界結構。
- 實現創世塊加載組件。
- 介紹創世賬戶。
- 引入權限驗證器構建器。
- 使用 Github Actions 將標籤添加到 Iroha2 PR。
- 引入權限框架。
- 隊列 tx tx 數量限制和 Iroha 初始化修復。
- 將哈希包裝在結構中。
- 提高日誌級別：
  - 將信息級別日誌添加到共識中。
  - 將網絡通信日誌標記為跟踪級別。
  - 從 WSV 中刪除塊向量，因為它是重複的，並且它在日誌中顯示了所有區塊鏈。
  - 將信息日誌級別設置為默認值。
- 刪除可變的 WSV 引用以進行驗證。
- 海姆版本增量。
- 將默認可信對等點添加到配置中。
- 客戶端 API 遷移到 http。
- 將傳輸 isi 添加到 CLI。
- Iroha 對等相關指令的配置。
- 實施缺失的 ISI 執行方法和測試。
- URL查詢參數解析
- 添加 `HttpResponse::ok()`、`HttpResponse::upgrade_required(..)`
- 使用 Iroha DSL 方法替換舊的指令和查詢模型。
- 添加 BLS 簽名支持。
- 引入 http 服務器箱。
- 使用符號鏈接修補了 libssl.so.1.0.0。
- 驗證交易的帳戶簽名。
- 重構事務階段。
- 初始域改進。
- 實現 DSL 原型。
- 改進 Torii 基準：禁用基準中的日誌記錄，添加成功率斷言。
- 改進測試覆蓋率管道：用 `grcov` 替換 `tarpaulin`，將測試覆蓋率報告發佈到 `codecov.io`。
- 修復 RTD 主題。
- iroha 子項目的交付工件。
- 介紹 `SignedQueryRequest`。
- 修復簽名驗證的錯誤。
- 回滾事務支持。
- 將生成的密鑰對打印為 json。
- 支持 `Secp256k1` 密鑰對。
- 初步支持不同的加密算法。
- 去中心化交易所功能。
- 用 cli 參數替換硬編碼的配置路徑。
- 工作台主工作流程修復。
- Docker 事件連接測試。
- Iroha 監視器指南和 CLI。
- 事件 CLI 改進。
- 事件過濾器。
- 事件連接。
- 修復主工作流程。
- iroha2 的RTD。
- 用於區塊交易的 Merkle 樹根哈希。
- 發佈到 docker hub。
- 用於維護連接的 CLI 功能。
- 用於維護連接的 CLI 功能。
- Eprintln 記錄宏。
- 日誌改進。
- IR-802 訂閱塊狀態更改。
- 交易和區塊的事件發送。
- 將 Sumeragi 消息處理移至消息實現中。
- 通用連接機制。
- 為非標準客戶端提取 Iroha 域實體。
- 交易 TTL。
- 每個塊配置的最大交易量。
- 存儲無效的塊哈希值。
- 批量同步區塊。
- 連接功能的配置。
- 連接到 Iroha 功能。
- 塊驗證更正。
- 塊同步：圖表。
- 連接到 Iroha 功能。
- 橋接：刪除客戶端。
- 塊同步。
- 添加對等 ISI。
- 命令到指令重命名。
- 簡單的指標端點。
- Bridge：獲取註冊的橋樑和外部資產。
- Docker 在管道中編寫測試。
- 票數不足 Sumeragi 測試。
- 塊鏈。
- 橋：手動外部傳輸處理。
- 簡單的維護端點。
- 遷移到 serde-json。
- 消滅三軍情報局。
- 添加橋接客戶端、AddSignatory ISI 和 CanAddSignatory 權限。
- Sumeragi：b 組中的同級相關 TODO 修復。
- 在登錄 Sumeragi 之前驗證區塊。
- 橋接外部資產。
- Sumeragi 消息中的簽名驗證。
- 二進制資產商店。
- 將 PublicKey 別名替換為類型。
- 準備用於發布的板條箱。
- NetworkTopology 內的最低投票邏輯。
- TransactionReceipt 驗證重構。
- OnWorldStateViewChange 觸發更改：IrohaQuery 而不是指令。
- 將網絡拓撲中的構造與初始化分開。
- 添加與 Iroha 事件相關的 Iroha 特別說明。
- 塊創建超時處理。
- 術語表和如何添加 Iroha 模塊文檔。
- 用原始 Iroha 模型替換硬編碼橋模型。
- 引入 NetworkTopology 結構。
- 通過指令轉換添加權限實體。
- Sumeragi 消息模塊中的消息。
- Kura 的創世塊功能。
- 添加 Iroha 包的自述文件。
- 橋接和寄存器橋接 ISI。
- 與 Iroha 的初始工作改變了聽眾。
- 將權限檢查注入 OOB ISI。
- Docker 多個對等點修復。
- 點對點 Docker 示例。
- 交易收據處理。
- Iroha 權限。
- Dex 模塊和 Bridges 板條箱。
- 修復與多個同行的資產創建的集成測試。
- 將資產模型重新實施到 EC-S- 中。
- 提交超時處理。
- 塊頭。
- 域實體的 ISI 相關方法。
- Kura 模式枚舉和可信對等配置。
- 文檔檢查規則。
- 添加CommitedBlock。
- 將 kura 與 `sumeragi` 解耦。
- 在創建區塊之前檢查交易是否不為空。
- 重新實施 Iroha 特殊說明。
- 交易和區塊轉換的基準。
- 交易生命週期和狀態重新設計。
- 塊生命週期和狀態。
- 修復驗證錯誤，`sumeragi` 循環週期與 block_build_time_ms 配置參數同步。
- Sumeragi 算法封裝在 `sumeragi` 模塊內。
- 通過通道實現 Iroha 網絡箱的模擬模塊。
- 遷移到 async-std API。
- 網絡模擬功能。
- 異步相關代碼清理。
- 事務處理循環中的性能優化。
- 密鑰對的生成是從 Iroha 開始提取的。
- Docker Iroha 可執行文件的打包。
- 介紹Sumeragi基本場景。
- Iroha CLI 客戶端。
- 替補組執行後，伊洛哈掉落。
- 集成 `sumeragi`。
- 將 `sort_peers` 實現更改為使用先前塊哈希作為種子的 rand shuffle。
- 刪除對等模塊中的消息包裝器。
- 將網絡相關信息封裝在`torii::uri`和`iroha_network`內。
- 添加實施的對等指令，而不是硬編碼處理。
- 通過受信任的同行列表進行同行通信。
- Torii 內部網絡請求處理的封裝。
- 將加密邏輯封裝在加密模塊內。- 使用時間戳和前一個塊哈希作為有效負載的塊符號。
- 加密功能放置在模塊頂部，並與封裝到簽名中的 ursa 簽名者一起使用。
- Sumeragi 初始。
- 在提交存儲之前驗證世界狀態視圖克隆上的事務指令。
- 驗證交易接受時的簽名。
- 修復請求反序列化中的錯誤。
- Iroha 簽名的實現。
- 刪除區塊鏈實體以清理代碼庫。
- 事務 API 的更改：更好地創建和處理請求。
- 修復會創建具有空交易向量的塊的錯誤
- 轉發待處理的交易。
 - 修復 u128 Norito 編碼 TCP 數據包中丟失字節的錯誤。
- 用於方法跟踪的屬性宏。
- P2p 模塊。
- 在torii和客戶端中使用iroha_network。
- 添加新的 ISI 信息。
- 網絡狀態的特定類型別名。
- Box<dyn Error> 替換為字符串。
- 網絡監聽狀態。
- 交易的初始驗證邏輯。
- Iroha_network 箱子。
- 派生 Io、IntoContract 和 IntoQuery 特徵的宏。
- Iroha 客戶端的查詢實現。
- 將命令轉變為三軍情報局合同。
- 添加條件多重簽名的建議設計。
- 遷移到 Cargo 工作區。
- 模塊遷移。
- 通過環境變量進行外部配置。
- Torii 的獲取和放置請求處理。
- Github ci 修正。
- Cargo-make 在測試後清理塊。
- 引入 `test_helper_fns` 模塊，具有用塊清理目錄的功能。
- 通過默克爾樹實施驗證。
- 刪除未使用的派生。
- 傳播異步/等待並修復未等待的 `wsv::put`。
- 使用 `futures` 箱中的連接。
- 實現並行存儲執行：寫入磁盤和更新 WSV 並行發生。
- 使用引用而不是所有權進行（反）序列化。
- 從文件中彈出代碼。
- 使用 ursa::blake2。
- 貢獻指南中有關 mod.rs 的規則。
- 哈希 32 字節。
- Blake2 哈希。
- 磁盤接受對塊的引用。
- 重構命令模塊和初始 Merkle 樹。
- 重構模塊結構。
- 正確的格式。
- 將文檔註釋添加到 read_all。
- 實現`read_all`，重新組織存儲測試，並將異步函數測試轉變為異步測試。
- 刪除不必要的可變捕獲。
- 審查問題，修復剪輯。
- 刪除破折號。
- 添加格式檢查。
- 添加令牌。
- 為 github 操作創建 rust.yml。
- 引入磁盤存儲原型。
- 轉移資產測試和功能。
- 將默認初始化程序添加到結構中。
- 更改 MSTCache 結構的名稱。
- 添加忘記借用。
- iroha2 代碼的初始輪廓。
- 初始 Kura API。
- 添加一些基本文件，並發布概述 iroha v2 願景的白皮書初稿。
- 基本 iroha v2 分支。

## [1.5.0] - 2022-04-08

### CI/CD 更改
- 刪除 Jenkinsfile 和 JenkinsCI。

### 添加

- 為 Burrow 添加 RocksDB 存儲實現。
- 使用布隆過濾器引入流量優化
- 將 `MST` 模塊網絡更新為位於 `batches_cache` 中的 `OS` 模塊中。
- 提出流量優化建議。

### 文檔

- 修復構建。添加數據庫差異、遷移實踐、健康檢查端點、有關 iroha-swarm 工具的信息。

### 其他

- 文檔構建的要求修復。
- 修剪髮布文檔以突出剩餘的關鍵後續項目。
- 修復“檢查 docker 映像是否存在”/build all Skip_testing。
- /構建所有skip_testing。
- /build 跳過測試；還有更多文檔。
- 添加 `.github/_README.md`。
- 刪除 `.packer`。
- 刪除測試參數的更改。
- 使用新參數跳過測試階段。
- 添加到工作流程。
- 刪除存儲庫調度。
- 添加存儲庫調度。
- 為測試人員添加參數。
- 刪除 `proposal_delay` 超時。

## [1.4.0] - 2022-01-31

### 添加

- 添加同步節點狀態
- 添加 RocksDB 指標
- 通過 http 和指標添加健康檢查接口。

### 修復

- 修復 Iroha v1.4-rc.2 中的列族
- 在 Iroha v1.4-rc.1 中添加 10 位布隆過濾器

### 文檔

- 將 zip 和 pkg-config 添加到構建依賴列表。
- 更新自述文件：修復構建狀態、構建指南等的損壞鏈接。
- 修復配置和 Docker 指標。

### 其他

- 更新 GHA docker 標籤。
- 修復使用 g++11 編譯時的 Iroha 1 編譯錯誤。
- 將 `max_rounds_delay` 替換為 `proposal_creation_timeout`。
- 更新示例配置文件以刪除舊的數據庫連接參數。