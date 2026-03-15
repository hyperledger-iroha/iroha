---
lang: pt
direction: ltr
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/connect_config.md (Torii Connect Configuration) -->

## Configuração do Torii Connect

O Iroha Torii expõe endpoints WebSocket opcionais no estilo WalletConnect e um
relay mínimo dentro do nó quando o feature Cargo `connect` está habilitado
(`default`). O comportamento em tempo de execução é controlado via
configuração:

- Defina `connect.enabled=false` para desativar todas as rotas Connect
  (`/v2/connect/*`).
- Deixe como `true` (padrão) para habilitar os endpoints de sessão WS e
  `/v2/connect/status`.

Overrides de ambiente (config do usuário → config efetiva):

- `CONNECT_ENABLED` (bool; padrão: `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize`; padrão: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize`; padrão: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32`; padrão: `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize`; padrão: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize`; padrão: `262144`)
- `CONNECT_PING_INTERVAL_MS` (duração; padrão: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32`; padrão: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (duração; padrão: `15000`)
- `CONNECT_DEDUPE_CAP` (`usize`; padrão: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; padrão: `true`)
- `CONNECT_RELAY_STRATEGY` (string; padrão: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8`; padrão: `0`)

Notas:

- `CONNECT_SESSION_TTL_MS` e `CONNECT_DEDUPE_TTL_MS` usam literais de duração
  na configuração do usuário e são mapeados para os campos efetivos
  `session_ttl` e `dedupe_ttl`.
- A aplicação do heartbeat limita o intervalo configurado ao mínimo amigável
  para navegador (`ping_min_interval_ms`); o servidor tolera
  `ping_miss_tolerance` pings sem resposta (pongs perdidos) antes de fechar o
  WebSocket e incrementar a métrica `connect.ping_miss_total`.
- Quando desativado em runtime (`connect.enabled=false`), as rotas WS e de
  status do Connect não são registradas; requisições para
  `/v2/connect/ws` e `/v2/connect/status` retornam 404.
- O servidor exige um `sid` fornecido pelo cliente em
  `/v2/connect/session` (base64url ou hex, 32 bytes). Ele não gera mais um
  `sid` de fallback.

Ver também:
`crates/iroha_config/src/parameters/{user,actual}.rs` e os defaults em
`crates/iroha_config/src/parameters/defaults.rs` (módulo `connect`).

