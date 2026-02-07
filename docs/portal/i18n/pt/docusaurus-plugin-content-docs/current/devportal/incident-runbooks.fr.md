---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes e exercícios de reversão

## Objetivo

O item de roteiro **DOCS-9** exige playbooks acionáveis, além de um plano de repetição para que
os operadores do portal podem recuperar as contas de entrega sem deviner. Esta nota
Couvre trois incidentes um sinal forte - taxas de implantação, degradação de replicação et
painéis de análise - e documente os exercícios trimestrais que provam que a reversão do alias
e a validação sintética funciona todos os dias de luta em luta.

### Material conectado

- [`devportal/deploy-guide`](./deploy-guide) - fluxo de trabalho de embalagem, assinatura e promoção de alias.
- [`devportal/observability`](./observability) - tags de lançamento, análises e referências de sondas ci-dessous.
-`docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetrie du registre et seusils d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` e ajudantes `npm run probe:*`
  referências nas listas de verificação.

### Partilhas de telemetria e ferramentas

| Sinal / Util | Objetivo |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | Detecte bloqueios de replicação e violações de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifique a profundidade do backlog e a latência de conclusão da triagem. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Monte os echecs cote gateway que apresentam uma taxa de implantação. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Testes sintéticos que controlam os lançamentos e validam as reversões. |
| `npm run check:links` | Casos de gate de liens; utilizar apres chaque mitigação. |
| `sorafs_cli manifest submit ... --alias-*` (embrulhado par `scripts/sorafs-pin-release.sh`) | Mecanismo de promoção/reversão de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agregar recusas/alias/TLS/replicação de telemetria. Os alertas do PagerDuty referem-se a esses painéis como antes. |

## Runbook - Taxa de implantação ou artefato defeituoso

### Condições de encerramento

