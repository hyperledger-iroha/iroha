---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# بيئة Try It التجريبية

يوفر بوابة المطورين وحدة تحكم اختيارية "Try it" حتى تتمكن من استدعاء نقاط نهاية Torii بدون مغادرة الوثائق. تقوم وحدة التحكم بتمرير الطلبات عبر الوكيل المضمن حتى تتمكن المتصفحات من تجاوز قيود CORS مع الاستمرار في فرض حدود المعدل والمصادقة.

## المتطلبات المسبقة

- Node.js 18.18 او احدث (يتطابق مع متطلبات بناء البوابة)
- وصول شبكة الى بيئة staging من Torii
- bearer token يمكنه استدعاء مسارات Torii التي تنوي اختبارها

تتم كل تهيئة الوكيل عبر متغيرات البيئة. الجدول ادناه يسرد اهم المفاتيح:

| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الاساسي الذي يعيد الوكيل توجيه الطلبات اليه | **Required** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` او `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة مفصولة بفواصل للمصادر المسموح لها باستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف يوضع في `X-TryIt-Client` لكل طلب upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | bearer token افتراضي يعاد توجيهه الى Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | السماح للمستخدمين بتقديم token خاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الاقصى لحجم جسم الطلب (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة upstream بالميلي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | عدد الطلبات المسموح بها لكل نافذة معدل لكل IP عميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة انزلاقية للحد من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان استماع اختياري لنقطة metrics بأسلوب Prometheus (`host:port` او `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | مسار HTTP الذي تخدمه نقطة metrics | `/metrics` |

يعرض الوكيل ايضا `GET /healthz` ويعيد اخطاء JSON منظمة ويخفي bearer tokens من مخرجات السجل.

فعّل `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` عند عرض الوكيل لمستخدمي الوثائق حتى تتمكن لوحات Swagger وRapiDoc من تمرير bearer tokens المقدمة من المستخدم. لا يزال الوكيل يفرض حدود المعدل ويخفي البيانات الحساسة ويسجل ما اذا كان الطلب استخدم token الافتراضي او تجاوزا لكل طلب. اضبط `TRYIT_PROXY_CLIENT_ID` بالوسم الذي تريد ارساله كـ `X-TryIt-Client`
(الافتراضي `docs-portal`). يقوم الوكيل بقص والتحقق من قيم `X-TryIt-Client` المقدمة من العميل والعودة الى هذا الافتراضي حتى تتمكن بوابات staging من تدقيق المصدر دون ربط بيانات المتصفح.

## تشغيل الوكيل محليا

ثبت التبعيات اول مرة تجهز فيها البوابة:

```bash
cd docs/portal
npm install
```

شغل الوكيل واشره الى مثيل Torii الخاص بك:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

يسجل السكربت عنوان الربط ويعيد توجيه الطلبات من `/proxy/*` الى اصل Torii المحدد.

قبل ربط المنفذ يتحقق السكربت من ان
`static/openapi/torii.json` يطابق الـ digest المسجل في
`static/openapi/manifest.json`. اذا انحرفت الملفات ينهي الامر بخطأ ويطلب منك تشغيل
`npm run sync-openapi -- --latest`. صدّر
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط لحالات الطوارئ؛ سيسجل الوكيل تحذيرا ويكمل حتى تتمكن من
التعافي خلال نوافذ الصيانة.

## ربط عناصر البوابة

عند بناء البوابة او تشغيلها، اضبط عنوان URL الذي يجب ان تستخدمه العناصر للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

المكونات التالية تقرا هذه القيم من `docusaurus.config.js`:

- **Swagger UI** - يعرض في `/reference/torii-swagger`; يسبق تفويض مخطط bearer عند وجود token،
  يوسم الطلبات بـ `X-TryIt-Client`، يحقن `X-TryIt-Auth`، ويعيد توجيه الاستدعاءات عبر
  الوكيل عندما يتم ضبط `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - يعرض في `/reference/torii-rapidoc`; يعكس حقل token،
  ويعيد استخدام نفس headers الخاصة بـ Swagger، ويستهدف الوكيل تلقائيا عند ضبط العنوان.
- **Try it console** - مضمّنة في صفحة نظرة عامة على الـ API؛ تتيح ارسال طلبات مخصصة،
  عرض headers، وفحص اجسام الاستجابة.

يعرض كلا اللوحتين **محدد snapshots** الذي يقرأ
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` حتى يتمكن المراجعون من
التنقل بين المواصفات التاريخية ومشاهدة digest SHA-256 المسجل وتأكيد ما اذا كان snapshot
الاصدار يحمل manifest موقعا قبل استخدام العناصر التفاعلية.

تغيير الـ token في اي عنصر يؤثر فقط على جلسة المتصفح الحالية؛ الوكيل لا يخزن ولا يسجل الـ token المقدم.

## رموز OAuth قصيرة العمر

لتجنب توزيع رموز Torii طويلة العمر على المراجعين، اربط وحدة Try it بخادم OAuth لديك. عندما تكون
متغيرات البيئة ادناه موجودة، تعرض البوابة واجهة تسجيل دخول device code، وتصدر bearer tokens قصيرة
العمر وتحقنها تلقائيا في نموذج وحدة التحكم.

| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | نقطة تفويض الجهاز OAuth (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة token تقبل `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة الوثائق | _empty_ |
| `DOCS_OAUTH_SCOPE` | نطاقات مفصولة بمسافة مطلوبة اثناء تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API اختياري لربط الـ token | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | اقل فترة polling اثناء انتظار الموافقة (ms) | `5000` (القيم < 5000 ms مرفوضة) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | مدة صلاحية device code الاحتياطية (ثوان) | `600` (يجب ان تبقى بين 300 s و 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | مدة صلاحية access token الاحتياطية (ثوان) | `900` (يجب ان تبقى بين 300 s و 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | اضبط `1` لمعاينات محلية تتجاوز OAuth عمدا | _unset_ |

مثال تهيئة:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

عند تشغيل `npm run start` او `npm run build` تقوم البوابة بتضمين هذه القيم في
`docusaurus.config.js`. اثناء المعاينة المحلية تعرض بطاقة Try it زر
"Sign in with device code". يدخل المستخدمون الرمز المعروض في صفحة التحقق OAuth؛ عند نجاح
device flow تقوم الاداة بما يلي:

- حقن bearer token الصادر في حقل وحدة Try it،
- وسم الطلبات بالـ headers الحالية `X-TryIt-Client` و `X-TryIt-Auth`،
- عرض العمر المتبقي، و
- حذف الـ token تلقائيا عند انتهاء الصلاحية.

يظل ادخال Bearer اليدوي متاحا؛ احذف متغيرات OAuth عندما تريد اجبار المراجعين على لصق token مؤقت
بانفسهم، او صدّر `DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينات محلية معزولة يكون فيها الوصول المجهول مقبولا.
عمليات build بدون OAuth مكون الان تفشل بسرعة لتلبية شرط DOCS-1b في خارطة الطريق.

ملاحظة: راجع [Security hardening & pen-test checklist](./security-hardening.md)
قبل فتح البوابة خارج المختبر؛ فهي توثق نموذج التهديد وملف CSP/Trusted Types وخطوات pen-test التي
باتت تحجب DOCS-1b.

## امثلة Norito-RPC

طلبات Norito-RPC تشترك في نفس proxy و OAuth plumbing كما في مسارات JSON؛ فهي تضبط
`Content-Type: application/x-norito` وترسل payload Norito المشفر مسبقا الموضح في مواصفة NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر المستودع payloads قياسية تحت
`fixtures/norito_rpc/` حتى يتمكن مؤلفو البوابة ومالكو SDK والمراجعون من اعادة ارسال نفس
البايتات التي يستخدمها CI.

### ارسال payload Norito من وحدة Try It

1. اختر fixture مثل `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   اغلفة Norito خام؛ **لا** تقم بتحويلها الى base64.
2. في Swagger او RapiDoc، حدد endpoint NRPC (مثلا
   `POST /v1/pipeline/submit`) وغيّر محدد **Content-Type** الى
   `application/x-norito`.
3. بدّل محرر جسم الطلب الى **binary** (وضع "File" في Swagger او محدد "Binary/File" في RapiDoc)
   وارفع ملف `.norito`. تقوم الاداة بتمرير البايتات عبر الوكيل بدون تعديل.
4. ارسل الطلب. اذا اعاد Torii `X-Iroha-Error-Code: schema_mismatch`، تحقق انك تستدعي
   endpoint يقبل payloads ثنائية وتاكد ان schema hash المسجل في
   `fixtures/norito_rpc/schema_hashes.json` يطابق build Torii الذي تستخدمه.

تحفظ وحدة التحكم اخر ملف في الذاكرة لتتمكن من اعادة ارسال نفس payload مع اختبار tokens مختلفة
او مضيفي Torii. اضافة `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الى سير عملك
تنتج bundle الادلة المشار اليه في خطة اعتماد NRPC-4 (log + ملخص JSON)، وهو ما يتكامل جيدا مع
التقاط screenshots لاستجابة Try It اثناء المراجعات.

### مثال CLI (curl)

يمكن اعادة تشغيل نفس fixtures خارج البوابة عبر `curl`، وهو مفيد عند التحقق من الوكيل او
تصحيح ردود gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

بدل fixture باي ادخال مدرج في `transaction_fixtures.manifest.json` او قم بترميز payload خاص بك عبر
`cargo xtask norito-rpc-fixtures`. عندما يكون Torii في وضع canary يمكنك توجيه `curl` الى
proxy try-it (`https://docs.sora.example/proxy/v1/pipeline/submit`) لاختبار نفس البنية
التي تستخدمها عناصر البوابة.

## المراقبة والعمليات

يتم تسجيل كل طلب مرة واحدة مع method و path و origin وحالة upstream ومصدر المصادقة
(`override` او `default` او `client`). لا يتم تخزين الـ tokens اطلاقا؛ يتم تنقيح bearer headers
وقيم `X-TryIt-Auth` قبل التسجيل، لذا يمكنك تمرير stdout الى مجمع مركزي دون القلق من تسرب الاسرار.

### فحوصات الصحة والتنبيه

شغل probe المرفق اثناء النشر او بجدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

مفاتيح البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - مسار Torii اختياري (بدون `/proxy`) لاختباره.
- `TRYIT_PROXY_SAMPLE_METHOD` - الافتراضي `GET`; اضبط `POST` لعمليات الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - يحقن bearer token مؤقت للطلب التجريبي.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يستبدل المهلة الافتراضية 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - وجهة نص Prometheus اختيارية لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - ازواج `key=value` مفصولة بفواصل تضاف الى المقاييس (الافتراضي `job=tryit-proxy` و `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - عنوان اختياري لنقطة metrics (مثل `http://localhost:9798/metrics`) يجب ان يستجيب عند تمكين `TRYIT_PROXY_METRICS_LISTEN`.

ادمج النتائج في textfile collector عبر توجيه probe الى مسار قابل للكتابة
(مثل `/var/lib/node_exporter/textfile_collector/tryit.prom`) واضافة وسوم مخصصة:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يعيد السكربت كتابة ملف المقاييس بشكل ذري حتى يقرأ المجمع دائما payload كاملا.

عند ضبط `TRYIT_PROXY_METRICS_LISTEN`، قم بتعيين
`TRYIT_PROXY_PROBE_METRICS_URL` الى نقطة metrics حتى تفشل probe بسرعة اذا اختفت واجهة scrape
(مثل ingress غير مضبوط او قواعد firewall مفقودة). اعداد production شائع هو
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

للتنبيه الخفيف، اربط probe بواجهة المراقبة لديك. مثال Prometheus يقوم بالتصعيد بعد فشلين متتاليين:

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

### نقطة metrics ولوحات المتابعة

اضبط `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (او اي زوج host/port) قبل تشغيل الوكيل
لكشف نقطة metrics بتنسيق Prometheus. المسار الافتراضي هو `/metrics` ويمكن تغييره عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. كل scrape يعيد عدادات لاجمالي الطلبات حسب method ورفض
rate limit واخطاء/timeouts upstream ونتائج الوكيل وملخصات latency:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

وجّه مجمعات Prometheus/OTLP الى نقطة metrics واعِد استخدام لوحات
`dashboards/grafana/docs_portal.json` حتى يتمكن SRE من مراقبة latencies الذيل وارتفاعات الرفض
دون تحليل السجلات. ينشر الوكيل تلقائيا `tryit_proxy_start_timestamp_ms` لمساعدة المشغلين
على اكتشاف اعادة التشغيل.

### اتمتة التراجع

استخدم اداة الادارة لتحديث او استعادة عنوان Torii المستهدف. يحفظ السكربت الاعداد السابق في
`.env.tryit-proxy.bak` حتى يكون التراجع امرا واحدا.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

يمكنك تجاوز مسار ملف env عبر `--env` او `TRYIT_PROXY_ENV` اذا كان النشر يحفظ الاعدادات في موقع اخر.
