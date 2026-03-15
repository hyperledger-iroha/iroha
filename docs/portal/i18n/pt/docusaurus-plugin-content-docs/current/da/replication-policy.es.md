---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflexo `docs/source/da/replication_policy.md`. Mantenha ambas as versões em
:::

# Política de replicação de disponibilidade de dados (DA-4)

_Estado: En progreso -- Responsáveis: Core Protocol WG / Storage Team / SRE_

O pipeline de ingestão DA agora aplica objetivos de retenção deterministas para
cada classe de blob descrita em `roadmap.md` (fluxo de trabalho DA-4). Torii rechaza
persistir envelopes de retenção desde que o chamador não coincida com o
política definida, garantindo que cada nodo validador/almacenamiento retenha
o número necessário de épocas e réplicas não depende da intenção do emissor.

## Política por defeito

| Classe de blob | Retenção quente | Retenção de frio | Réplicas exigidas | Classe de armazenamento | Tag de governança |
|--------------|---------------|----------------|---------------------|-----------------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Default (todas as primeiras aulas)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Esses valores são incrustados em `torii.da_ingest.replication_policy` e aplicados a
todas as solicitações `/v1/da/ingest`. Torii reescrever manifestos com o perfil de
retenção imposta e emissão de uma advertência quando os chamadores entregam valores
não há coincidência para que os operadores detectem SDKs desactualizados.

### Aulas de disponibilidade Taikai

Os manifestos de treinamento Taikai (`taikai.trm`) declaram um
`availability_class` (`hot`, `warm`, ou `cold`). Torii aplica-se à política
correspondente antes do chunking para que os operadores possam escalar o
conteúdo de réplicas por stream sem editar a tabela global. Padrões:

| Classe de disponibilidade | Retenção quente | Retenção de frio | Réplicas exigidas | Classe de armazenamento | Tag de governança |
|-------------------------|---------------|----------------|---------------------|-----------------------------------|-------------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

As faixas faltantes usam `hot` por defeito para que as transmissões ao vivo
retengan la politica mas forte. Sobrescrever os padrões via
`torii.da_ingest.replication_policy.taikai_availability` si su red usa objetivos
diferentes.

## Configuração

A política vive abaixo de `torii.da_ingest.replication_policy` e expõe um modelo
*default* mas um conjunto de substituições por classe. Os identificadores de classe não
filho sensível a mayus/menos e aceitar `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para extensões aprovadas pelo governo.
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

Deixe o bloco intacto para usar os padrões listados abaixo. Para suportar
uma classe, atualize a substituição correspondente; para mudar a base de novas
aulas, edite `default_retention`.

As aulas de disponibilidade de Taikai podem ser escritas como independentes
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

- Torii substitui o `RetentionPolicy` fornecido pelo usuário com o perfil
  imposto antes do chunking ou da emissão de manifestos.
- Os manifestos pré-construídos que declaram um perfil de retenção distinto se
  rechazan con `400 schema mismatch` para que clientes obsoletos não possam
  debilitar o contrato.
- Cada evento de substituição se registra (`blob_class`, política enviada vs novidades)
  para exponer callers não conformes durante a implementação.

Ver [Plano de ingestão de disponibilidade de dados](ingest-plan.md) (checklist de validação)
para o portão atualizado que cobre a aplicação de retenção.

## Fluxo de re-replicação (seguimento DA-4)

A aplicação da retenção é apenas o primeiro passo. Os operadores também devem
Probar que os manifestos en vivo e as ordens de replicação sejam mantidas
alinhados com a política configurada para que SoraFS possa replicar novamente blobs
fora de cumprimento de forma automática.1. **Vigile el drift.** Torii emite
   `overriding DA retention policy to match configured network baseline` quando
   um chamador envia valores de retenção desactualizados. Empareje ese log con
   la telemetria `torii_sorafs_replication_*` para detectar faltantes de réplica
   o reimplanta demorados.
2. **Diferença de intenção versus réplicas ao vivo.** Use o novo auxiliar de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   El carga comando `torii.da_ingest.replication_policy` desde a configuração
   provista, decodifica cada manifesto (JSON ou Norito), e opcionalmente empareja
   payloads `ReplicationOrderV1` por resumo do manifesto. O currículo marca dos
   condições:

   - `policy_mismatch` - o perfil de retenção do manifesto divergente da
     política impuesta (isto não deveria ocorrer salvo que Torii este mal
     configurado).
   - `replica_shortfall` - a ordem de replicação ao vivo solicita menos réplicas
     que `RetentionPolicy.required_replicas` o entrega menos atribuições que su
     objetivo.

   Um status de saída no zero indica uma falta de ativo para a automatização
   CI/on-call pode paginar imediatamente. Adicionando o relatório JSON ao pacote
   `docs/examples/da_manifest_review_template.md` para votos do Parlamento.
3. **Dispare re-replicacion.** Quando os auditórios relatam uma falta, emita uma
   nova `ReplicationOrderV1` através das ferramentas de governo descritas em
   [Mercado de capacidade de armazenamento SoraFS](../sorafs/storage-capacity-marketplace.md)
   e volte a executar o auditório até que o conjunto de réplicas seja convertido. Pará
   substituições de emergência, compare a saída da CLI com `iroha app da prove-availability`
   para que SREs possam referenciar o mesmo resumo e evidência PDP.

A cobertura de regressão vive em
`integration_tests/tests/da/replication_policy.rs`; la suite envia uma política de
retenção não coincidente com `/v1/da/ingest` e verifica se o manifesto foi obtido
exponha o perfil imposto no lugar da intenção do chamador.