---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: "Plano de rollout de adverts de providers SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plano de rollout de adverts de providers SoraFS

Este plano coordena o cut-over de adverts permissivos de providers para a
superficie totalmente governada `ProviderAdvertV1` exigida para retrieval
multi-source de chunks. Ele foca em tres deliverables:

- **Guia de operadores.** Passos que storage providers precisam concluir antes de cada gate.
- **Cobertura de telemetria.** Dashboards e alerts que Observability e Ops usam
  para confirmar que a rede aceita apenas adverts conformes.
  para que equipes de SDK e tooling planejem releases.

O rollout se alinha aos marcos SF-2b/2c no
[roadmap de migracao SoraFS](./migration-roadmap) e assume que a policy de admissao
no [provider admission policy](./provider-admission-policy) ja esta ativa.

## Timeline de fases

| Fase | Janela (alvo) | Comportamento | Acoes do operador | Foco de observabilidade |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist do operador

1. **Inventariar adverts.** Liste cada advert publicado e registre:
   - Caminho do governing envelope (`defaults/nexus/sorafs_admission/...` ou equivalente em production).
   - `profile_id` e `profile_aliases` do advert.
   - Lista de capabilities (espera-se pelo menos `torii_gateway` e `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (necessaria quando TLVs vendor-reserved estiverem presentes).
2. **Regenerar com tooling do provider.**
   - Reconstrua o payload com seu publisher de provider advert, garantindo:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com `max_span` definido
     - `allow_unknown_capabilities=<true|false>` quando houver TLVs GREASE
   - Valide via `/v1/sorafs/providers` e `sorafs_fetch`; warnings sobre capabilities
     desconhecidas devem ser triageadas.
3. **Validar readiness multi-source.**
   - Execute `sorafs_fetch` com `--provider-advert=<path>`; o CLI agora falha quando
     `chunk_range_fetch` esta ausente e mostra warnings para capabilities desconhecidas
     ignoradas. Capture o report JSON e arquive com logs de operacoes.
4. **Preparar renovacoes.**
   - Envie envelopes `ProviderAdmissionRenewalV1` pelo menos 30 dias antes do
     enforcement no gateway (R2). Renovacoes devem manter o handle canonico e o
     set de capabilities; apenas stake, endpoints ou metadata devem mudar.
5. **Comunicar equipes dependentes.**
   - Donos de SDK devem lancar versoes que mostrem warnings aos operadores quando
     adverts forem rejeitados.
   - DevRel anuncia cada transicao de fase; inclua links de dashboards e a logica
     de thresholds abaixo.
6. **Instalar dashboards e alerts.**
   - Importe o Grafana export e coloque sob **SoraFS / Provider Rollout** com UID
     `sorafs-provider-admission`.
   - Garanta que as regras de alert apontem para o canal compartilhado
     `sorafs-advert-rollout` em staging e production.

## Telemetria e dashboards

As seguintes metricas ja estao expostas via `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` - conta aceitos, rejeitados e
  warnings. Os motivos incluem `missing_envelope`, `unknown_capability`, `stale`
  e `policy_violation`.

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe o arquivo no repositorio compartilhado de dashboards (`observability/dashboards`)
e atualize apenas o UID do datasource antes de publicar.

O board e publicado na pasta Grafana **SoraFS / Provider Rollout** com o UID
estavel `sorafs-provider-admission`. As regras de alert
`sorafs-admission-warn` (warning) e `sorafs-admission-reject` (critical) estao
preconfiguradas para usar a policy de notificacao `sorafs-advert-rollout`; ajuste
esse contact point se o destino mudar, em vez de editar o JSON do dashboard.

Painels Grafana recomendados:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart para visualizar accept vs warn vs reject. Alert quando warn > 0.05 * total (warning) ou reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Timeseries de linha unica que alimenta o threshold do pager (5% warning rate rolando 15 minutos). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia triage do runbook; anexe links para mitigacoes. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica providers que perderam o refresh deadline; cruze com logs do discovery cache. |

Artefacts de CLI para dashboards manuais:

- `sorafs_fetch --provider-metrics-out` escreve contadores `failures`, `successes`
  e `disabled` por provider. Importe em dashboards ad-hoc para monitorar dry-runs
  do orchestrator antes de trocar providers em production.
- Os campos `chunk_retry_rate` e `provider_failure_rate` do report JSON destacam
  throttling ou sintomas de payloads stale que costumam anteceder rejeicoes de admissao.

### Layout do dashboard Grafana

Observability publica um board dedicado - **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) - sob **SoraFS / Provider Rollout**
com os seguintes IDs canonicos de panel:

- Panel 1 - *Admission outcome rate* (stacked area, unidade "ops/min").
- Panel 2 - *Warning ratio* (single series), com a expressao
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 - *Rejection reasons* (time series agrupada por `reason`), ordenada por
  `rate(...[5m])`.
- Panel 4 - *Refresh debt* (stat), espelha a query da tabela acima e e anotada
  com refresh deadlines dos adverts extraidos do migration ledger.

Copie (ou crie) o JSON skeleton no repo de dashboards de infra em
`observability/dashboards/sorafs_provider_admission.json`, depois atualize apenas
o UID do datasource; os IDs de panel e regras de alert sao referenciados pelos
runbooks abaixo, entao evite renumerar sem revisar esta documentacao.

Por conveniencia, o repo ja inclui uma definicao de dashboard de referencia em
`docs/source/grafana_sorafs_admission.json`; copie para sua pasta Grafana se
precisar de um ponto de partida para testes locais.

### Regras de alerta Prometheus

Adicione o seguinte grupo de regras em
`observability/prometheus/sorafs_admission.rules.yml` (crie o arquivo se este for
o primeiro grupo de regras SoraFS) e inclua-o na configuracao do Prometheus.
Substitua `<pagerduty>` pelo label de roteamento real da sua rotacao on-call.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Execute `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de enviar mudancas para garantir que a sintaxe passe `promtool check rules`.

## Matriz de rollout

| Caracteristicas do advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, aliases canonicos, `signature_strict=true` | OK | OK | OK | OK |
| Ausencia de `chunk_range_fetch` capability | WARN (ingest + telemetry) | WARN | REJECT (`reason="missing_capability"`) | REJECT |
| TLVs de capability desconhecida sem `allow_unknown_capabilities=true` | OK | WARN (`reason="unknown_capability"`) | REJECT | REJECT |
| `refresh_deadline` expirado | REJECT | REJECT | REJECT | REJECT |
| `signature_strict=false` (diagnostic fixtures) | OK (development apenas) | WARN | WARN | REJECT |

Todos os horarios usam UTC. Datas de enforcement sao refletidas no migration
ledger e nao mudam sem voto do council; qualquer mudanca requer atualizar este
arquivo e o ledger no mesmo PR.

> **Nota de implementacao:** R1 introduz a serie `result="warn"` em
> `torii_sorafs_admission_total`. O patch de ingestao do Torii que adiciona o
> novo label e acompanhado junto das tarefas de telemetria SF-2; ate la, use

## Comunicacao e tratamento de incidentes

- **Weekly status mailer.** DevRel compartilha um resumo de admission metrics,
  warnings pendentes e deadlines proximas.
- **Incident response.** Se alerts `reject` dispararem, on-call engineers:
  1. Buscam o advert ofensivo via Torii discovery (`/v1/sorafs/providers`).
  2. Reexecutam a validacao do advert no pipeline do provider e comparam com
     `/v1/sorafs/providers` para reproduzir o erro.
  3. Coordenam com o provider a rotacao do advert antes do proximo refresh deadline.
- **Change freezes.** Nenhuma mudanca no schema de capabilities durante R1/R2 a
  menos que o comite de rollout aprove; trials GREASE devem ser agendados na
  janela semanal de manutencao e registrados no migration ledger.

## Referencias

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
