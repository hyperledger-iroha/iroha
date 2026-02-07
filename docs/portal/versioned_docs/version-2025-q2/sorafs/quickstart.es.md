---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Inicio rápido

Esta guía práctica recorre el perfil determinista del fragmentador SF-1,
firma de manifiesto y flujo de recuperación de múltiples proveedores que sustentan el SoraFS
tubería de almacenamiento. Combínelo con el [análisis profundo del canal de manifiesto](manifest-pipeline.md)
para notas de diseño y material de referencia de banderas CLI.

## Requisitos previos

- Cadena de herramientas Rust (`rustup update`), espacio de trabajo clonado localmente.
- Opcional: [par de claves Ed25519 generado por OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para firmar manifiestos.
- Opcional: Node.js ≥ 18 si planea obtener una vista previa del portal Docusaurus.

Configure `export RUST_LOG=info` mientras experimenta para mostrar mensajes CLI útiles.

## 1. Actualizar los accesorios deterministas

Regenerar los vectores de fragmentación canónicos SF-1. El comando también emite firmado
sobres de manifiesto cuando se suministre `--signing-key`; utilizar `--allow-unsigned`
sólo durante el desarrollo local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Salidas:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si está firmado)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Divida una carga útil e inspeccione el plan

Utilice `sorafs_chunker` para fragmentar un archivo o archivo arbitrario:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos clave:

- `profile` / `break_mask` – confirma los parámetros `sorafs.sf1@1.0.0`.
- `chunks[]`: desplazamientos ordenados, longitudes y resúmenes de fragmentos BLAKE3.

Para dispositivos más grandes, ejecute la regresión respaldada por proptest para garantizar la transmisión y
La fragmentación por lotes permanece sincronizada:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Cree y firme un manifiesto

Envuelva el plan de fragmentos, los alias y las firmas de gobierno en un manifiesto usando
`sorafs-manifest-stub`. El siguiente comando muestra una carga útil de un solo archivo; pasar
una ruta de directorio para empaquetar un árbol (la CLI lo recorre lexicográficamente).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Revise `/tmp/docs.report.json` para:

- `chunking.chunk_digest_sha3_256` – Resumen SHA3 de compensaciones/longitudes, coincide con el
  accesorios más gruesos.
- `manifest.manifest_blake3` – Resumen BLAKE3 firmado en el sobre del manifiesto.
- `chunk_fetch_specs[]` – instrucciones de búsqueda ordenadas para orquestadores.

Cuando esté listo para proporcionar firmas reales, agregue `--signing-key` e `--signer`
argumentos. El comando verifica cada firma Ed25519 antes de escribir el
sobre.

## 4. Simular la recuperación de múltiples proveedores

Utilice la CLI de recuperación del desarrollador para reproducir el plan fragmentado en uno o más
proveedores. Esto es ideal para pruebas de humo de CI y creación de prototipos de orquestadores.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Afirmaciones:

- `payload_digest_hex` debe coincidir con el informe del manifiesto.
- `provider_reports[]` muestra recuentos de éxito/fracaso por proveedor.
- `chunk_retry_total` distinto de cero resalta los ajustes de contrapresión.
- Pase `--max-peers=<n>` para limitar la cantidad de proveedores programados para una ejecución
  y mantener las simulaciones de CI centradas en los candidatos principales.
- `--retry-budget=<n>` anula el recuento de reintentos por fragmento predeterminado (3) para que
  puede hacer emerger las regresiones del orquestador más rápidamente al inyectar fallas.

Agregue `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para fallar
rápido cuando la carga útil reconstruida se desvía del manifiesto.

## 5. Próximos pasos- **Integración de gobernanza**: canalice el resumen del manifiesto y
  `manifest_signatures.json` en el flujo de trabajo del consejo para que el Registro de PIN pueda
  anunciar disponibilidad.
- **Negociación de registro** – consultar [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar nuevos perfiles. La automatización debería preferir manijas canónicas.
  (`namespace.name@semver`) sobre ID numéricos.
- **Automatización de CI**: agregue los comandos anteriores para liberar canalizaciones, por lo que los documentos,
  accesorios y artefactos publican manifiestos deterministas junto con firmas
  metadatos.