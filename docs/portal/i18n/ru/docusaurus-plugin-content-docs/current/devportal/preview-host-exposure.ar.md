---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تعريض مضيف المعاينة

Загрузите DOCS-SORA в файл, указанный в документе. Проверьте контрольную сумму и установите флажок. Написано в честь президента США Дональда Трампа (Нидерланды) Это было сделано для того, чтобы сделать это.

## المتطلبات المسبقة

- Он был отправлен в Нью-Йорк в Нью-Йорке.
- Выполняется проверка контрольной суммы `docs/portal/build/` и контрольной суммы (`build/checksums.sha256`).
- Выбрано SoraFS (название Torii, установленное приложение). الخاص, epoch المرسل) Создается в формате JSON в формате JSON. [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Создание DNS-запроса (`docs-preview.sora.link`, `docs.iroha.tech`, ... ) الاتصال المناوبة.

## الخطوة 1 - بناء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Для проверки контрольной суммы необходимо ввести контрольную сумму, а также проверить контрольную сумму, которую он выполняет. معاينة مدققا.

## الخطوة 2 - حزم آثار SoraFS

حوّل الموقع الثابت الى زوج CAR/manifest حتمي. `ARTIFACT_DIR` или `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Для `portal.car` и `portal.manifest.*` требуется проверка контрольной суммы.

## الخطوة 3 - псевдоним نشر المعاينة

Установите контактный номер **بدوون** `--skip-submit`. В формате JSON и интерфейсе CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Установите `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, а затем нажмите кнопку . الخاصة بالدعوات.

## Ошибка 4 – отключить DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Создайте JSON-файл для работы с DNS-файлом и дайджестом манифеста. Выполнен откат и выполнен откат `--previous-dns-plan path/to/previous.json`.

## 5 - فحص المضيف المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Он был отправлен в компанию CSP, а также в США. Для этого необходимо использовать (с помощью Curl) доступ к кэшу.

## حزمة الادلة

Он сказал, что в фильме "Триумф" и в "Бостоне" есть:

| الاثر | غرض |
|-------|-------|
| `build/checksums.sha256` | В центре внимания CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Создайте файл SoraFS + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Вы можете указать псевдоним وربط الـ. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS-сервер (открытый DNS-сервер), установленный DNS-сервер (`Sora-Route-Binding`), удаленный `route_plan` (заголовок JSON + заголовок) и очистка данных для отката и отката операции Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + контрольная сумма. |
| خرج `probe` | Он был убит в 2007 году. |