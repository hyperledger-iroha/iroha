---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Espelha `docs/source/da/replication_policy.md`. Mantenha as duas versoes em
:::

# Política de replicação de Disponibilidade de Dados (DA-4)

_Status: Em progresso -- Responsáveis: Core Protocol WG / Storage Team / SRE_

O pipeline de ingestão DA agora aplica metas determinísticas de retenção para cada
classe de blob descrita em `roadmap.md` (fluxo de trabalho DA-4). Torii recusa persistir
envelopes de retenção fornecidos pelo chamador que não cobre a política
configuração, garantindo que cada nó validador/armazenamento retenha o número
requer de épocas e réplicas sem depender da intenção do emissor.

## Política padrão

| Classe de blob | Retenção quente | Retenção de frio | Réplicas exigidas | Classe de armazenamento | Tag de governança |
|---------------|----------|---------------|---------------------|----------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Padrão (todas as demais classes)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Esses valores são embutidos em `torii.da_ingest.replication_policy` e aplicados
a todas as submissões `/v1/da/ingest`. Torii reescreve manifestos com o perfil
de retenção imposto e emite um aviso quando os chamadores fornecem valores divergentes
para que os operadores detectem SDKs desatualizados.

### Classes de disponibilidade Taikai

Manifestos de roteamento Taikai (`taikai.trm`) declaram `availability_class`
(`hot`, `warm`, ou `cold`). Torii aplica a política correspondente antes de fazer
chunking para que os operadores possam escalar contagens de réplicas por stream sem
editar a tabela global. Padrões:

| Classe de disponibilidade | Retenção quente | Retenção de frio | Réplicas exigidas | Classe de armazenamento | Tag de governança |
|---------------------------|--------------|---------------|---------------------|-----------------------------------|-------------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Hints ausentes usam `hot` por padrão para que transmissões ao vivo retenham a
política mais forte. Substitua os padrões via
`torii.da_ingest.replication_policy.taikai_availability` se sua rede usar
alvos diferentes.

## Configuração

A política vive sob `torii.da_ingest.replication_policy` e expoe um template
*default* mais um array de overrides por classe. Identificadores de classe nao
diferenciam maiúsculas/minúsculas e aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para extensões aprovadas por governança.
Classes de armazenamento aceitam `hot`, `warm`, ou `cold`.

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

Deixe o bloco intacto para rodar com os padrões acima. Para suportar uma
classe, atualizar ou substituir correspondente; para mudar a base de novas classes,
edite `default_retention`.

Aulas de disponibilidade Taikai podem ser sobrescritas de forma independente
através de `torii.da_ingest.replication_policy.taikai_availability`:

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

- Torii substitui o `RetentionPolicy` fornecido pelo usuário pelo perfil imposto
  antes do chunking ou da emissão de manifesto.
- Manifestos pré-construídos que declaram um perfil de retenção divergente são
  rejeitados com `400 schema mismatch` para que clientes obsoletos não possam
  enfraquecer o contrato.
- Cada evento de override e logado (`blob_class`, política enviada vs novidades)
  para exportar chamadores não conformes durante o lançamento.

Veja [Plano de Ingestão de Disponibilidade de Dados](ingest-plan.md) (Lista de verificação de validação) para
o portão atualizado cobrindo execução de retenção.

## Workflow de re-replicação (seguimento DA-4)

A execução de retenção e apenas o primeiro passo. Operadores também devem
provar que manifestos ao vivo e ordens de replicação permanecem alinhados à política
configurado para que SoraFS possa replicar blobs fora de conformidade de formato
automático.

1. **Observe o desvio.** Torii emite
   `overriding DA retention policy to match configured network baseline` quando
   um chamador submete valores de retenção desatualizados. Combine esse log com a
   telemetria `torii_sorafs_replication_*` para detectar falta de réplicas ou
   realoca os atrasados.
2. **Diferença de intenção versus réplicas ao vivo.** Use o novo helper de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```O comando carrega `torii.da_ingest.replication_policy` da configuração
   Fornece, decodifica cada manifesto (JSON ou Norito), e opcionalmente faz match
   as cargas úteis `ReplicationOrderV1` por resumo do manifesto. O resumo sinaliza duas
   condições:

   - `policy_mismatch` - o perfil de retenção do manifesto diverge da política
     imposta (isto não deveria ocorrer a menos que Torii esteja mal configurado).
   - `replica_shortfall` - a ordem de replicação ao vivo solicita menos réplicas do
     que `RetentionPolicy.required_replicas` ou fornece menos atribuições do que
     o alvo.

   Um status de saida não zero indica um déficit ativo para que a automação de
   CI/on-call pode paginar imediatamente. Anexo do relatório JSON ao pacote
   `docs/examples/da_manifest_review_template.md` para votos do Parlamento.
3. **Dispare re-replicacao.** Quando um auditório reporta um déficit, emita um
   novo `ReplicationOrderV1` via as ferramentas de governança descritas em
   [Mercado de capacidade de armazenamento SoraFS](../sorafs/storage-capacity-marketplace.md)
   e andamos de auditório novamente até que o conjunto de réplicas foi convertido. Substituições de parágrafo
   de emergência, pareado a saida da CLI com `iroha app da prove-availability` para
   que SREs podem referenciar o mesmo resumo e evidência PDP.

A cobertura de regressão vive em `integration_tests/tests/da/replication_policy.rs`;
a suite envia uma política de retenção divergente para `/v1/da/ingest` e verifica
que o manifesto buscado expoe o perfil imposto em vez da intenção do chamador.