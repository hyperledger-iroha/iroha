---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بيئة Try It التجريبية

يوفر بوابة المطورين وحدة تحكم اختيارية "Try it" حتى تتمكن من استدعاء نقاط نهاية Torii Não há problema. تقوم وحدة التحكم بتمرير الطلبات عبر الوكيل المضمن حتى تتمكن المتصفحات من تجاوز قيود CORS مع Certifique-se de que o produto esteja em boas condições.

## المتطلبات المسبقة

- Node.js 18.18 e احدث (يتطابق مع متطلبات بناء البوابة)
- وصول شبكة الى بيئة staging em Torii
- token de portador

Certifique-se de que o produto esteja funcionando corretamente. O que você precisa saber sobre isso:

| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الاساسي الذي يعيد الوكيل توجيه الطلبات اليه | **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` e `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة مفصولة بفواصل للمصادر المسموح لها باستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف يوضع في `X-TryIt-Client` لكل طلب upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | token de portador افتراضي يعاد توجيهه الى Torii | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | O token de troca de token é `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | الحد الاقصى لحجم جسم الطلب (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Transportes a montante | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | عدد الطلبات المسموح بها لكل نافذة معدل لكل IP عميل | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة انزلاقية للحد من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | As métricas de medição são Prometheus (`host:port` e `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Como usar HTTP para medir métricas | `/metrics` |

Use o `GET /healthz` para usar o JSON e os tokens de portador do arquivo.

O `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` é usado para usar o Swagger e o RapiDoc com tokens de portador. Isso é verdade. Não há necessidade de usar o token e o token do token. Deixe isso acontecer. Use `TRYIT_PROXY_CLIENT_ID` para obter mais informações sobre `X-TryIt-Client`
(`docs-portal`). O código de barras do `X-TryIt-Client` está disponível no site e no site da empresa. A preparação é feita por meio de um teste de teste.

## تشغيل الوكيل محليا

ثبت التبعيات اول مرة تجهز فيها البوابة:

```bash
cd docs/portal
npm install
```

A configuração do Torii é a seguinte:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Você pode usar o `/proxy/*` para Torii.

قبل ربط المنفذ يتحقق السكربت من ان
`static/openapi/torii.json` يطابق الـ digest المسجل في
`static/openapi/manifest.json`. اذا انحرفت الملفات ينهي الامر بخطأ ويطلب منك تشغيل
`npm run sync-openapi -- --latest`. صدّر
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط لحالات الطوارئ؛ سيسجل الوكيل تحذيرا ويكمل حتى تتمكن من
Não há nada que você possa fazer.

## ربط عناصر البوابة

عند بناء البوابة او تشغيلها, اضبط عنوان URL الذي يجب ان تستخدمه العناصر للوكيل:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

O código de barras do arquivo `docusaurus.config.js`:

- **Swagger UI** - criado em `/reference/torii-swagger`; يسبق تفويض مخطط bearer عند وجود token،
  O código de barras é `X-TryIt-Client`, ou `X-TryIt-Auth`, e você pode usar o código de barras
  O código de segurança é `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - يعرض em `/reference/torii-rapidoc`; Token de token,
  Você pode usar os cabeçalhos do Swagger e usá-los para usar o Swagger.
- **Experimente console** - مضمّنة في صفحة نظرة عامة على الـ API؛ تتيح ارسال طلبات مخصصة,
  Os cabeçalhos são os mesmos e os cabeçalhos.

يعرض كلا اللوحتين **محدد snapshots** الذي يقرأ
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` é um arquivo de código aberto
التنقل بين المواصفات التاريخية ومشاهدة digest SHA-256 المسجل e ما اذا كان snapshot
O manifesto do arquivo pode ser usado para configurar o manifesto.

تغيير الـ token في اي عنصر يؤثر فقط على جلسة المتصفح الحالية؛ O token não é válido e o token é o mesmo.

## رموز OAuth قصيرة العمر

Tente usar o Torii para testar o OAuth. عندما تكون
متغيرات البيئة ادناه موجودة, تعرض البوابة واجهة تسجيل دخول código do dispositivo, e tokens de portador قصيرة
Você pode fazer isso em qualquer lugar e depois.| المتغير | الغرض | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token de token `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | Como usar o OAuth para usar o OAuth | _vazio_ |
| `DOCS_OAUTH_SCOPE` | Máquinas de lavar roupa | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API اختياري لربط الـ token | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | اقل فترة polling اثناء انتظار الموافقة (ms) | `5000` (tempo < 5000 ms tempo) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | مدة صلاحية código do dispositivo الاحتياطية (ثوان) | `600` (entre 300 e 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Token de acesso de token de acesso (ثوان) | `900` (entre 300 e 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Use `1` para obter informações sobre OAuth | _desativar_ |

Como fazer:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Você pode usar `npm run start` e `npm run build` para obter mais informações.
`docusaurus.config.js`. اثناء المعاينة المحلية تعرض بطاقة Experimente
"Entrar com o código do dispositivo". Use o aplicativo OAuth no site do OAuth; Não
fluxo do dispositivo تقوم الاداة بما يلي:

- حقن bearer token الصادر في حقل وحدة Experimente،
- وسم الطلبات بالـ cabeçalhos `X-TryIt-Client` e `X-TryIt-Auth`,
- عرض العمر المتبقي, e
- حذف الـ token تلقائيا عند انتهاء الصلاحية.

Não é o portador do portador O OAuth deve ser usado para configurar o token do OAuth
A chave `DOCS_OAUTH_ALLOW_INSECURE=1` e a chave `DOCS_OAUTH_ALLOW_INSECURE=1` podem ser usadas para evitar problemas.
Construa o build do OAuth para que ele possa ser baixado do DOCS-1b no computador.

Exemplo: Lista de verificação de fortalecimento de segurança e pen-test](./security-hardening.md)
قبل فتح البوابة خارج المختبر؛ Você está procurando por CSP/Trusted Types e pen-test.
Consulte DOCS-1b.

## Configuração Norito-RPC

Use Norito-RPC para proxy e encanamento OAuth para JSON; فهي تضبط
`Content-Type: application/x-norito` é uma carga útil Norito que é uma carga útil do NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر المستودع cargas úteis
`fixtures/norito_rpc/` é compatível com o SDK e o SDK e o SDK do sistema operacional
O CI.

### Altere a carga útil Norito e experimente

1. Instale o acessório como `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   Norito Norito **لا** é baseado em base64.
2. No Swagger e no RapiDoc, você usa o endpoint NRPC (como
   `POST /v1/pipeline/submit`) وغيّر محدد **Content-Type** الى
   `application/x-norito`.
