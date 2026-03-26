---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: Эластичное выделение レーン (NX-7)
サイドバーラベル: レーン
説明: ブートストラップ - レーン Nexus と доказательств ロールアウト。
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_elastic_lane.md`. Держите обе копии синхронизированными, пока волна переводов не попадет в портал.
:::

# Набор инструментов для эластичного выделения レーン (NX-7)

> **ロードマップ:** NX-7 - レーンの説明  
> **Статус:** инструменты заверbolога - генерируют манифесты、фрагменты каталога、Norito ペイロード、スモーク テスト、
> ヘルパー ロード テスト バンドル、スロット レイテンシー + スロット レイテンシ、および чтобы прогоны нагрузки валидаторов
> можно было публиковать без кастомных скриптов.

Этот гайд проводит операторов через новый helper `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию манифестов レーン,レーン/データスペースとロールアウトの組み合わせ。 Цель - легко поднимать новые Nexus レーン (パブリック レーンとプライベートレーン) のレーン (パブリック レーンとプライベート) のレーン。 Пересчета геометрии каталога。

## 1. Предварительные требования

1. ガバナンス - エイリアス レーン、データスペース、フォールト トレランス (`f`) および決済。
2. 名前空間 (アカウント ID) と名前空間。
3. Доступ к репозиторию конфигураций узлов, чтобы добавить сгенерированные фрагменты.
4. Пути для рестра манифестов レーン (см. `nexus.registry.manifest_directory` и `cache_directory`)。
5. Контакты телеметрии/PagerDuty は、レーン、タスク、およびタスクを処理します。

## 2. Сгенерируйте артефакты レーン

ヘルパーの機能:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Ключевые флаги:

- `--lane-id` は、`nexus.lane_catalog` のインデックスを取得します。
- `--dataspace-alias` および `--dataspace-id/hash` は、データスペースと каталоге (ID レーン) を表します。
- `--validator` は、`--validators-file` を表示します。
- `--route-instruction` / `--route-account` が表示されます。
- `--metadata key=value` (`--telemetry-contact/channel/runbook`) は、Runbook、ダッシュボードなどの機能を備えています。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` フック ランタイム アップグレードが実行され、レーンが更新されます。
- `--encode-space-directory` は、`cargo xtask space-directory encode` を表します。 `--space-directory-out` を使用して、`.to` を使用してください。

Скрипт создает три артефакта в `--output-dir` (по умолчанию текущий каталог) и опционально четвертый приエンコーディング:1. `<slug>.manifest.json` - レーン、定足数、名前空間、フック ランタイム アップグレード。
2. `<slug>.catalog.toml` - TOML-фрагмент с `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` と、これに対応します。 Убедитесь、что `fault_tolerance` задан в записи データスペース、lane-relay комитет (`3f+1`)。
3. `<slug>.summary.json` - аудит-сводка с геометрией (スラッグ、сегменты、метаданные)、требуемыми загами ロールアウト、точной командой `cargo xtask space-directory encode` (`space_directory_encode.command`)。 JSON クラスのオンボーディング クラスを確認してください。
4. `<slug>.manifest.to` - `--encode-space-directory`; Torii `iroha app space-directory manifest publish` を確認してください。

Используйте `--dry-run`, чтобы посмотреть JSON/фрагменты без записи файлов, и `--force` для перезаписи существующих артефактов。

## 3. Примените изменения

1. JSON メソッドと `nexus.registry.manifest_directory` (キャッシュ ディレクトリ、レジストリ バンドル)。 Закоммитьте файл, если манифесты версионируются в конфигурационном репозитории.
2. Єрагмент каталога в `config/config.toml` (или в подходящий `config.d/*.toml`)。 Убедитесь, что `nexus.lane_count` не меньзе `lane_id + 1`, и обновите `nexus.routing_policy.rules`, которые должны указывать на новую lan.
3. スペース ディレクトリの概要 (`--encode-space-directory`) とスペース ディレクトリの概要 (`space_directory_encode.command`)。 Это создает `.manifest.to`, который ожидает Torii, и фиксирует доказательства для аудиторов; отправьте через `iroha app space-directory manifest publish`。
4. `irohad --sora --config path/to/config.toml --trace-config` とトレースのロールアウトを実行します。 Это подтверждает, что новая геометрия соответствует сгенерированным slug/Kura сегментам.
5. Перезапустите валидаторы、назначенные на lane、после развертывания изменений манифеста/каталога. JSON の概要を説明します。

## 4. バンドルの価格

オーバーレイ、オーバーレイ、ガバナンス、レーンの管理を行うことができます。 ждом хосте。ヘルパーは、レイアウト、ガバナンス カタログ オーバーレイ、`nexus.registry.cache_directory` および tarball アップデートを実行します。 офлайн-передачи:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

例:

1. `manifests/<slug>.manifest.json` - копируйте их в настроенный `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` と同じです。 Каждая запись `--module` становится подключаемым определением модуля, позволяя замену ガバナンス - модуля (NX-2) черезキャッシュ オーバーレイ `config.toml`。
3. `summary.json` - オーバーレイとオーバーレイを表示します。
4. `registry_bundle.tar.*` - SCP、S3 のバージョン。

Синхронизируйте весь каталог (или архив) на каждый валидатор, распакуйте на air-gaped хостах и скопируйте манифесты + キャッシュ オーバーレイは、Torii をサポートします。

## 5. 煙テストПосле перезапуска Torii запустите новый スモーク ヘルパー、чтобы проверить、что lan сообщает `manifest_ready=true`、метрикиレーンとゲージシールを備えています。レーン、которым нужны манифесты、обязаны иметь непустой `manifest_path`;ヘルパー теперь мгновенно падает при отсутствии пути, чтобы каждый деплой NX-7 включал доказательство подписанногоメッセージ:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`--insecure` は自己署名済みです。 Скрипт заверДается ненулевым кодом, если lan отсутствует, sealed, или метрики/телеметрия расходятся с ожидаемыми значениями。 Используйте `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` および `--max-headroom-events`, чтобы удерживать телеметрию по lan (высота) `--max-slot-p95` / `--max-slot-p99` (`--min-slot-samples`) の日付が表示されます。 NX-18 はヘルパーとして機能します。

エアギャップ パターン (CI) メソッド сохраненный ответ Torii вместо обращения к живому endpoint:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` のフィクスチャ、ブートストラップ ヘルパー、lint-ить の機能問題は解決しました。 CI гоняет тот же поток через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (別名: `make check-nexus-lanes`)、чтобы убедиться、что スモークヘルパー NX-7 остаетсяペイロードとダイジェスト/オーバーレイ バンドルを利用できます。