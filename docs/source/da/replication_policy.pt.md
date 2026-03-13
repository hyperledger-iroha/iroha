---
lang: pt
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Política de replicação de disponibilidade de dados (DA-4)

_Status: Em andamento — Proprietários: Core Protocol WG / Storage Team / SRE_

O pipeline de ingestão de DA agora impõe metas de retenção determinísticas para
cada classe de blob descrita em `roadmap.md` (fluxo de trabalho DA-4). Torii se recusa a
persistir envelopes de retenção fornecidos pelo chamador que não correspondem ao configurado
política, garantindo que cada nó validador/armazenamento retenha o necessário
número de épocas e réplicas sem depender da intenção do remetente.

## Política padrão

| Classe de blob | Retenção quente | Retenção de frio | Réplicas necessárias | Classe de armazenamento | Etiqueta de governança |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Padrão (todas as outras classes)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Esses valores estão incorporados em `torii.da_ingest.replication_policy` e aplicados a
todos os envios `/v2/da/ingest`. Torii reescreve manifestos com o imposto
perfil de retenção e emite um aviso quando os chamadores fornecem valores incompatíveis para
os operadores podem detectar SDKs obsoletos.

### Aulas de disponibilidade de Taikai

Os manifestos de roteamento Taikai (metadados `taikai.trm`) agora incluem um
Dica `availability_class` (`Hot`, `Warm` ou `Cold`). Quando presente, Torii
seleciona o perfil de retenção correspondente de `torii.da_ingest.replication_policy`
antes de dividir a carga útil, permitindo que os operadores de eventos façam downgrade
representações sem editar a tabela de política global. Os padrões são:

| Classe de disponibilidade | Retenção quente | Retenção de frio | Réplicas necessárias | Classe de armazenamento | Etiqueta de governança |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Se o manifesto omitir `availability_class`, o caminho de ingestão retornará ao
Perfil `hot` para que as transmissões ao vivo mantenham seu conjunto completo de réplicas. Os operadores podem
substitua esses valores editando o novo
Bloco `torii.da_ingest.replication_policy.taikai_availability` na configuração.

## Configuração

A política está sob `torii.da_ingest.replication_policy` e expõe um
Modelo *padrão* mais uma série de substituições por classe. Os identificadores de classe são
não diferencia maiúsculas de minúsculas e aceita `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` ou `custom:<u16>` para extensões aprovadas pela governança.
As classes de armazenamento aceitam `hot`, `warm` ou `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Deixe o bloco intacto para executar com os padrões listados acima. Para apertar um
class, atualize a substituição correspondente; para alterar a linha de base para novas classes,
edite `default_retention`.Para ajustar classes específicas de disponibilidade do Taikai, adicione entradas em
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Semântica de aplicação

- Torii substitui o `RetentionPolicy` fornecido pelo usuário pelo perfil imposto
  antes da fragmentação ou emissão de manifesto.
- Manifestos pré-criados que declaram um perfil de retenção incompatível são rejeitados
  com `400 schema mismatch` para que clientes obsoletos não possam enfraquecer o contrato.
- Cada evento de substituição é registrado (`blob_class`, política enviada versus política esperada)
  para revelar chamadores não conformes durante a implementação.

Consulte `docs/source/da/ingest_plan.md` (lista de verificação de validação) para obter o portão atualizado
abrangendo a aplicação da retenção.

## Fluxo de trabalho de nova replicação (acompanhamento DA-4)

A aplicação da retenção é apenas o primeiro passo. Os operadores também devem provar que
manifestos ao vivo e pedidos de replicação permanecem alinhados com a política configurada para que
que SoraFS pode replicar automaticamente blobs fora de conformidade.

1. **Observe o desvio.** Torii emite
   `overriding DA retention policy to match configured network baseline` sempre que
   um chamador envia valores de retenção obsoletos. Emparelhe esse registro com
   Telemetria `torii_sorafs_replication_*` para detectar deficiências ou atrasos na réplica
   reafectações.
2. **Intenção diferente versus réplicas ativas.** Use o novo auxiliar de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   O comando carrega `torii.da_ingest.replication_policy` do fornecido
   config, decodifica cada manifesto (JSON ou Norito) e, opcionalmente, corresponde a qualquer
   Cargas úteis `ReplicationOrderV1` por resumo do manifesto. O resumo sinaliza dois
   condições:

   - `policy_mismatch` – o perfil de retenção do manifesto diverge do imposto
     política (isso nunca deve acontecer, a menos que Torii esteja configurado incorretamente).
   - `replica_shortfall` – o pedido de replicação em tempo real solicita menos réplicas do que
     `RetentionPolicy.required_replicas` ou fornece menos atribuições do que seu
     alvo.

   Um status de saída diferente de zero indica um déficit ativo, portanto a automação de CI/de plantão
   pode paginar imediatamente. Anexe o relatório JSON ao
   Pacote `docs/examples/da_manifest_review_template.md` para votações no Parlamento.
3. **Acione a nova replicação.** Quando a auditoria relatar uma deficiência, emita uma nova
   `ReplicationOrderV1` por meio das ferramentas de governança descritas em
   `docs/source/sorafs/storage_capacity_marketplace.md` e execute novamente a auditoria
   até que o conjunto de réplicas convirja. Para substituições de emergência, emparelhe a saída CLI
   com `iroha app da prove-availability` para que os SREs possam referenciar o mesmo resumo
   e evidências de PDP.

A cobertura de regressão reside em `integration_tests/tests/da/replication_policy.rs`;
o pacote envia uma política de retenção incompatível para `/v2/da/ingest` e verifica
que o manifesto obtido expõe o perfil imposto em vez do chamador
intenção.

## Telemetria e painéis de prova de integridade (ponte DA-5)

O item do roteiro **DA-5** exige que os resultados da aplicação do PDP/PoTR sejam auditáveis em
tempo real. Os eventos `SorafsProofHealthAlert` agora acionam um conjunto dedicado de
Métricas Prometheus:

-`torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
-`torii_sorafs_proof_health_pdp_failures{provider_id}`
-`torii_sorafs_proof_health_potr_breaches{provider_id}`
-`torii_sorafs_proof_health_penalty_nano{provider_id}`
-`torii_sorafs_proof_health_cooldown{provider_id}`
-`torii_sorafs_proof_health_window_end_epoch{provider_id}`

A placa **SoraFS PDP e PoTR Health** Grafana
(`dashboards/grafana/sorafs_pdp_potr_health.json`) agora expõe esses sinais:- *Prova de alertas de saúde por gatilho* gráficos de taxas de alerta por gatilho/sinalizador de penalidade, então
  Os operadores de Taikai/CDN podem provar se os ataques somente PDP, somente PoTR ou duplos são
  disparando.
- *Provedores em Tempo de Recarga* informa a soma ao vivo dos provedores atualmente sob um
  Tempo de espera do SorafsProofHealthAlert.
- *Instantâneo da janela de prova de integridade* mescla os contadores PDP/PoTR, valor da penalidade,
  sinalizador de resfriamento e época final da janela de ataque por provedor para que os revisores de governança
  pode anexar a tabela aos pacotes de incidentes.

Os runbooks devem vincular esses painéis ao apresentar evidências de aplicação da DA; eles
vincular as falhas do fluxo de prova da CLI diretamente aos metadados de penalidade na cadeia e
forneça o gancho de observabilidade indicado no roteiro.