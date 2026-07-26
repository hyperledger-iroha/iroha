---
lang: ja
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//!ペイロード v1 ロールアウトの承認 (SDK 評議会、2026 年 4 月 28 日)。
//!
//! `roadmap.md:M1` で必要な SDK 評議会の決定メモをキャプチャします。
//!暗号化ペイロード v1 ロールアウトには監査可能なレコード (成果物 M1.4) があります。

# Payload v1 ロールアウトの決定 (2026-04-28)

- **議長:** SDK 評議会リード (竹宮 正)
- **投票メンバー:** Swift リード、CLI メンテナ、機密資産 TL、DevRel WG
- **オブザーバー:** プログラム管理、テレメトリ運用

## 入力を確認しました

1. **Swift バインディングとサブミッター** — `ShieldRequest`/`UnshieldRequest`、非同期サブミッター、および Tx ビルダー ヘルパーはパリティ テストとdocs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI 人間工学** — `iroha app zk envelope` ヘルパーは、ロードマップの人間工学要件に合わせて、エンコード/検査ワークフローと障害診断をカバーします。【crates/iroha_cli/src/zk.rs:1256】
3. **決定論的フィクスチャとパリティ スイート** — 共有フィクスチャ + Rust/Swift 検証により Norito バイト/エラー サーフェスを維持【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## 決定

- SDK および CLI の **ペイロード v1 ロールアウトを承認**。これにより、特注の配管なしで Swift ウォレットが機密エンベロープを作成できるようになります。
- **条件:** 
  - パリティ フィクスチャを CI ドリフト アラートの下に維持します (`scripts/check_norito_bindings_sync.py` に関連付けられています)。
  - `docs/source/confidential_assets.md` の運用プレイブックを文書化します (Swift SDK PR によってすでに更新されています)。
  - 本番フラグを反転する前に、キャリブレーションとテレメトリの証拠を記録します (M2 で追跡)。

## アクションアイテム

|オーナー |アイテム |期限 |
|------|------|-----|
|スイフトリード | GA の提供を発表 + README スニペット | 2026-05-01 |
| CLI メンテナ | `iroha app zk envelope --from-fixture` ヘルパーを追加 (オプション) |バックログ (ブロックされていない) |
|開発WG |ペイロード v1 手順によるウォレットのクイックスタートの更新 | 2026-05-05 |

> **注:** このメモは、`roadmap.md:2426` の一時的な「審議会の承認待ち」コールアウトに優先し、トラッカー項目 M1.4 を満たします。フォローアップ アクション アイテムが終了するたびに、`status.md` を更新します。