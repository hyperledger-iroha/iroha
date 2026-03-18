---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
リフレテ`docs/source/da/replication_policy.md`。 Gardez les deux バージョン en
:::

# データ可用性のレプリケーションに関するポリシー (DA-4)

_ステータス: 強制 -- 責任者: コア プロトコル WG / ストレージ チーム / SRE_

DA アップリケのメンテナンスの目的を保持するためのパイプラインの取り込み
決定者は、`roadmap.md` によるブロブ クラスの決定を注ぎます (ワークストリーム
DA-4)。 Torii 保持用封筒の永続化を拒否する
発信者 qui ne 特派員 pas a la politique configuree, garantissant que
Chaque noeud validateur/stockage retient le nombre requis d'epoques et de
依存性のないレプリカは、意図によるものではありません。

## デフォルトの政治

|ブロブクラス |ホットリテンション |保冷剤 |レプリカの要件 |クラッセ・ド・ストックケージ |統治タグ |
|--------------|--------------|----------------|----------------------------|----------------------------|----------------------------|
| `taikai_segment` | 24時間 | 14時間 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6時間 | 7時間 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12時間 | 180時間 | 3 | `cold` | `da.governance` |
| _デフォルト (トゥート レ オートル クラス)_ | 6時間 | 30時間 | 3 | `warm` | `da.default` |

`torii.da_ingest.replication_policy` およびアップリケの評価対象者
提出物 `/v1/da/ingest` を宣伝しています。 Torii リクリット ファイル マニフェストの平均値
プロファイル デ リテンションは、発信者に 4 つの不注意を課し、回避することを強制します
廃止された SDK を検出する操作者は、一貫性がありません。

### Classes de disponibilite Taikai

ルートマニフェスト大海 (`taikai.trm`) 宣言 une `availability_class`
(`hot`、`warm`、ou `cold`)。 Torii アップリケ前衛政治特派員
操作者がレプリカの計算を行うためのチャンク化
テーブルグローバルのエディターのないストリーム。デフォルト:

|クラス・ド・ディスポニビライト |ホットリテンション |保冷剤 |レプリカの要件 |クラッセ・ド・ストックケージ |統治タグ |
|----------------------|-----------------|----------------|----------------------|----------------------|-----------|
| `hot` | 24時間 | 14時間 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6時間 | 30時間 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1時間 | 180時間 | 3 | `cold` | `da.taikai.archive` |

`hot` に関するヒントは、LA でライブ放送されています。
ポリティーク・ラ・プラス・フォルテ。 Remplacez のデフォルトは次のとおりです
`torii.da_ingest.replication_policy.taikai_availability` si votre reseau を利用する
違います。

## 構成

`torii.da_ingest.replication_policy` およびテンプレートを公開するための政治政策
*デフォルト* プラス、クラスのオーバーライドを解除します。クラス識別子
大文字と小文字を区別せず、受け入れ可能 `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact`、ou `custom:<u16>` は拡張機能を承認しています
統治。在庫受諾クラス `hot`、`warm`、または `cold`。```toml
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

Laissez le bloc はそのままの状態で utiliser les ci-dessus を提供します。ドゥルシル・ウンを注ぐ
クラス、メッテズ・ア・ジュール・オーバーライド特派員。ポアチェンジャー ラ・ベース・ポア・ド
成金クラス、編集 `default_retention`。

Les class de disponibilite Taikai peuvent etre surchargees independent via
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

## 強制の意味

- Torii を `RetentionPolicy` のプロフィールを使用して置き換えます
  事前のチャンク化とマニフェストの排出を課します。
- マニフェストの前提条件は、保持期間のプロファイルと異なると宣言されます
  Sont rejetes avec `400 schema mismatch` afin que les clientes obsoletes ne
  puissent pas affaiblir le contrat。
- Chaque Evenement d'override est logge (`blob_class`、ポリシー ソウミセと出席者)
  証拠を注ぎ、呼び出し者不適合ペンダントをロールアウトします。

Voir [データ可用性取り込み計画](ingest-plan.md) (検証チェックリスト) を注ぐ
le Gate miss a jour couvrant l'enforcement de Retention.

## 再レプリケーションのワークフロー (スイビ DA-4)

L'enforcement de retentionn'est que la premier etape。操作者の指示
オーストラリアのプルーバーがライブでマニフェストを表示し、複製を維持する
SoraFS のブロックを再リプリケートして、政治的な構成を調整します。
馬は自動化に準拠しています。

1. **ドリフト監視** Torii emet
   `overriding DA retention policy to match configured network baseline` クアンド
   保持期間が廃止された呼び出し元は廃止されました。 Associez ce log avec la
   テレメトリ `torii_sorafs_replication_*` レプリカの不足分を確認する
   再配置が遅れています。
2. **インテントとレプリカの違いをライブで確認します。** 新しい監査を利用します:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   コマンドチャージ `torii.da_ingest.replication_policy` 設定を解除します
   fournie、デコード チャク マニフェスト (JSON または Norito)、および関連付けオプション
   ファイル ペイロード `ReplicationOrderV1` マニフェストのダイジェスト。再開信号
   二重条件:

   - `policy_mismatch` - マニフェストのプロファイルの保持、プロファイルの分岐
     課す (セシネ デブレイト ジャマイズ到着者 sauf si Torii est malconfigure)。
   - `replica_shortfall` - レプリケーションの命令、ライブ要求、レプリカの要求
     que `RetentionPolicy.required_replicas` 4 つの割り当てを要求してください
     サシブル。

   出撃ステータスはゼロではありませんが、体力不足によりアクティブな状態です
   l'自動化 CI/オンコールのポケットベルの即時設定。ジョイネスとの関係 JSON
   au paquet `docs/examples/da_manifest_review_template.md` は票を投じます
   議会。
3. **デクレンシェスは再複製を行います。** 監査は機能不全を示します。
   emettez un nouveau `ReplicationOrderV1` via les outils de gouvernance decrits
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)
   セットのレプリカの収束を監査します。オーバーライドを注ぐ
   緊急、出撃のための CLI アベック `iroha app da prove-availability` が完了しました
   SRE の主要な参照者、ミーム ダイジェストおよび PDP の作成。`integration_tests/tests/da/replication_policy.rs` による回帰のクーベルチュール。
La suite soumet une politique de retention a `/v1/da/ingest` et verifie
マニフェストを回復し、プロフィールを公開し、発信者の意図を明らかにします。