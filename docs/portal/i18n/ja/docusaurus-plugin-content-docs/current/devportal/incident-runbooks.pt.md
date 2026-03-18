---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# インシデントのランブックとロールバックの訓練

## プロポジト

O 項目のロードマップ **DOCS-9** のプレイブックを実行するための計画を実行する
オペラドールは、ポータルコンシガム回復ファルハスデエンヴィオセムアディヴィンハカオを行います。エスタ ノタ コブレ トレス
大規模な事件 - ファルホスの展開、レプリカの劣化、および分析の実行 - 電子
ドキュメンタ OS は、エイリアスと有効性を検証するためのロールバックをトリメストレイスで実行します。
エンドツーエンドの連続機能。

### 素材関係

- [`devportal/deploy-guide`](./deploy-guide) - パッケージ化、プロモーションのエイリアスの署名のワークフロー。
- [`devportal/observability`](./observability) - タグ、分析、プローブをリリースし、参照します。
- `docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - テレメトリはエスカロナメントの制限を登録します。
- `docs/portal/scripts/sorafs-pin-release.sh` e ヘルパー `npm run probe:*`
  参照番号チェックリスト。

### Telemetria とツールの互換性

|サイナル / ツール |プロポジト |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) | SLA の複製と不正行為のブロックを検出します。 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |トリアージの完了までのバックログと遅延を定量化します。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |ほとんどのファルハスは、ゲートウェイを頻繁に実行し、ルールをデプロイします。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Gateiam がリリースする検証ロールバックをプローブします。 |
| `npm run check:links` |ゲート デ リンクス ケブラドス。米国アポス カダ ミティガカオ。 |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` からの米国) |プロモーション/別名のメカニズム。 |
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |拒否/エイリアス/TLS/レプリカオのテレメトリアの集合体。 PagerDuty に関するアラートは、痛みを伴う証拠を参照します。 |

## Runbook - ファルホとアルティファト ルイムのデプロイ

### コンディコエス デ ディスパロ

- プレビュー/ファルハムのプローブ (`npm run probe:portal -- --expect-release=...`)。
- アラート Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` アポ UM ロールアウト。
- QA マニュアルは、プロキシを実行する必要はありません。今すぐ試してみてください。
  プロモーションのエイリアス。

### すぐにコンテンサオ

1. **Congelar のデプロイ:** marcar o パイプライン CI com `DEPLOY_FREEZE=1` (ワークフローの入力)
   GitHub) あなたの仕事は Jenkins が最も優れたものであることを示しています。
2. **キャプチャー アーティファト:** baixar `build/checksums.sha256`、
   `portal.manifest*.{json,to,bundle,sig}`、つまり dos プローブが com falha para que をビルドすると言いました
   o ロールバック参照 OS は exatos をダイジェストします。
3. **著名な関係者:** ストレージ SRE、ドキュメント/DevRel リード、当直責任者
   ガバナンカ パラ意識 (especialmente quando `docs.sora` esta Impactado)。

### ロールバックの手順

1. マニフェストの最後に確認された正常な状態 (LKG) を識別します。アルマゼナの生産ワークフロー
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`。
2. 発送ヘルパーのマニフェストのエイリアスを再設定します。

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. レジストリまたはレスモをロールバックし、チケットを発行せずにイベントを実行し、COMOS ダイジェストを実行します
   マニフェスト LKG はマニフェスト com falha を実行します。

### バリダカオ1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` と `sorafs_cli proof verify ...`
   (展開の確認) 継続的なマニフェストの再現を確認する
   バテンド・コム・オー・カー・アーキバード。
4. `npm run probe:tryit-proxy` は、プロキシ Try-It ステージング ボルトを保証します。

### 事件後

