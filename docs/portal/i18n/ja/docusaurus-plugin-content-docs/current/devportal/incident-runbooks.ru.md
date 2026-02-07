---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ロールバック

## Назначение

Пункт дорожной карты **DOCS-9** требует прикладных playbook'ов и плана репетиций, чтобы
операторы портала могли восстанавливаться после сбоев доставки без догадок. Эта заметка
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки、доказывающие、что ロールバック エイリアス
端から端まですべてを確認します。

### Связанные материалы

- [`devportal/deploy-guide`](./deploy-guide) — ワークフロー、プロモーション エイリアス。
- [`devportal/observability`](./observability) — タグ、分析、プローブをリリースします。
- `docs/source/sorafs_node_client_protocol.md`
  и [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия реестра и пороги эскалации.
- `docs/portal/scripts/sorafs-pin-release.sh` およびヘルパー `npm run probe:*`
  そうです。

### Общая телеметрия инструменты

| Сигнал / Инструмент | Назначение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (一致/失敗/保留中) | SLA を使用してください。 |
| `torii_sorafs_replication_backlog_total`、`torii_sorafs_replication_completion_latency_epochs` |バックログとトリアージ。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |ゲートウェイを実行し、デプロイを実行します。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |プローブ、ゲート ロールバック、ロールバックをサポートします。 |
| `npm run check:links` |ゲート битых ссылок;緩和策が必要です。 |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh`) |エイリアスを昇格/元に戻す。 |
| `Docs Portal Publishing` Grafana ボード (`dashboards/grafana/docs_portal.json`) |拒否/エイリアス/TLS/レプリケーションを実行します。 PagerDuty は、最新の機能を備えています。 |

## Ранбук - Неудачный деплой или плохой артефакт

### Условия срабатывания

- プレビュー/プロダクション機能 (`npm run probe:portal -- --expect-release=...`)。
- Grafana アラート - `torii_sorafs_gateway_refusals_total` または
  `torii_sorafs_manifest_submit_total{status="error"}` ロールアウト。
- QA を使用して、プロキシを試してみてください。
  プロモーションの別名。

### Немедленное сдерживание

1. **バージョン:** CI パイプライン `DEPLOY_FREEZE=1` (入力 GitHub ワークフロー)
   Jenkins のジョブを確認してください。
2. **Зафиксировать артефакты:** скачать `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`、プローブ、ビルドの失敗、
   ダイジェストをロールバックします。
3. **Уведомить стейкхолдеров:** ストレージ SRE、リード Docs/DevRel、ガバナンス担当責任者
   (особенно если затронут `docs.sora`)。

### ロールバックをロールバックします

1. 前回正常起動時 (LKG) マニフェストを作成します。制作ワークフローの説明
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`。
2. 配送ヘルパーのエイリアス к этому マニフェスト Перепривязать :

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

3. 概要ロールバック、インシデント チケット、ダイジェスト LKG およびマニフェストを確認します。

### Валидация

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2.`npm run check:links`。
3. `sorafs_cli manifest verify-signature ...` および `sorafs_cli proof verify ...`
   (展開ガイド)、再昇格されたマニフェスト с архивным CAR。
4. `npm run probe:tryit-proxy` は、Try-It ステージング プロキシです。

### После инцидента1. パイプラインの根本的な原因。
2. 「教訓」 в [`devportal/deploy-guide`](./deploy-guide)
   новыми выводами、если есть。
3. 欠陥を修正 (プローブ、リンク チェッカー、および т.д.)。

## Ранбук - Деградация репликации

### Условия срабатывания

- 例: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95`から10分。
- `torii_sorafs_replication_backlog_total > 10` в течение 10 минут (см.
  `pin-registry-ops.md`)。
- ガバナンスは別名リリースで提供されます。

### トリアージ

1. ダッシュボード [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)、ダッシュボード
   バックログ、ストレージ クラス、およびフリートの数を確認します。
2. Перепроверить логи Torii на `sorafs_registry::submit_manifest`, чтобы определить,
   提出物。
3. Выборочно проверить здоровье реплик через `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам)。

### 緩和策

1. マニフェストの作成 (`--pin-min-replicas 7`) のマニフェスト
   `scripts/sorafs-pin-release.sh`、スケジューラの機能
   ジャンク。インシデントログとダイジェストを表示します。
