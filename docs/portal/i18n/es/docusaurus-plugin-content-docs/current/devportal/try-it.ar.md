---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بيئة Pruébalo التجريبية

يوفر بوابة المطورين وحدة تحكم اختيارية "Pruébelo" حتى تتمكن من استدعاء نقاط نهاية Torii بدون مغادرة الوثائق. تقوم وحدة بتمرير الطلبات عبر الوكيل المضمن حتى تتمكن المتصفحات من تجاوز قيود CORS مع الاستمرار في فرض حدود المعدل والمصادقة.

## المتطلبات المسبقة

- Node.js 18.18 او احدث (يتطابق مع متطلبات بناء البوابة)
- وصول شبكة الى بيئة puesta en escena de Torii
- token de portador يمكنه استدعاء مسارات Torii التي تنوي اختبارها

تتم كل تهيئة الوكيل عبر متغيرات البيئة. الجدول ادناه يسرد اهم المفاتيح:| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الاساسي الذي يعيد الوكيل توجيه الطلبات اليه | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` او `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Productos sanitarios y sanitarios para el hogar | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف يوضع في `X-TryIt-Client` لكل طلب upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | token al portador افتراضي يعاد توجيهه الى Torii | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | El token de token de uso de `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الاقصى لحجم جسم الطلب (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة aguas arriba بالميلي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | عدد الطلبات المسموح بها لكل نافذة معدل لكل IP عميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة انزلاقية للحد من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان استماع اختياري لنقطة metrics بأسلوب Prometheus (`host:port` او `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | مسار HTTP الذي تخدمه نقطة métricas | `/metrics` |

يعرض الوكيل ايضا `GET /healthz` ويعيد اخطاء JSON منظمة ويخفي tokens de portador مخرجات السجل.`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` incluye tokens de Swagger y RapiDoc para tokens de portador. لا يزال الوكيل يفرض حدود المعدل ويخفي البيانات الحساسة ويسجل ما اذا كان الطلب استخدم token الافتراضي او تجاوزا لكل طلب. اضبط `TRYIT_PROXY_CLIENT_ID` بالوسم الذي تريد ارساله كـ `X-TryIt-Client`
(الافتراضي `docs-portal`). يقوم الوكيل بقص والتحقق من قيم `X-TryIt-Client` المقدمة من العميل والعودة الى هذا الافتراضي حتى تمكن بوابات puesta en escena من تدقيق المصدر دون ربط بيانات المتصفح.

## تشغيل الوكيل محليا

ثبت التبعيات اول مرة تجهز فيها البوابة:

```bash
cd docs/portal
npm install
```

Haga clic en el botón Torii para:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Utilice el `/proxy/*` y el Torii.

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

Nombres de usuario según `docusaurus.config.js`:- ** UI Swagger ** - يعرض في `/reference/torii-swagger`; يسبق تفويض مخطط portador عند وجود token،
  يوسم الطلبات بـ `X-TryIt-Client`, يحقن `X-TryIt-Auth`, ويعيد توجيه الاستدعاءات عبر
  الوكيل عندما يتم ضبط `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - يعرض في `/reference/torii-rapidoc`; يعكس حقل token،
  ويعيد استخدام نفس headers الخاصة بـ Swagger, ويستهدف الوكيل تلقائيا عند ضبط العنوان.
- **Pruébelo en la consola** - مضمّنة في صفحة نظرة عامة على الـ API؛ تتيح ارسال طلبات مخصصة،
  عرض encabezados, وفحص اجسام الاستجابة.

يعرض كلا اللوحتين **محدد instantáneas** الذي يقرأ
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` حتى يتمكن المراجعون من
Resumen de resumen SHA-256 y resumen de instantáneas
الاصدار يحمل manifiesto موقعا قبل استخدام العناصر التفاعلية.