1. 原因を特定するためにパイプラインを展開し、保管する。
2. 「教訓」としてのプリエンチャ em [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos、se houver.
3. テスト ファルハダ スイート (プローブ、リンク チェッカーなど) の Abra 欠陥。

## ランブック - 複製のデグラダカオ

### コンディコエス デ ディスパロ

- アラート: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  10 分あたり 0.95 秒。
- `torii_sorafs_replication_backlog_total > 10` ポル 10 分 (veja
  `pin-registry-ops.md`)。
- Governanca reporta 別名 lento apos um リリース。

### トリアージ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) パラメタのダッシュボードを検査する
   バックログのローカル化とストレージの管理、プロバイダーのフリートの確認を行います。
2. Cruze ログに Torii por warnings `sorafs_registry::submit_manifest` para determinar se
   提出物として estao falhando。
3. `sorafs_cli manifest status --manifest ...` 経由でレプリカを再生する (lista
   プロバイダーによる結果）。

### ミティガカオ

1. レプリカの感染マニフェストの再発行 (`--pin-min-replicas 7`)
   `scripts/sorafs-pin-release.sh` パラケオ スケジューラ配布カーガ セット メイン
   プロバイダー。新規ダイジェストを登録し、ログは発生しません。
2. プロバイダー unico のバックログを事前に確認し、o 経由で一時的にデサビリテを実行する
   レプリカのスケジューラ (ドキュメント em `pin-registry-ops.md`) をもう一度羨望の的に見る
   cando os outros プロバイダーのマニフェストは、エイリアスまたはエイリアスを提供します。
3. Quando a frescura do alias for mais crica que a paridade de replicacao, re-vincule o
   別名、マニフェスト クエンテ、ステージング (`docs-preview`)、公開マニフェスト デポイ
   コンパンハメント、SRE リンパー、バックログ。

### 回復とフェチャメント

1. 保証付き `torii_sorafs_replication_sla_total{outcome="missed"}` を監視します
   コンタドールが安定する。
2. Saida `sorafs_cli manifest status` como evidencia de que cada レプリカ ボルトをキャプチャします。
   コンフィダード。
3. アブラは、過去のレプリカのバックログを事後処理して実際に作成します
   (プロバイダーのスケーリング、チャンカーのチューニングなど)。

## Runbook - 分析とテレメトリの実行手順

### コンディコエス デ ディスパロ

- `npm run probe:portal` パス、マス ダッシュボード パラメータの設定イベント
  `AnalyticsTracker` 時間 > 15 分。
- プライバシー レビューはイベントを報告するためのものです。
- `npm run probe:tryit-proxy` ファルハエムパス `/probe/analytics`。

### レスポスタ1. ビルドの入力を検証します: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` アーティファト リリースはありません (`build/release.json`)。
2. `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para o を再実行します。
   コレクターは、ペイロードを発行するためのトラッカーの確認のためのステージングを行います。
3. SE OS コレクターがダウンし、`DOCS_ANALYTICS_ENDPOINT=""` を再構築することが定義されています
   パラケオトラッカーファカショートサーキット。ナリンハの停電を登録する
   ド・テンポ・ド・インシデンテ。
4. `scripts/check-links.mjs` の指紋を `checksums.sha256` から有効にします。
   (サイトマップを検証するための分析 *nao* 開発ブロック)。
5. Quando コレクター Voltar、`npm run test:widgets` パラ Exercitar OS 単体テストに参加
   再公開する前に分析のヘルパーを実行します。

### 事件後

1. Atualize [`devportal/observability`](./observability) com novaslimitacoes do Collector
   あなたはアモスストラジェムを要求しています。
2. 分析フォーラムの管理と管理の強化
   政治だ。

## 回復力を訓練する

**primeira terca-feira de cada trimestre** の期間にわたる訓練を実行する (1 月/4 月/7 月/終了)
主要なインフラストラクチャを今すぐに確認してください。アルマゼン アルテファトス エム
`artifacts/devportal/drills/<YYYYMMDD>/`。

|ドリル |パソス |証拠 |
| ----- | ----- | -------- |
|別名ロールバックの説明 | 1. 「ファルホのデプロイ」のロールバックを繰り返し、最近の制作マニフェストを実行します。<br/>2.必要に応じてプローブを再採取します。<br/>3.レジストラ `portal.manifest.submit.summary.json` は、パスタやドリルのプローブのログを記録します。 | `rollback.submit.json`、プローブは、タグを解放すると言いました。 |
|オーディトリア デ バリダカオ シンテティカ | 1. Rodar `npm run probe:portal` および `npm run probe:tryit-proxy` コントラプロダクションステージング。<br/>2. Rodar `npm run check:links` および arquivar `build/link-report.json`。<br/>3. Anexar のスクリーンショット/エクスポートは Grafana を確認し、プローブを成功させます。 |プローブのログ + `link-report.json` 参照、フィンガープリントのマニフェスト。 |

Escalone は、ドキュメント/DevRel マネージャーの権限で SRE ガバナンスの見直しを行います。
ロードマップの証拠を示すトリメストラル決定論的なロールバックとエイリアス EOS
プローブはポータルを継続的に実行します。

## オンコールで PagerDuty を調整します- PagerDuty **ドキュメント ポータル公開** のサービスを提供し、一部のアラートを通知します
  `dashboards/grafana/docs_portal.json`。 `DocsPortal/GatewayRefusals` として、
  `DocsPortal/AliasCache`、`DocsPortal/TLSExpiry` Docs/DevRel のプライマリページ
  com ストレージ SRE como secundario。
- `DOCS_RELEASE_TAG` を含むページを開く、Grafana の別紙スクリーンショット
  不正行為は、事件発生前に調査/リンクチェックに関連するものではありません
  ミティガカオ。
- Depois da mitigacao (ロールバックまたは再デプロイ)、`npm run probe:portal` を再実行、
  `npm run check:links`、スナップショットをキャプチャ Grafana メトリカとしての最新のモストランド
  de volta aos しきい値。 PagerDuty で発生した証拠を添付する別紙
  アンテ・デ・リゾルバー。
- 管理テンポの違いによるアラートの発生 (TLS のバックログの期限切れの例)、
  トリアージ拒否のプライマリ (出版パラレル)、ロールバック手順の実行
  resolva TLS/バックログ com ストレージ SRE をデポジットします。