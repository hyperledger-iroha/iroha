---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بيئة جربها التجريبي

يوفر بوابة المطورين وحدة تحكم اختيارية "Try it" حتى يتعاون من الاتصال نهاية Torii بدون إخفاء الوثائق. تقوم وحدة التحكم بتمرير الطلبات عبر الوكيل المضمن حتى إطلاق الإصدار من تجاوز إلكترونيات CORS مع بالتالي في فرض حدود التعديل والمصادقة.

##المتطلبات المسبقة

- Node.js 18.18 او احدث (يتطابق مع متطلبات بناء البوابة)
- الوصول إلى شبكة الى بيئة التدريج من Torii
- الرمز المميز لحامله يمكنه الاتصال بشوارع Torii التي تشير إلى اختبارها

تم كل تهيئة أساسية عبر متغيرات البيئة. الجدول ادناه يسرد اهم المفاتيح:

| المتغير | اللحوم | افتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الأساسي الذي يعيد عنوان توجيه الطلبات | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` او `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة مفصولة بفواصل للمصادر الخاصة بها باستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف عميق في `X-TryIt-Client` لكل طلب upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer Token افتراضي يعاد توجيهه الى Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | المستخدمين المستخدمين لتقديم رمز خاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الاقصى لحجم الطلب الكلي (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة المنبع بالميلي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | عدد الطلبات حسب نوع النافذة لكل IP عميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة قابلة للتحويل لتصبح من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان الحصول على اختياري لنقطة metrics G18NT00000000X (`host:port` او `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | مسار HTTP الذي تاجره نقطة المقاييس | `/metrics` |

يُنتج بواسطة المخرج أيضًا `GET /healthz` ويعيد اخطاء JSON منظمة ويخفي Bearer tokens من مخرجات السجل.

فعّل `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` عند عرض وثائق الموظفين الرسميين حتى بدء تسجيل لوحات Swagger وRapiDoc من تسارع Bearer Tokens المقدمة من المستخدم. لا يزال الوكيل يفرض حدود التعديل ويخفي البيانات ويسجل ما اذا كان الطلب استخدام الرمز الافتراضي او تجاوزا لكل طلب. ضبط `TRYIT_PROXY_CLIENT_ID` بالوسم الذي تريد ارساله كـ `X-TryIt-Client`
(الافتراضي `docs-portal`). يقوم الوكيل بقص والتحقق من قيم `X-TryIt-Client` العملاء من أجل هذا الافتراضي حتى يبدأ بوابات التدريج من تدقيق المصدر دون اتصال بيانات الإصدار.

## تشغيل الوكيل المحلي

ضبط التبعيات لأول مرة تجهز فيها البوابة:

```bash
cd docs/portal
npm install
```

الوظيفة التنفيذية للغسالة الى المثال Torii الخاص بك:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

سجل السكربت عنوان الجنوبي ويعيد توجيه الطلبات من `/proxy/*` الى اصل Torii.

قبل ربط الحادثة بالسكرت من ان
`static/openapi/torii.json` يتوافق مع المسجل المسجل في
`static/openapi/manifest.json`. اذا انحرفت الملفات ينهي الأمر بالخطأ ويطلب منك تشغيل
`npm run sync-openapi -- --latest`. صدّر
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط للحالات الطارئة؛ سيسجل المدير التنفيذي ويكمل حتى المبتدئ من
صدر خلال نوافذ البناء.

## عناصر ربط البوابة

عند إنشاء البوابة او تشغيلها، قم بضبط عنوان URL الذي يجب أن تستخدمه العناصر للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

المكونات التالية تقرا هذه القيم من `docusaurus.config.js`:

- **Swagger UI** - تم إنتاجه في `/reference/torii-swagger`; أرجو أن يقترح مخطط حامله عند وجود الرمز المميز،
  يوسمطلبات بـ `X-TryIt-Client`، يحقن `X-TryIt-Auth`، ويعيد توجيه الدعاءات عبر
  الوكيل عندما يتم ضبط `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - تم إنتاجه في `/reference/torii-rapidoc`; انعكاس رمز البحث،
  ويعيد استخدام نفس الرؤوس الخاصة بـ Swagger، ويستهدف الوكيل الحصري عند تحديد العنوان.
- **Try it console** - متقنة في صفحة نظرة عامة على الـ API؛ إرسال طلبات مخصصة،
  عرض الرؤوس، وفحص اجسام XP.

يُعرض الكلالوحتين **لقطات محددة** الذي يُنتج
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` حتى أسرع إلى المراجعين من
القتال بين المواصفات التاريخية والهضم SHA-256 المسجل وتأكيد ما إذا كان لقطة
الاصدار عصر الضغط موقعا قبل استخدام العناصر التفاعلية.

تغيير الـ الرمز المميز في اي عنصر يؤثر فقط على جلسة الاستعراض الحالية؛ الوكيل لا يخزن ولا يسجل الـ الرمز المميز.

## رموز OAuth قصيرة العمر

توزيع الرموز Torii طويل العمر على المراجعين، اربط الوحدة جربها بخادم OAuth لديك. عندما تكون
تنوعات البيئة ادناه موجودة، تعرض واجهة البوابة تسجيل دخول رمز الجهاز، وتصدر الرموز المميزة لحاملها
العمر وحقنها مباشرة في نموذج وحدة التحكم.

| المتغير | اللحوم | افتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | وجهة نظر الجهاز OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة قبول `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة الوثائق | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | نطاقات مفصولة بمسافة مطلوبة أثناء تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API اختياري لربط الـ token | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | أقل مدة التصويت للموافقة على الموافقة (ms) | `5000` (القيم < 5000 مللي ثانية مرفوضة) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | مدة رمز الجهاز احتياطية (ثوان) | `600` (يجب ان يستمر بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | مدة رمز الوصول الاحتياطية (ثوان) | `900` (يجب ان يستمر بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_ALLOW_INSECURE` | اضبط `1` لمعاينات عابرة تتجاوز OAuth عمدا | _unset_ |

تهيئة المثال:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

عند تشغيل `npm run start` او `npm run build` تقوم بتشغيل البوابة بتضمين هذه القيم في
`docusaurus.config.js`. أثناء المعاينة تم التقاط بطاقة Try it زر
"تسجيل الدخول باستخدام رمز الجهاز". أدخل المستخدم الرمز المعروض في صفحة التحقق من OAuth؛ عند النجاح
تدفق الجهاز تقوم بالاداء بما يلي:

- قم بتعبئة الرمز المميز لحامله في وحدة البحث المتقدمة جربها،
- طلبات الطلباتـ headers الحالية `X-TryIt-Client` و `X-TryIt-Auth`،
- عرض العمر النهائي، و
- حذف الـ الرمز المميز تلقائيا عند انتهاء الصلاحية.

يظلم ادخال حامل الحق في خيارنا؛ حذف تنسيقات OAuth عندما تريد اجبار المراجعين على لصق رمز مميز مؤقت
بانفسهم، او صدّر `DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينات الوصول معزولة يكون فيها المجهول المقبولا.
عمليات بناء بدون OAuth انتهت الان تفشل بسرعة لعبة شرطه DOCS-1b في بخارطة الطريق.

ملاحظة: راجع [قائمة التحقق من التعزيز الأمني واختبار القلم](./security-hardening.md)
قبل فتح باب المختبر؛ فهي ت نموذج موثوق وملف CSP/Trusted Types وخطوات pen-test التي
باتت تحجب DOCS-1b.

## امثلة Norito-RPC

طلبات Norito-RPC للاشتراك في نفس الوكيل و OAuth السباكة كما في مسارات JSON؛ فهي تضبط
`Content-Type: application/x-norito` وترسل الحمولة Norito المشفر المسبق الموضح في موصفة NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر مستودع الحمولات الممتدة تحت
`fixtures/norito_rpc/` حتى بدأ كتابة البوابة ومالكو SDK والمراجعون من إعادة إرسال نفس
البايتات التي يستخدمها CI.

### إرسال payload Norito من وحدة Try It

1. اختر تركيبات مثل `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   اصطلاح Norito خام؛ **لا** لم تفعلها الى base64.
2. في Swagger أو RapiDoc، حدد نقطة النهاية NRPC (مثلا
   `POST /v1/pipeline/submit`) وغيور محدد **Content-Type** الى
   `application/x-norito`.
3. بدّل محرر تحرير الطلب إلى **binary** (وضع "ملف" في Swagger او محدد "Binary/File" في RapiDoc)
   وارفع الملف `.norito`. تقوم بالاداء بتمرير البايتات عبر الوكيل بدون تعديل.
4. ارسل الطلب. اذا اعاد Torii `X-Iroha-Error-Code: schema_mismatch`، تحقق انك تستدعي
   إن نقطة النهاية تقبل الحمولات الصافية المختلفة والتأكد من أن schema hash المسجل في
   `fixtures/norito_rpc/schema_hashes.json` يتوافق مع البنية Torii الذي استخدمه.

تحفظ وحدة التحكم اخر ملف في الذاكرة لتتمكن من إعادة إرسال نفس الحمولة باستخدام الرموز المميزة المختلفة
او المضيفي Torii. اضافة `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الى سير عملك
قم بإنتاج حزمة المتسابقة المراد في خطة تعتمد على NRPC-4 (log + ملخص JSON)، وهو ما يتكامل بشكل جيد معها
التقاط لقطات الشاشة للاستجابة جربها أثناء المراجعات.

### مثال CLI (curl)

يمكن إعادة تشغيل نفس التركيبات خارج البوابة عبر `curl`، وهو مفيد عند التحقق من الوكيل أو
بوابة تصحيح ردود الفعل:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

بدل تركيب باي ادخال مدرج في `transaction_fixtures.manifest.json` او قم بترميز payload خاص بك عبر
`cargo xtask norito-rpc-fixtures`. عندما يكون Torii في وضع canary يمكنك توجيه `curl` الى
proxy Try-it (`https://docs.sora.example/proxy/v1/pipeline/submit`) نفس البنية
التي تستخدمها عناصر البوابة.

##المحاكمة

يتم تسجيل كل طلب مرة واحدة باستخدام الطريقة والمسار والأصل والحالة الأولية ومصدر المصادقة
(`override` او `default` او `client`). لا يتم تخزين الـ الرموز المميزة؛ يتم تأهيل الرؤوس الحاملة
وقيم `X-TryIt-Auth` قبل التسجيل، لذلك يمكنك stdout الى مجمع مركزي دون القلق من طعام الاسرار.

### فحوصات الصحة والتنبيه

شغل مسبار التشغيل أثناء النشر او بجدول العمل:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

مفاتيح البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - مسار Torii اختياري (بدون `/proxy`)
- `TRYIT_PROXY_SAMPLE_METHOD` - افتراضي `GET`; اضبط `POST` الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - يحقن الرمز المميز للطلب التجريبي.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يستبدل المهلة الافتراضية 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - وجهة نص Prometheus اختيارية لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - ازواج `key=value` مفصولة بفواصل تضاف الى المقاييس (الافتراضي `job=tryit-proxy` و `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - عنوان اختياري لنقطة القياسات (مثل `http://localhost:9798/metrics`) يجب ان يستجيب عند أساسي `TRYIT_PROXY_METRICS_LISTEN`.

تم دمج النتائج في textfile Collector عبر توجيه التحقيق الى مسار قابل للكتابة
(مثل `/var/lib/node_exporter/textfile_collector/tryit.prom`) إضافة وسوم مخصصة:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

أعاد السكربت كتابة ملف المعايير بشكل ذري حتى مزيج المجمع دائما payload كاملا.

عند ضبط `TRYIT_PROXY_METRICS_LISTEN`، قم بعمل
`TRYIT_PROXY_PROBE_METRICS_URL` الى نقطة المقاييس حتى تفشل التحقيق بسرعة اذا اختفت واجهة كشط
(مثل الدخول غير مضبوط او قواعد جدار الحماية مفقودة). بعد الإنتاج شاع هو
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.للتنبيه الخفيف، اربط المسبار بمواجهتك. مثال Prometheus يقوم بالتصعيد بعد فشلين متتاليين:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### مقاييس النقطة ولوحات المتابعة

اضبط `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (او اي زوج مضيف/منفذ) قبل تشغيل الوكيل
لكشف مقاييس النقاط Prometheus. المسار الافتراضي هو `/metrics` ويمكن تغييره عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. كل كشط يعيد عدادات لاجماليطلبات حسب الطلب ورفض
Rate Limit واخطاء timeouts upstream ونتائج الوكيل وملخصات latency:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

وجّه جامعات Prometheus/OTLP الى نقطة المقاييس واعِد استخدام اللوحات
`dashboards/grafana/docs_portal.json` حتى يتمكن SRE من مراقبة زمن الاستجابة الذي يجذب الرفض
دون كتابة سجل. ينشر الوكيل الرسمي `tryit_proxy_start_timestamp_ms` لمساعدة العاملين لديهم
على اختراع إعادة التشغيل.

### تمتعة السماء

استخدم اداة الادارة لتحديث او استعادة عنوان Torii المستهدف. يحفظ السكربت الاعداد السابق في
`.env.tryit-proxy.bak` حتى يكون السماء امارا واحدا.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

يمكنك تجاوز مسار ملف env عبر `--env` او `TRYIT_PROXY_ENV` اذا كان النشر يحفظ الاعدادات في موقع اخر.