تغيير الـ token في اي عنصر يؤثر فقط على جلسة المتصفح الحالية؛ الوكيل لا يخزن ولا يسجل الـ token المقدم.

## رموز OAuth قصيرة العمر

Haga clic en el enlace Torii. Pruébelo con OAuth. عندما تكون
Aplicación de código de dispositivo y tokens de portador
العمر وتحقنها تلقائيا في نموذج وحدة التحكم.| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Certificado de autenticación OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Nombre del token `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة الوثائق | _vacío_ |
| `DOCS_OAUTH_SCOPE` | Productos sanitarios para el hogar | Productos sanitarios | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | API de audiencia اختياري لربط الـ token | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | اقل فترة encuestas اثناء انتظار الموافقة (ms) | `5000` (más < 5000 ms de duración) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | مدة صلاحية código de dispositivo الاحتياطية (ثوان) | `600` (يجب ان تبقى بين 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | مدة صلاحية token de acceso الاحتياطية (ثوان) | `900` (يجب ان تبقى بين 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | اضبط `1` لمعاينات محلية تجاوز OAuth عمدا | _desarmado_ |

Esta es la siguiente frase:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Utilice el `npm run start` y el `npm run build` para cambiar el estado del sistema.
`docusaurus.config.js`. اثناء المعاينة المحلية تعرض بطاقة Pruébelo زر
"Iniciar sesión con el código del dispositivo". يدخل مستخدمون الرمز المعروض في صفحة التحقق OAuth؛ عند نجاح
flujo del dispositivo تقوم الاداة بما يلي:

- حقن token de portador الصادر في حقل وحدة Pruébelo,
- Y los encabezados de los nombres `X-TryIt-Client` y `X-TryIt-Auth`,
- عرض العمر المتبقي، و
- حذف الـ token تلقائيا عند انتهاء الصلاحية.يظل ادخال Portador اليدوي متاحا؛ Uso de OAuth y uso de tokens
بانفسهم، او صدّر `DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينات محلية معزولة يكون فيها الوصول المجهول مقبولا.
Esta compilación de OAuth requiere que el archivo DOCS-1b esté disponible.

ملاحظة: راجع [Lista de verificación de seguridad y prueba de penetración](./security-hardening.md)
قبل فتح البوابة خارج المختبر؛ Aquí encontrará información sobre CSP/Trusted Types y pen-test.
Utilice DOCS-1b.

## Mensaje Norito-RPC

Descarga Norito-RPC para proxy y plomería OAuth para JSON فهي تضبط
`Content-Type: application/x-norito` Carga útil Norito Contenido del paquete NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر المستودع cargas útiles قياسية تحت
`fixtures/norito_rpc/` حتى يتمكن مؤلفو البوابة ومالكو SDK والمراجعون من اعادة ارسال نفس
البايتات التي يستخدمها CI.

### Carga útil Norito Más información Pruébelo1. Aplique el accesorio `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   Fuente Norito خام؛ **لا** تقم بتحويلها الى base64.
2. Aquí Swagger y RapiDoc son el punto final NRPC (en
   `POST /v1/pipeline/submit`) وغيّر محدد **Tipo de contenido** الى
   `application/x-norito`.
3. Haga clic en **binario** (y en "Archivo" en Swagger y en "Binario/Archivo" en RapiDoc)
   وارفع ملف `.norito`. تقوم الاداة بتمرير البايتات عبر الوكيل بدون تعديل.
4. ارسل الطلب. اذا اعاد Torii `X-Iroha-Error-Code: schema_mismatch`, تحقق انك تستدعي
   punto final con cargas útiles y con hash de esquema
   `fixtures/norito_rpc/schema_hashes.json` يطابق build Torii الذي تستخدمه.

