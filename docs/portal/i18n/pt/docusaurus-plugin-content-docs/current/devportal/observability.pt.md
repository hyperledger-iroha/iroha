---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidade e análise do portal

O roadmap DOCS-SORA exige análises, sondas sintéticas e automação de links quebrados
para cada build de visualização. Esta nota documenta a infraestrutura que agora acompanha o portal
para que operadores conectem monitoramento sem vazar dados de visitantes.

## Marcação de lançamento

- Defina `DOCS_RELEASE_TAG=<identifier>` (substituição para `GIT_COMMIT` ou `dev`) ao
  construir o portal. O valor e injetado em `<meta name="sora-release">`
  para que probes e dashboards distingam implantações.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) descrevendo a tag, timestamp e o
  `DOCS_RELEASE_SOURCE` opcional. O mesmo arquivo e embaladodo nos artefatos de visualização e
  referenciado pelo relatorio do link checker.

## Analytics com preservação de privacidade

- Configurar `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  ativar o nível do rastreador. Cargas úteis contendo `{ event, path, locale, release, ts }`
  sem metadados de referência ou IP, e `navigator.sendBeacon` e usados sempre que possível
  para evitar bloquear navegações.
- Controle de amostragem com `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). O tracker armazena
  o último caminho enviado e nunca emite eventos duplicados para a mesma navegação.
- A implementação fica em `src/components/AnalyticsTracker.jsx` e e montado
  globalmente via `src/theme/Root.js`.

## Sondas sintéticas

- `npm run probe:portal` solicitações diferentes GET contra rotas comuns
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) e verifique se o
  A meta tag `sora-release` corresponde a `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas são reportadas por path, facilitando gatear o CD pelo sucesso dos probes.

## Automação de links quebrados

- `npm run check:links` varre `build/sitemap.xml`, garante que cada entrada mapeia para
  um arquivo local (checando fallbacks `index.html`), e escreve
  `build/link-report.json` contém metadados de lançamento, totais, falhas e impressão
  SHA-256 de `checksums.sha256` (exposta como `manifest.id`) para que cada relato possa
  estar ligado ao manifesto dos artistas.
- O script termina com código não-zero quando falta uma página, assim a CI pode bloquear
  lançamentos em rotas antigas ou quebradas. Os relatos citam os caminhos candidatos tentados,
  o que ajuda a rastrear regressos de roteamento de volta para a árvore de documentos.

## Dashboard Grafana e alertas

- `dashboards/grafana/docs_portal.json` publicação na placa Grafana **Publicação no Portal de Documentos**.
  Ele inclui os seguintes paineis:
  - *Recusas de gateway (5m)* usa `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` para que SREs detectem pushes de política, ruínas ou falhas de tokens.
  - *Alias Cache Refresh Outcomes* e *Alias Proof Age p90* acompanham
    `torii_sorafs_alias_cache_*` para provar que provas recentes existem antes de um corte
    sobre o DNS.
  - *Pin Registry Manifest Counts* e a estatística *Active Alias Count* espelham o
    backlog do pin-registry e o total de aliases para que a governança possa auditar
    cada lançamento.
  - *Gateway TLS Expiry (hours)* destaca quando o cert TLS do gateway de publicação
    se aproxima do vencimento (limiar de alerta em 72 h).
  - *Replication SLA Outcomes* e *Replication Backlog* acompanham a telemetria
    `torii_sorafs_replication_*` para garantir que todas as réplicas atendam o
    patamar GA após publicação.
- Utilize as variáveis de template embutidos (`profile`, `reason`) para focar no perfil
  de publicar `docs.sora` ou investigar picos em todos os gateways.
- O roteamento do PagerDuty usa os painéis do dashboard como evidência: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry`
  disparam quando a série correspondente ultrapassa seus limites. Ligue o runbook
  do alerta a esta página para que o on-call consiga repetir as consultas exatas do Prometheus.

## Juntando tudo1. Durante `npm run build`, defina as variáveis de ambiente de release/analytics e
   deixe o pós-build emitir `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Rode `npm run probe:portal` contra o nome do host de preview com
   `--expect-release` conectado à mesma tag. Salve o stdout para uma lista de verificação de publicação.
3. Rode `npm run check:links` para falhar rapidamente em entradas quebradas do sitemap e arquivo
   o relatorio JSON gerado junto com os artistas de visualização. Um depósito CI o
   último relato em `artifacts/docs_portal/link-report.json` para que a governança
   baixe o pacote de evidências direto dos logs de build.
4. Encaminhe o endpoint de análise para seu coletor com preservação de privacidade (Plausível,
   OTEL inger self-hosted, etc.) e garanta que as taxas de amostragem estejam documentadas por
   release para que os dashboards interpretem os volumes corretamente.
5. Um CI já conecta essas etapas nos fluxos de trabalho de visualização/implantação
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), então os dry runs locais são necessários
   cobrir comportamento específico de segredos.