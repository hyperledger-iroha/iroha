---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-conformance
title: Guía de conformidad del chunker de SoraFS
sidebar_label: Conformidad de chunker
description: Requisitos y flujos para preservar el perfil determinista de chunker SF1 en fixtures y SDKs.
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_conformance.md`. Mantén ambas versiones sincronizadas hasta que se retiren los docs heredados.
:::

Esta guía codifica los requisitos que toda implementación debe seguir para mantenerse
alineada con el perfil determinista de chunker de SoraFS (SF1). También
documenta el flujo de regeneración, la política de firmas y los pasos de verificación para que
los consumidores de fixtures en los SDKs permanezcan sincronizados.

## Perfil canónico

- Handle del perfil: `sorafs.sf1@1.0.0` (alias heredado `sorafs.sf1@1.0.0`)
- Seed de entrada (hex): `0000000000dec0ded`
- Tamaño objetivo: 262144 bytes (256 KiB)
- Tamaño mínimo: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polinomio de rolling: `0x3DA3358B4DC173`
- Seed de la tabla gear: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Implementación de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Cualquier aceleración SIMD debe producir límites y digests idénticos.

## Bundle de fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera los
fixtures y emite los siguientes archivos bajo `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — límites de chunk canónicos para consumidores
  Rust, TypeScript y Go. Cada archivo anuncia el handle canónico como la primera
  entrada en `profile_aliases`, seguido por cualquier alias heredado (p. ej.,
  `sorafs.sf1@1.0.0`, luego `sorafs.sf1@1.0.0`). El orden se impone por
  `ensure_charter_compliance` y NO DEBE alterarse.
- `manifest_blake3.json` — manifest verificado con BLAKE3 que cubre cada archivo de fixtures.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sobre el digest del manifest.
- `sf1_profile_v1_backpressure.json` y corpora en bruto dentro de `fuzz/` —
  escenarios deterministas de streaming usados por pruebas de back-pressure del chunker.

### Política de firmas

La regeneración de fixtures **debe** incluir una firma válida del consejo. El generador
rechaza la salida sin firmar a menos que se pase explícitamente `--allow-unsigned` (pensado
solo para experimentación local). Los sobres de firma son append-only y se
deduplican por firmante.

Para agregar una firma del consejo:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificación

El helper de CI `ci/check_sorafs_fixtures.sh` reejecuta el generador con
`--locked`. Si los fixtures divergen o faltan firmas, el job falla. Usa
este script en workflows nocturnos y antes de enviar cambios de fixtures.

Pasos de verificación manual:

1. Ejecuta `cargo test -p sorafs_chunker`.
2. Invoca `ci/check_sorafs_fixtures.sh` localmente.
3. Confirma que `git status -- fixtures/sorafs_chunker` esté limpio.

## Playbook de actualización

Cuando propongas un nuevo perfil de chunker o actualices SF1:

Ver también: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadatos, plantillas de propuesta y checklists de validación.

1. Redacta un `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) con nuevos parámetros.
2. Regenera fixtures vía `export_vectors` y registra el nuevo digest del manifest.
3. Firma el manifest con el quórum del consejo requerido. Todas las firmas deben
   anexarse a `manifest_signatures.json`.
4. Actualiza las fixtures de SDK afectadas (Rust/Go/TS) y asegura paridad cross-runtime.
5. Regenera corpora fuzz si cambian los parámetros.
6. Actualiza esta guía con el nuevo handle de perfil, seeds y digest.
7. Envía el cambio junto con pruebas actualizadas y actualizaciones del roadmap.

Los cambios que afecten los límites de chunk o los digests sin seguir este proceso
son inválidos y no deben fusionarse.
