---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Plano de remoção de anúncios de fornecedores SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plano de remoção de anúncios de provedores SoraFS

Este plano coordena a transferência de anúncios permitidos de fornecedores para lá
superfície governamental `ProviderAdvertV1` necessária para recuperação de múltiplas origens
de pedaços. Se concentra em três resultados:

- **Guia de operadores.** Ações passo a passo que os provedores de armazenamento devem
  completar antes de cada portão.
- **Cobertura de telemetria.** Painéis e alertas que a Observabilidade e as Operações usam
  para confirmar que la red solo aceita anúncios conformes.
  para que os equipamentos de SDK e ferramentas planejem seus lançamentos.

O lançamento é alinhado com os sucessos SF-2b/2c do
[roteiro de migração de SoraFS](./migration-roadmap) e presumir que a política de
admissão de [política de admissão do provedor](./provider-admission-policy) ya esta en
vigor.

## Cronograma de fases

| Fase | Ventana (objetivo) | Comportamento | Ações do operador | Foco de observação |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist dos operadores

1. **Inventariar anúncios.** Lista cada anúncio publicado e registrado:
   - Rota do envelope governamental (`defaults/nexus/sorafs_admission/...` ou equivalente em produção).
   - `profile_id` e `profile_aliases` do anúncio.
   - Lista de capacidades (se espera pelo menos `torii_gateway` e `chunk_range_fetch`).
   - Bandeira `allow_unknown_capabilities` (requerido quando há TLVs reservados por fornecedor).
2. **Regenerar com ferramentas de provedores.**
   - Reconstrua a carga útil com o anúncio do editor do provedor, garantindo:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com um `max_span` definido
     - `allow_unknown_capabilities=<true|false>` quando haya TLVs GREASE
   - Validação via `/v2/sorafs/providers` e `sorafs_fetch`; as advertências sobre
     capacidades desconhecidas devem ser triageadas.
3. **Validar prontidão multiorigem.**
   - Execução `sorafs_fetch` com `--provider-advert=<path>`; o CLI agora falha
     quando falta `chunk_range_fetch` e mostra advertências para capacidades
     desconocidas ignoradas. Captura o relatório JSON e arquiva os logs
     de operações.
4. **Preparar renovações.**
   - Envia envelopes `ProviderAdmissionRenewalV1` pelo menos 30 dias antes de
     aplicação no gateway (R2). As renovações devem conservar o cabo
     canonico e o conjunto de capacidades; aposta solo, endpoints ou metadados deben
     cambiar.
5. **Comunique-se com os equipamentos dependentes.**
   - Os proprietários do SDK devem liberar versões que expõem anúncios aos
     operadores quando os anúncios são rechazados.
   - DevRel anuncia cada transição de fase; incluir links a dashboards e la
     lógica de umbral de baixo.
6. **Instale painéis e alertas.**
   - Importe a exportação de Grafana e coloque-a abaixo **SoraFS / Provider
     Implementação** com o UID `sorafs-provider-admission`.
   - Certifique-se de que as regras de alertas apunten al canal compartido
     `sorafs-advert-rollout` em preparação e produção.

## Telemetria e painéisAs métricas seguintes foram expostas via `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — conta aceita, rechazada
  e resultados com anúncios. As razões incluem `missing_envelope`, `unknown_capability`,
  `stale` e `policy_violation`.

Exportar de Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe o arquivo no repositório compartilhado de dashboards (`observability/dashboards`)
e atualize apenas o UID da fonte de dados antes de publicar.

A tabela é publicada abaixo da pasta de Grafana **SoraFS / Provider Rollout** com
o UID estável `sorafs-provider-admission`. As regras de alertas
`sorafs-admission-warn` (aviso) e `sorafs-admission-reject` (crítico) estão
pré-configurados para usar a política de notificação `sorafs-advert-rollout`;
ajuste esse ponto de contato na lista de destinos em vez de editar o
JSON do painel.

Painéis Grafana recomendados:

| Painel | Consulta | Notas |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pilha para visualizar aceitar vs avisar vs rejeitar. Alerta quando avisar > 0,05 * total (aviso) ou rejeitar > 0 (crítico). |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporal de uma única linha que alimenta o umbral do pager (taxa de aviso de 5% rolando 15 minutos). |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia de triagem do runbook; adjunta enlaces a passos de mitigação. |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica provedores que não cumprem o prazo de atualização; cruza com logs de cache de descoberta. |

