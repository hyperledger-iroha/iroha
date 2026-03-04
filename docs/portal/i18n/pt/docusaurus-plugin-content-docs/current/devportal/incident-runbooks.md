---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbooks de incidentes e drills de rollback

## Proposito

O item do roadmap **DOCS-9** exige playbooks acionaveis mais um plano de ensaio para que
operadores do portal consigam recuperar falhas de envio sem adivinhacao. Esta nota cobre tres
incidentes de alto sinal - deploys falhos, degradacao de replicacao e quedas de analytics - e
documenta os drills trimestrais que provam que o rollback de alias e a validacao sintetica
continuam funcionando end to end.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - workflow de packaging, signing e promocao de alias.
- [`devportal/observability`](./observability) - release tags, analytics e probes referenciados abaixo.
- `docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria do registro e limites de escalonamento.
- `docs/portal/scripts/sorafs-pin-release.sh` e helpers `npm run probe:*`
  referenciados nos checklists.

### Telemetria e tooling compartilhados

| Sinal / Tool | Proposito |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | Detecta bloqueios de replicacao e violacoes de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifica profundidade do backlog e latencia de completacao para triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Mostra falhas do gateway que frequentemente seguem um deploy ruim. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes sinteticos que gateiam releases e validam rollbacks. |
| `npm run check:links` | Gate de links quebrados; usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promocao/reversao de alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de refusals/alias/TLS/replicacao. Alertas do PagerDuty referenciam estes paineis como evidencia. |

## Runbook - Deploy falho ou artefato ruim

### Condicoes de disparo

- Probes de preview/producao falham (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` apos um rollout.
- QA manual nota rotas quebradas ou falhas do proxy Try it imediatamente apos
  a promocao do alias.

### Contencao imediata

1. **Congelar deploys:** marcar o pipeline CI com `DEPLOY_FREEZE=1` (input do workflow
   GitHub) ou pausar o job Jenkins para que nenhum artefato seja enviado.
2. **Capturar artefatos:** baixar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e a saida dos probes do build com falha para que
   o rollback referencie os digests exatos.
3. **Notificar stakeholders:** storage SRE, lead Docs/DevRel, e o duty officer de
   governanca para awareness (especialmente quando `docs.sora` esta impactado).

### Procedimento de rollback

1. Identifique o manifest last-known-good (LKG). O workflow de producao os armazena em
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincule o alias a esse manifest com o helper de shipping:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Registre o resumo do rollback no ticket do incidente junto com os digests do
   manifest LKG e do manifest com falha.

### Validacao

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (veja o guia de deploy) para confirmar que o manifest repromovido continua
   batendo com o CAR arquivado.
4. `npm run probe:tryit-proxy` para garantir que o proxy Try-It staging voltou.

### Pos-incidente

1. Reative o pipeline de deploy apenas depois de entender a causa raiz.
2. Preencha as entradas "Lessons learned" em [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se houver.
3. Abra defects para a suite de testes falhada (probe, link checker, etc.).

## Runbook - Degradacao de replicacao

### Condicoes de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (veja
  `pin-registry-ops.md`).
- Governanca reporta alias lento apos um release.

### Triage

1. Inspecione dashboards de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirmar se o backlog esta localizado em uma classe de storage ou em um fleet de providers.
2. Cruze logs do Torii por warnings `sorafs_registry::submit_manifest` para determinar se
   as submissions estao falhando.
3. Amostre a saude das replicas via `sorafs_cli manifest status --manifest ...` (lista
   resultados por provider).

### Mitigacao

1. Reemita o manifest com maior contagem de replicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que o scheduler distribua carga em um set maior
   de providers. Registre o novo digest no log do incidente.
2. Se o backlog estiver preso a um provider unico, desabilite-o temporariamente via o
   scheduler de replicacao (documentado em `pin-registry-ops.md`) e envie um novo
   manifest forcando os outros providers a atualizar o alias.
3. Quando a frescura do alias for mais critica que a paridade de replicacao, re-vincule o
   alias a um manifest quente ja em staging (`docs-preview`), depois publique um manifest de
   acompanhamento quando o SRE limpar o backlog.

