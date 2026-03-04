---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Inicio rapido de SoraFS

Esta guía práctica recorre el perfil determinista de chunker SF-1,
la firma de manifiestos y el flujo de recuperación multiproveedor que sustentan el
tubería de almacenamiento de SoraFS. Complétala con el
[análisis profundo del pipeline de manifiestos](manifest-pipeline.md)
para notas de diseño y referencia de banderas de la CLI.

##Requisitos previos

- Toolchain de Rust (`rustup update`), espacio de trabajo clonado localmente.
- Opcional: [par de claves Ed25519 generado con OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para firmar manifiestos.
- Opcional: Node.js ≥ 18 si planeas previsualizar el portal de Docusaurus.

Defina `export RUST_LOG=info` mientras experimenta para mostrar mensajes útiles de la CLI.

## 1. Actualiza los deterministas del calendario

Regenera los vectores canónicos de fragmentación SF-1. El comando también emite
sobres de manifiesto firmados cuando se proporciona `--signing-key`; estados unidos
`--allow-unsigned` solo durante el desarrollo local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Salidas:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si se firma)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmenta una carga útil e inspecciona el plan

Usa `sorafs_chunker` para fragmentar un archivo o un archivo comprimido arbitrario:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos clave:

- `profile` / `break_mask` – confirme los parámetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – compensa ordenados, longitudes y digiere BLAKE3 de los fragmentos.Para accesorios más grandes, ejecuta la regresión respaldada por proptest para
garantizar que el fragmentación en streaming y por lotes se mantenga sincronizado:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construye y firma un manifiesto

Envuelve el plan de chunks, los alias y las firmas de gobernanza en un manifiesto usando
`sorafs-manifest-stub`. El comando de abajo muestra una carga útil de un solo archivo; pasa
una ruta de directorio para empaquetar un árbol (la CLI lo recorre en orden lexicográfico).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Revisa `/tmp/docs.report.json` párrafo:

- `chunking.chunk_digest_sha3_256` – resumen SHA3 de desplazamientos/longitudes, coinciden con los
  accesorios del chunker.
- `manifest.manifest_blake3` – resumen BLAKE3 firmado en el sobre del manifiesto.
- `chunk_fetch_specs[]` – instrucciones de recuperación ordenadas para los orquestadores.

Cuando estés listo para aportar firmas reales, añade los argumentos `--signing-key` y
`--signer`. El comando verifica cada firma Ed25519 antes de escribir el sobre.

## 4. Simula la recuperación multiproveedor

Usa la CLI de fetch de desarrollo para reproducir el plan de fragmentos contra uno o más
proveedores. Es ideal para pruebas de humo de CI y prototipos de orquestador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Comprobaciones:- `payload_digest_hex` debe coincidir con el informe del manifiesto.
- `provider_reports[]` muestra relatos de éxito/fallo por proveedor.
- Un `chunk_retry_total` distinto de cero destaca ajustes de contrapresión.
- Pasa `--max-peers=<n>` para limitar la cantidad de proveedores programados para una ejecución
  y mantener las simulaciones de CI enfocadas en los candidatos principales.
- `--retry-budget=<n>` sobrescribe el recuento por defecto de reintentos por trozo (3) para
  detectar más rápidas regresiones del orquestador al inyectar fallos.

Agregue `--expect-payload-digest=<hex>` y `--expect-payload-len=<bytes>` para fallar rápido
cuando la carga reconstruida se desvíe del manifiesto.

## 5. Siguientes pasos

- **Integración de gobernanza** – canaliza el digest del manifiesto y
  `manifest_signatures.json` en el flujo del consejo para que el Pin Registry pueda
  anunciar disponibilidad.
- **Negociación del registro** – consulta [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar nuevos perfiles. La automatización debe preferir controladores canónicos.
  (`namespace.name@semver`) sobre ID numéricos.
- **Automatización de CI** – añade los comandos anteriores a los pipelines de liberación para que
  la documentación, accesorios y artefactos publiquen manifiestos deterministas junto con
  metadatos firmados.