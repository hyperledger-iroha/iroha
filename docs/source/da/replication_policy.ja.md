---
lang: ja
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# データ可用性レプリケーション ポリシー (DA-4)

_ステータス: 進行中 — 所有者: コア プロトコル WG / ストレージ チーム / SRE_

DA 取り込みパイプラインは、決定的な保持目標を強制するようになりました。
`roadmap.md` (ワークストリーム DA-4) に記述されているすべての BLOB クラス。 Torii は拒否します
設定されたエンベロープと一致しない呼び出し元提供の保持エンベロープを永続化する
ポリシー。すべてのバリデータ/ストレージ ノードが必要なデータを保持することを保証します。
送信者の意図に依存せずに、エポックとレプリカの数を決定します。

## デフォルトのポリシー

| BLOB クラス |保温性 |保冷力 |必要なレプリカ |ストレージクラス |ガバナンスタグ |
|-----------|---------------|----------------|---------------------|----------------|--------------|
| `taikai_segment` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6時間 | 7日間 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12時間 | 180日 | 3 | `cold` | `da.governance` |
| _デフォルト (他のすべてのクラス)_ | 6時間 | 30日 | 3 | `warm` | `da.default` |

これらの値は `torii.da_ingest.replication_policy` に埋め込まれ、
すべての `/v2/da/ingest` 送信。 Torii は、強制されたものでマニフェストを書き換えます。
保持プロファイルを作成し、呼び出し元が一致しない値を指定した場合に警告を発します。
オペレーターは古い SDK を検出できます。

### Taikai 利用可能クラス

Taikai ルーティング マニフェスト (`taikai.trm` メタデータ) には、
`availability_class` ヒント (`Hot`、`Warm`、または `Cold`)。存在する場合、Torii
`torii.da_ingest.replication_policy` から一致する保持プロファイルを選択します
ペイロードをチャンク化する前に、イベント オペレーターが非アクティブな状態をダウングレードできるようにする
グローバル ポリシー テーブルを編集せずにレンディションを実行できます。デフォルトは次のとおりです。

|可用性クラス |保温性 |保冷力 |必要なレプリカ |ストレージクラス |ガバナンスタグ |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6時間 | 30日 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1時間 | 180日 | 3 | `cold` | `da.taikai.archive` |

マニフェストで `availability_class` が省略されている場合、取り込みパスは
`hot` プロファイルなので、ライブ ストリームは完全なレプリカ セットを保持します。オペレーターは次のことができます
新しい値を編集してこれらの値をオーバーライドします。
構成内の `torii.da_ingest.replication_policy.taikai_availability` ブロック。

## 構成

このポリシーは `torii.da_ingest.replication_policy` の下に存在し、
*デフォルト* テンプレートとクラスごとのオーバーライドの配列。クラス識別子は、
大文字と小文字は区別されず、`taikai_segment`、`nexus_lane_sidecar`、を受け入れます。
`governance_artifact`、またはガバナンスが承認した拡張子の場合は `custom:<u16>`。
ストレージ クラスは、`hot`、`warm`、または `cold` を受け入れます。

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

上記のデフォルトで実行するには、ブロックをそのままにしておきます。締めるには
クラス、一致するオーバーライドを更新します。新しいクラスのベースラインを変更するには、
`default_retention`を編集します。特定の Taikai 利用可能クラスを調整するには、以下にエントリを追加します。
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## 強制セマンティクス

- Torii は、ユーザー指定の `RetentionPolicy` を強制プロファイルに置き換えます。
  チャンク化またはマニフェスト放出の前に。
- 不一致の保持プロファイルを宣言する事前構築済みマニフェストは拒否されます
  `400 schema mismatch` を使用するため、古いクライアントは契約を弱めることができません。
- すべてのオーバーライド イベントがログに記録されます (`blob_class`、送信されたポリシーと予想されるポリシー)
  ロールアウト中に非準拠の発信者を明らかにするため。

更新されたゲートについては、`docs/source/da/ingest_plan.md` (検証チェックリスト) を参照してください。
保持の強制をカバーします。

