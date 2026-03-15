---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflete `docs/source/da/replication_policy.md`. Gardez les deux versões em
:::

# Política de replicação de disponibilidade de dados (DA-4)

_Estatuto: En cours -- Responsáveis: Core Protocol WG / Storage Team / SRE_

O pipeline de ingestão DA aplica manutenção aos objetivos de retenção
deterministas para cada classe de blob descrita em `roadmap.md` (workstream
DA-4). Torii recusa a persistência de envelopes de retenção fornecidos par le
chamador qui ne correspondente pas a la politique configuradoe, garantissant que
cada novo validador/estoque retient le nombre requis d'epoques et de
réplicas sem dependência com a intenção do emissor.

## Política por padrão

| Classe de blob | Retenção quente | Retenção fria | Requisitos de réplicas | Classe de estoque | Tag de governança |
|---------------|---------------|----------------|-------------------|--------------------|--------------------|
| `taikai_segment` | 24 horas | 14 horas | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 horas | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 horas | 3 | `cold` | `da.governance` |
| _Default (todas as outras classes)_ | 6 horas | 30 horas | 3 | `warm` | `da.default` |

Esses valores são integrados em `torii.da_ingest.replication_policy` e apliques
para todos os envios `/v2/da/ingest`. Torii escreve os manifestos com o arquivo
perfil de retenção impõe e emet un avertissement quand les callers fornissent
valores incoerentes porque os operadores detectam SDKs obsoletos.

### Aulas de disponibilidade Taikai

Os manifestos de rota Taikai (`taikai.trm`) declaram um `availability_class`
(`hot`, `warm`, ou `cold`). Torii aplique a política correspondente antes de le
chunking para que os operadores possam ajustar as contas de réplicas por par
stream sem editor da tabela global. Padrões:

| Classe de disponibilidade | Retenção quente | Retenção fria | Requisitos de réplicas | Classe de estoque | Tag de governança |
|-------------------------|---------------|----------------|-------------------|--------------------|--------------------|
| `hot` | 24 horas | 14 horas | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 horas | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 horas | 3 | `cold` | `da.taikai.archive` |

As dicas foram enviadas para `hot` para que as transmissões ao vivo fossem retidas para
política la plus forte. Substitua os padrões por meio de
`torii.da_ingest.replication_policy.taikai_availability` se seu recurso for utilizado
des cibles diferentes.

## Configuração

A política é baseada em `torii.da_ingest.replication_policy` e expõe um modelo
*padrão* mais um quadro de substituições por classe. Os identificadores de classe são
sem distinção entre maiúsculas e minúsculas e aceitando `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para extensões aprovadas por la
governança. As classes de armazenamento aceitam `hot`, `warm`, ou `cold`.

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

Deixe o bloco intacto para utilizar os padrões ci-dessus. Despeje durcir une
classe, mettez a jour l'override correspondente; pour changer la base pour de
novas aulas, edite `default_retention`.

As aulas de Taikai disponíveis podem ter sobretaxas independentes via
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Semântica de aplicação

- Torii substitui o `RetentionPolicy` fornecido pelo usuário do perfil
  impor antes do chunking ou da emissão do manifesto.
- Les manifestos pré-construídos que declaram um perfil de retenção diferente
  são rejeitados com `400 schema mismatch` para que os clientes sejam obsoletos
  pode não fechar o contrato.
- Cada evento de substituição está registrado (`blob_class`, política soumise vs atendimento)
  para mostrar e evidenciar os chamadores não conformes durante a implementação.

Ver [Plano de ingestão de disponibilidade de dados](ingest-plan.md) (Lista de verificação de validação) para
le gate mis a jour couvrant l'enforcement detention.

## Fluxo de trabalho de nova replicação (suivi DA-4)

A aplicação da retenção não é a primeira etapa. Os operadores não devem
também comprovamos que os manifestos estão ativos e as ordens de replicação permanecem
alinhados com a política configurada para SoraFS podem replicar novamente os blobs
hors conforme automaticamente.1. **Vigie o desvio.** Torii emet
   `overriding DA retention policy to match configured network baseline` quando
   um chamador soumet des valores de retenção obsoletos. Associe este registro com
   telemetria `torii_sorafs_replication_*` para reproduzir deficiências de réplicas
   ou de redistribuições retardadas.
2. **Diferença de intenção versus réplicas ao vivo.** Utilize o novo auxiliar de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   O comando charge `torii.da_ingest.replication_policy` depois da configuração
   quatro, decodificar cada manifesto (JSON ou Norito) e associar opções
   as cargas úteis `ReplicationOrderV1` do resumo do manifesto. O sinal de currículo
   duas condições:

   - `policy_mismatch` - o perfil de retenção do manifesto diverge do perfil
     impor (ceci ne devrait jamais chegou sauf si Torii está mal configurado).
   - `replica_shortfall` - a ordem de replicação ao vivo exige menos réplicas
     que `RetentionPolicy.required_replicas` ou quatro atribuições que
     sa cível.

   Um status de surtida diferente de zero indica uma insuficiência ativa depois
   l'automatization CI/on-call puisse pager imediato. Faça o relacionamento JSON
   no pacote `docs/examples/da_manifest_review_template.md` para os votos do
   Parlamento.
3. **Declenchez la re-replication.** Quando a auditoria sinaliza uma insuficiência,
   emita um novo `ReplicationOrderV1` por meio de ferramentas de governo decrits
   em [SoraFS mercado de capacidade de armazenamento](../sorafs/storage-capacity-marketplace.md)
   e relacione a auditoria apenas com a convergência do conjunto de réplicas. Para as substituições
   de urgência, associe a surtida CLI com `iroha app da prove-availability` para que
   Os SREs podem fazer referência ao resumo do meme e ao PDP anterior.

A cobertura de regressão vit em `integration_tests/tests/da/replication_policy.rs`;
la suite recebeu uma política de retenção não conforme `/v2/da/ingest` e verificou
que o manifesto se recupere, exponha o perfil, imponha tanto quanto a intenção do chamador.