---
lang: es
direction: ltr
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# Chunking de SoraFS → Pipeline de manifiestos

Esta nota recoge los pasos mínimos necesarios para transformar un payload de bytes en
un manifiesto codificado en Norito apto para el pinning en el registro de SoraFS.

1. **Fragmenta el payload de forma determinista**
   - Usa `sorafs_car::CarBuildPlan::single_file` (internamente usa el chunker SF-1)
     para derivar offsets de chunks, longitudes y digests BLAKE3.
   - El plan expone el digest del payload y los metadatos de chunks que el tooling
     downstream puede reutilizar para el ensamblaje de CAR y la planificación de
     Proof-of-Replication.
   - Alternativamente, el prototipo `sorafs_car::ChunkStore` ingiere bytes y
     registra metadatos de chunks deterministas para la construcción posterior de CAR.
     El store ahora deriva el árbol de muestreo PoR de 64 KiB / 4 KiB (etiquetado por dominio,
     alineado a chunks) para que los planificadores puedan solicitar pruebas Merkle sin volver
     a leer el payload.
    Usa `--por-proof=<chunk>:<segment>:<leaf>` para emitir un testigo JSON de una
    hoja muestreada y `--por-json-out` para escribir la instantánea del digest raíz para
    verificación posterior. Combina `--por-proof` con `--por-proof-out=path` para persistir
    el testigo, y usa `--por-proof-verify=path` para confirmar que una prueba existente
    coincide con el `por_root_hex` calculado para el payload actual. Para múltiples
    hojas, `--por-sample=<count>` (con `--por-sample-seed` y
    `--por-sample-out` opcionales) produce muestras deterministas mientras marca
    `por_samples_truncated=true` cuando la solicitud supera las hojas disponibles.
   - Persiste los offsets/longitudes/digests de chunks si tienes intención de construir
     pruebas de bundles (manifiestos CAR, cronogramas PoR).
   - Consulta [`sorafs/chunker_registry.md`](chunker_registry.md) para las
     entradas canónicas del registro y la guía de negociación.

2. **Envuelve un manifiesto**
   - Introduce los metadatos de chunking, el CID raíz, los compromisos CAR, la política de pin,
     claims de alias y firmas de gobernanza en `sorafs_manifest::ManifestBuilder`.
   - Llama a `ManifestV1::encode` para obtener los bytes Norito y
     `ManifestV1::digest` para obtener el digest canónico registrado en el Pin
     Registry.

3. **Publicar**
   - Envía el digest del manifiesto a través de gobernanza (firma del consejo, pruebas de alias)
     y pinea los bytes del manifiesto en SoraFS usando el pipeline determinista.
   - Asegura que el archivo CAR (y el índice CAR opcional) referenciado por el manifiesto se
     almacene en el mismo conjunto de pins de SoraFS.

### Inicio rápido de la CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

El comando imprime digests de chunks y detalles del manifiesto; cuando se suministran
`--manifest-out` y/o `--car-out` escribe el payload Norito y un archivo CARv2 conforme a la
especificación (pragma + header + bloques CARv1 + índice Multihash) en disco. Si pasas
una ruta de directorio, la herramienta la recorre recursivamente (orden lexicográfico),
fragmenta cada archivo y emite un árbol dag-cbor con raíz de directorio cuyo CID aparece
como raíz tanto del manifiesto como del CAR. El reporte JSON incluye el digest calculado
del payload CAR, el digest completo del archivo, el tamaño, el CID bruto y la raíz
(separando el codec dag-cbor), junto con las entradas de alias/metadata del manifiesto.
Usa `--root-cid`/`--dag-codec` para *verificar* la raíz o el codec calculado durante
corridas de CI, `--car-digest` para exigir el hash del payload, `--car-cid` para exigir
un identificador CAR precomputado en bruto (CIDv1, codec `raw`, multihash BLAKE3) y
`--json-out` para persistir el JSON impreso junto a los artefactos de manifiesto/CAR
para la automatización downstream.

Cuando se proporciona `--manifest-signatures-out` (junto con al menos un flag
`--council-signature*`) la herramienta también escribe un sobre
`manifest_signatures.json` que contiene el digest BLAKE3 del manifiesto, el digest
SHA3-256 agregado del plan de chunks (offsets, longitudes y digests BLAKE3 de chunks) y
las firmas del consejo suministradas. El sobre ahora registra el perfil de chunker en
la forma canónica `namespace.name@semver`. La automatización downstream puede publicar el sobre en
los logs de gobernanza o distribuirlo con los artefactos del manifiesto y CAR. Cuando
recibas un sobre de un firmante externo, agrega `--manifest-signatures-in=<path>` para
que la CLI confirme los digests y verifique cada firma Ed25519 contra el digest del
manifiesto recién calculado.

Cuando se registran varios perfiles de chunker puedes seleccionar uno explícitamente
con `--chunker-profile-id=<id>`. El flag mapea a los identificadores numéricos en
[`chunker_registry`](chunker_registry.md) y asegura que tanto la pasada de chunking como
el manifiesto emitido referencien la misma tupla `(namespace, name, semver)`. Prefiere
la forma de handle canónico en la automatización (`--chunker-profile=sorafs.sf1@1.0.0`),
lo que evita codificar IDs numéricos. Ejecuta `sorafs_manifest_chunk_store --list-profiles`
para ver las entradas actuales del registro (la salida refleja el listado provisto por
`sorafs_manifest_chunk_store`), o usa `--promote-profile=<handle>` para exportar el
handle canónico y los metadatos de alias al preparar una actualización del registro.

