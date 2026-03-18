---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# インシデントのランブックとロールバックの訓練

## プロポジト

ロードマップ **DOCS-9** のアイテムは、実行計画の実行に必要なアクションを実行します。
ロス・オペラドーレス・デル・ポータル・プエダン・レキュペラーセ・デ・フォールス・デ・エントレガ・シン・アディヴィナー。エスタノート
アルタセナル事件の解決: 落下、複製の劣化
分析の成果、プルーバンとロールバックの期間を経て、ロスドリルを記録する
別名 y la validacion sintetica siguen funcionando エンドツーエンド。

### 素材関係

- [`devportal/deploy-guide`](./deploy-guide) - パッケージング、プロモーションのエイリアスに署名します。
- [`devportal/observability`](./observability) - タグをリリースし、分析と調査を参照します。
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - 遠隔登録とエスカラミエント管理。
- `docs/portal/scripts/sorafs-pin-release.sh` y ヘルパー `npm run probe:*`
  チェックリストを参照してください。

### テレメトリアとツールのコンパルティド

|セナル / ツール |プロポジト |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) | SLA の複製と違反のブロックを検出します。 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |豊富なバックログとトリアージの完了までの待ち時間。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |ゲートウェイの展開に失敗する可能性があります。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Gatean がリリースする有効なロールバックを調査します。 |
| `npm run check:links` |ロトスの門。米国は安全保障を緩和します。 |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` からの米国) |別名の推進/復帰のメカニズム。 |
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |拒否/エイリアス/TLS/レプリカのテレメトリアの集合体。 PagerDuty のアラートは証拠を参照するパネルです。 |

## Runbook - 誤ったアーティファクトのデスリーグ

### コンディショネス デ ディスパロ

- Fallan los はプレビュー/生産を調査します (`npm run probe:portal -- --expect-release=...`)。
- アラート Grafana と `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` はロールアウトを拒否します。
- QA マニュアルでプロキシの回転や落下を検出します。すぐに試してみてください。
  別名プロモーション。

### メディア間の争い

1. **Congelar despliegues:** マルカ エル パイプライン CI con `DEPLOY_FREEZE=1` (ワークフローの入力
   GitHub) は、サルガン マス アーティファクトのない Jenkins の仕事を一時停止します。
2. **キャプチャ アーティファクト:** `build/checksums.sha256` をダウンロード、
   `portal.manifest*.{json,to,bundle,sig}`、あなたはフォールドパラケを構築するために調査を行います
   ロールバック参照は正確な内容をダイジェストします。
3. **著名な関係者:** ストレージ SRE、ドキュメント/DevRel リーダー、公式保護者
   gobernanza パラ意識 (especialmente cuando `docs.sora` esta Impactado)。

### ロールバックの手順

1. 究極のコノシド ブエノ (LKG) を識別します。ロスガード生産のワークフロー
   バジョ`artifacts/devportal/<release>/sorafs/portal.manifest.to`。
2. ヘルパー デ envio に対するマニフェストのエイリアスを再設定します。

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

3. レジストラとロールバックの再開、およびインシデント チケットとロス ダイジェストの登録
   マニフェスト LKG とマニフェスト fallido を削除します。

### 検証1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (ver la guia de despliegue) 確認のためのマニフェストの再促進
   CAR アーカイブとの偶然の一致。
4. `npm run probe:tryit-proxy` は、プロキシの Try-It ステージング レジストリを設定します。

### 事件後

1. 原因となった問題を解決するためのパイプラインのリハビリテーション。
2. 「教訓」の内容 [`devportal/deploy-guide`](./deploy-guide)
   con nuevas notas、si aplica。
3. プルエバス スイートに含まれる欠陥 (プローブ、リンク チェッカーなど)。

## Runbook - レプリケーションの劣化

### コンディショネス デ ディスパロ

- アラート: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  10 分あたり 0.95 秒。
- `torii_sorafs_replication_backlog_total > 10` 10 分 (ver
  `pin-registry-ops.md`)。
- Gobernanza レポートは、非公開の別名を公開します。

### トリアージ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) パラメタのダッシュボードの検査
   プロバイダーのストレージやフリートのローカル環境でバックログを確認します。
2.Torii パラ警告 `sorafs_registry::submit_manifest` パラ判定シの Cruza ログ
   ラス提出エスタン・ファランド。
3. `sorafs_cli manifest status --manifest ...` 経由でレプリカの検索 (lista resultados)
   プロバイダーによるレプリケーション）。

### 軽減策

1. レプリカのマニフェストを再確認する (`--pin-min-replicas 7`)
   `scripts/sorafs-pin-release.sh` パラケエルスケジューラ配布カーガアンセット市長
   プロバイダー。事件記録の新しいダイジェストを記録します。
2. SI EL バックログは、UN プロバイダー Unico に送信され、EL 経由で一時的にデシャビリタロ
   レプリケーションのスケジューラ (`pin-registry-ops.md` のドキュメント) 新しい環境を実現
   マニフェスト forzando a los otros は、refrescar el alias を提供します。
3. クアンド・ラ・フレスクーラ・デル・エイリアス・マス・クリティカ・ク・ラ・パリダード・デ・レプリケーション、再ビンキュラ・エル
   エイリアスは、マニフェスト カリエンテ ヤ ステージド (`docs-preview`)、公開マニフェスト デ
   SRE のバックログを確実に解決します。

### 回復と治療

1. Monitorea `torii_sorafs_replication_sla_total{outcome="missed"}` パラセキュラークエリ
   コンテオセスタビリザ。
2. `sorafs_cli manifest status` の証拠を確認し、レプリカを取得します。
   新しい絶頂。
3. 状況に応じてバックログの複製を事後処理する
   (プロバイダーのエスカレード、チャンカーのチューニングなど)。

## ランブック - 遠隔分析の分析

### コンディショネス デ ディスパロ

- `npm run probe:portal` ダッシュボードでのイベントの終了
  `AnalyticsTracker` 時間 > 15 分。
- プライバシー レビューは、不正なイベントを検出します。
- `npm run probe:tryit-proxy` はパス `/probe/analytics` に落ちます。

###レスペスタ1. Verifica のビルド入力: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` リリース版 (`build/release.json`)。
2. 再取り出し `npm run probe:portal` コン `DOCS_ANALYTICS_ENDPOINT` をプンタンド
   コレクターは、ペイロードを確認するためのステージングおよびトラッカーのステージングを行います。
