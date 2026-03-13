---
lang: zh-hans
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% 内容托管通道
% Iroha 核心

# 内容托管通道

内容通道在链上存储小型静态包（tar 档案）并提供服务
直接来自 Torii 的各个文件。

- **发布**：提交带有 tar 存档的 `PublishContentBundle`，可选到期时间
  高度和可选清单。包 ID 是 blake2b 哈希值
  压缩包。 Tar 条目必须是常规文件；名称是规范化的 UTF-8 路径。
  大小/路径/文件计数上限来自 `content` 配置（`max_bundle_bytes`，
  `max_files`、`max_path_len`、`max_retention_blocks`、`chunk_size_bytes`）。
  清单包括 Norito 索引哈希、数据空间/通道、缓存策略
  (`max_age_seconds`、`immutable`)、验证模式(`public` / `role:<role>` /
  `sponsor:<uaid>`)、保留策略占位符和 MIME 覆盖。
- **重复数据删除**：tar 有效负载被分块（默认 64KiB）并每个存储一次
  带有引用计数的哈希值；退役一个包会减少并修剪块。
- **服务**：Torii 暴露 `GET /v2/content/{bundle}/{path}`。响应流
  直接来自块存储，`ETag` = 文件哈希，`Accept-Ranges: bytes`，
  范围支持和源自清单的缓存控制。阅读荣誉
  清单身份验证模式：角色门控和赞助商门控响应需要规范
  签名的请求标头（`X-Iroha-Account`、`X-Iroha-Signature`）
  账户；丢失/过期的捆绑包返回 404。
- **CLI**：现在 `iroha content publish --bundle <path.tar>`（或 `--root <dir>`）
  自动生成清单，发出可选的 `--manifest-out/--bundle-out`，并且
  接受 `--auth`、`--cache-max-age-secs`、`--dataspace`、`--lane`、`--immutable`、
  和 `--expires-at-height` 覆盖。 `iroha content pack --root <dir>` 构建
  确定性 tarball + 清单，无需提交任何内容。
- **配置**：缓存/身份验证旋钮位于 `content.*` 下的 `iroha_config` 中
  （`default_cache_max_age_secs`、`max_cache_max_age_secs`、`immutable_bundles`、
  `default_auth_mode`）并在发布时强制执行。
- **SLO + 限制**：`content.max_requests_per_second` / `request_burst` 和
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` 帽读取端
  吞吐量； Torii 在提供字节和导出之前强制执行
  `torii_content_requests_total`、`torii_content_request_duration_seconds` 和
  带有结果标签的 `torii_content_response_bytes_total` 指标。延迟
  目标位于 `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`。
- **滥用控制**：费率桶由 UAID/API 令牌/远程 IP 和一个
  可选的 PoW 防护（`content.pow_difficulty_bits`、`content.pow_header`）可以
  读取前需要。 DA 条带布局默认值来自
  `content.stripe_layout` 并在收据/清单哈希中得到回显。
- **收据和 DA 证据**：附上成功回复
  `sora-content-receipt`（base64 Norito 帧的 `ContentDaReceipt` 字节）携带
  `bundle_id`、`path`、`file_hash`、`served_bytes`，服务的字节范围，
  `chunk_root` / `stripe_layout`，可选的 PDP 承诺，以及时间戳
  客户可以固定所获取的内容，而无需重新读取正文。

主要参考资料：- 数据模型：`crates/iroha_data_model/src/content.rs`
- 执行：`crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii 处理程序：`crates/iroha_torii/src/content.rs`
- CLI 帮助程序：`crates/iroha_cli/src/content.rs`