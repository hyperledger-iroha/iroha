---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
title: Контрольный список развертывания реестра блоков данных SoraFS
Sidebar_label: Контрольный список развертывания чанкера
описание: План развертывания проходит для настройки регистрации фрагментов.
---

:::примечание Fonte canonica
Отразить `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Мантенья представился как копиас синхронизадас.
:::

# Контрольный список развертывания в реестре SoraFS

Этот контрольный список включает в себя все необходимые действия для продвижения новой загрузки
или пакет допусков к досмотру для производства депо, который или чартер
де губернатора для ратификации.

> **Эскопо:** Применить все, что касается выпусков, которые модифицируются.
> `sorafs_manifest::chunker_registry`, конверты для приема поставщиков, или пачки
> светильники Canonicos (`fixtures/sorafs_chunker/*`).

## 1. Валидакао перед полетом

1. Регенерация приборов и проверка детерминированности:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Подтвердите, что хэши детерминированы.
   `docs/source/sorafs/reports/sf1_determinism.md` (или отношение Perfil
   актуально) batem com os artefatos regenerados.
3. Гарантия, что `sorafs_manifest::chunker_registry` будет скомпилирован с помощью com.
   `ensure_charter_compliance()` выполнить:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Подготовьте предварительное досье:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Ввод данных по совету с `docs/source/sorafs/council_minutes_*.md`
   - Отношения детерминизма

## 2. Утверждение правительства

1. Представление информации о рабочей группе по инструментам и дайджест предложения.
   Группа по инфраструктуре парламента Сора.
2. Зарегистрируйте данные подтверждения.
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Публика или конверт в парламентский союз с расписанием матчей:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Убедитесь, что конверт доступен с помощью помощника управления:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Постановка внедрения

Обратитесь к [сборнику манифеста по созданию] (./staging-manifest-playbook) для вас.
пассо а пассо подробно.

1. Имплантация Torii с открытием `torii.sorafs` хабилитадо и обеспечение допуска
   лигадо (`enforce_admission = true`).
2. Отправьте поставщикам входные конверты, одобренные для регистрации.
   ссылка на промежуточную версию `torii.sorafs.discovery.admission.envelopes_dir`.
3. Проверьте, распространяется ли реклама поставщика через API обнаружения:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Проверьте конечные точки манифеста/плана с заголовками управления:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Подтвердите, что панели телеметрии (`torii_sorafs_*`) и регистрация предупреждений
   Reportam o Novo Perfil с ошибками.

## 4. Внедрение в производство

1. Повторите промежуточные этапы производства узлов Torii.
2. Объявление об активации (данные/время, льготный период, план отката) нет
   каналы управления и SDK.
3. Объедините или PR конкурирующего релиза:
   - Готовые светильники и конверты
   - Mudancas na documentacao (ссылки на устав, отношения к детерминизму)
   - Обновление дорожной карты/статуса
4. Откройте выпуск и архивируйте убитые артефакты для происхождения.

## 5. Размещение постов в аудитории1. Сбор конечных показателей (количество открытий, таксоны успешного извлечения, гистограммы).
   Ошибка) 24 часа после выпуска.
2. Настройте `status.md` с кратким резюме и ссылкой для связи детерминизма.
3. Регистрация тарифов сопровождения (например, дополнительная ориентация для авторских работ)
   de perfis) em `roadmap.md`.