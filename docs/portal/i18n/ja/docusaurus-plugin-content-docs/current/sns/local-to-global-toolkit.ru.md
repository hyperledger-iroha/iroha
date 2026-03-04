---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ローカル -> グローバル -> ローカル -> グローバル

Эта страница отражает `docs/source/sns/local_to_global_toolkit.md` из モノリポジトリ。 CLI ヘルパーと Runbook、**ADDR-5c** の機能。

## Обзор

- `scripts/address_local_toolkit.sh` CLI `iroha`、次のとおりです。
  - `audit.json` -- структурированный вывод `iroha tools address audit --format json`。
  - `normalized.txt` -- преобразованные IH58 (предпочтительно) / 圧縮 (`sora`) (второй выбор) リテラル каждого ローカル ドメイン セレクター。
- ダッシュボードの取り込みを完了する (`dashboards/grafana/address_ingest.json`)
  および Alertmanager (`dashboards/alerts/address_ingest_rules.yml`)、ローカル 8 のカットオーバー /
  ローカル-12。 Local-8 と Local-12 および алертами の接続
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、`AddressInvalidRatioSlo`
  マニフェスト。
- Сверяйтесь с [アドレス表示ガイドライン](address-display-guidelines.md) и
  [アドレス マニフェスト ランブック](../../../source/runbooks/address_manifest_ops.md) UX およびインシデント対応機能。

## Использование

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

説明:

- `--format compressed (`sora`)` は `sora...` と IH58 を接続します。
- `--no-append-domain` для вывода 裸のリテラル。
- `--audit-only` чтобы пропустить заг конвертации.
- `--allow-errors` は、CLI にアクセスできます。

Скрипт выводит пути артефактов в конце выполнения. Приложите оба файла к
変更管理チケットの Grafana スクリーンショット、ファイル
ローカル-8 日、ローカル-12 日、>=30 日。

## Интеграция CI

1. ジョブと出力を確認します。
2. マージ、`audit.json` とローカル セレクター (`domain.kind = local12`)。
   со значением по умолчанию `true` (меняйте на `false` только в dev/test при диагностике регрессий) и
   `iroha tools address normalize --fail-on-warning --only-local` と CI、чтобы попытки регрессии
   製作。

См. исходный документ для деталей、証拠 чеклистов およびリリースノート スニペット、который можно
カットオーバーが必要になります。