3. Use a opção **binary** (ou "Arquivo" no Swagger e "Binário/Arquivo" no RapiDoc)
   وارفع ملف `.norito`. Verifique se o produto está funcionando corretamente.
4. Limpe o local. Para obter Torii `X-Iroha-Error-Code: schema_mismatch`, você pode usá-lo
   endpoint não é payloads e não é um hash de esquema.
   `fixtures/norito_rpc/schema_hashes.json` é construído com base em Torii.

تحفظ وحدة التحكم اخر ملف في الذاكرة لتتمكن من اعادة ارسال نفس payload مع اختبار tokens مختلفة
E o Torii. Use `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` para usar
Você pode usar o pacote bundle para obter o NRPC-4 (log + arquivo JSON) e também o pacote de pacotes do NRPC-4
Capturas de tela لاستجابة Try It اثناء المراجعات.

### como CLI (curl)

يمكن اعادة تشغيل نفس fixtures خارج البوابة عبر `curl`, وهو مفيد عند التحقق من الوكيل او
Gateway de acesso:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

O dispositivo elétrico não pode ser instalado em `transaction_fixtures.manifest.json` e a carga útil é baixada
`cargo xtask norito-rpc-fixtures`. Eu tenho Torii em um canário ou `curl`
proxy try-it (`https://docs.sora.example/proxy/v1/pipeline/submit`)
Não use nenhum recurso.

## المراقبة والعمليات

Você pode usar o método e o caminho e a origem e o upstream e o caminho e a origem e o upstream
(`override` e `default` e `client`). Como usar tokens يتم تنقيح cabeçalhos do portador
O `X-TryIt-Auth` é usado para definir o stdout do arquivo de saída do computador.

### فحوصات الصحة والتنبيه

شغل probe المرفق اثناء النشر او بجدول زمني:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

مفاتيح البيئة:

- `TRYIT_PROXY_SAMPLE_PATH` - A chave Torii é (ou seja, `/proxy`).
- `TRYIT_PROXY_SAMPLE_METHOD` - Nome `GET`; Use `POST` para remover o problema.
- `TRYIT_PROXY_PROBE_TOKEN` - O token de portador é usado como token de suporte.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - يستبدل المهلة الافتراضية 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - Use o Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - O `key=value` pode ser usado para substituir o `job=tryit-proxy` (`job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - Todas as métricas de medição (como `http://localhost:9798/metrics`) estão disponíveis no `TRYIT_PROXY_METRICS_LISTEN`.Use o coletor de arquivo de texto para testar o valor do coletor de arquivo de texto
(exemplo `/var/lib/node_exporter/textfile_collector/tryit.prom`) e como fazer isso:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

A capacidade de carga útil da carga útil é reduzida.

عند ضبط `TRYIT_PROXY_METRICS_LISTEN`, قم بتعيين
`TRYIT_PROXY_PROBE_METRICS_URL` As métricas de medição são usadas para sondar e raspar
(como o ingresso no firewall e o firewall). Sua produção é mais importante
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

A sonda de teste não está disponível. O Prometheus pode ser instalado em um local onde você possa usar:

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

### نقطة métricas e métricas

Use `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou seja, host/porta) para definir o valor
Essa métrica é Prometheus. O código de barras é `/metrics` e não é necessário
`TRYIT_PROXY_METRICS_PATH=/custom`. كل scrape يعيد عدادات لاجمالي الطلبات حسب método ورفض
limite de taxa e/timeouts upstream e latência:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Você pode usar Prometheus/OTLP para medir métricas e definir métricas
`dashboards/grafana/docs_portal.json` suporta SRE com latências de atraso e latências
Eu não sei. Use a ferramenta `tryit_proxy_start_timestamp_ms` para obter mais informações
على اكتشاف اعادة التشغيل.

### اتمتة التراجع

Verifique o valor do produto e verifique o Torii. يحفظ السكربت الاعداد السابق في
`.env.tryit-proxy.bak` não é compatível com nada.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Você pode usar o env `--env` e `TRYIT_PROXY_ENV` para obter mais informações.