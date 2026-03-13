---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC ici

Norito-RPC Torii API en cours de développement `/v2/pipeline` et les charges utiles HTTP pour les charges utiles Norito Il y a des hachages de schéma et des sommes de contrôle Il s'agit d'un pipeline de pipeline JSON جوابات بوتل نیک بن جائیں۔

## کیوں تبدیل کریں؟
- CRC64 et les hachages de schéma sont utilisés pour le cadrage et les hachages de schéma.
- SDK pour le modèle de données Norito helpers pour le modèle de données et le modèle de données
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز کو ٹَیگ کرتا ہے، اس لئے آپریٹرز فراہم کردہ tableaux de bord کے ذریعے اپنانے کی نگرانی کر سکتے ہیں۔

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Charge utile et codec Norito (`iroha_client`, aides SDK et `norito::to_bytes`) et codec
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. Réponse `Accept: application/x-norito` en réponse à la réponse Norito
4. L'assistant du SDK et la réponse sont également disponibles.Le SDK est disponible en version :
- **Rust** : `iroha_client::Client` pour l'en-tête `Accept` et l'en-tête Norito pour négocier le prix.
- **Python** : `iroha_python.norito_rpc` et `NoritoRpcClient` en cours de développement
- **Android** : SDK Android pour `NoritoRpcClient` et `NoritoRpcRequestOptions` pour la version `NoritoRpcRequestOptions`.
- **JavaScript/Swift** : helpers comme `docs/source/torii/norito_rpc_tracker.md` pour créer un fichier NRPC-3 ou NRPC-3.

## Essayez-le maintenant

Essayez-le maintenant et essayez-le maintenant. Charges utiles Norito pour les charges utiles

1. [پراکسی شروع کریں](./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ widgets جان سکیں کہ ٹریفک کہاں بھیجنی ہے۔
2. Voici comment **Essayez-le** avec `/reference/torii-swagger` pour le point final et avec `POST /v2/pipeline/submit`.
3. **Content-Type** comme `application/x-norito` pour le fichier **Binaire** pour le fichier `fixtures/norito_rpc/transfer_asset.norito` pour le fichier `application/x-norito`. (`fixtures/norito_rpc/transaction_fixtures.manifest.json` est une charge utile)
4. Widget de code de périphérique OAuth pour un jeton de porteur de jeton de porteur (par `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` pour un jeton de code de périphérique OAuth) `X-TryIt-Auth` remplace قبول کرتا ہے)۔
5. Mettre en place une solution pour Torii `fixtures/norito_rpc/schema_hashes.json` et `schema_hash` et `schema_hash` ہے۔ Les hachages sont utilisés pour les hachages et les sauts d'en-tête Norito.Essayez-le maintenant avec `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. یہ اسکرپٹ `cargo xtask norito-rpc-verify` کو ریپ کرتا ہے، JSON خلاصہ `artifacts/norito_rpc/<timestamp>/` میں لکھتا ہے، اور وہی luminaires کرتا ہے جو پورٹل نے استعمال کئے تھے۔

## مسئلہ حل کرنا| علامت | کہاں نظر آتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Réponse Torii | `Content-Type` en-tête | charge utile en cours d'exécution `Content-Type: application/x-norito` en cours d'exécution |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Torii corps de réponse/en-têtes | luminaires et hachage de schéma Torii build pour le projet | `cargo xtask norito-rpc-fixtures` سے luminaires دوبارہ بنائیں اور `fixtures/norito_rpc/schema_hashes.json` میں hash تصدیق کریں؛ Le point de terminaison est Norito, il s'agit d'une solution de secours JSON et d'une solution de secours. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Essayez-le, réponse proxy | درخواست ایسے origin آئی جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں | Origine de l'origine (`https://docs.devnet.sora.example`) env var میں شامل کریں اور پراکسی ری اسٹارٹ کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | Essayez-le, réponse proxy | Pour IP `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` pour la connexion | اندرونی load ٹیسٹ کے لئے حد بڑھائیں یا ونڈو ری سیٹ ہونے تک انتظار کریں (réponse JSON میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) et `{"error":"upstream_error"}` (HTTP 502) | Essayez-le, réponse proxy | Torii Un backend configuré est disponible pour tous | `TRYIT_PROXY_TARGET` Prise en main Torii Prise en main et `TRYIT_PROXY_TIMEOUT_MS` Prise en charge کر دوبارہ کوشش کریں۔ |

Avec les diagnostics Try It et les conseils OAuth [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) میں موجود ہیں۔## اضافی وسائل
- RFC pour les transports : `docs/source/torii/norito_rpc.md`
- Résumé exécutif : `docs/source/torii/norito_rpc_brief.md`
- Suivi des actions : `docs/source/torii/norito_rpc_tracker.md`
- Instructions d'essai du proxy : `docs/portal/docs/devportal/try-it.md`