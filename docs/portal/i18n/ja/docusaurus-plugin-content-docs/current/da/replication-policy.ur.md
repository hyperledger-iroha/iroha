---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
ریٹائر ہونے تک دونوں ورژنز کو 同期 رکھیں۔
:::

# データ可用性レプリケーション ポリシー (DA-4)

_進行中 -- 所有者: コア プロトコル WG / ストレージ チーム / SRE_

DA 取り込みパイプライン `roadmap.md` (ワークストリーム DA-4) BLOB クラス
決定的な保持目標を設定するTorii 保持
エンベロープは永続化され、構成済みの呼び出し元は保持されます。
ポリシーの一致の確認 チェック バリデータ/ストレージ ノードのエポック数
レプリカは、送信者の意図を保持するために使用されます。

## デフォルトのポリシー

| BLOB クラス |保温性 |保冷力 |必要なレプリカ |ストレージクラス |ガバナンスタグ |
|-----------|---------------|----------------|---------------------|----------------|--------------|
| `taikai_segment` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6時間 | 7日間 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12時間 | 180日 | 3 | `cold` | `da.governance` |
| _デフォルト (他のすべてのクラス)_ | 6時間 | 30日 | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں 埋め込み ہیں اور تمام
`/v1/da/ingest` の投稿数 پر لاگو ہوتی ہیں۔ Torii 強制保存プロファイル
呼び出し元の値が一致しません فراہم کرتے ہیں マニフェストが表示されます。
警告: オペレータの SDK が古いです。

### Taikai 利用可能クラス

Taikai ルーティング マニフェスト (`taikai.trm`) `availability_class` (`hot`、`warm`、
یا `cold`) 宣言する کرتے ہیں۔ Torii チャンキング マッチング ポリシー
演算子 グローバル テーブル編集 ストリーム ストリーム レプリカ数
スケール سکیں۔デフォルト:

|可用性クラス |保温性 |保冷力 |必要なレプリカ |ストレージクラス |ガバナンスタグ |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6時間 | 30日 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1時間 | 180日 | 3 | `cold` | `da.taikai.archive` |

ヒントがありませんデフォルト طور پر `hot` رکھتے ہیں تاکہ ライブブロードキャスト سب سے مضبوط
ポリシーを維持するネットワークのターゲットをターゲットにします。
`torii.da_ingest.replication_policy.taikai_availability` デフォルト
オーバーライド

## 構成

یہ ポリシー `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک *デフォルト*
テンプレート クラスごとのオーバーライド 配列 クラスごとのオーバーライドクラス識別子
大文字と小文字は区別されません。 `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact`、`custom:<u16>` (政府承認の拡張子)
ありがとうございますストレージ クラス `hot`、`warm`、`cold`

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

デフォルト ブロック ブロック ブロック ブロッククラスを引き締める
アップデートをオーバーライドしますクラス ベースライン クラス ベースライン クラス
`default_retention` 編集Taikai 利用可能クラス کو الگ سے オーバーライド کیا جا سکتا ہے via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## 強制セマンティクス

- Torii ユーザー指定の `RetentionPolicy` 強制プロファイルを置き換えます。
  チャンキング یا マニフェスト排出 سے پہلے۔
- 事前構築済みマニフェストの不一致保持プロファイル宣言 `400 schema mismatch`
  拒否する ہوتے ہیں 古くなったクライアント契約を弱体化させる سکیں۔
- イベント ログを上書きする ہوتا ہے (`blob_class`、送信されたポリシーと期待されるポリシー)
  ロールアウトの結果、準拠していない発信者の数が増加しました。

ゲートを更新しました [データ可用性取り込み計画](ingest-plan.md)
(検証チェックリスト) 保存の実施 表紙の確認

## 再レプリケーションのワークフロー (DA-4 フォローアップ)

保持の執行 صرف پہلا قدم ہے۔オペレーター بھی ثابت کرنا ہوگا کہ ライブ
マニフェスト、レプリケーション順序、構成済みポリシーの説明 SoraFS
非準拠の BLOB は再複製されます。

1. **ドリフト پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` を発する
   呼び出し元の保持値が古いです。ログ記録
   `torii_sorafs_replication_*` テレメトリが不足しています。レプリカが不足しています。
   再展開の遅れ
2. **目的はライブレプリカと差分** 監査ヘルパーの役割:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   コマンド `torii.da_ingest.replication_policy` 設定 سے لوڈ کرتا ہے، ہر
   マニフェスト (JSON یا Norito) デコード کرتا ہے، اور اختیاری طور پر `ReplicationOrderV1`
   ペイロードとマニフェスト ダイジェストと一致するデータ条件フラグの設定:

   - `policy_mismatch` - マニフェスト保持プロファイル強制ポリシー
     (Torii 構成ミスです。)
   - `replica_shortfall` - ライブ レプリケーション順序 `RetentionPolicy.required_replicas`
     レプリカの作成 ターゲットの割り当て 割り当ての作成

   ゼロ以外の終了ステータス فعال shortfall کی نشاندہی کرتا ہے تاکہ CI/オンコール
   自動化ページへのアクセスJSON 文字列
   `docs/examples/da_manifest_review_template.md` パケットを添付します
   議会の投票 ئے دستیاب ہو۔
3. **再レプリケーション トリガー** 監査不足が発生しました。
   `ReplicationOrderV1` ガバナンス ツール経由のセキュリティ
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)
   レプリカ セットが収束するまで監査を行う
   緊急オーバーライド - CLI 出力 - `iroha app da prove-availability` -
   説明 SRE ダイジェスト PDP 証拠参照 説明 説明

回帰カバレッジ `integration_tests/tests/da/replication_policy.rs` میں ہے؛
スイート `/v1/da/ingest` 保持ポリシーが一致しません。検証してください。
呼び出し元のマニフェストの意図を取得する プロファイルを強制的に公開する