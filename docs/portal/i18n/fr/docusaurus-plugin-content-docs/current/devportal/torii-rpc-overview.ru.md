---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Objet Norito-RPC

Norito-RPC - Transport binaire pour API Torii. Si vous utilisez le port HTTP, c'est-à-dire `/v1/pipeline`, vous ne pouvez pas utiliser Norito avec vos schémas et votre somme de contrôle. Utilisez-le, mais vous avez maintenant des informations de détection et de vérification ou un code JSON ouvre la voie au pipeline.

## Est-ce que tu es prêt à le faire ?
- Déterminez la fréquence de décodage avec CRC64 et vos schémas de décodage.
- L'assistance Norito peut permettre au SDK de résoudre les types de modèles les plus récents.
- Torii peut permettre aux sessions de télémétrie Norito d'être utilisées par les opérateurs lors de la mise en service. dachbords.

## Отправка запроса

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Sélectionnez la charge utile en série à partir du codec Norito (`iroha_client`, assistant SDK ou `norito::to_bytes`).
2. Ouvrez la session avec `Content-Type: application/x-norito`.
3. Sélectionnez Norito avec `Accept: application/x-norito`.
4. Décodez l'aide du SDK.

Recommandations concernant le SDK :
- **Rouille** : `iroha_client::Client` s'applique automatiquement à Norito, en même temps que `Accept`.
- **Python** : utilisez `NoritoRpcClient` ou `iroha_python.norito_rpc`.
- **Android** : utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` dans le SDK Android.
- **JavaScript/Swift** : l'assistance est disponible dans `docs/source/torii/norito_rpc_tracker.md` et est disponible dans les programmes NRPC-3.

## Exemple de console Essayez-leLe portail du robot propose le processus Try It, pour que vous puissiez utiliser la charge utile Norito sans avoir à écrire de scripts supplémentaires.

1. [Ouvrir le processus](./try-it.md#start-the-proxy-locally) et télécharger `TRYIT_PROXY_PUBLIC_URL` pour afficher maintenant le trafic.
2. Ouvrez la carte **Essayez-le** sur cette page ou sur le panneau `/reference/torii-swagger` et sélectionnez le point de terminaison, par exemple `POST /v1/pipeline/submit`.
3. Cliquez sur **Content-Type** sur `application/x-norito`, sélectionnez le rédacteur **Binary** et téléchargez `fixtures/norito_rpc/transfer_asset.norito` (ou charge utile vers `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Téléchargez le jeton du porteur à partir du widget de code de périphérique OAuth ou en utilisant l'option `X-TryIt-Auth`, puis `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`.
5. Ouvrez la porte et vérifiez que Torii correspond à `schema_hash`, en passant par `fixtures/norito_rpc/schema_hashes.json`. Совпадение хешей подтверждает, что заголовок Norito пережил прыжок браузер/прокси.

Pour la feuille de route de recherche, veuillez écrire Try It à l'adresse `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Le script utilise `cargo xtask norito-rpc-verify`, en plaçant le fichier JSON dans `artifacts/norito_rpc/<timestamp>/` et en installant les appareils que vous utilisez sur le portail.

## Устранение неполадок| Symptôme | Где проявляется | Вероятная причина | Mise en œuvre |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Afficher Torii | Ouvrir un magasin ou ne pas le faire `Content-Type` | Installez `Content-Type: application/x-norito` avant d'ouvrir la charge utile. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Téléphones/accessoires pour téléphones Torii | Ces agencements ne sont pas compatibles avec le bâtiment Torii | Пересоздайте luminaires через `cargo xtask norito-rpc-fixtures` и проверьте хеш в `fixtures/norito_rpc/schema_hashes.json` ; Utilisez le repli JSON, si le point de terminaison n'est pas activé Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ Try It прокси | En ce qui concerne l'origine, cela ne correspond pas à `TRYIT_PROXY_ALLOWED_ORIGINS` | Ouvrez le port d'origine (par exemple, `https://docs.devnet.sora.example`) lors de l'ouverture temporaire et du processus de démarrage. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ Try It прокси | Le sujet de l'IP est `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Définissez la limite pour les tests effectués par les utilisateurs ou indiquez la valeur de votre choix (en utilisant `retryAfterMs` dans JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Ответ Try It прокси | Torii n'est pas disponible sur le backend | Vérifiez l'installation du `TRYIT_PROXY_TARGET`, l'installation du Torii ou l'installation du `TRYIT_PROXY_TIMEOUT_MS`. |

Les autres diagnostics Try It et les suggestions d'OAuth sont disponibles dans [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).## Ressources supplémentaires
-Transport RFC : `docs/source/torii/norito_rpc.md`
- Résumé d'utilisation : `docs/source/torii/norito_rpc_brief.md`
- Lieu de voyage : `docs/source/torii/norito_rpc_tracker.md`
- Instructions pour le processus Try-It : `docs/portal/docs/devportal/try-it.md`