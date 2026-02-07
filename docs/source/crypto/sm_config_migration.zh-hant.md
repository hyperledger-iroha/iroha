---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T14:45:02.068538+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ SM配置遷移

# SM配置遷移

推出 SM2/SM3/SM4 功能集需要的不僅僅是使用
`sm` 功能標誌。節點控制分層背後的功能
`iroha_config` 配置文件並期望創世清單攜帶匹配
默認值。本說明捕獲了推廣時推薦的工作流程
現有網絡從“僅 Ed25519”到“啟用 SM”。

## 1. 驗證構建配置文件

- 使用 `--features sm` 編譯二進製文件；僅當您添加 `sm-ffi-openssl`
  計劃行使OpenSSL/Tongsuo預覽路徑。無需 `sm` 即可構建
  即使配置啟用，功能也會在准入期間拒絕 `sm2` 簽名
  他們。
- 確認 CI 發布 `sm` 工件並且所有驗證步驟（“cargo
  測試 -p iroha_crypto --features sm`、集成裝置、模糊套件）通過
  關於您打算部署的確切二進製文件。

## 2. 層配置覆蓋

`iroha_config` 應用三層：`defaults` → `user` → `actual`。運送 SM
覆蓋運營商分發給驗證者的 `actual` 配置文件中的內容
僅將 `user` 保留為 Ed25519，以便開發人員默認值保持不變。

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

通過“kagami genesis”將相同的塊複製到 `defaults/genesis` 清單中
如果需要，生成…` (add `--allowed-signing sm2 --default-hash sm3-256`
覆蓋），因此 `parameters` 塊和注入的元數據與
運行時配置。當清單和配置相同時，對等點拒絕啟動
快照存在分歧。

## 3. 重新生成創世清單

- 為每個運行 `kagami genesis generate --consensus-mode <mode>`
  環境並提交更新的 JSON 以及 TOML 覆蓋。
- 簽署清單 (`kagami genesis sign …`) 並分發 `.nrt` 有效負載。
  從未簽名的 JSON 清單引導的節點派生運行時加密
  直接從文件進行配置——仍然具有相同的一致性
  檢查。

## 4. 流量前驗證

- 使用新的二進製文件和配置配置臨時集群，然後驗證：
  - 一旦對等點重新啟動，`/status` 就會公開 `crypto.sm_helpers_available = true`。
  - Torii 入場仍然拒絕 SM2 簽名，而 `sm2` 不存在
    `allowed_signing` 並接受混合 Ed25519/SM2 批次時列表
    包括這兩種算法。
  - `iroha_cli tools crypto sm2 export …` 往返密鑰材料通過新種子播種
    默認值。
- 運行涵蓋 SM2 確定性簽名的集成煙霧腳本
  SM3 哈希用於確認主機/VM 一致性。

## 5. 回滾計劃- 記錄逆轉：從 `allowed_signing` 中刪除 `sm2` 並恢復
  `default_hash = "blake2b-256"`。通過相同的 `actual` 推送更改
  配置文件管道，以便每個驗證器單調翻轉。
- 將 SM 清單保存在磁盤上；看到配置和創世不匹配的同行
  數據拒絕啟動，這可以防止部分回滾。
- 如果涉及OpenSSL/Tongsuo預覽版，請包含禁用步驟
  `crypto.enable_sm_openssl_preview` 並從
  運行時環境。

## 參考資料

- [`docs/genesis.md`](../../genesis.md) – 創世清單的結構和
  `crypto` 塊。
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  `iroha_config` 部分和默認值的概述。
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – 結束
  用於運輸 SM 加密的最終操作員清單。