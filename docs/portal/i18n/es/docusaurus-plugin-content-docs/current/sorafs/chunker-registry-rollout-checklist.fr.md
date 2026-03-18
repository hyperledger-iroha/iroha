---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: Lista de verificación de implementación del registro fragmentado SoraFS
sidebar_label: fragmentación del lanzamiento de la lista de verificación
Descripción: Plan de implementación paso a paso para las actualizaciones del día del registro fragmentado.
---

:::nota Fuente canónica
Refleje `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Guarde las dos copias sincronizadas junto con la retraite complète du set Sphinx hérité.
:::

# Lista de verificación de implementación del registro SoraFS

Esta lista de verificación detalla las etapas necesarias para promover un nuevo perfil
Chucker o un paquete de admisión facilitador de la revista a la producción después de
ratificación de la carta de gobierno.

> **Portée :** S'applique à toutes les releases qui modificant
> `sorafs_manifest::chunker_registry`, les sobres de los facilitadores de admisión o
> les paquetes de accesorios canónicos (`fixtures/sorafs_chunker/*`).

## 1. Validación preliminar

1. Régénérez les fixtures et vérifiez le determinismo :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmez que les hashes de determinismo dans
   `docs/source/sorafs/reports/sf1_determinism.md` (ou le rapport de profil
   pertinente) corrent aux artefactos régénérés.
3. Asegúrese de compilar `sorafs_manifest::chunker_registry` con
   `ensure_charter_compliance()` en funcionamiento:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Mettez à jour le dossier de proposition :
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de actas del consejo bajo `docs/source/sorafs/council_minutes_*.md`
   - Informe de determinismo

## 2. Gobernanza de validación1. Presente el informe del Grupo de Trabajo sobre Herramientas y el resumen de la propuesta
   Panel de Infraestructura del Parlamento de au Sora.
2. Registre los detalles de aprobación en
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publiez l'sobre firmado por le Parlement à côté des fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique que el sobre esté accesible a través del asistente de recuperación de gobierno:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Puesta en escena del lanzamiento

Référez-vous au [puesta en escena del manifiesto del libro de jugadas](./staging-manifest-playbook) para une
procedimiento detallado.

1. Implemente Torii con el descubrimiento `torii.sorafs` activado y la aplicación de
   La admisión activa (`enforce_admission = true`).
2. Poussez les sobres de admisión aprobados por los proveedores en el repertorio
   de registre staging référencé par `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique que los anuncios del proveedor se propaguen a través del descubrimiento de API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Ejercer el manifiesto/plan de puntos finales con los encabezados de gobierno:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme que los paneles de télémétrie (`torii_sorafs_*`) y las reglas
   d'alerte reportent le nouveau profil sans erreurs.

## 4. Producción de lanzamiento1. Repétez les étapes de staging sur les nœuds Torii de production.
2. Annoncez la fenêtre d'activation (fecha/hora, período de gracia, plan de reversión)
   aux canales operadores y SDK.
3. Fusionar el PR de lanzamiento de contenido:
   - Accesorios y sobre mises à jour
   - Cambios de documentación (références à la charte, rapport de determinismo)
   - Actualizar hoja de ruta/estado
4. Taguez la release et archivez les artefactos signés pour la provenance.

## 5. Auditoría posterior al lanzamiento

1. Capturez les métriques finales (cuenta descubrimiento, taux de succès fetch,
   histogramas de errores) 24 h después del lanzamiento.
2. Mettez à jour `status.md` avec un bref résumé et un lien vers le rapport de determinismo.
3. Consignez les tâches de suivi (ej. guiar d'authoring de profils) dans
   `roadmap.md`.