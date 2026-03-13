---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visa général du Norito-RPC

Norito-RPC et le système de transport binaire pour les API Torii. Il réutilise nos chemins HTTP de `/v2/pipeline` mais utilise des charges utiles émulées par Norito qui incluent des hachages de schéma et des sommes de contrôle. Utilisez lorsque vous précisez les réponses déterministes et validées ou lorsque les réponses JSON font que le pipeline devient un gargalo.

## Pourquoi changer ?
- Un quadramento déterministe avec CRC64 et les hachages de schéma réduisent les erreurs de décodage.
- Les Helpers Norito partagés entre les SDK permettent de réutiliser les types existants du modèle de données.
- Torii et la marque Norito pour la télémétrie, alors les opérateurs peuvent accompagner l'avocat des tableaux de bord fournis.

## Fazendo une condition requise

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Sérialisez votre charge utile avec le codec Norito (`iroha_client`, les assistants du SDK ou `norito::to_bytes`).
2. Envie d'une demande avec `Content-Type: application/x-norito`.
3. Sollicitez une réponse Norito en utilisant `Accept: application/x-norito`.
4. Décodez la réponse de l'assistant du correspondant SDK.

Guide du SDK :
- **Rust** : `iroha_client::Client` négocie automatiquement Norito lorsque vous définissez l'en-tête `Accept`.
- **Python** : utilisez `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android** : utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` sans SDK Android.
- **JavaScript/Swift** : les assistants sont rastrés sur `docs/source/torii/norito_rpc_tracker.md` et transférés comme partie du NRPC-3.## Exemple de console Essayez-le

Le portail de développement inclut un proxy. Essayez-le pour réviser les charges utiles Norito avec des scripts d'écriture à moyen terme.

1. [Démarrer le proxy](./try-it.md#start-the-proxy-locally) et définir `TRYIT_PROXY_PUBLIC_URL` pour que les widgets saibam pour l'envoi ou le trafic.
2. Ouvrez la carte **Essayez-la** sur la page ou sur la page `/reference/torii-swagger` et sélectionnez un point de terminaison comme `POST /v2/pipeline/submit`.
3. Choisissez **Content-Type** pour `application/x-norito`, recherchez l'éditeur **Binary** et envoyez `fixtures/norito_rpc/transfer_asset.norito` (ou quelle charge utile est répertoriée dans `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Obtenez un jeton de porteur via le code de périphérique OAuth du widget ou le manuel du champ (le proxy remplace `X-TryIt-Auth` lorsqu'il est configuré avec `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envoyez une demande et vérifiez si Torii ou `schema_hash` sont répertoriés dans `fixtures/norito_rpc/schema_hashes.json`. Les hachages confirment que le fichier Norito est activé vers le navigateur/proxy.

Pour prouver la feuille de route, combinez la capture du tissu Try It avec l'exécution de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Le script implique `cargo xtask norito-rpc-verify`, écrivez le résumé JSON dans `artifacts/norito_rpc/<timestamp>/` et capturez vos appareils consommés sur le portail.

## Résolution des problèmes| Sintome | Onde apparaît | Cause provavel | Correção |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Réponse du Torii | En-tête `Content-Type` ausente ou incorreto | Définissez `Content-Type: application/x-norito` avant d'envoyer la charge utile. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corps/en-têtes de réponse du Torii | Le hachage du schéma des appareils est différent de celui de la construction Torii | Régénérez les appareils avec `cargo xtask norito-rpc-fixtures` et confirmez ou hachez-les `fixtures/norito_rpc/schema_hashes.json` ; utilisez JSON de secours pour le point de terminaison et n'avez donc pas la capacité Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Réponse du proxy Essayez-le | La demande se voit d'un origem nao listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Ajoutez l'original du portail (par exemple, `https://docs.devnet.sora.example`) à la variable ambiante et réinitialisez le proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Réponse du proxy Essayez-le | La cote par IP dépasse le budget `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Augmentez la limite pour les tests internes de charge ou attendez un nouveau redémarrage (voir `retryAfterMs` en réponse JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Réponse du proxy Essayez-le | Torii expire ou le proxy ne permet pas de configurer le backend | Vérifiez que `TRYIT_PROXY_TARGET` est acessivel, confira a saude do Torii ou tentez récemment avec un `TRYIT_PROXY_TIMEOUT_MS` majeur. |

Plus de diagnostics de Try It et de OAuth ficam em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).## Ressources supplémentaires
- RFC de transport : `docs/source/torii/norito_rpc.md`
- CV exécutif : `docs/source/torii/norito_rpc_brief.md`
- Tracker de gaz : `docs/source/torii/norito_rpc_tracker.md`
- Instructions pour l'essai de proxy : `docs/portal/docs/devportal/try-it.md`