---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بيئة Essayez-le التجريبية

يوفر بوابة المطورين وحدة تحكم اختيارية "Try it" حتى تتمكن من استدعاء نقاط نهاية Torii بدون مغادرة الوثائق. Vous avez besoin de plus d'informations pour que CORS soit en mesure de le faire حدود المعدل والمصادقة.

## المتطلبات المسبقة

- Node.js 18.18 et version (يتطابق مع متطلبات بناء البوابة)
- Comment utiliser la mise en scène pour Torii
- jeton du porteur

تتم كل تهيئة الوكيل عبر متغيرات البيئة. الجدول ادناه يسرد اهم المفاتيح:| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الاساسي الذي يعيد الوكيل توجيه الطلبات اليه | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` et `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة مفصولة بفواصل للمصادر المسموح لها باستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف يوضع في `X-TryIt-Client` لكل طلب en amont | `docs-portal` |
| `TRYIT_PROXY_BEARER` | jeton du porteur افتراضي يعاد توجيهه الى Torii | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Jeton de jeton pour `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الاقصى لحجم جسم الطلب (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة en amont بالميلي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | عدد الطلبات المسموح بها لكل نافذة معدل لكل IP عميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة انزلاقية للحد من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | عنوان استماع اختياري لنقطة metrics by Prometheus (`host:port` او `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | Plus de métriques HTTP que تخدمه نقطة | `/metrics` |

JSON utilise `GET /healthz` pour les jetons JSON et les jetons porteurs.`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` est un fournisseur de jetons Swagger et RapiDoc pour les jetons porteurs. المستخدم. Pour créer un jeton et un jeton لكل طلب. اضبط `TRYIT_PROXY_CLIENT_ID` pour le transfert vers `X-TryIt-Client`
(افتراضي `docs-portal`). يقوم الوكيل بقص والتحقق من قيم `X-TryIt-Client` المقدمة من العميل والعودة الى هذا الافتراضي حتى تتمكن بوابات staging من تدقيق المصدر دون ربط بيانات المتصفح.

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
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` pour les appareils photo سيسجل الوكيل تحذيرا ويكمل حتى تتمكن من
التعافي خلال نوافذ الصيانة.

## ربط عناصر البوابة

عند بناء البوابة او تشغيلها، اضبط عنوان URL الذي يجبان تستخدمه العناصر للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les informations relatives à l'achat sont liées à `docusaurus.config.js` :- **Swagger UI** - يعرض في `/reference/torii-swagger` ; يسبق تفويض مخطط porteur عند وجود jeton,
  يوسم الطلبات by `X-TryIt-Client`, يحقن `X-TryIt-Auth`, ويعيد توجيه الاستدعاءات عبر
  La description est `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - يعرض في `/reference/torii-rapidoc` ; يعكس حقل jeton،
  Il y a des en-têtes pour Swagger et des en-têtes pour Swagger.
- **Essayez-la console** - مضمّنة في صفحة نظرة عامة على الـ API؛ تتيح ارسال طلبات مخصصة،
  عرض en-têtes, وفحص اجسام الاستجابة.

يعرض كلا اللوحتين **محدد instantanés** الذي يقرأ
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` حتى يتمكن المراجعون من
Résumé du résumé SHA-256 pour l'instantané
الاصدار يحمل manifeste موقعا قبل استخدام العناصر التفاعلية.

تغيير الـ token في اي عنصر يؤثر فقط على جلسة المتصفح الحالية؛ Il s'agit d'un jeton.

## رموز OAuth pour le client

Utilisez le code Torii pour utiliser OAuth. عندما تكون
Utilisez le code de l'appareil et les jetons du porteur
العمر وتحقنها تلقائيا في نموذج وحدة التحكم.| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Utiliser OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | نقطة token تقبل `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | معرف عميل OAuth المسجل لمعاينة الوثائق | _vide_ |
| `DOCS_OAUTH_SCOPE` | نطاقات مفصولة بمسافة مطلوبة اثناء تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | API d'audience Jeton d'audience | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | اقل فترة sondage اثناء انتظار الموافقة (ms) | `5000` (temps < 5000 ms) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | مدة صلاحية code de l'appareil الاحتياطية (ثوان) | `600` (jusqu'à 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | مدة صلاحية access token الاحتياطية (ثوان) | `900` (jusqu'à 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Fichier `1` pour les applications OAuth | _unset_ |

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
`docusaurus.config.js`. اثناء المعاينة المحلية تعرض بطاقة Essayez-le ici
"Connectez-vous avec le code de l'appareil". يدخل المستخدمون الرمز المعروض في صفحة التحقق OAuth؛ عند نجاح
flux de l'appareil تقوم الاداة بما يلي:

- حقن Bearer Token الصادر في حقل وحدة Essayez-le,
- Comme les en-têtes `X-TryIt-Client` et `X-TryIt-Auth`,
- عرض العمر المتبقي، و
- حذف الـ token تلقائيا عند انتهاء الصلاحية.يظل ادخال Bearer اليدوي متاحا؛ Comment utiliser OAuth pour utiliser le jeton
بانفسهم، او صدّر `DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينات محلية يكون فيها الوصول المجهول مقبولا.
Les builds OAuth sont également compatibles avec DOCS-1b.

ملاحظة: راجع [Liste de contrôle de renforcement de la sécurité et de test d'intrusion](./security-hardening.md)
قبل فتح البوابة خارج المختبر؛ Voir aussi CSP/Trusted Types et pen-test
Vous utilisez DOCS-1b.

## Télécharger Norito-RPC

Le Norito-RPC est utilisé pour le proxy et OAuth Plumbing pour JSON. فهي تضبط
`Content-Type: application/x-norito` et charge utile Norito pour le NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر المستودع payloads قياسية تحت
`fixtures/norito_rpc/` Utilisez le SDK et le SDK pour créer un nouveau produit
البايتات التي يستخدمها CI.

### Charge utile Norito pour essayer Essayez-le1. Installez le luminaire `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   اغلفة Norito خام؛ **لا** est utilisé en base64.
2. Avec Swagger et RapiDoc, il s'agit du point final NRPC (avec
   `POST /v2/pipeline/submit`) et **Content-Type** الى
   `application/x-norito`.
