---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes e drills de rollback

## Propósito

O item do roadmap **DOCS-9** exige playbooks acionaveis mais um plano de ensaio para que
Os operadores do portal conseguem recuperar falhas de envio sem adivinhação. Esta nota cobre três
incidentes de alto sinal - incidentes com falhas, degradação de replicação e quedas de analítica - e
documenta os exercícios trimestrais que provam que o rollback de alias e a validação sintética
continue funcionando de ponta a ponta.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - workflow de empacotamento, assinatura e promoção de alias.
- [`devportal/observability`](./observability) - libera tags, análises e sondagens referenciadas abaixo.
-`docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria do registro e limites de escalonamento.
- `docs/portal/scripts/sorafs-pin-release.sh` e ajudantes `npm run probe:*`
  referenciados nos checklists.

### Telemetria e ferramentas compartilhadas

| Sinal / Ferramenta | Proposta |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | Detecta bloqueios de replicação e violações de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifica a profundidade do backlog e a latência de conclusão para triagem. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Mostra falhas do gateway que frequentemente seguem um deploy ruim. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas que gateiam releases e validam rollbacks. |
| `npm run check:links` | Portão de links quebrados; usado após cada mitigação. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promoção/reversão de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de recusas/alias/TLS/replicação. Alertas do PagerDuty referenciam estes dolorosos como evidências. |

## Runbook - Deploy falho ou artefatos ruins

### Condições de disparo

- Testes de visualização/falha de produção (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` após um lançamento.
- QA manual nota rotas quebradas ou falhas do proxy Experimente imediatamente após
  uma promoção do alias.

### Contenção Imediata

1. **Congelar implants:** marcar o pipeline CI com `DEPLOY_FREEZE=1` (input do workflow
   GitHub) ou pausar o trabalho Jenkins para que nenhum objeto seja enviado.
2. **Capturar artistas:** baixar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e a saida dos probes do build com falha para que
   o rollback referencie os resumos exatos.
3. **Notificar as partes interessadas:** SRE de armazenamento, líder Docs/DevRel e o oficial de serviço de
   governança para conscientização (especialmente quando `docs.sora` é impactado).

### Procedimento de reversão

1. Identifique o manifesto do último bem conhecido (LKG). O fluxo de trabalho de produção dos armazenados em
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincule o alias a esse manifesto com o ajudante de envio:

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

### Validação

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (veja o guia de implantação) para confirmar que o manifesto repromovido continua
   batendo com o CAR arquivado.
4. `npm run probe:tryit-proxy` para garantir que o proxy Try-It staging voltou.

### Pós-incidente

1. Reative o pipeline de implantação apenas depois de entender a causa raiz.
2. Preencha as entradas "Lições aprendidas" em [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se houver.
3. Abra defeitos para um conjunto de testes com falha (sonda, verificador de link, etc.).

## Runbook - Degradação de replicação

### Condições de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (veja
  `pin-registry-ops.md`).
- Governança reporta alias lenta após um lançamento.

### Triagem

1. Inspecione painéis de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirme se o backlog está localizado em uma classe de armazenamento ou em uma frota de fornecedores.
2. Cruze logs do Torii por avisos `sorafs_registry::submit_manifest` para determinar se
   as submissões estão falhando.
3. Amostre a saude das replicas via `sorafs_cli manifest status --manifest ...` (lista
   resultados por provedor).

### Mitigação1. Reemita o manifesto com maior contagem de réplicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que o agendador distribua carga em um conjunto maior
   de provedores. Registre o novo resumo no log do incidente.
2. Se o backlog estiver preso a um provedor único, desabilite-o temporariamente via o
   agendador de replicação (documentado em `pin-registry-ops.md`) e envie um novo
   manifest forçando os outros provedores a atualizar o alias.
3. Quando a frescura do alias for mais crítica que a paridade de replicação, re-vincule o
   alias a um manifesto quente já em staging (`docs-preview`), depois publique um manifesto de
   envio quando o SRE limpar o backlog.

### Recuperação e fechamento

