---
lang: fr
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Référence de configuration de l'API client

Ce document suit les boutons de configuration côté client Torii qui sont
surfaces via `iroha_config::parameters::user::Torii`. La rubrique ci-dessous
se concentre sur les contrôles de transport Norito-RPC introduits pour NRPC-1 ; avenir
les paramètres de l'API client doivent étendre ce fichier.

### `torii.transport.norito_rpc`

| Clé | Tapez | Par défaut | Descriptif |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Commutateur principal qui permet le décodage binaire Norito. Lorsque `false`, Torii rejette chaque requête Norito-RPC avec `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | Niveau de déploiement : `disabled`, `canary` ou `ga`. Les étapes déterminent les décisions d’admission et la sortie `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | Applique mTLS pour le transport Norito-RPC : lorsque `true`, Torii rejette les requêtes Norito-RPC qui ne comportent pas d'en-tête de marqueur mTLS (par exemple, `X-Forwarded-Client-Cert`). L'indicateur apparaît via `/rpc/capabilities` afin que les SDK puissent avertir en cas d'environnements mal configurés. |
| `allowed_clients` | `array<string>` | `[]` | Liste verte des Canaries. Lorsque `stage = "canary"`, seules les requêtes portant un en-tête `X-API-Token` présent dans cette liste sont acceptées. |

Exemple de configuration :

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Sémantique de l'étape :

- **désactivé** — Norito-RPC n'est pas disponible même si `enabled = true`. Clientèle
  recevez `403 norito_rpc_disabled`.
- **canary** — Les requêtes doivent inclure un en-tête `X-API-Token` qui correspond à celui-ci.
  du `allowed_clients`. Toutes les autres demandes reçoivent `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC est disponible pour chaque appelant authentifié (sous réserve des
  taux habituel et limites de préautorisation).

Les opérateurs peuvent mettre à jour ces valeurs de manière dynamique via `/v2/config`. Chaque changement
se reflète immédiatement dans `/rpc/capabilities`, permettant les SDK et l'observabilité
des tableaux de bord pour montrer la posture du transport en direct.