---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# インシデントとロールバックの手順書

## 目的

ロードマップ **DOCS-9** の項目は、プレイブックのアクション可能性と反復の計画を要求します
管理者なしで安全な医療機器を回収できるポータルの操作者。セッテノート
クーヴル・トロワ事件、砦の信号 - 展開率、複製などの劣化
パンヌ・ダナリティクス - ロールバック・ダリアスを証明するトリメストリクスと文書のドリル
試合ごとに総合的な機能を検証します。

### マテリアル接続

- [`devportal/deploy-guide`](./deploy-guide) - パッケージ化、署名、プロモーションのワークフロー。
- [`devportal/observability`](./observability) - タグ、分析、プローブ参照の ci-dessous をリリースします。
- `docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - 登録とエスカレードの遠隔測定。
- `docs/portal/scripts/sorafs-pin-release.sh` およびヘルパー `npm run probe:*`
  チェックリストを参照します。

### テレメトリとツールの一部

|信号 / 出力 |目的 |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) |レプリケーションの障害と SLA 違反を検出します。 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |バックログの詳細とトリアージの完了までの遅延を定量化します。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs のコート ゲートウェイの導入率が低くなります。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |プローブは、ファイルのリリースと有効なロールバックを合成します。 |
| `npm run check:links` | Gate de liens casses;アフターチャク緩和を利用します。 |
| `sorafs_cli manifest submit ... --alias-*` (ラップされたパー `scripts/sorafs-pin-release.sh`) |エイリアスの昇進/復帰のメカニズム。 |
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |テレメトリの拒否/エイリアス/TLS/レプリケーションを調整します。 PagerDuty のリファレンスをすべて把握しておいてください。 |

## ランブック - 導入率とアーティファクトの欠陥

### 歯止め解除の条件

- プレビュー/プロダクション エコーエント (`npm run probe:portal -- --expect-release=...`) をプローブします。
- アラート Grafana sur `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` は展開予定です。
- QA manuel remarque des Routes の代理人として、すぐに試してみてください
  ラ・プロモーション・デ・ラリアス。

### 即時監禁

1. **Geler les deployments:** marquer le Pipeline CI avec `DEPLOY_FREEZE=1` (ワークフローの入力)
   GitHub) ジェンキンスは仕事を一時停止して、一部のアーティファクトを注ぎました。
2. **キャプチャー レ アーティファクト:** テレチャージャー `build/checksums.sha256`、
   `portal.manifest*.{json,to,bundle,sig}`、その他の構築と検査の調査に出撃します
   ファイルのロールバック参照は、正確な内容をダイジェストします。
3. **事前通知担当者:** ストレージ SRE、リード Docs/DevRel、および役員
   ガバナンスへの意識の向上 (surtout si `docs.sora` est Impacte)。

### ロールバックの手順

1. 識別子ファイル マニフェストの最後に確認された有効なファイル (LKG)。ストック生産におけるワークフロー
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`。
2. 出荷時のヘルパーのエイリアス マニフェストを再設定します。

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

3. 登録者はロールバックの履歴書とインシデントのチケットのダイジェストを記録します
   マニフェスト LKG およびマニフェスト en echec。

### 検証1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` および `sorafs_cli proof verify ...`
   (voir le guide de deploiement) 確認者からマニフェストの再発行に対応します
   toujours au CAR アーカイブ。
4. `npm run probe:tryit-proxy` は、プロキシ Try-It ステージングの収益を保証します。

### 事件後

1. ラシーンを引き起こす前に展開するユニークなパイプラインの反応器。
2. 1 時間のレシピ「教訓」と [`devportal/deploy-guide`](./deploy-guide)
   avec de nouveau ポイント、si besoin。
3. 一連の検査テスト (プローブ、リンク チェッカーなど) を実行して欠陥を洗い出します。

## Runbook - レプリケーションの機能低下

### 歯止め解除の条件

- 警告: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95インチのペンダント10分。
- `torii_sorafs_replication_backlog_total > 10` ペンダント 10 分 (ヴォワール
  `pin-registry-ops.md`)。
- ガバナンスは、解放される前に別名を与えられるよう通知します。

### トリアージ

1. インスペクターのダッシュボード [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) を注ぐ
   確認者は、在庫クラスとプロバイダーの在庫を確実にローカライズします。
2. Croiser のログ Torii の警告 `sorafs_registry::submit_manifest` のログ
   決定者は提出物をエコーエントします。
3. `sorafs_cli manifest status --manifest ...` 経由の Echantillonner la sante des レプリカ
   (プロバイダーごとに結果をリストします)。

### 緩和策

1. レプリカ番号プラス 1 つのマニフェスト (`--pin-min-replicas 7`) を取得します。
   `scripts/sorafs-pin-release.sh` は、プロバイダーに料金を請求するスケジューラーを決定します。
   出来事の記録とヌーボーのダイジェストを登録します。
2. ファイルのバックログは、プロバイダーに固有であり、ファイル スケジューラを介してファイルを一時停止する
   レプリケーション (`pin-registry-ops.md` の文書) と新しいマニフェストの変更
   forcant les autres は rafraichir l'alias を提供します。
