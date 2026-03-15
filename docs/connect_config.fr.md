---
lang: fr
direction: ltr
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/connect_config.md (Torii Connect Configuration) -->

## Configuration de Torii Connect

Iroha Torii expose des endpoints WebSocket optionnels de type WalletConnect
ainsi qu’un relay minimal en nœud lorsque le feature Cargo `connect` est
activé (par défaut). Le comportement runtime est piloté par la configuration :

- Définir `connect.enabled=false` pour désactiver toutes les routes Connect
  (`/v2/connect/*`).
- Laisser `true` (valeur par défaut) pour activer les endpoints de session
  WebSocket et `/v2/connect/status`.

Overrides d’environnement (config utilisateur → config effective) :

- `CONNECT_ENABLED` (bool ; par défaut : `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize` ; par défaut : `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize` ; par défaut : `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32` ; par défaut : `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize` ; par défaut : `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize` ; par défaut : `262144`)
- `CONNECT_PING_INTERVAL_MS` (durée ; par défaut : `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32` ; par défaut : `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (durée ; par défaut : `15000`)
- `CONNECT_DEDUPE_CAP` (`usize` ; par défaut : `8192`)
- `CONNECT_RELAY_ENABLED` (bool ; par défaut : `true`)
- `CONNECT_RELAY_STRATEGY` (string ; par défaut : `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8` ; par défaut : `0`)

Notes :

- `CONNECT_SESSION_TTL_MS` et `CONNECT_DEDUPE_TTL_MS` utilisent des littéraux
  de durée dans la configuration utilisateur et sont mappés respectivement
  vers les champs effectifs `session_ttl` et `dedupe_ttl`.
- Le mécanisme de heartbeat borne l’intervalle configuré à un minimum
  pris en charge par les navigateurs (`ping_min_interval_ms`) ; le serveur
  tolère `ping_miss_tolerance` pongs consécutifs manquants avant de fermer le
  WebSocket et d’incrémenter la métrique `connect.ping_miss_total`.
- Lorsque `connect.enabled=false`, les routes WS et de statut Connect ne sont
  pas enregistrées ; les requêtes vers `/v2/connect/ws` et
  `/v2/connect/status` renvoient 404.
- Le serveur exige un `sid` fourni par le client pour
  `/v2/connect/session` (base64url ou hex, 32 octets). Il ne génère plus de
  `sid` de secours.

Voir aussi :
`crates/iroha_config/src/parameters/{user,actual}.rs` et les valeurs par
défaut dans `crates/iroha_config/src/parameters/defaults.rs` (module
`connect`).