2. バックログ、レプリケーション スケジューラ、バックログ、レプリケーション スケジューラ
   (описано в `pin-registry-ops.md`) и отправить новый マニフェスト、принуждающий остальных
   別名。
3. エイリアスをパリティに変更し、エイリアスを再バインドし、ウォーム マニフェストをステージングします。
   (`docs-preview`)、バックログ SRE のフォローアップ マニフェストを表示します。

### 復旧と閉鎖

1. Мониторить `torii_sorafs_replication_sla_total{outcome="missed"}` и убедиться, что
   счетчик стабилизировался。
2. Сохранить вывод `sorafs_cli manifest status` как 証拠、что каждая реплика снова в норме.
3. レプリケーション バックログの事後分析を行う
   (маслытабирование провайдеров、チューニング チャンカー、и т.д.)。

## Ранбук - Отключение аналитики или телеметрии

### Условия срабатывания

- `npm run probe:portal` は、ダッシュボードにアクセスできます。
  `AnalyticsTracker` 15 分までです。
- プライバシー レビュー фиксирует неожиданный рост がイベントを削除しました。
- `npm run probe:tryit-proxy` は、`/probe/analytics` です。

### 応答

1. ビルド時の入力を指定します: `DOCS_ANALYTICS_ENDPOINT` および
   `DOCS_ANALYTICS_SAMPLE_RATE` × リリース アーティファクト (`build/release.json`)。
2. Перезапустить `npm run probe:portal` с `DOCS_ANALYTICS_ENDPOINT`, направленным
   ステージング コレクター、ペイロードのトラッカー、ペイロードなどの機能があります。
3. コレクター、`DOCS_ANALYTICS_ENDPOINT=""` および再構築、
   トラッカーのショート。停電とインシデントのタイムラインを確認します。
4. 指紋 `scripts/check-links.mjs` 指紋 `checksums.sha256`
   (аналитические сбои *не* должны блокировать проверку サイトマップ)。
5. コレクター запустить `npm run test:widgets`, чтобы прогнать
   単体テスト分析ヘルパーが再公開されます。

### После инцидента1. Обновить [`devportal/observability`](./observability) с новыми ограничениями
   コレクターまたはサンプリング。
2. ガバナンスに関する通知、分析機能の提供
   ありがとうございます。

## Квартальные учения по устойчивости

Запускайте оба ドリル в **первый вторник каждого квартала** (1 月/4 月/7 月/10 月)
と言うのは、そのようなことを意味します。 Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`。

| Учение | Шаги | Доказательства |
| ----- | ----- | -------- |
|エイリアス ロールバック | 1. ロールバック「デプロイに失敗しました」を本番マニフェストで確認します。<br/>2.運用プローブを再バインドします。<br/>3. Сохранить `portal.manifest.submit.summary.json` とドリルのプローブ。 | `rollback.submit.json`、プローブとリリース タグを解除します。 |
| Аудит синтетической валидации | 1. `npm run probe:portal` および `npm run probe:tryit-proxy` はプロダクションおよびステージングです。<br/>2. `npm run check:links` または `build/link-report.json`。<br/>3. Grafana のスクリーンショット/エクスポート、プローブを使用できます。 |プローブ + `link-report.json` は、指紋マニフェストを作成します。 |

Docs/DevRel とレビュー ガバナンス SRE の演習を行います。
так как ロードマップ требует детерминированных квартальных доказательств、что エイリアス ロールバック
およびポータル プローブを利用できます。

## PagerDuty とオンコール対応

- PagerDuty **ドキュメント ポータル パブリッシング** の機能
  `dashboards/grafana/docs_portal.json`。 Правила `DocsPortal/GatewayRefusals`、
  `DocsPortal/AliasCache` および `DocsPortal/TLSExpiry` のプライマリ Docs/DevRel
  ストレージ SRE はセカンダリです。
- При пейджинге укажите `DOCS_RELEASE_TAG`, приложите スクリーンショット затронутых
  Grafana はプローブ/リンクチェックを実行します。
  緩和。
- 緩和策 (ロールバックまたは再デプロイ) による `npm run probe:portal`、
  `npm run check:links` と сохраните свежие Grafana スナップショット、показывающие возврат метрик
  ああ。 PagerDuty の証拠を確認してください。
- Если два алерта срабатывают одновременно (TLS の有効期限とバックログ)、сначала
  トリアージ拒否 (公開)、ロールバック、TLS/バックログ
  ストレージ SRE - ブリッジ。