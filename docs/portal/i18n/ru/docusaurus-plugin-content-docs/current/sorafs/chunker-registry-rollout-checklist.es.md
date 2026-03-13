---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
заголовок: Контрольный список развертывания реестра фрагментов SoraFS
Sidebar_label: Контрольный список развертывания чанкера
описание: План развертывания для актуализации реестра блоков.
---

:::примечание Фуэнте каноника
Рефлея `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Нам пришлось скопировать синхронизированные копии, чтобы удалить комплект документации Sphinx.
:::

# Контрольный список развертывания реестра SoraFS

Этот контрольный список содержит необходимые шаги для продвижения новой порции фрагмента
o пакет допусков поставщиков после пересмотра и производства после того, как
Хартия губернатора была ратифицирована.

> **Alcance:** Приложение ко всем последним выпускам, которые модифицируются
> `sorafs_manifest::chunker_registry`, о допуске поставщиков или лос
> комплекты светильников canónicos (`fixtures/sorafs_chunker/*`).

## 1. Предварительная проверка

1. Регенерация светильников и проверка детерминизма:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Подтвердите, что хеши детерминированы
   `docs/source/sorafs/reports/sf1_determinism.md` (отчет о соответствующих результатах)
   совпало с восстановленными артефактами.
3. Убедитесь, что `sorafs_manifest::chunker_registry` скомпилирован с
   `ensure_charter_compliance()` выбросил:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Актуализация досье собственности:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Ввод актов совета на `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Апробация правительства

1. Представление информации рабочей группы по инструментам и дайджеста материалов.
   Группа по инфраструктуре парламента Сора.
2. Зарегистрируйте подробную информацию об апробации в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Публикация о своей фирме парламентского объединения в ходе матчей:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Убедитесь, что море доступно с помощью помощника по выбору правительства:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Поэтапное внедрение

Проконсультируйтесь с [сборником манифеста в постановке] (./staging-manifest-playbook) для
запишите подробную информацию об этом пасеке.

1. Despliega Torii с открытием `torii.sorafs`, навык и применение
   доступ активирован (`enforce_admission = true`).
2. Получите информацию о допуске проверяющих в директорию регистрации.
   ссылка на промежуточную версию `torii.sorafs.discovery.admission.envelopes_dir`.
3. Проверка того, что реклама поставщика распространяется через API обнаружения:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Выведите конечные точки манифеста/плана с заголовками правительства:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Подтвердите, что панели телеметрии (`torii_sorafs_*`) и правила
   Оповещение сообщает о новых ошибках.

## 4. Внедрение в производство

1. Повторите этапы подготовки против узлов Torii производства.
2. Anuncia la ventana de activacion (fecha/hora, periodo de gracia, plan de откат)
   Лос-каналы операторов и SDK.
3. Слияние PR-релизов, содержащих:
   - Светильники и обновленные версии
   - Cambios de documentación (ссылки на карту, отчеты о детерминизме)
   - Обновление дорожной карты/статуса
4. Этика выпуска и архивирования фирменных артефактов для обработки.

## 5. Аудитория после внедрения1. Окончательный захват метрик (сообщения об открытии, вызов извлечения,
   гистограммы ошибок) 24 часа после развертывания.
2. Актуализируйте `status.md` с возобновлением кортежа и подключением отчета о детерминизме.
3. Registra cualquier tarea de seguimiento (стр. ej., большая инструкция по авторизации файлов)
   ru `roadmap.md`.