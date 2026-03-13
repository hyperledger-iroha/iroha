---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
title: Развертывание реестра чанка SoraFS.
Sidebar_label: Развертывание Chunker
описание: обновления реестра chunker کے لیے قدم بہ قدم развертывание پلان۔
---

:::примечание
:::

# SoraFS Развертывание реестра.

یہ چیک لسٹ نئے chunker Profile یا входной пакет поставщика کو обзор سے Production
تک продвигать کرنے کے لیے درکار مراحل کو захватывать کرتی ہے جب хартию управления
ратифицировать ہو چکا ہو۔

> **Объем:** تمام выпускает پر لاگو ہے جو
> `sorafs_manifest::chunker_registry`, конверты приема провайдера, канонический
> Комплекты креплений (`fixtures/sorafs_chunker/*`)

## 1. Предполетная проверка

1. Фикстуры генерируют детерминизм и проверяют результат:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (отчет о профиле یا متعلقہ) میں
   детерминизм хеширует восстановленные артефакты سے match کریں۔
3. Установите флажок `sorafs_manifest::chunker_registry`,
   `ensure_charter_compliance()` Для компиляции данных:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Пакет предложений:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Запись в протоколе совета `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Утверждение управления

1. Отчет рабочей группы по инструментам и дайджест предложений.
   Панель инфраструктуры парламента Сора میں پیش کریں۔
2. Сведения об утверждении
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. Подписанный парламентом конверт с документами для публикации:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Помощник по управлению выборкой Доступен конверт, который можно использовать:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Поэтапное развертывание

Шаги и пошаговое руководство [сборник промежуточных манифестов] (./staging-manifest-playbook) دیکھیں۔

1. Torii или обнаружение `torii.sorafs` включило принудительное допуск на
   (`enforce_admission = true`) Можно развернуть.
2. конверты для приема утвержденных поставщиков и каталог промежуточного реестра, а также push-уведомления
   جسے `torii.sorafs.discovery.admission.envelopes_dir` обратитесь к کرتا ہے۔
3. Обнаружение API для рекламы поставщика и распространения для проверки:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Заголовки управления и упражнение по конечным точкам манифеста/плана:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Панели телеметрии (`torii_sorafs_*`) и правила оповещений, а также профиль профиля.
   رپورٹنگ بغیر ошибки کے подтвердить کریں۔

## 4. Развертывание производства

1. Этапы подготовки к производству узлов Torii и повторение.
2. окно активации (дата/время, льготный период, план отката) Оператор اور SDK
   каналы پر анонсируют کریں۔
3. Выпустите PR-слияние в ближайшее время:
   - обновлены светильники в конверте
   - изменения в документации (ссылки на устав, отчет о детерминизме)
   - обновление дорожной карты/статуса
4. тег выпуска کریں اور подписанные артефакты کو происхождение и архив کریں۔

## 5. Аудит после внедрения

1. Развертывание в течение 24 часов с окончательными метриками (количество обнаружений, вероятность успешного получения, ошибка
   гистограммы) захватывают کریں۔
2. `status.md` Сводка сводного отчета о детерминизме и ссылка на обновление.
3. Последующие задачи (руководство по созданию профиля).