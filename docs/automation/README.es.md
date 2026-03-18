---
lang: es
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Bases de automatización de documentación

Este directorio captura las superficies de automatización a las que se refieren
los elementos del roadmap como AND5/AND6 (Android Developer Experience + Release
Readiness) y DA-1 (automatización del modelo de amenazas de Disponibilidad de
Datos) cuando solicitan evidencia documental auditable. Mantener en el repositorio
las referencias de comandos y los artefactos esperados deja disponibles los
prerrequisitos para revisiones de cumplimiento incluso cuando los pipelines de CI
o los paneles estén fuera de línea.

## Estructura del directorio

| Ruta | Propósito |
|------|-----------|
| `docs/automation/android/` | Baselines de automatización para documentación y localización de Android (AND5), incluidos los registros de sincronización de stubs i18n, resúmenes de paridad y la evidencia de publicación del SDK requerida antes del visto bueno de AND6. |
| `docs/automation/da/` | Salidas de automatización del modelo de amenazas de Disponibilidad de Datos referenciadas por `cargo xtask da-threat-model-report` y la actualización nocturna de documentación. |

Cada subdirectorio documenta los comandos que producen la evidencia junto con el
layout de archivos que esperamos registrar (normalmente resúmenes JSON, logs o
manifiestos). Los equipos dejan nuevos artefactos bajo la carpeta correspondiente
cuando una ejecución de automatización cambia de forma material los docs
publicados, y luego enlazan el commit desde la entrada relevante de status/roadmap.

## Uso

1. **Ejecuta la automatización** con los comandos descritos en el README del
   subdirectorio (por ejemplo, `ci/check_android_fixtures.sh` o
   `cargo xtask da-threat-model-report`).
2. **Copia los artefactos JSON/log resultantes** desde `artifacts/…` hacia la
   carpeta `docs/automation/<program>/…` correspondiente, usando una marca de
   tiempo ISO-8601 en el nombre del archivo para que los auditores puedan
   correlacionar la evidencia con las actas de gobernanza.
3. **Referencia el commit** en `status.md`/`roadmap.md` al cerrar un gate del
   roadmap, para que los revisores puedan confirmar la baseline de automatización
   usada en esa decisión.
4. **Mantén los archivos ligeros.** La expectativa es metadatos estructurados,
   manifiestos o resúmenes, no blobs binarios voluminosos. Los volúmenes grandes
   deben quedarse en object storage con la referencia firmada registrada aquí.

Al centralizar estas notas de automatización desbloqueamos el requisito de
“baselines de docs/automation disponibles para auditoría” que menciona AND6 y
le damos al flujo del modelo de amenazas de DA un hogar determinista para los
reportes nocturnos y las comprobaciones puntuales manuales.
