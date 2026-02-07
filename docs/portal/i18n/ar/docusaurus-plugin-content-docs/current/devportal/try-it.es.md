---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox جربه

تشتمل بوابة المطورين على وحدة تحكم اختيارية "Try it" لتتمكن من الاتصال بنقاط نهاية Torii بدون إزالة المستندات. تتضمن وحدة التحكم إعادة إرسال الطلبات عبر الوكيل حتى يتمكن المتصفحون من تجنب حدود CORS بينما يلتزمون بحدود المهام والمصادقة.

## المتطلبات الأساسية

- Node.js 18.18 أو جديد (يتوافق مع متطلبات إنشاء البوابة)
- الوصول إلى اللون الأحمر في مقدمة العرض Torii
- رمز مميز يمكنك من خلاله استدعاء المسار Torii الذي يمكنك تشغيله

يتم تنفيذ كل تكوين الوكيل عبر متغيرات التشغيل. الجدول التالي هو أهم المقابض:

| متغير | اقتراح | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | قاعدة URL الخاصة بـ Torii التي يقدمها الوكيل لطلبات | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | اتجاه الصوت للتصميم المحلي (التنسيق `host:port` أو `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة منفصلة حسب الأصول التي يمكن الاتصال بها بواسطة الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | المعرف المحدد في `X-TryIt-Client` لكل طلب من المنبع | `docs-portal` |
| `TRYIT_PROXY_BEARER` | الرمز المميز لحامله من أجل إرجاع Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | السماح للمستخدمين بتوفير الرمز المميز الخاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | حجم الطلب الأقصى (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | المهلة المنبع في milisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | الطلبات المسموح بها لنافذة العمل لـ IP للعميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | فتح النافذة لتحديد المعدل (مللي ثانية) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان اختياري لنقطة نهاية المقاييس بأسلوب Prometheus (`host:port` أو `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | يتم استخدام Ruta HTTP من خلال نقطة نهاية القياسات | `/metrics` |

يعرض الوكيل أيضًا `GET /healthz`، ويتسبب في أخطاء إنشاءات JSON ويحمل الرموز المميزة لإخراج السجلات.

نشط `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` لتنشيط الوكيل لمستخدمي المستندات حتى تتمكن لوحات Swagger وRapiDoc من إعادة حامل الرموز المميزة المناسبة للمستخدم. الوكيل عبارة عن تطبيق له حدود في التكلفة وبيانات الاعتماد المخفية والتسجيل في حالة طلب استخدام الرمز المميز بسبب عيب أو إلغاء بسبب الطلب. تكوين `TRYIT_PROXY_CLIENT_ID` بالعلامة التي تريد إرسالها مثل `X-TryIt-Client`
(بسبب عيب `docs-portal`). يتم تسجيل الوكيل والتحقق من القيم `X-TryIt-Client` من قبل العميل، وينتقل إلى هذا الإعداد الافتراضي حتى تتمكن بوابات التدريج من مراجعة الإجراءات دون ربط بيانات التعريف الخاصة بالمتصفح.

## بدء الوكيل المحلي

قم بتثبيت التبعيات أولاً عندما تقوم بتكوين البوابة الإلكترونية:

```bash
cd docs/portal
npm install
```

قم بتشغيل الوكيل وتثبيته على مثيلك Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

يقوم البرنامج النصي بتسجيل الاتجاه المحذوف وإعادة تقديم الطلبات من `/proxy/*` إلى الأصل Torii الذي تم تكوينه.

قبل توسيع المقبس النصي صالح لذلك
`static/openapi/torii.json` يتزامن مع الملخص المسجل
`static/openapi/manifest.json`. إذا تمت إزالة الملفات المحذوفة، فسيتم إنهاء الأمر بواحدة
خطأ ويشير إلى تشغيل `npm run sync-openapi -- --latest`. اكسبورتا
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` منفردًا لتجاوز حالات الطوارئ؛ تسجيل الوكيل واحد
الإعلان والاستمرار حتى تتمكن من التعافي أثناء فترات الصيانة.

## قم بتوصيل عناصر واجهة المستخدم للبوابة

عندما تقوم ببناء أو إنشاء بوابة المطورين، قم بتحديد عنوان URL للعناصر المصغّرة
deben usar para el proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

المكونات التالية لها هذه القيم من `docusaurus.config.js`:

- **Swagger UI** - تم عرضه على `/reference/torii-swagger`; preautoriza el esquema
  عندما يكون حامل الرمز المميز، يلصق الطلبات على `X-TryIt-Client`،
  قم بإدخال `X-TryIt-Auth`، وأعد كتابة المكالمات عبر الوكيل أثناء ذلك
  `TRYIT_PROXY_PUBLIC_URL` هذا محدد.
- **RapiDoc** - تم عرضه على `/reference/torii-rapidoc`; Refleja El Campo de Token,
  قم بإعادة استخدام نفس الرؤوس الموجودة في لوحة Swagger، ودعم الوكيل
  يتم تلقائيًا عندما يتم تكوين عنوان URL.
- **Try it console** - قم بتضمين صفحة نظرة عامة على واجهة برمجة التطبيقات (API)؛ سمح بالحسد
  طلبات مخصصة، عرض الرؤوس والتحقق من حجم الاستجابة.

تعرض العديد من اللوحات **محدد اللقطات** الذي تريده
`docs/portal/static/openapi/versions.json`. Llena ese مؤشر يخدع
`npm run sync-openapi -- --version=<label> --mirror=current --latest` للمراجعين
يمكنك الاطلاع على المواصفات التاريخية، والاطلاع على ملخص SHA-256 المسجل، وتأكيد ما إذا كان لديك
لقطة الإصدار هي عبارة عن بيان ثابت قبل استخدام الأدوات التفاعلية.

تغيير الرمز المميز في أي عنصر واجهة مستخدم يؤثر فقط على جلسة المتصفح الفعلية؛ الوكيل الوحيد
استمر في عدم تسجيل الرمز المميز المناسب.

## الرموز المميزة لـ OAuth de corta vida

لتجنب توزيع الرموز المميزة Torii لفترة طويلة على المراجعين، قم بتوصيل وحدة التحكم جربها
خادم OAuth الخاص بك. عندما تعرض متغيرات الجزء العلوي من البوابة عرضًا واحدًا
أداة تسجيل الدخول مع رمز الجهاز، وإنشاء الرموز المميزة لحاملها، وإدخالها تلقائيًا
في صيغة وحدة التحكم.

| متغير | اقتراح | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | نقطة نهاية ترخيص جهاز OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة النهاية للرمز المميز الذي يقبل `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة المستندات | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | نطاقات منفصلة حسب المساحات المطلوبة أثناء تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | جمهور API اختياري للتغلب على الرمز المميز | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | الحد الأدنى من الفاصل الزمني للاستقصاء مع توقع الاحتمال (مللي ثانية) | `5000` (القيم < 5000 مللي ثانية في حالة إعادة التجهيز) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | نافذة انتهاء صلاحية رمز الجهاز (الثواني) | `600` (يجب الاستمرار فيه بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | مدة رمز الوصول (الثواني) | `900` (يجب الاستمرار فيه بين 300 ثانية و 900 ثانية) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Pon `1` لمعاينة المواقع التي تحذف فرض OAuth بشكل متعمد | _unset_ |

مثال التكوين:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

عندما يتم تشغيل `npm run start` أو `npm run build`، فإن البوابة تضيف هذه القيم
أون `docusaurus.config.js`. Durante un Preview local la tarjeta جربها مرة واحدة
زر "تسجيل الدخول باستخدام رمز الجهاز". يقوم المستخدمون بإدخال الرمز الذي يظهر في صفحة التحقق من OAuth؛ بمجرد خروج الجهاز من القطعة:

- أدخل الرمز المميز لحامله في مجال وحدة التحكم جربه،
- تذكير الطلبات بالرؤوس الموجودة `X-TryIt-Client` و`X-TryIt-Auth`،
- muestra el timepo de vida Restante، y
- يتم حفظ الرمز تلقائيًا عند انتهاء الصلاحية.

يمكن الوصول إلى حامل الدليل يدويًا; حذف متغيرات OAuth عندما تريد
قم بإرسال المراجعين إلى ربط رمز مؤقت بنفسهم، أو التصدير
`DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينة المواقع المحلية دون الوصول المجهول
مقبول. يتم إنشاء الإنشاءات دون تكوين OAuth بعد الآن بسرعة لإرضاء البوابة
خريطة الطريق DOCS-1b.

ملحوظة: قم بمراجعة [قائمة التحقق من التعزيز الأمني واختبار القلم](./security-hardening.md)
قبل فتح بوابة المختبر المستقبلية؛ نموذج دوكومنتا التهديد,
ملف CSP/الأنواع الموثوقة، وخطوات اختبار القلم التي تمنع الآن DOCS-1b.

## مويسترا Norito-RPC

تشمل الطلبات Norito-RPC نفس الوكيل والسباكة OAuth التي تقوم بتدوير JSON؛
قم بتكوين `Content-Type: application/x-norito` ببساطة وقم بإرسال الحمولة Norito
تم وصفه مسبقًا في مواصفات NRPC
(`docs/source/torii/nrpc_spec.md`).
يتضمن المستودع حمولات Canonicos bajo `fixtures/norito_rpc/` لمسؤولي البوابة،
يمكن لمالكي SDK والمراجعين إعادة إنتاج وحدات البايت الدقيقة التي تستخدمها CI.

### أرسل حمولة Norito من وحدة التحكم جربها

1. قم بتثبيت وحدة تثبيت مثل `fixtures/norito_rpc/transfer_asset.norito`. إستوس
   أرشيفات ابن المغلفات Norito مباشرة؛ **لا** الرموز الموجودة في base64.
2. في Swagger أو RapidDoc، قم بتعيين نقطة النهاية NRPC (على سبيل المثال
   `POST /v1/pipeline/submit`) وتغيير المحدد **نوع المحتوى** a
   `application/x-norito`.
3. تغيير محرر جسم الطلب إلى **ثنائي** (وضع "ملف" من Swagger أو
   محدد "Binary/File" من RapiDoc) وشحن الملف `.norito`. القطعة
   إرسال البايتات عبر الوكيل دون تغيير.
4. أرسل الطلب. سي Torii devuelve `X-Iroha-Error-Code: schema_mismatch`,
   التحقق من أن هذا الاتصال هو نقطة نهاية تقبل الحمولات الثنائية والتأكيد
   تم تسجيل تجزئة المخطط في `fixtures/norito_rpc/schema_hashes.json`
   يتزامن مع الإصدار Torii الذي هو مستخدم.

تحافظ وحدة التحكم على الأرشيف بشكل أحدث في الذاكرة حتى تتمكن من استعادة نفس الشيء
الحمولة أثناء اختبار رموز الترخيص المختلفة أو مضيفي Torii. أجريجار
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ينتج سير العمل الخاص بك الحزمة
الأدلة المرجعية في خطة الاعتماد NRPC-4 (سجل + استئناف JSON)، والتي يتم دمجها
يمكنك التقاط لقطات شاشة من الرد على Try It خلال المراجعات.

### مثال CLI (تجعيد)يمكن إعادة إنتاج التركيبات نفسها من خلال البوابة عبر `curl`، ما هو مفيد
عند التحقق من صحة الوكيل أو استجابة البوابة:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

تغيير التركيب لأي قائمة مدخلة في `transaction_fixtures.manifest.json`
قم بتدوين الحمولة الخاصة بك مع `cargo xtask norito-rpc-fixtures`. Cuando Torii esta en
وضع الكناري puedes apuntar `curl` البروكسي جربه
(`https://docs.sora.example/proxy/v1/pipeline/submit`) لتشغيل نفس الشيء
البنية التحتية التي تستخدم عناصر واجهة المستخدم للبوابة.

## إمكانية الملاحظة والعمليات

يتم تسجيل كل طلب مرة باستخدام الطريقة والمسار والأصل والحالة عند المنبع والمصدر
المصادقة (`override`، `default` أو `client`). الرموز المميزة غير موجودة في مكانها: تانتو
يتم إعادة تحميل الرؤوس مثل القيم `X-TryIt-Auth` قبل التسجيل،
كما يمكنك إعادة إنشاء مجمع مركزي دون الحاجة إلى الترشيح.

### مجسات الصحة والتنبيهات

قم بتشغيل المسبار بما في ذلك خلال فترات زمنية محددة أو في جدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

مقابض الداخل:

- `TRYIT_PROXY_SAMPLE_PATH` - ruta Torii اختياري (sin `/proxy`) للقذف.
- `TRYIT_PROXY_SAMPLE_METHOD` - بسبب العيب `GET`؛ حدد `POST` لمسارات الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - أدخل رمزًا مميزًا مؤقتًا لاستدعاء الشاشة.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - قم بكتابة المهلة لمدة 5 ثوانٍ.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - هذا نص اختياري Prometheus لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - متساوي `key=value` مفصول بفواصل متصلة بالمقاييس (من خلال خلل `job=tryit-proxy` و`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - عنوان URL اختياري لنقطة نهاية المقاييس (على سبيل المثال، `http://localhost:9798/metrics`) الذي يجب الرد عليه بالخروج عندما يكون `TRYIT_PROXY_METRICS_LISTEN` مؤهلاً.

قم بتغذية النتائج في مجمع ملفات نصية خلف التحقيق في طريق مكتوب
(على سبيل المثال، `/var/lib/node_exporter/textfile_collector/tryit.prom`) وتوافق على الآداب
تخصيص:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يقوم البرنامج النصي بإعادة كتابة ملف المقاييس الذرية حتى تتمكن من جمع البيانات دائمًا
حمولة كاملة.

عندما يتم تكوين `TRYIT_PROXY_METRICS_LISTEN`، حدد
`TRYIT_PROXY_PROBE_METRICS_URL` نقطة النهاية للقياسات حتى يفشل المسبار بسرعة إذا حدث ذلك
اختفى سطح الكشط (على سبيل المثال، الدخول بشكل غير صحيح أو إعدادات جدار الحماية الخاطئة).
نوع عادل من الإنتاج
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

لتنبيهات الحياة، قم بتوصيل المسبار بمكدس الشاشة الخاص بك. مثال على Prometheus
الصفحة بعد سقوطين متتاليين:

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

### نقطة النهاية للمقاييس ولوحات المعلومات

تكوين `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (أي مضيف/منفذ) قبل ذلك
بدء الوكيل لتوضيح نقطة نهاية للمقاييس بالتنسيق Prometheus. لا روتا
بسبب الخلل هو `/metrics` ولكن يمكن تغييره مع `TRYIT_PROXY_METRICS_PATH=/custom`. كادا
التخلص من عدادات إجمالية من خلال الطريقة، والإرجاع من خلال الحد الأقصى للمعدل، والأخطاء/المهلات
المنبع، نتائج الوكيل واستئناف التأخير:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

قم بجمع أدوات التجميع Prometheus/OTLP من خلال قياسات نقطة النهاية وإعادة استخدام اللوحات الموجودة
en `dashboards/grafana/docs_portal.json` حتى يتمكن SRE من ملاحظة زمن انتقال الكولا والصور
rechazo sin سجلات التحليل. يتم نشر الوكيل تلقائيًا `tryit_proxy_start_timestamp_ms`
لمساعدة المشغلين على الكشف عن الأنشطة.

### أتمتة التراجع

استخدم مساعد الإدارة لتحديث أو استعادة عنوان URL الهدف Torii. البرنامج النصي
قم بحماية التكوين السابق في `.env.tryit-proxy.bak` حتى تتم عمليات الاستعادة
كوماندوز منفرد.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

قم بكتابة مسار الملف مع `--env` أو `TRYIT_PROXY_ENV` إذا قمت بنشره
حماية التكوين في مكان آخر.