Los auditores pueden solicitar el árbol completo de Proof-of-Retrievability mediante
`--por-json-out=path`, que serializa los digests de chunk/segment/leaf para la verificación
de muestreo. Los testigos individuales se pueden exportar con
`--por-proof=<chunk>:<segment>:<leaf>` (y validar con `--por-proof-verify=path`), mientras
que `--por-sample=<count>` genera muestras deterministas y sin duplicados para
comprobaciones puntuales.

Cualquier flag que escriba JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`,
etc.) también acepta `-` como ruta, lo que te permite transmitir el payload directamente
a stdout sin crear archivos temporales.

Usa `--chunk-fetch-plan-out=path` para persistir la especificación de fetch de chunks
ordenada (índice de chunk, offset del payload, longitud, digest BLAKE3) que acompaña el
plan del manifiesto. Los clientes multi-fuente pueden alimentar el JSON resultante
directamente en el orquestador de fetch de SoraFS sin volver a leer el payload de origen.
El reporte JSON impreso por la CLI también incluye este array bajo `chunk_fetch_specs`.
Tanto la sección `chunking` como el objeto `manifest` exponen `profile_aliases` junto al
handle `profile` canónico.

Al volver a ejecutar el stub (por ejemplo en CI o en un pipeline de release) puedes
pasar `--plan=chunk_fetch_specs.json` o `--plan=-` para importar la especificación
generada previamente. La CLI verifica que el índice, offset, longitud y digest BLAKE3 de
cada chunk aún coincidan con el plan CAR recién derivado antes de continuar la ingesta,
lo que protege frente a planes obsoletos o manipulados.

### Prueba rápida de orquestación local

El crate `sorafs_car` ahora incluye `sorafs-fetch`, una CLI para desarrolladores que
consume el array `chunk_fetch_specs` y simula la recuperación multi-proveedor desde
archivos locales. Apúntalo al JSON emitido por `--chunk-fetch-plan-out`, proporciona una
o más rutas de payloads de proveedores (opcionalmente con `#N` para aumentar la
concurrencia) y verificará los chunks, reensamblará el payload e imprimirá un reporte
JSON que resume los conteos de éxito/fracaso por proveedor y los recibos por chunk:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

Usa este flujo para validar el comportamiento del orquestador o para comparar payloads de
proveedores antes de conectar transportes de red reales al nodo SoraFS.

Cuando necesites llegar a un gateway Torii en vivo en lugar de archivos locales, sustituye
los flags `--provider=/path` por las nuevas opciones orientadas a HTTP:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

La CLI valida el stream token, impone la alineación de chunker/perfil y registra los
metadatos del gateway junto a los recibos habituales de proveedores para que los operadores
puedan archivar el reporte como evidencia de rollout (consulta el manual de despliegue para
el flujo blue/green completo).

Si pasas `--provider-advert=name=/path/to/advert.to`, la CLI ahora decodifica el sobre Norito,
verifica la firma Ed25519 y exige que el proveedor anuncie la capacidad `chunk_range_fetch`.
Esto mantiene la simulación de fetch multi-fuente alineada con la política de admisión de
gobernanza y evita el uso accidental de proveedores heredados que no pueden satisfacer
solicitudes de chunks por rango.

El sufijo `#N` aumenta el límite de concurrencia del proveedor, mientras que `@W` define su
peso de planificación (por defecto 1 cuando se omite). Cuando se suministran adverts o
descriptores de gateway, la CLI ahora evalúa el scoreboard del orquestador antes de
lanzar un fetch: los proveedores elegibles heredan pesos conscientes de telemetría y la
instantánea JSON persiste en `--scoreboard-out=<path>` cuando se proporciona. Los proveedores
que fallan comprobaciones de capacidad o plazos de gobernanza se descartan automáticamente
con una advertencia para que las ejecuciones sigan alineadas con la política de admisión. Consulta
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` para un par de entrada/salida
de ejemplo.

Pasa `--expect-payload-digest=<hex>` y/o `--expect-payload-len=<bytes>` para afirmar que el
payload ensamblado coincide con las expectativas del manifiesto antes de escribir salidas,
útil para smoke-tests de CI que quieren asegurar que el orquestador no omitió ni reordenó
chunks de forma silenciosa.

Si ya tienes el reporte JSON creado por `sorafs-manifest-stub`, pásalo directamente mediante
`--manifest-report=docs.report.json`. La CLI de fetch reutilizará los campos embebidos
`chunk_fetch_specs`, `payload_digest_hex` y `payload_len`, así que no necesitas gestionar
archivos de plan o validación por separado.

El reporte de fetch también expone telemetría agregada para ayudar al monitoreo:
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
`chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate` y `provider_disabled_total` capturan la salud general de una
sesión de fetch y son adecuados para dashboards de Grafana/Loki o aserciones de CI.
Usa `--provider-metrics-out` para escribir solo el array `provider_reports` si el tooling
downstream solo necesita estadísticas a nivel de proveedor.

### Siguientes pasos

- Captura los metadatos de CAR junto a los digests de manifiesto en los logs de gobernanza
  para que los observadores puedan verificar los contenidos del CAR sin volver a descargar
  el payload.
- Integra el flujo de publicación de manifiestos y CAR en CI para que cada build de docs/artefactos
  produzca automáticamente un manifiesto, obtenga firmas y pinee los payloads resultantes.
