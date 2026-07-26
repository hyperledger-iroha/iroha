---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plan de previsualización con suma de comprobación

Este plan detalla el trabajo restante necesario para que cada artefacto de previsualización del portal sea verificable antes de la publicación. El objetivo es garantizar que los revisores descarguen exactamente la instantánea construida en CI, que el manifiesto de checksum sea inmutable y que la previsualización sea descubrible a través de SoraFS con metadatos Norito.

## Objetivos- **Compilaciones determinísticas:** Asegurar que `npm run build` produce salida reproducible y siempre emite `build/checksums.sha256`.
- **Previsualizaciones verificadas:** Exigir que cada artefacto de previsualización incluya un manifiesto de checksum y rechace la publicación cuando falle la verificación.
- **Metadatos publicados con Norito:** Persistir los descriptores de previsualización (metadatos de commit, digest de checksum, CID de SoraFS) como JSON de Norito para que las herramientas de gobernanza puedan auditar los lanzamientos.
- **Herramientas para operadores:** Proveer un script de verificación de un solo paso que los consumidores puedan ejecutar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); el script ahora envuelve el flujo de validación de checksum + descriptor de punta a punta. El comando estándar de previsualización (`npm run serve`) ahora invoca este ayudante automáticamente antes de `docusaurus serve` para que las instantáneas locales permanezcan protegidas por checksum (con `npm run serve:verified` mantenido como alias explícito).

## Fase 1 - Aplicación en CI1. Actualizar `.github/workflows/docs-portal-preview.yml` para:
   - Ejecutar `node docs/portal/scripts/write-checksums.mjs` después del build de Docusaurus (ya se invoca localmente).
   - Ejecutar `cd build && sha256sum -c checksums.sha256` y fallar el trabajo si hay discrepancias.
   - Empaquetar el directorio build como `artifacts/preview-site.tar.gz`, copiar el manifiesto de checksum, ejecutar `scripts/generate-preview-descriptor.mjs` y ejecutar `scripts/sorafs-package-preview.sh` con una configuración JSON (ver `docs/examples/sorafs_preview_publish.json`) para que el flujo de trabajo emita tanto metadatos como un paquete SoraFS determinístico.
   - Subir el sitio estático, los artefactos de metadatos (`docs-portal-preview`, `docs-portal-preview-metadata`) y el paquete SoraFS (`docs-portal-preview-sorafs`) para que el manifiesto, el resumen CAR y el plan puedan inspeccionarse sin volver a ejecutar el build.
2. Agregar un comentario con insignia de CI que resuma el resultado de la verificación de suma de verificación en los pull request (implementado vía el paso de comentario GitHub Script de `docs-portal-preview.yml`).
3. Documentar el flujo de trabajo en `docs/portal/README.md` (sección CI) y enlazar a los pasos de verificación en la checklist de publicación.

## Guión de verificación

`docs/portal/scripts/preview_verify.sh` valida los artefactos de previsualización descargados sin requerir invocaciones manuales de `sha256sum`. Usa `npm run serve` (o el alias explícito `npm run serve:verified`) para ejecutar el script y lanzar `docusaurus serve` en un solo paso cuando compartas instantáneas locales. La lógica de verificación:1. Ejecuta la herramienta SHA adecuada (`sha256sum` o `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compare el digest/nombre de archivo del descriptor de previsualización `checksums_manifest` y, cuando se proporciona, el digest/nombre de archivo del archivo de previsualización.
3. Sale con código distinto de cero cuando se detecte cualquier discrepancia para que los revisores puedan bloquear previsualizaciones manipuladas.

Ejemplo de uso (después de extraer los artefactos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Los ingenieros de CI y de liberación deben ejecutar el script siempre que descarguen un paquete de previsualización o artefactos adjuntos a un ticket de liberación.

## Fase 2 - Publicación en SoraFS

1. Extender el flujo de trabajo de previsualización con un trabajo que:
   - Suba el sitio construido al gateway de staging de SoraFS usando `sorafs_cli car pack` y `manifest submit`.
   - Capture el digest del manifiesto devuelto y el CID de SoraFS.
   - Serialice `{ commit, branch, checksum_manifest, cid }` y JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Almacenar el descriptor junto al artefacto de construcción y exponer el CID en el comentario del pull request.
3. Agregar pruebas de integración que ejerzan `sorafs_cli` en modo dry-run para garantizar que los cambios futuros mantengan la coherencia del esquema de metadatos.

## Fase 3 - Gobernanza y auditorios1. Publicar un esquema Norito (`PreviewDescriptorV1`) que describe la estructura del descriptor en `docs/portal/schemas/`.
2. Actualizar la lista de verificación de publicación DOCS-SORA para requerir:
   - Ejecutar `sorafs_cli manifest verify` contra el CID cargado.
   - Registrar el digest del manifiesto de checksum y el CID en la descripción del PR de liberación.
3. Conectar la automatización de gobernanza para cruzar el descriptor con el manifiesto de checksum durante las votaciones de liberación.

## Entregables y responsables

| hito | Responsable(s) | Objetivo | Notas |
|------|----------------|----------|-------|
| Aplicación de suma de comprobación en CI completada | Infraestructura de Documentos | Semana 1 | Agrega una puerta de fallo y cargas de artefactos. |
| Publicación de previsualizaciones en SoraFS | Infraestructura de Documentos / Equipo de Almacenamiento | Semana 2 | Requiere acceso a credenciales de staging y actualizaciones del esquema Norito. |
| Integración de gobernanza | Líder de Docs/DevRel / WG de Gobernanza | Semana 3 | Publica el esquema y actualiza checklists y entradas del roadmap. |

## Preguntas abiertas- ¿Qué entorno de SoraFS debe alojar los artefactos de previsualización (staging vs. carril de previsualización dedicado)?
- Necesitamos firmas duales (Ed25519 + ML-DSA) en el descriptor de previsualización antes de la publicación?
- ¿Debe el flujo de trabajo de CI fijar la configuración del orquestador (`orchestrator_tuning.json`) al ejecutar `sorafs_cli` para mantener los manifiestos reproducibles?

Registra las decisiones en `docs/portal/docs/reference/publishing-checklist.md` y actualiza este plan cuando se resuelvan las dudas.