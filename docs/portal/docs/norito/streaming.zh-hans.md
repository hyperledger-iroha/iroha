---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9df713c3e078ac2ccbd74eb215b91bb80d08306d0ca455dc122fde535601ce8
source_last_modified: "2026-01-18T10:42:52.828202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 流媒体

Norito 流定义了有线格式、控制帧和参考编解码器
用于跨 Torii 和 SoraNet 的实时媒体流。规范规范位于
`norito_streaming.md` 位于工作空间根目录；此页面提炼了以下内容
运营商和 SDK 作者需要配置接触点。

## 线路格式和控制平面

- **清单和框架。** `ManifestV1` 和 `PrivacyRoute*` 描述该段
  时间线、块描述符和路线提示。控制帧（`KeyUpdate`，
  `ContentKeyUpdate` 和节奏反馈）与清单一起存在，因此
  观众可以在解码之前验证承诺。
- **基线编解码器。** `BaselineEncoder`/`BaselineDecoder` 强制单调
  块 ID、时间戳算术和承诺验证。主办方必须致电
  `EncodedSegment::verify_manifest` 在为观众或中继提供服务之前。
- **功能位。** 能力协商通告 `streaming.feature_bits`
  （默认 `0b11` = 基线反馈 + 隐私路由提供商）因此中继和
  客户端可以在没有确定性匹配能力的情况下拒绝对等点。

## 调性、组曲和节奏

- **身份要求。**流控制帧始终用
  Ed25519。专用密钥可以通过提供
  `streaming.identity_public_key`/`streaming.identity_private_key`；否则
  节点身份被重用。
- **HPKE 套件。** `KeyUpdate` 选择最低的通用套件；套件 #1 是
  强制（`AuthPsk`、`Kyber768`、`HKDF-SHA3-256`、`ChaCha20-Poly1305`），
  可选的 `Kyber1024` 升级路径。套件选择存储在
  会话并在每次更新时进行验证。
- **轮换。** 发布者每 64MiB 或 5 分钟发出一次签名的 `KeyUpdate`。
  `key_counter`必须严格增加；回归是一个硬错误。
  `ContentKeyUpdate` 分发滚动组内容密钥，包裹在
  协商的 HPKE 套件，以及通过 ID + 有效性进行门分段解密
  窗口。
- **快照。** `StreamingSession::snapshot_state` 和
  `restore_from_snapshot` 坚持 `{session_id, key_counter, suite, sts_root,
  节奏状态}` under `streaming.session_store_dir`（默认
  `./storage/streaming`）。传输密钥在恢复时重新派生，因此会崩溃
  不要泄露会话秘密。

## 运行时配置

- **钥匙材料。** 提供专用钥匙
  `streaming.identity_public_key`/`streaming.identity_private_key`（Ed25519
  multihash）和可选的 Kyber 材料通过
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`。四个都必须是
  覆盖默认值时出现； `streaming.kyber_suite` 接受
  `mlkem512|mlkem768|mlkem1024`（别名 `kyber512/768/1024`，默认
  `mlkem768`）。
- **编解码器护栏。** CABAC 保持禁用状态，除非构建启用它；
  捆绑 RANS 需要 `ENABLE_RANS_BUNDLES=1`。强制通过
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` 和可选
  提供定制表时为 `streaming.codec.rans_tables_path`。捆绑式
- **SoraNet 路由。** `streaming.soranet.*` 控制匿名传输：
  `exit_multiaddr`（默认 `/dns/torii/udp/9443/quic`）、`padding_budget_ms`
  （默认 25ms），`access_kind`（`authenticated` 与 `read-only`），可选
  `channel_salt`、`provision_spool_dir`（默认
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (默认 0,
  无限制）、`provision_window_segments`（默认 4）和
  `provision_queue_capacity`（默认 256）。
- **同步门。** `streaming.sync` 切换视听漂移强制
  流：`enabled`、`observe_only`、`ewma_threshold_ms` 和 `hard_cap_ms`
  控制何时段因时序漂移而被拒绝。

## 验证和固定装置

- 规范类型定义和助手存在
  `crates/iroha_crypto/src/streaming.rs`。
- 集成覆盖练习 HPKE 握手、内容密钥分发、
  和快照生命周期 (`crates/iroha_crypto/tests/streaming_handshake.rs`)。
  运行 `cargo test -p iroha_crypto streaming_handshake` 以验证流式传输
  局部表面。
- 要深入了解布局、错误处理和未来升级，请阅读
  存储库根目录中的 `norito_streaming.md`。