### Recuperacao e fechamento

1. Monitore `torii_sorafs_replication_sla_total{outcome="missed"}` para garantir que o
   contador estabilize.
2. Capture a saida `sorafs_cli manifest status` como evidencia de que cada replica voltou
   a conformidade.
3. Abra ou atualize o post-mortem do backlog de replicacao com proximos passos
   (scaling de providers, tuning do chunker, etc.).

## Runbook - Queda de analytics ou telemetria

### Condicoes de disparo

- `npm run probe:portal` passa, mas dashboards param de ingerir eventos do
  `AnalyticsTracker` por >15 minutos.
- Privacy review aponta um aumento inesperado de eventos descartados.
- `npm run probe:tryit-proxy` falha em paths `/probe/analytics`.

### Resposta

1. Verifique inputs de build: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` no artefato do release (`build/release.json`).
2. Reexecute `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para o
   collector de staging para confirmar que o tracker ainda emite payloads.
3. Se os collectors estiverem down, defina `DOCS_ANALYTICS_ENDPOINT=""` e rebuild
   para que o tracker faca short-circuit; registre a janela de outage na linha
   do tempo do incidente.
4. Valide que `scripts/check-links.mjs` ainda faz fingerprint de `checksums.sha256`
   (quedas de analytics *nao* devem bloquear a validacao do sitemap).
5. Quando o collector voltar, rode `npm run test:widgets` para exercitar os unit tests
   do helper de analytics antes de republish.

### Pos-incidente

1. Atualize [`devportal/observability`](./observability) com novas limitacoes do collector
   ou requisitos de amostragem.
2. Abra um aviso de governanca se dados de analytics foram perdidos ou redigidos fora
   da politica.

## Drills trimestrais de resiliencia

Execute os dois drills durante a **primeira terca-feira de cada trimestre** (Jan/Abr/Jul/Out)
ou imediatamente apos qualquer mudanca maior de infraestrutura. Armazene artefatos em
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Drill | Passos | Evidencia |
| ----- | ----- | -------- |
| Ensaio de rollback de alias | 1. Repetir o rollback de "Deploy falho" usando o manifest de producao mais recente.<br/>2. Re-vincular a producao assim que os probes passarem.<br/>3. Registrar `portal.manifest.submit.summary.json` e logs de probes na pasta do drill. | `rollback.submit.json`, saida de probes, e release tag do ensaio. |
| Auditoria de validacao sintetica | 1. Rodar `npm run probe:portal` e `npm run probe:tryit-proxy` contra producao e staging.<br/>2. Rodar `npm run check:links` e arquivar `build/link-report.json`.<br/>3. Anexar screenshots/exports de paineis Grafana confirmando o sucesso dos probes. | Logs de probes + `link-report.json` referenciando o fingerprint do manifest. |

Escalone drills perdidos para o manager de Docs/DevRel e a revisao de governanca de SRE,
pois o roadmap exige evidencia trimestral determinista de que o rollback de alias e os
probes do portal continuam saudaveis.

## Coordenacao PagerDuty e on-call

- O servico PagerDuty **Docs Portal Publishing** e dono dos alertas gerados a partir de
  `dashboards/grafana/docs_portal.json`. As regras `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` paginam o primary de Docs/DevRel
  com Storage SRE como secundario.
- Quando a pagina tocar, inclua o `DOCS_RELEASE_TAG`, anexe screenshots dos paineis Grafana
  afetados e linke a saida de probe/link-check nas notas do incidente antes de iniciar
  a mitigacao.
- Depois da mitigacao (rollback ou redeploy), reexecute `npm run probe:portal`,
  `npm run check:links`, e capture snapshots Grafana recentes mostrando as metricas
  de volta aos thresholds. Anexe toda a evidencia ao incidente do PagerDuty
  antes de resolver.
- Se dois alertas dispararem ao mesmo tempo (por exemplo expiracao de TLS mais backlog),
  triage refusals primeiro (parar o publishing), execute o procedimento de rollback e
  depois resolva TLS/backlog com Storage SRE na ponte.
