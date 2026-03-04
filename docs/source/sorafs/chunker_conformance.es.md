---
lang: es
direction: ltr
source: docs/source/sorafs/chunker_conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e2cf1e1bd738a7f6cf022886a687873972f7dadb485a41d2bfb277be5062d4
source_last_modified: "2025-11-10T04:30:01.561891+00:00"
translation_last_reviewed: "2026-01-30"
---

# Guia de conformidad del chunker SoraFS

Esta guia codifica los requisitos que toda implementacion debe seguir para
mantenerse alineada con el perfil determinista de chunker SoraFS (SF1). Tambien
documenta el workflow de regeneracion, la politica de firmado y los pasos de
verificacion para que los consumidores de fixtures en SDKs permanezcan en sync.

> **Portal:** Espejado en `docs/portal/docs/sorafs/chunker-conformance.md`.
> Actualiza ambas copias para mantener alineados a los reviewers.

## Perfil canonico

- Handle de perfil: `sorafs.sf1@1.0.0`
- Semilla de entrada (hex): `0000000000dec0ded`
- Tamano objetivo: 262 144 bytes (256 KiB)
- Tamano minimo: 65 536 bytes (64 KiB)
- Tamano maximo: 524 288 bytes (512 KiB)
- Polinomio rolling: `0x3DA3358B4DC173`
- Semilla de tabla gear: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Implementacion de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Cualquier aceleracion SIMD debe producir limites y digests identicos.

## Bundle de fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera los
fixtures y emite los siguientes archivos bajo `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — limites de chunk canonicos para
  consumidores Rust, TypeScript y Go. Cada archivo anuncia el handle canonico
  como la primera (y unica) entrada en `profile_aliases`. El orden lo impone
  `ensure_charter_compliance` y NO DEBE alterarse.
- `manifest_blake3.json` — manifiesto verificado con BLAKE3 que cubre cada
  archivo fixture.
- `manifest_signatures.json` — firmas del council (Ed25519) sobre el digest del
  manifiesto.
- `sf1_profile_v1_backpressure.json` y corpora crudos dentro de `fuzz/` —
  escenarios de streaming deterministas usados por pruebas de back-pressure de
  chunker.

### Politica de firmado

La regeneracion de fixtures **debe** incluir una firma valida del council. El
 generador rechaza output sin firma salvo que se pase `--allow-unsigned`
explicitamente (intencionado solo para experimentos locales). Los sobres de
firma son append-only y se deduplican por firmante.

Para agregar una firma del council:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificacion

El helper de CI `ci/check_sorafs_fixtures.sh` re-ejecuta el generador con
`--locked`. Si los fixtures hacen drift o faltan firmas, el job falla. Usa este
script en workflows nightly y antes de enviar cambios de fixtures.

Pasos de verificacion manual:

1. Ejecutar `cargo test -p sorafs_chunker`.
2. Invocar `ci/check_sorafs_fixtures.sh` localmente.
3. Confirmar que `git status -- fixtures/sorafs_chunker` esta limpio.

## Playbook de upgrade

Al proponer un nuevo perfil de chunker o actualizar SF1:

Ver tambien: [`docs/source/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md) para
requisitos de metadata, plantillas de propuesta y checklists de validacion.

1. Redactar una `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) con nuevos parametros.
2. Regenerar fixtures via `export_vectors` y registrar el nuevo digest de manifiesto.
3. Firmar el manifiesto con el quorum requerido del council. Todas las firmas deben
   anexarse a `manifest_signatures.json`.
4. Actualizar fixtures de SDK afectados (Rust/Go/TS) y asegurar paridad cross-runtime.
5. Regenerar corpora fuzz si cambian los parametros.
6. Actualizar esta guia con el nuevo handle de perfil, seeds y digest.
7. Enviar el cambio junto a tests actualizados y updates de roadmap.

Cambios que afectan limites de chunk o digests sin seguir este proceso son
invalidos y no deben mergearse.
