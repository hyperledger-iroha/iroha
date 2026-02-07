---
lang: es
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% carril de alojamiento de contenido
% Iroha Núcleo

# Línea de alojamiento de contenido

La línea de contenido almacena pequeños paquetes estáticos (archivos tar) en la cadena y sirve
archivos individuales directamente desde Torii.

- **Publicar**: enviar `PublishContentBundle` con un archivo tar, caducidad opcional
  altura y un manifiesto opcional. El ID del paquete es el hash blake2b del
  bola de tar. Las entradas de tar deben ser archivos regulares; Los nombres son rutas UTF-8 normalizadas.
  Los límites de tamaño/ruta/recuento de archivos provienen de la configuración `content` (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Los manifiestos incluyen el hash de índice Norito, el espacio de datos/carril y la política de caché.
  (`max_age_seconds`, `immutable`), modo de autenticación (`public` / `role:<role>` /
  `sponsor:<uaid>`), marcador de posición de política de retención y anulaciones de MIME.
- **Deduplicación**: las cargas útiles de tar se fragmentan (64 KB por defecto) y se almacenan una vez por
  hash con recuentos de referencias; Retirar un paquete disminuye y poda trozos.
- **Servir**: Torii expone `GET /v1/content/{bundle}/{path}`. Flujo de respuestas
  directamente desde el almacén de fragmentos con `ETag` = hash de archivo, `Accept-Ranges: bytes`,
  Soporte de rango y control de caché derivado del manifiesto. Lee honrar el
  modo de autenticación manifiesta: las respuestas controladas por roles y patrocinadores requieren canónicas
  encabezados de solicitud (`X-Iroha-Account`, `X-Iroha-Signature`) para el firmado
  cuenta; los paquetes faltantes/vencidos devuelven 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (o `--root <dir>`) ahora
  genera automáticamente un manifiesto, emite `--manifest-out/--bundle-out` opcional y
  acepta `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  y anulaciones `--expires-at-height`. `iroha content pack --root <dir>` compila
  un tarball determinista + manifiesto sin enviar nada.
- **Configuración**: los controles de caché/autenticación se encuentran en `content.*` en `iroha_config`
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) y se aplican en el momento de la publicación.
- **SLO + límites**: `content.max_requests_per_second` / `request_burst` y
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` tapa del lado de lectura
  rendimiento; Torii aplica ambos antes de entregar bytes y exportar
  `torii_content_requests_total`, `torii_content_request_duration_seconds` y
  Métricas `torii_content_response_bytes_total` con etiquetas de resultados. Latencia
  los objetivos viven bajo `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Controles de abuso**: los grupos de tarifas están codificados por UAID/token API/IP remota, y una
  La protección PoW opcional (`content.pow_difficulty_bits`, `content.pow_header`) puede
  ser requerido antes de las lecturas. Los valores predeterminados del diseño de la banda DA provienen de
  `content.stripe_layout` y se repiten en recibos/hashes de manifiesto.
- **Recibos y evidencia DA**: se adjuntan respuestas exitosas
  `sora-content-receipt` (base64 Norito-bytes `ContentDaReceipt` enmarcados) que transportan
  `bundle_id`, `path`, `file_hash`, `served_bytes`, el rango de bytes servidos,
  `chunk_root` / `stripe_layout`, compromiso PDP opcional y una marca de tiempo para
  los clientes pueden fijar lo que se obtuvo sin volver a leer el cuerpo.

Referencias clave:- Modelo de datos: `crates/iroha_data_model/src/content.rs`
- Ejecución: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Controlador Torii: `crates/iroha_torii/src/content.rs`
- Ayudante de CLI: `crates/iroha_cli/src/content.rs`