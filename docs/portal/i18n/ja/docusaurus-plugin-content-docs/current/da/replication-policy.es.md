---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/da/replication_policy.md`。 Mantenga ambas バージョン en
:::

# データ可用性の複製に関する政治 (DA-4)

_Estado: En progreso -- 責任者: コア プロトコル WG / ストレージ チーム / SRE_

DA アホラ アプリケーションの保持決定データのパイプライン
`roadmap.md` のブロブ記述情報 (ワークストリーム DA-4)。 Torii レチャザ
州の保持期間の封筒を保持するための呼び出し元の問い合わせは偶然ではありません
政治構成、ガランティザンド・ケ・カダ・ノド・バリドール/アルマセナミエント・レティエネ
歴史上のレプリカは、その意図に依存するものです。

## 欠陥のある政治

|クラスドブロブ |ホットなリテンション |保冷剤 |レプリカの要求 |アルマセナミエントのクラス |タグ・ド・ゴベルナンザ |
|--------------|--------------|----------------|----------------------------|-------------------------------------|----------------------------|
| `taikai_segment` | 24時間 | 14径 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ホラ | 7径 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ホラ | 180径 | 3 | `cold` | `da.governance` |
| _デフォルト (今日の授業)_ | 6 ホラ |直径30 | 3 | `warm` | `da.default` |

`torii.da_ingest.replication_policy` のセキュリティに関する重要性
今日はお願いです `/v2/da/ingest`。 Torii パフォーマンス コントロールのマニフェストを再記述します
保持の衝動と放出、クアンド・ロス・コーラーズ・エントレガン・バロレス
SDK の非現実化を検出するために、偶然の一致はありません。

### Clases de disponibilidad Taikai

ロスマニフェストデエンルタミエント大会 (`taikai.trm`) 宣言
`availability_class` (`hot`、`warm`、o `cold`)。 Torii アプリカ・ラ・ポリティカ
スペインのオペラドールに向けて、事前に大量の通信を行います
グローバル編集用ストリームのレプリカコンテオ。デフォルト:

|ディスポニビリダードのクラス |ホットなリテンション |保冷剤 |レプリカの要求 |アルマセナミエントのクラス |タグ・ド・ゴベルナンザ |
|----------------------|------|----------------|---------------------|----------------------|---------------------|
| `hot` | 24時間 | 14径 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ホラ |直径30 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1ホラ | 180径 | 3 | `cold` | `da.taikai.archive` |

生体内での感染経路に関する問題 `hot`
Retengan la politica mas fuerte。 Sobrescriba はデフォルトを経由して失われます
`torii.da_ingest.replication_policy.taikai_availability` si su red usa オブジェクト
違う。

## 設定

La politica vive bajo `torii.da_ingest.replication_policy` y expone un template
*デフォルト* はクラスごとにオーバーライドされます。識別情報の喪失はありません
息子はマユス/マイナス y アセプタンに敏感 `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact`、`custom:<u16>` は、拡張機能を拡張します。
アセプタン感染症のクラス `hot`、`warm`、または `cold`。

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
```デヘ エル ブロックは、デフォルトのリストをそのまま使用できます。パラ耐久者
クラス、実際のオーバーライド対応。パラ カンビア ラ ベース デ ヌエバス
クラス、`default_retention` を編集します。

独立した法的規制を遵守するための特別措置の実施
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

- Torii reemplaza el `RetentionPolicy` provisto por el usuario con el perfil
  マニフェストの放出をチャンク化する前に実行します。
- 損失は、明確な保持期間を宣言する前に解釈されることを示しています
  rechazan con `400 schema mismatch` para que los clientes obsoletos no puedan
  デビリタール・エル・コントラート。
- イベント登録を上書きする (`blob_class`、politica enviada vs esperada)
  説明者の呼び出し元は、ロールアウト期間中は適合しません。

Ver [データ可用性の取り込み計画](ingest-plan.md) (検証のチェックリスト)
ゲートを実際に保持するための強制執行。

## 再複製のフルホ (DA-4 のセギミエント)

単独での保持の強制および入門。ロス オペラドーレス タンビアン デベン
プロバー・ケ・ロスは生体内で再現性を発揮します
Alineados con la politica configurada para que SoraFS pueda 再レプリカ BLOB
自動運転の自動運転。

1. **ビジャイル エル ドリフト** Torii を発する
   `overriding DA retention policy to match configured network baseline` クアンド
   電話をかけてきた人は、現実の価値を保持する価値を羨望しています。 Empareje ese ログコン
   ラ テレメトリア `torii_sorafs_replication_*` パラ検出器ファルタンテス デ レプリカ
   o デモラドスを再配置します。
2. **生体内での意図とレプリカの差異。** 聴覚の新しいヘルパーを使用します。

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   構成を指定するコマンド `torii.da_ingest.replication_policy`
   provista、decodifica cada マニフェスト (JSON または Norito)、オプションのエンパレジャ
   マニフェストのダイジェストのペイロード `ReplicationOrderV1`。マルカドスの再開
   条件:

   - `policy_mismatch` - マニフェストの保持に関する詳細情報
     politica impuesta (デベリアの一斉射撃はありません Torii este mal
     設定)。
   - `replica_shortfall` - 生体内でのレプリカの管理
     que `RetentionPolicy.required_replicas` o entrega menos asignaciones que su
     客観的。

   自動化されたファルタンテ活動のステータスはありません
   CI/オンコールのメディアページ。 JSON パケットの付属レポート
   `docs/examples/da_manifest_review_template.md` 議会からの投票。
3. **再複製を無視します。** Cuando la audiotoria reporte un faltante, Emita una
   nueva `ReplicationOrderV1` via las herramientas de gobernanza descritas en
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)
   あなたは、レプリカのセットを聴衆から取り出し、会議を行うことができます。パラ
   緊急時、CLI コン `iroha app da prove-availability` をオーバーライドします。
   パラケ SRE は、参照、ミスモ、ダイジェストおよび証拠 PDP を提供します。人生の回帰問題
`integration_tests/tests/da/replication_policy.rs`;ラ・スイート・エンヴィア・ウナ・ポリティカ
保持期間は偶然ではありません `/v2/da/ingest` とマニフェストの検証
発信者の意図を説明します。