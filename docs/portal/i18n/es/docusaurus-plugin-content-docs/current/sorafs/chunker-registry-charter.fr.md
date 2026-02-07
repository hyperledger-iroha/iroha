---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: carta-registro-fragmento
título: Charte du registre fragmentador SoraFS
sidebar_label: fragmentador de registro de cartas
descripción: Carta de gobierno para las misiones y aprobaciones de perfiles fragmentados.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_registry_charter.md`. Guarde las dos copias sincronizadas junto con la retraite complète du set Sphinx hérité.
:::

# Carta de gobierno del registro fragmentador SoraFS

> **Ratificado :** 2025-10-29 por el Panel de Infraestructura del Parlamento de Sora (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Toda enmienda exige una
> votación de gobierno formal; les équipes d'implementation doivent traidor ce document comme
> normatif jusqu'à l'approbation d'une charte de replacement.

Esta tabla define el proceso y las funciones para hacer evolucionar el registro fragmentador SoraFS.
Elle complète le [Guía de creación de perfiles fragmentados](./chunker-profile-authoring.md) y decrivant comment de nouveaux
Los perfiles son propuestos, revisados, ratificados y finalmente depreciados.

## Portée

La carta se aplica a cada plato principal de `sorafs_manifest::chunker_registry` et
Todas las herramientas que utilizan el registro (CLI de manifiesto, CLI de anuncio de proveedor,
SDK). Elle impone les invariants d'alias et de handle vérifiés par
`chunker_registry::ensure_charter_compliance()`:- Los ID de perfil son todos los aspectos positivos que aumentan la apariencia monótona.
- Le handle canonique `namespace.name@semver` **hacer** aparato en estreno
- Las cadenas de alias son trimées, únicas y no colisionan con los mangos.
  canónicos de otros platos principales.

## Roles

- **Autor(es)** – preparar la propuesta, regular los accesorios y cobrar
  Preuves de determinismo.
- **Grupo de Trabajo sobre Herramientas (TWG)** – validar la propuesta de ayuda a las listas de verificación
  publiées et s'assure que les invariants du registre sont respectés.
- **Consejo de Gobernanza (GC)** – examinar el informe del GTT y firmar el sobre de la propuesta
  y apruebe los calendarios de publicación/dépreciación.
- **Equipo de almacenamiento** – mantenimiento de la implementación del registro y publicación
  les mises à jour de la documentación.

## Flujo del ciclo de vida

1. **Sumisión de la propuesta**
   - El autor ejecuta la lista de verificación de validación de la guía del autor y la creación.
     un JSON `ChunkerProfileProposalV1` bajo
     `docs/source/sorafs/proposals/`.
   - Incluir la salida CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Soumettre une PR contenidos de fijación, propuesta, relación de determinismo y
     Mises à jour du registre.2. **Revisión de herramientas (TWG)**
   - Actualizar la lista de verificación de validación (accesorios, fuzz, manifiesto de canalización/PoR).
   - Ejecutador `cargo test -p sorafs_car --chunker-registry` y verificador que
     `ensure_charter_compliance()` Pase con la nueva entrada.
   - Verificador de comportamiento de CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflejan los alias y manejan mis à jour.
   - Produire un Court rapport résumant les constats et le statut pass/fail.

3. **Aprobación del Consejo (CG)**
   - Examinador de la relación del GTT y de los metadonnées de la propuesta.
   - Firmante del resumen de la proposición (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     y agregue las firmas al sobre del consejo de mantenimiento con los accesorios.
   - Consignar el resultado del voto en las actas de gobierno.

4. **Publicación**
   - Fusionador de relaciones públicas en pleno día:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`, guías de autor/conformidad).
     - Accesorios y relaciones de determinismo.
   - Notificar a los operadores y equipos SDK del nuevo perfil y del lanzamiento anterior.

5. **Dépréciation / Retrait**
   - Las proposiciones que reemplazan un perfil existente deben incluir una ventana de publicación.
     double (périodes de grâce) y un plan de actualización.
   - Después de la expiración de la ventana de gracia, marque el perfil reemplazado como depreciado.
     dans le registre et mettre à jour le ledger de migraciones.6. **Cambios de urgencia**
   - Las supresiones o revisiones exigen un voto del consejo mayoritario.
   - El GTT debe documentar las etapas de mitigación de riesgos y seguir el diario del incidente.

## Herramientas Attentes

- Exponente `sorafs_manifest_chunk_store` e `sorafs_manifest_stub`:
  - `--list-profiles` para la inspección del registro.
  - `--promote-profile=<handle>` para generar el bloque de metadóneos canónicos utilizados
    Lors de la promoción de un perfil.
  - `--json-out=-` para transmitir las relaciones con la salida estándar, permitiendo los registros de revisión
    reproducibles.
- `ensure_charter_compliance()` est invoqué au démarrage dans les binaires concernés
  (`manifest_chunk_store`, `provider_advert_stub`). Las pruebas CI deben repetirse si
  de nouvelles entretrées violento la charte.

## Registrarse

- Stocker tous les rapports de determinisme dans `docs/source/sorafs/reports/`.
- Las actas del consejo referentes a las decisiones que se toman en cuenta.
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` e `status.md` después de cada cambio mayor del registro.

## Referencias

- Guía de creación: [Guía de creación de perfiles fragmentados](./chunker-profile-authoring.md)
- Lista de verificación de conformidad: `docs/source/sorafs/chunker_conformance.md`
- Référence du registre: [Registre des profils fragmenter](./chunker-registry.md)