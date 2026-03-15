---
lang: ar
direction: rtl
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: دليل Sandbox «جرّبها» (Try It)
summary: كيفية تشغيل بروكسي Torii لبيئة staging وبيئة sandbox في بوابة المطورين.
---

توفر بوابة المطورين (Developer Portal) وحدة تحكم “Try it” لواجهة Torii REST. يشرح هذا
الدليل كيفية تشغيل البروكسي الداعم وربط وحدة التحكم ببوابة staging من دون كشف
البيانات السرية (credentials).

## المتطلبات المسبقة

- نسخة من مستودع Iroha (جذر الـ workspace).
- Node.js 18.18 أو أحدث (يتوافق مع baseline الخاص بالبوابة).
- Endpoint لـ Torii يمكن الوصول إليه من جهازك (staging أو محلي).

## 1. توليد Snapshot لـ OpenAPI (اختياري)

تعيد وحدة التحكم استخدام نفس payload الخاص بـ OpenAPI المستخدم في صفحات المرجع
(reference) التابعة للبوابة. إذا قمت بتعديل مسارات Torii، أعد توليد الـ snapshot:

```bash
cargo xtask openapi
```

تقوم هذه المهمة بكتابة الملف `docs/portal/static/openapi/torii.json`.

## 2. تشغيل بروكسي Try It

من جذر المستودع:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# قيم افتراضية اختيارية
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### متغيرات البيئة

| المتغير | الوصف |
|---------|-------|
| `TRYIT_PROXY_TARGET` | عنوان Torii الأساسي (مطلوب). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة origins مفصولة بفواصل، مسموح لها باستخدام البروكسي (القيمة الافتراضية `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Bearer token اختياري يُطبَّق افتراضيًا على جميع الطلبات التي تمر عبر البروكسي. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | عيّن القيمة `1` لتمرير ترويسة `Authorization` الصادرة من العميل كما هي. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | إعدادات rate limiter في الذاكرة (الافتراضي: 60 طلبًا كل 60 ثانية). |
| `TRYIT_PROXY_MAX_BODY` | الحد الأقصى لحجم الـ payload المسموح به (بالبايت، افتراضيًا 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة (timeout) طلبات Torii upstream (الافتراضي 10 000 ملِّي ثانية). |

يوفّر البروكسي:

- `GET /healthz` — فحص جاهزية (readiness).
- `/proxy/*` — طلبات proxied مع الاحتفاظ بمسار (path) الطلب وسلسلة الاستعلام (query string).

## 3. تشغيل البوابة

في Terminal آخر:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

انتقل إلى `http://localhost:3000/api/overview` واستخدم وحدة التحكم Try It. يتم ضبط
نفس متغيرات البيئة لتكوين الـ Swagger UI وواجهات RapiDoc المضمّنة.

## 4. تشغيل اختبارات الوحدة (Unit Tests)

يوفّر البروكسي مجموعة سريعة من اختبارات الوحدة مكتوبة بـ Node:

```bash
npm run test:tryit-proxy
```

تغطي هذه الاختبارات تحليل العناوين (address parsing)، التعامل مع الـ origins،
الـ rate limiting، وحقن (injection) الـ bearer token.

## 5. أتمتة الاختبارات والقياسات

استخدم سكربت الـ probe المرفق للتحقق من ‎`/healthz` ومسار تجريبي:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

أهم متغيرات البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` — مسار Torii اختياري (من دون ‎`/proxy`) ترغب في اختباره.
- `TRYIT_PROXY_SAMPLE_METHOD` — القيمة الافتراضية ‎`GET`؛ غيّرها إلى ‎`POST` لمسارات الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` — يحقن bearer token مؤقتًا لطلب العينة.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — يستبدل مهلة الـ 5 ثوانٍ الافتراضية.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — ملف بصيغة Prometheus textfile لقياسات ‎`probe_success` و ‎`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — قائمة `مفتاح=قيمة` مفصولة بفواصل تُضاف إلى الـ labels (القيمة الافتراضية `job=tryit-proxy` و `instance=<عنوان البروكسي>`).

عند تفعيل `TRYIT_PROXY_PROBE_METRICS_FILE` يعيد السكربت كتابة الملف بشكل ذري، ما يضمن
أن يقرأ node_exporter/textfile collector payload كاملًا. مثال:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

مرّر القياسات إلى Prometheus واستفد من قاعدة التنبيه الموجودة لإطلاق إنذار عندما تصبح
قيمة ‎`probe_success` مساوية للصفر.

## 6. قائمة التحقق لتقوية الإعداد في الإنتاج

قبل نشر البروكسي خارج بيئة التطوير المحلية:

- أنهِ (terminate) اتصال TLS قبل البروكسي (من خلال reverse proxy أو بوابة مدارة).
- اضبط logging منظّم (structured) ومرّره إلى أنظمة المراقبة (observability pipelines).
- قم بتدوير (rotate) الـ bearer tokens بشكل دوري وخزنها في مدير أسرار (secrets manager).
- راقب endpoint `/healthz` الخاص بالبروكسي واجمع metrics زمن الاستجابة (latency).
- اجعل حدود الـ rate متماشية مع حصص Torii في بيئة staging؛ اضبط سلوك `Retry-After`
  لنقل معلومات الـ throttling إلى العملاء بشكل واضح.

</div>