- Eco de visualização/produção das sondas (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana sobre `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` após o lançamento.
- QA manuel remarque des rotas cassees ou des pannes du proxy Experimente imediatamente depois
  a promoção do alias.

### Confinamento imediato

1. **Resolva as implantações:** marque o pipeline CI com `DEPLOY_FREEZE=1` (entrada do fluxo de trabalho
   GitHub) ou suspenda o trabalho do Jenkins para que nenhum artefato seja parte.
2. **Capture os artefatos:** telecarregador `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e a triagem das sondas du build en echec depois que
   a referência de reversão aos resumos exatos.
3. **Notificador das partes prenantes:** storage SRE, lead Docs/DevRel, et l'officier de garde
   governança para conscientização (surtout si `docs.sora` est impacte).

### Procedimento de reversão

1. Identificador do último manifesto em bom estado (LKG). O fluxo de trabalho de produção em estoque
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Consulte o apelido deste manifesto com o ajudante de envio:

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

3. Registre o resumo da reversão no ticket de incidente com os resumos do
   manifest LKG e du manifest en echec.

### Validação

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (veja o guia de implantação) para confirmar que o manifesto repromu corresponde
   sempre no arquivo CAR.
4. `npm run probe:tryit-proxy` para garantir que o proxy Try-It staging seja arrecadado.

### Pós-incidente

1. Reative o pipeline de implantação somente depois de incluir a causa.
2. Insira as entradas "Lições aprendidas" em [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se necessário.
3. Descobrir defeitos para o conjunto de testes em echec (sonda, verificador de link, etc.).

## Runbook - Degradação de replicação

### Condições de encerramento

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` pendente 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` pendente 10 minutos (ver
  `pin-registry-ops.md`).
- Governança sinaliza a disponibilidade de um alias depois de um lançamento.

### Triagem

1. Inspecione os painéis [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirme se o backlog está localizado em uma classe de estoque ou em uma frota de fornecedores.
2. Cruze os logs Torii para os avisos `sorafs_registry::submit_manifest` até
   determina se os envios são ecoados.
3. Echantillonner la sante des replicas via `sorafs_cli manifest status --manifest ...`
   (liste os resultados por provedor).

### Mitigação1. Reemita o manifesto com um nome de réplicas mais onze (`--pin-min-replicas 7`) via
   `scripts/sorafs-pin-release.sh` para que o agendador etale a carga no plus dos provedores.
   Registre o novo resumo no registro de incidentes.
2. Se o backlog for um provedor exclusivo, ele será desativado temporariamente por meio do agendador
   de replicação (documento em `pin-registry-ops.md`) e adiciona um novo manifesto
   Forcant les outros fornecedores para rafraichir l'alias.
3. Quando o fraicheur do apelido é mais crítica que a parte de replicação, re-lier
   l'alias a un manifest chaud deja stage (`docs-preview`), depois publica um manifesto de
   Eu vi uma vez que o SRE eliminou o backlog.

### Recuperação e coagulação

1. Vigilante `torii_sorafs_replication_sla_total{outcome="missed"}` para garantir que ele
   Compteur se estabiliza.
2. Capturar a saída `sorafs_cli manifest status` como antes de cada réplica estar de
   retornar em conformidade.
3. Abra ou envie a cada dia o post-mortem do backlog de replicação com as próximas cadeias
   etapes (provedores de escalonamento, ajuste do chunker, etc.).

## Runbook - Painel de análise ou telemetria

### Condições de encerramento

- `npm run probe:portal` reativa mais os painéis que cessam de consumir eventos
  `AnalyticsTracker` pendente >15 minutos.
- A revisão de privacidade detecta um grande descaso com eventos abandonados.
- `npm run probe:tryit-proxy` ecoa nos caminhos `/probe/analytics`.

### Resposta

1. Verificando as entradas de build: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` no artefato de lançamento (`build/release.json`).
2. Re-executor `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontador ver le
   coletor de teste para confirmar que o rastreador emite mais cargas úteis.
3. Se os coletores forem desativados, defina `DOCS_ANALYTICS_ENDPOINT=""` e reconstrua para
   que le tracker tribunal-circuito; consignar a janela de interrupção na linha do tempo.
4. Validador que `scripts/check-links.mjs` continua a impressão digital `checksums.sha256`
   (os painéis de análise não devem *passar* bloquear a validação do mapa do site).
5. Uma vez o coletor retoli, lancer `npm run test:widgets` para executar os
   testes unitários de análises auxiliares antes de republicar.

### Pós-incidente

1. Mettre a jour [`devportal/observability`](./observability) com novos limites
   du coletor ou as exigências de amostragem.
2. Emettre une warning governança si des donnees analytics ont ete perdues ou redigidos
   fora da política.

## Exercícios trimestrais de resiliência

Lancer les deux drills durante le **premier mardi de chaque trimestre** (janeiro/abr/julho/outubro)
ou imediatamente após todas as mudanças maiores de infraestrutura. Armazene os artefatos sous
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Broca | Étapes | Prévia |
| ----- | ----- | -------- |
| Repetição da reversão de alias | 1. Atualize a reversão "Taxa de implantação" com o manifesto de produção mais recente.<br/>2. Repita a produção uma vez que as sondas passaram.<br/>3. Registre `portal.manifest.submit.summary.json` e os registros das sondas no dossiê da broca. | `rollback.submit.json`, sortie des probes, et release tag da repetição. |
| Auditoria de validação sintética | 1. Lancer `npm run probe:portal` e `npm run probe:tryit-proxy` contra produção e preparação.<br/>2. Lancer `npm run check:links` e arquivador `build/link-report.json`.<br/>3. Junte capturas de tela/exportações de painéis Grafana confirmando o sucesso das sondas. | Logs de probes + `link-report.json` referem-se à impressão digital do manifesto. |

Escalader les drills manques au manager Docs/DevRel et a la revue governança SRE, car le
roadmap exige uma prévia trimestral para determinar que a reversão do alias e das sondas
portal santos descansados.

## Coordenação PagerDuty e plantão

- O serviço PagerDuty **Docs Portal Publishing** possui alertas gerados a partir de então
  `dashboards/grafana/docs_portal.json`. As regras `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, e `DocsPortal/TLSExpiry` página principal Docs/DevRel
  com armazenamento SRE e secundário.
- Quando estiver na página, inclua o `DOCS_RELEASE_TAG`, junte as capturas de tela dos painéis
  Grafana impacta e desliga a sonda/verificação de link nas notas do incidente antes
  de começar a mitigação.
- Apres mitigação (reversão ou reimplantação), reexecutor `npm run probe:portal`,
  `npm run check:links`, e o capturador de instantâneos Grafana monta as métricas
  receitas em seus países. Junte-se a todas as evidências do incidente PagerDuty
  resolução avançada.
- Se dois alertas forem desativados em tempo de meme (por exemplo, expiração de TLS mais backlog), teste
  recusas em primeiro lugar (arreter la publicação), executar o procedimento de reversão, depois
  trair TLS/backlog com Storage SRE na ponte.