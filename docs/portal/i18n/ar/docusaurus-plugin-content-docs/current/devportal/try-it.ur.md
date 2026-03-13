---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# جربه سینڈ باكس

منفذ ويلبر هو أحد متخصصي "جربه" وهو عبارة عن أداة لتكوين نقاط النهاية Torii لنقاط النهاية. تعمل تقنية كنسول بنسل على توفير الحماية لـ CORS ذات النطاق العريض لسجلات الألعاب والهواتف الذكية.

##قطاع

- Node.js 18.18 أو أحدث (تتوافق متطلبات بناء الصفحة)
- Torii تحويل عمل جديد
- هذا الرمز المميز لحامله هو طرق Torii التي ستساعدك على الوصول إلى ما تريد

تم تحديث عملية تهيئة كل متغيرات البيئة. بعض المقابض التي يمكن وضعها في مكانها:

| متغير | قصدي | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii هو عنوان URL الأساسي وهو عبارة عن عنوان URL للصفحة الرئيسية | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | لوكل ميمنت لعنوان الاستماع (`host:port` أو `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | أصول مفصولة بفواصل هي عملية الإنتاج | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | طلب المنبع `X-TryIt-Client` يستخدم للمعرف | `docs-portal` |
| `TRYIT_PROXY_BEARER` | تم إصدار الرمز المميز لحامله Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | تم استخدام هذا الرمز المميز `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | نص الطلب کا زیادہ سے زیادہ سائز (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة المنبع (ملي ثانية) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | ہر العميل IP کے لئے فی نافذة اجازت شدہ الطلبات | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | تحديد معدل کے لئے نافذة منزلقة (مللي ثانية) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus طرز مقاييس نقطة النهاية کے لئے اختیاری عنوان الاستماع (`host:port` أو `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | مقاييس نقطة النهاية کا مسار HTTP | `/metrics` |

تم حذف `GET /healthz` من أخطاء JSON المنظمة، وتم تنقيح الرموز المميزة لحاملها في إخراج السجل.

`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` نشط في عمليات الدفع لمستخدمي المستندات الذين يكشفون عن لوحات Swagger وRapiDoc للرموز المميزة لحاملها التي يوفرها المستخدم. تعمل هذه السياسة أيضًا على حدود معدل استخدام البطاقة وبيانات الاعتماد وتنقيح البطاقة وتسجيل البطاقة وطلب عدم استخدام الرمز المميز الافتراضي أو تجاوز لكل طلب. `TRYIT_PROXY_CLIENT_ID` وهو التصنيف الذي يحمل عنوان `X-TryIt-Client` وهو ما يحدث الآن
(ڈیفالٹ `docs-portal`). تعمل قيم `X-TryIt-Client` المقدمة من المتصل على تقليم البيانات والتحقق من صحتها، كما تعمل بشكل افتراضي على الاتصال بالبوابات المرحلية وربط البيانات التعريفية للمتصفح بكل ما يتعلق بتدقيق المصدر.

## التوفير في عمليات الشراء

تثبيت تبعيات موقع البوابة:

```bash
cd docs/portal
npm install
```

وكيل العمليات وهذا هو مثيل Torii على سبيل المثال:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

عنوان منضم إلى كرتا و`/proxy/*` هو عبارة عن طلبات تم تكوينها Torii أصل رأس إلى الأمام كرتا.

قم بربط الملف بالكامل ثم تحقق من الرابط
`static/openapi/torii.json` هو ملخص `static/openapi/manifest.json` يحتوي على ملخص سريع. إذا حدث الانجراف الزائد، يمكنك التحكم في الخطأ ثم الخروج من السجل وتسجيل البيانات `npm run sync-openapi -- --latest`. `TRYIT_PROXY_ALLOW_STALE_SPEC=1` ينفق على تجاوز الاستخدام؛ تحذير وقائي لا داعي للقلق بشأن الصيانة المستمرة لنوافذ الصيانة.

## عناصر واجهة المستخدم للبورتات

عند إنشاء بوابة المطورين أو تقديمها، يمكنك استخدام أدوات URL الخاصة بـ Web Widgets:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

هناك قيم محددة للمكونات `docusaurus.config.js`:

- **Swagger UI** - `/reference/torii-swagger` تم تقديمه؛ الرمز المميز موجود في مخطط حامل التفويض المسبق لـ كرتا، وطلبات `X-TryIt-Client` سے علامة كرتا ہے، `X-TryIt-Auth` حقن كرتا ہے، و`TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے إعادة كتابة المكالمات التي يتم إجراؤها بواسطة پروكسی کرتا ہے.
- **RapiDoc** - `/reference/torii-rapidoc` تقديم العرض ہوتا ہے؛ يقوم الرمز المميز بعكس الكرتا، ورؤوس لوحة Swagger التي تعيد استخدام الكرتا، وتكوين عنوان URL مرة واحدة للوكيل المستهدف للكرتا.
- **Try it console** - تم تضمين صفحة نظرة عامة على واجهة برمجة التطبيقات؛ يتم تحديد الطلبات المخصصة والعناوين الرئيسية وهيئات الاستجابة التي تقوم بفحصها.

لوحات التبرع هي **محدد اللقطة** معززة
`docs/portal/static/openapi/versions.json` هذا صحيح. فهرس کو
`npm run sync-openapi -- --version=<label> --mirror=current --latest` يحتوي على المواصفات التاريخية للمراجعين، ومسجل SHA-256، وعناصر واجهة المستخدم التفاعلية المستخدمة في تصديق البطاقة لإصدار لقطة موقعة لبيان کریا ہے۔

تحتوي هذه القطعة أيضًا على رمز مميز بدلًا من جلسة المتصفح الحالية؛ الوكيل هو أيضًا عبارة عن رمز مميز لاستمرار أو تسجيل الدخول.

##مختصر مدت والے رموز OAuth

لقد تم بالفعل استخدام Torii لمراجعي الرموز المميزة التي يمكنك تجربتها جربها وحدة التحكم في خادم OAuth. عندما لا يكون هناك أي متغيرات بيئة متاحة، يمكنك عرض أداة تسجيل الدخول لرمز الجهاز، ومختصر المدة والرموز المميزة لحاملها، ونموذج وحدة التحكم الذي يمكن إدخاله على شكل حرف krta.

| متغير | قصدي | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | نقطة نهاية ترخيص جهاز OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة نهاية الرمز المميز جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` قبول کرتا ہے | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth معاينة مستندات الجو | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | تسجيل الدخول کے دوران مانگے گئے النطاقات المحددة بمسافات | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | رمز مميز لجمهور واجهة برمجة التطبيقات | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | أتوقع أن أنتظر الفاصل الزمني للاستقصاء (مللي ثانية) | `5000` (قدرة < 5000 مللي ثانية رد ہوتی ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | رمز الجهاز کی نافذة انتهاء الصلاحية الاحتياطية (بالثواني) | `600` (300 ثانية و 900 ثانية للجلد) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | رمز الوصول کی عمر احتياطي (بالثواني) | `900` (300 ثانية و 900 ثانية للجلد) |
| `DOCS_OAUTH_ALLOW_INSECURE` | معاينات منخفضة لـ `1` من خلال فرض OAuth من خلال عملية الاختيار | _unset_ |

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

عند استخدام `npm run start` أو `npm run build`، قم بتضمين المدخل أو القيم `docusaurus.config.js`. معاينة لوکل کے دوران بطاقة جربها "تسجيل الدخول باستخدام رمز الجهاز" بٹن دکھاتا ہے۔ يقوم المستخدم بإدخال رمز صفحة التحقق من OAuth في أعلى البطاقة؛ تدفق الجهاز کامیاب ہونے کے بعد القطعة:

- جارى حامل الرمز المميز کو جربه وحدة التحكم فیلڈ میں حقن کرتا ہے،
- موجود `X-TryIt-Client` و`X-TryIt-Auth` الرؤوس التي ستطلب علامة كرتا،
- باقی ماندہ مدت دکھاتا ہے، و
- ختم الرمز المميز لسلامة بياناتك.

جهاز الإدخال اليدوي للحامل - متغيرات OAuth التي تحدد كيفية لصق المراجعين لعرض الرمز المميز الخاص بهم، أو `DOCS_OAUTH_ALLOW_INSECURE=1`، ومعاينات منخفضة معزولة وصول مجهول مقبول مقبول ہو۔ يقوم OAuth بإنشاء بوابة خارطة طريق DOCS-1b التي تفشل فورًا وتفشل.

ملاحظة: تعرض الصفحة الرئيسية للملف [قائمة التحقق من تشديد الأمان واختبار القلم](./security-hardening.md) ملاحظه؛ يتضمن نموذج التهديد، وCSP/الأنواع الموثوقة، وخطوات اختبار القلم كلًا من DOCS-1b وبوابة البطاقة.

## Norito-RPC نمونے

يطلب Norito-RPC وكيلًا خاصًا وسباكة OAuth للحصول على مسارات JSON؛ تم تخصيص بطاقة `Content-Type: application/x-norito` ومواصفات NRPC لبيانات الحمولة النافعة Norito المشفرة مسبقًا
(`docs/source/torii/nrpc_spec.md`)۔ يتم استخدام الإصدار `fixtures/norito_rpc/` الموجود ضمن الحمولات الأساسية الموجودة بالإضافة إلى مؤلفي البوابة الإلكترونية ومالكي SDK والمراجعين وإعادة تشغيل وحدات البايت في CI.

### جرب وحدة التحكم Norito الحمولة الصافية

1. `fixtures/norito_rpc/transfer_asset.norito` لاعبا اساسيا جيسا منتخب كريژ. هناك مغلفات خام Norito؛ لا يوجد ** ترميز base64 **.
2. يمكن لـ Swagger أو RapidDoc استخدام نقطة نهاية NRPC (مثل `POST /v2/pipeline/submit`) ومحدد **Content-Type** الذي هو `application/x-norito`.
3. طلب ​​محرر نص **ثنائي** للملف الأول (Swagger هو "ملف" أو RapiDoc هو محدد "Binary/File") و`.norito` متاح للتنزيل. بايتات القطعة للوكيل ذریعے بغداد تبدیلی کے دفق كرتا ہے.
4. طلب ​​مساعدة. إذا كنت Torii `X-Iroha-Error-Code: schema_mismatch`، قم بالتحقق من نقطة النهاية الخاصة بك وقبول الحمولات الثنائية وتسجيل `fixtures/norito_rpc/schema_hashes.json` تم إنشاء تجزئة المخطط في Torii.

تحتوي أحدث الرموز المميزة على رموز ترخيص مختلفة أو مضيفات Torii التي تحتوي على حمولة ساتية وحمولة إضافية. `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` يتضمن سير العمل خطة اعتماد NRPC-4 التي توفر حزمة أدلة (سجل + ملخص JSON) وتستعرض مراجعات Try It Response لقطة شاشة نقرة واحدة.

### مثال CLI (الضفيرة)

والتركيبات `curl` التي تعمل على إعادة تشغيل واجهة البوابة، حيث يقوم الوكيل بالتحقق من صحة استجابات البوابة أو البوابة وتصحيح الأخطاء:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

يتوفر `transaction_fixtures.manifest.json` كإدخال ثابت بدلاً من أو `cargo xtask norito-rpc-fixtures` مع تشفير الحمولة النافعة. يمكنك استخدام Torii في وضع الكناري `curl` وtry-it proxy (`https://docs.sora.example/proxy/v2/pipeline/submit`) للتنقل واختبار البنية التحتية واستخدام أدوات البوابة ہیں۔

## إمكانية الملاحظة والعملياتقم بطلب طريقة شريط، والمسار، والأصل، وحالة المنبع، ومصدر المصادقة (`override`، `default`، أو `client`) وقم بتسجيل الدخول. لم يتم تخزين الرموز المميزة - الرؤوس الحاملة وقيم `X-TryIt-Auth` التي تم تسجيلها بعد تنقيح البوابة، مما يسمح بإعادة توجيه المجمّع المركزي إلى الأمام من خلال تسرب أسرار الببغاء ے خدشے ے۔

### المسابر الصحية والتنبيه

عمليات النشر أثناء أو جدولة المسبار المجمع:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

مقابض البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری Torii مسار (بغير `/proxy`) جسے چیك کرنا ہو۔
- `TRYIT_PROXY_SAMPLE_METHOD` - ڈیفالٹ `GET`؛ اكتب المسارات لـ `POST` .
- `TRYIT_PROXY_PROBE_TOKEN` - نموذج استدعاء کے لئے عارض حامل الرمز المميز حقن کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - مهلة تصل إلى 5 ثوانٍ لتجاوز هذه المهلة.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus وجهة الملف النصي.
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` تتضمن مقاييس مفصولة بفواصل (ڈیفالٹ `job=tryit-proxy` و`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختيار عنوان URL لنقطة نهاية المقاييس (مثلاً `http://localhost:9798/metrics`) جو `TRYIT_PROXY_METRICS_LISTEN` فعال في الإجابة على السؤال.

نتائج جامع الملفات النصية تحقق نجاحًا كبيرًا، وتستكشف المسار القابل للكتابة للتمكن من التمكن منه
(مثلاً `/var/lib/node_exporter/textfile_collector/tryit.prom`) والتسميات المخصصة تشمل:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

المقاييس النصية التي تعيد كتابة الكرات ذريًا وتزيد من حجم الحمولة الإجمالية للمجمع.

عند تكوين `TRYIT_PROXY_METRICS_LISTEN` أو `TRYIT_PROXY_PROBE_METRICS_URL`، قد يفشل مسبار نقطة نهاية المقاييس في مسبار الشبكة أو إذا كشط السطح (على سبيل المثال، دخول تم تكوينه بشكل خاطئ أو قواعد جدار الحماية المفقودة). یک إعداد الإنتاج النموذجي ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

يتم تحديث قائمة التنبيهات من خلال مسبار مراقبة مكدس المراقبة. Prometheus مثال على سلسلة من حالات الفشل بعد الصفحة التالية:

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

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (أو المضيف/المنفذ) يتم تشغيل الخادم الوكيل وبدء تشغيل الوكيل باستخدام Prometheus لنقطة نهاية المقاييس المنسقة. المسار ڈفالٹ `/metrics` و`TRYIT_PROXY_METRICS_PATH=/custom` هو أحدث إصدار. استخلاص الإجماليات حسب الطريقة، وحالات رفض الحد الأقصى للمعدل، والأخطاء الأولية/المهلات، ونتائج الوكيل، وملخصات زمن الوصول:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

يقوم جامعو Prometheus/OTLP بمقاييس نقطة النهاية للسرعة و`dashboards/grafana/docs_portal.json` من اللوحات الموجودة التي تعيد استخدام زمن الاستجابة لذيل SRE وارتفاعات الرفض وتحليل السجلات بشكل كبير. جهاز الوكيل `tryit_proxy_start_timestamp_ms` نشر الكرتا وإعادة تشغيل المشغلين والكشف عن الكرتا.

### أتمتة التراجع

يستخدم مساعد الإدارة Torii عنوان URL المستهدف لتحديث أو استعادة أي شيء. يحتوي تكوين النص البرمجي `.env.tryit-proxy.bak` على مفتاح التراجع وهو أمر رائع.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

إذا تم ضبط تكوين النشر والملف على `--env` أو `TRYIT_PROXY_ENV`، فسيتم تجاوز التجاوز.