3. 別名を削除し、複製、再利用するための批評を行う
   別名、マニフェスト ショー デジャ ステージ (`docs-preview`)、マニフェスト 発行者
   SRE のネットワー クのバックログを調査します。

### 療養等

1. Surveiller `torii_sorafs_replication_sla_total{outcome="missed"}` 保証者クエリを実行します
   コンピュータを安定させます。
2. キャプチャー 出撃 `sorafs_cli manifest status` comme preuve que chaque レプリカ est de
   準拠して帰還します。
3. レプリケーションのバックログの事後分析を 1 時間かけて実行し、プロチェーンを実行する
   etapes (スケーリング プロバイダー、チューニング デュ チャンカーなど)。

## ランブック - テレメトリーによる分析の全体像

### 歯止め解除の条件

- `npm run probe:portal` ダッシュボードが夕方に表示されます
  `AnalyticsTracker` ペンダント >15 分。
- プライバシー レビューは、イベントの放棄を検出します。
- `npm run probe:tryit-proxy` はパス `/probe/analytics` をエコーし​​ます。

### 応答1. ビルドの検証ファイル入力: `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` リリースのアーティファクト (`build/release.json`)。
2. 再実行者 `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` ポインタとファイル
   コレクターはステージングを実行し、ペイロードのアンコールをトラッカーに確認します。
3. コレクターはすぐにダウンし、`DOCS_ANALYTICS_ENDPOINT=""` を定義して再構築します
   トラッカーコートサーキット;荷主ラ・フェネートル・ドタージュ・ダン・ラ・タイムライン。
4. 検証者 `scripts/check-links.mjs` フィンガープリントを続行 `checksums.sha256`
   (サイトマップの検証を *パス* ブロックするための分析)。
5. コレクター ファイルを再作成し、ランサー `npm run test:widgets` 実行ファイルを注ぎます
   再公開前のヘルパー分析による単体テスト。

### 事件後

1. 1 時間にかかる [`devportal/observability`](./observability) 平均的なヌーベルの限界
   サンプリングのコレクターです。
2. 編集者に対するガバナンスの分析に関する注意事項
   政治的なこと。

## 回復力を 3 分間訓練する

ランサー レ ドゥ ドリル デュラン ル **プレミア マルディ ド シャク トリメストル** (1 月/4 月/7 月/10 月)
インフラストラクチャの不可抗力な変更が発表される直前に。ストッカー レ アーティファクト スー
`artifacts/devportal/drills/<YYYYMMDD>/`。

|ドリル |エテープ |プレウヴェ |
| ----- | ----- | -------- |
|ロールバックの繰り返し1. Rejouer ファイルのロールバック「デプロイメント レート」の平均マニフェスト プロダクション ファイルと最近のファイル。<br/>2.本番環境の調査の合格者を再確認します。<br/>3.登録者 `portal.manifest.submit.summary.json` は、ドリル文書の調査ログを記録します。 | `rollback.submit.json`、探査機を出撃し、繰り返しタグを解放します。 |
|監査と検証の統合 | 1. ランサー `npm run probe:portal` と `npm run probe:tryit-proxy` の生産とステージング。<br/>2.ランサー `npm run check:links` およびアーカイバー `build/link-report.json`。<br/>3.すべての Grafana のスクリーンショット/エクスポートは、調査の成功を確認します。 |マニフェストのフィンガープリントのプローブ + `link-report.json` 参照ログ。 |

エスカレーターは、マネージャーのドキュメント/DevRel などのレビュー ガバナンス SRE、カー ルを管理します。
ロードマップは、完全なトリメストリエールを決定し、ロールバックを実行し、エイリアスとプローブを決定します
ポータルの残りの聖者。

## PagerDuty とオンコールの調整- PagerDuty **ドキュメント ポータル パブリッシング** サービスのアラートを所有しています。
  `dashboards/grafana/docs_portal.json`。レグル `DocsPortal/GatewayRefusals`、
  `DocsPortal/AliasCache`、および `DocsPortal/TLSExpiry` ページの主な Docs/DevRel
  avec ストレージ SRE は二次的です。
- EST ページに Quand を追加し、`DOCS_RELEASE_TAG` を含め、スクリーンショットを公開します
  Grafana は、前衛的な事件に関する調査/リンクチェックと出撃に影響を与えます
  緩和策の開始。
- 後の緩和策 (ロールバックまたは再デプロイ)、再実行者 `npm run probe:portal`、
  `npm run check:links`、およびスナップショットのキャプチャー Grafana モントラント ファイル メトリクス
  収益は、生活に影響を与えるものです。事件の証拠を収集する PagerDuty
  前衛的な解像度。
- ミームの一時的な不安定なアラート (TLS の有効期限とバックログなど)、問題
  プレミアの拒否 (出版物の逮捕)、ロールバックの手続きの実行者、ピュイ
  traiter TLS/backlog avec Storage SRE sur le Bridge。