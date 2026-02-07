---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Plano de lançamento de anúncios de provedores SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plano de lançamento de anúncios de provedores SoraFS

Este plano coordena o corte de anúncios permitidos de provedores para um
superfície totalmente governada `ProviderAdvertV1` usada para recuperação
pedaços de múltiplas fontes. Ele foca em três resultados:

- **Guia de operadores.** Passos que os provedores de armazenamento precisam concluir antes de cada portão.
- **Cobertura de telemetria.** Dashboards e alertas que Observabilidade e Operações usam
  para confirmar que a rede aceita apenas anúncios conformes.
  para que equipes de SDK e ferramentas planejem lançamentos.

O rollout se alinha aos marcos SF-2b/2c no
[roteiro de migração SoraFS](./migration-roadmap) e assumir que uma política de admissão
não [política de admissão do provedor](./provider-admission-policy) já está ativado.

## Defasagens da linha do tempo

| Fase | Janela (alvo) | Comportamento | Aços do operador | Foco de observabilidade |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist do operador

1. **Inventariar anúncios.** Listar cada anúncio publicado e registrar:
   - Caminho do envelope governamental (`defaults/nexus/sorafs_admission/...` ou equivalente em produção).
   - `profile_id` e `profile_aliases` do anúncio.
   - Lista de capacidades (espera-se pelo menos `torii_gateway` e `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (necessário quando TLVs vendor-reserved estiverem presentes).
2. **Regenerar as ferramentas do provedor.**
   - Reconstrua o payload com seu editor de anúncio do provedor, garantindo:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com `max_span` definido
     - `allow_unknown_capabilities=<true|false>` quando houver TLVs GREASE
   - Validar via `/v1/sorafs/providers` e `sorafs_fetch`; avisos sobre capacidades
     pessoas desconhecidas devem ser triadas.
3. **Validar prontidão multifonte.**
   - Execute `sorafs_fetch` com `--provider-advert=<path>`; o CLI agora falha quando
     `chunk_range_fetch` está ausente e mostra avisos para capacidades desconhecidas
     ignorados. Capturar o relatório JSON e arquivar com logs de operações.
4. **Preparar renovações.**
   - Enviar envelopes `ProviderAdmissionRenewalV1` pelo menos 30 dias antes do
     aplicação sem gateway (R2). As renovações devem manter o tratamento canônico e o
     definir as capacidades; apenas stake, endpoints ou metadados devem mudar.
5. **Comunicar equipes dependentes.**
   - Donos de SDK devem lançar versoes que mostram avisos aos operadores quando
     anúncios foram rejeitados.
   - DevRel anuncia cada transição de fase; incluindo links de dashboards e lógica
     de limites abaixo.
6. **Instale painéis e alertas.**
   - Importe o Grafana export e coloque sob **SoraFS / Provider Rollout** com UID
     `sorafs-provider-admission`.
   - Garanta que as regras de alerta apontem para o canal compartilhado
     `sorafs-advert-rollout` em preparação e produção.

## Telemetria e dashboards

As seguintes métricas já foram expostas via `iroha_telemetry`:- `torii_sorafs_admission_total{result,reason}` - conta aceita, rejeitada e
  avisos. Os motivos `missing_envelope`, `unknown_capability`, `stale`
  e `policy_violation`.

Exportação Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe o arquivo no repositório compartilhado de dashboards (`observability/dashboards`)
e atualize apenas o UID da fonte de dados antes de publicar.

O board e publicado na pasta Grafana **SoraFS / Provider Rollout** com o UID
estavel `sorafs-provider-admission`. As regras de alerta
`sorafs-admission-warn` (aviso) e `sorafs-admission-reject` (crítico) estado
pré-configuradas para usar a política de notificação `sorafs-advert-rollout`; ajuste
esse contact point se o destino mudar, em vez de editar o JSON do dashboard.

Painéis Grafana Recomendados:

| Painel | Consulta | Notas |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pilha para visualizar aceitar vs avisar vs rejeitar. Alerta quando avisar > 0,05 * total (aviso) ou rejeitar > 0 (crítico). |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporal de linha única que alimenta o limite do pager (5% warning rate rolando 15 minutos). |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia triagem do runbook; anexo links para mitigações. |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica provedores que perderam o prazo de atualização; cruze com logs do cache de descoberta. |

Artefatos da CLI para dashboards manuais:

- `sorafs_fetch --provider-metrics-out` escreve contadores `failures`, `successes`
  e `disabled` por provedor. Importe em dashboards ad-hoc para monitorar simulações
  do orquestrador antes dos fornecedores de trocater em produção.
- Os campos `chunk_retry_rate` e `provider_failure_rate` do relatório JSON destacam
  estrangulamento ou sintomas de cargas obsoletas que costumam anteceder rejeições de admissão.

### Layout do painel Grafana

Observabilidade publica um board dedicado - **SoraFS Provider Admission
Lançamento** (`sorafs-provider-admission`) - sob **SoraFS / Lançamento do provedor**
com os seguintes IDs canônicos do painel:

- Painel 1 - *Taxa de resultado de admissão* (área empilhada, unidade "ops/min").
- Painel 2 - *Warning ratio* (série única), com a expressão
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 - *Motivos de rejeição* (série temporal agrupada por `reason`), ordenada por
  `rate(...[5m])`.
- Painel 4 - *Atualizar dívida* (stat), espelha a consulta da tabela acima e e anotada
  com atualizar prazos dos anúncios extraidos do registro de migração.

Copie (ou chore) o esqueleto JSON no repo de dashboards de infra em
`observability/dashboards/sorafs_provider_admission.json`, depois de atualizar apenas
o UID da fonte de dados; os IDs do painel e regras de alerta são referenciados pelos
runbooks abaixo, então evite renumerar sem verificar esta documentação.

Por conveniência, o repositório já inclui uma definição de painel de referência em
`docs/source/grafana_sorafs_admission.json`; copie para sua pasta Grafana se
precisar de um ponto de partida para testes locais.

### Regras de alerta PrometheusAdicione o seguinte grupo de regras em
`observability/prometheus/sorafs_admission.rules.yml` (crie o arquivo se este for
o primeiro grupo de regras SoraFS) e incluindo-o na configuração do Prometheus.
Substitua `<pagerduty>` pelo rótulo de roteamento real da sua rotação on-call.

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
antes de enviar mudanças para garantir que a sintaxe passe `promtool check rules`.

## Matriz de lançamento

| Características do anúncio | R0 | R1 | R2 | R3 |
|-----|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, apelidos canônicos, `signature_strict=true` | OK | OK | OK | OK |
| Ausência de capacidade `chunk_range_fetch` | AVISO (ingestão + telemetria) | AVISO | REJEITAR (`reason="missing_capability"`) | REJEITAR |
| TLVs de capacidade desconhecida sem `allow_unknown_capabilities=true` | OK | AVISO (`reason="unknown_capability"`) | REJEITAR | REJEITAR |
| `refresh_deadline` expirado | REJEITAR | REJEITAR | REJEITAR | REJEITAR |
| `signature_strict=false` (dispositivos de diagnóstico) | OK (desenvolvimento apenas) | AVISO | AVISO | REJEITAR |

Todos os horários usam UTC. Dados de aplicação são refletidos sem migração
ledger e não mudam sem voto do conselho; qualquer mudança requer atualizar este
arquivo e o razão no mesmo PR.

> **Nota de implementação:** R1 introduz a série `result="warn"` em
> `torii_sorafs_admission_total`. O patch de ingestão do Torii que adiciona o
> novo selo e acompanhado junto das tarefas de telemetria SF-2; comi la, use

## Comunicação e tratamento de incidentes

- **Mailer semanal de status.** DevRel compartilha um resumo de métricas de admissão,
  avisos pendentes e prazos próximos.
- **Resposta a incidentes.** Se os alertas `reject` dispararem, engenheiros de plantão:
  1. Procurem o anúncio ofensivo via descoberta Torii (`/v1/sorafs/providers`).
  2. Reexecutamos a validação do anúncio no pipeline do provedor e comparamos com
     `/v1/sorafs/providers` para reproduzir ou erro.
  3. Coordenar com o fornecedor a rotação do anúncio antes do próximo prazo de atualização.
- **Change freezes.** Nenhuma mudança no esquema de recursos durante R1/R2 a
  menos que o comitê de rollout aprove; ensaios GREASE devem ser agendados na
  janela semanal de manutenção e registradas no registro de migração.

## Referências

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)