---
lang: es
direction: ltr
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_config.md (Torii Connect Configuration) -->

## Configuración de Torii Connect

Iroha Torii expone endpoints WebSocket opcionales de estilo WalletConnect y un
relay mínimo dentro del nodo cuando la feature Cargo `connect` está activada
(`default`). El comportamiento en tiempo de ejecución se controla por
configuración:

- Establece `connect.enabled=false` para deshabilitar todas las rutas Connect
  (`/v2/connect/*`).
- Déjalo en `true` (por defecto) para habilitar los endpoints de sesión WS y
  `/v2/connect/status`.

Overrides de entorno (config usuario → config efectiva):

- `CONNECT_ENABLED` (bool; por defecto: `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize`; por defecto: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize`; por defecto: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32`; por defecto: `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize`; por defecto: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize`; por defecto: `262144`)
- `CONNECT_PING_INTERVAL_MS` (duración; por defecto: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32`; por defecto: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (duración; por defecto: `15000`)
- `CONNECT_DEDUPE_CAP` (`usize`; por defecto: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; por defecto: `true`)
- `CONNECT_RELAY_STRATEGY` (string; por defecto: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8`; por defecto: `0`)

Notas:

- `CONNECT_SESSION_TTL_MS` y `CONNECT_DEDUPE_TTL_MS` usan literales de
  duración en la configuración de usuario y se mapean a los campos efectivos
  `session_ttl` y `dedupe_ttl`.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` deshabilita el límite de sesiones por IP.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` deshabilita el limitador de tasa de handshake por IP.
- La lógica de heartbeat limita el intervalo configurado al mínimo aceptable
  para navegadores (`ping_min_interval_ms`); el servidor tolera
  `ping_miss_tolerance` pongs consecutivos perdidos antes de cerrar el
  WebSocket e incrementar la métrica `connect.ping_miss_total`.
- Cuando se deshabilita en tiempo de ejecución (`connect.enabled=false`), las
  rutas WS y de estado de Connect no se registran; las peticiones a
  `/v2/connect/ws` y `/v2/connect/status` devuelven 404.
- El servidor requiere un `sid` proporcionado por el cliente en
  `/v2/connect/session` (base64url o hex, 32 bytes). Ya no genera un `sid`
  de fallback.

Consulta también:
`crates/iroha_config/src/parameters/{user,actual}.rs` y los valores por
defecto en `crates/iroha_config/src/parameters/defaults.rs` (módulo
`connect`).
