---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
заголовок: Контрольный список развертывания блока регистрации SoraFS
Sidebar_label: Блокировщик развертывания контрольного списка
описание: План развертывания для всех случаев в день регистрации.
---

:::note Источник канонический
Reflète `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Gardez les deux копирует синхронизированные копии только для полного восстановления набора наследия Сфинкса.
:::

# Контрольный список развертывания регистрации SoraFS

Этот контрольный список с подробным описанием необходимых этапов для продвижения нового профиля
кусок или входной билет для ревю после производства
ратификация Хартии управления.

> **Portée:** Приложение для всех релизов, которые можно модифицировать
> `sorafs_manifest::chunker_registry`, конверты для приема четырех женщин или
> Канонические комплекты светильников (`fixtures/sorafs_chunker/*`).

## 1. Предварительная проверка

1. Обновите светильники и проверьте детерминизм:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Подтвердите, что хеши детерминированы.
   `docs/source/sorafs/reports/sf1_determinism.md` (или связь профиля
   уместно) корреспондент aux artefacts régénérés.
3. Убедитесь, что `sorafs_manifest::chunker_registry` скомпилирован с учетом
   `ensure_charter_compliance()` на английском языке:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Сообщите в течение дня о досье предложения:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Входная минута совета `docs/source/sorafs/council_minutes_*.md`
   - Раппорт детерминизма

## 2. Управление валидацией

1. Презентация взаимопонимания с рабочей группой по инструментам и обзор предложений.
   au Сора Парламентская группа по инфраструктуре.
2. Зарегистрируйте детали одобрения в рамках
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Публикация конверта с подписью парламента на бумаге:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Убедитесь, что конверт доступен через помощника по выборке управления:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Поэтапное внедрение

Référez-vous au [промежуточный манифест playbook] (./staging-manifest-playbook) для вас
Подробная процедура.

1. Разверните Torii с активным обнаружением `torii.sorafs` и приложением.
   Активный вход (`enforce_admission = true`).
2. Poussez les конверты для приема в репертуар
   ссылка на промежуточную регистрацию по `torii.sorafs.discovery.admission.envelopes_dir`.
3. Проверьте, что рекламные объявления провайдера распространяются через обнаружение API:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Выполните манифест/план конечных точек с заголовками управления:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Подтвердите, что панели телеметрии (`torii_sorafs_*`) и правила
   d'alerte reportent le nouveau profil sans erreurs.

## 4. Внедрение производства

1. Повторите этапы подготовки к производству Torii.
2. Объявить об открытии активации (дата/время, период отсрочки, план отката)
   для операторов каналов и SDK.
3. Объедините PR-контента выпуска:
   - Светильники и конверты на день
   - Изменения документации (ссылки на устав, соглашение о детерминизме)
   - Обновить дорожную карту/статус
4. Выпустите и архивируйте артефакты, подписанные для происхождения.

## 5. Аудит после внедрения1. Capturez les métriques Finales (полные открытия, все успешные выборки,
   гистограммы ошибок) через 24 часа после развертывания.
2. Подождите `status.md` с кратким резюме и залогом взаимопонимания по детерминизму.
3. Отправьте свои уроки (например, руководство по созданию профилей) в
   `roadmap.md`.