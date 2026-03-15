---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: Publicacao e monitoramento SoraFS
sidebar_label: Publicacao e monitoramento
description: Captura o fluxo de monitoramento end-to-end para releases do portal SoraFS para que DOCS-3c tenha probes deterministas, telemetria e bundles de evidencia.
---

O item de roadmap **DOCS-3c** exige mais do que um checklist de empacotamento: apos cada
publicacao SoraFS devemos provar continuamente que o portal do desenvolvedor, o proxy Try it
e os bindings do gateway permanecem saudaveis. Esta pagina documenta a superficie de monitoramento
que acompanha o [guia de deploy](./deploy-guide.md) para que CI e engenheiros on call possam
aplicar as mesmas verificacoes que Ops usa para impor o SLO.

## Recapitulacao do pipeline

1. **Build e assinatura** - siga o [guia de deploy](./deploy-guide.md) para rodar
   `npm run build`, `scripts/preview_wave_preflight.sh`, e as etapas de envio Sigstore + manifest.
   O script de preflight emite `preflight-summary.json` para que cada preview carregue
   metadata de build/link/probe.
2. **Pin e verificacao** - `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   e o plano de cutover DNS fornecem artefatos deterministas para a governanca.
3. **Arquivar evidencia** - guarde o resumo CAR, bundle Sigstore, proof de alias,
   saida de probe e snapshots do dashboard `docs_portal.json` sob
   `artifacts/sorafs/<tag>/`.

## Canais de monitoramento

### 1. Monitores de publicacao (`scripts/monitor-publishing.mjs`)

O novo comando `npm run monitor:publishing` agrupa o probe do portal, o probe do proxy Try it
e o verificador de bindings em um unico check amigavel para CI. Forneca uma config JSON
(guardada nos secrets de CI ou `configs/docs_monitor.json`) e execute:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Adicione `--prom-out ../../artifacts/docs_monitor/monitor.prom` (e opcionalmente
`--prom-job docs-preview`) para emitir metricas em formato de texto Prometheus
adequadas para Pushgateway ou scrapes diretos em staging/production. As metricas
espelham o resumo JSON para que dashboards de SLO e regras de alerta possam
acompanhar a saude do portal, Try it, bindings e DNS sem parsear o bundle de evidencia.

Exemplo de config com knobs requeridos e multiplos bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/i105.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

O monitor escreve um resumo JSON (friendly para S3/SoraFS) e sai com codigo diferente de zero
quando algum probe falha, tornando-o adequado para Cron jobs, steps de Buildkite ou webhooks
Alertmanager. Passar `--evidence-dir` persiste `summary.json`, `portal.json`, `tryit.json` e
`binding.json` junto com um manifest `checksums.sha256` para que reviewers de governanca
possam comparar resultados sem reexecutar probes.

> **TLS guardrail:** `monitorPortal` rejeita URLs base `http://` a menos que
> `allowInsecureHttp: true` esteja definido na config. Mantenha probes production/staging
> em HTTPS; a opcao existe apenas para previews locais.

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.


O bloco opcional `dns` conecta o rollout SoraDNS do DOCS-7 ao mesmo monitor.
Cada entrada resolve um par hostname/record-type (por exemplo o CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) e confirma
que as respostas batem com `expectedRecords` ou `expectedIncludes`. A segunda
entrada do snippet acima fixa o hostname canonico hasheado produzido por
`cargo xtask soradns-hosts --name docs-preview.sora.link`; o monitor agora prova
que tanto o alias friendly quanto o hash canonico (`igjssx53...gw.sora.id`)
resolvem para o pretty host fixado. Isso torna automatica a evidencia de promocao DNS:
o monitor falhara se qualquer host desviar, mesmo quando os bindings HTTP
continuam staplando o manifest correto.

### 2. Verificacao do manifesto de versoes OpenAPI

A exigencia DOCS-2b de "manifest OpenAPI assinado" agora possui um guard automatizado:
`ci/check_openapi_spec.sh` chama `npm run check:openapi-versions`, que invoca
`scripts/verify-openapi-versions.mjs` para cruzar
`docs/portal/static/openapi/versions.json` com as specs e manifests reais de Torii.
O guard verifica que:

- Cada versao listada em `versions.json` possui um diretorio correspondente em
  `static/openapi/versions/`.
- Os campos `bytes` e `sha256` batem com o arquivo on-disk da spec.
- O alias `latest` reflete a entrada `current` (metadata de digest/size/signature)
  para que o download padrao nao derive.
- Entradas assinadas referenciam um manifest cujo `artifact.path` aponta de volta
  para a mesma spec e cujos valores de assinatura/chave publica em hex batem com o manifest.

Rode o guard localmente sempre que espelhar uma nova spec:

```bash
cd docs/portal
npm run check:openapi-versions
```

As mensagens de falha incluem a dica de arquivo desatualizado (`npm run sync-openapi -- --latest`)
para que contribuidores do portal saibam como atualizar os snapshots.
Manter o guard em CI evita releases do portal em que o manifest assinado e o digest
publicado ficam fora de sincronia.

### 2. Dashboards e alertas

- **`dashboards/grafana/docs_portal.json`** - board principal para DOCS-3c. Os panels
  acompanham `torii_sorafs_gateway_refusals_total`, falhas de SLA de replicacao, erros do
  proxy Try it e latencia de probes (overlay `docs.preview.integrity`). Exporte o board
  apos cada release e anexe ao ticket de operacoes.
- **Alertas do proxy Try it** - a regra Alertmanager `TryItProxyErrors` dispara com
  quedas sustentadas de `probe_success{job="tryit-proxy"}` ou picos de
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` garante que os bindings de alias
  continuem anunciando o digest do manifest fixado; escalations apontam para a
  transcricao do CLI `cargo xtask soradns-verify-binding` capturada durante a publicacao.

### 3. Trilho de evidencia

Cada execucao de monitoramento deve anexar:

- Bundle de evidencia do `monitor-publishing` (`summary.json`, arquivos por secao e
  `checksums.sha256`).
- Screenshots do Grafana para o board `docs_portal` na janela de release.
- Transcripts de change/rollback do proxy Try it (logs de `npm run manage:tryit-proxy`).
- Saida de verificacao de alias de `cargo xtask soradns-verify-binding`.

Armazene isto em `artifacts/sorafs/<tag>/monitoring/` e linke no issue de release
para que a trilha de auditoria sobreviva apos a expiracao dos logs de CI.

## Checklist operacional

1. Rode o guia de deploy ate a etapa 7.
2. Execute `npm run monitor:publishing` com configuracao de producao; arquive
   a saida JSON.
3. Capture os panels do Grafana (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) e anexe ao ticket de release.
4. Agende monitores recorrentes (recomendado: a cada 15 minutos) apontando para
   URLs de producao com a mesma config para satisfazer o gate SLO do DOCS-3c.
5. Durante incidentes, reexecute o comando de monitor com `--json-out` para
   registrar evidencia antes/depois e anexar ao postmortem.

Seguir este loop encerra DOCS-3c: o fluxo de build do portal, o pipeline de publicacao
e o stack de monitoramento agora vivem em um unico playbook com comandos reproduziveis,
configs de exemplo e hooks de telemetria.
