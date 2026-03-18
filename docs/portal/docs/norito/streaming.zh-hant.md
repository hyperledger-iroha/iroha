---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9df713c3e078ac2ccbd74eb215b91bb80d08306d0ca455dc122fde535601ce8
source_last_modified: "2026-01-18T10:42:52.828202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 流媒體

Norito 流定義了有線格式、控制幀和參考編解碼器
用於跨 Torii 和 SoraNet 的實時媒體流。規範規範位於
`norito_streaming.md` 位於工作空間根目錄；此頁面提煉了以下內容
運營商和 SDK 作者需要配置接觸點。

## 線路格式和控制平面

- **清單和框架。 ** `ManifestV1` 和 `PrivacyRoute*` 描述該段
  時間線、塊描述符和路線提示。控制幀（`KeyUpdate`，
  `ContentKeyUpdate` 和節奏反饋）與清單一起存在，因此
  觀眾可以在解碼之前驗證承諾。
- **基線編解碼器。 ** `BaselineEncoder`/`BaselineDecoder` 強制單調
  塊 ID、時間戳算術和承諾驗證。主辦方必須致電
  `EncodedSegment::verify_manifest` 在為觀眾或中繼提供服務之前。
- **功能位。 ** 能力協商通告 `streaming.feature_bits`
  （默認 `0b11` = 基線反饋 + 隱私路由提供商）因此中繼和
  客戶端可以在沒有確定性匹配能力的情況下拒絕對等點。

## 調性、組曲和節奏

- **身份要求。 **流控制幀始終用
  Ed25519。專用密鑰可以通過提供
  `streaming.identity_public_key`/`streaming.identity_private_key`；否則
  節點身份被重用。
- **HPKE 套件。 ** `KeyUpdate` 選擇最低的通用套件；套件 #1 是
  強制（`AuthPsk`、`Kyber768`、`HKDF-SHA3-256`、`ChaCha20-Poly1305`），
  可選的 `Kyber1024` 升級路徑。套件選擇存儲在
  會話並在每次更新時進行驗證。
- **輪換。 ** 發布者每 64MiB 或 5 分鐘發出一次簽名的 `KeyUpdate`。
  `key_counter`必須嚴格增加；回歸是一個硬錯誤。
  `ContentKeyUpdate` 分發滾動組內容密鑰，包裹在
  協商的 HPKE 套件，以及通過 ID + 有效性進行門分段解密
  窗口。
- **快照。 ** `StreamingSession::snapshot_state` 和
  `restore_from_snapshot` 堅持 `{session_id, key_counter, suite, sts_root,
  節奏狀態}` under `streaming.session_store_dir`（默認
  `./storage/streaming`）。傳輸密鑰在恢復時重新派生，因此會崩潰
  不要洩露會話秘密。

## 運行時配置

- **鑰匙材料。 ** 提供專用鑰匙
  `streaming.identity_public_key`/`streaming.identity_private_key`（Ed25519
  multihash）和可選的 Kyber 材料通過
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`。四個都必須是
  覆蓋默認值時出現； `streaming.kyber_suite` 接受
  `mlkem512|mlkem768|mlkem1024`（別名 `kyber512/768/1024`，默認
  `mlkem768`）。
- **編解碼器護欄。 ** CABAC 保持禁用狀態，除非構建啟用它；
  捆綁 RANS 需要 `ENABLE_RANS_BUNDLES=1`。強制通過
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` 和可選
  提供定製表時為 `streaming.codec.rans_tables_path`。捆綁式
- **SoraNet 路由。 ** `streaming.soranet.*` 控制匿名傳輸：
  `exit_multiaddr`（默認 `/dns/torii/udp/9443/quic`）、`padding_budget_ms`
  （默認 25ms），`access_kind`（`authenticated` 與 `read-only`），可選
  `channel_salt`、`provision_spool_dir`（默認
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (默認 0,
  無限制）、`provision_window_segments`（默認 4）和
  `provision_queue_capacity`（默認 256）。
- **同步門。 ** `streaming.sync` 切換視聽漂移強制
  流：`enabled`、`observe_only`、`ewma_threshold_ms` 和 `hard_cap_ms`
  控制何時段因時序漂移而被拒絕。

## 驗證和固定裝置

- 規範類型定義和助手存在
  `crates/iroha_crypto/src/streaming.rs`。
- 集成覆蓋練習 HPKE 握手、內容密鑰分發、
  和快照生命週期 (`crates/iroha_crypto/tests/streaming_handshake.rs`)。
  運行 `cargo test -p iroha_crypto streaming_handshake` 以驗證流式傳輸
  局部表面。
- 要深入了解佈局、錯誤處理和未來升級，請閱讀
  存儲庫根目錄中的 `norito_streaming.md`。