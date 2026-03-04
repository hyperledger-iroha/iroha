---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf6cb06692c224f25a9034badd5bc43927e3e7d893851fedf09beb51ddc6857b
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plan de previsualizacion con checksum

Este plan detalla el trabajo restante necesario para que cada artefacto de previsualizacion del portal sea verificable antes de la publicacion. El objetivo es garantizar que los revisores descarguen exactamente la instantanea construida en CI, que el manifiesto de checksum sea inmutable y que la previsualizacion sea descubrible a traves de SoraFS con metadatos Norito.

## Objetivos

- **Compilaciones deterministicas:** Asegurar que `npm run build` produzca salida reproducible y siempre emita `build/checksums.sha256`.
- **Previsualizaciones verificadas:** Exigir que cada artefacto de previsualizacion incluya un manifiesto de checksum y rechazar la publicacion cuando falle la verificacion.
- **Metadatos publicados con Norito:** Persistir los descriptores de previsualizacion (metadatos de commit, digest de checksum, CID de SoraFS) como JSON de Norito para que las herramientas de gobernanza puedan auditar los lanzamientos.
- **Herramientas para operadores:** Proveer un script de verificacion de un solo paso que los consumidores puedan ejecutar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); el script ahora envuelve el flujo de validacion de checksum + descriptor de punta a punta. El comando estandar de previsualizacion (`npm run serve`) ahora invoca este helper automaticamente antes de `docusaurus serve` para que las instantaneas locales permanezcan protegidas por checksum (con `npm run serve:verified` mantenido como alias explicito).

## Fase 1 - Aplicacion en CI

1. Actualizar `.github/workflows/docs-portal-preview.yml` para:
   - Ejecutar `node docs/portal/scripts/write-checksums.mjs` despues del build de Docusaurus (ya se invoca localmente).
   - Ejecutar `cd build && sha256sum -c checksums.sha256` y fallar el job si hay discrepancias.
   - Empaquetar el directorio build como `artifacts/preview-site.tar.gz`, copiar el manifiesto de checksum, ejecutar `scripts/generate-preview-descriptor.mjs` y ejecutar `scripts/sorafs-package-preview.sh` con una configuracion JSON (ver `docs/examples/sorafs_preview_publish.json`) para que el workflow emita tanto metadatos como un bundle SoraFS deterministico.
   - Subir el sitio estatico, los artefactos de metadatos (`docs-portal-preview`, `docs-portal-preview-metadata`) y el bundle SoraFS (`docs-portal-preview-sorafs`) para que el manifiesto, el resumen CAR y el plan puedan inspeccionarse sin volver a ejecutar el build.
2. Agregar un comentario con insignia de CI que resuma el resultado de la verificacion de checksum en los pull requests (implementado via el paso de comentario GitHub Script de `docs-portal-preview.yml`).
3. Documentar el workflow en `docs/portal/README.md` (seccion CI) y enlazar a los pasos de verificacion en la checklist de publicacion.

## Script de verificacion

`docs/portal/scripts/preview_verify.sh` valida los artefactos de previsualizacion descargados sin requerir invocaciones manuales de `sha256sum`. Usa `npm run serve` (o el alias explicito `npm run serve:verified`) para ejecutar el script y lanzar `docusaurus serve` en un solo paso cuando compartas instantaneas locales. La logica de verificacion:

1. Ejecuta la herramienta SHA adecuada (`sha256sum` o `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compara el digest/nombre de archivo del descriptor de previsualizacion `checksums_manifest` y, cuando se proporciona, el digest/nombre de archivo del archivo de previsualizacion.
3. Sale con codigo distinto de cero cuando se detecta cualquier discrepancia para que los revisores puedan bloquear previsualizaciones manipuladas.

Ejemplo de uso (despues de extraer los artefactos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Los ingenieros de CI y de release deben ejecutar el script siempre que descarguen un bundle de previsualizacion o adjunten artefactos a un ticket de release.

## Fase 2 - Publicacion en SoraFS

1. Extender el workflow de previsualizacion con un job que:
   - Suba el sitio construido al gateway de staging de SoraFS usando `sorafs_cli car pack` y `manifest submit`.
   - Capture el digest del manifiesto devuelto y el CID de SoraFS.
   - Serialice `{ commit, branch, checksum_manifest, cid }` en JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Almacenar el descriptor junto al artefacto de build y exponer el CID en el comentario del pull request.
3. Agregar pruebas de integracion que ejerzan `sorafs_cli` en modo dry-run para garantizar que cambios futuros mantengan la coherencia del esquema de metadatos.

## Fase 3 - Gobernanza y auditoria

1. Publicar un esquema Norito (`PreviewDescriptorV1`) que describa la estructura del descriptor en `docs/portal/schemas/`.
2. Actualizar la checklist de publicacion DOCS-SORA para requerir:
   - Ejecutar `sorafs_cli manifest verify` contra el CID cargado.
   - Registrar el digest del manifiesto de checksum y el CID en la descripcion del PR de release.
3. Conectar la automatizacion de gobernanza para cruzar el descriptor con el manifiesto de checksum durante las votaciones de release.

## Entregables y responsables

| Hito | Responsable(s) | Objetivo | Notas |
|------|----------------|----------|-------|
| Aplicacion de checksum en CI completada | Infraestructura de Docs | Semana 1 | Agrega un gate de fallo y cargas de artefactos. |
| Publicacion de previsualizaciones en SoraFS | Infraestructura de Docs / Equipo de Storage | Semana 2 | Requiere acceso a credenciales de staging y actualizaciones del esquema Norito. |
| Integracion de gobernanza | Lider de Docs/DevRel / WG de Gobernanza | Semana 3 | Publica el esquema y actualiza checklists y entradas del roadmap. |

## Preguntas abiertas

- Que entorno de SoraFS debe alojar los artefactos de previsualizacion (staging vs. carril de previsualizacion dedicado)?
- Necesitamos firmas duales (Ed25519 + ML-DSA) en el descriptor de previsualizacion antes de la publicacion?
- Debe el workflow de CI fijar la configuracion del orquestador (`orchestrator_tuning.json`) al ejecutar `sorafs_cli` para mantener los manifiestos reproducibles?

Registra las decisiones en `docs/portal/docs/reference/publishing-checklist.md` y actualiza este plan cuando se resuelvan las dudas.
