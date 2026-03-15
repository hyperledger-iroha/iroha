---
lang: zh-hant
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% 內容託管通道
% Iroha 核心

# 內容託管通道

內容通道在鏈上存儲小型靜態包（tar 檔案）並提供服務
直接來自 Torii 的各個文件。

- **發布**：提交帶有 tar 存檔的 `PublishContentBundle`，可選到期時間
  高度和可選清單。包 ID 是 blake2b 哈希值
  壓縮包。 Tar 條目必須是常規文件；名稱是規範化的 UTF-8 路徑。
  大小/路徑/文件計數上限來自 `content` 配置（`max_bundle_bytes`，
  `max_files`、`max_path_len`、`max_retention_blocks`、`chunk_size_bytes`）。
  清單包括 Norito 索引哈希、數據空間/通道、緩存策略
  (`max_age_seconds`、`immutable`)、驗證模式(`public` / `role:<role>` /
  `sponsor:<uaid>`)、保留策略佔位符和 MIME 覆蓋。
- **重複數據刪除**：tar 有效負載被分塊（默認 64KiB）並每個存儲一次
  帶有引用計數的哈希值；退役一個包會減少並修剪塊。
- **服務**：Torii 暴露 `GET /v1/content/{bundle}/{path}`。響應流
  直接來自塊存儲，`ETag` = 文件哈希，`Accept-Ranges: bytes`，
  範圍支持和源自清單的緩存控制。閱讀榮譽
  清單身份驗證模式：角色門控和讚助商門控響應需要規範
  簽名的請求標頭（`X-Iroha-Account`、`X-Iroha-Signature`）
  賬戶；丟失/過期的捆綁包返回 404。
- **CLI**：現在 `iroha content publish --bundle <path.tar>`（或 `--root <dir>`）
  自動生成清單，發出可選的 `--manifest-out/--bundle-out`，並且
  接受 `--auth`、`--cache-max-age-secs`、`--dataspace`、`--lane`、`--immutable`、
  和 `--expires-at-height` 覆蓋。 `iroha content pack --root <dir>` 構建
  確定性 tarball + 清單，無需提交任何內容。
- **配置**：緩存/身份驗證旋鈕位於 `content.*` 下的 `iroha_config` 中
  （`default_cache_max_age_secs`、`max_cache_max_age_secs`、`immutable_bundles`、
  `default_auth_mode`）並在發佈時強制執行。
- **SLO + 限制**：`content.max_requests_per_second` / `request_burst` 和
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` 帽讀取端
  吞吐量； Torii 在提供字節和導出之前強制執行
  `torii_content_requests_total`、`torii_content_request_duration_seconds` 和
  帶有結果標籤的 `torii_content_response_bytes_total` 指標。延遲
  目標位於 `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`。
- **濫用控制**：費率桶由 UAID/API 令牌/遠程 IP 和一個
  可選的 PoW 防護（`content.pow_difficulty_bits`、`content.pow_header`）可以
  讀取前需要。 DA 條帶佈局默認值來自
  `content.stripe_layout` 並在收據/清單哈希中得到回顯。
- **收據和 DA 證據**：附上成功回复
  `sora-content-receipt`（base64 Norito 幀的 `ContentDaReceipt` 字節）攜帶
  `bundle_id`、`path`、`file_hash`、`served_bytes`，服務的字節範圍，
  `chunk_root` / `stripe_layout`，可選的 PDP 承諾，以及時間戳
  客戶可以固定所獲取的內容，而無需重新讀取正文。

主要參考資料：- 數據模型：`crates/iroha_data_model/src/content.rs`
- 執行：`crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii 處理程序：`crates/iroha_torii/src/content.rs`
- CLI 幫助程序：`crates/iroha_cli/src/content.rs`