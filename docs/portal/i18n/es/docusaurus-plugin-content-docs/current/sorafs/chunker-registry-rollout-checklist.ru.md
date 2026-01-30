---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-rollout-checklist
title: Чеклист rollout реестра chunker SoraFS
sidebar_label: Чеклист rollout chunker
description: Пошаговый план rollout для обновлений реестра chunker.
---

:::note Канонический источник
Отражает `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Держите обе копии синхронизированными, пока набор документации Sphinx не будет выведен из эксплуатации.
:::

# Чеклист rollout реестра SoraFS

Этот чеклист фиксирует шаги, необходимые для продвижения нового профиля chunker
или bundle provider admission от ревью до продакшена после ратификации
governance charter.

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, provider admission envelopes или
> канонические fixture bundles (`fixtures/sorafs_chunker/*`).

## 1. Предварительная валидация

1. Перегенерируйте fixtures и проверьте детерминизм:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Убедитесь, что hashes детерминизма в
   `docs/source/sorafs/reports/sf1_determinism.md` (или релевантном отчете
   профиля) совпадают с регенерированными артефактами.
3. Убедитесь, что `sorafs_manifest::chunker_registry` компилируется с
   `ensure_charter_compliance()` при запуске:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите dossier предложения:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Запись minutes совета в `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Governance sign-off

1. Представьте отчет Tooling Working Group и digest предложения в
   Sora Parliament Infrastructure Panel.
2. Зафиксируйте детали одобрения в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Опубликуйте envelope, подписанный парламентом, вместе с fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Проверьте, что envelope доступен через helper получения governance:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Staging rollout

Подробный walkthrough см. в [staging manifest playbook](./staging-manifest-playbook).

1. Разверните Torii с включенным discovery `torii.sorafs` и включенным enforcement
   admission (`enforce_admission = true`).
2. Загрузите approved provider admission envelopes в staging registry directory,
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. Проверьте, что provider adverts распространяются через discovery API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Прогоните endpoints manifest/plan с governance headers:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Убедитесь, что telemetry dashboards (`torii_sorafs_*`) и alert rules
   отображают новый профиль без ошибок.

## 4. Production rollout

1. Повторите шаги staging на продакшен Torii-узлах.
2. Объявите окно активации (дата/время, grace period, rollback plan) в каналы
   операторов и SDK.
3. Смёрджите релизный PR с:
   - Обновленными fixtures и envelope
   - Документационными изменениями (ссылки на charter, отчет о детерминизме)
   - Обновлением roadmap/status
4. Поставьте тег релиза и архивируйте подписанные артефакты для provenance.

## 5. Post-rollout аудит

1. Снимите финальные метрики (discovery counts, fetch success rate, error
   histograms) через 24 часа после rollout.
2. Обновите `status.md` кратким резюме и ссылкой на отчет детерминизма.
3. Заведите follow-up задачи (например, дополнительная guidance по authoring
   профилей) в `roadmap.md`.
