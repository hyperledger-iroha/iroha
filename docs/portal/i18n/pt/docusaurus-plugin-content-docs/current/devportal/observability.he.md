---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 21c400df98d594941921465619b0ea58a16d11f182ea19dc26928ea34a1d45fd
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Observabilidade e analytics do portal

O roadmap DOCS-SORA exige analytics, probes sinteticos e automacao de links quebrados
para cada build de preview. Esta nota documenta a infraestrutura que agora acompanha o portal
para que operadores conectem monitoramento sem vazar dados de visitantes.

## Tagging de release

- Defina `DOCS_RELEASE_TAG=<identifier>` (fallback para `GIT_COMMIT` ou `dev`) ao
  buildar o portal. O valor e injetado em `<meta name="sora-release">`
  para que probes e dashboards distingam deployments.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) descrevendo o tag, timestamp e o
  `DOCS_RELEASE_SOURCE` opcional. O mesmo arquivo e empacotado nos artefatos de preview e
  referenciado pelo relatorio do link checker.

## Analytics com preservacao de privacidade

- Configure `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  habilitar o tracker leve. Payloads contem `{ event, path, locale, release, ts }`
  sem metadata de referrer ou IP, e `navigator.sendBeacon` e usado sempre que possivel
  para evitar bloquear navegacoes.
- Controle o sampling com `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). O tracker armazena
  o ultimo path enviado e nunca emite eventos duplicados para a mesma navegacao.
- A implementacao fica em `src/components/AnalyticsTracker.jsx` e e montada
  globalmente via `src/theme/Root.js`.

## Probes sinteticos

- `npm run probe:portal` dispara requests GET contra rotas comuns
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) e verifica se o
  meta tag `sora-release` corresponde a `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas sao reportadas por path, facilitando gatear o CD pelo sucesso dos probes.

## Automacao de links quebrados

- `npm run check:links` varre `build/sitemap.xml`, garante que cada entrada mapeia para
  um arquivo local (checando fallbacks `index.html`), e escreve
  `build/link-report.json` contendo metadata de release, totais, falhas e a impressao
  SHA-256 de `checksums.sha256` (exposta como `manifest.id`) para que cada relatorio possa
  ser ligado ao manifesto do artefato.
- O script termina com codigo nao-zero quando falta uma pagina, assim a CI pode bloquear
  releases em rotas antigas ou quebradas. Os relatorios citam os caminhos candidatos tentados,
  o que ajuda a rastrear regressoes de roteamento de volta para a arvore de docs.

## Dashboard Grafana e alertas

- `dashboards/grafana/docs_portal.json` publica o board Grafana **Docs Portal Publishing**.
  Ele inclui os seguintes paineis:
  - *Gateway Refusals (5m)* usa `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` para que SREs detectem pushes de politica ruins ou falhas de tokens.
  - *Alias Cache Refresh Outcomes* e *Alias Proof Age p90* acompanham
    `torii_sorafs_alias_cache_*` para provar que proofs recentes existem antes de um cut
    over de DNS.
  - *Pin Registry Manifest Counts* e a estatistica *Active Alias Count* espelham o
    backlog do pin-registry e o total de aliases para que a governanca possa auditar
    cada release.
  - *Gateway TLS Expiry (hours)* destaca quando o cert TLS do gateway de publishing
    se aproxima do vencimento (limiar de alerta em 72 h).
  - *Replication SLA Outcomes* e *Replication Backlog* acompanham a telemetria
    `torii_sorafs_replication_*` para garantir que todas as replicas atendam o
    patamar GA apos a publicacao.
- Use as variaveis de template embutidas (`profile`, `reason`) para focar no perfil
  de publishing `docs.sora` ou investigar picos em todos os gateways.
- O routing do PagerDuty usa os paineis do dashboard como evidencia: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry`
  disparam quando a serie correspondente ultrapassa seus limiares. Ligue o runbook
  do alerta a esta pagina para que o on-call consiga repetir as queries exatas do Prometheus.

## Juntando tudo

1. Durante `npm run build`, defina as variaveis de ambiente de release/analytics e
   deixe o pos-build emitir `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Rode `npm run probe:portal` contra o hostname de preview com
   `--expect-release` conectado ao mesmo tag. Salve o stdout para a checklist de publishing.
3. Rode `npm run check:links` para falhar rapido em entradas quebradas do sitemap e arquive
   o relatorio JSON gerado junto com os artefatos de preview. A CI deposita o
   ultimo relatorio em `artifacts/docs_portal/link-report.json` para que a governanca
   baixe o bundle de evidencias direto dos logs de build.
4. Encaminhe o endpoint de analytics para seu coletor com preservacao de privacidade (Plausible,
   OTEL ingest self-hosted, etc.) e garanta que as taxas de amostragem estejam documentadas por
   release para que os dashboards interpretem os volumes corretamente.
5. A CI ja conecta esses passos nos workflows de preview/deploy
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), entao os dry runs locais so precisam
   cobrir comportamento especifico de segredos.
