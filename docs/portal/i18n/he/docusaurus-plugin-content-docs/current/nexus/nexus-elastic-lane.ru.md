---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-elastic-lane
כותרת: Эластичное выделение ליין (NX-7)
sidebar_label: Эластичное выделение ליין
תיאור: Bootstrap-процесс для создания манифестов lane Nexus, השקה מזדמנת ו- доказательств.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_elastic_lane.md`. Держите обе копии синхронизированными, пока волна переводов не попадет в портал.
:::

# Набор инструментов для эластичного выделения ליין (NX-7)

> **מפת הדרכים של Пункт:** NX-7 - инструменты эластичного выделения ליין  
> ** סטאטוסים:** אינסטרומנטים - מטענים, מטענים, מטעני Norito, בדיקות עשן,
> עזר ל-load-test bundle теперь связывает гейтинг по משבצת משבצת + манифесты доказательств, чтобы прогоны нагрузки валидаторов
> можно было публиковать без кастомных скриптов.

Этот гайд проводит операторов через новый helper `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию манифестратов lane, נתיב/מרחב נתונים והשקה. Цель - легко поднимать новые Nexus נתיבים (ציבורי או פרטי) ללא ручного редактирования нескольких файнгов ипч геометрии каталога.

## 1. Предварительные требования

1. Governance-одобрение для alias lane, dataspace, набора валидаторов, סובלנות תקלות (`f`) והסדר פולי.
2. Финальный список валидаторов (מזהי חשבון) ומרחבי שמות список защищенных.
3. Доступ к репозиторию конфигураций узлов, чтобы добавить сгенерированные фрагменты.
4. Пути для реестра манифестов ליין (см. `nexus.registry.manifest_directory` ו-`cache_directory`).
5. חיבורי טלפונים/PagerDuty ידיות לנתיב, чтобы алерты были подключены сразу после онлайна.

## 2. סרגל אמנותיים

Запустите helper из корня репозитория:

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

- `--lane-id` должен совпадать с index новой записи в `nexus.lane_catalog`.
- `--dataspace-alias` ו-`--dataspace-id/hash` מעבירים מרחב נתונים בקטגוריות (בנתיב זיהוי זיהוי).
- `--validator` можно повторять или читать из `--validators-file`.
- `--route-instruction` / `--route-account` выводят готовые к вставке правила маршрутизации.
- `--metadata key=value` (או `--telemetry-contact/channel/runbook`) ספר רונבוק של תקצירים, לוחות מחוונים פנויים לשירותים.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` משדרגים את שדרוג זמן ריצה של הוק במניפסט, מסייעים לנתיב אופנתי אופנתי.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Используйте вместе с `--space-directory-out`, если хотите положить кодированный `.to` в другое место.

סקריפט создает три артефакта в `--output-dir` (по умолчанию текущий каталог) и опционально четвично четвично:1. `<slug>.manifest.json` - מניפסט ליין עם קוורום валидаторов, מרחבי שמות ואופניים מתקדמים שדרוג זמן ריצה.
2. `<slug>.catalog.toml` - TOML-фрагмент с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` ו- запрошенными правилами маршрутизации. Убедитесь, что `fault_tolerance` задан в записи מרחב הנתונים, чтобы размерить commit lane-relay (`3f+1`).
3. `<slug>.summary.json` - аудит-сводка с геометрией (שבלול, сегменты, метаданные), требуемыми шагами шагами и точной `cargo xtask space-directory encode` (פאד `space_directory_encode.command`). Приложите этот JSON к тикету onboarding как доказательство.
4. `<slug>.manifest.to` - появляется при `--encode-space-directory`; готов для потока Torii `iroha app space-directory manifest publish`.

הצג את `--dry-run`, פנה ל-JSON/фрагменты ללא ספקים, ו-`--force` לשירותי פרסומת артефактов.

## 3. Примените изменения

