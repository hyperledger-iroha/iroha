---
lang: pt
direction: ltr
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<!-- Tradução para português de docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Guia do Sandbox “Try It”
summary: Como executar o proxy de staging do Torii e o sandbox do portal de desenvolvedores.
---

O portal de desenvolvedores fornece um console “Try it” para a API REST do Torii. Este
guia explica como subir o proxy de suporte e conectar o console a um gateway de staging
sem expor credenciais.

## Pré‑requisitos

- Checkout do repositório Iroha (raiz do workspace).
- Node.js 18.18+ (alinhado com o baseline do portal).
- Endpoint Torii acessível a partir da sua máquina (staging ou local).

## 1. Gerar o snapshot OpenAPI (opcional)

O console reutiliza o mesmo payload OpenAPI das páginas de referência do portal. Se você
alterou rotas do Torii, regenere o snapshot:

```bash
cargo xtask openapi
```

O comando grava `docs/portal/static/openapi/torii.json`.

## 2. Iniciar o proxy do Try It

A partir da raiz do repositório:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Defaults opcionais
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Variáveis de ambiente

| Variável | Descrição |
|----------|-----------|
| `TRYIT_PROXY_TARGET` | URL base do Torii (obrigatória). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por vírgulas de origens autorizadas a usar o proxy (default `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Token bearer opcional aplicado por padrão a todas as requisições proxied. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Defina como `1` para encaminhar o header `Authorization` do cliente como está. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Configuração do rate limiter em memória (padrão: 60 requisições a cada 60 s). |
| `TRYIT_PROXY_MAX_BODY` | Tamanho máximo de payload aceito (bytes, padrão 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout de upstream para requisições Torii (padrão 10 000 ms). |

O proxy expõe:

- `GET /healthz` — verificação de readiness.
- `/proxy/*` — requisições proxied, preservando path e query string.

## 3. Iniciar o portal

Em outro terminal:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Acesse `http://localhost:3000/api/overview` e use o console Try It. As mesmas variáveis
de ambiente configuram os embeds do Swagger UI e do RapiDoc.

## 4. Executar testes unitários

O proxy expõe uma suíte de testes rápida baseada em Node:

```bash
npm run test:tryit-proxy
```

Os testes cobrem parsing de endereços, tratamento de origens, rate limiting e injeção de
bearer token.

## 5. Automação de probes e métricas

Use o probe incluído para verificar `/healthz` e um endpoint de exemplo:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Variáveis relevantes:

- `TRYIT_PROXY_SAMPLE_PATH` — rota Torii opcional (sem `/proxy`) que você deseja exercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` — padrão `GET`; ajuste para `POST` em rotas de escrita.
- `TRYIT_PROXY_PROBE_TOKEN` — injeta um bearer token temporário para a chamada de exemplo.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — sobrescreve o timeout padrão de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — destino em formato textfile Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — lista `chave=valor` separada por vírgulas que compõe as labels (defaults `job=tryit-proxy` e `instance=<URL do proxy>`).

Quando `TRYIT_PROXY_PROBE_METRICS_FILE` está definido, o script reescreve o arquivo
atomícamente para que o node_exporter/textfile collector leia um payload completo. Exemplo:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Encaminhe as métricas para o Prometheus e reutilize a regra de alerta do portal para disparar
sempre que `probe_success` cair para `0`.

## 6. Checklist de endurecimento para produção

Antes de expor o proxy além do desenvolvimento local:

- Termine o TLS antes do proxy (reverse proxy ou gateway gerenciado).
- Configure logging estruturado e encaminhe‑o para os pipelines de observabilidade.
- Gire periodicamente os bearer tokens e armazene‑os em um gerenciador de segredos.
- Monitore o endpoint `/healthz` do proxy e agregue métricas de latência.
- Alinhe os limites de rate com as quotas de staging do Torii; ajuste o comportamento de
  `Retry-After` para comunicar throttling aos clientes.
