---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox جربه

تشتمل بوابة المطورين على وحدة تحكم اختيارية "جربها" حتى تتمكن من الاتصال بنقاط نهاية Torii دون الاطلاع على المستندات. تقوم وحدة التحكم بإعادة إرسال متطلبات الوكيل المحظور حتى توفر المتصفحات حدود CORS أثناء وجود حدود معدل التطبيق والمصادقة.

## المتطلبات الأساسية

- Node.js 18.18 أو أحدث (يجمع مع متطلبات إنشاء البوابة)
- الوصول إلى بيئة التدريج Torii
- رمز حامل يمكن أن يقوم بدور Torii الذي يتظاهر بالتمرين

كل ما عليك هو تكوين الوكيل وتشغيله حسب البيئة المتغيرة. لوحة إضافية تحتوي على المقابض الأكثر أهمية:

| متغير | اقتراح | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | قاعدة URL الخاصة بـ Torii لمتطلبات الوكيل | **مطلوب** |
| `TRYIT_PROXY_LISTEN` | Endereco de escuta para desenvolvimento local (تنسيق `host:port` أو `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة منفصلة بواسطة العذراء الأصلية التي يمكنك من خلالها الاتصال بالوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | المعرف المحدد في `X-TryIt-Client` لكل متطلبات المنبع | `docs-portal` |
| `TRYIT_PROXY_BEARER` | حامل الرمز المميز Padrao encaminhado ao Torii | _فارغة_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | السماح للمستخدمين بالحصول على الرمز المميز الخاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحجم الأقصى للمتطلبات (بايت) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | المهلة المنبع م milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | المتطلبات المسموح بها لتصنيف أصناف IP للعميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Janela deslizante لتحديد المعدل (مللي ثانية) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | إجراء فحص اختياري لنقطة نهاية المقاييس بأسلوب Prometheus (`host:port` أو `[ipv6]:port`) | _فارغة (معطل)_ |
| `TRYIT_PROXY_METRICS_PATH` | Caminho HTTP servido pelo endpoint de metricas | `/metrics` |

يعرض الوكيل أيضًا `GET /healthz`، ويعيد أخطاء JSON estruturados ورموز حامل الماسكارا في السجلات.

نشط `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` من أجل تصدير وكيل لمستخدمي المستندات حتى يتمكنوا من استخدام Swagger وRapiDoc لامتلاك حامل الرموز المميزة الخاصة بالمستخدم. يوجد وكيل يطبق حدود التصنيفات، ويطبق اعتمادات الماسكارا ويسجل إذا كان هناك حاجة لاستخدام رمز مميز أو تجاوز حسب الطلب. قم بتكوين `TRYIT_PROXY_CLIENT_ID` من خلال الدائرة التي تريد إرسالها مثل `X-TryIt-Client`
(بادراو `docs-portal`). يقطع الوكيل وقيم التحقق من الصحة `X-TryIt-Client` العميل الخاص به، ويرشده إلى هذا الإعداد الافتراضي حتى تتمكن بوابات التدريج من مراجعة الإجراءات غير المرتبطة ببيانات المتصفح.

## بدء تشغيل الوكيل المحلي

قم بتثبيت التبعيات أولاً عندما تقوم بتكوين البوابة الإلكترونية:

```bash
cd docs/portal
npm install
```

ركب الوكيل والوكيل لمثيلك Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

يتم تسجيل البرنامج النصي أو ربطه وتجميع متطلبات `/proxy/*` لتكوين Torii الأصلي.

Antes de fazer لا يوجد ربط بمقبس أو برنامج نصي صالح
`static/openapi/torii.json` يتوافق مع التسجيل الملخص
`static/openapi/manifest.json`. إذا كانت الملفات متباينة، فسيتم إغلاق الأمر بخطأ e
قم بإعداد الملف التنفيذي `npm run sync-openapi -- --latest`. تصدير
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط لتجاوزات الطوارئ؛ o تسجيل الوكيل أو التحذير
واستمر في ذلك حتى تتمكن من التعافي خلال فترة الصيانة الطويلة.

## عناصر واجهة المستخدم Conecte os تفعل البوابة

عند إنشاء بوابة للمطورين أو خدمتها، حدد عنوان URL الذي ستنشئه الأدوات
استخدام الفقرة أو الوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

المكونات التي تحتوي على قيم `docusaurus.config.js`:

- **Swagger UI** - تم عرضه على `/reference/torii-swagger`؛ التفويض المسبق أو الاختيار
  الحامل عندما يكون هناك رمز مميز، العلامة التجارية المطلوبة com `X-TryIt-Client`،
  injeta `X-TryIt-Auth`، وإعادة فتح المكالمات بالوكيل عند استخدام الوكيل
  تم تكوين `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - تم العرض على `/reference/torii-rapidoc`; espelha أو كامبو دي توكن،
  إعادة استخدام رؤوس كل من لوحة Swagger و Aponta للوكيل
  يتم ذلك تلقائيًا عندما يتم تكوين عنوان URL.
- **Try it console** - مضمن في صفحة نظرة عامة على واجهة برمجة التطبيقات؛ سمح بالحسد
  المتطلبات المخصصة، وعرض الرؤوس، والتحقق من مجموعة الرد.

ما عليك فعله هو اختيار **مختار اللقطات** الذي تريده
`docs/portal/static/openapi/versions.json`. Preencha esse indice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` للمراجعين
بديل بين المواصفات التاريخية، وإصدار ملخص SHA-256، والتسجيل والتأكيد
تم التقاط لقطة من الإصدار لبيان تم حذفه قبل استخدام الأدوات التفاعلية.

قم بإدارة أو رمز مميز في أي عنصر واجهة مستخدم حتى يتم تنشيطه أثناء التنقل في الوقت الحالي؛ o الوكيل غير المستمر
لا يوجد سجل أو رمز مميز.

## الرموز المميزة OAuth de curta duracao

لتجنب توزيع الرموز المميزة Torii من Longa Duracao مع المراجعين، قم بالاتصال بوحدة التحكم جربها أيضًا
الخادم الخاص بك OAuth. عند تنوع البيئة المحيطة، أو عرض البوابة
أداة لتسجيل الدخول مع رمز الجهاز، ورموز حاملة لـ Curta Duracao وOS Injeta
لا توجد صيغة وحدة التحكم تلقائيًا.

| متغير | اقتراح | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | نقطة نهاية ترخيص جهاز OAuth (`/oauth/device/code`) | _فارغة (معطل)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة النهاية للرمز المميز الذي تستخدمه `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _فارغة_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة المستندات | _فارغة_ |
| `DOCS_OAUTH_SCOPE` | نطاقات منفصلة من أجل espaco solicitados بدون تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | جمهور واجهة برمجة التطبيقات الاختيارية للرمز المميز | _فارغة_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | الحد الأدنى من الفاصل الزمني للاستقصاء خلال فترة aguarda aprovacao (ms) | `5000` (القيم < 5000 مللي ثانية في الوقت المحدد) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Janela de expiracao do رمز الجهاز (الثانية) | `600` (تم تطويره بين 300 ثانية و900 ثانية) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | دوراكاو يفعل رمز الوصول (الثاني) | `900` (تم تطويره بين 300 ثانية و900 ثانية) |
| `DOCS_OAUTH_ALLOW_INSECURE` | قم بتعريف `1` لمعاينة المواقع التي ستنفذ OAuth بشكل مقصود | _unset_ |

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

عند التحدث إلى `npm run start` أو `npm run build`، تقوم البوابة بإدخال قيمها
`docusaurus.config.js`. Durante لمعاينة البطاقة المحلية، جربها أكثر من botao
"تسجيل الدخول باستخدام رمز الجهاز". المستخدمون الرقميون أو الكود الظاهر على صفحتهم OAuth؛ عندما ينجح تدفق الجهاز أو القطعة:

- injeta o Bearer Token Emitido no Campo Do Console جربه،
- العلامة التجارية المطلوبة مع الرؤوس الموجودة `X-TryIt-Client` و`X-TryIt-Auth`،
- عرض وتيرة الحياة المتبقية، ه
- يتم الإرجاع تلقائيًا أو الرمز المميز عند انتهاء الصلاحية.

يستمر حامل الدليل في الوصول إليه; omita as variaveis OAuth quando quiser
يجب على مراجعي السيارات الحصول على رمز مؤقت للحساب الخاص أو التصدير
`DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينة المواقع المعزولة من خلال الوصول المجهول
com.aceitavel. يبني كل تكوين OAuth قبل أن يصبح سريعًا للاستقبال أو البوابة
خريطة الطريق DOCS-1b.

ملاحظة: قم بمراجعة [قائمة التحقق من التشديد الأمني واختبار القلم](./security-hardening.md)
قبل التصدير أو بوابة منتديات المختبر؛ إلا دوكومنتا أو نموذج التهديد,
ملف تعريف CSP/الأنواع الموثوقة وخطوات اختبار القلم التي تحظر الآن DOCS-1b.

## اموستراس Norito-RPC

تتم مشاركة متطلبات Norito-RPC مع الوكيل والسباكة OAuth الذي يدور JSON؛
يتم تعريفها فقط `Content-Type: application/x-norito` وإرسال الحمولة Norito
تم الوصف المشفر مسبقًا على NRPC محدد
(`docs/source/torii/nrpc_spec.md`).
يحتوي المستودع على حمولات canonicos sob `fixtures/norito_rpc/` حتى يقوم المصنعون بذلك
البوابة، يستطيع مالكو SDK والمراجعون إنتاج وحدات البايت الإضافية التي تستخدمها CI.

### Enviar um payload Norito pelo console جربها

1. قم بتجميع التركيبات مثل `fixtures/norito_rpc/transfer_asset.norito`. جوهر
   arquivos sao مغلفات Norito بروتوس؛ ** ناو ** faca base64.
2. في Swagger أو RapidDoc، قم بتحديد مكان نقطة النهاية NRPC (على سبيل المثال
   `POST /v2/pipeline/submit`) وتغيير محدد **Content-Type** لـ
   `application/x-norito`.
3. ابحث عن محرر الجسم **binary** (وضع "ملف" في Swagger أو
   حدد "Binary/File" في RapiDoc) ثم أرسل الملف `.norito`. يا القطعة
   إرسال بايتات نظام التشغيل إلى الوكيل نفسه.
4. حسد أحد المتطلبات. إذا كان Torii معيدًا `X-Iroha-Error-Code: schema_mismatch`،
   تحقق مما إذا كان هذا هو عنوان نقطة النهاية التي تستخدم الحمولات الثنائية وتأكيدها
   تم تسجيل تجزئة المخطط في `fixtures/norito_rpc/schema_hashes.json`
   تتوافق مع البنية Torii التي تستخدمها.

وحدة التحكم تحافظ على الملف الأحدث في الذاكرة لتتمكن من تجديده في نفس الوقت
الحمولة أثناء اختبار الرموز المميزة المختلفة للتفويض أو المضيفين Torii. أديسيونار
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` لسير العمل الخاص بك أو حزمة المنتج
 الأدلة المرجعية لا تحتوي على مخطط Adocao NRPC-4 (السجل + ملخص JSON)، الذي يجمع بينهما
com التقط لقطات شاشة من الرد جربها خلال المراجعات.

### Exemplo CLI (الضفيرة)

يمكن إعادة إنتاج التركيبات نفسها من خلال البوابة عبر `curl`، أو ما يساعدها
عند التحقق من صحة الوكيل أو إعادة إرسال البوابة:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```ابحث عن تركيب لأي قائمة مدخلة في `transaction_fixtures.manifest.json`
أو قم بتشفير الحمولة الخاصة بك مع `cargo xtask norito-rpc-fixtures`. عندما Torii هذا
يمكن وضع الصوت الكناري على `curl` للوكيل التجريبي
(`https://docs.sora.example/proxy/v2/pipeline/submit`) للتمرين على البرنامج
البنية التحتية usada pelos الحاجيات تفعل البوابة.

## إمكانية المراقبة والعمليات

كل ما هو مطلوب وتسجيله هو الطريقة والمسار والأصل والحالة الأولية والخط
de autenticacao (`override`, `default` أو `client`). الرموز نونكا ساو أرمازينادوس: تانتو
حامل رؤوس نظام التشغيل بقدر قيم نظام التشغيل `X-TryIt-Auth` sao redigidos antes do log،
يمكنك إجراء مكالمات صوتية من أجل تجميع مركزي دون الحاجة إلى الاهتمام بالأدوات.

### مجسات الصوت والتنبيهات

يشمل ركوب المسبار أثناء النشر في جدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

مقابض البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - دوران Torii اختياري (sem `/proxy`) للتمرين.
- `TRYIT_PROXY_SAMPLE_METHOD` - بادراو `GET`; قم بتعريف `POST` لتدوير الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - أدخل رمزًا مؤقتًا لحامل الرمز المميز لدعوة الأموسترا.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يتم تحديد فترة المهلة لمدة 5 ثوانٍ.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - اتجاه النص Prometheus اختياري لـ `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - متساوي `key=value` منفصلين بواسطة عذراء anexados كمقاييس (padrao `job=tryit-proxy` و`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - عنوان URL اختياري لنقطة نهاية المقاييس (على سبيل المثال، `http://localhost:9798/metrics`) الذي يجب الاستجابة له عندما يكون `TRYIT_PROXY_METRICS_LISTEN` مؤهلاً.

قم بتزويد النتائج في مُجمع ملفات نصية للإجابة على سؤال لملف حصى
(على سبيل المثال، `/var/lib/node_exporter/textfile_collector/tryit.prom`) وإضافة التسميات
تخصيص:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يحتوي البرنامج النصي على ملف مقاييس الشكل الذري حتى يتمكن جامعه من التعلم دائمًا
أم الحمولة كاملة.

عندما يتم تكوين `TRYIT_PROXY_METRICS_LISTEN`، حدد
`TRYIT_PROXY_PROBE_METRICS_URL` لنقطة نهاية القياسات لإجراء التحقيق بسرعة
إزالة سطح الكشط (على سبيل المثال، الدخول بشكل سيء أو إعادة تنظيم جدار الحماية).
نوع عادل من الإنتاج e
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

لتنبيهات المستويات، قم بتوصيل أو فحص مجموعة المراقبة الخاصة بك. مثال Prometheus كيو
صفحة apos duas falhas consecutivas:

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

### نقطة النهاية للقياسات ولوحات المعلومات

تعريف `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (أو أي مضيف/مدخل) قبل ذلك
ابدأ الوكيل لعرض نقطة النهاية للمقاييس بالتنسيق Prometheus. يا كامينهو
Padrao e `/metrics` mas pode ser sobrescrito عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. كل ما عليك فعله هو إعادة موازنة جميع العناصر عن طريق الطريقة،
الترحيب بالحد الأقصى للمعدل، والأخطاء/المهلات الأولية، ونتائج الوكيل، واستئناف زمن الوصول:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Aponte جامعيها Prometheus/OTLP لنقطة نهاية المقاييس وإعادة استخدام اللوحات
موجود في `dashboards/grafana/docs_portal.json` حتى يتمكن SRE من مراقبة زمن الوصول في الذيل
وصور الاسترداد دون تحليل السجلات. يتم نشر الوكيل تلقائيًا `tryit_proxy_start_timestamp_ms`
para ajudaroperadores a reinicios Detector.

### التراجع التلقائي

استخدم مساعد الإدارة لتحديث أو استعادة عنوان URL أعلى من Torii. يا السيناريو
قم بتخزين التكوين السابق في `.env.tryit-proxy.bak` حتى تتمكن من إجراء عمليات التراجع
com.unico comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

يتم فتح الرابط الخاص بالملف عبر `--env` أو `TRYIT_PROXY_ENV` فيما يتعلق بزرعها
قم بضبط التكوين في مكان آخر.