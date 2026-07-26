---
lang: es
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Génesis Bootstrap de pares confiables

Los pares Iroha sin un `genesis.file` local pueden recuperar un bloque de génesis firmado de pares confiables
utilizando el protocolo de arranque codificado Norito.

- **Protocolo:** los pares intercambian `GenesisRequest` (`Preflight` para metadatos, `Fetch` para carga útil) y
  Marcos `GenesisResponse` codificados por `request_id`. Los respondedores incluyen la identificación de la cadena, la clave pública del firmante,
  hash y una sugerencia de tamaño opcional; las cargas útiles se devuelven solo en `Fetch` y los ID de solicitud duplicados
  recibir `DuplicateRequest`.
- **Guardias:** los respondedores imponen una lista de permitidos (`genesis.bootstrap_allowlist` o los pares de confianza
  conjunto), coincidencia de ID de cadena/clave pública/hash, límites de velocidad (`genesis.bootstrap_response_throttle`) y un
  tapa de tamaño (`genesis.bootstrap_max_bytes`). Las solicitudes fuera de la lista de permitidos reciben `NotAllowed`, y
  las cargas útiles firmadas con la clave incorrecta reciben `MismatchedPubkey`.
- **Flujo de solicitante:** cuando el almacenamiento está vacío y `genesis.file` no está configurado (y
  `genesis.bootstrap_enabled=true`), el nodo realiza comprobaciones previas de pares confiables con el opcional
  `genesis.expected_hash`, luego recupera la carga útil, valida las firmas a través de `validate_genesis_block`,
  y persiste `genesis.bootstrap.nrt` junto a Kura antes de aplicar el bloqueo. Reintentos de arranque
  honor `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` y
  `genesis.bootstrap_max_attempts`.
- **Modos de error:** las solicitudes se rechazan por errores en la lista de permitidos, discrepancias de cadena/pubkey/hash, tamaño
  infracciones de límites, límites de tarifas, falta de génesis local o identificadores de solicitud duplicados. hashes conflictivos
  entre pares abortar la búsqueda; ningún respondedor/tiempo de espera recurre a la configuración local.
- **Pasos del operador:** asegúrese de que se pueda acceder al menos a un par confiable con una génesis válida, configure
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` y las perillas de reintento, y
  Opcionalmente, fije `expected_hash` para evitar aceptar cargas útiles que no coincidan. Las cargas útiles persistentes pueden ser
  reutilizado en arranques posteriores apuntando `genesis.file` a `genesis.bootstrap.nrt`.