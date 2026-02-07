---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Fragmentación → Canalización de manifiesto

Este complemento del inicio rápido rastrea el proceso de extremo a extremo que se vuelve crudo
bytes en manifiestos Norito adecuados para el registro de PIN SoraFS. El contenido es
adaptado de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte ese documento para conocer las especificaciones canónicas y el registro de cambios.

## 1. Fragmentar de forma determinista

SoraFS utiliza el perfil SF-1 (`sorafs.sf1@1.0.0`): un rodamiento inspirado en FastCDC
hash con un tamaño de fragmento mínimo de 64 KB, un objetivo de 256 KB, un máximo de 512 KB y un
`0x0000ffff` romper máscara. El perfil está registrado en
`sorafs_manifest::chunker_registry`.

### Ayudantes de óxido

- `sorafs_car::CarBuildPlan::single_file` – Emite desplazamientos de fragmentos, longitudes y
  BLAKE3 hace resúmenes mientras prepara los metadatos del CAR.
- `sorafs_car::ChunkStore`: transmite cargas útiles, conserva metadatos fragmentados y
  deriva el árbol de muestreo de prueba de recuperación (PoR) de 64 KB/4 KB.
- `sorafs_chunker::chunk_bytes_with_digests`: ayudante de biblioteca detrás de ambas CLI.

### Herramientas CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

El JSON contiene los desplazamientos, longitudes y resúmenes de fragmentos ordenados. persistir el
planifique al construir manifiestos o especificaciones de búsqueda del orquestador.

### Testigos del PoR

`ChunkStore` expone `--por-proof=<chunk>:<segment>:<leaf>` y
`--por-sample=<count>` para que los auditores puedan solicitar conjuntos de testigos deterministas. par
esas banderas con `--por-proof-out` o `--por-sample-out` para registrar el JSON.

## 2. Envolver un manifiesto

`ManifestBuilder` combina metadatos fragmentados con archivos adjuntos de gobernanza:

- Compromisos Root CID (dag-cbor) y CAR.
- Pruebas de alias y reclamaciones de capacidad del proveedor.
- Firmas del consejo y metadatos opcionales (por ejemplo, ID de compilación).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Resultados importantes:

- `payload.manifest`: bytes de manifiesto codificados con Norito.
- `payload.report.json` – Resumen legible por humanos/automatización, incluyendo
  `chunk_fetch_specs`, `payload_digest_hex`, resúmenes CAR y metadatos de alias.
- `payload.manifest_signatures.json` – Sobre que contiene el manifiesto BLAKE3
  resumen, resumen SHA3 de plan fragmentado y firmas Ed25519 ordenadas.

Utilice `--manifest-signatures-in` para verificar los sobres suministrados por fuentes externas.
firmantes antes de volver a escribirlos, y `--chunker-profile-id` o
`--chunker-profile=<handle>` para bloquear la selección del registro.

## 3. Publicar y fijar

1. **Presentación de gobernanza**: proporcione el resumen del manifiesto y la firma
   sobre al ayuntamiento para que se admita el pin. Los auditores externos deben
   almacene el resumen SHA3 del plan fragmentado junto con el resumen del manifiesto.
2. **Cargas útiles de PIN**: cargue el archivo CAR (y el índice CAR opcional) al que se hace referencia
   en el manifiesto del Registro Pin. Asegúrese de que el manifiesto y el CAR compartan la
   mismo CID raíz.
3. **Registrar telemetría**: mantenga el informe JSON, los testigos de PoR y cualquier recuperación.
   métricas en artefactos de lanzamiento. Estos registros alimentan los paneles de control del operador y
   Ayude a reproducir problemas sin descargar grandes cargas útiles.

## 4. Simulación de búsqueda de múltiples proveedores

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` aumenta el paralelismo por proveedor (`#4` arriba).
- `@<weight>` sintoniza el sesgo de programación; por defecto es 1.
- `--max-peers=<n>` limita el número de proveedores programados para una ejecución cuando
  El descubrimiento produce más candidatos de los deseados.
- `--expect-payload-digest` and `--expect-payload-len` guard against silent
  corrupción.
- `--provider-advert=name=advert.to` verifica las capacidades del proveedor antes
  utilizándolos en la simulación.
- `--retry-budget=<n>` anula el recuento de reintentos por fragmento (predeterminado: 3), por lo que CI
  Puede hacer surgir regresiones más rápidamente al probar escenarios de falla.

`fetch_report.json` métricas agregadas de superficies (`chunk_retry_total`,
`provider_failure_rate`, etc.) adecuado para afirmaciones y observabilidad de CI.

## 5. Actualizaciones y gobernanza del registro

Al proponer nuevos perfiles de fragmentación:

1. Cree el descriptor en `sorafs_manifest::chunker_registry_data`.
2. Actualice `docs/source/sorafs/chunker_registry.md` y estatutos relacionados.
3. Regenerar accesorios (`export_vectors`) y capturar manifiestos firmados.
4. Presentar el informe de cumplimiento del estatuto con firmas de gobierno.

La automatización debe preferir manijas canónicas (`namespace.name@semver`) y caer