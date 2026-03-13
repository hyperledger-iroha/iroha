---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Norito-RPC جائزه

Norito-RPC Torii APIs لـ باينر ٹرانسپورٹ. يتم استخدام `/v2/pipeline` وHTTP كرتا ورمز Norito لإطار الحمولات النافعة لتبادل الكرتا واستخدام تجزئات المخطط والمجاميع الاختبارية التي تتضمنها. تم استخدام هذا التطبيق بشكل ممتع وتحقق من الإجابات المضمنة، أو من خلال خط أنابيب JSON جوابات بوتل.

## کیوں تبديل کریں؟
- CRC64 وتجزئات المخطط هي بمثابة تأطير مفيد لتصحيح الأخطاء.
- أدوات تطوير البرمجيات (SDKs) هي عبارة عن أدوات مساعدة Norito ونماذج بيانات متاحة وأقسام قابلة للاستخدام مرة أخرى.
- Torii أعلى وأسفل لوحة القيادة Norito، وهي عبارة عن إطار من لوحات المعلومات المحدثة هذه هي المشكلة.

## درخواست بھيجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. الحمولة النافعة لبرنامج الترميز Norito (`iroha_client`، مساعدات SDK، أو `norito::to_bytes`) ممتازة.
2. تم إلغاء `Content-Type: application/x-norito`.
3. `Accept: application/x-norito` تم الرد عليه Norito.
4. يتعلق مساعد SDK بالاستجابة السريعة لكل شيء.

حزمة SDK لحاظ الصورة:
- **الصدأ**: `iroha_client::Client` إلى `Accept` رأس البطاقة إلى Norito للتفاوض على البطاقة.
- **Python**: استخدام `iroha_python.norito_rpc` `NoritoRpcClient`.
- **Android**: يتم استخدام Android SDK `NoritoRpcClient` و`NoritoRpcRequestOptions`.
- **JavaScript/Swift**: يتضمن المساعدون `docs/source/torii/norito_rpc_tracker.md` جاتا و NRPC-3 الموجود حاليًا.

## جربه كمثال

ويلبر بورت هو برنامج Try It العملي للحصول على ريوورز، وهو خصوصية خاصة للحمولات Norito وإعادة كل شيء.

1. [بدء التشغيل العملي](./try-it.md#start-the-proxy-locally) و`TRYIT_PROXY_PUBLIC_URL` يحتويان على عناصر واجهة المستخدم الخاصة بالبرنامج والتي تم إنشاؤها بالفعل.
2. هذه الصفحة **جربها** البطاقة أو `/reference/torii-swagger` بطاقة الذاكرة و`POST /v2/pipeline/submit` هي نقطة النهاية المنتخبة.
3. **نوع المحتوى** الذي يتم تنزيله من خلال `application/x-norito` و **Binary** كخيار اختياري و`fixtures/norito_rpc/transfer_asset.norito` من البطاقة (أو `fixtures/norito_rpc/transaction_fixtures.manifest.json` أيضًا). الحمولة)۔
4. أداة رمز جهاز OAuth أو أداة إنشاء رمز مميز لحاملها (مفاتيح `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` وتجاوزات `X-TryIt-Auth` تقبل التجاوزات).
5. تم تشغيل ميزة الاتصال والنقر على Torii `fixtures/norito_rpc/schema_hashes.json` من الدرجة `schema_hash`. يتم حفظ علامات التجزئة المتوافقة مع رأس Norito للقفزة/القفزة النشطة.

قم بتجريب هذا المتجر من خلال تسجيل الفيديو `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الذي تم نشره مؤخرًا. تم إنشاء سكربت `cargo xtask norito-rpc-verify`، وخلاصة JSON `artifacts/norito_rpc/<timestamp>/`، والتركيبات التي لا يمكن استخدامها.

## مسئلہ حل کرنا

| إعلامت | هذا رأي | ممكن وج | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | استجابة Torii | `Content-Type` header غائب أو خطأ | تم رفع الحمولة النافعة إلى `Content-Type: application/x-norito`. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | نص الاستجابة/الرؤوس Torii | تركيبات تجزئة المخطط Torii بناء مختلف | `cargo xtask norito-rpc-fixtures` هي تركيبات متجددة و`fixtures/norito_rpc/schema_hashes.json` تقوم بتتبع التجزئة؛ إذا كانت نقطة النهاية Norito نشطة، فسيتم استخدام خيار JSON الاحتياطي. |
| `{"error":"origin_forbidden"}` (HTTP 403) | جربه استجابة الوكيل | هذا الأصل هو `TRYIT_PROXY_ALLOWED_ORIGINS` لا يوجد درج | تختلف البوابة الأصلية (مثل `https://docs.devnet.sora.example`) عن الفن العملي والكريكيت. |
| `{"error":"rate_limited"}` (HTTP 429) | جربه استجابة الوكيل | في IP کوٹا `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` بجٹ تجاوز تجاوز | يتم تحميل موقع التحميل الإلكتروني بشكل متواصل أو غير متوقع على الإطلاق (استجابة JSON `retryAfterMs` ثابتة). |
| `{"error":"upstream_timeout"}` (HTTP 504) أو `{"error":"upstream_error"}` (HTTP 502) | جربه استجابة الوكيل | Torii هذه هي الواجهة الخلفية التي تم تكوينها أو العملية التي لا تزال قائمة | `TRYIT_PROXY_TARGET` هو جهاز كمبيوتر صغير، Torii جهاز كمبيوتر صحي، أو `TRYIT_PROXY_TIMEOUT_MS` لديه عدد كبير من أجهزة الكمبيوتر المحمولة الجديدة. |

مزيد من تشخيصات Try It ونصائح OAuth [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) متاحة.

## إضافة وسائل
- النقل RFC: `docs/source/torii/norito_rpc.md`
- ملخص تنفيذي: `docs/source/torii/norito_rpc_brief.md`
- متتبع الحركة: `docs/source/torii/norito_rpc_tracker.md`
- تعليمات الوكيل للتجربة: `docs/portal/docs/devportal/try-it.md`