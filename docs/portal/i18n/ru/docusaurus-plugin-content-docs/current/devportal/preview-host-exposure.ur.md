---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA может быть использован в качестве пакета с проверкой контрольной суммы. جو ریویورز لوکل طور پر چلاتے ہیں۔ ریویور آن بورڈنگ (можно пригласить участника, которого вы хотите) для того, чтобы создать Runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- ریویور آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- В сборке `docs/portal/build/` используется контрольная сумма, установленная в (`build/checksums.sha256`).
- SoraFS پریویو اسناد (Torii URL, авторитет, закрытый ключ, эпоха) Конфигурация JSON используется для загрузки [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Имя хоста (`docs-preview.sora.link`, `docs.iroha.tech` и др.), а также DNS-запрос и возможность вызова по вызову. رابطے شامل ہوں۔

## مرحلہ 1 - Bundle بنائیں اور verify کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Проверьте манифест контрольной суммы, чтобы убедиться, что вы хотите проверить манифест контрольной суммы. سے ہر پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - SoraFS артефакты پیک کریں

اسٹیٹک سائٹ کو CAR/manifest جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` کی ڈیفالٹ ویلیو `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Для `portal.car`, `portal.car`, `portal.manifest.*`, дескриптора и манифеста контрольной суммы требуется, чтобы они были доступны. کریں۔

## مرحلہ 3 - پریویو псевдоним شائع کریں

Вы можете использовать контактный помощник для **بغیر** `--skip-submit` کے دوبارہ چلائیں۔ Конфигурация JSON и флаги CLI, например:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Используйте `portal.pin.report.json`, `portal.manifest.submit.summary.json` или `portal.submit.response.json`, чтобы пригласить пакет доказательств. چاہئیں۔

## Шаг 4 - Переключение DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Доступ к JSON и Ops для создания DNS-запросов и дайджеста манифеста для проверки Для отката можно использовать дескриптор, который можно использовать для `--previous-dns-plan path/to/previous.json`.

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

зонд, тег выпуска, заголовки CSP, метаданные подписи и т. д. Если вам нужен выходной сигнал Curl, вы можете использовать его. Использование пограничного кэша

## Пакет доказательств

Здесь можно найти артефакты, которые можно использовать для создания артефактов. Ответ:

| Артефакт | مقصد |
|----------|------|
| `build/checksums.sha256` | Используйте пакет CI-сборки или сборку CI |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | каноническая полезная нагрузка SoraFS + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Отправка манифеста или привязка псевдонима کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | Метаданные DNS (в виде файла), продвижение маршрута (`Sora-Route-Binding`), указатель `route_plan` (файл JSON + шаблоны заголовков), очистка кэша Чтобы выполнить откат, используйте Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Дескриптор и архив + контрольная сумма. |
| Выход `probe` | تصدیق کرتا ہے کہ لائیو ہوسٹ متوقع Release tag دکھا رہا ہے۔ |