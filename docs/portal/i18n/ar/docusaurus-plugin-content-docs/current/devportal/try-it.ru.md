---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# رسالة جربها

تشتمل بوابة المطور على وحدة تحكم اختيارية "جربها"، لتتمكن من معرفة نقاط النهاية Torii، ولا تحذف الوثائق. تقوم وحدة التحكم بإجراء عمليات بحث من خلال الوكيل الخارجي، وذلك من أجل الحصول على حماية CORS للسترات، وحدود معدل الاشتراك الفرعي والمصادقة.

## خدمة مسبقة

- Node.js 18.18 أو الجديد (بوابة البناء المطلوبة بشدة)
- الوصول إلى وحدة التدريج Torii
- الرمز المميز لحامله، والذي يمكنك من خلاله التعرف على المخططات الجديدة Torii

يتم استخدام كل الوكيل المعتمد من خلال الحماية المؤقتة. في الجدول هناك مقابض أساسية جديدة:

| متغير | الاسم | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان URL الأساسي Torii، حيث يقوم الوكيل بإرسال المساعدة | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | عنوان البريد الإلكتروني للمؤسسات المحلية (تنسيق `host:port` أو `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة الأصل التي يتم استبدالها بسهولة بالوكيل (من خلال الاتصال) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | المعرف الذي تم إضافته إلى `X-TryIt-Client` للتكلفة المنبع | `docs-portal` |
| `TRYIT_PROXY_BEARER` | الرمز المميز لحامله، يتم تحويله إلى Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | التعرف على المستفيد من خلال تحويل الرمز المميز الخاص بك عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الأقصى لحجم الخط (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة المنبع في ملي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | إجراءات الحد من معدل عملاء IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | الحد الأقصى لمعدل النقر (مللي ثانية) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان البريد للمقياس Prometheus (`host:port` أو `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | وضع HTTP لمقياس نقطة النهاية | `/metrics` |

يعرض الوكيل أيضًا `GET /healthz`، وينشئ هياكل JSON ويعدل الرموز المميزة لحاملها في السجل.

قم بضم `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`، إذا قام الوكيل بتوفير مستندات مفيدة، بحيث يمكن للوحة Swagger وRapiDoc نقل الرموز المميزة لحاملها، شكرا جزيلا. يقوم الوكيل بإلغاء حدود المعدلات، وتعديل نقاط الاعتماد، وإلغاء الاشتراك، واستخدام الرمز المميز للتمويل أو تجاوز الطلب. قم بتثبيت `TRYIT_PROXY_CLIENT_ID` بالأسلوب الذي تريد القيام به مثل `X-TryIt-Client`
(من خلال `docs-portal`). يقوم الوكيل بإبلاغ `X-TryIt-Client` من العميل والتحقق من صحته ويوافق على هذا الافتراضي، بحيث يمكن للبوابات المرحلية تدقيق المصدر دون الارتباط مع السراويل المتغيرة.

## Запуск proxy локально

قم بتعزيز المعرفة عند البوابة الأولى:

```bash
cd docs/portal
npm install
```

قم بتثبيت الوكيل وتشغيل مثيل Torii الخاص بك:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

يقوم البرنامج النصي بربط عنوان السجل وبروكسل مع `/proxy/*` من الأصل Torii.

قبل ربط البرنامج النصي، تأكد من ذلك
`static/openapi/torii.json` ملخص رائع، مكتوب في
`static/openapi/manifest.json`. إذا تم إلغاء الملف، يقوم الأمر بإغلاق الملف ويقترح استخدام `npm run sync-openapi -- --latest`. قم بالتصدير
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط لكائنات الطيور; يقوم الوكيل بإنهاء الاقتراحات وإطالة العمل حتى تتمكن من الترقية خلال نافذة الصيانة.

## إضافة بوابة مقاطع الفيديو

عند الإنشاء أو الخدمة، أضف عنوان URL الذي تستخدمه البوابة التي تستخدمها للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

المكونات التالية تبدأ من `docusaurus.config.js`:

- **Swagger UI** - يتم النشر على `/reference/torii-swagger`; مخطط حامل التفويضات المسبقة عند الحصول على الرمز المميز، واستعادة `X-TryIt-Client`، وإدخال `X-TryIt-Auth`، وإعادة نسخ الأذونات عبر الوكيل، عندما `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - يتم النشر على `/reference/torii-rapidoc`; قم بإزالة الرمز المميز، واستخدم الرؤوس Swagger ويتم التحكم تلقائيًا من خلال الوكيل في عنوان URL المستأجر.
- **Try it console** - موجود في واجهة برمجة التطبيقات (API) للواجهة؛ يتيح لك إجراء فحوصات مخصصة ونشر الرؤوس وفحص إجابة الهاتف.

تظهر هذه اللوحة **محدد اللقطة** الذي تقرأه
`docs/portal/static/openapi/versions.json`. مؤشر القيادة كوماندو
`npm run sync-openapi -- --version=<label> --mirror=current --latest`، حيث يمكن للمراجعين الاختيار بين المواصفات التاريخية، ومشاهدة ملخص SHA-256 المسجل والتأكد من أن إصدار اللقطة تم توقيعه البيان قبل استخدام مقاطع الفيديو التفاعلية.

يمكن استخدام الرمز المميز الموجود في أي مكان على السترة القصيرة جدًا؛ لا يوجد وكيل مسجل ولا يقوم بتسجيل الدخول إلى الرمز المميز.

## رموز OAuth المميزة

من أجل عدم توسيع نطاق الرموز المميزة Torii بين المراجعين، قم بإضافة وحدة تحكم Try it إلى خادم OAuth. عندما تحتاج إلى توسيع نطاق الخدمة، تقوم البوابة بإظهار عنصر واجهة المستخدم لرمز الجهاز، واستخدام الرموز المميزة لحاملها والأدوات التلقائية قم بإدخالها في نموذج وحدة التحكم.

| متغير | الاسم | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | نقطة نهاية ترخيص جهاز OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة نهاية الرمز المميز، priнимающий `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth، مسجل لمعاينة المستندات | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | نطاقات، أسئلة مختلفة، مغلقة عند الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | جمهور API الاختياري للحصول على الرموز المميزة | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | الحد الأدنى للفاصل الزمني للاستقصاء قبل إجراء الاستقصاء (مللي ثانية) | `5000` (المسافة < 5000 مللي ثانية) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | رمز جهاز Фоллбек TTL (بالثواني) | `600` (المدة بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | رمز وصول Фоллбек TTL (بالثواني) | `900` (المدة بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_ALLOW_INSECURE` | تثبيت `1` للمعاينة المحلية التي تستخدم OAuth | _unset_ |

تكوين المثال:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

عندما تقوم بإغلاق `npm run start` أو `npm run build`، سيتم عرض هذه البوابة في `docusaurus.config.js`. أثناء معاينة البطاقة المحلية، جربها بالضغط على الزر "تسجيل الدخول باستخدام رمز الجهاز". يقوم المستجيبون بإدخال رمز الكشف إلى صفحة التحقق من OAuth الخاصة بك؛ مقطع تدفق الجهاز الناجح التالي:

- قم بتثبيت رمز حامل مميز في لوحة التحكم جربه،
- إضافة الرؤوس `X-TryIt-Client` و`X-TryIt-Auth`،
- تمديد فترة الحياة،
- تسجيل تلقائي للرمز المميز عند إنشائه.

يتمتع Bearer بميزة الوصول السريع - استخدم OAuth الدائم، إذا كنت ترغب في تثبيت المراجعين على رمز مميز فوري، أو قم بتصدير `DOCS_OAUTH_ALLOW_INSECURE=1` للمعاينة المحلية المعزولة، من خلال نموذج توصيل مجهول. يتم إنشاء عمليات إنشاء بدون بروتوكول OAuth القوي، مما يتيح لك الاستمتاع ببوابة DOCS-1b.

ملاحظة: قبل نشر بوابة المختبرات السابقة، تحقق من [قائمة التحقق من التعزيز الأمني ​​واختبار القلم](./security-hardening.md); ضمن نموذج التهديد الموضح، قم بإنشاء ملف تعريف لـ CSP/الأنواع الموثوقة وخيارات اختبار القلم، والتي تتضمن بوابة DOCS-1b.

## الأمثلة Norito-RPC

تستخدم إجراءات Norito-RPC كلًا من الوكيل والسباكة OAuth، ومسارات JSON؛ يقوم هذا ببساطة بتثبيت `Content-Type: application/x-norito` ويقوم بإدارة الحمولة الصافية Norito، الموضحة في مواصفات NRPC (`docs/source/torii/nrpc_spec.md`). يقوم المستودع بتحميل الحمولات الأساسية ضمن `fixtures/norito_rpc/`، لبوابة المؤلفين، ويمكن لمالكي SDK والمراجعين الحصول على وحدات بايت كبيرة جدًا، يستخدم CI.

### حمولة Norito من وحدة التحكم Try It

1. اختر التركيب، على سبيل المثال `fixtures/norito_rpc/transfer_asset.norito`. تتضمن هذه الملفات مغلفات Norito؛ **لا** تقوم بترميزها في base64.
2. في Swagger أو RapiDoc، ابحث عن نقطة نهاية NRPC (على سبيل المثال، `POST /v2/pipeline/submit`) وحدد محدد **Content-Type** لـ `application/x-norito`.
3. قم بإلغاء تحديد المصحح في **binary** (اختر "File" في Swagger أو "Binary/File" في RapiDoc) وقم بتصفح الملف `.norito`. قم بعرض البايتات من خلال الوكيل دون تغيير.
4. قم بالحل. إذا قام Torii بالتعبير عن `X-Iroha-Error-Code: schema_mismatch`، فتأكد من تحديد نقطة النهاية والحمولات الثنائية الأولية والتأكد من ذلك تم دمج تجزئة المخطط في `fixtures/norito_rpc/schema_hashes.json` مع الإصدار Torii.

قم بإعادة توجيه الملف التالي إلى المستشار بحيث يمكنك إعادة توجيه الحمولة باستخدام رموز التفويض المختلفة أو مضيفي Torii. يؤدي إدخال `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` إلى سير العمل إلى إنشاء حزمة أدلة، معززة بمخطط الارتباط NRPC-4 (سجل + ملخص JSON)، والذي يتم تلقيه بشكل جيد من خلال الشاشة الرد على جربه خلال المراجعات.

### نموذج CLI (الضفيرة)

يمكن أيضًا للتركيبات الوصول إلى البوابة عبر `curl`، وهو أمر مفيد للتحقق من الوكيل أو بوابة الخروج:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

قم بتثبيت وحدة الإدخال لأي إدخال من `transaction_fixtures.manifest.json` أو قم بفك تشفير أمر الحمولة الخاص بك `cargo xtask norito-rpc-fixtures`. عند استخدام Torii في نظام الكناري، يمكنك تشغيل `curl` على وكيل Try-it (`https://docs.sora.example/proxy/v2/pipeline/submit`)، للتحقق من ذلك. البنية التحتية لبوابة المشاهدة والمشاهدة.

## إمكانية الملاحظة والعمليات

يتم تسجيل كل عملية من خلال الطريقة والمسار والأصل والحالة الأولية والمصادقة الأصلية (`override` أو `default` أو `client`). يتم تعديل الرموز المميزة التي لا يتم تخزينها - الرؤوس الحاملة والاختصار `X-TryIt-Auth` قبل تسجيل الدخول، ويمكن نقل هذا العنصر إلى stdout المجمع المركزي لا يخاطر بأي أسرار.### المجسات الصحية والتنبيه

قم بإغلاق المسبار الداخلي عند النشر أو في الوصف:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

زينة المقابض:

- `TRYIT_PROXY_SAMPLE_PATH` - Torii اختياري (باستثناء `/proxy`) للاختبار.
- `TRYIT_PROXY_SAMPLE_METHOD` - по умолчанию `GET`; أضف `POST` لكتابة المخططات.
- `TRYIT_PROXY_PROBE_TOKEN` - إضافة رمز مميز للحامل إلى عينة إيصال.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - إعادة ضبط المهلة لمدة 5 ثوانٍ.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - ملف نصي اختياري Prometheus لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - زوجان من `key=value`، مضافان إلى المقياس (من خلال `job=tryit-proxy` و`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - نقطة نهاية مقاييس URL الاختيارية (على سبيل المثال `http://localhost:9798/metrics`)، والتي يتم تحقيقها بنجاح بما في ذلك `TRYIT_PROXY_METRICS_LISTEN`.

قم بنقل النتائج إلى أداة تجميع الملفات النصية، وضبط المسار القابل للكتابة (على سبيل المثال، `/var/lib/node_exporter/textfile_collector/tryit.prom`) وإضافة التسميات المخصصة:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يقوم البرنامج النصي بإعادة كتابة مقياس الملف الذي يجعل المجمع يصل إلى الحمولة الكاملة.

إذا تم إنشاء `TRYIT_PROXY_METRICS_LISTEN`، تفضل بالاشتراك
`TRYIT_PROXY_PROBE_METRICS_URL` لنقطة نهاية المقاييس، لإجراء اختبار أفضل قبل نشر الكشط الشامل (على سبيل المثال، الدخول غير المشروط أو الخروج من قواعد جدار الحماية). أنظمة الإنتاج النموذجية:
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

للتنبيه البسيط، قم بربط المسبار بمكدس المراقبة الخاص بك. المثال Prometheus، الذي تم طرحه بعد اثنتين من السطور التالية:

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

### نقطة نهاية المقاييس ولوحات المعلومات

قم بتثبيت `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (أو أي مضيف/منفذ) قبل تشغيل الوكيل لفتح نقطة نهاية المقاييس بتنسيق Prometheus. أدخل إلى `/metrics`، ولكن يمكن إعادة تقديمه من خلال `TRYIT_PROXY_METRICS_PATH=/custom`. كل ما يتخلص منه هو استخلاص الملاحظات من خلال الطريقة، ورفض الحد الأقصى للمعدل، والأخطاء/المهلات الأولية، ونتائج الوكيل، وزمن الاستجابة الموجز:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

قم بتكوين مجمعات Prometheus/OTLP لنقطة نهاية المقاييس واستخدم اللوحات من `dashboards/grafana/docs_portal.json` لتتمكن SRE من مراقبة زمن الوصول الخلفي والتفاصيل تعليقات بدون شعار التحليل. الوكيل العام التلقائي `tryit_proxy_start_timestamp_ms`، يساعد المشغل على إعادة التشغيل.

### التراجع التلقائي

استخدم مساعد الإدارة للتحديث أو تحسين عنوان URL الكامل Torii. يحفظ البرنامج النصي التكوين المسبق في `.env.tryit-proxy.bak`، وهو التراجع - وهو أمر واحد.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

قم بتمرير ملف env عبر `--env` أو `TRYIT_PROXY_ENV`، إذا قمت بإعادة تكوين القرص الخاص بك في المكان الآخر.