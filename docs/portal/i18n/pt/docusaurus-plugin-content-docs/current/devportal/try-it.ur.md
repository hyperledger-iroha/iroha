---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Experimente

ڈویلپر پورٹل ایک اختیاری "Experimente" کنسول فراہم کرتا ہے تاکہ آپ دستاویزات چھوڑے بغیر Pontos de extremidade Torii کنسول بنڈل پروکسی کے ذریعے درخواستیں ری لے کرتا ہے تاکہ براؤزر CORS حدود بائی پاس کر سکیں جبکہ ریٹ لمٹس اور آتھنٹیکیشن نافذ رہیں۔

## شرائط

- Node.js 18.18 یا نیا (build build ضروریات کے مطابق)
- Preparação Torii
- ایسا token de portador جو آپ جن rotas Torii کو ٹیسٹ کرنا چاہتے ہیں انہیں کال کر سکے

پروکسی کی تمام کنفیگریشن variáveis de ambiente سے ہوتی ہے۔ Quais são os botões que você precisa usar:

| Variável | مقصد | Padrão |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii é URL base. **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | لوکل ڈیولپمنٹ کے لئے endereço de escuta (`host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | origens کی separadas por vírgula فہرست جو پروکسی کو کال کر سکتے ہیں | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | ہر solicitação upstream میں `X-TryIt-Client` کے طور پر لگایا گیا identificador | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token de portador ڈیفالٹ جو Torii کو فارورڈ ہوتا ہے | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو `X-TryIt-Auth` کے ذریعے اپنا token دینے کی اجازت | `0` |
| `TRYIT_PROXY_MAX_BODY` | corpo da solicitação کا زیادہ سے زیادہ سائز (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | tempo limite upstream (milissegundos) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | ہر IP do cliente کے لئے فی janela اجازت شدہ solicitações | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | limitação de taxa کے لئے janela deslizante (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus طرز endpoint de métricas کے لئے اختیاری endereço de escuta (`host:port` یا `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | endpoint de métricas کا Caminho HTTP | `/metrics` |

پروکسی `GET /healthz` بھی فراہم کرتا ہے، erros JSON estruturados واپس کرتا ہے، اور tokens de portador کو saída de log سے redigir کرتا ہے۔

`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` فعال کریں جب آپ پروکسی کو usuários de documentos کے لئے expor کریں تاکہ Swagger اور RapiDoc painéis tokens de portador fornecidos pelo usuário فارورڈ کر سکیں۔ پروکسی اب بھی limites de taxa نافذ کرتا ہے, credenciais کو redigir کرتا ہے, اور ریکارڈ کرتا ہے کہ solicitação de token padrão استعمال کیا یا substituição por solicitação۔ `TRYIT_PROXY_CLIENT_ID` کو اس rótulo پر سیٹ کریں جو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(ڈیفالٹ `docs-portal`)۔ پروکسی valores `X-TryIt-Client` fornecidos pelo chamador کو trim اور validar کرتا ہے اور اسی padrão پر واپس آتا ہے تاکہ staging gateways metadados do navegador سے correlacionar کئے بغیر auditoria de proveniência کر سکیں۔

## لوکل طور پر پروکسی چلائیں

پورٹل سیٹ اپ کرنے کی پہلی بار dependências انسٹال کریں:

```bash
cd docs/portal
npm install
```

پروکسی چلائیں اور اسے اپنے Torii instance کی طرف پوائنٹ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

اسکرپٹ endereço vinculado لاگ کرتا ہے اور `/proxy/*` سے solicitações کو origem Torii configurada کی طرف forward کرتا ہے۔

ساکٹ bind کرنے سے پہلے اسکرپٹ verify کرتا ہے کہ
`static/openapi/torii.json` کا digest `static/openapi/manifest.json` میں ریکارڈ شدہ digest سے میچ کرے۔ اگر فائلز drift ہو جائیں تو کمانڈ error کے ساتھ exit کر دیتا ہے اور `npm run sync-openapi -- --latest` چلانے کی ہدایت دیتا ہے۔ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی substituir کے لئے استعمال کریں؛ پروکسی aviso لاگ کرے گا اور جاری رہے گا تاکہ janelas de manutenção میں ریکوری ہو سکے۔

## Widgets پورٹل کو جوڑیں

جب آپ portal do desenvolvedor کو construir یا servir کرتے ہیں تو وہ URL سیٹ کریں جسے widgets پروکسی کے لئے استعمال کریں:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

یہ componentes `docusaurus.config.js` سے valores پڑھتے ہیں:

- **Swagger UI** - `/reference/torii-swagger` para renderização ہوتا ہے؛ token موجود ہونے پر esquema de portador کو pré-autorizar کرتا ہے، solicitações کو `X-TryIt-Client` سے tag کرتا ہے, `X-TryIt-Auth` injetar کرتا ہے، اور `TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے پر chamadas کو پروکسی کے ذریعے reescrever کرتا ہے۔
- **RapiDoc** - `/reference/torii-rapidoc` para renderizar ہوتا ہے؛ token فیلڈ کو espelho کرتا ہے، Painel Swagger جیسے reutilização de cabeçalhos کرتا ہے, e configuração de URL ہونے پر خودکار طور پر proxy کو destino کرتا ہے۔
- **Experimente console** - Página de visão geral da API میں incorporado ہے؛ solicitações personalizadas بھیجنے، cabeçalhos دیکھنے, e órgãos de resposta inspecionam کرنے دیتا ہے۔

دونوں painéis ایک **seletor de instantâneo** دکھاتے ہیں جو
`docs/portal/static/openapi/versions.json` پڑھتا ہے۔ O índice é
`npm run sync-openapi -- --version=<label> --mirror=current --latest` سے بھریں تاکہ revisores especificações históricas میں جا سکیں, ریکارڈ شدہ SHA-256 digest دیکھ سکیں, اور widgets interativos استعمال کرنے سے پہلے تصدیق کر سکیں کہ liberar instantâneo assinado manifesto لے کر آیا ہے۔

کسی بھی widget میں token بدلنا صرف موجودہ sessão do navegador پر اثر ڈالتا ہے؛ proxy کبھی بھی فراہم کردہ token کو persist یا log نہیں کرتا۔

## Como usar tokens OAuth e OAuthطویل مدت والے Revisores de tokens Torii کو دینے سے بچنے کے لئے Experimente o console کو اپنے Servidor OAuth سے جوڑیں۔ جب نیچے دیے گئے variáveis de ambiente موجود ہوں تو پورٹل widget de login de código de dispositivo دکھاتا ہے، مختصر مدت والے tokens de portador بناتا ہے، اور انہیں formulário de console میں خودکار طور پر injetar کرتا ہے۔

| Variável | مقصد | Padrão |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorização de dispositivo OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Ponto de extremidade do token `grant_type=urn:ietf:params:oauth:grant-type:device_code` قبول کرتا ہے | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth ou visualização de documentos کے لئے رجسٹر ہے | _vazio_ |
| `DOCS_OAUTH_SCOPE` | login کے دوران مانگے گئے escopos delimitados por espaço | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | token کو باندھنے کے لئے اختیاری Público da API | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم intervalo de pesquisa (ms) | `5000` (قدریں < 5000 ms tempo de espera) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | código do dispositivo کی janela de expiração de fallback (segundos) | `600` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | token de acesso کی tempo de vida de fallback (segundos) | `900` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | لوکل previews کے لئے `1` جو Aplicação OAuth جان بوجھ کر چھوڑتے ہیں | _desativar_ |

Configuração de exemplo:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو پورٹل یہ valores `docusaurus.config.js` میں incorporar کرتا ہے۔ لوکل visualização کے دوران Experimente o cartão "Fazer login com o código do dispositivo" بٹن دکھاتا ہے۔ صارفین دکھایا گیا código اپنی página de verificação OAuth پر درج کرتے ہیں؛ fluxo do dispositivo کامیاب ہونے کے بعد widget:

- جاری شدہ token de portador کو Experimente o console فیلڈ میں injetar کرتا ہے،
- موجودہ `X-TryIt-Client` e `X-TryIt-Auth` cabeçalhos کے ساتھ tags de solicitações کرتا ہے،
- باقی ماندہ مدت دکھاتا ہے، اور
- token ختم ہونے پر خودکار طور پر صاف کر دیتا ہے۔

entrada manual do portador دستیاب رہتا ہے - Variáveis OAuth کو چھوڑ دیں جب آپ revisores کو عارضی token خود colar کرنے پر مجبور کرنا چاہتے ہیں, یا `DOCS_OAUTH_ALLOW_INSECURE=1` exportar کریں تاکہ isolado لوکل visualizações میں acesso anônimo قابل قبول ہو۔ OAuth é construído com builds e DOCS-1b roadmap gate.

Nota: پورٹل کو لیب سے باہر expor کرنے سے پہلے [Lista de verificação de fortalecimento de segurança e pen-test](./security-hardening.md) دیکھیں؛ O modelo de ameaça, CSP/Tipos confiáveis, as etapas do pen-test, as etapas do pen-test e o DOCS-1b e o gate کرتے ہیں۔

## Norito-RPC Nome

Solicitações Norito-RPC de proxy e encanamento OAuth کو شیئر کرتی ہیں جیسے rotas JSON; یہ صرف `Content-Type: application/x-norito` سیٹ کرتی ہیں اور especificação NRPC میں بیان کردہ carga útil Norito pré-codificada بھیجتی ہیں
(`docs/source/torii/nrpc_spec.md`)۔ ریپو میں `fixtures/norito_rpc/` کے تحت payloads canônicos موجود ہیں تاکہ autores de portal, proprietários de SDK, revisores e replay de bytes کر سکیں جو CI استعمال کرتا ہے۔

### Try It console سے Norito carga útil بھیجیں

1. `fixtures/norito_rpc/transfer_asset.norito` Dispositivo elétrico de fixação منتخب کریں۔ یہ فائلیں envelopes Norito brutos ہیں؛ انہیں **codificação base64 نہ کریں**۔
2. Swagger é o RapiDoc com o endpoint NRPC definido para (`POST /v2/pipeline/submit`) e o seletor **Content-Type** e `application/x-norito` é o seletor de **Content-Type**.
3. editor de corpo de solicitação کو **binário** پر ٹوگل کریں (Swagger کا "Arquivo" موڈ یا RapiDoc کا seletor "Binário/Arquivo") اور `.norito` فائل اپ لوڈ کریں۔ widget bytes کو proxy کے ذریعے بغیر تبدیلی کے stream کرتا ہے۔
4. solicitação de solicitação اگر Torii `X-Iroha-Error-Code: schema_mismatch` واپس کرے تو تصدیق کریں کہ آپ ایسا endpoint کال کر رہے ہیں جو cargas úteis binárias قبول کرتا ہے اور `fixtures/norito_rpc/schema_hashes.json` میں ریکارڈ شدہ esquema hash آپ کے Torii build سے میچ کرتا ہے۔

Você pode usar tokens de autorização para hosts Torii Carga útil útil `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کو fluxo de trabalho میں شامل کرنے سے NRPC-4 plano de adoção میں حوالہ دیا گیا pacote de evidências (log + resumo JSON) تیار ہوتا ہے، جو comentários کے دوران Resposta Try It کے captura de tela کے ساتھ اچھی طرح جاتا ہے۔

### CLI Função (curl)

وہی fixtures `curl` کے ذریعے پورٹل سے باہر بھی replay کیے جا سکتے ہیں, جو proxy validar کرنے یا respostas de gateway depurar O que fazer:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں موجود کسی بھی entrada کے ساتھ fixture بدلیں یا `cargo xtask norito-rpc-fixtures` سے اپنا codificação de carga útil کریں۔ جب Torii modo canário میں ہو تو آپ `curl` کو proxy try-it (`https://docs.sora.example/proxy/v2/pipeline/submit`) پر پوائنٹ کر سکتے ہیں Teste de infraestrutura e teste de infraestrutura e widgets de portal

## Observabilidade e operaçõesہر solicitação ایک بار método, caminho, origem, status upstream, e fonte de autenticação (`override`, `default`, یا `client`) کے ساتھ log ہوتی ہے۔ tokens کبھی armazenar نہیں ہوتے - cabeçalhos de portador اور `X-TryIt-Auth` registro de valores سے پہلے redigir ہو جاتی ہیں, اس لئے آپ stdout کو coletor central میں encaminhar کر سکتے ہیں بغیر vazamento de segredos ہونے کے خدشے کے۔

### Sondas de saúde e alertas

implantações کے دوران یا agendamento e sondagem agrupada چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Botões de ambiente:

- `TRYIT_PROXY_SAMPLE_PATH` - Rota Torii (بغیر `/proxy`) جسے چیک کرنا ہو۔
- `TRYIT_PROXY_SAMPLE_METHOD` - ڈیفالٹ `GET`؛ escrever rotas کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - amostra de chamada کے لئے عارضی bearer token inject کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - Tempo limite de 5 s e substituição کرتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus destino do arquivo de texto۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے métricas separadas por vírgula میں شامل ہوتے ہیں (ڈیفالٹ `job=tryit-proxy` ou `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - اختیاری URL de endpoint de métricas (مثلاً `http://localhost:9798/metrics`) ou `TRYIT_PROXY_METRICS_LISTEN` فعال ہونے پر کامیابی سے جواب sim

نتائج کو coletor de arquivo de texto میں فیڈ کریں, probe کو caminho gravável پر پوائنٹ کر کے
(مثلاً `/var/lib/node_exporter/textfile_collector/tryit.prom`) E etiquetas personalizadas são as seguintes:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ métricas فائل کو reescrever atomicamente کرتا ہے تاکہ coletor ہمیشہ مکمل carga útil پڑھے۔

جب `TRYIT_PROXY_METRICS_LISTEN` configure ہو تو `TRYIT_PROXY_PROBE_METRICS_URL` کو endpoint de métricas پر سیٹ کریں تاکہ probe تیزی سے fail ہو اگر raspar superfície غائب ہو جائے (como entrada mal configurada یا regras de firewall ausentes)۔ ایک configuração de produção típica ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

لائٹ ویٹ alertando کے لئے probe کو اپنے pilha de monitoramento میں جوڑیں۔ Prometheus contém todas as falhas na página da página کرتی ہے:

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

### Endpoint de métricas em painéis

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (یا کوئی host/porta) سیٹ کریں اور پھر proxy start کریں تاکہ Prometheus exposição de endpoint de métricas formatadas ہو۔ caminho ڈیفالٹ `/metrics` ہے لیکن `TRYIT_PROXY_METRICS_PATH=/custom` سے بدل سکتے ہیں۔ Os totais do método de raspagem, as rejeições de limite de taxa, os erros/tempos limite de upstream, os resultados do proxy, os resumos de latência e os resultados:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Coletores Prometheus/OTLP کو endpoint de métricas پر پوائنٹ کریں اور `dashboards/grafana/docs_portal.json` کے painéis existentes کو reutilização کریں تاکہ Latências finais SRE اور picos de rejeição کو logs analisar proxy خودکار طور پر `tryit_proxy_start_timestamp_ms` publicar کرتا ہے تاکہ operadores reinicializar detectar کر سکیں۔

### Automação de reversão

auxiliar de gerenciamento استعمال کریں تاکہ Torii URL de destino کو atualizar یا restaurar کیا جا سکے۔ اسکرپٹ پچھلی configuração کو `.env.tryit-proxy.bak` میں محفوظ کرتا ہے تاکہ rollback ایک ہی کمانڈ ہو۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

A configuração de implantação é a configuração de implantação que é definida como `--env` e `TRYIT_PROXY_ENV` com env e substituição de substituição کریں۔