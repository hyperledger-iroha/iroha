---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: Чеклист rollout реестра fragmentador SoraFS
sidebar_label: fragmento de implementación de resumen
descripción: Implementación del plan de actualización para el fragmentador de registro exclusivo.
---

:::nota Канонический источник
Introduzca `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Deje copias sincronizadas de los documentos de Sphinx que no contienen especificaciones.
:::

# Чеклист rollout реестра SoraFS

Esta lista de archivos ficticios no está disponible para el nuevo perfil de fragmentación del producto.
или admisión del proveedor del paquete от ревью до продакшена после ратификации
carta de gobernanza.

> **Область:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, sobres de admisión de proveedores или
> paquetes de accesorios канонические (`fixtures/sorafs_chunker/*`).

## 1. Validación previa

1. Pregenere los accesorios y proteja la determinación:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Убедитесь, что hashes детерминизма в
   `docs/source/sorafs/reports/sf1_determinism.md` (или релевантном отчете
   профиля) совпадают с regеnerированными артефактами.
3. Убедитесь, что `sorafs_manifest::chunker_registry` компилируется с
   `ensure_charter_compliance()` por mensaje:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите expediente предложения:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Запись minutos совета в `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. Aprobación de la gobernanza1. Consulte el grupo de trabajo sobre herramientas y resuma la publicación en
   Panel de Infraestructura del Parlamento de Sora.
2. Зафиксируйте детали одобрения в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Опубликуйте sobre, подписанный парламентом, вместе с accesorios:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Проверьте, что sobre доступен через ayudante получения gobernanza:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Lanzamiento de la puesta en escena

Подробный tutorial см. в [libro de estrategias del manifiesto de puesta en escena] (./staging-manifest-playbook).

1. Разверните Torii с включенным descubrimiento `torii.sorafs` y включенным aplicación
   admisión (`enforce_admission = true`).
2. Introducir los sobres de admisión de proveedores aprobados en el directorio de registro de preparación,
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. Asegúrese de que los anuncios del proveedor se adapten a su API de descubrimiento:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Progrese el manifiesto/plan de puntos finales con los encabezados de gobernanza:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Убедитесь, что paneles de telemetría (`torii_sorafs_*`) y reglas de alerta
   отображают новый профиль без ошибок.

## 4. Lanzamiento de la producción

1. Coloque la puesta en escena en el producto Torii-узлах.
2. Activar activaciones (datos/inicios, período de gracia, plan de reversión) en los canales
   operadores y SDK.
3. Смёрджите релизный PR с:
   - Обновленными accesorios y sobre
   - Documentos de identidad (sobre el alquiler, la documentación sobre la determinación)
   - Hoja de ruta/estado actualizados
4. Asegúrese de seleccionar y almacenar artefactos de procedencia.## 5. Auditoría posterior al lanzamiento

1. Снимите финальные метрики (recuentos de descubrimiento, tasa de éxito de recuperación, error
   histogramas) hace 24 horas después del lanzamiento.
2. Retire `status.md` para seleccionar la resolución y la configuración necesarias.
3. Заведите seguimiento задачи (por primera vez, дополнительная orientación по autoría
   perfil) en `roadmap.md`.