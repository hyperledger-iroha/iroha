<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
id: torii-mcp
title: API MCP Torii
description: Guide de référence pour l'utilisation du pont natif Model Context Protocol de Torii.
---

Torii expose un pont MCP (Model Context Protocol) natif à `/v1/mcp`.
Ce point de terminaison permet aux agents de découvrir des outils et d'appeler des routes Torii/Connect via JSON-RPC.

## Forme du point final

- `GET /v1/mcp` renvoie les métadonnées des capacités (non encapsulées JSON-RPC).
- `POST /v1/mcp` accepte les requêtes JSON-RPC 2.0.
- Si `torii.mcp.enabled = false`, aucune des deux routes n'est exposée.
- Si `torii.require_api_token` est activé, le jeton manquant/invalide est rejeté avant l'envoi JSON-RPC.

##Configuration

Activez MCP sous `torii.mcp` :

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Comportement clé :

- `profile` contrôle la visibilité des outils (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` applique une stratégie supplémentaire basée sur le nom.
- `rate_per_minute`/`burst` appliquent une limitation du nombre de jetons pour les requêtes MCP.
- L'état du travail asynchrone de `tools/call_async` est conservé en mémoire à l'aide de `async_job_ttl_secs` et `async_job_max_entries`.

## Flux de clients recommandé

1. Appelez le `initialize`.
2. Appelez `tools/list` et mettez en cache `toolsetVersion`.
3. Utilisez `tools/call` pour les opérations normales.
4. Utilisez `tools/call_async` + `tools/jobs/get` pour des opérations plus longues.
5. Réexécutez `tools/list` lorsque `listChanged` est `true`.

Ne codez pas en dur le catalogue d’outils complet. Découverte au moment de l'exécution.

## Méthodes et sémantique

Méthodes JSON-RPC prises en charge :

-`initialize`
-`tools/list`
-`tools/call`
-`tools/call_batch`
-`tools/call_async`
-`tools/jobs/get`

Remarques :- `tools/list` accepte à la fois `toolset_version` et `toolsetVersion`.
- `tools/jobs/get` accepte à la fois `job_id` et `jobId`.
- `tools/list.cursor` est un décalage de chaîne numérique ; les valeurs non valides reviennent à `0`.
- `tools/call_batch` correspond au meilleur effort par élément (un appel échoué ne fait pas échouer les appels frères et sœurs).
- `tools/call_async` valide immédiatement uniquement la forme de l'enveloppe ; les erreurs d'exécution apparaissent plus tard dans l'état du travail.
- `jsonrpc` doit être `"2.0"` ; `jsonrpc` omis est accepté pour des raisons de compatibilité.

## Authentification et transfert

L'envoi MCP ne contourne pas l'autorisation Torii. Les appels exécutent des gestionnaires de route normaux et des vérifications d'authentification.

Torii transmet les en-têtes liés à l'authentification entrante pour l'envoi des outils :

-`Authorization`
-`x-api-token`
-`x-iroha-account`
-`x-iroha-signature`
-`x-iroha-api-version`

Les clients peuvent également fournir des en-têtes supplémentaires par appel via `arguments.headers`.
`content-length`, `host` et `connection` de `arguments.headers` sont ignorés.

## Modèle d'erreur

Couche HTTP :

- `400` JSON invalide
- Jeton API `403` rejeté avant la gestion JSON-RPC
- La charge utile `413` dépasse `max_request_bytes`
- `429` à débit limité
- `200` pour les réponses JSON-RPC (y compris les erreurs JSON-RPC)

Couche JSON-RPC :- Le niveau supérieur `error.data.error_code` est stable (par exemple `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, `rate_limited`).
- Les défaillances des outils apparaissent sous forme de résultats de l'outil MCP avec `isError = true` et des détails structurés.
- Les échecs de l'outil distribué par route mappent l'état HTTP à `structuredContent.error_code` (par exemple `forbidden`, `not_found`, `server_error`).

## Nommage des outils

Les outils dérivés de OpenAPI utilisent des noms stables basés sur des routes :

-`torii.<method>_<path...>`
- Exemple : `torii.get_v1_accounts`

Les alias sélectionnés sont également exposés sous `iroha.*` et `connect.*`.

## Spécification canonique

Le contrat complet au niveau du fil est conservé dans :

-`crates/iroha_torii/docs/mcp_api.md`

Lorsque le comportement change dans `crates/iroha_torii/src/mcp.rs` ou `crates/iroha_torii/src/lib.rs`,
mettez à jour cette spécification dans la même modification, puis reflètez les conseils d'utilisation des clés ici.