---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito-RPC

Norito-RPC et Torii. Vous pouvez utiliser HTTP pour `/v1/pipeline` et utiliser les hachages Norito. للمخطط et sommes de contrôle. Il s'agit d'une solution de gestion de pipeline JSON pour le pipeline زجاجة.

## لماذا ننتقل؟
- L'utilisation du CRC64 et des hachages est également possible.
- Les kits SDK Norito sont également compatibles avec les SDK.
- Torii pour le téléphone Norito pour le téléphone portable لوحات المراقبة المتاحة.

## تنفيذ طلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Utilisez le codec Norito (`iroha_client`, SDK et `norito::to_bytes`).
2. ارسل الطلب مع `Content-Type: application/x-norito`.
3. Remplacez le Norito par le `Accept: application/x-norito`.
4. Utilisez le SDK pour installer le SDK.

Utilisation du SDK :
- **Rouille** : `iroha_client::Client` يتفاوض على Norito تلقائيا عندما تضبط ترويسة `Accept`.
- **Python** : remplace `NoritoRpcClient` par `iroha_python.norito_rpc`.
- **Android** : utilisez `NoritoRpcClient` et `NoritoRpcRequestOptions` pour le SDK Android.
- **JavaScript/Swift** : éléments pris en charge par `docs/source/torii/norito_rpc_tracker.md` et NRPC-3.

## مثال وحدة Essayez-le

يوفر بوابة المطورين وكيل Try It كي يتمكن المراجعون من اعادة تشغيل حمولات Norito دون كتابة نصوص مخصصة.1. [ابدأ الوكيل](./try-it.md#start-the-proxy-locally) et `TRYIT_PROXY_PUBLIC_URL` حتى تعرف الادوات المصغرة اين ترسل الحركة.
2. Essayez **Essayez-le** en utilisant `/reference/torii-swagger` et `POST /v1/pipeline/submit`.
3. Utilisez **Content-Type** pour `application/x-norito`, pour utiliser **Binary**, pour `fixtures/norito_rpc/transfer_asset.norito` (ou pour `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Le jeton du porteur contient le code de périphérique OAuth et le code de périphérique (`X-TryIt-Auth` ou `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Mettre en œuvre le Torii et le `schema_hash` `fixtures/norito_rpc/schema_hashes.json`. تطابق القيم يؤكد ان راس Norito نجا من قفزة المتصفح/الوكيل.

لادلة خارطة الطريق، ارفق لقطة شاشة Essayez-le avec `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Il s'agit d'un `cargo xtask norito-rpc-verify` et d'un JSON `artifacts/norito_rpc/<timestamp>/` pour tous les appareils.

## استكشاف الاخطاء| العرض | مكان الظهور | السبب المحتمل | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | استجابة Torii | ترويسة `Content-Type` مفقودة او غير صحيحة | اضف `Content-Type: application/x-norito` قبل ارسال الحمولة. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | جسم/ترويسات استجابة Torii | hash مخطط luminaires يختلف عن build Torii | اعادة توليد luminaires عبر `cargo xtask norito-rpc-fixtures` et hash في `fixtures/norito_rpc/schema_hashes.json`؛ Utilisez JSON de secours pour utiliser Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | استجابة وكيل Essayez-le | جاء الطلب من اصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | اضف اصل البوابة (مثلا `https://docs.devnet.sora.example`) الى متغير البيئة واعادة تشغيل الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | استجابة وكيل Essayez-le | Paramètres IP pour IP `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Vous pouvez utiliser la fonction `retryAfterMs` pour utiliser JSON. |
| `{"error":"upstream_timeout"}` (HTTP 504) et `{"error":"upstream_error"}` (HTTP 502) | استجابة وكيل Essayez-le | Torii Câbles d'alimentation et câbles d'alimentation | Prenez le `TRYIT_PROXY_TARGET` et le Torii et le `TRYIT_PROXY_TIMEOUT_MS`. |

Vous pouvez également essayer Try It et OAuth par [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## موارد اضافية
- RFC Titre : `docs/source/torii/norito_rpc.md`
- Nom de la personne: `docs/source/torii/norito_rpc_brief.md`
- Nom du produit : `docs/source/torii/norito_rpc_tracker.md`
- Télécharger pour Try-It : `docs/portal/docs/devportal/try-it.md`