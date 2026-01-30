---
lang: es
direction: ltr
source: docs/source/sorafs/chunker_registry_charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8203af1ad0b2f7e929b94f8517829fc7a76d60ce923661457bc97c1f750d3800
source_last_modified: "2025-11-22T09:23:15.841289+00:00"
translation_last_reviewed: "2026-01-30"
---

# Carta de governance del registro de chunker SoraFS

> **Ratificada:** 2025-10-29 por el Sora Parliament Infrastructure Panel (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Cualquier enmienda
> requiere un voto formal de governance; los equipos de implementacion deben
> tratar este documento como normativo hasta que se apruebe una carta
> sustituta.

Esta carta define el proceso y los roles para evolucionar el registro de
chunker SoraFS. Complementa la guia de authoring
(`docs/source/sorafs/chunker_profile_authoring.md`) describiendo como se
proponen, revisan y aprueban nuevos perfiles.

## Alcance

La carta aplica a cada entrada en `sorafs_manifest::chunker_registry` y a
cualquier tooling que consuma el registry (manifest CLI, provider-advert CLI,
SDKs). Hace cumplir las invariantes de alias y handle verificadas por
`chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que incrementan de forma monotonica.
- El handle canonico `namespace.name@semver` **debe** aparecer como primera entrada en `profile_aliases`.
- Los strings de alias se recortan, son unicos y no colisionan con handles
  canonicos de otras entradas.

## Roles

- **Author(s)** – preparan la propuesta, regeneran fixtures y recolectan evidencia
de determinismo.
- **Tooling Working Group (TWG)** – valida la propuesta usando los checklists
  publicados y asegura que las invariantes del registry se cumplen.
- **Governance Council (GC)** – revisa el reporte TWG, firma el sobre de
  propuesta y aprueba timelines de publicacion/deprecacion.
- **Storage Team** – mantiene la implementacion del registry y publica updates
  de documentacion.

## Workflow de ciclo de vida

1. **Envio de propuesta**
   - El autor ejecuta el checklist de validacion de la guia de authoring y crea
     un JSON `ChunkerProfileProposalV1` bajo `docs/source/sorafs/proposals/`.
   - Incluir output de CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Enviar un PR que incluya fixtures, propuesta, reporte de determinismo y
     updates de registry.

2. **Revision de tooling (TWG)**
   - Repetir el checklist de validacion (fixtures, fuzz, pipeline manifest/PoR).
   - Ejecutar `cargo test -p sorafs_car --chunker-registry` y asegurar que
     `ensure_charter_compliance()` pasa con la entrada nueva.
   - Verificar el comportamiento del CLI (`--list-profiles`, `--promote-profile`,
     streaming `--json-out=-`) refleje los aliases y handles actualizados.
   - Producir un reporte corto con findings y estado pass/fail.

3. **Aprobacion del council (GC)**
   - Revisar el reporte TWG y metadata de la propuesta.
   - Firmar el digest de propuesta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     y anexar firmas al sobre del council mantenido junto a los fixtures.
   - Registrar el resultado de la votacion en las minutas de governance.

4. **Publicacion**
   - Mergear el PR, actualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentacion (`chunker_registry.md`, guias de authoring/conformance).
     - Fixtures y reportes de determinismo.
   - Notificar a operadores y equipos SDK del nuevo perfil y rollout planificado.

5. **Deprecacion / sunset**
   - Propuestas que sustituyan un perfil existente deben incluir una ventana de
     dual-publish (grace periods) y un plan de upgrade. Marcar el perfil
     anterior como deprecado en el registry y actualizar el migration ledger.

6. **Cambios de emergencia**
   - Remocion o hotfixes requieren un voto del council con mayoria.
   - TWG debe documentar los pasos de mitigacion de riesgo y actualizar el log
     de incidentes.

## Expectativas de tooling

- `sorafs_manifest_chunk_store` y `sorafs_manifest_stub` exponen:
  - `--list-profiles` para inspeccion del registry.
  - `--promote-profile=<handle>` para generar el bloque de metadata canonico
    usado al promover un perfil.
  - `--json-out=-` para streamear reportes a stdout, habilitando logs de
    revision reproducibles.
- `ensure_charter_compliance()` se invoca al arranque en binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Los tests CI deben fallar
  si nuevas entradas violan la carta.

## Record keeping

- Guardar reportes de determinismo en `docs/source/sorafs/reports/`.
- Minutas del council que referencian decisiones de chunker viven bajo
  `docs/source/sorafs/migration_ledger.md`.
- Actualizar `roadmap.md` y `status.md` despues de cada cambio mayor del registry.

## Referencias

- Guia de authoring: `docs/source/sorafs/chunker_profile_authoring.md`
- Checklist de conformidad: `docs/source/sorafs/chunker_conformance.md`
- Referencia del registry: `docs/source/sorafs/chunker_registry.md`
