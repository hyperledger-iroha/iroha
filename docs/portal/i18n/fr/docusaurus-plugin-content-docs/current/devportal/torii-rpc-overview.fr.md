---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ouverture de Norito-RPC

Norito-RPC est le transport binaire pour les API Torii. Il réutilise les mèmes chemins HTTP que `/v1/pipeline` mais échange des charges encadrées par Norito qui incluent des hashes de schéma et des sommes de contrôle. Utilisez-le lorsque vous avez besoin de réponses déterministes et validées ou lorsque les réponses JSON du pipeline deviennent un goulot d'étranglement.

## Pourquoi changer ?
- Un encadrement déterministe avec CRC64 et des hashes de schéma réduit les erreurs de décodage.
- Les helpers Norito partages entre SDKs vous permettent de réutiliser les types existants du modèle de données.
- Torii tague deja les sessions Norito dans la télémétrie, donc les opérateurs peuvent l'adoption avec les tableaux de bord fournis.

## Faire une demande

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Sérialisez votre payload avec le codec Norito (`iroha_client`, helpers SDK ou `norito::to_bytes`).
2. Envoyez la demande avec `Content-Type: application/x-norito`.
3. Demandez une réponse Norito via `Accept: application/x-norito`.
4. Décodez la réponse avec le helper SDK correspondant.Conseils par SDK :
- **Rust** : `iroha_client::Client` négocie Norito automatiquement lorsque vous définissez l'en-tête `Accept`.
- **Python** : utilisez `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android** : utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` dans le SDK Android.
- **JavaScript/Swift** : les helpers sont suivis dans `docs/source/torii/norito_rpc_tracker.md` et arrivent dans NRPC-3.

## Exemple de console Essayez-le

Le développeur de portail fournit un proxy Try It afin que les rélecteurs puissent rejouer des payloads Norito sans écrire de scripts sur mesure.

1. [Demarrez le proxy](./try-it.md#start-the-proxy-locally) et définissez `TRYIT_PROXY_PUBLIC_URL` pour que les widgets s'achètent ou envoient le trafic.
2. Ouvrez la carte **Try it** sur cette page ou le panneau `/reference/torii-swagger` et sélectionnez un point final comme `POST /v1/pipeline/submit`.
3. Passez le **Content-Type** à `application/x-norito`, choisissez l'éditeur **Binary** et chargez `fixtures/norito_rpc/transfer_asset.norito` (ou toute la liste de charges utiles dans `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Fournissez un jeton de porteur via le widget OAuth device-code ou le champ manuel (le proxy accepte les overrides `X-TryIt-Auth` lorsqu'il est configuré avec `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envoyez la demande et vérifiez que Torii renvoie le `schema_hash` liste dans `fixtures/norito_rpc/schema_hashes.json`. Des hashes identiques confirment que l'en-tête Norito a survecu au hop navigateur/proxy.Pour l'evidence roadmap, associez la capture d'écran Try It à une exécution de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Le script encapsule `cargo xtask norito-rpc-verify`, écrit le CV JSON dans `artifacts/norito_rpc/<timestamp>/` et capture les mèmes luminaires que le portail a consomme.

## Dépannage| Symptôme | Ou cela apparaît | Cause probable | Correctif |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Réponse Torii | En-tête `Content-Type` manquant ou incorrect | Définissez `Content-Type: application/x-norito` avant d'envoyer le payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corps/en-têtes de réponse Torii | Le hachage du schéma des appareils diffère du build Torii | Régénérez les luminaires avec `cargo xtask norito-rpc-fixtures` et confirmez le hash dans `fixtures/norito_rpc/schema_hashes.json`; repassez en JSON si l'endpoint n'a pas encore actif Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Réponse du proxy Essayez-le | La demande provient d'une origine non répertoriée dans `TRYIT_PROXY_ALLOWED_ORIGINS` | Ajoutez l'origine du portail (par ex. `https://docs.devnet.sora.example`) à la variable d'environnement et redemarrez le proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Réponse du proxy Essayez-le | Le quota par IP a dépassé le budget `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Augmentez la limite pour des tests de charge interne ou attendez la réinitialisation de la fenêtre (voir `retryAfterMs` dans la réponse JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Réponse du proxy Essayez-le | Torii a expiré ou le proxy n'a pas pu atteindre le backend configure | Vérifiez que `TRYIT_PROXY_TARGET` est accessible, contrôlez la santé de Torii ou réessayez avec un `TRYIT_PROXY_TIMEOUT_MS` plus élevé. |Plus de diagnostics Try It et des conseils OAuth se trouvent dans [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Ressources supplémentaires
-Transport RFC : `docs/source/torii/norito_rpc.md`
- CV exécutif : `docs/source/torii/norito_rpc_brief.md`
- Suivi des actions : `docs/source/torii/norito_rpc_tracker.md`
- Instructions du proxy Try-It : `docs/portal/docs/devportal/try-it.md`