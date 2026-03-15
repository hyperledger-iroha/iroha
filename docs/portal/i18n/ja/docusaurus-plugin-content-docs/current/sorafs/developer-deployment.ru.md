---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
タイトル: Заметки по развертыванию SoraFS
サイドバーラベル: Заметки по развертыванию
説明: Чеклист для продвижения пайплайна SoraFS из CI в продаклен.
---

:::note Канонический источник
:::

# Заметки по развертыванию

Пайплайн упаковки SoraFS усиливает детерминизм, поэтому переход из CI в продаклен в основном требует операционных ガードレール。ゲートウェイとストレージ プロバイダーのロールアウトを確認できます。

## Предварительная проверка

- **Выравнивание реестра** — убедитесь、что профили chunker и、manifests ссылаются на одинаковый кортеж `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`)。
- **入場料** — プロバイダーの広告とエイリアス証明、`manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) を確認してください。
- **Runbook ピン レジストリ** — держите `docs/source/sorafs/runbooks/pin_registry_ops.md` は、сценариев восстановления (ротация エイリアス、сбои репликации)。

## Конфигурация окружения

- ゲートウェイは、エンドポイント プルーフ ストリーミング (`POST /v1/sorafs/proof/stream`)、CLI でのテストを実行します。
- `sorafs_alias_cache` の機能と `iroha_config` または CLI ヘルパー (`sorafs_cli manifest submit --alias-*`)。
- ストリーム トークン (Torii) のシークレット マネージャー。
- エクスポーター (`torii_sorafs_proof_stream_*`、`torii_sorafs_chunk_range_*`) と Prometheus/OTel をサポートします。

## Стратегия ロールアウト

1. **青/緑のマニフェスト**
   - `manifest submit --summary-out` がロールアウトされました。
   - Следите за `torii_sorafs_gateway_refusals_total`, чтобы рано ловить несовместимость возможностей.
2. **Проверка の証明**
   - Считайте сбои в `sorafs_cli proof stream` блокерами развертывания;層を調整する必要があります。
   - `proof verify` 煙テストのピン、自動車のテスト、ダイジェストどうも。
3. **ダッシュボードの表示**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` × Grafana です。
   - ピン レジストリ (`docs/source/sorafs/runbooks/pin_registry_ops.md`) およびチャンク範囲。
4. **マルチソース**
   - `docs/source/sorafs/runbooks/multi_source_rollout.md` ロールアウトとオーケストレーター、スコアボード/スコアボードの表示。

## Обработка инцидентов

- Следуйте путям эскалации в `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` 停止ゲートウェイとストリーム トークン。
  - `dispute_revocation_runbook.md`、これは最高です。
  - `sorafs_node_ops.md` 日、今日。
  - `multi_source_rollout.md` オーバーライド、ピアのブラックリストおよびロールアウト。
- GovernanceLog での証明と検証、PoR トラッカー API、ガバナンスの検証確認してください。

## Следующие заги

- Интегрируйте автоматизацию オーケストレーター (`sorafs_car::multi_fetch`)、マルチソース フェッチ オーケストレーター (SF-6b)。
- PDP/PoTR と SF-13/SF-14 の組み合わせ。 CLI とドキュメント、レベル、層、証明を確認できます。

Объединив эти заметки по развертыванию с Quickstart и CI рецептами, команды смогут перейти от локальных экспериментов кプロダクション グレードの SoraFS パイプラインは、最高のパフォーマンスを発揮します。