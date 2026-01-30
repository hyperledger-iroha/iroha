---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/da/replication-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b97ecfd9939d003396f261a664d41417f3f7a0b8348043cad08800a51614aff3
source_last_modified: "2025-11-14T04:43:19.749934+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正規ソース
このページは `docs/source/da/replication_policy.md` を反映します。旧ドキュメントが
退役するまで両方のバージョンを同期してください。
:::

# Data Availability レプリケーションポリシー (DA-4)

_ステータス: 進行中 -- オーナー: Core Protocol WG / Storage Team / SRE_

DA ingest パイプラインは、`roadmap.md` (DA-4 ワークストリーム) に記載された
各 blob クラスに対して決定的な retention 目標を適用します。Torii は
呼び出し側が提供した retention envelope が設定ポリシーと一致しない場合、
保存を拒否し、全てのバリデータ/ストレージノードが必要なエポック数と
レプリカ数を保持できるようにします。

## デフォルトポリシー

| Blob クラス | Hot retention | Cold retention | 必要レプリカ | Storage クラス | Governance タグ |
|------------|---------------|----------------|---------------|----------------|----------------|
| `taikai_segment` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 hours | 7 days | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 hours | 180 days | 3 | `cold` | `da.governance` |
| _Default (その他すべて)_ | 6 hours | 30 days | 3 | `warm` | `da.default` |

これらの値は `torii.da_ingest.replication_policy` に埋め込まれ、
`/v1/da/ingest` の全送信に適用されます。Torii は強制された retention
プロファイルで manifest を書き換え、呼び出し側が不一致の値を指定した
場合は警告を出して古い SDK を検知できるようにします。

### Taikai availability classes

Taikai ルーティング manifest (`taikai.trm`) は `availability_class`
(`hot`, `warm`, `cold`) を宣言します。Torii は chunking 前に該当ポリシー
を強制し、グローバル表を編集せずにストリーム単位でレプリカ数を
スケールできます。Defaults:

| Availability class | Hot retention | Cold retention | 必要レプリカ | Storage クラス | Governance タグ |
|--------------------|---------------|----------------|---------------|----------------|----------------|
| `hot` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 hours | 30 days | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hour | 180 days | 3 | `cold` | `da.taikai.archive` |

ヒントがない場合は `hot` にフォールバックし、ライブ配信が最も強い
ポリシーを保持します。ネットワークの目標が異なる場合は
`torii.da_ingest.replication_policy.taikai_availability` で上書きします。

## 設定

ポリシーは `torii.da_ingest.replication_policy` にあり、*default* テンプレート
とクラス別 override 配列を提供します。クラス識別子は大文字小文字を
区別せず、`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`,
`custom:<u16>` (ガバナンス承認拡張) を受け入れます。Storage クラスは
`hot`, `warm`, `cold` を受け入れます。

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

上記のデフォルトで動かす場合はブロックをそのままにしてください。
クラスを厳格化する場合は該当 override を更新し、新しいクラスのベースを
変える場合は `default_retention` を編集します。

Taikai availability classes は `torii.da_ingest.replication_policy.taikai_availability`
で個別に上書きできます:

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

## Enforcement semantics

- Torii はユーザー指定の `RetentionPolicy` を強制プロファイルで置き換え、
  chunking または manifest 生成の前に適用します。
- 既存の manifest が不一致の retention プロファイルを宣言している場合、
  `400 schema mismatch` で拒否し、古いクライアントが契約を弱めることを
  防ぎます。
- すべての override イベントは (`blob_class`, 提出値 vs 期待値) をログし、
  rollout 中の不適合 caller を可視化します。

[Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) を参照し、
retention enforcement を含む最新ゲートを確認してください。

## Re-replication workflow (DA-4 follow-up)

retention enforcement は第一歩にすぎません。オペレーターは、live manifest
と replication order が設定ポリシーと整合していることを証明し、SoraFS が
非準拠 blob を自動で再レプリケートできるようにする必要があります。

1. **Drift を監視。** Torii は
   `overriding DA retention policy to match configured network baseline` を
   出力し、caller が古い retention 値を送信したことを示します。
   `torii_sorafs_replication_*` テレメトリと組み合わせて、レプリカ不足や
   遅延再配置を検知します。
2. **Intent と live replicas を比較。** 新しい監査ヘルパーを使います:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   コマンドは `torii.da_ingest.replication_policy` を設定から読み込み、
   各 manifest (JSON/Norito) をデコードし、必要に応じて `ReplicationOrderV1`
   を manifest digest で突き合わせます。サマリーは次を示します:

   - `policy_mismatch` - manifest の retention プロファイルが強制ポリシー
     と乖離 (Torii の誤設定がない限り起きないはずです)。
   - `replica_shortfall` - live replication order が
     `RetentionPolicy.required_replicas` 未満を要求する、または割当が
     目標未達。

   非ゼロ終了は短缺が発生中であることを示し、CI/on-call 自動化が即時
   ページできるようにします。JSON レポートを
   `docs/examples/da_manifest_review_template.md` パケットに添付し、
   議会投票に回してください。
3. **再レプリケーションを起動。** 監査が短缺を報告したら、
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   で説明されているガバナンスツールを使って新しい `ReplicationOrderV1`
   を発行し、レプリカが収束するまで監査を再実行します。緊急 override
   の場合は CLI 出力を `iroha app da prove-availability` と組み合わせ、SRE が
   同じ digest と PDP 証跡を参照できるようにします。

回帰カバレッジは `integration_tests/tests/da/replication_policy.rs` にあり、
不一致の retention ポリシーを `/v1/da/ingest` に送って、取得した manifest が
caller の意図ではなく強制プロファイルを公開することを検証します。
