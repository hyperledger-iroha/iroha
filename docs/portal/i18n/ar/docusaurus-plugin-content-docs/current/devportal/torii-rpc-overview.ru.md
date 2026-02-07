---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#النموذج Norito-RPC

Norito-RPC - النقل الثنائي لـ API Torii. وهي تستخدم منفذ HTTP و`/v1/pipeline`، ولكنها تحدد إطار Norito باستخدام المخططات والمجموع الاختباري. استخدم ذاتك عندما تكون هناك إجابة محددة ومثبتة أو عندما يفتح خط أنابيب JSON مكانًا رائعًا.

## هل تم اختيارك؟
- تحديد التردد باستخدام CRC64 ومخططات المخططات توضح فك التشفير.
- يتيح لك مساعد Norito الأساسي - من خلال SDK - الاستفادة من أنواع النماذج المتاحة.
- Torii يمكنك الآن توصيل Norito بأجهزة القياس عن بعد، ويمكن لمشغلي الاتصالات مراقبة الاتصال عبر الإنترنت لوحات جاهزة للشحن.

## تخليص المعاملات

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. قم بتسلسل الحمولة عبر برنامج الترميز Norito (`iroha_client` أو مساعد SDK أو `norito::to_bytes`).
2. قم بإجراء العملية باستخدام `Content-Type: application/x-norito`.
3. قم بالإجابة على Norito باستخدام `Accept: application/x-norito`.
4. فك التشفير من خلال مساعد SDK الرائع.

توصيات بشأن SDK:
- **الصدأ**: `iroha_client::Client` يتوافق تلقائيًا مع Norito، عند إغلاق `Accept`.
- **Python**: استخدم `NoritoRpcClient` من `iroha_python.norito_rpc`.
- **Android**: استخدم `NoritoRpcClient` و`NoritoRpcRequestOptions` في Android SDK.
- **JavaScript/Swift**: المساعد يتنقل إلى `docs/source/torii/norito_rpc_tracker.md` وينتقل إلى NRPC-3.

## وحدات التحكم الأولية جربها

تقوم بوابة المطور بنشر بروكسي Try It حتى تتمكن من الحصول على تخفيضات من Norito payload-بدون تسجيل الدخول السيناريو.

1. [أدخل الوكيل](./try-it.md#start-the-proxy-locally) وأضف `TRYIT_PROXY_PUBLIC_URL` لتتمكن من التحكم في حركة المرور.
2. افتح البطاقة **جربها** على هذا الجانب أو اللوحة `/reference/torii-swagger` واختر نقطة النهاية، على سبيل المثال `POST /v1/pipeline/submit`.
3. قم بإلغاء تحديد **Content-Type** في `application/x-norito`، ثم اختر المحرر **Binary** ثم قم بتأمين `fixtures/norito_rpc/transfer_asset.norito` (أو أي حمولة من `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. قم بتحميل الرمز المميز للحامل من خلال أداة رمز جهاز OAuth أو مباشرة (يتجاوز المبدأ الوكيل `X-TryIt-Auth`، عند `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. قم بالتحقق من الأمر وتأكد من أن Torii يتصل بـ `schema_hash`، الموضح في `fixtures/norito_rpc/schema_hashes.json`. تشير هذه الرسالة إلى أن المفتاح Norito قد تم تثبيته على البروزر/الوكيل.

لتوضيح خارطة الطريق، قم بالنقر على الشاشة جربها باستخدام `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يرمز البرنامج النصي إلى `cargo xtask norito-rpc-verify`، ويسجل رمز JSON في `artifacts/norito_rpc/<timestamp>/`، ويحدد هذه التركيبات التي تستخدم البوابة.

## Устранение неполадок

| العَرَض | هناك فرصة | سبب حقيقي | مهارة |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | الرد على Torii | الرد أو الملاذ الآمن `Content-Type` | قم بتثبيت `Content-Type: application/x-norito` قبل الحمولة الصافية. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Тело/заголовки ответа Torii | لم يتم تضمين هذه المخططات مع Torii | قم بتغيير التركيبات من خلال `cargo xtask norito-rpc-fixtures` وتحقق من ذلك في `fixtures/norito_rpc/schema_hashes.json`؛ استخدم خيار JSON الاحتياطي، إذا لم تتضمن نقطة النهاية Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | الرد على جربه بالوكالة | نبع جديد من أصل ليس في `TRYIT_PROXY_ALLOWED_ORIGINS` | قم بإضافة بوابة الأصل (على سبيل المثال، `https://docs.devnet.sora.example`) في الحماية المؤقتة وإعادة تعيين الوكيل. |
| `{"error":"rate_limited"}` (HTTP 429) | الرد على جربه بالوكالة | كلمة ميزانية IP السابقة `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | ضع حدًا للاختبارات القاسية أو قم بالإجابة عليها (اضغط على `retryAfterMs` في JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) أو `{"error":"upstream_error"}` (HTTP 502) | الرد على جربه بالوكالة | Torii لا يتم توصيل الوقت أو الوكيل إلى الواجهة الخلفية | تحقق من اكتمال `TRYIT_PROXY_TARGET` أو Torii أو قم بالتحقق باستخدام `TRYIT_PROXY_TIMEOUT_MS` الأكبر. |

تأتي معظم بيانات التشخيص ونصائح حول OAuth في [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## الموارد الإضافية
- نقل RFC: `docs/source/torii/norito_rpc.md`
- السيرة الذاتية المجانية: `docs/source/torii/norito_rpc_brief.md`
- مصدر تريكر: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات بروكسي Try-It: `docs/portal/docs/devportal/try-it.md`