---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: SoraFS チャンカー適合ガイド
Sidebar_label: チャンカーの適合性
説明: フィクスチャ SDK 決定論的 SF1 チャンカー プロファイル 説明 要件 ワークフロー
---

:::note メモ
:::

再生成ワークフロー、署名ポリシー、検証手順、ドキュメント、SDK、フィクスチャ消費者同期、確認手順

## 正規プロファイル

- 入力シード (16 進数): `0000000000dec0ded`
- ターゲット サイズ: 262144 バイト (256 KiB)
- 最小サイズ: 65536 バイト (64 KiB)
- 最大サイズ: 524288 バイト (512 KiB)
- ローリング多項式: `0x3DA3358B4DC173`
- ギアテーブルシード: `sorafs-v1-gear`
- ブレークマスク：`0x0000FFFF`

参照実装: `sorafs_chunker::chunk_bytes_with_digests_profile`。
SIMD アクセラレーションの境界線ダイジェストの確認

## フィクスチャバンドル

`cargo run --locked -p sorafs_chunker --bin export_vectors` フィクスチャを再生成します
ةرتا ہے اور درج ذیل فائلیں `fixtures/sorafs_chunker/` میں بناتا ہے:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust、TypeScript、Go コンシューマー向けの正規チャンク境界いいえ
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`، پھر `sorafs.sf1@1.0.0`)۔注文する
  `ensure_charter_compliance` کے ذریعے enforce ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — BLAKE3 検証済みマニフェスト フィクスチャ カバー カバー
- `manifest_signatures.json` — マニフェスト ダイジェスト 議会署名 (Ed25519)۔
- `sf1_profile_v1_backpressure.json` 日本語版 `fuzz/` 日本語版 生のコーパス —
  決定論的ストリーミング シナリオとチャンカー バック プレッシャー テストの実行

### 署名ポリシー

備品の再生 **لازم** 有効な議会署名 شامل کرے۔ジェネレータの符号なし出力を拒否します کرتا ہے جب تک `--allow-unsigned` واضح طور پر نہ دیا جائے (صرف مقامی تجربات کے لیے)۔署名封筒は追加専用 ہوتے ہیں اور 署名者 کے لحاظ سے 重複排除 ہوتے ہیں۔

評議会署名 شامل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 検証

CI ヘルパー `ci/check_sorafs_fixtures.sh` ジェネレーター `--locked` ساتھ دوبارہ چلاتا ہے۔
試合の試合、ドリフト、サインが見つからない、仕事の失敗、など。スクリプト
夜間のワークフロー フィクスチャーの変更 送信 سے پہلے استعمال کریں۔

手動検証手順:

1.`cargo test -p sorafs_chunker`
2. `ci/check_sorafs_fixtures.sh` ٩کل چلائیں۔
3. تصدیق کریں کہ `git status -- fixtures/sorafs_chunker` صاف ہے۔

## プレイブックをアップグレードする

チャンカー プロフィールが SF1 を提案します:

یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
メタデータ要件、提案テンプレート、検証チェックリスト、重要な要素

1. パラメータ `ChunkProfileUpgradeProposalV1` (RFC SF-1 パラメータ)
2. `export_vectors` フィクスチャは、マニフェスト ダイジェストを再生成します。
3. 評議会定足数、マニフェストサイン、マニフェストサイン署名
   `manifest_signatures.json` میں 追加 ہونی چاہئیں۔
4. SDK フィクスチャ (Rust/Go/TS) クロスランタイム パリティ
5. パラメータとファズコーパスの再生成
6. プロフィールハンドル、種子、ダイジェスト、پڈیٹ کریں۔
7. テスト、ロードマップの更新、送信、テスト

チャンク境界、ダイジェスト、無効、マージ、マージ、およびマージ