## 再レプリケーションのワークフロー (DA-4 フォローアップ)

保持の強制は最初のステップにすぎません。オペレーターは次のことも証明する必要があります。
ライブマニフェストとレプリケーションの順序は、設定されたポリシーと常に一致するため、
SoraFS は、コンプライアンス違反の BLOB を自動的に再レプリケートできるということです。

1. **ドリフトに注意してください。** Torii が出力されます。
   `overriding DA retention policy to match configured network baseline` いつでも
   呼び出し元が古い保持値を送信します。そのログを次とペアリングします
   レプリカの不足または遅延を特定する `torii_sorafs_replication_*` テレメトリ
   再配置。
2. **意図とライブレプリカの違い** 新しい監査ヘルパーを使用します。

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   このコマンドは、提供されたファイルから `torii.da_ingest.replication_policy` をロードします。
   config を作成し、各マニフェスト (JSON または Norito) をデコードし、オプションで任意のマニフェストと一致します。
   マニフェスト ダイジェストによる `ReplicationOrderV1` ペイロード。要約では 2 つのフラグが立てられます
   条件:

   - `policy_mismatch` – マニフェスト保持プロファイルが強制されたプロファイルと異なる
     ポリシー (Torii が正しく構成されていない限り、これは決して起こりません)。
   - `replica_shortfall` – ライブ レプリケーションの順序で要求されるレプリカの数が
     `RetentionPolicy.required_replicas` またはその割り当てよりも少ない割り当てを提供します
     ターゲット。

   ゼロ以外の終了ステータスはアクティブな不足を示しているため、CI/オンコール自動化
   すぐにページを開くことができます。 JSON レポートを
   議会投票用の `docs/examples/da_manifest_review_template.md` パケット。
3. **再レプリケーションをトリガーします。** 監査で不足が報告された場合、新しいレプリケーションを発行します。
   `ReplicationOrderV1` で説明されているガバナンス ツール経由
   `docs/source/sorafs/storage_capacity_marketplace.md` を選択し、監査を再実行します。
   レプリカ セットが収束するまで。緊急オーバーライドの場合は、CLI 出力をペアリングします。
   SRE が同じダイジェストを参照できるように、`iroha app da prove-availability` を使用します。
   そしてPDPの証拠。

回帰カバレッジは `integration_tests/tests/da/replication_policy.rs` にあります。
スイートは、不一致の保持ポリシーを `/v2/da/ingest` に送信し、検証します。
フェッチされたマニフェストが呼び出し元ではなく強制されたプロファイルを公開すること
意図。

## 健全性の証明テレメトリーとダッシュボード (DA-5 ブリッジ)

ロードマップ項目 **DA-5** では、PDP/PoTR 施行の結果が監査可能であることが求められています。
リアルタイム。 `SorafsProofHealthAlert` イベントは、専用のセットを駆動するようになりました。
Prometheus メトリクス:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Health** Grafana ボード
(`dashboards/grafana/sorafs_pdp_potr_health.json`) はこれらのシグナルを公開するようになりました。- *トリガーごとの健全性アラートの証明* は、トリガー/ペナルティ フラグごとにアラート率をグラフ化します。
  Taikai/CDN オペレーターは、PDP のみ、PoTR のみ、またはデュアルストライクが有効であるかどうかを証明できます。
  発砲中。
- *クールダウン中のプロバイダー* は、現在クールダウン中のプロバイダーのライブ合計をレポートします。
  SorafsProofHealthAlert のクールダウン。
- *Proof Health Window Snapshot* は、PDP/PoTR カウンター、ペナルティ金額、
  クールダウン フラグとストライク ウィンドウ終了エポックをプロバイダーごとに設定できるため、ガバナンス レビュー担当者が
  インシデントパケットにテーブルを添付できます。

ランブックは、DA 施行の証拠を提示するときにこれらのパネルをリンクする必要があります。彼らは
CLI プルーフストリームの失敗をオンチェーンのペナルティメタデータに直接結び付け、
ロードマップで呼び出される可観測性フックを提供します。