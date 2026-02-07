---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes e exercícios de reversão

## Propósito

O item do roteiro **DOCS-9** exige playbooks acionáveis, mas um plano prático para que
Os operadores do portal podem recuperar falhas de entrega sem avisar. Esta nota
cubre tres incidentes de alta senal: despliegues fallidos, degradacion de replicacion y
caidas de análise, e documenta os exercícios trimestrais que verificam a reversão de
alias e a validação sintética continuam funcionando de ponta a ponta.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - fluxo de embalagem, assinatura e promoção de alias.
- [`devportal/observability`](./observability) - tags de lançamento, análise e sondas referenciadas abaixo.
-`docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria de registro e guarda-chuvas de escalada.
- `docs/portal/scripts/sorafs-pin-release.sh` e ajudantes `npm run probe:*`
  referenciados nas listas de verificação.

### Telemetria e ferramentas compartilhadas

| Senal / Ferramenta | Proposta |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | Detecta bloqueios de replicação e violações de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Calcula a profundidade do backlog e a latência de conclusão para a triagem. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Mostra falhas no gateway que o menu segue após uma implantação com defeito. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas que geram lançamentos e validam reversões. |
| `npm run check:links` | Gate de enlaces rotos; é usado após cada mitigação. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promoção/reversão de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de recusas/alias/TLS/replicação. Alertas de PagerDuty referenciam esses painéis como evidência. |

## Runbook - Despliegue falha ou artefato incorreto

### Condições de disparo

- Queda das sondas de visualização/produção (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana e `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` após uma implementação.
- Manual de controle de qualidade detecta rotas ou falhas de proxy Experimente imediatamente depois de la
  promoção do alias.

### Contenção imediata

1. **Congelar despliegues:** marque o pipeline CI com `DEPLOY_FREEZE=1` (entrada do fluxo de trabalho de
   GitHub) ou pausar o trabalho do Jenkins para que não haja mais artefatos.
2. **Capturar artefatos:** descarga `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e a saída das sondas da construção falhou para que
   a reversão faz referência aos resumos exatos.
3. **Notificar as partes interessadas:** storage SRE, lead de Docs/DevRel, e o oficial de guardia de
   governança para conscientização (especialmente quando `docs.sora` está impactado).

### Procedimento de reversão

