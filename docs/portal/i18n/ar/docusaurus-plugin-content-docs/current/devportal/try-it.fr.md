---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# باك السمور جربه

يوفر تطوير البوابة خيار وحدة التحكم "Try it" لاستدعاء نقاط النهاية Torii دون إنهاء الوثائق. تقوم وحدة التحكم بترحيل الطلبات عبر الوكيل لمنع المتصفحين من تحديد حدود CORS وتطبيق تحديد المعدل والمصادقة.

## المتطلبات الأساسية

- Node.js 18.18 ou plus الأحدث (يتوافق مع متطلبات بناء البوابة)
- الوصول إلى بيئة التدريج Torii
- رمز مميز لحامل قادر على استدعاء المسارات Torii الذي تريد اختباره

قم بتمرير تكوين الوكيل وفقًا لمتغيرات البيئة. تسرد اللوحة هذه المقابض والأهم:

| متغير | موضوعي | افتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii de base vers laquelle le proxy relaye les requetes | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | عنوان البريد الإلكتروني للتطوير المحلي (تنسيق `host:port` أو `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة منفصلة حسب الأصول الأصلية المرخصة لاستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | مكان التعريف في `X-TryIt-Client` لكل طلب المنبع | `docs-portal` |
| `TRYIT_PROXY_BEARER` | الرمز المميز لحامل التتابع الافتراضي مقابل Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | السماح للمستخدمين النهائيين بتزويد الرمز المميز الخاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الأقصى لحجم الطلب (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة المنبع بالمللي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | طلبات ترخيص تلقائية من خلال نافذة الحماية لعميل IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة لامعة لتحديد معدل الصب (مللي ثانية) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان الخيار البيئي لنقطة نهاية المقاييس النمطية Prometheus (`host:port` أو `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | خدمة HTTP من خلال نقطة نهاية المقاييس | `/metrics` |

يكشف الوكيل أيضًا `GET /healthz`، ويتسبب في أخطاء بنيات JSON، ويخفي حامل الرموز المميزة في السجلات.

Activez `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` عندما تعرض الوكيل لمستخدمى المستندات حتى تتمكن لوحات Swagger وRapiDoc من ترحيل حاملي الرموز المميزة من قبل المستخدم. يطبق الوكيل طوال الوقت حدود الاستخدام، ويخفي بيانات الاعتماد، ويسجل إذا طلب استخدام الرمز المميز افتراضيًا أو رسومًا إضافية حسب الطلب. تكوين `TRYIT_PROXY_CLIENT_ID` مع التسمية التي تريد إرسالها مثل `X-TryIt-Client`
(الاسم الافتراضي `docs-portal`). الوكيل يقوم بالتحقق من القيم `X-TryIt-Client` الأربعة من جانب المستأنف، ثم يعود إلى هذا الوضع الافتراضي حتى تتمكن بوابات التدريج من مراجعة المصدر بدون ربط بيانات المتصفح.

## إنشاء الوكيل المحلي

قم بتثبيت التبعيات عند التكوين الأول للبوابة:

```bash
cd docs/portal
npm install
```

قم بتمرير الوكيل ونقطة المثيل مقابل Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

يقوم البرنامج النصي بوضع العنوان وإرسال الطلبات من `/proxy/*` إلى الأصل Torii الذي تم تكوينه.

قبل أن يكون مأخذ التوصيل صالحًا للنص
`static/openapi/torii.json` يتوافق مع ملخص التسجيل في
`static/openapi/manifest.json`. إذا كانت الملفات متباعدة، فسيصدر الأمر صدى مع أحد
خطأ وتطلب تنفيذ `npm run sync-openapi -- --latest`. تصدير
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فريد من أجل تجاوزات الطوارئ؛ لو لو الوكيل
تحذير واستمر في السماح لك باستعادة نوافذ الصيانة.

## كابل الحاجيات دو Portail

عندما تقوم بإنشاء أو تطوير البوابة، قم بتحديد عنوان URL الذي ستفعله الأدوات
استخدم للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

تستمع المكونات التالية إلى هذه القيم من `docusaurus.config.js`:

- **Swagger UI** - يقدم `/reference/torii-swagger`؛ التهيئة المسبقة لحامل المخطط
  عندما يكون الرمز المميز موجودًا، قم بوضع الطلبات مع `X-TryIt-Client`، وحقن
  `X-TryIt-Auth`، وأعد كتابة المكالمات عبر الوكيل عندما
  تم تعريف `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - أعد `/reference/torii-rapidoc`; إعادة تمثيل رمز البطل،
  أعد استخدام رؤوس الميمات التي تحتوي على لوحة Swagger، واتصل تلقائيًا
  الوكيل عندما يتم تكوين عنوان URL.
- **Try it console** - integree sur la page d'overview API; تصريح المبعوث des
  الطلبات الشخصية، والنظر في الرؤوس، وتفقد مجموعة الاستجابة.

تعرض الشاشتان **محدد اللقطات** المضاء
`docs/portal/static/openapi/versions.json`. قم بإعادة ملء الفهرس بـ cet
`npm run sync-openapi -- --version=<label> --mirror=current --latest` للتعرف على المراجعين
تمرير قوي بين المواصفات التاريخية، ومشاهدة ملخص SHA-256، والتأكيد
إذا كانت لقطة الإصدار ستؤدي إلى ظهور علامة واضحة قبل استخدام الأدوات التفاعلية.

تغيير الرمز المميز في عنصر واجهة المستخدم لا يلمس مسار التنقل في الجلسة؛ لو الوكيل ني
استمر في التشويش ولا يوجد أي تشويش في الرمز المميز.

## مدة صلاحية رموز OAuth

لتجنب توزيع الرموز المميزة Torii لفترة طويلة من قبل المراجعين، حرر وحدة التحكم جربها
خادم OAuth الخاص بك. عندما تكون متغيرات البيئة موجودة في الباب
قم بإنشاء عنصر واجهة مستخدم لرمز جهاز تسجيل الدخول، وإصدار الرموز المميزة لحاملها لمدة طويلة، وإدخالها
أتمتة في صيغة وحدة التحكم.

| متغير | موضوعي | افتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | جهاز تفويض نقطة النهاية OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | رمز نقطة النهاية الذي يقبل `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | سجل عميل OAuth المعرف لمعاينة المستندات | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | نطاقات تحدد حسب المساحات المطلوبة عند تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | خيار Audience API لتكوين الرمز المميز | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | الحد الأدنى للفاصل الزمني للاستقصاء لانتظار الموافقة (مللي ثانية) | `5000` (قيم أقل من 5000 مللي ثانية) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | رمز جهاز انتهاء الصلاحية (ثواني) | `600` (افعل ذلك بين 300 ثانية و900 ثانية) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duree de vie رمز الوصول (ثواني) | `900` (افعل ذلك بين 300 ثانية و900 ثانية) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Mettre `1` للمعاينة المحلية التي تساعد على فرض OAuth بشكل مقصود | _unset_ |

مثال على التكوين:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

عندما تقوم برمي `npm run start` أو `npm run build`، فإن البوابة متكاملة بهذه القيم
في `docusaurus.config.js`. في معاينة المنطقة الانتقائية، جرب عرض زر
"تسجيل الدخول باستخدام رمز الجهاز". سيخبر المستخدمون الرمز المعروض على صفحتك OAuth؛ مرة واحدة يقوم تدفق الجهاز بإعادة استخدام القطعة:

- قم بإدخال الرمز المميز لحامله في وحدة تحكم البطل جربه،
- قم بإضافة الطلبات باستخدام الرؤوس الموجودة `X-TryIt-Client` و`X-TryIt-Auth`،
- عرض مدة الراحة، وآخرون
- مسح الرمز المميز تلقائيًا عند انتهاء صلاحيته.

L'entree manuelle Bearer متاح؛ قم بإزالة متغيرات OAuth عندما تريد ذلك
إجبار المراجعين على تجميع رمز مميز مؤقت مثل الميمات أو تصديرها
`DOCS_OAUTH_ALLOW_INSECURE=1` للمعاينة المحلية المعزولة أو الوصول المجهول
مقبول. تقوم عمليات البناء بدون OAuth بتكوين صدى صيانة سريع لتحقيق الرضا
لإرضاء بوابة خريطة الطريق DOCS-1b.

ملحوظة: راجع [قائمة التحقق من تقوية الأمان واختبار القلم](./security-hardening.md)
قبل عرض الباب خارج العمل؛ elle documente لو نموذج التهديد,
ملف تعريف CSP/الأنواع الموثوقة، وخطوات اختبار القلم التي تمنع صيانة DOCS-1b.

## أمثلة Norito-RPC

تتضمن الطلبات Norito-RPC الوكيل الوكيل والسباكة OAuth للمسارات JSON؛
إنها ببساطة `Content-Type: application/x-norito` وتتطلب الحمولة Norito
تم فك التشفير مسبقًا في مواصفات NRPC
(`docs/source/torii/nrpc_spec.md`).
يوفر المستودع الحمولات الأساسية `fixtures/norito_rpc/` لمؤلفيها
يمكن للبوابة وأصحاب SDK والمراجعين تجديد وحدات البايت الدقيقة التي تستخدم CI.

### Envoyer un payload Norito من وحدة التحكم جربها

1. اختر وحدة تثبيت مثل `fixtures/norito_rpc/transfer_asset.norito`. سيس
   الملفات هي مغلفات Norito bruts؛ **ne** les base64-encodez pas.
2. في Swagger أو RapidDoc، قم بتحديد نقطة النهاية NRPC (على سبيل المثال
   `POST /v2/pipeline/submit`) واضغط على محدد **Content-Type** على
   `application/x-norito`.
3. قم بسحب محرر النص إلى **binary** (وضع "ملف" في Swagger أو
   حدد "Binary/File" من RapiDoc) ثم قم بتحميل الملف `.norito`. القطعة لو
   إرسال البايتات عبر الوكيل بدون تغيير.
4. قم بالطلب. سي Torii رينفوي `X-Iroha-Error-Code: schema_mismatch`,
   تحقق من أنك تتصل بنقطة نهاية تقبل الحمولات الثنائية وتؤكدها
   سيتم تسجيل تجزئة المخطط في `fixtures/norito_rpc/schema_hashes.json`
   تتوافق مع بناء Torii cible.

تحتفظ وحدة التحكم بالملف الأحدث والذاكرة حتى تتمكن من استرجاعه
meme payload tout en testing مختلفة الرموز المميزة للتفويض أو المضيفين Torii. اجووتر
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` حزمة منتج سير العمل الخاص بك
مرجع مسبق في خطة اعتماد NRPC-4 (سجل + استئناف JSON)، وهذا أمر جيد
مع التقاط شاشة الاستجابة، جربها عند المراجعات.

### مثال CLI (الضفيرة)يمكن أن تكون تركيبات الميمات ممتعة بمجرد فتحها عبر `curl`، وهذا أمر مفيد
عند التحقق من الوكيل أو تصحيح بوابة الردود:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

استبدل التركيب الذي تريده داخل القائمة في `transaction_fixtures.manifest.json`
أو قم بتشفير الحمولة الخاصة بك مع `cargo xtask norito-rpc-fixtures`. Quand Torii est en
وضع الكناري يمكنك وضع المؤشر `curl` مقابل الوكيل حاول تجربته
(`https://docs.sora.example/proxy/v2/pipeline/submit`) لممارسة البنية التحتية للميمات
يتم استخدام عناصر واجهة المستخدم الخاصة بالبوابة.

## الملاحظة والعمليات

كل طلب هو عبارة عن طريقة ومسار وأصل وحالة المنبع والمصدر
المصادقة (`override`، `default` أو `client`). الرموز المميزة ليست مجرد أسهم جماعية: les
الرؤوس وحاملها والقيم `X-TryIt-Auth` هي عبارة عن عمليات إعادة تسجيل مسبقة، حتى تتمكن من التمكن من ذلك
Relayer stdout vers un Collecteur Central sans craindre des fuites.

### مجسات الصحة والتنبيهات

قم بتمرير المسبار أثناء عمليات النشر أو ضمن جدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

مقابض البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - المسار Torii optionnelle (بدون `/proxy`) ممارس.
- `TRYIT_PROXY_SAMPLE_METHOD` - الاسم الافتراضي `GET`؛ حدد `POST` لمسارات الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - أدخل رمزًا مميزًا لحامل مؤقت لاستدعاء المثال.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - قم بإلغاء المهلة الافتراضية لمدة 5 ثوانٍ.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - نص الوجهة Prometheus خيار لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - أزواج `key=value` منفصلة عن بعضها البعض بواسطة المقاييس الإضافية (افتراضيًا `job=tryit-proxy` و`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - خيار عنوان URL لنقطة نهاية المقاييس (على سبيل المثال، `http://localhost:9798/metrics`) الذي يتم الرد عليه بنجاح عندما يكون `TRYIT_PROXY_METRICS_LISTEN` نشطًا.

قم بإدخال النتائج في مُجمع ملف نصي يشير إلى المسبار نحو طريق قابل للكتابة
(على سبيل المثال، `/var/lib/node_exporter/textfile_collector/tryit.prom`) بالإضافة إلى الملصقات
يشخصن:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يقوم النص بكتابة ملف مقاييس القوة الذرية ليقرأه جامعك
toujours un payload كاملة.

تم تكوين وتعريف Quand `TRYIT_PROXY_METRICS_LISTEN`
`TRYIT_PROXY_PROBE_METRICS_URL` على نقطة نهاية المقاييس بحيث يقوم المسبار بالصدى السريع
يتباين سطح الخدش (على سبيل المثال، عدم تكوين الدخول أو ضبط قواعد جدار الحماية).
Un regage production typique est
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

من أجل التنبيهات القانونية، قم بتوصيل المسبار بمجموعة المراقبة الخاصة بك. مثال Prometheus
qui page apres deux echecs consecutifs:

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

قم بتعريف `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (أو كل المضيف/المنفذ) مسبقًا
أطلق الوكيل لكشف نقطة نهاية للقياسات بتنسيق Prometheus. لو شيمين
الاسم الافتراضي هو `/metrics` ويمكن استبداله على قدم المساواة `TRYIT_PROXY_METRICS_PATH=/custom`. تشاك
التخلص من جميع أجهزة الكمبيوتر وفقًا للطريقة، والحد الأقصى لمعدل الرفض، والأخطاء/المهلات
المنبع، وكيل النتائج واستئناف الكمون:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

قم بتوجيه جامعي Prometheus/OTLP إلى نقطة نهاية المقاييس وإعادة استخدام اللوحات
الموجودين في `dashboards/grafana/docs_portal.json` حتى يتمكن SRE من ملاحظة فترات استجابة قائمة الانتظار
et les pics de rejet sans parser les logs. يتم نشر الوكيل تلقائيًا
`tryit_proxy_start_timestamp_ms` لمساعدة المشغلين على اكتشاف عمليات إعادة التشغيل.

### أتمتة التراجع

استخدم مساعد الإدارة للمحافظة على اليوم أو استعادة عنوان URL cible Torii. لو البرنامج النصي
مخزون التكوين السابق في `.env.tryit-proxy.bak` يجعل عمليات التراجع مستمرة
أمر وحيد.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

قم بزيادة شحن شريط الملف باستخدام `--env` أو `TRYIT_PROXY_ENV` في حالة النشر
مخزون خيارات التكوين.