1. Monitore `torii_sorafs_replication_sla_total{outcome="missed"}` para garantir que o
   contador estabilizar.
2. Capture a saida `sorafs_cli manifest status` como evidência de que cada réplica voltou
   uma conformidade.
3. Abra ou atualize o post-mortem do backlog de replicação com próximos passos
   (escalonamento de provedores, ajuste do chunker, etc.).

## Runbook - Queda de análise ou telemetria

### Condições de disparo

- `npm run probe:portal` passa, mas dashboards param de gerenciar eventos do
  `AnalyticsTracker` por >15 minutos.
- Revisão de privacidade aponta um aumento inesperado de eventos descartados.
- `npm run probe:tryit-proxy` falha nos caminhos `/probe/analytics`.

### Resposta

1. Verifique as entradas de build: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` sem artistas do release (`build/release.json`).
2. Execute novamente `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para o
   coletor de teste para confirmar que o rastreador ainda emite payloads.
3. Se os coletores estiverem desativados, defina `DOCS_ANALYTICS_ENDPOINT=""` e reconstrua
   para que o rastreador faça curto-circuito; registre uma janela de interrupção na linha
   do tempo do incidente.
4. Valide que `scripts/check-links.mjs` ainda faz impressão digital de `checksums.sha256`
   (quedas de analytics *não* devem bloquear a validação do mapa do site).
5. Quando o coletor voltar, rode `npm run test:widgets` para praticar os testes unitários
   faça helper de analytics antes de republicar.

### Pós-incidente

1. Atualize [`devportal/observability`](./observability) com novas limitações do coletor
   ou requisitos de amostragem.
2. Abra um aviso de governança se dados de análise foram perdidos ou redigidos fora
   da política.

## Exercícios trimestrais de resiliência

Execute os dois treinos durante a **primeira terça-feira de cada trimestre** (Jan/Abr/Jul/Out)
ou imediatamente após qualquer mudança maior de infraestrutura. Armazene artistas em
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Broca | Passos | Evidência |
| ----- | ----- | -------- |
| Ensaio de rollback de alias | 1. Repita o rollback de "Deploy falho" usando o manifesto de produção mais recente.<br/>2. Re-vincular a produção assim que as sondas passamem.<br/>3. Registrador `portal.manifest.submit.summary.json` e logs de sondas na pasta do drill. | `rollback.submit.json`, saida de probes, e release tag do ensaio. |
| Auditoria de validação sintética | 1. Rodar `npm run probe:portal` e `npm run probe:tryit-proxy` contra produção e encenação.<br/>2. Rodar `npm run check:links` e arquivar `build/link-report.json`.<br/>3. Anexar screenshots/exports de paineis Grafana confirmando o sucesso dos probes. | Logs de probes + `link-report.json` referenciando a impressão digital do manifesto. |

Escalone perfura perdas para o gerente de Docs/DevRel e a revisão de governança de SRE,
pois o roadmap exige evidência trimestral determinista de que o rollback de alias e os
sondas do portal continuam saudaveis.

## Coordenação PagerDuty e on-call

- O serviço PagerDuty **Docs Portal Publishing** e dono dos alertas gerados a partir de
  `dashboards/grafana/docs_portal.json`. Como regras `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` páginas primárias de Docs/DevRel
  com Armazenamento SRE como secundário.
- Quando a página tocar, incluindo o `DOCS_RELEASE_TAG`, anexo screenshots dos paineis Grafana
  abordagens e link a saida de probe/link-check nas notas do incidente antes de iniciar
  uma mitigação.
- Depois da mitigação (reversão ou reimplantação), execute novamente `npm run probe:portal`,
  `npm run check:links`, e capture snapshots Grafana recentes mostrando as métricas
  de volta aos limites. Anexo toda a evidência ao incidente do PagerDuty
  antes de resolver.
- Se dois alertas dispararem ao mesmo tempo (por exemplo expiração de TLS mais backlog),
  triagem de recusas primeiro (parar o publicar), executar o procedimento de rollback e
  depois resolve TLS/backlog com Storage SRE na ponte.