---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-rollout-checklist
כותרת: רשימת רשימת פתיחה של רישום של chunker de SoraFS
sidebar_label: רשימת בדיקה להפצה של chunker
תיאור: תוכנית ההפצה לאחר מכן עבור אקטואליזציה של רישום ה-chunker.
---

:::הערה Fuente canónica
Refleja `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# רשימת רשימת רישום של SoraFS

Este checklist captura los pasos cecesarios para promotor un nuevo perfil de chunker
o un bundle de admisión de proveedores desde revisión a producción después de que la
קרטה דה גוברננצה חיה סידו ratificada.

> **Alcance:** אפליקציית עדכוני המהדורות que modifican
> `sorafs_manifest::chunker_registry`, los sobres de admisión de proveedores o los
> חבילות של אביזרי canónicos (`fixtures/sorafs_chunker/*`).

## 1. Validación previa

1. אביזרי Regenera y verifica el determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. אשר que los hashes de determinismo en
   `docs/source/sorafs/reports/sf1_determinism.md` (o el reporte de perfil relevante)
   coinciden con los artefactos regenerados.
3. Asegura que `sorafs_manifest::chunker_registry` compila con
   הוצאת `ensure_charter_compliance()`:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualiza el dossier de la propuesta:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de actas del consejo en `docs/source/sorafs/council_minutes_*.md`
   - Reporte determinismo

## 2. Aprobación de gobernanza

1. Presenta el informe del Tooling Working Group y el digest de la propuesta al
   פאנל תשתיות פרלמנט סורה.
2. Registra los detalles de aprobación en
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. מערכות ההפעלה Publica el sobre firmado por el Parlamento junto a los:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifica que el sobre sea accesible vía el helper de fetch de gobernanza:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. השקה בשלבים

Consulta el [playbook de manifest en staging](./staging-manifest-playbook) para un
recorrido detallado de estos pasos.

1. Despliega Torii עם גילוי `torii.sorafs` התאמתו ליישום
   הפעלת גישה (`enforce_admission = true`).
2. Sube los sobres de admisión de proveedores aprobados al directorio de registro
   de staging referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. אימות הפרסומות של ספקי הפרסום באמצעות גילוי ה-API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Ejecuta los point endpoints de manifest/plan con headers de gobernanza:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. אשר את לוחות המחוונים de telemetría (`torii_sorafs_*`) y las reglas de
   התראה מדווחת אל נואבו פרפיל חטא שגיאות.

## 4. השקה בהפקה

1. Repite los pasos de staging contra los nodos Torii de producción.
2. Anuncia la ventana de activación (fecha/hora, periodo de gracia, plan de rollback)
   a los canales de operadores y SDK.
3. Fusiona el PR de release que contiene:
   - מתקנים ומציאותיים
   - Cambios de documentación (התייחסויות א-לה-קארטה, reporte determinismo)
   - רענון מפת הדרכים/סטטוס
4. Etiqueta la release y archiva los artefactos firmados para la procedencia.

## 5. Auditoría לאחר ההשקה1. Captura métricas finals (conteos de discovery, tasa de éxito de fetch,
   היסטוגרמות של שגיאה) 24 שעות ספוג של השקה.
2. Actualiza `status.md` con un resumen corto y un enlace al reporte determinismo.
3. Registra cualquier tarea de seguimiento (עמוד ej., más guía de autoría de perfiles)
   en `roadmap.md`.