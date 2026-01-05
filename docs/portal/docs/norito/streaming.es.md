---
lang: es
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

Norito Streaming define el formato en la red, los frames de control y el codec de referencia usados para flujos de medios en vivo a traves de Torii y SoraNet. La especificacion canonica vive en `norito_streaming.md` en la raiz del workspace; esta pagina destila las piezas que necesitan los operadores y los autores de SDK junto con los puntos de configuracion.

## Formato de wire y plano de control

- **Manifests y frames.** `ManifestV1` y `PrivacyRoute*` describen la linea de tiempo de segmentos, los descriptores de chunks y las pistas de ruta. Los frames de control (`KeyUpdate`, `ContentKeyUpdate` y el feedback de cadencia) viven junto al manifest para que los viewers puedan validar commitments antes de decodificar.
- **Codec base.** `BaselineEncoder`/`BaselineDecoder` fuerzan ids de chunk monotonos, aritmetica de timestamps y verificacion de commitments. Los hosts deben llamar a `EncodedSegment::verify_manifest` antes de servir a viewers o relays.
- **Bits de feature.** La negociacion de capacidades anuncia `streaming.feature_bits` (por defecto `0b11` = baseline feedback + proveedor de rutas de privacidad) para que relays y clientes rechacen peers sin capacidades coincidentes de forma determinista.

## Claves, suites y cadencia

- **Requisitos de identidad.** Los frames de control de streaming siempre se firman con Ed25519. Se pueden suministrar claves dedicadas mediante `streaming.identity_public_key`/`streaming.identity_private_key`; de lo contrario se reutiliza la identidad del nodo.
- **Suites HPKE.** `KeyUpdate` selecciona la suite comun mas baja; la suite #1 es obligatoria (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), con una ruta de actualizacion opcional a `Kyber1024`. La seleccion de la suite se almacena en la sesion y se valida en cada actualizacion.
- **Rotacion.** Los publishers emiten un `KeyUpdate` firmado cada 64 MiB o 5 minutos. `key_counter` debe aumentar estrictamente; una regresion es un error critico. `ContentKeyUpdate` distribuye la Group Content Key rotatoria, envuelta bajo la suite HPKE negociada, y limita el descifrado de segmentos por ID + ventana de validez.
- **Snapshots.** `StreamingSession::snapshot_state` y `restore_from_snapshot` persisten `{session_id, key_counter, suite, sts_root, cadence state}` bajo `streaming.session_store_dir` (por defecto `./storage/streaming`). Las claves de transporte se re-derivan al restaurar para que los fallos no filtren secretos de sesion.

## Configuracion de runtime

- **Material de claves.** Proporciona claves dedicadas con `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) y material Kyber opcional via `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Las cuatro deben estar presentes al sobrescribir los valores por defecto; `streaming.kyber_suite` acepta `mlkem512|mlkem768|mlkem1024` (alias `kyber512/768/1024`, por defecto `mlkem768`).
- **Guardrails del codec.** CABAC permanece deshabilitado a menos que el build lo habilite; rANS empaquetado requiere `ENABLE_RANS_BUNDLES=1`. Se impone via `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` y el opcional `streaming.codec.rans_tables_path` cuando se proveen tablas personalizadas. El `bundle_width` empaquetado debe estar entre 2 y 3 (inclusive); ancho 1 no se admite.
- **Rutas SoraNet.** `streaming.soranet.*` controla el transporte anonimo: `exit_multiaddr` (por defecto `/dns/torii/udp/9443/quic`), `padding_budget_ms` (por defecto 25 ms), `access_kind` (`authenticated` vs `read-only`), `channel_salt` opcional y `provision_spool_dir` (por defecto `./storage/streaming/soranet_routes`).
- **Gate de sincronizacion.** `streaming.sync` habilita el control de drift para streams audiovisuales: `enabled`, `observe_only`, `ewma_threshold_ms` y `hard_cap_ms` gobiernan cuando se rechazan segmentos por deriva temporal.

## Validacion y fixtures

- Las definiciones canonicas de tipos y helpers viven en `crates/iroha_crypto/src/streaming.rs`.
- La cobertura de integracion ejerce el handshake HPKE, la distribucion de content-key y el ciclo de vida de snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). Ejecuta `cargo test -p iroha_crypto streaming_handshake` para verificar la superficie de streaming localmente.
- Para un analisis profundo de layout, manejo de errores y futuras actualizaciones, lee `norito_streaming.md` en la raiz del repositorio.
