---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о soak начисления емкости SF-2c

日付: 2026-03-21

## Область

Этот отчет фиксирует детерминированные тесты soak начисления емкости SoraFS и выплат,
SF-2c と互換性があります。

- **30 日間のマルチプロバイダー ソーク:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic`×
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  ハーネスはプロバイダー、30 日以内に決済されます。
  проверяет、что итоги 元帳 совпадают с независимо вычисленной эталонной
  ジャンク。 Blake3 ダイジェスト (`capacity_soak_digest=...`)、CI をダウンロード
  スナップショットを作成します。
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл)。攻撃、クールダウン、スラッシュ
  担保と元帳 остаются детерминированными。

## Выполнение

浸す方法:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Тесты завербуке и не требуют Тесты завербуке и не требуют
備品。

## Наблюдаемость

Torii スナップショットとプロバイダーの料金台帳、サービス
ダッシュボード、ゲート、ペナルティストライク:

- 残り: `GET /v1/sorafs/capacity/state` と `credit_ledger[*]`、
  台帳を保存したり、浸したりできます。 См.
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` 最低価格
  ストライキ、担保、担保、など
  ベースライン ソークが живыми окружениями で発生します。

## Дальнейбие заги

- Запланировать еженедельные ゲート-прогоны в CI для воспроизведения を浸す теста (煙層)。
- Grafana をスクレープ Torii でテレメトリを確認します。