1. Identifique o manifesto último conhecido bom (LKG). O fluxo de trabalho de produção da guarda
   baixo `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Revincule o alias deste manifesto com o ajudante de envio:

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

3. Registre o resumo da reversão no ticket do incidente junto com os resumos do
   manifesto LKG e del manifesto falido.

### Validação

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (ver o guia de despliegue) para confirmar que o manifesto repromocionado segue
   coincidindo com o CAR arquivado.
4. `npm run probe:tryit-proxy` para garantir que o proxy Try-It staging regresse.

### Pós-incidente

1. Reabilite o pipeline de despliegue somente depois de entender a causa raiz.
2. Rellena entradas de "Lições aprendidas" em [`devportal/deploy-guide`](./deploy-guide)
   com novas notas, se aplica.
3. Abre defeitos para o conjunto de testes falhos (sonda, verificador de link, etc.).

## Runbook - Degradação de replicação

### Condições de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (ver
  `pin-registry-ops.md`).
- Relatório governamental de disponibilidade lenta do alias após um lançamento.

### Triagem

1. Inspecione os painéis de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirmar se o backlog está localizado em uma classe de armazenamento ou em uma frota de fornecedores.
2. Cruze os logs de Torii para avisos `sorafs_registry::submit_manifest` para determinar se
   as submissões estão falhando.
3. Mostre a saúde das réplicas via `sorafs_cli manifest status --manifest ...` (lista de resultados
   de replicação por provedor).

### Mitigação1. Reemita o manifesto com maior conteúdo de réplicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que o agendador distribua carga em um conjunto maior
   de provedores. Registre o novo resumo no registro do incidente.
2. Se o backlog estiver vinculado a um provedor único, ele será desativado temporariamente por meio do
   agendador de replicação (documentado em `pin-registry-ops.md`) e envie um novo
   manifest forzando a los outros provedores para atualizar o alias.
3. Quando a atualização do alias é mais crítica do que a paridade de replicação, vincule novamente o
   alias a um manifesto caliente ya encenado (`docs-preview`), depois publica um manifesto de
   Seguir uma vez que o SRE limpou o backlog.

### Recuperação e fechamento

1. Monitore `torii_sorafs_replication_sla_total{outcome="missed"}` para garantir que o
   o conteúdo se estabiliza.
2. Capture a saída de `sorafs_cli manifest status` como evidência de que cada réplica esta
   de novo em cumprimento.
3. Abra ou atualize a autópsia do backlog de replicação com passos seguintes
   (escalado de provedores, ajuste do chunker, etc.).

## Runbook - Caida de análise ou telemetria

### Condições de disparo

- `npm run probe:portal` você sai, mas os painéis deixam de gerenciar eventos de
  `AnalyticsTracker` por >15 minutos.
- A análise de privacidade detecta um aumento inesperado em eventos descartados.
- `npm run probe:tryit-proxy` falha nos caminhos `/probe/analytics`.

### Resposta

1. Verifique as entradas de construção: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` no artefato de lançamento (`build/release.json`).
2. Execute novamente `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para
   coletor de teste para confirmar que o rastreador continua emitindo cargas úteis.
3. Se os coletores estiverem caídos, sete `DOCS_ANALYTICS_ENDPOINT=""` e reconstruir
   para que o rastreador tenha curto-circuito; registre a janela de interrupção no site
   linha de tempo do incidente.
4. Valida que `scripts/check-links.mjs` siga impressão digital `checksums.sha256`
   (as ações de análise *não* devem bloquear a validação do mapa do site).
5. Quando o coletor for recuperado, corrija `npm run test:widgets` para executar o
   testes unitários do auxiliar de análise antes de republicar.

### Pós-incidente

1. Atualizar [`devportal/observability`](./observability) com novas limitações do
   coletor o requisitos de museu.
2. Emitir aviso de governo se perder ou redigir dados de análise futura
   de política.

## Exercícios trimestrais de resiliência

Ejecuta ambos os treinos durante o **primer março de cada trimestre** (Ene/Abr/Jul/Out)
ou imediatamente após qualquer mudança maior de infraestrutura. Guarda artefatos bajo
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Broca | Passos | Evidência |
| ----- | ----- | -------- |
| Ensaio de reversão de alias | 1. Repita o rollback de "Despliegue fallido" usando o manifesto de produção mais recente.<br/>2. Re-vincular a produção uma vez que as sondas passam.<br/>3. Registrador `portal.manifest.submit.summary.json` e logs de sondas na pasta da furadeira. | `rollback.submit.json`, saída de sondas e tag de liberação do ensaio. |
| Auditoria de validação sintética | 1. Executar `npm run probe:portal` e `npm run probe:tryit-proxy` contra produção e preparação.<br/>2. Execute `npm run check:links` e arquive `build/link-report.json`.<br/>3. Adjuntar capturas de tela/exportações de painéis Grafana confirmando a saída da sonda. | Logs de probe + `link-report.json` referenciando a impressão digital do manifesto. |

Escalar os exercícios perdidos no gerenciador de Docs/DevRel e na revisão de governança do SRE,
sim, o roteiro exige evidência determinista trimestral de que a reversão do alias e os
sondas do portal continuam saludáveis.

## Coordenação de PagerDuty e plantão

- O serviço PagerDuty **Docs Portal Publishing** é devido aos alertas gerados desde
  `dashboards/grafana/docs_portal.json`. Regras `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` página principal do Docs/DevRel
  com armazenamento SRE como secundário.
- Quando esta página inclui o `DOCS_RELEASE_TAG`, junto com as capturas de tela dos painéis Grafana
  afetados e coloque a saída da sonda/verificação de link nas notas do incidente antes de
  iniciar mitigação.
- Após a mitigação (reversão ou reimplantação), reejecuta `npm run probe:portal`,
  `npm run check:links`, e captura de instantâneos Grafana afrescos mostrando as métricas
  de novo dentro dos umbrais. Juntando toda a evidência ao incidente do PagerDuty
  antes de resolver isso.
- Se os alertas dispararem ao mesmo tempo (por exemplo, expiração de TLS mas backlog), triagem
  recusas primeiro (detener publicação), ejecuta o procedimento de reversão, depois
  limpar itens de TLS/backlog com Storage SRE na ponte.