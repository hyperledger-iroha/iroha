---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidade e análise do portal

O roteiro DOCS-SORA exige análises, sondas sintéticas e uma automatização de garantias
casos para cada build de visualização. Esta nota documenta o planejamento livre com o portal abaixo
que os operadores podem ramificar o monitoramento sem expor os dados dos visitantes.

## Marcação de lançamento

- Definir `DOCS_RELEASE_TAG=<identifier>` (fallback em `GIT_COMMIT` ou `dev`) por
  construir o portal. O valor é injetado em `<meta name="sora-release">`
  para que os testes e painéis diferenciem as implantações.
- `npm run build` gênero `build/release.json` (par escrito
  `scripts/write-checksums.mjs`) que descreve a tag, o carimbo de data/hora e o
  Opcional `DOCS_RELEASE_SOURCE`. O arquivo meme foi embarcado na visualização dos artefatos e
  referência para o relacionamento do verificador de links.

## Analytics respeita a vida privada

- Configurador `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para ativo
  le rastreador leger. As cargas úteis contêm `{ event, path, locale, release, ts }`
  sem metadados de referência ou IP, e `navigator.sendBeacon` é utilizado o que for possível
  para evitar bloquear a navegação.
- Controle o echantillonnage com `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Le rastreador conservar
  le dernier path envoye et n'emmet jamais d'evenements duplicados para a navegação meme.
- A implementação foi encontrada em `src/components/AnalyticsTracker.jsx` e está montada
  globalização via `src/theme/Root.js`.

## Sondas sintéticas

- `npm run probe:portal` emet des requetes GET nas rotas correntes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) e verifique se ele
  a meta tag `sora-release` corresponde a `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les echecs são relatados por caminho, o que facilita o acesso ao CD no sucesso das sondas.

## Automatização de casos de garantias

- `npm run check:links` scanne `build/sitemap.xml`, certifique-se de que cada mapa entre um e outro
  arquivo local (verificando os substitutos `index.html`), e ecrit
  `build/link-report.json` contém os metadados de lançamento, os totais, os echecs et
  a empresa SHA-256 de `checksums.sha256` (exposta como `manifest.id`) até cada uma
  rapport puisse etre rattache au manifeste d'artefact.
- O script retorna um código diferente de zero quando uma página está perdida, para que o CI possa bloquear
  os lançamentos nas rotas obsoletas ou cassees. Os relatórios citam os candidatos aos candidatos
  Tente, isso é o que ajuda a traçar as regressões de rota até a árvore de documentos.

## Dashboard Grafana e alertas

- `dashboards/grafana/docs_portal.json` publica o quadro Grafana **Publicação no Portal de Documentos**.
  Ele contém os seguintes painéis:
  - *Recusas de gateway (5m)* utilizam par de escopo `torii_sorafs_gateway_refusals_total`
    `profile`/`reason` para que o SRE detecte impulsos políticos incorretos ou
    verificações de tokens.
  - *Resultados de atualização do cache de alias* e *Alias Proof Age p90* a seguir
    `torii_sorafs_alias_cache_*` para provar que as provas frescas existem antes do corte
    sobre DNS.
  - *Pin Registry Manifest Counts* e a estatística *Active Alias Count* reflete
    backlog do registro de pinos e o total de aliases para que o governo possa auditar
    lançamento de cada.
  - *Expiração do Gateway TLS (horas)* encontrada antes da abordagem da expiração do certificado TLS do
    gateway de publicação (seuil d'alerte às 72 h).
  - *Replication SLA Outcomes* e *Replication Backlog* monitorando a telemetria
    `torii_sorafs_replication_*` para garantir que todas as réplicas respeitem
    nível GA após publicação.
- Utilize as variáveis ​​integradas do modelo (`profile`, `reason`) para concentrá-lo em
  o perfil de publicação `docs.sora` ou enqueter sur des pics sur l'ensemble des gateways.
- O roteamento PagerDuty utiliza os painéis do painel como antes: os alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry`
  se declinou quando a série correspondente depasse son seuil. Leia o runbook de
  Alerte esta página para que o chamador possa atender as solicitações Prometheus exatas.

## Assembler tudo1. Pendente `npm run build`, defina as variáveis de ambiente de release/analytics et
   laisser a fita pós-construção emetre `checksums.sha256`, `release.json` et
   `link-report.json`.
2. Executor `npm run probe:portal` com visualização quente com
   `--expect-release` ramifica-se na tag meme. Salvar o padrão para a lista de verificação
   de publicação.
3. Execute `npm run check:links` para ecoar rapidamente nas entradas dos casos de mapa do site
   e arquivar o relatório JSON gerado com a visualização dos artefatos. La CI depõe le
   melhor relacionamento em `artifacts/docs_portal/link-report.json` para o governo
   Você pode baixar o pacote de testes diretamente a partir dos logs de compilação.
4. Router l'endpoint analytics vers votre colecionador respeitoso da vida privada (Plausível,
   OTEL ingere auto-heberge, etc.) e certifique-se de que as taxas de echantillonnage sejam documentadas
   par release afin que os painéis interpretam a correção dos volumes.
5. O cabo CI deixou essas etapas por meio da visualização/implantação dos fluxos de trabalho
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), pois os locais de funcionamento a seco não devem ser cobertos
   que o comportamento específico dos segredos.