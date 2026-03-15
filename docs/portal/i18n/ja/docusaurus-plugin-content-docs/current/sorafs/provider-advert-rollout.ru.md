---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0f78c9a5b5e359302a30e843698c568c4f34f5dc9d8908d116d250bc371f8d0
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) を基に作成。

# SoraFS プロバイダー広告のロールアウトと互換計画

この計画は、許容的な provider adverts から、マルチソースの chunk 取得に必要な
完全ガバナンスの `ProviderAdvertV1` へ移行するための cut-over を調整する。以下の
三つの deliverables に集中する:

- **オペレーターガイド。** 各 gate が切り替わる前に storage providers が完了すべき
  手順を段階的に示す。
- **テレメトリカバレッジ。** Observability と Ops が、ネットワークが準拠 adverts のみを
  受け付けていることを確認するための dashboards と alerts。
  チームのリリース計画を支援する。

ロールアウトは [SoraFS migration roadmap](./migration-roadmap) の SF-2b/2c マイルストーンに
合わせており、[provider admission policy](./provider-admission-policy) の admission policy が
既に有効である前提で進める。

## フェーズタイムライン

| フェーズ | 期間 (目標) | 挙動 | オペレーターの対応 | Observability の焦点 |
|-------|-----------------|-----------|------------------|-------------------|

## オペレーター向けチェックリスト

1. **adverts の棚卸し。** 公開されている advert を列挙し、以下を記録する:
   - Governing envelope のパス (`defaults/nexus/sorafs_admission/...` または本番相当)。
   - advert の `profile_id` と `profile_aliases`。
   - capability リスト (最低でも `torii_gateway` と `chunk_range_fetch`)。
   - `allow_unknown_capabilities` フラグ (vendor-reserved TLV がある場合必須)。
2. **provider tooling で再生成。**
   - provider advert publisher で payload を再構築し、以下を満たすこと:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` と `max_span` の定義
     - GREASE TLV がある場合 `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` と `sorafs_fetch` で検証する。未知 capabilities の警告は triage する。
3. **マルチソース readiness の検証。**
   - `sorafs_fetch` を `--provider-advert=<path>` で実行する。CLI は `chunk_range_fetch` が
     ないと失敗し、未知 capabilities の無視警告を出す。JSON レポートを保存して
     operations logs と一緒にアーカイブする。
4. **更新のステージング。**
   - gateway enforcement (R2) の 30 日前までに `ProviderAdmissionRenewalV1` envelopes を提出する。
     更新は canonical handle と capability セットを維持し、変更できるのは stake、endpoints、metadata のみ。
5. **依存チームとの連携。**
   - SDK owners は advert が拒否されたときにオペレーターへ警告を出すバージョンをリリースする。
   - DevRel は各フェーズ移行を告知し、dashboard リンクと下記しきい値ロジックを含める。
6. **dashboards と alerts の導入。**
   - Grafana export をインポートし、**SoraFS / Provider Rollout** に配置する。UID は `sorafs-provider-admission`。
   - alert rules が staging と production の共通通知チャネル `sorafs-advert-rollout` を指すことを確認する。

## テレメトリとダッシュボード

以下の metrics は `iroha_telemetry` で公開済み:

- `torii_sorafs_admission_total{result,reason}` — 受理、拒否、warning の件数。
  reasons は `missing_envelope`, `unknown_capability`, `stale`, `policy_violation` を含む。

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)。
共有 dashboards リポジトリ (`observability/dashboards`) にインポートし、
公開前に datasource UID だけ更新する。

ボードは Grafana フォルダ **SoraFS / Provider Rollout** に stable UID
`sorafs-provider-admission` で公開される。alert rules
`sorafs-admission-warn` (warning) と `sorafs-admission-reject` (critical) は
`sorafs-advert-rollout` 通知ポリシーを使用するように事前設定済み。
宛先が変わる場合は dashboard JSON を編集せず contact point を変更する。

推奨 Grafana パネル:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | accept vs warn vs reject を可視化する stack chart。warn > 0.05 * total (warning) または reject > 0 (critical) で alert。 |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | 15 分ロールの 5% warning rate しきい値を駆動する single-line timeseries。 |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | runbook triage の入口。緩和手順へのリンクを添付する。 |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | refresh deadline を逃した providers を示す。discovery cache logs と照合する。 |

手動 dashboards 用の CLI artefacts:

- `sorafs_fetch --provider-metrics-out` は provider ごとに `failures`, `successes`,
  `disabled` カウンタを出力する。production provider を切り替える前の
  orchestrator dry-run 監視に ad-hoc dashboards で利用する。
- JSON レポートの `chunk_retry_rate` と `provider_failure_rate` は、throttling や
  stale payloads の兆候を示し、admission の拒否に先行することが多い。

### Grafana ダッシュボードのレイアウト

Observability は **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) 専用ボードを **SoraFS / Provider Rollout** に
公開し、次の canonical panel IDs を持つ:

- Panel 1 — *Admission outcome rate* (stacked area, 単位 "ops/min")。
- Panel 2 — *Warning ratio* (single series)。式は
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`。
- Panel 3 — *Rejection reasons* (`reason` 別の time series)、`rate(...[5m])` でソート。
- Panel 4 — *Refresh debt* (stat)。上の表のクエリを使い、migration ledger から
  抽出した advert refresh deadlines を注記する。

インフラの dashboards repo で JSON skeleton をコピー (または作成) し、
`observability/dashboards/sorafs_provider_admission.json` に配置する。その後、
更新するのは datasource UID のみ。panel IDs と alert rules は下記 runbooks から
参照されるため、再採番する場合はドキュメントも更新すること。

利便性のため、リポジトリには `docs/source/grafana_sorafs_admission.json` として
参照ダッシュボード定義が含まれる。ローカルテストの出発点に利用してよい。

### Prometheus アラートルール

次の rule group を `observability/prometheus/sorafs_admission.rules.yml` に追加する
(初めての SoraFS ルールグループならファイルを作成)。Prometheus 設定に
このファイルを含める。`<pagerduty>` は実際の on-call routing label に置換する。

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`promtool check rules` が通ることを確認するため、変更前に
`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
を実行する。

## 互換マトリクス

| advert 特性 | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` が存在、canonical aliases、`signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| `chunk_range_fetch` capability が欠落 | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| `allow_unknown_capabilities=true` なしの未知 capability TLV | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| `refresh_deadline` の期限切れ | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (診断用 fixtures) | ✅ (開発のみ) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

すべて UTC 表記。enforcement の日付は migration ledger に反映されており、
カウンシルの投票なしに変更しない。変更する場合は、このファイルと ledger を
同じ PR で更新すること。

> **実装ノート:** R1 で `torii_sorafs_admission_total` に `result="warn"` シリーズを追加する。
> 新しいラベルを追加する Torii ingestion パッチは SF-2 のテレメトリタスクと

## コミュニケーションとインシデント対応

- **週次ステータスメール。** DevRel が admission メトリクス、未解決 warnings、
  直近の期限を短く共有する。
- **インシデント対応。** `reject` アラートが発火した場合、on-call は:
  1. Torii discovery (`/v2/sorafs/providers`) で該当 advert を取得する。
  2. provider pipeline で advert 検証を再実行し、`/v2/sorafs/providers` と比較して
     エラーを再現する。
  3. 次の refresh deadline 前に advert をローテーションするよう provider と調整する。
- **変更凍結。** R1/R2 期間中は capability schema の変更を出さない (rollout 委員会の
  承認がある場合を除く)。GREASE の試験は週次メンテナンスウィンドウに
  スケジュールし、migration ledger に記録する。

## 参考

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
