---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: Guía de conformidad del fragmentador SoraFS
sidebar_label: trozo conformité
Descripción: Exigencias y flujos de trabajo para preservar el perfil fragmentador SF1 determinado en los dispositivos y SDK.
---

:::nota Fuente canónica
:::

Esta guía codifica las exigencias que cada implementación debe seguir para descansar.
alineado con el perfil fragmentador determinado por SoraFS (SF1). El documento también
el flujo de trabajo de regeneración, la política de firmas y las etapas de verificación para que
Los desarrolladores de accesorios y SDK están sincronizados.

## Perfil canónico

- Semilla de entrada (hexadecimal): `0000000000dec0ded`
- Taille cible: 262144 bytes (256 KiB)
- Tamaño mínimo: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polínomo de rodadura: `0x3DA3358B4DC173`
- Semillas de engranaje de mesa : `sorafs-v1-gear`
- Máscara de ruptura : `0x0000FFFF`

Implementación de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Toda aceleración SIMD debe producir límites y resúmenes idénticos.

## Paquete de accesorios

`cargo run --locked -p sorafs_chunker --bin export_vectors` regénère les
accesorios y guarde los archivos siguientes bajo `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — límites de fragmentos canónicos para les
  desarrolladores Rust, TypeScript y Go. Chaque fichier annonce le handle canonique
  `sorafs.sf1@1.0.0`, luego `sorafs.sf1@1.0.0`). El orden está impuesto por
  `ensure_charter_compliance` y NE DOIT PAS être modifié.
- `manifest_blake3.json` — manifiesto verificado BLAKE3 que contiene cada archivo de accesorios.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sur le digest du manifest.
- `sf1_profile_v1_backpressure.json` y cuerpos brutos en `fuzz/` —
  Escenarios de transmisión determinados utilizados por las pruebas de contrapresión del fragmentador.

### Política de firma

La regeneración de los accesorios **doit** incluye una firma válida del consejo. El generador
rejette la sortie non signée sauf si `--allow-unsigned` est passé explicitement (prévu
único para la experimentación local). Los sobres de firma son sólo para adjuntar y
sont dédupliquées par signataire.

Para agregar una firma del consejo:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificación

El asistente CI `ci/check_sorafs_fixtures.sh` activa el generador con
`--locked`. Si los accesorios divergen o si las firmas son frecuentes, el trabajo se hace eco. utilizar
Este script en los flujos de trabajo de la noche y antes de los cambios de accesorios.

Étapes de verification manuelle:

1. Ejecute `cargo test -p sorafs_chunker`.
2. Localización Lancez `ci/check_sorafs_fixtures.sh`.
3. Confirme que `git status -- fixtures/sorafs_chunker` es apropiado.

## Libro de jugadas de puesta a nivelLorsqu'on propone un nuevo perfil fragmentado o lo que se conoció en el día SF1:

Ver también: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para les
exigencias de metadonnées, les templates de proposition et les checklists de validation.

1. Registre un `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) con nuevos parámetros.
2. Regénérez les fixtures via `export_vectors` et consignez le nouveau digest du manifest.
3. Firmar el manifiesto con el quórum requerido. Todas las firmas deben existir
   apéndices de `manifest_signatures.json`.
4. Actualice los accesorios SDK correspondientes (Rust/Go/TS) y asegure la paridad entre tiempos de ejecución.
5. Regénérez les corpora fuzz si les paramètres changent.
6. Mettez à jour ce Guide avec le nouveau handle de profil, les seeds et le digest.
7. Soumettez la modificación con las pruebas y las actualizaciones del día de la hoja de ruta.

Los cambios que afectan los límites de trozos o digestiones sin seguir este proceso
sont invalides et ne doivent pas être fusionnés.