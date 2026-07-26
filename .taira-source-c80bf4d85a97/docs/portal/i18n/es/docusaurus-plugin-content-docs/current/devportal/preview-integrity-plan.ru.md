---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# План предпросмотра с контролем checksum

Este plan describe cómo funciona el robot, no hay modo de hacerlo, estos son algunos de los artefactos que predisponen el portal. проверить перед публикацией. Asegúrese de garantizar que los registros registrados no sean válidos, de que la suma de comprobación del manifiesto no sea necesaria y del programa previo. Utilice el SoraFS con metadanos Norito.

## Цели

- **Terminadores:** observe, que `npm run build` da su resultado y su fuente `build/checksums.sha256`.
- **Proverennye предпросмотры:** требовать, чтобы каждый артефакт предпросмотра включал манифест checksum and запрещать публикацию. при провале проверки.
- **Метаданные, публикуемые через Norito:** сохранять дескрипторы предпросмотра (метаданные комита, digest checksum, CID SoraFS) como Norito JSON, estos instrumentos mejoran las respuestas de audio.
- **Instrumentos para los operadores:** Previamente se pueden descargar scripts locales y se pueden descargar archivos locales (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); скрипт теперь оборачивает поток проверки checksum + descriptor целиком. El controlador de comando estándar (`npm run serve`) se activa automáticamente antes de `docusaurus serve`, чтобы Los valores locales configuran la suma de comprobación del control (por ejemplo `npm run serve:verified` como un alias).

## Fase 1 — Control en CI1. Обновить `.github/workflows/docs-portal-preview.yml`, чтобы:
   - Introduzca `node docs/portal/scripts/write-checksums.mjs` después de Docusaurus (muy localmente).
   - Utilice `cd build && sha256sum -c checksums.sha256` y valide el trabajo según sus necesidades.
   - Instalar el directorio build en `artifacts/preview-site.tar.gz`, copiar la suma de comprobación del manifiesto, eliminar `scripts/generate-preview-descriptor.mjs` y activar `scripts/sorafs-package-preview.sh` s Configuración JSON (como `docs/examples/sorafs_preview_publish.json`), cada flujo de trabajo personalizado y metadano y determinado por la banda SoraFS.
   - Загружать статический сайт, артефакты метаданных (`docs-portal-preview`, `docs-portal-preview-metadata`) и SoraFS-bandl (`docs-portal-preview-sorafs`), чтобы манифест, сводка CAR and план могли быть проверены без повторной сборки.
2. Agregue un comentario a CI-beйджем, reseñe el resultado de la suma de verificación de la solicitud de extracción (revise el comentario de GitHub Script en `docs-portal-preview.yml`).
3. Complete el flujo de trabajo en `docs/portal/README.md` (parte CI) y consulte los datos de las publicaciones.

## Скрипт проверки

`docs/portal/scripts/preview_verify.sh` проверяет скачанные артефакты предпросмотра без ручных вызовов `sha256sum`. Utilice `npm run serve` (o un alias `npm run serve:verified`), cómo descargar el script y `docusaurus serve` para el inicio de sesión распространении локальных снимков. Логика проверки:1. Utilice el instrumento SHA (`sha256sum` o `shasum -a 256`) junto con `build/checksums.sha256`.
2. При необходимости сравнивает digest/имя файла descriptor предпросмотра `checksums_manifest` y, если указан, digest/имя файла архива предпросмотра.
3. Завершается с ненулевым кодом при любом несоответствии, чтобы рецензенты могли блокировать подмененные предпросмотры.

Primera implementación (después de la instalación de artefactos CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Los dispositivos CI y las configuraciones correspondientes utilizan un script de código de barras, descargan paquetes previos al programa o ejecutan artefactos. к релизному тикету.

## Faza 2 — Publicación SoraFS

1. Расширить flujo de trabajo previo al proceso de ejecución, которая:
   - Загружает собранный сайт в staging-шлюз SoraFS с помощью `sorafs_cli car pack` и `manifest submit`.
   - Захватывает возвращенный digest манифеста and CID SoraFS.
   - Serie `{ commit, branch, checksum_manifest, cid }` en Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Utilice el descriptor de pantalla de los artefactos y abra el CID en los comentarios de la solicitud de extracción.
3. Realice pruebas integradas, use el botón `sorafs_cli` en el modo de ejecución en seco y seleccione la configuración adecuada. совместимость схемы метаданных.

## Faza 3 — Actualización y auditoría1. Publique Norito-схему (`PreviewDescriptorV1`), una descripción de la estructura del escritorio, en `docs/portal/schemas/`.
2. Consulte la lista de publicaciones DOCS-SORA, требуя:
   - Introduzca `sorafs_cli manifest verify` para proteger el CID.
   - Fije la suma de comprobación del resumen del resumen y el CID en las descripciones de configuración de PR.
3. Puede automatizar la actualización del descriptor de pantalla con la suma de comprobación del manifiesto en el momento de la configuración.

## Результаты и владельцы

| Etapa | Владелец(ы) | Celo | Примечания |
|------|------------|------|------------|
| Контроль checksum в CI внедрен | Documentos de infraestructura | Неделя 1 | Добавляет gate отказа и загрузку артефактов. |
| Publicaciones previas a la transmisión en SoraFS | Infraestructura Docs / Almacenamiento de comandos | Неделя 2 | El trío está disponible para la puesta en escena de algunos y nuevos programas Norito. |
| Integración superior | Лид Docs/DevRel / WG по управлению | Неделя 3 | Publica esto y actualiza listas y hojas de ruta breves. |

## Открытые вопросы

- ¿Qué nombre tiene SoraFS?
- ¿Hay dos archivos (Ed25519 + ML-DSA) en el descriptor previo al anuncio público?
- Si el CI de flujo de trabajo elimina la configuración del orquestador (`orchestrator_tuning.json`) desde el puerto `sorafs_cli`, podrá configurar el CI. ¿manifestantes?Realice una revisión en `docs/portal/docs/reference/publishing-checklist.md` y elimine este plan después de una instalación no deseada.