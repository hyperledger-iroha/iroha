---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c958f1044ce6ae35dde36629b55aa3880c3926e60349cb06e80efdd8a3f9211c
source_last_modified: "2025-12-31T15:58:47.310713+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operator-onboarding
title: Sora Nexus data-space operator onboarding
description: Mirror of `docs/source/sora_nexus_operator_onboarding.md`, tracking the end-to-end release checklist for Nexus operators.
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/sora_nexus_operator_onboarding.md`。保持兩個副本對齊，直到本地化版本到達門戶。
:::

# Sora Nexus 數據空間操作員入門

本指南介紹了 Sora Nexus 數據空間操作員在發布版本後必須遵循的端到端流程。它通過描述如何在使節點上線之前將下載的捆綁包/映像、清單和配置模板與全局通道期望保持一致，對雙軌運行手冊 (`docs/source/release_dual_track_runbook.md`) 和工件選擇說明 (`docs/source/release_artifact_selection.md`) 進行了補充。

## 受眾和先決條件
- 您已獲得 Nexus 計劃批准並收到您的數據空間分配（通道索引、數據空間 ID/別名和路由策略要求）。
- 您可以訪問發布工程發布的簽名發布工件（tarball、圖像、清單、簽名、公鑰）。
- 您已為驗證者/觀察者角色生成或收到生產密鑰材料（Ed25519 節點身份；BLS 共識密鑰 + 驗證者的 PoP；加上任何機密功能切換）。
- 您可以訪問現有的 Sora Nexus 對等點來引導您的節點。

## 步驟 1 — 確認發布配置文件
1. 確定為您提供的網絡別名或鏈 ID。
2. 簽出此存儲庫時運行 `scripts/select_release_profile.py --network <alias>`（或 `--chain-id <id>`）。幫助程序查閱 `release/network_profiles.toml` 並打印要部署的配置文件。對於 Sora Nexus，響應必須是 `iroha3`。對於任何其他值，請停止並聯繫發布工程。
3. 記下發佈公告引用的版本標籤（例如 `iroha3-v3.2.0`）；您將使用它來獲取文物和清單。

## 步驟 2 — 檢索並驗證人工製品
1. 下載 `iroha3` 捆綁包 (`<profile>-<version>-<os>.tar.zst`) 及其配套文件（`.sha256`、可選的 `.sig/.pub`、`<profile>-<version>-manifest.json` 和 `<profile>-<version>-image.json`（如果部署容器））。
2. 開箱前驗證完整性：
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   如果您使用硬件支持的 KMS，請將 `openssl` 替換為組織批准的驗證程序。
3. 檢查 tarball 內的 `PROFILE.toml` 和 JSON 清單以確認：
   - `profile = "iroha3"`
   - `version`、`commit` 和 `built_at` 字段與發佈公告相符。
   - 操作系統/架構與您的部署目標相匹配。
4. 如果使用容器鏡像，請對`<profile>-<version>-<os>-image.tar`重複哈希/簽名驗證，並確認`<profile>-<version>-image.json`中記錄的鏡像ID。

## 步驟 3 — 從模板進行階段配置
1. 解壓捆綁包並將 `config/` 複製到節點將讀取其配置的位置。
2. 將 `config/` 下的文件視為模板：
   - 將 `public_key`/`private_key` 替換為您的生產 Ed25519 密鑰。如果節點將從 HSM 獲取私鑰，則從磁盤中刪除私鑰；更新配置以指向 HSM 連接器。
   - 調整 `trusted_peers`、`network.address` 和 `torii.address`，以便它們反映您可訪問的接口以及分配給您的引導對等點。
   - 使用面向操作員的 Torii 端點（包括 TLS 配置，如果適用）以及您為操作工具提供的憑據更新 `client.toml`。
3. 保留捆綁包中提供的鏈 ID，除非治理明確指示，否則全局通道需要單個規範鏈標識符。
4. 計劃啟動帶有 Sora 配置文件標誌的節點：`irohad --sora --config <path>`。當該標誌不存在時，配置加載器將拒絕 SoraFS 或多通道設置。

## 步驟 4 — 調整數據空間元數據和路由
1. 編輯 `config/config.toml`，使 `[nexus]` 部分與 Nexus 委員會提供的數據空間目錄相匹配：
   - `lane_count` 必須等於當前時期啟用的通道總數。
   - `[[nexus.lane_catalog]]` 和 `[[nexus.dataspace_catalog]]` 中的每個條目必須包含唯一的 `index`/`id` 和商定的別名。不要刪除現有的全局條目；如果理事會分配了額外的數據空間，請添加您委託的別名。
   - 確保每個數據空間條目包括 `fault_tolerance (f)`；車道接力委員會的規模為 `3f+1`。
2. 更新 `[[nexus.routing_policy.rules]]` 以獲取為您提供的策略。默認模板將治理指令路由到通道 `1`，將合約部署路由到通道 `2`；附加或修改規則，以便將發往您的數據空間的流量轉發到正確的通道和別名。在更改規則順序之前與發布工程人員協調。
3. 查看 `[nexus.da]`、`[nexus.da.audit]` 和 `[nexus.da.recovery]` 閾值。運營商應保持理事會批准的值；僅在更新的政策獲得批准後才進行調整。
4. 在操作跟踪器中記錄最終配置。雙軌發布操作手冊需要將有效的 `config.toml`（已編輯機密）附加到入職票據。

## 步驟 5 — 飛行前驗證
1. 在加入網絡之前運行內置配置驗證器：
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   這會打印已解析的配置，如果目錄/路由條目不一致或者創世和配置不一致，則會提前失敗。
2. 如果部署容器，請在使用 `docker load -i <profile>-<version>-<os>-image.tar` 加載映像後在映像中運行相同的命令（請記住包含 `--sora`）。
3. 檢查日誌中有關佔位符通道/數據空間標識符的警告。如果出現任何情況，請重新訪問步驟 4 - 生產部署不得依賴於模板附帶的佔位符 ID。
4. 執行本地冒煙程序（例如，使用 `iroha_cli` 提交 `FindNetworkStatus` 查詢，確認遙測端點公開 `nexus_lane_state_total`，並驗證流密鑰是否按要求輪換或導入）。

## 步驟 6 — 切換和移交
1. 將經過驗證的 `manifest.json` 和簽名工件存儲在發布票據中，以便審核員可以重現您的檢查。
2. 通知Nexus Operations該節點已準備好引入；包括：
   - 節點身份（對等 ID、主機名、Torii 端點）。
   - 有效的車道/數據空間目錄和路由策略值。
   - 您驗證的二進製文件/圖像的哈希值。
3. 與 `@nexus-core` 協調最終的同伴准入（八卦種子和通道分配）。在獲得批准之前請勿加入網絡； Sora Nexus 強制執行確定性車道佔用並需要更新的入場清單。
4. 節點上線後，使用您引入的任何替代更新 Runbook，並記下發布標籤，以便下一次迭代可以從此基線開始。

## 參考清單
- [ ] 發布配置文件已驗證為 `iroha3`。
- [ ] 捆綁包/圖像哈希值和簽名已驗證。
- [ ] 密鑰、對等地址和 Torii 端點已更新為生產值。
- [ ] Nexus 通道/數據空間目錄和路由策略匹配委員會分配。
- [ ] 配置驗證器 (`irohad --sora --config … --trace-config`) 通過，沒有警告。
- [ ] 清單/簽名存檔在入職通知單和通知的操作人員中。

有關 Nexus 遷移階段和遙測預期的更廣泛背景，請查看 [Nexus 轉換說明](./nexus-transition-notes)。