Artefatos da CLI para painéis manuais:

- `sorafs_fetch --provider-metrics-out` descrever contadores `failures`, `successes` y
  `disabled` por provedor. Importar painéis ad-hoc para monitorar
  testes do orquestrador antes de alterar os fornecedores na produção.
- Os campos `chunk_retry_rate` e `provider_failure_rate` do relatório JSON
  resaltan estrangulamento ou sintomas de cargas obsoletas que suelen preceder rechazos
  de admissão.

### Layout do painel de Grafana

Observabilidade publica un board dedicado — **SoraFS Provider Admission
Implementação** (`sorafs-provider-admission`) — bajo **SoraFS / Implementação do provedor**
com os seguintes IDs canônicos do painel:

- Painel 1 — *Taxa de resultados de admissão* (área empilhada, unidade "ops/min").
- Painel 2 — *Taxa de advertência* (série única), emitindo a expressão
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 — *Motivos de rejeição* (série de tempo agrupada por `reason`), ordenado por
  `rate(...[5m])`.
- Painel 4 — *Atualizar dívida* (stat), refletindo a consulta da tabela anterior e
  anotada com os prazos de atualização de anúncios extraidos do registro de migração.

Copie (ou crie) o esqueleto JSON no repositório de dashboards de infraestrutura em
`observability/dashboards/sorafs_provider_admission.json`, depois de atualizar apenas o el
UID da fonte de dados; os IDs do painel e as regras de alerta são referenciados em
os runbooks de baixo, para evitar renumerar-los sem revisar esta documentação.Para maior comodidade, o repositório inclui uma definição de painel de controle
referência em `docs/source/grafana_sorafs_admission.json`; copiala em sua pasta
Grafana requer um ponto de partida para verificar locais.

### Regras de alerta de Prometheus

Agregar o próximo grupo de regras a
`observability/prometheus/sorafs_admission.rules.yml` (crie o arquivo se este for
o primeiro grupo de regras SoraFS) e inclui sua configuração de
Prometheus. Substitua a placa `<pagerduty>` com a etiqueta de treinamento real para você
rotação de plantão.

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

Ejecuta `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de subir mudanças para garantir que a sintaxe passe `promtool check rules`.

## Matriz de lançamento

| Características do anúncio | R0 | R1 | R2 | R3 |
|-----|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, apelidos canônicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Cuidado com a capacidade `chunk_range_fetch` | ⚠️ Avisar (ingestão + telemetria) | ⚠️Avisar | ❌ Rejeitar (`reason="missing_capability"`) | ❌ Rejeitar |
| TLVs de capacidade desconhecida sin `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rejeitar | ❌ Rejeitar |
| `refresh_deadline` expirado | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar |
| `signature_strict=false` (dispositivos de diagnóstico) | ✅ (desarrollo solo) | ⚠️Avisar | ⚠️Avisar | ❌ Rejeitar |

Todos os horários usam UTC. As datas de aplicação são refletidas na migração
ledger e não se moveran sin un voto del conselho; qualquer mudança requer atualização
este arquivo e o livro-razão no mesmo PR.

> **Nota de implementação:** R1 apresenta a série `result="warn"` en
> `torii_sorafs_admission_total`. O patch de ingestão Torii que adiciona a nova
> etiqueta se segue junto com as tarefas de telemetria SF-2; até que saia,

## Comunicação e manejo de incidentes

- **Mailer semanal de estado.** DevRel circula um resumo breve de métricas de
  admissão, anúncios pendentes e prazos próximos.
- **Resposta a incidentes.** Se os alertas `reject` forem ativados, de plantão:
  1. Recupere o anúncio ofensivo via descoberta de Torii (`/v2/sorafs/providers`).
  2. Execute novamente a validação do anúncio no pipeline do fornecedor e compare com
     `/v2/sorafs/providers` para reproduzir o erro.
  3. Coordenar com o provedor a rotação do anúncio antes da próxima atualização
     prazo.
- **Congelamentos de mudanças.** Não se aplicam mudanças de esquema de capacidades
  durante R1/R2, a menos que o comitê de implementação seja aprovado; os ensaios GREASE deben
  programar durante a janela semanal de manutenção e registrar-se no
  livro de migração.

## Referências

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)