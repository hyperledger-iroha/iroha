---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Путеводитель по экспозиции предварительного просмотра

Маршрут DOCS-SORA содержит информацию о том, как просмотреть общедоступный пакет мемов, проверив контрольную сумму, которую тестируют локальные отражатели. Используйте этот Runbook после регистрации участников (и билета одобрения приглашений) для получения бета-версии линии.

## Предварительное условие

- Неясная регистрация одобренных и зарегистрированных читателей в предварительном просмотре трекера.
- Последняя сборка порта представляет собой `docs/portal/build/` и проверку контрольной суммы (`build/checksums.sha256`).
- Предварительный просмотр идентификаторов SoraFS (URL Torii, autorite, cle privee, epoch soumis) хранится в переменных среды или конфигурации JSON по телефону [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Билет на смену DNS за пределами вашего дома (`docs-preview.sora.link`, `docs.iroha.tech` и т. д.), а также контакты по вызову.

## Этап 1 — Создание и проверка пакета

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Сценарий проверки отказывается от продолжения манифеста контрольной суммы или его изменения, так как он охраняет артефакт предварительного просмотра.

## Этап 2 — Упаковщик артефактов SoraFS

Преобразуйте статический сайт в пару CAR/детерминированный манифест. `ARTIFACT_DIR` соответствует значению по умолчанию `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, дескриптор и манифест контрольной суммы или билет неопределенного предварительного просмотра.

## Etape 3 - Предварительный просмотр Publisher l'alias

Relancez le helper de pin **sans** `--skip-submit` lorsque vous etes pret a разоблачитель любви. В конфигурации JSON для флагов CLI указано следующее:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Команда `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, которая будет сопровождать пакет доказательств приглашений.

## Этап 4 — Генерация плана соединения DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Разделите результирующий JSON с Ops, чтобы получить базовую ссылку DNS в точном дайджесте манифеста. Прецедент описания Lorsqu'un - это повторное использование источника отката, добавление `--previous-dns-plan path/to/previous.json`.

## Этап 5 — Развертывание зонта

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Зонд подтверждает тег выпуска службы, заголовки CSP и метадоны подписи. Relancez la Commande Depuis Deux Region (или joignez une Sortie Curl) для того, чтобы аудиторы видели, что край кэша est chaud.

## Пакет доказательств

Включите артефакты в билет неопределенного предварительного просмотра и ссылки в приглашении по электронной почте:| Артефакт | Объектив |
|----------|----------|
| `build/checksums.sha256` | Докажите, что пакет соответствует сборке CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Полезная нагрузка SoraFS canonique + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du манифест + le привязка псевдонима ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Метадонники DNS (билет, окно, контакты), возобновление продвижения маршрута (`Sora-Route-Binding`), указатель `route_plan` (план JSON + шаблоны заголовка), информация об очистке кеша и инструкции по откату для операций. |
| `artifacts/sorafs/preview-descriptor.json` | Описатель, который содержит архив + контрольную сумму. |
| Вылазка `probe` | Подтвердите, что вы хотите объявить о выпуске приглашения. |