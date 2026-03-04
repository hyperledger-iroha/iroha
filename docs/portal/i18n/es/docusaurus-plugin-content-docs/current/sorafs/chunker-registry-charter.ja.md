---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 543ad9b0298e311683e59ac9e1f99bbe9e02534439e0788c262c3b432ceae8ae
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry-charter
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry_charter.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# Carta de gobernanza del registro de chunker de SoraFS

> **Ratificado:** 2025-10-29 por el Sora Parliament Infrastructure Panel (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Cualquier enmienda requiere un
> voto de gobernanza formal; los equipos de implementación deben tratar este documento como
> normativo hasta que se apruebe una carta que lo sustituya.

Esta carta define el proceso y los roles para evolucionar el registro de chunker de SoraFS.
Complementa la [Guía de autoría de perfiles de chunker](./chunker-profile-authoring.md) al describir cómo nuevos
perfiles se proponen, revisan, ratifican y eventualmente se deprecian.

## Alcance

La carta aplica a cada entrada en `sorafs_manifest::chunker_registry` y
a cualquier tooling que consuma el registro (CLI de manifest, CLI de provider-advert,
SDKs). Impone las invariantes de alias y handle verificadas por
`chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monótona.
- El handle canónico `namespace.name@semver` **debe** aparecer como primera
  entrada en `profile_aliases`. Siguen los alias heredados.
- Las cadenas de alias se recortan, son únicas y no colisionan con handles canónicos
  de otras entradas.

## Roles

- **Autor(es)** – preparan la propuesta, regeneran fixtures y recopilan la
  evidencia de determinismo.
- **Tooling Working Group (TWG)** – valida la propuesta usando las checklists
  publicadas y asegura que las invariantes del registro se cumplan.
- **Governance Council (GC)** – revisa el reporte del TWG, firma el sobre de la propuesta
  y aprueba los plazos de publicación/deprecación.
- **Storage Team** – mantiene la implementación del registro y publica
  actualizaciones de documentación.

## Flujo del ciclo de vida

1. **Presentación de propuesta**
   - El autor ejecuta la checklist de validación de la guía de autoría y crea
     un JSON `ChunkerProfileProposalV1` en
     `docs/source/sorafs/proposals/`.
   - Incluye la salida del CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envía un PR que contenga fixtures, propuesta, reporte de determinismo y
     actualizaciones del registro.

2. **Revisión de tooling (TWG)**
   - Reproduce la checklist de validación (fixtures, fuzz, pipeline de manifest/PoR).
   - Ejecuta `cargo test -p sorafs_car --chunker-registry` y asegura que
     `ensure_charter_compliance()` pase con la nueva entrada.
   - Verifica que el comportamiento del CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) refleje los alias y handles actualizados.
   - Produce un reporte breve que resuma los hallazgos y el estado de aprobación/rechazo.

3. **Aprobación del consejo (GC)**
   - Revisa el reporte del TWG y la metadata de la propuesta.
   - Firma el digest de la propuesta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     y añade las firmas al sobre del consejo mantenido junto a los fixtures.
   - Registra el resultado de la votación en las actas de gobernanza.

4. **Publicación**
   - Fusiona el PR, actualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`, guías de autoría/conformidad).
     - Fixtures y reportes de determinismo.
   - Notifica a operadores y equipos SDK del nuevo perfil y del rollout planificado.

5. **Deprecación / Retiro**
   - Las propuestas que sustituyen un perfil existente deben incluir una ventana de publicación
     dual (periodos de gracia) y un plan de actualización.
     en el registro y actualiza el ledger de migración.

6. **Cambios de emergencia**
   - Las eliminaciones o hotfixes requieren un voto del consejo con aprobación por mayoría.
   - El TWG debe documentar los pasos de mitigación de riesgos y actualizar el registro de incidentes.

## Expectativas de tooling

- `sorafs_manifest_chunk_store` y `sorafs_manifest_stub` exponen:
  - `--list-profiles` para inspección del registro.
  - `--promote-profile=<handle>` para generar el bloque de metadatos canónico usado
    al promover un perfil.
  - `--json-out=-` para transmitir reportes a stdout, habilitando logs de revisión
    reproducibles.
- `ensure_charter_compliance()` se invoca al inicio en los binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Las pruebas CI deben fallar si
  nuevas entradas violan la carta.

## Registro

- Guarda todos los reportes de determinismo en `docs/source/sorafs/reports/`.
- Las actas del consejo que referencian decisiones de chunker viven en
  `docs/source/sorafs/migration_ledger.md`.
- Actualiza `roadmap.md` y `status.md` después de cada cambio mayor del registro.

## Referencias

- Guía de autoría: [Guía de autoría de perfiles de chunker](./chunker-profile-authoring.md)
- Checklist de conformidad: `docs/source/sorafs/chunker_conformance.md`
- Referencia del registro: [Registro de perfiles de chunker](./chunker-registry.md)
