---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سسیڈنٹ رن بکس اور رول بیک ڈرلز

## 大事

**DOCS-9** のプレイブックとリハーサルのテスト
پورٹل آپریٹرز ڈلیوری فیلئرز سے بغیر اندازے کے ریکور کر سکیں۔ یہ نوٹ تین ہائی سگنل
問題 — 展開、レプリケーションの低下、分析の停止 — 問題
四半期ごとのドリル テスト エイリアス ロールバック 合成検証
エンドツーエンドの کام کرتے ہیں۔

### 大事なこと

- [`devportal/deploy-guide`](./deploy-guide) — パッケージ化、署名、エイリアス プロモーション ワークフロー
- [`devportal/observability`](./observability) — タグをリリース、分析、プローブ、分析、分析、分析、分析、分析、調査
- `docs/source/sorafs_node_client_protocol.md`
  [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — レジストリ テレメトリとエスカレーションのしきい値
- `docs/portal/scripts/sorafs-pin-release.sh` アイコン `npm run probe:*` ヘルパー
  جو چیک لسٹس میں ریفرنس ہیں۔

### テレメトリ ツール

|信号 / ツール |すごい |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) |レプリケーションの停止、SLA 違反の検出、 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |バックログの深さ、完了までの待ち時間、トリアージ、定量化、 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |ゲートウェイ側の障害 دکھاتا ہے جو اکثر خراب 導入 کے بعد آتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |合成プローブはゲートをリリースします ロールバックは検証します|
| `npm run check:links` |リンク切れのゲート。緩和策 بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` ذریعے) |エイリアスの昇格/復帰メカニズム|
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |拒否/エイリアス/TLS/レプリケーション テレメトリの集計PagerDuty アラート パネル、証拠、参照、参照|

## ランブック - 開発アーティファクト

### شروع ہونے کی شرائط

- プレビュー/実稼働プローブが失敗します (`npm run probe:portal -- --expect-release=...`)。
- `torii_sorafs_gateway_refusals_total` یا での Grafana アラート
  `torii_sorafs_manifest_submit_total{status="error"}` ロールアウト
- マニュアル QA エイリアス プロモーション 壊れたルート 試してみる プロキシの失敗 فوراً بعد

### فوری روک تھام

1. **デプロイメントのフリーズ:** CI パイプライン `DEPLOY_FREEZE=1` マーク (GitHub ワークフロー入力)
   Jenkins のジョブが一時停止され、アーティファクトが発生しました。
2. **アーティファクト キャプチャ:** ビルドの失敗 `build/checksums.sha256`、
   `portal.manifest*.{json,to,bundle,sig}` プローブ出力のロールバック
   ダイジェストとリファレンスを参照
3. **利害関係者:** ストレージ SRE、ドキュメント/DevRel リード、ガバナンス担当責任者
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### بیک طریقہ کار

1. last-known-good (LKG) マニフェスト制作ワークフロー
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. 出荷ヘルパーのエイリアス、マニフェスト、バインド、およびバインド:

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

3. ロールバックの概要、インシデント チケット、LKG、失敗したマニフェスト ダイジェスト、およびその結果。

### 年

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` と `sorafs_cli proof verify ...`
   (デプロイ ガイド دیکھیں) 再昇格されたマニフェスト アーカイブ CAR کے ساتھ match کرتا ہے۔
4. `npm run probe:tryit-proxy` テスト Try-It ステージング プロキシ بحالی یقینی ہو۔### واقعے کے بعد

1. 根本原因の解決と展開パイプラインの解決
2. [`devportal/deploy-guide`](./deploy-guide) 「教訓」エントリ کو نئے ポイント سے بھر دیں، اگر ہوں۔
3. テスト スイート (プローブ、リンク チェッカー、欠陥) の失敗

## ランブック - عرض المزيد

### شروع ہونے کی شرائط

- 結果: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 منٹ تک۔
- `torii_sorafs_replication_backlog_total > 10` 10 منٹ تک (`pin-registry-ops.md` دیکھیں)۔
- ガバナンス ریلیز کے بعد alias کی دستیابی سست ہونے کی رپورٹ کرے۔

### بتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) ダッシュボード دیکھیں تاکہ معلوم ہو سکے کہ バックログ
   ストレージ クラス、プロバイダー フリート、ストレージ クラス、プロバイダー フリート、ストレージ クラス、プロバイダー フリート
2. Torii ログ `sorafs_registry::submit_manifest` 警告 چیک کریں تاکہ معلوم ہو سکے کہ 送信が失敗しました ہو رہی ہیں یا और देखें
3. `sorafs_cli manifest status --manifest ...` ذریعے レプリカ健康サンプル کریں (プロバイダーごとの結果 دکھاتا ہے)۔

### और देखें

1. `scripts/sorafs-pin-release.sh` レプリカ数 (`--pin-min-replicas 7`) マニフェスト マニフェスト マニフェスト
   スケジューラーの負荷とプロバイダーの負荷事件ダイジェストログ میں ریکارڈ کریں۔
