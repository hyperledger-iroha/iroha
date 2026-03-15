---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c27122708469398e96314d4684d3d43d1129a5288bf428ea34e85aed39eb586
source_last_modified: "2026-01-03T18:07:58+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Apercu de Norito-RPC

Norito-RPC est le transport binaire pour les API Torii. Il reutilise les memes chemins HTTP que `/v1/pipeline` mais echange des charges encadrees par Norito qui incluent des hashes de schema et des checksums. Utilisez-le lorsque vous avez besoin de reponses deterministes et validees ou lorsque les reponses JSON du pipeline deviennent un goulot d'etranglement.

## Pourquoi changer?
- Un encadrement deterministe avec CRC64 et des hashes de schema reduit les erreurs de decodage.
- Les helpers Norito partages entre SDKs vous permettent de reutiliser les types existants du modele de donnees.
- Torii tague deja les sessions Norito dans la telemetrie, donc les operateurs peuvent suivre l'adoption avec les dashboards fournis.

## Faire une requete

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serialisez votre payload avec le codec Norito (`iroha_client`, helpers SDK ou `norito::to_bytes`).
2. Envoyez la requete avec `Content-Type: application/x-norito`.
3. Demandez une reponse Norito via `Accept: application/x-norito`.
4. Decodez la reponse avec le helper SDK correspondant.

Conseils par SDK:
- **Rust**: `iroha_client::Client` negocie Norito automatiquement quand vous definissez l'en-tete `Accept`.
- **Python**: utilisez `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` dans le SDK Android.
- **JavaScript/Swift**: les helpers sont suivis dans `docs/source/torii/norito_rpc_tracker.md` et arriveront dans NRPC-3.

## Exemple de console Try It

Le portail developpeur fournit un proxy Try It afin que les relecteurs puissent rejouer des payloads Norito sans ecrire de scripts sur mesure.

1. [Demarrez le proxy](./try-it.md#start-the-proxy-locally) et definissez `TRYIT_PROXY_PUBLIC_URL` pour que les widgets sachent ou envoyer le trafic.
2. Ouvrez la carte **Try it** sur cette page ou le panneau `/reference/torii-swagger` et selectionnez un endpoint comme `POST /v1/pipeline/submit`.
3. Passez le **Content-Type** a `application/x-norito`, choisissez l'editeur **Binary** et chargez `fixtures/norito_rpc/transfer_asset.norito` (ou tout payload liste dans `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Fournissez un bearer token via le widget OAuth device-code ou le champ manuel (le proxy accepte les overrides `X-TryIt-Auth` lorsqu'il est configure avec `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envoyez la requete et verifiez que Torii renvoie le `schema_hash` liste dans `fixtures/norito_rpc/schema_hashes.json`. Des hashes identiques confirment que l'en-tete Norito a survecu au hop navigateur/proxy.

Pour l'evidence roadmap, associez la capture d'ecran Try It a une execution de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Le script encapsule `cargo xtask norito-rpc-verify`, ecrit le resume JSON dans `artifacts/norito_rpc/<timestamp>/` et capture les memes fixtures que le portail a consommes.

## Depannage

| Symptome | Ou cela apparait | Cause probable | Correctif |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Reponse Torii | En-tete `Content-Type` manquant ou incorrect | Definissez `Content-Type: application/x-norito` avant d'envoyer le payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Corps/en-tetes de reponse Torii | Le hash de schema des fixtures differe du build Torii | Regenerez les fixtures avec `cargo xtask norito-rpc-fixtures` et confirmez le hash dans `fixtures/norito_rpc/schema_hashes.json`; repassez en JSON si l'endpoint n'a pas encore active Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Reponse du proxy Try It | La requete provient d'une origine non listee dans `TRYIT_PROXY_ALLOWED_ORIGINS` | Ajoutez l'origine du portail (par ex. `https://docs.devnet.sora.example`) a la variable d'environnement et redemarrez le proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Reponse du proxy Try It | Le quota par IP a depasse le budget `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Augmentez la limite pour des tests de charge internes ou attendez la reinitialisation de la fenetre (voir `retryAfterMs` dans la reponse JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Reponse du proxy Try It | Torii a expire ou le proxy n'a pas pu atteindre le backend configure | Verifiez que `TRYIT_PROXY_TARGET` est accessible, controlez la sante de Torii ou reessayez avec un `TRYIT_PROXY_TIMEOUT_MS` plus eleve. |

Plus de diagnostics Try It et des conseils OAuth se trouvent dans [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Ressources supplementaires
- RFC transport: `docs/source/torii/norito_rpc.md`
- Resume executif: `docs/source/torii/norito_rpc_brief.md`
- Action tracker: `docs/source/torii/norito_rpc_tracker.md`
- Instructions du proxy Try-It: `docs/portal/docs/devportal/try-it.md`
