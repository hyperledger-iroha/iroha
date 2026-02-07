---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: Guía de conformidad del fragmentador de SoraFS
sidebar_label: Conformidad de fragmentador
descripción: Requisitos y flujos para preservar el perfil determinista de chunker SF1 en accesorios y SDK.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_conformance.md`. Mantén ambas versiones sincronizadas hasta que se retiren los documentos heredados.
:::

Esta guía codifica los requisitos que toda implementación debe seguir para mantenerse
alineada con el perfil determinista de fragmentador de SoraFS (SF1). También
documenta el flujo de regeneración, la política de firmas y los pasos de verificación para que
los consumidores de dispositivos en los SDK permanecerán sincronizados.

## Perfil canónico

- Handle del perfil: `sorafs.sf1@1.0.0` (alias heredado `sorafs.sf1@1.0.0`)
- Semilla de entrada (hex): `0000000000dec0ded`
- Tamaño objetivo: 262144 bytes (256 KiB)
- Tamaño mínimo: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polinomio de rodadura: `0x3DA3358B4DC173`
- Semilla de la tabla gear: `sorafs-v1-gear`
- Máscara de rotura: `0x0000FFFF`

Implementación de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Cualquier aceleración SIMD debe producir límites y digestos idénticos.

## Paquete de accesorios

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera los
accesorios y emite los siguientes archivos bajo `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — límites de fragmentos canónicos para consumidores
  Rust, TypeScript y Go. Cada archivo anuncia el mango canónico como la primera
  entrada en `profile_aliases`, seguida por cualquier alias heredado (p. ej.,
  `sorafs.sf1@1.0.0`, luego `sorafs.sf1@1.0.0`). El orden se impone por
  `ensure_charter_compliance` y NO DEBE alterarse.
- `manifest_blake3.json` — manifiesto verificado con BLAKE3 que cubre cada archivo de accesorios.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sobre el digest del manifiesto.
- `sf1_profile_v1_backpressure.json` y corpus en bruto dentro de `fuzz/` —
  escenarios deterministas de streaming usados por pruebas de contrapresión del chunker.

### Política de firmas

La regeneración de accesorios **debe** incluir una firma válida del consejo. El generador
rechaza la salida sin firmar a menos que se pase explícitamente `--allow-unsigned` (pensado
solo para experimentación local). Los sobres de firma son append-only y se
deduplican por firmante.

Para agregar una firma del consejo:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

##Verificación

El helper de CI `ci/check_sorafs_fixtures.sh` reejecuta el generador con
`--locked`. Si los partidos divergen o faltan firmas, el trabajo falla. Estados Unidos
este script en flujos de trabajo nocturnos y antes de enviar cambios de accesorios.

Manual de pasos de verificación:

1. Ejecuta `cargo test -p sorafs_chunker`.
2. Invoca `ci/check_sorafs_fixtures.sh` localmente.
3. Confirma que `git status -- fixtures/sorafs_chunker` esté limpio.

## Playbook de actualizaciónCuando propongas un nuevo perfil de fragmentador o actualizas SF1:

Ver también: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadatos, plantillas de propuesta y checklists de validación.

1. Redacta un `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) con nuevos parámetros.
2. Regenera accesorios vía `export_vectors` y registra el nuevo digest del manifest.
3. Firma el manifiesto con el quórum del consejo requerido. Todas las firmas deben
   anexar a `manifest_signatures.json`.
4. Actualiza los dispositivos de SDK afectados (Rust/Go/TS) y asegura la paridad entre tiempos de ejecución.
5. Regenera corpora fuzz si cambian los parámetros.
6. Actualiza esta guía con el nuevo handle de perfil, seeds y digest.
7. Envía el cambio junto con pruebas actualizadas y actualizaciones del roadmap.

Los cambios que afectan los límites de chunk o los digests sin seguir este proceso
son inválidos y no deben fusionarse.