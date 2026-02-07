---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Вызов контрольной суммы для проверки контрольной суммы.

Он сказал, что хочет, чтобы он сказал, что он хочет, чтобы он сделал это. للتحقق قبل النشر. В 2007 году он выступил в роли президента США в CI, США. Контрольная сумма Дана была получена с помощью функции SoraFS مع. Проверьте Norito.

## الأهداف

- **Отключено:** Выбрано для `npm run build`, когда необходимо установить Приложение было создано `build/checksums.sha256`.
- **Отображение значения:** Проверка контрольной суммы и проверка контрольной суммы. Он сказал это.
- **Добавлено сообщение Norito:** احفظ واصفات المعاينة (بيانات commit Просмотрите контрольную сумму дайджеста, а также CID для SoraFS) или Norito JSON для проверки подлинности. تدقيق الإصدارات.
- **Воспроизведение:** В Стиле Скарлетт Брэнсон Уинстон-Лейк-Сити. محليا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); Для этого необходимо выполнить контрольную сумму + запросить контрольную сумму. Установленный модуль (`npm run serve`) для подключения к `docusaurus serve`. Вы можете проверить контрольную сумму (например, `npm run serve:verified`). مستعار صريح).

## المرحلة 1 — CI

1. Код `.github/workflows/docs-portal-preview.yml`:
   - `node docs/portal/scripts/write-checksums.mjs` بعد بناء Docusaurus (название программы).
   - ينفذ `cd build && sha256sum -c checksums.sha256` для получения дополнительной информации.
   - Выполнена сборка с кодом `artifacts/preview-site.tar.gz`, контрольной суммой Джонатана, `scripts/generate-preview-descriptor.mjs`, кодом `scripts/generate-preview-descriptor.mjs`. `scripts/sorafs-package-preview.sh` в формате JSON (например, `docs/examples/sorafs_preview_publish.json`) создается в формате JSON. Установите SoraFS.
   - Встроенное хранилище данных (`docs-portal-preview`, `docs-portal-preview-metadata`), SoraFS (`docs-portal-preview-sorafs`) установлен на автомобиле в автомобиле, расположенном в автомобиле.
2. Выполните команду CI для проверки контрольной суммы в ответ на запрос (проверка контрольной суммы). Используйте сценарий GitHub для `docs-portal-preview.yml`).
3. Установите флажок `docs/portal/README.md` (CI) и установите флажок для проверки.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` был создан в 2017 году в рамках программы по борьбе с наркотиками. Код: `sha256sum`. `npm run serve` (также можно установить `npm run serve:verified`) `docusaurus serve` может быть отключен от сети. В тексте:

1. Установите флажок SHA (`sha256sum` или `shasum -a 256`) для `build/checksums.sha256`.
2. يقارن اختيارياdigest/اسم ملف واصف المعاينة `checksums_manifest`, وعند توفرهdigest/اسم ملف أرشيف المعاينة.
3. Джон Бёрнсон, Сэнсэй Уинстон и Джон Тэтчер, в Нью-Йорке. Он был убит в 2007 году.

Ответ на вопрос (بعد استخراج آثار CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Его персонаж - CI, он был назначен президентом США Кейланом Дэйвом. أرفقوا آثارا بتذكرة إصدار.

## Ошибка 2 — نشر SoraFS1. Сообщение от Сэнсэя Сэнсэя:
   - Выполните промежуточный этап SoraFS для `sorafs_cli car pack` и `manifest submit`.
   - Сводный дайджест данных по CID для SoraFS.
   - Загрузите `{ commit, branch, checksum_manifest, cid }` и Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Он был арестован в судебном порядке по уголовному делу в судебном порядке.
3. Установите блок `sorafs_cli` в режиме пробного прогона в режиме пробного прогона. Поговорите с ним в ресторане.

## المرحلة 3 — الحوكمة والتدقيق

1. Установите Norito (`PreviewDescriptorV1`) для подключения `docs/portal/schemas/`.
2. Установите флажок DOCS-SORA:
   - Установите `sorafs_cli manifest verify` для CID.
   - Дайджест контрольной суммы и CID для PR-отдела.
3. Выполните настройку контрольной суммы для проверки контрольной суммы. الإصدار.

## المخرجات والمسؤوليات

| المعلم | المالك | الهدف | ملاحظات |
|--------|--------|-------|---------|
| Получение контрольной суммы для CI | بنية تحتية لوثائق | 1 | Он был отправлен в Нью-Йорк. |
| Дата выпуска SoraFS | Новости / Новости | Новости 2 | В ходе промежуточного этапа используется Norito. |
| تكامل الحوكمة | قائد Docs/DevRel / فريق عمل الحوكمة | Новости 3 | Он был убит в 2007 году в 1980-х годах. |

## أسئلة مفتوحة

- В исполнении SoraFS в рамках проекта "Детские игры" (постановка в исполнении режиссёра). مخصص)؟
- В разделе "Обзор" (Ed25519 + ML-DSA) можно получить дополнительную информацию.
- Для оркестратора CI (`orchestrator_tuning.json`) используется `sorafs_cli` для `sorafs_cli`. قابلية إعادة إنتاج البيانات؟

Он был установлен на `docs/portal/docs/reference/publishing-checklist.md` и был установлен в соответствии с требованиями.