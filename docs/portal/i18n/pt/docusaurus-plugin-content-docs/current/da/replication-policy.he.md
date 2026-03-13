---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/da/replication-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1262a33c965539921fe1e85fb9222117df4d680bae656e59d67ea4e42507c3
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Espelha `docs/source/da/replication_policy.md`. Mantenha as duas versoes em
:::

# Politica de replicacao de Data Availability (DA-4)

_Status: Em progresso -- Responsaveis: Core Protocol WG / Storage Team / SRE_

O pipeline de ingest DA agora aplica metas deterministicas de retencao para cada
classe de blob descrita em `roadmap.md` (workstream DA-4). Torii recusa persistir
envelopes de retencao fornecidos pelo caller que nao correspondem a politica
configurada, garantindo que cada node validador/armazenamento retenha o numero
requerido de epocas e replicas sem depender da intencao do emissor.

## Politica padrao

| Classe de blob | Retencao hot | Retencao cold | Replicas requeridas | Classe de armazenamento | Tag de governanca |
|---------------|--------------|---------------|---------------------|-------------------------|-------------------|
| `taikai_segment` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 dias | 3 | `cold` | `da.governance` |
| _Default (todas as demais classes)_ | 6 horas | 30 dias | 3 | `warm` | `da.default` |

Esses valores sao embutidos em `torii.da_ingest.replication_policy` e aplicados
a todas as submissions `/v2/da/ingest`. Torii reescreve manifests com o perfil
de retencao imposto e emite um warning quando callers fornecem valores divergentes
para que operadores detectem SDKs desatualizados.

### Classes de disponibilidade Taikai

Manifests de roteamento Taikai (`taikai.trm`) declaram `availability_class`
(`hot`, `warm`, ou `cold`). Torii aplica a politica correspondente antes do
chunking para que operadores possam escalar contagens de replicas por stream sem
editar a tabela global. Defaults:

| Classe de disponibilidade | Retencao hot | Retencao cold | Replicas requeridas | Classe de armazenamento | Tag de governanca |
|---------------------------|--------------|---------------|---------------------|-------------------------|-------------------|
| `hot` | 24 horas | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 dias | 3 | `cold` | `da.taikai.archive` |

Hints ausentes usam `hot` por padrao para que transmissoes ao vivo retenham a
politica mais forte. Substitua os defaults via
`torii.da_ingest.replication_policy.taikai_availability` se sua rede usar
alvos diferentes.

## Configuracao

A politica vive sob `torii.da_ingest.replication_policy` e expoe um template
*default* mais um array de overrides por classe. Identificadores de classe nao
diferenciam maiusculas/minusculas e aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, ou `custom:<u16>` para extensoes aprovadas por governanca.
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

Deixe o bloco intacto para rodar com os defaults acima. Para endurecer uma
classe, atualize o override correspondente; para mudar a base de novas classes,
edite `default_retention`.

Classes de disponibilidade Taikai podem ser sobrescritas de forma independente
via `torii.da_ingest.replication_policy.taikai_availability`:

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

## Semantica de enforcement

- Torii substitui o `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  antes do chunking ou da emissao de manifest.
- Manifests preconstruidos que declaram um perfil de retencao divergente sao
  rejeitados com `400 schema mismatch` para que clientes obsoletos nao possam
  enfraquecer o contrato.
- Cada evento de override e logado (`blob_class`, politica enviada vs esperada)
  para expor callers nao conformes durante o rollout.

Veja [Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) para
o gate atualizado cobrindo enforcement de retencao.

## Workflow de re-replicacao (seguimento DA-4)

O enforcement de retencao e apenas o primeiro passo. Operadores tambem devem
provar que manifests live e ordens de replicacao permanecem alinhados a politica
configurada para que SoraFS possa re-replicar blobs fora de conformidade de forma
automatica.

1. **Observe o drift.** Torii emite
   `overriding DA retention policy to match configured network baseline` quando
   um caller submete valores de retencao desatualizados. Combine esse log com a
   telemetria `torii_sorafs_replication_*` para detectar falta de replicas ou
   redeploys atrasados.
2. **Diff intent vs replicas live.** Use o novo helper de auditoria:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   O comando carrega `torii.da_ingest.replication_policy` da configuracao
   fornecida, decodifica cada manifest (JSON ou Norito), e opcionalmente faz match
   de payloads `ReplicationOrderV1` por digest de manifest. O resumo sinaliza duas
   condicoes:

   - `policy_mismatch` - o perfil de retencao do manifest diverge da politica
     imposta (isto nao deveria ocorrer a menos que Torii esteja mal configurado).
   - `replica_shortfall` - a ordem de replicacao live solicita menos replicas do
     que `RetentionPolicy.required_replicas` ou fornece menos atribuicoes do que
     o alvo.

   Um status de saida nao zero indica um shortfall ativo para que a automacao de
   CI/on-call possa paginar imediatamente. Anexe o relatorio JSON ao pacote
   `docs/examples/da_manifest_review_template.md` para votos do Parlamento.
3. **Dispare re-replicacao.** Quando a auditoria reportar um shortfall, emita um
   novo `ReplicationOrderV1` via as ferramentas de governanca descritas em
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   e rode a auditoria novamente ate que o set de replicas converja. Para overrides
   de emergencia, emparelhe a saida da CLI com `iroha app da prove-availability` para
   que SREs possam referenciar o mesmo digest e evidencia PDP.

A cobertura de regressao vive em `integration_tests/tests/da/replication_policy.rs`;
a suite envia uma politica de retencao divergente para `/v2/da/ingest` e verifica
que o manifest buscado expoe o perfil imposto em vez da intencao do caller.
