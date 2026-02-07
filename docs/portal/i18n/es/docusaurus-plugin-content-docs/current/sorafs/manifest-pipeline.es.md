---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Pipeline de manifiestos

Este complemento del inicio rápido recorre el pipeline de extremo a extremo que convierte
bytes en bruto en manifiestos Norito aptos para el Pin Registry de SoraFS. El contenido está
adaptador de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte ese documento para la especificación canónica y el registro de cambios.

## 1. Fragmentar de forma determinista

SoraFS usa el perfil SF-1 (`sorafs.sf1@1.0.0`): un hash rodante inspirado en FastCDC con un
tamaño mínimo de fragmento de 64 KiB, un objetivo de 256 KiB, un máximo de 512 KiB y una máscara
de corte `0x0000ffff`. El perfil está registrado en `sorafs_manifest::chunker_registry`.

### Ayudantes de Rust

- `sorafs_car::CarBuildPlan::single_file` – Emite offsets de fragmentos, longitudes y resúmenes
  BLAKE3 mientras prepara los metadatos de CAR.
- `sorafs_car::ChunkStore` – Streamea payloads, persiste metadatos de chunks y deriva el árbol
  de muestreo Prueba de recuperabilidad (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Ayudante de biblioteca detrás de ambas CLI.

### Herramientas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

El JSON contiene los desplazamientos ordenados, las longitudes y los resúmenes de los fragmentos. Guarda
el plan al construir manifiestos o especificaciones de búsqueda del orquestador.

### Testigos PoR`ChunkStore` exponen `--por-proof=<chunk>:<segment>:<leaf>` y `--por-sample=<count>` para que
los auditores podrán solicitar conjuntos de testigos deterministas. Combina esos flags con
`--por-proof-out` o `--por-sample-out` para registrar el JSON.

## 2. Envolver un manifiesto

`ManifestBuilder` combina los metadatos de chunks con adjuntos de gobernanza:

- CID raíz (dag-cbor) y compromisos de CAR.
- Pruebas de alias y reclamaciones de capacidad de proveedores.
- Firmas del consejo y metadatos opcionales (p. ej., IDs de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Salidas importantes:

- `payload.manifest` – Bytes del manifiesto codificados en Norito.
- `payload.report.json` – Resumen legible para humanos/automatización, incluye
  `chunk_fetch_specs`, `payload_digest_hex`, resume CAR y metadatos de alias.
- `payload.manifest_signatures.json` – Sobre que contiene el digest BLAKE3 del manifiesto, el
  digest SHA3 del plan de chunks y firmas Ed25519 ordenadas.

Usa `--manifest-signatures-in` para verificar sobres proporcionados por firmantes externos
antes de volver a escribirlos, y `--chunker-profile-id` o `--chunker-profile=<handle>` para
fijar la selección del registro.

## 3. Publicar y pinar1. **Envío a gobernanza** – Proporciona el digest del manifiesto y el sobre de firmas al
   consejo para que el pin pueda ser admitido. Los auditores externos deben almacenar el
   digest SHA3 del plan de chunks junto al digest del manifiesto.
2. **Pinar cargas útiles** – Sube el archivo CAR (y el índice CAR opcional) referenciado en el
   manifiesto al Registro Pin. Asegura que el manifiesto y el CAR comparten el mismo CID raíz.
3. **Registrar telemetría** – Conserva el reporte JSON, los testigos PoR y cualquier métrica
   de fetch en los artefactos de liberación. Estos registros alimentan los paneles de control de
   operadores y ayudan a reproducir incidencias sin descargar payloads grandes.

## 4. Simulación de búsqueda multiproveedor

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` aumenta el paralelo por proveedor (`#4` arriba).
- `@<weight>` ajusta el ciclo de planificación; por defecto es 1.
- `--max-peers=<n>` limita el número de proveedores programados para una ejecución cuando el
  descubrimiento produce más candidatos de los deseados.
- `--expect-payload-digest` y `--expect-payload-len` protegen contra corrupción silenciosa.
- `--provider-advert=name=advert.to` verifica las capacidades del proveedor antes de usarlas
  en la simulación.
- `--retry-budget=<n>` reemplaza el recuento de reintentos por chunk (por defecto: 3) para que
  CI pueda exponer regresiones más rápidas al probar escenarios de fallo.

`fetch_report.json` muestra métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adecuados para aserciones de CI y observabilidad.

## 5. Actualizaciones del registro y gobernanza

Al proponer nuevos perfiles de chunker:

1. Redacta el descriptor en `sorafs_manifest::chunker_registry_data`.
2. Actualiza `docs/source/sorafs/chunker_registry.md` y los charter relacionados.
3. Regenera aparatos (`export_vectors`) y captura manifiestos firmados.
4. Envía el informe de cumplimiento del charter con firmas de gobernanza.

La automatización debe preferir handles canónicos (`namespace.name@semver`) y recurrir a IDs
numéricos solo cuando sea necesario por el registro.