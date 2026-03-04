---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ロールバック

## いいえ

**DOCS-9** 評価、評価、評価、評価、評価、評価、レビュー、評価、口コミ、写真、地図など、グルメ
重要な問題は、次のとおりです。 تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر،
ロールバック ロールバック エイリアス
エンドツーエンド。

### 大事なこと

- [`devportal/deploy-guide`](./deploy-guide) — パッケージングと署名のエイリアス。
- [`devportal/observability`](./observability) — タグをリリースします。
- `docs/source/sorafs_node_client_protocol.md`
  و [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — テレメトリア。
- `docs/portal/scripts/sorafs-pin-release.sh` と `npm run probe:*` ヘルパー
  ありがとうございます。

### القياس عن بعد والادوات المشتركة

|信号 / ツール |ああ |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) | SLA を取得します。 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |バックログのトリアージ。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |ゲートウェイを展開してください。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |ゲートはロールバックをリリースします。 |
| `npm run check:links` |ログインしてください。緩和策です。 |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh`) | آلية ترقية/اعادة の別名。 |
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |テレメトリアの拒否/エイリアス/TLS/レプリケーション。 PagerDuty を使用してください。 |

## ランブック - アーティファクトの作成

### और देखें

- プレビュー/プロダクション用のプローブ (`npm run probe:portal -- --expect-release=...`)。
- 回答 Grafana 評価 `torii_sorafs_gateway_refusals_total` 回答
  `torii_sorafs_manifest_submit_total{status="error"}` ロールアウト。
- QA يدوي يلاحظ مسارات مكسورة او فشل proxy Try it مباشرة بعد ترقية エイリアス。

### ああ、

1. **評価:** `DEPLOY_FREEZE=1` パイプライン CI (入力ワークフロー GitHub)
   Jenkins のジョブは、アーティファクトを処理します。
2. **アーティファクト:** `build/checksums.sha256`、
   `portal.manifest*.{json,to,bundle,sig}`、ビルド ロールバックをプローブします
   الى は الدقيقة をダイジェストします。
3. **ストレージ SRE とリード、Docs/DevRel、** ストレージ SRE、ドキュメント/DevRel、およびストレージ SRE
   (خصوصا عند تاثير `docs.sora`)。

### ロールバック

1. マニフェストファイル (LKG)。ワークフローの説明
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`。
2. エイリアス マニフェスト ヘルパー エイリアス:

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

3. ロールバックとダイジェストとマニフェスト LKG とマニフェスト。

### ああ

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` と `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان マニフェスト المعاد ترقيته ما زال يطابق CAR المؤرشف。
4. `npm run probe:tryit-proxy` は、プロキシ Try-It のステージング テストです。

### ما بعد الحادثة

1. パイプラインの開発。
2. تحديث قسم 「教訓」 في [`devportal/deploy-guide`](./deploy-guide)
   すごいです。
3. 欠陥 (プローブ、リンク チェッカー、分析)。

## ランブック - ランブック

### और देखें- 評価: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 ` 。
- `torii_sorafs_replication_backlog_total > 10` لمدة 10 دقائق (انظر)
  `pin-registry-ops.md`)。
- 別名リリース。

### トリアージ

1. ダッシュボード [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) のダッシュボード
   バックログのプロバイダー。
2. 評価 Torii 評価 `sorafs_registry::submit_manifest` 評価
   提出物です。
3. `sorafs_cli manifest status --manifest ...` (プロバイダー)。

### 緩和策

1. マニフェスト (`--pin-min-replicas 7`) のマニフェスト
   `scripts/sorafs-pin-release.sh` スケジューラ プロバイダー。
   ダイジェスト版をご覧ください。
2. バックログ、プロバイダー、レプリケーション スケジューラー
   (موثق في `pin-registry-ops.md`) マニフェスト プロバイダーのエイリアス。
3. エイリアス マニフェスト エイリアス マニフェスト エイリアス パリティ パリティ エイリアス マニフェスト マニフェスト エイリアス
   ステージングされた (`docs-preview`) マニフェスト、SRE バックログ。

### और देखें

1. 番号 `torii_sorafs_replication_sla_total{outcome="missed"}` 番号。
2. `sorafs_cli manifest status` のレプリカ。
3. 事後処理のバックログと処理
   (プロバイダーとチャンカー)。

## ランブック - ランブック - ランブック - ランブック

### और देखें

- `npm run probe:portal` ダッシュボードを表示する
  `AnalyticsTracker` 15 月。
- プライバシー レビューを確認してください。
- `npm run probe:tryit-proxy` は、`/probe/analytics` です。

### ああ

1. 重要な入力: `DOCS_ANALYTICS_ENDPOINT`
   `DOCS_ANALYTICS_SAMPLE_RATE` アーティファクト (`build/release.json`)。
2. 回答 `npm run probe:portal` 回答 `DOCS_ANALYTICS_ENDPOINT` 回答
   コレクターは、ステージングとペイロードのトラッカーを管理します。
3. コレクターズ متوقفة، اضبط `DOCS_ANALYTICS_ENDPOINT=""` واعمل 再構築
   トラッカーのショートサーキットタイムラインを確認してください。
4. 指紋 `scripts/check-links.mjs` 指紋 `checksums.sha256`
   (انقطاعات التحليلات يجب *الا* تمنع التحقق من サイトマップ)。
5. コレクター、`npm run test:widgets` 単体テスト、分析ヘルパー
   そうです。

### ما بعد الحادثة

1. تحديث [`devportal/observability`](./observability) باي قيود جديدة للcollector او
   サンプリング。
2. テストを実行してください。

## 토렌트 토렌트 다운로드 마그넷 마그넷 링크

شغل كلا التمرينين خلال **اول ثلاثاء من كل ربع** (1月/4月/7月/10月)
お金を払ってください。素晴らしいアーティファクト
`artifacts/devportal/drills/<YYYYMMDD>/`。|認証済み | और देखें意味 |
| ----- | ----- | -------- |
|ロールバック エイリアス | 1. ロールバック、「展開の失敗」、マニフェスト、<br/>2.調査。<br/>3. `portal.manifest.submit.summary.json` は調査を行っています。 | `rollback.submit.json`、プローブのリリース タグが表示されます。 |
|ニュース | ニュース1. `npm run probe:portal` と `npm run probe:tryit-proxy` のステージング。<br/>2. `npm run check:links` と `build/link-report.json`。<br/>3.スクリーンショット/エクスポート Grafana プローブ。 |プローブ + `link-report.json` 指紋マニフェスト。 |

Docs/DevRel のドキュメントと SRE のドキュメントを参照してください。
セキュリティ ロールバック、エイリアス、プローブのテストを実行します。

## PagerDuty とオンコール

- PagerDuty **ドキュメント ポータル パブリッシング** を使用してください。
  `dashboards/grafana/docs_portal.json`。 `DocsPortal/GatewayRefusals`、
  `DocsPortal/AliasCache`、`DocsPortal/TLSExpiry` のページングと Docs/DevRel
  ストレージ SRE です。
- スクリーンショット `DOCS_RELEASE_TAG` スクリーンショット Grafana スクリーンショット
  プローブ/リンクチェックと緩和策。
- 緩和策 (ロールバックと再デプロイ)、`npm run probe:portal`、
  `npm run check:links`、スナップショット、Grafana の写真
  ああ、そうです。 PagerDuty を使用してください。
- 応答 (TLS 有効期限切れ、バックログ) 応答拒否、応答
  (パブリッシング) ロールバック、TLS/バックログ、ストレージ SRE、ブリッジ。