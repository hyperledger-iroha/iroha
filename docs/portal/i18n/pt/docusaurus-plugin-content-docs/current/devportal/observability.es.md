---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidade e análise do portal

O roteiro DOCS-SORA requer análise, sondas sintéticas e automatização de laços rotos
para cada build de visualização. Esta nota documenta a plomeria que agora viene com o portal
para que os operadores possam conectar o monitor sem filtrar dados de visitantes.

## Etiquetado de lançamento

- Definir `DOCS_RELEASE_TAG=<identifier>` (fazer fallback para `GIT_COMMIT` ou `dev`) al
  construir o portal. O valor é indicado em `<meta name="sora-release">`
  para que probes e dashboards distingam aplicações.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) descrevendo a tag, timestamp e el
  `DOCS_RELEASE_SOURCE` opcional. O mesmo arquivo é empacotado nos artefatos de visualização e
  é referenciado no relatório do verificador de links.

## Análise com preservação da privacidade

- Configurar `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  ativar o rastreador liviano. As cargas úteis contêm `{ evento, caminho, localidade,
  lançamento, ts}` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  sempre que for possível evitar o bloqueio de navegações.
- Controle o mapa com `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). El rastreador guarda
  o caminho final foi enviado e nunca emite eventos duplicados para a má navegação.
- A implementação vive em `src/components/AnalyticsTracker.jsx` e é montada
  globalmente através de `src/theme/Root.js`.

## Sondas sintéticas

- `npm run probe:portal` emite solicitações GET contra rutas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) e verifique se o
  meta tag `sora-release` coincide com `--expect-release` (o
  `DOCS_RELEASE_TAG`). Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

As falhas são relatadas por caminho, o que facilita o acesso ao CD com o resultado da sonda.

## Automatização de links rotos

- `npm run check:links` escanea `build/sitemap.xml`, certifique-se de que cada entrada mapea a un
  arquivo local (chequeando fallbacks `index.html`), e escreva
  `build/link-report.json` com metadados de liberação, totais, falhas e impressão digital
  SHA-256 de `checksums.sha256` (exposto como `manifest.id`) para cada relatório
  pode ser vinculado à manifestação do artefato.
- El script sale con codigo distinto de cero cuando falta una pagina, asi que CI puede
  bloquear lançamentos em rotas obsoletas ou rotas. Os relatórios citam as rotas candidatas
  que se tentou, o que ajudou a rastrear regiões de roteamento até o arquivo de documentos.

## Dashboard e alertas de Grafana

- `dashboards/grafana/docs_portal.json` publica a tabela Grafana **Docs Portal Publishing**.
  Inclui os seguintes painéis:
  - *Recusas de gateway (5m)* usa `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` para que o SRE detecte pushes políticos ruins ou falhas de tokens.
  - *Resultados de atualização do cache de alias* e *Idade da prova de alias p90* a seguir
    `torii_sorafs_alias_cache_*` para demonstrar que existem provas de afrescos antes de serem cortados
    sobre o DNS.
  - *Pin Registry Manifest Counts* e a estatística *Active Alias Count* reflete o
    backlog do registro de pinos e os aliases totais para que o governo possa auditar
    cada lançamento.
  - *Gateway TLS Expiry (hours)* destaca quando o certificado TLS do gateway de publicação
    se acerca do vencimiento (umbral de alerta em 72 h).
  - *Resultados de SLA de replicação* e *Backlog de replicação* vigiando a telemetria
    `torii_sorafs_replication_*` para garantir que todas as réplicas cumpram o
    nível GA após publicação.
- Use as variáveis da planta integrada (`profile`, `reason`) para enfocar no
  perfil de publicação `docs.sora` o investiga picos em todos os gateways.
- O roteamento do PagerDuty usa os painéis do painel como evidência: os alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry`
  disparan cuando la serie correspondente cruza su umbral. Abra o runbook do alerta
  esta página para que o on-call possa reproduzir as consultas exatas de Prometheus.

## Poniendolo em conjunto1. Durante `npm run build`, defina as variáveis de ambiente de release/analítica e
   deixe que o pós-construção emita `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Execute `npm run probe:portal` contra o nome do host de visualização com
   `--expect-release` conectado ao mesmo tag. Guarda o padrão para a lista de verificação de publicação.
3. Execute `npm run check:links` para cair rapidamente nas entradas das rotas do mapa do site e arquivar
   o relatório JSON gerado junto com os artefatos de visualização. CI deixou o último relatório em
   `artifacts/docs_portal/link-report.json` para que o governo possa baixar o pacote de evidências
   direto dos logs de build.
4. Entre no endpoint de análise para seu coletor com preservação de privacidade (Plausível,
   OTEL ingere auto-hospedado, etc.) e certifique-se de que as tarefas de museu sejam documentadas por
   release para que os painéis interpretem corretamente o conteúdo.
5. CI você conecta essas etapas aos fluxos de trabalho de visualização/implantação
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), assim como os locais de execução a seco são apenas necessários
   descobrir comportamento específico de segredos.