Carga útil y carga útil de tokens
Este es el nombre Torii. اضافة `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الى سير عملك
Paquete de instalación de NRPC-4 (log + JSON) y otros archivos de configuración
التقاط capturas de pantalla لاستجابة Pruébalo اثناء المراجعات.

### CLI (curvatura)

يمكن اعادة تشغيل نفس accesorios خارج البوابة عبر `curl`, y مفيد عند التحقق من الوكيل او
تصحيح ردود puerta de enlace:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

بدل accesorio باي ادخال مدرج في `transaction_fixtures.manifest.json` خاص بك عبر
`cargo xtask norito-rpc-fixtures`. عندما يكون Torii في وضع canary يمكنك توجيه `curl` الى
Probar proxy (`https://docs.sora.example/proxy/v1/pipeline/submit`) لاختبار نفس البنية
التي تستخدمها عناصر البوابة.

## المراقبة والعملياتيتم تسجيل كل طلب مرة واحدة مع método و ruta و origen وحالة aguas arriba ومصدر المصادقة
(`override` y `default` y `client`). لا يتم تخزين الـ tokens اطلاقا؛ يتم تنقيح encabezados al portador
Y `X-TryIt-Auth` está conectado a la salida estándar para que funcione correctamente.

### فحوصات الصحة والتنبيه

شغل probe المرفق اثناء النشر او بجدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

مفاتيح البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - مسار Torii اختياري (بدون `/proxy`) لاختباره.
- `TRYIT_PROXY_SAMPLE_METHOD` - الافتراضي `GET`; Utilice `POST` para garantizar la seguridad.
- `TRYIT_PROXY_PROBE_TOKEN` - Token al portador يحقن مؤقت للطلب التجريبي.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يستبدل المهلة الافتراضية 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - La conexión entre Prometheus y `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - ازواج `key=value` مفصولة بفواصل تضاف الى المقاييس (الافتراضي `job=tryit-proxy` and `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - Métricas de última generación (مثل `http://localhost:9798/metrics`) يجب ان يستجيب عند تمكين `TRYIT_PROXY_METRICS_LISTEN`.

ادمج النتائج في recopilador de archivos de texto عبر توجيه sonda الى مسار قابل للكتابة
(مثل `/var/lib/node_exporter/textfile_collector/tryit.prom`) واضافة وسوم مخصصة:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

يعيد السكربت كتابة ملف المقاييس بشكل ذري حتى يقرأ المجمع دائما carga útil كاملا.

عند ضبط `TRYIT_PROXY_METRICS_LISTEN`, قم بتعيين
`TRYIT_PROXY_PROBE_METRICS_URL` Métricas de la sonda حتى تفشل بسرعة اذا اختفت واجهة scrape
(Para la entrada o para el firewall o para el firewall). اعداد producción شائع هو
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.للتنبيه الخفيف، اربط probe بواجهة المراقبة لديك. Para Prometheus, esta es la configuración del fabricante:

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

### Métricas de نقطة y ولوحات المتابعة

اضبط `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (او اي زوج host/puerto) قبل تشغيل الوكيل
Aquí hay métricas de Prometheus. المسار الافتراضي هو `/metrics` ويمكن تغييره عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. كل scrape يعيد عدادات لاجمالي الطلبات حسب método ورفض
límite de velocidad y tiempos de espera ascendentes y latencia:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Otras métricas y aplicaciones Prometheus/OTLP
`dashboards/grafana/docs_portal.json` حتى يتمكن SRE من مراقبة latencias الذيل وارتفاعات الرفض
دون تحليل السجلات. ينشر الوكيل تلقائيا `tryit_proxy_start_timestamp_ms` لمساعدة المشغلين
على اكتشاف اعادة التشغيل.

### اتمتة التراجع

Utilice el cable de alimentación Torii. يحفظ السكربت الاعداد السابق في
`.env.tryit-proxy.bak` حتى يكون التراجع امرا واحدا.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

يمكنك تجاوز مسار ملف env عبر `--env` او `TRYIT_PROXY_ENV` اذا كان النشر يحفظ الاعدادات في موقع اخر.