3. Utilisez l'option **binaire** (ou "Fichier" pour Swagger et "Binaire/Fichier" pour RapiDoc)
   Il s'agit de `.norito`. تقوم الاداة بتمرير البايتات عبر الوكيل بدون تعديل.
4. ارسل الطلب. اذا اعاد Torii `X-Iroha-Error-Code: schema_mismatch`, تحقق انك تستدعي
   endpoint contient des charges utiles et un hachage de schéma
   `fixtures/norito_rpc/schema_hashes.json` est utilisé pour construire Torii.

Vous devez également ajouter des jetons à la charge utile pour les jetons.
C'est Torii. اضافة `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الى سير عملك
Il s'agit d'un bundle contenant des informations sur NRPC-4 (log + JSON) et plus encore.
Captures d'écran de la version Try It ci-dessous.

### مثال CLI (boucle)

يمكن اعادة تشغيل نفس luminaires خارج البوابة عبر `curl`, وهو مفيد عند التحقق من الوكيل او
Utiliser la passerelle ردود :

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Le luminaire est connecté à `transaction_fixtures.manifest.json` et la charge utile est disponible.
`cargo xtask norito-rpc-fixtures`. Torii pour Canary et `curl` pour `curl`
proxy try-it (`https://docs.sora.example/proxy/v2/pipeline/submit`) pour la première fois
التي تستخدمها عناصر البوابة.

## المراقبة والعملياتIl s'agit de la méthode, du chemin et de l'origine, ainsi que de l'amont et de la méthode
(`override` et `default` et `client`). لا يتم تخزين الـ tokens اطلاقا؛ يتم تنقيح en-têtes de porteur
وقيم `X-TryIt-Auth` قبل التسجيل، لذا يمكنك تمرير stdout الى مجمع مركزي دون القلق من تسرب الاسرار.

### فحوصات الصحة والتنبيه

شغل sonde المرفق اثناء النشر او بجدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

مفاتيح البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - Pour Torii (par `/proxy`) pour.
- `TRYIT_PROXY_SAMPLE_METHOD` - Version `GET` ; اضبط `POST` لعمليات الكتابة.
- `TRYIT_PROXY_PROBE_TOKEN` - jeton de porteur مؤقت للطلب التجريبي.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يستبدل المهلة الافتراضية 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - Prometheus est compatible avec `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` مفصولة بفواصل تضاف الى المقاييس (افتراضي `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - عنوان اختياري لنقطة metrics (مثل `http://localhost:9798/metrics`) يجب ان يستجيب عند تمكين `TRYIT_PROXY_METRICS_LISTEN`.

Utiliser le collecteur de fichiers texte pour utiliser la sonde comme outil de collecte de fichiers texte
(مثل `/var/lib/node_exporter/textfile_collector/tryit.prom`) et le cas échéant :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Il s'agit d'une charge utile importante.

عند ضبط `TRYIT_PROXY_METRICS_LISTEN`, قم بتعيين
`TRYIT_PROXY_PROBE_METRICS_URL` métriques de sonde pour le grattage
(Il s'agit d'une entrée ou d'un pare-feu). اعداد production شائع هو
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.للتنبيه الخفيف، اربط sonde بواجهة المراقبة لديك. مثال Prometheus يقوم بالتصعيد بعد فشلين متتاليين:

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

### نقطة métriques et statistiques

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (pour hôte/port) pour le téléchargement
J'utilise les métriques Prometheus. المسار الافتراضي هو `/metrics` ويمكن تغييره عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. كل scrape يعيد عدادات لاجمالي الطلبات حسب méthode ورفض
limite de débit et délais d'attente en amont et latence :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

وجّه مجمعات Prometheus/OTLP pour les métriques et les métriques
`dashboards/grafana/docs_portal.json` حتى يتمكن SRE pour les latences مراقبة et les latences
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