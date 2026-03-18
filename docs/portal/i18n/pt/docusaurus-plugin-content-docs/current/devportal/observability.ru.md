---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Use e analise o portal

O cartão DOCS-SORA contém análise, sonda de síntese e automação
проверки битых ссылок для каждого preview build. Nesta descrição da infraestrutura,
você pode postar no portal, seus operadores podem monitorar o monitoramento
Não há necessidade de fazer isso.

## Тегирование релиза

- Instale `DOCS_RELEASE_TAG=<identifier>` (substituição para `GIT_COMMIT` ou `dev`) através do portal.
  Usando `<meta name="sora-release">`, suas sondas e painéis podem ser acessados
  развертывания.
- `npm run build` contém `build/release.json` (definido `scripts/write-checksums.mjs`), sua descrição
  тег, timestamp e opcional `DOCS_RELEASE_SOURCE`. Este arquivo foi atualizado no arquivo de visualização
  e упоминается no verificador de link.

## Análise de privacidade

- Instale `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para um rastreamento leve.
  O código `{ event, path, locale, release, ts }` é um referenciador ou metadado de IP, e mais
  Se você usar o `navigator.sendBeacon`, ele não bloqueará a navegação.
- Atualize a chave `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Трекер хранит последний отправленный caminho
  e никогда não отправляет дубликаты событий para одной навигации.
- Realize a instalação em `src/components/AnalyticsTracker.jsx` e instale-a globalmente em `src/theme/Root.js`.

## Teste de sintaxe

- `npm run probe:portal` делает GET-запросы на типичные маршруты (`/`, `/norito/overview`,
  `/reference/torii-swagger`, etc.) e prova, isso é o que `sora-release` é compatível
  `--expect-release` (ou `DOCS_RELEASE_TAG`). Exemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Сбои показываются по path, что упрощает gate CD по успеху проб.

## Автоматизация битых ссылок

- `npm run check:links` digitaliza `build/sitemap.xml`, убеждается, что каждая запись мапится на локальный файл
  (proveryaя fallback `index.html`), e selecione `build/link-report.json`, содержащий метаданные релиза, итоги,
  ошибки и SHA-256 отпечаток `checksums.sha256` (usado como `manifest.id`), чтобы каждый отчет можно
  Ele foi criado com um artefato artístico.
- Скрипт завершается с ненулевым кодом, когда страница отсутствует, поэтому CI может блокировать релизы
  при устаревших или сломанных маршрутах. Отчеты содержат кандидатные пути, которые пытались открыть, что
  помогает проследить регрессию маршрутизации до дерева docs.

## Painel Grafana e alertas

- `dashboards/grafana/docs_portal.json` publicado Grafana доску **Publicação no Portal de Documentos**.
  Para ativar o painel de controle:
  - *Recusas de gateway (5m)* usado `torii_sorafs_gateway_refusals_total` na resolução
    `profile`/`reason`, este SRE pode gerar um pacote de política push ou um token.
  - *Resultados de atualização de cache de alias* e *Alias Proof Age p90* отслеживают `torii_sorafs_alias_cache_*`,
    чтобы подтвердить наличие свежих prova antes do corte de DNS.
  - *Pin Registry Manifest Counts* e стат *Active Alias Count* отражают backlog pin-registry e общее
    число alias, чтобы governança могла аудировать каждый релиз.
  - *Expiração do Gateway TLS (horas)* подсвечивает приближение истечения Gateway de publicação de certificado TLS
    (período de espera 72 horas).
  - *Resultados de SLA de replicação* e *Backlog de replicação* relacionados à telefonia
    `torii_sorafs_replication_*`, este é o produto que está disponível em todas as suas réplicas.
- Используйте встроенные modelo переменные (`profile`, `reason`), opções de foco
  perfil de publicação `docs.sora` ou исследовать всплески по всем шлюзам.
- Роутинг PagerDuty использует панели дашборда как доказательство: алерты
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry` são compatíveis,
  que a série de problemas é necessária para o trabalho. Use o alerta do runbook nesta página,
  Os engenheiros de plantão podem usar o recurso Prometheus.

## Сводим вместе1. No final do `npm run build`, configure o período de liberação/análises e data de pós-construção
   Selecione `checksums.sha256`, `release.json` e `link-report.json`.
2. Selecione `npm run probe:portal` para visualizar o nome do host com `--expect-release`, связанным с тем же тегом.
   Сохраните stdout para a seção de publicação.
3. Abra `npm run check:links`, instale-o no sitemap e arquive-o
   O gerador JSON é fornecido com os artefatos de visualização. CI кладет последний отчет в
   `artifacts/docs_portal/link-report.json`, este pacote de evidências de governança pode ser coletado por meio de construção de logs.
4. Proteja o endpoint analítico em seu coletor de preservação de privacidade (ingestão OTEL plausível e auto-hospedada, etc.)
   e убедитесь, qual é o documento de taxa de amostragem para a confiabilidade, os painéis de controle corretos e integrados
   счетчики.
5. CI é projetado para visualizar/implantar fluxos de trabalho
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), поэтому локальные dry runs должны покрывать
   только поведение, связанное с секретами.