3. サイ ロス コレクター エスタン カイドス、セテア `DOCS_ANALYTICS_ENDPOINT=""` y 再構築
   パラケエルトラッカーハガショートサーキット。停止中のレジストラ・ラ・ベンタナ・デ・レジストラ
   事件の連続。
4. Valida que `scripts/check-links.mjs` siga フィンガープリンティング `checksums.sha256`
   (ラス・カイダス・デ・アナリティカ*いいえ*デベン・ブロック・ラ・バリダシオン・デル・サイトマップ)。
5. クアンド エル コレクター セキュペレ、コレ `npm run test:widgets` パラ エジェクター ロス
   再公開する前に、単体テストのヘルパーと分析を行います。

### 事件後

1. Actualiza [`devportal/observability`](./observability) の制限はありません
   収集家、博物館の必需品。
2. 暴力的な分析を実行する必要があることを明らかにする
   デ・ポリティカ。

## 回復力のトリメストラレスを訓練する

エジェクタ・アンボス・ドリル・デュランテ・エル **初級マルテス・デ・カダ・トリメストレ** (Ene/4月/7月/10月)
インフラストラクチャ市長の選挙の準備。グアルダ アーティファクト バホ
`artifacts/devportal/drills/<YYYYMMDD>/`。

|ドリル |パソス |証拠 |
| ----- | ----- | -------- |
|別名ロールバックを実行します。 1. 「Despliegue fallido」のロールバックを繰り返し、製造のマニフェストを実行します。<br/>2.問題を解決するために必要な情報を再収集します。<br/>3.レジストラ `portal.manifest.submit.summary.json` は、ドリルでのプローブのログを記録します。 | `rollback.submit.json`、サリダ デ プローブ、y リリース タグ デル アンサヨ。 |
|オーディトリア デ バリダシオン シンテティカ | 1. エジェクター `npm run probe:portal` および `npm run probe:tryit-proxy` の逆生産およびステージング。<br/>2.エジェクター `npm run check:links` とアーカイブ変数 `build/link-report.json`。<br/>3.補助的なスクリーンショット/パネル Grafana のエクスポートとプローブの終了を確認します。 |プローブのログ + `link-report.json` はマニフェストの指紋を参照します。 |

ドキュメント/DevRel マネージャーのトレーニングを強化し、SRE の改訂版を作成します。
ロードマップは、別名とロスのロールバックを証明するトリメストラル決定論を証明します
ポータル siguen saludables を調査します。

## PagerDuty とオンコールの調整- PagerDuty **ドキュメント ポータル パブリッシング** に関する最新のアラートの提供
  `dashboards/grafana/docs_portal.json`。ラスレグラス `DocsPortal/GatewayRefusals`、
  `DocsPortal/AliasCache`、y `DocsPortal/TLSExpiry` Docs/DevRel のプライマリページ
  con Storage SRE como secundario。
- Cuando のページ、`DOCS_RELEASE_TAG` を含む、パネル Grafana の付属スクリーンショット
  事件発生時の調査/リンクチェックの影響と影響
  最初の緩和。
- 軽減策 (ロールバックまたは再デプロイ)、再取り出し `npm run probe:portal`、
  `npm run check:links`、y スナップショットをキャプチャ Grafana フレスコ画のほとんどのメトリックス
  デ・ヌエボ・デントロ・デ・アンブラレス。 PagerDuty の事件に関する証拠の補助機関
  解決する前に。
- 不正な情報に関するアラート (TLS のバックログの期限切れ)、トリアージ
  拒否のプリメロ (デテナー出版)、ロールバックの手続きの完了、ルエゴ
  TLS/バックログとストレージ SRE ブリッジのリンピア アイテム。