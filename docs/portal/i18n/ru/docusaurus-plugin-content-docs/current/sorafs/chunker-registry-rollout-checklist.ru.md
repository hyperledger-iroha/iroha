---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
title: Чеклист развертывания реестра chunker SoraFS
Sidebar_label: Чанкер развертывания Чеклиста
описание: Пошаговый план внедрения обновленного реестра chunker.
---

:::note Канонический источник
Отражает `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Держите копии синхронизированными, пока набор документации Sphinx не будет выведен из эксплуатации.
:::

# Чеклист реестра внедрения SoraFS

Этот чек-лист фиксирует шаги, необходимые для продвижения нового профиля чанкера.
или допуск поставщика пакета от проверки до продакшена после ратификации
устав управления.

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, конверты приема поставщика услуг или
> Канонные комплекты крепежа (`fixtures/sorafs_chunker/*`).

## 1. Предварительная валидация

1. Перегенерируйте светильники и проверьте детерминизм:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Убедитесь, что хеши детерминированы в
   `docs/source/sorafs/reports/sf1_determinism.md` (или соответствующий отчет
   профиль) происходит с регенерированными документами.
3. Убедитесь, что `sorafs_manifest::chunker_registry` скомпилирован с
   `ensure_charter_compliance()` при запуске:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите досье предложения:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Запись протокола совета в `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Утверждение управления

1. Просмотрите отчет Tooling Working Group и обобщите предложения в
   Группа по инфраструктуре парламента Сора.
2. Зафиксируйте детали одобрения в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Опубликуйте конверт, подписанный парламентом, вместе с приспособлениями:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Проверьте, что конверт доступен через вспомогательное управление получением:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Поэтапное развертывание

Подробное прохождение см. в [промежуточный манифест манифеста] (./staging-manifest-playbook).

1. Развернуть Torii с включенным открытием `torii.sorafs` и включенным применением.
   допуск (`enforce_admission = true`).
2. Загрузите конверты одобренного поставщика в каталог промежуточного реестра,
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. Проверить, что провайдер размещает рекламу через обнаружение API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Прогоните манифест/план конечных точек с заголовками управления:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Убедитесь, что панели телеметрии (`torii_sorafs_*`) и правила оповещений
   отображают новый профиль без ошибок.

## 4. Развертывание производства

1. Повторите шаги по продакшену Torii-узлах.
2. Объявите окно активации (дата/время, льготный период, план отката) в файлах.
   операторов и SDK.
3. Смёрджите релизный PR с:
   - Обновленными приборами и конвертом
   - Документационными изменениями (ссылки на устав, отчет о детерминизме)
   - Обновление дорожной карты/статуса
4. Поставьте тег релиза и заархивируйте подписанные документы для проверки происхождения.

## 5. Аудит после внедрения

1. Удалить окончательные метрики (количество открытий, вероятность успеха выборки, ошибка).
   гистограммы) через 24 часа после развертывания.
2. Обновите `status.md` краткое резюме и ссылку на отчет о детерминизме.
3. Задайте последующие задачи (например, дополнительные рекомендации по написанию
   профиль) в `roadmap.md`.