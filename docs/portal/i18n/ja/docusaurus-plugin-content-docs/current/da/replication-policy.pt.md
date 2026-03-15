---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
エスペラ `docs/source/da/replication_policy.md`。デュアス・ヴェルソエス・エムとしてのマンテンハ
:::

# データ可用性の複製政策 (DA-4)

_ステータス: 進行中 -- 対応: コア プロトコル WG / ストレージ チーム / SRE_

DA のアゴラ アプリのメタを取り込むパイプラインは、確実に保持される決定性を示します
BLOB 説明クラス `roadmap.md` (ワークストリーム DA-4)。 Torii リクサ・パーシスティル
フォルネシドス・ペロ・コールの封筒、政治家との特派員
configurada、garantindo que cada Node validador/armazenamento retenha o numero
エポカのレプリカは、送信者からの要求に応じて送信されます。

## ポリティカ・パドラオ

|ブロブクラス | Retencao ホット | Retencao 寒い |レプリカの要求 |クラッセ・デ・アルマゼナメント |行政タグ |
|--------------|--------------|---------------|----------------------------|-------------------------------------|----------------------------|
| `taikai_segment` | 24時間 | 14径 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ホラ | 7径 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ホラ | 180径 | 3 | `cold` | `da.governance` |
| _デフォルト（デフォルトクラスとしてのtodas）_ | 6 ホラ |直径30 | 3 | `warm` | `da.default` |

Esses valores sao embutidos em `torii.da_ingest.replication_policy` e aplicados
提出物 `/v2/da/ingest` としての todas。 Torii パーフィルのマニフェストを再取得します
警告を保持し、価値観の相違を発する警告を発する
パラケ オペラドールは、SDK のデサチュアリザドを検出します。

### Classes de disponibilidade Taikai

マニフェスト・デ・ロテアメント大会 (`taikai.trm`) 宣言 `availability_class`
(`hot`、`warm`、ou `cold`)。 Torii 政治特派員としての事前の申請
ストリーム sem のレプリカのオペラドール ポッサムのエスカラー感染症をチャンク化する
tabela グローバルを編集します。デフォルト:

|ディスポニビリダードクラス | Retencao ホット | Retencao 寒い |レプリカの要求 |クラッセ・デ・アルマゼナメント |行政タグ |
|-------------------------------------|--------------|--------------|----------------------------|--------------------------------------|----------------------------|
| `hot` | 24時間 | 14径 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ホラ |直径30 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1ホラ | 180径 | 3 | `cold` | `da.taikai.archive` |

米国 `hot` によるパドラオ パラ ケ トランスミッソス アオ ビボ レテナムのヒント
政治は得意です。 OS のデフォルトを置き換える
`torii.da_ingest.replication_policy.taikai_availability` あなたのことを思い出してください
アルヴォス・ディフェレンテス。

## 設定

政治的勝利のすすり泣く `torii.da_ingest.replication_policy` テンプレートの説明
*デフォルト* はクラスの配列をオーバーライドします。 Identificadores de classe nao
差分マイウスキュラス/マイナスキュラスとアセイタム `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact`、または `custom:<u16>` は政府の範囲内で承認されます。
セキュリティ保護クラス `hot`、`warm`、または `cold`。

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
```Deixe o bloco intacto para robar com os はデフォルトの acima です。パラ耐久者uma
classe、atualize または override 対応者。パラ・ムダル・ベース・デ・ノバス・クラス、
`default_retention` を編集します。

独立した形式での Taikai podem ser sobrescritas のクラス
`torii.da_ingest.replication_policy.taikai_availability`経由:

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

## 施行のセマンティカ

- Torii 置換 `RetentionPolicy` フォルネシド ペロ ウスアリオ ペロ ペルフィル インポスト
  マニフェストを分割して送信する必要があります。
- さまざまな情報を保存するための宣言を事前にマニフェストします。
  rejeitados com `400 schema mismatch` para que clientes obsoletos nao possam
  アンフラケサー・オ・コントラート。
- オーバーライド イベント (`blob_class`、politica enviada vs esperada)
  para expor 呼び出し者 nao は、durante o ロールアウトに準拠します。

Veja [データ可用性取り込み計画](ingest-plan.md) (検証チェックリスト) パラ
o ゲート アタリザド コブリンド エンフォースメント デ リテンカオ。

## 再レプリカのワークフロー (DA-4 セギメント)

おお、保持期間と優先期間の執行。オペラドール タンベム デベム
政治家は、永続的な政策の再現をライブでマニフェストします
SoraFS 形式の再レプリカ BLOB を設定します。
自動。

1. **ドリフトを観察します。** Torii を発します。
   `overriding DA retention policy to match configured network baseline` クアンド
   うーん、呼び出し元の値が保存されています。 esseログコムaを組み合わせる
   テレメトリア `torii_sorafs_replication_*` パラ検出器ファルタ デ レプリカ OU
   アトラサードを再展開します。
2. **インテントとレプリカの違いをライブで確認します。** 新たな聴覚ヘルパーを使用します。

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   カレガ `torii.da_ingest.replication_policy` を設定してください
   fornecida、decodifica cada マニフェスト (JSON ou Norito)、オプションの一致
   ペイロード `ReplicationOrderV1` またはマニフェストのダイジェスト。オー・レスモ・シナリザ・ドゥアス
   コンディコ:

   - `policy_mismatch` - 政治的マニフェストの保持を許可しません
     imposta (Torii esteja mal configurado を参照してください)。
   - `replica_shortfall` - ライブ ソリシタ メノス レプリカの命令
     que `RetentionPolicy.required_replicas` ou fornece menos atribuicoes do que
     ああアルボ。

   ええと、ステータスはゼロだと言いましたが、不足分は自動的に表示されます
   CI/オンコールページの即時対応。 JSON の関連付けに関する付録
   `docs/examples/da_manifest_review_template.md` は議会に投票します。
3. **再複製を無視します。** 聴衆の不足分を報告し、放出します
   novo `ReplicationOrderV1` via as ferramentas de Governmenta descritas em
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)
   私は、レプリカの会議を前に、オーディトリア・ノヴァメントに乗りました。パラオーバーライド
   緊急事態、CLI com `iroha app da prove-availability` パラレルへの緊急対応
   SRE のポッサム参照、メスモ ダイジェスト、証拠 PDP を参照してください。

`integration_tests/tests/da/replication_policy.rs` で問題が解決されました。
`/v2/da/ingest` と Verifica の相違点を示すスイート
呼び出し元のマニフェスト バスカド エキスポを実行し、インポストを実行します。