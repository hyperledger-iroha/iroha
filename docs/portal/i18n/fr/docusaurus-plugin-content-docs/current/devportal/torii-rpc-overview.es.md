---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Résumé de Norito-RPC

Norito-RPC est le transport binaire pour les API de Torii. Réutilisez les mêmes routes HTTP que `/v2/pipeline` mais intercambia charge des marques par Norito qui incluent des hachages de schéma et des sommes de contrôle. Utilisez-le lorsque vous avez besoin de réponses déterminées et validées ou lorsque les réponses JSON du pipeline sont vues dans un boîtier de bouteille.

## Pourquoi changer ?
- Le marquage déterministe avec CRC64 et les hachages d'esquema réduisent les erreurs de décodification.
- Les assistants Norito partagés entre les SDK vous permettent de réutiliser des types existants du modèle de données.
- Torii est l'étiquette des sessions Norito en télémétrie, car les opérateurs peuvent surveiller l'adoption avec les panneaux fournis.

## Comment faire une demande

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Sérialisez votre charge utile avec le codec Norito (`iroha_client`, assistants du SDK ou `norito::to_bytes`).
2. Envoyez la sollicitude avec `Content-Type: application/x-norito`.
3. Sollicitez une réponse Norito en utilisant `Accept: application/x-norito`.
4. Décodez la réponse avec l'assistant correspondant du SDK.Guide du SDK :
- **Rust** : `iroha_client::Client` négocie Norito automatiquement lorsque l'en-tête `Accept` est activé.
- **Python** : utilise `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android** : utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` dans le SDK d'Android.
- **JavaScript/Swift** : les helpers sont basés sur `docs/source/torii/norito_rpc_tracker.md` et sont ajoutés comme partie de NRPC-3.

## Exemple de console Essayez-le

Le portail des développeurs inclut un proxy Try It pour que les réviseurs puissent reproduire les charges utiles Norito sans écrire de scripts à moyen.

1. [Inicia el proxy](./try-it.md#start-the-proxy-locally) et définissez `TRYIT_PROXY_PUBLIC_URL` pour que les widgets se séparent de celui qui envoie le trafic.
2. Ouvrez la page **Essayez-le** sur cette page du panneau `/reference/torii-swagger` et sélectionnez un point de terminaison comme `POST /v2/pipeline/submit`.
3. Passez le **Content-Type** à `application/x-norito`, sélectionnez l'éditeur **Binary** et remplacez `fixtures/norito_rpc/transfer_asset.norito` (ou toute charge utile répertoriée dans `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Proposez un jeton de porteur via le code de périphérique OAuth du widget ou le champ du manuel du jeton (le proxy accepté remplace `X-TryIt-Auth` lorsqu'il est configuré avec `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envoyez la demande et vérifiez que Torii reflète le `schema_hash` répertorié dans `fixtures/norito_rpc/schema_hashes.json`. Les hachages coïncident confirment que le code Norito est activé sur le saut du navigateur/proxy.Pour prouver la feuille de route, combinez la capture d'écran de Try It avec une exécution de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Le script enveloppe `cargo xtask norito-rpc-verify`, écris le résumé JSON en `artifacts/norito_rpc/<timestamp>/` et capture les différents appareils qui consomment le portail.

## Résolution des problèmes| Sintome | Donde apparec | Cause probable | Solution |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Réponse de Torii | Une erreur ou une erreur dans l'en-tête `Content-Type` | Définissez `Content-Type: application/x-norito` avant d’envoyer la charge utile. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corps/en-têtes de réponse de Torii | Le hachage de l'esquema de luminaires diffère de la build de Torii | Régénérez les appareils avec `cargo xtask norito-rpc-fixtures` et confirmez le hachage en `fixtures/norito_rpc/schema_hashes.json` ; Utilisez JSON de secours si le point de terminaison n'a pas la capacité Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Réponse du proxy Essayez-le | La demande provient d'une origine qui n'est pas répertoriée dans `TRYIT_PROXY_ALLOWED_ORIGINS` | Ajoutez l'origine du portail (par exemple, `https://docs.devnet.sora.example`) à la variable d'entrée et réinitialisez le proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Réponse du proxy Essayez-le | La case IP dépasse le présupposé de `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Augmentez la limite pour tester les charges internes ou attendez que la fenêtre se rétablisse (consultez `retryAfterMs` dans la réponse JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Réponse du proxy Essayez-le | Torii il y a quelques jours ou le proxy ne peut pas accéder au backend configuré | Vérifiez que `TRYIT_PROXY_TARGET` est accessible à la mer, révisez la santé de Torii ou réessayez avec un maire `TRYIT_PROXY_TIMEOUT_MS`. |Les diagnostics de Try It et les conseils d'OAuth sont présents dans [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Ressources supplémentaires
- RFC de transport : `docs/source/torii/norito_rpc.md`
- CV exécutif : `docs/source/torii/norito_rpc_brief.md`
- Tracker d'actions : `docs/source/torii/norito_rpc_tracker.md`
- Instructions pour le proxy Try-It : `docs/portal/docs/devportal/try-it.md`