1. צור מדריך JSON ב-`nexus.registry.manifest_directory` (ובספריית המטמון, eсли Registry зеркалит удаленные חבילות). Закоммитьте файл, если манифесты версионируются в конфигурационном репозитории.
2. Добавьте фрагмент каталога в `config/config.toml` (אולי в подходящий `config.d/*.toml`). Убедитесь, что `nexus.lane_count` не меньше `lane_id + 1`, и обновите `nexus.routing_policy.rules`, которые должнываука.
3. Закодируйте (если пропустили `--encode-space-directory`) опубликуйте манифест в Space Directory, используя команду из summary (`config.d/*.toml`). Это создает `.manifest.to`, который ожидает Torii, ו фиксирует доказательства для аудиторов; отправьте через `iroha app space-directory manifest publish`.
4. התקן את `irohad --sora --config path/to/config.toml --trace-config` ו- архивируйте вывод trace в тикете השקה. Это подтверждает, что новая геометрия соответствует сгенерированным slug/Kura сегментам.
5. פתח את המסלולים, מצא את הנתיב, פנה לשירותי הנדסה/קטאלוגה. תקציר תקציר JSON в тикете для будущих аудитов.

## 4. צור חבילה

Упакуйте сгенерированный манифест и שכבת-על, чтобы операторы могли распространять данные ניהול по נתיבים ללא правок конфиг. Helper упаковки копирует манифесты в канонический פריסה, создает опциональный שכבת-על קטלוג ממשל ל-`nexus.registry.cache_directory` и может для собрать офлайн-передачи:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Выходы:

1. `manifests/<slug>.manifest.json` - копируйте их в настроенный `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - кладите в `nexus.registry.cache_directory`. Каждая запись `--module` становится подключаемым определением модуля, позволяя замену governance-моледуля (Navn-2) שכבת-על вместо редактирования `config.toml`.
3. `summary.json` - שיטות עבודה, שכבת-על מתקדמת ואינסטרוקציות לאופניים.
4. Опционально `registry_bundle.tar.*` - готово для SCP, S3 или трекеров артефактов.

Синхронизируйте весь каталог (אולי архив) на каждый валидатор, распакуйте весь каталог (אולי архив) на каждый валидатор, распакуйте весь каталог (אולי архив) на каждый валидатор, распакуйте весь каталог (אולי архив) на каждый валидатор, распакуйте על חלון מרווח אוויר או סקיפטיר + скепиру пути реестра перед перезапуском Torii.

## 5. בדיקות עשן валидаторовПосле перезапуска Torii запустите новый עשן עוזר, чтобы проверить, что lane сообщает `manifest_ready=true`, мевтокиз ожидаемое количество נתיבים и מד אטום чист. נתיבים, которым нужны манифесты, обязаны иметь непустой `manifest_path`; helper теперь мгновенно падает при отсутствии пути, чтобы каждый деплой NX-7 включал доказательство подидпи:

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

Добавьте `--insecure` при тестировании חתום עצמי окружений. סקריפט завершается ненулевым кодом, если lane отсутствует, אטום, или метрики/телеметрия расходятсия с отсутствует. Используйте `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` ו-`--max-headroom-events`, чтобы удерживать телемептри блока/финальность/backlog/headroom) в допустимых рамках, и сочетайте их с `--max-slot-p95` / `--max-slot-p99` (ו I18NI целей NX-18 по длительности слота, не выходя из עוזר.

Для air-gapped проверок (או CI) можно проигрывать сохраненный ответ Torii вместо обращения к живом:

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

מתקנים Записанные в `fixtures/nexus/lanes/` отражают артефакты, создаваемые bootstrap helper, чтобы новые манифесты можоно битьно кастомных скриптов. CI гоняет тот же поток через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (כינוי: `make check-nexus-lanes`), чтобы убедиться, чтобы убедиться, чаXтося helper совместимым с опубликованным форматом מטען וצרור תקצירים/שכבות-על остаются воспроизводимыми.