2. バックログ、プロバイダ、レプリケーション スケジューラ、無効化、無効化
   (`pin-registry-ops.md` ドキュメント化) マニフェストの送信 プロバイダーのエイリアス更新
3. エイリアスの鮮度、レプリケーション パリティ、クリティカル、エイリアス、ステージングされたマニフェスト (`docs-preview`) のリバインド
   SRE バックログをクリアする フォローアップ マニフェストを公開する

### और देखें

1. `torii_sorafs_replication_sla_total{outcome="missed"}` モニター カウントプラトー ہو۔
2. `sorafs_cli manifest status` 出力 証拠 キャプチャ キャプチャ レプリカ 準拠 6
3. レプリケーション バックログの事後分析 (プロバイダーのスケーリング、チャンカーのチューニング、) (プロバイダーのスケーリング、チャンカーの調整)

## ランブック - اینالیٹکس یا ٹیلیمیٹری آؤٹیج

### شروع ہونے کی شرائط

- `npm run probe:portal` ダッシュボード `AnalyticsTracker` イベント 15 分以上 取り込み時間
- プライバシー レビューがイベントを削除しました。
- `npm run probe:tryit-proxy` `/probe/analytics` パスが失敗します

### جوابی اقدامات

1. ビルド時の入力を検証します: `DOCS_ANALYTICS_ENDPOINT` `DOCS_ANALYTICS_SAMPLE_RATE`
   リリース アーティファクトの失敗 (`build/release.json`)
2. `DOCS_ANALYTICS_ENDPOINT` ステージング コレクターのポイント `npm run probe:portal` トラッカー ペイロードが放出するポイント
3. コレクターのダウン `DOCS_ANALYTICS_ENDPOINT=""` セットの再構築、トラッカーの短絡停止ウィンドウ インシデント タイムライン میں ریکارڈ کریں۔
4. `scripts/check-links.mjs` 指紋 `checksums.sha256` 指紋を検証します。
   (分析停止、サイトマップ検証 *ブロック* ٩رنا چاہیے)۔
5. コレクターが回復する `npm run test:widgets` 分析ヘルパー単体テストを実行する 再公開する

### واقعے کے بعد1. [`devportal/observability`](./observability) コレクターの制限事項 サンプリング要件
2. 分析データ ポリシー、ドロップ、編集、ガバナンス通知、

## سہ ماہی استقامت کی مشقیں

訓練 ** 四半期 پہلے منگل** (1 月/4 月/7 月/10 月)
インフラストラクチャの変更 فوراً بعد۔アーティファクト
`artifacts/devportal/drills/<YYYYMMDD>/` کے تحت محفوظ کریں۔

| और देखें重要 | और देखें
| ----- | ----- | -------- |
|エイリアスのロールバック1. プロダクション マニフェストの「失敗したデプロイメント」のロールバック<br/>2.プローブは、プロダクションに合格し、再バインドされます。<br/>3. `portal.manifest.submit.summary.json` プローブ ログとドリルの訓練| `rollback.submit.json`、プローブ出力、リハーサル、リリース タグ|
| صنوعی توثیق کا آڈٹ | 1. 制作、ステージング、`npm run probe:portal` 、`npm run probe:tryit-proxy` 、<br/>2. `npm run check:links` アーカイブ `build/link-report.json` アーカイブ<br/>3. Grafana パネルのスクリーンショット/エクスポートの添付 プローブの成功の確認|プローブ ログ + `link-report.json` マニフェスト フィンガープリント حوالہ دیتا ہے۔ |

見逃した演習 ドキュメント/DevRel マネージャー SRE ガバナンス レビュー エスカレート ロードマップ ロードマップ ドキュメント/DevRel マネージャー SRE ガバナンス レビュー
エイリアス ロールバック ポータル プローブ セキュリティ 決定論的 四半期証拠 セキュリティ

## PagerDuty のオンコール調整

- PagerDuty サービス **Docs Portal Publishing** `dashboards/grafana/docs_portal.json` سے پیدا ہونے والے アラート کی مالک ہے۔
  `DocsPortal/GatewayRefusals`、`DocsPortal/AliasCache`、`DocsPortal/TLSExpiry` Docs/DevRel プライマリ ページ
  ストレージ SRE セカンダリ ہوتا ہے۔
- ページ `DOCS_RELEASE_TAG` セキュリティ Grafana パネルのスクリーンショットが添付されています 緩和策 セキュリティ対策
  プローブ/リンクチェックの出力、インシデントメモ、リンク、リンク
- 緩和 (ロールバック、再デプロイ) `npm run probe:portal`、`npm run check:links`、Grafana
  スナップショットのキャプチャ、メトリクス、しきい値、データのキャプチャPagerDuty 事件の証拠を添付する
  解決する 解決する
- アラート、火災、TLS 有効期限、バックログ）、拒否、トリアージ
  (パブリッシング) ロールバック手順 ストレージ SRE ブリッジ TLS/バックログ ロールバック手順